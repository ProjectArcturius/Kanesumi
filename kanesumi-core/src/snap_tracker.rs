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
//
// 2026-11-03 裁定 §254 改「UWP 风格阻尼」：单调趋近、绝不过冲——初速朝目标且超过
// `ω·|T − x₀|` 时本次改用 `ω = |v₀| / |T − x₀|`，曲线退化为指数趋近；
// 按键从静止朝开方向起步给初速 `ω_key·(T − x₀)`（参 GESTURE_PLAN §Ⅳ-4 / §Ⅳ-5、§Ⅶ）。

use std::cell::Cell;

use crate::interaction::{
    GESTURE_DISTANCE, GESTURE_MIN_VELOCITY, GESTURE_VELOCITY_WINDOW_MS, INERTIA_TAU_S,
    SNAP_EPSILON, SNAP_OMEGA_CLOSE, SNAP_OMEGA_OPEN,
};

/// 落定的速度阈值（/s）：与位置阈值同判，静止才发 `Settled`（GESTURE_PLAN §Ⅳ.4）。
const SETTLE_VELOCITY: f64 = 0.05;

/// 按键 / 客户端「从静止」的判据（/s）：取 [`SETTLE_VELOCITY`]，与落定同口径——
/// 低于它的运动按 §Ⅳ.4 已算落定，此时给初速才不至于把残余速度重复计入。
const START_FROM_REST_VELOCITY: f64 = SETTLE_VELOCITY;

