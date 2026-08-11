// MetroTabView —— Chrome 式标签页。参 CONTROL_SPEC §25。
//
// 移植自 microsoft-ui-xaml/dev/TabView（TabView.cpp + TabView_themeresources.xaml）：
// - Item MinHeight 32、MinWidth 100、MaxWidth 240；等宽分配（clamp(avail/len, 100, 240)）；
// - Header Padding 0,8,0,0；Close 32×24（hovered/selected 显示）、Add 32×24；
// - 选中底 surface_variant / 前景 on_surface；未选 on_surface_variant。
// 拖拽 Reorder 暂略（Phase 3 续）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, FontWeight, MetroTheme, Point, Rect};

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
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = TextStyle::new(TABVIEW_ITEM_FONT, 16.0, FontWeight::Normal);

        for k in 0..self.tabs.len() {
            let tr = self.tab_rect(rect, k);
            let selected = k == self.selected_index;
            let hovered = self.hovered == Some(TabHover::Tab(k));
            let close_hovered = self.hovered == Some(TabHover::Close(k));

            // 底
            let bg = if selected {
                colors.surface_variant
            } else if hovered {
                colors.on_surface.with_alpha(0.08)
            } else {
                Color::TRANSPARENT
            };
            if bg.a > 0.0 {
                scene.fill_rect(bg, tr);
            }

            // 标题（左 padding 8/9）
            let pad = if selected { 9.0 } else { 8.0 };
            let label_w = engine.measure(&self.tabs[k], style.size);
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
            scene.text(
                self.tabs[k].clone(),
                Rect::new(
                    tr.origin.x + pad,
                    tr.origin.y + (tr.size.height - style.line_height) / 2.0,
                    (label_w.min(title_w)).max(0.0),
                    style.line_height,
                ),
                fg,
                style,
                TextAlign::Left,
            );

            // Close（selected 或 hover 时显示）
            if let Some(c) = self.close_rect(rect, k)
                && (selected || hovered)
            {
                if close_hovered {
                    scene.fill_rounded_rect(
                        colors.on_surface.with_alpha(0.15),
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
                    colors.on_surface.with_alpha(0.10),
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
        let t = tabs();
        let a = area();
        let add = t.add_rect(a).unwrap();
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
