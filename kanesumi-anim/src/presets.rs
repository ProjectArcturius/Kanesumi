use sokuou::{EasingMode, MetroAnim, SpringAnim, UwpEasing};

/// Metro 标准动画时长：0.25s。轻盈短促、60Hz 友好。参 PLAN.md §2。
pub const METRO_STANDARD_DURATION: f64 = 0.25;

/// 面板 / 弹窗入场时长。
pub const DURATION_SHEET_APPEAR: f64 = 0.30;
/// 面板 / 弹窗收起时长。短促收敛，避免拖尾。
pub const DURATION_SHEET_DISMISS: f64 = 0.26;
/// 快速切换（列表高亮、播放/暂停图标）。对齐 UWP ControlFastAnimationDuration 0.167s。
pub const DURATION_QUICK_SWITCH: f64 = 0.167;
/// 封面 / 大图淡入。
pub const DURATION_COVER_FADE: f64 = 0.40;
/// 颜色 / 主题过渡（accent transition）。
pub const DURATION_COLOR_TRANSITION: f64 = 0.30;
/// 开关滑动。对齐 UWP RepositionThemeAnimation 0.15s（Cubic EaseOut）。参 CONTROL_SPEC §3。
pub const DURATION_TOGGLE_FLIP: f64 = 0.15;
/// ProgressBar/Ring 不确定模式循环周期。对齐 UWP 2.0s。参 CONTROL_SPEC §4/§5。
pub const DURATION_INDETERMINATE: f64 = 2.0;
/// 下拉面板遮罩淡入。对齐 UWP OverlayOpeningAnimation 0.383s。参 CONTROL_SPEC §8。
pub const DURATION_OVERLAY_OPEN: f64 = 0.383;
/// 下拉面板遮罩淡出。对齐 UWP OverlayClosingAnimation 0.216s。
pub const DURATION_OVERLAY_CLOSE: f64 = 0.216;
/// 对话框本体缩放。对齐 UWP ContentDialog 0.5s（spline 0.1,0.9,0.2,1）。参 CONTROL_SPEC §9。
pub const DURATION_DIALOG_ENTER: f64 = 0.5;
/// 对话框淡入（含遮罩）。UWP 0.167s 线性。
pub const DURATION_DIALOG_FADE_IN: f64 = 0.167;
/// 对话框淡出。UWP 0.083s 线性（opacity 先行熄灭）。
pub const DURATION_DIALOG_FADE_OUT: f64 = 0.083;

/// Metro 动画预设。参 PLAN.md §4-1/§4-2 动画三层：
///
/// - **主运动**（位移 / 缩放 / 面板滑入）→ `SpringAnim` 解析解弹簧，帧率无关、可中断。
/// - **次要属性**（透明度 / 颜色）→ `MetroAnim` 时长驱动 + UWP 缓动。
///
/// 弹簧参数来源：SOKUOU_ENGINE.md §6.3 Apple 风格典型值表；
/// 时长 / 曲线来源：真实 Metro / UWP 组件观测聚类。
pub struct MetroPresets;

impl MetroPresets {
    // ── 弹簧：主运动 ─────────────────────────────────────────────

    /// 通用交互（点击、面板切换）。response=0.5s, damping=0.825。
    pub fn standard_interaction() -> SpringAnim {
        SpringAnim::new(0.50, 0.825, 0.0)
    }

    /// 快速交互（按钮按下、图标反馈）。response=0.3s, damping=0.6。
    pub fn quick_interaction() -> SpringAnim {
        SpringAnim::new(0.30, 0.60, 0.0)
    }

    /// 慢速展示（通知横幅、引导提示）。response=0.65s, damping=0.85。
    pub fn slow_reveal() -> SpringAnim {
        SpringAnim::new(0.65, 0.85, 0.0)
    }

    /// 弹窗强调弹入（对话框、菜单）。response=0.45s, damping=0.7。
    pub fn dialog_enter() -> SpringAnim {
        SpringAnim::new(0.45, 0.70, 0.0)
    }

    /// 页面转场。response=0.4s, damping=0.8。
    pub fn page_transition() -> SpringAnim {
        SpringAnim::new(0.40, 0.80, 0.0)
    }

    // ── 时长驱动：次要属性 ───────────────────────────────────────

    /// 面板入场：慢起快收，300ms Cubic/EaseOut（对齐 Metro 面板出场典型曲线）。
    pub fn sheet_appear() -> MetroAnim {
        MetroAnim::new(DURATION_SHEET_APPEAR, UwpEasing::Cubic, EasingMode::EaseOut)
    }

