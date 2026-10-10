// snap_tracker.rs —— 手势进度源（吸附 / 投影选点 / 解析弹簧落定 / 打断）。
//
// 设计定稿：Ether `docs/GESTURE_PLAN.md` §Ⅳ（三指上滑拉起 Launcher 的进度源），
// 裁定 `docs/DECISIONS_2026-10-08.md` §129。
//
// 本模块纯逻辑、无平台依赖：时间一律由调用方给出单调秒；[`SnapTracker::sample`]
// 只依赖 `t`（不依赖采样频率），故 60 / 120 / 165 Hz 乃至随机时刻取值一致
// （参 AnimationRules §Ⅳ `render_state = resolve(progress)`、§Ⅶ 可中断）。
//
// 与 `sokuou` 的弹簧不同：此处按**绝对时刻的临界阻尼解析式**求值，不做 dt 积分——
// 逐帧积分会让采样频率影响曲线，违反「任意采样频率同一曲线」（GESTURE_PLAN §Ⅳ）。

use std::cell::Cell;

use crate::interaction::{
    GESTURE_DISTANCE, GESTURE_MIN_VELOCITY, GESTURE_VELOCITY_WINDOW_MS, INERTIA_TAU_S,
    SNAP_EPSILON, SNAP_OMEGA_CLOSE, SNAP_OMEGA_OPEN,
};

/// 落定的速度阈值（/s）：与位置阈值同判，静止才发 `Settled`（GESTURE_PLAN §Ⅳ.4）。
const SETTLE_VELOCITY: f64 = 0.05;
/// 求落定时刻的扫描步长（秒）与上限（秒），仅用于确定 `Settled` 的发生时刻。
const SETTLE_SCAN_STEP: f64 = 0.0005;
const SETTLE_SCAN_MAX: f64 = 10.0;

/// `SnapTracker` 的可调参数。`Default` = GESTURE_PLAN §Ⅳ 的正典值。
#[derive(Debug, Clone, PartialEq)]
pub struct SnapParams {
    /// 距离归一 D：libinput 手势增量单位（GNOME `TOUCHPAD_BASE_HEIGHT = 300`）。
    pub distance: f64,
    /// 松手速度估计窗口（秒；GNOME `EVENT_HISTORY_THRESHOLD_MS = 150`）。
    pub velocity_window_s: f64,
    /// 触发投影的最小速度（/s）：低于此值按最近点落定。
    pub min_velocity: f64,
    /// 投影时间常数 τ_proj（秒）：取 [`INERTIA_TAU_S`]，与滚动惯性统一物理。
    pub proj_tau_s: f64,
    /// 开方向（T > x₀）临界阻尼角频率（/s）。
    pub omega_open: f64,
    /// 关方向（T ≤ x₀）临界阻尼角频率（/s）。
    pub omega_close: f64,
    /// 落定位置阈值（GNOME `EPSILON = 0.005`）。
    pub epsilon: f64,
    /// 吸附点（升序）；缺省 `[0, 1]`。结构上支持多点。
    pub snap_points: Vec<f64>,
}

impl Default for SnapParams {
    fn default() -> Self {
        Self {
            distance: GESTURE_DISTANCE,
            velocity_window_s: GESTURE_VELOCITY_WINDOW_MS as f64 / 1000.0,
            min_velocity: GESTURE_MIN_VELOCITY,
            proj_tau_s: INERTIA_TAU_S as f64,
            omega_open: SNAP_OMEGA_OPEN,
            omega_close: SNAP_OMEGA_CLOSE,
            epsilon: SNAP_EPSILON,
            snap_points: vec![0.0, 1.0],
        }
    }
}

/// 进度变化的来源：手势 / 按键 / 客户端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// 手指跟手（swipe / hold）。
    Gesture,
    /// 系统按键（如 Super 拉起 Launcher）。
    Key,
    /// 客户端请求（协议 v3 的 `animate_to`）。
    Client,
}

