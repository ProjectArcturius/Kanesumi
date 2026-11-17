# k-dock-present 报告：Dock 的呈现几何与按槽绘制

> 卡：Ether `tools/dispatch/tasks/m1115/k-dock-present.md`。基线 `main`（含 `0467d92d`），
> 分支 `oc/k-dock-present`。工作树 `~/.oc/wt/k-dock-present`。执行机 t14p，`rustc 1.99.1`。
> 只提交、不推送；未改 Ether 任何文件，未改 `launcher/**`。

## 〇 · 结论

补了两层，旧出口没动：

1. **呈现输入** `DockPresentation`：板宽 / 板顶 / 板高、逐槽位移（id 或 `#source`）、
   逐区分数滚动、逐槽线色进度。`DockRail::present_layout` 在落定 `DockLayout` 上重排，
   旧 `DockRail::layout` 一字未改。
2. **按槽绘制** `MetroDock::render_slots`：返回 `Plate / Entry / Slot / Chip` 四类
   `DockCellDraw`，每格带 `rect`、`damage` 与 `Scene.commands` 半开区间。悬停 / 按下 /
   激活底、定位线、紧急点、计数徽标、入口格、计数格、幽灵、呼吸、轻跳都在控件里。

旧 `MetroDock::render` 是**落定路径**，即使 `set_presentation(Some(...))` 也不读呈现输入；
回归 `legacy_render_ignores_presentation` 逐命令相等。`set_items` / `layout()` /
`indicator_rect` / `try_indicator_rect` 的签名与默认输出未变；`DockRail::layout` 未改。

## 一 · 规格先行：API 形状

### 1.1 输入 `navigation::dock_rail::DockPresentation`

```rust
pub struct DockPresentation {
    pub plate_width: Option<f32>,   // 完整板宽（含左右 12）；None = 落定；0 = 无板
    pub plate_top: Option<f32>,
    pub plate_height: Option<f32>,
    pub slot_offsets: Vec<(String, f32)>, // 稳定 id；找不到再按 "#<source>"
    pub scroll: Option<[f32; 2]>,   // 每区当前滚动位置（可含小数槽）
    pub line_progress: Vec<(String, f32)>, // 0=Divider / 0.5=Primary / 1=PrimaryHover
}
```

- 整数滚动目标仍由 `MetroDock::set_scroll([usize; 2])` 给；`scroll` 只给动画中的当前小数位。
  偏移 = `(before − scroll) × 槽宽`，与旧 `arrange` 的 `(local − scroll_pos) × stride` 同式。
- `plate_width` 是**完整外宽**（旧 `plate_shown` 是内容宽；调用方加 `2 × 12` 即等价）。
- `line_progress` 缺省取 `DockIndicatorKind` 的自然端点：Pinned=0、Running=0.5、Focused=1。

### 1.2 输出

```rust
pub struct DockPresentedLayout {
    pub metrics: DockMetrics, pub dock: Rect,
    pub plate: Option<Rect>, pub entry: Option<Rect>,
    pub zones: [DockPresentedZone; 2], pub slots: Vec<DockPresentedSlot>,
    pub line_y: f32, pub line_height: f32, pub overflow: bool,
}
pub struct DockPresentedSlot {
    pub source: usize, pub display: usize, pub zone: DockZone,
    pub id: String, pub icon: String,
    pub rect: Rect, pub hit: Option<Rect>, pub icon_rect: Rect,
    pub visible: bool, pub indicator: Option<Rect>,
    pub indicator_kind: DockIndicatorKind, pub line_progress: f32, pub dx: f32,
}
pub struct DockPresentedChip { pub zone: DockZone, pub direction: DockChipDirection,
    pub count: usize, pub rect: Rect, pub line: Rect }
pub struct DockPresentedZone { pub zone: DockZone, pub rect: Rect, pub viewport: Rect,
    pub before: usize, pub after: usize, pub visible: usize,
    pub chips: Vec<DockPresentedChip> }
```

- `DockPresentedLayout::hit(pos)` 与呈现矩形同源（计数格 → 入口格 → 槽位）。
- `MetroDock::presented_layout()` 把启动轻跳并进 `icon_rect`，消费方不再减偏移；
  `MetroDock::hit(pos)` 仍是落定命中，避免过渡帧命中抖动。

### 1.3 按格绘制

```rust
pub enum DockCell {
    Plate, Entry,
    Slot { display: usize, source: usize },
    Chip { zone: DockZone, direction: DockChipDirection },
}
pub struct DockCellDraw {
    pub cell: DockCell, pub rect: Rect, pub damage: Rect,
    pub commands: std::ops::Range<usize>,
}
```

