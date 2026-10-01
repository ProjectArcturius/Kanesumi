# Kanesumi 元素树（Element Tree）设计 —— 2026-09-30 立

> **地位**：本文件合并并取代 `ROADMAP.md` 的 M2（布局接管）、M3（交互协调）、M4（保留树 + 失效传播）
> 三个阶段的实现路径。三者的**目标与验收不变**，变的是「怎么做」：不再分别给 47 个控件补
> `measure()`/`hit_test()`、给 App 补焦点登记、给 `RetainedScene` 补增量，而是一次性引入
> **框架持有的保留元素树**，让这三件事成为框架的职责，而不是每个控件、每个 App 各自的职责。
>
> 维护者 2026-09-30 批准方向。实现分工：框架骨架与参照控件由调度者（Claude）写；
> 控件批量迁移、测试、Gallery / Ether 应用移植由工人（opencode）按本文模板执行。

---

## §Ⅰ 为什么：病灶在「没有树」

2026-09-30 实测（`Rect::new` 字面量 / 手写命中函数 / 布局引擎调用）：

| 消费方 | 手写坐标 | 手写命中 | 布局引擎 |
|---|---|---|---|
| Ether `settings/kanesumi_topbar.rs` | 49 | 10 | 0 |
| Ether `settings/kanesumi_window.rs` | 34 | 14 | 少量 |
| Ether `librarian/kanesumi_app/mod.rs` | 30 | 9 | 少量 |
| `kanesumi-gallery/src/app.rs`（3319 行） | 38 | 47 | — |

根因不在引擎（排版、Measure/Arrange、CPU 局部光栅化都是真实现，见 `MATURITY_AUDIT` §Ⅰ），
而在**没有一棵由框架持有的树**。于是：

- 一个屏幕的坐标在「布局常量 / 控件 render / 命中函数」三处各写一遍（审计「开发效率」痛点）；
- 焦点只能每帧由 App 手工登记（`FocusRing`，登记顺序即 Tab 顺序，漏登即不可达）；
- 悬停/按下/捕获由 App 自持（`hover_signature` / `damage_hint` / `needs_redraw` 三个钩子都要 App 手算）；
- `layout()` 是无状态函数，每层重复量测子树（嵌套深度指数级），且结果不跨帧复用。

对照：Ncrust Windows（`Ncrust/windows/docs/KANESUMI_XAML.md` §定位）把**文本、IME、滚动、虚拟化、
键盘导航、UI 自动化**全部交给 XAML 框架，Kanesumi.Xaml 只负责令牌与样式。Kanesumi 自绘路线
**必须自己提供这一层** —— 这一层就是本文件。

## §Ⅱ 对标与取舍

| 来源 | 取 | 不取 |
|---|---|---|
| **XAML `UIElement` / `FrameworkElement`** | Measure/Arrange 契约；框架级布局属性（Margin / Alignment / Min/Max / 固定尺寸）；`InvalidateMeasure`/`InvalidateArrange` 失效传播；路由事件（冒泡）；指针捕获；`FocusManager` + Tab 顺序由树序派生；`PopupRoot` 覆盖层；UIA 节点 | 依赖属性 / 附加属性系统（`ROADMAP` §Ⅰ「不做」2）；模板 / 样式字典；数据绑定引擎 |
| **masonry（Rust，xilem 的保留层）** | arena + id 的所有权模型；每节点状态位（`needs_layout`/`needs_paint`/`request_anim`）；`*Ctx` 上下文对象；AccessKit 接入点 | vello GPU 渲染依赖（我们有 CPU 光栅 + dmabuf 直通，专为 Ether 定制） |
| **现有 Kanesumi** | `canvas::layout` 的分配算法（grow/shrink/min_main，作为 `Stack` 容器的实现）；`Scene` 命令模型；`TextEngine`；Sokuou 动画；harness 外壳全部 | `Decl`（5 种元素的原型，退役为测试夹具）；`RetainedScene`（被本文 §Ⅴ 取代） |

**为什么不直接换用现成框架**（masonry / iced / slint）：会废掉为 Ether 定制的 CPU 光栅 + dmabuf
直通、layer-shell 多表面、IME 引擎宿主等 harness 能力；且 Kanesumi 的控件库（47 个、有一手源规格）
正是最值钱的部分。本方案只补「树」这一层，其余全部复用。

