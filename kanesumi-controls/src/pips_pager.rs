// MetroPipsPager —— 圆点分页指示。参 CONTROL_SPEC §15。
//
// 移植自 microsoft-ui-xaml/dev/PipsPager（PipsPager.cpp + PipsPager_themeresources.xaml）：
// - Pip 命中区：横排 12×20 / 纵排 20×12；正常胶囊条高 4、选中高 6（glyph font 4/6）；
// - Nav 按钮 20×20、glyph 8、Pressed 缩放 0.875；
// - Nav 按钮可见性：`show_nav` 或 `pointer_over` 时显示；首/尾边缘对应按钮隐藏。
// - 选中色：上游同色仅放大；Kanesumi 适配选中用强调色（铁律 5）。
// Kanesumi 自绘胶囊条（V7 不依赖 Segoe MDL2 EA3B）。

use kanesumi_canvas::Scene;
use kanesumi_canvas::glyph;
use kanesumi_core::{CornerRadius, MetroTheme, Point, Rect};

/// Pips 方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipsOrientation {
    Horizontal,
    Vertical,
}

/// Pip 命中区（横排 12×20 / 纵排 20×12）。
const PIP_HIT_W: f32 = 12.0;
const PIP_HIT_H: f32 = 20.0;
/// 正常胶囊条「主轴向」厚度（glyph font 4）。
const PIP_NORMAL_BAR: f32 = 4.0;
/// 选中/悬停胶囊条厚度（glyph font 6）。
const PIP_SELECTED_BAR: f32 = 6.0;
/// Nav 按钮边长（20）。
const NAV_SIZE: f32 = 20.0;

/// 分页点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipsAction {
    None,
    /// 点击某页（选中）。
    Select(usize),
    Prev,
    Next,
}

/// MetroPipsPager —— 圆点分页。参 CONTROL_SPEC §15。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroPipsPager {
    pub number_of_pages: usize,
    pub selected_index: usize,
    pub orientation: PipsOrientation,
    /// 最大可见 pip（0 = 全部可见）。
    pub max_visible_pips: usize,
    /// Nav 按钮是否常显（false = 仅 hover 显示）。
    pub show_nav: bool,
    /// 指针是否在控件内（驱动 Nav 显示）。
    pub pointer_over: bool,
    /// 当前 hover 的 pip。
    pub hovered_pip: Option<usize>,
    pub nav_prev_hovered: bool,
    pub nav_next_hovered: bool,
}

impl Default for MetroPipsPager {
    fn default() -> Self {
        Self {
            number_of_pages: 0,
            selected_index: 0,
            orientation: PipsOrientation::Horizontal,
            max_visible_pips: 0,
            show_nav: false,
            pointer_over: false,
            hovered_pip: None,
            nav_prev_hovered: false,
            nav_next_hovered: false,
        }
    }
}

