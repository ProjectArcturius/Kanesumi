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

    /// 渲染（委托 TextBox，掩码已注入）。取 `&mut self` 是因为内层 TextBox 渲染时要推进水平滚动。
    pub fn render(
        &mut self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
    ) {
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

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 text_box.rs）───────────────
//
// 盘点结论：PasswordBox 是 TextBox 的掩码变体，控件自身不含编辑逻辑 —— 元素树侧**整体转发**
// 给被委托的 `boxed`，只在两处修正：聚焦时注入掩码（`new()` 构造未带掩码，若不补，未聚焦
// 的默认实例会渲染明文），以及把 IME 内容提示改成 `Password` 并抹掉周边文本。动作复用
// `text_box::{TextChanged, TextSubmitted}`，不另起同义动作。

impl kanesumi_element::Widget for MetroPasswordBox {
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        <MetroTextBox as kanesumi_element::Widget>::measure(&mut self.boxed, ctx, available)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        <MetroTextBox as kanesumi_element::Widget>::paint(&mut self.boxed, ctx, scene);
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, dt: f64) {
        <MetroTextBox as kanesumi_element::Widget>::update(&mut self.boxed, ctx, dt);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        // 聚焦 = 进入编辑，此时注入掩码（旧 `focus()` 同款；放在转发之前，随后由 TextBox
        // 的 FocusIn 逻辑调用 `boxed.focus()`）。
        if matches!(event, kanesumi_element::Event::FocusIn { .. }) {
            self.boxed.field.set_mask(Some(PASSWORD_MASK_CHAR));
        }
        <MetroTextBox as kanesumi_element::Widget>::event(&mut self.boxed, ctx, event);
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 输入框自绘聚焦边框（2px），不要框架再叠一层焦点视觉。
    fn focus_visual(&self) -> bool {
        false
    }

    fn ime(
        &self,
        rect: Rect,
        theme: &MetroTheme,
        engine: &TextEngine,
    ) -> Option<ImeContext> {
        let mut ctx = <MetroTextBox as kanesumi_element::Widget>::ime(&self.boxed, rect, theme, engine)?;
        // 密码字段：内容提示 = Password，且不外发周边文本（fcitx5 据此自禁候选窗）。
        ctx.content_hint = ImeContentHint::Password;
        ctx.surrounding_before.clear();
        ctx.surrounding_after.clear();
        ctx.cursor_byte = 0;
        ctx.anchor_byte = 0;
        Some(ctx)
    }

    /// 无障碍：只暴露标题 / 占位，**不外发明文**。
    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::TextInput,
            name: if self.boxed.header.is_empty() {
                self.boxed.placeholder.clone()
            } else {
                self.boxed.header.clone()
            },
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::text_box::{TextChanged, TextSubmitted};
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroPasswordBox::with_placeholder("请输入密码"),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    fn password(h: &TestHarness, id: WidgetId) -> String {
        h.tree.get::<MetroPasswordBox>(id).unwrap().password()
    }

    #[test]
    fn tab_focus_masks_and_typing_reports_changed() {
        let (mut h, id) = harness(LayoutProps {
            width: Some(240.0),
            ..LayoutProps::default()
        });
        assert_eq!(h.tree.focused(), None);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Char('s'));
        h.key(Key::Char('e'));
        h.type_text("c");
        assert_eq!(password(&h, id), "sec");
        // 掩码生效：显示层不是明文。
        assert_eq!(
            h.tree.get::<MetroPasswordBox>(id).unwrap().boxed.field.display_text(),
            "●●●"
        );
        let changes = h.take::<TextChanged>();
        assert_eq!(changes.last().unwrap().1, TextChanged("sec".into()));
        // 回车复用 TextSubmitted。
        h.key(Key::Enter);
        assert_eq!(h.take::<TextSubmitted>(), vec![(id, TextSubmitted("sec".into()))]);
    }

    #[test]
    fn ime_context_hints_password_and_omits_surrounding() {
        let (mut h, id) = harness(LayoutProps {
            width: Some(240.0),
            ..LayoutProps::default()
        });
        assert!(h.tree.ime_context().is_none());
        h.click(id);
        let ctx = h.tree.ime_context().expect("聚焦后应有 IME 上下文");
        assert_eq!(ctx.content_hint, ImeContentHint::Password);
        assert!(ctx.surrounding_before.is_empty(), "密码不外发周边文本");
        assert!(ctx.surrounding_after.is_empty());
        assert_eq!(ctx.cursor_byte, 0);
        assert_eq!(ctx.anchor_byte, 0);
        assert!(ctx.caret_rect.size.width > 0.0, "光标矩形仍有效");
    }

    #[test]
    fn passes_insurance_checks_including_squeezed() {
        let (mut h, id) = harness(LayoutProps::default());
        h.move_to(h.center(id));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);

        // 压窄：掩码后的长文本仍须裁在控件内。
        let narrow = h.tree.insert_with(
            h.root(),
            MetroPasswordBox::with_text("一段很长很长的密码用于测试压窄时的裁剪行为"),
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(narrow);
        h.assert_paint_within(narrow, Insets::ZERO);
    }

    #[test]
    fn disabled_box_ignores_input() {
        let (mut h, id) = harness(LayoutProps {
            width: Some(240.0),
            ..LayoutProps::default()
        });
        h.tree.set_enabled(id, false);
        h.frame();
        h.tab();
        assert_ne!(h.tree.focused(), Some(id), "禁用不可聚焦");
        h.type_text("x");
        assert!(h.take::<TextChanged>().is_empty());
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
        let mut p = MetroPasswordBox::with_text("abc");
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
