use std::cell::Cell;
use std::rc::Rc;

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, MetroTypography, Point, Rect, Size};
use kanesumi_element::{
    Event, EventCtx, Key, MeasureCtx, PaintCtx, PointerButton, PopupSpec, UpdateCtx, Widget,
    WidgetId,
};

use crate::popup::{PopupAnim, PopupState, render_overlay};

/// MetroSelectorFlyout —— 下拉选择器（ComboBox 参考）。参 CONTROL_SPEC §8：
/// - 触发器 MinHeight 32、箭头区右 32px、glyph `E70D` 12px；
/// - 面板 MaxDropDownHeight 504（或 15 项）；选中项强调色低透，悬停中性；
/// - 遮罩 0.383s 入 / 0.216s 出，面板 `sheet_appear` 展开。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSelectorFlyout {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    /// 触发器聚焦。
    pub focused: bool,
    /// 触发器内标题文本（占位，未选时显示）。
    pub placeholder: String,
    pub anim: PopupAnim,
    /// 下拉面板矩形（Phase 3 续做方向自适应：Top>0 向下展开，参 ComboBoxHelper）。
    pub panel_rect: Rect,
    /// 面板最大高。
    pub max_dropdown_height: f32,
    /// 元素树下打开的选项面板（`SelectorPanel` 节点）；旧路径不用。
    tree_popup: Option<WidgetId>,
    /// 与面板共享的「本次选中项」：面板点选时写入，触发器在 `PopupClosed` 里读取并回写。
    tree_pick: Rc<Cell<Option<usize>>>,
}

impl Default for MetroSelectorFlyout {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            hovered: None,
            focused: false,
            placeholder: String::new(),
            anim: PopupAnim::new(),
            panel_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            max_dropdown_height: 504.0,
            tree_popup: None,
            tree_pick: Rc::new(Cell::new(None)),
        }
    }
}

impl MetroSelectorFlyout {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    pub fn open(&mut self, at: Rect) {
        self.panel_rect = at;
        self.anim.open();
    }

    pub fn close(&mut self) {
        self.anim.close();
    }

    pub fn toggle(&mut self, at: Rect) {
        if self.anim.is_open() {
            self.close();
        } else {
            self.open(at);
        }
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
    }

    pub fn state(&self) -> PopupState {
        self.anim.state()
    }

    /// 面板高度：min(项数×项高, max_dropdown_height)。
    pub fn panel_height(&self) -> f32 {
        let item_h = 32.0;
        (self.items.len() as f32 * item_h).min(self.max_dropdown_height)
    }

