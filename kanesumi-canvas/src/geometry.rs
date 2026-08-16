// geometry.rs — 形状三角化（逻辑坐标，纯几何，无副作用）。参 TOPBAR_RENDER_REFACTOR §4.3。
//
// wgpu 后端（kanesumi-harness render.rs）与 CPU 光栅化器（harness cpu_raster.rs）共用
// 同一三角化源，杜绝「两套几何越写越分叉」。函数只吃逻辑坐标、产三角形点集。
// 圆角矩形弧段数沿用 GPU 路径的 12 段/角（MSAA 4× 下的视觉经验值）。

use kanesumi_core::{Point, Rect};

/// 三角形（逻辑坐标三顶点）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub p0: Point,
    pub p1: Point,
    pub p2: Point,
}

impl Triangle {
    pub fn new(p0: Point, p1: Point, p2: Point) -> Self {
        Self { p0, p1, p2 }
    }

    /// 三角形面积（叉积绝对值的二分之一；逻辑坐标）。
    pub fn area(&self) -> f32 {
        let (a, b, c) = (self.p0, self.p1, self.p2);
        ((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)).abs() / 2.0
    }
}

/// 圆角矩形边界点（逆时针，含所有角弧）。`segs` 每角弧段数。
///
/// **总是**输出 `segs × 4` 个点（即使 r=0），以便描边路径的外 / 内多边形点数一致
/// （V10 修复：过去 r ≤ 0.5 时每角只 1 点，与 r=2 的 6 点/角不匹配 → 大三角形
/// 跨越整个矩形，把 stroke 变成 fill）。参 harness render.rs 历史注释。
pub fn rounded_rect_polygon(rect: Rect, radius: f32, segs: usize) -> Vec<Point> {
    let r = radius.clamp(0.0, rect.size.width.min(rect.size.height) / 2.0);
    let (x0, y0) = (rect.origin.x, rect.origin.y);
    let (x1, y1) = (rect.right(), rect.bottom());
    let mut pts = Vec::with_capacity(segs * 4);
    // 四个角弧，逆时针：左上 → 右上 → 右下 → 左下
    for (cx, cy, a_start) in [
        (x0 + r, y0 + r, 180.0_f32),
        (x1 - r, y0 + r, 270.0),
        (x1 - r, y1 - r, 0.0),
        (x0 + r, y1 - r, 90.0),
    ] {
        // 即便 r ≤ 0.5 也发 segs 点（都聚在 (cx,cy)），保持点数一致。
        if r <= 0.5 {
            for _ in 0..segs {
                pts.push(Point::new(cx, cy));
            }
            continue;
        }
        for i in 0..segs {
            let a = (a_start + 90.0 * i as f32 / segs as f32).to_radians();
            pts.push(Point::new(cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

/// 圆角矩形弧段数（GPU 路径历史取值；两后端共用保持像素一致）。
pub const ROUNDED_SEGS: usize = 12;

/// 填充矩形 → 三角形列表。
/// `radius ≤ 0.5`：直矩形两个三角形；否则圆角矩形多边形重心三角扇。
pub fn triangulate_fill(rect: Rect, radius: f32) -> Vec<Triangle> {
    let (x0, y0, x1, y1) = (rect.origin.x, rect.origin.y, rect.right(), rect.bottom());
    if radius <= 0.5 {
        return vec![
            Triangle::new(
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
            ),
            Triangle::new(
                Point::new(x0, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ),
        ];
    }
    let pts = rounded_rect_polygon(rect, radius, ROUNDED_SEGS);
    let center = Point::new(
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    let mut tris = Vec::with_capacity(pts.len());
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        tris.push(Triangle::new(center, pts[i], pts[j]));
    }
    tris
}

/// 描边矩形 → 三角形列表（外 / 内多边形连接成的环，或四条矩形边）。
pub fn triangulate_stroke(rect: Rect, radius: f32, thickness: f32) -> Vec<Triangle> {
    if thickness <= 0.0 {
        return Vec::new();
    }
    if radius <= 0.5 {
        // 四条矩形边。
        let (x0, y0) = (rect.origin.x, rect.origin.y);
        let (x1, y1) = (rect.right(), rect.bottom());
        let t = thickness;
        let inner_h = (y1 - y0).max(t) - 2.0 * t;
        let mut tris = Vec::with_capacity(8);
        for r in [
            Rect::new(x0, y0, x1 - x0, t),
            Rect::new(x0, y1 - t, x1 - x0, t),
            Rect::new(x0, y0 + t, t, inner_h.max(0.0)),
            Rect::new(x1 - t, y0 + t, t, inner_h.max(0.0)),
        ] {
            tris.extend(triangulate_fill(r, 0.0));
        }
        return tris;
    }
    let outer = rounded_rect_polygon(rect, radius, ROUNDED_SEGS);
    let inner_r = (radius - thickness).max(0.0);
    let inner_rect = Rect::new(
        rect.origin.x + thickness,
        rect.origin.y + thickness,
        rect.size.width - 2.0 * thickness,
        rect.size.height - 2.0 * thickness,
    );
    if inner_rect.size.width <= 0.0 || inner_rect.size.height <= 0.0 {
        // 内矩形退化 → 退化为填充整个圆角矩形。
        return triangulate_fill(rect, radius);
    }
    let inner = rounded_rect_polygon(inner_rect, inner_r, ROUNDED_SEGS);
    let n = outer.len().min(inner.len());
    let mut tris = Vec::with_capacity(n * 2);
    for i in 0..n {
        let j = (i + 1) % n;
        let (ox, oy) = (outer[i].x, outer[i].y);
        let (ox2, oy2) = (outer[j].x, outer[j].y);
        let (ix, iy) = (inner[i].x, inner[i].y);
        let (ix2, iy2) = (inner[j].x, inner[j].y);
        tris.push(Triangle::new(
            Point::new(ox, oy),
            Point::new(ix, iy),
            Point::new(ox2, oy2),
        ));
        tris.push(Triangle::new(
            Point::new(ix, iy),
            Point::new(ix2, iy2),
            Point::new(ox2, oy2),
        ));
    }
    tris
}

/// 弧线（ProgressRing）→ 内外圆环扇形三角带。
/// 0° = 正上，顺时针（screen y 向下）。|sweep| ≤ 0.5 或厚度 ≤ 0 → 空列表。
pub fn triangulate_arc(
    center: Point,
    radius: f32,
    thickness: f32,
    start_deg: f32,
    end_deg: f32,
) -> Vec<Triangle> {
    let sweep = end_deg - start_deg;
    if sweep.abs() <= 0.5 || thickness <= 0.0 {
        return Vec::new();
    }
    let r_out = radius + thickness / 2.0;
    let r_in = (radius - thickness / 2.0).max(0.0);
    let n = ((sweep.abs() / 8.0).ceil() as usize).clamp(1, 256);
    let mut tris = Vec::with_capacity(n * 2);
    let mut prev_o: Option<Point> = None;
    let mut prev_i: Option<Point> = None;
    for k in 0..=n {
        let a = (start_deg + sweep * k as f32 / n as f32).to_radians();
        // 0° 向上、顺时针：x = r*sin, y = -r*cos
        let (s, cs) = a.sin_cos();
        let (ox, oy) = (center.x + r_out * s, center.y - r_out * cs);
        let (ix, iy) = (center.x + r_in * s, center.y - r_in * cs);
        if let (Some(po), Some(pi)) = (prev_o, prev_i) {
            tris.push(Triangle::new(po, pi, Point::new(ox, oy)));
            tris.push(Triangle::new(pi, Point::new(ix, iy), Point::new(ox, oy)));
        }
        prev_o = Some(Point::new(ox, oy));
        prev_i = Some(Point::new(ix, iy));
    }
    tris
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn polygon_always_emits_segs_times_four_points() {
        // r=0 也输出 segs×4 个点（聚在四角），保证内外多边形点数一致。
        let pts = rounded_rect_polygon(rect(0.0, 0.0, 100.0, 50.0), 0.0, 12);
        assert_eq!(pts.len(), 48);
        // 四点位置 = 四角（r=0 → 弧点聚于角心）。
        assert_eq!(pts[0], Point::new(0.0, 0.0));
        assert_eq!(pts[12], Point::new(100.0, 0.0));
        assert_eq!(pts[24], Point::new(100.0, 50.0));
        assert_eq!(pts[36], Point::new(0.0, 50.0));
    }

    #[test]
    fn polygon_radius_clamped_to_half_size() {
        let pts = rounded_rect_polygon(rect(0.0, 0.0, 100.0, 40.0), 999.0, 4);
        assert_eq!(pts.len(), 16);
        // 圆角半径夹到 min(w,h)/2 = 20。
        for p in &pts {
            assert!(p.x >= 0.0 && p.x <= 100.0 && p.y >= 0.0 && p.y <= 40.0);
        }
    }

    #[test]
    fn fill_rect_is_two_triangles_covering_area() {
        let tris = triangulate_fill(rect(10.0, 20.0, 30.0, 40.0), 0.0);
        assert_eq!(tris.len(), 2);
        let area: f32 = tris.iter().map(|t| t.area()).sum();
        assert!((area - 1200.0).abs() < 0.01);
    }

    #[test]
    fn fill_rounded_is_center_fan() {
        let tris = triangulate_fill(rect(0.0, 0.0, 100.0, 100.0), 10.0);
        assert_eq!(tris.len(), 48); // 12 segs × 4 角
        for t in &tris {
            assert_eq!(t.p0, Point::new(50.0, 50.0));
        }
    }

    #[test]
    fn stroke_rect_ring_area_equals_outer_minus_inner() {
        // 直矩形描边 = 4 条边；面积 = w*h - (w-2t)(h-2t)。
        let tris = triangulate_stroke(rect(0.0, 0.0, 100.0, 60.0), 0.0, 4.0);
        let area: f32 = tris.iter().map(|t| t.area()).sum();
        assert!((area - (6000.0 - 92.0 * 52.0)).abs() < 0.1);
    }

    #[test]
    fn stroke_rounded_ring_approximates_ring_area() {
        // 圆角矩形环面积 ≈ 外圆角矩形 - 内圆角矩形（圆角 r=10, t=4 → 内 r=6）。
        let outer: f32 = triangulate_fill(rect(0.0, 0.0, 100.0, 60.0), 10.0)
            .iter()
            .map(|t| t.area())
            .sum();
        let inner: f32 = triangulate_fill(rect(4.0, 4.0, 92.0, 52.0), 6.0)
            .iter()
            .map(|t| t.area())
            .sum();
        let ring: f32 = triangulate_stroke(rect(0.0, 0.0, 100.0, 60.0), 10.0, 4.0)
            .iter()
            .map(|t| t.area())
            .sum();
        assert!((ring - (outer - inner)).abs() < 1.0);
    }

    #[test]
    fn stroke_degenerate_inner_falls_back_to_fill() {
        // thickness 吃掉内矩形 → 退化为填充（面积 = 填充面积）。
        let fill: f32 = triangulate_fill(rect(0.0, 0.0, 20.0, 20.0), 4.0)
            .iter()
            .map(|t| t.area())
            .sum();
        let ring: f32 = triangulate_stroke(rect(0.0, 0.0, 20.0, 20.0), 4.0, 15.0)
            .iter()
            .map(|t| t.area())
            .sum();
        assert!((ring - fill).abs() < 0.5);
    }

    #[test]
    fn arc_quarter_ring_covers_quarter_annulus() {
        let tris = triangulate_arc(Point::new(50.0, 50.0), 40.0, 10.0, 0.0, 90.0);
        let area: f32 = tris.iter().map(|t| t.area()).sum();
        // 四分之一圆环面积 = π/4 (r_out² - r_in²)。
        let expect = std::f32::consts::FRAC_PI_4 * (45.0 * 45.0 - 35.0 * 35.0);
        assert!((area - expect).abs() < expect * 0.01);
    }

    #[test]
    fn arc_too_small_sweep_is_empty() {
        assert!(triangulate_arc(Point::new(0.0, 0.0), 10.0, 4.0, 0.0, 0.4).is_empty());
        assert!(triangulate_arc(Point::new(0.0, 0.0), 10.0, 0.0, 0.0, 90.0).is_empty());
    }
}
