use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect, Scene, TextAlign};

/// MetroList —— 垂直列表。行高 = body 行高 + 上下 8px；视口外行裁剪。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroList {
    pub rows: Vec<String>,
    pub selected: Option<usize>,
    /// 滚动偏移（px，向上滚动为正值）。输入层接入后由滚轮驱动（Phase 3 后续）。
    pub scroll: f32,
    /// 行内边距（水平）。默认 16。
    pub padding_x: f32,
}

impl MetroList {
    pub fn new(rows: Vec<String>) -> Self {
        Self {
            rows,
            selected: None,
            scroll: 0.0,
            padding_x: 16.0,
        }
    }

    pub fn select(&mut self, index: Option<usize>) {
        self.selected = index.filter(|i| *i < self.rows.len());
    }

    /// 行高：body 行高 + 上下 8px。
    pub fn row_height(&self, theme: &MetroTheme) -> f32 {
        theme.typography.body.line_height + 16.0
    }

    /// 渲染到视口 `rect`。顺序：选中行高亮 → 行文字。
    /// `engine` 暂未用于行测量（行文本由外壳统一排版），保留签名以与其余控件一致。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let style = theme.typography.body;
        let row_height = self.row_height(theme);
        let colors = &theme.colors;

        for (i, row) in self.rows.iter().enumerate() {
            let y = rect.origin.y - self.scroll + i as f32 * row_height;
            if y + row_height < rect.origin.y || y > rect.origin.y + rect.size.height {
                continue;
            }
            let row_rect = Rect::new(rect.origin.x, y, rect.size.width, row_height);
            let selected = self.selected == Some(i);

            if selected {
                // Metro 选中高亮：强调色 15% 半透明底
                scene.fill_rect(colors.primary.with_alpha(0.15), row_rect);
            }

            let fg = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let label_rect = Rect::new(
                rect.origin.x + self.padding_x,
                y + 8.0,
                rect.size.width - self.padding_x * 2.0,
                style.line_height,
            );
            scene.text(row.clone(), label_rect, fg, style, TextAlign::Left);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::{Scene, SceneCommand};

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
}
