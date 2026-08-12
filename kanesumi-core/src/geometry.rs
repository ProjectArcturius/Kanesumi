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

    /// 归一化外部尺寸：NaN / 负值归零，正无穷保留为无界约束。
    pub fn normalized(self) -> Self {
        Self::new(normalize_extent(self.width), normalize_extent(self.height))
    }

    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }
}

fn normalize_extent(value: f32) -> f32 {
    if value.is_nan() || value <= 0.0 {
        0.0
    } else {
        value
    }
}

/// 圆角规格 —— 直角几何铁律（参 PLAN.md §4-5）。
///
/// 类型级不变量：只允许三种合法值，**禁止 Fluent 式大圆角（4px/8px）混入**。
/// `Capsule` 仅限全圆角形态（Switch 轨道/Knob、ProgressBar 指示条）——它们是
/// CONTROL_SPEC §3/§4 里「胶囊」形状的一部分，不是通用圆角。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CornerRadius {
    /// 直角（0px）—— **默认**。4× MSAA 保证直角边缘无锯齿（参 harness render.rs）。
    #[default]
    Square,
    /// 极轻微圆角（2px）—— 个案 opt-in，非默认（曾为"防锯齿"默认，被直角取代）。
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

    /// 归一化矩形。位置非有限时回到原点，尺寸遵循 `Size::normalized`。
    pub fn normalized(self) -> Self {
        let x = if self.origin.x.is_finite() {
            self.origin.x
        } else {
            0.0
        };
        let y = if self.origin.y.is_finite() {
            self.origin.y
        } else {
            0.0
        };
        Self::new(
            x,
            y,
            self.size.normalized().width,
            self.size.normalized().height,
        )
    }

    pub fn inset(self, left: f32, top: f32, right: f32, bottom: f32) -> Self {
        let rect = self.normalized();
        let left = left.max(0.0);
        let top = top.max(0.0);
        let right = right.max(0.0);
        let bottom = bottom.max(0.0);
        Self::new(
            rect.origin.x + left,
            rect.origin.y + top,
            (rect.size.width - left - right).max(0.0),
            (rect.size.height - top - bottom).max(0.0),
        )
    }

    pub fn is_empty(self) -> bool {
        self.size.is_empty()
    }

    /// 中心点。
    pub fn center(self) -> Point {
        Point::new(
            self.origin.x + self.size.width / 2.0,
            self.origin.y + self.size.height / 2.0,
        )
    }

    /// 半开区间包含判定：左/上闭，右/下开。
    pub fn contains(self, p: Point) -> bool {
        !self.is_empty()
            && p.x >= self.origin.x
            && p.x < self.right()
            && p.y >= self.origin.y
            && p.y < self.bottom()
    }

    /// 与 `other` 的交集（半开区间）。不相交时返回 `None`（或退化矩形）。
    /// 布局引擎裁剪（box 语义）用：`clip ∩ child_rect` 决定可见区域。
    pub fn intersect(self, other: Rect) -> Option<Rect> {
        let x0 = self.origin.x.max(other.origin.x);
        let y0 = self.origin.y.max(other.origin.y);
        let x1 = self.right().min(other.right());
        let y1 = self.bottom().min(other.bottom());
        if x0 < x1 && y0 < y1 {
            Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
        } else {
            None
        }
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

    #[test]
    fn invalid_geometry_normalizes_without_negative_extent() {
        let size = Size::new(f32::NAN, -4.0).normalized();
        assert_eq!(size, Size::ZERO);
        let rect = Rect::new(f32::INFINITY, f32::NAN, -20.0, 10.0).normalized();
        assert_eq!(rect, Rect::new(0.0, 0.0, 0.0, 10.0));
        assert!(!rect.contains(Point::ORIGIN));
    }

    #[test]
    fn inset_clamps_at_empty() {
        assert_eq!(
            Rect::new(10.0, 20.0, 30.0, 40.0).inset(20.0, 8.0, 20.0, 8.0),
            Rect::new(30.0, 28.0, 0.0, 24.0)
        );
    }
}
