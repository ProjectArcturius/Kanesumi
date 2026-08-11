// MetroSwitch —— Kanesumi 开关。参 CONTROL_SPEC §3（Lumia 950 / UWP ToggleSwitch 复刻）。
//
// 结构：Header（可选，顶部）+ Track（下方）+ State text（Track 右侧）。
// 参 A1 重做与 docs/VISUAL_ISSUES.md V11：原实现 label 居左、Knob 塞满轨道，
// 与 Lumia 观感不符（更像 iOS）。真机截图（wp_ss_20250619_0002）显示：
//   - Knob 明显小于 track（约 70% 高，上下 3-4px 留白）；
//   - ON = 强调色实心无描边；OFF = 透明 + 白色 2px 描边；
//   - Pressed = 中灰实心（accent 被 dim 掉）；
//   - Header 在上、State text（"開啟"/"關閉"）在 Track 右侧。
// 交互：支持点动 + 拖动 slide。3px 阈值区分：
//   - <3px 位移 = 点动 → toggle；
//   - ≥3px 位移 = 拖动 → 按 knob 中心过半判 on/off。

use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect};

use crate::state::ControlState;

/// Switch 形态。Capsule = Lumia/UWP 复刻（本轮重做）；Square = WP7 直角（待完善）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchShape {
    Capsule,
    Square,
}

impl SwitchShape {
    /// 轨道宽/高（逻辑像素）。
    /// 数据源：
    /// - Capsule = `reference/microsoft-ui-xaml/dev/CommonStyles/ToggleSwitch_themeresources_v1.xaml`
    ///   `OuterBorder Width="40" Height="20"`（WinUI 2 v1 = Metro/Lumia 规格）。
    /// - Square = Windows 8 Modern UI ToggleSwitch 真机截图（`11~2.jpg`）——
    ///   同 40×20 底盘，只是 0 圆角 + 瘦长 knob。
    fn track_size(self) -> (f32, f32) {
        match self {
            SwitchShape::Capsule => (40.0, 20.0),
            SwitchShape::Square => (40.0, 20.0),
        }
    }
    /// Knob 宽/高。
    /// - Capsule：10×10 圆（UWP v1 SwitchKnobOn/Off `Width="10" Height="10"`）
    /// - Square：**10×20 瘦长矩形**（Win8 观感 —— knob 与轨道等高、瘦竖块）
    fn knob_size(self) -> (f32, f32) {
        match self {
            SwitchShape::Capsule => (10.0, 10.0),
            SwitchShape::Square => (10.0, 20.0),
        }
    }
    /// Knob 距轨道边缘留白（上下 / 左右对称）。
    /// Capsule = (20−10)/2 = 5；Square = 0（贴满高度）。
    fn knob_margin(self) -> f32 {
        match self {
            SwitchShape::Capsule => 5.0,
            SwitchShape::Square => 0.0,
        }
    }
    fn corner(self) -> CornerRadius {
        match self {
            SwitchShape::Capsule => CornerRadius::Capsule,
            SwitchShape::Square => CornerRadius::Square,
        }
    }
    fn knob_corner(self) -> CornerRadius {
        match self {
            SwitchShape::Capsule => CornerRadius::Capsule,
            SwitchShape::Square => CornerRadius::Square,
        }
    }
}

/// 拖动状态。记录起点位移，用于区分点动/拖动。
#[derive(Debug, Clone, Copy, PartialEq)]
struct DragState {
    start_pointer_x: f32,
    start_progress: f32,
    /// 是否已越过 3px 阈值（越过后按拖动处理）。
    moved: bool,
}

/// MetroSwitch —— 开关。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSwitch {
    pub checked: bool,
    pub state: ControlState,
    pub shape: SwitchShape,
    /// 顶部标题（可选）—— UWP `Header`。
    pub header: String,
    /// ON 状态文本，默认 "On"。UWP `OnContent`。
    pub on_text: String,
    /// OFF 状态文本，默认 "Off"。UWP `OffContent`。
    pub off_text: String,
    /// Knob 滑动进度 [0,1]，由 `update(dt)` 推进 或 拖动时 jump_to。
    knob: MetroAnim,
    /// 拖动状态。`Some` = 指针按住中。
    drag: Option<DragState>,
}