/// 进度源对外事件（协议 v3 的 `begin` / `target` / `settled`），参 GESTURE_PLAN §Ⅴ。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SnapEvent {
    /// 一轮开始（来源）。
    Begin(Origin),
    /// 去向已定（吸附点）。
    Target(f64),
    /// 落定于该值。
    Settled(f64),
}

/// 内部运动状态：跟手 / 抓持（常量）、解析弹簧、已落定。
#[derive(Debug, Clone, Copy)]
enum Motion {
    /// 值固定（跟手或 `grab` 冻结），速度为 0。
    Hold { value: f64 },
    /// 临界阻尼弹簧，从 `(t0, x0, v0)` 落向 `target`，`settle_t` 为落定时刻。
    Spring {
        t0: f64,
        x0: f64,
        v0: f64,
        target: f64,
        omega: f64,
        settle_t: f64,
    },
    /// 已落定，`sample` 恒为 `target`。
    Settled { target: f64 },
}

/// 临界阻尼解析式在 `t` 处的 (值, 速度)：
/// `x(t) = T + (A + B·Δ)·e^{−ωΔ}`，`v(t) = (v₀ − ω·B·Δ)·e^{−ωΔ}`，
/// 其中 `A = x₀ − T`、`B = v₀ + ω·A`、`Δ = t − t₀`（GESTURE_PLAN §Ⅳ.4）。
fn eval_spring(t: f64, t0: f64, x0: f64, v0: f64, target: f64, omega: f64) -> (f64, f64) {
    let d = t - t0;
    let a = x0 - target;
    let b = v0 + omega * a;
    let e = (-omega * d).exp();
    (target + (a + b * d) * e, (v0 - omega * b * d) * e)
}

/// 求首个满足 `|x − T| < epsilon` 且 `|v| < 0.05` 的时刻（GESTURE_PLAN §Ⅳ.4）。
fn find_settle_t(t0: f64, x0: f64, v0: f64, target: f64, omega: f64, epsilon: f64) -> f64 {
    let mut d = 0.0;
    while d <= SETTLE_SCAN_MAX {
        let (x, v) = eval_spring(t0 + d, t0, x0, v0, target, omega);
        if (x - target).abs() < epsilon && v.abs() < SETTLE_VELOCITY {
            return t0 + d;
        }
        d += SETTLE_SCAN_STEP;
    }
    t0 + SETTLE_SCAN_MAX
}

/// 手势进度源。跟手 1:1；松手按速度投影选吸附点，再用临界阻尼解析弹簧接速度落定；
/// 任意时刻可被 `grab` / `finger_begin` / `animate_to` 打断而保持值连续。
#[derive(Debug)]
pub struct SnapTracker {
    params: SnapParams,
    motion: Motion,
    /// 本轮跟手的样本 `(t, p)`，`finger_begin` 清空、`finger_delta` 追加；用于估速度。
    samples: Vec<(f64, f64)>,
    /// 本轮跟手开始时的进度（`cancelled` 回退目标）。
    gesture_start: f64,
    /// 待取事件队列。
    events: Vec<SnapEvent>,
    /// 最近一次 `sample(t)` 的时间；`take_events` 据此判定弹簧是否已落定。
    /// `sample` 声明为 `&self`（取值是 `t` 的纯函数），故用 `Cell` 暂存这一缓存。
    last_sample_t: Cell<Option<f64>>,
}

impl SnapTracker {
    /// 以参数与初始进度建源；初始即视为已落定（`sample` 恒为 `initial`）。
    pub fn new(mut params: SnapParams, initial: f64) -> Self {
        if params.snap_points.len() < 2 {
            params.snap_points = vec![0.0, 1.0];
        }
        params.snap_points.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let initial = clamp_points(&params.snap_points, initial);
        Self {
            params,
            motion: Motion::Settled { target: initial },
            samples: Vec::new(),
            gesture_start: initial,
            events: Vec::new(),
            last_sample_t: Cell::new(None),
        }
    }

    /// 距 `x` 最近的吸附点（`x` 超出范围时即端点）。
    fn nearest_point(&self, x: f64) -> f64 {
        let mut best = self.params.snap_points[0];
        let mut best_d = (x - best).abs();
        for &p in &self.params.snap_points[1..] {
            let d = (x - p).abs();
            if d < best_d {
                best_d = d;
                best = p;
            }
        }
        best
    }

