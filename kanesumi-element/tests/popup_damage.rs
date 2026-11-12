// popup_damage.rs —— 弹层的局部损伤契约（补 n1112 `k-popup-damage` 缺的证据）。
//
// 背景：供应门「局部损伤」一格的实现早已存在（`tree.rs:1812-1818` 节点重画按「旧 ∪ 新」登记，
// `remove()` 逐个记旧矩形），但**没有测试钉住它** —— 本 crate 原本只有 1 条损伤相关测试。
// 这里补四条状态转录：弹层打开 / 关闭 / 内容变大 / 两层叠着，断言 `FrameOutput.damage`
// 是弹层矩形（∪ 旧矩形）而**不是整表面**。参 `docs/research/2026-11-12_popup_damage_audit.md`。

use kanesumi_core::{Color, Point, Rect};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Border;
use kanesumi_element::{Align, Insets, LayoutProps, PopupSpec};

fn fixed(w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

fn panel() -> Border {
    Border::new().background(Color::rgb(0.3, 0.3, 0.3))
}

fn area(r: Rect) -> f32 {
    r.size.width.max(0.0) * r.size.height.max(0.0)
}

/// `outer` 是否完整包含 `inner`（坐标轴对齐）。
fn covers(outer: Rect, inner: Rect) -> bool {
    outer.origin.x <= inner.origin.x
        && outer.origin.y <= inner.origin.y
        && outer.right() >= inner.right()
        && outer.bottom() >= inner.bottom()
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.origin.x < b.right() && b.origin.x < a.right() && a.origin.y < b.bottom() && b.origin.y < a.bottom()
}

/// 先跑两帧把缓冲与布局稳定下来（首帧的 `full` 语义由外壳决定，本文件不作断言）。
fn settle_two_frames(h: &mut TestHarness) {
    h.frame();
    h.frame();
}

#[test]
fn popup_open_damages_popup_region_not_whole_surface() {
    let mut h = TestHarness::new(400.0, 300.0);
    h.tree.insert_with(h.root(), panel(), fixed(400.0, 300.0));
    settle_two_frames(&mut h);

    let pop = h.tree.open_popup(panel(), PopupSpec::default());
    h.tree.insert_with(pop, panel(), fixed(80.0, 40.0));
    let o = h.frame();
    let (damage, full) = (o.damage, o.full);
    let pop_rect = h.rect(pop);

    assert!(!full, "打开弹层不该整幅重绘");
    let d = damage.expect("打开弹层必须产出局部损伤");
    assert!(covers(d, pop_rect), "损伤必须覆盖弹层本身：damage={d:?} popup={pop_rect:?}");
    assert!(
        area(d) < area(Rect::new(0.0, 0.0, 400.0, 300.0)) * 0.9,
        "损伤不应接近整表面：damage={d:?}"
    );
}

#[test]
fn popup_close_damages_its_old_rect() {
    let mut h = TestHarness::new(400.0, 300.0);
    h.tree.insert_with(h.root(), panel(), fixed(400.0, 300.0));
    settle_two_frames(&mut h);

    let pop = h.tree.open_popup(panel(), PopupSpec::default());
    h.tree.insert_with(pop, panel(), fixed(80.0, 40.0));
    h.frame();
    let old = h.rect(pop);

    h.tree.close_popup(pop);
    let o = h.frame();
    let (damage, full) = (o.damage, o.full);

    assert!(!full, "关闭弹层不该整幅重绘");
    let d = damage.expect("关闭弹层必须把旧矩形记进损伤（否则留残影）");
    assert!(covers(d, old), "损伤必须覆盖弹层旧矩形：damage={d:?} old={old:?}");
}

#[test]
fn popup_content_growth_damages_old_union_new() {
    let mut h = TestHarness::new(400.0, 300.0);
    h.tree.insert_with(h.root(), panel(), fixed(400.0, 300.0));
    settle_two_frames(&mut h);

    let pop = h.tree.open_popup(panel(), PopupSpec::default());
    let body = h.tree.insert_with(pop, panel(), fixed(80.0, 40.0));
    let o = h.frame();
    let _ = (o.damage, o.full);
    let old = h.rect(body);

    // 内容变大：只改子节点尺寸
    h.tree.update_props(body, |p| {
        p.width = Some(180.0);
        p.height = Some(90.0);
    });
    let o = h.frame();
    let (damage, full) = (o.damage, o.full);
    let new = h.rect(body);

    assert!(!full, "内容变化不该整幅重绘");
    let d = damage.expect("内容变大必须产出损伤");
    assert!(covers(d, old), "必须覆盖旧矩形（防残影）：damage={d:?} old={old:?}");
    assert!(covers(d, new), "必须覆盖新矩形：damage={d:?} new={new:?}");
}

#[test]
fn stacked_popups_damage_only_their_own_rects() {
    let mut h = TestHarness::new(420.0, 320.0);
    h.tree.insert_with(h.root(), panel(), fixed(420.0, 320.0));
    // 两个锚点拉开距离，保证两个弹层不重叠。
    let top_anchor = h
        .tree
        .insert_with(h.root(), panel(), fixed(60.0, 24.0));
    h.tree
        .update_props(top_anchor, |p| p.margin = Insets::new(10.0, 10.0, 0.0, 0.0));
    let bottom_anchor = h
        .tree
        .insert_with(h.root(), panel(), fixed(60.0, 24.0));
    h.tree
        .update_props(bottom_anchor, |p| p.margin = Insets::new(260.0, 10.0, 0.0, 0.0));
    h.frame();
    h.frame();

    let lower = h.tree.open_popup(
        panel(),
        PopupSpec {
            anchor: Some(bottom_anchor),
            gap: 4.0,
            ..PopupSpec::default()
        },
    );
    h.tree.insert_with(lower, panel(), fixed(120.0, 40.0));
    h.frame();
    let lower_rect = h.rect(lower);

    let upper = h.tree.open_popup(
        panel(),
        PopupSpec {
            anchor: Some(top_anchor),
            gap: 4.0,
            ..PopupSpec::default()
        },
    );
    h.tree.insert_with(upper, panel(), fixed(120.0, 40.0));
    let o = h.frame();
    let (damage, full) = (o.damage, o.full);
    let upper_rect = h.rect(upper);

    assert!(!full, "叠第二层弹层不该整幅重绘");
    let d = damage.expect("第二层打开必须产出损伤");
    assert!(covers(d, upper_rect), "损伤必须覆盖新弹层：damage={d:?} upper={upper_rect:?}");
    assert!(
        !overlaps(d, lower_rect),
        "损伤不得波及下层弹层：damage={d:?} lower={lower_rect:?}"
    );

    // 关掉上层：损伤只记上层旧矩形，不动下层。
    h.tree.close_popup(upper);
    let o = h.frame();
    let (damage2, full2) = (o.damage, o.full);
    assert!(!full2, "关上层弹层不该整幅重绘");
    let d2 = damage2.expect("关上层必须记旧矩形");
    assert!(covers(d2, upper_rect), "关闭损伤必须覆盖上层旧矩形：damage={d2:?}");
    assert!(
        !overlaps(d2, lower_rect),
        "关闭损伤不得波及下层弹层：damage={d2:?} lower={lower_rect:?}"
    );
    let _ = Point::new(0.0, 0.0);
}
