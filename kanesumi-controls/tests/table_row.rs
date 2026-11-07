// table_row.rs —— 表格行（`kanesumi_controls::table`）的行为契约。
//
// 守住四件事：① 列宽按策略切分（`Star` 吃余量，固定列不让位）；② 整行点击带回**列号**，
// 行内多列才分得清点在哪；③ 行首三角只展开、不选择；④ 按下后位移超过 4pt 是拖动而非点选。
// 参 `docs/CONTROL_SPEC.md` §51、`docs/INTERACTION_CANON.md` §Ⅱ。

use kanesumi_controls::table::{self, Column, ColumnSet, ColumnWidth, MetroTableRow};
use kanesumi_controls::{
    TableRowActivated, TableRowClicked, TableRowContextRequested, TableRowDragged, TableRowToggled,
};
use kanesumi_core::{Point, Rect};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Stack;
use kanesumi_element::{LayoutProps, PointerButton, WidgetId};

/// 1000 宽窗口里的 400 宽行容器 —— 行宽由调用方给，不由内容撑。
const ROW_W: f32 = 400.0;

/// 三列：名称吃余量、日期 180、类型 90。行内可用宽 400-24=376 → 名称列 106。
fn columns() -> ColumnSet {
    ColumnSet::new([
        Column::new("名称", ColumnWidth::Star),
        Column::new("日期", ColumnWidth::Px(180.0)),
        Column::new("类型", ColumnWidth::Px(90.0)),
    ])
}

/// 建一行（或两行），返回各行 id。
fn build(h: &mut TestHarness, rows: &[MetroTableRow]) -> Vec<WidgetId> {
    let mut ids = Vec::new();
    let rows = rows.to_vec();
    h.frame_with_layout(|tree| {
        let col = tree.insert_with(
            tree.root(),
            Stack::column(),
            LayoutProps {
                width: Some(ROW_W),
                ..LayoutProps::default()
            },
        );
        for row in rows {
            ids.push(tree.insert(col, row));
        }
    });
    h.settle();
    ids
}

fn plain_row(index: usize) -> MetroTableRow {
    MetroTableRow::new(index, columns(), ["报告.pdf", "2026-11-07", "PDF"])
}

#[test]
fn columns_split_fixed_then_star() {
    // 纯几何函数：绘制、命中、表头都消费它，故此处的期望即全局口径。
    let cols = table::column_rects(&columns(), Rect::new(0.0, 0.0, 400.0, 28.0));
    let widths: Vec<f32> = cols.iter().map(|c| c.size.width).collect();
    assert_eq!(widths, vec![130.0, 180.0, 90.0], "余量应全部归 Star 列");
    assert_eq!(cols[2].right(), 400.0, "最后一列应贴到右缘");
}

#[test]
fn row_height_is_dense_row_token() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    assert_eq!(
        h.rect(ids[0]).size.height,
        table::ROW_H,
        "行高应取尺寸表的 28（密集列表档，与设置页 48 的列表行不同族）"
    );
}

#[test]
fn click_reports_which_column() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    let row = h.rect(ids[0]);
    // 行内可用宽 376（左右内边距 12）：名称 12..118、日期 118..298、类型 298..388。
    h.click_at(Point::new(row.origin.x + 350.0, row.center().y));
    let clicked: Vec<TableRowClicked> = h
        .take::<TableRowClicked>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        clicked,
        vec![TableRowClicked {
            index: 0,
            column: 2
        }],
        "点「类型」列应带回列号 2"
    );
}

#[test]
fn triangle_toggles_without_selecting() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0).with_expandable(false)]);
    let row = h.rect(ids[0]);
    // 三角槽：左内边距 12 → 12..24，取中心 18。
    h.click_at(Point::new(row.origin.x + 18.0, row.center().y));
    let toggled: Vec<TableRowToggled> = h
        .take::<TableRowToggled>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    let clicked: Vec<TableRowClicked> = h
        .take::<TableRowClicked>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(toggled, vec![TableRowToggled(0)], "点三角应投展开动作");
    assert!(clicked.is_empty(), "点三角不得同时选择整行");
}

