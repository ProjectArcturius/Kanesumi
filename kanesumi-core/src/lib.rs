// Kanesumi（矩隅）· Ether 扇区核心
//
// 声明式控件树 + 设计 tokens / 主题 / MetroText / 交互指示。
// 参 Ether-main PLAN.md §4（Runtime 架构）与 §6（命名规划）。

pub mod color;
pub mod colors;
pub mod geometry;
pub mod indicator;
pub mod scene;
pub mod text;
pub mod theme;
pub mod tokens;
pub mod typography;

pub use color::Color;
pub use colors::MetroColors;
pub use geometry::{Point, Rect, Size};
pub use indicator::MetroIndication;
pub use scene::{Scene, SceneCommand, TextAlign};
pub use text::{Line, TextEngine, TextLoadError};
pub use theme::MetroTheme;
pub use tokens::{Spacing, Tokens};
pub use typography::{FontWeight, MetroTypography, TextStyle};
