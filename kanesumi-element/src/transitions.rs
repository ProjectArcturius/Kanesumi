// transitions.rs —— 元素树主题转场库。参 ANIMATION_SPEC §Ⅱ / §Ⅳ / §Ⅴ、ELEMENT_TREE「转场」。
//
// 把 UWP 的主题转场（Entrance / Content / DrillIn·Out / Reposition / PopIn·Out）落成**图层命令**：
// 给节点 `set_layer`，放初值（`Offset` / `Opacity`），再 `animate_layer` 到终值；动画由合成器
// 按自己的时钟推进，**动画期间树不重画、不量测**（参 layer.rs）。容器传播（stagger）对直接子项
// 依次发，顺序 = children 顺序（ANIMATION_SPEC §Ⅱ 的实测提醒）。
//
// 中断：同一节点再次 `play_transition` 时，从**当前呈现值**接续 —— 用 `kanesumi_anim::bezier_y`
// 反解已过时间的缓动进度估算当前视觉值（同 Arch Settings `set-m1-motion` 的 `bezier_y`）。
//
// 生命周期：外壳回报 `Done` / `Cancelled`（经 harness `TreeApp::on_layer_event` 转达）后，
// `keep_layer == false` 的图层被撤销（参 `LayerOutcome`）。

use std::time::Instant;

use kanesumi_anim::{CURVE_EASE_OUT_CUBIC, CURVE_UWP_CLOSE, CURVE_UWP_OPEN, bezier_y};
use kanesumi_core::Point;

use crate::id::WidgetId;
use crate::layer::LayerAnimSpec;
use crate::tree::Tree;

/// 容器传播的错位上限：第 `stagger` 个之后的子项与第 `STAGGER_MAX_ITEMS` 个同时。
pub const STAGGER_MAX_ITEMS: usize = 10;

/// 弹层进出的锚点方位（从哪一侧滑入 / 滑出）。参 `PopupSide`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}

impl Edge {
    /// 沿该方位的外部 → 元素方向的偏移单位向量（初值方向）。
    fn unit(self) -> (f32, f32) {
        match self {
            Edge::Top => (0.0, -1.0),
            Edge::Bottom => (0.0, 1.0),
            Edge::Left => (-1.0, 0.0),
            Edge::Right => (1.0, 0.0),
        }
    }
}

/// 一次转场的词汇。参 ANIMATION_SPEC §Ⅱ 的场景列。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transition {
    /// 内容**首次**出现：下移 + 透明 → 原位不透明。`stagger` 为真时对 `id` 的直接子项
    /// 依次错位（`delay = i × stagger_ms`，上限 `STAGGER_MAX_ITEMS`）；无子项时退回对 `id` 自身。
    Entrance { stagger: bool },
    /// 内容**变化**（非首次）：同 Entrance，位移更小、无 stagger。
    Content,
    /// 层级前进：新内容从略缩小 + 透明放大到位（图层无缩放，用位移 + 透明近似）。
    DrillIn,
    /// 层级后退：当前内容反向离场（透明 + 位移）。
    DrillOut,
    /// 元素被移到新位置：从旧位置平移到新位置（导航选中条、列表重排）。
    Reposition { from: Point },
    /// 弹层出现：从锚点方位滑入 + 淡入。
    PopIn { edge: Edge },
    /// 弹层关闭：向锚点方位滑出 + 淡出。
    PopOut { edge: Edge },
}

/// 转场参数。每种转场有 `const` 缺省（见 `TransitionParams::for_transition`），
/// 缺省取值来源逐条见 ANIMATION_SPEC §Ⅴ「转场缺省值」。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionParams {
    pub duration_ms: u32,
    pub delay_ms: u32,
    /// 容器传播的相邻子项错位步长。
    pub stagger_ms: u32,
    /// 位移幅度（逻辑像素，方向由转场类型决定）。
    pub offset: f32,
    /// cubic-bezier (x1, y1, x2, y2)。
    pub curve: [f32; 4],
    /// 终值后是否保留图层。`false`（默认）= 收到 `Done` / `Cancelled` 后撤图层。
    pub keep_layer: bool,
}

/// 转场句柄：本次转场的身份与起始时刻。`start` 供调用方估算当前视觉值（中断接续）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransitionHandle {
    pub id: WidgetId,
    pub serial: u32,
    pub start: Instant,
    pub keep_layer: bool,
}

