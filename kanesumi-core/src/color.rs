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

    /// 0xRRGGBB / 0xRRGGBBAA 整型字面量构造。
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
}
