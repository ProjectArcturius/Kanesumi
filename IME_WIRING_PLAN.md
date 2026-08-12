# IME Wiring 计划 · `text-input-v3` 客户端接入

**目标：** 让 kanesumi-gallery 及所有基于 kanesumi-harness 的应用在 Plasma / KDE / mutter 等
支持 `zwp_text_input_v3` 的合成器下，接住 fcitx5 / ibus 等 IME 的中文/日文/韩文输入。

**现状：** kanesumi-harness 完全未绑 `zwp_text_input_v3`（`grep zwp_text_input` on harness 零命中），
fcitx5 候选窗弹出后 `commit_string` 事件无人接收，用户看到方框且无中文入 TextBox。
主仓 `CLAUDE.md` Known Issue #12 记录同一缺口的合成器侧；本计划解决 **客户端侧**。

**非目标：** input-method-v2（合成器侧 IME UI 宿主，Ether 合成器另立计划）。

**总量：** ~940 LOC，分 5 阶段，每阶段独立可 merge / 可测。事件循环结构不动，纯增量。

---

## 阶段 A · TextField 组合态（无 harness 依赖，纯逻辑）

**`kanesumi-controls/src/text_field.rs`**
- 加字段：`preedit: String` / `preedit_cursor: Option<usize>`（不入 `text`、不入 undo、不进 `selection()`）
- 加 API：
  - `set_preedit(text, cursor_byte)` — 空串清除
  - `commit_ime(&str)` — 快照 + 删选区 + 插入 + 清 preedit（原子编辑）
  - `delete_surrounding(before_bytes, after_bytes)` — 字节→字符转换，UTF-8 边界外扩夹紧
  - `surrounding_text(max_bytes) -> (before, after, cursor_byte, anchor_byte)`
  - `handle_key(Escape)` 顺带清 preedit
- 单测 ~10 条：commit 有/无选区、CJK 边界 delete、byte-boundary snap、preedit 不进 undo

**LOC：** ~150 impl + ~120 tests **能独立 merge：** ✅

---

## 阶段 B · TextBox / PasswordBox 渲染 preedit

**`kanesumi-controls/src/text_box.rs` + `password_box.rs`**
- render 里把 preedit 拼进显示流（`cursor` 位置分片）
- 加 **虚线下划线**（60% opacity，`colors.on_surface`，一段 stroke）—— CONTROL_SPEC 未定 preedit 规格，走平台默认
- 光标 x 后移 `preedit_cursor_width`
- 加 `pub fn caret_rect_absolute(...)` + `pub fn ime_context(engine, body) -> ImeContext`
- PasswordBox 走同一路径，preedit 也被 mask（规避靠 `content_hint`，见 E）

**LOC：** ~80 impl + ~50 tests **能独立 merge：** ✅（gallery 手动注入 InputEvent 验证）

---

## 阶段 C · InputEvent 变体 + App trait + Gallery 路由

**`kanesumi-harness/src/app.rs`**
```rust
InputEvent::Preedit { text: String, cursor_byte: Option<usize> }
InputEvent::Commit  { text: String }
InputEvent::DeleteSurrounding { before_bytes: u32, after_bytes: u32 }

pub struct ImeContext {
    pub surrounding_before: String,   // 每侧 cap ~1000 bytes
    pub surrounding_after: String,
    pub cursor_byte: u32,
    pub anchor_byte: u32,
    pub caret_rect: kanesumi_core::Rect,  // 表面本地逻辑像素
    pub content_hint: ImeContentHint,     // Normal / Password / Digits
}

pub trait App {
    // 已有方法 …
    fn ime_focus(&self) -> Option<ImeContext> { None }  // 默认不启 IME
}
```

**Gallery：** 复用现有 `FocusedInput` 焦点级联（`app.rs:1536`），加 Preedit/Commit/DeleteSurrounding 分派 + 实现 `ime_focus()`（读控件缓存的 caret_rect/body）

**破坏性变更：** `InputEvent` 因含 `String` 失去 `Copy`；Gallery 测试 ~30 处 by-value copy 改 by-value clone。机械改动。

**LOC：** ~60 harness + ~80 gallery + ~40 tests **能独立 merge：** ✅（无 Wayland 依赖，注入即测）

---

## 阶段 D · Harness 协议绑定

**`kanesumi-harness/Cargo.toml`**（Linux 段）
```toml
wayland-protocols = { version = "0.32", features = ["client", "unstable"] }
```
拿到 `wayland_protocols::wp::text_input::zv3::client::{ZwpTextInputManagerV3, ZwpTextInputV3}`。