/// 图层动画的终态回报（元素侧镜像，避免元素树依赖 harness）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerOutcome {
    Done,
    Cancelled,
}

/// 树内一条进行中的转场（中断 / 估算用）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct ActiveTransition {
    pub(crate) serial: u32,
    pub(crate) start: Instant,
    pub(crate) delay_ms: f64,
    pub(crate) duration_ms: f64,
    pub(crate) curve: [f32; 4],
    pub(crate) from_offset: Point,
    pub(crate) from_opacity: f32,
    pub(crate) to_offset: Point,
    pub(crate) to_opacity: f32,
    pub(crate) keep_layer: bool,
}

/// 图层尚未 Create 时暂存的起始命令；`sync_layers` 建层后由 `flush_pending_starts` 发出。
#[derive(Debug, Clone, Copy)]
pub(crate) struct PendingStart {
    pub(crate) offset: Point,
    pub(crate) opacity: f32,
    pub(crate) animate: LayerAnimSpec,
}

impl TransitionParams {
    /// Entrance 缺省：300ms / stagger 30ms / 下移 40px / UWP 开曲线。
    /// 对齐 Arch Settings `set-m1-motion` 手写值（入场 300ms、stagger 30ms、下移 40px）。
    pub const ENTRANCE: Self = Self {
        duration_ms: 300,
        delay_ms: 0,
        stagger_ms: 30,
        offset: 40.0,
        curve: CURVE_UWP_OPEN,
        keep_layer: false,
    };

    /// Content 缺省：同 Entrance 时长，位移更小（16px）、stagger 归零。
    pub const CONTENT: Self = Self {
        duration_ms: 300,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 16.0,
        curve: CURVE_UWP_OPEN,
        keep_layer: false,
    };

    /// DrillIn 缺省：300ms / 位移 16px / UWP 开曲线（图层无缩放，位移 + 透明近似）。
    pub const DRILL_IN: Self = Self {
        duration_ms: 300,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 16.0,
        curve: CURVE_UWP_OPEN,
        keep_layer: false,
    };

    /// DrillOut 缺省：200ms / 位移 16px / UWP 关曲线（离场快于入场）。
    pub const DRILL_OUT: Self = Self {
        duration_ms: 200,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 16.0,
        curve: CURVE_UWP_CLOSE,
        keep_layer: false,
    };

    /// Reposition 缺省：150ms / 立方缓出。一手源：UWP `RepositionThemeAnimation` 0.15s（参 CONTROL_SPEC §10）。
    pub const REPOSITION: Self = Self {
        duration_ms: 150,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 0.0,
        curve: CURVE_EASE_OUT_CUBIC,
        keep_layer: false,
    };

    /// PopIn 缺省：300ms / 位移 24px / UWP 开曲线。一手源：UWP CommandBarFlyout OpeningStoryboard 300ms。
    pub const POP_IN: Self = Self {
        duration_ms: 300,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 24.0,
        curve: CURVE_UWP_OPEN,
        keep_layer: false,
    };

    /// PopOut 缺省：150ms / 位移 24px / UWP 关曲线。一手源：同源 150ms（参 presets DURATION_SHEET_DISMISS）。
    pub const POP_OUT: Self = Self {
        duration_ms: 150,
        delay_ms: 0,
        stagger_ms: 0,
        offset: 24.0,
        curve: CURVE_UWP_CLOSE,
        keep_layer: false,
    };

    /// 按转场词汇取缺省参数。
    pub const fn for_transition(t: Transition) -> Self {
        match t {
            Transition::Entrance { .. } => Self::ENTRANCE,
            Transition::Content => Self::CONTENT,
            Transition::DrillIn => Self::DRILL_IN,
            Transition::DrillOut => Self::DRILL_OUT,
            Transition::Reposition { .. } => Self::REPOSITION,
            Transition::PopIn { .. } => Self::POP_IN,
            Transition::PopOut { .. } => Self::POP_OUT,
        }
    }
}

impl Transition {
    fn propagates_to_children(self) -> bool {
        matches!(self, Transition::Entrance { stagger: true })
    }

