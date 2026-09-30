# UWP → Kanesumi 反馈表

> **这是本扇区最重要的产出。** 在真 UWP 上做控件会拿到自绘路线拿不到的东西：
> 官方默认值、真实视觉状态机、平台行为的边界情况。这些必须回流，否则本扇区
> 就只是一次性支出。
>
> 规则（参 `AGENTS.md` 铁律 4）：**发现即记录，不要攒着。**
>
> 回流目标（按发现的性质选）：
> - `Kanesumi/CONTROL_SPEC.md` —— 补全「未在快照中」的条目；修正与官方默认不符的读数
> - `Kanesumi/ANIMATION_SPEC.md` —— 动画模型与时序数值
> - `Kanesumi/COMPOSITION.md` —— 布局 / 命中 / 裁剪这类构图契约
> - `Kanesumi/PORT_ROADMAP.md` —— 闭源控件由「逆推」升级为「实测确证」
> - `Kanesumi/CANON_VS_TEMPORARY.md` —— 扇区刻意偏离与待裁定项
> - `Ether/docs/KANESUMI_DESIGN.md` —— 若发现正典未覆盖的规格
>
> **扇区现状（2026-09-30）**：本扇区的实体是 `Ncrust/windows` 的 **Kanesumi.Xaml**
> （UWP + WinUI 2，2026-09-26 ~ 27 起可日常使用，已发 v1.0.0），不再是只取数的验证工程 ——
> 它跑着真实业务、跨控件族都有落地，实证密度高一档。

---

## 记录模板

每条反馈用这个结构。**「差异」一栏是价值所在** —— 一致的地方不必记，
不一致的地方才是自绘路线会做错的地方。

```markdown
### <控件名> · <一句话结论>

- **UWP 实证**：实测到的具体值 / 行为（附观察方式：截图、Inspector、代码位置）
- **现行规格**：`CONTROL_SPEC.md §N` 或 `PORT_ROADMAP.md` 里现在怎么写的
- **差异**：两者不同在哪
- **处置**：改规格 / 改实现 / 标注为刻意偏离（并说明为什么）
- **状态**：⬜ 待确认 · 🔧 待修正 · ✅ 已回流
```

---

## 已确认：圆角全局归零成立

- **UWP 实证**：`ControlCornerRadius` 与 `OverlayCornerRadius` 是全局资源，
  在 `App.xaml` 覆盖为 `0` 后，Button / TextBox / ComboBox / ListView /
  Flyout / ContentDialog 的圆角一并归零，**无需重写任何控件模板**。
  依据：`App.xaml` 的覆盖，以及我方对 `MainPage.xaml` 控件验证台的目视核验。
- **现行规格**：`Kanesumi/CONTROL_SPEC.md` 未记录圆角来源 —— 因为自绘路线
  根本不需要（`SceneCommand::FillRect` 没有 radius 字段）。
- **差异**：**架构层面的差异，不是数值差异。**
  自绘路线是「结构性排除」：命令模型里不存在圆角这个概念。
  WinUI 2 是「覆盖默认值」：框架有能力画圆角，靠资源覆盖关掉。
- **处置**：**不改 Kanesumi 规格**，但应在 `CONTROL_SPEC.md` 补一节说明：
  「WinUI 2 扇区依赖两个全局资源归零；新增控件时须核验其是否经由这两个资源」。
  这是**跨扇区知识**，自绘路线不会遇到。
- **状态**：✅ 已回流（2026-09-30）：`CONTROL_SPEC.md` §11.3 补了「直角的**资源版本**前提」
  一条 —— 归零两个 CornerRadius 之外，还须把 WinUI 2 的资源版本钉在 `Version1`，否则
  `Version2` 的圆角中灰内容面板改不动这两个键。

---

## 2026-09-30 · 本轮回流（扇区实体：`Ncrust/windows` 的 Kanesumi.Xaml）

> 本轮实证来自一个跑着真实业务的 UWP 应用（2026-09-26 ~ 27，已发 v1.0.0）及其
> `windows/docs/KANESUMI_XAML.md` / `windows/AGENTS.md`，不再是验证台上的单点取数。

### 命中测试 · 合成变换不参与命中

- **UWP 实证**：Composition 层的 `Translation` / `Scale` 不参与命中，迷你栏「视觉上移」后仍
  按布局位置接收点击，因此按状态另设了命中门控。`Opacity = 0` 的元素照样接收点击；空白处
  穿透靠「无背景」而非「透明背景」；焦点视觉画在最上层，覆盖层不接管焦点就漏出下层的焦点环。
