# Kanesumi 控件行为规格（Metro / UWP 时代）

> 从 `ProjectArcturius/Ether` monorepo 的 `reference/` 参考快照提取（2026-08-10）。
> 目的：控件行为、尺寸、视觉状态、动画参数的**自足规格**——实现时以本文为准，无需再翻参考代码，可安全丢弃 `reference/`。
>
> 来源：
> - `microsoft-ui-xaml` **v2.8.7**（WinUI 2 最终版，Metro 时代）：`dev/CommonStyles/*_v1.xaml`（经典控件完整模板 + 视觉状态 + Storyboard）、`dev/{ProgressBar,ProgressRing,ComboBox}/`（C++ 实现）
> - `WinUI-Gallery` **winui2 分支**：`WinUIGallery/ControlPages/*Page.xaml(.cs)`（用法/交互）
> - 标注 **(OS)** 的 `SystemControl*` 资源为操作系统主题笔刷，**快照内无字面色值**，已按 Metro 时代惯例给出近似值或标注待定。

## 通用规律（先读）

1. **颜色切换是硬切换**：Metro 时代控件状态色用 `DiscreteObjectKeyFrame KeyTime="0"`，**无颜色过渡动画**。除明确标注外，视觉状态变化一律瞬时。
2. **按压反馈 = 位移不是缩放**：`PointerDownThemeAnimation`/`PointerUpThemeAnimation`（OS 主题动画，Y 向微下沉/复位，~100ms，参数 OS 预置）。无 Scale 反馈。
3. **禁用态可辨识度靠前景降透明度**（前景 40%，即 `disabled_opacity = 0.4` 语义；我们取 0.38）。
4. **无 Focused 视觉状态**：焦点用系统焦点视觉（外框，Margin -3），与控制状态机解耦。Kanesumi 可自绘焦点环（`indication.focus_stroke`），但它是**自适应增强**，非 Metro 状态。
5. **悬停/按压在列表类控件用中性高亮**，不用强调色；**只有选中态用强调色**。
6. 动画时长对 Kanesumi 预设的修正见 §9。

---

## 1 · Button（MetroButton）

| 项 | 值 |
|---|---|
| 模板 | 单个 `ContentPresenter`（背景/边框/内容一体），无包裹 Grid |
| Padding | `8,5,8,6`（RS3+；RS1 为 `8,5,8,5`） |
| BorderThickness | 1（但四态边框全 Transparent → 视觉无边框，只占位） |
| CornerRadius | 2px（`ControlCornerRadius`，参考值）—— **Kanesumi 定夺：默认 `Square`（直角）**。4× MSAA 保证直角边缘质量；`Capsule` 仅限结构性胶囊（Switch/进度条）。参 geometry.rs `CornerRadius` |
| FontSize / Weight | 14 / Normal |
| 无 MinWidth/MinHeight | 尺寸 = 内容 + Padding；最小可点区域靠宿主约束 |
| 激活键 | Space / Enter |

### 视觉状态（CommonStates，全部瞬时）

| 状态 | 背景 | 前景 | 动画 |
|---|---|---|---|
| Normal | 20% 白（BaseLow） | 不透明基色 | PointerUp |
| PointerOver | **10% 白独立笔刷**（比常态更淡的一层） | 不透明基色 | PointerUp |
| Pressed | **40% 白** | 不透明基色 | PointerDown（Y 下沉） |
| Disabled | 同 Normal | 前景 40% | — |

> Kanesumi 实现注：Ether 为深色空间桌面，底色用 `colors.surface` 不透明（而非半透明白叠层）；悬停/按压用 tint 叠加表达明暗。**tint 强度参考上方白%**：hover ≈ 0.10、press ≈ 0.22（见 §9 对 `MetroIndication` 的修正）。

### Accent 按钮

| 项 | 值 |
|---|---|
| 背景三态 | 强调色 → 提亮一档（PointerOver）→ 压暗一档（Pressed）；禁用 = 灰 |
| 前景 | **恒白**（含三态），禁用时 40% |
| 色档派生 | 强调色 Light1/Dark1 为 OS 运行时派生，无固定 hex → Kanesumi 需"提亮/压暗"函数或留 token |

### 焦点
- 系统焦点视觉，`FocusVisualMargin = -3`（框在控件外 3px）。Kanesumi 自绘焦点环用 `indication.focus_stroke`，宽 1。

---

## 2 · IconButton（AppBarButton 参考）

| 项 | 值 |
|---|---|
| 按钮尺寸 | **68 宽 × 56 最小高**（`AppBarThemeMinHeight`） |
| 图标区 | 16px（Viewbox），Margin `0,12,0,4`；标签 FontSize 12，Margin `2,0,2,8` |
| 常态背景 | **Transparent**（无常驻底） |
| PointerOver 背景 | 白 10%（`HighlightListLow`）；Pressed 白 20%（`HighlightListMedium`） |
| 前景 | PointerOver 与 Pressed **同色**（不像 Button 区分） |
| 焦点 | `AllowFocusOnInteraction = False`（点击不夺焦点） |
| 标签塌缩 | Compact 态隐藏标签（`ApplicationViewStates`）——纯图标模式用 |

---

## 3 · Switch（ToggleSwitch 参考）

> 数据源：`microsoft-ui-xaml/dev/CommonStyles/ToggleSwitch_themeresources_v1.xaml`
> （winui2/main 分支 = Metro/Lumia 时代规格）。真机对照：Lumia 950 显示器设置界面
> （`wp_ss_20250619_0002.png`，A1 重做参考图）。

### 结构 & 布局

```
┌ Header（可选，body 字号）
│
├──────────────────┐          ← Track 行（含 State text）
│  ⚪──────────────│  On/Off  ← 轨道 40×20 + 右侧 12px + 状态文本
└──────────────────┘
```

- **控件形状 = `SwitchShape { Capsule, Square }`**（Kanesumi 扩展）
- Capsule = UWP/Lumia 复刻本轮做的形态；Square = WP7 直角变体（占位，待完善）

### 尺寸（Capsule）

| 项 | 值 | 来源 |
|---|---|---|
| 控件 MinWidth | 154 | `ToggleSwitchThemeMinWidth` |
| 轨道 | **40 × 20**，全圆角胶囊 | `OuterBorder Width="40" Height="20"` |
| Knob（Ellipse） | **10 × 10** 圆 | `SwitchKnobOn Width="10" Height="10"` |
| Knob 上下留白 | **5px**（对称）= (20−10)/2 | `SwitchKnobBounds` 20×20 承载 10 knob |
| Knob 左右留白 | **5px**（对称） | 同上 |
| Knob 行程 | **20px** = 40 − 5 − 5 − 10 | 由上派生 |
| Track 右→State text | **12px** gap | `ColumnDefinition Width="12" MaxWidth="12"` |
| Header → Track 间距 | 8px（body.line_height + 8） | 排版一致 |
| Track 描边 | **2px**（OFF 态） | Kanesumi 修：v1 用 1px 在 HiDPI 亚像素消失，参 V10 同理 |

### 视觉状态

| 状态 | 轨道 fill | 轨道 stroke | Knob |
|---|---|---|---|
| Off Normal | Transparent | `on_surface_variant` 2px | 白 |
| Off Hovered | Transparent | `on_surface` 2px（加深） | 白 |
| Off Pressed | `on_surface_variant` 实心 | none | 白 |
| On Normal | **强调色**实心 | none | 白 |
| On Hovered | 强调色 lerp 白 15% | none | 白 |
| On Pressed | 强调色 lerp `on_surface_variant` 55%（dim 灰调，Lumia 观感） | none | 白 |
| Disabled | 上述任一 × `disabled_opacity` | 同上 × alpha | 白 × alpha |

> Kanesumi 决策：Pressed 灰调只在**真拖动**（moved=true）时呈现，避免点动闪灰。

### 交互（Kanesumi 扩展 A1）

- **点动**：press → release，位移 <3px → toggle
- **拖动**：press → drag_to（位移 ≥3px 触发）→ release：按 knob 中心过半判 on/off
- **取消**：press 后指针移出轨道再 release → `cancel()`，knob 回原位不 commit
- 命中区 = 整个 Track 矩形（不限于 Knob 圆内），方便触摸/鼠标

### 动画
- **切换滑动 = 150ms Cubic EaseOut**（`RepositionThemeAnimation`，行程 20px）
- 拖动过程中动画暂停（knob 由指针 `jump_to` 直接决定），release 后恢复 set_target 滑动
- 轨道/Knob 换色**瞬时**（无 crossfade）
- v1 模板无拖拽放大（Win8.1 遗产已删）

> Kanesumi 修正：`DURATION_TOGGLE_FLIP` 原 0.22s **改为 0.15s**（§9）。

---

## 4 · ProgressBar

### 尺寸
- MinHeight **4**（v1）/ 经典 3（Track 1）；CornerRadius 1.5（经典）/ 0（v1）
- 无默认宽，由宿主定（Gallery 用 Width=130）

### 确定模式
- 指示条宽 = `(value−min)/(max−min) × (宽 − padding)`
- 值变化 → `RepositionThemeAnimation FromHorizontalOffset=IndicatorLengthDelta` → **150ms 滑动**
- Paused：条色换灰 + Opacity→0.6（**0.25s** 淡出）；Error：错误色（0.25s）；恢复 0.25s

### 不确定模式（核心参数）
- **循环 2.0s**，`RepeatBehavior=Forever`，KeySpline `0.4,0,0.6,1`（对称 ease-in-out）
- **两波脉冲**：Indicator1 宽 40%、起点 −100%、0→1.5s 滑到 +300%、2s 保持；Indicator2 宽 60%、起点 −150%、0.75s 保持、0.75→2.0s 滑到 +166%
- 相位差 **0.75s**

---

## 5 · ProgressRing

### 尺寸
- 默认 **32×32**，Min 16；StrokeThickness 4；`Maximum=100`

