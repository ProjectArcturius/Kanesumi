// IME 上下文已下沉到 kanesumi-element（元素树的焦点控件经 `Widget::ime` 产出它）。
// 此处重导出，保持 `kanesumi_controls::ime::*` 与 `crate::ime::*` 旧路径可用。
pub use kanesumi_element::ime::{ImeContentHint, ImeContext};
