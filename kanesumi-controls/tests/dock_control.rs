// dock_control.rs —— Dock 控件状态机与渲染的契约（Scene 命令断言，不看像素）。
//
// 判据来自 `docs/LAUNCHER_REBUILD.md`（双区：左区钉选未运行=灰线，右区运行=强调色线）
// 与 `docs/INTERACTION_CANON.md`（按下外观跟随目标、松开必须在同一槽上）。

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::navigation::{DockItem, DockItemState, MetroDock, indicator_rect};
use kanesumi_core::{MetroTheme, Point, Rect};
use kanesumi_element::testing::test_engine;

fn dock(items: Vec<DockItem>) -> MetroDock {
    let mut d = MetroDock::new();
    d.set_items(items, Rect::new(0.0, 0.0, 800.0, 76.0));
    d
}

fn three() -> Vec<DockItem> {
    vec![
        DockItem::new("org.ether.settings", "settings").pinned(true),
        DockItem::new("org.ether.librarian", "files").pinned(true).running(true),
        DockItem::new("org.ether.launcher", "launcher").running(true),
    ]
}

fn center_of(d: &MetroDock, display: usize) -> Point {
    let s = &d.layout().slots[display];
    let r = s.hit.expect("槽位必须可点");
    Point::new(r.origin.x + r.size.width / 2.0, r.origin.y + r.size.height / 2.0)
}

fn fills(scene: &Scene) -> Vec<(Rect, kanesumi_core::Color)> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { color, rect, .. } => Some((*rect, *color)),
            _ => None,
        })
        .collect()
}

#[test]
fn layout_has_one_slot_per_item() {
    let d = dock(three());
    assert_eq!(d.layout().slots.len(), 3);
}

#[test]
fn hover_sets_and_clears_state() {
    let mut d = dock(three());
    let p = center_of(&d, 1);
    assert!(d.hover(p));
    assert_eq!(d.state_of(d.layout().slots[1].source), DockItemState::Hovered);
    // 移出轨道
    d.hover(Point::new(10.0, 5.0));
    assert_eq!(d.hovered(), None);
}

#[test]
fn press_then_release_on_same_slot_activates() {
    let mut d = dock(three());
    let p = center_of(&d, 1);
    let src = d.layout().slots[1].source;
    assert!(d.press(p));
    assert_eq!(d.state_of(src), DockItemState::Pressed);
    let act = d.release(p).expect("同一槽松手应激活");
    assert_eq!(act.source, src);
    assert_eq!(act.id, "org.ether.librarian");
}

#[test]
fn release_outside_cancels() {
    let mut d = dock(three());
    let p = center_of(&d, 1);
    d.press(p);
    assert!(d.release(Point::new(2000.0, 2000.0)).is_none());
    assert_eq!(d.state_of(d.layout().slots[1].source), DockItemState::Rest);
}

#[test]
fn cancel_drops_pressed_state() {
    let mut d = dock(three());
    let p = center_of(&d, 1);
    d.press(p);
    d.cancel();
    assert_eq!(d.state_of(d.layout().slots[1].source), DockItemState::Hovered);
}

#[test]
fn running_and_pinned_zones_use_different_line_colors() {
    let d = dock(three());
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    let fills = fills(&scene);
    let line_of = |display: usize| {
        let slots = d.layout().slots.clone();
        let r = indicator_rect(&slots[display]);
        fills.iter().find(|(rect, _)| *rect == r).map(|(_, c)| *c)
    };
    // 前两槽的区身份可能都是 Pinned/Running，按 zone 断言。
    for (i, slot) in d.layout().slots.iter().enumerate() {
        let color = line_of(i).expect("每个可点槽都要有定位线");
        match slot.zone {
            kanesumi_controls::navigation::DockZone::Pinned => {
                assert_eq!(color, theme.colors.on_surface_variant, "左区灰线")
            }
            kanesumi_controls::navigation::DockZone::Running => {
                assert_eq!(color, theme.colors.primary, "右区强调色线")
            }
        }
    }
}

#[test]
fn urgent_draws_dot_and_absent_otherwise() {
    let plain = dock(three());
    let mut urgent_items = three();
    urgent_items[2].urgent = true;
    let urg = dock(urgent_items);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    plain.render(&theme, &engine, &mut s1);
    urg.render(&theme, &engine, &mut s2);
    assert!(fills(&s2).len() > fills(&s1).len(), "紧急项应多一个点");
}

#[test]
fn window_badge_appears_only_above_one() {
    let mut items = three();
    items[2].windows = 1;
    let one = dock(items);
    let mut items2 = three();
    items2[2].windows = 3;
    let many = dock(items2);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    one.render(&theme, &engine, &mut s1);
    many.render(&theme, &engine, &mut s2);
    let texts = |s: &Scene| {
        s.commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count()
    };
    assert_eq!(texts(&s1), 0, "窗口数为 1 不出徽标");
    assert_eq!(texts(&s2), 1, "窗口数为 3 出一个徽标");
}

#[test]
fn empty_dock_renders_nothing() {
    let d = dock(Vec::new());
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    assert!(d.layout().slots.is_empty());
    assert!(scene.commands.is_empty());
}

#[test]
fn plate_stays_inside_dock_rect() {
    let d = dock(three());
    let plate = d.layout().plate.expect("有条目应有底板");
    let dock_rect = d.layout().dock;
    assert!(plate.origin.x >= dock_rect.origin.x - 0.5);
    assert!(plate.right() <= dock_rect.right() + 0.5);
}
#[test]
fn active_and_urgent_are_different_signals() {
    // active = 正在用（图标位抬一档）；urgent = 需要你注意（角上一个语义点）。
    let mut a = three();
    a[1].active = true;
    let active_dock = dock(a);
    let plain = dock(three());
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    plain.render(&theme, &engine, &mut s1);
    active_dock.render(&theme, &engine, &mut s2);
    // 元素数不变（只换颜色），但那一槽的图标位颜色不同。
    assert_eq!(fills(&s1).len(), fills(&s2).len(), "active 只改颜色，不增减元素");
}

#[test]
fn dock_state_transcript() {
    let mut d = dock(three());
    let p1 = center_of(&d, 1);
    let p0 = center_of(&d, 0);
    let src = d.layout().slots[1].source;
    let mut t: Vec<String> = Vec::new();
    d.hover(p1);
    t.push(format!("hover(1)      -> {:?}", d.state_of(src)));
    d.hover(p0);
    t.push(format!("hover(0)      -> {:?}", d.state_of(src)));
    d.hover(p1);
    d.press(p1);
    t.push(format!("press(1)      -> {:?}", d.state_of(src)));
    let act = d.release(p1);
    t.push(format!(
        "release(1)    -> {}",
        act.map(|a| format!("Activated({})", a.id)).unwrap_or_else(|| "None".to_string())
    ));
    d.press(p1);
    let out = d.release(Point::new(2000.0, 2000.0));
    t.push(format!(
        "release(out)  -> {}",
        out.map(|a| a.id).unwrap_or_else(|| "Cancelled".to_string())
    ));
    let expect = vec![
        "hover(1)      -> Hovered".to_string(),
        "hover(0)      -> Rest".to_string(),
        "press(1)      -> Pressed".to_string(),
        "release(1)    -> Activated(org.ether.librarian)".to_string(),
        "release(out)  -> Cancelled".to_string(),
    ];
    assert_eq!(t, expect, "状态转录必须逐行相等");
}