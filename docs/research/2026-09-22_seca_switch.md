# sec-a ↦ 主仓 回流可行性评估 · 滑动开关（MetroSwitch）

> **取证口径**：只读源码，未改任何源码，未跑 `cargo` / `gradle`。
> sec-a 检出版本 `acddf3e`（"fix(controls): MetroSwitch 适配深浅色"，2026-09-11，工作区 clean）；
> 主仓检出版本 `af43f8e`（工作区 clean）。
>
> **主证据文件**
> - Kotlin：`Kanesumi-sec-a/kanesumi-controls/src/main/java/io/github/takahashirinta/kanesumi/controls/MetroSwitch.kt`（155 行，8.2 KB）→ 下文简写 **`K.kt:N`**
> - Rust：`Ether/shared/kanesumi/kanesumi-controls/src/switch.rs`（644 行，含 11 个测试）→ 下文简写 **`S.rs:N`**
> - 辅助：sec-a `anim/sokuou/Sokuou.kt`、`anim/sokuou/UwpEasing.kt`、`core/theme/MetroColors.kt`、`core/theme/MetroTheme.kt`、`core/theme/MetroTypography.kt`
> - 辅助：主仓 `anim/presets.rs`、`core/colors.rs`、`core/indicator.rs`、`core/theme.rs`、`core/geometry.rs`、`core/typography.rs`、`docs/CONTROL_SPEC.md` §3、`docs/ROADMAP.md`、`docs/CANON_VS_TEMPORARY.md`、`kanesumi-gallery/src/app.rs`

---

## §1 结论先行

### 1.1 一句话：好在哪里

> **sec-a 的开关「更让人满意」，本质是它把**行程与滑块的分量**给足了 —— 滑块高 22dp 占轨道高 28dp 的 **78.6%**、行程 **24dp** 占轨道宽 **46.2%**；再叠上「释放时 15% 位移宽容」与「单一 progress 同时驱动位置 + 轨道填充 + 描边 + 滑块色、220ms 一次到位」——
> 手感上就是**一坨有分量的实体在轨道里走满全程、点错也不会只抖一下、松手时位移和换色同时收束**。
> 主仓 `switch.rs` 的滑块只有轨道高的 **50%**（10/20）、行程 **50%**（20/40），且**换色是瞬时的、点动与拖动之间没有宽容带** —— 同一套逻辑，观感却像「轻盈的小方块跳一下 + 颜色啪地跳变」。

三处体感差异的量化对照（数字全部来自两侧源码）：

| 观感维度 | sec-a | 主仓 Rust | 倍数 |
|---|---|---|---|
| 滑块高 / 轨道高 | 22 / 28 = **78.6%**（`K.kt:89` / `K.kt:145,151`） | 10 / 20 = **50%**（`S.rs:37` / `S.rs:46`） | 1.57× |
| 行程 / 轨道宽 | 24 / 52 = **46.2%**（`K.kt:94` / `K.kt:146`） | 20 / 40 = **50%**（`S.rs:175`） | — （比例接近，但绝对行程短，见下） |
| 行程绝对长度 | **24dp** | **20px** | 1.2× |
| 松手后滑块到位 + 全件换色 | **同一根 progress、同一段 220ms**（`K.kt:130` + `K.kt:139-140`） | 滑块 150ms 动、**颜色瞬时**（`S.rs:250` + `S.rs:324` 注释「ON=accent 实心；OFF=透明+白描边」） | 结构性差异 |
| 「想点，却拖了」 | **被宽容成点动**（`K.kt:120-125`） | 无此判据：越过 3px 即按拖动吸半（`S.rs:228-230, 245-249`） | 结构性差异 |

### 1.2 能不能回流：**能，而且收益集中；但必须双向回流（不只是抄 Kotlin）**

| 类别 | 条目数 | 结论 |
|---|---|---|
| **(a) 可直接搬**（纯逻辑 / 几何 / 状态机，与平台无关） | **6 条** | 可直接落进 `switch.rs`，无需新依赖；4 条是 sec-a 优于 Rust 的净增量 |
| **(b) 需改造**（依赖 Compose / Android API，但机制可等价重建） | **6 条** | 落点分别在 `kanesumi-canvas`（事件）、`kanesumi-controls`、`kanesumi-core`（令牌） |
| **(c) 不可搬**（平台专属） | **5 条** | 明确排除，避免把 Compose 惯性带进 Rust |

**关键非对称**：sec-a 并非全面更强。**主仓在三条上反超 sec-a**，回流时**不要**把 Rust 的强项改掉：

1. **拖动映射的鲁棒性**：Rust 用「绝对位移 `pos.x - drag.start_pointer_x` ÷ travel」（`S.rs:227-232`）—— 抗丢帧、抗事件合并；sec-a 用 `horizontalDrag` **逐事件累加 `positionChange().x`**（`K.kt:112-115`）—— 累计浮点误差，且依赖事件不丢。
2. **拖动中的按下视觉**：Rust 有完整的 Pressed 灰调（`S.rs:327-330, 345-349`）；sec-a 拖动期间**除了滑块位移什么都不变**（`K.kt:137-152` 只读 `progress.value`）。
3. **同步提交**：Rust 的 `knob.jump_to(new_progress)`（`S.rs:233`）在同一帧内生效；sec-a 走 `animScope.launch { progress.snapTo(...) }`（`K.kt:111, 115`）—— 异步协程排队。
4. **安全差异各一处**：Rust 有 `cancel()` 完整实现（`S.rs:256-260`，Gallery 在指针离开/释放到轨道外时调用，`app.rs:1418-1419, 2165-2166`），sec-a **没有取消语义**（拖出去再松手照样 commit，`K.kt:120-133` 无条件走释放决策）；反过来 sec-a 有「tap 宽容」，Rust 没有。

### 1.3 Rust 侧要复刻那种手感，**必须动到的三处**

1. **【尺规】把行程与滑块分量提上去** —— `travel` 上界 + 滑块高占比。
   落点：`switch.rs` 的 `SwitchShape::track_size()` / `knob_size()` / `knob_margin()`（`S.rs:35-57`）+ `travel()`（`S.rs:172-176`）。
   现在 Capsule 是 `(40,20)` + `(10,10)` + margin `5` → `travel=20`（`S.rs:37, 46, 54, 402-403` 的测试把它钉死了）。要复刻 sec-a 的「走满」手感需要 travel 上界 ~24 且滑块高占比 ~0.75；sec-a 是「52dp 轨道强制宽」（`K.kt:88`），主仓是「宿主给 rect」（`S.rs:264`），所以这不是抄数字、而是要**改 §3 规格并在 `measure()` 里定宽**（`switch.rs` 目前**没有 `measure()`**，13/47 个控件才有，参 `docs/ROADMAP.md` M2-3）。
   **工作量：小（改 3 个常量 + 1 个测试）／中（连带补 `measure()` 与 §3 规格）。**

2. **【决策】加「位移宽容带」，把「点动 vs 拖动」从单一阈值变成两档**。
   落点：`switch.rs` 的 `DragState`（`S.rs:73-79`）+ `release()`（`S.rs:239-252`）。
   现在只有 `if dx.abs() > 3.0 { drag.moved = true }`（`S.rs:228-230`），越过 3px 就一律按拖动吸半（`S.rs:245-249`）—— 手指微抖越过阈值、用户本意是点，结果是「滑块只挪了一点然后弹回，状态变了但看着像没生效」。sec-a 的 `displacement < 0.15f → 翻转当前状态`（`K.kt:120-125`）正是治这个。
   **工作量：小（~15 行 + 2 个测试）。**

3. **【驱动】让一根 progress 同时驱动「位移 + 轨道填充 + 描边 + 滑块色」，并把「颜色瞬时」改成与位移同一段动画**。
   落点：`switch.rs` 的 `compute_colors()`（`S.rs:325-366`）改为接受 `progress` 参数，用 `theme.colors.primary.lerp(...)` 按 `progress()` 插值（Rust 已有 `Color::lerp`，`S.rs:339` 在用）。
   现状：颜色只看 `(self.checked, is_pressed)` 两个布尔（`S.rs:335`），所以**滑块滑到一半时颜色已经整块跳完了**。sec-a 的 `lerp(trackOffColor, accentColor, p)`（`K.kt:139-140`）让「滑块到位」和「填充满」是同一个视觉事件 —— 这是「一并收束」的来源。
   顺带这条会暴露一个**新令牌需求**（详见 §3-a·5 / §3-b·3）：off 轨道的**填充**色主仓没有语义等价的令牌。
   **工作量：中（改 `compute_colors` 签名 + 调用点 + 4 个既有颜色断言测试需重写）。**

---

## §2 逐项对照表

### 2.1 结构（几何 / 尺寸 / 圆角 / 命中区 / 两侧标签）