- **现行规格**：`COMPOSITION.md` 契约 3 只写「一次构图，多方消费」，没有命中与合成变换的分级。
- **差异**：缺一条 —— 变换只改视觉，不改可点区域。
- **处置**：已补 `COMPOSITION.md` 强制契约 12。
- **状态**：✅ 已回流

### 直角 · 资源版本须钉在 `Version1`

- **UWP 实证**：只覆盖 `ControlCornerRadius` / `OverlayCornerRadius` 不够；WinUI 2 的资源版本
  还必须选 `Version1`（Win10 / Groove 一代）。`Version2` 自带 Win11 的圆角与中灰内容面板。
- **现行规格**：`CONTROL_SPEC.md` §11.3 的「圆角」一条只列了两个全局资源。
- **差异**：直角是「三件事」，不是一件。
- **处置**：已补 `CONTROL_SPEC.md` §11.3 一行。
- **状态**：✅ 已回流

### 资源键 · 缺键静默失效

- **UWP 实证**：`{ThemeResource}` 引用不存在的键**不报错**，只静默失效；WinUI 3 的键在
  WinUI 2 里不存在时最容易踩，故新增引用后先跑资源审计（`ResourceAudit.ps1`）。
- **现行规格**：本仓 M1 的「未知令牌必须报错」对标 `XamlResourceReferenceFailed`，但没有写
  平台侧真实的失败方式是**静默**。
- **差异**：同一个失败模式，两边互为对照 —— 记下来才知道那条铁律为什么存在。
- **处置**：已补 `CONTROL_SPEC.md` §11.3 一行。
- **状态**：✅ 已回流

### 动画 · 句柄只建一次，参数进集合

- **UWP 实证**：表达式 / 动画绑定后只启动一次，变化量放进 `CompositionPropertySet`；反例是为
  「更新参数」重新 `StartAnimation` —— 替换那一帧会露出静态初值（播放卡片的封面按原始尺寸
  闪一下）。
- **现行规格**：`ANIMATION_SPEC.md` §Ⅰ 否定了 Storyboard 的播放控制，未写平台侧怎么落地。
- **差异**：缺「句柄与参数的生命周期分离」，而这正是 `set_target` 可中断语义的另一面。
- **处置**：已补 `ANIMATION_SPEC.md` §Ⅰ「平台侧等价物：Composition 参数集」。
- **状态**：✅ 已回流

### 交互 tint · 无依据值已被「规格化」并扩散到第二端

- **UWP 实证**：`Kanesumi.Xaml/Themes/Colors.xaml` 的 `KPressTintColor` = `#22FFFFFF`
  （白 13.3%）/ 亮 `#14000000`，取自 `Ncrust/spec/design/tokens.json` 的 `color.*.pressTint`；
  悬停用同一笔刷 ×0.5，实际约 6.7%。
- **现行规格**：本仓 `MetroIndication` 悬停 9.8%（`ListLow`）、按下 20%（`ListMedium`），各有
  一手源；sec-a 的 `MetroColors.pressTint` = 13.3% 无依据（2026-09-22 已记录，见
  `docs/research/2026-09-22_seca_switch.md`）。
- **差异**：同一语义出现三个取值（≈6.7% / 13.3% / 20%）。无依据的 13.3% 已走完
  「应用值 → 写进 `tokens.json` 升格为规格 → 第二端照规格实现」的三级扩散。
- **处置**：**改在源头** —— `Ncrust/spec/design/tokens.json` 与 sec-a；本仓是一手源侧，不动。
- **状态**：⬜ 待跨仓处理

### Switch · 尺寸与时长的第三组取值

- **UWP 实证**：Windows 端取 52×28 直角、滑块 22、内边距 3、220ms `metroCubic`；关态轨道
  `onSurfaceVariant@0.45` + `divider` 描边、开态 `primary`、滑块恒 `onPrimary`。
- **现行规格**：本仓 40×20 / 滑块 10 / 行程 20（`ToggleSwitch` 一手源）、150ms；sec-a 52×28、220ms。
- **差异**：Windows 与 sec-a 同取大尺寸与 220ms，本仓是三者中唯一的 40×20 —— 待裁定项
  U1/U2 的天平据此更偏 sec-a 一侧。
- **处置**：登记为 U1/U2 的新输入（`SECA_SYNC_2026-09-22.md` §Ⅱ）；不自行改值（属观感变更）。
- **状态**：⬜ 待裁定

### ProgressRing · 时长的第三组取值

