# 浅色调色板审计（Wave A4 · 2026-10-01）

> **为什么**：系统主题链已接通（Ether `docs/INTEGRATION_PLAN.md` Wave A），切到浅色后所有元素树控件
> 都改用 `MetroColors::light`。但浅色档从未被系统核对 —— 无头实测：浅色下次级按钮（如「重置」）
> 白底压近白页面几乎不可见。本文按 UWP 一手源逐控件对照笔刷，给出改前 / 改后值与样张。
>
> 一手源与取数纪律见 `docs/UWP_PRIMARY_SOURCES.md` §Ⅰ：每个值 `文件:行号` + 原行；
> XAML `#AARRGGBB` → 本库 **RRGGBBAA**；相对承载色再给 alpha 百分比。

## §Ⅰ 取数方法

- 一手源 A：`C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\10.0.26100.0\Generic\themeresources.xaml`
  —— `Default`（暗）L4-1961、`Light`（亮）**L3920-5878**。
- 用脚本解析三个 ThemeDictionary，把 `{StaticResource X}` / `{ThemeResource X}` 解析到字面量
  （不猜、不允许未解析）。基色常量族（`SystemBase*Color` / `SystemList*Color` / `SystemAlt*Color`）：
  暗 L210-229、亮 L4126-4145。
- 本库数值以 `kanesumi-core/src/colors.rs` 的 `MetroColors::{dark,light}` 为准。
- 样张由 `kanesumi-gallery/examples/theme_sheet.rs` 无窗渲染（见 §Ⅳ）。

## §Ⅱ 结论摘要

1. **根因**：标准控件的「控件填充」被实现成 `surface`（面板色）。浅色下 `surface = #FFFFFF`
   与页面 `background = #FAFAFA` 几乎同色 —— 按钮、下拉按钮、分体按钮的底因此隐形；文本框、
   数字框、勾选框 / 单选钮 / 开关的描边则取了偏淡的 `on_surface_variant`。
   UWP 的控件底/描边不是页面色，而是 `SystemControlBackground*` / `SystemControlForeground*`
   的 **Base* 叠加档**。故新增三个语义字段（§Ⅲ）并改控件取色。
2. 暗色档**逐像素未变**：`before_dark.png` 与 `after_dark.png` 的 MD5 相同
   （`0ca6f96c…`），证明本次未触碰深色观感。
3. 文本框静止描边旧值 `divider`（#D6D6D6）过淡，改取 `control_stroke`（亮 = 40% 黑）。
4. 有意偏离 / 待裁定项登记于 §Ⅵ 与 `CANON_VS_TEMPORARY.md`。

## §Ⅲ 新增语义字段（`MetroColors`）

| 字段 | 语义 | 一手源笔刷 → 色 | 暗（本库） | 亮（本库） |
|---|---|---|---|---|
| `control_fill` | 标准按钮填充 | `SystemControlBackgroundBaseLowBrush` → `SystemBaseLowColor`（L211/L4127） | `#242424`（保留旧 `surface` 观感） | `#00000033`（BaseLow 20% 黑） |
| `control_stroke` | 交互控件静止描边 | `SystemControlForegroundBaseMediumLowBrush` → `SystemBaseMediumLowColor`（L214/L4130） | `#3A3A3A`（保留旧 `divider` 观感） | `#00000066`（BaseMediumLow 40% 黑） |
| `control_stroke_strong` | 勾选 / 单选 / 开关外圈 | `SystemControlForegroundBaseMediumHighBrush` → `SystemBaseMediumHighColor`（L213/L4129） | `#9AA0A6`（保留旧 `on_surface_variant` 观感） | `#000000CC`（BaseMediumHigh 80% 黑） |

**暗色档为何不取一手源真值**：任务约束「深色档视觉不得因此变化」。一手源证明暗色按钮底同样是
BaseLow（`#33FFFFFF`，叠加在页面上约得 `#4A4A4A`）、勾选框外圈是 BaseMediumHigh（`#CCFFFFFF`）
—— 采用会把深色观感整体提亮，属回归。故暗色档保留旧观感，真值差异在此单列，**未顺手改**：

| 字段 | 暗色一手源真值 | 本库暗色现值 | 影响 |
|---|---|---|---|
| `control_fill` | `#33FFFFFF`（BaseLow 白 20%） | `#242424` | 按钮底应比现值亮 |
| `control_stroke` | `#66FFFFFF`（BaseMediumLow 白 40%） | `#3A3A3A` | 文本框边框应比现值亮 |
| `control_stroke_strong` | `#CCFFFFFF`（BaseMediumHigh 白 80%） | `#9AA0A6` | 勾选框外圈应比现值亮 |

## §Ⅳ 样张

`docs/research/light_palette/`（960×1720，`--snapshot` 1×，PNG ≤ 100 KiB）：

| 文件 | 说明 |
|---|---|
| `before_dark.png` / `before_light.png` | 改动前（旧调色板） |
| `after_dark.png` / `after_light.png` | 改动后 |