### 不确定模式（Lottie）
- **循环 2.0s**（`c_durationTicks = 20000000`）
- 整体旋转 **0°→900°（2.5 圈）**，双段 cubic-bezier `(0.167,0.167,0.833,0.833)`（平滑 ease-in-out）→ 平均 **450°/s**
- 弧形态：前 1s `TrimEnd 0→0.5`（弧长到 180°），后 1s `TrimStart 0→0.5`（弧尾前推）——「半圆弧 + 旋转」呼吸
- 线帽 round

### 确定模式
- 弧角度 = value% **线性映射**（TrimEnd），值回退时**瞬跳不播反向动画**

---

## 6 · TabRow（Pivot 参考）

### 结构尺寸

| 项 | 值 |
|---|---|
| Header 高 | **48** |
| Header Padding | `12,0,12,0` |
| 头字 FontSize / Weight / 字距 | **24 / SemiLight / −2.5%**（CharacterSpacing −25） |
| 内容 Margin | `12,0,12,0` |

### 选中指示器（SelectedPipe）
- 每个 Header 底部独立 `Rectangle`，**不是共享滑动元素**：高 **2**、贴底 Margin `0,0,0,2`、Fill = **强调色**、宽 = 头文字宽
- 切换 = 各 Header `Visibility` 瞬时切换（无管道滑动 Storyboard）
- 「滑动指示器」观感来自整条 Header 面板平移（`PivotPanel` OS 代码，快照无源）

### 视觉状态（头文本色）
| 状态 | 色 |
|---|---|
| Selected | 不透明基色（最深，`#FFFFFFFF` dark） |
| Unselected | 中性 52%（`#66FFFFFF` dark = 40% 白） |
| 无头背景高亮 | 选中只靠文字色 + 管道 |

### 动画
- `Unselected → UnselectedLocked`：**0.33s**，头滑出 **+40px** + 淡出（`PivotHeaderItemLockedTranslation`）——非选中头滑出签名动效
- Selected 切换瞬时

---

## 7 · List（ListView/ListViewItem 参考）

### 列表项

| 项 | 值 |
|---|---|
| Padding | `12,0,12,0` |
| MinHeight | **40**；MinWidth 88 |
| 禁用 | 整行 Opacity **0.55** |
| 选中描边（多选用） | 4（`ListViewItemSelectedBorderThemeThickness`） |

### 选中/悬停视觉（单选）

| 状态 | 底 |
|---|---|
| 默认 | Transparent |
| PointerOver | **中性**高亮（`HighlightListLow`，≈30% 白/黑，非强调色） |
| Pressed | 中性高亮 Medium |
| **Selected** | **强调色 75% 不透明整行填充**（`ListAccentMediumLow`） |
| SelectedPointerOver | 强调色低档；SelectedPressed 强调色 90% |

> Kanesumi 修正：现实现 selected = `primary.with_alpha(0.15)` **明显偏轻**，不达 UWP 规格。改为 **0.60**（UWP 为 0.75，Ether 深色桌面调低一档），`padding_x` 16→**12**，行高下限 **40**（现 38）。

### 多选（备） | 网格（备）
- 多选：左侧 20×20、2px 边框勾选框，出现动画 X 位移 0↔−32，**0.333s**，spline `0.1,0.9,0.2,1`
- 网格：右上 34×34 对勾角标

---

## 8 · SelectorFlyout / DropdownMenu（ComboBox / MenuFlyout 参考）

### ComboBox 触发器（SelectorFlyout）

| 项 | 值 |
|---|---|
| MinHeight 32 / MinWidth 64；Padding `12,5,0,7` | |
| 箭头区 | 右列固定 **32px**，glyph `E70D`（ChevronDown）FontSize 12，Margin `0,0,10,0` |
| 状态色 | Normal=`AltMediumLow`；PointerOver=`PageBackgroundAltMedium`；Pressed=`ListMedium`；Disabled=`BaseLow`（均瞬时） |
| 聚焦 | 强调色低透明衬底（`HighlightListAccentLow`）+ 边框 |

### 下拉面板

| 项 | 值 |
|---|---|
| MaxDropDownHeight | **504**；最多 15 项、单侧最多 7 项 |
| 面板 MinWidth | 80（触控 240） |
| 底色 | Metro 时代纯色（`BackgroundChromeMediumLow`），Win10 后期 Acrylic |
| 边框 | 1px；项 Padding 鼠标 `11,5,11,7` / 触控 `11,11,11,13` |
| 项选中 | Selected=`ListAccentLow`（强调色低透）；SelectedPointerOver=`ListAccentMedium`；SelectedPressed=`ListAccentHigh` |
| 面板动画 | 遮罩淡入 **0.383s**、淡出 **0.216s**（spline `0.1,0.9,0.2,1`）；面板 SplitOpen ~333ms（OS，**未在快照**） |
| 方向自适应 | `ComboBoxHelper` 判据：弹出容器相对触发器的 `Top > 0` 即向下展开 |

### MenuFlyout（DropdownMenu）

| 项 | 值 |
|---|---|
| 项 Padding | `11,9,11,10`（14px 字 → 项高 ≈32）；紧凑 `11,4,11,7` |
| Presenter | MinHeight 32、Padding 0、1px 边框、Metro 时代纯色底（后期 Acrylic） |
| 图标区 | 16×16；有图标/勾选时占位 28px（双占位 56px） |
| 分隔线 | **高 1**、左右留白 12、色 `BaseMediumLow`（遗留纯值 #FF7A7A7A） |
| 状态色 | PointerOver = 中性高亮（暗 #FF212121 / 亮 #FFE5E5E5）；Pressed = 中（暗 #FFFFFFFF / 亮 #FF000000）；**瞬时** |
| 弹出动画 | `PopupThemeAnimation`（OS，~200ms 位移+淡入 ease-out；**未在快照**，用 `sheet_appear`/`sheet_dismiss` 对齐） |

---

## 9 · Dialog（ContentDialog 参考）

### 尺寸与布局

| 项 | 值 |
|---|---|
| Min/Max 宽 | **320 / 548**；Min/Max 高 **184 / 756** |
| Padding | `24,18,24,24`；TitleMargin 下 12；CommandSpace 上 24 |
| Title | FontSize 20 / Normal，MaxLines 2，MaxHeight 56 |
| 按钮 | 高 **32**、Min 宽 **130** / Max 202 |
| 按钮区 | 四列网格 `Stretch \| 0.5* \| 0.5* \| Stretch`：Primary / Secondary / Close |
| 背景 | Metro 时代纯色 chrome（`BackgroundChromeMediumLow`）；边框 `BaseLow` 1px |

### 遮罩（scrim）
- 整个 `LayoutRoot`（含遮罩）背景 = `SystemControlPageBackgroundMediumAltMediumBrush`（OS）
- 经典遗留：`ContentDialogDimmingThemeBrush` = **#99FFFFFF**（白 60% 遮罩）→ 无 Acrylic 时用它
- 无遮罩变体：`DialogShowingWithoutSmokeLayer` 状态 → 背景 null

### 动画（`DialogShowingStates`）

| 轨道 | 入场 | 退场 |
|---|---|---|
| Opacity（整根含遮罩） | 0→1，**0.167s 线性** | 1→0，**0.083s 线性** |
| Scale（对话框本体） | 1.05→1.0，**0.5s** spline `0.1,0.9,0.2,1` | 1.0→1.05，0.5s 同曲线 |

> 退场 opacity 先行熄灭（83ms），缩放随后。遮罩与盒体分别渲染。

### 交互
- **Esc = 轻解除**（走 hide 轨道）；**点击遮罩不关闭**（ContentDialog 不可点外解除）
- 默认按钮（Primary）用 `AccentButtonStyle`（Enter 触发）；按钮顺序 Primary → Secondary → Close（Close 恒最右）
- 按钮可见性由 `ButtonsVisibilityStates` 重组（AllVisible / NoneVisible / 各组合）

---

## 10 · 动画参数汇总表（vs Kanesumi 预设）

| 用途 | 参考时长 | 缓动 | Kanesumi 预设 | 结论 |
|---|---|---|---|---|
| 标准过渡 | 0.25s | Cubic EaseOut | `METRO_STANDARD_DURATION` | ✅ 一致 |
| 快速切换 | **0.167s** | FastOutSlowIn | `quick_switch` 0.18 | ⚠️ 对齐 0.167 |
| Switch 滑动 | **0.15s** | Cubic EaseOut | `toggle_flip` 0.22 | ❌ 改 0.15 |
| 面板入场 | 0.30s | Cubic EaseOut | `sheet_appear` 0.30 | ✅ |
| 面板收起 | 0.26s | Quadratic EaseOut | `sheet_dismiss` 0.26 | ✅ |
| 下拉遮罩入/出 | 0.383 / 0.216 | spline(0.1,0.9,0.2,1) | 新增 `overlay_open/close` | 需新增 |
| Dialog 缩放 | 0.5s | spline(0.1,0.9,0.2,1) | 新增 `dialog_enter` | 需新增 |
| Dialog 淡入/淡出 | 0.167 / 0.083 线性 | Linear | 新增时长常量 | 需新增 |
| ProgressBar/Ring 不确定循环 | **2.0s** | ease-in-out | 新增 `progress_indeterminate` | 需新增 |
| 颜色过渡 | 状态色多为瞬时或 ≤0.25s | — | `color_transition` 0.30 | ⚠️ 状态色勿套 0.30 |

### 已实施的预设修正（2026-08-10）
1. `DURATION_TOGGLE_FLIP`: 0.22 → **0.15**
2. `DURATION_QUICK_SWITCH`: 0.18 → **0.167**
3. 新增：`DURATION_INDETERMINATE = 2.0`、`DURATION_OVERLAY_OPEN = 0.383`、`DURATION_OVERLAY_CLOSE = 0.216`、`DURATION_DIALOG_ENTER = 0.5`、`DURATION_DIALOG_FADE_IN = 0.167`、`DURATION_DIALOG_FADE_OUT = 0.083`
4. `MetroIndication`：hover_tint 0.04 → **0.10**，press_tint 0.10 → **0.22**
5. `MetroList`：selected alpha 0.15 → **0.60**，`padding_x` 16 → **12**，行高下限 **40**