impl MetroPipsPager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 夹紧选中索引到合法范围（OnSelectedPageIndexChanged 语义）。
    pub fn clamp_selection(&mut self) {
        if self.number_of_pages == 0 {
            self.selected_index = 0;
            return;
        }
        self.selected_index = self.selected_index.min(self.number_of_pages - 1);
    }

    /// 设置选中页（越界夹紧）。返回是否变化。
    pub fn select(&mut self, index: usize) -> bool {
        let old = self.selected_index;
        self.selected_index = index;
        self.clamp_selection();
        self.selected_index != old
    }

    /// Pip 命中区尺寸。
    pub fn pip_hit_size(&self) -> (f32, f32) {
        match self.orientation {
            PipsOrientation::Horizontal => (PIP_HIT_W, PIP_HIT_H),
            PipsOrientation::Vertical => (PIP_HIT_H, PIP_HIT_W),
        }
    }

    /// 当前可见 pip 数（window：min(pages, max_visible)）。
    pub fn visible_count(&self) -> usize {
        if self.number_of_pages == 0 {
            return 0;
        }
        if self.max_visible_pips == 0 || self.max_visible_pips >= self.number_of_pages {
            self.number_of_pages
        } else {
            self.max_visible_pips
        }
    }

    /// 滚动偏移（主轴向，px）。选中页居中；首尾夹紧（CalculateScrollViewerSize 语义）。
    pub fn scroll_offset(&self) -> f32 {
        let (w, _) = self.pip_hit_size();
        let total = self.number_of_pages as f32;
        let view = self.visible_count() as f32;
        let max_offset = (total - view) * w;
        if max_offset <= 0.0 {
            return 0.0;
        }
        let target = self.selected_index as f32 * w + w / 2.0 - view * w / 2.0;
        target.clamp(0.0, max_offset)
    }

    /// 主轴向总量尺寸（pips 段）。
    pub fn measure(&self) -> (f32, f32) {
        let (w, h) = self.pip_hit_size();
        let count = self.visible_count();
        match self.orientation {
            PipsOrientation::Horizontal => (count as f32 * w, h),
            PipsOrientation::Vertical => (h, count as f32 * w),
        }
    }

    /// 可见 pip 矩形集（`rect` = 控件区域，pips 段居中）。
    pub fn pip_rects(&self, rect: Rect) -> impl Iterator<Item = (usize, Rect)> + '_ {
        let (w, h) = self.pip_hit_size();
        let view = self.visible_count();
        let start = self.first_visible_index();
        let (total_w, total_h) = self.measure();
        let origin = match self.orientation {
            PipsOrientation::Horizontal => (
                rect.origin.x + (rect.size.width - total_w) / 2.0,
                rect.origin.y + (rect.size.height - h) / 2.0,
            ),
            PipsOrientation::Vertical => (
                rect.origin.x + (rect.size.width - h) / 2.0,
                rect.origin.y + (rect.size.height - total_h) / 2.0,
            ),
        };
        (0..view).map(move |i| {
            let idx = start + i;
            let r = match self.orientation {
                PipsOrientation::Horizontal => Rect::new(origin.0 + i as f32 * w, origin.1, w, h),
                PipsOrientation::Vertical => Rect::new(origin.0, origin.1 + i as f32 * w, h, w),
            };
            (idx, r)
        })
    }

    fn first_visible_index(&self) -> usize {
        if self.max_visible_pips == 0 || self.max_visible_pips >= self.number_of_pages {
            0
        } else {
            let offset = self.scroll_offset();
            let (w, _) = self.pip_hit_size();
            ((offset / w).round() as usize).min(self.number_of_pages - self.visible_count())
        }
    }

    /// 单条 pip 胶囊条矩形（主轴向宽度 = 命中区宽，厚度按选中/悬停/正常）。
    fn pip_bar_rect(&self, pip_rect: Rect, selected: bool) -> Rect {
        let (w, h) = (pip_rect.size.width, pip_rect.size.height);
        match self.orientation {
            PipsOrientation::Horizontal => {
                let bar_h = if selected {
                    PIP_SELECTED_BAR
                } else {
                    PIP_NORMAL_BAR
                };
                Rect::new(
                    pip_rect.origin.x,
                    pip_rect.origin.y + (h - bar_h) / 2.0,
                    w,
                    bar_h,
                )
            }
            PipsOrientation::Vertical => {
                let bar_w = if selected {
                    PIP_SELECTED_BAR
                } else {
                    PIP_NORMAL_BAR
                };
                Rect::new(
                    pip_rect.origin.x + (w - bar_w) / 2.0,
                    pip_rect.origin.y,
                    bar_w,
                    h,
                )
            }
        }
    }

    /// Nav 按钮 rect（None = 不显示）。
    pub fn prev_rect(&self, rect: Rect) -> Option<Rect> {
        if !self.nav_visible() || self.selected_index == 0 {
            return None;
        }
        Some(self.nav_rect_for(rect, true))
    }

    pub fn next_rect(&self, rect: Rect) -> Option<Rect> {
        if !self.nav_visible()
            || self.number_of_pages == 0
            || self.selected_index >= self.number_of_pages - 1
        {
            return None;
        }
        Some(self.nav_rect_for(rect, false))
    }

    fn nav_visible(&self) -> bool {
        self.show_nav || self.pointer_over
    }

    fn nav_rect_for(&self, rect: Rect, is_prev: bool) -> Rect {
        match self.orientation {
            PipsOrientation::Horizontal => {
                let (total_w, _) = self.measure();
                let center = rect.origin.x + (rect.size.width - total_w) / 2.0;
                let y = rect.origin.y + (rect.size.height - NAV_SIZE) / 2.0;
                if is_prev {
                    Rect::new(center - NAV_SIZE - 4.0, y, NAV_SIZE, NAV_SIZE)
                } else {
                    Rect::new(center + total_w + 4.0, y, NAV_SIZE, NAV_SIZE)
                }
            }
            PipsOrientation::Vertical => {
                let (_, total_h) = self.measure();
                let center = rect.origin.y + (rect.size.height - total_h) / 2.0;
                let x = rect.origin.x + (rect.size.width - NAV_SIZE) / 2.0;
                if is_prev {
                    Rect::new(x, center - NAV_SIZE - 4.0, NAV_SIZE, NAV_SIZE)
                } else {
                    Rect::new(x, center + total_h + 4.0, NAV_SIZE, NAV_SIZE)
                }
            }
        }
    }

    /// 命中测试：优先 Nav，再 pip。
    pub fn click(&self, rect: Rect, pos: Point) -> PipsAction {
        if let Some(p) = self.prev_rect(rect)
            && p.contains(pos)
        {
            return PipsAction::Prev;
        }
        if let Some(n) = self.next_rect(rect)
            && n.contains(pos)
        {
            return PipsAction::Next;
        }
        for (i, r) in self.pip_rects(rect) {
            if r.contains(pos) {
                return PipsAction::Select(i);
            }
        }
        PipsAction::None
    }

    /// 应用点击（返回事件 + 是否更新选中）。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> PipsAction {
        match self.click(rect, pos) {
            PipsAction::Select(i) => {
                self.select(i);
                PipsAction::Select(i)
            }
            PipsAction::Prev => {
                let changed = self.selected_index > 0;
                if changed {
                    self.selected_index -= 1;
                }
                if changed {
                    PipsAction::Prev
                } else {
                    PipsAction::None
                }
            }
            PipsAction::Next => {
                let changed = self.selected_index + 1 < self.number_of_pages;
                if changed {
                    self.selected_index += 1;
                }
                if changed {
                    PipsAction::Next
                } else {
                    PipsAction::None
                }
            }
            PipsAction::None => PipsAction::None,
        }
    }

    /// 渲染 pips +（可选）Nav 按钮。
    pub fn render(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        for (i, r) in self.pip_rects(rect) {
            let selected = i == self.selected_index;
            let hovered = self.hovered_pip == Some(i);
            let bar = self.pip_bar_rect(r, selected || hovered);
            let color = if selected {
                colors.primary
            } else if hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.fill_rounded_rect(color, bar, CornerRadius::Capsule);
        }

        // Nav 按钮
        if let Some(p) = self.prev_rect(rect) {
            self.render_nav_button(theme, p, self.nav_prev_hovered, true, scene);
        }
        if let Some(n) = self.next_rect(rect) {
            self.render_nav_button(theme, n, self.nav_next_hovered, false, scene);
        }
    }

    fn render_nav_button(
        &self,
        theme: &MetroTheme,
        rect: Rect,
        hovered: bool,
        is_prev: bool,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        if hovered {
            scene.fill_rounded_rect(
                theme.indication.hover_tint,
                rect,
                CornerRadius::Slight,
            );
        }
        let chevron_rect = Rect::new(
            rect.origin.x + rect.size.width * 0.25,
            rect.origin.y + rect.size.height * 0.25,
            rect.size.width * 0.5,
            rect.size.height * 0.5,
        );
        if is_prev {
            glyph::chevron_left(scene, chevron_rect, colors.on_surface);
        } else {
            glyph::chevron_right(scene, chevron_rect, colors.on_surface);
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3）─────────────────────────────────
//
// 圆点分页：点 pip 选中，两侧 Nav 翻页。盘点：Nav 按钮在窄 rect 会外扩（旧 `measure` 只算
// pips 段宽），故在 `Widget::paint` 里把自身绘制夹进 rect（§6）。指针 hover 维护
// `pointer_over` / `hovered_pip` / `nav_*_hovered`（Nav 仅 hover 显示）。

/// 元素树动作：当前页被用户改变。携带新的选中页（0 基）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipsPageChanged(pub usize);

impl kanesumi_element::Widget for MetroPipsPager {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let (w, h) = MetroPipsPager::measure(self);
        kanesumi_core::Size::new(w, h)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // §6 修正：Nav 按钮会画到 pips 段之外（窄 rect 越界）；成对把自身绘制夹进 rect。
        scene.push_clip(ctx.rect());
        self.render(ctx.theme(), ctx.rect(), scene);
        scene.pop_clip();
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerEnter => {
                self.pointer_over = true;
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.pointer_over = false;
                self.hovered_pip = None;
                self.nav_prev_hovered = false;
                self.nav_next_hovered = false;
                ctx.invalidate_paint();
            }
            Event::PointerMove { pos } => {
                let rect = ctx.rect();
                let was_over = self.pointer_over;
                self.pointer_over = true;
                let mut hovered_pip = None;
                for (i, r) in self.pip_rects(rect) {
                    if r.contains(*pos) {
                        hovered_pip = Some(i);
                        break;
                    }
                }
                let prev = self.prev_rect(rect).is_some_and(|r| r.contains(*pos));
                let next = self.next_rect(rect).is_some_and(|r| r.contains(*pos));
                if !was_over
                    || self.hovered_pip != hovered_pip
                    || self.nav_prev_hovered != prev
                    || self.nav_next_hovered != next
                {
                    self.hovered_pip = hovered_pip;
                    self.nav_prev_hovered = prev;
                    self.nav_next_hovered = next;
                    ctx.invalidate_paint();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let action = self.handle_click(ctx.rect(), *pos);
                if action != PipsAction::None {
                    ctx.emit(PipsPageChanged(self.selected_index));
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => match key {
                Key::Left => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                        ctx.emit(PipsPageChanged(self.selected_index));
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                }
                Key::Right => {
                    if self.number_of_pages > 0 && self.selected_index + 1 < self.number_of_pages {
                        self.selected_index += 1;
                        ctx.emit(PipsPageChanged(self.selected_index));
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: String::from("分页指示"),
            value: Some(format!("{} / {}", self.selected_index + 1, self.number_of_pages)),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroPipsPager {
                number_of_pages: 5,
                selected_index: 0,
                ..MetroPipsPager::default()
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    fn pip_rects(h: &TestHarness, id: WidgetId) -> Vec<(usize, Rect)> {
        h.tree
            .get::<MetroPipsPager>(id)
            .unwrap()
            .pip_rects(h.rect(id))
            .collect()
    }

    #[test]
    fn click_pip_changes_page() {
        let (mut h, id) = harness(LayoutProps::default());
        h.move_to(h.center(id));
        h.frame();
        let target = pip_rects(&h, id)[3].1.center();
        h.click_at(target);
        assert_eq!(
            h.take::<PipsPageChanged>(),
            vec![(id, PipsPageChanged(3))]
        );
        assert_eq!(h.tree.get::<MetroPipsPager>(id).unwrap().selected_index, 3);
    }

    #[test]
    fn nav_buttons_change_page_on_hover() {
        // Nav 在 pips 段两侧 —— 需宿主给足宽度（窄 rect 下按 §6 裁掉，不可达）。
        let (mut h, id) = harness(LayoutProps {
            width: Some(400.0),
            ..LayoutProps::default()
        });
        h.move_to(h.center(id));
        h.frame();
        let next = {
            let p = h.tree.get::<MetroPipsPager>(id).unwrap();
            p.next_rect(h.rect(id)).expect("hover 时显示 Next")
        };
        assert!(h.rect(id).contains(next.center()), "宽 rect 下 Next 落在控件内");
        h.click_at(next.center());
        assert_eq!(
            h.take::<PipsPageChanged>(),
            vec![(id, PipsPageChanged(1))]
        );
    }

    #[test]
    fn arrow_keys_change_page() {
        let (mut h, id) = harness(LayoutProps::default());
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Left); // 首页：无变化、不发动作
        assert!(h.take::<PipsPageChanged>().is_empty());
        h.key(Key::Right);
        h.key(Key::Right);
        assert_eq!(
            h.take::<PipsPageChanged>(),
            vec![(id, PipsPageChanged(1)), (id, PipsPageChanged(2))]
        );
    }

    #[test]
    fn passes_insurance_checks_including_hover_and_squeezed() {
        let (mut h, id) = harness(LayoutProps::default());
        // 悬停时两侧 Nav 会画出 pips 段之外 —— 必须仍落在 rect 内。
        h.move_to(h.center(id));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);

        let narrow = h.tree.insert_with(
            h.root(),
            MetroPipsPager {
                number_of_pages: 5,
                selected_index: 2,
                show_nav: true,
                ..MetroPipsPager::default()
            },
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(narrow);
        h.assert_paint_within(narrow, Insets::ZERO);
    }

    #[test]
    fn disabled_pager_ignores_input() {
        let (mut h, id) = harness(LayoutProps::default());
        h.tree.set_enabled(id, false);
        h.frame();
        h.click(id);
        assert!(h.take::<PipsPageChanged>().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn pager() -> MetroPipsPager {
        MetroPipsPager {
            number_of_pages: 5,
            selected_index: 2,
            ..MetroPipsPager::default()
        }
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 400.0, 40.0)
    }

    #[test]
    fn clamp_selection_validates() {
        let mut p = pager();
        p.selected_index = 99;
        p.clamp_selection();
        assert_eq!(p.selected_index, 4);
        p.number_of_pages = 0;
        p.selected_index = 3;
        p.clamp_selection();
        assert_eq!(p.selected_index, 0);
    }

    #[test]
    fn select_clamps() {
        let mut p = pager();
        assert!(p.select(1));
        assert_eq!(p.selected_index, 1);
        assert!(!p.select(1), "同值不变化");
        p.select(99);
        assert_eq!(p.selected_index, 4);
    }

    #[test]
    fn pip_hit_size_by_orientation() {
        let p = pager();
        assert_eq!(p.pip_hit_size(), (12.0, 20.0));
        let v = MetroPipsPager {
            orientation: PipsOrientation::Vertical,
            ..pager()
        };
        assert_eq!(v.pip_hit_size(), (20.0, 12.0));
    }

    #[test]
    fn visible_count_respects_max() {
        let p = pager();
        assert_eq!(p.visible_count(), 5, "max=0 → 全部");
        let p2 = MetroPipsPager {
            max_visible_pips: 3,
            ..pager()
        };
        assert_eq!(p2.visible_count(), 3);
    }

    #[test]
    fn click_selects_pip() {
        let mut p = pager();
        let rects: Vec<_> = p.pip_rects(area()).collect();
        let (idx, r) = rects[0];
        let center = r.center();
        let action = p.handle_click(area(), center);
        assert_eq!(action, PipsAction::Select(idx));
        assert_eq!(p.selected_index, idx);
    }

    #[test]
    fn click_outside_is_none() {
        let p = pager();
        assert_eq!(p.click(area(), Point::new(5.0, 5.0)), PipsAction::None);
    }

    #[test]
    fn nav_buttons_hidden_by_default() {
        let p = pager();
        assert_eq!(p.prev_rect(area()), None, "无 hover 不显示 Nav");
        assert_eq!(p.next_rect(area()), None);
        let mut p2 = pager();
        p2.pointer_over = true;
        assert!(p2.prev_rect(area()).is_some());
        assert!(p2.next_rect(area()).is_some());
    }

    #[test]
    fn prev_next_edges_hidden() {
        let mut p = pager();
        p.pointer_over = true;
        assert!(p.prev_rect(area()).is_some());
        assert!(p.next_rect(area()).is_some());
        p.selected_index = 0;
        assert_eq!(p.prev_rect(area()), None, "首页隐藏 Prev");
        p.selected_index = 4;
        assert_eq!(p.next_rect(area()), None, "末页隐藏 Next");
    }

    #[test]
    fn handle_prev_next_changes() {
        let mut p = pager();
        p.pointer_over = true;
        let prev = p.prev_rect(area()).unwrap();
        assert_eq!(p.handle_click(area(), prev.center()), PipsAction::Prev);
        assert_eq!(p.selected_index, 1);
        let next = p.next_rect(area()).unwrap();
        assert_eq!(p.handle_click(area(), next.center()), PipsAction::Next);
        assert_eq!(p.selected_index, 2);
    }

    #[test]
    fn render_emits_pills() {
        let theme = MetroTheme::ether_dark();
        let p = pager();
        let mut scene = Scene::default();
        p.render(&theme, area(), &mut scene);
        let fills: Vec<_> = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .collect();
        assert_eq!(fills.len(), 5, "5 个 pip");
        // 选中 pip 用强调色
        let has_accent = scene.commands.iter().any(
            |c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.primary),
        );
        assert!(has_accent, "选中 pip 应为强调色");
    }
}
