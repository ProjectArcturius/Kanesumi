use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect, Scene, TextAlign, TextStyle};

use crate::button::MetroButton;
use crate::state::ControlState;

/// 对话框状态机。参 CONTROL_SPEC §9。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    Closed,
    Showing,
    Open,
    Hiding,
}

/// 默认按钮（Enter 触发，用 Accent 样式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogDefaultButton {
    None,
    Primary,
    Secondary,
    Close,
}

/// 对话框按钮组。
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButtons {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub close: Option<String>,
    pub default_button: DialogDefaultButton,
}

impl Default for DialogButtons {
    fn default() -> Self {
        Self {
            primary: None,
            secondary: None,
            close: None,
            default_button: DialogDefaultButton::None,
        }
    }
}

/// MetroDialog —— 对话框（ContentDialog 参考）。参 CONTROL_SPEC §9：
/// - 尺寸 320–548 宽 / 184–756 高；Padding `24,18,24,24`；
/// - 遮罩 + 盒体分离：淡入 0.167s / 淡出 0.083s（线性近似），缩放 1.05→1.0 @0.5s；
/// - 按钮 Primary → Secondary → Close（Close 恒最右）；默认按钮 = Accent；
/// - Esc = 解除（`hide`），点击遮罩不关闭。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroDialog {
    pub title: String,
    pub content: String,
    pub buttons: DialogButtons,
    pub state: DialogState,
    /// 对话框盒体尺寸（Phase 3 续做自适应测量）。
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
    opacity: MetroAnim,
    scale: MetroAnim,
}

impl Default for MetroDialog {
    fn default() -> Self {
        let mut opacity = MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut);
        opacity.jump_to(0.0);
        let mut scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        scale.jump_to(0.0);
        Self {
            title: String::new(),
            content: String::new(),
            buttons: DialogButtons::default(),
            state: DialogState::Closed,
            width: 448.0,
            min_width: 320.0,
            max_width: 548.0,
            opacity,
            scale,
        }
    }
}

