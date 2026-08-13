// context_menu.rs —— 右键菜单状态机（App 侧 helper）。参 CONTEXT_MENU_SPEC.md §Ⅵ/§Ⅶ。
//
// App 持有一个 `ContextMenuState`，在 `handle_input` / `render` 中接入：
// - 右键按下 → `App::context_menu(x, y)` 取菜单内容 → `state.open(...)`；
// - 每帧 `state.update(dt)` 推进动画；`render` 把菜单画进主表面 Scene（坐标天然一致，
//   无浮层坐标转换）；点选 → `state.handle_pointer` 返回命令路径 → `on_context_command`。
//
// 菜单画在**主表面内**（右键目标是应用内容，坐标即表面本地坐标），无需浮层表面。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::app::{InputEvent, Key, PointerButton};
use kanesumi_controls::{MenuItem, MetroContextMenu};

/// 指针/键盘事件经菜单路由的结果。
#[derive(Debug, Clone, PartialEq)]
pub enum ContextMenuAction {
    /// 事件未被菜单消费（App 继续正常处理）。
    PassThrough,
    /// 事件被菜单消费（悬停/关闭/外点），App 不再处理。
    Consumed,
    /// 菜单项被点选。`path` 为命令路径（顶层 = `[i]`；级联 = `[parent, child]`）。
    Activate(Vec<usize>),
}

/// 右键菜单状态机 —— 打开/关闭/命中/点选/LightDismiss 全封装。
///
/// 与 `MetroContextMenu`（控件，纯画 + 定位）的职责划分：
/// 控件只画（内容注入、高亮态、翻折定位）；本状态机管生命周期与事件路由。
#[derive(Debug, Clone, Default)]
pub struct ContextMenuState {
    menu: MetroContextMenu,
    open: bool,
}

impl ContextMenuState {
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开右键菜单。`anchor` = 右键按下点（表面本地坐标）；`screen` = 可显示区域
    /// （主表面矩形）。空菜单直接不开。
    pub fn open(&mut self, engine: &TextEngine, anchor: Point, items: Vec<MenuItem>, screen: Rect) {
        if items.is_empty() {
            self.close();
            return;
        }
        self.menu.open_at(engine, anchor, items, screen);
        self.open = true;
    }

    /// 关闭（点选 / LightDismiss / Esc）。
    pub fn close(&mut self) {
        self.open = false;
        self.menu.close();
    }

    /// 菜单是否处于完全打开态（稳定 Open）。
    pub fn is_open(&self) -> bool {
        self.open && self.menu.is_open()
    }

    /// 菜单是否可见（含开关动画期间）。
    pub fn is_visible(&self) -> bool {
        self.open && self.menu.is_visible()
    }

    /// 菜单面板矩形（渲染/命中参考）。
    pub fn panel_rect(&self) -> Rect {
        self.menu.panel_rect()
    }

