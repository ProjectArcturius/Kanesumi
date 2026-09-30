# Kanesumi Runtime —— 在 Ether 上的复用与性能（2026-09-30 立）

> 缘起：维护者建议「Kanesumi 建立一个 runtime 类似物，保证在 Ether 上的复用性与性能」。
> `Ether/docs/PACKAGING_PLAN.md` 早已预留了这个概念（manifest `identity.min_runtime`、deb 元包
> `ether-runtime`、「未声明的能力在 runtime 侧拒绝」），但从未定义它**技术上是什么**。本文定义之。

## §Ⅰ 「runtime」拆成三层

Ether 同时常驻 5~6 个 Kanesumi 进程（TopBar/Settings、Dock/Launcher、桌面层、Ceyboard，外加打开的窗口）。
每个进程各带一份 Kanesumi —— 这就是复用性与性能问题的来源。三层按「收益 ÷ 代价」排序：

| 层 | 是什么 | 解决 | 代价 |
|---|---|---|---|
| **R1 共享资源** | 字体、字形、探测结果等**数据**不按进程重复 | 常驻内存、启动时间 | 小；不涉 ABI |
| **R2 第一方动态库** | Kanesumi 编成一个随 Ether 一起发布的 `.so`（`-C prefer-dynamic`），第一方应用共享代码页 | 二进制体积、代码页内存 | Rust 无稳定 ABI → 必须与所有第一方应用**同一次构建、同一个包**发布；第三方不可用 |
| **R3 第三方 SDK** | C ABI / `abi_stable` 边界 + 语义版本，manifest `min_runtime` 门槛生效 | 第三方应用接入 | 大；有第三方应用时才值得 |

另议：Android 式 zygote（预初始化进程 fork 出应用）暂缓 —— fork 必须早于 Wayland 连接与 GPU 初始化，
与合成器现行的伴生进程拉起方式冲突；待 R1/R2 数据出来再评估。

## §Ⅱ R1 —— 已完成的第一项：字形按需解码（2026-09-30）

**实测基线**（Arch，release，`kanesumi-canvas/examples/font_mem.rs`）：

| 字体 | 文件 | 旧（fontdue） | 新（按需） |
|---|---|---|---|
| Noto Sans CJK Regular (.ttc) | 18.6 MiB | VmRSS **+333.6 MiB**，加载 **366 ms** | VmRSS **+19.2 MiB**，加载 **32 ms** |
| Source Han Sans SC Bold (.otf) | 16.2 MiB | +322.3 MiB，363 ms | +16.8 MiB，28 ms |

根因：fontdue 在 `Font::from_bytes` 时预解析**全部**字形轮廓（CJK 字体数万字形）。改为塑形用 rustybuzz、
光栅用其自带 ttf-parser 的轮廓 + `ab_glyph_rasterizer`，字形首次绘制时才解码（外壳已有位图缓存）。
**按 6 个常驻进程计，省下约 1.8 GB 常驻内存，每个进程启动快约 330 ms。** 附带收益：kanesumi-controls
599 项测试 32 s → 1.3 s（每项测试都在付字体加载代价）。视觉经无窗口快照（`kanesumi-harness::snapshot`，
`tree_demo --snapshot`）在 1× / 2× 下核对。

### R1 余项

| 项 | 做法 | 预期 |
|---|---|---|
| R1-2 字体文件 mmap | 以只读映射取代 `fs::read`，各进程共享页缓存 | 每进程再省 ≈ 字体文件大小（~19 MiB × N）。需 `memmap2` 的一处 `unsafe`（映射期间文件被改写属未定义行为）—— Kanesumi 目前 0 unsafe，此项需维护者点头，或放在 harness（已依赖 memmap2）以「只映射 /usr/share/fonts 下只读文件」为不变式 |
| R1-3 会话级字体索引 | 字体查找 / 回退链在会话内算一次，落 `$XDG_RUNTIME_DIR/kanesumi/fonts.idx`，各进程直接读 | 省重复的目录扫描；为多字重（`FontWeight` 死字段，T7）铺路 |
| R1-4 启动探测结果共享 | dmabuf / GPU 探测已按会话缓存（`LINUX_DMABUF_PLAN.md`）；把同一机制推广为 runtime 的通用「会话级探测缓存」 | 伴生进程拉起更快 |
| R1-5 进程常驻内存基线 | 在 Debian 的 Ether 会话里一次性落盘每个 Kanesumi 进程的 VmRSS / RssFile（零交互：启动脚本写 `~/.local/state/ether/rss.log`） | R2 的决策依据 |

## §Ⅲ R2 —— 第一方动态库（待 R1-5 数据）

- 形态：`libkanesumi_runtime-<ver>.so` 装在 `/usr/lib/ether/`，由 `ether-runtime` deb 提供；第一方应用以
  `-C prefer-dynamic` 链接（libstd 同样共享）。**同一次 workspace 构建产出全部第一方二进制与该库**，
  版本一一绑定（manifest `min_runtime` = 该构建的版本）。
- 预期收益：每个第一方二进制去掉一份 Kanesumi + wgpu + std 的代码（发布构建下各约数 MB 到十余 MB），
  代码页在进程间共享。收益大小以 R1-5 的实测为准，**无数据不做**。
- 风险：构建系统复杂度（deploy.sh / install.sh 要同时装库与二进制）；任何一个第一方应用单独重编即 ABI 失配 ——
  必须由 `install.sh` 校验「二进制与库的构建号一致」，不一致拒装（同 Ceyboard Mock 引擎防呆思路）。

## §Ⅳ R3 —— 第三方 SDK（远期）

有第三方应用需求时再立项：以 C ABI 或 `abi_stable` 暴露元素树 + 控件的稳定子集，`min_runtime` 语义版本门槛，
能力声明在 runtime 侧强制（PACKAGING_PLAN §三）。在此之前第三方应用静态链接 Kanesumi，不承诺 ABI。
