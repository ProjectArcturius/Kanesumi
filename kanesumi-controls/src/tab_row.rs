use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, TextStyle};

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
}

impl Default for MetroTabRow {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            selected: 0,
            hovered: None,
            header_height: 48.0,
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

    /// Header 文字样式：24 SemiLight，字距 −2.5%（UWP CharacterSpacing=−25）。
    /// V16：字距落到 TextStyle.letter_spacing_em，render/measure 全局生效。
    pub fn header_style() -> TextStyle {
        TextStyle::new(24.0, 30.0, FontWeight::Semilight).with_letter_spacing_em(-0.025)
    }

    /// 单个 Header 宽度 = 文字宽（含字距）+ 左右 12px。
    pub fn header_width(&self, engine: &TextEngine, index: usize) -> f32 {
        if index >= self.tabs.len() {
            return 0.0;
        }
        let style = Self::header_style();
        engine.measure_with_spacing(&self.tabs[index].label, style.size, style.letter_spacing_em)
            + 24.0
    }

    /// 全部 Header 总宽。
    pub fn total_width(&self, engine: &TextEngine) -> f32 {
        let style = Self::header_style();
        self.tabs
            .iter()
            .map(|t| {
                engine.measure_with_spacing(&t.label, style.size, style.letter_spacing_em) + 24.0
            })
            .sum()
    }

    /// Header 命中测试：`rect` = TabRow 布局矩形；`pos` = **绝对**指针坐标。
    ///
    /// 与 [`MetroSwitch::hit_test`] 约定一致——命中逻辑集中在控件内，调用方
    /// 无需手动减 origin。历史 bug：旧签名 `(engine, x)` 把 x 当**相对**坐标，
    /// Gallery 传绝对 `p.x` → 命中偏移一个 `rect.origin.x`。
    pub fn tab_at(&self, engine: &TextEngine, rect: Rect, pos: Point) -> Option<usize> {
        if !rect.contains(pos) {
            return None;
        }
        let style = Self::header_style();
        let mut cursor = rect.origin.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = engine.measure_with_spacing(&tab.label, style.size, style.letter_spacing_em)
                + 24.0;
            if pos.x >= cursor && pos.x < cursor + w {
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
            let label_w =
                engine.measure_with_spacing(&tab.label, style.size, style.letter_spacing_em);
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
        // rect 有非零 origin，验证内部相对化正确
        let rect = Rect::new(50.0, 100.0, 400.0, 48.0);
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(50.0 + 1.0, 100.0 + 10.0)),
            Some(0)
        );
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(50.0 + w0 + 2.0, 100.0 + 10.0)),
            Some(1)
        );
        // rect 外
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(10.0, 10.0)),
            None,
            "点在 rect 外应返回 None"
        );
        // rect 内但 x 超过所有 header 宽度
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(400.0, 110.0)),
            None,
            "命中失败应返回 None"
        );
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
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(pipes, 1, "只有选中 tab 有管道");
    }

    #[test]
    fn select_out_of_range_ignored() {
        let mut row = MetroTabRow::new(vec![MetroTab::new("A")]);
        row.select(5);
        assert_eq!(row.selected, 0);
    }

    #[test]
    fn header_style_carries_negative_letter_spacing() {
        // V16: header_style 应带 −0.025em 字距（UWP CharacterSpacing=−25）
        let s = MetroTabRow::header_style();
        assert!((s.letter_spacing_em - (-0.025)).abs() < 1e-6);
        // 24px × −0.025 = −0.6 px/字
        assert!((s.letter_spacing_px() - (-0.6)).abs() < 1e-6);
    }

    #[test]
    fn header_width_reflects_letter_spacing() {
        // V16: header_width（含字距）应 < 无字距 measure（负字距收紧）
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("邮件")]);
        let style = MetroTabRow::header_style();
        let w_spaced = row.header_width(&engine, 0);
        let w_raw = engine.measure("邮件", style.size) + 24.0;
        assert!(
            w_spaced < w_raw,
            "字距 −0.025em 应收紧宽度：w_spaced={w_spaced}, w_raw={w_raw}"
        );
        // 差异 = n_chars × letter_spacing_px = 2 × −0.6 = −1.2
        assert!((w_spaced - w_raw - (2.0 * -0.6)).abs() < 0.01);
    }
}
