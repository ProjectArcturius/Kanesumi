use sokuou::{Easing, EasingMode, MetroAnim, Progress, SpringAnim, UwpEasing};

/// Kanesumi 标准动画时长：0.25s。轻盈短促、60Hz 友好。参 PLAN.md §2。
pub const METRO_STANDARD_DURATION: f64 = 0.25;

/// 面板 / 弹窗入场时长。一手源：UWP `CommandBarFlyoutCommandBar` 的
/// `OpeningStoryboard` = **300ms**，`KeySpline="0.1,0.9 0.2,1"`（SDK 26100 `generic.xaml` L22093-22118）。
/// 这是 UWP 快照里唯一一组显式 flyout 开合时长（`FlyoutPresenter`/`MenuFlyoutPresenter`
/// 模板零 Storyboard，开关全靠系统预置）。
pub const DURATION_SHEET_APPEAR: f64 = 0.30;
/// 面板 / 弹窗收起时长。同源 = **150ms**、`KeySpline="0.7,0 1,0.5"`（同文件 L22110-22118）。
///
/// ⚠ 旧值 0.26 无依据（既不对应开合比，也不对应任何一手值），已按 150ms 更正 ——
/// UWP 的收起比展开快一倍，是「短促收敛」的量级来源。
pub const DURATION_SHEET_DISMISS: f64 = 0.15;
/// 快速切换（列表高亮、播放/暂停图标）。对齐 **WinUI**（非 UWP）`ControlFastAnimationDuration`
/// = 167ms。
///
/// ⚠ 归属更正（2026-09-22 实测）：`Control*AnimationDuration` 一族**不是 UWP 运行时资源** ——
/// UWP SDK 的 `generic.xaml`/`themeresources.xaml`（26100 与 22621）与
/// `C:\Windows\System32\Windows.UI.Xaml.dll` 全部 0 命中；它们只存在于 WinUI 的 resources.pri
/// （Normal 250 / Fast 167 / FastAnimationAfter 168 / Faster 83ms，无 Slow）。
/// 因此这些键**不可在 UWP 风格 XAML 里引用**，数值本身正确。
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

/// Expander 展开时长。对齐 UWP ExpandDown/Up 333ms。参 CONTROL_SPEC §13。
pub const DURATION_EXPANDER_EXPAND: f64 = 0.333;
/// Expander 收起时长。对齐 UWP Collapse 167ms。参 CONTROL_SPEC §13。
pub const DURATION_EXPANDER_COLLAPSE: f64 = 0.167;
/// Expander chevron 旋转时长。UWP 0.1s（Checked DoubleAnimation）。参 CONTROL_SPEC §13。
pub const DURATION_EXPANDER_CHEVRON: f64 = 0.1;

// ── 弹层轨道：换向入口 + 可换向曲线（RM-1028-5 缺陷 1/2 的修法载体） ──────────

/// 弹层轨道曲线。正典 Bézier（sheet_*，§200 / SDK 26100 generic.xaml CommandBarFlyout
/// 模板）与 Sokuou UWP 缓动变体（overlay_*）两态；经 `Easing::Bezier` / `Easing::Uwp`
/// 交给 `Progress` 求值（求值器已回流 `sokuou::bezier_y`，Uwp 转发 `apply_uwp`
/// 与旧 `MetroAnim` 路径逐位一致）。本枚举只描述曲线，不持有推进状态。
#[derive(Debug, Clone, PartialEq)]
pub enum TrackCurve {
    /// cubic-bezier(x1,y1,x2,y2)。
    Bezier([f32; 4]),
    /// UWP 缓动变体 + 方向。
    Uwp(UwpEasing, EasingMode),
}

impl From<TrackCurve> for Easing {
    fn from(curve: TrackCurve) -> Self {
        match curve {
            TrackCurve::Bezier(c) => Easing::Bezier(c),
            TrackCurve::Uwp(v, m) => Easing::Uwp(v, m),
        }
    }
}

/// 弹层开合轨道：换向只 `retune_to`（时长 + 曲线）+ `set_target`，
/// 从当前呈现值接续、可中断（参 compositor/docs/AnimationRules.md）。
///
/// 时间推进**唯一**由 Sokuou `Progress` 承担（SOKUOU §Ⅲ：progress 唯一真源）；
/// 本结构不再复制 value/target/from/elapsed/duration 等推进状态，
/// 只是预设层的换向入口：曲线两态（正典 Bézier / UWP 变体）经
/// [`TrackCurve`] → `sokuou::Easing`（`Easing::Bezier` / `Easing::Uwp`）交给 Progress。
#[derive(Debug, Clone, PartialEq)]
pub struct FlyoutTrack {
    progress: Progress,
}

