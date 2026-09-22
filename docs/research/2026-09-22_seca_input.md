# sec-a → 主仓 回流可行性评估 · **输入框族**

> 范围：`MetroTextField` / `MetroChatInputBar` / `MetroDrawer` / `sample/MainActivity.kt` 的真实用法
> ↔ 主仓 `text_box.rs` / `text_field.rs` / `password_box.rs` / `auto_suggest_box.rs` / `ime.rs` / `kanesumi-canvas/src/text.rs`
> 取证日：2026-09-22。sec-a 工作树 clean；主仓工作树有**一处先前存在的未提交改动**
> `M kanesumi-controls/src/switch.rs`（非本次产生，本文未触碰）。本次**只读源码**，未改任何源文件，未跑 cargo/gradle。
>
> 证据格式：`sec-a:<file>:<line>` / `主仓:<file>:<line>`。路径省略两侧共同的
> `kanesumi-controls/src/main/java/io/github/takahashirinta/kanesumi/controls/` 前缀（简写 `kt/`）。

---

## §1 结论先行

**一句话：sec-a 的输入框族几乎没有「实现」可回流 —— 它的价值是「把框架免费给的东西正确地没做」，而主仓是自绘栈，缺的恰恰是那些「免费的东西」。可回流物是三组数值常量（几何/行数/动画时长）、一个聊天条布局范式、一套 IME 垫高机制；不可回流的是 `BasicTextField` 背后的一整套编辑内核。**

> **回流机制已有先例（本报告的方法论基准）**：同一天（2026-09-22）主仓已从 sec-a 回流过一次 ——
> `switch.rs` 的两档释放决策：sec-a `MetroSwitch.kt:118-125`（`displacement < 0.15f → tap 意图翻转`，否则 `>= 0.5f` 就近吸附）
> → 主仓 `kanesumi-controls/src/switch.rs:91`（KDoc 明写「来源：sec-a（Android 扇区）`MetroSwitch.kt` L118-125，2026-09-22 回流」）
> + 回归测试 `switch.rs:558-578`。**该次回流的工作树改动当前未提交**（`git status` 显示 `M kanesumi-controls/src/switch.rs`，
> 104 插入 / 14 删除；本报告未触碰该文件）。它证明了两点：**「sec-a 的指针/手势决策逻辑」是可回流的**，
> 而**「sec-a 的框架免费能力」不是** —— 输入框族恰好几乎全部属于后者，这就是本报告结论偏保守的原因。

1. **体量对比是压倒性的**：`kt/MetroTextField.kt` 全文 **80 行**（含 KDoc），
   `主仓:kanesumi-controls/src/text_box.rs` **893 行**（含测试，渲染实现 518 行）。
   sec-a 的输入框 = `Box.background(containerColor).padding(12.dp)` + `BasicTextField` 两件套
   （`kt/MetroTextField.kt:53-79`），**没有边框、没有状态、没有选区绘制、没有光标绘制、
   没有删除键、没有 IME 组合态** —— 因为它全部由 Compose 免费提供。
   回流不可能是「把 Kotlin 翻成 Rust」，只能是「把 Compose 免费提供的那部分，在主仓补出来」。

2. **`MetroTextField` 目前在 sec-a 里是死代码**。全仓（含 `:sample` 全部 Kotlin）
   grep `MetroTextField` 只有 2 处命中：自己文件里的定义与 KDoc（`kt/MetroTextField.kt:34`
   定义、`kt/MetroTextField.kt:25` 注释），以及 `kt/MetroChatInputBar.kt:61` 的唯一调用者。
   `sample/src/main/java/.../sample/MainActivity.kt` **零命中** —— 第 349 行的
   `FormControlsShowcase()`（TabRow + Switch + IconButton，`MainActivity.kt:349-400`）
   并不含任何输入框。也就是说：**sec-a 侧对输入框的「用户手感验证」其实没做过**
   （`MetroTextField` 的 KDoc 说「来自 Sakichan 实战」，但本仓内无消费证据）。
   用户说的「输入框也挺舒服」应被理解为**对 sec-a 版「无边框方底 + 光标即状态」这种极简观感的评价**，
   而不是对某段可搬逻辑的评价。**这一点直接改变可回流清单的规模：能搬的不是控件，是判断。**

3. **两侧的取舍方向相反，而且各有各的缺口**：
   - sec-a 选择「**无边框**」——`kt/MetroChatInputBar.kt` 与 `MetroTextField` 都不画描边；
     这是 sec-a 设计铁律「无边框（除非信息需要）」（sec-a `AGENTS.md:86`）的直接后果；
   - 主仓选择「**有边框 + UWP 状态表**」——`主仓:.../text_box.rs:416-428` 已按一手源实现
     Normal/Hover/Focus 三态描边；
   - **两边都不完整**：sec-a 没有状态视觉，主仓的 Hover 态**在 Gallery 里从未被触发**
     （`主仓:kanesumi-gallery/src/app.rs:1097-1117` 的 `match target` 不含 `Target::TextBox`，
     `main.rs` 级别也没有别处设置 `textbox.state`），所以 `975a44e` 这次「悬停边框改
     BaseMedium 实色」是**正确但当前不可见**的改动。

4. **回流的真实收益不在控件本身，在三条「已经想清楚但没落地」的机制**（详见 §3/§7）：
   单行滚动的裁剪闭环（在 `auto_suggest_box.rs` 已实现、`text_box.rs` 反而缺）、
   点击热区/避让类的可用性常量、以及**软键盘/IME 垫高**（sec-a 有一半机制、主仓完全无此概念）。

5. **净裁定**：可回流条目 **11 条**（a 类可直接搬 5 / b 类需改造 5 / c 类不可搬 1 组），
   其中**真正值得本周做的只有 3 条**（§7），其余属「登记待办」。**不存在「sprint 级搬运」的可能**。

---

## §2 逐项对照

### 2.1 结构

