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

    /// 列表选中行底：强调色 60%。来源 `CONTROL_SPEC` §215 的 Kanesumi 修正
    /// （UWP 为 75%，Ether 深色桌面调低一档至 0.60）。
    pub selection_tint: Color,
    /// 文本选区高亮：强调色 35%。来源 `CONTROL_SPEC` §34（`TextControlSelectionHighlightColor`
    /// → 强调色 **35%**，preedit 虚线下划线 60% 同节）。
    ///
    /// 与 [`selection_tint`](Self::selection_tint)（行选中 60%）**分角色**：选区是叠在正文
    /// 字形之下的临时高亮，行选中是整行底色 —— 前者更淡才不压字形。
    pub text_selection_tint: Color,
    /// 强调色低透底（24%）：ComboBox 触发器聚焦衬底 / 下拉项选中
    /// （`CONTROL_SPEC` §237 `HighlightListAccentLow`、§247 `ListAccentLow`）。
    ///
    /// 规格只写「强调色低透」未给数值，0.24 属 Kanesumi 取值（登记于
    /// `docs/CANON_VS_TEMPORARY.md`，待实测定稿）。
    pub accent_low_tint: Color,
    /// 弱化轨道底：`surface_variant` 的 60% 强度（ProgressBar 轨道，`CONTROL_SPEC` §4）。
    ///
    /// Slider 轨道用**不透明** `surface_variant`（§1451）；ProgressBar 更低一档，
    /// 以免轨道与指示条抢注意（指示条才是信息）。
    pub track_subtle: Color,
    /// 焦点描边（由 accent 派生，两种方案下都可辨）。
    pub focus_stroke: Color,
}

impl MetroColors {
    /// 暗色方案。
    ///
    /// ⚠ 取值口径：`background = #1A1A1A` 是**暗色方案内的一个选择**，不是正典级规定 ——
    /// 原「OLED 纯黑」表述已由 `KANESUMI_DESIGN.md` §Ⅲ.3 修正为「深浅两套并列方案、
    /// 取值属实现决策」，故该项在 `docs/CANON_VS_TEMPORARY.md` 的登记（T1）已解除；
    /// 若日后仍想改纯黑，那属视觉调优，不再是「与正典冲突」。
    pub fn dark(accent: Accent) -> Self {
        let surface_variant = Color::from_hex(0x2E_2E_2E);
        Self {
            background: Color::from_hex(0x1A_1A_1A),
            surface: Color::from_hex(0x24_24_24),
            surface_variant,
            divider: Color::from_hex(0x3A_3A_3A),
            primary: accent.base,
            primary_hover: accent.hover_for(ColorScheme::Dark),
            primary_pressed: accent.pressed_for(ColorScheme::Dark),
            on_primary: accent.on_accent,
            on_background: Color::from_hex(0xF0_F0_F0),
            on_surface: Color::from_hex(0xF0_F0_F0),
            on_surface_variant: Color::from_hex(0x9A_A0_A6),
            selection_tint: accent.base.with_alpha(0.60),
            text_selection_tint: accent.base.with_alpha(0.35),
            accent_low_tint: accent.base.with_alpha(0.24),
            track_subtle: surface_variant.with_alpha(0.60),
            focus_stroke: accent.focus_for(ColorScheme::Dark),
        }
    }

    /// 亮色方案。与暗色**字段一一对应** —— 缺少任一字段即编译失败，以此强制「深浅对称」。
    pub fn light(accent: Accent) -> Self {
        let surface_variant = Color::from_hex(0xF0_F0_F0);
        Self {
            background: Color::from_hex(0xFA_FA_FA),
            surface: Color::from_hex(0xFF_FF_FF),
            surface_variant,
            divider: Color::from_hex(0xD6_D6_D6),
            primary: accent.base,
            primary_hover: accent.hover_for(ColorScheme::Light),
            primary_pressed: accent.pressed_for(ColorScheme::Light),
            on_primary: accent.on_accent,
            on_background: Color::from_hex(0x1A_1A_1A),
            on_surface: Color::from_hex(0x1A_1A_1A),
            on_surface_variant: Color::from_hex(0x5A_5F_66),
            selection_tint: accent.base.with_alpha(0.60),
            text_selection_tint: accent.base.with_alpha(0.35),
            accent_low_tint: accent.base.with_alpha(0.24),
            track_subtle: surface_variant.with_alpha(0.60),
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
    fn selection_tint_is_translucent_accent() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let c = MetroColors::for_scheme(scheme, Accent::default());
            assert!(c.selection_tint.a < 1.0 && c.selection_tint.a > 0.0);
            assert_eq!(c.selection_tint.r, c.primary.r, "选中底取自强调色");
        }
    }

