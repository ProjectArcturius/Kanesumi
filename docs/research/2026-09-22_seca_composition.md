# sec-a → 主仓 构图/壳层/响应式布局 回流可行性评估

> 只读评估。未改动任何源码，未执行 cargo / gradle。
> 两侧基线：
> - **sec-a**（Android/Kotlin + Compose）：`C:\Users\mc158\Documents\Projects\Kanesumi-sec-a`，`master` @ `acddf3e`，工作区干净。
> - **主仓**（Rust）：`C:\Users\mc158\Documents\Projects\Ether\shared\kanesumi`。
> 术语遵 `KANESUMI_DESIGN.md`：`Metro*` 是代码标识符（保留），缓动族称「UWP 缓动」。

---

## §1 结论先行

**总判：构图层可回流性「中偏低」，但有三件高价值资产必须回流，其中两件是主仓当前的真实缺口。**

1. **断点表是真资产，且主仓已经有了 —— 只是没落在 `kanesumi-structure`。**
   任务预设的「sec-a 有断点表、Rust 没有」**不成立**：sec-a 全仓 **没有任何宽度断点**（`grep` 仅命中 `MetroResponsiveContent.kt:23 maxWidthDp = 360.dp` 这一条内容最大宽度，不是导航形态判据）。
   真正的断点表在 **主仓的 Ether 侧**：`settings/src/kanesumi_window.rs:33 const EXPANDED_BREAKPOINT: f32 = 720.0`，配合 `:435` 的分派与 `kanesumi-controls/src/navigation_view.rs:18/20` 的 `NAV_PANE_EXPANDED = 320.0` / `NAV_PANE_COMPACT = 48.0`。
   → **回流方向是「主仓内部上提」（`settings` → `kanesumi-structure`），不是「sec-a → 主仓」。** 详见 §3-(a)-2。

2. **sec-a 的 `MetroSidebar` 是死代码，且 sec-a 根本没有形态切换器。**
   `MetroSidebar` 全仓唯一引用是它自己的 KDoc（`MetroSidebar.kt:47`）；`sample/MainActivity.kt:133-175` 只用 `MetroShell` + `MetroBottomNav`，**从未使用侧栏**，也没有任何 `BoxWithConstraints` / `WindowSizeClass` / 折叠态 / 车机态的分支。
   提交 `1296e3f` 的信息是「对应窄屏 `MetroBottomNav` 的宽屏形态」，即**并列 API、由调用方自选**，无判据。
   → 任务书 §6 假设的「sec-a 的折叠/平板/车机形态切换」**在源码中不存在**；这一条应裁定为「无经验可搬」。

3. **`MetroTopScrim` 与主仓正典存在真实冲突，且 sec-a 自己也违规。**
   `MetroTopScrim.kt:53-56` 用 `Brush.verticalGradient` 做渐隐遮罩。而 `KANESUMI_DESIGN.md:102` 铁律 6 明文「**无 ripple、无渐变、无阴影**」，`DEV_GUIDE.md:309` 重复一次，`scene.rs:21` 的注释也写「参 SD §II —— 纯色、无渐变」。
   注：`compositor/docs/SD.md`（413 行）**全文 0 次出现「渐变」二字** —— 「SD §II 禁渐变」是被反复引用的**派生结论**，源头是 `KANESUMI_DESIGN.md` §Ⅲ.3/§Ⅳ-6，不是 SD.md 原文。这是个文档溯源问题，值得顺手纠正（见 §5-1）。
   → 回流的正确形态**不是**搬渐变，而是**搬「阶梯色带」方案**并把它登记为 Kanesumi 取值（`CONTROL_SPEC.md:1017` 已有先例：Spectrum 2D 渐变即用阶梯色带近似）。

4. **sec-a 的 `MetroInsets` 是「被文档大幅高估」的抽象。**
   `README.md:47` / `CLAUDE.md:88` / `AGENTS.md:82` 三处都把它说成层 0 的「护城河」。实际：`rememberMetroInsets()` 全仓**唯一消费者是 `sample/MainActivity.kt:187,205` 的调试面板**（`DebugPanel`，`MainActivity.kt:677-705` 只把值打印出来）。所有真实控件的贴边都绕过了它、直接调 `InsetModifiers.kt` 的四个薄封装；`MetroInsets.kt:41` 的 `ime` 字段**零消费者**。
   → 回流价值应在 §3-(b) 低调处理，**不要按文档承诺的规格去设计 Rust API**。

5. **真正值得回流的高信号设计只有两条半**：
   (a)「底栏 = 悬浮 overlay、内容铺满全屏 + 自适应留白」的**反 Scaffold 契约**（`MetroShell.kt:20-23`，`MetroBottomStack.kt:50-60`）—— 主仓 `ShellLayout` 现在还在做 Scaffold 式的空间切分（`layout.rs:26-39`），两者是对立的；
   (b)「顶栏 = 滚动列表第一项，shell 无 topBar 槽」（`MetroShell.kt:29-30`）—— 与 sec-a `MetroAppBar.kt` 的实现自洽，与主仓 `MetroShell::render` 硬切 48px（`lib.rs:46-52`）冲突；
   (c) 断点 + nav-rail 几何（但来源在主仓内部，见结论 1）。

**回流总账（粗估）**：(a) 类可直接搬 4 项 ≈ 1 人日；(b) 类需改造 6 项 ≈ 4~6 人日；(c) 类不可搬 5 项。**其中真正影响 M2 的只有 (b)-1/(b)-2 两条，是「契约级」而非「代码级」回流。**

---

## §2 逐项对照表

### 2.1 壳层契约

| 维度 | sec-a（Kotlin） | 主仓（Rust） | 差异裁定 |
|---|---|---|---|
| 壳的容器 | `MetroShell.kt:39-52`：`MetroBottomStackScope { Box(fillMaxSize + background) { content(); Box(align=BottomCenter){ bottomBar() } } }` | `lib.rs:24-28`：`MetroShell<PageId> { theme, nav, app_bar }`；`lib.rs:46-52` `render()` 填背景 + 画 app_bar，返回 content 矩形 | **对立**。sec-a = 叠加式（overlay）；Rust = 切分式（split） |
| topBar 槽 | **没有**（`MetroShell.kt:29-30` 明写「不管 topBar」）；顶栏是 LazyColumn 的 item（`MainActivity.kt:192-204`） | **有**：`lib.rs:41-43 layout()` → `ShellLayout::of(window)`，`layout.rs:26-31` 硬切 `APP_BAR_HEIGHT=48` 给 app_bar | **对立**。Rust 是 Scaffold 语义，正是 sec-a 刻意反叛的形态 |
| bottomBar 槽 | 单 slot `bottomBar: @Composable (() -> Unit)?`（`MetroShell.kt:35`），叠层自己在 `Column` 里排（`MainActivity.kt:135-149`） | **无**。`ShellLayout` 只有 `app_bar` / `content` / `nav_rail`（`layout.rs:13-20`） | sec-a 的 overlay 概念 Rust 侧不存在 |
| 侧栏 | `ShellLayout` 无对应物；sec-a 有独立控件 `MetroSidebar`（不参与壳布局，`MetroSidebar.kt:60-73` 自持 `width`） | **有**：`layout.rs:43-58 with_nav_rail(rail_width)` —— 从 content 左侧切出，含夹紧（`layout.rs:44 rail_width.min(content.width)`） | Rust 侧契约更完整（切分 + 夹紧 + 测试 `layout.rs:85-102`）；sec-a 侧栏是「未接入的控件」 |
| 导航状态归属 | **调用方**：`MetroShell` 不持 selectedIndex；`MainActivity.kt:118 var selectedTab` + `:143-147` 传给 `MetroBottomNav` | **壳内持状态**：`lib.rs:27 pub nav: Navigation<PageId>`，`navigation.rs:13-20` 页栈 + `transition` + `leaving` | **归属相反**。Rust 壳即导航宿主；sec-a 导航是纯受控组件（stateless） |
| 导航模型 | 无。扁平 `selectedIndex: Int`（`MetroBottomNav.kt:75`），页面切换是调用方 `when` 切（`MainActivity.kt:161-172`）；KDoc 自承「顿」来自调用方整屏 destroy+mount（`MetroBottomNav.kt:66-67`） | `Navigation<PageId>` 页栈状态机：`navigate_to`/`go_back`/`can_go_back`/`leaving_page`/`is_transitioning`（`navigation.rs:33-83`） | **Rust 明显更强**，无需回流 |
| 内容留白 | 由叠层栈自动汇总：`contentPadding = bottomOverlayPadding()`（`MainActivity.kt:190`，`MetroDetailScaffold.kt:150`） | 无。`MetroScaffold` 是固定四边等 `padding`（`lib.rs:96-119`） | sec-a 概念可搬，见 §3-(b)-2 |
| 安全区归属 | 壳**不管** inset（`MetroShell.kt` 全文无 inset 引用） | 壳**不管** inset（`layout.rs` 全文无 inset 概念） | **一致** —— 两边都把 inset 放在壳之外 |
| 渲染模型 | 保留式（Compose 重组） | 无保留视觉树：`render` 每帧产 `Scene`（`lib.rs:46`），命中由 `LaidTree::hit_at` 派生（`canvas/layout.rs:246-272`） | 不可直接对照，但**构图契约要求「一次构图多方消费」**（`COMPOSITION.md:33`）与 sec-a 「画在哪点在哪」的 Compose 保证同源 |