| 项 | sec-a（Android/Compose） | 主仓（Rust 自绘） | 判定 |
|---|---|---|---|
| 外框 | 无外框。只有 `Box.background(containerColor)`，`containerColor` 默认 `surfaceVariant`。`kt/MetroTextField.kt:53-57`（`.background(containerColor).padding(contentPadding)`） | 有底色 + 有描边。底色 `colors.surface`（`text_box.rs:301-302`）、描边 `stroke_rounded_rect`（`text_box.rs:428`） | **不同取向**；sec-a 的「无描边」不回流（主仓 §34 规格要求描边），但**底下那句「光标是唯一的状态反馈」的取舍值得登记** |
| 标签 / 浮动标签 | **完全没有**。`BasicTextField(value, onValueChange, modifier, enabled, readOnly, textStyle, singleLine, maxLines, minLines, visualTransformation, cursorBrush)` —— 11 个参数里无 `label`、无 `decorationBox`（`kt/MetroTextField.kt:66-78`）。文件头 KDoc 明确拒绝：*「取代 M3 TextField / OutlinedTextField：它们带圆角边框 + floating label + 聚集态变色，Metro 不需要」*（`kt/MetroTextField.kt:25-28`） | **静态 Header**（非浮动）。`header: String` 字段（`text_box.rs:34`），`body_rect()` 从顶部扣掉 `style.line_height + 4.0`（`text_box.rs:504-517`，注释写 `TextBoxTopHeaderMargin 下 4`） | sec-a 的「拒绝浮动标签」= 主仓已有结论；主仓比 sec-a 多一个 Header，**sec-a 无回流物** |
| 下划线 / 边框 | 无（同上） | `border_thickness()`：聚焦 **2.0**，其余 **1.0**（`text_box.rs:166-168`）；三态取色见 §2.2 | **主仓内部与规格不一致**，见 §5-U1 |
| 计数器 | 无 | 无 | 两侧都没有；不构成回流项 |
| 清除键 | **无**。KDoc：*「图标 / 前后缀 / 错误态都不内置 —— 没有真实用例驱动，避免预抽象。需要时用 Box 包一层自己拼」*（`kt/MetroTextField.kt:30-31`） | **有几何、有渲染、默认关**。`TEXTBOX_DELETE_BUTTON_W = 34.0`（`text_box.rs:20`）；`delete_button_rect()`（`text_box.rs:155-163`）；渲染 `if self.show_delete && has_text && !disabled`（`text_box.rs:394-414`）；`show_delete` 默认 `false`（`text_box.rs:55`）且**全仓无任何处置 true** | **两侧都缺**。sec-a 的「先不做」与主仓的「做了不接」是同一个洞的两个面：主仓缺的是**一个自动策略**（UWP ButtonStates：有内容 + hover/focus → 显示），落点 `text_box.rs` 内部即可 |
| 错误提示 | 无 | 无（`ControlState` 有 `Disabled` 但**没有 `Error`**，`主仓:kanesumi-controls/src/state.rs:5-11`） | **两侧都缺**；sec-a 无可借鉴 |
| 前缀 / 后缀图标 | 无（`MetroTextField` 层面）。**但 `MetroChatInputBar` 有「后缀动作键」范式**：`sendIcon: ImageVector?` + `sendText: String = "发送"` 双形态（`kt/MetroChatInputBar.kt:43-45`、`74-91`） | Threading 无；但 AutoSuggestBox/NumberBox 有右侧列（`number_box.rs` Spin 区、`auto_suggest_box.rs`） | **sec-a 的「图标可选、文字兜底」二分法可搬**（见 §3-a2） |
| **高度取值** | `contentPadding = PaddingValues(horizontal = 12.dp, vertical = 12.dp)`（`kt/MetroTextField.kt:46`）；`BasicTextField` 自身 `heightIn(min = 20.dp)`（`kt/MetroTextField.kt:69`）→ **单行自然高 ≈ 12+22+12 = 46dp**（`body` 行高 22sp，`kt/../../core/theme/MetroTypography.kt:25`） | UWP Padding **`10,6,6,5`**（左10/上6/右6/下5，`text_box.rs:142-145`）；`min_height() = 32.0`（`text_box.rs:170-173`，**但全仓无调用点**，是死常量）；`padding` 直接构建于 `content_rect()` | **数值不同且不可互换**：sec-a 是触屏基线（46dp），主仓是 UWP 桌面基线（32pt）。回流结论：**不要搬 12dp，要搬「padding 与行高联动的算法」**（见 §3-a3） |
| **字号取值** | 不硬编码 —— 从主题取 `LocalMetroTypography.current.body`（`kt/MetroTextField.kt:45`）→ **15sp / lineHeight 22sp**（`kt/../../core/theme/MetroTypography.kt:25`） | `theme.typography.body`（`text_box.rs:266`），ControlContentThemeFontSize = 14 | 结构相同（都取主题），**sec-a 无可回流物**；数值差异属平台基线差异，不裁定 |

**关键代码（sec-a 全部结构实现，照抄）：** `kt/MetroTextField.kt:53-79`
```kotlin
    Box(
        modifier = modifier
            .background(containerColor)
            .padding(contentPadding),
        contentAlignment = Alignment.CenterStart,
    ) {
        if (value.isEmpty() && placeholder.isNotEmpty()) {
            MetroText(
                text = placeholder,
                color = colors.onSurfaceVariant,
                style = mergedStyle,
            )
        }
        BasicTextField(
            value = value,
            onValueChange = onValueChange,
            modifier = Modifier.heightIn(min = 20.dp),
            enabled = enabled,
            readOnly = readOnly,
            textStyle = mergedStyle,
            singleLine = singleLine,
            maxLines = maxLines,
            minLines = minLines,
            visualTransformation = visualTransformation,
            cursorBrush = SolidColor(cursorColor),
        )
    }
```

**主仓结构实现对应段（照抄）：** `主仓:.../text_box.rs:291-302` + `416-428`
```rust
        let body_rect = self.body_rect(theme, rect);
        let b = self.border_thickness();
        let inner = Rect::new(
            body_rect.origin.x,
            body_rect.origin.y,
            (body_rect.size.width - 2.0 * b).max(0.0),
            (body_rect.size.height - 2.0 * b).max(0.0),
        );

        // 底色（UWP TextControlBackground；Kanesumi = surface）
        let bg = colors.surface.with_alpha(colors.surface.a * alpha);
        scene.fill_rounded_rect(bg, inner, theme.tokens.corner_radius);
```
```rust
        // 边框（Focused 2px + focus 色；其余 divider 1px）
        let (stroke, stroke_w) = if self.focused {
            (colors.focus_stroke.with_alpha(alpha), 2.0)
        } else if self.state == ControlState::Hovered {
            // 悬停边框 = UWP `TextControlBorderBrushPointerOver`
            // → `SystemControlHighlightBaseMediumBrush` → `SystemBaseMediumColor`
            // （themeresources L855 → L298 → L212）= 60% 基色、**不透明**。
            // 本库 `on_surface_variant` 即该档的实色对应物；旧实现再乘 0.9 无依据。
            (colors.on_surface_variant.with_alpha(alpha), 1.0)
        } else {
            (colors.divider.with_alpha(alpha), 1.0)
        };
        scene.stroke_rounded_rect(stroke, inner, stroke_w, theme.tokens.corner_radius);
```

---

### 2.2 状态与视觉（Focus / Hover / Press / Disabled / Error）

**sec-a 侧：状态表不存在。**

| 态 | sec-a 的表现 | 证据 |
|---|---|---|
| Normal | `Box.background(surfaceVariant)` + 文字 `onSurface`（`0xFFF0F0F0`） | `kt/MetroTextField.kt:55`、`:51`；`kt/../../core/theme/MetroColors.kt:10,15` |
| Hover | **无代码**。控件未取 `InteractionSource`，无 `hoverable`，无 Hover 视觉 | grep `MetroTextField.kt` 无 `interactionSource` / `HoverInteraction` |
| Press | **无代码**（输入框本身不消费按压反馈；`MetroIndication` 只在 `.clickable {}` 上生效，`BasicTextField` 走的是自己的指针路径） | `kt/../../core/theme/MetroTheme.kt:21-25` 只注入 `LocalIndication`；`BasicTextField` 无 `indication` 参数 |
| Focused | **只有光标**。`cursorBrush = SolidColor(cursorColor)`，`cursorColor` 默认 `colors.primary`（`kt/MetroTextField.kt:48`、`:77`）。**底色、边框、占位一律不变** | 同上；KDoc 原话：*「光标是唯一的状态反馈」*（`kt/MetroTextField.kt:27`） |
| Disabled | 前景色降级：`textStyle.copy(color = if (enabled) onSurface else onSurfaceVariant)`（`kt/MetroTextField.kt:51`），并把 `enabled` 透传给 `BasicTextField` | `kt/MetroTextField.kt:51,71` |
| Error | **无** | 无字段、无参数 |
| 占位 | 恒定 `onSurfaceVariant`（`0xFF9AA0A6`），**聚焦时不变化**（`kt/MetroTextField.kt:59-64`） | 同上 |
| 选区 | **不设** `LocalTextSelectionColors`（`MetroTheme` 的 `CompositionLocalProvider` 只 provide 三项，`kt/../../core/theme/MetroTheme.kt:22-26`）→ 走 Compose 框架默认选区色 | 见 §5-U2（默认值未在本机验证） |
| 光标 | 由 `BasicTextField` 内部闪烁（框架节奏），库不控制 | 无代码可引 |

**主仓侧：状态表已按一手源实现，但有一处未按规格、一处未接线。**

