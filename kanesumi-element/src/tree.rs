// tree.rs —— 保留元素树本体。参 ELEMENT_TREE §Ⅲ~§Ⅵ。
//
// 一帧的顺序（`frame`）：动画 tick → 布局（只算脏子树，量测带缓存）→ 绘制（只重画脏节点，
// 逐节点命令缓存）→ 拼接 Scene + 产出损伤。输入在帧与帧之间到达，命中用的是**上一帧**
// 的布局产物 —— 正是用户眼睛看到的那一帧（画在哪就点在哪）。

use std::any::Any;

use kanesumi_anim::Animation;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::event::{Event, Key, Modifiers, PointerButton};
use crate::id::WidgetId;
use crate::layer::{LayerAnimSpec, LayerOp, LayerState};
use crate::ime::ImeContext;
use crate::props::{Align, LayoutProps};
use crate::widget::{
    ArrangeCtx, ControlStates, EventCtx, MeasureCtx, PaintCtx, RealizeCtx, UpdateCtx, Widget,
};
use crate::widgets::{TOOLTIP_GAP, Tooltip};

/// 控件发给 App 的动作（控件自定义类型，App downcast）。
pub type Action = Box<dyn Any>;

/// 框架动作：弹层被轻触关闭（点外部 / Esc）。来源 id = 该弹层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PopupDismissed;

/// 弹层相对锚点的方位（XAML `FlyoutPlacementMode` 的主方位）。放不下时翻到对侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupSide {
    /// 锚点下方（默认；下方放不下而上方放得下则上翻 —— ComboBoxHelper 判据）。
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    /// 表面居中（对话框）。锚点仍用于关闭通知与焦点交还 —— `EventCtx::open_popup` 总会把锚点设为
    /// 打开者，故「居中」要显式声明，不能靠「无锚点」（E3 批 F 对话框迁移时发现）。
    Center,
}

/// 弹层规格。参 ELEMENT_TREE §Ⅴ.2。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopupSpec {
    /// 锚点元素：面板贴其下缘放置，下方放不下则上翻；`None` 且无 `at` = 表面居中（对话框）。
    /// 锚点同时决定「谁收 `PopupClosed`、焦点交还给谁」。
    pub anchor: Option<WidgetId>,
    /// 点锚定（右键菜单）：面板左上角落在该点，右侧放不下翻左、下方放不下翻上。
    /// 设置后优先于 `anchor` 的下缘放置（`anchor` 仍用于关闭通知与焦点交还）。
    pub at: Option<Point>,
    /// 相对锚点的方位（仅 `anchor` 生效且未设 `at` 时使用）。
    pub side: PopupSide,
    /// 沿锚点边的对齐：`Start`（默认，左 / 上缘对齐）/ `Center` / `End`；`Stretch` 视同 `Start`。
    /// 例：命令条在选区上方居中 = `side: Top, align: Center`。
    pub align: crate::props::Align,
    /// 与锚点的间距。
    pub gap: f32,
    /// 模态：点外部不关闭、吞掉点击；Tab 限制在弹层内（焦点陷阱）。
    pub modal: bool,
    /// 点外部 / Esc 关闭（LightDismiss）。模态弹层忽略「点外部」。
    pub light_dismiss: bool,
    /// 在弹层之下、内容之上铺一层遮罩（主题 `overlay_color`）：对话框 / 需要压暗背景的面板。
    pub scrim: bool,
    /// 输入穿透（工具提示）：框架不把它当作「顶层弹层」参与轻触关闭 / Esc / 焦点陷阱，
    /// 落在它上面的点击继续投给下层内容。参见 `Tree::top_popup`。
    pub passthrough: bool,
}

impl Default for PopupSpec {
    fn default() -> Self {
        Self {
            anchor: None,
            at: None,
            side: PopupSide::Bottom,
            align: crate::props::Align::Start,
            gap: 0.0,
            modal: false,
            light_dismiss: true,
            scrim: false,
            passthrough: false,
        }
    }
}

/// 一帧产物。
#[derive(Debug, Clone, Default)]
pub struct FrameOutput {
    pub scene: Scene,
    /// 本帧变化区域；`None` = 全量重绘（首帧 / 尺寸变化 / 主题变化）**或本帧什么都没变** —— 二者用 `full` 区分。
    pub damage: Option<Rect>,
    /// 本帧是否整幅重绘。`!full && damage == None` = 什么都没变（外壳可跳过光栅与提交）。
    pub full: bool,
    /// 是否仍有动画在跑（外壳据此继续请求帧）。
    pub animating: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    needs_measure: bool,
    needs_arrange: bool,
    needs_paint: bool,
    /// 已在待绘集合里排队（去重；`invalidate_paint` 不重复入队）。参 `Tree::paint_queue`。
    paint_queued: bool,
    /// 下一帧 measure 之前需要调用 `Widget::realize`（虚拟化容器增删子节点）。
    needs_realize: bool,
    disabled: bool,
}

struct Node {
    widget: Option<Box<dyn Widget>>,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    props: LayoutProps,
    flags: Flags,
    /// 量测缓存：期望尺寸（含 margin）与当时的可用约束。
    desired: Size,
    last_available: Option<Size>,
    rect: Rect,
    paint: Vec<SceneCommand>,
    /// `Widget::paint_after` 的命令缓存（子树之后输出）。
    paint_after: Vec<SceneCommand>,
    /// 上次绘制覆盖范围（空 = 从未绘制 / 已清除）。
    painted_bounds: Option<Rect>,
    state: ControlStates,
    /// 挂在本节点上的工具提示文字（`Tree::set_tooltip`）。空 / `None` = 无提示。
    tooltip: Option<String>,
}

impl Node {
    fn new(widget: Box<dyn Widget>, parent: Option<WidgetId>, props: LayoutProps) -> Self {
        Self {
            widget: Some(widget),
            parent,
            children: Vec::new(),
            props,
            flags: Flags {
                needs_measure: true,
                needs_arrange: true,
                needs_paint: true,
                paint_queued: false,
                needs_realize: true,
                disabled: false,
            },
            desired: Size::ZERO,
            last_available: None,
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            paint: Vec::new(),
            paint_after: Vec::new(),
            painted_bounds: None,
            state: ControlStates::default(),
            tooltip: None,
        }
    }
}

/// 编辑上下文：`Tree::edit` 闭包里声明本次修改的影响面。未声明 → 按量测失效（宁多算不漏画）。
#[derive(Debug, Default)]
pub struct EditCtx {
    measure: bool,
    arrange: bool,
    paint: bool,
}

impl EditCtx {
    pub fn invalidate_measure(&mut self) {
        self.measure = true;
    }
    pub fn invalidate_arrange(&mut self) {
        self.arrange = true;
    }
    pub fn invalidate_paint(&mut self) {
        self.paint = true;
    }
}

/// 保留元素树。
pub struct Tree {
    nodes: Vec<Option<Node>>,
    gens: Vec<u32>,
    free: Vec<usize>,
    root: WidgetId,
    overlay: WidgetId,
    theme: MetroTheme,
    size: Size,
    focus: Option<WidgetId>,
    focus_keyboard: bool,
    hover_chain: Vec<WidgetId>,
    captured: Option<WidgetId>,
    press_target: Option<WidgetId>,
    context_target: Option<WidgetId>,
    popups: Vec<(WidgetId, PopupSpec)>,
    actions: Vec<(WidgetId, Action)>,
    /// 待绘节点集合（去重）。`invalidate_paint` / 新建 / 排列改矩形时入队，
    /// `frame` 只处理这些节点，不再每帧递归整棵树找 `needs_paint`（参 ELEMENT_TREE）。
    paint_queue: Vec<WidgetId>,
    anim: Vec<WidgetId>,
    /// 定时器：(节点, 剩余秒)。到期把节点放进动画 tick（调用其 `update`）。
    timers: Vec<(WidgetId, f64)>,
    /// 因不可见而暂停的动画节点：不 tick、不占帧；重新可见时恢复。
    parked: Vec<WidgetId>,
    damage: Option<Rect>,
    full_repaint: bool,
    dirty: bool,
    /// 本帧的真实经过时间（秒，**未限幅**）—— `animate` 用它推进动画，故掉帧不会拉长总时长。
    /// 由外壳经 `frame` 注入（参 ELEMENT_TREE §帧调度）。
    frame_dt: f64,
    /// 失效代数：每次 `mark_dirty` 自增。`frame` 记录起始值，帧末若已变化说明帧内产生了新失效
    /// （布局 / 绘制 / 动画推进中），`dirty` 必须留给下一帧，不得被清掉（参 ELEMENT_TREE §帧调度）。
    invalidate_epoch: u64,
    /// 最近一次指针位置（`Click` 等不带坐标的事件里，控件经 `EventCtx::pointer` 取用）。
    pointer: Option<Point>,
    /// 焦点控件本帧的 IME 上下文（`frame` 末尾计算，外壳查询时直接返回）。
    ime: Option<ImeContext>,
    /// 上一帧的排版引擎（`TextEngine` clone 为零拷贝）。事件处理里的文本命中
    /// （点击定位光标）要用它 —— 外壳的输入回调不带引擎。
    engine: Option<TextEngine>,
    /// 弹层放置区（树坐标）。None = 表面本身。外壳可放大到整个输出：
    /// 30px 高的 TopBar 上，菜单要能落到表面之外（由外壳另开 xdg_popup 承载）。
    popup_bounds: Option<Rect>,
    /// 弹层分离：主表面 Scene 不含覆盖层（及遮罩），每个弹层经 `popup_scene` 单独取，
    /// 由外壳画进各自的 xdg_popup 表面。参 ELEMENT_TREE §弹层分离。
    detached_popups: bool,
    /// 按本帧 damage 剔除拼接（只拼与 damage 相交的节点）。**仅当消费方只重绘 damage 区**
    ///（CPU 局部光栅）才可开；整幅重画的消费方（wgpu 直出、快照）开了会丢内容。默认关。
    damage_cull: bool,
    /// 图层（G3-c）：被标为图层的节点及其提交状态；BTreeMap 保证产出顺序稳定。参 layer.rs。
    layers: std::collections::BTreeMap<WidgetId, LayerState>,
    /// 待外壳取走的图层操作。
    layer_ops: Vec<LayerOp>,
    /// compose 时允许进入的图层根：拼某图层内容时 = 该图层；拼主场景时 = None（跳过所有图层子树）。
    compose_layer: std::cell::Cell<Option<WidgetId>>,