## §Ⅲ 核心模型

新 crate **`kanesumi-element`**（依赖 core + canvas + anim；**不依赖 harness**，跨平台纯逻辑可测）。

```
kanesumi-core ← kanesumi-canvas ← kanesumi-element ← kanesumi-controls ← kanesumi-harness
                                   （树 / 布局 / 路由 / 焦点）  （控件实现 Widget）   （TreeHost 接外壳）
```

### Ⅲ.1 所有权：arena + `WidgetId`

```rust
pub struct Tree {
    nodes: Vec<Option<Node>>,        // arena；槽位复用
    generations: Vec<u32>,           // WidgetId = (index, generation)，防悬垂 id
    root: WidgetId,
    overlay: WidgetId,               // 覆盖层根（弹层 / 菜单 / 对话框），画在内容之上
    focus: Option<WidgetId>,
    hovered: Vec<WidgetId>,          // 指针下的祖先链（根 → 叶）
    captured: Option<WidgetId>,      // 指针捕获者
    actions: Vec<(WidgetId, Action)>,
    damage: Option<Rect>,
    /* 帧时钟、TextEngine 句柄等 */
}

struct Node {
    widget: Option<Box<dyn Widget>>, // 回调期间 take() 出来，回调结束放回（绕开借用冲突）
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    props: LayoutProps,              // 框架级布局属性（§Ⅳ.2）
    flags: Flags,                    // needs_measure / needs_arrange / needs_paint / anim / disabled / hidden
    desired: Size, last_available: Size,   // 量测缓存
    rect: Rect,                      // arrange 产物 —— 绘制 / 命中 / 弹层锚点 / IME caret 的唯一真源
    paint: Vec<SceneCommand>,        // 本节点自绘命令缓存（绝对坐标）
    painted_bounds: Rect,            // 上次绘制覆盖范围（算损伤用）
    state: ControlStates,            // hovered / pressed / focused / disabled —— 由框架维护
}
```

控件**不持有子控件**，只通过上下文访问 `ctx.children()`。这就是 XAML「元素不自知位置」
与 masonry「WidgetPod」的同一件事，只是用 arena 表达，避免 Rust 里父子互借。

### Ⅲ.2 `Widget` trait

```rust
pub trait Widget: Any {
    /// 量测：返回期望尺寸（内容 + 内边距；**不含** Margin —— Margin 由框架加）。
    /// 容器用 `ctx.measure_child(id, avail)` 量子节点（有缓存，约束不变即命中）。
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size;

    /// 排列：叶子默认什么都不做；容器对每个子调用 `ctx.arrange_child(id, slot)`。
    fn arrange(&mut self, _ctx: &mut ArrangeCtx, _rect: Rect) {}

    /// 自绘（只画自己；子节点由框架按树序接着画）。可读 `ctx.state()`（hover/pressed/focused）。
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene);

    /// 路由事件（冒泡：命中目标 → 各级祖先）。`ctx.set_handled()` 截停冒泡。
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    /// 动画 tick（只在 `ctx.request_anim_frame()` 之后被调用；稳态自动停）。
    fn update(&mut self, _ctx: &mut UpdateCtx, _dt: f64) {}

    // —— 以下为有默认值的策略钩子 ——
    fn focusable(&self) -> bool { false }                      // 占 Tab 位
    fn hit_test(&self, rect: Rect, pos: Point) -> bool { rect.contains(pos) }
    fn clips_children(&self) -> bool { true }                  // 容器裁剪（§Ⅳ.3）
    fn paint_overflow(&self) -> Insets { Insets::ZERO }        // 画出 rect 的显式许可（§Ⅳ.3）
    fn ime(&self, _rect: Rect) -> Option<ImeRequest> { None }  // 聚焦时外壳据此开 text-input
    fn accessibility(&self) -> Option<AccessInfo> { None }     // §Ⅸ 预留
    fn type_name(&self) -> &'static str;                       // 调试 / 诊断
}
```

**关键约束**：`measure` 与 `paint` 必须消费同一份排版结果（`COMPOSITION.md` 契约 7）。控件若需在
量测时缓存 `TextEngine` 布局，存在自身字段里，`paint` 直接用；属性变化时经 `ctx` 置失效。

## §Ⅳ 布局

### Ⅳ.1 失效与缓存

