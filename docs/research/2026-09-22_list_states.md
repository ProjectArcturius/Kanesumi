# 列表 / 树 / 标签行视觉状态 —— 权威取数报告

**取数日期**：2026-09-22（本机 Windows）
**方式**：只读 grep/read 两份一手 XAML。未运行 cargo，未改动任何其他文件。
**结论口径**：分不清就写「未找到」。凡两源不同，两个都列。

---

## 0. 源、行号别名与读法

| 别名 | 完整路径 | 大小 |
|---|---|---|
| `A-generic.xaml` | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\generic.xaml` | 2,653,934 B（31,189 行） |
| `A-themeresources.xaml` | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml` | 1,248,103 B（13,125 行） |
| `B`（WinUI 2.8.6） | `C:\Users\mc158\.nuget\packages\microsoft.ui.xaml\2.8.6\lib\uap10.0\Microsoft.UI.Xaml\Themes\Generic.xaml` | 371,890 B（4,683 行） |

### 0.1 字典分区（决定「暗色值」是哪一块）

`A-generic.xaml`：
```
L10    :         <ResourceDictionary x:Key="Default">      ← 暗色
L2065  :         <ResourceDictionary x:Key="HighContrast">
L4119  :         <ResourceDictionary x:Key="Light">         ← 亮色
L6174  :     </ResourceDictionary.ThemeDictionaries>
```
`A-themeresources.xaml`：
```
L4     :     <ResourceDictionary x:Key="Default">           ← 暗色
L1962  :     <ResourceDictionary x:Key="HighContrast">
L3920  :     <ResourceDictionary x:Key="Light">             ← 亮色
L5878  :   </ResourceDictionary.ThemeDictionaries>
```

> 注意：两份文件同目录、同一次 SDK 安装，`A-generic.xaml` 里的**笔刷**用 `{StaticResource <色>}` 指向
> `A-themeresources.xaml` 里的**色常量**。所以「解析到字面量」必须跨这两个文件。下面每条链都给出两端的行号。

### 0.2 三处结构性事实（影响下面几乎所有判断）

1. `A-generic.xaml:L28054` 把 `ListViewItem` 的默认样式指向 Reveal 样式：
   ```
       <Style TargetType="ListViewItem" BasedOn="{StaticResource ListViewItemRevealStyle}" />
   ```
   即 **26100 上 ListViewItem 的「默认模板」就是 Reveal 模板**（`A-generic.xaml:L24377`）。
   `GridViewItem` 同理（`A-generic.xaml:L30255`）。
2. ListViewItem / GridViewItem 的底色**不是**由 VisualState 的 Setter 设的，而是由
   `ListViewItemPresenter` 的依赖属性（`PointerOverBackground` / `PressedBackground` / …）决定；
   VisualState 只改 `RevealBrush.State` 与 `RevealBorderBrush` 两个附加属性。
3. **`B` 只是 MUX 控件（NavigationView/TabView/TreeView/NumberBox/Expander/…）的样式与模板字典，
   不含通用笔刷常量字典** —— 它大量 `{ThemeResource X}` 的 X 在 `B` 内**没有定义**
   （实测：`grep TreeViewItemBackground` 在 `B` 内只有引用、0 处定义）。
   因此「某个笔刷在 `B` 里叫什么」可查，「它等于什么颜色」必须回到 `A`。

---

## 1. 控件 × 视觉状态 × 笔刷 × 解析值

下面所有百分比 = alpha 字节 ÷ 255；RRGGBBAA = 本库顺序；暗色承载色 = 白，亮色承载色 = 黑。

### 1.0 解析链上的全部色常量（数值的唯一出处）

| 常量 | 暗色（Default） | 高对比 | 亮色（Light） | 出处（照抄整行） |
|---|---|---|---|---|
| `SystemListLowColor` | `#19FFFFFF` | `#19FFFFFF` | `#19000000` | `A-themeresources.xaml:L228 / L1986 / L4144` |
| `SystemListMediumColor` | `#33FFFFFF` | `#33FFFFFF` | `#33000000` | `A-themeresources.xaml:L229 / L1987 / L4145` |
| `SystemRevealListLowColor` | `#18FFFFFF` | `#19FFFFFF` | `#17000000` | `A-themeresources.xaml:L1388 / L3346 / L5304` |
| `SystemRevealListMediumColor` | `#30FFFFFF` | `#33FFFFFF` | `#2E000000` | `A-themeresources.xaml:L1389 / L3347 / L5305` |
| `SystemBaseLowColor` | `#33FFFFFF` | — | `#33000000` | `A-themeresources.xaml:L211 / L4127` |
| `SystemBaseMediumLowColor` | `#66FFFFFF` | — | `#66000000` | `A-themeresources.xaml:L214 / L4130` |
| `SystemAccentColor` | **无字面值**（系统提供 = 用户强调色） | — | 同 | 全文件 0 处 `<Color x:Key="SystemAccentColor">` |

照抄整行（暗色块）：
```
      <Color x:Key="SystemListLowColor">#19FFFFFF</Color>
      <Color x:Key="SystemListMediumColor">#33FFFFFF</Color>
      <Color x:Key="SystemRevealListLowColor">#18FFFFFF</Color>
      <Color x:Key="SystemRevealListMediumColor">#30FFFFFF</Color>
      <Color x:Key="SystemBaseLowColor">#33FFFFFF</Color>
      <Color x:Key="SystemBaseMediumLowColor">#66FFFFFF</Color>
```
（`A-themeresources.xaml:L228 / L229 / L1388 / L1389 / L211 / L214`）

