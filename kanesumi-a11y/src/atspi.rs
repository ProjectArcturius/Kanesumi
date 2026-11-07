// atspi —— Linux 读屏（AT-SPI）适配器：把数据层的树交给 AccessKit 的 Unix 后端。
//
// 编译已验证：`cargo check -p kanesumi-a11y --features at-spi` 在 Windows 上就能编过
// （`accesskit_unix` 是纯 Rust），所以这份代码的类型与 API 用法不是「写着看看」。
// 尚未验证的是**运行时**：真的连上 AT-SPI 总线、读屏（Orca）能读出界面，那要 Linux 真机。
// 接线位置：外壳每帧 `Tree::frame()` 之后调 `bridge.publish(update_for_tree(&tree))`。
//
// 为什么默认关：接上 AT-SPI 会把一条 D-Bus 线程与一套 D-Bus 对象树拉进进程。
// 不接无障碍的客户端（合成器里的普通窗口）不该为此付代价，故用 feature 门控。

use accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, TreeUpdate,
};
use accesskit_unix::Adapter;

use crate::SnapshotSlot;

/// AT-SPI 桥：外壳持有一个，每帧把最新的树 `publish` 进来。
pub struct AtspiBridge {
    adapter: Adapter,
    slot: SnapshotSlot,
}

impl AtspiBridge {
    /// 装上 AT-SPI。读屏在**别的线程**上被唤醒时才回来要树（惰性激活），所以这里不必
    /// 预先算好树：没有读屏的会话里这行代码只登记一对回调。
    pub fn install() -> Self {
        Self::with_slot(SnapshotSlot::new())
    }

    /// 挂到**已有的**快照槽上：外壳自己每帧发布到那个槽（跨平台、可单测），
    /// 适配器只负责把槽里的树转给 AT-SPI。分成「发布」与「转交」两件事，是为了让
    /// 发布路径能在任意平台测试，而这条适配器路径只在 Linux 上真的跑起来。
    pub fn with_slot(slot: SnapshotSlot) -> Self {
        let adapter = Adapter::new(
            Activation { slot: slot.clone() },
            IgnoreActions,
            Deactivate,
        );
        Self { adapter, slot }
    }

    /// 把槽里最新的树推给 AT-SPI。
    ///
    /// 只有读屏已激活时才会真的发出去（这个调用在未激活时什么也不做）—— 所以每帧调它
    /// 对「没人在听」的会话没有代价。
    pub fn flush(&mut self) {
        let slot = self.slot.clone();
        self.adapter.update_if_active(|| {
            slot.current().expect("flush 之前槽里必须有树")
        });
    }

    /// 发布 + 转交（不关心槽的持有者时用这个）。
    pub fn publish(&mut self, update: TreeUpdate) {
        self.slot.publish(update);
        self.flush();
    }

    /// 所在窗口的焦点状态（Wayland 下拿不到窗口位置，故不设 root window bounds）。
    pub fn set_window_focused(&mut self, focused: bool) {
        self.adapter.update_window_focus_state(focused);
    }

    pub fn slot(&self) -> &SnapshotSlot {
        &self.slot
    }
}

/// 读屏激活时来要首帧树：直接给最近发布的那份。
struct Activation {
    slot: SnapshotSlot,
}

impl ActivationHandler for Activation {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        self.slot.current()
    }
}

/// 动作回灌尚未实现（本轮只读桥）。**什么也不做**是协议要求的行为：
/// `ActionHandler::do_action` 对不支持的动作就必须「什么都不做」。
///
/// 也正因为如此，`update()` 里不声明任何 `Action`（参 src/lib.rs 头注）：声明了却做不到，
/// 读屏会显示一堆按了没反应的按钮。等动作回灌通了，这里改成把请求投回元素树事件队列，
/// 同时补上动作声明 —— 那时是纯新增。
struct IgnoreActions;

impl ActionHandler for IgnoreActions {
    fn do_action(&mut self, _request: ActionRequest) {}
}

struct Deactivate;

impl DeactivationHandler for Deactivate {
    fn deactivate_accessibility(&mut self) {}
}
