// MetroSlider —— 连续数值输入（音量 / 亮度 / 色温）。参 CONTROL_SPEC §43。
//
// 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/Slider_themeresources_v1.xaml`
// （闭源 B 类，v1 = Metro 时代）。尺寸：轨道 2px、拇指 20×20、上下留白 15px、
// 水平整体 MinHeight 32、Header Margin 0,0,0,4。颜色：轨道底 surface_variant、
// 填充段 + 拇指 primary、Pressed 拇指 press_tint 叠加、Disabled 前景 0.38 alpha。
// 交互：点击轨道即跳值（UWP 默认）、press 后 drag_to 连续更新、set_value 夹紧 [min,max]。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{CornerRadius, MetroTheme, Point, Rect, Size};

use crate::state::ControlState;

// ── 规格常量（CONTROL_SPEC §43）───────────────────────────────────────────

/// 轨道高（SliderTrackThemeHeight）。
pub const SLIDER_TRACK_H: f32 = 2.0;
/// 拇指边长（SliderHorizontalThumbWidth/Height）。
pub const SLIDER_THUMB: f32 = 20.0;
/// 轨道上下留白（SliderPreContentMargin / SliderPostContentMargin）。
pub const SLIDER_TRACK_MARGIN: f32 = 15.0;
/// 水平整体最小高（SliderHorizontalHeight）。
pub const SLIDER_MIN_H: f32 = 32.0;
/// Header 下边距（SliderHeaderThemeMargin）。
pub const SLIDER_HEADER_MARGIN: f32 = 4.0;
/// 默认最小宽（对齐 NumberBox MinW 120）。
pub const SLIDER_MIN_W: f32 = 120.0;
/// Header 字号。
pub const SLIDER_HEADER_SIZE: f32 = 12.0;

/// MetroSlider —— 连续数值输入。参 CONTROL_SPEC §43。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSlider {
    /// 当前值（夹紧 [min, max]）。
    pub value: f64,
    pub min: f64,
    pub max: f64,
    /// 步进（可选；None = 连续）。拖动 / 点击吸附到步进。
    pub step: Option<f64>,
    /// 顶部标题（可选）—— UWP `Header`。
    pub header: String,
    pub state: ControlState,
    /// 拖动中（按下后连续更新）。
    dragging: bool,
}

impl Default for MetroSlider {
    fn default() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 100.0,
            step: None,
            header: String::new(),
            state: ControlState::Normal,
            dragging: false,
        }
    }
}