亮色块（`A-themeresources.xaml:L4144 / L4145 / L5304 / L5305 / L4127 / L4130`）：
```
      <Color x:Key="SystemListLowColor">#19000000</Color>
      <Color x:Key="SystemListMediumColor">#33000000</Color>
      <Color x:Key="SystemRevealListLowColor">#17000000</Color>
      <Color x:Key="SystemRevealListMediumColor">#2E000000</Color>
      <Color x:Key="SystemBaseLowColor">#33000000</Color>
      <Color x:Key="SystemBaseMediumLowColor">#66000000</Color>
```

数值换算表：

| 字面量（AARRGGBB） | RRGGBBAA | alpha 字节 | 相对承载色 |
|---|---|---|---|
| `#19FFFFFF` | `#FFFFFF19` | 25 | 白 **9.8%** |
| `#33FFFFFF` | `#FFFFFF33` | 51 | 白 **20.0%** |
| `#18FFFFFF` | `#FFFFFF18` | 24 | 白 **9.4%** |
| `#30FFFFFF` | `#FFFFFF30` | 48 | 白 **18.8%** |
| `#21FFFFFF` | `#FFFFFF21` | 33 | 白 **12.9%** |
| `#4DFFFFFF` | `#FFFFFF4D` | 77 | 白 **30.2%** |
| `#66FFFFFF` | `#FFFFFF66` | 102 | 白 **40.0%** |
| `#19000000` | `#00000019` | 25 | 黑 **9.8%** |
| `#33000000` | `#00000033` | 51 | 黑 **20.0%** |
| `#17000000` | `#00000017` | 23 | 黑 **9.0%** |
| `#2E000000` | `#0000002E` | 46 | 黑 **18.0%** |
| `#3D000000` | `#0000003D` | 61 | 黑 **23.9%** |
| `#4D000000` | `#0000004D` | 77 | 黑 **30.2%** |
| `#66000000` | `#00000066` | 102 | 黑 **40.0%** |

### 1.1 ListViewItem

用到的基础笔刷（三个主题块各一份，**完全同构**）：

| 键 | 解析 | 出处 |
|---|---|---|
| `SystemControlHighlightListLowBrush` | → `SystemListLowColor` | `A-generic.xaml:L327`（暗）/ `L2174`（高对比 = `SystemColorHighlightColor`）/ `L4436`（亮） |
| `SystemControlHighlightListMediumBrush` | → `SystemListMediumColor` | `A-generic.xaml:L326` / `L2173` / `L4435` |
| `SystemControlTransparentBrush` | → `Color="Transparent"` | `A-themeresources.xaml:L326` |
| `SystemControlHighlightTransparentBrush` | → `Color="Transparent"` | `A-themeresources.xaml:L309` |

照抄整行（暗色块，`A-generic.xaml:L326-327`）：
```
            <SolidColorBrush x:Key="SystemControlHighlightListMediumBrush" Color="{StaticResource SystemListMediumColor}" />
            <SolidColorBrush x:Key="SystemControlHighlightListLowBrush" Color="{StaticResource SystemListLowColor}" />
```
亮色块（`A-generic.xaml:L4435-4436` 与 `A-themeresources.xaml:L4222-4223`）逐字相同的一对（仍指 `SystemListMediumColor`/`SystemListLowColor`，而这两个色常量在亮色块里已是黑值）：
```
            <SolidColorBrush x:Key="SystemControlHighlightListMediumBrush" Color="{StaticResource SystemListMediumColor}" />
            <SolidColorBrush x:Key="SystemControlHighlightListLowBrush" Color="{StaticResource SystemListLowColor}" />
```

**状态表**（键定义暗色块 `A-generic.xaml:L1882-1887`；高对比 `L3925-3930`；亮色 `L5991-5996`）：

| 视觉状态 | 资源键 | 最终笔刷 | 暗色 | 亮色 |
|---|---|---|---|---|
| Normal | `ListViewItemBackground` | `SystemControlTransparentBrush` | Transparent | Transparent |
| **PointerOver** | `ListViewItemBackgroundPointerOver` | **`SystemControlHighlightListLowBrush`** | `#19FFFFFF` → `#FFFFFF19` = 白 **9.8%** | `#19000000` → `#00000019` = 黑 **9.8%** |
| **Pressed** | `ListViewItemBackgroundPressed` | **`SystemControlHighlightListMediumBrush`** | `#33FFFFFF` → `#FFFFFF33` = 白 **20.0%** | `#33000000` → `#00000033` = 黑 **20.0%** |
| Selected | `ListViewItemBackgroundSelected` | `SystemControlHighlightListAccentLowBrush` | 强调色 ×**0.6** 不透明度 | 强调色 ×**0.4** |
| SelectedPointerOver | `ListViewItemBackgroundSelectedPointerOver` | `SystemControlHighlightListAccentMediumBrush` | 强调色 ×**0.8** | 强调色 ×**0.6** |
| SelectedPressed | `ListViewItemBackgroundSelectedPressed` | `SystemControlHighlightListAccentHighBrush` | 强调色 ×**0.9** | 强调色 ×**0.7** |
| Disabled | **未找到**：不存在 `ListViewItemBackgroundDisabled` 键 | 禁用靠 presenter 的 `DisabledOpacity` = `ListViewItemDisabledThemeOpacity` = **0.55** | — | — |