    /// 强调色低透族（选区 35% / 强调低透 24%）同样必须跟着 accent 走，
    /// 且**弱于**行选中底（60%）—— 顺序反了就等于选区压住字形。
    #[test]
    fn accent_low_tints_track_accent_and_stay_below_selection() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for accent in [Accent::default(), Accent::parse("00897B")] {
                let c = MetroColors::for_scheme(scheme, accent);
                assert_eq!(c.text_selection_tint.a, 0.35);
                assert_eq!(c.accent_low_tint.a, 0.24);
                assert_eq!(c.text_selection_tint.r, c.primary.r, "选区高亮取自强调色");
                assert_eq!(c.accent_low_tint.r, c.primary.r, "低透底取自强调色");
                assert!(
                    c.text_selection_tint.a < c.selection_tint.a,
                    "选区高亮必须弱于行选中底"
                );
                assert!(
                    c.accent_low_tint.a < c.text_selection_tint.a,
                    "强调低透必须弱于选区高亮"
                );
            }
        }
    }

    /// 弱轨道底 = `surface_variant` 的 60% 强度：同色系、更淡。
    #[test]
    fn track_subtle_is_a_weaker_surface_variant() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let c = MetroColors::for_scheme(scheme, Accent::default());
            assert_eq!(c.track_subtle.a, 0.60);
            assert_eq!(c.track_subtle.r, c.surface_variant.r);
            assert_eq!(c.track_subtle.g, c.surface_variant.g);
            assert_eq!(c.track_subtle.b, c.surface_variant.b);
        }
    }

    /// 强调色低透族是**叠在承载面之上的半透明色**：其上的正文必须合成后再判对比度。
    /// 三条令牌都要在「surface / surface_variant」两种承载面上都合格 ——
    /// 选区叠在表面、ComboBox 聚焦衬底叠在表面、下拉选中项叠在面板底上。
    #[test]
    fn text_on_accent_low_tints_meets_contrast() {
        use crate::color::over;

        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for accent in [
                Accent::default(),
                Accent::parse("00897B"),
                Accent::parse("0078D7"),
            ] {
                let c = MetroColors::for_scheme(scheme, accent);
                for (name, tint) in [
                    ("text_selection_tint", c.text_selection_tint),
                    ("accent_low_tint", c.accent_low_tint),
                ] {
                    for (carrier, surface) in [
                        ("surface", c.surface),
                        ("surface_variant", c.surface_variant),
                    ] {
                        let effective = over(tint, surface);
                        let ratio = c.on_surface.contrast_ratio(effective);
                        assert!(
                            ratio >= 4.5,
                            "{scheme:?}/{name} 叠在 {carrier} 上时正文对比度仅 {ratio}"
                        );
                    }
                }
            }
        }
    }

    /// 指示条必须能从轨道上认出（非文本元素阈值 3.0，WCAG 1.4.11）。
    ///
    /// ⚠ 亮色方案当前只有 2.7~2.8 —— 亮色轨道取自 `surface_variant`（本身就是浅灰）再 60% 叠白，
    /// 与强调色橙的对比度天然不足。这是**登记在案的缺口**（`CANON_VS_TEMPORARY.md` T15），
    /// 故此处钉的是「不得比现状更差」的下界：真值调好后应把下界提到 3.0 并删除本注释。
    #[test]
    fn progress_indicator_stands_out_from_track() {
        use crate::accent::Accent;
        use crate::color::over;

        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let c = MetroColors::for_scheme(scheme, Accent::default());
            let track = over(c.track_subtle, c.surface);
            let floor = if scheme.is_dark() { 3.0 } else { 2.5 };
            for (name, indicator) in [
                ("primary", c.primary),
                (
                    "error_fill",
                    crate::status::StatusColors::for_scheme(scheme).error_fill,
                ),
            ] {
                let ratio = indicator.contrast_ratio(track);
                assert!(
                    ratio >= floor,
                    "{scheme:?}/{name} 与轨道对比度仅 {ratio}，低于下界 {floor}"
                );
            }
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
        assert_ne!(orange.selection_tint, teal.selection_tint);
        assert_ne!(orange.text_selection_tint, teal.text_selection_tint);
        assert_ne!(orange.accent_low_tint, teal.accent_low_tint);
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
