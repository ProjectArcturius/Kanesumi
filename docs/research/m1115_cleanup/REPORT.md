# m1115_cleanup 报告：删重复通知卡 + 「生产代码不得在测试模块之后」检查

> 卡：Kanesumi 审计级收尾（k-cleanup），分支 `oc/k-cleanup`，基线 `main@6f0e1705`。
> 工作树 `~/.oc/wt/k-cleanup`；执行机 Debian GNU/Linux 13、rustc/cargo 1.97.1，
> `CARGO_TARGET_DIR=~/.oc/target/k-cleanup`。**只改本 worktree**；已在 `oc/k-cleanup` 提交，未推送。

## 〇 · 结论

三件都做完了：删掉旧量测版通知卡（文件内生产段 285 行 + 同文件旧测试 120 行，另删 279 行的
独立测试文件），新增 `item_order` 静态检查（扫全部 13 个 `kanesumi-*` crate 的 `src/`，共 202 个
`.rs`），并按检查结果搬正 3 处测试模块位置。
`cargo test -p kanesumi-controls` 由删前 **1160 passed** 变为 **1143 passed / 0 failed**
（−19 条被删测试，+2 条新检查）；`size_discipline` / `token_discipline` / `item_order` /
`api_surface` / `quality.sh` 全绿，硬禁项全零。§四 的全部门命令在 2026-11-18T15:47–15:52
逐条复跑，数字与下表一致。

| # | 交付物 | 状态 |
|---|---|---|
| 1 | `notification_card.rs` 只留 `MetroNotificationCard` 一份实现 | 完成 |
| 1 | `tests/notification_card_render.rs` 删除（9 条测试） | 完成 |
| 1 | `docs/API_SURFACE.txt` 重新生成（−8 行） | 完成 |
| 2 | `kanesumi-controls/tests/item_order.rs`（2 条测试，含反向自检） | 完成 |
| 3 | 3 处违规的测试模块搬到文件末尾（只搬位置） | 完成 |
| 4 | 本报告 | 完成 |

## 一 · 删掉重复实现

### 1.1 删了什么

`kanesumi-controls/src/notification_card.rs`：**1029 → 635 行**，`git diff --numstat` 为
`+15 / −409`（+15 是文件头注释与测试段注释的重写）。409 个删除行 = 生产段 **285 行**
（HEAD:1–173 旧实现与文件头 + HEAD:918–1029 旧渲染段）+ 同文件旧测试 **120 行**
（HEAD:546–916 的测试模块里只保留 `migrate_*` 6 条，即现 385–635）+ 4 行被改写的小节注释 / 空行；
净减 394 行。

HEAD 版被删的名字与位置（行号指 `HEAD:kanesumi-controls/src/notification_card.rs`）：

| 名字 | HEAD 行 | 说明 |
|---|---|---|
| `CARD_BODY_MAX_LINES` | 21–22 | 旧卡正文两行上限；新卡用 `NOTIFY_BODY_MAX_LINES = 3`，故删 |
| `CARD_CLOSE` | 23–24 | 旧卡 32 px 关闭钮；新卡用 `NOTIFY_CLOSE_SIZE = 18`，故删 |
| `title_style` / `body_style` | 26–34 | 只被旧卡与旧测试调用 |
| `CardGeometry` | 36–173 | 含 `layout` / `height_for` / `action_at` / `hits_close` 与 `text_left` / `text_width` |
| `CardContent` | 920–930 | 旧渲染入口的内容结构 |
| `CardVisual` | 932–941 | 旧渲染的悬停态 |
| `render_card` | 943–1029 | 旧渲染函数 |
| 同文件旧测试 | 551–668 | 10 条只测 `CardGeometry` 的测试 + `long_cjk` / `geo` 辅助 |

`contains(r, p)` 保留：新卡 `hit()` 仍在用（`notification_card.rs:17-19` 定义、`:235` / `:238` / `:241` 调用）。

`kanesumi-controls/tests/notification_card_render.rs`：**279 行 / 9 条测试**，整体删除（只测被删的旧实现）。

`kanesumi-controls/src/lib.rs`：**无需改动** —— 它在 158–160 行只重导出了
`MetroNotificationCard / NotificationContent / NotifyCardGeometry / NotifyHit`，旧名字从未在上层导出。
（任务书要求「lib.rs 的相应导出同步删」，核对后确认本来就没有。）

`docs/API_SURFACE.txt`：重新生成后删 8 行 —— `action_at` / `height_for` / `hits_close` /
`layout(` / `render_card` / `CardContent` / `CardGeometry` / `CardVisual`。`api_surface` 测试绿。

