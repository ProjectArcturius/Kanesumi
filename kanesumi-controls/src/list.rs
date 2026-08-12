use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect};

use crate::repeater::{MetroRepeater, RepeaterOrientation};

/// MetroList —— 垂直列表。行高下限 40（UWP ListViewItem MinHeight，参 CONTROL_SPEC §7）。
///
/// 虚拟化：经 `MetroRepeater::visible_range` 只渲染视口内行（参 CONTROL_SPEC §41）——
/// 长列表不遍历全部行，仅计算可见窗口，避免掉帧。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroList {
    pub rows: Vec<String>,
    pub selected: Option<usize>,
    /// 悬停行（PointerOver 中性高亮，非强调色，参 CONTROL_SPEC §5 规律 5）。
    pub hovered: Option<usize>,
    /// 整表禁用：行降透明度（CONTROL_SPEC §7 禁用 = 整行 Opacity 0.55）。
    pub disabled: bool,
    /// 滚动偏移（px）。正值 = 内容上移（显示更靠后行）。由滚轮驱动（`scroll_by`）。
    pub scroll: f32,
    /// 行内边距（水平）。UWP 为 12。
    pub padding_x: f32,
}

impl MetroList {
    pub fn new(rows: Vec<String>) -> Self {
        Self {
            rows,
            selected: None,
            hovered: None,
            disabled: false,
            scroll: 0.0,
            padding_x: 12.0,
        }
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|i| *i < self.rows.len());
    }

    /// 内容总高（行数 × 行高）。
    pub fn content_height(&self, theme: &MetroTheme) -> f32 {
        self.rows.len() as f32 * self.row_height(theme)
    }

    /// 最大滚动偏移：内容总高 − 视口高（不小于 0）。
    pub fn max_scroll(&self, theme: &MetroTheme, viewport_h: f32) -> f32 {
        (self.content_height(theme) - viewport_h).max(0.0)
    }

    /// 按增量滚动（`dy` 正 = 向下，同 `InputEvent::Scroll`）。夹紧到 [0, max]。
    pub fn scroll_by(&mut self, theme: &MetroTheme, viewport_h: f32, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll(theme, viewport_h));
    }

    /// 滚动到指定偏移。夹紧到 [0, max]。
    pub fn scroll_to(&mut self, theme: &MetroTheme, viewport_h: f32, offset: f32) {
        self.scroll = offset.clamp(0.0, self.max_scroll(theme, viewport_h));
    }

    /// 行高：body 行高 + 上下 8px，下限 40（UWP MinHeight）。
    pub fn row_height(&self, theme: &MetroTheme) -> f32 {
        (theme.typography.body.line_height + 16.0).max(40.0)
    }

    /// 虚拟化布局器（MetroRepeater StackLayout，参 CONTROL_SPEC §41）。
    pub fn virtualizer(&self, theme: &MetroTheme) -> MetroRepeater {
        MetroRepeater {
            item_count: self.rows.len(),
            main_extent: self.row_height(theme),
            layout: crate::repeater::RepeaterLayout::Stack,
            orientation: RepeaterOrientation::Vertical,
            ..MetroRepeater::default()
        }
    }

    /// 渲染到视口 `rect`。顺序：选中高亮 → 悬停高亮 → 行文字。
    /// 只渲染视口内行（Repeater 虚拟化）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let style = theme.typography.body;
        let row_height = self.row_height(theme);
        let colors = &theme.colors;
        let alpha = if self.disabled {
            0.55 // CONTROL_SPEC §7：禁用 = 整行 Opacity 0.55
        } else {
            1.0
        };

        let repeater = self.virtualizer(theme);
        let Some((first, last)) = repeater.visible_range(rect.size.height, self.scroll) else {
            return;
        };

        for i in first..=last {
            let y = rect.origin.y - self.scroll + i as f32 * row_height;
            let row_rect = Rect::new(rect.origin.x, y, rect.size.width, row_height);
            let selected = self.selected == Some(i);
            let hovered = self.hovered == Some(i);

            if selected {
                // 选中高亮：UWP ListViewItem Selected = 强调色 ~75%（ListAccentMediumLow）。
                // Ether 深色空间桌面下调一档至 0.60（参 CONTROL_SPEC §7）。
                scene.fill_rect(colors.primary.with_alpha(0.60 * alpha), row_rect);
            } else if hovered && !self.disabled {
                // 悬停 = 中性高亮（HighlightListLow ≈30% 白），非强调色（CONTROL_SPEC §5 规律 5）。
                scene.fill_rect(colors.on_surface.with_alpha(0.30), row_rect);
            }

            let base = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let fg = base.with_alpha(base.a * alpha);
            let label_rect = Rect::new(
                rect.origin.x + self.padding_x,
                y + 8.0,
                rect.size.width - self.padding_x * 2.0,
                style.line_height,
            );
            scene.text(self.rows[i].clone(), label_rect, fg, style, TextAlign::Left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::{Scene, SceneCommand};

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
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
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

    fn render(rows: Vec<String>, selected: Option<usize>, scroll: f32, rect: Rect) -> Scene {
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(rows);
        list.select(selected);
        list.scroll = scroll;
        let mut scene = Scene::default();
        list.render(&theme, &engine, rect, &mut scene);
        scene
    }

    #[test]
    fn emits_row_texts() {
        if !font_available() {
            return;
        }
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            None,
            0.0,
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 3);
    }

    #[test]
    fn selection_highlights_row() {
        if !font_available() {
            return;
        }
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            Some(1),
            0.0,
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let highlights = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(highlights, 1, "只有选中行有高亮底");
    }

    #[test]
    fn scroll_skips_out_of_view_rows() {
        if !font_available() {
            return;
        }
        // 滚动 500px：三行全部移出视口 → 无文本命令
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            None,
            500.0,
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        assert!(scene.is_empty());
    }

    #[test]
    fn select_out_of_range_is_none() {
        let mut list = MetroList::new(vec!["A".into()]);
        list.select(Some(5));
        assert_eq!(list.selected, None);
    }

    #[test]
    fn scroll_by_clamps_to_content() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(), "B".into(), "C".into()]);
        let viewport_h = 100.0;
        let max = list.max_scroll(&theme, viewport_h);
        // 3 行 × 40 = 120，视口 100 → max 20
        assert_eq!(max, 20.0);

        list.scroll_by(&theme, viewport_h, 10.0);
        assert_eq!(list.scroll, 10.0);
        // 向下滚超出 → 夹紧到 max
        list.scroll_by(&theme, viewport_h, 100.0);
        assert_eq!(list.scroll, max);
        // 向上滚超出 → 夹紧到 0
        list.scroll_by(&theme, viewport_h, -100.0);
        assert_eq!(list.scroll, 0.0);
    }

    #[test]
    fn scroll_to_clamps_negative() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(); 10]);
        list.scroll_to(&theme, 200.0, -5.0);
        assert_eq!(list.scroll, 0.0);
        list.scroll_to(&theme, 200.0, 9999.0);
        assert_eq!(list.scroll, list.max_scroll(&theme, 200.0));
    }

    #[test]
    fn no_scroll_when_content_fits() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(), "B".into()]);
        list.scroll_by(&theme, 300.0, 50.0);
        assert_eq!(list.scroll, 0.0, "内容不足一屏不滚动");
    }

    #[test]
    fn hover_highlights_neutral_not_accent() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["Alpha".into(), "Beta".into()]);
        list.hovered = Some(1);
        let mut scene = Scene::default();
        list.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &mut scene,
        );
        // 悬停行用中性高亮（on_surface 白 30%），不用强调色
        let hover_fills = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, .. } if color.r > 0.0 => Some(color),
                _ => None,
            })
            .count();
        assert_eq!(hover_fills, 1, "只有悬停行有中性高亮");
    }

    #[test]
    fn disabled_lowers_row_alpha() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["Alpha".into()]);
        list.disabled = true;
        let mut scene = Scene::default();
        list.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &mut scene,
        );
        let Some(SceneCommand::Text { color, .. }) = scene.commands.first() else {
            panic!("首命令应为行文本");
        };
        assert!(color.a < 1.0, "禁用态行文字应降透明度，实际 a={}", color.a);
    }
}