    // ── 工具提示（`Tree::set_tooltip`；计时与挂载由框架负责）──────────────────
    /// 已武装但尚未到期的提示目标（悬停 / 键盘焦点持有时累计延迟）。
    tooltip_pending: Option<WidgetId>,
    /// 距显示的剩余秒数（`tooltip_pending` 有效时）。
    tooltip_delay: Option<f64>,
    /// 正在显示的提示所锚定的元素（无提示显示时 None）。
    tooltip_anchor: Option<WidgetId>,
    /// 正在显示的提示弹层节点 id。
    tooltip_popup: Option<WidgetId>,
    /// 正在显示的提示距自动消失（5 s）的剩余秒数。
    tooltip_hide: Option<f64>,
}

impl Tree {
    pub fn new(theme: MetroTheme) -> Self {
        let mut tree = Self {
            nodes: Vec::new(),
            gens: Vec::new(),
            free: Vec::new(),
            root: WidgetId::new(0, 0),
            overlay: WidgetId::new(0, 0),
            theme,
            size: Size::ZERO,
            focus: None,
            focus_keyboard: false,
            hover_chain: Vec::new(),
            captured: None,
            press_target: None,
            context_target: None,
            popups: Vec::new(),
            actions: Vec::new(),
            paint_queue: Vec::new(),
            anim: Vec::new(),
            timers: Vec::new(),
            parked: Vec::new(),
            damage: None,
            damage_cull: false,
            layers: std::collections::BTreeMap::new(),
            layer_ops: Vec::new(),
            compose_layer: std::cell::Cell::new(None),
            full_repaint: true,
            dirty: true,
            frame_dt: 0.0,
            invalidate_epoch: 0,
            ime: None,
            engine: None,
            pointer: None,
            popup_bounds: None,
            detached_popups: false,
            tooltip_pending: None,
            tooltip_delay: None,
            tooltip_anchor: None,
            tooltip_popup: None,
            tooltip_hide: None,
        };
        tree.root = tree.alloc(Box::new(ZStack), None, LayoutProps::default());
        tree.overlay = tree.alloc(Box::new(OverlayRoot), None, LayoutProps::default());
        tree
    }

    // ── 结构 ────────────────────────────────────────────────────────────────

    /// 内容根：App 把页面挂在它下面。根的每个子节点都铺满整个表面（Z 叠放）。
    /// 开 / 关按 damage 剔除拼接（见字段注释）。外壳确认本表面走局部光栅后才开。
    pub fn set_damage_cull(&mut self, on: bool) {
        self.damage_cull = on;
    }

    /// 下一帧整幅重画（外壳的缓冲失效：表面重新显示 / 尺寸变化 / 光栅器新建）。
    pub fn request_full_repaint(&mut self) {
        self.full_repaint = true;
        self.mark_dirty();
    }

    pub fn root(&self) -> WidgetId {
        self.root
    }

    fn alloc(
        &mut self,
        widget: Box<dyn Widget>,
        parent: Option<WidgetId>,
        props: LayoutProps,
    ) -> WidgetId {
        let node = Node::new(widget, parent, props);
        let id = if let Some(slot) = self.free.pop() {
            self.nodes[slot] = Some(node);
            WidgetId::new(slot, self.gens[slot])
        } else {
            self.nodes.push(Some(node));
            self.gens.push(0);
            WidgetId::new(self.nodes.len() - 1, 0)
        };
        // 新节点初始 `needs_paint = true`：直接进待绘集合，避免首帧整树递归。
        if let Some(n) = self.node_mut(id) {
            n.flags.paint_queued = true;
        }
        self.paint_queue.push(id);
        id
    }

    /// 把需要绘制的节点入待绘集合（去重）。
    fn queue_paint(&mut self, id: WidgetId) {
        let Some(n) = self.node_mut(id) else { return };
        if n.flags.needs_paint && !n.flags.paint_queued {
            n.flags.paint_queued = true;
            self.paint_queue.push(id);
        }
    }

    fn node(&self, id: WidgetId) -> Option<&Node> {
        if self.gens.get(id.slot()) != Some(&id.generation) {
            return None;
        }
        self.nodes.get(id.slot())?.as_ref()
    }

    fn node_mut(&mut self, id: WidgetId) -> Option<&mut Node> {
        if self.gens.get(id.slot()) != Some(&id.generation) {
            return None;
        }
        self.nodes.get_mut(id.slot())?.as_mut()
    }

    pub fn contains(&self, id: WidgetId) -> bool {
        self.node(id).is_some()
    }

    /// 追加子节点（默认布局属性）。父不存在时挂到内容根并记错误日志。
    pub fn insert(&mut self, parent: WidgetId, widget: impl Widget) -> WidgetId {
        self.insert_with(parent, widget, LayoutProps::default())
    }

    pub fn insert_with(
        &mut self,
        parent: WidgetId,
        widget: impl Widget,
        props: LayoutProps,
    ) -> WidgetId {
        self.insert_boxed_with(parent, Box::new(widget), props)
    }

    /// 装箱版 `insert_with`（元素工厂 `ItemFactory::build` 返回 `Box<dyn Widget>` 时用）。
    pub fn insert_boxed_with(
        &mut self,
        parent: WidgetId,
        widget: Box<dyn Widget>,
        props: LayoutProps,
    ) -> WidgetId {
        let parent = if self.contains(parent) {
            parent
        } else {
            log::error!("kanesumi-element: insert 的父节点不存在，改挂内容根");
            self.root
        };
        let id = self.alloc(widget, Some(parent), props);
        if let Some(p) = self.node_mut(parent) {
            p.children.push(id);
        }
        self.invalidate_measure(parent);
        id
    }

    /// 删除节点及其子树。其绘制范围计入损伤；焦点 / 捕获 / 悬停引用一并清除。
    pub fn remove(&mut self, id: WidgetId) {
        if id == self.root || id == self.overlay || !self.contains(id) {
            return;
        }
        let parent = self.node(id).and_then(|n| n.parent);
        let mut stack = vec![id];
        let mut doomed = Vec::new();
        while let Some(cur) = stack.pop() {
            if let Some(n) = self.node(cur) {
                stack.extend(n.children.iter().copied());
                doomed.push(cur);
            }
        }
        for cur in &doomed {
            if let Some(b) = self.node(*cur).and_then(|n| n.painted_bounds) {
                self.add_damage(b);
            }
        }
        for cur in &doomed {
            if self.focus == Some(*cur) {
                self.focus = None;
            }
            if self.captured == Some(*cur) {
                self.captured = None;
            }
            if self.press_target == Some(*cur) {
                self.press_target = None;
            }
            if self.context_target == Some(*cur) {
                self.context_target = None;
            }
            self.hover_chain.retain(|h| h != cur);
            self.anim.retain(|a| a != cur);
            self.timers.retain(|(t, _)| t != cur);
            self.parked.retain(|p| p != cur);
            self.popups.retain(|(p, _)| p != cur);
            let slot = cur.slot();
            self.nodes[slot] = None;
            self.gens[slot] = self.gens[slot].wrapping_add(1);
            self.free.push(slot);
        }
        // 提示目标 / 弹层随节点删除而失效：清状态并按需收起存活的提示弹层。
        if self.tooltip_pending.is_some_and(|t| !self.contains(t)) {
            self.tooltip_pending = None;
            self.tooltip_delay = None;
        }
        if self.tooltip_popup.is_some_and(|p| !self.contains(p)) {
            self.tooltip_popup = None;
            self.tooltip_anchor = None;
            self.tooltip_hide = None;
        } else if self.tooltip_anchor.is_some_and(|a| !self.contains(a)) {
            // 锚点已删而弹层尚在：收起弹层。
            if let Some(p) = self.tooltip_popup.take() {
                self.close_popup(p);
            }
        }
        if let Some(p) = parent {
            if let Some(pn) = self.node_mut(p) {
                pn.children.retain(|c| *c != id);
            }
            self.invalidate_measure(p);
        }
        self.mark_dirty();
    }

    /// 删除某节点的全部子节点（页面切换用）。
    pub fn clear_children(&mut self, id: WidgetId) {
        for c in self.children(id).to_vec() {
            self.remove(c);
        }
    }

    pub fn children(&self, id: WidgetId) -> &[WidgetId] {
        self.node(id).map(|n| n.children.as_slice()).unwrap_or(&[])
    }

    pub fn parent(&self, id: WidgetId) -> Option<WidgetId> {
        self.node(id)?.parent
    }

    pub fn props(&self, id: WidgetId) -> Option<LayoutProps> {
        self.node(id).map(|n| n.props)
    }

    /// 修改布局属性。隐藏节点时立即把其子树绘制范围计入损伤。
    pub fn update_props(&mut self, id: WidgetId, f: impl FnOnce(&mut LayoutProps)) {
        let Some(n) = self.node_mut(id) else { return };
        let before = n.props;
        f(&mut n.props);
        let after = n.props;
        if before == after {
            return;
        }
        if before.visible && !after.visible {
            self.flush_subtree_paint(id);
        }
        if !before.visible && after.visible {
            // 重新可见：恢复该子树里被暂停的动画（不确定进度环等）。
            let resume: Vec<WidgetId> = self
                .parked
                .iter()
                .copied()
                .filter(|p| self.is_ancestor_or_self(id, *p))
                .collect();
            self.parked.retain(|p| !resume.contains(p));
            for r in resume {
                self.request_anim(r);
            }
            // 重新可见：确保该节点重画（隐藏期间 needs_paint 被保留但未入队）。
            self.invalidate_paint(id);
        }
        self.invalidate_measure(id);
        if let Some(p) = self.parent(id) {
            self.invalidate_measure(p);
        }
    }

    /// 布局产物（上一帧）。
    pub fn rect(&self, id: WidgetId) -> Option<Rect> {
        self.node(id).map(|n| n.rect)
    }

    pub fn theme(&self) -> &MetroTheme {
        &self.theme
    }

    /// 更换主题（Chorus 推送）：全树重量测重画（字号梯度可能随主题变化）。
    pub fn set_theme(&mut self, theme: MetroTheme) {
        self.theme = theme;
        for n in self.nodes.iter_mut().flatten() {
            n.flags.needs_measure = true;
            n.flags.needs_arrange = true;
            n.flags.needs_paint = true;
            n.flags.paint_queued = true;
            n.flags.needs_realize = true;
        }
        self.paint_queue = self.all_ids();
        self.full_repaint = true;
        self.mark_dirty();
    }

    /// 以具体类型读取控件。
    pub fn get<T: Widget>(&self, id: WidgetId) -> Option<&T> {
        let w: &dyn Widget = self.node(id)?.widget.as_deref()?;
        let any: &dyn Any = w;
        any.downcast_ref::<T>()
    }

