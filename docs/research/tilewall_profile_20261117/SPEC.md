# 磁贴墙提供方兼容规格（2026-11-17）

> 任务：`k-tilewall-profile`。基线 `99f7c944`；应用真值只读，C0 已合 Ether `4d7388355`。
> 本件只补提供方的布局、组头、4×2 尺寸；不做应用 C1，不改拖放结果 / 键盘默认行为。
> 缺口出处：Ether `docs/research/m1115_tilewall/PROVIDER_REVIEW.md`。

## 一 · 兼容入口与默认

- 旧入口 `TileWallInput` / `layout` / `MetroTileWall::set_groups` / `relayout` /
  `render` / `cycle_size` 的签名、缺省值与语义全部不变。
- 新入口必须显式启用：`MetroTileWall::set_compat_metrics(Some(metrics))`。
  未调用或传 `None` 时，控制器逐字沿用 `TileWallMetrics::BASE`。
- 不给 `TileWallInput`、`TileWallMetrics`、`TileWallGroupLayout` 增加字段；
  4×2 只用 `TileWallSize` 的新变体（枚举已 `#[non_exhaustive]`）。
- 旧 `render` 仍画 15/20、`base_weight()`、`on_surface`、标题框高 20 且贴组块顶；
  兼容组头走新入口，不暗改旧输出。

## 二 · 响应式度量

应用 `launcher/src/shell/tile_metrics.rs::metrics_for(avail_h, viewport_w)` 产出五个字段。
提供方用 `TileWallMetrics::from_provider(cell, gap, group_gap, title_band, margin_x)`
接收，不要求 launcher 在控件外加偏移或重写矩形。

| 提供方字段 | 落到 `TileWallMetrics` | 缺省 / 备注 |
|---|---|---|
| `cell` | `cell` | 提供方值 |
| `gap` | `gap` | 提供方值 |
| `group_gap` | `group_gap` | 提供方值 |
| `title_band` | `title_band` | 提供方值（C0 = 72，组块顶 → 首行磁贴） |
| `margin_x` | `margin_x` | 提供方值（C0 = 1280×120/1366 = 112.4451） |
| — | `columns` | 仍取 `TILE_COLUMNS` = 12 |
| — | `min_rows` | 仍取 `TILE_MIN_ROWS` = 6 |
| — | `bottom_band` | 仍取 `BASE.bottom_band` = 48 |

`from_provider` 只做字段搬运；不做 `metrics_for` 的缩放算术，避免把应用屏高策略复制进控件。
控制器在 `relayout` 时把当前度量传给 `TileWallInput.metrics`，几何层仍是唯一算术处。

### C0 独立真值

输入：两组 `Standard`（18 + 2），墙矩形 `0,0 1280×752`，提供方度量
`cell=70, gap=10, group_gap=70, title_band=72, margin_x=112.4451`。

| 项 | 期望（C0 / 原代码） |
|---|---|
| 首组首格 | `(112.4451, 201, 150, 150)` |
| 第二组首格 | `x = 1132.4451`（112.4451 + 950 + 70） |
| 首组标题带 | `y = 201 − 40 − 8 = 153`，高 40 |
| caption 文本框 | `y = 153 + (40 − 18)/2 = 164`，高 18，宽 950 |
| `(200,170)` | 落在首组 caption 文本框内，读到「常用」 |

测试里的期望值取自上表字面量，不由被测 `layout` 生成。

## 三 · 兼容组头

- 常量：`TILE_TITLE_H = 40`、`TILE_TITLE_GAP = 8`，对应应用 `TITLE_H` / `TITLE_GAP`。
- 标题带顶 = 组内最上磁贴顶 − `TILE_TITLE_H` − `TILE_TITLE_GAP`；
  无磁贴的组回落到组块顶（与应用 `title_top` 的 `unwrap_or` 一致）。
- 文本：`theme.typography.caption`、`theme.colors.on_surface_variant`、左对齐、单行省略；
  在 40 高标题带内按 caption 行高垂直居中。
- 提供按组入口 `MetroTileWall::render_group_title(theme, scene, group)` 与
  `compat_title_band` / `compat_title_rect`，供保留树局部损伤重绘。组名只来自模型，
  控件不新增任何硬编码文案；空标题不画。
- `render_compat` 画兼容组头 + 与旧 `render` 相同的磁贴底；旧 `render` 保持旧组头。

## 四 · 尺寸映射与 4×2

| 应用 `TileSize` | 墙尺寸 | 单元 |
|---|---|---|
| `Mini` | `TileWallSize::Small` | 1×1 |
| `Standard` | `TileWallSize::Large` | 2×2 |
| `Large` | `TileWallSize::ExtraLarge` | 4×2 |
| —（库内旧档） | `TileWallSize::Wide` | 2×1 |

- `Large` 仍是 2×2，含义不变；`Wide` 仍是 2×1，不改。
- `ExtraLarge` 用 4×2：宽 = `4×cell + 3×gap`（C0 度量 = 310），高 = `2×cell + gap` = 150。
- 放置按应用 `find_large`：宏单元（2×2）行优先，找横向相邻两个宏单元都空闲的位置；
  不占用非偶数列。
- 缺省循环仍是 `Small → Wide → Large → Small`；`ExtraLarge` 不进入循环，循环落到它时
  回到 `Small`。旧三档之间的顺序不变。

## 五 · 验收与边界

- 回归必须覆盖：旧默认输出未变、C0 数字、组头角色、4×2 排布、旧尺寸含义、
  窄屏与 2× 逻辑度量。
- 不验证 / 不实现：应用 C1、拖放结果类型、键盘逻辑、时间动画、GPU、自动从屏高
  推导度量、C0 图与转录更新。
- 已知边界：`title_band < TITLE_H + TITLE_GAP` 时标题带会伸到组块顶之上；本件按应用
  公式照算，不额外夹紧。

## 六 · 对照图

`dark_1x.png` / `dark_2x.png` / `light_1x.png` / `light_2x.png` 都是**同一 C0 逻辑布局**：
两组 18+2 枚 `Standard`（墙尺寸 `Large`）、墙矩形 `0,0 1280×752`、提供方度量
（`cell=70, gap=10, group_gap=70, title_band=72, margin_x=112.4451`）。2× 只放大光栅
（`CpuRenderer::new(1280, 752, 2.0)`），逻辑几何与 1× 相同；4×2 的排布由单元测试覆盖，
不在这四张 C0 图里，以免改动 C0 的可视范围。

生成器是临时 example（仓内不留调试入口，参 `PROVIDER_REVIEW.md` 同款做法），持久副本在
`~/.local/state/tilewall-profile-20261117/tilewall_profile_shots.rs`；重跑命令
`cargo run -p kanesumi-gallery --example tilewall_profile_shots`，本次重跑四张图与入库文件
**逐字节相同**（`cmp` 全等）。

本会话模型不接受图像输入，`read_image` 返回
`model "deepseek/deepseek-v4.1-flash-1017" does not declare image input`；因此只用像素断言
（尺寸、标题带墨迹包围盒、磁贴边 x 序列、深浅背景色）核对，**没有亲看**，人眼版式复核
留给调度者。像素断言脚本在 `~/.local/state/tilewall-profile-20261117/assert_shots.py`。