`docs/CONTROL_SPEC.md` §56 的旁注原本写「同文件里量测版 `CardGeometry` / `render_card` … 两代合流」，
删后已成假话；改成「已在 m1115_cleanup 整块删除」并指向本报告（4 增 / 3 删）。

### 1.2 为什么确定没人用（grep 证据）

在**本仓**与**本机 Ether monorepo 检出**里按词边界搜六个名字（排除定义文件与旧测试文件）：

```text
$ grep -rnE "\b(CardGeometry|CardContent|CardVisual|render_card|CARD_BODY_MAX_LINES|CARD_CLOSE)\b" \
    /home/rain/projects/Arcturius/Ether --include=*.rs \
  | grep -v kanesumi-controls/src/notification_card.rs \
  | grep -v kanesumi-controls/tests/notification_card_render.rs
（无输出）

$ grep -rnE "\b(CardGeometry|CardContent|CardVisual|render_card|CARD_BODY_MAX_LINES|CARD_CLOSE)\b" \
    --include=*.rs . | grep -v kanesumi-controls/src/notification_card.rs
（无输出）
```

删前同一组名字在旧定义文件里命中 30 行 / 32 处（`CardGeometry` 12、`CARD_BODY_MAX_LINES` 7、
`CARD_CLOSE` 6、`render_card` 3、`CardContent` 2、`CardVisual` 2），除此之外**零引用**；`card.rs` / `notification.rs` /
`shell` / gallery 均未使用。`notification_card` 虽是 `pub mod`（外部可按路径导入），但 Ether 检出里
没有任何调用点。

**分级**：仓内 + 本机 Ether 检出范围内「无人引用」是**已验证**（上面命令）；仓外消费者本机看不到，
属**无法替你验证**。

## 二 · 新检查：`kanesumi-controls/tests/item_order.rs`

- **范围**：工作区根下所有 `kanesumi-*/src/**/*.rs`（202 个文件、13 个 crate），不只是任务书点名的
  三个 crate —— 因为 harness 也有一处同类违规，只扫三个会漏（见 §2.2）。
- **规则**：找到首个 `#[cfg(test)] mod … { … }`，配对到闭合花括号；闭合之后只允许空白、注释与
  其它 `#[cfg(test)] mod`。出现任何其它内容即红，打印 `文件:行 与首个违规项的名字`，并在报错里
  写死修法（测试模块整块挪到文件末尾）。
- **手写配对**：不引依赖。逐字符跳过 `//` / 嵌套 `/* */` / 普通字符串 / 字符字面量（`'{'`）/
  raw string（`r#"…"#` / `br#"…"#`），保留换行以便行号准确。
- **一个必要的细化**：本工作区有 14 处 `#[cfg(test)]` 标注的是**函数 / 字段**（如
  `kanesumi-core/src/color.rs:159`、`kanesumi-canvas/src/text.rs:599`），不是测试模块。检查器会跳过
  这些，继续找真正修饰 `mod` 的那一个；否则会把后面的普通 `mod` 误当成测试段起点。
- **反向自检**（`checker_detects_violation_and_accepts_compliant_layout`）：合成源码覆盖
  ①测试模块后接 `pub struct`（报 `struct ListRowMeta`、行号指向属性行）；②合规布局（多个测试模块
  连续放末尾、尾注释）；③字面量 / 块注释里的花括号不干扰配对；④`#[cfg(test)] fn` 被跳过。

### 2.1 违规清单（检查器实际输出，HEAD 版三文件）

为留证，临时把三个文件还原到 HEAD、跑检查、再按 sha256 还原：

```text
生产代码写在测试模块之后（3 处）。这些行永远不会被 size_discipline / token_discipline 扫到：
  kanesumi-controls/src/list.rs:1301  struct ListRowMeta
  kanesumi-element/src/event.rs:69  enum Key
  kanesumi-harness/src/session_lock.rs:1122  impl wayland_client::Dispatch<crate::platform::layers::proto::ether_backdrop_v1:
修法：把该文件里的 #[cfg(test)] 模块整块挪到文件末尾（只搬位置，不改测试内容，不改生产代码）；多个测试模块按原顺序全部放在末尾。
```

还原校验：`0874a444…` → `0874a444…`（RESTORED-OK）。

历史版本 `HEAD:kanesumi-controls/src/notification_card.rs` 也能被抓到（这就是本次任务的起因）：

```text
  kanesumi-controls/src/notification_card.rs:921  struct CardContent
```

### 2.2 逐条修法（只搬位置）

