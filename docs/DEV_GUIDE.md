# Kanesumi 开发者文档（DEV_GUIDE）

> 面向在 Kanesumi 上**开发应用**与**扩展控件库**的开发者。工程指引/构建命令见
> `CLAUDE.md`；设计语言正典见 Ether monorepo 仓根 `KANESUMI_DESIGN.md`；
> 控件规格见 `docs/CONTROL_SPEC.md`。本文档回答「怎么写、怎么跑、怎么调」。

---

## 一、这是什么

**Kanesumi（矩隅）**是 Ether 扇区的**原生应用 Runtime**（Rust SDK）。它把
「设计语言 tokens + 控件库 + 布局引擎 + 渲染管线 + 进程外壳」打包成一条
`App trait`：应用只描述**状态与绘制命令**，外壳负责 Wayland 连接、输入、
逐帧驱动与渲染。

一个 Kanesumi 应用 = 一个实现 **`TreeApp`** 的 Rust 结构体 + 一个 `TreeHost`。
新应用与迁移应用**一律走元素树**（§二，推荐写法）；旧手写几何的 `App` 接口仅维护存量
（见文末「附：旧 App 接口」）。

```
┌──────────────────────── 应用层（你的代码）────────────────────────┐
│  App trait 实现：状态 → App::render() → Scene 命令                 │
└──────────────────────────────┬───────────────────────────────────┘
                               │ Scene + TextEngine + MetroTheme
┌──────────────────────────────▼───────────────────────────────────┐
│  kanesumi-harness（进程外壳）                                      │
│  sctk Wayland 客户端 · ETHER_ROLE 分派 · 输入/IME · AppMenu · 渲染  │
└──────────────────────────────┬───────────────────────────────────┘
                               │ wgpu 离屏 → 读回 → wl_shm 提交
┌──────────────────────────────▼───────────────────────────────────┐
│  合成器（Ether / Plasma）—— 标准 Wayland 协议                      │
└──────────────────────────────────────────────────────────────────┘
```

---

## 二、元素树应用（推荐写法）

Ether 已规定**新应用 / 迁移应用一律走元素树**（参 Ether `docs/SYSTEM_APPS_PLAN.md` §Ⅰ）。
元素树是框架持有的保留树：应用只建树、挂控件、响应动作；命中 / 焦点 / 悬停 / 损伤 / IME
上下文全部由树负责。参照实现（先读这三个，照抄结构）：
`kanesumi-gallery/examples/tree_minimal.rs`（最小）、`tree_demo.rs`（表单 + 弹层对话）、
`virtual_list.rs`（虚拟化长列表）。

### 2.1 最小 TreeApp

下面这段与 `kanesumi-gallery/examples/tree_minimal.rs` 的 `TreeApp` 实现逐行同源：

```rust
use kanesumi_controls::{ButtonClicked, MetroButton};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Insets, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

struct Counter {
    config: AppConfig,
    theme: MetroTheme,
    value: u32,
    label: Option<WidgetId>,
}

impl TreeApp for Counter {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert(
            col,
            Label::new("最小元素树应用").style(self.theme.typography.title),
        );
        self.label = Some(tree.insert(
            col,
            Label::new(format!("计数：{}", self.value)).color(ThemeColor::OnSurfaceVariant),
        ));
        tree.insert(col, MetroButton::accent("加一"));
    }

    fn on_action(&mut self, tree: &mut Tree, _from: WidgetId, action: Action) {
        if action.is::<ButtonClicked>() {
            self.value += 1;
            if let Some(id) = self.label {
                tree.edit::<Label, _>(id, |l, _| l.text = format!("计数：{}", self.value));
            }
        }
    }
}
```

外壳接线（完整含 `--snapshot` 见示例文件）：

```rust
fn main() {
    let app = Counter { /* config / theme / value / label，见示例构造 */ };
    let host = kanesumi_harness::TreeHost::new(app);
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}
```