照抄整行（暗色块，`A-generic.xaml:L1882-1887`）：
```
            <StaticResource x:Key="ListViewItemBackground" ResourceKey="SystemControlTransparentBrush" />
            <StaticResource x:Key="ListViewItemBackgroundPointerOver" ResourceKey="SystemControlHighlightListLowBrush" />
            <StaticResource x:Key="ListViewItemBackgroundPressed" ResourceKey="SystemControlHighlightListMediumBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelected" ResourceKey="SystemControlHighlightListAccentLowBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightListAccentMediumBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelectedPressed" ResourceKey="SystemControlHighlightListAccentHighBrush" />
```
亮色块（`A-generic.xaml:L5991-5996`）：
```
            <StaticResource x:Key="ListViewItemBackground" ResourceKey="SystemControlTransparentBrush" />
            <StaticResource x:Key="ListViewItemBackgroundPointerOver" ResourceKey="SystemControlHighlightListLowBrush" />
            <StaticResource x:Key="ListViewItemBackgroundPressed" ResourceKey="SystemControlHighlightListMediumBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelected" ResourceKey="SystemControlHighlightListAccentLowBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightListAccentMediumBrush" />
            <StaticResource x:Key="ListViewItemBackgroundSelectedPressed" ResourceKey="SystemControlHighlightListAccentHighBrush" />
```
Accent 族的解析（`A-themeresources.xaml:L303-305` 暗 / `L4219-4221` 亮）：
```
      <SolidColorBrush x:Key="SystemControlHighlightListAccentHighBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.9" />
      <SolidColorBrush x:Key="SystemControlHighlightListAccentLowBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.6" />
      <SolidColorBrush x:Key="SystemControlHighlightListAccentMediumBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.8" />
```
```
      <SolidColorBrush x:Key="SystemControlHighlightListAccentHighBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.7" />
      <SolidColorBrush x:Key="SystemControlHighlightListAccentLowBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.4" />
      <SolidColorBrush x:Key="SystemControlHighlightListAccentMediumBrush" Color="{ThemeResource SystemAccentColor}" Opacity="0.6" />
```
> ⚠ 这几个**无法解析到 `#AARRGGBB` 字面量**：`SystemAccentColor` 由系统在运行时按用户强调色注入，
> 两份 XAML 全文 0 处字面定义（`grep 'SystemAccentColor"'` 只命中 AcrylicBrush 的 `TintColor="{ThemeResource …}"`）。

禁用透明度照抄整行（暗 `A-themeresources.xaml:L1772` / 高对比 `L3750` / 亮 `L5688`）：
```
      <x:Double x:Key="ListViewItemDisabledThemeOpacity">0.55</x:Double>
```

**Reveal 模式**（键定义暗色块 `A-generic.xaml:L1596-1602`，高对比 `L3650-3656`，亮色 `L5705-5711`；笔刷本体在 `A-themeresources.xaml:L1472/1520/1522/1524/1528/1530/1532` 暗、`L5388/5436/5438/5440/5444/5446/5448` 亮）：

| 资源键 | 最终笔刷 | 暗色（reveal 生效 / fallback） | 亮色（reveal 生效 / fallback） |
|---|---|---|---|
| `ListViewItemRevealBackground` | `SystemControlTransparentRevealBackgroundBrush` | Transparent / Transparent | Transparent / Transparent |
| `ListViewItemRevealBackgroundPointerOver` | `SystemControlHighlightListLowRevealBackgroundBrush` | `#18FFFFFF` = 白 **9.4%** / `#19FFFFFF` = 白 9.8% | `#17000000` = 黑 **9.0%** / `#19000000` = 黑 9.8% |
| `ListViewItemRevealBackgroundPressed` | `SystemControlHighlightListMediumRevealListLowBackgroundBrush` | `#18FFFFFF` = 白 **9.4%** / `#33FFFFFF` = 白 20.0% | `#17000000` = 黑 **9.0%** / `#33000000` = 黑 20.0% |
| `ListViewItemRevealBackgroundSelected` | `SystemControlHighlightAccent3RevealBackgroundBrush` | `SystemAccentColorDark3`（无字面量） | `SystemAccentColorLight3`（无字面量） |
| `ListViewItemRevealBackgroundSelectedPointerOver` | `SystemControlHighlightAccent2RevealBackgroundBrush` | `SystemAccentColorDark2` | `SystemAccentColorLight2` |
| `ListViewItemRevealBackgroundSelectedPressed` | `SystemControlHighlightAccent3RevealAccent2BackgroundBrush` | `SystemAccentColorDark2`（fallback `…Dark3`） | `SystemAccentColorLight2`（fallback `…Light3`） |
| `ListViewItemRevealBorderBrush` | `SystemControlTransparentBrush` | Transparent | Transparent |
| `ListViewItemRevealBorderBrushPointerOver` / `…Pressed` | `SystemControlTransparentRevealBorderBrush` | Transparent（`A-generic.xaml:L1633`） | Transparent（`A-generic.xaml:L5742`） |