| 态 | 主仓实现 | 一手源 / 规格 | 一致性 |
|---|---|---|---|
| Normal 边框 | `colors.divider` @ **1.0px**（`text_box.rs:426`） | `TextControlBorderBrush` → `BaseMediumLow` = **40% 实色**、粗细 **2px**（`docs/UWP_PRIMARY_SOURCES.md:98`；`docs/CONTROL_SPEC.md:1166`） | ⚠ **不一致**（见 §5-U1） |
| Hover 边框 | `colors.on_surface_variant` @ **1.0（实色）**（`text_box.rs:424`） | `…PointerOver` → `HighlightBaseMedium` = **60% 实色**、2px（`UWP_PRIMARY_SOURCES.md:99`） | ✅ **颜色已对**（`on_surface_variant` = `#9AA0A6` 即 60% 档实色，「不乘 alpha」正是本次 `975a44e` 修正）；粗细仍 1px |
| Focus 边框 | `colors.focus_stroke` @ **2.0px**（`text_box.rs:417-418`） | `…Focused` → accent、粗细**仍 2px**（`UWP_PRIMARY_SOURCES.md:100`） | ✅ 一致 |
| Focus 底色 | `colors.surface`（`text_box.rs:300-302`） | UWP 是 `#FFFFFFFF` **纯白**（暗色下「变白纸」+ 切 Light）（`UWP_PRIMARY_SOURCES.md:97`；`CONTROL_SPEC.md:1168`） | ❌ **未采用**，属**有意偏离**，已登记 `docs/CANON_VS_TEMPORARY.md:73`（D10）。**sec-a 同样未采用**（它的聚焦底也不变） |
| Focus 占位 | `on_surface_variant × placeholder_focused_opacity(0.7)`（`text_box.rs:354-362`） | UWP Focused 占位 = 黑 40%，但建立于白纸底，**不可直接移植**（`CANON_VS_TEMPORARY.md:35` T16） | ⚠ 待裁定（T16 前置：是否接受白纸行为） |
| Disabled | 整体 `× disabled_opacity(0.38)`（`text_box.rs:267-272`、`:301`、`:347`…） | UWP 无通用禁用不透明度档；0.38 属 Kanesumi 取值（`CANON_VS_TEMPORARY.md:23` T4） | ✅ 已登记为 Kanesumi 取值 |
| Press | **无**。`ControlState::Pressed` 存在（`state.rs:8`）但 `text_box.rs` 的 render **不读它** | UWP TextBox 无按压态视觉（模板内 CommonStates 只有 Normal/PointerOver/Focused/Disabled） | ✅ 一致，**不是缺口** |
| Error | 无 | UWP TextBox 有 `ValidationErrors`（闭源）；`docs/CONTROL_SPEC.md:1138-1196` 未含 | 两侧皆无 |
| 选区 | `colors.text_selection_tint` = `accent.base.with_alpha(0.35)`（`text_box.rs:306-326`；`kanesumi-core/src/colors.rs:89`） | UWP = `HighlightAccentBrush` = accent **100%**（`UWP_PRIMARY_SOURCES.md:102`） | **有意偏离候选**，登记 T19（`CANON_VS_TEMPORARY.md:38`） |
| 光标 | `on_surface × base_medium_high(0.8)`（`text_box.rs:385-390`） | — | ⚠ `0.8` 档取自 BaseMediumHigh（`indicator.rs:75`）——**光标用 80% 而非 100% 无一手依据**，见 §5-U3 |

**结论（本项最重要的一条）：用户注意到的「舒服」在 sec-a 侧来自「状态不变 / 只有光标」；主仓已经把状态做多了（三态描边），但照 §2.2 表，主仓的落差集中在「**边框粗细 1px vs 规格 2px**」和「**Hover 态在 Gallery 未被触发**」两处 —— 也就是说，**sec-a 在这件事上没有可抄的东西，反而提供了「少即是多」的反证**。**

---

### 2.3 文本编辑语义：哪些是框架免费的，哪些是自写的

**这是本题的核心。下表左侧结论均可由「sec-a 侧对应的 Kotlin 代码确实不存在」反证 ——
`MetroTextField.kt` 全文 80 行、无一行编辑逻辑；`MetroChatInputBar.kt` 全文 93 行、无一行编辑逻辑。**

| 语义 | sec-a 状态 | sec-a 证据 | 主仓状态 | 主仓证据 |
|---|---|---|---|---|
| 光标（插入点） | **框架免费** | 无代码；`cursorBrush` 只是颜色（`kt/MetroTextField.kt:77`） | **自写** | `text_box.rs:205-218`（`caret_x` / `caret_rect_absolute`）+ `:379-391`（渲染）；`text_field.rs:119-121`（`cursor()`） |
| 选区 | **框架免费** | 无代码 | **自写** | `text_field.rs:129-141`（规范区间）；`text_box.rs:306-326`（渲染）；`text.rs:119-157`（`selection_spans` 按视觉 cluster + RTL 拆分） |
| 点击定位光标 | **框架免费** | 无代码 | **自写** | `text_box.rs:238-253`（`place_caret_at` → `geometry.caret_at_x`）；`text.rs:102-116`（`caret_at_x`：命中最近光标、中点偏后） |
| 双击选词 | **框架免费** | 无代码 | ❌ **缺**。harness 有 `InputEvent::DoubleClick`（判定同按钮 / ≤250ms / ≤5px，`kanesumi-harness/src/app.rs:190-197`、`:576-577`），但 Gallery 显式忽略：*「双击：演示无特殊语义（按下已照常处理），忽略」*（`kanesumi-gallery/src/app.rs:2155-2156`） | 两侧差距最大的一条 |
| 长按 | **框架免费**（Android 长按 → 选中 + 手柄；`BasicTextField` 内置） | 无代码 | ❌ **缺**。也无长按判定器（`ClickTracker` 只做双击） | 同上 |
| 拖拽选择 | **框架免费** | 无代码 | ❌ **缺**。无 drag-select 状态机；`place_caret_at` 只在 press 调用（`kanesumi-gallery/src/app.rs:1286-1287`） | — |
| 撤销 | **框架免费**（`BasicTextField` 内部 `UndoManager`） | 无代码 | **自写** | `text_field.rs:47-50,287-311`（快照栈，`max_undo = 64`）；Gallery 接 `Ctrl+Z`：`kanesumi-gallery/src/app.rs:2073-2078` |
| 重做 | 框架免费（同上） | 无代码 | ❌ **缺**。`TextField` 只有 `undo()` / `can_undo()`，**无 `redo()`**（grep `redo` on `kanesumi-controls/src` 零命中） | — |
| 剪切 / 复制 / 粘贴 | 框架免费（系统剪贴板 + 手柄菜单） | 无代码 | **半自写**。Gallery 有应用内剪贴板：`Ctrl+C` 取 `selection()` 拼串、`Ctrl+V` 走 `insert_str`（`kanesumi-gallery/src/app.rs:2079-2098`），**注释明写「跨应用待 data_device」** | — |
| 全选 | 框架免费 | 无代码 | **自写** | `text_field.rs:448-456`；Gallery `Ctrl+A`（`app.rs:2067-2072`） |
| IME 组合态 preedit | **框架免费**（`PlatformTextInputService` 全链路） | 无代码 | **自写，且已完整落地** | `text_field.rs:53-55,154-213`（`preedit` / `preedit_cursor` / `set_preedit` / `commit_ime` / `delete_surrounding` / `surrounding_text`）；渲染 `text_box.rs:180-202,431-501`（整行塑形 + dash 下划线）；契约 `ime.rs:22-49`；harness 接线 `kanesumi-harness/src/platform.rs:385-393,2525-2660`；Gallery `ime_focus()` `app.rs:1894-1900` |
| 软键盘类型 | **框架免费但未使用**。`MetroTextField` **不暴露** `KeyboardOptions` / `KeyboardType` / `ImeAction` —— grep 全仓 `KeyboardOptions`/`keyboardType`/`ImeAction`：**0 命中** | 无代码 | **无此概念**；最接近的是 `ImeContentHint::{Normal,Password,Digits}`（`ime.rs:10-19`），由 harness 映射为 `content_purpose` | sec-a 的 `Digits` 与 `Password` 已在主仓有对应；sec-a 未用而主仓已做 |
| 掩码（密码） | 框架免费（`VisualTransformation` —— `MetroTextField` 透传参数，`kt/MetroTextField.kt:44,76`；**但库内无 `MetroPasswordField`**） | `kt/MetroTextField.kt:44` | **自写** | `text_field.rs:45,105-116,148-151,187-193`（`mask` / `display_chars` / `preedit_display`）；`password_box.rs` 全文件（含「绝不渲染明文」测试 `:173-192`） |
| 焦点级联 | 框架免费（`FocusManager`） | 无代码 | **自写**（App 层手写 `FocusedInput` 枚举 + 互斥 blur） | `kanesumi-gallery/src/app.rs:1273-1318`（每个分支手写 `self.textbox.blur(); self.password.blur(); …`） |

