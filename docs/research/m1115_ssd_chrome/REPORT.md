# m1115 · SSD 几何控件（MetroSsdChrome · Kanesumi 提供方）报告

> 卡：Ether `tools/dispatch/tasks/m1115/k-ssd-chrome.md`（判据 2 的最后一块）。
> 真值：Ether `compositor/src/wm/ssd.rs`（979 行）与 `compositor/src/config.rs:31-46`，**只读**。
> 分支 `oc/k-ssd-chrome`，基线 `main@80f215f4`，工作树 `~/.oc/wt/k-ssd-chrome`。
> 执行机：Debian GNU/Linux 13、rustc / cargo 1.97.1、`CARGO_TARGET_DIR=~/.oc/target/k-ssd-chrome`。
> **`compositor/` 一行未改**（`git -C Ether diff --stat compositor/` 为空）；只提交、不推送。

## 〇 · 结论

新增 `kanesumi-controls/src/ssd_chrome.rs`（205 行生产段 + 201 行测试，共 406 行），把 `ssd.rs` 里五处自造算术
收成 `MetroSsdChrome` 的四个方法；`lib.rs` 导出四个类型；`docs/API_SURFACE.txt` **+9 行 / −0 行**；
`docs/CONTROL_SPEC.md` 新增 §56。旧 `wm/ssd.rs` 六条数值用例逐条搬成控件侧用例（同输入同输出），
另加 6 条边界用例 —— 控件侧共 12 条。

| 门 | 结果 |
|---|---|
| `cargo test --offline -p kanesumi-controls --lib` | **1036 passed / 0 failed / 0 ignored**（基线 1024 + 12） |
| `cargo test --offline -p kanesumi-controls --test size_discipline` | **2 passed / 0 failed** |
| `cargo test --offline -p kanesumi-element --test api_surface` | **1 passed**（快照 2166 行） |
| `bash tools/supply/quality.sh`（Ether 根） | 退出 0，硬禁项全零（见 §四-4 的口径说明） |

## 一 · 数值表（值 / 出处行）

| 字段 | 值 | 出处 | 控件内使用点 |
|---|---|---|---|
| `titlebar_h` | 28 | `config.rs:31` `WIN_TITLEBAR_HEIGHT` | `geometry`：`y0 = loc.y − 28`、`outer.h += 28` |
| `btn_w` | 44 | `config.rs:33` `WIN_BTN_WIDTH` | `geometry`：三按钮从右缘倒数 |
| `btn_h` | 28 | `config.rs:34` `WIN_BTN_HEIGHT` | `geometry`：按钮高 |
| `title_pad_left` | 12 | `config.rs:36` `WIN_TITLE_PAD_LEFT` | `title_row`：`icon_x = 左缘 + px(12)` |
| `app_icon_gap` | 8 | `config.rs:40` `WIN_APP_ICON_GAP` | `title_row`：图标后间隔 |
| `app_name_gap` | 12 | `config.rs:42` `WIN_APP_NAME_GAP` | `title_row`：应用名后间隔 |
| `sep_w` | 1 | `config.rs:46` `WIN_DIVIDER` | `title_row`：`px(1).max(1)` |
| `sep_gap` | 8 | `ssd.rs:216` 的 `px(8.0)` | `title_row`：分隔线后间隔 |
| `right_pad` | 8 | `ssd.rs:205` 的 `px(8.0)` | `title_row`：右界留白 |
| `divider` | 1 | `config.rs:46` `WIN_DIVIDER` | `border_thickness` |
| `position_line` | 2 | `config.rs:44` `WIN_POSITION_LINE` | `border_thickness` |

`sep_w` 与 `divider` 在 `config.rs` 是同一个常量 `WIN_DIVIDER`，拆两个字段是因为 `title_row`
把它当线宽（`px(1).max(1)`）、`border_thickness` 把它当描边厚（`round().max(1)`），语义不同、
将来可能各自可调；取值仍逐字对齐现实现。

