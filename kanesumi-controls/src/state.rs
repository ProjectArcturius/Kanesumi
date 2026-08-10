/// 控件通用交互状态。参 CONTROL_SPEC §1 通用规律：
/// - 颜色切换为硬切换（Metro 无渐变）；禁用态靠前景/整体降透明度。
/// - `Focused` 非 Metro 状态（UWP 用系统焦点视觉），为 Kanesumi 自绘焦点环的适配。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlState {
    Normal,
    Hovered,
    Pressed,
    Focused,
    Disabled,
}

/// 兼容别名（旧名 ButtonState，Phase 3 统一为 ControlState）。
pub type ButtonState = ControlState;
