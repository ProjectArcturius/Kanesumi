// MetroPasswordBox —— 密码输入框。参 CONTROL_SPEC §35（PasswordBox 参考，闭源）。
//
// 数据源：PasswordBox 是 TextBox 的掩码变体（Windows.UI.Xaml 平台内置，无独立模板
// 源码；掩码字符默认 `●`）。Kanesumi 以 `MetroTextBox` + `TextField::set_mask` 实现：
// 显示层用 `●` 掩码，明文保留在 `field.text()` 供提交。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::ime::{ImeContentHint, ImeContext};
use crate::state::ControlState;
use crate::text_box::MetroTextBox;
use crate::text_field::{TextInputKey, TextField};

/// 掩码字符（UWP PasswordBox 默认 `●`）。
pub const PASSWORD_MASK_CHAR: char = '●';

/// MetroPasswordBox —— 掩码文本输入。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroPasswordBox {
    pub boxed: MetroTextBox,
}

impl Default for MetroPasswordBox {
    fn default() -> Self {
        Self {
            boxed: MetroTextBox::new(),
        }
    }
}

impl MetroPasswordBox {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带占位文本构造（如「请输入密码」）。
    pub fn with_placeholder(text: impl Into<String>) -> Self {
        Self {
            boxed: MetroTextBox::with_placeholder(text),
        }
    }

    /// 带标题构造。
    pub fn with_header(text: impl Into<String>) -> Self {
        Self {
            boxed: MetroTextBox::with_header(text),
        }
    }

    /// 设置初始明文（光标置尾，掩码显示）。
    pub fn with_text(text: impl Into<String>) -> Self {
        let mut boxed = MetroTextBox::from_text(text);
        boxed.field.set_mask(Some(PASSWORD_MASK_CHAR));
        Self { boxed }
    }

    /// 明文。
    pub fn password(&self) -> String {
        self.boxed.field.text()
    }

    /// 编辑核心（宿主可复用以 set_mask 自定义掩码）。
    pub fn field(&self) -> &TextField {
        &self.boxed.field
    }

    pub fn field_mut(&mut self) -> &mut TextField {
        &mut self.boxed.field
    }

    /// 进入聚焦（全选 + 掩码）。
    pub fn focus(&mut self) {
        self.boxed.field.set_mask(Some(PASSWORD_MASK_CHAR));
        self.boxed.focus();
    }

    /// 失焦。
    pub fn blur(&mut self) {
        self.boxed.blur();
    }

    /// 每帧推进（光标闪烁）。
    pub fn update(&mut self, dt: f64) {
        self.boxed.update(dt);
    }

    /// 处理编辑键。
    pub fn handle_key(&mut self, key: TextInputKey) -> bool {
        self.boxed.handle_key(key)
    }

    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        self.boxed.hit_test(rect, pos)
    }

    pub fn focused(&self) -> bool {
        self.boxed.focused
    }

    pub fn state(&self) -> ControlState {
        self.boxed.state
    }

    /// 渲染（委托 TextBox，掩码已注入）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        self.boxed.render(theme, engine, rect, scene);
    }

    /// IME 上下文 —— 内容提示 = Password，且**不外发周边文本**（敏感字段不暴露
    /// 给输入法；fcitx5 收到 password|sensitive_data|hidden_text 自禁候选窗）。
    /// 光标矩形仍有效（掩码宽度）。
    pub fn ime_context(&self, theme: &MetroTheme, engine: &TextEngine, body: Rect) -> ImeContext {
        let mut ctx = self.boxed.ime_context(theme, engine, body);
        ctx.content_hint = ImeContentHint::Password;
        ctx.surrounding_before.clear();
        ctx.surrounding_after.clear();
        ctx.cursor_byte = 0;
        ctx.anchor_byte = 0;
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    #[test]
    fn mask_hides_password() {
        let mut p = MetroPasswordBox::new();
        p.focus();
        for c in ['s', 'e', 'c'] {
            p.handle_key(TextInputKey::Char(c));
        }
        assert_eq!(p.password(), "sec");
        assert_eq!(p.boxed.field.display_text(), "●●●");
    }

    #[test]
    fn explicit_text_keeps_plaintext() {
        let p = MetroPasswordBox::with_text("hunter2");
        assert_eq!(p.password(), "hunter2");
        assert_eq!(p.boxed.field.display_text(), "●●●●●●●");
    }

    #[test]
    fn render_emits_masked_text() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let p = MetroPasswordBox::with_text("abc");
        let mut scene = Scene::default();
        p.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        let texts: Vec<String> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "●●●"));
        assert!(!texts.iter().any(|t| t == "abc"), "绝不渲染明文");
    }

    #[test]
    fn editing_via_handle_key() {
        let mut p = MetroPasswordBox::new();
        p.focus();
        p.handle_key(TextInputKey::Char('a'));
        p.handle_key(TextInputKey::Char('b'));
        p.handle_key(TextInputKey::Backspace);
        assert_eq!(p.password(), "a");
    }

    // ── IME 组合态（阶段 B/E，参 IME_WIRING_PLAN） ──────────────

    #[test]
    fn preedit_is_masked() {
        let mut p = MetroPasswordBox::with_text("ab");
        p.boxed.field.move_end(false);
        p.boxed.field.set_preedit("cd", None);
        assert_eq!(p.boxed.field.preedit(), "cd", "明文保留");
        assert_eq!(p.boxed.field.preedit_display(), "●●", "组合态也掩码");
    }

    #[test]
    fn ime_context_hints_password_and_omits_surrounding() {
        let e = TextEngine::load(find_font().unwrap()).unwrap();
        let th = MetroTheme::ether_dark();
        let p = MetroPasswordBox::with_text("hunter2");
        let ctx = p.ime_context(&th, &e, Rect::new(0.0, 0.0, 200.0, 32.0));
        assert_eq!(ctx.content_hint, crate::ime::ImeContentHint::Password);
        assert!(ctx.surrounding_before.is_empty(), "密码不外发周边文本");
        assert!(ctx.surrounding_after.is_empty());
        assert_eq!(ctx.cursor_byte, 0);
        assert_eq!(ctx.anchor_byte, 0);
        assert!(ctx.caret_rect.size.width > 0.0, "光标矩形仍有效");
    }
}