**小结**：`MetroShell` 同名，但**语义相反**。Rust 版是「Scaffold 的轻量复刻」（切 AppBar + 可选 nav rail），sec-a 版是「overlay 宿主 + 叠层协调作用域」。这不是代码能否搬的问题，是**契约选哪边**的问题 —— 见 §4.1 与 §5-2。

### 2.2 响应式判据 / 断点

| 维度 | sec-a | 主仓 | 裁定 |
|---|---|---|---|
| 宽度断点表 | **不存在**。全仓 `grep "600.dp\|840.dp\|1000.dp\|WindowSizeClass\|breakpoint"` 零命中 | **存在**（Ether 侧）：`settings/src/kanesumi_window.rs:33 EXPANDED_BREAKPOINT: f32 = 720.0` | 主仓已有 |
| 断点 → 行为 | — | `kanesumi_window.rs:435-437`：`width >= 720.0` → `nav.set_pane_expanded(true && !user_collapsed)` | — |
| 断点取值 | — | **720.0**（展开/紧凑导航）；**640.0**（`kanesumi_window.rs:447 pad = if width >= 640.0 { 24.0 } else { 16.0 }`）；**320.0**（`kanesumi_window.rs:465 vertically_compact = inner.height < 320.0`） | 三条，非同族 |
| rail 几何 | `MetroSidebar.kt:65 width: Dp = 240.dp`（KDoc `:56` 注「Apple Music iPad 侧栏量级」） | `navigation_view.rs:18 NAV_PANE_EXPANDED = 320.0` / `:20 NAV_PANE_COMPACT = 48.0`；`navigation_view.rs:149-152 pane_width()` 按 `pane_progress` 插值；测试 `kanesumi_window.rs:1392 assert_eq!(compact, 48.0)` | **数值不一致**：240 vs 320，需裁定 |
| 断点切换的动画 | — | `navigation_view.rs:160-166 set_pane_expanded` → `self.pane.set_target(if expanded {1.0} else {0.0})`；`:24 NAV_TOP_HEIGHT = 48.0` | 主仓已有（Progress 驱动，参 §2.6） |
| 形态枚举 | **无**。`MetroSidebar.kt:46` KDoc 只写「平板 / 折叠展开 / 车机等宽屏」，无枚举、无判据 | `EtherRole` 枚举（`role.rs:6-19`）：`Desktop/Browser/TopBar/Dock/Launcher/Candidate` | **不同轴**，见下 |
| 内容最大宽 | `MetroResponsiveContent.kt:23 maxWidthDp: Dp = 360.dp`，`:32 widthIn(max=)`，`:28 contentAlignment = TopCenter`（KDoc `:17-18` 注「21:9 竖屏基准」） | **无对应物** | 可搬，见 §3-(a)-3 |
| 系统级角色分派 | 无（Android 无 layer/shell 概念） | `role.rs:69-78 surface_kind()`：`TopBar→LayerTop`、`Dock→LayerBottom`、`Launcher/Candidate→LayerOverlay`、`Desktop→LayerBackground`、`Browser→XdgShell` | **不算响应式断点** |

**关键裁定**：`role.rs` 的角色分派 **不是断点机制**。它是「**这个进程是哪种系统组件**」的**进程级静态环境变量**（`role.rs:59-64 from_env()` 读 `ETHER_ROLE`，非法即回退 Browser），与「窗口宽度变了，导航该换成侧栏吗」是**正交的两件事**：
- `EtherRole` 轴 = 空间职责（谁画在 Top 层、谁占排他区）；
- 断点轴 = 同一进程内、同一 surface 内的**布局重排**。

sec-a 的 `MetroSidebar`/`MetroBottomNav` 属后者，`EtherRole` 答不了后者。**主仓缺的正是一个 `structure` 层的断点判据**（现在它被埋在 `settings` 应用里）。

### 2.3 insets / 安全区

| 概念 | sec-a | 主仓 | 性质 |
|---|---|---|---|
| 状态栏 | `MetroInsets.kt:38 statusBar: EdgeInset`；`InsetModifiers.kt:9 metroStatusBarsPadding()` = `statusBarsPadding()` | TopBar 排他区：`config.rs:19-23 top_bar_height(output_h) = (output_h/36).max(30)`；harness `platform.rs:787 ls.set_exclusive_zone(height as i32)` | **同构但机制不同** |
| 导航栏 / 手势条 | `MetroInsets.kt:39 navigationBar`；`InsetModifiers.kt:11 metroNavigationBarsPadding()` | Dock 排他区：`platform.rs:787` 同款（`LayerBottom` 分支）；`config.rs:27 DOCK_HEIGHT: f64 = 64.0` | 同上 |
| 挖孔 / 刘海 | `MetroInsets.kt:25-34 DisplayCutoutInsets { top, bottom, left, right }`（四边，`rememberMetroInsets` 用 `calculateLeftPadding(layoutDirection)` 处理 RTL，`:67-68`） | **不存在**（桌面无挖孔） | **Android 专属** |
| IME / 键盘 | `MetroInsets.kt:41 ime: EdgeInset`（`WindowInsets.ime`），但**零消费者** | IME 走协议：`platform.rs:815-820 ZwpTextInputManagerV3`、`:824-828 ZwpInputMethodManagerV2`；候选窗是独立 layer-surface（`role.rs:17-18 Candidate → LayerOverlay`） | **不可通约**。Android 是「inset 数值」，Wayland 是「独立表面」 |
| Dp + Px 双份 | `MetroInsets.kt:18 data class EdgeInset(val dp: Dp, val px: Float)`（`Dp.toEdge()` `:74-75`） | Rust 侧只有逻辑像素 `f32`；物理缩放只在栅格化发生（`COMPOSITION.md:29-30` 契约 1） | **有意分歧**，Rust 侧设计更干净，见 §4.3 |
| 统一快照 | `MetroInsets` 单一 immutable 快照（`@Immutable`，`:36-42`） | `layer.rs:38-56 WorkArea { x, y, width, height }`，由 `calculate_work_area()` (`layer.rs:174-204`) 从排他区累计 | **这才是 Rust 侧的对位物** |
| 工作区计算 | — | `layer.rs:182-196`：`ExZone::Exclusive(px)` 且 `anchor_top && level == Top` → `top_exclusion`；`anchor_bottom && level == Bottom` → `bottom_exclusion`；左/右明文不计算（`layer.rs:173` 注释「当前 Ether 无左右边缘组件」） | 主仓已有，比 sec-a 更「已实装」 |
| 谁消费 | `rememberMetroInsets()` 唯一消费者 = `MainActivity.kt:187,205` 调试面板 | `WorkArea` 真实消费者：`state/xdg_shell.rs:85-89`（桌面窗口填满工作区）、`state/layer_shell.rs:47`、`state/pointer.rs:374,800`、`wm/mod.rs:132,329,357` | **主仓的 inset 是真在跑，sec-a 的是在打日志** |
| 底部叠层高度汇总 | `MetroBottomStack.kt:16-33`：`mutableStateMapOf<Any, Dp>` + `totalHeightDp = reservations.values.fold(0.dp) { acc, h -> acc + h }`（`:28-29`）—— **纯求和，不处理叠放遮挡** | 无 | 见 §3-(b)-2 |

**结论**：`MetroInsets` 回流的**概念**（单一真源快照）值得，但**实现**（`WindowInsets.*` 派生的四个 `PaddingValues`）不可搬；而且它的「护城河」地位在 sec-a 内部就没兑现。Rust 侧应把 `WorkArea` 当对位物，并在 §5-3 裁定是否需要一个 App 可见的 inset 快照。

### 2.4 层叠与遮挡（`MetroTopScrim`）