## 11 · 未在快照中（勿再翻代码）

| 项 | 位置 | 处理 |
|---|---|---|
| `SystemControl*` 笔刷具体色值 | OS 主题 | 按 §1–§8 标注的近似值/惯例定；后续可从 Win10 SDK `Common_themeresources` 核对一次固化为常量 |
| `PointerDown/UpThemeAnimation` 时长/像素 | OS 主题动画 | ~100ms Y 位移微反馈；可调 token |
| `SplitOpen/CloseThemeAnimation` | OS | 面板展开 ~333ms，用 `sheet_appear` 对齐 |
| `PivotPanel` 头面板平移 | OS | 签名动效"非选中头滑出 +40px/0.33s"已记录，可选实现 |
| `ListViewItemPresenter` 原生选中绘制 | OS | 行为已由 §7 覆盖 |
| `ComboBox.cpp` 开合定位 | OS | 方向判据已由 `ComboBoxHelper` 记录（Top>0 向下） |
| `ContentDialog.cpp` Esc/遮罩语义 | OS | 已按 UWP 语义记录（Esc 解除、点遮罩不关） |
| MenuFlyout 弹出动画 | OS Popup | ~200ms 位移+淡入 ease-out，用 `sheet_appear/dismiss` 对齐 |

---

## 12 · InfoBar（InfoBar 参考）

> 数据源：`microsoft-ui-xaml/dev/InfoBar/`（InfoBar.cpp + InfoBar_themeresources.xaml + InfoBar.xaml）。
> WinUI 2 时代开源控件（Fluent 风格资源）；Kanesumi 按铁律 6 将 Severity 配色映射为深色空间桌面纯色面板。

### 结构与布局

```
┌ ContentRoot（边框 1px divider）───────────────────────────────┐
│ Padding 16,0,0,0，MinHeight 48                                  │
│  [Icon] │ InfoBarPanel（横排或纵排）            │ [× 38×38] │
│          Title (14 SemiBold)  Message (14)  [Action]             │
└──────────────────────────────────────────────────────────────────┘
```

### 尺寸

| 项 | 值 |
|---|---|
| MinHeight | **48** |
| BorderThickness / 色 | 1 / `divider`（上游 `CardStrokeColorDefaultBrush`） |
| ContentRoot Padding | `16,0,0,0` |
| Icon | 16px；Margin `0,16,14,16`（上 16、右 14、下 16） |
| Panel Margin | `0,0,16,0` |
| 横排 Padding | `0,0,0,0`；Title `0,14,0,0`、Message `12,14,0,0`、Action `16,8,0,0` |
| 纵排 Padding | `0,14,0,18`；Title `0,14,0,0`、Message `0,4,0,0`、Action `0,12,0,0` |
| Title / Message | 14px；SemiBold / Normal |
| Close 按钮 | **38×38**，glyph 16，Margin `5` |

### 横排/纵排判据（InfoBarPanel::MeasureOverride）

1. 仅 1 项 → 纵排；
2. `totalWidth > availableWidth` → 纵排；
3. 任一项在横排高度 `> MinHeight(48)` → 纵排；
4. 否则横排（Title、Message、Action 一字排开，Message 前 12、Action 前 16）。

### Severity 四色（Kanesumi 深色适配）

| Severity | 面板底色 | 图标方块色 | 图标字形 |
|---|---|---|---|
| Informational | `#1E2A38` | `#4FC1FF` | `i` |
| Success | `#1E3328` | `#4CC38A` | `✓` |
| Warning | `#332B1E` | `#E5A94E` | `!` |
| Error | `#331E1E` | `#E5534A` | `✕` |

> 上游为 Fluent `SystemFillColor{Critical/Caution/Success/Attention}{Background}Brush`；Kanesumi 映射为纯色深底面板 + 高亮图标方块（铁律 6 无渐变纯色）。Title/Message 前景 = `on_surface`，图标字形白色。

### 交互
- `IsOpen=false` → 整个 ContentRoot 隐藏（无动画，模板 Collapsed 直接改 Visibility）。
- Close 按钮点击 → `close()` 置 `open=false`（回传关闭理由 CloseButton，无确认钩子）。
- Action 按钮（可选）点击 → 返回动作事件，宿主处理。

---

## 13 · Expander（Expander 参考）

> 数据源：`microsoft-ui-xaml/dev/Expander/`（Expander.cpp + Expander.xaml + Expander_themeresources.xaml）。

### 结构与尺寸

```
┌ Header（ToggleButton，MinHeight 48，Padding 16,0,0,0，bg surface，border divider 1px）┐
│  标题                                 [⋎ 32×32，glyph 12]                          │
├ Content（Padding 16，bg surface_variant，border 1,0,1,1 / 1,1,1,0）────────────────┤
└─────────────────────────────────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| MinHeight | **48**；MinWidth = `FlyoutThemeMinWidth` |
| Header Padding | `16,0,0,0`；Header bg `surface`、边框 `divider` 1px |
| Chevron 按钮 | **32×32**；Margin `20,0,8,0`（左 gap 20、右 8）；glyph 12 |
| Content | Padding **16**；bg `surface_variant` |
| Content 边框 | Down 模式 `1,0,1,1`；Up 模式 `1,1,1,0` |

### 动画（ExpandStates，progress 驱动）

| 方向 | 展开 | 收起 |
|---|---|---|
| Down | Content TranslateY `-contentH → 0`，**0.333s**，KeySpline `(0,0,0,1)` | `0 → +contentH`，**0.167s**，KeySpline `(1,1,0,1)`，0.2s 处隐藏 |
| Up | `+contentH → 0`，**0.333s** | `0 → -contentH`，**0.167s** |

- Chevron 旋转 `0° → 180°`，**0.1s**（`Checked` 状态）。
- Kanesumi 实现：`MetroAnim` 时长 0.333s / 0.167s（Cubic EaseOut 近似），chevron 0.1s。
- 铁律 4：展开只动 Content 位移（视觉属性），不动宿主布局。

### 视觉状态（Header 头）
- Normal / PointerOver / Pressed / Disabled：前景恒 `on_surface`（上游各态同 TextFillColorPrimaryBrush）；Chevron 按钮 PointerOver `secondary`（白 15%）底、Pressed `tertiary`（白 25%）。

---

## 14 · InfoBadge（InfoBadge 参考）

> 数据源：`microsoft-ui-xaml/dev/InfoBadge/`（InfoBadge.cpp + InfoBadge_themeresources.xaml）。

### 显示形态（DisplayKindStates）

| 状态 | 触发 | 视觉 |
|---|---|---|
| **Value** | `Value >= 0` | 数字文本（>99 → **"99+"**），FontSize **11**，Padding `4,0,4,2` |
| **Icon** | 有 icon 且 Value<0 | 图标 12×8 / 9×9，Padding `4,4,4,4` |
| **Dot** | 均无 | 最小 **4×4** 圆点 |

### 尺寸 / 色

| 项 | 值 |
|---|---|
| Min | **4×4**；MaxHeight **16** |
| CornerRadius | **ActualHeight/2**（全圆角胶囊；方形时 = 圆） |
| MeasureOverride | 若 `W < H` → 强制 `H×H` 方形（短边取齐） |
| 底 / 前景 | **强调色**（`primary`）/ 白（`on_primary`）——上游 `AccentFillColorDefault` / `TextOnAccentFillColorPrimary` |
| 派生风格 | Attention（`#4FC1FF`）/ Success（`#4CC38A`）/ Caution（`#E5A94E`）/ Critical（`#E5534A`）底（对齐 InfoBar 图标色） |

---

## 15 · PipsPager（PipsPager 参考）

> 数据源：`microsoft-ui-xaml/dev/PipsPager/`（PipsPager.cpp + PipsPager_themeresources.xaml）。
> 上游用 Segoe MDL2 glyph `EA3B` 画 pip；Kanesumi 自绘胶囊条（V7 不依赖私有区字形）。

### 尺寸

| 项 | 值 |
|---|---|
| Pip 命中区 | 横排 **12×20**；纵排 **20×12** |
| Pip 视觉（横排） | 横胶囊条：正常高 **4**（glyph font 4）、选中高 **6**（glyph font 6），宽 12 |
| Pip 视觉（纵排） | 纵胶囊条：正常 **4×12**、选中 **6×12** |
| Nav 按钮 | **20×20**；glyph 8（chevron）；Pressed 缩放 **0.875** |

### 色（上游 ControlStrongFillColorDefault → Kanesumi 映射）

| 项 | 值 |
|---|---|
| Pip（未选） | `on_surface_variant`（上游 ControlStrongFill ≈ 高强调白） |
| Pip（选中） | **强调色**（Kanesumi 适配：上游选中仅放大同色，铁律 5 选中态用强调色） |
| Nav 前景 | `on_surface`；PointerOver `on_surface_variant` |
| Nav 按钮可见性 | `pointer_over` 或 `show_nav` 时显示；选中首尾边缘时对应按钮禁用/隐藏 |

### 行为
- 点 pip → `selected_index` 更新（返回新索引）。
- Nav `<`/`>`：首/尾时隐藏。
- `MaxVisiblePips` < 页数时**居中滚动**（`CalculateScrollViewerSize = default×(n−1)+selected`）；Kanesumi 简化为滚动偏移渲染。

---

## 16 · PersonPicture（PersonPicture 参考）

> 数据源：`microsoft-ui-xaml/dev/PersonPicture/`（PersonPicture.cpp + InitialsGenerator.cpp + PersonPicture_themeresources_v1.xaml）。

### 结构与尺寸

```
┌────────────────┐
│   ╭─────╮      │ ← 头像圆（min(w,h) 方形），Ellipse fill surface_variant
│   │  JJ │      │   Initials 字号 = 42% of size，SemiBold，白
│   ╰─────╯      │
│     ┌──┐       │ ← Badge（可选）：50% of size 圆，右上角 Margin 0,-4,-4,0
└─────┴──┴───────┘   Badge 字号 = 60% of badge，2px 描边
```

