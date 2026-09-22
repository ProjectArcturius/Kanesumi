# UWP 一手源清单与取值对照（2026-09-22 立）

> **存在理由**：过去半年多次出现「规格里的百分比与笔刷名对不上」，而每次都只能推断 ——
> 因为一手源不在 Linux 上（`CONTROL_SPEC.md` 多处标注「未在快照中」）。
> 2026-09-22 在 Windows 上把**权威源找齐了**，并据此更正了一批取值。
> 本文是**取证路径与取值对照表**，供任何平台上的后续会话复核，不必再靠推测。
>
> 配套：`REFERENCE.md`（微软栈总索引）、`CONTROL_SPEC.md`（控件规格，已按本文更正）、
> `CANON_VS_TEMPORARY.md`（临时/偏离登记）、`ROADMAP.md`（M1-1 分类学）。

---

## §Ⅰ 一手源在哪（Windows 侧，全部本地可读）

| # | 源 | 路径 | 覆盖面 | 说明 |
|---|---|---|---|---|
| A | **OS UWP 主题字典** | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml` | `SystemControl*` 全族、`System*Color` 常量、各控件状态→笔刷映射、全局时长字符串 | **Windows SDK 自带**，正是控件库当年想取却「未在快照中」的那份。13125 行；`Default`＝暗 **L4-1961**、`HighContrast`＝**L1962-3919**、`Light`＝**L3920-5878** |
| A2 | OS UWP 控件模板 | 同目录 `generic.xaml`（2.65 MB / 29184 行） | 视觉状态 → Storyboard、模板结构、`PointerDown/UpThemeAnimation` 用法 | 同一 SDK |
| A′ | 上一版 SDK | `…\UAP\10.0.22621.0\Generic\`（同名两文件） | 同 A | 与 A **逐字节相同**，抽查无版本差异 —— 不必两版都查 |
| B | **WinUI 2.8.6 模板** | `%USERPROFILE%\.nuget\packages\microsoft.ui.xaml\2.8.6\lib\uap10.0\Microsoft.UI.Xaml\Themes\Generic.xaml`（4683 行） | 控件模板与状态引用 | ⚠ **不含任何 `<Color x:Key=`** —— 令牌字典编在 `tools\AppX\x64\Release\Microsoft.UI.Xaml.2.8.appx → resources.pri`（XBF 二进制）里，直接读不到 |
| C | **WinUI 2.x 令牌字典（源码）** | GitHub `microsoft/microsoft-ui-xaml` → `dev/CommonStyles/Common_themeresources_any.xaml`（58 KB） | `TextFillColor*` / `SubtleFillColor*` / `ControlFillColor*` / `SolidBackgroundFillColor*` / `SystemFillColor*` / 时长字符串 | **网络可用时最省事的一手源**（MIT）。raw 路径见 §Ⅳ |
| C2 | WinUI 2.x 各控件字典 | 同仓 `dev/<Control>/<Control>_themeresources.xaml` | TabView / NumberBox / InfoBar / Expander / RatingControl 等 | 例如 TabView 关闭键的笔刷就在这里 |
| D | WinUI 3 令牌字典 | 同仓 `main` 分支 **`controls/dev/CommonStyles/Common_themeresources_any.xaml`**（⚠ 路径已于 2026-09-22 更正：旧写的 `dev/CommonStyles/…` 在 main 上 404；`winui2/main/dev/…` 仍有效） | 现代令牌（本库 `StatusColors` 按其取值；T3 的亮色中性色已按其取到） | 已与发布版 `Microsoft.WindowsAppSDK.WinUI/1.8.250906003` 的 `Themes/generic.xaml` 交叉验证：**249 个 Color 键逐字节一致，差异 0** | 
| E | 运行时二进制 | `C:\Windows\System32\Windows.UI.Xaml.dll` | 反查「某键是否存在于 UWP 运行时」 | 本次用它**证伪** `Control*AnimationDuration` 属于 UWP（UTF-16 全量扫描 0 命中） |

**取数纪律（本次实际执行）**：每条值都要 `文件:行号` + 照抄原行；`{StaticResource X}`/`{ThemeResource X}`
必须解析到字面量；找不到就写「未找到」，不猜；两源不同就并列。XAML 的 `#AARRGGBB` 与本库的
**RRGGBBAA** 不同序，换算后还要给出「相对承载色的 alpha 百分比」。