    /// 以 `(t, x0, v0)` 起弹落向 `target`。
    fn start_spring(&mut self, t: f64, x0: f64, v0: f64, target: f64) {
        let target = clamp_points(&self.params.snap_points, target);
        let omega = if target > x0 {
            self.params.omega_open
        } else {
            self.params.omega_close
        };
        let settle_t = find_settle_t(t, x0, v0, target, omega, self.params.epsilon);
        self.motion = Motion::Spring {
            t0: t,
            x0,
            v0,
            target,
            omega,
            settle_t,
        };
        self.last_sample_t.set(None);
    }

    /// 最近窗口内的 Δp/Δt（事件时间戳）；样本不足两点时用全部样本；无样本为 0。
    fn estimate_velocity(&self, t: f64) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let window_start = t - self.params.velocity_window_s;
        let recent: Vec<(f64, f64)> = self
            .samples
            .iter()
            .copied()
            .filter(|(ts, _)| *ts >= window_start)
            .collect();
        let used = if recent.len() >= 2 { &recent } else { &self.samples };
        let (t0, p0) = used[0];
        let (t1, p1) = used[used.len() - 1];
        let dt = t1 - t0;
        if dt <= 0.0 { 0.0 } else { (p1 - p0) / dt }
    }

    /// 按速度选去向（GESTURE_PLAN §Ⅳ.3）：慢速取最近点；否则投影取最近点，
    /// 但该点不得与速度方向相反，反向时改取速度方向上的下一个吸附点。
    fn select_target(&self, p: f64, v: f64) -> f64 {
        if v.abs() < self.params.min_velocity {
            return self.nearest_point(p);
        }
        let star = p + v * self.params.proj_tau_s;
        let cand = self.nearest_point(star);
        if (cand - p) * v < 0.0 {
            // 候选点在速度反方向：取速度方向上从 p 起的下一个吸附点。
            let next = if v > 0.0 {
                self.params.snap_points.iter().copied().find(|&q| q > p)
            } else {
                self.params.snap_points.iter().rev().copied().find(|&q| q < p)
            };
            return next.unwrap_or(cand);
        }
        cand
    }
}

/// 把 `x` 夹到吸附点范围内 `[min, max]`。
fn clamp_points(points: &[f64], x: f64) -> f64 {
    let min = points[0];
    let max = points[points.len() - 1];
    x.clamp(min, max)
}

impl SnapTracker {
    /// 在 `t` 时刻的 `(进度, 速度/秒)`。纯函数：只依赖 `t` 与当前运动参数，
    /// 与采样频率 / 调用次数无关；值夹在吸附点范围内。落定后恒为 `target`、速度为 0。
    pub fn sample(&self, t: f64) -> (f64, f64) {
        // 记下最近采样时刻，供 `take_events` 判定落定（见字段注释）。
        self.last_sample_t.set(Some(t));
        match self.motion {
            Motion::Hold { value } => (value, 0.0),
            Motion::Settled { target } => (target, 0.0),
            Motion::Spring {
                t0,
                x0,
                v0,
                target,
                omega,
                settle_t,
            } => {
                if t >= settle_t {
                    (target, 0.0)
                } else {
                    let (x, v) = eval_spring(t, t0, x0, v0, target, omega);
                    (clamp_points(&self.params.snap_points, x), v)
                }
            }
        }
    }

    /// 在 `t` 时刻是否已落定（弹簧的首个落定时刻已过，或本就静止）。
    pub fn is_settled(&self, t: f64) -> bool {
        match self.motion {
            Motion::Settled { .. } => true,
            Motion::Spring { settle_t, .. } => t >= settle_t,
            Motion::Hold { .. } => false,
        }
    }

