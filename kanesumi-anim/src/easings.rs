use sokuou::{EasingMode, UwpEasing, apply_uwp};

/// 默认 Metro 缓动：Quadratic / EaseOut。0.25s 标准时长搭配。
///
/// 命名缓动族 —— Metro 短时、克制的过渡。参 Sokuou `uwp.rs`（UWP EasingFunctionBase 全量移植）。
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
}
