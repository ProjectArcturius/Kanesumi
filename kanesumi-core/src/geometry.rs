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
