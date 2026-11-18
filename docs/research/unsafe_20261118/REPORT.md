# unsafe 审计前置报告（2026-11-18）

> 卡：给 49 处 `unsafe` 补 SAFETY 依据。分支 `oc/k-unsafe-audit`，工作树
> `~/.oc/wt/k-unsafe-audit`，执行机本机（rustc 1.97.1 / cargo 1.97.1）。
> 只加注释 + `docs/UNSAFE_AUDIT.md`；**未改任何行为**。改动已在 `oc/k-unsafe-audit`
> 提交，未推送。

## 〇 · 结论

- **45 个 unsafe 代码位置全部有依据**：生产 39 处（逐处紧贴 `// SAFETY:`），
  集成测试 6 处（按任务要求在 `tests/font_selection_regression.rs` 文件头统一说明，
  原有逐处 SAFETY 保留）。
- `SAFETY` 注释行 **14 → 51**；`unsafe` 命中行 50 → 53（新增文字里 3 行引用了
  「unsafe」这个词，**代码块 0 增 0 删**，见 `docs/UNSAFE_AUDIT.md` §四）。
- 行为零改动：`git diff -U0 -- '*.rs'` 过滤掉注释行后，非注释差异 **0 行**。
- **疑点 5 组**（4 组测试内改环境变量 + 1 组生产表面 drop 顺序），只记录未改逻辑，
  详见 `docs/UNSAFE_AUDIT.md` §三与本报告 §三。

## 一 · 改了什么

按文件（`SAFETY` 行数为改后；括号内为实际 unsafe 代码位置）：

| 文件 | 改动 |
|---|---|
| `kanesumi-harness/src/platform.rs` | 10 处：`transmute` 提升 `'static`、裸指针重建借用、xkb keymap fd、wl_shm mmap、6 处测试 env 改动 |
| `kanesumi-element/src/alloc_budget.rs` | 9 处：`GlobalAlloc` impl + 4 个 `unsafe fn` + 4 次 `System` 转发 |
| `kanesumi-render/src/canvas_v2.rs` | 4 处：wgpu `create_pipeline_cache`、3 处测试 env 改动 |
| `kanesumi-harness/src/dmabuf.rs` | 3 处：`setrlimit`、`sigaction`+`raise`、`abort`（仅探测子进程） |
| `kanesumi-harness/src/role.rs` | 3 处：测试内改 `ETHER_ROLE` |
| `kanesumi-appmenu/src/install.rs` | 3 处：`pub unsafe fn` 契约、`from_foreign_display`、`ObjectId::from_ptr` |
| `kanesumi-harness/src/platform/font_mmap.rs` | 1 处：`Mmap::map` 只读字体映射（原有 SAFETY 已充分，补不变式） |
| `kanesumi-harness/src/render.rs` | 1 处：`create_surface_unsafe` |
| `kanesumi-harness/src/system_theme.rs` | 3 处：测试内改 `ETHER_THEME_CONFIG`（原有 SAFETY，补残余竞争说明） |
| `kanesumi-harness/src/timeline.rs` | 1 处：`clock_gettime` |
| `kanesumi-harness/src/frame_clock.rs` | 1 处：`clock_gettime` |
| `kanesumi-harness/tests/font_selection_regression.rs` | 6 处：文件头统一说明（原有逐处 SAFETY 未动） |
| `kanesumi-element/src/strict.rs`、`diagnostics.rs` | 各 1 句「为何不需要 unsafe」；两文件无 unsafe 块（散文命中） |

新增两份文档：

- `docs/UNSAFE_AUDIT.md`：逐处表（文件:行 / 在做什么 / 不变量 / 结论）+ 统计 + 疑点。
- `docs/research/unsafe_20261118/REPORT.md`：本报告。

## 二 · 验证

### 2.1 四道 lib 测试（全绿）

```text
$ cargo test --offline -p kanesumi-harness --lib
test result: ok. 235 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline -p kanesumi-element --lib
test result: ok. 146 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline -p kanesumi-render --lib
test result: ok. 52 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo test --offline -p kanesumi-appmenu --lib
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### 2.2 行为零改动

```text
$ git diff -U0 -- '*.rs' | grep -E '^[+-]' | grep -vE '^(\+\+\+|---)' \
    | grep -vE '^[+-][[:space:]]*(//|$)' | wc -l
0
```

（`assets/kanesumi-assets` 的本地指针修复不在 `*.rs` 内；见 §五。）

### 2.3 计数（前 / 后）

```text
$ git grep -n "SAFETY" HEAD -- '*.rs' | wc -l      # 前
14
$ grep -rn "SAFETY" --include=*.rs . | wc -l        # 后（本 worktree）
51
```

`unsafe` 命中行：前 50（44 src + 6 tests），后 53。按质量门的 `\bunsafe\b` 口径：
前 49、后 52；差出的 3 行都是新增注释里的词，不是新代码块。

### 2.4 `bash tools/supply/quality.sh`

**在 Ether 仓根原样跑**（脚本把 `KAN` 写死为 `$ROOT/shared/kanesumi`，即主检出）：

```text
== Kanesumi 质量门 2026-11-18T15:08:54+08:00 ==
rustc: 1.97.1  cargo: 1.97.1

--- 硬禁项（非零即退回）---
todo!()                             0
unimplemented!()                    0
dbg!()                              0
#[ignore]                           0

--- 不是硬禁、但每次交件要看 ---
快照 bless 入口                 2
生产段 .unwrap()               693
unsafe 块                         49

