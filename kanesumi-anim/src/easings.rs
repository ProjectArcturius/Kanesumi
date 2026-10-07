use sokuou::{EasingMode, UwpEasing, apply_uwp};

/// 默认 UWP 缓动：Quadratic / EaseOut。0.25s 标准时长搭配。
///
/// 命名缓动族 —— Kanesumi 短时、克制的过渡。参 Sokuou `uwp.rs`（UWP EasingFunctionBase 全量移植）。
/// 用法：把归一化进度 t ∈ [0,1] 映射为缓动后的进度。
///
/// ```
/// use kanesumi_anim::metro_default;
/// let eased = metro_default(0.5); // 0.75（Quadratic EaseOut 前快后缓）
/// ```
pub fn metro_default(t: f64) -> f64 {
    apply_uwp(t, &UwpEasing::Quadratic, EasingMode::EaseOut)
}

pub fn metro_cubic(t: f64) -> f64 {
    apply_uwp(t, &UwpEasing::Cubic, EasingMode::EaseOut)
}

pub fn metro_out_quart(t: f64) -> f64 {
    apply_uwp(t, &UwpEasing::Quartic, EasingMode::EaseOut)
}

pub fn metro_quintic(t: f64) -> f64 {
    apply_uwp(t, &UwpEasing::Quintic, EasingMode::EaseOut)
}

pub fn metro_sine(t: f64) -> f64 {
    apply_uwp(t, &UwpEasing::Sine, EasingMode::EaseOut)
}

// ── 图层转场曲线与 cubic-bezier 求值 ──────────────────────────────────────
//
// 合成图层动画（元素树 `Transitions`）的缓动是 cubic-bezier (x1,y1,x2,y2)：
// 与 harness `kanesumi-harness/src/layers.rs` 同名常量同值（那里外壳协议用、这里元素树
// 估值用，两处都指同一 UWP 曲线；等上层收敛时可删一处）。参 ANIMATION_SPEC §Ⅴ。

/// UWP 开曲线（弹层 / 面板展开、入场）。参 `UWP_PRIMARY_SOURCES.md`。
pub const CURVE_UWP_OPEN: [f32; 4] = [0.1, 0.9, 0.2, 1.0];
/// UWP 关曲线（收起、离场）。
pub const CURVE_UWP_CLOSE: [f32; 4] = [0.7, 0.0, 1.0, 0.5];
/// 立方缓出 —— Reposition（定位 / 重排）类快停曲线，近似 `Cubic EaseOut`。
pub const CURVE_EASE_OUT_CUBIC: [f32; 4] = [0.33, 1.0, 0.68, 1.0];

/// cubic-bezier 缓动求值：给定归一化时间 `x ∈ [0,1]`，返回缓动后进度 `y`。
///
/// 控制点 `(x1,y1,x2,y2)`，端点固定 `P0=(0,0)`、`P3=(1,1)`（同 CSS `cubic-bezier`）。
/// 牛顿迭代反解参数 `u` 使 `X(u)=x`，再求 `Y(u)`；牛顿不收敛时二分兜底。
/// 元素树「从当前呈现值中断接续」时用它把已过时间换算成当前视觉值（参 `Transitions`）。
pub fn bezier_y(curve: [f32; 4], x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0) as f64;
    let [x1, y1, x2, y2] = curve.map(|v| v as f64);
    let bx = |u: f64| {
        let m = 1.0 - u;
        3.0 * m * m * u * x1 + 3.0 * m * u * u * x2 + u * u * u
    };
    let by = |u: f64| {
        let m = 1.0 - u;
        3.0 * m * m * u * y1 + 3.0 * m * u * u * y2 + u * u * u
    };
    let dbx = |u: f64| {
        let m = 1.0 - u;
        3.0 * m * m * x1 + 6.0 * m * u * (x2 - x1) + 3.0 * u * u * (1.0 - x2)
    };
    // 恒等曲线（控制点都在对角线）直接返回。
    if (x1 - y1).abs() < 1e-9 && (x2 - y2).abs() < 1e-9 {
        return x as f32;
    }
    let mut u = x;
    for _ in 0..8 {
        let err = bx(u) - x;
        if err.abs() < 1e-7 {
            break;
        }
        let d = dbx(u);
        if d.abs() < 1e-9 {
            break;
        }
        u = (u - err / d).clamp(0.0, 1.0);
    }
    if (bx(u) - x).abs() > 1e-4 {
        // 牛顿失败：单调区间二分。
        let (mut lo, mut hi) = (0.0, 1.0);
        for _ in 0..40 {
            u = (lo + hi) / 2.0;
            if bx(u) < x {
                lo = u;
            } else {
                hi = u;
            }
        }
    }
    by(u).clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ease_out_endpoints() {
        assert_eq!(metro_default(0.0), 0.0);
        assert_eq!(metro_default(1.0), 1.0);
    }

    #[test]
    fn ease_out_starts_fast() {
        // EaseOut 前段快：t=0.5 时应 > 0.25（线性基准）
        assert!(metro_default(0.5) > 0.25);
    }

    #[test]
    fn mono_increasing() {
        let mut prev = 0.0;
        for i in 0..=100 {
            let t = i as f64 / 100.0;
            let v = metro_cubic(t);
            assert!(v >= prev, "t={t} 不增");
            prev = v;
        }
    }

    #[test]
    fn bezier_endpoints_and_linear() {
        assert!((bezier_y(CURVE_UWP_OPEN, 0.0) - 0.0).abs() < 1e-6);
        assert!((bezier_y(CURVE_UWP_OPEN, 1.0) - 1.0).abs() < 1e-6);
        // 恒等曲线逐点等于输入。
        for i in 0..=10 {
            let t = i as f32 / 10.0;
            assert!((bezier_y([0.0, 0.0, 1.0, 1.0], t) - t).abs() < 1e-5);
        }
    }

    #[test]
    fn bezier_open_is_fast_out_and_monotonic() {
        // 开曲线前段快：x=0.5 时缓动后明显大于线性。
        assert!(bezier_y(CURVE_UWP_OPEN, 0.5) > 0.5);
        let mut prev = 0.0;
        for i in 0..=200 {
            let t = i as f32 / 200.0;
            let v = bezier_y(CURVE_UWP_OPEN, t);
            assert!(v >= prev - 1e-4, "t={t} 非单调");
            prev = v;
        }
    }

    #[test]
    fn bezier_close_starts_slow() {
        // 关曲线前段慢（起步缓、末段快）。
        assert!(bezier_y(CURVE_UWP_CLOSE, 0.5) < 0.5);
    }
}
