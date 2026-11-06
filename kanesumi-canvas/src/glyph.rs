// Metro 自绘几何 glyph —— Kanesumi 不依赖 Fluent / Segoe MDL2 私有区 codepoint。
//
// 参 docs/VISUAL_ISSUES.md V7：Ether 正体是思源黑体（Source Han Sans SC），其中无
// MDL2 私有区（E7xx / E8xx）字形。Metro 时代 ComboBox / MenuFlyout 的 chevron /
// 展开指示等，Kanesumi 通过 Scene 几何原语自绘，不占任何 codepoint。
//
// 所有 glyph 在给定 `rect` 内居中，尺寸按 rect 短边比例派生（不依赖字体度量）。
// 每个 glyph 只在 Scene 里产出很少几条命令（Triangle / FillRect），零字形上传。

use kanesumi_core::{Color, Point, Rect};

use crate::scene::Scene;

/// ChevronDown —— 下向指示三角，用于 ComboBox / DropdownMenu / Selector 触发器。
///
/// 视觉：等腰三角形尖端朝下，宽 = rect.width × 0.6、高 = rect.height × 0.4，
/// 整体在 rect 内居中。对应 Segoe MDL2 `E70D`。
pub fn chevron_down(scene: &mut Scene, rect: Rect, color: Color) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    // 尺寸：宽略大于高（Metro chevron 略扁平），限制在 rect 内
    let w = (rect.size.width * 0.6).min(rect.size.width - 2.0);
    let h = (rect.size.height * 0.4).min(rect.size.height - 2.0);
    let left = Point::new(cx - w / 2.0, cy - h / 2.0);
    let right = Point::new(cx + w / 2.0, cy - h / 2.0);
    let tip = Point::new(cx, cy + h / 2.0);
    scene.triangle(left, right, tip, color);
}

/// ChevronUp —— 上向指示三角（反向 chevron_down）。
pub fn chevron_up(scene: &mut Scene, rect: Rect, color: Color) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    let w = (rect.size.width * 0.6).min(rect.size.width - 2.0);
    let h = (rect.size.height * 0.4).min(rect.size.height - 2.0);
    let left = Point::new(cx - w / 2.0, cy + h / 2.0);
    let right = Point::new(cx + w / 2.0, cy + h / 2.0);
    let tip = Point::new(cx, cy - h / 2.0);
    scene.triangle(left, right, tip, color);
}

/// ChevronRight —— 右向指示三角（PipsPager 下一页等）。
pub fn chevron_right(scene: &mut Scene, rect: Rect, color: Color) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    let w = (rect.size.width * 0.4).min(rect.size.width - 2.0);
    let h = (rect.size.height * 0.6).min(rect.size.height - 2.0);
    let top = Point::new(cx - w / 2.0, cy - h / 2.0);
    let bottom = Point::new(cx - w / 2.0, cy + h / 2.0);
    let tip = Point::new(cx + w / 2.0, cy);
    scene.triangle(top, bottom, tip, color);
}

/// ChevronLeft —— 左向指示三角（PipsPager 上一页等）。
pub fn chevron_left(scene: &mut Scene, rect: Rect, color: Color) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    let w = (rect.size.width * 0.4).min(rect.size.width - 2.0);
    let h = (rect.size.height * 0.6).min(rect.size.height - 2.0);
    let top = Point::new(cx + w / 2.0, cy - h / 2.0);
    let bottom = Point::new(cx + w / 2.0, cy + h / 2.0);
    let tip = Point::new(cx - w / 2.0, cy);
    scene.triangle(top, bottom, tip, color);
}

