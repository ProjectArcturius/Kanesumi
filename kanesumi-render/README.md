# kanesumi-render

Kanesumi（矩隅）· 平台无关渲染基座。

## 职责与边界

- **本 crate 职责（「画」在这里）**：
  - 提供 `CanvasV2`：基于 wgpu 的实例化批渲染 GPU 画布，解析式 SDF 抗锯齿，图集分配，排版缓存，常驻离屏目标与增量重绘。
  - 提供 `CpuRenderer`：纯 CPU 光栅器（回退与对照路径）。
  - 提供 `glyph_layout`：字形排版与文字量测纯逻辑。
  - 提供平台无关的 `GpuContext`、`RendererError`、几何求交与裁剪工具（`intersect`、`scissor_rect`、`damage_clip`）。
  - **绝不依赖** Wayland 协议库（`wayland-*`、`smithay-client-toolkit`）或任何特定平台窗口系统句柄。
- **与 `kanesumi-harness` 的边界（「上屏」在 harness）**：
  - `kanesumi-render` 负责把 `Scene` 场景命令渲染到 `wgpu::Surface` 或离屏目标 `wgpu::Texture`。
  - `kanesumi-harness` 负责操作系统集成：Wayland 表面连接与生命周期、输入分发、事件循环与帧调度。

## 离屏用法示例

```rust,no_run
use kanesumi_render::{CanvasV2, GpuContext};
use kanesumi_canvas::{Scene, text::TextEngine};
use kanesumi_core::{Color, Rect};

// 1. 初始化无头 GPU 上下文（无需窗口或表面）
let ctx = GpuContext::headless().expect("无可用 GPU 适配器");

// 2. 创建离屏画布（宽 800、高 600、逻辑缩放 1.0）
let mut canvas = CanvasV2::offscreen(ctx, 800.0, 600.0, 1.0)
    .expect("离屏画布创建失败");

// 3. 构建 Scene 并绘制
let mut scene = Scene::default();
scene.fill_rect(Color::from_rgb(40, 120, 200), Rect::new(50.0, 50.0, 200.0, 100.0));

let engine = TextEngine::default();
canvas.render(&engine, &scene);

// 4. 读回 RGBA8 物理像素缓冲
let rgba_pixels = canvas.read_back();
assert_eq!(rgba_pixels.len(), 800 * 600 * 4);
```
