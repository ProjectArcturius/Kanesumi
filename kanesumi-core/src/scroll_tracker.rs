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
///
/// 越界量先钳到 `[0, M·(1 − 1e-9)]` 再取对数（任务书 2026-10-11 必修 3）：反变换对任何
/// 越界量（含惯性峰值、异常注入）都不得产生 NaN / inf。钳位后往返误差 < 1e-6，跟手不跳。
fn overscroll_raw(display: f64, min: f64, max: f64, m: f64) -> f64 {
    let cap = m * (1.0 - 1.0e-9);
    if display > max {
        let over = (display - max).clamp(0.0, cap);
        max - m * (1.0 - over / m).ln()
    } else if display < min {
        let over = (min - display).clamp(0.0, cap);
        min + m * (1.0 - over / m).ln()
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
            it.bounce(to, ret, stop, params.overscroll_max);
        } else {
            if v0.abs() <= stop {
                it.free_stop = true;
                return it;
            }
            let cross = (it.b - p0) / (v0 * tau);
            // 停止速度先于边界（任务书 2026-10-11 必修 2）：自然停点 `p_end` 在界内就停在那里，
            // 不触边。`cross` 是到边界的位移 / 一阶渐近位移，`1 − stop/|v0|` 即自然停点比例。
            if cross >= 1.0 - stop / v0.abs() {
                it.free_stop = true;
                it.t_stop = tau * (v0.abs() / stop).ln();
                it.p_end = p0 + v0 * tau * (1.0 - stop / v0.abs());
                it.end_t = it.t_stop;
                return it;
            }
            it.tc = -tau * (1.0 - cross).ln();
            it.v_hit = v0 - (it.b - p0) / tau;
            it.ex0 = 0.0;
            it.bounce(to, ret, stop, params.overscroll_max);
        }
        it
    }

    /// 越界减速到峰值再回程（相对时刻，`it.tc` 已定）。
    ///
    /// 峰值封顶在越界上限 `m`（任务书 2026-10-11 必修 6：外向越界松手的峰值越界 < M）：
    /// 与跟手饱和曲线同界，且令 [`overscroll_raw`] 的反变换始终落在定义域内。若位移先到
    /// `m`，`t_pk` 取到达 `m` 的时刻；到不了 `m` 则取速度衰到 `stop` 的时刻（原物理峰值）。
    fn bounce(&mut self, to: f64, ret: f64, stop: f64, m: f64) {
        let av = self.v_hit.abs();
        let cap = m * (1.0 - 1.0e-9);
        if av > stop {
            let t_pk_raw = self.tc + to * (av / stop).ln();
            let peak_raw = self.ex0 + av * to * (1.0 - stop / av);
            if self.ex0 >= cap {
                // 起点已在封顶处（仅异常注入可达）：不再外移，直接回程。
                self.t_pk = self.tc;
                self.peak = self.ex0;
            } else if peak_raw > cap {
                let denom = av * to - (cap - self.ex0);
                self.t_pk = if denom > 0.0 { self.tc + to * (av * to / denom).ln() } else { t_pk_raw };
                self.peak = cap;
            } else {
                self.t_pk = t_pk_raw;
                self.peak = peak_raw;
            }
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

    /// `t` 时刻的相位（任务书 2026-10-11 必修 1）：运动段到 `t` 已结束即 `Idle`。
    ///
    /// 相位是 `t` 的函数，不依赖 [`ScrollTracker::settle`]；故 `sample(t)` 对同一状态同一 `t`
    /// 恒定，`settle` 前后结果相同。
    fn phase_at(&self, t: f64) -> Phase {
        match self.motion {
            Motion::Idle { .. } => Phase::Idle,
            Motion::Follow { .. } => Phase::Interacting,
            Motion::Inertia(it) => {
                if t >= it.t0 + it.end_t {
                    Phase::Idle
                } else {
                    Phase::Inertia
                }
            }
            Motion::Anim { t0, end_t, .. } => {
                if t >= t0 + end_t {
                    Phase::Idle
                } else {
                    Phase::Animating
                }
            }
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
            phase = match (phase, ax.phase_at(t)) {
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

/// 起一段 `scroll_by` / `scroll_to` 动画：`end_pos` 恰为目标，无结束硬跳。
///
/// 位置在 `t` 处连续（新段从 `x_now` 起步）；目标 = 调用前目标 + Δ（夹界，任务书
/// 2026-10-11 必修 5）。解析曲线 `x0 + v0·τ·(1 − e^{−dt/τ})`，结束时刻由「曲线到达目标」
/// 解出：`end_t = −τ·ln(1 − (target − x0)/(v0·τ))`，故 `end_t` 处解析值 = `target`、末速 ≈
/// 停止速度，`eval` 在 `dt ≥ end_t` 保持 `end_pos` 不会产生跳变（裁定后续修 1：不得靠结束
/// 硬跳凑终点）。若改用「速度衰到停止速度」定 `end_t`，会因 `SCROLL_BY_VELOCITY_PER_PX` 是
/// `−ln(1 − 衰减率)` 的**四舍五入值**（`k·τ ≈ 1 − 6.1e-6`）使终点差约 `|d|·6.1e-6`，
/// 逼近终点时即成为小硬跳。
///
/// 兜底：调用方给出的 `v0` 由解析键盘模型保证方向与 `target − x0` 一致且 `|v0| > 停止速度`；
/// 浮点退化时按方向以两倍停止速度补足，避免 `end_t` 非正导致瞬跳。
fn start_anim(ax: &mut Axis, t: f64, x0: f64, v0: f64, target: f64, params: &TrackerParams) {
    let tau = params.scroll_by_tau();
    let stop = params.stop_velocity;
    if (target - x0).abs() <= EPS {
        ax.motion = Motion::Idle { pos: target };
        return;
    }
    let v0 = if v0.abs() <= stop { (target - x0).signum() * stop * 2.0 } else { v0 };
    let frac = ((target - x0) / (v0 * tau)).clamp(0.0, 1.0 - 1.0e-12);
    let end_t = -tau * (1.0 - frac).ln();
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

    /// 最近速度窗内的 `Δp/Δt`（每轴）（任务书 2026-10-11 必修 4）：**只用** `t` 前
    /// `velocity_window_s` 内的样本；窗内不足两点（含无位移）即速度 0，不退回归整轮历史。
    fn estimate_velocity(&self, axis: usize, t: f64, params: &TrackerParams) -> f64 {
        let start = t - params.velocity_window_s;
        let recent: Vec<(f64, [f64; 2])> = self
            .samples
            .iter()
            .copied()
            .filter(|(ts, _)| *ts >= start && *ts <= t)
            .collect();
        if recent.len() < 2 {
            return 0.0;
        }
        let (t0, p0) = recent[0];
        let (t1, p1) = recent[recent.len() - 1];
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
            // 新起点 / 反向：解析键盘模型的起速由「当前位置 → 新目标」的方向与距离确定。
            // 加号项必须随行进方向 `target − x_now`，而非目标增量 `added` 的方向：新目标落在
            // 当前位置与旧目标之间时（部分反向），两者方向相反，旧式 `added.signum()` 会让解析
            // 曲线到不了目标、结束点硬跳（裁定后续修 1）。基准速度 = 停止速度（皆 30），起速与
            // `target − x_now` 同向；终止时刻由 `start_anim` 按「曲线到达目标」解出，终点恰为目标。
            (target - x_now) * k + (target - x_now).signum() * params.baseline_velocity
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

    /// 必修 1：相位是 `t` 的函数——运动段结束后 `sample` 归 `Idle`，且与 `settle` 无关。
    #[test]
    fn phase_is_function_of_time() {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let (p, ph) = tr.sample(10.0);
        assert_eq!(ph, Phase::Idle, "运动段结束后应为 Idle");
        assert!((p[1] - 48.0).abs() < 0.01, "位置 {}", p[1]);
        let before = tr.sample(10.0);
        tr.settle(10.0);
        assert_eq!(before, tr.sample(10.0), "settle 不得改变 sample 结果");
    }

    /// 必修 2：停止速度先于边界——自然停点在界内就地停住、从不触边。
    #[test]
    fn natural_stop_before_boundary() {
        let tau = TrackerParams::default().tau_s;
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1.0e5, 30.0]);
        tr.flick(0.0, 0.0, 0.0, 0.0, 100.0);
        let expect = 100.0 * tau * (1.0 - 30.0 / 100.0);
        let mut peak = f64::NEG_INFINITY;
        let mut t = 0.0;
        while t <= 2.0 {
            peak = peak.max(tr.sample(t).0[1]);
            t += 1.0e-3;
        }
        let end = tr.sample(2.0);
        assert!((end.0[1] - expect).abs() < 0.01, "停点 {} vs {expect}", end.0[1]);
        assert_eq!(end.1, Phase::Idle);
        assert!(peak <= 30.0 + 1.0e-9, "从未越过上界，峰值 {peak}");
    }

    /// 必修 3：越界反变换对任意越界量有限；峰值附近按下位置连续。
    #[test]
    fn overscroll_inverse_finite_and_continuous() {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1.0e5, 300.0]);
        tr.flick(0.0, 0.0, 299.0, 0.0, 8000.0);
        let (mut t_pk, mut best) = (0.0, f64::NEG_INFINITY);
        let mut t = 0.0;
        while t < 0.5 {
            let v = tr.sample(t).0[1];
            assert!(v.is_finite(), "t={t} 非有限 {v}");
            if v > best {
                best = v;
                t_pk = t;
            }
            t += 1.0e-4;
        }
        let before = tr.sample(t_pk).0[1];
        tr.finger_begin(t_pk);
        let after = tr.sample(t_pk).0[1];
        assert!(after.is_finite(), "按下后非有限 {after}");
        assert!((after - before).abs() < 1.0e-6, "按下不连续 {before} → {after}");
    }

    /// 必修 3 性质：固定种子随机 2000 组操作，每个 `sample` 输出有限。
    #[test]
    fn all_inputs_finite() {
        let mut s: u64 = 0x1234_5678_9abc_def0;
        let mut rnd = move || -> u64 {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            s >> 11
        };
        let unit = |raw: u64| (raw as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0;
        for _ in 0..2000 {
            let v = unit(rnd()) * 1.0e5;
            let base = unit(rnd()) * 5000.0;
            let width = unit(rnd()).abs() * 5000.0;
            let mut tr = ScrollTracker::new(TrackerParams::default());
            tr.set_bounds([base, base], [base + width, base + width]);
            tr.set_axes(rnd() & 1 == 0, rnd() & 1 == 0);
            let mut t = 0.0;
            for _ in 0..8 {
                match rnd() % 6 {
                    0 => tr.finger_begin(t),
                    1 => tr.finger_delta(t, unit(rnd()) * 200.0, unit(rnd()) * 200.0),
                    2 => tr.finger_end(t),
                    3 => tr.scroll_by(t, unit(rnd()) * 100.0, unit(rnd()) * 100.0),
                    4 => tr.set_position(t, unit(rnd()) * 500.0, unit(rnd()) * 500.0),
                    _ => tr.flick(t, unit(rnd()) * 500.0, unit(rnd()) * 500.0, v, v),
                }
                for k in 0..4 {
                    let (p, _) = tr.sample(t + k as f64 * 0.05);
                    assert!(p[0].is_finite() && p[1].is_finite(), "非有限 {p:?} @ t={t}");
                }
                t += 0.05;
            }
        }
    }

    /// 必修 5：运动中 `scroll_by` 位置连续、目标累加；反向一次回到 0。
    #[test]
    fn scroll_by_during_motion_continuous() {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let t = 0.1;
        let before = tr.sample(t - 1.0e-9).0[1];
        tr.scroll_by(t, 0.0, 48.0);
        let after = tr.sample(t).0[1];
        assert!((after - before).abs() < 1.0e-6, "位置跳变 {before} → {after}");
        assert!((tr.sample(5.0).0[1] - 96.0).abs() < 0.01, "累加目标 {}", tr.sample(5.0).0[1]);

        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        tr.scroll_by(0.1, 0.0, -48.0);
        assert!(tr.sample(5.0).0[1].abs() < 0.01, "反向终点 {}", tr.sample(5.0).0[1]);
    }

    /// 必修 6：外向越界松手——先按 `τ_over` 继续外移，再单调回程；峰值越界 < M、终点 = 边界。
    #[test]
    fn outward_overscroll_release() {
        let m = TrackerParams::default().overscroll_max;
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1.0e5, 300.0]);
        tr.set_position(0.0, 0.0, 300.0);
        tr.finger_begin(0.0);
        tr.finger_delta(0.02, 0.0, 50.0);
        tr.finger_delta(0.03, 0.0, 50.0);
        let start = tr.sample(0.03).0[1];
        tr.finger_end(0.03);
        let mut ps = Vec::new();
        let mut t = 0.03;
        while t < 1.0 {
            ps.push(tr.sample(t).0[1]);
            t += 1.0e-3;
        }
        let (ipk, peak) = ps
            .iter()
            .enumerate()
            .fold((0usize, f64::NEG_INFINITY), |(bi, bv), (i, &v)| if v > bv { (i, v) } else { (bi, bv) });
        assert!(peak > start, "应先继续外移：start={start} peak={peak}");
        assert!(peak - 300.0 < m, "峰值越界 {} 未 < M={m}", peak - 300.0);
        for i in ipk + 1..ps.len() {
            assert!(ps[i] <= ps[i - 1] + 1.0e-9, "回程非单调 i={i}");
        }
        assert!((ps[ps.len() - 1] - 300.0).abs() < 0.01, "终点 {}", ps[ps.len() - 1]);
    }

    /// 二分求运动段结束时刻（相位首次归 `Idle`）；`[lo, hi]` 须括号住这一刻。
    fn phase_end(tr: &ScrollTracker, mut lo: f64, mut hi: f64) -> f64 {
        for _ in 0..80 {
            let mid = 0.5 * (lo + hi);
            if tr.sample(mid).1 == Phase::Idle {
                hi = mid;
            } else {
                lo = mid;
            }
        }
        hi
    }

    /// 断言 `scroll_by` 结果的调用点连续、结束点连续、恰好停在 `target`。
    fn assert_scroll_by_lands(tr: &ScrollTracker, t_call: f64, before: f64, target: f64, tag: &str) {
        let after = tr.sample(t_call).0[1];
        assert!((after - before).abs() < 1.0e-6, "{tag}: 调用点不连续 {before} → {after}");
        let te = phase_end(tr, t_call, t_call + 5.0);
        let left = tr.sample(te - 1.0e-9).0[1];
        let right = tr.sample(te).0[1];
        assert!((right - left).abs() < 1.0e-6, "{tag}: 结束点跳变 {left} → {right}");
        assert!((right - target).abs() < 1.0e-6, "{tag}: 终点 {right} vs 目标 {target}");
    }

    /// 必修 5 补：运动中 `scroll_by`（部分反向 / 完全反向 / 极小目标 / 同向追加）——
    /// 调用点与结束点都连续、恰好停在夹界后的累加目标，不得靠结束硬跳到位。
    #[test]
    fn scroll_by_mid_flight_lands_continuously() {
        // 部分反向：增量方向与当前速度相反，但新目标仍在当前位置前方（旧实现按增量符号
        // 定起速，结束点与解析曲线差约 7.9 px，靠硬跳补上）。
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let before = tr.sample(0.1 - 1.0e-9).0[1];
        tr.scroll_by(0.1, 0.0, -5.0);
        assert_scroll_by_lands(&tr, 0.1, before, 43.0, "部分反向");

        // 完全反向：新目标 = 起点的 0。
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let before = tr.sample(0.1 - 1.0e-9).0[1];
        tr.scroll_by(0.1, 0.0, -48.0);
        assert_scroll_by_lands(&tr, 0.1, before, 0.0, "完全反向");

        // 极小目标距离：新目标仅略在当前位置前方（位速方向与增量符号相反）。
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let before = tr.sample(0.1 - 1.0e-9).0[1];
        tr.scroll_by(0.1, 0.0, -20.0);
        assert_scroll_by_lands(&tr, 0.1, before, 28.0, "极小目标");

        // 同向追加：目标累加。
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1000.0, 1000.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        let before = tr.sample(0.1 - 1.0e-9).0[1];
        tr.scroll_by(0.1, 0.0, 48.0);
        assert_scroll_by_lands(&tr, 0.1, before, 96.0, "同向追加");
    }

    /// 必修 5 补：运动中同向追加，目标夹界后不得越过该目标。
    #[test]
    fn scroll_by_clamped_target_not_overshot() {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [100.0, 100.0]);
        tr.scroll_by(0.0, 0.0, 48.0);
        tr.scroll_by(0.1, 0.0, 100.0);
        let mut t = 0.1;
        while t < 3.0 {
            assert!(tr.sample(t).0[1] <= 100.0 + 1.0e-6, "越过夹界目标 t={t} {}", tr.sample(t).0[1]);
            t += 1.0e-3;
        }
        assert!((tr.sample(3.0).0[1] - 100.0).abs() < 1.0e-6, "终点 {}", tr.sample(3.0).0[1]);
    }
}
