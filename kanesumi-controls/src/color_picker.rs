// MetroColorPicker —— 颜色选择器。参 CONTROL_SPEC §29。
//
// 移植自 microsoft-ui-xaml/dev/ColorPicker（ColorPicker.cpp + ColorPicker.xaml）：
// - RGB/A 四滑轨（0..255）：实心轨道 + 填充段 + 10×10 拇指；
// - 预览色块 44 高 + 2px 边框；Hex 文本；
// - Spectrum 2D 渐变 → Kanesumi 阶梯 hue 色带近似（铁律 6 纯色无渐变）。
// 仅垂直朝向（Min 312 / Max 392）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, CornerRadius, FontWeight, MetroTheme, Point, Rect};

/// 滑轨拇指边长（ColorPickerSliderInnerThumb = 10）。
pub const COLOR_THUMB: f32 = 10.0;
/// 滑轨高（含拇指留白）。
pub const COLOR_SLIDER_H: f32 = 24.0;
/// 预览块高（Spectrum 隐藏时 44）。
pub const COLOR_PREVIEW_H: f32 = 44.0;
/// 轨道圆角（ColorPickerSliderCornerRadius = 6，Kanesumi 取 Slight=2 适配直角铁律）。
const TRACK_CORNER: CornerRadius = CornerRadius::Slight;

/// 通道。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    Red,
    Green,
    Blue,
    Alpha,
}

impl ColorChannel {
    /// 当前值（0..255）。
    fn value(self, c: &Color) -> f32 {
        match self {
            ColorChannel::Red => c.r * 255.0,
            ColorChannel::Green => c.g * 255.0,
            ColorChannel::Blue => c.b * 255.0,
            ColorChannel::Alpha => c.a * 255.0,
        }
    }

    fn set(self, c: &mut Color, v: f32) {
        let v = v.clamp(0.0, 255.0) / 255.0;
        match self {
            ColorChannel::Red => c.r = v,
            ColorChannel::Green => c.g = v,
            ColorChannel::Blue => c.b = v,
            ColorChannel::Alpha => c.a = v,
        }
    }

    fn label(self) -> &'static str {
        match self {
            ColorChannel::Red => "R",
            ColorChannel::Green => "G",
            ColorChannel::Blue => "B",
            ColorChannel::Alpha => "A",
        }
    }
}

pub const ALL_CHANNELS: [ColorChannel; 4] = [
    ColorChannel::Red,
    ColorChannel::Green,
    ColorChannel::Blue,
    ColorChannel::Alpha,
];

/// MetroColorPicker —— 颜色选择器。参 CONTROL_SPEC §29。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroColorPicker {
    /// 当前颜色（RGBA 0..1）。
    pub color: Color,
    /// 显示 Spectrum 阶梯色带。
    pub show_spectrum: bool,
    /// 滑轨区高（含顶部 label）。
    pub slider_area_h: f32,
    /// hover 的通道。
    pub hovered_channel: Option<ColorChannel>,
    /// 拖动中的通道。
    pub dragging: Option<ColorChannel>,
}

impl Default for MetroColorPicker {
    fn default() -> Self {
        Self {
            color: Color::from_hex(0xE5_78_12),
            show_spectrum: true,
            slider_area_h: 28.0,
            hovered_channel: None,
            dragging: None,
        }
    }
}

