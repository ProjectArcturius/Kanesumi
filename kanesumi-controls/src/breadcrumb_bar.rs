// MetroBreadcrumbBar —— 面包屑导航。参 CONTROL_SPEC §18。
//
// 移植自 microsoft-ui-xaml/dev/Breadcrumb（BreadcrumbBar.cpp + BreadcrumbBar.xaml）：
// - Item 14px Normal / LineHeight 20 / Padding 1,3；
// - 项间 chevron（E974 → 自绘）12px、Padding 2,0（占 ~16px）；
// - 当前项（末项）非按钮、无尾部 chevron、前景 on_surface；
// - 超宽折叠：前缀换 "…"（至少保留末项），点 "…" 弹出隐藏项下拉（MetroDropdownMenu）。
// 折叠与下拉复用 `MetroDropdownMenu`（隐藏项即其 items）。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size};

use crate::dropdown_menu::{MenuItem, MetroDropdownMenu};
use crate::popup::{place_popup, popup_gap};

/// Item 字号（BreadcrumbBarItemThemeFontSize = ControlContentThemeFontSize 14）。
const ITEM_FONT: f32 = 14.0;
/// Item 水平 Padding（`1,3` → 左右各 1）。
const ITEM_PAD_X: f32 = 1.0;
/// Item 垂直 Padding（上下各 3）。
const ITEM_PAD_Y: f32 = 3.0;
/// Chevron 总占宽（12 glyph + 2×2 padding）。
const CHEVRON_GAP: f32 = 16.0;
/// Ellipsis 水平内边距（Padding 3 → 左右各 3）。
const ELLIPSIS_PAD: f32 = 6.0;

/// 面包屑点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreadcrumbClick {
    None,
    /// 命中面包屑项（索引；含下拉隐藏项）。
    Index(usize),
    /// 命中 Ellipsis（打开下拉）。
    Ellipsis,
}

/// 布局结果：可见起点 + 是否折叠。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreadcrumbLayout {
    /// 首个可见项在 `items` 的索引。
    pub start: usize,
    /// 是否渲染 Ellipsis（start > 0）。
    pub ellipsis: bool,
}

/// MetroBreadcrumbBar —— 面包屑。参 CONTROL_SPEC §18。
#[derive(Debug, Clone)]
pub struct MetroBreadcrumbBar {
    pub items: Vec<String>,
    /// hover 的可见面包屑项（绝对索引）。
    pub hovered_item: Option<usize>,
    /// 是否 hover 在 Ellipsis 上。
    pub hovered_ellipsis: bool,
    /// Ellipsis 下拉（隐藏项）。
    pub menu: MetroDropdownMenu,
}

impl Default for MetroBreadcrumbBar {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            hovered_item: None,
            hovered_ellipsis: false,
            menu: MetroDropdownMenu::new(Vec::new()),
        }
    }
}

impl MetroBreadcrumbBar {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// Item 文本样式。
    pub fn item_style() -> TextStyle {
        TextStyle::new(ITEM_FONT, 20.0, FontWeight::Normal)
    }

    /// 计算折叠布局。参 CONTROL_SPEC §18 折叠语义。
    pub fn layout(&self, engine: &TextEngine, rect: Rect) -> BreadcrumbLayout {
        let avail = rect.size.width;
        let style = Self::item_style();
        let widths: Vec<f32> = self
            .items
            .iter()
            .map(|i| engine.measure(i, style.size) + ITEM_PAD_X * 2.0)
            .collect();
        let n = self.items.len();
        if n == 0 {
            return BreadcrumbLayout {
                start: 0,
                ellipsis: false,
            };
        }
        // 全展示所需宽度
        let total: f32 = widths.iter().sum::<f32>() + (n as f32 - 1.0) * CHEVRON_GAP;
        if total <= avail {
            return BreadcrumbLayout {
                start: 0,
                ellipsis: false,
            };
        }
        // 折叠：从末项往前累积，至少保留末项；前缀让位 Ellipsis。
        let ellipsis_w = engine.measure("…", style.size) + ELLIPSIS_PAD;
        let budget = (avail - ellipsis_w).max(0.0);
        let mut used = 0.0f32;
        let mut start = n; // 尚未保留任何项
        for i in (0..n).rev() {
            let w = widths[i];
            let with_gap = if start < n { w + CHEVRON_GAP } else { w };
            if start < n && used + with_gap > budget {
                break;
            }
            used += with_gap;
            start = i;
        }
        if start == n {
            start = n - 1; // 至少保留末项
        }
        BreadcrumbLayout {
            start,
            ellipsis: start > 0,
        }
    }

    /// 单个面包屑项 rect（含 padding）。
    fn item_rect(&self, engine: &TextEngine, rect: Rect, index: usize) -> Rect {
        let style = Self::item_style();
        let x = self.item_x(engine, rect, index);
        Rect::new(
            x,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
            engine.measure(&self.items[index], style.size) + ITEM_PAD_X * 2.0,
            style.line_height + ITEM_PAD_Y * 2.0,
        )
    }

