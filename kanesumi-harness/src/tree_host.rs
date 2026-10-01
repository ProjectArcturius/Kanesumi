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
// | enable_popups 等    | 弹层分离：覆盖层弹层各由外壳开 xdg_popup 承载（POPUP_PLAN）|

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};
use kanesumi_element::{Action, Key, Modifiers, Tree, WidgetId};

use crate::app::{App, AppConfig, FloatingLayer, ImeContext, InputEvent, PopupRequest};
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

    /// 应用级快捷键（XAML `KeyboardAccelerator`）：**没有任何控件处理**的按键才会到这里
    /// （无焦点时的数字键、Ctrl+Q 等）。返回 true = 已消费。
    fn on_key(&mut self, _tree: &mut Tree, _key: Key, _modifiers: Modifiers) -> bool {
        false
    }

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

    // ── 浮层表面（Launcher 全屏层、面板…）：每个浮层一棵独立的元素树 ─────────

    /// 浮层表面声明（启动时读一次，外壳据此建 layer 表面）。
    fn floating_layers(&self) -> Vec<FloatingLayer> {
        Vec::new()
    }

    /// 启动时为第 `index` 个浮层建树。
    fn build_floating(&mut self, _index: usize, _tree: &mut Tree) {}

    /// 浮层当前是否显示（隐藏时不渲染、不收输入）。
    fn floating_visible(&self, _index: usize) -> bool {
        true
    }

    /// 浮层请求高度（非全屏浮层；0 = 收起）。
    fn floating_height(&self, _index: usize) -> f32 {
        0.0
    }

    /// 浮层树上的控件动作。`main` 是主表面树（一个动作可以同时改两边，如 Launcher 里点应用后关层）。
    fn on_floating_action(
        &mut self,
        _index: usize,
        _tree: &mut Tree,
        _main: &mut Tree,
        _from: WidgetId,
        _action: Action,
    ) {
    }

    /// 浮层树的每帧 tick（与 `tick` 对应）。
    fn tick_floating(&mut self, _index: usize, _tree: &mut Tree, _dt: f64) {}
}

/// 一个浮层表面的树与输入状态。
struct FloatingTree {
    tree: Tree,
    pending_dt: f64,
    pointer: Point,
    /// 上一次 `render_floating` 的帧损伤（`None` = 整幅）。
    damage: Option<Rect>,
}

/// 外壳输入 → 树方法（主表面 / 浮层共用）。返回没有控件处理的按键（交给 App 加速键）。
fn feed(t: &mut Tree, pointer: &mut Point, event: InputEvent) -> Option<(Key, Modifiers)> {
    match event {
        InputEvent::PointerMoved { x, y } => {
            *pointer = Point::new(x, y);
            t.pointer_move(*pointer);
        }
        InputEvent::PointerPressed { x, y, button, modifiers } => {
            *pointer = Point::new(x, y);
            t.pointer_down(*pointer, button, modifiers);
        }
        InputEvent::PointerReleased { x, y, button, modifiers } => {
            *pointer = Point::new(x, y);
            t.pointer_up(*pointer, button, modifiers);
        }
        InputEvent::DoubleClick { x, y, button, .. } => {
            t.pointer_double(Point::new(x, y), button);
        }
        InputEvent::Scroll { x, y, modifiers } => {
            t.scroll(*pointer, x, y, modifiers);
        }
        InputEvent::KeyPressed { key, modifiers } => {
            if !t.key_down(key, modifiers) {
                return Some((key, modifiers));
            }
        }
        InputEvent::PointerLeft => t.pointer_leave(),
        InputEvent::Preedit { text, cursor_byte } => {
            t.preedit(text, cursor_byte);
        }
        InputEvent::Commit { text } => {
            t.commit(text);
        }
        InputEvent::DeleteSurrounding { before_bytes, after_bytes } => {
            t.delete_surrounding(before_bytes, after_bytes);
        }
    }
    None
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
    /// 弹层分离模式下需重画的弹层（key = `WidgetId::to_u64`）。
    popup_dirty: std::collections::HashSet<u64>,
    /// 浮层表面的树（与 `TreeApp::floating_layers` 一一对应）。
    floating: Vec<FloatingTree>,
}

