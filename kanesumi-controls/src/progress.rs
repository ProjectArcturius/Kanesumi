use kanesumi_anim::{DURATION_INDETERMINATE, EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect};

/// 进度指示模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressMode {
    Determinate,
    Indeterminate,
}

/// MetroProgressBar —— 横向进度条。参 CONTROL_SPEC §4：
/// - MinHeight 4；确定模式值变化 **0.15s** 滑动（RepositionThemeAnimation）；
/// - 不确定模式 **2.0s** 循环，两波脉冲（40%/60% 块），KeySpline≈Cubic/EaseInOut；
/// - Paused 条色 0.25s 淡出至 0.6；Error 换错误色。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroProgressBar {
    /// 确定模式进度 [0,1]。
    pub value: f32,
    pub mode: ProgressMode,
    pub paused: bool,
    pub error: bool,
    pub min_height: f32,
    /// 不确定循环相位 [0,2.0)s，由 `update(dt)` 推进。
    phase: f64,
    /// 确定模式显示值（0.15s 滑动），由 `update(dt)` 推进。
    slide: MetroAnim,
}

/// 错误色 —— Windows 系统错误红（无主题 token，常量）。
pub const ERROR_COLOR: Color = Color::from_hex(0xE8_11_23);

impl Default for MetroProgressBar {
    fn default() -> Self {
        Self {
            value: 0.0,
            mode: ProgressMode::Determinate,
            paused: false,
            error: false,
            min_height: 4.0,
            phase: 0.0,
            slide: MetroAnim::new(0.15, UwpEasing::Cubic, EasingMode::EaseOut),
        }
    }
}

impl MetroProgressBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn indeterminate() -> Self {
        Self {
            mode: ProgressMode::Indeterminate,
            ..Self::default()
        }
    }

    /// 设置确定进度：显示值 0.15s 滑向新目标。
    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
        self.slide.set_target(self.value as f64);
    }

    /// 每帧推进（0.15s 滑动 / 2.0s 不确定循环相位）。
    pub fn update(&mut self, dt: f64) {
        if self.mode == ProgressMode::Indeterminate {
            self.phase = (self.phase + dt) % DURATION_INDETERMINATE;
        } else {
            self.slide.update(dt);
        }
    }

    /// 当前显示进度 [0,1]（已含滑动动画）。
    pub fn display_value(&self) -> f32 {
        self.slide.value() as f32
    }

    /// 渲染到 `rect`。轨道为细底，指示条为强调色（错误红 / 暂停 0.6 不透明）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let height = self.min_height.max(rect.size.height.min(4.0));
        let bar_rect = Rect::new(
            rect.origin.x,
            rect.origin.y + (rect.size.height - height) / 2.0,
            rect.size.width,
            height,
        );

        // 轨道（细底）
        scene.fill_rounded_rect(
            colors.surface_variant.with_alpha(0.6),
            bar_rect,
            CornerRadius::Capsule,
        );

        // 指示条裁剪到轨道内（box 语义，参 scene.rs ClipRect）—— 不确定模式指示条
        // 从轨道外滑入，必须裁剪到轨道边界，禁止溢出到控件外。
        scene.clip(Some(bar_rect));

        let indicator_color = if self.error {
            ERROR_COLOR
        } else {
            colors.primary
        };
        let indicator_alpha = if self.paused { 0.6 } else { 1.0 };
        let color = indicator_color.with_alpha(indicator_color.a * indicator_alpha);

        match self.mode {
            ProgressMode::Determinate => {
                let w = self.display_value() * bar_rect.size.width;
                if w > 0.0 {
                    let ind_rect = Rect::new(bar_rect.origin.x, bar_rect.origin.y, w, height);
                    scene.fill_rounded_rect(color, ind_rect, CornerRadius::Capsule);
                }
            }
            ProgressMode::Indeterminate => {
                // 两波脉冲在轨道内往返滑动（始终部分可见，不滑出轨道）。
                // 对齐 Metro 观感：40% 块左→右（0→1.0s），60% 块相位 +0.5s。
                // 参 CONTROL_SPEC §4（KeySpline Cubic/EaseInOut，2.0s 循环）。
                let phase = self.phase / DURATION_INDETERMINATE; // [0,1)
                let w = bar_rect.size.width;
                let (w1, x1) = pulse_in_track(phase, 0.40, 0.0);
                let (w2, x2) = pulse_in_track(phase, 0.60, 0.5);
                for (pw, px) in [(w1, x1), (w2, x2)] {
                    if pw <= 0.0 {
                        continue;
                    }
                    let ind_rect = Rect::new(
                        bar_rect.origin.x + px * w,
                        bar_rect.origin.y,
                        pw * w,
                        height,
                    );
                    scene.fill_rounded_rect(color, ind_rect, CornerRadius::Capsule);
                }
            }
        }
        // 清除裁剪
        scene.clip(None);
    }
}

