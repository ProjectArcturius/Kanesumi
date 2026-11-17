# k-dock-v2 报告：`MetroDock` 补齐到能替 launcher 的 Dock

> 卡：Ether `tools/dispatch/tasks/m1115/m-dock.md` 的后续（本卡）；上游证据
> `docs/research/m1115_dock/REPORT.md` §三（只读，不在本仓）。基线：`main`。
> 工作树：`~/.oc/wt/k-dock-v2`，分支 `oc/k-dock-v2`。执行机：Fedora，`rustc 1.99.1`。
> 范围：只提交、不推送；未改 launcher 一行。

## 〇 · 结论

`MetroDock` 已接上 `DockRailInput` 的 `scale / show_entry / scroll / hover`，定位线与状态色
改用 `slot.indicator` / `DockIndicatorKind::token()`，静置不再铺底。入口格、计数格、
右上徽标、让位空槽、关闭幽灵、启动呼吸 + 轻跳都已补进控件；图标仍只给位、不解码。
C0 三项输入的几何与命中逐项对上卡 §一。全部 `kanesumi-controls` 测试绿（含 21 条
`dock_control`）。**门没有全绿**：`cargo fmt --all -- --check` 与
`cargo clippy -p kanesumi-controls --all-targets -- -D warnings` 在基线（未改一行）就红，
红点全在范围外文件，详见 §四。

## 一 · 改动清单

| 文件 | 改动 | 位置 |
|---|---|---|
| `kanesumi-controls/src/navigation/dock.rs` | 输入打通：新增 `scale / show_entry / scroll / hover_pos / pin_gap / ghosts / starting / starting_progress` 字段与 setter；`relayout` 每帧灌进 `DockRailInput` | `set_scale` L181、`set_show_entry` L187、`set_scroll` L193、`set_hover` L228、`relayout` L302 |
| 同上 | 定位线改用 `slot.indicator`；`indicator_rect` 返回 `Option<Rect>`；颜色走 `DockIndicatorKind::token()` | `render` L452、`indicator_rect` L630 |
| 同上 | 状态色：静置不铺底；悬停 / 按下 / 运行且激活 = `hover_tint` 10% 叠 `surface`；按下不再变强调色 | `render` L455、L489–L501 |
| 同上 | 入口格悬停底 + `entry_icon_rect()`（32 居中，图标由调用方 blit） | `entry_icon_rect` L270、`render` L459 |
| 同上 | 计数格：格底 + 区基色线 + `‹ N` / `N ›` 文字（`Scene::label`，行高走 engine） | `render` L531–L570 |
| 同上 | 徽标换到图标右上、15 pt 胶囊，宽度 15/19/24，`99+` 截断 | `paint_count_badge` L589 |
| 同上 | 让位空槽 `set_pin_gap` / `pin_gap_rect`：用合成条目复用 `DockRail` 的排序 / 底板 / 视口，溢出时不让位 | `set_pin_gap` L202、`relayout` L315–L334、`pin_gap_rect` L261 |
| 同上 | 关闭幽灵 `set_ghosts`：线回 `divider`、`icon_opacity` = 25%、命中过滤；启动 `set_starting` + `set_starting_progress`：线在 Primary↔Divider 呼吸，`icon_jump` 导出 0..3 pt 上移 | `set_ghosts` L208、`icon_opacity` L282、`icon_jump` L293、`breath_color` L578 |
| `kanesumi-controls/tests/dock_control.rs` | 新增 9 条 C0 / 入口 / 定位线 / 状态色 / 徽标 / 幽灵 / 滚动 / 让位 / 缩放用例；改 2 条旧用例（见 §三） | L279–L536 |
| `kanesumi-core/src/color.rs` | 新增 `Color::over`（source-over，非预乘）与回归测试；控件用它把 10% tint 算成实色（token 纪律禁止控件内 `Color::new`） | `over` L78、测试 L169 |
| `kanesumi-core/src/sizes.rs` | 新增 `DOCK_START_HOP = 3.0`（§185 轻跳幅度）；入口图标复用既有 `ICON_32` | L54 |

`dock_rail.rs` 与 `kanesumi-element/src/access.rs` 零改动（`git diff main -- …` 为空）。