| 项 | sec-a（Kotlin） | 主仓（Rust） | 差异裁定 |
|---|---|---|---|
| 控件外框 | **固定 52×28dp**：`.width(52.dp).height(28.dp)`（`K.kt:88-89`） | **宿主给 rect**，尺寸只在 `track_size()` 里；`render(theme, engine, rect, scene)` 不声明固有尺寸（`S.rs:264`）；**无 `measure()`** | Rust 缺固有尺寸契约 → Gallery 硬编码 200×60（`app.rs:532, 636-640`） |
| 轨道 | 恒等于外框 `52×28`（`drawRect(track)` 用整个 `size`，`K.kt:141`） | `40×20`（`S.rs:37`，源 = WinUI 2 `OuterBorder Width=40 Height=20`，`S.rs:31-32`） | **规格冲突**：sec-a 走 Ncrust 手滚版尺寸（`K.kt:36` 注释），Rust 走 UWP v1 一手源（`CONTROL_SPEC.md:100`）。二者都对，是**取值口径分歧** |
| 滑块 | `22×22`（`thumbW=22.dp`，高 = `size.height - pad*2` = 28−6 = 22，`K.kt:145,151`） | Capsule `10×10` 圆（`S.rs:46`）；Square `10×20`（`S.rs:47`） | **滑块高占比 78.6% vs 50%** —— §1.1 的核心 |
| 内边距 | `pad = 3dp`（`K.kt:144`） | Capsule margin `5`（`=(20−10)/2`，`S.rs:52-56`） | 均为对称留白；比例上 sec-a 更「撑」 |
| 行程 | `size.width - pad*2 - thumbW` = 52−6−22 = **24**（`K.kt:146`；`pointerInput` 里同式重算，`K.kt:94`） | `tw - 2*margin - kw` = 40−10−10 = **20**（`S.rs:172-176`，测试 `S.rs:403`） | sec-a 行程更长 |
| 圆角 | **全部直角**：`drawRect` 无圆角参数（`K.kt:141,143,148-152`）；注释「直角不圆」（`K.kt:57`） | **Capsule 胶囊**（`CornerRadius::Capsule`，`S.rs:60, 66`）；Square 变体直角（`S.rs:61, 67`） | **设计语言分歧**：sec-a 出题「直角矩形 track + 直角矩形 thumb」（`K.kt:35`）；主仓 Capsule 是 Lumia 复刻（`S.rs:5-9`）。主仓 `CornerRadius::Capsule` 的正典豁免理由写的是「Switch 轨道/Knob」（`geometry.rs:50-51`） |
| 描边 | `Stroke(width = 1.dp)`（`K.kt:142-143`） | `stroke_rounded_rect(..., 2.0, ...)`（`S.rs:300`），`CONTROL_SPEC.md:107` 明写「v1 用 1px 在 HiDPI 亚像素消失」 | Rust 有 HiDPI 依据，**不要回退成 1px** |
| 命中区 | **= 整个 52×28 外框**（轨道即外框，`K.kt:141`） | **= track rect 40×20**（`hit_test` 用 `track_rect().contains(pos)`，`S.rs:204-206`） | Rust 的命中区比视觉占位小 5 倍以上；且与 Gallery 的**目标级命中**（`switch_rect()` 整个 200×60，`app.rs:1395`）**不一致**：`HitTester` 判中 → `press()` 再自查 track → 若在 host 内 track 外，`self.drag` 保持 `None`，此时 `release()` 提前 `return false`（`S.rs:240-242`）→ 行为安全但**契约分裂** |
| **两侧标签（开/关文字）** | **没有**。`MetroSwitch` 无 `onText`/`offText` 参数，签名只有 `checked / onCheckedChange / modifier / accentColor / trackOffColor / borderOffColor`（`K.kt:60-68`）。**标签由调用方在外部 Row/Column 里自己放**（`sample/MainActivity.kt:381` 裸调 `MetroSwitch(checked = autoPlay, …)`；`MainActivity.kt:389-391` 是另一个 Row 的 "Icon buttons" 文本） | **有**：`on_text`/`off_text`（`S.rs:90-92`）+ `header`（`S.rs:88`），布局为「Header（上）→ Track（左）+ State text（右，gap 12px）」（`S.rs:264-320`；`track_rect` 里 `header_h = body.line_height + 8`，`S.rs:182-191`；state text `text_x = track.right() + 12.0`，`S.rs:315`） | **Rust 在此项上更强**：sec-a 缺 UWP 的 `Header` / `OnContent` / `OffContent` 三件；Rust 落实了 `CONTROL_SPEC.md:84-90, 105-106` 的规格 + Lumia 真机截图依据（`S.rs:5-9`）。**此项不回流，反而应反向输出给 sec-a** |
| 布局契约 | 定宽定高 → 布局阶段就确定，`drawBehind` 里用 `size` 现算（`K.kt:137-152`） | `track_rect(rect, theme)` / `knob_rect(rect, theme)` 是纯函数（`S.rs:179-201`），但**渲染与命中共用同一对函数**（`S.rs:294, 304, 205`）—— 这是好设计，保留 | Rust 的「布局-命中-绘制同源」优于 sec-a 的「绘制里散落常量 + pointerInput 里重算同一式」（`K.kt:94` vs `K.kt:146`，两处必须手动保持同步） |

### 2.2 交互状态机

| 项 | sec-a（Kotlin） | 主仓（Rust） | 差异裁定 |
|---|---|---|---|
| 手势框架 | `Modifier.pointerInput(Unit)` + `awaitEachGesture`（`K.kt:90, 95`） | 由 App 转发：`press()` / `drag_to()` / `release()` / `cancel()`（`S.rs:209, 222, 239, 256`） | Rust 的模型更好（控件不持事件循环）；sec-a 的模型更省调用方代码 |
| 按下 | `awaitFirstDown()`（`K.kt:96`） | `press(rect, theme, pos)`：先 `hit_test` 再记 `DragState { start_pointer_x, start_progress, moved:false }`，置 `ControlState::Pressed`（`S.rs:209-219`） | Rust 命中不合格时**静默忽略**（`S.rs:210-212`） |
| **点按 vs 拖拽判据** | **两级**：① 系统 `touchSlop`（`awaitHorizontalTouchSlopOrCancellation`，`K.kt:97-99`）—— 未越过 = tap；② 越过 slop 后**再加一道 15% 位移宽容**（`K.kt:120-125`） | **一级**：固定 `3.0` 逻辑像素（`if dx.abs() > 3.0 { moved = true }`，`S.rs:228-230`）；`CONTROL_SPEC.md:125-126` 写的就是「位移 <3px → toggle / ≥3px → 拖动」 | **sec-a 的二级判据是净增量**（§1.3 第二处）。另：`3px` 是**绝对像素**，不随 DPI / 控件尺寸缩放；sec-a 的 `15%` 是**归一化比例**，天然随行程缩放 |
| 速度判据 | 无 | 无 | 两侧都**没有用速度**做点按/拖拽分类 |
| 跟手映射 | 逐事件累加：越过 slop 那一下先吃进去 `progress.value + drag.positionChange().x / travelPx`（`K.kt:109-110`，注释「越 slop 那一下 delta 也要吃进去，不然起手有丢帧感」），随后 `horizontalDrag` 内继续累加 `change.positionChange().x / travelPx`（`K.kt:112-115`），每步 `coerceIn(0f,1f)` 并 `snapTo` | 绝对位移：`new_progress = (drag.start_progress + dx / travel).clamp(0,1)`，`dx = pos.x - drag.start_pointer_x`（`S.rs:227-233`） | **各有胜负**：sec-a 无起手丢帧（净胜），Rust 抗丢帧/抗事件合并（净胜）。**建议融合**：保留 Rust 的「绝对位移」公式，但把**起点锚定到 `checked` 的语义端点**而非 `progress()` |
| 起点锚定 | `initialProgress = if (currentChecked) 1f else 0f` —— **语义起点**，注释明写「不是当前动画中位」（`K.kt:105-107`） | `start_progress: self.progress()` —— **当前动画值**（`S.rs:216`） | **sec-a 更正确**：Rust 若在 `set_target` 动画途中按下，`start_progress` 是中间值，同一次拖动会让 knob 最终停在与指针不成比例的位置 |
| 起手死区 | 无（越过 slop 即跟手，且吃掉 slop 溢出量） | **有 3px 死区**：`if drag.moved && travel > 0.0` 才动 knob（`S.rs:231`）—— 3px 内 knob 完全不动，然后突然跳到 +3px 处 | sec-a 的跟手更连续；Rust 的「跳变」是可见的 |
| **提交条件** | `displacement < 0.15f → !currentChecked`（宽容带）；否则 `localProgress >= 0.5f`（`K.kt:120-125`）。**无论是否翻转都本地 `animateTo(target)`**，注释「释放位置几乎不会正好在 0/1 上，得让 thumb 滑到端点」（`K.kt:128-130`） | `if drag.moved { checked = progress() >= 0.5 } else { checked = !checked }`（`S.rs:245-249`）；随后 `knob.set_target(endpoint)`（`S.rs:250`）。**吸半依据是「knob 中心过半」**（`CONTROL_SPEC.md:126`） | 同构；sec-a 多一层宽容带。注意 Rust 用的是 **knob progress** 而非指针位置 —— 比 sec-a 更贴合「中心过半」的规格文字 |
| **取消（拖出去再松手）** | **无**。`awaitHorizontalTouchSlopOrCancellation` 只处理 slop 阶段的取消（`K.kt:97`）；进入 `horizontalDrag` 后没有出界检测，松手**无条件**走释放决策（`K.kt:120-133`） | **有**：`cancel()` 清 `drag` + 复位 state + `set_target(当前 checked 端点)`（`S.rs:256-260`）；调用点：释放到命中区外（`app.rs:1417-1419`）、指针离开窗口（`app.rs:2164-2166`）；测试 `S.rs:533-555` | **Rust 在此项上更强**，且 `CONTROL_SPEC.md:127` 已把它写成规格。**不要回流掉** |
| 拖动中抑制外部动画 | `isDragging` 标志（`K.kt:73, 81, 104, 127`）门控 `LaunchedEffect(checked)`（`K.kt:80-84`） | `update(dt)` 里 `if self.drag.is_none()` 才推进 knob（`S.rs:156-160`） | 同构；Rust 更简单（无「忘记复位标志」风险） |
| 悬停 | **无**（`pointerInput` 不产生 hover） | `ControlState::Hovered` → off 描边改 `on_surface`、on 填充 lerp 白 15%（`S.rs:331, 338-341, 352-356`） | **Rust 更强**（桌面鼠标必需） |
| 禁用 | **无**（无 `enabled` 参数，`K.kt:60-68`） | `ControlState::Disabled` → 全局 alpha `disabled_opacity` 0.38（`S.rs:267-272, 333`） | **Rust 更强** |
| **闭包冻结陷阱** | 已修但留坑：`pointerInput(Unit)` 闭包只创建一次，必须用 `rememberUpdatedState` 包成 `currentChecked`/`currentOnCheckedChange`（`K.kt:76-78`，注释 46-49 行详述历史上「tap 只生效第一次」「从 on 拖回 off 不发回调」两个 bug） | 无此陷阱（控件不持闭包，逐帧读 `self.checked`） | **Rust 模型天然免疫**；sec-a 的这段注释是极好的病历，值得在 Rust 侧文档留一句「为什么控件不持回调」 |