    /// 面板收起：260ms Quadratic/EaseOut，短促收敛。
    pub fn sheet_dismiss() -> MetroAnim {
        MetroAnim::new(
            DURATION_SHEET_DISMISS,
            UwpEasing::Quadratic,
            EasingMode::EaseOut,
        )
    }

    /// 快速切换：180ms Quadratic/EaseOut。
    pub fn quick_switch() -> MetroAnim {
        MetroAnim::new(
            DURATION_QUICK_SWITCH,
            UwpEasing::Quadratic,
            EasingMode::EaseOut,
        )
    }

    /// 封面淡入：400ms Cubic/EaseOut。
    pub fn cover_fade() -> MetroAnim {
        MetroAnim::new(DURATION_COVER_FADE, UwpEasing::Cubic, EasingMode::EaseOut)
    }

    /// 颜色过渡：300ms Quadratic/EaseOut。
    pub fn color_transition() -> MetroAnim {
        MetroAnim::new(
            DURATION_COLOR_TRANSITION,
            UwpEasing::Quadratic,
            EasingMode::EaseOut,
        )
    }

    /// 开关翻转：220ms Cubic/EaseOut（对齐 UWP toggle）。
    pub fn toggle_flip() -> MetroAnim {
        MetroAnim::new(DURATION_TOGGLE_FLIP, UwpEasing::Cubic, EasingMode::EaseOut)
    }

    /// 不确定进度循环：2.0s Cubic/EaseInOut（对齐 UWP ProgressBar/Ring，参 CONTROL_SPEC §4/§5）。
    pub fn progress_indeterminate() -> MetroAnim {
        MetroAnim::new(
            DURATION_INDETERMINATE,
            UwpEasing::Cubic,
            EasingMode::EaseInOut,
        )
    }

    /// 下拉面板遮罩淡入：383ms Cubic/EaseOut（对齐 UWP OverlayOpeningAnimation，§8）。
    pub fn overlay_open() -> MetroAnim {
        MetroAnim::new(DURATION_OVERLAY_OPEN, UwpEasing::Cubic, EasingMode::EaseOut)
    }

    /// 下拉面板遮罩淡出：216ms Quadratic/EaseOut（对齐 UWP OverlayClosingAnimation，§8）。
    pub fn overlay_close() -> MetroAnim {
        MetroAnim::new(
            DURATION_OVERLAY_CLOSE,
            UwpEasing::Quadratic,
            EasingMode::EaseOut,
        )
    }

    /// 对话框缩放：500ms Cubic/EaseOut（对齐 UWP spline 0.1,0.9,0.2,1，§9）。
    /// 淡入/淡出另用 `DURATION_DIALOG_FADE_IN/OUT` 常量 + 线性近似。
    pub fn dialog_scale() -> MetroAnim {
        MetroAnim::new(DURATION_DIALOG_ENTER, UwpEasing::Cubic, EasingMode::EaseOut)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_duration_is_metro() {
        assert_eq!(METRO_STANDARD_DURATION, 0.25);
    }

    #[test]
    fn metro_anim_is_interruptible() {
        let mut a = MetroPresets::sheet_appear();
        a.set_target(1.0);
        a.update(0.10);
        // 中途改目标：从当前值继续，不跳变
        a.set_target(0.5);
        let mid = a.value();
        assert!(mid > 0.0 && mid < 1.0);
        a.update(1.0);
        assert_eq!(a.value(), 0.5);
        assert!(a.is_steady());
    }

    #[test]
    fn spring_is_interruptible() {
        let mut s = MetroPresets::standard_interaction();
        s.set_target(1.0);
        s.update(0.10);
        s.set_target(0.0);
        assert!(s.value() < 1.0);
        for _ in 0..600 {
            s.update(1.0 / 60.0);
        }
        assert!(s.is_steady());
        assert_eq!(s.value(), 0.0);
    }

    #[test]
    fn durations_are_distinct() {
        assert!(DURATION_SHEET_DISMISS < DURATION_SHEET_APPEAR);
        assert!(DURATION_QUICK_SWITCH < DURATION_COVER_FADE);
    }

    #[test]
    fn toggle_flip_matches_uwp_reposition() {
        // 参 CONTROL_SPEC §10：UWP RepositionThemeAnimation 0.15s；不确定进度循环 2.0s
        assert_eq!(DURATION_TOGGLE_FLIP, 0.15);
        assert_eq!(DURATION_INDETERMINATE, 2.0);
    }
}
