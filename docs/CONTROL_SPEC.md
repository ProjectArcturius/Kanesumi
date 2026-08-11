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