| 维度 | sec-a | 主仓 | 裁定 |
|---|---|---|---|
| 实现 | `MetroTopScrim.kt:48-58`：`Box(fillMaxWidth).height(scrimHeightDp).background(Brush.verticalGradient(0f to Black.copy(alpha=0.55f), 1f to Color.Transparent))` | `scene.rs:20-77 SceneCommand` 只有 `FillRect`(纯色)/`StrokeRect`/`Text`/`Arc`/`PushClip`/`PopClip`/`Image`/`Triangle`。**无渐变原语** | **冲突** |
| 取值 | `:40 scrimHeightDp: Dp = 120.dp`，`:41 scrimAlphaTop: Float = 0.55f`，`:43 iconSizeDp = 24.dp`，`:65 .size(48.dp)` 触控区 | — | — |
| 图标 | `:63 .metroStatusBarsPadding()`（自己吃掉状态栏），`:64 padding(4.dp)`，`:73-78 MetroIcon(tint = iconColor)`，`:44 indication: Indication? = null`（KDoc `:29-31`：返回箭头不做按动反馈） | 无对应物 | 图标逻辑可搬 |
| 正典依据 | `KANESUMI_DESIGN.md:102` 铁律 6「**无 ripple、无渐变、无阴影**」 | `scene.rs:21` 注释即「参 SD §II —— 纯色、无渐变」 | **sec-a 违规** |
| 旁证：sec-a 自己也承认 | 同仓 `MetroDrawer.kt:34` KDoc 批 M3：取代「M3 `ModalDrawer` 的 **tonal scrim 渐变** + 圆角抽屉面」—— 即 sec-a 一边反 M3 渐变、一边在 `MetroTopScrim` 用渐变 | `CONTROL_SPEC.md:1017`：「**Scene 纯色无渐变（铁律 6），Spectrum 2D 渐变用阶梯色带近似**」 | **已有先例可循** |

**调和方案（推荐）**：**阶梯色带（banded ramp）**。把 120dp 遮罩切成 N 段（例如 6 段 × 20dp），每段一个纯色 `FillRect`，alpha 由 `0.55 * (1 - i/N)` 递减。代价：6 个 `SceneCommand` 替代 1 个渐变；视觉上是硬边阶梯，但 20dp 段高在 ~2x 缩放下几乎不可辨（`emphasized` 抗锯齿会自然混合相邻段）。
**不建议**：(i) 为它加 `SceneCommand::Gradient` —— 那是把正典铁律 6 打开一个口子；(ii) 用模糊 —— Rust 侧无 blur 原语，且要引入 offscreen pass，与 `ETHER_RENDER_LESSONS.md` 的「离屏 wgpu → 读回」昂贵路径同类。
**登记要求**：若采用阶梯色带，按 `CANON_VS_TEMPORARY.md:9-11` 规则登记为 **Kanesumi 取值**（因为它不是 UWP 一手值），并在 `CONTROL_SPEC.md` 补一节（参 §ⅩⅨ 式的「Spectrum 阶梯色带」先例）。

### 2.5 滚动与壳层联动

| 维度 | sec-a | 主仓 | 裁定 |
|---|---|---|---|
| AppBar 收起/展开 | **完全没有**。全仓 `grep "nestedScroll\|NestedScroll\|firstVisibleItem\|collapse"` **零命中** | 无（`MetroAppBar` 无滚动感知） | **两边都没有**，无经验可回流 |
| 底栏隐藏 | **没有**（`MetroBottomNav` 无 `visible`/`hidden` 参数，`MetroBottomNav.kt:73-85`） | 无 | 同上 |
| 滚动→壳的唯一联动 | **留白**：`MainActivity.kt:190 contentPadding = bottomOverlayPadding()`；`MetroDetailScaffold.kt:150` 同款 | 无。`ShellLayout` 切完就完，无内容留白概念 | sec-a 侧有，见 §3-(b)-2 |
| fling 手感 | **有且完整**：`MetroScroll.kt:28-80 MetroFlingBehavior(initialVelocity)`：`:30 minFlingVelocity = 40f`、`:32 friction = 220f`、`:34 exponent = 1.5f`（`v^1.5` 超线性映射，`:47 target = sign(v) * absV.pow(exponent) / friction`）、`:35 baseDurationMs = 280`、`:37 perVelocityMs = 0.15f`、`:38-39 min/maxDurationMs = 280/1100`、`:40 easing = MetroQuintic`。边界处理 `:69-72`（`abs(delta - consumed) > 0.5f` → 归零速度 + `cancelAnimation()`，省 8~16 帧空转） | 无 fling 模型（`InputEvent::Scroll` 是离散步 50px/格，参 `CLAUDE.md` 输入层节） | **可搬（几何/数值）**，见 §3-(a)-4 |
| touchSlop | `MetroScroll.kt:92-95 metroViewConfiguration(base)` → `touchSlop = base.touchSlop * 1.5f`（KDoc `:89-91`：Compose 默认 ≈8dp → ≈12dp，去「Sensitive」印象） | 无 | 可搬数值 |
| Fling 注入点 | `MetroScroll.kt:84 val LocalMetroFling = staticCompositionLocalOf<FlingBehavior> { MetroFlingBehavior() }`（`:82-83`：不参与重组，无 Provider 时用默认实例） | 无对应物 | 需改造，见 §3-(b)-3 |
| 滚动位置保持 | 无（列表状态由调用方持） | `controls/scroll_view.rs` 有 `scroll_by`/`scroll_into_view`/`offset`（`scroll_view.rs:267-268` 测试） | Rust 侧已有基座 |

**关键结论**：任务书问的「`MetroScroll` 与 AppBar 收起/底栏隐藏等联动」—— **不存在**。`MetroScroll.kt` 是**纯滚动物理**（fling + slop），与壳层零耦合。它可回流的是**手感数值表**，不是联动逻辑。

### 2.6 状态与动画