照抄整行（暗色块，`A-generic.xaml:L1596-1602`）：
```
            <StaticResource x:Key="ListViewItemRevealBackground" ResourceKey="SystemControlTransparentRevealBackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealBackgroundPointerOver" ResourceKey="SystemControlHighlightListLowRevealBackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealBackgroundPressed" ResourceKey="SystemControlHighlightListMediumRevealListLowBackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealBackgroundSelected" ResourceKey="SystemControlHighlightAccent3RevealBackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightAccent2RevealBackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealBackgroundSelectedPressed" ResourceKey="SystemControlHighlightAccent3RevealAccent2BackgroundBrush" />
            <StaticResource x:Key="ListViewItemRevealPlaceholderBackground" ResourceKey="SystemControlDisabledChromeDisabledHighBrush" />
```
笔刷本体照抄整行（暗，`A-themeresources.xaml:L1472 / L1520 / L1522 / L1524`）：
```
      <RevealBackgroundBrush x:Key="SystemControlHighlightListMediumRevealBackgroundBrush" TargetTheme="Dark" Color="{StaticResource SystemRevealListMediumColor}" FallbackColor="{StaticResource SystemListMediumColor}" />
      <RevealBackgroundBrush x:Key="SystemControlTransparentRevealBackgroundBrush" TargetTheme="Dark" Color="Transparent" FallbackColor="Transparent" />
      <RevealBackgroundBrush x:Key="SystemControlHighlightListMediumRevealListLowBackgroundBrush" TargetTheme="Dark" Color="{StaticResource SystemRevealListLowColor}" FallbackColor="{StaticResource SystemListMediumColor}" />
      <RevealBackgroundBrush x:Key="SystemControlHighlightListLowRevealBackgroundBrush" TargetTheme="Dark" Color="{StaticResource SystemRevealListLowColor}" FallbackColor="{StaticResource SystemListLowColor}" />
```
亮色（`A-themeresources.xaml:L5388 / L5436 / L5438 / L5440`）：
```
      <RevealBackgroundBrush x:Key="SystemControlHighlightListMediumRevealBackgroundBrush" TargetTheme="Light" Color="{StaticResource SystemRevealListMediumColor}" FallbackColor="{StaticResource SystemListMediumColor}" />
      <RevealBackgroundBrush x:Key="SystemControlTransparentRevealBackgroundBrush" TargetTheme="Light" Color="Transparent" FallbackColor="Transparent" />
      <RevealBackgroundBrush x:Key="SystemControlHighlightListMediumRevealListLowBackgroundBrush" TargetTheme="Light" Color="{StaticResource SystemRevealListLowColor}" FallbackColor="{StaticResource SystemListMediumColor}" />
      <RevealBackgroundBrush x:Key="SystemControlHighlightListLowRevealBackgroundBrush" TargetTheme="Light" Color="{StaticResource SystemRevealListLowColor}" FallbackColor="{StaticResource SystemListLowColor}" />
```

> **关键结构性发现（决定 30% 那个数从哪来，见 §3）**：默认样式把 presenter 的 `RevealBackground`
> 恒绑到基础键 `ListViewItemRevealBackground`，**从不引用** `…RevealBackgroundPointerOver` /
> `…RevealBackgroundPressed`。
> 照抄整行（`A-generic.xaml:L24421`，样式内 presenter）：
> ```
>                         RevealBackground="{ThemeResource ListViewItemRevealBackground}"
> ```
> 实测 `grep 'ListViewItemRevealBackgroundPointerOver'` 在两份 A 文件里共 6 处命中，
> **全部是 `x:Key=` 定义，0 处 `Value=`/`RevealBackground=` 引用**。即：现代 UWP 下
> ListView 行的悬停底**只**由 presenter 的 `PointerOverBackground`（= ListLow = 9.8%）决定。

presenter 属性绑定照抄整行（`A-generic.xaml:L24407 / L24412 / L24414`）：
```
                        PointerOverBackground="{ThemeResource ListViewItemBackgroundPointerOver}"
                        PressedBackground="{ThemeResource ListViewItemBackgroundPressed}"
                        DisabledOpacity="{ThemeResource ListViewItemDisabledThemeOpacity}"
```

### 1.2 GridViewItem

键定义：暗色 `A-generic.xaml:L1009-1014`，高对比 `L2858-2863`，亮色 `L5119-5124`。

| 视觉状态 | 资源键 | 最终笔刷 | 暗色 | 亮色 |
|---|---|---|---|---|
| Normal | `GridViewItemBackground` | `SystemControlTransparentBrush` | Transparent | Transparent |
| PointerOver | `GridViewItemBackgroundPointerOver` | `SystemControlHighlightListLowBrush` | 白 **9.8%** | 黑 **9.8%** |
| Pressed | `GridViewItemBackgroundPressed` | `SystemControlHighlightListMediumBrush` | 白 **20.0%** | 黑 **20.0%** |
| Selected | `GridViewItemBackgroundSelected` | **`SystemControlHighlightAccentBrush`** | 强调色 **100%**（无 opacity 乘数） | 同 |
| SelectedPointerOver | `GridViewItemBackgroundSelectedPointerOver` | `SystemControlHighlightListAccentMediumBrush` | 强调色 ×0.8 | 强调色 ×0.6 |
| SelectedPressed | `GridViewItemBackgroundSelectedPressed` | `SystemControlHighlightListAccentHighBrush` | 强调色 ×0.9 | 强调色 ×0.7 |
| Disabled | **未找到**：无 `GridViewItemBackgroundDisabled` 键 | 同 ListView，`ListViewItemDisabledThemeOpacity` = 0.55 | — | — |

照抄整行（暗色块，`A-generic.xaml:L1009-1014`）：
```
            <StaticResource x:Key="GridViewItemBackground" ResourceKey="SystemControlTransparentBrush" />
            <StaticResource x:Key="GridViewItemBackgroundPointerOver" ResourceKey="SystemControlHighlightListLowBrush" />
            <StaticResource x:Key="GridViewItemBackgroundPressed" ResourceKey="SystemControlHighlightListMediumBrush" />
            <StaticResource x:Key="GridViewItemBackgroundSelected" ResourceKey="SystemControlHighlightAccentBrush" />
            <StaticResource x:Key="GridViewItemBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightListAccentMediumBrush" />
            <StaticResource x:Key="GridViewItemBackgroundSelectedPressed" ResourceKey="SystemControlHighlightListAccentHighBrush" />
```
亮色块（`A-generic.xaml:L5119-5124`）逐字相同。

