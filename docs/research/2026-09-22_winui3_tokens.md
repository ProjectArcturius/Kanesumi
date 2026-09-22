# WinUI 3 现代令牌字典 —— 权威取数与 T3 裁定依据（2026-09-22 立）

> **存在理由**：`CANON_VS_TEMPORARY.md` 的 **T3** 登记「亮色中性色是借 WinUI 3 邻近令牌的值，
> 非直接对应」，而 `UWP_PRIMARY_SOURCES.md` §Ⅴ-1 记「WinUI 3 现代令牌本机取不到（NuGet 只有 WinUI 2）」。
> 本次把 **WinUI 3 令牌字典取到手并交叉验证**，T3 从此有权威值可依。
>
> 本文是**取证路径 + 取值对照**，供后续会话复核，不必再靠推测。
> 取证方式：只读研究 —— 未改任何源码，未跑 `cargo`。

---

## §0 取数口径与源的可信度（先读这段）

### 0.1 源的定位（重要：`main` 不在 `dev/` 了）

`UWP_PRIMARY_SOURCES.md` §Ⅰ-D 记的路径 `dev/CommonStyles/Common_themeresources_any.xaml` **已失效（404）**。
仓库结构已迁移，**WinUI 3 的实际位置是 `controls/dev/CommonStyles/`**：

| 源 | URL | 探测结果 |
|---|---|---|
| **WinUI 3 令牌字典（本次主源）** | `https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/controls/dev/CommonStyles/Common_themeresources_any.xaml` | ✅ 200，55804 字节 |
| WinUI 3 附属字典 | `https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/controls/dev/CommonStyles/Common_themeresources.xaml` | ✅ 200，4494 字节 |
| ~~旧记路径~~ | `https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/dev/CommonStyles/Common_themeresources_any.xaml` | ❌ **404** |
| WinUI 2.x（旧记路径仍有效） | `https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/winui2/main/dev/CommonStyles/Common_themeresources_any.xaml` | ✅ 200，58042 字节 |

**`main` 确实是 WinUI 3**（非 WinUI 2 的改名分支）：`controls/dev/` 下含 WinUI 3 独占控件
（`ItemsView` / `SelectorBar` / `AnnotatedScrollBar` / `TableView` / `Breadcrumb`），且存在
`*_perf2026.xaml` 这一近期性能工作产物；`winui2` 是独立分支。取数时 `main` HEAD =
`b7643857b661ab5aa8cfa5ff37ec2c2793696ddb`（2026-09-22T04:21:06Z）。

### 0.2 交叉验证：GitHub 源 == 实际发布的 Windows App SDK（这一步决定了源的权威性）

WinUI 2 与 WinUI 3 的**同名键取值完全相同**（见 §3），这本身就是「不该轻信」的信号，
故本次**额外下载了实际发布的 SDK 包做对照**：

```powershell
# 1) 元包只含 nuspec，需展开依赖
Invoke-WebRequest "https://www.nuget.org/api/v2/package/Microsoft.WindowsAppSDK/1.8.250907003" -OutFile wasdk.zip
# 2) 真正的 WinUI 运行时资源包（58.6 MB）
Invoke-WebRequest "https://www.nuget.org/api/v2/package/Microsoft.WindowsAppSDK.WinUI/1.8.250906003" -OutFile winui.zip
# 3) 编译进运行时的字典
#    lib\net6.0-windows10.0.17763.0\Microsoft.WinUI\Themes\generic.xaml   (36898 行, 3356257 字节)
```

**对照结果：GitHub `main` 的 249 个 `Color` 键（Default+Light+HighContrast）与发布版
`generic.xaml` 的对应键 —— `VALUE DIFFS = 0`。逐字节一致，无一处取值不同。**

发布版多出 197 个键是**旧族与无障碍族**（`System*Color` / `SystemReveal*` / `*AA*` / `Temporary*`），
GitHub 的 `Common_themeresources_any.xaml` 只承载 Fluent 现代令牌，这是**文件切分**而非版本差异。
GitHub `main` 反倒多出 12 个键（`CardBackgroundFillColorTertiary`、`ControlFillColorQuarternary`、
`SolidBackgroundFillColorQuinary/Senary` 等）—— 见 §5-1。

> **结论：`main/controls/dev/CommonStyles/Common_themeresources_any.xaml` 可作权威源使用。**
> 想同时要旧族（`SystemListLowColor` 等）时，改读发布版 `generic.xaml`。

### 0.3 字典边界与行号映射（照抄整行前先核对这里）

| 区段 | `Common_themeresources_any.xaml`（main） | 发布版 `generic.xaml`（1.8） |
|---|---|---|
| 暗色 | `x:Key="Default"` **L4–L207** | **L10–L2812** |
| 亮色 | `x:Key="Light"` **L208–L414** | **L5601–L8406** |
| 高对比 | `x:Key="HighContrast"` **L415–L600** | **L2813–L5600** |

⚠ **两个文件的字典顺序不同**（GitHub 是 Default→Light→HighContrast；发布版是 Default→HighContrast→Light）。
抄行号时勿跨文件照搬。

### 0.4 取值格式：XAML `#AARRGGBB`（alpha 在前）

```xml
<Color x:Key="TextFillColorSecondary">#C5FFFFFF</Color>
                                       ^^ = alpha, FF = white
```
- XAML：`#AARRGGBB`。本仓 `Color::from_hex` 是 **`RRGGBBAA`**，**必须换算**。
- 本文「亮」栏给「**不透明化后的十六进制 + 相对承载底的 alpha%**」两种读法。
- alpha% 换算：`C5 = 197 → 197/255 = 77.2%`。

### 0.5 本次落盘的原始件（可复核，`md5` 固定）

| 文件 | md5 | 字节 |
|---|---|---|
| `.win/_raw/winui3_any.xaml` | `4A292D4EF56B68624E1A1BAD231F0A49` | 55806 |
| `.win/_raw/winui2_any.xaml` | `AF240EB1906E910BAEA6D991071CB1D7` | 58044 |
| `.win/_raw/winui3_common.xaml` | `6E146E09B87E21871240A3DC3450C442` | 4496 |

---

## §1 结论先行 —— T3 该怎么取值

### 1.1 一句话结论

**WinUI 3 的暗/亮中性色不是一个「基准 + 偏移」的对称阶梯，而是两套方向相反的 6 档梯子。**

- **亮色梯子（越大越亮）**：`Secondary #EEEEEE` < `Base #F3F3F3` < `Tertiary #F9F9F9` <
  `Quarternary #FFFFFF`（`Quinary #FDFDFD`、`Senary #FFFFFF`）。
- **暗色梯子（越大越亮）**：`BaseAlt #0A0A0A` < `Secondary #1C1C1C` < `Base #202020` <
  `Tertiary #282828` < `Quarternary #2C2C2C` < `Quinary #333333` < `Senary #373737`。
- **两套梯子的「Base」都不在两端**：亮色 Base 靠上（下面还有 Secondary），暗色 Base 在偏下第三档。
  这不是对称设计 —— 亮色的 `Secondary` 比 `Base` **更暗**，暗色的 `Secondary` 也比 `Base` **更暗**。
  换言之 **`Secondary` 恒定是「比应用底更暗/更深」的那一档**，在两个方案里语义一致。

**T3 要的四个直接对应值（WinUI 3，权威）：**

| 本仓字段 | 应取的 WinUI 3 令牌 | **暗（Default）** | **亮（Light）** | 行号（main / 发布版） |
|---|---|---|---|---|
| `background` | `SolidBackgroundFillColorBase` | **`#202020`** | **`#F3F3F3`** | L68 / L204、L5795 |
| `surface` | `SolidBackgroundFillColorTertiary` | **`#282828`** | **`#F9F9F9`** | L70 / L2028、L7619 |
| `surface_variant` | `SolidBackgroundFillColorSecondary` ⚠ 见下 | `#1C1C1C` | `#EEEEEE` | L69 / L2027、L7618 |
| `on_background`/`on_surface` | `TextFillColorPrimary` | **`#FFFFFF`** | **`#E4000000`**（= 黑 89.4%） | L5 / L1966、L7557 |

⚠ **`surface_variant` 是本仓的语义，WinUI 3 里没有对应物** —— 本仓的 `surface_variant` 是「悬停 /
次级面板，**比 surface 更暗**」（暗 `#2E2E2E` > surface `#242424`，亮 `#F0F0F0` < surface `#FFFFFF`），
而 WinUI 3 的亮色 `Secondary` 确实比 `Base`/`Tertiary` 更暗，**但暗色的 `Secondary` 也比 `Base` 更暗** ——
本仓暗色的 `surface_variant` 方向与 WinUI 3 **相反**。故该项**不应**做映射（见 1.3）。

### 1.2 与本仓 `MetroColors::light` 的逐项差异清单

本仓现值（`kanesumi-core/src/colors.rs` L96–L115）vs WinUI 3 亮色直接对应值：