    /// 取走并清空待发事件；在弹簧已（按最近 `sample` 时刻）落定时补发 `Settled`。
    pub fn take_events(&mut self) -> Vec<SnapEvent> {
        if let Motion::Spring {
            target, settle_t, ..
        } = self.motion
            && let Some(t) = self.last_sample_t.get()
            && t >= settle_t
        {
            self.motion = Motion::Settled { target };
            self.events.push(SnapEvent::Settled(target));
        }
        std::mem::take(&mut self.events)
    }

    /// 手指按下：打断当前（落定）运动，以 `sample(t)` 为起点、速度清零，开始新一轮跟手。
    pub fn finger_begin(&mut self, t: f64) {
        let (x, _) = self.sample(t);
        self.motion = Motion::Hold { value: x };
        self.gesture_start = x;
        self.samples.clear();
        self.samples.push((t, x));
        self.last_sample_t.set(None);
        self.events.push(SnapEvent::Begin(Origin::Gesture));
    }

    /// 手指移动：原始位移（libinput 单位）累加，除以 D 后夹在吸附点范围内。
    pub fn finger_delta(&mut self, t: f64, delta_units: f64) {
        let cur = if let Motion::Hold { value } = self.motion {
            value
        } else {
            self.sample(t).0
        };
        let np = clamp_points(
            &self.params.snap_points,
            cur + delta_units / self.params.distance,
        );
        self.motion = Motion::Hold { value: np };
        self.samples.push((t, np));
    }

    /// 手指松开：估速度 → 选吸附点 → 起弹落定。`cancelled` 时回到本轮跟手开始值。
    pub fn finger_end(&mut self, t: f64, cancelled: bool) {
        let (p, _) = self.sample(t);
        let (v, target) = if cancelled {
            (0.0, self.gesture_start)
        } else {
            let v = self.estimate_velocity(t);
            (v, self.select_target(p, v))
        };
        self.start_spring(t, p, v, target);
        self.events.push(SnapEvent::Target(target));
    }

    /// 按键 / 客户端：从 `sample(t)` 的值与速度起弹落向 `target`（速度连续、可打断进行中的落定）。
    pub fn animate_to(&mut self, t: f64, target: f64, origin: Origin) {
        let (x, v) = self.sample(t);
        self.events.push(SnapEvent::Begin(origin));
        self.events.push(SnapEvent::Target(target));
        self.start_spring(t, x, v, target);
    }

