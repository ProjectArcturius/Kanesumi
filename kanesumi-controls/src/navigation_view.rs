// MetroNavigationView —— 侧边导航。参 CONTROL_SPEC §28。
//
// 移植自 microsoft-ui-xaml/dev/NavigationView（NavigationView.xaml + NavigationView_themeresources.xaml）：
// - Left 模式：Expanded Pane 320 / Compact 48；Toggle 40×40；Item 高 40、icon 16、字 14；
//   选中指示条 3×16 强调色；Header Margin 56,44,0,0；
// - Top 模式：顶栏 48，项横排。
// 子项级联 / flyout / 动画式收窄暂略（Phase 3 续）。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect};

/// Expanded Pane 宽（320）。
pub const NAV_PANE_EXPANDED: f32 = 320.0;
/// Compact Pane 宽（48）。
pub const NAV_PANE_COMPACT: f32 = 48.0;
/// Top Pane 高（48）。
pub const NAV_TOP_HEIGHT: f32 = 48.0;
/// Toggle 按钮 40×40。
pub const NAV_TOGGLE: f32 = 40.0;
/// Item 高（40）。
pub const NAV_ITEM_H: f32 = 40.0;
/// Header Margin（56,44,0,0 → 左 56、上 44）。
pub const NAV_HEADER_MARGIN: (f32, f32) = (56.0, 44.0);
/// 选中指示条 3×16。
pub const NAV_INDICATOR: (f32, f32) = (3.0, 16.0);

/// 导航模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationPaneMode {
    Left,
    Top,
}

/// 导航项。
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationViewItem {
    pub label: String,
    /// 图标文本（16px；None = 无图标）。
    pub icon: Option<String>,
    /// 子项（Left 模式缩进展示）。
    pub children: Vec<NavigationViewItem>,
}

impl NavigationViewItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            children: Vec::new(),
        }
    }

    pub fn with_icon(label: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon.into()),
            children: Vec::new(),
        }
    }
}

/// 导航点击结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationAction {
    None,
    /// 选中项（索引路径）。
    Select(Vec<usize>),
    /// Pane toggle（Left 模式展开/收窄）。
    Toggle,
}

/// MetroNavigationView —— 侧边导航。参 CONTROL_SPEC §28。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroNavigationView {
    pub items: Vec<NavigationViewItem>,
    pub footer_items: Vec<NavigationViewItem>,
    /// 选中路径（索引）。
    pub selected: Option<Vec<usize>>,
    pub mode: NavigationPaneMode,
    /// Left 模式 Pane 展开态（320）vs 收窄（48）。
    pub pane_expanded: bool,
    /// 头部标题（Header 区，宿主也可自绘）。
    pub header: String,
    pub toggle_hovered: bool,
    pub footer_selected: Option<usize>,
}

impl Default for MetroNavigationView {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            footer_items: Vec::new(),
            selected: None,
            mode: NavigationPaneMode::Left,
            pane_expanded: true,
            header: String::new(),
            toggle_hovered: false,
            footer_selected: None,
        }
    }
}

