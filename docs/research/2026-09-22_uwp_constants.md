# UWP / Metro 控件的尺寸类常量 —— 权威取数全表

> 本文件是**只读取数产物**，不是设计文档。所有条目逐条来自 Windows SDK 自带的 UWP OS 主题字典，
> 数值保持原文（未做单位换算）。Rust 侧（`kanesumi-controls` / `kanesumi-core`）改任何尺寸前先查这里。

| 项 | 值 |
|---|---|
| 源 1 | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml`（12 481 026 字节 / 13 125 行） |
| 源 2 | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\generic.xaml`（2 653 934 字节 / 31 190 行） |
| 抽取日期 | 2026-09-22 |
| 抽取方式 | PowerShell 5.1 正则全量扫描（见 §6，可复现） |
| 行号约定 | 全部为 1-based 行号，指向上表两个文件 |

### 文件内区段（`themeresources.xaml`）

| 行号区间 | 区段 | 说明 |
|---|---|---|
| L1–L3 | `<ResourceDictionary.ThemeDictionaries>` 开始 | — |
| **L4–L1961** | `<ResourceDictionary x:Key="Default">` | 暗色字典 |
| **L1962–L3919** | `<ResourceDictionary x:Key="HighContrast">` | 高对比字典 |
| **L3920–L5878** | `<ResourceDictionary x:Key="Light">` | 亮色字典 |
| **L5879–L13125** | 主题字典之外 | **尺寸常量主体**（Style / ControlTemplate / 非主题资源） |

> 三份主题字典里同名尺寸键**绝大多数取值完全相同**（尺寸不随明暗变化）；只有 13 个 `x:Double` 键
> 与 14 个 `Thickness` 键三态不同，已在 §2.0 / §3.0 单独列出。**高对比态的差异几乎全部是"补 1px/2px 描边"**。

## §1 摘要

### 1.1 总条目数

| 类别 | 源 | 出现条目数 | 唯一键数 | 本文件所在节 |
|---|---|---|---|---|
| `x:Double` | themeresources.xaml | 408 | 162 | §2 |
| `Thickness` | themeresources.xaml | 421 | 175 | §3 |
| `CornerRadius` | themeresources.xaml | 3 | 1 | §4.1 |
| `GridLength` | themeresources.xaml | 3 | 1 | §4.2 |
| `x:Int32` | themeresources.xaml | 5 | 3 | §4.3 |
| `x:Boolean` | themeresources.xaml | 17 | 9 | §4.4 |
| **themresources 数值类小计** | themeresources.xaml | **857** | **351** | §2–§4 |
| `Setter` 几何键（全部，含 ThemeResource 引用） | generic.xaml | 513 | — | §5.1 §5.2 §5.5 |
| ├ 其中**写死数值** | generic.xaml | **346** | — | §5.1 §5.2 |
| └ 其中引用 ThemeResource / StaticResource | generic.xaml | 167 | — | §5.5 |
| 元素属性几何键（写死数值） | generic.xaml | 458 | — | §5.3 |
| `Setter` 几何键（写死数值） | themeresources.xaml | 123 | — | §5.4 |
| 元素属性几何键（写死数值） | themeresources.xaml | 161 | — | §5.4 |
| `*ThemeAnimation` 元素 | generic.xaml 168 + themeresources.xaml 113 | 281 | 10 种 | §4.5 |

### 1.2 按前缀分组的条目数

> 分组规则：取键名的最长匹配前缀（§6 给出完整前缀表）。`†` 标记表示该前缀由「首段驼峰词」兜底推得，非人工确认的族名。
> 「缺」= 该族在本 SDK 快照里 **0 条**。

| 前缀组 | `x:Double` 键数 | `Thickness` 键数 | 小计 |
|---|---|---|---|
| `AppBar*` | 7 | 21 | 28 |
| `AutoSuggestBox*` | 2 | 2 | 4 |
| `AutoSuggestList*` | 2 | 4 | 6 |
| `Button*` | 0 | 3 | 3 |
| `CalendarDatePicker*` | 1 | 3 | 4 |
| `CheckBox*` | 2 | 0 | 2 |
| `ComboBox*` | 5 | 15 | 20 |
| `CommandBar*` | 4 | 11 | 15 |
| `ContentControl*` | 1 | 0 | 1 |
| `ContentDialog*` | 9 | 8 | 17 |
| `Control*` | 1 | 0 | 1 |
| `DatePicker*` | 9 | 5 | 14 |
| `DateTimeFlyout*` | 0 | 6 | 6 |
| `FlipView*` | 0 | 1 | 1 |
| `Flyout*` | 5 | 4 | 9 |
| `GridView*` | 6 | 3 | 9 |
| `HandwritingView*` | 0 | 1 | 1 |
| `HelperButton*` | 0 | 1 | 1 |
| `Hub*` | 3 | 2 | 5 |
| `HyperlinkButton*` | 0 | 2 | 2 |
| `InfoBar*` | 0 | 0 | 0 **缺（0 条）** |
| `InkToolbar*` | 1 | 1 | 2 |
| `KeyTip*` | 1 | 2 | 3 |
| `LanguageSwitcher*` | 0 | 3 | 3 |
| `ListBox*` | 0 | 2 | 2 |
| `ListPickerFlyout*` | 1 | 2 | 3 |
| `ListView*` | 12 | 2 | 14 |
| `MediaTransportControls*` | 0 | 1 | 1 |
| `MenuBar*` | 1 | 1 | 2 |
| `MenuFlyout*` | 2 | 12 | 14 |
| `MTC*` | 15 | 0 | 15 |
| `NavigationBackButton*` | 2 | 0 | 2 |
| `NavigationView*` | 2 | 5 | 7 |
| `PaneToggleButton*` | 3 | 0 | 3 |
| `PasswordBox*` | 1 | 2 | 3 |
| `PersonPicture*` | 4 | 0 | 4 |
| `PickerFlyout*` | 0 | 3 | 3 |
| `Pivot*` | 3 | 6 | 9 |
| `ProgressBar*` | 2 | 1 | 3 |
| `RadioButton*` | 1 | 0 | 1 |
| `RepeatButton*` | 0 | 2 | 2 |
| `RichEditBox*` | 1 | 2 | 3 |
| `ScrollBar*` | 3 | 1 | 4 |
| `SearchBox*` | 6 | 7 | 13 |
| `SemanticZoom*` | 1 | 0 | 1 |
| `SettingsFlyout*` | 1 | 0 | 1 |
| `Slider*` | 7 | 4 | 11 |
| `SmallScrollThumb*` | 2 | 0 | 2 |
| `SplitButton*` | 2 | 1 | 3 |
| `SplitView*` | 2 | 2 | 4 |
| `TabView*` | 0 | 0 | 0 **缺（0 条）** |
| `TextBox*` | 1 | 2 | 3 |
| `TextControl*` | 11 | 4 | 15 |
| `TextStyle*` | 2 | 0 | 2 |
| `TimePicker*` | 7 | 6 | 13 |
| `ToggleButton*` | 0 | 2 | 2 |
| `ToggleMenuFlyoutItem*` | 0 | 1 | 1 |
| `ToggleSwitch*` | 5 | 3 | 8 |
| `ToolTip*` | 1 | 2 | 3 |
| `TreeViewItem*` | 2 | 1 | 3 |
| **合计** | **162** | **175** | **337** |

### 1.3 与 `docs/CONTROL_SPEC.md` 的对照（Rust 侧缺失 / 曾标「未在快照」）

在 `CONTROL_SPEC.md` 中检索「未在快照」「待定」「未找到」：

| 检索词 | 命中 |
|---|---|
| `未在快照` | 4 处（L282、L346、L361、L395） |
| `待定` | **0 处** |
| `未找到` | **0 处** |

命中的**条目名**（`CONTROL_SPEC.md` §11.4 表 + §8 L282）与本次取数的裁决：

| # | CONTROL_SPEC 标记的条目 | 位置 | 本次一手源裁决 |
|---|---|---|---|
| 1 | `PopupThemeAnimation`（MenuFlyout 弹出动画，标注 ~200ms） | L282 | **仍未取到**：`PopupThemeAnimation` 在 `generic.xaml` 与 `themeresources.xaml` 中**各 0 次出现**（§4.5）。该动画不在 OS 主题字典里，属编译期/运行期内置，XAML 侧无法读出。 |
| 2 | `SystemControl*` 笔刷具体色值 | §11.4 | 本次只取几何类，不含色值；但**尺寸侧的 `SystemControl*` 键在本快照中 0 条**——该前缀族不是 OS 尺寸资源命名（见 1.4）。 |
| 3 | `PointerDown/UpThemeAnimation` 时长 / 像素 | §11.4 | **确证不可读**：`generic.xaml` 38×`PointerDown` + 78×`PointerUp`，`themeresources.xaml` 26×`PointerDown` + 53×`PointerUp`，**全部只带 `TargetName`/`Storyboard.TargetName`，无任何偏移/时长/像素属性**（§4.5.2）。CONTROL_SPEC「OS 预置、XAML 读不到」的判断**成立**。 |
| 4 | `SplitOpen/CloseThemeAnimation` | §11.4 | **确证不可读**：各 1 次（`generic.xaml` L10293 / L10302，ComboBox `DropDownStates`），属性只有 `OpenedTargetName`/`ClosedTargetName`/`OffsetFromCenter`/`OpenedLength`，**后两者全是 `{Binding …TemplateSettings.DropDownOffset / DropDownOpenedHeight}`**，无字面数值（§4.5.3）。~333ms 无法从此文件读出。 |
| 5 | `PivotPanel` 头面板平移（+40px / 0.33s） | §11.4 | **仍未取到**：`generic.xaml` 无 `PivotPanel` 模板，无平移数值；`PivotHeaderItemLockedTranslation` = **40**（§2，themeresources.xaml:L69 / L2797 / L3985）是唯一与「+40px」同量级的字面值。 |
| 6 | `ListViewItemPresenter` 原生选中绘制 | §11.4 | 本快照**有**模板（`ListViewItem` / `GridViewItem`），几何定值见 §5；但**选中态由原生 `ListViewItemPresenter` 绘制**，模板里只有 `MultiSelectSquare` 的淡入淡出，无选中矩形几何 → 与 CONTROL_SPEC 一致。 |
| 7 | `ComboBox.cpp` 开合定位 | §11.4 | 尺寸侧可取：`ComboBoxPopupThemeMinWidth` = 80 / `ComboBoxPopupThemeTouchMinWidth` = 240 / `ComboBoxPopupMaxNumberOfItems` = 15 / `…ThatCanBeShownOnOneSide` = 7（§2、§4.3）。 |
| 8 | `ContentDialog.cpp` Esc / 遮罩语义 | §11.4 | 尺寸侧全部可取：Min/Max 宽 **320 / 548**、Min/Max 高 **184 / 756**、按钮 **32** 高 / **130** 最小宽 / **202** 最大宽、标题 MaxHeight **56**（§2）。 |
| 9 | MenuFlyout 弹出动画 | §11.4 | 同 #1。几何侧可取：`MenuFlyoutThemeMinHeight` = 32、`MenuFlyoutSeparatorThemeHeight` = 1、`MenuFlyoutItemThemePadding` = 11,9,11,10（§2、§3）。 |

**本快照整族为 0、Rust 侧需要外部来源的组**：

| 前缀组 | 状态 | 结论 |
|---|---|---|
| `TabView*` | 0 条 | `TabView` 是 **WinUI 2（`muxc:`）新增控件**，不在 UWP OS 主题字典中。全库检索：`generic.xaml` 0 次、`themeresources.xaml` 0 次。Rust 侧须查 `microsoft-ui-xaml` 的 `TabView` 资源字典。 |
| `InfoBar*` | 0 条 | 同上（WinUI 2 新增）。两文件各 0 次。 |
| `Expander*` / `PipsPager*` / `NumberBox*` / `BreadcrumbBar*` / `TeachingTip*` / `AnimatedIcon*` | 0 条 | WinUI 2 新增控件族，两文件各 0 次（本表未单列，仅此备注）。 |
| `DropDownButton*` | 1 条 | 仅 `generic.xaml` L21938 / L21939 两处 `BorderThickness`/`Padding` Setter，**无专属尺寸资源键**。 |

**在快照中、但 CONTROL_SPEC 未登记的组**（本次新发现，Rust 侧若已有实现需回填）：`MTC*`（MediaTransportControls，15 条 `x:Double`）、
`InkToolbar*`、`PersonPicture*`、`RefreshVisualizer*`、`RatingControl`、`MenuBar*`、`LanguageSwitcher*`、`SplitButton*`、`TextStyle*`（字号阶梯）、`SearchBox*`（13 条）。
详见 §1.2 全表。

### 1.4 本次取数附带确证的三条硬结论

1. **`ControlCornerRadius` / `OverlayCornerRadius` 不在 OS SDK 快照里**。两文件检索均 **0 次**。
   `CONTROL_SPEC.md` §11.3 记录的「4px / 8px 两个全局资源」来源**不是这份 UWP OS 字典**，
   而是 WinUI 2 资源字典 —— 引用时不要写成「UWP OS 主题字典」出处。
2. **快照里唯一的 `CornerRadius` 资源只有 1 个**：`HyperlinkFocusRectCornerRadius` = `4,4,4,4`（三态各一份，共 3 处）。
   其余圆角在模板里一律走 `{TemplateBinding CornerRadius}`，即**由控件自身属性决定，没有主题字典默认值**。
3. **尺寸类常量确实集中在文件末尾的非主题区**：`x:Double` 有 42 条出现在 L5879 之后（含 2 条与主题区重复定义），
   `Thickness` 有 52 条全部只在 L5879 之后。而三份主题字典中的 122/123 条是**历史遗留的副本**。

## §2 `x:Double` 全表（按前缀分组）

**行号列读法**：`L16(D)` = Default 暗色字典；`L2743(HC)` = HighContrast；`L3932(L)` = Light；`L6063(N)` = 非主题区。
同一行号列表里的键，三态取值**完全相同**（该键不随主题变化），因此只列一次值。
取值三态不同的 13 个键在分组表里写作 `暗=x / HC=y / 亮=z`，并在 §2.0 展开逐态行。

### 2.1 `AppBar*` —— 7 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AppBarButtonContentHeight` | 16 | themeresources.xaml:L6064(N) | `<x:Double x:Key="AppBarButtonContentHeight">16</x:Double>` |
| `AppBarExpandButtonCircleDiameter` | 3 | themeresources.xaml:L19(D), L2746(HC), L3935(L) | `<x:Double x:Key="AppBarExpandButtonCircleDiameter">3</x:Double>` |
| `AppBarExpandButtonThemeHeight` | 24 | themeresources.xaml:L14(D), L2741(HC), L3930(L) | `<x:Double x:Key="AppBarExpandButtonThemeHeight">24</x:Double>` |
| `AppBarExpandButtonThemeWidth` | 48 | themeresources.xaml:L15(D), L2742(HC), L3931(L) | `<x:Double x:Key="AppBarExpandButtonThemeWidth">48</x:Double>` |
| `AppBarThemeCompactHeight` | 40 | themeresources.xaml:L18(D), L2745(HC), L3934(L), L6073(N) | `<x:Double x:Key="AppBarThemeCompactHeight">40</x:Double>` |
| `AppBarThemeMinHeight` | 56 | themeresources.xaml:L16(D), L2743(HC), L3932(L), L6063(N) | `<x:Double x:Key="AppBarThemeMinHeight">56</x:Double>` |
| `AppBarThemeMinimalHeight` | 24 | themeresources.xaml:L17(D), L2744(HC), L3933(L) | `<x:Double x:Key="AppBarThemeMinimalHeight">24</x:Double>` |

### 2.2 `AutoSuggestBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AutoSuggestBoxIconFontSize` | 12 | themeresources.xaml:L1901(D), L3859(HC), L5817(L) | `<x:Double x:Key="AutoSuggestBoxIconFontSize">12</x:Double>` |
| `AutoSuggestBoxLeftHeaderMaxWidth` | 296 | themeresources.xaml:L6092(N) | `<x:Double x:Key="AutoSuggestBoxLeftHeaderMaxWidth">296</x:Double>` |

### 2.3 `AutoSuggestList*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AutoSuggestListBorderOpacity` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L21(D), L2748(HC), L3937(L) | `<x:Double x:Key="AutoSuggestListBorderOpacity">0</x:Double>` |
| `AutoSuggestListMaxHeight` | 374 | themeresources.xaml:L20(D), L2747(HC), L3936(L) | `<x:Double x:Key="AutoSuggestListMaxHeight">374</x:Double>` |

### 2.4 `CalendarDatePicker*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CalendarDatePickerLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5920(N) | `<x:Double x:Key="CalendarDatePickerLeftHeaderMaxWidth">296</x:Double>` |

### 2.5 `CheckBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CheckBoxBorderThemeThickness` | 2 | themeresources.xaml:L22(D), L2749(HC), L3938(L) | `<x:Double x:Key="CheckBoxBorderThemeThickness">2</x:Double>` |
| `CheckBoxCheckedStrokeThickness` | **Default=0 / HighContrast=2 / Light=0** | themeresources.xaml:L23(D), L2750(HC), L3939(L) | `<x:Double x:Key="CheckBoxCheckedStrokeThickness">0</x:Double>` |

### 2.6 `ComboBox*` —— 5 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ComboBoxArrowThemeFontSize` | 21 | themeresources.xaml:L24(D), L2751(HC), L3940(L) | `<x:Double x:Key="ComboBoxArrowThemeFontSize">21</x:Double>` |
| `ComboBoxLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5921(N) | `<x:Double x:Key="ComboBoxLeftHeaderMaxWidth">296</x:Double>` |
| `ComboBoxPopupThemeMinWidth` | 80 | themeresources.xaml:L26(D), L2753(HC), L3942(L) | `<x:Double x:Key="ComboBoxPopupThemeMinWidth">80</x:Double>` |
| `ComboBoxPopupThemeTouchMinWidth` | 240 | themeresources.xaml:L27(D), L2754(HC), L3943(L) | `<x:Double x:Key="ComboBoxPopupThemeTouchMinWidth">240</x:Double>` |
| `ComboBoxThemeMinWidth` | 64 | themeresources.xaml:L25(D), L2752(HC), L3941(L) | `<x:Double x:Key="ComboBoxThemeMinWidth">64</x:Double>` |

### 2.7 `CommandBar*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CommandBarOverflowMaxHeight` | 198 | themeresources.xaml:L1763(D), L3728(HC), L5679(L) | `<x:Double x:Key="CommandBarOverflowMaxHeight">198</x:Double>` |
| `CommandBarOverflowMaxWidth` | 480 | themeresources.xaml:L1762(D), L3727(HC), L5678(L) | `<x:Double x:Key="CommandBarOverflowMaxWidth">480</x:Double>` |
| `CommandBarOverflowMinWidth` | 160 | themeresources.xaml:L1760(D), L3725(HC), L5676(L) | `<x:Double x:Key="CommandBarOverflowMinWidth">160</x:Double>` |
| `CommandBarOverflowTouchMinWidth` | 240 | themeresources.xaml:L1761(D), L3726(HC), L5677(L) | `<x:Double x:Key="CommandBarOverflowTouchMinWidth">240</x:Double>` |

### 2.8 `ContentControl*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ContentControlFontSize` | 14 | themeresources.xaml:L29(D), L2756(HC), L3945(L) | `<x:Double x:Key="ContentControlFontSize">14</x:Double>` |

### 2.9 `ContentDialog*` —— 9 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ContentDialogButtonHeight` | 32 | themeresources.xaml:L37(D), L2764(HC), L3953(L) | `<x:Double x:Key="ContentDialogButtonHeight">32</x:Double>` |
| `ContentDialogButtonMaxWidth` | 202 | themeresources.xaml:L35(D), L2762(HC), L3951(L) | `<x:Double x:Key="ContentDialogButtonMaxWidth">202</x:Double>` |
| `ContentDialogButtonMinHeight` | 32 | themeresources.xaml:L36(D), L2763(HC), L3952(L) | `<x:Double x:Key="ContentDialogButtonMinHeight">32</x:Double>` |
| `ContentDialogButtonMinWidth` | 130 | themeresources.xaml:L34(D), L2761(HC), L3950(L) | `<x:Double x:Key="ContentDialogButtonMinWidth">130</x:Double>` |
| `ContentDialogMaxHeight` | 756 | themeresources.xaml:L33(D), L2760(HC), L3949(L) | `<x:Double x:Key="ContentDialogMaxHeight">756</x:Double>` |
| `ContentDialogMaxWidth` | 548 | themeresources.xaml:L31(D), L2758(HC), L3947(L) | `<x:Double x:Key="ContentDialogMaxWidth">548</x:Double>` |
| `ContentDialogMinHeight` | 184 | themeresources.xaml:L32(D), L2759(HC), L3948(L) | `<x:Double x:Key="ContentDialogMinHeight">184</x:Double>` |
| `ContentDialogMinWidth` | 320 | themeresources.xaml:L30(D), L2757(HC), L3946(L) | `<x:Double x:Key="ContentDialogMinWidth">320</x:Double>` |
| `ContentDialogTitleMaxHeight` | 56 | themeresources.xaml:L38(D), L2765(HC), L3954(L) | `<x:Double x:Key="ContentDialogTitleMaxHeight">56</x:Double>` |

### 2.10 `Control*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ControlContentThemeFontSize` | 14 | themeresources.xaml:L28(D), L2755(HC), L3944(L) | `<x:Double x:Key="ControlContentThemeFontSize">14</x:Double>` |

### 2.11 `DatePicker*` —— 9 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `DatePickerFlyoutPresenterAcceptDismissHostGridHeight` | 41 | themeresources.xaml:L5905(N) | `<x:Double x:Key="DatePickerFlyoutPresenterAcceptDismissHostGridHeight">41</x:Double>` |
| `DatePickerFlyoutPresenterHighlightHeight` | 40 | themeresources.xaml:L5904(N) | `<x:Double x:Key="DatePickerFlyoutPresenterHighlightHeight">40</x:Double>` |
| `DatePickerFlyoutPresenterItemHeight` | 40 | themeresources.xaml:L5906(N) | `<x:Double x:Key="DatePickerFlyoutPresenterItemHeight">40</x:Double>` |
| `DatePickerLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5922(N) | `<x:Double x:Key="DatePickerLeftHeaderMaxWidth">296</x:Double>` |
| `DatePickerSelectorThemeMinWidth` | 80 | themeresources.xaml:L39(D), L2766(HC), L3955(L) | `<x:Double x:Key="DatePickerSelectorThemeMinWidth">80</x:Double>` |
| `DatePickerSpacingThemeHeight` | 20 | themeresources.xaml:L41(D), L2768(HC), L3957(L) | `<x:Double x:Key="DatePickerSpacingThemeHeight">20</x:Double>` |
| `DatePickerSpacingThemeWidth` | 20 | themeresources.xaml:L40(D), L2767(HC), L3956(L) | `<x:Double x:Key="DatePickerSpacingThemeWidth">20</x:Double>` |
| `DatePickerThemeMaxWidth` | 456 | themeresources.xaml:L5916(N) | `<x:Double x:Key="DatePickerThemeMaxWidth">456</x:Double>` |
| `DatePickerThemeMinWidth` | 296 | themeresources.xaml:L5915(N) | `<x:Double x:Key="DatePickerThemeMinWidth">296</x:Double>` |

### 2.12 `Flyout*` —— 5 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `FlyoutThemeMaxHeight` | 758 | themeresources.xaml:L42(D), L2769(HC), L3958(L) | `<x:Double x:Key="FlyoutThemeMaxHeight">758</x:Double>` |
| `FlyoutThemeMaxWidth` | 456 | themeresources.xaml:L43(D), L2770(HC), L3959(L) | `<x:Double x:Key="FlyoutThemeMaxWidth">456</x:Double>` |
| `FlyoutThemeMinHeight` | 40 | themeresources.xaml:L44(D), L2771(HC), L3960(L) | `<x:Double x:Key="FlyoutThemeMinHeight">40</x:Double>` |
| `FlyoutThemeMinWidth` | 96 | themeresources.xaml:L45(D), L2772(HC), L3961(L) | `<x:Double x:Key="FlyoutThemeMinWidth">96</x:Double>` |
| `FlyoutThemeTouchMinWidth` | 240 | themeresources.xaml:L46(D), L2773(HC), L3962(L) | `<x:Double x:Key="FlyoutThemeTouchMinWidth">240</x:Double>` |

### 2.13 `GridView*` —— 6 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `GridViewHeaderItemMinHeight` | 44 | themeresources.xaml:L103(D), L2830(HC), L4019(L) | `<x:Double x:Key="GridViewHeaderItemMinHeight">44</x:Double>` |
| `GridViewHeaderItemThemeFontSize` | 20 | themeresources.xaml:L105(D), L2832(HC), L4021(L) | `<x:Double x:Key="GridViewHeaderItemThemeFontSize">20</x:Double>` |
| `GridViewItemMinHeight` | 44 | themeresources.xaml:L108(D), L2835(HC), L4024(L) | `<x:Double x:Key="GridViewItemMinHeight">44</x:Double>` |
| `GridViewItemMinWidth` | 44 | themeresources.xaml:L107(D), L2834(HC), L4023(L) | `<x:Double x:Key="GridViewItemMinWidth">44</x:Double>` |
| `GridViewItemReorderHintThemeOffset` | 16.0 | themeresources.xaml:L52(D), L2779(HC), L3968(L) | `<x:Double x:Key="GridViewItemReorderHintThemeOffset">16.0</x:Double>` |
| `GridViewItemSelectedBorderThemeThickness` | 4 | themeresources.xaml:L47(D), L2774(HC), L3963(L) | `<x:Double x:Key="GridViewItemSelectedBorderThemeThickness">4</x:Double>` |

### 2.14 `Hub*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HubHeaderThemeFontSize` | 34 | themeresources.xaml:L48(D), L2775(HC), L3964(L) | `<x:Double x:Key="HubHeaderThemeFontSize">34</x:Double>` |
| `HubSectionHeaderSeeMoreThemeFontSize` | 14 | themeresources.xaml:L50(D), L2777(HC), L3966(L) | `<x:Double x:Key="HubSectionHeaderSeeMoreThemeFontSize">14</x:Double>` |
| `HubSectionHeaderThemeFontSize` | 20 | themeresources.xaml:L49(D), L2776(HC), L3965(L) | `<x:Double x:Key="HubSectionHeaderThemeFontSize">20</x:Double>` |

### 2.15 `InkToolbar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `InkToolbarButtonContentSize` | 16 | themeresources.xaml:L1289(D), L3247(HC), L5205(L) | `<x:Double x:Key="InkToolbarButtonContentSize">16</x:Double>` |

### 2.16 `KeyTip*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `KeyTipContentThemeFontSize` | 12 | themeresources.xaml:L109(D), L2836(HC), L4025(L) | `<x:Double x:Key="KeyTipContentThemeFontSize">12</x:Double>` |

### 2.17 `ListPickerFlyout*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ListPickerFlyoutFooterThemeHeight` | 80 | themeresources.xaml:L51(D), L2778(HC), L3967(L) | `<x:Double x:Key="ListPickerFlyoutFooterThemeHeight">80</x:Double>` |

### 2.18 `ListView*` —— 12 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ListViewHeaderItemMinHeight` | 44 | themeresources.xaml:L102(D), L2829(HC), L4018(L) | `<x:Double x:Key="ListViewHeaderItemMinHeight">44</x:Double>` |
| `ListViewHeaderItemThemeFontSize` | 20 | themeresources.xaml:L104(D), L2831(HC), L4020(L) | `<x:Double x:Key="ListViewHeaderItemThemeFontSize">20</x:Double>` |
| `ListViewItemContentOffsetX` | -40.5 | themeresources.xaml:L1771(D), L3749(HC), L5687(L) | `<x:Double x:Key="ListViewItemContentOffsetX">-40.5</x:Double>` |
| `ListViewItemDisabledThemeOpacity` | 0.55 | themeresources.xaml:L1772(D), L3750(HC), L5688(L) | `<x:Double x:Key="ListViewItemDisabledThemeOpacity">0.55</x:Double>` |
| `ListViewItemDragThemeOpacity` | 0.80 | themeresources.xaml:L1773(D), L3751(HC), L5689(L) | `<x:Double x:Key="ListViewItemDragThemeOpacity">0.80</x:Double>` |
| `ListViewItemMinHeight` | 40 | themeresources.xaml:L1780(D), L3758(HC), L5696(L) | `<x:Double x:Key="ListViewItemMinHeight">40</x:Double>` |
| `ListViewItemMinWidth` | 88 | themeresources.xaml:L1779(D), L3757(HC), L5695(L) | `<x:Double x:Key="ListViewItemMinWidth">88</x:Double>` |
| `ListViewItemReorderHintThemeOffset` | 10.0 | themeresources.xaml:L1777(D), L3755(HC), L5693(L) | `<x:Double x:Key="ListViewItemReorderHintThemeOffset">10.0</x:Double>` |
| `ListViewItemReorderTargetThemeOpacity` | 0.50 | themeresources.xaml:L1775(D), L3753(HC), L5691(L) | `<x:Double x:Key="ListViewItemReorderTargetThemeOpacity">0.50</x:Double>` |
| `ListViewItemReorderTargetThemeScale` | 0.95 | themeresources.xaml:L1776(D), L3754(HC), L5692(L) | `<x:Double x:Key="ListViewItemReorderTargetThemeScale">0.95</x:Double>` |
| `ListViewItemReorderThemeOpacity` | 0.80 | themeresources.xaml:L1774(D), L3752(HC), L5690(L) | `<x:Double x:Key="ListViewItemReorderThemeOpacity">0.80</x:Double>` |
| `ListViewItemSelectedBorderThemeThickness` | 4 | themeresources.xaml:L1778(D), L3756(HC), L5694(L) | `<x:Double x:Key="ListViewItemSelectedBorderThemeThickness">4</x:Double>` |

### 2.19 `MenuBar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MenuBarHeight` | 40 | themeresources.xaml:L6053(N) | `<x:Double x:Key="MenuBarHeight">40</x:Double>` |

### 2.20 `MenuFlyout*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MenuFlyoutSeparatorThemeHeight` | 1 | themeresources.xaml:L1546(D), L3530(HC), L5462(L) | `<x:Double x:Key="MenuFlyoutSeparatorThemeHeight">1</x:Double>` |
| `MenuFlyoutThemeMinHeight` | 32 | themeresources.xaml:L1547(D), L3531(HC), L5463(L) | `<x:Double x:Key="MenuFlyoutThemeMinHeight">32</x:Double>` |

### 2.21 `MTC*` —— 15 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MTCControlPanelHeight` | 42 | themeresources.xaml:L53(D), L2780(HC), L3969(L) | `<x:Double x:Key="MTCControlPanelHeight">42</x:Double>` |
| `MTCHorizontalVolumeSliderWidth` | 180 | themeresources.xaml:L54(D), L2781(HC), L3970(L) | `<x:Double x:Key="MTCHorizontalVolumeSliderWidth">180</x:Double>` |
| `MTCMediaButtonHeight` | 48 | themeresources.xaml:L56(D), L2783(HC), L3972(L) | `<x:Double x:Key="MTCMediaButtonHeight">48</x:Double>` |
| `MTCMediaButtonWidth` | 48 | themeresources.xaml:L57(D), L2784(HC), L3973(L) | `<x:Double x:Key="MTCMediaButtonWidth">48</x:Double>` |
| `MTCMediaFontSize` | 12 | themeresources.xaml:L55(D), L2782(HC), L3971(L) | `<x:Double x:Key="MTCMediaFontSize">12</x:Double>` |
| `MTCPositionSliderMinimumWidth` | 96 | themeresources.xaml:L58(D), L2785(HC), L3974(L) | `<x:Double x:Key="MTCPositionSliderMinimumWidth">96</x:Double>` |
| `MTCSideMargins` | 16 | themeresources.xaml:L59(D), L2786(HC), L3975(L) | `<x:Double x:Key="MTCSideMargins">16</x:Double>` |
| `MTCTimeButtonHeight` | 21 | themeresources.xaml:L60(D), L2787(HC), L3976(L) | `<x:Double x:Key="MTCTimeButtonHeight">21</x:Double>` |
| `MTCTimeButtonWidth` | 62 | themeresources.xaml:L61(D), L2788(HC), L3977(L) | `<x:Double x:Key="MTCTimeButtonWidth">62</x:Double>` |
| `MTCVerticalVolumeHostVerticalOffset` | -112 | themeresources.xaml:L62(D), L2789(HC), L3978(L) | `<x:Double x:Key="MTCVerticalVolumeHostVerticalOffset">-112</x:Double>` |
| `MTCVerticalVolumeHostWidth` | 42 | themeresources.xaml:L63(D), L2790(HC), L3979(L) | `<x:Double x:Key="MTCVerticalVolumeHostWidth">42</x:Double>` |
| `MTCVerticalVolumeSliderMaxHeight` | 289 | themeresources.xaml:L64(D), L2791(HC), L3980(L) | `<x:Double x:Key="MTCVerticalVolumeSliderMaxHeight">289</x:Double>` |
| `MTCVerticalVolumeSliderMinHeight` | 96 | themeresources.xaml:L65(D), L2792(HC), L3981(L) | `<x:Double x:Key="MTCVerticalVolumeSliderMinHeight">96</x:Double>` |
| `MTCVerticalVolumeSliderTopGap` | 8 | themeresources.xaml:L66(D), L2793(HC), L3982(L) | `<x:Double x:Key="MTCVerticalVolumeSliderTopGap">8</x:Double>` |
| `MTCVerticalVolumeSliderTopPadding` | 16 | themeresources.xaml:L67(D), L2794(HC), L3983(L) | `<x:Double x:Key="MTCVerticalVolumeSliderTopPadding">16</x:Double>` |

### 2.22 `NavigationBackButton*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `NavigationBackButtonHeight` | 40 | themeresources.xaml:L6052(N) | `<x:Double x:Key="NavigationBackButtonHeight">40</x:Double>` |
| `NavigationBackButtonWidth` | 40 | themeresources.xaml:L6051(N) | `<x:Double x:Key="NavigationBackButtonWidth">40</x:Double>` |

### 2.23 `NavigationView*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `NavigationViewCompactPaneLength` | 40 | themeresources.xaml:L6042(N) | `<x:Double x:Key="NavigationViewCompactPaneLength">40</x:Double>` |
| `NavigationViewTopPaneHeight` | 40 | themeresources.xaml:L6043(N) | `<x:Double x:Key="NavigationViewTopPaneHeight">40</x:Double>` |

### 2.24 `PaneToggleButton*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PaneToggleButtonHeight` | 40 | themeresources.xaml:L6047(N) | `<x:Double x:Key="PaneToggleButtonHeight">40</x:Double>` |
| `PaneToggleButtonSize` | 40 | themeresources.xaml:L6041(N) | `<x:Double x:Key="PaneToggleButtonSize">40</x:Double>` |
| `PaneToggleButtonWidth` | 40 | themeresources.xaml:L6048(N) | `<x:Double x:Key="PaneToggleButtonWidth">40</x:Double>` |

### 2.25 `PasswordBox*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PasswordBoxLeftHeaderMaxWidth` | 296 | themeresources.xaml:L6088(N) | `<x:Double x:Key="PasswordBoxLeftHeaderMaxWidth">296</x:Double>` |

### 2.26 `PersonPicture*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PersonPictureEllipseBadgeImageSourceStrokeOpacity` | 1.0 | themeresources.xaml:L1662(D), L3620(HC), L5578(L) | `<x:Double x:Key="PersonPictureEllipseBadgeImageSourceStrokeOpacity">1.0</x:Double>` |
| `PersonPictureEllipseBadgeStrokeOpacity` | **Default=0.8 / HighContrast=1.0 / Light=0.8** | themeresources.xaml:L1661(D), L3619(HC), L5577(L) | `<x:Double x:Key="PersonPictureEllipseBadgeStrokeOpacity">0.8</x:Double>` |
| `PersonPictureEllipseBadgeStrokeThickness` | 2 | themeresources.xaml:L1664(D), L3622(HC), L5580(L) | `<x:Double x:Key="PersonPictureEllipseBadgeStrokeThickness">2</x:Double>` |
| `PersonPictureEllipseStrokeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L1663(D), L3621(HC), L5579(L) | `<x:Double x:Key="PersonPictureEllipseStrokeThickness">0</x:Double>` |

### 2.27 `Pivot*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PivotHeaderItemFontSize` | 24 | themeresources.xaml:L68(D), L2795(HC), L3984(L) | `<x:Double x:Key="PivotHeaderItemFontSize">24</x:Double>` |
| `PivotHeaderItemLockedTranslation` | 40 | themeresources.xaml:L69(D), L2797(HC), L3985(L) | `<x:Double x:Key="PivotHeaderItemLockedTranslation">40</x:Double>` |
| `PivotTitleFontSize` | 14 | themeresources.xaml:L70(D), L2796(HC), L3986(L) | `<x:Double x:Key="PivotTitleFontSize">14</x:Double>` |

### 2.28 `ProgressBar*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ProgressBarIndicatorPauseOpacity` | **Default=0.6 / HighContrast=1 / Light=0.6** | themeresources.xaml:L71(D), L2798(HC), L3987(L) | `<x:Double x:Key="ProgressBarIndicatorPauseOpacity">0.6</x:Double>` |
| `ProgressBarThemeMinHeight` | 4 | themeresources.xaml:L72(D), L2799(HC), L3988(L) | `<x:Double x:Key="ProgressBarThemeMinHeight">4</x:Double>` |

### 2.29 `RadioButton*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `RadioButtonBorderThemeThickness` | 2 | themeresources.xaml:L73(D), L2800(HC), L3989(L) | `<x:Double x:Key="RadioButtonBorderThemeThickness">2</x:Double>` |

### 2.30 `RichEditBox*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `RichEditBoxLeftHeaderMaxWidth` | 296 | themeresources.xaml:L6085(N) | `<x:Double x:Key="RichEditBoxLeftHeaderMaxWidth">296</x:Double>` |

### 2.31 `ScrollBar*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ScrollBarButtonArrowIconFontSize` | 8 | themeresources.xaml:L614(D), L2372(HC), L4530(L) | `<x:Double x:Key="ScrollBarButtonArrowIconFontSize">8</x:Double>` |
| `ScrollBarSize` | 16 | themeresources.xaml:L611(D), L2369(HC), L4527(L) | `<x:Double x:Key="ScrollBarSize">16</x:Double>` |
| `ScrollBarTrackBorderThemeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L74(D), L2801(HC), L3990(L) | `<x:Double x:Key="ScrollBarTrackBorderThemeThickness">0</x:Double>` |

### 2.32 `SearchBox*` —— 6 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SearchBoxContentThemeFontSize` | 14 | themeresources.xaml:L75(D), L2802(HC), L3991(L) | `<x:Double x:Key="SearchBoxContentThemeFontSize">14</x:Double>` |
| `SearchBoxResultSuggestionImageThemeHeight` | 32 | themeresources.xaml:L77(D), L2804(HC), L3993(L) | `<x:Double x:Key="SearchBoxResultSuggestionImageThemeHeight">32</x:Double>` |
| `SearchBoxResultSuggestionImageThemeWidth` | 32 | themeresources.xaml:L76(D), L2803(HC), L3992(L) | `<x:Double x:Key="SearchBoxResultSuggestionImageThemeWidth">32</x:Double>` |
| `SearchBoxSuggestionPopupThemeMaxHeight` | 300 | themeresources.xaml:L79(D), L2806(HC), L3995(L) | `<x:Double x:Key="SearchBoxSuggestionPopupThemeMaxHeight">300</x:Double>` |
| `SearchBoxSuggestionPopupThemeMinWidth` | 270 | themeresources.xaml:L78(D), L2805(HC), L3994(L) | `<x:Double x:Key="SearchBoxSuggestionPopupThemeMinWidth">270</x:Double>` |
| `SearchBoxTextBoxThemeMinHeight` | 28 | themeresources.xaml:L80(D), L2807(HC), L3996(L) | `<x:Double x:Key="SearchBoxTextBoxThemeMinHeight">28</x:Double>` |

### 2.33 `SemanticZoom*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SemanticZoomButtonFontSize` | 4 | themeresources.xaml:L81(D), L2808(HC), L3997(L) | `<x:Double x:Key="SemanticZoomButtonFontSize">4</x:Double>` |

### 2.34 `SettingsFlyout*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SettingsFlyoutHeaderThemeFontSize` | 26.667 | themeresources.xaml:L82(D), L2809(HC), L3998(L) | `<x:Double x:Key="SettingsFlyoutHeaderThemeFontSize">26.667</x:Double>` |

### 2.35 `Slider*` —— 7 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SliderHorizontalHeight` | 32 | themeresources.xaml:L5911(N) | `<x:Double x:Key="SliderHorizontalHeight">32</x:Double>` |
| `SliderLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5923(N) | `<x:Double x:Key="SliderLeftHeaderMaxWidth">296</x:Double>` |
| `SliderOutsideTickBarThemeHeight` | 4 | themeresources.xaml:L83(D), L2810(HC), L3999(L) | `<x:Double x:Key="SliderOutsideTickBarThemeHeight">4</x:Double>` |
| `SliderPostContentMargin` | 15 | themeresources.xaml:L5910(N) | `<x:Double x:Key="SliderPostContentMargin">15</x:Double>` |
| `SliderPreContentMargin` | 15 | themeresources.xaml:L5909(N) | `<x:Double x:Key="SliderPreContentMargin">15</x:Double>` |
| `SliderTrackThemeHeight` | 2 | themeresources.xaml:L84(D), L2811(HC), L4000(L) | `<x:Double x:Key="SliderTrackThemeHeight">2</x:Double>` |
| `SliderVerticalWidth` | 32 | themeresources.xaml:L5912(N) | `<x:Double x:Key="SliderVerticalWidth">32</x:Double>` |

### 2.36 `SmallScrollThumb*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SmallScrollThumbOffset` | -2 | themeresources.xaml:L613(D), L2371(HC), L4529(L) | `<x:Double x:Key="SmallScrollThumbOffset">-2</x:Double>` |
| `SmallScrollThumbScale` | 0.125 | themeresources.xaml:L612(D), L2370(HC), L4528(L) | `<x:Double x:Key="SmallScrollThumbScale">0.125</x:Double>` |

### 2.37 `SplitButton*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SplitButtonPrimaryButtonSize` | 32 | themeresources.xaml:L6077(N) | `<x:Double x:Key="SplitButtonPrimaryButtonSize">32</x:Double>` |
| `SplitButtonSecondaryButtonSize` | 32 | themeresources.xaml:L6078(N) | `<x:Double x:Key="SplitButtonSecondaryButtonSize">32</x:Double>` |

### 2.38 `SplitView*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SplitViewCompactPaneThemeLength` | 48 | themeresources.xaml:L86(D), L2813(HC), L4002(L) | `<x:Double x:Key="SplitViewCompactPaneThemeLength">48</x:Double>` |
| `SplitViewOpenPaneThemeLength` | 320 | themeresources.xaml:L85(D), L2812(HC), L4001(L) | `<x:Double x:Key="SplitViewOpenPaneThemeLength">320</x:Double>` |

### 2.39 `TextBox*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TextBoxLeftHeaderMaxWidth` | 296 | themeresources.xaml:L6090(N) | `<x:Double x:Key="TextBoxLeftHeaderMaxWidth">296</x:Double>` |

### 2.40 `TextControl*` —— 11 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TextControlBackgroundFocusedOpacity` | 1 | themeresources.xaml:L95(D), L2826(HC), L4011(L) | `<x:Double x:Key="TextControlBackgroundFocusedOpacity">1</x:Double>` |
| `TextControlBackgroundHoverOpacity` | 0.6 | themeresources.xaml:L94(D), L2825(HC), L4010(L) | `<x:Double x:Key="TextControlBackgroundHoverOpacity">0.6</x:Double>` |
| `TextControlBackgroundRestOpacity` | 0.4 | themeresources.xaml:L93(D), L2824(HC), L4009(L) | `<x:Double x:Key="TextControlBackgroundRestOpacity">0.4</x:Double>` |
| `TextControlBackgroundThemeOpacity` | **Default=0.8 / HighContrast=1 / Light=0.8** | themeresources.xaml:L87(D), L2814(HC), L4003(L) | `<x:Double x:Key="TextControlBackgroundThemeOpacity">0.8</x:Double>` |
| `TextControlBorderThemeBrushOpacity` | **Default=1 / HighContrast=1 / Light=0.5625** | themeresources.xaml:L89(D), L2816(HC), L4005(L) | `<x:Double x:Key="TextControlBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlBorderThemeOpacity` | **Default=0.8 / HighContrast=1 / Light=0.45** | themeresources.xaml:L88(D), L2815(HC), L4004(L) | `<x:Double x:Key="TextControlBorderThemeOpacity">0.8</x:Double>` |
| `TextControlPointerOverBackgroundThemeOpacity` | **Default=0.87 / HighContrast=1 / Light=0.87** | themeresources.xaml:L90(D), L2817(HC), L4006(L) | `<x:Double x:Key="TextControlPointerOverBackgroundThemeOpacity">0.87</x:Double>` |
| `TextControlPointerOverBorderThemeBrushOpacity` | **Default=1 / HighContrast=1 / Light=0.839** | themeresources.xaml:L92(D), L2819(HC), L4008(L) | `<x:Double x:Key="TextControlPointerOverBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlPointerOverBorderThemeOpacity` | **Default=0.87 / HighContrast=1 / Light=0.73** | themeresources.xaml:L91(D), L2818(HC), L4007(L) | `<x:Double x:Key="TextControlPointerOverBorderThemeOpacity">0.87</x:Double>` |
| `TextControlThemeMinHeight` | 32 | themeresources.xaml:L96(D), L2820(HC), L4012(L) | `<x:Double x:Key="TextControlThemeMinHeight">32</x:Double>` |
| `TextControlThemeMinWidth` | 64 | themeresources.xaml:L97(D), L2821(HC), L4013(L) | `<x:Double x:Key="TextControlThemeMinWidth">64</x:Double>` |

### 2.41 `TextStyle*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TextStyleExtraLargeFontSize` | 25.5 | themeresources.xaml:L99(D), L2823(HC), L4015(L) | `<x:Double x:Key="TextStyleExtraLargeFontSize">25.5</x:Double>` |
| `TextStyleLargeFontSize` | 18.14 | themeresources.xaml:L98(D), L2822(HC), L4014(L) | `<x:Double x:Key="TextStyleLargeFontSize">18.14</x:Double>` |

### 2.42 `TimePicker*` —— 7 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TimePickerFlyoutPresenterAcceptDismissHostGridHeight` | 41 | themeresources.xaml:L5902(N) | `<x:Double x:Key="TimePickerFlyoutPresenterAcceptDismissHostGridHeight">41</x:Double>` |
| `TimePickerFlyoutPresenterHighlightHeight` | 40 | themeresources.xaml:L5901(N) | `<x:Double x:Key="TimePickerFlyoutPresenterHighlightHeight">40</x:Double>` |
| `TimePickerFlyoutPresenterItemHeight` | 40 | themeresources.xaml:L5903(N) | `<x:Double x:Key="TimePickerFlyoutPresenterItemHeight">40</x:Double>` |
| `TimePickerLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5924(N) | `<x:Double x:Key="TimePickerLeftHeaderMaxWidth">296</x:Double>` |
| `TimePickerSelectorThemeMinWidth` | 80 | themeresources.xaml:L100(D), L2827(HC), L4016(L) | `<x:Double x:Key="TimePickerSelectorThemeMinWidth">80</x:Double>` |
| `TimePickerThemeMaxWidth` | 456 | themeresources.xaml:L5918(N) | `<x:Double x:Key="TimePickerThemeMaxWidth">456</x:Double>` |
| `TimePickerThemeMinWidth` | 242 | themeresources.xaml:L5917(N) | `<x:Double x:Key="TimePickerThemeMinWidth">242</x:Double>` |

### 2.43 `ToggleSwitch*` —— 5 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToggleSwitchLeftHeaderMaxWidth` | 296 | themeresources.xaml:L5925(N) | `<x:Double x:Key="ToggleSwitchLeftHeaderMaxWidth">296</x:Double>` |
| `ToggleSwitchOnStrokeThickness` | **Default=0 / HighContrast=2 / Light=0** | themeresources.xaml:L106(D), L2833(HC), L4022(L) | `<x:Double x:Key="ToggleSwitchOnStrokeThickness">0</x:Double>` |
| `ToggleSwitchPostContentMargin` | 6 | themeresources.xaml:L5914(N) | `<x:Double x:Key="ToggleSwitchPostContentMargin">6</x:Double>` |
| `ToggleSwitchPreContentMargin` | 6 | themeresources.xaml:L5913(N) | `<x:Double x:Key="ToggleSwitchPreContentMargin">6</x:Double>` |
| `ToggleSwitchThemeMinWidth` | 154 | themeresources.xaml:L5919(N) | `<x:Double x:Key="ToggleSwitchThemeMinWidth">154</x:Double>` |

### 2.44 `ToolTip*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToolTipContentThemeFontSize` | 12 | themeresources.xaml:L101(D), L2828(HC), L4017(L) | `<x:Double x:Key="ToolTipContentThemeFontSize">12</x:Double>` |

### 2.45 `TreeViewItem*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TreeViewItemContentHeight` | 32 | themeresources.xaml:L5908(N) | `<x:Double x:Key="TreeViewItemContentHeight">32</x:Double>` |
| `TreeViewItemMinHeight` | 32 | themeresources.xaml:L5907(N) | `<x:Double x:Key="TreeViewItemMinHeight">32</x:Double>` |

### 2.0 跨主题字典取值不同的 `x:Double` 键（13 个）

> 高对比态把「透明/无描边」改成「1 或 2 的不透明描边」，是这批差异的全部成因。

| 键名 | 态 | 值 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `AutoSuggestListBorderOpacity` | Default 暗 | 0 | themeresources.xaml:L21 | `<x:Double x:Key="AutoSuggestListBorderOpacity">0</x:Double>` |
| `AutoSuggestListBorderOpacity` | HighContrast | 1 | themeresources.xaml:L2748 | `<x:Double x:Key="AutoSuggestListBorderOpacity">1</x:Double>` |
| `AutoSuggestListBorderOpacity` | Light 亮 | 0 | themeresources.xaml:L3937 | `<x:Double x:Key="AutoSuggestListBorderOpacity">0</x:Double>` |
| `CheckBoxCheckedStrokeThickness` | Default 暗 | 0 | themeresources.xaml:L23 | `<x:Double x:Key="CheckBoxCheckedStrokeThickness">0</x:Double>` |
| `CheckBoxCheckedStrokeThickness` | HighContrast | 2 | themeresources.xaml:L2750 | `<x:Double x:Key="CheckBoxCheckedStrokeThickness">2</x:Double>` |
| `CheckBoxCheckedStrokeThickness` | Light 亮 | 0 | themeresources.xaml:L3939 | `<x:Double x:Key="CheckBoxCheckedStrokeThickness">0</x:Double>` |
| `PersonPictureEllipseBadgeStrokeOpacity` | Default 暗 | 0.8 | themeresources.xaml:L1661 | `<x:Double x:Key="PersonPictureEllipseBadgeStrokeOpacity">0.8</x:Double>` |
| `PersonPictureEllipseBadgeStrokeOpacity` | HighContrast | 1.0 | themeresources.xaml:L3619 | `<x:Double x:Key="PersonPictureEllipseBadgeStrokeOpacity">1.0</x:Double>` |
| `PersonPictureEllipseBadgeStrokeOpacity` | Light 亮 | 0.8 | themeresources.xaml:L5577 | `<x:Double x:Key="PersonPictureEllipseBadgeStrokeOpacity">0.8</x:Double>` |
| `PersonPictureEllipseStrokeThickness` | Default 暗 | 0 | themeresources.xaml:L1663 | `<x:Double x:Key="PersonPictureEllipseStrokeThickness">0</x:Double>` |
| `PersonPictureEllipseStrokeThickness` | HighContrast | 1 | themeresources.xaml:L3621 | `<x:Double x:Key="PersonPictureEllipseStrokeThickness">1</x:Double>` |
| `PersonPictureEllipseStrokeThickness` | Light 亮 | 0 | themeresources.xaml:L5579 | `<x:Double x:Key="PersonPictureEllipseStrokeThickness">0</x:Double>` |
| `ProgressBarIndicatorPauseOpacity` | Default 暗 | 0.6 | themeresources.xaml:L71 | `<x:Double x:Key="ProgressBarIndicatorPauseOpacity">0.6</x:Double>` |
| `ProgressBarIndicatorPauseOpacity` | HighContrast | 1 | themeresources.xaml:L2798 | `<x:Double x:Key="ProgressBarIndicatorPauseOpacity">1</x:Double>` |
| `ProgressBarIndicatorPauseOpacity` | Light 亮 | 0.6 | themeresources.xaml:L3987 | `<x:Double x:Key="ProgressBarIndicatorPauseOpacity">0.6</x:Double>` |
| `ScrollBarTrackBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L74 | `<x:Double x:Key="ScrollBarTrackBorderThemeThickness">0</x:Double>` |
| `ScrollBarTrackBorderThemeThickness` | HighContrast | 1 | themeresources.xaml:L2801 | `<x:Double x:Key="ScrollBarTrackBorderThemeThickness">1</x:Double>` |
| `ScrollBarTrackBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L3990 | `<x:Double x:Key="ScrollBarTrackBorderThemeThickness">0</x:Double>` |
| `TextControlBackgroundThemeOpacity` | Default 暗 | 0.8 | themeresources.xaml:L87 | `<x:Double x:Key="TextControlBackgroundThemeOpacity">0.8</x:Double>` |
| `TextControlBackgroundThemeOpacity` | HighContrast | 1 | themeresources.xaml:L2814 | `<x:Double x:Key="TextControlBackgroundThemeOpacity">1</x:Double>` |
| `TextControlBackgroundThemeOpacity` | Light 亮 | 0.8 | themeresources.xaml:L4003 | `<x:Double x:Key="TextControlBackgroundThemeOpacity">0.8</x:Double>` |
| `TextControlBorderThemeBrushOpacity` | Default 暗 | 1 | themeresources.xaml:L89 | `<x:Double x:Key="TextControlBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlBorderThemeBrushOpacity` | HighContrast | 1 | themeresources.xaml:L2816 | `<x:Double x:Key="TextControlBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlBorderThemeBrushOpacity` | Light 亮 | 0.5625 | themeresources.xaml:L4005 | `<x:Double x:Key="TextControlBorderThemeBrushOpacity">0.5625</x:Double>` |
| `TextControlBorderThemeOpacity` | Default 暗 | 0.8 | themeresources.xaml:L88 | `<x:Double x:Key="TextControlBorderThemeOpacity">0.8</x:Double>` |
| `TextControlBorderThemeOpacity` | HighContrast | 1 | themeresources.xaml:L2815 | `<x:Double x:Key="TextControlBorderThemeOpacity">1</x:Double>` |
| `TextControlBorderThemeOpacity` | Light 亮 | 0.45 | themeresources.xaml:L4004 | `<x:Double x:Key="TextControlBorderThemeOpacity">0.45</x:Double>` |
| `TextControlPointerOverBackgroundThemeOpacity` | Default 暗 | 0.87 | themeresources.xaml:L90 | `<x:Double x:Key="TextControlPointerOverBackgroundThemeOpacity">0.87</x:Double>` |
| `TextControlPointerOverBackgroundThemeOpacity` | HighContrast | 1 | themeresources.xaml:L2817 | `<x:Double x:Key="TextControlPointerOverBackgroundThemeOpacity">1</x:Double>` |
| `TextControlPointerOverBackgroundThemeOpacity` | Light 亮 | 0.87 | themeresources.xaml:L4006 | `<x:Double x:Key="TextControlPointerOverBackgroundThemeOpacity">0.87</x:Double>` |
| `TextControlPointerOverBorderThemeBrushOpacity` | Default 暗 | 1 | themeresources.xaml:L92 | `<x:Double x:Key="TextControlPointerOverBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlPointerOverBorderThemeBrushOpacity` | HighContrast | 1 | themeresources.xaml:L2819 | `<x:Double x:Key="TextControlPointerOverBorderThemeBrushOpacity">1</x:Double>` |
| `TextControlPointerOverBorderThemeBrushOpacity` | Light 亮 | 0.839 | themeresources.xaml:L4008 | `<x:Double x:Key="TextControlPointerOverBorderThemeBrushOpacity">0.839</x:Double>` |
| `TextControlPointerOverBorderThemeOpacity` | Default 暗 | 0.87 | themeresources.xaml:L91 | `<x:Double x:Key="TextControlPointerOverBorderThemeOpacity">0.87</x:Double>` |
| `TextControlPointerOverBorderThemeOpacity` | HighContrast | 1 | themeresources.xaml:L2818 | `<x:Double x:Key="TextControlPointerOverBorderThemeOpacity">1</x:Double>` |
| `TextControlPointerOverBorderThemeOpacity` | Light 亮 | 0.73 | themeresources.xaml:L4007 | `<x:Double x:Key="TextControlPointerOverBorderThemeOpacity">0.73</x:Double>` |
| `ToggleSwitchOnStrokeThickness` | Default 暗 | 0 | themeresources.xaml:L106 | `<x:Double x:Key="ToggleSwitchOnStrokeThickness">0</x:Double>` |
| `ToggleSwitchOnStrokeThickness` | HighContrast | 2 | themeresources.xaml:L2833 | `<x:Double x:Key="ToggleSwitchOnStrokeThickness">2</x:Double>` |
| `ToggleSwitchOnStrokeThickness` | Light 亮 | 0 | themeresources.xaml:L4022 | `<x:Double x:Key="ToggleSwitchOnStrokeThickness">0</x:Double>` |

## §3 `Thickness` 全表（按前缀分组）

行号列读法同 §2。`Thickness` 的三态差异共 14 个键，见 §3.1。

### 3.1 `AppBar*` —— 21 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AppBarBottomBorderThemeThickness` | **Default=0,0,0,0 / HighContrast=0,2,0,0 / Light=0,0,0,0** | themeresources.xaml:L111(D), L2838(HC), L4027(L) | `<Thickness x:Key="AppBarBottomBorderThemeThickness">0,0,0,0</Thickness>` |
| `AppBarBottomThemePadding` | 0,0,0,0 | themeresources.xaml:L112(D), L2839(HC), L4028(L) | `<Thickness x:Key="AppBarBottomThemePadding">0,0,0,0</Thickness>` |
| `AppBarButtonContentViewboxCollapsedMargin` | 0,12,0,4 | themeresources.xaml:L6065(N) | `<Thickness x:Key="AppBarButtonContentViewboxCollapsedMargin">0,12,0,4</Thickness>` |
| `AppBarButtonContentViewboxCompactMargin` | 0,12,0,12 | themeresources.xaml:L6054(N) | `<Thickness x:Key="AppBarButtonContentViewboxCompactMargin">0,12,0,12</Thickness>` |
| `AppBarButtonContentViewboxMargin` | 12,10,0,10 | themeresources.xaml:L6059(N) | `<Thickness x:Key="AppBarButtonContentViewboxMargin">12,10,0,10</Thickness>` |
| `AppBarButtonOverflowTextLabelPadding` | 0,5,0,8 | themeresources.xaml:L6058(N) | `<Thickness x:Key="AppBarButtonOverflowTextLabelPadding">0,5,0,8</Thickness>` |
| `AppBarButtonOverflowTextTouchMargin` | 0,9,0,12 | themeresources.xaml:L6056(N) | `<Thickness x:Key="AppBarButtonOverflowTextTouchMargin">0,9,0,12</Thickness>` |
| `AppBarButtonRevealBorderThemeThickness` | **Default=1 / HighContrast=0 / Light=1** | themeresources.xaml:L1417(D), L3375(HC), L5333(L) | `<Thickness x:Key="AppBarButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarButtonTextLabelMargin` | 2,0,2,8 | themeresources.xaml:L6057(N) | `<Thickness x:Key="AppBarButtonTextLabelMargin">2,0,2,8</Thickness>` |
| `AppBarButtonTextLabelOnRightMargin` | 8,11,12,13 | themeresources.xaml:L6055(N) | `<Thickness x:Key="AppBarButtonTextLabelOnRightMargin">8,11,12,13</Thickness>` |
| `AppBarEllipsisButtonRevealBorderThemeThickness` | **Default=1 / HighContrast=0 / Light=1** | themeresources.xaml:L1416(D), L3374(HC), L5332(L) | `<Thickness x:Key="AppBarEllipsisButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarExpandButtonCircleInnerPadding` | 3,0,3,0 | themeresources.xaml:L115(D), L2842(HC), L4031(L) | `<Thickness x:Key="AppBarExpandButtonCircleInnerPadding">3,0,3,0</Thickness>` |
| `AppBarToggleButtonOverflowCheckMargin` | 12,4,12,4 | themeresources.xaml:L6067(N) | `<Thickness x:Key="AppBarToggleButtonOverflowCheckMargin">12,4,12,4</Thickness>` |
| `AppBarToggleButtonOverflowCheckTouchMargin` | 12,10,12,10 | themeresources.xaml:L6062(N) | `<Thickness x:Key="AppBarToggleButtonOverflowCheckTouchMargin">12,10,12,10</Thickness>` |
| `AppBarToggleButtonOverflowTextLabelPadding` | 0,5,0,8 | themeresources.xaml:L6068(N) | `<Thickness x:Key="AppBarToggleButtonOverflowTextLabelPadding">0,5,0,8</Thickness>` |
| `AppBarToggleButtonOverflowTextTouchMargin` | 0,9,0,12 | themeresources.xaml:L6061(N) | `<Thickness x:Key="AppBarToggleButtonOverflowTextTouchMargin">0,9,0,12</Thickness>` |
| `AppBarToggleButtonRevealBorderThemeThickness` | **Default=1 / HighContrast=0 / Light=1** | themeresources.xaml:L1418(D), L3376(HC), L5334(L) | `<Thickness x:Key="AppBarToggleButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarToggleButtonTextLabelMargin` | 2,0,2,8 | themeresources.xaml:L6066(N) | `<Thickness x:Key="AppBarToggleButtonTextLabelMargin">2,0,2,8</Thickness>` |
| `AppBarToggleButtonTextLabelOnRightMargin` | 8,11,12,13 | themeresources.xaml:L6060(N) | `<Thickness x:Key="AppBarToggleButtonTextLabelOnRightMargin">8,11,12,13</Thickness>` |
| `AppBarTopBorderThemeThickness` | **Default=0,0,0,0 / HighContrast=0,0,0,2 / Light=0,0,0,0** | themeresources.xaml:L113(D), L2840(HC), L4029(L) | `<Thickness x:Key="AppBarTopBorderThemeThickness">0,0,0,0</Thickness>` |
| `AppBarTopThemePadding` | 0,0,0,0 | themeresources.xaml:L114(D), L2841(HC), L4030(L) | `<Thickness x:Key="AppBarTopThemePadding">0,0,0,0</Thickness>` |

### 3.2 `AutoSuggestBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AutoSuggestBoxLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L6091(N) | `<Thickness x:Key="AutoSuggestBoxLeftHeaderMargin">0,5,32,0</Thickness>` |
| `AutoSuggestBoxTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L6076(N) | `<Thickness x:Key="AutoSuggestBoxTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.3 `AutoSuggestList*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AutoSuggestListBorderThemeThickness` | 1 | themeresources.xaml:L116(D), L2843(HC), L4032(L) | `<Thickness x:Key="AutoSuggestListBorderThemeThickness">1</Thickness>` |
| `AutoSuggestListMargin` | 0,2,0,2 | themeresources.xaml:L117(D), L2844(HC), L4033(L) | `<Thickness x:Key="AutoSuggestListMargin">0,2,0,2</Thickness>` |
| `AutoSuggestListPadding` | -1,0,-1,0 | themeresources.xaml:L118(D), L2845(HC), L4034(L) | `<Thickness x:Key="AutoSuggestListPadding">-1,0,-1,0</Thickness>` |
| `AutoSuggestListViewItemMargin` | **Default=12,11,0,13 / HighContrast=10,11,0,13 / Light=12,11,0,13** | themeresources.xaml:L119(D), L2846(HC), L4035(L) | `<Thickness x:Key="AutoSuggestListViewItemMargin">12,11,0,13</Thickness>` |

### 3.4 `Button*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ButtonBorderThemeThickness` | 2 | themeresources.xaml:L120(D), L2847(HC), L4036(L) | `<Thickness x:Key="ButtonBorderThemeThickness">2</Thickness>` |
| `ButtonPadding` | 8,4,8,5 | themeresources.xaml:L5895(N) | `<Thickness x:Key="ButtonPadding">8,4,8,5</Thickness>` |
| `ButtonRevealBorderThemeThickness` | 2 | themeresources.xaml:L1413(D), L3371(HC), L5329(L) | `<Thickness x:Key="ButtonRevealBorderThemeThickness">2</Thickness>` |

### 3.5 `CalendarDatePicker*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CalendarDatePickerBorderThemeThickness` | 2 | themeresources.xaml:L121(D), L2848(HC), L4037(L) | `<Thickness x:Key="CalendarDatePickerBorderThemeThickness">2</Thickness>` |
| `CalendarDatePickerLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L5884(N) | `<Thickness x:Key="CalendarDatePickerLeftHeaderMargin">0,5,32,0</Thickness>` |
| `CalendarDatePickerTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5883(N) | `<Thickness x:Key="CalendarDatePickerTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.6 `ComboBox*` —— 15 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ComboBoxBorderThemeThickness` | 2 | themeresources.xaml:L122(D), L2849(HC), L4038(L) | `<Thickness x:Key="ComboBoxBorderThemeThickness">2</Thickness>` |
| `ComboBoxDropdownBorderPadding` | 0 | themeresources.xaml:L124(D), L2851(HC), L4040(L) | `<Thickness x:Key="ComboBoxDropdownBorderPadding">0</Thickness>` |
| `ComboBoxDropdownBorderThickness` | 1 | themeresources.xaml:L123(D), L2850(HC), L4039(L) | `<Thickness x:Key="ComboBoxDropdownBorderThickness">1</Thickness>` |
| `ComboBoxDropdownContentMargin` | 0,4,0,4 | themeresources.xaml:L125(D), L2852(HC), L4041(L) | `<Thickness x:Key="ComboBoxDropdownContentMargin">0,4,0,4</Thickness>` |
| `ComboBoxHeaderThemeMargin` | 0,0,0,4 | themeresources.xaml:L126(D), L2853(HC), L4042(L) | `<Thickness x:Key="ComboBoxHeaderThemeMargin">0,0,0,4</Thickness>` |
| `ComboBoxItemRevealBorderThemeThickness` | 1 | themeresources.xaml:L1421(D), L3379(HC), L5337(L) | `<Thickness x:Key="ComboBoxItemRevealBorderThemeThickness">1</Thickness>` |
| `ComboBoxItemRevealThemeGameControllerPadding` | 10,8,10,11 | themeresources.xaml:L1424(D), L3382(HC), L5340(L) | `<Thickness x:Key="ComboBoxItemRevealThemeGameControllerPadding">10,8,10,11</Thickness>` |
| `ComboBoxItemRevealThemePadding` | 10,4,10,7 | themeresources.xaml:L1422(D), L3380(HC), L5338(L) | `<Thickness x:Key="ComboBoxItemRevealThemePadding">10,4,10,7</Thickness>` |
| `ComboBoxItemRevealThemeTouchPadding` | 10,8,10,11 | themeresources.xaml:L1423(D), L3381(HC), L5339(L) | `<Thickness x:Key="ComboBoxItemRevealThemeTouchPadding">10,8,10,11</Thickness>` |
| `ComboBoxItemThemeGameControllerPadding` | 11,11,11,13 | themeresources.xaml:L130(D), L2857(HC), L4046(L) | `<Thickness x:Key="ComboBoxItemThemeGameControllerPadding">11,11,11,13</Thickness>` |
| `ComboBoxItemThemePadding` | 11,5,11,7 | themeresources.xaml:L128(D), L2855(HC), L4044(L) | `<Thickness x:Key="ComboBoxItemThemePadding">11,5,11,7</Thickness>` |
| `ComboBoxItemThemeTouchPadding` | 11,11,11,13 | themeresources.xaml:L129(D), L2856(HC), L4045(L) | `<Thickness x:Key="ComboBoxItemThemeTouchPadding">11,11,11,13</Thickness>` |
| `ComboBoxLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L5888(N) | `<Thickness x:Key="ComboBoxLeftHeaderMargin">0,5,32,0</Thickness>` |
| `ComboBoxPopupBorderThemeThickness` | 2 | themeresources.xaml:L127(D), L2854(HC), L4043(L) | `<Thickness x:Key="ComboBoxPopupBorderThemeThickness">2</Thickness>` |
| `ComboBoxTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5887(N) | `<Thickness x:Key="ComboBoxTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.7 `CommandBar*` —— 11 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CommandBarFlyoutBorderDownThemeThickness` | 1,0,1,1 | themeresources.xaml:L1899(D), L3857(HC), L5815(L) | `<Thickness x:Key="CommandBarFlyoutBorderDownThemeThickness">1,0,1,1</Thickness>` |
| `CommandBarFlyoutBorderThemeThickness` | 1 | themeresources.xaml:L1897(D), L3855(HC), L5813(L) | `<Thickness x:Key="CommandBarFlyoutBorderThemeThickness">1</Thickness>` |
| `CommandBarFlyoutBorderUpThemeThickness` | 1,1,1,0 | themeresources.xaml:L1898(D), L3856(HC), L5814(L) | `<Thickness x:Key="CommandBarFlyoutBorderUpThemeThickness">1,1,1,0</Thickness>` |
| `CommandBarMoreButtonMargin` | 14,19,14,0 | themeresources.xaml:L6072(N) | `<Thickness x:Key="CommandBarMoreButtonMargin">14,19,14,0</Thickness>` |
| `CommandBarOverflowPresenterBorderDownPadding` | 0 | themeresources.xaml:L1958(D), L3916(HC), L5874(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderDownPadding">0</Thickness>` |
| `CommandBarOverflowPresenterBorderDownThickness` | 0,0,0,1 | themeresources.xaml:L1955(D), L3913(HC), L5871(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderDownThickness">0,0,0,1</Thickness>` |
| `CommandBarOverflowPresenterBorderPadding` | 0 | themeresources.xaml:L1957(D), L3915(HC), L5873(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderPadding">0</Thickness>` |
| `CommandBarOverflowPresenterBorderThickness` | 1 | themeresources.xaml:L1954(D), L3912(HC), L5870(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderThickness">1</Thickness>` |
| `CommandBarOverflowPresenterBorderUpPadding` | 0 | themeresources.xaml:L1959(D), L3917(HC), L5875(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderUpPadding">0</Thickness>` |
| `CommandBarOverflowPresenterBorderUpThickness` | 0,1,0,0 | themeresources.xaml:L1956(D), L3914(HC), L5872(L) | `<Thickness x:Key="CommandBarOverflowPresenterBorderUpThickness">0,1,0,0</Thickness>` |
| `CommandBarOverflowPresenterMargin` | 0,4,0,4 | themeresources.xaml:L6086(N) | `<Thickness x:Key="CommandBarOverflowPresenterMargin">0,4,0,4</Thickness>` |

### 3.8 `ContentDialog*` —— 8 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ContentDialogBorderWidth` | 1 | themeresources.xaml:L131(D), L2858(HC), L4047(L) | `<Thickness x:Key="ContentDialogBorderWidth">1</Thickness>` |
| `ContentDialogButton1HostMargin` | 0,0,4,0 | themeresources.xaml:L132(D), L2859(HC), L4048(L) | `<Thickness x:Key="ContentDialogButton1HostMargin">0,0,4,0</Thickness>` |
| `ContentDialogButton2HostMargin` | 0,0,0,0 | themeresources.xaml:L133(D), L2860(HC), L4049(L) | `<Thickness x:Key="ContentDialogButton2HostMargin">0,0,0,0</Thickness>` |
| `ContentDialogCommandSpaceMargin` | 0,24,0,0 | themeresources.xaml:L136(D), L2863(HC), L4052(L) | `<Thickness x:Key="ContentDialogCommandSpaceMargin">0,24,0,0</Thickness>` |
| `ContentDialogContentMargin` | 0,0,0,0 | themeresources.xaml:L134(D), L2861(HC), L4050(L) | `<Thickness x:Key="ContentDialogContentMargin">0,0,0,0</Thickness>` |
| `ContentDialogContentScrollViewerMargin` | 0,0,0,0 | themeresources.xaml:L135(D), L2862(HC), L4051(L) | `<Thickness x:Key="ContentDialogContentScrollViewerMargin">0,0,0,0</Thickness>` |
| `ContentDialogPadding` | 24,18,24,24 | themeresources.xaml:L138(D), L2865(HC), L4054(L) | `<Thickness x:Key="ContentDialogPadding">24,18,24,24</Thickness>` |
| `ContentDialogTitleMargin` | 0,0,0,12 | themeresources.xaml:L137(D), L2864(HC), L4053(L) | `<Thickness x:Key="ContentDialogTitleMargin">0,0,0,12</Thickness>` |

### 3.9 `DatePicker*` —— 5 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `DatePickerFlyoutPresenterItemPadding` | 0,3,0,6 | themeresources.xaml:L5893(N) | `<Thickness x:Key="DatePickerFlyoutPresenterItemPadding">0,3,0,6</Thickness>` |
| `DatePickerFlyoutPresenterMonthPadding` | 9,3,0,6 | themeresources.xaml:L5894(N) | `<Thickness x:Key="DatePickerFlyoutPresenterMonthPadding">9,3,0,6</Thickness>` |
| `DatePickerHeaderThemeMargin` | 0,0,0,4 | themeresources.xaml:L139(D), L2866(HC), L4055(L) | `<Thickness x:Key="DatePickerHeaderThemeMargin">0,0,0,4</Thickness>` |
| `DatePickerLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L5882(N) | `<Thickness x:Key="DatePickerLeftHeaderMargin">0,5,32,0</Thickness>` |
| `DatePickerTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5881(N) | `<Thickness x:Key="DatePickerTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.10 `DateTimeFlyout*` —— 6 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `DateTimeFlyoutBorderPadding` | 0 | themeresources.xaml:L141(D), L2868(HC), L4057(L) | `<Thickness x:Key="DateTimeFlyoutBorderPadding">0</Thickness>` |
| `DateTimeFlyoutBorderThickness` | 1 | themeresources.xaml:L140(D), L2867(HC), L4056(L) | `<Thickness x:Key="DateTimeFlyoutBorderThickness">1</Thickness>` |
| `DateTimeFlyoutButtonBorderThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L142(D), L2869(HC), L4058(L) | `<Thickness x:Key="DateTimeFlyoutButtonBorderThickness">0</Thickness>` |
| `DateTimeFlyoutContentPanelLandscapeThemeMargin` | 0,19,0,0 | themeresources.xaml:L144(D), L2871(HC), L4060(L) | `<Thickness x:Key="DateTimeFlyoutContentPanelLandscapeThemeMargin">0,19,0,0</Thickness>` |
| `DateTimeFlyoutContentPanelPortraitThemeMargin` | 0,37,0,0 | themeresources.xaml:L143(D), L2870(HC), L4059(L) | `<Thickness x:Key="DateTimeFlyoutContentPanelPortraitThemeMargin">0,37,0,0</Thickness>` |
| `DateTimeFlyoutTitleThemeMargin` | 19,0,19,17.5 | themeresources.xaml:L145(D), L2872(HC), L4061(L) | `<Thickness x:Key="DateTimeFlyoutTitleThemeMargin">19,0,19,17.5</Thickness>` |

### 3.11 `FlipView*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `FlipViewButtonBorderThemeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L146(D), L2873(HC), L4062(L) | `<Thickness x:Key="FlipViewButtonBorderThemeThickness">0</Thickness>` |

### 3.12 `Flyout*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `FlyoutBorderThemePadding` | 0 | themeresources.xaml:L881(D), L2639(HC), L4797(L) | `<Thickness x:Key="FlyoutBorderThemePadding">0</Thickness>` |
| `FlyoutBorderThemeThickness` | 1 | themeresources.xaml:L880(D), L2638(HC), L4796(L) | `<Thickness x:Key="FlyoutBorderThemeThickness">1</Thickness>` |
| `FlyoutContentThemeMargin` | 0,0,0,0 | themeresources.xaml:L147(D), L2874(HC), L4063(L) | `<Thickness x:Key="FlyoutContentThemeMargin">0,0,0,0</Thickness>` |
| `FlyoutContentThemePadding` | 12,11,12,12 | themeresources.xaml:L148(D), L2875(HC), L4064(L) | `<Thickness x:Key="FlyoutContentThemePadding">12,11,12,12</Thickness>` |

### 3.13 `GridView*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `GridViewItemCompactSelectedBorderThemeThickness` | 4 | themeresources.xaml:L149(D), L2876(HC), L4065(L) | `<Thickness x:Key="GridViewItemCompactSelectedBorderThemeThickness">4</Thickness>` |
| `GridViewItemMultiselectBorderThickness` | 2.5 | themeresources.xaml:L150(D), L2877(HC), L4066(L) | `<Thickness x:Key="GridViewItemMultiselectBorderThickness">2.5</Thickness>` |
| `GridViewItemRevealBorderThemeThickness` | 1 | themeresources.xaml:L1420(D), L3378(HC), L5336(L) | `<Thickness x:Key="GridViewItemRevealBorderThemeThickness">1</Thickness>` |

### 3.14 `HandwritingView*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HandwritingViewExpandedButtonMargin` | 5,6,5,6 | themeresources.xaml:L151(D), L2878(HC), L4067(L) | `<Thickness x:Key="HandwritingViewExpandedButtonMargin">5,6,5,6</Thickness>` |

### 3.15 `HelperButton*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HelperButtonThemePadding` | 0,0,-2,0 | themeresources.xaml:L183(D), L2910(HC), L4099(L) | `<Thickness x:Key="HelperButtonThemePadding">0,0,-2,0</Thickness>` |

### 3.16 `Hub*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HubSectionHeaderSeeMoreThemeMargin` | 24,0,0,11 | themeresources.xaml:L153(D), L2880(HC), L4069(L) | `<Thickness x:Key="HubSectionHeaderSeeMoreThemeMargin">24,0,0,11</Thickness>` |
| `HubSectionHeaderThemeMargin` | 0,0,0,9 | themeresources.xaml:L152(D), L2879(HC), L4068(L) | `<Thickness x:Key="HubSectionHeaderThemeMargin">0,0,0,9</Thickness>` |

### 3.17 `HyperlinkButton*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HyperlinkButtonBorderThemeThickness` | 0 | themeresources.xaml:L154(D), L2881(HC), L4070(L) | `<Thickness x:Key="HyperlinkButtonBorderThemeThickness">0</Thickness>` |
| `HyperlinkButtonPadding` | 0,6,0,7 | themeresources.xaml:L5896(N) | `<Thickness x:Key="HyperlinkButtonPadding">0,6,0,7</Thickness>` |

### 3.18 `InkToolbar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `InkToolbarButtonBorderThemeThickness` | 1 | themeresources.xaml:L1323(D), L3281(HC), L5239(L) | `<Thickness x:Key="InkToolbarButtonBorderThemeThickness">1</Thickness>` |

### 3.19 `KeyTip*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `KeyTipBorderThemeThickness` | 1 | themeresources.xaml:L189(D), L2916(HC), L4105(L) | `<Thickness x:Key="KeyTipBorderThemeThickness">1</Thickness>` |
| `KeyTipThemePadding` | 4 | themeresources.xaml:L190(D), L2917(HC), L4106(L) | `<Thickness x:Key="KeyTipThemePadding">4</Thickness>` |

### 3.20 `LanguageSwitcher*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `LanguageSwitcherMenuFlyoutItemPlaceholderThemeThickness` | 44,0,0,0 | themeresources.xaml:L1960(D), L3918(HC), L5876(L) | `<Thickness x:Key="LanguageSwitcherMenuFlyoutItemPlaceholderThemeThickness">44,0,0,0</Thickness>` |
| `LanguageSwitcherMenuFlyoutItemThemePadding` | 2,9,11,10 | themeresources.xaml:L622(D), L2380(HC), L4538(L) | `<Thickness x:Key="LanguageSwitcherMenuFlyoutItemThemePadding">2,9,11,10</Thickness>` |
| `LanguageSwitcherMenuFlyoutItemThemePaddingNarrow` | 2,4,11,7 | themeresources.xaml:L623(D), L2381(HC), L4539(L) | `<Thickness x:Key="LanguageSwitcherMenuFlyoutItemThemePaddingNarrow">2,4,11,7</Thickness>` |

### 3.21 `ListBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ListBoxBorderThemeThickness` | **Default=0 / HighContrast=2 / Light=0** | themeresources.xaml:L1744(D), L3702(HC), L5660(L) | `<Thickness x:Key="ListBoxBorderThemeThickness">0</Thickness>` |
| `ListBoxItemPadding` | 12,9,12,12 | themeresources.xaml:L6083(N) | `<Thickness x:Key="ListBoxItemPadding">12,9,12,12</Thickness>` |

### 3.22 `ListPickerFlyout*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ListPickerFlyoutPresenterItemMargin` | 0,0,0,19 | themeresources.xaml:L156(D), L2883(HC), L4072(L) | `<Thickness x:Key="ListPickerFlyoutPresenterItemMargin">0,0,0,19</Thickness>` |
| `ListPickerFlyoutPresenterMultiselectCheckBoxMargin` | 0,9.5,0,0 | themeresources.xaml:L155(D), L2882(HC), L4071(L) | `<Thickness x:Key="ListPickerFlyoutPresenterMultiselectCheckBoxMargin">0,9.5,0,0</Thickness>` |

### 3.23 `ListView*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ListViewItemCompactSelectedBorderThemeThickness` | 4 | themeresources.xaml:L1781(D), L3759(HC), L5697(L) | `<Thickness x:Key="ListViewItemCompactSelectedBorderThemeThickness">4</Thickness>` |
| `ListViewItemRevealBorderThemeThickness` | 1 | themeresources.xaml:L1419(D), L3377(HC), L5335(L) | `<Thickness x:Key="ListViewItemRevealBorderThemeThickness">1</Thickness>` |

### 3.24 `MediaTransportControls*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MediaTransportControlsTitleSafeBounds` | 48,0,48,27 | themeresources.xaml:L203(D), L2930(HC), L4119(L) | `<Thickness x:Key="MediaTransportControlsTitleSafeBounds">48,0,48,27</Thickness>` |

### 3.25 `MenuBar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MenuBarItemBorderThickness` | **Default=0 / HighContrast=2 / Light=0** | themeresources.xaml:L1674(D), L3632(HC), L5590(L) | `<Thickness x:Key="MenuBarItemBorderThickness">0</Thickness>` |

### 3.26 `MenuFlyout*` —— 12 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `MenuFlyoutItemCheckGlyphMargin` | 12,11,0,13 | themeresources.xaml:L1549(D), L3533(HC), L5465(L) | `<Thickness x:Key="MenuFlyoutItemCheckGlyphMargin">12,11,0,13</Thickness>` |
| `MenuFlyoutItemChevronMargin` | 24,0,0,0 | themeresources.xaml:L1550(D), L3534(HC), L5466(L) | `<Thickness x:Key="MenuFlyoutItemChevronMargin">24,0,0,0</Thickness>` |
| `MenuFlyoutItemDoublePlaceholderThemeThickness` | 56,0,0,0 | themeresources.xaml:L1896(D), L3854(HC), L5812(L) | `<Thickness x:Key="MenuFlyoutItemDoublePlaceholderThemeThickness">56,0,0,0</Thickness>` |
| `MenuFlyoutItemPlaceholderThemeThickness` | 28,0,0,0 | themeresources.xaml:L1553(D), L3537(HC), L5469(L) | `<Thickness x:Key="MenuFlyoutItemPlaceholderThemeThickness">28,0,0,0</Thickness>` |
| `MenuFlyoutItemRevealBorderThickness` | 1 | themeresources.xaml:L6080(N) | `<Thickness x:Key="MenuFlyoutItemRevealBorderThickness">1</Thickness>` |
| `MenuFlyoutItemThemePadding` | 11,9,11,10 | themeresources.xaml:L1551(D), L3535(HC), L5467(L) | `<Thickness x:Key="MenuFlyoutItemThemePadding">11,9,11,10</Thickness>` |
| `MenuFlyoutItemThemePaddingNarrow` | 11,4,11,7 | themeresources.xaml:L1552(D), L3536(HC), L5468(L) | `<Thickness x:Key="MenuFlyoutItemThemePaddingNarrow">11,4,11,7</Thickness>` |
| `MenuFlyoutPresenterBorderThemeThickness` | 1 | themeresources.xaml:L1918(D), L3876(HC), L5834(L) | `<Thickness x:Key="MenuFlyoutPresenterBorderThemeThickness">1</Thickness>` |
| `MenuFlyoutPresenterThemePadding` | 1 | themeresources.xaml:L1548(D), L3532(HC), L5464(L) | `<Thickness x:Key="MenuFlyoutPresenterThemePadding">1</Thickness>` |
| `MenuFlyoutScrollerMargin` | 0,4,0,4 | themeresources.xaml:L6026(N) | `<Thickness x:Key="MenuFlyoutScrollerMargin">0,4,0,4</Thickness>` |
| `MenuFlyoutSeparatorThemePadding` | 12,4,12,4 | themeresources.xaml:L1554(D), L3538(HC), L5470(L) | `<Thickness x:Key="MenuFlyoutSeparatorThemePadding">12,4,12,4</Thickness>` |
| `MenuFlyoutSubItemRevealBorderThickness` | 1 | themeresources.xaml:L6082(N) | `<Thickness x:Key="MenuFlyoutSubItemRevealBorderThickness">1</Thickness>` |

### 3.27 `NavigationView*` —— 5 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `NavigationViewAutoSuggestBoxMargin` | 10,0,16,0 | themeresources.xaml:L6045(N) | `<Thickness x:Key="NavigationViewAutoSuggestBoxMargin">10,0,16,0</Thickness>` |
| `NavigationViewItemBorderThickness` | 1 | themeresources.xaml:L6050(N) | `<Thickness x:Key="NavigationViewItemBorderThickness">1</Thickness>` |
| `NavigationViewItemIconBoxMargin` | 10,12,16,12 | themeresources.xaml:L6049(N) | `<Thickness x:Key="NavigationViewItemIconBoxMargin">10,12,16,12</Thickness>` |
| `NavigationViewItemInnerHeaderMargin` | 10,0,0,0 | themeresources.xaml:L6044(N) | `<Thickness x:Key="NavigationViewItemInnerHeaderMargin">10,0,0,0</Thickness>` |
| `NavigationViewToggleBorderThickness` | 1 | themeresources.xaml:L6046(N) | `<Thickness x:Key="NavigationViewToggleBorderThickness">1</Thickness>` |

### 3.28 `PasswordBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PasswordBoxLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L6087(N) | `<Thickness x:Key="PasswordBoxLeftHeaderMargin">0,5,32,0</Thickness>` |
| `PasswordBoxTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L6074(N) | `<Thickness x:Key="PasswordBoxTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.29 `PickerFlyout*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PickerFlyoutContentPanelLandscapeThemeMargin` | 19,19,19,0 | themeresources.xaml:L157(D), L2884(HC), L4073(L) | `<Thickness x:Key="PickerFlyoutContentPanelLandscapeThemeMargin">19,19,19,0</Thickness>` |
| `PickerFlyoutContentPanelPortraitThemeMargin` | 19,37,19,0 | themeresources.xaml:L158(D), L2885(HC), L4074(L) | `<Thickness x:Key="PickerFlyoutContentPanelPortraitThemeMargin">19,37,19,0</Thickness>` |
| `PickerFlyoutTitleThemeMargin` | 0,0,0,32.5 | themeresources.xaml:L159(D), L2886(HC), L4075(L) | `<Thickness x:Key="PickerFlyoutTitleThemeMargin">0,0,0,32.5</Thickness>` |

### 3.30 `Pivot*` —— 6 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `PivotHeaderItemMargin` | 12,0,12,0 | themeresources.xaml:L160(D), L2887(HC), L4076(L) | `<Thickness x:Key="PivotHeaderItemMargin">12,0,12,0</Thickness>` |
| `PivotItemMargin` | 12,0,12,0 | themeresources.xaml:L161(D), L2888(HC), L4077(L) | `<Thickness x:Key="PivotItemMargin">12,0,12,0</Thickness>` |
| `PivotLandscapeThemePadding` | 12,14,0,13 | themeresources.xaml:L162(D), L2889(HC), L4078(L) | `<Thickness x:Key="PivotLandscapeThemePadding">12,14,0,13</Thickness>` |
| `PivotNavButtonBorderThemeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L163(D), L2890(HC), L4079(L) | `<Thickness x:Key="PivotNavButtonBorderThemeThickness">0</Thickness>` |
| `PivotNavButtonMargin` | 0,6,0,0 | themeresources.xaml:L164(D), L2891(HC), L4080(L) | `<Thickness x:Key="PivotNavButtonMargin">0,6,0,0</Thickness>` |
| `PivotPortraitThemePadding` | 12,14,0,13 | themeresources.xaml:L165(D), L2892(HC), L4081(L) | `<Thickness x:Key="PivotPortraitThemePadding">12,14,0,13</Thickness>` |

### 3.31 `ProgressBar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ProgressBarBorderThemeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L166(D), L2893(HC), L4082(L) | `<Thickness x:Key="ProgressBarBorderThemeThickness">0</Thickness>` |

### 3.32 `RepeatButton*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `RepeatButtonBorderThemeThickness` | **Default=2 / HighContrast=2 / Light=0** | themeresources.xaml:L167(D), L2894(HC), L4083(L) | `<Thickness x:Key="RepeatButtonBorderThemeThickness">2</Thickness>` |
| `RepeatButtonRevealBorderThemeThickness` | 2 | themeresources.xaml:L1414(D), L3372(HC), L5330(L) | `<Thickness x:Key="RepeatButtonRevealBorderThemeThickness">2</Thickness>` |

### 3.33 `RichEditBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `RichEditBoxLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L6084(N) | `<Thickness x:Key="RichEditBoxLeftHeaderMargin">0,5,32,0</Thickness>` |
| `RichEditBoxTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L6069(N) | `<Thickness x:Key="RichEditBoxTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.34 `ScrollBar*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ScrollBarPanningBorderThemeThickness` | 1 | themeresources.xaml:L168(D), L2895(HC), L4084(L) | `<Thickness x:Key="ScrollBarPanningBorderThemeThickness">1</Thickness>` |

### 3.35 `SearchBox*` —— 7 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SearchBoxBorderThemeThickness` | 2 | themeresources.xaml:L175(D), L2902(HC), L4091(L) | `<Thickness x:Key="SearchBoxBorderThemeThickness">2</Thickness>` |
| `SearchBoxIMECandidateListSeparatorThemeThickness` | 0,2,0,0 | themeresources.xaml:L174(D), L2901(HC), L4090(L) | `<Thickness x:Key="SearchBoxIMECandidateListSeparatorThemeThickness">0,2,0,0</Thickness>` |
| `SearchBoxQuerySuggestionThemeMargin` | 12,11,8,13 | themeresources.xaml:L169(D), L2896(HC), L4085(L) | `<Thickness x:Key="SearchBoxQuerySuggestionThemeMargin">12,11,8,13</Thickness>` |
| `SearchBoxResultSuggestionThemeMargin` | 12,11,8,13 | themeresources.xaml:L170(D), L2897(HC), L4086(L) | `<Thickness x:Key="SearchBoxResultSuggestionThemeMargin">12,11,8,13</Thickness>` |
| `SearchBoxSeparatorSuggestionThemeMargin` | 12,11,8,13 | themeresources.xaml:L171(D), L2898(HC), L4087(L) | `<Thickness x:Key="SearchBoxSeparatorSuggestionThemeMargin">12,11,8,13</Thickness>` |
| `SearchBoxSuggestionSubcomponentThemeMargin` | 0,0,12,0 | themeresources.xaml:L172(D), L2899(HC), L4088(L) | `<Thickness x:Key="SearchBoxSuggestionSubcomponentThemeMargin">0,0,12,0</Thickness>` |
| `SearchBoxThemePadding` | 12,4,8,4 | themeresources.xaml:L173(D), L2900(HC), L4089(L) | `<Thickness x:Key="SearchBoxThemePadding">12,4,8,4</Thickness>` |

### 3.36 `Slider*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SliderBorderThemeThickness` | **Default=0 / HighContrast=1 / Light=0** | themeresources.xaml:L176(D), L2903(HC), L4092(L) | `<Thickness x:Key="SliderBorderThemeThickness">0</Thickness>` |
| `SliderHeaderThemeMargin` | 0,0,0,4 | themeresources.xaml:L177(D), L2904(HC), L4093(L) | `<Thickness x:Key="SliderHeaderThemeMargin">0,0,0,4</Thickness>` |
| `SliderLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L5886(N) | `<Thickness x:Key="SliderLeftHeaderMargin">0,5,32,0</Thickness>` |
| `SliderTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5885(N) | `<Thickness x:Key="SliderTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.37 `SplitButton*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SplitButtonBorderThemeThickness` | 2 | themeresources.xaml:L6079(N) | `<Thickness x:Key="SplitButtonBorderThemeThickness">2</Thickness>` |

### 3.38 `SplitView*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `SplitViewLeftBorderThemeThickness` | 0,0,1,0 | themeresources.xaml:L178(D), L2905(HC), L4094(L) | `<Thickness x:Key="SplitViewLeftBorderThemeThickness">0,0,1,0</Thickness>` |
| `SplitViewRightBorderThemeThickness` | 1,0,0,0 | themeresources.xaml:L179(D), L2906(HC), L4095(L) | `<Thickness x:Key="SplitViewRightBorderThemeThickness">1,0,0,0</Thickness>` |

### 3.39 `TextBox*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TextBoxLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L6089(N) | `<Thickness x:Key="TextBoxLeftHeaderMargin">0,5,32,0</Thickness>` |
| `TextBoxTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L6075(N) | `<Thickness x:Key="TextBoxTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.40 `TextControl*` —— 4 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TextControlBorderThemeThickness` | 2 | themeresources.xaml:L180(D), L2907(HC), L4096(L) | `<Thickness x:Key="TextControlBorderThemeThickness">2</Thickness>` |
| `TextControlMarginThemeThickness` | 0,9.5,0,9.5 | themeresources.xaml:L181(D), L2908(HC), L4097(L) | `<Thickness x:Key="TextControlMarginThemeThickness">0,9.5,0,9.5</Thickness>` |
| `TextControlPlaceholderThemePadding` | 12,5,10,5 | themeresources.xaml:L184(D), L2911(HC), L4100(L) | `<Thickness x:Key="TextControlPlaceholderThemePadding">12,5,10,5</Thickness>` |
| `TextControlThemePadding` | 10,3,6,6 | themeresources.xaml:L182(D), L2909(HC), L4098(L) | `<Thickness x:Key="TextControlThemePadding">10,3,6,6</Thickness>` |

### 3.41 `TimePicker*` —— 6 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TimePickerFirstHostThemeMargin` | 0,0,20,0 | themeresources.xaml:L186(D), L2913(HC), L4102(L) | `<Thickness x:Key="TimePickerFirstHostThemeMargin">0,0,20,0</Thickness>` |
| `TimePickerFlyoutPresenterItemPadding` | 0,3,0,6 | themeresources.xaml:L5892(N) | `<Thickness x:Key="TimePickerFlyoutPresenterItemPadding">0,3,0,6</Thickness>` |
| `TimePickerHeaderThemeMargin` | 0,0,0,4 | themeresources.xaml:L185(D), L2912(HC), L4101(L) | `<Thickness x:Key="TimePickerHeaderThemeMargin">0,0,0,4</Thickness>` |
| `TimePickerLeftHeaderMargin` | 0,5,32,0 | themeresources.xaml:L5880(N) | `<Thickness x:Key="TimePickerLeftHeaderMargin">0,5,32,0</Thickness>` |
| `TimePickerThirdHostThemeMargin` | 20,0,0,0 | themeresources.xaml:L187(D), L2914(HC), L4103(L) | `<Thickness x:Key="TimePickerThirdHostThemeMargin">20,0,0,0</Thickness>` |
| `TimePickerTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5879(N) | `<Thickness x:Key="TimePickerTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.42 `ToggleButton*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToggleButtonBorderThemeThickness` | 2 | themeresources.xaml:L188(D), L2915(HC), L4104(L) | `<Thickness x:Key="ToggleButtonBorderThemeThickness">2</Thickness>` |
| `ToggleButtonRevealBorderThemeThickness` | 2 | themeresources.xaml:L1415(D), L3373(HC), L5331(L) | `<Thickness x:Key="ToggleButtonRevealBorderThemeThickness">2</Thickness>` |

### 3.43 `ToggleMenuFlyoutItem*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToggleMenuFlyoutItemRevealBorderThickness` | 1 | themeresources.xaml:L6081(N) | `<Thickness x:Key="ToggleMenuFlyoutItemRevealBorderThickness">1</Thickness>` |

### 3.44 `ToggleSwitch*` —— 3 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToggleSwitchHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5891(N) | `<Thickness x:Key="ToggleSwitchHeaderMargin">0,0,0,4</Thickness>` |
| `ToggleSwitchLeftHeaderMargin` | 0,6,32,0 | themeresources.xaml:L5890(N) | `<Thickness x:Key="ToggleSwitchLeftHeaderMargin">0,6,32,0</Thickness>` |
| `ToggleSwitchTopHeaderMargin` | 0,0,0,4 | themeresources.xaml:L5889(N) | `<Thickness x:Key="ToggleSwitchTopHeaderMargin">0,0,0,4</Thickness>` |

### 3.45 `ToolTip*` —— 2 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ToolTipBorderThemePadding` | 8,5,8,7 | themeresources.xaml:L726(D), L2533(HC), L4642(L) | `<Thickness x:Key="ToolTipBorderThemePadding">8,5,8,7</Thickness>` |
| `ToolTipBorderThemeThickness` | 1 | themeresources.xaml:L725(D), L2532(HC), L4641(L) | `<Thickness x:Key="ToolTipBorderThemeThickness">1</Thickness>` |

### 3.46 `TreeViewItem*` —— 1 条

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `TreeViewItemBorderThemeThickness` | 1 | themeresources.xaml:L1864(D), L3822(HC), L5780(L) | `<Thickness x:Key="TreeViewItemBorderThemeThickness">1</Thickness>` |

### 3.0 跨主题字典取值不同的 `Thickness` 键（14 个）

| 键名 | 态 | 值 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `AppBarBottomBorderThemeThickness` | Default 暗 | 0,0,0,0 | themeresources.xaml:L111 | `<Thickness x:Key="AppBarBottomBorderThemeThickness">0,0,0,0</Thickness>` |
| `AppBarBottomBorderThemeThickness` | HighContrast | 0,2,0,0 | themeresources.xaml:L2838 | `<Thickness x:Key="AppBarBottomBorderThemeThickness">0,2,0,0</Thickness>` |
| `AppBarBottomBorderThemeThickness` | Light 亮 | 0,0,0,0 | themeresources.xaml:L4027 | `<Thickness x:Key="AppBarBottomBorderThemeThickness">0,0,0,0</Thickness>` |
| `AppBarButtonRevealBorderThemeThickness` | Default 暗 | 1 | themeresources.xaml:L1417 | `<Thickness x:Key="AppBarButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarButtonRevealBorderThemeThickness` | HighContrast | 0 | themeresources.xaml:L3375 | `<Thickness x:Key="AppBarButtonRevealBorderThemeThickness">0</Thickness>` |
| `AppBarButtonRevealBorderThemeThickness` | Light 亮 | 1 | themeresources.xaml:L5333 | `<Thickness x:Key="AppBarButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarEllipsisButtonRevealBorderThemeThickness` | Default 暗 | 1 | themeresources.xaml:L1416 | `<Thickness x:Key="AppBarEllipsisButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarEllipsisButtonRevealBorderThemeThickness` | HighContrast | 0 | themeresources.xaml:L3374 | `<Thickness x:Key="AppBarEllipsisButtonRevealBorderThemeThickness">0</Thickness>` |
| `AppBarEllipsisButtonRevealBorderThemeThickness` | Light 亮 | 1 | themeresources.xaml:L5332 | `<Thickness x:Key="AppBarEllipsisButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarToggleButtonRevealBorderThemeThickness` | Default 暗 | 1 | themeresources.xaml:L1418 | `<Thickness x:Key="AppBarToggleButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarToggleButtonRevealBorderThemeThickness` | HighContrast | 0 | themeresources.xaml:L3376 | `<Thickness x:Key="AppBarToggleButtonRevealBorderThemeThickness">0</Thickness>` |
| `AppBarToggleButtonRevealBorderThemeThickness` | Light 亮 | 1 | themeresources.xaml:L5334 | `<Thickness x:Key="AppBarToggleButtonRevealBorderThemeThickness">1</Thickness>` |
| `AppBarTopBorderThemeThickness` | Default 暗 | 0,0,0,0 | themeresources.xaml:L113 | `<Thickness x:Key="AppBarTopBorderThemeThickness">0,0,0,0</Thickness>` |
| `AppBarTopBorderThemeThickness` | HighContrast | 0,0,0,2 | themeresources.xaml:L2840 | `<Thickness x:Key="AppBarTopBorderThemeThickness">0,0,0,2</Thickness>` |
| `AppBarTopBorderThemeThickness` | Light 亮 | 0,0,0,0 | themeresources.xaml:L4029 | `<Thickness x:Key="AppBarTopBorderThemeThickness">0,0,0,0</Thickness>` |
| `AutoSuggestListViewItemMargin` | Default 暗 | 12,11,0,13 | themeresources.xaml:L119 | `<Thickness x:Key="AutoSuggestListViewItemMargin">12,11,0,13</Thickness>` |
| `AutoSuggestListViewItemMargin` | HighContrast | 10,11,0,13 | themeresources.xaml:L2846 | `<Thickness x:Key="AutoSuggestListViewItemMargin">10,11,0,13</Thickness>` |
| `AutoSuggestListViewItemMargin` | Light 亮 | 12,11,0,13 | themeresources.xaml:L4035 | `<Thickness x:Key="AutoSuggestListViewItemMargin">12,11,0,13</Thickness>` |
| `DateTimeFlyoutButtonBorderThickness` | Default 暗 | 0 | themeresources.xaml:L142 | `<Thickness x:Key="DateTimeFlyoutButtonBorderThickness">0</Thickness>` |
| `DateTimeFlyoutButtonBorderThickness` | HighContrast | 1 | themeresources.xaml:L2869 | `<Thickness x:Key="DateTimeFlyoutButtonBorderThickness">1</Thickness>` |
| `DateTimeFlyoutButtonBorderThickness` | Light 亮 | 0 | themeresources.xaml:L4058 | `<Thickness x:Key="DateTimeFlyoutButtonBorderThickness">0</Thickness>` |
| `FlipViewButtonBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L146 | `<Thickness x:Key="FlipViewButtonBorderThemeThickness">0</Thickness>` |
| `FlipViewButtonBorderThemeThickness` | HighContrast | 1 | themeresources.xaml:L2873 | `<Thickness x:Key="FlipViewButtonBorderThemeThickness">1</Thickness>` |
| `FlipViewButtonBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L4062 | `<Thickness x:Key="FlipViewButtonBorderThemeThickness">0</Thickness>` |
| `ListBoxBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L1744 | `<Thickness x:Key="ListBoxBorderThemeThickness">0</Thickness>` |
| `ListBoxBorderThemeThickness` | HighContrast | 2 | themeresources.xaml:L3702 | `<Thickness x:Key="ListBoxBorderThemeThickness">2</Thickness>` |
| `ListBoxBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L5660 | `<Thickness x:Key="ListBoxBorderThemeThickness">0</Thickness>` |
| `MenuBarItemBorderThickness` | Default 暗 | 0 | themeresources.xaml:L1674 | `<Thickness x:Key="MenuBarItemBorderThickness">0</Thickness>` |
| `MenuBarItemBorderThickness` | HighContrast | 2 | themeresources.xaml:L3632 | `<Thickness x:Key="MenuBarItemBorderThickness">2</Thickness>` |
| `MenuBarItemBorderThickness` | Light 亮 | 0 | themeresources.xaml:L5590 | `<Thickness x:Key="MenuBarItemBorderThickness">0</Thickness>` |
| `PivotNavButtonBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L163 | `<Thickness x:Key="PivotNavButtonBorderThemeThickness">0</Thickness>` |
| `PivotNavButtonBorderThemeThickness` | HighContrast | 1 | themeresources.xaml:L2890 | `<Thickness x:Key="PivotNavButtonBorderThemeThickness">1</Thickness>` |
| `PivotNavButtonBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L4079 | `<Thickness x:Key="PivotNavButtonBorderThemeThickness">0</Thickness>` |
| `ProgressBarBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L166 | `<Thickness x:Key="ProgressBarBorderThemeThickness">0</Thickness>` |
| `ProgressBarBorderThemeThickness` | HighContrast | 1 | themeresources.xaml:L2893 | `<Thickness x:Key="ProgressBarBorderThemeThickness">1</Thickness>` |
| `ProgressBarBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L4082 | `<Thickness x:Key="ProgressBarBorderThemeThickness">0</Thickness>` |
| `RepeatButtonBorderThemeThickness` | Default 暗 | 2 | themeresources.xaml:L167 | `<Thickness x:Key="RepeatButtonBorderThemeThickness">2</Thickness>` |
| `RepeatButtonBorderThemeThickness` | HighContrast | 2 | themeresources.xaml:L2894 | `<Thickness x:Key="RepeatButtonBorderThemeThickness">2</Thickness>` |
| `RepeatButtonBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L4083 | `<Thickness x:Key="RepeatButtonBorderThemeThickness">0</Thickness>` |
| `SliderBorderThemeThickness` | Default 暗 | 0 | themeresources.xaml:L176 | `<Thickness x:Key="SliderBorderThemeThickness">0</Thickness>` |
| `SliderBorderThemeThickness` | HighContrast | 1 | themeresources.xaml:L2903 | `<Thickness x:Key="SliderBorderThemeThickness">1</Thickness>` |
| `SliderBorderThemeThickness` | Light 亮 | 0 | themeresources.xaml:L4092 | `<Thickness x:Key="SliderBorderThemeThickness">0</Thickness>` |

---

## §4 `CornerRadius` / 其它类型全表

### 4.1 `CornerRadius`（全库只有 1 个键）

> 全文件仅此一个 `CornerRadius` 资源。控件圆角在模板里一律走 `{TemplateBinding CornerRadius}`。`ControlCornerRadius` / `OverlayCornerRadius` 在此快照中 **0 次**（见 §1.4）。

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `HyperlinkFocusRectCornerRadius` | 4,4,4,4 | themeresources.xaml:L204(D), L2931(HC), L4120(L) | `<CornerRadius x:Key="HyperlinkFocusRectCornerRadius">4,4,4,4</CornerRadius>` |

### 4.2 `GridLength`

> 三个主题字典各一份同名同值。

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `AppBarExpandButtonThemeWidthGridLength` | 48 | themeresources.xaml:L202(D), L2929(HC), L4118(L) | `<GridLength x:Key="AppBarExpandButtonThemeWidthGridLength">48</GridLength>` |

### 4.3 `x:Int32`

> `PivotHeaderItemCharacterSpacing` = -25（字距，负数收紧）；`ComboBoxPopupMaxNumberOfItems*` 是**个数**上限，不是像素——但直接决定弹出高度，故列入。

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `ComboBoxPopupMaxNumberOfItems` | 15 | themeresources.xaml:L6070(N) | `<x:Int32 x:Key="ComboBoxPopupMaxNumberOfItems">15</x:Int32>` |
| `ComboBoxPopupMaxNumberOfItemsThatCanBeShownOnOneSide` | 7 | themeresources.xaml:L6071(N) | `<x:Int32 x:Key="ComboBoxPopupMaxNumberOfItemsThatCanBeShownOnOneSide">7</x:Int32>` |
| `PivotHeaderItemCharacterSpacing` | -25 | themeresources.xaml:L110(D), L2837(HC), L4026(L) | `<x:Int32 x:Key="PivotHeaderItemCharacterSpacing">-25</x:Int32>` |

### 4.4 `x:Boolean`（几何/绘制开关）

> 这些不是数值，但**直接决定几何是否存在**（是否画勾选、是否用圆角 chrome、阴影走哪种实现），Rust 侧实现对应逻辑时需要同一组默认值。

| 键名 | 值 | 文件:行号 | 原行 |
|---|---|---|---|
| `CalendarViewBaseItemRoundedChromeEnabled` | False | themeresources.xaml:L5899(N) | `<x:Boolean x:Key="CalendarViewBaseItemRoundedChromeEnabled">False</x:Boolean>` |
| `GridViewItemSelectionCheckMarkVisualEnabled` | True | themeresources.xaml:L955(D), L2713(HC), L4871(L) | `<x:Boolean x:Key="GridViewItemSelectionCheckMarkVisualEnabled">True</x:Boolean>` |
| `HyperlinkUnderlineVisible` | True | themeresources.xaml:L5897(N) | `<x:Boolean x:Key="HyperlinkUnderlineVisible">True</x:Boolean>` |
| `IsApplicationFocusVisualKindReveal` | False | themeresources.xaml:L331(D), L2089(HC), L4247(L) | `<x:Boolean x:Key="IsApplicationFocusVisualKindReveal">False</x:Boolean>` |
| `IsDefaultShadowEnabled` | True | themeresources.xaml:L5926(N) | `<x:Boolean x:Key="IsDefaultShadowEnabled">True</x:Boolean>` |
| `ListViewBaseItemRoundedChromeEnabled` | False | themeresources.xaml:L5898(N) | `<x:Boolean x:Key="ListViewBaseItemRoundedChromeEnabled">False</x:Boolean>` |
| `ListViewItemSelectionCheckMarkVisualEnabled` | True | themeresources.xaml:L1800(D), L3747(HC), L5716(L) | `<x:Boolean x:Key="ListViewItemSelectionCheckMarkVisualEnabled">True</x:Boolean>` |
| `ThemeShadowIsUsingDropShadows` | False | themeresources.xaml:L5900(N) | `<x:Boolean x:Key="ThemeShadowIsUsingDropShadows">False</x:Boolean>` |
| `UseSystemFocusVisuals` | True | themeresources.xaml:L1545(D), L3503(HC), L5461(L) | `<x:Boolean x:Key="UseSystemFocusVisuals">True</x:Boolean>` |

### 4.5 `*ThemeAnimation` 全清单（几何相关，含「读不到」的负面证据）

> UWP 的**状态动画**（按压/弹出/拖动）用的是 OS 内置 `*ThemeAnimation` 类型。它们**没有时长属性**，
> 时长与幅度由 OS 预置。本节的用途是**证明哪些动画的几何可以从 XAML 读出、哪些不能**。

#### 4.5.1 出现次数

| 动画类型 | generic.xaml | themeresources.xaml | 合计 | 是否携带几何字面值 |
|---|---|---|---|---|
| `DragItemThemeAnimation` | 4 | 4 | 8 | **否**（只有 TargetName） |
| `DragOverThemeAnimation` | 8 | 8 | 16 | 仅资源/绑定引用，**无字面数值** |
| `DropTargetItemThemeAnimation` | 2 | 2 | 4 | **否**（只有 TargetName） |
| `FadeInThemeAnimation` | 17 | 12 | 29 | **否**（只有 TargetName） |
| `FadeOutThemeAnimation` | 14 | 8 | 22 | **否**（只有 TargetName） |
| `PointerDownThemeAnimation` | 38 | 26 | 64 | **否**（只有 TargetName） |
| `PointerUpThemeAnimation` | 78 | 53 | 131 | **否**（只有 TargetName） |
| `RepositionThemeAnimation` | 5 | 0 | 5 | 仅资源/绑定引用，**无字面数值** |
| `SplitCloseThemeAnimation` | 1 | 0 | 1 | 仅资源/绑定引用，**无字面数值** |
| `SplitOpenThemeAnimation` | 1 | 0 | 1 | 仅资源/绑定引用，**无字面数值** |
| **合计** | **168** | **113** | **281** | — |

#### 4.5.2 全部「不同的属性签名」（去重后只剩这些）

> 把 `Storyboard.TargetName` / `TargetName` 剥掉后，281 个元素只剩下面这些不同形态。
> **这是 CONTROL_SPEC.md §11.4「时长/像素 XAML 读不到」的完整证据**。

| 次数 | 动画 | 除 TargetName 外的属性 | 结论 |
|---|---|---|---|
| 8 | `DragItemThemeAnimation` | `（无）` | 无几何字面值 |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource GridViewItemReorderHintThemeOffset}" Direction="Bottom" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource GridViewItemReorderHintThemeOffset}" Direction="Left" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource GridViewItemReorderHintThemeOffset}" Direction="Right" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource GridViewItemReorderHintThemeOffset}" Direction="Top" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource ListViewItemReorderHintThemeOffset}" Direction="Bottom" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource ListViewItemReorderHintThemeOffset}" Direction="Left" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource ListViewItemReorderHintThemeOffset}" Direction="Right" /` | 几何来自主题资源键（见 §2/§3） |
| 2 | `DragOverThemeAnimation` | `ToOffset="{ThemeResource ListViewItemReorderHintThemeOffset}" Direction="Top" /` | 几何来自主题资源键（见 §2/§3） |
| 4 | `DropTargetItemThemeAnimation` | `（无）` | 无几何字面值 |
| 29 | `FadeInThemeAnimation` | `（无）` | 无几何字面值 |
| 22 | `FadeOutThemeAnimation` | `（无）` | 无几何字面值 |
| 64 | `PointerDownThemeAnimation` | `（无）` | 无几何字面值 |
| 131 | `PointerUpThemeAnimation` | `（无）` | 无几何字面值 |
| 1 | `RepositionThemeAnimation` | `FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.IndicatorLengthDelta}" /` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `RepositionThemeAnimation` | `FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.KnobCurrentToOffOffset}" /` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `RepositionThemeAnimation` | `FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.KnobCurrentToOnOffset}" /` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `RepositionThemeAnimation` | `FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.KnobOffToOnOffset}" /` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `RepositionThemeAnimation` | `FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.KnobOnToOffOffset}" /` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `SplitCloseThemeAnimation` | `OpenedTargetName="PopupBorder" ClosedTargetName="ContentPresenter" OffsetFromCenter="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOffset}…` | 几何来自运行时 `TemplateSettings.*`，非常量 |
| 1 | `SplitOpenThemeAnimation` | `OpenedTargetName="PopupBorder" ClosedTargetName="ContentPresenter" OffsetFromCenter="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOffset}…` | 几何来自运行时 `TemplateSettings.*`，非常量 |

#### 4.5.3 关键结论（可直接写进 Rust 侧注释）

1. `PopupThemeAnimation`（CONTROL_SPEC 为 MenuFlyout 标注的那个）**两个文件各 0 次**。
   它的时长/位移在 XAML 侧完全不存在，只能实测或取 WinUI 2 源码。
2. `PointerDownThemeAnimation`（64 次）/ `PointerUpThemeAnimation`（131 次）**只有 `TargetName` 一个属性**。
   「桌面按压缩小 + 倾斜」的幅度与时长在**任何 XAML 里都没有字面值**。
3. `SplitOpenThemeAnimation` / `SplitCloseThemeAnimation` 各 1 次（ComboBox `DropDownStates`），
   几何来自 `TemplateSettings.DropDownOffset` 与 `TemplateSettings.DropDownOpenedHeight`，**运行时算得**。
4. `RepositionThemeAnimation` 5 次，几何来自 `TemplateSettings.IndicatorLengthDelta`（ProgressBar）
   与 `TemplateSettings.Knob*Offset`（ToggleSwitch）。**这两个控件需要 Rust 侧自己算这四个偏移。**
5. `DragOverThemeAnimation` 16 次，`ToOffset` 指向**主题资源键**（可读出，见 §2）：
   `GridViewItemReorderHintThemeOffset`、`ListViewItemReorderHintThemeOffset`。

#### 4.5.4 `RepositionThemeAnimation` 与 `DragOverThemeAnimation` 逐处行号

| 动画 | 目标 | 几何来源 | 文件:行号 |
|---|---|---|---|
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | generic.xaml |
| `RepositionThemeAnimation` | — | `TemplateSettings.IndicatorLengthDelta` | generic.xaml |
| `RepositionThemeAnimation` | — | `TemplateSettings.KnobCurrentToOnOffset` | generic.xaml |
| `RepositionThemeAnimation` | — | `TemplateSettings.KnobCurrentToOffOffset` | generic.xaml |
| `RepositionThemeAnimation` | — | `TemplateSettings.KnobOnToOffOffset` | generic.xaml |
| `RepositionThemeAnimation` | — | `TemplateSettings.KnobOffToOnOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | generic.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource GridViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | themeresources.xaml |
| `DragOverThemeAnimation` | — | `ThemeResource ListViewItemReorderHintThemeOffset` | themeresources.xaml |

> 逐处行号（`generic.xaml`）：RepositionThemeAnimation → L12095（ProgressBarIndicator）、
> L13009 / L13030 / L13039 / L13048（ToggleSwitch `SwitchKnob`）；
> DragOverThemeAnimation → L10931 / L10938 / L10945 / L10952（GridViewItem）、L28317 / L28324 / L28331 / L28338（ListViewItem），
> `themeresources.xaml` → L7279 / L7284 / L7289 / L7294（GridViewItem）、L11292 / L11297 / L11302 / L11307（ListViewItem）。

### 4.6 附赠：`x:String` 时长类键（48 处 / 16 键，**非尺寸类**，但同为「量」）

> 这些是快照里**唯一以字面值给出的时长**，与 §4.5 的「读不到」形成对照。Rust 侧 ScrollBar / SplitView 动画应对齐这里。

| 键名 | 值 | 文件:行号 |
|---|---|---|
| `HandwritingViewGestureTipsElementEaseInDuration` | `00:00:00.1` | themeresources.xaml:L615(D), L2373(HC), L4531(L) |
| `ScrollBarContractBeginTime` | `00:00:02.00` | themeresources.xaml:L607(D), L2365(HC), L4523(L) |
| `ScrollBarContractDelay` | `00:00:02` | themeresources.xaml:L608(D), L2366(HC), L4524(L) |
| `ScrollBarContractDuration` | `00:00:00.1` | themeresources.xaml:L609(D), L2367(HC), L4525(L) |
| `ScrollBarContractFinalKeyframe` | `00:00:02.1` | themeresources.xaml:L610(D), L2368(HC), L4526(L) |
| `ScrollBarExpandBeginTime` | `00:00:00.40` | themeresources.xaml:L606(D), L2364(HC), L4522(L) |
| `ScrollBarExpandDuration` | `00:00:00.1` | themeresources.xaml:L605(D), L2363(HC), L4521(L) |
| `ScrollViewerSeparatorContractBeginTime` | `00:00:02.00` | themeresources.xaml:L629(D), L2387(HC), L4545(L) |
| `ScrollViewerSeparatorContractDelay` | `00:00:02` | themeresources.xaml:L630(D), L2388(HC), L4546(L) |
| `ScrollViewerSeparatorContractDuration` | `00:00:00.1` | themeresources.xaml:L631(D), L2389(HC), L4547(L) |
| `ScrollViewerSeparatorContractFinalKeyframe` | `00:00:02.1` | themeresources.xaml:L632(D), L2390(HC), L4548(L) |
| `ScrollViewerSeparatorExpandBeginTime` | `00:00:00.40` | themeresources.xaml:L627(D), L2385(HC), L4543(L) |
| `ScrollViewerSeparatorExpandDuration` | `00:00:00.1` | themeresources.xaml:L628(D), L2386(HC), L4544(L) |
| `SplitViewPaneAnimationCloseDuration` | `00:00:00.1` | themeresources.xaml:L1330(D), L3288(HC), L5246(L) |
| `SplitViewPaneAnimationOpenDuration` | `00:00:00.2` | themeresources.xaml:L1328(D), L3286(HC), L5244(L) |
| `SplitViewPaneAnimationOpenPreDuration` | `00:00:00.19999` | themeresources.xaml:L1329(D), L3287(HC), L5245(L) |

---

## §5 `generic.xaml` 写死的几何（按控件分组）

### 5.0 取数说明与统计

**收录范围**：`Width` / `Height` / `MinWidth` / `MinHeight` / `MaxWidth` / `MaxHeight` / `BorderThickness` / `Padding` / `Margin` / `CornerRadius`，
值**不含 `{`**（即不是 `{ThemeResource …}` / `{TemplateBinding …}` / `{Binding …}` / `{StaticResource …}`）。

**三种书写形态**，本节分三张表：

| 形态 | 语法 | 含义 | 本文件小节 |
|---|---|---|---|
| A. 模板/样式级 `Setter` | `<Setter Property="Padding" Value="12,0,12,0"/>` | 控件的**默认值** | A |
| B. 视觉状态内 `Setter` | `<Setter Target="PrimaryButton.Margin" Value="2,0,0,0"/>` | **某个视觉状态**下的覆盖值 | B |
| C. 元素属性 | `<Border Width="20" Height="20"/>` | 模板里的**硬编码布局** | C |

> B 形态**没有 `Property=` 属性**——属性名编码在 `Target="元素.属性"` 里。用 `Property=` 去找会全部漏掉，
> 这是本表相对旧取数的最大增量（多出 242 条）。

| 数据 | generic.xaml 条数 | 写死 | 引用资源 |
|---|---|---|---|
| `Setter` 几何键 | 513 | **346** | 167 |
| 元素属性几何键 | 458 | **458** | 0 |

**控件归属规则**：优先取最近的 `<ControlTemplate TargetType="…">`；若该 Setter 在模板之外（样式级），取最近的 `<Style TargetType="…">`；
两者皆无时归入「（无上下文）」。

> 逐条写死 `Setter` 共 346 条：**A 形态（`Property=`）143 条**、**B 形态（`Target=`）203 条**。
> 按归属层级分：模板级（`ControlTemplate` 内）238 条、样式级（`Style` 内）108 条、无上下文 0 条。
> 元素属性 C 表中：模板级 436 条、样式级 22 条。

> 涉及控件/样式 TargetType 共 **96** 个：`AppBar`、`AppBarButton`、`AppBarSeparator`、`AppBarToggleButton`、`AutoSuggestBox`、`Button`、`ButtonBase`、`CalendarDatePicker`、`CalendarView`、`CalendarViewDayItem`、`CheckBox`、`ColorPicker`、`ColorSpectrum`、`ComboBox`、`ComboBoxItem`、`CommandBar`、`CommandBarFlyoutCommandBar`、`CommandBarOverflowPresenter`、`ContentControl`、`ContentDialog`、`Control`、`DatePicker`、`DatePickerFlyoutPresenter`、`DropDownButton`、`FlipView`、`FlyoutPresenter`、`Grid`、`GridView`、`GridViewHeaderItem`、`GridViewItem`、`HandwritingView`、`Hub`、`HubSection`、`HyperlinkButton`、`InkToolbarBallpointPenButton`、`InkToolbarCustomPenButton`、`InkToolbarCustomToggleButton`、`InkToolbarCustomToolButton`、`InkToolbarEraserButton`、`InkToolbarFlyoutItem`、`InkToolbarHighlighterButton`、`InkToolbarPencilButton`、`InkToolbarPenConfigurationControl`、`InkToolbarRulerButton`、`InkToolbarStencilButton`、`ListBox`、`ListBoxItem`、`ListViewHeaderItem`、`ListViewItem`、`LoopingSelectorItem`、`MediaTransportControls`、`MenuBar`、`MenuBarItem`、`MenuFlyoutItem`、`MenuFlyoutPresenter`、`MenuFlyoutSeparator`、`MenuFlyoutSubItem`、`NavigationView`、`NavigationViewItem`、`NavigationViewItemHeader`、`NavigationViewItemPresenter`、`NavigationViewItemSeparator`、`PasswordBox`、`PersonPicture`、`Pivot`、`PivotHeaderItem`、`PivotItem`、`ProgressBar`、`ProgressRing`、`RadioButton`、`RatingControl`、`Rectangle`、`RefreshVisualizer`、`RepeatButton`、`RichEditBox`、`ScrollBar`、`ScrollViewer`、`SearchBox`、`SemanticZoom`、`SettingsFlyout`、`Slider`、`SplitButton`、`SplitView`、`StackPanel`、`SwipeControl`、`TextBlock`、`TextBox`、`Thumb`、`TimePicker`、`TimePickerFlyoutPresenter`、`ToggleButton`、`ToggleMenuFlyoutItem`、`ToggleSwitch`、`ToolTip`、`TreeViewItem`、`TwoPaneView`

### 5.1 `AppBar`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | ExpandButton | `Padding` | `14,23,14,0` | ButtonStates.CompactClosed | generic.xaml:L11825 | `<Button x:Name="ExpandButton" Foreground="{TemplateBinding Foreground}" Style="{StaticResource EllipsisButton}" Padding="14,23,14,0" MinHeight="{ThemeResource A…` |

### 5.2 `AppBarButton`

条目数：A 模板/样式级 **10**，B 视觉状态 **41**，C 元素属性 **17**，引用资源 **5** · 模板定义始于 L23746。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `68` | 样式级 | generic.xaml:L23685 | `<Setter Property="Width" Value="68" />` |
| `Width` | `Auto` | 样式级 | generic.xaml:L23965 | `<Setter Property="Width" Value="Auto" />` |
| `Width` | `Auto` | 样式级 | generic.xaml:L23970 | `<Setter Property="Width" Value="Auto" />` |
| `Width` | `68` | 样式级 | generic.xaml:L26638 | `<Setter Property="Width" Value="68" />` |
| `MinWidth` | `68` | 样式级 | generic.xaml:L29478 | `<Setter Property="MinWidth" Value="68" />` |
| `Width` | `Auto` | 样式级 | generic.xaml:L29479 | `<Setter Property="Width" Value="Auto" />` |
| `MinHeight` | `40` | 样式级 | generic.xaml:L29480 | `<Setter Property="MinHeight" Value="40" />` |
| `BorderThickness` | `0` | 样式级 | generic.xaml:L29697 | `<Setter Property="BorderThickness" Value="0" />` |
| `Width` | `40` | 样式级 | generic.xaml:L29702 | `<Setter Property="Width" Value="40" />` |
| `Height` | `40` | 样式级 | generic.xaml:L29703 | `<Setter Property="Height" Value="40" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L23746 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L23754 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L23758 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L23763 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L23766 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L23767 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `12,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L23768 | `<Setter Target="ContentViewbox.Margin" Value="12,0,12,0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L23771 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L23776 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L23779 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L23780 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L23781 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L23784 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L26700 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L26708 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L26712 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L26717 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L26720 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L26721 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `12,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L26722 | `<Setter Target="ContentViewbox.Margin" Value="12,0,12,0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L26725 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L26730 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L26733 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L26734 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L26735 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L26738 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L29724 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L29731 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtons** | generic.xaml:L29734 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29739 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29742 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29743 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `12,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29744 | `<Setter Target="ContentViewbox.Margin" Value="12,0,12,0" />` |
| `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29746 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L29751 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L29754 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L29755 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L29756 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | generic.xaml:L29758 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **TouchInputMode** | generic.xaml:L29837 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **GameControllerInputMode** | generic.xaml:L29842 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | Root | `Margin` | `1,0` | .DynamicOverflowDisabled | generic.xaml:L23691 | `<Grid x:Name="Root" MinWidth="{TemplateBinding MinWidth}" MaxWidth="{TemplateBinding MaxWidth}" Background="{TemplateBinding Background}" CornerRadius="{Templat…` |
| `Grid` | ContentRoot | `Margin` | `-1,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L23898 | `<Grid x:Name="ContentRoot" MinHeight="{ThemeResource AppBarThemeMinHeight}" Margin="-1,0">` |
| `TextBlock` | OverflowTextLabel | `Margin` | `12,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L23928 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Foreground="{TemplateBinding Foreground}" FontSize="15" FontFamily="{TemplateBinding FontFa…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L23942 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `Border` | Border | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L23953 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="{TemplateBinding CornerRad…` |
| `TextBlock` | OverflowTextLabel | `Margin` | `12,0,12,0` | .NoFlyout | generic.xaml:L26916 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Foreground="{TemplateBinding Foreground}" FontFamily="{TemplateBinding FontFamily}" TextAli…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .NoFlyout | generic.xaml:L26929 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `FontIcon` | SubItemChevron | `Margin` | `12,0,12,0` | .NoFlyout | generic.xaml:L26940 | `<FontIcon x:Name="SubItemChevron" Grid.Column="2" Glyph="&#xE0E3;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="12" AutomationProperties.Accessi…` |
| `Grid` | ContentRoot | `Margin` | `4,4,4,2` | .PointerOver | generic.xaml:L29506 | `<Grid x:Name="ContentRoot" Margin="4,4,4,2" VerticalAlignment="Center" HorizontalAlignment="Center" >` |
| `Viewbox` | — | `MinWidth` | `16` | .PointerOver | generic.xaml:L29515 | `<Viewbox MaxHeight="16" MinWidth="16">` |
| `Viewbox` | — | `MaxHeight` | `16` | .PointerOver | generic.xaml:L29515 | `<Viewbox MaxHeight="16" MinWidth="16">` |
| `ContentPresenter` | Content | `Margin` | `0,0,0,2` | .PointerOver | generic.xaml:L29516 | `<ContentPresenter x:Name="Content" Margin="0,0,0,2" Content="{TemplateBinding Icon}" />` |
| `Viewbox` | ContentViewbox | `Height` | `16` | .NoFlyout | generic.xaml:L29874 | `<Viewbox x:Name="ContentViewbox" Height="16" HorizontalAlignment="Stretch" AutomationProperties.AccessibilityView="Raw" >` |
| `TextBlock` | OverflowTextLabel | `Padding` | `0,5,0,7` | .NoFlyout | generic.xaml:L29882 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Style="{ThemeResource BodyTextBlockStyle}" Foreground="{TemplateBinding Foreground}" FontFa…` |
| `TextBlock` | OverflowTextLabel | `Margin` | `12,0,12,0` | .NoFlyout | generic.xaml:L29882 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Style="{ThemeResource BodyTextBlockStyle}" Foreground="{TemplateBinding Foreground}" FontFa…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .NoFlyout | generic.xaml:L29896 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `FontIcon` | SubItemChevron | `Margin` | `12,0,12,0` | .NoFlyout | generic.xaml:L29897 | `<FontIcon x:Name="SubItemChevron" Grid.Column="2" Glyph="&#xE76C;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="12" AutomationProperties.Accessi…` |

### 5.3 `AppBarSeparator`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **3**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | SeparatorRectangle | `Margin` | `16,12,15,12` | .Normal | generic.xaml:L6736 | `<Rectangle x:Name="SeparatorRectangle" Width="1" Height="20" Fill="{TemplateBinding Foreground}" Margin="16,12,15,12" VerticalAlignment="Top">` |
| `Rectangle` | SeparatorRectangle | `Height` | `20` | .Normal | generic.xaml:L6736 | `<Rectangle x:Name="SeparatorRectangle" Width="1" Height="20" Fill="{TemplateBinding Foreground}" Margin="16,12,15,12" VerticalAlignment="Top">` |
| `Rectangle` | SeparatorRectangle | `Width` | `1` | .Normal | generic.xaml:L6736 | `<Rectangle x:Name="SeparatorRectangle" Width="1" Height="20" Fill="{TemplateBinding Foreground}" Margin="16,12,15,12" VerticalAlignment="Top">` |

### 5.4 `AppBarToggleButton`

条目数：A 模板/样式级 **6**，B 视觉状态 **22**，C 元素属性 **16**，引用资源 **9** · 模板定义始于 L24042。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `68` | 样式级 | generic.xaml:L23981 | `<Setter Property="Width" Value="68" />` |
| `Width` | `Auto` | 样式级 | generic.xaml:L24369 | `<Setter Property="Width" Value="Auto" />` |
| `Width` | `Auto` | 样式级 | generic.xaml:L24374 | `<Setter Property="Width" Value="Auto" />` |
| `Width` | `68` | 样式级 | generic.xaml:L26980 | `<Setter Property="Width" Value="68" />` |
| `Width` | `40` | 样式级 | generic.xaml:L29929 | `<Setter Property="Width" Value="40" />` |
| `Height` | `40` | 样式级 | generic.xaml:L29930 | `<Setter Property="Height" Value="40" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L24042 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L24052 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.MaxWidth` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L24056 | `<Setter Target="ContentViewbox.MaxWidth" Value="16" />` |
| `ContentViewbox.MaxHeight` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L24057 | `<Setter Target="ContentViewbox.MaxHeight" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L24058 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L24063 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L27042 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L27052 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.MaxWidth` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L27056 | `<Setter Target="ContentViewbox.MaxWidth" Value="16" />` |
| `ContentViewbox.MaxHeight` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L27057 | `<Setter Target="ContentViewbox.MaxHeight" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L27058 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L27063 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | generic.xaml:L29951 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29960 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `ContentViewbox.MaxWidth` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29964 | `<Setter Target="ContentViewbox.MaxWidth" Value="16" />` |
| `ContentViewbox.MaxHeight` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29965 | `<Setter Target="ContentViewbox.MaxHeight" Value="16" />` |
| `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29966 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | generic.xaml:L29970 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **TouchInputMode** | generic.xaml:L30117 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `OverflowCheckGlyph.Margin` | `12,10,12,10` | InputModeStates | **TouchInputMode** | generic.xaml:L30118 | `<Setter Target="OverflowCheckGlyph.Margin" Value="12,10,12,10" />` |
| `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **GameControllerInputMode** | generic.xaml:L30123 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `OverflowCheckGlyph.Margin` | `12,10,12,10` | InputModeStates | **GameControllerInputMode** | generic.xaml:L30124 | `<Setter Target="OverflowCheckGlyph.Margin" Value="12,10,12,10" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | Root | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L23987 | `<Grid x:Name="Root" MinWidth="{TemplateBinding MinWidth}" MaxWidth="{TemplateBinding MaxWidth}" CornerRadius="{TemplateBinding CornerRadius}" Margin="1,0">` |
| `Grid` | ContentRoot | `Margin` | `-1,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24289 | `<Grid x:Name="ContentRoot" MinHeight="{ThemeResource AppBarThemeMinHeight}" Margin="-1,0">` |
| `TextBlock` | OverflowCheckGlyph | `Width` | `14` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24300 | `<TextBlock x:Name="OverflowCheckGlyph" Text="&#xE73E;" Foreground="{ThemeResource AppBarToggleButtonCheckGlyphForeground}" FontFamily="{ThemeResource SymbolThem…` |
| `TextBlock` | OverflowCheckGlyph | `Height` | `14` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24300 | `<TextBlock x:Name="OverflowCheckGlyph" Text="&#xE73E;" Foreground="{ThemeResource AppBarToggleButtonCheckGlyphForeground}" FontFamily="{ThemeResource SymbolThem…` |
| `TextBlock` | OverflowTextLabel | `Margin` | `38,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24332 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Foreground="{TemplateBinding Foreground}" FontSize="15" FontFamily="{TemplateBinding FontFa…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24346 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `Border` | Border | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L24357 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="{TemplateBinding CornerRad…` |
| `TextBlock` | OverflowCheckGlyph | `Width` | `14` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L27300 | `<TextBlock x:Name="OverflowCheckGlyph" Text="&#xE73E;" Foreground="{ThemeResource AppBarToggleButtonCheckGlyphForeground}" FontFamily="{ThemeResource SymbolThem…` |
| `TextBlock` | OverflowCheckGlyph | `Height` | `14` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L27300 | `<TextBlock x:Name="OverflowCheckGlyph" Text="&#xE73E;" Foreground="{ThemeResource AppBarToggleButtonCheckGlyphForeground}" FontFamily="{ThemeResource SymbolThem…` |
| `TextBlock` | OverflowTextLabel | `Margin` | `38,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L27332 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Foreground="{TemplateBinding Foreground}" FontFamily="{TemplateBinding FontFamily}" TextAli…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L27345 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `TextBlock` | OverflowCheckGlyph | `Margin` | `12,4,12,4` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L30147 | `<TextBlock x:Name="OverflowCheckGlyph" Text="&#xE0E7;" Foreground="{ThemeResource SystemControlForegroundBaseHighBrush}" FontFamily="{ThemeResource SymbolThemeF…` |
| `Viewbox` | ContentViewbox | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L30158 | `<Viewbox x:Name="ContentViewbox" Height="16" HorizontalAlignment="Stretch" AutomationProperties.AccessibilityView="Raw" >` |
| `TextBlock` | OverflowTextLabel | `Padding` | `0,5,0,7` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L30166 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Style="{ThemeResource BodyTextBlockStyle}" Foreground="{TemplateBinding Foreground}" FontFa…` |
| `TextBlock` | OverflowTextLabel | `Margin` | `38,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L30166 | `<TextBlock x:Name="OverflowTextLabel" Text="{TemplateBinding Label}" Style="{ThemeResource BodyTextBlockStyle}" Foreground="{TemplateBinding Foreground}" FontFa…` |
| `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L30180 | `<TextBlock x:Name="KeyboardAcceleratorTextLabel" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |

### 5.5 `AutoSuggestBox`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `TextBox` | TextBox | `Margin` | `0` | .Portrait | generic.xaml:L29438 | `<TextBox x:Name="TextBox" Style="{TemplateBinding TextBoxStyle}" PlaceholderText="{TemplateBinding PlaceholderText}" Header="{TemplateBinding Header}" Width="{T…` |

### 5.6 `Button`

条目数：A 模板/样式级 **13**，B 视觉状态 **0**，C 元素属性 **18**，引用资源 **17** · 模板定义始于 L8182。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 模板级 | generic.xaml:L8189 | `<Setter Property="Padding" Value="0" />` |
| `Margin` | `0` | 模板级 | generic.xaml:L8190 | `<Setter Property="Margin" Value="0" />` |
| `BorderThickness` | `0` | 样式级 | generic.xaml:L11306 | `<Setter Property="BorderThickness" Value="0" />` |
| `Padding` | `0,0,9,0` | 样式级 | generic.xaml:L11307 | `<Setter Property="Padding" Value="0,0,9,0" />` |
| `Height` | `32` | 样式级 | generic.xaml:L15324 | `<Setter Property="Height" Value="32" />` |
| `Width` | `32` | 样式级 | generic.xaml:L15325 | `<Setter Property="Width" Value="32" />` |
| `Padding` | `0,0,9,0` | 样式级 | generic.xaml:L22744 | `<Setter Property="Padding" Value="0,0,9,0" />` |
| `Padding` | `0` | 样式级 | generic.xaml:L25581 | `<Setter Property="Padding" Value="0" />` |
| `MinHeight` | `40` | 样式级 | generic.xaml:L25703 | `<Setter Property="MinHeight" Value="40" />` |
| `Padding` | `12,0` | 样式级 | generic.xaml:L25714 | `<Setter Property="Padding" Value="12,0" />` |
| `Padding` | `0` | 样式级 | generic.xaml:L30248 | `<Setter Property="Padding" Value="0" />` |
| `Width` | `40` | 样式级 | generic.xaml:L30249 | `<Setter Property="Width" Value="40" />` |
| `Height` | `40` | 样式级 | generic.xaml:L30250 | `<Setter Property="Height" Value="40" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | HandwritingViewGestureIconButton | `Padding` | `0` | ButtonStates.Enabled | generic.xaml:L8212 | `<Button x:Name="HandwritingViewGestureIconButton" Style="{StaticResource ButtonWithoutVisualStates}" Background="{ThemeResource HandwritingviewGestureTipsBackgr…` |
| `Button` | HandwritingViewGestureIconButton | `BorderThickness` | `0` | ButtonStates.Enabled | generic.xaml:L8212 | `<Button x:Name="HandwritingViewGestureIconButton" Style="{StaticResource ButtonWithoutVisualStates}" Background="{ThemeResource HandwritingviewGestureTipsBackgr…` |
| `Button` | HandwritingViewGestureIconButton | `Margin` | `0` | ButtonStates.Enabled | generic.xaml:L8212 | `<Button x:Name="HandwritingViewGestureIconButton" Style="{StaticResource ButtonWithoutVisualStates}" Background="{ThemeResource HandwritingviewGestureTipsBackgr…` |
| `Button` | HandwritingViewGestureIconButton | `Width` | `84` | ButtonStates.Enabled | generic.xaml:L8212 | `<Button x:Name="HandwritingViewGestureIconButton" Style="{StaticResource ButtonWithoutVisualStates}" Background="{ThemeResource HandwritingviewGestureTipsBackgr…` |
| `Button` | HandwritingViewGestureIconButton | `Height` | `84` | ButtonStates.Enabled | generic.xaml:L8212 | `<Button x:Name="HandwritingViewGestureIconButton" Style="{StaticResource ButtonWithoutVisualStates}" Background="{ThemeResource HandwritingviewGestureTipsBackgr…` |
| `TextBlock` | HandwritingViewGestureIconTextBlock | `Margin` | `0,8,0,0` | ButtonStates.Enabled | generic.xaml:L8225 | `<TextBlock x:Name="HandwritingViewGestureIconTextBlock" FontFamily="Segoe UI Regular" HorizontalAlignment="Center" Grid.Row="1" FontSize="14" Visibility="Visibl…` |
| `TextBlock` | HandwritingViewGestureIconTextBlock | `Padding` | `0` | ButtonStates.Enabled | generic.xaml:L8225 | `<TextBlock x:Name="HandwritingViewGestureIconTextBlock" FontFamily="Segoe UI Regular" HorizontalAlignment="Center" Grid.Row="1" FontSize="14" Visibility="Visibl…` |
| `ContentPresenter` | ContentPresenter | `BorderThickness` | `2` | ButtonStates.PointerFocused | generic.xaml:L9998 | `<ContentPresenter x:Name="ContentPresenter" BorderBrush="{ThemeResource DatePickerButtonBorderBrush}" Background="{ThemeResource DatePickerButtonBackground}" Bo…` |
| `ContentPresenter` | ContentPresenter | `BorderThickness` | `2` | ButtonStates.PointerFocused | generic.xaml:L11940 | `<ContentPresenter x:Name="ContentPresenter" BorderBrush="{ThemeResource TimePickerButtonBorderBrush}" Background="{ThemeResource TimePickerButtonBackground}" Bo…` |
| `Grid` | RootGrid | `Width` | `32` | ButtonStates.Normal | generic.xaml:L13749 | `<Grid x:Name="RootGrid" Height="32" Width="32">` |
| `Grid` | RootGrid | `Height` | `32` | ButtonStates.Normal | generic.xaml:L13749 | `<Grid x:Name="RootGrid" Height="32" Width="32">` |
| `TextBlock` | NormalGlyph | `Margin` | `6,6,6,6` | ButtonStates.Normal | generic.xaml:L13750 | `<TextBlock x:Name="NormalGlyph" FontWeight="SemiLight" FontFamily="{TemplateBinding FontFamily}" FontSize="{TemplateBinding FontSize}" Text="&#xE0D5;" Margin="6…` |
| `TextBlock` | ChevronTextBlock | `Margin` | `6,0,0,0` | ButtonStates.SecondaryButtonRight | generic.xaml:L22026 | `<TextBlock x:Name="ChevronTextBlock" Grid.Column="1" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="12" Text="&#xE70D;" VerticalAlignment="Center"…` |
| `Grid` | Root | `Margin` | `1,0,0,0` | .Normal | generic.xaml:L22758 | `<Grid x:Name="Root" Margin="1,0,0,0" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding Bor…` |
| `ContentPresenter` | ContentPresenter | `Margin` | `-2,-1,-1,-1` | .Normal | generic.xaml:L22825 | `<ContentPresenter x:Name="ContentPresenter" ContentTransitions="{TemplateBinding ContentTransitions}" ContentTemplate="{TemplateBinding ContentTemplate}" Margin…` |
| `Viewbox` | IconHost | `Height` | `16` | .Normal | generic.xaml:L25663 | `<Viewbox x:Name="IconHost" Width="16" Height="16" HorizontalAlignment="{TemplateBinding HorizontalContentAlignment}" VerticalAlignment="{TemplateBinding Vertica…` |
| `Viewbox` | IconHost | `Width` | `16` | .Normal | generic.xaml:L25663 | `<Viewbox x:Name="IconHost" Width="16" Height="16" HorizontalAlignment="{TemplateBinding HorizontalContentAlignment}" VerticalAlignment="{TemplateBinding Vertica…` |
| `FontIcon` | Icon | `Margin` | `8,0,0,-4` | .Normal | generic.xaml:L25770 | `<FontIcon x:Name="Icon" Margin="8,0,0,-4" VerticalAlignment="Center" AutomationProperties.AccessibilityView="Raw" FontFamily="{ThemeResource SymbolThemeFontFami…` |

### 5.7 `ButtonBase`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MinWidth` | `0` | 样式级 | generic.xaml:L15173 | `<Setter Property="MinWidth" Value="0" />` |
| `MinHeight` | `0` | 样式级 | generic.xaml:L15174 | `<Setter Property="MinHeight" Value="0" />` |

### 5.8 `CalendarDatePicker`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **3**，引用资源 **3** · 模板定义始于 L16767。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 模板级 | generic.xaml:L16779 | `<Setter Property="Padding" Value="0" />` |
| `BorderThickness` | `0` | 模板级 | generic.xaml:L16780 | `<Setter Property="BorderThickness" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `Width` | `32` | ButtonStates.TopHeader | generic.xaml:L16822 | `<ColumnDefinition Width="32" />` |
| `Border` | Background | `MinHeight` | `32` | ButtonStates.TopHeader | generic.xaml:L16835 | `<Border x:Name="Background" Grid.Row="1" Grid.Column="1" Grid.ColumnSpan="2" BorderThickness="{TemplateBinding BorderThickness}" BorderBrush="{TemplateBinding B…` |
| `TextBlock` | DateText | `Padding` | `12, 0, 0, 2` | ButtonStates.TopHeader | generic.xaml:L16845 | `<TextBlock x:Name="DateText" Grid.Row="1" Grid.Column="1" HorizontalAlignment="Left" Foreground="{ThemeResource CalendarDatePickerTextForeground}" Text="{Templa…` |

### 5.9 `CalendarView`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **21**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `1` | 样式级 | generic.xaml:L16204 | `<Setter Property="BorderThickness" Value="1" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `RowDefinition` | — | `Height` | `40` | ButtonStates.Month | generic.xaml:L16536 | `<RowDefinition Height="40" />` |
| `ColumnDefinition` | — | `Width` | `5*` | ButtonStates.Month | generic.xaml:L16542 | `<ColumnDefinition Width="5*" />` |
| `Button` | HeaderButton | `Padding` | `12,0,0,0` | ButtonStates.Month | generic.xaml:L16546 | `<Button x:Name="HeaderButton" Content="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.HeaderText}" IsEnabled="{Binding Relative…` |
| `Button` | PreviousButton | `Padding` | `1` | ButtonStates.Month | generic.xaml:L16553 | `<Button x:Name="PreviousButton" Grid.Column="1" Content="&#xE0E4;" FontFamily="{ThemeResource SymbolThemeFontFamily}" IsTabStop="True" Padding="1" Foreground="{…` |
| `Button` | NextButton | `Padding` | `1` | ButtonStates.Month | generic.xaml:L16563 | `<Button x:Name="NextButton" Grid.Column="2" Content="&#xE0E5;" FontFamily="{ThemeResource SymbolThemeFontFamily}" IsTabStop="True" Padding="1" Foreground="{Temp…` |
| `RowDefinition` | — | `Height` | `38` | ButtonStates.Month | generic.xaml:L16585 | `<RowDefinition Height="38" />` |
| `StackPanel` | InkToolbarEraserButtonFlyoutContent | `Margin` | `0,8,0,8` | ButtonStates.PointerFocused | generic.xaml:L18549 | `<StackPanel x:Name="InkToolbarEraserButtonFlyoutContent" Margin="0,8,0,8">` |
| `TextBlock` | StrokeEraserIcon | `Margin` | `12,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18559 | `<TextBlock x:Name="StrokeEraserIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xF128;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlig…` |
| `TextBlock` | StrokeEraserName | `Margin` | `0,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18562 | `<TextBlock x:Name="StrokeEraserName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12…` |
| `TextBlock` | SmallEraserIcon | `Margin` | `12,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18577 | `<TextBlock x:Name="SmallEraserIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xF129;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlign…` |
| `TextBlock` | SmallEraserName | `Margin` | `0,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18580 | `<TextBlock x:Name="SmallEraserName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12,…` |
| `TextBlock` | LargeEraserIcon | `Margin` | `12,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18595 | `<TextBlock x:Name="LargeEraserIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xF12A;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlign…` |
| `TextBlock` | LargeEraserName | `Margin` | `0,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18598 | `<TextBlock x:Name="LargeEraserName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12,…` |
| `TextBlock` | ClearAllIcon | `Margin` | `12,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18613 | `<TextBlock x:Name="ClearAllIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xE74D;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlignmen…` |
| `TextBlock` | ClearAllName | `Margin` | `0,0,12,0` | ButtonStates.PointerFocused | generic.xaml:L18616 | `<TextBlock x:Name="ClearAllName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12,0"/…` |
| `StackPanel` | InkToolbarStencilButtonFlyoutContent | `Margin` | `0,8,0,8` | ButtonStates.BottomDirection | generic.xaml:L19212 | `<StackPanel x:Name="InkToolbarStencilButtonFlyoutContent" Margin="0,8,0,8">` |
| `TextBlock` | RulerIcon | `Margin` | `12,0,12,0` | ButtonStates.BottomDirection | generic.xaml:L19223 | `<TextBlock x:Name="RulerIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xECC6;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlignment="…` |
| `TextBlock` | RulerName | `Margin` | `0,0,12,0` | ButtonStates.BottomDirection | generic.xaml:L19226 | `<TextBlock x:Name="RulerName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12,0"/>` |
| `TextBlock` | ProtractorIcon | `Margin` | `12,0,12,0` | ButtonStates.BottomDirection | generic.xaml:L19242 | `<TextBlock x:Name="ProtractorIcon" Style="{StaticResource InkToolbarGlyphFontStyle}" Text="&#xF0B4;" Grid.Column="0" HorizontalAlignment="Center" VerticalAlignm…` |
| `TextBlock` | ProtractorName | `Margin` | `0,0,12,0` | ButtonStates.BottomDirection | generic.xaml:L19245 | `<TextBlock x:Name="ProtractorName" Style="{StaticResource InkToolbarFlyoutItemContentTextStyle}" Text="" TextAlignment="Center" Grid.Column="1" Margin="0,0,12,0…` |
| `Border` | — | `Width` | `1` | ButtonStates.Unfocused | generic.xaml:L19626 | `<Border Height="{ThemeResource ListPickerFlyoutFooterThemeHeight}" Width="1" />` |

### 5.10 `CalendarViewDayItem`

条目数：A 模板/样式级 **4**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MinWidth` | `40` | 样式级 | generic.xaml:L16138 | `<Setter Property="MinWidth" Value="40" />` |
| `MinHeight` | `40` | 样式级 | generic.xaml:L16139 | `<Setter Property="MinHeight" Value="40" />` |
| `Margin` | `1` | 样式级 | generic.xaml:L16140 | `<Setter Property="Margin" Value="1" />` |
| `Padding` | `0, 0, 0, 4` | 样式级 | generic.xaml:L16141 | `<Setter Property="Padding" Value="0, 0, 0, 4" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | Root | `Width` | `0` | ButtonStates.RepeatNoneState | generic.xaml:L16150 | `<Grid x:Name="Root" Width="0">` |

### 5.11 `CheckBox`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **8**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `8,5,0,0` | 样式级 | generic.xaml:L7001 | `<Setter Property="Padding" Value="8,5,0,0" />` |
| `MinWidth` | `120` | 样式级 | generic.xaml:L7008 | `<Setter Property="MinWidth" Value="120" />` |
| `MinHeight` | `32` | 样式级 | generic.xaml:L7009 | `<Setter Property="MinHeight" Value="32" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `Width` | `20` | .Indeterminate | generic.xaml:L7361 | `<ColumnDefinition Width="20" />` |
| `Grid` | — | `Height` | `32` | .Indeterminate | generic.xaml:L7365 | `<Grid VerticalAlignment="Top" Height="32">` |
| `Rectangle` | NormalRectangle | `Width` | `20` | .Indeterminate | generic.xaml:L7366 | `<Rectangle x:Name="NormalRectangle" Fill="{ThemeResource CheckBoxCheckBackgroundFillUnchecked}" Stroke="{ThemeResource CheckBoxCheckBackgroundStrokeUnchecked}" …` |
| `Rectangle` | NormalRectangle | `Height` | `20` | .Indeterminate | generic.xaml:L7366 | `<Rectangle x:Name="NormalRectangle" Fill="{ThemeResource CheckBoxCheckBackgroundFillUnchecked}" Stroke="{ThemeResource CheckBoxCheckBackgroundStrokeUnchecked}" …` |
| `Grid` | RootGrid | `Width` | `32` | ButtonStates.NotDragging | generic.xaml:L21269 | `<Grid x:Name="RootGrid" Width="32" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding Borde…` |
| `Grid` | — | `Height` | `32` | ButtonStates.NotDragging | generic.xaml:L21457 | `<Grid VerticalAlignment="Stretch" Height="32">` |
| `Rectangle` | NormalRectangle | `Width` | `20` | ButtonStates.NotDragging | generic.xaml:L21458 | `<Rectangle x:Name="NormalRectangle" VerticalAlignment="Center" HorizontalAlignment="Center" Fill="{ThemeResource CheckBoxCheckBackgroundFillUnchecked}" Stroke="…` |
| `Rectangle` | NormalRectangle | `Height` | `20` | ButtonStates.NotDragging | generic.xaml:L21458 | `<Rectangle x:Name="NormalRectangle" VerticalAlignment="Center" HorizontalAlignment="Center" Fill="{ThemeResource CheckBoxCheckBackgroundFillUnchecked}" Stroke="…` |

### 5.12 `ColorPicker`

条目数：A 模板/样式级 **2**，B 视觉状态 **3**，C 元素属性 **42**，引用资源 **0** · 模板定义始于 L20418。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MaxWidth` | `392` | 样式级 | generic.xaml:L20309 | `<Setter Property="MaxWidth" Value="392" />` |
| `MinWidth` | `312` | 样式级 | generic.xaml:L20310 | `<Setter Property="MinWidth" Value="312" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ColorPreviewRectangleGrid.Width` | `NaN` | ColorSpectrumVisibility | **ColorSpectrumCollapsed** | generic.xaml:L20418 | `<Setter Target="ColorPreviewRectangleGrid.Width" Value="NaN" />` |
| `ColorPreviewRectangleGrid.Height` | `44` | ColorSpectrumVisibility | **ColorSpectrumCollapsed** | generic.xaml:L20419 | `<Setter Target="ColorPreviewRectangleGrid.Height" Value="44" />` |
| `ColorPreviewRectangleGrid.Margin` | `0` | ColorSpectrumVisibility | **ColorSpectrumCollapsed** | generic.xaml:L20420 | `<Setter Target="ColorPreviewRectangleGrid.Margin" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColorSpectrum` | ColorSpectrum | `MinWidth` | `256` | ButtonStates.AlphaDisabled | generic.xaml:L20559 | `<ColorSpectrum x:Name="ColorSpectrum" Grid.Column="0" Grid.Row="0" MaxWidth="336" MaxHeight="336" MinWidth="256" MinHeight="256" MinHue="{TemplateBinding MinHue…` |
| `ColorSpectrum` | ColorSpectrum | `MinHeight` | `256` | ButtonStates.AlphaDisabled | generic.xaml:L20559 | `<ColorSpectrum x:Name="ColorSpectrum" Grid.Column="0" Grid.Row="0" MaxWidth="336" MaxHeight="336" MinWidth="256" MinHeight="256" MinHue="{TemplateBinding MinHue…` |
| `ColorSpectrum` | ColorSpectrum | `MaxWidth` | `336` | ButtonStates.AlphaDisabled | generic.xaml:L20559 | `<ColorSpectrum x:Name="ColorSpectrum" Grid.Column="0" Grid.Row="0" MaxWidth="336" MaxHeight="336" MinWidth="256" MinHeight="256" MinHue="{TemplateBinding MinHue…` |
| `ColorSpectrum` | ColorSpectrum | `MaxHeight` | `336` | ButtonStates.AlphaDisabled | generic.xaml:L20559 | `<ColorSpectrum x:Name="ColorSpectrum" Grid.Column="0" Grid.Row="0" MaxWidth="336" MaxHeight="336" MinWidth="256" MinHeight="256" MinHue="{TemplateBinding MinHue…` |
| `Grid` | ColorPreviewRectangleGrid | `Margin` | `12,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20560 | `<Grid x:Name="ColorPreviewRectangleGrid" Grid.Column="1" Grid.Row="0" Width="44" Margin="12,0,0,0">` |
| `Grid` | ColorPreviewRectangleGrid | `Width` | `44` | ButtonStates.AlphaDisabled | generic.xaml:L20560 | `<Grid x:Name="ColorPreviewRectangleGrid" Grid.Column="1" Grid.Row="0" Width="44" Margin="12,0,0,0">` |
| `Grid` | ThirdDimensionSliderGrid | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20580 | `<Grid Margin="0,12,0,0" x:Name="ThirdDimensionSliderGrid">` |
| `Rectangle` | — | `Height` | `11` | ButtonStates.AlphaDisabled | generic.xaml:L20581 | `<Rectangle Height="11" VerticalAlignment="Center">` |
| `Grid` | AlphaSliderGrid | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20588 | `<Grid Margin="0,12,0,0" x:Name="AlphaSliderGrid">` |
| `Rectangle` | — | `Height` | `11` | ButtonStates.AlphaDisabled | generic.xaml:L20589 | `<Rectangle Height="11" VerticalAlignment="Center">` |
| `Rectangle` | AlphaSliderBackgroundRectangle | `Height` | `11` | ButtonStates.AlphaDisabled | generic.xaml:L20594 | `<Rectangle x:Name="AlphaSliderBackgroundRectangle" Height="11" VerticalAlignment="Center">` |
| `ToggleButton` | MoreButton | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20601 | `<ToggleButton x:Name="MoreButton" Height="32" MinWidth="120" Margin="0,12,0,0" Padding="0" HorizontalAlignment="Right" HorizontalContentAlignment="Right">` |
| `ToggleButton` | MoreButton | `Padding` | `0` | ButtonStates.AlphaDisabled | generic.xaml:L20601 | `<ToggleButton x:Name="MoreButton" Height="32" MinWidth="120" Margin="0,12,0,0" Padding="0" HorizontalAlignment="Right" HorizontalContentAlignment="Right">` |
| `ToggleButton` | MoreButton | `Height` | `32` | ButtonStates.AlphaDisabled | generic.xaml:L20601 | `<ToggleButton x:Name="MoreButton" Height="32" MinWidth="120" Margin="0,12,0,0" Padding="0" HorizontalAlignment="Right" HorizontalContentAlignment="Right">` |
| `ToggleButton` | MoreButton | `MinWidth` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20601 | `<ToggleButton x:Name="MoreButton" Height="32" MinWidth="120" Margin="0,12,0,0" Padding="0" HorizontalAlignment="Right" HorizontalContentAlignment="Right">` |
| `StackPanel` | — | `Margin` | `0,5,0,7` | ButtonStates.AlphaDisabled | generic.xaml:L20610 | `<StackPanel Orientation="Horizontal" HorizontalAlignment="Right" Margin="0,5,0,7">` |
| `FontIcon` | MoreGlyph | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20612 | `<FontIcon x:Name="MoreGlyph" Margin="8,0,0,0" FontFamily="{ThemeResource SymbolThemeFontFamily}" Glyph="&#xE70D;" FontSize="12" />` |
| `ComboBox` | ColorRepresentationComboBox | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20715 | `<ComboBox Grid.Row="0" x:Name="ColorRepresentationComboBox" Width="120" Margin="0,12,0,0">` |
| `ComboBox` | ColorRepresentationComboBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20715 | `<ComboBox Grid.Row="0" x:Name="ColorRepresentationComboBox" Width="120" Margin="0,12,0,0">` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20722 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | RedTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20723 | `<TextBox x:Name="RedTextBox" Width="120" MaxLength="3" Text="255" />` |
| `TextBlock` | RedLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20724 | `<TextBlock x:Name="RedLabel" Text="Red" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20726 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | GreenTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20727 | `<TextBox x:Name="GreenTextBox" Width="120" MaxLength="3" Text="255" />` |
| `TextBlock` | GreenLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20728 | `<TextBlock x:Name="GreenLabel" Text="Green" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20730 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | BlueTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20731 | `<TextBox x:Name="BlueTextBox" Width="120" MaxLength="3" Text="255" />` |
| `TextBlock` | BlueLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20732 | `<TextBlock x:Name="BlueLabel" Text="Blue" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20736 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | HueTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20737 | `<TextBox x:Name="HueTextBox" Width="120" MaxLength="3" Text="0" />` |
| `TextBlock` | HueLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20738 | `<TextBlock x:Name="HueLabel" Text="Hue" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20740 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | SaturationTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20741 | `<TextBox x:Name="SaturationTextBox" Width="120" MaxLength="3" Text="0" />` |
| `TextBlock` | SaturationLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20742 | `<TextBlock x:Name="SaturationLabel" Text="Saturation" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | — | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20744 | `<StackPanel Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | ValueTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20745 | `<TextBox x:Name="ValueTextBox" Width="120" MaxLength="3" Text="100" />` |
| `TextBlock` | ValueLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20746 | `<TextBlock x:Name="ValueLabel" Text="Value" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `StackPanel` | AlphaPanel | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20750 | `<StackPanel x:Name="AlphaPanel" Orientation="Horizontal" Margin="0,12,0,0">` |
| `TextBox` | AlphaTextBox | `Width` | `120` | ButtonStates.AlphaDisabled | generic.xaml:L20751 | `<TextBox x:Name="AlphaTextBox" Width="120" MaxLength="4" Text="100%" />` |
| `TextBlock` | AlphaLabel | `Margin` | `8,0,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20752 | `<TextBlock x:Name="AlphaLabel" Text="Opacity" VerticalAlignment="Center" Margin="8,0,0,0" />` |
| `TextBox` | HexTextBox | `Width` | `132` | ButtonStates.AlphaDisabled | generic.xaml:L20755 | `<TextBox x:Name="HexTextBox" Grid.Column="1" MaxLength="7" Text="#FFFFFF" Margin="0,12,0,0" Width="132" HorizontalAlignment="Right" VerticalAlignment="Top" />` |
| `TextBox` | HexTextBox | `Margin` | `0,12,0,0` | ButtonStates.AlphaDisabled | generic.xaml:L20755 | `<TextBox x:Name="HexTextBox" Grid.Column="1" MaxLength="7" Text="#FFFFFF" Margin="0,12,0,0" Width="132" HorizontalAlignment="Right" VerticalAlignment="Top" />` |

### 5.13 `ColorSpectrum`

条目数：A 模板/样式级 **0**，B 视觉状态 **2**，C 元素属性 **6**，引用资源 **0** · 模板定义始于 L20785。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `SelectionEllipsePanel.Width` | `48` | CommonStates | **PressedLarge** | generic.xaml:L20785 | `<Setter Target="SelectionEllipsePanel.Width" Value="48" />` |
| `SelectionEllipsePanel.Height` | `48` | CommonStates | **PressedLarge** | generic.xaml:L20786 | `<Setter Target="SelectionEllipsePanel.Height" Value="48" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Ellipse` | SpectrumEllipse | `Margin` | `0,0,-1,-1` | ButtonStates.PointerFocused | generic.xaml:L20833 | `<Ellipse x:Name="SpectrumEllipse" IsHitTestVisible="False" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Visibility="Collapsed" Margin="0,0,-1,-1" /…` |
| `Ellipse` | SpectrumOverlayEllipse | `Margin` | `0,0,-1,-1` | ButtonStates.PointerFocused | generic.xaml:L20834 | `<Ellipse x:Name="SpectrumOverlayEllipse" IsHitTestVisible="False" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Visibility="Collapsed" Margin="0,0,-…` |
| `Grid` | SelectionEllipsePanel | `Height` | `16` | ButtonStates.PointerFocused | generic.xaml:L20836 | `<Grid x:Name="SelectionEllipsePanel" Width="16" Height="16">` |
| `Grid` | SelectionEllipsePanel | `Width` | `16` | ButtonStates.PointerFocused | generic.xaml:L20836 | `<Grid x:Name="SelectionEllipsePanel" Width="16" Height="16">` |
| `Ellipse` | FocusEllipse | `Margin` | `-2` | ButtonStates.PointerFocused | generic.xaml:L20837 | `<Ellipse x:Name="FocusEllipse" Stroke="{ThemeResource SystemControlBackgroundChromeBlackHighBrush}" Margin="-2" StrokeThickness="2" IsHitTestVisible="False" Hor…` |
| `Ellipse` | EllipseBorder | `Margin` | `-0.5,-0.5,-1.5,-1.5` | ButtonStates.PointerFocused | generic.xaml:L20847 | `<Ellipse x:Name="EllipseBorder" Style="{StaticResource ColorPickerBorderStyle}" IsHitTestVisible="False" Visibility="Collapsed" HorizontalAlignment="Stretch" Ve…` |

### 5.14 `ComboBox`

条目数：A 模板/样式级 **1**，B 视觉状态 **3**，C 元素属性 **7**，引用资源 **1** · 模板定义始于 L10315。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `12,5,0,7` | 样式级 | generic.xaml:L10132 | `<Setter Property="Padding" Value="12,5,0,7" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `DropDownOverlay.Margin` | `0,3,2,2` | EditableModeStates | **TextBoxFocused** | generic.xaml:L10315 | `<Setter Target="DropDownOverlay.Margin" Value="0,3,2,2" />` |
| `DropDownOverlay.Margin` | `0,3,2,2` | EditableModeStates | **TextBoxFocusedOverlayPointerOver** | generic.xaml:L10322 | `<Setter Target="DropDownOverlay.Margin" Value="0,3,2,2" />` |
| `DropDownOverlay.Margin` | `0,3,2,2` | EditableModeStates | **TextBoxFocusedOverlayPressed** | generic.xaml:L10329 | `<Setter Target="DropDownOverlay.Margin" Value="0,3,2,2" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `Width` | `32` | ButtonStates.TextBoxUnfocused | generic.xaml:L10356 | `<ColumnDefinition Width="32" />` |
| `TextBox` | EditableText | `Padding` | `10,3,30,5` | ButtonStates.TextBoxUnfocused | generic.xaml:L10400 | `<TextBox x:Name="EditableText" Grid.Row="1" Grid.Column="0" Grid.ColumnSpan="2" Style="{StaticResource ComboBoxTextBoxStyle}" Margin="0,0,0,0" Padding="10,3,30,…` |
| `TextBox` | EditableText | `Margin` | `0,0,0,0` | ButtonStates.TextBoxUnfocused | generic.xaml:L10400 | `<TextBox x:Name="EditableText" Grid.Row="1" Grid.Column="0" Grid.ColumnSpan="2" Style="{StaticResource ComboBoxTextBoxStyle}" Margin="0,0,0,0" Padding="10,3,30,…` |
| `Border` | DropDownOverlay | `Width` | `30` | ButtonStates.TextBoxUnfocused | generic.xaml:L10417 | `<Border x:Name="DropDownOverlay" Grid.Row="1" Grid.Column="1" Background="Transparent" Margin="0,2,2,2" Visibility="Collapsed" Width="30" HorizontalAlignment="R…` |
| `Border` | DropDownOverlay | `Margin` | `0,2,2,2` | ButtonStates.TextBoxUnfocused | generic.xaml:L10417 | `<Border x:Name="DropDownOverlay" Grid.Row="1" Grid.Column="1" Background="Transparent" Margin="0,2,2,2" Visibility="Collapsed" Width="30" HorizontalAlignment="R…` |
| `FontIcon` | DropDownGlyph | `Margin` | `0,10,10,10` | ButtonStates.TextBoxUnfocused | generic.xaml:L10426 | `<FontIcon x:Name="DropDownGlyph" Grid.Row="1" Grid.Column="1" IsHitTestVisible="False" Margin="0,10,10,10" Foreground="{ThemeResource ComboBoxDropDownGlyphForeg…` |
| `Border` | PopupBorder | `Margin` | `0,-1,0,-1` | ButtonStates.TextBoxUnfocused | generic.xaml:L10446 | `<Border x:Name="PopupBorder" Background="{ThemeResource ComboBoxDropDownBackground}" BackgroundSizing="OuterBorderEdge" BorderBrush="{ThemeResource ComboBoxDrop…` |

### 5.15 `ComboBoxItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **4** · 模板定义始于 L24696。

### 5.16 `CommandBar`

条目数：A 模板/样式级 **2**，B 视觉状态 **4**，C 元素属性 **0**，引用资源 **0** · 模板定义始于 L23536。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `NaN` | 模板级 | generic.xaml:L23658 | `<Setter Property="Width" Value="NaN" />` |
| `Width` | `NaN` | 模板级 | generic.xaml:L31029 | `<Setter Property="Width" Value="NaN" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ContentControlColumnDefinition.Width` | `Auto` | DynamicOverflowStates | **DynamicOverflowEnabled** | generic.xaml:L23536 | `<Setter Target="ContentControlColumnDefinition.Width" Value="Auto" />` |
| `PrimaryItemsControlColumnDefinition.Width` | `*` | DynamicOverflowStates | **DynamicOverflowEnabled** | generic.xaml:L23537 | `<Setter Target="PrimaryItemsControlColumnDefinition.Width" Value="*" />` |
| `ContentControlColumnDefinition.Width` | `Auto` | DynamicOverflowStates | **DynamicOverflowEnabled** | generic.xaml:L30929 | `<Setter Target="ContentControlColumnDefinition.Width" Value="Auto" />` |
| `PrimaryItemsControlColumnDefinition.Width` | `*` | DynamicOverflowStates | **DynamicOverflowEnabled** | generic.xaml:L30930 | `<Setter Target="PrimaryItemsControlColumnDefinition.Width" Value="*" />` |

### 5.17 `CommandBarFlyoutCommandBar`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **5** · 模板定义始于 L22308。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MaxWidth` | `440` | 样式级 | generic.xaml:L22078 | `<Setter Property="MaxWidth" Value="440" />` |
| `Height` | `40` | 样式级 | generic.xaml:L22079 | `<Setter Property="Height" Value="40" />` |
| `Width` | `NaN` | 模板级 | generic.xaml:L22430 | `<Setter Property="Width" Value="NaN" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ItemsControl` | PrimaryItemsControl | `Height` | `40` | .Default | generic.xaml:L22361 | `<ItemsControl x:Name="PrimaryItemsControl" Height="40" Grid.Column="0" IsTabStop="False">` |

### 5.18 `CommandBarOverflowPresenter`

条目数：A 模板/样式级 **4**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **6** · 模板定义始于 L27741。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 样式级 | generic.xaml:L30196 | `<Setter Property="BorderThickness" Value="0" />` |
| `MinWidth` | `136` | 样式级 | generic.xaml:L30197 | `<Setter Property="MinWidth" Value="136" />` |
| `MaxWidth` | `440` | 样式级 | generic.xaml:L30198 | `<Setter Property="MaxWidth" Value="440" />` |
| `MaxHeight` | `480` | 样式级 | generic.xaml:L30199 | `<Setter Property="MaxHeight" Value="480" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ItemsPresenter` | ItemsPresenter | `Margin` | `0,4,0,4` | .FullWidthOpenUp | generic.xaml:L30234 | `<ItemsPresenter x:Name="ItemsPresenter" Margin="0,4,0,4" />` |

### 5.19 `ContentControl`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `12,5,0,11` | 样式级 | generic.xaml:L26612 | `<Setter Property="Margin" Value="12,5,0,11" />` |

### 5.20 `ContentDialog`

条目数：A 模板/样式级 **0**，B 视觉状态 **7**，C 元素属性 **5**，引用资源 **0** · 模板定义始于 L9709。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `PrimaryButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **PrimaryVisible** | generic.xaml:L9709 | `<Setter Target="PrimaryButton.Margin" Value="2,0,0,0" />` |
| `SecondaryButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **SecondaryVisible** | generic.xaml:L9718 | `<Setter Target="SecondaryButton.Margin" Value="2,0,0,0" />` |
| `CloseButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **CloseVisible** | generic.xaml:L9727 | `<Setter Target="CloseButton.Margin" Value="2,0,0,0" />` |
| `SecondaryButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **PrimaryAndSecondaryVisible** | generic.xaml:L9737 | `<Setter Target="SecondaryButton.Margin" Value="2,0,0,0" />` |
| `CloseButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **PrimaryAndCloseVisible** | generic.xaml:L9746 | `<Setter Target="CloseButton.Margin" Value="2,0,0,0" />` |
| `SecondaryButton.Margin` | `0,0,2,0` | ButtonsVisibilityStates | **SecondaryAndCloseVisible** | generic.xaml:L9754 | `<Setter Target="SecondaryButton.Margin" Value="0,0,2,0" />` |
| `CloseButton.Margin` | `2,0,0,0` | ButtonsVisibilityStates | **SecondaryAndCloseVisible** | generic.xaml:L9757 | `<Setter Target="CloseButton.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `Width` | `0.5*` | ButtonStates.NoBorder | generic.xaml:L9869 | `<ColumnDefinition Width="0.5*" />` |
| `ColumnDefinition` | — | `Width` | `0.5*` | ButtonStates.NoBorder | generic.xaml:L9870 | `<ColumnDefinition Width="0.5*" />` |
| `Button` | PrimaryButton | `Margin` | `0,0,2,0` | ButtonStates.NoBorder | generic.xaml:L9873 | `<Button x:Name="PrimaryButton" Content="{TemplateBinding PrimaryButtonText}" IsEnabled="{TemplateBinding IsPrimaryButtonEnabled}" Style="{TemplateBinding Primar…` |
| `Button` | SecondaryButton | `Margin` | `2,0,2,0` | ButtonStates.NoBorder | generic.xaml:L9882 | `<Button x:Name="SecondaryButton" Content="{TemplateBinding SecondaryButtonText}" IsEnabled="{TemplateBinding IsSecondaryButtonEnabled}" Style="{TemplateBinding …` |
| `Button` | CloseButton | `Margin` | `2,0,0,0` | ButtonStates.NoBorder | generic.xaml:L9892 | `<Button x:Name="CloseButton" Content="{TemplateBinding CloseButtonText}" Style="{TemplateBinding CloseButtonStyle}" ElementSoundMode="FocusOnly" HorizontalAlign…` |

### 5.21 `Control`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **4**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 样式级 | generic.xaml:L8583 | `<Setter Property="BorderThickness" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `RepeatButton` | UpButton | `Padding` | `0` | ButtonStates.Normal | generic.xaml:L15016 | `<RepeatButton x:Name="UpButton" Content="&#xE70E;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="8" Height="22" Padding="0" HorizontalAlignment="…` |
| `RepeatButton` | UpButton | `Height` | `22` | ButtonStates.Normal | generic.xaml:L15016 | `<RepeatButton x:Name="UpButton" Content="&#xE70E;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="8" Height="22" Padding="0" HorizontalAlignment="…` |
| `RepeatButton` | DownButton | `Padding` | `0` | ButtonStates.Normal | generic.xaml:L15028 | `<RepeatButton x:Name="DownButton" Content="&#xE70D;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="8" Height="22" Padding="0" HorizontalAlignment…` |
| `RepeatButton` | DownButton | `Height` | `22` | ButtonStates.Normal | generic.xaml:L15028 | `<RepeatButton x:Name="DownButton" Content="&#xE70D;" FontFamily="{ThemeResource SymbolThemeFontFamily}" FontSize="8" Height="22" Padding="0" HorizontalAlignment…` |

### 5.22 `DatePicker`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **6**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | DayColumn | `Width` | `78*` | ButtonStates.HasDate | generic.xaml:L10079 | `<ColumnDefinition Width="78*" x:Name="DayColumn" />` |
| `ColumnDefinition` | MonthColumn | `Width` | `132*` | ButtonStates.HasDate | generic.xaml:L10081 | `<ColumnDefinition Width="132*" x:Name="MonthColumn" />` |
| `ColumnDefinition` | YearColumn | `Width` | `78*` | ButtonStates.HasDate | generic.xaml:L10083 | `<ColumnDefinition Width="78*" x:Name="YearColumn" />` |
| `TextBlock` | MonthTextBlock | `Margin` | `1,0,0,0` | ButtonStates.HasDate | generic.xaml:L10093 | `<TextBlock x:Name="MonthTextBlock" Text="Month" TextAlignment="Left" Padding="{ThemeResource DatePickerFlyoutPresenterMonthPadding}" Margin="1,0,0,0" FontFamily…` |
| `Rectangle` | FirstPickerSpacing | `Width` | `2` | ButtonStates.HasDate | generic.xaml:L10110 | `<Rectangle x:Name="FirstPickerSpacing" Fill="{ThemeResource DatePickerSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="1" />` |
| `Rectangle` | SecondPickerSpacing | `Width` | `2` | ButtonStates.HasDate | generic.xaml:L10115 | `<Rectangle x:Name="SecondPickerSpacing" Fill="{ThemeResource DatePickerSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="3" />` |

### 5.23 `DatePickerFlyoutPresenter`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **7**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `296` | 样式级 | generic.xaml:L14673 | `<Setter Property="Width" Value="296" />` |
| `MinWidth` | `296` | 样式级 | generic.xaml:L14674 | `<Setter Property="MinWidth" Value="296" />` |
| `MaxHeight` | `398` | 样式级 | generic.xaml:L14675 | `<Setter Property="MaxHeight" Value="398" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Background | `MaxHeight` | `398` | ButtonStates.Normal | generic.xaml:L14687 | `<Border x:Name="Background" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickn…` |
| `ColumnDefinition` | DayColumn | `Width` | `78*` | ButtonStates.Normal | generic.xaml:L14703 | `<ColumnDefinition Width="78*" x:Name="DayColumn" />` |
| `ColumnDefinition` | MonthColumn | `Width` | `132*` | ButtonStates.Normal | generic.xaml:L14705 | `<ColumnDefinition Width="132*" x:Name="MonthColumn" />` |
| `ColumnDefinition` | YearColumn | `Width` | `78*` | ButtonStates.Normal | generic.xaml:L14707 | `<ColumnDefinition Width="78*" x:Name="YearColumn" />` |
| `Rectangle` | FirstPickerSpacing | `Width` | `2` | ButtonStates.Normal | generic.xaml:L14715 | `<Rectangle x:Name="FirstPickerSpacing" Fill="{ThemeResource DatePickerFlyoutPresenterSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="1" />` |
| `Rectangle` | SecondPickerSpacing | `Width` | `2` | ButtonStates.Normal | generic.xaml:L14720 | `<Rectangle x:Name="SecondPickerSpacing" Fill="{ThemeResource DatePickerFlyoutPresenterSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="3" />` |
| `Rectangle` | — | `Height` | `2` | ButtonStates.Normal | generic.xaml:L14732 | `<Rectangle Height="2" VerticalAlignment="Top" Fill="{ThemeResource DatePickerFlyoutPresenterSpacerFill}" Grid.ColumnSpan="2" />` |

### 5.24 `DropDownButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **2**。

### 5.25 `FlipView`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **8**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 样式级 | generic.xaml:L13295 | `<Setter Property="BorderThickness" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | PreviousButtonHorizontal | `Height` | `36` | ButtonStates.Normal | generic.xaml:L13531 | `<Button x:Name="PreviousButtonHorizontal" Template="{StaticResource HorizontalPreviousTemplate}" Width="20" Height="36" IsTabStop="False" UseSystemFocusVisuals=…` |
| `Button` | PreviousButtonHorizontal | `Width` | `20` | ButtonStates.Normal | generic.xaml:L13531 | `<Button x:Name="PreviousButtonHorizontal" Template="{StaticResource HorizontalPreviousTemplate}" Width="20" Height="36" IsTabStop="False" UseSystemFocusVisuals=…` |
| `Button` | NextButtonHorizontal | `Height` | `36` | ButtonStates.Normal | generic.xaml:L13539 | `<Button x:Name="NextButtonHorizontal" Template="{StaticResource HorizontalNextTemplate}" Width="20" Height="36" IsTabStop="False" UseSystemFocusVisuals="False" …` |
| `Button` | NextButtonHorizontal | `Width` | `20` | ButtonStates.Normal | generic.xaml:L13539 | `<Button x:Name="NextButtonHorizontal" Template="{StaticResource HorizontalNextTemplate}" Width="20" Height="36" IsTabStop="False" UseSystemFocusVisuals="False" …` |
| `Button` | PreviousButtonVertical | `Height` | `20` | ButtonStates.Normal | generic.xaml:L13547 | `<Button x:Name="PreviousButtonVertical" Template="{StaticResource VerticalPreviousTemplate}" Width="36" Height="20" IsTabStop="False" UseSystemFocusVisuals="Fal…` |
| `Button` | PreviousButtonVertical | `Width` | `36` | ButtonStates.Normal | generic.xaml:L13547 | `<Button x:Name="PreviousButtonVertical" Template="{StaticResource VerticalPreviousTemplate}" Width="36" Height="20" IsTabStop="False" UseSystemFocusVisuals="Fal…` |
| `Button` | NextButtonVertical | `Height` | `20` | ButtonStates.Normal | generic.xaml:L13555 | `<Button x:Name="NextButtonVertical" Template="{StaticResource VerticalNextTemplate}" Width="36" Height="20" IsTabStop="False" UseSystemFocusVisuals="False" Hori…` |
| `Button` | NextButtonVertical | `Width` | `36` | ButtonStates.Normal | generic.xaml:L13555 | `<Button x:Name="NextButtonVertical" Template="{StaticResource VerticalNextTemplate}" Width="36" Height="20" IsTabStop="False" UseSystemFocusVisuals="False" Hori…` |

### 5.26 `FlyoutPresenter`

条目数：A 模板/样式级 **4**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **6**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0,0,0,0` | 样式级 | generic.xaml:L17616 | `<Setter Property="Padding" Value="0,0,0,0" />` |
| `BorderThickness` | `1` | 样式级 | generic.xaml:L17619 | `<Setter Property="BorderThickness" Value="1" />` |
| `MinWidth` | `0` | 样式级 | generic.xaml:L17620 | `<Setter Property="MinWidth" Value="0" />` |
| `MinHeight` | `0` | 样式级 | generic.xaml:L17621 | `<Setter Property="MinHeight" Value="0" />` |

### 5.27 `Grid`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `1` | 样式级 | generic.xaml:L8643 | `<Setter Property="BorderThickness" Value="1" />` |

### 5.28 `GridView`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0,0,0,10` | 样式级 | generic.xaml:L10546 | `<Setter Property="Padding" Value="0,0,0,10" />` |

### 5.29 `GridViewHeaderItem`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0,0,0,4` | 样式级 | generic.xaml:L10615 | `<Setter Property="Margin" Value="0,0,0,4" />` |
| `Padding` | `12,8,12,0` | 样式级 | generic.xaml:L10616 | `<Setter Property="Padding" Value="12,8,12,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | — | `Margin` | `12,8,12,0` | ButtonStates.TextBoxUnfocused | generic.xaml:L10632 | `<Rectangle Stroke="{ThemeResource GridViewHeaderItemDividerStroke}" StrokeThickness="0.5" Height="1" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Ma…` |
| `Rectangle` | — | `Height` | `1` | ButtonStates.TextBoxUnfocused | generic.xaml:L10632 | `<Rectangle Stroke="{ThemeResource GridViewHeaderItemDividerStroke}" StrokeThickness="0.5" Height="1" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Ma…` |

### 5.30 `GridViewItem`

条目数：A 模板/样式级 **2**，B 视觉状态 **5**，C 元素属性 **7**，引用资源 **4** · 模板定义始于 L19551。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0,0,4,4` | 样式级 | generic.xaml:L10688 | `<Setter Property="Margin" Value="0,0,4,4" />` |
| `Margin` | `0,0,4,4` | 样式级 | generic.xaml:L24494 | `<Setter Property="Margin" Value="0,0,4,4" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ItemContent.Margin` | `-2,-2,-2,-2` | CommonStates | **PointerOver** | generic.xaml:L19551 | `<Setter Target="ItemContent.Margin" Value="-2,-2,-2,-2" />` |
| `ItemBorder.Margin` | `2,2,2,2` | CommonStates | **PointerOver** | generic.xaml:L19552 | `<Setter Target="ItemBorder.Margin" Value="2,2,2,2" />` |
| `ItemContent.Margin` | `-2,-2,-2,-2` | CommonStates | **Pressed** | generic.xaml:L19557 | `<Setter Target="ItemContent.Margin" Value="-2,-2,-2,-2" />` |
| `ItemBorder.Margin` | `2,2,2,2` | CommonStates | **Pressed** | generic.xaml:L19559 | `<Setter Target="ItemBorder.Margin" Value="2,2,2,2" />` |
| `ItemContent.Margin` | `2,2,2,2` | CommonStates | **Selected** | generic.xaml:L19564 | `<Setter Target="ItemContent.Margin" Value="2,2,2,2" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | MultiSelectSquare | `Margin` | `0,2,2,0` | ButtonStates.DraggedPlaceholder | generic.xaml:L11113 | `<Border x:Name="MultiSelectSquare" Background="{ThemeResource SystemControlBackgroundChromeMediumBrush}" Width="20" Height="20" Margin="0,2,2,0" VerticalAlignme…` |
| `Border` | MultiSelectSquare | `Height` | `20` | ButtonStates.DraggedPlaceholder | generic.xaml:L11113 | `<Border x:Name="MultiSelectSquare" Background="{ThemeResource SystemControlBackgroundChromeMediumBrush}" Width="20" Height="20" Margin="0,2,2,0" VerticalAlignme…` |
| `Border` | MultiSelectSquare | `Width` | `20` | ButtonStates.DraggedPlaceholder | generic.xaml:L11113 | `<Border x:Name="MultiSelectSquare" Background="{ThemeResource SystemControlBackgroundChromeMediumBrush}" Width="20" Height="20" Margin="0,2,2,0" VerticalAlignme…` |
| `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | ButtonStates.DraggedPlaceholder | generic.xaml:L11128 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | ButtonStates.DraggedPlaceholder | generic.xaml:L11128 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | ButtonStates.DraggedPlaceholder | generic.xaml:L11128 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Ellipse` | ItemBorder | `Margin` | `6,6,6,6` | ButtonStates.Unfocused | generic.xaml:L19574 | `<Ellipse x:Name="ItemBorder" Margin="6,6,6,6" UseLayoutRounding="false" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Fill="Transparent" Stroke="Tra…` |

### 5.31 `HandwritingView`

条目数：A 模板/样式级 **15**，B 视觉状态 **0**，C 元素属性 **7**，引用资源 **4** · 模板定义始于 L8019。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 模板级 | generic.xaml:L8019 | `<Setter Property="Padding" Value="0" />` |
| `Margin` | `0` | 模板级 | generic.xaml:L8020 | `<Setter Property="Margin" Value="0" />` |
| `Width` | `40` | 模板级 | generic.xaml:L8021 | `<Setter Property="Width" Value="40" />` |
| `Height` | `40` | 模板级 | generic.xaml:L8022 | `<Setter Property="Height" Value="40" />` |
| `BorderThickness` | `0` | 模板级 | generic.xaml:L8026 | `<Setter Property="BorderThickness" Value="0" />` |
| `Width` | `16` | 模板级 | generic.xaml:L8032 | `<Setter Property="Width" Value="16" />` |
| `Height` | `20` | 模板级 | generic.xaml:L8033 | `<Setter Property="Height" Value="20" />` |
| `Width` | `25` | 模板级 | generic.xaml:L8036 | `<Setter Property="Width" Value="25" />` |
| `Height` | `20` | 模板级 | generic.xaml:L8037 | `<Setter Property="Height" Value="20" />` |
| `MaxWidth` | `480` | 模板级 | generic.xaml:L8140 | `<Setter Property="MaxWidth" Value="480" />` |
| `Padding` | `0` | 模板级 | generic.xaml:L8142 | `<Setter Property="Padding" Value="0" />` |
| `Margin` | `0` | 模板级 | generic.xaml:L8284 | `<Setter Property="Margin" Value="0" />` |
| `Margin` | `0` | 模板级 | generic.xaml:L8289 | `<Setter Property="Margin" Value="0" />` |
| `Padding` | `0` | 模板级 | generic.xaml:L8294 | `<Setter Property="Padding" Value="0" />` |
| `Width` | `92` | 模板级 | generic.xaml:L8295 | `<Setter Property="Width" Value="92" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | — | `Height` | `63` | ButtonStates.Enabled | generic.xaml:L8238 | `<Grid Grid.Row="0" Height="63">` |
| `TextBlock` | HandwritingViewTipsTitle | `Margin` | `14,18,0,0` | ButtonStates.Enabled | generic.xaml:L8243 | `<TextBlock x:Name="HandwritingViewTipsTitle" Grid.Column="0" Margin="14,18,0,0" FontFamily="Segoe UI Regular" FontSize="14" Visibility="Visible" FontWeight="Sem…` |
| `Button` | HandwritingViewTipsHelpButton | `Padding` | `0` | ButtonStates.Enabled | generic.xaml:L8252 | `<Button x:Name="HandwritingViewTipsHelpButton" Grid.Column="1" Margin="0,18,22,0" HorizontalAlignment="Right" VerticalAlignment="Top" FontSize="20" Background="…` |
| `Button` | HandwritingViewTipsHelpButton | `BorderThickness` | `0` | ButtonStates.Enabled | generic.xaml:L8252 | `<Button x:Name="HandwritingViewTipsHelpButton" Grid.Column="1" Margin="0,18,22,0" HorizontalAlignment="Right" VerticalAlignment="Top" FontSize="20" Background="…` |
| `Button` | HandwritingViewTipsHelpButton | `Margin` | `0,18,22,0` | ButtonStates.Enabled | generic.xaml:L8252 | `<Button x:Name="HandwritingViewTipsHelpButton" Grid.Column="1" Margin="0,18,22,0" HorizontalAlignment="Right" VerticalAlignment="Top" FontSize="20" Background="…` |
| `ScrollViewer` | — | `Padding` | `6,0,6,0` | ButtonStates.Enabled | generic.xaml:L8264 | `<ScrollViewer HorizontalAlignment="Stretch" VerticalAlignment="Stretch" HorizontalScrollBarVisibility="Auto" HorizontalScrollMode="Auto" VerticalScrollBarVisibi…` |
| `ScrollViewer` | — | `Height` | `137` | ButtonStates.Enabled | generic.xaml:L8264 | `<ScrollViewer HorizontalAlignment="Stretch" VerticalAlignment="Stretch" HorizontalScrollBarVisibility="Auto" HorizontalScrollMode="Auto" VerticalScrollBarVisibi…` |

### 5.32 `Hub`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `12,12,12,0` | 样式级 | generic.xaml:L13875 | `<Setter Property="Padding" Value="12,12,12,0" />` |

### 5.33 `HubSection`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `12,10,12,0` | 样式级 | generic.xaml:L13956 | `<Setter Property="Padding" Value="12,10,12,0" />` |

### 5.34 `HyperlinkButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **2**。

### 5.35 `InkToolbarBallpointPenButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L17758。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L17758 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17766 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17767 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17770 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17778 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17779 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17782 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L17790 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L17791 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L17794 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L17802 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L17803 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L17806 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L17847 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.36 `InkToolbarCustomPenButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L18369。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L18369 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18377 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18378 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18381 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18389 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18390 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18393 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18401 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18402 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18405 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18413 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18414 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18417 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L18435 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.37 `InkToolbarCustomToggleButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **4**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L19175。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L19175 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L19176 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L19183 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L19184 | `<Setter Target="SelectionAccent.Width" Value="2" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L19197 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.38 `InkToolbarCustomToolButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L18900。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L18900 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18908 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18909 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18912 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18920 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18921 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18924 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18932 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18933 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18936 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18944 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18945 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18948 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L18966 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.39 `InkToolbarEraserButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L18716。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L18716 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18724 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18725 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18728 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18736 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18737 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18740 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18748 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18749 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18752 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18760 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18761 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18764 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L18796 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.40 `InkToolbarFlyoutItem`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 样式级 | generic.xaml:L18457 | `<Setter Property="BorderThickness" Value="0" />` |
| `MinWidth` | `136` | 样式级 | generic.xaml:L18460 | `<Setter Property="MinWidth" Value="136" />` |
| `Height` | `48` | 样式级 | generic.xaml:L18461 | `<Setter Property="Height" Value="48" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `BorderThickness` | `0` | ButtonStates.PointerFocused | generic.xaml:L18537 | `<Border x:Name="Border" BorderBrush="Transparent" BorderThickness="0" Background="Transparent">` |

### 5.41 `InkToolbarHighlighterButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L17965。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L17965 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17973 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17974 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L17977 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17985 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17986 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L17989 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L17997 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L17998 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18001 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18009 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18010 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18013 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L18054 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.42 `InkToolbarPencilButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L18172。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L18172 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18180 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18181 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L18184 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18192 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18193 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L18196 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18204 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18205 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L18208 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18216 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18217 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L18220 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L18261 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.43 `InkToolbarPenConfigurationControl`

条目数：A 模板/样式级 **6**，B 视觉状态 **0**，C 元素属性 **15**，引用资源 **0** · 模板定义始于 L19529。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0` | 模板级 | generic.xaml:L19529 | `<Setter Property="Margin" Value="0" />` |
| `Padding` | `0` | 模板级 | generic.xaml:L19530 | `<Setter Property="Padding" Value="0" />` |
| `MinHeight` | `52` | 模板级 | generic.xaml:L19531 | `<Setter Property="MinHeight" Value="52" />` |
| `MinWidth` | `52` | 模板级 | generic.xaml:L19532 | `<Setter Property="MinWidth" Value="52" />` |
| `Height` | `52` | 模板级 | generic.xaml:L19533 | `<Setter Property="Height" Value="52" />` |
| `Width` | `52` | 模板级 | generic.xaml:L19534 | `<Setter Property="Width" Value="52" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | RootElement | `MinWidth` | `320` | ButtonStates.BottomDirection | generic.xaml:L19481 | `<Grid x:Name="RootElement" MinWidth="320">` |
| `RowDefinition` | — | `Height` | `8` | ButtonStates.BottomDirection | generic.xaml:L19493 | `<RowDefinition Height="8" />` |
| `RowDefinition` | — | `Height` | `12` | ButtonStates.BottomDirection | generic.xaml:L19495 | `<RowDefinition Height="12" />` |
| `RowDefinition` | — | `Height` | `12` | ButtonStates.BottomDirection | generic.xaml:L19497 | `<RowDefinition Height="12" />` |
| `RowDefinition` | — | `Height` | `12` | ButtonStates.BottomDirection | generic.xaml:L19501 | `<RowDefinition Height="12" />` |
| `TextBlock` | PenColorPaletteTitle | `Padding` | `12,0,12,0` | ButtonStates.BottomDirection | generic.xaml:L19507 | `<TextBlock x:Name="PenColorPaletteTitle" Grid.Row="1" Grid.Column="0" Padding="12,0,12,0" Style="{ThemeResource BodyTextBlockStyle}" HighContrastAdjustment="Non…` |
| `GridView` | PenColorPalette | `Padding` | `4,0,4,2` | ButtonStates.BottomDirection | generic.xaml:L19511 | `<GridView x:Name="PenColorPalette" Grid.Row="3" Grid.Column="0" Padding="4,0,4,2" Background="{TemplateBinding Background}" >` |
| `Ellipse` | — | `Margin` | `8,8,8,8` | ButtonStates.BottomDirection | generic.xaml:L19516 | `<Ellipse Margin="8,8,8,8" UseLayoutRounding="false" Fill="{Binding}" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Stroke="{ThemeResource InkToolbar…` |
| `Ellipse` | — | `Margin` | `8,8,8,8` | ButtonStates.Unfocused | generic.xaml:L19588 | `<Ellipse Margin="8,8,8,8" UseLayoutRounding="false" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Fill="{Binding}" />` |
| `TextBlock` | PenStrokeWidthTitle | `Padding` | `12,0,12,0` | ButtonStates.Unfocused | generic.xaml:L19595 | `<TextBlock x:Name="PenStrokeWidthTitle" Grid.Row="5" Grid.Column="0" Padding="12,0,12,0" VerticalAlignment="Center" Style="{ThemeResource BodyTextBlockStyle}" T…` |
| `Grid` | StrokePreviewGrid | `MinHeight` | `24` | ButtonStates.Unfocused | generic.xaml:L19598 | `<Grid x:Name="StrokePreviewGrid" Grid.Row="6" UseLayoutRounding="false" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" MinHeight="24">` |
| `Canvas` | StrokePreviewCanvas | `Margin` | `12, 0, 12, 4` | ButtonStates.Unfocused | generic.xaml:L19601 | `<Canvas x:Name="StrokePreviewCanvas" Margin="12, 0, 12, 4" />` |
| `Slider` | PenStrokeWidthSlider | `Margin` | `12,0,12,0` | ButtonStates.Unfocused | generic.xaml:L19604 | `<Slider x:Name="PenStrokeWidthSlider" Grid.Row="7" Grid.Column="0" Width="296" Height="44" Margin="12,0,12,0" HorizontalAlignment="Stretch" Minimum="{Binding Re…` |
| `Slider` | PenStrokeWidthSlider | `Height` | `44` | ButtonStates.Unfocused | generic.xaml:L19604 | `<Slider x:Name="PenStrokeWidthSlider" Grid.Row="7" Grid.Column="0" Width="296" Height="44" Margin="12,0,12,0" HorizontalAlignment="Stretch" Minimum="{Binding Re…` |
| `Slider` | PenStrokeWidthSlider | `Width` | `296` | ButtonStates.Unfocused | generic.xaml:L19604 | `<Slider x:Name="PenStrokeWidthSlider" Grid.Row="7" Grid.Column="0" Width="296" Height="44" Margin="12,0,12,0" HorizontalAlignment="Stretch" Minimum="{Binding Re…` |

### 5.44 `InkToolbarRulerButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.LeftToRight | generic.xaml:L19085 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.45 `InkToolbarStencilButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **13**，C 元素属性 **1**，引用资源 **0** · 模板定义始于 L19367。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `ExtensionGlyph.Margin` | `0,2,0,0` | ButtonFlyoutDirectionStates | **TopDirection** | generic.xaml:L19367 | `<Setter Target="ExtensionGlyph.Margin" Value="0,2,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L19375 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L19376 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirection** | generic.xaml:L19379 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L19387 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L19388 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirection** | generic.xaml:L19391 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L19399 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L19400 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `0,0,2,0` | ButtonFlyoutDirectionStates | **RightDirectionRTL** | generic.xaml:L19403 | `<Setter Target="ExtensionGlyph.Margin" Value="0,0,2,0" />` |
| `SelectionAccent.Height` | `auto` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L19411 | `<Setter Target="SelectionAccent.Height" Value="auto" />` |
| `SelectionAccent.Width` | `2` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L19412 | `<Setter Target="SelectionAccent.Width" Value="2" />` |
| `ExtensionGlyph.Margin` | `2,0,0,0` | ButtonFlyoutDirectionStates | **LeftDirectionRTL** | generic.xaml:L19415 | `<Setter Target="ExtensionGlyph.Margin" Value="2,0,0,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Border | `Margin` | `1,0` | ButtonStates.BottomDirection | generic.xaml:L19447 | `<Border x:Name="Border" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Grid.RowSpan="2" Grid.ColumnSpan="2" Mar…` |

### 5.46 `ListBox`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

### 5.47 `ListBoxItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

### 5.48 `ListViewHeaderItem`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0,0,0,4` | 样式级 | generic.xaml:L10649 | `<Setter Property="Margin" Value="0,0,0,4" />` |
| `Padding` | `12,8,12,0` | 样式级 | generic.xaml:L10650 | `<Setter Property="Padding" Value="12,8,12,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | — | `Margin` | `12,8,12,0` | ButtonStates.TextBoxUnfocused | generic.xaml:L10666 | `<Rectangle Stroke="{ThemeResource ListViewHeaderItemDividerStroke}" StrokeThickness="0.5" Height="1" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Ma…` |
| `Rectangle` | — | `Height` | `1` | ButtonStates.TextBoxUnfocused | generic.xaml:L10666 | `<Rectangle Stroke="{ThemeResource ListViewHeaderItemDividerStroke}" StrokeThickness="0.5" Height="1" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Ma…` |

### 5.49 `ListViewItem`

条目数：A 模板/样式级 **6**，B 视觉状态 **0**，C 元素属性 **26**，引用资源 **5**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `12,0,12,0` | 样式级 | generic.xaml:L24384 | `<Setter Property="Padding" Value="12,0,12,0" />` |
| `BorderThickness` | `0` | 样式级 | generic.xaml:L28060 | `<Setter Property="BorderThickness" Value="0" />` |
| `Padding` | `12,0,12,0` | 样式级 | generic.xaml:L28065 | `<Setter Property="Padding" Value="12,0,12,0" />` |
| `Padding` | `8,0,0,0` | 样式级 | generic.xaml:L29549 | `<Setter Property="Padding" Value="8,0,0,0" />` |
| `MinHeight` | `32` | 样式级 | generic.xaml:L29553 | `<Setter Property="MinHeight" Value="32" />` |
| `MinWidth` | `120` | 样式级 | generic.xaml:L29554 | `<Setter Property="MinWidth" Value="120" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | ContentPresenterGrid | `Margin` | `0,0,0,0` | .DraggedPlaceholder | generic.xaml:L28483 | `<Grid x:Name="ContentPresenterGrid" Background="Transparent" Margin="0,0,0,0">` |
| `Border` | MultiSelectSquare | `Height` | `20` | .DraggedPlaceholder | generic.xaml:L28507 | `<Border x:Name="MultiSelectSquare" BorderBrush="{ThemeResource SystemControlForegroundBaseMediumHighBrush}" BorderThickness="2" Width="20" Height="20" Margin="1…` |
| `Border` | MultiSelectSquare | `Margin` | `12,0,0,0` | .DraggedPlaceholder | generic.xaml:L28507 | `<Border x:Name="MultiSelectSquare" BorderBrush="{ThemeResource SystemControlForegroundBaseMediumHighBrush}" BorderThickness="2" Width="20" Height="20" Margin="1…` |
| `Border` | MultiSelectSquare | `BorderThickness` | `2` | .DraggedPlaceholder | generic.xaml:L28507 | `<Border x:Name="MultiSelectSquare" BorderBrush="{ThemeResource SystemControlForegroundBaseMediumHighBrush}" BorderThickness="2" Width="20" Height="20" Margin="1…` |
| `Border` | MultiSelectSquare | `Width` | `20` | .DraggedPlaceholder | generic.xaml:L28507 | `<Border x:Name="MultiSelectSquare" BorderBrush="{ThemeResource SystemControlForegroundBaseMediumHighBrush}" BorderThickness="2" Width="20" Height="20" Margin="1…` |
| `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | .DraggedPlaceholder | generic.xaml:L28534 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" Margin="12,0,0,0" MinWidth="20" Height="20" VerticalAlignment="Center" Horiz…` |
| `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | .DraggedPlaceholder | generic.xaml:L28534 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" Margin="12,0,0,0" MinWidth="20" Height="20" VerticalAlignment="Center" Horiz…` |
| `Border` | MultiArrangeOverlayTextBorder | `Margin` | `12,0,0,0` | .DraggedPlaceholder | generic.xaml:L28534 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" Margin="12,0,0,0" MinWidth="20" Height="20" VerticalAlignment="Center" Horiz…` |
| `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | .DraggedPlaceholder | generic.xaml:L28534 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" Margin="12,0,0,0" MinWidth="20" Height="20" VerticalAlignment="Center" Horiz…` |
| `Rectangle` | NormalRectangle | `Width` | `25.5` | .NoHighlight | generic.xaml:L28720 | `<Rectangle x:Name="NormalRectangle" Fill="{ThemeResource CheckBoxBackgroundThemeBrush}" Stroke="{ThemeResource CheckBoxBorderThemeBrush}" StrokeThickness="{Them…` |
| `Rectangle` | NormalRectangle | `Height` | `25.5` | .NoHighlight | generic.xaml:L28720 | `<Rectangle x:Name="NormalRectangle" Fill="{ThemeResource CheckBoxBackgroundThemeBrush}" Stroke="{ThemeResource CheckBoxBorderThemeBrush}" StrokeThickness="{Them…` |
| `Path` | CheckGlyph | `Height` | `17` | .NoHighlight | generic.xaml:L28726 | `<Path x:Name="CheckGlyph" IsHitTestVisible="False" Width="18.5" Height="17" Stretch="Fill" Opacity="0" HorizontalAlignment="Center" VerticalAlignment="Center" F…` |
| `Path` | CheckGlyph | `Width` | `18.5` | .NoHighlight | generic.xaml:L28726 | `<Path x:Name="CheckGlyph" IsHitTestVisible="False" Width="18.5" Height="17" Stretch="Fill" Opacity="0" HorizontalAlignment="Center" VerticalAlignment="Center" F…` |
| `Grid` | SelectedCheckMark | `Width` | `34` | .NoHighlight | generic.xaml:L28783 | `<Grid x:Name="SelectedCheckMark" Opacity="0" Height="34" Width="34" HorizontalAlignment="Right" VerticalAlignment="Top">` |
| `Grid` | SelectedCheckMark | `Height` | `34` | .NoHighlight | generic.xaml:L28783 | `<Grid x:Name="SelectedCheckMark" Opacity="0" Height="34" Width="34" HorizontalAlignment="Right" VerticalAlignment="Top">` |
| `Path` | SelectedGlyph | `Margin` | `0,1,1,0` | .NoHighlight | generic.xaml:L28793 | `<Path x:Name="SelectedGlyph" Data="M0,123 L39,93 L124,164 L256,18 L295,49 L124,240 z" Fill="{ThemeResource ListViewItemCheckThemeBrush}" Height="14.5" Stretch="…` |
| `Path` | SelectedGlyph | `Width` | `17` | .NoHighlight | generic.xaml:L28793 | `<Path x:Name="SelectedGlyph" Data="M0,123 L39,93 L124,164 L256,18 L295,49 L124,240 z" Fill="{ThemeResource ListViewItemCheckThemeBrush}" Height="14.5" Stretch="…` |
| `Path` | SelectedGlyph | `Height` | `14.5` | .NoHighlight | generic.xaml:L28793 | `<Path x:Name="SelectedGlyph" Data="M0,123 L39,93 L124,164 L256,18 L295,49 L124,240 z" Fill="{ThemeResource ListViewItemCheckThemeBrush}" Height="14.5" Stretch="…` |
| `Grid` | RootGrid | `Margin` | `0,0,11,0` | .PointerOver | generic.xaml:L29562 | `<Grid x:Name="RootGrid" Margin="0,0,11,0" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBindin…` |
| `Grid` | — | `Height` | `32` | .Enabled | generic.xaml:L29649 | `<Grid VerticalAlignment="Top" Height="32">` |
| `Ellipse` | OuterEllipse | `Height` | `20` | .Enabled | generic.xaml:L29650 | `<Ellipse x:Name="OuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseStroke}" Fill="{StaticResource Rad…` |
| `Ellipse` | OuterEllipse | `Width` | `20` | .Enabled | generic.xaml:L29650 | `<Ellipse x:Name="OuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseStroke}" Fill="{StaticResource Rad…` |
| `Ellipse` | CheckOuterEllipse | `Height` | `20` | .Enabled | generic.xaml:L29657 | `<Ellipse x:Name="CheckOuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseCheckedStroke}" Fill="{ThemeR…` |
| `Ellipse` | CheckOuterEllipse | `Width` | `20` | .Enabled | generic.xaml:L29657 | `<Ellipse x:Name="CheckOuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseCheckedStroke}" Fill="{ThemeR…` |
| `Ellipse` | CheckGlyph | `Height` | `10` | .Enabled | generic.xaml:L29665 | `<Ellipse x:Name="CheckGlyph" Width="10" Height="10" UseLayoutRounding="False" Opacity="0" Fill="{ThemeResource RadioButtonCheckGlyphFill}" Stroke="{ThemeResourc…` |
| `Ellipse` | CheckGlyph | `Width` | `10` | .Enabled | generic.xaml:L29665 | `<Ellipse x:Name="CheckGlyph" Width="10" Height="10" UseLayoutRounding="False" Opacity="0" Fill="{ThemeResource RadioButtonCheckGlyphFill}" Stroke="{ThemeResourc…` |

### 5.50 `LoopingSelectorItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ContentPresenter` | ContentPresenter | `Margin` | `2,0,2,0` | ButtonStates.Expanded | generic.xaml:L15103 | `<ContentPresenter x:Name="ContentPresenter" Foreground="{TemplateBinding Foreground}" Content="{TemplateBinding Content}" ContentTemplate="{TemplateBinding Cont…` |

### 5.51 `MediaTransportControls`

条目数：A 模板/样式级 **1**，B 视觉状态 **1**，C 元素属性 **12**，引用资源 **6** · 模板定义始于 L15342。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 模板级 | generic.xaml:L15681 | `<Setter Property="Padding" Value="0" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `MediaControlsCommandBar.Margin` | `0` | MediaTransportControlMode | **CompactMode** | generic.xaml:L15800 | `<Setter Target="MediaControlsCommandBar.Margin" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | ErrorBorder | `Height` | `96` | ButtonStates.RepeatNoneState | generic.xaml:L15876 | `<Border x:Name="ErrorBorder" Width="320" Height="96" Grid.ColumnSpan="3" HorizontalAlignment="Center" Background="{ThemeResource MediaTransportControlsPanelBack…` |
| `Border` | ErrorBorder | `Width` | `320` | ButtonStates.RepeatNoneState | generic.xaml:L15876 | `<Border x:Name="ErrorBorder" Width="320" Height="96" Grid.ColumnSpan="3" HorizontalAlignment="Center" Background="{ThemeResource MediaTransportControlsPanelBack…` |
| `TextBlock` | ErrorTextBlock | `Margin` | `12` | ButtonStates.RepeatNoneState | generic.xaml:L15883 | `<TextBlock x:Name="ErrorTextBlock" Style="{StaticResource MediaTextBlockStyle}" TextWrapping="WrapWholeWords" Margin="12" />` |
| `Slider` | ProgressSlider | `Height` | `33` | ButtonStates.RepeatNoneState | generic.xaml:L15897 | `<Slider x:Name="ProgressSlider" Style="{StaticResource MediaSliderStyle}" Margin="12,0" MinWidth="80" Height="33" VerticalAlignment="Center" IsThumbToolTipEnabl…` |
| `Slider` | ProgressSlider | `MinWidth` | `80` | ButtonStates.RepeatNoneState | generic.xaml:L15897 | `<Slider x:Name="ProgressSlider" Style="{StaticResource MediaSliderStyle}" Margin="12,0" MinWidth="80" Height="33" VerticalAlignment="Center" IsThumbToolTipEnabl…` |
| `Slider` | ProgressSlider | `Margin` | `12,0` | ButtonStates.RepeatNoneState | generic.xaml:L15897 | `<Slider x:Name="ProgressSlider" Style="{StaticResource MediaSliderStyle}" Margin="12,0" MinWidth="80" Height="33" VerticalAlignment="Center" IsThumbToolTipEnabl…` |
| `ProgressBar` | BufferingProgressBar | `Margin` | `0,2,0,0` | ButtonStates.RepeatNoneState | generic.xaml:L15904 | `<ProgressBar x:Name="BufferingProgressBar" Height="4" IsIndeterminate="True" IsHitTestVisible="False" VerticalAlignment="Top" Margin="0,2,0,0" Visibility="Colla…` |
| `ProgressBar` | BufferingProgressBar | `Height` | `4` | ButtonStates.RepeatNoneState | generic.xaml:L15904 | `<ProgressBar x:Name="BufferingProgressBar" Height="4" IsIndeterminate="True" IsHitTestVisible="False" VerticalAlignment="Top" Margin="0,2,0,0" Visibility="Colla…` |
| `Grid` | TimeTextGrid | `Margin` | `12,0` | ButtonStates.RepeatNoneState | generic.xaml:L15911 | `<Grid x:Name="TimeTextGrid" Margin="12,0" Grid.Row="1">` |
| `TextBlock` | TimeElapsedElement | `Margin` | `0` | ButtonStates.RepeatNoneState | generic.xaml:L15914 | `<TextBlock x:Name="TimeElapsedElement" Style="{StaticResource MediaTextBlockStyle}" Margin="0" Text="00:00" HorizontalAlignment="Left" VerticalAlignment="Bottom…` |
| `AppBarButton` | PlayPauseButtonOnLeft | `Margin` | `0` | ButtonStates.RepeatNoneState | generic.xaml:L15932 | `<AppBarButton x:Name="PlayPauseButtonOnLeft" Margin="0" VerticalAlignment="Center" Style="{StaticResource AppBarButtonStyle}">` |
| `CommandBar` | MediaControlsCommandBar | `Margin` | `0,0` | ButtonStates.RepeatNoneState | generic.xaml:L15944 | `<CommandBar x:Name="MediaControlsCommandBar" Margin="0,0" Style="{StaticResource CommandBarStyle}" IsDynamicOverflowEnabled="False">` |

### 5.52 `MenuBar`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

### 5.53 `MenuBarItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **1**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | — | `Padding` | `12,0,12,0` | ButtonStates.Normal | generic.xaml:L21087 | `<Button x:Name ="ContentButton" Content="{TemplateBinding Title}" Background="Transparent" BorderThickness="0" VerticalAlignment="Stretch" Padding="12,0,12,0" I…` |
| `Button` | — | `BorderThickness` | `0` | ButtonStates.Normal | generic.xaml:L21087 | `<Button x:Name ="ContentButton" Content="{TemplateBinding Title}" Background="Transparent" BorderThickness="0" VerticalAlignment="Stretch" Padding="12,0,12,0" I…` |

### 5.54 `MenuFlyoutItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **9**，引用资源 **9** · 模板定义始于 L8487。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Polyline` | CurrentSelectionIcon | `Margin` | `0,0,0,0` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8527 | `<Polyline x:Name="CurrentSelectionIcon" Points="0,0 0,24" Stroke="{ThemeResource HandwritingViewLanguageSelectionColor}" StrokeThickness="3" StrokeStartLineCap=…` |
| `Viewbox` | IconRoot | `Margin` | `12,0,0,0` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8539 | `<Viewbox x:Name="IconRoot" HorizontalAlignment="Center" VerticalAlignment="Center" Width="24" Height="24" Margin="12,0,0,0" Grid.Column="1">` |
| `Viewbox` | IconRoot | `Height` | `24` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8539 | `<Viewbox x:Name="IconRoot" HorizontalAlignment="Center" VerticalAlignment="Center" Width="24" Height="24" Margin="12,0,0,0" Grid.Column="1">` |
| `Viewbox` | IconRoot | `Width` | `24` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8539 | `<Viewbox x:Name="IconRoot" HorizontalAlignment="Center" VerticalAlignment="Center" Width="24" Height="24" Margin="12,0,0,0" Grid.Column="1">` |
| `TextBlock` | TextBlock | `Margin` | `12,0,0,0` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8550 | `<TextBlock x:Name="TextBlock" Text="{TemplateBinding Text}" TextTrimming="Clip" Margin="12,0,0,0" Foreground="{TemplateBinding Foreground}" HorizontalAlignment=…` |
| `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `12,0,0,0` | ButtonStates.KeyboardAcceleratorTextCollapsed | generic.xaml:L8559 | `<TextBlock x:Name="KeyboardAcceleratorTextBlock" Grid.Column="2" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |
| `Viewbox` | IconRoot | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25191 | `<Viewbox x:Name="IconRoot" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `Viewbox` | IconRoot | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25191 | `<Viewbox x:Name="IconRoot" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `24,0,0,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25207 | `<TextBlock x:Name="KeyboardAcceleratorTextBlock" Grid.Column="1" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |

### 5.55 `MenuFlyoutPresenter`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **5**。

### 5.56 `MenuFlyoutSeparator`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

### 5.57 `MenuFlyoutSubItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **6**，引用资源 **6** · 模板定义始于 L25468。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Viewbox` | IconRoot | `Height` | `16` | .DefaultPadding | generic.xaml:L25507 | `<Viewbox x:Name="IconRoot" Grid.Column="0" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `Viewbox` | IconRoot | `Width` | `16` | .DefaultPadding | generic.xaml:L25507 | `<Viewbox x:Name="IconRoot" Grid.Column="0" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `TextBlock` | — | `Margin` | `-8,-8,0,0` | .DefaultPadding | generic.xaml:L25545 | `<TextBlock Foreground="{ThemeResource RatingControlUnselectedForeground}" Margin="-8,-8,0,0" FontSize="32" Text="&#xE734;" AutomationProperties.AccessibilityVie…` |
| `TextBlock` | — | `Margin` | `-8,-8,0,0` | .DefaultPadding | generic.xaml:L25556 | `<TextBlock Margin="-8,-8,0,0" FontSize="32" Text="&#xE735;" AutomationProperties.AccessibilityView="Raw" FontFamily="{ThemeResource SymbolThemeFontFamily}"/>` |
| `Image` | — | `Margin` | `-8,-8,0,0` | .DefaultPadding | generic.xaml:L25564 | `<Image Margin="-8,-8,0,0" AutomationProperties.AccessibilityView="Raw" />` |
| `Image` | — | `Margin` | `-8,-8,0,0` | .DefaultPadding | generic.xaml:L25568 | `<Image Margin="-8,-8,0,0" AutomationProperties.AccessibilityView="Raw" />` |

### 5.58 `NavigationView`

条目数：A 模板/样式级 **2**，B 视觉状态 **5**，C 元素属性 **9**，引用资源 **4** · 模板定义始于 L19732。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0,8` | 模板级 | generic.xaml:L19956 | `<Setter Property="Padding" Value="0,8" />` |
| `Margin` | `0,-4,0,0` | 模板级 | generic.xaml:L19958 | `<Setter Property="Margin" Value="0,-4,0,0" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `HeaderContent.Margin` | `48,5,0,0` | DisplayModeGroup | **Minimal** | generic.xaml:L19732 | `<Setter Target="HeaderContent.Margin" Value="48,5,0,0" />` |
| `HeaderContent.Margin` | `104,5,0,0` | DisplayModeGroup | **MinimalWithBackButton** | generic.xaml:L19738 | `<Setter Target="HeaderContent.Margin" Value="104,5,0,0" />` |
| `PaneContentGridToggleButtonRow.Height` | `4` | TogglePaneGroup | **TogglePaneButtonCollapsed** | generic.xaml:L19748 | `<Setter Target="PaneContentGridToggleButtonRow.Height" Value="4" />` |
| `PaneContentGrid.Margin` | `0,32,0,0` | TitleBarVisibilityGroup | **TitleBarCollapsed** | generic.xaml:L19821 | `<Setter Target="PaneContentGrid.Margin" Value="0,32,0,0" />` |
| `BackButtonPlaceholderOnTopNav.Width` | `0` | BackButtonGroup | **BackButtonCollapsed** | generic.xaml:L19841 | `<Setter Target="BackButtonPlaceholderOnTopNav.Width" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | PaneToggleButtonGrid | `Margin` | `0,0,0,8` | ButtonStates.BackButtonVisible | generic.xaml:L19849 | `<Grid x:Name="PaneToggleButtonGrid" Margin="0,0,0,8" HorizontalAlignment="Left" VerticalAlignment="Top" Canvas.ZIndex="100">` |
| `ColumnDefinition` | — | `MinWidth` | `48` | ButtonStates.BackButtonVisible | generic.xaml:L19914 | `<ColumnDefinition Width="*" MinWidth="48" />` |
| `Grid` | TopNavLeftPadding | `Width` | `0` | ButtonStates.BackButtonVisible | generic.xaml:L19920 | `<Grid x:Name="TopNavLeftPadding" Grid.Column="1" Width="0"/>` |
| `ContentControl` | TopPaneAutoSuggestBoxPresenter | `MinWidth` | `48` | ButtonStates.BackButtonVisible | generic.xaml:L19982 | `<ContentControl x:Name="TopPaneAutoSuggestBoxPresenter" Margin="12,0,12,0" MinWidth="48" IsTabStop="False" HorizontalContentAlignment="Stretch" VerticalContentA…` |
| `ContentControl` | TopPaneAutoSuggestBoxPresenter | `Margin` | `12,0,12,0` | ButtonStates.BackButtonVisible | generic.xaml:L19982 | `<ContentControl x:Name="TopPaneAutoSuggestBoxPresenter" Margin="12,0,12,0" MinWidth="48" IsTabStop="False" HorizontalContentAlignment="Stretch" VerticalContentA…` |
| `RowDefinition` | — | `Height` | `0` | ButtonStates.BackButtonVisible | generic.xaml:L20023 | `<RowDefinition Height="0" />` |
| `RowDefinition` | — | `Height` | `8` | ButtonStates.BackButtonVisible | generic.xaml:L20028 | `<RowDefinition Height="8" />` |
| `RowDefinition` | — | `Height` | `8` | ButtonStates.BackButtonVisible | generic.xaml:L20033 | `<RowDefinition Height="8" />` |
| `NavigationViewList` | MenuItemsHost | `Margin` | `0,0,0,20` | ButtonStates.BackButtonVisible | generic.xaml:L20081 | `<NavigationViewList x:Name="MenuItemsHost" Grid.Row="6" Margin="0,0,0,20" SelectionMode="Single" IsItemClickEnabled="True" HorizontalAlignment="Stretch" Selecte…` |

### 5.59 `NavigationViewItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **2**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Viewbox` | IconBox | `Margin` | `0,0,0,0` | .Enabled | generic.xaml:L26204 | `<Viewbox x:Name="IconBox" Grid.Row="1" Grid.Column="1" Height="16" Width ="48" Margin="0,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Height` | `16` | .Enabled | generic.xaml:L26204 | `<Viewbox x:Name="IconBox" Grid.Row="1" Grid.Column="1" Height="16" Width ="48" Margin="0,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |

### 5.60 `NavigationViewItemHeader`

条目数：A 模板/样式级 **1**，B 视觉状态 **1**，C 元素属性 **2**，引用资源 **0** · 模板定义始于 L20247。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MinHeight` | `0` | 样式级 | generic.xaml:L20188 | `<Setter Property="MinHeight" Value="0" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `InnerHeaderGrid.Height` | `20` | PaneStates | **HeaderTextCollapsed** | generic.xaml:L20247 | `<Setter Target="InnerHeaderGrid.Height" Value="20" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | InnerHeaderGrid | `Height` | `40` | ButtonStates.HeaderTextVisible | generic.xaml:L20254 | `<Grid x:Name="InnerHeaderGrid" Height="40" HorizontalAlignment="Left" Margin="{ThemeResource NavigationViewItemInnerHeaderMargin}">` |
| `TextBlock` | HeaderText | `Margin` | `0,-1,0,-1` | ButtonStates.HeaderTextVisible | generic.xaml:L20255 | `<TextBlock x:Name="HeaderText" VerticalAlignment="Center" Margin="0,-1,0,-1" Style="{StaticResource NavigationViewItemHeaderTextStyle}" Text="{TemplateBinding C…` |

### 5.61 `NavigationViewItemPresenter`

条目数：A 模板/样式级 **0**，B 视觉状态 **11**，C 元素属性 **30**，引用资源 **2** · 模板定义始于 L25941。

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `IconColumn.Width` | `16` | IconStates | **IconCollapsed** | generic.xaml:L25941 | `<Setter Target="IconColumn.Width" Value="16" />` |
| `IconColumn.Width` | `16` | IconStates | **IconCollapsed** | generic.xaml:L26050 | `<Setter Target="IconColumn.Width" Value="16" />` |
| `LayoutRoot.Width` | `48` | NavigationViewIconPositionStates | **IconOnly** | generic.xaml:L26303 | `<Setter Target="LayoutRoot.Width" Value="48" />` |
| `SelectionIndicatorGrid.Margin` | `4,0,4,4` | NavigationViewIconPositionStates | **IconOnly** | generic.xaml:L26305 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="4,0,4,4" />` |
| `ContentPresenter.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | generic.xaml:L26311 | `<Setter Target="ContentPresenter.Margin" Value="12,0" />` |
| `SelectionIndicatorGrid.Margin` | `12,0,12,4` | NavigationViewIconPositionStates | **ContentOnly** | generic.xaml:L26312 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="12,0,12,4" />` |
| `LayoutRoot.Width` | `48` | NavigationViewIconPositionStates | **IconOnly** | generic.xaml:L26413 | `<Setter Target="LayoutRoot.Width" Value="48" />` |
| `SelectionIndicatorGrid.Margin` | `4,0` | NavigationViewIconPositionStates | **IconOnly** | generic.xaml:L26415 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="4,0" />` |
| `ContentPresenter.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | generic.xaml:L26421 | `<Setter Target="ContentPresenter.Margin" Value="12,0" />` |
| `SelectionIndicatorGrid.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | generic.xaml:L26422 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="12,0" />` |
| `ContentPresenter.Margin` | `16,0` | NavigationViewIconPositionStates | **ContentOnly** | generic.xaml:L26555 | `<Setter Target="ContentPresenter.Margin" Value="16,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | — | `Height` | `44` | ButtonStates.Normal | generic.xaml:L21103 | `<Grid Height="44">` |
| `Grid` | LayoutRoot | `Height` | `40` | .Normal | generic.xaml:L25868 | `<Grid x:Name="LayoutRoot" Height="40" Background="{TemplateBinding Background}" Control.IsTemplateFocusTarget="True">` |
| `Grid` | — | `Margin` | `4,0,0,0` | .IconVisible | generic.xaml:L25950 | `<Grid Margin="4,0,0,0" HorizontalAlignment="Left" VerticalAlignment="Center">` |
| `Rectangle` | SelectionIndicator | `Height` | `24` | .IconVisible | generic.xaml:L25955 | `<Rectangle x:Name="SelectionIndicator" Width="2" Height="24" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0.0"/>` |
| `Rectangle` | SelectionIndicator | `Width` | `2` | .IconVisible | generic.xaml:L25955 | `<Rectangle x:Name="SelectionIndicator" Width="2" Height="24" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0.0"/>` |
| `Grid` | ContentGrid | `Height` | `40` | .IconVisible | generic.xaml:L25969 | `<Grid Height="40" HorizontalAlignment="Left" x:Name="ContentGrid">` |
| `ColumnDefinition` | IconColumn | `Width` | `48` | .IconVisible | generic.xaml:L25972 | `<ColumnDefinition x:Name="IconColumn" Width="48" />` |
| `Grid` | LayoutRoot | `Height` | `40` | .IconVisible | generic.xaml:L26014 | `<Grid x:Name="LayoutRoot" Height="40" Background="{TemplateBinding Background}" Control.IsTemplateFocusTarget="True">` |
| `Grid` | — | `Margin` | `4,0,0,0` | .IconVisible | generic.xaml:L26059 | `<Grid Margin="4,0,0,0" HorizontalAlignment="Left" VerticalAlignment="Center">` |
| `Rectangle` | SelectionIndicator | `Height` | `24` | .IconVisible | generic.xaml:L26064 | `<Rectangle x:Name="SelectionIndicator" Width="2" Height="24" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0.0"/>` |
| `Rectangle` | SelectionIndicator | `Width` | `2` | .IconVisible | generic.xaml:L26064 | `<Rectangle x:Name="SelectionIndicator" Width="2" Height="24" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0.0"/>` |
| `Grid` | ContentGrid | `Height` | `40` | .IconVisible | generic.xaml:L26073 | `<Grid x:Name="ContentGrid" Height="40" HorizontalAlignment="Left">` |
| `ColumnDefinition` | IconColumn | `Width` | `48` | .IconVisible | generic.xaml:L26079 | `<ColumnDefinition x:Name="IconColumn" Width="48" />` |
| `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnLeft | generic.xaml:L26329 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Width` | `16` | .IconOnLeft | generic.xaml:L26329 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Height` | `16` | .IconOnLeft | generic.xaml:L26329 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `ContentPresenter` | ContentPresenter | `Margin` | `8,0,16,0` | .IconOnLeft | generic.xaml:L26341 | `<ContentPresenter x:Name="ContentPresenter" Grid.Column="1" Margin="8,0,16,0" Foreground="{ThemeResource TopNavigationViewItemForeground}" TextWrapping="NoWrap"…` |
| `Grid` | SelectionIndicatorGrid | `Margin` | `16,0,16,4` | .IconOnLeft | generic.xaml:L26356 | `<Grid x:Name="SelectionIndicatorGrid" Margin="16,0,16,4" VerticalAlignment="Bottom">` |
| `Rectangle` | SelectionIndicator | `Height` | `2` | .IconOnLeft | generic.xaml:L26360 | `<Rectangle x:Name="SelectionIndicator" Height="2" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0" />` |
| `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnLeft | generic.xaml:L26439 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Width` | `16` | .IconOnLeft | generic.xaml:L26439 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Height` | `16` | .IconOnLeft | generic.xaml:L26439 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `ContentPresenter` | ContentPresenter | `Margin` | `8,0,16,0` | .IconOnLeft | generic.xaml:L26451 | `<ContentPresenter x:Name="ContentPresenter" Grid.Column="1" Margin="8,0,16,0" Foreground="{ThemeResource DefaultTextForegroundThemeBrush}" TextWrapping="NoWrap"…` |
| `Grid` | SelectionIndicatorGrid | `Margin` | `16,0,16,4` | .IconOnLeft | generic.xaml:L26464 | `<Grid x:Name="SelectionIndicatorGrid" Margin="16,0,16,4" VerticalAlignment="Bottom">` |
| `Rectangle` | SelectionIndicator | `Height` | `2` | .IconOnLeft | generic.xaml:L26468 | `<Rectangle x:Name="SelectionIndicator" Height="2" Fill="{ThemeResource NavigationViewSelectionIndicatorForeground}" Opacity="0" />` |
| `Grid` | LayoutRoot | `Height` | `40` | .IconOnLeft | generic.xaml:L26486 | `<Grid x:Name="LayoutRoot" Height="40" Background="{TemplateBinding Background}" Control.IsTemplateFocusTarget="True">` |
| `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnly | generic.xaml:L26568 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Width` | `16` | .IconOnly | generic.xaml:L26568 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `Viewbox` | IconBox | `Height` | `16` | .IconOnly | generic.xaml:L26568 | `<Viewbox x:Name="IconBox" Height="16" Width="16" Margin="16,0,0,0" VerticalAlignment="Center" HorizontalAlignment="Center">` |
| `ContentPresenter` | ContentPresenter | `Margin` | `12,0,16,0` | .IconOnly | generic.xaml:L26580 | `<ContentPresenter x:Name="ContentPresenter" Grid.Column="1" Margin="12,0,16,0" Foreground="{TemplateBinding Foreground}" TextWrapping="NoWrap" ContentTransition…` |

### 5.62 `NavigationViewItemSeparator`

条目数：A 模板/样式级 **1**，B 视觉状态 **3**，C 元素属性 **2**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MinHeight` | `0` | 样式级 | generic.xaml:L20274 | `<Setter Property="MinHeight" Value="0" />` |

**B. 视觉状态内 `Setter`（写死）**

| 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `SeparatorLine.Height` | `20` | NavigationSeparatorLineStates | **VerticalLine** | generic.xaml:L20286 | `<Setter Target="SeparatorLine.Height" Value="20" />` |
| `SeparatorLine.Width` | `1` | NavigationSeparatorLineStates | **VerticalLine** | generic.xaml:L20287 | `<Setter Target="SeparatorLine.Width" Value="1" />` |
| `SeparatorLine.Margin` | `10,0` | NavigationSeparatorLineStates | **VerticalLine** | generic.xaml:L20288 | `<Setter Target="SeparatorLine.Margin" Value="10,0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | SeparatorLine | `Margin` | `16,10` | ButtonStates.HorizontalLine | generic.xaml:L20296 | `<Rectangle x:Name="SeparatorLine" Height="1" Margin="16,10" Fill="{ThemeResource SystemControlForegroundBaseLowBrush}" />` |
| `Rectangle` | SeparatorLine | `Height` | `1` | ButtonStates.HorizontalLine | generic.xaml:L20296 | `<Rectangle x:Name="SeparatorLine" Height="1" Margin="16,10" Fill="{ThemeResource SystemControlForegroundBaseLowBrush}" />` |

### 5.63 `PasswordBox`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **2**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ToggleButton` | RevealButton | `MinWidth` | `34` | .ButtonCollapsed | generic.xaml:L28027 | `<ToggleButton x:Name="RevealButton" Grid.Row="1" Grid.Column="1" Style="{StaticResource RevealButtonStyle}" BorderThickness="{TemplateBinding BorderThickness}" …` |

### 5.64 `PersonPicture`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `100` | 样式级 | generic.xaml:L20859 | `<Setter Property="Width" Value="100" />` |
| `Height` | `100` | 样式级 | generic.xaml:L20860 | `<Setter Property="Height" Value="100" />` |

### 5.65 `Pivot`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **4**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0` | 样式级 | generic.xaml:L14071 | `<Setter Property="Margin" Value="0" />` |
| `Padding` | `0` | 样式级 | generic.xaml:L14072 | `<Setter Property="Padding" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | PreviousButton | `Height` | `36` | ButtonStates.Normal | generic.xaml:L14364 | `<Button x:Name="PreviousButton" Grid.Column="1" Template="{StaticResource PreviousTemplate}" Width="20" Height="36" UseSystemFocusVisuals="False" Margin="{Theme…` |
| `Button` | PreviousButton | `Width` | `20` | ButtonStates.Normal | generic.xaml:L14364 | `<Button x:Name="PreviousButton" Grid.Column="1" Template="{StaticResource PreviousTemplate}" Width="20" Height="36" UseSystemFocusVisuals="False" Margin="{Theme…` |
| `Button` | NextButton | `Height` | `36` | ButtonStates.Normal | generic.xaml:L14377 | `<Button x:Name="NextButton" Grid.Column="1" Template="{StaticResource NextTemplate}" Width="20" Height="36" UseSystemFocusVisuals="False" Margin="{ThemeResource…` |
| `Button` | NextButton | `Width` | `20` | ButtonStates.Normal | generic.xaml:L14377 | `<Button x:Name="NextButton" Grid.Column="1" Template="{StaticResource NextTemplate}" Width="20" Height="36" UseSystemFocusVisuals="False" Margin="{ThemeResource…` |

### 5.66 `PivotHeaderItem`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Height` | `48` | 样式级 | generic.xaml:L14459 | `<Setter Property="Height" Value="48" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | SelectedPipe | `Margin` | `0,0,0,2` | ButtonStates.Center | generic.xaml:L14590 | `<Rectangle x:Name="SelectedPipe" Fill="{ThemeResource PivotHeaderItemSelectedPipeFill}" Height="2" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Marg…` |
| `Rectangle` | SelectedPipe | `Height` | `2` | ButtonStates.Center | generic.xaml:L14590 | `<Rectangle x:Name="SelectedPipe" Fill="{ThemeResource PivotHeaderItemSelectedPipeFill}" Height="2" VerticalAlignment="Bottom" HorizontalAlignment="Stretch" Marg…` |

### 5.67 `PivotItem`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 样式级 | generic.xaml:L14420 | `<Setter Property="Padding" Value="0" />` |

### 5.68 `ProgressBar`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **2**。

### 5.69 `ProgressRing`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `MinHeight` | `20` | 样式级 | generic.xaml:L12361 | `<Setter Property="MinHeight" Value="20" />` |
| `MinWidth` | `20` | 样式级 | generic.xaml:L12362 | `<Setter Property="MinWidth" Value="20" />` |

### 5.70 `RadioButton`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **8**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `8,6,0,0` | 样式级 | generic.xaml:L6799 | `<Setter Property="Padding" Value="8,6,0,0" />` |
| `MinWidth` | `120` | 样式级 | generic.xaml:L6806 | `<Setter Property="MinWidth" Value="120" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `Width` | `20` | .Indeterminate | generic.xaml:L6949 | `<ColumnDefinition Width="20" />` |
| `Grid` | — | `Height` | `32` | .Indeterminate | generic.xaml:L6953 | `<Grid VerticalAlignment="Top" Height="32">` |
| `Ellipse` | OuterEllipse | `Height` | `20` | .Indeterminate | generic.xaml:L6954 | `<Ellipse x:Name="OuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseStroke}" Fill="{StaticResource Rad…` |
| `Ellipse` | OuterEllipse | `Width` | `20` | .Indeterminate | generic.xaml:L6954 | `<Ellipse x:Name="OuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseStroke}" Fill="{StaticResource Rad…` |
| `Ellipse` | CheckOuterEllipse | `Height` | `20` | .Indeterminate | generic.xaml:L6961 | `<Ellipse x:Name="CheckOuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseCheckedStroke}" Fill="{ThemeR…` |
| `Ellipse` | CheckOuterEllipse | `Width` | `20` | .Indeterminate | generic.xaml:L6961 | `<Ellipse x:Name="CheckOuterEllipse" Width="20" Height="20" UseLayoutRounding="False" Stroke="{ThemeResource RadioButtonOuterEllipseCheckedStroke}" Fill="{ThemeR…` |
| `Ellipse` | CheckGlyph | `Height` | `10` | .Indeterminate | generic.xaml:L6969 | `<Ellipse x:Name="CheckGlyph" Width="10" Height="10" UseLayoutRounding="False" Opacity="0" Fill="{ThemeResource RadioButtonCheckGlyphFill}" Stroke="{ThemeResourc…` |
| `Ellipse` | CheckGlyph | `Width` | `10` | .Indeterminate | generic.xaml:L6969 | `<Ellipse x:Name="CheckGlyph" Width="10" Height="10" UseLayoutRounding="False" Opacity="0" Fill="{ThemeResource RadioButtonCheckGlyphFill}" Stroke="{ThemeResourc…` |

### 5.71 `RatingControl`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **6**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Height` | `32` | 样式级 | generic.xaml:L19639 | `<Setter Property="Height" Value="32" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `StackPanel` | — | `Margin` | `-20,-20,-20,-20` | ButtonStates.Unfocused | generic.xaml:L19688 | `<StackPanel Orientation="Horizontal" Grid.Row="0" Margin="-20,-20,-20,-20">` |
| `StackPanel` | RatingBackgroundStackPanel | `Margin` | `20,20,0,20` | ButtonStates.Unfocused | generic.xaml:L19689 | `<StackPanel x:Name="RatingBackgroundStackPanel" Orientation="Horizontal" Background="Transparent" Margin="20,20,0,20" />` |
| `TextBlock` | Caption | `Margin` | `4,9,20,0` | ButtonStates.Unfocused | generic.xaml:L19690 | `<TextBlock x:Name="Caption" Height="32" Margin="4,9,20,0" TextLineBounds="TrimToBaseline" Style="{ThemeResource CaptionTextBlockStyle}" VerticalAlignment="Cente…` |
| `TextBlock` | Caption | `Height` | `32` | ButtonStates.Unfocused | generic.xaml:L19690 | `<TextBlock x:Name="Caption" Height="32" Margin="4,9,20,0" TextLineBounds="TrimToBaseline" Style="{ThemeResource CaptionTextBlockStyle}" VerticalAlignment="Cente…` |
| `StackPanel` | — | `Margin` | `-40,-40,-40,-40` | ButtonStates.Unfocused | generic.xaml:L19700 | `<StackPanel Orientation="Horizontal" Margin="-40,-40,-40,-40">` |
| `StackPanel` | RatingForegroundStackPanel | `Margin` | `40,40,40,40` | ButtonStates.Unfocused | generic.xaml:L19701 | `<StackPanel x:Name="RatingForegroundStackPanel" Orientation="Horizontal" IsHitTestVisible="False" Margin="40,40,40,40" />` |

### 5.72 `Rectangle`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Height` | `2` | 样式级 | generic.xaml:L17659 | `<Setter Property="Height" Value="2" />` |

### 5.73 `RefreshVisualizer`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Height` | `100` | 样式级 | generic.xaml:L20993 | `<Setter Property="Height" Value="100" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | Root | `MinHeight` | `80` | ButtonStates.NoBadge | generic.xaml:L20997 | `<Grid x:Name="Root" MinHeight="80" Background="{TemplateBinding Background}" />` |

### 5.74 `RepeatButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **4**。

### 5.75 `RichEditBox`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **2**。

### 5.76 `ScrollBar`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **18**，引用资源 **2**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | HorizontalTrackRect | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9221 | `<Rectangle x:Name="HorizontalTrackRect" Opacity="0" Grid.ColumnSpan="5" Margin="0" StrokeThickness="{ThemeResource ScrollBarTrackBorderThemeThickness}" Fill="{T…` |
| `RepeatButton` | HorizontalSmallDecrease | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9222 | `<RepeatButton x:Name="HorizontalSmallDecrease" Opacity="0" Grid.Column="0" MinHeight="{ThemeResource ScrollBarSize}" IsTabStop="False" Interval="50" Margin="0" …` |
| `RepeatButton` | HorizontalLargeDecrease | `Width` | `0` | ButtonStates.Collapsed | generic.xaml:L9223 | `<RepeatButton x:Name="HorizontalLargeDecrease" Opacity="0" Grid.Column="1" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" IsTabStop="False" Interval=…` |
| `RepeatButton` | HorizontalSmallIncrease | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9230 | `<RepeatButton x:Name="HorizontalSmallIncrease" Opacity="0" Grid.Column="4" MinHeight="{ThemeResource ScrollBarSize}" IsTabStop="False" Interval="50" Margin="0" …` |
| `Grid` | HorizontalPanningRoot | `MinWidth` | `24` | ButtonStates.Collapsed | generic.xaml:L9233 | `<Grid x:Name="HorizontalPanningRoot" MinWidth="24" Visibility="Collapsed" Opacity="0">` |
| `Border` | HorizontalPanningThumb | `MinWidth` | `32` | ButtonStates.Collapsed | generic.xaml:L9234 | `<Border x:Name="HorizontalPanningThumb" VerticalAlignment="Bottom" HorizontalAlignment="Left" Background="{ThemeResource ScrollBarPanningThumbBackground}" Borde…` |
| `Border` | HorizontalPanningThumb | `Margin` | `0,2,0,2` | ButtonStates.Collapsed | generic.xaml:L9234 | `<Border x:Name="HorizontalPanningThumb" VerticalAlignment="Bottom" HorizontalAlignment="Left" Background="{ThemeResource ScrollBarPanningThumbBackground}" Borde…` |
| `Border` | HorizontalPanningThumb | `BorderThickness` | `0` | ButtonStates.Collapsed | generic.xaml:L9234 | `<Border x:Name="HorizontalPanningThumb" VerticalAlignment="Bottom" HorizontalAlignment="Left" Background="{ThemeResource ScrollBarPanningThumbBackground}" Borde…` |
| `Border` | HorizontalPanningThumb | `Height` | `2` | ButtonStates.Collapsed | generic.xaml:L9234 | `<Border x:Name="HorizontalPanningThumb" VerticalAlignment="Bottom" HorizontalAlignment="Left" Background="{ThemeResource ScrollBarPanningThumbBackground}" Borde…` |
| `Rectangle` | VerticalTrackRect | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9246 | `<Rectangle x:Name="VerticalTrackRect" Opacity="0" Grid.RowSpan="5" Margin="0" StrokeThickness="{ThemeResource ScrollBarTrackBorderThemeThickness}" Fill="{ThemeR…` |
| `RepeatButton` | VerticalSmallDecrease | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9247 | `<RepeatButton x:Name="VerticalSmallDecrease" Opacity="0" Height="{ThemeResource ScrollBarSize}" MinWidth="{ThemeResource ScrollBarSize}" IsTabStop="False" Inter…` |
| `RepeatButton` | VerticalLargeDecrease | `Height` | `0` | ButtonStates.Collapsed | generic.xaml:L9248 | `<RepeatButton x:Name="VerticalLargeDecrease" Opacity="0" HorizontalAlignment="Stretch" VerticalAlignment="Stretch" Height="0" IsTabStop="False" Interval="50" Gr…` |
| `RepeatButton` | VerticalSmallIncrease | `Margin` | `0` | ButtonStates.Collapsed | generic.xaml:L9255 | `<RepeatButton x:Name="VerticalSmallIncrease" Opacity="0" Height="{ThemeResource ScrollBarSize}" MinWidth="{ThemeResource ScrollBarSize}" IsTabStop="False" Inter…` |
| `Grid` | VerticalPanningRoot | `MinHeight` | `24` | ButtonStates.Collapsed | generic.xaml:L9258 | `<Grid x:Name="VerticalPanningRoot" MinHeight="24" Visibility="Collapsed" Opacity="0">` |
| `Border` | VerticalPanningThumb | `MinHeight` | `32` | ButtonStates.Collapsed | generic.xaml:L9259 | `<Border x:Name="VerticalPanningThumb" VerticalAlignment="Top" HorizontalAlignment="Right" Background="{ThemeResource ScrollBarPanningThumbBackground}" BorderThi…` |
| `Border` | VerticalPanningThumb | `Margin` | `2,0,2,0` | ButtonStates.Collapsed | generic.xaml:L9259 | `<Border x:Name="VerticalPanningThumb" VerticalAlignment="Top" HorizontalAlignment="Right" Background="{ThemeResource ScrollBarPanningThumbBackground}" BorderThi…` |
| `Border` | VerticalPanningThumb | `BorderThickness` | `0` | ButtonStates.Collapsed | generic.xaml:L9259 | `<Border x:Name="VerticalPanningThumb" VerticalAlignment="Top" HorizontalAlignment="Right" Background="{ThemeResource ScrollBarPanningThumbBackground}" BorderThi…` |
| `Border` | VerticalPanningThumb | `Width` | `2` | ButtonStates.Collapsed | generic.xaml:L9259 | `<Border x:Name="VerticalPanningThumb" VerticalAlignment="Top" HorizontalAlignment="Right" Background="{ThemeResource ScrollBarPanningThumbBackground}" BorderThi…` |

### 5.77 `ScrollViewer`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 样式级 | generic.xaml:L9280 | `<Setter Property="Padding" Value="0" />` |
| `BorderThickness` | `0` | 样式级 | generic.xaml:L9281 | `<Setter Property="BorderThickness" Value="0" />` |

### 5.78 `SearchBox`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **6** · 模板定义始于 L7586。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0` | 模板级 | generic.xaml:L7742 | `<Setter Property="Margin" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `TextBox` | SearchTextBox | `BorderThickness` | `0` | ButtonStates.Disabled | generic.xaml:L7709 | `<TextBox x:Name="SearchTextBox" BorderThickness="0" Background="Transparent" FontFamily="{TemplateBinding FontFamily}" FontSize="{TemplateBinding FontSize}" Fon…` |
| `Border` | — | `BorderThickness` | `0,1,0,0` | ButtonStates.Enabled | generic.xaml:L7942 | `<Border Grid.Column="1" BorderBrush="{ThemeResource SearchBoxSeparatorSuggestionForegroundThemeBrush}" BorderThickness="0,1,0,0" VerticalAlignment="Center" />` |

### 5.79 `SemanticZoom`

条目数：A 模板/样式级 **2**，B 视觉状态 **0**，C 元素属性 **8**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 样式级 | generic.xaml:L11157 | `<Setter Property="BorderThickness" Value="0" />` |
| `BorderThickness` | `0` | 样式级 | generic.xaml:L24726 | `<Setter Property="BorderThickness" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | ZoomOutButton | `Height` | `12` | ButtonStates.DraggedPlaceholder | generic.xaml:L11282 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Padding` | `0` | ButtonStates.DraggedPlaceholder | generic.xaml:L11282 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Margin` | `0,0,19,19` | ButtonStates.DraggedPlaceholder | generic.xaml:L11282 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Width` | `12` | ButtonStates.DraggedPlaceholder | generic.xaml:L11282 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Height` | `12` | .InputModeDefault | generic.xaml:L24851 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Padding` | `0` | .InputModeDefault | generic.xaml:L24851 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Margin` | `0,0,19,19` | .InputModeDefault | generic.xaml:L24851 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |
| `Button` | ZoomOutButton | `Width` | `12` | .InputModeDefault | generic.xaml:L24851 | `<Button x:Name="ZoomOutButton" IsTabStop="False" Margin="0,0,19,19" HorizontalAlignment="Right" VerticalAlignment="Bottom" UseSystemFocusVisuals="{StaticResourc…` |

### 5.80 `SettingsFlyout`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **7**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `12,0,12,12` | 样式级 | generic.xaml:L13710 | `<Setter Property="Padding" Value="12,0,12,12" />` |
| `MaxWidth` | `646` | 样式级 | generic.xaml:L13719 | `<Setter Property="MaxWidth" Value="646" />` |
| `MinWidth` | `320` | 样式级 | generic.xaml:L13720 | `<Setter Property="MinWidth" Value="320" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | BackButton | `Width` | `32` | ButtonStates.Right | generic.xaml:L13823 | `<Button x:Name="BackButton" Margin="12,8,8,8" Height="32" Width="32" Style="{StaticResource BackButtonStyle}" VerticalAlignment="Top" />` |
| `Button` | BackButton | `Height` | `32` | ButtonStates.Right | generic.xaml:L13823 | `<Button x:Name="BackButton" Margin="12,8,8,8" Height="32" Width="32" Style="{StaticResource BackButtonStyle}" VerticalAlignment="Top" />` |
| `Button` | BackButton | `Margin` | `12,8,8,8` | ButtonStates.Right | generic.xaml:L13823 | `<Button x:Name="BackButton" Margin="12,8,8,8" Height="32" Width="32" Style="{StaticResource BackButtonStyle}" VerticalAlignment="Top" />` |
| `TextBlock` | — | `Margin` | `0,6,0,10` | ButtonStates.Right | generic.xaml:L13829 | `<TextBlock Text="{TemplateBinding Title}" Foreground="{TemplateBinding Foreground}" Style="{StaticResource TitleTextBlockStyle}" TextWrapping="NoWrap" Grid.Colu…` |
| `Image` | — | `Margin` | `0,12,12,12` | ButtonStates.Right | generic.xaml:L13836 | `<Image Height="24" Width="24" Source="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.IconSource}" Grid.Column="2" Margin="0,12,…` |
| `Image` | — | `Width` | `24` | ButtonStates.Right | generic.xaml:L13836 | `<Image Height="24" Width="24" Source="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.IconSource}" Grid.Column="2" Margin="0,12,…` |
| `Image` | — | `Height` | `24` | ButtonStates.Right | generic.xaml:L13836 | `<Image Height="24" Width="24" Source="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.IconSource}" Grid.Column="2" Margin="0,12,…` |

### 5.81 `Slider`

条目数：A 模板/样式级 **4**，B 视觉状态 **0**，C 元素属性 **35**，引用资源 **2** · 模板定义始于 L12603。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `0` | 模板级 | generic.xaml:L12603 | `<Setter Property="BorderThickness" Value="0" />` |
| `BorderThickness` | `0` | 模板级 | generic.xaml:L15383 | `<Setter Property="BorderThickness" Value="0" />` |
| `BorderThickness` | `1` | 模板级 | generic.xaml:L15404 | `<Setter Property="BorderThickness" Value="1" />` |
| `BorderThickness` | `0` | 模板级 | generic.xaml:L20325 | `<Setter Property="BorderThickness" Value="0" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `TickBar` | TopTickBar | `Margin` | `0,0,0,4` | ButtonStates.FocusDisengaged | generic.xaml:L12786 | `<TickBar x:Name="TopTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Height="{ThemeResource SliderOutsideTickBarThemeHeight}" VerticalAl…` |
| `TickBar` | BottomTickBar | `Margin` | `0,4,0,0` | ButtonStates.FocusDisengaged | generic.xaml:L12799 | `<TickBar x:Name="BottomTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Height="{ThemeResource SliderOutsideTickBarThemeHeight}" Vertica…` |
| `Thumb` | HorizontalThumb | `Margin` | `-14,-6,-14,-6` | ButtonStates.FocusDisengaged | generic.xaml:L12807 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Height="24" Width="8" Grid.Row="0" Grid.RowSpan=…` |
| `Thumb` | HorizontalThumb | `Width` | `8` | ButtonStates.FocusDisengaged | generic.xaml:L12807 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Height="24" Width="8" Grid.Row="0" Grid.RowSpan=…` |
| `Thumb` | HorizontalThumb | `Height` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L12807 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Height="24" Width="8" Grid.Row="0" Grid.RowSpan=…` |
| `TickBar` | LeftTickBar | `Margin` | `0,0,4,0` | ButtonStates.FocusDisengaged | generic.xaml:L12840 | `<TickBar x:Name="LeftTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Width="{ThemeResource SliderOutsideTickBarThemeHeight}" Horizontal…` |
| `TickBar` | RightTickBar | `Margin` | `4,0,0,0` | ButtonStates.FocusDisengaged | generic.xaml:L12853 | `<TickBar x:Name="RightTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Width="{ThemeResource SliderOutsideTickBarThemeHeight}" Horizonta…` |
| `Thumb` | VerticalThumb | `Margin` | `-6,-14,-6,-14` | ButtonStates.FocusDisengaged | generic.xaml:L12861 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Thumb` | VerticalThumb | `Height` | `8` | ButtonStates.FocusDisengaged | generic.xaml:L12861 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Thumb` | VerticalThumb | `Width` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L12861 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Grid` | HorizontalTemplate | `MinHeight` | `44` | ButtonStates.FocusDisengaged | generic.xaml:L15546 | `<Grid x:Name="HorizontalTemplate" MinHeight="44">` |
| `RowDefinition` | — | `Height` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L15553 | `<RowDefinition Height="18" />` |
| `RowDefinition` | — | `Height` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L15555 | `<RowDefinition Height="18" />` |
| `TickBar` | TopTickBar | `Margin` | `0,0,0,4` | ButtonStates.FocusDisengaged | generic.xaml:L15569 | `<TickBar x:Name="TopTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Height="{ThemeResource SliderOutsideTickBarThemeHeight}" VerticalAl…` |
| `TickBar` | BottomTickBar | `Margin` | `0,4,0,0` | ButtonStates.FocusDisengaged | generic.xaml:L15582 | `<TickBar x:Name="BottomTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Height="{ThemeResource SliderOutsideTickBarThemeHeight}" Vertica…` |
| `Thumb` | HorizontalThumb | `Margin` | `-14,-6,-14,-6` | ButtonStates.FocusDisengaged | generic.xaml:L15590 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" Height="24" Width="24" Grid.Row="0" Grid.RowSpan="3" Grid.Column="1" FocusVisualMargin…` |
| `Thumb` | HorizontalThumb | `Width` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L15590 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" Height="24" Width="24" Grid.Row="0" Grid.RowSpan="3" Grid.Column="1" FocusVisualMargin…` |
| `Thumb` | HorizontalThumb | `Height` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L15590 | `<Thumb x:Name="HorizontalThumb" Style="{StaticResource SliderThumbStyle}" Height="24" Width="24" Grid.Row="0" Grid.RowSpan="3" Grid.Column="1" FocusVisualMargin…` |
| `Grid` | — | `Width` | `192` | ButtonStates.FocusDisengaged | generic.xaml:L15605 | `<Grid Height="112" Width="192">` |
| `Grid` | — | `Height` | `112` | ButtonStates.FocusDisengaged | generic.xaml:L15605 | `<Grid Height="112" Width="192">` |
| `TextBlock` | TimeElapsedPreview | `Margin` | `6,1,6,3` | ButtonStates.FocusDisengaged | generic.xaml:L15610 | `<TextBlock x:Name="TimeElapsedPreview" Margin="6,1,6,3" Style="{StaticResource BodyTextBlockStyle}" IsTextScaleFactorEnabled="True" Foreground="{ThemeResource S…` |
| `Grid` | VerticalTemplate | `MinWidth` | `44` | ButtonStates.FocusDisengaged | generic.xaml:L15620 | `<Grid x:Name="VerticalTemplate" MinWidth="44" Visibility="Collapsed">` |
| `ColumnDefinition` | — | `Width` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L15627 | `<ColumnDefinition Width="18" />` |
| `ColumnDefinition` | — | `Width` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L15629 | `<ColumnDefinition Width="18" />` |
| `TickBar` | LeftTickBar | `Margin` | `0,0,4,0` | ButtonStates.FocusDisengaged | generic.xaml:L15640 | `<TickBar x:Name="LeftTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Width="{ThemeResource SliderOutsideTickBarThemeHeight}" Horizontal…` |
| `TickBar` | RightTickBar | `Margin` | `4,0,0,0` | ButtonStates.FocusDisengaged | generic.xaml:L15653 | `<TickBar x:Name="RightTickBar" Visibility="Collapsed" Fill="{ThemeResource SliderTickBarFill}" Width="{ThemeResource SliderOutsideTickBarThemeHeight}" Horizonta…` |
| `Thumb` | VerticalThumb | `Margin` | `-6,-14,-6,-14` | ButtonStates.FocusDisengaged | generic.xaml:L15661 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Thumb` | VerticalThumb | `Height` | `8` | ButtonStates.FocusDisengaged | generic.xaml:L15661 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Thumb` | VerticalThumb | `Width` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L15661 | `<Thumb x:Name="VerticalThumb" Style="{StaticResource SliderThumbStyle}" DataContext="{TemplateBinding Value}" Width="24" Height="8" Grid.Row="1" Grid.Column="0"…` |
| `Grid` | — | `Margin` | `0,5,0,0` | ButtonStates.HorizontalLine | generic.xaml:L20322 | `<Grid Margin="0,5,0,0">` |
| `Grid` | HorizontalTemplate | `MinHeight` | `44` | ButtonStates.FocusDisengaged | generic.xaml:L20384 | `<Grid x:Name="HorizontalTemplate" MinHeight="44">` |
| `RowDefinition` | — | `Height` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L20391 | `<RowDefinition Height="18" />` |
| `RowDefinition` | — | `Height` | `18` | ButtonStates.FocusDisengaged | generic.xaml:L20393 | `<RowDefinition Height="18" />` |
| `Thumb` | HorizontalThumb | `Width` | `8` | ButtonStates.FocusDisengaged | generic.xaml:L20397 | `<Thumb x:Name="HorizontalThumb" AutomationProperties.AccessibilityView="Raw" Grid.Column="1" DataContext="{TemplateBinding Value}" Height="24" Grid.Row="0" Grid…` |
| `Thumb` | HorizontalThumb | `Height` | `24` | ButtonStates.FocusDisengaged | generic.xaml:L20397 | `<Thumb x:Name="HorizontalThumb" AutomationProperties.AccessibilityView="Raw" Grid.Column="1" DataContext="{TemplateBinding Value}" Height="24" Grid.Row="0" Grid…` |

### 5.82 `SplitButton`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **3** · 模板定义始于 L21681。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Button` | SecondaryButton | `Padding` | `0,0,8,0` | ButtonStates.SecondaryButtonRight | generic.xaml:L21903 | `<Button x:Name="SecondaryButton" Grid.Column="1" Foreground="{TemplateBinding Foreground}" Background="{TemplateBinding Background}" BorderThickness="{TemplateB…` |

### 5.83 `SplitView`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | HCPaneBorder | `Width` | `1` | ButtonStates.OverlayNotVisible | generic.xaml:L17515 | `<Rectangle x:Name="HCPaneBorder" x:DeferLoadStrategy="Lazy" Visibility="Collapsed" Fill="{ThemeResource SystemControlForegroundTransparentBrush}" Width="1" Hori…` |

### 5.84 `StackPanel`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0,0,0,39` | 样式级 | generic.xaml:L15245 | `<Setter Property="Margin" Value="0,0,0,39" />` |

### 5.85 `SwipeControl`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **2**。

### 5.86 `TextBlock`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Margin` | `0,0,0,2` | 样式级 | generic.xaml:L17645 | `<Setter Property="Margin" Value="0,0,0,2" />` |

### 5.87 `TextBox`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **4**，引用资源 **6**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ContentPresenter` | HeaderContentPresenter | `Margin` | `0,4,0,4` | ButtonStates.Disabled | generic.xaml:L7661 | `<ContentPresenter x:Name="HeaderContentPresenter" x:DeferLoadStrategy="Lazy" Visibility="Collapsed" Grid.Row="0" Foreground="{ThemeResource TextBoxForegroundHea…` |
| `Button` | DeleteButton | `MinWidth` | `34` | .ButtonCollapsed | generic.xaml:L29052 | `<Button x:Name="DeleteButton" Grid.Row="1" Grid.Column="1" Style="{StaticResource DeleteButtonStyle}" BorderThickness="{TemplateBinding BorderThickness}" Margin…` |
| `Button` | DeleteButton | `MinWidth` | `34` | .ButtonCollapsed | generic.xaml:L29374 | `<Button x:Name="DeleteButton" Grid.Row="1" Style="{StaticResource DeleteButtonStyle}" BorderThickness="{TemplateBinding BorderThickness}" IsTabStop="False" Grid…` |
| `Button` | QueryButton | `MinWidth` | `34` | .ButtonCollapsed | generic.xaml:L29385 | `<Button x:Name="QueryButton" Grid.Row="1" Style="{StaticResource QueryButtonStyle}" BorderThickness="{TemplateBinding BorderThickness}" IsTabStop="False" Grid.C…` |

### 5.88 `Thumb`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **0**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `BorderThickness` | `1` | 样式级 | generic.xaml:L8649 | `<Setter Property="BorderThickness" Value="1" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | — | `CornerRadius` | `4` | ButtonStates.Inactive | generic.xaml:L12608 | `<Border Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="…` |
| `Border` | — | `CornerRadius` | `4` | ButtonStates.HorizontalLine | generic.xaml:L20330 | `<Border BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" Background="{TemplateBinding Background}" CornerRadius="…` |

### 5.89 `TimePicker`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **2**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Rectangle` | FirstColumnDivider | `Width` | `2` | ButtonStates.HasTime | generic.xaml:L12036 | `<Rectangle x:Name="FirstColumnDivider" Fill="{ThemeResource TimePickerSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="1" />` |
| `Rectangle` | SecondColumnDivider | `Width` | `2` | ButtonStates.HasTime | generic.xaml:L12050 | `<Rectangle x:Name="SecondColumnDivider" Fill="{ThemeResource TimePickerSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="3" />` |

### 5.90 `TimePickerFlyoutPresenter`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **4**，引用资源 **1**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Width` | `242` | 样式级 | generic.xaml:L14882 | `<Setter Property="Width" Value="242" />` |
| `MinWidth` | `242` | 样式级 | generic.xaml:L14883 | `<Setter Property="MinWidth" Value="242" />` |
| `MaxHeight` | `398` | 样式级 | generic.xaml:L14884 | `<Setter Property="MaxHeight" Value="398" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Border` | Background | `MaxHeight` | `398` | ButtonStates.Portrait | generic.xaml:L14895 | `<Border x:Name="Background" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickn…` |
| `Rectangle` | FirstPickerSpacing | `Width` | `2` | ButtonStates.Portrait | generic.xaml:L14924 | `<Rectangle x:Name="FirstPickerSpacing" Fill="{ThemeResource TimePickerFlyoutPresenterSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="1" />` |
| `Rectangle` | SecondPickerSpacing | `Width` | `2` | ButtonStates.Portrait | generic.xaml:L14930 | `<Rectangle x:Name="SecondPickerSpacing" Fill="{ThemeResource TimePickerFlyoutPresenterSpacerFill}" HorizontalAlignment="Center" Width="2" Grid.Column="3" />` |
| `Rectangle` | — | `Height` | `2` | ButtonStates.Portrait | generic.xaml:L14943 | `<Rectangle Height="2" VerticalAlignment="Top" Fill="{ThemeResource TimePickerFlyoutPresenterSpacerFill}" Grid.ColumnSpan="2" />` |

### 5.91 `ToggleButton`

条目数：A 模板/样式级 **1**，B 视觉状态 **0**，C 元素属性 **0**，引用资源 **9**。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0,0,0,0` | 样式级 | generic.xaml:L17634 | `<Setter Property="Padding" Value="0,0,0,0" />` |

### 5.92 `ToggleMenuFlyoutItem`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **5**，引用资源 **4** · 模板定义始于 L25309。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `FontIcon` | CheckGlyph | `Margin` | `0,0,12,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25352 | `<FontIcon x:Name="CheckGlyph" FontFamily="{ThemeResource SymbolThemeFontFamily}" Glyph="&#xE001;" FontSize="16" Foreground="{ThemeResource ToggleMenuFlyoutItemC…` |
| `FontIcon` | CheckGlyph | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25352 | `<FontIcon x:Name="CheckGlyph" FontFamily="{ThemeResource SymbolThemeFontFamily}" Glyph="&#xE001;" FontSize="16" Foreground="{ThemeResource ToggleMenuFlyoutItemC…` |
| `Viewbox` | IconRoot | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25360 | `<Viewbox x:Name="IconRoot" Grid.Column="1" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `Viewbox` | IconRoot | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25360 | `<Viewbox x:Name="IconRoot" Grid.Column="1" HorizontalAlignment="Left" VerticalAlignment="Center" Width="16" Height="16" Visibility="Collapsed">` |
| `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `24,0,0,0` | .KeyboardAcceleratorTextCollapsed | generic.xaml:L25377 | `<TextBlock x:Name="KeyboardAcceleratorTextBlock" Grid.Column="2" Style="{ThemeResource CaptionTextBlockStyle}" Text="{TemplateBinding KeyboardAcceleratorTextOve…` |

### 5.93 `ToggleSwitch`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **13**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | — | `MaxWidth` | `12` | ButtonStates.Off | generic.xaml:L13157 | `<ColumnDefinition Width="12" MaxWidth="12" />` |
| `ColumnDefinition` | — | `Width` | `12` | ButtonStates.Off | generic.xaml:L13157 | `<ColumnDefinition Width="12" MaxWidth="12" />` |
| `Grid` | SwitchAreaGrid | `Margin` | `0,5` | ButtonStates.Off | generic.xaml:L13160 | `<Grid x:Name="SwitchAreaGrid" Grid.RowSpan="3" Grid.ColumnSpan="3" Margin="0,5" Control.IsTemplateFocusTarget="True" Background="{ThemeResource ToggleSwitchCont…` |
| `Rectangle` | OuterBorder | `Width` | `44` | ButtonStates.Off | generic.xaml:L13188 | `<Rectangle x:Name="OuterBorder" Grid.Row="1" Height="20" Width="44" RadiusX="10" RadiusY="10" Fill="{ThemeResource ToggleSwitchFillOff}" Stroke="{ThemeResource …` |
| `Rectangle` | OuterBorder | `Height` | `20` | ButtonStates.Off | generic.xaml:L13188 | `<Rectangle x:Name="OuterBorder" Grid.Row="1" Height="20" Width="44" RadiusX="10" RadiusY="10" Fill="{ThemeResource ToggleSwitchFillOff}" Stroke="{ThemeResource …` |
| `Rectangle` | SwitchKnobBounds | `Width` | `44` | ButtonStates.Off | generic.xaml:L13197 | `<Rectangle x:Name="SwitchKnobBounds" Grid.Row="1" Height="20" Width="44" RadiusX="10" RadiusY="10" Fill="{ThemeResource ToggleSwitchFillOn}" Stroke="{ThemeResou…` |
| `Rectangle` | SwitchKnobBounds | `Height` | `20` | ButtonStates.Off | generic.xaml:L13197 | `<Rectangle x:Name="SwitchKnobBounds" Grid.Row="1" Height="20" Width="44" RadiusX="10" RadiusY="10" Fill="{ThemeResource ToggleSwitchFillOn}" Stroke="{ThemeResou…` |
| `Grid` | SwitchKnob | `Height` | `20` | ButtonStates.Off | generic.xaml:L13207 | `<Grid x:Name="SwitchKnob" Grid.Row="1" HorizontalAlignment="Left" Width="20" Height="20">` |
| `Grid` | SwitchKnob | `Width` | `20` | ButtonStates.Off | generic.xaml:L13207 | `<Grid x:Name="SwitchKnob" Grid.Row="1" HorizontalAlignment="Left" Width="20" Height="20">` |
| `Ellipse` | SwitchKnobOn | `Height` | `10` | ButtonStates.Off | generic.xaml:L13212 | `<Ellipse x:Name="SwitchKnobOn" Fill="{ThemeResource ToggleSwitchKnobFillOn}" Width="10" Height="10" Opacity="0" />` |
| `Ellipse` | SwitchKnobOn | `Width` | `10` | ButtonStates.Off | generic.xaml:L13212 | `<Ellipse x:Name="SwitchKnobOn" Fill="{ThemeResource ToggleSwitchKnobFillOn}" Width="10" Height="10" Opacity="0" />` |
| `Ellipse` | SwitchKnobOff | `Height` | `10` | ButtonStates.Off | generic.xaml:L13217 | `<Ellipse x:Name="SwitchKnobOff" Fill="{ThemeResource ToggleSwitchKnobFillOff}" Width="10" Height="10" />` |
| `Ellipse` | SwitchKnobOff | `Width` | `10` | ButtonStates.Off | generic.xaml:L13217 | `<Ellipse x:Name="SwitchKnobOff" Fill="{ThemeResource ToggleSwitchKnobFillOff}" Width="10" Height="10" />` |

### 5.94 `ToolTip`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **1**，引用资源 **2**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ContentPresenter` | LayoutRoot | `MaxWidth` | `320` | ButtonStates.Off | generic.xaml:L13256 | `<ContentPresenter x:Name="LayoutRoot" BorderThickness="{TemplateBinding BorderThickness}" Background="{TemplateBinding Background}" BackgroundSizing="OuterBorde…` |

### 5.95 `TreeViewItem`

条目数：A 模板/样式级 **3**，B 视觉状态 **0**，C 元素属性 **10**，引用资源 **2** · 模板定义始于 L21262。

**A. 模板 / 样式级 `Setter`（写死）**

| 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|
| `Padding` | `0` | 样式级 | generic.xaml:L21135 | `<Setter Property="Padding" Value="0" />` |
| `MinWidth` | `32` | 模板级 | generic.xaml:L21262 | `<Setter Property="MinWidth" Value="32" />` |
| `MinHeight` | `32` | 模板级 | generic.xaml:L21263 | `<Setter Property="MinHeight" Value="32" />` |

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `Grid` | ContentPresenterGrid | `Margin` | `0,0,0,0` | ButtonStates.Normal | generic.xaml:L21144 | `<Grid x:Name="ContentPresenterGrid" Margin="0,0,0,0" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{Tem…` |
| `CheckBox` | MultiSelectCheckBox | `Width` | `32` | ButtonStates.NotDragging | generic.xaml:L21478 | `<CheckBox x:Name="MultiSelectCheckBox" Width="32" VerticalAlignment="Stretch" Style="{StaticResource TreeViewMultiSelectCheckBox}" Visibility="Collapsed" IsTabS…` |
| `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | ButtonStates.NotDragging | generic.xaml:L21485 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | ButtonStates.NotDragging | generic.xaml:L21485 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | ButtonStates.NotDragging | generic.xaml:L21485 | `<Border x:Name="MultiArrangeOverlayTextBorder" Opacity="0" IsHitTestVisible="False" MinWidth="20" Height="20" VerticalAlignment="Center" HorizontalAlignment="Ce…` |
| `Grid` | ExpandCollapseChevron | `Padding` | `12,0,12,0` | ButtonStates.NotDragging | generic.xaml:L21508 | `<Grid x:Name="ExpandCollapseChevron" Padding="12,0,12,0" Width="Auto" Opacity="{TemplateBinding GlyphOpacity}" Background="Transparent">` |
| `TextBlock` | — | `Height` | `12` | ButtonStates.NotDragging | generic.xaml:L21513 | `<TextBlock Foreground="{TemplateBinding GlyphBrush}" Width="12" Height="12" Visibility="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TreeViewI…` |
| `TextBlock` | — | `Width` | `12` | ButtonStates.NotDragging | generic.xaml:L21513 | `<TextBlock Foreground="{TemplateBinding GlyphBrush}" Width="12" Height="12" Visibility="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TreeViewI…` |
| `TextBlock` | — | `Height` | `12` | ButtonStates.NotDragging | generic.xaml:L21521 | `<TextBlock Foreground="{TemplateBinding GlyphBrush}" Width="12" Height="12" Visibility="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TreeViewI…` |
| `TextBlock` | — | `Width` | `12` | ButtonStates.NotDragging | generic.xaml:L21521 | `<TextBlock Foreground="{TemplateBinding GlyphBrush}" Width="12" Height="12" Visibility="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TreeViewI…` |

### 5.96 `TwoPaneView`

条目数：A 模板/样式级 **0**，B 视觉状态 **0**，C 元素属性 **3**，引用资源 **0**。

**C. 元素属性（写死）**

| 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 | 原行 |
|---|---|---|---|---|---|---|
| `ColumnDefinition` | PART_ColumnMiddle | `Width` | `0` | ButtonStates.ViewMode_LeftRight | generic.xaml:L21633 | `<ColumnDefinition x:Name="PART_ColumnMiddle" Width="0" />` |
| `RowDefinition` | PART_RowMiddle | `Height` | `0` | ButtonStates.ViewMode_LeftRight | generic.xaml:L21639 | `<RowDefinition x:Name="PART_RowMiddle" Height="0" />` |
| `RowDefinition` | PART_RowBottom | `Height` | `0` | ButtonStates.ViewMode_LeftRight | generic.xaml:L21640 | `<RowDefinition x:Name="PART_RowBottom" Height="0" />` |

### 5.97 `themeresources.xaml` 里写死的几何（Style / ControlTemplate 内）

> 该文件主题字典之外还有大量 Style / ControlTemplate（L5879–L13125）。其中写死的几何 Setter **123** 条、
> 写死的数值元素属性 **161** 条。这些与 `generic.xaml` 是**两套并行的模板定义**，Rust 侧若以本文件为准，需要一并消费。

> 涉及 30 个 TargetType：`（无上下文）`、`AppBarButton`、`AppBarToggleButton`、`Button`、`ButtonBase`、`CalendarView`、`CalendarViewDayItem`、`CheckBox`、`CommandBar`、`CommandBarOverflowPresenter`、`ContentControl`、`Control`、`FlyoutPresenter`、`Grid`、`GridViewItem`、`InkToolbarFlyoutItem`、`ListViewItem`、`MenuFlyoutItem`、`MenuFlyoutSubItem`、`NavigationViewItem`、`NavigationViewItemPresenter`、`Rectangle`、`SemanticZoom`、`SplitButton`、`StackPanel`、`TextBlock`、`TextBox`、`ToggleButton`、`ToggleMenuFlyoutItem`、`TreeViewItem`

**A. 模板 / 样式级 `Setter`（写死）**

| 控件(TargetType) | 属性 | 值 | 层级 | 文件:行号 | 原行 |
|---|---|---|---|---|---|
| `AppBarButton` | `Width` | `68` | 样式级 | themeresources.xaml:L6106 | `<Setter Property="Width" Value="68" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6163 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6171 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L6175 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6180 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Width` | `16` | 模板级 | themeresources.xaml:L6183 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `AppBarButton` | `Height` | `16` | 模板级 | themeresources.xaml:L6184 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `AppBarButton` | `Margin` | `12,0,12,0` | 模板级 | themeresources.xaml:L6185 | `<Setter Target="ContentViewbox.Margin" Value="12,0,12,0" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L6188 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6193 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Width` | `16` | 模板级 | themeresources.xaml:L6196 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `AppBarButton` | `Height` | `16` | 模板级 | themeresources.xaml:L6197 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L6198 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `Margin` | `76,0,12,0` | 模板级 | themeresources.xaml:L6201 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `AppBarButton` | `Width` | `Auto` | 样式级 | themeresources.xaml:L9642 | `<Setter Property="Width" Value="Auto" />` |
| `AppBarButton` | `Width` | `Auto` | 样式级 | themeresources.xaml:L9646 | `<Setter Property="Width" Value="Auto" />` |
| `AppBarButton` | `MinWidth` | `68` | 样式级 | themeresources.xaml:L11775 | `<Setter Property="MinWidth" Value="68" />` |
| `AppBarButton` | `Width` | `Auto` | 样式级 | themeresources.xaml:L11776 | `<Setter Property="Width" Value="Auto" />` |
| `AppBarButton` | `MinHeight` | `40` | 样式级 | themeresources.xaml:L11777 | `<Setter Property="MinHeight" Value="40" />` |
| `AppBarButton` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L11917 | `<Setter Property="BorderThickness" Value="0" />` |
| `AppBarButton` | `Width` | `40` | 样式级 | themeresources.xaml:L11922 | `<Setter Property="Width" Value="40" />` |
| `AppBarButton` | `Height` | `40` | 样式级 | themeresources.xaml:L11923 | `<Setter Property="Height" Value="40" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L11938 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L11945 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L11948 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L11953 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Width` | `16` | 模板级 | themeresources.xaml:L11956 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `AppBarButton` | `Height` | `16` | 模板级 | themeresources.xaml:L11957 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `AppBarButton` | `Margin` | `12,0,12,0` | 模板级 | themeresources.xaml:L11958 | `<Setter Target="ContentViewbox.Margin" Value="12,0,12,0" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L11960 | `<Setter Target="OverflowTextLabel.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L11965 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarButton` | `Width` | `16` | 模板级 | themeresources.xaml:L11968 | `<Setter Target="ContentViewbox.Width" Value="16" />` |
| `AppBarButton` | `Height` | `16` | 模板级 | themeresources.xaml:L11969 | `<Setter Target="ContentViewbox.Height" Value="16" />` |
| `AppBarButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L11970 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `AppBarButton` | `Margin` | `76,0,12,0` | 模板级 | themeresources.xaml:L11972 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `AppBarButton` | `Padding` | `0,9,0,11` | 模板级 | themeresources.xaml:L12039 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `AppBarButton` | `Padding` | `0,9,0,11` | 模板级 | themeresources.xaml:L12044 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `AppBarToggleButton` | `Width` | `68` | 样式级 | themeresources.xaml:L6331 | `<Setter Property="Width" Value="68" />` |
| `AppBarToggleButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6388 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarToggleButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L6398 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarToggleButton` | `MaxWidth` | `16` | 模板级 | themeresources.xaml:L6402 | `<Setter Target="ContentViewbox.MaxWidth" Value="16" />` |
| `AppBarToggleButton` | `MaxHeight` | `16` | 模板级 | themeresources.xaml:L6403 | `<Setter Target="ContentViewbox.MaxHeight" Value="16" />` |
| `AppBarToggleButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L6404 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `AppBarToggleButton` | `Margin` | `76,0,12,0` | 模板级 | themeresources.xaml:L6409 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `AppBarToggleButton` | `Width` | `Auto` | 样式级 | themeresources.xaml:L9649 | `<Setter Property="Width" Value="Auto" />` |
| `AppBarToggleButton` | `Width` | `Auto` | 样式级 | themeresources.xaml:L9653 | `<Setter Property="Width" Value="Auto" />` |
| `AppBarToggleButton` | `Width` | `40` | 样式级 | themeresources.xaml:L12094 | `<Setter Property="Width" Value="40" />` |
| `AppBarToggleButton` | `Height` | `40` | 样式级 | themeresources.xaml:L12095 | `<Setter Property="Height" Value="40" />` |
| `AppBarToggleButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L12110 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarToggleButton` | `MinHeight` | `0` | 模板级 | themeresources.xaml:L12119 | `<Setter Target="ContentRoot.MinHeight" Value="0" />` |
| `AppBarToggleButton` | `MaxWidth` | `16` | 模板级 | themeresources.xaml:L12123 | `<Setter Target="ContentViewbox.MaxWidth" Value="16" />` |
| `AppBarToggleButton` | `MaxHeight` | `16` | 模板级 | themeresources.xaml:L12124 | `<Setter Target="ContentViewbox.MaxHeight" Value="16" />` |
| `AppBarToggleButton` | `Margin` | `38,0,12,0` | 模板级 | themeresources.xaml:L12125 | `<Setter Target="ContentViewbox.Margin" Value="38,0,12,0" />` |
| `AppBarToggleButton` | `Margin` | `76,0,12,0` | 模板级 | themeresources.xaml:L12129 | `<Setter Target="OverflowTextLabel.Margin" Value="76,0,12,0" />` |
| `AppBarToggleButton` | `Padding` | `0,9,0,11` | 模板级 | themeresources.xaml:L12258 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `AppBarToggleButton` | `Margin` | `12,10,12,10` | 模板级 | themeresources.xaml:L12259 | `<Setter Target="OverflowCheckGlyph.Margin" Value="12,10,12,10" />` |
| `AppBarToggleButton` | `Padding` | `0,9,0,11` | 模板级 | themeresources.xaml:L12264 | `<Setter Target="OverflowTextLabel.Padding" Value="0,9,0,11" />` |
| `AppBarToggleButton` | `Margin` | `12,10,12,10` | 模板级 | themeresources.xaml:L12265 | `<Setter Target="OverflowCheckGlyph.Margin" Value="12,10,12,10" />` |
| `Button` | `Padding` | `0,0,9,0` | 样式级 | themeresources.xaml:L6647 | `<Setter Property="Padding" Value="0,0,9,0" />` |
| `Button` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L6789 | `<Setter Property="BorderThickness" Value="0" />` |
| `Button` | `Padding` | `0,0,9,0` | 样式级 | themeresources.xaml:L6790 | `<Setter Property="Padding" Value="0,0,9,0" />` |
| `Button` | `Height` | `32` | 样式级 | themeresources.xaml:L7594 | `<Setter Property="Height" Value="32" />` |
| `Button` | `Width` | `32` | 样式级 | themeresources.xaml:L7595 | `<Setter Property="Width" Value="32" />` |
| `Button` | `Padding` | `0` | 样式级 | themeresources.xaml:L10428 | `<Setter Property="Padding" Value="0" />` |
| `Button` | `MinHeight` | `40` | 样式级 | themeresources.xaml:L10505 | `<Setter Property="MinHeight" Value="40" />` |
| `Button` | `Padding` | `12,0` | 样式级 | themeresources.xaml:L10515 | `<Setter Property="Padding" Value="12,0" />` |
| `Button` | `Padding` | `0` | 样式级 | themeresources.xaml:L12336 | `<Setter Property="Padding" Value="0" />` |
| `Button` | `Width` | `40` | 样式级 | themeresources.xaml:L12337 | `<Setter Property="Width" Value="40" />` |
| `Button` | `Height` | `40` | 样式级 | themeresources.xaml:L12338 | `<Setter Property="Height" Value="40" />` |
| `ButtonBase` | `MinWidth` | `0` | 样式级 | themeresources.xaml:L7479 | `<Setter Property="MinWidth" Value="0" />` |
| `ButtonBase` | `MinHeight` | `0` | 样式级 | themeresources.xaml:L7480 | `<Setter Property="MinHeight" Value="0" />` |
| `CalendarView` | `BorderThickness` | `1` | 样式级 | themeresources.xaml:L7672 | `<Setter Property="BorderThickness" Value="1" />` |
| `CalendarViewDayItem` | `MinWidth` | `40` | 样式级 | themeresources.xaml:L7615 | `<Setter Property="MinWidth" Value="40" />` |
| `CalendarViewDayItem` | `MinHeight` | `40` | 样式级 | themeresources.xaml:L7616 | `<Setter Property="MinHeight" Value="40" />` |
| `CalendarViewDayItem` | `Margin` | `1` | 样式级 | themeresources.xaml:L7617 | `<Setter Property="Margin" Value="1" />` |
| `CalendarViewDayItem` | `Padding` | `0, 0, 0, 4` | 样式级 | themeresources.xaml:L7618 | `<Setter Property="Padding" Value="0, 0, 0, 4" />` |
| `CommandBar` | `Width` | `Auto` | 模板级 | themeresources.xaml:L9555 | `<Setter Target="ContentControlColumnDefinition.Width" Value="Auto" />` |
| `CommandBar` | `Width` | `*` | 模板级 | themeresources.xaml:L9556 | `<Setter Target="PrimaryItemsControlColumnDefinition.Width" Value="*" />` |
| `CommandBar` | `Width` | `NaN` | 模板级 | themeresources.xaml:L9626 | `<Setter Property="Width" Value="NaN" />` |
| `CommandBar` | `Width` | `Auto` | 模板级 | themeresources.xaml:L12984 | `<Setter Target="ContentControlColumnDefinition.Width" Value="Auto" />` |
| `CommandBar` | `Width` | `*` | 模板级 | themeresources.xaml:L12985 | `<Setter Target="PrimaryItemsControlColumnDefinition.Width" Value="*" />` |
| `CommandBar` | `Width` | `NaN` | 模板级 | themeresources.xaml:L13042 | `<Setter Property="Width" Value="NaN" />` |
| `CommandBarOverflowPresenter` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L12301 | `<Setter Property="BorderThickness" Value="0" />` |
| `CommandBarOverflowPresenter` | `MinWidth` | `136` | 样式级 | themeresources.xaml:L12302 | `<Setter Property="MinWidth" Value="136" />` |
| `CommandBarOverflowPresenter` | `MaxWidth` | `440` | 样式级 | themeresources.xaml:L12303 | `<Setter Property="MaxWidth" Value="440" />` |
| `CommandBarOverflowPresenter` | `MaxHeight` | `480` | 样式级 | themeresources.xaml:L12304 | `<Setter Property="MaxHeight" Value="480" />` |
| `ContentControl` | `Margin` | `12,5,0,11` | 样式级 | themeresources.xaml:L11088 | `<Setter Property="Margin" Value="12,5,0,11" />` |
| `Control` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L6974 | `<Setter Property="BorderThickness" Value="0" />` |
| `FlyoutPresenter` | `Padding` | `0,0,0,0` | 样式级 | themeresources.xaml:L8066 | `<Setter Property="Padding" Value="0,0,0,0" />` |
| `FlyoutPresenter` | `BorderThickness` | `1` | 样式级 | themeresources.xaml:L8069 | `<Setter Property="BorderThickness" Value="1" />` |
| `FlyoutPresenter` | `MinWidth` | `0` | 样式级 | themeresources.xaml:L8070 | `<Setter Property="MinWidth" Value="0" />` |
| `FlyoutPresenter` | `MinHeight` | `0` | 样式级 | themeresources.xaml:L8071 | `<Setter Property="MinHeight" Value="0" />` |
| `Grid` | `BorderThickness` | `1` | 样式级 | themeresources.xaml:L7026 | `<Setter Property="BorderThickness" Value="1" />` |
| `GridViewItem` | `Margin` | `0,0,4,4` | 样式级 | themeresources.xaml:L7102 | `<Setter Property="Margin" Value="0,0,4,4" />` |
| `GridViewItem` | `Margin` | `0,0,4,4` | 样式级 | themeresources.xaml:L9732 | `<Setter Property="Margin" Value="0,0,4,4" />` |
| `InkToolbarFlyoutItem` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L8114 | `<Setter Property="BorderThickness" Value="0" />` |
| `InkToolbarFlyoutItem` | `MinWidth` | `136` | 样式级 | themeresources.xaml:L8117 | `<Setter Property="MinWidth" Value="136" />` |
| `InkToolbarFlyoutItem` | `Height` | `48` | 样式级 | themeresources.xaml:L8118 | `<Setter Property="Height" Value="48" />` |
| `ListViewItem` | `Padding` | `12,0,12,0` | 样式级 | themeresources.xaml:L9662 | `<Setter Property="Padding" Value="12,0,12,0" />` |
| `ListViewItem` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L11099 | `<Setter Property="BorderThickness" Value="0" />` |
| `ListViewItem` | `Padding` | `12,0,12,0` | 样式级 | themeresources.xaml:L11104 | `<Setter Property="Padding" Value="12,0,12,0" />` |
| `ListViewItem` | `Padding` | `8,0,0,0` | 样式级 | themeresources.xaml:L11819 | `<Setter Property="Padding" Value="8,0,0,0" />` |
| `ListViewItem` | `MinHeight` | `32` | 样式级 | themeresources.xaml:L11823 | `<Setter Property="MinHeight" Value="32" />` |
| `ListViewItem` | `MinWidth` | `120` | 样式级 | themeresources.xaml:L11824 | `<Setter Property="MinWidth" Value="120" />` |
| `NavigationViewItemPresenter` | `Width` | `16` | 模板级 | themeresources.xaml:L10674 | `<Setter Target="IconColumn.Width" Value="16" />` |
| `NavigationViewItemPresenter` | `Width` | `16` | 模板级 | themeresources.xaml:L10735 | `<Setter Target="IconColumn.Width" Value="16" />` |
| `NavigationViewItemPresenter` | `Width` | `48` | 模板级 | themeresources.xaml:L10903 | `<Setter Target="LayoutRoot.Width" Value="48" />` |
| `NavigationViewItemPresenter` | `Margin` | `4,0,4,4` | 模板级 | themeresources.xaml:L10905 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="4,0,4,4" />` |
| `NavigationViewItemPresenter` | `Margin` | `12,0` | 模板级 | themeresources.xaml:L10911 | `<Setter Target="ContentPresenter.Margin" Value="12,0" />` |
| `NavigationViewItemPresenter` | `Margin` | `12,0,12,4` | 模板级 | themeresources.xaml:L10912 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="12,0,12,4" />` |
| `NavigationViewItemPresenter` | `Width` | `48` | 模板级 | themeresources.xaml:L10967 | `<Setter Target="LayoutRoot.Width" Value="48" />` |
| `NavigationViewItemPresenter` | `Margin` | `4,0` | 模板级 | themeresources.xaml:L10969 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="4,0" />` |
| `NavigationViewItemPresenter` | `Margin` | `12,0` | 模板级 | themeresources.xaml:L10975 | `<Setter Target="ContentPresenter.Margin" Value="12,0" />` |
| `NavigationViewItemPresenter` | `Margin` | `12,0` | 模板级 | themeresources.xaml:L10976 | `<Setter Target="SelectionIndicatorGrid.Margin" Value="12,0" />` |
| `NavigationViewItemPresenter` | `Margin` | `16,0` | 模板级 | themeresources.xaml:L11059 | `<Setter Target="ContentPresenter.Margin" Value="16,0" />` |
| `Rectangle` | `Height` | `2` | 样式级 | themeresources.xaml:L8109 | `<Setter Property="Height" Value="2" />` |
| `SemanticZoom` | `BorderThickness` | `0` | 样式级 | themeresources.xaml:L9901 | `<Setter Property="BorderThickness" Value="0" />` |
| `StackPanel` | `Margin` | `0,0,0,39` | 样式级 | themeresources.xaml:L7537 | `<Setter Property="Margin" Value="0,0,0,39" />` |
| `TextBlock` | `Margin` | `0,0,0,2` | 样式级 | themeresources.xaml:L8095 | `<Setter Property="Margin" Value="0,0,0,2" />` |
| `ToggleButton` | `Padding` | `0,0,0,0` | 样式级 | themeresources.xaml:L8084 | `<Setter Property="Padding" Value="0,0,0,0" />` |
| `TreeViewItem` | `Padding` | `0` | 样式级 | themeresources.xaml:L8194 | `<Setter Property="Padding" Value="0" />` |
| `TreeViewItem` | `MinWidth` | `32` | 模板级 | themeresources.xaml:L8303 | `<Setter Property="MinWidth" Value="32" />` |
| `TreeViewItem` | `MinHeight` | `32` | 模板级 | themeresources.xaml:L8304 | `<Setter Property="MinHeight" Value="32" />` |

**B. 视觉状态内 `Setter`（写死）**

| 控件(TargetType) | 目标.属性 | 值 | 视觉状态组 | 视觉状态 | 文件:行号 |
|---|---|---|---|---|---|
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | themeresources.xaml:L6163 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtons** | themeresources.xaml:L6171 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtons** | themeresources.xaml:L6175 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6180 |
| `AppBarButton` | `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6183 |
| `AppBarButton` | `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6184 |
| `AppBarButton` | `ContentViewbox.Margin` | `12,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6185 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6188 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L6193 |
| `AppBarButton` | `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L6196 |
| `AppBarButton` | `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L6197 |
| `AppBarButton` | `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L6198 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L6201 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | themeresources.xaml:L11938 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtons** | themeresources.xaml:L11945 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtons** | themeresources.xaml:L11948 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L11953 |
| `AppBarButton` | `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L11956 |
| `AppBarButton` | `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L11957 |
| `AppBarButton` | `ContentViewbox.Margin` | `12,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L11958 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L11960 |
| `AppBarButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L11965 |
| `AppBarButton` | `ContentViewbox.Width` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L11968 |
| `AppBarButton` | `ContentViewbox.Height` | `16` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L11969 |
| `AppBarButton` | `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L11970 |
| `AppBarButton` | `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithToggleButtonsAndMenuIcons** | themeresources.xaml:L11972 |
| `AppBarButton` | `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **TouchInputMode** | themeresources.xaml:L12039 |
| `AppBarButton` | `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **GameControllerInputMode** | themeresources.xaml:L12044 |
| `AppBarToggleButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | themeresources.xaml:L6388 |
| `AppBarToggleButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6398 |
| `AppBarToggleButton` | `ContentViewbox.MaxWidth` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6402 |
| `AppBarToggleButton` | `ContentViewbox.MaxHeight` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6403 |
| `AppBarToggleButton` | `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6404 |
| `AppBarToggleButton` | `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L6409 |
| `AppBarToggleButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **Overflow** | themeresources.xaml:L12110 |
| `AppBarToggleButton` | `ContentRoot.MinHeight` | `0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L12119 |
| `AppBarToggleButton` | `ContentViewbox.MaxWidth` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L12123 |
| `AppBarToggleButton` | `ContentViewbox.MaxHeight` | `16` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L12124 |
| `AppBarToggleButton` | `ContentViewbox.Margin` | `38,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L12125 |
| `AppBarToggleButton` | `OverflowTextLabel.Margin` | `76,0,12,0` | ApplicationViewStates | **OverflowWithMenuIcons** | themeresources.xaml:L12129 |
| `AppBarToggleButton` | `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **TouchInputMode** | themeresources.xaml:L12258 |
| `AppBarToggleButton` | `OverflowCheckGlyph.Margin` | `12,10,12,10` | InputModeStates | **TouchInputMode** | themeresources.xaml:L12259 |
| `AppBarToggleButton` | `OverflowTextLabel.Padding` | `0,9,0,11` | InputModeStates | **GameControllerInputMode** | themeresources.xaml:L12264 |
| `AppBarToggleButton` | `OverflowCheckGlyph.Margin` | `12,10,12,10` | InputModeStates | **GameControllerInputMode** | themeresources.xaml:L12265 |
| `CommandBar` | `ContentControlColumnDefinition.Width` | `Auto` | DynamicOverflowStates | **DynamicOverflowEnabled** | themeresources.xaml:L9555 |
| `CommandBar` | `PrimaryItemsControlColumnDefinition.Width` | `*` | DynamicOverflowStates | **DynamicOverflowEnabled** | themeresources.xaml:L9556 |
| `CommandBar` | `ContentControlColumnDefinition.Width` | `Auto` | DynamicOverflowStates | **DynamicOverflowEnabled** | themeresources.xaml:L12984 |
| `CommandBar` | `PrimaryItemsControlColumnDefinition.Width` | `*` | DynamicOverflowStates | **DynamicOverflowEnabled** | themeresources.xaml:L12985 |
| `NavigationViewItemPresenter` | `IconColumn.Width` | `16` | IconStates | **IconCollapsed** | themeresources.xaml:L10674 |
| `NavigationViewItemPresenter` | `IconColumn.Width` | `16` | IconStates | **IconCollapsed** | themeresources.xaml:L10735 |
| `NavigationViewItemPresenter` | `LayoutRoot.Width` | `48` | NavigationViewIconPositionStates | **IconOnly** | themeresources.xaml:L10903 |
| `NavigationViewItemPresenter` | `SelectionIndicatorGrid.Margin` | `4,0,4,4` | NavigationViewIconPositionStates | **IconOnly** | themeresources.xaml:L10905 |
| `NavigationViewItemPresenter` | `ContentPresenter.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | themeresources.xaml:L10911 |
| `NavigationViewItemPresenter` | `SelectionIndicatorGrid.Margin` | `12,0,12,4` | NavigationViewIconPositionStates | **ContentOnly** | themeresources.xaml:L10912 |
| `NavigationViewItemPresenter` | `LayoutRoot.Width` | `48` | NavigationViewIconPositionStates | **IconOnly** | themeresources.xaml:L10967 |
| `NavigationViewItemPresenter` | `SelectionIndicatorGrid.Margin` | `4,0` | NavigationViewIconPositionStates | **IconOnly** | themeresources.xaml:L10969 |
| `NavigationViewItemPresenter` | `ContentPresenter.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | themeresources.xaml:L10975 |
| `NavigationViewItemPresenter` | `SelectionIndicatorGrid.Margin` | `12,0` | NavigationViewIconPositionStates | **ContentOnly** | themeresources.xaml:L10976 |
| `NavigationViewItemPresenter` | `ContentPresenter.Margin` | `16,0` | NavigationViewIconPositionStates | **ContentOnly** | themeresources.xaml:L11059 |

**C. 元素属性（写死数值）**

| 控件(TargetType) | 元素 | x:Name | 属性 | 值 | 视觉状态(组.状态) | 文件:行号 |
|---|---|---|---|---|---|---|
| `（无上下文）` | `StackPanel` | InkToolbarEraserButtonFlyoutContent | `Margin` | `0,8,0,8` | — | themeresources.xaml:L5930 |
| `（无上下文）` | `TextBlock` | StrokeEraserIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L5938 |
| `（无上下文）` | `TextBlock` | StrokeEraserName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L5939 |
| `（无上下文）` | `TextBlock` | SmallEraserIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L5950 |
| `（无上下文）` | `TextBlock` | SmallEraserName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L5951 |
| `（无上下文）` | `TextBlock` | LargeEraserIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L5962 |
| `（无上下文）` | `TextBlock` | LargeEraserName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L5963 |
| `（无上下文）` | `TextBlock` | ClearAllIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L5974 |
| `（无上下文）` | `TextBlock` | ClearAllName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L5975 |
| `（无上下文）` | `StackPanel` | InkToolbarStencilButtonFlyoutContent | `Margin` | `0,8,0,8` | — | themeresources.xaml:L5982 |
| `（无上下文）` | `TextBlock` | RulerIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L5990 |
| `（无上下文）` | `TextBlock` | RulerName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L5991 |
| `（无上下文）` | `TextBlock` | ProtractorIcon | `Margin` | `12,0,12,0` | — | themeresources.xaml:L6002 |
| `（无上下文）` | `TextBlock` | ProtractorName | `Margin` | `0,0,12,0` | — | themeresources.xaml:L6003 |
| `（无上下文）` | `Border` | — | `Width` | `1` | — | themeresources.xaml:L6012 |
| `（无上下文）` | `Grid` | — | `Height` | `44` | — | themeresources.xaml:L6022 |
| `（无上下文）` | `TextBlock` | — | `Margin` | `-8,-8,0,0` | — | themeresources.xaml:L6029 |
| `（无上下文）` | `TextBlock` | — | `Margin` | `-8,-8,0,0` | — | themeresources.xaml:L6033 |
| `（无上下文）` | `Image` | — | `Margin` | `-8,-8,0,0` | — | themeresources.xaml:L6036 |
| `（无上下文）` | `Image` | — | `Margin` | `-8,-8,0,0` | — | themeresources.xaml:L6039 |
| `AppBarButton` | `Grid` | Root | `Margin` | `1,0` | — | themeresources.xaml:L6112 |
| `AppBarButton` | `Grid` | ContentRoot | `Margin` | `-1,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6301 |
| `AppBarButton` | `TextBlock` | OverflowTextLabel | `Margin` | `12,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6314 |
| `AppBarButton` | `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6315 |
| `AppBarButton` | `Border` | Border | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6316 |
| `AppBarButton` | `Grid` | ContentRoot | `Margin` | `4,4,4,2` | .PointerOver | themeresources.xaml:L11795 |
| `AppBarButton` | `Viewbox` | — | `MinWidth` | `16` | .PointerOver | themeresources.xaml:L11800 |
| `AppBarButton` | `Viewbox` | — | `MaxHeight` | `16` | .PointerOver | themeresources.xaml:L11800 |
| `AppBarButton` | `ContentPresenter` | Content | `Margin` | `0,0,0,2` | .PointerOver | themeresources.xaml:L11801 |
| `AppBarButton` | `Viewbox` | ContentViewbox | `Height` | `16` | .NoFlyout | themeresources.xaml:L12071 |
| `AppBarButton` | `TextBlock` | OverflowTextLabel | `Padding` | `0,5,0,7` | .NoFlyout | themeresources.xaml:L12074 |
| `AppBarButton` | `TextBlock` | OverflowTextLabel | `Margin` | `12,0,12,0` | .NoFlyout | themeresources.xaml:L12074 |
| `AppBarButton` | `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .NoFlyout | themeresources.xaml:L12075 |
| `AppBarButton` | `FontIcon` | SubItemChevron | `Margin` | `12,0,12,0` | .NoFlyout | themeresources.xaml:L12076 |
| `AppBarToggleButton` | `Grid` | Root | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6337 |
| `AppBarToggleButton` | `Grid` | ContentRoot | `Margin` | `-1,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6619 |
| `AppBarToggleButton` | `TextBlock` | OverflowCheckGlyph | `Height` | `14` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6628 |
| `AppBarToggleButton` | `TextBlock` | OverflowCheckGlyph | `Width` | `14` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6628 |
| `AppBarToggleButton` | `TextBlock` | OverflowTextLabel | `Margin` | `38,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6633 |
| `AppBarToggleButton` | `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6634 |
| `AppBarToggleButton` | `Border` | Border | `Margin` | `1,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6635 |
| `AppBarToggleButton` | `TextBlock` | OverflowCheckGlyph | `Margin` | `12,4,12,4` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L12284 |
| `AppBarToggleButton` | `Viewbox` | ContentViewbox | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L12285 |
| `AppBarToggleButton` | `TextBlock` | OverflowTextLabel | `Margin` | `38,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L12288 |
| `AppBarToggleButton` | `TextBlock` | OverflowTextLabel | `Padding` | `0,5,0,7` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L12288 |
| `AppBarToggleButton` | `TextBlock` | KeyboardAcceleratorTextLabel | `Margin` | `24,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L12289 |
| `Button` | `Grid` | Root | `Margin` | `1,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6661 |
| `Button` | `ContentPresenter` | ContentPresenter | `Margin` | `-2,-1,-1,-1` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6718 |
| `Button` | `Viewbox` | IconHost | `Height` | `16` | .Normal | themeresources.xaml:L10494 |
| `Button` | `Viewbox` | IconHost | `Width` | `16` | .Normal | themeresources.xaml:L10494 |
| `Button` | `FontIcon` | Icon | `Margin` | `8,0,0,-4` | .Normal | themeresources.xaml:L10555 |
| `CalendarView` | `RowDefinition` | — | `Height` | `40` | .Month | themeresources.xaml:L7972 |
| `CalendarView` | `ColumnDefinition` | — | `Width` | `5*` | .Month | themeresources.xaml:L7977 |
| `CalendarView` | `Button` | HeaderButton | `Padding` | `12,0,0,0` | .Month | themeresources.xaml:L7981 |
| `CalendarView` | `Button` | PreviousButton | `Padding` | `1` | .Month | themeresources.xaml:L7982 |
| `CalendarView` | `Button` | NextButton | `Padding` | `1` | .Month | themeresources.xaml:L7983 |
| `CalendarView` | `RowDefinition` | — | `Height` | `38` | .Month | themeresources.xaml:L7996 |
| `CalendarViewDayItem` | `Grid` | Root | `Width` | `0` | .Normal | themeresources.xaml:L7627 |
| `CheckBox` | `Grid` | RootGrid | `Width` | `32` | .NotDragging | themeresources.xaml:L8310 |
| `CheckBox` | `Grid` | — | `Height` | `32` | .NotDragging | themeresources.xaml:L8468 |
| `CheckBox` | `Rectangle` | NormalRectangle | `Height` | `20` | .NotDragging | themeresources.xaml:L8469 |
| `CheckBox` | `Rectangle` | NormalRectangle | `Width` | `20` | .NotDragging | themeresources.xaml:L8469 |
| `CommandBarOverflowPresenter` | `ItemsPresenter` | ItemsPresenter | `Margin` | `0,4,0,4` | .FullWidthOpenUp | themeresources.xaml:L12325 |
| `GridViewItem` | `Border` | MultiSelectSquare | `Margin` | `0,2,2,0` | .DraggedPlaceholder | themeresources.xaml:L7373 |
| `GridViewItem` | `Border` | MultiSelectSquare | `Width` | `20` | .DraggedPlaceholder | themeresources.xaml:L7373 |
| `GridViewItem` | `Border` | MultiSelectSquare | `Height` | `20` | .DraggedPlaceholder | themeresources.xaml:L7373 |
| `GridViewItem` | `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | .DraggedPlaceholder | themeresources.xaml:L7376 |
| `GridViewItem` | `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | .DraggedPlaceholder | themeresources.xaml:L7376 |
| `GridViewItem` | `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | .DraggedPlaceholder | themeresources.xaml:L7376 |
| `InkToolbarFlyoutItem` | `Border` | Border | `BorderThickness` | `0` | .PointerFocused | themeresources.xaml:L8185 |
| `ListViewItem` | `Grid` | ContentPresenterGrid | `Margin` | `0,0,0,0` | .DraggedPlaceholder | themeresources.xaml:L11380 |
| `ListViewItem` | `Border` | MultiSelectSquare | `Height` | `20` | .DraggedPlaceholder | themeresources.xaml:L11391 |
| `ListViewItem` | `Border` | MultiSelectSquare | `Margin` | `12,0,0,0` | .DraggedPlaceholder | themeresources.xaml:L11391 |
| `ListViewItem` | `Border` | MultiSelectSquare | `Width` | `20` | .DraggedPlaceholder | themeresources.xaml:L11391 |
| `ListViewItem` | `Border` | MultiSelectSquare | `BorderThickness` | `2` | .DraggedPlaceholder | themeresources.xaml:L11391 |
| `ListViewItem` | `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | .DraggedPlaceholder | themeresources.xaml:L11404 |
| `ListViewItem` | `Border` | MultiArrangeOverlayTextBorder | `Margin` | `12,0,0,0` | .DraggedPlaceholder | themeresources.xaml:L11404 |
| `ListViewItem` | `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | .DraggedPlaceholder | themeresources.xaml:L11404 |
| `ListViewItem` | `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | .DraggedPlaceholder | themeresources.xaml:L11404 |
| `ListViewItem` | `Rectangle` | NormalRectangle | `Width` | `25.5` | .NoHighlight | themeresources.xaml:L11523 |
| `ListViewItem` | `Rectangle` | NormalRectangle | `Height` | `25.5` | .NoHighlight | themeresources.xaml:L11523 |
| `ListViewItem` | `Path` | CheckGlyph | `Height` | `17` | .NoHighlight | themeresources.xaml:L11524 |
| `ListViewItem` | `Path` | CheckGlyph | `Width` | `18.5` | .NoHighlight | themeresources.xaml:L11524 |
| `ListViewItem` | `Grid` | SelectedCheckMark | `Height` | `34` | .NoHighlight | themeresources.xaml:L11546 |
| `ListViewItem` | `Grid` | SelectedCheckMark | `Width` | `34` | .NoHighlight | themeresources.xaml:L11546 |
| `ListViewItem` | `Path` | SelectedGlyph | `Margin` | `0,1,1,0` | .NoHighlight | themeresources.xaml:L11548 |
| `ListViewItem` | `Path` | SelectedGlyph | `Width` | `17` | .NoHighlight | themeresources.xaml:L11548 |
| `ListViewItem` | `Path` | SelectedGlyph | `Height` | `14.5` | .NoHighlight | themeresources.xaml:L11548 |
| `ListViewItem` | `Grid` | RootGrid | `Margin` | `0,0,11,0` | .PointerOver | themeresources.xaml:L11831 |
| `ListViewItem` | `Grid` | — | `Height` | `32` | .Enabled | themeresources.xaml:L11902 |
| `ListViewItem` | `Ellipse` | OuterEllipse | `Height` | `20` | .Enabled | themeresources.xaml:L11903 |
| `ListViewItem` | `Ellipse` | OuterEllipse | `Width` | `20` | .Enabled | themeresources.xaml:L11903 |
| `ListViewItem` | `Ellipse` | CheckOuterEllipse | `Width` | `20` | .Enabled | themeresources.xaml:L11904 |
| `ListViewItem` | `Ellipse` | CheckOuterEllipse | `Height` | `20` | .Enabled | themeresources.xaml:L11904 |
| `ListViewItem` | `Ellipse` | CheckGlyph | `Height` | `10` | .Enabled | themeresources.xaml:L11905 |
| `ListViewItem` | `Ellipse` | CheckGlyph | `Width` | `10` | .Enabled | themeresources.xaml:L11905 |
| `MenuFlyoutItem` | `Polyline` | CurrentSelectionIcon | `Margin` | `0,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6959 |
| `MenuFlyoutItem` | `Viewbox` | IconRoot | `Width` | `24` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6960 |
| `MenuFlyoutItem` | `Viewbox` | IconRoot | `Height` | `24` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6960 |
| `MenuFlyoutItem` | `Viewbox` | IconRoot | `Margin` | `12,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6960 |
| `MenuFlyoutItem` | `TextBlock` | TextBlock | `Margin` | `12,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6963 |
| `MenuFlyoutItem` | `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `12,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L6964 |
| `MenuFlyoutItem` | `Viewbox` | IconRoot | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10191 |
| `MenuFlyoutItem` | `Viewbox` | IconRoot | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10191 |
| `MenuFlyoutItem` | `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `24,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10195 |
| `MenuFlyoutSubItem` | `Viewbox` | IconRoot | `Height` | `16` | .DefaultPadding | themeresources.xaml:L10412 |
| `MenuFlyoutSubItem` | `Viewbox` | IconRoot | `Width` | `16` | .DefaultPadding | themeresources.xaml:L10412 |
| `NavigationViewItem` | `Viewbox` | IconBox | `Height` | `16` | .Enabled | themeresources.xaml:L10831 |
| `NavigationViewItem` | `Viewbox` | IconBox | `Width` | `48` | .Enabled | themeresources.xaml:L10831 |
| `NavigationViewItem` | `Viewbox` | IconBox | `Margin` | `0,0,0,0` | .Enabled | themeresources.xaml:L10831 |
| `NavigationViewItemPresenter` | `Grid` | LayoutRoot | `Height` | `40` | .Normal | themeresources.xaml:L10616 |
| `NavigationViewItemPresenter` | `Grid` | — | `Margin` | `4,0,0,0` | .IconVisible | themeresources.xaml:L10680 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Height` | `24` | .IconVisible | themeresources.xaml:L10681 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Width` | `2` | .IconVisible | themeresources.xaml:L10681 |
| `NavigationViewItemPresenter` | `Grid` | ContentGrid | `Height` | `40` | .IconVisible | themeresources.xaml:L10684 |
| `NavigationViewItemPresenter` | `ColumnDefinition` | IconColumn | `Width` | `48` | .IconVisible | themeresources.xaml:L10686 |
| `NavigationViewItemPresenter` | `Grid` | LayoutRoot | `Height` | `40` | .IconVisible | themeresources.xaml:L10709 |
| `NavigationViewItemPresenter` | `Grid` | — | `Margin` | `4,0,0,0` | .IconVisible | themeresources.xaml:L10741 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Width` | `2` | .IconVisible | themeresources.xaml:L10742 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Height` | `24` | .IconVisible | themeresources.xaml:L10742 |
| `NavigationViewItemPresenter` | `Grid` | ContentGrid | `Height` | `40` | .IconVisible | themeresources.xaml:L10744 |
| `NavigationViewItemPresenter` | `ColumnDefinition` | IconColumn | `Width` | `48` | .IconVisible | themeresources.xaml:L10746 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Height` | `16` | .IconOnLeft | themeresources.xaml:L10923 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnLeft | themeresources.xaml:L10923 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Width` | `16` | .IconOnLeft | themeresources.xaml:L10923 |
| `NavigationViewItemPresenter` | `ContentPresenter` | ContentPresenter | `Margin` | `8,0,16,0` | .IconOnLeft | themeresources.xaml:L10926 |
| `NavigationViewItemPresenter` | `Grid` | SelectionIndicatorGrid | `Margin` | `16,0,16,4` | .IconOnLeft | themeresources.xaml:L10928 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Height` | `2` | .IconOnLeft | themeresources.xaml:L10929 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Width` | `16` | .IconOnLeft | themeresources.xaml:L10987 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnLeft | themeresources.xaml:L10987 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Height` | `16` | .IconOnLeft | themeresources.xaml:L10987 |
| `NavigationViewItemPresenter` | `ContentPresenter` | ContentPresenter | `Margin` | `8,0,16,0` | .IconOnLeft | themeresources.xaml:L10990 |
| `NavigationViewItemPresenter` | `Grid` | SelectionIndicatorGrid | `Margin` | `16,0,16,4` | .IconOnLeft | themeresources.xaml:L10992 |
| `NavigationViewItemPresenter` | `Rectangle` | SelectionIndicator | `Height` | `2` | .IconOnLeft | themeresources.xaml:L10993 |
| `NavigationViewItemPresenter` | `Grid` | LayoutRoot | `Height` | `40` | .IconOnLeft | themeresources.xaml:L11004 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Margin` | `16,0,0,0` | .IconOnly | themeresources.xaml:L11069 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Width` | `16` | .IconOnly | themeresources.xaml:L11069 |
| `NavigationViewItemPresenter` | `Viewbox` | IconBox | `Height` | `16` | .IconOnly | themeresources.xaml:L11069 |
| `NavigationViewItemPresenter` | `ContentPresenter` | ContentPresenter | `Margin` | `12,0,16,0` | .IconOnly | themeresources.xaml:L11072 |
| `SemanticZoom` | `Button` | ZoomOutButton | `Margin` | `0,0,19,19` | .InputModeDefault | themeresources.xaml:L9985 |
| `SemanticZoom` | `Button` | ZoomOutButton | `Width` | `12` | .InputModeDefault | themeresources.xaml:L9985 |
| `SemanticZoom` | `Button` | ZoomOutButton | `Height` | `12` | .InputModeDefault | themeresources.xaml:L9985 |
| `SemanticZoom` | `Button` | ZoomOutButton | `Padding` | `0` | .InputModeDefault | themeresources.xaml:L9985 |
| `SplitButton` | `Button` | SecondaryButton | `Padding` | `0,0,8,0` | .SecondaryButtonRight | themeresources.xaml:L8690 |
| `TextBox` | `Button` | DeleteButton | `MinWidth` | `34` | .ButtonCollapsed | themeresources.xaml:L11762 |
| `TextBox` | `Button` | QueryButton | `MinWidth` | `34` | .ButtonCollapsed | themeresources.xaml:L11763 |
| `ToggleMenuFlyoutItem` | `FontIcon` | CheckGlyph | `Margin` | `0,0,12,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10307 |
| `ToggleMenuFlyoutItem` | `FontIcon` | CheckGlyph | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10307 |
| `ToggleMenuFlyoutItem` | `Viewbox` | IconRoot | `Width` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10308 |
| `ToggleMenuFlyoutItem` | `Viewbox` | IconRoot | `Height` | `16` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10308 |
| `ToggleMenuFlyoutItem` | `TextBlock` | KeyboardAcceleratorTextBlock | `Margin` | `24,0,0,0` | .KeyboardAcceleratorTextCollapsed | themeresources.xaml:L10312 |
| `TreeViewItem` | `Grid` | ContentPresenterGrid | `Margin` | `0,0,0,0` | .PointerFocused | themeresources.xaml:L8203 |
| `TreeViewItem` | `CheckBox` | MultiSelectCheckBox | `Width` | `32` | .NotDragging | themeresources.xaml:L8478 |
| `TreeViewItem` | `Border` | MultiArrangeOverlayTextBorder | `BorderThickness` | `2` | .NotDragging | themeresources.xaml:L8479 |
| `TreeViewItem` | `Border` | MultiArrangeOverlayTextBorder | `Height` | `20` | .NotDragging | themeresources.xaml:L8479 |
| `TreeViewItem` | `Border` | MultiArrangeOverlayTextBorder | `MinWidth` | `20` | .NotDragging | themeresources.xaml:L8479 |
| `TreeViewItem` | `Grid` | ExpandCollapseChevron | `Padding` | `12,0,12,0` | .NotDragging | themeresources.xaml:L8483 |
| `TreeViewItem` | `TextBlock` | — | `Width` | `12` | .NotDragging | themeresources.xaml:L8484 |
| `TreeViewItem` | `TextBlock` | — | `Height` | `12` | .NotDragging | themeresources.xaml:L8484 |
| `TreeViewItem` | `TextBlock` | — | `Width` | `12` | .NotDragging | themeresources.xaml:L8485 |
| `TreeViewItem` | `TextBlock` | — | `Height` | `12` | .NotDragging | themeresources.xaml:L8485 |

### 5.98 索引：`generic.xaml` 中引用资源的几何 `Setter`（167 条）

> 这些 Setter 的值是 `{ThemeResource X}` / `{StaticResource X}`，**实际值在 §2 / §3 里**。
> 本表的作用是**接线图**：告诉 Rust 侧「这个控件的这个属性该去读哪个资源键」。
> 全部去重后只剩下面这些组合（`{ThemeResource …}` 与 `{StaticResource …}` 分列）。

| 控件(TargetType) | 属性 | 资源引用 | 文件:行号 |
|---|---|---|---|
| `AppBarButton` | `BorderThickness` | `{ThemeResource AppBarButtonRevealBorderThemeThickness}` | generic.xaml:L23680 |
| `AppBarButton` | `Padding` | `{ThemeResource AppBarButtonOverflowTextTouchMargin}` | generic.xaml:L23877, L23882, L26854, L26859 |
| `AppBarToggleButton` | `BorderThickness` | `{ThemeResource AppBarToggleButtonRevealBorderThemeThickness}` | generic.xaml:L23976 |
| `AppBarToggleButton` | `Margin` | `{ThemeResource AppBarToggleButtonOverflowCheckTouchMargin}` | generic.xaml:L24265, L24271, L27264, L27270 |
| `AppBarToggleButton` | `Padding` | `{ThemeResource AppBarToggleButtonOverflowTextTouchMargin}` | generic.xaml:L24264, L24270, L27263, L27269 |
| `Button` | `BorderThickness` | `{ThemeResource AppBarEllipsisButtonRevealBorderThemeThickness}` | generic.xaml:L22743 |
| `Button` | `BorderThickness` | `{ThemeResource ButtonBorderThemeThickness}` | generic.xaml:L6243, L8182 |
| `Button` | `BorderThickness` | `{ThemeResource ButtonRevealBorderThemeThickness}` | generic.xaml:L22452 |
| `Button` | `BorderThickness` | `{ThemeResource NavigationViewToggleBorderThickness}` | generic.xaml:L25588, L25706 |
| `Button` | `Height` | `{StaticResource PaneToggleButtonHeight}` | generic.xaml:L25715, L25794 |
| `Button` | `Height` | `{ThemeResource NavigationBackButtonHeight}` | generic.xaml:L15253 |
| `Button` | `MinHeight` | `{StaticResource PaneToggleButtonHeight}` | generic.xaml:L25579 |
| `Button` | `MinWidth` | `{StaticResource PaneToggleButtonWidth}` | generic.xaml:L25580 |
| `Button` | `Padding` | `{StaticResource ButtonPadding}` | generic.xaml:L6244 |
| `Button` | `Padding` | `{ThemeResource ButtonPadding}` | generic.xaml:L22453 |
| `Button` | `Width` | `{StaticResource PaneToggleButtonWidth}` | generic.xaml:L25795 |
| `Button` | `Width` | `{ThemeResource AppBarExpandButtonThemeWidth}` | generic.xaml:L11315, L22752 |
| `Button` | `Width` | `{ThemeResource NavigationBackButtonWidth}` | generic.xaml:L15254 |
| `CalendarDatePicker` | `BorderThickness` | `{ThemeResource CalendarDatePickerBorderThemeThickness}` | generic.xaml:L16675 |
| `CalendarDatePicker` | `Margin` | `{StaticResource CalendarDatePickerLeftHeaderMargin}` | generic.xaml:L16767 |
| `CalendarDatePicker` | `MaxWidth` | `{StaticResource CalendarDatePickerLeftHeaderMaxWidth}` | generic.xaml:L16768 |
| `ComboBox` | `BorderThickness` | `{ThemeResource ComboBoxBorderThemeThickness}` | generic.xaml:L10137 |
| `ComboBoxItem` | `BorderThickness` | `{ThemeResource ComboBoxItemRevealBorderThemeThickness}` | generic.xaml:L24592 |
| `ComboBoxItem` | `Margin` | `{ThemeResource ComboBoxItemRevealThemeGameControllerPadding}` | generic.xaml:L24701 |
| `ComboBoxItem` | `Margin` | `{ThemeResource ComboBoxItemRevealThemeTouchPadding}` | generic.xaml:L24696 |
| `ComboBoxItem` | `Padding` | `{ThemeResource ComboBoxItemRevealThemePadding}` | generic.xaml:L24594 |
| `CommandBarFlyoutCommandBar` | `BorderThickness` | `{ThemeResource CommandBarFlyoutBorderDownThemeThickness}` | generic.xaml:L22313 |
| `CommandBarFlyoutCommandBar` | `BorderThickness` | `{ThemeResource CommandBarFlyoutBorderThemeThickness}` | generic.xaml:L22069, L22318, L22323 |
| `CommandBarFlyoutCommandBar` | `BorderThickness` | `{ThemeResource CommandBarFlyoutBorderUpThemeThickness}` | generic.xaml:L22308 |
| `CommandBarOverflowPresenter` | `BorderThickness` | `{ThemeResource CommandBarOverflowPresenterBorderDownThickness}` | generic.xaml:L27742 |
| `CommandBarOverflowPresenter` | `BorderThickness` | `{ThemeResource CommandBarOverflowPresenterBorderUpThickness}` | generic.xaml:L27748 |
| `CommandBarOverflowPresenter` | `MaxWidth` | `{ThemeResource CommandBarOverflowMaxWidth}` | generic.xaml:L27718 |
| `CommandBarOverflowPresenter` | `Padding` | `{ThemeResource CommandBarOverflowPresenterBorderDownPadding}` | generic.xaml:L27741 |
| `CommandBarOverflowPresenter` | `Padding` | `{ThemeResource CommandBarOverflowPresenterBorderPadding}` | generic.xaml:L27717 |
| `CommandBarOverflowPresenter` | `Padding` | `{ThemeResource CommandBarOverflowPresenterBorderUpPadding}` | generic.xaml:L27747 |
| `ContentControl` | `Margin` | `{StaticResource PivotPortraitThemePadding}` | generic.xaml:L16133 |
| `DatePickerFlyoutPresenter` | `BorderThickness` | `{ThemeResource DateTimeFlyoutBorderThickness}` | generic.xaml:L14683 |
| `DropDownButton` | `BorderThickness` | `{ThemeResource ButtonBorderThemeThickness}` | generic.xaml:L21938 |
| `DropDownButton` | `Padding` | `{StaticResource ButtonPadding}` | generic.xaml:L21939 |
| `FlyoutPresenter` | `BorderThickness` | `{ThemeResource FlyoutBorderThemeThickness}` | generic.xaml:L13665 |
| `FlyoutPresenter` | `MaxHeight` | `{ThemeResource FlyoutThemeMaxHeight}` | generic.xaml:L13670 |
| `FlyoutPresenter` | `MaxWidth` | `{ThemeResource FlyoutThemeMaxWidth}` | generic.xaml:L13668 |
| `FlyoutPresenter` | `MinHeight` | `{ThemeResource FlyoutThemeMinHeight}` | generic.xaml:L13669 |
| `FlyoutPresenter` | `MinWidth` | `{ThemeResource FlyoutThemeMinWidth}` | generic.xaml:L13667 |
| `FlyoutPresenter` | `Padding` | `{ThemeResource FlyoutContentThemePadding}` | generic.xaml:L13666 |
| `GridViewHeaderItem` | `MinHeight` | `{ThemeResource GridViewHeaderItemMinHeight}` | generic.xaml:L10619 |
| `GridViewItem` | `MinHeight` | `{ThemeResource GridViewItemMinHeight}` | generic.xaml:L10690, L24496 |
| `GridViewItem` | `MinWidth` | `{ThemeResource GridViewItemMinWidth}` | generic.xaml:L10689, L24495 |
| `HandwritingView` | `BorderThickness` | `{ThemeResource TextControlBorderThemeThickness}` | generic.xaml:L7969 |
| `HandwritingView` | `MinHeight` | `{ThemeResource TextControlThemeMinHeight}` | generic.xaml:L7965 |
| `HandwritingView` | `MinWidth` | `{ThemeResource TextControlThemeMinWidth}` | generic.xaml:L7964 |
| `HandwritingView` | `Padding` | `{ThemeResource TextControlThemePadding}` | generic.xaml:L7977 |
| `HyperlinkButton` | `BorderThickness` | `{ThemeResource HyperlinkButtonBorderThemeThickness}` | generic.xaml:L6646 |
| `HyperlinkButton` | `Padding` | `{StaticResource HyperlinkButtonPadding}` | generic.xaml:L6647 |
| `ListBox` | `BorderThickness` | `{ThemeResource ListBoxBorderThemeThickness}` | generic.xaml:L27498 |
| `ListBoxItem` | `Padding` | `{StaticResource ListBoxItemPadding}` | generic.xaml:L27369 |
| `ListViewHeaderItem` | `MinHeight` | `{ThemeResource ListViewHeaderItemMinHeight}` | generic.xaml:L10653 |
| `ListViewItem` | `Margin` | `{ThemeResource ListPickerFlyoutPresenterItemMargin}` | generic.xaml:L28568 |
| `ListViewItem` | `MinHeight` | `{ThemeResource ListViewItemMinHeight}` | generic.xaml:L24388, L28069 |
| `ListViewItem` | `MinWidth` | `{ThemeResource ListViewItemMinWidth}` | generic.xaml:L24387, L28068 |
| `MediaTransportControls` | `BorderThickness` | `{ThemeResource SliderBorderThemeThickness}` | generic.xaml:L15369 |
| `MediaTransportControls` | `Height` | `{ThemeResource MTCMediaButtonHeight}` | generic.xaml:L15343, L15349, L15354 |
| `MediaTransportControls` | `Width` | `{ThemeResource MTCMediaButtonWidth}` | generic.xaml:L15342, L15348 |
| `MenuBar` | `Height` | `{StaticResource MenuBarHeight}` | generic.xaml:L21006 |
| `MenuBarItem` | `BorderThickness` | `{ThemeResource MenuBarItemBorderThickness}` | generic.xaml:L21027 |
| `MenuFlyoutItem` | `BorderThickness` | `{ThemeResource MenuFlyoutItemRevealBorderThickness}` | generic.xaml:L8405, L25073 |
| `MenuFlyoutItem` | `Margin` | `{ThemeResource MenuFlyoutItemDoublePlaceholderThemeThickness}` | generic.xaml:L25156 |
| `MenuFlyoutItem` | `Margin` | `{ThemeResource MenuFlyoutItemPlaceholderThemeThickness}` | generic.xaml:L8487, L25145, L25150, L25157 |
| `MenuFlyoutItem` | `Padding` | `{ThemeResource LanguageSwitcherMenuFlyoutItemThemePadding}` | generic.xaml:L8407 |
| `MenuFlyoutItem` | `Padding` | `{ThemeResource MenuFlyoutItemThemePadding}` | generic.xaml:L25075 |
| `MenuFlyoutPresenter` | `BorderThickness` | `{ThemeResource MenuFlyoutPresenterBorderThemeThickness}` | generic.xaml:L31151 |
| `MenuFlyoutPresenter` | `Margin` | `{ThemeResource MenuFlyoutScrollerMargin}` | generic.xaml:L8579 |
| `MenuFlyoutPresenter` | `MaxWidth` | `{ThemeResource FlyoutThemeMaxWidth}` | generic.xaml:L31160 |
| `MenuFlyoutPresenter` | `MinHeight` | `{ThemeResource MenuFlyoutThemeMinHeight}` | generic.xaml:L31161 |
| `MenuFlyoutPresenter` | `Padding` | `{ThemeResource MenuFlyoutPresenterThemePadding}` | generic.xaml:L31152 |
| `MenuFlyoutSeparator` | `Padding` | `{ThemeResource MenuFlyoutSeparatorThemePadding}` | generic.xaml:L25055 |
| `MenuFlyoutSubItem` | `BorderThickness` | `{ThemeResource MenuFlyoutSubItemRevealBorderThickness}` | generic.xaml:L25401 |
| `MenuFlyoutSubItem` | `Margin` | `{ThemeResource MenuFlyoutItemDoublePlaceholderThemeThickness}` | generic.xaml:L25479 |
| `MenuFlyoutSubItem` | `Margin` | `{ThemeResource MenuFlyoutItemPlaceholderThemeThickness}` | generic.xaml:L25468, L25473, L25480 |
| `MenuFlyoutSubItem` | `Padding` | `{ThemeResource MenuFlyoutItemThemePadding}` | generic.xaml:L25403 |
| `NavigationView` | `Width` | `{Binding RelativeSource={RelativeSource TemplatedParent}, Path=CompactPaneLength}` | generic.xaml:L19803, L19805, L19809, L19811 |
| `NavigationViewItem` | `BorderThickness` | `{StaticResource NavigationViewItemBorderThickness}` | generic.xaml:L20136, L26118 |
| `NavigationViewItemPresenter` | `BorderThickness` | `{StaticResource NavigationViewItemBorderThickness}` | generic.xaml:L25862, L26008 |
| `PasswordBox` | `BorderThickness` | `{ThemeResource TextControlBorderThemeThickness}` | generic.xaml:L27778 |
| `PasswordBox` | `Padding` | `{ThemeResource TextControlThemePadding}` | generic.xaml:L27785 |
| `PivotHeaderItem` | `Padding` | `{ThemeResource PivotHeaderItemMargin}` | generic.xaml:L14458 |
| `PivotItem` | `Margin` | `{ThemeResource PivotItemMargin}` | generic.xaml:L14419 |
| `ProgressBar` | `BorderThickness` | `{ThemeResource ProgressBarBorderThemeThickness}` | generic.xaml:L12077 |
| `ProgressBar` | `MinHeight` | `{ThemeResource ProgressBarThemeMinHeight}` | generic.xaml:L12080 |
| `RepeatButton` | `BorderThickness` | `{ThemeResource ButtonBorderThemeThickness}` | generic.xaml:L6340 |
| `RepeatButton` | `BorderThickness` | `{ThemeResource RepeatButtonRevealBorderThemeThickness}` | generic.xaml:L22525 |
| `RepeatButton` | `Padding` | `{StaticResource ButtonPadding}` | generic.xaml:L6341 |
| `RepeatButton` | `Padding` | `{ThemeResource ButtonPadding}` | generic.xaml:L22526 |
| `RichEditBox` | `BorderThickness` | `{ThemeResource TextControlBorderThemeThickness}` | generic.xaml:L27558 |
| `RichEditBox` | `Padding` | `{ThemeResource TextControlThemePadding}` | generic.xaml:L27567 |
| `ScrollBar` | `MinHeight` | `{ThemeResource ScrollBarSize}` | generic.xaml:L8720 |
| `ScrollBar` | `MinWidth` | `{ThemeResource ScrollBarSize}` | generic.xaml:L8719 |
| `SearchBox` | `BorderThickness` | `{ThemeResource SearchBoxBorderThemeThickness}` | generic.xaml:L7427 |
| `SearchBox` | `BorderThickness` | `{ThemeResource TextControlBorderThemeThickness}` | generic.xaml:L7592 |
| `SearchBox` | `MinHeight` | `{ThemeResource TextControlThemeMinHeight}` | generic.xaml:L7587 |
| `SearchBox` | `MinWidth` | `{ThemeResource TextControlThemeMinWidth}` | generic.xaml:L7586 |
| `SearchBox` | `Padding` | `{ThemeResource SearchBoxThemePadding}` | generic.xaml:L7432 |
| `SearchBox` | `Padding` | `{ThemeResource TextControlThemePadding}` | generic.xaml:L7598 |
| `Slider` | `BorderThickness` | `{ThemeResource SliderBorderThemeThickness}` | generic.xaml:L12588 |
| `Slider` | `Height` | `{ThemeResource SliderTrackThemeHeight}` | generic.xaml:L15398 |
| `SplitButton` | `BorderThickness` | `{ThemeResource SplitButtonBorderThemeThickness}` | generic.xaml:L21660, L21681 |
| `SplitButton` | `Padding` | `{ThemeResource ButtonPadding}` | generic.xaml:L21669 |
| `SwipeControl` | `MinHeight` | `{ThemeResource ListViewItemMinHeight}` | generic.xaml:L21553 |
| `SwipeControl` | `MinWidth` | `{ThemeResource ListViewItemMinWidth}` | generic.xaml:L21554 |
| `TextBox` | `BorderThickness` | `{ThemeResource TextControlBorderThemeThickness}` | generic.xaml:L28822, L29089 |
| `TextBox` | `MinHeight` | `{ThemeResource TextControlThemeMinHeight}` | generic.xaml:L29084 |
| `TextBox` | `MinWidth` | `{ThemeResource TextControlThemeMinWidth}` | generic.xaml:L29083 |
| `TextBox` | `Padding` | `{ThemeResource TextControlThemePadding}` | generic.xaml:L28830, L29097 |
| `TimePickerFlyoutPresenter` | `BorderThickness` | `{ThemeResource DateTimeFlyoutBorderThickness}` | generic.xaml:L14891 |
| `ToggleButton` | `BorderThickness` | `{ThemeResource InkToolbarButtonBorderThemeThickness}` | generic.xaml:L17628 |
| `ToggleButton` | `BorderThickness` | `{ThemeResource ToggleButtonBorderThemeThickness}` | generic.xaml:L6430 |
| `ToggleButton` | `BorderThickness` | `{ThemeResource ToggleButtonRevealBorderThemeThickness}` | generic.xaml:L22585 |
| `ToggleButton` | `MaxHeight` | `{ThemeResource InkToolbarButtonHeight}` | generic.xaml:L17627 |
| `ToggleButton` | `MaxWidth` | `{ThemeResource InkToolbarButtonWidth}` | generic.xaml:L17626 |
| `ToggleButton` | `MinHeight` | `{ThemeResource InkToolbarButtonHeight}` | generic.xaml:L17625 |
| `ToggleButton` | `MinWidth` | `{ThemeResource InkToolbarButtonWidth}` | generic.xaml:L17624 |
| `ToggleButton` | `Padding` | `{StaticResource ButtonPadding}` | generic.xaml:L6431 |
| `ToggleButton` | `Padding` | `{ThemeResource ButtonPadding}` | generic.xaml:L22586 |
| `ToggleMenuFlyoutItem` | `BorderThickness` | `{ThemeResource ToggleMenuFlyoutItemRevealBorderThickness}` | generic.xaml:L25229 |
| `ToggleMenuFlyoutItem` | `Margin` | `{ThemeResource MenuFlyoutItemPlaceholderThemeThickness}` | generic.xaml:L25309, L25315 |
| `ToggleMenuFlyoutItem` | `Padding` | `{ThemeResource MenuFlyoutItemThemePadding}` | generic.xaml:L25231 |
| `ToolTip` | `BorderThickness` | `{ThemeResource ToolTipBorderThemeThickness}` | generic.xaml:L13249 |
| `ToolTip` | `Padding` | `{ThemeResource ToolTipBorderThemePadding}` | generic.xaml:L13252 |
| `TreeViewItem` | `BorderThickness` | `{ThemeResource TreeViewItemBorderThemeThickness}` | generic.xaml:L21138 |
| `TreeViewItem` | `MinHeight` | `{ThemeResource TreeViewItemMinHeight}` | generic.xaml:L21140 |

> 去重组合数：**130**（原始 167 条）。

---

## §6 取数方法与可复现命令

### 6.1 环境与依赖

- Windows PowerShell 5.1（`$PSVersionTable.PSVersion` = 5.1.26100.9444，`PSEdition` = Desktop，默认编码 **gb2312**）。
- **脚本必须存为带 BOM 的 UTF-8**，否则脚本里的中文字面量会被按 gb2312 解码而解析失败。
  本仓库的 `write` 工具产出无 BOM → 必须先做一次转码（见 6.2 第 0 步）。
- 无第三方模块、无网络。

### 6.2 复现步骤（把 `$dir` 指向本机 SDK）

```powershell
$dir = "C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic"

# --- 0. 若脚本含中文，先转成带 BOM 的 UTF-8 再执行 ---
$c = Get-Content -Raw -Encoding UTF8 $src
[System.IO.File]::WriteAllText($dst, $c, (New-Object System.Text.UTF8Encoding($true)))

# --- 1. 键值型资源：x:Double / Thickness / CornerRadius / GridLength / x:Int32 / x:Boolean ---
$pats = @{
  "x:Double"     = '<x:Double\s+x:Key="([^"]+)"\s*>([^<]*)</x:Double>'
  "Thickness"    = '<Thickness\s+x:Key="([^"]+)"\s*>([^<]*)</Thickness>'
  "CornerRadius" = '<CornerRadius\s+x:Key="([^"]+)"\s*>([^<]*)</CornerRadius>'
  "GridLength"   = '<GridLength\s+x:Key="([^"]+)"\s*>([^<]*)</GridLength>'
  "x:Int32"      = '<x:Int32\s+x:Key="([^"]+)"\s*>([^<]*)</x:Int32>'
  "x:Boolean"    = '<x:Boolean\s+x:Key="([^"]+)"\s*>([^<]*)</x:Boolean>'
}
$lines = Get-Content "$dir\themeresources.xaml"
$res = for ($i=0; $i -lt $lines.Count; $i++) {
  foreach ($k in $pats.Keys) {
    $m = [regex]::Match($lines[$i], $pats[$k])
    if ($m.Success) { [pscustomobject]@{ Type=$k; Key=$m.Groups[1].Value; Value=$m.Groups[2].Value.Trim(); Line=$i+1 } }
  }
}
$res | Group-Object Type | Select-Object Name, Count   # 期望 408 / 421 / 3 / 3 / 5 / 17

# --- 2. 几何 Setter（三种形态，必须整文匹配，不能逐行） ---
#     关键：UWP 的 VisualState.Setters 写成 <Setter Target="元素.属性" Value="…"/>，没有 Property= 属性。
function Get-Setters($file) {
  $txt   = (Get-Content $file) -join "`n"
  $idx   = { param($pos) ($txt.Substring(0,$pos) -split "`n").Count }
  foreach ($m in [regex]::Matches($txt, '<Setter\s+([^<>]*?)>')) {
    $blob = $m.Groups[1].Value
    $v = [regex]::Match($blob, '\bValue="([^"]*)"');    if (-not $v.Success) { continue }
    $p = [regex]::Match($blob, '\bProperty="([^"]*)"')
    $t = [regex]::Match($blob, '\bTarget="([^"]*)"')
    $prop = if ($p.Success) { $p.Groups[1].Value } elseif ($t.Success) { $t.Groups[1].Value } else { continue }
    if (($prop -split '\.')[-1] -notin @('Width','Height','MinWidth','MinHeight','MaxWidth','MaxHeight','BorderThickness','Padding','Margin','CornerRadius')) { continue }
    [pscustomobject]@{ Property=$prop; Value=$v.Groups[1].Value; Line=(& $idx $m.Index); Hardcoded=(-not $v.Groups[1].Value.Contains('{')) }
  }
}
(Get-Setters "$dir\generic.xaml").Count          # 期望 513，其中 Hardcoded 346
(Get-Setters "$dir\themeresources.xaml").Count  # 期望 204，其中 Hardcoded 123

# --- 3. 元素属性写死几何（整文匹配！多行属性必须跨行取） ---
$txt = (Get-Content "$dir\generic.xaml") -join "`n"
$n = 0
foreach ($m in [regex]::Matches($txt, '<([A-Za-z_][A-Za-z0-9_\.:]*)((?:[^<>])*)>')) {
  foreach ($a in [regex]::Matches($m.Groups[2].Value, '(Width|Height|MinWidth|MinHeight|MaxWidth|MaxHeight|BorderThickness|Padding|Margin|CornerRadius)="([^"]*)"')) {
    $v = $a.Groups[2].Value
    if (-not $v.Contains('{') -and $v -match '^[-0-9]') { $n++ }
  }
}
$n    # 期望 458（generic.xaml）；themeresources.xaml 期望 161

# --- 4. 三份主题字典的三态边界 ---
Select-String -Path "$dir\themeresources.xaml" -Pattern '<ResourceDictionary\s+x:Key="' |
  ForEach-Object { "L$($_.LineNumber): $($_.Line.Trim())" }
# 期望 L4 Default / L1962 HighContrast / L3920 Light，L5878 关闭 ThemeDictionaries

# --- 5. WinUI 2 新增控件是否在 OS 快照里（负数取证） ---
foreach ($k in @('TabView','InfoBar','Expander','PipsPager','NumberBox','BreadcrumbBar','TeachingTip','AnimatedIcon','PopupThemeAnimation','ControlCornerRadius','OverlayCornerRadius')) {
  "{0,-22} generic={1,-5} themeresources={2}" -f $k, (Select-String -Path "$dir\generic.xaml" -Pattern $k).Count, (Select-String -Path "$dir\themeresources.xaml" -Pattern $k).Count
}
# 期望全为 0；NavigationView / RatingControl 不为 0（它们进了 OS）。

# --- 6. ThemeAnimation 属性签名（证明「读不到几何」） ---
$txt = (Get-Content "$dir\generic.xaml") -join "`n"
[regex]::Matches($txt, '<([A-Za-z]*ThemeAnimation)\b((?:[^<>])*)/?>') |
  ForEach-Object { ($_.Groups[1].Value + ' ||| ' + (($_.Groups[2].Value -replace '\s+',' ').Trim() -replace 'Storyboard\.TargetName="[^"]*"','')) } |
  Group-Object | Sort-Object Name | Select-Object Count, Name
```

### 6.3 分组用的前缀表（§1.2 / §2 / §3 共用）

取「键名的最长匹配前缀」，表内按长度降序比较；无匹配时兜底取首段驼峰词并以 `†` 标注。

```text
MediaTransportControls, ToggleMenuFlyoutItem, NavigationBackButton, CalendarDatePicker, HyperlinkFocusRect, RefreshVisualizer, ListPickerFlyout, LanguageSwitcher, PaneToggleButton, SmallScrollThumb, AutoSuggestList, HandwritingView, FlyoutPresenter, HyperlinkButton, AutoSuggestBox, DropDownButton, DateTimeFlyout, ContentControl, NavigationView, SettingsFlyout, ContentDialog, RatingControl, ColorSpectrum, PersonPicture, ToggleButton, PickerFlyout, ProgressRing, SemanticZoom, ScrollViewer, CalendarView, SwipeControl, TreeViewItem, HelperButton, ToggleSwitch, RepeatButton, ColorPicker, PasswordBox, SplitButton, RadioButton, RichEditBox, TextControl, ProgressBar, StackPanel, TimePicker, MenuFlyout, InkToolbar, ButtonBase, DatePicker, CommandBar, SplitView, ScrollBar, SearchBox, TextBlock, TextStyle, Rectangle, FontIcon, CheckBox, ComboBox, GridView, FlipView, ListView, TextBox, ToolTip, TabView, MenuBar, Control, InfoBar, ListBox, Flyout, AppBar, Button, KeyTip, Slider, Thumb, Pivot, Grid, Hub, MTC
```

> 注：`Button` 只吃 `Button*`；`RepeatButton` / `RadioButton` / `HyperlinkButton` / `ToggleButton` / `SplitButton` / `DropDownButton` 各自成组（它们不以 `Button` 开头）。
> `Grid` 与 `GridView`、`MenuBar` 与 `MenuFlyout`、`ScrollBar` 与 `ScrollViewer` 同理由最长匹配区分。

### 6.4 已知局限（诚实登记）

1. **元素属性的 C 表不含 `{TemplateBinding …}` / `{ThemeResource …}`**（按题目要求只收「写死」）。
   因此模板里 `Padding="{TemplateBinding Padding}"` 这类**转发**不会出现——它们不是常量，是控件属性的透传。
2. **`generic.xaml` 与 `themeresources.xaml` 是两套并行模板**，同一个控件的同一属性可能在两处各写一份且取值不同。
   本表不做合并裁决，两处都列出。
3. **Style 级 Setter 归到 `Style TargetType`**，可能与「实际被谁用」不同（例如某个 `Button` 样式只服务 AppBar）。
   模板级（A 表 `层级=模板级`）才是该控件模板的权威默认值。
4. 行号对 `generic.xaml` 为 1-based；该文件含大量回车换行混合，行号按 `Get-Content` 的行计数给出。
5. **`ScrollViewer` / `Border` / `Grid` 等布局原语里的写死值在内**（C 表），它们是模板内部结构，不是主题常量。