| 本仓字段 | 本仓现值（行号） | WinUI 3 对应令牌 | WinUI 3 值 | 差异 | 判定 |
|---|---|---|---|---|---|
| `background: #FAFAFA` | L99 | `SolidBackgroundFillColorBase` | `#F3F3F3` | **Δ = +7**（本仓更亮） | **应改**：本仓亮色底比 WinUI 更亮一档，接近 WinUI 的 `Tertiary #F9F9F9` |
| `surface: #FFFFFF` | L100 | `SolidBackgroundFillColorTertiary` | `#F9F9F9` | **Δ = −6**（本仓是纯白，WinUI 不是） | **应改**；若坚持纯白，则 `surface` 恰好等于 `SolidBackgroundFillColorQuarternary #FFFFFF`（L275） |
| `surface_variant: #F0F0F0` | L97 | 无同构对应（见 1.3） | — | — | **不做映射**；数值上最接近的是 `ControlAltFillColorQuarternary`（亮 `#C2F3F3F3` = `#F3F3F3` 76.1%，合成后 ≈ `#F4F4F4`），但那是「指针悬停态填充」的语义，与本仓「次级面板底」不同 |
| `divider: #D6D6D6` | L102 | `SolidBackgroundFillColorBaseAlt` | `#DADADA` | **Δ = +4** | **宜改**：`BaseAlt` 正是 UWP/WinUI 的「分隔/边界」档 |
| `on_background: #1A1A1A` | L107 | `TextFillColorPrimary` = `#E4000000` | 合成到 `#FAFAFA` 上 ≈ **`#1A1A1A`**（压在 `#F3F3F3` 上亦是 `#1A1A1A`） | **≈ 零差异** | ✅ **已是「直接对应」**：`E4 = 228 → 89.4%` 黑；`250×(1−0.894) = 26 = 0x1A`（`#FAFAFA` 底）、`243×(1−0.894) = 26 = 0x1A`（`#F3F3F3` 底） |
| `on_surface: #1A1A1A` | L108 | 同上 | 同上 | **≈ 零差异** | ✅ 同上 |
| `on_surface_variant: #5A5F66` | L109 | `TextFillColorTertiary` = `#72000000` | 合成到 `#F3F3F3` 上 ≈ **`#868686`**（对比度 3.28:1） | 本仓 6.43:1 vs WinUI 3.28:1 —— **本仓更保守** | **可保留**：本仓是「次级正文」需 ≥3.0 且实际要读，WinUI 的 `Tertiary` 是弱提示级。若追求严格对应应另立 `on_surface_tertiary` 字段 |

**差异汇总（可落地）**：`background` 与 `divider` 各差 4~7 级，`surface` 差 6 级；
`on_background` / `on_surface` **已经就是 WinUI 3 的直接对应值**。

### 1.3 具体建议（可直接落地）

**建议 A（推荐，最小改动 —— 3 行）**：把亮色三档向 WinUI 3 亮色梯子靠齐，其余不动。

```rust
// kanesumi-core/src/colors.rs  MetroColors::light()
background:     Color::from_hex(0xF3_F3_F3),  // was 0xFA_FA_FA — SolidBackgroundFillColorBase(Light) L272
surface:        Color::from_hex(0xF9_F9_F9),  // was 0xFF_FF_FF — SolidBackgroundFillColorTertiary(Light) L274
divider:        Color::from_hex(0xDA_DA_DA),  // was 0xD6_D6_D6 — SolidBackgroundFillColorBaseAlt(Light) L279
// surface_variant 留 #F0F0F0（WinUI 3 无同构令牌，见下）；on_background/on_surface/on_surface_variant 全部不动
```

- **T3 的登记到此可解除**：`background`/`surface`/`on_background`/`on_surface` 四项全部变成
  「**直接来自 WinUI 3 命名令牌**」，不再是「邻近令牌借值」。
- **代价须知情**：`background` 从 `#FAFAFA` 降到 `#F3F3F3` 后，`on_background #1A1A1A` 对比度
  由 **16.67:1** 变为约 **15.68:1**（略降但仍远超 4.5，属可接受）；`divider` 变暗一点，
  `surface`/`divider` 与 `background` 的相对层次基本不变（三档间距 `F3/F9/DA` vs 原 `FA/FF/D6`）。
- **`surface_variant` 必须是 `#F0F0F0`（保持）** —— 理由写进注释：本仓 `surface_variant` 的语义是
  「**比 surface 更暗**的一块可辨识面」（悬停底 / 下拉面板底），WinUI 3 的亮色 `Secondary`
  虽然也「比 Base 更暗」，但它是**应用底的回退色**，不是「叠在 surface 上的次级面」。
  两者形状相似、语义不同 —— 这正是 `ROADMAP.md` §Ⅴ-3「WinUI 的分层语义与本项目不同构」的**具体出处**，
  建议把这句话补进该裁定，并注明**只有 `surface_variant` 一项不适配，其余四项适配**（裁定原文偏严）。

**建议 B（若不想停下 —— 把 T3 降级为「部分对应」）**：只改 `on_*` 族的注释，声明
「`on_background`/`on_surface` = WinUI 3 `TextFillColorPrimary` 直接对应（`#E4000000`）；
`background`/`surface`/`divider` 为本仓取值」。

**建议 C（补一个暗色侧的既有缺口，与 T3 同批做）**：见 §1.4，暗色同族也**不是** WinUI 3 值。

### 1.4 顺带发现的同族缺口：**暗色**也不是 WinUI 3 值

T3 只登记了亮色，但对照后发现**暗色同样不是直接对应**（此前无人登记）：

| 本仓 `MetroColors::dark` | 本仓现值（行号） | WinUI 3 暗色对应令牌 | WinUI 3 值 | 说明 |
|---|---|---|---|---|
| `background` | `#1A1A1A`（L77） | `SolidBackgroundFillColorBase` | `#202020`（L68） | 本仓更暗；`#1A1A1A` 恰好落在 WinUI 的 `Secondary #1C1C1C` 与 `BaseAlt #0A0A0A` 之间，**不是任何一档** |
| `surface` | `#242424`（L78） | `SolidBackgroundFillColorTertiary` | `#282828`（L70） | Δ = +4 |
| `surface_variant` | `#2E2E2E`（L75） | 无同构（同 1.3 理由） | — | 数值恰等于 `SolidBackgroundFillColorQuarternary #2C2C2C`（L71）邻档与 `SystemFillColorSolidNeutralBackground #2E2E2E`（L87，**语义完全不同**，勿采） |
| `divider` | `#3A3A3A`（L80） | `SolidBackgroundFillColorBaseAlt` | `#0A0A0A`（L75） | ⚠ **暗色的 `BaseAlt` 是「比 Base 更黑」，亮色的是「比 Base 更灰」** —— 方向相反，**不可**照亮色那样映射 |
| `on_background`/`on_surface` | `#F0F0F0`（L85/L86） | `TextFillColorPrimary` | `#FFFFFF`（L5） | 本仓不是纯白 |

> **建议**：**不要**把暗色也硬塞进 WinUI 3 梯子 —— 暗色 `BaseAlt` 方向相反这一事实说明
> WinUI 的暗色梯子语义（`BaseAlt` = OLED 更黑档）与本仓不同构。
> 更稳的做法：**给 T1（暗色背景，已解除登记）补一行注**，写明「暗色四项亦非 WinUI 3 直接对应，
> 属本仓取值；仅亮色在建议 A 后成为直接对应」。**不要**为了对称而改暗色（会破坏 T1 已解除的稳定态）。

---

## §2 WinUI 3 令牌表（族 / 暗 / 亮 / 行号 / 原行）

源文件（下称 **W3**）：`https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/controls/dev/CommonStyles/Common_themeresources_any.xaml`
（`main` = `b7643857b661ab5aa8cfa5ff37ec2c2793696ddb`）。
「行号」列格式 `暗L…/亮L…`，均为该文件内行号；「原行」列为**照抄整行**（去掉行首缩进以外的内容一律不删）。

### 2.1 SolidBackgroundFillColor（不透明实底族 —— 本仓 background/surface 的正源）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | 原行（暗） |
|---|---|---|---|---|
| `SolidBackgroundFillColorBase` | `#202020` | `#F3F3F3` | 暗L68 / 亮L272 | `<Color x:Key="SolidBackgroundFillColorBase">#202020</Color>` |
| `SolidBackgroundFillColorSecondary` | `#1C1C1C` | `#EEEEEE` | 暗L69 / 亮L273 | `<Color x:Key="SolidBackgroundFillColorSecondary">#1C1C1C</Color>` |
| `SolidBackgroundFillColorTertiary` | `#282828` | `#F9F9F9` | 暗L70 / 亮L274 | `<Color x:Key="SolidBackgroundFillColorTertiary">#282828</Color>` |
| `SolidBackgroundFillColorQuarternary` | `#2C2C2C` | `#FFFFFF` | 暗L71 / 亮L275 | `<Color x:Key="SolidBackgroundFillColorQuarternary">#2C2C2C</Color>` |
| `SolidBackgroundFillColorQuinary` | `#333333` | `#FDFDFD` | 暗L72 / 亮L276 | `<Color x:Key="SolidBackgroundFillColorQuinary">#333333</Color>` |
| `SolidBackgroundFillColorSenary` | `#373737` | `#FFFFFF` | 暗L73 / 亮L277 | `<Color x:Key="SolidBackgroundFillColorSenary">#373737</Color>` |
| `SolidBackgroundFillColorBaseAlt` | `#0A0A0A` | `#DADADA` | 暗L75 / 亮L279 | `<Color x:Key="SolidBackgroundFillColorBaseAlt">#0A0A0A</Color>` |
| `SolidBackgroundFillColorTransparent` | `#00202020`（全透明） | `#00F3F3F3`（全透明） | 暗L74 / 亮L278 | `<Color x:Key="SolidBackgroundFillColorTransparent">#00202020</Color>` |