**不新增 `const f32`**：11 个值全部住在 `SsdChromeMetrics` 的字段缺省
（`kanesumi-controls/src/ssd_chrome.rs:34-50`），`size_discipline` 白名单未动一行（§四-2 为证）。

## 二 · 算法逐行对照（旧函数 → 新方法）

### 2.1 `ssd_rects_logical`（`wm/ssd.rs:88-120`）→ `geometry`（`ssd_chrome.rs:120-145`）

| 旧行 | 新行 | 说明 |
|---|---|---|
| `:95-97` `content_w <= 0 || content_h <= 0` → default | `:121-127` | 另挡 NaN（`ssd.rs` 入参是 i32，不存在 NaN；控件入参是 f32，不挡会流进渲染） |
| `:98-100` 三个常量 | `:128-130` | 改成读 `metrics` 字段 |
| `:101` `x0 = loc.x + geo_offset_x` | `:133` | 入参 f64，`loc.x as f64` |
| `:102` `y0 = loc.y − tb_h` | `:134` | 同 |
| `:103` `right = x0 + content_w` | `:135` | 同 |
| `:105` titlebar | `:138` | `Rect::new(x0, y0, cw, tb_h)` |
| `:107-118` close/maximize/minimize | `:140-142` | 顺序与偏移逐字相同（右缘 −1/−2/−3 个 `btn_w`） |
| 无（`ssd_rects` 里另算） | `:143` `outer` | 逻辑外框 `(x0, y0, cw, tb_h + ch)`，见 2.2 |

### 2.2 `ssd_rects`（`wm/ssd.rs:134-166`）→ `geometry().outer` + 调用方 `display::scale_rect`

旧函数做两件事：（a）算逻辑几何；（b）用 `crate::display::scale_rect` 把 titlebar / 三按钮 /
outer 换成物理。控件的 `geometry` **没有 scale 入参**（卡面签名），只给逻辑 `outer`
（`ssd_chrome.rs:143`），物理换算留给合成器 —— 这与「逻辑坐标 f64、物理量由调用方给 scale」
的口径一致，也避免把 `display::scale_rect` 的两端点取整搬进控件（它会与合成器的输出 scale
生命周期耦合）。

旧 `:149-158` 的 outer 逻辑矩形 `(x0, y0, content_w, 28 + content_h)` 在新 `:143` 逐字复现；
测试里用一份只存在于 `#[cfg(test)]` 的 `scale_rect` 复刻（`ssd_chrome.rs:213-219`），把旧
`physical_geometry_scales_at_two` 的期望值原样断言（§三）。

### 2.3 `title_row`（`wm/ssd.rs:194-222`）→ `title_row`（`ssd_chrome.rs:149-175`）

| 旧行 | 新行 | 说明 |
|---|---|---|
| `:202` `px = (pt*scale).round() as i32` | `:150` `(pt*scale).round()` | 出参 f64，不提前截成 i32 |
| `:203` `x = loc.x + px(12)` | `:151` | `titlebar.origin.x as f64` |
| `:205` `right = loc.x + w − 3*btn_w − px(8)` | `:153-155` | 同式（`right_pad` 字段） |
| `:206-209` icon | `:156-159` | `icon_w > 0.0` 同判据 |
| `:210-213` name | `:160-163` | 同 |
| `:214-220` sep | `:165-172` | `name_w > 0.0 && has_title`；`px(sep_w).max(1) + px(sep_gap)` |
| `:221` title_avail = right − x | `:172` | 同 |

### 2.4 `border_thickness`（`wm/ssd.rs:639-643`）→ `border_thickness`（`ssd_chrome.rs:177-181`）

| 旧 | 新 | 说明 |
|---|---|---|
| `(WIN_DIVIDER*scale).round() as i32` 再 `.max(1)` | `(divider*scale).round().max(1.0)` | f64 出参，`.max` 在 round 之后（顺序不变） |
| `(WIN_POSITION_LINE*scale).round() as i32` 再 `.max(1)` | `(position_line*scale).round().max(1.0)` | 同 |

