// frame_clock.rs —— 客户端帧时钟、出帧节流状态机与输入按帧合并（F3）。
//
// 参 Ether docs/SMOOTHNESS_PLAN.md §Ⅰ 第 2 类采样错位、§Ⅲ-3 客户端帧循环。
// - 呈现时钟采样：动画按本帧「将显示的时刻」取值，dt = predicted(本帧) − predicted(上一帧)；
// - 出帧节奏：动画进行中只在 frame 回调到达后出一帧，100ms 超时防冻结；
// - 输入按帧合并：移动 / 滚动在两帧间累积，出帧前一次性交付，按钮与键保持顺序。

use std::time::{Duration, Instant};
use crate::app::InputEvent;

/// `CLOCK_MONOTONIC` 绝对纳秒采样。`wp_presentation` 的时间戳时间域（clock_id=1）。
pub fn monotonic_now() -> Duration {
    #[cfg(target_os = "linux")]
    {
        let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
        // SAFETY: ts 为栈上合法 timespec。
        unsafe {
            libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
        }
        Duration::new(ts.tv_sec.max(0) as u64, ts.tv_nsec.max(0) as u32)
    }
    #[cfg(not(target_os = "linux"))]
    {
        Duration::from_nanos(0)
    }
}

/// `Instant` ↔ `CLOCK_MONOTONIC` 绝对时间的双向换算基准。
#[derive(Debug, Clone, Copy)]
pub struct MonoBase {
    pub(crate) instant0: Instant,
    pub(crate) mono0: Duration,
}

impl MonoBase {
    pub fn capture() -> Self {
        let mono0 = monotonic_now();
        let instant0 = Instant::now();
        Self { instant0, mono0 }
    }

    /// `Instant` → CLOCK_MONOTONIC 绝对时间。
    pub fn mono_of(&self, t: Instant) -> Duration {
        if t >= self.instant0 {
            self.mono0 + t.saturating_duration_since(self.instant0)
        } else {
            self.mono0.saturating_sub(self.instant0.saturating_duration_since(t))
        }
    }

    /// CLOCK_MONOTONIC 绝对时间 → `Instant`（硬件时间戳换算）。
    pub fn instant_of(&self, mono: Duration) -> Instant {
        if mono >= self.mono0 {
            self.instant0 + (mono - self.mono0)
        } else {
            let delta = self.mono0 - mono;
            self.instant0.checked_sub(delta).unwrap_or(self.instant0)
        }
    }
}

/// 客户端呈现帧时钟（不对外暴露新类型）。
#[derive(Debug, Clone)]
pub struct FrameClock {
    period: Duration,
    mono_base: MonoBase,
    last_feedback_presented: Option<Instant>,
    fresh_feedback: bool,
    last_feedback_discarded: bool,
    grid_anchor: Option<Instant>,
    last_frame_callback: Option<Instant>,
    fresh_frame_callback: bool,
    last_predicted_present: Option<Instant>,
    current_predicted_present: Option<Instant>,
}

impl Default for FrameClock {
    fn default() -> Self {
        Self::new(Duration::from_nanos(16_666_667))
    }
}

impl FrameClock {
    pub fn new(period: Duration) -> Self {
        Self {
            period: if period.is_zero() { Duration::from_nanos(16_666_667) } else { period },
            mono_base: MonoBase::capture(),
            last_feedback_presented: None,
            fresh_feedback: false,
            last_feedback_discarded: false,
            grid_anchor: None,
            last_frame_callback: None,
            fresh_frame_callback: false,
            last_predicted_present: None,
            current_predicted_present: None,
        }
    }

    pub fn period(&self) -> Duration {
        self.period
    }

    pub fn set_period(&mut self, period: Duration) {
        if !period.is_zero() {
            self.period = period;
        }
    }

    pub fn mono_base(&self) -> &MonoBase {
        &self.mono_base
    }

