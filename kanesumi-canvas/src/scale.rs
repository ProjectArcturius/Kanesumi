// scale.rs —— 当前表面的逻辑 → 物理缩放（外壳注入的进程级单一真源）。
//
// 文字由外壳在光栅化时按引擎已知的 scale 出物理字形；图标是在**应用侧**预光栅成
// `Icon` 位图再交给外壳 blit 的，应用必须知道同一次缩放才能按物理像素出图（参
// ether-theme docs/ICON_DESIGN.md §ⅩⅣ-3「按 pt × 输出缩放直接出图」）。
//
// 外壳在建立 surface / 收到 `wl_surface.enter` / `wp_fractional_scale_v1` 时注入；
// 应用在光栅前读取。以 1/120 为单位存储（与 wp_fractional_scale_v1 的 120 分母同构，
// 整数 2× 与 1.5× 均可表示）；未注入时默认 1×。

use std::sync::atomic::{AtomicU32, Ordering};

/// 缩放（×120）。默认 120 = 1.0。
static SURFACE_SCALE: AtomicU32 = AtomicU32::new(120);

/// 缩放单位：整数 1× 的 ×120 表示。
const SCALE_UNIT: u32 = 120;

/// 外壳注入当前表面缩放。非有限 / ≤ 0 忽略（坏值不得污染全局）。
pub fn set_surface_scale(scale: f32) {
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }
    let units = (scale * SCALE_UNIT as f32).round().max(1.0) as u32;
    SURFACE_SCALE.store(units, Ordering::Relaxed);
}

/// 当前表面缩放（默认 1.0）。
pub fn surface_scale() -> f32 {
    SURFACE_SCALE.load(Ordering::Relaxed) as f32 / SCALE_UNIT as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 注：全进程共享一个原子，测试串行断言（用唯一值再还原）。
    #[test]
    fn default_is_one_and_roundtrips() {
        set_surface_scale(1.0);
        assert_eq!(surface_scale(), 1.0);
        set_surface_scale(2.0);
        assert_eq!(surface_scale(), 2.0);
        set_surface_scale(1.5);
        assert!((surface_scale() - 1.5).abs() < 1e-3);
        // 坏值忽略：保持上一次有效值。
        set_surface_scale(0.0);
        assert!((surface_scale() - 1.5).abs() < 1e-3);
        set_surface_scale(-3.0);
        assert!((surface_scale() - 1.5).abs() < 1e-3);
        set_surface_scale(f32::NAN);
        assert!((surface_scale() - 1.5).abs() < 1e-3);
        set_surface_scale(1.0);
    }
}
