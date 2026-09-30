// 元素树契约测试。参 docs/ELEMENT_TREE.md —— 每条测试对应文中一条契约。

use std::cell::Cell;
use std::rc::Rc;

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::{Color, Point, Rect, Size};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::{Border, Label, Stack};
use kanesumi_element::{
    Align, Event, EventCtx, Insets, Key, LayoutProps, MeasureCtx, PaintCtx, PointerButton,
    PopupDismissed, PopupSpec, UpdateCtx, VisualState, Widget,
};

// ── 测试控件 ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Clicked;

/// 最小交互控件：固定尺寸、可聚焦、Click / Enter 发 `Clicked`，hover 用 VisualState 过渡。
struct TestButton {
    size: Size,
    hover: VisualState,
    measures: Rc<Cell<u32>>,
}

impl TestButton {
    fn new(w: f32, h: f32) -> Self {
        Self {
            size: Size::new(w, h),
            hover: VisualState::pointer(),
            measures: Rc::new(Cell::new(0)),
        }
    }
}

impl Widget for TestButton {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        self.measures.set(self.measures.get() + 1);
        self.size
    }
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let t = self.hover.drive(ctx.state().hovered, ctx);
        let base = Color::rgb(0.2, 0.2, 0.2);
        scene.fill_rect(base.lerp(Color::rgb(0.4, 0.4, 0.4), t as f64), ctx.rect());
    }
    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        self.hover.tick(ctx, dt);
    }
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Click
            | Event::KeyDown {
                key: Key::Enter, ..
            } => {
                ctx.emit(Clicked);
                ctx.set_handled();
            }
            _ => {}
        }
    }
    fn focusable(&self) -> bool {
        true
    }
}

fn fixed(w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

// ── 布局 ──────────────────────────────────────────────────────────────────────

#[test]
fn column_stacks_children_and_grow_takes_remaining_space() {
    let mut h = TestHarness::new(200.0, 300.0);
    let col = h.tree.insert(h.root(), Stack::column().with_spacing(10.0));
    let a = h.tree.insert(col, TestButton::new(50.0, 40.0));
    let b = h.tree.insert_with(
        col,
        TestButton::new(50.0, 40.0),
        LayoutProps {
            grow: 1.0,
            ..LayoutProps::default()
        },
    );
    let c = h.tree.insert(col, TestButton::new(50.0, 40.0));
    h.frame();
    assert_eq!(h.rect(a), Rect::new(0.0, 0.0, 200.0, 40.0));
    // 300 - 3*40 - 2*10 = 160 剩余全给 b。
    assert_eq!(h.rect(b), Rect::new(0.0, 50.0, 200.0, 200.0));
    assert_eq!(h.rect(c), Rect::new(0.0, 260.0, 200.0, 40.0));
    h.assert_contained();
}

#[test]
fn child_larger_than_slot_is_clamped_into_slot() {
    // 保险机制 §Ⅳ.3-1：固定 500 宽的子放进 100 宽的父，结果夹进父槽位，不越界。
    let mut h = TestHarness::new(300.0, 300.0);
    let parent = h.tree.insert_with(h.root(), Border::new(), fixed(100.0, 100.0));
    let child = h.tree.insert_with(parent, TestButton::new(10.0, 10.0), fixed(500.0, 20.0));
    h.frame();
    assert_eq!(h.rect(child).size.width, 100.0);
    h.assert_contained();
}

#[test]
fn margin_and_alignment_are_resolved_by_the_framework() {
    let mut h = TestHarness::new(200.0, 100.0);
    let b = h.tree.insert_with(
        h.root(),
        TestButton::new(40.0, 20.0),
        LayoutProps {
            margin: Insets::all(10.0),
            h_align: Align::End,
            v_align: Align::Center,
            ..LayoutProps::default()
        },
    );
    h.frame();
    assert_eq!(h.rect(b), Rect::new(150.0, 40.0, 40.0, 20.0));
}

#[test]
fn measure_is_cached_until_invalidated() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = TestButton::new(40.0, 20.0);
    let counter = btn.measures.clone();
    let id = h.tree.insert(h.root(), btn);
    h.frame();
    let after_first = counter.get();
    assert!(after_first >= 1);
    h.frame();
    h.frame();
    assert_eq!(counter.get(), after_first, "未失效时不得重复量测");
    h.tree.edit::<TestButton, _>(id, |b, ctx| {
        b.size = Size::new(60.0, 20.0);
        ctx.invalidate_measure();
    });
    h.frame();
    assert!(counter.get() > after_first);
}

