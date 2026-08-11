use crate::geometry::CornerRadius;

/// 间距网格：4px 基数。参 UWP 布局惯例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spacing {
    pub s4: f32,
    pub s8: f32,
    pub s12: f32,
    pub s16: f32,
    pub s24: f32,
    pub s32: f32,
}

impl Spacing {
    pub const fn base4() -> Self {
        Self {
            s4: 4.0,
            s8: 8.0,
            s12: 12.0,
            s16: 16.0,
            s24: 24.0,
            s32: 32.0,
        }
    }
}

impl Default for Spacing {
    fn default() -> Self {
        Self::base4()
    }
}

/// 全局设计 tokens。参 PLAN.md §4-5 —— Metro 形态：直角优先、无渐变纯色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// 圆角：**直角为默认**（`Square`，4× MSAA 保证边缘质量，参 harness render.rs）。
    /// `Slight` 仅作个案 opt-in；`Capsule` 仅限结构性胶囊（Switch 轨道/Knob、进度条指示条）。
    pub corner_radius: CornerRadius,
    pub spacing: Spacing,
    /// 字体族。Ether 唯一字体，禁止静默回退（SD §IX）。
    pub font_family: &'static str,
}

impl Tokens {
    pub const fn ether() -> Self {
        Self {
            corner_radius: CornerRadius::Square,
            spacing: Spacing::base4(),
            font_family: "Source Han Sans SC",
        }
    }
}

impl Default for Tokens {
    fn default() -> Self {
        Self::ether()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spacing_is_4px_grid() {
        let s = Spacing::base4();
        assert_eq!(s.s4, 4.0);
        assert_eq!(s.s16, 16.0);
        assert_eq!(s.s24, 24.0);
    }
}
