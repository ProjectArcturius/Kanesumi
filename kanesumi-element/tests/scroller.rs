// 合成滚动（fp4b）契约测试：滚动器图层的建层 / 实现窗口 / offset 不重提交 / 回退。
// 参 layer.rs「合成滚动」、Ether fp4-compositor-scroll.md §Ⅴ。

const RED: Color = Color::rgb(1.0, 0.0, 0.0);

use kanesumi_core::{Color, Rect, Size};
use kanesumi_element::layer::{
    scroller_needs_realize, scroller_realized_window, ScrollerRequest, ScrollerSnapshot,
};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Border;
use kanesumi_element::{Event, Key, LayerOp, LayoutProps, Widget, WidgetId};

// ── 实现窗口几何（纯函数）────────────────────────────────────────────────────

#[test]
fn realized_window_is_three_screens_clamped_to_content() {
    // 视口 100、内容 1000：窗口 3 屏 = 300；offset 500 → 上缘 400（上方备一屏）。
    assert_eq!(scroller_realized_window(500.0, 100.0, 1000.0), (400.0, 300.0));
    // 顶部：offset 0 → 上缘 clamp 0。
    assert_eq!(scroller_realized_window(0.0, 100.0, 1000.0), (0.0, 300.0));
    // 底部：offset 900（= 1000−100）→ 上缘 800 clamp 到 700（窗口底贴内容底）。
    assert_eq!(scroller_realized_window(900.0, 100.0, 1000.0), (700.0, 300.0));
    // 内容不足 3 屏：窗口 = 全长。
    assert_eq!(scroller_realized_window(50.0, 100.0, 200.0), (0.0, 200.0));
}

#[test]
fn needs_realize_triggers_inside_half_screen_margin() {
    // 窗口 [400,700)，视口 100：offset 500 → 上距 100、下距 100 ≥ 50 → 不需要。
    assert!(!scroller_needs_realize(500.0, 100.0, (400.0, 300.0), 1000.0));
    // 距上缘 40 < 50 → 需要。
    assert!(scroller_needs_realize(440.0, 100.0, (400.0, 300.0), 1000.0));
    // 距下缘 40 < 50 → 需要。
    assert!(scroller_needs_realize(560.0, 100.0, (400.0, 300.0), 1000.0));
    // 窗口贴合内容全长（无滚动余量）→ 永不需要。
    assert!(!scroller_needs_realize(50.0, 100.0, (0.0, 200.0), 200.0));
    // 贴底窗口（底 = 内容底）：上方富余即可，底侧不再要求余量（否则每帧重实现）。
    assert!(!scroller_needs_realize(900.0, 100.0, (700.0, 300.0), 1000.0));
    // 贴顶窗口同理。
    assert!(!scroller_needs_realize(0.0, 100.0, (0.0, 300.0), 1000.0));
}

// ── 树级：滚动器图层生命周期 ────────────────────────────────────────────────

/// 测试滚动容器：视口 = 自身矩形（固定 200×100），内容全长 `content_h`；
/// 单个子块排在 `−offset`（与 MetroScrollView 的排布同构）。
struct Pane {
    content_h: f32,
    offset: f32,
    composed: bool,
    viewport: Size,
    request: Option<ScrollerRequest>,
    disabled_called: bool,
}

impl Pane {
    fn new(content_h: f32) -> Self {
        Self {
            content_h,
            offset: 0.0,
            composed: true,
            viewport: Size::ZERO,
            request: None,
            disabled_called: false,
        }
    }
}

impl Widget for Pane {
    fn measure(&mut self, _ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        Size::new(available.width.min(200.0), 100.0)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.viewport = rect.size;
        let max = (self.content_h - rect.size.height).max(0.0);
        if self.offset > max {
            self.offset = max;
        }
        for c in ctx.children() {
            ctx.arrange_child(c, Rect::new(rect.origin.x, rect.origin.y - self.offset, rect.size.width, 40.0));
        }
    }

    fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut kanesumi_canvas::Scene) {}

    fn scrolls_children(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &Event) {
        // 键盘路径（合成模式）：PageDown 交给最近的滚动器图层。
        if let Event::KeyDown { key: Key::PageDown, .. } = event {
            ctx.scroller_scroll_by(52.0);
            ctx.set_handled();
        }
    }

    fn take_scroller_snapshot(&mut self) -> Option<ScrollerSnapshot> {
        if !self.composed {
            return None;
        }
        Some(ScrollerSnapshot {
            viewport: self.viewport,
            extent: Size::new(self.viewport.width, self.content_h),
            offset: self.offset,
            request: self.request.take(),
        })
    }

    fn on_scroller_offset(&mut self, ctx: &mut kanesumi_element::EventCtx, _x: f32, y: f32, _present_ns: u64) {
        self.offset = y;
        ctx.invalidate_arrange();
        ctx.invalidate_paint();
    }

    fn on_scroller_realized(&mut self, _realized: Rect) {}

    fn on_scroller_disabled(&mut self) {
        self.composed = false;
        self.disabled_called = true;
    }
}