| 场景 | sec-a（`Sokuou.kt` / 消费点） | 主仓（`presets.rs`） | 裁定 |
|---|---|---|---|
| 通用交互弹簧 | `Sokuou.kt:44 StandardInteraction = sokuouSpring(0.5f, 0.825f)`（换算 `:24-27 omega0 = 2π/response; stiffness = omega0²`） | `presets.rs:64-66 standard_interaction() = SpringAnim::new(0.50, 0.825, 0.0)` | **完全一致** ✅ |
| 快速交互弹簧 | `Sokuou.kt:47 QuickInteraction = sokuouSpring(0.3f, 0.6f)` | `presets.rs:69-71 quick_interaction() = SpringAnim::new(0.30, 0.60, 0.0)` | **一致** ✅ |
| 慢速展示弹簧 | `Sokuou.kt:50 SlowReveal = sokuouSpring(0.65f, 0.85f)` | `presets.rs:74-76 slow_reveal() = SpringAnim::new(0.65, 0.85, 0.0)` | **一致** ✅ |
| 弹窗入场弹簧 | `Sokuou.kt:53 DialogEnter = sokuouSpring(0.45f, 0.7f)` | `presets.rs:79-81 dialog_enter() = SpringAnim::new(0.45, 0.70, 0.0)` | **一致** ✅ |
| 页面转场弹簧 | `Sokuou.kt:56 PageTransition = sokuouSpring(0.4f, 0.8f)` | `presets.rs:84-86 page_transition() = SpringAnim::new(0.40, 0.80, 0.0)` | **一致** ✅ |
| 面板入场 | `Sokuou.kt:61-64 SheetAppear = tween(300, CubicBezierEasing(0.2f, 0f, 0f, 1f))` | `presets.rs:10 DURATION_SHEET_APPEAR = 0.30` + `:91-93 sheet_appear() = MetroAnim::new(0.30, UwpEasing::Cubic, EasingMode::EaseOut)` | **时长一致 / 曲线不同**（Bézier(0.2,0,0,1) vs Cubic EaseOut） |
| 面板收起 | `Sokuou.kt:67-70 SheetDismiss = tween(260, FastOutSlowInEasing)` | `presets.rs:15 DURATION_SHEET_DISMISS = 0.15`（一手源 150ms，注释 `:12-14` 明写「⚠ 旧值 0.26 无依据」）+ `:96-102 sheet_dismiss() = MetroAnim::new(0.15, Quadratic, EaseOut)` | **冲突且主仓已裁定**：sec-a 的 **260ms** 正是主仓 2026-09-22 用一手源**推翻**的旧值（`generic.xaml` L22110-22118，收起恰为展开一半）→ **sec-a 应改，不是主仓** |
| 快速切换 | `Sokuou.kt:73-76 QuickSwitch = tween(180, FastOutSlowInEasing)` | `presets.rs:24 DURATION_QUICK_SWITCH = 0.167`（WinUI `ControlFastAnimationDuration` 167ms，注释 `:16-23` 说明 UWP 无此键）+ `:105-111 quick_switch() = MetroAnim::new(0.167, Quadratic, EaseOut)` | sec-a 的 180 无依据，主仓 167 有源 |
| 封面淡入 | `Sokuou.kt:79-82 CoverFade = tween(400, CubicBezierEasing(0.2f,0f,0f,1f))` | `presets.rs:26 DURATION_COVER_FADE = 0.40` + `:114-116 cover_fade() = MetroAnim::new(0.40, Cubic, EaseOut)` | 时长一致 |
| 开关翻转 | `Sokuou.kt:91-94 ToggleFlip = tween(220, MetroCubic)` | `presets.rs:30 DURATION_TOGGLE_FLIP = 0.15`（UWP RepositionThemeAnimation，参 CONTROL_SPEC §3）+ `:128-130 toggle_flip() = MetroAnim::new(0.15, Cubic, EaseOut)` | **冲突**：220 vs 150，主仓有源 |
| 缓动族 | `UwpEasing.kt:18-30` sealed class 全族（Quadratic…Elastic）；`:115-122` 命名 easing：`MetroDefault = uwpEasing(Quadratic, EaseOut)`、`MetroCubic`、`MetroQuartic`、`MetroQuintic`、`MetroSine`、`MetroBackOut`、`MetroBounceOut`、`MetroElasticOut` | `kanesumi-anim/src/easings.rs` + sokuou `UwpEasing`；`presets.rs:1 use sokuou::{EasingMode, MetroAnim, SpringAnim, UwpEasing}` | **同源**（`UwpEasing.kt:12` 注明「从 PezMax-One/src/sokuou/uwp.rs 移植」，`AGENTS.md:109-111` 确认） ✅ |
| **壳层自身动画时长** | `MetroBottomNav.kt:92-97`：`LaunchedEffect(selectedIndex) { indicatorProgress.animateTo(selectedIndex, tween(200, MetroCubic)) }`；`MetroSidebar.kt:76-81`：**同款 200ms MetroCubic** | 无。`navigation.rs:9 DURATION_PAGE_TRANSITION = 0.25`，但全仓**唯一消费者是它自己的 `advance()`（`navigation.rs:88`）与测试**（`grep DURATION_PAGE_TRANSITION` 命中 5 处，全在 `navigation.rs`/`lib.rs` 重导出）；`presets.rs` 的 `page_transition()`/`cover_fade()` 也**零消费者** | **主仓壳层动画词汇表是空的** —— 指标条滑动时长无处可依 |
| 详情页 3 态转场 | `MetroDetailScaffold.kt:75-80 Crossfade(targetState = state, animationSpec = SokuouTweens.CoverFade)`；`:140-147 AnimatedVisibility(enter = fadeIn(tween(220, MetroDefault)) + slideInVertically(tween(220, MetroDefault), initialOffsetY = { slideOffsetPx }))`，`:136 slideOffsetPx = 12.dp.roundToPx()` | 无 Crossfade 原语；`Navigation` 只给 `transition_progress`（`navigation.rs:63-65`），双页同帧渲染由 App 负责（`navigation.rs:5` 注释） | 概念可搬，见 §3-(a)-4 / §4.4 |
| 抽屉 / 底部 sheet | `MetroDrawer.kt:56 progress: Animatable(0f)`，`:60-62` 入场 `SheetAppear`，`:64-69` 收起 `SheetDismiss`；`:75 alpha = progress * scrimAlpha`（默认 `:51 0.6f`）；`:89 translationX = -drawerWidthPx * (1 - progress)`。`MetroBottomSheet.kt:63-83` 同构，`:81 translationY = size.height * (1 - progress)`，`:59 dismissThreshold = 0.6f`，`:45-46` 拖拽 `progress.snapTo((progress - drag/sheetHeight).coerceIn(0,1))` | 无。`dialog.rs:175-179 overlay_alpha()`、`dialog.rs:244-247 scrim = overlay_color.with_alpha(...) * overlay_alpha()` + `scene.fill_rect(scrim, screen)` | Rust 侧 scrim 已在（纯色 + alpha），但**无 sheet/drawer 滑入控件** |
| 指示条零重组模式 | `MetroBottomNav.kt:119-121 activeAlpha = { (1f - abs(indicatorProgress.value - index)) .coerceIn(0f,1f) }`，`:131-133 graphicsLayer { translationX = indicatorProgress.value * tabWidthPx + indicatorLeftInTabPx }`；双 Icon 叠 Alpha（`:164-176`） | `tab_row.rs:11` V17「管道时长驱动滑行 + 文字色 crossfade」；`tab_row.rs:68/157/176` | **同款技术，主仓已有** ✅ |
| 低端机时序教训 | `MetroBottomNav.kt:58-64` KDoc：曾用 `launch(UNDISPATCHED)` 在 tap 回调同帧启动，低端机上父组件重组占 UI 线程数帧 → 动画计时已走到 40%+ → 视觉「跳」。改用 `LaunchedEffect(selectedIndex)` 等重组完成再启动 | Rust 侧无重组概念（每帧重建 Scene） | **Android 专属问题**，但「动画计时不等人」的教训对 `canvas/layout.rs` 的 `Progress` 推进也有参考价值 |

**裁定**：**动画词汇表的回流方向是「主仓 → sec-a」**。主仓 `presets.rs` 有权威数值与一手源注释（`UWP_PRIMARY_SOURCES.md`），sec-a 的 `Sokuou.kt` 是**旧快照**（`SheetDismiss` 260 / `QuickSwitch` 180 / `ToggleFlip` 220 全是主仓已推翻的值）。这是本次对比中**唯一一条反向回流项**，值得单独开一条给 sec-a 的 issue。

---

## §3 可回流清单

工作量口径：**小** = ≤2h，**中** = 0.5~1.5 人日，**大** = ≥2 人日。假设回流目标为纯 Rust 逻辑层（`kanesumi-structure` / `kanesumi-canvas` / `kanesumi-anim`），不含 Ether 应用侧接线。

### (a) 可直接搬（纯几何 / 状态机 / 断点表 / 数值表）

| # | 项 | sec-a 证据 | Rust 落点 | 量 | 备注 |
|---|---|---|---|---|---|
| a-1 | **底栏叠层高度汇总（求和栈）** | `MetroBottomStack.kt:16-33`（`reservations: MutableMap<Any, Dp>`、`register`/`unregister`/`totalHeightDp = fold(sum)`）+ `:49-56 rememberBottomStackReservation(key, heightDp)` + `:58-63 bottomOverlayPadding()` | `kanesumi-structure/src/layout.rs` 新增 `BottomStack { reservations: Vec<(Key, f32)> }` + `ShellLayout::with_bottom_overlay(h)`；**纯数据 + 纯函数，无平台依赖** | 小 | 注意 sec-a 是**纯求和**（`:29`），不处理叠放遮挡 —— 若要处理，需升级为「最大堆叠高度」而非求和，属 (b) 范畴。参见 §5-4 |
| a-2 | **断点常量表**（来源是主仓自己，见 §1 结论 1） | `settings/src/kanesumi_window.rs:33,447,465` | `kanesumi-structure/src/layout.rs` 新增 `pub const BREAKPOINT_NAV_EXPANDED: f32 = 720.0;` / `BREAKPOINT_PAD_WIDE: f32 = 640.0;` / `BREAKPOINT_VERT_COMPACT: f32 = 320.0;` + `pub fn shell_layout_for(window: Rect) -> ShellLayout`（内含断点分派，替代调用方手写 `>= 720.0`） | 小 | **纯搬**。顺带把 `settings` 的 `EXPANDED_BREAKPOINT` 改为引用它（消重） |
| a-3 | **内容最大宽居中**（响应式） | `MetroResponsiveContent.kt:21-37`：`maxWidthDp = 360.dp`、`widthIn(max =) + fillMaxHeight()`、`TopCenter` 对齐 | `kanesumi-structure/src/layout.rs`：`pub fn constrain_content_max_width(rect: Rect, max_w: f32) -> Rect`（居中收窄）+ `pub const CONTENT_MAX_WIDTH: f32 = 360.0`。**与 (a)-2 合起来就构成一个真正的 structure 层响应式模块** | 小 | 360.0 是 Android 21:9 基准，桌面是否沿用需一次裁定（§5-5） |
| a-4 | **Fling 手感数值表 + 触控容差** | `MetroScroll.kt:30,32,34,35,37,38,39,40,92-95`：`minFlingVelocity=40`、`friction=220`、`exponent=1.5`、`baseDurationMs=280`、`perVelocityMs=0.15`、`min/maxDurationMs=280/1100`、`easing=MetroQuintic`；`touchSlop ×1.5` | `kanesumi-anim/src/presets.rs`：`pub struct FlingProfile { min_velocity: f32, friction: f32, exponent: f32, base_duration_ms: f32, per_velocity_ms: f32, min_duration_ms: u32, max_duration_ms: u32 }` + `FlingProfile::metro()`；`kanesumi-controls/src/scroll_view.rs` 消费（`target = sign(v) * \|v\|^exponent / friction`，`duration = (base + \|v\| * per_velocity).clamp`） | 小~中 | **公式是纯数学**，可直接搬。边界处理（`MetroScroll.kt:69-72` 的 `\|delta - consumed\| > 0.5` → 归零 + cancel）也一并搬，省 8~16 帧空转 |
| a-5 | **详情页 3 态枚举 + 缓存短路** | `MetroDetailScaffold.kt:69-73`：`error != null → Error; isLoading && !hasCachedContent → Loading; else → Content`；`:158 private enum class DetailState` | `kanesumi-structure/src/lib.rs`：`pub enum PageState { Loading, Error, Content }` + `pub fn resolve_page_state(is_loading: bool, has_cached: bool, error: Option<&str>) -> PageState` | 小 | 纯状态机。`hasCachedContent` 短路（有缓存就不显示 spinner，`:38-40` KDoc）是**有价值的行为细节**，值得做成契约 |
| a-6 | **图标触控区 + 状态栏自吃** | `MetroTopScrim.kt:40,43,63,64`：`scrimHeightDp=120`、`iconSizeDp=24`、`.size(48.dp)` 触控区、`.metroStatusBarsPadding()` | `kanesumi-controls`（若采纳 §4.2 的纯色 scrim） | 小 | 48dp 是最小可触尺寸（`MetroTopScrim.kt:29-31` KDoc 明写无障碍依据） |

