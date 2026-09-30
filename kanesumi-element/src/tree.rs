// tree.rs —— 保留元素树本体。参 ELEMENT_TREE §Ⅲ~§Ⅵ。
//
// 一帧的顺序（`frame`）：动画 tick → 布局（只算脏子树，量测带缓存）→ 绘制（只重画脏节点，
// 逐节点命令缓存）→ 拼接 Scene + 产出损伤。输入在帧与帧之间到达，命中用的是**上一帧**
// 的布局产物 —— 正是用户眼睛看到的那一帧（画在哪就点在哪）。

use std::any::Any;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::event::{Event, Key, Modifiers, PointerButton};
use crate::id::WidgetId;
use crate::ime::ImeContext;
use crate::props::{Align, LayoutProps};
use crate::widget::{ArrangeCtx, ControlStates, EventCtx, MeasureCtx, PaintCtx, UpdateCtx, Widget};

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
        }
    }
}

/// 一帧产物。
#[derive(Debug, Clone, Default)]
pub struct FrameOutput {
    pub scene: Scene,
    /// 本帧变化区域；`None` = 全量重绘（首帧 / 尺寸变化 / 主题变化）。
    pub damage: Option<Rect>,
    /// 是否仍有动画在跑（外壳据此继续请求帧）。
    pub animating: bool,
}

