use crate::accent::{Accent, ColorScheme};
use crate::color::Color;

/// 交互指示 —— 悬停 / 按下 / 禁用 / 焦点四态。
/// Metro 的即时反馈以 tint 与描边表达，不做模糊 / 投影等重特效。
///
/// tint 方向必须随方案翻转：暗底叠白、亮底叠黑。旧实现把「白 10%」写死，
/// 在亮色主题下等于**在浅背景上再叠白** —— 悬停完全不可见。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroIndication {
    /// 悬停 tint（叠加在表面上）。
    pub hover_tint: Color,
    /// 按下 tint（叠加在表面上）。
    pub press_tint: Color,
    /// **次要按压**底色：大面积按压或次要控件的按压反馈（滑块拇指、TopBar 芯片）。
    ///
    /// 与 [`press_tint`](Self::press_tint)（22%）不是同一个角色 —— 与 hover 族同构：
    /// 强按压用于按钮类，弱按压用于大面积/次要控件。两者此前分别叫
    /// `MetroColors::press_tint`（10%）与 `MetroIndication::press_tint`（22%），同名不同义，
    /// 已按角色改名收敛（参 `ROADMAP.md` M1-1 分类学）。
    pub press_subtle_tint: Color,
    /// 极轻的中性底色（分组底 / 次级容器 / 弱分隔）。来源：WinUI 3
    /// `SubtleFillColorSecondary`（暗 `#0FFFFFFF` = 白 5.9%，亮 `#09000000` = 黑 3.5%）。
    pub subtle_tint: Color,
    /// 列表行悬停底。**与普通悬停不同值**：UWP 的 `HighlightListLow` 本就是按控件取值 ——
    /// AppBarButton 是白 10%（`CONTROL_SPEC` §65），ListView 行是 ≈白 30%（§215）。
    /// 暗色取 §215 的 30%；亮色暂用 WinUI 3 `ControlAltFillColorQuarternary` 亮 `#18000000`
    /// （登记于 `docs/CANON_VS_TEMPORARY.md`，待实测）。
    pub list_hover_tint: Color,
    /// **浅叠悬停底**：白 15%（暗）/ 黑 15%（亮）。来源 UWP `SubtleFillColorSecondary`
    /// （`CONTROL_SPEC` §471 标题栏返回键、§790 Back Hover、§864 TabView 关闭按钮、
    /// §936 TreeView 行 PointerOver）。
    ///
    /// 与 [`hover_tint`](Self::hover_tint)（10%）是**不同的角色**，不合并 ——
    /// 恰如 `HighlightListLow` 分控件的先例（参 `ROADMAP.md` M1-1 分类学）：
    /// 10% 用于普通控件悬停，15% 用于**自带底色的行 / 按钮**（树行、滑动项、标题栏按钮），
    /// 在有色底上再叠 10% 几乎不可辨。
    pub subtle_hover_tint: Color,
    /// **浅叠按压底**：白 25%（暗）/ 黑 25%（亮）。来源 UWP `SubtleFillColorTertiary`
    /// （`CONTROL_SPEC` §791 Back Pressed、§864 关闭按钮 Pressed）。
    pub subtle_press_tint: Color,
    /// 禁用态不透明度。UWP 惯例 0.38。
    pub disabled_opacity: f32,

    // ── 前景强度档 ──────────────────────────────────────────────────────────
    // 下面这组**不是色调**（参 `ROADMAP.md` M1-1 分类学）：叠加色仍是 `on_surface` /
    // `on_surface_variant` 一等令牌，本族只声明「同一个颜色用多强」。
    // 命名沿用 UWP `SystemControlForegroundBase*Brush` 家族 —— `CONTROL_SPEC` 多处
    // 就是按 BaseMediumHigh / BaseMedium / BaseMediumLow 描述边框与字形强度的。
    //
    // 之所以要在**令牌层**具名：散落的 `with_alpha(0.9)` 既读不出角色，也会在改配色时
    // 被漏改（同一角色在 4 个控件里各写一遍数字即已开始漂移）。
    /// BaseMediumHigh（白 90%）：悬停边框、文本光标、自绘字形。
    pub base_medium_high: f32,
    /// BaseMedium（白 60%）：弱化前景（preedit 下划线 `CONTROL_SPEC` §1154、已填充轨道）。
    pub base_medium: f32,
    /// BaseMediumLow（白 35%）：中灰实心（CheckBox UncheckedPressed，`CONTROL_SPEC` §1216）。
    pub base_medium_low: f32,
    /// 次级前景（0.8）：有色底上的次级行 / 次级字形 / 次级描边。
    ///
    /// 取值属 Kanesumi（UWP 无 0.8 档，最接近的 `BaseMediumHigh` 是 0.9），
    /// 登记于 `docs/CANON_VS_TEMPORARY.md`，待一次真机实测后定稿。
    pub secondary_opacity: f32,
    /// 非激活 / 只读前景（0.5）：只读评分星、候选窗未选中项的序号。
    ///
    /// 取值属 Kanesumi（`CONTROL_SPEC` §819 只写「低透」未给数），登记于
    /// `docs/CANON_VS_TEMPORARY.md`。
    pub inactive_opacity: f32,
    /// 聚焦态占位文本（0.7）：UWP `TextControlPlaceholderForegroundFocused` 的「略暗」观感。
    ///
    /// 规格给的是笔刷名而非数值，0.7 属 Kanesumi，登记于 `docs/CANON_VS_TEMPORARY.md`。
    pub placeholder_focused_opacity: f32,
    /// 焦点描边（由 accent 派生）。
    pub focus_stroke: Color,
}