impl<A: TreeApp> TreeHost<A> {
    pub fn new(mut app: A) -> Self {
        let mut tree = Tree::new(app.theme());
        app.build(&mut tree);
        let floating = (0..app.floating_layers().len())
            .map(|i| {
                let mut t = Tree::new(app.theme());
                app.build_floating(i, &mut t);
                // 浮层恒为 CPU 光栅 + 局部提交（外壳按 `floating_damage` 只重画损伤区）。
                t.set_damage_cull(true);
                FloatingTree { tree: t, pending_dt: 0.0, pointer: Point::new(0.0, 0.0), damage: None }
            })
            .collect();
        Self {
            app,
            tree,
            pending_dt: 0.0,
            damage: None,
            pointer: Point::new(0.0, 0.0),
            popup_dirty: std::collections::HashSet::new(),
            floating,
        }
    }

    /// 第 `index` 个浮层的树（测试 / 外部驱动用）。
    pub fn floating_tree(&self, index: usize) -> Option<&Tree> {
        self.floating.get(index).map(|f| &f.tree)
    }

    /// 弹层 key → 元素 id（只认当前打开的弹层）。
    fn popup_id(&self, key: u64) -> Option<WidgetId> {
        self.tree.popups().find(|id| id.to_u64() == key)
    }

    /// 弹层表面局部坐标 → 树坐标（平移弹层绘制范围原点）。
    fn popup_to_tree(&self, key: u64, event: InputEvent) -> Option<InputEvent> {
        let id = self.popup_id(key)?;
        let o = self.tree.popup_extent(id)?.origin;
        Some(match event {
            InputEvent::PointerMoved { x, y } => InputEvent::PointerMoved { x: x + o.x, y: y + o.y },
            InputEvent::PointerPressed { x, y, button, modifiers } => {
                InputEvent::PointerPressed { x: x + o.x, y: y + o.y, button, modifiers }
            }
            InputEvent::PointerReleased { x, y, button, modifiers } => {
                InputEvent::PointerReleased { x: x + o.x, y: y + o.y, button, modifiers }
            }
            InputEvent::DoubleClick { x, y, button, modifiers } => {
                InputEvent::DoubleClick { x: x + o.x, y: y + o.y, button, modifiers }
            }
            other => other,
        })
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
            let mut any = false;
            let actions = self.tree.take_actions();
            any |= !actions.is_empty();
            for (from, action) in actions {
                self.app.on_action(&mut self.tree, from, action);
            }
            for i in 0..self.floating.len() {
                let actions = self.floating[i].tree.take_actions();
                any |= !actions.is_empty();
                for (from, action) in actions {
                    self.app
                        .on_floating_action(i, &mut self.floating[i].tree, &mut self.tree, from, action);
                }
            }
            if !any {
                return;
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
            for f in &mut self.floating {
                f.tree.set_theme(theme);
            }
            self.app.on_theme(theme);
        }
    }

    fn font_path(&self) -> Option<std::path::PathBuf> {
        self.app.font_path()
    }

    fn preferred_height(&self) -> Option<f32> {
        self.app.preferred_height()
    }

    fn floating_layers(&self) -> Vec<FloatingLayer> {
        self.app.floating_layers()
    }

    fn floating_visible(&self, index: usize) -> bool {
        self.app.floating_visible(index)
    }

    fn floating_height(&self, index: usize) -> f32 {
        self.app.floating_height(index)
    }

    fn floating_needs_redraw(&self, index: usize) -> bool {
        self.floating.get(index).is_some_and(|f| f.tree.needs_frame())
    }

    fn render_floating(&mut self, engine: &TextEngine, index: usize, size: Size) -> Scene {
        let Some(f) = self.floating.get_mut(index) else {
            return Scene::default();
        };
        let dt = std::mem::take(&mut f.pending_dt);
        let out = f.tree.frame(engine, size, dt);
        f.damage = out.damage;
        out.scene
    }

    fn floating_damage(&mut self, index: usize) -> Option<Rect> {
        self.floating.get(index).and_then(|f| f.damage)
    }

    fn floating_full_repaint(&mut self, index: usize) {
        if let Some(f) = self.floating.get_mut(index) {
            f.tree.request_full_repaint();
        }
    }

    fn floating_input(&mut self, index: usize, event: InputEvent) {
        if let Some(f) = self.floating.get_mut(index) {
            // 浮层没有加速键：未处理的按键丢弃（加速键属于主表面 / 应用级）。
            let _ = feed(&mut f.tree, &mut f.pointer, event);
        }
        self.drain_actions();
    }

