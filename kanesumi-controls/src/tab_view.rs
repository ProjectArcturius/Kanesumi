// MetroTabView —— Chrome 式标签页。参 CONTROL_SPEC §25。
//
// 移植自 microsoft-ui-xaml/dev/TabView（TabView.cpp + TabView_themeresources.xaml）：
// - Item MinHeight 32、MinWidth 100、MaxWidth 240；等宽分配（clamp(avail/len, 100, 240)）；
// - Header Padding 0,8,0,0；Close 32×24（hovered/selected 显示）、Add 32×24；
// - 选中底 surface_variant / 前景 on_surface；未选 on_surface_variant。
// 拖拽 Reorder 暂略（Phase 3 续）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextVAlign;
use kanesumi_core::MetroTypography;
use kanesumi_core::{Color, MetroTheme, Point, Rect};

/// 顶部留白（TabViewHeaderPadding 0,8,0,0）。
pub const TABVIEW_HEADER_PAD: f32 = 8.0;
/// Tab 最小高（TabViewItemMinHeight = 32）。
pub const TABVIEW_ITEM_MIN_H: f32 = 32.0;
/// Tab 最小宽（100）。
pub const TABVIEW_ITEM_MIN_W: f32 = 100.0;
/// Tab 最大宽（240）。
pub const TABVIEW_ITEM_MAX_W: f32 = 240.0;
/// 标题字号（12）。
pub const TABVIEW_ITEM_FONT: f32 = 12.0;
/// Close 按钮宽（32）。
pub const TABVIEW_CLOSE_W: f32 = 32.0;
/// Close 按钮高（24）。
pub const TABVIEW_CLOSE_H: f32 = 24.0;
/// Add 按钮宽（32）。
pub const TABVIEW_ADD_W: f32 = 32.0;

/// 命中目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabHover {
    Tab(usize),
    Close(usize),
    Add,
}

/// 点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabViewAction {
    None,
    Select(usize),
    Close(usize),
    Add,
}

/// MetroTabView —— 标签页。参 CONTROL_SPEC §25。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTabView {
    pub tabs: Vec<String>,
    pub selected_index: usize,
    /// 显示 Close 按钮。
    pub closable: bool,
    /// 显示 Add（＋）按钮。
    pub add_enabled: bool,
    /// 滚动偏移（overflow 时）。
    pub scroll_offset: f32,
    pub hovered: Option<TabHover>,
}

impl Default for MetroTabView {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            selected_index: 0,
            closable: true,
            add_enabled: true,
            scroll_offset: 0.0,
            hovered: None,
        }
    }
}

impl MetroTabView {
    pub fn new(tabs: Vec<String>) -> Self {
        Self {
            tabs,
            ..Self::default()
        }
    }