--- 接口闸门（api_surface 快照）---
api_surface: ok —— 快照 2166 行

结论：硬禁项全零 = 可以交件；上面三张表是审计口径的数字，贴进报告。
```

**注意**：这次跑的是 **Ether 主检出**（旧提交），所以 `unsafe 块 49` 是**改动前**的数字，
它看不到本 worktree 的注释。为拿到改动后的同一口径，我用同一脚本 + 临时
`shared/kanesumi -> ..` 指回本 worktree 再跑了一次：

```text
== Kanesumi 质量门 2026-11-18T15:09:18+08:00 ==
rustc: 1.97.1  cargo: 1.97.1

--- 硬禁项（非零即退回）---
todo!()                             0
unimplemented!()                    0
dbg!()                              0
#[ignore]                           0

--- 不是硬禁、但每次交件要看 ---
快照 bless 入口                 2
生产段 .unwrap()               693
unsafe 块                         52

--- 接口闸门（api_surface 快照）---
api_surface: ok —— 快照 2166 行

结论：硬禁项全零 = 可以交件；上面三张表是审计口径的数字，贴进报告。
```

两次都是硬禁项全零、`api_surface: ok`；`unsafe 块` 49 → 52 的差值是注释里的词（§2.3）。

## 三 · 疑点（5 组，均未改逻辑）

1. **测试内改环境变量（4 组，同一类）**：`platform.rs:5939-5981`、`canvas_v2.rs:2592-2602`、
   `role.rs:201-211`、`system_theme.rs:273-293`。`cargo test` 默认多线程，Rust 2024 把
   `env::set_var` 标 unsafe 是因为它与任意线程的 `env::var` 并发即构成 libc `environ`
   数据竞争。四处都没有**进程级** env 锁；`system_theme` 的 `PIN_TEST_LOCK` 只串行本模块
   三个用例。建议：加一把进程级 env 锁（所有读写 env 的测试都拿），或把 env 用例移进
   独立 `tests/` 二进制。本卡只记录，避免把「只加注释」扩成逻辑改动。
2. **生产表面 drop 顺序（1 处）**：`render.rs:392` 的 `create_surface_unsafe` 要求 raw
   handle 活到返回的 `Surface` drop 之后；`platform.rs` 的 `Shell` 把
   `surface: wl_surface::WlSurface` 声明在 `renderer: Option<SurfaceRenderer>` 之前，
   按字段声明序先 drop surface。当前 wgpu Wayland 析构大概率不碰 wl_surface，所以
   「碰巧不炸」，但契约上没有保证。建议：把 `renderer` 调到 `surface` 之前，或给
   `Shell` 写 `Drop` 先 `take()` 渲染器。**这是唯一涉及生产路径的疑点，建议单开一张卡。**

其余位置（transmute、keymap fd、shm mmap、GlobalAlloc、sigaction/abort、字体只读映射、
appmenu 外部句柄、clock_gettime）逐条核对后判「安全」，依据见 `docs/UNSAFE_AUDIT.md`。

## 四 · 未完成 / 不确定

- 没有在真 Wayland 会话里跑 Gallery / 目视验证：本卡只改注释，四道 lib 测试已覆盖编译与
  测试路径；GUI 行为不在本卡判据内。
- **没有实测疑点-5 的 drop 顺序**（需要 GPU + Wayland 会话）。按 wgpu 22.1 的文档判为
  「契约上不满足、运行时大概率无害」，不是已证实的崩溃。
- `quality.sh` 的 `生产段 .unwrap() 693` 与 `bless 入口 2` 是既有数字，本卡未动；未逐条
  复核这 693 处（不在本卡范围）。
- 任务给的「49 处 unsafe」是 `\bunsafe\b` 行数口径；按**代码位置**实际 45 处
  （39 src + 6 tests），差值是 `unsafe fn` / `create_surface_unsafe` / 散文命中。本卡
  按代码位置逐处补，而不是按行数凑数。

## 五 · 范围外发现

- **`assets/kanesumi-assets` 子模块在本 worktree 未初始化**，`cargo test` 因缺 SVG 资源
  编不过。为跑门，从 Ether 主检出复制了**同一 commit（`7af85b9`）**的内容，并把
  `assets/kanesumi-assets/.git` 指回主检出的 submodule gitdir；这只是本地测试环境修复，
  未入库、未改 `.gitmodules`。交付前已确认 `git status` 只剩预期的源码/文档改动。
- **`quality.sh` 的作用域**：脚本写死 `$ROOT/shared/kanesumi`，在 Ether 根跑时检查的是
  主检出而不是当前 worktree；上面的 52 是临时符号链接指回本 worktree 后得到的。合并时
  应以主检出为准重跑一次。
- `kanesumi-controls/**` 一字未动（任务禁止）；`Metro*` 标识符未重命名；无新增依赖。

## 六 · 下一步

1. 调度者复核 `docs/UNSAFE_AUDIT.md` 的 5 组疑点，决定是否开卡修 env 锁与 `Shell` drop 顺序。
2. 合入前在主检出再跑一次 `bash tools/supply/quality.sh`，确认 `unsafe 块` 与 SAFETY
   口径同步（主检出当前仍是改动前状态）。
3. 若审计要求「一行一个 SAFETY」，可把 `tests/font_selection_regression.rs` 的 6 处
   逐处展开；本卡按任务书「文件头统一说明」执行，未展开。
