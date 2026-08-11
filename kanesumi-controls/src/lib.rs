// Kanesumi（矩隅）· 标准控件库
//
// 对应 Kanesumi-sec-a 的 `:kanesumi-controls`。Phase 3 首套控件已完成（参 CONTROL_SPEC）：
// MetroText / MetroButton / MetroIconButton / MetroSwitch / MetroProgressBar / MetroProgressRing /
// MetroTabRow / MetroList / MetroSelectorFlyout / MetroDropdownMenu / MetroDialog / MetroSurface。
// 状态驱动渲染：控件持有状态，`render(theme, engine, rect, scene)` 把当前状态解析为 Scene 命令。

pub mod button;
pub mod decl;
pub mod dialog;
pub mod drop_down_button;
pub mod dropdown_menu;
pub mod expander;
pub mod icon_button;
pub mod info_badge;
pub mod info_bar;
pub mod list;
pub mod menu_bar;
pub mod metro_tile;
pub mod person_picture;
pub mod pips_pager;
pub mod popup;
pub mod progress;
pub mod retained;
pub mod selector_flyout;
pub mod state;
pub mod surface;
pub mod switch;
pub mod tab_row;
pub mod text;

pub use button::{ButtonKind, MetroButton};
pub use decl::{
    Decl, DeclAction, DeclChange, DeclHit, DeclPath, collect_hits, diff_decl, render_decl,
};
pub use dialog::{DialogButton, DialogButtons, DialogDefaultButton, DialogState, MetroDialog};
pub use drop_down_button::MetroDropDownButton;
pub use dropdown_menu::{MenuItem, MetroDropdownMenu};
pub use expander::{ExpandDirection, MetroExpander};
pub use icon_button::MetroIconButton;
pub use info_badge::{InfoBadgeKind, MetroInfoBadge};
pub use info_bar::{InfoBarClick, InfoBarSeverity, MetroInfoBar};
pub use list::MetroList;
pub use menu_bar::{MenuBarItem, MetroMenuBar};
pub use metro_tile::{MetroTile, TileLive, TileSize};
pub use person_picture::{MetroPersonPicture, initials_from_display_name};
pub use pips_pager::{MetroPipsPager, PipsAction, PipsOrientation};
pub use popup::{
    POPUP_GAP, PopupAnim, PopupDirection, PopupPlacement, PopupState, place_popup, popup_gap,
    render_overlay,
};
pub use progress::{MetroProgressBar, MetroProgressRing, ProgressMode};
pub use retained::RetainedScene;
pub use selector_flyout::MetroSelectorFlyout;
pub use state::{ButtonState, ControlState};
pub use surface::MetroSurface;
pub use switch::MetroSwitch;
pub use tab_row::{MetroTab, MetroTabRow};
pub use text::MetroText;

/// Phase 3 后续控件（对照 Kanesumi-sec-a + WinUI-Gallery）。
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