#[test]
fn hidden_node_takes_no_space_and_is_not_hit() {
    let mut h = TestHarness::new(200.0, 200.0);
    let col = h.tree.insert(h.root(), Stack::column());
    let a = h.tree.insert_with(col, TestButton::new(50.0, 40.0), fixed(50.0, 40.0));
    let b = h.tree.insert_with(col, TestButton::new(50.0, 40.0), fixed(50.0, 40.0));
    h.frame();
    let a_center = h.center(a);
    h.tree.update_props(a, |p| p.visible = false);
    h.frame();
    assert_eq!(h.rect(b).origin.y, 0.0, "隐藏节点不占位");
    assert_ne!(h.tree.hit(a_center), Some(a));
}

// ── 输入 ──────────────────────────────────────────────────────────────────────

#[test]
fn click_on_child_label_is_delivered_to_interactive_ancestor() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert_with(h.root(), TestButton::new(100.0, 40.0), fixed(100.0, 40.0));
    let label = h.tree.insert(btn, Label::new("OK"));
    h.frame();
    assert_eq!(h.tree.hit(h.center(label)), Some(label));
    h.click(label);
    let acts = h.take::<Clicked>();
    assert_eq!(acts, vec![(btn, Clicked)]);
}

#[test]
fn press_then_release_outside_does_not_click() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert_with(h.root(), TestButton::new(100.0, 40.0), fixed(100.0, 40.0));
    h.frame();
    h.press_at(h.center(btn), PointerButton::Left);
    assert!(h.tree.states_of(btn).pressed);
    h.move_to(Point::new(150.0, 150.0));
    assert!(!h.tree.states_of(btn).pressed, "移出后按下外观恢复");
    h.release_at(Point::new(150.0, 150.0), PointerButton::Left);
    h.frame();
    assert!(h.take::<Clicked>().is_empty());
}

#[test]
fn empty_stack_area_passes_hits_through() {
    // COMPOSITION 契约 12：无背景容器的空白处不命中。
    let mut h = TestHarness::new(200.0, 200.0);
    let col = h.tree.insert(h.root(), Stack::column());
    h.tree.insert_with(col, TestButton::new(50.0, 40.0), fixed(50.0, 40.0));
    h.frame();
    assert_eq!(h.tree.hit(Point::new(150.0, 150.0)), None);
}

#[test]
fn right_click_records_context_target() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert_with(h.root(), TestButton::new(100.0, 40.0), fixed(100.0, 40.0));
    h.frame();
    h.right_click(btn);
    assert_eq!(h.tree.context_target(), Some(btn));
    assert!(h.take::<Clicked>().is_empty(), "右键不触发 Click");
}

#[test]
fn hover_repaints_only_the_hovered_control() {
    let mut h = TestHarness::new(400.0, 400.0);
    let bg = h.tree.insert(h.root(), Border::new().background(Color::rgb(0.1, 0.1, 0.1)));
    let btn = h.tree.insert_with(bg, TestButton::new(100.0, 40.0), fixed(100.0, 40.0));
    let first = h.frame().damage;
    assert_eq!(first, None, "首帧全量重绘");
    h.move_to(h.center(btn));
    let out = h.frame().clone();
    let d = out.damage.expect("悬停应产出局部损伤而非全量");
    assert!(d.size.width <= 104.0 + 0.01 && d.size.height <= 44.0 + 0.01, "损伤 {d:?} 应只覆盖按钮（含焦点视觉外扩）");
    assert!(out.animating, "hover 过渡在动画中");
    h.settle();
    assert!(!h.tree.needs_frame(), "稳态后不再请求帧");
}

