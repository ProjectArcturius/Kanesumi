// MetroPagerControl —— 数字分页器（NumberPanel 模式）。参 CONTROL_SPEC §20。
//
// 移植自 microsoft-ui-xaml/dev/PagerControl（PagerControl.cpp + PagerControl.xaml）：
// - Nav 40×40：First(◀◀)/Prev(◀)/Next(▶)/Last(▶▶)，首/末边缘对应按钮隐藏（保留空间）；
// - 数字按钮 MinWidth 32 / MinHeight 20 / 间距 5；选中 = 强调色前景 + 下方 2px 强调条；
// - 数字窗口：n≤7 全展示；前四页 `1..5 … n`；后四页 `1 … n−4..n`；中间 `1 … s−1 s s+1 … n`。
// 仅 NumberPanel 模式（NumberBox/ComboBox 模式依赖闭源控件）。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size};

/// Nav 按钮边长（40）。
const NAV_SIZE: f32 = 40.0;
/// 数字按钮最小宽（PagerControlNumberPanelButtonWidth = 32）。
const PAGE_BUTTON_MIN_W: f32 = 32.0;
/// 数字按钮最小高（20）。
const PAGE_BUTTON_MIN_H: f32 = 20.0;
/// 数字按钮间距（StackLayout Spacing = 5）。
const PAGE_SPACING: f32 = 5.0;
/// 选中指示条高（2px）。
const INDICATOR_H: f32 = 2.0;

/// 数字面板项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerItem {
    Number(usize),
    Ellipsis,
}

/// 分页动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerAction {
    None,
    First,
    Prev,
    Next,
    Last,
    /// 选中页码（0 基）。
    Select(usize),
}

/// MetroPagerControl —— 数字分页器。参 CONTROL_SPEC §20。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroPagerControl {
    pub number_of_pages: usize,
    /// 选中页（0 基）。
    pub selected_index: usize,
    /// 窗口模式是否展示首/末页（默认 true）。
    pub always_show_first_last: bool,
    /// hover 的部件（Nav / 页码）。
    pub hovered: Option<PagerHover>,
}

/// hover 目标。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PagerHover {
    First,
    Prev,
    Next,
    Last,
    Page(usize),
}

impl Default for MetroPagerControl {
    fn default() -> Self {
        Self {
            number_of_pages: 0,
            selected_index: 0,
            always_show_first_last: true,
            hovered: None,
        }
    }
}

