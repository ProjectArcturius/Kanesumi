use kanesumi_core::text::TextEngine;
use kanesumi_core::{FontWeight, MetroTheme, Rect, Scene, TextAlign, TextStyle};

/// MetroTabRow —— 标签行（Pivot 参考）。参 CONTROL_SPEC §6：
/// - Header 高 48、Padding `12,0,12,0`；头字 24 SemiLight、字距 −2.5%；
/// - 选中 = 文字最深 + 底部 2px 强调色管道（各 Header 独立，切换瞬时）；
/// - 无头背景高亮（选中只靠文字色 + 管道）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTab {
    pub label: String,
}

impl MetroTab {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MetroTabRow {
    pub tabs: Vec<MetroTab>,
    pub selected: usize,
    pub hovered: Option<usize>,
    /// Header 高（UWP 48）。
    pub header_height: f32,
    /// 头字间距（px）。UWP 字距 −2.5% ≈ 每字 0.6px 收紧，简单起见留空实现。
    pub header_spacing: f32,
}

impl Default for MetroTabRow {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            selected: 0,
            hovered: None,
            header_height: 48.0,
            header_spacing: 0.0,
        }
    }
}

impl MetroTabRow {
    pub fn new(tabs: Vec<MetroTab>) -> Self {
        Self {
            tabs,
            ..Self::default()
        }
    }

    pub fn select(&mut self, index: usize) {
        if index < self.tabs.len() {
            self.selected = index;
        }
    }

    /// Header 文字样式：24 SemiLight（PivotHeaderItemFontSize/Weight）。
    pub fn header_style() -> TextStyle {
        TextStyle::new(24.0, 30.0, FontWeight::Semilight)
    }

    /// 单个 Header 宽度 = 文字宽 + 左右 12px。
    pub fn header_width(&self, engine: &TextEngine, index: usize) -> f32 {
        if index >= self.tabs.len() {
            return 0.0;
        }
        engine.measure(&self.tabs[index].label, Self::header_style().size) + 24.0
    }

    /// 全部 Header 总宽。
    pub fn total_width(&self, engine: &TextEngine) -> f32 {
        self.tabs
            .iter()
            .map(|t| engine.measure(&t.label, Self::header_style().size) + 24.0)
            .sum()
    }

    /// Header 命中测试：返回命中的 tab 索引。
    pub fn tab_at(&self, engine: &TextEngine, x: f32) -> Option<usize> {
        let mut cursor = 0.0;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = engine.measure(&tab.label, Self::header_style().size) + 24.0;
            if x >= cursor && x < cursor + w {
                return Some(i);
            }
            cursor += w;
        }
        None
    }

    /// 渲染到 `rect`（横向排列，垂直到顶）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = Self::header_style();
        let mut cursor = rect.origin.x;

        for (i, tab) in self.tabs.iter().enumerate() {
            let label_w = engine.measure(&tab.label, style.size);
            let header_w = label_w + 24.0;
            let selected = i == self.selected;

            // 文字色：选中或悬停 = 最深（on_surface），未选中 = 次级（on_surface_variant）
            let fg = if selected || self.hovered == Some(i) {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };

            let text_rect = Rect::new(cursor + 12.0, rect.origin.y, label_w, self.header_height);
            scene.text(tab.label.clone(), text_rect, fg, style, TextAlign::Left);

            // 选中管道：高 2、贴底 2px、宽 = 文字宽、强调色
            if selected {
                let pipe_rect = Rect::new(
                    cursor + 12.0,
                    rect.origin.y + self.header_height - 2.0 - 2.0,
                    label_w,
                    2.0,
                );
                scene.fill_rect(colors.primary, pipe_rect);
            }

            cursor += header_w;
        }
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
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn header_height_matches_spec() {
        assert_eq!(MetroTabRow::default().header_height, 48.0);
        assert_eq!(MetroTabRow::header_style().size, 24.0);
    }

    #[test]
    fn tab_at_maps_x() {
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("Mail"), MetroTab::new("Calendar")]);
        let w0 = row.header_width(&engine, 0);
        assert_eq!(row.tab_at(&engine, 1.0), Some(0));
        assert_eq!(row.tab_at(&engine, w0 + 2.0), Some(1));
        assert_eq!(row.tab_at(&engine, 99999.0), None);
    }

    #[test]
    fn selected_draws_pipe_only_on_selected() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroTabRow::new(vec![MetroTab::new("Mail"), MetroTab::new("Calendar")]);
        row.select(1);
        let mut scene = Scene::default();
        row.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 48.0),
            &mut scene,
        );
        let pipes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_core::SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(pipes, 1, "只有选中 tab 有管道");
    }

    #[test]
    fn select_out_of_range_ignored() {
        let mut row = MetroTabRow::new(vec![MetroTab::new("A")]);
        row.select(5);
        assert_eq!(row.selected, 0);
    }
}
