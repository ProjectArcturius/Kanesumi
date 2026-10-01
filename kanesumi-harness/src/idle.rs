// idle.rs —— 主循环空闲唤醒计算（跨平台纯函数，参任务 k-perf 第 5 步）。
//
// 旧主循环：脏时 16ms、空闲固定 100ms 轮询 —— 静止时仍每秒醒 10 次跑 `step`。
// 现在把「下一次唤醒的时刻」抽成纯函数：有待渲染内容才保留 16ms 帧兜底（I-2 不冻结），
// 空闲时阻塞到「下一个定时器」与「系统主题检测节流点」中较早者；两者皆无则可无限阻塞
// （`None`），由 Wayland 事件唤醒。

use std::time::Duration;

/// 有内容待呈现时的帧兜底：16ms（≈60fps）。仅在 `busy` 时使用（I-2）。
pub const FRAME_FALLBACK: Duration = Duration::from_millis(16);
/// 系统主题（Chorus `theme.toml`）变更检测的节流间隔。
pub const THEME_POLL: Duration = Duration::from_millis(500);

/// 计算事件循环本次 `dispatch` 的超时（`None` = 阻塞到下一个事件）。
///
/// - `busy`：确有待渲染内容（脏 / 浮层脏 / 动画推进中）→ `Some(FRAME_FALLBACK)`；
/// - 否则取「下一个定时器」与「主题检测节流点」较早者；
/// - 两者皆无 → `None`（由事件唤醒）。
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
    match (timer, theme_poll) {
        (Some(t), Some(p)) => Some(t.min(p)),
        (Some(t), None) => Some(t),
        (None, Some(p)) => Some(p),
        (None, None) => None,
    }
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
    fn idle_uses_earliest_of_timer_and_theme() {
        // 定时器 0.1s 早于主题 0.5s。
        assert_eq!(
            next_wake(false, Some(0.1), Some(THEME_POLL)),
            Some(Duration::from_millis(100))
        );
        // 主题 0.5s 早于定时器 2s。
        assert_eq!(
            next_wake(false, Some(2.0), Some(THEME_POLL)),
            Some(THEME_POLL)
        );
    }

    #[test]
    fn timer_only_and_theme_only() {
        assert_eq!(
            next_wake(false, Some(0.25), None),
            Some(Duration::from_millis(250))
        );
        assert_eq!(next_wake(false, None, Some(THEME_POLL)), Some(THEME_POLL));
    }

    #[test]
    fn nothing_pending_blocks_forever() {
        assert_eq!(next_wake(false, None, None), None);
    }

    #[test]
    fn invalid_timer_is_ignored() {
        assert_eq!(next_wake(false, Some(f64::NAN), None), None);
        assert_eq!(next_wake(false, Some(-1.0), None), None);
        assert_eq!(next_wake(false, Some(f64::INFINITY), None), None);
    }
}
