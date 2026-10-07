// Kanesumi（矩隅）· 应用壳（harness）
//
// 把 Kanesumi 从组件库变成应用 SDK：进程入口、ETHER_ROLE 角色解析、App trait、
// 场景描述（Scene），以及 Linux 下的 Wayland+wgpu 外壳。
// 参 Ether-main PLAN.md §4.2（三层握手）/ §4.3（角色模型，harness 归属决策 2026-08-10）。

pub mod app;
pub mod appmenu;
pub mod context_menu;
pub mod idle;
pub mod input_config;
pub mod layers;
pub mod perf;
pub mod renderer_policy;
pub mod role;
pub mod system_theme;
pub mod timeline;
pub mod tree_host;

pub mod cpu_raster;
pub(crate) mod glyph_layout;

/// 文字浓度实验旋钮（tx1 spike）。经 `CpuRenderer::set_text_tuning` 传入。
pub use glyph_layout::TextRenderTuning;

#[cfg(target_os = "linux")]
pub mod dmabuf;

// dmabuf 探测子进程入口：应用 `main` 第一行调用（若 `ETHER_DMABUF_PROBE` 存在则跑探测并
// exit，否则立即返回）。前移入口避免子进程重执行时先跑应用前半段而误判段错误来源。
// 参 docs/STATE_2026-10-02.md §Ⅳ-8（g1a 诊断改造）。
#[cfg(target_os = "linux")]
pub use dmabuf::dmabuf_probe_entry;

#[cfg(target_os = "linux")]
pub mod platform;

#[cfg(target_os = "linux")]
pub mod render;

pub mod snapshot;

pub use cpu_raster::CpuRenderer;

pub use app::{
    AnchorKind, App, AppConfig, FloatingLayer, ImeAction, ImeContentHint, ImeContext, InputEvent,
    Key, LayerKind, Modifiers, PendingImeBatch, PointerButton, PopupRequest, ScrollInput,
    ScrollPhase, ScrollSource, compute_ime_action,
};
#[cfg(target_os = "linux")]
pub use appmenu::install;
pub use appmenu::{AppMenuHandle, MenuItem, MenuTree, MenuUpdate, ToggleType};
pub use context_menu::{ContextMenuAction, ContextMenuState};
pub use renderer_policy::{RendererKind, SurfaceClass, choose_renderer};
pub use role::{EtherRole, RoleParseError, SurfaceKind};
pub use tree_host::{TreeApp, TreeHost};

// 元素树（参 docs/ELEMENT_TREE.md）。重导出，元素树应用只需依赖 harness。
pub use kanesumi_element as element;

// Scene 属 kanesumi-canvas（渲染命令层）。此处重导出供应用使用。
pub use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
