# Kanesumi 框架路线图（2026-09-22 立）

> **与 `PORT_ROADMAP.md` 的分工**：那份管**控件层**（WinUI 控件移植，已清空开源项）；
> 本文件管**框架层**（保险机制、协调、优化）。两者不重叠。
>
> **配套文档**：`MATURITY_AUDIT_2026-09-22.md`（缺口证据与优先级）、
> `REFERENCE.md`（微软栈源清单与对照表）、`CANON_VS_TEMPORARY.md`（临时值登记）、
> `COMPOSITION.md`（构图强制契约）。
>
> **本文件是「接下来做什么」的唯一真源。** 每完成一批，先改本表状态，再动其他文档。

---

## §Ⅰ 定位：做什么、不做什么

判断依据（维护者 2026-09-22）：UWP 真正强在**保险机制**（开发者不必预防"按钮飞出窗口"
"文本大于文本框"）与**极其好的优化**；一体动画与设计语言 Kanesumi + Sokuou **已有**。

**做**：保险机制、元件协调、优化纵深。

**不做**（明确写下来，防止反复）：
1. **不重建动画架构** —— Sokuou 是唯一真源，本路线只补词汇表覆盖与"接线"。
2. **不照搬依赖属性 / 附加属性** —— Rust 侧代价大于收益，只借其**失效传播**语义。
3. **不引入 Fluent 外观** —— 大圆角、Acrylic、长动效一律不取（参 `REFERENCE.md` §Ⅰ）。
4. **不为"原创"另起炉灶** —— 有权威实现可读就先读（WPF/WinUI 3 均已开源可读）。

---

## §Ⅱ 三条攻坚线

| 线 | 缺口本质 | 对应阶段 |
|---|---|---|
| **A 保险机制** | 结构上仍可能写出"飞出窗口 / 字大于框 / 点不到" | M1、M2 |
| **B 协调** | 无焦点管理器、无指针捕获、无右键目标语义 | M3 |
| **C 优化** | 无保留树、无布局失效、xdg 角色不吃损伤、无容器回收 | M4、M5 |

M6 是 A/B/C 的收尾（动画词汇补齐 + 优化可见化）。M7 是独立长线（无障碍）。

---

## §Ⅲ 阶段与批次

状态记号：⬜ 未开始 ｜ 🔵 进行中 ｜ ✅ 已完成 ｜ ⏸ 待裁定阻塞

### M1 · 令牌纪律（保险机制线）—— **已完成（2026-09-22）**

**目标**：控件不可能再写出错颜色；未知令牌必须报错（对标 WinUI 的
`XamlResourceReferenceFailed`：写错必须报错，不能静默回退成另一个样子）。

| 批次 | 内容 | 状态 |
|---|---|---|
| M1-1 | **令牌分类学 + 来源对齐**：把魔法 alpha 按语义归族，每族对齐权威来源 | ✅ 2026-09-22 |
| M1-2 | **迁移**：内联字面量 6 处 / 5 文件；魔法 alpha 约 32 处 / 20 文件 → 令牌 | ✅ 2026-09-22（M1-2a 20 处 + M1-2b 38 处） |
| M1-3 | **静态检查**：测试扫描 `kanesumi-controls/src/*.rs` 生产段，禁止 `Color::from_hex/rgb/new/from_rgba` 与裸 `with_alpha(<数字>)` | ✅ 2026-09-22（`tests/token_discipline.rs`，含反向自检） |
| M1-4 | **对比度自检扩展**：新增令牌纳入 WCAG 断言（正文 ≥4.5 / 次级 ≥3.0） | ✅ 2026-09-22（tint 合成后判、实心件 ≥3.0、指示条 vs 轨道） |

#### M1-1 分类学（2026-09-22 定；当日经一手源复核后修订）

