/// 2D 几何原语。尺寸为逻辑像素（display.rs 逻辑/物理分离的同一原则）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub const ORIGIN: Point = Point::new(0.0, 0.0);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub const ZERO: Size = Size::new(0.0, 0.0);
}

/// 圆角规格 —— Metro 形态铁律（参 PLAN.md §4-5）。
///
/// 类型级不变量：只允许三种合法值，**禁止 Fluent 式大圆角（4px/8px）混入**。
/// `Capsule` 仅限全圆角形态（Switch 轨道/Knob、ProgressBar 指示条）——它们是
/// CONTROL_SPEC §3/§4 里「胶囊」形状的一部分，不是通用圆角。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerRadius {
    /// 直角（0px）。
    Square,
    /// 极轻微圆角（2px 防锯齿，对应 ControlCornerRadius）。
    #[default]
    Slight,
    /// 全胶囊：短边一半（仅全圆角形态，如 Switch 轨道 40×20 → 10）。
    Capsule,
}

impl CornerRadius {
    /// 解析为实际像素值。`Capsule` 以矩形短边一半计算。
    pub const fn px(self, size: Size) -> f32 {
        match self {
            CornerRadius::Square => 0.0,
            CornerRadius::Slight => 2.0,
            CornerRadius::Capsule => {
                if size.width < size.height {
                    size.width / 2.0
                } else {
                    size.height / 2.0
                }
            }
        }
    }
}

/// 轴对齐矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub origin: Point,
    pub size: Size,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: Point::new(x, y),
            size: Size::new(width, height),
        }
    }

    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    /// 半开区间包含判定：左/上闭，右/下开。
    pub fn contains(self, p: Point) -> bool {
        p.x >= self.origin.x && p.x < self.right() && p.y >= self.origin.y && p.y < self.bottom()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_bounds() {
        let r = Rect::new(0.0, 0.0, 10.0, 20.0);
        assert_eq!(r.right(), 10.0);
        assert_eq!(r.bottom(), 20.0);
    }

    #[test]
    fn rect_half_open_contains() {
        let r = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(r.contains(Point::new(0.0, 0.0)));
        assert!(r.contains(Point::new(9.999, 9.999)));
        assert!(!r.contains(Point::new(10.0, 0.0)));
        assert!(!r.contains(Point::new(0.0, -1.0)));
    }
}
