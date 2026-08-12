# Ether 客户端渲染教训（2026-08-12）

> 本次排查 Ether 合成器下外部客户端（layer-shell）渲染不可见的完整记录与教训。
> 血的教训：**不要瞎猜，用日志/证据定位**。

## 一、核心结论

**Ether 合成器（smithay GlesRenderer）对 layer-shell 表面的 wgpu dmabuf buffer 渲染不可见**（与 Known Issue #8 同源）。这是合成器侧问题，**不是客户端配置问题**。

验证矩阵：

| 客户端 | buffer 类型 | layer-shell 渲染 | 结果 |
|---|---|---|---|
| egui 版 TopBar（settings/topbar.rs） | SHM（wl_shm Argb8888） | `collect_layer_draws` | **可见** ✓ |
| librarian（eframe wgpu） | wgpu（xdg-shell 走 `render_ssd`） | render_ssd | **可见** ✓ |
| kanesumi TopBar（wgpu） | dmabuf | collect_layer_draws | 不可见 ✗ |
| launcher Dock（wgpu） | dmabuf | collect_layer_draws | 不可见 ✗（试遍 Vulkan/GL、sRGB/非sRGB、alpha_mode 全部配置） |

**结论**：SHM buffer 走 `collect_layer_draws → import_memory` 稳定可靠；wgpu dmabuf 走同一路径不可见。

## 二、可行方案（Ether 下客户端渲染）

**offscreen wgpu → readback → SHM 提交**（保留全部 egui 渲染逻辑，只改输出路径）：

1. wgpu 渲染到**离屏纹理**（不 present surface）
2. `copy_texture_to_buffer` + `map_async` → BGRA bytes
3. 写 **wl_shm buffer**（Argb8888，内存 = BGRA little-endian）
4. `wl_surface.attach + commit`

launcher 已按此实现（`render_dock_shm` + `commit_dock_shm`），Dock 可显示。

## 三、关键坑

### 1. wgpu offscreen texture 格式必须匹配 egui pipeline

egui_wgpu 的 pipeline 用 surface format（`Bgra8UnormSrgb`）。offscreen texture 若用 `Bgra8Unorm`：

```
wgpu error: Validation Error
  In RenderPass::end
    Render pipeline targets are incompatible with render pass
      the RenderPass uses textures with formats [Some(Bgra8Unorm)]
      but the RenderPipeline with 'egui_pipeline' uses [Some(Bgra8UnormSrgb)]
```

→ panic（wgpu 22 默认 error fatal）→ 进程崩溃。**offscreen texture 必须用 `Bgra8UnormSrgb`**。

### 2. readback 的 bytes_per_row 需 256 对齐

`copy_texture_to_buffer` 要求 `bytes_per_row` 是 256 的倍数，否则 Validation Error。读回后按每行 `width*4` 解包（跳过 padding）。

### 3. wgpu 22 默认把错误当 fatal

`Handling wgpu errors as fatal by default`——任何 wgpu 错误直接 panic。调试时看 `session.log` 的 panic 定位。

## 四、调试方法（必须记住）

Ether session 的日志在：**`~/.cache/ether-session/session.log`**（`ether-session` 脚本重定向全部 stderr）。

排查链路：
1. **子进程是否存活**：`pgrep -af ether-xxx`
2. **子进程崩溃原因**：`grep -iE "panicked|panic|wgpu|Adapter" ~/.cache/ether-session/session.log`
3. **layer surface 是否被合成器收集**：`cat ~/ether-layer-draws.log`（每行 `layer[ns] size anchor pos dims elements=N`，N=1 有 buffer，N=0 无 buffer）
4. **合成器诊断双写 /tmp + $HOME**（LightDM 多 session 的 /tmp 可能隔离，home 跨 session 共享）

## 五、本次时间线（2026-08-12）

1. kanesumi 版 TopBar（wgpu dmabuf）在 Ether 下不可见 → **恢复 egui 版（SHM）** → 可见
2. launcher Dock（wgpu dmabuf）不可见 → 试 Vulkan/GL、sRGB/非sRGB、alpha_mode、GL 复位时机 → 都不行
3. 确认：`collect_layer_draws` 渲染 SHM 可见、dmabuf 不可见（与 buffer 类型强相关）
4. 实施 **offscreen readback → SHM**（保留 egui 逻辑）→ 途中踩 Validation Error（format 不匹配）→ 修复 → **Dock 显示**
5. **教训**：前几轮在客户端 wgpu 配置上反复试（吃了大量 token），实际根因在合成器对 dmabuf 的渲染 + offscreen 格式匹配。

## 六、待办

- [ ] Dock 图标挤在一起（布局/几何）—— 待修
- [ ] Launcher overlay 同样走 offscreen readback → SHM
- [ ] kanesumi 版 Dock/TopBar 待合成器 dmabuf 渲染修复后启用（届时 kanesumi 的 wgpu 也可用）
