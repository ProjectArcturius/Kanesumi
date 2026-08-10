// Kanesumi（矩隅）· Ether 扇区核心
//
// 声明式控件树 + 设计 tokens / 主题 / MetroText / 交互指示。
// 参 Ether-main PLAN.md §4（Runtime 架构）与 §6（命名规划）。
// 2026-08-10：Scene/TextEngine 已迁出至 `kanesumi-canvas`（渲染命令 + 排版），
// 本 crate 回归纯运行时（tokens/主题/排版/几何），参 PLAN.md §6.1。

pub mod color;
pub mod colors;
pub mod geometry;
pub mod indicator;
pub mod theme;
pub mod tokens;
pub mod typography;

pub use color::Color;
pub use colors::MetroColors;
pub use geometry::{CornerRadius, Point, Rect, Size};
pub use indicator::MetroIndication;
pub use theme::MetroTheme;
pub use tokens::{Spacing, Tokens};
pub use typography::{FontWeight, MetroTypography, TextStyle};