// ── 焦点 ──────────────────────────────────────────────────────────────────────

#[test]
fn tab_order_follows_tree_and_skips_disabled_and_hidden() {
    let mut h = TestHarness::new(400.0, 400.0);
    let col = h.tree.insert(h.root(), Stack::column());
    let a = h.tree.insert(col, TestButton::new(50.0, 20.0));
    let b = h.tree.insert(col, TestButton::new(50.0, 20.0));
    let c = h.tree.insert(col, TestButton::new(50.0, 20.0));
    let d = h.tree.insert(col, TestButton::new(50.0, 20.0));
    h.tree.set_enabled(b, false);
    h.tree.update_props(c, |p| p.visible = false);
    h.frame();
    h.tab();
    assert_eq!(h.tree.focused(), Some(a));
    h.tab();
    assert_eq!(h.tree.focused(), Some(d));
    h.tab();
    assert_eq!(h.tree.focused(), Some(a), "越过末尾环绕");
    h.shift_tab();
    assert_eq!(h.tree.focused(), Some(d));
}

#[test]
fn enter_on_focused_control_activates_it() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert(h.root(), TestButton::new(50.0, 20.0));
    h.frame();
    h.tab();
    assert!(h.key(Key::Enter));
    assert_eq!(h.take::<Clicked>(), vec![(btn, Clicked)]);
}

#[test]
fn focus_visual_only_for_keyboard_focus() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert_with(h.root(), TestButton::new(50.0, 20.0), fixed(50.0, 20.0));
    h.frame();
    let strokes = |h: &TestHarness| {
        h.last
            .scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count()
    };
    h.click(btn);
    assert_eq!(h.tree.focused(), Some(btn));
    assert_eq!(strokes(&h), 0, "指针聚焦不画焦点视觉");
    h.tab(); // 唯一可聚焦项 → 环绕回自身，转为键盘焦点
    assert_eq!(strokes(&h), 1);
}

#[test]
fn removing_focused_node_clears_focus_and_invalidates_id() {
    let mut h = TestHarness::new(200.0, 200.0);
    let btn = h.tree.insert(h.root(), TestButton::new(50.0, 20.0));
    h.frame();
    h.tab();
    h.tree.remove(btn);
    h.frame();
    assert_eq!(h.tree.focused(), None);
    assert!(h.tree.get::<TestButton>(btn).is_none(), "悬垂 id 查不到");
    // 槽位复用后旧 id 仍查不到新节点。
    let other = h.tree.insert(h.root(), TestButton::new(1.0, 1.0));
    assert_ne!(other, btn);
    assert!(h.tree.get::<TestButton>(btn).is_none());
}

// ── 覆盖层 ────────────────────────────────────────────────────────────────────

fn panel() -> Border {
    Border::new().background(Color::rgb(0.3, 0.3, 0.3))
}

#[test]
fn popup_is_placed_below_anchor_and_clamped_into_surface() {
    let mut h = TestHarness::new(300.0, 200.0);
    let btn = h.tree.insert_with(h.root(), TestButton::new(80.0, 30.0), fixed(80.0, 30.0));
    h.tree.update_props(btn, |p| p.margin = Insets::new(250.0, 20.0, 0.0, 0.0));
    h.frame();
    let pop = h.tree.open_popup(
        panel(),
        PopupSpec {
            anchor: Some(btn),
            gap: 4.0,
            ..PopupSpec::default()
        },
    );
    let inner = h.tree.insert_with(pop, TestButton::new(120.0, 60.0), fixed(120.0, 60.0));
    h.frame();
    let r = h.rect(pop);
    assert_eq!(r.origin.y, h.rect(btn).bottom() + 4.0);
    assert!(r.right() <= 300.0, "面板夹进表面 {r:?}");
    assert_eq!(h.tree.hit(h.center(inner)), Some(inner), "覆盖层优先命中");
}