运行：`cargo run -p kanesumi-gallery --example tree_minimal`（Ether 会话内任意支持
xdg-shell 的合成器），或 `-- --snapshot out.png 1` 出 PNG。

### 2.2 TreeApp 钩子职责

| 钩子 | 何时用 |
|---|---|
| `config()` | 必填：返回 `AppConfig`（app_id / 标题 / 角色 / 逻辑尺寸）。 |
| `theme()` | 初始主题（默认 `MetroTheme::ether_dark()`）。之后系统主题由 `TreeHost` 直接推给树。 |
| `build(tree)` | 启动时建树一次：把页面挂到 `tree.root()` 下。 |
| `on_action(tree, from, action)` | 控件动作回调（`action.is::<T>()` / `downcast_ref`）；用 `tree.edit::<T>(id, ..)` 改控件。 |
| `tick(tree, dt)` | 非控件状态（时钟 / 后台任务结果）每帧写回树。 |
| `on_key(tree, key, mods)` | 应用级快捷键，**仅当没有任何控件**处理该键时到达（XAML `KeyboardAccelerator`）。 |
| `on_theme(theme)` | 系统主题变更通知；树已自行重排重画，仅当 App 自持颜色时才需要处理。 |
| `font_path()` | 指定字体路径（默认系统查找，不得静默回退）。 |
| `app_menu()` / `on_menu_command()` | 全局应用菜单（§5.5）。 |
| `floating_layers()` / `build_floating()` | 独立 layer 表面（Launcher / 面板），每层一棵独立树。 |

**定时器与重绘**：控件在 `paint` / `event` 里经 `ctx.request_timer(secs)` 申请低频唤醒
（光标闪烁、延时提示），`TreeHost` 驱动 `Tree::tick_timers`，等待期不占帧（`Tree::next_timer`
供外壳安排唤醒）。`Widget::paint_after` 是控件绘制阶段，在本节点子内容之后追加命令
（滚动条叠在内容上、焦点环等），由树统一拼接。动画经 `ctx.request_anim_frame()` 申请下帧，
且只许 `invalidate_paint`（动画只动视觉属性）。参 `ELEMENT_TREE.md` §Ⅳ.1 / §Ⅴ.1。

### 2.3 布局与容器

- **`LayoutProps`**（框架级属性）：`margin / width / height / min / max / h_align / v_align /
  grow / shrink / visible`。子矩形一律被框架夹紧进父槽位，「按钮飞出窗口」在结构上不可能
  （参 `ELEMENT_TREE.md` §Ⅳ.2、§Ⅳ.3）。`grow` 只对 `Stack` 父有效（吃掉剩余主轴）。
- **`Stack::row()` / `Stack::column()`**：唯一的一维布局容器，`with_spacing` 设间距。
- **`MetroScrollView`**：放不下就有滚动。子节点以视口宽、无界高量测，框架的容器裁剪同时
  约束绘制与命中（画不到、也点不到视口外的子节点）；滚轮与「焦点进入视口外后代时自动
  bring-into-view」内建。

  ```rust
  let sv = tree.insert(col, MetroScrollView::default());
  tree.insert(sv, Stack::column());   // 内容挂在滚动容器下
  ```

- **`ItemsRepeater`**：长列表虚拟化（Librarian 一个目录几千项、Launcher 全部应用几百项）。
  容器在 `measure` 前经 `Widget::realize` 按有效视口向 `ItemFactory` 要行、回收离开视口的行——
  **子节点是数据在视口内的投影，不是全量数据**（参 `ELEMENT_TREE.md` §Ⅳ-bis）。应用侧只需：

  ```rust
  let factory = SampleFactory { data };
  tree.insert_with(
      col,
      ItemsRepeater::stack(56.0, factory),
      LayoutProps { grow: 1.0, ..LayoutProps::default() },
  );
  ```

  容器自己实现 realize 钩子；钩子签名如下（框架机制，应用一般不必直接实现）：

  ```rust
  pub trait Widget {
      fn realize(&mut self, _ctx: &mut RealizeCtx) {}
      fn wants_realize(&self) -> bool { false }
  }
  ```

  完整示例见 `kanesumi-gallery/examples/virtual_list.rs`（一万行）。
