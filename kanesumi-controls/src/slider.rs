// MetroSlider —— 连续数值输入（音量 / 亮度 / 色温）。参 CONTROL_SPEC §43。
//
// 数据源：Windows SDK `Generic/generic.xaml`（Slider 模板 + ThemeResources，一手源）。
// 尺寸：轨道 2、拇指 **8×24 直角**（`HorizontalThumb` Width 8 / Height 24；正典 §Ⅲ-1
// 不取 SDK 的 CornerRadius 4）、水平整体 32、Header Margin 0,0,0,4。轨道横向占满宿主宽，
// 拇指中心行程左右各让半个拇指（拇指永不越出控件）。颜色：轨道底 = 前景 40%
// （SliderTrackFill = BaseMediumLow），悬停 60%（BaseMedium）；填充段与静止拇指 = primary；
// 悬停拇指 = 前景（ChromeAltLow）；按下拇指 = 前景与表面的中灰（ChromeHigh 近似）；
// Disabled 整体 0.38 alpha。交互：点击轨道即跳值（UWP 默认）、press 后 drag_to 连续更新。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::MetroTypography;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::state::ControlState;

// ── 规格常量（CONTROL_SPEC §43）───────────────────────────────────────────

/// 轨道高（SliderTrackThemeHeight）。
pub const SLIDER_TRACK_H: f32 = 2.0;
/// 拇指宽（generic.xaml `HorizontalThumb` Width）。
pub const SLIDER_THUMB_W: f32 = 8.0;
/// 拇指高（generic.xaml `HorizontalThumb` Height）。
pub const SLIDER_THUMB_H: f32 = 24.0;
/// 轨道底不透明度（SliderTrackFill = BaseMediumLow 0x66）。
pub const SLIDER_TRACK_ALPHA: f32 = 0.4;
/// 悬停轨道底不透明度（SliderTrackFillPointerOver = BaseMedium 0x99）。
pub const SLIDER_TRACK_HOVER_ALPHA: f32 = 0.6;
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

    /// 轨道矩形（横向占满宿主宽，y 向贴底 32 区的中线）。有 Header 时滑杆区在 Header 之下。
    fn track_rect(&self, rect: Rect) -> Rect {
        let cy = rect.origin.y + rect.size.height - SLIDER_MIN_H / 2.0;
        Rect::new(
            rect.origin.x,
            cy - SLIDER_TRACK_H / 2.0,
            rect.size.width.max(1.0),
            SLIDER_TRACK_H,
        )
    }

    /// 拇指中心的横向行程 (起点, 长度)：左右各让半个拇指，0 / 100 时拇指恰贴控件边。
    fn travel(&self, rect: Rect) -> (f32, f32) {
        let track = self.track_rect(rect);
        let half = SLIDER_THUMB_W / 2.0;
        (track.origin.x + half, (track.size.width - SLIDER_THUMB_W).max(1.0))
    }

    /// 拇指中心 x。
    fn thumb_x(&self, rect: Rect) -> f32 {
        let (x0, len) = self.travel(rect);
        x0 + len * self.fraction()
    }

    /// 填充段矩形（轨道左端 → 拇指中心）。
    fn fill_rect(&self, rect: Rect) -> Rect {
        let track = self.track_rect(rect);
        Rect::new(
            track.origin.x,
            track.origin.y,
            (self.thumb_x(rect) - track.origin.x).max(0.0),
            track.size.height,
        )
    }

    /// 拇指矩形（8×24 直角，中心对 value）。
    fn thumb_rect(&self, rect: Rect) -> Rect {
        let track = self.track_rect(rect);
        let cy = track.origin.y + SLIDER_TRACK_H / 2.0;
        Rect::new(
            self.thumb_x(rect) - SLIDER_THUMB_W / 2.0,
            cy - SLIDER_THUMB_H / 2.0,
            SLIDER_THUMB_W,
            SLIDER_THUMB_H,
        )
    }

    /// 命中区（轨道全宽 × 以轨道为中线的 32 高）。
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
        let (x0, len) = self.travel(rect);
        let frac = ((pos.x - x0) / len).clamp(0.0, 1.0);
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
            let style = MetroTypography::metro().body_small;
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

        let fg = colors.on_surface;
        let hovered = self.state == ControlState::Hovered;
        let pressed = self.state == ControlState::Pressed;
        // 轨道底：前景 40%，悬停 60%（按下回到 40%，与 SDK 一致）。
        let track_alpha = if hovered {
            SLIDER_TRACK_HOVER_ALPHA
        } else {
            SLIDER_TRACK_ALPHA
        };
        scene.fill_rect(fg.with_alpha(track_alpha * alpha), self.track_rect(rect));
        // 填充段（轨道左端 → 拇指中心）
        let fill = self.fill_rect(rect);
        if fill.size.width > 0.01 {
            scene.fill_rect(colors.primary.with_alpha(alpha), fill);
        }
        // 拇指：静止 primary；悬停 = 前景（ChromeAltLow）；按下 = 前景与表面的中灰（ChromeHigh）。
        let thumb_color = if pressed {
            fg.lerp(colors.surface, 0.5)
        } else if hovered {
            fg
        } else {
            colors.primary
        };
        scene.fill_rect(thumb_color.with_alpha(alpha), self.thumb_rect(rect));
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）──────────────────
//
// 旧 API（`measure(width)` / `press` / `drag_to` / `release` / `render` / `hit_test`）原样
// 保留给未迁移的 App；`Widget` 实现把指针与方向键翻译成值变更动作。拖动期间每次移动都发
// `SliderValueChanged`（UWP 语义，参规范 §4）。命中与焦点交给框架，命中区沿用旧轨道区。

