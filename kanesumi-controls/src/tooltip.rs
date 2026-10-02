// MetroToolTip —— 工具提示（悬停 / 键盘焦点气泡）。参 CONTROL_SPEC «ToolTip»。
//
// 工具提示是**框架行为**，不是逐个控件自持的状态：应用经 `Tree::set_tooltip(id, "…")`
// 给任意元素挂一段文字，框架在指针静止 / 键盘焦点停留满延迟（`kanesumi_core::interaction`
// 的正典常量：初始 500 ms、再现 100 ms、5 s 消失，指针离开 / 按下即消失）后，在锚点下方
// （空间不足时上方）开一个 `PopupSpec::passthrough` 弹层显示。提示不抢焦点、不吃输入。
//
// 气泡本体是 `kanesumi-element` 的框架 chrome `Tooltip`（元素树自动挂载）；此处以控件库
// 命名 `MetroToolTip` 暴露，供文档、测试与需要直接构造面板的宿主使用。

pub use kanesumi_element::widgets::{
    TOOLTIP_GAP, TOOLTIP_MAX_WIDTH, Tooltip as MetroToolTip, tooltip_style,
};

/// 提示初始延迟（毫秒；正典 §Ⅲ，用户可在 `input.toml` 覆盖）。
pub const TOOLTIP_DELAY_MS: u64 = kanesumi_core::interaction::TOOLTIP_DELAY_MS;
/// 提示再现延迟（毫秒；已有提示时移到相邻目标）。
pub const TOOLTIP_RESHOW_MS: u64 = kanesumi_core::interaction::TOOLTIP_RESHOW_MS;
/// 提示自动消失时长（毫秒）。
pub const TOOLTIP_HIDE_MS: u64 = kanesumi_core::interaction::TOOLTIP_HIDE_MS;
