// MetroScrollView —— 滚动容器。参 CONTROL_SPEC §42（ScrollView / ScrollPresenter 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/ScrollView/`（ScrollView.idl + ScrollPresenter.cpp）：
// - ScrollableHeight = ExtentHeight − ViewportHeight（内容超视口才可滚）；
// - ScrollMode（Auto/Enabled/Disabled）、ScrollBarVisibility（Auto/Visible/Hidden）；
// - 滚轮 = 逻辑滚动（正典 §Ⅴ：3 行 = 48px，对齐合成器 Axis discrete）；
// - 平滑滚动 = Kanesumi 以 sokuou SpringAnim 实现（UWP 用 Composition 惯性）。
//
// Kanesumi 移植：**纯状态 + 几何**（不持视觉树）。offset 夹紧、scrollbar 拇指/轨道几何、
// 滚轮路由、可选弹簧平滑滚动。宿主渲染内容时以 `content_offset` 平移 + 视口裁剪。

use kanesumi_anim::{Animation, EasingMode, FrictionAnim, MetroAnim, UwpEasing};
use kanesumi_canvas::Scene;
use kanesumi_core::interaction::{
    FINGER_VELOCITY_WINDOW_MS, INERTIA_MIN_START_VELOCITY, INERTIA_STOP_VELOCITY, INERTIA_TAU_S,
    WHEEL_SMOOTH_MS,
};
use kanesumi_core::{Rect, Size};
use kanesumi_element::{Event, Key, ScrollPhase, ScrollSource};

/// 滚轮离散步（正典 §Ⅴ：3 行 × 16px = 48px）。
pub const SCROLL_WHEEL_STEP: f32 = kanesumi_core::interaction::WHEEL_STEP_PX;
/// 滚动条宽度（UWP ScrollBar 常规 8px，桌面 hover 展开 16px；Kanesumi 取 8）。
pub const SCROLLBAR_THICKNESS: f32 = 8.0;
/// 滚动条拇指最小长度（避免内容极长时拇指缩为点）。
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;

/// 滚动模式。对齐 ScrollingScrollMode。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMode {
    Auto,
    Enabled,
    Disabled,
}

/// 滚动条可见性。对齐 ScrollingScrollBarVisibility。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarVisibility {
    Auto,
    Visible,
    Hidden,
}

/// MetroScrollView —— 滚动容器状态机。
///
/// 单轴滚动（主轴 = 传入内容/视口尺寸的对应轴）。`content_size` 由宿主更新；
/// `offset` 由 `scroll_by`/`scroll_to` 驱动（可平滑）；滚动条几何由 `vertical_scrollbar_rect`
/// 计算。内容渲染：宿主平移 `-offset` 后绘制，再以视口裁剪。
pub struct MetroScrollView {
    /// 内容尺寸（宿主维护）。
    pub content_size: Size,
    /// 视口尺寸。
    pub viewport_size: Size,
    /// 滚动偏移（主轴）。
    pub offset: f32,
    /// 滚动模式。
    pub mode: ScrollMode,
    /// 滚动条可见性。
    pub scrollbar_visibility: ScrollBarVisibility,
    /// 是否使用平滑滚动（默认开）。关时滚轮 / 键盘瞬时定位，无惯性。
    pub smooth_scroll: bool,
    /// 滚轮 / 键盘平滑追踪（150 ms UWP 缓出，可中断 / 可累加）。
    wheel: MetroAnim,
    /// 平滑追踪的目标偏移（连滚累加；`MetroAnim` 无 target getter，故自持）。
    wheel_target: f32,
    /// 触摸板松手后的摩擦惯性。
    inertia: FrictionAnim,
    /// 本帧尚未计入速度采样的跟手位移（`End` 前未出帧时兜底计入）。
    finger_pending: f32,
    /// 最近 ~100 ms 的跟手采样，用于估松手速度。
    samples: Vec<FingerSample>,
}

/// 跟手速度采样：一次 `update` 内的位移与对应时长。
#[derive(Debug, Clone, Copy)]
struct FingerSample {
    dt: f64,
    delta: f32,
}

impl PartialEq for MetroScrollView {
    fn eq(&self, other: &Self) -> bool {
        self.content_size == other.content_size
            && self.viewport_size == other.viewport_size
            && self.offset == other.offset
            && self.mode == other.mode
            && self.scrollbar_visibility == other.scrollbar_visibility
            && self.smooth_scroll == other.smooth_scroll
    }
}

impl Default for MetroScrollView {
    fn default() -> Self {
        Self {
            content_size: Size::ZERO,
            viewport_size: Size::ZERO,
            offset: 0.0,
            mode: ScrollMode::Auto,
            scrollbar_visibility: ScrollBarVisibility::Auto,
            smooth_scroll: true,
            wheel: new_wheel_anim(),
            wheel_target: 0.0,
            inertia: FrictionAnim::new(INERTIA_TAU_S as f64, INERTIA_STOP_VELOCITY as f64),
            finger_pending: 0.0,
            samples: Vec::new(),
        }
    }
}

