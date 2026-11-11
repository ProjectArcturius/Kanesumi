// scroll.rs —— 滚动物理：速度采样 / 松手惯性 / 按下瞬停 / 越界夹紧。
// 参 `docs/SCROLL_PHYSICS.md`、`docs/ELEMENT_TREE.md` §Ⅵ、Ether `docs/INTERACTION_CANON.md` §Ⅴ。
//
// 为什么放在元素层、且**不读系统时钟**：滚动的每一拍最终都由外壳的出帧节奏驱动，
// 读 `Instant::now()` 会让同一段输入在 60 Hz / 120 Hz / 掉帧时给出不同结果，测试也就
// 只能断言「大概」。「时间一律由调用方传 dt」是本模块唯一的时间入口 —— 于是整条
// 速度采样 → 松手 → 衰减 → 停止的序列可以喂固定 dt 逐点复算（参 §测试）。
//
// 与既有实现的关系：`kanesumi-controls` 的 `MetroScrollView` 自带一套 WinUI 口径的
// 采样 + `FrictionAnim` 惯性（`scroll_view.rs`「触控板：跟手 + 松手惯性」）。那一套的
// 采样窗口、衰减常数与本模块同源（`kanesumi-core::interaction`），差别是本模块把状态
// 收成一个可复用的纯量（偏移 / 速度 / 采样环）并把 dt 防御写进模型，供还没有惯性的
// 容器（如 `MetroList`）直接持有。两条路径尚未收敛，属于已知重复，见文档「未收敛项」。

use kanesumi_core::interaction::{
    FINGER_VELOCITY_WINDOW_MS, INERTIA_MIN_START_VELOCITY, INERTIA_STOP_VELOCITY, INERTIA_TAU_S,
};

/// 速度采样窗口（毫秒）。与 `FINGER_VELOCITY_WINDOW_MS`（100）同源：只看最近 ~100 ms 的
/// 位移，手指停住超过这个窗口就不再算「在动」。
pub const SAMPLE_WINDOW_MS: f32 = FINGER_VELOCITY_WINDOW_MS as f32;

/// 单次采样允许的最大间隔（毫秒）。超过即视为两段独立手势，样本丢弃。
///
/// 出处：Qt `qscroller.cpp:1026` 只在 `deltaTime < 100` 时做速度平滑 —— 手指停顿
/// 100 ms 以上再动，上一段速度不该带进新一段。
pub const MAX_SAMPLE_GAP_MS: f32 = 100.0;

/// 单拍允许推进的最大时长（毫秒）。掉帧 / 断点后的一拍按此截断，避免内容瞬移。
///
/// 临时值：100 ms ≈ 6 帧 @60 Hz。Qt 用 `frameRateSkip` 丢帧、GTK 直接按真实时间积分
/// （`gtkkineticscrolling.c:186`），都没有等价的「截断」口径；这里取采样窗口同值，
/// 让「停一下再回来」与「采样断档」用同一条时间界。参 `CANON_VS_TEMPORARY.md` T27。
pub const MAX_TICK_MS: f32 = 100.0;

/// 速度上限（px/s）。临时值：6000 px/s ≈ 1080p 屏高 / 180 ms。
///
/// 出处：Qt 把「快于 2.5 mm/ms」当成伪输入并截断（`qscroller.cpp:1018-1020`，
/// 约等于 20 ms 扫过一屏）；本模块按 60 Hz 下的合理上限取整，参 `CANON_VS_TEMPORARY.md` T27。
pub const MAX_VELOCITY_PX_S: f32 = 6000.0;

/// 一次采样的原始记录：这一拍走了多少、用了多久。
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    dt_ms: f32,
    dy: f32,
}