    /// 解析本次转场的初值 / 终值（偏移 + 不透明度）。`rect_origin` = 节点当前布局原点
    /// （`Reposition` 由 `from - rect_origin` 求旧位偏移）。
    fn resolve(self, p: TransitionParams, rect_origin: Point) -> Resolved {
        let d = p.offset;
        match self {
            Transition::Entrance { .. } | Transition::Content => Resolved {
                from_offset: Point::new(0.0, d),
                from_opacity: 0.0,
                to_offset: Point::ORIGIN,
                to_opacity: 1.0,
            },
            // 图层 API 无缩放：用「下方偏移 + 透明」近似「略缩小放大到位」。
            Transition::DrillIn => Resolved {
                from_offset: Point::new(0.0, d),
                from_opacity: 0.0,
                to_offset: Point::ORIGIN,
                to_opacity: 1.0,
            },
            Transition::DrillOut => Resolved {
                from_offset: Point::ORIGIN,
                from_opacity: 1.0,
                to_offset: Point::new(0.0, d),
                to_opacity: 0.0,
            },
            Transition::Reposition { from } => Resolved {
                from_offset: Point::new(
                    from.x - rect_origin.x,
                    from.y - rect_origin.y,
                ),
                from_opacity: 1.0,
                to_offset: Point::ORIGIN,
                to_opacity: 1.0,
            },
            Transition::PopIn { edge } => {
                let (ux, uy) = edge.unit();
                Resolved {
                    from_offset: Point::new(ux * d, uy * d),
                    from_opacity: 0.0,
                    to_offset: Point::ORIGIN,
                    to_opacity: 1.0,
                }
            }
            Transition::PopOut { edge } => {
                let (ux, uy) = edge.unit();
                Resolved {
                    from_offset: Point::ORIGIN,
                    from_opacity: 1.0,
                    to_offset: Point::new(ux * d, uy * d),
                    to_opacity: 0.0,
                }
            }
        }
    }
}

/// 转场解析后的初值 / 终值。
#[derive(Debug, Clone, Copy)]
struct Resolved {
    from_offset: Point,
    from_opacity: f32,
    to_offset: Point,
    to_opacity: f32,
}

impl Tree {
    /// 给节点（`Entrance { stagger: true }` 时改为其直接子项）发转场：自动 `set_layer`、
    /// 放初值、`animate_layer` 到终值。返回本次转场句柄。
    ///
    /// stagger 的上限为 `STAGGER_MAX_ITEMS`（超出的与第 N 个同时）；顺序 = children 顺序。
    pub fn play_transition(
        &mut self,
        id: WidgetId,
        t: Transition,
        p: TransitionParams,
    ) -> TransitionHandle {
        self.play_transition_at(id, t, p, Instant::now())
    }

    /// `play_transition` 的可注入时钟版本（测试用确定性时刻；外壳走 `Instant::now()`）。
    pub(crate) fn play_transition_at(
        &mut self,
        id: WidgetId,
        t: Transition,
        p: TransitionParams,
        now: Instant,
    ) -> TransitionHandle {
        if !self.contains(id) {
            return TransitionHandle {
                id,
                serial: 0,
                start: now,
                keep_layer: p.keep_layer,
            };
        }
        if t.propagates_to_children() && !self.children(id).is_empty() {
            let kids: Vec<WidgetId> = self.children(id).to_vec();
            let mut first = TransitionHandle {
                id,
                serial: 0,
                start: now,
                keep_layer: p.keep_layer,
            };
            for (i, k) in kids.into_iter().enumerate() {
                let step = i.min(STAGGER_MAX_ITEMS);
                let pk = TransitionParams {
                    delay_ms: p.delay_ms + (step as u32) * p.stagger_ms,
                    ..p
                };
                let h = self.play_one(k, t, pk, now);
                if i == 0 {
                    first.serial = h.serial;
                    first.start = h.start;
                }
            }
            first
        } else {
            self.play_one(id, t, p, now)
        }
    }

