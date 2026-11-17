# AppDriver 固定主题入口 —— 报告（2026-11-17）

> 任务：给 `kanesumi-harness` 的 `system_theme` 加**进程级固定主题**，并给迁移快照夹具
> `testing::AppDriver` 一个便捷入口。由来见 Ether `docs/research/m1115_dock/REPORT.md` §二：
> `m-dock-c1` 在未改一行的 HEAD 上两份快照全红，换录制主题后 `2 passed` —— 快照读的是宿主
> `~/.config/ether/theme.toml`，不是测试规定的主题。

## 结论速览

1. `system_theme` 新增 `pin` / `unpin` / `pinned`；`load()` 在固定期间**短路**，不读盘、
   不看 `ETHER_THEME_CONFIG`。原有解析格式与 `load_backdrop` / `fingerprint` 语义未动。
2. `AppDriver::with_pinned_theme(self, theme) -> Self` 只是 `system_theme::pin` 的一行包装；
   `Drop` 不自动 unpin。
3. 三条新用例覆盖：坏路径下 pin 仍生效、unpin 恢复读盘、暗色 pin 的 `colors.surface`
   不等于宿主亮色值。
4. 公共 API 快照随新 API 重生成（`docs/API_SURFACE.txt` +4 行，无移除）。

## 一、API 形状

落点 `kanesumi-harness/src/system_theme.rs`：

| 项 | 签名 | 行 |
|---|---|---|
| 固定 | `pub fn pin(theme: MetroTheme)` | L42 |
| 解除 | `pub fn unpin()` | L47 |
| 查询 | `pub fn pinned() -> Option<MetroTheme>` | L52 |
| 短路 | `pub fn load() -> MetroTheme`（固定时首行返回） | L128 |

- 存储：`static PINNED: Mutex<Option<MetroTheme>>`（L29）。`MetroTheme: Copy`，锁内只做
  取值/置值，不跑用户代码。
- 锁中毒不传染：`pinned_lock()` 用 `unwrap_or_else(|e| e.into_inner())`，某个用例 panic
  不会让后续 `load()` 永远失败。
- 只影响主题读取：`load_backdrop()` / `load_wallpaper()` / `fingerprint()` 仍读盘 ——
  固定主题的目的是「同一份代码换机同色」，不是冻结整个宿主配置。**这条是取舍，已在此记录。**
- 固定期间再次 `pin` 即改值；`unpin` 未固定时无副作用。

`kanesumi-harness/src/testing.rs` L135：

```rust
pub fn with_pinned_theme(self, theme: MetroTheme) -> Self {
    crate::system_theme::pin(theme);
    self
}
```

**与任务书签名的差异**：任务书写 `AppDriver::with_pinned_theme(theme) -> Self`，但
`AppDriver<A>` 持有 `app: A`，没有 `A` 就无法构造实例，故做成 builder 方法（与既有
`with_floating` / `with_popups` 同形）：`AppDriver::new(app, w, h).with_pinned_theme(theme)`。

**时序限制（诚实记录）**：`App` 在传给 `AppDriver::new` 之前已构造。若应用的 `new()`
里已经 `system_theme::load()` 过，驱动里的 pin 影响不到那次读取，只影响之后的读取
（含工作线程 / 每帧重读）。测试要在**构造应用之前**先 `system_theme::pin`；驱动入口是给
「应用按帧/按需读主题」以及「同一测试里后建的驱动」用的。`Drop` 不 unpin：同一测试进程
内多驱动共用固定值，由测试显式 `unpin()`。

## 二、为什么不是线程局部

1. **读主题的不止测试主线程**：外壳 `platform::apply_system_theme` 在主循环读一次推给
   `App::set_theme`，应用的渲染 / 工作线程也可能自行 `system_theme::load()`。线程局部的
   固定值只对设置它的线程有效，spawn 出去的线程照样读宿主 `theme.toml` —— 正是要消除的
   不确定性。
