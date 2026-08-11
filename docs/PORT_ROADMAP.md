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

## §Ⅲ 已移植（22 个 + 2 部分）

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

**已移植开源占比 13/22 ≈ 59%** —— 首批（Metro 时代基础控件）多为闭源平台层；第二批按 §Ⅳ
自足小型控件推进，开源占比显著提升。PagerControl 仅 NumberPanel 模式；RadioButtons 单项
自绘（闭源 RadioButton 基元由容器内实现）。

### 已移植但部分能力未完（Phase 3 续做）

| 控件 | 未完成 |
|---|---|
| MetroMenuBar | 键盘遍历（Alt / Arrow / Enter / Esc） |
| MetroDropdownMenu | 二级级联（`MenuItem.submenu` 字段已在，render/命中未消费） |
| MetroTabRow | 页体切换动画（只做 header + pipe，UWP Pivot 还有页面 slide） |
| MetroList | 虚拟化（长列表全量渲染，长表掉帧） |
| MetroSelectorFlyout | 分组 / 图标项 / 自定义 template |

---

## §Ⅳ 待移植 —— **开源**（`microsoft-ui-xaml`，可直接读 .cpp）

按微软 dev/ 目录字母序，标 P0-P3。行数越多、组合越复杂。

| 控件 | dev/ 有 .cpp 数 | Ether 用途 | 优先级 | 备注 |
|---|---:|---|:---:|---|
| **NavigationView** | 19 | Settings 左侧导航、Librarian 侧栏 | **P1** | 现有 MetroTabRow ≈ NavigationView Top 模式的简化；侧栏 Left 模式 + Header/Pane/Content 分离全都没做。19 个 cpp 是移植量最大的一档 |
| **TabView** | 5 | Ether 浏览器 / 文档编辑类应用（Kanesumi Gallery 页导航不算） | **P1** | Chrome 式标签页：Add/Close/Drag/Reorder。区别于 Pivot（不能关闭、拖拽） |
| **TreeView** | 15 | Librarian 文件夹树、Settings 层级选项 | **P2** | 项展开动画 + 缩进层级 + 键盘 Right/Left 展开 |
| **Repeater** | 59 | List / TabView / TreeView 底层虚拟化 | **P1** | 最大工程量；先做上层控件、共用一份 Repeater |
| **ScrollView / ScrollPresenter** | 5 + 31 | 上述所有滚动容器 | **P1** | Repeater 前置。ScrollPresenter 30 个 cpp 是内核 |
| **NumberBox** | 3 | Settings 数字项（音量、屏幕缩放…） | **P1** | TextBox + 上下步进 + 校验（依赖闭源 TextBox） |
| **RadioMenuFlyoutItem** | 1 | 菜单内单选（View → Zoom Level） | **P2** | 依赖 MenuFlyout 级联完善 |
| **RatingControl** | 5 | Librarian 文件评分（非核心） | **P3** | |
| **ColorPicker** | 8 | Settings 主题色（Ether 暂无用户可选主题） | **P3** | |
| **CommandBarFlyout** | 5 | 选中文本弹出 Cut/Copy/Paste 浮出 | **P2** | 需 TextBox / 选区 |
| **AutoSuggestBox Helper** | 1 (Helper) | 搜索框建议下拉 | **P1** | 主体 TextBox 闭源；Helper 只处理键盘导航 |
| **TeachingTip** | 6 | 新特性引导气泡 | **P3** | |
| **TitleBar** | 3 | 应用 SSD 标题 | **P1** | 直接影响 Ether 窗口装饰观感（Issue #8 已列 known） |
| **SwipeControl** | 7 | 触屏滑动手势项 | **P3** | Kanesumi 桌面为主，触屏优先低 |
| **TwoPaneView** | 4 | 双面板自适应（宽屏并排 / 窄屏堆叠） | **P2** | 手机/折叠屏取向；Ether 桌面暂无强需 |
| **AnimatedIcon** | 2 | 图标微动画 | **P3** | |
| **AnimatedVisualPlayer** | 2 | Lottie 播放 | **—** | 依赖 Lottie runtime，暂不引入 |
| **ParallaxView** | 2 | 视差滚动 | **P3** | |
| **WebView2** | 2 | Web 嵌入 | **—** | 依赖 Chromium runtime，另立方案 |