### 2.3 动画

| 项 | sec-a（Kotlin） | 主仓（Rust） | 差异裁定 |
|---|---|---|---|
| API | `Animatable<Float>` + `animateTo(spec)`（`K.kt:72, 82, 130`）/ `snapTo`（`K.kt:111, 115`） | `MetroAnim`（sokuou，`presets.rs:1`；`S.rs:94`） | 同源：sec-a 的 `sokuouSpring` 换算与 Rust `SpringAnim::new(response, damping)` 同名同参（`Sokuou.kt:24-27` vs `presets.rs:64-66`） |
| 预设 | `SokuouPresets.ToggleFlip = tween(220ms, MetroCubic)`（`Sokuou.kt:91-94`）；`MetroCubic = uwpEasing(Cubic, EaseOut)`（`UwpEasing.kt:116`） | `MetroPresets::toggle_flip() = MetroAnim::new(0.15, UwpEasing::Cubic, EaseMode::EaseOut)`（`presets.rs:128-130`，常量 `DURATION_TOGGLE_FLIP = 0.15`，`presets.rs:30`） | **同族同曲线，时长差 220 vs 150ms** |
| 时长归属 | sec-a 注释「与 UWP toggle 时长对齐」（`K.kt:54`，`Sokuou.kt:90`） | Rust 已按一手源更正为 `RepositionThemeAnimation 0.15s`（`presets.rs:29`，`CONTROL_SPEC.md:131, 136`，测试 `presets.rs:244-248`） | **Rust 正确、sec-a 的 220ms 无一手依据**（`0.22` 是 Rust 侧同一批被推翻的旧值）→ 见 §5 待裁定 |
| **是否帧率无关** | **是**：Compose `Animatable` 内部按 `withFrameNanos` 的**时间**推进（不是按帧计数） | **是**：`MetroAnim::update(dt)` 用 `elapsed += dt` 再算 `t = elapsed/duration`（`uwp.rs:200-212`）；`SpringAnim` 为解析解（`spring.rs`） | 两侧都合格 ✅ |
| **是否可中断** | **是**：`Animatable.animateTo` 在已运行时自动从**当前值**续跑（新 target 打断旧 target） | **是**：`MetroAnim::set_target` 把 `from = self.value` 再清零 `elapsed`（`uwp.rs:182-189`）；有测试 `toggle_is_interruptible`（`S.rs:447-459`） | 两侧都合格 ✅ |
| **是否有速度继承** | **无**。`Animatable.animateTo` 不接受初速度（Compose 侧要继承速度走 `Animatable.animateTo` 的变体或 `splineBasedDecay`，此处未用）；`SokuouPresets` 里唯一的物理项是弹簧（`Sokuou.kt:24-27, 44-56`），`ToggleFlip` 是 tween（`Sokuou.kt:91-94`） | **无**。`MetroAnim` 无速度字段（`uwp.rs:152-160`）；Rust 侧**有能力但没用**：`SpringAnim::set_target_with_velocity(new_target, from_vel)` 存在（`spring.rs:84`） | **两侧都缺** ⚠️ 这是「甩一下开关」手感天花板的共同原因（§5 待裁定） |
| 滑块位移驱动 | 单根 `progress.value` → `thumbX = padPx + travel * p`（`K.kt:146-147`） | 单根 `knob.value()` → `knob_x = track.x + margin + travel()*progress()`（`S.rs:194-201`） | 同构 ✅ |
| **轨道填充驱动** | **随 progress 渐变**：`lerp(trackOffColor, accentColor, p)`（`K.kt:139`） | **瞬时布尔切换**：`compute_colors` 只看 `(checked, is_pressed)`（`S.rs:335`），注释明写「轨道/Knob 换色**瞬时**（无 crossfade）」（`CONTROL_SPEC.md:133`） | **净增量在 sec-a**（§1.3 第三处）。注意 Rust 的「瞬时」是**有规格依据的**（UWP v1 模板如此），所以这条回流要先动规格 |
| **描边驱动** | 同样随 progress：`lerp(borderOffColor, accentColor, p)`（`K.kt:140`） | 只有 off 态有描边（`S.rs:299-301, 357`），on 态 stroke = TRANSPARENT（`S.rs:343`）—— 无渐变 | 同上 |
| **图标交叉淡入** | **不存在**。sec-a 的开关里**没有图标**（`K.kt` 全文无 icon 相关 import / 绘制）；track 内只有滑块 | **也没有**。`switch.rs` 的 Scene 命令只有 `FillRect` / `StrokeRect` / `Text`（测试 `S.rs:572-591` 断言「ON = track fill + knob」2 个 fill、0 个 stroke） | **两侧都无图标** ⚠️ UWP `ToggleSwitch` 的 `SwitchKnobOn/Off` 内含 Glyph（✓/✗），两侧都没做 → 见 §5 |
| 拖动期间动画 | 暂停外部 `animateTo`（`isDragging` 门控），knob 由 `snapTo` 直给（`K.kt:111, 115`）—— 严格说 `snapTo` 仍走协程 | `jump_to(new_progress)`（`S.rs:233`）**同步**生效，且 `update()` 整个跳过（`S.rs:157`） | Rust 更干净（同步 + 无协程排队） |
| 释放后到位动画 | 无论 `checked` 是否变化都 `animateTo(target)`（`K.kt:128-130`） | 无条件 `set_target(endpoint)`（`S.rs:250`） | 同构 ✅ |
| `update(dt)` 契约 | 由 Compose 帧时钟驱动（零重组：视觉只在 `drawBehind` 读 `progress.value`，`K.kt:137`） | 由 App 每帧调 `switch.update(dt)`（`app.rs:1990`，dt 限幅 50ms，参 `CLAUDE.md`） | 两侧都是「单值单帧推进」 ✅ |

### 2.4 配色与深浅色

**先看 sec-a 的「适配深浅色」那条提交到底做了什么**（commit `acddf3e`，全量 diff 仅 6 增 3 删，只动了默认参数与一行绘制）：

