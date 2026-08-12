# Kanesumi 控件移植路线图

> **目的**：`CONTROL_MATRIX.md` 记录「已实现且带测试」的验收面。本文档记录**全景空缺** —— WinUI 2 / UWP 尚未移植的所有控件、按微软源码开放度分类、按 Ether 应用需求排优先级。
>
> 更新原则：每次新增/完成一个控件，先动本表（挪行 + 改状态），再动 `CONTROL_MATRIX.md`（加验收行）。

---

## §Ⅰ 源码分类

WinUI 2 控件按微软对源码的开放度分两类。移植策略截然不同。

### A. **开源**（`microsoft-ui-xaml`）

Repo：<https://github.com/microsoft/microsoft-ui-xaml>，C++/WinRT。本仓已 shallow-clone 至 `reference/microsoft-ui-xaml/dev/<Control>/`。

**辨认标志**：目录下有真实 `.cpp`（非仅 helper / themeresources.xaml）。

**移植方法**：`<Control>.cpp` + `<Control>.h` + `<Control>_themeresources.xaml` 三件套逐字对读。视觉状态、交互、动画时长、尺寸都能从代码读出，猜测量最少。**先读源码，再写规格，再写 Rust**（用户铁律，参 memory `uwp_reference_first.md`）。

WinUI 2 时代加入 microsoft-ui-xaml 的新控件多属此类：MenuBar / NavigationView / TabView / InfoBar / Expander / …

### B. **闭源**（`Windows.UI.Xaml` 平台内置）

无公开 C++ 源。信息来源只有：

