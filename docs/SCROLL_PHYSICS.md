# 滚动物理（速度采样 / 惯性 / 瞬停 / 夹紧）

> 代码：`kanesumi-element/src/scroll.rs`（`ScrollKinetics`）。
> 正典口径：Ether `docs/INTERACTION_CANON.md` §Ⅴ（滚轮步进 48px、触控板跟手 + 惯性）。
> 临时数值登记：`docs/CANON_VS_TEMPORARY.md` T27。

## 一、为什么在元素层、为什么不读时钟

滚动的每一拍最终由外壳出帧驱动。模型里读 `Instant::now()` 会让同一段输入在 60 / 120 Hz
与掉帧时给出不同结果，测试只能断言「大概」。因此 `ScrollKinetics` 只有四个时间入口：
`finger(dy, dt)`、`release()`、`press()`、`tick(dt)` —— **dt 一律由调用方给**。
这样整条「采样 → 松手 → 衰减 → 停止」可以喂固定 dt 序列逐点复算（本文件 §五）。

## 二、模型

| 环节 | 规则 | 迁自 |
|---|---|---|
| 滚轮 | `wheel(dy)` 直接落位、不采样、不产生惯性；到界返回「未变化」供调用方冒泡 | Qt `InputPress` 打断滑行（`qscroller.cpp:485`、`:1626`） |
| 跟手 | `finger(dy, dt)` 1:1 落位，同时记一条 `(dt, 位移)` 样本 | GTK `tick` 按真实时间差积分（`gtkkineticscrolling.c:186`） |
| 速度采样 | 只保留最近 100 ms 样本，速度 = Σ位移 / Σ时长（**时间加权**，不是各样本速度平均） | Qt 的 `min(dt,50)/50` 平滑强度（`qscroller.cpp:1019-1024`）；GTK 用解析式而非逐样本平均 |
| dt=0 | 位移照旧、**不入样本**（否则 1/0 造出无穷速度） | Qt `if (deltaTime <= 0) return;`（`qscroller.cpp:1000`） |
| 超长间隔 | 单条样本 > 100 ms 视为两段手势，旧样本清空 | Qt 只在 `deltaTime < 100` 时平滑（`qscroller.cpp:1023`） |
| 松手 | `release()`：|v| < 50 px/s 不起甩；否则按 τ 衰减滑行 | Qt `minimumVelocity`（`qscroller.cpp:1248`）；GTK 起甩即衰减（`:120-126`） |
| 惯性 | 一阶指数：`v(t)=v₀e^{−t/τ}`、`x(t)=x₀+v₀τ(1−e^{−t/τ})`，逐拍解析积分 | GTK `pos = c1 + c2·e^{−friction·t}`（`gtkkineticscrolling.c:194-196`） |
| 停止 | |v| < 30 px/s 吸附为 0、结束滑行 | GTK `fabs(velocity) < 0.1`（`gtkkineticscrolling.c:206-208`，量级见 §四） |
| 瞬停 | `press()`：速度清零、样本清空，后续 `tick` 不推进 | Qt `QScroller::stop`（`qscroller.cpp:485-500`）+ `pressWhileScrolling`（`:1626-1641`） |
| 夹紧 | 越界即停在 `[0, max_offset]`，速度清零，**不回弹** | GTK 越界转 overshoot（`gtkkineticscrolling.c:198-203`）—— Ether 明确不要这一段 |
| 掉帧 | `tick(dt)` 的 dt 截断到 100 ms | Qt 无等价口径（用 `frameRateSkip` 丢帧）；本仓取采样窗口同值 |
| 速度上限 | 松手速度夹到 6000 px/s | Qt 把快于 2.5 mm/ms 当伪输入（`qscroller.cpp:1011-1015`） |

**与 GTK / Qt 的有意差异**：

1. **不做 overshoot / 回弹**：GTK 越界会切到 `GTK_KINETIC_SCROLLING_PHASE_OVERSHOOTING`
   再弹回平衡位（`gtkkineticscrolling.c:198-203`、`:213-236`）。Ether 正典是越界即停。
2. **不做吸附（snap）与加速甩**：Qt 的 `nextSnapPos` / `acceleratingFlickSpeedupFactor`
   （`qscroller.cpp:1244-1274`、`:1578-1601`）本仓不移植 —— 列表没有吸附点语义。
3. **速度按窗口总位移算**：Qt 用递推平滑（`newv·s + releaseVelocity·(1−s)`），本仓用
   窗口和除窗口时长。两者在匀速输入下等价；窗口和更便于逐点复算与解释。

## 三、常量（全部集中在 `kanesumi-core/src/interaction.rs`，本模块只引用）