`render_slots(theme, engine, scene)` 按 `Plate → Entry → Slot → Chip` 顺序画，每格命令连续，
`commands` 直接索引 `scene.commands`。`damage` 只覆盖本格：槽位 x 到本槽边界（溢出时到区视口），
徽标上方 1 pt 与轻跳已算进 y；计数格占自己的 36 pt；入口格独立。

## 二 · 改动清单

| 文件 | 改动 |
|---|---|
| `kanesumi-controls/src/navigation/dock_rail.rs` | 新增 `DockPresentation` / `DockPresented*` / `DockRail::present` / `present_layout` / `DockPresentedLayout::hit`；旧 `layout` 未改。 |
| `kanesumi-controls/src/navigation/dock.rs` | 新增 `DockCell` / `DockCellDraw` / `set_presentation` / `presentation` / `presented_layout` / `render_slots`；把 `relayout` 的条目与让位合成抽成 `entries` / `composed_layout` / `rail_input_with`（旧输出不变）。 |
| `kanesumi-controls/tests/dock_control.rs` | 追加 6 条呈现 / 损伤 / 线色 / 旧路径回归；旧 21 条未改。 |
| `kanesumi-controls/src/navigation/dock_rail.rs` 测试模块 | 追加 6 条呈现几何回归；旧用例未改。 |
| `docs/CONTROL_SPEC.md` | 追加「MetroDock 呈现几何与按槽绘制（2026-11-17）」。 |
| `docs/API_SURFACE.txt` | 重新生成：新增 21 行、移除 0 行。 |
| `docs/research/dock_present_20261117/REPORT.md` | 本报告。 |

`kanesumi-core` 未改：线色用现有 `Color::lerp`，10% 叠色用现有 `Color::over`，控件生产段
没有 `Color::new`。`navigation/mod.rs` 未改（不在允许范围），新类型用
`navigation::dock_rail::*` / `navigation::dock::*` 全路径访问。

## 三 · 真值取证（独立取，不从 `layout()` 反推）

### 3.1 取证口径

期望值只来自两处：

- §158 / §185 / §186 / §187 / §188 的尺寸常量：槽宽 60、图标 48、板高 68、板内边距 12、
  线高 3、入口格 1 格、计数格 36、轻跳 3；
- Ether 冻结的 C0 现场：1536 × 76、入口格开、1×，librarian / nvim 钉选未运行、
  kitty 运行且激活。C0 转录文件未改、未重录。

没有用被测的 `layout()` 生成 expected：测试里的 636 / 264 / 708 / 768 / 828 / 65 / 3
都是按上述常量手算后写成字面量。

### 3.2 逐条命令与读数

```bash
cargo test --offline -p kanesumi-controls --lib navigation::dock_rail::tests::presentation_settled_matches_c0_constants
cargo test --offline -p kanesumi-controls --lib navigation::dock_rail::tests::presentation_plate_halfway_reflows_content
cargo test --offline -p kanesumi-controls --lib navigation::dock_rail::tests::presentation_pin_gap_offsets_by_one_slot
cargo test --offline -p kanesumi-controls --lib navigation::dock_rail::tests::presentation_fractional_scroll_moves_half_a_slot
cargo test --offline -p kanesumi-controls --test dock_control presentation_damage_is_per_cell_and_non_overlapping
cargo test --offline -p kanesumi-controls --test dock_control presentation_line_progress_endpoints_and_midpoints
cargo test --offline -p kanesumi-controls --test dock_control presentation_plate_and_offsets_move_rendered_cells
cargo test --offline -p kanesumi-controls --test dock_control presentation_chip_damage_is_its_own_cell
cargo test --offline -p kanesumi-controls --test dock_control legacy_render_ignores_presentation
cargo test --offline -p kanesumi-controls --test dock_control presented_icon_rect_includes_starting_hop
```