    fn enable_popups(&mut self, bounds: Rect) -> bool {
        self.tree.set_popup_bounds(Some(bounds));
        self.tree.set_detached_popups(true);
        true
    }

    fn popups(&self) -> Vec<PopupRequest> {
        if !self.tree.detached_popups() {
            return Vec::new();
        }
        self.tree
            .popups()
            .filter_map(|id| {
                let rect = self.tree.popup_extent(id)?;
                if rect.size.width < 1.0 || rect.size.height < 1.0 {
                    return None; // 尚未排版（打开当帧）
                }
                let spec = self.tree.popup_spec(id)?;
                Some(PopupRequest {
                    key: id.to_u64(),
                    rect,
                    // 模态弹层不抓取：点外部不能关闭它（合成器抓取语义就是「点外部 popup_done」）。
                    grab: spec.light_dismiss && !spec.modal,
                })
            })
            .collect()
    }

    fn popup_needs_redraw(&self, key: u64) -> bool {
        self.popup_dirty.contains(&key)
    }

    fn render_popup(&mut self, _engine: &TextEngine, key: u64, _size: Size) -> Scene {
        self.popup_dirty.remove(&key);
        self.popup_id(key)
            .and_then(|id| self.tree.popup_scene(id))
            .unwrap_or_default()
    }

    fn popup_input(&mut self, key: u64, event: InputEvent) {
        if let Some(ev) = self.popup_to_tree(key, event) {
            self.handle_input(ev);
        }
    }

    fn popup_dismissed(&mut self, key: u64) {
        if let Some(id) = self.popup_id(key) {
            self.tree.close_popup(id);
            self.drain_actions();
        }
    }

    fn update(&mut self, dt: f64) {
        self.pending_dt += dt;
        // 定时器在 update 里推进：外壳空闲时也有兜底唤醒（~100ms），到期者置脏出帧。
        self.tree.tick_timers(dt);
        self.app.tick(&mut self.tree, dt);
        for (i, f) in self.floating.iter_mut().enumerate() {
            f.pending_dt += dt;
            f.tree.tick_timers(dt);
            self.app.tick_floating(i, &mut f.tree, dt);
        }
        self.drain_actions();
    }

    fn needs_redraw(&self) -> bool {
        self.tree.needs_frame()
    }

    /// 主表面与各浮层树中最近的定时器（元素树 `next_timer`）—— 空闲唤醒用。
    fn next_wake_hint(&self) -> Option<f64> {
        let mut next = self.tree.next_timer();
        for f in &self.floating {
            if let Some(t) = f.tree.next_timer() {
                next = Some(next.map_or(t, |c| c.min(t)));
            }
        }
        next
    }

