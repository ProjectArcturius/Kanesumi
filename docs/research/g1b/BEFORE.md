# G1 BEFORE —— 内容变化场景的每表面光栅成本（2026-10-04）

> 任务：`.oc/tasks/g1b-surface-gpu.md` 第 1 步「测量先行」。上游：
> Ether `docs/GPU_COMPOSITION_PLAN.md` §Ⅲ「G1 壳层 GPU 光栅（按 G0 修订）」、
> `docs/research/gpu_g0/REPORT.md`（G0 实测）。
>
> **仓边界说明**：本任务的 worktree 是 **Kanesumi 仓**（分支 `oc/g1b-surface-gpu`），
> 只在本仓提交。任务书「允许改动」列出的 Ether 主仓 `tools/perf/` 与 `docs/research/g1b/`
> 不在本 worktree 的 git 仓内，无法由工人提交；故本文件落在 Kanesumi
> `docs/research/g1b/`，Ether 侧副本与 `tools/perf` 场景脚本由调度者迁移。

## 一、结论先行

1. **不满足早停条件。** G0 已测得最大破预算链路 = Launcher 覆盖层（浮层）
   3072×1920：CPU 光栅 p50 ≈ **19 ms** > 16.7 ms，且受限档（CPUQuota=200%）同样 ≈ 19 ms。
   远超「受限档 2× 下 p95 < 8 ms」的早停线 → 继续做第 2、3 步。
2. **G3 没有改变这一结论。** G3 只把「动画」搬到合成器侧（动画期间不再重光栅），
   但**内容变化**（应用页滚动、磁贴活动内容、悬停、桌面框选）仍由应用线程每帧
   整面 CPU 光栅 —— 正是本任务要处理的部分。
3. **决策依据**：CPU 光栅吞吐 ≈ 310k px/ms（5.90 Mpx / 19 ms）；wgpu 每帧固定 ≈ 1 ms。
   面积阈值 1.0 Mpx² 对应 CPU ≈ 3.2 ms，是明确的 GPU 收益区。

## 二、复用的既有测量（G0，`docs/research/gpu_g0/REPORT.md` 表 4.2）

以下为 G0 实测的每表面 CPU/GPU 光栅 p50（ms，release，无头真 GPU，scale=1）：

| 表面（3072×1920 会话） | 物理面积 px² | CPU raster p50 | GPU raster p50 |
|---|---|---|---|
| TopBar 主表面（3072×30） | 92 160 | 0.07 – 0.10 | ~1.06 |
| Dock 主表面（3072×64） | 196 608 | 0.43 | 0.66 – 1.65 |
| 桌面 主表面（3072×1920） | 5 898 240 | 未单列（同量级 19 ms） | — |
| **Launcher 覆盖层（浮层，3072×1920）** | **5 898 240** | **18.8 – 19.1** | 未被 G0 开关覆盖（恒 CPU） |

要点：小表面 GPU 反而慢（固定 ~1 ms）；大表面 CPU 线性于面积。破预算的只有大浮层。

## 三、内容变化场景（本任务新增，脚本见 §五）

任务要求补测：①所有应用页滚动；②桌面框选；③Launcher 磁贴悬停扫动。
这三者的共同点：**内容变化帧触发整面 CPU 光栅**（`App::render_floating` / `render_into`
产出整幅 Scene，CPU 光栅 + dmabuf 提交）。其每帧成本与「静止后首次整面重绘」同量级 ——
即 G0 表 4.2 的 CPU 列。

> **未执行声明（诚实标注）**：由于本次会话的 worktree 是 Kanesumi 仓，而端到端
> 无头会话依赖 Ether 主仓的 `compositor` + 伴生进程 + `shared/kanesumi` 子模块，
> 本工人**未在本机重跑**内容变化矩阵（构建 Ether 会改动 worktree 之外的文件）。
> 上表为 G0 既有实测；内容变化帧的估计值由 G0 的稳态 CPU 光栅数据外推，
> 待调度者在 Ether 侧用 §五 脚本复跑生成权威 BEFORE 数字。

### 3.1 估算（3072×1920，受限档 ≈ 双核）

| 场景 | 触发的表面 | 变化帧 CPU 光栅估计 p50 | 估计 p95 |
|---|---|---|---|
| 所有应用页滚动 | Launcher 覆盖层 5.9 Mpx | ≈ 19 ms | ≈ 20–22 ms（受 10% 波动） |
| Launcher 磁贴悬停扫动 | Launcher 覆盖层 5.9 Mpx | ≈ 19 ms（整面） | ≈ 20–22 ms |
| 桌面框选 | 桌面主表面 5.9 Mpx | ≈ 19 ms | ≈ 20–22 ms |
| TopBar 时钟/悬停 | TopBar 0.09 Mpx | ≈ 0.1 ms | ≈ 0.2 ms |

结论：大浮层 / 桌面在内容变化帧上稳定超 16.7 ms 预算；TopBar 等小表面远低于阈值。

## 四、策略阈值依据（供第 2 步 `choose_renderer` 常量）

- `GPU_MIN_AREA_PX2 = 1_000_000`：CPU 约 3.2 ms 的面积；小表面（TopBar/Dock）
  远低于此，留在 CPU（G0 证明其 GPU 反慢）。
- `GPU_MIN_WORK_PX2_HZ = 5_000_000`：面积 × 预期刷新频率的下限，避免「大但几乎不变」
  的表面白付一份 +60 MB 设备（G0-c）。Launcher 覆盖层 5.9 Mpx × 10 Hz = 59 M 通过。
- xdg 窗口恒 GPU（现状）；IME 候选窗（约 0.2 Mpx）按面积留在 CPU。

## 五、内容变化场景脚本（供 Ether `tools/perf/` 迁移）

在 `g0_bench.sh` 的场景序列（静止 / TopBar 扫动 / Launcher 开合）之后追加：

```bash
# ④ 所有应用页滚动：Launcher 打开后逐个应用页 + 滚轮
echo "log PHASE pages-start"
echo "key 125"; echo "wait 900"          # Super → 开 Launcher
for i in $(seq 1 8); do
  echo "scroll 3"                        # 下一个应用页（Launcher 页签/滚轮语义，按实际调）
  echo "wait 300"
done
echo "log PHASE pages-end"

# ⑤ 桌面框选：关闭 Launcher，桌面按下拖动
echo "key 125"; echo "wait 700"
echo "log PHASE desktop-select-start"
echo "move 200 200"; echo "press 1"
for x in $(seq 220 20 900); do echo "move $x 400"; echo "wait 16"; done
echo "release 1"
echo "log PHASE desktop-select-end"

# ⑥ 磁贴悬停扫动：Launcher 打开后指针横扫磁贴区
echo "key 125"; echo "wait 900"
echo "log PHASE hover-start"
for y in 200 320 440 560; do
  for x in $(seq 100 30 1200); do echo "move $x $y"; echo "wait 16"; done
done
echo "log PHASE hover-end"
```

采样与归档沿用 `g0_bench.sh`（`ether-harness-perf.log` 的每表面三段耗时 p50/p95 +
`cpu.txt` 的进程 CPU%/RSS）。新增 `PHASE` 标记使 `g0_analyze.py` 可按阶段切窗口。

- [ ] 在 3072×1920 受限档（`CPUQuota=200%`）复跑 cpu / gpu 两路，填 §三 实测列。
- [ ] 2× 档：无头后端 `OutputInfo::from_winit(w,h,1.0)` 恒 scale=1，2× 需先在
  `backend_headless.rs` 或 harness 侧注入 scale（本任务不改合成器）。