    /// 收到 `wp_presentation_feedback.presented`：更新周期并记下实际上屏时刻。
    pub fn on_presented(&mut self, presented_at: Instant, refresh_ns: u32) {
        let new_period = if refresh_ns > 0 {
            Duration::from_nanos(refresh_ns as u64)
        } else {
            self.period
        };
        let period_changed = new_period != self.period;
        self.period = new_period;
        self.last_feedback_presented = Some(presented_at);
        self.fresh_feedback = true;
        self.last_feedback_discarded = false;

        let p_nanos = self.period.as_nanos().max(1);
        match self.grid_anchor {
            Some(anchor) if !period_changed => {
                // 与现有网格相差超过 1/8 周期（跳相）→ 重锚；锚点满 1 s 也换成最新回执，
                // 吸收 refresh_ns 取整与时钟漂移的累积（否则锚点永不更新，数小时后预测偏离真 vblank）。
                // 其余情况保留锚点，相位稳定，不随回执时间戳的微小抖动漂移。参 SMOOTHNESS_PLAN §Ⅶ-2。
                let diff_nanos = if presented_at >= anchor {
                    let rem = presented_at.duration_since(anchor).as_nanos() % p_nanos;
                    rem.min(p_nanos - rem)
                } else {
                    let rem = anchor.duration_since(presented_at).as_nanos() % p_nanos;
                    rem.min(p_nanos - rem)
                };
                let stale = presented_at.saturating_duration_since(anchor) >= Duration::from_secs(1);
                if diff_nanos > p_nanos / 8 || stale {
                    self.grid_anchor = Some(presented_at);
                }
            }
            _ => {
                self.grid_anchor = Some(presented_at);
            }
        }
    }

    /// 收到 `wp_presentation_feedback.discarded`：丢弃当前回执预期。
    pub fn on_discarded(&mut self) {
        self.fresh_feedback = false;
        self.last_feedback_discarded = true;
        self.grid_anchor = None;
    }

    /// 收到 Wayland `wl_surface.frame` 回调。
    pub fn on_frame_callback(&mut self, cb_at: Instant) {
        self.last_frame_callback = Some(cb_at);
        self.fresh_frame_callback = true;
    }

    /// 预测下一帧的目标呈现时刻（退回层次：回执网格 → 回调网格 → now）。
    pub fn predict_next_present(&mut self, now: Instant) -> Instant {
        if !self.last_feedback_discarded && let Some(anchor) = self.grid_anchor {
            let p = self.period.as_nanos().max(1);
            if now <= anchor {
                return anchor + self.period;
            }
            let elapsed = now.saturating_duration_since(anchor).as_nanos();
            let k = (elapsed / p + 1) as u32;
            return anchor + self.period.saturating_mul(k);
        }
        if let Some(cb) = self.last_frame_callback {
            let p = self.period.as_nanos().max(1);
            if now <= cb {
                return cb + self.period;
            }
            let elapsed = now.saturating_duration_since(cb).as_nanos();
            let k = (elapsed / p + 1) as u32;
            return cb + self.period.saturating_mul(k);
        }
        now
    }

    /// 出一帧：计算预测呈现时刻，并得出夹在 `[0, 0.05 s]` 的 dt。
    pub fn advance_frame(&mut self, now: Instant) -> f64 {
        let predicted = self.predict_next_present(now);
        let dt = match self.last_predicted_present {
            Some(last) => {
                if predicted > last {
                    predicted.duration_since(last).as_secs_f64()
                } else {
                    self.period.as_secs_f64()
                }
            }
            None => self.period.as_secs_f64(),
        };
        let clamped = dt.clamp(0.0, 0.05);
        self.last_predicted_present = Some(predicted);
        self.current_predicted_present = Some(predicted);
        clamped
    }

    pub fn current_predicted_present(&self) -> Option<Instant> {
        self.current_predicted_present
    }
}

/// 表面出帧节流状态机：一个回调最多出一帧；100ms 超时防冻结；非动画合并等回调。
#[derive(Debug, Clone)]
pub struct PacingThrottle {
    callback_pending: bool,
    requested_at: Option<Instant>,
    callback_received: bool,
    rendered_in_cycle: bool,
}

impl Default for PacingThrottle {
    fn default() -> Self {
        Self::new()
    }
}

impl PacingThrottle {
    pub fn new() -> Self {
        Self {
            callback_pending: false,
            requested_at: None,
            callback_received: false,
            rendered_in_cycle: false,
        }
    }

    pub fn callback_pending(&self) -> bool {
        self.callback_pending
    }