Reveal 模式（暗 `A-generic.xaml:L1603-1610` / 亮 `L5712-5719`）：

| 资源键 | 最终笔刷 | 暗色 | 亮色 |
|---|---|---|---|
| `GridViewItemRevealBackground` | `SystemControlTransparentRevealBackgroundBrush` | Transparent | Transparent |
| `GridViewItemRevealBackgroundPointerOver` | `SystemControlHighlightListLowRevealBackgroundBrush` | 白 **9.4%** / fb 9.8% | 黑 **9.0%** / fb 9.8% |
| `GridViewItemRevealBackgroundPressed` | `SystemControlHighlightListMediumRevealListLowBackgroundBrush` | 白 **9.4%** / fb 20.0% | 黑 **9.0%** / fb 20.0% |
| `GridViewItemRevealBackgroundSelected` | **`SystemControlHighlightAccentRevealBackgroundBrush`** | `SystemAccentColor`（100%，`A-themeresources.xaml:L1515`） | 同（`L5431`） |
| `GridViewItemRevealBackgroundSelectedPointerOver` | `SystemControlHighlightAccent2RevealBackgroundBrush` | `SystemAccentColorDark2` | `Light2` |
| `GridViewItemRevealBackgroundSelectedPressed` | `SystemControlHighlightAccent3RevealAccent2BackgroundBrush` | `Dark2`（fb `Dark3`） | `Light2`（fb `Light3`） |

**GridViewItem 与 ListViewItem 的差异（四处）**：

| 项 | ListViewItem | GridViewItem | 出处 |
|---|---|---|---|
| Selected 底 | `SystemControlHighlightListAccentLowBrush`（强调色 ×0.6 暗 / ×0.4 亮） | `SystemControlHighlightAccentBrush`（**强调色 100%**） | `A-generic.xaml:L1012` vs `L1885` |
| Reveal Selected 底 | `SystemControlHighlightAccent3RevealBackgroundBrush`（`AccentColorDark3`） | `SystemControlHighlightAccentRevealBackgroundBrush`（**`SystemAccentColor` 全强度**） | `L1607` vs `L1599` |
| ForegroundPointerOver | `SystemControlHighlightAltBaseHighBrush` | `SystemControlForegroundBaseHighBrush` | `L1016` vs `L1889` |
| CheckMode | `Inline` | `Overlay` | `L1028` vs `L1901` |
| PointerOver / Pressed 底 | **两者完全一致**（ListLow 9.8% / ListMedium 20%） | 同 | `L1010-1011` = `L1883-1884` |

照抄整行（差异 1、2）：
```
            <StaticResource x:Key="GridViewItemBackgroundSelected" ResourceKey="SystemControlHighlightAccentBrush" />
            <StaticResource x:Key="GridViewItemRevealBackgroundSelected" ResourceKey="SystemControlHighlightAccentRevealBackgroundBrush" />
```
（`A-generic.xaml:L1012` / `L1607`）

### 1.3 TreeViewItem

源 A 定义：暗色 `A-generic.xaml:L1937-1944`（= `A-themeresources.xaml:L1837-1844`），
高对比 `A-generic.xaml:L3991-3998`，亮色 `A-generic.xaml:L6046-6053`。

| 视觉状态 | 资源键 | 最终笔刷 | 暗色 | 亮色 |
|---|---|---|---|---|
| Normal | `TreeViewItemBackground` | `SystemControlTransparentRevealBackgroundBrush` | Transparent | Transparent |
| **PointerOver** | `TreeViewItemBackgroundPointerOver` | `SystemControlHighlightListLowRevealBackgroundBrush` | `#18FFFFFF` → `#FFFFFF18` = 白 **9.4%**（fallback `#19FFFFFF` = 9.8%） | `#17000000` → `#00000017` = 黑 **9.0%**（fallback 9.8%） |
| **Pressed** | `TreeViewItemBackgroundPressed` | `SystemControlHighlightListMediumRevealBackgroundBrush` | `#30FFFFFF` → `#FFFFFF30` = 白 **18.8%**（fallback 20.0%） | `#2E000000` → `#0000002E` = 黑 **18.0%**（fallback 20.0%） |
| **Selected** | `TreeViewItemBackgroundSelected` | `SystemControlHighlightAccent3RevealBackgroundBrush` | **强调色 `SystemAccentColorDark3`** | **强调色 `SystemAccentColorLight3`** |
| **SelectedPointerOver** | `TreeViewItemBackgroundSelectedPointerOver` | `SystemControlHighlightAccent2RevealBackgroundBrush` | **强调色 `SystemAccentColorDark2`** | **强调色 `SystemAccentColorLight2`** |
| SelectedPressed | `TreeViewItemBackgroundSelectedPressed` | `SystemControlHighlightListMediumRevealBackgroundBrush` | 白 **18.8%** | 黑 **18.0%** |
| Disabled | `TreeViewItemBackgroundDisabled` | `SystemControlTransparentBrush` | Transparent | Transparent |
| SelectedDisabled | `TreeViewItemBackgroundSelectedDisabled` | `SystemControlDisabledBaseMediumLowBrush` → `SystemBaseMediumLowColor` | `#66FFFFFF` = 白 **40.0%** | `#66000000` = 黑 **40.0%** |