/// 轨道内脉冲：指示条始终在轨道内，按相位滑动（往返）。
/// `width_frac` = 指示条宽（轨道宽比例）；`phase_shift` = 相位偏移 [0,1)。
/// 返回 (宽度比例, 轨道内起点比例 [0, 1-width])。
/// 用三角波在 [0, 1-width] 往返，Cubic EaseInOut 平滑。
fn pulse_in_track(phase: f64, width_frac: f32, phase_shift: f64) -> (f32, f32) {
    let t = (phase + phase_shift).fract();
    // 往返：前半 [0,0.5) 左→右，后半 [0.5,1) 右→左
    let e = if t < 0.5 {
        cubic_ease_in_out(t * 2.0) // 0→1
    } else {
        1.0 - cubic_ease_in_out((t - 0.5) * 2.0) // 1→0
    };
    let max_x = 1.0 - width_frac;
    (width_frac, e as f32 * max_x)
}

/// Cubic EaseInOut（KeySpline 0.4,0,0.6,1 近似）。
fn cubic_ease_in_out(t: f64) -> f64 {
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}

/// MetroProgressRing —— 环形进度。参 CONTROL_SPEC §5：
/// - 默认 32×32、线宽 4；确定模式弧角度 = value×360（回退瞬跳）；
/// - 不确定模式 **2.0s** 旋转 0°→900°，弧 90°→180° 呼吸（TrimEnd/TrimStart 近似）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroProgressRing {
    /// 确定模式进度 [0,1]。
    pub value: f32,
    pub indeterminate: bool,
    /// false → 隐藏（Inactive 态，整根不画）。
    pub active: bool,
    pub size: f32,
    pub thickness: f32,
    /// 不确定旋转相位 [0,2.0)s，由 `update(dt)` 推进。
    phase: f64,
}

impl Default for MetroProgressRing {
    fn default() -> Self {
        Self {
            value: 0.0,
            indeterminate: false,
            active: true,
            size: 32.0,
            thickness: 4.0,
            phase: 0.0,
        }
    }
}

impl MetroProgressRing {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_value(&mut self, value: f32) {
        self.value = value.clamp(0.0, 1.0);
    }

    /// 每帧推进不确定相位。
    pub fn update(&mut self, dt: f64) {
        if self.indeterminate {
            self.phase = (self.phase + dt) % DURATION_INDETERMINATE;
        }
    }

    /// 渲染到 `rect` 中心。
    pub fn render(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        if !self.active {
            return;
        }
        let size = self.size.min(rect.size.width.min(rect.size.height));
        let center = Point::new(
            rect.origin.x + (rect.size.width - size) / 2.0 + size / 2.0,
            rect.origin.y + (rect.size.height - size) / 2.0 + size / 2.0,
        );
        let radius = (size - self.thickness) / 2.0;
        let color = theme.colors.primary;

        if self.indeterminate {
            // 旋转 0°→900°（2.5 圈）@2s，双段平滑；弧 90°→180° 呼吸
            let t = (self.phase / DURATION_INDETERMINATE) as f32; // [0,1)
            let rotation = 900.0 * t;
            let sweep = 90.0
                + 90.0
                    * if t < 0.5 {
                        cubic_ease_in_out(t as f64 * 2.0) as f32
                    } else {
                        1.0
                    };
            scene.arc(
                center,
                radius,
                self.thickness,
                color,
                rotation,
                rotation + sweep,
            );
        } else {
            let sweep = self.value * 360.0;
            if sweep > 0.0 {
                scene.arc(center, radius, self.thickness, color, 0.0, sweep);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn determinate_slides_to_value() {
        let mut bar = MetroProgressBar::new();
        assert_eq!(bar.display_value(), 0.0, "初始 0");
        bar.set_value(0.5);
        for _ in 0..120 {
            bar.update(1.0 / 60.0);
        }
        assert_eq!(bar.display_value(), 0.5, "0.15s 后到达目标");
    }

    #[test]
    fn determinate_renders_indicator() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut bar = MetroProgressBar::new();
        bar.set_value(0.5);
        for _ in 0..120 {
            bar.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        bar.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 4.0), &mut scene);
        // 轨道 + 指示条（+ clip 进出）
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 2, "轨道 + 指示条");
        let Some(SceneCommand::FillRect { rect: ind, .. }) = scene
            .commands
            .iter()
            .find(|c| matches!(c, SceneCommand::FillRect { color, .. } if (color.r - theme.colors.primary.r).abs() < 0.1))
        else {
            panic!("应有指示条");
        };
        assert!(
            (ind.size.width - 100.0).abs() < 0.5,
            "50% → 100px，实际 {}",
            ind.size.width
        );
    }

