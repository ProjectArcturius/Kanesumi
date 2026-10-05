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

    /// 合成图层（G3-b）：外壳每帧取走图层命令。参 `crate::layers`。
    fn take_layer_commands(&mut self) -> Vec<crate::layers::LayerCommand> {
        Vec::new()
    }

    /// 合成器回报图层动画完成 / 打断（或不支持合成图层）。`tree` 为该图层所在的树
    /// （主表面或浮层），`widget` = 树图层的节点（`Tree::set_layer` 标记的那个）；App 自建图层为 None。
    fn on_layer_event(
        &mut self,
        _tree: &mut Tree,
        _widget: Option<WidgetId>,
        _event: crate::layers::LayerEvent,
    ) {
    }

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
    /// 真实经过时间的定时器累计（`advance_clock` 加、`update` 取；墙钟语义）。
    timer_dt: f64,
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
    /// 真实经过时间的定时器累计（`advance_clock` 加、`update` 取）。
    /// 定时器是墙钟语义（「多久之后」），挂起恢复后的长间隔按真实时长扣；
    /// 与动画的 `pending_dt` 分开记，方便 `update` 里只取走定时器份额。
    timer_dt: f64,
    /// 上一帧的损伤（外壳在 render 之后经 `damage_hint` 取走）。
    damage: Option<Rect>,
    /// 最近指针位置（滚轮事件不带坐标，命中要用）。
    pointer: Point,
    /// 弹层分离模式下需重画的弹层（key = `WidgetId::to_u64`）。
    popup_dirty: std::collections::HashSet<u64>,
    /// 浮层表面的树（与 `TreeApp::floating_layers` 一一对应）。
    floating: Vec<FloatingTree>,
    /// 元素树图层（G3-c）↔ 外壳图层 id。key = (表面序号：0 主表面 / i+1 第 i 浮层, 节点)。
    layer_ids: std::collections::HashMap<(usize, WidgetId), crate::layers::LayerId>,
    layer_rev: std::collections::HashMap<crate::layers::LayerId, (usize, WidgetId)>,
    next_layer: u32,
}

