// widget.rs —— 控件契约与各阶段上下文。参 ELEMENT_TREE §Ⅲ.2。
//
// 控件不持有子控件、不自知位置：子节点经 `ctx.children()` 访问，位置由框架 arrange 决定。
// 回调期间控件实例被框架从 arena 暂时取出，上下文持有树的可变借用 —— 这是 Rust 里
// 表达「父控件在量测时递归量测子控件」而不互借的方式（masonry 同款思路，arena 实现）。

use std::any::Any;

use kanesumi_anim::Animation;
use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::event::Event;
use crate::id::WidgetId;
use crate::ime::ImeContext;
use crate::props::{Insets, LayoutProps};
use crate::tree::{PopupSpec, Tree};

/// 框架维护的交互状态（对应 XAML 的 PointerOver / Pressed / Focused / Disabled 视觉状态）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ControlStates {
    pub hovered: bool,
    pub pressed: bool,
    pub focused: bool,
    /// 焦点由键盘获得（Tab 等）—— 只有此时才画焦点视觉（XAML `FocusState::Keyboard`）。
    pub keyboard_focused: bool,
    /// 自身或任一祖先被禁用。
    pub disabled: bool,
}

/// 无障碍角色（第一期只定接口，参 ELEMENT_TREE §Ⅸ）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessRole {
    Button,
    CheckBox,
    Switch,
    TextInput,
    Label,
    List,
    ListItem,
    Group,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccessInfo {
    pub role: AccessRole,
    pub name: String,
    pub value: Option<String>,
    /// 勾选 / 开关态（不适用为 None）。
    pub checked: Option<bool>,
}

/// 控件契约。
pub trait Widget: Any {
    /// 量测：返回期望尺寸（内容 + 内边距；**不含** margin，margin 由框架加）。
    /// `available` 已扣 margin 并被固定尺寸 / max 收窄；可为无穷（该轴无界）。
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size;

