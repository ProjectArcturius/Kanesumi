// MetroInfoBadge —— 徽标（Dock 应用未读计数 / 通知角标）。参 CONTROL_SPEC §14。
//
// 移植自 microsoft-ui-xaml/dev/InfoBadge（InfoBadge.cpp + InfoBadge_themeresources.xaml）：
// - Value >= 0 → 数字文本（>99 → "99+"），FontSize 11、Padding 4,0,4,2；
// - 有 icon 且 Value < 0 → 图标态（Kanesumi 暂以文本 glyph 承载）；
// - 均无 → 最小 4×4 圆点。
// - CornerRadius = ActualHeight/2（全胶囊）；MeasureOverride：W<H 时强制方形。
// 底/前景：强调色 / on_primary（上游 AccentFillColorDefault / TextOnAccentFillColorPrimary）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{CornerRadius, FontWeight, MetroTheme, Rect, Size};

/// InfoBadge 派生风格底色（对齐 InfoBar Severity 图标色，参 CONTROL_SPEC §14 表）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoBadgeKind {
    /// 默认强调色（accent）。
    Accent,
    Attention,
    Success,
    Caution,
    Critical,
}

impl InfoBadgeKind {
    /// 徽标底色。
    fn color(self, theme: &MetroTheme) -> kanesumi_core::Color {
        match self {
            InfoBadgeKind::Accent => theme.colors.primary,
            InfoBadgeKind::Attention => kanesumi_core::Color::from_hex(0x4F_C1_FF),
            InfoBadgeKind::Success => kanesumi_core::Color::from_hex(0x4C_C3_8A),
            InfoBadgeKind::Caution => kanesumi_core::Color::from_hex(0xE5_A9_4E),
            InfoBadgeKind::Critical => kanesumi_core::Color::from_hex(0xE5_53_4A),
        }
    }
}

/// MetroInfoBadge —— 角标。参 CONTROL_SPEC §14。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroInfoBadge {
    /// 数值显示：`>= 0` 显示数字；`< 0` 走 icon / dot。
    pub value: i32,
    /// 图标文本（Value < 0 且 Some 时显示）。
    pub icon: Option<String>,
    /// 派生风格（仅影响底色）。
    pub kind: InfoBadgeKind,
}

impl Default for MetroInfoBadge {
    fn default() -> Self {
        Self {
            value: -1,
            icon: None,
            kind: InfoBadgeKind::Accent,
        }
    }
}

impl MetroInfoBadge {
    pub fn new() -> Self {
        Self::default()
    }