/// 树图层的外壳 id 从高位段分配，避免与 App 自建图层（`TreeApp::take_layer_commands`）冲突。
const TREE_LAYER_BASE: u32 = 0x8000_0000;

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
                FloatingTree { tree: t, pending_dt: 0.0, timer_dt: 0.0, pointer: Point::new(0.0, 0.0), damage: None }
            })
            .collect();
        Self {
            app,
            tree,
            pending_dt: 0.0,
            timer_dt: 0.0,
            damage: None,
            pointer: Point::new(0.0, 0.0),
            popup_dirty: std::collections::HashSet::new(),
            floating,
            layer_ids: std::collections::HashMap::new(),
            layer_rev: std::collections::HashMap::new(),
            next_layer: TREE_LAYER_BASE,
        }
    }

    /// 取走各棵树的图层操作，翻译成外壳图层命令（G3-c）。参 kanesumi-element layer.rs。
    fn drain_tree_layers(&mut self, out: &mut Vec<crate::layers::LayerCommand>) {
        use crate::layers::{LayerAnim, LayerCommand, LayerParent};
        use kanesumi_element::LayerOp;
        let mut batches = vec![(0usize, self.tree.take_layer_ops())];
        for (i, f) in self.floating.iter_mut().enumerate() {
            batches.push((i + 1, f.tree.take_layer_ops()));
        }
        for (surf, ops) in batches {
            let parent = if surf == 0 { LayerParent::Main } else { LayerParent::Floating(surf - 1) };
            for op in ops {
                match op {
                    LayerOp::Create { id, rect } => {
                        let lid = crate::layers::LayerId(self.next_layer);
                        self.next_layer = self.next_layer.wrapping_add(1).max(TREE_LAYER_BASE);
                        self.layer_ids.insert((surf, id), lid);
                        self.layer_rev.insert(lid, (surf, id));
                        out.push(LayerCommand::Create { id: lid, parent, rect });
                    }
                    LayerOp::Remove { id } => {
                        if let Some(lid) = self.layer_ids.remove(&(surf, id)) {
                            self.layer_rev.remove(&lid);
                            out.push(LayerCommand::Destroy { id: lid });
                        }
                    }
                    other => {
                        let wid = match &other {
                            LayerOp::Content { id, .. }
                            | LayerOp::Rect { id, .. }
                            | LayerOp::Offset { id, .. }
                            | LayerOp::Opacity { id, .. }
                            | LayerOp::Animate { id, .. } => *id,
                            LayerOp::Create { .. } | LayerOp::Remove { .. } => unreachable!(),
                        };
                        let Some(&lid) = self.layer_ids.get(&(surf, wid)) else { continue };
                        out.push(match other {
                            LayerOp::Content { scene, .. } => LayerCommand::SetContent { id: lid, scene },
                            LayerOp::Rect { rect, .. } => LayerCommand::SetRect { id: lid, rect },
                            LayerOp::Offset { x, y, .. } => LayerCommand::SetOffset { id: lid, x, y },
                            LayerOp::Opacity { opacity, .. } => LayerCommand::SetOpacity { id: lid, opacity },
                            LayerOp::Animate { spec, .. } => LayerCommand::Animate {
                                id: lid,
                                anim: LayerAnim {
                                    serial: spec.serial,
                                    duration_ms: spec.duration_ms,
                                    delay_ms: spec.delay_ms,
                                    curve: spec.curve,
                                    to_x: spec.to_x,
                                    to_y: spec.to_y,
                                    to_opacity: spec.to_opacity,
                                },
                            },
                            LayerOp::Create { .. } | LayerOp::Remove { .. } => unreachable!(),
                        });
                    }
                }
            }
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
        f.damage = frame_damage(&out);
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

    /// 真实（未限幅）经过时间 → 各树的动画时钟。`frame` 据此推进动画，掉帧不拉长总时长。
    /// 同时累计定时器份额：定时器是墙钟语义（STATE 2026-10-02 §Ⅲ #8——
    /// 「1.5 s 后隐藏」在空闲大间隔唤醒下不再被限幅拉长），到期的定时器由
    /// `tick_timers` 的 `retain_mut` 语义保证只触发一次。
    fn advance_clock(&mut self, dt: f64) {
        self.pending_dt += dt;
        self.timer_dt += dt;
        for f in &mut self.floating {
            f.pending_dt += dt;
            f.timer_dt += dt;
        }
    }

    fn update(&mut self, dt: f64) {
        crate::timeline::note_once("app_update_start");
        // 定时器（含工具提示计时）按真实经过时间推进，取走累计份额；挂起恢复的
        // 大步长在这里是期望行为（一次结算），不会重复触发。
        let timers = std::mem::take(&mut self.timer_dt);
        self.tree.tick_timers(timers);
        // 每步积分类逻辑（App tick / 控件 hover 过渡）仍用限幅 dt 防跳变。
        let logic = dt.min(0.05);
        self.app.tick(&mut self.tree, logic);
        for (i, f) in self.floating.iter_mut().enumerate() {
            let ft = std::mem::take(&mut f.timer_dt);
            f.tree.tick_timers(ft);
            self.app.tick_floating(i, &mut f.tree, logic);
        }
        self.drain_actions();
        crate::timeline::note_once("app_update_done");
    }

    fn needs_redraw(&self) -> bool {
        self.tree.needs_frame()
    }

    fn take_layer_commands(&mut self) -> Vec<crate::layers::LayerCommand> {
        let mut out = self.app.take_layer_commands();
        self.drain_tree_layers(&mut out);
        out
    }

    fn on_layer_event(&mut self, event: crate::layers::LayerEvent) {
        use crate::layers::LayerEvent;
        // 树图层的事件交给所在那棵树、并带上节点 id；App 自建图层的事件 widget = None。
        let owner = match event {
            LayerEvent::Done { id, .. } | LayerEvent::Cancelled { id, .. } => self.layer_rev.get(&id).copied(),
            LayerEvent::Unsupported => None,
        };
        match owner {
            Some((0, wid)) => self.app.on_layer_event(&mut self.tree, Some(wid), event),
            Some((s, wid)) => match self.floating.get_mut(s - 1) {
                Some(f) => self.app.on_layer_event(&mut f.tree, Some(wid), event),
                None => {}
            },
            None => self.app.on_layer_event(&mut self.tree, None, event),
        }
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
        crate::timeline::note_once("render_into_start");
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
        // 外壳契约：None = 全量；零面积 = 本帧没变（外壳跳过光栅与提交）。多帧未被取走时并入（保守）。
        self.damage = match (self.damage.take(), frame_damage(&frame)) {
            (_, None) => None,
            (None, Some(d)) => Some(d),
            (Some(a), Some(b)) => Some(union(a, b)),
        };
        *out = frame.scene;
    }
}

/// 元素树帧 → 外壳损伤：整幅 → `None`；没变 → 零面积矩形（外壳据此跳过光栅与提交）；局部 → 该矩形。
/// 元素树的 `damage == None` 同时表示「整幅」与「没变」，必须用 `full` 区分，否则「没变」被当整幅光栅
///（Launcher 浮层每次指针移动整屏重光栅 40 ms，2026-10-01 实测）。
fn frame_damage(out: &kanesumi_element::FrameOutput) -> Option<Rect> {
    if out.full {
        None
    } else {
        Some(out.damage.unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)))
    }
}