### (b) 需改造（Compose 的 `Scaffold`/`WindowInsets`/`AnimatedVisibility` 等）

| # | 项 | sec-a 证据 | 改造要点 | Rust 落点 | 量 |
|---|---|---|---|---|---|
| b-1 | **「底栏 = overlay，内容铺满 + 自适应留白」契约** | `MetroShell.kt:20-23`（KDoc 明写这是「Metro 与 M3 最大的分歧」）、`:39-52` 实现 | 不是搬代码，是**改 Rust 的 `ShellLayout` 语义**：现 `layout.rs:26-39` 把 window 切成 `app_bar + content`（content 高度已减去 48），若改 overlay 则 content 应是**完整 window**、bottom bar 矩形另列。**这是破坏性契约变更**，需 `with_nav_rail` 与新 `with_bottom_overlay` 的一致性审查 + `lib.rs:147-158` 的 3 个既有测试重算 | `kanesumi-structure/src/layout.rs`（+ `lib.rs:46-52 render` 顺序：背景 → 内容 → **最后**画 bottom overlay） | 中 |
| b-2 | **`MetroBottomStackScope` 的 Compose 作用域机制** | `MetroBottomStack.kt:35-41 LocalMetroBottomStack`（`staticCompositionLocalOf`，未提供时 `error(...)` 崩溃）+ `:43-47 MetroBottomStackScope` + `DisposableEffect(stack, key, heightDp) { register; onDispose { unregister } }`（`:52-55`） | `DisposableEffect` 的「重组即重注册 / 离开即注销」在 Rust 无对应物 —— Rust 是**每帧重建 Scene**，所以正确形态是「**每帧重建 reservations 表**」而非「注册/注销」。改造点：`ShellLayout` 每帧从 App 拿一个 `&[(ReservationKey, f32)]`，`total` 由 `layout()` 现算，**不做增量维护**。副作用：Rust 侧不需要 key 类型（无需 `unregister`），只需 `Vec<f32>` 或带标签的 `Vec<(&'static str, f32)>` | `kanesumi-structure/src/layout.rs` | 中 |
| b-3 | **`MetroInsets` 快照抽象** | `MetroInsets.kt:36-51`（`@Immutable data class`，四个 `EdgeInset` + 四边 `DisplayCutoutInsets`）+ `:53-72 rememberMetroInsets()` | `WindowInsets.{statusBars,navigationBars,displayCutout,ime}.asPaddingValues()` **完全不可搬**。Rust 对位物是 `WorkArea`（`layer.rs:38-56`），但它由**合成器**算、**App** 看不到。改造要点：是否给 `App` 加一个 `fn safe_area(&self) -> Insets`（由 harness 在 configure 时注入）？现有替代路径是 App 硬编码（TopBar `height=30`、`Dock_HEIGHT=64`）。另：`EdgeInset{ dp, px }` 双份表示与 `COMPOSITION.md:29-30` 契约 1「布局只用 f32 逻辑像素」**直接冲突**，Rust 侧只能保留一份 | `kanesumi-harness/src/app.rs`（新增 trait 方法）+ `kanesumi-core/src/geometry.rs`（若加 `Insets` 结构） | 中 |
| b-4 | **`MetroTopScrim` 渐隐遮罩** | `MetroTopScrim.kt:48-58` `Brush.verticalGradient` | **不得搬渐变**（`KANESUMI_DESIGN.md:102` 铁律 6）。改造为 **6 段阶梯色带**（每段纯色 `FillRect`，alpha `0.55 * (1 - i/6)`），或按 `CONTROL_SPEC.md:1017` 已有先例登记。附带：`indication = null` 默认（`:44`）在 Rust 侧表现为「不登记 hover/press 态」 | `kanesumi-controls`（新控件 `TopScrim`）+ 若需色带助手则 `kanesumi-canvas/src/scene.rs` | 中 |
| b-5 | **`MetroDetailScaffold` 的 `Crossfade` + `AnimatedVisibility`** | `MetroDetailScaffold.kt:75-86` `Crossfade(targetState, SokuouTweens.CoverFade)`；`:140-147` `fadeIn + slideInVertically(initialOffsetY = { 12.dp.roundToPx() })`，`:137-139 MutableTransitionState(false).apply { targetState = true }`（首帧即入场） | Rust 无 Crossfade/AnimatedVisibility。改造要点：需要一个「**双分支同帧渲染 + 各自 alpha/offset 由 progress 驱动**」的手写路径。注意 `COMPOSITION.md:34` 契约 3「一次构图，多方消费」与 `KANESUMI_DESIGN.md:99` 铁律 3「动画只动视觉属性，不动布局」→ **入场 slide 只能走绘制偏移，不得改 `LayoutNode` 的 rect**（否则违反铁律 3）。而 sec-a 的 `AnimatedVisibility + slideInVertically` 在 Compose 里**是改布局的**（它推动 sibling），这正是一条需要改造的语义差异 | `kanesumi-structure/src/navigation.rs`（扩 `transition_progress` 为可查询的 entering/leaving 双方 progress）+ App 侧渲染 | 中~大 |
| b-6 | **抽屉 / 底部 sheet 滑入** | `MetroDrawer.kt:56-95`、`MetroBottomSheet.kt:63-100`：单 `Animatable<Float>` 驱动 `translationX/Y` + `alpha`；`:45-46` 拖拽 `snapTo`；`:59 dismissThreshold = 0.6f` | 同 (b)-5：`translationX/Y` 在 Rust 侧要么改 rect（违反铁律 3）要么走绘制偏移（需 `SceneCommand` 支持或调用方自算目的地）。`dialog.rs:244-247` 已有 scrim 先例，可扩展为通用 `Sheet`。拖拽 `snapTo` 语义（手势中途直接改 progress、不插值）在 Rust 的 `Progress`（`sokuou`）上需确认是否有等价的 `jump_to` | `kanesumi-controls`（新 `Sheet`/`Drawer`） | 中~大 |
| b-7 | **壳层动画时长补齐** | `MetroBottomNav.kt:92-97`、`MetroSidebar.kt:76-81`：指示条滑动 **200ms MetroCubic** | 主仓 `presets.rs` **没有壳层指标条时长**（`page_transition()`/`cover_fade()` 零消费者，`DURATION_PAGE_TRANSITION` 只被 `navigation.rs` 自己用）。改造要点：新增 `pub fn shell_indicator() -> MetroAnim { MetroAnim::new(0.20, UwpEasing::Cubic, EasingMode::EaseOut) }`，并**按 `presets.rs` 的风格标注来源** —— 200ms 目前**无一手源**，须按 `CANON_VS_TEMPORARY.md:9-11` 登记为 Kanesumi 取值 | `kanesumi-anim/src/presets.rs` | 小~中 |

### (c) 不可搬