/// 单轴滚动物理状态（偏移 + 速度 + 采样环）。
///
/// 调用方负责把外壳输入翻译成四类调用：
/// - 滚轮：`wheel(dy)`（离散步，直接落位，不产生惯性）；
/// - 触控板 / 连续源：`finger(dy, dt)`（跟手 1:1，累计速度样本）；
/// - 手指离开：`release()`（够快才启动滑行）；
/// - 每帧：`tick(dt)`（推进衰减；返回偏移是否变化）。
///
/// 指针按下（含滚轮介入）走 `press()` —— Qt `QScroller::stop` 语义：立即停、不滑完。
#[derive(Debug, Clone)]
pub struct ScrollKinetics {
    offset: f32,
    max_offset: f32,
    /// 当前滑行速度（px/s，正 = 向下 / 向右）。
    velocity: f32,
    /// 是否正在惯性滑行。
    gliding: bool,
    /// 最近一段手势的样本（窗口内的才会计入速度）。
    samples: Vec<Sample>,
    /// `samples` 的时长之和（毫秒），随入队 / 出队维护。
    window_ms: f32,
}

impl ScrollKinetics {
    /// 新建一个静止于 0、上限为 `max_offset` 的物理状态。
    pub fn new(max_offset: f32) -> Self {
        Self {
            offset: 0.0,
            max_offset: max_offset.max(0.0),
            velocity: 0.0,
            gliding: false,
            samples: Vec::new(),
            window_ms: 0.0,
        }
    }

    /// 当前偏移（px，已夹在 `[0, max_offset]`）。
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// 当前速度（px/s）。
    pub fn velocity(&self) -> f32 {
        self.velocity
    }

    /// 可滚上限（px）。
    pub fn max_offset(&self) -> f32 {
        self.max_offset
    }

    /// 是否正在惯性滑行。
    pub fn is_gliding(&self) -> bool {
        self.gliding
    }

    /// 更新可滚上限。内容变短 / 视口变大时把偏移拉回界内并**清零速度**
    /// （越界即停，不回弹 —— Ether 不用 overshoot）。
    pub fn set_max_offset(&mut self, max_offset: f32) {
        self.max_offset = max_offset.max(0.0);
        if self.offset > self.max_offset {
            self.offset = self.max_offset;
            self.velocity = 0.0;
            self.gliding = false;
        }
    }

    /// 滚轮离散步：直接落位，打断任何滑行（Qt `pressWhileScrolling` → `stop`）。
    /// 返回偏移是否变化；到界时不变，调用方据此决定是否把事件冒泡给外层。
    pub fn wheel(&mut self, dy: f32) -> bool {
        self.press();
        self.nudge(dy)
    }

    /// 触控板 / 连续源的一拍增量：跟手 1:1 落位，并记一条速度样本。
    ///
    /// `dt_ms` 是这一拍对应的时长（由调用方给，通常是一帧）。`dt_ms <= 0` 时位移照旧
    /// 落位、但**不入样本**（Qt `qscroller.cpp:1000` 对 `deltaTime <= 0` 直接返回），
    /// `dt_ms > MAX_SAMPLE_GAP_MS` 视为两段手势、清空旧样本。
    /// 返回偏移是否变化（到界不动时调用方可把事件冒泡给外层）。
    pub fn finger(&mut self, dy: f32, dt_ms: f32) -> bool {
        // 新输入立即打断滑行（Qt `moveWhileDragging` → `setState(Dragging)`）。
        self.velocity = 0.0;
        self.gliding = false;
        let before = self.offset;
        self.nudge(dy);
        let moved = self.offset - before;
        if dt_ms <= 0.0 {
            // 无时长的位移只影响位置，不参与速度估计 —— 否则 dt=0 会造出无穷速度。
            return moved.abs() > f32::EPSILON;
        }
        if dt_ms > MAX_SAMPLE_GAP_MS {
            // 停手太久：上一段的速度不能带进新一段（Qt 只在 deltaTime < 100 时平滑）。
            self.samples.clear();
            self.window_ms = 0.0;
            return moved.abs() > f32::EPSILON;
        }
        self.push_sample(dt_ms, moved);
        moved.abs() > f32::EPSILON
    }

    /// 手指离开 / `axis_stop`：按最近窗口的加权速度决定是否续惯性。
    /// 返回是否真的启动了滑行。
    pub fn release(&mut self) -> bool {
        let v = self.sampled_velocity();
        self.samples.clear();
        self.window_ms = 0.0;
        if v.abs() < INERTIA_MIN_START_VELOCITY || self.max_offset <= 0.0 {
            // 极慢输入 = 有意停住；无可滚空间也不启动（否则原地空转一帧）。
            self.velocity = 0.0;
            self.gliding = false;
            return false;
        }
        self.velocity = v.clamp(-MAX_VELOCITY_PX_S, MAX_VELOCITY_PX_S);
        self.gliding = true;
        true
    }

