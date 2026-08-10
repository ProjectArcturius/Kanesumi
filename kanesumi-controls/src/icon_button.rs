use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect, Scene, Size, TextAlign};

use crate::state::ControlState;

/// MetroIconButton —— 图标按钮（AppBarButton 参考）。参 CONTROL_SPEC §2：
/// - 68 宽 × 56 最小高；图标 16px 上置 + 标签 12px 下置；
/// - 常态背景 Transparent；PointerOver 白 10%、Pressed 白 20%；
/// - 点击不夺焦点（`AllowFocusOnInteraction=false`）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroIconButton {
    /// 图标 glyph（字体字符，如 `\u{E72D}`）。
    pub icon: String,
    /// 标签（可空 → 纯图标模式，48×48）。
    pub label: String,
    pub state: ControlState,
}

impl MetroIconButton {
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: String::new(),
            state: ControlState::Normal,
        }
    }

    pub fn with_label(icon: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            label: label.into(),
            state: ControlState::Normal,
        }
    }

    pub fn set_state(&mut self, state: ControlState) {
        self.state = state;
    }

    /// 固有尺寸：带标签 68×56，纯图标 48×48。
    pub fn measure(&self) -> Size {
        if self.label.is_empty() {
            Size::new(48.0, 48.0)
        } else {
            Size::new(68.0, 56.0)
        }
    }

    /// 命中测试。
    pub fn hit_test(&self, rect: Rect, pos: kanesumi_core::Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染到 `rect`。顺序：交互 tint 底 → 图标 → 标签。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let indication = &theme.indication;

        match self.state {
            ControlState::Hovered => scene.fill_rect(indication.hover_tint, rect),
            ControlState::Pressed => scene.fill_rect(indication.press_tint, rect),
            _ => {}
        }
        if self.state == ControlState::Focused {
            scene.stroke_rect(indication.focus_stroke, rect, 1.0);
        }

        let fg = if self.state == ControlState::Disabled {
            colors
                .on_surface
                .with_alpha(colors.on_surface.a * indication.disabled_opacity)
        } else {
            colors.on_surface
        };

        if self.label.is_empty() {
            // 纯图标：居中
            let icon_rect = Rect::new(
                rect.origin.x + (rect.size.width - 16.0) / 2.0,
                rect.origin.y + (rect.size.height - 16.0) / 2.0,
                16.0,
                16.0,
            );
            scene.text(
                self.icon.clone(),
                icon_rect,
                fg,
                icon_style(),
                TextAlign::Center,
            );
        } else {
            // 图标上置（16px），标签下置（12px）
            let icon_rect = Rect::new(
                rect.origin.x + (rect.size.width - 16.0) / 2.0,
                rect.origin.y + 12.0,
                16.0,
                16.0,
            );
            scene.text(
                self.icon.clone(),
                icon_rect,
                fg,
                icon_style(),
                TextAlign::Center,
            );
            let label_style = label_style();
            let label_rect = Rect::new(
                rect.origin.x + 2.0,
                rect.origin.y + rect.size.height - 8.0 - label_style.line_height,
                rect.size.width - 4.0,
                label_style.line_height,
            );
            scene.text(
                self.label.clone(),
                label_rect,
                fg,
                label_style,
                TextAlign::Center,
            );
        }
    }
}

/// 图标字形样式：16px 正常。
fn icon_style() -> kanesumi_core::TextStyle {
    kanesumi_core::TextStyle::new(16.0, 16.0, kanesumi_core::FontWeight::Normal)
}

/// 标签样式：12px 正常。
fn label_style() -> kanesumi_core::TextStyle {
    kanesumi_core::TextStyle::new(12.0, 16.0, kanesumi_core::FontWeight::Normal)
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
    fn measure_follows_label() {
        assert_eq!(MetroIconButton::new("x").measure(), Size::new(48.0, 48.0));
        assert_eq!(
            MetroIconButton::with_label("x", "Share").measure(),
            Size::new(68.0, 56.0)
        );
    }

    #[test]
    fn renders_icon_and_label() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let btn = MetroIconButton::with_label("\u{E72D}", "Share");
        let mut scene = Scene::default();
        btn.render(&theme, &engine, Rect::new(0.0, 0.0, 68.0, 56.0), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 2, "图标 + 标签");
    }

    #[test]
    fn hover_adds_tint() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut btn = MetroIconButton::new("x");
        btn.state = ControlState::Hovered;
        let mut scene = Scene::default();
        btn.render(&theme, &engine, Rect::new(0.0, 0.0, 48.0, 48.0), &mut scene);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { .. }))
        );
    }

    #[test]
    fn hit_test_contains() {
        let btn = MetroIconButton::new("x");
        let rect = Rect::new(0.0, 0.0, 48.0, 48.0);
        assert!(btn.hit_test(rect, kanesumi_core::Point::new(24.0, 24.0)));
        assert!(!btn.hit_test(rect, kanesumi_core::Point::new(100.0, 100.0)));
    }

    #[test]
    fn disabled_lowers_fg() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut btn = MetroIconButton::new("x");
        btn.state = ControlState::Disabled;
        let mut scene = Scene::default();
        btn.render(&theme, &engine, Rect::new(0.0, 0.0, 48.0, 48.0), &mut scene);
        let Some(SceneCommand::Text { color, .. }) = scene
            .commands
            .iter()
            .find(|c| matches!(c, SceneCommand::Text { .. }))
        else {
            panic!("应画图标文本");
        };
        assert!(color.a < 1.0, "禁用态前景应降透明度");
    }
}