| # | 项 | 证据 | 理由 |
|---|---|---|---|
| c-1 | `WindowInsets.{statusBars,navigationBars,displayCutout,ime}` 及其 `PaddingValues` | `MetroInsets.kt:3-8,57-60` | Android 窗口系统专有。Wayland 侧对应机制是 layer-shell 排他区（协议层，合成器算，已实装于 `layer.rs:174-204`） |
| c-2 | `enableEdgeToEdge()` / Activity 边到边 | `MainActivity.kt:86` | Android Activity 专有 |
| c-3 | `Modifier.graphicsLayer { }` 的零重组 draw-phase 读取 | `MetroBottomNav.kt:131-133,175,189`；`MetroDrawer.kt:75,89` | Compose 专有优化。Rust 是每帧重建 Scene，**该问题不存在**；对应正典条目 `KANESUMI_DESIGN.md:98`（GPU 零重组）在 Rust 侧无对位物 |
| c-4 | `LocalIndication` / `Indication` 注入 | `MetroTopScrim.kt:20,44,68`；`AGENTS.md:84-85` | Compose `Indication` 体系专有。Rust 侧 `kanesumi-core/src/indicator.rs` 是显式令牌 + 控件自绘，不是注入 |
| c-5 | `rememberBottomStackReservation` 的 `DisposableEffect` 生命周期 | `MetroBottomStack.kt:52-55` | 依赖 Compose 重组/离开语义，Rust 无对位物（见 (b)-2 的改造方案） |
| c-6 | `MetroSidebar` 作为独立控件 | `MetroSidebar.kt:60-131` | **不搬的理由是主仓已有更强的** `MetroNavigationView`（`navigation_view.rs:18-20,149-181`：pane 展开/收窄插值、toggle 40×40、item 高 40）。sec-a 侧栏是静态列 + 指示条，功能是子集 |
| c-7 | `MetroFlingBehavior : FlingBehavior`（Compose 接口实现） | `MetroScroll.kt:28-80` | 接口不可搬；**数值可搬**（已计入 (a)-4） |
| c-8 | `LocalMetroFling` 的 `staticCompositionLocalOf` 注入 | `MetroScroll.kt:84` | 同 c-4。Rust 侧形态应是 `AppConfig` 字段或 `MetroShell` 字段 |

### (d) 反向回流（主仓 → sec-a）—— 本评估的意外收获

| # | 项 | 主仓权威值 | sec-a 现值 | 影响 |
|---|---|---|---|---|
| d-1 | 面板收起时长 | `presets.rs:15 DURATION_SHEET_DISMISS = 0.15`（一手源 `generic.xaml` L22110-22118；`:12-14` 明写旧值 0.26 已被推翻） | `Sokuou.kt:67-70 tween(260, FastOutSlowInEasing)`，`SokuouTweens.SheetDismiss`（`:106`） | sec-a 的 `MetroDrawer.kt:39`、`MetroBottomSheet.kt:38` 都依赖它 → **面板收起慢 73%** |
| d-2 | 快速切换时长 | `presets.rs:24 DURATION_QUICK_SWITCH = 0.167`（WinUI `ControlFastAnimationDuration`） | `Sokuou.kt:73-76 tween(180, FastOutSlowInEasing)`（`:107`） | 轻微 |
| d-3 | 开关翻转时长 | `presets.rs:30 DURATION_TOGGLE_FLIP = 0.15`（UWP RepositionThemeAnimation） | `Sokuou.kt:91-94 tween(220, MetroCubic)`（`:109`） | sec-a 的 `MetroSwitch` 慢 47% |

> 这三条**不是**本次任务要求的「回流」，但既然对比发现了，应反馈给 sec-a 侧（或至少登记）。sec-a 的 `Sokuou.kt` 是旧快照，未同步主仓 2026-09-22 的一手源更正。

---

## §4 关键代码片段与 Rust 等价写法

### 4.1 壳层契约：overlay vs split（本次最核心的一条）

**sec-a（`MetroShell.kt:39-52`，照抄）**
```kotlin
MetroBottomStackScope {
    Box(
        modifier = modifier
            .fillMaxSize()
            .background(background),
    ) {
        content()
        if (bottomBar != null) {
            Box(Modifier.align(Alignment.BottomCenter)) {
                bottomBar()
            }
        }
    }
}
```
配套 KDoc（`MetroShell.kt:20-23`，照抄）：
> 把 bottomBar 作为 overlay 画在 content 之上底部 —— 这是 Metro 与 M3 最大的分歧:M3 Scaffold 把 bottomBar 从 content 空间里"切"出来,Metro 让 content 铺满全屏、bottomBar 悬浮,内容侧靠 bottomOverlayPadding() 自适应留白。

**主仓现状（`layout.rs:24-40`，照抄）** —— 注意这正是 sec-a 批的 Scaffold 行为：
```rust
pub fn of(window: Rect) -> Self {
    Self {
        app_bar: Rect::new(
            window.origin.x,
            window.origin.y,
            window.size.width,
            APP_BAR_HEIGHT.min(window.size.height),
        ),
        content: Rect::new(
            window.origin.x,
            window.origin.y + APP_BAR_HEIGHT.min(window.size.height),
            window.size.width,
            window.size.height - APP_BAR_HEIGHT.min(window.size.height),
        ),
        nav_rail: None,
    }
}
```

**Rust 等价写法（建议，非本次改动）**
```rust
/// 壳布局结果 —— 一次划分，控件消费。参 COMPOSITION.md §强制契约 3（一次构图，多方消费）。
pub struct ShellLayout {
    pub app_bar: Rect,
    /// 内容区。**overlay 语义**：不扣除底部叠层高度 —— 底栏悬浮其上。
    pub content: Rect,
    pub nav_rail: Option<Rect>,
    /// 底部叠层保留高度（悬浮 overlay 占位，参 sec-a MetroBottomStack）。
    pub bottom_overlay: f32,
}

impl ShellLayout {
    /// 悬浮叠层：content 保持满窗，仅记录叠层高度供内容自留白。
    pub fn with_bottom_overlay(mut self, h: f32) -> Self {
        self.bottom_overlay = h.max(0.0);
        self
    }
    /// 内容侧自适应留白矩形（等价 sec-a `bottomOverlayPadding()`）。
    pub fn content_padded(&self) -> Rect {
        Rect::new(
            self.content.origin.x,
            self.content.origin.y,
            self.content.size.width,
            (self.content.size.height - self.bottom_overlay).max(0.0),
        )
    }
}
```
**取舍**：`app_bar` 是否保留？若同时采 sec-a 的「顶栏 = 滚动列表第一项」（`MetroShell.kt:29-30`），则 `app_bar` 应删除、由 App 自己在 `content` 内画。这是**二选一**，见 §5-2。

### 4.2 顶部渐隐 → 阶梯色带

**sec-a（`MetroTopScrim.kt:48-58`，照抄）**
```kotlin
Box(
    modifier = Modifier
        .fillMaxWidth()
        .height(scrimHeightDp)
        .background(
            Brush.verticalGradient(
                0f to Color.Black.copy(alpha = scrimAlphaTop),
                1f to Color.Transparent,
            )
        )
)
```

**主仓等价写法（建议）** —— `scene.rs:20-77` 无渐变原语，用 N 段纯色 `FillRect` 近似（先例：`CONTROL_SPEC.md:1017` 的 Spectrum 阶梯色带）：
```rust
/// 顶部渐隐遮罩 —— 阶梯色带近似。参 KANESUMI_DESIGN.md §Ⅳ-6「无渐变」，
/// 与 CONTROL_SPEC §Spectrum 的阶梯色带同款手法（Scene 无渐变原语）。
/// 600 段高在 ~2x 缩放下硬边不可辨；段数须为偶数以保证端点对齐像素。
pub fn top_scrim(scene: &mut Scene, rect: Rect, alpha_top: f32, bands: usize) {
    let n = bands.max(2);
    let band_h = rect.size.height / n as f32;
    for i in 0..n {
        // 顶部最浓，向下线性衰减到 0（对齐 sec-a scrimAlphaTop 0.55 / 120dp）
        let a = alpha_top * (1.0 - i as f32 / n as f32);
        if a <= 0.0 {
            continue;
        }
        scene.fill_rect(
            Color::BLACK.with_alpha(a),
            Rect::new(
                rect.origin.x,
                rect.origin.y + band_h * i as f32,
                rect.size.width,
                band_h,
            ),
        );
    }
}
```
> 注：Rust 侧 `with_alpha` 直传数字会被 `kanesumi-controls/tests/token_discipline.rs:79-81` 的静态检查拦下（「强度须用具名令牌」）。上例的 `a` 是**计算值**不是字面量，但落地时应把 `alpha_top` 做成具名令牌（如 `themed.top_scrim_top`），以通过 M1-3 的 token discipline 门。