    /// 单节点转场（stagger 传播后的每个目标各走一次）。
    fn play_one(
        &mut self,
        id: WidgetId,
        t: Transition,
        p: TransitionParams,
        now: Instant,
    ) -> TransitionHandle {
        let rect_origin = self.rect(id).map_or(Point::ORIGIN, |r| r.origin);
        let mut r = t.resolve(p, rect_origin);
        // 中断：已有进行中的转场且图层已建 → 从当前呈现值接续（不跳变、不叠加）。
        if self.layer_created(id)
            && let Some(active) = self.transitions.get(&id)
        {
            let (off, op) = estimate(active, now);
            r.from_offset = off;
            r.from_opacity = op;
        }
        self.set_layer(id, true);
        let serial = self.next_layer_serial.max(1);
        self.next_layer_serial = self.next_layer_serial.wrapping_add(1).max(1);
        let spec = LayerAnimSpec {
            serial,
            duration_ms: p.duration_ms,
            delay_ms: p.delay_ms,
            curve: p.curve,
            to_x: r.to_offset.x,
            to_y: r.to_offset.y,
            to_opacity: r.to_opacity,
        };
        let active = ActiveTransition {
            serial,
            start: now,
            delay_ms: p.delay_ms as f64,
            duration_ms: p.duration_ms.max(1) as f64,
            curve: p.curve,
            from_offset: r.from_offset,
            from_opacity: r.from_opacity,
            to_offset: r.to_offset,
            to_opacity: r.to_opacity,
            keep_layer: p.keep_layer,
        };
        self.transitions.insert(id, active);
        if self.layer_created(id) {
            self.set_layer_offset(id, r.from_offset.x, r.from_offset.y);
            self.set_layer_opacity(id, r.from_opacity);
            self.animate_layer(id, spec);
        } else {
            self.pending_starts.insert(
                id,
                PendingStart {
                    offset: r.from_offset,
                    opacity: r.from_opacity,
                    animate: spec,
                },
            );
        }
        TransitionHandle {
            id,
            serial,
            start: now,
            keep_layer: p.keep_layer,
        }
    }

    /// 节点当前呈现的图层视觉 `(偏移, 不透明度)`。无进行中转场时返回中性值 `((0,0), 1)`。
    ///
    /// `now` 与转场起始时刻之差换算成缓动进度；用于中断接续时估算起点。
    pub fn current_layer_visual(&self, id: WidgetId, now: Instant) -> (Point, f32) {
        match self.transitions.get(&id) {
            Some(a) => estimate(a, now),
            None => (Point::ORIGIN, 1.0),
        }
    }

    /// 是否仍有进行中的转场（诊断 / 测试）。
    pub fn has_active_transition(&self, id: WidgetId) -> bool {
        self.transitions.contains_key(&id)
    }

    /// 外壳回报图层动画完成 / 被打断（harness `TreeApp::on_layer_event` 转达）。
    /// `keep_layer == false` 的转场在终态撤销图层；`serial` 不匹配（已被新转场替换）忽略。
    pub fn on_layer_event(&mut self, id: WidgetId, serial: u32, outcome: LayerOutcome) {
        match self.transitions.get(&id) {
            Some(a) if a.serial == serial => {}
            _ => return,
        }
        let keep = self.transitions.get(&id).is_some_and(|a| a.keep_layer);
        self.transitions.remove(&id);
        self.pending_starts.remove(&id);
        if !keep {
            self.set_layer(id, false);
        }
        let _ = outcome; // Done 与 Cancelled 目前同处置：终态撤层。
    }

    /// 合成器不支持图层动画（harness `LayerEvent::Unsupported`）：直接到终值 —— 撤掉图层后
    /// 节点回到主场景的自然位置（视觉终值），保持可见。
    pub fn on_layer_unsupported(&mut self) {
        let ids: Vec<WidgetId> = self.transitions.keys().copied().collect();
        for id in ids {
            let keep = self.transitions.get(&id).is_some_and(|a| a.keep_layer);
            self.transitions.remove(&id);
            self.pending_starts.remove(&id);
            if !keep {
                self.set_layer(id, false);
            }
        }
    }

    /// 图层建好后发出暂存的起始命令（`Create` → 初值 → `Animate`）。每帧 `sync_layers` 之后调用。
    pub(crate) fn flush_pending_starts(&mut self) {
        if self.pending_starts.is_empty() {
            return;
        }
        let ids: Vec<WidgetId> = self.pending_starts.keys().copied().collect();
        for id in ids {
            if !self.layer_created(id) {
                continue;
            }
            let Some(p) = self.pending_starts.remove(&id) else {
                continue;
            };
            self.set_layer_offset(id, p.offset.x, p.offset.y);
            self.set_layer_opacity(id, p.opacity);
            self.animate_layer(id, p.animate);
        }
    }
}

