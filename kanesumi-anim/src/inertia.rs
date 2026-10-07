// inertia.rs —— 摩擦惯性衰减原语（触摸板松手后的自由滚动）。
//
// 模型：一阶指数衰减 `p(t) = p₀ + v₀·τ·(1 − e^{−t/τ})`、`v(t) = v₀·e^{−t/τ}`。
// 这是 WinUI `InteractionTracker` 的默认位置惯性 —— `w-comp-ref` §① 探针实测为
// 一阶指数（`A = v₀·τ` 恒定，RMSE < 0.3 px），与 UWP 缓动族无关，故在 Kanesumi
// 动画层单列一个原语，而非以弹簧近似。
//
// 本原语只推进位置 / 速度，**不知边界**：撞边界由宿主（滚动容器）夹紧后调
// [`FrictionAnim::stop`] 即停，不做橡皮筋回弹（参 `w-comp-ref` §① 边界实验）。

use crate::animation::Animation;

/// 一阶指数摩擦衰减。
///
/// - `tau`：时间常数（秒），由衰减率 `d` 换算 `τ = −1/ln(1−d)`（`w-comp-ref` §①）。
/// - `stop_velocity`：停止阈值（单位/秒），速度绝对值降到其下即吸附进入稳态。
#[derive(Debug, Clone)]
pub struct FrictionAnim {
    pos: f64,
    vel: f64,
    tau: f64,
    stop_velocity: f64,
    steady: bool,
}

impl FrictionAnim {
    /// 创建静止于 `initial`（默认 0）的摩擦衰减，尚未启动。
    pub fn new(tau: f64, stop_velocity: f64) -> Self {
        Self {
            pos: 0.0,
            vel: 0.0,
            tau: tau.max(f64::MIN_POSITIVE),
            stop_velocity: stop_velocity.max(0.0),
            steady: true,
        }
    }

    /// 从位置 `from`、初速度 `velocity` 开始衰减（取代任何进行中的惯性）。
    pub fn start(&mut self, from: f64, velocity: f64) {
        self.pos = from;
        self.vel = velocity;
        self.steady = false;
    }

    /// 立即吸附当前速度到零并进入稳态（撞边界 / 被新输入打断）。
    pub fn stop(&mut self) {
        self.vel = 0.0;
        self.steady = true;
    }

    /// 当前速度（单位/秒）。
    pub fn velocity(&self) -> f64 {
        self.vel
    }
}

impl Animation for FrictionAnim {
    fn advance(&mut self, dt: f64) {
        if self.steady || dt <= 0.0 {
            return;
        }
        // 解析解积分：位移 = ∫ v₀·e^{−t/τ} dt = v₀·τ·(1 − e^{−dt/τ})。
        let decay = (-dt / self.tau).exp();
        self.pos += self.vel * self.tau * (1.0 - decay);
        self.vel *= decay;
        if self.vel.abs() < self.stop_velocity {
            self.vel = 0.0;
            self.steady = true;
        }
    }

    fn is_steady(&self) -> bool {
        self.steady
    }

    fn value(&self) -> f64 {
        self.pos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 一阶指数：v₀=1000、τ=0.3338 的总位移 ≈ v₀·τ ≈ 333.8（`w-comp-ref` §① 模型）。
    #[test]
    fn total_distance_follows_velocity_times_tau() {
        let mut a = FrictionAnim::new(0.3338, 30.0);
        a.start(0.0, 1000.0);
        for _ in 0..1000 {
            a.advance(1.0 / 60.0);
            if a.is_steady() {
                break;
            }
        }
        assert!(a.is_steady());
        let expected = 1000.0 * 0.3338 - 30.0 * 0.3338;
        assert!(
            (a.value() - expected).abs() < expected * 0.02,
            "总位移 {} vs 期望约 {expected}",
            a.value()
        );
    }

    /// 惯性距离随初速度单调（同 τ / 阈值）。
    #[test]
    fn distance_is_monotonic_in_velocity() {
        let travel = |v: f64| {
            let mut a = FrictionAnim::new(0.3338, 30.0);
            a.start(0.0, v);
            for _ in 0..2000 {
                a.advance(1.0 / 60.0);
                if a.is_steady() {
                    break;
                }
            }
            a.value()
        };
        assert!(travel(300.0) < travel(1000.0));
        assert!(travel(1000.0) < travel(4000.0));
    }

    /// 停止后不再移动；速度降到阈值即吸附。
    #[test]
    fn stops_at_threshold() {
        let mut a = FrictionAnim::new(0.3338, 30.0);
        a.start(0.0, 30.0 * 1.5);
        let mut frames = 0;
        while !a.is_steady() && frames < 1000 {
            a.advance(1.0 / 60.0);
            frames += 1;
        }
        assert!(a.is_steady());
        assert_eq!(a.velocity(), 0.0);
        let settled = a.value();
        a.advance(1.0);
        assert_eq!(a.value(), settled, "稳态后不再推进");
    }

    /// 负速度（向上 / 向左）对称衰减。
    #[test]
    fn negative_velocity_decays_symmetrically() {
        let mut a = FrictionAnim::new(0.3338, 30.0);
        a.start(0.0, -2000.0);
        for _ in 0..1000 {
            a.advance(1.0 / 60.0);
            if a.is_steady() {
                break;
            }
        }
        assert!(a.value() < 0.0);
    }
}