```diff
-    trackOffColor: Color = Color(0xFF333333),
-    borderOffColor: Color = Color.Gray.copy(alpha = 0.35f),
+    // off 态轨道：onSurfaceVariant 半透明，深浅色下都是可辨的中性灰。
+    trackOffColor: Color = LocalMetroColors.current.onSurfaceVariant.copy(alpha = 0.45f),
+    borderOffColor: Color = LocalMetroColors.current.divider,
 ) {
+    // thumb 恒为 onPrimary（浅色主题白、深色主题白），压在 primary/中性灰轨道上都清晰；
+    // 旧实现 on 态写死黑色，浅色主题下就是一块突兀的黑。
+    val thumbColor = LocalMetroColors.current.onPrimary
...
-                val thumbColor = if (p > 0.5f) Color.Black else Color.White
```

**机制**：它**没有**引入「深浅两套常量」，而是把三个写死的颜色字面量换成**语义令牌 + 半透明强度**（`onSurfaceVariant@45%` / `divider` / `onPrimary`），因此深浅色由 `MetroTheme` 的 `colors` 参数一次性切换，控件零改动。这与主仓 `theme.rs:9-12` 的「由 `(scheme, accent)` 唯一决定，不存在两份常量」是**同一条设计原则**。而它修掉的病（「on 态写死黑色，浅色主题下就是一块突兀的黑」）与主仓 `indicator.rs:6-8` 记录的旧病（「白 10% 写死，亮色主题下悬停完全不可见」）**是同一类**。

**逐令牌映射**（✦ = 主仓缺，需新增或需裁定）

| sec-a 令牌（`MetroColors.kt:7-18`） | sec-a 取值 | 用在哪里 | 主仓对应（`colors.rs:13-65`） | 差异 |
|---|---|---|---|---|
| `background` | `#FF000000` | —（开关不用） | `background` = 暗 `#1A1A1A` / 亮 `#FAFAFA`（`colors.rs:77, 99`） | 取值口径不同（两侧都是方案内选择） |
| `surface` | `#FF0A0F16` | —（开关不用） | `surface`（`colors.rs:78, 100`） | — |
| `surfaceVariant` | `#FF14181F` | —（开关不用） | `surface_variant`（`colors.rs:79, 97`） | — |
| `divider` | `#FF2A2A2A` | **off 态描边**（`K.kt:67` → `K.kt:140`） | `divider` 暗 `#3A3A3A` / 亮 `#D6D6D6`（`colors.rs:80, 102`） | ✦ **Rust 的 switch 完全没用 `divider`**：off 描边取的是 `on_surface_variant`（`S.rs:355`） |
| `primary` | `#FF2E67B5` | **accentColor 默认值**（`K.kt:64`）→ 轨道、描边插值终点 | `primary`（`colors.rs:81, 103`）= `accent.base` | 一致 ✅（Rust 单一 accent 真源更强，`theme.rs:99-110` 有测试守着） |
| `onPrimary` | `#FFFFFFFF` | **滑块色**（`K.kt:71` → `K.kt:149`） | `on_primary` = `accent.on_accent`（`colors.rs:84, 106`；按对比度自动取黑白，`CANON_VS_TEMPORARY.md:64` D1） | **语义一致，实现更强**：Rust 的 `on_primary` 会随 accent 变黑（浅色 accent 下），sec-a 恒白。Rust 现在**没用** `on_primary`，而是写死 `Color::WHITE.with_alpha(alpha)`（`S.rs:333`） ✦ |
| `onBackground` | `#FFF0F0F0` | —（开关不用） | `on_background`（`colors.rs:85, 107`） | — |
| `onSurface` | `#FFF0F0F0` | —（开关不用） | `on_surface`（`colors.rs:86, 108`）；Rust switch 用于 header / state text（`S.rs:277, 314`）与 hover 描边（`S.rs:353`） | 一致 ✅ |
| `onSurfaceVariant` | `#FF9AA0A6` | **off 轨道填充 @45%**（`K.kt:66`） | `on_surface_variant`（`colors.rs:87, 109`） | ✦ 主仓**无「off 轨道填充」令牌**，也不在 switch 里用 `on_surface_variant` 做填充（只做描边，`S.rs:355`） |
| `pressTint` | `#22FFFFFF`（白 **13.3%**） | 由 `MetroTheme` 注入 `LocalIndication`（`MetroTheme.kt:21`）→ `MetroIndication` 直角闪切（`MetroIndication.kt:32-39, 81-84`）；**开关本身不吃它**（用的是裸 `pointerInput`，不走 `clickable`，`K.kt:90`） | `MetroIndication::press_tint` = 白 `#33` **20%**（`indicator.rs:111`）/ 亮 黑 20%（`indicator.rs:128`），一手源 = `SystemListMediumColor`（`indicator.rs:25-31`） | 强度不同：sec-a **13.3%** vs Rust **20%**。Rust 有一手源 ✅，sec-a 无依据 ✦ |
| —（无 hover 档） | — | — | `MetroIndication::hover_tint` = **10%**（`indicator.rs:110, 127`，一手源 `SystemListLowColor`） | ✦ sec-a **没有 hover 令牌**（触屏优先），Rust 有 |
| —（无 pressed 派生档） | — | — | `primary_hover` / `primary_pressed`（`colors.rs:82-83, 104-105`，由 `Accent::hover_for/pressed_for` 派生） | ✦ **Rust 有、switch 没用**：`S.rs:339` hover 用 `primary.lerp(WHITE, 0.15)` 硬算，`S.rs:347` pressed 用 `primary.lerp(on_surface_variant, 0.55)` 硬算 —— 两者都**绕过了 `primary_hover`/`primary_pressed` 令牌**（`0.15`/`0.55` 是裸字面量，但 `token_discipline` 只禁 `Color::*` 构造器与裸 `with_alpha(<数字>)`，见 `tests/token_discipline.rs:23-29, 80-93`，故未被拦） |
| —（无 AccentLow / Base* 档） | — | — | `selection_tint`（暗 accent 0.60 / 亮 0.40，`colors.rs:88, 110`，一手源 `SystemControlHighlightListAccentLowBrush`）、`track_subtle`（`surface_variant` 60%，`colors.rs:90, 112`）、`MetroIndication` 的 `base_medium_high 0.8` / `base_medium 0.6` / `base_medium_low 0.4`（`indicator.rs:75-80`） | ✦ sec-a **完全没有这一族**（它的令牌表只有 10 个字段，`MetroColors.kt:7-18`）。**这一族是主仓单方面的近期成果**（2026-09-22 一手源更正，`UWP_PRIMARY_SOURCES.md:36-44`） |
| 深浅切换机制 | `MetroColors` 作为 `data class` 默认参数暴露，调用方传另一套实例（`MetroColors.kt:7`，注释「深浅两套并列方案」）；`MetroTheme(colors = …)`（`MetroTheme.kt:14-21`） | `MetroColors::dark(accent)` / `light(accent)` 字段一一对应（`colors.rs:95` 注释「缺少任一字段即编译失败，以此强制深浅对称」），`MetroTheme::dark/light` 组装（`theme.rs:38-65`） | **Rust 的「结构强制对称」更强**；sec-a 靠人工保证两套实例字段齐全 |

**结论**：§2.4 的核心落差**不在开关本身，在令牌层 —— 且方向与直觉相反**。主仓的令牌层（深浅双态、accent 派生、Base* 档、一手源更正）**明显强于 sec-a**；sec-a 的开关「适配深浅色」只是**正确地消费了它自己那套薄令牌**。所以这条回流是**小小的 1 个新令牌 + 3 处「改用已有令牌」**，不是把 sec-a 的配色体系搬过来。

具体三个 Rust 侧缺口（详见 §3）：
- ✦**缺** off 轨道**填充**令牌（`K.kt:66` 的 `onSurfaceVariant@45%`）—— 语义上最接近的是 `track_subtle`（`surface_variant@60%`，`colors.rs:90`），但那是 ProgressBar 轨道的角色（`colors.rs:58-62` 注释明确分角色）；
- ✦**没用** `on_primary` 做滑块色（`S.rs:333` 写死 `Color::WHITE`）—— 这是 `CANON_VS_TEMPORARY.md:64` D1「on_accent 取黑白对比度大者」的直接漏用；
- ✦**没用** `primary_hover` / `primary_pressed`（`S.rs:339, 347` 各自 lerp 硬算）。

### 2.5 无障碍 / 可用性

