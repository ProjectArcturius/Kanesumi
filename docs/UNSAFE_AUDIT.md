# Kanesumi unsafe 审计表（2026-11-18）

审计前置：给 Kanesumi 源码里每一处 `unsafe` 补「为什么这里安全」的依据。
本卡只加注释与本表，**不改任何行为**；`git diff` 逐行核对过，源码侧只有注释行变化
（唯一的非注释差异是工作树里 `assets/kanesumi-assets` 子模块指针的本地修复，见报告 §范围外）。

## 一 · 统计口径

`grep -rn "unsafe" --include=*.rs` 数的是**含该词的行**，不是 `unsafe` 块：
`unsafe fn` / `create_surface_unsafe` / 注释里的词都会命中。按代码位置数：

| 项 | 前 | 后 |
|---|---|---|
| `unsafe` 命中行（src） | 44 | 45（新增注释里引用 `unsafe` 字面量的 1 行） |
| `unsafe` 命中行（tests） | 6 | 8（文件头统一说明引用 `unsafe` 的 2 行） |
| **实际 unsafe 代码位置（src）** | **39** | **39（未增删）** |
| 实际 unsafe 代码位置（tests） | 6 | 6 |
| `SAFETY` 注释行 | 14 | 51 |
| 有紧贴 SAFETY 依据的代码位置 | 14 | **45 / 45** |

> `strict.rs` / `diagnostics.rs` 命中的是散文里的「unsafe」二字，文件内没有 unsafe 块，
> 本表不为它们编造条目；两处都补了「为何不需要 unsafe」的说明（§五）。

## 二 · 逐处审计

