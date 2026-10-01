// Brush —— 颜色引用：字面量或主题令牌（XAML `{ThemeResource}` 的对应物）。
//
// 为什么（2026-10-01）：元素树页面在构建时把 `theme.colors.surface` 抄成字面量写进 Border /
// Label，主题切换（深 ↔ 浅、换 accent）时这些值不会变 —— Ether 无头实测：切到浅色后控件变浅、
// 页面底仍是深色，标题 / 标签成了「深字压深底」完全看不见。令牌在**绘制时**按当前主题解析，
// 主题一换（`Tree::set_theme` 全量重画）自然跟随。
//
// 规则：应用代码里凡是「主题色」一律写 `ThemeColor::*`；只有真正不随主题变的颜色（品牌色、
// 图片底板）才写字面量 `Color`。

use crate::color::Color;
use crate::theme::MetroTheme;

/// 主题颜色令牌 —— 一一对应 [`crate::MetroColors`] 字段与主题级颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeColor {
    Background,
    Surface,
    SurfaceVariant,
    Divider,
    Primary,
    PrimaryHover,
    PrimaryPressed,
    OnPrimary,
    OnBackground,
    OnSurface,
    OnSurfaceVariant,
    SelectionTint,
    TextSelectionTint,
    TrackSubtle,
    FocusStroke,
    /// 控件笔刷族（参 `MetroColors` 同名字段）。
    ControlFill,
    ControlStroke,
    ControlStrokeStrong,
    /// 弹层遮罩色（`MetroTheme::overlay_color`）。
    Overlay,
}

impl ThemeColor {
    pub fn resolve(self, theme: &MetroTheme) -> Color {
        let c = &theme.colors;
        match self {
            Self::Background => c.background,
            Self::Surface => c.surface,
            Self::SurfaceVariant => c.surface_variant,
            Self::Divider => c.divider,
            Self::Primary => c.primary,
            Self::PrimaryHover => c.primary_hover,
            Self::PrimaryPressed => c.primary_pressed,
            Self::OnPrimary => c.on_primary,
            Self::OnBackground => c.on_background,
            Self::OnSurface => c.on_surface,
            Self::OnSurfaceVariant => c.on_surface_variant,
            Self::SelectionTint => c.selection_tint,
            Self::TextSelectionTint => c.text_selection_tint,
            Self::TrackSubtle => c.track_subtle,
            Self::FocusStroke => c.focus_stroke,
            Self::ControlFill => c.control_fill,
            Self::ControlStroke => c.control_stroke,
            Self::ControlStrokeStrong => c.control_stroke_strong,
            Self::Overlay => theme.overlay_color,
        }
    }
}

/// 颜色引用：字面量，或主题令牌（可再乘一层不透明度）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Brush {
    Solid(Color),
    Theme(ThemeColor),
    /// 令牌 × 不透明度（如「分隔线 50%」）。
    ThemeAlpha(ThemeColor, f32),
}

impl Brush {
    /// 按当前主题解析为具体颜色（绘制时调用）。
    pub fn resolve(self, theme: &MetroTheme) -> Color {
        match self {
            Self::Solid(c) => c,
            Self::Theme(t) => t.resolve(theme),
            Self::ThemeAlpha(t, a) => {
                let c = t.resolve(theme);
                c.with_alpha(c.a * a.clamp(0.0, 1.0))
            }
        }
    }
}

impl From<Color> for Brush {
    fn from(c: Color) -> Self {
        Self::Solid(c)
    }
}

impl From<ThemeColor> for Brush {
    fn from(t: ThemeColor) -> Self {
        Self::Theme(t)
    }
}

impl ThemeColor {
    /// `ThemeColor::Divider.alpha(0.5)` —— 令牌 × 不透明度。
    pub fn alpha(self, a: f32) -> Brush {
        Brush::ThemeAlpha(self, a)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Accent;

    #[test]
    fn theme_tokens_follow_scheme() {
        let dark = MetroTheme::dark(Accent::default());
        let light = MetroTheme::light(Accent::default());
        let b: Brush = ThemeColor::Surface.into();
        assert_eq!(b.resolve(&dark), dark.colors.surface);
        assert_eq!(b.resolve(&light), light.colors.surface);
        assert_ne!(b.resolve(&dark), b.resolve(&light), "同一令牌深浅方案取值不同");
    }

    #[test]
    fn solid_is_fixed_and_alpha_multiplies() {
        let dark = MetroTheme::dark(Accent::default());
        let red = Color::rgb(1.0, 0.0, 0.0);
        assert_eq!(Brush::from(red).resolve(&dark), red);
        let half = ThemeColor::OnSurface.alpha(0.5).resolve(&dark);
        assert!((half.a - dark.colors.on_surface.a * 0.5).abs() < 1e-6);
    }
}