| 项 | sec-a（Kotlin） | 主仓（Rust） | 裁定 |
|---|---|---|---|
| **语义标注** | ❌ **无**。`MetroSwitch.kt` 全文无 `semantics` / `contentDescription` / `Role.Switch` / `toggleable`；用的是裸 `pointerInput`（`K.kt:90`）—— 因此**连 `clickable` 自带的 role 都没有**。这与它自己的铁律「a11y 不可妥协：控件必须带 semantics（role/contentDescription）」（`Kanesumi-sec-a/AGENTS.md:88-89`、`CLAUDE.md:84-86`）**直接冲突** | ❌ **无**。`switch.rs` 无任何可访问性语义层（无 role / 无名字 / 无状态导出）。主仓 `ControlState` 有 `Focused` 变体（`state.rs:5-11`），但 switch 不产生它 | **两侧都缺** ⚠️ Rust 侧是**框架级缺口**（M3-1 才做 FocusRing + `is_tab_stop`，`ROADMAP.md:135`），不是 switch 单独的锅 |
| **Ripple 关闭** | ✅ 事实关闭：不用 `clickable`，故无 `indication`（`K.kt:90`；历史提交 `c7ce3fc` 的 diff 显示旧版是 `.clickable(interactionSource=…, indication = null)`，本次改为 pointerInput）—— 关得**比显式传 `indication = null` 更彻底**（连 interaction 都不产生） | ✅ 架构上不存在 ripple（Kanesumi 无 ripple 概念，`README.md:30`「非物理动画，无弹簧/回弹」；`Kanesumi-sec-a/AGENTS.md:76-78`「无弹簧/回弹/ripple/elevation 联动」） | 两侧都合格 ✅ |
| 直角闪切（替代 ripple 的反馈） | 控件**不吃**自己的 `MetroIndication`（因为它不走 `clickable`，`K.kt:90`）—— 也就是说**开关没有 press 闪切**。整个反馈只剩滑块位移 | 有 `state == Pressed && drag.moved` 的灰调（`S.rs:327-330, 345-363`），且**故意**只在真拖动时呈现（`S.rs:327-328` 注释「避免点动闪灰」，规格 `CONTROL_SPEC.md:121`） | **Rust 更强** ✅ 但「点动无任何按下反馈」两侧都偏弱（见 §5） |
| 触觉反馈 | ❌ 无（无 `HapticFeedback` / `LocalHapticFeedback`） | ❌ 无（且 Wayland 侧无触觉协议接入） | 两侧都缺；Rust 桌面上属「不适用」 |
| **禁用态** | ❌ **无**。签名无 `enabled`（`K.kt:60-68`），绘制里也读不到禁用档 | ✅ 有：`ControlState::Disabled` → `theme.indication.disabled_opacity`（0.38，`indicator.rs:114`），alpha 乘到 fill / stroke / 文字（`S.rs:267-272, 333, 343, 357, 277, 314`）；测试 `disabled_lowers_alpha`（`S.rs:623-643`） | **Rust 更强** ✅ |
| 键盘 / 焦点 | ❌ 无（Compose 不自动给裸 `pointerInput` 加键盘；未用 `toggleable` / `focusable`） | ❌ 无（M3-1/M3-5 未开工，`ROADMAP.md:135, 139`；`ControlState::Focused` 在 switch 里从未被设置） | 两侧都缺 |
| 指针出界取消 | ❌ 无（见 §2.2） | ✅ 有 `cancel()`（`S.rs:256-260`）+ 两个调用点（`app.rs:1418-1419, 2164-2166`） | **Rust 更强** ✅ |
| 目标尺寸（触屏可用性） | 52×28dp（`K.kt:88-89`）—— 未达 48dp 最小触控目标，但**轨道即外框**、且无外部 padding | 轨道 40×20，**宿主 rect 与轨道解耦**（Gallery 给 200×60，`app.rs:532`）—— 实际可点区域只有 40×20 | 两者都小于 48dp 触控标准；改造方向是「**视觉尺寸不动、命中区外扩**」（sec-a 的「命中区=轨道」已经比 Rust 的「命中区=轨道」在数值上更大） |

**Android 特有 vs 值得照搬**：

| 项 | 归属 | 说明 |
|---|---|---|
| `awaitEachGesture` / `awaitFirstDown` / `awaitHorizontalTouchSlopOrCancellation` / `horizontalDrag` | **Android 特有**（Compose gesture DSL） | 机制可等价重建（见 §3-b·2），API 不可搬 |
| 系统 `touchSlop` 常量 | **半可搬** | 数值本身（Android 约 8dp）在 Rust 侧应换算成「按 DPI 缩放的比例」；sec-a 已经用 `15%` 把**第二级**判据归一化了，这一手值得照搬 |
| `rememberUpdatedState` / `LaunchedEffect` / `Animatable` / `snapTo` | **Android 特有** | Rust 的等价物是「控件无闭包、逐帧 `update(dt)` + `jump_to`」——**Rust 的模型更简单**，不要模仿协程 |
| `Modifier.pointerInput(Unit)` 的 `Unit` key 陷阱 + 闭包冻结 | **Android 特有的坑** | 值得照搬的是**教训**（在 `switch.rs` 顶注里写一句「为什么本控件不持回调」），不是代码 |
| 命中区 = 视觉外框（`K.kt:141`） | **可搬的设计决策** | 见上表最后一行 |
| 无 ripple / 无 elevation / 直角 | **可搬的设计决策** | 主仓已满足 ✅ |

---

## §3 可回流清单

工作量口径：**小** = 1 个文件、≤30 行、测试改动 ≤2 处；**中** = 跨 2~3 个文件或改公开签名/规格；**大** = 触及引擎事件模型或需先补规格再实现。

### (a) 可直接搬（纯逻辑 / 几何 / 状态机，与平台无关）

| # | 条目 | sec-a 证据 | 建议的 Rust 落点 | 新令牌 / 新预设 | 工作量 |
|---|---|---|---|---|---|
| **a-1** | **释放决策的「位移宽容带」**：归一化位移 `< 0.15 × travel` 时判为**点动意图**，翻转当前状态而非按位置吸半 | `K.kt:120-125`（`displacement = abs(localProgress - initialProgress)`；`if (displacement < 0.15f) !currentChecked else localProgress >= 0.5f`） | `switch.rs`：新增 `const DRAG_TAP_TOLERANCE: f32 = 0.15;`；`DragState`（`S.rs:73-79`）加 `initial_progress: f32` + `displacement: f32`；`release()`（`S.rs:239-252`）改判据 | 否 | **小** |
| **a-2** | **拖动映射锚定「语义起点」而非「动画当前值」**：按下瞬间以 `checked` 决定 `initialProgress`（0 或 1），不用动画中位 | `K.kt:105-107`（注释明写「记下 tap 时的 on/off 位置（语义起点，不是当前动画中位）」） | `switch.rs`：`press()` 里 `start_progress: if self.checked { 1.0 } else { 0.0 }`（改 `S.rs:216`）。**保留 Rust 原有的绝对位移公式** `pos.x - start_pointer_x`（`S.rs:227-232`，比 sec-a 的逐事件累加更抗丢帧） | 否 | **小** |
| **a-3** | **收紧 3px 死区**：sec-a 明写「越过 slop 那一下的 delta 也要吃进去，不然起手有丢帧感」 | `K.kt:108-111` + 注释 | `switch.rs`：`drag.moved` 一旦置真，立刻按 `dx/travel` 同步 knob（现 `S.rs:231-234` 已是这个结构，但 `moved` 的阈值 3px 与「丢帧感」无关）—— 真正要改的是**把 3px 绝对阈值降级为「仅用于区分 tap/drag」**，不要让它同时充当跟手死区 | 否 | **小** |
| **a-4** | **释放后无条件补一次「滑到端点」动画**（即便 `checked` 没变），因为松手位置几乎不会正好在端点 | `K.kt:128-130`（注释同文） | `switch.rs`：`release()` 末尾的 `self.knob.set_target(endpoint)`（`S.rs:250`）**已经满足**；`cancel()`（`S.rs:259`）也满足 → **此项为「确认已具备」，无需改动** | 否 | **小（仅核对）** |
| **a-5** | **滑块色改用 `on_primary` 令牌**（而非写死白）：sec-a 恒用 `onPrimary`（浅色主题白、深色主题白）以保证压在 primary / 中性灰上都清晰 | `K.kt:69-71`（`val thumbColor = LocalMetroColors.current.onPrimary`），注释解释旧实现写死黑的病 | `switch.rs:333`：`let knob = theme.colors.on_primary.with_alpha(alpha);` —— 这正是 `CANON_VS_TEMPORARY.md:64` D1 的机制 | 否（**消费已有令牌**） | **小** |
| **a-6** | **off 轨道填充改用语义令牌 + 半透明强度**（而不是「无填充」）：sec-a 的 `onSurfaceVariant@45%` 让 off 态也有实体感 | `K.kt:65-66`（`trackOffColor = onSurfaceVariant.copy(alpha = 0.45f)`，注释「深浅色下都是可辨的中性灰」） | `switch.rs`：`compute_colors()`（`S.rs:350-358`）的 `(false, false)` 分支从 `(TRANSPARENT, stroke)` 改为 `(fill, stroke)` | **是**（见 b-3：需新增 `track_off` 或裁定复用 `track_subtle`） | **中** |

