# Kanesumi 视觉审计问题清单（Metro / WinUI 2 复刻）

> 本文件是 2026-08-10 视觉审计的产出。**参考目标严格限定 Metro / WinUI 2（Windows 8 – 10 时代）**，
> 具体锚点 = `microsoft-ui-xaml v2.8.7` + WinUI-Gallery `winui2` 分支。
> **Fluent（Win10 后期外观 / Win11 UI）仅作反面教材，不是复刻目标。**
> 规格自足档见 `docs/CONTROL_SPEC.md` / `docs/CONTROL_MATRIX.md`，本文件只列**实现与规格 / 视觉的偏离**。

审计方法：Gallery 在 Plasma Wayland 上以 `KANESUMI_DEMO_STATE=<state>` 打十种交互态，截图 + 代码交叉核对。
截图归档：`/tmp/audit-01…10-*.png`（临时）。

## 严重度

| 记号 | 含义 |
|---|---|
| 🔴 | **阻塞**：视觉完全崩坏，不能作为 daily driver |
| 🟠 | **高**：明显规格偏离，一眼能看出不对 |
| 🟡 | **中**：细节漂移，需要对照 SPEC 才发现 |
| 🟢 | **低**：结构/未来债务，不影响当前视觉 |

## 状态

| 记号 | 含义 |
|---|---|
| ⬜ | 未处理 |
| 🔧 | 进行中 |
| ✅ | 已修（含 commit 短 hash） |

---

## 🔴 阻塞级

### V1 · DropdownMenu 面板文字逐字换行 ✅ `e6f65e0`
**位置：** `kanesumi-controls/src/dropdown_menu.rs:190`

```rust
let text_rect = Rect::new(
    x,
    y + (self.item_height - style.line_height) / 2.0,
    self.panel_rect.size.width - x,   // ← bug：x 是绝对坐标（几百 px），减 panel 相对宽（约 150）= 巨大负值
    style.line_height,
);
```

传入 `engine.layout(text, size, max_width=负数)` 触发 CJK 硬断路径，每字都被推到新行。菜单变成 "新/建/[icon]/打/开/..." 字塔。

**修法：** `((self.panel_rect.right() - RIGHT_PAD) - x).max(0.0)`。

**证据：** `/tmp/audit-03-dropdown.png`。

---

### V2 · Dialog 标题与正文上下重叠 ✅ `c7d8b1e`
**位置：** `kanesumi-controls/src/dialog.rs:262-266`

```rust
let title_gap = if self.title.is_empty() { 0.0 } else { 12.0 };
let content_rect = Rect::new(
    inner.origin.x,
    inner.origin.y + title_gap,   // ← 少加了 title 的 line_height (26)
    ...
);
```

CONTROL_SPEC §9 「TitleMargin 下 12」= 标题下**沿**再向下 12。

**修法：** `inner.origin.y + title_style.line_height + title_gap`。

**证据：** `/tmp/audit-02-dialog.png` —— "保存工作？" 与 "是否保存对当前文件的更改？" 直接压在一起。

---

### V3 · Dialog 缩放从 0 起而不是 1.05 ✅ `9d909e0`
**位置：** `kanesumi-controls/src/dialog.rs:78, 111, 123`

```rust
scale.jump_to(0.0);                       // 初值 0
// show(): self.scale = MetroAnim::new(...);   // 重建，value 又是 0
//         self.scale.set_target(1.0);          // 0→1 暴力放大
// hide(): self.scale = MetroAnim::new(...);    // 又重建
//         self.scale.set_target(1.05);         // 0→1.05
```

CONTROL_SPEC §9 要求 `1.05→1.0`（轻微收缩，让 opacity 淡入承担出现感）。当前是"从无到有暴力放大 + 退场重置回 0 再涨到 1.05"，视觉不对。

**修法：** show 前 `scale.jump_to(1.05); scale.set_target(1.0);`；hide 前保留当前 value（1.0），`set_target(1.05)`。不能新建 MetroAnim，会丢当前 value。

---

### V4 · SelectorFlyout 触发器双画（右侧幽灵箭头方框） ✅ 本 commit
**位置：** `kanesumi-gallery/src/app.rs:522-546` + `kanesumi-controls/src/selector_flyout.rs::render`

Gallery 手绘一遍 (fill + 文本 `"选择 ▾"`)，然后 `selector.render(trigger=st, ...)` 内部又画一遍 (fill + selected/placeholder + arrow glyph)。两个 chevron glyph 位置不同，都是缺字导致的方框，就变成两个方框。