    /// 面包屑项 x 起点（考虑 Ellipsis 与 chevron）。
    fn item_x(&self, engine: &TextEngine, rect: Rect, index: usize) -> f32 {
        let layout = self.layout(engine, rect);
        let style = Self::item_style();
        let mut x = rect.origin.x;
        if layout.ellipsis {
            x += engine.measure("…", style.size) + ELLIPSIS_PAD + CHEVRON_GAP;
        }
        for i in layout.start..index {
            x += engine.measure(&self.items[i], style.size) + ITEM_PAD_X * 2.0;
            if i < self.items.len() - 1 {
                x += CHEVRON_GAP;
            }
        }
        x
    }

    /// Ellipsis rect（折叠时）。
    pub fn ellipsis_rect(&self, engine: &TextEngine, rect: Rect) -> Option<Rect> {
        let layout = self.layout(engine, rect);
        if !layout.ellipsis {
            return None;
        }
        let style = Self::item_style();
        let w = engine.measure("…", style.size) + ELLIPSIS_PAD;
        Some(Rect::new(
            rect.origin.x,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
            w,
            style.line_height + ITEM_PAD_Y * 2.0,
        ))
    }

    /// 命中面包屑项（含 Ellipsis）。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> BreadcrumbClick {
        if let Some(e) = self.ellipsis_rect(engine, rect)
            && e.contains(pos)
        {
            return BreadcrumbClick::Ellipsis;
        }
        let layout = self.layout(engine, rect);
        for i in layout.start..self.items.len() {
            let r = self.item_rect(engine, rect, i);
            if r.contains(pos) {
                return BreadcrumbClick::Index(i);
            }
        }
        BreadcrumbClick::None
    }

    /// 打开/关闭 Ellipsis 下拉（隐藏项）。
    pub fn toggle_ellipsis(&mut self, engine: &TextEngine, rect: Rect, screen: Rect) {
        let layout = self.layout(engine, rect);
        let hidden = self.items[0..layout.start].to_vec();
        self.menu.items = hidden.into_iter().map(MenuItem::new).collect();
        self.menu.invalidate_layout();
        if self.menu.anim.is_open() {
            self.menu.close();
        } else if let Some(er) = self.ellipsis_rect(engine, rect) {
            let size = self.menu.panel_size(engine);
            let placement = place_popup(er, size, screen, popup_gap());
            self.menu.open(placement.rect);
        }
    }

    /// 综合命中处理：Ellipsis toggle / 下拉项 / 面包屑项。
    pub fn handle_click(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        pos: Point,
    ) -> BreadcrumbClick {
        // 下拉已开：项 → 返回；外 → 关闭。
        if self.menu.anim.is_open() {
            if let Some(i) = self.menu.item_at(pos) {
                self.menu.close();
                return BreadcrumbClick::Index(i);
            }
            if !rect.contains(pos) {
                self.menu.close();
                return BreadcrumbClick::None;
            }
        }
        match self.hit(engine, rect, pos) {
            BreadcrumbClick::Ellipsis => {
                self.toggle_ellipsis(engine, rect, screen);
                BreadcrumbClick::Ellipsis
            }
            other => other,
        }
    }

    /// 悬停路由：Ellipsis / 项 / 下拉项。
    pub fn hover(&mut self, engine: &TextEngine, rect: Rect, pos: Point) {
        self.hovered_ellipsis = self
            .ellipsis_rect(engine, rect)
            .map(|r| r.contains(pos))
            .unwrap_or(false);
        self.hovered_item = match self.hit(engine, rect, pos) {
            BreadcrumbClick::Index(i) => Some(i),
            _ => None,
        };
        if self.menu.anim.is_open() {
            self.menu.hovered = self.menu.item_at(pos);
        }
    }

    /// 每帧推进下拉动画。
    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
    }

    /// 固有尺寸（不折叠时的完整宽度）。
    pub fn measure(&self, engine: &TextEngine) -> Size {
        let style = Self::item_style();
        let widths: f32 = self
            .items
            .iter()
            .map(|i| engine.measure(i, style.size) + ITEM_PAD_X * 2.0)
            .sum();
        let n = self.items.len().saturating_sub(1) as f32;
        Size::new(
            widths + n * CHEVRON_GAP,
            style.line_height + ITEM_PAD_Y * 2.0,
        )
    }

    /// 渲染面包屑 +（折叠时）Ellipsis 下拉。
    pub fn render(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let style = Self::item_style();
        let layout = self.layout(engine, rect);

        let mut x = rect.origin.x;
        // Ellipsis
        if layout.ellipsis {
            let w = engine.measure("…", style.size) + ELLIPSIS_PAD;
            let er = Rect::new(
                x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
                w,
                style.line_height + ITEM_PAD_Y * 2.0,
            );
            if self.hovered_ellipsis {
                scene.fill_rounded_rect(
                    colors.on_surface.with_alpha(0.10),
                    er,
                    theme.tokens.corner_radius,
                );
            }
            scene.text(
                "…".into(),
                Rect::new(
                    x + ELLIPSIS_PAD / 2.0,
                    rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                    w - ELLIPSIS_PAD,
                    style.line_height,
                ),
                colors.on_surface,
                style,
                TextAlign::Center,
            );
            x += w + CHEVRON_GAP;
        }

        // 项
        for (i, label) in self.items.iter().enumerate().skip(layout.start) {
            let is_last = i == self.items.len() - 1;
            let w = engine.measure(label, style.size) + ITEM_PAD_X * 2.0;
            let hovered = self.hovered_item == Some(i);
            let bg = if hovered && !is_last {
                colors.on_surface.with_alpha(0.10)
            } else {
                kanesumi_core::Color::TRANSPARENT
            };
            let item_rect = Rect::new(
                x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
                w,
                style.line_height + ITEM_PAD_Y * 2.0,
            );
            if bg.a > 0.0 {
                scene.fill_rounded_rect(bg, item_rect, theme.tokens.corner_radius);
            }
            let fg = if is_last || hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.text(
                label.clone(),
                Rect::new(
                    x + ITEM_PAD_X,
                    rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                    w - ITEM_PAD_X * 2.0,
                    style.line_height,
                ),
                fg,
                style,
                TextAlign::Left,
            );
            x += w;
            // Chevron（非末项）
            if !is_last {
                let chevron_rect = Rect::new(
                    x + 2.0,
                    rect.origin.y + (rect.size.height - 12.0) / 2.0,
                    12.0,
                    12.0,
                );
                glyph::chevron_right(scene, chevron_rect, colors.on_surface_variant);
                x += CHEVRON_GAP;
            }
        }

        // Ellipsis 下拉
        if self.menu.anim.is_visible() {
            self.menu.render(theme, engine, screen, scene);
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

    fn bar() -> MetroBreadcrumbBar {
        MetroBreadcrumbBar::new(
            ["首页", "文档", "项目", "Ether"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    #[test]
    fn wide_fits_no_collapse() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 800.0, 32.0);
        let l = b.layout(&engine, r);
        assert_eq!(l.start, 0);
        assert!(!l.ellipsis);
    }

    #[test]
    fn narrow_collapses_prefix() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let l = b.layout(&engine, r);
        assert!(l.ellipsis, "窄宽应折叠");
        assert!(l.start > 0);
        // 至少保留末项
        assert!(l.start <= 3);
    }

    #[test]
    fn last_item_always_kept() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 60.0, 32.0);
        let l = b.layout(&engine, r);
        assert!(l.ellipsis);
        assert_eq!(l.start, 3, "极窄也保留末项");
    }

    #[test]
    fn hit_maps_items_and_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 800.0, 32.0);
        // 第一项中心
        let first = b.item_rect(&engine, r, 0);
        assert_eq!(
            b.hit(&engine, r, Point::new(first.center().x, first.center().y)),
            BreadcrumbClick::Index(0)
        );
        // 无折叠 → 无 ellipsis
        assert_eq!(b.ellipsis_rect(&engine, r), None);
    }

    #[test]
    fn hit_ellipsis_when_collapsed() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let er = b.ellipsis_rect(&engine, r);
        assert!(er.is_some());
        assert_eq!(
            b.hit(
                &engine,
                r,
                Point::new(er.unwrap().center().x, er.unwrap().center().y)
            ),
            BreadcrumbClick::Ellipsis
        );
    }

    #[test]
    fn toggle_ellipsis_opens_menu() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let screen = Rect::new(0.0, 0.0, 400.0, 400.0);
        b.toggle_ellipsis(&engine, r, screen);
        assert!(!b.menu.items.is_empty(), "隐藏项应进入下拉");
        assert!(b.menu.anim.is_visible(), "下拉应打开（含 Opening）");
    }

    #[test]
    fn handle_click_ellipsis_then_hidden_item() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let screen = Rect::new(0.0, 0.0, 400.0, 400.0);
        // 点 ellipsis
        let er = b.ellipsis_rect(&engine, r).unwrap();
        let c = b.handle_click(&engine, r, screen, Point::new(er.center().x, er.center().y));
        assert_eq!(c, BreadcrumbClick::Ellipsis);
        // 点下拉第一项
        b.menu.update(1.0);
        let panel = b.menu.panel_rect;
        let item = Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0);
        let c = b.handle_click(&engine, r, screen, item);
        assert!(matches!(c, BreadcrumbClick::Index(0)));
        b.menu.update(1.0);
        assert!(!b.menu.anim.is_open());
    }

    #[test]
    fn render_emits_items_and_chevrons() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 32.0),
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 4, "4 个面包屑项");
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 3, "3 个 chevron");
    }

    #[test]
    fn render_collapsed_has_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            Rect::new(0.0, 0.0, 400.0, 400.0),
            &mut scene,
        );
        let texts: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "…"), "折叠应渲染 Ellipsis");
    }
}
