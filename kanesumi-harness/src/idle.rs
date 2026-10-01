// idle.rs —— 主循环空闲唤醒计算（跨平台纯函数，参任务 k-perf 第 5 步）。
//
// 旧主循环：脏时 16ms、空闲固定 100ms 轮询 —— 静止时仍每秒醒 10 次跑 `step`。
// 现在把「下一次唤醒的时刻」抽成纯函数：有待渲染内容才保留 16ms 帧兜底（I-2 不冻结），
// 空闲时阻塞到「下一个定时器」与「系统主题检测节流点」中较早者，但不超过 `IDLE_MAX`
//（App 在 update 里轮询的非 Wayland 通道没有事件唤醒）。

use std::time::Duration;

/// 有内容待呈现时的帧兜底：16ms（≈60fps）。仅在 `busy` 时使用（I-2）。
pub const FRAME_FALLBACK: Duration = Duration::from_millis(16);
/// 系统主题（Chorus `theme.toml`）变更检测的节流间隔。
pub const THEME_POLL: Duration = Duration::from_millis(500);
/// 空闲阻塞上限：App 在 `update` 里轮询的非 Wayland 通道（Launcher 的 Super 开合套接字、
/// sysstate / D-Bus 线程、后台文件操作结果）没有事件唤醒，等太久就是可感知的延迟。
/// 50ms：比旧的固定 100ms 轮询更跟手，空闲每秒 20 次轻量 `step`。
pub const IDLE_MAX: Duration = Duration::from_millis(50);

/// 计算事件循环本次 `dispatch` 的超时（`None` = 阻塞到下一个事件）。
///
/// - `busy`：确有待渲染内容（脏 / 浮层脏 / 动画推进中）→ `Some(FRAME_FALLBACK)`；
/// - 否则取「下一个定时器」与「主题检测节流点」较早者；
/// - 一律不超过 `IDLE_MAX`。
///
/// `next_timer` 为秒；非有限或负值视为无定时器。
pub fn next_wake(
    busy: bool,
    next_timer: Option<f64>,
    theme_poll: Option<Duration>,
) -> Option<Duration> {
    if busy {
        return Some(FRAME_FALLBACK);
    }
    let timer = next_timer
        .filter(|s| s.is_finite() && *s >= 0.0)
        .map(Duration::from_secs_f64);
    let wake = match (timer, theme_poll) {
        (Some(t), Some(p)) => t.min(p),
        (Some(t), None) => t,
        (None, Some(p)) => p,
        (None, None) => IDLE_MAX,
    };
    Some(wake.min(IDLE_MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn busy_keeps_frame_fallback() {
        assert_eq!(
            next_wake(true, Some(10.0), Some(THEME_POLL)),
            Some(FRAME_FALLBACK)
        );
    }

    #[test]
    fn idle_uses_earliest_wake_capped() {
        // 定时器 10ms 早于上限 → 取定时器。
        assert_eq!(
            next_wake(false, Some(0.01), Some(THEME_POLL)),
            Some(Duration::from_millis(10))
        );
        // 定时器 / 主题都晚于上限 → 上限（非 Wayland 通道的轮询延迟不超过 IDLE_MAX）。
        assert_eq!(next_wake(false, Some(2.0), Some(THEME_POLL)), Some(IDLE_MAX));
        assert_eq!(next_wake(false, None, Some(THEME_POLL)), Some(IDLE_MAX));
    }

    #[test]
    fn nothing_pending_still_wakes_at_cap() {
        assert_eq!(next_wake(false, None, None), Some(IDLE_MAX));
    }

    #[test]
    fn invalid_timer_is_ignored() {
        assert_eq!(next_wake(false, Some(f64::NAN), None), Some(IDLE_MAX));
        assert_eq!(next_wake(false, Some(-1.0), None), Some(IDLE_MAX));
        assert_eq!(next_wake(false, Some(f64::INFINITY), None), Some(IDLE_MAX));
    }
}