    /// 已请求下一帧回调。
    pub fn on_request_callback(&mut self, now: Instant) {
        self.callback_pending = true;
        self.requested_at = Some(now);
        self.callback_received = false;
    }

    /// 收到 frame 回调。
    pub fn on_frame_callback(&mut self) {
        self.callback_pending = false;
        self.callback_received = true;
        self.rendered_in_cycle = false;
    }

    /// 检查当前是否允许渲染该表面。
    /// 返回 `(can_render, is_timeout)`。
    pub fn can_render(&mut self, now: Instant, animating: bool) -> (bool, bool) {
        // 100ms 超时防冻结（记日志 frame_cb_timeout）
        if self.callback_pending
            && let Some(req) = self.requested_at
            && now.saturating_duration_since(req) >= Duration::from_millis(100)
        {
            return (true, true);
        }

        if animating {
            // 动画进行中：只在 frame 回调到达后出下一帧；一个回调最多一帧。
            if self.rendered_in_cycle {
                (false, false)
            } else if !self.callback_pending {
                // 首帧 / 无挂起回调：允许渲染
                (true, false)
            } else if self.callback_received {
                (true, false)
            } else {
                (false, false)
            }
        } else {
            // 非动画的一次性重画（输入导致）：照旧立即出；但若本周期已出过一帧且回调未到，则等回调（合并）。
            if self.callback_pending && !self.callback_received {
                (false, false)
            } else {
                (true, false)
            }
        }
    }

    /// 记录本周期已出一帧。
    pub fn on_frame_rendered(&mut self) {
        self.rendered_in_cycle = true;
        self.callback_received = false;
    }
}

/// 输入按帧合并队列：指针移动 / 轴事件在两帧之间累积，出帧前一次性交付给 App。
#[derive(Debug, Default, Clone)]
pub struct InputCoalescer {
    events: Vec<(Option<usize>, InputEvent)>,
}

impl InputCoalescer {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn push(&mut self, target: Option<usize>, event: InputEvent) {
        match &event {
            InputEvent::PointerMoved { x, y } => {
                if let Some((last_tgt, InputEvent::PointerMoved { x: lx, y: ly })) = self.events.last_mut()
                    && *last_tgt == target
                {
                    *lx = *x;
                    *ly = *y;
                    return;
                }
            }
            InputEvent::Scroll { x, y, modifiers } => {
                if let Some((last_tgt, InputEvent::Scroll { x: lx, y: ly, modifiers: lm })) = self.events.last_mut()
                    && *last_tgt == target
                    && *lm == *modifiers
                {
                    *lx += *x;
                    *ly += *y;
                    return;
                }
            }
            InputEvent::ScrollInput(scroll) => {
                if let Some((last_tgt, InputEvent::ScrollInput(ls))) = self.events.last_mut()
                    && *last_tgt == target
                    && ls.source == scroll.source
                    && ls.phase == kanesumi_element::ScrollPhase::Update
                    && scroll.phase == kanesumi_element::ScrollPhase::Update
                    && ls.modifiers == scroll.modifiers
                {
                    ls.dx += scroll.dx;
                    ls.dy += scroll.dy;
                    return;
                }
            }
            _ => {}
        }
        self.events.push((target, event));
    }