impl MetroPagerControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// 夹紧选中索引。
    pub fn clamp_selection(&mut self) {
        if self.number_of_pages == 0 {
            self.selected_index = 0;
        } else {
            self.selected_index = self.selected_index.min(self.number_of_pages - 1);
        }
    }

    /// 数字面板项序列。参 CONTROL_SPEC §20 窗口逻辑。
    pub fn panel_items(&self) -> Vec<PagerItem> {
        let n = self.number_of_pages;
        if n == 0 {
            return Vec::new();
        }
        let s = self.selected_index + 1; // 1 基选中页
        if n <= 7 {
            return (1..=n).map(PagerItem::Number).collect();
        }
        let show = self.always_show_first_last;
        let mut items = Vec::new();
        if s <= 4 {
            for i in 1..=5 {
                items.push(PagerItem::Number(i));
            }
            if show {
                items.push(PagerItem::Ellipsis);
                items.push(PagerItem::Number(n));
            }
        } else if s >= n - 3 {
            if show {
                items.push(PagerItem::Number(1));
                items.push(PagerItem::Ellipsis);
            }
            for i in (n - 4)..=n {
                items.push(PagerItem::Number(i));
            }
        } else {
            if show {
                items.push(PagerItem::Number(1));
                items.push(PagerItem::Ellipsis);
            }
            for i in (s - 1)..=(s + 1) {
                items.push(PagerItem::Number(i));
            }
            if show {
                items.push(PagerItem::Ellipsis);
                items.push(PagerItem::Number(n));
            }
        }
        items
    }

    /// 选中项在 `panel_items` 中的下标（用于定位指示条）。
    pub fn selected_item_index(&self) -> Option<usize> {
        self.panel_items()
            .iter()
            .position(|i| matches!(i, PagerItem::Number(p) if *p == self.selected_index + 1))
    }

    fn page_number_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Normal)
    }

    /// 单个数字按钮宽（Max(32, 数字文本宽)）。
    fn page_button_width(engine: &TextEngine, page: usize) -> f32 {
        let style = Self::page_number_style();
        engine
            .measure(&page.to_string(), style.size)
            .max(PAGE_BUTTON_MIN_W)
    }

    /// 整个分页器固有尺寸。
    pub fn measure(&self, engine: &TextEngine) -> Size {
        let items = self.panel_items();
        let pages_w: f32 = items
            .iter()
            .map(|i| match i {
                PagerItem::Number(p) => Self::page_button_width(engine, *p),
                PagerItem::Ellipsis => PAGE_BUTTON_MIN_W,
            })
            .sum();
        let spacing = (items.len().saturating_sub(1)) as f32 * PAGE_SPACING;
        // 首/末边缘隐藏 Nav 时仍保留空间（opacity 0，不参与跳变）
        let nav_pairs = 2.0; // First+Prev 与 Next+Last 两组，各 40×2
        let nav_w = nav_pairs * NAV_SIZE * 2.0;
        Size::new(nav_w + pages_w + spacing, NAV_SIZE)
    }

    /// Nav 与页码段的几何。
    fn geom(&self, engine: &TextEngine, rect: Rect) -> (Rect, Rect, Vec<(PagerItem, Rect)>) {
        let total = self.measure(engine);
        let x0 = rect.origin.x + (rect.size.width - total.width) / 2.0;
        let nav_left = Rect::new(x0, rect.origin.y, NAV_SIZE * 2.0, NAV_SIZE);
        let pages_x = nav_left.right();
        let pages_w = total.width - NAV_SIZE * 4.0;
        let pages_rect = Rect::new(pages_x, rect.origin.y, pages_w, NAV_SIZE);
        // 页码 rects
        let items = self.panel_items();
        let mut rects = Vec::new();
        let mut x = pages_rect.origin.x;
        let y = pages_rect.origin.y + (pages_rect.size.height - PAGE_BUTTON_MIN_H) / 2.0;
        for item in &items {
            let w = match item {
                PagerItem::Number(p) => Self::page_button_width(engine, *p),
                PagerItem::Ellipsis => PAGE_BUTTON_MIN_W,
            };
            rects.push((*item, Rect::new(x, y, w, PAGE_BUTTON_MIN_H)));
            x += w + PAGE_SPACING;
        }
        let nav_right = Rect::new(pages_rect.right(), rect.origin.y, NAV_SIZE * 2.0, NAV_SIZE);
        (nav_left, nav_right, rects)
    }

    fn nav_part_at(nav_left: Rect, nav_right: Rect, pos: Point) -> Option<PagerHover> {
        let first = Rect::new(nav_left.origin.x, nav_left.origin.y, NAV_SIZE, NAV_SIZE);
        let prev = Rect::new(
            nav_left.origin.x + NAV_SIZE,
            nav_left.origin.y,
            NAV_SIZE,
            NAV_SIZE,
        );
        let next = Rect::new(nav_right.origin.x, nav_right.origin.y, NAV_SIZE, NAV_SIZE);
        let last = Rect::new(
            nav_right.origin.x + NAV_SIZE,
            nav_right.origin.y,
            NAV_SIZE,
            NAV_SIZE,
        );
        if first.contains(pos) {
            Some(PagerHover::First)
        } else if prev.contains(pos) {
            Some(PagerHover::Prev)
        } else if next.contains(pos) {
            Some(PagerHover::Next)
        } else if last.contains(pos) {
            Some(PagerHover::Last)
        } else {
            None
        }
    }

    /// 命中：Nav / 页码 / None。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> PagerAction {
        let (nl, nr, rects) = self.geom(engine, rect);
        if let Some(h) = Self::nav_part_at(nl, nr, pos) {
            let at_first = self.selected_index == 0;
            let at_last =
                self.number_of_pages == 0 || self.selected_index == self.number_of_pages - 1;
            return match h {
                PagerHover::First if !at_first => PagerAction::First,
                PagerHover::Prev if !at_first => PagerAction::Prev,
                PagerHover::Next if !at_last => PagerAction::Next,
                PagerHover::Last if !at_last => PagerAction::Last,
                _ => PagerAction::None,
            };
        }
        for (item, r) in rects {
            if r.contains(pos)
                && let PagerItem::Number(p) = item
            {
                return PagerAction::Select(p - 1);
            }
        }
        PagerAction::None
    }

    /// 应用动作（更新选中）。返回动作。
    pub fn handle_click(&mut self, engine: &TextEngine, rect: Rect, pos: Point) -> PagerAction {
        match self.hit(engine, rect, pos) {
            PagerAction::Select(p) => {
                self.selected_index = p;
                self.clamp_selection();
                PagerAction::Select(self.selected_index)
            }
            PagerAction::First => {
                self.selected_index = 0;
                PagerAction::First
            }
            PagerAction::Prev => {
                let changed = self.selected_index > 0;
                if changed {
                    self.selected_index -= 1;
                }
                if changed {
                    PagerAction::Prev
                } else {
                    PagerAction::None
                }
            }
            PagerAction::Next => {
                let changed = self.selected_index + 1 < self.number_of_pages;
                if changed {
                    self.selected_index += 1;
                }
                if changed {
                    PagerAction::Next
                } else {
                    PagerAction::None
                }
            }
            PagerAction::Last => {
                let changed = self.number_of_pages > 0;
                if changed {
                    self.selected_index = self.number_of_pages - 1;
                }
                if changed {
                    PagerAction::Last
                } else {
                    PagerAction::None
                }
            }
            PagerAction::None => PagerAction::None,
        }
    }

    /// 悬停路由。
    pub fn hover(&mut self, engine: &TextEngine, rect: Rect, pos: Point) {
        let (nl, nr, rects) = self.geom(engine, rect);
        self.hovered = Self::nav_part_at(nl, nr, pos);
        if self.hovered.is_none() {
            for (item, r) in rects {
                if r.contains(pos) {
                    if let PagerItem::Number(p) = item {
                        self.hovered = Some(PagerHover::Page(p - 1));
                    }
                    break;
                }
            }
        }
    }

    /// 渲染分页器。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let (nl, nr, rects) = self.geom(engine, rect);
        let at_first = self.selected_index == 0;
        let at_last = self.number_of_pages == 0 || self.selected_index == self.number_of_pages - 1;

        // Nav 左组（首/前）
        let nav_hovered = |h: PagerHover| self.hovered == Some(h);
        self.render_nav(
            theme,
            Rect::new(nl.origin.x, nl.origin.y, NAV_SIZE, NAV_SIZE),
            at_first,
            nav_hovered(PagerHover::First),
            |s, r, c| double_chevron(s, r, c, true),
            scene,
        );
        self.render_nav(
            theme,
            Rect::new(nl.origin.x + NAV_SIZE, nl.origin.y, NAV_SIZE, NAV_SIZE),
            at_first,
            nav_hovered(PagerHover::Prev),
            glyph::chevron_left,
            scene,
        );

        // 页码
        let selected_item = self.selected_item_index();
        for (i, (item, r)) in rects.iter().enumerate() {
            let selected = selected_item == Some(i);
            let hovered = match item {
                PagerItem::Number(p) => self.hovered == Some(PagerHover::Page(*p - 1)),
                PagerItem::Ellipsis => false,
            };
            let bg = if selected || hovered {
                colors.on_surface.with_alpha(0.10)
            } else {
                kanesumi_core::Color::TRANSPARENT
            };
            if bg.a > 0.0 {
                scene.fill_rounded_rect(bg, *r, theme.tokens.corner_radius);
            }
            match item {
                PagerItem::Number(p) => {
                    let fg = if selected {
                        colors.primary
                    } else {
                        colors.on_surface
                    };
                    let text = p.to_string();
                    let style = Self::page_number_style();
                    scene.text(
                        text,
                        Rect::new(
                            r.origin.x,
                            r.origin.y + (r.size.height - style.line_height) / 2.0,
                            r.size.width,
                            style.line_height,
                        ),
                        fg,
                        style,
                        TextAlign::Center,
                    );
                    // 选中指示条（2px 强调色，数字下方）
                    if selected {
                        scene.fill_rect(
                            colors.primary,
                            Rect::new(
                                r.origin.x,
                                r.bottom() - INDICATOR_H,
                                r.size.width,
                                INDICATOR_H,
                            ),
                        );
                    }
                }
                PagerItem::Ellipsis => {
                    let style = Self::page_number_style();
                    scene.text(
                        "…".into(),
                        *r,
                        colors.on_surface_variant,
                        style,
                        TextAlign::Center,
                    );
                }
            }
        }

        // Nav 右组（后/末）
        self.render_nav(
            theme,
            Rect::new(nr.origin.x, nr.origin.y, NAV_SIZE, NAV_SIZE),
            at_last,
            nav_hovered(PagerHover::Next),
            glyph::chevron_right,
            scene,
        );
        self.render_nav(
            theme,
            Rect::new(nr.origin.x + NAV_SIZE, nr.origin.y, NAV_SIZE, NAV_SIZE),
            at_last,
            nav_hovered(PagerHover::Last),
            |s, r, c| double_chevron(s, r, c, false),
            scene,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn render_nav(
        &self,
        theme: &MetroTheme,
        rect: Rect,
        hidden_on_edge: bool,
        hovered: bool,
        draw: impl Fn(&mut Scene, Rect, kanesumi_core::Color),
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        if hidden_on_edge {
            // 边缘隐藏：保留空间（对应模板 Opacity 0 + Disabled）
            return;
        }
        if hovered {
            scene.fill_rounded_rect(
                colors.on_surface.with_alpha(0.10),
                rect,
                theme.tokens.corner_radius,
            );
        }
        let chevron = Rect::new(
            rect.origin.x + rect.size.width * 0.22,
            rect.origin.y + rect.size.height * 0.25,
            rect.size.width * 0.56,
            rect.size.height * 0.5,
        );
        draw(scene, chevron, colors.on_surface);
    }
}