/// 估算一条进行中转场在 `now` 的视觉值。
fn estimate(a: &ActiveTransition, now: Instant) -> (Point, f32) {
    let elapsed = now.saturating_duration_since(a.start).as_secs_f64() * 1000.0;
    let e = elapsed - a.delay_ms;
    if e <= 0.0 {
        return (a.from_offset, a.from_opacity);
    }
    let t = (e / a.duration_ms).clamp(0.0, 1.0) as f32;
    let k = bezier_y(a.curve, t);
    let lerp = |from: f32, to: f32| from + (to - from) * k;
    (
        Point::new(
            lerp(a.from_offset.x, a.to_offset.x),
            lerp(a.from_offset.y, a.to_offset.y),
        ),
        lerp(a.from_opacity, a.to_opacity),
    )
}

/// 供 `tree.rs` 在节点移除时清理其转场状态（避免残留）。
impl Tree {
    pub(crate) fn drop_transition_state(&mut self, id: WidgetId) {
        self.transitions.remove(&id);
        self.pending_starts.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use kanesumi_core::ThemeColor;

    use super::*;
    use crate::layer::{LayerAnimSpec, LayerOp};
    use crate::props::{Align, Insets, LayoutProps};
    use crate::testing::TestHarness;
    use crate::widgets::{Border, Stack};

    /// 单节点：400×300 表面里一个 120×40 的实心 Border。
    fn setup_single() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let props = LayoutProps {
            width: Some(120.0),
            height: Some(40.0),
            h_align: Align::Start,
            v_align: Align::Start,
            margin: Insets::new(20.0, 20.0, 0.0, 0.0),
            ..LayoutProps::default()
        };
        let node = h
            .tree
            .insert_with(h.root(), Border::new().background(ThemeColor::Background), props);
        h.frame();
        (h, node)
    }

    /// 容器 + `n` 个子项（列）。
    fn setup_children(n: usize) -> (TestHarness, WidgetId, Vec<WidgetId>) {
        let mut h = TestHarness::new(400.0, 300.0);
        let container = h.tree.insert(h.root(), Stack::column().with_spacing(8.0));
        let mut kids = Vec::new();
        for _ in 0..n {
            let props = LayoutProps {
                width: Some(120.0),
                height: Some(24.0),
                h_align: Align::Start,
                ..LayoutProps::default()
            };
            kids.push(h.tree.insert_with(
                container,
                Border::new().background(ThemeColor::Background),
                props,
            ));
        }
        h.frame();
        (h, container, kids)
    }

    fn animate_spec(op: &LayerOp) -> Option<LayerAnimSpec> {
        match op {
            LayerOp::Animate { spec, .. } => Some(*spec),
            _ => None,
        }
    }

    fn offset_of(op: &LayerOp) -> Option<(f32, f32)> {
        match op {
            LayerOp::Offset { x, y, .. } => Some((*x, *y)),
            _ => None,
        }
    }

    fn opacity_of(op: &LayerOp) -> Option<f32> {
        match op {
            LayerOp::Opacity { opacity, .. } => Some(*opacity),
            _ => None,
        }
    }

    #[test]
    fn entrance_emits_create_initial_then_animate() {
        let (mut h, node) = setup_single();
        h.tree
            .play_transition(node, Transition::Entrance { stagger: false }, TransitionParams::ENTRANCE);
        // 图层未 Create 前不产生任何命令；建层帧一次性补上 Create → 初值 → Animate。
        assert!(h.tree.take_layer_ops().is_empty(), "建层前无图层命令");
        h.frame();
        let ops = h.tree.take_layer_ops();
        assert!(matches!(ops.first(), Some(LayerOp::Create { id, .. }) if *id == node));
        assert_eq!(ops.iter().find_map(offset_of), Some((0.0, 40.0)), "初值：下移 40");
        assert_eq!(ops.iter().find_map(opacity_of), Some(0.0), "初值：透明");
        let spec = ops.iter().find_map(animate_spec).expect("含 Animate");
        // 起始命令顺序：初值（Offset / Opacity）先于 Animate。
        let io = ops.iter().position(|o| offset_of(o).is_some()).unwrap();
        let ia = ops.iter().position(|o| animate_spec(o).is_some()).unwrap();
        assert!(io < ia, "先设初值再 Animate");
        assert_eq!(spec.duration_ms, 300);
        assert_eq!(spec.delay_ms, 0);
        assert_eq!(spec.curve, CURVE_UWP_OPEN);
        assert_eq!((spec.to_x, spec.to_y), (0.0, 0.0), "终值：原位");
        assert_eq!(spec.to_opacity, 1.0);
    }

