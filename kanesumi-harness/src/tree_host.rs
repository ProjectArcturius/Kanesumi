// tree_host.rs —— 元素树 App 的外壳适配器。参 docs/ELEMENT_TREE.md §Ⅷ。
//
// `TreeHost<A>` 实现现有 `App` trait，因此**外壳主循环零改动**，新（元素树）旧（手写几何）
// App 可以并存、逐个迁移。App 作者只写 `TreeApp`：启动时建树、响应控件动作；
// 命中、焦点、hover、损伤、IME 上下文全部由树负责，`TreeHost` 把它们接到 `App` 钩子上：
//
// | App 钩子            | 元素树                                              |
// |---------------------|-----------------------------------------------------|
// | handle_input        | 指针 / 键盘 / IME 路由（翻译 InputEvent → 树方法）  |
// | focus_move          | Tab 先投给焦点控件，未消费再走树序焦点遍历          |
// | needs_redraw        | `Tree::needs_frame`（失效 / 动画 / 状态变化）       |
// | hover_signature     | 恒定值：悬停变化由树自行失效，外壳不必每次 Move 都重画 |
// | render_into         | `Tree::frame`（动画 → 布局 → 绘制 → 拼接）          |
// | damage_hint         | 本帧重画节点的新旧绘制范围之并（外壳在 render 后取）|
// | ime_focus           | 焦点控件经 `Widget::ime` 产出的上下文               |
// | focus_changed(false)| 关闭可轻触关闭的弹层                                |

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};
use kanesumi_element::{Action, Key, Modifiers, Tree, WidgetId};

use crate::app::{App, AppConfig, ImeContext, InputEvent};
use crate::appmenu::{AppMenuHandle, MenuTree};

/// 元素树应用。
pub trait TreeApp {
    fn config(&self) -> &AppConfig;

    /// 启动时建树（把页面挂到 `tree.root()` 下）。
    fn build(&mut self, tree: &mut Tree);

    /// 控件动作回调（按钮点击、勾选切换、文本变化……）。`action` 按具体类型 downcast。
    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action);

    /// 每帧 tick：非控件状态（时钟、后台任务结果）在这里写回树。
    fn tick(&mut self, _tree: &mut Tree, _dt: f64) {}

    /// 初始主题（默认 Ether 深色）。之后外壳推送的系统主题由 `TreeHost` 直接交给树。
    fn theme(&self) -> MetroTheme {
        MetroTheme::ether_dark()
    }

    /// 系统主题变更通知（树已自行重排重画；App 只在自持颜色时需要处理）。
    fn on_theme(&mut self, _theme: MetroTheme) {}

    fn font_path(&self) -> Option<std::path::PathBuf> {
        None
    }

    fn preferred_height(&self) -> Option<f32> {
        None
    }

    fn should_close(&self) -> bool {
        false
    }

    fn app_menu(&self) -> Option<MenuTree> {
        None
    }

    fn on_menu_command(&mut self, _tree: &mut Tree, _id: i32) {}

    fn set_appmenu_handle(&mut self, _handle: AppMenuHandle) {}
}

/// 把 `TreeApp` 接成外壳可运行的 `App`。
pub struct TreeHost<A: TreeApp> {
    app: A,
    tree: Tree,
    /// 外壳 `update(dt)` 给的时间步，下一次 `render_into` 交给 `Tree::frame`。
    pending_dt: f64,
    /// 上一帧的损伤（外壳在 render 之后经 `damage_hint` 取走）。
    damage: Option<Rect>,
    /// 最近指针位置（滚轮事件不带坐标，命中要用）。
    pointer: Point,
}

impl<A: TreeApp> TreeHost<A> {
    pub fn new(mut app: A) -> Self {
        let mut tree = Tree::new(app.theme());
        app.build(&mut tree);
        Self {
            app,
            tree,
            pending_dt: 0.0,
            damage: None,
            pointer: Point::new(0.0, 0.0),
        }
    }

    pub fn app(&self) -> &A {
        &self.app
    }

    pub fn tree(&self) -> &Tree {
        &self.tree
    }

