use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect, Size};

use crate::state::ControlState;

/// MetroIconButton —— 图标按钮（AppBarButton 参考）。参 CONTROL_SPEC §2：
/// - 68 宽 × 56 最小高；图标 16px 上置 + 标签 12px 下置；
/// - 常态背景 Transparent；PointerOver 白 10%、Pressed 白 20%；
/// - 点击不夺焦点（`AllowFocusOnInteraction=false`）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroIconButton {
    /// 图标 glyph（字体字符，如 `\u{E72D}`）。`icon_bitmap` 为 `Some` 时优先用位图。
    pub icon: String,
    /// SVG 光栅化位图（可选）。`Some` 时替代字体 glyph 绘制。
    pub icon_bitmap: Option<Icon>,
    /// 图标染色（`Some` 时按 alpha 蒙版替换颜色；`None` = 原色）。
    pub icon_tint: Option<kanesumi_core::Color>,
    /// 标签（可空 → 纯图标模式，48×48）。
    pub label: String,
    pub state: ControlState,
}

impl MetroIconButton {
    pub fn new(icon: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            icon_bitmap: None,
            icon_tint: None,
            label: String::new(),
            state: ControlState::Normal,
        }
    }

    pub fn with_label(icon: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            icon: icon.into(),
            icon_bitmap: None,
            icon_tint: None,
            label: label.into(),
            state: ControlState::Normal,
        }
    }

    /// 用 SVG 图标构建（`rasterize_svg` 输出）。`target_size` 为图标最长边像素。
    pub fn with_svg(
        svg_path: impl AsRef<std::path::Path>,
        target_size: u32,
        label: impl Into<String>,
    ) -> Option<Self> {
        let icon = kanesumi_canvas::rasterize_svg(svg_path, target_size)?;
        Some(Self {
            icon: String::new(),
            icon_bitmap: Some(icon),
            icon_tint: None,
            label: label.into(),
            state: ControlState::Normal,
        })
    }

    /// 设置 SVG 位图与染色。
    pub fn set_svg_icon(&mut self, icon: Icon, tint: Option<kanesumi_core::Color>) {
        self.icon_bitmap = Some(icon);
        self.icon_tint = tint;
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
    /// 图标：`icon_bitmap` 优先（SVG），否则字体 glyph。
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
            self.draw_icon(fg, icon_rect, scene);
        } else {
            // 图标上置（16px），标签下置（12px）
            let icon_rect = Rect::new(
                rect.origin.x + (rect.size.width - 16.0) / 2.0,
                rect.origin.y + 12.0,
                16.0,
                16.0,
            );
            self.draw_icon(fg, icon_rect, scene);
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

    /// 绘制图标：SVG 位图优先（tint 染色），否则字体 glyph。
    fn draw_icon(&self, fg: kanesumi_core::Color, rect: Rect, scene: &mut Scene) {
        if let Some(icon) = &self.icon_bitmap {
            let tint = self.icon_tint.or(Some(fg));
            scene.image(icon, rect, tint);
        } else {
            scene.text(self.icon.clone(), rect, fg, icon_style(), TextAlign::Center);
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
    use kanesumi_canvas::SceneCommand;

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

    #[test]
    fn svg_bitmap_renders_image_command() {
        // 最小 SVG → IconButton 位图
        let dir = std::env::temp_dir();
        let path = dir.join("kanesumi_iconbtn_test.svg");
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16"><rect width="16" height="16" fill="#fff"/></svg>"##;
        std::fs::write(&path, svg).unwrap();
        let Some(btn) = MetroIconButton::with_svg(&path, 16, "Share") else {
            panic!("SVG 图标构建失败");
        };
        assert!(btn.icon_bitmap.is_some(), "位图已光栅化");
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        btn.render(&theme, &engine, Rect::new(0.0, 0.0, 68.0, 56.0), &mut scene);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Image { .. })),
            "SVG 图标应产出 Image 命令"
        );
    }
}