impl Default for MetroSwitch {
    fn default() -> Self {
        Self {
            checked: false,
            state: ControlState::Normal,
            shape: SwitchShape::Capsule,
            header: String::new(),
            on_text: "On".into(),
            off_text: "Off".into(),
            knob: MetroAnim::new(0.15, UwpEasing::Cubic, EasingMode::EaseOut),
            drag: None,
        }
    }
}

impl MetroSwitch {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带 Header 构造（UWP `<ToggleSwitch Header="…" />`）。
    pub fn with_header(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            ..Self::default()
        }
    }

    /// 兼容旧调用点 —— label 在 Lumia/UWP 里叫 Header 且位于上方（本次重做修正）。
    #[deprecated(note = "改用 with_header —— label 概念已归入 Header（顶部）")]
    pub fn with_label(label: impl Into<String>) -> Self {
        Self::with_header(label)
    }

    /// 设置 On/Off 状态文本（UWP `OnContent` / `OffContent`）。
    pub fn with_state_text(mut self, on: impl Into<String>, off: impl Into<String>) -> Self {
        self.on_text = on.into();
        self.off_text = off.into();
        self
    }

    /// 切换形态（Capsule / Square）。
    pub fn with_shape(mut self, shape: SwitchShape) -> Self {
        self.shape = shape;
        self
    }

    /// 编程式设置选中 —— 动画从当前 progress 滑向新目标。
    pub fn set_checked(&mut self, checked: bool) {
        if self.checked == checked {
            return;
        }
        self.checked = checked;
        self.knob.set_target(if checked { 1.0 } else { 0.0 });
    }

    /// 每帧推进 Knob 滑动动画。拖动进行中不推进 —— knob 由指针位置直接决定。
    pub fn update(&mut self, dt: f64) {
        if self.drag.is_none() {
            self.knob.update(dt);
        }
    }

    /// 当前 Knob progress [0,1]。
    pub fn progress(&self) -> f32 {
        self.knob.value() as f32
    }

    pub fn is_animating(&self) -> bool {
        !self.knob.is_steady()
    }

    /// 轨道行程（Knob 可移动像素数）= 轨宽 − 2×留白 − 轮宽。
    fn travel(&self) -> f32 {
        let (tw, _) = self.shape.track_size();
        let (kw, _) = self.shape.knob_size();
        tw - 2.0 * self.shape.knob_margin() - kw
    }

    /// 轨道在宿主 `rect` 内的实际位置（Header 占顶后再垂直居中）。
    pub fn track_rect(&self, rect: Rect, theme: &MetroTheme) -> Rect {
        let (tw, th) = self.shape.track_size();
        let body = theme.typography.body;
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            body.line_height + 8.0
        };
        // Track 行高 = max(track, body.line_height) —— 让 state text 与 track 同基线居中
        let track_row_h = th.max(body.line_height);
        let track_y = rect.origin.y + header_h + (track_row_h - th) / 2.0;
        Rect::new(rect.origin.x, track_y, tw, th)
    }

    /// Knob 矩形（含当前 progress 位移）。
    pub fn knob_rect(&self, rect: Rect, theme: &MetroTheme) -> Rect {
        let track = self.track_rect(rect, theme);
        let (kw, kh) = self.shape.knob_size();
        let margin = self.shape.knob_margin();
        let knob_x = track.origin.x + margin + self.travel() * self.progress();
        let knob_y = track.origin.y + (track.size.height - kh) / 2.0;
        Rect::new(knob_x, knob_y, kw, kh)
    }

    /// 命中：整个轨道范围（含 knob 周围留白），便于拖动。
    pub fn hit_test(&self, rect: Rect, theme: &MetroTheme, pos: Point) -> bool {
        self.track_rect(rect, theme).contains(pos)
    }

    /// 指针按下 —— 命中轨道则记录起点，进入 Pressed 态。
    pub fn press(&mut self, rect: Rect, theme: &MetroTheme, pos: Point) {
        if !self.hit_test(rect, theme, pos) {
            return;
        }
        self.state = ControlState::Pressed;
        self.drag = Some(DragState {
            start_pointer_x: pos.x,
            start_progress: self.progress(),
            moved: false,
        });
    }

    /// 指针移动 —— 若正在拖动，实时更新 knob progress。
    pub fn drag_to(&mut self, pos: Point) {
        let travel = self.travel();
        let Some(drag) = self.drag.as_mut() else {
            return;
        };
        let dx = pos.x - drag.start_pointer_x;
        if dx.abs() > 3.0 {
            drag.moved = true;
        }
        if drag.moved && travel > 0.0 {
            let new_progress = (drag.start_progress + dx / travel).clamp(0.0, 1.0);
            self.knob.jump_to(new_progress as f64);
        }
    }

    /// 指针释放 —— 拖动过则按 knob 过半判 on/off；未拖动视为点动 toggle。
    /// 返回 true 表示 `checked` 变化。
    pub fn release(&mut self) -> bool {
        let Some(drag) = self.drag.take() else {
            return false;
        };
        self.state = ControlState::Normal;
        let old = self.checked;
        if drag.moved {
            self.checked = self.progress() >= 0.5;
        } else {
            self.checked = !self.checked;
        }
        self.knob.set_target(if self.checked { 1.0 } else { 0.0 });
        self.checked != old
    }

    /// 取消拖动 —— 不 commit，knob 回到当前 checked 对应位置。
    /// 用于「按下后指针移出轨道再释放」的取消语义（对齐 UWP 拖出取消行为）。
    pub fn cancel(&mut self) {
        self.drag = None;
        self.state = ControlState::Normal;
        self.knob.set_target(if self.checked { 1.0 } else { 0.0 });
    }

    /// 渲染到宿主 `rect`。
    /// 布局：Header（可选，顶部一行）→ Track（左）+ State text（右）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let body = theme.typography.body;
        let disabled = self.state == ControlState::Disabled;
        let a = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };
        let _ = engine; // 现阶段不需要度量文本（rect 由外层给定）

        // Header（顶部一行）
        if !self.header.is_empty() {
            let fg = colors.on_surface.with_alpha(colors.on_surface.a * a);
            let header_rect = Rect::new(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                body.line_height,
            );
            scene.text(
                self.header.clone(),
                header_rect,
                fg,
                body,
                TextAlign::Left,
            );
        }

        // Track（底 / 描边）
        let track_rect = self.track_rect(rect, theme);
        let (track_fill, track_stroke, knob_color) = self.compute_colors(theme, a);
        if track_fill.a > 0.0 {
            scene.fill_rounded_rect(track_fill, track_rect, self.shape.corner());
        }
        if track_stroke.a > 0.0 {
            scene.stroke_rounded_rect(track_stroke, track_rect, 2.0, self.shape.corner());
        }

        // Knob
        let knob_rect = self.knob_rect(rect, theme);
        scene.fill_rounded_rect(knob_color, knob_rect, self.shape.knob_corner());

        // State text（Track 右侧）
        let state_text = if self.checked {
            &self.on_text
        } else {
            &self.off_text
        };
        if !state_text.is_empty() {
            let fg = colors.on_surface.with_alpha(colors.on_surface.a * a);
            let text_x = track_rect.right() + 12.0;
            let text_y = track_rect.origin.y + (track_rect.size.height - body.line_height) / 2.0;
            let right_avail = (rect.right() - text_x).max(0.0);
            let text_rect = Rect::new(text_x, text_y, right_avail, body.line_height);
            scene.text(state_text.clone(), text_rect, fg, body, TextAlign::Left);
        }
    }

    /// 计算三色 —— (track_fill, track_stroke, knob_color)。
    /// 参 Lumia 950：ON=accent 实心；OFF=透明+白描边；Pressed=中灰实心。
    fn compute_colors(&self, theme: &MetroTheme, alpha: f32) -> (Color, Color, Color) {
        let colors = &theme.colors;
        // 只有真实"按下 + 拖动"（moved=true）视觉才走 pressed 灰调；
        // 仅按住未拖动时保持原色（避免点动闪灰）。
        let is_pressed = matches!(self.state, ControlState::Pressed)
            && self.drag.map(|d| d.moved).unwrap_or(false);
        let is_hovered = matches!(self.state, ControlState::Hovered);

        let knob = Color::WHITE.with_alpha(alpha);

        let (fill, stroke) = match (self.checked, is_pressed) {
            (true, false) => {
                // ON：accent 实心
                let base = if is_hovered {
                    colors.primary.lerp(Color::WHITE, 0.15)
                } else {
                    colors.primary
                };
                (base.with_alpha(base.a * alpha), Color::TRANSPARENT)
            }
            (true, true) => {
                // ON 按下：dim（accent → 中灰调，Lumia 观感）
                let base = colors.primary.lerp(colors.on_surface_variant, 0.55);
                (base.with_alpha(base.a * alpha), Color::TRANSPARENT)
            }
            (false, false) => {
                // OFF：透明 + 描边（悬停加深）
                let s = if is_hovered {
                    colors.on_surface
                } else {
                    colors.on_surface_variant
                };
                (Color::TRANSPARENT, s.with_alpha(s.a * alpha))
            }
            (false, true) => {
                // OFF 按下：中灰实心（Lumia 观感）
                let base = colors.on_surface_variant;
                (base.with_alpha(base.a * alpha), Color::TRANSPARENT)
            }
        };
        (fill, stroke, knob)
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
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    #[test]
    fn capsule_dimensions_match_uwp_v1_spec() {
        // 数据源：microsoft-ui-xaml winui2/main ToggleSwitch_themeresources_v1.xaml
        // Track 40×20；Knob 10×10 Ellipse；margin=(20−10)/2=5；travel=40−10−10=20
        let s = MetroSwitch::new();
        assert_eq!(s.shape, SwitchShape::Capsule);
        assert_eq!(s.shape.track_size(), (40.0, 20.0));
        assert_eq!(s.shape.knob_size(), (10.0, 10.0));
        assert_eq!(s.shape.knob_margin(), 5.0);
        assert!((s.travel() - 20.0).abs() < 0.001);
    }

    #[test]
    fn square_dimensions_match_win8_style() {
        // 数据源：Windows 8 Modern UI ToggleSwitch 真机截图（11~2.jpg）
        // 同 40×20 底盘；knob = 10×20 瘦长矩形；margin=0；travel=30
        let s = MetroSwitch::new().with_shape(SwitchShape::Square);
        assert_eq!(s.shape.track_size(), (40.0, 20.0));
        assert_eq!(s.shape.knob_size(), (10.0, 20.0));
        assert_eq!(s.shape.knob_margin(), 0.0);
        assert!((s.travel() - 30.0).abs() < 0.001);
    }

    #[test]
    fn square_renders_with_zero_radius() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut s = MetroSwitch::new().with_shape(SwitchShape::Square);
        s.set_checked(true);
        s.update(1.0);
        let mut scene = Scene::default();
        s.render(&theme, &engine, Rect::new(0.0, 0.0, 100.0, 40.0), &mut scene);
        // Square: 所有 FillRect 的 corner_radius = 0（track + knob）
        for c in &scene.commands {
            if let SceneCommand::FillRect { corner_radius, .. } = c {
                assert_eq!(*corner_radius, 0.0, "Square 变体所有 fill 应为 0 圆角");
            }
        }
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
        assert!((s.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn toggle_is_interruptible() {
        let mut s = MetroSwitch::new();
        s.set_checked(true);
        s.update(0.05);
        let mid = s.progress();
        assert!(mid > 0.0 && mid < 1.0);
        s.set_checked(false);
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        assert!((s.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn tap_toggles_via_press_release() {
        let mut s = MetroSwitch::new();
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        let track = s.track_rect(rect, &theme);
        s.press(
            rect,
            &theme,
            Point::new(track.origin.x + 5.0, track.origin.y + 10.0),
        );
        assert!(s.drag.is_some());
        assert_eq!(s.state, ControlState::Pressed);
        let changed = s.release();
        assert!(changed);
        assert!(s.checked);
        assert!(s.drag.is_none());
        assert_eq!(s.state, ControlState::Normal);
    }

    #[test]
    fn drag_beyond_threshold_snaps_to_end() {
        let mut s = MetroSwitch::new(); // off
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        let track = s.track_rect(rect, &theme);
        s.press(
            rect,
            &theme,
            Point::new(track.origin.x + 5.0, track.origin.y + 10.0),
        );
        // 拖到轨道 60% —— 应最终吸到 on（≥50%）
        s.drag_to(Point::new(
            track.origin.x + track.size.width * 0.6,
            track.origin.y + 10.0,
        ));
        let changed = s.release();
        assert!(changed);
        assert!(s.checked, "拖到过半应吸到 on");
    }

    #[test]
    fn drag_below_threshold_is_treated_as_tap() {
        let mut s = MetroSwitch::new(); // off
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        let track = s.track_rect(rect, &theme);
        s.press(
            rect,
            &theme,
            Point::new(track.origin.x + 5.0, track.origin.y + 10.0),
        );
        // 1.5px 位移 < 3px 阈值 → 视为点动
        s.drag_to(Point::new(
            track.origin.x + 6.5,
            track.origin.y + 10.0,
        ));
        let changed = s.release();
        assert!(changed);
        assert!(s.checked, "微小位移视为点动，翻转");
    }

    #[test]
    fn press_outside_track_ignored() {
        let mut s = MetroSwitch::new();
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        s.press(rect, &theme, Point::new(1000.0, 1000.0));
        assert!(s.drag.is_none());
    }

    #[test]
    fn cancel_reverts_knob_and_clears_drag() {
        let mut s = MetroSwitch::new(); // off
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        let track = s.track_rect(rect, &theme);
        s.press(
            rect,
            &theme,
            Point::new(track.origin.x + 5.0, track.origin.y + 10.0),
        );
        s.drag_to(Point::new(
            track.origin.x + track.size.width * 0.7,
            track.origin.y + 10.0,
        ));
        s.cancel();
        assert!(s.drag.is_none());
        assert!(!s.checked, "取消后状态不变");
        // 动画目标应回到 0（off）
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        assert!((s.progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn renders_header_track_knob_state_text() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut s = MetroSwitch::with_header("Wi-Fi");
        s.set_checked(true);
        s.update(1.0);
        let mut scene = Scene::default();
        s.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 200.0, 60.0),
            &mut scene,
        );
        // header + state text = 2 texts；track fill + knob = 2 fills（ON 无描边）
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 2, "header + state text");
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 2, "ON = track fill + knob");
        assert!(
            scene
                .commands
                .iter()
                .all(|c| !matches!(c, SceneCommand::StrokeRect { .. })),
            "ON 无描边"
        );
    }

    #[test]
    fn off_state_renders_stroke_not_fill() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let s = MetroSwitch::new(); // off
        let mut scene = Scene::default();
        s.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 100.0, 40.0),
            &mut scene,
        );
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::StrokeRect { .. })),
            "OFF 应有 track 描边"
        );
        // 仅 knob 是 fill（1 个），track 无 fill
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 1, "OFF 只有 knob 是 fill");
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
        s.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 100.0, 40.0),
            &mut scene,
        );
        let Some(SceneCommand::FillRect { color, .. }) = scene.commands.first() else {
            panic!("首命令应为轨道填充");
        };
        assert!(color.a < 1.0, "禁用降 alpha，实际 a={}", color.a);
    }
}
