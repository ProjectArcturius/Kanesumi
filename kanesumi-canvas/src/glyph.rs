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
}