> **注意 a-1 与主仓既有规格的冲突**：`CONTROL_SPEC.md:123-128` 把交互写死为「位移 <3px → toggle / ≥3px → 拖动」。加 15% 宽容带等于**把单阈值改成两阈值**，是规格变更 —— 必须先改 `CONTROL_SPEC.md` §3「交互」段再动代码（`AGENTS.md` §二：「新增/修改控件前先读 `CONTROL_SPEC.md` 对应节；该节缺失时**先补规格再写实现**」）。

### (b) 需改造（依赖 Compose / Android API，机制可等价重建）

| # | 条目 | sec-a 依赖 | 改造方向 | 建议的 Rust 落点 | 工作量 |
|---|---|---|---|---|---|
| **b-1** | **指针捕获 / 滑动所有权**：sec-a 靠 `pointerInput` + `horizontalDrag` 在控件内独占水平手势；Rust 侧拖拽所有权在 **App**（`app.rs:1054-1058` 判 `pressed == Some(Target::Switch)` 后自行喂 `drag_to`，`app.rs:1417-1424` 自行判出界取消） | `K.kt:90-136` | 把「按下滑动时锁定目标」下沉到框架：`ROADMAP.md` M3-2 已立项「指针捕获（拖拽 / Slider 拖动 / 文本选择）—— 旧实现在 App 侧自持”。switch 是**第一个受益者** | `kanesumi-harness`（事件路由）+ `kanesumi-gallery/src/app.rs`（去掉 `pressed == Some(Target::Switch)` 特殊分支，`app.rs:1054-1058, 1268-1272, 1417-1424, 1448-1451, 2164-2166`） | **大**（跨引擎） |
| **b-2** | **`touchSlop` 的等价物**：sec-a 用系统 slop（`K.kt:97-99`）作为第一级判据 + `15%` 作第二级 | `K.kt:97-99, 120-125` | Rust 无触摸 slop 概念；桌面鼠标场景下「第一级」退化为「按钮按下/抬起」。改造点：把第二级判据（15%）做成**按 travel 归一化**的常量，第一级在指针输入下保留 3px（鼠标抖动） | `switch.rs`（已含在 a-1）；若日后接触摸，`kanesumi-harness` 需按 DPI 提供 slop | **小**（桌面）/ **中**（接触摸时） |
| **b-3** | **off 轨道填充令牌**：sec-a 用 `onSurfaceVariant.copy(alpha=0.45f)` 就地派生 | `K.kt:66` | 主仓令牌纪律**禁止**控件生产段出现裸 alpha（`tests/token_discipline.rs:11, 80-93`），故**必须**在 core 加具名令牌。两条路：① 新增 `MetroColors::track_off = on_surface_variant.with_alpha(0.45)`；② 裁定复用 `track_subtle`（`colors.rs:90`，`surface_variant@60%`）。**建议 ①** —— `colors.rs:58-62` 已明确 `track_subtle` 是 ProgressBar 轨道角色（「以免轨道与指示条抢注意」），switch 的 off 轨道是「未激活控件的实体感」，角色不同 | `kanesumi-core/src/colors.rs`（新增字段 + 深浅两方案 + 测试）+ `switch.rs::compute_colors`；并按 `CANON_VS_TEMPORARY.md` 登记（45% 无 UWP 一手源 —— UWP 的 off 态是**纯描边**，见 §5） | **中** |
| **b-4** | **颜色随 progress 插值**（把「换色瞬时」改成与位移同段动画） | `K.kt:139-140`（`lerp(trackOffColor, accentColor, p)` / `lerp(borderOffColor, accentColor, p)`） | `compute_colors(&self, theme, alpha)` → `compute_colors(&self, theme, alpha, progress: f32)`，用 `Color::lerp`（`S.rs:339` 已在用）。**前提**：先动 `CONTROL_SPEC.md:133`「轨道/Knob 换色**瞬时**（无 crossfade）」这条 UWP 规格 —— 这是**有意偏离**，须按 `CANON_VS_TEMPORARY.md` §二登记为 D 条目 | `switch.rs::compute_colors`（`S.rs:325-366`）、`render`（`S.rs:295`）+ 4 个颜色断言测试（`S.rs:558-621`） | **中** |
| **b-5** | **用 `primary_hover` / `primary_pressed` 令牌替换硬算 lerp** | —（sec-a 无此族，此项是主仓内部整改） | `S.rs:339` 的 `colors.primary.lerp(Color::WHITE, 0.15)` → 直接读 `colors.primary_hover`；`S.rs:347` 的 `lerp(on_surface_variant, 0.55)` → 读 `colors.primary_pressed`（若二者观感可接受；`docs/CANON_VS_TEMPORARY.md` §二需裁「Pressed 灰调」是否等于 `primary_pressed`）。**顺带**：`0.15` / `0.55` 这两个裸字面量现在**逃过**了 `token_discipline` 检查（该检查只管 `Color::*` 构造器与 `with_alpha(<数字>)`，`tests/token_discipline.rs:23-29, 80-93`），可考虑扩检查范围 | `switch.rs::compute_colors`；可选扩 `tests/token_discipline.rs` | **中** |
| **b-6** | **`measure()` 固有尺寸契约**：sec-a 靠 `.width(52.dp).height(28.dp)` 让布局阶段就定尺寸；Rust 的 switch **没有 `measure()`**（47 个控件里只有 13 个有，`ROADMAP.md` M2-3） | `K.kt:88-89` | 补 `pub fn measure(&self, theme: &MetroTheme) -> Size`：`width = header 宽 / track+12+state text 宽 取大者`，`height = header_h + track_row_h`。这样 Gallery 就不必硬编码 200×60（`app.rs:532, 636-640`） | `switch.rs`（新增方法）+ `app.rs`（改用 `measure`） | **中** |

### (c) 不可搬（平台专属）

| # | 条目 | 位置 | 原因 |
|---|---|---|---|
| **c-1** | Compose gesture DSL：`awaitEachGesture` / `awaitFirstDown` / `awaitHorizontalTouchSlopOrCancellation` / `horizontalDrag` / `positionChange()` | `K.kt:4-7, 28, 95-117` | Compose 指针事件模型专属；Rust 侧对应物是 `kanesumi-harness::InputEvent::{PointerPressed, PointerMoved, PointerReleased, PointerLeft}`（`app.rs:2057, 2163`） |
| **c-2** | `Animatable` / `animateTo` / `snapTo` / `LaunchedEffect` / `rememberUpdatedState` / `rememberCoroutineScope` | `K.kt:3, 12, 16-17, 32, 72, 74, 77-78, 82, 111, 115, 130` | 协程 + 重组运行时专属。Rust 等价物：`MetroAnim` + `jump_to` + 逐帧 `update(dt)`（`S.rs:156-160, 233`）。**顺带**：sec-a 在拖动中对 `snapTo` 用 `animScope.launch { }` 是异步的（`K.kt:111, 115`），Rust 的同步 `jump_to` 更干净 —— 这一类**回流方向应是 Rust → sec-a** |
| **c-3** | `Modifier` / `.width()/.height()/.drawBehind{}` / `graphicsLayer` 零重组模式 | `K.kt:4-9, 20-21, 87-89, 137` | Compose Modifier 链专属。Rust 的等价物是 `render(theme, engine, rect, scene)` 直接产出 `SceneCommand`（`S.rs:264-321`），已具备「单值单帧、绘制期读写」的等价性质 ✅ |
| **c-4** | `LocalMetroColors` / `staticCompositionLocalOf` / `CompositionLocalProvider` | `MetroTheme.kt:9-28`；`K.kt:31, 64-67, 71` | Compose CompositionLocal 专属。Rust 等价物 = `MetroTheme` 按参数显式下传（`S.rs:264` 的 `theme: &MetroTheme`），**已经是 Rust 的现状**，无需改 |
| **c-5** | `androidx.compose.ui.graphics.Color.lerp` / `Offset` / `Size` / `Stroke` / `drawRect` | `K.kt:22-26, 137-152` | Skia/Compose 绘制原语专属。Rust 对应 `Color::lerp`（`S.rs:339`）、`Rect`/`Point`/`Size`（`geometry.rs:3-85`）、`Scene::{fill_rounded_rect, stroke_rounded_rect, text}`（`S.rs:297-319`）。`Rect::contains` 是**半开区间**（`geometry.rs:150-156`）—— 与 Compose 的命中语义需注意一致性 |
| **c-6**（附加） | 52×28dp 的取值本身 | `K.kt:36, 88-89` | 注释明写「与 Ncrust UserScreen 现有手滚版同尺寸」—— 这是**应用侧尺寸**，不是设计语言规定；不能作为 Rust 侧（UWP 一手源 40×20，`CONTROL_SPEC.md:100`）的替换依据。见 §5 |