| 文件 | 违规项 | 搬法 | `git diff --numstat` |
|---|---|---|---|
| `kanesumi-controls/src/list.rs` | `ListRowMeta` 及 `impl`（HEAD:1293–1353） | 生产段上移到 `#[cfg(test)] mod tree_tests` 之前，两个测试模块留在末尾 | `+61 / −61`（纯移动，行多重集逐一相同） |
| `kanesumi-element/src/event.rs` | `enum Key` 起的生产段 | `mod pointer_button_tests` 整块移到文件末尾 | `+22 / −22` |
| `kanesumi-harness/src/session_lock.rs` | `impl Dispatch<…> for LockShell` 等 | `mod tests`（连同小节注释）移到文件末尾 | `+47 / −46`（多的 1 行是块间空行） |

`list.rs` / `session_lock.rs` 因搬移各多 1 行空行（1352→1353、1180→1181），`event.rs` 行数不变（287）。
三份文件的断言、测试函数体、生产代码一字未改：搬移用脚本切段 + 段 sha256 校验完成，`git diff`
去掉空行后「删除行集合 == 新增行集合」。

## 三 · 用例数：删前 / 删后

同一台机、同一 `CARGO_TARGET_DIR`，`cargo test --offline -p kanesumi-controls`：

| 目标 | 删前（HEAD） | 删后 |
|---|---:|---:|
| `unittests src/lib.rs` | 1042 | **1032** |
| `tests/deprecated_names.rs` | 3 | 3 |
| `tests/dock_api_compat.rs` | 2 | 2 |
| `tests/dock_control.rs` | 27 | 27 |
| `tests/form_grammar.rs` | 9 | 9 |
| `tests/idioms_enforce.rs` | 7 | 7 |
| `tests/item_order.rs` | — | **2** |
| `tests/list_row_model.rs` | 8 | 8 |
| `tests/notification_card_render.rs` | 9 | —（文件删除） |
| `tests/osd.rs` | 11 | 11 |
| `tests/panel_row.rs` | 12 | 12 |
| `tests/size_discipline.rs` | 2 | 2 |
| `tests/table_row.rs` | 11 | 11 |
| `tests/textbox_vcenter.rs` | 1 | 1 |
| `tests/tile_wall_control.rs` | 14 | 14 |
| `tests/token_discipline.rs` | 2 | 2 |
| **合计** | **1160 passed / 0 failed** | **1143 passed / 0 failed** |
| doctest | 1 passed / 4 ignored | 1 passed / 4 ignored |

差额 −17 = 删掉同文件旧测试 10 条 + 删掉独立测试文件 9 条 + 新增检查 2 条。

**搬测试模块前后同数同绿**（硬约束）：
- controls 库测试：删前 1042 − 被删旧卡测试 10 = **1032**，三处搬移后实测 **1032** → 搬移净增减 0；
  `list_row_model` 8 条前后不变，`list::tests` / `list::tree_tests` 仍在（见 §四日志）。
- element / harness 的搬移复跑（证明测试仍在且通过）：

```text
$ cargo test --offline -p kanesumi-element --lib event::
running 3 tests
test event::pointer_button_tests::evdev_buttons_preserve_navigation_and_unknown_identity ... ok
test event::tests::end_has_no_delta_and_end_phase ... ok
test event::tests::wheel_scroll_input_counts_steps ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 143 filtered out; finished in 0.00s

$ cargo test --offline -p kanesumi-harness --lib session_lock
running 4 tests
test session_lock::tests::finished_exits_without_unlock ... ok
test session_lock::tests::unlock_only_after_locked_and_request ... ok
test session_lock::tests::no_render_before_locked ... ok
test session_lock::tests::no_exit_before_locked ... ok
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 231 filtered out; finished in 0.00s
```

## 四 · 门（全文）

> `cargo test` 的逐条 `test … ok` 输出有 3000+ 行，报告只贴每条命令的 `Running` / `test result` /
> doctest 汇总行（失败数、用例数都在；逐条名单不重复粘贴）。以下命令均在本 worktree 根执行，
> 退出码全部为 0。

### 1) `cargo test --offline -p kanesumi-controls`（EXIT=0）