### 4.3 insets：`MetroInsets` vs `WorkArea`

**sec-a（`MetroInsets.kt:36-42, 53-72`，摘录）**
```kotlin
@Immutable
data class MetroInsets(
    val statusBar: EdgeInset,
    val navigationBar: EdgeInset,
    val displayCutout: DisplayCutoutInsets,
    val ime: EdgeInset,
)

@Composable
fun rememberMetroInsets(): MetroInsets {
    val density = LocalDensity.current
    val layoutDirection = LocalLayoutDirection.current
    val statusBars = WindowInsets.statusBars.asPaddingValues()
    // …（navigationBars / displayCutout / ime 同款）
    return MetroInsets(
        statusBar = statusBars.calculateTopPadding().toEdge(density),
        navigationBar = navigationBars.calculateBottomPadding().toEdge(density),
        displayCutout = DisplayCutoutInsets(/* 四边 */),
        ime = ime.calculateBottomPadding().toEdge(density),
    )
}
```

**主仓对位物（`compositor/src/layer.rs:174-204`，照抄核心）**
```rust
pub fn calculate_work_area(&self, output_w: i32, output_h: i32) -> WorkArea {
    let mut top_exclusion = self.external_top_exclusion as i32;
    let mut bottom_exclusion = 0i32;

    for entry in &self.internal_layers {
        if !entry.visible { continue; }
        match entry.exclusive_zone {
            ExZone::Exclusive(px) => {
                let px_i32 = px as i32;
                if entry.anchor_top && entry.level == LayerLevel::Top {
                    top_exclusion = top_exclusion.max(px_i32);
                }
                if entry.anchor_bottom && entry.level == LayerLevel::Bottom {
                    bottom_exclusion = bottom_exclusion.max(px_i32);
                }
            }
            ExZone::Neutral | ExZone::None => {}
        }
    }

    WorkArea {
        x: 0,
        y: top_exclusion,
        width: output_w,
        height: output_h - top_exclusion - bottom_exclusion,
    }
}
```

**对照要点**：
- 概念对位：`MetroInsets.statusBar` ↔ `top_exclusion`（来源：TopBar 表面 `platform.rs:787 set_exclusive_zone(height)`）；`MetroInsets.navigationBar` ↔ `bottom_exclusion`（Dock）。
- **方向相反**：Android 是「App 读 inset 自己避让」；Wayland 是「合成器算好后**给窗口分配更小的区域**」（`state/xdg_shell.rs:85-89` 桌面窗口直接 `mark_as_desktop(&wa)` 填满工作区）。**App 根本不需要 inset 数值** —— 这是机制上的优越，不是缺失。
- 缺口：**非全屏窗口（xdg-shell Browser）不消费 `WorkArea`**（`calculate_work_area` 的消费者是桌面/最大化/命中路径，`wm/mod.rs:132,329,357`；普通 `add_window` 是 cascade 放置，`wm/policy/layout.rs`）。若 TopBar 展开（`App::preferred_height`，`app.rs:252-259`）改变了排他区，普通窗口不知道。**这才是真正需要补的缺口**，而不是「补一个 Android 式 insets 结构」。
- `EdgeInset{ dp, px }` 双份表示**不可搬**：`COMPOSITION.md:29-30` 契约 1 要求「布局只使用 `f32` 逻辑像素，物理缩放只进入栅格化；不得按显示器分辨率分支」。Rust 侧只保留逻辑像素。

### 4.4 指示条零重组 → Progress 驱动几何

**sec-a（`MetroBottomNav.kt:90-97, 119-135`，摘录）**
```kotlin
val indicatorProgress = remember { Animatable(selectedIndex.toFloat()) }

LaunchedEffect(selectedIndex) {
    indicatorProgress.animateTo(
        targetValue = selectedIndex.toFloat(),
        animationSpec = tween(200, easing = MetroCubic),
    )
}
// …
// 每个 tab 的 active alpha = 1 - |indicator - index|,clamp [0,1]。
activeAlpha = {
    (1f - abs(indicatorProgress.value - index.toFloat())).coerceIn(0f, 1f)
},
// …
Box(
    modifier = Modifier
        .align(Alignment.TopStart)
        .size(indicatorSize)
        .graphicsLayer {
            translationX = indicatorProgress.value * tabWidthPx + indicatorLeftInTabPx
        }
        .background(indicatorColor),
)
```
`MetroSidebar.kt:107-124` 是同款，只是轴向换 Y、指示条 `width(3.dp).height(rowHeight)`：
```kotlin
.graphicsLayer { translationY = indicatorProgress.value * rowHeightPx }
```

**主仓等价写法** —— 主仓**已有同款技术**（`tab_row.rs:11` V17「管道时长驱动滑行 + 文字色 crossfade」），其形态是「progress → 几何」的纯函数。可搬的**只是命名与数值**（200ms MetroCubic）：
```rust
// kanesumi-anim/src/presets.rs 追加。
// ⚠ 200ms 无一手源 —— 来自 sec-a MetroBottomNav.kt:95 / MetroSidebar.kt:79 的实测值，
// 按 CANON_VS_TEMPORARY.md §一 登记为 Kanesumi 取值（勿写作 UWP 规格）。
/// 壳层指示条滑行时长（底部导航 / 侧栏共用）。
pub const DURATION_SHELL_INDICATOR: f64 = 0.20;

pub fn shell_indicator() -> MetroAnim {
    MetroAnim::new(DURATION_SHELL_INDICATOR, UwpEasing::Cubic, EasingMode::EaseOut)
}
```
几何侧的 Rust 惯用写法（`kanesumi-structure` 风格，对照 `navigation_view.rs:149-152 pane_width()`）：
```rust
/// 指示条位置 —— progress ∈ [0, n-1]，index 项 alpha = 1 - |progress - index|。
/// 与 sec-a MetroBottomNav.kt:119-121 同式；保证「图标动、指示条同源」，无错位。
pub fn indicator_offset(progress: f32, tab_extent: f32, indicator_extent: f32) -> f32 {
    progress * tab_extent + (tab_extent - indicator_extent) / 2.0
}

pub fn item_active_alpha(progress: f32, index: usize) -> f32 {
    (1.0 - (progress - index as f32).abs()).clamp(0.0, 1.0)
}
```
验证锚点：`MetroBottomNav.kt:116-118` KDoc 明写「indicator 恰好停在 index 时该 tab alpha=1;滑到中间时相邻两 tab 各 0.5」→ 可直接做单测。

### 4.5 叠层汇总栈

**sec-a（`MetroBottomStack.kt:16-33, 49-60`，摘录）**
```kotlin
@Stable
class MetroBottomStack internal constructor() {
    private val reservations = mutableStateMapOf<Any, Dp>()
    internal fun register(key: Any, height: Dp) { reservations[key] = height }
    internal fun unregister(key: Any) { reservations.remove(key) }
    val totalHeightDp: Dp
        get() = reservations.values.fold(0.dp) { acc, height -> acc + height }
    val reservationsByKey: Map<Any, Dp> get() = reservations.toMap()
}

@Composable
fun bottomOverlayPadding(): PaddingValues =
    PaddingValues(bottom = LocalMetroBottomStack.current.totalHeightDp)
```
典型用法（`MainActivity.kt:135-149, 156-157, 190`）：bottomBar 里 `Column { if (miniBarVisible) OverlayBar(56dp); MetroBottomNav(...) }`，nav 自己经 `autoReserveBottomStack`（`MetroBottomNav.kt:83-88`，`reservationKey = "kanesumi.metroBottomNav"`、`heightDp = 56.dp`）登记，mini player 由调用方登记（`:156`），内容侧一句 `contentPadding = bottomOverlayPadding()`。

**Rust 等价写法（建议）** —— 注意 Rust 无 `DisposableEffect`，改为**每帧重建**：
```rust
/// 底部叠层汇总 —— 每帧重建（Rust 侧无 Compose 的重组/离开生命周期，
/// 故不做 register/unregister 增量维护，直接从 App 取当帧清单）。
/// 对位 sec-a MetroBottomStack.totalHeightDp（MetroBottomStack.kt:28-29）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BottomStack {
    reservations: Vec<(&'static str, f32)>,
}

impl BottomStack {
    pub fn new() -> Self { Self::default() }

    /// 登记一项底部叠层高度（等价 rememberBottomStackReservation）。
    pub fn reserve(&mut self, key: &'static str, height: f32) -> &mut Self {
        self.reservations.push((key, height.max(0.0)));
        self
    }

    /// 汇总高度（等价 totalHeightDp）。纯求和 —— 与 sec-a 一致，
    /// 不处理叠放遮挡（两者都是纵向堆叠，无遮挡）。
    pub fn total_height(&self) -> f32 {
        self.reservations.iter().map(|(_, h)| h).sum()
    }
}
```
集成方式：`MetroShell::layout(window, &bottom)` 或 `App::bottom_stack(&self) -> BottomStack`（后者更贴近「每帧重建」）。

