/// SRGBA 颜色类型。参 PLAN.md §4-5 与 SD §II —— 纯色、无渐变。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// 0xRRGGBB / 0xRRGGBBAA 启发式构造（≤ `0x00FFFFFF` 视为 RGB，其它视为 RGBA）。
    ///
    /// **⚠ 阈值坑（V19）**：`0x0100_0000` 会被当 RGBA（几乎全透明黑），
    /// 无法表达"RGB=(1,0,0)"这种低值 RGB。要显式指定 alpha 时用 [`Color::from_rgba`]。
    pub const fn from_hex(hex: u32) -> Self {
        let (r, g, b, a) = match hex {
            0..=0x00FF_FFFF => (
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
                1.0,
            ),
            _ => (
                ((hex >> 24) & 0xFF) as f32 / 255.0,
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
            ),
        };
        Self { r, g, b, a }
    }

    /// 0xRRGGBBAA 显式构造（V19）—— 无 alpha 推断，全部 32 位按 RGBA 拆。
    /// 用它避免 [`from_hex`] 的阈值歧义（如 `0x0100_0000` 想表达"极暗红全不透明"时）。
    pub const fn from_rgba(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xFF) as f32 / 255.0,
            g: ((hex >> 16) & 0xFF) as f32 / 255.0,
            b: ((hex >> 8) & 0xFF) as f32 / 255.0,
            a: (hex & 0xFF) as f32 / 255.0,
        }
    }

    pub const fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    /// 线性插值。t ∈ [0, 1]，越界自动夹紧。
    pub fn lerp(self, other: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0) as f32;
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_rgb() {
        let c = Color::from_hex(0xE5_78_12);
        assert_eq!(c.r, 0xE5 as f32 / 255.0);
        assert_eq!(c.g, 0x78 as f32 / 255.0);
        assert_eq!(c.b, 0x12 as f32 / 255.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn hex_rgba() {
        let c = Color::from_hex(0xFF_FF_FF_1A);
        assert_eq!(c.a, 0x1A as f32 / 255.0);
    }

    #[test]
    fn lerp_clamps() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        assert_eq!(a.lerp(b, 0.5).r, 0.5);
        assert_eq!(a.lerp(b, -1.0), a);
        assert_eq!(a.lerp(b, 2.0), b);
    }

    #[test]
    fn from_rgba_bypasses_hex_heuristic() {
        // V19：from_rgba 显式，不受 0x00FFFFFF 阈值影响
        let c = Color::from_rgba(0x0100_0000);
        // 期望：r = 1/255（极暗红），a = 0（透明）—— 明确 RGBA 解释
        assert!((c.r - 1.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 0.0);
        // from_hex 会走 RGBA 分支得到同样结果（此值恰好越阈值），但语义不明；
        // 而 0x00FF_FFFF 在 from_hex 会被 RGB 化：
        let opaque_white = Color::from_hex(0x00FF_FFFF);
        assert_eq!(opaque_white.a, 1.0);
        // from_rgba 同值则 alpha=0xFF 全不透明白
        let rgba_white = Color::from_rgba(0xFFFF_FFFF);
        assert_eq!(rgba_white.a, 1.0);
        assert_eq!(rgba_white.r, 1.0);
    }
}