    /// 同时借出 App 与树（测试 / 外部驱动用）。
    pub fn parts_mut(&mut self) -> (&mut A, &mut Tree) {
        (&mut self.app, &mut self.tree)
    }

    /// 把本批控件动作交给 App。动作处理中 App 可能再改树、再产生动作 —— 排到空为止
    /// （上限防止两个控件互相触发的死循环）。
    fn drain_actions(&mut self) {
        for _ in 0..16 {
            let actions = self.tree.take_actions();
            if actions.is_empty() {
                return;
            }
            for (from, action) in actions {
                self.app.on_action(&mut self.tree, from, action);
            }
        }
        log::error!("TreeHost: 动作链 16 轮仍未收敛，疑似控件间循环触发");
    }
}

impl<A: TreeApp> App for TreeHost<A> {
    fn config(&self) -> &AppConfig {
        self.app.config()
    }

    fn theme(&self) -> MetroTheme {
        *self.tree.theme()
    }

    fn set_theme(&mut self, theme: MetroTheme) {
        if *self.tree.theme() != theme {
            self.tree.set_theme(theme);
            self.app.on_theme(theme);
        }
    }

    fn font_path(&self) -> Option<std::path::PathBuf> {
        self.app.font_path()
    }

    fn preferred_height(&self) -> Option<f32> {
        self.app.preferred_height()
    }

    fn update(&mut self, dt: f64) {
        self.pending_dt += dt;
        self.app.tick(&mut self.tree, dt);
        self.drain_actions();
    }

    fn needs_redraw(&self) -> bool {
        self.tree.needs_frame()
    }

    fn focus_move(&mut self, backward: bool) -> bool {
        let modifiers = Modifiers {
            shift: backward,
            ..Modifiers::NONE
        };
        let consumed = self.tree.key_down(Key::Tab, modifiers);
        self.drain_actions();
        consumed
    }

    fn hover_signature(&self) -> Option<u64> {
        Some(0)
    }

    fn damage_hint(&mut self) -> Option<Rect> {
        self.damage.take()
    }

    fn focus_changed(&mut self, focused: bool) {
        if !focused {
            self.tree.dismiss_popups();
            self.drain_actions();
        }
    }

    fn should_close(&self) -> bool {
        self.app.should_close()
    }

    fn handle_input(&mut self, event: InputEvent) {
        let t = &mut self.tree;
        match event {
            InputEvent::PointerMoved { x, y } => {
                self.pointer = Point::new(x, y);
                t.pointer_move(self.pointer);
            }
            InputEvent::PointerPressed {
                x,
                y,
                button,
                modifiers,
            } => {
                self.pointer = Point::new(x, y);
                t.pointer_down(self.pointer, button, modifiers);
            }
            InputEvent::PointerReleased {
                x,
                y,
                button,
                modifiers,
            } => {
                self.pointer = Point::new(x, y);
                t.pointer_up(self.pointer, button, modifiers);
            }
            InputEvent::DoubleClick { x, y, button, .. } => {
                t.pointer_double(Point::new(x, y), button);
            }
            InputEvent::Scroll { x, y, modifiers } => {
                t.scroll(self.pointer, x, y, modifiers);
            }
            InputEvent::KeyPressed { key, modifiers } => {
                t.key_down(key, modifiers);
            }
            InputEvent::PointerLeft => t.pointer_leave(),
            InputEvent::Preedit { text, cursor_byte } => {
                t.preedit(text, cursor_byte);
            }
            InputEvent::Commit { text } => {
                t.commit(text);
            }
            InputEvent::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => {
                t.delete_surrounding(before_bytes, after_bytes);
            }
        }
        self.drain_actions();
    }

    fn ime_focus(&self) -> Option<ImeContext> {
        self.tree.ime_context().cloned()
    }

    fn app_menu(&self) -> Option<MenuTree> {
        self.app.app_menu()
    }

    fn on_menu_command(&mut self, id: i32) {
        self.app.on_menu_command(&mut self.tree, id);
        self.drain_actions();
    }

    fn set_appmenu_handle(&mut self, handle: AppMenuHandle) {
        self.app.set_appmenu_handle(handle);
    }

    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene {
        let mut out = Scene::default();
        self.render_into(engine, size, &mut out);
        out
    }