---

## §Ⅱ 推翻的旧值（这些是本次真正修掉的东西）

| 本库旧值 | 出处（旧） | 一手源真值 | 依据 |
|---|---|---|---|
| 按下 tint **22%** | 无（疑误抄） | **20%** | `SystemListMediumColor` = `#33FFFFFF`/`#33000000`（A L229/L4145）；22% 的 `#38FFFFFF` 只出现在 `MediaDownloadProgressIndicatorThemeBrush`（A L1117），与交互无关 |
| 亮色悬停/按下 **5%/10%** | §65 | **10% / 20%** | `SystemListLowColor`/`ListMediumColor` 亮色字面值（A L4144/L4145）；UWP 两方案强度**对称** |
| 列表行悬停 **30%**（暗） | §215 | **9.8%** | `ListViewItemBackgroundPointerOver` → `SystemControlHighlightListLowBrush` → `SystemListLowColor`（A L1783→L307→L228）。30% 的真出处是 `ListViewItemPointerOverBackgroundThemeBrush` = `#4DFFFFFF`（A2 L1912）——**定义了却从未被任何样式引用的 Win8 遗留键** |
| 列表行悬停（亮）**9.4%** | WinUI 3 邻近令牌 | **9.8%** 黑 | 同上，亮色 L4144 |
| 行选中 **75%（UWP）→ 60%（Kanesumi 调低）** | §215/§217 | **AccentLow = 0.6 暗 / 0.4 亮** | `SystemControlHighlightListAccentLowBrush`（A L304/L4220）；一手源里没有 75% |
| `accent_low_tint` **24%**（ComboBox 聚焦衬底 / 下拉项选中） | §237/§247 | **AccentLow 0.6/0.4**，且聚焦**无边框** | 模板 `HighlightBackground` 的 Background = `ComboBoxBackgroundUnfocused` → AccentLow（A L666），聚焦把其 Opacity 动画到 1（A2 L10381-10385 + L10237）；边框笔刷 `ComboBoxBackgroundBorderBrushFocused` = **Transparent**（A L667）。令牌已删除，与 `selection_tint` 合并 |
| 浅叠 **15% / 25%**（`subtle_hover/press_tint`） | §471/§790/§864/§936「SubtleFillColorSecondary/Tertiary」 | **Secondary 5.9%（暗）/3.5%（亮）、Tertiary 3.9%/2.4%** | C（WinUI 2.x 字典）；消费状态见 `TabView_themeresources.xaml` L27/L52（悬停 Secondary）与 L26/L51（按压 Tertiary）——**按下比悬停更淡**。两个令牌已删除 |
| `base_medium_high` **0.9** | 旧注释「BaseMediumHigh 白 90%」 | **0.8** | `SystemBaseMediumHighColor` = `#CC…`（A L213/L4129） |
| `base_medium_low` **0.35** | §1216「35%（BaseMediumLow）」 | **0.40** | `SystemBaseMediumLowColor` = `#66…`（A L214/L4130）；Base* 族等距 20 点：`FF`/`CC`/`99`/`66`/`33` |
| 悬停边框 = `on_surface_variant` × **0.9** | §1139 | **`on_surface_variant` 实色（＝BaseMedium 60%）** | `TextControlBorderBrushPointerOver` → `SystemControlHighlightBaseMediumBrush` → `SystemBaseMediumColor`（A L855→L298→L212） |
| 弹窗收起 **0.26s** | 无 | **0.15s**（开 0.30 不变） | `CommandBarFlyoutCommandBar` 的 `OpeningStoryboard` 300ms / `ClosingStoryboard` 150ms（A2 L22093-22118，唯一显式 flyout 时长） |
| ProgressBar 暂停不透明度 **0.6（两方案）** | §4 | **暗 0.6 / 亮 1.0** | `ProgressBarIndicatorPauseOpacity`（A L71 / L2798） |
| 「对齐 UWP `ControlFastAnimationDuration`」 | presets 注释 | **该族属 WinUI，不属 UWP** | A/A′/A2/B 四份 XAML + `Windows.UI.Xaml.dll` 全 0 命中；只在 WinUI `resources.pri` 里（Normal 250 / Fast 167 / FastAnimationAfter 168 / Faster 83ms，**无 Slow**）。值 0.167 正确，归属错误 |