生成：
```bash
cargo run -p kanesumi-gallery --example theme_sheet -- --scheme light --snapshot out.png 1
cargo run -p kanesumi-gallery --example theme_sheet -- --scheme dark  --snapshot out.png 1
```

> ⚠ 本轮 Windows 会话的读图服务不可用（SenseNova 引擎返回 500 / 连接被重置），故未做「读图」式
> 主观巡检；改为**逐控件采样渲染像素 + `Tree::painted` 笔刷值核对**（见 §Ⅴ 的实测列）。

## §Ⅴ 逐控件对照表（浅色为主，暗色真值单列）

「换算值」为 XAML `#AARRGGBB` → 本库 RRGGBBAA；`accent` 表示由 `Accent` 派生（一手源不定义绝对值）。

### 5.1 Button（`button.rs`）+ 按钮族（`drop_down_button.rs` / `split_button.rs`）

| 状态 | 一手源键 | 文件:行 原行 | 换算值 | 本库改前 | 本库改后 |
|---|---|---|---|---|---|
| Normal | `ButtonBackground` | A L4269 `ResourceKey="SystemControlBackgroundBaseLowBrush"` | `#00000033` | `surface` `#FFFFFF` | `control_fill` `#00000033` |
| PointerOver | `ButtonBackgroundPointerOver` | A L4270 同上 | `#00000033` | surface + hover tint | control_fill + hover tint |
| Pressed | `ButtonBackgroundPressed` | A L4271 `…BackgroundBaseMediumLowBrush` | `#00000066` | surface + press tint | control_fill + press tint |
| Disabled | `ButtonBackgroundDisabled` | A L4272 BaseLow | `#00000033` | surface ×0.38 | control_fill ×0.38 |
| Foreground | `ButtonForeground` | A L4273 `…ForegroundBaseHighBrush` | `#FF000000` | `on_surface` `#1A1A1A`（保留） | 同左 |
| Border | `ButtonBorderBrush` | A L4277 `Transparent` | 透明 | 无 | 无（UWP 常规按钮无边框） |

实测（样张 `MetroButton@(24,108,46,33)`）：改前填充 = 页面色 `#FFFFFF`（在 `#FAFAFA` 页上不可见），
改后 = 线性合成后 `#E3E3E3`。Accent 按钮两版一致（强调色本就可见）。

### 5.2 CheckBox / RadioButton / ToggleSwitch（未选中 / 关闭态的外圈）

| 控件 | 一手源键 | 文件:行 原行 | 换算值 | 本库改前 | 本库改后 |
|---|---|---|---|---|---|
| CheckBox 未选中 | `CheckBoxCheckBackgroundStrokeUnchecked` | A L4353 `…ForegroundBaseMediumHighBrush` | `#000000CC` | `on_surface_variant` `#5A5F66` | `control_stroke_strong` `#000000CC` |
| RadioButton 未选中 | `RadioButtonOuterEllipseStroke` | A L4293 同上 | `#000000CC` | `on_surface_variant` | `control_stroke_strong` |
| ToggleSwitch 关闭轨道 | `ToggleSwitchStrokeOff` | A L4425 同上 | `#000000CC` | `on_surface_variant` | `control_stroke_strong` |

实测（RadioButton `@(24,257)` 外圈像素）：改前 `#5A5F66`（约 65% 灰），改后 `#797979`（80% 黑线性合成）。

### 5.3 TextBox / PasswordBox / NumberBox（静止边框）

| 控件 | 一手源键 | 文件:行 原行 | 换算值 | 本库改前 | 本库改后 |
|---|---|---|---|---|---|
| 静止边框 | `TextControlBorderBrush` | A L4770 `…ForegroundBaseMediumLowBrush` | `#00000066` | `divider` `#D6D6D6` | `control_stroke` `#00000066` |
| 静止底 | `TextControlBackground` | A L4766 `SystemControlBackgroundAltMediumLowBrush` | `#66FFFFFF` | `surface` `#FFFFFF` | 同左（白底正确） |
| 悬停边框 | `TextControlBorderBrushPointerOver` | A L4771 `…HighlightBaseMediumBrush` | `#99000000` | `on_surface_variant` | 同左（近似） |
| 聚焦边框 | `TextControlBorderBrushFocused` | A L4772 `…HighlightAccentBrush` | accent | `focus_stroke` | 同左 |

实测（`MetroTextBox@(24,445)` 左边框像素）：改前 `#D6D6D6`，改后 `#CBCBCB`。
PasswordBox 走 `MetroTextBox`，一并生效；NumberBox 同步改。

### 5.4 ComboBox / DropDownButton（下拉触发区）