| 要求 | 读数 | 用例 |
|---|---|---|
| ① 落定几何 | 板 `(636, 0, 264, 68)`；入口 `(648, 0, 60, 68)`；槽 x `708 / 768 / 828`；图标 `(714, 10, 48, 48)`；线 `y 65 h 3 w 60` | `presentation_settled_matches_c0_constants` |
| ① 落定命中 | `738 → Slot(0)`、`858 → Slot(2)`、`678 → Entry` | 同上 |
| ② 板宽插值一半 | 264 → 234：板 `(651, 0, 234, 68)`、入口 `663`、槽 `723 / 783 / 843` | `presentation_plate_halfway_reflows_content` |
| ② 让位偏移一格 | 板 324、槽 `678 / 798 / 858`（nvim / kitty 各 +60） | `presentation_pin_gap_offsets_by_one_slot` |
| ② 滚动 0.5 槽 | 视口 x 190；落定首槽 130，滚动 0.5 → 160；第 1 槽 220；回 1.0 → 130 / 190 | `presentation_fractional_scroll_moves_half_a_slot` |
| ③ 按槽损伤 | C0 三个槽损伤 x 各 60、y 0..68、两两重叠面积 0；入口格 648 + 60；板 636 + 264 | `presentation_damage_is_per_cell_and_non_overlapping` |
| ③ 让位后损伤 | 板 324 时四个槽损伤两两不重叠 | `presentation_plate_and_offsets_move_rendered_cells` |
| ③ 计数格损伤 | 每枚计数格只盖自己的 36 pt，与所有槽损伤重叠面积 0 | `presentation_chip_damage_is_its_own_cell` |
| ④ 线色端点 / 中点 | `0 → Divider`、`0.25 → Divider.lerp(Primary, 0.5)`、`0.5 → Primary`、`0.75 → Primary.lerp(PrimaryHover, 0.5)`、`1 → PrimaryHover` | `presentation_line_progress_endpoints_and_midpoints` |
| 旧出口不变 | 启用呈现输入前后，旧 `render` 的 `Scene` 逐命令相等；`layout().slots[2].x` 不变 | `legacy_render_ignores_presentation` |
| 轻跳进呈现位 | `set_starting_motion(0.0, 0.5)` → kitty 图标 y 10 → 7；旧 `layout()` 仍 10 | `presented_icon_rect_includes_starting_hop` |

### 3.3 现有用例变更表

**无。** 既有 21 条 `dock_control` 与 `dock_rail` 旧用例的输入、断言、夹具、忽略项均未改；
只追加了 12 条（rail 6 + dock_control 6）。`#[ignore]` 未新增（验收命令单独检查）。

### 3.4 API 增删

`cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt`：

- 新增 21 行、移除 0 行。
- 新增：`DockCell`、`DockCellDraw`、`DockPresentation`、`DockPresentedChip`、
  `DockPresentedLayout`、`DockPresentedSlot`、`DockPresentedZone`；
  `present` / `present_layout` / `presentation` / `presented_layout` / `render_slots` /
  `set_presentation` / `slot`（呈现版）/ 7 个 `with_*` 构造器（含按 source 的两个）。
- 旧签名一行未删：`layout` / `indicator_rect` / `try_indicator_rect` / `render` /
  `set_items` 等都在快照里保持原样。

## 四 · 没做的 / 边界

1. **未迁应用**：本卡只补提供方，未改 launcher / Ether；入口格点击、计数格翻页动作、
   图标解码仍归调用方。`render_slots` 只给命令与损伤，不代登记到具体合成器。
2. **旧 `render` 不消费呈现输入**是有意的：先保证迁移期旧截图逐像素不变；调用方要中间态
   必须显式切 `presented_layout` / `render_slots`。若后续要求 `render` 也吃呈现输入，应另开
   卡并同时改「默认输出不变」的契约。
3. **`MetroDock::hit` 仍是落定命中**。呈现期命中由 `presented_layout().hit()` 提供；
   没有把过渡帧的命中塞回 `MetroDock::hit`，避免状态机在动画中抖动。
4. **RTL 未实现**：沿用 `DockDirection` 的既有 TODO，呈现层不做镜像。
5. **逐槽位移是水平分量**：轻跳是纵向，由 `set_starting_motion` 的 jump 相位产出并并进
   `presented_layout().icon_rect`；`slot_offsets` 不承担 y。
6. **`navigation/mod.rs` 未加重导出**：不在本卡允许改动范围，新类型走
   `navigation::dock_rail::DockPresentation` / `navigation::dock::DockCell` 全路径。
7. **损伤与任意大位移**：`slot_offsets` 是调用方给的呈现位移；若把两个槽硬推到同一格，
   两份 `damage` 会重叠（几何事实）。回归覆盖的是让位 / 换位 / 分数滚动这些旧路径实际
   产生的位移；没有在控件里替调用方做「位移后防重叠」的二次约束。
8. **`cargo fmt --all` 基线红**未处理：只对本次改动的三个文件跑
   `rustfmt --edition 2024`；没有格式化范围外文件。

## 五 · 范围外发现

1. `navigation/mod.rs` 的 Dock 重导出列表没有 `DockPresentation` / `DockCell`；因不在允许
   范围未改。若下游希望 `navigation::DockPresentation`，需要一次显式 API 增补。
2. `DockLayout.hovered` 与 `MetroDock::hit` 的「幽灵过滤」两套语义仍并存（上一卡已记录）；
   本卡的 `DockPresentedLayout::hit` 不做幽灵过滤，幽灵语义仍由 `MetroDock` 层负责。
3. `render_slots` 的 `DockCell::Plate` 是整块背景损伤；若合成器只想重绘变化槽，必须把
   Plate 与 Slot 的损伤分开处理，不能把 Plate 当成「第一格」合并。
