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
        let slot = SnapshotSlot::new();
        let adapter = Adapter::new(
            Activation { slot: slot.clone() },
            IgnoreActions,
            Deactivate,
        );
        Self { adapter, slot }
    }

    /// 把本帧的树交给桥（无读屏时是纯内存操作：一次覆盖 + 一次惰性检查）。
    pub fn publish(&mut self, update: TreeUpdate) {
        self.slot.publish(update);
        let slot = self.slot.clone();
        // 只有读屏已经激活时才会真的把更新推出去；未激活时这个调用什么都不做。
        self.adapter
            .update_if_active(|| slot.current().expect("刚发布的树不应该丢了"));
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
