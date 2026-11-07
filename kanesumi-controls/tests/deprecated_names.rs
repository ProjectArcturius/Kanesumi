// deprecated_names —— 兼容层契约：旧名必须仍能编译、且与**新名**行为一致。
//
// 为什么单独一个集成测试：`#[deprecated]` 只发警告、不改行为，所以「留一个周期」这件事
// 没有任何东西在挡——有人把旧名的实现删掉或改歪，编译照样过、单元测试照样绿。本测把
// 「旧名 == 新名」钉住，等真正删除旧名时连同本文件一起删（那也是一次破坏性变更，
// 参 docs/API_STABILITY.md §三）。
//
// 本文件整体 `#![allow(deprecated)]`：它就是来调旧名的。

#![allow(deprecated)]

use kanesumi_controls::{
    MetroDropdownMenu, MetroTab, MetroTabRow, MetroTreeView, MenuItem, TreeViewNode,
};
use kanesumi_core::{Point, Rect};

#[test]
fn visible_rows_forwards_to_compute_visible_rows() {
    let root = TreeViewNode::with_children(
        "",
        vec![TreeViewNode::new("文档"), TreeViewNode::new("图片")],
    );
    let view = MetroTreeView::new(root);
    assert_eq!(view.compute_visible_rows().len(), 2);
    assert_eq!(
        view.visible_rows(),
        view.compute_visible_rows(),
        "旧名 visible_rows 必须与新名同一结果"
    );
}

#[test]
fn overflowing_forwards_to_has_overflow() {
    let row = MetroTabRow::new(vec![
        MetroTab::new("一"),
        MetroTab::new("二"),
        MetroTab::new("三"),
    ]);
    let engine = kanesumi_element::testing::test_engine();
    let wide = Rect::new(0.0, 0.0, 2000.0, 32.0);
    let narrow = Rect::new(0.0, 0.0, 12.0, 32.0);
    assert!(!row.has_overflow(&engine, wide), "宽矩形不溢出");
    assert!(row.has_overflow(&engine, narrow), "窄矩形溢出");
    for rect in [wide, narrow] {
        assert_eq!(
            row.overflowing(&engine, rect),
            row.has_overflow(&engine, rect),
            "旧名 overflowing 必须与新名同一结果"
        );
    }
}

#[test]
fn path_at_forwards_to_try_path_at() {
    let menu = MetroDropdownMenu::new(vec![
        MenuItem::new("新建"),
        MenuItem::new("删除"),
    ]);
    // 面板外的位置：两条路径都必须给 None（这里不依赖具体弹层几何）。
    let outside = Point::new(-100.0, -100.0);
    assert_eq!(menu.try_path_at(outside), None);
    assert_eq!(
        menu.path_at(outside),
        menu.try_path_at(outside),
        "旧名 path_at 必须与新名同一结果"
    );
}