impl MetroColorPicker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 固有尺寸（垂直朝向 Min 312 / Max 392）。
    pub fn measure(&self) -> kanesumi_core::Size {
        let w = 312.0;
        let content = 200.0 + ALL_CHANNELS.len() as f32 * (COLOR_SLIDER_H + 8.0) + COLOR_PREVIEW_H;
        let h = if self.show_spectrum {
            312.0_f32.max(content)
        } else {
            ALL_CHANNELS.len() as f32 * (COLOR_SLIDER_H + 8.0) + COLOR_PREVIEW_H + 16.0
        };
        kanesumi_core::Size::new(w, h)
    }

    /// Spectrum rect（顶部）。
    pub fn spectrum_rect(&self, rect: Rect) -> Option<Rect> {
        if !self.show_spectrum {
            return None;
        }
        let side = rect
            .size
            .width
            .min(rect.size.height - COLOR_SLIDER_H * 4.0 - COLOR_PREVIEW_H - 16.0);
        Some(Rect::new(rect.origin.x, rect.origin.y, side, side))
    }

    /// 第 k 个通道滑轨 rect（label 行 + 轨道）。
    pub fn slider_rect(&self, rect: Rect, k: usize) -> Rect {
        let spectrum_h = if self.show_spectrum {
            self.spectrum_rect(rect)
                .map(|r| r.size.height)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        Rect::new(
            rect.origin.x,
            rect.origin.y + spectrum_h + 8.0 + k as f32 * (COLOR_SLIDER_H + 8.0),
            rect.size.width,
            COLOR_SLIDER_H,
        )
    }

    /// 预览块 rect。
    pub fn preview_rect(&self, rect: Rect) -> Rect {
        let spectrum_h = if self.show_spectrum {
            self.spectrum_rect(rect)
                .map(|r| r.size.height)
                .unwrap_or(0.0)
        } else {
            0.0
        };
        let slider_top = rect.origin.y + spectrum_h + 8.0;
        Rect::new(
            rect.origin.x,
            slider_top + ALL_CHANNELS.len() as f32 * (COLOR_SLIDER_H + 8.0),
            rect.size.width,
            COLOR_PREVIEW_H,
        )
    }

    /// 命中通道：按 y 判断滑轨区。
    fn channel_at(&self, rect: Rect, pos: Point) -> Option<ColorChannel> {
        for (k, ch) in ALL_CHANNELS.iter().enumerate() {
            if self.slider_rect(rect, k).contains(pos) {
                return Some(*ch);
            }
        }
        None
    }

    /// 轨道内位置 → 通道值（0..255）。
    fn value_from_pos(&self, rect: Rect, ch: ColorChannel, pos: Point) -> f32 {
        let sr = self.slider_rect(rect, ALL_CHANNELS.iter().position(|c| *c == ch).unwrap());
        // 轨道 = label 右侧到 rect 右缘
        let track_x = sr.origin.x + 28.0;
        let track_w = (sr.right() - track_x).max(1.0);
        ((pos.x - track_x) / track_w * 255.0).clamp(0.0, 255.0)
    }

    /// 轨道几何（填充段 + 拇指）。
    fn track_geom(&self, rect: Rect, ch: ColorChannel) -> (Rect, f32) {
        let sr = self.slider_rect(rect, ALL_CHANNELS.iter().position(|c| *c == ch).unwrap());
        let track_x = sr.origin.x + 28.0;
        let track_w = (sr.right() - track_x).max(1.0);
        let track = Rect::new(
            track_x,
            sr.origin.y + (sr.size.height - 6.0) / 2.0,
            track_w,
            6.0,
        );
        let frac = ch.value(&self.color) / 255.0;
        (track, frac)
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.hovered_channel = self.channel_at(rect, pos);
    }

    /// 按下：记录拖动通道。
    pub fn press(&mut self, rect: Rect, pos: Point) -> bool {
        if let Some(ch) = self.channel_at(rect, pos) {
            self.dragging = Some(ch);
            self.apply(rect, ch, pos);
            return true;
        }
        false
    }

    /// 拖动：更新通道（拖动中）。
    pub fn drag_to(&mut self, rect: Rect, pos: Point) {
        if let Some(ch) = self.dragging {
            self.apply(rect, ch, pos);
        }
    }

    /// 释放拖动。
    pub fn release(&mut self) {
        self.dragging = None;
    }

    fn apply(&mut self, rect: Rect, ch: ColorChannel, pos: Point) {
        let v = self.value_from_pos(rect, ch, pos);
        let old = ch.value(&self.color);
        if (old - v).abs() > 0.5 {
            ch.set(&mut self.color, v);
        }
    }

    /// 综合点击：按下+释放（用于单点）。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> Option<Color> {
        let ch = self.channel_at(rect, pos)?;
        let old = self.color;
        self.apply(rect, ch, pos);
        if self.color != old {
            Some(self.color)
        } else {
            None
        }
    }

    /// Hex 文本。
    pub fn hex(&self) -> String {
        let r = (self.color.r * 255.0).round() as u8;
        let g = (self.color.g * 255.0).round() as u8;
        let b = (self.color.b * 255.0).round() as u8;
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    /// 渲染：Spectrum（阶梯带）→ 四滑轨 → 预览 + Hex。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = TextStyle::new(12.0, 16.0, FontWeight::Normal);

        // Spectrum 阶梯 hue 带（离散 12 色列）
        if let Some(sr) = self.spectrum_rect(rect) {
            let bands = 12;
            let w = sr.size.width / bands as f32;
            for i in 0..bands {
                let hue = i as f32 / bands as f32;
                let band_color = Color::hsv(hue, 1.0, 1.0);
                scene.fill_rect(
                    band_color,
                    Rect::new(sr.origin.x + i as f32 * w, sr.origin.y, w, sr.size.height),
                );
            }
            scene.stroke_rect(colors.divider, sr, 1.0);
        }

        // 滑轨
        for (k, ch) in ALL_CHANNELS.iter().enumerate() {
            let sr = self.slider_rect(rect, k);
            // label
            scene.text(
                ch.label().to_string(),
                Rect::new(
                    sr.origin.x,
                    sr.origin.y + (sr.size.height - style.line_height) / 2.0,
                    20.0,
                    style.line_height,
                ),
                colors.on_surface,
                style,
                TextAlign::Left,
            );
            let (track, frac) = self.track_geom(rect, *ch);
            // 轨道底
            scene.fill_rounded_rect(colors.surface_variant, track, TRACK_CORNER);
            // 填充段（0..value）
            if frac > 0.01 {
                let fill = Rect::new(
                    track.origin.x,
                    track.origin.y,
                    track.size.width * frac,
                    track.size.height,
                );
                scene.fill_rounded_rect(colors.on_surface.with_alpha(0.6), fill, TRACK_CORNER);
            }
            // 拇指
            let thumb_x = track.origin.x + track.size.width * frac - COLOR_THUMB / 2.0;
            let thumb = Rect::new(
                thumb_x,
                sr.origin.y + (sr.size.height - COLOR_THUMB) / 2.0,
                COLOR_THUMB,
                COLOR_THUMB,
            );
            let thumb_color = if self.dragging == Some(*ch) || self.hovered_channel == Some(*ch) {
                colors.primary
            } else {
                colors.on_surface
            };
            scene.fill_rounded_rect(thumb_color, thumb, CornerRadius::Capsule);
        }

        // 预览 + Hex
        let pr = self.preview_rect(rect);
        scene.fill_rounded_rect(
            self.color,
            Rect::new(
                pr.origin.x,
                pr.origin.y,
                pr.size.width,
                COLOR_PREVIEW_H - 4.0,
            ),
            theme.tokens.corner_radius,
        );
        scene.stroke_rect(
            colors.divider,
            Rect::new(
                pr.origin.x,
                pr.origin.y,
                pr.size.width,
                COLOR_PREVIEW_H - 4.0,
            ),
            2.0,
        );
        scene.text(
            self.hex(),
            Rect::new(
                pr.origin.x,
                pr.origin.y + COLOR_PREVIEW_H - 4.0 + 6.0,
                pr.size.width,
                style.line_height,
            ),
            colors.on_surface_variant,
            style,
            TextAlign::Left,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn picker() -> MetroColorPicker {
        MetroColorPicker::default()
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 312.0, 392.0)
    }

    #[test]
    fn default_color_and_hex() {
        let p = picker();
        assert_eq!(p.hex(), "#E57812");
    }

    #[test]
    fn slider_geometry() {
        let p = picker();
        let a = area();
        let s0 = p.slider_rect(a, 0);
        let s1 = p.slider_rect(a, 1);
        assert!(s1.origin.y > s0.origin.y, "滑轨纵向排布");
    }

    #[test]
    fn click_updates_channel() {
        let mut p = picker();
        let a = area();
        // 点 R 滑轨 50% 处
        let sr = p.slider_rect(a, 0);
        let track_x = sr.origin.x + 28.0;
        let mid = Point::new(track_x + (sr.right() - track_x) * 0.5, sr.center().y);
        let old = p.color;
        let result = p.handle_click(a, mid);
        assert!(result.is_some());
        assert!(
            (p.color.r - 0.5).abs() < 0.02,
            "R 通道更新，实际 {}",
            p.color.r
        );
        assert_eq!(p.color.g, old.g);
    }

    #[test]
    fn click_same_value_no_change() {
        let mut p = picker();
        let a = area();
        let sr = p.slider_rect(a, 0);
        let track_x = sr.origin.x + 28.0;
        let old = p.color;
        // 点当前值位置 → 无变化
        let pos = Point::new(track_x + (sr.right() - track_x) * old.r, sr.center().y);
        assert_eq!(p.handle_click(a, pos), None);
    }

    #[test]
    fn drag_updates_continuously() {
        let mut p = picker();
        let a = area();
        let sr = p.slider_rect(a, 3); // A
        let track_x = sr.origin.x + 28.0;
        assert!(p.press(a, Point::new(track_x, sr.center().y)));
        assert_eq!(p.dragging, Some(ColorChannel::Alpha));
        p.drag_to(
            a,
            Point::new(track_x + (sr.right() - track_x) * 0.25, sr.center().y),
        );
        assert!((p.color.a - 0.25).abs() < 0.02);
        p.release();
        assert_eq!(p.dragging, None);
    }

    #[test]
    fn hex_formats() {
        let p = MetroColorPicker {
            color: Color::from_hex(0x4F_C1_FF),
            ..MetroColorPicker::default()
        };
        assert_eq!(p.hex(), "#4FC1FF");
    }

    #[test]
    fn render_emits_spectrum_sliders_preview() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let p = picker();
        let mut scene = Scene::default();
        p.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        // 12 hue 带 + 4 轨道底 + 4 填充 + 4 拇指 + 预览 = ≥25
        assert!(fills >= 25, "Spectrum 带 + 滑轨 + 预览，实际 {fills}");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 5, "4 label + hex，实际 {texts}");
    }
}
