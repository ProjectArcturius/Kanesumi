// scroll_tracker.rs —— 合成器滚动跟踪器（DirectManipulation / InteractionTracker 的对应物）。
//
// 设计定稿：`tools/dispatch/tasks/fp4-compositor-scroll.md` §Ⅱ（语义八条），
// 数值由真机真值拟合：`docs/research/w_scroll_tracker/README.md`（Lumia 15 段 + Win11 探针）。
// 合成器（fp4a）与 harness 退回路径（fp4b）共用本模块，故纯逻辑、无平台依赖。
//
// 时间一律由调用方给出单调秒（f64）。[`ScrollTracker::sample`] 是 `t` 的**纯函数**：
// 运动段用解析式求值（非逐帧积分），任何采样频率 / 相位得同一曲线（参 fp4 §Ⅱ-1）。
// 松手即惯性起点：`finger_end(t0)` 后 `sample(t0 + Δ)` 已含惯性位移，无交接空档（§Ⅱ-3）。

use crate::interaction::{
    FINGER_VELOCITY_WINDOW_MS, INERTIA_MIN_START_VELOCITY, INERTIA_STOP_VELOCITY, INERTIA_TAU_S,
    SCROLL_BY_BASELINE_VELOCITY, SCROLL_BY_DECAY_RATE, SCROLL_BY_VELOCITY_PER_PX,
    SCROLL_OVERSCROLL_MAX_PX, SCROLL_OVERSCROLL_TAU_S, SCROLL_RETURN_BEZIER,
    SCROLL_RETURN_DURATION_S,
};

/// 横向轴下标（`dx`）。
const AXIS_X: usize = 0;
/// 纵向轴下标（`dy`）。
const AXIS_Y: usize = 1;
/// 视为「无位移」的阈值（px），用于滚动目标已到边界时的空操作判定。
const EPS: f64 = 1e-9;

/// 跟踪器可调参数。`Default` = A 节拟合值 + `interaction.rs` §Ⅴ 现有常量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackerParams {
    /// 自由惯性时间常数 τ（秒）：`INERTIA_TAU_S` = 0.3338（Win11 探针五档一致；`w_comp_ref` §①）。
    pub tau_s: f64,
    /// 惯性停止速度 30 px/s（Win11 `s_minimumVelocity`；`w_comp_ref` §②）。
    pub stop_velocity: f64,
    /// 起甩门限 50 px/s：低于此速度不起惯性（正典 §Ⅴ）。
    pub min_start_velocity: f64,
    /// 松手速度估计窗口（秒）：`FINGER_VELOCITY_WINDOW_MS` = 100 ms。
    pub velocity_window_s: f64,
    /// 越界上限 M（px）：Lumia 实测 135.59（`README.md` §二 A3）。
    pub overscroll_max: f64,
    /// 越界减速 τ_over（秒）：Lumia 拟合 0.0449（`README.md` §二 A2a）。
    pub overscroll_tau_s: f64,
    /// 回程时长 T（秒）：Lumia 拟合 0.2804（`README.md` §二 A2b）。
    pub return_duration_s: f64,
    /// 回程三次贝塞尔控制点 `(x1, y1, x2, y2)`：Lumia 拟合（同上）。
    pub return_bezier: [f64; 4],
    /// `scroll_by` 每像素所需速度：WinUI `ScrollView.cpp:2399` 7.600855902349023。
    pub scroll_by_velocity_per_px: f64,
    /// `scroll_by` 惯性衰减率：WinUI `ScrollView.cpp:2393` 0.9995。
    pub scroll_by_decay_rate: f64,
    /// `scroll_by` 基准速度（px/s）：WinUI `ScrollView.cpp:2404` = 30。
    pub baseline_velocity: f64,
}