## 二 · 真值取证

测法：`kanesumi-controls/tests/dock_control.rs` 用 C0 同一份输入（1536 × 76、`show_entry = true`、
`scale = 1`、librarian / nvim 钉选未运行、kitty 运行且激活），由 `MetroDock` 产出布局与 Scene，
再断言矩形 / 命中 / 颜色。下表每行都能单独跑：

```bash
cargo test -p kanesumi-controls --test dock_control c0_truth_geometry_entry_and_hits
cargo test -p kanesumi-controls --test dock_control entry_off_keeps_previous_centering
cargo test -p kanesumi-controls --test dock_control indicator_uses_layout_rect_and_token_colors
cargo test -p kanesumi-controls --test dock_control rest_hover_press_and_active_fills
cargo test -p kanesumi-controls --test dock_control badge_is_top_right_capsule_fifteen_high
cargo test -p kanesumi-controls --test dock_control ghost_ignores_hit_and_grays_line
cargo test -p kanesumi-controls --test dock_control scroll_moves_left_zone_only
cargo test -p kanesumi-controls --test dock_control pin_gap_opens_a_slot_and_shifts_left_zone
```

| 真值（卡 §一） | 控件实测 | 用例 |
|---|---|---|
| plate x 636..900（w 264） | `Rect(636, 0, 264, 68)` | `c0_truth_geometry_entry_and_hits` |
| entry x 648..708（w 60） | `Rect(648, 0, 60, 68)` | 同上 |
| slot x 708 / 768 / 828 | `[708, 768, 828]` | 同上 |
| `hit(738,34)` = librarian（source 0） | `DockHit::Slot(0) → source 0` | 同上 |
| `hit(858,34)` = kitty（source 2） | `DockHit::Slot(2) → source 2` | 同上 |
| `hit(678,34)` = Entry | `DockHit::Entry` | 同上 |
| 定位线 y 65..68、高 3 | 每槽 `indicator.origin.y = 65`、`height = 3` | `indicator_uses_layout_rect_and_token_colors` |
| Pinned → Divider / Running → Primary / 激活 → PrimaryHover | `slots[0].token = Divider`、`slots[2].kind = Focused`、`token = PrimaryHover`；渲染色与 token 逐槽相等 | 同上 |
| 静置不铺底（surface #171717） | 图标中心 `fill_at = surface` | `rest_hover_press_and_active_fills` |
| 悬停 / 运行且激活 = 10% 叠色（#2e2e2e） | `fill_at = hover_tint.over(surface)`（暗色实算 #2e2e2e） | 同上 |
| 按下不变强调色 | `fill_at = 悬停同档`，`≠ primary` | 同上 |
| 徽标右上、高 15 | `height = 15`、`y = icon.y − 1`、`right = icon.right + 3`、Capsule 7.5 | `badge_is_top_right_capsule_fifteen_high` |
| `show_entry = false` 回到 plate 666 w 204、槽 678/738/798 | `Rect(666, 0, 204, 68)`、`[678, 738, 798]` | `entry_off_keeps_previous_centering` |
| `set_ghosts` 后该槽 hit = None | kitty 幽灵：`hit(858,34) = None`，线回 `divider`，`icon_opacity = 0.25` | `ghost_ignores_hit_and_grays_line` |
| `set_scroll([1,0])` 左区首槽变化、右区不滚 | `zones[0].before 0→1`，左区首个可见 `source 0→1`；`zones[1].before = 0`，右区槽序 / `first_display` 不变 | `scroll_moves_left_zone_only` |

按下取 `hover_tint` 而不是 `press_tint`：`press_tint` 在 `MetroIndication` 里是 20%，
叠 `surface` 得 #464646，与 C0 的 #2e2e2e 不符；卡 §二 C 的「press_tint 同档」按真值
落在 10% 这一档，故三态共用 `hover_tint`。

补充：`scale = 2` 时板宽 528、线 y 130 / 高 6（`scale_scales_line_and_plate`）；
`set_pin_gap(Some(1))` 让出一整格 60、板宽 +60、后续槽相对板右移一格（`pin_gap_opens_a_slot_and_shifts_left_zone`）。