原行（亮，照抄）：

```
272:       <Color x:Key="SolidBackgroundFillColorBase">#F3F3F3</Color>
273:       <Color x:Key="SolidBackgroundFillColorSecondary">#EEEEEE</Color>
274:       <Color x:Key="SolidBackgroundFillColorTertiary">#F9F9F9</Color>
275:       <Color x:Key="SolidBackgroundFillColorQuarternary">#FFFFFF</Color>
276:       <Color x:Key="SolidBackgroundFillColorQuinary">#FDFDFD</Color>
277:       <Color x:Key="SolidBackgroundFillColorSenary">#FFFFFF</Color>
279:       <Color x:Key="SolidBackgroundFillColorBaseAlt">#DADADA</Color>
```

### 2.2 TextFillColor（前景 / 正文族）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | alpha（暗 / 亮） | 原行（暗，照抄） |
|---|---|---|---|---|---|
| `TextFillColorPrimary` | `#FFFFFF` | `#E4000000`（黑 89.4%） | 暗L5 / 亮L209 | 100% / 89.4% | `<Color x:Key="TextFillColorPrimary">#FFFFFF</Color>` |
| `TextFillColorSecondary` | `#C5FFFFFF`（白 77.3%） | `#9E000000`（黑 62.0%） | 暗L6 / 亮L210 | 77.3% / 62.0% | `<Color x:Key="TextFillColorSecondary">#C5FFFFFF</Color>` |
| `TextFillColorTertiary` | `#87FFFFFF`（白 52.9%） | `#72000000`（黑 44.7%） | 暗L7 / 亮L211 | 52.9% / 44.7% | `<Color x:Key="TextFillColorTertiary">#87FFFFFF</Color>` |
| `TextFillColorDisabled` | `#5DFFFFFF`（白 36.5%） | `#5C000000`（黑 36.1%） | 暗L8 / 亮L212 | 36.5% / 36.1% | `<Color x:Key="TextFillColorDisabled">#5DFFFFFF</Color>` |
| `TextFillColorInverse` | `#E4000000`（黑 89.4%） | `#FFFFFF` | 暗L9 / 亮L213 | 89.4% / 100% | `<Color x:Key="TextFillColorInverse">#E4000000</Color>` |
| `AccentTextFillColorDisabled` | `#5DFFFFFF` | `#5C000000` | 暗L10 / 亮L214 | 36.5% / 36.1% | `<Color x:Key="AccentTextFillColorDisabled">#5DFFFFFF</Color>` |

> **注（对 T16 / D10 有用）**：WinUI 3 的 `TextFillColorDisabled` **不是**一个统一的不透明度档 ——
> 暗色 **36.5%** 白、亮色 **36.1%** 黑，**两方案并不等值**（`5D=93` vs `5C=92`）。
> 且它**不随承载面**变化（是纯 alpha 令牌，不是「相对底色的百分比」）。

### 2.3 SubtleFillColor（浅叠族 —— 悬停 / 按压底）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | alpha（暗 / 亮） | 原行（暗，照抄） |
|---|---|---|---|---|---|
| `SubtleFillColorSecondary` | `#0FFFFFFF`（白 5.9%） | `#09000000`（黑 3.5%） | 暗L26 / 亮L230 | 5.9% / 3.5% | `<Color x:Key="SubtleFillColorSecondary">#0FFFFFFF</Color>` |
| `SubtleFillColorTertiary` | `#0AFFFFFF`（白 3.9%） | `#06000000`（黑 2.4%） | 暗L27 / 亮L231 | 3.9% / 2.4% | `<Color x:Key="SubtleFillColorTertiary">#0AFFFFFF</Color>` |
| `SubtleFillColorDisabled` | `#00FFFFFF`（全透明） | `#00FFFFFF`（全透明） | 暗L28 / 亮L232 | 0% / 0% | `<Color x:Key="SubtleFillColorDisabled">#00FFFFFF</Color>` |
| `SubtleFillColorTransparent` | `#00FFFFFF` | `#00FFFFFF` | 暗L25 / 亮L229 | 0% / 0% | `<Color x:Key="SubtleFillColorTransparent">#00FFFFFF</Color>` |

> ✅ **交叉验证通过**：WinUI 3 与 WinUI 2 的 Secondary/Tertiary **完全相同**（5.9%/3.9% 暗、
> 3.5%/2.4% 亮），与 `UWP_PRIMARY_SOURCES.md` §Ⅱ 已据 WinUI 2 更正的值**逐位吻合**
> （该次更正的结论在 WinUI 3 上同样成立，无需再改）。
> **重要语义**：`Tertiary`（3.9%）**比 `Secondary`（5.9%）更淡** —— 即**按下比悬停更淡**，
> 这与「按下应更重」的直觉相反，是一手源反复确认的反直觉项。

### 2.4 ControlFillColor（控件底族）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | alpha（暗 / 亮） | 原行（暗，照抄） |
|---|---|---|---|---|---|
| `ControlFillColorDefault` | `#0FFFFFFF`（白 5.9%） | `#B3FFFFFF`（白 70.2%） | 暗L15 / 亮L219 | 5.9% / 70.2% | `<Color x:Key="ControlFillColorDefault">#0FFFFFFF</Color>` |
| `ControlFillColorSecondary` | `#15FFFFFF`（白 8.2%） | `#80F9F9F9`（`#F9F9F9` 50.2%） | 暗L16 / 亮L220 | 8.2% / 50.2% | `<Color x:Key="ControlFillColorSecondary">#15FFFFFF</Color>` |
| `ControlFillColorTertiary` | `#08FFFFFF`（白 3.1%） | `#4DF9F9F9`（`#F9F9F9` 30.2%） | 暗L17 / 亮L221 | 3.1% / 30.2% | `<Color x:Key="ControlFillColorTertiary">#08FFFFFF</Color>` |
| `ControlFillColorQuarternary` | `#0FFFFFFF`（白 5.9%） | `#C2F3F3F3`（`#F3F3F3` 76.1%） | 暗L18 / 亮L222 | 5.9% / 76.1% | `<Color x:Key="ControlFillColorQuarternary">#0FFFFFFF</Color>` |
| `ControlFillColorDisabled` | `#0BFFFFFF`（白 4.3%） | `#4DF9F9F9`（`#F9F9F9` 30.2%） | 暗L19 / 亮L223 | 4.3% / 30.2% | `<Color x:Key="ControlFillColorDisabled">#0BFFFFFF</Color>` |
| `ControlFillColorInputActive` | `#B31E1E1E`（`#1E1E1E` 70.2%） | `#FFFFFF`（纯白） | 暗L21 / 亮L225 | 70.2% / 100% | `<Color x:Key="ControlFillColorInputActive">#B31E1E1E</Color>` |
| `ControlFillColorTransparent` | `#00FFFFFF` | `#00FFFFFF` | 暗L20 / 亮L224 | 0% / 0% | `<Color x:Key="ControlFillColorTransparent">#00FFFFFF</Color>` |

> **注意**：亮色的 Secondary / Tertiary / Disabled **不是 白色+alpha，而是 `#F9F9F9`+alpha** ——
> 底色参与了（`80F9F9F9`、`4DF9F9F9`）。若只抄 alpha 会偏白。这是本次唯一容易抄错的一族。