照抄整行（暗色块，`A-generic.xaml:L1937-1944`）：
```
            <StaticResource x:Key="TreeViewItemBackground" ResourceKey="SystemControlTransparentRevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundPointerOver" ResourceKey="SystemControlHighlightListLowRevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundPressed" ResourceKey="SystemControlHighlightListMediumRevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundDisabled" ResourceKey="SystemControlTransparentBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundSelected" ResourceKey="SystemControlHighlightAccent3RevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightAccent2RevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundSelectedPressed" ResourceKey="SystemControlHighlightListMediumRevealBackgroundBrush" />
            <StaticResource x:Key="TreeViewItemBackgroundSelectedDisabled" ResourceKey="SystemControlDisabledBaseMediumLowBrush" />
```
亮色块（`A-generic.xaml:L6046-6053`）逐字相同。高对比块（`L3991-3998`）逐字相同。

**源 B（WinUI 2.8.6）里的 TreeViewItem**：`B` 有**完整模板**（`B:L4533` 起 `MUX_TreeViewItemStyle`），
但**没有这些键的定义** —— 它引用的是与源 A 同名的键，运行期由系统字典兜底。
照抄整行（`B:L4533 / L4535 / L4550 / L4561 / L4572 / L4593 / L4604`）：
```
  <Style TargetType="controls:TreeViewItem" BasedOn="{StaticResource DefaultListViewItemStyle}" x:Key="MUX_TreeViewItemStyle">
    <Setter Property="Background" Value="{ThemeResource TreeViewItemBackground}" />
                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundPointerOver}" />
                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundPressed}" />
                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelected}" />
                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelectedPointerOver}" />
                    <Setter Target="ContentPresenterGrid.Background" Value="{ThemeResource TreeViewItemBackgroundSelectedPressed}" />
```
> 判定：TreeViewItem **不是「只在源 B 有」** —— 两源都有；但**色值只能从源 A 取**。
> 两源对此控件的状态语义一致（同样的 6 个状态 + Disabled/SelectedDisabled）。

### 1.4 TabViewItem 的关闭按钮（源 B）—— **未找到**

`B` 里存在关闭按钮样式 `TabViewCloseButtonStyle`，它引用了 6 个 `TabViewItemHeaderCloseButton*` 笔刷键：

照抄整行（`B:L2688 / L2696-2699 / L2711 / L2724`）：
```
  <Style x:Key="TabViewCloseButtonStyle" TargetType="Button">
    <Setter Property="Background" Value="{ThemeResource TabViewItemHeaderCloseButtonBackground}" />
    <Setter Property="Foreground" Value="{ThemeResource TabViewItemHeaderCloseButtonForeground}" />
    <Setter Property="BorderBrush" Value="{ThemeResource TabViewItemHeaderCloseButtonBorderBrush}" />
    <Setter Property="BorderThickness" Value="{ThemeResource TabViewItemHeaderCloseButtonBorderThickness}" />
                      <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource TabViewItemHeaderCloseButtonBackgroundPointerOver}" />
                      <DiscreteObjectKeyFrame KeyTime="0" Value="{ThemeResource TabViewItemHeaderCloseButtonBackgroundPressed}" />
```

但**这两份源里都没有这些键的定义**，据以判定「未找到」的实测证据：

| 检索 | 结果 |
|---|---|
| `B` 内 `TabViewItemHeaderCloseButton` | 33 处，**全部是引用**（L2694–L2730、L2917），0 处 `x:Key=` 定义 |
| `A-generic.xaml` + `A-themeresources.xaml` 内 `TabView` | **0 处命中**（UDK 26100 不含 TabView，TabView 是 WinUI 2 专有控件） |
| 同机另一 SDK `UAP\10.0.22621.0\Generic\*.xaml` 内 `TabViewItemHeaderCloseButton` | **0 处命中** |
| `Microsoft.UI.Xaml.pri`（1,400 B）内 ASCII/UTF-16 检索该键 | **0 处命中**（该 pri 是空壳索引，不含 XBF 数据） |
| `B` 内 `SubtleFillColorSecondary` / `SubtleFillColorTertiary` | **0 处命中** |
| `A` 两文件内 `SubtleFillColor` | **0 处命中** |

→ **TabView 关闭按钮的 PointerOver / Pressed 底色：未找到权威色值。** 不猜。
（能确定的只有：`B:L2711` 明确说 PointerOver 用的是 `…CloseButtonBackgroundPointerOver`，
`B:L2724` 明确说 Pressed 用的是 `…CloseButtonBackgroundPressed`；两者的**取值不在这两份源里**。）

### 1.5 Pivot / ListView 头部

`PivotHeaderItemBackground*`（暗色 `A-generic.xaml:L989-995`，= `A-themeresources.xaml:L919-925`；高对比 `L2677-2683`；亮色 `A-themeresources.xaml:L4835-4841`）：

| 资源键 | 最终笔刷 | 解析值 |
|---|---|---|
| `PivotHeaderItemBackgroundUnselected` | `SystemControlTransparentBrush` | Transparent |
| `PivotHeaderItemBackgroundUnselectedPointerOver` | `SystemControlHighlightTransparentBrush` | **Transparent** |
| `PivotHeaderItemBackgroundUnselectedPressed` | `SystemControlHighlightTransparentBrush` | **Transparent** |
| `PivotHeaderItemBackgroundSelected` | `SystemControlHighlightTransparentBrush` | **Transparent** |
| `PivotHeaderItemBackgroundSelectedPointerOver` | `SystemControlHighlightTransparentBrush` | **Transparent** |
| `PivotHeaderItemBackgroundSelectedPressed` | `SystemControlHighlightTransparentBrush` | **Transparent** |
| `PivotHeaderItemBackgroundDisabled` | `SystemControlTransparentBrush` | Transparent |