| 项 | 值 |
|---|---|
| 默认尺寸 | **96×96**；尺寸变化维持方形（min 值强制到宽高） |
| 头像圆 | Ellipse fill `surface_variant`、无描边、fg 白 |
| Initials 字号 | **42%** of 边长；SemiBold |
| Badge | **50%** of 边长；位置右上 Margin `0,-4,-4,0`；fill `#1A1A1A`、描边 `divider` **2px** opacity 0.8、fg `on_surface`（白） |
| Badge 字号 | **60%** of badge 圆 |
| Badge 数字 | `>99` → **"99+"** |

### 首字母生成（InitialsGenerator::InitialsFromDisplayName）

1. 名字含 CJK/字形（Symbolic/Glyph）→ 空（回退默认图形/留空）；
2. 去尾随括号对（`(…)`/`[…]`/`{…}`）；
3. 按空格拆分：
   - 单词 → 取首字符；
   - 多词 → 首词首字符 + 末词首字符；
   - 全空 → 空；
4. 跳过开头标点（`!…/`、`:…@`、`{|}~`）与后续组合变音符；结果转大写。

---

## 17 · DropDownButton（DropDownButton 参考）

> 数据源：`microsoft-ui-xaml/dev/DropDownButton/`（DropDownButton.cpp + DropDownButton_v1.xaml）。
> WinUI 2 开源；本质 = Button + 右侧 chevron + Flyout。

### 结构与尺寸

```
┌ InnerGrid（Padding 8,5,8,6，bg surface）─────────────────────┐
│  标签                                     [⋎ 12，Margin 6,0,0,0] │
└───────────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| Padding | `8,5,8,6`（同 MetroButton） |
| Chevron | `E70D`（ChevronDown）→ Kanesumi 自绘 `chevron_down`；FontSize **12**；Margin `6,0,0,0` |
| 视觉状态 | 同 MetroButton Standard（四态硬切换、禁用降透明度） |

### 交互
- 点击 → toggle 关联 flyout（`MetroDropdownMenu`）。
- Flyout 打开时按钮呈 Pressed 亮度（对齐 MenuBar 的 Selected 语义）。
- 点 flyout 项 → 关闭并返回 `(item_idx)`；点外部 → 关闭。

---

## 18 · BreadcrumbBar（BreadcrumbBar 参考）

> 数据源：`microsoft-ui-xaml/dev/Breadcrumb/`（BreadcrumbBar.cpp + BreadcrumbBar.xaml + BreadcrumbBar_themeresources.xaml）。

### 结构与尺寸

```
[首页]  ›  [文件夹]  ›  [子目录]  ›  当前页
```

| 项 | 值 |
|---|---|
| Item 字号 | **14**（ControlContentThemeFontSize）/ Normal；LineHeight 20；Padding `1,3` |
| Chevron | `E974`（右）→ Kanesumi 自绘 `chevron_right`；FontSize **12**、Padding `2,0`（每 chevron 占 ~16px） |
| Ellipsis | `E712`（…）→ 文本 `…`；FontSize 14、Padding 3 |
| 当前项（末项） | 非按钮（无 hover）、无尾部 chevron、前景 `on_surface` |

### 前景色（TextFillColor 映射）

| 态 | 值 |
|---|---|
| 正常 | `on_surface`（TextFillColorPrimary） |
| PointerOver | `on_surface` 强（上游 Secondary 转暗；Kanesumi 取 hover 变亮） |
| 当前项 | `on_surface`（同 Primary） |

### 折叠（BreadcrumbLayout 语义）
- 总宽 ≤ 可用宽 → 全部展示；
- 超宽 → 前缀折叠：隐藏前 `k` 项，换成 `…`（Ellipsis），至少保留末项；
- 点 `…` → 弹出隐藏项下拉（Flyout MinHeight 40）；点隐藏项 → 返回其索引。

### 交互
- 点非当前项 → 返回索引（宿主导航）。
- 当前项（末项）不可点。

---

## 19 · SplitButton（SplitButton 参考）

> 数据源：`microsoft-ui-xaml/dev/SplitButton/`（SplitButton.cpp + SplitButton_v1.xaml + SplitButton_themeresources.xaml）。

### 结构与尺寸

```
┌ [PrimaryButton（*，MinWidth 35）] │ 分隔线 1px │ [SecondaryButton 35px] ┐
│  标签                             │            │    ⋎（E70D，12px，Pad 0,0,9,0）│
└──────────────────────────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| Primary 最小宽 | **35** |
| Secondary 宽 | **35**；chevron `E70D` → 自绘 `chevron_down` 12px、Padding `0,0,9,0` |
| Separator | 1px 列，色 `ControlStrokeColorDefaultBrush`（→ Kanesumi `divider`） |
| Border | 1px（ControlElevationBorderBrush → `divider`） |
| Padding | `ButtonPadding`（同 MetroButton `8,5,8,6`） |

### 视觉状态（SplitButtonBackground → Kanesumi 映射）

| 态 | 背景 |
|---|---|
| Normal | `surface`（ControlFillColorDefault） |
| PrimaryPointerOver | Primary 区白 10%、Secondary 区常态 |
| PrimaryPressed | Primary 区白 22%、Secondary 常态 |
| SecondaryPointerOver / Pressed | 同上，作用于 Secondary 区 |
| FlyoutOpen | **两区全 Pressed**（白 22%） |

### 交互
- 点 Primary → 返回 `Primary`（宿主执行主命令）。
- 点 Secondary → toggle 关联 `MenuFlyout`；点 flyout 项 → 返回 `Index`；点外 → 关闭。

---

## 20 · PagerControl（PagerControl 参考 · NumberPanel 模式）

> 数据源：`microsoft-ui-xaml/dev/PagerControl/`（PagerControl.cpp + PagerControl.xaml + PagerControl_themeresources.xaml）。
> Kanesumi 实现 **NumberPanel 显示模式**（数字按钮分页）——NumberBox / ComboBox 模式依赖闭源
> NumberBox / ComboBox，不在此列。

### 结构与尺寸

```
[First ◀◀] [Prev ◀]  1  2  3  4  5  …  n  [Next ▶] [Last ▶▶]
                 （选中数字下方 2px 强调色指示条）
```

| 项 | 值 |
|---|---|
| Nav 按钮 | **40×40**；glyph `E892`（◀◀）/`E76B`（◀）/`E76C`（▶）/`E893`（▶▶）→ Kanesumi 自绘 chevron |
| 数字按钮 | MinWidth **32**、MinHeight **20**、间距 **5** |
| 选中指示 | 数字下 **2px** 强调色条（`PagerControlSelectionIndicatorForeground` = Accent；`RepositionThemeTransition` 滑动 → Kanesumi 直接画在选中数字下） |
| 选中数字前景 | **强调色**（AccentFillColorDefault） |
| Nav 可见性 | 首页隐藏 First/Prev、末页隐藏 Next/Last（保留布局空间，opacity 0） |

### 数字窗口（UpdateNumberPanel 逻辑）

- `n ≤ 7` → 全部 `1..n`；
- `s ≤ 4`（前四页）→ `1 2 3 4 5 … n`（`always_show_first_last` 时含末页）；
- `s ≥ n−3`（后四页）→ `1 … n−4 n−3 n−2 n−1 n`；
- 中间 → `1 … s−1 s s+1 … n`。

> `ButtonPanelAlwaysShowFirstLastPageIndex` 默认 true。`s` 为 1 基选中页。

### 交互
- 点数字按钮 → `Select(page)`（0 基返回）。
- First/Prev/Next/Last → 分页动作；首/末边缘对应按钮隐藏。

---

## 21 · RadioButtons（RadioButtons 参考）

> 数据源：`microsoft-ui-xaml/dev/RadioButtons/`（RadioButtons.cpp + RadioButtons.xaml +
> RadioButtons_themeresources.xaml）。单个 RadioButton 闭源，Kanesumi 自绘单选圆（Metro 时代观感）。

### 结构与尺寸

```
Header（可选，Margin 0,0,0,8）
○  选项 A
○  选项 B          ← RowSpacing 8
●  选项 C          ← 选中：圆内强调色圆点
```

| 项 | 值 |
|---|---|
| 单选圆 | **20×20**、描边 2px（`on_surface_variant`）、透明底 |
| 选中 | 圆心 **10px 强调色**圆点（Metro 8 观感：外圈 + 圆点） |
| 悬停 | 描边转 `on_surface` |
| 圆 → 标签 gap | **6px** |
| ColumnSpacing / RowSpacing | **7 / 8** |
| MaxColumns | 容器列数（默认 **1** = 纵向堆叠；网格按列宽自适应） |
| Header | Margin `0,0,0,8`；前景 `on_surface` |

### 交互
- 点任意项 → 选中（单选框语义，`selected_index` 更新）。
- 点已选中项 → 保持选中（不取消）。

---

## 22 · TwoPaneView（TwoPaneView 参考）

> 数据源：`microsoft-ui-xaml/dev/TwoPaneView/`（TwoPaneView.cpp + TwoPaneView.xaml）。
> 双面板自适应容器（宽屏并排 / 高屏堆叠 / 窄屏单面板）。Kanesumi 实现为**纯布局**
> （返回两面板 rect，宿主渲染内容），无自绘。

### 属性与默认值

| 项 | 默认 | 说明 |
|---|---|---|
| MinWideModeWidth | **641** | 宽于此切 Wide 模式 |
| MinTallModeHeight | **641** | 高于此切 Tall 模式 |
| WideModeConfiguration | LeftRight | LeftRight / RightLeft / SinglePane |
| TallModeConfiguration | TopBottom | TopBottom / BottomTop / SinglePane |
| PanePriority | Pane1 | 单面板模式显示哪个面板 |
| Pane1/Pane2 长度 | 1:1 | Kanesumi 用 `pane1_ratio`（默认 0.5）分栏 |

### 模式判定（UpdateMode，单区域）