| 文件:行 | 这段 unsafe 在做什么 | 不变量 | 结论 |
|---|---|---|---|
| `kanesumi-harness/src/platform.rs:390` | `mem::transmute` 把 `&mut dyn App` 延长为 `'static` | `run -> !` 永不返回，调用方无法观察借用结束；App 在 `main` 栈上活到进程结束，胖指针（数据 + vtable）始终有效 | 安全 |
| `kanesumi-harness/src/platform.rs:399` | 从 `*mut dyn App` 解引用重建 `'static` 借用 | 指针来自 390 行的唯一借用；闭包同步执行、不跨线程，无并存的 `&mut` 使用 | 安全 |
| `kanesumi-harness/src/platform.rs:1228` | `xkbcommon::Keymap::new_from_fd` 编译合成器下发的 keymap | fd/size 来自 `zwp_input_method_keyboard_grab_v2::keymap`；协议保证 fd 可 mmap、size 含结尾 NUL；fd 短于 size 时 xkbcommon 返回 `Err` 而非越界 | 安全 |
| `kanesumi-harness/src/platform.rs:5382` | `MmapMut::map_mut` 把 wl_shm 文件映射为可写像素缓冲 | fd 由 `shm_open(expected*2)` 新建、`set_len` 到映射长度、创建后立即 unlink；进程独占，映射期无外部写者；`MmapMut` 不取得 fd 所有权 | 安全 |
| `kanesumi-harness/src/platform.rs:5939/5941/5963/5973/5977/5981` | 测试内 `env::set_var` / `remove_var`（字体索引开关） | 用例保存旧值并在函数末尾恢复；但同进程 `context_menu::tests::engine` 会并发读 `KANESUMI_TEST_FONT`，libc environ 的读写构成数据竞争 | **疑点-1** |
| `kanesumi-element/src/alloc_budget.rs:27` | `unsafe impl GlobalAlloc` | 只把分配动作转发给 `System` + 线程局部计数，不改指针/尺寸/对齐 | 安全 |
| `kanesumi-element/src/alloc_budget.rs:32/37` | `alloc` 声明 + `System.alloc` 转发 | `layout` 由调用方按 trait 契约给出；`System.alloc` 满足同一契约（空指针失败、按 align 对齐、转移所有权） | 安全 |
| `kanesumi-element/src/alloc_budget.rs:43/46` | `alloc_zeroed` + `System.alloc_zeroed` | 同 `alloc`，额外保证返回块全零（trait 要求） | 安全 |
| `kanesumi-element/src/alloc_budget.rs:53/58` | `realloc` + `System.realloc` | `ptr` 来自本分配器（实际即 `System`）同 `layout` 且未释放；保留 `min(old,new)` 字节，失败时旧块仍有效 | 安全 |
| `kanesumi-element/src/alloc_budget.rs:65/68` | `dealloc` + `System.dealloc` | 指针来自本分配器同 `layout`、只释放一次 | 安全 |
| `kanesumi-render/src/canvas_v2.rs:920` | wgpu `create_pipeline_cache` 读落盘管线缓存 | `data` 只来自配对的 `PipelineCache::get_data()`；文件名含适配器/驱动 hash；`fallback: true` 让坏数据被丢弃重建 | 安全 |
| `kanesumi-render/src/canvas_v2.rs:2592/2597/2602` | 测试内改 `KANESUMI_CANVAS_FULL` | 用例保存/恢复；键只被 `canvas_full_forced()` 读；但并行测试线程的其它 env 读取仍与 set/remove 构成竞争 | **疑点-2** |
| `kanesumi-harness/src/dmabuf.rs:832` | `setrlimit(RLIMIT_CORE, 0)` 关闭 core dump | `rlimit` 是栈上已初始化结构体、`RLIMIT_CORE` 是合法资源常量、指针在调用期有效；失败可忽略 | 安全 |
| `kanesumi-harness/src/dmabuf.rs:959` | `sigaction(SIGSEGV, SIG_DFL)` + `raise(SIGSEGV)` | `sigaction` 结构体允许全零位模式（`mem::zeroed` 合法）、`SIG_DFL` 可赋给处理函数字段、第三参数 null 表示不要旧处置；仅测试故障注入 | 安全 |
| `kanesumi-harness/src/dmabuf.rs:967` | `abort()` | 无参数、无前置条件、保证不返回 | 安全 |
| `kanesumi-harness/src/role.rs:201/206/211` | 测试内改 `ETHER_ROLE` | 保存/恢复；键只被 `EtherRole::from_env` 读；并行测试线程的 env 读取仍构成竞争 | **疑点-3** |
| `kanesumi-appmenu/src/install.rs:68` | `pub unsafe fn install_from_foreign_handles` 的调用契约 | 两个指针必须来自 eframe raw-window-handle，且在窗口/句柄生命周期内有效、属于同一 Wayland 连接（`# Safety`） | 安全 |
| `kanesumi-appmenu/src/install.rs:77` | `Backend::from_foreign_display` 包装 wl_display | 调用方保证 display 是有效 `wl_display` 指针；`Backend` 只借用、不取得所有权 | 安全 |
| `kanesumi-appmenu/src/install.rs:80` | `ObjectId::from_ptr` 包装 wl_surface | surface 是同连接的 `wl_proxy` 指针，interface 与 `WlSurface` 匹配；无效时返回 `Err` | 安全 |
| `kanesumi-harness/src/platform/font_mmap.rs:68` | `Mmap::map` 只读映射系统字体文件 | 映射对象是 root 拥有、组/其他无写位的白名单普通文件；包升级用「临时文件 + rename」原子替换，旧 inode 内容不变 | 安全 |
| `kanesumi-harness/src/render.rs:392` | `create_surface_unsafe` 用 raw handle 建 wgpu 表面 | 两个 handle 均判非空、来自同一连接；**但 wgpu 要求 handle 活到 Surface drop 之后**，而 `Shell.surface` 声明在 `renderer` 之前（先 drop） | **疑点-5** |
| `kanesumi-harness/src/system_theme.rs:273/283/293` | 测试内改 `ETHER_THEME_CONFIG` | `PIN_TEST_LOCK` 串行本模块三个用例、`PinGuard::drop`（含 panic 展开）恢复；锁不覆盖其它测试线程的 env 读取 | **疑点-4** |
| `kanesumi-harness/src/timeline.rs:43` | `clock_gettime(CLOCK_MONOTONIC, &mut ts)` | `ts` 是栈上合法 `timespec`，clockid 为常量；返回值忽略也不会越界 | 安全 |
| `kanesumi-harness/src/frame_clock.rs:17` | `clock_gettime(CLOCK_MONOTONIC, &mut ts)` | 同 `timeline.rs:43` | 安全 |
| `kanesumi-harness/tests/font_selection_regression.rs:144/185/191/205/260/285` | 集成测试内 `env::set_var` / `remove_var` | 独立测试二进制、唯一 `#[test]` 单线程顺序执行、末尾恢复；文件头有统一说明 | 安全 |

