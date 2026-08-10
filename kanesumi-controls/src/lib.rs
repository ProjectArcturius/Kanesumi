// Kanesumi（矩隅）· Metro 标准控件库
//
// 对应 Kanesumi-sec-a 的 `:kanesumi-controls`。Phase 3 首套控件：Text / Button / List / Surface。
// 状态驱动渲染：控件持有状态，`render(theme, engine, rect, scene)` 把当前状态解析为 Scene 命令。
// 逐一对照 microsoft-ui-xaml 控件实现与 WinUI-Gallery 交互（参 PLAN.md §5 Phase 3）。

pub mod button;
pub mod list;
pub mod surface;
pub mod text;

pub use button::{ButtonKind, ButtonState, MetroButton};
pub use list::MetroList;
pub use surface::MetroSurface;
pub use text::MetroText;

/// Phase 3 后续控件清单（对照 Kanesumi-sec-a 同名控件 + WinUI-Gallery）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    MetroSurface,
    MetroButton,
    MetroIconButton,
    MetroListRow,
    MetroSwitch,
    MetroProgressIndicator,
    MetroTabRow,
    MetroDialog,
    MetroDropdownMenu,
    MetroSelectorFlyout,
    MetroDivider,
    MetroBottomSheet,
}

impl ControlKind {
    /// 已实现 + 计划控件。
    pub const ALL: [ControlKind; 12] = [
        ControlKind::MetroSurface,
        ControlKind::MetroButton,
        ControlKind::MetroIconButton,
        ControlKind::MetroListRow,
        ControlKind::MetroSwitch,
        ControlKind::MetroProgressIndicator,
        ControlKind::MetroTabRow,
        ControlKind::MetroDialog,
        ControlKind::MetroDropdownMenu,
        ControlKind::MetroSelectorFlyout,
        ControlKind::MetroDivider,
        ControlKind::MetroBottomSheet,
    ];
}