- `tree.edit::<T>(id, |w, ctx| ...)` 是 App 修改控件的**唯一入口**；闭包里调用
  `ctx.invalidate_measure()` / `invalidate_arrange()` / `invalidate_paint()` 声明影响面。
  未声明 → 保守按 measure 处理（宁可多算，不可漏画）。
- `invalidate_measure` 沿父链上传直到根（或直到一个尺寸由父决定且期望不变的节点 —— 第二期优化）。
- 帧内顺序：**先布局（仅脏子树）→ 再绘制（仅脏节点）→ 合成 Scene + 损伤**。
- 量测缓存键 = `available`；约束不变且节点不脏 → 直接返回 `desired`。
  这一条就把现 `layout()` 的指数级重复量测降为每帧每节点至多一次。

### Ⅳ.2 框架级布局属性（`LayoutProps`，对应 `FrameworkElement`）

| 属性 | 语义 |
|---|---|
| `margin: Insets` | 外边距，框架在 measure 时减、arrange 时扣 |
| `width/height: Option<f32>` | 固定尺寸（XAML `Width`/`Height`） |
| `min/max: Size` | 夹紧（XAML `MinWidth`…） |
| `h_align / v_align: Align` | `Stretch`（默认）/ `Start` / `Center` / `End` —— 在父分配的槽位内定位 |
| `grow / shrink / min_main` | 只对 `Stack` 父有效（沿用 `canvas::layout` 算法语义） |
| `visible: bool` | `false` = 不量测、不绘制、不命中、不可聚焦（XAML `Collapsed`） |

### Ⅳ.3 保险机制（M2 的核心验收，框架强制而非约定）

1. **`rect ⊆ slot`**：`arrange_child` 在父给的槽位内按 margin/align/min/max 解析最终矩形，
   结果**一律夹紧进槽位**。子期望大于槽位时被压缩，不会越界 —— 「按钮飞出窗口」在结构上不可能。
2. **裁剪默认开**：`clips_children() == true` 的节点，其子树绘制与命中都被裁到自身矩形（命中与
   绘制共用同一裁剪链，`COMPOSITION` 契约 6/12）。
3. **越界必须显式声明**：控件要画到自身 rect 外（徽标外溢、焦点环、描边外扩）必须通过
   `paint_overflow()` 声明外扩量；框架据此扩展损伤矩形，且**仍受祖先裁剪约束**。
   `ROADMAP` M2-1 勘察列出的三类例外里，「浮层」**不走这条**，而是走覆盖层（§Ⅴ.2）。
4. **调试断言**（`debug_assertions`）：arrange 后逐节点校验 `rect ⊆ parent.rect`、非 NaN、非负；
   paint 后校验命令未越过 `rect ⊕ paint_overflow`。违反即 `log::error!` + 诊断落盘（不 panic）。

### Ⅳ-bis 实现钩子（虚拟化容器，2026-10-01 立）

长列表不能把全部行插成节点（Librarian 一个目录几千项、Launcher 全部应用几百项）。对照 WinUI
`ItemsRepeater`：容器在 **measure 里按「有效视口」向 `ElementFactory` 要元素、回收离开视口的
元素**（参 `reference/microsoft-ui-xaml/dev/Repeater/`）。元素树用 `Widget::realize` 钩子表达
同一件事 —— 容器的**子节点不是数据，而是数据在视口内的投影**。

```rust
pub trait Widget {
    /// 布局前的子节点实现钩子（虚拟化容器用）。默认不做事。
    fn realize(&mut self, _ctx: &mut RealizeCtx) {}
    /// 是否需要 realize（避免框架每帧对所有节点做可实现性判断）。
    fn wants_realize(&self) -> bool { false }
}
```

- **时机**：`Tree::frame` 在 **measure 之前**，对「`wants_realize()` 为真且被标记
  `needs_realize`」的可见节点调用一次（先序）。调用后标记清除 —— 回调里 `insert` / `remove` /
  `invalidate_measure` 再次产生的标记视为本帧已消费（避免每帧反复 realize）。
- **标记来源**：`invalidate_measure`（子树 / 数据变了）、`insert`（新子节点）、尺寸变化、
  以及显式的 `Tree::invalidate_realize` / `EventCtx::invalidate_realize` /
  `UpdateCtx::invalidate_realize` / `ArrangeCtx::invalidate_realize`（滚动偏移变化、视口变化）。