**小计**：开源可移植 ~18 个，工程量最大三档：Repeater(59) / ScrollPresenter(31) / NavigationView(19)。

---

## §Ⅴ 待移植 —— **闭源**（`Windows.UI.Xaml`，靠 spec + Gallery 逆推）

`reference/` 里无源码目录（或仅 XAML themeresources）的控件。Kanesumi 已移植的 11 个闭源控件是**同套方法**：读 `CONTROL_SPEC.md` 对应节 + 跑 WinUI 2 Gallery 观察 + 猜测数值。移植时同时**新增** `CONTROL_SPEC.md §N` 一节记录规格。

按类别聚合。

### V.1 基础输入（P0-P1）

**最缺的一批。Ether 目前没有任何文本输入控件，settings/ceyboard 无法上线。**

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **TextBox** | Settings wifi 密码、Librarian 文件名、搜索框 | **P0** | 光标 / 选区 / IME / 撤销 —— 依赖 harness 键盘输入（P2-1 待做） |
| **PasswordBox** | wifi / 锁屏密码 | **P0** | TextBox 变体（显式 masked） |
| **CheckBox** | Settings 开关组（三态：Checked / Unchecked / Indeterminate） | **P0** | 与 Switch 语义不同：CheckBox = 多选一组内独立项，Switch = 布尔即时应用 |
| **RadioButton**（单个） | 组内单选项 | **P1** | 组容器 RadioButtons 开源，单项自身闭源 |
| **Slider** | 音量 / 亮度 / 色温 | **P1** | 连续数值输入；`MetroSwitch` 是布尔离散 |
| **HyperlinkButton** | Settings 里指向外部 URL / 文档链接 | **P1** | Button 变体（下划线 + accent 前景） |
| **RichEditBox** | Notes 应用富文本 | **P3** | TextBox 高级形态 |
| **TextBlock**（多样式 span） | 简单 MetroText 已覆盖单样式；混排未做 | **P2** | Run / Bold / Italic / Hyperlink inline |

### V.2 基础容器 / 布局（P0-P1）

| 控件 | Ether 用途 | 优先级 | 备注 |
|---|---|:---:|---|
| **Grid** | 通用二维布局 | **P0** | `kanesumi-structure::Ui` 有 Row/Column（一维），二维 Grid 未做 |
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

排完 P0 之后依次推 P1。**P0 必须先于 Ceyboard / Settings 真正上线**。

### P0（当前阻塞项，全属闭源基础输入 + 布局）

1. **TextBox** —— 无它 Settings/Ceyboard 都进不了下一步；依赖 harness `KeyPressed` 事件已到位，缺光标/选区/IME 接口
2. **PasswordBox** —— TextBox 派生
3. **CheckBox** —— Settings 组件标配
4. **Grid** —— 二维布局，Settings 面板必需

### P1（Ether 常见应用需求）

5. **NavigationView** —— Settings 左侧栏
6. **SplitView** —— Librarian 双栏
7. **Slider** —— Settings 数值调节
8. **HyperlinkButton** —— 关于/许可页
9. **Border** —— 通用装饰
10. **RadioButton + RadioButtons** —— 单选组
11. **Image** —— 应用内图片
12. **TitleBar** —— 应用窗口 SSD 标题（Ether Issue #8）
13. **Repeater + ScrollPresenter + ScrollView** —— List 虚拟化前置
14. ~~**InfoBar / Expander / Breadcrumb**~~ ✅ 已完成；**NumberBox** —— 开源可读、小工程量
15. **AutoSuggestBox（Helper 开源，TextBox 依赖 P0-1）** —— 搜索框
16. **NumberBox** —— 数字步进
17. **MenuBar 键盘遍历 + MenuFlyoutSubItem** —— 补齐 MenuBar 二级级联与键盘（本次已埋钩子）
18. **Frame / Page transition 动画** —— 已有 Navigation 状态机，接页面 slide 动画即可

> 本批已完成的自足小型控件（§Ⅳ 首批）：InfoBar / Expander / InfoBadge / PipsPager /
> PersonPicture / DropDownButton（P1–P2，2026-08-12 移植，参 CONTROL_SPEC §12–§17）。

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