/// 滚轮 / 键盘平滑追踪动画：`WHEEL_SMOOTH_MS` 的 UWP Quadratic 缓出（正典 §Ⅴ）。
fn new_wheel_anim() -> MetroAnim {
    MetroAnim::new(
        WHEEL_SMOOTH_MS as f64 / 1000.0,
        UwpEasing::Quadratic,
        EasingMode::EaseOut,
    )
}

impl MetroScrollView {
    pub fn new(content: Size, viewport: Size) -> Self {
        Self {
            content_size: content,
            viewport_size: viewport,
            ..Self::default()
        }
    }

    /// 可滚主轴长（内容超视口的部分）。对齐 `ScrollableHeight = Extent − Viewport`。
    /// Kanesumi 单轴垂直滚动（主轴 = height）。
    pub fn max_offset(&self) -> f32 {
        (self.content_size.height - self.viewport_size.height).max(0.0)
    }

    /// 内容是否可滚（超视口）。
    pub fn is_scrollable(&self) -> bool {
        self.max_offset() > 0.0 && self.mode != ScrollMode::Disabled
    }

    /// 是否应显示滚动条（Auto = 可滚时显示）。
    pub fn scrollbar_visible(&self) -> bool {
        match self.scrollbar_visibility {
            ScrollBarVisibility::Visible => true,
            ScrollBarVisibility::Hidden => false,
            ScrollBarVisibility::Auto => self.is_scrollable(),
        }
    }

    /// 滚轮滚动（主轴；正 = 向下）。离散步 48px（正典 §Ⅴ）。
    ///
    /// 平滑开时按「目标累加 + 150 ms 缓出追目标」：连滚三格目标 +144、从中途当前呈现值接续；
    /// 平滑关时瞬时定位（旧行为）。Disabled 模式不滚。
    pub fn scroll_wheel(&mut self, dy: f32) {
        if self.mode == ScrollMode::Disabled {
            return;
        }
        if !self.smooth_scroll {
            self.scroll_to(self.offset + dy, false);
            return;
        }
        let base = self.smooth_base();
        self.begin_smooth(base + dy);
    }

    /// 增量滚动（带平滑，对齐旧 `scroll_by` 语义）。
    pub fn scroll_by(&mut self, delta: f32) {
        if !self.smooth_scroll {
            self.scroll_to(self.offset + delta, false);
            return;
        }
        self.begin_smooth(self.smooth_base() + delta);
    }

    /// 直接滚动到目标偏移（夹紧）。`animate=true` 时平滑过渡。
    pub fn scroll_to(&mut self, offset: f32, animate: bool) {
        if animate && self.smooth_scroll {
            self.begin_smooth(offset);
            return;
        }
        let target = offset.clamp(0.0, self.max_offset());
        self.offset = target;
        self.wheel.jump_to(target as f64);
        self.wheel_target = target;
        self.inertia.stop();
        self.samples.clear();
        self.finger_pending = 0.0;
    }

    /// 平滑追踪的起点：动画在跑时用累计目标（连滚不跳变），否则用当前呈现值。
    fn smooth_base(&self) -> f32 {
        if self.wheel.is_steady() {
            self.offset
        } else {
            self.wheel_target
        }
    }

    /// 启动 / 接续平滑追踪到 `target`（夹紧在可滚范围；打断惯性）。
    fn begin_smooth(&mut self, target: f32) {
        let target = target.clamp(0.0, self.max_offset());
        if self.wheel.is_steady() {
            // 从当前呈现值（可能来自惯性 / 跟手）接续，不跳变。
            self.wheel.jump_to(self.offset as f64);
        }
        self.offset = self.wheel.value() as f32;
        self.inertia.stop();
        self.samples.clear();
        self.finger_pending = 0.0;
        self.wheel_target = target;
        self.wheel.set_target(target as f64);
    }

    /// 跳到指定项（`item_main_pos` = 条目主轴起点，`item_extent` = 条目主轴长）。
    pub fn scroll_into_view(&mut self, item_main_pos: f32, item_extent: f32, animate: bool) {
        let viewport = self.viewport_size.height;
        let cur = self.offset;
        let target = if item_main_pos < cur {
            item_main_pos
        } else if item_main_pos + item_extent > cur + viewport {
            item_main_pos + item_extent - viewport
        } else {
            cur
        };
        self.scroll_to(target, animate);
    }

    /// 每帧推进平滑滚动 / 惯性（真实时钟 `dt` 秒）。返回偏移是否变化。
    pub fn update(&mut self, dt: f64) {
        self.step(dt);
    }

