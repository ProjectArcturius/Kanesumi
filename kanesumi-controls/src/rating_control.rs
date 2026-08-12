// MetroRatingControl —— 星级评分。参 CONTROL_SPEC §24。
//
// 移植自 microsoft-ui-xaml/dev/RatingControl（RatingControl.cpp + RatingControl.xaml）：
// - MaxRating 默认 5；star cell ≈ 24×24；实星 ★（强调色）/ 空星 ☆（on_surface_variant）；
// - Value 支持小数（部分星 = 前景星按比例裁剪，用 Scene::clip）；
// - PointerOver 预览（hover_value）；IsClearEnabled 点当前值星清零；IsReadOnly 只读。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect};

/// star cell 边长（≈24）。
pub const RATING_ITEM_SIZE: f32 = 24.0;
/// 星间距（4）。
pub const RATING_ITEM_SPACING: f32 = 4.0;

/// MetroRatingControl —— 星级评分。参 CONTROL_SPEC §24。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroRatingControl {
    /// 当前值（0..max_rating，可小数）。
    pub value: f64,
    pub max_rating: usize,
    pub is_read_only: bool,
    /// 点当前值星清零。
    pub is_clear_enabled: bool,
    /// PointerOver 预览值（None = 无预览）。
    pub hover_value: Option<f64>,
    /// 星字号。
    pub item_size: f32,
    pub item_spacing: f32,
}

impl Default for MetroRatingControl {
    fn default() -> Self {
        Self {
            value: 0.0,
            max_rating: 5,
            is_read_only: false,
            is_clear_enabled: false,
            hover_value: None,
            item_size: RATING_ITEM_SIZE,
            item_spacing: RATING_ITEM_SPACING,
        }
    }
}

impl MetroRatingControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// 星字形样式（item_size）。
    pub fn star_style(&self) -> TextStyle {
        TextStyle::new(self.item_size, self.item_size, FontWeight::Normal)
    }

    /// 固有尺寸。
    pub fn measure(&self) -> kanesumi_core::Size {
        kanesumi_core::Size::new(
            self.max_rating as f32 * self.item_size
                + (self.max_rating.saturating_sub(1)) as f32 * self.item_spacing,
            self.item_size,
        )
    }

    /// 第 k 个星 cell rect（1 基）。
    pub fn star_rect(&self, rect: Rect, k: usize) -> Rect {
        let x = rect.origin.x + (k as f32 - 1.0) * (self.item_size + self.item_spacing);
        Rect::new(x, rect.origin.y, self.item_size, self.item_size)
    }

    /// 命中星（1 基）。0 = 无。
    fn star_at(&self, rect: Rect, pos: Point) -> usize {
        for k in 1..=self.max_rating {
            if self.star_rect(rect, k).contains(pos) {
                return k;
            }
        }
        0
    }

    /// 悬停预览。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        if self.is_read_only {
            self.hover_value = None;
            return;
        }
        let k = self.star_at(rect, pos);
        self.hover_value = if k == 0 { None } else { Some(k as f64) };
    }

    /// 点击：设置值（含 Clear 语义）。返回新值（None = 未变化）。
    pub fn click(&mut self, rect: Rect, pos: Point) -> Option<f64> {
        if self.is_read_only {
            return None;
        }
        let k = self.star_at(rect, pos);
        if k == 0 {
            return None;
        }
        let new_value = if self.is_clear_enabled && (self.value.round() as usize) == k {
            0.0
        } else {
            k as f64
        };
        self.hover_value = None;
        if (new_value - self.value).abs() < 1e-9 {
            None
        } else {
            self.value = new_value;
            Some(new_value)
        }
    }

    /// 渲染：每星（空星底 + 实星覆盖，部分星裁剪）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = self.star_style();
        // 预览优先，否则当前值。
        let shown = self.hover_value.unwrap_or(self.value);
        for k in 1..=self.max_rating {
            let sr = self.star_rect(rect, k);
            // 空星底
            let outline_color = if self.is_read_only {
                colors.on_surface_variant.with_alpha(0.5)
            } else {
                colors.on_surface_variant
            };
            scene.text(
                "☆".into(),
                star_text_rect(&sr, &style),
                outline_color,
                style,
                TextAlign::Center,
            );
            // 实星覆盖
            let fill_frac = (shown - (k as f64 - 1.0)).clamp(0.0, 1.0);
            if fill_frac > 0.0 {
                let filled_color = if self.is_read_only {
                    colors.on_surface_variant.with_alpha(0.5)
                } else {
                    colors.primary
                };
                // 全星：直接画
                if fill_frac >= 1.0 - 1e-6 {
                    scene.text(
                        "★".into(),
                        star_text_rect(&sr, &style),
                        filled_color,
                        style,
                        TextAlign::Center,
                    );
                } else {
                    // 部分星：裁剪到 fill_frac 宽
                    let clip = Rect::new(
                        sr.origin.x,
                        sr.origin.y,
                        sr.size.width * fill_frac as f32,
                        sr.size.height,
                    );
                    scene.push_clip(clip);
                    scene.text(
                        "★".into(),
                        star_text_rect(&sr, &style),
                        filled_color,
                        style,
                        TextAlign::Center,
                    );
                    scene.pop_clip();
                }
            }
        }
        let _ = engine;
    }
}