**一句话：中性底色不是"一个 hover 值"，而是按角色分族。** 初版依据是
「AppBarButton 白 10%（§65）vs ListView 行 ≈白 30%（§215）」这两条看似矛盾的规格。
**当日晚些时候在 Windows 上取到一手主题字典后，该前提被推翻**：UWP 里两者用的是
**同一个笔刷** `HighlightListLow`（= `SystemListLowColor` = 10%），30% 出自一个
**从未被任何样式引用**的 Win8 遗留键。故「按控件分族」在**列表行**这一项上不再成立 ——
分族仍然必要（不同的**强度档**确实存在），但判据从「按控件」改为「按强度档 + 是否自带底色」。

| 族 | 令牌 | 暗色 | 亮色 | 一手源（`themeresources.xaml` 行号 / WinUI 字典） |
|---|---|---|---|---|
| 中性交互（悬停） | `MetroIndication::hover_tint` | 白 **9.8%** | 黑 **9.8%** | `SystemListLowColor` `#19FFFFFF`/`#19000000`（L228/L4144），用于 AppBarButton / ListViewItem / ComboBoxItem / TreeViewItem |
| 中性交互（按下） | `MetroIndication::press_tint` | 白 **20%** | 黑 **20%** | `SystemListMediumColor` `#33…`（L229/L4145） |
| 中性交互（弱按压） | `MetroIndication::press_subtle_tint` | 白 9.8% | 黑 9.8% | = ListLow；「弱按压」是 Kanesumi 角色选择 |
| 极轻容器底 | `MetroIndication::subtle_tint` | 白 **5.9%** | 黑 **3.5%** | WinUI 2.x `SubtleFillColorSecondary`（TabView 关闭键悬停等，`TabView_themeresources.xaml` L52） |
| 行 / 项选中 | `MetroColors::selection_tint` | accent **0.6** | accent **0.4** | `SystemControlHighlightListAccentLowBrush`（L304/L4220）—— ListViewItem 与 ComboBoxItem 共用 |
| 文本选区 | `MetroColors::text_selection_tint` | accent 35% | 同 | ⚠ 一手源是 accent **100%**（L864→L282）；35% 属有意偏离候选（T19） |
| 语义 | `MetroColors::focus_stroke` / `StatusColors` | — | — | 见 `REFERENCE.md` §9.2 |

> **已删除的令牌**（初版有、复核后没了）：
> `list_hover_tint`（与 `hover_tint` 同值同义）、`accent_low_tint`（24% 无出处；其两个用途分别
> 归入 `selection_tint` 与「无边框的 AccentLow 衬底」）、`subtle_hover_tint`(15%) 与
> `subtle_press_tint`(25%)（笔刷名对、百分比错：真值 5.9% / 3.9%，且按下的 Tertiary 比悬停的
> Secondary **更淡**）。取证路径见 `docs/UWP_PRIMARY_SOURCES.md`。

**不透明度乘数不算色调**（`0.8/0.7/0.6/0.5/0.4/0.38` 等）：它们表达"次要 / 禁用 / 水印"的
**透明度**，不是叠加色。处置：具名为 `MetroIndication` 的前景强度档
（`base_medium_high` **0.8** / `secondary_opacity` 0.8 / `placeholder_focused_opacity` 0.7 /
`base_medium` 0.6 / `inactive_opacity` 0.5 / `base_medium_low` **0.4** / `disabled_opacity` 0.38），
**不得**与 tint 混为一谈；档位与方案无关（叠加色已翻转），已有回归测试固定。
前两档的取值也按一手源更正过：Base* 族是**等距 20 点**（`FF`/`CC`/`99`/`66`/`33`）。

**验收**：M1-3 的静态检查通过 + 全量测试绿 + `CANON_VS_TEMPORARY` 临时项减少。

- 静态检查：`cargo test -p kanesumi-controls --test token_discipline` 绿（附反向自检：
  在样例上验证检查器抓得到构造器与裸 alpha，并断言扫描文件数 ≥40，防「空集合静默通过」）。
- 全量测试：本机（Windows）712 个单元测试 + 2 个静态检查 + 2 个 doctest 全绿（合计 716）；
  `cargo clippy --workspace --all-targets` 告警数与基线逐条一致（无新增）。