**修法：** 让 `selector.render` 独占触发器 + 面板绘制，删除 Gallery 侧的手工重复；Gallery 通过 `selector.placeholder = "选择"` 传占位文本。DropdownMenu 侧现状是 `render()` 只画面板（Gallery 手绘触发器），两个控件 API 契约不一致——本次至少统一 Selector；DropdownMenu 的 API 契约留 V7 一并处理。

---

## 🟠 高优

### V5 · Gallery 标题压到按钮 ✅ 本 commit
`kanesumi-gallery/src/app.rs:19-20`。CTRL_Y0=44，但标题从 y=20、高 42 到 y=62。重叠 18 px。**根本解**是引入 box 布局器（V22），本条只是"当前 Gallery 硬编码常量错"。

### V6 · Accent 按钮 "打开对话框" 文字溢出 ⬜
按钮 130 px 宽，`measure("打开对话框", 15)` 在思源黑体下约 90-100 px，本身能装下；但截图里文字左缘伸出到按钮外——原因是 `label_rect.origin.x = accent_rect.x + (130 - label_width) / 2` 允许负偏移。CONTROL_SPEC §1 明确"无 MinWidth，尺寸 = 内容 + Padding"，所以按钮宽度本应由内容驱动，而不是硬编码 130。
**位置：** `kanesumi-controls/src/button.rs:92-100` + Gallery `accent_rect()`。

### V7 · Menu / Selector 图标 glyph 全是方框 ✅ 本 commit
Menu items 用 `\u{E8E5}` `\u{E74E}` `\u{E792}` `\u{E7E8}`，Selector 触发器用 `\u{E70D}`。这些是 Segoe MDL2 / Fluent Icons 私有区编码，思源黑体没有 → 全 `.notdef` 方框。
**Metro 定位下这是致命选择**：Metro 时代（Win8-10 早期）UI 图标是 Segoe UI Symbol + Segoe MDL2；Ether 既然选思源黑体作正体，就必须自带图标资产（SVG 位图，走 Gallery 里 `share.svg` 同款 `MetroIconButton::with_svg` 管线），或至少 fallback 到 ASCII 几何符号（"▼" "▲" "+" "×"）。
**修法：** (a) Kanesumi 内置最小图标集（chevron_down / chevron_up / plus / close / file_open / …），走 SVG；(b) 移除代码里所有 Segoe MDL2 codepoint。

### V8 · Row / Column DSL 强制等分 ⬜
`kanesumi-controls/src/decl.rs:174-194`。`spacing` 字段声明了但 `let _ = spacing;` 直接扔。Gallery footer `Row(Text, Button)` 两个子都拿 464 px，导致"点我 +1" 按钮撑到半屏。
**修法：** 加 `Decl::Spacer{ grow: f32 }` + `Sized{ width, child }`，或让 `Text` / `Button` 有"自然宽度模式"、Row 只均分标 flex 的子。

### V9 · 深色主题下 Dialog 遮罩几乎不可见 ✅ 本 commit
`kanesumi-core/src/theme.rs:27` `overlay_color = Color::BLACK.with_alpha(0.45)`。在 #1E1E1E 深底上只暗 10%，遮罩形同虚设。Metro 原版 dark 主题也用**白色遮罩**（`ContentDialogDimmingThemeBrush = #99FFFFFF`）—— dark 底加白遮罩才有"上一层"的观感。
**修法：** 深色主题遮罩换 `Color::WHITE.with_alpha(0.15)` 附近的值（对齐 UWP 白 60% 的观感，Ether 空间桌面调低一档避免刺眼），或黑 0.7+。

### V10 · Focus 描边看不见 ✅ 本 commit
`indication.focus_stroke = 0xFFA626`（橙）但 `emit_stroke` 在 thickness=1 + 圆角 2 时环带面积退化到亚像素。1 逻辑 px 在 2× buffer 上只有 2 物理 px，抗锯齿再一模糊就没了。
**修法：** focus_stroke thickness ≥ 2；或按 UWP 惯例焦点框在控件**外侧** 3 px（FocusVisualMargin=-3）+ 2 px 宽双色（黑 + accent）。

---

## 🟡 中优

### V11 · Switch 标签视觉重心与轨道中心不齐 ⬜
`switch.rs:83-92`。label 用 line_height 居中在 40 高的宿主矩形里，轨道也居中，但 label 的视觉重心在 line_height 内偏上，视觉上比 knob 中心略高。

### V12 · MetroSurface `shape.corner_radius` 从未使用 ⬜
`surface.rs:49` 只 `fill_rect`，忽略 corner_radius。

### V13 · ProgressBar `height` 公式恒等于 `min_height` ⬜
`progress.rs:83` `min_height.max(rect.size.height.min(4.0))` 恒 = 4。表达冗余。