    /// 排列：容器对每个子节点调用 `ctx.arrange_child`。
    /// 默认把每个子节点铺满自身矩形（XAML `ContentControl` 语义）—— 带内容的控件
    /// （按钮里的标签）不写 arrange 也不会让子节点静默落在零矩形上。叶子无子节点，零开销。
    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        for c in ctx.children() {
            ctx.arrange_child(c, rect);
        }
    }

    /// 自绘（只画自己；子节点由框架随后按树序绘制）。
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene);

    /// 子节点之后的自绘（叠在内容之上：滚动条、覆盖描边）。与 `paint` 同一次失效重画，
    /// 画在子树裁剪之外、仍受祖先裁剪约束。
    fn paint_after(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 路由事件。`ctx.set_handled()` 截停冒泡。
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    /// 动画 tick —— 在 `request_anim_frame` 或 `UpdateCtx::animate` 登记后被调用。
    /// 用 `ctx.animate(&mut anim)` 推进动画：按真实时钟求值、未稳态自动续帧（参 ELEMENT_TREE §Ⅴ-bis）。
    /// 结构保证「动画只动视觉」：`UpdateCtx` 只提供 `invalidate_paint` / `invalidate_arrange`，
    /// 不提供量测失效。
    fn update(&mut self, _ctx: &mut UpdateCtx, _dt: f64) {}

    /// 布局前的子节点实现钩子（虚拟化容器用）。默认不做事。
    ///
    /// 调用时机：`Tree::frame` 在 measure 之前，对本节点被标记 `needs_realize` 且
    /// `wants_realize()` 为真时调用一次，调用后标记清除。容器在这里按**上一帧**的 `rect`
    /// 视口增删子节点（`RealizeCtx::insert_child` / `remove_child`）—— 被移除的子节点会走
    /// 框架的回收清理（焦点 / 捕获 / 悬停 / 弹层引用一并断开）。参 docs/ELEMENT_TREE.md §Ⅳ-bis。
    fn realize(&mut self, _ctx: &mut RealizeCtx) {}

    /// 是否需要 `realize`（避免框架每帧对所有节点做可实现性判断）。
    /// 返回 true 的容器必须能从**任意帧**的 `realize` 幂等收敛（重复调用不改变结果）。
    fn wants_realize(&self) -> bool {
        false
    }

    /// 占 Tab 位、可被点击聚焦。
    fn focusable(&self) -> bool {
        false
    }

    /// 是否有随 hover / pressed / focused 变化的外观。`true` 时框架在状态变化后自动重画本节点；
    /// 且本节点是指针按下时的「交互目标」（Click 投给它）。默认同 `focusable`。
    fn has_visual_states(&self) -> bool {
        self.focusable()
    }

    /// 键盘焦点时是否由框架画焦点视觉（直角描边，`focus_stroke`）。自绘焦点的控件返回 false。
    fn focus_visual(&self) -> bool {
        true
    }

    /// 命中测试。默认矩形内即命中。无背景的容器应返回 false（空白处穿透，COMPOSITION 契约 12）。
    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 子树绘制与命中是否裁到自身矩形（容器裁剪，参 §Ⅳ.3）。
    fn clips_children(&self) -> bool {
        true
    }

    /// 子节点按滚动偏移排在自身矩形之外（ScrollViewer）。框架的「子矩形 ⊆ 父矩形」
    /// 调试断言对这类节点的直接子节点豁免（可见性由裁剪保证）。
    fn scrolls_children(&self) -> bool {
        false
    }

    /// 键盘焦点落到后代 `target`（表面坐标）时调用：滚动容器应把它滚进视口并返回 true
    /// （XAML `BringIntoView`）。默认不处理。
    fn bring_into_view(&mut self, _ctx: &mut EventCtx, _target: Rect) -> bool {
        false
    }

    /// 画出自身矩形的显式许可（徽标外溢、描边外扩）。框架据此扩展损伤；仍受祖先裁剪。
    fn paint_overflow(&self) -> Insets {
        Insets::ZERO
    }

    /// 聚焦时的 IME 上下文（周边文本 + 光标矩形）。`Some` → 外壳开 text-input。
    /// 框架在每帧绘制后对焦点控件调用一次并缓存（此时布局与排版都是本帧的）。
    fn ime(&self, _rect: Rect, _theme: &MetroTheme, _engine: &TextEngine) -> Option<ImeContext> {
        None
    }

    fn accessibility(&self) -> Option<AccessInfo> {
        None
    }

    /// 诊断用类型名。
    fn type_name(&self) -> &'static str {
        std::any::type_name::<Self>()
    }
}

// ── 上下文 ─────────────────────────────────────────────────────────────────────

pub struct MeasureCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
    pub(crate) engine: &'a TextEngine,
}

impl MeasureCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }
    pub fn engine(&self) -> &TextEngine {
        self.engine
    }
    pub fn theme(&self) -> &MetroTheme {
        self.tree.theme()
    }
    pub fn children(&self) -> Vec<WidgetId> {
        self.tree.children(self.id).to_vec()
    }
    pub fn child_props(&self, child: WidgetId) -> LayoutProps {
        self.tree.props(child).unwrap_or_default()
    }
    /// 量测子节点，返回其期望尺寸（**含** margin）。约束不变且未失效时命中缓存。
    pub fn measure_child(&mut self, child: WidgetId, available: Size) -> Size {
        self.tree.measure_node(child, available, self.engine)
    }
    /// measure 期内再次失效量测：标记保留到下一帧（本帧末尾不清，参 ELEMENT_TREE §帧调度）。
    pub fn invalidate_measure(&mut self) {
        self.tree.invalidate_measure(self.id);
    }
    pub fn invalidate_arrange(&mut self) {
        self.tree.invalidate_arrange(self.id);
    }
    pub fn invalidate_paint(&mut self) {
        self.tree.invalidate_paint(self.id);
    }
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
}

pub struct ArrangeCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
    pub(crate) engine: &'a TextEngine,
}

