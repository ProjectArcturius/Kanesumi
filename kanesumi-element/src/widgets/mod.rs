// widgets —— 框架内置的基础元素（容器、文本与图像）。参 ELEMENT_TREE §Ⅹ E1。
//
// 只放「任何界面都离不开、且与设计语言无关」的四件：`Stack`（行 / 列）、`Border`
// （背景 / 描边 / 内边距）、`Label`（文本）、`Image`（位图 / SVG 图标）。
// Kanesumi 风格的控件在 kanesumi-controls 里实现。

mod border;
mod image;
mod label;
mod stack;

pub use border::Border;
pub use image::{Image, ImageSource, Stretch, fitted_rect};
pub use label::Label;
pub use stack::{Axis, Stack};
