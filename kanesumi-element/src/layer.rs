// layer.rs —— 元素树图层（G3-c）。参 Ether docs/GPU_COMPOSITION_PLAN.md G3、ELEMENT_TREE「图层」。
//
// 对应 UWP 给元素挂 Composition Visual：被标为图层的子树不进入主表面的场景，而是单独拼成一张
// Scene，由外壳交给一个独立的合成器子表面；**只有子树绘制结果真的变了才重新提交内容**。
// 图层的位移 / 不透明度动画经 `Tree::animate_layer` 交给合成器推进 —— 动画期间不重画、不量测。
// 布局与命中不受影响：图层节点照常参与 measure / arrange / 命中（视觉偏移不改可点区域，COMPOSITION 契约 12）。
//
// 本模块只定义纯数据（元素树不依赖外壳）；外壳侧（TreeHost）把 `LayerOp` 翻译成 harness 的图层命令。

use kanesumi_canvas::Scene;
use kanesumi_core::{Rect, Size};

use crate::id::WidgetId;

/// 一次合成器动画（从当前呈现值过渡到终值）。语义同 harness `LayerAnim`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAnimSpec {
    pub serial: u32,
    pub duration_ms: u32,
    pub delay_ms: u32,
    /// cubic-bezier (x1, y1, x2, y2)。
    pub curve: [f32; 4],
    pub to_x: f32,
    pub to_y: f32,
    pub to_opacity: f32,
}

/// 图层裁剪内缩（逻辑像素，自图层四边向内量；全 0 = 不裁）。语义同 harness `LayerInsets`、
/// 协议 `ether_visual_v1.set_clip`（v2）：只影响绘制，不影响布局与命中。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LayerInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// 一次合成器裁剪动画（从当前呈现内缩过渡到 `to`）。与位移 / 不透明度动画是两路，互不取消。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerClipSpec {
    pub serial: u32,
    pub duration_ms: u32,
    pub delay_ms: u32,
    /// cubic-bezier (x1, y1, x2, y2)。
    pub curve: [f32; 4],
    pub to: LayerInsets,
}

/// 元素树产出的图层操作（每帧经 `Tree::take_layer_ops` 取走）。
#[derive(Debug, Clone)]
pub enum LayerOp {
    /// 图层开始存在（节点被标为图层且可见）。`rect` 为表面局部逻辑坐标。
    Create { id: WidgetId, rect: Rect },
    /// 内容变了：`scene` 已平移到图层自身坐标（原点 = 图层矩形左上角）。
    Content { id: WidgetId, scene: Scene },
    /// 布局矩形变了（尺寸变化时随后必有 Content）。
    Rect { id: WidgetId, rect: Rect },
    Offset { id: WidgetId, x: f32, y: f32 },
    Opacity { id: WidgetId, opacity: f32 },
    Animate { id: WidgetId, spec: LayerAnimSpec },
    /// 立即设裁剪内缩（取消进行中的裁剪动画，不碰位移 / 不透明度动画）。
    Clip { id: WidgetId, insets: LayerInsets },
    AnimateClip { id: WidgetId, spec: LayerClipSpec },
    /// 图层不再存在（取消标记 / 节点删除 / 不可见）。
    Remove { id: WidgetId },
    // ── 合成滚动（fp4b）：滚动器图层。参 Ether fp4-compositor-scroll.md §Ⅴ ──
    /// 滚动器图层开始存在。`rect` = 视口（表面局部逻辑坐标，同时是输入区）。
    ScrollerCreate { id: WidgetId, rect: Rect },
    /// 视口 / 内容全长 / 背景色变化（双缓冲，随内容表面 commit 生效）。
    ScrollerGeometry {
        id: WidgetId,
        viewport: Rect,
        extent: Size,
        /// 视口内未被已实现区覆盖处的填充色（0xAARRGGBB，不许露黑）。
        background: u32,
    },
    /// 内容缓冲对应的内容坐标矩形 + 该窗口的内容（同一次提交原子生效）。
    /// `scene` 已平移到缓冲坐标（原点 = realized 左上角）。
    ScrollerRealize { id: WidgetId, realized: Rect, scene: Scene },
    /// 客户端主动滚动（键盘 / `bring_into_view`；立即生效，不经 commit）。
    ScrollerScroll {
        id: WidgetId,
        serial: u32,
        request: ScrollerRequest,
    },
}

/// 客户端主动滚动请求（协议 `scroll_by` / `scroll_to`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollerRequest {
    ScrollBy { delta: f32 },
    ScrollTo { target: f32, animate: bool },
}

/// 滚动容器每帧交给框架的快照（`Widget::take_scroller_snapshot`）。
/// 全部主轴 = 垂直（与 `MetroScrollView` 一致；协议 `axes` 恒 1）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollerSnapshot {
    /// 视口尺寸（位置由节点矩形给出）。
    pub viewport: Size,
    /// 内容全长（协议 extent）。
    pub extent: Size,
    /// 当前滚动偏移（最近一次合成器 `offset` 事件；自绘模式为本机值）。
    pub offset: f32,
    /// 挂起的客户端主动滚动（键盘 / bring_into_view），取出即清。
    pub request: Option<ScrollerRequest>,
}

/// 树内每个滚动器图层的跟踪状态。
#[derive(Debug, Default)]
pub(crate) struct ScrollerState {
    /// 已向外壳发过 ScrollerCreate（且未 Remove）。
    pub(crate) created: bool,
    pub(crate) viewport: Option<Rect>,
    pub(crate) extent: Option<Size>,
    /// 当前已实现窗口（内容坐标）与该缓冲的已提交内容（缓冲坐标）。
    pub(crate) realized: Option<Rect>,
    pub(crate) scene: Option<Scene>,
}

// ── 实现窗口几何（纯函数，单测在 tree_tests）────────────────────────────────

/// 实现窗口：沿滚动轴在视口前后各备一屏（共 3 屏），夹在内容范围。
/// 返回 (y, h)；内容不足 3 屏时取全长。参 fp4-compositor-scroll.md §Ⅴ-2。
pub fn scroller_realized_window(offset: f32, viewport_h: f32, content_h: f32) -> (f32, f32) {
    let h = (viewport_h * 3.0).min(content_h).max(0.0);
    let y = (offset - viewport_h).clamp(0.0, (content_h - h).max(0.0));
    (y, h)
}

/// 是否需要重实现：视口任一边距已实现区边缘不足 0.5 屏。
/// `realized` = (y, h)。窗口贴合内容全长（无滚动余量），或该侧已贴到内容边界
/// （顶 / 底之外无内容可备）时，该侧不再要求余量——否则贴底会每帧重实现。
pub fn scroller_needs_realize(offset: f32, viewport_h: f32, realized: (f32, f32), content_h: f32) -> bool {
    let (y, h) = realized;
    if h >= content_h {
        return false;
    }
    let margin = viewport_h * 0.5;
    let top_ok = offset - y >= margin || y <= 0.0;
    let bottom_ok = (y + h) - (offset + viewport_h) >= margin || y + h >= content_h - 0.5;
    !(top_ok && bottom_ok)
}

/// 树内每个图层的跟踪状态。
#[derive(Debug, Default)]
pub(crate) struct LayerState {
    /// 已向外壳发过 Create（且未 Remove）。
    pub(crate) created: bool,
    pub(crate) rect: Option<Rect>,
    /// 上次提交的内容（比较是否需要重新提交）。
    pub(crate) scene: Option<Scene>,
}