impl ArrangeCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }
    pub fn engine(&self) -> &TextEngine {
        self.engine
    }
    pub fn children(&self) -> Vec<WidgetId> {
        self.tree.children(self.id).to_vec()
    }
    pub fn child_props(&self, child: WidgetId) -> LayoutProps {
        self.tree.props(child).unwrap_or_default()
    }
    /// 量测子节点（arrange 中重量测同约束会命中缓存）。
    pub fn measure_child(&mut self, child: WidgetId, available: Size) -> Size {
        self.tree.measure_node(child, available, self.engine)
    }
    /// 把子节点排进 `slot`。框架按子节点的 margin / 对齐 / min / max 解析最终矩形，
    /// 且**结果一律夹紧进 slot**（保险机制 §Ⅳ.3-1）。
    pub fn arrange_child(&mut self, child: WidgetId, slot: Rect) {
        self.tree.arrange_node(child, slot, self.engine);
    }
    /// 覆盖层专用：按弹层规格与锚点求槽位。
    pub(crate) fn popup_slot(&self, child: WidgetId, desired: Size, bounds: Rect) -> Rect {
        self.tree.popup_slot(child, desired, bounds)
    }
    /// 视口 / 可用尺寸变化后要求本节点重新 `realize`（下一帧 measure 之前）。
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
    /// arrange 期内再失效：标记保留到下一帧。
    pub fn invalidate_measure(&mut self) {
        self.tree.invalidate_measure(self.id);
    }
    pub fn invalidate_arrange(&mut self) {
        self.tree.invalidate_arrange(self.id);
    }
    pub fn invalidate_paint(&mut self) {
        self.tree.invalidate_paint(self.id);
    }
}

pub struct PaintCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
    pub(crate) engine: &'a TextEngine,
    pub(crate) rect: Rect,
    pub(crate) state: ControlStates,
}

impl PaintCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }
    pub fn engine(&self) -> &TextEngine {
        self.engine
    }
    pub fn theme(&self) -> &MetroTheme {
        self.tree.theme()
    }
    /// 本节点 arrange 后的矩形 —— 绘制的唯一依据。
    pub fn rect(&self) -> Rect {
        self.rect
    }
    /// 弹层放置区（画级联子菜单等越出自身的弹层内容时用）。默认 = 表面，外壳可放大到整个输出。
    pub fn surface(&self) -> Rect {
        self.tree.popup_bounds()
    }
    pub fn state(&self) -> ControlStates {
        self.state
    }
    /// 请求下一帧 `update`（视觉状态过渡 / 动画未到稳态）。
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
    }
    /// `secs` 秒后调用本节点 `update`（等待期间不占帧）。
    pub fn request_timer(&mut self, secs: f64) {
        self.tree.request_timer(self.id, secs);
    }
    /// 按本帧真实经过时间推进动画，未到稳态自动登记下一帧（无需再手动请求）。
    /// 返回动画当前值；调用方仍需 `invalidate_paint()` 以触发重画。
    pub fn animate<A: Animation>(&mut self, anim: &mut A) -> f64 {
        self.tree.animate(self.id, anim)
    }
    /// 绘制期内再失效（重画自己）：标记保留到下一帧。
    pub fn invalidate_paint(&mut self) {
        self.tree.invalidate_paint(self.id);
    }
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
}

pub struct EventCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
    pub(crate) handled: bool,
}