- **`Grid`**：等宽几何 `UniformGrid` 在 `kanesumi-structure::grid`；元素树 `Grid` 容器属 E4，
  接入前用 `Stack` + `LayoutProps` 组合（参 `ELEMENT_TREE.md` §Ⅹ E4）。

### 2.4 颜色只用 `ThemeColor` 令牌
元素树页面**禁止写颜色字面量**：`Border::background` / `Label::color` 接受 `impl Into<Brush>`，
主题色一律写 `ThemeColor::*`（`Brush` 在**绘制时**按当前主题解析，主题一换自然跟随）：

```rust
tree.insert(col, Label::new("标题").style(self.theme.typography.title));
tree.insert(
    col,
    Label::new("副标题").color(ThemeColor::OnSurfaceVariant),
);
```

只有真正不随主题变的颜色（品牌色、图片底板）才用字面量 `Color`。为什么：字面量会把
「深 ↔ 浅、换 accent」切主题时的旧值冻进树里，出现「深字压深底」。参 `Brush` 文件头说明
（`kanesumi-core/src/brush.rs`）。

### 2.5 弹层（overlay / xdg_popup）

下拉菜单、选择器浮层、TeachingTip、对话框都是**覆盖层**节点：`tree.open_popup` 把整棵子树挂到
`overlay`（内容之后绘制、之前命中），位置按触发器 `rect` 用 `place_popup` 算；LightDismiss /
模态由 `PopupSpec` 声明。关闭经 `tree.close_popup(id)`，锚点控件会收到 `PopupDismissed` 动作。

```rust
let panel = tree.open_popup(
    Border::new()
        .background(ThemeColor::Surface)
        .stroke(ThemeColor::Divider, 1.0)
        .padding(Insets::all(20.0)),
    PopupSpec { modal: true, ..PopupSpec::default() },
);
// 在 panel 下挂内容；关闭：tree.close_popup(panel)
```

控件层 `MenuFlyout::open(ctx, items, keyboard)` / `open_at` 就是这样把菜单挂上覆盖层的
（`kanesumi-controls/src/menu_flyout.rs`）。

**顶栏 / Dock 上的菜单**（30px 矮表面放不下菜单）：由外壳在该表面之外开一个 `xdg_popup`
（父 = layer 表面，经 `get_popup`）承载，点外部按合成器 `popup_done` 关闭。完整验收见
`kanesumi-gallery/examples/popup_demo.rs`，协议细节参 Ether `docs/POPUP_PLAN.md`
与 `ELEMENT_TREE.md` §Ⅴ.2。

### 2.6 键盘

外壳把 wl_keyboard 经 xkbcommon 语义化后的 keysym 映射为 `Key`（`platform::map_key`）。
应用与控件匹配**语义键**，不再比原始码。全表：

| `Key` | 来源 / 说明 |
|---|---|
| `Char(char)` | 可打印字符，含空格 `' '`、Shift 符号、小键盘。 |
| `Enter` | Return / KP_Enter。 |
| `Backspace` / `Delete` | 退格 / 删除。 |
| `Escape` | Esc（弹层关闭也由框架处理）。 |
| `Tab` | 框架级焦点遍历；控件**不要**消费（`ELEMENT_MIGRATION.md` §3）。 |
| `Space` | 语义化空格（合成事件 / 无 IME 组字）；外壳对可打印空格仍投 `Char(' ')`，TextBox 与所有「Space 激活」控件按 `Char(' ')` 识别。 |
| `Insert` | X11 `0xff63`。 |
| `Home` / `End` | 行首 / 行尾。 |
| `PageUp` / `PageDown` | 翻页（CalendarView 换月即用它）。 |
| `Left` / `Right` / `Up` / `Down` | 方向键。 |
| `F(1..=12)` | 功能键 F1..F12（重命名 F2 / 刷新 F5）；F13+ 落 `Unknown`。 |
| `Unknown(u32)` | 未分类，透传原始 keysym。 |