### 2.5 AccentFillColor / TextOnAccentFillColor（强调族 —— **无绝对值，只给结构**）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | 原行（暗，照抄） |
|---|---|---|---|---|
| `AccentFillColorDefault` | `{ThemeResource SystemAccentColorLight2}` | `{ThemeResource SystemAccentColorDark1}` | 暗L125 / 亮L329 | `<SolidColorBrush x:Key="AccentFillColorDefaultBrush" Color="{ThemeResource SystemAccentColorLight2}" />` |
| `AccentFillColorSecondary` | `SystemAccentColorLight2` @ `Opacity="0.9"` | `SystemAccentColorDark1` @ `0.9` | 暗L126 / 亮L330 | `<SolidColorBrush x:Key="AccentFillColorSecondaryBrush" Color="{ThemeResource SystemAccentColorLight2}" Opacity="0.9" />` |
| `AccentFillColorTertiary` | `SystemAccentColorLight2` @ `0.8` | `SystemAccentColorDark1` @ `0.8` | 暗L127 / 亮L331 | `<SolidColorBrush x:Key="AccentFillColorTertiaryBrush" Color="{ThemeResource SystemAccentColorLight2}" Opacity="0.8" />` |
| `AccentFillColorDisabled` | `#28FFFFFF`（白 15.7%） | `#37000000`（黑 21.6%） | 暗L38 / 亮L242 | `<Color x:Key="AccentFillColorDisabled">#28FFFFFF</Color>` |
| `AccentFillColorSelectedTextBackground` | `{ThemeResource SystemAccentColor}`（**原色**） | 同 | 暗L124 / 亮L328 | `<SolidColorBrush x:Key="AccentFillColorSelectedTextBackgroundBrush" Color="{ThemeResource SystemAccentColor}" />` |
| `TextOnAccentFillColorSelectedText` | `#FFFFFF` | `#FFFFFF` | 暗L11 / 亮L215 | `<Color x:Key="TextOnAccentFillColorSelectedText">#FFFFFF</Color>` |
| `TextOnAccentFillColorPrimary` | `#000000` | `#FFFFFF` | 暗L12 / 亮L216 | `<Color x:Key="TextOnAccentFillColorPrimary">#000000</Color>` |
| `TextOnAccentFillColorSecondary` | `#80000000`（黑 50.2%） | `#B3FFFFFF`（白 70.2%） | 暗L13 / 亮L217 | `<Color x:Key="TextOnAccentFillColorSecondary">#80000000</Color>` |
| `TextOnAccentFillColorDisabled` | `#87FFFFFF`（白 52.9%） | `#FFFFFF` | 暗L14 / 亮L218 | `<Color x:Key="TextOnAccentFillColorDisabled">#87FFFFFF</Color>` |

> ✅ **与 `UWP_PRIMARY_SOURCES.md` §Ⅴ-2 的结论一致**：`SystemAccentColor` 在字典里**从不定义绝对值**，
> 由运行时按用户主题注入。本仓 `Accent` 派生是正确做法。**强调色族取不到绝对值，这是源的固有性质，不是取数失败。**
>
> ⚠ **对 T20（强调色档位派生）有用**：WinUI 3 的强调档位**不是 V 缩放，而是命名档**：
> 暗色用 `Light2`（Default/Secondary/Tertiary 全用 `Light2`，只靠 `Opacity` 0.9/0.8 区分），
> 亮色用 `Dark1`。即 **WinUI 3 的「Secondary/Tertiary」= 同色不同 alpha，不是不同明度的色**。
> 这与本仓 `accent.rs` 的 `LIGHT_MIX`/`DARK_MIX`（向白/向黑 lerp）**也不同构** —— T20 的候选方案②
> （维持自定档位）因此更合理：WinUI 3 的做法本身就不是「档位 = 明度」。

### 2.6 Stroke 族（描边 / 分隔）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | 原行（暗，照抄） |
|---|---|---|---|---|
| `ControlStrokeColorDefault` | `#12FFFFFF`（白 7.1%） | `#0F000000`（黑 5.9%） | 暗L39 / 亮L243 | `<Color x:Key="ControlStrokeColorDefault">#12FFFFFF</Color>` |
| `ControlStrokeColorSecondary` | `#18FFFFFF`（白 9.4%） | `#29000000`（黑 16.1%） | 暗L40 / 亮L244 | `<Color x:Key="ControlStrokeColorSecondary">#18FFFFFF</Color>` |
| `ControlStrokeColorOnAccentDefault` | `#14FFFFFF`（白 7.8%） | `#14FFFFFF`（白 7.8%） | 暗L41 / 亮L245 | `<Color x:Key="ControlStrokeColorOnAccentDefault">#14FFFFFF</Color>` |
| `ControlStrokeColorOnAccentSecondary` | `#23000000`（黑 13.7%） | `#66000000`（黑 40%） | 暗L42 / 亮L246 | `<Color x:Key="ControlStrokeColorOnAccentSecondary">#23000000</Color>` |
| `ControlStrokeColorOnAccentTertiary` | `#37000000`（黑 21.6%） | `#37000000`（黑 21.6%） | 暗L43 / 亮L247 | `<Color x:Key="ControlStrokeColorOnAccentTertiary">#37000000</Color>` |
| `ControlStrokeColorOnAccentDisabled` | `#33000000`（黑 20%） | `#0F000000`（黑 5.9%） | 暗L44 / 亮L248 | `<Color x:Key="ControlStrokeColorOnAccentDisabled">#33000000</Color>` |
| `ControlStrokeColorForStrongFillWhenOnImage` | `#6B000000`（黑 42%） | `#59FFFFFF`（白 34.9%） | 暗L45 / 亮L249 | `<Color x:Key="ControlStrokeColorForStrongFillWhenOnImage">#6B000000</Color>` |
| **`ControlStrongStrokeColorDefault`** | **`#8BFFFFFF`（白 54.5%）** | **`#72000000`（黑 44.7%）** | 暗L48 / 亮L252 | `<Color x:Key="ControlStrongStrokeColorDefault">#8BFFFFFF</Color>` |
| `ControlStrongStrokeColorDisabled` | `#28FFFFFF`（白 15.7%） | `#37000000`（黑 21.6%） | 暗L49 / 亮L253 | `<Color x:Key="ControlStrongStrokeColorDisabled">#28FFFFFF</Color>` |
| `CardStrokeColorDefault` | `#19000000`（黑 9.8%） | `#0F000000`（黑 5.9%） | 暗L46 / 亮L250 | `<Color x:Key="CardStrokeColorDefault">#19000000</Color>` |
| `CardStrokeColorDefaultSolid` | `#1C1C1C` | `#EBEBEB` | 暗L47 / 亮L251 | `<Color x:Key="CardStrokeColorDefaultSolid">#1C1C1C</Color>` |
| `SurfaceStrokeColorDefault` | `#66757575`（`#757575` 40%） | `#66757575`（同） | 暗L50 / 亮L254 | `<Color x:Key="SurfaceStrokeColorDefault">#66757575</Color>` |
| `SurfaceStrokeColorFlyout` | `#33000000`（黑 20%） | `#0F000000`（黑 5.9%） | 暗L51 / 亮L255 | `<Color x:Key="SurfaceStrokeColorFlyout">#33000000</Color>` |
| `SurfaceStrokeColorInverse` | `#0F000000`（黑 5.9%） | `#15FFFFFF`（白 8.2%） | 暗L52 / 亮L256 | `<Color x:Key="SurfaceStrokeColorInverse">#0F000000</Color>` |
| `DividerStrokeColorDefault` | `#15FFFFFF`（白 8.2%） | `#0F000000`（黑 5.9%） | 暗L53 / 亮L257 | `<Color x:Key="DividerStrokeColorDefault">#15FFFFFF</Color>` |
| `FocusStrokeColorOuter` | `#FFFFFF`（纯白） | `#E4000000`（黑 89.4%） | 暗L54 / 亮L258 | `<Color x:Key="FocusStrokeColorOuter">#FFFFFF</Color>` |
| `FocusStrokeColorInner` | `#B3000000`（黑 70.2%） | `#B3FFFFFF`（白 70.2%） | 暗L55 / 亮L259 | `<Color x:Key="FocusStrokeColorInner">#B3000000</Color>` |

> ⚠ **`ControlStrongStrokeColorDefault` 命名与 `ControlStrongFillColorDefault` 同值但不同键**
> （暗色两者都是 `#8BFFFFFF`，L48 vs L22；亮色两者都是 `#72000000`，L252 vs L226）。
> 抄写时极易张冠李戴 —— **必须按 key 取，不能按值取**。

> ⚠ **对 T9（焦点描边）有用**：WinUI 3 的焦点环**与 accent 无关**，是「外纯白/内黑 70%」的
> **双环**（暗色：外 `#FFFFFF` + 内 `#B3000000`；亮色：外 `#E4000000` + 内 `#B3FFFFFF`）。
> 本仓 D2 已裁定「焦点描边由 accent 派生」—— 这是**有意偏离**，现在有了确切对照物可写入 D2 的理由栏。

### 2.7 CardBackgroundFillColor / LayerFillColor

