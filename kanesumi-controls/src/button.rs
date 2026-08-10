use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{MetroTheme, Rect, Size};

use crate::state::ControlState;

/// 按钮种类：Standard（surface 底）/ Accent（强调色底）。对齐 UWP Standard/Accent。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Standard,
    Accent,
}

/// MetroButton —— 命令按钮。参 CONTROL_SPEC §1：
/// - Padding `8,5,8,6`、无 MinWidth（尺寸 = 内容 + Padding）；
/// - 四态颜色硬切换（无过渡）；按压反馈 = 位移微下沉（PointerDown，~100ms）；
/// - Accent 前景恒白，仅禁用降 40%。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroButton {
    pub label: String,
    pub state: ControlState,
    pub kind: ButtonKind,
}

impl MetroButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            state: ControlState::Normal,
            kind: ButtonKind::Standard,
        }
    }

    pub fn accent(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            state: ControlState::Normal,
            kind: ButtonKind::Accent,
        }
    }

    pub fn set_state(&mut self, state: ControlState) {
        self.state = state;
    }

    /// 固有尺寸：文本宽度 + 左右 8px 内边距（UWP `8,5,8,6`），高 = 行高 + 上下 5/6px。
    pub fn measure(&self, engine: &TextEngine, style: TextStyle) -> Size {
        let width = engine.measure(&self.label, style.size) + 16.0;
        let height = style.line_height + 11.0;
        Size::new(width, height)
    }

    /// 命中测试。
    pub fn hit_test(&self, rect: Rect, pos: kanesumi_core::Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染到 `rect`。顺序：底色 → 交互 tint → 焦点描边 → 居中标签。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let indication = &theme.indication;
        let style = theme.typography.body;

        let (bg, fg) = match self.kind {
            ButtonKind::Standard => (colors.surface, colors.on_surface),
            ButtonKind::Accent => (colors.primary, colors.on_primary),
        };

        let bg = match self.state {
            ControlState::Disabled => bg.with_alpha(bg.a * indication.disabled_opacity),
            _ => bg,
        };
        scene.fill_rounded_rect(bg, rect, theme.tokens.corner_radius);

        match self.state {
            ControlState::Hovered => scene.fill_rect(indication.hover_tint, rect),
            ControlState::Pressed => scene.fill_rect(indication.press_tint, rect),
            _ => {}
        }

        if self.state == ControlState::Focused {
            scene.stroke_rounded_rect(
                indication.focus_stroke,
                rect,
                1.0,
                theme.tokens.corner_radius,
            );
        }

        // 标签居中
        let label_width = engine.measure(&self.label, style.size);
        let label_rect = Rect::new(
            rect.origin.x + (rect.size.width - label_width) / 2.0,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0,
            label_width,
            style.line_height,
        );
        scene.text(self.label.clone(), label_rect, fg, style, TextAlign::Left);
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
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn scene_of(btn: &MetroButton, state: ControlState) -> Scene {
        let p = find_font().expect("测试字体缺失，请设 KANESUMI_TEST_FONT");
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut btn = btn.clone();
        btn.state = state;
        let rect = Rect::new(0.0, 0.0, 100.0, 38.0);
        let mut scene = Scene::default();
        btn.render(&theme, &engine, rect, &mut scene);
        scene
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    #[test]
    fn renders_surface_and_label() {
        if !font_available() {
            return;
        }
        let scene = scene_of(&MetroButton::new("OK"), ControlState::Normal);
        assert_eq!(scene.commands.len(), 2, "底色 + 标签");
        assert!(matches!(scene.commands[0], SceneCommand::FillRect { .. }));
        assert!(matches!(scene.commands[1], SceneCommand::Text { .. }));
    }

    #[test]
    fn focus_adds_stroke() {
        if !font_available() {
            return;
        }
        let scene = scene_of(&MetroButton::new("OK"), ControlState::Focused);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::StrokeRect { .. }))
        );
    }

    #[test]
    fn disabled_reduces_alpha() {
        if !font_available() {
            return;
        }
        let scene = scene_of(&MetroButton::new("OK"), ControlState::Disabled);
        let Some(SceneCommand::FillRect { color, .. }) = scene.commands.first() else {
            panic!("首命令应为底色");
        };
        assert!(color.a < 1.0, "禁用态底色应半透明，实际 a={}", color.a);
    }

    #[test]
    fn measure_grows_with_label() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let style = MetroTheme::default().typography.body;
        let short = MetroButton::new("OK");
        let long = MetroButton::new("很长很长的按钮文本很长很长的按钮文本");
        assert!(long.measure(&engine, style).width > short.measure(&engine, style).width);
    }

    #[test]
    fn hit_test_contains() {
        let btn = MetroButton::new("OK");
        let rect = Rect::new(10.0, 10.0, 100.0, 38.0);
        assert!(btn.hit_test(rect, kanesumi_core::Point::new(60.0, 30.0)));
        assert!(!btn.hit_test(rect, kanesumi_core::Point::new(200.0, 30.0)));
    }
}
