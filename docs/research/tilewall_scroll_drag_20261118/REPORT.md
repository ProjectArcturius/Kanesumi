# 磁贴墙提供方补丁：滚动偏移、拖动基准、视口外预取（2026-11-18）

> 任务卡：`k-tilewall-scroll-drag`（Ether `tools/dispatch/tasks/m1115/k-tilewall-scroll-drag.md`）。
> 仓：Kanesumi，分支 `oc/k-tilewall-scroll-drag`，基线 `main`（前两卡已合入 `8193dcf`）。
> 范围：只补提供方三件（B1 / B2 / B3）；不改前两卡已达标的部分，不做应用 C1。
> 代码提交：`4d59d947`；本报告与 `CONTROL_SPEC.md` 随后单独提交。

## 结论

- **B1 拖动基准**：新增 `layout_without`（几何层与控件层）、`drag_baseline`、
  `drop_target_at` 与 `WallDropTarget`（组内插入位 / 新建组 / 原位激活）。九行表里
  **八行逐行相等**；第 9 行 `(1270,760)` 的旧模型实际读数是 `InGroup{1,2}`，不是表里的
  `{1,1}` —— 表值在 `y = 276`（磁贴首行）上成立。证据与判定见 §一·B1 的差异段。
- **B2 滚动偏移**：新增 `set_scroll` / `scroll`，夹到 `[0, max_scroll]`，命中与绘制
  共用同一份平移后的 `layout`；未设滚动时输出逐命令不变。
- **B3 视口外预取**：新增 `set_overscan_cols` / `overscan_cols` 与几何层
  `layout_with_overscan`；1 列 = 组列步距，C0 的 20 枚全在，12 组 @1366 回 4 组。
- 四条验收命令全绿（§三）；既有单测零变更（§四）。

## 一 · 三张表的逐行读数

### B1 · 拖动落点（C0 夹具，拖 app01；app01 是组 0 第 0 枚 2×2）

读数由 `MetroTileWall::drop_target_at` 给出（测试：
`navigation::tile_wall_control::tests::drop_target_matches_legacy_launcher_table`）。
表中只给 x 的行，y 取被拖项所在行 `276`；旧 `drop_target` 的命中判据是完整磁贴矩形，y 必须
落在那一行读数才可比。

| 松手点 | 旧模型 `tiles::drop_target` | 控件旧 `release` | 本实现 `drop_target_at` | 判定 |
|---|---|---|---|---|
| 180 | `InGroup{0,0}` | `Activated("app01")` | `Activated("app01")` | 保留（落点即原位） |
| 200 | `InGroup{0,1}` | `Cancelled` ❌ | `InGroup{0,1}` | 相等 |
| 250 | `InGroup{0,1}` | `Cancelled` ❌ | `InGroup{0,1}` | 相等 |
| 312 | `InGroup{0,1}` | `Moved{0,0→0,1}` ✔ | `InGroup{0,1}` | 相等 |
| 350 | `InGroup{0,2}` | `Moved{0,0→0,1}` ❌ | `InGroup{0,2}` | 相等 |
| 400 | `InGroup{0,2}` | `Moved{0,0→0,1}` ❌ | `InGroup{0,2}` | 相等 |
| (-10,376) | `NewGroup{at:0}` | `Cancelled` ❌ | `NewGroup{at:0}` | 相等 |
| (50,400) | `NewGroup{at:0}` | `Cancelled` ❌ | `NewGroup{at:0}` | 相等 |
| (1270,760) | `InGroup{1,1}`（表） | `Cancelled` ❌ | **`InGroup{1,2}`** | 见下 |
| (1270,276) | `InGroup{1,1}` | — | `InGroup{1,1}` | 表值在首行成立 |

**第 9 行的差异（已核实，不是实现取巧）**：launcher `drop_target` 先判
`tile_rect.contains(pointer)`（含 y；`launcher/src/shell/tiles.rs:202-235`）。C0 基准布局
（摘除 app01）第二组只有 app19 / app20 两枚，矩形 y 都是 `201..351`；`(1270,760)` 的
`y = 760` 既超过墙高 752，也超过组块底 671，两枚都不 contains，于是走「组内空白 → 追加组尾」，
读数是 `InGroup{1,2}`。同一 x 在 `y = 276` 时才是 `InGroup{1,1}`。
独立复算脚本 `probe_drop_target.py`（本目录，逐行复刻 launcher 算法）输出：

```text
pos=(1270,760) -> ('InGroup', 1, 2)
pos=(1270,276) -> ('InGroup', 1, 1)
```

另据 Ether `docs/research/m1115_tilewall/C0.md` §3.6.2，`(1270,760)` 是墙外的 Dock 钉选带，
旧代码在那里松手会钉 Dock（`pin_index` 0→18），**不会**进磁贴墙落点判定；表里这一行的
`InGroup{1,1}` 应是同一 x 在首行探针上的读数。两种读数都写进了用例，等调度者裁定以哪个
作为第 9 行判据（实现按 launcher 语义，不做 y 忽略的特殊分支）。

### B2 · 滚动偏移（C0 夹具）