/// 元素树动作：滑块值被用户改变（点击 / 拖动 / 方向键）。携带新值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliderValueChanged(pub f64);

impl kanesumi_element::Widget for MetroSlider {
    /// 固有尺寸：宽度取 `available.width`（无界时默认 200），高 = Header + 32（CONTROL_SPEC §43）。
    fn measure(&mut self, _ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        let width = if available.width.is_finite() {
            available.width
        } else {
            200.0
        };
        MetroSlider::measure(self, width)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let saved = self.state;
        self.state = crate::state::control_state(ctx.state());
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        self.state = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let rect = ctx.rect();
                if let Some(v) = self.press(rect, *pos) {
                    ctx.emit(SliderValueChanged(v));
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::PointerMove { pos } => {
                if let Some(v) = self.drag_to(ctx.rect(), *pos) {
                    ctx.emit(SliderValueChanged(v));
                    ctx.invalidate_paint();
                }
            }
            Event::PointerUp {
                button: PointerButton::Left,
                ..
            } => {
                self.release();
                ctx.invalidate_paint();
            }
            Event::KeyDown { key, .. } => {
                // 无步长时按范围 1% 步进；范围为 0 时夹紧后不变，不发动作。
                let step = self
                    .step
                    .unwrap_or(((self.max - self.min).abs() * 0.01).max(1e-9));
                let target = match key {
                    Key::Left | Key::Down => self.value - step,
                    Key::Right | Key::Up => self.value + step,
                    _ => return,
                };
                let old = self.value;
                self.set_value(target);
                if (self.value - old).abs() > 1e-9 {
                    ctx.emit(SliderValueChanged(self.value));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        MetroSlider::hit_test(self, rect, pos)
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: self.header.clone(),
            value: Some(format!("{}", self.value)),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, PointerButton, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(320.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroSlider::new().with_header("音量"),
            LayoutProps {
                width: Some(200.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_sets_value_and_reports() {
        let (mut h, id) = harness();
        h.click(id);
        let actions = h.take::<SliderValueChanged>();
        assert_eq!(actions.len(), 1);
        let v = actions[0].1;
        assert!((v.0 - 50.0).abs() < 2.0, "中点应约 50，实际 {}", v.0);
        assert!((h.tree.get::<MetroSlider>(id).unwrap().value - v.0).abs() < 1e-9);
    }

    #[test]
    fn drag_updates_value_continuously() {
        let (mut h, id) = harness();
        let r = h.rect(id);
        let y = r.center().y;
        h.press_at(Point::new(r.origin.x + 20.0, y), PointerButton::Left);
        // 拖过拇指行程右端（右缘内 2pt，行程止于右缘内 4pt）→ 夹紧到 100。
        h.move_to(Point::new(r.right() - 2.0, y));
        h.release_at(Point::new(r.right() - 2.0, y), PointerButton::Left);
        h.frame();
        let vals: Vec<f64> = h
            .take::<SliderValueChanged>()
            .into_iter()
            .map(|(_, v)| v.0)
            .collect();
        assert!(vals.len() >= 2, "按下 + 拖动各发一次，实际 {}", vals.len());
        assert!((vals.last().unwrap() - 100.0).abs() < 5.0, "拖到最右应接近 100");
    }

    #[test]
    fn arrow_keys_step_one_percent_and_tab_focuses() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Right);
        let v = h.tree.get::<MetroSlider>(id).unwrap().value;
        assert!((v - 1.0).abs() < 1e-6, "无步长时按范围 1% 步进，实际 {v}");
        h.take::<SliderValueChanged>();
        h.key(Key::Left);
        assert!(h.tree.get::<MetroSlider>(id).unwrap().value.abs() < 1e-9);
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (mut h, id) = harness();
        assert!(h.rect(id).size.width >= SLIDER_MIN_W);
        h.move_to(h.center(id));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click(id);
        assert!(h.take::<SliderValueChanged>().is_empty());
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
        assert!(s.hit_test(rect, Point::new(0.0, 16.0)), "轨道占满宽：左缘命中");
        assert!(s.hit_test(rect, Point::new(199.9, 16.0)), "轨道占满宽：右缘命中");
        assert!(!s.hit_test(rect, Point::new(200.5, 16.0)), "控件外不命中");
        assert!(!s.hit_test(rect, Point::new(100.0, 32.0)), "32 高命中区外不命中");
    }

    #[test]
    fn thumb_is_8x24_square_and_stays_inside() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        for v in [0.0, 50.0, 100.0] {
            s.set_value(v);
            let t = s.thumb_rect(rect);
            assert_eq!((t.size.width, t.size.height), (SLIDER_THUMB_W, SLIDER_THUMB_H));
            assert!(t.origin.x >= 0.0 && t.right() <= 200.0, "v={v} 拇指越出控件：{t:?}");
        }
        s.set_value(0.0);
        assert_eq!(s.thumb_rect(rect).origin.x, 0.0, "0 时拇指贴左缘");
        s.set_value(100.0);
        assert_eq!(s.thumb_rect(rect).right(), 200.0, "100 时拇指贴右缘");
    }

    #[test]
    fn track_sits_in_slider_band_below_header() {
        let rect = Rect::new(0.0, 0.0, 200.0, 48.0);
        let s = MetroSlider::new().with_header("音量");
        let t = s.track_rect(rect);
        assert_eq!(t.origin.y + SLIDER_TRACK_H / 2.0, 32.0, "轨道中线 = 底部 32 区中线");
    }

    #[test]
    fn press_sets_value_to_pos() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        // 拇指中心行程 = x∈[4,196]，200 宽 → value = (x-4)/192
        let v = s.press(rect, Point::new(4.0 + 96.0, 16.0)).unwrap();
        assert!((v - 50.0).abs() < 1.0, "中点 → 约 50，got {v}");
    }

    #[test]
    fn press_outside_hit_returns_none() {
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        let mut s = app();
        assert_eq!(s.press(rect, Point::new(100.0, -1.0)), None);
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
        // 三元素（轨道底 + 填充段 + 拇指）整体乘 0.38：轨道 0.4×0.38，其余 0.38。
        let alphas: Vec<f32> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                kanesumi_canvas::SceneCommand::FillRect { color, .. } => Some(color.a),
                _ => None,
            })
            .collect();
        assert_eq!(alphas.len(), 3, "轨道底 + 填充段 + 拇指");
        assert!((alphas[0] - SLIDER_TRACK_ALPHA * 0.38).abs() < 0.01, "轨道 {}", alphas[0]);
        for a in &alphas[1..] {
            assert!((a - 0.38).abs() < 0.01, "禁用态前景 0.38，got {a}");
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
        // 正典 §Ⅲ-1：无圆角——三条都是直角。
        assert!(
            scene.commands.iter().all(|c| !matches!(
                c,
                kanesumi_canvas::SceneCommand::FillRect { corner_radius, .. }
                    if *corner_radius > 0.0
            )),
            "滑杆不得有圆角"
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
            "C:/Windows/Fonts/segoeui.ttf",
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