**§2.3 的推论：** sec-a 在编辑语义上**零可回流物** —— 它的 80 行里没有任何一行能翻译成 Rust。
但它反证了主仓的三处真缺口：**双击选词、拖拽选择、redo**。这三条都不是「sec-a 有而主仓没有」，
而是「Compose 有而自绘栈必须自己写」。

---

### 2.4 滚动与单行 / 多行

| 项 | sec-a | 主仓 |
|---|---|---|
| 单行水平滚动 | **框架免费**。`BasicTextField(singleLine = true)` 内建：光标移出视口时把文本整体平移，并把内容裁进控件矩形（`kt/MetroTextField.kt:73`）。库侧无一行滚动代码 | **自写但未完成**。`scroll: f32` 字段存在（`text_box.rs:42`）、被 `caret_x`/`place_caret_at`/渲染消费（`:210,248,312,336,340,451,456`），**但唯一的写入者 `ensure_caret_visible()` 是空实现**：<br>`fn ensure_caret_visible(&mut self) { self.reset_blink(); }`（`text_box.rs:257-259`，注释自承「内容宽 / 光标 x 由调用方（App 层）结合视口宽度驱动 `scroll`」）<br>→ **Gallery 从未驱动它** ⇒ `scroll` 恒 0 ⇒ **长文本在主仓输入框里会画出框外** |
| 「正确的做法」在哪 | — | **同仓已有范本**：`auto_suggest_box.rs:299-309` 每次 render 自算 `self.scroll = text_w - view_w`（`engine.measure` vs `content.size.width`），并用 `scene.push_clip(content)`（`:309`）裁进框内；KDoc 明写这是修复「长文本不在框内 / 换行溢出框」的回归（`:264-266`）。**`text_box.rs` 没有 `push_clip`**（grep `push_clip` on `text_box.rs` 零命中） |
| 多行纵向增长 | `singleLine=false` + `maxLines` + `minLines` 三参数透传（`kt/MetroTextField.kt:41-43,73-75`）：`maxLines = if (singleLine) 1 else Int.MAX_VALUE`（默认无限），`minLines = 1`；**高度由 `BasicTextField` 的测量自然增长**，父容器不裁剪（Compose 的 `Box` 不裁） | ❌ **完全不支持**。`MetroTextBox` 头部注释：*「MetroTextBox —— **单行**文本输入框」*（`text_box.rs:1`）；渲染全程用 `engine.line_geometry(...)`（单行几何，`text_box.rs:209,250,309,462`），无 `layout_box`/`wrap` 调用；`TextInputKey::Enter` 明确不插入（`text_field.rs:467-470`）。**多行排版能力在 canvas 层已备好**（`TextLayoutOptions{ max_lines, wrap, overflow }`，`kanesumi-canvas/src/text.rs:20-43`），只是 TextBox 未消费 |
| 最大行数 | **5**（聊天条） | 无 |
| 与容器的裁剪关系 | 框架保证：Compose 的文本绘制裁进 `BasicTextField` 布局边界 | **无保证**。`text_box.rs` 不 `push_clip`；`docs/COMPOSITION.md:37-39` 要求「文本必须声明 wrap/max lines 与 Clip/Ellipsis」「只允许 PushClip/PopClip 成对使用」—— **`MetroTextBox::render` 未遵守该契约** |

**聊天条的多行策略（sec-a 唯一的具体数值，照抄）：** `kt/MetroChatInputBar.kt:61-71`
```kotlin
        MetroTextField(
            value = text,
            onValueChange = onTextChange,
            modifier = Modifier
                .weight(1f)
                .heightIn(min = 40.dp),
            placeholder = placeholder,
            enabled = enabled,
            singleLine = false,
            maxLines = 5,
        )
```
→ **`min 40dp` + `maxLines 5` + `weight(1f)`** 的组合，是「输入条随内容长高、上限 5 行后内部滚动」的完整策略。
主仓要抄的正是这三元组（见 §3-a4 / §7-①）。

---

### 2.5 可用性细节（用户说「舒服」的具体机制）

> 逐条核查「sec-a 到底做了什么」。**结论：多数「舒服」来自框架，少数来自 sec-a 的数值选择，
> 且 sec-a 侧至少有一处明显遗漏（IME 垫高）。**