    /// 每帧动画 tick。
    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
    }

    /// 渲染到主表面 Scene。菜单关闭时零命令（无痕迹）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        if self.is_visible() {
            self.menu.render(theme, engine, scene);
        }
    }

    /// 指针/键盘事件路由。返回 [`ContextMenuAction`]：
    /// - 菜单未开：右键 PassThrough（App 自行决定是否 `context_menu`），其余 PassThrough；
    /// - 菜单开着：悬停 → Consumed；左键命中项 → Activate(path) + 关闭；
    ///   左键面板外 → LightDismiss 关闭（Consumed）；再按右键 → 关闭（Consumed）；
    ///   Esc → 关闭（Consumed）。
    pub fn handle_pointer(
        &mut self,
        engine: Option<&TextEngine>,
        event: &InputEvent,
        screen: Rect,
    ) -> ContextMenuAction {
        if !self.open || !self.menu.is_visible() {
            return ContextMenuAction::PassThrough;
        }
        match event {
            InputEvent::PointerMoved { x, y } => {
                let Some(engine) = engine else {
                    return ContextMenuAction::Consumed;
                };
                self.menu.hover(engine, screen, Point::new(*x, *y));
                ContextMenuAction::Consumed
            }
            InputEvent::PointerPressed {
                x,
                y,
                button: PointerButton::Left,
                ..
            } => {
                let pos = Point::new(*x, *y);
                if let Some(path) = self.menu.path_at(pos) {
                    let path = match path.parent {
                        None => vec![path.index],
                        Some(p) => vec![p, path.index],
                    };
                    self.select(&path);
                    self.close();
                    ContextMenuAction::Activate(path)
                } else {
                    // LightDismiss：点面板外关闭。
                    self.close();
                    ContextMenuAction::Consumed
                }
            }
            InputEvent::PointerPressed {
                button: PointerButton::Right,
                ..
            } => {
                // 再按右键：关闭当前菜单（Windows 惯例）。
                self.close();
                ContextMenuAction::Consumed
            }
            InputEvent::PointerReleased { .. } | InputEvent::Scroll { .. } => {
                ContextMenuAction::Consumed
            }
            InputEvent::KeyPressed { key: Key::Escape, .. } => {
                self.close();
                ContextMenuAction::Consumed
            }
            _ => ContextMenuAction::PassThrough,
        }
    }

    /// 选中菜单项后的勾选同步（单选组互斥）。`path` 与 [`ContextMenuAction::Activate`] 同形。
    fn select(&mut self, path: &[usize]) {
        let path = match path {
            [i] => kanesumi_controls::MenuPath {
                parent: None,
                index: *i,
            },
            [p, c] => kanesumi_controls::MenuPath {
                parent: Some(*p),
                index: *c,
            },
            _ => return,
        };
        let _ = self.menu.select(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: Rect = Rect::new(0.0, 0.0, 1024.0, 768.0);

    fn engine() -> Option<TextEngine> {
        // 菜单布局需文本引擎；测试环境缺失字体时跳过面板尺寸相关断言。
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            return TextEngine::load(p).ok();
        }
        for p in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::with_icon("剪切", "✂"),
            MenuItem::with_icon("复制", "📋"),
            MenuItem::new("粘贴").separator(),
            MenuItem::new("更多").with_submenu(vec![
                MenuItem::new("重命名"),
                MenuItem::new("删除"),
            ]),
        ]
    }

    #[test]
    fn open_sets_visible() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        assert!(s.is_open());
        assert!(s.is_visible());
    }

    #[test]
    fn empty_items_do_not_open() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), Vec::new(), SCREEN);
        assert!(!s.is_open());
    }

    #[test]
    fn click_on_item_activates() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        let r = s.panel_rect();
        let ev = InputEvent::PointerPressed {
            x: r.origin.x + 20.0,
            y: r.origin.y + 10.0,
            button: PointerButton::Left,
            modifiers: crate::Modifiers::NONE,
        };
        let action = s.handle_pointer(Some(&engine), &ev, SCREEN);
        assert_eq!(action, ContextMenuAction::Activate(vec![0]));
        assert!(!s.is_visible(), "点选后关闭");
    }

    #[test]
    fn click_outside_dismisses() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        let ev = InputEvent::PointerPressed {
            x: 5.0,
            y: 5.0,
            button: PointerButton::Left,
            modifiers: crate::Modifiers::NONE,
        };
        assert_eq!(s.handle_pointer(Some(&engine), &ev, SCREEN), ContextMenuAction::Consumed);
        assert!(!s.is_visible(), "LightDismiss 关闭");
    }

    #[test]
    fn right_press_closes_when_open() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        let ev = InputEvent::PointerPressed {
            x: 50.0,
            y: 50.0,
            button: PointerButton::Right,
            modifiers: crate::Modifiers::NONE,
        };
        assert_eq!(s.handle_pointer(Some(&engine), &ev, SCREEN), ContextMenuAction::Consumed);
        assert!(!s.is_visible());
    }

    #[test]
    fn escape_closes() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        let ev = InputEvent::KeyPressed {
            key: Key::Escape,
            modifiers: crate::Modifiers::NONE,
        };
        assert_eq!(s.handle_pointer(Some(&engine), &ev, SCREEN), ContextMenuAction::Consumed);
        assert!(!s.is_visible());
    }

    #[test]
    fn passes_through_when_closed() {
        let mut s = ContextMenuState::new();
        let ev = InputEvent::PointerPressed {
            x: 50.0,
            y: 50.0,
            button: PointerButton::Right,
            modifiers: crate::Modifiers::NONE,
        };
        // 菜单未开时 handle_pointer 直接 PassThrough（不触碰 engine）。
        let none_engine: Option<TextEngine> = None;
        assert_eq!(
            s.handle_pointer(none_engine.as_ref(), &ev, SCREEN),
            ContextMenuAction::PassThrough
        );
    }

    #[test]
    fn hover_is_consumed() {
        let Some(engine) = engine() else { return };
        let mut s = ContextMenuState::new();
        s.open(&engine, Point::new(100.0, 100.0), items(), SCREEN);
        s.update(1.0);
        let ev = InputEvent::PointerMoved { x: 100.0, y: 100.0 };
        assert_eq!(s.handle_pointer(Some(&engine), &ev, SCREEN), ContextMenuAction::Consumed);
        assert!(s.is_visible(), "悬停不关闭");
    }
}
