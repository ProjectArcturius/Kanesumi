# 动画权威取数：UWP OS 控件模板 vs WinUI 2.8.6

> 只读研究。未修改任何其他文件，未运行 cargo。
> 目的：把 Kanesumi 中标注「未在快照中」的动画时长/缓动，从本机一手源逐条取出，给 M6-1 / M6-2 提供权威值。
> 生成日期：本机 Windows，源文件 mtime 见下。

## 0 · 源清单与取数方法

| 代号 | 路径 | 大小 | 行数 |
|---|---|---|---|
| **A** | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\generic.xaml` | 2 653 934 B | 29 184（Read 工具按 31 189 计数，末尾空行计入） |
| **A′** | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml` | 1 248 103 B | 13 125 |
| **A2** | `...\UAP\10.0.22621.0\Generic\generic.xaml` + `themeresources.xaml` | 同 | 同 |
| **B** | `C:\Users\mc158\.nuget\packages\microsoft.ui.xaml\2.8.6\lib\uap10.0\Microsoft.UI.Xaml\Themes\Generic.xaml` | 371 890 B | 4 683 |

下文 `文件:行号` 中：
- `A:NNNN` = 上面的 UWP `generic.xaml`（26100）
- `A′:NNNN` = UWP `themeresources.xaml`（26100）
- `B:NNNN` = WinUI 2.8.6 `Generic.xaml`

**⚠ 三条必须先说的取数边界（决定了哪些条目「未找到」）**

1. **OS 侧的时长常量资源不在 XAML 里。** 全文件正则搜索 `AnimationDuration|AnimationDelay|ControlFast|ControlNormal|ControlSlow` 在 **A、A′、A2、B 四个 XAML 文件中均 0 命中**；进一步把 `C:\Windows\System32\Windows.UI.Xaml.dll`（17 940 480 B）整文件按 UTF-16 扫描，`ControlFastAnimationDuration` / `ControlNormalAnimationDuration` / `ControlSlowAnimationDuration` / `ControlFasterAnimationDuration` / `ControlFastOutSlowInKeySpline` **同样 0 命中** → **UWP 运行时本身不定义这些键**。
   `ControlFastAnimationDuration` / `ControlNormalAnimationDuration` / `ControlFasterAnimationDuration` 只存在于 **WinUI 2.x / WinUI 3**（WinUI 2.8.6 的 `Microsoft.UI.Xaml.2.8.appx` → `resources.pri` 字符串表中各 ×12，见 §1）。
2. **XAML 里存在的时长资源只有 ScrollBar / ScrollViewer / SplitView / HandwritingView 这几族**，全部在 A′ 里以 `<x:String>` 形式定义（§1 表）。
3. **凡模板里写 `<PointerDownThemeAnimation/>`、`<RepositionThemeAnimation/>`、`<FadeInThemeAnimation/>`、`<SplitOpenThemeAnimation/>`、`<EntranceThemeTransition/>` 而没写 `Duration`/`SpeedRatio` 的，取值由 native 代码预置，XAML 不可读**。官方 API 文档明确写了这一点（PointerDownThemeAnimation 的 Remarks："Setting the Duration property has no effect on this object as the duration is preconfigured."）——见 §2.3 与 §9 外部佐证。

---

## 1 · 全局时长常量资源

### 1.1 `Control*AnimationDuration` 一族

