// IME 上下文 —— 控件 → 外壳的输入法契约（参 IME_WIRING_PLAN 阶段 B/C）。
//
// 控件层暴露 `ImeContext`（周边文本 + 光标矩形 + 内容提示），harness 把它映射为
// zwp_text_input_v3 的 set_surrounding_text / set_content_type / set_cursor_rectangle。
// 本模块放控件库（依赖方向 core ← canvas ← controls ← harness），harness 重导出。

use kanesumi_core::Rect;

/// IME 内容提示 —— 决定候选窗 / 软键盘行为（映射 content_hint / content_purpose）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImeContentHint {
    /// 普通文本。
    #[default]
    Normal,
    /// 密码（候选窗自禁 + 不外发周边文本，参 IME_WIRING_PLAN 阶段 E）。
    Password,
    /// 纯数字。
    Digits,
}

/// IME 上下文 —— `App::ime_focus()` 返回值，harness 据此灌上下文。
#[derive(Debug, Clone, PartialEq)]
pub struct ImeContext {
    /// 光标前周边文本（每侧 cap ~1000 字节，UTF-8 边界夹紧）。
    pub surrounding_before: String,
    /// 光标后周边文本。
    pub surrounding_after: String,
    /// 光标在 `before + after` 拼回串中的字节偏移。
    pub cursor_byte: u32,
    /// 选区锚点字节偏移（无选区 = cursor_byte）。
    pub anchor_byte: u32,
    /// 光标矩形（表面本地逻辑像素）。
    pub caret_rect: Rect,
    /// 内容提示（Normal / Password / Digits）。
    pub content_hint: ImeContentHint,
}

impl Default for ImeContext {
    fn default() -> Self {
        Self {
            surrounding_before: String::new(),
            surrounding_after: String::new(),
            cursor_byte: 0,
            anchor_byte: 0,
            caret_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            content_hint: ImeContentHint::Normal,
        }
    }
}