/// 星文本 rect（cell 内居中）。
fn star_text_rect(cell: &Rect, style: &TextStyle) -> Rect {
    Rect::new(
        cell.origin.x,
        cell.origin.y + (cell.size.height - style.line_height) / 2.0,
        cell.size.width,
        style.line_height,
    )
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
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn rating() -> MetroRatingControl {
        MetroRatingControl::default()
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 24.0)
    }

    #[test]
    fn defaults() {
        let r = rating();
        assert_eq!(r.max_rating, 5);
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn star_geometry() {
        let r = rating();
        let a = area();
        let s1 = r.star_rect(a, 1);
        let s2 = r.star_rect(a, 2);
        assert_eq!(s1.origin.x, 0.0);
        assert_eq!(s1.size.width, 24.0);
        assert_eq!(s2.origin.x, 24.0 + 4.0, "星间距 4");
    }

    #[test]
    fn click_sets_value() {
        let mut r = rating();
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), Some(3.0));
        assert_eq!(r.value, 3.0);
    }

    #[test]
    fn clear_enabled_resets() {
        let mut r = rating();
        r.is_clear_enabled = true;
        r.value = 3.0;
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), Some(0.0), "点当前值星清零");
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn read_only_no_change() {
        let mut r = rating();
        r.is_read_only = true;
        r.value = 2.0;
        let a = area();
        let s4 = r.star_rect(a, 4);
        assert_eq!(r.click(a, s4.center()), None);
        assert_eq!(r.value, 2.0);
    }

    #[test]
    fn click_same_value_no_change() {
        let mut r = rating();
        r.value = 3.0;
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), None, "同值不触发变更");
    }

    #[test]
    fn hover_sets_preview() {
        let mut r = rating();
        let a = area();
        let s4 = r.star_rect(a, 4);
        r.hover(a, s4.center());
        assert_eq!(r.hover_value, Some(4.0));
        r.hover(a, Point::new(1000.0, 1000.0));
        assert_eq!(r.hover_value, None);
    }

    #[test]
    fn render_emits_stars() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut r = rating();
        r.value = 2.5;
        let mut scene = Scene::default();
        r.render(&theme, &engine, area(), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 8, "5 空星 + 3 实星（2 整 + 1 半），实际 {texts}");
        // 部分星（0.5）→ 有 clip（仅 set，clear 为 None 不计）
        let clips = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::PushClip { .. }))
            .count();
        assert_eq!(clips, 1, "部分星裁剪 set 一次");
    }
}
