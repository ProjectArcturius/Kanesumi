use crate::accent::{Accent, ColorScheme};
use crate::color::Color;

/// Kanesumi 颜色令牌 —— 语义层。参 `KANESUMI_DESIGN.md` §Ⅲ.3（深底 / 浅底 + 单一强调色）。
///
/// **一律由 `(scheme, accent)` 派生，不存在写死的强调色**：
/// - `scheme`（dark/light）决定基底与前景方向；
/// - `accent` 决定唯一的强调色及其悬停 / 按下 / 焦点派生档。
///
/// 控件只消费本结构体的字段，**不得内联颜色字面量**（参 `docs/REFERENCE.md` §Ⅲ 的
/// `XamlResourceReferenceFailed` 教训：写错的颜色必须报错，不能静默回退成另一个样子）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroColors {
    /// 应用背景（基底）。
    pub background: Color,
    /// 面板 / 控件表面。
    pub surface: Color,
    /// 悬停 / 次级面板表面。
    pub surface_variant: Color,
    /// 分隔线。
    pub divider: Color,

    /// 强调色（主）。由 accent 基色派生。
    pub primary: Color,
    /// 强调色悬停档。
    pub primary_hover: Color,
    /// 强调色按下档。
    pub primary_pressed: Color,
    /// 强调色之上的前景（按对比度自动取黑 / 白）。
    pub on_primary: Color,

    /// 背景上的正文。
    pub on_background: Color,
    /// 表面上的正文。
    pub on_surface: Color,
    /// 次级正文 / 图标。
    pub on_surface_variant: Color,

    /// 按下 tint（叠加在表面上表达按压）。
    pub press_tint: Color,
    /// 列表选中行底：强调色 60%。来源 `CONTROL_SPEC` §215 的 Kanesumi 修正
    /// （UWP 为 75%，Ether 深色桌面调低一档至 0.60）。
    pub selection_tint: Color,
    /// 焦点描边（由 accent 派生，两种方案下都可辨）。
    pub focus_stroke: Color,
}

impl MetroColors {
    /// 暗色方案。
    ///
    /// ⚠ **登记在案的临时值**：`background = #1A1A1A` 是「浅色主题尚未完成」时期的取值，
    /// 正典 `KANESUMI_DESIGN.md` §Ⅲ.3 要求 OLED 纯黑 `#000000`。两者的差异见
    /// `docs/CANON_VS_TEMPORARY.md`；在维护者裁定前**不得**把它当成正典。
    pub fn dark(accent: Accent) -> Self {
        Self {
            background: Color::from_hex(0x1A_1A_1A),
            surface: Color::from_hex(0x24_24_24),
            surface_variant: Color::from_hex(0x2E_2E_2E),
            divider: Color::from_hex(0x3A_3A_3A),
            primary: accent.base,
            primary_hover: accent.hover_for(ColorScheme::Dark),
            primary_pressed: accent.pressed_for(ColorScheme::Dark),
            on_primary: accent.on_accent,
            on_background: Color::from_hex(0xF0_F0_F0),
            on_surface: Color::from_hex(0xF0_F0_F0),
            on_surface_variant: Color::from_hex(0x9A_A0_A6),
            press_tint: Color::from_hex(0xFF_FF_FF_1A), // 白 10%
            selection_tint: accent.base.with_alpha(0.60),
            focus_stroke: accent.focus_for(ColorScheme::Dark),
        }
    }

    /// 亮色方案。与暗色**字段一一对应** —— 缺少任一字段即编译失败，以此强制「深浅对称」。
    pub fn light(accent: Accent) -> Self {
        Self {
            background: Color::from_hex(0xFA_FA_FA),
            surface: Color::from_hex(0xFF_FF_FF),
            surface_variant: Color::from_hex(0xF0_F0_F0),
            divider: Color::from_hex(0xD6_D6_D6),
            primary: accent.base,
            primary_hover: accent.hover_for(ColorScheme::Light),
            primary_pressed: accent.pressed_for(ColorScheme::Light),
            on_primary: accent.on_accent,
            on_background: Color::from_hex(0x1A_1A_1A),
            on_surface: Color::from_hex(0x1A_1A_1A),
            on_surface_variant: Color::from_hex(0x5A_5F_66),
            press_tint: Color::from_rgba(0x00_00_00_1A), // 黑 10%（半透明黑必须 from_rgba，参 V19 阈值坑）
            selection_tint: accent.base.with_alpha(0.60),
            focus_stroke: accent.focus_for(ColorScheme::Light),
        }
    }