    /// 项命中测试（相对面板）。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.x < self.panel_rect.origin.x
            || pos.x >= self.panel_rect.origin.x + self.panel_rect.size.width
        {
            return None;
        }
        let local_y = pos.y - self.panel_rect.origin.y;
        if local_y < 0.0 || local_y >= self.panel_height() {
            return None;
        }
        Some((local_y / 32.0) as usize)
    }

    /// 渲染触发器 +（可见时）遮罩与面板。
    pub fn render(
        &self,
        theme: &MetroTheme,
        _engine: &TextEngine,
        trigger: Rect,
        screen: Rect,
        scene: &mut Scene,
    ) {
        // 触发器
        let colors = &theme.colors;
        // 聚焦衬底 = 强调色 AccentLow。**一手源**（2026-09-22）：UWP ComboBox 的面板里有一个
        // `HighlightBackground` 矩形，其 Background = `ComboBoxBackgroundUnfocused`
        // → `SystemControlHighlightListAccentLowBrush`（themeresources L666 → L304/L4220：
        // accent 0.6 暗 / 0.4 亮），平时 Opacity 0，**聚焦态动画抬到 1**
        // （generic.xaml L10381-10385 + L10237）；同态边框 `ComboBoxBackgroundBorderBrushFocused`
        // = **全透明**（L667）——即权威的 ComboBox 聚焦**只加衬底、不加边框**。
        // 本库原先的「强调色 24% 衬底 + 1px 边框」：思路对（确有衬底），强度错（24%），
        // 边框属多余（UWP 聚焦时边框是透明的）。
        let bg = if self.focused {
            colors.selection_tint
        } else {
            colors.surface
        };
        scene.fill_rounded_rect(bg, trigger, theme.tokens.corner_radius);

        let style = MetroTypography::metro().body_medium;
        let text = self
            .selected
            .and_then(|i| self.items.get(i))
            .cloned()
            .unwrap_or_else(|| self.placeholder.clone());
        let text_rect = Rect::new(
            trigger.origin.x + 12.0,
            trigger.origin.y + (trigger.size.height - style.line_height) / 2.0,
            (trigger.size.width - 42.0).max(0.0),
            style.line_height,
        );
        let fg = if self.selected.is_some() {
            colors.on_surface
        } else {
            colors.on_surface_variant
        };
        scene.label(text, text_rect, fg, style, TextAlign::Left);

        // 箭头 —— Metro 自绘 chevron（不依赖 Fluent codepoint，参 V7）。
        let arrow_rect = Rect::new(
            trigger.origin.x + trigger.size.width - 22.0,
            trigger.origin.y + (trigger.size.height - 12.0) / 2.0,
            12.0,
            12.0,
        );
        glyph::chevron_down(scene, arrow_rect, colors.on_surface);

        // 弹层
        if !self.anim.is_visible() {
            return;
        }
        render_overlay(theme, &self.anim, screen, scene);
        self.render_panel(theme, scene, 0.0);
    }

    /// 画选项面板（底座 + 项，裁到面板矩形）——不含遮罩。`scroll` = 内容上移量：
    /// 旧路径传 0；元素树面板超出 `max_dropdown_height` 时用内部滚动把余下项移进视口。
    fn render_panel(&self, theme: &MetroTheme, scene: &mut Scene, scroll: f32) {
        crate::popup::render_panel_base(theme, self.panel_rect, self.anim.panel_progress(), scene);
        self.paint_items(theme, scene, scroll);
    }

    /// 画面板项。布局与旧实现一致：项高 32，选中 = `selection_tint`，悬停 = `hover_tint`。
    fn paint_items(&self, theme: &MetroTheme, scene: &mut Scene, scroll: f32) {
        let colors = &theme.colors;
        let style = MetroTypography::metro().body_medium;
        let viewport_top = self.panel_rect.origin.y;
        let viewport_bottom = viewport_top + self.panel_height();
        // 容器语义 = 裁到面板矩形（审计 P0-2）：末项可能只露出半行，不裁会画到面板之外。
        scene.push_clip(self.panel_rect);
        for (i, item) in self.items.iter().enumerate() {
            let y = viewport_top + i as f32 * 32.0 - scroll;
            if y + 32.0 <= viewport_top || y >= viewport_bottom {
                continue; // 完全滚出视口
            }
            let item_rect = Rect::new(self.panel_rect.origin.x, y, self.panel_rect.size.width, 32.0);
            let selected = self.selected == Some(i);
            if selected {
                // 项选中 = 强调色 AccentLow（暗 0.6 / 亮 0.4）。
                // 一手源：`ComboBoxItemBackgroundSelected` = `SystemControlHighlightListAccentLowBrush`
                // （themeresources L648 → L304/L4220）——与 ListView 行选中**同一个笔刷**，
                // 故直接共用 selection_tint，不再另设一个「低透」令牌。
                scene.fill_rect(colors.selection_tint, item_rect);
            } else if self.hovered == Some(i) {
                scene.fill_rect(theme.indication.hover_tint, item_rect);
            }
            let fg = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let text_rect = Rect::new(
                self.panel_rect.origin.x + 11.0,
                y + (32.0 - style.line_height) / 2.0,
                (self.panel_rect.size.width - 22.0).max(0.0),
                style.line_height,
            );
            scene.label(item.clone(), text_rect, fg, style, TextAlign::Left);
        }
        scene.pop_clip();
    }
}

