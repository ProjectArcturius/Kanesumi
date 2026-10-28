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
use kanesumi_canvas::scene::SceneCommand;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};
use kanesumi_element::{Action, Key, Modifiers, Tree, WidgetId};

use crate::app::{App, AppConfig, FloatingLayer, ImeContext, InputEvent, LockEvent, PopupRequest};
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

    /// 系统背板参数变更；应用按需更新既有节点，不必重建整棵树。
    fn on_backdrop(&mut self, _backdrop: crate::system_theme::Backdrop) {}

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

    // ── 输入预拦截（阶段机）。参 K4「应用模型统一」（archive/k-app-model.md）──────

    /// 树之前的输入总入口：返回 `true` = 事件不再投给树（App 已消费）。
    ///
    /// 供「阶段机」应用在前端拦事件（如锁屏第一层：任意键 / 点击 / 滚轮 → 进入第二层，
    /// 事件不再进树）。默认 `false` —— 事件照常投给树；未处理的按键再走 [`TreeApp::on_key`]。
    fn on_input(&mut self, _tree: &mut Tree, _event: &InputEvent) -> bool {
        false
    }

    // ── IME 引擎宿主（zwp_input_method_v2）。参 CEYBOARD_SPEC §Ⅴ ─────────────

    /// 是否作为 IME 引擎宿主。`true` 时外壳绑定 `input-method-v2` / 虚拟键盘，
    /// 并把按键经 [`TreeApp::ime_engine_key`] 投给本应用。
    ///
    /// 宿主应用的界面画在**候选窗 popup 表面**（[`TreeApp::ime_engine_popup_size`] 定尺寸，
    /// 内容 = 本应用的树整幅出帧）；主表面恒透明（`TreeHost` 不为它出帧）。
    fn ime_engine_host(&self) -> bool {
        false
    }

    /// 引擎按键（按下）。返回 `true` = 引擎消费（不重放给焦点应用）。
    fn ime_engine_key(&mut self, _tree: &mut Tree, _key: Key, _modifiers: Modifiers) -> bool {
        false
    }

    /// 引擎按键（松开）。默认空实现。
    fn ime_engine_key_release(&mut self, _tree: &mut Tree, _key: Key, _modifiers: Modifiers) -> bool {
        false
    }

    /// 当前组合态（preedit, 光标字节偏移）。空串 = 无组合态。
    fn ime_engine_preedit(&self) -> (String, Option<usize>) {
        (String::new(), None)
    }

    /// 取走待上屏文本（选词 / 空格 / 回车后引擎产出的 commit）。
    fn ime_engine_take_commit(&mut self) -> Option<String> {
        None
    }

    /// 取走待删除的周边字节数 `(before, after)`。
    fn ime_engine_take_delete(&mut self) -> (u32, u32) {
        (0, 0)
    }

    /// 候选窗 popup 表面逻辑尺寸（0×0 = 不显示）。
    fn ime_engine_popup_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// 下一次需要唤醒的时刻（秒，自现在起）—— 主循环据此安排事件循环超时。
    ///
    /// 与旧 [`App::next_wake_hint`] 同义，供「无控件定时器但要按墙钟推进状态」的应用
    /// （如 Ceyboard 的中英切换提示到点隐藏）。`TreeHost` 取它与树中最近定时器的较小者。
    fn next_wake_hint(&self) -> Option<f64> {
        None
    }

    // ── 锁屏（session_lock 外壳）。参 docs/LOCKSCREEN_DESIGN.md §Ⅳ ──────────

    /// 锁屏表面出帧前的输出分派：`primary=false` 的副输出只画背板，App 应隐藏
    /// 只属于主输出的界面节点（多输出各一个锁屏表面的分派逻辑保留）。
    fn lock_output(&mut self, _tree: &mut Tree, _size: Size, _primary: bool) {}

    /// 锁屏表面树帧后的装配钩子（图层宿主用）。
    ///
    /// `scene` = 本输出的树整幅帧；`full` / `damage` = 该帧的损伤语义。默认原样返回。
    /// 需要「主表面与图层内容分离装配」的锁屏应用（`ether-lock`：幕布 / 时钟 / 认证
    /// 内容走合成器图层，主表面只留背板）在此改写 `scene` 并填充自己的图层队列 ——
    /// 单一树帧表达不了「主表面 ≠ 树全幅」的装配，这是 K4 迁移保留 10-17 图层时序的
    /// 最小接口（参 tools/dispatch/tasks/n1028/README.md §三-1）。
    fn lock_compose(
        &mut self,
        _tree: &mut Tree,
        _size: Size,
        _primary: bool,
        _full: bool,
        _damage: Option<Rect>,
        _scene: &mut Scene,
    ) {
    }

    /// 锁屏外壳事件（锁确认 / 拒绝 / 输出增删）。
    fn on_lock_event(&mut self, _tree: &mut Tree, _event: crate::app::LockEvent) {}

    /// 应用请求解锁（认证成功）。**安全不变量：认证成功前恒 `false`**。
    fn lock_unlock_requested(&self) -> bool {
        false
    }

    /// 大写锁定状态变化（锁屏状态行提示用）。
    fn lock_caps_lock_changed(&mut self, _tree: &mut Tree, _caps: bool) {}
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
        InputEvent::ScrollInput(input) => {
            t.scroll_ex(*pointer, input);
        }
        InputEvent::KeyPressed { key, modifiers, repeat } => {
            if !t.key_down_ex(key, modifiers, repeat) {
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
    /// 上一帧是否整幅（元素树 `full=true`）。`damage == None` 同时表示「整幅」与「尚未出帧」，
    /// 桥接层（如锁屏外壳）要把树损伤转给旧 `App::damage_hint` 时必须能区分这两者。
    damage_full: bool,
    /// 最近指针位置（滚轮事件不带坐标，命中要用）。
    pointer: Point,
    /// 弹层分离模式下需重画的弹层（key = `WidgetId::to_u64`）。
    popup_dirty: std::collections::HashSet<u64>,
    /// 各弹层上一帧的 Scene —— 逐条比对得出本帧真正变了的区域（参 `scene_damage`）。
    popup_scene: std::collections::HashMap<u64, Scene>,
    /// 各弹层表面本帧的损伤（key 在场 = 已算出；值 `None` = 整幅，`Some` = 局部）。
    /// `render_popup` 写入，外壳随后经 `popup_damage` 取走。
    popup_damage: std::collections::HashMap<u64, Option<Rect>>,
    /// 浮层表面的树（与 `TreeApp::floating_layers` 一一对应）。
    floating: Vec<FloatingTree>,
    /// 元素树图层（G3-c）↔ 外壳图层 id。key = (表面序号：0 主表面 / i+1 第 i 浮层, 节点)。
    layer_ids: std::collections::HashMap<(usize, WidgetId), crate::layers::LayerId>,
    layer_rev: std::collections::HashMap<crate::layers::LayerId, (usize, WidgetId)>,
    next_layer: u32,
    /// dk3-perf：最近一次 `update` 里主树 tick 的墙钟（毫秒）。
    last_tick_ms: f32,
    /// dk3-perf：最近一次 `update` 里各浮层 tick 的墙钟（毫秒，与 `floating` 同序）。
    last_floating_tick_ms: Vec<f32>,
}

/// 树图层的外壳 id 从高位段分配，避免与 App 自建图层（`TreeApp::take_layer_commands`）冲突。
const TREE_LAYER_BASE: u32 = 0x8000_0000;

impl<A: TreeApp> TreeHost<A> {
    pub fn new(mut app: A) -> Self {
        // 合成滚动总开关：KANESUMI_NO_SCROLLER=1 强制全部滚动容器走应用线程自绘
        //（回退验收 / 手感对照用；参 fp4b 任务书）。锁屏外壳等非树宿主不读——它们没有树。
        if std::env::var("KANESUMI_NO_SCROLLER").is_ok_and(|v| v == "1") {
            kanesumi_element::set_composed_scroll_enabled(false);
        }
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
            damage_full: true,
            pointer: Point::new(0.0, 0.0),
            popup_dirty: std::collections::HashSet::new(),
            popup_scene: std::collections::HashMap::new(),
            popup_damage: std::collections::HashMap::new(),
            floating,
            layer_ids: std::collections::HashMap::new(),
            layer_rev: std::collections::HashMap::new(),
            next_layer: TREE_LAYER_BASE,
            last_tick_ms: 0.0,
            last_floating_tick_ms: Vec::new(),
        }
    }

    /// 取走各棵树的图层操作，翻译成外壳图层命令（G3-c）。参 kanesumi-element layer.rs。
    fn drain_tree_layers(&mut self, out: &mut Vec<crate::layers::LayerCommand>) {
        use crate::layers::{LayerAnim, LayerClipAnim, LayerCommand, LayerInsets, LayerParent};
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
                    LayerOp::ScrollerCreate { id, rect } => {
                        let lid = crate::layers::LayerId(self.next_layer);
                        self.next_layer = self.next_layer.wrapping_add(1).max(TREE_LAYER_BASE);
                        self.layer_ids.insert((surf, id), lid);
                        self.layer_rev.insert(lid, (surf, id));
                        out.push(LayerCommand::CreateScroller {
                            id: lid,
                            parent,
                            viewport: rect,
                            // 首帧几何随建层一起发；树侧随后每帧按需发 SetScrollerGeometry。
                            extent: (rect.size.width, rect.size.height),
                            background: 0,
                        });
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
                            | LayerOp::Animate { id, .. }
                            | LayerOp::Clip { id, .. }
                            | LayerOp::AnimateClip { id, .. }
                            | LayerOp::ScrollerGeometry { id, .. }
                            | LayerOp::ScrollerRealize { id, .. }
                            | LayerOp::ScrollerScroll { id, .. } => *id,
                            LayerOp::Create { .. }
                            | LayerOp::Remove { .. }
                            | LayerOp::ScrollerCreate { .. } => unreachable!(),
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
                            LayerOp::Clip { insets: i, .. } => LayerCommand::SetClip {
                                id: lid,
                                insets: LayerInsets { top: i.top, right: i.right, bottom: i.bottom, left: i.left },
                            },
                            LayerOp::AnimateClip { spec, .. } => LayerCommand::AnimateClip {
                                id: lid,
                                anim: LayerClipAnim {
                                    serial: spec.serial,
                                    duration_ms: spec.duration_ms,
                                    delay_ms: spec.delay_ms,
                                    curve: spec.curve,
                                    to: LayerInsets {
                                        top: spec.to.top,
                                        right: spec.to.right,
                                        bottom: spec.to.bottom,
                                        left: spec.to.left,
                                    },
                                },
                            },
                            // ── 合成滚动（fp4b）──
                            LayerOp::ScrollerGeometry { viewport, extent, background, .. } => {
                                LayerCommand::SetScrollerGeometry {
                                    id: lid,
                                    viewport,
                                    extent: (extent.width, extent.height),
                                    background,
                                }
                            }
                            LayerOp::ScrollerRealize { realized, scene, .. } => {
                                LayerCommand::RealizeScroller { id: lid, realized, scene }
                            }
                            LayerOp::ScrollerScroll { serial, request, .. } => match request {
                                kanesumi_element::layer::ScrollerRequest::ScrollBy { delta } => {
                                    LayerCommand::ScrollerScrollBy { id: lid, serial, dy: delta }
                                }
                                kanesumi_element::layer::ScrollerRequest::ScrollTo { target, animate } => {
                                    LayerCommand::ScrollerScrollTo {
                                        id: lid,
                                        serial,
                                        y: target,
                                        animate,
                                    }
                                }
                            },
                            LayerOp::Create { .. }
                            | LayerOp::Remove { .. }
                            | LayerOp::ScrollerCreate { .. } => unreachable!(),
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

    /// 上一帧树帧是否整幅（`full=true`）。桥接层把树损伤转给旧 `App::damage_hint`
    /// 时，`damage == None` 的两种含义（整幅 / 尚未出帧）必须分开，否则会把整幅当局部。
    pub fn last_frame_full(&self) -> bool {
        self.damage_full
    }

    /// 给进行中的弹层表面动画记一帧（帧节拍日志）。参 `Tree::popup_motion`。
    ///
    /// 弹层动画由合成器按 vsync 推进，客户端**不提交弹层帧**：本函数只记「客户端在动画
    /// 窗口内自己出过的帧」，因此条数天然接近零 —— 呈现帧数的判据在合成器侧
    /// （`ether-frame-pacing.log` 的 `surface:NN` 段，vblank 对齐）。参 tools/perf/popup_motion.sh。
    fn note_popup_motion_frames(&self) {
        let now = std::time::Instant::now();
        for id in self.tree.popups().collect::<Vec<_>>() {
            let Some(plan) = self.tree.popup_motion(id) else {
                continue;
            };
            let Some(base) = self.tree.popup_motion_name(id) else {
                continue;
            };
            let phase = if plan.closing() { "退场" } else { "入场" };
            crate::perf::pacing_present_named(&format!("{base} {phase}"), now, None);
        }
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

    fn set_backdrop(&mut self, backdrop: crate::system_theme::Backdrop) {
        self.app.on_backdrop(backdrop);
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
        // 本帧产物即将提交：把进入提交的节点汇报给树（内容就绪门控就绪信号）。
        f.tree.report_content_committed(&out.painted);
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
                let plan = self.tree.popup_motion(id);
                Some(PopupRequest {
                    key: id.to_u64(),
                    rect,
                    // 模态弹层不抓取：点外部不能关闭它（合成器抓取语义就是「点外部 popup_done」）。
                    grab: spec.light_dismiss && !spec.modal,
                    // 退场中仍申报（表面要接着画），但外壳据此停投输入。
                    closing: plan.as_ref().is_some_and(|p| p.closing()),
                    enter: plan.as_ref().and_then(|p| p.enter),
                    exit: plan.as_ref().and_then(|p| p.exit),
                    motion_name: self
                        .tree
                        .popup_motion_name(id)
                        .unwrap_or_else(|| "弹层".to_string()),
                })
            })
            .collect()
    }

    fn popup_needs_redraw(&self, key: u64) -> bool {
        self.popup_dirty.contains(&key)
    }

    fn popup_damage(&mut self, key: u64) -> Option<Rect> {
        // 外壳在 `render_popup` **之后**调用（顺序见 platform/popups.rs）：损伤由本帧 Scene
        // 与上一帧的差异算出（参 `scene_damage`）。
        match self.popup_damage.remove(&key) {
            // `Some(None)` = 整幅；`Some(Some(d))` = 局部。
            Some(d) => d,
            // 没有记录 = 本帧没画这个弹层 → 零面积（外壳跳过光栅与提交）。
            None => Some(Rect::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    fn render_popup(&mut self, _engine: &TextEngine, key: u64, _size: Size) -> Scene {
        self.popup_dirty.remove(&key);
        let Some(id) = self.popup_id(key) else {
            return Scene::default();
        };
        let Some(scene) = self.tree.popup_scene(id) else {
            return Scene::default();
        };
        // 上一帧的 Scene 留着做下一帧的差异基准；clone 只复制命令与 `Arc`，不复制像素。
        let prev = self.popup_scene.insert(key, scene.clone());
        // 没有上一帧（首帧 / 表面重建）= 整幅；其余按差异。
        let damage = scene_damage(prev.as_ref(), &scene);
        self.popup_damage.insert(key, damage);
        scene
    }

    fn popup_input(&mut self, key: u64, event: InputEvent) {
        // 退场期间不接收输入：合成器抓取还没撤，指针事件仍会投进来 —— 一律丢弃，
        // 不能让用户「点到正在消失的面板」（参 FEEL_PLAN §Ⅰ-4）。
        if self
            .popup_id(key)
            .is_some_and(|id| self.tree.popup_closing(id))
        {
            return;
        }
        if let Some(ev) = self.popup_to_tree(key, event) {
            self.handle_input(ev);
        }
    }

    fn popup_dismissed(&mut self, key: u64) {
        log::debug!("合成器 popup_done：弹层 key={key}");
        if let Some(id) = self.popup_id(key) {
            // 已在退场 → `close_popup` 不重复发 `PopupClosed`（内部早退）。
            self.tree.close_popup(id);
            // 这条路径上表面已被合成器销毁（xdg-shell：popup_done 之后客户端必须销毁弹层），
            // 没有表面可放动画 → 立刻结算，否则会拖到兜底超时、期间 `popups()` 还会把它重建出来。
            self.tree.on_popup_motion_done(id);
            self.drain_actions();
        }
    }

    fn popup_motion_done(&mut self, key: u64) {
        log::debug!("弹层动画结束回调 key={key}");
        if let Some(id) = self.popup_id(key) {
            // 退场播完：此刻才真正摘掉元素树弹层，下一轮 `popups()` 比对销毁表面。
            self.tree.on_popup_motion_done(id);
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
        // dk3-perf：tick 是同一主线程上的工作，Dock 的下一帧会被它推迟；分表面记下来。
        let t_main = std::time::Instant::now();
        self.app.tick(&mut self.tree, logic);
        self.last_tick_ms = t_main.elapsed().as_secs_f32() * 1000.0;
        self.last_floating_tick_ms.clear();
        for (i, f) in self.floating.iter_mut().enumerate() {
            let ft = std::mem::take(&mut f.timer_dt);
            f.tree.tick_timers(ft);
            let t = std::time::Instant::now();
            self.app.tick_floating(i, &mut f.tree, logic);
            let ms = t.elapsed().as_secs_f32() * 1000.0;
            self.last_floating_tick_ms.push(ms);
        }
        self.drain_actions();
        crate::timeline::note_once("app_update_done");
    }

    /// dk3-perf：最近一次 update 的主树 tick 墙钟（毫秒）。
    fn last_tick_ms(&self) -> f32 {
        self.last_tick_ms
    }

    /// dk3-perf：最近一次 update 某浮层的 tick 墙钟（毫秒）。
    fn last_floating_tick_ms(&self, index: usize) -> f32 {
        self.last_floating_tick_ms.get(index).copied().unwrap_or(0.0)
    }

    fn needs_redraw(&self) -> bool {
        // 引擎宿主的主表面恒透明（界面在候选窗 popup 表面，由按键驱动 `im_popup_dirty`）：
        // 不为它请求帧 —— 否则外壳每迭代都会光栅 + 提交整幅全屏透明表面（60 fps 白烧）。
        if self.app.ime_engine_host() {
            return false;
        }
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
        let owner = match &event {
            LayerEvent::Done { id, .. }
            | LayerEvent::Cancelled { id, .. }
            | LayerEvent::ScrollerOffset { id, .. }
            | LayerEvent::ScrollerState { id, .. } => self.layer_rev.get(id).copied(),
            LayerEvent::Unsupported => None,
        };
        // 滚动器事件先驱动对应控件（更新偏移 / 相位），再交给 App（默认空实现）。
        if let Some((surf, wid)) = owner {
            let tree = match surf {
                0 => &mut self.tree,
                s => match self.floating.get_mut(s - 1) {
                    Some(f) => &mut f.tree,
                    None => return,
                },
            };
            match &event {
                LayerEvent::ScrollerOffset { x, y, present_ns, .. } => {
                    tree.deliver_scroller_offset(wid, *x, *y, *present_ns);
                }
                LayerEvent::ScrollerState { phase, serial, .. } => {
                    tree.deliver_scroller_state(wid, *phase, *serial);
                }
                _ => {}
            }
        }
        match owner {
            Some((0, wid)) => self.app.on_layer_event(&mut self.tree, Some(wid), event),
            Some((s, wid)) => {
                if let Some(f) = self.floating.get_mut(s - 1) {
                    self.app.on_layer_event(&mut f.tree, Some(wid), event);
                }
            }
            None => {
                // 「合成器不支持合成滚动」→ 撤下全部滚动器标记，控件回自绘（fp4b 回退路径）。
                if matches!(event, LayerEvent::Unsupported) {
                    self.tree.disable_all_scrollers();
                    for f in self.floating.iter_mut() {
                        f.tree.disable_all_scrollers();
                    }
                }
                self.app.on_layer_event(&mut self.tree, None, event);
            }
        }
    }

    /// 主表面与各浮层树中最近的定时器（元素树 `next_timer`）—— 空闲唤醒用。
    /// 与 `TreeApp::next_wake_hint`（无控件定时器的墙钟状态，如 Ceyboard 提示方块）取较小者。
    fn next_wake_hint(&self) -> Option<f64> {
        let mut next = self.tree.next_timer();
        for f in &self.floating {
            if let Some(t) = f.tree.next_timer() {
                next = Some(next.map_or(t, |c| c.min(t)));
            }
        }
        if let Some(t) = self.app.next_wake_hint() {
            next = Some(next.map_or(t, |c| c.min(t)));
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
        // 引擎宿主：主表面无内容 → 报零面积损伤，外壳据此跳过光栅与提交（省一整幅
        // 全屏透明光栅 + 一次无用提交）。参 `render_into` 同名分支。
        if self.app.ime_engine_host() {
            return Some(Rect::new(0.0, 0.0, 0.0, 0.0));
        }
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
        // 阶段机预拦截：消费的事件不再进树（锁屏第一层任意输入 → 进第二层）。
        if self.app.on_input(&mut self.tree, &event) {
            self.drain_actions();
            return;
        }
        if let Some((key, modifiers)) = feed(&mut self.tree, &mut self.pointer, event) {
            self.app.on_key(&mut self.tree, key, modifiers);
        }
        self.drain_actions();
    }

    // ── IME 引擎宿主（zwp_input_method_v2）：转发给 TreeApp。参 CEYBOARD_SPEC §Ⅴ ──

    fn ime_engine_host(&self) -> bool {
        self.app.ime_engine_host()
    }

    fn ime_engine_key(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let consumed = self.app.ime_engine_key(&mut self.tree, key, modifiers);
        self.drain_actions();
        consumed
    }

    fn ime_engine_key_release(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let consumed = self.app.ime_engine_key_release(&mut self.tree, key, modifiers);
        self.drain_actions();
        consumed
    }

    fn ime_engine_preedit(&self) -> (String, Option<usize>) {
        self.app.ime_engine_preedit()
    }

    fn ime_engine_take_commit(&mut self) -> Option<String> {
        self.app.ime_engine_take_commit()
    }

    fn ime_engine_take_delete(&mut self) -> (u32, u32) {
        self.app.ime_engine_take_delete()
    }

    fn ime_engine_popup_size(&self) -> (f32, f32) {
        self.app.ime_engine_popup_size()
    }

    /// 候选窗内容 = 本应用的树按 popup 逻辑尺寸整幅出帧。
    /// 整幅（`request_full_repaint`）：popup 表面每次都是新缓冲 / 无损伤历史，
    /// 局部帧会丢掉未变化节点（参 ELEMENT_TREE「compose 剔除」）。
    fn ime_engine_popup_scene(&mut self, engine: &TextEngine) -> Scene {
        let (w, h) = self.app.ime_engine_popup_size();
        if w <= 0.0 || h <= 0.0 {
            return Scene::default();
        }
        self.tree.request_full_repaint();
        let dt = std::mem::take(&mut self.pending_dt);
        let frame = self.tree.frame(engine, Size::new(w, h), dt);
        self.tree.report_content_committed(&frame.painted);
        frame.scene
    }

    // ── 锁屏（session_lock 外壳）：转发给 TreeApp。参 LOCKSCREEN_DESIGN §Ⅳ ──────

    /// 锁屏表面：先让 App 按输出分派（副输出隐藏主输出界面节点），树整幅出帧后再交
    /// [`TreeApp::lock_compose`] 装配（图层宿主在此改写主表面并填图层队列）。
    fn render_lock(&mut self, engine: &TextEngine, size: Size, primary: bool) -> Scene {
        self.app.lock_output(&mut self.tree, size, primary);
        // 每个输出各出一帧整幅：同一次渲染里 primary / 副输出交替，局部帧会互相剔除。
        self.tree.request_full_repaint();
        let dt = std::mem::take(&mut self.pending_dt);
        let frame = self.tree.frame(engine, size, dt);
        self.tree.report_content_committed(&frame.painted);
        // 锁屏外壳不消费局部损伤（每输出整幅提交），损伤只随帧交给装配钩子记日志。
        self.damage = None;
        let mut scene = frame.scene;
        self.app
            .lock_compose(&mut self.tree, size, primary, frame.full, frame.damage, &mut scene);
        scene
    }

    fn on_lock_event(&mut self, event: LockEvent) {
        self.app.on_lock_event(&mut self.tree, event);
        self.drain_actions();
    }

    fn lock_unlock_requested(&self) -> bool {
        self.app.lock_unlock_requested()
    }

    fn lock_caps_lock_changed(&mut self, caps: bool) {
        self.app.lock_caps_lock_changed(&mut self.tree, caps);
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

    fn on_decode_ready(&mut self, keys: &[kanesumi_canvas::decode::DecodeKey]) {
        self.tree.on_decode_ready(keys);
        for f in &mut self.floating {
            f.tree.on_decode_ready(keys);
        }
    }

    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene {
        let mut out = Scene::default();
        self.render_into(engine, size, &mut out);
        out
    }

    fn render_into(&mut self, engine: &TextEngine, size: Size, out: &mut Scene) {
        // 引擎宿主：界面在候选窗 popup 表面（`ime_engine_popup_scene`），主表面恒透明。
        if self.app.ime_engine_host() {
            out.commands.clear();
            return;
        }
        crate::timeline::note_once("render_into_start");
        // 弹层表面动画的帧节拍：动画由合成器按 vsync 推进，客户端**不提交弹层帧**，
        // 所以只能按「主表面真实出帧」记（外壳每渲染一帧调一次本函数，渲染又受帧节流
        // 约束）。段名与 `platform/popups.rs` 的起 / 收一致，同一进程同一线程共享计数器。
        self.note_popup_motion_frames();
        let dt = std::mem::take(&mut self.pending_dt);
        let frame = self.tree.frame(engine, size, dt);
        self.damage_full = frame.full;
        // 本帧产物即将由外壳提交：汇报进入提交的节点（内容就绪门控就绪信号）。
        // 平台提交点紧随本调用（platform.rs 主循环），故就绪与解挂同帧生效。
        self.tree.report_content_committed(&frame.painted);
        // 弹层分离：判定本帧哪些弹层**参与**重画；画什么由 `render_popup` 的 Scene 差异定
        //（参 `scene_damage`）。弹层节点与主表面同树，但画在**自己的表面**上：主表面为什么
        // 重画，不代表弹层内容变了。旧实现把 `frame.damage == None` 当整幅（它同时表示
        // 「什么都没变」），且任意与弹层矩形相交的主表面损伤都整块重画弹层 —— 面板开着不动
        // 时每帧整幅光栅 + 整块上传（真机 `surface=popup` 的 `dmg_pct` 恒 100.00，2026-10-23）。
        // 参 docs/research/tb_card_perf/README.md §根因。
        if self.tree.detached_popups() {
            let ids = self.tree.popups().collect::<Vec<_>>();
            // 已关弹层的残留记账清掉（key = WidgetId 的 index+generation，可被复用）。
            let live = |k: &u64| ids.iter().any(|id| id.to_u64() == *k);
            self.popup_damage.retain(|k, _| live(k));
            self.popup_scene.retain(|k, _| live(k));
            for id in ids {
                let Some(extent) = self.tree.popup_extent(id) else {
                    continue;
                };
                let key = id.to_u64();
                if popup_damage_hits(frame.full, frame.damage, extent) {
                    self.popup_dirty.insert(key);
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

/// 本帧主表面损伤是否碰到该弹层 —— 只决定「参与不参与判定」。
///
/// 两个易错点：
/// 1. `damage == None` 且 `full == false` 表示「本帧没变」，**不是**整幅（旧实现按整幅处理，
///    面板开着不动时每帧重画弹层）；
/// 2. 与弹层**相交不到**的主表面损伤（例如条上状态图标的重画）不得唤醒弹层。
fn popup_damage_hits(full: bool, damage: Option<Rect>, extent: Rect) -> bool {
    if full {
        return true;
    }
    match damage {
        Some(d) if d.size.width > 0.0 && d.size.height > 0.0 => d.intersect(extent).is_some(),
        _ => false,
    }
}

/// 两帧弹层 Scene 的差异 → 本帧要画的局部损伤（`None` = 整幅）。
///
/// 为什么要在外壳比对 Scene：弹层根通常是一个**整块自绘**的自定义控件（网络卡 / 音量卡…），
/// 元素树对它的重画只能记「整块绘制范围」→ 每帧整块光栅 + 整块上传（真机 `dmg_pct=100`）。
/// Scene 是最终绘制指令，比对「哪些命令变了」就能得出真正变了的区域，且对任何 App 成立，
/// 不要求控件自己维护损伤账本（漏记一次就是残影）。
///
/// 按**内容**匹配而不是按位置：悬停高亮常在某个位置插入 / 移除一条 `FillRect`，按位置比对
/// 会让其后所有命令都「变了」→ 退化成整幅。这里做多重集匹配（同一条命令可出现多次），
/// 只把「新增 / 消失」的命令矩形算进损伤；匹配后的相对顺序必须单调，否则说明命令跨越了
/// 彼此的绘制顺序（叠放关系变了，像素可能变而命令值不变）→ 整幅。
///
/// 保守规则（宁可多画，绝不漏画）：
/// - 没有上一帧（首帧）→ `None`；
/// - `PushClip` / `PopClip` 新增 / 消失 / 改值 → `None`（裁剪变了，其后命令的可见区域都变）；
/// - 匹配顺序出现倒置（重排 / 跨裁剪移动）→ `None`；
/// - 其余取新增 ∪ 消失命令的矩形之并，并外扩 1 pt 吸收抗锯齿边缘。
fn scene_damage(prev: Option<&Scene>, next: &Scene) -> Option<Rect> {
    let prev = prev?;
    let mut used = vec![false; prev.commands.len()];
    let mut order: Vec<usize> = Vec::with_capacity(next.commands.len());
    let mut damage: Option<Rect> = None;
    for b in &next.commands {
        // 预筛：变体 + 绘制矩形一致的才做逐字段比较（文本内容 / 颜色等）。
        let hit = prev.commands.iter().enumerate().find(|(i, a)| {
            !used[*i] && command_rect(a) == command_rect(b) && same_command(a, b)
        });
        match hit.map(|(i, _)| i) {
            Some(i) => {
                used[i] = true;
                order.push(i);
            }
            None => {
                if is_clip(b) {
                    return None;
                }
                // 新增命令：它盖住的区域要重画（底下的旧像素被覆盖 / 新像素要出现）。
                damage = union_opt(damage, command_rect(b));
            }
        }
    }
    for (i, a) in prev.commands.iter().enumerate() {
        if used[i] {
            continue;
        }
        if is_clip(a) {
            return None;
        }
        // 消失命令：它原来盖住的区域要露出底下的内容。
        damage = union_opt(damage, command_rect(a));
    }
    // 匹配顺序必须单调：倒置 = 绘制顺序变了（叠放关系影响像素），保守整幅。
    if order.windows(2).any(|w| w[0] >= w[1]) {
        return None;
    }
    Some(match damage {
        Some(d) => Rect::new(
            d.origin.x - DAMAGE_BLEED,
            d.origin.y - DAMAGE_BLEED,
            d.size.width + DAMAGE_BLEED * 2.0,
            d.size.height + DAMAGE_BLEED * 2.0,
        ),
        None => Rect::new(0.0, 0.0, 0.0, 0.0),
    })
}

fn is_clip(c: &SceneCommand) -> bool {
    matches!(c, SceneCommand::PushClip { .. } | SceneCommand::PopClip)
}

fn union_opt(acc: Option<Rect>, r: Option<Rect>) -> Option<Rect> {
    match (acc, r) {
        (Some(a), Some(b)) => Some(union(a, b)),
        (None, Some(b)) => Some(b),
        (a, None) => a,
    }
}

/// 抗锯齿 / 亚像素字形的边缘可能越过命令矩形 1 px 以内：损伤外扩吸收，
/// 否则局部重画会在边缘留下上一帧的残色。
const DAMAGE_BLEED: f32 = 1.0;

/// 命令的绘制范围（逻辑像素）。`PopClip` 自身不画东西。
fn command_rect(c: &SceneCommand) -> Option<Rect> {
    use SceneCommand::*;
    match c {
        FillRect { rect, .. } | Text { rect, .. } | Image { rect, .. } => Some(*rect),
        // 描边以矩形边线为中心，向外扩 thickness/2。
        StrokeRect { rect, thickness, .. } => Some(Rect::new(
            rect.origin.x - thickness * 0.5,
            rect.origin.y - thickness * 0.5,
            rect.size.width + thickness,
            rect.size.height + thickness,
        )),
        Arc {
            center,
            radius,
            thickness,
            ..
        } => {
            let r = *radius + thickness * 0.5;
            Some(Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0))
        }
        Triangle { p0, p1, p2, .. } => {
            let (x0, x1) = (p0.x.min(p1.x).min(p2.x), p0.x.max(p1.x).max(p2.x));
            let (y0, y1) = (p0.y.min(p1.y).min(p2.y), p0.y.max(p1.y).max(p2.y));
            Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
        }
        PushClip { rect } => Some(*rect),
        PopClip => None,
    }
}

/// 两条命令是否画出**完全相同**的像素。位图用 `Arc::ptr_eq`：同一份缓冲零拷贝复用，
/// 内容必然相同；直接比内容会在每帧对整张图标做一次逐字节比较。
fn same_command(a: &SceneCommand, b: &SceneCommand) -> bool {
    use SceneCommand::*;
    match (a, b) {
        (
            FillRect {
                color: c1,
                rect: r1,
                corner_radius: k1,
            },
            FillRect {
                color: c2,
                rect: r2,
                corner_radius: k2,
            },
        ) => c1 == c2 && r1 == r2 && k1 == k2,
        (
            StrokeRect {
                color: c1,
                rect: r1,
                thickness: t1,
                corner_radius: k1,
            },
            StrokeRect {
                color: c2,
                rect: r2,
                thickness: t2,
                corner_radius: k2,
            },
        ) => c1 == c2 && r1 == r2 && t1 == t2 && k1 == k2,
        (
            Text {
                content: s1,
                rect: r1,
                color: c1,
                style: y1,
                align: a1,
                wrap: w1,
                max_lines: m1,
                overflow: o1,
            },
            Text {
                content: s2,
                rect: r2,
                color: c2,
                style: y2,
                align: a2,
                wrap: w2,
                max_lines: m2,
                overflow: o2,
            },
        ) => s1 == s2 && r1 == r2 && c1 == c2 && y1 == y2 && a1 == a2 && w1 == w2 && m1 == m2 && o1 == o2,
        (
            Arc {
                center: p1,
                radius: r1,
                thickness: t1,
                color: c1,
                start_deg: s1,
                end_deg: e1,
            },
            Arc {
                center: p2,
                radius: r2,
                thickness: t2,
                color: c2,
                start_deg: s2,
                end_deg: e2,
            },
        ) => p1 == p2 && r1 == r2 && t1 == t2 && c1 == c2 && s1 == s2 && e1 == e2,
        (PushClip { rect: r1 }, PushClip { rect: r2 }) => r1 == r2,
        (PopClip, PopClip) => true,
        (
            Image {
                rgba: g1,
                width: w1,
                height: h1,
                rect: r1,
                tint: t1,
                opacity: o1,
            },
            Image {
                rgba: g2,
                width: w2,
                height: h2,
                rect: r2,
                tint: t2,
                opacity: o2,
            },
        ) => {
            std::sync::Arc::ptr_eq(g1, g2)
                && w1 == w2
                && h1 == h2
                && r1 == r2
                && t1 == t2
                && o1 == o2
        }
        (
            Triangle {
                p0: a0,
                p1: a1,
                p2: a2,
                color: c1,
            },
            Triangle {
                p0: b0,
                p1: b1,
                p2: b2,
                color: c2,
            },
        ) => a0 == b0 && a1 == b1 && a2 == b2 && c1 == c2,
        _ => false,
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
            repeat: false,
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
            repeat: false,
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

    // ── 弹层局部损伤（tb-card-perf）────────────────────────────────────────────
    //
    // 弹层根常是整块自绘的自定义控件（网络卡 / 音量卡），元素树只能给它整块绘制范围。
    // 外壳按 Scene 差异算损伤 —— 下面钉住「只重画变化那块」与几条保守回退规则。

    /// 一张 360×340 的卡：整块底色 + 一行（可选高亮），全部在一层裁剪里。
    fn card_scene(row_hl: bool) -> Scene {
        let mut s = Scene::default();
        s.fill_rect(
            kanesumi_core::Color::rgb(0.10, 0.10, 0.10),
            Rect::new(0.0, 0.0, 360.0, 340.0),
        );
        s.push_clip(Rect::new(0.0, 0.0, 360.0, 340.0));
        let c = if row_hl {
            kanesumi_core::Color::rgb(0.20, 0.20, 0.20)
        } else {
            kanesumi_core::Color::rgb(0.15, 0.15, 0.15)
        };
        s.fill_rect(c, Rect::new(16.0, 56.0, 328.0, 56.0));
        s.pop_clip();
        s
    }

    /// 只换一行的底色 → 损伤 = 那一行（外扩 1 pt 吸收抗锯齿），不是整块。
    #[test]
    fn popup_scene_diff_is_local_for_a_row_change() {
        let (a, b) = (card_scene(false), card_scene(true));
        let d = scene_damage(Some(&a), &b).expect("应得局部损伤");
        assert!(
            (d.origin.x - 15.0).abs() < 0.01
                && (d.origin.y - 55.0).abs() < 0.01
                && (d.size.width - 330.0).abs() < 0.01
                && (d.size.height - 58.0).abs() < 0.01,
            "损伤应恰为那一行外扩 1 pt：{d:?}"
        );
        assert!(
            d.size.width * d.size.height < 360.0 * 340.0 * 0.4,
            "局部损伤必须远小于整卡（门 < 40%）：{d:?}"
        );
    }

    /// 两帧一模一样 → 零面积（外壳据此跳过光栅与提交，不产生新帧）。
    #[test]
    fn popup_scene_diff_of_identical_frames_is_empty() {
        let a = card_scene(false);
        let d = scene_damage(Some(&a), &a.clone()).expect("应得零面积");
        assert_eq!((d.size.width, d.size.height), (0.0, 0.0), "无变化 = 零面积");
    }

    /// 首帧（没有上一帧）必须整幅；裁剪命令消失也整幅 —— 宁可多画。
    #[test]
    fn popup_scene_diff_falls_back_to_full() {
        let a = card_scene(false);
        assert!(scene_damage(None, &a).is_none(), "首帧整幅");

        let mut clipped = a.clone();
        clipped.commands[1] = SceneCommand::PushClip {
            rect: Rect::new(0.0, 0.0, 360.0, 300.0),
        };
        assert!(scene_damage(Some(&a), &clipped).is_none(), "裁剪变化整幅");
    }

    /// 悬停高亮这类「插入一条命令」只该重画那条命令的范围，不能退化成整幅。
    #[test]
    fn popup_scene_diff_handles_inserted_command() {
        let a = card_scene(false);
        let mut b = a.clone();
        // 第 2 条（裁剪之后）插入一行悬停底：其后命令整体后移一位。
        b.commands.insert(
            2,
            SceneCommand::FillRect {
                color: kanesumi_core::Color::rgb(0.25, 0.25, 0.25),
                rect: Rect::new(16.0, 120.0, 328.0, 56.0),
                corner_radius: 0.0,
            },
        );
        let d = scene_damage(Some(&a), &b).expect("插入命令应得局部损伤");
        assert!(
            (d.origin.y - 119.0).abs() < 0.01 && (d.size.height - 58.0).abs() < 0.01,
            "只该重画插入的那一行：{d:?}"
        );
    }

    /// 两条命令互换了绘制顺序（叠放关系变了，像素可能变而命令值不变）→ 整幅。
    #[test]
    fn popup_scene_diff_reorder_is_full() {
        let mut a = card_scene(false);
        a.commands.swap(0, 2);
        assert!(scene_damage(Some(&card_scene(false)), &a).is_none(), "重排整幅");
    }

    /// 主表面损伤是否唤醒弹层：没变（`None` + 非整幅）不唤醒，碰不到弹层的损伤也不唤醒。
    #[test]
    fn popup_wakeup_requires_intersecting_damage() {
        let extent = Rect::new(100.0, 30.0, 360.0, 340.0);
        assert!(!popup_damage_hits(false, None, extent), "没变不得唤醒弹层");
        assert!(
            !popup_damage_hits(false, Some(Rect::new(0.0, 0.0, 30.0, 30.0)), extent),
            "条上图标的损伤不得唤醒弹层"
        );
        assert!(popup_damage_hits(false, Some(Rect::new(120.0, 60.0, 10.0, 10.0)), extent));
        assert!(popup_damage_hits(true, None, extent), "主表面整幅 → 全部参与判定");
    }
}