## 三 · 现有用例变更表

| 用例 | 旧断言 | 新断言 | 为什么新值才是对的 |
|---|---|---|---|
| `running_and_pinned_zones_use_different_line_colors` | 线矩形用 `indicator_rect` 的旧自算式；Pinned 期望 `on_surface_variant` | `indicator_rect` 返回 `Option<Rect>`（= `slot.indicator`）；期望色 = `indicator_kind.token().resolve(theme)`，Pinned 显式 `divider` | C0 真值左区线 = `Divider(#333333)`，旧 `on_surface_variant(#9AA0A6)` 不在真值里；线 y 也从 60..63 改为 65..68 |
| `active_and_urgent_are_different_signals` | 「active 只改颜色，不增减元素」：两份 Scene 的 FillRect 数相等 | 静置格 `fill_at = surface`；active 格 `fill_at = hover_tint over surface`；urgent 另有一条 `status.critical` 填充 | 新真值静置不铺底、active 才铺 10%，命令条数必然不同；颜色与语义点才是这两个信号的真实区别 |

其余 10 条旧用例原样保留并通过（`hover` / `press` / `release` / `cancel` / 空 Dock /
底板边界 / 紧急点 / 存在徽标 / 状态转录等）。`dock_state_transcript` 的逐行文本未改。

## 四 · 门执行记录（原样命令）

| 命令 | 结果 |
|---|---|
| `cargo test -p kanesumi-controls` | **绿**：lib 964 passed；`dock_control` 21 passed；全部集成测试通过（size/token/idioms 等） |
| `cargo test -p kanesumi-controls --test dock_control` | **绿**：21 passed；0 failed；0 ignored |
| `test "$(git diff main -- kanesumi-controls/src/navigation/dock_rail.rs \| wc -l)" -eq 0` | **绿**（exit 0） |
| `! grep -q "#\[ignore\]" kanesumi-controls/tests/dock_control.rs` | **绿**（exit 0） |
| `grep -q "65" … \|\| grep -q indicator …` | **绿**（exit 0，命中 `indicator_rect`） |
| `cargo clippy -p kanesumi-controls --all-targets -- -D warnings` | **红（基线，非本卡）**：`kanesumi-core/src/accent.rs:149` `clippy::single_match`；放行后还有 `kanesumi-canvas/src/acrylic.rs:116`、`icon.rs:79/183/187` 的 `chunks_exact_to_as_chunks`，以及既有测试里的 `unused_variables` / `unused_mut`。本卡四个文件的 diff 没有 clippy 提示（`cargo clippy … 2>&1 \| grep -E "dock.rs\|dock_control.rs\|color.rs\|sizes.rs"` 无输出） |
| `cargo fmt --all -- --check` | **红（基线，非本卡）**：1490 行 `Diff in`，覆盖约 190 个文件（a11y / appmenu / canvas / controls / core / element / harness / render…）。本卡改动的 `dock.rs` / `dock_control.rs` 已用 `rustfmt --edition 2024` 单独格式化；`color.rs` / `sizes.rs` 只保留新增块，未改周边既有格式 |

红点取证：`git diff main -- kanesumi-core/src/accent.rs kanesumi-canvas/src/acrylic.rs kanesumi-canvas/src/icon.rs` 为空，
即这些告警在**未改一行**的 `main` 上就存在；本卡没有顺手修范围外文件。

## 五 · 没做的 / 与 launcher 的已知差异

1. **图标解码仍在调用方**（按卡 §二 E）。控件只给位：`layout.entry` / `slot.icon_rect` /
   `entry_icon_rect()` / `icon_jump(id)` / `icon_opacity(id)`。本卡没有读任何图标文件。
2. **入口格的点击 / 展开没有接**。控件现在能命中 `DockHit::Entry` 并画悬停底，但
   `press/release` 只产出槽位激活（`DockActivated`）；入口格的「打开 Launcher」动作
   需要调用方按 `hit()` 自己分派。卡只要求「画悬停底 + 图标位」。