### 2.5 `over`（`wm/ssd.rs:386-393`）→ `over`（`ssd_chrome.rs:189-196`）

| 旧 | 新 |
|---|---|
| `base[i] + (top[i] − base[i]) * a`（i = 0..3） | 同式，`r/g/b` 三通道 |
| 第四项常量 `1.0` | `a: 1.0`（输出恒不透明） |

输入从 `[f32; 4]` 换成 `kanesumi_core::Color`（字段同名同序）。实现用结构体字面量而不是
`Color::new`：`token_discipline` 禁止生产段出现颜色构造器，而这里消费调用方给的两个颜色、
不定义令牌（`ssd_chrome.rs:187-188` 的注释记了这条理由）。

## 三 · 测试名对照（旧 → 新）

| 旧（`compositor/src/wm/ssd.rs`） | 新（`kanesumi-controls/src/ssd_chrome.rs`） | 备注 |
|---|---|---|
| `logical_geometry_matches_spec`（`:847`） | `geometry_matches_spec` | 同输入 `(100,200), 0, 800×600`，四矩形同值 |
| `physical_geometry_scales_at_two`（`:892`） | `physical_geometry_scales_at_two` | 测试内 `scale_rect` 复刻两端点取整；titlebar/close/outer 同值 |
| `border_and_line_thickness_scales_at_two`（`:902`） | `border_and_line_thickness_scales_at_two` | `(1,2)` / `(2,4)` 同值，另加 1.5× 与 0× |
| `title_row_matches_measured_layout`（`:957`） | `title_row_matches_measured_layout` | 112 / 136 / 211 / 220 / 540 同值；无标题时 `sep=None` |
| `narrow_titlebar_leaves_no_room_for_title`（`:972`） | `narrow_titlebar_leaves_no_room_for_title` | `avail <= 0`；`fit_title` 不在本件，改断言 `title_x` 越过右界 |
| `zero_content_yields_empty_rects`（`:910`） | `no_content_yields_empty_rects` | 全 `default`；另加高 0 / 负 / NaN |
| —（新） | `default_metrics_match_compositor_config` | 11 字段逐值对 `config.rs` |
| —（新） | `geometry_applies_content_offset` | `geo_offset_x = 7` 整体右移 |
| —（新） | `title_row_without_icon_starts_name_at_icon_slot` | `icon_w = 0` 时 `name_x = icon_x` |
| —（新） | `title_row_scales_padding_at_fractional_scale` | 1.5× 逐项 `px()` 手算 |
| —（新） | `over_forces_opaque_alpha` | 0.5 / 0 / 1 三档 + 0.08 悬停比例，alpha 恒 1.0 |
| —（新） | `metrics_override_drives_geometry_and_lines` | `with_metrics` 改标题栏高 / 按钮宽高 / 定位线 |

旧 `button_at_maps_regions`、`hover_change_requests_both_button_rects` **未搬**：它们测的是
`SsdRectsLogical::button_at` 与 `hover_redraw_rects`，不在本卡点名的五个函数里（见 §五）。

## 四 · 门的全文

环境：`CARGO_TARGET_DIR=~/.oc/target/k-ssd-chrome`（会话环境自带），日志留在
`.oc/out/m1115_ssd_chrome/`（未入库）。

### 4.1 `cargo test --offline -p kanesumi-controls --lib`

```text
     Running unittests src/lib.rs (/home/rain/.oc/target/k-ssd-chrome/debug/deps/kanesumi_controls-a6092677b4812b61)

running 1036 tests
test ssd_chrome::tests::border_and_line_thickness_scales_at_two ... ok
test ssd_chrome::tests::default_metrics_match_compositor_config ... ok
test ssd_chrome::tests::geometry_applies_content_offset ... ok
test ssd_chrome::tests::geometry_matches_spec ... ok
test ssd_chrome::tests::metrics_override_drives_geometry_and_lines ... ok
test ssd_chrome::tests::narrow_titlebar_leaves_no_room_for_title ... ok
test ssd_chrome::tests::no_content_yields_empty_rects ... ok
test ssd_chrome::tests::over_forces_opaque_alpha ... ok
test ssd_chrome::tests::physical_geometry_scales_at_two ... ok
test ssd_chrome::tests::title_row_matches_measured_layout ... ok
test ssd_chrome::tests::title_row_scales_padding_at_fractional_scale ... ok
test ssd_chrome::tests::title_row_without_icon_starts_name_at_icon_slot ... ok

test result: ok. 1036 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 29.70s
```

