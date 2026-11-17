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
