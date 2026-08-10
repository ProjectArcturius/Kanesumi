use crate::color::Color;

/// 交互指示 —— 悬停 / 按下 / 禁用 / 焦点四态。
/// Metro 的即时反馈以 tint 与描边表达，不做模糊/投影等重特效。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroIndication {
    /// 悬停 tint（叠加在表面上）。
    pub hover_tint: Color,
    /// 按下 tint（叠加在表面上）。
    pub press_tint: Color,
    /// 禁用态不透明度。UWP 惯例 0.38。
    pub disabled_opacity: f32,
    /// 焦点描边。
    pub focus_stroke: Color,
}

impl MetroIndication {
    pub const fn ether() -> Self {
        Self {
            hover_tint: Color::from_hex(0xFF_FF_FF_0A), // rgba(255,255,255,0.04)
            press_tint: Color::from_hex(0xFF_FF_FF_1A), // rgba(255,255,255,0.10)
            disabled_opacity: 0.38,
            focus_stroke: Color::from_hex(0xFF_A6_26),
        }
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
}