impl Default for TrackerParams {
    fn default() -> Self {
        Self {
            tau_s: INERTIA_TAU_S as f64,
            stop_velocity: INERTIA_STOP_VELOCITY as f64,
            min_start_velocity: INERTIA_MIN_START_VELOCITY as f64,
            velocity_window_s: FINGER_VELOCITY_WINDOW_MS as f64 / 1000.0,
            overscroll_max: SCROLL_OVERSCROLL_MAX_PX,
            overscroll_tau_s: SCROLL_OVERSCROLL_TAU_S,
            return_duration_s: SCROLL_RETURN_DURATION_S,
            return_bezier: SCROLL_RETURN_BEZIER,
            scroll_by_velocity_per_px: SCROLL_BY_VELOCITY_PER_PX,
            scroll_by_decay_rate: SCROLL_BY_DECAY_RATE,
            baseline_velocity: SCROLL_BY_BASELINE_VELOCITY,
        }
    }
}

impl TrackerParams {
    /// `scroll_by` 惯性时间常数 `τ_k = −1 / ln(1 − 衰减率)`（与 `ScrollView` 的
    /// `s_velocityNeededPerPixel` 互为倒数）。
    fn scroll_by_tau(&self) -> f64 {
        -1.0 / (1.0 - self.scroll_by_decay_rate).ln()
    }
}

/// 跟踪器相位（两轴合成：跟手 > 惯性 > 动画 > 静止）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// 静止。
    Idle,
    /// 手指跟手。
    Interacting,
    /// 松手惯性 / 越界回程。
    Inertia,
    /// `scroll_by` / `scroll_to` 动画。
    Animating,
}

// ── 解析式工具 ──────────────────────────────────────────────────────────────

/// 一维三次贝塞尔（控制点 `[0, c1, c2, 1]`）在参数 `u` 处的值。
fn bezier1(u: f64, c1: f64, c2: f64) -> f64 {
    let mu = 1.0 - u;
    3.0 * mu * mu * u * c1 + 3.0 * mu * u * u * c2 + u * u * u
}