// ── 元素树接入（弹层类控件，参 docs/ELEMENT_MIGRATION.md §8）────────────────────
//
// 触发器只负责「打开」与「展开中外观」；选项面板是覆盖层上的独立节点 `SelectorPanel`，
// 放置 / 点外部关闭 / Esc / 焦点交还全部由框架承担。面板与触发器不互相持有实例：
// 面板点选写入共享格子 `tree_pick`，触发器在 `PopupClosed` 里读取、回写 `selected`
// 并发 `SelectorSelectionChanged`（任何原因的关闭都会到达该事件，据此复位 `tree_popup`）。

/// 元素树动作：选择项变更。载荷 = 新的选中项索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectorSelectionChanged(pub usize);

/// 选项面板（覆盖层节点）。底座与项绘制复用 `MetroSelectorFlyout`；面板高超出
/// `max_dropdown_height` 时内部滚动（框架现无把 `MetroScrollView` 挂进弹层子树的 API，
/// 详见本批报告）。
struct SelectorPanel {
    list: MetroSelectorFlyout,
    /// 面板 → 触发器回写选中项（`PopupClosed` 时读取）。
    pick: Rc<Cell<Option<usize>>>,
    /// 内部滚动偏移（内容坐标系）。
    scroll: f32,
}

impl SelectorPanel {
    fn new(
        items: Vec<String>,
        selected: Option<usize>,
        pick: Rc<Cell<Option<usize>>>,
        width: f32,
        max_height: f32,
    ) -> Self {
        let mut list = MetroSelectorFlyout::new(items);
        list.selected = selected;
        list.max_dropdown_height = max_height;
        list.panel_rect = Rect::new(0.0, 0.0, width, list.panel_height());
        list.anim.open();
        Self {
            list,
            pick,
            scroll: 0.0,
        }
    }

    /// 内容坐标系下的项命中（不受视口高度裁剪限制 —— `panel_height` 只约束可见区）。
    fn item_at(&self, pos: Point) -> Option<usize> {
        let r = self.list.panel_rect;
        if pos.x < r.origin.x || pos.x >= r.origin.x + r.size.width {
            return None;
        }
        let content_y = pos.y - r.origin.y + self.scroll;
        if content_y < 0.0 || content_y >= self.list.items.len() as f32 * 32.0 {
            return None;
        }
        Some((content_y / 32.0) as usize)
    }

    /// 内容总高与视口高之差（无溢出为 0）。
    fn max_scroll(&self) -> f32 {
        (self.list.items.len() as f32 * 32.0 - self.list.panel_height()).max(0.0)
    }

    /// Up/Down 移动悬停项，并把新项滚进视口。
    fn step(&mut self, delta: isize, ctx: &mut EventCtx) {
        let n = self.list.items.len();
        if n == 0 {
            return;
        }
        let next = match self.list.hovered {
            Some(i) => (i as isize + delta).rem_euclid(n as isize) as usize,
            None if delta > 0 => 0,
            None => n - 1,
        };
        self.list.hovered = Some(next);
        let h = self.list.panel_height();
        let top = next as f32 * 32.0;
        if top < self.scroll {
            self.scroll = top;
        } else if top + 32.0 > self.scroll + h {
            self.scroll = top + 32.0 - h;
        }
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
        ctx.invalidate_paint();
    }

    /// 选中第 `index` 项：写入共享格子并关闭自身（触发器在 `PopupClosed` 里回写）。
    fn select(&mut self, ctx: &mut EventCtx, index: usize) {
        self.pick.set(Some(index));
        ctx.close_popup(ctx.id());
    }
}