### 2.7 测试与验收

控件与页面用 `kanesumi_element::testing::TestHarness` 做**无窗口**交互测试（注入指针 / 按键 /
IME 提交，断言树状态与动作）：

```rust
use kanesumi_element::testing::TestHarness;

let mut h = TestHarness::new(400.0, 300.0);
let id = h.tree.insert_with(h.root(), MetroButton::new("确定"), /* LayoutProps */);
h.frame();
h.click(id);                       // 命中 / 按下 / 释放 / 出一帧
assert!(h.take::<ButtonClicked>().iter().any(|(w, _)| *w == id));
h.key(Key::F(5));                  // 语义键直接注入
```

视觉核对用 `kanesumi_harness::snapshot::render_png`（**跨平台**，不开窗，走真实 CPU
光栅器，产 PNG）：

```rust
let (w, h) = kanesumi_harness::snapshot::render_png(
    &mut host, &engine, Size::new(420.0, 300.0), 1.0 /* scale */, 3 /* frames */,
    std::path::Path::new("out.png"),
)?;
```

`tree_minimal.rs` / `tree_demo.rs` / `virtual_list.rs` 都带 `--snapshot` 子命令，照抄即可。

### 2.8 禁止事项

| 禁止 | 改用 |
|---|---|
| 手算 `Rect::new` 几何做命中测试 | 把控件挂进树，命中由框架按布局产物判定（`tree.hit` / 事件路由）。 |
| 渲染期读指针坐标判悬停 | `ctx.state().hovered` / `Event::PointerEnter` / `PointerLeave`。 |
| 写死颜色字面量 | `ThemeColor` 令牌（`Brush`），绘制时按当前主题解析。 |
| 诊断写 `/tmp` 套接字 / tmpfs | 持久路径 `~/.local/state/`。 |
| 每帧重建整棵树 / 重算布局 | `build` 建树一次，动作里 `tree.edit` 改状态；布局只算脏子树。 |
| 控件消费 `Key::Tab` / `Key::Escape` | 留给框架焦点遍历与弹层关闭。 |

---

## 三、核心概念

### 3.1 状态驱动渲染管线

```
state → progress → resolved spatial state → render → Scene
```

- **无保留视觉树、无 timeline**：每帧 `App::render` 从当前状态产出完整 `Scene`。
- 动画由 **Sokuou** 的 `Progress`/`SpringAnim` 驱动，只动**视觉属性**
  （位移/缩放/透明），绝不触发布局（`docs/COMPOSITION.md`）。
- 合成器时钟（frame callback）驱动 `App::update(dt)` → 重渲染。

### 3.2 Scene 命令模型（kanesumi-canvas）

`Scene` 是渲染命令的**无状态描述**，外壳负责光栅化：

| 命令 | 说明 |
|---|---|
| `fill_rect` / `fill_rounded_rect` | 纯色填充（含 `CornerRadius::Square/Slight/Capsule`）|
| `stroke_rect` / `stroke_rounded_rect` | 描边 |
| `text` / `text_with_options` | 文本（对齐 / 换行 / max_lines / 省略）|
| `image` | 位图（`rasterize_svg` → `Icon`，可 tint）|
| `push_clip` / `pop_clip` | 成对裁剪栈（禁止裸 clip，见 COMPOSITION）|
| `fill_arc` | 圆环/扇形（进度指示）|

绘制与命中必须消费**同一布局产物**（`LayoutRect`），禁止另行计算坐标。

### 3.3 TextEngine（排版唯一真源）

