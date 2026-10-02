// layers — 合成图层（G3-b）：App 侧的图层命令与事件。参 Ether docs/GPU_COMPOSITION_PLAN.md G3。
//
// DirectComposition 式分工：App 把一块内容（一张 Scene）交给外壳，外壳把它光栅进一个独立子表面
// **只画一次**；位移 / 不透明度动画交给合成器（`ether_composition_v1`）按自己的时钟推进，
// 动画期间 App 与外壳都不重画、不提交。
//
// 用法：App 持一个 `LayerQueue`，在 update / 事件处理里推命令；外壳每帧经 `App::take_layer_commands`
// 取走执行；合成器回报的完成 / 打断经 `App::on_layer_event` 送回。本模块跨平台（纯数据），
// 平台实现见 `platform/layers.rs`。
//
// 语义约定：
// - 图层是**纯视觉**的：不接收输入（外壳给子表面设空输入区域），命中仍由 App 按布局判定
//   （参 COMPOSITION 契约 12）。
// - 图层矩形是相对父表面的逻辑坐标；层叠按创建顺序，后建的在上，全部在父表面内容之上。
// - 合成器不支持 `ether_composition_v1` 时：Animate 直接跳到终值并立即回报 Done，
//   SetOpacity 被忽略（`LayerEvent::Unsupported` 只发一次），功能不缺、只是没有动画。

use kanesumi_canvas::Scene;
use kanesumi_core::Rect;

/// 图层标识（App 自选，进程内唯一）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayerId(pub u32);

/// 图层挂在哪个表面上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerParent {
    /// 主表面（窗口 / layer-shell 主表面）。
    Main,
    /// 第 i 个浮层（`App::floating_*` 的索引）。
    Floating(usize),
}

/// cubic-bezier 缓动曲线 (x1, y1, x2, y2)。
pub type Curve = [f32; 4];

/// UWP 开曲线（弹层 / 面板展开、入场）。参 Kanesumi docs/UWP_PRIMARY_SOURCES.md。
pub const CURVE_UWP_OPEN: Curve = [0.1, 0.9, 0.2, 1.0];
/// UWP 关曲线（收起、离场）。
pub const CURVE_UWP_CLOSE: Curve = [0.7, 0.0, 1.0, 0.5];

/// 一次合成器动画：从当前呈现值过渡到 `to_*`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayerAnim {
    /// App 自选序号，完成 / 打断事件原样带回。
    pub serial: u32,
    pub duration_ms: u32,
    pub delay_ms: u32,
    pub curve: Curve,
    /// 终点视觉偏移（逻辑像素，叠加在图层矩形位置上）。
    pub to_x: f32,
    pub to_y: f32,
    /// 终点不透明度 0..1。
    pub to_opacity: f32,
}

/// App → 外壳的图层命令。
#[derive(Debug, Clone)]
pub enum LayerCommand {
    Create { id: LayerId, parent: LayerParent, rect: Rect },
    /// 设置内容并光栅一次（尺寸 = 图层矩形）。只在内容真的变了时发。
    SetContent { id: LayerId, scene: Scene },
    /// 改位置 / 尺寸（尺寸变化后需重新 SetContent）。
    SetRect { id: LayerId, rect: Rect },
    /// 立即设视觉偏移（取消进行中的动画）。
    SetOffset { id: LayerId, x: f32, y: f32 },
    /// 立即设不透明度（取消进行中的动画）。
    SetOpacity { id: LayerId, opacity: f32 },
    Animate { id: LayerId, anim: LayerAnim },
    Destroy { id: LayerId },
}

/// 外壳 → App 的图层事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerEvent {
    Done { id: LayerId, serial: u32 },
    Cancelled { id: LayerId, serial: u32 },
    /// 合成器不支持 `ether_composition_v1`（只发一次）：动画会被直接跳到终值。
    Unsupported,
}

/// App 侧命令队列：推命令，外壳每帧 `take`。
#[derive(Debug, Default)]
pub struct LayerQueue {
    commands: Vec<LayerCommand>,
}

impl LayerQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, c: LayerCommand) {
        self.commands.push(c);
    }

    pub fn create(&mut self, id: LayerId, parent: LayerParent, rect: Rect) {
        self.push(LayerCommand::Create { id, parent, rect });
    }

    pub fn set_content(&mut self, id: LayerId, scene: Scene) {
        self.push(LayerCommand::SetContent { id, scene });
    }

    pub fn animate(&mut self, id: LayerId, anim: LayerAnim) {
        self.push(LayerCommand::Animate { id, anim });
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// 外壳取走全部命令（保持推入顺序）。
    pub fn take(&mut self) -> Vec<LayerCommand> {
        std::mem::take(&mut self.commands)
    }
}

/// 回落语义（合成器无 `ether_composition_v1`）：一条 Animate 立即完成。
/// 返回应立即回报的事件。平台实现与单测共用。
pub fn fallback_events(id: LayerId, anim: &LayerAnim) -> [LayerEvent; 1] {
    [LayerEvent::Done { id, serial: anim.serial }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_preserves_order_and_drains() {
        let mut q = LayerQueue::new();
        let id = LayerId(7);
        q.create(id, LayerParent::Floating(0), Rect::new(0.0, 0.0, 10.0, 10.0));
        q.set_content(id, Scene::default());
        q.animate(
            id,
            LayerAnim { serial: 3, duration_ms: 350, delay_ms: 25, curve: CURVE_UWP_OPEN, to_x: 0.0, to_y: 0.0, to_opacity: 1.0 },
        );
        let cmds = q.take();
        assert_eq!(cmds.len(), 3);
        assert!(matches!(cmds[0], LayerCommand::Create { .. }));
        assert!(matches!(cmds[2], LayerCommand::Animate { anim: LayerAnim { serial: 3, .. }, .. }));
        assert!(q.is_empty(), "take 后清空");
    }

    #[test]
    fn fallback_completes_immediately() {
        let anim = LayerAnim { serial: 9, duration_ms: 200, delay_ms: 0, curve: CURVE_UWP_CLOSE, to_x: 0.0, to_y: 40.0, to_opacity: 0.0 };
        assert_eq!(fallback_events(LayerId(1), &anim), [LayerEvent::Done { id: LayerId(1), serial: 9 }]);
    }
}