impl FlyoutTrack {
    pub fn new(duration: f64, curve: TrackCurve) -> Self {
        Self {
            progress: Progress::with_easing(duration, curve.into()),
        }
    }

    /// 按预设原型换时长与曲线，不动进度状态。**换向前调用，随后应 `set_target`**：
    /// 重新计段的 `elapsed` 归零由 `set_target` 承担，呈现值从当前值接续（不重建对象，
    /// 修复「每次 open/close 重建轨道 → 从 0 重跑」，参 RM-1028-5 缺陷 1）。
    pub fn retune_to(&mut self, preset: &FlyoutTrack) {
        self.progress.set_duration(preset.progress.duration());
        self.progress.set_easing(preset.progress.easing());
    }

    /// 设定新目标，从当前 value 开始过渡（可中断）。语义同 Sokuou `Progress::set_target`。
    pub fn set_target(&mut self, target: f64) {
        self.progress.set_target(target);
    }

    /// 立即跳到指定值，无过渡。
    pub fn jump_to(&mut self, value: f64) {
        self.progress.jump_to(value);
    }

    /// 每帧推进。`dt` 单位为秒。推进唯一走 Sokuou `Progress::update`，不另立状态机。
    pub fn update(&mut self, dt: f64) {
        self.progress.update(dt);
    }

    pub fn value(&self) -> f64 {
        self.progress.value()
    }

    pub fn target(&self) -> f64 {
        self.progress.target()
    }

    /// 已到达目标时为 true。
    pub fn is_steady(&self) -> bool {
        self.progress.is_steady()
    }
}

