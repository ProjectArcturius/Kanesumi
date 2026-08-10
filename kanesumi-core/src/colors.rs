use crate::color::Color;

/// Kanesumi 设计 tokens —— 颜色层。参 PLAN.md §4-5（无渐变纯色）。
/// 默认值派生自 Ether 合成器色板（`compositor/src/config.rs`）——深色空间桌面。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroColors {
    /// 应用背景。比桌面底色（#1E1E1E）略深，与桌面拉开层次。
    pub background: Color,
    /// 面板 / 控件表面。对齐合成器 SSD 标题栏色（0.14）。
    pub surface: Color,
    /// 悬停 / 次级面板表面。
    pub surface_variant: Color,
    /// 分隔线。
    pub divider: Color,
    /// 强调色 —— Ether 橙金（对齐 Dock 运行指示线）。
    pub primary: Color,
    /// 强调色上的文字。
    pub on_primary: Color,
    /// 背景上的正文。
    pub on_background: Color,
    /// 表面上的正文。
    pub on_surface: Color,
    /// 次级正文 / 图标。
    pub on_surface_variant: Color,
    /// 按下 tint（叠加在表面上表达按压）。
    pub press_tint: Color,
    /// 焦点描边（对齐 Dock 聚焦指示线 #FFA626）。
    pub focus_stroke: Color,
}

impl MetroColors {
    /// Ether 深色空间桌面默认色板。
    pub const fn ether_dark() -> Self {
        Self {
            background: Color::from_hex(0x1A_1A_1A),
            surface: Color::from_hex(0x24_24_24),
            surface_variant: Color::from_hex(0x2E_2E_2E),
            divider: Color::from_hex(0x3A_3A_3A),
            primary: Color::from_hex(0xE5_78_12),
            on_primary: Color::from_hex(0xFF_FF_FF),
            on_background: Color::from_hex(0xF0_F0_F0),
            on_surface: Color::from_hex(0xF0_F0_F0),
            on_surface_variant: Color::from_hex(0x9A_A0_A6),
            press_tint: Color::from_hex(0xFF_FF_FF_1A), // rgba(255,255,255,0.10)
            focus_stroke: Color::from_hex(0xFF_A6_26),
        }
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
    fn ether_dark_has_opaque_background() {
        let c = MetroColors::ether_dark();
        assert_eq!(c.background.a, 1.0);
        assert_eq!(c.surface.a, 1.0);
    }

    #[test]
    fn press_tint_is_translucent() {
        let c = MetroColors::ether_dark();
        assert!(c.press_tint.a < 1.0 && c.press_tint.a > 0.0);
    }
}