---

## §5 不确定 / 需人裁定

> 以下每条都给出**证据**与**建议**，但需要维护者定夺。前两条阻塞 M2。

1. **「SD §II 禁渐变」溯源错误 —— 需要一次文档更正。**
   `scene.rs:21`、`CLAUDE.md`（Ether 仓根）设计不变量表、`compositor/CLAUDE.md` 设计不变量表都写「纯色，无渐变 ← 参 SD §II」。但 `compositor/docs/SD.md`（413 行，全文）**0 次出现「渐变」**；其 §Ⅱ（行 36-108）只讲 Base/TopBar/Dock/Launchpad 的空间结构，一个字没提颜色。
   真正的源头是 `docs/KANESUMI_DESIGN.md:102`（§Ⅳ 铁律 6）与 `:77`（§Ⅲ.3）。**建议**：把这些 `参 SD §II` 改成 `参 KANESUMI_DESIGN.md §Ⅳ-6`（或 `参 SD §II / KANESUMI_DESIGN §Ⅳ-6`）。这是纯文档修正，但影响到 §4.2 的裁定依据链。

2. **M2 要接管的是「split」还是「overlay」壳层契约？**（阻塞 M2-2/M2-4）
   现状：Rust `ShellLayout` 是 split（`layout.rs:26-39` 切 48px 给 app_bar），sec-a `MetroShell` 是 overlay（`MetroShell.kt:39-52`，且 KDoc `:20-23` 把 split 明确点名为「M3 Scaffold」并拒绝）。
   两者不能并存（同一个 `ShellLayout` 无法既是切分又是叠加）。
   **建议**：采 overlay（理由：`KANESUMI_DESIGN.md:63` 正典已明文背书「底部叠层作为悬浮 overlay 画在内容之上，内容侧经 `MetroBottomStack` 自适应留白——这是 M3 Scaffold『切空间』的刻意反叛」）。但这会使 `layout.rs:66-102` 的 4 个既有测试与 `lib.rs:147-158` 的 `shell_render_returns_content_rect` 需重算。
   **若采 overlay，还须同时裁定**：`app_bar` 字段留不留？sec-a 的立场是「顶栏 = 滚动列表第一项」（`MetroShell.kt:29-30`，`MetroAppBar.kt:26-31`）—— 若照此，`ShellLayout.app_bar` 应删除，但会与 `settings`/`librarian` 现有的固定 AppBar 用法冲突（`lib.rs:46-52 render` 现在直接画 app_bar）。

3. **是否需要 App 可见的安全区快照？**（不阻塞，但影响 §3-(b)-3 的取舍）
   现在 App 避让 TopBar/Dock 的方式是**硬编码高度**（TopBar `height = 30`，参 `settings/CLAUDE.md`"TopBar ... layer-shell TOP 30px"；Dock `DOCK_HEIGHT = 64.0`，`config.rs:27`）。而 `App::preferred_height`（`app.rs:252-259`）允许 TopBar **动态变高**（控制中心面板展开），此时排他区会变、普通窗口不知道。
   **建议**：不加 Android 式 `Insets` 结构，而给 `App` 加一个窄接口 `fn safe_area(&self) -> Rect`（由 harness 在 `LayerShellHandler::configure` / `XdgShellHandler` 里注入 `WorkArea`，`layer.rs:198-203`）。这比 `MetroInsets` 更省，且复用已有 `WorkArea`。
   **需人裁定**：这是否属于 M2 范围（M2 的 5 个批次都没提 inset）？若否，应单独登记为 M2 之后的项。

4. **`MetroBottomStack` 的「纯求和」是否够用？**
   `MetroBottomStack.kt:28-29` 是**纯求和**。在「mini player 56dp + bottom nav 56dp」的用例下正确（两者纵向堆叠，`MainActivity.kt:135-149`）。
   但若出现**叠放**（如悬浮按钮压在底栏上、或 sheet 从底栏之下滑出），求和会**高估**留白。sec-a 无此用例（无证据）。
   **建议**：回流时**先照搬求和**（与 sec-a 一致，`a-1`），并在 KDoc 里写明「本表语义为纵向堆叠之和；叠放场景需另立 API」。不要在无用例的情况下预先设计遮挡算法。

5. **`CONTENT_MAX_WIDTH = 360.0` 在桌面是否沿用？**
   `MetroResponsiveContent.kt:17-18` KDoc：360dp「对应 21:9 竖屏基准 —— 与 Ncrust 长期实践值一致」，并提到 iPad 竖屏惯用 414dp。
   桌面（1920×1080 / 3072×1920）下 360 会显得极窄；而主仓现状是「内容铺满窗口」（`settings` 窗口 960×640 定宽，`kanesumi_window.rs:31-32`）。
   **建议**：把 360.0 当**手机形态默认**，桌面形态用另一个值（或 `None` = 不限制），并登记取值来源。**需人裁定** 具体数值。

6. **指示条 200ms（`b-7`）的归属。**
   `MetroBottomNav.kt:95` / `MetroSidebar.kt:79` 都是 200ms MetroCubic，但**无一手源**。主仓 `presets.rs` 的惯例是「每个时长标注来源」（一手源 or 明写 Kanesumi 取值）。
   **建议**：按 `CANON_VS_TEMPORARY.md:36`（T17 的处置先例：「不得写成 UWP 规格」）登记为 Kanesumi 取值。
   **需人裁定**：是否先去 `UWP_PRIMARY_SOURCES.md` 的 `generic.xaml` 找 `Pivot`/`NavigationView` 指示条的一手时长，再决定用 200ms 还是 UWP 值。

7. **`MetroTopScrim` 的阶梯段数。**
   §4.2 建议 6 段（120dp / 20dp），但段数是**视觉裁定**不是技术裁定：段数太少 → 硬边可见；太多 → `SceneCommand` 膨胀（`Scene` 每帧重建，`scene.rs:80-83`）。
   **需人裁定**：需一次真机视觉确认（与 `CANON_VS_TEMPORARY.md:89-90` 的 T13/T15 同类「需一次真机确认」）。
   **另需裁定**：阶梯色带**是否要在 `CONTROL_SPEC.md` 立项**。按 `AGENTS.md` §二「该节缺失时先补规格再写实现」，应先补规格。

8. **`MetroInsets.ime` 在主仓是否需要对位物？**
   sec-a 有 `MetroInsets.kt:41 ime: EdgeInset`（零消费者）。主仓的 IME 是**独立 layer-surface**（`role.rs:17-18 Candidate`，`platform.rs:815-828`），候选窗跟随光标是合成器/协议的事，不是 App 的 inset。
   **建议**：明确判定为**不可搬且不需要对位物**（已计入 c-1）。但这需要维护者确认：是否存在「IME 弹出时 App 内容需要避让」的桌面场景（如 Ceyboard 全屏态）？若有，则需另立机制。

---

## 附：本次评估的检索命令留痕（可复现）

```
# sec-a 断点（结果：无）
grep -rn "BoxWithConstraints|maxWidth|WindowSizeClass|600\.dp|840\.dp|1000\.dp|breakpoint" --include=*.kt Kanesumi-sec-a

# sec-a 侧栏引用（结果：仅自身 KDoc）
grep -rn "MetroSidebar" --include=*.kt Kanesumi-sec-a

# sec-a 滚动联动（结果：无）
grep -rn "nestedScroll|firstVisibleItem|collapse" --include=*.kt Kanesumi-sec-a

# sec-a 形态切换（结果：无 WindowSizeClass / 无折叠分支）
grep -rn "WindowSizeClass|windowSizeClass" --include=*.kt Kanesumi-sec-a

# 主仓断点（结果：命中 settings 应用侧）
grep -rn "720|with_nav_rail|nav_rail" --include=*.rs shared/kanesumi
Select-String settings/src/kanesumi_window.rs "EXPANDED_BREAKPOINT"

# 主仓渐变/模糊（结果：无渐变原语，只有 with_alpha 纯色）
grep -rn "gradient|Brush|Blur" --include=*.rs shared/kanesumi

# SD.md 渐变（结果：413 行全文 0 命中）
grep -n "渐变|gradient" compositor/docs/SD.md
```
