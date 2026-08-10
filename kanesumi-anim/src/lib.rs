// Kanesumi（矩隅）· 动画层
//
// 消费 Sokuou（即応エンジン），提供 Kanesumi 动画预设与命名缓动。
// 参 PLAN.md §2.1（Sokuou 为动画唯一真源）与 §4（动画三层：弹簧主运动 / 时长驱动次要属性 / 过渡组）。

pub mod easings;
pub mod presets;

pub use easings::{metro_cubic, metro_default, metro_out_quart, metro_quintic, metro_sine};
pub use presets::{
    DURATION_COLOR_TRANSITION, DURATION_COVER_FADE, DURATION_DIALOG_ENTER, DURATION_DIALOG_FADE_IN,
    DURATION_DIALOG_FADE_OUT, DURATION_INDETERMINATE, DURATION_OVERLAY_CLOSE,
    DURATION_OVERLAY_OPEN, DURATION_QUICK_SWITCH, DURATION_SHEET_APPEAR, DURATION_SHEET_DISMISS,
    DURATION_TOGGLE_FLIP, METRO_STANDARD_DURATION, MetroPresets,
};

// 动画原语直接重导出 Sokuou，调用方无需再引入 sokuou 依赖。
pub use sokuou::{EasingMode, MetroAnim, Progress, SpringAnim, UwpEasing};
