// 全局应用菜单 —— 独立 crate `kanesumi-appmenu` 的重导出。
//
// 实现（dbusmenu 服务 / org_kde_kwin_appmenu 绑定 / Registrar 兜底 / 运行时更新）
// 驻 kanesumi-appmenu，harness 仅重导出，供 `App` trait 与 `platform.rs` 使用。
// eframe 应用（不经 harness）直接依赖 kanesumi-appmenu。参 kanesumi-appmenu/src/lib.rs。

pub use kanesumi_appmenu::*;
