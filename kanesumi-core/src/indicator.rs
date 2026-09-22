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
    /// 禁用态不透明度。UWP 惯例 0.38。
    pub disabled_opacity: f32,
    /// 焦点描边（由 accent 派生）。
    pub focus_stroke: Color,
}

impl MetroIndication {
    /// 按方案派生。tint 强度沿用 UWP Metro 白%（参 CONTROL_SPEC §1）：
    /// PointerOver ≈ 10%，Pressed ≈ 22%。
    pub fn for_scheme(scheme: ColorScheme, accent: Accent) -> Self {
        match scheme {
            ColorScheme::Dark => Self {
                hover_tint: Color::from_hex(0xFF_FF_FF_1A), // 白 10%
                press_tint: Color::from_hex(0xFF_FF_FF_38), // 白 22%
                press_subtle_tint: Color::from_hex(0xFF_FF_FF_1A), // 白 10%
                subtle_tint: Color::from_hex(0xFF_FF_FF_0F), // 白 5.9%
                list_hover_tint: Color::from_hex(0xFF_FF_FF_4D), // 白 30%
                disabled_opacity: 0.38,
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
                // ⚠ 亮色禁用不透明度沿用暗色 0.38，**未实测**（登记于 CANON_VS_TEMPORARY）。
                disabled_opacity: 0.38,
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
}