用例：`scroll_shifts_hit_and_render_together`、`scroll_clamps_to_max_and_non_finite_resets`、
`explicit_zero_scroll_keeps_layout_and_render_identical`（均在
`navigation::tile_wall_control::tests`）。

| 操作 | 读数 |
|---|---|
| C0 `layout.max_scroll` | `914.8901` |
| `set_scroll(500)` | `scroll() = 500`、`layout.scroll = 500` |
| 同一枚 app06 的 `rect.origin.x` | `912.4451` → `412.4451`（只横向平移） |
| 指针移到 `p.x − 500` 再命中 | 仍是 `app06`（命中消费同一份几何） |
| `render` 的磁贴底 | `FillRect.rect == slot.clip`（滚动后的裁剪矩形） |
| `set_scroll(99_999)` | `914.8901`（夹到上界） |
| `set_scroll(-50)` / `set_scroll(NaN)` | `0` |
| 内容变少后 `set_groups` 重排 | 写回值被夹到新上界（本例 `0`） |
| `relayout` 同视口 | 滚动保持 `500` |
| 另建一面墙 `set_scroll(0)` | `layout` 逐字段相同、`render` 命令序列逐条相同 |

### B3 · 视口外预取

用例：`overscan_zero_is_field_by_field_identical`、
`overscan_returns_twentieth_tile_beyond_right_edge`、
`overscan_two_sides_include_whole_group_columns`（几何层）；
`overscan_default_unchanged_and_two_columns_adds_app20`、
`overscan_twelve_groups_at_1366_returns_viewport_plus_columns`（控件层）。

| 场景 | 默认（`cols = 0`） | `cols = 2` |
|---|---|---|
| C0 夹具 slot 数 | 19（`app20` 被丢） | 20（`app20` 在 x=1292.4451，`clip` / `hit` 空矩形，`partial = false`） |
| C0 夹具组数 | 2 | 2 |
| 12 组 @1366 组数 | 2 | 4（组 0..3；组 3 在 x=3180 完全在视口外） |
| 12 组滚动 500 后 | — | 同时含 `origin.x < 0` 的左侧列与 `origin.x ≥ 1366` 的右侧列 |
| `cols = 0` 与旧 `layout` | — | 逐字段相同（`assert_eq!` 整结构） |
| 关掉预取（`set_overscan_cols(0)`） | — | 逐字段回到旧输出 |

列单位说明：1 列 = 组列步距（`group_width + group_gap`）。launcher 的
`PREFETCH_COLS = 2` 按**宏单元列**算（`2 × 2 × (cell + gap)` ≈ 320 px，参 Ether
`docs/research/n1109_tiles/REPORT.md` §3.1）；320 px 的窗口在 12 组 @1366 上只与
组 0 / 组 1 相交，达不到本卡「≥ 3 组」的判据，所以这里按组列计步距（`cols = 2`
回 4 组、`cols = 1` 回 3 组）。步距变大意味着替更多列付图层缓冲，调用方可用更小的
`cols` 换；这条取舍写进了 `tile_wall.rs` 的文档注释。

## 二 · 实现（逐文件）

### `kanesumi-controls/src/navigation/tile_wall.rs`

- `Visibility`（301 行）：`Viewport` / `Overscan(n)` / `All` 三种过滤策略，`layout` 仍走
  `Viewport`，与旧实现同一条 `rect.intersect(viewport)` 判据。
- `layout_with_overscan(input, cols)`（327 行）：按组列步距把视口左右各扩 `cols` 列；
  `cols = 0` 直接回 `layout`（逐字段不变）。
- `layout_without(input, group, index)`（339 行）：摘除一枚后走 `All`（全量，落点判定要
  全部组矩形）；组索引不变、摘空的组保留。
- 预取槽位：`clip` / `hit` 为空矩形，`partial` 只在「部分可见」时为真；`max_scroll` /
  `pages` / 滚动夹紧仍只看视口，不受预取影响。

### `kanesumi-controls/src/navigation/tile_wall_control.rs`

- `WallDropTarget`（97 行）：`Activated(String)` / `InGroup{group,index}` / `NewGroup{at}`。
- `set_scroll` / `scroll`（190 / 202 行）：夹到 `[0, max_scroll]`，非有限值归零；
  `relayout` 后写回夹紧值。
- `set_overscan_cols` / `overscan_cols`（207 / 214 行）。
- `relayout`（244 行）：把 `scroll` / `overscan` / 当前度量一起交给几何层，并写回
  `layout.scroll`。
- `layout_without(group, index)`（260 行）、`drag_baseline()`（268 行）、
  `drop_target_at(pos)`（280 行）。
- `drop_target_in`（663 行）：逐条复刻 launcher `tiles.rs::drop_target` 的组区间 / 左右半 /
  组内空白 / 组间缝隙判定；`settle`（701 行）把「基准下标 == 原位」收敛成 `Activated`。
- 绘制：`render_slots` / `render_legacy_titles` / `render_group_title` 跳过空裁剪
  （视口外预取列不发屏外命令；默认无空裁剪，输出不变）。