照抄整行（暗色块，`A-generic.xaml:L989-995`）：
```
            <StaticResource x:Key="PivotHeaderItemBackgroundUnselected" ResourceKey="SystemControlTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundUnselectedPointerOver" ResourceKey="SystemControlHighlightTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundUnselectedPressed" ResourceKey="SystemControlHighlightTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundSelected" ResourceKey="SystemControlHighlightTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundSelectedPointerOver" ResourceKey="SystemControlHighlightTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundSelectedPressed" ResourceKey="SystemControlHighlightTransparentBrush" />
            <StaticResource x:Key="PivotHeaderItemBackgroundDisabled" ResourceKey="SystemControlTransparentBrush" />
```
`SystemControlHighlightTransparentBrush` 照抄整行（`A-themeresources.xaml:L309`）：
```
      <SolidColorBrush x:Key="SystemControlHighlightTransparentBrush" Color="Transparent" />
```
→ **Pivot 头悬停/按压/选中一律无底色**（Pivot 的反馈走前景与 Pipe/边框，不走底）。

另（ListView 头部，键在 `A-themeresources.xaml:L633`）：
```
      <StaticResource x:Key="ListViewHeaderItemBackground" ResourceKey="SystemControlTransparentBrush" />
```
`GridViewHeaderItemBackground` = `SystemControlTransparentBrush`（`A-generic.xaml:L1006`，= `A-themeresources.xaml:L935`）。

---

## 2. 一句话回答

**列表行 PointerOver 的真实 alpha = 9.8%（暗色 `SystemListLowColor` = `#19FFFFFF` → RRGGBBAA `#FFFFFF19`，alpha 25/255），
用的是 `SystemControlHighlightListLowBrush`，不是 `SystemControlHighlightListMediumBrush`；**
Medium（`#33FFFFFF` = 白 20.0%）是 **Pressed** 档；Reveal 模式下悬停对应的 `SystemRevealListLowColor` = `#18FFFFFF` = **白 9.4%**。

---

## 3. 与本库当前值的差异

> ⚠ **快照边界**：本节「本库现值」取自取数时刻的工作区，而该工作区**正被并发修改**
> （取数时 `git status` 只有 ` M kanesumi-core/src/indicator.rs`；本节写完后再查，已变成
> ` M kanesumi-controls/{auto_suggest_box,list,number_box,selector_flyout,text_box,tree_view}.rs`
> + ` M kanesumi-core/{colors,indicator}.rs`）。
> 因此 §3.1–§3.4 描述的是**取数时刻**的写法；写报告末尾复查时，
> `list.rs` 与 `tree_view.rs` 对 `list_hover_tint` / `subtle_hover_tint` 的调用点**已被改掉**
> （最后一次 grep 只剩 `swipe_control.rs:L245/L272`、`tab_view.rs:L285`、`title_bar.rs:L167` 四处）。
> 换言之：**§3 的判定（权威值是什么）不随之失效，但「本库现值」一列的 file:line 可能已过期**，
> 复审时应以「权威值」列为准，回查当时的最新代码。

本库现状同时给 HEAD 值与工作区值（`list_hover_tint` 在工作区已被删除 —— 见 §3.6 备注）。

| # | 本库现值 | 权威值 | 判定 |
|---|---|---|---|
| 1 | 行悬停 **暗色白 30%** | **白 9.8%**（`#19FFFFFF`） | **不一致** |
| 2 | 行悬停 **亮色黑 9.4%** | **黑 9.8%**（`#19000000`） | **不一致**（9.4% 是暗色 reveal 值，被错配到亮色） |
| 3 | 树行悬停 **白 15%** | **白 9.4%**（reveal）/ 9.8%（fallback） | **不一致** |
| 4 | 树行选中 **白 15%** | **强调色 `SystemAccentColorDark3`** | **不一致**（角色错了：UWP 树行选中是强调色底，不是中性白叠） |
| 5 | 标题栏按钮悬停白 15% / 按压白 25% | 无 15%/25% 依据。一手续候选两套：现代 `Button` = `SystemBaseLowColor` **白 20%** / `SystemBaseMediumLowColor` **白 40%**；旧式 `BackButtonPointerOverBackgroundThemeBrush` = `#21FFFFFF` **白 12.9%**（亮 `#3D000000` **黑 23.9%**） | **找不到权威依据**（本库注释引的 `SubtleFillColorSecondary/Tertiary` 在两份源里 0 命中 —— 那是 WinUI 3 token，不在本机一手源内） |
| 6 | 关闭按钮悬停白 15% | TabView 关闭按钮笔刷在源 A/B 均未定义 | **找不到权威依据** |

逐条证据：

**#1 / #2 行悬停**：HEAD `kanesumi-core/src/indicator.rs:L90`（经 `git show HEAD:…`）
`list_hover_tint: Color::from_hex(0xFF_FF_FF_4D), // 白 30%`（= 白 30.2%），
L109 `list_hover_tint: Color::from_rgba(0x00_00_00_18), // 黑 9.4%（待实测）`。
权威值见 §1.1。**30% 这个数的来源已定位**：源 A 里确实存在一个 30.2% 的旧笔刷
（照抄整行，暗 `A-generic.xaml:L1912` / 亮 `L6021` / 高对比 `L3966`）：
```
            <SolidColorBrush x:Key="ListViewItemPointerOverBackgroundThemeBrush" Color="#4DFFFFFF" />
            <SolidColorBrush x:Key="ListViewItemPointerOverBackgroundThemeBrush" Color="#4D000000" />
```
但它**定义了、从未被任何样式或模板引用**（`grep ListViewItemPointerOverBackgroundThemeBrush` 在两份 A 文件里共 6 处命中，全部是 `x:Key=` 定义，0 处 `Value=` 引用）。
它是 Win8 时代遗留键（同族还有 `ListViewItemSelectedBackgroundThemeBrush` = `#FF4617B4` 等）。
→ 结论：`CONTROL_SPEC §215` 的「白 30%」应改注为「已废弃的遗留笔刷，现行模板不使用」。