    fn focus_move(&mut self, backward: bool) -> bool {
        log::debug!("focus_move backward={backward} focus_before={:?}", self.tree.focused());
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

    fn set_damage_cull(&mut self, on: bool) {
        self.tree.set_damage_cull(on);
    }

    fn focus_changed(&mut self, focused: bool) {
        if !focused {
            self.tree.dismiss_popups();
            for f in &mut self.floating {
                f.tree.dismiss_popups();
            }
            self.drain_actions();
        }
    }

    fn should_close(&self) -> bool {
        self.app.should_close()
    }

    fn handle_input(&mut self, event: InputEvent) {
        // 输入轨迹（RUST_LOG=kanesumi_harness::tree_host=debug）：排查「谁动了焦点 / 谁改了文本」。
        if !matches!(event, InputEvent::PointerMoved { .. }) {
            log::debug!("input {event:?} focus_before={:?}", self.tree.focused());
        }
        if let Some((key, modifiers)) = feed(&mut self.tree, &mut self.pointer, event) {
            self.app.on_key(&mut self.tree, key, modifiers);
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
        // 弹层分离：本帧损伤（None = 全量）碰到的弹层要重画其表面。
        if self.tree.detached_popups() {
            for id in self.tree.popups().collect::<Vec<_>>() {
                let hit = match (frame.damage, self.tree.popup_extent(id)) {
                    (None, _) => true,
                    (Some(d), Some(r)) => d.intersect(r).is_some(),
                    (Some(_), None) => false,
                };
                if hit {
                    self.popup_dirty.insert(id.to_u64());
                }
            }
        }
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
        accelerators: u32,
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
        fn on_key(&mut self, _tree: &mut Tree, key: Key, _m: Modifiers) -> bool {
            if key == Key::Char('q') {
                self.accelerators += 1;
                return true;
            }
            false
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
            accelerators: 0,
        });
        (host, test_engine())
    }

    fn frame(h: &mut TreeHost<Demo>, e: &TextEngine) -> Scene {
        h.update(1.0 / 60.0);
        let mut s = Scene::default();
        h.render_into(e, Size::new(400.0, 300.0), &mut s);
        s
    }

    /// 带一个浮层的应用：浮层里一个按钮，点它 → 主表面状态标签改写（跨树动作）。
    struct WithOverlay {
        config: AppConfig,
        status: Option<WidgetId>,
        overlay_button: Option<WidgetId>,
        open: bool,
    }

    impl TreeApp for WithOverlay {
        fn config(&self) -> &AppConfig {
            &self.config
        }
        fn build(&mut self, tree: &mut Tree) {
            self.status = Some(tree.insert(tree.root(), kanesumi_element::widgets::Label::new("idle")));
        }
        fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
        fn floating_layers(&self) -> Vec<FloatingLayer> {
            vec![FloatingLayer::new(
                "test-overlay",
                crate::app::LayerKind::Overlay,
                crate::app::AnchorKind::Fullscreen,
                0.0,
                0.0,
            )]
        }
        fn build_floating(&mut self, _i: usize, tree: &mut Tree) {
            let start = LayoutProps { h_align: Align::Start, v_align: Align::Start, ..LayoutProps::default() };
            self.overlay_button = Some(tree.insert_with(tree.root(), MetroButton::new("关闭"), start));
        }
        fn floating_visible(&self, _i: usize) -> bool {
            self.open
        }
        fn on_floating_action(&mut self, _i: usize, _tree: &mut Tree, main: &mut Tree, from: WidgetId, action: Action) {
            if Some(from) == self.overlay_button && action.downcast_ref::<ButtonClicked>().is_some() {
                self.open = false;
                if let Some(id) = self.status {
                    main.edit::<kanesumi_element::widgets::Label, _>(id, |l, _| l.text = "closed".into());
                }
            }
        }
    }

    #[test]
    fn floating_surface_has_its_own_tree_and_cross_tree_actions() {
        let mut h = TreeHost::new(WithOverlay {
            config: AppConfig::new("org.ether.test", "t", EtherRole::Browser, 400.0, 300.0),
            status: None,
            overlay_button: None,
            open: true,
        });
        let e = test_engine();
        assert_eq!(h.floating_layers().len(), 1);
        assert!(h.floating_visible(0));
        h.update(1.0 / 60.0);
        let _ = h.render_floating(&e, 0, Size::new(800.0, 600.0));
        let b = h.app().overlay_button.unwrap();
        let c = h.floating_tree(0).unwrap().rect(b).unwrap().center();
        // 浮层坐标系里的点击只进浮层树。
        h.floating_input(0, InputEvent::PointerMoved { x: c.x, y: c.y });
        h.floating_input(0, InputEvent::PointerPressed { x: c.x, y: c.y, button: PointerButton::Left, modifiers: Modifiers::NONE });
        h.floating_input(0, InputEvent::PointerReleased { x: c.x, y: c.y, button: PointerButton::Left, modifiers: Modifiers::NONE });
        assert!(!h.floating_visible(0), "浮层动作关掉了浮层");
        let status = h.app().status.unwrap();
        assert_eq!(h.tree().get::<kanesumi_element::widgets::Label>(status).unwrap().text, "closed", "跨树改主表面");
        // 主题推送到所有树。
        let light = MetroTheme::light(kanesumi_core::Accent::default());
        h.set_theme(light);
        assert_eq!(*h.floating_tree(0).unwrap().theme(), light);
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
    fn unhandled_keys_reach_app_accelerators_but_focused_text_box_wins() {
        let (mut h, e) = host();
        frame(&mut h, &e);
        let q = InputEvent::KeyPressed {
            key: Key::Char('q'),
            modifiers: Modifiers::NONE,
        };
        h.handle_input(q.clone()); // 无焦点 → 应用级快捷键
        assert_eq!(h.app().accelerators, 1);
        h.focus_move(false); // 按钮
        h.focus_move(false); // 输入框
        frame(&mut h, &e);
        h.handle_input(q);
        assert_eq!(h.app().accelerators, 1, "输入框消费了 q，不再触发快捷键");
        assert_eq!(h.app().last_text, "q");
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