/// Check —— 菜单勾选标记（两段折线，短笔画在下、长笔画向上）。
///
/// 为什么自绘而不写字形：菜单勾选列要随行高缩放、要与文字基线无关地居中，
/// 而且不能依赖字体里有 `U+2713`（参本文件头注的 codepoint 政策）。
/// 每段折线 = 一个平行四边形 = 两个三角，整枚 4 条 Triangle 命令，与 chevron 同量级。
pub fn check(scene: &mut Scene, rect: Rect, color: Color) {
    // 归一化坐标（0..1）→ 实际矩形：勾的拐点在下方偏左，长笔画向右上。
    let p = |x: f32, y: f32| {
        Point::new(
            rect.origin.x + x * rect.size.width,
            rect.origin.y + y * rect.size.height,
        )
    };
    let thickness = (rect.size.height * 0.16).max(1.0);
    stroke_segment(scene, p(0.16, 0.54), p(0.42, 0.78), thickness, color);
    stroke_segment(scene, p(0.42, 0.78), p(0.86, 0.26), thickness, color);
}

/// 把一条有宽度的线段画成平行四边形（两个三角）。
fn stroke_segment(scene: &mut Scene, from: Point, to: Point, thickness: f32, color: Color) {
    let (dx, dy) = (to.x - from.x, to.y - from.y);
    let len = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    // 法向单位向量 × 半宽。
    let (nx, ny) = (-dy / len * thickness / 2.0, dx / len * thickness / 2.0);
    let a = Point::new(from.x + nx, from.y + ny);
    let b = Point::new(to.x + nx, to.y + ny);
    let c = Point::new(to.x - nx, to.y - ny);
    let d = Point::new(from.x - nx, from.y - ny);
    scene.triangle(a, b, c, color);
    scene.triangle(a, c, d, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SceneCommand;

    #[test]
    fn chevron_down_emits_triangle() {
        let mut scene = Scene::default();
        chevron_down(&mut scene, Rect::new(0.0, 0.0, 12.0, 12.0), Color::WHITE);
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], SceneCommand::Triangle { .. }));
    }

    #[test]
    fn chevron_down_tip_below_baseline() {
        let mut scene = Scene::default();
        chevron_down(&mut scene, Rect::new(10.0, 10.0, 20.0, 10.0), Color::WHITE);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        // 尖端（Y 最大）在 rect 中心下方
        let center_y = 15.0;
        let max_y = p0.y.max(p1.y).max(p2.y);
        assert!(max_y > center_y, "尖端应在中心下方，max_y={max_y}");
    }

    #[test]
    fn chevron_up_tip_above() {
        let mut scene = Scene::default();
        chevron_up(&mut scene, Rect::new(10.0, 10.0, 20.0, 10.0), Color::WHITE);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let center_y = 15.0;
        let min_y = p0.y.min(p1.y).min(p2.y);
        assert!(min_y < center_y, "尖端应在中心上方，min_y={min_y}");
    }

    #[test]
    fn check_emits_four_triangles_inside_rect() {
        let mut scene = Scene::default();
        let rect = Rect::new(10.0, 20.0, 12.0, 12.0);
        check(&mut scene, rect, Color::WHITE);
        assert_eq!(scene.commands.len(), 4, "两段折线各两个三角");
        for cmd in &scene.commands {
            let SceneCommand::Triangle { p0, p1, p2, .. } = cmd else {
                panic!("勾选标记只画三角");
            };
            for p in [p0, p1, p2] {
                assert!(
                    p.x >= rect.origin.x - 1.0
                        && p.x <= rect.right() + 1.0
                        && p.y >= rect.origin.y - 1.0
                        && p.y <= rect.bottom() + 1.0,
                    "笔迹越出 rect：{p:?} ⊄ {rect:?}"
                );
            }
        }
    }

    #[test]
    fn check_thickness_scales_with_rect() {
        // 行高翻倍时笔画也应更粗，否则 2× 下勾会细成一根发丝。
        let mut small = Scene::default();
        check(&mut small, Rect::new(0.0, 0.0, 12.0, 12.0), Color::WHITE);
        let mut large = Scene::default();
        check(&mut large, Rect::new(0.0, 0.0, 24.0, 24.0), Color::WHITE);
        let width = |scene: &Scene| {
            let SceneCommand::Triangle { p0, p1, .. } = &scene.commands[0] else {
                panic!("应画三角");
            };
            (p1.y - p0.y).abs() + (p1.x - p0.x).abs()
        };
        assert!(width(&large) > width(&small), "大尺寸笔画更粗");
    }
}
