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

    /// 快速换向（打断进行中的动画）—— 目标翻转后 progress 应回落再上升。
    #[test]
    fn rapid_flip_interrupts_prior_anim() {
        let mut i = MetroAnimatedIcon::new();
        i.set_state(true);
        i.update(0.03); // 走 30% 途中
        let mid = i.progress();
        assert!(mid > 0.0 && mid < 1.0, "中途 progress 应在开区间");
        // 立即翻回 false —— 新一轮动画从头（progress 归零后向 1 走）
        i.set_state(false);
        assert!(!i.state_on, "state 应翻");
        assert!(i.is_animating(), "新动画在跑");
        assert_eq!(i.progress(), 0.0, "换向应从 0 起 —— MetroAnim 重建");
        for _ in 0..60 {
            i.update(1.0 / 60.0);
        }
        assert_eq!(i.progress(), 1.0);
    }

    /// 正交对方向（Down↔Up、Left↔Right）在初始（progress=0）和终止（progress=1）时
    /// 分别对应 dir_off / dir_on 的 tip 位置。
    #[test]
    fn morph_endpoints_align_with_direction() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon::new();
        // 未触发（state_off，anim 未 set_target）→ 直接画 dir_off = Down，tip 在下方。
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let max_y = p0.y.max(p1.y).max(p2.y);
        assert!(max_y > 8.0, "Down chevron tip 在 y > 8（rect 中心 8）");
        // 切到 on 并跑完 → dir_on = Up，tip 在上方。
        i.set_state(true);
        for _ in 0..60 {
            i.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let min_y = p0.y.min(p1.y).min(p2.y);
        assert!(min_y < 8.0, "Up chevron tip 在 y < 8");
    }

    /// 自定义方向对（Left↔Right）—— 换向后 tip 水平位置翻转。
    #[test]
    fn custom_direction_pair_left_right() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon {
            dir_off: IconDirection::Left,
            dir_on: IconDirection::Right,
            ..MetroAnimatedIcon::default()
        };
        // off → Left：tip 在左（x < 中心 8）
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let min_x = p0.x.min(p1.x).min(p2.x);
        assert!(min_x < 8.0, "Left chevron tip 在 x < 8");
        // on → Right，跑完
        i.set_state(true);
        for _ in 0..60 {
            i.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let max_x = p0.x.max(p1.x).max(p2.x);
        assert!(max_x > 8.0, "Right chevron tip 在 x > 8");
    }

    /// 同方向对（dir_off == dir_on）—— 不走 morph 分支，直接画静态 chevron。
    #[test]
    fn same_direction_pair_renders_static() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon {
            dir_off: IconDirection::Down,
            dir_on: IconDirection::Down,
            ..MetroAnimatedIcon::default()
        };
        // state 切换但方向相同 —— render 走 else 分支（静态 chevron）
        i.set_state(true);
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        assert_eq!(scene.commands.len(), 1, "同方向对只画一个三角");
        assert!(matches!(scene.commands[0], SceneCommand::Triangle { .. }));
    }

    /// Progress 边界（0 / 1）时 tip 严格对齐端点方向，无插值残留。
    #[test]
    fn progress_zero_matches_from_direction() {
        let theme = MetroTheme::ether_dark();
        let mut i = MetroAnimatedIcon::new();
        i.set_state(true);
        // 未 update → progress 应为 0，此时 render 用 tip=dir_off=Down（tip 在下方）
        assert_eq!(i.progress(), 0.0);
        let mut scene = Scene::default();
        i.render(&theme, Rect::new(0.0, 0.0, 16.0, 16.0), &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = &scene.commands[0] else {
            panic!("应画三角");
        };
        let max_y = p0.y.max(p1.y).max(p2.y);
        assert!(max_y > 8.0, "progress=0 时 tip 保持 dir_off（Down）");
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