    /// 动画推进一步。返回偏移是否变化（调用方据此失效重画）。
    fn step(&mut self, dt: f64) -> bool {
        let before = self.offset;
        // 跟手采样独立于动画：即使无动画也要把本帧累计位移计入速度估计。
        self.record_finger_sample(dt);
        if !self.smooth_scroll {
            return self.offset != before;
        }
        if !self.inertia.is_steady() {
            self.inertia.advance(dt);
            let pos = self.inertia.value() as f32;
            let clamped = pos.clamp(0.0, self.max_offset());
            self.offset = clamped;
            if (clamped - pos).abs() > f32::EPSILON {
                // 撞边界即停（不做橡皮筋回弹；`w-comp-ref` §①）。
                self.inertia.stop();
            }
        } else if !self.wheel.is_steady() {
            self.wheel.update(dt);
            self.offset = self.wheel.value() as f32;
        }
        self.offset != before
    }

    /// 是否正在动画（滚轮平滑追踪或惯性）。
    pub fn is_animating(&self) -> bool {
        self.smooth_scroll && (!self.wheel.is_steady() || !self.inertia.is_steady())
    }

    // ── 触控板：跟手 + 松手惯性 ──────────────────────────────────────────────

    /// 触控板 / 连续源的位置增量：**跟手**（呈现值 1:1 直加，无动画），
    /// 并累计本帧位移用于估松手速度。
    pub fn scroll_finger(&mut self, dy: f32) {
        if self.mode == ScrollMode::Disabled {
            return;
        }
        // 新输入立即打断惯性与滚轮平滑追踪。
        self.inertia.stop();
        if !self.wheel.is_steady() {
            self.wheel.jump_to(self.offset as f64);
            self.wheel_target = self.offset;
        }
        let before = self.offset;
        self.offset = (self.offset + dy).clamp(0.0, self.max_offset());
        self.finger_pending += self.offset - before;
    }

    /// 手指离开（`End`）：按估计速度决定是否续惯性。
    pub fn scroll_end(&mut self) {
        // 未出帧的剩余位移按最近一次帧间隔兜底计入（否则松手速度偏小）。
        if self.finger_pending != 0.0 {
            let dt = self.samples.last().map(|s| s.dt).unwrap_or(1.0 / 60.0);
            self.push_sample(dt);
        }
        let v = self.estimated_velocity();
        // 撞边界由 `step` 夹紧后即停（`max_offset==0` 时不启动，无可滚空间）。
        let start = self.smooth_scroll
            && v.abs() > INERTIA_MIN_START_VELOCITY
            && self.is_scrollable();
        if start {
            self.inertia.start(self.offset as f64, v as f64);
        }
        self.samples.clear();
    }

    /// 估计最近 ~100 ms 的跟手速度（px/s，正 = 向下）。
    pub fn estimated_velocity(&self) -> f32 {
        let total_dt: f64 = self.samples.iter().map(|s| s.dt).sum();
        if total_dt <= 0.0 {
            return 0.0;
        }
        let total: f32 = self.samples.iter().map(|s| s.delta).sum();
        (total as f64 / total_dt) as f32
    }

    fn record_finger_sample(&mut self, dt: f64) {
        if self.finger_pending != 0.0 {
            self.push_sample(dt);
        }
    }

    fn push_sample(&mut self, dt: f64) {
        let delta = std::mem::take(&mut self.finger_pending);
        self.samples.push(FingerSample {
            dt: dt.max(1e-6),
            delta,
        });
        let window = FINGER_VELOCITY_WINDOW_MS as f64 / 1000.0;
        let mut acc = 0.0;
        let mut keep = self.samples.len();
        for (i, s) in self.samples.iter().enumerate().rev() {
            acc += s.dt;
            if acc > window {
                keep = i + 1;
                break;
            }
        }
        if keep < self.samples.len() {
            self.samples.rotate_left(keep);
            self.samples.truncate(self.samples.len() - keep);
        }
    }

    // ── 键盘：PageUp / PageDown / Home / End ─────────────────────────────────

    /// PageUp / PageDown：步长 = 视口高 − 一行（正典 §Ⅴ）。`dir` = ±1。
    pub fn scroll_page(&mut self, dir: f32) {
        let step = (self.viewport_size.height - SCROLL_WHEEL_STEP).max(0.0);
        if !self.smooth_scroll {
            self.scroll_to(self.offset + dir * step, false);
        } else {
            self.begin_smooth(self.smooth_base() + dir * step);
        }
    }

    /// Home / End：绝对跳到顶 / 底，走与滚轮相同的平滑追踪。
    pub fn scroll_home(&mut self) {
        self.scroll_to_smooth(0.0);
    }

