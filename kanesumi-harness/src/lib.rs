// Kanesumi（矩隅）· 应用壳（harness）
//
// 把 Kanesumi 从组件库变成应用 SDK：进程入口、ETHER_ROLE 角色解析、App trait、
// 场景描述（Scene），以及 Linux 下的 Wayland+wgpu 外壳。
// 参 Ether-main PLAN.md §4.2（三层握手）/ §4.3（角色模型，harness 归属决策 2026-08-10）。

pub mod app;
pub mod appmenu;
pub mod context_menu;
pub mod frame_clock;
pub mod idle;
pub mod input_config;
pub mod key_repeat;
pub mod layers;
pub mod perf;
pub mod renderer_policy;
pub mod role;
pub mod system_theme;
pub mod timeline;
// ── K4 旧应用模型（`App`）的豁免（参 docs/PIPELINE_PLAN.md §Ⅳ-K4）─────────────
//
// `App` 自 K4 起标记弃用；本 crate 是它的唯一持有者：外壳（`platform` / `session_lock`）、
// 快照工具与 `TreeHost` 都必须按它驱动应用。故在此按模块豁免弃用告警 —— 这些模块是
// 旧模型的边界实现，新应用一律用 `TreeApp`。
#[allow(deprecated)]
pub mod tree_host;
pub mod wake;

#[cfg(target_os = "linux")]
pub mod canvas_v2;
pub use kanesumi_render::cpu_raster;
#[cfg(target_os = "linux")]
pub(crate) use kanesumi_render::glyph_layout;

/// 文字浓度实验旋钮（tx1 spike）。经 `CpuRenderer::set_text_tuning` 传入。
pub use kanesumi_render::TextRenderTuning;

#[cfg(target_os = "linux")]
pub mod dmabuf;

// dmabuf 探测子进程入口：应用 `main` 第一行调用（若 `ETHER_DMABUF_PROBE` 存在则跑探测并
// exit，否则立即返回）。前移入口避免子进程重执行时先跑应用前半段而误判段错误来源。
// 参 docs/STATE_2026-10-02.md §Ⅳ-8（g1a 诊断改造）。
#[cfg(target_os = "linux")]
pub use dmabuf::dmabuf_probe_entry;

#[cfg(target_os = "linux")]
#[allow(deprecated)]
pub mod platform;

#[cfg(target_os = "linux")]
#[allow(deprecated)]
pub mod session_lock;

#[cfg(target_os = "linux")]
pub mod render;

// 快照工具按外壳契约（`App`）驱动应用，K4 起显式豁免弃用告警。
#[allow(deprecated)]
pub mod snapshot;

pub use cpu_raster::CpuRenderer;

// 旧 `App` 的再导出保留（外壳与测试宿主需要它在本作用域内），弃用标注随定义生效。
#[allow(deprecated)]
pub use app::{
    AnchorKind, App, AppConfig, FloatingLayer, ImeAction, ImeContentHint, ImeContext, InputEvent,
    Key, LayerKind, LockEvent, Modifiers, PendingImeBatch, PointerButton, PopupRequest,
    ScrollInput, ScrollPhase, ScrollSource, compute_ime_action,
};
#[cfg(target_os = "linux")]
pub use appmenu::install;
pub use appmenu::{AppMenuHandle, MenuItem, MenuTree, MenuUpdate, ToggleType};
pub use context_menu::{ContextMenuAction, ContextMenuState};
pub use frame_clock::{FrameClock, InputCoalescer, MonoBase, PacingThrottle, monotonic_now};
pub use key_repeat::{KeyRepeat, RepeatConfig, key_id};
pub use renderer_policy::{RendererKind, SurfaceClass, choose_renderer};
pub use role::{EtherRole, RoleParseError, SurfaceKind};
pub use tree_host::{TreeApp, TreeHost};

// 元素树（参 docs/ELEMENT_TREE.md）。重导出，元素树应用只需依赖 harness。
pub use kanesumi_element as element;

// Scene 属 kanesumi-canvas（渲染命令层）。此处重导出供应用使用。
pub use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
