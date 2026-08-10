// Linux 外壳：Wayland 客户端（sctk）+ wgpu 渲染循环。
//
// 对应 §4.2 三层握手的 Runtime 侧：普通 Wayland 客户端（xdg-shell / layer-shell），
// 动画由 frame callback 推进。Phase 3 在 Linux 上实现并验证（§4.4 三层测试阶梯）。
// 当前骨架仅固定入口签名。

use crate::app::App;

/// 启动 harness 主循环（Linux）。阻塞运行，不返回。
///
/// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
/// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(size)` → 光栅化 Scene。
pub fn run(_app: &mut dyn App) -> ! {
    unimplemented!("Phase 3: Wayland+wgpu 外壳，在 Linux 上实现并验证")
}