    #[test]
    fn stagger_delays_children_in_order_and_caps() {
        let (mut h, container, kids) = setup_children(13);
        h.tree.play_transition(
            container,
            Transition::Entrance { stagger: true },
            TransitionParams::ENTRANCE,
        );
        h.frame();
        let ops = h.tree.take_layer_ops();
        assert!(!h.tree.is_layer(container), "stagger 只动子项，容器自身不建层");
        for k in &kids {
            assert!(h.tree.is_layer(*k), "每个直接子项各建一层");
        }
        let delays: Vec<u32> = ops.iter().filter_map(animate_spec).map(|s| s.delay_ms).collect();
        assert_eq!(delays.len(), kids.len());
        let expect = |i: usize| (i.min(STAGGER_MAX_ITEMS) as u32) * 30;
        for (i, d) in delays.iter().enumerate() {
            assert_eq!(*d, expect(i), "第 {i} 个子项 delay");
        }
    }

    #[test]
    fn stagger_falls_back_to_self_without_children() {
        let (mut h, node) = setup_single();
        h.tree.play_transition(
            node,
            Transition::Entrance { stagger: true },
            TransitionParams::ENTRANCE,
        );
        h.frame();
        assert!(h.tree.is_layer(node), "无子项时退回对自身建层");
    }

    #[test]
    fn interruption_continues_from_estimated_current_value() {
        let (mut h, node) = setup_single();
        let t0 = Instant::now();
        let h1 = h
            .tree
            .play_transition_at(node, Transition::Content, TransitionParams::CONTENT, t0);
        h.frame();
        h.tree.take_layer_ops();
        assert!(h.tree.is_layer(node));

        // 进行到 150ms（时长 300ms 的一半）：估值应等于贝塞尔解析值。
        let mid = t0 + Duration::from_millis(150);
        let (est, est_op) = h.tree.current_layer_visual(node, mid);
        let k = bezier_y(CURVE_UWP_OPEN, 0.5);
        let expect_y = 16.0 + (0.0 - 16.0) * k;
        let expect_op = 0.0 + (1.0 - 0.0) * k;
        assert!(
            (est.y - expect_y).abs() < 0.5,
            "估值 {est:?} 与贝塞尔解析值 {expect_y} 误差应 < 0.5px"
        );
        assert!((est_op - expect_op).abs() < 0.02);

        // 中断：新转场的起点 = 当前呈现值，不跳变。
        let h2 = h
            .tree
            .play_transition_at(node, Transition::PopIn { edge: Edge::Top }, TransitionParams::POP_IN, mid);
        let ops = h.tree.take_layer_ops();
        let start = ops.iter().find_map(offset_of).expect("新转场先设初值");
        assert!(
            (start.0 - est.x).abs() < 0.5 && (start.1 - est.y).abs() < 0.5,
            "中断起点 {start:?} 应接续当前值 {est:?}"
        );
        assert!(h.tree.has_active_transition(node));
        assert_eq!(h2.serial, h1.serial + 1, "序号递增，旧句柄失效");
    }

    #[test]
    fn transition_does_not_repaint_subtree() {
        let (mut h, node) = setup_single();
        h.frame();
        h.tree
            .play_transition(node, Transition::Entrance { stagger: false }, TransitionParams::ENTRANCE);
        h.frame(); // 建层帧：一次重画把内容搬进图层
        let painted = h.tree.painted(node).to_vec();
        for _ in 0..8 {
            h.frame();
        }
        assert_eq!(h.tree.painted(node), painted.as_slice(), "转场期间内容不变不得重画");
        assert!(!h.tree.needs_frame(), "转场由合成器推进，树不必续帧");
    }