impl MetroIndication {
    /// 按方案派生。tint 强度沿用 UWP Metro 白%（参 CONTROL_SPEC §1）：
    /// PointerOver ≈ 10%，Pressed ≈ 22%。
    pub fn for_scheme(scheme: ColorScheme, accent: Accent) -> Self {
        // 浅叠族以 `on_surface` 为叠加色 —— 它本身随方案翻转（暗底近白、亮底近黑），
        // 故「白 15% / 黑 15%」不需要写两套，也不会出现「亮底再叠白」那种隐形反馈。
        let on_surface = crate::colors::MetroColors::for_scheme(scheme, accent).on_surface;
        match scheme {
            ColorScheme::Dark => Self {
                hover_tint: Color::from_hex(0xFF_FF_FF_1A), // 白 10%
                press_tint: Color::from_hex(0xFF_FF_FF_38), // 白 22%
                press_subtle_tint: Color::from_hex(0xFF_FF_FF_1A), // 白 10%
                subtle_tint: Color::from_hex(0xFF_FF_FF_0F), // 白 5.9%
                list_hover_tint: Color::from_hex(0xFF_FF_FF_4D), // 白 30%
                subtle_hover_tint: on_surface.with_alpha(0.15),
                subtle_press_tint: on_surface.with_alpha(0.25),
                disabled_opacity: 0.38,
                base_medium_high: 0.9,
                base_medium: 0.6,
                base_medium_low: 0.35,
                secondary_opacity: 0.8,
                inactive_opacity: 0.5,
                placeholder_focused_opacity: 0.7,
                focus_stroke: accent.focus_for(ColorScheme::Dark),
            },
            ColorScheme::Light => Self {
                // 亮底叠黑。半透明黑必须 `from_rgba`（`from_hex` 对 ≤0x00FFFFFF 走 RGB 分支，
                // 会静默变成不透明黑 —— 参 V19 阈值坑）。
                hover_tint: Color::from_rgba(0x00_00_00_0D), // 黑 5%
                press_tint: Color::from_rgba(0x00_00_00_1A), // 黑 10%
                press_subtle_tint: Color::from_rgba(0x00_00_00_0D), // 黑 5%
                subtle_tint: Color::from_rgba(0x00_00_00_09), // 黑 3.5%
                list_hover_tint: Color::from_rgba(0x00_00_00_18), // 黑 9.4%（待实测）
                subtle_hover_tint: on_surface.with_alpha(0.15),
                subtle_press_tint: on_surface.with_alpha(0.25),
                // ⚠ 亮色禁用不透明度沿用暗色 0.38，**未实测**（登记于 CANON_VS_TEMPORARY）。
                disabled_opacity: 0.38,
                // 前景强度档与方案无关：叠加色（`on_surface`）已经翻转，强度再按方案变
                // 会让同一控件在两种方案下浓淡不一（回归测试固定这一点）。
                base_medium_high: 0.9,
                base_medium: 0.6,
                base_medium_low: 0.35,
                secondary_opacity: 0.8,
                inactive_opacity: 0.5,
                placeholder_focused_opacity: 0.7,
                focus_stroke: accent.focus_for(ColorScheme::Light),
            },
        }
    }

