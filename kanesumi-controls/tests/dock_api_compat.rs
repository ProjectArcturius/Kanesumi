// 旧查询仍可编译且保持历史几何，新查询读取轨道真值；不依赖重录的 API 快照。
use kanesumi_controls::navigation::{DockItem, MetroDock, try_indicator_rect};
use kanesumi_core::Rect;

#[test]
#[allow(deprecated)]
fn legacy_rect_query_and_visible_query_coexist() {
    let mut dock = MetroDock::new();
    dock.set_items(
        vec![DockItem::new("files", "files").pinned(true)],
        Rect::new(0.0, 0.0, 1536.0, 76.0),
    );
    let slot = &dock.layout().slots[0];
    let legacy: Rect = kanesumi_controls::navigation::indicator_rect(slot);
    assert_eq!(
        legacy,
        Rect::new(
            slot.icon_rect.origin.x,
            slot.icon_rect.bottom() + 2.0,
            slot.icon_rect.size.width,
            3.0
        )
    );
    assert_eq!(legacy.origin.y, 60.0);
    assert_eq!(try_indicator_rect(slot), slot.indicator);
    assert_eq!(try_indicator_rect(slot).unwrap().origin.y, 65.0);
}

#[test]
fn starting_line_and_icon_accept_independent_phases() {
    let mut dock = MetroDock::new();
    dock.set_items(
        vec![DockItem::new("files", "files").pinned(true)],
        Rect::new(0.0, 0.0, 1536.0, 76.0),
    );
    dock.set_starting(&["files".into()]);
    dock.set_starting_motion(0.5, 0.0);
    assert_eq!(dock.icon_jump("files"), 0.0);
    let theme = kanesumi_core::MetroTheme::ether_dark();
    let engine = kanesumi_element::testing::test_engine();
    let mut before = kanesumi_canvas::Scene::default();
    dock.render(&theme, &engine, &mut before);
    dock.set_starting_motion(0.5, 0.5);
    assert_eq!(dock.icon_jump("files"), 3.0);
    let mut after = kanesumi_canvas::Scene::default();
    dock.render(&theme, &engine, &mut after);
    assert_eq!(before, after, "改变轻跳相位不能改变定位线呼吸");
    dock.set_starting_motion(0.0, 0.5);
    assert_eq!(dock.icon_jump("files"), 3.0);
    let mut line_changed = kanesumi_canvas::Scene::default();
    dock.render(&theme, &engine, &mut line_changed);
    assert_ne!(after, line_changed, "呼吸必须能在轻跳不变时独立变化");
    assert_eq!(dock.icon_jump("not-starting"), 0.0);
    dock.set_starting_progress(0.5);
    assert_eq!(dock.icon_jump("files"), 3.0, "旧入口仍共同驱动");
    dock.set_starting_motion(f32::NAN, f32::INFINITY);
    assert_eq!(dock.icon_jump("files"), 0.0);
}
