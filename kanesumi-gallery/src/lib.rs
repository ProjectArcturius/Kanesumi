// Kanesumi Gallery —— 三层测试阶梯的 daily driver（参 Ether-main PLAN.md §4.4）。
//
// 阶段：GalleryApp 实现 App trait（交互 + 动画），Linux 上由 harness 外壳驱动
// （kanesumi_harness::platform::run）。对照 WinUI-Gallery：每页 = 控件展示 + 交互示例。
// 三层阶梯：日常开发（Plasma）→ 集成（Ether 嵌套 winit）→ 验证（TTY/DRM）。

pub mod app;
pub mod demo;
pub mod pages;

pub use app::GalleryApp;
pub use demo::{command_summary, render_demo_scene};
pub use pages::{GalleryPage, PaletteEntry, page_tree, palette};