#[test]
fn drag_past_threshold_is_not_a_click() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    let row = h.rect(ids[0]);
    let start = Point::new(row.origin.x + 60.0, row.center().y);
    h.press_at(start, PointerButton::Left);
    // 位移 10pt > 阈值 4pt。
    h.move_to(Point::new(start.x + 10.0, start.y));
    h.release_at(Point::new(start.x + 10.0, start.y), PointerButton::Left);
    h.frame();
    let dragged: Vec<TableRowDragged> = h
        .take::<TableRowDragged>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    let clicked: Vec<TableRowClicked> = h
        .take::<TableRowClicked>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(dragged.len(), 1, "超阈值的松开应投拖动");
    assert_eq!(dragged[0].index, 0);
    assert!(clicked.is_empty(), "拖动不得顺带改掉选择");
}

#[test]
fn small_jitter_still_selects() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    let row = h.rect(ids[0]);
    let start = Point::new(row.origin.x + 60.0, row.center().y);
    h.press_at(start, PointerButton::Left);
    // 位移 2pt < 阈值 4pt：这是手抖，仍然算点选。
    h.move_to(Point::new(start.x + 2.0, start.y));
    h.release_at(Point::new(start.x + 2.0, start.y), PointerButton::Left);
    h.frame();
    let clicked: Vec<TableRowClicked> = h
        .take::<TableRowClicked>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        clicked,
        vec![TableRowClicked {
            index: 0,
            column: 0
        }],
        "阈值内的抖动仍应点选"
    );
}

#[test]
fn row_index_travels_with_every_action() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0), plain_row(1)]);
    let second = h.rect(ids[1]);
    h.click_at(second.center());
    let clicked: Vec<TableRowClicked> = h
        .take::<TableRowClicked>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        clicked[0].index, 1,
        "第二行要带回行号 1，否则容器分不清点的是谁"
    );
}

#[test]
fn double_click_activates_and_right_click_requests_context() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    h.double_click_at(h.rect(ids[0]).center());
    let activated: Vec<TableRowActivated> = h
        .take::<TableRowActivated>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(activated, vec![TableRowActivated(0)], "双击应投打开");
    h.right_click(ids[0]);
    let ctx: Vec<TableRowContextRequested> = h
        .take::<TableRowContextRequested>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(ctx.len(), 1, "右键应投上下文请求");
    assert_eq!(ctx[0].index, 0);
}

#[test]
fn enter_activates_focused_row() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let ids = build(&mut h, &[plain_row(0)]);
    h.click(ids[0]);
    h.key(kanesumi_element::Key::Enter);
    let activated: Vec<TableRowActivated> = h
        .take::<TableRowActivated>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        activated,
        vec![TableRowActivated(0)],
        "聚焦行上按 Enter 应打开"
    );
}

// ── 表头 ────────────────────────────────────────────────────────────────────

/// 建一个表头，返回 id。
fn build_header(h: &mut TestHarness, sort: Option<(usize, bool)>) -> WidgetId {
    let mut id = None;
    h.frame_with_layout(|tree| {
        let col = tree.insert_with(
            tree.root(),
            Stack::column(),
            LayoutProps {
                width: Some(ROW_W),
                ..LayoutProps::default()
            },
        );
        let mut header = table::MetroTableHeader::new(columns());
        if let Some((c, asc)) = sort {
            header = header.with_sort(c, asc);
        }
        id = Some(tree.insert(col, header));
    });
    h.settle();
    id.expect("表头没建出来")
}

#[test]
fn header_click_requests_ascending_first() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let id = build_header(&mut h, None);
    let rect = h.rect(id);
    assert_eq!(rect.size.height, table::HEADER_H, "表头高应取 28");
    // 日期列（180 宽，起点 118）中点 → 列号 1。
    h.click_at(Point::new(rect.origin.x + 200.0, rect.center().y));
    let sorts: Vec<kanesumi_controls::SortRequested> = h
        .take::<kanesumi_controls::SortRequested>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        sorts,
        vec![kanesumi_controls::SortRequested {
            column: 1,
            ascending: true
        }],
        "首次点列头应从升序起步"
    );
}

#[test]
fn header_second_click_on_same_column_flips() {
    let mut h = TestHarness::new(1000.0, 400.0);
    let id = build_header(&mut h, Some((1, true)));
    let rect = h.rect(id);
    h.click_at(Point::new(rect.origin.x + 200.0, rect.center().y));
    let sorts: Vec<kanesumi_controls::SortRequested> = h
        .take::<kanesumi_controls::SortRequested>()
        .into_iter()
        .map(|(_, a)| a)
        .collect();
    assert_eq!(
        sorts,
        vec![kanesumi_controls::SortRequested {
            column: 1,
            ascending: false
        }],
        "同一列再点应反序"
    );
}