/// 视口 200×100、内容 500 的滚动容器（一个色块子树）。
fn harness() -> (TestHarness, WidgetId) {
    let mut h = TestHarness::new(400.0, 300.0);
    let pane = h.tree.insert_with(
        h.root(),
        Pane::new(500.0),
        LayoutProps {
            width: Some(200.0),
            height: Some(100.0),
            ..LayoutProps::default()
        },
    );
    h.tree.insert(pane, Border::new().background(RED));
    assert!(h.tree.set_scroller(pane, true), "全局开关默认开");
    h.frame();
    (h, pane)
}

#[test]
fn scroller_create_then_realize_next_frame() {
    let (mut h, pane) = harness();
    // 首帧：建层 + 几何；窗口已登记但内容**下一帧**才提交（行按窗口实现的时序）。
    let ops = h.tree.take_layer_ops();
    assert!(
        ops.iter().any(|o| matches!(o, LayerOp::ScrollerCreate { id, rect } if *id == pane && rect.size == Size::new(200.0, 100.0))),
        "应有 ScrollerCreate：{ops:?}"
    );
    assert!(ops.iter().any(|o| matches!(o, LayerOp::ScrollerGeometry { .. })), "应有 ScrollerGeometry：{ops:?}");
    assert!(!ops.iter().any(|o| matches!(o, LayerOp::ScrollerRealize { .. })), "首帧不得提交内容：{ops:?}");
    h.frame();
    let ops = h.tree.take_layer_ops();
    let LayerOp::ScrollerRealize { id, realized, .. } = ops.iter().find(|o| matches!(o, LayerOp::ScrollerRealize { .. })).expect("第二帧提交窗口内容") else { unreachable!() };
    assert_eq!(*id, pane);
    // offset 0 → 窗口 [0, 300)（内容 500 时 3 屏 = 300）。
    assert_eq!(*realized, Rect::new(0.0, 0.0, 200.0, 300.0));
}

#[test]
fn offset_event_updates_layout_without_resubmitting_content() {
    let (mut h, pane) = harness();
    h.frame();
    h.tree.take_layer_ops();

    // 合成器回报 offset = 150（窗口 [0,300) 内，距两缘 ≥ 50 → 不重实现）。
    h.tree.deliver_scroller_offset(pane, 0.0, 150.0, 1_000);
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(
        ops.iter().all(|o| !matches!(o, LayerOp::ScrollerRealize { .. })),
        "窗口内滑动不得重提交内容：{ops:?}"
    );
    h.tree.edit::<Pane, _>(pane, |p, _| assert_eq!(p.offset, 150.0, "offset 事件要驱动控件"));
}

#[test]
fn offset_near_edge_rerealizes_window() {
    let (mut h, pane) = harness();
    h.frame();
    h.tree.take_layer_ops();

    // offset 260：距窗口下缘 [300) 只剩 300−(260+100) = −60 < 50 → 重实现。
    h.tree.deliver_scroller_offset(pane, 0.0, 260.0, 2_000);
    h.frame(); // 登记新窗口（不发）。
    let ops = h.tree.take_layer_ops();
    assert!(!ops.iter().any(|o| matches!(o, LayerOp::ScrollerRealize { .. })), "新窗口先登记：{ops:?}");
    h.frame(); // 按新窗口提交。
    let ops = h.tree.take_layer_ops();
    let LayerOp::ScrollerRealize { realized, .. } = ops
        .iter()
        .find(|o| matches!(o, LayerOp::ScrollerRealize { .. }))
        .expect("越缘后应重实现")
    else {
        unreachable!()
    };
    // 新窗口以 offset 260 为中心：上缘 160、300 高，夹在 [0, 200]。
    assert_eq!(*realized, Rect::new(0.0, 160.0, 200.0, 300.0));
}

#[test]
fn keyboard_scroll_routes_to_scroller_op() {
    let (mut h, pane) = harness();
    h.frame();
    h.tree.take_layer_ops();
    // PageDown 落到容器子块 → 冒泡到 Pane → EventCtx 的合成滚动通道。
    h.tree.focus(pane, true);
    h.frame();
    h.key(Key::PageDown);
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(
        ops.iter().any(|o| matches!(o, LayerOp::ScrollerScroll { id, request: ScrollerRequest::ScrollBy { delta: 52.0 }, .. } if *id == pane)),
        "键盘滚动应发 ScrollerScroll：{ops:?}"
    );
}

#[test]
fn disable_all_scrollers_resets_widget_and_returns_content() {
    let (mut h, pane) = harness();
    h.frame();
    h.frame();
    h.tree.take_layer_ops();

    h.tree.disable_all_scrollers();
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(matches!(ops[0], LayerOp::Remove { id } if id == pane), "应撤下滚动器：{ops:?}");
    h.tree.edit::<Pane, _>(pane, |p, _| {
        assert!(!p.composed, "控件应复位回自绘");
        assert!(p.disabled_called);
    });
    // 主场景恢复内容（子块不再被滚动器剔除）。
    assert!(
        h.last.scene.commands.iter().any(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { color, .. } if *color == RED)),
        "Border 内容应回主场景"
    );
}