| 令牌 | 暗（Default） | 亮（Light） | 行号 | 原行（暗，照抄） |
|---|---|---|---|---|
| `CardBackgroundFillColorDefault` | `#0DFFFFFF`（白 5.1%） | `#B3FFFFFF`（白 70.2%） | 暗L56 / 亮L260 | `<Color x:Key="CardBackgroundFillColorDefault">#0DFFFFFF</Color>` |
| `CardBackgroundFillColorSecondary` | `#08FFFFFF`（白 3.1%） | `#80F6F6F6`（`#F6F6F6` 50.2%） | 暗L57 / 亮L261 | `<Color x:Key="CardBackgroundFillColorSecondary">#08FFFFFF</Color>` |
| `CardBackgroundFillColorTertiary` | `#12FFFFFF`（白 7.1%） | `#FFFFFF` | 暗L58 / 亮L262 | `<Color x:Key="CardBackgroundFillColorTertiary">#12FFFFFF</Color>` |
| `LayerFillColorDefault` | `#4C3A3A3A`（`#3A3A3A` 29.8%） | `#80FFFFFF`（白 50.2%） | 暗L60 / 亮L264 | `<Color x:Key="LayerFillColorDefault">#4C3A3A3A</Color>` |
| `LayerFillColorAlt` | `#0DFFFFFF`（白 5.1%） | `#FFFFFF` | 暗L61 / 亮L265 | `<Color x:Key="LayerFillColorAlt">#0DFFFFFF</Color>` |
| `LayerOnAcrylicFillColorDefault` | `#09FFFFFF`（白 3.5%） | `#40FFFFFF`（白 25.1%） | 暗L62 / 亮L266 | `<Color x:Key="LayerOnAcrylicFillColorDefault">#09FFFFFF</Color>` |
| `LayerOnAccentAcrylicFillColorDefault` | `#09FFFFFF` | `#40FFFFFF` | 暗L63 / 亮L267 | `<Color x:Key="LayerOnAccentAcrylicFillColorDefault">#09FFFFFF</Color>` |
| `LayerOnMicaBaseAltFillColorDefault` | `#733A3A3A`（`#3A3A3A` 45.1%） | `#B3FFFFFF`（白 70.2%） | 暗L64 / 亮L268 | `<Color x:Key="LayerOnMicaBaseAltFillColorDefault">#733A3A3A</Color>` |
| `LayerOnMicaBaseAltFillColorSecondary` | `#0FFFFFFF`（白 5.9%） | `#0A000000`（黑 3.9%） | 暗L65 / 亮L269 | `<Color x:Key="LayerOnMicaBaseAltFillColorSecondary">#0FFFFFFF</Color>` |
| `LayerOnMicaBaseAltFillColorTertiary` | `#2C2C2C` | `#F9F9F9` | 暗L66 / 亮L270 | `<Color x:Key="LayerOnMicaBaseAltFillColorTertiary">#2C2C2C</Color>` |
| `LayerOnMicaBaseAltFillColorTransparent` | `#00FFFFFF` | `#00000000` | 暗L67 / 亮L271 | `<Color x:Key="LayerOnMicaBaseAltFillColorTransparent">#00FFFFFF</Color>` |
| `SmokeFillColorDefault` | `#4D000000`（黑 30.2%） | `#4D000000`（黑 30.2%） | 暗L59 / 亮L263 | `<Color x:Key="SmokeFillColorDefault">#4D000000</Color>` |

> ⚠ **`LayerOnMicaBaseAltFillColorTransparent` 两方案写法不同**（暗 `#00FFFFFF` / 亮 `#00000000`），
> **但都是全透明**，等效。抄写时勿误认为差异。

> **对 D3（遮罩 70%）有用**：WinUI 3 的 `SmokeFillColorDefault` = **黑 30.2%**（两方案同值），
> 比本仓 D3 记的「UWP dark 白 60%」轻得多，也**印证本仓 70% 是有意加重的偏离**（不是笔误）。

### 2.8 SystemFillColor（语义色族 —— **本仓 `StatusColors` 的正源**）

| 令牌 | 暗（Default） | 亮（Light） | 行号 | 原行（暗，照抄） |
|---|---|---|---|---|
| `SystemFillColorSuccess` | `#6CCB5F` | `#0F7B0F` | 暗L76 / 亮L280 | `<Color x:Key="SystemFillColorSuccess">#6CCB5F</Color>` |
| `SystemFillColorCaution` | `#FCE100` | `#9D5D00` | 暗L77 / 亮L281 | `<Color x:Key="SystemFillColorCaution">#FCE100</Color>` |
| `SystemFillColorCritical` | `#FF99A4` | `#C42B1C` | 暗L78 / 亮L282 | `<Color x:Key="SystemFillColorCritical">#FF99A4</Color>` |
| `SystemFillColorNeutral` | `#8BFFFFFF`（白 54.5%） | `#72000000`（黑 44.7%） | 暗L79 / 亮L283 | `<Color x:Key="SystemFillColorNeutral">#8BFFFFFF</Color>` |
| `SystemFillColorSolidNeutral` | `#9D9D9D` | `#8A8A8A` | 暗L80 / 亮L284 | `<Color x:Key="SystemFillColorSolidNeutral">#9D9D9D</Color>` |
| `SystemFillColorAttentionBackground` | `#08FFFFFF`（白 3.1%） | `#80F6F6F6`（`#F6F6F6` 50.2%） | 暗L81 / 亮L285 | `<Color x:Key="SystemFillColorAttentionBackground">#08FFFFFF</Color>` |
| `SystemFillColorSuccessBackground` | `#393D1B` | `#DFF6DD` | 暗L82 / 亮L286 | `<Color x:Key="SystemFillColorSuccessBackground">#393D1B</Color>` |
| `SystemFillColorCautionBackground` | `#433519` | `#FFF4CE` | 暗L83 / 亮L287 | `<Color x:Key="SystemFillColorCautionBackground">#433519</Color>` |
| `SystemFillColorCriticalBackground` | `#442726` | `#FDE7E9` | 暗L84 / 亮L288 | `<Color x:Key="SystemFillColorCriticalBackground">#442726</Color>` |
| `SystemFillColorNeutralBackground` | `#08FFFFFF`（白 3.1%） | `#06000000`（黑 2.4%） | 暗L85 / 亮L289 | `<Color x:Key="SystemFillColorNeutralBackground">#08FFFFFF</Color>` |
| `SystemFillColorSolidAttentionBackground` | `#2E2E2E` | `#F7F7F7` | 暗L86 / 亮L290 | `<Color x:Key="SystemFillColorSolidAttentionBackground">#2E2E2E</Color>` |
| `SystemFillColorSolidNeutralBackground` | `#2E2E2E` | `#F3F3F3` | 暗L87 / 亮L291 | `<Color x:Key="SystemFillColorSolidNeutralBackground">#2E2E2E</Color>` |
| `SystemFillColorAttention`（仅 brush） | `{ThemeResource SystemAccentColorLight2}` | `{ThemeResource SystemAccentColor}` | 暗L165 / 亮L369 | `<SolidColorBrush x:Key="SystemFillColorAttentionBrush" Color="{ThemeResource SystemAccentColorLight2}" />` |

> ✅ **交叉验证通过**：`SystemFillColorCritical` 暗 `#FF99A4` / 亮 `#C42B1C` 与
> `UWP_PRIMARY_SOURCES.md` §Ⅲ.6 所记一致 —— 本仓 `StatusColors` 取值正确（WinUI 2 与 WinUI 3 同值，见 §3）。
>
> ⚠ **`SystemFillColorAttentionBackground` 的亮色不是 alpha，而是 `#F6F6F6`+50.2%** —— 同 2.4 的陷阱。
>
> ✅ **对 D7 有用**：`SystemFillColorAttention` 绑定 accent，而 success/caution/critical 是独立字面量 ——
> 这正是维护者裁定「attention 归 accent、语义色另成一组」的一手依据，可直接引为 D7 的出处。

### 2.9 与承载面合成后的对比度（供选值参考，本次实测计算）

| 组合 | 对比度 |
|---|---|
| 本仓亮 `on_background #1A1A1A` on `#FAFAFA` | **16.67:1** |
| 本仓亮 `on_surface #1A1A1A` on `#FFFFFF` | **17.40:1** |
| 本仓亮 `on_surface_variant #5A5F66` on `#FFFFFF` | **6.43:1** |
| WinUI3 亮 `TextFillColorPrimary`（`#E4000000`）合成于 `#F3F3F3` | ≈ `#1A1A1A`，约 **15.6:1** |
| WinUI3 亮 `TextFillColorTertiary`（`#72000000`）合成于 `#F3F3F3` | ≈ `#868686`，**3.28:1** |

**若采纳 §1.3 建议 A**，`background → #F3F3F3` 后 `on_background #1A1A1A` 对比度约 **15.6:1**（仍远超 4.5），
`colors.rs` 的 `text_tokens_meet_wcag_contrast` 自检仍会通过。

---

## §3 WinUI 2 与 WinUI 3 的差异（只列有差异的）

源：`https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/winui2/main/dev/CommonStyles/Common_themeresources_any.xaml`
（下称 **W2**，607 行；字典 Default L9–L263、Light L264–L523、HighContrast L524–L732）。

### 3.1 结论：**同名键的取值 —— 零差异**

机械对照（Default+Light+HighContrast 三字典全部 `Color` 键）：

| 项 | 结果 |
|---|---|
| W2 `Color` 键数 | 158 |
| W3 `Color` 键数 | 166 |
| **取值不同的键** | **0（无）** |
| 仅 W3 有 | **8 个**（见 3.2） |
| 仅 W2 有 | **0 个** |

**即：在两代都存在的 158 个键上，W2 与 W3 逐字节相同。**
故 `UWP_PRIMARY_SOURCES.md` §Ⅱ 那些「依 WinUI 2 更正的取值」**在 WinUI 3 上原样成立，不需要二次更正**。

### 3.2 W3 相对 W2 新增的键（8 个，全部为梯子延伸，无一处改值）

