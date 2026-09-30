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

/// 元素树的框架状态 → 控件既有的单值 `ControlState`（参 docs/ELEMENT_TREE.md §Ⅶ）。
///
/// 优先级：禁用 > 按下 > 悬停 > 常态。**不映射 `Focused`**：元素树下键盘焦点视觉由框架统一
/// 绘制（`Widget::focus_visual`），控件再画一遍就会出现双层焦点框。
pub fn control_state(s: kanesumi_element::ControlStates) -> ControlState {
    if s.disabled {
        ControlState::Disabled
    } else if s.pressed {
        ControlState::Pressed
    } else if s.hovered {
        ControlState::Hovered
    } else {
        ControlState::Normal
    }
}