| 机制 | sec-a 的实现 | 证据 | 主仓 |
|---|---|---|---|
| 光标闪烁节奏 | **不由库控制** —— `BasicTextField` 内建（Decorative 闪烁），库只给 `cursorBrush` 颜色 | `kt/MetroTextField.kt:77`（无节奏参数） | **自写且参数明确**：`CARET_BLINK_HALF_PERIOD = 0.5`（on/off 各 0.5s）（`text_box.rs:23-24`），`update(dt)` 累加 `blink_phase`（`:113-117`），`caret_visible()`（`:119-122`）；内容变化后 `reset_blink()` 让光标立刻亮起（`:131-134`，注意：**`ensure_caret_visible()` 目前只做这件事**）。Gallery 每帧 `self.textbox.update(dt)`（`kanesumi-gallery/src/app.rs:2000`） |
| 点击热区 | **= 整个 `Box`**（`background`+`padding` 参与命中），即 **46dp 高 × 全宽**，远大于文字行高 22dp | `kt/MetroTextField.kt:53-57` | **= 整个传入 `rect`**：`hit_test(&self, rect, pos) -> rect.contains(pos)`（`text_box.rs:233-236`）。**等价，无可回流物** |
| 防误触 | 无特殊处理（触屏场景由 Compose 处理） | — | **无**。TextCommandBarFlyout 是唯一的「防误触」相关机制，且在 App 层：仅当**有非空选区**才浮出（`kanesumi-gallery/src/app.rs:2002-2015`；KDoc 记录 V22 修复：旧实现「聚焦即弹命令条」导致「用户仅想聚焦 → 光标被抹掉 + 命令条遮挡输入区」，`:1274-1279`） |
| 动画时长 / 缓动 | 输入框族**无动画**（`MetroTextField`/`MetroChatInputBar` 零 `Animatable`/`animate*`）。**`MetroDrawer` 是唯一带动画的**：入场 `SokuouTweens.SheetAppear`（**300ms**，`CubicBezierEasing(0.2f, 0f, 0f, 1f)`），收起 `SokuouTweens.SheetDismiss`（**260ms**，`FastOutSlowIn`） | `kt/MetroDrawer.kt:60-69`；预设定义 `kt/../../anim/sokuou/Sokuou.kt:105-106` | 主仓文本控件族同样无动画；`SheetAppear 300ms` 与主仓 `CommandBarFlyout` 的 **300ms** 一致（`UWP_PRIMARY_SOURCES.md:108`），`SheetDismiss 260ms` 与一手源 **150ms** 不符（同记录，`eb9daf9` 已把主仓改成 150ms） |
| 按压反馈 | 库级：`MetroIndication` —— 直角矩形闪切，`enter 100ms` / `exit 200ms`，均 `EaseOutCubic`，**无 ripple / 无扩散 / 无 elevation**，`Modifier.Node` + draw phase 读 alpha（零重组） | `kt/../../core/theme/MetroIndication.kt:32-36,64-84` | 主仓输入框无按压反馈（与 UWP 一致，见 §2.2）；该机制**对本族不适用** |
| **键盘弹出避让（insets）** | ⚠ **半成品**。`rememberMetroInsets()` **读了** `WindowInsets.ime`（`kt/../../core/insets/MetroInsets.kt:60,70`），`sample` 也把它显示出来了（`MainActivity.kt:697`：`"  ime.bottom = ${fmt(insets.ime.dp)}"`）——**但全仓无任何组件消费 `insets.ime`**。`MetroChatInputBar` 挂的是 **`metroNavigationBarsPadding()`**（`kt/MetroChatInputBar.kt:56`），**不是** IME padding；grep 全仓 `imePadding`：**0 命中** | `kt/MetroChatInputBar.kt:56-57`；`kt/../../core/insets/InsetModifiers.kt:9-15`（只提供 status/navigation/systemBars/displayCutout 四个包装，**无 IME 包装**） | **主仓完全无此概念**。Wayland 侧对应物是 `zwp_text_input_v3` 的 `set_cursor_rectangle`（主仓已实现，`ime.rs:33` / `text_box.rs:214-231`）→ 由 **合成器/IME** 决定候选窗位置；但**屏上软键盘（ceyboard）的契约尚未在本仓体现**（grep `keyboard` on `kanesumi-controls/src` 零命中） |
| 点击空白收起键盘 | **无代码**（Compose 不做；需 App 写 `clearFocus()`）。sec-a 亦无 | — | **主仓有等价语义**：点空白 → `place_caret` / 失焦清选区（`text_box.rs:104-110`）；Gallery 明写「点 TextBox 空白处应清选区、放光标」（`kanesumi-gallery/src/app.rs:1226-1228`） |
| Enter / 发送键语义 | **框架 + 布局组合**。`MetroTextField` 的 `singleLine=false` 使 Enter 插入换行；「发送」是**独立按钮**（`MetroIconButton` 或 `MetroButton`），**不在软键盘上**（无 `ImeAction.Send`）。按钮可用性硬门：`val active = enabled && text.isNotBlank()`（`kt/MetroChatInputBar.kt:73`） | `kt/MetroChatInputBar.kt:73-91` | **主仓单行语义**：`TextInputKey::Enter => false`（不插入、不消费，交宿主，`text_field.rs:467-470`）；**「按钮可用性硬门」是 sec-a 可搬的真实逻辑**（空白即禁发，含 `enabled` 与 `isNotBlank` 合取） |
| 无障碍 | **框架免费**。`MetroTextField` 自身**零 `semantics`**（grep 全仓 `semantics`：命中 `MetroIcon`（`kt/../../core/theme/MetroIcon.kt:33-49`）、`MetroBottomNav`、`MetroSidebar`、`MetroLyricsPanel`，**`MetroTextField`/`MetroChatInputBar` 都不在其中**）→ 靠 `BasicTextField` 内建的 `EditableText` 语义节点 | `kt/MetroTextField.kt:66-78`（无 semantics 参数） | **主仓无 a11y 层**（无 AccessKit 等；grep `accesskit|Accessibility` on 主仓 `kanesumi-controls/src` 零命中） |

**聊天条布局范式（sec-a 唯一完整的输入相关布局，照抄）：** `kt/MetroChatInputBar.kt:52-72`
```kotlin
    Row(
        modifier = modifier
            .fillMaxWidth()
            .background(colors.surface)
            .metroNavigationBarsPadding()
            .padding(horizontal = 8.dp, vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        MetroTextField(
            value = text,
            onValueChange = onTextChange,
            modifier = Modifier
                .weight(1f)
                .heightIn(min = 40.dp),
            …
        )
        Spacer(Modifier.width(4.dp))
```
数值清单：**外距 8dp（水平/垂直）、行距 4dp、Spacer 4dp、输入框 min 40dp、`measureHeightDp` 默认 72dp**（`kt/MetroChatInputBar.kt:47,57,59,66,72`）。
`measureHeightDp = 72.dp` 进 `rememberBottomStackReservation`（`:50`），让内容侧 `bottomOverlayPadding()` 自动留白（sec-a `AGENTS.md:79-81`）。

**`MetroDrawer` 与输入框的联动核查：零。** 全文 96 行无任何输入框引用（grep `TextField|Focus|keyboard` on `kt/MetroDrawer.kt`：0 命中）；它的 `content` 是 `@Composable ColumnScope.() -> Unit`（`kt/MetroDrawer.kt:54`），
「需要横向拖拽关闭的手势暂不内置 —— 没有真实用例驱动」（`:44-45`），scrim `0.6` + 点击收起（`:51,77-80`），
宽度 `280.dp`（`:53`），`graphicsLayer { translationX = -drawerWidthPx * (1f - progress.value) }`（`:89`）。
**它是「单一 `Animatable` progress 驱动全部视觉、零重组」范式的载体，而不是输入框联动件。**
主仓若要抄，落点是「面板族动画模型」而非输入框族（主仓已有等价物，见 §3-b4）。

---

## §3 可回流清单

> 工作量标尺：**小** = ≤半天 / 单文件；**中** = 1–2 天 / 需补测试；**大** = ≥3 天 / 跨 crate 或需新机制。

### (a) 可直接搬 —— 纯逻辑 / 几何 / 常量 / 状态机，平台无关

| # | 条目 | 来源（sec-a） | 建议 Rust 落点 | 工作量 | 备注 |
|---|---|---|---|---|---|
| a1 | **聊天条输入框三元组：`min 40` + `maxLines 5` + `weight(1f)`** | `kt/MetroChatInputBar.kt:64-71` | `kanesumi-controls/src/text_box.rs`：`pub fn multi_line(max_lines: usize) -> Self` + `min_body_height() = 40.0`；宽度由 App 传入的 `Rect` 决定，无需 Rust 侧概念 | **中** | 前置是 §3-b1（多行渲染）。**这是本次最值得做的一条** |
| a2 | **动作键「图标可选、文字兜底」双形态 + 空白禁用硬门** | `kt/MetroChatInputBar.kt:43-45,73-91`（`active = enabled && text.isNotBlank()`） | 新增 `kanesumi-controls/src/chat_input_bar.rs`：`ChatInputBar { text: TextField, send_enabled(), send_rect(), hit_send() }`（纯逻辑，无 Compose 概念） | **小** | 纯逻辑部分（`send_enabled`）可立刻写并单测；渲染复用 Button/IconButton |
| a3 | **padding 与行高联动的几何算法**（而非抄 12dp 数值） | `kt/MetroTextField.kt:46,69`（`PaddingValues(12.dp)` + `heightIn(min = 20.dp)`） | `text_box.rs::content_rect()` 已是此形（`text_box.rs:139-152`）；要搬的是**「min 内容高 = 行高 × 1.0」的表述**，替换当前散落的 `min_height() = 32.0` 死常量（`text_box.rs:170-173`，无调用点） | **小** | 顺手把 `min_height()` 接上：`min_height(theme) = 2*b + pad_t + pad_b + line_height` |
| a4 | **光标闪烁与「内容变化后立刻亮起」的状态机**（含半周期常量） | 无直接 Kotlin 对应（sec-a 由框架给）；**主仓已实现**：`CARET_BLINK_HALF_PERIOD = 0.5` + `reset_blink()` | `text_box.rs:23-24,113-134` —— **已就位，无需搬**。此条登记为「已闭环，勿重复实现」 | — | 列此条以明确边界 |
| a5 | **IME 内容提示枚举 `Normal / Password / Digits`** | sec-a 侧是框架的 `KeyboardType`（**库内未使用**，grep 0 命中）；主仓已自建 | `ime.rs:10-19` + `password_box.rs:114-122` —— **已就位** | — | sec-a **无可搬**；此条存在的意义是「避免误以为要抄 KeyboardOptions」 |