```
if  宽 > MinWideModeWidth && config≠SinglePane → Wide（按 config 左右/右左）
else if 高 > MinTallModeHeight && config≠SinglePane → Tall（按 config 上下/下上）
else → SinglePane（PanePriority 指定单面板）
```

### 面板矩形
- **Wide**：Pane1 左 `[0, w·r]`、Pane2 右 `[w·r, w]`；
- **Tall**：Pane1 上 `[0, h·r]`、Pane2 下 `[h·r, h]`；
- **SinglePane**：PanePriority 面板占满，另一面板空。

> 多显示区域（折叠屏 hinge）逻辑不移植（Ether 桌面单屏），单区域判据已覆盖。
> 中缝（PART_ColumnMiddle/RowMiddle）在单区域为 0 —— Kanesumi 无中缝。

---

## 23 · TitleBar（TitleBar 参考）

> 数据源：`microsoft-ui-xaml/dev/TitleBar/`（TitleBar.cpp + TitleBar.xaml + TitleBar_themeresources.xaml）。
> 应用 SSD 标题栏（对齐 Ether Issue #8）。

### 结构与尺寸

```
[← 44×H] [icon 16]  Title ....................  [custom content]
```

| 项 | 值 |
|---|---|
| 高度 | Compact **32** / Expanded **48** |
| Back 按钮 | **44×H**；glyph `E72B`（Back）→ 自绘 `chevron_left` 16px |
| Icon | 16×16、Margin `4,0,0,0` |
| Title | Caption（12px）；Margin `16,0,16,2`；MinWidth **48**；`TextTrimming` |
| Custom content | 右侧（宿主注入） |
| Back 隐藏时 | Icon Margin → `16,0,0,0` |

### 视觉状态

| 态 | Title / Back 前景 |
|---|---|
| Activated | `on_surface`（TextFillColorPrimary） |
| Deactivated | `on_surface_variant`（TextFillColorTertiary，转暗） |
| Back Hover | 底 `SubtleFillColorSecondary`（白 15%） |
| Back Pressed | 底 `SubtleFillColorTertiary`（白 25%） |

### 交互
- Back 点击 → 返回 `Back`（宿主导航）。
- 无其它交互（图标/标题不可点）。

---

## 24 · RatingControl（RatingControl 参考）

> 数据源：`microsoft-ui-xaml/dev/RatingControl/`（RatingControl.cpp + RatingControl.xaml +
> RatingControl_themeresources.xaml）。上游用 MDL2 `E735`（实星）/`E734`（空星）；
> Kanesumi 用标准 Unicode `★`/`☆`（思源黑体包含，V7 不依赖私有区字形）。

### 结构与尺寸

```
★ ★ ★ ☆ ☆        ← MaxRating 5，值 3（整星）；支持小数（半星 = 裁剪）
```

| 项 | 值 |
|---|---|
| 控件 Height | **32**；star cell ≈ **24×24**（FontSize 32 + Margin −8 补偿） |
| MaxRating | 默认 **5** |
| 实星（Selected） | **强调色**（AccentFillColorDefault → `primary`） |
| 空星（Unselected） | `on_surface_variant`（TextFillColorSecondary） |
| Placeholder | `on_surface` |
| PointerOver 实/空星 | 强调色 / `on_surface` |
| Disabled | `on_surface_variant` 低透（TextFillColorDisabled） |

### 行为
- `Value` 可为小数（部分星：前景星按比例裁剪）。
- **PointerOver 预览**：悬停到某星 → `hover_value`（释放时提交）。
- **Clear**：`IsClearEnabled` 时点当前值所在星 → 值清零。
- `IsReadOnly`：只读（无 hover/点击）。

### 交互
- 点击第 k 星 → `Value = k`；`IsClearEnabled` 且点当前值星 → `Value = 0`。
- 返回 `Option<f64>` 新值（None = 未变化）。

---

## 25 · TabView（TabView 参考）

> 数据源：`microsoft-ui-xaml/dev/TabView/`（TabView.cpp + TabView_themeresources.xaml）。
> Chrome 式标签页（区别于 Pivot：可关闭、可拖拽排序）。Kanesumi 实现 tab strip
> （Add/Close/Select + 滚动），拖拽 Reorder 暂略（Phase 3 续）。

### 结构与尺寸

```
┌ Header（Padding 0,8,0,0）─────────────────────────────┐
│ [ Tab1  ×] [ Tab2  ×] [Tab3] [＋]                     │
└───────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| Item | MinHeight **32**、MinWidth **100**、MaxWidth **240** |
| Header Padding | `8,3,4,3`（Selected `9,3,5,4`） |
| 字号 | **12**；Icon 16（Margin 0,0,10,0） |
| Close 按钮 | **32×24**、glyph 16、Margin `4,0,0,0` |
| Add 按钮 | **32×24**、+（FontSize 12）、Padding `3,0,0,3` |
| 等宽分配 | `tab_w = clamp(avail/len, 100, 240)`；总宽超可用 → 滚动 |
| 分隔线 | `divider`（DividerStrokeColorDefault，`ShowTabsSeparator` 时） |

### 视觉状态

| 态 | 底 | 前景 |
|---|---|---|
| 未选中 | Transparent | `on_surface_variant` |
| PointerOver | `on_surface` 8% | `on_surface_variant` |
| **Selected** | `surface_variant`（SolidBackgroundFillColorTertiary） | `on_surface` |
| Close 按钮 | hover 白 15% / press 白 25% | — |

### 行为
- 点 tab → `Select`；点 Close（hovered/selected 显示）→ `Close`；点 ＋ → `Add`。
- `IsClosable` 隐藏 Close；`AddButtonEnabled` 隐藏 ＋。
- 滚动：`scroll_by(delta)` 夹紧 `[0, total−avail]`。

---

## 26 · TeachingTip（TeachingTip 参考）

> 数据源：`microsoft-ui-xaml/dev/TeachingTip/`（TeachingTip.cpp + TeachingTip.xaml +
> TeachingTip_rs1/rs2_themeresources.xaml）。新特性引导气泡。

### 结构与尺寸

```
        ┌──────────────────────────────┐
        │ [×]                          │  ← AlternateCloseButton 40×40（glyph 16）
        │  Title（SemiBold 14）         │
        │  Subtitle（14）               │
        │  Body                        │
        │  [Action] [Close]            │
        └───────────▾──────────────────┘
                    尾（Tail 指向目标）