- 临时项：M1 收官时共登记 T1~T16（把原先**连名字都没有**的无源取值第一次登记下来）；
  同日一手源复核后 **T4/T5/T11/T12/T13/T14 结案**、新增 T17~T19，见 `CANON_VS_TEMPORARY.md` §三。

**已记账（M0，2026-09-22 完成）**：溢出契约（默认省略号 + `label/paragraph` + 框矮于一行不静默消失）、
容器裁剪 6 处、可达 panic 5 处、`shm_open`/IME 边界/`guard()` 鲁棒性、`Accent` 色阶、深浅双态令牌、
`StatusColors` 语义色、`FocusRing` + Tab 路由。

### M2 · 布局接管（保险机制线，最大的一块）

**目标**：一个屏幕的坐标只在**一处**定义；控件矩形由引擎产出；`child ⊆ parent` 可断言。

| 批次 | 内容 | 依赖 |
|---|---|---|
| M2-1 | `LayoutLeaf` 契约补 `clips_children()`（默认 true）+ `arrange` 强制 `Constraints::max` | — |
| M2-2 | 三套布局去留裁定后收敛（`canvas/layout`（两遍约束）/ `structure/ui`（单遍 cursor）/ `structure/grid`（零消费者）） | **⏸ 待裁定** |
| M2-3 | 控件补 `measure()` / `hit_test()`：现 13/47 有 measure、11/47 有 hit_test | M2-1 |
| M2-4 | `kanesumi-gallery` 三页全量迁移为**参照实现**（真实 rect 由引擎产出） | M2-2 |
| M2-5 | 通用回归：「控件矩形外 1px 不可命中」「所有绘制矩形 ⊆ 祖先裁剪」 | M2-1 |

**验收**：gallery 迁移后视觉回归通过；M2-5 两条通用测试覆盖全部已迁移控件。
**风险**：高。**不碰 Ether 侧应用**（那属另一轮，需单独评估）。

> **M2-1 实施前必读（2026-09-22 静态勘察，未动代码）**：`clips_children()` 默认 `true` 这条不能照字面实现。
> 引擎现在只对**容器**节点 `push_clip`（`layout.rs::render_rec`），叶子自绘不裁剪；改成「叶子也默认裁到自身矩形」
> 会切掉三类**故意画出矩形之外**的内容：
> 1. 溢出式装饰 —— `MetroPersonPicture` 徽标按规格 Margin `0,-4,-4,0` 外溢 4px（`CONTROL_SPEC` §16），
>    裁了就成缺角圆；
> 2. 浮层 —— `MetroDropdownMenu` / `MetroSelectorFlyout` / `TeachingTip` / `CommandBarFlyout` 的面板画在触发器之外
>    （gallery 因此把整屏矩形传给 `render`，见 `app.rs` 的 `Rect::new(0.0, 0.0, size.width, size.height)`）；
>    矮表面（TopBar 30px / Dock）上裁到矩形即整块面板消失，正是审计 P1-6「弹层画在主 surface」的加重版；
> 3. 焦点环 / 描边 —— 画在控件矩形之外一圈。
> 故这一批**必须先做一次真机视觉确认**（或先给这三类控件显式 `clips_children() = false` 的名单并逐条核对）。
> 「arrange 强制 `Constraints::max`」相对安全（只会收窄溢出者），但仍需 M2-5 的 `child ⊆ parent` 断言兜住。

### M3 · 交互协调（协调线）

**目标**：纯键盘可操作；指针捕获与右键目标由框架记录，App 不再自己维护。

| 批次 | 内容 |
|---|---|
| M3-1 | `FocusRing` 接入各可聚焦控件（`is_tab_stop`）；方向键遍历 |
| M3-2 | 指针捕获（拖拽 / Slider 拖动 / 文本选择）—— 旧实现在 App 侧自持 |
| M3-3 | `PointerEntered` / `Left` 与**真实 Modifiers**（现恒 default，KeyReleased 缺失） |
| M3-4 | `ContextTarget` 一等语义：`PointerPressed` 时框架记录命中项，菜单只能读它（对应 arc-deck R15「右键作用于错误行」） |
| M3-5 | 输入模型补全：`KeyReleased`、`TextInput` 与 IME `Commit` 分立 |

