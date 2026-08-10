use crate::color::Color;
use crate::colors::MetroColors;
use crate::indicator::MetroIndication;
use crate::tokens::Tokens;
use crate::typography::MetroTypography;

/// 主题容器 —— 单一渲染权威。参 SD §III。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroTheme {
    pub colors: MetroColors,
    pub typography: MetroTypography,
    pub tokens: Tokens,
    pub indication: MetroIndication,
    /// 弹层遮罩色（对话框 / 下拉）。UWP 经典为白 60%（#99FFFFFF），
    /// Ether 深色空间桌面改黑 45% 半透明（参 CONTROL_SPEC §9 + LP_DIM 惯例）。
    pub overlay_color: Color,
}

impl MetroTheme {
    /// Ether 深色空间桌面主题（默认）。
    pub const fn ether_dark() -> Self {
        Self {
            colors: MetroColors::ether_dark(),
            typography: MetroTypography::metro(),
            tokens: Tokens::ether(),
            indication: MetroIndication::ether(),
            overlay_color: Color::BLACK.with_alpha(0.45),
        }
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
        assert_eq!(t.colors.primary, MetroColors::ether_dark().primary);
    }
}