```text
     Running unittests src/lib.rs (/home/rain/.oc/target/k-cleanup/debug/deps/kanesumi_controls-a6092677b4812b61)
test result: ok. 1032 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 25.63s
     Running tests/deprecated_names.rs (/home/rain/.oc/target/k-cleanup/debug/deps/deprecated_names-1c970ba4acc97ea0)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.54s
     Running tests/dock_api_compat.rs (/home/rain/.oc/target/k-cleanup/debug/deps/dock_api_compat-bc5dafbec3fcc3f9)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.50s
     Running tests/dock_control.rs (/home/rain/.oc/target/k-cleanup/debug/deps/dock_control-20ef04d0827d487a)
test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 2.44s
     Running tests/form_grammar.rs (/home/rain/.oc/target/k-cleanup/debug/deps/form_grammar-8fd4a5baa8380bab)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.76s
     Running tests/idioms_enforce.rs (/home/rain/.oc/target/k-cleanup/debug/deps/idioms_enforce-e5a0ce828eb1ede5)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.02s
     Running tests/item_order.rs (/home/rain/.oc/target/k-cleanup/debug/deps/item_order-132b24c9c3be85da)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.66s
     Running tests/list_row_model.rs (/home/rain/.oc/target/k-cleanup/debug/deps/list_row_model-a038e1206e417f3c)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.56s
     Running tests/osd.rs (/home/rain/.oc/target/k-cleanup/debug/deps/osd-decca06d65f26fe0)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.03s
     Running tests/panel_row.rs (/home/rain/.oc/target/k-cleanup/debug/deps/panel_row-b2f01c9f1acb4272)
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.44s
     Running tests/size_discipline.rs (/home/rain/.oc/target/k-cleanup/debug/deps/size_discipline-b5601526dac2d04d)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.05s
     Running tests/table_row.rs (/home/rain/.oc/target/k-cleanup/debug/deps/table_row-229f81ef0ee7fa0f)
test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.61s
     Running tests/textbox_vcenter.rs (/home/rain/.oc/target/k-cleanup/debug/deps/textbox_vcenter-1b0cb1725c73e8c2)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 24.25s
     Running tests/tile_wall_control.rs (/home/rain/.oc/target/k-cleanup/debug/deps/tile_wall_control-95bce36aadba0abc)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.49s
     Running tests/token_discipline.rs (/home/rain/.oc/target/k-cleanup/debug/deps/token_discipline-8ea8c643152c565f)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
   Doc-tests kanesumi_controls
test result: ok. 1 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### 2) `cargo test --offline -p kanesumi-controls --test size_discipline`（EXIT=0）

```text
     Running tests/size_discipline.rs (/home/rain/.oc/target/k-cleanup/debug/deps/size_discipline-b5601526dac2d04d)

running 2 tests
test scanner_self_check ... ok
test no_new_bare_size_consts_in_production_code ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

### 3) `cargo test --offline -p kanesumi-controls --test token_discipline`（EXIT=0）

```text
     Running tests/token_discipline.rs (/home/rain/.oc/target/k-cleanup/debug/deps/token_discipline-8ea8c643152c565f)

running 2 tests
test checker_detects_violations_and_respects_test_sections ... ok
test no_inline_colors_in_production_code ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.09s
```

### 4) `cargo test --offline -p kanesumi-controls --test item_order`（EXIT=0）

```text
     Running tests/item_order.rs (/home/rain/.oc/target/k-cleanup/debug/deps/item_order-132b24c9c3be85da)

running 2 tests
test checker_detects_violation_and_accepts_compliant_layout ... ok
test no_production_code_after_test_modules ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.65s
```

### 5) `cargo test --offline -p kanesumi-element --test api_surface`（EXIT=0）

```text
     Running tests/api_surface.rs (/home/rain/.oc/target/k-cleanup/debug/deps/api_surface-6ae999830c1de8c0)

running 1 test
test public_api_surface_matches_snapshot ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.12s
```

### 6) `bash tools/supply/quality.sh`（EXIT=0）

> 本 worktree 没有 `tools/supply/quality.sh`（Kanesumi 单仓检出）。这里用 Ether 那份脚本原样
> 拷贝进来、并把 `shared/kanesumi` 软链到本树根，让 `ROOT/shared/kanesumi` 指向**本次改动**；
> 跑完即删（见 §五-2）。脚本内容未改一字。

```text
== Kanesumi 质量门 2026-11-18T15:50:16+08:00 ==
rustc: 1.97.1  cargo: 1.97.1

--- 硬禁项（非零即退回）---
todo!()                             0
unimplemented!()                    0
dbg!()                              0
#[ignore]                           0

--- 不是硬禁、但每次交件要看 ---
快照 bless 入口                 2  （testing.rs:32；只许调度者复核后使用，工人不得 bless）
生产段 .unwrap()               692  （测试里允许；生产段逐处要有理由）
unsafe 块                         52  （每处要有 SAFETY 注释与审计条目）

--- 每 crate 体量（Rust 行 / 测试数 / f32 常量）---
crate                     rs行   测试 生产f32常量
kanesumi-core              5064       99      105
kanesumi-canvas           10815      133        2
kanesumi-controls         56025     1032      227
kanesumi-element          13870      146       13
kanesumi-harness          23443      237        4
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
api_surface: ok —— 快照 2167 行

结论：硬禁项全零 = 可以交件；上面三张表是审计口径的数字，贴进报告。
```