/// 双 chevron（◀◀ / ▶▶）。`is_first` = 左向双。
fn double_chevron(scene: &mut Scene, rect: Rect, color: kanesumi_core::Color, is_first: bool) {
    let gap = rect.size.width * 0.25;
    let w = rect.size.width * 0.32;
    let h = rect.size.height * 0.7;
    for x in [0.0, gap] {
        let sub = Rect::new(rect.origin.x + x, rect.origin.y, w, h);
        if is_first {
            glyph::chevron_left(scene, sub, color);
        } else {
            glyph::chevron_right(scene, sub, color);
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

    fn pager(pages: usize, selected: usize) -> MetroPagerControl {
        MetroPagerControl {
            number_of_pages: pages,
            selected_index: selected,
            ..MetroPagerControl::default()
        }
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 600.0, 40.0)
    }

    #[test]
    fn small_pages_show_all() {
        let p = pager(5, 2);
        let items = p.panel_items();
        assert_eq!(items.len(), 5);
        assert!(items.iter().all(|i| matches!(i, PagerItem::Number(_))));
    }

    #[test]
    fn start_window_shows_first_five() {
        let p = pager(20, 1);
        let items = p.panel_items();
        // 1 2 3 4 5 … 20
        assert_eq!(
            items,
            vec![
                PagerItem::Number(1),
                PagerItem::Number(2),
                PagerItem::Number(3),
                PagerItem::Number(4),
                PagerItem::Number(5),
                PagerItem::Ellipsis,
                PagerItem::Number(20),
            ]
        );
    }

    #[test]
    fn end_window_shows_last_five() {
        let p = pager(20, 18);
        let items = p.panel_items();
        // 1 … 16 17 18 19 20
        assert_eq!(
            items,
            vec![
                PagerItem::Number(1),
                PagerItem::Ellipsis,
                PagerItem::Number(16),
                PagerItem::Number(17),
                PagerItem::Number(18),
                PagerItem::Number(19),
                PagerItem::Number(20),
            ]
        );
    }

    #[test]
    fn center_window_shows_neighbors() {
        let p = pager(20, 9); // s=10
        let items = p.panel_items();
        // 1 … 9 10 11 … 20
        assert_eq!(
            items,
            vec![
                PagerItem::Number(1),
                PagerItem::Ellipsis,
                PagerItem::Number(9),
                PagerItem::Number(10),
                PagerItem::Number(11),
                PagerItem::Ellipsis,
                PagerItem::Number(20),
            ]
        );
    }

    #[test]
    fn selected_item_index_located() {
        let p = pager(20, 9);
        assert_eq!(p.selected_item_index(), Some(3), "10 在中间窗口第 4 位");
    }

    #[test]
    fn click_page_selects() {
        let mut p = pager(20, 9);
        let p0 = pager(20, 9);
        let engine = find_engine().unwrap();
        let (_, _, rects) = p0.geom(&engine, area());
        // 点 "9"（第 3 个元素）
        let r = rects[2].1;
        assert_eq!(
            p.handle_click(&engine, area(), r.center()),
            PagerAction::Select(8)
        );
        assert_eq!(p.selected_index, 8);
    }

    #[test]
    fn nav_changes_pages() {
        let mut p = pager(20, 5);
        let engine = find_engine().unwrap();
        let (nl, nr, _) = p.geom(&engine, area());
        // Prev
        assert_eq!(
            p.handle_click(
                &engine,
                area(),
                Point::new(nl.origin.x + NAV_SIZE + 5.0, 20.0)
            ),
            PagerAction::Prev
        );
        assert_eq!(p.selected_index, 4);
        // Next
        assert_eq!(
            p.handle_click(&engine, area(), Point::new(nr.origin.x + 5.0, 20.0)),
            PagerAction::Next
        );
        assert_eq!(p.selected_index, 5);
    }

    #[test]
    fn first_last_nav() {
        let mut p = pager(20, 5);
        let engine = find_engine().unwrap();
        let (nl, nr, _) = p.geom(&engine, area());
        // First（nav_left 左半）
        assert_eq!(
            p.handle_click(&engine, area(), Point::new(nl.origin.x + 10.0, 20.0)),
            PagerAction::First
        );
        assert_eq!(p.selected_index, 0);
        // Last（nav_right 右半）
        assert_eq!(
            p.handle_click(
                &engine,
                area(),
                Point::new(nr.origin.x + NAV_SIZE + 10.0, 20.0)
            ),
            PagerAction::Last
        );
        assert_eq!(p.selected_index, 19);
    }

    #[test]
    fn edges_hide_nav() {
        let engine = find_engine().unwrap();
        let p = pager(20, 0);
        let (nl, _, _) = p.geom(&engine, area());
        assert_eq!(
            p.hit(&engine, area(), Point::new(nl.origin.x + 10.0, 20.0)),
            PagerAction::None,
            "首页隐藏 First"
        );
        let p2 = pager(20, 19);
        let (_, nr, _) = p2.geom(&engine, area());
        assert_eq!(
            p2.hit(
                &engine,
                area(),
                Point::new(nr.origin.x + NAV_SIZE + 10.0, 20.0)
            ),
            PagerAction::None,
            "末页隐藏 Last"
        );
    }

    #[test]
    fn render_emits_selected_indicator() {
        let engine = find_engine().unwrap();
        let theme = MetroTheme::ether_dark();
        let p = pager(20, 9);
        let mut scene = Scene::default();
        p.render(&theme, &engine, area(), &mut scene);
        // 选中页下方应有强调色指示条
        let indicator = scene.commands.iter().filter(
            |c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.primary),
        );
        assert!(indicator.count() >= 1, "选中指示条为强调色填充");
        // 有双 chevron 三角形
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert!(
            tris >= 6,
            "First(双2)+Prev(1)+Next(1)+Last(双2) = 6 三角形，实际 {tris}"
        );
    }
}
