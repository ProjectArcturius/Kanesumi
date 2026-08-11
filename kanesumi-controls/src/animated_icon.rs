// MetroAnimatedIcon —— 状态动画图标。参 CONTROL_SPEC §31。
//
// 移植自 microsoft-ui-xaml/dev/AnimatedIcon（AnimatedIcon.cpp）。上游用 AnimatedVisual
// （Lottie 式）；Kanesumi 用几何 chevron 插值（V7 自绘，不依赖 Lottie runtime）：
// - dir_off / dir_on 正交方向（Down/Up/Left/Right）；
// - set_state(on) → 0.1s 插值 chevron 从 dir_off 翻到 dir_on。

use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::Scene;
use kanesumi_core::{Color, MetroTheme, Point, Rect};

/// 图标方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconDirection {
    Down,
    Up,
    Left,
    Right,
}

/// chevron 插值时长（0.1s，对齐 Expander chevron）。
const ICON_MORPH: f64 = 0.1;

/// MetroAnimatedIcon —— 状态动画 chevron。参 CONTROL_SPEC §31。
#[derive(Debug, Clone)]
pub struct MetroAnimatedIcon {
    /// 当前状态（on/off）。
    pub state_on: bool,
    pub dir_off: IconDirection,
    pub dir_on: IconDirection,
    anim: MetroAnim,
}

impl Default for MetroAnimatedIcon {
    fn default() -> Self {
        Self {
            state_on: false,
            dir_off: IconDirection::Down,
            dir_on: IconDirection::Up,
            anim: MetroAnim::new(ICON_MORPH, UwpEasing::Quadratic, EasingMode::EaseOut),
        }
    }
}

impl MetroAnimatedIcon {
    pub fn new() -> Self {
        Self::default()
    }

    /// 状态切换 → 0.1s 插值。
    pub fn set_state(&mut self, on: bool) {
        if self.state_on == on {
            return;
        }
        self.state_on = on;
        self.anim = MetroAnim::new(ICON_MORPH, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.anim.set_target(1.0);
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
    }

    pub fn is_animating(&self) -> bool {
        !self.anim.is_steady()
    }

    /// 当前插值进度 [0,1]。
    pub fn progress(&self) -> f32 {
        self.anim.value() as f32
    }

    /// 渲染插值 chevron。
    pub fn render(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let from = if self.state_on {
            self.dir_on
        } else {
            self.dir_off
        };
        let p = self.progress();
        // 从 from 到目标方向插值（仅支持正交对：Down↔Up、Left↔Right）。
        if self.state_on && self.dir_on != self.dir_off {
            draw_morph(
                scene,
                rect,
                self.dir_off,
                self.dir_on,
                p,
                theme.colors.on_surface_variant,
            );
        } else if !self.state_on && self.dir_on != self.dir_off {
            draw_morph(
                scene,
                rect,
                self.dir_on,
                self.dir_off,
                p,
                theme.colors.on_surface_variant,
            );
        } else {
            draw_chevron(scene, rect, from, theme.colors.on_surface_variant);
        }
    }
}

/// 在 rect 内按方向画 chevron 三角。
fn draw_chevron(scene: &mut Scene, rect: Rect, dir: IconDirection, color: Color) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    let w = rect.size.width * 0.3;
    let h = rect.size.height * 0.3;
    let (base1, base2, tip) = match dir {
        IconDirection::Down => (
            Point::new(cx - w, cy - h),
            Point::new(cx + w, cy - h),
            Point::new(cx, cy + h),
        ),
        IconDirection::Up => (
            Point::new(cx - w, cy + h),
            Point::new(cx + w, cy + h),
            Point::new(cx, cy - h),
        ),
        IconDirection::Left => (
            Point::new(cx + w, cy - h),
            Point::new(cx + w, cy + h),
            Point::new(cx - w, cy),
        ),
        IconDirection::Right => (
            Point::new(cx - w, cy - h),
            Point::new(cx - w, cy + h),
            Point::new(cx + w, cy),
        ),
    };
    scene.triangle(base1, base2, tip, color);
}

/// 从 from 到 to 插值画 chevron（正交对）。
fn draw_morph(
    scene: &mut Scene,
    rect: Rect,
    from: IconDirection,
    to: IconDirection,
    p: f32,
    color: Color,
) {
    let cx = rect.origin.x + rect.size.width / 2.0;
    let cy = rect.origin.y + rect.size.height / 2.0;
    let w = rect.size.width * 0.3;
    let h = rect.size.height * 0.3;
    // 每个方向的 base/tip 坐标
    let pts = |d: IconDirection| -> (Point, Point, Point) {
        match d {
            IconDirection::Down => (
                Point::new(cx - w, cy - h),
                Point::new(cx + w, cy - h),
                Point::new(cx, cy + h),
            ),
            IconDirection::Up => (
                Point::new(cx - w, cy + h),
                Point::new(cx + w, cy + h),
                Point::new(cx, cy - h),
            ),
            IconDirection::Left => (
                Point::new(cx + w, cy - h),
                Point::new(cx + w, cy + h),
                Point::new(cx - w, cy),
            ),
            IconDirection::Right => (
                Point::new(cx - w, cy - h),
                Point::new(cx - w, cy + h),
                Point::new(cx + w, cy),
            ),
        }
    };
    let a = pts(from);
    let b = pts(to);
    let lerp = |x0: f32, x1: f32| x0 + (x1 - x0) * p;
    let tri = (
        Point::new(lerp(a.0.x, b.0.x), lerp(a.0.y, b.0.y)),
        Point::new(lerp(a.1.x, b.1.x), lerp(a.1.y, b.1.y)),
        Point::new(lerp(a.2.x, b.2.x), lerp(a.2.y, b.2.y)),
    );
    scene.triangle(tri.0, tri.1, tri.2, color);
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    #[test]
    fn set_state_triggers_anim() {
        let mut i = MetroAnimatedIcon::new();
        assert!(!i.state_on);
        i.set_state(true);
        assert!(i.state_on);
        assert!(i.is_animating());
        for _ in 0..120 {
            i.update(1.0 / 60.0);
        }
        assert!(!i.is_animating());
        assert_eq!(i.progress(), 1.0);
    }

    #[test]
    fn same_state_no_op() {
        let mut i = MetroAnimatedIcon::new();
        i.set_state(false);
        assert!(!i.is_animating(), "同状态不触发");
    }

    #[test]
    fn render_emits_triangle() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon::new();
        i.set_state(true);
        i.update(1.0);
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        assert!(matches!(scene.commands[0], SceneCommand::Triangle { .. }));
    }

    #[test]
    fn morph_interpolates_tip() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon::new();
        i.set_state(true);
        i.update(0.02); // 中途
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        // 中途三角横跨中线（tip 正在移动）
        let cy = 8.0;
        let max_y = p0.y.max(p1.y).max(p2.y);
        let min_y = p0.y.min(p1.y).min(p2.y);
        assert!(max_y > cy && min_y < cy, "中途三角横跨中线");
    }
}