**验收**：`kanesumi-gallery` 全控件可纯键盘走完；M3-4 有回归测试。

### M4 · 优化：保留树 + 失效传播（优化线，**收益最大**）

**目标**：静止零重绘；变更只重算受影响子树；「动画只动视觉属性」在结构上有保证。

| 批次 | 内容 |
|---|---|
| M4-1 | `RetainedScene` 真增量：去掉每帧两次全量 `render_decl`，命中表复用同一 `LaidTree` |
| M4-2 | `diff_decl` keyed reconciliation（现按位置路径匹配 → 列表增删导致后续全 `Changed`） |
| M4-3 | 布局失效标记（`invalidate_measure` / `invalidate_arrange`），命中即复用 |
| M4-4 | damage 由 diff 产出，接 `App::damage_hint`（不再由 App 手工算） |
| M4-5 | xdg-shell 角色吃损伤重绘（现只有 CPU 路径有） |

**验收**：A/B 基准（帧耗时 / 命令数 / `layout_misses`）有数字对比；
「增量结果与全量路径逐条等价」测试。
**风险**：中高。**前置**：M4-3 之前不得做 M5 的回收优化（流沙上优化）。

### M5 · 优化：滚动与虚拟化统一

**目标**：任何内容放不下就有滚动的**统一**容器，而不是每种控件各写一套。

| 批次 | 内容 |
|---|---|
| M5-1 | 通用 `ScrollHost`（clip + 虚拟化 + 命中三合一）；`MetroScrollView` 补 `render` |
| M5-2 | `SelectorFlyout` 504px 截断不可达 → 接滚动 |
| M5-3 | `DropdownMenu` 面板高度夹紧（现可画到屏外）；`place_popup` 契约变更需维护者确认 |
| M5-4 | `TreeView` 接滚动；`MetroRepeater` 容器回收（依赖 M4-2 的 keyed） |

### M6 · 动画词汇补齐 + 优化可见化

> **2026-09-22 一手源复核（见 `docs/UWP_PRIMARY_SOURCES.md` §Ⅲ.5）对本段的三点影响**：
> 1. **M6-1 的「100ms Y 下沉」无一手依据** —— UWP 桌面语义是**缩小+倾斜**（写 `Projection`/`RenderTransform`），
>    时长与幅度 OS 预置、XAML 读不到；本文旧值属推断（登记 `CANON_VS_TEMPORARY` T17）。
>    故 M6-1 落地时必须先裁定「照 UWP 的缩小+倾斜」还是「保留 Kanesumi 的下沉」。
> 2. **M6-3 的归属要改**：`ControlFastAnimationDuration`（0.167s）属 **WinUI**，不属 UWP ——
>    UWP SDK 两版 XAML 与 `Windows.UI.Xaml.dll` 全 0 命中。数值正确、注释已更正（`presets.rs`）。
> 3. **M6-2 的 stagger 仍无一手支撑**：`EntranceThemeTransition` 在 UWP 里只有无参标签；
>    可借的同类权威是 CalendarView 页面过渡（离场 0.233s / 入场 0.733s / 延迟 0.233s，
>    曲线 `0.1,0.9,0.2,1`）。另：Expander / ScrollBar / 对话框淡入淡出的时长与一手值一致 ✅。

| 批次 | 内容 | 来源 |
|---|---|---|
| M6-1 | `PointerDown/UpThemeAnimation`（**先裁定语义**：UWP 缩小+倾斜 vs Kanesumi Y 下沉），现 17 个交互控件硬切换 | `UWP_PRIMARY_SOURCES.md` §Ⅲ.5 |
| M6-2 | stagger 入场（`EntranceThemeTransition` 语义） | `ANIMATION_SPEC.md` 处置表 + CalendarView 旁证 |
| M6-3 | `ControlFastAnimationDuration = 0.167s` 对齐（**winUI 值**，非 UWP；`ControlSlowAnimationDuration` 一手源里不存在） | `UWP_PRIMARY_SOURCES.md` §Ⅲ.5 |
| M6-4 | 帧统计浮层 + 过度绘制可视化（对标 `EnableFrameRateCounter` / `IsOverdrawHeatMapEnabled`） | `REFERENCE.md` §Ⅲ.1 |
| M6-5 | **（新）ProgressBar / ProgressRing 不确定模式二选一**：ProgressBar 现有 2.0s 与 WinUI 2.8.6 一致、与 UWP OS 3.917s 冲突；ProgressRing 现有 2.0s/900° 与 UWP OS 3.47s/−110°→585°/6 点 stagger 冲突（登记 T18） | `UWP_PRIMARY_SOURCES.md` §Ⅲ.5 |