> **(a) 净结论：真正待搬的只有 a1 / a2 / a3 三条，合计工作量「小～中」。**

### (b) 需改造 —— 依赖 Compose 的 `BasicTextField` / `TextFieldValue` / `KeyboardOptions` / `imePadding`

| # | 条目 | sec-a 依赖的 Compose 能力 | 改造方案（Rust 落点） | 工作量 |
|---|---|---|---|---|
| b1 | **单行/多行自适应高（`minLines`/`maxLines`/`singleLine` 三参数）** | `BasicTextField` 的 measure 自然增长 + 内建滚动 | `text_box.rs::render` 改用 `engine.layout_box(text, size, TextLayoutOptions { max_lines: Some(n), wrap: true, overflow: Clip, .. })`（API 已在 `kanesumi-canvas/src/text.rs:20-43,618`），并在 `render` 内自持 `PushClip/PopClip`。**同时暴露 `measured_height(theme, engine, width) -> f32` 供外层布局** | **大** |
| b2 | **单行水平跟随光标滚动** | `BasicTextField` 内建 | **主仓已有正确范本**：把 `auto_suggest_box.rs:299-309` 的 `scroll = max(0, text_w - view_w)` + `push_clip(content)` 移植进 `text_box.rs::render`，并把 `ensure_caret_visible()` 从空实现改成「内容变 / 光标移 → 重算 scroll」（`text_box.rs:255-259`） | **小** |
| b3 | **点击定位 + 拖拽选择** | `BasicTextField` 的指针手势管线 | 点击定位**已实现**（`text_box.rs:239-253`）。缺的是拖拽：需在 App/harness 层加 `PointerMoved` 期间 `if pressed_on_textbox { place_caret_at(...) }` 并让 `set_cursor` 保留 anchor（当前 `set_cursor` 强制清 anchor，`text_field.rs:395-398` ⇒ 需新增 `extend_selection_to(pos)`） | **中** |
| b4 | **面板入场/收起动画（`MetroDrawer` 范式）** | 无（Compose `Animatable` 只是宿主） | **主仓已有等价物**（`kanesumi-anim` + `Progress`）。`SheetAppear 300ms / (0.2,0,0,1)` 与主仓 `DURATION_OVERLAY_OPEN` 吻合（`UWP_PRIMARY_SOURCES.md:109`）；**`SheetDismiss 260ms` 与一手源 150ms 不符** —— 若 sec-a 的抽屉要回流，须改用 150ms | **小** |
| b5 | **IME 垫高 / 键盘避让** | `WindowInsets.ime`（sec-a 读了但没用）+（应有的）`imePadding()` | 主仓无 Compose 语义。可搬的是**机制契约**：`rect` 消费端（App）须按「IME 占用的底部区域」把输入框上移。**落点建议**：`kanesumi-harness/src/app.rs` 增 `fn ime_inset(&self) -> f32 { 0.0 }`（或复用 `AppConfig.height` + 合成器上报），Gallery 里做一次演示。**主仓的 `set_cursor_rectangle` 已能驱动候选窗**（`ime.rs:33`），缺的是**屏上键盘的版面契约** | **中** |
| b6 | **动作键（发送）语义：`ImeAction.Send` vs 独立按钮** | `KeyboardOptions.imeAction`（sec-a **未使用**，grep 0 命中） | 主仓无软键盘 API；**改为纯按钮语义**（a2）。若将来接 ceyboard，落点是 `harness` 的虚拟键盘协议（`platform.rs:426-428,2803-2850`） | **小** |

### (c) 不可搬

| # | 条目 | 原因 |
|---|---|---|
| c1 | **整个编辑内核**（光标/选区/双击选词/长按/拖拽/撤销/重做/剪贴板/IME 组合态/掩码/焦点级联） | 在 sec-a 侧**全部是 `BasicTextField` + `PlatformTextInputService` + `FocusManager` + 系统剪贴板 + 系统 IME 的产物**，`MetroTextField.kt` 80 行内**一行都没有**（§2.3 全表已证）。这些能力主仓必须以 `text_field.rs` + `text_box.rs` + `kanesumi-canvas/src/text.rs` 自行实现，**不存在「从 Kotlin 翻译」的路径** —— 只能参考 Compose 的行为契约（如「双击=选词、三击=选段」「撤销分组」）**重新设计** |
| c2 | **`VisualTransformation` 掩码抽象** | sec-a 只是**透传**该参数（`kt/MetroTextField.kt:44,76`），未实现任何 transformation；主仓的 `mask: Option<char>`（`text_field.rs:46`）语义更窄但已够用。要搬的「通用输入变换」在主仓无需求驱动 |
| c3 | **`MetroIndication` 闪切（100/200ms）** | 依赖 `IndicationNodeFactory` + `InteractionSource`（`MetroIndication.kt:6-8,36-39`）；主仓无 `clickable` 树、无 interaction 源。主仓的等价物是各控件内部的 `ControlState`（`state.rs:5-11`），**不需要这个中间层** |
| c4 | **`MetroInsets` / `rememberBottomStackReservation` / `bottomOverlayPadding`** | Android 系统栏 / 挖孔 / 手势条 / 软键盘的几何分裂是**平台事实**（sec-a `README.md:47` ADR）；Wayland 下无同构物。主仓对应的是 layer-shell 排他区（TopBar 30px）与合成器侧几何 |

---

## §4 关键代码片段与 Rust 等价写法

### 4.1 sec-a：唯一的编辑相关「策略」—— 焦点/校验门（可搬）

```kotlin
// kt/MetroChatInputBar.kt:73-91
        val active = enabled && text.isNotBlank()
        if (sendIcon != null) {
            MetroIconButton(onClick = onSend, enabled = active) {
                MetroIcon(
                    imageVector = sendIcon,
                    contentDescription = sendContentDescription,
                    tint = if (active) colors.primary else colors.onSurfaceVariant,
                    sizeDp = 22.dp,
                )
            }
        } else {
            MetroButton(
                text = sendText,
                onClick = onSend,
                enabled = active,
                containerColor = colors.primary,
                contentColor = colors.onPrimary,
            )
        }
```
**Rust 等价写法（建议）：**
```rust
// kanesumi-controls/src/chat_input_bar.rs（新文件）
impl ChatInputBar {
    /// 发送键可用性硬门 —— 与 sec-a `MetroChatInputBar.kt:73` 同义：
    /// `enabled && !text.is_blank()`。空白判定走 Unicode 空白（非 `== ""`）。
    pub fn send_enabled(&self) -> bool {
        self.enabled && self.field.text().chars().any(|c| !c.is_whitespace())
    }
}
```
> 与主仓现有风格一致（`text_field.rs` 的 `can_undo()` / `text_box.rs` 的 `caret_visible()` 都是这种「一问一答」纯逻辑方法）。

### 4.2 sec-a：多行 + 最大行数（要搬的「策略」，不是 API）

