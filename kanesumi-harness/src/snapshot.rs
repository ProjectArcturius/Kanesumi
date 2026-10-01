// snapshot.rs —— 无窗口快照：把任意 `App` 的一帧经真实 CPU 光栅器渲染成 PNG。
//
// 用途：视觉核对与回归（字体光栅、布局、主题）而**不在任何桌面上开窗** —— 调度者 / 工人
// 可在 Arch 终端或 CI 里直接产出图片查看，不打扰正在使用该机器的人。
// 与线上路径同源：`App::update` → `App::render_into` → `CpuRenderer::render`（TopBar 等
// layer-shell 表面的线上光栅器就是它）。

use std::path::Path;

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::Size;

use crate::app::App;
use crate::cpu_raster::CpuRenderer;

/// 渲染 `frames` 帧（让动画 / 定时器走几步），把最后一帧写成 PNG。返回物理像素尺寸。
pub fn render_png(
    app: &mut dyn App,
    engine: &TextEngine,
    size: Size,
    scale: f32,
    frames: u32,
    out: &Path,
) -> Result<(u32, u32), String> {
    let mut scene = Scene::default();
    let mut cpu = CpuRenderer::new(size.width, size.height, scale);
    let mut rgba = Vec::new();
    // 逐帧按各帧 damage 增量光栅进同一缓冲：元素树 compose 会剔除与 damage 不相交的
    // 节点（k-perf），故不能只把最后一帧的局部 Scene 当全量重绘 —— 会丢静态内容。
    // 帧 1 全量、后续局部，累积得到最终完整图像（与线上「渲染后提交」语义同构）。
    for _ in 0..frames.max(1) {
        app.update(1.0 / 60.0);
        app.render_into(engine, size, &mut scene);
        let damage = app.damage_hint();
        rgba = cpu.render(engine, &scene, damage).to_vec();
    }
    let (w, h) = cpu.physical_size();
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        rgba,
        resvg::tiny_skia::IntSize::from_wh(w, h).ok_or("尺寸为零")?,
    )
    .ok_or("像素缓冲尺寸不符")?;
    pixmap.save_png(out).map_err(|e| e.to_string())?;
    Ok((w, h))
}