**`platform.rs` Shell 新字段：**
```
text_input_manager: Option<ZwpTextInputManagerV3>
text_input: Option<ZwpTextInputV3>     // per-seat
ime_enabled: bool                       // 上次发的 enable 状态
ime_focus_surface: bool                 // wl_keyboard enter/leave
ime_focused_control: bool               // App.ime_focus().is_some()
commit_serial: u32                      // 每次 commit() 后 +=1
pending_preedit / pending_commit / pending_delete   // done 前累积
```

**`impl Dispatch<ZwpTextInputV3, ()> for Shell`：**
- enter/leave → 更新 `ime_focus_surface` → 调 `reconcile_ime`
- preedit_string / commit_string / delete_surrounding_text → 塞进 pending
- **done { serial }** → 仅当 `serial == commit_serial` 生效（stale 帧丢弃）；按协议顺序派发 `DeleteSurrounding → Commit → Preedit`

**`reconcile_ime()` 幂等：**
- `want = ime_focus_surface && ime_focused_control`
- 与 `ime_enabled` 不一致时才 `enable()` 或 `disable()`；随即 `set_surrounding_text` / `set_content_type` / `set_cursor_rectangle` 灌上下文，最后 `commit()` + `commit_serial += 1`
- 每次派发完 Commit/Preedit 也 reconcile（文本变了要重新灌上下文）

**降级：** manager 拿不到 → `text_input = None`，App 仍收裸 KeyPressed；日志一次。

**纯逻辑抽出可测：**
- `compute_ime_action(focus_surface, focus_control, currently_enabled) -> Option<ImeAction>`
- `struct PendingImeBatch { fn apply_done(serial, current_serial) -> Option<Vec<InputEvent>> }`

**LOC：** ~250 impl + ~80 tests（都在纯 helper 上） **事件循环变动：** 只加两个 `impl Dispatch`，calloop / frame-callback 不动

---

## 阶段 E · PasswordBox 内容提示 + 文档

- `MetroPasswordBox::ime_focus()` 返回 `content_hint: Password`
- harness `reconcile_ime` 里将其映射为 `content_purpose = password | content_hint = sensitive_data | hidden_text`
- fcitx5 收到 → 自禁候选窗
- CONTROL_SPEC 加 preedit 视觉规格（虚线下划线 / 60% opacity）

**LOC：** ~30

---

## 五大风险 + 缓解

1. **Serial 错位**（fcitx5 经典锅） — 单调 `commit_serial`，只有实际 `commit()` 时才 +=1；stale done 丢弃。抽 `PendingImeBatch` 单测覆盖。
2. **非文本表面 enable 抖动** — 编译器 enter 早于 App focus；`reconcile_ime` 幂等，首次真 focus 前不发。
3. **UTF-8 字节 vs 字符转换** — 单一 helper `char_range_from_byte_range(&[char], byte_lo, byte_hi)` + fuzz test；中码点外扩 snap。
4. **wl_keyboard focus 与 App focus 竞态** — keyboard 事件 + 每 `App::update` 都 reconcile（幂等无成本）。
5. **合成器无 manager**（GNOME <45 / 最小 wlroots） — bind 失败仅记一次日志，App 仍能用普通 KeyPressed 走。

---

## 各阶段汇总

| 阶段 | 交付 | 独立可 merge |
|---|---|---|
| **A** | TextField preedit 状态 + `commit_ime` / `delete_surrounding` / `surrounding_text` + 单测 | ✅ 纯逻辑 |
| **B** | TextBox/PasswordBox 渲染 preedit + 下划线 + 光标位移 + 暴露 `ime_context` | ✅ 注入可测 |
| **C** | `InputEvent` 变体 + `ImeContext` 结构 + `App::ime_focus` 默认 + Gallery 级联接线 | ✅ 无 Wayland 依赖 |
| **D** | harness 绑 manager + get_text_input + 事件批处理 + 串行/done + reconcile_ime | ✅ 暗启，App 无需 opt-in |
| **E** | PasswordBox `content_hint = Password` + CONTROL_SPEC 补 preedit 规格 | ✅ |
| **总** | **~940 LOC** | 结构纯增量 |

---

## 建议节奏

阶段 A/B/C 都不动 Wayland，可以先合三步（gallery 里手动注入 InputEvent 验证渲染 + 路由完全没问题）；
阶段 D 是「点亮真实 IME」的开关；E 是护栏。