#[test]
fn click_outside_light_dismisses_and_is_swallowed() {
    let mut h = TestHarness::new(300.0, 300.0);
    let under = h.tree.insert_with(h.root(), TestButton::new(300.0, 300.0), fixed(300.0, 300.0));
    h.frame();
    let pop = h.tree.open_popup(panel(), PopupSpec::default());
    h.tree.insert_with(pop, TestButton::new(50.0, 50.0), fixed(50.0, 50.0));
    h.frame();
    h.click_at(Point::new(5.0, 5.0));
    let acts = h.take_actions();
    assert!(acts.iter().any(|(id, a)| *id == pop && a.is::<PopupDismissed>()));
    assert!(!acts.iter().any(|(id, a)| *id == under && a.is::<Clicked>()), "关闭弹层的那次点击被吞掉");
    assert!(!h.tree.contains(pop));
}

#[test]
fn modal_popup_swallows_outside_click_and_traps_focus() {
    let mut h = TestHarness::new(300.0, 300.0);
    let outside = h.tree.insert(h.root(), TestButton::new(50.0, 20.0));
    h.frame();
    let pop = h.tree.open_popup(
        panel(),
        PopupSpec {
            modal: true,
            ..PopupSpec::default()
        },
    );
    let col = h.tree.insert(pop, Stack::column());
    let a = h.tree.insert(col, TestButton::new(50.0, 20.0));
    let b = h.tree.insert(col, TestButton::new(50.0, 20.0));
    h.frame();
    h.click_at(Point::new(1.0, 299.0));
    assert!(h.tree.contains(pop), "模态弹层点外部不关闭");
    for _ in 0..4 {
        h.tab();
        let f = h.tree.focused().unwrap();
        assert!(f == a || f == b, "焦点不得逃出模态弹层（得到 {f:?}，外部 {outside:?}）");
    }
}

#[test]
fn escape_closes_topmost_popup() {
    let mut h = TestHarness::new(300.0, 300.0);
    h.frame();
    let pop = h.tree.open_popup(panel(), PopupSpec::default());
    h.frame();
    assert!(h.key(Key::Escape));
    assert!(!h.tree.contains(pop));
}

// ── 通用保险断言自检 ───────────────────────────────────────────────────────────

#[test]
fn builtin_widgets_pass_the_insurance_assertions() {
    let mut h = TestHarness::new(320.0, 240.0);
    let border = h.tree.insert_with(
        h.root(),
        Border::new()
            .background(Color::rgb(0.2, 0.2, 0.2))
            .padding(Insets::all(8.0)),
        fixed(200.0, 60.0),
    );
    let label = h.tree.insert(border, Label::new("一段很长很长很长很长很长很长很长的中文标签，必须省略而不是溢出"));
    h.frame();
    h.assert_contained();
    h.assert_no_hit_outside(border);
    h.assert_paint_within(border, Insets::ZERO);
    h.assert_paint_within(label, Insets::ZERO);
    assert!(h.rect(label).size.width <= 184.0);
}

#[test]
#[should_panic(expected = "绘制越界")]
fn paint_assertion_catches_overflowing_widget() {
    // 反向自检：断言本身必须抓得到越界者，防「检查形同虚设」。
    struct Overflowing;
    impl Widget for Overflowing {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(10.0, 10.0)
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            let r = ctx.rect();
            scene.fill_rect(Color::rgb(1.0, 0.0, 0.0), Rect::new(r.origin.x, r.origin.y, 100.0, 100.0));
        }
    }
    let mut h = TestHarness::new(200.0, 200.0);
    let id = h.tree.insert_with(h.root(), Overflowing, fixed(10.0, 10.0));
    h.frame();
    h.assert_paint_within(id, Insets::ZERO);
}