| 量 | 取值 | 来源 | 状态 |
|---|---|---|---|
| 衰减时间常数 τ | 0.3338 s（衰减率 0.95） | WinUI `ScrollPresenter.cpp:31` `c_scrollPresenterDefaultInertiaDecayRate = 0.95`，τ = −1/ln(1−0.95)；Ether `docs/research/w-comp-ref` §① 五档实测拟合 | 有源，未真机校准 |
| 停止阈值 | 30 px/s | WinUI `ScrollPresenter.cpp:6503` `s_minimumVelocity = 30.0f`；`w-comp-ref` §① 实测 v_stop ≈ 30 | 有源，未真机校准 |
| 起甩门限 | 50 px/s | 任务书取值，`w-comp-ref` §⑤ 只给停止阈值 | **临时**（T27） |
| 采样窗口 | 100 ms | `INTERACTION_CANON.md` §Ⅴ「最近 ~100 ms」；Qt 平滑窗口 50 ms、断档 100 ms（`qscroller.cpp:1019-1023`） | 有源，未真机校准 |
| 采样断档 | 100 ms | Qt `deltaTime < 100`（`qscroller.cpp:1023`） | 有源 |
| 单拍截断 | 100 ms | 本仓自定（Qt 用丢帧、GTK 按真实时间积分，无等价口径） | **临时**（T27） |
| 速度上限 | 6000 px/s | Qt 2.5 mm/ms 伪输入门限（`qscroller.cpp:1011-1015`）换算后取整 | **临时**（T27） |

## 四、真机需要校准的三个数（按优先级）

1. **衰减时间常数 τ（当前 0.3338 s）** —— 决定一甩能滑多远：总位移 ≈ v₀·τ。
   同一甩速下 τ 偏大 = 滑过头，偏小 = 拖不动。量法：记松手速度 v₀ 与最终停位，τ = 位移 / v₀。
2. **停止阈值（当前 30 px/s）** —— 决定尾巴长短。太小会在末尾拖出「不到一像素」的抖动帧，
   太大会显得戛然而止。量法：录逐帧 offset，取最后一段非零帧的速度。
3. **采样窗口（当前 100 ms）** —— 决定「慢慢拖」会不会被上一段快甩带飞。窗口太长会把已停手的
   速度算进去，太短则正常连续输入也被切成两段。量法：快甩后立刻慢拖，看松手速度是否回到慢拖值
   （回归 `sample_window_forgets_old_fast_motion` 即此判据）。

三者都在 `CANON_VS_TEMPORARY.md` T27 登记；取到真机值后改 `kanesumi-core` 常量并删登记行。

## 五、测试（19 条，全部按固定 dt 序列驱动）

| 用例 | 判据 |
|---|---|
| `wheel_step_lands_exactly_and_never_glides` | 一格 48px 落位、不惯性、后续帧不动 |
| `wheel_steps_stop_at_content_end` | 贴底后 `wheel` 返回「未变化」（冒泡依据） |
| `trackpad_updates_follow_the_finger_pointwise` | 跟手位置逐点相等 |
| `zero_dt_moves_but_does_not_sample` | dt=0 落位但不产生速度 |
| `glide_positions_match_first_order_decay` | 20 拍逐点等于闭式解（1e-3）+ 总位移 |
| `press_stops_glide_immediately` | 按下后位置冻住 |
| `opposite_input_interrupts_and_reverses` | 反向输入打断并反向起甩 |
| `equal_opposite_samples_cancel_to_zero_velocity` | 正反抵消不起甩 |
| `clamp_at_bottom_stops_without_rebound` | 全程不越过上限 |
| `clamp_at_top_is_symmetric` | 全程不越过下限 |
| `shrinking_max_offset_clamps_and_stops` | 上限缩小即夹紧清零 |
| `long_dt_is_truncated_to_max_tick` | 1 s 与 100 ms 同位移 |
| `long_gap_discards_previous_samples` | 断档样本作废 |
| `slow_input_never_starts_inertia` | 25 px/s 不起甩 |
| `velocity_is_time_weighted` | 500 而非等权 667 px/s |
| `sample_window_forgets_old_fast_motion` | 窗口滑过后速度回落 |
| `implausible_flick_speed_is_capped` | 20000 → 6000 px/s |
| `no_scrollable_space_never_glides` | 无可滚空间不起甩 |
| `wheel_during_glide_stops_it` | 滚轮打断滑行 |

复跑：`cargo test -p kanesumi-element --lib scroll::tests`（19 passed）。

## 六、未收敛项（已知，不要当成已接线）

1. **`MetroScrollView` 仍用自己的采样 + `FrictionAnim`**（`kanesumi-controls/src/scroll_view.rs`
   「触控板：跟手 + 松手惯性」）。两者常数同源、模型等价，但代码是两份。收敛时以本模块为
   唯一模型，`FrictionAnim` 只留给非滚动动画。
2. **`MetroList` 尚未接**：它的 `Event::Scroll` 只认滚轮 `dy`，元素级 `Event::Scroll` 不带 dt，
   直接接本模块需要「每个滚动事件携带时间戳」或「控件按帧 flush 累计位移」。
   后者的接线方案见 `docs/ELEMENT_TREE.md` §Ⅵ 注，属下一件任务。
3. **合成滚动（fp4b）路径的惯性在合成器跟踪器里**，不在本模块；两条路径的常数需对齐。