其余 1024 行是既有用例名，完整输出 1080 行在 `.oc/out/m1115_ssd_chrome/lib.log`（工作树里的
`.oc/` 是未跟踪目录，未进本次提交；`git status` 只列一行 `?? .oc/`）。编译段有 5 条既有告警（3 条 unused `engine`、1 条 unused `primary`、1 条
不必要的 `mut`，例如 `candidate_window.rs:560`、`split_button.rs:597`，都在本件未改动的文件里），
`ssd_chrome.rs` 零告警（日志里 `ssd_chrome` 只出现在 12 条用例名中）。

### 4.2 `cargo test --offline -p kanesumi-controls --test size_discipline`

```text
   Compiling kanesumi-controls v0.1.0 (/home/rain/.oc/wt/k-ssd-chrome/kanesumi-controls)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 1.71s
     Running tests/size_discipline.rs (/home/rain/.oc/target/k-ssd-chrome/debug/deps/size_discipline-b5601526dac2d04d)

running 2 tests
test scanner_self_check ... ok
test no_new_bare_size_consts_in_production_code ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
```

### 4.3 `cargo test --offline -p kanesumi-element --test api_surface`

```text
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.13s
     Running tests/api_surface.rs (/home/rain/.oc/target/k-ssd-chrome/debug/deps/api_surface-6ae999830c1de8c0)

running 1 test
test public_api_surface_matches_snapshot ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

快照 `docs/API_SURFACE.txt` 是重新生成的：`git diff --stat` 为 **9 insertions(+)、0 deletions**，
新增行就是 `MetroSsdChrome` / 三个值对象的签名（`pub fn new() -> Self` 因去重已在快照里）。
生成用 vetted 命令 `cargo run -q --offline -p kanesumi-element --example api_surface > docs/API_SURFACE.txt`，
逐行 diff 确认只有这 9 行。

### 4.4 `bash tools/supply/quality.sh`（在 Ether 仓根，原样）

```text
== Kanesumi 质量门 2026-11-18T15:07:14+08:00 ==
rustc: 1.97.1  cargo: 1.97.1

--- 硬禁项（非零即退回）---
todo!()                             0
unimplemented!()                    0
dbg!()                              0
#[ignore]                           0

--- 不是硬禁、但每次交件要看 ---
快照 bless 入口                 2  （testing.rs:32；只许调度者复核后使用，工人不得 bless）
生产段 .unwrap()               693  （测试里允许；生产段逐处要有理由）
unsafe 块                         49  （每处要有 SAFETY 注释与审计条目）

--- 每 crate 体量（Rust 行 / 测试数 / f32 常量）---
crate                     rs行   测试 生产f32常量
kanesumi-core              5064       99      105
kanesumi-canvas           10815      133        2
kanesumi-controls         56010     1030      228
kanesumi-element          13837      146       13
kanesumi-harness          23385      236        4
kanesumi-anim               717       18        0
kanesumi-a11y               348        0        0

--- 出处密度（CONTROL_SPEC / WinUI / 参，每个控件文件）---
ime.rs                                0
lib.rs                                0
retained.rs                           0
icon_slot.rs                          1
menu_flyout.rs                        1
notification.rs                       1
panel_row.rs                          1
tooltip.rs                            1
focus.rs                              2
metro_tile.rs                         2
（列出出处最少的 10 个文件；为 0 的文件在审计前必须补来源，否则答不出「这个数怎么来的」）

--- 接口闸门（api_surface 快照）---
api_surface: ok —— 快照 2166 行