| 资源名 | 值 | 来源 | 结论 |
|---|---|---|---|
| `ControlNormalAnimationDuration` | 250 ms | [Microsoft Learn · Timing and easing](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing) | **不在 A / A′ / B 的快照中**；WinUI 3 平台资源 |
| `ControlFastAnimationDuration` | 167 ms | 同上 | **不在快照中** |
| `ControlFasterAnimationDuration` | 83 ms | 同上 | **不在快照中** |
| `ControlFastAnimationAfterDuration` | 168 ms | [unoplatform/uno#7168](https://github.com/unoplatform/uno/issues/7168) 引 WinUI 2.6 定义 | **不在 A / A′ 中**；WinUI 2 侧确认存在（见下） |
| `ControlFastOutSlowInKeySpline` | 键名确认存在；**值未取到**（[inferred] 疑为 `0.1,0.9,0.2,1`） | 见下 | **不在 A / A′ 中** |
| `ControlSlowAnimationDuration` | — | — | **未找到**（该键名在 Microsoft 官方文档、三个 XAML 源、以及 `Windows.UI.Xaml.dll` 中均无命中）。WinUI 标准表只有 Normal/Fast/Faster 三档。 |

**四层取证（三层本地 + 一层文档），结论：这几个键是 WinUI 2.x/3 专有，UWP 本身不定义。**

| 取证位置 | `ControlFast/ Normal/ Slow/ Faster AnimationDuration`、`ControlFastOutSlowInKeySpline` 命中数 |
|---|---|
| A（UWP SDK 26100 generic.xaml） | **0** |
| A′（UWP SDK 26100 themeresources.xaml） | **0** |
| A2（UWP SDK 22621 两份） | **0** |
| B（WinUI 2.8.6 `Themes\Generic.xaml`） | **0**（B 只是转发引用，定义在 PRI 里） |
| **`C:\Windows\System32\Windows.UI.Xaml.dll`**（UTF-16 全量扫描，17 940 480 B） | **0 × 5 个键名** → **UWP 运行时本身不定义这些资源**（本次实测） |
| **WinUI 2.8.6 `tools\AppX\x64\Release\Microsoft.UI.Xaml.2.8.appx` → `resources.pri`**（10 612 128 B，UTF-16 字符串表） | `ControlFastAnimationDuration` ×12、`ControlNormalAnimationDuration` ×12、`ControlFasterAnimationDuration` ×12、`ControlFastAnimationAfterDuration` ×12、`ControlFastOutSlowInKeySpline` ×12、**`ControlSlowAnimationDuration` ×0**；XBF 值流中含 `00:00:00.250` ×5、`00:00:00.167` ×15、`00:00:00.168` ×5、`00:00:00.083` ×13 |

WinUI 2.6 的原文定义（[unoplatform/uno#7168](https://github.com/unoplatform/uno/issues/7168) 引用，与上表 PRI 值流完全一致）：

```xml
<x:String x:Key="ControlNormalAnimationDuration">00:00:00.250</x:String>
<x:String x:Key="ControlFastAnimationDuration">00:00:00.167</x:String>
<x:String x:Key="ControlFastAnimationAfterDuration">00:00:00.168</x:String>
<x:String x:Key="ControlFasterAnimationDuration">00:00:00.083</x:String>
```

> **⚠ 对 Kanesumi 的直接后果**：`ControlFastAnimationDuration` 等键**不得在 UWP 风格 XAML 中引用**（UWP 解析不到，A/A′/A2/B 与 `Windows.UI.Xaml.dll` 全 0 命中）。若要对齐，只能用字面量或自建 ThemeResource：**167 / 250 / 83 ms**，配 spline `0.1,0.9,0.2,1`。
>
> **`ControlFastOutSlowInKeySpline` 的值 = 未取到**（XBF 值流不可读，远程代码搜索被限流）。**旁证**：A 全文件中 `0.1,0.9,0.2,1` 出现 **34 次**、`0.1,0.9 0.2,1.0` 出现 **79 次**（同一曲线的两种写法，是 A 的**主导 spline**）；WinUI 2.8.6 的 PRI 中含字符串 `0.1,0.9,0.2,1` ×118。**但这是推断，不是文档确证——不得当作权威写入规格。**

**A 全文件中写死的 `Duration=` / `GeneratedDuration=` 去重全集**（备查）：`0:0:0.467` / `0:0:0.333` / `0:0:0.300` / `0:0:0.25` / `0:0:0.240` / `0:0:0.2` / `0:0:0.167` / `0:0:0.15` / `0:0:0.083` / `0:0:0` / `0`。
**主导 KeySpline 频次**：`0.1,0.9 0.2,1.0` ×79、`0.1,0.9,0.2,1` ×34、`0.2,0 0,1` ×51、`0.0,0.35 0.15,1.0` ×12、`0.7,0 1,0.5` ×10、`0.4,0,0.6,1` ×10。
（注：`0:0:0.333` **只**以 `KeyTime=`/`Duration=` 出现，**从不作为 `GeneratedDuration`**。）

### 1.2 快照里**真实存在**的全局时长资源（A′，全部为 `<x:String>` 时长字面量）

| 资源名 | 值 | 文件:行号 | 照抄整行 |
|---|---|---|---|
| `ScrollBarExpandDuration` | 100 ms | `A′:605` | `<x:String x:Key="ScrollBarExpandDuration">00:00:00.1</x:String>` |
| `ScrollBarExpandBeginTime` | 400 ms | `A′:606` | `<x:String x:Key="ScrollBarExpandBeginTime">00:00:00.40</x:String>` |
| `ScrollBarContractBeginTime` | 2 s | `A′:607` | `<x:String x:Key="ScrollBarContractBeginTime">00:00:02.00</x:String>` |
| `ScrollBarContractDelay` | 2 s | `A′:608` | `<x:String x:Key="ScrollBarContractDelay">00:00:02</x:String>` |
| `ScrollBarContractDuration` | 100 ms | `A′:609` | `<x:String x:Key="ScrollBarContractDuration">00:00:00.1</x:String>` |
| `ScrollBarContractFinalKeyframe` | 2.1 s | `A′:610` | `<x:String x:Key="ScrollBarContractFinalKeyframe">00:00:02.1</x:String>` |
| `HandwritingViewGestureTipsElementEaseInDuration` | 100 ms | `A′:615` | `<x:String x:Key="HandwritingViewGestureTipsElementEaseInDuration">00:00:00.1</x:String>` |
| `ScrollViewerSeparatorExpandBeginTime` | 400 ms | `A′:627` | `<x:String x:Key="ScrollViewerSeparatorExpandBeginTime">00:00:00.40</x:String>` |
| `ScrollViewerSeparatorExpandDuration` | 100 ms | `A′:628` | `<x:String x:Key="ScrollViewerSeparatorExpandDuration">00:00:00.1</x:String>` |
| `ScrollViewerSeparatorContractBeginTime` | 2 s | `A′:629` | `<x:String x:Key="ScrollViewerSeparatorContractBeginTime">00:00:02.00</x:String>` |
| `ScrollViewerSeparatorContractDelay` | 2 s | `A′:630` | `<x:String x:Key="ScrollViewerSeparatorContractDelay">00:00:02</x:String>` |
| `ScrollViewerSeparatorContractDuration` | 100 ms | `A′:631` | `<x:String x:Key="ScrollViewerSeparatorContractDuration">00:00:00.1</x:String>` |
| `ScrollViewerSeparatorContractFinalKeyframe` | 2.1 s | `A′:632` | `<x:String x:Key="ScrollViewerSeparatorContractFinalKeyframe">00:00:02.1</x:String>` |
| `SplitViewPaneAnimationOpenDuration` | 200 ms | `A′:1328`（又见 3286 / 5244 / `A:1416`） | `<x:String x:Key="SplitViewPaneAnimationOpenDuration">00:00:00.2</x:String>` |
| `SplitViewPaneAnimationOpenPreDuration` | 199.99 ms | `A′:1329` | `<x:String x:Key="SplitViewPaneAnimationOpenPreDuration">00:00:00.19999</x:String>` |
| `SplitViewPaneAnimationCloseDuration` | 100 ms | `A′:1330` | `<x:String x:Key="SplitViewPaneAnimationCloseDuration">00:00:00.1</x:String>` |

**两源差异**：B（WinUI 2.8.6）里**完全没有** `x:String x:Key` 形式的资源定义——它只覆写「新控件」样式，经典控件的色值/时长资源引用系统 `ThemeResources`（native）。A2（22621）与 A（26100）在这些行上**逐字节相同**（两份文件均 29 184 行，同行号同内容），无版本差异。

**「引用 vs 写死」标注**：上表是**定义**；以下模板行是**引用**：
- `A:9046–9063 / 9078–9079 / 9104–9119 / 9136–9151` 用 `Duration="{ThemeResource ScrollBarContractDuration}"` / `ScrollBarExpandDuration` + `BeginTime="{ThemeResource …BeginTime}"`。
- `A:9437 / 9449` 用 `Duration="{ThemeResource ScrollViewerSeparatorContractDuration}"` / `…ExpandDuration`。
- **B:1663 / 1668 / 1675 / 1680**（`ScrollView`）引用的是 WinUI 3 命名：`ScrollViewScrollBarsSeparatorContractDuration` / `…ContractDelay` / `…ExpandDuration` / `…DisplayDelayWithoutAnimation` / `…ExpandDelayWithoutAnimation` / `…ContractDelayDisabled` / `…ExpandDelayWithoutAnimation` —— **这些键的定义不在 B 内**（B 只有引用，无定义），属 native 资源。

---

## 2 · ButtonBase / Button

### 2.1 UWP `Button` 默认模板（A:6238–6333）

**Normal / PointerOver / Pressed / Disabled 四种状态通篇没有写任何时长**：颜色全部走 `ObjectAnimationUsingKeyFrames` + `DiscreteObjectKeyFrame KeyTime="0"`，即**瞬时切换（0 ms）**。

`PointerOver`（A:6278–6292）原文：

```xml
<VisualState x:Name="PointerOver">

    <Storyboard>
        <ObjectAnimationUsingKeyFrames Storyboard.TargetName="ContentPresenter" Storyboard.TargetProperty="Background">
            <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource ButtonBackgroundPointerOver}" />
        </ObjectAnimationUsingKeyFrames>
        <ObjectAnimationUsingKeyFrames Storyboard.TargetName="ContentPresenter" Storyboard.TargetProperty="BorderBrush">
            <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource ButtonBorderBrushPointerOver}" />
        </ObjectAnimationUsingKeyFrames>
        <ObjectAnimationUsingKeyFrames Storyboard.TargetName="ContentPresenter" Storyboard.TargetProperty="Foreground">
            <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource ButtonForegroundPointerOver}" />
        </ObjectAnimationUsingKeyFrames>
        <PointerUpThemeAnimation Storyboard.TargetName="ContentPresenter" />
    </Storyboard>
</VisualState>
```

`Pressed`（A:6294–6308）末尾一行 —— **Button 的「下沉」就在这里，且只有一行、无任何参数**：

```xml
        <PointerDownThemeAnimation Storyboard.TargetName="ContentPresenter" />
```

`Disabled`（A:6310–6323）：同样是 3 个 `DiscreteObjectKeyFrame KeyTime="0"`，**没有 PointerUp/Down，也没有任何时长**。

### 2.2 逐条结论表

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 | 备注 |
|---|---|---|---|---|---|
| Button | Normal | 0 ms | 无（瞬时） | `A:6271–6276` | 仅 `PointerUpThemeAnimation` |
| Button | PointerOver | **0 ms**（模板写死 `KeyTime="0"`） | 无（Discrete） | `A:6278–6292` | + `PointerUpThemeAnimation`（无参数） |
| Button | Pressed | **0 ms** 颜色；**主题动画时长未在快照** | 无（Discrete）+ PointerDown 预置曲线 | `A:6294–6308` | + `PointerDownThemeAnimation`（无参数） |
| Button | Disabled | **0 ms** | 无（Discrete） | `A:6310–6323` | 无 Pointer 动画 |
| Button | `PointerDownThemeAnimation` 的 `Duration` | **未在快照中**（XAML 未写 → native 预置；官方文档：写 Duration 无效，见 §9） | 未在快照 | `A:6306` | 参数一律缺省 |
| Button | `PointerDownThemeAnimation` 的 `SpeedRatio` | **未在快照中**（A 全文件 `SpeedRatio` = **0 命中**） | — | — | 同族全 0 命中 |
| Button | `PointerUpThemeAnimation` 的 `Duration` / `SpeedRatio` | **未在快照中**（XAML 未写） | 未在快照 | `A:6274` | — |

**`SpeedRatio` 在 A、A′、B 三个文件中 0 命中** —— 所有主题动画/过渡的 SpeedRatio 都是预置值，快照中不可读。

### 2.3 同族按钮（用同一对动画，参数同样缺省）

**精确计数（本次实测，含两个文件）**：

| | `<PointerDownThemeAnimation>` | `<PointerUpThemeAnimation>` |
|---|---|---|
| A（generic.xaml 26100） | **38** | **78** |
| A′（themeresources.xaml 26100） | **26** | **53** |
| 合计 | **64** | **131** |

**这 195 个标签中，带 `Duration` / `SpeedRatio` / `KeySpline` / `From` / `To` / `FromHorizontalOffset` / `FromVerticalOffset` 任一属性的数量 = `0`**（精确正则实测）。全部只有 `TargetName` 或 `Storyboard.TargetName`。

代表行：

| 控件 | Down 行 | Up 行 |
|---|---|---|
| RepeatButton | `A:6493` | `A:6461 / 6477` |
| ToggleButton | `A:6554 / 6613` | `A:6524/6539/6583/6598` |
| HyperlinkButton | `A:6901` | `A:6875 / 6888` |
| CheckBox（Reveal/树内） | `A:7231 / 7070` | `A:7042/7047/7056…` |
| RadioButton | — | — |
| ListViewItem / GridViewItem | `A:8459 / 9587 / 10773 / 10863` | `A:8429/8444/9555/9571/10727/10750/10803/10833` |
| ComboBoxItem | `A:10773 / 10863` | `A:10727/10750/10803/10833` |
| NavigationViewItem | `A:21989 / 22497 / 22630 / 22674 / 22716` | 相邻偶数行 |
| AppBarButton / AppBarToggleButton | `A:23793…24255` 一带 | 同带内 `PointerUpThemeAnimation` |
| MenuBarItem | `A:25283` | `A:25251 / 25267` |
| SplitButton 内嵌 Button | `A:28140 / 28221` | `A:28094/28117/28167/28194` |
| ToggleSwitch knob | `A:28594`（`CheckboxTiltContainer`） | `A:28581 / 28588` |

> **注意**：`PointerDownThemeAnimation` 在 A 中**大量出现在 ListViewItem/GridViewItem 的 Phone 向 tilt 分支**（`TiltContainer` / `CheckboxTiltContainer`），桌面 UWP 语义是「轻微缩小」（§9 官方文档原文："On Windows, the animation slightly shrinks the item to indicate that it is pressed; on Windows Phone, the animation tilts the item slightly around the positive y-axis in a 2.5D effect."）。

### 2.4 WinUI 2.8.6 侧

**B 里根本没有经典 `Button` 的隐式样式。** B 的 `<Style>` 清单（全 4 683 行的样式表）只有 WinUI 2 新增控件 + 少数覆写：`RatingControl / InfoBadge / NavigationView / FlyoutPresenter / NavigationViewItem / NavigationViewItemHeader / NavigationViewItemSeparator / NavigationBackButtonNormal|Small / ColorPicker / ColorPickerSlider / SliderThumb / ColorSpectrum / PersonPicture / RefreshContainer / RefreshVisualizer / MenuBar / MenuBarItem / TreeView / TreeViewItem / ScrollView / SwipeControl / TwoPaneView / SplitButton / ToggleSplitButton / DropDownButton / RadioButtons / RadioButton(转发) / TeachingTip / TabView / TabViewItem / ProgressBar / ProgressRing / NumberBox / Expander / PagerControl / InfoBar / HyperlinkButton / BreadcrumbBar / BreadcrumbBarItem / PipsPager / ImageIcon / CommandBarFlyoutCommandBar`。
- 其中出现 `TargetType="Button"` 的位置只有 3 处，全部是**局部资源覆写**（NavigationBackButton `B:665`、SplitButton 内嵌按钮 `B:1829`、BreadcrumbBarItem 内嵌 `B:4319`），且这三处的 `Normal/PointerOver/Pressed` 都改用 `VisualState.Setters`（**无 Storyboard、无动画**）。
- 因此 **WinUI 2.8.6 下 `Button`/`CheckBox`/`ToggleSwitch`/`ComboBox` 的模板与动画来自 UWP 系统 `ThemeResources`（native），B 内不可见** —— 与 A 的结论一致，且**B 无法提供任何额外时长**。

---

## 3 · CheckBox / RadioButton / ToggleSwitch

### 3.1 CheckBox（A:6997–7360）

**`Checked` 视觉态没有动画**：`CheckedNormal`（A:7119–7149）里勾选出现是 `Duration="0"` 的两条 `DoubleAnimation`：

```xml
<DoubleAnimation Storyboard.TargetName="NormalRectangle"
Storyboard.TargetProperty="StrokeThickness"
To="{ThemeResource CheckBoxCheckedStrokeThickness}"
Duration="0" />
<DoubleAnimation Storyboard.TargetName="CheckGlyph"
Storyboard.TargetProperty="Opacity"
To="1"
Duration="0" />
```

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| CheckBox | Unchecked→Checked（勾号出现） | **0 ms**（`Duration="0"`） | 无 | `A:7140–7147` |
| CheckBox | Checked 边框线宽（StrokeThickness） | **0 ms** | 无 | `A:7140–7143` |
| CheckBox | 全部 PointerOver/Pressed/Disabled 色变 | **0 ms** | 无（Discrete `KeyTime="0"`） | `A:7046–7353` |
| CheckBox | 状态组名 | — | — | `A:7022` `<VisualStateGroup x:Name="CombinedStates">`（组合态命名：`UncheckedNormal` / `UncheckedPointerOver` / `UncheckedPressed` / `UncheckedDisabled` / `CheckedNormal` / `CheckedPointerOver` / `CheckedPressed` / `CheckedDisabled` / `Indeterminate*`） |

> **结论：UWP CheckBox 的勾选视觉态是「瞬切」，没有时长可抄。** Kanesumi 若要「勾号淡入/缩放」，是自加效果，不是 UWP 行为。

### 3.2 RadioButton（A:6795–6995）

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 | 原文 |
|---|---|---|---|---|---|
| RadioButton | CheckStates → `Checked` | **0 ms** | 无 | `A:6924–6940` | 见下 |
| RadioButton | PointerOver / Pressed / Disabled | **0 ms** | 无（Discrete） | `A:6822–6919` | — |

```xml
<VisualState x:Name="Checked">

    <Storyboard>
        <DoubleAnimation Storyboard.TargetName="CheckGlyph"
        Storyboard.TargetProperty="Opacity"
        To="1"
        Duration="0" />
        <DoubleAnimation Storyboard.TargetName="OuterEllipse"
        Storyboard.TargetProperty="Opacity"
        To="0"
        Duration="0" />
        <DoubleAnimation Storyboard.TargetName="CheckOuterEllipse"
        Storyboard.TargetProperty="Opacity"
        To="1"
        Duration="0" />
    </Storyboard>
</VisualState>
```

> **另一个 RadioButton 动画不在本文件**：`A:14472–14473` 的 `VisualTransition From="Unselected" To="UnselectedLocked" GeneratedDuration="0:0:0.33"` 与反向同为 **0.33 s** —— 但那是 **`ContentDialog` 内 `RadioButton` 的 Locked 态**（`A:14476–14512` 的 `Disabled/Unselected/Selected` 组），不是普通勾选。

### 3.3 ToggleSwitch knob 位移（A:12883–13245）

**knob 的开关位移同样是瞬切 + 一个无参 `RepositionThemeAnimation`**，三条过渡的 `GeneratedDuration` 全部写死为 `0`：

```xml
<VisualTransition x:Name="OffToOnTransition"
  From="Off"
  To="On"
  GeneratedDuration="0">

    <Storyboard>
        <RepositionThemeAnimation TargetName="SwitchKnob" FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.KnobOffToOnOffset}" />
        <ObjectAnimationUsingKeyFrames Storyboard.TargetName="SwitchKnobBounds" Storyboard.TargetProperty="Opacity">
            <DiscreteObjectKeyFrame KeyTime="0" Value="1" />
        </ObjectAnimationUsingKeyFrames>
        …
    </Storyboard>
</VisualTransition>
```
（`A:13042–13062`）

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| ToggleSwitch | `OffToOnTransition`（knob 吸合到 On） | `GeneratedDuration="0"`；实际位移由 `RepositionThemeAnimation` 预置时长驱动 → **未在快照中** | 未在快照 | `A:13042–13062` |
| ToggleSwitch | `OnToOffTransition`（knob 回 Off） | `GeneratedDuration="0"` + `RepositionThemeAnimation` → **未在快照中** | 未在快照 | `A:13033–13041` |
| ToggleSwitch | `DraggingToOnTransition` / `DraggingToOffTransition` | `GeneratedDuration="0"` + `RepositionThemeAnimation` → **未在快照中** | 未在快照 | `A:13003–13032` |
| ToggleSwitch | `On` 状态（`KnobTranslateTransform.X`） | **0 ms**，`To="24"`（**位移写死 24 px**） | 无 | `A:13069–13072` |
| ToggleSwitch | `On`/`Off` 色变与不透明度 | **0 ms** | 无（Discrete） | `A:13073–13084` |
| ToggleSwitch | OffContent / OnContent 切换 | **0 ms** | 无 | `A:13090–13121` |

```xml
<DoubleAnimation Storyboard.TargetName="KnobTranslateTransform"
Storyboard.TargetProperty="X"
To="24"
Duration="0" />
```
（`A:13069–13072`）

> knob 行程：模板里写死 `To="24"`（模板坐标系）；`RepositionThemeAnimation` 的 `FromHorizontalOffset` 由 `TemplateSettings.KnobOffToOnOffset` 等 native 计算值提供。
> **B 侧无 `ToggleSwitch` 样式**（B 样式表无 `TargetType="ToggleSwitch"`），WinUI 2.8.6 的 ToggleSwitch 走 UWP 系统模板，与本结果一致。

---

## 4 · ProgressBar

**⚠ 两源给出的是两个不同的 ProgressBar 视觉模型，必须并列列出。**

### 4.1 UWP OS（A:12074–12353）—— 五圆点 / E1–E5 + B1–B5 模型

```xml
<VisualState x:Name="Indeterminate">
    <Storyboard RepeatBehavior="Forever">
        <DoubleAnimation Storyboard.TargetName="IndeterminateRoot"
        Duration="0:0:3.917"
        Storyboard.TargetProperty="(UIElement.RenderTransform).(TranslateTransform.X)"
        From="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationStartPosition}"
        To="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationEndPosition}" />
```
（`A:12116–12122`）

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| ProgressBar | Indeterminate 整体位移（`IndeterminateRoot.TranslateTransform.X`） | **3.917 s**，`RepeatBehavior="Forever"` | **无（线性 DoubleAnimation）** | `A:12117–12122` |
| ProgressBar | Indeterminate E1–E5 圆点位移 | 关键帧 `0 → 1 s（Well）→ 2 s（Well 保持）→ 3 s（End）`，逐点 +0.167 s 错峰 | `KeySpline="0.4,0,0.6,1"`（对称 ease-in-out） | `A:12123–12156` |
| ProgressBar | Indeterminate B1–B5 背板位移 | `0 → 0.5/0.667/0.833/1/1.167 s` 归 0，`2/2.167/…/2.667 s` 保持，`3/3.167/…/3.667 s → 100` | 无（`EasingDoubleKeyFrame` 默认） | `A:12157–12186` |
| ProgressBar | Indeterminate E1–E5 不透明度 | 离散：`0` 起，`3/3.167/3.333/3.5/3.667 s` 置 0 | 无 | `A:12202–12225` |
| ProgressBar | Indeterminate→Determinate 过渡 | `FadeInThemeAnimation`（**时长未在快照**） | 未在快照 | `A:12107–12112` |
| ProgressBar | **Updating→Determinate（Reposition，用户问的「Reposition 动画」）** | **未在快照中**（`RepositionThemeAnimation` 无 Duration） | 未在快照 | `A:12092–12097` |
| ProgressBar | `Error` 态 | **0 ms**（`DiscreteObjectKeyFrame KeyTime="0"` 置 Opacity=0） | 无 | `A:12228–12235` |
| ProgressBar | `Paused` 态（Opacity → `ProgressBarIndicatorPauseOpacity`） | **0.25 s**（`Duration="0:0:0.25"`，**写死**） | 无（DoubleAnimation 默认线性） | `A:12236–12247` |
| ProgressBar | `Paused`→`Determinate` 过渡（Opacity→1） | **0.25 s**（写死） | 无（线性） | `A:12098–12106` |

`Paused` 原文（A:1236x）：

```xml
<VisualState x:Name="Paused">

    <Storyboard>
        <ObjectAnimationUsingKeyFrames Storyboard.TargetName="ProgressBarIndicator" Storyboard.TargetProperty="Fill">
            <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource SystemControlForegroundAccentBrush}" />
        </ObjectAnimationUsingKeyFrames>
        <DoubleAnimation Storyboard.TargetName="ProgressBarIndicator"
        Storyboard.TargetProperty="Opacity"
        To="{ThemeResource ProgressBarIndicatorPauseOpacity}"
        Duration="0:0:0.25" />
    </Storyboard>
</VisualState>
```
（`A:12236–12247`）

`RepositionThemeAnimation` 原文（A:12092–12097）：

```xml
<VisualTransition From="Updating" To="Determinate">

    <Storyboard>
        <RepositionThemeAnimation TargetName="ProgressBarIndicator" FromHorizontalOffset="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.IndicatorLengthDelta}" />
    </Storyboard>
</VisualTransition>
```

`ProgressBarIndicatorPauseOpacity` 定义：`A:83` light = `0.6`，`A:2971` high-contrast = `1`，`A:4192` dark = `0.6`。

```xml
<x:Double x:Key="ProgressBarIndicatorPauseOpacity">0.6</x:Double>
```
（`A:83`、`A:4192`）

> **A 中不存在「两矩形 + 2.0s 循环」的 ProgressBar 模板**（全文件 `ProgressBar` 命中仅 21 处，见 §4.3）。26100 SDK 的隐式 ProgressBar 样式就是上面这套。

### 4.2 WinUI 2.8.6（B:3135–3312）—— 经典「两条指示条」模型，**循环 2.0 s**

```xml
<VisualState x:Name="Indeterminate">
  <VisualState.Setters>
    <Setter Target="IndeterminateProgressBarIndicator.Opacity" Value="1" />
    <Setter Target="IndeterminateProgressBarIndicator2.Opacity" Value="1" />
    <Setter Target="ProgressBarTrack.Opacity" Value="0" />
  </VisualState.Setters>
  <Storyboard RepeatBehavior="Forever">
    <DoubleAnimationUsingKeyFrames Storyboard.TargetName="IndeterminateProgressBarIndicator" Storyboard.TargetProperty="(UIElement.RenderTransform).(CompositeTransform.TranslateX)">
      <DiscreteDoubleKeyFrame KeyTime="0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationStartPosition}" />
      <SplineDoubleKeyFrame KeyTime="0:0:1.5" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationEndPosition}" KeySpline="0.4, 0.0, 0.6, 1.0" />
      <DiscreteDoubleKeyFrame KeyTime="0:0:2" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationEndPosition}" />
    </DoubleAnimationUsingKeyFrames>
    <DoubleAnimationUsingKeyFrames Storyboard.TargetName="IndeterminateProgressBarIndicator2" Storyboard.TargetProperty="(UIElement.RenderTransform).(CompositeTransform.TranslateX)">
      <DiscreteDoubleKeyFrame KeyType="0" … />  <!-- 原文：KeyTime="0"，见下方整段 -->
```
（`B:3225–3243`，完整原文见下）

**逐字节原文（B:3231–3243）**：

```xml
                  <Storyboard RepeatBehavior="Forever">
                    <DoubleAnimationUsingKeyFrames Storyboard.TargetName="IndeterminateProgressBarIndicator" Storyboard.TargetProperty="(UIElement.RenderTransform).(CompositeTransform.TranslateX)">
                      <DiscreteDoubleKeyFrame KeyTime="0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationStartPosition}" />
                      <SplineDoubleKeyFrame KeyTime="0:0:1.5" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationEndPosition}" KeySpline="0.4, 0.0, 0.6, 1.0" />
                      <DiscreteDoubleKeyFrame KeyTime="0:0:2" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.ContainerAnimationEndPosition}" />
                    </DoubleAnimationUsingKeyFrames>
                    <DoubleAnimationUsingKeyFrames Storyboard.TargetName="IndeterminateProgressBarIndicator2" Storyboard.TargetProperty="(UIElement.RenderTransform).(CompositeTransform.TranslateX)">
                      <DiscreteDoubleKeyFrame KeyTime="0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.Container2AnimationStartPosition}" />
                      <DiscreteDoubleKeyFrame KeyTime="0:0:0.75" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.Container2AnimationStartPosition}" />
                      <SplineDoubleKeyFrame KeyTime="0:0:2" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.Container2AnimationEndPosition}" KeySpline="0.4, 0.0, 0.6, 1.0" />
                    </DoubleAnimationUsingKeyFrames>
                  </Storyboard>
```

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| ProgressBar (WinUI 2.8.6) | Indeterminate 循环（Indicator 1） | **2.0 s**，`RepeatBehavior="Forever"`；`0→1.5 s` 位移，`1.5→2.0 s` 保持 | `KeySpline="0.4, 0.0, 0.6, 1.0"`（对称 ease-in-out） | `B:3232–3236` |
| ProgressBar (WinUI 2.8.6) | Indeterminate 循环（Indicator 2） | **2.0 s**；`0→0.75 s` 保持，`0.75→2.0 s` 位移 | `KeySpline="0.4, 0.0, 0.6, 1.0"` | `B:3237–3241` |
| ProgressBar (WinUI 2.8.6) | **Determinate `Paused`→`Determinate` 颜色** | **0.167 s**（写死 `Duration="0:0:0.167"`） | 无（ColorAnimation 默认线性） | `B:3163` |
| ProgressBar (WinUI 2.8.6) | **Determinate `Error`→`Determinate` 颜色** | **0.167 s** | 无 | `B:3168` |
| ProgressBar (WinUI 2.8.6) | `Error` / `Paused` 态换色 | **0.167 s**（**不是 0.25 s**） | 无 | `B:3217 / 3222` |
| ProgressBar (WinUI 2.8.6) | `IndeterminateError` / `IndeterminatePaused` 换色 + 收束 | **0.167 s** 换色；位移 `0.167 / 0.167 / 0.75 s` | `KeySpline="1.0, 1.0, 0.0, 1.0"` 与 `"0.0, 0.0, 0.0, 1.0"` | `B:3252–3261 / 3272–3281` |
| ProgressBar (WinUI 2.8.6) | **`Updating`→`Determinate`（Reposition）** | **未在快照中**（`RepositionThemeAnimation` 无 Duration） | 未在快照 | `B:3153` |
| ProgressBar (WinUI 2.8.6) | `Indeterminate`→`Determinate` | `FadeInThemeAnimation`×3，**时长未在快照** | 未在快照 | `B:3173–3175` |
| ProgressBar (WinUI 2.8.6) | `IndeterminatePaused`→`Indeterminate` 收束 | `0.333 s` / `0.5 s` | `KeySpline="1.0, 0.0, 1.0, 1.0"` | `B:3184–3188` |

**原文（B:3161–3170，Paused/Error→Determinate 颜色，0.167 s）**：

```xml
                  <VisualTransition From="Paused" To="Determinate">
                    <Storyboard>
                      <ColorAnimation Storyboard.TargetName="DeterminateProgressBarIndicator" Storyboard.TargetProperty="(Shape.Fill).(SolidColorBrush.Color)" To="{TemplateBinding Foreground}" Duration="0:0:0.167" />
                    </Storyboard>
                  </VisualTransition>
                  <VisualTransition From="Error" To="Determinate">
                    <Storyboard>
                      <ColorAnimation Storyboard.TargetName="DeterminateProgressBarIndicator" Storyboard.TargetProperty="(Shape.Fill).(SolidColorBrush.Color)" To="{TemplateBinding Foreground}" Duration="0:0:0.167" />
                    </Storyboard>
                  </VisualTransition>
```

**原文（B:3215–3224，Error / Paused 态，0.167 s）**：

```xml
                <VisualState x:Name="Error">
                  <Storyboard>
                    <ColorAnimation Storyboard.TargetName="DeterminateProgressBarIndicator" Storyboard.TargetProperty="(Shape.Fill).(SolidColorBrush.Color)" To="{ThemeResource ProgressBarErrorForegroundColor}" Duration="0:0:0.167" />
                  </Storyboard>
                </VisualState>
                <VisualState x:Name="Paused">
                  <Storyboard>
                    <ColorAnimation Storyboard.TargetName="DeterminateProgressBarIndicator" Storyboard.TargetProperty="(Shape.Fill).(SolidColorBrush.Color)" To="{ThemeResource ProgressBarPausedForegroundColor}" Duration="0:0:0.167" />
                  </Storyboard>
                </VisualState>
```

### 4.3 两源差异汇总

| 项 | UWP OS（A，26100） | WinUI 2.8.6（B） | Kanesumi 现用 |
|---|---|---|---|
| Indeterminate 视觉模型 | 五圆点（E1–E5）+ 背板（B1–B5）+ `IndeterminateRoot` 整条位移 | 两条 `Rectangle` 指示条（`IndeterminateProgressBarIndicator` / `…2`）+ `ProgressBarTrack` | 见 §9 |
| Indeterminate 循环 | **3.917 s**（`RepeatBehavior="Forever"`，线性整体位移；圆点关键帧以 3 s 为动作段） | **2.0 s**（`RepeatBehavior="Forever"`） | 2.0 s |
| Indeterminate 缓动 | 圆点位移 `0.4,0,0.6,1`；整体位移线性 | `0.4, 0.0, 0.6, 1.0` | ease-in-out 近似 |
| Paused 视觉 | `Opacity → 0.6`（资源 `ProgressBarIndicatorPauseOpacity`） | 换 `ProgressBarPausedForegroundColor` 颜色 | alpha 1.0→0.6 |
| Paused / Error 时长 | **0.25 s**（Opacity / 恢复 0.25 s；Error 本身 0 s） | **0.167 s**（颜色） | 0.25 s |
| Reposition（值变化）时长 | **未在快照中** | **未在快照中** | 0.15 s |

> **注意**：`A2`（22621）与 `A`（26100）在 ProgressBar 段**完全一致**（同行号同内容）。
> `MediaSliderProgressBarStyle`（`A:15397`）只是把 `MinHeight` 换成 `4`（`A:15397` 起，实际用 `ProgressBarThemeMinHeight`），**没有独立 Storyboard**，动画沿用上表。

---

## 5 · ProgressRing

### 5.1 UWP OS（A:12355–12580）—— 6 圆点 + RotateTransform，纯 XAML 关键帧

```xml
<VisualState x:Name="Active">
    <Storyboard RepeatBehavior="Forever">
        <ObjectAnimationUsingKeyFrames Duration="0" Storyboard.TargetName="Ring" Storyboard.TargetProperty="Visibility">
            <DiscreteObjectKeyFrame KeyTime="0">
                <DiscreteObjectKeyFrame.Value>
                    <Visibility>Visible</Visibility>
                </DiscreteObjectKeyFrame.Value>
            </DiscreteObjectKeyFrame>
        </ObjectAnimationUsingKeyFrames>
        <DoubleAnimationUsingKeyFrames Storyboard.TargetName="E1" Storyboard.TargetProperty="Opacity" BeginTime="0">
            <DiscreteDoubleKeyFrame KeyTime="0" Value="1" />
            <DiscreteDoubleKeyFrame KeyTime="0:0:3.21" Value="1" />
            <DiscreteDoubleKeyFrame KeyTime="0:0:3.22" Value="0" />
            <DiscreteDoubleKeyFrame KeyTime="0:0:3.47" Value="0" />
        </DoubleAnimationUsingKeyFrames>
        …
        <DoubleAnimationUsingKeyFrames Storyboard.TargetName="E1R" BeginTime="0" Storyboard.TargetProperty="Angle">
            <SplineDoubleKeyFrame KeyTime="0" Value="-110" KeySpline="0.13,0.21,0.1,0.7" />
            <SplineDoubleKeyFrame KeyTime="0:0:0.433" Value="10" KeySpline="0.02,0.33,0.38,0.77" />
            <SplineDoubleKeyFrame KeyTime="0:0:1.2" Value="93" />
            <SplineDoubleKeyFrame KeyTime="0:0:1.617" Value="205" KeySpline="0.57,0.17,0.95,0.75" />
            <SplineDoubleKeyFrame KeyTime="0:0:2.017" Value="357" KeySpline="0,0.19,0.07,0.72" />
            <SplineDoubleKeyFrame KeyTime="0:0:2.783" Value="439" />
            <SplineDoubleKeyFrame KeyTime="0:0:3.217" Value="585" KeySpline="0,0,0.95,0.37" />
        </DoubleAnimationUsingKeyFrames>
    </Storyboard>
</VisualState>
```
（`A:12406–12506`）

| 控件 | 状态/动画名 | 时长 | 缓动 / 关键帧 | 文件:行号 |
|---|---|---|---|---|
| ProgressRing | 循环周期（`Active` Storyboard） | **3.47 s**（`RepeatBehavior="Forever"`；`E1` 可见 0→3.21 s，3.22 s 灭，末帧 3.47 s）；**旋转动作段 3.217 s** | 见下 | `A:12407–12506` |
| ProgressRing | `E1..E6` 逐个延迟（stagger） | **0.167 s / 点**（`BeginTime`：`0`、`0.167`、`0.334`、`0.501`、`0.668`、`0.835`） | — | `A:12415`(0) / `12421`(0.167) / `12427`(0.334) / `12433`(0.501) / `12439`(0.668) / `12445`(0.835) |
| ProgressRing | `E1R..E6R` 旋转角度关键帧（RotateTransform.Angle） | 关键帧时刻：`0 / 0.433 / 1.2 / 1.617 / 2.017 / 2.783 / 3.217 s` | E1：`-110`（`0.13,0.21,0.1,0.7`）→ `10`（`0.02,0.33,0.38,0.77`）→ `93`（无）→ `205`（`0.57,0.17,0.95,0.75`）→ `357`（`0,0.19,0.07,0.72`）→ `439`（无）→ `585`（`0,0,0.95,0.37`） | `A:12451–12459` |
| ProgressRing | E2R（同构，整体 −6°） | 同上 | `-116 / 4 / 87 / 199 / 351 / 433 / 579`，同 KeySpline 序列 | `A:12460–12468` |
| ProgressRing | E3R | 同上 | `-122 / -2 / 81 / 193 / 345 / 427 / 573` | `A:12469–12477` |
| ProgressRing | E4R | 同上 | `-128 / -8 / 75 / 187 / 339 / 421 / 567` | `A:12478–12486` |
| ProgressRing | E5R | 同上 | `-134 / -14 / 69 / 181 / 331 / 415 / 561` | `A:12487–12495` |
| ProgressRing | E6R | 同上 | `-140 / -20 / 63 / 175 / 325 / 409 / 555` | `A:12496–12504` |
| ProgressRing | `Large` 态显示第六点（`SixthCircle`） | **0 ms** | 无（Discrete） | `A:12389–12400` |
| ProgressRing | `TrimStart` / `TrimEnd` 关键帧 | **未找到**（A 中 ProgressRing 用 `Ellipse` + `RotateTransform`，**没有 `TrimStart`/`TrimEnd` 属性**） | — | — |
| ProgressRing | 双段 cubic-bezier `(0.167,0.167,0.833,0.833)` | **未找到**（A 中无此控制点；实际曲线是上表这一串非对称 KeySpline；`0.167` 在此处是**时间**不是控制点） | — | — |

> **⚠ 与 Kanesumi `CONTROL_SPEC §5`（`docs/CONTROL_SPEC.md:160–161`）的冲突**：该节写「循环 2.0 s（`c_durationTicks = 20000000`）；整体旋转 0°→900°（2.5 圈），双段 cubic-bezier `(0.167,0.167,0.833,0.833)`」——这是 **WinUI 2 的 C++ `ProgressRing` 实现**（`dev/ProgressRing/`，快照已丢弃）的值，**不是本机 UWP OS XAML 的值**。本机 UWP OS 的 ProgressRing 是 6 点 + `3.47 s` 周期 + `E1R` 角度 `-110 → 585`（净 +695°，非 900°）。**两者不可混用**，见 §9 差异表。

### 5.2 WinUI 2.8.6（B:3313–3347）—— **Lottie，无 XAML 时长**

```xml
  <Style TargetType="controls:ProgressRing">
    <Setter Property="Foreground" Value="{ThemeResource ProgressRingForegroundThemeBrush}" />
    …
    <Setter Property="Template">
      <Setter.Value>
        <ControlTemplate TargetType="controls:ProgressRing">
          <Grid x:Name="LayoutRoot" Background="Transparent">
            <VisualStateManager.VisualStateGroups>
              <VisualStateGroup x:Name="CommonStates">
                <VisualState x:Name="Inactive">
                  <VisualState.Setters>
                    <Setter Target="LayoutRoot.Opacity" Value="0" />
                    <Setter Target="LottiePlayer.(AutomationProperties.AccessibilityView)" Value="Raw" />
                  </VisualState.Setters>
                </VisualState>
                <VisualState x:Name="DeterminateActive" />
                <VisualState x:Name="Active" />
              </VisualStateGroup>
            </VisualStateManager.VisualStateGroups>
            <!-- AnimatedVisualPlayer for Lottie -->
            <controls:AnimatedVisualPlayer x:Name="LottiePlayer" AutoPlay="false" Stretch="fill" Opacity="1" />
          </Grid>
        </ControlTemplate>
      </Setter.Value>
    </Setter>
  </Style>
```
（`B:3313–3347`）

> **WinUI 2.8.6 的 ProgressRing 完全无 XAML 动画**：动画体是一个 **Lottie 动画**（`AnimatedVisualPlayer`），关键帧数据在 native/DLL 资源里。**时长 / 角度 / TrimStart / TrimEnd 在 B 中 = 未找到**。这是「WinUI 2.8.6 无法提供 ProgressRing 数值」的硬结论。

---

## 6 · ComboBox / DropdownMenu（MenuFlyoutPresenter）/ ToolTip / TeachingTip

### 6.1 ComboBox —— 弹出/关闭本体是 `SplitOpen`/`SplitCloseThemeAnimation`（**时长未在快照**），但**遮罩有时长**

弹出本体（A:10289–10309）：

```xml
<VisualStateGroup x:Name="DropDownStates">
    <VisualState x:Name="Opened">

        <Storyboard>
            <SplitOpenThemeAnimation OpenedTargetName="PopupBorder"
ClosedTargetName="ContentPresenter"
OffsetFromCenter="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOffset}"
OpenedLength="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOpenedHeight}" />
        </Storyboard>
    </VisualState>
    <VisualState x:Name="Closed">

        <Storyboard>
            <SplitCloseThemeAnimation OpenedTargetName="PopupBorder"
ClosedTargetName="ContentPresenter"
OffsetFromCenter="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOffset}"
OpenedLength="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=TemplateSettings.DropDownOpenedHeight}" />
        </Storyboard>
    </VisualState>
</VisualStateGroup>
```

**遮罩（Overlay）时长写死在模板局部 Storyboard 里 —— 这是本任务里 ComboBox 唯一可抄的数**（A:10165–10178）：

```xml
                        <Grid.Resources>
                            <Storyboard x:Key="OverlayOpeningAnimation">
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetProperty="Opacity">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="0.0" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.383" KeySpline="0.1,0.9 0.2,1.0" Value="1.0" />
                                </DoubleAnimationUsingKeyFrames>
                            </Storyboard>
                            <Storyboard x:Key="OverlayClosingAnimation">
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetProperty="Opacity">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="1.0" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.216" KeySpline="0.1,0.9 0.2,1.0" Value="0.0" />
                                </DoubleAnimationUsingKeyFrames>
                            </Storyboard>
                        </Grid.Resources>
```

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| ComboBox | `Opened`（下拉本体） | **未在快照中**（`SplitOpenThemeAnimation` 无 Duration） | 未在快照 | `A:10290–10298` |
| ComboBox | `Closed`（收起本体） | **未在快照中**（`SplitCloseThemeAnimation`） | 未在快照 | `A:10299–10307` |
| ComboBox | **遮罩淡入** `OverlayOpeningAnimation` | **0.383 s**（写死 `KeyTime="0:0:0.383"`） | `KeySpline="0.1,0.9 0.2,1.0"` | `A:10166–10171` |
| ComboBox | **遮罩淡出** `OverlayClosingAnimation` | **0.216 s**（写死） | `KeySpline="0.1,0.9 0.2,1.0"` | `A:10172–10177` |
| ComboBox | `FocusedDropDown` 显示 `PopupBorder` | **0 ms** | 无（Discrete，`Duration="0"`） | `A:10275–10286` |
| ComboBox | `Focused` / `FocusedPressed` / PointerOver / Pressed / Disabled | **0 ms** | 无 | `A:10184–10274` |
| ComboBoxItem | Normal/PointerOver/Pressed/Selected | **0 ms** + `PointerUpThemeAnimation`（无参） | 无 | `A:10724–10871` |

**同名资源在不同控件里值不同（必须区分）**：`OverlayOpeningAnimation` / `OverlayClosingAnimation` 在 A 中出现 4 处：
- `A:10166–10177`（**ComboBox**）：**0.383 s / 0.216 s**，两支都用 `KeySpline="0.1,0.9 0.2,1.0"`。
- `A:11408–11419`（NavigationView 侧）：**0.467 s / 0.167 s**，淡出用 `KeySpline="0.2,0 0,1"`。
- `A:22859–22870`、`A:30273–30284`（同一族）：0.467 / 0.167。
- A′（themeresources）里的 `CommandBar` 局部 `OverlayOpeningAnimation` / `OverlayClosingAnimation`：**0.467 s（`0.1,0.9 0.2,1.0`）/ 0.167 s（`0.2,0 0,1`）**
  ```xml
              <Storyboard x:Key="OverlayOpeningAnimation">
                <DoubleAnimationUsingKeyFrames Storyboard.TargetProperty="Opacity">
                  <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="0" />
                  <SplineDoubleKeyFrame KeyTime="0:0:0.467" KeySpline="0.1,0.9 0.2,1.0" Value="1" />
                </DoubleAnimationUsingKeyFrames>
              </Storyboard>
              <Storyboard x:Key="OverlayClosingAnimation">
                <DoubleAnimationUsingKeyFrames Storyboard.TargetProperty="Opacity">
                  <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="1" />
                  <SplineDoubleKeyFrame KeyTime="0:0:0.167" KeySpline="0.2,0 0,1" Value="0" />
                </DoubleAnimationUsingKeyFrames>
              </Storyboard>
  ```
  （`A′:8907–8918`，即 `CommandBar` 的 `Grid.Resources`）

> Kanesumi 用的 0.383 / 0.216 对应的是 **ComboBox**，正确。

### 6.2 MenuFlyoutPresenter（A:31148–31186）—— **零动画**

```xml
    <Style TargetType="MenuFlyoutPresenter" x:Key="DefaultMenuFlyoutPresenterStyle">
        …
        <Setter Property="Template">
            <Setter.Value>
                <ControlTemplate TargetType="MenuFlyoutPresenter">
                    <Grid Background="{TemplateBinding Background}" CornerRadius="{TemplateBinding CornerRadius}">
                        <ScrollViewer x:Name="MenuFlyoutPresenterScrollViewer"
                            Margin="{TemplateBinding Padding}"
                            …
                            <ItemsPresenter Margin="{ThemeResource MenuFlyoutScrollerMargin}" />
                        </ScrollViewer>
                        <Border x:Name="MenuFlyoutPresenterBorder" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="{TemplateBinding CornerRadius}" />

                    </Grid>

                </ControlTemplate>
            </Setter.Value>
        </Setter>
    </Style>
```
（`A:31148–31186`，紧接 `A:31187` 的 `<!-- End Windows.UI.Xaml.Controls.dll resources - DO NOT MANUALLY EDIT ABOVE THIS LINE! -->`）

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| MenuFlyoutPresenter | 打开 / 关闭 | **未在快照中 / 模板内根本无 Storyboard** | — | `A:31148–31186` |
| MenuFlyoutPresenter（LanguageSwitcher 变体） | 打开 / 关闭 | 同上（`BasedOn` 同一模板） | — | `A:8578`（`x:Key="LanguageSwitcherMenuFlyoutPresenterStyle"`）、`A:25051` |
| MenuFlyoutItem | Normal/PointerOver/Pressed/Disabled/Selected | **0 ms**（Discrete）+ `PointerUp/DownThemeAnimation`（无参） | 无 | `A:23793–24255` 一带 |
| MenuBarItem | 同上 | **0 ms** + `PointerUp/DownThemeAnimation`（无参） | 无 | `A:25251 / 25267 / 25283` |
| FlyoutPresenter | 打开 / 关闭（presenter 本体） | 模板内**无 Storyboard**（`Border` + `ScrollViewer`），**打开/关闭动画在 `FlyoutBase` native 侧，未在快照中** | — | `A:13659–13705`；变体 `A:8139–8148`、`A:16778–16784`、`A:17615`、`A:19955` |
| **CommandBarFlyoutCommandBar（真正的 Flyout 开合动画）** | `OpeningStoryboard`（`OuterContentRootClipTransform.X` / `OuterOverflowContentRootClipTransform.X`） | **0.300 s**（写死 `KeyTime="0:0:0.300"`） | **`KeySpline="0.1,0.9 0.2,1"`** | `A:22093–22102` |
| 同上 | `ClosingStoryboard` | **0.150 s**（写死）；`0.151 s` 处 snap 到 `CloseAnimationEndPosition` | **`KeySpline="0.7,0 1,0.5"`** | `A:22107–22118` |
| 同上（`ExpansionStates` 宽度扩张 / 上翻 / 下翻） | `MoreButtonTransform.X` / `ContentRootClipTransform.X` / `OverflowContentRootClipTransform.X` | **0.300 s** | `KeySpline="0.1,0.9 0.2,1"` | `A:22172–22184`、`A:22226–22238` |
| 同上（收起方向） | 同上一族 | **0.150 s** | `KeySpline="0.7,0 1,0.5"` | `A:22198–22213`、`A:22249–22264` |

**原文（`A:22093–22102`，Flyout 打开 300 ms）**：

```xml
                            <Storyboard x:Name="OpeningStoryboard" FillBehavior="Stop">
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetName="OuterContentRootClipTransform" Storyboard.TargetProperty="X">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationStartPosition}" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.300" KeySpline="0.1,0.9 0.2,1" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationEndPosition}" />
                                </DoubleAnimationUsingKeyFrames>
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetName="OuterOverflowContentRootClipTransform" Storyboard.TargetProperty="X">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationStartPosition}" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.300" KeySpline="0.1,0.9 0.2,1" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationEndPosition}" />
                                </DoubleAnimationUsingKeyFrames>
                            </Storyboard>
```

**原文（`A:22107–22118`，Flyout 关闭 150 ms）**：

```xml
                            <Storyboard x:Name="ClosingStoryboard">
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetName="OuterContentRootClipTransform" Storyboard.TargetProperty="X">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationEndPosition}" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.150" KeySpline="0.7,0 1,0.5" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationStartPosition}" />
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0.151" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.CloseAnimationEndPosition}" />
                                </DoubleAnimationUsingKeyFrames>
                                <DoubleAnimationUsingKeyFrames Storyboard.TargetName="OuterOverflowContentRootClipTransform" Storyboard.TargetProperty="X">
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationEndPosition}" />
                                    <SplineDoubleKeyFrame KeyTime="0:0:0.150" KeySpline="0.7,0 1,0.5" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.OpenAnimationStartPosition}" />
                                    <DiscreteDoubleKeyFrame KeyTime="0:0:0.151" Value="{Binding RelativeSource={RelativeSource TemplatedParent}, Path=FlyoutTemplateSettings.CloseAnimationEndPosition}" />
                                </DoubleAnimationUsingKeyFrames>
                            </Storyboard>
```

> **这是快照里唯一一组「Flyout 打开/关闭」的显式时长**（A:22090–22092 的注释自证：*"we aren't able to suppress the default flyout open/close animations"* —— 说明它是在**叠加** native 默认动画）。
> **归属需谨慎**：这套 Storyboard 属于 `CommandBarFlyoutCommandBar` 模板（`A:22084`），目标是 `FlyoutTemplateSettings.OpenAnimationStartPosition/EndPosition` —— 即**它表达的是 Flyout 的位移参数化时长，可视为 UWP Flyout 的开合权威**：**开 300 ms / 关 150 ms**，曲线 `0.1,0.9 0.2,1` / `0.7,0 1,0.5`。
> **可直接给 M6 用**：本库 `sheet_appear = 0.30` 恰与 **300 ms** 一致 ✅；`sheet_dismiss = 0.26` 对应权威 **150 ms**（偏慢 110 ms，建议改为 0.15 + `KeySpline 0.7,0 1,0.5`）。

### 6.3 ToolTip（A:13245–13291）—— **只有 FadeIn/FadeOut 主题动画，无时长**

```xml
                        <VisualStateManager.VisualStateGroups>
                            <VisualStateGroup x:Name="OpenStates">
                                <VisualState x:Name="Closed">

                                    <Storyboard>
                                        <FadeOutThemeAnimation TargetName="LayoutRoot" />
                                    </Storyboard>
                                </VisualState>
                                <VisualState x:Name="Opened">

                                    <Storyboard>
                                        <FadeInThemeAnimation TargetName="LayoutRoot" />
                                    </Storyboard>
                                </VisualState>

                            </VisualStateGroup>

                        </VisualStateManager.VisualStateGroups>
```
（`A:13268–13285`）

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| ToolTip | `Opened` | **未在快照中**（`FadeInThemeAnimation` 无 Duration） | 未在快照 | `A:13276–13281` |
| ToolTip | `Closed` | **未在快照中**（`FadeOutThemeAnimation`） | 未在快照 | `A:13270–13275` |

### 6.4 TeachingTip —— **不在 UWP OS 模板里**

| 控件 | 结论 |
|---|---|
| `TeachingTip` | **A / A′ 中 0 命中**（UWP OS 侧没有该控件）。 |
| `TeachingTip`（WinUI 2.8.6） | 在 B 中存在：`B:2165–2166` `<Style TargetType="controls:TeachingTip" BasedOn="{StaticResource DefaultTeachingTipStyle}" />` / `<Style x:Key="DefaultTeachingTipStyle" TargetType="controls:TeachingTip">`。其模板同样以 `AnimatedVisualPlayer`（Lottie）承担动画，**XAML 内无时长**。→ **时长/缓动 = 未在快照中**。 |

---

## 7 · SwipeControl

**两源的 SwipeControl 模板都只有静态 Grid，零动画资源；展开/吸合/执行动画全在 native 侧。**

UWP OS（A:21550–21574）：

```xml
    <Style TargetType="SwipeControl">
        <Setter Property="IsTabStop" Value="False" />
        <Setter Property="Background" Value="Transparent" />
        <Setter Property="MinHeight" Value="{ThemeResource ListViewItemMinHeight}" />
        <Setter Property="MinWidth" Value="{ThemeResource ListViewItemMinWidth}" />
        <Setter Property="Template">
            <Setter.Value>
                <ControlTemplate TargetType="SwipeControl">
                    <Grid x:Name="RootGrid">
                        <Grid x:Name="SwipeContentRoot">
                            <StackPanel x:Name="SwipeContentStackPanel" />
                            
                        </Grid>
                        <Grid x:Name="ContentRoot">
                            <ContentPresenter x:Name="ContentPresenter" Background="{TemplateBinding Background}" BorderBrush="{TemplateBinding BorderBrush}" BorderThickness="{TemplateBinding BorderThickness}" CornerRadius="{TemplateBinding CornerRadius}" Padding="{TemplateBinding Padding}" Content="{TemplateBinding Content}" ContentTransitions="{TemplateBinding ContentTransitions}" ContentTemplate="{TemplateBinding ContentTemplate}" HorizontalContentAlignment="Stretch" VerticalContentAlignment="Stretch" AutomationProperties.AccessibilityView="Raw" />
                            <Grid x:Name="InputEater" />
                            
                        </Grid>

                    </Grid>

                </ControlTemplate>
            </Setter.Value>
        </Setter>
    </Style>
```

WinUI 2.8.6（B:1721–1742）—— 结构完全相同，只是 `controls:` 前缀、无 `CornerRadius`：

```xml
  <Style x:Key="DefaultSwipeControlStyle" TargetType="controls:SwipeControl">
    …
        <ControlTemplate TargetType="controls:SwipeControl">
          <Grid x:Name="RootGrid">
            <Grid x:Name="SwipeContentRoot">
              <StackPanel x:Name="SwipeContentStackPanel" />
            </Grid>
            <Grid x:Name="ContentRoot">
              <ContentPresenter x:Name="ContentPresenter" … />
              <Grid x:Name="InputEater" />
            </Grid>
          </Grid>
        </ControlTemplate>
  </Style>
  <Style TargetType="controls:SwipeControl" BasedOn="{StaticResource DefaultSwipeControlStyle}" />
```

| 控件 | 状态/动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| SwipeControl | 展开（reveal） | **未在快照中**（模板无 Storyboard，无时长资源） | 未在快照 | `A:21550–21574` / `B:1721–1742` |
| SwipeControl | 吸合（dismiss / 回弹） | **未在快照中** | 未在快照 | 同上 |
| SwipeControl | 执行（invoke 后的收束） | **未在快照中** | 未在快照 | 同上 |
| SwipeControl | `Open`/`Closed`/`Reveal` 视觉态 | **模板中不存在任何 VisualState**（连状态组都没有） | — | 同上 |

> **两源一致：SwipeControl 在 XAML 层是纯粹的空壳。** 时长需从 `Microsoft.UI.Xaml.dll` / `Windows.UI.Xaml.dll` 的 native 实现取，本机快照无法提供。

---

## 8 · 导航类主题过渡（NavigationThemeTransition / EntranceThemeTransition / AddDeleteThemeTransition）

### 8.1 `EntranceThemeTransition` / `AddDeleteThemeTransition` / `ContentThemeTransition` / `ReorderThemeTransition` 的**默认时长与缓动 = 未在快照中**

A 中这些过渡**只以「无参标签」出现，从不带 Duration/缓动属性**。全部 25 处 `TransitionCollection` / 直接标签命中如下：

| 控件 | 文件:行号 | 照抄整段 |
|---|---|---|
| `ItemsControl`（ListView/GridView 族默认） | `A:10497–10502` | 见下 |
| `ItemsControl`（第二处） | `A:10563–10568` | 同构 |
| `ContentDialog`（`ContentTransitions`） | `A:13622–13625` | `<AddDeleteThemeTransition />` / `<ContentThemeTransition />` / `<ReorderThemeTransition />` / `<EntranceThemeTransition IsStaggeringEnabled="False" />` |
| `ContentPresenter` 内容过渡 | `A:19483` | `<EntranceThemeTransition />`（**唯一一处不显式关 stagger 的默认用法**） |
| `ScrollViewer` 族 | `A:21118–21122` | `<ContentThemeTransition />` / `<ReorderThemeTransition />` / `<EntranceThemeTransition IsStaggeringEnabled="False" />` |

`A:10497–10502` 原文：

```xml
                <TransitionCollection>
                    <AddDeleteThemeTransition />
                    <ContentThemeTransition />
                    <ReorderThemeTransition />
                    <EntranceThemeTransition IsStaggeringEnabled="False" />
                </TransitionCollection>
```

| 过渡 | 快照里能取到的信息 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| `EntranceThemeTransition` | `IsStaggeringEnabled="False"`（显式关掉 stagger）；`FromHorizontalOffset` / `FromVerticalOffset` **从未写入 → 取类默认值** | **未在快照中** | 未在快照 | `A:10501 / 10567 / 13625 / 21121`；默认用法 `A:19483` |
| `AddDeleteThemeTransition` | 仅无参标签 | **未在快照中** | 未在快照 | `A:10498 / 10564 / 13622` |
| `ContentThemeTransition` | 仅无参标签 | **未在快照中** | 未在快照 | `A:10499 / 10565 / 13623 / 21119` |
| `ReorderThemeTransition` | 仅无参标签 | **未在快照中** | 未在快照 | `A:10500 / 10566 / 13624 / 21120` |
| `NavigationThemeTransition` | **A / A′ / B 三文件 0 命中** | **未找到** | **未找到** | — |
| `PaneThemeTransition` / `EdgeUIThemeTransition` / `DrillInThemeTransition` / `PopupThemeTransition` / `RepositionThemeTransition`（作为 Transition，非 Animation） | **A / A′ / B 三文件 0 命中** | **未找到** | — | — |
| `SwipeHintThemeAnimation` | **A / A′ / B 三文件 0 命中** | — | — | — |

> **M6-2 stagger 入场的直接后果**：UWP 快照**没有给出** Entrance 的时长、缓动、stagger 步长。它只在 `ContentPresenter`（`A:19483`）与 `TransitionCollection` 里以默认参数出现。要「对齐 UWP stagger 入场」，必须另找权威（native XBF / 官方文档 / 实测），或明确登记为 Kanesumi 自定值。

### 8.2 **能取到的替代权威**：A 里与「入场/页面切换」最接近的**真实写入时长**（CalendarView DisplayMode 过渡）

这是整个 A 文件中**唯一一组**「页面/视图级切换 + stagger 式缩放 + 遮罩」的完整关键帧，可直接借给 M6-2：

```xml
                  <VisualTransition From="Month" To="Year">
                    <Storyboard>
                      <ObjectAnimationUsingKeyFrames Storyboard.TargetName="YearViewScrollViewer" Storyboard.TargetProperty="Visibility">
                        <DiscreteObjectKeyFrame KeyTime="0" Value="Visible" />
                      </ObjectAnimationUsingKeyFrames>
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="MonthView" Storyboard.TargetProperty="Opacity">
                        <SplineDoubleKeyFrame KeyTime="0:0:0.233" Value="0" KeySpline="0.1,0.9,0.2,1" />
                      </DoubleAnimationUsingKeyFrames>
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="YearViewScrollViewer" Storyboard.TargetProperty="Opacity">
                        <DiscreteDoubleKeyFrame KeyTime="0" Value="0" />
                        <DiscreteDoubleKeyFrame KeyTime="0:0:0.233" Value="0" />
                        <SplineDoubleKeyFrame KeyTime="0:0:0.733" Value="1" KeySpline="0.1,0.9,0.2,1" />
                      </DoubleAnimationUsingKeyFrames>
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="MonthViewTransform" Storyboard.TargetProperty="ScaleX">
                        <SplineDoubleKeyFrame KeyTime="0:0:0.233" Value="0.84" KeySpline="0.1,0.9,0.2,1" />
                      </DoubleAnimationUsingKeyFrames>
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="MonthViewTransform" Storyboard.TargetProperty="ScaleY">
                        <SplineDoubleKeyFrame KeyTime="0:0:0.233" Value="0.84" KeySpline="0.1,0.9,0.2,1" />
                      </DoubleAnimationUsingKeyFrames>
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="YearViewTransform" Storyboard.TargetProperty="ScaleX">
                        <DiscreteDoubleKeyFrame KeyTime="0:0:0.233" Value="1.29" />
                        <SplineDoubleKeyFrame KeyTime="0:0:0.733" Value="1" KeySpline="0.1,0.9,0.2,1" />
                      </DoubleAnimationUsingKeyFrames>
                      …
                      <DoubleAnimationUsingKeyFrames Storyboard.TargetName="BackgroundLayer" Storyboard.TargetProperty="Opacity">
                        <LinearDoubleKeyFrame KeyTime="0:0:0.000" Value="0" />
                        <LinearDoubleKeyFrame KeyTime="0:0:0.250" Value="0" />
                        <SplineDoubleKeyFrame KeyTime="0:0:0.733" Value="1" KeySpline="0.15,0.64,0.25,1" />
                      </DoubleAnimationUsingKeyFrames>
                    </Storyboard>
                  </VisualTransition>
```
（`A′:7797–7830`）

| 控件 | 动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| CalendarView | Month→Year 离场（Opacity、ScaleX/Y→0.84） | **0.233 s** | `KeySpline="0.1,0.9,0.2,1"` | `A′:7802–7814` |
| CalendarView | Month→Year 入场（Opacity 0→1、Scale 1.29→1） | **0.233 s 延迟 + 0.733 s 总时长**（0.233→0.733 s） | `KeySpline="0.1,0.9,0.2,1"` | `A′:7805–7822` |
| CalendarView | Month→Year 遮罩（BackgroundLayer.Opacity） | `0 → 0.250 s` 保持 0，`0.250 → 0.733 s` 升到 1 | `KeySpline="0.15,0.64,0.25,1"` | `A′:7824–7828` |
| CalendarView | Year→Month / Year→Decade / Decade→Year / … 全族 | 同构（0.233 / 0.733 / 0.250 / 1.29 / 0.84） | 同上 | `A′:7831–7963`（共 6 组） |
| CalendarView | ViewChanging 时 HeaderButton 淡入 | **0.167 s** | 无（默认） | `A′:7764` |
| CalendarView | 多选勾选框 X 位移（`TranslateX` −32↔0） | **0.333 s** | `KeySpline="0.1,0.9,0.2,1"` | `A′:11233–11261` |

### 8.3 NavigationView / CommandBar 面板展开（A / A′）

| 控件 | 动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| CommandBar | `CompactClosed`→`CompactOpenUp`（展开） | `GeneratedDuration="0:0:0.467"` | 内部关键帧 `0.1,0.9 0.2,1.0` | `A:11439`、`A′:8933` |
| CommandBar | `CompactOpenUp`→`CompactClosed`（收起） | `GeneratedDuration="0:0:0.167"` | `0.2,0 0,1` | `A:11454` |
| CommandBar | `CompactClosed`→`CompactOpenDown` | `0:0:0.467` | `0.1,0.9 0.2,1.0` | `A:11469` |
| CommandBar | `CompactOpenDown`→`CompactClosed` | `0:0:0.167` | `0.2,0 0,1` | `A:11484` |
| CommandBar | `MinimalClosed`→`MinimalOpenUp/Down` | `0:0:0.467` | `0.1,0.9 0.2,1.0` | `A:11499 / 11549` |
| CommandBar | `MinimalOpenUp/Down`→`MinimalClosed` | `0:0:0.167` | `0.2,0 0,1` | `A:11524 / 11571` |
| CommandBar | `HiddenClosed`→`HiddenOpenUp/Down` | `0:0:0.467` | `0.1,0.9 0.2,1.0` | `A:11593 / 11623` |
| CommandBar | `HiddenOpenUp/Down`→`HiddenClosed` | `0:0:0.167` | `0.2,0 0,1` | `A:11608 / 11638` |
| CommandBar | 同一族在 AppBar/CommandBar 第二处 | 同上 | 同上 | `A:22890–23285`、`A:30276–30710` |
| NavigationView / SplitView 阴影 | 展开 | `0:0:0.35` | `0.1,0.9 0.2,1.0` | `B:162`（`ShadowCasterTransform.TranslateX`）；收起 `B:176` 为 `0:0:0.12` 同曲线 |
| NavigationView 项 Header 淡入/淡出 | Expanded 状态 | `0:0:0.2` / `0:0:0.1` | `0.0,0.35 0.15,1.0` | `B:587 / 601` |
| NavigationView 展开 | SplitView 面板位移 | `0:0:0.35` | `0.1,0.9 0.2,1.0` | `A:16904–16915 / 16936–16947 / 16971–16978 / 17008–17015` |
| NavigationView 收起 | SplitView 面板位移 | `0:0:0.12` | `0.1,0.9 0.2,1.0` | `A:17029–17069 / 17093–17137` |
| NavigationView / TreeView 等 | 面板内元素 | — | `KeySpline="0.0,0.35 0.15,1.0"` | `A:17161–17289`、`A:20216–20237` |

关键帧原文（`A:16904–16915`）：

```xml
                                                <SplineDoubleKeyFrame KeyTime="0:0:0.35" KeySpline="0.1,0.9 0.2,1.0" Value="0" />
                                                <SplineDoubleKeyFrame KeyTime="0:0:0.35" KeySpline="0.1,0.9 0.2,1.0" Value="0" />
                                                <SplineDoubleKeyFrame KeyTime="0:0:0.35" KeySpline="0.1,0.9 0.2,1.0" Value="1.0" />
```

### 8.4 其他「有具体时长」的 A 内条目（备查）

| 控件 | 动画名 | 时长 | 缓动 | 文件:行号 |
|---|---|---|---|---|
| 列表项重排提示 | `To="NoReorderHint"` | `0:0:0.2` | — | `A:10957 / 28343`；`B:2948` |
| 列表项拖拽/重排 | `NotDragging` | `0:0:0.2` | — | `A:11086 / 28472`；`B:2985` |
| ListViewItem | Reordering / ReorderingTarget 不透明度与 Scale | `0:0:0.240` | — | `A:11018 / 11028 / 11032 / 11036 / 11064`；`B:2965 / 2970` |
| GridView/ListView 多选勾选框 | `TranslateX` ±32 | `0:0:0.333` | `0.1,0.9,0.2,1` | `A:28248–28278`；`A′:11233–11261` |
| FlipView / Pivot 之类 | 垂直位移 `50 → 0.5` / `0.5 → 50` | `0:0:0.3` / `0:0:0.7` | 无 | `A:15695 / 15708` |
| `A:16317` | 单条 | `0:0:0.167` | — | `A:16317` |
| `A:17315 / 17342` | — | `0:0:0` | — | — |
| TreeViewItem 多选勾选框出现 | — | `0.333 s` | `0.1,0.9,0.2,1` | `A′:11233–11261` |
| ListViewItem 多选态切换 | `ListMultiSelect` ↔ `NoMultiSelect` | **0.15 s** | — | `A:28682–28683` |
| **MediaTransportControls** 控制面板淡入（Opacity + `TranslateVertical.Y` 50→0.5） | `ControlPanelFadeIn` | **0.3 s**（写死 `Duration="0:0:0.3"` / `KeyTime="0:0:0.3"`） | 无（`EasingDoubleKeyFrame` 默认） | `A:15690–15696` |
| **MediaTransportControls** 控制面板淡出 | `ControlPanelFadeOut` | **0.7 s**（写死 `0:0:0.7`） | 无 | `A:15698–15709` |
| **MediaTransportControls** 内嵌 CommandBar 展开/收起 | `DisplayModeStates` | **0.300 s** 展开 / **0.150 s** 收起（`GeneratedDuration`；Storyboard 为空——注释说明只为让 CommandBar 等待关闭动画） | 面板内元素用 `0.1,0.9 0.2,1` | `A:22139–22151`（空 Storyboard）、`A:22161–22264`（ExpansionStates 实动画，`0.1,0.9 0.2,1`） |

---

## 9 · 外部佐证（非快照，标注来源与置信度）

> 以下**不是**两台一手 XAML 源的内容，仅用于解释「XAML 里为什么取不到」以及提供可引用的公开值。每条标注置信度。

| 条目 | 值 | 来源 | 置信度 |
|---|---|---|---|
| `ControlNormalAnimationDuration` / `ControlFastAnimationDuration` / `ControlFasterAnimationDuration` | 250 / 167 / 83 ms | [Timing and easing · Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing) | **官方文档（确证）** |
| WinUI 2.6 的四个键原文 | `ControlNormalAnimationDuration 00:00:00.250` / `ControlFastAnimationDuration 00:00:00.167` / `ControlFastAnimationAfterDuration 00:00:00.168` / `ControlFasterAnimationDuration 00:00:00.083`（均为 `<x:String>`，被吃进 `GeneratedDuration`） | [unoplatform/uno#7168](https://github.com/unoplatform/uno/issues/7168) | **文档引用 + 本机 PRI 值流交叉验证（确证）** |
| `ControlSlowAnimationDuration` | 不存在 | 官方表只有三档 + `Windows.UI.Xaml.dll` 0 命中 + 三个 XAML 源 0 命中 | **确证不存在** |
| `PointerDownThemeAnimation` 的 Duration | 「写 `Duration` 属性无效，时长已预置」（**未给出数值**） | [PointerDownThemeAnimation · Microsoft Learn](https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.media.animation.pointerdownthemeanimation?view=winrt-26100) | **官方文档（确证「取不到」）**；数值 **未找到** |
| `PointerDownThemeAnimation` 的桌面语义 | 「On Windows, the animation slightly shrinks the item to indicate that it is pressed; on Windows Phone, the animation tilts the item slightly around the positive y-axis in a 2.5D effect.」+「slightly shrinks and tilts」；API 页另注「**overrides the current values of `Projection` and `RenderTransform`**」→ **桌面是「缩小」，写的是 Projection/RenderTransform，不是 TranslateTransform** | [Animating pointer actions (XAML) · Microsoft Learn](https://learn.microsoft.com/en-us/previous-versions/windows/apps/jj649432(v=win.10))；[motion-pointer](https://learn.microsoft.com/en-us/previous-versions/windows/uwp/ui-input/motion-pointer) | **官方文档（确证语义）**；像素/缩放系数 **未找到** |
| `PointerUpThemeAnimation` "restores the item to its original state" | 语义确证 | 同上 | **官方文档** |
| `EntranceThemeTransition` 的 Duration / 缓动 / stagger 步长 / 三个属性的默认值 | 官方 API 页**完全不提**时长与默认值，只说明 `FromHorizontalOffset` / `FromVerticalOffset`（"The vertical offset translation, in pixels"，**无默认值**）/ `IsStaggeringEnabled` 三个属性；行为描述仅「swiftly slides into view」 | [EntranceThemeTransition · Microsoft Learn](https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.media.animation.entrancethemetransition?view=winrt-26100)；[FromVerticalOffset](https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.media.animation.entrancethemetransition.fromverticaloffset)；[Quickstart: library animations](https://learn.microsoft.com/en-us/previous-versions/windows/apps/hh452703(v=win.10)) | **官方文档（确证"文档也没写"）**；数值 **未找到** |
| Content 过渡指引 | 多容器同时更新时**不加 stagger / 不加延迟**；新内容自底部滑入 | [Content transition animations](https://learn.microsoft.com/en-us/windows/apps/design/motion/content-transition-animations) | **官方文档** |
| Fluent 缓动基线 | 入场 `cubic-bezier(0, 0, 0, 1)`（Fast Out, Slow In）；退场 `cubic-bezier(1, 0, 1, 1)`（Slow Out, Fast In） | [Timing and easing · Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing) | **官方文档**；注意这是 **Fluent / WinUI 3**，**不是 UWP Metro**，本库铁律 1 视 Fluent 为反面教材 |

> **PointerDownThemeAnimation 的「100 ms + Y 下沉 N px」在本次取数中未能从任何一手源确证。**
> - `A / A′ / B` 三文件：参数一律缺省，`Duration`/`SpeedRatio` 0 命中。
> - 官方 API 文档：明确说 Duration 预置但**不给数值**，且桌面语义是**缩小**而非下沉。
> - Kanesumi `docs/CONTROL_SPEC.md:14` 写「Y 向微下沉/复位，~100ms，参数 OS 预置」、`:374` 写「仅时长待实测」——**该 ~100 ms 与「Y 下沉」的说法在本机快照中无一手支撑**。
> 若 M6-1 要落这个效果，建议：① 先按 `CONTROL_SPEC` 的 100 ms 落地并**在 `CANON_VS_TEMPORARY.md` 登记为「实测待定」**；或 ② 用 `A` 中唯一可抄的同类「按压缩放」证据——`A:12452` 一族的 `0.13,0.21,0.1,0.7` / `0.02,0.33,0.38,0.77` 曲线族——做近似。

---

## 10 · 与本库当前值的差异

本库现用值取自 `kanesumi-anim/src/presets.rs`（`MetroPresets`）与 `kanesumi-controls/src/progress.rs`、`docs/CONTROL_SPEC.md`。

| # | 条目 | 本库现用 | 本次取到的权威值 | 差异判定 | 证据 |
|---|---|---|---|---|---|
| 1 | **`ControlFastAnimationDuration`** | `DURATION_QUICK_SWITCH = 0.167`（`presets.rs:11`），`quick_switch()` = 167 ms Quadratic/EaseOut（`presets.rs:92–98`） | **167 ms**（WinUI 3 官方 `ControlFastAnimationDuration`）；**UWP 快照中该常量不存在** | **数值正确，但归属要改**：167 ms 是 WinUI 3 / Windows App SDK 的平台常量，**不是 UWP OS 的 XAML 资源**。UWP 侧同一位置的证据是 `ObjectAnimationUsingKeyFrames` + `KeyTime="0"` ⇒ **UWP Button 的 hover/press 色变其实是 0 ms 瞬切**，167 ms 并不来自 Button 状态切换。**待对齐**：注释里的「对齐 UWP ControlFastAnimationDuration 0.167s」应改为「对齐 WinUI 3 `ControlFastAnimationDuration`」；或改为对齐 UWP 实测的 0 ms 瞬切。 | `presets.rs:10–11`；[Learn](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing) |
| 2 | **按钮无下沉动画（M6-1 待加）** | 无 `PointerDown` 下沉（`button.rs` 只有背景/边框/padding） | UWP 模板里**确实调用** `PointerDownThemeAnimation`（`A:6306`）与 `PointerUpThemeAnimation`（`A:6274`），**但参数全缺省**；像素与时长 **未找到**。官方文档三处确证语义：①「on Windows, the animation **slightly shrinks** the item; on Windows Phone, the animation **tilts** the item slightly around the positive y-axis in a 2.5D effect」；②「slightly shrinks **and tilts**」；③ API 页：「PointerDownThemeAnimation **overrides the current values of `Projection` and `RenderTransform`**」——即它写的是 **Projection（倾斜）+ RenderTransform（缩小）**，**不是 TranslateTransform（位移）** | **M6-1 的「Y 下沉」方向与官方文档不符**：桌面 UWP 是 **shrink + tilt（写 Projection/RenderTransform）**，不是 Y 位移。旁证：A 全文件中 `ScaleX/ScaleY` 字面量只有 `1.0`（×2）与 `SmallScrollThumbScale` 资源，`TranslateX/TranslateY` 字面量只有 `0`（×2）与 `SmallScrollThumbOffset` ——**没有任何 Pressed 态用的字面量 Scale/Translate**，按压反馈**只**由这两个主题动画承担（各 38 / 78 处，**全部只有 `TargetName`**）。若坚持 Y 下沉，须标注为 **Kanesumi 自定**（或 Phone 分支）；像素与时长仍**无法从一手源取到**。「1.5px / 2px」**在任何 Microsoft 源中都不存在**，不得写成 Windows 对齐，建议登记 `CANON_VS_TEMPORARY.md`。 | `A:6306`、`A:6274`；[Animating pointer actions](https://learn.microsoft.com/en-us/previous-versions/windows/apps/jj649432(v=win.10))；[motion-pointer](https://learn.microsoft.com/en-us/previous-versions/windows/uwp/ui-input/motion-pointer)；`docs/CONTROL_SPEC.md:14, 374` |
| 3 | **ProgressBar 不确定循环** | `DURATION_INDETERMINATE = 2.0`（`presets.rs:19`），`progress_indeterminate()` = 2.0 s Cubic/EaseInOut | **两源不同**：WinUI 2.8.6（B）= **2.0 s** + `KeySpline="0.4, 0.0, 0.6, 1.0"`（`B:3231–3241`，与现用值**完全吻合**）；UWP OS 26100（A）= **3.917 s** 整体位移 + 圆点 `0.4,0,0.6,1`（`A:12117–12156`） | **现用值对 WinUI 2 正确**；若目标视觉是 UWP OS 26100，则应为 **3.917 s**。缓动：B 的 `0.4,0,0.6,1` 与现用 Cubic/EaseInOut 接近但**不是同一个函数**，建议直接实现为 KeySpline。 | `presets.rs:19, 119–126`；`B:3231–3241`；`A:12117` |
| 4 | **ProgressBar Paused / Error 过渡** | 0.25 s（`progress.rs:50–51`，`UwpEasing::Cubic, EasingMode::EaseOut`；注释称 "V17"） | **两源不同**：UWP OS（A）= **0.25 s**（Paused 的 Opacity/恢复；Error 本身 0 s 瞬切）；WinUI 2.8.6（B）= **0.167 s** 颜色（`B:3217/3222`，`Duration="0:0:0.167"`） | **现用值对 UWP OS 正确（0.25 s）**；对 WinUI 2.8.6 应为 **0.167 s**。缓动：两源都是 **线性**（`ColorAnimation` / `DoubleAnimation` 无缓动），现用 Cubic/EaseOut 是近似（`progress.rs:49` 自注"UWP 无 Linear，Cubic/EaseOut 观感接近"）——**该近似可保留但应登记**。 | `progress.rs:48–51, 74–81`；`A:12236–12247`；`B:3217–3222` |
| 5 | **ProgressRing** | 2.0 s / 900°（`progress.rs:186, 243–248`：`rotation = 900.0 * t`；`DURATION_INDETERMINATE = 2.0`） | **UWP OS XAML（A）= 3.47 s 周期**，`E1R` 角度 `-110° → 585°`（净 +695°，**不是 900°**），关键帧 `0 / 0.433 / 1.2 / 1.617 / 2.017 / 2.783 / 3.217 s`，6 点 stagger **0.167 s**，曲线为**非对称** KeySpline 串（`0.13,0.21,0.1,0.7` / `0.02,0.33,0.38,0.77` / `0.57,0.17,0.95,0.75` / `0,0.19,0.07,0.72` / `0,0,0.95,0.37`）。**WinUI 2.8.6 = Lottie，XAML 无值** | **与 UWP OS 严重不符**：周期 2.0 vs **3.47 s**、角度 900° vs **695°**。现用的 2.0 s/900° 来自 `CONTROL_SPEC.md:160–161` 的 **WinUI 2 C++ 实现**（`c_durationTicks = 20000000`）快照，该文件与 UWP OS XAML **是两个不同模型**。**必须二选一并写清依据**。另外：本库「TrimStart/TrimEnd 呼吸」的近似在 UWP OS 中无对应（UWP 是 6 个 `Ellipse` + `RotateTransform`，没有 Stroke dash）。 | `progress.rs:186, 243–248`；`docs/CONTROL_SPEC.md:160–161`；`A:12407–12506`；`B:3313–3347` |
| 6 | **Overlay 淡入/淡出** | `DURATION_OVERLAY_OPEN = 0.383` / `DURATION_OVERLAY_CLOSE = 0.216`（`presets.rs:21, 23`；Cubic/EaseOut） | ComboBox 遮罩：**0.383 s / 0.216 s**，KeySpline 均为 `0.1,0.9 0.2,1.0`（`A:10166–10177`） | **数值完全正确 ✅**（唯一一条逐字命中）。缓动建议改为 KeySpline `0.1,0.9 0.2,1.0`。**注意**：同一对资源名在 CommandBar/NavigationView 里是 **0.467 / 0.167**（`A′:8907–8918`、`A:11408–11419`），**不可混用**。 | `presets.rs:20–23`；`A:10166–10177` |
| 7 | **Dialog 缩放 / 淡入淡出** | `DURATION_DIALOG_ENTER = 0.5`（spline `0.1,0.9,0.2,1`）、`FADE_IN = 0.167`、`FADE_OUT = 0.083`（`presets.rs:24–29`） | `ContentDialog` 样式在 `A:9616–9900` 一带：**模板只有 `TransitionCollection`（`A:13622–13625`，无时长）+ 静态布局，没有任何 `Duration` 关键帧**；`A` 全文件 `0:0:0.5` / `0:0:0.083` = **0 命中**；`B` 全文件 `0:0:0.5` 仅 2 处（`B:3188 / 3202`，ProgressBar Reposition 收束，非 Dialog）。**0.5 / 0.167 / 0.083 在 A / A′ / B 中均未找到** | **未找到一手支撑**（与 `CONTROL_SPEC.md:286` 的「0.167 线性 / 0.083 线性」同源，那份值来自已丢弃的 reference 快照）。建议登记为 **WinUI 2 来源**。 | `presets.rs:24–29`；`docs/CONTROL_SPEC.md:286`；`A:9616`、`A:13622–13625` |
| 8 | **ToggleSwitch 滑动** | `DURATION_TOGGLE_FLIP = 0.15`（Cubic/EaseOut，`presets.rs:17, 116`）；`CONTROL_SPEC.md:127` 称 150 ms + `RepositionThemeAnimation` 行程 20 px | 快照中 ToggleSwitch 的 knob 位移是 **`RepositionThemeAnimation`（无参，时长未在快照）** + `KnobTranslateTransform.X` **写死 `To="24"`**（`A:13069–13072`）；三条过渡 `GeneratedDuration="0"`（`A:13005/13027/13036/13045`） | **时长 150 ms 未找到一手支撑**（`RepositionThemeAnimation` 预置值取不到）。**行程应为 24 px（模板坐标）而非 20 px** —— 除非 24 是含 padding 的模板坐标而 20 是逻辑值，需再核。 | `presets.rs:17, 116`；`docs/CONTROL_SPEC.md:127`；`A:13003–13072` |
| 9 | **ProgressBar Reposition（值变化）** | `CONTROL_SPEC.md:144`、`presets.rs:221` 称 150 ms | `A:12092–12097` / `B:3153` 的 `RepositionThemeAnimation` **无 Duration** | **150 ms 未找到一手支撑**（预置值）。 | `docs/CONTROL_SPEC.md:144`；`assert_eq!(DURATION_TOGGLE_FLIP, 0.15)` `presets.rs:222` |
| 10 | **CheckBox / RadioButton 勾选动画** | `check_box.rs` 有勾选视觉；`presets.rs` 无对应时长 | UWP：**`Duration="0"` 瞬切**（`A:7140–7147` / `A:6924–6940`） | 若本库做了淡入/缩放，是**自加效果**，须登记；UWP 权威行为是瞬切。 | `A:6924–6940`、`A:7119–7149` |
| 11 | **MenuFlyout / Flyout / ToolTip 弹出（CONTROL_SPEC §8「未在快照」）** | `sheet_appear` **0.30 s** / `sheet_dismiss` **0.26 s** 近似 | `A:31148–31186`（MenuFlyoutPresenter）= **模板零 Storyboard**；`A:13659–13705`（FlyoutPresenter 本体）= **零 Storyboard**；`A:13268–13285`（ToolTip）= 仅 `FadeIn/FadeOutThemeAnimation`（无参）；`SplitOpen/CloseThemeAnimation`（`A:10293/10302`）无参；`PopupThemeTransition` = **三文件 0 命中**。**但 `CommandBarFlyoutCommandBar` 的 `OpeningStoryboard`/`ClosingStoryboard` 给了显式值：开 0.300 s（`0.1,0.9 0.2,1`）/ 关 0.150 s（`0.7,0 1,0.5`）**（`A:22093–22118`，目标为 `FlyoutTemplateSettings.OpenAnimation*`） | **「未在快照」基本成立**（presenter 确实无动画），**但 Flyout 开合有替代权威 300 / 150 ms**。→ `sheet_appear = 0.30` **✅ 命中 300 ms**；`sheet_dismiss = 0.26` 应为 **0.15 s + KeySpline `0.7,0 1,0.5`**（现用偏慢 110 ms）。 | `docs/CONTROL_SPEC.md:248, 260`；`presets.rs:7–9`；`A:31148–31186`、`A:13659–13705`、`A:13245–13291`、`A:22093–22118` |
| 12 | **SwipeControl 展开/吸合/执行** | 未见专门时长常量 | **两源都是空壳模板**（`A:21550–21574`、`B:1721–1742`，连 VisualState 都没有） | **未在快照中**（本任务 §7 原判断成立）。 | 同上 |
| 13 | **M6-2 stagger 入场（EntranceThemeTransition）** | 无 | `A:19483` / `A:10501` 等只给**无参标签**；官方 API 页**不给时长**；`NavigationThemeTransition` 三文件 **0 命中** | **UWP 快照无法支撑 M6-2 的 stagger 值**。可用的**唯一同类权威**是 `A′:7797–7830`（CalendarView Month↔Year）：**离场 0.233 s、入场 0.733 s、stagger 延迟 0.233 s、遮罩 0.250→0.733 s**，曲线 `0.1,0.9,0.2,1`（遮罩 `0.15,0.64,0.25,1`），缩放 1.29 / 0.84。**建议 M6-2 以此为 stagger 入场模板**，或明确登记为 Kanesumi 自定。 | `A:19483`、`A′:7797–7830` |
| 14 | **`METRO_STANDARD_DURATION = 0.25`** | 0.25 s（`presets.rs:4`） | UWP OS 中 0.25 s 只出现在 **ProgressBar Paused**（`A:12104 / 12245`）；WinUI 3 的 `ControlNormalAnimationDuration` 也是 **250 ms** | **一致 ✅**（但 UWP OS 里它**不是**通配常量，只是 ProgressBar 的局部写死值）。 | `presets.rs:4`；`A:12104`；[Learn](https://learn.microsoft.com/en-us/windows/apps/design/motion/timing-and-easing) |

---

## 11 · 「未找到」汇总（不要猜，逐条列出）

| 诉求 | 结论 |
|---|---|
| `ControlFastAnimationDuration` / `ControlNormalAnimationDuration` / `ControlSlowAnimationDuration` 在 A / A′ / A2 / B 中的定义 | **未找到**（四份 SDK XAML + B 全部 0 命中）。**并且 `C:\Windows\System32\Windows.UI.Xaml.dll`（UTF-16 全量扫描）0 命中 → UWP 运行时本身不定义这些键**。只有 WinUI 2.x/3 有：Normal=250 / Fast=167 / Faster=83 / FastAnimationAfter=168 ms；**Slow 不存在**。`ControlFastOutSlowInKeySpline` **键名确认存在（WinUI 2.8.6 PRI ×12），但值未取到**。 |
| `PointerDownThemeAnimation` 的 Duration / SpeedRatio / 位移像素 | **未在快照中**（195 个标签 0 属性）；官方文档明确 Duration 预置但不给数值，且桌面语义是 **shrink + tilt（写 Projection/RenderTransform）**，**不是 Y 位移**。 |
| `PointerUpThemeAnimation` 的 Duration / SpeedRatio | **未在快照中**。 |
| `RepositionThemeAnimation`（ProgressBar / ToggleSwitch）的 Duration | **未在快照中**。 |
| `FadeInThemeAnimation` / `FadeOutThemeAnimation` / `SplitOpenThemeAnimation` / `SplitCloseThemeAnimation` 的 Duration | **未在快照中**（全 A 无 Duration 属性）。 |
| `EntranceThemeTransition` / `AddDeleteThemeTransition` / `ContentThemeTransition` / `ReorderThemeTransition` 的默认 Duration 与缓动 | **未在快照中**；官方 API 文档亦不给（连 `FromVerticalOffset` 的默认值也不给）。 |
| `NavigationThemeTransition` / `PaneThemeTransition` / `EdgeUIThemeTransition` / `DrillInThemeTransition` / `PopupThemeTransition` / `SwipeHintThemeAnimation` | **A / A′ 0 命中 = 未找到**（`NavigationThemeTransition` 在 A / A′ / B 均 0 命中，即 SDK 模板**从不使用**它）。 |
| ProgressRing 的 `TrimStart` / `TrimEnd` 关键帧 | **未找到**（UWP OS ProgressRing 无这两个属性；WinUI 2.8.6 是 Lottie）。 |
| ProgressRing 的 `cubic-bezier(0.167,0.167,0.833,0.833)` 双段曲线 | **未找到**（A 中的 ProgressRing 曲线是 §5.1 那一串非对称 KeySpline；`0.167` 在 A 里是时间不是控制点）。 |
| ProgressRing 在 WinUI 2.8.6 的时长 / 角度 | **未找到**（Lottie，动画体在 native 资源）。 |
| SwipeControl 展开/吸合/执行的时长 | **未在快照中**（两源模板均为空壳，连 VisualState 都没有）。 |
| ToolTip 弹出/关闭时长 | **未在快照中**（仅 `FadeIn/FadeOutThemeAnimation`）。 |
| TeachingTip 在 UWP OS 的模板 | **A / A′ 0 命中**；仅 WinUI 2.8.6（Lottie）。 |
| FlyoutPresenter **本体**的开合时长 | **未在快照中**（模板零 Storyboard）；**但 `CommandBarFlyoutCommandBar` 模板里的 `OpeningStoryboard`/`ClosingStoryboard` 给出了可用的 300 / 150 ms**（`A:22093–22118`），见 §6.2。 |
| MenuFlyoutPresenter 开合时长 | **未在快照中**（`A:31148–31186` 零 Storyboard）。 |
| ContentDialog 0.5 / 0.167 / 0.083 s | **A / A′ / B 中未找到**（`0:0:0.5` 在 B 仅 2 处且属 ProgressBar）。 |
| ToggleSwitch 150 ms、ProgressBar Reposition 150 ms | **A / B 中未找到**（`RepositionThemeAnimation` 预置）。 |

---

## 12 · 取数脚本（可复现）

```powershell
$A  = 'C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\generic.xaml'
$A2 = 'C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.22621.0\Generic\generic.xaml'
$Ap = 'C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml'
$B  = 'C:\Users\mc158\.nuget\packages\microsoft.ui.xaml\2.8.6\lib\uap10.0\Microsoft.UI.Xaml\Themes\Generic.xaml'

# 1) 全局时长常量是否存在
Select-String -Path $A,$Ap,$B -Pattern 'AnimationDuration|ControlFast|ControlNormal|ControlSlow'   # → 0 命中

# 1b) UWP 运行时本体是否定义这些键（UTF-16 扫描）
$dll='C:\Windows\System32\Windows.UI.Xaml.dll'; $b=[System.IO.File]::ReadAllBytes($dll)
$s=[System.Text.Encoding]::Unicode.GetString($b)
foreach ($k in 'ControlFastAnimationDuration','ControlNormalAnimationDuration','ControlSlowAnimationDuration','ControlFasterAnimationDuration','ControlFastOutSlowInKeySpline') {
  "{0}: {1}" -f $k, ([regex]::Matches($s,[regex]::Escape($k))).Count    # → 全部 0
}

# 1c) WinUI 2.8.6 的 PRI 里是否有这些键（appx 内 entries/resources.pri）
#     路径：...\microsoft.ui.xaml\2.8.6\tools\AppX\x64\Release\Microsoft.UI.Xaml.2.8.appx
#     → resources.pri 的 UTF-16 字符串表中各键 ×12；值流含 00:00:00.250/.167/.168/.083

# 2) 全部 Duration 字面量（非 0）
Select-String -Path $A -Pattern 'Duration="[^0"][^"]*"|GeneratedDuration="[^0"][^"]*"'

# 3) 全部 KeySpline
Select-String -Path $A -Pattern 'KeySpline'

# 4) SpeedRatio 是否存在
Select-String -Path $A,$Ap,$B -Pattern 'SpeedRatio'   # → 0 命中

# 5) 时长资源定义
Select-String -Path $A,$Ap -Pattern 'x:String x:Key="[^"]*(Duration|BeginTime|Delay)[^"]*"'

# 6) 主题过渡/动画标签全部位置
Select-String -Path $A -Pattern 'PointerDownThemeAnimation|PointerUpThemeAnimation|RepositionThemeAnimation|EntranceThemeTransition|AddDeleteThemeTransition|Split(Open|Close)ThemeAnimation|ThemeTransitionCollection|TransitionCollection'

# 7) 关键负向证据：Pointer 主题动画是否带任何时长/缓动/位移属性
$pat='<Pointer(Up|Down)ThemeAnimation[^>]*\s(Duration|SpeedRatio|KeySpline|From|To|FromHorizontalOffset|FromVerticalOffset)\s*='
(Select-String -Path $A,$Ap -Pattern $pat -AllMatches | Measure-Object).Count   # → 0

# 8) 标签计数（应为 A: 38/78，A′: 26/53）
foreach ($f in @($A,$Ap)) { foreach ($k in '<PointerDownThemeAnimation','<PointerUpThemeAnimation') {
  "{0} {1}: {2}" -f (Split-Path $f -Leaf), $k, (Select-String -Path $f -Pattern $k -AllMatches | ForEach-Object { $_.Matches.Count } | Measure-Object -Sum).Sum } }
```

**版本一致性校验**：`A2`（22621）与 `A`（26100）均为 29 184 行，本次抽查的全部行号（Duration、KeySpline、ProgressBar、ProgressRing、ToggleSwitch、Button）**内容完全相同**，无 SDK 版本差异。