impl MetroSlider {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带 Header 构造（UWP `<Slider Header="…" />`）。
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.header = header.into();
        self
    }

    /// 设置值域（夹紧当前值）。
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = min;
        self.max = max;
        self.value = self.value.clamp(min, max);
        self
    }

    /// 设置步进。
    pub fn with_step(mut self, step: f64) -> Self {
        self.step = if step > 0.0 { Some(step) } else { None };
        self
    }

    /// 值 → 吸附到步进 + 夹紧 [min, max]。
    fn snap(&self, v: f64) -> f64 {
        let v = v.clamp(self.min, self.max);
        match self.step {
            Some(s) => {
                let steps = ((v - self.min) / s).round();
                (self.min + steps * s).clamp(self.min, self.max)
            }
            None => v,
        }
    }

    /// 设置值（夹紧 + 步进吸附）。
    pub fn set_value(&mut self, v: f64) {
        self.value = self.snap(v);
    }

    /// 值在 [0,1] 归一化（绘制 / 几何用）。
    pub fn fraction(&self) -> f32 {
        let span = (self.max - self.min).max(1e-9);
        ((self.value - self.min) / span) as f32
    }

    /// 固有尺寸（Header + 32 高）。Header 高度固定，无需引擎量测。
    pub fn measure(&self, width: f32) -> Size {
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            SLIDER_HEADER_SIZE + SLIDER_HEADER_MARGIN
        };
        Size::new(
            width.max(SLIDER_MIN_W),
            header_h + SLIDER_MIN_H,
        )
    }

    // ── 几何 ──────────────────────────────────────────────────────

    /// 轨道矩形（宿主 rect 内左右各 15px，y 向中心 2px 轨道）。
    fn track_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.origin.x + SLIDER_TRACK_MARGIN,
            rect.origin.y + rect.size.height / 2.0 - SLIDER_TRACK_H / 2.0,
            (rect.size.width - 2.0 * SLIDER_TRACK_MARGIN).max(1.0),
            SLIDER_TRACK_H,
        )
    }

    /// 填充段矩形（0..value）。
    fn fill_rect(&self, rect: Rect) -> Rect {
        let track = self.track_rect(rect);
        Rect::new(
            track.origin.x,
            track.origin.y,
            track.size.width * self.fraction(),
            track.size.height,
        )
    }

    /// 拇指矩形（20×20，中心对轨道 value x）。
    fn thumb_rect(&self, rect: Rect) -> Rect {
        let track = self.track_rect(rect);
        let cx = track.origin.x + track.size.width * self.fraction();
        Rect::new(
            cx - SLIDER_THUMB / 2.0,
            rect.origin.y + rect.size.height / 2.0 - SLIDER_THUMB / 2.0,
            SLIDER_THUMB,
            SLIDER_THUMB,
        )
    }

    /// 命中区（轨道矩形上下扩至 32 高总命中；x 向 = 轨道全长）。
    fn hit_rect(&self, rect: Rect) -> Rect {
        let track = self.track_rect(rect);
        let half = (SLIDER_MIN_H - SLIDER_TRACK_H) / 2.0;
        Rect::new(
            track.origin.x,
            track.origin.y - half,
            track.size.width,
            track.size.height + 2.0 * half,
        )
    }

    /// 命中区是否包含点（宿主据此路由 hit_test）。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        self.hit_rect(rect).contains(pos)
    }

    /// 位置 → 值（步进吸附 + 夹紧）。
    fn value_from_pos(&self, rect: Rect, pos: Point) -> f64 {
        let track = self.track_rect(rect);
        let frac = ((pos.x - track.origin.x) / track.size.width).clamp(0.0, 1.0);
        self.snap(self.min + (self.max - self.min) * frac as f64)
    }

    // ── 交互 ──────────────────────────────────────────────────────

    /// 悬停（命中区外清除高亮）。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.state = if self.hit_test(rect, pos) {
            ControlState::Hovered
        } else {
            ControlState::Normal
        };
    }

    /// 按下：记录拖动并置值到点击位置。返回新值（None = 未变化）。
    pub fn press(&mut self, rect: Rect, pos: Point) -> Option<f64> {
        if !self.hit_test(rect, pos) {
            return None;
        }
        self.dragging = true;
        self.state = ControlState::Pressed;
        let old = self.value;
        self.value = self.value_from_pos(rect, pos);
        if (self.value - old).abs() > 1e-9 {
            Some(self.value)
        } else {
            None
        }
    }

    /// 拖动：更新值（拖动中）。返回新值。
    pub fn drag_to(&mut self, rect: Rect, pos: Point) -> Option<f64> {
        if !self.dragging {
            return None;
        }
        let old = self.value;
        self.value = self.value_from_pos(rect, pos);
        if (self.value - old).abs() > 1e-9 {
            Some(self.value)
        } else {
            None
        }
    }

    /// 释放拖动。
    pub fn release(&mut self) {
        self.dragging = false;
        self.state = ControlState::Normal;
    }

    // ── 渲染 ──────────────────────────────────────────────────────

    /// 渲染：Header（可选）→ 轨道底 → 填充段 → 拇指。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let disabled = self.state == ControlState::Disabled;
        let alpha = if disabled { 0.38 } else { 1.0 };

        // Header（可选）
        if !self.header.is_empty() {
            let style = TextStyle::new(
                SLIDER_HEADER_SIZE,
                SLIDER_HEADER_SIZE + 4.0,
                kanesumi_core::FontWeight::Normal,
            );
            scene.text(
                self.header.clone(),
                Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    style.line_height,
                ),
                colors.on_surface.with_alpha(alpha),
                style,
                TextAlign::Left,
            );
        }

        let track = self.track_rect(rect);
        // 轨道底
        scene.fill_rect(
            colors.surface_variant.with_alpha(alpha),
            track,
        );
        // 填充段（0..value）
        let fill = self.fill_rect(rect);
        if fill.size.width > 0.01 {
            scene.fill_rect(colors.primary.with_alpha(alpha), fill);
        }
        // 拇指
        let thumb = self.thumb_rect(rect);
        let thumb_color = if self.state == ControlState::Pressed {
            // SliderThumbBackgroundPressed = accentDark1；Kanesumi 用 press_tint 向暗侧压。
            colors.primary.lerp(colors.press_tint, 0.5).with_alpha(alpha)
        } else {
            colors.primary.with_alpha(alpha)
        };
        scene.fill_rounded_rect(thumb_color, thumb, CornerRadius::Capsule);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> MetroSlider {
        MetroSlider::new()
            .with_range(0.0, 100.0)
    }

    #[test]
    fn defaults() {
        let s = MetroSlider::new();
        assert_eq!(s.value, 0.0);
        assert_eq!((s.min, s.max), (0.0, 100.0));
        assert_eq!(s.step, None);
        assert!(s.header.is_empty());
    }

    #[test]
    fn set_value_clamps() {
        let mut s = app();
        s.set_value(150.0);
        assert_eq!(s.value, 100.0);
        s.set_value(-20.0);
        assert_eq!(s.value, 0.0);
    }

    #[test]
    fn set_value_snaps_to_step() {
        let mut s = app().with_step(5.0);
        s.set_value(37.0);
        assert_eq!(s.value, 35.0, "37 → 吸附到 35（5 的倍数）");
        s.set_value(38.0);
        assert_eq!(s.value, 40.0, "38 → 吸附到 40");
    }

    #[test]
    fn fraction_maps_range() {
        let mut s = app();
        s.set_value(50.0);
        assert!((s.fraction() - 0.5).abs() < 1e-6);
        let mut s2 = app().with_range(-10.0, 10.0);
        s2.set_value(0.0);
        assert!((s2.fraction() - 0.5).abs() < 1e-6);
    }

    #[test]
    fn measure_includes_header() {
        let s = MetroSlider::new();
        let m = s.measure(200.0);
        assert_eq!(m.height, SLIDER_MIN_H);
        let s2 = MetroSlider::new().with_header("音量");
        let m2 = s2.measure(200.0);
        assert!(m2.height > m.height, "Header 占用额外高度");
        assert!(m2.width >= SLIDER_MIN_W);
    }

    #[test]
    fn hit_test_centers_on_track() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let s = MetroSlider::new();
        assert!(s.hit_test(rect, Point::new(100.0, 16.0)));
        assert!(s.hit_test(rect, Point::new(15.0, 16.0)), "轨道左缘命中");
        assert!(s.hit_test(rect, Point::new(184.9, 16.0)), "轨道右缘命中");
        assert!(!s.hit_test(rect, Point::new(0.0, 16.0)), "margin 内不命中");
        assert!(!s.hit_test(rect, Point::new(100.0, 32.0)), "32 高命中区外不命中");
    }

    #[test]
    fn press_sets_value_to_pos() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        // 轨道 = x∈[15,185]，200 宽 → value = (x-15)/170
        let v = s.press(rect, Point::new(15.0 + 85.0, 16.0)).unwrap();
        assert!((v - 50.0).abs() < 1.0, "中点 → 约 50，got {v}");
    }

    #[test]
    fn press_outside_hit_returns_none() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        assert_eq!(s.press(rect, Point::new(0.0, 0.0)), None);
        assert!(!s.dragging);
    }

    #[test]
    fn drag_to_continues_after_press() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        s.press(rect, Point::new(30.0, 16.0));
        let v = s.drag_to(rect, Point::new(150.0, 16.0)).unwrap();
        assert!(v > 70.0, "拖动到右侧 → 值增大，got {v}");
        // 未 press 时 drag 无效
        let mut s2 = app();
        assert_eq!(s2.drag_to(rect, Point::new(150.0, 16.0)), None);
    }

    #[test]
    fn release_clears_dragging() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        s.press(rect, Point::new(100.0, 16.0));
        s.release();
        assert!(!s.dragging);
        assert_eq!(s.state, ControlState::Normal);
    }

    #[test]
    fn disabled_state_lowers_render_alpha() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let theme = MetroTheme::ether_dark();
        let engine = TextEngine::load(kanesumi_gallery_font()).expect("字体");
        let mut s = app();
        s.set_value(50.0);
        s.state = ControlState::Disabled;
        let scene = render(&s, &theme, &engine, rect);
        // 三元素（轨道底 + 填充段 + 拇指）均带 0.38 alpha
        let fills: Vec<&kanesumi_canvas::SceneCommand> = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
            .collect();
        assert_eq!(fills.len(), 3, "轨道底 + 填充段 + 拇指");
        for c in fills {
            if let kanesumi_canvas::SceneCommand::FillRect { color, .. } = c {
                assert!(
                    (color.a - 0.38).abs() < 0.01,
                    "禁用态前景 0.38，got {}",
                    color.a
                );
            }
        }
    }

    #[test]
    fn render_emits_track_fill_thumb() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let theme = MetroTheme::ether_dark();
        let engine = TextEngine::load(kanesumi_gallery_font()).expect("字体");
        let mut s = app();
        s.set_value(50.0);
        let scene = render(&s, &theme, &engine, rect);
        // FillRect 单变体：轨道底 + 填充段 + 拇指共 3 条
        assert_eq!(
            scene
                .commands
                .iter()
                .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
                .count(),
            3,
            "轨道底 + 填充段 + 拇指"
        );
        // 拇指 Capsule 圆角（corner_radius > 0）
        assert!(
            scene.commands.iter().any(|c| {
                matches!(
                    c,
                    kanesumi_canvas::SceneCommand::FillRect { corner_radius, .. }
                        if *corner_radius > 0.0
                )
            }),
            "拇指为 Capsule 圆角"
        );
    }

    #[test]
    fn header_renders_when_set() {
        let rect = Rect::new(0.0, 0.0, 200.0, 48.0);
        let theme = MetroTheme::ether_dark();
        let engine = TextEngine::load(kanesumi_gallery_font()).expect("字体");
        let s = MetroSlider::new().with_header("音量");
        let scene = render(&s, &theme, &engine, rect);
        assert!(
            scene.commands.iter().any(|c| matches!(
                c,
                kanesumi_canvas::SceneCommand::Text { content, .. } if content == "音量"
            )),
            "Header 文本应入场景"
        );
    }

    /// 测试用字体（KANESUMI_TEST_FONT → 系统顺序，参 harness find_font）。
    fn kanesumi_gallery_font() -> std::path::PathBuf {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return p;
            }
        }
        for p in [
            "/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return p;
            }
        }
        panic!("未找到测试字体（设 KANESUMI_TEST_FONT）");
    }

    fn render(
        s: &MetroSlider,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
    ) -> Scene {
        let mut scene = Scene::default();
        s.render(theme, engine, rect, &mut scene);
        scene
    }
}