- 外壳加载字体后注入 `App::render(&TextEngine, size)`。
- 字体**不得静默回退**（SD §IX）：`font_path()` 指定或系统查找，缺失即错误。
- 文本度量（`metrics`/`advance`）以 `TextEngine` 为准；BiDi + OpenType shaping
  由引擎处理，应用只提供逻辑字符串与显式溢出策略。

### 3.4 主题与 tokens（kanesumi-core）

| 类型 | 职责 |
|---|---|
| `MetroTheme` | 聚合 `MetroColors` + `MetroTypography` + `Tokens` + `MetroIndication` |
| `MetroColors` | 纯色板：`background/surface/surface_variant/divider/primary/on_*` |
| `MetroTypography` | `page_heading/title/body/caption/label` 等样式 |
| `Tokens` | 圆角/间距/字体族 |
| `MetroIndication` | 悬停/按压 tint、禁用透明度、焦点描边 |

控件一律读 `theme.colors.*`，**禁止硬编码颜色**；强调色由系统统一管理（应用不得
自建强调色体系，参 ENCS §X）。

### 3.5 布局：Measure/Arrange（kanesumi-canvas/layout.rs）

- `Constraints` → `LaidTree`：两遍布局引擎（约束传递 → 分配）。`LayoutLeaf` 产出
  `LayoutRect` 供绘制与命中。
- `kanesumi-structure`：`Navigation`（页栈 + 过渡进度）、`ShellLayout`（AppBar /
  内容区划分）、`MetroShell`/`MetroScaffold`。
- 契约：逻辑像素、宽度断点、内容内在尺寸、显式溢出。窗口缩放/折叠时全部重算。

### 3.6 控件契约（kanesumi-controls）

每个控件 = 持有状态的 `struct` + 两个方法：

```rust
pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene);
pub fn hit_test(&self, rect: Rect, pos: Point) -> bool;   // 或 track/thumb 命中
```

- **状态驱动**：应用层改控件状态（`set_checked`/`set_value`/`set_state`），控件把
  状态解析为 Scene 命令；动画内部用 Sokuou `Progress`。
- **无隐藏控件**：reconciler 逻辑组件不产生额外原生控件。
- 行为规格以 `docs/CONTROL_SPEC.md` 为准（Metro/UWP 时代规格，35 控件全量，
  验收见 `CONTROL_MATRIX.md`）。

### 3.7 声明式 DSL + 增量渲染（kanesumi-controls/decl.rs + retained.rs）

> 这是元素树之前的声明式层（`Decl` + `RetainedScene`），仅存量 gallery 页面在用；
> **新应用改用元素树**（§二），它的逐节点命令缓存已覆盖同样的增量渲染目标。

- `view!` 宏：Rust 声明式语法糖 → `Decl` 元素树（编译期检查）。
- `render_decl`：元素树 → 布局展开 → 现有控件渲染 → `Scene` + 命中表 `DeclHit`。
- `diff_decl`：两帧树按路径 diff → `DeclChange`；`RetainedScene` 据此只重建变化
  元素命令段（为合成器「保留视觉树 + damage 重绘」打基础）。

---

## 四、Crate 地图

```
kanesumi-core        （无依赖）tokens/主题/MetroText/指示/几何原语
    ├── kanesumi-anim      dep: sokuou —— 动画预设（弹簧 5 + 时长驱动 6）
    ├── kanesumi-canvas    dep: core —— Scene 命令 + TextEngine + Icon + Measure/Arrange
    ├── kanesumi-structure dep: core+canvas —— Navigation 状态机 + 壳布局 + Grid/TileWall
    ├── kanesumi-controls  dep: core+canvas+anim —— 35 控件 + decl DSL + retained
    ├── kanesumi-harness   dep: 全部 —— App trait + 角色解析 + Linux Wayland+wgpu 外壳
    ├── kanesumi-appmenu   （Linux-gated）全局应用菜单：dbusmenu 服务 + kwin appmenu 绑定
    ├── kanesumi-gallery   —— 控件 Gallery 应用（daily driver / 测试载体）
    └── kanesumi-calculator —— 首个狗粮应用（键盘输入层验证）
```

