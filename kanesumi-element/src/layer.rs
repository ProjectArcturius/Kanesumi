// layer.rs —— 元素树图层（G3-c）。参 Ether docs/GPU_COMPOSITION_PLAN.md G3、ELEMENT_TREE「图层」。
//
// 对应 UWP 给元素挂 Composition Visual：被标为图层的子树不进入主表面的场景，而是单独拼成一张
// Scene，由外壳交给一个独立的合成器子表面；**只有子树绘制结果真的变了才重新提交内容**。
// 图层的位移 / 不透明度动画经 `Tree::animate_layer` 交给合成器推进 —— 动画期间不重画、不量测。
// 布局与命中不受影响：图层节点照常参与 measure / arrange / 命中（视觉偏移不改可点区域，COMPOSITION 契约 12）。
//
// 本模块只定义纯数据（元素树不依赖外壳）；外壳侧（TreeHost）把 `LayerOp` 翻译成 harness 的图层命令。

use kanesumi_canvas::Scene;
use kanesumi_core::Rect;

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
    /// 图层不再存在（取消标记 / 节点删除 / 不可见）。
    Remove { id: WidgetId },
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