/// Kanesumi 动画预设。参 PLAN.md §4-1/§4-2 动画三层：
///
/// - **主运动**（位移 / 缩放 / 面板滑入）→ `SpringAnim` 解析解弹簧，帧率无关、可中断。
/// - **次要属性**（透明度 / 颜色）→ `MetroAnim` 时长驱动 + UWP 缓动。
///
/// 弹簧参数来源：SOKUOU_ENGINE.md §6.3 Apple 风格典型值表；
/// 时长 / 曲线来源：真实 UWP（Metro 时代）组件观测聚类。
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

    /// 面板入场：300ms 正典开曲线 cubic-bezier(0.1,0.9,0.2,1)（§200；SDK 26100
    /// `generic.xaml` CommandBarFlyout 模板 OpeningStoryboard）。旧值 Cubic/EaseOut
    /// 与声明不符（RM-1028-5 缺陷 2），已改 Bézier；时长不改。
    pub fn sheet_appear() -> FlyoutTrack {
        FlyoutTrack::new(
            DURATION_SHEET_APPEAR,
            TrackCurve::Bezier(crate::easings::CURVE_UWP_OPEN),
        )
    }

    /// 面板收起：150ms 正典关曲线 cubic-bezier(0.7,0,1,0.5)（§200；同模板
    /// ClosingStoryboard）。旧值 Quadratic/EaseOut 与声明不符，已改 Bézier；时长不改。
    pub fn sheet_dismiss() -> FlyoutTrack {
        FlyoutTrack::new(
            DURATION_SHEET_DISMISS,
            TrackCurve::Bezier(crate::easings::CURVE_UWP_CLOSE),
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
    /// 返回 [`FlyoutTrack`] 使 PopupAnim 两条轨道都能不重建换向；曲线逐位同旧 MetroAnim。
    pub fn overlay_open() -> FlyoutTrack {
        FlyoutTrack::new(
            DURATION_OVERLAY_OPEN,
            TrackCurve::Uwp(UwpEasing::Cubic, EasingMode::EaseOut),
        )
    }

    /// 下拉面板遮罩淡出：216ms Quadratic/EaseOut（对齐 UWP OverlayClosingAnimation，§8）。
    pub fn overlay_close() -> FlyoutTrack {
        FlyoutTrack::new(
            DURATION_OVERLAY_CLOSE,
            TrackCurve::Uwp(UwpEasing::Quadratic, EasingMode::EaseOut),
        )
    }

    /// 对话框缩放：500ms Cubic/EaseOut（对齐 UWP spline 0.1,0.9,0.2,1，§9）。
    /// 淡入/淡出另用 `DURATION_DIALOG_FADE_IN/OUT` 常量 + 线性近似。
    pub fn dialog_scale() -> MetroAnim {
        MetroAnim::new(DURATION_DIALOG_ENTER, UwpEasing::Cubic, EasingMode::EaseOut)
    }

    /// Expander 展开：333ms Cubic/EaseOut（对齐 UWP ExpandDown/Up spline (0,0,0,1)，CONTROL_SPEC §13）。
    pub fn expander_expand() -> MetroAnim {
        MetroAnim::new(
            DURATION_EXPANDER_EXPAND,
            UwpEasing::Cubic,
            EasingMode::EaseOut,
        )
    }

    /// Expander 收起：167ms Cubic/EaseOut（对齐 UWP CollapseUp/Down spline (1,1,0,1)，§13）。
    pub fn expander_collapse() -> MetroAnim {
        MetroAnim::new(
            DURATION_EXPANDER_COLLAPSE,
            UwpEasing::Cubic,
            EasingMode::EaseOut,
        )
    }

    /// Expander chevron 旋转：100ms（对齐 UWP Checked DoubleAnimation Duration 0:0:0.1，§13）。
    pub fn expander_chevron() -> MetroAnim {
        MetroAnim::new(
            DURATION_EXPANDER_CHEVRON,
            UwpEasing::Quadratic,
            EasingMode::EaseOut,
        )
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

    /// 弹窗开合时长与一手源逐字对齐：开 300ms / 关 150ms。
    #[test]
    fn sheet_open_close_match_commandbarflyout_storyboards() {
        assert_eq!(DURATION_SHEET_APPEAR, 0.30);
        assert_eq!(DURATION_SHEET_DISMISS, 0.15);
        assert!(
            (DURATION_SHEET_APPEAR / DURATION_SHEET_DISMISS - 2.0).abs() < 1e-9,
            "UWP 收起恰为展开的一半时长"
        );
    }

    #[test]
    fn toggle_flip_matches_uwp_reposition() {
        // 参 CONTROL_SPEC §10：UWP RepositionThemeAnimation 0.15s；不确定进度循环 2.0s
        assert_eq!(DURATION_TOGGLE_FLIP, 0.15);
        assert_eq!(DURATION_INDETERMINATE, 2.0);
    }

    /// RM-1028-5 缺陷 2 回归：`sheet_appear` 进度必须等于正典
    /// cubic-bezier(0.1,0.9,0.2,1)（§200；SDK 26100 generic.xaml CommandBarFlyout 模板），
    /// 不再是 1−(1−t)^3。容差 1e-3（参任务书 k-framework-fix1 §2-1）。
    #[test]
    fn sheet_appear_matches_canon_bezier() {
        let mut a = MetroPresets::sheet_appear();
        for &t in &[0.25_f32, 0.5, 0.75] {
            a.jump_to(0.0);
            a.set_target(1.0);
            a.update(t as f64 * DURATION_SHEET_APPEAR);
            let expect = crate::easings::bezier_y(crate::easings::CURVE_UWP_OPEN, t) as f64;
            assert!(
                (a.value() - expect).abs() <= 1e-3,
                "sheet_appear t={t}: 实际 {}，正典 Bézier {expect}",
                a.value()
            );
        }
    }

    /// RM-1028-5 缺陷 2 回归：`sheet_dismiss` 进度必须等于正典
    /// cubic-bezier(0.7,0,1,0.5)，不再是 1−(1−t)^2。
    #[test]
    fn sheet_dismiss_matches_canon_bezier() {
        let mut a = MetroPresets::sheet_dismiss();
        for &t in &[0.25_f32, 0.5, 0.75] {
            a.jump_to(0.0);
            a.set_target(1.0);
            a.update(t as f64 * DURATION_SHEET_DISMISS);
            let expect = crate::easings::bezier_y(crate::easings::CURVE_UWP_CLOSE, t) as f64;
            assert!(
                (a.value() - expect).abs() <= 1e-3,
                "sheet_dismiss t={t}: 实际 {}，正典 Bézier {expect}",
                a.value()
            );
        }
    }
}