    /// 数值徽标（`> 99` 渲染为 "99+"）。
    pub fn value(value: i32) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }

    /// 圆点徽标。
    pub fn dot() -> Self {
        Self {
            value: -1,
            icon: None,
            kind: InfoBadgeKind::Accent,
        }
    }

    /// 图标徽标（Value 必须 < 0 才生效）。
    pub fn icon(glyph: impl Into<String>) -> Self {
        Self {
            value: -1,
            icon: Some(glyph.into()),
            kind: InfoBadgeKind::Accent,
        }
    }

    /// 当前显示文本（Value 态）。>99 → "99+"；否则数字。
    pub fn display_text(&self) -> Option<String> {
        if self.value >= 0 {
            Some(if self.value > 99 {
                "99+".to_string()
            } else {
                self.value.to_string()
            })
        } else if self.icon.is_some() {
            self.icon.clone()
        } else {
            None
        }
    }

    /// 显示形态：Value 数字 / 图标 / 圆点。
    pub fn is_dot(&self) -> bool {
        self.value < 0 && self.icon.is_none()
    }

    /// 值文本样式：11px（`InfoBadgeValueFontSize`）。
    pub fn value_style() -> TextStyle {
        TextStyle::new(11.0, 14.0, FontWeight::Normal)
    }

    /// 固有尺寸（含 Padding `4,0,4,2`；图标态 `4,4,4,4`）。
    /// 圆点 = 4×4；否则内容宽 + 左右 Padding，高 = 行高 + 上下 Padding。
    pub fn measure(&self, engine: &TextEngine) -> Size {
        if self.is_dot() {
            return Size::new(4.0, 4.0);
        }
        let style = Self::value_style();
        let text = self.display_text().unwrap_or_default();
        let (pad_x, pad_top, pad_bottom) = if self.value >= 0 {
            (4.0, 0.0, 2.0)
        } else {
            (4.0, 4.0, 4.0)
        };
        let text_w = engine.measure(&text, style.size);
        let w = text_w + pad_x * 2.0;
        let h = style.line_height + pad_top + pad_bottom;
        Size::new(w, h)
    }

    /// 渲染到 `rect`。全胶囊（CornerRadius = 高/2）；W < H 时按方形（H×H）居中绘制。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let _ = engine;
        let bg = self.kind.color(theme);
        let side = rect.size.width.min(rect.size.height).max(4.0);
        // MeasureOverride：W<H → 短边取齐（方形），但保留给定位置、按 H 定宽居中。
        let size = if rect.size.width < rect.size.height {
            Size::new(side, side)
        } else {
            rect.size
        };
        let badge_rect = Rect::new(
            rect.origin.x + (rect.size.width - size.width) / 2.0,
            rect.origin.y + (rect.size.height - size.height) / 2.0,
            size.width,
            size.height,
        );

        // 底色 —— 全胶囊（CornerRadius::Capsule = 短边一半 → 方形时即圆）。
        scene.fill_rounded_rect(bg, badge_rect, CornerRadius::Capsule);

        if let Some(text) = self.display_text() {
            let style = Self::value_style();
            let (pad_top, pad_bottom) = if self.value >= 0 {
                (0.0, 2.0)
            } else {
                (4.0, 4.0)
            };
            let text_rect = Rect::new(
                badge_rect.origin.x,
                badge_rect.origin.y + pad_top,
                badge_rect.size.width,
                style.line_height,
            );
            scene.text(
                text,
                text_rect,
                theme.colors.on_primary,
                style,
                TextAlign::Center,
            );
            let _ = pad_bottom;
        }
    }
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
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn dot_is_minimum_and_square() {
        let badge = MetroInfoBadge::dot();
        assert!(badge.is_dot());
        let Some(engine) = find_engine() else { return };
        let size = badge.measure(&engine);
        assert_eq!(size.width, 4.0);
        assert_eq!(size.height, 4.0);
    }

    #[test]
    fn value_clamps_to_99_plus() {
        let badge = MetroInfoBadge::value(150);
        assert_eq!(badge.display_text().as_deref(), Some("99+"));
        assert_eq!(
            MetroInfoBadge::value(42).display_text().as_deref(),
            Some("42")
        );
        assert_eq!(
            MetroInfoBadge::value(99).display_text().as_deref(),
            Some("99")
        );
    }

    #[test]
    fn value_renders_accent_fill_and_text() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let badge = MetroInfoBadge::value(7);
        let size = badge.measure(&engine);
        let rect = Rect::new(0.0, 0.0, size.width, size.height);
        let mut scene = Scene::default();
        badge.render(&theme, &engine, rect, &mut scene);
        // 底色 Fill + 文本
        let fills: Vec<_> = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .collect();
        assert_eq!(fills.len(), 1, "一个底色填充");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 1, "数值文本");
    }

    #[test]
    fn dot_renders_no_text() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let badge = MetroInfoBadge::dot();
        let mut scene = Scene::default();
        badge.render(&theme, &engine, Rect::new(0.0, 0.0, 4.0, 4.0), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 0, "圆点无文本");
        // 全胶囊 = 圆
        let SceneCommand::FillRect { corner_radius, .. } = &scene.commands[0] else {
            panic!("应画底色");
        };
        assert_eq!(*corner_radius, 2.0, "4×4 圆点 CornerRadius = 高/2 = 2");
    }

    #[test]
    fn kind_colors_map() {
        let theme = MetroTheme::ether_dark();
        assert_eq!(InfoBadgeKind::Accent.color(&theme), theme.colors.primary);
        assert_eq!(
            InfoBadgeKind::Success.color(&theme),
            kanesumi_core::Color::from_hex(0x4C_C3_8A)
        );
    }
}
