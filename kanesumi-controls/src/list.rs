use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect};

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

        // 容器语义 = 裁到自身矩形（2026-09-22 审计 P0-2）：虚拟化只保证「不渲染视口外的行」，
        // 但部分滚出的行（半行）其高亮/文字仍会画到列表 rect 之外。
        scene.push_clip(rect);

        for i in first..=last {
            let y = rect.origin.y - self.scroll + i as f32 * row_height;
            let row_rect = Rect::new(rect.origin.x, y, rect.size.width, row_height);
            let selected = self.selected == Some(i);
            let hovered = self.hovered == Some(i);

            if selected {
                // 选中高亮 = 强调色 AccentLow（暗 0.6 / 亮 0.4，一手源 themeresources L304/L4220）。
                scene.fill_rect(
                    theme
                        .colors
                        .selection_tint
                        .with_alpha(theme.colors.selection_tint.a * alpha),
                    row_rect,
                );
            } else if hovered && !self.disabled {
                // 悬停 = 中性高亮。**一手源更正**：ListView 行 PointerOver 用的就是
                // `SystemControlHighlightListLowBrush`（= 通用悬停强度 10%），与 AppBarButton 同值；
                // 旧记录「≈30%」在一手字典里没有对应档（L1783 → L307 → L228）。
                scene.fill_rect(theme.indication.hover_tint, row_rect);
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
            scene.label(self.rows[i].clone(), label_rect, fg, style, TextAlign::Left);
        }

        scene.pop_clip();
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// 自绘虚拟化列表：行由控件按数据自画（旧 `render` 只画可见行），**不**把每行变成子节点。
// 旧 API（`select` / `scroll_by` / `scroll_to` / `render`）原样保留给未迁移的 App。
// `Click` 不带坐标 → 点击在 `PointerUp` 用 pos 映射到行；方向键 / Home / End 移动选中并用
// 旧 `scroll_to` 保证选中行可见。滚轮由自身持有偏移，偏移未变（已到端 / 内容不足）时不截停，
// 让外层容器接着滚（XAML ScrollChaining）。

/// 元素树动作：列表选中行改变（点击 / 方向键 / Home / End）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListSelectionChanged(pub usize);

impl MetroList {
    /// 行命中：`pos` 在列表矩形内时按 `scroll` 映射到行索引。
    fn row_at(&self, theme: &MetroTheme, rect: Rect, pos: Point) -> Option<usize> {
        if !rect.contains(pos) {
            return None;
        }
        let row_h = self.row_height(theme);
        if row_h <= 0.0 {
            return None;
        }
        let idx = ((pos.y - rect.origin.y + self.scroll) / row_h).floor();
        if idx < 0.0 {
            return None;
        }
        let i = idx as usize;
        (i < self.rows.len()).then_some(i)
    }

    /// 把选中行滚进视口（视口高 = 控件矩形高）。已可见时不滚。
    fn ensure_selected_visible(&mut self, theme: &MetroTheme, viewport_h: f32) {
        let Some(i) = self.selected else { return };
        let row_h = self.row_height(theme);
        let top = i as f32 * row_h;
        let bottom = top + row_h;
        if top < self.scroll {
            self.scroll_to(theme, viewport_h, top);
        } else if bottom > self.scroll + viewport_h {
            self.scroll_to(theme, viewport_h, bottom - viewport_h);
        }
    }
}

impl kanesumi_element::Widget for MetroList {
    /// 宽取可用宽（无界则 0）；高 = min(内容高, 可用高)，可用高无界时取内容高。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let width = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let content_h = self.content_height(ctx.theme());
        let height = if available.height.is_finite() {
            content_h.min(available.height.max(0.0))
        } else {
            content_h
        };
        kanesumi_core::Size::new(width, height)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // 禁用态由框架维护（框架不给禁用节点投事件）→ 映射进自绘字段后再恢复。
        let saved = self.disabled;
        self.disabled = ctx.state().disabled;
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        self.disabled = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.hovered = self.row_at(ctx.theme(), rect, *pos);
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
                let Some(i) = self.row_at(ctx.theme(), rect, *pos) else {
                    return;
                };
                let before = self.selected;
                self.select(Some(i));
                if self.selected != before {
                    ctx.emit(ListSelectionChanged(i));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            Event::Scroll { dy, .. } => {
                let viewport_h = rect.size.height;
                let before = self.scroll;
                self.scroll_by(ctx.theme(), viewport_h, *dy);
                if self.scroll != before {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => {
                if self.rows.is_empty() {
                    return;
                }
                let last = self.rows.len() - 1;
                let next = match key {
                    Key::Down => Some(self.selected.map_or(0, |i| (i + 1).min(last))),
                    Key::Up => Some(self.selected.map_or(last, |i| i.saturating_sub(1))),
                    Key::Home => Some(0),
                    Key::End => Some(last),
                    _ => None,
                };
                let Some(i) = next else { return };
                let before = self.selected;
                self.select(Some(i));
                if self.selected != before {
                    self.ensure_selected_visible(ctx.theme(), rect.size.height);
                    ctx.emit(ListSelectionChanged(i));
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
            role: kanesumi_element::AccessRole::List,
            name: String::from("列表"),
            value: self.selected.and_then(|i| self.rows.get(i).cloned()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::scroll_view::MetroScrollView;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Stack;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, Modifiers, WidgetId};

    fn harness(rows: usize, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let items = (0..rows).map(|i| format!("行 {i}")).collect();
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(items),
            LayoutProps {
                width: Some(200.0),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn row_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let list = h.tree.get::<MetroList>(id).unwrap();
        let row_h = list.row_height(h.tree.theme());
        let r = h.rect(id);
        Point::new(
            r.origin.x + 10.0,
            r.origin.y - list.scroll + i as f32 * row_h + row_h / 2.0,
        )
    }

    #[test]
    fn click_selects_row_and_reports() {
        let (mut h, id) = harness(10, 100.0);
        h.click_at(row_center(&h, id, 1));
        assert_eq!(
            h.take::<ListSelectionChanged>(),
            vec![(id, ListSelectionChanged(1))]
        );
        assert_eq!(
            h.tree.get::<MetroList>(id).unwrap().selected,
            Some(1)
        );
    }

    #[test]
    fn keyboard_moves_selection_and_scrolls_into_view() {
        let (mut h, id) = harness(10, 100.0);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        for _ in 0..5 {
            assert!(h.key(Key::Down), "方向键应被列表消费");
        }
        let list = h.tree.get::<MetroList>(id).unwrap();
        assert_eq!(list.selected, Some(4), "Down 五次从无选中到第 4 行");
        let row_h = list.row_height(h.tree.theme());
        let top = 4.0 * row_h;
        let bottom = top + row_h;
        assert!(
            list.scroll <= top && bottom <= list.scroll + 100.0,
            "选中行必须滚进视口：scroll={} row=({top},{bottom})",
            list.scroll
        );
        assert!(list.scroll > 0.0, "第 4 行在视口外 → 应滚动");
        h.key(Key::End);
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, Some(9));
        h.key(Key::Home);
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, Some(0));
        assert_eq!(h.take::<ListSelectionChanged>().len(), 7, "5 Down + End + Home");
    }

    #[test]
    fn wheel_bubbles_to_outer_when_not_scrollable() {
        // 列表内容不足一屏（不可滚）→ 滚轮不截停，外层 ScrollView 接着滚（ScrollChaining）。
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        let list = h.tree.insert_with(
            col,
            MetroList::new(vec!["A".into()]),
            LayoutProps {
                height: Some(40.0),
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(
            col,
            crate::button::MetroButton::new("高内容"),
            LayoutProps {
                height: Some(300.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tree.scroll(row_center(&h, list, 0), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(
            h.tree.get::<MetroList>(list).unwrap().scroll,
            0.0,
            "不可滚的列表自身不滚"
        );
        assert!(
            h.tree.get::<MetroScrollView>(sv).unwrap().offset > 0.0,
            "滚动链应冒泡到外层 ScrollView"
        );
    }

    #[test]
    fn wheel_at_end_bubbles_to_outer() {
        // 列表可滚但已到底 → 再向下滚轮不截停，外层接到。
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        let list = h.tree.insert_with(
            col,
            MetroList::new((0..10).map(|i| format!("行 {i}")).collect()),
            LayoutProps {
                height: Some(100.0),
                ..LayoutProps::default()
            },
        );
        // 兄弟高内容：让外层 ScrollView 可滚，列表到底后才能冒泡给它。
        h.tree.insert_with(
            col,
            crate::button::MetroButton::new("高内容"),
            LayoutProps {
                height: Some(300.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        let c = h.rect(list).center();
        // 先滚到列表底部。
        for _ in 0..20 {
            h.tree.scroll(c, 0.0, 50.0, Modifiers::NONE);
        }
        h.frame();
        let at_end = h.tree.get::<MetroList>(list).unwrap().scroll;
        assert!(at_end > 0.0 && at_end >= h.tree.get::<MetroList>(list).unwrap().max_scroll(h.tree.theme(), 100.0) - 0.01);
        // 到底后再滚：列表偏移不变，滚动链冒泡到外层。
        h.tree.scroll(c, 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(h.tree.get::<MetroList>(list).unwrap().scroll, at_end);
        let off = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!(
            off > 0.0,
            "到底后滚动链应冒泡，列表 scroll={at_end} 外层 offset={off}"
        );
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(10, 100.0);
        assert_eq!(h.rect(id).size.width, 200.0);
        assert_eq!(h.rect(id).size.height, 100.0, "高 = min(内容高 400, 可用 100)");
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(vec!["很长很长很长的一行文本".into(); 10]),
            LayoutProps {
                width: Some(40.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(10, 100.0);
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(row_center(&h, id, 1));
        assert!(h.take::<ListSelectionChanged>().is_empty());
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, None);
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
        // 首命令现在是成对 PushClip（容器裁剪，P0-2），故按类型查行文本命令。
        let Some(SceneCommand::Text { color, .. }) = scene
            .commands
            .iter()
            .find(|c| matches!(c, SceneCommand::Text { .. }))
        else {
            panic!("应有行文本命令");
        };
        assert!(color.a < 1.0, "禁用态行文字应降透明度，实际 a={}", color.a);
    }
}