---

## §4 关键代码片段对照

### 4.1 【a-1 + a-2】释放决策：sec-a 的两级判据（原样照抄）

```kotlin
// MetroSwitch.kt:103-134
                    } else {
                        isDragging = true
                        // 记下 tap 时的 on/off 位置 (语义起点,不是当前动画中位) --
                        // 用来判"总位移是否小到该按 tap 处理"。
                        val initialProgress = if (currentChecked) 1f else 0f
                        // 越过 slop 那一下的 delta 也要吃进去,不然起手有丢帧感。
                        var localProgress = (progress.value + drag.positionChange().x / travelPx)
                            .coerceIn(0f, 1f)
                        animScope.launch { progress.snapTo(localProgress) }
                        horizontalDrag(drag.id) { change ->
                            localProgress = (localProgress + change.positionChange().x / travelPx)
                                .coerceIn(0f, 1f)
                            animScope.launch { progress.snapTo(localProgress) }
                            change.consume()
                        }
                        // 释放决策:位移 < 15% = tap 意图 (手指微抖越过 slop),翻转当前状态。
                        // 否则按最终位置就近吸附 -- >= 0.5 = on,否则 off。
                        val displacement = abs(localProgress - initialProgress)
                        val targetChecked = if (displacement < 0.15f) {
                            !currentChecked
                        } else {
                            localProgress >= 0.5f
                        }
                        val target = if (targetChecked) 1f else 0f
                        isDragging = false
                        // 无论 checked 是否翻转,都本地 animateTo 一次 -- 因为释放位置几乎
                        // 不会正好在 0/1 上,得让 thumb 滑到端点。
                        animScope.launch { progress.animateTo(target, SokuouPresets.ToggleFlip) }
                        if (targetChecked != currentChecked) {
                            currentOnCheckedChange(targetChecked)
                        }
                    }
```

### 4.2 对应的 Rust 建议写法（伪码，保留 Rust 的强项：绝对位移 + 同步 jump_to + `cancel()`）

```rust
// switch.rs —— 建议形态（伪码；保留 S.rs:222-260 的绝对位移公式与同步提交）

/// 点动 / 拖动 的第二级判据：progress 空间的总位移低于此值 = 点动意图。
/// 参 sec-a `MetroSwitch.kt:120-125`（15%）。progress 已是归一化量（1.0 = 全行程），
/// 故此常量随控件尺寸自动缩放 —— 这正是 sec-a 用 15% 而非 3px 的理由。
const DRAG_TAP_TOLERANCE: f32 = 0.15;

struct DragState {
    start_pointer_x: f32,
    /// 【a-2】锚定语义起点（checked 的端点），不是动画中位。
    /// sec-a `K.kt:105-107`；本仓旧值用 self.progress()（S.rs:216）→ 动画途中按下会错位。
    initial_progress: f32,
    moved: bool,
}

pub fn press(&mut self, rect: Rect, theme: &MetroTheme, pos: Point) {
    if !self.hit_test(rect, theme, pos) { return; }
    self.state = ControlState::Pressed;
    self.drag = Some(DragState {
        start_pointer_x: pos.x,
        initial_progress: if self.checked { 1.0 } else { 0.0 }, // ← a-2
        moved: false,
    });
}

pub fn release(&mut self) -> bool {
    let Some(drag) = self.drag.take() else { return false };
    self.state = ControlState::Normal;
    let old = self.checked;

    if drag.moved {
        // 【a-1】宽容带：progress 空间的位移小到可视为点动 → 翻转意图；
        // 否则按 knob 中心过半吸半。progress ∈ [0,1]，1.0 = 全行程，
        // 故 DRAG_TAP_TOLERANCE 直接就是 sec-a 的 15%（无需再除以 travel）。
        let displacement = (self.progress() - drag.initial_progress).abs();
        self.checked = if displacement < DRAG_TAP_TOLERANCE {
            !old                                              // 点动意图
        } else {
            self.progress() >= 0.5                            // 就近吸半
        };
    } else {
        self.checked = !old;
    }
    // 【a-4】无条件补到位动画（松手位置几乎不在端点上）。sec-a K.kt:128-130。
    self.knob.set_target(if self.checked { 1.0 } else { 0.0 });
    self.checked != old
}
```

> **量纲说明（重要）**：sec-a 的 `displacement` 是 **progress 空间**（0..1）的差值（`K.kt:120`，`localProgress - initialProgress`），其 `0.15` 直接对应「15% 行程」，**不需要**再乘 `travel` 或除以 travel。Rust 亦同 —— `self.progress()` 返回 [0,1]，所以 `DRAG_TAP_TOLERANCE = 0.15` 是**同一量纲的直接搬运**。早期草稿曾写成 `displacement < DRAG_TAP_TOLERANCE * travel` 与 `span / travel` 两式并用，那是把 progress 空间与像素空间混淆了，已删除。

### 4.3 【b-4】颜色随 progress 插值：sec-a 的绘制（原样照抄）

```kotlin
// MetroSwitch.kt:137-153
            .drawBehind {
                val p = progress.value
                val track = lerp(trackOffColor, accentColor, p)
                val border = lerp(borderOffColor, accentColor, p)
                drawRect(track)
                val strokeWidth = 1.dp.toPx()
                drawRect(color = border, style = Stroke(width = strokeWidth))
                val padPx = 3.dp.toPx()
                val thumbW = 22.dp.toPx()
                val travel = size.width - padPx * 2f - thumbW
                val thumbX = padPx + travel * p
                drawRect(
                    color = thumbColor,
                    topLeft = Offset(thumbX, padPx),
                    size = Size(thumbW, size.height - padPx * 2f),
                )
            },
```

### 4.4 对应的 Rust 建议写法（伪码）

```rust
// switch.rs::compute_colors —— 建议签名与主体（伪码）
// 现状：只看 (checked, is_pressed) 两个布尔（S.rs:335）→ 滑块滑到一半颜色已跳完。
// 目标：位移与换色同段收束（sec-a K.kt:139-140）。
fn compute_colors(&self, theme: &MetroTheme, alpha: f32, progress: f32) -> (Color, Color, Color) {
    let colors = &theme.colors;
    // 【a-5】滑块色改用 on_primary 令牌（现 S.rs:333 写死 Color::WHITE）
    let knob = colors.on_primary.with_alpha(alpha);

    let is_pressed = matches!(self.state, ControlState::Pressed)
        && self.drag.map(|d| d.moved).unwrap_or(false);
    let is_hovered = matches!(self.state, ControlState::Hovered);

    // 【b-3】off 轨道填充令牌（需在 core 新增 track_off；见 §3-b-3）
    let off_fill = colors.track_off.with_alpha(colors.track_off.a * alpha);
    // 【b-5】on 端色改用已有令牌（现 S.rs:338-341 硬算 primary.lerp(WHITE, 0.15)）
    let on_fill = if is_hovered { colors.primary_hover } else { colors.primary };
    let on_fill = on_fill.with_alpha(on_fill.a * alpha);

    // 【b-4】按 progress 插值：off 填充 ⇄ on 填充；描边 off → 透明
    let fill = off_fill.lerp(on_fill, progress);
    let stroke = colors.on_surface_variant
        .with_alpha(colors.on_surface_variant.a * alpha * (1.0 - progress));

    // 按下档仍是「真拖动才有」（保留本仓规格 CONTROL_SPEC:121，不回流掉）
    if is_pressed {
        let dim = if self.checked { colors.primary_pressed } else { colors.on_surface_variant };
        return (dim.with_alpha(dim.a * alpha), Color::TRANSPARENT, knob);
    }
    (fill, stroke, knob)
}
```

### 4.5 【结构对照】主仓的 Header + Track + State text（**建议反向输出给 sec-a**，原样照抄带行号）

```rust
// switch.rs:178-206 —— 「布局-命中」同源，比 sec-a 在绘制里重算常量更稳
    /// 轨道在宿主 `rect` 内的实际位置（Header 占顶后再垂直居中）。
    pub fn track_rect(&self, rect: Rect, theme: &MetroTheme) -> Rect {
        let (tw, th) = self.shape.track_size();
        let body = theme.typography.body;
        let header_h = if self.header.is_empty() { 0.0 } else { body.line_height + 8.0 };
        // Track 行高 = max(track, body.line_height) —— 让 state text 与 track 同基线居中
        let track_row_h = th.max(body.line_height);
        let track_y = rect.origin.y + header_h + (track_row_h - th) / 2.0;
        Rect::new(rect.origin.x, track_y, tw, th)
    }
```