## 三 · 疑点（只记录，不在本卡改逻辑）

1. **`platform.rs:5939/5942/5964/5974/5978/5982`** —— 同一 lib 测试二进制内，
   `context_menu::tests::engine`（`context_menu.rs:251`）会读 `KANESUMI_TEST_FONT`；
   `cargo test` 默认多线程，与这里的 set/remove 并发时 libc `environ` 读写构成数据竞争
   （Rust 2024 把 `set_var` 标 unsafe 的原因）。建议：加一把进程级 env 锁让所有
   env 读/写测试串行，或把该用例移到 `tests/` 独立二进制（同 `font_selection_regression.rs` 的做法）。
2. **`canvas_v2.rs:2592/2597/2602`** —— 同类问题：`KANESUMI_CANVAS_FULL` 只在本 crate 读，
   但并行测试线程的任意 `env::var` 仍与 `set_var` 竞争。建议同上。
3. **`role.rs:201/206/211`** —— 同类问题；`ETHER_ROLE` 只有 `from_env` 读，但没有锁，
   并行测试线程的 env 读仍可能撞上 set/remove。建议同上。
4. **`system_theme.rs:273/283/293`** —— `PIN_TEST_LOCK` 只串行本模块用例，不阻止其它测试
   线程读环境；属于同一类残余竞争。建议把锁提升为进程级 env 锁。
5. **`render.rs:392`（调用方 `platform.rs` Shell 字段序）** —— wgpu 的契约要求
   `raw_window_handle` / `raw_display_handle` 活到返回的 `Surface` drop 之后；
   `Shell` 里 `surface: wl_surface::WlSurface` 声明在 `renderer: Option<SurfaceRenderer>`
   之前，按 Rust 字段声明序**先 drop surface、后 drop renderer**。当前 wgpu 的 Wayland
   表面析构多半不碰 wl_surface，所以大概率「碰巧不炸」，但契约上没有保证。
   建议：把 `renderer`（及持有 Surface 的字段）调到 `surface` 之前，或给 `Shell` 写
   `Drop` 先 `take()` 渲染器。**这是本表唯一涉及生产路径的疑点**，建议下一张卡处理。

## 四 · 为何 `unsafe` 行数会从 50 变 53

新增文字里有 3 行引用了「`unsafe`」这个词（src 1 行 + tests 文件头 2 行），
`grep -rn "unsafe"` 会一并计数；**源码里的 unsafe 块一个没增、一个没删**。
以代码位置计仍是 39（src）+ 6（tests）= 45，与改前一致。

## 五 · `strict.rs` / `diagnostics.rs`：没有 unsafe 块

这两个文件在任务给的「unsafe 数」表里各占 1，但命中的是散文（`env::set_var` 在 Rust 2024
是 unsafe 的说明）。它们**刻意不调用** `env::set_var`：测试开关走 `AtomicU8` 覆盖量，
生产路径只读一次环境变量并缓存进 `OnceLock`。本卡在这两处各补了一句「为何不需要
unsafe」的说明，不虚构 SAFETY 条目。

## 六 · 复核方法

```bash
# 每个 unsafe 代码行上方 3 行内必须有 SAFETY（本表 45 处全部满足）
python3 - <<'PY'
import re, pathlib
for f in pathlib.Path('.').rglob('*.rs'):
    if 'target' in f.parts: continue
    lines = f.read_text().splitlines()
    for i, l in enumerate(lines):
        if re.search(r'\bunsafe\b', l.split('//')[0]):
            if not any('SAFETY' in w for w in lines[max(0, i-3):i]):
                print('missing', f, i+1)
PY
```

行号会随源码漂移；以本表为准复核时先 `git log -1` 对齐 2026-11-18 的 `oc/k-unsafe-audit`。