4. 本卡开工后 `main` 前进到 `8193dcf`（tile_wall 卡），本分支仍基于卡面基线 `0467d92d`；
   `git diff main..HEAD` 会把 tile_wall 的删除误显示成本卡改动，真实差异应看
   `git diff 0467d92d..HEAD`（6 文件、+1399/−16）。本卡未合并、未重放 main 的新提交。

## 六 · 门执行记录（2026-11-17，t14p）

环境：`export PATH=$HOME/.local/state/ether-build-compact-20261117/bin:$HOME/.cargo/bin:$PATH`，
`CARGO_BUILD_JOBS=1`，`CARGO_TARGET_DIR=$HOME/.oc/target/k-dock-present`，
`KANESUMI_TEST_FONT=/usr/share/fonts/google-noto-sans-cjk-vf-fonts/NotoSansCJK-VF.ttc`（继承当前会话）。
日志落盘在 `~/.local/state/k-dock-present-20261117/`。

| 命令 | 结果 | 日志 |
|---|---|---|
| `cargo test --offline -p kanesumi-controls --lib` | **通过**：978 passed / 0 failed / 0 ignored | `k-dock-lib-final.log` |
| `cargo test --offline -p kanesumi-controls --test dock_control` | **通过**：27 passed / 0 failed / 0 ignored | `k-dock-dock-final.log` |
| `cargo test --offline -p kanesumi-element --test api_surface` | **通过**：1 passed / 0 failed | `k-dock-api-final.log` |
| `cargo test --offline --workspace --features kanesumi-harness/a11y` | **红**：退出 101，在 `kanesumi-canvas --lib` 的 `font_index::tests::real_stack_identity_matches_text_engine` 失败处停止；该命令已跑到的 4 个测试二进制均 ok，未跑到的二进制不作断言 | `k-dock-workspace-fg.log` |
| `cargo clippy --offline --workspace --all-targets --features kanesumi-harness/a11y` | **通过**：退出 0；`navigation/dock*` 无告警（既有告警仍在范围外文件） | `k-dock-clippy-final.log` |
| `! grep -q "#\[ignore\]" kanesumi-controls/src/navigation/dock.rs kanesumi-controls/tests/dock_control.rs` | **通过**：退出 0 | — |

### 6.1 精确命令观察到的红：本体基线既有，不是本卡引入

失败读数：

```text
font_index::tests::real_stack_identity_matches_text_engine
identity: index=0x57a723571e5b4073 engine=0x42750271300758f8 font_count=2
assertion `left == right` failed: 索引身份必须与 TextEngine 相等
```

取证：把本卡改动（含未跟踪报告）`git stash push -u` 后，在**干净 HEAD `main`** 上单跑同一条：

```text
cargo test --offline -p kanesumi-canvas --lib font_index::tests::real_stack_identity_matches_text_engine
→ exit 101，同样的 index/engine 身份不一致
```

随后 `git stash pop` 恢复。故这是本机字体布局（只有 `NotoSansCJK-VF.ttc`，无静态 Bold 兄弟）
触发的**基线红**，不是本卡改出来的；本卡未改 `kanesumi-canvas/**`。
本机 `system_font_chain()` 的主字面是 VF，`real_font_paths()` 找不到文件名含 `Bold` 的兄弟，
退化成同一路径两条；`TextEngine::load_stack` 去重后 font_count=1，索引仍是 2 条，身份必然不等。
修它需要改 `kanesumi-canvas` 的测试夹具或装静态 Bold 字体，均超出本卡允许范围，故未动。

该命令不带 `--no-fail-fast`，在 canvas 失败处即停，所以不能据此宣称「整个 workspace 只剩这一条」。
另外，带 `--no-fail-fast` 并行跑时，同一测试进程里确有 `remove_var("KANESUMI_TEST_FONT")`
（`kanesumi-harness/src/platform.rs:5917`、`kanesumi-harness/tests/font_selection_regression.rs:141`），
`kanesumi-element` / `kanesumi-harness` / `kanesumi-render` 会再红一批；`find_test_font` 的静态表
也不含本机 `/usr/share/fonts/dejavu-sans-fonts/`。这**推测**是环境变量竞态（未单独复现），
同样是基线测试的环境耦合，不是本卡引入；本卡只保证本卡四个包的门与未改动的旧回归。

## 七 · 格式与提交

- `cargo fmt --all -- --check` 基线本来就红（约 190 文件），未顺手格式化范围外文件。
- 本卡只对三个改动文件执行 `rustfmt --edition 2024`：`dock.rs`、`dock_rail.rs`、
  `tests/dock_control.rs`；`rustfmt --check` 对这三文件退出 0。
- 提交：`feat(dock): …`（代码 + 回归 + 规格 + 快照）+ `docs(dock): …`（本报告）两次提交；
  只提交、不推送。