### V14 · Text layout `split_ascii_whitespace` 对 CJK 标点切分粗暴 ⬜
`text.rs:97`。全角空格 U+3000、CJK 标点未识别为断行点，只靠硬断兜底。egui 用允许换行字符集是更合理的做法。

### V15 · ProgressRing 不确定态弧长呼吸公式错 ⬜
`progress.rs:232-238` 后半段 sweep 恒 180°，只有旋转没有 TrimStart 前推 → 视觉是匀速转半圆，不是 Metro 呼吸感。

### V16 · TabRow 字距 −2.5% 未实现 ⬜
`tab_row.rs:30` `header_spacing: 0.0` + 注释 "简单起见留空"。CONTROL_SPEC §6 明确 CharacterSpacing −25。

### V17 · ProgressBar Paused/Error 无淡出 / 换色动画 ⬜
CONTROL_SPEC §4 要 0.25s 淡出到 0.6 / 0.25s 换错误色。当前布尔直接切换。

---

## 🟢 低优 / 未来债务

### V18 · Dialog `hide()` 用 Quadratic EaseIn 而非 SPEC 要求的 Linear ⬜
`dialog.rs:121`。

### V19 · `Color::from_hex` 分支阈值有坑 ⬜
`color.rs:21`。`0x00FFFFFF` 会被当纯 RGB（cyan），`0x01000000` 当 RGBA（几乎透明黑）。建议加 `from_rgba(u32)` 显式方法。

### V20 · `PopupAnim::default()` 的 `MetroAnim::default_metro()` 是浪费构造 ⬜
`popup.rs:83-86`。open() 立即又新建 overlay_open 覆盖，default 那次构造纯浪费。

### V21 · Gallery `dropdown_panel()` 每帧重复计算 panel_size / place_popup ⬜
`panel_size` 遍历所有 items × `engine.measure`。retained 层没利用。

### V22 · **无布局器**（SESSION_HANDOVER §1 核心） ⬜
所有控件的 render 拿一个 "给定 rect" 就画；控件间坐标由 App 硬编码。这是"构图混乱"的机制原因。**修完 V22 后 V5 V6 V8 V11 自然消解。**
**修法：** 搬 egui 的最小模型 —— `Ui { max_rect, cursor, min_rect }` + `allocate(size) -> Rect` 三样，足以承起当前所有控件；不用整套 egui。

### V23 · 无焦点管理器 ⬜
`ControlState::Focused` 是控件自身状态，没 Tab 遍历、没单实例保证。多个控件同时 Focused 会有多个焦点环。

### V24 · 无 Xdg Popup —— 弹层直接画在主 surface ⬜
DropdownMenu / SelectorFlyout / Dialog 全部合成到主 surface，`place_popup` 的 "屏幕" 其实是 window。TopBar / Dock 之类要弹到 window 外面时得改成真正的 wp-popup 或 layer-shell popup。

---

## 修复顺序

1. **本轮：V1 → V2 → V3 → V4**（每条一个 commit，先解锁 daily driver 可用性）。
2. 下一轮：V7（Metro 图标资产 —— 是"我们说自己不是 Fluent"的关键证据）。
3. 之后：V22 布局器 —— 完工后 V5 V6 V8 V11 顺势解决。
4. 中/低优随控件迭代逐步清。

## 修复日志

| 编号 | 状态 | Commit | 备注 |
|---|---|---|---|
| ymin sign fix | ✅ | `922eaf7` | descender 上顶（本会话首个修）|
| V1 | ✅ | `e6f65e0` | dropdown 文字 rect 用 panel.right() 而非 panel.width - x |
| V2 | ✅ | `c7d8b1e` | content_rect.y 加 title.line_height |
| V3 | ✅ | `9d909e0` | jump_to 保 value 后再 set_target；default scale=1.05 |
| V4 | ✅ | `92df519` | 删 Gallery 侧手工触发器绘制，selector.render 独占 |
| V7 | ✅ | `5746c60` | Metro 自绘 chevron（Scene::triangle + canvas glyph 模块），移除 5 处 MDL2 codepoint |
| V9 | ✅ | `a27f90e` | overlay_color BLACK 0.45→0.7；补 alpha ≥ 0.6 断言；恢复 KANESUMI_DEMO_STATE 视觉审计钩子 |
| V10 | ✅ | `66c0dc5` | focus stroke 1→2px + 修 rounded_rect_polygon 点数一致（原本 r=2/inner_r=0 时 stroke 变 fill） |
| V5 | ✅ | 本 commit | Gallery TITLE_H 36→42 对齐 line_height；CTRL_Y0 从常量 44 改为派生 = TITLE_Y+TITLE_H+12 |