    /// End：底部。
    pub fn scroll_end_key(&mut self) {
        self.scroll_to_smooth(self.max_offset());
    }

    /// 平滑开时走平滑追踪，否则瞬时定位。
    fn scroll_to_smooth(&mut self, target: f32) {
        if self.smooth_scroll {
            self.begin_smooth(target);
        } else {
            self.scroll_to(target, false);
        }
    }

    /// 滚动条轨道矩形（主轴 = 垂直滚动条，右缘 8px 宽）。
    pub fn scrollbar_track_rect(&self) -> Rect {
        let v = self.viewport_size;
        Rect::new(
            v.width - SCROLLBAR_THICKNESS,
            0.0,
            SCROLLBAR_THICKNESS,
            v.height,
        )
    }

    /// 滚动条拇指矩形。大小 = 视口/内容比例 × 轨道长，下限 SCROLLBAR_MIN_THUMB；
    /// 位置 = 偏移/内容长 × 轨道长。
    pub fn scrollbar_thumb_rect(&self) -> Rect {
        let track = self.scrollbar_track_rect();
        let max = self.max_offset();
        if max <= 0.0 {
            return Rect::new(track.origin.x, track.origin.y, track.size.width, 0.0);
        }
        let ratio = track.size.height / (self.content_size.height.max(1.0));
        let thumb_h = (ratio * track.size.height).max(SCROLLBAR_MIN_THUMB);
        let travel = (track.size.height - thumb_h).max(0.0);
        let y = travel * (self.offset / max);
        Rect::new(
            track.origin.x + 1.0,
            track.origin.y + y,
            track.size.width - 2.0,
            thumb_h,
        )
    }

    /// 内容平移偏移（渲染内容前应用）。
    pub fn content_offset(&self) -> f32 {
        -self.offset
    }
}

// ── 元素树接入：滚动容器（参 docs/ELEMENT_TREE.md §Ⅹ E4 / ROADMAP M5-1）────────────
//
// 「任何内容放不下就有滚动」的统一容器：子节点以视口宽、无界高量测，按 `-offset` 排在
// 视口之外（`scrolls_children`），框架的容器裁剪同时约束绘制与命中（画不到、也点不到
// 视口外的子节点）。滚动条画在内容之上（`paint_after`）。键盘焦点落到视口外的后代时，
// 框架经 `bring_into_view` 让本容器把它滚进来（XAML BringIntoView）。
//
// 注意（与 XAML 同）：放进 `Stack::column` 且不给 `grow` 时，可用高度是无界的 ——
// 视口会长到与内容一样高，也就无从滚动。滚动容器需要一个有界的高度（grow / 固定高 / 根）。

impl kanesumi_element::Widget for MetroScrollView {
    fn measure(&mut self, ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        let mut content = Size::ZERO;
        for c in ctx.children() {
            let d = ctx.measure_child(c, Size::new(available.width, f32::INFINITY));
            content = Size::new(content.width.max(d.width), content.height.max(d.height));
        }
        self.content_size = content;
        Size::new(
            content.width.min(available.width),
            content.height.min(available.height),
        )
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.viewport_size = rect.size;
        // 视口变大 / 内容变短后，旧偏移可能越界 → 夹紧（不做动画）。
        let max = self.max_offset();
        if self.offset > max {
            self.scroll_to(max, false);
        }
        let content_h = self.content_size.height.max(rect.size.height);
        for c in ctx.children() {
            ctx.arrange_child(
                c,
                Rect::new(
                    rect.origin.x,
                    rect.origin.y - self.offset,
                    rect.size.width,
                    content_h,
                ),
            );
        }
    }

    fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut Scene) {}

    /// 滚动条（叠在内容之上）。拇指几何取自 `scrollbar_thumb_rect`（视口本地 → 表面坐标）。
    fn paint_after(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        if !self.scrollbar_visible() {
            return;
        }
        let rect = ctx.rect();
        let t = self.scrollbar_thumb_rect();
        let thumb = Rect::new(
            rect.origin.x + t.origin.x,
            rect.origin.y + t.origin.y,
            t.size.width,
            t.size.height,
        );
        let theme = ctx.theme();
        let color = theme
            .colors
            .on_surface_variant
            .with_alpha(theme.indication.base_medium_low);
        scene.fill_rect(color, thumb);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &Event) {
        match event {
            Event::Scroll {
                dy,
                source,
                phase,
                ..
            } => {
                // 不可滚时不截停：滚动链交给外层容器（XAML ScrollChaining）。
                if !self.is_scrollable() {
                    return;
                }
                let before = self.offset;
                // 跟手帧需下一帧 `update` 采样估速（`update` 只对本节点在 `anim` 时调用）。
                let mut needs_tick = false;
                match source {
                    ScrollSource::Wheel { .. } => {
                        if *phase == ScrollPhase::Update {
                            self.scroll_wheel(*dy);
                        }
                    }
                    ScrollSource::Finger | ScrollSource::Continuous => {
                        if *phase == ScrollPhase::End {
                            self.scroll_end();
                        } else {
                            self.scroll_finger(*dy);
                            needs_tick = true;
                        }
                    }
                }
                if self.offset != before {
                    ctx.invalidate_arrange();
                    ctx.invalidate_paint();
                    ctx.emit(ScrollOffsetChanged(self.offset));
                }
                if self.is_animating() || needs_tick {
                    ctx.request_anim_frame();
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } if self.is_scrollable() => {
                let handled = match key {
                    Key::PageUp => {
                        self.scroll_page(-1.0);
                        true
                    }
                    Key::PageDown => {
                        self.scroll_page(1.0);
                        true
                    }
                    Key::Home => {
                        self.scroll_home();
                        true
                    }
                    Key::End => {
                        self.scroll_end_key();
                        true
                    }
                    _ => false,
                };
                if handled {
                    ctx.invalidate_arrange();
                    ctx.invalidate_paint();
                    if self.is_animating() {
                        ctx.request_anim_frame();
                    }
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, dt: f64) {
        let before = self.offset;
        self.step(dt);
        if self.offset != before {
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
        }
        if self.is_animating() {
            ctx.request_anim_frame();
        }
    }

    fn wants_anim(&self) -> bool {
        self.is_animating()
    }

    fn scrolls_children(&self) -> bool {
        true
    }

    fn bring_into_view(&mut self, ctx: &mut kanesumi_element::EventCtx, target: Rect) -> bool {
        let rect = ctx.rect();
        // 表面坐标 → 内容坐标（内容原点 = 视口原点 − offset）。
        let pos = target.origin.y - rect.origin.y + self.offset;
        let before = self.offset;
        self.scroll_into_view(pos, target.size.height, false);
        if self.offset != before {
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
            ctx.emit(ScrollOffsetChanged(self.offset));
            true
        } else {
            false
        }
    }
}

/// 元素树动作：滚动偏移变化（虚拟化列表据此决定实现哪些项）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollOffsetChanged(pub f32);

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::button::MetroButton;
    use kanesumi_core::Point;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Stack;
    use kanesumi_element::{Align, Key, LayoutProps, Modifiers, ScrollInput, ScrollPhase, ScrollSource, WidgetId};

    /// 视口 200×100，内容 = 10 个 40 高的按钮（共 400）。
    fn harness() -> (TestHarness, WidgetId, Vec<WidgetId>) {
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = MetroScrollView {
            smooth_scroll: false,
            ..MetroScrollView::default()
        };
        let sv = h.tree.insert_with(
            h.root(),
            sv,
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        let items = (0..10)
            .map(|i| {
                h.tree.insert_with(
                    col,
                    MetroButton::new(format!("项 {i}")),
                    LayoutProps {
                        height: Some(40.0),
                        ..LayoutProps::default()
                    },
                )
            })
            .collect();
        h.frame();
        (h, sv, items)
    }

    #[test]
    fn wheel_scrolls_and_clamps() {
        let (mut h, sv, items) = harness();
        assert_eq!(h.rect(items[0]).origin.y, 0.0);
        h.tree.scroll(Point::new(50.0, 50.0), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(h.rect(items[0]).origin.y, -50.0);
        assert_eq!(h.take::<ScrollOffsetChanged>(), vec![(sv, ScrollOffsetChanged(50.0))]);
        for _ in 0..20 {
            h.tree.scroll(Point::new(50.0, 50.0), 0.0, 50.0, Modifiers::NONE);
        }
        h.frame();
        assert_eq!(h.tree.get::<MetroScrollView>(sv).unwrap().offset, 300.0, "夹紧到内容 − 视口");
        assert_eq!(h.rect(items[9]).bottom(), 100.0, "末项贴视口底");
    }

    #[test]
    fn content_outside_viewport_is_neither_hit_nor_contained_violation() {
        let (mut h, sv, items) = harness();
        h.tree.scroll(Point::new(50.0, 50.0), 0.0, 60.0, Modifiers::NONE);
        h.frame();
        // 项 1 的上半截（y ∈ [-20, 0)）被滚出视口：视口外不命中。
        assert_eq!(h.tree.hit(Point::new(50.0, 105.0)), None, "视口下方不命中内容");
        assert_eq!(h.tree.hit(Point::new(50.0, 5.0)), Some(items[1]));
        h.assert_contained();
        h.assert_no_hit_outside(sv);
    }

    #[test]
    fn tab_brings_offscreen_item_into_view() {
        let (mut h, sv, items) = harness();
        for _ in 0..5 {
            h.tab();
        }
        assert_eq!(h.tree.focused(), Some(items[4]));
        let r = h.rect(items[4]);
        assert!(r.origin.y >= 0.0 && r.bottom() <= 100.0, "焦点项在视口内 {r:?}");
        assert!(h.tree.get::<MetroScrollView>(sv).unwrap().offset > 0.0);
    }

    #[test]
    fn unscrollable_content_lets_wheel_bubble() {
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert(h.root(), MetroScrollView::default());
        h.tree.insert(sv, MetroButton::new("短"));
        h.frame();
        h.tree.scroll(Point::new(10.0, 10.0), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert!(h.take::<ScrollOffsetChanged>().is_empty());
        assert_eq!(h.tree.get::<MetroScrollView>(sv).unwrap().offset, 0.0);
    }

    #[test]
    fn scrollbar_is_drawn_over_content_when_scrollable() {
        use kanesumi_canvas::SceneCommand;
        let (h, sv, _) = harness();
        let cmds = &h.last.scene.commands;
        let thumb_x = h.rect(sv).right() - SCROLLBAR_THICKNESS + 1.0;
        let thumb = cmds
            .iter()
            .position(|c| matches!(c, SceneCommand::FillRect { rect, .. } if rect.origin.x == thumb_x))
            .expect("可滚时应画出滚动条拇指");
        let last_text = cmds
            .iter()
            .rposition(|c| matches!(c, SceneCommand::Text { .. }))
            .unwrap();
        assert!(thumb > last_text, "拇指画在全部内容之后（叠在上层）");
    }

    // ── 元素树路径：`Tree::scroll` / `scroll_ex`（TestHarness 推进虚拟时钟）─────────

    /// 视口 200×100、内容 400，平滑滚保持默认开（与 `harness` 相对）。
    fn harness_smooth() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        for i in 0..10 {
            h.tree.insert_with(
                col,
                MetroButton::new(format!("项 {i}")),
                LayoutProps {
                    height: Some(40.0),
                    ..LayoutProps::default()
                },
            );
        }
        h.frame();
        (h, sv)
    }

    /// 旧 `Tree::scroll` 走 `Wheel`/`Update`：经元素树平滑追踪，连滚三格到 144。
    #[test]
    fn tree_wheel_scroll_smooths_and_settles() {
        let (mut h, sv) = harness_smooth();
        for _ in 0..3 {
            h.tree
                .scroll(Point::new(50.0, 50.0), 0.0, 48.0, Modifiers::NONE);
        }
        h.settle();
        let off = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!(
            (off - 144.0).abs() < 1.0,
            "连滚三格平滑到 144，实际 {off}"
        );
    }

    /// `scroll_ex` 触控板跟手 1:1；`End` 后惯性继续前移（帧间采样估速）。
    #[test]
    fn tree_finger_then_release_inertia_extends_scroll() {
        let (mut h, sv) = harness_smooth();
        let p = Point::new(50.0, 50.0);
        for _ in 0..4 {
            h.tree.scroll_ex(
                p,
                ScrollInput::new(
                    0.0,
                    15.0,
                    ScrollSource::Finger,
                    ScrollPhase::Update,
                    Modifiers::NONE,
                ),
            );
            h.frame();
        }
        let at_end = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!((at_end - 60.0).abs() < 0.5, "跟手到 60，实际 {at_end}");
        h.tree.scroll_ex(
            p,
            ScrollInput::end(ScrollSource::Finger, Modifiers::NONE),
        );
        h.settle();
        let off = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!(off > at_end + 1.0, "松手惯性继续前移 {at_end} → {off}");
    }

    /// PageDown 键经焦点后代冒泡到滚动容器：位移 = 视口高 − 48。
    #[test]
    fn tree_page_down_key_scrolls_viewport_minus_line() {
        let (mut h, sv, items) = harness();
        h.tree.focus(items[0], true);
        h.frame();
        let handled = h.key(Key::PageDown);
        assert!(handled, "PageDown 被滚动容器消费");
        h.settle();
        let off = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!(
            (off - (100.0 - SCROLL_WHEEL_STEP)).abs() < 1.0,
            "PageDown = 视口高 − 48，实际 {off}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_offset_clamps_to_zero() {
        let sv = MetroScrollView::new(Size::new(200.0, 100.0), Size::new(200.0, 100.0));
        assert_eq!(sv.max_offset(), 0.0, "内容 ≤ 视口不可滚");
        let sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        assert_eq!(sv.max_offset(), 200.0, "内容 300 视口 100 → 可滚 200");
    }

    #[test]
    fn wheel_step_matches_canon() {
        assert_eq!(SCROLL_WHEEL_STEP, 48.0, "正典 §Ⅴ：3 行 × 16px");
    }

    #[test]
    fn scroll_wheel_accumulates_and_clamps() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_wheel(50.0);
        assert_eq!(sv.offset, 50.0);
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 150.0);
        // 超界夹紧
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 200.0, "夹紧到 max_offset");
    }

    #[test]
    fn scroll_to_clamps() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(500.0, false);
        assert_eq!(sv.offset, 200.0);
        sv.scroll_to(-10.0, false);
        assert_eq!(sv.offset, 0.0);
    }

    #[test]
    fn smooth_scroll_animates_then_settles() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        assert!(sv.smooth_scroll);
        sv.scroll_to(100.0, true);
        assert!(sv.is_animating(), "平滑滚动进行中");
        for _ in 0..600 {
            sv.update(1.0 / 60.0);
        }
        assert!(!sv.is_animating());
        assert!((sv.offset - 100.0).abs() < 1.0, "稳定到目标，实际 {}", sv.offset);
    }

    #[test]
    fn scroll_into_view_brings_item_into_viewport() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        // 条目 250..290 不在 [0,100) → 滚到 250+40-100=190
        sv.scroll_into_view(250.0, 40.0, false);
        assert_eq!(sv.offset, 190.0);
        // 条目在视口内 → 不滚
        sv.scroll_into_view(200.0, 40.0, false);
        assert_eq!(sv.offset, 190.0, "已在视口内不滚动");
        // 条目在视口上方 → 回滚到其上缘
        sv.scroll_into_view(30.0, 40.0, false);
        assert_eq!(sv.offset, 30.0, "视口上方条目回滚到上缘");
    }

    #[test]
    fn scrollbar_thumb_geometry() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(0.0, false);
        let thumb0 = sv.scrollbar_thumb_rect();
        assert_eq!(thumb0.size.width, SCROLLBAR_THICKNESS - 2.0);
        assert_eq!(thumb0.origin.y, 0.0, "offset 0 拇指在顶");
        // 滚到中间 → 拇指下移
        sv.scroll_to(150.0, false);
        let thumb_mid = sv.scrollbar_thumb_rect();
        assert!(
            thumb_mid.origin.y > thumb0.origin.y,
            "滚动后拇指下移"
        );
        // 拇指高度 = 100/400 * 100 = 25（> min 24）
        assert!((thumb_mid.size.height - 25.0).abs() < 1.0);
    }

    #[test]
    fn scrollbar_thumb_min_size() {
        // 极长内容：拇指缩到最小
        let mut sv = MetroScrollView::new(Size::new(200.0, 5000.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        let thumb = sv.scrollbar_thumb_rect();
        assert_eq!(thumb.size.height, SCROLLBAR_MIN_THUMB, "拇指不小于 24");
    }

    #[test]
    fn auto_scrollbar_only_when_scrollable() {
        let small = MetroScrollView::new(Size::new(200.0, 80.0), Size::new(200.0, 100.0));
        assert!(!small.scrollbar_visible(), "内容不足不显示滚动条");
        let big = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        assert!(big.scrollbar_visible());
    }

    #[test]
    fn disabled_mode_blocks_scroll() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.mode = ScrollMode::Disabled;
        sv.smooth_scroll = false;
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 0.0, "Disabled 不滚动");
    }

    #[test]
    fn content_offset_negates_scroll() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(100.0, false);
        assert_eq!(sv.content_offset(), -100.0);
    }

    // ── 滚动输入语义（k-scroll-input）：滚轮平滑 / 触控板跟手 + 惯性 / Page 键 ──

    /// 连滚三格：目标累加 144，呈现值单调不回跳，150 ms（9 帧）后到位。
    #[test]
    fn wheel_three_steps_smooth_monotonic_to_144() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        assert!(sv.smooth_scroll);
        sv.scroll_wheel(48.0);
        sv.scroll_wheel(48.0);
        sv.scroll_wheel(48.0);
        let mut prev = sv.offset;
        let mut curve = vec![prev];
        for _ in 0..9 {
            sv.update(1.0 / 60.0);
            assert!(
                sv.offset >= prev - 1e-4,
                "呈现值不得回跳：{prev} → {}",
                sv.offset
            );
            prev = sv.offset;
            curve.push(prev);
        }
        assert!(!sv.is_animating(), "150 ms 后应稳态");
        assert!(
            (sv.offset - 144.0).abs() < 0.5,
            "连滚三格到 144，实际 {}",
            sv.offset
        );
        assert!(
            curve.windows(2).any(|w| w[1] > w[0]),
            "缓出应有中间帧，曲线 {curve:?}"
        );
    }

    /// 中途再滚：从当前呈现值接续（不跳变），目标继续累加。
    #[test]
    fn wheel_append_midway_continues_without_jump() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.scroll_wheel(48.0);
        for _ in 0..3 {
            sv.update(1.0 / 60.0);
        }
        let mid = sv.offset;
        assert!(mid > 0.0 && mid < 48.0, "中途呈现值 {mid}");
        sv.scroll_wheel(48.0);
        assert!(sv.offset >= mid - 1e-4, "追加不回跳 {mid} → {}", sv.offset);
        for _ in 0..12 {
            sv.update(1.0 / 60.0);
        }
        assert!(
            (sv.offset - 96.0).abs() < 0.5,
            "追加后目标 96，实际 {}",
            sv.offset
        );
    }

    /// 触控板位置增量 1:1 跟手（无动画），并夹紧在可滚范围。
    #[test]
    fn finger_input_is_one_to_one() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.scroll_finger(7.0);
        assert_eq!(sv.offset, 7.0, "跟手 1:1");
        sv.scroll_finger(-3.0);
        assert_eq!(sv.offset, 4.0);
        sv.scroll_finger(-100.0);
        assert_eq!(sv.offset, 0.0, "上界夹紧");
        sv.scroll_finger(1000.0);
        assert_eq!(sv.offset, sv.max_offset(), "下界夹紧");
    }

    /// 松手速度 > 阈值 → 惯性前移；速度越大距离越远（同 τ / 阈值）。
    #[test]
    fn finger_release_inertia_distance_monotonic_in_velocity() {
        let travel = |per_frame: f32| {
            let mut sv = MetroScrollView::new(Size::new(200.0, 100_000.0), Size::new(200.0, 100.0));
            for _ in 0..4 {
                sv.scroll_finger(per_frame);
                sv.update(1.0 / 60.0);
            }
            let start = sv.offset;
            sv.scroll_end();
            assert!(sv.is_animating(), "松手速度 {per_frame}/帧 应启惯性");
            for _ in 0..2000 {
                sv.update(1.0 / 60.0);
                if !sv.is_animating() {
                    break;
                }
            }
            sv.offset - start
        };
        let slow = travel(1.0);
        let fast = travel(4.0);
        assert!(slow > 0.0, "低速也要有惯性位移");
        assert!(fast > slow, "惯性距离随速度单调：{slow} vs {fast}");
    }

    /// 松手速度低于 50 px/s 不启惯性（有意停住）。
    #[test]
    fn finger_release_below_threshold_no_inertia() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 100_000.0), Size::new(200.0, 100.0));
        for _ in 0..4 {
            sv.scroll_finger(0.5);
            sv.update(1.0 / 60.0);
        }
        sv.scroll_end();
        assert!(!sv.is_animating(), "30 px/s < 50 阈值，不启惯性");
    }

    /// 惯性撞边界即停（不做橡皮筋回弹）。
    #[test]
    fn inertia_stops_at_boundary() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 160.0), Size::new(200.0, 100.0));
        sv.scroll_finger(50.0);
        sv.update(1.0 / 60.0);
        sv.scroll_finger(50.0);
        sv.update(1.0 / 60.0);
        sv.scroll_end();
        for _ in 0..600 {
            sv.update(1.0 / 60.0);
            if !sv.is_animating() {
                break;
            }
        }
        assert!(!sv.is_animating(), "撞底后停");
        assert_eq!(sv.offset, sv.max_offset(), "停在边界，不回弹");
    }

    /// 新输入立即打断惯性。
    #[test]
    fn new_input_interrupts_inertia() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 100_000.0), Size::new(200.0, 100.0));
        for _ in 0..4 {
            sv.scroll_finger(20.0);
            sv.update(1.0 / 60.0);
        }
        sv.scroll_end();
        assert!(sv.is_animating(), "惯性进行中");
        sv.update(1.0 / 60.0);
        sv.scroll_finger(5.0);
        assert!(!sv.is_animating(), "跟手输入打断惯性");
    }

    /// PageDown 位移 = 视口高 − 一行（48）；Home / End 到顶底（走平滑追踪）。
    #[test]
    fn page_down_moves_viewport_minus_line() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 5000.0), Size::new(200.0, 100.0));
        sv.scroll_page(1.0);
        for _ in 0..12 {
            sv.update(1.0 / 60.0);
        }
        assert!(
            (sv.offset - (100.0 - SCROLL_WHEEL_STEP)).abs() < 0.5,
            "PageDown = 视口高 − 48，实际 {}",
            sv.offset
        );
        sv.scroll_end_key();
        for _ in 0..600 {
            sv.update(1.0 / 60.0);
            if !sv.is_animating() {
                break;
            }
        }
        assert!((sv.offset - sv.max_offset()).abs() < 0.5, "End 到底");
        sv.scroll_home();
        for _ in 0..600 {
            sv.update(1.0 / 60.0);
            if !sv.is_animating() {
                break;
            }
        }
        assert!(sv.offset.abs() < 0.5, "Home 到顶");
    }
}