    #[test]
    fn indeterminate_cycles_two_pulses() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut bar = MetroProgressBar::indeterminate();
        bar.update(1.5); // phase = 1.5s
        let mut scene = Scene::default();
        bar.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 4.0), &mut scene);
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 3, "轨道 + 两波脉冲");
    }

    #[test]
    fn indeterminate_indicators_stay_in_track() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let track = Rect::new(0.0, 0.0, 200.0, 4.0);
        // 多个相位：指示条必须始终在轨道内（不溢出）
        for t in [0.0, 0.25, 0.5, 0.75, 1.0, 1.5, 1.9] {
            let mut bar = MetroProgressBar::indeterminate();
            bar.phase = t;
            let mut scene = Scene::default();
            bar.render(&theme, &engine, track, &mut scene);
            for c in &scene.commands {
                if let SceneCommand::FillRect { rect, color, .. } = c {
                    // 只检查指示条（primary 色）
                    if (color.r - theme.colors.primary.r).abs() < 0.1 {
                        assert!(
                            rect.origin.x >= track.origin.x - 0.01,
                            "t={t} 指示条左越界 x={}",
                            rect.origin.x
                        );
                        assert!(
                            rect.right() <= track.right() + 0.01,
                            "t={t} 指示条右越界 right={}",
                            rect.right()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn indeterminate_phase_wraps() {
        let mut bar = MetroProgressBar::indeterminate();
        bar.update(2.5);
        assert!(bar.phase < DURATION_INDETERMINATE);
    }

    #[test]
    fn ring_determinate_sweep_maps_value() {
        let theme = MetroTheme::ether_dark();
        let mut ring = MetroProgressRing::new();
        ring.set_value(0.75);
        let mut scene = Scene::default();
        ring.render(&theme, Rect::new(0.0, 0.0, 32.0, 32.0), &mut scene);
        let Some(SceneCommand::Arc { end_deg, .. }) = scene.commands.first() else {
            panic!("确定模式应画弧");
        };
        assert!((*end_deg - 270.0).abs() < 0.5, "75% → 270°，实际 {end_deg}");
    }

    #[test]
    fn ring_indeterminate_rotates() {
        let theme = MetroTheme::ether_dark();
        let mut ring = MetroProgressRing::new();
        ring.indeterminate = true;
        let mut scene = Scene::default();
        ring.render(&theme, Rect::new(0.0, 0.0, 32.0, 32.0), &mut scene);
        let start = match &scene.commands[0] {
            SceneCommand::Arc { start_deg, .. } => *start_deg,
            _ => panic!("应画弧"),
        };
        assert_eq!(start, 0.0);
        ring.update(1.0); // phase = 1.0s → 旋转 450°
        let mut scene2 = Scene::default();
        ring.render(&theme, Rect::new(0.0, 0.0, 32.0, 32.0), &mut scene2);
        let start2 = match &scene2.commands[0] {
            SceneCommand::Arc { start_deg, .. } => *start_deg,
            _ => panic!("应画弧"),
        };
        assert!((start2 - 450.0).abs() < 1.0, "1s → 450°，实际 {start2}");
    }

    #[test]
    fn ring_inactive_hides() {
        let theme = MetroTheme::ether_dark();
        let mut ring = MetroProgressRing::new();
        ring.active = false;
        let mut scene = Scene::default();
        ring.render(&theme, Rect::new(0.0, 0.0, 32.0, 32.0), &mut scene);
        assert!(scene.is_empty());
    }
}