impl MetroNavigationView {
    pub fn new(items: Vec<NavigationViewItem>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// 当前 Pane 宽（Left 模式）。
    pub fn pane_width(&self) -> f32 {
        if self.pane_expanded {
            NAV_PANE_EXPANDED
        } else {
            NAV_PANE_COMPACT
        }
    }

    /// Toggle 按钮 rect。
    pub fn toggle_rect(&self, rect: Rect) -> Rect {
        match self.mode {
            NavigationPaneMode::Left => {
                Rect::new(rect.origin.x, rect.origin.y, NAV_TOGGLE, NAV_TOGGLE)
            }
            NavigationPaneMode::Top => {
                Rect::new(rect.origin.x, rect.origin.y, NAV_TOGGLE, NAV_TOGGLE)
            }
        }
    }

    /// 项区（Left：Pane 内 Toggle 下方；Top：Toggle 右侧横排）。
    fn item_area(&self, rect: Rect) -> Rect {
        match self.mode {
            NavigationPaneMode::Left => Rect::new(
                rect.origin.x,
                rect.origin.y + NAV_TOGGLE,
                self.pane_width(),
                rect.size.height - NAV_TOGGLE,
            ),
            NavigationPaneMode::Top => Rect::new(
                rect.origin.x + NAV_TOGGLE,
                rect.origin.y,
                rect.size.width - NAV_TOGGLE,
                NAV_TOP_HEIGHT,
            ),
        }
    }

    /// 顶层项 rect（按模式排布）。
    pub fn top_item_rects(&self, rect: Rect) -> Vec<Rect> {
        let area = self.item_area(rect);
        match self.mode {
            NavigationPaneMode::Left => (0..self.items.len())
                .map(|i| {
                    Rect::new(
                        area.origin.x,
                        area.origin.y + i as f32 * NAV_ITEM_H,
                        area.size.width,
                        NAV_ITEM_H,
                    )
                })
                .collect(),
            NavigationPaneMode::Top => {
                let mut x = area.origin.x;
                (0..self.items.len())
                    .map(|_| {
                        let w = NAV_ITEM_H + 16.0; // 固定 item 宽（含 label）
                        let r = Rect::new(x, area.origin.y, w, NAV_ITEM_H);
                        x += w;
                        r
                    })
                    .collect()
            }
        }
    }

    /// Header rect（Left：pane 右、y=44；Top：顶栏下）。
    pub fn header_rect(&self, rect: Rect) -> Rect {
        match self.mode {
            NavigationPaneMode::Left => Rect::new(
                rect.origin.x + NAV_HEADER_MARGIN.0,
                rect.origin.y + NAV_HEADER_MARGIN.1,
                rect.size.width - self.pane_width() - NAV_HEADER_MARGIN.0,
                NAV_ITEM_H,
            ),
            NavigationPaneMode::Top => Rect::new(
                rect.origin.x + NAV_TOGGLE,
                rect.origin.y + NAV_TOP_HEIGHT,
                rect.size.width - NAV_TOGGLE,
                NAV_ITEM_H,
            ),
        }
    }

    /// Content rect（剩余区域）。
    pub fn content_rect(&self, rect: Rect) -> Rect {
        let hr = self.header_rect(rect);
        Rect::new(
            hr.origin.x,
            hr.bottom(),
            hr.size.width,
            rect.bottom() - hr.bottom(),
        )
    }

    fn item_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Normal)
    }

    /// 命中：Toggle / 顶层项。
    pub fn hit(&self, rect: Rect, pos: Point) -> NavigationAction {
        if self.toggle_rect(rect).contains(pos) {
            return NavigationAction::Toggle;
        }
        for (i, r) in self.top_item_rects(rect).iter().enumerate() {
            if r.contains(pos) {
                return NavigationAction::Select(vec![i]);
            }
        }
        NavigationAction::None
    }

    /// 应用点击：Toggle / Select。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> NavigationAction {
        match self.hit(rect, pos) {
            NavigationAction::Select(path) => {
                self.selected = Some(path.clone());
                NavigationAction::Select(path)
            }
            NavigationAction::Toggle if self.mode == NavigationPaneMode::Left => {
                self.pane_expanded = !self.pane_expanded;
                NavigationAction::Toggle
            }
            NavigationAction::Toggle => NavigationAction::Toggle,
            NavigationAction::None => NavigationAction::None,
        }
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.toggle_hovered = self.toggle_rect(rect).contains(pos);
    }

    /// 渲染 Pane / Top 栏（Toggle + 项列表 + Footer）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = Self::item_style();

        // Toggle（☰ 自绘三横）
        let tr = self.toggle_rect(rect);
        if self.toggle_hovered {
            scene.fill_rounded_rect(
                colors.on_surface.with_alpha(0.10),
                tr,
                theme.tokens.corner_radius,
            );
        }
        let cx = tr.center().x;
        let cy = tr.center().y;
        let bar_w = 14.0;
        let t = 1.5;
        for dy in [-6.0, 0.0, 6.0] {
            scene.fill_rect(
                colors.on_surface,
                Rect::new(cx - bar_w / 2.0, cy + dy - t / 2.0, bar_w, t),
            );
        }

        let rects = self.top_item_rects(rect);
        // 子项可见性（Left 展开 + 选中展开）——简化为展开态展示 children。
        for (i, item) in self.items.iter().enumerate() {
            let r = rects[i];
            let selected = self.selected.as_deref() == Some([i].as_slice());
            // 底
            if selected {
                scene.fill_rect(colors.on_surface.with_alpha(0.08), r);
            }
            // 选中指示条（3×16 强调色，左侧）
            if selected {
                scene.fill_rect(
                    colors.primary,
                    Rect::new(
                        r.origin.x,
                        r.origin.y + (r.size.height - NAV_INDICATOR.1) / 2.0,
                        NAV_INDICATOR.0,
                        NAV_INDICATOR.1,
                    ),
                );
            }
            // icon
            let mut x = r.origin.x + 16.0;
            if let Some(icon) = &item.icon {
                let icon_rect = Rect::new(x, r.origin.y + (r.size.height - 16.0) / 2.0, 16.0, 16.0);
                scene.text(
                    icon.clone(),
                    icon_rect,
                    colors.on_surface_variant,
                    TextStyle::new(16.0, 16.0, FontWeight::Normal),
                    TextAlign::Center,
                );
                x += 16.0 + 12.0;
            } else {
                x += 16.0; // 无 icon 时 label 顶到 padding 16
            }
            // label（Compact 模式隐藏）
            if self.pane_expanded || self.mode == NavigationPaneMode::Top {
                let label_w = (r.right() - x - 12.0).max(0.0);
                let fg = if selected {
                    colors.on_surface
                } else {
                    colors.on_surface_variant
                };
                scene.text(
                    item.label.clone(),
                    Rect::new(
                        x,
                        r.origin.y + (r.size.height - style.line_height) / 2.0,
                        label_w,
                        style.line_height,
                    ),
                    fg,
                    style,
                    TextAlign::Left,
                );
                // 子项 chevron
                if !item.children.is_empty() {
                    let chev = Rect::new(
                        r.right() - 28.0,
                        r.origin.y + (r.size.height - 12.0) / 2.0,
                        12.0,
                        12.0,
                    );
                    glyph::chevron_right(scene, chev, colors.on_surface_variant);
                }
            }

            // 子项（展开时）
            if self.pane_expanded && !item.children.is_empty() {
                let base_y = r.bottom();
                for (j, child) in item.children.iter().enumerate() {
                    let cr = Rect::new(
                        rect.origin.x + 16.0,
                        base_y + j as f32 * NAV_ITEM_H,
                        self.pane_width() - 16.0,
                        NAV_ITEM_H,
                    );
                    let csel = self.selected.as_deref() == Some([i, j].as_slice());
                    if csel {
                        scene.fill_rect(colors.on_surface.with_alpha(0.08), cr);
                    }
                    scene.text(
                        child.label.clone(),
                        Rect::new(
                            cr.origin.x + 16.0,
                            cr.origin.y + (cr.size.height - style.line_height) / 2.0,
                            cr.size.width - 16.0,
                            style.line_height,
                        ),
                        colors.on_surface_variant,
                        style,
                        TextAlign::Left,
                    );
                }
            }
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
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn nav() -> MetroNavigationView {
        MetroNavigationView::new(vec![
            NavigationViewItem::with_icon("设置", "⚙"),
            NavigationViewItem::with_icon("外观", "◐"),
            NavigationViewItem::new("关于"),
        ])
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    #[test]
    fn pane_widths() {
        let n = nav();
        assert_eq!(n.pane_width(), 320.0);
        let compact = MetroNavigationView {
            pane_expanded: false,
            ..nav()
        };
        assert_eq!(compact.pane_width(), 48.0);
    }

    #[test]
    fn toggle_click_switches() {
        let mut n = nav();
        let r = area();
        assert_eq!(
            n.handle_click(r, Point::new(20.0, 20.0)),
            NavigationAction::Toggle
        );
        assert!(!n.pane_expanded, "点 toggle 收窄");
        assert_eq!(n.pane_width(), 48.0);
    }

    #[test]
    fn select_top_item() {
        let mut n = nav();
        let r = area();
        let rects = n.top_item_rects(r);
        let second = rects[1];
        assert_eq!(
            n.handle_click(r, second.center()),
            NavigationAction::Select(vec![1])
        );
        assert_eq!(n.selected.as_deref(), Some([1].as_slice()));
    }

    #[test]
    fn content_rect_beside_pane() {
        let n = nav();
        let r = area();
        let cr = n.content_rect(r);
        assert_eq!(cr.origin.x, 56.0, "Header Margin 左 56");
        assert_eq!(
            cr.origin.y,
            44.0 + 40.0,
            "Header Margin 上 44 + header 高 40"
        );
    }

    #[test]
    fn top_mode_horizontal() {
        let n = MetroNavigationView {
            mode: NavigationPaneMode::Top,
            ..nav()
        };
        let r = area();
        let rects = n.top_item_rects(r);
        // 横排：y 相同、x 递增
        assert_eq!(rects[0].origin.y, rects[1].origin.y);
        assert!(rects[1].origin.x > rects[0].origin.x);
        // 顶栏下 header
        assert_eq!(n.header_rect(r).origin.y, NAV_TOP_HEIGHT);
    }

    #[test]
    fn indicator_emitted_on_selected() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut n = nav();
        n.selected = Some(vec![1]);
        let mut scene = Scene::default();
        n.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let indicator = scene.commands.iter().any(
            |c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.primary),
        );
        assert!(indicator, "选中项应有强调色指示条");
    }

    #[test]
    fn compact_hides_labels() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut n = nav();
        n.pane_expanded = false;
        let mut scene = Scene::default();
        n.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        // Compact：只留 icon 文本（2 个带图标项），无 label（"关于" 无图标 → 无文本）
        assert_eq!(texts, 2, "Compact 只显示图标，实际 {texts}");
    }
}