    /// 指针按下 / 滚轮介入：立即停住滑行并清空样本（Qt `QScroller::stop`，
    /// `qscroller.cpp:485`；`pressWhileScrolling` 直接回 `Inactive`）。
    pub fn press(&mut self) {
        self.velocity = 0.0;
        self.gliding = false;
        self.samples.clear();
        self.window_ms = 0.0;
    }

    /// 推进一拍。`dt_ms` 超过 [`MAX_TICK_MS`] 时按上限截断（掉帧不瞬移）。
    /// 返回偏移是否变化。撞到边界即停、速度清零，不回弹。
    pub fn tick(&mut self, dt_ms: f32) -> bool {
        if !self.gliding {
            return false;
        }
        let dt = dt_ms.clamp(0.0, MAX_TICK_MS);
        if dt <= 0.0 {
            return false;
        }
        // 一阶指数：v(t) = v₀·e^{−t/τ}，位移 = v₀·τ·(1 − e^{−t/τ})。
        // 与 GTK 的解析式同形（`gtkkineticscrolling.c:194-196`：pos = c1 + c2·e^{−friction·t}）。
        let tau = INERTIA_TAU_S;
        let decay = (-dt / 1000.0 / tau).exp();
        let step = self.velocity * tau * (1.0 - decay);
        self.velocity *= decay;

        let before = self.offset;
        let raw = self.offset + step;
        let clamped = raw.clamp(0.0, self.max_offset);
        let hit_edge = (clamped - raw).abs() > f32::EPSILON;
        self.offset = clamped;
        if hit_edge || self.velocity.abs() < INERTIA_STOP_VELOCITY {
            // 越界即停（Ether 不做 overshoot 回弹）；速度降到停止阈值也吸附。
            self.velocity = 0.0;
            self.gliding = false;
        }
        (self.offset - before).abs() > f32::EPSILON
    }
}

impl ScrollKinetics {
    /// 直接位移并夹紧。撞界时速度清零（手指拖到边缘不该攒出一段滑行）。
    fn nudge(&mut self, dy: f32) -> bool {
        let before = self.offset;
        let raw = self.offset + dy;
        let clamped = raw.clamp(0.0, self.max_offset);
        if (clamped - raw).abs() > f32::EPSILON {
            self.velocity = 0.0;
            self.gliding = false;
        }
        self.offset = clamped;
        (self.offset - before).abs() > f32::EPSILON
    }

    /// 记一条样本并维护 ~[`SAMPLE_WINDOW_MS`] 的窗口（超窗的旧样本从队首丢）。
    fn push_sample(&mut self, dt_ms: f32, dy: f32) {
        self.samples.push(Sample { dt_ms, dy });
        self.window_ms += dt_ms;
        while self.window_ms > SAMPLE_WINDOW_MS && self.samples.len() > 1 {
            let dropped = self.samples.remove(0);
            self.window_ms -= dropped.dt_ms;
        }
    }

    /// 窗口内按时间加权的速度（px/s）：总位移 / 总时长。
    ///
    /// 时间加权而非「各样本速度取平均」：一次 5 ms 的快甩与一次 50 ms 的慢拖，
    /// 后者代表用户真正停下来的意图，不能被等权平均抬快（Qt 同结论，
    /// `qscroller.cpp:1019-1024` 按 `min(dt, 50)/50` 缩放平滑强度）。
    fn sampled_velocity(&self) -> f32 {
        if self.window_ms <= 0.0 {
            return 0.0;
        }
        let total: f32 = self.samples.iter().map(|s| s.dy).sum();
        total / self.window_ms * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 浮点逐点比较容差（任务口径 1e-3）。
    const EPS: f32 = 1e-3;

    fn assert_close(actual: f32, expected: f32, what: &str) {
        assert!(
            (actual - expected).abs() <= EPS,
            "{what}：实际 {actual}，期望 {expected}（差 {}）",
            (actual - expected).abs()
        );
    }

    /// 用固定 dt 序列喂出一段「快甩」：每拍 `dy`、`dt` 毫秒，共 `n` 拍。
    fn flick(k: &mut ScrollKinetics, dy: f32, dt_ms: f32, n: usize) {
        for _ in 0..n {
            k.finger(dy, dt_ms);
        }
    }

    // ── 单次滚轮 ─────────────────────────────────────────────────────────────

    /// 一格滚轮 = 48px 直接落位、不产生惯性、后续帧不推进（正典 §Ⅴ 步进不变）。
    #[test]
    fn wheel_step_lands_exactly_and_never_glides() {
        let mut k = ScrollKinetics::new(1000.0);
        assert!(k.wheel(48.0));
        assert_eq!(k.offset(), 48.0);
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.is_gliding());

        assert!(!k.tick(16.0));
        assert_eq!(k.offset(), 48.0);
    }

