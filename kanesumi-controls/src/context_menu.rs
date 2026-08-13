// context_menu.rs —— 右键菜单（ContextMenu）。参 CONTEXT_MENU_SPEC.md。
//
// 语义：右键按下 → 在指针位置弹出菜单（锚定指针点，四象限翻折，无遮罩）。
// 复用 `MetroDropdownMenu`（项规格/级联/单选组/悬停高亮全部复用），差异：
// - 锚点 = 指针坐标（`place_context_menu`），非触发器矩形（`place_popup`）；
// - 无遮罩（LightDismiss，点菜单外关闭）；
// - 按「按下触发」打开（非 DropdownMenu 的 toggle）。
//
// Kanesumi 只负责画：菜单内容（哪些命令/图标/勾选态）由 App 提供（`Vec<MenuItem>` 纯数据），
// 本控件不持目标/命令语义。参 CONTEXT_MENU_SPEC §Ⅰ.3。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::dropdown_menu::{MenuItem, MenuPath, MetroDropdownMenu};
use crate::popup::{PopupState, place_context_menu};

/// 右键菜单控件 —— 无遮罩弹层，锚定指针。
///
/// 内部持一个 [`MetroDropdownMenu`] 承担项/级联/悬停；本控件加右键语义层：
/// - `anchor`：右键按下点（定位基准）；
/// - 打开 = `open_at`（锚点 + 内容 + 屏幕矩形），关闭 = `close`（LightDismiss）。
#[derive(Debug, Clone)]
pub struct MetroContextMenu {
    menu: MetroDropdownMenu,
    /// 右键按下点（面板定位基准）。
    pub anchor: Point,
}

impl MetroContextMenu {
    pub fn new() -> Self {
        Self {
            menu: MetroDropdownMenu::new(Vec::new()),
            anchor: Point::new(0.0, 0.0),
        }
    }

    /// 在 `anchor`（指针坐标）打开右键菜单。`screen` 为可显示区域（输出/表面矩形）。
    ///
    /// 面板经 [`place_context_menu`] 四象限翻折定位；动画按菜单弹出轨道（无遮罩）。
    pub fn open_at(
        &mut self,
        engine: &TextEngine,
        anchor: Point,
        items: Vec<MenuItem>,
        screen: Rect,
    ) {
        self.anchor = anchor;
        self.menu.items = items;
        self.menu.invalidate_layout();
        let size = self.menu.panel_size(engine);
        let rect = place_context_menu(anchor, size, screen);
        self.menu.open(rect);
    }

    /// 关闭（LightDismiss / 点选 / Esc）。回收级联。
    pub fn close(&mut self) {
        self.menu.close();
    }

    /// 每帧动画 tick。
    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
    }

    /// 面板是否可见（含开/关动画期间）。
    pub fn is_visible(&self) -> bool {
        self.menu.anim.is_visible()
    }

    /// 面板是否已完全打开（稳定 Open 态）。
    pub fn is_open(&self) -> bool {
        self.menu.anim.is_open()
    }

    /// 悬停路由（复用 DropdownMenu 级联语义）：命中项高亮，悬停嵌套项自动展开子菜单。
    pub fn hover(&mut self, engine: &TextEngine, screen: Rect, pos: Point) {
        self.menu.hover(engine, screen, pos);
    }

    /// 命中测试（含级联）：返回 `(parent, child)` 路径或顶层索引。
    pub fn path_at(&self, pos: Point) -> Option<MenuPath> {
        self.menu.path_at(pos)
    }

    /// 菜单项点选后标记勾选态（单选组互斥）。
    pub fn select(&mut self, path: MenuPath) -> bool {
        match path.parent {
            None => self.menu.select(path.index),
            Some(_) => self.menu.select_submenu(path),
        }
    }

    /// 渲染：面板 + 级联（**无遮罩**）。关态渲染空（透明不可见）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        self.menu.render_panel(theme, engine, scene);
        self.menu.render_submenu(theme, engine, scene);
    }

    /// 面板矩形（渲染/命中参考）。
    pub fn panel_rect(&self) -> Rect {
        self.menu.panel_rect
    }

    /// 菜单状态（供外部判断/测试）。
    pub fn state(&self) -> PopupState {
        self.menu.state()
    }
}

impl Default for MetroContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCREEN: Rect = Rect::new(0.0, 0.0, 1024.0, 768.0);

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            return TextEngine::load(p).ok();
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

    #[test]
    fn open_at_anchors_at_pointer() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(300.0, 300.0),
            vec![MenuItem::new("Reset")],
            TEST_SCREEN,
        );
        assert!(menu.is_visible());
        // 屏幕中央，右侧/下方空间足 → 面板右上角 = 指针。
        assert_eq!(menu.panel_rect().origin, Point::new(300.0, 300.0));
        // 无遮罩：不渲染全屏遮罩矩形（render_panel 只画面板）。
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        menu.render(&theme, &engine, &mut scene);
        let panel = menu.panel_rect();
        assert!(
            scene.commands.iter().all(|c| match c {
                kanesumi_canvas::SceneCommand::FillRect { rect, .. } => {
                    // 全屏遮罩 = 覆盖整个 TEST_SCREEN 的 FillRect；面板内矩形允许。
                    let full = rect.origin.x <= 0.5
                        && rect.origin.y <= 0.5
                        && (rect.origin.x + rect.size.width) >= TEST_SCREEN.right() - 0.5
                        && (rect.origin.y + rect.size.height) >= TEST_SCREEN.bottom() - 0.5;
                    !full || (rect.origin.x >= panel.origin.x - 0.5
                        && rect.origin.y >= panel.origin.y - 0.5)
                }
                _ => true,
            }),
            "右键菜单不渲染全屏遮罩"
        );
    }

    #[test]
    fn open_at_flips_to_fit_screen_corner() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(1000.0, 750.0),
            vec![MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C")],
            TEST_SCREEN,
        );
        let r = menu.panel_rect();
        assert!(
            r.origin.x + r.size.width <= TEST_SCREEN.right() + 0.01,
            "面板右缘不越屏"
        );
        assert!(
            r.origin.y + r.size.height <= TEST_SCREEN.bottom() + 0.01,
            "面板下缘不越屏"
        );
    }

    #[test]
    fn closes_to_invisible() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(300.0, 300.0),
            vec![MenuItem::new("Reset")],
            TEST_SCREEN,
        );
        menu.close();
        assert!(menu.is_visible(), "关闭动画期间仍可见");
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert!(!menu.is_visible());
        assert_eq!(menu.state(), PopupState::Closed);
    }

    #[test]
    fn default_is_invisible() {
        let menu = MetroContextMenu::new();
        assert!(!menu.is_visible());
        assert!(!menu.is_open());
    }

    #[test]
    fn path_at_hits_items_after_open() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(100.0, 100.0),
            vec![MenuItem::new("A"), MenuItem::new("B")],
            TEST_SCREEN,
        );
        menu.update(1.0);
        let r = menu.panel_rect();
        // 第一项：面板内顶部。
        let hit = menu.path_at(Point::new(r.origin.x + 20.0, r.origin.y + 10.0));
        assert_eq!(hit, Some(MenuPath { parent: None, index: 0 }));
        // 面板外 → None。
        assert_eq!(menu.path_at(Point::new(5.0, 5.0)), None);
    }
}