结论：硬禁项全零 = 可以交件；上面三张表是审计口径的数字，贴进报告。
```

**口径说明（重要）**：脚本把 `KAN` 硬编码成 `$ROOT/shared/kanesumi`，在 Ether 根跑时
`$ROOT` = Ether 主检出，量的是**主检出的 Kanesumi**（`e5e849ce`），不是本工作树
（`80f215f4` + 本卡改动）—— 所以上表的 controls 数字是 1030 / 228，与本工作树的 1036 / 224
不同。为了不乱读，另跑了一次**同一脚本、只把 `KAN` 指到本工作树**（`sed` 单行替换到
`/tmp/quality_wt_ssd.sh`，其余逐字未动），退出 0：

```text
== Kanesumi 质量门 2026-11-18T15:07:27+08:00 ==
rustc: 1.97.1  cargo: 1.97.1

--- 硬禁项（非零即退回）---
todo!()                             0
unimplemented!()                    0
dbg!()                              0
#[ignore]                           0

--- 不是硬禁、但每次交件要看 ---
快照 bless 入口                 2  （testing.rs:32；只许调度者复核后使用，工人不得 bless）
生产段 .unwrap()               693  （测试里允许；生产段逐处要有理由）
unsafe 块                         49  （每处要有 SAFETY 注释与审计条目）

--- 每 crate 体量（Rust 行 / 测试数 / f32 常量）---
crate                     rs行   测试 生产f32常量
kanesumi-core              5064       99      105
kanesumi-canvas           10815      133        2
kanesumi-controls         55800     1036      224
kanesumi-element          13837      146       13
kanesumi-harness          23385      236        4
kanesumi-anim               717       18        0
kanesumi-a11y               348        0        0

--- 出处密度（CONTROL_SPEC / WinUI / 参，每个控件文件）---
ime.rs                                0
lib.rs                                0
retained.rs                           0
icon_slot.rs                          1
menu_flyout.rs                        1
notification.rs                       1
panel_row.rs                          1
tooltip.rs                            1
focus.rs                              2
metro_tile.rs                         2
（列出出处最少的 10 个文件；为 0 的文件在审计前必须补来源，否则答不出「这个数怎么来的」）

--- 接口闸门（api_surface 快照）---
api_surface: ok —— 快照 2166 行

