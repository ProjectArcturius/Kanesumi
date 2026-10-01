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

## 改后（after，2026-10-01，同机同字体 release）

| 场景 | 阶段 | tree p50 | tree p95 | tree max | raster p50 | raster p95 | raster max | 命令 min/avg/max |
|---|---|---|---|---|---|---|---|---|
| long_list | 首帧 | 36.87 | — | — | 1.37 | — | — | 143 |
| long_list | 悬停200 | 0.02 | 0.02 | 0.03 | 0.11 | 0.32 | 0.37 | 12/21/76 |
| long_list | 滚轮100 | 3.01 | 3.47 | 4.30 | 0.89 | 1.11 | 1.39 | 143/146/151 |
| settings | 首帧 | 0.41 | — | — | 7.72 | — | — | 153 |
| settings | 悬停200 | 0.02 | 0.02 | 0.04 | 6.90 | 9.53 | 11.02 | 153/153/154 |
| settings | 滚轮100 | 0.01 | 0.02 | 0.02 | 6.78 | 8.05 | 9.47 | 153/153/153 |

### 对照（改前 → 改后，tree p50）

| 场景 / 阶段 | 改前 | 改后 | 降幅 | 说明 |
|---|---|---|---|---|
| long_list 首帧 | 37.81 | 36.87 | ~2% | 首帧全量绘制仍 O(内容)；仅拼接阶段剔除视口外（命令 16007→143） |
| long_list 悬停200 | 1.19 | 0.02 | **~60×** | compose 按 damage 剔除 + 脏集合定向重画（命令 16008→12~76） |
| long_list 滚轮100 | 4.29 | 3.01 | ~30% | 每帧 2000 行重排/重画的固有成本；拼接与画阶段已剔除视口外 |
| settings 悬停200 | 0.01 | 0.02 | — | 页面小，原就快；主要成本在 raster（控件 AA） |
| settings raster | 6.57 | 6.90 | — | 未触及光栅内核，属噪声范围 |

### 逐像素快照回归

`theme_sheet`（dark + light）与 `calendar_sheet`（dark）三张 PNG，改前（HEAD 版
compose + 旧快照）与改后逐像素一致（`cmp` 字节相等；另经解码比对 0 差异）。
`tree_demo` 的 `--snapshot` 入口在 Linux 下 `#[cfg(target_os = "linux")]`，本机
（Windows）无法运行，故以其余三张为准。

## 备注

- 第 4 步「命令克隆」未做：`SceneCommand::Text.content` 为 `String`，改成 `Arc<str>`
  需改 `kanesumi-canvas` 的 Scene 类型并被 `kanesumi-controls`（大量测试按 `content`
  比较 / clone 成 `String`）消费，超出任务「允许改动」范围；且第 2 步剔除后单帧
  命令数已从 16008 降到 12~151，逐条克隆不再是热点。详见最终报告。
- `measure` / `arrange` 仍是全树遍历（本任务只记录，未改）。