#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    needs_measure: bool,
    needs_arrange: bool,
    needs_paint: bool,
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
                disabled: false,
            },
            desired: Size::ZERO,
            last_available: None,
            rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            paint: Vec::new(),
            paint_after: Vec::new(),
            painted_bounds: None,
            state: ControlStates::default(),
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
    anim: Vec<WidgetId>,
    /// 定时器：(节点, 剩余秒)。到期把节点放进动画 tick（调用其 `update`）。
    timers: Vec<(WidgetId, f64)>,
    /// 因不可见而暂停的动画节点：不 tick、不占帧；重新可见时恢复。
    parked: Vec<WidgetId>,
    damage: Option<Rect>,
    full_repaint: bool,
    dirty: bool,
    /// 最近一次指针位置（`Click` 等不带坐标的事件里，控件经 `EventCtx::pointer` 取用）。
    pointer: Option<Point>,
    /// 焦点控件本帧的 IME 上下文（`frame` 末尾计算，外壳查询时直接返回）。
    ime: Option<ImeContext>,
    /// 上一帧的排版引擎（`TextEngine` clone 为零拷贝）。事件处理里的文本命中
    /// （点击定位光标）要用它 —— 外壳的输入回调不带引擎。
    engine: Option<TextEngine>,
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
            anim: Vec::new(),
            timers: Vec::new(),
            parked: Vec::new(),
            damage: None,
            full_repaint: true,
            dirty: true,
            ime: None,
            engine: None,
            pointer: None,
        };
        tree.root = tree.alloc(Box::new(ZStack), None, LayoutProps::default());
        tree.overlay = tree.alloc(Box::new(OverlayRoot), None, LayoutProps::default());
        tree
    }

    // ── 结构 ────────────────────────────────────────────────────────────────

    /// 内容根：App 把页面挂在它下面。根的每个子节点都铺满整个表面（Z 叠放）。
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
        if let Some(slot) = self.free.pop() {
            self.nodes[slot] = Some(node);
            WidgetId::new(slot, self.gens[slot])
        } else {
            self.nodes.push(Some(node));
            self.gens.push(0);
            WidgetId::new(self.nodes.len() - 1, 0)
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
        let parent = if self.contains(parent) {
            parent
        } else {
            log::error!("kanesumi-element: insert 的父节点不存在，改挂内容根");
            self.root
        };
        let id = self.alloc(Box::new(widget), Some(parent), props);
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
        if let Some(p) = parent {
            if let Some(pn) = self.node_mut(p) {
                pn.children.retain(|c| *c != id);
            }
            self.invalidate_measure(p);
        }
        self.dirty = true;
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
        }
        self.full_repaint = true;
        self.dirty = true;
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

    pub fn invalidate_measure(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_measure = true;
            n.flags.needs_arrange = true;
            cur = n.parent;
        }
        self.dirty = true;
    }

    pub fn invalidate_arrange(&mut self, id: WidgetId) {
        let mut cur = Some(id);
        while let Some(c) = cur {
            let Some(n) = self.node_mut(c) else { break };
            n.flags.needs_arrange = true;
            cur = n.parent;
        }
        self.dirty = true;
    }

    pub fn invalidate_paint(&mut self, id: WidgetId) {
        if let Some(n) = self.node_mut(id) {
            n.flags.needs_paint = true;
            self.dirty = true;
        }
    }

    pub(crate) fn request_anim(&mut self, id: WidgetId) {
        if !self.anim.contains(&id) {
            self.anim.push(id);
        }
        self.dirty = true;
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
    pub fn tick_timers(&mut self, dt: f64) {
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
        self.timers.iter().map(|(_, l)| *l).reduce(f64::min)
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
            if let Some(b) = self.node_mut(cur).and_then(|n| {
                n.paint.clear();
                n.paint_after.clear();
                n.flags.needs_paint = true;
                n.painted_bounds.take()
            }) {
                self.add_damage(b);
            }
        }
        self.dirty = true;
    }

    // ── 帧 ──────────────────────────────────────────────────────────────────

    /// 出一帧：动画 tick → 布局 → 绘制 → 拼接 + 损伤。
    pub fn frame(&mut self, engine: &TextEngine, size: Size, dt: f64) -> FrameOutput {
        if self.engine.is_none() {
            self.engine = Some(engine.clone());
        }
        let size = size.normalized();
        if size != self.size {
            self.size = size;
            self.invalidate_measure(self.root);
            self.invalidate_measure(self.overlay);
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

        // 2. 布局。弹层位置依赖锚点矩形 —— 有弹层时每帧重排覆盖层（弹层子树本身有缓存，代价只是重算槽位）。
        if !self.popups.is_empty() {
            self.invalidate_arrange(self.overlay);
        }
        let full = Rect::new(0.0, 0.0, size.width, size.height);
        for r in [self.root, self.overlay] {
            self.measure_node(r, size, engine);
            self.arrange_node(r, full, engine);
        }
        self.validate_focus();
        if cfg!(debug_assertions) {
            self.check_containment();
        }

        // 3. 绘制脏节点。
        for r in [self.root, self.overlay] {
            self.paint_dirty(r, engine);
        }

        // 4. 拼接。
        let mut scene = Scene::default();
        self.compose(self.root, &mut scene);
        // 遮罩：任一打开的弹层要求时，在覆盖层之前整面压暗（弹层自身画在遮罩之上）。
        if self.popups.iter().any(|(_, s)| s.scrim) {
            scene.fill_rect(
                self.theme.overlay_color,
                Rect::new(0.0, 0.0, size.width, size.height),
            );
        }
        self.compose(self.overlay, &mut scene);

        // 5. 焦点控件的 IME 上下文（用本帧布局与排版）。
        self.ime = self.focus.and_then(|f| {
            let n = self.node(f)?;
            n.widget.as_ref()?.ime(n.rect, &self.theme, engine)
        });

        let damage = if self.full_repaint { None } else { self.damage };
        self.full_repaint = false;
        self.damage = None;
        self.dirty = false;
        FrameOutput {
            scene,
            damage,
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
            n.last_available = Some(available);
            n.flags.needs_measure = false;
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
        let Some(n) = self.node_mut(id) else { return };
        if n.rect == rect && !n.flags.needs_arrange {
            return;
        }
        if n.rect != rect {
            n.flags.needs_paint = true;
        }
        n.rect = rect;
        n.flags.needs_arrange = false;
        let Some(mut wdg) = n.widget.take() else {
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

    fn paint_dirty(&mut self, id: WidgetId, engine: &TextEngine) {
        let Some(n) = self.node(id) else { return };
        if !n.props.visible {
            return;
        }
        if n.flags.needs_paint {
            let rect = n.rect;
            let state = self.states(id);
            if let Some(mut w) = self.node_mut(id).and_then(|n| n.widget.take()) {
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
                    n.flags.needs_paint = false;
                }
            }
        }
        for c in self.children(id).to_vec() {
            self.paint_dirty(c, engine);
        }
    }

    fn compose(&self, id: WidgetId, scene: &mut Scene) {
        let Some(n) = self.node(id) else { return };
        if !n.props.visible {
            return;
        }
        scene.commands.extend(n.paint.iter().cloned());
        if !n.children.is_empty() {
            let clip = n.widget.as_ref().is_none_or(|w| w.clips_children());
            if clip {
                scene.push_clip(n.rect);
            }
            for &c in &n.children {
                self.compose(c, scene);
            }
            if clip {
                scene.pop_clip();
            }
        }
        scene.commands.extend(n.paint_after.iter().cloned());
        // 键盘焦点视觉：框架统一绘制（直角描边），画在子树之上。
        if self.focus == Some(id)
            && self.focus_keyboard
            && n.widget.as_ref().is_some_and(|w| w.focus_visual())
        {
            scene.stroke_rect(
                self.theme.colors.focus_stroke,
                n.rect,
                FOCUS_VISUAL_THICKNESS,
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

    fn update_hover(&mut self, pos: Option<Point>) {
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
    }

    /// 最近一次指针位置（首次指针事件前为 None）。
    pub fn pointer(&self) -> Option<Point> {
        self.pointer
    }

    pub fn pointer_move(&mut self, pos: Point) {
        self.pointer = Some(pos);
        self.update_hover(Some(pos));
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
        // 覆盖层有弹层时：点在所有弹层之外 → LightDismiss（模态则只吞不关）。
        if let Some(&(top, spec)) = self.popups.last() {
            let inside = self.hit_rec(self.overlay, pos, None).is_some();
            if !inside {
                if spec.light_dismiss && !spec.modal {
                    self.dismiss_popup(top);
                }
                self.dirty = true;
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
        self.update_hover(Some(pos));
    }

    pub fn pointer_leave(&mut self) {
        if self.captured.is_none() {
            self.update_hover(None);
        }
    }

    pub fn scroll(&mut self, pos: Point, dx: f32, dy: f32, modifiers: Modifiers) {
        if let Some(t) = self.hit(pos) {
            self.deliver(t, &Event::Scroll { dx, dy, modifiers });
        }
    }

    /// 键按下。返回是否被消费（未消费的键外壳可另作他用）。
    pub fn key_down(&mut self, key: Key, modifiers: Modifiers) -> bool {
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
                if let Some(&(top, spec)) = self.popups.last()
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

    /// 表面矩形（逻辑坐标，原点 0,0）。弹层放置 / 级联子菜单翻转用。
    pub fn surface(&self) -> Rect {
        Rect::new(0.0, 0.0, self.size.width, self.size.height)
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
        let scope = match self.popups.last() {
            Some(&(top, spec)) if spec.modal => vec![top],
            Some(&(top, _)) => vec![top, self.root],
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

/// 框架焦点视觉描边粗细（Ncrust KANESUMI_XAML「键盘焦点」：粗细 2、直角、FocusVisualMargin 0）。
const FOCUS_VISUAL_THICKNESS: f32 = 2.0;

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