---

## §Ⅲ 取到的权威值（按族，暗/亮两套）

### 3.1 前景强度族（A L210-214 暗 / L4126-4130 亮）

| 令牌 | 暗 | 亮 | alpha |
|---|---|---|---|
| `SystemBaseHighColor` | `#FFFFFFFF` | `#FF000000` | 100% |
| `SystemBaseMediumHighColor` | `#CCFFFFFF` | `#CC000000` | **80%** |
| `SystemBaseMediumColor` | `#99FFFFFF` | `#99000000` | 60% |
| `SystemBaseMediumLowColor` | `#66FFFFFF` | `#66000000` | **40%** |
| `SystemBaseLowColor` | `#33FFFFFF` | `#33000000` | 20% |
| `SystemChromeBlackMediumLowColor` | `#66000000` | `#66000000` | 黑 40%（聚焦占位文本用） |

### 3.2 交互 / 列表族（A）

| 令牌 | 暗 | 亮 | 用途 |
|---|---|---|---|
| `SystemListLowColor`（→ `HighlightListLowBrush`） | `#19FFFFFF` | `#19000000` | **悬停**：AppBarButton、ListViewItem、ComboBoxItem、TreeViewItem |
| `SystemListMediumColor`（→ `HighlightListMediumBrush`） | `#33FFFFFF` | `#33000000` | **按下**：同上四类 |
| `SystemControlHighlightListAccentLowBrush` | accent **0.6** | accent **0.4** | 行/项 **Selected** |
| `SystemControlHighlightListAccentMediumBrush` | accent 0.8 | accent 0.6 | SelectedPointerOver |
| `SystemControlHighlightListAccentHighBrush` | accent 0.9 | accent 0.7 | SelectedPressed |
| `SystemControlHighlightAccentBrush` | accent 不透明 | 同 | 文本框**选区高亮**（本体 100%，非 35%）、ComboBox 聚焦边框候选 |
| `ListViewItemDisabledThemeOpacity` | 0.55 | 0.55 | 列表行禁用（OS 里唯一明写的禁用不透明度） |
| `ButtonBackgroundPointerOver/Pressed` | BaseLow 20% / BaseMediumLow 40% | 同 | **常规按钮**（底色本就可见） |
| `AppBarButtonBackgroundPointerOver/Pressed` | ListLow 10% / ListMedium 20% | 同 | **透明底按钮**（本库按钮族取此对照） |
| `BackButtonPointerOverBackgroundThemeBrush`（旧式） | `#21FFFFFF` 12.9% | `#3D000000` 23.9% | 8.1 时代遗留，供参考 |

### 3.3 ComboBox（A L662-680 + A2 模板）