- 旧 `release` 未改：它仍按静息布局判落点、墙外仍 `Cancelled`，既有集成用例不受影响；
  需要旧 launcher 落点的调用方改用 `drop_target_at`。

## 三 · 门与命令

环境：`export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH`、`CARGO_BUILD_JOBS=8`、
`CARGO_TARGET_DIR=/home/rain/.oc/target/k-tilewall-scroll-drag`。

| # | 命令（原样） | 退出码 | 汇总 |
|---|---|---|---|
| 1 | `cargo test --offline -p kanesumi-controls --lib` | 0 | `997 passed; 0 failed; 0 ignored` |
| 2 | `cargo test --offline -p kanesumi-controls --test tile_wall_control` | 0 | `14 passed; 0 failed; 0 ignored` |
| 3 | `cargo test --offline -p kanesumi-element --test api_surface` | 0 | `1 passed; 0 failed` |
| 4 | `! grep -q "#\[ignore\]" kanesumi-controls/src/navigation/tile_wall.rs kanesumi-controls/src/navigation/tile_wall_control.rs` | 0 | 无 `#[ignore]` |

`cargo clippy --offline -p kanesumi-controls --all-targets` 退出 0；告警数与基线一致
（lib test 36 条，含 3 条 duplicate）。本件两个文件的告警只有 `tile_wall_control.rs:422/424`
的 `if_same_then_else`，位于**未改动**的 `apply_move`（diff 无覆盖该行的 hunk），属基线存量。

未跑 `cargo test --offline --workspace --features kanesumi-harness/a11y`（任务卡已列
`kanesumi-canvas::font_index::tests::real_stack_identity_matches_text_engine` 为基线红）；
本件只改 `kanesumi-controls` 两个文件，没有触碰字体栈。此条**未验证**，如实列出。

## 四 · 既有用例变更表

**无。** 既有用例的输入、断言、夹具、忽略项一个都没改；新增用例只追加。

| 文件 | 既有用例 | 本件改动 | 新增 |
|---|---|---|---|
| `kanesumi-controls/src/navigation/tile_wall.rs` | 10 条 | 0（`scroll_clamps_to_max_and_pages_end_at_max` 等原样） | 5 条 |
| `kanesumi-controls/src/navigation/tile_wall_control.rs` | 5 条 | 0（C0 几何 / 组头 / 4×2 / 度量缩放原样） | 7 条 |
| `kanesumi-controls/tests/tile_wall_control.rs` | 11 条 | 0（只追加导入与 3 条用例） | 3 条 |
| `docs/API_SURFACE.txt` | 快照 | 新增 10 行 / 移除 0 行 | — |

新增的 10 行 API：`WallDropTarget`、`drag_baseline`、`drop_target_at`、
`layout_with_overscan`、`layout_without`（几何层 + 控件层两条）、`overscan_cols`、
`scroll`、`set_overscan_cols`、`set_scroll`。

## 五 · 未做 / 不确定 / 范围外

- **第 9 行判据待裁定**：`(1270,760)` 按 launcher 语义读 `InGroup{1,2}`，表写 `{1,1}`
  （该值在 y=276 成立）。实现没有为了对齐表而加「忽略 y」的特例，需要调度者选一个判据。
- **`release` 未迁移**：新落点入口与旧 `release` 并存。把 `release` 也切到基准布局会改
  `WallRelease` 语义（`NewGroup` 与「墙外 = 取消」冲突，`drag_out_of_wall_cancels` 会红），
  超出本卡允许面，留给应用 C1 或下一张卡。
- **没有 `apply_drop` / `preview_drop`**：本卡只要求「落点能区分三态」。基准下标可直接
  `remove` 后 `insert`（组索引不漂移），但应用侧要不要删空组是产品策略，未在控件里定。
- **未 re-export**：`WallDropTarget` 只从 `navigation::tile_wall_control` 可达，
  没有动 `navigation/mod.rs`（不在本卡允许文件列表）。应用接线时可一行补上。
- **范围外发现（未修）**：`jump_to_group`（`tile_wall_control.rs:470-476`）返回
  `(g.rect.origin.x - self.layout.scroll)`，
  而 `rect.origin.x` 已经含 `-scroll`，滚动非零时会二次扣减；`scroll = 0` 时读数也把
  `margin_x` 算进了「滚动偏移」。本卡未改它（既有行为、不在三条契约内）。
- **未验**：真实合成器 / DRM 下的拖动与滚动（本件全是几何与命令流的单测）；
  应用 C1 的接线效果。

## 六 · 复现

```text
python3 docs/research/tilewall_scroll_drag_20261118/probe_drop_target.py
cargo test --offline -p kanesumi-controls --lib navigation::tile_wall
cargo test --offline -p kanesumi-controls --lib navigation::tile_wall_control
cargo test --offline -p kanesumi-controls --test tile_wall_control
```

`probe_drop_target.py` 只复算 launcher `drop_target` 的算术（几何照 C0：摘除 app01 后
组 0 有 17 枚、组 1 有 2 枚；`rows=6, group_h=542, y0=129, grid_y=201`），不依赖被测控件；
它与 `drop_target_in` 的独立实现同时落在同一组读数上，用于交叉核对表值。