    /// 按方案分派。
    pub fn for_scheme(scheme: ColorScheme, accent: Accent) -> Self {
        match scheme {
            ColorScheme::Dark => Self::dark(accent),
            ColorScheme::Light => Self::light(accent),
        }
    }

    /// 兼容别名：旧调用点（`MetroColors::ether_dark()`）= 暗色 + 默认 accent。
    pub fn ether_dark() -> Self {
        Self::dark(Accent::default())
    }
}

impl Default for MetroColors {
    fn default() -> Self {
        Self::ether_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_schemes_have_opaque_background() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let c = MetroColors::for_scheme(scheme, Accent::default());
            assert_eq!(c.background.a, 1.0, "{scheme:?} 背景必须不透明");
            assert_eq!(c.surface.a, 1.0);
        }
    }

    #[test]
    fn press_tint_is_translucent() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let c = MetroColors::for_scheme(scheme, Accent::default());
            assert!(c.press_tint.a < 1.0 && c.press_tint.a > 0.0);
        }
    }

    /// 关键回归：换 accent 必须改变全部强调相关令牌 ——
    /// 这是「Chorus 改 accent、应用仍旧橙色」那个 bug 的守卫。
    #[test]
    fn accent_change_propagates_to_all_accent_tokens() {
        let orange = MetroColors::dark(Accent::default());
        let teal = MetroColors::dark(Accent::parse("00897B"));
        assert_ne!(orange.primary, teal.primary);
        assert_ne!(orange.primary_hover, teal.primary_hover);
        assert_ne!(orange.primary_pressed, teal.primary_pressed);
        assert_ne!(orange.focus_stroke, teal.focus_stroke);
    }

    /// 深浅对称：前景与背景的明暗方向必须互换，否则浅色主题必然「白字白底」。
    #[test]
    fn schemes_flip_foreground_direction() {
        let dark = MetroColors::dark(Accent::default());
        let light = MetroColors::light(Accent::default());
        assert!(dark.on_background.relative_luminance() > dark.background.relative_luminance());
        assert!(light.on_background.relative_luminance() < light.background.relative_luminance());
    }

    /// 令牌对比度自检（对标 UWP `DebugSettings` 家族的可自检思路）：
    /// 正文 / 次级文字在其承载面上必须达 WCAG 阈值，任何方案、任何 accent 都不例外。
    #[test]
    fn text_tokens_meet_wcag_contrast() {
        for (name, accent) in [
            ("orange", Accent::default()),
            ("teal", Accent::parse("00897B")),
            ("blue", Accent::parse("0078D7")),
        ] {
            for scheme in [ColorScheme::Dark, ColorScheme::Light] {
                let c = MetroColors::for_scheme(scheme, accent);
                let ctx = format!("{name}/{scheme:?}");
                assert!(
                    c.on_background.contrast_ratio(c.background) >= 4.5,
                    "{ctx} on_background 对比度不足：{}",
                    c.on_background.contrast_ratio(c.background)
                );
                assert!(
                    c.on_surface.contrast_ratio(c.surface) >= 4.5,
                    "{ctx} on_surface 对比度不足：{}",
                    c.on_surface.contrast_ratio(c.surface)
                );
                assert!(
                    c.on_surface_variant.contrast_ratio(c.surface) >= 3.0,
                    "{ctx} on_surface_variant 对比度不足：{}",
                    c.on_surface_variant.contrast_ratio(c.surface)
                );
                assert!(
                    c.on_primary.contrast_ratio(c.primary) >= 4.5,
                    "{ctx} on_primary 对比度不足：{}",
                    c.on_primary.contrast_ratio(c.primary)
                );
            }
        }
    }
}