| 令牌 | 暗（W3） | 亮（W3） | 行号 | 原行（暗，照抄，W3） |
|---|---|---|---|---|
| `SolidBackgroundFillColorQuinary` | `#333333` | `#FDFDFD` | 暗L72 / 亮L276 | `<Color x:Key="SolidBackgroundFillColorQuinary">#333333</Color>` |
| `SolidBackgroundFillColorSenary` | `#373737` | `#FFFFFF` | 暗L73 / 亮L277 | `<Color x:Key="SolidBackgroundFillColorSenary">#373737</Color>` |
| `ControlFillColorQuarternary` | `#0FFFFFFF` | `#C2F3F3F3` | 暗L18 / 亮L222 | `<Color x:Key="ControlFillColorQuarternary">#0FFFFFFF</Color>` |
| `CardBackgroundFillColorTertiary` | `#12FFFFFF` | `#FFFFFF` | 暗L58 / 亮L262 | `<Color x:Key="CardBackgroundFillColorTertiary">#12FFFFFF</Color>` |

（其余 4 个为同上四键在 HighContrast 字典里的占位 `#FF0000`，无信息量。）

### 3.3 差异的真正所在：**文件切分与键的归属**，不是取值

| 项 | WinUI 2 | WinUI 3 |
|---|---|---|
| 令牌字典路径 | `dev/CommonStyles/Common_themeresources_any.xaml` | **`controls/dev/CommonStyles/Common_themeresources_any.xaml`** |
| 令牌字典行数 | 607 | 607 |
| 现代令牌 `Color` 键数 | 158 | 166 |
| 附属字典 | `dev/CommonStyles/Common_themeresources.xaml` | `controls/dev/CommonStyles/Common_themeresources.xaml`（51 行，内容同构） |
| 旧族 `System*Color` | 在 `Common_themeresources_any.xaml` 内 | **不在该文件**；在发布版 `Themes/generic.xaml`（`SystemBaseHighColor` 发布版 L210 等） |
| 无障碍族 `*AA*` | 无 | **仅发布版有**（`ControlAAFillColorDefault` 发布版 L2163 等） |
| 强调档位绑定 | 暗 `SystemAccentColorLight2`（W2 L163）/ 亮 `Dark1`（W2 L420） | **完全相同**（W3 L125 / L329） |
| 焦点环 | 暗 外`#FFFFFF`/内`#B3000000`（W2 L76/L77）；亮 外`#E4000000`/内`#B3FFFFFF`（W2 L333/L334） | **完全相同**（W3 L54/L55、L258/L259） |
| 附属字典新增 | — | `SystemControlHighlightListAccentVeryHighBrush`（accent @0.9）、`SystemControlHighlightListAccentMediumLowBrush`（accent @0.75） |

> **给 `UWP_PRIMARY_SOURCES.md` 的修正**：§Ⅰ-D「WinUI 3 令牌字典 = 同仓 `main` 分支
> `dev/CommonStyles/Common_themeresources_any.xaml`」**路径已失效**，应改为
> `main` 分支 **`controls/dev/CommonStyles/Common_themeresources_any.xaml`**。
> §Ⅰ-C 的 WinUI 2 路径仍然有效，不必改。
>
> **给 §Ⅱ 表的一行补充**：「浅叠 Secondary/Tertiary」那条依据写的是「C（WinUI 2.x 字典）」，
> 现在可加注「**WinUI 3 同值，W3 L26/L27（暗）、L230/L231（亮）**」，把该结论从「WinUI 2 独证」升为「两代同证」。

---

## §4 UWP 字号梯度表 + 与本仓 `typography.rs` 的差异

### 4.1 先说一个反直觉但重要的结论：**UWP 的字号阶梯里没有行高**

一手核查结果：

| 检查项 | 结果 | 出处 |
|---|---|---|
| `themeresources.xaml` / `generic.xaml` 里 `x:Double x:Key="…FontSize"` | **有**，共 21 个唯一键（列出见 4.2） | 见下 |
| `themeresources.xaml` 里的 `LineHeight` | **仅 4 处，全在 InkToolbar 按钮**（`InkToolbarButtonContentSize`、`8`），**与正文排版无关** | `themeresources.xaml` L8046、L8057、L8089 |
| `themeresources.xaml` 里的 `LineStackingStrategy` | 仅 L7436、L7469（同为 `MaxHeight`） | 同上 |
| `generic.xaml` 里 TypeRamp 的 `LineHeight` 设置 | **一处都没有**（7 个 `*TextBlockStyle` 全无 `LineHeight`） | `generic.xaml` L15120–L15156 |
| 取而代之的机制 | `LineStackingStrategy="MaxHeight"` + `TextLineBounds="Full"` | `generic.xaml` L15126–L15127 |

**⇒ UWP 的行高不是资源，而是「字体度量的自然结果」** ——
`LineStackingStrategy=MaxHeight` 意为「行高取该行所有 inline 的最大高度」，配合
`TextLineBounds=Full`（用字体完整 em 框而非字形包围盒），
**行高由字体自身的 ascent+descent+lineGap 决定**。
故本节只能给出「**字号阶梯**」，行高列标为「未找到（由字体度量决定）」。

> **对本仓的意义**：本仓 `TextStyle` 里 `line_height` 是**必填字段**且逐档给了具体值 ——
> 这是**本仓自定**（为 CJK 紧凑行距），UWP 没有可对应的资源。`typography.rs` L52 的注释
> 「所有样式都定 line_height，避免默认行距让中文段落过松」**是准确的自我描述**，
> 但若要引 UWP 为依据，**必须写明「UWP 无行高资源」**，否则会被半年后的人当成漏抄。

### 4.2 UWP 字号资源全表（暗/亮无关 —— 这些键在三个字典中同值）

`themeresources.xaml` 的 Default 字典（L4–L1961）内的全部 `FontSize` 键
（亮色字典 L3940–L4025 为同值重复，不另列）：

| 键 | 值 | `themeresources.xaml` 行号 | 原行（照抄） |
|---|---|---|---|
| `ComboBoxArrowThemeFontSize` | `21` | L24 | `<x:Double x:Key="ComboBoxArrowThemeFontSize">21</x:Double>` |
| **`ControlContentThemeFontSize`** | **`14`** | **L28** | `<x:Double x:Key="ControlContentThemeFontSize">14</x:Double>` |
| `ContentControlFontSize` | `14` | L29 | `<x:Double x:Key="ContentControlFontSize">14</x:Double>` |
| `HubHeaderThemeFontSize` | `34` | L48 | `<x:Double x:Key="HubHeaderThemeFontSize">34</x:Double>` |
| `HubSectionHeaderThemeFontSize` | `20` | L49 | `<x:Double x:Key="HubSectionHeaderThemeFontSize">20</x:Double>` |
| `HubSectionHeaderSeeMoreThemeFontSize` | `14` | L50 | `<x:Double x:Key="HubSectionHeaderSeeMoreThemeFontSize">14</x:Double>` |
| `MTCMediaFontSize` | `12` | L55 | `<x:Double x:Key="MTCMediaFontSize">12</x:Double>` |
| `PivotHeaderItemFontSize` | `24` | L68 | `<x:Double x:Key="PivotHeaderItemFontSize">24</x:Double>` |
| `PivotTitleFontSize` | `14` | L70 | `<x:Double x:Key="PivotTitleFontSize">14</x:Double>` |
| `SearchBoxContentThemeFontSize` | `14` | L75 | `<x:Double x:Key="SearchBoxContentThemeFontSize">14</x:Double>` |
| `SemanticZoomButtonFontSize` | `4` | L81 | `<x:Double x:Key="SemanticZoomButtonFontSize">4</x:Double>` |
| `SettingsFlyoutHeaderThemeFontSize` | `26.667` | L82 | `<x:Double x:Key="SettingsFlyoutHeaderThemeFontSize">26.667</x:Double>` |
| `TextStyleLargeFontSize` | `18.14` | L98 | `<x:Double x:Key="TextStyleLargeFontSize">18.14</x:Double>` |
| `TextStyleExtraLargeFontSize` | `25.5` | L99 | `<x:Double x:Key="TextStyleExtraLargeFontSize">25.5</x:Double>` |
| `ToolTipContentThemeFontSize` | `12` | L101 | `<x:Double x:Key="ToolTipContentThemeFontSize">12</x:Double>` |
| `ListViewHeaderItemThemeFontSize` | `20` | L104 | `<x:Double x:Key="ListViewHeaderItemThemeFontSize">20</x:Double>` |
| `GridViewHeaderItemThemeFontSize` | `20` | L105 | `<x:Double x:Key="GridViewHeaderItemThemeFontSize">20</x:Double>` |
| `KeyTipContentThemeFontSize` | `12` | L109 | `<x:Double x:Key="KeyTipContentThemeFontSize">12</x:Double>` |
| `ScrollBarButtonArrowIconFontSize` | `8` | L614 | `<x:Double x:Key="ScrollBarButtonArrowIconFontSize">8</x:Double>` |
| `AutoSuggestBoxIconFontSize` | `12` | L1901 | `<x:Double x:Key="AutoSuggestBoxIconFontSize">12</x:Double>` |