| 键 | 笔刷 / 值 |
|---|---|
| `ComboBoxBackground` | `SystemControlBackgroundAltMediumLowBrush` |
| `…PointerOver` | `SystemControlPageBackgroundAltMediumBrush` |
| `…Pressed` | `SystemControlBackgroundListMediumBrush` |
| `…Unfocused`（＝聚焦衬底） | `SystemControlHighlightListAccentLowBrush`（accent 0.6/0.4），聚焦时 Opacity 0→1 |
| `…BackgroundBorderBrushFocused` | `SystemControlHighlightTransparentBrush`（**全透明**） |
| `ComboBoxItemBackgroundPointerOver / Pressed / Selected` | ListLow 10% / ListMedium 20% / AccentLow 0.6·0.4 |
| `ComboBoxBorderBrush` / `…PointerOver` | BaseMediumLow 40% / BaseMedium 60% |

### 3.4 文本控件（A L850-871）

| 键 | 笔刷 → 值 |
|---|---|
| `TextControlBackground` / `…PointerOver` | `SystemControlBackgroundAltMediumLowBrush` / `…AltMediumBrush` |
| `TextControlBackgroundFocused` | `SystemControlBackgroundChromeWhiteBrush` = **`#FFFFFFFF`**（暗色下「聚焦变白纸」，A2 还会把 `RequestedTheme` 切 Light） |
| `TextControlBorderBrush` | `ForegroundBaseMediumLow` = 40%，模板粗细 **2px** |
| `TextControlBorderBrushPointerOver` | `HighlightBaseMedium` = **60% 实色**（本库已按此更正） |
| `TextControlBorderBrushFocused` | `HighlightAccentBrush`（accent），粗细仍 **2px** |
| `TextControlPlaceholderForeground` / `…Focused` | `PageTextBaseMedium` = BaseMedium 60% / `PageTextChromeBlackMediumLow` = **黑 40%** |
| `TextControlSelectionHighlightColor` | `HighlightAccentBrush` = **accent 100%**（本库 35% 属有意偏离，见 CANON D10） |

### 3.5 动画与时长

| 项 | 一手值 | 源 |
|---|---|---|
| Flyout 开 / 关 | **300ms**（`0.1,0.9 0.2,1`）/ **150ms**（`0.7,0 1,0.5`） | A2 L22093-22118（CommandBarFlyout），唯一显式值 |
| ComboBox 遮罩 开 / 关 | **0.383s / 0.216s**（`0.1,0.9 0.2,1`） | A L10166-10177 —— 与本库 `DURATION_OVERLAY_OPEN/CLOSE` **逐字吻合** ✅ |
| 常规按钮状态切换 | **0ms**（`DiscreteObjectKeyFrame KeyTime="0"`，全是瞬切） | A L6278-6323 |
| 按钮按压动画 | 仅 `<PointerDownThemeAnimation TargetName="ContentPresenter"/>`，**无 Duration/像素参数**；官方语义为桌面「轻微缩小 + 倾斜」（写 `Projection`/`RenderTransform`），Phone 才是绕 Y 轴倾斜 | A L6306；A+A′ 共 64/195 个标签全无参 |
| ProgressBar 不确定 | 两套模型：**WinUI 2.8.6 = 2.0s**（`0.4,0,0.6,1`，B L3231-3241，与本库吻合）／**UWP OS = 3.917s 五圆点**（A L12117-12156） | 须二选一 |
| ProgressBar Paused/Error | **UWP OS = 0.25s**（A L12236-12247）／WinUI 2 = 0.167s | 本库 0.25s 对 UWP OS |
| ProgressRing | **UWP OS = 3.47s 周期、`-110°→585°`（净 +695°）、6 点 stagger 0.167s**（A L12406-12506）；WinUI 2 是 Lottie（无 XAML 值） | 本库 2.0s/900° 源自已丢弃的 WinUI 2 C++ 快照 |
| `Control*AnimationDuration` | **WinUI 专有**：Normal 250 / Fast 167 / FastAnimationAfter 168 / Faster 83ms（无 Slow） | 反查 §Ⅰ-E |
| Expander / ScrollBar 等 | `DURATION_EXPANDER_*` 与一手值一致；ScrollBar 0.1s/2s | A′ L605-632 |
| Coffee：`EntranceThemeTransition` stagger | **无一手支撑**（仅 5 处无参标签，官方 API 页不给默认值） | A L19483 等；可借 CalendarView 页面过渡 0.233/0.733s |