```

| 项 | 值 |
|---|---|
| 面板 | MinW **320** / MaxW **336**；MinH **40** / MaxH **520** |
| ContentMargin | **12**；Border 1px |
| Title / Subtitle | 14px；SemiBold / Normal；前景 `on_surface` |
| Close（Alternate） | **40×40**、× 16；右上；Title 区右让位 Margin `0,0,28,0` |
| 操作区 | Row 2：[Action] [Close] 两列 `*/*`；Margin `0,12,0,0` |
| Tail | 三角指向目标，Fill 面板底 |

### 放置（TeachingTipPlacement 判定）
- 四方向（Top/Bottom/Left/Right）取**可用空间最大**的一侧（`place_teaching_tip` 返回 (rect, side)）。
- 面板 320 宽、贴目标侧；Tail 对齐目标中心。

### 交互
- Action 点击 → 返回 `Action` 并关闭；Close 点击 → `Close` 并关闭。
- 打开 = 淡入（0.167s 线性近似）；关闭 = 淡出（0.083s）。

---

## 27 · TreeView（TreeView 参考）

> 数据源：`microsoft-ui-xaml/dev/TreeView/`（TreeView.cpp + TreeViewItem.cpp +
> TreeView_themeresources.xaml）。文件夹树 / 层级选项。

### 结构与尺寸

```
▸ 文档                    ← 有子项：chevron（折叠 → ▸/展开 → ▾）
  ▾ 项目                  ← 缩进 depth×16
    · 源码
    · 文档
```

| 项 | 值 |
|---|---|
| Item MinHeight | **28**；PresenterPadding `0,3,0,5`（内容 20 居中） |
| PresenterMargin | `4,2` |
| 缩进 | **depth × 16**（TreeViewItem::UpdateIndentation `depth * 16`） |
| Chevron | 16px；折叠 `E70D` 旋转 → Kanesumi 自绘 `chevron_right`（折叠）/`chevron_down`（展开），翻转 0.1s |

### 视觉状态（Kanesumi 映射）

| 态 | 底 | 前景 |
|---|---|---|
| 默认 | Transparent | `on_surface` |
| PointerOver | 白 15%（SubtleFillColorSecondary） | `on_surface` |
| Pressed | 白 25%（Tertiary） | `on_surface_variant` |
| **Selected** | 白 15%（同 PointerOver，SubtleFillColorSecondary） | `on_surface` |
| 选中指示 | 多选时强调色（单选用底高亮） | — |

### 交互
- 点 chevron → toggle 展开/收起（翻转 0.1s）。
- 点行标签 → 选中（返回路径）。
- 折叠时子项隐藏（不参与命中/渲染）。

---

## 28 · NavigationView（NavigationView 参考）

> 数据源：`microsoft-ui-xaml/dev/NavigationView/`（NavigationView.xaml + NavigationView_themeresources.xaml）。
> Settings 左侧导航 / Librarian 侧栏。Kanesumi 实现 Left（Pane）与 Top 两模式核心：
> Pane toggle / 项列表（icon+label）/ 选中指示条 / Header+Content 布局。子项级联、flyout、
> 动画式展开收窄暂略（Phase 3 续）。

### 结构与尺寸（Left 模式）

```
┌ toggle 40 ────────┐
│ PaneHeader        │
│ ▍设置   ← 选中 3px 强调条│   ← Item 高 40、icon 16、字 14
│ ▸ 外观            │
│ ...               │
├───────────────────┤
│ footer 项         │
└───────────────────┘
```

| 项 | 值 |
|---|---|
| Expanded Pane 宽 | **320**（NavigationViewExpandedPaneWidth） |
| Compact Pane 宽 | **48**（icon 模式） |
| Top Pane 高 | **48**（NavigationViewTopPaneHeight） |
| PaneToggle 按钮 | **40×40**（左侧最上） |
| Item | 高 **40**、icon 16、字 14、Padding 左 16 |
| 选中指示条 | **3×16** 强调色（NavigationViewSelectionIndicator 3/16），项左侧 |
| Header Margin | `56,44,0,0`（Header 位于 pane 右、顶栏下） |
| 选中/悬停底 | 白 8% / 15%（SubtleFill） |

### 行为
- 点项 → 选中（返回索引路径）；点 toggle → 展开/收窄 Pane（320↔48）。
- Footer 项独立（会话操作，非导航）。
- `content_rect` = Pane 右（Left）或顶栏下（Top）；`header_rect` 依 Margin。

---

## 29 · ColorPicker（ColorPicker 参考）

> 数据源：`microsoft-ui-xaml/dev/ColorPicker/`（ColorPicker.cpp + ColorPicker.xaml +
> ColorPicker_themeresources.xaml）。Settings 主题色。
> **Kanesumi 适配**：Scene 纯色无渐变（铁律 6），Spectrum 2D 渐变用**阶梯色带**近似
> （离散 hue 列，on-brand）；RGB/A 滑轨为实心轨道 + 填充段 + 10×10 拇指（可渲染）。

### 结构与尺寸

```
┌ ColorSpectrum（阶梯 hue 带，可选）──────────────────┐
│ ▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒                  │
├───────────────────────────────────────────────────┤
│ R ▓▓▓▓▓▓▓▓○──────────  ← 实心轨道 + 填充段 + 拇指 │
│ G ▓▓▓▓▓▓▓▓○──────────                            │
│ B ▓▓▓▓▓▓▓▓○──────────                            │
│ A ▓▓▓▓▓▓▓▓○──────────                            │
├───────────────────────────────────────────────────┤
│ [预览色块 44]   #E57812                            │
└───────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| 垂直朝向 | Min **312×312** / Max **392×392**（VerticalOrientation 312..392） |
| 滑轨 | 拇指 **10×10**（ColorPickerSliderInnerThumb 10）；轨道圆角 6 |
| 预览块 | 高 **44**（Spectrum 隐藏时）；边 2px（ColorPickerBorderBrush） |
| 滑轨通道 | R / G / B / A（0..255） |

### 交互
- 点/拖滑轨 → 更新对应通道（`handle_click` / `drag_to` 返回变化）。
- 返回 `Option<Color>` 新颜色（None = 未变化）。

---

## 30 · ParallaxView（ParallaxView 参考）

> 数据源：`microsoft-ui-xaml/dev/ParallaxView/`（ParallaxView.cpp + ParallaxView.idl）。
> 视差滚动：内容以慢于滚动源的速度位移。

### 公式（HorizontalShift / VerticalShift）

```
shift = scroll_offset × ratio
clamp 到 [−MaxShift, +MaxShift]，MaxShift = MaxShiftRatio × 视口主轴
```

| 项 | 默认 |
|---|---|
| ParallaxRatio | **0.5**（视差系数，0..1） |
| Horizontal/VerticalShift | 0（基准） |
| MaxHorizontal/VerticalShiftRatio | **1.0**（上限 = 视口 × 比率） |
| IsShiftClamped | true |

### 行为
- `content_offset(scroll)` → 内容 rect（位移后的视口窗口），宿主据此渲染内容。
- 纯布局/位移辅助（无自绘）。

---

## 31 · AnimatedIcon（AnimatedIcon 参考）

> 数据源：`microsoft-ui-xaml/dev/AnimatedIcon/`（AnimatedIcon.cpp）。上游用 AnimatedVisual
> （Lottie 式）；Kanesumi 用**几何 chevron 插值**（V7 自绘，不依赖 Lottie runtime）。

### 行为
- `dir_off` / `dir_on`：两个正交方向（Down/Up/Left/Right）；
- `set_state(on)` → 0.1s 插值 chevron 从 `dir_off` 翻到 `dir_on`；
- 渲染：三角形 chevron，base/tip 坐标按进度线性插值。

### 交互
- 无指针交互（纯状态动画图标；宿主驱动 `set_state`）。

---

## 32 · SwipeControl（SwipeControl 参考 · Reveal 模式）

> 数据源：`microsoft-ui-xaml/dev/SwipeControl/`（SwipeControl.cpp + SwipeControl.idl）。
> 触屏滑动手势项（桌面优先低，P3）。Kanesumi 实现 Reveal 模式：
> 左右滑动露出操作项（LTR）。

### 结构与尺寸

```
┌ [操作A][操作B] ──────内容──────┐   ← 拖动露出左侧操作项
└──────────────────────────────┘
```

| 项 | 值 |
|---|---|
| SwipeItem | 文本 + 点击；LeftItems/RightItems |
| Mode | **Reveal**（拖出操作项）/ Execute（拖出即触发） |
| 释放吸合阈值 | 拖动距离 > 项区一半 → 吸合展开；否则回弹 |

### 交互
- `drag_to(dx)` → 露出距离（夹紧到项区宽）；`release()` 吸合/回弹。
- 点操作项 → 返回 `Invoke(index)`。
- 点内容 → `Close`（收起）。

---

## 33 · Grid（MetroGrid 布局原语）

> 数据源：UWP `Grid` 为平台内置布局容器（Windows.UI.Xaml，闭源），无独立 .cpp。
> 规格来自 XAML 用法惯例（RowDefinition/ColumnDefinition：Fixed / Auto / Star）+ Metro 时代网格布局。

### 尺寸定义

| 项 | 值 |
|---|---|
| 轨道类型 | `Fixed(px)` / `Auto`（内容自适应）/ `Star(w)`（比例分配剩余，`*`=1） |
| 间距 | UWP Grid 无 gap（子元素用 Margin）；Kanesumi 以 `gap`（row, col）可选扩展 |
| 子单元 | `GridChild { row, col, row_span, col_span }`（UWP `Grid.Row/Column/RowSpan/ColumnSpan`） |

### 布局算法（`resolve`）

1. Fixed 轨道占自身像素；Auto 轨道取宿主量测值（经 `auto_rows/auto_cols` 传入）；
2. 剩余空间（rect − Fixed − Auto − 间距）按 Star 权重比例分配；
3. 无 Star 轨道时剩余空间留空（UWP 行为）；
4. `child_rect` 合并跨度多轨道 + 其间间距。

> Kanesumi 实现：`kanesumi-structure::MetroGrid`（纯布局，不自绘、不持控件状态）。

---

## 34 · TextBox（MetroTextBox）

> 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/TextBox_themeresources_v1.xaml`
> （Windows.UI.Xaml 平台内置控件，主体闭源，只有默认模板 XAML 可读）。

### 结构与尺寸

```
┌ Header（可选，Margin 0,0,0,4）────────────────────────────┐
│ ┌─────────────────────────────────────────┬────────────┐ │
│ │ 文本 / 占位（Padding 10,6,6,5）           │ [× 34]     │ │
│ └─────────────────────────────────────────┴────────────┘ │
└──────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| MinHeight / MinWidth | **32 / 64**（TextControlThemeMinHeight/MinWidth） |
| BorderThickness | 1（`TextControlBorderThemeThickness`）；Focused **2** |
| Padding | `10,6,6,5`（TextControlThemePadding v1） |
| 字号 | ControlContentThemeFontSize（14，Kanesumi 用主题 body） |
| 删除按钮 | MinWidth **34**、glyph `E894`（×）→ Kanesumi 自绘 `✕` 字形 |
| 光标 | 1px（V10：HiDPI 用 **2px**）；闪烁 on/off 各 0.5s |

### 视觉状态（CommonStates，全部瞬时）

| 态 | 边框 | 底 |
|---|---|---|
| Normal | `TextControlBorderBrush`（BaseMedium → divider） | `TextControlBackground`（surface） |
| PointerOver | `TextControlBorderBrushPointerOver`（Highlight → on_surface_variant） | 同 Normal |
| Focused | `TextControlBorderBrushFocused`（**2px**，Kanesumi focus_stroke） | `TextControlBackgroundFocused`（AltHigh） |
| Disabled | 前景降透明度 | 前景降透明度 |

### 交互

- 聚焦（点击/键盘）：**全选**（UWP TextBox 聚焦行为）；点击定位光标（按字符边界最近）。
- 编辑核心（`TextField`）：插入/删除/Backspace/Delete/Left/Right/Home/End + **选区**（Shift+方向）+ **撤销**（栈上限 64）+ 掩码（PasswordBox）。
- 占位文本：空内容时显示，Focused 时转半透明（TextControlPlaceholderForegroundFocused）。
- 删除按钮：有内容 + 聚焦/悬停时显示（ButtonStates ButtonVisible）。

### IME 组合态（zwp_text_input_v3，参 IME_WIRING_PLAN）

- 组合态显示流 = 光标前文本 + **preedit** + 光标后文本；preedit 不入 `text` / 选区 / 撤销栈。
- preedit 视觉规格：**虚线下划线**（`on_surface` 60% opacity，dash 4px / gap 3px，基线下方 2px）；
  无平台固定规格（UWP 闭源），走以上平台默认（参 IME_WIRING_PLAN 阶段 B）。
- 光标 x 后移 `preedit_cursor` 宽；`caret_rect_absolute()` / `ime_context()` 暴露给 harness
  灌 set_surrounding_text / set_cursor_rectangle。
- Escape / 直接键插入 / 周边删除 → 打断组合态（清 preedit）。

### Kanesumi 适配

- 深色桌面底色 `surface`；选区高亮强调色 **35%**（TextControlSelectionHighlightColor → primary）；
- CJK 安全：下标按 char 非字节；IME 协议字节下标一律 UTF-8 边界外扩夹紧（不劈码点）。

---

## 35 · PasswordBox（MetroPasswordBox）

> PasswordBox 是 TextBox 的掩码变体（Windows.UI.Xaml，闭源）。Kanesumi 以
> `MetroTextBox` + `TextField::set_mask(Some('●'))` 实现。

### 规格

| 项 | 值 |
|---|---|
| 掩码字符 | `●`（UWP 默认 PasswordChar） |
| 明文 | 保留于 `field.text()`，显示层全掩码，**绝不渲染明文** |
| 尺寸/状态/交互 | 同 TextBox（§34），仅掩码差异 |
| 输入法 | IME 已接入：组合态同样掩码显示；`ime_focus()` 返回 `content_hint = Password` → harness 映射 `content_purpose = password \| content_hint = sensitive_data \| hidden_text`（fcitx5 自禁候选窗）；周边文本**不外发**（不暴露明文） |

### IME 组合态（PasswordBox 专属）

- preedit 逐字符掩码（与正文同一 `●`）；
- `ime_context()` 清空 surrounding（敏感字段不交给输入法），光标矩形保留。

---

## 36 · CheckBox（MetroCheckBox）

> 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/CheckBox_themeresources_v1.xaml`
> （Windows.UI.Xaml 平台内置，主体闭源，只有默认模板 XAML 可读）。

### 结构与尺寸

```
┌ [□ 20]  标签（Padding 左 8）──────────────┐
│   列0=20   列1=*                          │
└──────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| 勾选框 | **20×20**、列 0 宽 20 |
| Padding | `8,5,0,0` |
| MinWidth / MinHeight | **120 / 32** |
| 描边 | CheckBoxBorderThemeThickness **1** |
| 字形 | ✓（`E73E`）/ —（`E73C`）→ Kanesumi 用标准 Unicode（思源黑体含） |
| CornerRadius | ControlCornerRadius（Kanesumi 默认 Square） |

### 视觉状态（CombinedStates，全瞬时）

| 态 | 勾选框 fill | 勾选框 stroke | 字形 |
|---|---|---|---|
| Unchecked | Transparent | `on_surface_variant`（BaseMedium） | 无 |
| UncheckedPointerOver | Transparent | `on_surface`（Highlight） | 无 |
| UncheckedPressed | `on_surface_variant` 35%（BaseMediumLow） | 无 | 无 |
| **Checked** | **强调色** | Transparent | 白 ✓ |
| CheckedPointerOver | 强调色提亮（Accent Light1） | — | 白 ✓ |
| CheckedPressed | 强调色压暗（Accent Dark1） | — | 白 ✓ |
| **Indeterminate** | **强调色** | Transparent | 白 — |
| Disabled | 上述 × disabled_opacity | 同上 | 白 × alpha |

### 交互

- 点击 → toggle。默认 **两态循环**（Unchecked ↔ Checked，UWP 语义）；
- `allow_indeterminate` = true 时**三态循环**（Unchecked → Checked → Indeterminate → Unchecked）。

---

## 37 · NumberBox（MetroNumberBox）

> 数据源：`reference/microsoft-ui-xaml/dev/NumberBox/`（NumberBox.cpp + NumberBox.xaml，**开源**）。

### 结构与尺寸

```
┌ Header（可选）───────────────────────────────────────────┐
│ ┌ 文本 ────────────────┬───┬───┬───┐                     │
│ │ 数字（Padding 10,6,6,5）│ ▲ │ │ ▼ │  ← Spin 区 72 宽    │
│ └──────────────────────┴───┴───┴───┘                     │
└──────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| MinWidth | **120**（NumberBoxMinWidth） |
| SpinButtonsColumn | **72**（SpinButtonsVisible 态） |
| SpinButton MinWidth | **32**（NumberBoxSpinButtonStyle） |
| 分隔线 | `NumberBoxSpinButtonBorderThickness` `0,1,1,1` → 两按钮间 1px |
| 步进 | SmallChange（默认 1）；Minimum/Maximum（默认 ±∞ = 无界） |
| 模式 | Compact（并排）/ Popup（弹层，首期仅 Compact） |

### 行为

- 上/下按钮 → `Value ± SmallChange`，**夹紧到 [Minimum, Maximum]**（RepeatButton 按住重复语义，Kanesumi 首期单击）。
- 文本编辑：仅接受数字/小数点（唯一）/负号（开头）；Enter/失焦 → 解析 + clamp 回值域。
- 聚焦 → 全选（TextBox 语义）；失焦 → 提交。

### Kanesumi 适配

- chevron 三角自绘（V7 不依赖 Segoe MDL2 glyph E70E/E70D）；
- 数字格式化：整数去小数点（`3` 而非 `3.0`）。

---

## 38 · AutoSuggestBox（MetroAutoSuggestBox）

> 数据源：`reference/microsoft-ui-xaml/dev/AutoSuggestBox/`（AutoSuggestBoxHelper.cpp + 模板，**开源**；
> Helper 只处理键盘导航，主体 TextBox 闭源）。

### 结构与尺寸

```
┌ Header（可选）─────────────────────────────────────────┐
│ ┌ TextBox ───────────────────────────┐                 │
│ │ 文本 / 占位                         │                 │
│ └────────────────────────────────────┘                 │
│ ┌ SuggestionsPopup（MaxH 300）───────────────────────┐  │
│ │  [建议项 40px，Padding 12]                          │  │
│ │  [建议项 40px]                                     │  │
│ └────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

| 项 | 值 |
|---|---|
| 建议列表 MaxHeight | **300**（AutoSuggestListMaxHeight，OS 值） |
| 建议项高 | **40**（ListViewItem，参 CONTROL_SPEC §7） |
| 建议项 Padding | **12** |
| 面板边框 | AutoSuggestListBorderThemeThickness **1**；底色 ChromeMediumLow → surface_variant |

### 行为

- 输入变化 → 过滤建议（`suggestions.contains(query)`）+ 展开弹层；空文本不弹（IsSuggestionListOpen false）。
- **键盘导航**：Up/Down 移动高亮（循环）；Enter 提交高亮项（无高亮则提交当前文本）；Esc 关闭。
- 点击建议项 → 提交 + 关闭。
- 高亮 = **中性**（参 §5 规律 5，非强调色）。

> Kanesumi 实现：建议源由宿主 `set_suggestions` 注入（纯逻辑过滤，上限 50 防爆栈）；
> 弹层复用 ListView 语义渲染。

---

## 39 · MenuFlyoutSubItem / RadioMenuFlyoutItem（MetroDropdownMenu 级联）

> 数据源：`reference/microsoft-ui-xaml/dev/RadioMenuFlyoutItem/`（RadioMenuFlyoutItem.cpp，**开源**）。
> MenuFlyoutSubItem 为 MenuFlyout 原生嵌套（闭源）；RadioMenuFlyoutItem 依赖级联容器。

### MenuFlyoutSubItem（二级级联）

- 顶层项带 `submenu: Vec<MenuItem>` → **悬停自动展开**二级面板（hover-swap：悬停其它项收起）；
- 子菜单面板 = **父项右侧 + 2px gap、垂直对齐**父项上缘；
- 子菜单项命中返回 `(parent, child)` 路径（`MenuPath`）；
- 渲染：父项右侧 chevron-right 指示；子菜单项同 MenuFlyout 规格（项高 32、悬停中性高亮）。

### RadioMenuFlyoutItem（单选组）

- 菜单项带 `radio_group: Option<String>` → 同组内**勾选互斥**（选一项自动取消其它）；
- 已勾选项重选不取消（UWP RadioMenuFlyoutItem 语义：`InternalIsChecked` 阻止用户取消）；
- 组名基于字符串；顶层与子菜单内均可出现（组作用域 = 所在菜单层）。

### 视觉

| 态 | 指示 |
|---|---|
| 未勾选 | 无勾选标记 |
| 已勾选 | 左侧/文本前勾选态（Kanesumi 以文本前缀表示，后续可换图标） |

---

## 40 · CommandBarFlyout（MetroCommandBarFlyout）

> 数据源：`reference/microsoft-ui-xaml/dev/CommandBarFlyout/`（CommandBarFlyout.cpp + 模板，**开源**）。

### 结构与尺寸

```
┌ [40×40][40×40][40×40][40×40] ┐   ← 横向命令条（TextCommandBarFlyout 默认四命令）
└──────────────────────────────┘     边框 1px、按钮 40×40、图标 16
```

| 项 | 值 |
|---|---|
| 按钮 | **40×40**（CommandBarFlyoutAppBarButtonStyleBase Width/Height=40） |
| 图标 | 16px 居中 |
| 边框 | CommandBarFlyoutBorderThemeThickness **1** |
| 底色 | 系统 chrome → Kanesumi `surface_variant` |
| 默认命令 | Copy / Cut / Paste / Select All（TextCommandBarFlyout） |

### 行为

- **选中文本时浮出**（TextCommandBarFlyout 语义）；默认贴选区**上方**水平居中；
  上方不足翻到下方；左右收拢不越屏。
- 按钮 PointerOver = 中性高亮（HighlightListLow 白 10%）。
- **无遮罩**（轻量工具栏不压暗背景，区别于 DropdownMenu）。
- 点命令按钮 → 返回动作（Copy/Cut/Paste/SelectAll）；点外关闭。

### Kanesumi 适配

- 命令图标用标准 Unicode 字形（⧉/✂/★ 等，思源黑体包含），不依赖 MDL2；

---

## 41 · Repeater（MetroRepeater 虚拟化布局引擎）

> 数据源：`reference/microsoft-ui-xaml/dev/Repeater/`（FlowLayout.cpp + ItemsRepeater.cpp，**开源**）。

### 定位

WinUI `ItemsRepeater` 是**元素工厂 + 回收复用**（59 个 cpp）。Kanesumi 状态驱动渲染
无保留视觉树、无 DOM 复用，移植其**虚拟化核心**：给定视口 + 滚动偏移，只计算
应渲染的条目范围与矩形 —— 长列表不再全量遍历/绘制，避免掉帧。

### 布局模式

| 模式 | 对应 WinUI | 行为 |
|---|---|---|
| **Stack** | `StackLayout` | 单轴等尺寸堆叠（横/纵） |
| **UniformGrid** | `UniformGridLayout` | 等宽网格（GridView / 磁贴） |

### 几何

| 项 | 公式 |
|---|---|
| 内容主轴长 | `item_count × (item_extent + spacing) − spacing` |
| 可见范围 | `first = floor(offset/stride)`；`last = ceil((offset+viewport)/stride) − 1`；滚过末尾 → None |
| 条目矩形 | 主轴位置 `index × stride − offset`；Grid 模式含列/行换算 |
| 命中 | 视口内点 + offset 还原 → 条目索引（空白间距不命中） |
| scroll_into_view | 已可见不动；在上方 → 滚到上缘；在下方 → 底边贴视口底（最小滚动） |

### 应用

- `MetroList` 已改用 `virtualizer()`（`visible_range` 只渲染视口内行）；
- TabView / TreeView / Grid 长列表可复用同一引擎。

---

## 42 · ScrollView / ScrollPresenter（MetroScrollView 滚动容器）

> 数据源：`reference/microsoft-ui-xaml/dev/ScrollView/`（ScrollView.idl + ScrollPresenter.cpp，**开源**）。

### 定位

WinUI `ScrollView`/`ScrollPresenter` 是富交互滚动容器（惯性、Chaining、Railing、缩放）。
Kanesumi 移植**纯状态 + 几何**：offset 夹紧、滚动条拇指/轨道几何、滚轮路由、
可选弹簧平滑滚动。宿主渲染内容时以 `content_offset` 平移 + 视口裁剪。

### 尺寸

| 项 | 值 |
|---|---|
| `max_offset` | `ExtentHeight − ViewportHeight`（内容超视口才可滚，对齐 ScrollableHeight） |
| 滚轮离散步 | **50px/格**（对齐合成器 Axis discrete） |
| 滚动条宽度 | **8px**（UWP ScrollBar 常规宽） |
| 拇指最小长 | **24px**（避免内容极长时拇指缩为点） |

### 行为

- `ScrollMode`：Auto / Enabled / Disabled（Disabled 阻塞滚动）；
- `ScrollBarVisibility`：Auto（可滚时显示）/ Visible / Hidden；
- 滚动条拇指：大小 = `视口/内容 × 轨道长`（下限 24），位置 = `offset/max_offset × 轨道长`；
- 平滑滚动 = sokuou `SpringAnim`（UWP 用 Composition 惯性，Kanesumi 弹簧等价）；
- `scroll_into_view(item_pos, extent)`：已可见不动 / 上方滚到上缘 / 下方底边贴视口底。

---

## 43 · Slider（MetroSlider）

> 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/Slider_themeresources_v1.xaml`
> （**闭源 B 类**，无 dev/Slider 目录；v1 = Metro 时代规格）。Kanesumi 移植**纯状态 + 几何**。

### 定位

连续数值输入（音量 / 亮度 / 色温）。`MetroSwitch` 是布尔离散，本控件补连续档。

### 尺寸

| 项 | 值 |
|---|---|
| 轨道高 | **2px**（SliderTrackThemeHeight） |
| 拇指 | **20×20**（SliderHorizontalThumbWidth/Height；Metro 时代无圆角 → Kanesumi 取 Capsule） |
| 水平整体 MinHeight | **32**（SliderHorizontalHeight） |
| 轨道上下留白 | 各 **15px**（SliderPreContentMargin / SliderPostContentMargin） |
| Header | 可选项；Margin **0,0,0,4**（SliderHeaderThemeMargin） |
| 默认 MinWidth | **120**（对齐 NumberBox MinW） |

### 颜色（Slider_themeresources_v1）

| 角色 | Metro 资源 | Kanesumi 映射 |
|---|---|---|
| 轨道底 | SliderTrackFill = SystemControlForegroundBaseMediumLowBrush | `surface_variant` |
| 轨道填充段 | SliderTrackValueFill = SystemControlHighlightAccentBrush | `primary` |
| 拇指 | SliderThumbBackground = SystemControlForegroundAccentBrush | `primary` |
| PointerOver 拇指 | SystemAccentColorLight1 | `primary`（Kanesumi 无 Light1，hover 不变色） |
| Pressed 拇指 | SystemAccentColorDark1 | `press_tint` 叠加 |
| Disabled | DisabledChromeDisabledHighBrush | 前景 0.38 alpha（通用铁律 §通用规律 3） |

### 行为

- 状态驱动：`value ∈ [min, max]`，`set_value` 夹紧；`fraction()` = `(value−min)/(max−min)`。
- **点击即跳**：`click(rect, pos)` 在轨道区按下 → 直接置值到点击 x（UWP 默认 Slider 点轨道跳值）。
- **拖动**：`press` 记录拖动中，`drag_to` 连续更新（按下时未命中拇指也吸到最近值）。
- 命中区 = 轨道矩形（含拇指 20×20 行程余量）：轨道 = 宿主 rect 内，左右各留 15px，
  y 向以轨道 2px 为中心上下各扩 15px（总 32 高）。
- 纯色无渐变，拇指 Capsule 圆角（轨道本身 2px 无圆角）。

---

## 44 · CandidateWindow（MetroCandidateWindow · IME 候选窗）

> 数据源：**无平台公开规格**（Win10 微软拼音「新体验」候选窗，微软未发布几何文档）。
> 几何/状态/交互由 Kanesumi 定夺，完整产品规格见 `CEYBOARD_SPEC.md` §Ⅲ/§Ⅳ。
> 本控件**纯展示**（内容注入 / 高亮态 / 翻页态），候选生成与选词逻辑全在引擎层（Ceyboard），
> 控件不持输入法状态。参 CEYBOARD_SPEC §Ⅷ「Kanesumi 只负责画，Ceyboard 负责想」。

### 定位

Ceyboard 候选词窗口（`ETHER_ROLE=candidate`，input-method popup surface）。
**横排单行候选**（微软拼音「新体验」横排模式）：候选词横向延伸，preedit 拼音
**不显示在候选窗**（内联文本字段，合成器 text-input 桥接）。每页 9 项。

### 形状（CEYBOARD_SPEC §Ⅲ.1）

| 项 | 值 |
|---|---|
| 圆角 | **`Square`（直角）** |
| 边框 | **无**（transparent） |
| 阴影 | **无**（深度靠明暗） |
| 面板底色 | `surface`（不透明） |
| 透明度 | 不透明（无磨砂） |

### 结构（横排单行）

```
┌──────────────────────────────────────────┐
│ 1.你好  2.尼豪  3.泥蒿  4.逆好  …   ›    │
└──────────────────────────────────────────┘
```

- 每候选 = 序号 + 词横向排列；高亮项以 `primary` 色块包裹。
- preedit 不占候选窗空间。

### 尺寸（CEYBOARD_SPEC §Ⅲ.3）

| 项 | 值 | 状态 |
|---|---|---|
| 面板左右内边距 | 8px | 定 |
| 面板上下内边距 | 4px | 定 |
| 候选行高 | 32px | 定（对齐 Slider 32 / 列表行高惯例） |
| 序号宽 | ~10px（+2px 间隙） | 定 |
| 候选词字号 | 16px（body_large） | 参考 |
| 候选间距 | 10px | 定 |
| 面板最大宽度 | 480px（超省略） | 参考 |
| 面板高度 | 单行 40px | 定 |

### 颜色与状态（CEYBOARD_SPEC §Ⅲ.4）

| 态 | 序号 | 候选词前景 | 行背景 |
|---|---|---|---|
| Normal | `on_surface` 50% | `on_surface` | 透明 |
| Highlight | `on_primary` | `on_primary` | **`primary`** |
| Pressed | — | — | `primary` + press_tint |

preedit 不显示在候选窗（拼音内联文本字段）；高亮块 = 序号 + 词一体 primary 底。
颜色切换 = **瞬时硬切换**（通用规律 1：`DiscreteObjectKeyFrame`，无颜色过渡）。

### 动画（CEYBOARD_SPEC §Ⅲ.5）

| 场景 | 动画 |
|---|---|
| 弹出 | `Progress` 驱动，0.25s Quadratic/EaseOut |
| 高亮切换 / 翻页 | 瞬时 |
| 关闭 | Fade out 0.2s（可选） |

> 铁律：动画只动视觉属性（位移/透明），不动布局；进度驱动，无时间线（AnimRules §III）。

### 行为（CEYBOARD_SPEC §Ⅳ）

- 键盘：数字 1–9 选第 N 候选；空格选高亮；回车提交高亮；↑/↓ 移高亮（边界翻页）；
  PageUp/PageDown 或 +/− 翻页；Esc 取消组合态。
- 鼠标：左键点候选项提交；滚轮翻页。
- 位置：锚定文本字段光标矩形（parent_geometry + popup 光标矩形）；空间不足向上翻；
  位置随光标即时更新。
- 密码字段（content_hint = Password）：不弹候选窗、不外发周边文本。

### 控件 API（纯展示）

```
MetroCandidateWindow {
    candidates: Vec<String>    // 候选词（一页 ≤ 9，横排单行）
    highlighted: Option<usize> // 高亮下标
    page: usize                // 当前页（0-based）
    has_prev / has_next        // 翻页指示
    open: bool                 // 可见性（弹层开关）
    // render(theme, engine, rect, scene)
    // hit_candidate(rect, pos) -> Option<usize>
    // popup_size() -> Size（横排单行，供 popup surface 定位）
    // item_width(i) -> f32（单候选宽估算）
}
```

- 内容注入：引擎层（Ceyboard）填充 candidates / highlighted / page；
  控件只画 + 命中，不产生候选。选词/翻页由引擎层把结果写回本控件状态。
- **无 preedit 字段**：拼音内联文本字段（合成器 text-input 桥接），候选框只显示候选词。

### Kanesumi 适配

- 深色桌面 `surface`；高亮用 `primary`（列表类「选中用强调色」，通用规律 5）。
- CJK 安全：序号 = 数字键 1–9，候选词 UTF-8 边界不劈码点。
- 横排单行：面板最大宽度 480px，超出右缘省略截断（`TextOverflow::Ellipsis`）。

---


