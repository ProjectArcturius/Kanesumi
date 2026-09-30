// widget.rs —— 控件契约与各阶段上下文。参 ELEMENT_TREE §Ⅲ.2。
//
// 控件不持有子控件、不自知位置：子节点经 `ctx.children()` 访问，位置由框架 arrange 决定。
// 回调期间控件实例被框架从 arena 暂时取出，上下文持有树的可变借用 —— 这是 Rust 里
// 表达「父控件在量测时递归量测子控件」而不互借的方式（masonry 同款思路，arena 实现）。

use std::any::Any;

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::event::Event;
use crate::id::WidgetId;
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

    /// 路由事件。`ctx.set_handled()` 截停冒泡。
    fn event(&mut self, _ctx: &mut EventCtx, _event: &Event) {}

    /// 动画 tick —— 仅在请求过 `request_anim_frame` 后被调用。
    /// 结构保证「动画只动视觉」：`UpdateCtx` 只提供 `invalidate_paint`，不提供量测失效。
    fn update(&mut self, _ctx: &mut UpdateCtx, _dt: f64) {}

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

    /// 画出自身矩形的显式许可（徽标外溢、描边外扩）。框架据此扩展损伤；仍受祖先裁剪。
    fn paint_overflow(&self) -> Insets {
        Insets::ZERO
    }

    /// 聚焦时的 IME 请求。`Some` → 外壳开 text-input。
    fn ime(&self, _rect: Rect) -> Option<crate::tree::ImeRequest> {
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
    pub fn state(&self) -> ControlStates {
        self.state
    }
    /// 请求下一帧 `update`（视觉状态过渡 / 动画未到稳态）。
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
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
        self.tree.rect(self.id).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
    }
    pub fn state(&self) -> ControlStates {
        self.tree.states(self.id)
    }
    pub fn theme(&self) -> &MetroTheme {
        self.tree.theme()
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
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
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
    pub fn request_anim_frame(&mut self) {
        self.tree.request_anim(self.id);
    }
}