fn union(a: Rect, b: Rect) -> Rect {
    // 零面积 = 「没变」，是并集的单位元（否则原点处的空矩形会把包围盒拉到左上角）。
    if a.size.width <= 0.0 || a.size.height <= 0.0 {
        return b;
    }
    if b.size.width <= 0.0 || b.size.height <= 0.0 {
        return a;
    }
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
    use kanesumi_element::{
        Align, LayoutProps, MeasureCtx, PaintCtx, PointerButton, UpdateCtx, Widget,
    };

    /// 用统一入口驱动的动画控件：`update` 推进，未稳态自动续帧；`paint` 仅在启动帧登记一次。
    struct TreeAnim {
        anim: kanesumi_anim::Progress,
        kicked: bool,
    }

    impl Widget for TreeAnim {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(40.0, 20.0)
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut kanesumi_canvas::Scene) {
            if !self.kicked && !self.anim.is_steady() {
                self.kicked = true;
                ctx.request_anim_frame();
            }
            scene.fill_rect(
                kanesumi_core::Color::rgb(self.anim.value() as f32, 0.0, 0.0),
                ctx.rect(),
            );
        }
        fn update(&mut self, ctx: &mut UpdateCtx, _dt: f64) {
            if self.anim.is_steady() {
                return;
            }
            ctx.animate(&mut self.anim);
            ctx.invalidate_paint();
        }
    }

    /// 只有一个动画控件的 App。
    struct AnimApp {
        config: AppConfig,
        id: Option<WidgetId>,
    }

    impl TreeApp for AnimApp {
        fn config(&self) -> &AppConfig {
            &self.config
        }
        fn build(&mut self, tree: &mut Tree) {
            let mut anim = kanesumi_anim::Progress::new(0.1);
            anim.set_target(1.0);
            self.id = Some(tree.insert(tree.root(), TreeAnim { anim, kicked: false }));
        }
        fn on_action(&mut self, _: &mut Tree, _: WidgetId, _: Action) {}
    }

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

    fn frame<A: TreeApp>(h: &mut TreeHost<A>, e: &TextEngine) -> Scene {
        h.advance_clock(1.0 / 60.0);
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

    /// 工具提示：悬停满延迟后由框架开 passthrough 弹层，并经 `render_png` 真正光栅进像素。
    /// 复刻 `snapshot::render_png` 的逐帧累积路径（含「0 面积 damage 跳过」守卫）。
    #[test]
    fn tooltip_popup_is_composed_and_rasterized() {
        struct TipApp {
            config: AppConfig,
            id: Option<WidgetId>,
        }
        impl TreeApp for TipApp {
            fn config(&self) -> &AppConfig {
                &self.config
            }
            fn build(&mut self, tree: &mut Tree) {
                let page = tree.insert(
                    tree.root(),
                    kanesumi_element::widgets::Border::new()
                        .background(kanesumi_core::ThemeColor::Background)
                        .padding(kanesumi_element::Insets::all(24.0)),
                );
                let col = tree.insert(
                    page,
                    kanesumi_element::widgets::Stack::column().with_spacing(12.0),
                );
                tree.insert(col, kanesumi_element::widgets::Label::new("工具提示"));
                let btn = tree.insert(col, MetroButton::new("应用"));
                tree.set_tooltip(
                    btn,
                    "应用设置并打开确认对话框：这段文字较长，用来演示最大宽度 320 与自动换行。",
                );
                self.id = Some(btn);
            }
            fn on_action(&mut self, _: &mut Tree, _: WidgetId, _: Action) {}
        }
        let mut h = TreeHost::new(TipApp {
            config: AppConfig::new("org.ether.test", "tip", EtherRole::Browser, 360.0, 200.0),
            id: None,
        });
        let e = test_engine();
        let size = Size::new(360.0, 200.0);
        {
            let mut scene = Scene::default();
            h.advance_clock(1.0 / 60.0);
            h.update(1.0 / 60.0);
            h.render_into(&e, size, &mut scene);
        }
        let c = h.tree().rect(h.app().id.unwrap()).unwrap().center();
        h.handle_input(InputEvent::PointerMoved { x: c.x, y: c.y });
        let out = std::env::temp_dir().join("kanesumi_tooltip_raster.png");
        crate::snapshot::render_png(&mut h, &e, size, 1.0, 60, &out).expect("快照失败");
        let popup = h.tree().tooltip_popup().expect("延迟满应显示提示");
        let r = h.tree().rect(popup).unwrap();
        let pixmap = resvg::tiny_skia::Pixmap::decode_png(&std::fs::read(&out).unwrap()).unwrap();
        let p = pixmap
            .pixel(r.center().x.floor() as u32, r.center().y.floor() as u32)
            .expect("像素在界内");
        assert_ne!(
            [p.red(), p.green(), p.blue()],
            [0x1A, 0x1A, 0x1A],
            "提示气泡区不应还是背景色（说明未光栅化）"
        );
    }

    /// 动画期间 `needs_redraw()` 恒真（外壳据此不进入 50ms 空闲档），跑完转假。
    #[test]
    fn animation_keeps_host_busy_until_complete() {
        let mut h = TreeHost::new(AnimApp {
            config: AppConfig::new("org.ether.test", "anim", EtherRole::Browser, 200.0, 100.0),
            id: None,
        });
        let e = test_engine();
        frame(&mut h, &e);
        assert!(h.needs_redraw(), "动画启动后外壳应保持 busy");
        for _ in 0..600 {
            if !h.needs_redraw() {
                break;
            }
            frame(&mut h, &e);
        }
        assert!(!h.needs_redraw(), "动画完成后外壳转空闲");
    }
}