impl Widget for SelectorPanel {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        Size::new(self.list.panel_rect.size.width, self.list.panel_height())
    }

    fn arrange(&mut self, _ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.list.panel_rect = rect;
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        self.list.render_panel(&theme, scene, self.scroll);
        if matches!(self.list.state(), PopupState::Opening | PopupState::Closing) {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        self.list.update(dt);
        ctx.invalidate_paint();
        if matches!(self.list.state(), PopupState::Opening | PopupState::Closing) {
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerMove { pos } => {
                let hovered = self.item_at(*pos);
                if hovered != self.list.hovered {
                    self.list.hovered = hovered;
                    ctx.invalidate_paint();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                if let Some(i) = self.item_at(*pos) {
                    self.select(ctx, i);
                }
                ctx.set_handled();
            }
            Event::Scroll { dy, .. } => {
                let before = self.scroll;
                self.scroll = (self.scroll + *dy).clamp(0.0, self.max_scroll());
                if self.scroll != before {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => {
                match key {
                    Key::Down => self.step(1, ctx),
                    Key::Up => self.step(-1, ctx),
                    Key::Enter | Key::Char(' ') => {
                        if let Some(i) = self.list.hovered {
                            self.select(ctx, i);
                        }
                    }
                    // Tab / Esc 留给框架（焦点遍历 / 弹层关闭）。
                    _ => return,
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 面板以悬停 / 选中高亮表示当前项，不要框架焦点框。
    fn focus_visual(&self) -> bool {
        false
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: "选项".to_string(),
            value: None,
            checked: None,
        })
    }
}

impl MetroSelectorFlyout {
    /// 打开选项面板（锚在本触发器下缘）。`keyboard` 为真时焦点进面板。
    fn open_panel(&mut self, ctx: &mut EventCtx, keyboard: bool) -> WidgetId {
        let width = ctx.rect().size.width;
        let panel = SelectorPanel::new(
            self.items.clone(),
            self.selected,
            self.tree_pick.clone(),
            width,
            self.max_dropdown_height,
        );
        let id = ctx.open_popup(
            panel,
            PopupSpec {
                gap: crate::popup::popup_gap(),
                ..PopupSpec::default()
            },
        );
        ctx.focus_widget(id, keyboard);
        id
    }
}

impl Widget for MetroSelectorFlyout {
    /// 固有尺寸：显示文本（选中项或占位）+ 左 12 + 箭头区 30，高 32。
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        let style = MetroTypography::metro().body_medium;
        let display = self
            .selected
            .and_then(|i| self.items.get(i))
            .cloned()
            .unwrap_or_else(|| self.placeholder.clone());
        let w = ctx.engine().measure(&display, style.size) + 12.0 + 30.0;
        Size::new(w.max(120.0), 32.0)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let saved = self.focused;
        self.focused = ctx.state().focused;
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), ctx.rect(), ctx.surface(), scene);
        self.focused = saved;
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let keyboard = match event {
            Event::Click => false,
            Event::KeyDown {
                key: Key::Enter | Key::Char(' '),
                ..
            } => true,
            // 含 Alt+Down（ComboBox 惯例）；面板已开时该键在下面收起。
            Event::KeyDown { key: Key::Down, .. } => true,
            Event::PopupClosed { popup } => {
                if self.tree_popup == Some(*popup) {
                    self.tree_popup = None;
                    if let Some(i) = self.tree_pick.replace(None) {
                        self.selected = Some(i);
                        ctx.emit(SelectorSelectionChanged(i));
                    }
                    ctx.invalidate_paint();
                }
                return;
            }
            _ => return,
        };
        match self.tree_popup.take() {
            Some(p) => ctx.close_popup(p),
            None => {
                let p = self.open_panel(ctx, keyboard);
                self.tree_popup = Some(p);
            }
        }
        ctx.invalidate_paint();
        ctx.set_handled();
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: self.placeholder.clone(),
            value: self.selected.and_then(|i| self.items.get(i)).cloned(),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 400.0);
        let mut sel = MetroSelectorFlyout::new(vec![
            "Alpha".into(),
            "Bravo".into(),
            "Charlie".into(),
        ]);
        sel.placeholder = "选择…".into();
        let id = h.tree.insert_with(
            h.root(),
            sel,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(10.0, 10.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_opens_panel_below_and_item_selects() {
        let (mut h, id) = harness();
        h.click(id);
        let p = h.tree.popups().next().expect("点击打开选项面板");
        let pr = h.rect(p);
        assert!(pr.origin.y >= h.rect(id).bottom(), "面板在触发器下方 {pr:?}");
        assert!(
            h.tree
                .get::<MetroSelectorFlyout>(id)
                .unwrap()
                .tree_popup
                .is_some()
        );
        // 点第二项
        h.click_at(Point::new(pr.origin.x + 20.0, pr.origin.y + 32.0 + 16.0));
        assert_eq!(
            h.take::<SelectorSelectionChanged>(),
            vec![(id, SelectorSelectionChanged(1))]
        );
        let sel = h.tree.get::<MetroSelectorFlyout>(id).unwrap();
        assert_eq!(sel.selected, Some(1), "触发器回写选中");
        assert!(sel.tree_popup.is_none(), "面板关闭后复位");
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn keyboard_opens_selects_and_returns_focus() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Enter);
        let p = h.tree.popups().next().expect("Enter 打开");
        assert_eq!(h.tree.focused(), Some(p), "键盘打开焦点进面板");
        h.key(Key::Down); // 首项
        h.key(Key::Down); // 次项
        h.key(Key::Enter);
        assert_eq!(
            h.take::<SelectorSelectionChanged>(),
            vec![(id, SelectorSelectionChanged(1))]
        );
        assert_eq!(h.tree.focused(), Some(id), "关闭后焦点回触发器");
    }

    #[test]
    fn escape_dismisses_without_selection_change() {
        let (mut h, id) = harness();
        h.click(id);
        assert!(h.tree.popups().next().is_some());
        h.key(Key::Escape);
        assert!(h.tree.popups().next().is_none());
        assert!(h.take::<SelectorSelectionChanged>().is_empty());
        let sel = h.tree.get::<MetroSelectorFlyout>(id).unwrap();
        assert_eq!(sel.selected, None);
        assert!(sel.tree_popup.is_none());
    }

    #[test]
    fn outside_click_dismisses() {
        let (mut h, id) = harness();
        h.click(id);
        h.click_at(Point::new(380.0, 380.0));
        assert!(h.tree.popups().next().is_none());
        assert!(h.take::<SelectorSelectionChanged>().is_empty());
        assert!(
            h.tree
                .get::<MetroSelectorFlyout>(id)
                .unwrap()
                .tree_popup
                .is_none()
        );
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, id) = harness();
        h.click(id);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
        let p = h.tree.popups().next().unwrap();
        h.assert_paint_within(p, Insets::ZERO);
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
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn toggle_opens_closes() {
        let mut sel = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        sel.toggle(Rect::new(0.0, 40.0, 160.0, 160.0));
        for _ in 0..120 {
            sel.update(1.0 / 60.0);
        }
        assert!(sel.anim.is_open());
        sel.toggle(Rect::new(0.0, 40.0, 160.0, 160.0));
        for _ in 0..120 {
            sel.update(1.0 / 60.0);
        }
        assert_eq!(sel.state(), PopupState::Closed);
    }

    #[test]
    fn panel_height_capped() {
        let many: Vec<String> = (0..30).map(|i| format!("Item {i}")).collect();
        let sel = MetroSelectorFlyout::new(many);
        assert_eq!(sel.panel_height(), 504.0, "30 项截断到 504");
        let few = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        assert_eq!(few.panel_height(), 64.0);
    }

    #[test]
    fn item_at_maps_rows() {
        let sel = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        let rect = Rect::new(10.0, 100.0, 160.0, 64.0);
        // 模拟 panel_rect
        let mut sel = sel;
        sel.panel_rect = rect;
        assert_eq!(sel.item_at(Point::new(20.0, 110.0)), Some(0));
        assert_eq!(sel.item_at(Point::new(20.0, 142.0)), Some(1));
        assert_eq!(sel.item_at(Point::new(20.0, 200.0)), None, "面板外");
    }

    #[test]
    fn renders_trigger_when_closed() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut sel = MetroSelectorFlyout::new(vec!["Alpha".into()]);
        sel.selected = Some(0);
        let mut scene = Scene::default();
        sel.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 160.0, 32.0),
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 触发器底 + 选中文本 + 自绘 chevron（关闭时无遮罩，无 Text 箭头）
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        let triangles = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(texts, 1, "只有选中文本，箭头改自绘");
        assert_eq!(triangles, 1, "chevron 是 Triangle");
    }
}
