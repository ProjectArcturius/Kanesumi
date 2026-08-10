// Kanesumi Gallery —— 三层测试阶梯的 daily driver（参 Ether-main PLAN.md §4.4）。
//
// 骨架阶段：纯逻辑页树模型（跨平台），Wayland+wgpu 外壳 Phase 3 接入。
// 对照 WinUI-Gallery：每页 = 控件展示 + 交互示例。
// 三层阶梯：日常开发（Plasma）→ 集成（Ether 嵌套 winit）→ 验证（TTY/DRM）。

pub mod pages;

pub use pages::{GalleryPage, PaletteEntry, page_tree, palette};