### 4.3 UWP Typography 阶梯（TypeRamp）—— 在 `generic.xaml`，不在 `themeresources.xaml`

源：`C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\generic.xaml` L15120–L15156。

| 样式 | FontSize | FontWeight | 额外 | 行号 |
|---|---|---|---|---|
| `BaseTextBlockStyle`（基类） | **14** | `SemiBold` | `FontFamily=XamlAutoFontFamily`、`TextTrimming=None`、`TextWrapping=Wrap`、`LineStackingStrategy=MaxHeight`、`TextLineBounds=Full` | L15120–L15128 |
| `HeaderTextBlockStyle` | **46** | `Light` | `OpticalMarginAlignment=TrimSideBearings` | L15129–L15133 |
| `SubheaderTextBlockStyle` | **34** | `Light` | 同上 | L15134–L15138 |
| `TitleTextBlockStyle` | **24** | `SemiLight` | 同上 | L15139–L15143 |
| `SubtitleTextBlockStyle` | **20** | `Normal` | 同上 | L15144–L15148 |
| `BodyTextBlockStyle` | **14** | `Normal` | — | L15149–L15152 |
| `CaptionTextBlockStyle` | **12** | `Normal` | — | L15153–L15156 |

逐行照抄（关键几行）：

```
15120:     <Style x:Key="BaseTextBlockStyle" TargetType="TextBlock">
15121:         <Setter Property="FontFamily" Value="XamlAutoFontFamily" />
15122:         <Setter Property="FontWeight" Value="SemiBold" />
15123:         <Setter Property="FontSize" Value="14" />
15124:         <Setter Property="TextTrimming" Value="None" />
15125:         <Setter Property="TextWrapping" Value="Wrap" />
15126:         <Setter Property="LineStackingStrategy" Value="MaxHeight" />
15127:         <Setter Property="TextLineBounds" Value="Full" />
15130:         <Setter Property="FontSize" Value="46" />
15135:         <Setter Property="FontSize" Value="34" />
15141:         <Setter Property="FontSize" Value="24" />
15146:         <Setter Property="FontSize" Value="20" />
15151:         <Setter Property="FontSize" Value="14" />
15154:         <Setter Property="FontSize" Value="12" />
```

**⇒ UWP TypeRamp 是 6 档：`12 / 14 / 20 / 24 / 34 / 46`**（无 18、无 28、无 40、无 68）。
**行高：未找到**（无一档设 `LineHeight`）。

> ⚠ **`TextStyleLargeFontSize` 18.14 / `TextStyleExtraLargeFontSize` 25.5 不属于 TypeRamp** ——
> 它们是**旧 `TextBlock` 字号换算遗留**（18.14 ≈ 14pt，25.5 ≈ 20pt 的点→像素换算）。
> TypeRamp 用的是整数像素值（14/20/24…），**两套并存但互不引用**。不要把它们并进同一张表。

### 4.4 WinUI 3 的 TypeRamp（供对照，多出 4 档）

源：发布版 `Themes/generic.xaml`。

| 样式 / 键 | FontSize | FontWeight | 行号 |
|---|---|---|---|
| `CaptionTextBlockFontSize` / `CaptionTextBlockStyle` | **12** | `Normal` | L27353 / L11109 |
| `BodyTextBlockFontSize` / `BodyTextBlockStyle` | **14** | `Normal` | L27354 / L11106 |
| `BodyStrongTextBlockFontSize` / `BodyStrongTextBlockStyle` | **14** | `SemiBold` | L27380 / L27360 |
| `BodyLargeTextBlockFontSize` / `BodyLargeTextBlockStyle` | **18** | `Normal` | L27355 / L27363 |
| `SubtitleTextBlockFontSize` / `SubtitleTextBlockStyle` | **20** | `SemiBold` | L27356 / L11102 |
| `TitleTextBlockFontSize` / `TitleTextBlockStyle` | **28** | `SemiBold` | L27357 / L11098 |
| `TitleLargeTextBlockFontSize` / `TitleLargeTextBlockStyle` | **40** | `SemiBold` | L27358 / L27372 |
| `DisplayTextBlockFontSize` / `DisplayTextBlockStyle` | **68** | `SemiBold` | L27359 / L27376 |
| `HeaderTextBlockStyle` | **46** | `Light` | L11088 |
| `SubheaderTextBlockStyle` | **34** | `Light` | L11093 |

**WinUI 3 TypeRamp = `12 / 14 / 18 / 20 / 28 / 40 / 46 / 68`。**
相对 UWP：**`Title` 由 24 升到 28**、**新增 `BodyLarge 18` / `TitleLarge 40` / `Display 68`**、
**删掉了 `SemiLight` 字重**（`TitleTextBlockStyle` 不再设 `FontWeight`，回落基类 `SemiBold`）。
**行高同样是「未找到」**（`generic.xaml` L11085 同为 `LineStackingStrategy=MaxHeight`）。

> ⚠ 唯一的行高值在 WinUI 3 里也是**控件局部**：`BreadcrumbBar` 项与 `ComboBox` Header 的
> `LineHeight="20"`（发布版 L22465、L22470、L34638），**不是排版梯度**。

### 4.5 与本仓 `kanesumi-core/src/typography.rs` 的差异

本仓现值（`typography.rs` L71–L87 `MetroTypography::metro()`）：

| 本仓字段 | size | line_height | weight | 行号 |
|---|---|---|---|---|
| `page_heading` | 34.0 | 42.0 | `Normal` | L74 |
| `title` | 22.0 | 28.0 | `Normal` | L75 |
| `body` | 15.0 | 22.0 | `Normal` | L76 |
| `caption` | 13.0 | 18.0 | `Normal` | L77 |
| `label` | 11.0 | 14.0 | `Normal` | L78 |
| `headline_medium` | 28.0 | 36.0 | `Normal` | L79 |
| `title_large` | 22.0 | 28.0 | `Normal` | L80 |
| `title_medium` | 16.0 | 24.0 | `Medium` | L81 |
| `body_large` | 16.0 | 24.0 | `Normal` | L82 |
| `body_medium` | 14.0 | 20.0 | `Normal` | L83 |
| `body_small` | 12.0 | 16.0 | `Normal` | L84 |

**差异清单（对照 UWP TypeRamp `12/14/20/24/34/46`）**：

| # | 差异 | 本仓 | UWP | 判定 |
|---|---|---|---|---|
| 1 | `body` = **15** | 15.0 | **14**（`BodyTextBlockStyle` L15151）或 **20**（`Subtitle`） | ⚠ **15 不在 UWP 阶梯上** —— 介于 `Body 14` 与 `Subtitle 20` 之间，**无一手依据**（疑似为 CJK 观感自调）。建议登记（见下） |
| 2 | `caption` = **13** | 13.0 | **12**（`CaptionTextBlockStyle` L15154） | ⚠ **13 不在 UWP 阶梯上**（差 1）。无强理由则应回 12 |
| 3 | `label` = **11** | 11.0 | **12**（最小档，无 11） | ⚠ **11 不在 UWP 阶梯上**。UWP 最小正文档是 12 |
| 4 | `title` = **22** | 22.0 | **24**（`TitleTextBlockStyle` L15141） | ⚠ **22 vs 24**；本仓 `title_large` 也是 22，两者同值同义（重复） |
| 5 | `page_heading` = **34** | 34.0 | **34**（`SubheaderTextBlockStyle` L15135） | ✅ **精确对应**（仅命名不同：UWP 叫 Subheader，本仓叫 page_heading） |
| 6 | `body_small` = **12** | 12.0 | **12**（`Caption`） | ✅ 对应（但本仓 `caption`=13 与之重复命名） |
| 7 | `body_medium` = **14** | 14.0 | **14**（`Body`） | ✅ **精确对应** |
| 8 | `headline_medium` = **28** | 28.0 | UWP **无 28**；WinUI 3 `Title 28` | ⚠ 本仓这套是 **M3 命名**（L61–L68 注释已自认），28 来自 **WinUI 3**，不是 UWP |
| 9 | `title_medium` = **16** | 16.0 | UWP **无 16** | ⚠ 同上，M3 命名 |
| 10 | **字重**：全部 `Normal`/`Medium`，**无 `SemiLight`、无 `Light`** | — | UWP TypeRamp 的字重是**阶梯的一部分**：`Header/Subheader = Light`、`Title = SemiLight`、`Base = SemiBold` | ⚠ **本仓丢了 UWP 最重要的排版特征**：大字号档在 UWP 里是**更细**的字重（Light/SemiLight），本仓大标题用 `Normal` 会显得比 UWP 更「重」。且 T7 已登记「字重是死字段」—— 两问题叠加 |
| 11 | **行高**：本仓逐档给（1.23~1.29 倍） | `42/34=1.24`、`28/22=1.27`、`22/15=1.47`、`18/13=1.38`、`14/11=1.27` | UWP **无行高资源**（`MaxHeight` + 字体度量） | ✅ **本仓自定且合理**（`line_heights_tight_for_cjk` 测试 L108–L113 已守）；但**不得声称来自 UWP** |
| 12 | **没有 Typography 阶梯的顶层档** | 最大 34 | UWP `Header 46`；WinUI 3 `TitleLarge 40` / `Display 68` | ⚠ 本仓无 `46` 及以上档；若需要「页面大标题」将被迫借用 `page_heading 34` |
| 13 | `character_spacing` | `letter_spacing_em` 字段（L20），默认 0 | UWP：`PivotHeaderItemCharacterSpacing = -25`（L110，唯一负值）、`FlyoutPickerTitleTextBlockStyle` 的 `CharacterSpacing=15`（`generic.xaml` L16127） | ✅ 本仓机制正确（注释 L13 已写明「UWP CharacterSpacing/1000」）；**缺 `PivotHeaderItem` 的 −0.025 预设**（本仓注释里已提到 V16，但 `metro()` 未设） |
| 14 | **字体族** | 未在 `typography.rs` 中固定（由 harness 查找） | `ContentControlThemeFontFamily = XamlAutoFontFamily`（`themeresources.xaml` L5）、`SymbolThemeFontFamily = Segoe Fluent Icons`（L24，高对比字典另见）、正文族 `XamlAutoFontFamily` | ⚠ 本仓用思源黑体（`AGENTS.md`：`FontWeight` 注释 L1）—— **有意偏离**，无需改，但应写明「UWP 用 Segoe UI 变体 / `XamlAutoFontFamily` 解析」 |

