# g1a：dmabuf gbm 探测诊断改造与段错误真因

日期：2026-10-04。范围：Kanesumi 仓 `kanesumi-harness`（客户端直通探测）。
参 Ether `docs/STATE_2026-10-02.md` §Ⅳ-8、`docs/GPU_COMPOSITION_PLAN.md`。

## 结论先行

**「gbm 探测子进程被信号杀死」的真因是 HARNESS 用只读 fd 打开 DRM 节点。**

libgbm 的 `gbm_create_device` 在 `O_RDONLY` fd 上会段错误；`open_node_for` 旧代码用
`File::open`（即 O_RDONLY）。这不是 Mesa / Debian 独有缺陷：2026-10-04 在 Arch/i915
（Arrow Lake-P，i915）上同样复现，并与「旧探测永远失败 → 全壳层回落 SHM」完全吻合。

## 证据（最小复现）

独立小 crate（仅依赖 `gbm 0.18`，不依赖 harness）直连同一节点：

| 打开方式 | 结果 |
|---|---|
| `File::open("/dev/dri/renderD128")`（O_RDONLY） | `Device::new` SIGSEGV，exit 139 |
| `OpenOptions::read(true).write(true)`（O_RDWR） | `gbm device ok; bo ok fd=true`，exit 0 |
| `/dev/dri/card1` O_RDWR | `gbm device ok; bo ok fd=true`，exit 0 |

harness 侧同一结论：改 O_RDWR 前三种 recipe 的探测面包屑末行均为 `stage=gbm-device`
（即死在 `Device::new`）；改后末行均为 `stage=ok`、退出 0。

## 本任务改动

1. **阶段面包屑**（`dmabuf.rs`）：子进程每进入一个阶段**先**向
   `$XDG_STATE_HOME/ether/dmabuf-probe.log`（缺省 `~/.local/state/ether/`）追加一行并
   `sync_data`，段错误后最后一行即死点；每次探测先写头（时间 / 可执行名 / pid / 内核
   `/proc/sys/kernel/osrelease` / libgbm 文件名 / `GBM_BACKEND` 等环境变量）；超 256 KiB
   轮转为 `.1`。
2. **父进程信号解码**：`ExitStatusExt::signal()` 记信号编号与名字，失败串形如
   `fail:sig11:SIGSEGV@gbm-device`（死点阶段取面包屑末行）；不再一律记 255。
3. **入口前移**：新增 `pub fn dmabuf_probe_entry()`（`ETHER_DMABUF_PROBE` 存在则跑探测并
   `exit`，否则立即返回），`lib.rs` 导出。`platform::run_inner` 顶部改调它，去掉重复判断。
   Ether 主仓各应用 `main` 第一行的插桩点见任务报告（主仓改动不在本任务内）。
4. **备选探测**：首选失败时按 `ProbeRecipe::try_order` 顺序再试（feedback 主设备节点 /
   显式 `DRM_FORMAT_MOD_LINEAR` 修饰符 / XRGB8888），各一个子进程、各写面包屑；通过即把
   该组合存进共享 `DmabufDevice`，真实提交路径只在该证明可用的组合上分配，未知组合一律 SHM。
5. **根因修复**（独立提交）：`open_node_for` 改 `O_RDWR` 打开。

## 验证

- `cargo test --offline -p kanesumi-harness`：101 passed / 0 failed（基线 93）。
- `cargo clippy --offline -p kanesumi-harness --all-targets`：dmabuf.rs 仅余改动前既有告警。
- 快速单测故障注入：`ETHER_DMABUF_PROBE_FAULT=entry` → 子进程 SIGSEGV，面包屑末行
  `stage=entry`，父侧解码 `fail:sig11:SIGSEGV@entry`；`=abort` → `fail:sig6:SIGABRT@entry`。
- **真实端口（Arch/i915，无头合成器 + 本仓 harness 构建的 `kanesumi-calculator`，
  `ETHER_ROLE=launcher` → layer/CPU 光栅 → dmabuf 直通路径）**：
  - 正常：面包屑完整（头含时间 / exe / pid / 内核 / libgbm / 环境变量，逐阶段
    `entry→open-node→gbm-device→bo→map→fd→ok`），父侧 `result=ok:0`，缓存 `ok:0`；
    客户端日志 `dmabuf gbm 探测：可用（组合 any-node+linear-flag+ARGB）`、
    `dmabuf 直通启用`、`dmabuf 输出格式：DrmFourcc(AB24)`、
    `dmabuf gbm device 就绪：/dev/dri/renderD128`。截图
    `2026-10-04_headless-dmabuf-enabled.png`。
  - 故障注入（`ETHER_DMABUF_PROBE_FAULT=gbm-device`）：三种 recipe 全部死在
    `stage=gbm-device`，父侧 `result=fail:sig11:SIGSEGV@gbm-device`，缓存同串；客户端逐
    recipe 记 WARN 后「全部表面走 SHM」，走 `SHM 提交路径`，界面照常显示（截图
    `2026-10-04_headless-fault-sig11-shm.png`，与 dmabuf 截图同款界面 —— 回退不改外观，
    正是「不丢帧」契约）。

## 遗留

- Ether 主仓各应用 `main` 第一行的插桩点（本任务未改主仓）：`settings/src/main.rs:31`、
  `launcher/src/main.rs:18` 前、`librarian/src/main.rs:33` 前、`ceyboard/src/main.rs:18` 前、
  `shared/kanesumi/kanesumi-calculator/src/main.rs:11` 前、
  `shared/kanesumi/kanesumi-gallery/src/main.rs:33` 前，均插入
  `kanesumi_harness::dmabuf_probe_entry();`。
- 「主设备节点」备选在构造时 feedback dev_t 尚未到达（异步），当前传 `None` 跳过；待有
  调用点能在 probe 前得知 dev_t 时再启用。