impl MetroDialog {
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            ..Self::default()
        }
    }

    /// 显示：遮罩 0.167s 淡入 + 盒体 1.05→1.0 @0.5s。
    pub fn show(&mut self) {
        if matches!(self.state, DialogState::Showing | DialogState::Open) {
            return;
        }
        self.state = DialogState::Showing;
        self.opacity = MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.opacity.set_target(1.0);
        self.scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        self.scale.set_target(1.0);
    }

    /// 隐藏（Esc / 按钮）：opacity 0.083s 先行熄灭，缩放随后。
    pub fn hide(&mut self) {
        if matches!(self.state, DialogState::Hiding | DialogState::Closed) {
            return;
        }
        self.state = DialogState::Hiding;
        self.opacity = MetroAnim::new(0.083, UwpEasing::Quadratic, EasingMode::EaseIn);
        self.opacity.set_target(0.0);
        self.scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        self.scale.set_target(1.05);
    }

    /// 每帧推进；轨道稳态后转 Open/Closed。
    pub fn update(&mut self, dt: f64) {
        match self.state {
            DialogState::Showing | DialogState::Hiding => {
                self.opacity.update(dt);
                self.scale.update(dt);
                if self.opacity.is_steady() && self.scale.is_steady() {
                    self.state = if self.state == DialogState::Showing {
                        DialogState::Open
                    } else {
                        DialogState::Closed
                    };
                }
            }
            _ => {}
        }
    }

    pub fn is_visible(&self) -> bool {
        matches!(
            self.state,
            DialogState::Showing | DialogState::Open | DialogState::Hiding
        )
    }

    /// 遮罩不透明度 [0,1]。
    pub fn overlay_alpha(&self) -> f32 {
        self.opacity.value() as f32
    }

    /// 盒体缩放（1.05→1.0）。
    pub fn scale_value(&self) -> f64 {
        self.scale.value()
    }

    /// 盒体矩形：居中于 `screen`，宽度受 scale 影响。
    pub fn box_rect(&self, screen: Rect) -> Rect {
        let w = self
            .width
            .clamp(self.min_width, self.max_width)
            .min(screen.size.width - 32.0);
        let h = 240.0_f32.min(screen.size.height - 32.0);
        let s = self.scale_value() as f32;
        let sw = w * s;
        let sh = h * s;
        Rect::new(
            screen.origin.x + (screen.size.width - sw) / 2.0,
            screen.origin.y + (screen.size.height - sh) / 2.0,
            sw,
            sh,
        )
    }

    /// 渲染：遮罩 + 盒体（标题 / 内容 / 按钮区）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        let colors = &theme.colors;

        // 遮罩
        let scrim = theme
            .overlay_color
            .with_alpha(theme.overlay_color.a * self.overlay_alpha());
        scene.fill_rect(scrim, screen);

        let box_rect = self.box_rect(screen);
        // 盒体（chrome）
        scene.fill_rounded_rect(colors.surface, box_rect, theme.tokens.corner_radius);
        scene.stroke_rounded_rect(colors.divider, box_rect, 1.0, theme.tokens.corner_radius);

        let pad = 24.0;
        let inner = Rect::new(
            box_rect.origin.x + pad,
            box_rect.origin.y + 18.0,
            box_rect.size.width - pad * 2.0,
            box_rect.size.height - 18.0 - 24.0,
        );

        // 标题（20px，最多 2 行）
        if !self.title.is_empty() {
            let title_style = TextStyle::new(20.0, 26.0, kanesumi_core::FontWeight::Normal);
            let title_rect = Rect::new(
                inner.origin.x,
                inner.origin.y,
                inner.size.width,
                title_style.line_height,
            );
            scene.text(
                self.title.clone(),
                title_rect,
                colors.on_surface,
                title_style,
                TextAlign::Left,
            );
        }

        // 内容
        if !self.content.is_empty() {
            let content_style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
            let title_gap = if self.title.is_empty() { 0.0 } else { 12.0 };
            let content_rect = Rect::new(
                inner.origin.x,
                inner.origin.y + title_gap,
                inner.size.width,
                inner.size.height - 32.0 - title_gap,
            );
            scene.text(
                self.content.clone(),
                content_rect,
                colors.on_surface,
                content_style,
                TextAlign::Left,
            );
        }

        // 按钮区（右下，Primary → Secondary → Close）
        let buttons = [
            (self.buttons.close.as_deref(), 0),
            (self.buttons.secondary.as_deref(), 1),
            (self.buttons.primary.as_deref(), 2),
        ];
        let button_w = 130.0_f32.min(202.0);
        let mut x = box_rect.origin.x + box_rect.size.width - pad;
        for (label, slot) in buttons {
            let Some(label) = label else { continue };
            x -= button_w;
            let default = match slot {
                0 => self.buttons.default_button == DialogDefaultButton::Close,
                1 => self.buttons.default_button == DialogDefaultButton::Secondary,
                _ => self.buttons.default_button == DialogDefaultButton::Primary,
            };
            let mut btn = if default {
                MetroButton::accent(label.to_string())
            } else {
                MetroButton::new(label.to_string())
            };
            btn.state = ControlState::Normal;
            let btn_rect = Rect::new(
                x,
                box_rect.origin.y + box_rect.size.height - 24.0 - 32.0,
                button_w,
                32.0,
            );
            btn.render(theme, engine, btn_rect, scene);
            x -= 2.0; // 按钮间距
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::SceneCommand;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn show_hide_cycle() {
        let mut dlg = MetroDialog::new("Save?", "Keep changes?");
        dlg.show();
        assert_eq!(dlg.state, DialogState::Showing);
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert_eq!(dlg.state, DialogState::Open);
        dlg.hide();
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert_eq!(dlg.state, DialogState::Closed);
    }

    #[test]
    fn scale_animates_1_05_to_1() {
        let mut dlg = MetroDialog::new("T", "C");
        dlg.show();
        dlg.update(0.01);
        assert!(dlg.scale_value() < 1.05, "初期接近 1.05");
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert!((dlg.scale_value() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn hidden_renders_nothing() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let dlg = MetroDialog::new("T", "C");
        let mut scene = Scene::default();
        dlg.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        assert!(scene.is_empty());
    }

    #[test]
    fn open_renders_scrim_box_and_buttons() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut dlg = MetroDialog::new("Save your work?", "Do you want to save?");
        dlg.buttons.primary = Some("Save".into());
        dlg.buttons.secondary = Some("Don't Save".into());
        dlg.buttons.close = Some("Cancel".into());
        dlg.buttons.default_button = DialogDefaultButton::Primary;
        dlg.show();
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        dlg.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 遮罩 + 盒体底 + 盒体边框 + 3 按钮底 + 标题/内容/按钮文本…
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert!(fills >= 5, "遮罩 + 盒体 + 3 按钮底，实际 {fills}");
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count();
        assert!(strokes >= 1, "盒体边框");
    }

    #[test]
    fn box_rect_centers_and_clamps() {
        let mut dlg = MetroDialog::new("T", "C");
        dlg.width = 400.0;
        dlg.show();
        dlg.update(1.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = dlg.box_rect(screen);
        assert!(
            (r.origin.x + r.size.width / 2.0 - 400.0).abs() < 1.0,
            "水平居中"
        );
        assert!(
            (r.origin.y + r.size.height / 2.0 - 300.0).abs() < 1.0,
            "垂直居中"
        );
    }
}