    /// App 修改控件的唯一入口。闭包内经 `EditCtx` 声明影响面；未声明按量测失效。
    pub fn edit<T: Widget, R>(
        &mut self,
        id: WidgetId,
        f: impl FnOnce(&mut T, &mut EditCtx) -> R,
    ) -> Option<R> {
        let mut ctx = EditCtx::default();
        let r = {
            let w: &mut dyn Widget = self.node_mut(id)?.widget.as_deref_mut()?;
            let any: &mut dyn Any = w;
            let t = any.downcast_mut::<T>()?;
            f(t, &mut ctx)
        };
        if ctx.measure || !(ctx.arrange || ctx.paint) {
            self.invalidate_measure(id);
        } else if ctx.arrange {
            self.invalidate_arrange(id);
        }
        // 内容变化必然要重画自身（量测 / 排列失效只保证矩形变化时重画）。
        self.invalidate_paint(id);
        Some(r)
    }

    pub fn set_enabled(&mut self, id: WidgetId, enabled: bool) {
        let Some(n) = self.node_mut(id) else { return };
        if n.flags.disabled != enabled {
            return;
        }
        n.flags.disabled = !enabled;
        // 禁用影响整棵子树的外观。
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            stack.extend(self.children(cur).iter().copied());
            self.invalidate_paint(cur);
        }
        if !enabled && self.focus.is_some_and(|f| self.is_ancestor_or_self(id, f)) {
            self.set_focus(None, false);
        }
    }

    pub fn is_enabled(&self, id: WidgetId) -> bool {
        !self.effectively_disabled(id)
    }

    fn effectively_disabled(&self, id: WidgetId) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node(c) else { return true };
            if n.flags.disabled {
                return true;
            }
            cur = n.parent;
        }
        false
    }

    fn effectively_visible(&self, id: WidgetId) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node(c) else { return false };
            if !n.props.visible {
                return false;
            }
            cur = n.parent;
        }
        true
    }

    /// `ancestor` 是否为 `id` 自身或其祖先。
    pub fn is_ancestor_or_self(&self, ancestor: WidgetId, id: WidgetId) -> bool {
        let mut cur = Some(id);
        while let Some(c) = cur {
            if c == ancestor {
                return true;
            }
            cur = self.parent(c);
        }
        false
    }

    /// 自身到根的链（含自身，叶 → 根）。
    fn ancestors_inclusive(&self, id: WidgetId) -> Vec<WidgetId> {
        let mut out = Vec::new();
        let mut cur = Some(id);
        while let Some(c) = cur {
            if !self.contains(c) {
                break;
            }
            out.push(c);
            cur = self.parent(c);
        }
        out
    }

    /// 框架维护的交互状态（hover / pressed / focused / disabled）。
    pub fn states_of(&self, id: WidgetId) -> ControlStates {
        self.states(id)
    }

    pub(crate) fn states(&self, id: WidgetId) -> ControlStates {
        let mut s = self.node(id).map(|n| n.state).unwrap_or_default();
        s.disabled = self.effectively_disabled(id);
        s
    }

    // ── 失效 ────────────────────────────────────────────────────────────────

    /// 置脏并推进失效代数。所有「下一帧必须再出帧」的来源都应经此，而非直接写 `dirty`，
    /// 以便 `frame` 分辨「帧内新产生的失效」（参 `invalidate_epoch`）。
    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.invalidate_epoch = self.invalidate_epoch.wrapping_add(1);
    }

    pub fn invalidate_measure(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_measure = true;
            n.flags.needs_arrange = true;
            // 量测失效的源头是「子树 / 数据变了」—— 需要 realize 的容器（虚拟化列表）
            // 沿祖先链一并标记，下一帧按新视口重新实现（参 ELEMENT_TREE §Ⅳ-bis）。
            n.flags.needs_realize = true;
            cur = n.parent;
        }
        self.mark_dirty();
    }

    /// 只标记本节点需要 realize（滚动偏移变化 / 数据变化这类不动量测缓存的来源）。
    pub fn invalidate_realize(&mut self, id: WidgetId) {
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_realize = true;
            self.mark_dirty();
        }
    }

    pub fn invalidate_arrange(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_arrange = true;
            cur = n.parent;
        }
        self.mark_dirty();
    }

    pub fn invalidate_paint(&mut self, id: WidgetId) {
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_paint = true;
            self.mark_dirty();
        }
        self.queue_paint(id);
    }

    /// 只标记 arrange 失效、不置脏。帧内强制重排（弹层位置跟随锚点）用：结果被本帧
    /// arrange 消费，不应触发额外帧。参 `frame` 第 3 步。
    fn invalidate_arrange_quiet(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_arrange = true;
            cur = n.parent;
        }
    }

    /// 只标记量测失效、不置脏。帧内框架自身产生且会被本帧 measure 消费的失效用
    /// （尺寸变化 / 重排时改矩形的重画），不应触发额外帧。
    fn invalidate_measure_quiet(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_measure = true;
            n.flags.needs_arrange = true;
            n.flags.needs_realize = true;
            cur = n.parent;
        }
    }

    /// 只要求重画、不置脏。帧内框架自身改矩形后的重画用（本帧 paint 步会消费）。
    fn invalidate_paint_quiet(&mut self, id: WidgetId) {
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_paint = true;
        }
        self.queue_paint(id);
    }

    /// 框架统一动画推进入口（`UpdateCtx::animate` / `PaintCtx::animate` 的底层）：
    /// 按本帧真实经过时间推进 `anim`，未到稳态自动登记动画帧（控件无需再手动请求）。
    pub(crate) fn animate<A: Animation>(&mut self, id: WidgetId, anim: &mut A) -> f64 {
        anim.advance(self.frame_dt);
        if !anim.is_steady() {
            self.request_anim(id);
        }
        anim.value()
    }

    pub(crate) fn request_anim(&mut self, id: WidgetId) {
        if !self.anim.contains(&id) {
            self.anim.push(id);
        }
        self.mark_dirty();
    }

    /// 请求 `secs` 秒后调用该节点的 `update`（光标闪烁、延时提示这类低频唤醒）。
    /// 同一节点只保留最早的一个。与 `request_anim_frame` 不同：等待期间不占帧。
    pub(crate) fn request_timer(&mut self, id: WidgetId, secs: f64) {
        let secs = secs.max(0.0);
        match self.timers.iter_mut().find(|(t, _)| *t == id) {
            Some((_, left)) => *left = left.min(secs),
            None => self.timers.push((id, secs)),
        }
    }

    /// 推进定时器（外壳每次循环调用，含空闲兜底唤醒）。到期者转入动画 tick 并置脏。
    /// 工具提示计时也在此推进（等待期不占帧，靠 `next_timer` 唤醒）。
    pub fn tick_timers(&mut self, dt: f64) {
        self.tick_tooltip(dt);
        if self.timers.is_empty() {
            return;
        }
        let mut due = Vec::new();
        self.timers.retain_mut(|(id, left)| {
            *left -= dt;
            if *left <= 0.0 {
                due.push(*id);
                false
            } else {
                true
            }
        });
        for id in due {
            self.request_anim(id);
        }
    }

    /// 最近一个定时器还剩多久（外壳可据此安排唤醒；无定时器为 None）。
    pub fn next_timer(&self) -> Option<f64> {
        let widget = self.timers.iter().map(|(_, l)| *l).reduce(f64::min);
        let tooltip = [self.tooltip_delay, self.tooltip_hide]
            .into_iter()
            .flatten()
            .reduce(f64::min);
        match (widget, tooltip) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    pub(crate) fn push_action(&mut self, from: WidgetId, action: Action) {
        self.actions.push((from, action));
    }

    /// 取走本批控件动作（App 在输入处理后调用）。
    pub fn take_actions(&mut self) -> Vec<(WidgetId, Action)> {
        std::mem::take(&mut self.actions)
    }

    /// 是否需要出新帧（有失效 / 动画 / 状态变化）。
    pub fn needs_frame(&self) -> bool {
        self.dirty || !self.anim.is_empty()
    }

    fn add_damage(&mut self, r: Rect) {
        if r.size.width <= 0.0 || r.size.height <= 0.0 {
            return;
        }
        self.damage = Some(match self.damage {
            Some(d) => union(d, r),
            None => r,
        });
    }

    fn flush_subtree_paint(&mut self, id: WidgetId) {
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            stack.extend(self.children(cur).iter().copied());
            let old = self.node_mut(cur).and_then(|n| {
                n.paint.clear();
                n.paint_after.clear();
                n.flags.needs_paint = true;
                n.painted_bounds.take()
            });
            self.queue_paint(cur);
            if let Some(b) = old {
                self.add_damage(b);
            }
        }
        self.mark_dirty();
    }

    // ── 实现钩子（参 §Ⅳ-bis）───────────────────────────────────────────────

    /// 对「`wants_realize()` 且被标记 `needs_realize`」的可见节点调用 `Widget::realize`。
    /// 先序执行；回调里插入的新子节点若自身需要 realize，将在下一帧处理（本帧 measure 仍会量测它）。
    fn realize_dirty(&mut self, engine: &TextEngine) {
        let mut ids = Vec::new();
        for r in [self.root, self.overlay] {
            self.collect_realize(r, &mut ids);
        }
        if ids.is_empty() {
            return;
        }
        for id in ids {
            let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) else {
                continue;
            };
            {
                let mut ctx = RealizeCtx {
                    tree: self,
                    id,
                    engine,
                };
                w.realize(&mut ctx);
            }
            if let Some(n) = self.node_mut(id) {
                n.widget = Some(w);
                // 回调里 insert / remove / invalidate_measure 会再次标记本节点；本帧已实现，
                // 视为已消费 —— 连续重复 realize 会与「避免每帧调用」的初衷相悖。
                n.flags.needs_realize = false;
            }
        }
    }

    fn collect_realize(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        let Some(n) = self.node(id) else { return };
        if !n.props.visible {
            return;
        }
        if n.flags.needs_realize
            && n.widget.as_ref().is_some_and(|w| w.wants_realize())
        {
            out.push(id);
        }
        for &c in &n.children {
            self.collect_realize(c, out);
        }
    }

    // ── 帧 ──────────────────────────────────────────────────────────────────

    /// 出一帧：动画 tick → 布局 → 绘制 → 拼接 + 损伤。
    ///
    /// `dt` 为**真实经过时间**（秒，外壳未限幅）—— 动画据此按墙钟推进，掉帧不拉长总时长。
    /// 帧末只清「帧开始前就存在」的 `dirty`；帧内新产生的失效（布局 / 绘制 / 动画推进中）
    /// 经 `invalidate_epoch` 识别并留到下一帧（参 ELEMENT_TREE §帧调度）。
    pub fn frame(&mut self, engine: &TextEngine, size: Size, dt: f64) -> FrameOutput {
        let epoch0 = self.invalidate_epoch;
        self.frame_dt = dt;
        if self.engine.is_none() {
            self.engine = Some(engine.clone());
        }
        let size = size.normalized();
        if size != self.size {
            self.size = size;
            // 本帧即重排重画，quiet 版避免多出一帧（参 ELEMENT_TREE §帧调度）。
            self.invalidate_measure_quiet(self.root);
            self.invalidate_measure_quiet(self.overlay);
            self.full_repaint = true;
        }

        // 1. 动画：只允许动视觉（UpdateCtx 不提供量测失效）。
        for id in std::mem::take(&mut self.anim) {
            // 不可见节点的动画暂停：不 tick、不让外壳逐帧重画（隐藏的不确定进度环曾无限请求帧）。
            if !self.effectively_visible(id) {
                if self.contains(id) && !self.parked.contains(&id) {
                    self.parked.push(id);
                }
                continue;
            }
            let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) else {
                continue;
            };
            let mut ctx = UpdateCtx { tree: self, id };
            w.update(&mut ctx, dt);
            if let Some(n) = self.node_mut(id) {
                n.widget = Some(w);
            }
        }

        // 2. 实现钩子：虚拟化容器在 measure 之前按上一帧视口增删子节点（参 §Ⅳ-bis）。
        self.realize_dirty(engine);

        // 3. 布局。弹层位置依赖锚点矩形 —— 有弹层时每帧重排覆盖层（弹层子树本身有缓存，代价只是重算槽位）。
        // 用 quiet 版：结果在本帧 arrange 中被消费，不应计为「帧内新失效」而多出一帧。
        if !self.popups.is_empty() {
            self.invalidate_arrange_quiet(self.overlay);
        }
        let full = Rect::new(0.0, 0.0, size.width, size.height);
        self.measure_node(self.root, size, engine);
        self.arrange_node(self.root, full, engine);
        let bounds = self.popup_bounds();
        self.measure_node(self.overlay, bounds.size, engine);
        self.arrange_node(self.overlay, bounds, engine);
        self.validate_focus();
        if cfg!(debug_assertions) {
            self.check_containment();
        }

        // 4. 绘制脏节点：只处理待绘集合（不再每帧递归整棵树找 needs_paint）。
        // 集合里的悬垂 id（已删除）与不可见节点在 paint_node 内跳过。
        for id in std::mem::take(&mut self.paint_queue) {
            self.paint_node(id, engine);
        }

        // 5. 拼接。剔除条件 = 表面 ∩ 本帧 damage（局部帧只拼与 damage 相交的节点；
        // 全量帧退化为表面）。与 damage 不相交的命令光栅器本就裁掉 / 不画，跳过安全。
        // 参 ELEMENT_TREE「compose 剔除」。
        let frame_damage = if self.full_repaint { None } else { self.damage };
        let surface = Rect::new(0.0, 0.0, size.width, size.height);
        let cull = match frame_damage {
            Some(d) if self.damage_cull => d.intersect(surface),
            _ => Some(surface),
        };
        let mut scene = Scene::default();
        self.compose(self.root, &mut scene, cull);
        // 遮罩：任一打开的弹层要求时，在覆盖层之前整面压暗（弹层自身画在遮罩之上）。
        // 分离模式下弹层在别的表面上，主表面不压暗也不画覆盖层。
        if !self.detached_popups && self.popups.iter().any(|(_, s)| s.scrim) {
            scene.fill_rect(
                self.theme.overlay_color,
                Rect::new(0.0, 0.0, size.width, size.height),
            );
        }
        if !self.detached_popups {
            self.compose(self.overlay, &mut scene, cull);
        }

        // 5b. 图层：逐个拼子树内容，变了才产出 Content（内容只在真变化时重新提交）。
        self.sync_layers();

        // 6. 焦点控件的 IME 上下文（用本帧布局与排版）。
        self.ime = self.focus.and_then(|f| {
            let n = self.node(f)?;
            n.widget.as_ref()?.ime(n.rect, &self.theme, engine)
        });

        let full = self.full_repaint;
        self.full_repaint = false;
        self.damage = None;
        // 帧内产生过失效（布局改矩形 / 实现钩子增删 / 绘制期登记动画等）→ 留给下一帧；
        // 否则本帧工作已消费干净，可以进入空闲。
        self.dirty = self.invalidate_epoch != epoch0;
        FrameOutput {
            scene,
            damage: frame_damage,
            full,
            animating: !self.anim.is_empty(),
        }
    }

    pub(crate) fn measure_node(
        &mut self,
        id: WidgetId,
        available: Size,
        engine: &TextEngine,
    ) -> Size {
        let available = sanitize(available);
        let Some(n) = self.node(id) else {
            return Size::ZERO;
        };
        if !n.props.visible {
            return Size::ZERO;
        }
        if !n.flags.needs_measure && n.last_available == Some(available) {
            return n.desired;
        }
        let props = n.props;
        let inner = props.inner_available(available);
        // 先清 needs_measure（并记录约束）：measure 期内控件若再 `invalidate_measure`，
        // 标记会保留到下一帧，不会被本帧末尾清掉（参 ELEMENT_TREE §帧调度）。
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_measure = false;
            n.last_available = Some(available);
        }
        let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) else {
            return Size::ZERO;
        };
        let mut ctx = MeasureCtx {
            tree: self,
            id,
            engine,
        };
        let content = sanitize(w.measure(&mut ctx, inner));
        let content = props.clamp_size(content);
        let mut desired = props.margin.grow(content);
        // 期望尺寸不超出可用约束（无界轴除外）。
        if available.width.is_finite() {
            desired.width = desired.width.min(available.width);
        }
        if available.height.is_finite() {
            desired.height = desired.height.min(available.height);
        }
        if let Some(n) = self.node_mut(id) {
            n.widget = Some(w);
            n.desired = desired;
        }
        desired
    }

    pub(crate) fn arrange_node(&mut self, id: WidgetId, slot: Rect, engine: &TextEngine) {
        let slot = slot.normalized();
        let Some(n) = self.node(id) else { return };
        if !n.props.visible {
            return;
        }
        let props = n.props;
        let desired = n.desired;
        let inner_slot = props.margin.deflate(slot);
        let content_desired = props.margin.shrink(desired);
        let (x, w) = resolve_axis(
            inner_slot.origin.x,
            inner_slot.size.width,
            content_desired.width,
            props.width,
            props.min.width,
            props.max.width,
            props.h_align,
        );
        let (y, h) = resolve_axis(
            inner_slot.origin.y,
            inner_slot.size.height,
            content_desired.height,
            props.height,
            props.min.height,
            props.max.height,
            props.v_align,
        );
        let rect = Rect::new(x, y, w, h);
        let (rect_changed, arrange_pending) = {
            let Some(n) = self.node_mut(id) else { return };
            (n.rect != rect, n.flags.needs_arrange)
        };
        if !rect_changed && !arrange_pending {
            return;
        }
        if let Some(n) = self.node_mut(id) {
            n.rect = rect;
            n.flags.needs_arrange = false;
        }
        // 矩形变化 → 内容位置变，需重画。quiet：本帧 paint 步会消费，不应多出一帧。
        if rect_changed {
            self.invalidate_paint_quiet(id);
        }
        let Some(mut wdg) = self.node_mut(id).and_then(|n| n.widget.take()) else {
            return;
        };
        let mut ctx = ArrangeCtx {
            tree: self,
            id,
            engine,
        };
        wdg.arrange(&mut ctx, rect);
        if let Some(n) = self.node_mut(id) {
            n.widget = Some(wdg);
        }
    }

    /// 绘制单个脏节点（不再递归：待绘集合已由 `invalidate_paint` 等维护）。
    fn paint_node(&mut self, id: WidgetId, engine: &TextEngine) {
        let Some(n) = self.node(id) else { return };
        if !n.flags.needs_paint {
            return;
        }
        // 不可见（自身或祖先）不画：保留 needs_paint，释放队列位，待重新可见时再入队。
        if !self.effectively_visible(id) {
            if let Some(n) = self.node_mut(id) {
                n.flags.paint_queued = false;
            }
            return;
        }
        let rect = n.rect;
        let state = self.states(id);
        // 先清 needs_paint / 去重位：绘制期内控件若再 `invalidate_paint`（如动画推进），
        // 会重新入队并留到下一帧，不会被本帧末尾吞掉（参 ELEMENT_TREE §帧调度）。
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_paint = false;
            n.flags.paint_queued = false;
        }
        let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) else {
            // 控件正被回调持有（理论上不会出现在此）：留下次再画。
            return;
        };
        let mut scene = Scene::default();
        let mut ctx = PaintCtx {
            tree: self,
            id,
            engine,
            rect,
            state,
        };
        w.paint(&mut ctx, &mut scene);
        let mut after = Scene::default();
        w.paint_after(&mut ctx, &mut after);
        // 焦点视觉画在节点外一圈，一并计入绘制范围。
        let mut bounds = w.paint_overflow().inflate(rect);
        if w.focusable() && w.focus_visual() {
            bounds = crate::props::Insets::all(FOCUS_VISUAL_THICKNESS).inflate(bounds);
        }
        let old = self.node(id).and_then(|n| n.painted_bounds);
        if let Some(o) = old {
            self.add_damage(o);
        }
        self.add_damage(bounds);
        if let Some(n) = self.node_mut(id) {
            n.widget = Some(w);
            n.paint = scene.commands;
            n.paint_after = after.commands;
            n.painted_bounds = Some(bounds);
            // 不在此清 needs_paint / paint_queued —— 绘制期新产生的失效已在上面入队。
        }
    }

    /// 拼接 `id` 子树到 Scene。`cull` = 有效裁剪矩形（`None` = 不剔除，弹层独立 Surface 用）；
    /// 节点绘制范围（`painted_bounds`，含 overflow）与 `cull` 不相交 → **整棵子树跳过**
    /// （含 `paint_after` 与焦点视觉）。进入裁剪子容器时与容器矩形求交，随深度收窄。
    /// 参 ELEMENT_TREE「compose 剔除」。
    fn compose(&self, id: WidgetId, scene: &mut Scene, cull: Option<Rect>) {
        let Some(n) = self.node(id) else { return };
        if !n.props.visible {
            return;
        }
        // 图层子树由合成器子表面承载，不进入外层场景（拼该图层自身内容时除外）。参 layer.rs。
        if self.layers.contains_key(&id) && self.compose_layer.get() != Some(id) {
            return;
        }
        // 自身绘制范围；从未绘制过的节点退化为其矩形（容器）以避免误裁子树。
        let bounds = n.painted_bounds.unwrap_or(n.rect);
        if let Some(c) = cull
            && bounds.intersect(c).is_none()
        {
            return;
        }
        scene.commands.extend(n.paint.iter().cloned());
        if !n.children.is_empty() {
            let clip = n.widget.as_ref().is_none_or(|w| w.clips_children());
            if clip {
                scene.push_clip(n.rect);
            }
            // 裁剪子容器收窄剔除；非裁剪容器沿用当前裁剪（绘制仍受祖先约束）。
            let child_cull = if clip {
                match cull {
                    Some(c) => c.intersect(n.rect),
                    None => None,
                }
            } else {
                cull
            };
            for &c in &n.children {
                self.compose(c, scene, child_cull);
            }
            if clip {
                scene.pop_clip();
            }
        }
        scene.commands.extend(n.paint_after.iter().cloned());
        // 键盘焦点视觉：框架统一绘制（外 2px 焦点色 + 内 1px 对比色，正典 §Ⅳ），
        // 画在子树之上。只在键盘交互后显示（指针点击不画，`focus_keyboard`）。
        if self.focus == Some(id)
            && self.focus_keyboard
            && n.widget.as_ref().is_some_and(|w| w.focus_visual())
        {
            scene.stroke_rect(
                self.theme.colors.focus_stroke,
                n.rect,
                kanesumi_core::interaction::FOCUS_OUTER_PX,
            );
            let inner = n.rect.inset(
                kanesumi_core::interaction::FOCUS_OUTER_PX,
                kanesumi_core::interaction::FOCUS_OUTER_PX,
                kanesumi_core::interaction::FOCUS_OUTER_PX,
                kanesumi_core::interaction::FOCUS_OUTER_PX,
            );
            scene.stroke_rect(
                kanesumi_core::interaction::focus_inner_color(self.theme.scheme),
                inner,
                kanesumi_core::interaction::FOCUS_INNER_PX,
            );
        }
    }

    /// 调试断言：可见节点矩形必须落在父矩形内、非 NaN。违反只记错误日志（不 panic）。
    fn check_containment(&self) {
        for (slot, n) in self.nodes.iter().enumerate() {
            let Some(n) = n else { continue };
            let r = n.rect;
            let finite = [r.origin.x, r.origin.y, r.size.width, r.size.height]
                .iter()
                .all(|v| v.is_finite());
            if !finite {
                log::error!("kanesumi-element: 节点 {slot} 矩形非有限 {r:?}");
                continue;
            }
            let Some(p) = n.parent.and_then(|p| self.node(p)) else {
                continue;
            };
            if p.widget.as_ref().is_some_and(|w| w.scrolls_children()) {
                continue;
            }
            if n.props.visible && p.props.visible && !rect_within(r, p.rect) {
                log::error!(
                    "kanesumi-element: 节点 {slot} 越出父矩形 {r:?} ⊄ {:?}",
                    p.rect
                );
            }
        }
    }

    // ── 命中与输入 ──────────────────────────────────────────────────────────

    /// 命中测试：覆盖层优先，其次内容；后画者优先；沿祖先裁剪链。
    pub fn hit(&self, pos: Point) -> Option<WidgetId> {
        self.hit_rec(self.overlay, pos, None)
            .or_else(|| self.hit_rec(self.root, pos, None))
    }

    fn hit_rec(&self, id: WidgetId, pos: Point, clip: Option<Rect>) -> Option<WidgetId> {
        let n = self.node(id)?;
        if !n.props.visible {
            return None;
        }
        if let Some(c) = clip
            && !c.contains(pos)
        {
            return None;
        }
        let w = n.widget.as_ref()?;
        let child_clip = if w.clips_children() {
            match clip {
                Some(c) => Some(c.intersect(n.rect)?),
                None => Some(n.rect),
            }
        } else {
            clip
        };
        for &c in n.children.iter().rev() {
            if let Some(h) = self.hit_rec(c, pos, child_clip) {
                return Some(h);
            }
        }
        w.hit_test(n.rect, pos).then_some(id)
    }

    /// 投递事件（冒泡事件沿祖先链，禁用节点跳过）。返回是否被处理。
    fn deliver(&mut self, target: WidgetId, event: &Event) -> bool {
        let path = if event.bubbles() {
            self.ancestors_inclusive(target)
        } else {
            vec![target]
        };
        for id in path {
            if self.effectively_disabled(id) {
                continue;
            }
            let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) else {
                continue;
            };
            let mut ctx = EventCtx {
                tree: self,
                id,
                handled: false,
            };
            w.event(&mut ctx, event);
            let handled = ctx.handled;
            if let Some(n) = self.node_mut(id) {
                n.widget = Some(w);
            }
            if handled {
                return true;
            }
        }
        false
    }

    fn set_state(&mut self, id: WidgetId, f: impl FnOnce(&mut ControlStates)) {
        let Some(n) = self.node_mut(id) else { return };
        let before = n.state;
        f(&mut n.state);
        if n.state != before && n.widget.as_ref().is_some_and(|w| w.has_visual_states()) {
            self.invalidate_paint(id);
        }
    }

    /// 指针下的交互目标：自命中节点向上第一个有视觉状态的节点。
    fn interactive_ancestor(&self, id: WidgetId) -> Option<WidgetId> {
        self.ancestors_inclusive(id).into_iter().find(|a| {
            self.node(*a)
                .and_then(|n| n.widget.as_ref())
                .is_some_and(|w| w.has_visual_states())
        })
    }

    /// 更新悬停链并刷新提示目标。`moved` = 指针位置较上次确有变化（提示要求指针静止
    /// 满延迟，移动会重置计时）。
    fn update_hover(&mut self, pos: Option<Point>, moved: bool) {
        let mut chain = pos
            .and_then(|p| self.hit(p))
            .map(|t| self.ancestors_inclusive(t))
            .unwrap_or_default();
        chain.reverse(); // 根 → 叶
        let old = std::mem::take(&mut self.hover_chain);
        for id in old.iter().rev() {
            if !chain.contains(id) {
                self.set_state(*id, |s| s.hovered = false);
                self.deliver(*id, &Event::PointerLeave);
            }
        }
        for id in &chain {
            if !old.contains(id) {
                self.set_state(*id, |s| s.hovered = true);
                self.deliver(*id, &Event::PointerEnter);
            }
        }
        self.hover_chain = chain;
        self.refresh_tooltip(moved);
    }

    /// 最近一次指针位置（首次指针事件前为 None）。
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    pub fn pointer_move(&mut self, pos: Point) {
        let moved = self.pointer != Some(pos);
        self.pointer = Some(pos);
        self.update_hover(Some(pos), moved);
        // 按下期间，「按下」外观跟随指针是否仍在交互目标上（UWP Button 移出即恢复）。
        if let Some(p) = self.press_target {
            let inside = self
                .hit(pos)
                .is_some_and(|h| self.is_ancestor_or_self(p, h));
            self.set_state(p, |s| s.pressed = inside);
        }
        if let Some(t) = self.captured.or_else(|| self.hit(pos)) {
            self.deliver(t, &Event::PointerMove { pos });
        }
    }

    pub fn pointer_down(&mut self, pos: Point, button: PointerButton, modifiers: Modifiers) {
        self.pointer = Some(pos);
        // 按下即消失：提示在指针按下时立刻收起（正典 §Ⅲ，UWP 行为）。
        self.hide_tooltip();
        // 覆盖层有弹层时：点在所有弹层之外 → LightDismiss（模态则只吞不关）。
        // 提示层是 passthrough 弹层，不参与顶层判定（否则会把点击吞掉）。
        if let Some((top, spec)) = self.top_popup() {
            let inside = self.hit_rec(self.overlay, pos, None).is_some();
            if !inside {
                if spec.light_dismiss && !spec.modal {
                    self.dismiss_popup(top);
                }
                self.mark_dirty();
                return;
            }
        }
        let Some(target) = self.hit(pos) else { return };
        self.captured = Some(target);
        if let Some(f) = self
            .ancestors_inclusive(target)
            .into_iter()
            .find(|a| self.is_focusable(*a))
        {
            self.set_focus(Some(f), false);
        }
        if button == PointerButton::Left
            && let Some(p) = self.interactive_ancestor(target)
            && !self.effectively_disabled(p)
        {
            self.press_target = Some(p);
            self.set_state(p, |s| s.pressed = true);
        }
        if button == PointerButton::Right {
            self.context_target = Some(target);
        }
        self.deliver(
            target,
            &Event::PointerDown {
                pos,
                button,
                modifiers,
            },
        );
        if button == PointerButton::Right {
            self.deliver(target, &Event::ContextRequested { pos });
        }
    }

    /// 双击：外壳在第二次按下之后调用。投给按下时的命中目标（捕获者）或当前命中。
    pub fn pointer_double(&mut self, pos: Point, button: PointerButton) {
        if let Some(t) = self.captured.or_else(|| self.hit(pos)) {
            self.deliver(t, &Event::DoubleTapped { pos, button });
        }
    }

    pub fn pointer_up(&mut self, pos: Point, button: PointerButton, modifiers: Modifiers) {
        self.pointer = Some(pos);
        if let Some(t) = self.captured.take().or_else(|| self.hit(pos)) {
            self.deliver(
                t,
                &Event::PointerUp {
                    pos,
                    button,
                    modifiers,
                },
            );
        }
        if button == PointerButton::Left
            && let Some(p) = self.press_target.take()
        {
            self.set_state(p, |s| s.pressed = false);
            let inside = self
                .hit(pos)
                .is_some_and(|h| self.is_ancestor_or_self(p, h));
            if inside && self.contains(p) {
                self.deliver(p, &Event::Click);
            }
        }
        self.update_hover(Some(pos), false);
    }

    pub fn pointer_leave(&mut self) {
        if self.captured.is_none() {
            self.update_hover(None, false);
        }
    }

    pub fn scroll(&mut self, pos: Point, dx: f32, dy: f32, modifiers: Modifiers) {
        if let Some(t) = self.hit(pos) {
            self.deliver(t, &Event::Scroll { dx, dy, modifiers });
        }
    }

    /// 键按下。返回是否被消费（未消费的键外壳可另作他用）。
    pub fn key_down(&mut self, key: Key, modifiers: Modifiers) -> bool {
        // 键盘输入即收起提示（UWP：键盘交互优先于提示）。
        self.hide_tooltip();
        let handled = match self.focus {
            Some(f) => self.deliver(f, &Event::KeyDown { key, modifiers }),
            None => false,
        };
        if handled {
            return true;
        }
        match key {
            Key::Tab => self.focus_next(modifiers.shift),
            Key::Escape => {
                if let Some((top, spec)) = self.top_popup()
                    && spec.light_dismiss
                {
                    self.dismiss_popup(top);
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    pub fn preedit(&mut self, text: String, cursor_byte: Option<usize>) -> bool {
        self.deliver_focus(Event::Preedit { text, cursor_byte })
    }

    pub fn commit(&mut self, text: String) -> bool {
        self.deliver_focus(Event::Commit { text })
    }

    pub fn delete_surrounding(&mut self, before_bytes: u32, after_bytes: u32) -> bool {
        self.deliver_focus(Event::DeleteSurrounding {
            before_bytes,
            after_bytes,
        })
    }

    fn deliver_focus(&mut self, e: Event) -> bool {
        match self.focus {
            Some(f) => self.deliver(f, &e),
            None => false,
        }
    }

    /// 表面矩形（逻辑坐标，原点 0,0）。
    pub fn surface(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.width, self.size.height)
    }

    /// 弹层放置区（树坐标）：弹层定位、级联子菜单翻转、对话框居中都以它为界。
    /// 默认 = 表面；外壳经 `set_popup_bounds` 放大（如整个输出）。
    pub fn popup_bounds(&self) -> Rect {
        self.popup_bounds.unwrap_or_else(|| self.surface())
    }

    /// 设置弹层放置区。`None` 恢复为表面。
    pub fn set_popup_bounds(&mut self, bounds: Option<Rect>) {
        if self.popup_bounds != bounds {
            self.popup_bounds = bounds;
            self.invalidate_measure(self.overlay);
            self.full_repaint = true;
            self.mark_dirty();
        }
    }

    /// 弹层分离开关（见字段说明）。
    pub fn set_detached_popups(&mut self, on: bool) {
        if self.detached_popups != on {
            self.detached_popups = on;
            self.full_repaint = true;
            self.mark_dirty();
        }
    }

    pub fn detached_popups(&self) -> bool {
        self.detached_popups
    }

    /// 弹层规格（外壳据 `light_dismiss` 决定是否 `xdg_popup.grab`）。
    pub fn popup_spec(&self, id: WidgetId) -> Option<PopupSpec> {
        self.popups.iter().find(|(p, _)| *p == id).map(|(_, s)| *s)
    }

    /// 弹层子树的绘制范围（树坐标）：节点矩形 ∪ 子树全部 `painted_bounds`。
    /// 级联子菜单画在弹层节点之外，承载表面要按这个范围开。首帧前为节点矩形。
    pub fn popup_extent(&self, id: WidgetId) -> Option<Rect> {
        let mut r = self.rect(id)?;
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            let Some(node) = self.node(n) else { continue };
            if !node.props.visible {
                continue;
            }
            if let Some(b) = node.painted_bounds {
                r = union(r, b);
            }
            stack.extend(node.children.iter().copied());
        }
        Some(r)
    }

    /// 单个弹层的 Scene，平移到以 `popup_extent` 左上为原点（外壳直接画进弹层表面）。
    pub fn popup_scene(&self, id: WidgetId) -> Option<Scene> {
        let extent = self.popup_extent(id)?;
        let mut scene = Scene::default();
        // 弹层独立表面：不按 damage 剔除（整幅都要画）。
        self.compose(id, &mut scene, None);
        scene.translate(Point::new(-extent.origin.x, -extent.origin.y));
        Some(scene)
    }

    /// 上一帧的排版引擎（首帧之前为 None）。
    pub fn engine(&self) -> Option<&TextEngine> {
        self.engine.as_ref()
    }

    /// 右键按下时的命中元素（上下文菜单只能作用于它）。
    pub fn context_target(&self) -> Option<WidgetId> {
        self.context_target
    }

    // ── 焦点 ────────────────────────────────────────────────────────────────

    fn is_focusable(&self, id: WidgetId) -> bool {
        self.node(id)
            .and_then(|n| n.widget.as_ref())
            .is_some_and(|w| w.focusable())
            && self.effectively_visible(id)
            && !self.effectively_disabled(id)
    }

    pub fn focused(&self) -> Option<WidgetId> {
        self.focus
    }

    /// 程序化聚焦。不可聚焦 → 忽略并返回 false。
    pub fn focus(&mut self, id: WidgetId, keyboard: bool) -> bool {
        if !self.is_focusable(id) {
            return false;
        }
        self.set_focus(Some(id), keyboard);
        true
    }

    fn set_focus(&mut self, new: Option<WidgetId>, keyboard: bool) {
        let old = self.focus;
        if old == new {
            if self.focus_keyboard != keyboard {
                self.focus_keyboard = keyboard;
                if let Some(id) = new {
                    self.set_state(id, |s| s.keyboard_focused = keyboard);
                    self.invalidate_paint(id);
                }
                self.refresh_tooltip(false);
            }
            return;
        }
        if let Some(o) = old {
            self.set_state(o, |s| {
                s.focused = false;
                s.keyboard_focused = false;
            });
            self.invalidate_paint(o);
            self.deliver(o, &Event::FocusOut);
        }
        self.focus = new;
        self.focus_keyboard = keyboard;
        if let Some(n) = new {
            self.set_state(n, |s| {
                s.focused = true;
                s.keyboard_focused = keyboard;
            });
            self.invalidate_paint(n);
            self.deliver(n, &Event::FocusIn { keyboard });
            if keyboard {
                self.bring_into_view(n);
            }
        }
        // 焦点变化会改变提示目标：指针悬停优先，其次键盘焦点（正典 §Ⅲ，UWP 行为）。
        self.refresh_tooltip(false);
    }

    /// 请祖先滚动容器把 `id` 滚进视口（由近及远，嵌套滚动逐层处理）。
    pub fn bring_into_view(&mut self, id: WidgetId) {
        let Some(target) = self.rect(id) else { return };
        for anc in self.ancestors_inclusive(id).into_iter().skip(1) {
            let Some(mut w) = self.node_mut(anc).and_then(|n| n.widget.take()) else {
                continue;
            };
            let mut ctx = EventCtx {
                tree: self,
                id: anc,
                handled: false,
            };
            w.bring_into_view(&mut ctx, target);
            if let Some(n) = self.node_mut(anc) {
                n.widget = Some(w);
            }
        }
    }

    /// 某节点是否按滚动偏移排布子节点（`Widget::scrolls_children`）。
    pub fn scrolls_children(&self, id: WidgetId) -> bool {
        self.node(id)
            .and_then(|n| n.widget.as_ref())
            .is_some_and(|w| w.scrolls_children())
    }

    /// Tab 顺序：树先序中可聚焦的节点。顶层弹层为模态时限定在该弹层内（焦点陷阱）。
    pub fn focus_order(&self) -> Vec<WidgetId> {
        let scope = match self.top_popup() {
            Some((top, spec)) if spec.modal => vec![top],
            Some((top, _)) => vec![top, self.root],
            None => vec![self.root],
        };
        let mut out = Vec::new();
        for s in scope {
            self.collect_focusable(s, &mut out);
        }
        out
    }

    fn collect_focusable(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        let Some(n) = self.node(id) else { return };
        if !n.props.visible || n.flags.disabled {
            return;
        }
        if n.widget.as_ref().is_some_and(|w| w.focusable()) {
            out.push(id);
        }
        for &c in &n.children {
            self.collect_focusable(c, out);
        }
    }

    /// Tab / Shift+Tab。返回是否有节点获得焦点。
    pub fn focus_next(&mut self, backward: bool) -> bool {
        let order = self.focus_order();
        if order.is_empty() {
            self.set_focus(None, false);
            return false;
        }
        let n = order.len();
        let next = match self.focus.and_then(|f| order.iter().position(|o| *o == f)) {
            Some(i) if backward => order[(i + n - 1) % n],
            Some(i) => order[(i + 1) % n],
            None if backward => order[n - 1],
            None => order[0],
        };
        self.set_focus(Some(next), true);
        true
    }

    /// 焦点节点被隐藏 / 禁用 / 删除后清除焦点。
    fn validate_focus(&mut self) {
        if let Some(f) = self.focus
            && !self.is_focusable(f)
        {
            self.set_focus(None, false);
        }
    }

    /// 焦点控件的 IME 上下文（上一帧计算）。`None` = 无文本输入焦点。
    pub fn ime_context(&self) -> Option<&ImeContext> {
        self.ime.as_ref()
    }

    // ── 覆盖层 ──────────────────────────────────────────────────────────────

    /// 打开弹层：挂到覆盖层根。返回弹层节点 id。
    pub fn open_popup(&mut self, widget: impl Widget, spec: PopupSpec) -> WidgetId {
        if spec.scrim {
            self.full_repaint = true;
        }
        let id = self.insert(self.overlay, widget);
        self.popups.push((id, spec));
        id
    }

    /// 关闭弹层。锚点收到 `Event::PopupClosed`；若焦点原在弹层内，交还锚点
    /// （XAML Flyout 关闭后焦点回到触发器，键盘用户不会「掉焦点」）。
    pub fn close_popup(&mut self, id: WidgetId) {
        // 提示弹层被任意路径关闭：同步清掉提示状态，防止悬垂 id。
        if self.tooltip_popup == Some(id) {
            self.tooltip_popup = None;
            self.tooltip_anchor = None;
            self.tooltip_hide = None;
        }
        let Some(pos) = self.popups.iter().position(|(p, _)| *p == id) else {
            return;
        };
        let spec = self.popups[pos].1;
        if spec.scrim {
            self.full_repaint = true;
        }
        let focus_inside = self.focus.is_some_and(|f| self.is_ancestor_or_self(id, f));
        let keyboard = self.focus_keyboard;
        self.remove(id);
        if let Some(anchor) = spec.anchor.filter(|a| self.contains(*a)) {
            if focus_inside {
                self.focus(anchor, keyboard);
            }
            self.deliver(anchor, &Event::PopupClosed { popup: id });
        }
    }

    fn dismiss_popup(&mut self, id: WidgetId) {
        self.push_action(id, Box::new(PopupDismissed));
        self.close_popup(id);
    }

    /// 关闭全部可轻触关闭的弹层（表面失去键盘焦点时，避免「失焦残留」，
    /// 参 CONTEXT_MENU_SPEC §Ⅵ）。模态弹层保留。
    pub fn dismiss_popups(&mut self) {
        // 表面失焦：提示一并收起（不随其他弹层的 light_dismiss 规则）。
        self.hide_tooltip();
        let doomed: Vec<WidgetId> = self
            .popups
            .iter()
            .filter(|(_, s)| s.light_dismiss && !s.modal)
            .map(|(p, _)| *p)
            .collect();
        for p in doomed.into_iter().rev() {
            self.dismiss_popup(p);
        }
    }

    pub fn popups(&self) -> impl Iterator<Item = WidgetId> + '_ {
        self.popups.iter().map(|(p, _)| *p)
    }

    /// 最上层的**参与输入**的弹层（跳过提示这类 `passthrough` 弹层）。轻触关闭 / Esc /
    /// 焦点陷阱 / Tab 范围都以它为准，提示因此不会吃掉点击或抢焦点。
    fn top_popup(&self) -> Option<(WidgetId, PopupSpec)> {
        self.popups
            .iter()
            .rev()
            .find(|(_, s)| !s.passthrough)
            .map(|(p, s)| (*p, *s))
    }

    // ── 工具提示（`set_tooltip`；计时 / 挂载由框架负责）──────────────────────

    /// 给任意元素挂提示文字（空串 = 清除）。指针静止 / 键盘焦点停留满
    /// `TOOLTIP_DELAY_MS` 后由框架在锚点下方（空间不足时上方）显示，再现延迟
    /// `TOOLTIP_RESHOW_MS`、`TOOLTIP_HIDE_MS` 后自动消失，指针离开 / 按下即收起。
    /// 提示不抢焦点、不吃输入（`PopupSpec::passthrough`）。参 INTERACTION_CANON §Ⅲ。
    pub fn set_tooltip(&mut self, id: WidgetId, text: impl Into<String>) {
        let text = text.into();
        if let Some(n) = self.node_mut(id) {
            if text.is_empty() {
                n.tooltip = None;
            } else {
                n.tooltip = Some(text);
            }
        }
        // 目标可能正在显示旧提示：立即按新文字刷新（重开弹层代价可接受，提示低频率）。
        if self.tooltip_anchor == Some(id) {
            self.hide_tooltip();
        }
        self.refresh_tooltip(false);
    }

    /// 清除某元素的提示文字。
    pub fn clear_tooltip(&mut self, id: WidgetId) {
        self.set_tooltip(id, String::new());
    }

    /// 某元素的提示文字。
    pub fn tooltip_text(&self, id: WidgetId) -> Option<&str> {
        self.node(id)?.tooltip.as_deref()
    }

    /// 正在显示的提示弹层节点 id（测试 / 外壳查询用）。
    pub fn tooltip_popup(&self) -> Option<WidgetId> {
        self.tooltip_popup
    }

    /// 提示文字非空且节点可见、未禁用 → 返回其文字。
    fn has_tooltip(&self, id: WidgetId) -> bool {
        self.node(id)
            .and_then(|n| n.tooltip.as_deref())
            .is_some_and(|t| !t.is_empty())
            && self.effectively_visible(id)
            && !self.effectively_disabled(id)
    }

    /// 当前提示目标：指针悬停链最深处有提示的节点优先，其次键盘焦点链（含自身）。
    /// 模态弹层打开时不提示（提示画在覆盖层会盖住模态面板）。
    fn tooltip_candidate(&self) -> Option<WidgetId> {
        if self.popups.iter().any(|(_, s)| s.modal) {
            return None;
        }
        if let Some(t) = self
            .hover_chain
            .iter()
            .rev()
            .find(|id| self.has_tooltip(**id))
        {
            return Some(*t);
        }
        let f = self.focus?;
        self.ancestors_inclusive(f)
            .into_iter()
            .find(|id| self.has_tooltip(*id))
    }

    fn tooltip_delay_ms(reshow: bool) -> f64 {
        let ms = if reshow {
            kanesumi_core::interaction::TOOLTIP_RESHOW_MS
        } else {
            kanesumi_core::interaction::TOOLTIP_DELAY_MS
        };
        ms as f64 / 1000.0
    }

    fn arm_tooltip(&mut self, target: WidgetId, reshow: bool) {
        self.tooltip_pending = Some(target);
        self.tooltip_delay = Some(Self::tooltip_delay_ms(reshow));
    }

    /// 悬停 / 焦点变化后重算提示目标与计时。`moved` = 指针移动过（重置静止计时）。
    fn refresh_tooltip(&mut self, moved: bool) {
        // 指针按下期间不开始新提示（捕获中），避免「按下又冒出来」。
        if self.captured.is_some() {
            return;
        }
        let target = self.tooltip_candidate();
        match (self.tooltip_anchor, target) {
            (Some(a), Some(t)) if a == t => {}
            // 已在显示、目标改变：收起旧提示，对相邻目标用再现延迟（100 ms）。
            (Some(_), Some(t)) => {
                self.hide_tooltip();
                self.arm_tooltip(t, true);
            }
            (Some(_), None) => self.hide_tooltip(),
            (None, Some(t)) => {
                if self.tooltip_pending == Some(t) {
                    if moved {
                        // 指针又动了 —— 静止计时从头来（正典 §Ⅲ）。
                        self.tooltip_delay = Some(Self::tooltip_delay_ms(false));
                    }
                } else {
                    self.arm_tooltip(t, false);
                }
            }
            (None, None) => {
                self.tooltip_pending = None;
                self.tooltip_delay = None;
            }
        }
    }

    /// 显示当前 `tooltip_pending` 的提示。计时到期时由 `tick_timers` 调用。
    fn show_tooltip(&mut self) {
        let Some(target) = self.tooltip_pending else {
            return;
        };
        self.tooltip_pending = None;
        self.tooltip_delay = None;
        if !self.has_tooltip(target) {
            return;
        }
        let text = self
            .node(target)
            .and_then(|n| n.tooltip.clone())
            .unwrap_or_default();
        // 顶栏 / Dock 等矮表面：外壳放大 `popup_bounds` 并开 xdg_popup，放置自动适用。
        let spec = PopupSpec {
            anchor: Some(target),
            side: PopupSide::Bottom,
            gap: TOOLTIP_GAP,
            light_dismiss: false,
            modal: false,
            scrim: false,
            passthrough: true,
            ..PopupSpec::default()
        };
        let id = self.open_popup(Tooltip::new(text), spec);
        self.tooltip_anchor = Some(target);
        self.tooltip_popup = Some(id);
        self.tooltip_hide = Some(kanesumi_core::interaction::TOOLTIP_HIDE_MS as f64 / 1000.0);
    }

    /// 立即收起提示（不影响其他弹层）。
    fn hide_tooltip(&mut self) {
        self.tooltip_pending = None;
        self.tooltip_delay = None;
        self.tooltip_hide = None;
        self.tooltip_anchor = None;
        if let Some(p) = self.tooltip_popup.take()
            && self.contains(p)
        {
            self.close_popup(p);
        }
    }

    /// 推进提示计时（与控件定时器一同在 `tick_timers` 里走，等待期不占帧）。
    fn tick_tooltip(&mut self, dt: f64) {
        if let Some(d) = self.tooltip_delay.as_mut() {
            *d -= dt;
            if *d <= 0.0 {
                self.show_tooltip();
            }
            return;
        }
        if let Some(h) = self.tooltip_hide.as_mut() {
            *h -= dt;
            if *h <= 0.0 {
                self.hide_tooltip();
            }
        }
    }

    pub(crate) fn popup_slot(&self, popup: WidgetId, desired: Size, bounds: Rect) -> Rect {
        let spec = self
            .popups
            .iter()
            .find(|(p, _)| *p == popup)
            .map(|(_, s)| *s)
            .unwrap_or_default();
        let w = desired.width.min(bounds.size.width);
        let h = desired.height.min(bounds.size.height);
        let anchor = spec.anchor.and_then(|a| self.rect(a));
        let (x, y) = match (spec.at, anchor) {
            // 右键菜单：点锚定（CONTEXT_MENU_SPEC §Ⅲ，同 `place_context_menu`）。
            (Some(p), _) => (
                if p.x + w > bounds.right() {
                    p.x - w
                } else {
                    p.x
                },
                if p.y + h > bounds.bottom() {
                    p.y - h
                } else {
                    p.y
                },
            ),
            (None, Some(_)) | (None, None) if spec.side == PopupSide::Center => (
                bounds.origin.x + (bounds.size.width - w) / 2.0,
                bounds.origin.y + (bounds.size.height - h) / 2.0,
            ),
            (None, Some(a)) => place_on_side(a, w, h, spec, bounds),
            (None, None) => (
                bounds.origin.x + (bounds.size.width - w) / 2.0,
                bounds.origin.y + (bounds.size.height - h) / 2.0,
            ),
        };
        // 夹进表面：面板不得画到屏幕外（ROADMAP §Ⅴ-2 place_popup 同款契约）。
        let x = x.clamp(bounds.origin.x, (bounds.right() - w).max(bounds.origin.x));
        let y = y.clamp(bounds.origin.y, (bounds.bottom() - h).max(bounds.origin.y));
        Rect::new(x, y, w, h)
    }

    // ── 测试 / 诊断 ─────────────────────────────────────────────────────────

    /// 全部存活节点 id（先序，内容根在前、覆盖层在后）。
    pub fn all_ids(&self) -> Vec<WidgetId> {
        let mut out = Vec::new();
        for r in [self.root, self.overlay] {
            let mut stack = vec![r];
            while let Some(c) = stack.pop() {
                out.push(c);
                let mut ch = self.children(c).to_vec();
                ch.reverse();
                stack.extend(ch);
            }
        }
        out
    }

    // ── 图层（G3-c）。参 layer.rs ─────────────────────────────────────────────

    /// 把节点标为 / 取消图层。图层子树改由独立的合成器子表面承载，主场景整幅重拼一次。
    pub fn set_layer(&mut self, id: WidgetId, on: bool) {
        if on {
            self.layers.entry(id).or_default();
        } else if let Some(st) = self.layers.remove(&id)
            && st.created
        {
            self.layer_ops.push(LayerOp::Remove { id });
        }
        self.full_repaint = true;
        self.invalidate_paint(id);
    }

    pub fn is_layer(&self, id: WidgetId) -> bool {
        self.layers.contains_key(&id)
    }

    /// 请求合成器动画（位移 / 不透明度）。动画期间树不重画、不量测；完成经外壳回报。
    pub fn animate_layer(&mut self, id: WidgetId, spec: LayerAnimSpec) {
        if self.layers.contains_key(&id) {
            self.layer_ops.push(LayerOp::Animate { id, spec });
        }
    }

    /// 立即设图层视觉偏移（取消进行中的动画）。
    pub fn set_layer_offset(&mut self, id: WidgetId, x: f32, y: f32) {
        if self.layers.contains_key(&id) {
            self.layer_ops.push(LayerOp::Offset { id, x, y });
        }
    }

    /// 立即设图层不透明度（取消进行中的动画）。
    pub fn set_layer_opacity(&mut self, id: WidgetId, opacity: f32) {
        if self.layers.contains_key(&id) {
            self.layer_ops.push(LayerOp::Opacity { id, opacity });
        }
    }

    /// 外壳取走本帧（及帧间）累积的图层操作，保持产生顺序。
    pub fn take_layer_ops(&mut self) -> Vec<LayerOp> {
        std::mem::take(&mut self.layer_ops)
    }

    /// 同步全部图层：存在性、矩形、内容。每帧拼接之后调用。
    fn sync_layers(&mut self) {
        if self.layers.is_empty() {
            return;
        }
        let ids: Vec<WidgetId> = self.layers.keys().copied().collect();
        for id in ids {
            let alive = self.contains(id);
            if !alive || !self.effectively_visible(id) {
                if let Some(st) = self.layers.get_mut(&id)
                    && st.created
                {
                    *st = LayerState::default();
                    self.layer_ops.push(LayerOp::Remove { id });
                }
                if !alive {
                    self.layers.remove(&id);
                }
                continue;
            }
            let Some(rect) = self.node(id).map(|n| n.rect) else { continue };
            self.compose_layer.set(Some(id));
            let mut scene = Scene::default();
            self.compose(id, &mut scene, None);
            self.compose_layer.set(None);
            scene.translate(Point::new(-rect.origin.x, -rect.origin.y));
            let Some(st) = self.layers.get_mut(&id) else { continue };
            if !st.created {
                st.created = true;
                st.rect = Some(rect);
                st.scene = None;
                self.layer_ops.push(LayerOp::Create { id, rect });
            } else if st.rect != Some(rect) {
                if st.rect.map(|r| r.size) != Some(rect.size) {
                    // 尺寸变了：缓冲要按新尺寸重画。
                    st.scene = None;
                }
                st.rect = Some(rect);
                self.layer_ops.push(LayerOp::Rect { id, rect });
            }
            if st.scene.as_ref() != Some(&scene) {
                st.scene = Some(scene.clone());
                self.layer_ops.push(LayerOp::Content { id, scene });
            }
        }
    }

    /// 节点绘制缓存（测试断言用）。
    pub fn painted(&self, id: WidgetId) -> &[SceneCommand] {
        self.node(id).map(|n| n.paint.as_slice()).unwrap_or(&[])
    }

    pub fn is_visible(&self, id: WidgetId) -> bool {
        self.effectively_visible(id)
    }

    pub fn type_name(&self, id: WidgetId) -> &'static str {
        self.node(id)
            .and_then(|n| n.widget.as_ref())
            .map(|w| w.type_name())
            .unwrap_or("<none>")
    }
}