1. **XAML 默认样式** —— Windows SDK `generic.xaml`（视觉状态 / 尺寸 / 缓动时长可读）；
2. **官方文档 + [WinUI 2 Gallery](https://github.com/microsoft/WinUI-Gallery)** —— 视觉表现 + 交互描述；
3. **[ReactOS](https://github.com/reactos/reactos)** —— 部分反向实现（参考价值有限，Metro 时代控件多不在）；
4. **观察 + 逆向** —— 跑 Windows 上的 WinUI 2 Gallery，看视觉、量像素、掐时长。

Kanesumi 现有的 MetroButton / MetroTabRow（Pivot 派生）/ MetroList（ListView 派生）/ MetroDropdownMenu（MenuFlyout 派生）/ MetroDialog（ContentDialog 派生）/ MetroSelectorFlyout（ComboBox 派生）**全部属于闭源类**，规格由 `CONTROL_SPEC.md` 记录（这就是 CONTROL_SPEC 存在的意义）。

**辨认标志**：`reference/microsoft-ui-xaml/dev/` 里**不存在该目录**，或仅有 `*Helper.cpp` + `*_themeresources.xaml`（WinUI 只加 helper，主体仍在 Windows）。

---

## §Ⅱ 优先级定义（P0-P3）

| 级 | 含义 | 应用依赖示例 |
|---|---|---|
| **P0** | 阻塞 Ether daily driver / 常见系统应用无法运行 | Settings 面板无 TextBox 就没法输入 wifi 密码 |
| **P1** | 常见需求，缺则应用能跑但功能残缺 | Librarian 无 SplitView 就没法两栏浏览 |
| **P2** | 完善度 / 复杂应用需要 | 长列表无 Repeater 虚拟化会掉帧 |
| **P3** | 边缘 / 特效 | ParallaxView、Rating、TeachingTip |
| **—** | 明确不移植 / 依赖外部 crate | WebView2、AnimatedVisualPlayer (Lottie)、InkCanvas |

---

## §Ⅲ 已移植（45 个 + 2 部分）

见 `CONTROL_MATRIX.md` 详表。此处仅摘录源类归属：

| Kanesumi 控件 | 派生自 | 源码类 |
|---|---|---|
| MetroButton | Button | 闭源 |
| MetroIconButton | AppBarButton | 闭源 |
| MetroSwitch | ToggleSwitch | 闭源 |
| MetroProgressBar | ProgressBar (WinUI 2) | **开源** |
| MetroProgressRing | ProgressRing (WinUI 2) | **开源** |
| MetroTabRow | Pivot | 闭源 |
| MetroList | ListView | 闭源 |
| MetroDropdownMenu | MenuFlyout | 闭源 |
| MetroSelectorFlyout | ComboBox + ComboBoxHelper | 闭源（Helper 开源） |
| MetroDialog | ContentDialog | 闭源 |
| MetroSurface | Border + 覆盖层 | 闭源（原语） |
| MetroTile | Tile（Metro 时代 StartTile，非 WinUI） | Metro 遗产（无源） |
| MetroText | TextBlock | 闭源 |
| MetroMenuBar | MenuBar (WinUI 2) | **开源** |
| MetroInfoBar | InfoBar (WinUI 2) | **开源** |
| MetroExpander | Expander (WinUI 2) | **开源** |
| MetroInfoBadge | InfoBadge (WinUI 2) | **开源** |
| MetroPipsPager | PipsPager (WinUI 2) | **开源** |
| MetroPersonPicture | PersonPicture (WinUI 2) | **开源** |
| MetroDropDownButton | DropDownButton (WinUI 2) | **开源** |
| MetroBreadcrumbBar | BreadcrumbBar (WinUI 2) | **开源** |
| MetroSplitButton | SplitButton (WinUI 2) | **开源** |
| MetroPagerControl | PagerControl (WinUI 2, NumberPanel) | **开源** |
| MetroRadioButtons | RadioButtons (WinUI 2) | **开源**（单项自绘） |
| MetroTwoPaneView | TwoPaneView (WinUI 2) | **开源**（纯布局） |
| MetroTitleBar | TitleBar (WinUI 2) | **开源** |
| MetroRatingControl | RatingControl (WinUI 2) | **开源** |
| MetroTabView | TabView (WinUI 2) | **开源**（拖拽 Reorder 略） |
| MetroTeachingTip | TeachingTip (WinUI 2) | **开源** |
| MetroTreeView | TreeView (WinUI 2) | **开源** |
| MetroNavigationView | NavigationView (WinUI 2) | **开源**（子项级联/flyout 略） |
| MetroColorPicker | ColorPicker (WinUI 2) | **开源**（Spectrum 阶梯近似） |
| MetroParallaxView | ParallaxView (WinUI 2) | **开源**（纯位移辅助） |
| MetroAnimatedIcon | AnimatedIcon (WinUI 2) | **开源**（几何 chevron 插值） |
| MetroSwipeControl | SwipeControl (WinUI 2) | **开源**（Reveal 模式） |
| MetroGrid | Grid | 闭源（布局原语，structure） |
| MetroTextBox | TextBox | 闭源 |
| MetroPasswordBox | PasswordBox | 闭源（TextBox 掩码变体） |
| MetroCheckBox | CheckBox | 闭源 |
| MetroNumberBox | NumberBox (WinUI 2) | **开源** |
| MetroAutoSuggestBox | AutoSuggestBox (WinUI 2) | **开源**（主体 TextBox 闭源） |
| MetroDropdownMenu 级联 | MenuFlyoutSubItem | 闭源（悬停展开二级） |
| RadioMenuFlyoutItem | RadioMenuFlyoutItem (WinUI 2) | **开源**（单选组，级联容器内） |
| MetroCommandBarFlyout | CommandBarFlyout (WinUI 2) | **开源**（TextCommandBarFlyout 语义） |
| MetroRepeater | Repeater / ItemsRepeater (WinUI 2) | **开源**（虚拟化布局核心：visible_range/item_rect） |
| MetroScrollView | ScrollView / ScrollPresenter (WinUI 2) | **开源**（offset 夹紧 / 滚动条几何 / 平滑） |

**已移植 45 个（开源 29 个 + 3 部分能力）** —— §Ⅳ **可移植开源控件已全部清空**。
Repeater / ScrollView 虚拟化引擎落地并驱动 MetroList（只渲染可见行）。
剩余仅 AnimatedVisualPlayer / WebView2（外部 runtime，明确不移植）。

### 已移植但部分能力未完（Phase 3 续做）

| 控件 | 未完成 |
|---|---|
| MetroMenuBar | 键盘遍历（Alt / Arrow / Enter / Esc） |
| MetroTabRow | 页体切换动画（只做 header + pipe，UWP Pivot 还有页面 slide） |
| MetroSelectorFlyout | 分组 / 图标项 / 自定义 template |
| MetroTextBox | IME 组成未接（Phase 2-1 / Ceyboard）；Ctrl+Z 撤销钩子待宿主键盘 |
| MetroNumberBox | Popup 模式（首期仅 Compact）、按住重复步进 |
| MetroDropdownMenu | 三级以上级联（现支持二级）、键盘遍历 |
| MetroRepeater | 回收复用（Kanesumi 无保留视觉树，只做可见性虚拟化） |
| MetroScrollView | 惯性/Chaining/Railing/缩放（Kanesumi 弹簧平滑等价惯性） |

---

## §Ⅳ 待移植 —— **开源**（`microsoft-ui-xaml`，可直接读 .cpp）

按微软 dev/ 目录字母序，标 P0-P3。行数越多、组合越复杂。

| 控件 | dev/ 有 .cpp 数 | Ether 用途 | 优先级 | 备注 |
|---|---|---:|---:|---|
| ~~**Repeater**~~ | 59 | 虚拟化布局引擎 | ✅ | **2026-08-12 已移植**（visible_range/item_rect，驱动 MetroList） |
| ~~**ScrollView / ScrollPresenter**~~ | 5 + 31 | 滚动容器 | ✅ | **2026-08-12 已移植**（offset 夹紧 / 滚动条 / 平滑） |
| ~~**RadioMenuFlyoutItem**~~ | 1 | 菜单内单选 | ✅ | **2026-08-12 已移植**（MenuFlyout 级联完成后） |
| ~~**CommandBarFlyout**~~ | 5 | 选中文本浮出 | ✅ | **2026-08-12 已移植**（TextBox 选区 + 级联就绪） |
| ~~**NumberBox**~~ | 3 | Settings 数字项 | ✅ | **2026-08-12 已移植**（依赖 TextBox 已解决） |
| ~~**AutoSuggestBox**~~ | 1 (Helper) | 搜索框建议下拉 | ✅ | **2026-08-12 已移植**（TextBox 已解决） |
| **AnimatedVisualPlayer** | 2 | Lottie 播放 | **—** | 依赖 Lottie runtime，暂不引入 |
| **WebView2** | 2 | Web 嵌入 | **—** | 依赖 Chromium runtime，另立方案 |

**小计**：可移植开源控件已全部完成。剩余 2 项均依赖外部 runtime（Lottie / Chromium），
不移植（标注「—」）。虚拟化引擎（Repeater + ScrollView）已落地并驱动 MetroList。

---

## §Ⅴ 待移植 —— **闭源**（`Windows.UI.Xaml`，靠 spec + Gallery 逆推）

`reference/` 里无源码目录（或仅 XAML themeresources）的控件。Kanesumi 已移植的 15 个闭源控件是**同套方法**：读 `CONTROL_SPEC.md` 对应节 + 跑 WinUI 2 Gallery 观察 + 猜测数值。移植时同时**新增** `CONTROL_SPEC.md §N` 一节记录规格。

按类别聚合。

### V.1 基础输入（P0-P1）

**TextBox / PasswordBox / CheckBox 已落地（2026-08-12）—— Ether 首个文本输入通路打通，
settings/ceyboard 不再被输入控件阻塞（IME 接入仍待 Phase 2-1）。**

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|---|:---:|---|
| ~~**TextBox**~~ | Settings wifi 密码、Librarian 文件名、搜索框 | ✅ | **已移植**（`text_field.rs` + `text_box.rs`） |
| ~~**PasswordBox**~~ | wifi / 锁屏密码 | ✅ | **已移植**（`password_box.rs`） |
| ~~**CheckBox**~~ | Settings 开关组（三态） | ✅ | **已移植**（`check_box.rs`） |
| **RadioButton**（单个） | 组内单选项 | **P1** | 组容器 RadioButtons 开源，单项自身闭源 |
| **Slider** | 音量 / 亮度 / 色温 | **P1** | 连续数值输入；`MetroSwitch` 是布尔离散 |
| **HyperlinkButton** | Settings 里指向外部 URL / 文档链接 | **P1** | Button 变体（下划线 + accent 前景） |
| **RichEditBox** | Notes 应用富文本 | **P3** | TextBox 高级形态 |
| **TextBlock**（多样式 span） | 简单 MetroText 已覆盖单样式；混排未做 | **P2** | Run / Bold / Italic / Hyperlink inline |

### V.2 基础容器 / 布局（P0-P1）

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|---|:---:|---|
| ~~**Grid**~~ | 通用二维布局 | ✅ | **已移植**（`kanesumi-structure::MetroGrid`，Fixed/Auto/Star + span） |
| **StackPanel** | 一维排列 | ✅≈ | `kanesumi-structure::Ui::LayoutDirection` 已覆盖 |
| **Border** | 单独描边 / 圆角容器 | **P1** | Scene FillRect+StrokeRect 组合可替；显式控件更清晰 |
| **Canvas** | 绝对定位 | **P2** | 少用，MetroTile 磁贴墙内部已自绘 |
| **Viewbox** | 按内容缩放 | **P3** | 用于 icon 拉伸；Scene::image 目前直接指定目标 rect |
| **RelativePanel** | 约束相对布局 | **P3** | 少用 |
| **SplitView** | 侧栏容器（Pane + Content，Overlay / Inline / CompactOverlay 模式） | **P1** | Settings / Librarian 两栏布局。仅有 XAML themeresources，全靠逆推 |
| **ContentPresenter / ContentControl** | 组合原语（Dialog/Button 内部装任意子树） | **P2** | 需要 Scene 层"槽位"机制 |
| **ItemsControl / ItemsPresenter** | Repeater 依赖 | **P1** | 与 Repeater 一同做 |

### V.3 图形 / 图标

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **Image** | 应用内图片显示 | **P1** | 有 `Scene::image` + rasterize_svg 底子，缺 `MetroImage` 顶层控件（stretch/fallback/loading） |
| **SymbolIcon / PathIcon / FontIcon / BitmapIcon** | 图标源变体 | **P2** | Kanesumi 已选思源黑体 + SVG（不假设 Segoe MDL2 存在，参 V7），Font/Symbol Icon 可略；PathIcon（Canvas 自绘路径）可能有用 |

### V.4 命令栏

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **AppBar / CommandBar** | 顶部/底部固定命令栏 | **P2** | 与 MetroMenuBar 定位不同：MenuBar 是纵向下拉，CommandBar 是横排图标按钮 |
| **AppBarButton** | 与 MetroIconButton ≈ | ✅≈ | 已通过 MetroIconButton 覆盖大部分 |
| **AppBarToggleButton** | 状态化 AppBarButton | **P2** | IconButton + Toggle |
| **AppBarSeparator** | 分隔线 | **P3** | 单像素竖线，一行代码 |

### V.5 弹层 / 菜单基元

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **Popup** | 通用弹层基元 | ✅≈ | `crate::popup::PopupAnim` 已实现动画层 |
| **FlyoutPresenter** | 通用 flyout 内容承载 | **P2** | 目前每种 flyout 自绘 |
| **MenuFlyoutItem / MenuFlyoutSubItem / MenuFlyoutSeparator** | 菜单内项 | 🔶 | MenuItem 已覆盖 Item/Separator；SubItem 未做（menuflyout 级联） |
| **ToolTip** | 悬停提示气泡 | **P2** | hover 200-500ms 触发；Ether 至今无 tooltip |

### V.6 集合

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **GridView** | 网格排列（MetroTile 磁贴墙已覆盖桌面场景） | **P2** | 图片浏览类应用可能需要 |
| **ListViewItem** | List 单行 template | 🔶 | MetroList 已内置单一样式；ListViewItem 独立控件 = 可复用条目 |
| **SemanticZoom** | Zoom in/out 两级视图（联系人 A-Z 缩放） | **P3** | |
| **CalendarView / CalendarDatePicker / DatePicker / TimePicker** | 日历/时间输入 | **P2** | Settings 时间设置、Librarian 日期筛选 |

### V.7 内联文本

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **Hyperlink**（Run 内嵌） | 富文本内链接 | **P3** | RichTextBlock 依赖 |
| **RichTextBlock** | 段落式富文本 | **P3** | Notes 应用 |

### V.8 页面导航

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **Frame / Page** | 页栈 + 过渡动画 | 🔶 | `kanesumi-structure::Navigation` 已覆盖状态机 + transition_progress，缺页面 slide-in 动画 |

### V.9 墨迹（跳过）

| 控件 | 状态 | 备注 |
|---|:---:|---|
| InkCanvas / InkToolbar | **—** | 触屏笔迹，Ether 桌面向，暂不做 |
| MediaElement / MediaPlayerElement | **—** | 视频播放需 media backend，另立方案 |

---

## §Ⅵ 优先级摘要（"接下来做什么"）

排完 P0 之后依次推 P1。**P0 已于 2026-08-12 清空（TextBox / PasswordBox / CheckBox / Grid
落地），Ceyboard / Settings 的输入通路不再被控件阻塞；IME 接入仍待 Phase 2-1。**

### P0（已清空 ✅）

1. ~~**TextBox**~~ ✅ 已移植（`text_field.rs` + `text_box.rs`，光标/选区/撤销/掩码）
2. ~~**PasswordBox**~~ ✅ 已移植（`password_box.rs`）
3. ~~**CheckBox**~~ ✅ 已移植（`check_box.rs`，三态）
4. ~~**Grid**~~ ✅ 已移植（`kanesumi-structure::MetroGrid`）

### P1（Ether 常见应用需求）

5. **NavigationView** —— Settings 左侧栏（已实现，待接 Settings）
6. **SplitView** —— Librarian 双栏
7. **Slider** —— Settings 数值调节
8. **HyperlinkButton** —— 关于/许可页
9. **Border** —— 通用装饰
10. **RadioButton + RadioButtons** —— 单选组（组已实现，单项自绘）
11. **Image** —— 应用内图片
12. ~~**Repeater + ScrollPresenter + ScrollView**~~ ✅ 2026-08-12（虚拟化引擎，驱动 MetroList）
13. ~~**InfoBar / Expander / Breadcrumb**~~ ✅；~~**NumberBox**~~ ✅ 2026-08-12
14. ~~**AutoSuggestBox**~~ ✅ 2026-08-12
15. ~~**CommandBarFlyout**~~ ✅ 2026-08-12（TextBox 选区 + MenuFlyout 级联就绪）
16. ~~**MenuFlyoutSubItem + RadioMenuFlyoutItem**~~ ✅ 2026-08-12（级联落地）
17. **MenuBar 键盘遍历** —— 补齐 MenuBar 键盘（Alt / Arrow / Enter / Esc）
18. **Frame / Page transition 动画** —— 已有 Navigation 状态机，接页面 slide 动画即可

> 2026-08-12 批次三：Repeater 虚拟化引擎 + ScrollView 滚动容器。
> 参 CONTROL_SPEC §41–§42。Gallery Input 页新增 1000 项虚拟化长列表演示。

### P2-P3

见 §Ⅳ / §Ⅴ 各表末位。

---

## §Ⅶ 移植工作流

每移植一个控件：

1. **判类** —— 目录在 `reference/microsoft-ui-xaml/dev/<Name>/` 且有 `.cpp` = 开源；否则闭源。
2. **开源路径**：读 `.cpp` + `.h` + `_themeresources.xaml`，直接抄尺寸/状态/动画。
3. **闭源路径**：`CONTROL_SPEC.md` 新增一节，写清尺寸/视觉状态/动画时长/缓动。数据来源：Windows SDK generic.xaml + WinUI 2 Gallery 观察。
4. **实现**：`kanesumi-controls/src/<name>.rs`，`render(theme, engine, rect, scene) -> Scene`；输入接口 `press/release/hover` 返回消费/命中信息（参考 MetroMenuBar / MetroDropdownMenu 模式）。
5. **测试**：至少覆盖布局命中 / 状态切换 / 渲染命令数 / 关键动画数值。
6. **落库**：`lib.rs` 导出 + `CONTROL_MATRIX.md` 加验收行 + 本表 §Ⅲ 加行 + 从 §Ⅳ/Ⅴ 删行。

用户铁律：**UWP 派生控件先拉 reference**（memory `uwp_reference_first.md`）—— 猜测过的都返工。