    pub fn drain(&mut self) -> Vec<(Option<usize>, InputEvent)> {
        std::mem::take(&mut self.events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Modifiers, PointerButton};

    #[test]
    fn test_frame_clock_dt_sequences() {
        let t0 = Instant::now();
        let p60 = Duration::from_nanos(16_666_667);

        // 1. 有回执（wp_presentation feedback）情况
        let mut clock = FrameClock::new(p60);
        clock.on_presented(t0, 16_666_667);
        let dt0 = clock.advance_frame(t0);
        assert!((dt0 - 1.0 / 60.0).abs() < 1e-4, "首帧 dt 应约为 1/60s：{dt0}");

        let t1 = t0 + p60;
        clock.on_presented(t1, 16_666_667);
        let dt1 = clock.advance_frame(t1);
        assert!((dt1 - 1.0 / 60.0).abs() < 1e-4, "有回执序列帧 1 应稳定：{dt1}");

        let t2 = t1 + p60;
        clock.on_presented(t2, 16_666_667);
        let dt2 = clock.advance_frame(t2);
        assert!((dt2 - 1.0 / 60.0).abs() < 1e-4, "有回执序列帧 2 应稳定：{dt2}");

        // 2. 只有回调（无 wp_presentation）退回情况
        let mut clock_cb = FrameClock::new(p60);
        clock_cb.on_frame_callback(t0);
        let cdt0 = clock_cb.advance_frame(t0);
        assert!((cdt0 - 1.0 / 60.0).abs() < 1e-4);

        let cb1 = t0 + p60;
        clock_cb.on_frame_callback(cb1);
        let cdt1 = clock_cb.advance_frame(cb1);
        assert!((cdt1 - 1.0 / 60.0).abs() < 1e-4, "只有回调序列帧 1 应稳定：{cdt1}");

        // 3. 都没有（直接退回 now）情况
        let mut clock_none = FrameClock::new(p60);
        let ndt0 = clock_none.advance_frame(t0);
        assert!((ndt0 - 1.0 / 60.0).abs() < 1e-4);

        let t_next = t0 + Duration::from_millis(20);
        let ndt1 = clock_none.advance_frame(t_next);
        assert!((ndt1 - 0.020).abs() < 1e-4, "无回调回执时 dt 应跟随 now 墙钟差：{ndt1}");
    }

    #[test]
    fn test_frame_clock_refresh_rate_change() {
        let t0 = Instant::now();
        let p60 = Duration::from_nanos(16_666_667);
        let p120 = Duration::from_nanos(8_333_333);

        let mut clock = FrameClock::new(p60);
        clock.on_presented(t0, 16_666_667);
        let _ = clock.advance_frame(t0);

        // 周期从 60 Hz 切到 120 Hz
        let t1 = t0 + p120;
        clock.on_presented(t1, 8_333_333);
        let dt = clock.advance_frame(t1);
        assert_eq!(clock.period(), p120, "周期应更新为 120 Hz");
        assert!((dt - 1.0 / 120.0).abs() < 1e-4, "dt 应立刻跟随 120 Hz：{dt}");
    }

    #[test]
    fn test_pacing_throttle_one_frame_per_callback() {
        let t0 = Instant::now();
        let mut throttle = PacingThrottle::new();

        // 动画进行中：首帧允许
        assert_eq!(throttle.can_render(t0, true), (true, false));
        throttle.on_request_callback(t0);
        throttle.on_frame_rendered();

        // 回调到达前不得出下一帧
        assert_eq!(throttle.can_render(t0 + Duration::from_millis(8), true), (false, false));
        assert_eq!(throttle.can_render(t0 + Duration::from_millis(15), true), (false, false));

        // 回调到达：允许出一帧
        throttle.on_frame_callback();
        assert_eq!(throttle.can_render(t0 + Duration::from_millis(16), true), (true, false));
        throttle.on_frame_rendered();

        // 一个回调最多出一帧：同一回调下再次查询必须拒绝
        assert_eq!(throttle.can_render(t0 + Duration::from_millis(17), true), (false, false));

        // 100ms 超时防冻结
        throttle.on_request_callback(t0);
        let (can, timeout) = throttle.can_render(t0 + Duration::from_millis(101), true);
        assert!(can, "100ms 超时应放行渲染防冻结");
        assert!(timeout, "超时标志应置位");

        // 非动画一次性重画：本周期已出一帧且回调未到时等回调合并
        let mut t_input = PacingThrottle::new();
        t_input.on_request_callback(t0);
        assert_eq!(t_input.can_render(t0, false), (false, false), "回调未到且挂起时等回调");
        t_input.on_frame_callback();
        assert_eq!(t_input.can_render(t0, false), (true, false), "回调到达后立即允许");
    }

    #[test]
    fn test_input_coalescing() {
        let mut coalescer = InputCoalescer::new();
        let target = Some(0);

        // 移动累积、按钮不合并、顺序保持
        coalescer.push(target, InputEvent::PointerMoved { x: 10.0, y: 10.0 });
        coalescer.push(target, InputEvent::PointerMoved { x: 20.0, y: 20.0 });
        coalescer.push(
            target,
            InputEvent::PointerPressed {
                x: 20.0,
                y: 20.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
        );
        coalescer.push(target, InputEvent::PointerMoved { x: 30.0, y: 30.0 });
        coalescer.push(target, InputEvent::PointerMoved { x: 40.0, y: 40.0 });
        coalescer.push(
            target,
            InputEvent::PointerReleased {
                x: 40.0,
                y: 40.0,
                button: PointerButton::Left,
                modifiers: Modifiers::default(),
            },
        );
        coalescer.push(target, InputEvent::PointerMoved { x: 50.0, y: 50.0 });

        let drained = coalescer.drain();
        assert_eq!(drained.len(), 5, "移动累积后应只有 5 个事件");
        assert_eq!(drained[0].1, InputEvent::PointerMoved { x: 20.0, y: 20.0 });
        assert!(matches!(drained[1].1, InputEvent::PointerPressed { .. }));
        assert_eq!(drained[2].1, InputEvent::PointerMoved { x: 40.0, y: 40.0 });
        assert!(matches!(drained[3].1, InputEvent::PointerReleased { .. }));
        assert_eq!(drained[4].1, InputEvent::PointerMoved { x: 50.0, y: 50.0 });

        // 滚轮累积
        coalescer.push(
            target,
            InputEvent::Scroll {
                x: 0.0,
                y: 15.0,
                modifiers: Modifiers::default(),
            },
        );
        coalescer.push(
            target,
            InputEvent::Scroll {
                x: 0.0,
                y: 25.0,
                modifiers: Modifiers::default(),
            },
        );
        let drained_scroll = coalescer.drain();
        assert_eq!(drained_scroll.len(), 1);
        assert_eq!(
            drained_scroll[0].1,
            InputEvent::Scroll {
                x: 0.0,
                y: 40.0,
                modifiers: Modifiers::default()
            }
        );
    }

    #[test]
    fn test_frame_clock_periodic_phase_alignment() {
        let t0 = Instant::now();
        let p60 = Duration::from_nanos(16_666_667);
        let mut clock = FrameClock::new(p60);
        clock.on_presented(t0, 16_666_667);

        // 模拟连续 60 帧动画，并在两帧之间偶发微小的调度/休眠抖动（50-100µs）
        let mut predicted_times = Vec::new();
        for k in 0..60 {
            let jitter = Duration::from_micros((k * 17 % 80) as u64);
            let frame_now = t0 + p60 * k + jitter;
            // 收到上屏回执
            clock.on_presented(t0 + p60 * k + jitter, 16_666_667);
            let _dt = clock.advance_frame(frame_now);
            predicted_times.push(clock.current_predicted_present().unwrap());
        }

        let first = predicted_times[0];
        let p_sec = p60.as_secs_f64();
        let mut hits = 0;
        for &t in &predicted_times {
            let diff = t.duration_since(first).as_secs_f64();
            let ratio = diff / p_sec;
            let nearest = ratio.round();
            if (ratio - nearest).abs() < 0.01 {
                hits += 1;
            }
        }
        let hit_ratio = hits as f64 / predicted_times.len() as f64;
        assert!(hit_ratio >= 0.99, "周期整数倍命中率须 >= 99%：{hit_ratio}");
        assert_eq!(hit_ratio, 1.0, "网格锚定应达到 100% 整数倍命中");
    }

    #[test]
    fn grid_anchor_rebases_on_phase_jump_and_age() {
        let t0 = Instant::now();
        let p = Duration::from_nanos(16_666_667);
        let mut clock = FrameClock::new(p);
        clock.on_presented(t0, 16_666_667);
        // 小抖动（< 1/8 周期）：锚点不动，预测仍落在 t0 网格上。
        clock.on_presented(t0 + p + Duration::from_micros(80), 16_666_667);
        assert_eq!(clock.predict_next_present(t0 + p + Duration::from_millis(1)), t0 + p * 2);
        // 跳相 5 ms（> 1/8 周期）：改锚到新回执。
        let jump = t0 + p * 3 + Duration::from_millis(5);
        clock.on_presented(jump, 16_666_667);
        assert_eq!(clock.predict_next_present(jump + Duration::from_millis(1)), jump + p);
        // 锚点满 1 s：即便只差 30 µs 也换成最新回执，吸收漂移。
        let late = jump + p * 61 + Duration::from_micros(30);
        clock.on_presented(late, 16_666_667);
        assert_eq!(clock.predict_next_present(late + Duration::from_millis(1)), late + p);
    }
}