    /// 兼容别名：旧调用点（`MetroIndication::ether()`）= 暗色 + 默认 accent。
    pub fn ether() -> Self {
        Self::for_scheme(ColorScheme::Dark, Accent::default())
    }
}

impl Default for MetroIndication {
    fn default() -> Self {
        Self::ether()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_opacity_uwp_convention() {
        assert_eq!(MetroIndication::ether().disabled_opacity, 0.38);
    }

    /// 关键回归：亮色方案下 tint 必须叠黑而不是叠白，否则悬停在浅底上不可见。
    #[test]
    fn light_scheme_tints_darken_instead_of_lighten() {
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert!(dark.hover_tint.r > 0.5, "暗底悬停叠白");
        assert!(light.hover_tint.r < 0.5, "亮底悬停叠黑");
        assert_eq!(light.hover_tint.a, 13.0 / 255.0, "半透明黑不得退化成不透明（V19）");
    }

    #[test]
    fn focus_stroke_follows_accent() {
        let orange = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let teal = MetroIndication::for_scheme(ColorScheme::Dark, Accent::parse("00897B"));
        assert_ne!(orange.focus_stroke, teal.focus_stroke);
    }

    /// 浅叠族（15% / 25%）必须与悬停族同款「随方案翻转」，
    /// 但强度更大 —— 它们服务的是自带底色的行 / 按钮。
    #[test]
    fn subtle_overlay_tints_flip_with_scheme_and_exceed_hover() {
        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let i = MetroIndication::for_scheme(scheme, Accent::default());
            assert_eq!(i.subtle_hover_tint.a, 0.15, "{scheme:?} 浅叠悬停 15%");
            assert_eq!(i.subtle_press_tint.a, 0.25, "{scheme:?} 浅叠按压 25%");
            assert!(
                i.subtle_press_tint.a > i.subtle_hover_tint.a,
                "{scheme:?} 按压必须比悬停更实"
            );
            assert!(
                i.subtle_hover_tint.a > i.hover_tint.a,
                "{scheme:?} 浅叠族强度必须大于普通悬停（否则两族无区别）"
            );
        }
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert!(dark.subtle_hover_tint.r > 0.5, "暗底叠白");
        assert!(light.subtle_hover_tint.r < 0.5, "亮底叠黑");
    }

    /// 前景强度档是**不透明度乘数**，与方案无关：叠加色已经翻转，
    /// 强度再按方案变会让同一控件在两种方案下浓淡不一。
    #[test]
    fn foreground_opacity_tiers_are_scheme_independent() {
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert_eq!(dark.disabled_opacity, light.disabled_opacity);
        assert_eq!(dark.base_medium_high, light.base_medium_high);
        assert_eq!(dark.base_medium, light.base_medium);
        assert_eq!(dark.base_medium_low, light.base_medium_low);
        assert_eq!(dark.secondary_opacity, light.secondary_opacity);
        assert_eq!(dark.inactive_opacity, light.inactive_opacity);
        assert_eq!(
            dark.placeholder_focused_opacity,
            light.placeholder_focused_opacity
        );
        // 档位序：High > Secondary > Focused-placeholder > Medium > Inactive > Low。
        let i = dark;
        assert!(i.base_medium_high > i.secondary_opacity);
        assert!(i.secondary_opacity > i.placeholder_focused_opacity);
        assert!(i.placeholder_focused_opacity > i.base_medium);
        assert!(i.base_medium > i.inactive_opacity);
        assert!(i.inactive_opacity > i.base_medium_low);
    }
}