3. **启动中的槽仍可命中**。launcher 在 `starting` 期间吞点击（其
   `starting_slot_breathes_and_ignores_clicks`）；卡 §二 D-6 只写了画法，未要求命中语义，
   本卡没有擅自扩大。需要时在 `MetroDock::hit` 加一条 `is_starting` 过滤即可。
4. **呼吸与轻跳共用一个 `starting_progress`**。launcher 的呼吸周期 1.0 s、轻跳周期 0.9 s
   是两个时钟；控件不自带时钟（Kanesumi 状态驱动），由调用方用 Sokuou 进度驱动。
   若必须保持两周期，调用方需按时间算两个相位，或后续给 `icon_jump` 单独加相位输入。
5. **计数格悬停没有独立状态机**。`set_hover(pos)` 会让计数格出现 10% 悬停底，
   但 `press/release` 不产出翻页事件；翻页仍由调用方读 `hit()` 的 `DockHit::Chip` 处理。
6. **滚动时右区的物理 x 可能平移**。左区从 0 滚到 1 会多出 Prev 计数格 → 板宽 +36 →
   底板重新居中 → 右区整体 x +18。这是 `DockRail` 的居中真值（不许改几何），
   右区的**槽序 / 滚动位 / 首槽**不动（用例断言的就是这三项）。
7. **为了过静态纪律，core 动了两个小口子**（卡写「全部在 dock.rs」，但门不允许）：
   - `Color::over`：`token_discipline` 禁止控件生产段出现 `Color::new`，而 10% 叠色又必须算；
     做成 core 的通用算法方法，不新增颜色令牌。
   - `sizes::DOCK_START_HOP`：`size_discipline` 禁止控件生产段新增裸尺寸常量；入口图标
     复用既有 `ICON_32`，只新增轻跳幅度 3。

## 六 · 范围外发现

1. **`cargo fmt --all -- --check` 基线红**：`main` 上约 190 个文件、1490 行 diff，主要是
   rustfmt 版本差异（本机 `rustfmt 1.10.0` / `rustc 1.99.1`）。修它需要一次全仓格式化，
   不属于 Dock 卡，本卡未做。
2. **`cargo clippy -p kanesumi-controls --all-targets -- -D warnings` 基线红**：
   `kanesumi-core/src/accent.rs:149`（`single_match`）、`kanesumi-canvas/src/acrylic.rs:116`、
   `kanesumi-canvas/src/icon.rs:79/183/187`（`chunks_exact_to_as_chunks`），以及
   `kanesumi-controls` 既有测试模块的 `unused_variables` / `unused_mut`。这些文件本卡零改动。
3. **`DockItem.urgent` 与 `active` 的旧接线错误已修**：`relayout` 曾把 `urgent` 当
   `DockRailEntry.activated`，导致「运行且激活」永远不会命中 `PrimaryHover`。本卡改为
   `it.active`（C0 真值 kitty 即此档）；`urgent` 仍只画自己的语义点。
4. **`indicator_rect` 是公开函数，签名从 `Rect` 变为 `Option<Rect>`**。本仓调用点只有
   `dock_control.rs`；Ether 侧尚未接 `MetroDock`，重派 `m-dock-c1` 时若旧代码直接解引用，
   需要改成 `if let Some(line)`。这是有意为之：溢出时被挡住的槽本来就没有线。
5. **`DockLayout.hovered` 现在会被控件回写为「幽灵过滤后的命中」**，与 `MetroDock::hit`
   同源；如果外部直接调 `DockRail::layout`，仍是原始几何命中（不含幽灵语义）。

## 七 · 下一步

1. 调度者复核本分支（`oc/k-dock-v2`），把 Kanesumi 主仓推进到本分支；
2. 改 Ether 的 `shared/kanesumi` 指针并重派 `m-dock-c1`，按 §五-2 / §五-5 在 launcher 侧
   分派入口格与计数格动作；
3. `m-dock-c1` 接线时验证：`set_show_entry(true)` 后 C0 快照的 `files_line` / `kitty_line`
   应读到 `#333333` / `#00FEFE`，`hover(files)` / `hover(kitty)` 应读到 `#2e2e2e`；
4. 若要求启动中吞点击与 0.9 s 轻跳周期，另开小卡补 `hit` 过滤与独立 hop 相位。