结论：硬禁项全零 = 可以交件；上面三张表是审计口径的数字，贴进报告。
```

工作树口径：`kanesumi-controls` 1036 tests（基线 1024 + 12）、生产段 `const f32` 224 条
（基线同值 —— 本件**零新增**，`ssd_chrome.rs` 没有 `const`）。两跑硬禁项都是 0。

## 五 · 没做的 / 没验证的

1. **合成器未接线**（卡面明令 `compositor/` 一行不许改）：`wm/ssd.rs` 仍在用旧的五个函数。
   「新旧算术等价」的证据是 §二 的逐行对照 + §三 的 12 条同输入同输出用例，**不是**真实输出
   对拍 —— 1× / 1.25× / 1.5× / 2× 下合成器实际像素与新控件的一致性**还没验证**，要靠接线那
   一卡做像素断言或截图。这条属「论证过、未实测」，别当成已证。
2. **没有物理几何 API**：`geometry` 按卡面签名不带 `scale`，`outer` 是**逻辑**矩形；物理外框
   仍由调用方走 `display::scale_rect` 的两端点取整（`ssd.rs:149-158`）。若接线时误把
   `SsdGeometry` 当物理值直接用，2× 下会小一半 —— 这是签名决定的，已写进 §56.3。
3. **f32 / f64 口径有一处与卡面措辞不同**：卡面说「逻辑坐标是 f64，出参用 f64」，但
   `SsdGeometry` 的字段类型是 `kanesumi_core::Rect`，其字段是 f32（`geometry.rs:82-85`）。
   控件的**计算**走 f64（`ssd_chrome.rs:128-143`），落到 `Rect` 时转 f32；`title_row` /
   `border_thickness` / `geo_offset_x` 是纯 f64 出参。逻辑坐标是像素级整数 / 半整数，落在
   f32 的 24 位精确整数范围内，实际不丢精度，但这是**推理**，没有对超大坐标做过测试。
4. **`SsdGeometry::default` 是手写的**：`Rect` 没有 `Default`（core 只给 `Size::ZERO`），卡面
   要 `derive(Default)` 而本卡不改 core，故手写零矩形（`ssd_chrome.rs:67-78`）。语义等价，
   但若以后 core 给 `Rect` 加了 `Default`，这里可以改回 derive。
5. **命中与损伤没搬**：旧 `ssd.rs` 的 `button_at`（`:36-48`）与 `hover_redraw_rects`
   （`:73-84`）不在本卡点名的五个函数里，本件不做。要做「三块自绘同一元件层」的完整命中/
   损伤，得另开一件（建议沿用 `Rect::contains` 的半开区间语义）。
6. **主题色没搬**：`over` 只做逐通道算术，`titlebar` / `title_text` / `error_fill` /
   `divider` / `accent` 这些令牌仍在 `compositor/src/theme.rs`。控件只消费调用方给的
   `Color`，不引主题、不定义令牌（也正因如此 `token_discipline` 不需要例外）。
7. **没跑 workspace 全量测试**：本卡门只点名三个目标；且 Ether 主检出的 Kanesumi 领先本树
   基线两个提交（§六-3），全量跑会把别人的改动算进来。`cargo clippy -p kanesumi-controls
   --all-targets` 跑了（非门）：退出 0，`ssd_chrome.rs` 零告警。
8. **没跑 `cargo fmt`**：仓里既有签名有 >100 列的一行式（`cargo fmt` 基线本来就红），本件
   沿用同风格，没有格式化范围外文件。

## 六 · 范围外发现

1. **新工作树的嵌套子模块未初始化**：`assets/kanesumi-assets` 是子模块，worktree 建出来时是
   空目录，`cargo test` 直接红在 `include_str!("../../assets/…/warning.svg")`（16 个 error）。
   用 `git -c protocol.file.allow=always submodule update --init assets/kanesumi-assets` 从本机
   主检出的本地路径初始化（未联网、未改任何被跟踪文件，`git status` 干净）。后续每个新
   worktree 都会撞这一下，建议建树脚本带上这一步。
2. **`tools/supply/quality.sh` 的 `KAN` 硬编码**：`KAN=$ROOT/shared/kanesumi`，在 Ether 根跑
   永远量主检出，量不到任何 `oc/*` 工作树（§4.4 已用两跑对照说明）。建议脚本允许
   `KAN=${KAN:-$ROOT/shared/kanesumi}` 覆盖 —— 但那是 Ether 仓的文件，不在本卡范围，未改。
3. **主检出领先基线**：本工作树基线 `80f215f4`（面板部件四件），Ether 主检出的
   `shared/kanesumi` 在 `e5e849ce`（通知卡两提交）。合入时需 rebase；两边
   `docs/API_SURFACE.txt` 都是 2166 行属巧合（内容不同，主检出的 controls 是 1030 tests /
   228 个 f32 常量，本树 1036 / 224）。
4. **`title_row` 的两处 `px(8.0)` 没有 config 常量**：右留白（`ssd.rs:205`）与分隔后间隔
   （`:216`）都是裸 `8.0`。本件拆成 `right_pad` / `sep_gap` 两个字段并在注释里记了出处行；
   若将来 `config.rs` 补常量，应回来把字段缺省改成引用。
5. **`ssd.rs` 的 `border_thickness` 用 i32 返回、`.max(1)` 在整数域**：本件返回 f64 并
   `.max(1.0)`。对 `scale ≥ 0` 两者逐值相同（`1.5 → 2`、`2.0 → 4`、`0 → 1`），负数倍率下也
   都是 1；但 f64 出参意味着调用方要自己决定 `as i32` 的落点 —— 接线时别在 round 之外再取整。