依赖方向严格单向：`core ← canvas ← controls/harness/gallery`。

---

## 五、harness 外壳（Linux）

### 5.1 角色模型（`ETHER_ROLE` + `EtherRole`）

| 角色 | 表面类型 | 用途 | 例 |
|---|---|---|---|
| `Browser` | xdg-shell（Layer 2）| 普通窗口（默认）| Settings / Librarian |
| `Desktop` | xdg-shell（Layer 1，无 SSD）| 桌面投影 | Librarian 桌面模式 |
| `TopBar` | layer-shell TOP | 常驻顶栏 | Settings TopBar |
| `Dock` | layer-shell BOTTOM | 底部定位线 | Launcher |
| `Launcher` | layer-shell OVERLAY | 全屏浮层 | Launcher |
| `Candidate` | layer-shell OVERLAY（跟随光标）| IME 候选窗 | Ceyboard |

合成器按 `app_id`（`org.ether.*`）应用策略；角色经 `ETHER_ROLE` 环境变量分派
（`EtherRole::from_str`）。一个进程只开一个角色（ENCS 归属边界）。

### 5.2 渲染路径（重要）

**wgpu 离屏 → 读回 → wl_shm 提交**（`platform.rs` 全角色 `shm_output=true`）：

1. `App::render` 产出 `Scene`
2. `Renderer` 以 wgpu 光栅化到**离屏纹理**（`Bgra8UnormSrgb`，非 sRGB 会双伽马）
3. `copy_texture_to_buffer` 读回 BGRA（`bytes_per_row` 需 256 对齐）
4. 写 `wl_shm` 缓冲（Argb8888 = 内存 BGRA）→ `attach + commit`

原因：**Ether 合成器对 layer-shell 的 wgpu dmabuf buffer 渲染不可见**（
`ETHER_RENDER_LESSONS.md` 验证矩阵）。SHM 是唯一可靠路径。合成器提供 `wl_shm` 时
走此路径；缺失时回退直接 present。

### 5.3 输入

- `InputEvent`：`PointerMoved/Pressed/Released/Left`、`Scroll{x,y}`、
  `DoubleClick`（正典 §Ⅱ：500 ms / 4×4 px 容差框，即 ±2 px；常量在 `kanesumi-core::interaction`）、`Key`/`Modifiers`。
- 命中测试：元素树应用由**树**按布局产物判定（§二）；旧 `App` 才由应用自己
  `hit_test`（控件 + 消费布局产物）。外壳只转发事件。
- 文本输入：`key_to_text_input` 把键盘事件映射为 `TextInputKey`（Backspace/Enter/
  方向键等），TextBox/PasswordBox 消费。

### 5.4 IME（输入法）

`App` 实现 `ime_focus()` 返回 `Some(ImeContext)` = 有文本输入焦点 → 外壳启用
`zwp_text_input_v3` 灌周边文本/光标矩形；`PendingImeBatch` 交付 preedit/commit。
引擎宿主（Ceyboard 场景）走 `zwp_input_method_v2` + 候选窗 popup surface。
细节见 `IME_WIRING_PLAN.md` + `CEYBOARD_SPEC.md`。

### 5.5 AppMenu（全局应用菜单）

`App::app_menu()` 返回 `MenuTree` → `install()` 自动完成：
D-Bus 服务挂载 + `com.canonical.AppMenu.Registrar` 注册 + `org_kde_kwin_appmenu`
set_address + 点击路由（`on_menu_command`）。独立 crate `kanesumi-appmenu`
不经 harness 也可被 eframe 应用使用。

### 5.6 浮层（floating layers）

