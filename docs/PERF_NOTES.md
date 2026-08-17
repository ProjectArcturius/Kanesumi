# Kanesumi 性能/稳健度移植笔记（2026-08-17）

> 记录从参考仓库（`reference/`，不入版本控制）学到的机制，以及移植到 Kanesumi 的落地。
> 铁律：**只改善实际效果（性能/稳健度），不修改概念**（Scene 命令模型、颜色契约、
> MSAA 语义、App trait 均不动）。参 `ETHER_RENDER_LESSONS.md`、`TOPBAR_RENDER_REFACTOR.md`。

## 一、参考仓库清单（2026-08-17 克隆，depth-1）

| 仓库 | 领域 | 许可 | 移植方向 |
|---|---|---|---|
| linebender/tiny-skia | CPU 光栅（SkRasterPipeline） | BSD-3 | 边缘 AA / RLE blit / 双精度 pipeline |
| dfrg/swash | 塑形 + 字形光栅 | Apache-2.0 | 有界缓存（FontCache） |
| pop-os/cosmic-text | 多行文本布局 | Apache-2.0 | 排版缓存策略 |
| linebender/parley | 富文本布局 | Apache-2.0 | 排版缓存策略 |
| googlefonts/fontations | 字体读/写（skrifa） | Apache-2.0 | 字形缓存键设计 |
| RazrFalcon/fontdb | 字体回退 DB | MIT | fallback 评分替代线性扫描 |
| moka-rs/moka | 并发缓存 | Apache-2.0 | 有界 LRU 语义 |
| femtovg/femtovg | AA 矢量绘制 | MIT/Apache | 边缘 AA |
| linebender/vello | GPU compute 2D | Apache-2.0 | 未来 wgpu 路径 |
| linebender/kurbo | 曲线/几何 | Apache-2.0 | 几何三角化 |

> `reference/microsoft-ui-xaml`（已有）为控件移植数据源，与本笔记无关。

## 二、已移植（本轮，2026-08-17）

### P1 — 排版 galley 缓存（源：egui `GalleyCache`）

`layout_box` 改为按完整排版输入缓存 `Arc<TextLayout>`。静态文本（时钟/应用名/菜单项）
每帧重复 UAX #14 换行 + 逐段测量是仅次于塑形/光栅化的 CPU 大头；命中零数据拷贝
（仅增 Arc 引用计数）。`kanesumi-canvas/src/text.rs`：`LayoutKey` + `layout_cache`。

### P2 — 图标缓存去每帧全量克隆（源：egui texture atlas）

`CpuRenderer.images` 值类型 `(Vec<u8>,..)` → `(Arc<[u8]>,..)`。旧 `or_insert_with(||
rgba.to_vec()).clone()` 每帧整张拷一遍，缓存形同虚设；现命中仅增引用计数。
`kanesumi-harness/src/cpu_raster.rs`。

### P3 — 有界缓存（源：swash `FontCache`）

`shape_cache` / `layout_cache` 加容量上限（4096），超出整体清空。纯加速缓存，命中与
未命中结果等价，清空零正确性风险；只约束长会话内存单调增长（时钟每分钟、应用名/
菜单项均产生新 key）。`kanesumi-canvas/src/text.rs`：`SHAPE_CACHE_MAX`/`LAYOUT_CACHE_MAX`。

## 三、待评估（未移植，需 A/B 或概念决策）

| 项 | 源 | 收益 | 风险 / 未做原因 |
|---|---|---|---|
| 边缘解析 AA 取代 4× 超采样 | tiny-skia scan/ | `fill_triangles` 最大热点 | 覆盖率语义从 4×MSAA 变 scanline，视觉保真需截图 A/B；触颜色契约 |
| RLE run-length blit | tiny-skia blitter.rs | `blend_px` 逐像素 powf | 同上，需保真验证 |
| 快速哈希器 | nohash/rustc-hash | 热路径查表省 SipHash | 需新增直接依赖；ShapeKey/LayoutKey 含 String 仍需真哈希，收益打折 |
| 字体回退评分 | fontdb query() | 替代 `font_for_grapheme` 线性扫描 | 需引入字体 DB 概念，属结构性改动 |
| 有界 LRU（epoch 淘汰） | swash FontCache | 比「整体清空」更平滑 | 整体清空已够用；LRU 增加复杂度 |

## 四、验证

- `cargo test`（kanesumi 全仓 659 通过）
- `cargo check -p ether-settings -p ether-launcher`（monorepo 消费方通过）
- clippy 无新增告警
