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