`floating_layers()` 声明独立 layer-shell 表面（固定宽度浮层/右键菜单）：
`floating_visible/render_floating/floating_input/floating_height`。右键菜单状态机
封装在 `ContextMenuState`（`docs/CONTEXT_MENU_SPEC.md`）。

---

## 六、新增一个控件

1. **查规格**：读 `docs/CONTROL_SPEC.md` 对应章节（尺寸/视觉状态表/动画时长/主题色值）；
   对照 `Kanesumi-sec-a` 同名 `.kt` 与 `microsoft-ui-xaml` 的 C++ 实现。
2. **建文件**：`kanesumi-controls/src/<name>.rs`，导出 `struct Metro<Name>` +
   `render(theme, engine, rect, scene)` + `hit_test`。
3. **状态驱动**：控件状态字段 + 动画用 `kanesumi-anim`（消费 sokuou），不新建动画原语。
4. **颜色/字体**：一律 `theme.colors.*` / `theme.typography.*`，禁止硬编码。
5. **测试**：纯逻辑状态切换单测 + `Metro<Name>` 渲染不越界测试（参照
   `kanesumi-gallery/src/app.rs` 的交互测试模式）。
6. **验收**：在 `CONTROL_MATRIX.md` 登记功能对照。
7. **Gallery 演示**：在 `kanesumi-gallery` 加一页，作为 daily driver 验证。

---

## 七、测试

| 层级 | 载体 | 覆盖 |
|---|---|---|
| 单元测试 | 各 crate `#[cfg(test)]` | 布局不越界 / 控件状态机 / 纯逻辑 |
| App 接口测试 | `App::handle_input/render` 直接驱动 | 交互状态切换（gallery 9 个交互测试）|
| 集成 | Gallery + 真实合成器 | 渲染 / 输入 / 浮层 / IME |

三层测试阶梯（参 Ether monorepo `PLAN.md` §4.4）：

| 环境 | 用途 |
|---|---|
| Plasma（或任意合成器）| 日常开发：控件/动画/布局/主题/tokens |
| Ether 嵌套 winit 模式 | `org.ether.*` 策略、SSD、层序、排他区 |
| TTY/DRM 会话 | 真实性能、damage 收益 |

> ⚠ 不烘焙 KWin 特定假设（视 vsync/frame 为通用 Wayland），否则进 Ether 必露馅。

---

## 八、调试与常见坑

### 日志
- 各二进制 `env_logger`：`RUST_LOG=debug cargo run`
- Ether 会话日志：`~/.cache/ether-session/session.log`

### 渲染不可见排查（Ether 下）
1. 进程存活？`pgrep -af <app>`
2. 崩溃原因？`grep -iE "panic|wgpu|Adapter" session.log`
3. layer surface 被合成器收集？`ETHER_LAYER_DEBUG=1` 重启合成器 →
   `~/.cache/ether-session/../ether-layer-draws.log`（每行 `elements=N`，N=1 有 buffer）
4. 结论惯例：**先看 buffer 类型**（SHM ✓ / dmabuf ✗），再谈配置。详见
   `ETHER_RENDER_LESSONS.md`。

### 常见坑
| 坑 | 解法 |
|---|---|
| wgpu 离屏纹理 format 不匹配 → panic | 必须 `Bgra8UnormSrgb`（与 pipeline 一致）|
| `copy_texture_to_buffer` Validation Error | `bytes_per_row` 256 对齐，读回跳过 padding |
| wgpu 22 错误默认 fatal → 进程崩 | 看 session.log 的 panic 定位 |
| 层表面被合成器强制拉成全宽 | 固定宽度浮层处理见 platform.rs 注释 |
| 文本不渲染 / 字体异常 | 走 `TextEngine` 度量，禁裸 `fontdue` 调用 |

---

## 九、设计铁律速查

1. **Kanesumi Design，而非 Fluent**——轻盈短促 0.25s、Quadratic/EaseOut；Win11
   Fluent 仅作反面教材。