    #[test]
    fn keep_layer_false_removes_after_done_true_keeps() {
        let (mut h, node) = setup_single();
        let hd = h.tree.play_transition(
            node,
            Transition::Content,
            TransitionParams { keep_layer: false, ..TransitionParams::CONTENT },
        );
        h.frame();
        h.tree.take_layer_ops();
        assert!(h.tree.is_layer(node));
        h.tree.on_layer_event(node, hd.serial, LayerOutcome::Done);
        let ops = h.tree.take_layer_ops();
        assert!(ops.iter().any(|o| matches!(o, LayerOp::Remove { .. })), "Done 后撤图层");
        assert!(!h.tree.is_layer(node));

        // keep_layer: true 则保留。
        let hk = h.tree.play_transition(
            node,
            Transition::Content,
            TransitionParams { keep_layer: true, ..TransitionParams::CONTENT },
        );
        h.frame();
        h.tree.take_layer_ops();
        h.tree.on_layer_event(node, hk.serial, LayerOutcome::Done);
        assert!(h.tree.is_layer(node), "keep_layer 保留图层");
        assert!(!h.tree.has_active_transition(node));
    }

    #[test]
    fn stale_event_serial_is_ignored() {
        let (mut h, node) = setup_single();
        let h1 = h.tree.play_transition_at(
            node,
            Transition::Content,
            TransitionParams::CONTENT,
            Instant::now(),
        );
        h.frame();
        h.tree.take_layer_ops();
        // 中断产生新序号。
        let h2 = h.tree.play_transition_at(
            node,
            Transition::PopIn { edge: Edge::Bottom },
            TransitionParams::POP_IN,
            Instant::now(),
        );
        assert_ne!(h1.serial, h2.serial);
        // 旧序号的迟到 Done 不得撤掉新转场的图层。
        h.tree.on_layer_event(node, h1.serial, LayerOutcome::Done);
        assert!(h.tree.is_layer(node), "旧序号忽略");
        assert!(h.tree.has_active_transition(node));
    }

    #[test]
    fn unsupported_jumps_to_terminal_and_keeps_node_visible() {
        let (mut h, node) = setup_single();
        h.tree
            .play_transition(node, Transition::Entrance { stagger: false }, TransitionParams::ENTRANCE);
        h.frame();
        h.tree.take_layer_ops();
        assert!(h.tree.is_layer(node));
        h.tree.on_layer_unsupported();
        assert!(!h.tree.is_layer(node), "不支持时撤层，节点回主场景");
        assert!(h.tree.is_visible(node));
        assert!(!h.tree.has_active_transition(node));
        h.frame();
        assert!(!h.tree.painted(node).is_empty(), "节点照常绘制（可见）");
    }

    #[test]
    fn pop_and_drill_directions_and_targets() {
        let p_in = TransitionParams::POP_IN;
        let r = Transition::PopIn { edge: Edge::Top }.resolve(p_in, Point::ORIGIN);
        assert_eq!((r.from_offset.x, r.from_offset.y), (0.0, -24.0));
        assert_eq!((r.to_offset.x, r.to_offset.y), (0.0, 0.0));
        assert_eq!(r.to_opacity, 1.0);

        let p_out = TransitionParams::POP_OUT;
        let r = Transition::PopOut { edge: Edge::Right }.resolve(p_out, Point::ORIGIN);
        assert_eq!((r.from_offset.x, r.from_offset.y), (0.0, 0.0));
        assert_eq!((r.to_offset.x, r.to_offset.y), (24.0, 0.0));
        assert_eq!(r.to_opacity, 0.0);

        let r = Transition::DrillOut.resolve(TransitionParams::DRILL_OUT, Point::ORIGIN);
        assert_eq!(r.from_opacity, 1.0);
        assert_eq!(r.to_opacity, 0.0);
        assert_eq!((r.to_offset.x, r.to_offset.y), (0.0, 16.0));
    }

    #[test]
    fn reposition_offsets_from_old_position() {
        let (mut h, node) = setup_single();
        let origin = h.tree.rect(node).unwrap().origin;
        let old = Point::new(origin.x + 50.0, origin.y + 30.0);
        h.tree
            .play_transition(node, Transition::Reposition { from: old }, TransitionParams::REPOSITION);
        h.frame();
        let ops = h.tree.take_layer_ops();
        assert_eq!(ops.iter().find_map(offset_of), Some((50.0, 30.0)), "初值 = 旧位偏移");
        let spec = ops.iter().find_map(animate_spec).unwrap();
        assert_eq!((spec.to_x, spec.to_y), (0.0, 0.0), "终值 = 新位");
        assert_eq!(spec.duration_ms, 150);
    }
}