    /// 夹紧选中。
    pub fn clamp_selection(&mut self) {
        if self.tabs.is_empty() {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.tabs.len() - 1);
        }
    }

    /// 单个 tab 宽（等宽分配，clamp 100..240）。
    pub fn tab_width(&self, avail_w: f32) -> f32 {
        let len = self.tabs.len().max(1) as f32;
        (avail_w / len).clamp(TABVIEW_ITEM_MIN_W, TABVIEW_ITEM_MAX_W)
    }

    /// tab 段 + add 按钮总宽（含滚动裁剪前的自然宽）。
    pub fn total_width(&self, avail_w: f32) -> f32 {
        let tw = self.tab_width(avail_w);
        self.tabs.len() as f32 * tw + if self.add_enabled { TABVIEW_ADD_W } else { 0.0 }
    }

    /// 可滚动范围 [0, max]。
    pub fn max_scroll(&self, avail_w: f32) -> f32 {
        (self.total_width(avail_w) - avail_w).max(0.0)
    }

    /// 滚动（夹紧）。
    pub fn scroll_by(&mut self, delta: f32, avail_w: f32) {
        self.scroll_offset = (self.scroll_offset + delta).clamp(0.0, self.max_scroll(avail_w));
    }

    /// 第 k 个 tab rect（含偏移）。
    pub fn tab_rect(&self, rect: Rect, k: usize) -> Rect {
        let tw = self.tab_width(rect.size.width);
        Rect::new(
            rect.origin.x + k as f32 * tw - self.scroll_offset,
            rect.origin.y + TABVIEW_HEADER_PAD,
            tw,
            TABVIEW_ITEM_MIN_H,
        )
    }

    /// 第 k 个 tab 的 Close rect。
    pub fn close_rect(&self, rect: Rect, k: usize) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let tr = self.tab_rect(rect, k);
        Some(Rect::new(
            tr.right() - TABVIEW_CLOSE_W,
            tr.origin.y + (tr.size.height - TABVIEW_CLOSE_H) / 2.0,
            TABVIEW_CLOSE_W,
            TABVIEW_CLOSE_H,
        ))
    }

    /// Add（＋）按钮 rect。
    pub fn add_rect(&self, rect: Rect) -> Option<Rect> {
        if !self.add_enabled {
            return None;
        }
        let tw = self.tab_width(rect.size.width);
        let x = rect.origin.x + self.tabs.len() as f32 * tw - self.scroll_offset;
        Some(Rect::new(
            x,
            rect.origin.y + TABVIEW_HEADER_PAD,
            TABVIEW_ADD_W,
            TABVIEW_ITEM_MIN_H,
        ))
    }

    /// 命中。
    pub fn hit(&self, rect: Rect, pos: Point) -> TabViewAction {
        // 控件矩形外一律不命中（2026-09-22 审计 P0-2）：页签横向滚动/`scroll_offset`
        // 会让越界页签的矩形仍在坐标系里，旧实现只看子矩形 → 能从控件外点中不可见页签。
        if !rect.contains(pos) {
            return TabViewAction::None;
        }
        if let Some(a) = self.add_rect(rect)
            && a.contains(pos)
        {
            return TabViewAction::Add;
        }
        for k in 0..self.tabs.len() {
            if let Some(c) = self.close_rect(rect, k)
                && c.contains(pos)
            {
                return TabViewAction::Close(k);
            }
            if self.tab_rect(rect, k).contains(pos) {
                return TabViewAction::Select(k);
            }
        }
        TabViewAction::None
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        // 同 `hit`：矩形外不产生 hover（否则滚动出视口的页签仍会高亮）。
        if !rect.contains(pos) {
            self.hovered = None;
            return;
        }
        if let Some(a) = self.add_rect(rect)
            && a.contains(pos)
        {
            self.hovered = Some(TabHover::Add);
            return;
        }
        for k in 0..self.tabs.len() {
            if let Some(c) = self.close_rect(rect, k)
                && c.contains(pos)
            {
                self.hovered = Some(TabHover::Close(k));
                return;
            }
            if self.tab_rect(rect, k).contains(pos) {
                self.hovered = Some(TabHover::Tab(k));
                return;
            }
        }
        self.hovered = None;
    }

    /// 应用点击：Select / Close（移除 tab）/ Add。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> TabViewAction {
        match self.hit(rect, pos) {
            TabViewAction::Select(k) => {
                self.selected_index = k;
                TabViewAction::Select(k)
            }
            TabViewAction::Close(k) => {
                if k < self.tabs.len() {
                    self.tabs.remove(k);
                    self.clamp_selection();
                }
                TabViewAction::Close(k)
            }
            TabViewAction::Add => TabViewAction::Add,
            TabViewAction::None => TabViewAction::None,
        }
    }

    /// 渲染 tab strip（＋ Add 按钮）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = MetroTypography::metro().body_small;

        // 容器语义 = 裁到自身矩形（2026-09-22 审计 P0-2）：页签横向滚动后越界项仍会被绘制。
        scene.push_clip(rect);

        for k in 0..self.tabs.len() {
            let tr = self.tab_rect(rect, k);
            let selected = k == self.selected_index;
            let hovered = self.hovered == Some(TabHover::Tab(k));
            let close_hovered = self.hovered == Some(TabHover::Close(k));

            // 底
            let bg = if selected {
                colors.surface_variant
            } else if hovered {
                theme.indication.subtle_tint
            } else {
                Color::TRANSPARENT
            };
            if bg.a > 0.0 {
                scene.fill_rect(bg, tr);
            }

            // 标题（左 padding 8/9）：可用宽由 tab 矩形派生，单行省略号收束
            // （旧实现把 rect 宽写成 `label_w.min(title_w)`：量测宽当绘制宽 —— 审计 P0-1）。
            let pad = if selected { 9.0 } else { 8.0 };
            let title_w = if self.closable && (selected || hovered) {
                tr.size.width - pad - TABVIEW_CLOSE_W
            } else {
                tr.size.width - pad
            };
            let fg = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.label(
                self.tabs[k].clone(),
                // 纵向交给 TextVAlign::Center（与手算 (h−lh)/2 逐位同值）。参 o4 纵向对齐。
                Rect::new(
                    tr.origin.x + pad,
                    tr.origin.y,
                    title_w.max(0.0),
                    tr.size.height,
                ),
                fg,
                style.with_v_align(TextVAlign::Center),
                TextAlign::Left,
            );

            // Close（selected 或 hover 时显示）
            if let Some(c) = self.close_rect(rect, k)
                && (selected || hovered)
            {
                if close_hovered {
                    // 关闭键悬停 = 极轻底（WinUI 2.x `TabViewItemHeaderCloseButtonBackgroundPointerOver`
                    // → `SubtleFillColorSecondaryBrush` = 暗 5.9% / 亮 3.5%，
                    // TabView_themeresources.xaml L52 → Common_themeresources_any.xaml）。
                    // 旧值 15% 出自 §864 的同名笔刷但百分比抄错（按下真值更淡，为 Tertiary 3.9%）。
                    scene.fill_rounded_rect(
                        theme.indication.subtle_tint,
                        c,
                        theme.tokens.corner_radius,
                    );
                }
                draw_close_x(scene, c, colors.on_surface_variant);
            }
        }

        // Add（＋）
        if let Some(a) = self.add_rect(rect) {
            if self.hovered == Some(TabHover::Add) {
                scene.fill_rounded_rect(
                    theme.indication.hover_tint,
                    a,
                    theme.tokens.corner_radius,
                );
            }
            // ＋ 自绘（两横一竖，三条细矩形）
            let c = a.center();
            let t = 1.5;
            let r = 6.0;
            scene.fill_rect(
                colors.on_surface_variant,
                Rect::new(c.x - r, c.y - t / 2.0, r * 2.0, t),
            );
            scene.fill_rect(
                colors.on_surface_variant,
                Rect::new(c.x - t / 2.0, c.y - r, t, r * 2.0),
            );
        }

        scene.pop_clip();
    }
}