```kotlin
// kt/MetroChatInputBar.kt:64-71
            modifier = Modifier
                .weight(1f)
                .heightIn(min = 40.dp),
            placeholder = placeholder,
            enabled = enabled,
            singleLine = false,
            maxLines = 5,
```
**Rust 等价写法（建议，前置任务 b1）：**
```rust
// kanesumi-controls/src/text_box.rs
/// 多行输入：wrap + 行数上限 + 最小体高。参 sec-a `MetroChatInputBar.kt:64-71`
/// （`weight(1f)` 在 Rust 侧由外层布局给宽度，控件只负责「按给定宽度算高」）。
pub const TEXTBOX_MIN_BODY_H: f32 = 40.0;
pub const TEXTBOX_DEFAULT_MAX_LINES: usize = 5;

pub fn measured_height(&self, theme: &MetroTheme, engine: &TextEngine, width: f32) -> f32 {
    let style = theme.typography.body;
    let opts = TextLayoutOptions {
        max_width: width - 2.0 * self.border_thickness() - 10.0 - 6.0,
        max_height: f32::INFINITY,
        line_height: style.line_height,
        letter_spacing_em: style.letter_spacing_em,
        max_lines: Some(self.max_lines),
        wrap: true,
        overflow: TextOverflow::Clip,   // 参 COMPOSITION.md §5「溢出显式」
    };
    let layout = engine.layout_box(&self.field.display_text(), style.size, opts);
    (layout.size.height + 2.0 * self.border_thickness() + 6.0 + 5.0)
        .max(TEXTBOX_MIN_BODY_H)
}
```
> `TextLayoutOptions` 与 `layout_box` 已存在（`kanesumi-canvas/src/text.rs:20-43,618`），无需新 API。

### 4.3 sec-a：`MetroDrawer` 的单 progress 动画范式（对照用）

```kotlin
// kt/MetroDrawer.kt:56-69,83-91
    val progress = remember { Animatable(0f) }
    …
    LaunchedEffect(Unit) {
        progress.animateTo(1f, SokuouTweens.SheetAppear)
    }
    fun animateDismiss() {
        coroutineScope.launch {
            progress.animateTo(0f, SokuouTweens.SheetDismiss)
            onDismiss()
        }
    }
    …
        Column(
            modifier = Modifier
                .align(Alignment.CenterStart)
                .width(drawerWidth)
                .fillMaxHeight()
                .onSizeChanged { drawerWidthPx = it.width.toFloat() }
                .graphicsLayer { translationX = -drawerWidthPx * (1f - progress.value) }
```
**Rust 等价写法：** 主仓已有同构物（`Progress` + 解算后写入 Scene 的几何），无需新增；
**唯一要改的数值**是收起时长 —— sec-a `260ms FastOutSlowIn`（`Sokuou.kt:106`）
vs 一手源 `ClosingStoryboard 150ms`（`UWP_PRIMARY_SOURCES.md:46`，主仓 `eb9daf9` 已按 150ms）。
**若 sec-a 回流任何面板，必须用 150ms。**

### 4.4 主仓：单行滚动 + 裁剪的现成正解（本报告最有操作性的一段）

```rust
// kanesumi-controls/src/auto_suggest_box.rs:299-316（照抄，作为 text_box.rs 的改造范本）
        // 自适应滚动：单行文本超宽时，把 scroll 调至末尾可见（UWP 单行输入行为）。
        if !self.field.is_empty() {
            let text_w = engine.measure(&self.field.display_text(), style.size);
            let view_w = content.size.width;
            self.scroll = if text_w > view_w { text_w - view_w } else { 0.0 };
        } else {
            self.scroll = 0.0;
        }

        // 文本 / 占位 —— 单行不换行 + 裁剪进内容区（避免换行溢出框）。
        scene.push_clip(content);
```
而 `text_box.rs` 目前**没有** `push_clip`，且 `ensure_caret_visible()` 是空壳：
```rust
// kanesumi-controls/src/text_box.rs:255-259（现状）
    /// 确保光标在可视范围内（水平滚动夹紧）。内容宽 / 光标 x 由调用方（App 层）
    /// 结合视口宽度驱动 `scroll`；此处仅重设闪烁（内容变化后立刻亮起）。
    fn ensure_caret_visible(&mut self) {
        self.reset_blink();
    }
```
→ **改造目标（工作量小）**：让 `text_box.rs` 达到 `auto_suggest_box.rs` 已有的水平，
并把 `ensure_caret_visible()` 实现为「把 `caret_x` 夹进 `content` 视口」。

---

## §5 不确定 / 需人裁定

### 已确证的主仓内部不一致（建议尽快裁定）

- **U1 · 边框粗细：规格 vs 实现 vs 一手源三方不一致（🔴 建议优先）**
  `docs/CONTROL_SPEC.md:1172` 的 2026-09-22 一手源补全段明确写：*「边框粗细恒为 **2px**（本文旧写 1px+divider）」*，
  `UWP_PRIMARY_SOURCES.md:98` 亦记 `TextControlBorderBrush` 模板粗细 **2px**。
  但 `text_box.rs:166-168` 的实现是 `if self.focused { 2.0 } else { 1.0 }`，
  且 Normal 分支取的是 `colors.divider`（`#3A3A3A`，实色）而非规格要求的 **BaseMediumLow 40%**（`base_medium_low = 0.4`，`indicator.rs:80`）。
  **规格已改、实现未跟上**（或实现有意保留旧观感而未登记）。二选一：改实现，或把当前值登记进 `CANON_VS_TEMPORARY.md`。
  同类问题也在 `number_box.rs:396`、`auto_suggest_box.rs:396`（都注释「与 MetroTextBox 对齐」）。

- **U2 · 主仓 Hover 态在应用中从未被触发（🟡 建议优先）**
  `text_box.rs:419-424` 的 Hover 边框分支是**死分支**：`kanesumi-gallery/src/app.rs:1097-1117` 的
  `match target` 不含 `Target::TextBox`/`PasswordBox`/`AutoSuggest`/`NumberBox`，
  `clear_hover()`（`:1149-1161`）也不复位它们，全仓无 `textbox.state = ControlState::Hovered` 的赋值点。
  ⇒ `975a44e`「悬停边框改 BaseMedium 实色」的成果**当前不可见**。建议在 Gallery 的 `update_hover`
  里补 `Some(Target::TextBox) => self.textbox.state = ControlState::Hovered` + `clear_hover` 复位。

- **U3 · 光标用 `base_medium_high`（0.8）而非 100%（🟡 待裁定）**
  `text_box.rs:385-390`：`colors.on_surface.with_alpha(theme.indication.base_medium_high)` = 80%。
  UWP 的文本光标取前景基色（`TextControlForeground`= BaseHigh 100%）。80% 档取自 BaseMediumHigh 的**字形**语境（`indicator.rs:74`），
  用在光标上没有一手依据。**注意**：`975a44e` 刚把测试判据从魔法阈值改成读令牌（`:634-641`），
  改档位时测试会跟随，故现在改动的成本很低。

- **U4 · `text_selection_tint` 35% vs UWP 100%（已在 T19 登记，仍待真机确认）**
  `colors.rs:89` / `text_box.rs:319-324`；`CANON_VS_TEMPORARY.md:38` 已记录为「有意偏离候选」。
  **sec-a 在此处不提供任何对照**（它没设 `LocalTextSelectionColors`，走框架默认）。

### 未能本地验证的外部事实

- **U5 · Compose 默认选区色**：`kt/../../core/theme/MetroTheme.kt:22-26` 未 provide `LocalTextSelectionColors`，
  故 sec-a 输入框的选区色是 **Compose foundation 1.7.6 的框架默认**（本机 AAR 已在
  `~/.gradle/caches/.../foundation-android/1.7.6/.../foundation-release.aar`，反编译可见
  `TextSelectionColorsKt.DefaultSelectionColor` + `Color.copy`，但常量值未被解出）。
  社区通行值为 `primary.copy(alpha = 0.4f)`（≈40%），**本报告不据此下结论**；若要用作主仓 35% 的对照，需一次反编译或有网复核。
  影响：§2.2 表里「选区」一行 sec-a 侧留白。

- **U6 · sec-a 的「用户手感」没有仓内证据**。`MetroTextField` 在 sec-a 全仓（含 sample）**只有 1 个调用者**
  （`kt/MetroChatInputBar.kt:61`），sample 零引用（§1-2 已证）。故「输入框也挺舒服」是
  **对观感的评价，不是对已验证行为的评价**。若要据此事后裁定视觉方向（例如「是否去掉主仓描边」），
  建议**先让 sec-a 侧补一个输入框 demo 页**，否则裁定缺依据。