impl EventCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }
    pub fn rect(&self) -> Rect {
        self.tree
            .rect(self.id)
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
    }
    pub fn state(&self) -> ControlStates {
        self.tree.states(self.id)
    }
    pub fn theme(&self) -> &MetroTheme {
        self.tree.theme()
    }
    /// 弹层放置区（弹层放置 / 子菜单翻转用）。默认 = 表面，外壳可放大到整个输出。
    pub fn surface(&self) -> Rect {
        self.tree.popup_bounds()
    }
    /// 最近一次指针位置。`Event::Click` 不带坐标 —— 多区域控件（分体按钮、面包屑）据此判区。
    pub fn pointer(&self) -> Option<Point> {
        self.tree.pointer()
    }
    /// 聚焦任意节点（打开弹层后把焦点移进去）。不可聚焦 → false。
    pub fn focus_widget(&mut self, id: WidgetId, keyboard: bool) -> bool {
        self.tree.focus(id, keyboard)
    }
    /// 排版引擎（首帧之前为 None）。点击定位光标等文本命中用。
    pub fn engine(&self) -> Option<&TextEngine> {
        self.tree.engine()
    }
    /// 截停冒泡。
    pub fn set_handled(&mut self) {
        self.handled = true;
    }
    /// 向 App 发出动作（App 在 `take_actions` 中按来源 id 与类型 downcast 处理）。
    pub fn emit<A: Any>(&mut self, action: A) {
        self.tree.push_action(self.id, Box::new(action));
    }
    pub fn invalidate_measure(&mut self) {
        self.tree.invalidate_measure(self.id);
    }
    pub fn invalidate_arrange(&mut self) {
        self.tree.invalidate_arrange(self.id);
    }
    pub fn invalidate_paint(&mut self) {
        self.tree.invalidate_paint(self.id);
    }
    /// 要求本节点下一帧 measure 之前重新 `realize`（虚拟化容器：滚动偏移 / 数据变化后）。
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
    }
    /// `secs` 秒后调用本节点 `update`（等待期间不占帧）。
    pub fn request_timer(&mut self, secs: f64) {
        self.tree.request_timer(self.id, secs);
    }
    /// 程序化聚焦自身（`keyboard` = 是否显示焦点视觉）。
    pub fn request_focus(&mut self, keyboard: bool) {
        self.tree.focus(self.id, keyboard);
    }
    /// 打开弹层（挂到覆盖层）。锚点缺省为本节点。
    pub fn open_popup(&mut self, widget: impl Widget, mut spec: PopupSpec) -> WidgetId {
        if spec.anchor.is_none() {
            spec.anchor = Some(self.id);
        }
        self.tree.open_popup(widget, spec)
    }
    pub fn close_popup(&mut self, popup: WidgetId) {
        self.tree.close_popup(popup);
    }
    /// 修改**另一个**节点（典型：输入框在键入时更新它打开的建议弹层）。语义同 `Tree::edit`。
    /// 目标是自身（回调期间已被取出）或不存在 / 类型不符时返回 None。
    pub fn edit<T: Widget, R>(
        &mut self,
        id: WidgetId,
        f: impl FnOnce(&mut T, &mut crate::tree::EditCtx) -> R,
    ) -> Option<R> {
        self.tree.edit(id, f)
    }
}

pub struct UpdateCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
}

impl UpdateCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }
    pub fn invalidate_paint(&mut self) {
        self.tree.invalidate_paint(self.id);
    }
    /// 重排子节点位置 —— **只用于平移类动画**（平滑滚动）。不提供量测失效：
    /// 动画不得改变任何节点的尺寸（参 ELEMENT_TREE §Ⅴ.1「动画只动视觉」）。
    pub fn invalidate_arrange(&mut self) {
        self.tree.invalidate_arrange(self.id);
    }
    /// 要求本节点下一帧 measure 之前重新 `realize`（虚拟化容器：平滑滚动推进后）。
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
    }
    /// `secs` 秒后再调用本节点 `update`（等待期间不占帧）。
    pub fn request_timer(&mut self, secs: f64) {
        self.tree.request_timer(self.id, secs);
    }
    /// 按本帧真实经过时间推进动画，未到稳态自动登记下一帧（无需再手动请求）。
    /// 返回动画当前值；调用方仍需 `invalidate_paint()` 以触发重画。
    pub fn animate<A: Animation>(&mut self, anim: &mut A) -> f64 {
        self.tree.animate(self.id, anim)
    }
}