- **UWP 实证**：Windows 端自绘 `MetroProgressRing` —— 36 / 描边 3 / 270° 弧 / 圆头端帽 /
  1s 线性一圈；不重写平台 `ProgressRing`（其实现是 Lottie 动画，改不动弧形）。
- **现行规格**：本仓 2.0s / 0→900°（来源不明）；UWP OS 一手值 3.47s / −110°→585° / 6 点 stagger。
- **差异**：第三组取值，且与本仓现值同属「单弧」，结构上比 UWP OS 那组更接近现实现。
- **处置**：登记为 T18 的新输入（`CANON_VS_TEMPORARY.md` §三）；不自行改值。
- **状态**：⬜ 待裁定

### 平台控件优先 · 扇区路由判据

- **UWP 实证**：负责人 2026-09-26 实测自绘 Tab 行体验差，改为「能用平台原生就用原生、只做
  资源键级覆盖」（Pivot / NavigationView / ComboBox…），并据此撤掉了自绘 ListView 侧栏与 Tab 行。
- **现行规格**：本仓 `MetroTabRow`（Pivot 派生）/ `MetroSelectorFlyout`（ComboBox 派生）/
  `MetroNavigationView`（NavigationView 派生）都是自绘。
- **差异**：本仓是运行时，**没有可回归的平台控件**，此条不直接适用；但它给出判据 ——
  「本源就是移植某平台控件」的 Kanesumi 控件，在有该平台控件的平台上应回归原生。
- **处置**：待写进 `PORT_ROADMAP.md` 作为扇区路由判据；措辞需维护者确认后再落地。
- **状态**：⬜ 待登记

### 按钮 · 扇区刻意不用 Kanesumi 按钮外观

- **UWP 实证**：负责人 2026-09-27 决定 Windows 端按钮改用微软原生（主操作
  `AccentButtonStyle`、次要默认 `Button`、图标 `AppBarButton`），`Metro*ButtonStyle` 不再使用。
- **现行规格**：`CONTROL_SPEC.md` §1 的 MetroButton 是本库的按钮规格。
- **差异**：**扇区刻意偏离**，不是读数错误 —— 桌面端以平台一致为先。
- **处置**：登记为刻意偏离（`CANON_VS_TEMPORARY.md` §二），避免半年后被当成笔误改回。
- **状态**：⬜ 待登记

---

## 待记录（arc-deck 时代的验证台清单，仍待 Windows 本机）

- [ ] `Button` / `AccentButtonStyle` —— 圆角、按下反馈（UWP 是位移还是缩放？）
- [ ] `TextBox` —— 圆角、聚焦边框（focus stroke 厚度与颜色）
- [ ] `ToggleButton` / `CheckBox` / `RadioButton` —— 开关几何
- [ ] `ComboBox` —— 下拉浮层圆角（走 Overlay）与选中项背板
- [ ] `Slider` —— 轨道与滑块的圆角（正典：`Capsule` 仅限此类结构件）
- [ ] `ListViewItem` —— **选中背板圆角**（最容易残留的地方）
- [ ] `ProgressBar` —— 轨道与指示条圆角
- [ ] `InfoBar` —— 圆角、强调色用法
- [ ] `Flyout` / `ContentDialog` —— Overlay 圆角是否同样归零
- [ ] `ScrollBar` —— 粗细、圆角

### 需要特别留心的一类：动效时长

正典给出的是**Kanesumi 的取值**，不是 UWP 默认值：

| 项 | Kanesumi（Rust 侧权威） | UWP 默认（待测） |
|---|---|---|
| 标准时长 | `METRO_STANDARD_DURATION = 0.25s` | ? |
| 快速切换 | `QUICK_SWITCH = 0.167s` | ? |
| 开关翻转 | `TOGGLE_FLIP = 0.15s` | ? |
| 默认缓动 | `Quadratic / EaseOut` | ? |

**若 UWP 默认与 Kanesumi 不同，这就是一条高价值反馈** —— 它说明自绘路线
（按正典取值）与真 UWP 会在手感上分叉，需要决定以谁为准。

### 另一类：正典未覆盖的规格

已知一处上游空白：**IME 组合串的视觉规格**（下划线样式、颜色、透明度）。
`IME_WIRING_PLAN.md` 明确写「CONTROL_SPEC 未定 preedit 规格，走平台默认」。
UWP 的 TextBox 有真实实现 —— **实测它，然后把规格补进正典。**
这是本扇区能独立完成、自绘路线做不到的事。