/// 锚点方位放置：首选方位放不下而对侧放得下则翻转；沿边按 `align` 对齐。
/// 夹进表面由调用方统一做。
fn place_on_side(a: Rect, w: f32, h: f32, spec: PopupSpec, bounds: Rect) -> (f32, f32) {
    use crate::props::Align;
    let along = |start: f32, anchor_len: f32, len: f32| match spec.align {
        Align::Center => start + (anchor_len - len) / 2.0,
        Align::End => start + anchor_len - len,
        Align::Start | Align::Stretch => start,
    };
    let g = spec.gap;
    match spec.side {
        PopupSide::Bottom | PopupSide::Top => {
            let below = a.bottom() + g;
            let above = a.origin.y - g - h;
            let fits_below = below + h <= bounds.bottom();
            let fits_above = above >= bounds.origin.y;
            let y = match spec.side {
                PopupSide::Bottom if !fits_below && fits_above => above,
                PopupSide::Bottom => below,
                _ if !fits_above && fits_below => below,
                _ => above,
            };
            (along(a.origin.x, a.size.width, w), y)
        }
        PopupSide::Center => (a.origin.x, a.origin.y), // 调用方已拦截，保底
        PopupSide::Left | PopupSide::Right => {
            let right = a.right() + g;
            let left = a.origin.x - g - w;
            let fits_right = right + w <= bounds.right();
            let fits_left = left >= bounds.origin.x;
            let x = match spec.side {
                PopupSide::Right if !fits_right && fits_left => left,
                PopupSide::Right => right,
                _ if !fits_left && fits_right => right,
                _ => left,
            };
            (x, along(a.origin.y, a.size.height, h))
        }
    }
}