### 3.6 语义色（交叉验证）

`SystemErrorTextColor`：暗 `#FFF000`（黄！）/ 亮 `#C50500`（A L230/L4146）；
`SystemFillColorCritical`（WinUI）暗 `#FF99A4` / 亮 `#C42B1C` —— 与本库 `StatusColors` 一致 ✅。
**UWP 的 ProgressBar `Error` 视觉状态不换色，只把指示条 Opacity 归零**（A2 L12228-12235），
`SystemControlErrorTextForegroundBrush` 在 ProgressBar 模板里零引用。
**`SwipeItem` 的 Danger 底色在一手源中不存在**（危险语义靠调用方自赋 `SwipeItem.Background`）；
其 `PointerOver` 是**空状态**、`Pressed` = 中性 40%（A2 L29495 区段）。

---

## §Ⅳ 复核用命令（可复制）

```powershell
# A / A2：OS UWP 主题字典与模板（按版本号替换 10.0.26100.0）
$sdk = "C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic"
Select-String -Path "$sdk\themeresources.xaml" -Pattern 'x:Key="(SystemListLowColor|SystemListMediumColor|SystemBaseMediumHighColor|SystemBaseMediumLowColor)"'
Select-String -Path "$sdk\themeresources.xaml" -Pattern 'ResourceDictionary x:Key="(Default|Light|HighContrast)"'   # 字典边界
Select-String -Path "$sdk\themeresources.xaml" -Pattern 'x:Key="ListViewItemBackground|ComboBoxBackground|TextControlBorderBrush'

# C / C2：WinUI 2.x 令牌与控件字典（网络可用时）
$b = "https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/winui2/main"
Invoke-WebRequest "$b/dev/CommonStyles/Common_themeresources_any.xaml" -OutFile tokens.xaml
Invoke-WebRequest "$b/dev/TabView/TabView_themeresources.xaml"          -OutFile tabview.xaml

# E：反查某键是否属于 UWP 运行时
$dll = [System.IO.File]::ReadAllBytes("C:\Windows\System32\Windows.UI.Xaml.dll")
$txt = [System.Text.Encoding]::Unicode.GetString($dll)
$txt.Contains("ControlFastAnimationDuration")   # False → 不属 UWP
```

---

## §Ⅴ 仍未解决 / 需继续取的部分

1. **WinUI 3 现代令牌**（`TextFillColor*`/`SubtleFillColor*`/`ControlFillColor*`/`SolidBackgroundFillColor*`
   的 WinUI 3 取值）：本机 NuGet 只有 WinUI 2；需从 GitHub `main` 分支或 WinUI 3 NuGet
   （`Microsoft.WindowsAppSDK`）取。本次已用 C（WinUI 2.x 源码）覆盖同名键。
2. **强调色的绝对值**：`SystemAccentColor` 在一手源里**从不定义**（运行时按用户主题注入），
   故所有强调色族只能给「结构」（accent × alpha），给不出绝对值 —— 本库由 `Accent` 派生，
   这正是正确做法。
3. **WinUI 2 独占控件的键定义**（TabView 关闭键在 C2 已取到；TeachingTip 全色键、`ProgressBarErrorForegroundColor`、
   `TextControlBorderThemeThicknessFocused` 等）：要么继续读 C2 的对应文件，要么解包
   `Microsoft.UI.Xaml.2.8.appx → resources.pri`（XBF 二进制，需 XBF 解析器）。
4. **真实观感类**：`PointerDownThemeAnimation` 的时长/幅度、`EntranceThemeTransition` 的 stagger 数值 ——
   一手源里就是「系统预置、不可读」，只能记为待实测（登记于 `CANON_VS_TEMPORARY.md`）。