/// 实现钩子上下文（参 `Widget::realize` / docs/ELEMENT_TREE.md §Ⅳ-bis）。
///
/// 回调期间控件实例已被框架从 arena 取出，因此本上下文**不能编辑自身**（`edit(self.id, ..)`
/// 会因目标不存在而返回 None）；只能访问主题 / 引擎 / `children()`，并在自身下增删子节点。
pub struct RealizeCtx<'a> {
    pub(crate) tree: &'a mut Tree,
    pub(crate) id: WidgetId,
    pub(crate) engine: &'a TextEngine,
}

impl RealizeCtx<'_> {
    pub fn id(&self) -> WidgetId {
        self.id
    }

    /// 排版引擎（本帧的，与 measure / paint 同源）。
    pub fn engine(&self) -> &TextEngine {
        self.engine
    }

    pub fn theme(&self) -> &MetroTheme {
        self.tree.theme()
    }

    /// 本节点**上一帧**的矩形（首帧未排列时为零矩形）。虚拟化容器据此取视口。
    pub fn rect(&self) -> Rect {
        self.tree
            .rect(self.id)
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 0.0, 0.0))
    }

    /// 表面矩形（首帧视口未知时的兜底 —— 根下的容器首帧即可用）。
    pub fn surface(&self) -> Rect {
        self.tree.surface()
    }

    /// 本节点当前全部子节点（含被回收池隐藏的）。
    pub fn children(&self) -> Vec<WidgetId> {
        self.tree.children(self.id).to_vec()
    }

    pub fn child_props(&self, child: WidgetId) -> LayoutProps {
        self.tree.props(child).unwrap_or_default()
    }

    /// 在自身之下追加子节点（由框架回收）。
    pub fn insert_child(&mut self, widget: impl Widget) -> WidgetId {
        self.tree.insert(self.id, widget)
    }

    /// 在自身之下追加子节点（带框架布局属性）。
    pub fn insert_child_with(&mut self, widget: impl Widget, props: LayoutProps) -> WidgetId {
        self.tree.insert_with(self.id, widget, props)
    }

    /// 在自身之下追加**装箱**子节点（`ItemFactory::build` 的产物）。
    pub fn insert_child_boxed(&mut self, widget: Box<dyn Widget>) -> WidgetId {
        self.tree.insert_boxed_with(self.id, widget, LayoutProps::default())
    }

    /// 删除一个**直接子节点**（及其子树）。非直接子节点忽略。
    pub fn remove_child(&mut self, child: WidgetId) {
        if self.tree.parent(child) == Some(self.id) {
            self.tree.remove(child);
        }
    }

    /// 编辑子节点（语义同 `Tree::edit`）。目标为自身或不存在时返回 None。
    pub fn edit<T: Widget, R>(
        &mut self,
        child: WidgetId,
        f: impl FnOnce(&mut T, &mut crate::tree::EditCtx) -> R,
    ) -> Option<R> {
        self.tree.edit(child, f)
    }

    pub fn set_child_visible(&mut self, child: WidgetId, visible: bool) {
        self.tree
            .update_props(child, |p| p.visible = visible);
    }

    pub fn child_visible(&self, child: WidgetId) -> bool {
        self.tree
            .props(child)
            .is_some_and(|p| p.visible)
    }

    /// `node` 是否为焦点节点的祖先或自身（焦点节点不回收，参 docs/ELEMENT_TREE.md §Ⅳ-bis）。
    pub fn is_focus_related(&self, node: WidgetId) -> bool {
        self.tree
            .focused()
            .is_some_and(|f| self.tree.is_ancestor_or_self(node, f))
    }

    /// 子节点变化 / 视口变化后置本节点量测失效（下一帧重排）。
    pub fn invalidate_measure(&mut self) {
        self.tree.invalidate_measure(self.id);
    }

    /// 要求本节点下一个 realize 周期再实现一次（数据变化时用）。
    pub fn invalidate_realize(&mut self) {
        self.tree.invalidate_realize(self.id);
    }
}
