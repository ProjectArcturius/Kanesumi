// MetroRadioButtons —— 单选组。参 CONTROL_SPEC §21。
//
// 移植自 microsoft-ui-xaml/dev/RadioButtons（RadioButtons.cpp + RadioButtons.xaml）：
// - Header 可选（Margin 0,0,0,8）；ColumnSpacing 7 / RowSpacing 8；MaxColumns 网格；
// - 单选圆 20×20、描边 2px；选中 = 圆心 10px 强调色圆点（Metro 8 观感）；
// - 单个 RadioButton 闭源 → Kanesumi 自绘单选圆。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{CornerRadius, MetroTheme, Point, Rect, Size};

/// 单选圆直径（20）。
pub const RADIO_CIRCLE: f32 = 20.0;
/// 选中圆点直径（10）。
pub const RADIO_DOT: f32 = 10.0;
/// 圆 → 标签 gap（6）。
pub const RADIO_LABEL_GAP: f32 = 6.0;
/// 列间距（RadioButtonsColumnSpacing = 7）。
pub const RADIO_COL_SPACING: f32 = 7.0;
/// 行间距（RadioButtonsRowSpacing = 8）。
pub const RADIO_ROW_SPACING: f32 = 8.0;

/// MetroRadioButtons —— 单选组。参 CONTROL_SPEC §21。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroRadioButtons {
    /// 组标题（可选）。
    pub header: String,
    pub items: Vec<String>,
    /// 选中索引（None = 未选）。
    pub selected_index: Option<usize>,
    /// 最大列数（默认 1 = 纵向）。
    pub max_columns: usize,
    /// 当前 hover 项。
    pub hovered: Option<usize>,
}

impl Default for MetroRadioButtons {
    fn default() -> Self {
        Self {
            header: String::new(),
            items: Vec::new(),
            selected_index: None,
            max_columns: 1,
            hovered: None,
        }
    }
}

impl MetroRadioButtons {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// 列数（≤ items 数，≥1）。
    fn cols(&self) -> usize {
        self.max_columns.clamp(1, self.items.len().max(1))
    }

    /// 单行高 = max(圆 20, 行高)。
    fn row_height() -> f32 {
        let body = MetroTheme::default().typography.body;
        body.line_height.max(RADIO_CIRCLE)
    }

    /// 网格几何：总尺寸 + 每项 rect。
    pub fn layout(&self, engine: &TextEngine, rect: Rect) -> (Size, Vec<Rect>) {
        let cols = self.cols();
        let body = MetroTheme::default().typography.body;
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            body.line_height + 8.0
        };

        // 每列最大宽
        let mut col_widths = vec![0.0f32; cols];
        for (i, item) in self.items.iter().enumerate() {
            let col = i % cols;
            let w = RADIO_CIRCLE + RADIO_LABEL_GAP + engine.measure(item, body.size);
            col_widths[col] = col_widths[col].max(w);
        }
        let total_w: f32 =
            col_widths.iter().sum::<f32>() + RADIO_COL_SPACING * (cols.saturating_sub(1)) as f32;

        let rows = self.items.len().div_ceil(cols);
        let row_h = Self::row_height();
        let total_h = rows as f32 * row_h + (rows.saturating_sub(1)) as f32 * RADIO_ROW_SPACING;

        let origin_x = rect.origin.x;
        let origin_y = rect.origin.y + header_h;