| 项 | 一手源键 | 文件:行 原行 | 换算值 | 本库 |
|---|---|---|---|---|
| DropDownButton 底 | `ButtonBackground` | A L4269 BaseLow | `#00000033` | 改前 surface → 改后 `control_fill` |
| ComboBox 底 | `ComboBoxBackground` | A L4578 `…AltMediumLowBrush` | `#66FFFFFF` | `MetroSelectorFlyout` 用 `surface`（白，等价，保留） |
| ComboBox 边框 | `ComboBoxBorderBrush` | A L4591 `…BaseMediumLowBrush` | `#00000066` | **本库未画**（见 §Ⅵ） |
| 下拉项悬停 | `ComboBoxItemBackgroundPointerOver` | A L4562 `…HighlightListLowBrush` | `#19000000` | `indication.hover_tint` = `#00000019` ✅ |
| 下拉项按下 | `ComboBoxItemBackgroundPressed` | A L4561 `…HighlightListMediumBrush` | `#33000000` | `indication.press_tint` = `#00000033` ✅ |
| 下拉项选中 | `ComboBoxItemBackgroundSelected` | A L4564 `…HighlightListAccentLowBrush` | accent 0.4 | `selection_tint`（accent 0.4）✅ |

实测（DropDownButton `@(24,1003)`）：改前 `#F5F5F5`，改后 `#DADADA`。

### 5.5 ListView 行 / TabView / NavigationView / InfoBar（审计，未改笔刷）

| 控件 | 状态 | 一手源键 → 换算值 | 本库 | 判定 |
|---|---|---|---|---|
| ListView 行 | 悬停 | `…HighlightListLowBrush` → `#19000000` | `indication.hover_tint` `#00000019` | ✅ 一致 |
| ListView 行 | 按下 | `…HighlightListMediumBrush` → `#33000000` | `indication.press_tint` `#00000033` | ✅ |
| ListView 行 | 选中 | `…HighlightListAccentLowBrush` → accent 0.4 | `selection_tint`（0.4） | ✅ |
| TabView 选中页签 | — | WinUI 2 独占（`TabView_themeresources.xaml`），OS UWP 无对应键 | `surface_variant` `#F0F0F0` | 保留 |
| NavigationView 选中项 | — | WinUI `NavigationViewItemBackgroundSelected`（OS UWP 无） | `subtle_tint`（3.5%）+ 强调指示条 | 保留（Kanesumi 选择） |
| InfoBar | 错误 | 语义色由 `StatusColors` 派生 | `status.error_background` 等 | ✅ 与 WinUI 语义色交叉验证通过 |

### 5.6 ToolTip / MenuFlyout（面板类，浅色底本就可见）

| 控件 | 一手源键 | 文件:行 原行 | 换算值 | 本库 |
|---|---|---|---|---|
| ToolTip 底 | `ToolTipBackground` → `…ChromeMediumLowBrush` | A L723 → `SystemChromeMediumLowColor` L4141 | 亮 `#F2F2F2` | `teaching_tip` 用 `surface_variant` `#F0F0F0` ✅ 近似 |
| MenuFlyout / 弹层底 | —（面板一律 `surface`） | — | — | `popup.rs` 用 `surface` `#FFFFFF`，弹层有边框/遮罩区分 ✅ |

## §Ⅵ 未解决 / 待裁定

1. **`MetroSelectorFlyout` 触发区无边框**：UWP ComboBox 浅色靠 `ComboBoxBorderBrush`（40% 黑）与
   白页区分；本库触发器只填 `surface` 且不画边框 → 浅色下与页面难区分（白压近白）。修它是
   「新增描边命令」而非「改取色」，超出本次允许范围，登记为后续项。
2. **三字段暗色真值未采用**（§Ⅲ 表）：按任务约束保留旧观感，待裁定是否整体提亮深色。
3. **Button 禁用态填充**：现为 `control_fill × disabled_opacity = 0.2×0.38 ≈ 7.6%` 黑（浅色偏淡），
   UWP 禁用底仍是 BaseLow 20%、以**前景变灰**表达禁用。本库未改禁用前景逻辑，登记待裁定。
4. **ToggleButton 独立控件**：本库无单独 ToggleButton（由 CheckBox / Switch 承担），样张以二者覆盖。
5. **样张覆盖度**：`theme_sheet` 覆盖核心 17 个控件；`kanesumi-controls` 中另有约 20 个已接入元素树的
   控件（Dialog / ColorPicker / AutoSuggestBox / SwipeControl / CommandBarFlyout …）未纳入本轮样张，
   因其构造需复杂状态。同族笔刷已按规则核对（按钮族 / 文本族 / 列表族），未发现新的浅色隐形。
6. **读图服务不可用**：本轮无法做主观视觉巡检，已用像素采样 + `Tree::painted` 替代（§Ⅳ/§Ⅴ）。

## §Ⅶ 范围外发现

- `kanesumi-controls/src/selector_flyout.rs` 触发区缺边框（见 §Ⅵ-1）。
- `kanesumi-controls/src/expander.rs` 有 `render` 但**未实现 `Widget`**，故无法进主题样张网格。
- 其余未纳入样张的控件在浅色下的「同族取色」未逐一显式核对，建议后续补样张。