/// 焦点视觉绘制范围外扩量（描边内绘，此处为抗锯齿外溢留余量；正典 §Ⅳ 外圈 2px）。
const FOCUS_VISUAL_THICKNESS: f32 = kanesumi_core::interaction::FOCUS_OUTER_PX;

/// 单轴解析：在槽位内按固定尺寸 / min / max / 对齐求位置与长度，**结果夹进槽位**。
fn resolve_axis(
    origin: f32,
    slot: f32,
    desired: f32,
    fixed: Option<f32>,
    min: f32,
    max: f32,
    align: Align,
) -> (f32, f32) {
    let natural = match (fixed, align) {
        (Some(f), _) => f,
        (None, Align::Stretch) => slot,
        (None, _) => desired,
    };
    let len = natural.min(max).max(min).min(slot).max(0.0);
    let offset = match align {
        Align::Start | Align::Stretch => 0.0,
        Align::Center => (slot - len) / 2.0,
        Align::End => slot - len,
    };
    (origin + offset.max(0.0), len)
}

fn sanitize(s: Size) -> Size {
    let f = |v: f32| if v.is_nan() { 0.0 } else { v.max(0.0) };
    Size::new(f(s.width), f(s.height))
}

pub(crate) fn union(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

fn rect_within(inner: Rect, outer: Rect) -> bool {
    const EPS: f32 = 0.01;
    inner.origin.x >= outer.origin.x - EPS
        && inner.origin.y >= outer.origin.y - EPS
        && inner.right() <= outer.right() + EPS
        && inner.bottom() <= outer.bottom() + EPS
}

// ── 两个内部根 ─────────────────────────────────────────────────────────────

/// 内容根：每个子节点都铺满整个表面（Z 叠放）；自身透明、不命中。
struct ZStack;

impl Widget for ZStack {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let mut s = Size::ZERO;
        for c in ctx.children() {
            let d = ctx.measure_child(c, available);
            s = Size::new(s.width.max(d.width), s.height.max(d.height));
        }
        s
    }
    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        for c in ctx.children() {
            ctx.arrange_child(c, rect);
        }
    }
    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
}

