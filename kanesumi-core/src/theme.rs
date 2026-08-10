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
    /// 弹层遮罩色（对话框 / 下拉）。
    ///
    /// UWP Metro dark 用白 60% (#99FFFFFF) 淡淡"洗亮"背景以突出对话框，Ether
    /// 深色空间桌面选择相反方向：黑 70% 强压背景，因为 Ether 的 `surface`
    /// (#242424) 只比 `background` (#1A1A1A) 亮一档，白洗会让背景反而比对话框
    /// 更亮，视觉层次倒挂。参 CONTROL_SPEC §9 + VISUAL_ISSUES V9。
    /// 原值 0.45 太弱（在 #1E1E1E 上只暗 10%，视觉几乎无差），提升到 0.7。
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
            overlay_color: Color::BLACK.with_alpha(0.7),
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