2. **测试生命周期跨线程边界**：`#[test]` 的断言线程、驱动内部可能起的工作线程、以及
   `catch_unwind` 包裹的回调，都可能不在同一线程上。进程级固定值让「这份测试规定主题」
   对整棵调用树成立。
3. **成本可忽略**：`load()` 只在启动与配置变更时调用，一次 `Mutex` 取值的开销远低于读盘。
4. **风险由使用约束兜住**：固定值是测试专用，doc 注释写明「生产进程不得 pin」；
   固定值只由测试显式设置/清除，生产路径不调用 `pin` 时 `pinned()` 恒为 `None`，
   `load()` 行为与改动前逐字节一致。

## 三、用例清单

全部在 `kanesumi-harness/src/system_theme.rs` 的 `mod tests`：

| 用例 | 行 | 断言什么 |
|---|---|---|
| `pin_wins_over_missing_config_path` | L307 | `ETHER_THEME_CONFIG` 指向不存在的路径：未固定时 `load()` 回退浅色默认；`pin(dark)` 后 `load() == dark`、`pinned() == Some(dark)`。两者不同才证明短路生效。 |
| `unpin_restores_config_loading` | L324 | 临时 `theme.toml` 写 `scheme = "dark"`；pin 浅色时 `load()` 为浅色；`unpin()` 后 `pinned() == None` 且 `load()` 等于盘上解析出的主题。 |
| `dark_pin_surface_is_not_host_light_surface` | L344 | 宿主配置写 `scheme = "light"`；pin 暗色后 `load().colors.surface ==` 暗色值，且 `assert_ne!` 宿主亮色 `surface`（纯白）。用断言，不打印。 |

夹具卫生：

- 三条用例共用 `static PIN_TEST_LOCK` 串行 —— 固定值与 `ETHER_THEME_CONFIG` 都是
  进程级，并行会互相覆盖。
- `PinGuard` 在 `Drop`（含 panic 展开）里 `unpin()`、还原/删除 `ETHER_THEME_CONFIG`、
  删除临时文件。`unsafe { env::set_var }` 是 Rust 2024 的要求，注释写明了串行前提。

## 四、验证

命令与结果（t14p/Debian，`export PATH=$HOME/.cargo/bin:$PATH`）：

```text
cargo test -p kanesumi-harness
  lib: 235 passed; 0 failed
  font_selection_regression: 1 passed
  popup_motion: 3 passed
  popup_resize: 2 passed
  doc-tests: 0 passed

cargo test -p kanesumi-harness --lib system_theme
  10 passed; 0 failed; 225 filtered out

cargo test -p kanesumi-element --test api_surface
  1 passed（重生成快照后）
```

**已验证**：上述三条命令的通过与失败计数；`docs/API_SURFACE.txt` 差异为 `+4 / -0`，
新增行就是本报告 §一 的四个公开符号。

**未验证 / 无法在此仓验证**：Ether 侧 `m-dock` / `m-filelist` / `m-topbar` 的快照在
pin 之后是否「一机录、多机绿」—— 那些测试在 Ether monorepo，本仓没有它们的夹具。
本任务只提供入口与单测；跨仓效果要等 Ether 侧改用 `pin` 后由那边的门命令证明。

**范围外发现**：`load_backdrop()` 仍读宿主配置，若某个迁移快照断言了亚克力背板
（壁纸/模糊/着色），换机仍可能变色。本任务未扩到背板；需要时另开一件，加
`pin_backdrop` 同款短路。

## 五、下一步

1. Ether 侧各批次把「录制主题」改成测试开头 `system_theme::pin(theme)`（或
   `AppDriver::with_pinned_theme`），门命令里的 `ETHER_THEME_CONFIG=` 退成兜底。
2. 若背板也进快照，补 `pin_backdrop` / `pinned_backdrop`，语义与本节相同。