/// 自绘 ×（InfoBar 同款：对角四三角形）。
fn draw_close_x(scene: &mut Scene, rect: Rect, color: Color) {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let r = 6.0;
    let t = 1.6;
    let cross = color;
    scene.triangle(
        Point::new(cx - r, cy - r + t),
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r - t, cy + r),
        cross,
    );
    scene.triangle(
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r, cy + r - t),
        Point::new(cx + r - t, cy + r),
        cross,
    );
    scene.triangle(
        Point::new(cx + r - t, cy - r),
        Point::new(cx + r, cy - r + t),
        Point::new(cx - r + t, cy + r),
        cross,
    );
    scene.triangle(
        Point::new(cx + r, cy - r + t),
        Point::new(cx + r - t, cy + r),
        Point::new(cx - r, cy + r - t),
        cross,
    );
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// 自绘虚拟化标签页：页签条由控件按数据自画（旧 `render`），**不**把每页签变成子节点。
// 点击页签发 `TabSelected`；点击关闭键发 `TabCloseRequested`（**不**自己删页签，由 App 决定）；
// 点击加号发 `TabAddRequested`。页签超宽时横向滚动：`Event::Scroll` 的 dy 映射为水平偏移，
// 偏移未变（已到端 / 不溢出）时不截停以冒泡。Ctrl+Tab / Ctrl+Shift+Tab 切页并截停；
// 普通 Tab 不处理，留给框架做焦点遍历（参 §3）。

/// 元素树动作：页签被选中（点击 / Ctrl+Tab）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabSelected(pub usize);

/// 元素树动作：请求关闭页签（App 决定是否真的移除）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabCloseRequested(pub usize);

/// 元素树动作：请求新增页签。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabAddRequested;