- **`RealizeCtx` 能做的**：读自身上一帧 `rect`、读 `surface` / 主题 / 引擎；在**自身下**
  增删子节点（`insert_child` / `insert_child_boxed` / `remove_child` / `children`）、
  `edit::<T>(child, ..)` 改子控件、`set_child_visible`、`invalidate_measure`。**不能碰自身子树
  以外的节点**（也不能编辑自身 —— 回调期间控件实例已被框架取出）。
- **回收语义**：被 `remove_child` 移除的子节点走 `Tree::remove` 既有清理路径（焦点 / 指向 /
  按下 / 捕获 / 悬停 / 弹层 owner / 动画 / 定时器一并断开）。**焦点节点不回收**（XAML 同样钉住
  焦点元素）—— 容器的 `RealizeCtx::is_focus_related` 判定，焦点所在行留在原位直到失焦。
- **保险机制仍成立**：`clips_children` + `scrolls_children` 让「子矩形 ⊆ 父矩形」断言豁免滚动
  容器；被回收的行以 `visible = false` 保留在子节点列表里（不参与量测 / 绘制 / 命中），池上限
  = 可见数，超出的行才真正删除。

**首个消费者**：`kanesumi-controls/src/items_repeater.rs` 的 `ItemsRepeater`（纵向等高 Stack /
等宽 UniformGrid）—— 几何复用 `MetroRepeater`、滚动状态复用 `MetroScrollView`，realize 按
「可见范围 + 上下 overscan」维护「索引 → 节点」映射并按数据键回收复用。示例见
`kanesumi-gallery/examples/virtual_list.rs`（一万行），样张 `docs/research/items_repeater/`。

## §Ⅴ 绘制与损伤

### Ⅴ.1 逐节点命令缓存

- 每个节点缓存自己上一次 `paint` 产出的命令（绝对坐标）。只有 `needs_paint` 的节点重画。
- 帧 Scene = 按树序（先序）拼接各节点缓存，容器在子节点前后插入 `PushClip`/`PopClip`。
  拼接是 `Vec` 拷贝，远便宜于重跑控件绘制与排版。
- **损伤自动产出**：本帧重画节点的 `旧 painted_bounds ∪ 新 painted_bounds` 求并 →
  交给外壳 `App::damage_hint`。App 不再手算损伤（`ROADMAP` M4-4 结案）。
- 动画只动视觉（`AnimationRules`）：动画 tick 只能 `invalidate_paint`，**禁止**在 `update` 里
  `invalidate_measure`（debug 断言）。这把「动画只动视觉属性」从约定变成结构保证。

### Ⅴ.2 覆盖层（对应 XAML `PopupRoot`）

- `Tree` 有两棵根：`root`（内容）与 `overlay`（弹层）。覆盖层在内容之后绘制、之前命中。
- 下拉菜单 / 选择器浮层 / TeachingTip / 对话框作为 overlay 的子节点挂载，锚点取触发器的 `rect`
  （同一布局产物，`COMPOSITION` 契约 3），位置用现有 `place_popup` 计算。
- 覆盖层有内容时启用 **LightDismiss**：点在覆盖层外 → 关闭最上层弹层并吞掉该次点击（参
  `CONTEXT_MENU_SPEC` §Ⅵ）；模态对话框改为「吞掉但不关闭」+ 焦点陷阱。
- **矮表面问题**（TopBar 30px / Dock 上的菜单）：第一期仍画在本表面内（受表面尺寸限制，与现状同）；
  第二期把 overlay 的顶层节点映射到 harness 的 `floating_layers` / 真 `xdg_popup`（`ROADMAP` M1-6 / P1-6）。
  **本期不改 `place_popup` 签名**（`ROADMAP` §Ⅴ-2）。

## §Ⅵ 输入

框架把外壳的 `InputEvent` 翻译为元素级 `Event` 并路由；App 不再写命中函数。