### 平台语义差异（不建议作为回流项，仅登记）

- **U7 · 焦点进入语义**：主仓 `focus()` **全选**（`text_box.rs:95-102`，UWP 行为，且有测试 `:701-708`）；
  sec-a 无等价物（Compose 点按 = 定位光标，不选全）。Gallery 还专门反着做了一次修复：
  点 TextBox **不** `select_all`（`kanesumi-gallery/src/app.rs:1274-1280`，V22）。
  ⇒ 主仓内部「`focus()` 全选」与「点击聚焦不全选」已在 App 层被绕过，**属已解决但绕行**，
  留待是否把 `focus()` 的默认行为改为「不全选、由 `select_all()` 显式调用」的裁定。
- **U8 · 单行 vs 多行是控件级差异，不是参数**：主仓 `MetroTextBox` 头部即声明「单行」（`text_box.rs:1`），
  改多行会牵动 `handle_key(Enter)`（`text_field.rs:467-470`）、`caret_x`（单行 `line_geometry` → 需多行定位）、
  `place_caret_at`（x-only → 需 (x,y)）三处。**这不是「加个参数」，是一次控件重构**（§3-b1 判「大」的依据）。

---

## §6 附：证据索引（便于复核）

| 主题 | sec-a | 主仓 |
|---|---|---|
| 输入框主体 | `kanesumi-controls/.../MetroTextField.kt:1-80` | `kanesumi-controls/src/text_box.rs:1-518` |
| 编辑内核 | （无 —— 框架） | `kanesumi-controls/src/text_field.rs:1-571` |
| IME 契约 | （无 —— 框架） | `kanesumi-controls/src/ime.rs:1-49`、`IME_WIRING_PLAN.md` |
| 排版/几何 | （无 —— 框架） | `kanesumi-canvas/src/text.rs:20-43`（`TextLayoutOptions`）、`:81-158`（`TextLineGeometry`）、`:618`（`layout_box`） |
| 聊天条 | `.../MetroChatInputBar.kt:1-93` | （无） |
| 抽屉 | `.../MetroDrawer.kt:1-96` | （面板族在 `popup.rs`/`dialog.rs`/`bottom_sheet` 类） |
| 规格 | （无相应文档） | `docs/CONTROL_SPEC.md:1138-1218`（§34/§35）、`:1298-1332`（§38） |
| 一手源 | （无） | `docs/UWP_PRIMARY_SOURCES.md:92-102`（§3.4 文本控件）、`:35`（T16 白纸行为） |
| 临时/偏离登记 | （无对应机制） | `docs/CANON_VS_TEMPORARY.md:35`(T16) `:38`(T19) `:73`(D10) |
| 构图契约 | （无） | `docs/COMPOSITION.md:37-40`（溢出显式 / PushClip 成对） |
| 真实用法 | `sample/.../MainActivity.kt:349-400`（FormControlsShowcase，**不含输入框**） | `kanesumi-gallery/src/app.rs:1619-1658`（Input 页）、`:1273-1318`（press 路由）、`:2057-2162`（键盘路由） |

---

## §7 下一步：最值得抄的 3 件事 / 引以为戒的 2 件事

### 最值得抄的 3 件事（按性价比排序）

**① 单行滚动 + 裁剪的闭环 —— 但它不来自 sec-a，来自主仓自己的 `auto_suggest_box.rs`。**
sec-a 的贡献是**暴露了这件事的必要性**：它的 `BasicTextField(singleLine = true)` 免费做到，
而主仓 `text_box.rs` 的 `scroll` 字段（`:42`）**唯一写入者是空函数**（`:257-259`），全长文本会画出框外。
落点：`text_box.rs::render` 抄 `auto_suggest_box.rs:299-309`（`scroll = max(0, text_w - view_w)` + `scene.push_clip(content)`），
并实现 `ensure_caret_visible()`。**工作量：小。收益：修掉一个必然可见的溢出。**
（同时满足 `COMPOSITION.md:37-39` 的强制契约 —— 目前 `text_box.rs` 未遵守。）

**② 多行输入的三元组 `min 40dp / maxLines 5 / 按宽算高`（`kt/MetroChatInputBar.kt:64-71`）。**
这是 sec-a 唯一完整的「输入条」范式，也是聊天/命令条类 UI 的刚需。
落点：`text_box.rs` 增 `max_lines` + `measured_height(theme, engine, width)`，走已有的
`TextLayoutOptions { max_lines, wrap, .. }`（`text.rs:20-43`）+ `layout_box`（`:618`）。
**工作量：大**（牵动 `caret_x`/`place_caret_at`/`handle_key(Enter)`，见 §5-U8）—— 但这是输入框族
从「单行控件」升级为「可用输入件」的分水岭，**建议作为下一批的第一个功能块**。

**③ 「发送键可用性硬门」+「图标可选、文字兜底」（`kt/MetroChatInputBar.kt:43-45,73-91`）。**
纯逻辑、零依赖、可立即单测（`send_enabled() = enabled && !text.is_blank()`）。
落点：新增 `kanesumi-controls/src/chat_input_bar.rs`（纯逻辑 `ChatInputBar`）。
**工作量：小。** 它是 sec-a 输入框族里**唯一一段可以直接翻译成 Rust 的「策略」**。

> **不推荐**把「去掉边框、改为无边框方底」当成回流项：那是 sec-a 铁律与主仓 §34 规格的取向冲突，
> 且主仓 `975a44e` 刚把悬停边框按一手源改对 —— 现在退回无边框会推翻刚取到的权威值。**要改先裁定（§5-U1）。**

### Android 侧不能抄、但要引以为戒的 2 件事

**诫一：不要把「框架免费提供的能力」误读成「控件很轻」。**
`MetroTextField.kt` 只有 80 行，于是它看起来像「一个下午就能抄完的控件」。
但那 80 行之所以够用，是因为它背后站着：
`BasicTextField`（光标/选区/点击定位/双击选词/长按/拖拽/撤销重做/滚动/裁剪/无障碍语义节点）
+ `PlatformTextInputService`（IME 组合态全链路）+ `FocusManager`（焦点级联）
+ 系统剪贴板 + `WindowInsets.ime`。
主仓的等价物是 `text_field.rs`（571 行）+ `text_box.rs`（518 行）+ `kanesumi-canvas/src/text.rs`（929 行）
+ `harness/platform.rs` 的 IME 绑定 —— **并且仍未补齐双击选词、长按、拖拽选择、redo 四项**。
**判据：看到 sec-a 某控件「很短」，先数它依赖的框架 API 数量，再估工作量。**

**诫二：不要抄「声明了却没用上」的机制 —— sec-a 有三处，主仓有两处同型病灶。**
- sec-a：（i）`MetroTextField` 全仓只有 1 个调用者，sample 零引用；
  （ii）`MetroInsets.ime` 被读取并显示在 DebugPanel（`MainActivity.kt:697`），**全仓无人消费**，
  `MetroChatInputBar` 挂的是 `metroNavigationBarsPadding()`（`MetroChatInputBar.kt:56`）⇒ 软键盘弹出时输入条**不会上移**；
  （iii）`MetroDrawer` 的横向拖拽关闭明知需要却未做（`:44-45`）。
- 主仓同型：（i）`MetroTextBox.show_delete` 有几何有渲染（`text_box.rs:155-163,394-414`）但**无任何处置 true**
  ⇒ 清除键永远不出现；（ii）`min_height() = 32.0`（`:170-173`）**无调用点**。
**判据：回流前先确认 sec-a 那份能力「有真实消费点」，否则搬来的是同样会腐烂的第二份死代码。**