### M7 · 长线（P2）

无障碍语义树（对照 UIA，导出 AT-SPI）· 系统剪贴板 · 拖放 · `FontWeight` 真字面选择 ·
文本缓存失效 API（字体 identity / text-scale）。

---

## §Ⅳ 执行纪律（每批都必须满足）

1. **一个功能块一次 commit**，Conventional Commits 前缀、中文正文、句号结尾（参 `AGENTS.md`）。
2. **未经明确指示不得 push**。
3. 提交前 `cargo test --workspace` 全绿、`cargo clippy --workspace --all-targets` 无新增告警。
4. 行为变更（尤其默认值）**必须带回归测试**，并在 `CANON_VS_TEMPORARY.md` 或本文件留痕。
5. 数值来源必须可追（先读源再写规格再写码）；无源无实测时**登记为临时**，不伪装成正典。

---

## §Ⅴ 决策记录（原「待裁定」）

> 2026-09-22：以下各项经授权由代理裁定并落地，理由记录备查。**当前无阻塞项。**

| # | 议题 | 裁定 | 理由 |
|---|---|---|---|
| 1 | 三套布局的去留 | **`canvas/layout` 为唯一布局引擎**；`structure/ui.rs` 标为 LEGACY，随 M2-4（gallery 迁移）删除；`structure/grid.rs` 保留并计划接入 `LayoutNode` 容器 | `grid` 是 `CONTROL_SPEC` §33 的规格控件（二维 Fixed/Auto/Star + span），不是竞品引擎；`ui.rs` 与 canvas 语义重叠且仅 gallery 使用，属历史形态 |
| 2 | `place_popup` 契约是否可破 | **不破签名**，夹紧在函数内部完成 | 「签名冻结」约束的是调用形状，不是允许把面板画到屏幕外。已补「面板高于/宽于屏幕」两条回归测试 |
| 3 | T3：WinUI 分层 ↔ 本项目分层的映射 | **不做映射** | WinUI 的 Base/Secondary/Tertiary 表达「基底 / 替代色阶」而非层级高度（暗色里 Secondary 比 Base **更暗**），与本项目 `background < surface < surface_variant` 的单调递增不同构。强行映射等于引入另一套心智模型；缺的令牌另行按名取 WinUI 值 |
| 4 | T8：两个 `press_tint` 收敛 | 按角色收敛：`MetroIndication::press_tint`（22%，控件按压）与 `press_subtle_tint`（10%，大面积 / 次要按压）；**删除** `MetroColors::press_tint` | 与 hover 族同构 —— 同名不同义才是漂移源，分角色命名后两个值都成立 |
| 5 | `47e480f`（已推送的大杂烩提交） | **不拆分、不改写历史** | 规则禁止 force-push；且该提交触及的文件已被后续主题重构覆盖，拆分的回滚收益低于改写已发布历史的代价。其价值转为反例，已写入 `AGENTS.md` |
| 6 | `SubtleFillColorSecondary` 究竟是白 5.9% 还是白 15% | **~~两代取值并存、按角色分族~~ → 一手源推翻：不是「两代并存」，而是「笔刷名对、百分比错」**。真值 Secondary **5.9%/3.5%**、Tertiary **3.9%/2.4%**（WinUI 2.x 字典）；两个浅叠令牌已删除 | 当日取到 WinUI 2.x `Common_themeresources_any.xaml` 后作废：`0FFFFFFF` = 5.9%，`CONTROL_SPEC` 的 15%/25% 无出处（疑把 alpha 字节当百分比翻倍）。**同一条规格里的 5.9% 与 15% 不可能都对** —— 这正是「不能停在推断」的例子 |
| 7 | `ControlFastAnimationDuration` 是否立即对齐 0.167s | **不动，留给 M6-3** | 权威值已取到，但批量替换会同时改变 17 个交互控件的观感；须与 M6-1 同批落地，避免两次视觉变更叠加、无从归因。**且归属须改**：该族是 WinUI 而非 UWP（见 §Ⅴ-11） |
| 8 | M1-2 迁移中遇到的「无权威来源取值」怎么办 | **令牌化但登记为临时**（不猜、不丢） | 那些值（0.24 / 0.8 / 0.5 / 0.7 / 60% 轨道 / 两个错误红）原先连名字都没有，谈不上「登记」；现在先给名字与角色，再在 `CANON_VS_TEMPORARY.md` 登记（T12~T16）。**不借迁移之机顺手改视觉** —— 无实测就改值，等于把「临时」换成「另一个临时」 |
| 9 | **一手源取证轮**：本机 Windows 上取到权威字典后，是否立即按权威值改视觉？ | **改，但每处都要有 `文件:行号`，并在提交信息里写明「旧值从哪来、为什么错」** | 十处旧值被证明**无出处**（22% 按下、30% 行悬停、24% 强调低透、90% BaseMediumHigh、35% BaseMediumLow、输入框边框 ×90%、弹窗收起 0.26s、两方案同值的暂停不透明度…）。这些不是「调优分歧」，而是「抄错了」——留着等于把错误固化成规格。取证路径与新值见 `UWP_PRIMARY_SOURCES.md`，逐条对照见其 §Ⅱ |
| 10 | 一手源与 `CONTROL_SPEC` 冲突时，改哪边？ | **两边都改**：`CONTROL_SPEC` 就地更正并标注 `文件:行号`，代码同步改；冲突结论进 `CANON_VS_TEMPORARY` | 规格是实现的依据，若规格留着错值，下一轮实现一定会抄回去 —— 「临时方案被当成设计」的同款风险，只是对象换成「错误数值」 |
| 11 | `Control*AnimationDuration` 的归属 | **改注释为「WinUI」，并把 M6-3 的表述同步改掉** | 四份 XAML（UWP 两版 SDK + WinUI 2.8.6）与 `Windows.UI.Xaml.dll` 对这四个键名全 0 命中，只在 WinUI 的 `resources.pri` 里；数值 0.167 正确但**不可在 UWP 风格 XAML 里引用**。原注释「对齐 UWP」会误导下一个人去 UWP 里找不存在的键 |
| 12 | ProgressBar / ProgressRing 的「UWP OS 值」与「WinUI 2 值」冲突时取谁 | **先按控件代际归属，再一次性裁定；未裁定前保持现状并登记** | ProgressBar 现用 2.0s 与 WinUI 2.8.6 完全吻合（`0.4,0,0.6,1`），而 UWP OS 是 3.917s 五圆点；ProgressRing 现用 2.0s/900° 两者都不符（源自已丢弃快照）。**同一控件不能两个来源各取一半** —— 故列入 M6-5，不在此轮擅改 |

## §Ⅵ 跨仓联动（Ether 侧，不属本路线但会互相等待）

记录在此，避免"Kanesumi 做完了但没人接"：

1. **Chorus 主题消费端接线**：harness 侧已就绪（`App::set_theme` + `system_theme`），
   需要 `settings` / `librarian` 实现 `set_theme` 并返回它，才能"改 accent 全应用变色"。
2. **设置模块反转**（`DEPENDENCY_MODEL.md` 规则 2）：Settings 不再硬编码产品类目。
3. **媒体键与快捷键服务**（`SETTINGS_DESIGN.md` 附 · B）：`org.ether.Shortcuts` + Settings 界面。
4. **Settings 类目内容**：先取 Windows / macOS / GNOME / KDE 并集，再按本机裁，最后 polish。