impl kanesumi_element::Widget for MetroTabView {
    /// 宽取可用宽（页签条铺满视口，溢出靠横向滚动）；高 = 顶部留白 + 页签最小高。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let width = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            self.total_width(f32::INFINITY)
        };
        kanesumi_core::Size::new(width, TABVIEW_HEADER_PAD + TABVIEW_ITEM_MIN_H)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.hover(rect, *pos);
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
            } => match self.hit(rect, *pos) {
                TabViewAction::Select(k) => {
                    if k != self.selected_index {
                        self.selected_index = k;
                        ctx.emit(TabSelected(k));
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                }
                TabViewAction::Close(k) => {
                    // 不自己删页签：App 收到请求后再改 tabs。
                    ctx.emit(TabCloseRequested(k));
                    ctx.set_handled();
                }
                TabViewAction::Add => {
                    ctx.emit(TabAddRequested);
                    ctx.set_handled();
                }
                TabViewAction::None => {}
            },
            Event::Scroll { dy, .. } => {
                // 页签超宽时横向滚动：滚轮 dy 映射为水平偏移。
                let before = self.scroll_offset;
                self.scroll_by(*dy, rect.size.width);
                if self.scroll_offset != before {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, modifiers } => {
                // Ctrl+Tab / Ctrl+Shift+Tab 切页；普通 Tab 直接放行给框架焦点遍历。
                if *key != Key::Tab || !modifiers.ctrl || self.tabs.is_empty() {
                    return;
                }
                self.clamp_selection();
                let len = self.tabs.len();
                let next = if modifiers.shift {
                    (self.selected_index + len - 1) % len
                } else {
                    (self.selected_index + 1) % len
                };
                if next != self.selected_index {
                    self.selected_index = next;
                    ctx.emit(TabSelected(next));
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
            name: String::from("标签页"),
            value: self.tabs.get(self.selected_index).cloned(),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, Modifiers, WidgetId};

    fn harness(width: f32, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroTabView::new(
                ["首页", "文档", "设置"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            LayoutProps {
                width: Some(width),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_selects_tab_and_reports() {
        let (mut h, id) = harness(300.0, 40.0);
        let r = h.rect(id);
        let tab2 = h.tree.get::<MetroTabView>(id).unwrap().tab_rect(r, 2);
        h.click_at(tab2.center());
        assert_eq!(h.take::<TabSelected>(), vec![(id, TabSelected(2))]);
        assert_eq!(
            h.tree.get::<MetroTabView>(id).unwrap().selected_index,
            2
        );
    }

    #[test]
    fn click_close_requests_without_removing() {
        let (mut h, id) = harness(300.0, 40.0);
        let r = h.rect(id);
        let close = h.tree.get::<MetroTabView>(id).unwrap().close_rect(r, 0).unwrap();
        h.click_at(close.center());
        assert_eq!(
            h.take::<TabCloseRequested>(),
            vec![(id, TabCloseRequested(0))]
        );
        assert_eq!(
            h.tree.get::<MetroTabView>(id).unwrap().tabs.len(),
            3,
            "关闭键只发请求，不由控件删页签"
        );
    }

    #[test]
    fn click_add_requests_add() {
        let (mut h, id) = harness(300.0, 40.0);
        let r = h.rect(id);
        // 3 页签 + add 超出视口 → 先滚到末端让 add 可见。
        h.tree
            .scroll(r.center(), 0.0, 999.0, Modifiers::NONE);
        h.frame();
        let add = h.tree.get::<MetroTabView>(id).unwrap().add_rect(r).unwrap();
        assert!(r.contains(add.center()), "滚到末端后 add 应在视口内");
        h.click_at(add.center());
        assert_eq!(h.take::<TabAddRequested>(), vec![(id, TabAddRequested)]);
    }

    #[test]
    fn ctrl_tab_cycles_selection() {
        let (mut h, id) = harness(300.0, 40.0);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        let ctrl = Modifiers {
            ctrl: true,
            ..Modifiers::NONE
        };
        assert!(h.key_with(Key::Tab, ctrl), "Ctrl+Tab 应由控件截停");
        assert_eq!(h.tree.get::<MetroTabView>(id).unwrap().selected_index, 1);
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        assert!(h.key_with(Key::Tab, ctrl_shift));
        assert_eq!(h.tree.get::<MetroTabView>(id).unwrap().selected_index, 0);
        assert_eq!(h.take::<TabSelected>().len(), 2);
    }

    #[test]
    fn plain_tab_is_left_to_framework() {
        // 普通 Tab 不处理 → 框架焦点遍历到下一个可聚焦控件。
        let mut h = TestHarness::new(400.0, 300.0);
        let tv = h.tree.insert_with(
            h.root(),
            MetroTabView::new(vec!["仅一页".into()]),
            LayoutProps {
                width: Some(300.0),
                height: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let btn = h.tree.insert_with(
            h.root(),
            crate::button::MetroButton::new("下一个"),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tab();
        assert_eq!(h.tree.focused(), Some(tv));
        h.key(Key::Tab);
        assert_eq!(h.tree.focused(), Some(btn), "普通 Tab 应交给框架遍历");
    }

    #[test]
    fn wheel_scrolls_horizontally() {
        let (mut h, id) = harness(150.0, 40.0);
        let r = h.rect(id);
        assert!(h.tree.get::<MetroTabView>(id).unwrap().max_scroll(r.size.width) > 0.0);
        h.tree.scroll(r.center(), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(
            h.tree.get::<MetroTabView>(id).unwrap().scroll_offset,
            50.0,
            "滚轮 dy 映射为水平偏移"
        );
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(300.0, 40.0);
        assert_eq!(h.rect(id).size.width, 300.0);
        assert_eq!(h.rect(id).size.height, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        let (h, id) = harness(40.0, 40.0);
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(300.0, 40.0);
        h.tree.set_enabled(id, false);
        h.frame();
        let r = h.rect(id);
        let tab2 = h.tree.get::<MetroTabView>(id).unwrap().tab_rect(r, 2);
        h.click_at(tab2.center());
        assert!(h.take::<TabSelected>().is_empty());
        assert_eq!(h.tree.get::<MetroTabView>(id).unwrap().selected_index, 0);
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

    fn tabs() -> MetroTabView {
        MetroTabView::new(
            ["首页", "文档", "设置"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 600.0, 40.0)
    }

    #[test]
    fn tab_width_clamped() {
        let t = tabs();
        // 600/3 = 200 → 在 100..240
        assert_eq!(t.tab_width(600.0), 200.0);
        // 很窄 → 100
        assert_eq!(t.tab_width(200.0), 100.0);
        // 很宽 → 240
        assert_eq!(t.tab_width(2000.0), 240.0);
    }

    #[test]
    fn tab_geometry() {
        let t = tabs();
        let a = area();
        let tr1 = t.tab_rect(a, 0);
        let tr2 = t.tab_rect(a, 1);
        assert_eq!(tr1.origin.y, 8.0, "Header Padding 0,8,0,0");
        assert_eq!(tr1.size.height, 32.0);
        assert_eq!(tr2.origin.x - tr1.origin.x, 200.0);
    }

    #[test]
    fn close_rect_on_hovered() {
        let t = tabs();
        let a = area();
        let c = t.close_rect(a, 0).unwrap();
        assert_eq!(c.size.width, 32.0);
        assert_eq!(c.size.height, 24.0);
    }

    #[test]
    fn click_selects_tab() {
        let mut t = tabs();
        let a = area();
        let tr2 = t.tab_rect(a, 2);
        assert_eq!(t.handle_click(a, tr2.center()), TabViewAction::Select(2));
        assert_eq!(t.selected_index, 2);
    }

    #[test]
    fn click_close_removes() {
        let mut t = tabs();
        let a = area();
        let c = t.close_rect(a, 0).unwrap();
        assert_eq!(t.handle_click(a, c.center()), TabViewAction::Close(0));
        assert_eq!(t.tabs.len(), 2);
    }

    #[test]
    fn click_add_hit() {
        let mut t = tabs();
        let a = area();
        // 3 个 tab 各 200 + add 32 > 600：offset=0 时 add 按钮落在视口右侧之外，
        // 容器边界（P0-2）下不可命中 —— 这正是旧实现「能从控件外点中不可见目标」的回归点。
        let add0 = t.add_rect(a).unwrap();
        assert_eq!(t.hit(a, add0.center()), TabViewAction::None);
        // 滚到末端后 add 进入视口 → 可命中。
        t.scroll_by(999.0, a.size.width);
        let add = t.add_rect(a).unwrap();
        assert!(a.contains(add.center()), "滚动到末端后 add 应落在视口内");
        assert_eq!(t.hit(a, add.center()), TabViewAction::Add);
    }

    #[test]
    fn scroll_clamps() {
        let mut t = tabs();
        let a = Rect::new(0.0, 0.0, 200.0, 40.0);
        let max = t.max_scroll(a.size.width);
        assert!(max > 0.0, "窄宽溢出可滚动");
        t.scroll_by(999.0, a.size.width);
        assert_eq!(t.scroll_offset, max);
        t.scroll_by(-999.0, a.size.width);
        assert_eq!(t.scroll_offset, 0.0);
    }

    #[test]
    fn render_emits_tabs_and_add() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let t = tabs();
        let mut scene = Scene::default();
        t.render(&theme, &engine, area(), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 3, "3 个 tab 标题");
        // Add ＋ = 3 条 fill（横 + 竖 + 可能选中底）
        assert!(!scene.is_empty());
    }

    #[test]
    fn selected_tab_has_surface_bg() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let t = tabs();
        let mut scene = Scene::default();
        t.render(&theme, &engine, area(), &mut scene);
        // 选中 tab 底 = surface_variant
        let has_selected_bg = scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.surface_variant));
        assert!(has_selected_bg, "选中 tab 应有 surface_variant 底");
    }
}