    /// hold 打断：冻结在 `sample(t)` 的值（速度清零），等待 `finger_begin` 或 `animate_to`。
    pub fn grab(&mut self, t: f64) {
        let (x, _) = self.sample(t);
        self.motion = Motion::Hold { value: x };
        self.samples.clear();
        self.last_sample_t.set(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interaction::{
        GESTURE_DISTANCE, GESTURE_MIN_VELOCITY, GESTURE_VELOCITY_WINDOW_MS, SNAP_EPSILON,
        SNAP_OMEGA_CLOSE, SNAP_OMEGA_OPEN,
    };

    /// 扫描首个满足 `f` 的时刻（步长 0.1 ms，够第 6 组 ±5 ms 的断言）。
    fn first_time(f: impl Fn(f64) -> bool, hi: f64) -> f64 {
        let mut t = 0.0;
        while t <= hi {
            if f(t) {
                return t;
            }
            t += 0.0001;
        }
        hi
    }

    /// 构造一个「先到位、末尾静止」的慢速手势，在 `p` 处松手，返回 tracker。
    fn slow_release_at(p: f64) -> SnapTracker {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.05, p * GESTURE_DISTANCE);
        // 末尾窗口内两次同值样本 → 估速度为 0（< v_min）。
        tr.finger_delta(0.40, 0.0);
        tr.finger_delta(0.50, 0.0);
        tr.finger_end(0.55, false);
        tr
    }

    #[test]
    fn gesture_constants_are_pinned() {
        assert_eq!(GESTURE_DISTANCE, 300.0);
        assert_eq!(GESTURE_VELOCITY_WINDOW_MS, 150);
        assert_eq!(GESTURE_MIN_VELOCITY, 2.0);
        assert_eq!(SNAP_OMEGA_OPEN, 18.97);
        assert_eq!(SNAP_OMEGA_CLOSE, 33.2);
        assert_eq!(SNAP_EPSILON, 0.005);
    }

    /// 1 跟手：delta 累加 ÷ 300，夹 [0, 1]。
    #[test]
    fn follow_scales_and_clamps() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.1, 150.0);
        assert!((tr.sample(0.1).0 - 0.5).abs() < 1e-12);
        tr.finger_delta(0.2, 150.0);
        assert!((tr.sample(0.2).0 - 1.0).abs() < 1e-12);
        tr.finger_delta(0.3, 300.0);
        assert_eq!(tr.sample(0.3).0, 1.0);
        tr.finger_delta(0.4, -1000.0);
        assert_eq!(tr.sample(0.4).0, 0.0);
    }

    /// 2 慢速过半松手：v < v_min，按最近点。
    #[test]
    fn slow_release_picks_nearest() {
        let evs = slow_release_at(0.6).take_events();
        assert_eq!(evs.last(), Some(&SnapEvent::Target(1.0)));
        let evs = slow_release_at(0.4).take_events();
        assert_eq!(evs.last(), Some(&SnapEvent::Target(0.0)));
    }

    /// 3 快速甩：投影受速度方向约束，方向优先。
    #[test]
    fn fast_flick_follows_direction() {
        // 下甩：先到 1.0，再于末尾快速回落到 0.8（v ≈ −20/s）→ 关。
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.05, GESTURE_DISTANCE); // 1.0
        tr.finger_delta(0.40, 0.0);
        tr.finger_delta(0.41, -60.0); // 0.8
        tr.finger_end(0.41, false);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(0.0)));

        // 上甩：停在 0.0，末尾快速升到 0.2（v ≈ +20/s）→ 开。
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.40, 0.0);
        tr.finger_delta(0.41, 60.0); // 0.2
        tr.finger_end(0.41, false);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(1.0)));
    }

    /// 4 方向优先规则：投影最近点与速度反向时，取速度方向上的下一个吸附点。
    #[test]
    fn projection_rejects_opposite_direction() {
        let params = SnapParams {
            min_velocity: 0.5,
            ..SnapParams::default()
        };
        let mut tr = SnapTracker::new(params, 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.05, 0.7825 * 300.0);
        tr.finger_delta(0.35, 0.0);
        tr.finger_delta(0.50, -0.0825 * 300.0); // 0.7，窗口速度 −0.55/s
        tr.finger_end(0.50, false);
        // 投影 p* = 0.516 → 最近点 1，但与 −v 反向 → 改取 0。
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(0.0)));
    }

    /// 5 cancelled：回到手势开始值。
    #[test]
    fn cancelled_returns_to_start() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.1, 150.0);
        tr.finger_end(0.2, true);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(0.0)));
        tr.sample(2.0);
        assert_eq!(tr.sample(2.0).0, 0.0);
    }

    /// 6 静止起弹：开 0→0.99 ≈ 350 ms，关 1→0.01 ≈ 200 ms。
    #[test]
    fn settle_timing_open_close() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let t99 = first_time(|t| tr.sample(t).0 >= 0.99, 1.0);
        assert!((t99 - 0.350).abs() <= 0.005, "开 0→0.99 用时 {t99}");

        let mut tr = SnapTracker::new(SnapParams::default(), 1.0);
        tr.animate_to(0.0, 0.0, Origin::Key);
        let t01 = first_time(|t| tr.sample(t).0 <= 0.01, 1.0);
        assert!((t01 - 0.200).abs() <= 0.005, "关 1→0.01 用时 {t01}");
    }

    /// 7 速度连续：松手瞬间 `sample` 的速度 = 松手前估计速度（误差 < 1%）。
    #[test]
    fn velocity_is_continuous_at_release() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.1, 60.0); // 0.2
        tr.finger_delta(0.2, 60.0); // 0.4
        tr.finger_end(0.2, false);
        let (_, v) = tr.sample(0.2);
        let expect = (0.4 - 0.2) / (0.2 - 0.1); // 2.0 /s
        assert!((v - expect).abs() / expect < 0.01, "松手速度 {v} vs {expect}");
    }

    /// 8 采样无关：同一弹簧，60 / 120 / 165 Hz 与随机时刻在公共时刻值一致。
    #[test]
    fn sampling_rate_does_not_change_curve() {
        let trackers: Vec<SnapTracker> = [60.0, 120.0, 165.0]
            .iter()
            .map(|_| {
                let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
                tr.animate_to(0.0, 1.0, Origin::Key);
                tr
            })
            .collect();
        // 公共时刻：60 Hz 网格 + 若干非网格时刻。
        let mut times: Vec<f64> = Vec::new();
        let mut t = 0.0;
        while t <= 0.5 {
            times.push(t);
            t += 1.0 / 60.0;
        }
        times.extend([0.0001, 0.1234, 0.2671, 0.4999]);
        let vals: Vec<Vec<f64>> = trackers
            .iter()
            .map(|tr| times.iter().map(|&t| tr.sample(t).0).collect())
            .collect();
        let mut max_diff = 0.0_f64;
        for row in &vals[1..] {
            for i in 0..times.len() {
                max_diff = max_diff.max((row[i] - vals[0][i]).abs());
            }
        }
        assert!(max_diff < 1e-9, "采样无关最大差 {max_diff}");
    }

    /// 9 打断：落定中 `grab` 冻结；`finger_begin` 从冻结值续跟。
    #[test]
    fn grab_freezes_then_follow_resumes() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let frozen = tr.sample(0.15).0;
        tr.grab(0.15);
        assert_eq!(tr.sample(0.2).0, frozen);
        assert_eq!(tr.sample(0.5).0, frozen);
        tr.finger_begin(0.6);
        tr.finger_delta(0.7, 30.0); // +0.1
        assert!((tr.sample(0.7).0 - (frozen + 0.1)).abs() < 1e-12);
    }

    /// 10 反向：开到 0.5 时 `animate_to(0)` → 值与速度连续（无跳变）。
    #[test]
    fn reverse_is_continuous() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let (xb, vb) = tr.sample(0.1);
        tr.animate_to(0.1, 0.0, Origin::Key);
        let (xa, va) = tr.sample(0.1);
        assert!((xa - xb).abs() < 1e-12, "值跳变 {xa} vs {xb}");
        assert!((va - vb).abs() < 1e-12, "速度跳变 {va} vs {vb}");
        assert!(tr.sample(0.2).0 < xa, "反向应开始下降");
    }

    /// 11 事件顺序：手势一轮 = Begin(Gesture)/Target/Settled；按键一轮 = Begin(Key)/Target/Settled。
    #[test]
    fn event_sequence_gesture_and_key() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.45, 90.0); // 0.3
        tr.finger_end(0.45, false);
        assert_eq!(tr.take_events()[0], SnapEvent::Begin(Origin::Gesture));
        tr.sample(2.0);
        let evs = tr.take_events();
        assert_eq!(evs, vec![SnapEvent::Settled(0.0)]);

        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        assert_eq!(
            tr.take_events(),
            vec![SnapEvent::Begin(Origin::Key), SnapEvent::Target(1.0)]
        );
        tr.sample(2.0);
        assert_eq!(tr.take_events(), vec![SnapEvent::Settled(1.0)]);
    }

    /// 12 夹界：大初速越过目标那段夹在 [0, 1]，最终落定于目标。
    #[test]
    fn overshoot_is_clamped_to_range() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.01, 60.0); // 0.2
        tr.finger_delta(0.02, 60.0); // 0.4，v ≈ +20/s → 投影去 1
        tr.finger_end(0.02, false);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(1.0)));
        let mut max_x = 0.0_f64;
        let mut t = 0.0;
        while t <= 2.0 {
            let x = tr.sample(t).0;
            assert!((0.0..=1.0).contains(&x), "t={t} 越界 x={x}");
            max_x = max_x.max(x);
            t += 0.001;
        }
        assert!(max_x >= 0.999, "未到达目标附近 max_x={max_x}");
        tr.sample(2.0);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Settled(1.0)));
        assert_eq!(tr.sample(3.0).0, 1.0);
    }
}