/// 从静止朝开方向起弹的角频率 ω_key（/s）。参裁定 §254、GESTURE_PLAN §Ⅳ-5。
///
/// 出处：拟合 UWP 开曲线 `cubic-bezier(0.1, 0.9, 0.2, 1)` / 350 ms——60 Hz 逐帧最大偏差
/// 0.03（本轮实测 0.0294，见 `key_open_matches_uwp_open_curve`），首帧 0.32。
/// 静止临界阻尼弹簧（`ω_open`）首帧只有 0.04，起步「肉」。
/// 常量放本文件而非 `interaction.rs`：本卡只动 `snap_tracker.rs`，不扩散到手势参数表。
const SNAP_OMEGA_KEY: f64 = 23.8;

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
    /// 按键 / 客户端**从静止朝开方向**起弹的角频率（/s）：同时用作初速 `v₀ = ω·(T − x₀)`，
    /// 曲线即指数趋近，贴合 UWP 开曲线。关方向从静止不用它（UWP 关曲线本就慢起）。参裁定 §254。
    pub omega_key: f64,
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
            omega_key: SNAP_OMEGA_KEY,
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

    /// 以 `(t, x0, v0)` 起弹落向 `target`；`omega` 为 `None` 时按方向取 `ω_open` / `ω_close`。
    ///
    /// 不变量（裁定 §254）：阻尼比恒为 1，且**绝不过冲**。朝目标的初速若超过临界阻尼允许的
    /// 上限 `ω·|T − x₀|`，本次取 `ω = |v₀| / |T − x₀|`，曲线退化为指数趋近 `T + (x₀−T)e^{−ωt}`：
    /// 速度连续、单调趋近，不再冲过 0 / 1 被 `clamp_points` 夹住后急停（§254 之前快速上甩会
    /// 冲过 1 并停在平台上）。背离目标的速度不改 ω——临界阻尼本就不越过 T。
    fn start_spring(&mut self, t: f64, x0: f64, v0: f64, target: f64, omega: Option<f64>) {
        let target = clamp_points(&self.params.snap_points, target);
        let d = target - x0;
        let (omega, settle_t) = if d.abs() < f64::EPSILON {
            // 已在目标上：`ω` 无意义也无需调整，直接落定（防除零）。仍记成 `settle_t = t` 的
            // 弹簧而不是 `Motion::Settled`，好让 `take_events` 照原路补发 `Settled`。
            (omega.unwrap_or(self.params.omega_open), t)
        } else {
            let mut w = omega.unwrap_or(if d > 0.0 {
                self.params.omega_open
            } else {
                self.params.omega_close
            });
            if v0 * d > 0.0 && v0.abs() > w * d.abs() {
                w = v0.abs() / d.abs();
            }
            let st = find_settle_t(t, x0, v0, target, w, self.params.epsilon);
            (w, st)
        };
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
        self.start_spring(t, p, v, target, None);
        self.events.push(SnapEvent::Target(target));
    }

    /// 按键 / 客户端：从 `sample(t)` 的值与速度起弹落向 `target`（速度连续、可打断进行中的落定）。
    ///
    /// **从静止朝开方向**发起时给初速 `v₀ = ω_key·(T − x₀)`、`ω = ω_key`：即指数趋近，
    /// 逐帧贴合 UWP 开曲线（裁定 §254、GESTURE_PLAN §Ⅳ-5）。静止的临界阻尼弹簧首帧只有 0.04，
    /// 按键拉起显得「肉」。关方向从静止不加初速（UWP 关曲线本就慢起）；运动中被中断则沿用
    /// 当前速度（连续），只套用 [`Self::start_spring`] 的不过冲规则。
    pub fn animate_to(&mut self, t: f64, target: f64, origin: Origin) {
        let (x, v) = self.sample(t);
        self.events.push(SnapEvent::Begin(origin));
        self.events.push(SnapEvent::Target(target));
        let goal = clamp_points(&self.params.snap_points, target);
        let d = goal - x;
        if v.abs() < START_FROM_REST_VELOCITY && d > 0.0 {
            let omega = self.params.omega_key;
            self.start_spring(t, x, omega * d, goal, Some(omega));
        } else {
            self.start_spring(t, x, v, goal, None);
        }
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

    /// UWP 开曲线 `cubic-bezier(0.1, 0.9, 0.2, 1)` 在归一化时刻 `x` 的取值。
    ///
    /// `kanesumi-core` 无任何依赖（本卡不许为测试新增依赖），故按 `sokuou::easing::bezier_y`
    /// 的语义就地实现：控制点 `P0=(0,0)`、`P3=(1,1)`（同 CSS `cubic-bezier`），反解参数。
    /// 该曲线 `x(u)` 在 `[0,1]` 上单调（控制点 `x` 均 ∈ (0,1)），二分 60 次即到机器精度。
    fn uwp_open_at(x: f64) -> f64 {
        const CURVE: [f64; 4] = [0.1, 0.9, 0.2, 1.0];
        let [x1, y1, x2, y2] = CURVE;
        let bezier = |a: f64, b: f64, u: f64| {
            let m = 1.0 - u;
            3.0 * m * m * u * a + 3.0 * m * u * u * b + u * u * u
        };
        let x = x.clamp(0.0, 1.0);
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..60 {
            let u = 0.5 * (lo + hi);
            if bezier(x1, x2, u) < x { lo = u } else { hi = u }
        }
        bezier(y1, y2, 0.5 * (lo + hi))
    }

    /// 取当前弹簧的 `(t0, x0, v0, target, omega)`；非弹簧则报错。
    fn spring_of(tr: &SnapTracker) -> (f64, f64, f64, f64, f64) {
        match tr.motion {
            Motion::Spring {
                t0,
                x0,
                v0,
                target,
                omega,
                ..
            } => (t0, x0, v0, target, omega),
            other => panic!("应为弹簧，实际 {other:?}"),
        }
    }

    /// 区间 `[a, b]` 内解析速度的最大绝对值（细分 100 步）：核对离散帧的位移没有超过
    /// 连续曲线在同样时间内能走出的量，即「没有跳变」。符号翻转（帧内含速度零点）时
    /// 无法用相邻帧位移之比判断连续，改用它。
    fn max_speed_in(a: f64, b: f64, spring: (f64, f64, f64, f64, f64)) -> f64 {
        let (t0, x0, v0, target, omega) = spring;
        let mut m = 0.0_f64;
        for i in 0..=100 {
            let t = a + (b - a) * i as f64 / 100.0;
            m = m.max(eval_spring(t, t0, x0, v0, target, omega).1.abs());
        }
        m
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

    /// 6 静止起弹：开 0→0.99 ≈ 194 ms（ω_key 指数趋近），关 1→0.01 ≈ 200 ms。
    ///
    /// 开方向原判据「≈ 350 ms」随裁定 §254 废止：UWP 开曲线本身约 230 ms 就到 0.99，
    /// 按 350 ms 拟合的静止弹簧与曲线不符（§254、GESTURE_PLAN §Ⅶ）。开方向的形状判据
    /// 见 `key_open_matches_uwp_open_curve`，此处只钉时长。
    #[test]
    fn settle_timing_open_close() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let t99 = first_time(|t| tr.sample(t).0 >= 0.99, 1.0);
        // ln(100) / ω_key = 4.60517 / 23.8 = 0.1935 s。
        assert!((t99 - 0.1935).abs() <= 0.005, "开 0→0.99 用时 {t99}");

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
        // 下行 0.1：§254 之后开曲线在 0.15 s 已到 0.97，上行 +0.1 会碰到上夹（1.0），
        // 改下行仍验证「冻结值 + 跟手 1:1」这同一判据，不放松阈值。
        tr.finger_delta(0.7, -30.0);
        assert!((tr.sample(0.7).0 - (frozen - 0.1)).abs() < 1e-12);
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
    ///
    /// 裁定 §254 之后此路径不再越界（初速 20 /s > `ω_open·0.6` = 11.38，ω 已上调为 33.3），
    /// 夹取只剩兜底作用，用例名因此名不副实；判据原样保留，改名留待下一轮清理。
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

    /// 13 按键开：60 Hz 逐帧贴合 UWP 开曲线 `(0.1, 0.9, 0.2, 1)` / 350 ms（裁定 §254、§Ⅶ）。
    ///
    /// 判据三条：逐帧 `|p − UWP(t/0.35)| ≤ 0.03`、首帧 `p ≥ 0.30`、`p ≥ 0.99` 的最早帧 ≤ 200 ms。
    /// 表用 `cargo test -p kanesumi-core key_open_matches -- --nocapture` 打印，报告里的逐帧表即出自此处。
    #[test]
    fn key_open_matches_uwp_open_curve() {
        const DURATION: f64 = 0.35;
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let mut worst = 0.0_f64;
        let mut first99: Option<f64> = None;
        for k in 1..=30 {
            let t = k as f64 / 60.0;
            let p = tr.sample(t).0;
            let uwp = uwp_open_at(t / DURATION);
            let diff = (p - uwp).abs();
            worst = worst.max(diff);
            println!("k={k:2} t={t:.4} p={p:.4} uwp={uwp:.4} diff={diff:+.4}");
            assert!(diff <= 0.03, "k={k} t={t:.4} p={p:.4} uwp={uwp:.4} 差 {diff:.4}");
            if k == 1 {
                assert!(p >= 0.30, "首帧 p={p} < 0.30（起步仍「肉」）");
            }
            if p >= 0.99 && first99.is_none() {
                first99 = Some(t);
            }
        }
        let t99 = first99.expect("30 帧内应到 0.99");
        assert!(t99 <= 0.200, "到 0.99 用时 {t99} > 200 ms");
        assert!(worst <= 0.03, "全程最大偏差 {worst}");
    }

    /// 14 快速上甩：松手速度 ≥ 5 /s、位置 0.6 → 落定全程单调不减，落定前每帧 `p < 1`
    /// （无夹取平台），松手前后两帧 Δp 比 0.8~1.25（裁定 §254、GESTURE_PLAN §Ⅶ）。
    ///
    /// 第二段另给一个 §254 之前必定冲过 1 被夹住的速度（18 /s），钉住「ω 按初速上调」这条修复。
    #[test]
    fn fast_flick_settles_without_clamp_plateau() {
        // 60 Hz 喂 +0.1/帧 = 6 /s，第 6 帧到 0.6 松手。
        // 「松手前一帧」的值只能在喂的过程中记下：`Hold` 的 `sample(t)` 与 `t` 无关（冻结值），
        // 起弹后再取 `sample(t < t0)` 会反向求值。
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        let mut p_before = 0.0;
        for k in 1..=6 {
            if k == 6 {
                p_before = tr.sample((k - 1) as f64 / 60.0).0;
            }
            tr.finger_delta(k as f64 / 60.0, 0.1 * GESTURE_DISTANCE);
        }
        let t_rel = 6.0 / 60.0;
        let (p_rel, _) = tr.sample(t_rel);
        let v_rel = tr.estimate_velocity(t_rel);
        assert!((p_rel - 0.6).abs() < 1e-12, "松手位置 {p_rel}");
        assert!(v_rel >= 5.0, "松手速度 {v_rel} < 5 /s");
        tr.finger_end(t_rel, false);
        assert_eq!(tr.take_events().last(), Some(&SnapEvent::Target(1.0)));

        // 落定前每帧 p < 1；全程单调不减。
        let d_before = p_rel - p_before;
        assert!(d_before > 0.0);
        let mut prev = p_rel;
        let mut first_after = None;
        for k in 1..=60 {
            let t = t_rel + k as f64 / 60.0;
            let (p, _) = tr.sample(t);
            assert!(p >= prev - 1e-12, "t={t:.4} 非单调：{prev} → {p}");
            if !tr.is_settled(t) {
                assert!(p < 1.0, "t={t:.4} 落定前已夹到 1（平台）");
            } else if first_after.is_none() {
                first_after = Some(p);
            }
            if k == 1 {
                // 松手前后各一帧的位移比：速度连续则 ≈ 1。
                let ratio = (p - p_rel) / d_before;
                println!(
                    "flick: v_rel={v_rel:.3}/s p_rel={p_rel:.4} Δp前={d_before:.4} Δp后={:.4} 比={ratio:.3}",
                    p - p_rel
                );
                assert!(
                    (0.8..=1.25).contains(&ratio),
                    "松手跨界 Δp 比 {ratio}（前 {d_before}，后 {}）",
                    p - p_rel
                );
            }
            prev = p;
        }
        assert_eq!(first_after, Some(1.0), "最终应落定于 1");

        // 18 /s 上甩：§254 之前 ω_open·|T−x₀| = 7.59 < 18，弹簧会冲过 1 被夹住。
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.5 / 60.0, 0.3 * GESTURE_DISTANCE);
        tr.finger_delta(1.0 / 60.0, 0.3 * GESTURE_DISTANCE);
        assert!(tr.estimate_velocity(1.0 / 60.0) >= 5.0);
        tr.finger_end(1.0 / 60.0, false);
        let (t0, x0, v0, target, omega) = spring_of(&tr);
        assert!(v0 > SNAP_OMEGA_OPEN * (target - x0), "本段应触发 ω 上调：v0={v0}");
        assert!((omega - v0.abs() / (target - x0).abs()).abs() < 1e-12, "ω 未按初速上调：{omega}");
        println!("hard flick: v0={v0:.3}/s ω={omega:.3} (ω_open={SNAP_OMEGA_OPEN})");
        let mut prev = x0;
        for k in 1..=60 {
            let t = 1.0 / 60.0 + k as f64 / 60.0;
            let (p, _) = tr.sample(t);
            assert!(p >= prev - 1e-12, "t={t:.4} 非单调：{prev} → {p}");
            if !tr.is_settled(t) {
                assert!(p < 1.0, "t={t:.4} 落定前已夹到 1（平台）");
                let raw = eval_spring(t, t0, x0, v0, target, omega).0;
                assert!(raw < 1.0, "t={t:.4} 原始值已越 1：{raw}");
            }
            prev = p;
        }
        assert_eq!(tr.sample(2.0).0, 1.0);
    }

    /// 15 背离目标打断：开到一半 `animate_to(0)` → 速度连续，且夹取前的原始值也不越过 0
    /// （`−1e-9` 容差），即不会出现「冲过 0 再被夹住急停」（裁定 §254）。
    ///
    /// 此处不用「相邻两帧 Δp 比 0.8~1.25」判连续：打断瞬间去向翻转、ω 从 23.8 变 33.2，
    /// 一帧内速度变化很大，有限差分之比没有意义。改用两条更强的判据——打断时刻的解析速度
    /// 逐位相等；以及每帧位移都不超过连续曲线在该帧内能走出的最大速度（无跳变）。
    #[test]
    fn reverse_from_half_never_crosses_zero() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        // 60 Hz 逐帧开到 p ≥ 0.5（k = 2，p ≈ 0.548）。
        let mut k = 1;
        while tr.sample(k as f64 / 60.0).0 < 0.5 {
            k += 1;
        }
        let t_i = k as f64 / 60.0;
        let (x_before, v_before) = tr.sample(t_i);
        assert!(x_before >= 0.5, "打断点 p={x_before}");

        tr.animate_to(t_i, 0.0, Origin::Key);
        let (x_at, v_at) = tr.sample(t_i);
        assert!((x_at - x_before).abs() < 1e-12, "值跳变 {x_at} vs {x_before}");
        assert!((v_at - v_before).abs() < 1e-12, "速度跳变 {v_at} vs {v_before}");

        // 背离目标的速度不改 ω：仍按关方向 33.2 起弹，上行到头后单调回落。
        let spring = spring_of(&tr);
        let (t0, x0, v0, target, omega) = spring;
        assert_eq!(target, 0.0);
        assert!((omega - SNAP_OMEGA_CLOSE).abs() < 1e-12, "背离目标不应改 ω：{omega}");
        assert!(v0 > 0.0, "打断瞬间应仍朝开方向 {v0}");

        let mut min_raw = x0;
        let mut prev = x_before;
        let mut turned = false;
        for k in 1..=120 {
            let t = t_i + k as f64 / 60.0;
            let raw = eval_spring(t, t0, x0, v0, target, omega).0;
            min_raw = min_raw.min(raw);
            let (p, _) = tr.sample(t);
            assert!((0.0..=1.0).contains(&p), "t={t:.4} p={p} 越界");
            let d = p - prev;
            if d < 0.0 {
                turned = true;
            }
            if t < tr_settle_t(&tr) {
                let bound = max_speed_in(t - 1.0 / 60.0, t, spring) / 60.0 * 1.25;
                assert!(d.abs() <= bound + 1e-12, "t={t:.4} 位移 {d} 超过速度允许 {bound}");
            }
            prev = p;
        }
        assert!(turned, "应上行到头后回落");
        println!("reverse: 打断于 t={t_i:.4} p={x_before:.4} v={v_before:.3}/s ω={omega} 原始最小值={min_raw:.3e}");
        assert!(min_raw >= -1e-9, "夹取前原始值跌破 0：{min_raw}");
        assert_eq!(tr.sample(3.0).0, 0.0);
    }

    /// 取弹簧的 `settle_t`（非弹簧则返回 `f64::INFINITY`）。
    fn tr_settle_t(tr: &SnapTracker) -> f64 {
        match tr.motion {
            Motion::Spring { settle_t, .. } => settle_t,
            _ => f64::INFINITY,
        }
    }

    /// 16 关方向按键从静止：与修订前**完全一致**（逐帧逐位相等）。
    ///
    /// §254 只改开方向起步（`ω_key` + 初速）。关方向仍是「v₀ = 0、ω = ω_close」的临界阻尼，
    /// 故其闭式解与修订前同一条：`x(t) = (1 + ω_close·t)·e^{−ω_close·t}`（x₀=1、T=0）。
    #[test]
    fn key_close_from_rest_is_unchanged() {
        let mut tr = SnapTracker::new(SnapParams::default(), 1.0);
        tr.animate_to(0.0, 0.0, Origin::Key);
        let (t0, x0, v0, target, omega, settle_t) = match tr.motion {
            Motion::Spring {
                t0,
                x0,
                v0,
                target,
                omega,
                settle_t,
            } => (t0, x0, v0, target, omega, settle_t),
            other => panic!("应为弹簧，实际 {other:?}"),
        };
        assert_eq!((t0, x0, v0, target), (0.0, 1.0, 0.0, 0.0), "关方向起弹参数变了");
        assert_eq!(omega, SNAP_OMEGA_CLOSE, "关方向 ω 变了");
        assert!(settle_t > 0.0);
        let mut checked = 0;
        for k in 1..=16 {
            let t = k as f64 / 60.0;
            if t >= settle_t {
                break;
            }
            let old = (1.0 + SNAP_OMEGA_CLOSE * t) * (-SNAP_OMEGA_CLOSE * t).exp();
            assert_eq!(tr.sample(t).0, old, "k={k} 与修订前不逐位相等");
            checked += 1;
        }
        assert!(checked >= 10, "比较帧数太少：{checked}");
    }

    /// 17 落定时刻对「指数趋近」同样正确（`v₀ = ω·Δ`，`b = v0 + ω·a = 0`，解析式退化为纯指数）。
    ///
    /// 静止起弹的开方向即此情形：`|x−T| = Δ·e^{−ωd}`、`|v| = ω·Δ·e^{−ωd}`。落定要求两个阈值
    /// **同时**满足，故取较晚的那个；`ω_key·ε = 0.119 > 0.05`，位置先到、速度后到，
    /// `settle_t = ln(ω_key·Δ/0.05)/ω_key`。若求法仍按弹簧的一般形状（例如假设 `b ≠ 0`
    /// 的闭式反解）就会算错，这里钉住。
    #[test]
    fn settle_time_is_analytic_for_exponential_decay() {
        let mut tr = SnapTracker::new(SnapParams::default(), 0.0);
        tr.animate_to(0.0, 1.0, Origin::Key);
        let (t0, x0, v0, target, omega) = spring_of(&tr);
        assert_eq!((t0, x0, target), (0.0, 0.0, 1.0));
        // 退化为指数趋近：b = v0 + ω·(x0 − T) 应为 0（速度恰为 ω·Δ）。
        let b = v0 + omega * (x0 - target);
        assert!(b.abs() < 1e-12, "b={b}，不是纯指数趋近");

        let delta = (target - x0).abs();
        let by_velocity = (omega * delta / SETTLE_VELOCITY).ln() / omega;
        let by_position = (delta / SNAP_EPSILON).ln() / omega;
        assert!(by_velocity > by_position, "本组应由速度阈值决定落定时刻");
        let expect = by_velocity;

        assert!(!tr.is_settled(expect - 2.0 * SETTLE_SCAN_STEP), "提前落定");
        assert!(tr.is_settled(expect + 2.0 * SETTLE_SCAN_STEP), "落定时刻偏晚");
        // 落定瞬间的跳变不超过位置阈值：sample 在这一刻直接跳到 target。
        let (x_at, _) = eval_spring(expect, t0, x0, v0, target, omega);
        assert!((x_at - target).abs() < SNAP_EPSILON, "落定跳变 {} 超过 ε", (x_at - target).abs());
    }
}