**建议（本轮可落地，最小）**：

1. **把 `body 15 → 14`、`caption 13 → 12`、`label 11 → 12`** 三项向 UWP TypeRamp 靠齐 ——
   代价是正文小 1px、需一次视觉确认（CJK 在小字号下可读性会略降，故**建议先只改 `caption`/`label`**，
   `body` 留 15 并**登记为有意偏离**）。
2. **在 `typography.rs` 补一段注释**，写明「UWP TypeRamp 无行高资源（`MaxHeight` + 字体度量），
   本仓 `line_height` 为 CJK 自定；UWP 阶梯为 `12/14/20/24/34/46`，WinUI 3 为 `12/14/18/20/28/40/46/68`」。
   这一句能省掉未来至少一次返工。
3. **`title` 的 22 与 `title_large` 的 22 重复**（`typography.rs` L75 / L80，两者 size 与 line_height 全同）——
   建议 `title` 改 **24**（对齐 UWP `TitleTextBlockStyle`），让 `title_large` 保持 22（M3 命名）或直接删除其一。
4. **大字号档字的字重**：UWP 用 `Light`/`SemiLight`。在 T7（字重是死字段）修好之前，
   改字重无视觉意义，**建议与 T7 合并处理**，不要单独改。

---

## §5 未找到 / 存疑

1. **W3 独有而发布版没有的 4 个键**（`CardBackgroundFillColorTertiary`、`ControlFillColorQuarternary`、
   `SolidBackgroundFillColorQuinary`、`SolidBackgroundFillColorSenary`，格暗/亮各一）——
   GitHub `main` 的 `Common_themeresources_any.xaml` 里有定义（W3 暗 L18/L58/L72/L73、亮 L222/L262/L276/L277），
   **但发布的 Windows App SDK 1.8 `generic.xaml` 在 Default/Light 字典里搜不到同名键**。
   两处可能：① 发布版把它们编进了别的资源文件（XBF/`.pri`），② `main` 领先于已发布的 1.8。
   **未确认，存疑** —— 使用这 4 个键时应以 GitHub `main` 为准并**自行标注「未在 1.8 发布版中核到」**。
   （**注意**：其余 245 个键已逐字节核对通过，这 4 个不影响 §1/§2 的任何结论。）

2. **发布版独有的旧族**（197 个键）—— `System*Color`、`SystemReveal*`、`*AA*`、`Temporary*` 等。
   它们**不在** GitHub 的 `Common_themeresources_any.xaml` 中，只在发布版 `Themes/generic.xaml`。
   本次已抄录其 Default/Light 行号与取值（见 §0.3 区段表可自行复取），但**未逐条列表**（超出本次范围）。

3. **`TemporaryTextFillColorDisabled` 的语义未确认** —— 发布版 Default `#5DFEFEFE`（L2182）、
   Light `#5C010101`（L4970/L7776）。“Temporary” 前缀暗示过渡用，但**未找到引用点**，不猜。

4. **WinUI 3 的 TypeRamp 行高** —— **未找到**（`generic.xaml` 里 8 个 `*TextBlockStyle` 全无 `LineHeight`，
   与 UWP 同）。行高由 `LineStackingStrategy=MaxHeight` + 字体度量决定。
   若要精确复刻 WinUI 的行高，**必须实测字体度量**，无 XAML 可抄。

5. **UWP 的 Typography 阶梯是否还有未列出的档** —— 本次覆盖 `generic.xaml` 中全部
   `<Style x:Key="*TextBlockStyle">`（**共 11 个**），其中构成阶梯的是 7 个（L15120–L15156）；
   其余 4 个为控件局部（`MediaTextBlockStyle` L15358、`FlyoutPickerTitleTextBlockStyle` L16122、
   `NavigationViewItemHeaderTextStyle` L26603 等）。
   **未逐条展开控件局部样式**（它们不属阶梯）。

6. **`SolidBackgroundFillColorSecondary` 是否真是本仓 `surface_variant` 的对应物** —— **存疑**。
   本仓 `surface_variant` 的语义（「叠在 surface 上的次级面」）在 WinUI 3 里更接近
   `CardBackgroundFillColorDefault` 或 `LayerFillColorAlt`（都是「白/黑 + 低 alpha 的叠层」），
   而不是 `SolidBackgroundFillColor*`（不透明实底）。**§1.3 已据此建议不做映射**，
   但若要严格对齐，**需要维护者先裁定 `surface_variant` 到底是「实底第二档」还是「叠层」** ——
   这是本仓语义层的既有模糊点，本次不代为裁定。

7. **两代 `Common_themeresources.xaml`（附属字典）未做机械 diff** —— 只做了人工比对（结论：同构，见 §3.3）。
   W3 版 51 行含 `TextControlBorderThemeThickness=1` / `…Focused=1,1,1,2` / `TextControlThemePadding=10,5,6,6`
   等（W3 `Common_themeresources.xaml` L10–L12）。**未取 WinUI 2 版做逐行对照**（超出本次范围）。

---

## §6 复核用命令（可复制）

```powershell
# ① WinUI 3 令牌字典（主源；注意路径已不在 dev/）
$b = "https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/controls/dev/CommonStyles"
Invoke-WebRequest "$b/Common_themeresources_any.xaml" -OutFile winui3_any.xaml   # 55806 bytes

# ② 确认 dev/ 下老路径已失效（应为 404）
try { Invoke-WebRequest "https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/dev/CommonStyles/Common_themeresources_any.xaml" } catch { "404 as expected" }

# ③ 列目录确认 main 的结构（GitHub API；注意 ? 需转义）
$h = @{ "User-Agent"="dsh-research" }
(Invoke-WebRequest "https://api.github.com/repos/microsoft/microsoft-ui-xaml/contents/controls/dev/CommonStyles?ref=main" -Headers $h).Content | ConvertFrom-Json | Select-Object type,size,name

# ④ 交叉验证：取实际发布的 Windows App SDK WinUI 资源（58.6 MB）
Invoke-WebRequest "https://www.nuget.org/api/v2/package/Microsoft.WindowsAppSDK.WinUI/1.8.250906003" -OutFile winui.zip
Expand-Archive winui.zip -DestinationPath winuipkg
#   字典位置：lib\net6.0-windows10.0.17763.0\Microsoft.WinUI\Themes\generic.xaml （36898 行）
#   Default L10-2812 / HighContrast L2813-5600 / Light L5601-8406

# ⑤ 查 T3 四个目标值
Select-String -Path winui3_any.xaml -Pattern 'x:Key="(SolidBackgroundFillColor(Base|Secondary|Tertiary)|TextFillColorPrimary)"'
#   -> Default L68/L69/L70/L5 ; Light L272/L273/L274/L209

# ⑥ 查 UWP 字号（本机 SDK）
$sdk = "C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic"
Select-String "$sdk\themeresources.xaml" -Pattern 'x:Double x:Key="[^"]*FontSize[^"]*"'
Select-String "$sdk\generic.xaml" -Pattern '<Style x:Key="(Base|Header|Subheader|Title|Subtitle|Body|Caption)TextBlockStyle"'
#   行高资源：搜 LineHeight —— 只有 InkToolbar 局部（L8046/L8057/L8089），TypeRamp 无
Select-String "$sdk\generic.xaml" -Pattern 'LineHeight|LineStackingStrategy'

# ⑦ 暗/亮字典边界（两个文件的顺序不同，勿混）
Select-String -Path winui3_any.xaml -Pattern 'ResourceDictionary x:Key="(Default|Light|HighContrast)"'
#   -> L4 / L208 / L415（Default / Light / HighContrast）
```