| 事件 | 路由 |
|---|---|
| `PointerDown/Up/Move` | 命中测试（覆盖层 → 内容，后画者优先，沿裁剪链）→ 目标 → 冒泡到祖先。**按下即捕获**：直到释放，Move/Up 都投给捕获者（拖拽 / Slider / 文本选择，`ROADMAP` M3-2） |
| `PointerEnter/Leave` | 框架比对前后 hover 链，差集发 Enter/Leave；同时维护 `state.hovered` 并自动 `invalidate_paint` —— **`hover_signature` 退役** |
| `Click` | 框架合成：同一目标上 Down→Up 且未移出 → `Click`（控件多数只需要这一个） |
| `ContextRequested` | 右键按下时框架记录命中目标（`ContextTarget` 一等语义，`ROADMAP` M3-4）；菜单只能读它 |
| `Scroll` | 指针下目标冒泡，首个处理者（ScrollViewer）截停 |
| `Key` / `Text` / `Preedit` / `Commit` | 投给焦点节点并冒泡；Tab/Shift+Tab 在冒泡到根仍未处理时由框架消费 |
| `FocusIn/Out` | 框架聚焦变化时发出 |

**焦点**（`ROADMAP` M3-1）：Tab 顺序 = 树的先序中 `focusable && !disabled && visible` 的节点，
**由树派生，不需要每帧登记**；覆盖层有模态内容时 Tab 被限制在覆盖层内（焦点陷阱）。
点击可聚焦控件即聚焦；焦点节点被移除/隐藏/禁用时焦点移到下一个可聚焦节点。
`FocusRing` 保留给未迁移的旧 App，迁移完成后删除。

**IME**：焦点节点的 `ime(rect)` 返回 `Some` → 外壳开 text-input，caret 矩形由节点 `rect` 派生。

## §Ⅶ 视觉状态（对应 `VisualStateManager`）

- 框架维护 `ControlStates { hovered, pressed, focused, disabled }`，变化时自动 `invalidate_paint`。
- 过渡动画用 **`VisualState` 辅助结构**（element crate 提供，内部是 Sokuou `Progress`）：
  控件持有 `hover: VisualState, press: VisualState`，在 `paint` 里读 `ctx.state()` 设定目标、
  读当前进度画插值色；未到稳态时自动 `request_anim_frame`。时长/缓动取 `kanesumi-anim` 预设。
- 这给 `ROADMAP` M6-1（按下反馈）提供了**统一挂载点**：先裁定语义，再在 `VisualState` 一处接线，
  17 个交互控件自动获得，不需逐个改。

## §Ⅷ App 接线（`TreeHost`）

harness 新增 `TreeHost<A: TreeApp>`，**实现现有 `App` trait**，因此**外壳零改动**、新旧 App 可并存：

```rust
pub trait TreeApp {
    fn config(&self) -> &AppConfig;
    fn build(&mut self, tree: &mut Tree);                    // 启动时建树
    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action);  // 控件动作回调
    fn tick(&mut self, _tree: &mut Tree, _dt: f64) {}        // 非控件状态（时钟、后台结果）
    /* theme / set_theme / app_menu 等照旧透传 */
}
```

`TreeHost` 把 `App::render/handle_input/update/needs_redraw/damage_hint/focus_move/ime_focus`
全部实现为树操作。`Action` 是 `Box<dyn Any>`（控件定义自己的动作枚举，App `downcast`）。
这是「code-behind」模型：App 持有控件 id，响应动作，经 `tree.edit` 改控件。

**声明式层**（`Decl` 的继承者，keyed reconciliation，`ROADMAP` M4-2）**放到第二期**，建在树之上：
先证明树能承载 Settings，再决定是否需要声明式语法糖。不在第一期同时造两层。

## §Ⅸ 无障碍预留（`ROADMAP` M7）

`accessibility()` 返回角色 / 名称 / 值 / 状态。第一期只定接口并在参照控件上实现；
第二期接 **AccessKit**（Rust 生态标准，Linux 侧桥接 AT-SPI），树的 id 即 AccessKit 节点 id。
这是自绘路线相对 UWP 最大的结构缺口（审计 §Ⅲ P2-9），有了树它才第一次**可做**。

## §Ⅹ 实施分期与分工