        // 每项 rect（列 x 起点累积）
        let mut col_x = Vec::with_capacity(cols);
        let mut x = origin_x;
        for w in &col_widths {
            col_x.push(x);
            x += w + RADIO_COL_SPACING;
        }
        let mut rects = Vec::with_capacity(self.items.len());
        for (i, _) in self.items.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            rects.push(Rect::new(
                col_x[col],
                origin_y + row as f32 * (row_h + RADIO_ROW_SPACING),
                col_widths[col],
                row_h,
            ));
        }
        (Size::new(total_w, header_h + total_h), rects)
    }

    /// 固有尺寸（相对原点）。
    pub fn measure(&self, engine: &TextEngine) -> Size {
        let (size, _) = self.layout(engine, Rect::new(0.0, 0.0, 0.0, 0.0));
        size
    }

    /// 命中项。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> Option<usize> {
        let (_, rects) = self.layout(engine, rect);
        rects.iter().position(|r| r.contains(pos))
    }

    /// 选中（返回是否变化）。
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return false;
        }
        self.selected_index = Some(index);
        true
    }

    /// 处理点击。
    pub fn handle_click(&mut self, engine: &TextEngine, rect: Rect, pos: Point) -> Option<usize> {
        let i = self.hit(engine, rect, pos)?;
        self.select(i);
        Some(i)
    }

    /// 悬停路由。
    pub fn hover(&mut self, engine: &TextEngine, rect: Rect, pos: Point) {
        self.hovered = self.hit(engine, rect, pos);
    }

    /// 渲染：Header + 每项（单选圆 + 标签）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let body = theme.typography.body;

        // Header
        if !self.header.is_empty() {
            scene.text(
                self.header.clone(),
                Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    body.line_height,
                ),
                colors.on_surface,
                body,
                TextAlign::Left,
            );
        }

        let (_, rects) = self.layout(engine, rect);
        for (i, item) in self.items.iter().enumerate() {
            let r = rects[i];
            let cy = r.origin.y + (r.size.height - RADIO_CIRCLE) / 2.0;
            let circle_rect = Rect::new(r.origin.x, cy, RADIO_CIRCLE, RADIO_CIRCLE);
            let checked = self.selected_index == Some(i);
            let hovered = self.hovered == Some(i);

            // 外圈
            let stroke = if checked || hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.stroke_rounded_rect(stroke, circle_rect, 2.0, CornerRadius::Capsule);
            // 选中圆点
            if checked {
                let dot = Rect::new(
                    r.origin.x + (RADIO_CIRCLE - RADIO_DOT) / 2.0,
                    cy + (RADIO_CIRCLE - RADIO_DOT) / 2.0,
                    RADIO_DOT,
                    RADIO_DOT,
                );
                scene.fill_rounded_rect(colors.primary, dot, CornerRadius::Capsule);
            }

            // 标签
            let label_w = engine.measure(item, body.size);
            let text_rect = Rect::new(
                r.origin.x + RADIO_CIRCLE + RADIO_LABEL_GAP,
                r.origin.y + (r.size.height - body.line_height) / 2.0,
                label_w,
                body.line_height,
            );
            scene.text(
                item.clone(),
                text_rect,
                colors.on_surface,
                body,
                TextAlign::Left,
            );
        }
    }
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

    fn radio() -> MetroRadioButtons {
        MetroRadioButtons::new(["低", "中", "高"].iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn default_vertical_single_column() {
        let Some(engine) = find_engine() else { return };
        let r = radio();
        let (_, rects) = r.layout(&engine, Rect::new(0.0, 0.0, 300.0, 300.0));
        assert_eq!(rects.len(), 3);
        // 纵向：y 递增
        assert!(rects[1].origin.y > rects[0].origin.y);
        assert!(rects[2].origin.y > rects[1].origin.y);
    }

    #[test]
    fn grid_layout_with_max_columns() {
        let Some(engine) = find_engine() else { return };
        let mut r = radio();
        r.max_columns = 3;
        let (_, rects) = r.layout(&engine, Rect::new(0.0, 0.0, 600.0, 300.0));
        // 3 列一行：y 相同，x 递增
        assert_eq!(rects[0].origin.y, rects[1].origin.y);
        assert!(rects[1].origin.x > rects[0].origin.x);
    }

    #[test]
    fn select_updates_index() {
        let Some(engine) = find_engine() else { return };
        let mut r = radio();
        let (_, rects) = r.layout(&engine, Rect::new(0.0, 0.0, 300.0, 300.0));
        let i = r.handle_click(
            &engine,
            Rect::new(0.0, 0.0, 300.0, 300.0),
            rects[1].center(),
        );
        assert_eq!(i, Some(1));
        assert_eq!(r.selected_index, Some(1));
    }

    #[test]
    fn re_select_same_keeps() {
        let Some(engine) = find_engine() else { return };
        let mut r = radio();
        r.selected_index = Some(1);
        assert!(!r.select(1), "再选同项不变化");
        assert_eq!(r.selected_index, Some(1));
    }

    #[test]
    fn hit_outside_none() {
        let Some(engine) = find_engine() else { return };
        let r = radio();
        assert_eq!(
            r.hit(
                &engine,
                Rect::new(0.0, 0.0, 300.0, 300.0),
                Point::new(1000.0, 1000.0)
            ),
            None
        );
    }

    #[test]
    fn render_emits_circles_and_dot() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut r = radio();
        r.selected_index = Some(1);
        let mut scene = Scene::default();
        r.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 300.0),
            &mut scene,
        );
        // 3 外圈 stroke + 1 选中 dot fill
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count();
        assert_eq!(strokes, 3, "3 个外圈");
        let dots = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.primary))
            .count();
        assert_eq!(dots, 1, "1 个选中圆点");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 3, "3 个标签");
    }

    #[test]
    fn header_renders_when_set() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut r = radio();
        r.header = "缩放级别".into();
        let mut scene = Scene::default();
        r.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 300.0),
            &mut scene,
        );
        let texts = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(texts.iter().any(|t| t == "缩放级别"));
    }
}
