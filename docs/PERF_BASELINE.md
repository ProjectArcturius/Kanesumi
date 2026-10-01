# Kanesumi 运行时性能基准（k-perf）

> 任务 k-perf：先量后改。`kanesumi-harness/examples/frame_bench.rs` 平台无关，
> 每场景测「首帧 / 悬停移动 200 次 / 滚轮 100 次」，每次 = `Tree::frame` + `CpuRenderer::render`。
> 单位毫秒。运行：`cargo run --release -p kanesumi-harness --example frame_bench`。
>
> 场景 A「长列表」= `MetroScrollView` + 2000 行（图标 `MetroButton` + 两行 `Label`）。
> 场景 B「设置页」= `MetroNavigationView` + 一页 40 个控件（Switch/Slider/Button/TextBox）。
> 视口 1280×800。

## 改前（baseline，2026-10-01，Windows x86_64 release，字体 msyh.ttc）

| 场景 | 阶段 | tree p50 | tree p95 | tree max | raster p50 | raster p95 | raster max | 命令 min/avg/max |
|---|---|---|---|---|---|---|---|---|
| long_list | 首帧 | 37.81 | — | — | 1.34 | — | — | 16007 |
| long_list | 悬停200 | 1.19 | 1.71 | 2.49 | 0.61 | 0.72 | 0.90 | 16008/16008/16008 |
| long_list | 滚轮100 | 4.29 | 6.47 | 7.25 | 0.97 | 1.43 | 1.84 | 16008/16008/16008 |
| settings | 首帧 | 0.29 | — | — | 7.80 | — | — | 153 |
| settings | 悬停200 | 0.01 | 0.02 | 0.05 | 6.57 | 7.07 | 9.86 | 153/153/154 |
| settings | 滚轮100 | 0.01 | 0.01 | 0.05 | 6.55 | 7.10 | 7.79 | 153/153/153 |

> 热点观察：长列表每帧 `compose` 把全部 16008 条命令 clone 进 Scene（悬停/滚动亦然），
> 故 tree 耗时不随 damage 缩小；这正是第 2 步「compose 剔除」的目标。

## 改后（after）

（待补：第 2~4 步完成后追加，与此表并列。）