2. **Sokuou 动画唯一真源**——禁止自造动画原语。
3. **状态驱动渲染**——`state → progress → resolved → render`。
4. **动画只动视觉属性，不动布局**——绝不触发 Measure/Arrange。
5. **无隐藏控件**——reconciler 逻辑组件不产生额外原生控件。
6. **纯色无渐变**——直角或极轻微圆角；`Capsule` 仅限全圆角形态（Switch 轨道等）。
7. **字体不得静默回退**、**单一渲染权威**、**进度驱动不时间轴**。

---

## 附：旧 App 接口（仅维护存量）

> 旧手写几何的 `App` trait：应用自己算坐标、自己做命中、自己 `render` 出 `Scene`。
> **新代码不要用**；存量应用迁移到 §二 的元素树。保留的原因：`TreeHost` 正是把 `TreeApp`
> 接成这个 `App` 接口，故外壳主循环零改动、新旧应用可并存。

```rust
// 最小旧式 Kanesumi 应用（仅存量维护参考）
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect};
use kanesumi_harness::app::{App, AppConfig, InputEvent};
use kanesumi_harness::role::EtherRole;

const CONFIG: AppConfig = AppConfig::new(
    "org.ether.hello",      // app_id：合成器按此应用策略（桌面/Layer2/层位）
    "Hello Kanesumi",
    EtherRole::Browser,     // 角色决定表面类型与层策略
    480.0, 320.0,
);

struct HelloApp { theme: MetroTheme }

impl App for HelloApp {
    fn config(&self) -> &AppConfig { &CONFIG }
    fn theme(&self) -> MetroTheme { self.theme }

    fn handle_input(&mut self, _event: InputEvent) {
        // 指针/键盘事件（Moved/Pressed/Released/Scroll/DoubleClick/Key…）
    }

    fn render(&mut self, engine: &TextEngine, size: kanesumi_core::Size) -> Scene {
        let mut scene = Scene::default();
        scene.fill_rect(self.theme.colors.background, Rect::new(0.0, 0.0, size.width, size.height));
        scene.text("你好，Kanesumi".to_string(),
            Rect::new(40.0, 40.0, 400.0, 30.0),
            self.theme.colors.on_surface, self.theme.typography.title,
            TextAlign::Left);
        scene
    }
}

pub fn run() -> ! {
    let app = Box::leak(Box::new(HelloApp { theme: MetroTheme::ether_dark() }));
    kanesumi_harness::platform::run(app)
}
```

新旧对照：

| 关注点 | 旧 `App` 接口 | 元素树（§二）|
|---|---|---|
| 布局 | 应用手算矩形 | `LayoutProps` + `Stack` |
| 命中 | 应用 `hit_test` | 框架按布局产物 |
| 焦点 | `FocusRing` / 手动登记 | 树序派生 |
| 输入 | `handle_input` 自行分派 | `on_action` 动作回调 |
| 损伤 | 应用 `damage_hint` | 树自动产出 |
| 渲染 | `render` 每帧产 `Scene` | 树逐节点缓存 + 拼接 |

控件仍可以 `render(theme, engine, rect, scene)` / `hit_test` 直驱（参 §3.6）；
`InputEvent` / `Key` / `Modifiers` 由 `kanesumi_harness::app` 重导出。

---

## 附：术语（Kanesumi Design 定名）

- **`Metro*` 标识符**（MetroText/MetroTheme/MetroIndication/MetroShell…）是代码/组件名，
  **保留不重命名**。
- 写文档/注释用「Kanesumi 风格 / Kanesumi 铁律 / Kanesumi 控件库」，不写「Metro 风格」。
- 缓动族称「UWP 缓动」（`UwpEasing`）；「Metro 时代」仅作历史指称。
- 注释中文 Vintage Words 风格，`参 <doc> §<chapter>` 交叉引用。