## 五 · 未完成 / 不确定

1. **改动已提交在 `oc/k-cleanup`，未推送**（工人不推送）。两笔：`test(item_order): …`（检查 +
   三处搬移）与 `refactor(notification_card): …`（删卡 + API_SURFACE + CONTROL_SPEC + 本报告）。
   工作树内容：6 个修改 + 1 个删除 + 2 个新增（`item_order.rs` 与本报告），提交后应为 clean。
2. **`quality.sh` 的字面路径在本 worktree 不存在**。本 worktree 是 Kanesumi 单仓检出
   （`git worktree list` 显示 main 在 `Ether/.git/modules/shared/kanesumi`），没有 `tools/` 也没有
   `shared/kanesumi`。§四-6 的做法是临时铺 Ether 布局取得**本树**证据；另按字面在
   `/home/rain/projects/Arcturius/Ether` 跑过一次，但它扫的是 main 的 `shared/kanesumi`
   （controls 测试数 1042，不是本树的 1032），**不能**当成本次证据。
3. **`assets/kanesumi-assets` 子模块未初始化**：第一次 `cargo test` 直接失败在
   `couldn't read …/glyphs/search.svg`。我从同仓另一个 worktree（`~/.oc/wt/k-driver-theme`）把同一
   子模块内容复制进本树的空目录后编译通过；这是构建环境修复，不是代码改动，`git status` 无变化
   （子模块是 gitlink）。换机器复跑需要 `git submodule update --init`。
4. **行为验证分级**：`MetroNotificationCard` 的代码只动了文件头/小节注释，6 条 `migrate_*` 测试
   原样通过 —— 这是**已验证**。旧实现本就没有消费方，删它不改变任何调用路径。**没有做像素级
   验证**（本任务不触碰渲染路径）；真实合成器里的肉眼外观属**无法替你验证**。
5. **仓外消费者**：本机 Ether 检出里 grep 为零；不在这台机器上的下游无法验证。
6. `item_order` 只覆盖「测试**模块**」；`#[cfg(test)]` 标在函数/字段上的写法仍会让
   `size_discipline` / `token_discipline` 提前截断（见下）。这是同类审计洞，但不在本卡范围。

## 六 · 范围外发现

1. **`production_lines` 的终止条件仍是「首个 `#[cfg(test)]`」**，而 `#[cfg(test)]` 也标注函数/字段：
   `kanesumi-canvas/src/text.rs:599`、`kanesumi-core/src/color.rs:159`、
   `kanesumi-element/src/diagnostics.rs:50`、`kanesumi-harness/src/dmabuf.rs:253` 等共 14 处。
   这些文件里，该行之后的生产代码不会被两道纪律检查扫到。controls 内没有这种写法，故本次没有
   现成漏洞；建议下一卡把终止条件收窄为「首个 `#[cfg(test)] mod`」，或让 `item_order` 一并
   报告「测试属性标注的非模块项」。
2. `list.rs` / `event.rs` 在 HEAD 就有 rustfmt 漂移（`rustfmt --check` 对 HEAD 版即 DIFF）。本卡只搬
   位置，没有顺手格式化，避免把 100+ 行格式噪音混进 diff。
3. `docs/research/m1115_notify/REPORT.md:114–116` 仍写着 `--test notification_card_render`；该文件已删。
   历史报告不改写，留此备注。
4. harness 也有一处同类违规（`session_lock.rs`），任务书只点名三个 crate；因为新检查扫全工作区，
   这处一并修了（§二-2）。

## 七 · 需要你做的 / 下一步

- **审阅两笔提交**（已在 `oc/k-cleanup`，`git log main..HEAD`）：
  `test(item_order): 生产代码不得写在测试模块之后`（检查 + 三处搬移）与
  `refactor(notification_card): 删除量测版通知卡，只留 MetroNotificationCard`
  （删卡 + 删独立测试 + API_SURFACE + CONTROL_SPEC + 本报告）。
- **决定是否另开卡**收口 §六-1 的 `#[cfg(test)]` 非模块项（会改两道既有检查的扫描边界，需单独评审）。
- **推送由调度者做**（工人不推送）。

下一步：调度者审阅两笔提交后推送；若要复核，在 clean 环境 `git submodule update --init` 后复跑
本报告 §四 的 1) 与 5)，确认 1032 / 1143 与 `api_surface: ok`。