    /// 滚轮连续三格（144px）逐格落位；到内容底后不再变化（调用方据此冒泡）。
    #[test]
    fn wheel_steps_stop_at_content_end() {
        let mut k = ScrollKinetics::new(100.0);
        assert!(k.wheel(48.0));
        assert!(k.wheel(48.0));
        assert_close(k.offset(), 96.0, "两格");
        // 第三格：仍有一小段可滚（96→100），报「有变化」但在 100 夹住。
        assert!(k.wheel(48.0));
        assert_eq!(k.offset(), 100.0);
        // 第四格：已经贴底，位置不变 → 调用方据此把滚轮冒泡给外层。
        assert!(!k.wheel(48.0), "贴底后应报「未变化」");
        assert_eq!(k.offset(), 100.0);
    }

    // ── 触控板跟手 ───────────────────────────────────────────────────────────

    /// 连续触控板输入 1:1 跟手，位置序列与逐点累加严格相等。
    #[test]
    fn trackpad_updates_follow_the_finger_pointwise() {
        let mut k = ScrollKinetics::new(1000.0);
        let mut expected = 0.0f32;
        for dy in [7.0f32, 12.5, 3.25, 40.0, 0.75] {
            expected += dy;
            assert!(k.finger(dy, 8.0));
            assert_close(k.offset(), expected, "跟手位置");
        }
        assert_eq!(k.velocity(), 0.0, "手指未离开前不滑行");
        assert!(!k.is_gliding());
    }