| 期 | 内容 | 执行 | 验收 |
|---|---|---|---|
| **E1** 骨架 | `kanesumi-element`：Tree/Node/Widget/Ctx、布局（缓存+失效+保险机制）、绘制缓存+损伤、命中/路由/捕获/hover/Click、焦点+Tab、覆盖层+LightDismiss、`VisualState`；内置容器 `Stack`（行/列）、`Border`（背景/描边/内边距）、`Label`；**测试夹具 `TestHarness`**（无外壳驱动指针/键盘、断言矩形/命中/损伤） | 调度者 | 单元测试全绿；夹具能跑通「点击→动作→编辑→重绘损伤」闭环 |
| **E2** 参照 | `MetroButton`、`MetroCheckBox`、`MetroTextBox` 三个控件实现 `Widget`（覆盖：纯点击 / 带状态切换 / 焦点+IME）；harness `TreeHost`；gallery 一页迁移为参照页 | 调度者 | 参照页在 Arch 上真机跑通（Plasma 嵌套），键盘可走完 |
| **E3** 控件批量 | 其余控件按 E2 模板迁移，每批 4~6 个，每个控件附「矩形外 1px 不可命中」「绘制 ⊆ rect⊕overflow」「键盘可达」通用测试 | 工人（两机并行） | 每批 `cargo test`/`clippy` 基线；调度者逐批审阅 |
| **E4** 容器 | `ScrollViewer`（裁剪+滚动+命中三合一）、`ItemsRepeater` 虚拟化（keyed 回收）、`Grid`（`structure/grid` 接入）；覆盖层映射到 `floating_layers` | 调度者设计 + 工人填充 | `ROADMAP` M5 验收 |
| **E5** 应用 | Gallery 全量 → Settings 窗口 → TopBar → Librarian → Launcher（egui 形态直接重写） | 工人（调度者审阅），每个应用一份迁移任务书 | 手写 `Rect::new` 降到只剩协议/硬件几何 |
| **E6** 无障碍 | AccessKit 接入 | 调度者 | Orca 能读出 Settings |

### 进度（2026-09-30 收工时）

| 期 | 状态 | 说明 |
|---|---|---|
| E1 | ✅ | `kanesumi-element`；21 项契约测试 |
| E2 | ✅ | Button / CheckBox / TextBox 参照；`TreeHost`；`tree_demo` 在 Arch Plasma 真机截图核对 |
| E3 | 🔵 24/47 | 已迁：button check_box text_box · icon_button switch slider radio_buttons rating_control · text surface info_badge progress(Bar+Ring) person_picture info_bar animated_icon · password_box number_box metro_tile pips_pager pager_control title_bar · tab_row swipe_control color_picker。操作规范 `docs/ELEMENT_MIGRATION.md` |
| E4 | 🔵 | ✅ 定时器、`paint_after`、滚动钩子 / `bring_into_view`、隐藏动画暂停、`MetroScrollView` 滚动容器、**`ItemsRepeater` 虚拟化（§Ⅳ-bis realize 钩子；keyed 回收）**。⬜ 弹层类控件（下方）、覆盖层映射 `floating_layers` |

**E3 余下（需 E4 能力，调度者先定模板再派工）**：
- 弹层类（走覆盖层 `open_popup`）：dropdown_menu、selector_flyout、drop_down_button、split_button、menu_bar、context_menu、command_bar_flyout、teaching_tip、auto_suggest_box、breadcrumb_bar、dialog；
- 滚动 / 虚拟化类：list、tree_view、tab_view、navigation_view（底层 `ItemsRepeater` / `realize` 钩子已就绪，见 §Ⅳ-bis）；
- 特殊：expander（展开改变高度 —— 需裁定「布局动画」语义）、candidate_window（IME 引擎宿主专用表面）、
  two_pane_view / parallax_view / repeater（布局容器，非控件）。

**迁移中顺带修复的旧 bug**（均有回归测试）：光标从不闪烁（render 每帧重置闪烁相位）、密码框未聚焦时显示明文、
色板自然尺寸下预览与 Hex 行越界、IME 空提交误报 TextChanged、librarian 自 39b8574 起无法编译。

旧 API 在 E5 结束前保持可用（`LayoutLeaf` / `Decl` / `FocusRing` / 控件的旧 `render(theme, engine, rect, scene)`），
E5 结束后统一删除并记入 `CANON_VS_TEMPORARY`。

## §Ⅺ 本期明确不做

1. 声明式语法 / reconciler（第二期，建在树之上）。
2. 依赖属性、样式字典、数据绑定。
3. 变换（旋转 / 缩放）进入布局或命中 —— 变换只作用于绘制（`COMPOSITION` 契约 12）。
4. 多线程布局 / 绘制。
5. 修改 harness 外壳主循环（`TreeHost` 走现有 `App` trait）。