**#3 树行悬停**：`kanesumi-controls/src/tree_view.rs:L263-266`
```
            // 底：Selected / hover 白 15%（CONTROL_SPEC §936 SubtleFillColorSecondary）
            if selected || hovered {
                scene.fill_rect(theme.indication.subtle_hover_tint, r);
            }
```
权威 `TreeViewItemBackgroundPointerOver` = `SystemControlHighlightListLowRevealBackgroundBrush` = 白 **9.4%**（§1.3）。

**#4 树行选中**：同一处代码把 selected 与 hovered 合并成同一个 15% 白。
权威 `TreeViewItemBackgroundSelected` = `SystemControlHighlightAccent3RevealBackgroundBrush`
= `SystemAccentColorDark3`（暗）/ `SystemAccentColorLight3`（亮）—— **是强调色，不是中性白**。
若要走 UWP 语义，树的「选中」必须与「悬停」分成两个角色。

**#5 标题栏按钮**：`kanesumi-controls/src/title_bar.rs:L162-170`
```
            // 底：Hover 白 15% / Pressed 白 25%（CONTROL_SPEC §790/§791
            // SubtleFillColorSecondary / Tertiary）。
            let bg = if self.back_pressed {
                theme.indication.subtle_press_tint
            } else if self.back_hovered {
                theme.indication.subtle_hover_tint
```
（`subtle_hover_tint` = `on_surface.with_alpha(0.15)`，`subtle_press_tint` = 0.25，见 `indicator.rs:L119-120` / `L138-139`。）
一手源里没有 15%/25% 这一对。可用的两套一手续值：
- 现代返回键 = `NavigationBackButtonNormalStyle`（`A-generic.xaml:L15248`，`A-themeresources.xaml:L7539`），
  它是 `TargetType="Button"`，悬停/按压落在 Button 的通用状态上：
  ```
      <StaticResource x:Key="ButtonBackgroundPointerOver" ResourceKey="SystemControlBackgroundBaseLowBrush" />
      <StaticResource x:Key="ButtonBackgroundPressed" ResourceKey="SystemControlBackgroundBaseMediumLowBrush" />
  ```
  （`A-themeresources.xaml:L354-355`；`SystemBaseLowColor` = 白 `#33FFFFFF` **20%**，
  `SystemBaseMediumLowColor` = 白 `#66FFFFFF` **40%**）
- 旧式专用键（未被引用）：
  ```
      <SolidColorBrush x:Key="BackButtonPointerOverBackgroundThemeBrush" Color="#21FFFFFF" />
      <SolidColorBrush x:Key="BackButtonPointerOverBackgroundThemeBrush" Color="#3D000000" />
  ```
  （暗 `A-themeresources.xaml:L993` / 亮 `L4909`；= 白 12.9% / 黑 23.9%）

**#6 关闭按钮**：`kanesumi-controls/src/tab_view.rs:L282-289`
```
                if close_hovered {
                    // 关闭键悬停 = 浅叠 15%（CONTROL_SPEC §864 白 15% / 按压 25%）。
                    scene.fill_rounded_rect(
                        theme.indication.subtle_hover_tint,
```
权威值：未找到（§1.4）。另注：`tab_view.rs:L298` 的「＋」按钮悬停用 `hover_tint`（白 10.2%），
与 UWP `AppBarButtonBackgroundPointerOver` = `SystemControlHighlightListLowBrush`（白 9.8%）同档，
但 UWP 并无「TabView 加号按钮」这一控件可对照 —— 该处属 Kanesumi 自行指派，无一手对应项。

### 3.6 附带发现（不在题目清单，但会咬人）

1. **`indicator.rs` 取数时正被并发修改**：当时 `git status` = ` M kanesumi-core/src/indicator.rs`。
   工作区已删掉 `pub list_hover_tint` 字段，但
   `kanesumi-controls/src/list.rs:L110`（`theme.indication.list_hover_tint`）、
   `kanesumi-controls/src/auto_suggest_box.rs:L375`、以及 `indicator.rs:L259` 的对比度自检仍在用它。
   写报告末尾复查：`list.rs` / `tree_view.rs` 的调用点已被改掉，但
   `auto_suggest_box.rs` 与 `indicator.rs:L259` 的自检是否同步清理**未复核** —— 请以一次
   `cargo check` 收口。本报告不改代码，仅记录。
2. **禁用透明度**：UWP 的 ListView/GridView 行禁用用 `ListViewItemDisabledThemeOpacity` = **0.55**
   （`A-themeresources.xaml:L1772 / L3750 / L5688`），本库用 **0.38**。
   两份一手源里 `grep ControlDisabledThemeOpacity` **0 命中** —— 即 0.38 的出处不在这两份文件里。
3. **Accent 族不可解析为字面量**：`SystemAccentColor` / `…Dark1-3` / `…Light1-3` 由系统按用户强调色
   在运行时注入，XAML 里 0 处字面定义。凡本库需要「选中强调色底」，只能自定并登记，不能声称「照抄 UWP 字面量」。
   可照抄的只有**不透明度乘数**：暗 0.6/0.8/0.9，亮 0.4/0.6/0.7（ListAccent Low/Medium/High）。
4. **Pivot 头无底色**是设计事实而非缺失（`SystemControlHighlightTransparentBrush` = `Color="Transparent"`），
   本库若给 Pivot 头加 10% 白悬停底，是**偏离** UWP；若要保留，应在 `CANON_VS_TEMPORARY` 登记为角色选择。