    /// dt=0 的输入位移照旧落位，但不入样本、松手也不会凭空产生速度。
    #[test]
    fn zero_dt_moves_but_does_not_sample() {
        let mut k = ScrollKinetics::new(1000.0);
        assert!(k.finger(10.0, 0.0));
        assert_close(k.offset(), 10.0, "dt=0 位移");
        assert!(!k.release(), "没有时长就没有速度");
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.tick(16.0));
        assert_close(k.offset(), 10.0, "dt=0 后不推进");
    }

    // ── 松手滑行 ─────────────────────────────────────────────────────────────

    /// 松手滑行逐拍与一阶指数闭式解相等（v₀=1250 px/s，每拍 16 ms）。
    ///
    /// 期望值：x(t) = v₀·τ·(1 − e^{−t/τ})，t = k×16 ms；τ = 0.3338 s。
    #[test]
    fn glide_positions_match_first_order_decay() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 6); // 120px / 96ms → 1250 px/s
        assert!(k.release(), "1250 px/s 应启动滑行");
        assert_close(k.velocity(), 1250.0, "松手速度");

        let start = k.offset();
        let tau = INERTIA_TAU_S;
        for step in 1..=20 {
            assert!(k.tick(16.0), "第 {step} 拍应仍在滑行");
            let t = step as f32 * 0.016;
            let expected = start + 1250.0 * tau * (1.0 - (-t / tau).exp());
            assert_close(k.offset(), expected, "滑行位置");
        }
        assert!(k.is_gliding(), "20 拍（320 ms）后速度仍有 ~479 px/s");

        // 继续到静止：总位移 ≈ v₀·τ − v_stop·τ（停止阈值 30 px/s 截掉尾巴）。
        let mut guard = 0;
        while k.is_gliding() && guard < 1000 {
            k.tick(16.0);
            guard += 1;
        }
        assert!(!k.is_gliding(), "必须自行停下");
        assert_eq!(k.velocity(), 0.0);
        // 停止阈值在最后一拍才判：末速度落在 [v_stop·e^{−16ms/τ}, v_stop)，故总位移
        // 与 v₀·τ − v_stop·τ 的差最多一个「停止阈值×τ」的量化（≈10px），此处按 0.5px 收。
        let expected_total = start + 1250.0 * tau - INERTIA_STOP_VELOCITY * tau;
        assert!(
            (k.offset() - expected_total).abs() <= 0.5,
            "滑行总位移：实际 {}，期望 {expected_total}（量化上界 {}）",
            k.offset(),
            INERTIA_STOP_VELOCITY * tau
        );
    }

    /// 按下瞬停：滑行中的一拍之后按下，位置冻住，后续再大的 dt 也不动。
    #[test]
    fn press_stops_glide_immediately() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 6);
        assert!(k.release());
        assert!(k.tick(16.0));

        let frozen = k.offset();
        k.press();
        assert!(!k.is_gliding());
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.tick(100.0), "瞬停后不应再推进");
        assert_close(k.offset(), frozen, "瞬停位置");
    }

    /// 松手后反向输入：立即打断滑行、位置反向跟手，再松手朝反方向滑。
    #[test]
    fn opposite_input_interrupts_and_reverses() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 6);
        assert!(k.release());
        assert!(k.tick(16.0));

        let before = k.offset();
        assert!(k.finger(-30.0, 16.0));
        assert!(!k.is_gliding(), "新输入打断滑行");
        assert_close(k.offset(), before - 30.0, "反向跟手");

        assert!(k.release(), "反方向同样能起甩");
        assert!(k.velocity() < 0.0, "速度方向随手指：{}", k.velocity());
        assert!(k.tick(16.0));
        assert!(k.offset() < before - 30.0, "反向滑行位置应继续变小");
    }

    /// 正反交替等量输入相互抵消：松手速度为 0，不起惯性。
    #[test]
    fn equal_opposite_samples_cancel_to_zero_velocity() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 2);
        flick(&mut k, -20.0, 16.0, 2);
        assert!(!k.release(), "净位移为 0 不应起甩");
        assert_eq!(k.velocity(), 0.0);
    }

    // ── 夹紧 ─────────────────────────────────────────────────────────────────

    /// 撞底：位置精确停在 max，速度清零，之后不回弹（永不越过上限）。
    #[test]
    fn clamp_at_bottom_stops_without_rebound() {
        let mut k = ScrollKinetics::new(100.0);
        flick(&mut k, 30.0, 16.0, 4); // 120px 已越过上限，被夹到 100
        assert_eq!(k.offset(), 100.0, "跟手阶段就夹在底");
        assert!(k.release(), "速度样本仍有效");
        let mut max_seen = k.offset();
        for _ in 0..60 {
            k.tick(16.0);
            max_seen = max_seen.max(k.offset());
        }
        assert!(!k.is_gliding(), "到底后必须停");
        assert_eq!(k.velocity(), 0.0);
        assert_eq!(max_seen, 100.0, "任何时候都不得越过上限（无 overshoot）");
        assert_close(k.offset(), 100.0, "停在底");
    }

    /// 撞顶对称：从底部反向甩回 0，停在 0 不回弹。
    #[test]
    fn clamp_at_top_is_symmetric() {
        let mut k = ScrollKinetics::new(100.0);
        k.wheel(100.0);
        flick(&mut k, -20.0, 16.0, 6);
        assert!(k.release());
        let mut min_seen = k.offset();
        for _ in 0..200 {
            k.tick(16.0);
            min_seen = min_seen.min(k.offset());
        }
        assert_eq!(min_seen, 0.0, "任何时候都不得越过下限");
        assert_eq!(k.offset(), 0.0);
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.is_gliding());
    }

    /// 内容变短（可滚上限缩小）时偏移立即拉回界内、速度清零。
    #[test]
    fn shrinking_max_offset_clamps_and_stops() {
        let mut k = ScrollKinetics::new(1000.0);
        flick(&mut k, 20.0, 16.0, 6);
        assert!(k.release());
        assert!(k.tick(16.0));

        k.set_max_offset(50.0);
        assert_eq!(k.offset(), 50.0);
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.is_gliding());
        assert!(!k.tick(16.0));
    }

    // ── 时间防御 ─────────────────────────────────────────────────────────────

    /// 超长 dt 截断：掉帧 1 s 与正常 100 ms 一拍推进的距离相同（不瞬移）。
    #[test]
    fn long_dt_is_truncated_to_max_tick() {
        let make = || {
            let mut k = ScrollKinetics::new(10_000.0);
            flick(&mut k, 20.0, 16.0, 6);
            assert!(k.release());
            k
        };
        let mut a = make();
        let mut b = make();
        a.tick(1000.0);
        b.tick(MAX_TICK_MS);
        assert_close(a.offset(), b.offset(), "1 s 与 100 ms 应同位移");
        assert_close(a.velocity(), b.velocity(), "速度同样截断");
    }

    /// 采样间隔超过 100 ms：旧样本作废，松手不续上一段的速度。
    #[test]
    fn long_gap_discards_previous_samples() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 3); // 1250 px/s 的快甩
        assert!(k.finger(5.0, MAX_SAMPLE_GAP_MS + 1.0));
        assert!(!k.release(), "断档后的单条慢输入不应起甩");
        assert_eq!(k.velocity(), 0.0);
    }

    /// 极慢输入（25 px/s < 起甩门限 50 px/s）不产生惯性。
    #[test]
    fn slow_input_never_starts_inertia() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 0.4, 16.0, 3);
        assert!(!k.release(), "25 px/s 视为有意停住");
        assert_eq!(k.velocity(), 0.0);
        assert!(!k.tick(16.0));
    }

    /// 速度是时间加权（Σdy / Σdt），不是各样本速度的算术平均。
    ///
    /// 10px/10ms（1000 px/s）+ 10px/30ms（333 px/s）：加权 = 20/40 = 500 px/s，
    /// 等权平均会是 667 px/s —— 后者会让「快甩一下再慢慢拖」误判成快甩。
    #[test]
    fn velocity_is_time_weighted() {
        let mut k = ScrollKinetics::new(10_000.0);
        k.finger(10.0, 10.0);
        k.finger(10.0, 30.0);
        assert!(k.release());
        assert_close(k.velocity(), 500.0, "时间加权速度");
    }

    /// 采样窗口只保留最近 ~100 ms：快甩之后持续慢拖，速度回落到慢拖的值。
    #[test]
    fn sample_window_forgets_old_fast_motion() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 30.0, 16.0, 5);
        flick(&mut k, 2.0, 16.0, 12); // 窗口滑过之后只剩 2px/16ms
        assert!(k.release());
        assert_close(k.velocity(), 125.0, "窗口内只剩慢拖样本");
    }

    /// 伪输入截断：5 ms 扫 100px（20000 px/s）按 6000 px/s 上限起甩。
    #[test]
    fn implausible_flick_speed_is_capped() {
        let mut k = ScrollKinetics::new(1_000_000.0);
        k.finger(100.0, 5.0);
        assert!(k.release());
        assert_close(k.velocity(), MAX_VELOCITY_PX_S, "速度上限");
        assert!(k.tick(16.0));
    }

    /// 无可滚空间（内容不超视口）时不启动滑行，也不因滑行产生位移。
    #[test]
    fn no_scrollable_space_never_glides() {
        let mut k = ScrollKinetics::new(0.0);
        flick(&mut k, 20.0, 16.0, 6);
        assert_eq!(k.offset(), 0.0);
        assert!(!k.release());
        assert!(!k.tick(16.0));
        assert_eq!(k.offset(), 0.0);
    }

    /// 滑行中滚轮介入：立即停住滑行，滚轮步进照常落位。
    #[test]
    fn wheel_during_glide_stops_it() {
        let mut k = ScrollKinetics::new(10_000.0);
        flick(&mut k, 20.0, 16.0, 6);
        assert!(k.release());
        assert!(k.tick(16.0));
        let before = k.offset();

        assert!(k.wheel(48.0));
        assert!(!k.is_gliding());
        assert_eq!(k.velocity(), 0.0);
        assert_close(k.offset(), before + 48.0, "滚轮落位");
    }
}
