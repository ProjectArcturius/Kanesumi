use crate::accent::{Accent, ColorScheme};
use crate::color::Color;
use crate::colors::MetroColors;
use crate::indicator::MetroIndication;
use crate::tokens::Tokens;
use crate::typography::MetroTypography;

/// 主题容器 —— 单一渲染权威。参 SD §III。
///
/// 由 `(scheme, accent)` 唯一决定：**不存在「暗色写死一套、亮色另写一套」的两份常量**，
/// 因此不可能出现「改了 accent、某个控件忘了跟着变」。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroTheme {
    /// 颜色方案（dark / light）。
    pub scheme: ColorScheme,
    /// 强调色全阶（配置真源为 Chorus）。
    pub accent: Accent,
    pub colors: MetroColors,
    pub typography: MetroTypography,
    pub tokens: Tokens,
    pub indication: MetroIndication,
    /// 弹层遮罩色（对话框 / 下拉）。
    ///
    /// 暗色：黑 70% 强压背景 —— Ether 的 `surface` 只比 `background` 亮一档，
    /// 若按 UWP dark 的白 60% 洗亮，背景反而比对话框更亮，视觉层次倒挂
    /// （参 CONTROL_SPEC §9 + VISUAL_ISSUES V9）。
    /// 亮色：白 60%（UWP light 的常规洗亮方向）。
    pub overlay_color: Color,
}

impl MetroTheme {
    /// 暗色方案。
    pub fn dark(accent: Accent) -> Self {
        let indication = MetroIndication::for_scheme(ColorScheme::Dark, accent);
        Self {
            scheme: ColorScheme::Dark,
            accent,
            colors: MetroColors::dark(accent),
            typography: MetroTypography::metro(),
            tokens: Tokens::ether(),
            indication,
            overlay_color: Color::BLACK.with_alpha(0.7),
        }
    }

    /// 亮色方案。
    pub fn light(accent: Accent) -> Self {
        let indication = MetroIndication::for_scheme(ColorScheme::Light, accent);
        Self {
            scheme: ColorScheme::Light,
            accent,
            colors: MetroColors::light(accent),
            typography: MetroTypography::metro(),
            tokens: Tokens::ether(),
            indication,
            overlay_color: Color::WHITE.with_alpha(0.6),
        }
    }

    /// 按方案分派。
    pub fn for_scheme(scheme: ColorScheme, accent: Accent) -> Self {
        match scheme {
            ColorScheme::Dark => Self::dark(accent),
            ColorScheme::Light => Self::light(accent),
        }
    }

    /// 兼容别名：旧调用点（`MetroTheme::ether_dark()`）= 暗色 + 默认 accent。
    pub fn ether_dark() -> Self {
        Self::dark(Accent::default())
    }
}

impl Default for MetroTheme {
    fn default() -> Self {
        Self::ether_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ether_dark_is_default() {
        let t = MetroTheme::default();
        assert_eq!(t, MetroTheme::ether_dark());
        assert_eq!(t.scheme, ColorScheme::Dark);
        assert_eq!(t.colors.primary, MetroColors::ether_dark().primary);
    }

    /// 主题自洽：colors 的强调令牌、indication 的焦点描边、theme.accent 必须同源。
    #[test]
    fn theme_accent_is_single_source() {
        let accent = Accent::parse("00897B");
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let t = MetroTheme::for_scheme(scheme, accent);
            assert_eq!(t.accent.base, accent.base);
            assert_eq!(t.colors.primary, accent.base);
            assert_eq!(t.colors.focus_stroke, t.indication.focus_stroke);
            assert_eq!(t.indication.focus_stroke, accent.focus_for(scheme));
        }
    }

    /// 深浅两态都可构造且前景方向相反 —— 这是「深浅兼容」的最小验收。
    #[test]
    fn both_schemes_are_usable() {
        let d = MetroTheme::dark(Accent::default());
        let l = MetroTheme::light(Accent::default());
        assert!(d.colors.background.relative_luminance() < 0.2);
        assert!(l.colors.background.relative_luminance() > 0.8);
        assert_ne!(d.overlay_color, l.overlay_color, "遮罩方向必须随方案翻转");
    }
}
