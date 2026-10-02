// widgets —— 框架内置的基础元素（容器、文本与图像）。参 ELEMENT_TREE §Ⅹ E1。
//
// 放「任何界面都离不开」的基础件：`Stack`（行 / 列）、`Border`（背景 / 描边 / 内边距）、
// `Label`（文本）、`Image`（位图 / SVG 图标），以及框架 chrome `Tooltip`（提示气泡 ——
// 由 `Tree` 的提示计时自动挂载，应用经 `Tree::set_tooltip` 使用，不自行插入）。
// Kanesumi 风格的控件在 kanesumi-controls 里实现。

mod border;
mod image;
mod label;
mod stack;
mod tooltip;

pub use border::Border;
pub use image::{Image, ImageSource, Stretch, fitted_rect};
pub use label::Label;
pub use stack::{Axis, Stack};
pub use tooltip::{MAX_WIDTH as TOOLTIP_MAX_WIDTH, TOOLTIP_GAP, Tooltip, tooltip_style};