sec-a 侧的对应缺口：`MetroSwitch` 无 `header` / `onText` / `offText` 参数（`K.kt:60-68`），标签完全由调用方在外部拼（`sample/MainActivity.kt:381`）；且 `travelPx` 在 `pointerInput`（`K.kt:94`）与 `drawBehind`（`K.kt:146`）**两处各算一次**，靠注释「拿到的是 drawBehind 里同一套常量的像素值」（`K.kt:91-93`）人工保证同步 —— 主仓的 `track_rect`/`knob_rect` 纯函数共用法更安全。

---

## §5 不确定 / 需人裁定的点

| # | 待裁定 | 两侧证据 | 为什么不能由 agent 定 |
|---|---|---|---|
| **U1** | **尺寸口径二选一**：sec-a 的 `52×28`（滑块 22×22、travel 24）还是主仓的 `40×20`（滑块 10×10、travel 20）？「滑块高占轨道 78.6%」是 sec-a 手感的主要来源，但主仓 `CONTROL_SPEC.md:95-107` 的 40×20 有**一手源**（WinUI 2 `ToggleSwitch_themeresources_v1.xaml` 的 `OuterBorder Width="40" Height="20"` / `SwitchKnobOn Width="10" Height="10"`），且 `S.rs:394-404` 的测试把它钉死了 | `K.kt:36, 88-89, 145-146` vs `S.rs:31-39, 44-57, 394-404`、`CONTROL_SPEC.md:95-107` | 这是**设计语言 vs 一手源**的取舍，且 sec-a 的 52×28 自述是「与 Ncrust UserScreen 现有手滚版同尺寸」（`K.kt:36`）——**应用侧尺寸不是设计规定**。若采用，需要一次真机视觉确认 + 改 §3 规格 + 改 2 个测试（`S.rs:394-415`） |
| **U2** | **动画时长 220ms 还是 150ms**：sec-a `ToggleFlip = 220ms MetroCubic`（`Sokuou.kt:91-94`，注释「对齐 UWP toggle 的 220ms」），主仓 `DURATION_TOGGLE_FLIP = 0.15`（`presets.rs:30`，注释「对齐 UWP RepositionThemeAnimation 0.15s」+ 测试 `presets.rs:244-248`）。**`0.22` 正是主仓已按一手源推翻的旧值**（`CONTROL_SPEC.md:131, 136`「原 0.22s 改为 0.15s」） | `Sokuou.kt:90-94` vs `presets.rs:29-30, 127-130, 244-248`、`CONTROL_SPEC.md:131` | **Rust 侧有一手源、sec-a 没有**。合理结论是 sec-a 应向 Rust 对齐（Rust → sec-a），但需要维护者确认「220ms 是刻意的手感选择还是遗留误抄」 |
| **U3** | **off 轨道是否该有填充**：sec-a 给 `onSurfaceVariant@45%` 实体填充（`K.kt:66`）；主仓按 UWP v1 是**纯描边无填充**（`S.rs:355-357`，`CONTROL_SPEC.md:113`「Off Normal：轨道 fill = Transparent」）。若采用 sec-a 的做法即为**有意偏离 UWP**，须登记 D 条目 | `K.kt:65-66, 139` vs `S.rs:350-358`、`CONTROL_SPEC.md:111-119` | 涉及「照 UWP 一手源」还是「照观感」，且新令牌的 45% 无一手依据（需按 `CANON_VS_TEMPORARY.md` §一登记） |
| **U4** | **换色是否改成随 progress 渐变**：主仓 `CONTROL_SPEC.md:133` 明写「轨道/Knob 换色**瞬时**（无 crossfade）」；sec-a 是渐变（`K.kt:139-140`）。渐变是否违背「Kanesumi 无颜色动画」的铁律？注意 `Kanesumi-sec-a/AGENTS.md:76-78` 写「颜色插值用『叠层 alpha』…不用颜色动画」，而 `MetroSwitch` 恰恰**违反了它自己这条铁律**（直接 `lerp` 颜色） | `K.kt:139-140` vs `CONTROL_SPEC.md:133`、`Kanesumi-sec-a/AGENTS.md:76-78` | 「视觉上的『一并收束』收益」vs「铁律一致性」——且 sec-a 自己内部就不一致，无法作为权威 |
| **U5** | **图标交叉淡入要不要做**：任务焦点提到「图标交叉淡入」，但**两侧都没有图标**（`K.kt` 全文无 icon；`switch.rs` 的 Scene 只有 FillRect/StrokeRect/Text，测试 `S.rs:572-591` 断言 fill 数=2、stroke 数=0）。UWP `SwitchKnobOn/Off` 模板内含 Glyph（✓/✗），`CONTROL_SPEC.md:95-107` 的尺寸表也**没有列 Glyph 尺寸** | `K.kt:137-152`、`S.rs:264-321, 572-591` vs `CONTROL_SPEC.md:95-107` | 规格缺失（`AGENTS.md` §二：「该节缺失时先补规格再写实现」）。要么裁定「不做图标」并写进 §3，要么先取 UWP 模板的 Glyph 尺寸 |
| **U6** | **速度继承要不要做**：两侧**都缺**（sec-a 的 `ToggleFlip` 是 tween，`Sokuou.kt:91-94`；Rust `MetroAnim` 无速度字段，`uwp.rs:152-160`）。Rust 有现成能力未用：`SpringAnim::set_target_with_velocity(new_target, from_vel)`（`spring.rs:84`）。若要做，等于把 `switch.rs` 的 knob 从 `MetroAnim` 换成 `SpringAnim` —— 但 `CONTROL_SPEC.md:131` 明写「150ms Cubic EaseOut（RepositionThemeAnimation）」，**换弹簧即偏离 UWP** | `S.rs:94, 108, 152, 233, 250`、`presets.rs:127-130` vs `spring.rs:84`、`CONTROL_SPEC.md:131` | 手感提升 vs 规格一致性；且要新增动画预设（`presets.rs`）并登记 |
| **U7** | **`cancel()` 语义是否要回流给 sec-a，以及 `HitTester` 契约分裂是否要修**：Rust 有 `cancel()`（`S.rs:256-260`）落进了规格（`CONTROL_SPEC.md:127`）；sec-a 没有。但 Rust 侧自身有一处不一致：Gallery 的目标级命中用整个 `switch_rect()` 200×60（`app.rs:1395`），控件级命中用 `track_rect()` 40×20（`S.rs:205`）—— 前者判中后者判不中时，`press()` 静默返回、`release()` 提前返回（`S.rs:210-212, 240-242`），行为安全但语义重叠 | `S.rs:204-206, 209-219, 240-242` vs `app.rs:1395, 1268-1272, 1448-1451` | 是「控件命中区该不该=宿主 rect」的框架级问题，归 M2-3（`ROADMAP.md:110`）裁定 |
| **U8** | **`CONTROL_SPEC.md` §10 表格残旧**：`CONTROL_SPEC.md:328` 仍写「面板收起 0.26s Quadratic / `sheet_dismiss` 0.26」——**与代码不符**（`presets.rs:15` 已是 `0.15`，注释明写「旧值 0.26 无依据，已按 150ms 更正」，提交 `eb9daf9`）。同表 `:326` 的「Switch 滑动 0.22」也与代码 0.15 不符 | `CONTROL_SPEC.md:326, 328` vs `presets.rs:11-15, 29-30` | 纯文档滞后，但**同一份被本报告当作权威引用的文档**有陈旧行，说明 §10 表本身需要一次全面复核（agent 不宜单方面改规格文档） |

---

## §6 交付摘要（三分类计数）

- **(a) 可直接搬：6 条**（a-1 位移宽容带 ✦新增 / a-2 语义起点锚定 ✦新增 / a-3 收紧跟手死区 / a-4 补到位动画 ✅已有 / a-5 滑块改用 `on_primary` 令牌 ✦新增 / a-6 off 轨道填充 ✦新增·需新令牌）
- **(b) 需改造：6 条**（b-1 指针捕获【大·跨引擎】/ b-2 slop 等价物【小】/ b-3 `track_off` 新令牌【中】/ b-4 颜色随 progress 插值【中】/ b-5 改用 `primary_hover`·`primary_pressed`【中】/ b-6 补 `measure()`【中】）
- **(c) 不可搬：6 条**（手势 DSL / 协程运行时 / Modifier 链 / CompositionLocal / Skia 绘制原语 / 52×28 的应用侧尺寸）
- **反向输出（Rust → sec-a）：3 条** —— Header+OnContent/OffContent 布局、`cancel()` 取消语义、绝对位移拖动映射（+ 「颜色瞬时」的规格依据）
- **三处必动**：① `switch.rs` 的 `track_size/knob_size/knob_margin` + `measure()`；② `switch.rs` 的 `DragState` + `release()`；③ `switch.rs` 的 `compute_colors` 签名 + `kanesumi-core/src/colors.rs` 新增 `track_off`
