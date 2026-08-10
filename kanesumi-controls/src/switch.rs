use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Rect};

use crate::state::ControlState;

/// MetroSwitch —— 开关。参 CONTROL_SPEC §3：
/// - 轨道 40×20 胶囊、Knob 20×20、行程 20px；
/// - 切换滑动 **0.15s Cubic/EaseOut**（RepositionThemeAnimation），换色瞬时。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSwitch {
    pub checked: bool,
    pub state: ControlState,
    /// 标题（开关左侧文字）。空则不画。
    pub label: String,
    /// 轨道几何（可覆盖，默认 40×20）。
    pub track_width: f32,
    pub track_height: f32,
    /// Knob 滑动进度 [0,1]，由 `update(dt)` 推进。
    knob: MetroAnim,
}

impl Default for MetroSwitch {
    fn default() -> Self {
        Self {
            checked: false,
            state: ControlState::Normal,
            label: String::new(),
            track_width: 40.0,
            track_height: 20.0,
            knob: MetroAnim::new(0.15, UwpEasing::Cubic, EasingMode::EaseOut),
        }
    }
}

impl MetroSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_label(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    /// 切换选中态：目标改向，Knob 从当前位置 0.15s 滑动到新目标（可中断）。
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.knob.set_target(if checked { 1.0 } else { 0.0 });
    }

    /// 每帧推进（Knob 滑动动画）。
    pub fn update(&mut self, dt: f64) {
        self.knob.update(dt);
    }

    /// Knob 当前横向位移（px）。行程 = 轨道宽 − Knob 直径。
    pub fn knob_offset(&self) -> f32 {
        self.knob.value() as f32 * (self.track_width - self.track_height)
    }

    pub fn is_animating(&self) -> bool {
        !self.knob.is_steady()
    }

    /// 渲染到 `rect`。标签靠左，轨道靠右垂直居中。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let disabled = self.state == ControlState::Disabled;
        let a = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };

        // 标签（左侧）
        if !self.label.is_empty() {
            let style = theme.typography.body;
            let label_rect = Rect::new(
                rect.origin.x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                rect.size.width - self.track_width - 12.0,
                style.line_height,
            );
            let fg = colors.on_surface.with_alpha(colors.on_surface.a * a);
            scene.text(self.label.clone(), label_rect, fg, style, TextAlign::Left);
        }

        // 轨道（靠右，垂直居中）
        let track_rect = Rect::new(
            rect.origin.x + rect.size.width - self.track_width,
            rect.origin.y + (rect.size.height - self.track_height) / 2.0,
            self.track_width,
            self.track_height,
        );
        let knob_diameter = self.track_height;

        let (track_fill, track_stroke, knob_color) = if self.checked {
            let fill = match self.state {
                ControlState::Hovered => colors.primary.lerp(Color::WHITE, 0.20),
                ControlState::Pressed => colors.primary.lerp(Color::BLACK, 0.20),
                _ => colors.primary,
            };
            (fill.with_alpha(a), Color::TRANSPARENT, Color::WHITE)
        } else {
            let stroke = match self.state {
                ControlState::Hovered => colors.on_surface_variant.lerp(Color::WHITE, 0.30),
                ControlState::Pressed => colors.on_surface,
                _ => colors.on_surface_variant,
            };
            (
                Color::TRANSPARENT,
                stroke.with_alpha(a),
                Color::WHITE.with_alpha(a),
            )
        };

        if track_fill.a > 0.0 {
            scene.fill_rounded_rect(track_fill, track_rect, CornerRadius::Capsule);
        }
        if track_stroke.a > 0.0 {
            scene.stroke_rounded_rect(track_stroke, track_rect, 1.0, CornerRadius::Capsule);
        }

        // Knob（白圆，位置 = 行程 × 进度）
        let knob_x = track_rect.origin.x + self.knob_offset();
        let knob_rect = Rect::new(knob_x, track_rect.origin.y, knob_diameter, knob_diameter);
        scene.fill_rounded_rect(knob_color, knob_rect, CornerRadius::Capsule);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    #[test]
    fn travel_is_track_minus_knob() {
        let s = MetroSwitch::new();
        assert_eq!(s.knob_offset(), 0.0);
        assert_eq!(s.track_width - s.track_height, 20.0, "行程 20px");
    }

    #[test]
    fn toggle_animates_to_target() {
        let mut s = MetroSwitch::new();
        s.set_checked(true);
        assert!(s.is_animating());
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        assert!(!s.is_animating());
        assert_eq!(s.knob_offset(), 20.0, "On 态 Knob 在行程末端");
    }

    #[test]
    fn toggle_is_interruptible() {
        let mut s = MetroSwitch::new();
        s.set_checked(true);
        s.update(0.05);
        let mid = s.knob_offset();
        assert!(mid > 0.0 && mid < 20.0);
        s.set_checked(false);
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        assert_eq!(s.knob_offset(), 0.0);
    }

    #[test]
    fn renders_track_and_knob() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut s = MetroSwitch::with_label("Wi-Fi");
        s.set_checked(true);
        s.update(1.0);
        let mut scene = Scene::default();
        s.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 120.0, 40.0),
            &mut scene,
        );
        // 标签文本 + 轨道填充 + Knob
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { .. }))
        );
        let fills: Vec<f32> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { corner_radius, .. } => Some(*corner_radius),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 2, "轨道 + Knob");
        assert!(fills.iter().all(|r| *r == 10.0), "胶囊圆角");
    }

    #[test]
    fn disabled_lowers_alpha() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut s = MetroSwitch::new();
        s.state = ControlState::Disabled;
        s.set_checked(true);
        s.update(1.0);
        let mut scene = Scene::default();
        s.render(&theme, &engine, Rect::new(0.0, 0.0, 40.0, 40.0), &mut scene);
        let Some(SceneCommand::FillRect { color, .. }) = scene.commands.first() else {
            panic!("首命令应为轨道填充");
        };
        assert!(color.a < 1.0, "禁用态轨道应降透明度，实际 a={}", color.a);
    }
}
