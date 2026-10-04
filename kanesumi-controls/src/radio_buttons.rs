// MetroRadioButtons —— 单选组。参 CONTROL_SPEC §21。
//
// 移植自 microsoft-ui-xaml/dev/RadioButtons（RadioButtons.cpp + RadioButtons.xaml）：
// - Header 可选（Margin 0,0,0,8）；ColumnSpacing 7 / RowSpacing 8；MaxColumns 网格；
// - 单选圆 20×20、描边 2px；选中 = 圆心 10px 强调色圆点（Metro 8 观感）；
// - 单个 RadioButton 闭源 → Kanesumi 自绘单选圆。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextVAlign;
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

            // 外圈（未选中/未悬停取 UWP `RadioButtonOuterEllipseStroke` = BaseMediumHigh 80%）
            let stroke = if checked || hovered {
                colors.on_surface
            } else {
                colors.control_stroke_strong
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

            // 标签：可用宽 = 行宽 − 圆钮 − 间距（非量测宽），单行省略号。
            // 2026-09-22 审计 P0-1。
            // 纵向交给 TextVAlign::Center（与手算 (h−lh)/2 逐位同值）。参 o4 纵向对齐。
            let text_rect = Rect::new(
                r.origin.x + RADIO_CIRCLE + RADIO_LABEL_GAP,
                r.origin.y,
                (r.size.width - RADIO_CIRCLE - RADIO_LABEL_GAP).max(0.0),
                r.size.height,
            );
            scene.label(
                item.clone(),
                text_rect,
                colors.on_surface,
                body.with_v_align(TextVAlign::Center),
                TextAlign::Left,
            );
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）──────────────────
//
// 旧 API（`measure` / `handle_click` / `hover` / `render`）原样保留给未迁移的 App。
// `Click` 事件不带坐标，故点击在 `PointerUp` 里用 pos 调旧 `handle_click`；`ctx.engine()`
// 为 `None`（首帧之前）时忽略。方向键在选项间移动并选中。命中与焦点交给框架。

/// 元素树动作：单选组选中项改变（点击 / 方向键）。携带新的选中索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RadioSelectionChanged(pub usize);

impl kanesumi_element::Widget for MetroRadioButtons {
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        _available: Size,
    ) -> Size {
        MetroRadioButtons::measure(self, ctx.engine())
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                if let Some(engine) = ctx.engine() {
                    self.hover(engine, rect, *pos);
                }
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let before = self.selected_index;
                // 先取出点击结果，结束对 ctx 的不可变借用，再发动作。
                let clicked = match ctx.engine() {
                    Some(engine) => self.handle_click(engine, rect, *pos),
                    None => None,
                };
                if let Some(i) = clicked
                    && self.selected_index != before
                {
                    ctx.emit(RadioSelectionChanged(i));
                }
                if clicked.is_some() {
                    ctx.invalidate_paint();
                }
            }
            Event::KeyDown { key, .. } => {
                if self.items.is_empty() {
                    return;
                }
                let next = match key {
                    Key::Down => Some(match self.selected_index {
                        Some(i) => (i + 1).min(self.items.len() - 1),
                        None => 0,
                    }),
                    Key::Up => Some(match self.selected_index {
                        Some(i) => i.saturating_sub(1),
                        None => self.items.len() - 1,
                    }),
                    _ => None,
                };
                let Some(i) = next else { return };
                if self.select(i) {
                    ctx.emit(RadioSelectionChanged(i));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: self.header.clone(),
            value: self
                .selected_index
                .and_then(|i| self.items.get(i).cloned()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroRadioButtons::new(vec!["低".into(), "中".into(), "高".into()]),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn item_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let radio = h.tree.get::<MetroRadioButtons>(id).unwrap();
        let (_, rects) = radio.layout(&h.engine, h.rect(id));
        rects[i].center()
    }

    #[test]
    fn click_selects_item_and_reports() {
        let (mut h, id) = harness();
        h.click_at(item_center(&h, id, 1));
        assert_eq!(
            h.take::<RadioSelectionChanged>(),
            vec![(id, RadioSelectionChanged(1))]
        );
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().selected_index,
            Some(1)
        );
    }

    #[test]
    fn arrow_keys_move_selection_and_tab_focuses() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Down);
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().selected_index,
            Some(0)
        );
        h.key(Key::Down);
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().selected_index,
            Some(1)
        );
        h.key(Key::Up);
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().selected_index,
            Some(0)
        );
        assert_eq!(h.take::<RadioSelectionChanged>().len(), 3);
    }

    #[test]
    fn pointer_move_sets_and_leave_clears_hover() {
        let (mut h, id) = harness();
        h.move_to(item_center(&h, id, 2));
        h.frame();
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().hovered,
            Some(2)
        );
        h.tree.pointer_leave();
        h.frame();
        assert_eq!(h.tree.get::<MetroRadioButtons>(id).unwrap().hovered, None);
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness();
        assert!(h.rect(id).size.width > 0.0 && h.rect(id).size.height > 0.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(item_center(&h, id, 1));
        assert!(h.take::<RadioSelectionChanged>().is_empty());
        assert_eq!(
            h.tree.get::<MetroRadioButtons>(id).unwrap().selected_index,
            None
        );
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