/// 覆盖层根（XAML PopupRoot）：按弹层规格与锚点放置每个子节点；自身透明、不命中、不裁剪。
struct OverlayRoot;

impl Widget for OverlayRoot {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        for c in ctx.children() {
            // 弹层按内容量测（无界），放置时再夹进表面。
            ctx.measure_child(c, Size::new(available.width, available.height));
        }
        available
    }
    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        for c in ctx.children() {
            let desired = ctx.measure_child(c, rect.size);
            let slot = ctx.popup_slot(c, desired, rect);
            ctx.arrange_child(c, slot);
        }
    }
    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
    fn clips_children(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tooltip_tests {
    use super::*;
    use crate::testing::TestHarness;

    /// 可命中的最小目标（元素树测试不依赖 controls）。
    struct HitBox;
    impl Widget for HitBox {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(80.0, 24.0)
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            scene.fill_rect(kanesumi_core::Color::WHITE, ctx.rect());
        }
        fn focusable(&self) -> bool {
            true
        }
    }

    fn harness() -> (TestHarness, WidgetId, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let start = LayoutProps {
            h_align: Align::Start,
            v_align: Align::Start,
            margin: crate::props::Insets::new(20.0, 20.0, 0.0, 0.0),
            ..LayoutProps::default()
        };
        let a = h.tree.insert_with(h.root(), HitBox, start);
        let b = h.tree.insert_with(
            h.root(),
            HitBox,
            LayoutProps {
                margin: crate::props::Insets::new(20.0, 80.0, 0.0, 0.0),
                ..start
            },
        );
        h.tree.set_tooltip(a, "第一个提示");
        h.tree.set_tooltip(b, "第二个提示");
        h.frame();
        (h, a, b)
    }

    #[test]
    fn shows_after_delay_and_hides_after_five_seconds() {
        let (mut h, a, _) = harness();
        h.move_to(h.center(a));
        assert!(h.tree.tooltip_popup().is_none(), "延迟未满不显示");
        h.idle(0.3);
        assert!(h.tree.tooltip_popup().is_none(), "0.3s 仍不显示");
        h.idle(0.3);
        let popup = h.tree.tooltip_popup().expect("满 500ms 应显示提示");
        assert!(h.rect(popup).size.width > 0.0, "提示弹层已排版");
        h.idle(6.0);
        assert!(h.tree.tooltip_popup().is_none(), "5s 后自动消失");
    }

    #[test]
    fn reshow_between_targets_uses_100ms() {
        let (mut h, a, b) = harness();
        h.move_to(h.center(a));
        h.idle(0.6);
        let first = h.tree.tooltip_popup().expect("首提示显示");
        h.move_to(h.center(b));
        assert!(h.tree.tooltip_popup().is_none(), "移动即收起旧提示");
        h.idle(0.05);
        assert!(h.tree.tooltip_popup().is_none(), "再现延迟未满不显示");
        h.idle(0.1);
        let second = h.tree.tooltip_popup().expect("再现延迟 100ms 后显示");
        assert_ne!(first, second, "新的提示弹层");
    }

    #[test]
    fn pointer_down_hides_immediately() {
        let (mut h, a, _) = harness();
        h.move_to(h.center(a));
        h.idle(0.6);
        assert!(h.tree.tooltip_popup().is_some());
        h.press_at(h.center(a), PointerButton::Left);
        assert!(h.tree.tooltip_popup().is_none(), "按下即消失");
    }

    #[test]
    fn keyboard_focus_triggers_tooltip() {
        let (mut h, a, _) = harness();
        h.key(Key::Tab);
        assert_eq!(h.tree.focused(), Some(a), "Tab 落到首个可聚焦目标");
        assert!(h.tree.tooltip_popup().is_none());
        h.idle(0.6);
        assert!(h.tree.tooltip_popup().is_some(), "键盘焦点停留满延迟也显示");
    }

    #[test]
    fn tooltip_paints_bubble_and_text() {
        let (mut h, a, _) = harness();
        h.move_to(h.center(a));
        h.idle(0.6);
        let popup = h.tree.tooltip_popup().unwrap();
        let cmds = h.tree.painted(popup);
        assert!(
            cmds.iter()
                .any(|c| matches!(c, SceneCommand::FillRect { .. })),
            "气泡底"
        );
        assert!(
            cmds.iter().any(|c| matches!(c, SceneCommand::Text { .. })),
            "提示文字"
        );
    }

    #[test]
    fn tooltip_passthrough_does_not_steal_focus_or_hit() {
        let (mut h, a, _) = harness();
        h.move_to(h.center(a));
        h.idle(0.6);
        let popup = h.tree.tooltip_popup().unwrap();
        let spec = h.tree.popup_spec(popup).unwrap();
        assert!(spec.passthrough, "提示弹层声明输入穿透");
        assert!(!spec.light_dismiss, "提示不参与轻触关闭");
        assert_eq!(h.tree.focused(), None, "提示不抢焦点");
        // 提示显示中，命中仍落在锚点内容上而非提示弹层。
        assert_eq!(h.tree.hit(h.center(a)), Some(a));
    }
}