    fn render_into(&mut self, engine: &TextEngine, size: Size, out: &mut Scene) {
        let dt = std::mem::take(&mut self.pending_dt);
        let frame = self.tree.frame(engine, size, dt);
        // 外壳契约：None = 全量。多帧未被取走时并入（保守）。
        self.damage = match (self.damage.take(), frame.damage) {
            (_, None) => None,
            (None, Some(d)) => Some(d),
            (Some(a), Some(b)) => Some(union(a, b)),
        };
        *out = frame.scene;
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::role::EtherRole;
    use kanesumi_controls::{ButtonClicked, MetroButton, MetroTextBox, TextChanged};
    use kanesumi_element::testing::test_engine;
    use kanesumi_element::widgets::Stack;
    use kanesumi_element::{Align, LayoutProps, PointerButton};

    struct Demo {
        config: AppConfig,
        button: Option<WidgetId>,
        clicks: u32,
        last_text: String,
    }

    impl TreeApp for Demo {
        fn config(&self) -> &AppConfig {
            &self.config
        }
        fn build(&mut self, tree: &mut Tree) {
            let col = tree.insert(tree.root(), Stack::column().with_spacing(8.0));
            let start = LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            };
            self.button = Some(tree.insert_with(col, MetroButton::new("确定"), start));
            tree.insert_with(col, MetroTextBox::with_placeholder("输入"), start);
        }
        fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
            if action.is::<ButtonClicked>() {
                self.clicks += 1;
            } else if let Ok(t) = action.downcast::<TextChanged>() {
                self.last_text = t.0;
            }
        }
    }

    fn host() -> (TreeHost<Demo>, TextEngine) {
        let host = TreeHost::new(Demo {
            config: AppConfig::new("org.ether.test", "test", EtherRole::Browser, 400.0, 300.0),
            button: None,
            clicks: 0,
            last_text: String::new(),
        });
        (host, test_engine())
    }

    fn frame(h: &mut TreeHost<Demo>, e: &TextEngine) -> Scene {
        h.update(1.0 / 60.0);
        let mut s = Scene::default();
        h.render_into(e, Size::new(400.0, 300.0), &mut s);
        s
    }

    #[test]
    fn app_trait_round_trip_click_tab_type_and_ime() {
        let (mut h, e) = host();
        let scene = frame(&mut h, &e);
        assert!(!scene.is_empty());
        assert_eq!(h.damage_hint(), None, "首帧全量");

        let btn = h.app().button.unwrap();
        let c = h.tree().rect(btn).unwrap().center();
        let press = |b| InputEvent::PointerPressed {
            x: c.x,
            y: c.y,
            button: b,
            modifiers: Modifiers::NONE,
        };
        h.handle_input(InputEvent::PointerMoved { x: c.x, y: c.y });
        h.handle_input(press(PointerButton::Left));
        h.handle_input(InputEvent::PointerReleased {
            x: c.x,
            y: c.y,
            button: PointerButton::Left,
            modifiers: Modifiers::NONE,
        });
        assert_eq!(h.app().clicks, 1);
        assert!(h.needs_redraw());
        frame(&mut h, &e);
        let d = h.damage_hint().expect("点击后应为局部损伤");
        assert!(d.size.width < 400.0);

        // 按钮已被点击聚焦；Tab 进输入框，键入 + IME 提交。
        assert!(h.focus_move(false));
        frame(&mut h, &e);
        assert!(h.ime_focus().is_some(), "输入框聚焦后外壳应拿到 IME 上下文");
        h.handle_input(InputEvent::KeyPressed {
            key: Key::Char('w'),
            modifiers: Modifiers::NONE,
        });
        h.handle_input(InputEvent::Commit { text: "无线".into() });
        assert_eq!(h.app().last_text, "w无线");
    }

    #[test]
    fn idle_tree_requests_no_frames() {
        let (mut h, e) = host();
        frame(&mut h, &e);
        assert!(!h.needs_redraw(), "静止零重绘");
        h.handle_input(InputEvent::PointerMoved { x: 390.0, y: 290.0 });
        assert!(!h.needs_redraw(), "空白处移动不触发重画");
    }
}
