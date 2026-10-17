// wake.rs —— 让外部线程（D-Bus 订阅线程 / 工作线程）唤醒主循环。
//
// 主循环阻塞在 `event_loop.dispatch(timeout, …)`；外部线程往 mpsc 通道塞消息**不会**唤醒它，
// 只能等超时到期或 Wayland 事件——`ether-launcher` 的 Super 开合因此白等一拍（实测触发→提交
// 30 ms，参 docs/research/start_latency/NOTES.md）。
//
// 外壳启动时已经把 calloop ping 接到事件循环（解码就绪用的同一条桥），本模块把它暴露成一个
// 与 App 无关的全局句柄：任何线程调 `request()` 即可让主循环立刻醒来跑一轮 step / 渲染。

use std::sync::{Arc, Mutex, OnceLock};

type Hook = Arc<dyn Fn() + Send + Sync + 'static>;

static HOOK: OnceLock<Mutex<Option<Hook>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<Hook>> {
    HOOK.get_or_init(|| Mutex::new(None))
}

/// 外壳注册唤醒实现（进程内只应有一个入口；重复注册以后者为准）。
pub fn set_hook(hook: Hook) {
    if let Ok(mut g) = slot().lock() {
        *g = Some(hook);
    }
}

/// 请求主循环尽快醒来（线程安全、可重复调用；未初始化时静默丢弃）。
///
/// 典型用法：D-Bus 订阅线程把消息塞进通道后立刻调一次，主循环随即处理，不必等 16 ms 节拍。
pub fn request() {
    let hook = slot().lock().ok().and_then(|g| g.clone());
    if let Some(h) = hook {
        h();
    }
}

/// 是否已接上（供自检日志使用）。
pub fn is_installed() -> bool {
    slot().lock().map(|g| g.is_some()).unwrap_or(false)
}
