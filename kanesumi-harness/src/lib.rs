// Kanesumi（矩隅）· 应用壳（harness）
//
// 把 Kanesumi 从组件库变成应用 SDK：进程入口、ETHER_ROLE 角色解析、App trait、
// 场景描述（Scene），以及 Linux 下的 Wayland+wgpu 外壳。
// 参 Ether-main PLAN.md §4.2（三层握手）/ §4.3（角色模型，harness 归属决策 2026-08-10）。

pub mod app;
pub mod role;

#[cfg(target_os = "linux")]
pub mod platform;

#[cfg(target_os = "linux")]
pub mod render;

pub use app::{App, AppConfig, InputEvent, PointerButton};
pub use role::{EtherRole, RoleParseError, SurfaceKind};

// Scene 属 kanesumi-canvas（渲染命令层）。此处重导出供应用使用。
pub use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