/// 由进度 `s` 反解贝塞尔横轴参数（横轴单调，二分 60 次即达 f64 精度）。
fn bezier_solve(s: f64, x1: f64, x2: f64) -> f64 {
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if bezier1(mid, x1, x2) < s {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// 回程在 `tr` 秒后剩余的越界比例（1 → 0，单调、不过冲）。`tr >= duration` 时为 0。
fn return_remaining(tr: f64, duration: f64, bez: [f64; 4]) -> f64 {
    if tr <= 0.0 {
        return 1.0;
    }
    if tr >= duration {
        return 0.0;
    }
    let u = bezier_solve(tr / duration, bez[0], bez[2]);
    1.0 - bezier1(u, bez[1], bez[3])
}

/// 手指越界的饱和位移：界内 1:1；越界后 `M·(1 − e^{−x/M})`（fp4 §Ⅱ-2）。
fn overscroll_display(raw: f64, min: f64, max: f64, m: f64) -> f64 {
    if raw > max {
        max + m * (1.0 - (-(raw - max) / m).exp())
    } else if raw < min {
        min - m * (1.0 - (-(min - raw) / m).exp())
    } else {
        raw
    }
}

/// [`overscroll_display`] 的逆：由显示位置求未饱和的原始位置（用于跟手从当前显示值续接）。
fn overscroll_raw(display: f64, min: f64, max: f64, m: f64) -> f64 {
    if display > max {
        max - m * (1.0 - (display - max) / m).ln()
    } else if display < min {
        min + m * (1.0 - (min - display) / m).ln()
    } else {
        display
    }
}

/// 自由惯性 / 越界减速 / 回程的解析路径（参 `README.md` §三 C-4 复算模型）。
///
/// 由松手（或打断）时刻的 `(t0, p0, v0)` 与边界一次性求出各相变时刻与峰值，
/// 之后 [`Inertia::eval`] 只依赖 `t`——保证采样无关。
#[derive(Debug, Clone, Copy)]
struct Inertia {
    t0: f64,
    p0: f64,
    v0: f64,
    /// 目标边界（`min` 或 `max`）。
    b: f64,
    /// 越界正方向符号（`b == max` 为 +1，否则 −1）。
    sgn: f64,
    /// 起点是否已在界外。
    outside: bool,
    /// 界内惯性到边界的穿越时刻（相对秒；界外为 0）。
    tc: f64,
    /// 穿越 / 越界起点速度。
    v_hit: f64,
    /// 起点超出边界的初始量。
    ex0: f64,
    /// 峰值时刻（相对秒）。
    t_pk: f64,
    /// 峰值越界量。
    peak: f64,
    /// 界内未触边界即停（无越界、无回程）。
    free_stop: bool,
    t_stop: f64,
    p_end: f64,
    /// 整条路径结束（相对秒），之后位置恒为终点。
    end_t: f64,
}

impl Inertia {
    /// 由 `(t0, p0, v0)` 与 `[min, max]` 构造路径。
    fn new(t0: f64, p0: f64, v0: f64, min: f64, max: f64, params: &TrackerParams) -> Self {
        let stop = params.stop_velocity;
        let tau = params.tau_s;
        let to = params.overscroll_tau_s;
        let ret = params.return_duration_s;
        let mut it = Self {
            t0, p0, v0, b: 0.0, sgn: 1.0, outside: false, tc: 0.0, v_hit: v0, ex0: 0.0,
            t_pk: 0.0, peak: 0.0, free_stop: false, t_stop: 0.0, p_end: p0, end_t: 0.0,
        };
        if p0 > max {
            (it.b, it.sgn, it.outside) = (max, 1.0, true);
        } else if p0 < min {
            (it.b, it.sgn, it.outside) = (min, -1.0, true);
        } else {
            it.outside = false;
            if v0 > 0.0 {
                (it.b, it.sgn) = (max, 1.0);
            } else {
                (it.b, it.sgn) = (min, -1.0);
            }
        }
        if v0 == 0.0 {
            if it.outside {
                it.ex0 = (p0 - it.b).abs();
                it.peak = it.ex0;
                it.v_hit = 0.0;
                it.end_t = ret;
            } else {
                it.free_stop = true;
            }
            return it;
        }
        let outward = it.outside && ((it.b == max && v0 > 0.0) || (it.b == min && v0 < 0.0));
        if it.outside && !outward {
            it.ex0 = (p0 - it.b).abs();
            it.peak = it.ex0;
            it.v_hit = 0.0;
            it.end_t = ret;
        } else if outward {
            it.ex0 = (p0 - it.b).abs();
            it.v_hit = v0;
            it.bounce(to, ret, stop);
        } else {
            if v0.abs() <= stop {
                it.free_stop = true;
                return it;
            }
            let cross = (it.b - p0) / (v0 * tau);
            if cross >= 1.0 {
                it.free_stop = true;
                it.t_stop = tau * (v0.abs() / stop).ln();
                it.p_end = p0 + v0 * tau * (1.0 - stop / v0.abs());
                it.end_t = it.t_stop;
                return it;
            }
            it.tc = -tau * (1.0 - cross).ln();
            it.v_hit = v0 - (it.b - p0) / tau;
            it.ex0 = 0.0;
            it.bounce(to, ret, stop);
        }
        it
    }

    /// 越界减速到峰值再回程（相对时刻，`it.tc` 已定）。
    fn bounce(&mut self, to: f64, ret: f64, stop: f64) {
        if self.v_hit.abs() > stop {
            self.t_pk = self.tc + to * (self.v_hit.abs() / stop).ln();
            self.peak = self.ex0 + self.v_hit.abs() * to * (1.0 - stop / self.v_hit.abs());
            self.end_t = self.t_pk + ret;
        } else {
            self.t_pk = self.tc;
            self.peak = self.ex0;
            self.end_t = if self.peak > 0.0 { self.t_pk + ret } else { self.t_pk };
        }
    }

    /// `t` 时刻位置。纯函数。
    fn eval(&self, t: f64, params: &TrackerParams) -> f64 {
        let tau = params.tau_s;
        let to = params.overscroll_tau_s;
        let ret = params.return_duration_s;
        let dt = t - self.t0;
        if self.free_stop {
            if dt <= 0.0 {
                return self.p0;
            }
            if dt >= self.t_stop {
                return self.p_end;
            }
            return self.p0 + self.v0 * tau * (1.0 - (-dt / tau).exp());
        }
        if !self.outside && dt < self.tc {
            return self.p0 + self.v0 * tau * (1.0 - (-dt / tau).exp());
        }
        if self.t_pk > self.tc && dt >= self.tc && dt < self.t_pk {
            return self.b
                + self.sgn
                    * (self.ex0
                        + self.v_hit.abs() * to * (1.0 - (-(dt - self.tc) / to).exp()));
        }
        self.b + self.sgn * self.peak * return_remaining(dt - self.t_pk, ret, params.return_bezier)
    }
}

// ── 单轴状态机 ──────────────────────────────────────────────────────────────

/// 单轴运动状态。
#[derive(Debug, Clone, Copy)]
enum Motion {
    /// 静止于 `pos`。
    Idle { pos: f64 },
    /// 手指跟手（`raw` 为未饱和的原始位置，显示按 [`overscroll_display`] 饱和）。
    Follow { raw: f64 },
    /// 松手惯性 / 越界回程。
    Inertia(Inertia),
    /// `scroll_by` / `scroll_to` 动画（一阶指数，`τ_k`）。
    Anim {
        t0: f64,
        x0: f64,
        v0: f64,
        tau: f64,
        end_t: f64,
        end_pos: f64,
    },
}

/// 单轴状态（`min`/`max` 为可滚范围，偏移坐标）。
#[derive(Debug, Clone, Copy)]
struct Axis {
    enabled: bool,
    min: f64,
    max: f64,
    motion: Motion,
}

impl Axis {
    /// 无界、使能、静止于 0（未 `set_bounds` 前的缺省）。
    fn unbounded() -> Self {
        Self {
            enabled: true,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            motion: Motion::Idle { pos: 0.0 },
        }
    }

    /// `t` 时刻位置。纯函数。
    fn position(&self, t: f64, p: &TrackerParams) -> f64 {
        match self.motion {
            Motion::Idle { pos } => pos,
            Motion::Follow { raw } => overscroll_display(raw, self.min, self.max, p.overscroll_max),
            Motion::Inertia(it) => it.eval(t, p),
            Motion::Anim { t0, x0, v0, tau, end_t, end_pos } => {
                let dt = t - t0;
                if dt <= 0.0 {
                    x0
                } else if dt >= end_t {
                    end_pos
                } else {
                    x0 + v0 * tau * (1.0 - (-dt / tau).exp())
                }
            }
        }
    }

    /// `t` 时刻速度（仅动画段需要用于滚动累加；惯性 / 跟手视作 0，滚动按新起点处理）。
    fn velocity(&self, t: f64, _p: &TrackerParams) -> f64 {
        match self.motion {
            Motion::Anim { t0, v0, tau, end_t, .. } => {
                let dt = t - t0;
                if dt <= 0.0 { v0 } else if dt >= end_t { 0.0 } else { v0 * (-dt / tau).exp() }
            }
            _ => 0.0,
        }
    }

    fn phase(&self) -> Phase {
        match self.motion {
            Motion::Idle { .. } => Phase::Idle,
            Motion::Follow { .. } => Phase::Interacting,
            Motion::Inertia(_) => Phase::Inertia,
            Motion::Anim { .. } => Phase::Animating,
        }
    }

    /// 当前运动的绝对结束时刻（运动段才有）。
    fn end_time(&self) -> Option<f64> {
        match self.motion {
            Motion::Inertia(it) => Some(it.t0 + it.end_t),
            Motion::Anim { t0, end_t, .. } => Some(t0 + end_t),
            _ => None,
        }
    }
}

// ── 跟踪器 ──────────────────────────────────────────────────────────────────

/// 两轴滚动跟踪器。
#[derive(Debug, Clone)]
pub struct ScrollTracker {
    params: TrackerParams,
    axes: [Axis; 2],
    /// 本轮跟手的 `(t, [x, y])` 显示位置样本；`finger_begin` 清空、`finger_delta` 追加。
    samples: Vec<(f64, [f64; 2])>,
}

impl ScrollTracker {
    /// 以参数建跟踪器；两轴无界、使能、静止于 0。
    pub fn new(params: TrackerParams) -> Self {
        Self {
            params,
            axes: [Axis::unbounded(), Axis::unbounded()],
            samples: Vec::new(),
        }
    }

    /// 设置可滚范围（偏移坐标）；`min[i] <= max[i]`。
    pub fn set_bounds(&mut self, min: [f64; 2], max: [f64; 2]) {
        self.axes[AXIS_X].min = min[0];
        self.axes[AXIS_X].max = max[0];
        self.axes[AXIS_Y].min = min[1];
        self.axes[AXIS_Y].max = max[1];
    }

    /// 使能轴：`vertical` 管 `dy`，`horizontal` 管 `dx`。
    pub fn set_axes(&mut self, vertical: bool, horizontal: bool) {
        self.axes[AXIS_Y].enabled = vertical;
        self.axes[AXIS_X].enabled = horizontal;
    }

    /// `t` 时刻的 `([x, y], 相位)`。**纯函数**：不改状态、与采样频率相位无关。
    pub fn sample(&self, t: f64) -> ([f64; 2], Phase) {
        let pos = [
            self.axes[AXIS_X].position(t, &self.params),
            self.axes[AXIS_Y].position(t, &self.params),
        ];
        let mut phase = Phase::Idle;
        for ax in &self.axes {
            phase = match (phase, ax.phase()) {
                (Phase::Interacting, _) | (_, Phase::Interacting) => Phase::Interacting,
                (Phase::Inertia, _) | (_, Phase::Inertia) => Phase::Inertia,
                (Phase::Animating, _) | (_, Phase::Animating) => Phase::Animating,
                _ => Phase::Idle,
            };
        }
        (pos, phase)
    }

    /// 把已结束的运动折叠为 `Idle`（不影响 `sample` 结果）；可选优化，非必需。
    pub fn settle(&mut self, t: f64) {
        let params = self.params;
        for ax in &mut self.axes {
            if let Some(end) = ax.end_time()
                && t >= end
            {
                let pos = ax.position(t, &params);
                ax.motion = Motion::Idle { pos };
            }
        }
    }
}

// ── 手指与滚动输入 ──────────────────────────────────────────────────────────

/// 起一段 `scroll_by` / `scroll_to` 动画：`end_pos` 恰为目标（基准速度与停止速度相等时）。
fn start_anim(ax: &mut Axis, t: f64, x0: f64, v0: f64, target: f64, params: &TrackerParams) {
    let tau = params.scroll_by_tau();
    let stop = params.stop_velocity;
    if v0.abs() <= stop {
        ax.motion = Motion::Idle { pos: target };
        return;
    }
    let end_t = tau * (v0.abs() / stop).ln();
    ax.motion = Motion::Anim { t0: t, x0, v0, tau, end_t, end_pos: target };
}

impl ScrollTracker {
    /// 手指按下：打断一切运动，从 `sample(t)` 处起跟（fp4 §Ⅱ-2 / §Ⅱ-8）。
    pub fn finger_begin(&mut self, t: f64) {
        let params = self.params;
        let pos = [
            self.axes[AXIS_X].position(t, &params),
            self.axes[AXIS_Y].position(t, &params),
        ];
        for (i, ax) in self.axes.iter_mut().enumerate() {
            if ax.enabled {
                ax.motion = Motion::Follow { raw: overscroll_raw(pos[i], ax.min, ax.max, params.overscroll_max) };
            }
        }
        self.samples.clear();
        self.samples.push((t, pos));
    }

    /// 手指增量（事件时间戳）。界内 1:1，越界饱和（fp4 §Ⅱ-2）。
    pub fn finger_delta(&mut self, t: f64, dx: f64, dy: f64) {
        let params = self.params;
        let mut pos = [0.0; 2];
        for (i, ax) in self.axes.iter_mut().enumerate() {
            let cur = ax.position(t, &params);
            if !ax.enabled {
                pos[i] = cur;
                continue;
            }
            let d = if i == AXIS_X { dx } else { dy };
            let raw = match ax.motion {
                Motion::Follow { raw } => raw + d,
                _ => overscroll_raw(cur, ax.min, ax.max, params.overscroll_max) + d,
            };
            ax.motion = Motion::Follow { raw };
            pos[i] = overscroll_display(raw, ax.min, ax.max, params.overscroll_max);
        }
        self.samples.push((t, pos));
    }

    /// 手指松开：起惯性 / 回程 / 静止（fp4 §Ⅱ-3~5）。松手时刻即惯性起点。
    pub fn finger_end(&mut self, t: f64) {
        let params = self.params;
        let mut next = [Motion::Idle { pos: 0.0 }; 2];
        for (i, ax) in self.axes.iter().enumerate() {
            let pos = ax.position(t, &params);
            if !ax.enabled {
                next[i] = ax.motion;
                continue;
            }
            let v = self.estimate_velocity(i, t, &params);
            let outside = pos < ax.min || pos > ax.max;
            next[i] = if !outside && v.abs() < params.min_start_velocity {
                Motion::Idle { pos: pos.clamp(ax.min, ax.max) }
            } else {
                Motion::Inertia(Inertia::new(t, pos, v, ax.min, ax.max, &params))
            };
        }
        for (ax, m) in self.axes.iter_mut().zip(next) {
            ax.motion = m;
        }
        self.samples.clear();
    }

    /// 最近速度窗内的 `Δp/Δt`（每轴）；样本不足两点用全部；窗口内单点则退回全部。
    fn estimate_velocity(&self, axis: usize, t: f64, params: &TrackerParams) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let start = t - params.velocity_window_s;
        let recent: Vec<(f64, [f64; 2])> = self
            .samples
            .iter()
            .copied()
            .filter(|(ts, _)| *ts >= start)
            .collect();
        let used = if recent.len() >= 2 { &recent } else { &self.samples };
        let (t0, p0) = used[0];
        let (t1, p1) = used[used.len() - 1];
        let dt = t1 - t0;
        if dt <= 0.0 { 0.0 } else { (p1[axis] - p0[axis]) / dt }
    }

    /// 滚轮 / 键盘一步（fp4 §Ⅱ-6）：`d` 的位移按 WinUI 键盘模型累加，恰好停在目标。
    pub fn scroll_by(&mut self, t: f64, dx: f64, dy: f64) {
        self.axis_scroll_by(AXIS_X, t, dx);
        self.axis_scroll_by(AXIS_Y, t, dy);
    }

    fn axis_scroll_by(&mut self, i: usize, t: f64, delta: f64) {
        let params = self.params;
        let ax = &mut self.axes[i];
        if !ax.enabled || delta == 0.0 {
            return;
        }
        let x_now = ax.position(t, &params);
        let tgt_cur = match ax.motion {
            Motion::Anim { end_pos, .. } => end_pos,
            _ => x_now,
        };
        let target = (tgt_cur + delta).clamp(ax.min, ax.max);
        let added = target - tgt_cur;
        if added.abs() <= EPS {
            return;
        }
        let v_now = ax.velocity(t, &params);
        let k = params.scroll_by_velocity_per_px;
        let v0 = if matches!(ax.motion, Motion::Anim { .. })
            && v_now.abs() > 0.0
            && (added > 0.0) == (v_now > 0.0)
        {
            // 同向续加：基准速度被当前速度抵消，故不再加基准（`ScrollView.cpp:2401–2422`）。
            v_now + added * k
        } else {
            // 新起点 / 反向：速度按目标距离重算（含基准速度）。
            (target - x_now) * k + added.signum() * params.baseline_velocity
        };
        start_anim(ax, t, x_now, v0, target, &params);
    }

    /// 目标位置滚动（fp4 §Ⅱ-7）：`animate` 时同 `scroll_by(目标 − 当前)`；否则硬置。
    pub fn scroll_to(&mut self, t: f64, x: f64, y: f64, animate: bool) {
        if !animate {
            self.set_position(t, x, y);
            return;
        }
        let params = self.params;
        for (i, target_raw) in [x, y].into_iter().enumerate() {
            let ax = &mut self.axes[i];
            if !ax.enabled {
                continue;
            }
            let x_now = ax.position(t, &params);
            let target = target_raw.clamp(ax.min, ax.max);
            let added = target - x_now;
            if added.abs() <= EPS {
                ax.motion = Motion::Idle { pos: target };
                continue;
            }
            let v0 = added * params.scroll_by_velocity_per_px
                + added.signum() * params.baseline_velocity;
            start_anim(ax, t, x_now, v0, target, &params);
        }
    }

    /// 硬置位置并立即 `Idle`（InteractionTracker `TryUpdatePosition` 语义，fp4 §Ⅱ-8）。
    pub fn set_position(&mut self, _t: f64, x: f64, y: f64) {
        for (i, v) in [x, y].into_iter().enumerate() {
            let ax = &mut self.axes[i];
            ax.motion = Motion::Idle { pos: v.clamp(ax.min, ax.max) };
        }
        self.samples.clear();
    }

    /// 以给定位置与速度**直接起惯性**（对应 InteractionTracker
    /// `TryUpdatePositionWithAdditionalVelocity`）。供「已知松手状态」直连，也用于对拍注入真值初态。
    pub fn flick(&mut self, t: f64, x: f64, y: f64, vx: f64, vy: f64) {
        let params = self.params;
        for (i, (p, v)) in [(x, vx), (y, vy)].into_iter().enumerate() {
            let ax = &mut self.axes[i];
            if !ax.enabled {
                continue;
            }
            ax.motion = Motion::Inertia(Inertia::new(t, p, v, ax.min, ax.max, &params));
        }
        self.samples.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 大范围双轴跟踪器（测自由惯性 / 摆动，不触边界）。
    fn wide() -> ScrollTracker {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([-1.0e9, -1.0e9], [1.0e9, 1.0e9]);
        tr
    }

    /// 在 `t` 松手，纵向初速 `v`：先 `set_position` 到 `p0 - v*0.05`，再跟手 0.05 s。
    fn release_with_velocity(v: f64, t_release: f64) -> ScrollTracker {
        let mut tr = wide();
        let t_begin = t_release - 0.05;
        tr.set_position(t_begin, 0.0, -v * 0.05);
        tr.finger_begin(t_begin);
        tr.finger_delta(t_release, 0.0, v * 0.05);
        tr.finger_end(t_release);
        tr
    }

    /// 相位推进：硬置 = Idle，跟手 = Interacting，松手 = Inertia，折叠后 = Idle。
    #[test]
    fn phase_progression() {
        let mut tr = wide();
        tr.set_position(0.0, 0.0, 0.0);
        assert_eq!(tr.sample(0.0).1, Phase::Idle);
        tr.finger_begin(0.0);
        assert_eq!(tr.sample(0.0).1, Phase::Interacting);
        tr.finger_delta(0.05, 0.0, 30.0);
        tr.finger_end(0.05);
        assert_eq!(tr.sample(0.05).1, Phase::Inertia);
        tr.settle(10.0);
        assert_eq!(tr.sample(10.0).1, Phase::Idle);
    }

    /// 采样无关：采样不改状态，反复采样后同一时刻值仍与直接取值一致（解析式，非逐帧积分）。
    #[test]
    fn sampling_is_rate_independent() {
        let a = release_with_velocity(800.0, 0.05);
        let b = release_with_velocity(800.0, 0.05);
        let mut t = 0.05;
        while t < 2.0 {
            b.sample(t);
            t += 1.0 / 1000.0;
        }
        let mut t = 0.05;
        while t < 2.0 {
            let (pa, _) = a.sample(t);
            let (pb, _) = b.sample(t);
            assert!((pa[1] - pb[1]).abs() < 1.0e-12, "t={t}");
            t += 1.0 / 60.0;
        }
    }

    /// 松手无空档：松手后第一个 16.67 ms 的位移 ≈ v·Δ（相对误差 < 5%）。
    #[test]
    fn release_has_no_gap() {
        let tr = release_with_velocity(200.0, 0.05);
        let p0 = tr.sample(0.05).0[1];
        let dt = 1.0 / 60.0;
        let p1 = tr.sample(0.05 + dt).0[1];
        let expect = 200.0 * dt;
        let rel = ((p1 - p0) - expect).abs() / expect;
        assert!(rel < 0.05, "位移 {} vs v·Δ {expect}", p1 - p0);
    }

    /// `scroll_by`：一次 48 恰停 +48，三次累加 +144；边界夹住；方向反转回 0。
    #[test]
    fn scroll_by_lands_and_accumulates() {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        assert!((tr.sample(5.0).0[1] - 48.0).abs() < 0.01, "{}", tr.sample(5.0).0[1]);

        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        tr.scroll_by(0.0, 0.0, 48.0);
        tr.scroll_by(0.0, 0.0, 48.0);
        assert!((tr.sample(5.0).0[1] - 144.0).abs() < 0.01, "{}", tr.sample(5.0).0[1]);

        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [100.0, 100.0]);
        for _ in 0..3 {
            tr.scroll_by(0.0, 0.0, 48.0);
        }
        assert!((tr.sample(5.0).0[1] - 100.0).abs() < 0.01, "越界未夹住");

        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        tr.scroll_by(2.0, 0.0, -48.0);
        assert!(tr.sample(5.0).0[1].abs() < 0.01, "反转未归零 {}", tr.sample(5.0).0[1]);
    }

    /// 越界跟手：位移 = `M·(1 − e^{−x/M})`；松手后单调回贴边界、不越界。
    #[test]
    fn overscroll_follow_and_return() {
        let m = TrackerParams::default().overscroll_max;
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [300.0, 300.0]);
        tr.set_position(0.0, 0.0, 0.0);
        tr.finger_begin(0.0);
        tr.finger_delta(1.0, 0.0, 400.0);
        let want = 300.0 + m * (1.0 - (-100.0 / m).exp());
        assert!((tr.sample(1.0).0[1] - want).abs() < 1.0e-9, "跟手 {} vs {want}", tr.sample(1.0).0[1]);
        // 末尾静止样本 → 松手走回程。
        tr.finger_delta(1.2, 0.0, 0.0);
        tr.finger_delta(1.3, 0.0, 0.0);
        tr.finger_end(1.3);
        let mut prev = tr.sample(1.3).0[1];
        let mut t = 1.3;
        while t < 2.2 {
            t += 1.0 / 120.0;
            let x = tr.sample(t).0[1];
            assert!(x <= prev + 1.0e-9, "回程不单调 t={t}");
            prev = x;
        }
        assert!((prev - 300.0).abs() < 0.01, "回程未到边界 {prev}");
    }
}
