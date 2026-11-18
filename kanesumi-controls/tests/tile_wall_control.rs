// tile_wall_control.rs —— 磁贴墙状态机的契约（拖动 / 尺寸档 / 键盘 / 索引跳转）。

use kanesumi_canvas::Scene;
use kanesumi_controls::navigation::tile_wall::TileWallSize;
use kanesumi_controls::navigation::tile_wall_control::WallDropTarget;
use kanesumi_controls::navigation::{
    MetroTileWall, WallGroup, WallRelease, WallTile, group_layout,
};
use kanesumi_core::{MetroTheme, Point, Rect};
use kanesumi_element::testing::test_engine;

const VP: Rect = Rect { origin: Point { x: 0.0, y: 0.0 }, size: kanesumi_core::Size { width: 2000.0, height: 600.0 } };

fn wall() -> MetroTileWall {
    let mut w = MetroTileWall::new();
    w.set_groups(
        vec![
            WallGroup::new(
                "A",
                vec![
                    WallTile::new("a1", "A1", TileWallSize::Small),
                    WallTile::new("a2", "A2", TileWallSize::Small),
                ],
            ),
            WallGroup::new(
                "B",
                vec![WallTile::new("b1", "B1", TileWallSize::Small)],
            ),
        ],
        VP,
    );
    w
}

fn center(w: &MetroTileWall, group: usize, index: usize) -> Point {
    let s = w
        .layout()
        .slots
        .iter()
        .find(|s| s.group == group && s.index == index)
        .expect("槽位必须在场");
    Point::new(s.hit.origin.x + s.hit.size.width / 2.0, s.hit.origin.y + s.hit.size.height / 2.0)
}

#[test]
fn layout_lists_all_tiles() {
    let w = wall();
    assert_eq!(w.layout().slots.len(), 3);
}

#[test]
fn hover_sets_and_clears() {
    let mut w = wall();
    let p = center(&w, 0, 1);
    assert!(w.hover(p));
    assert_eq!(w.hovered(), Some((0, 1)));
    w.hover(Point::new(5.0, 590.0));
    assert_eq!(w.hovered(), None);
}

#[test]
fn tap_without_drag_activates() {
    let mut w = wall();
    let p = center(&w, 0, 0);
    assert!(w.press(p));
    assert!(!w.dragging());
    assert_eq!(w.release(p), WallRelease::Activated("a1".to_string()));
}

#[test]
fn drag_beyond_threshold_records_drop_target_then_moves() {
    let mut w = wall();
    let from = center(&w, 0, 0);
    let to = center(&w, 0, 1);
    w.press(from);
    assert!(w.drag(to), "越阈值应进入拖动");
    assert!(w.dragging());
    assert_eq!(w.drop_target(), Some((0, 1)));
    let rel = w.release(to);
    assert_eq!(
        rel,
        WallRelease::Moved {
            from_group: 0,
            from_index: 0,
            to_group: 0,
            to_index: 1
        }
    );
    assert!(w.apply_move(&rel, VP));
    let ids: Vec<&str> = w.groups()[0].tiles.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["a2", "a1"], "拖到后一位应换序");
}

#[test]
fn small_movement_does_not_start_drag() {
    let mut w = wall();
    let p = center(&w, 0, 0);
    w.press(p);
    assert!(!w.drag(Point::new(p.x + 1.0, p.y + 1.0)));
    assert!(!w.dragging());
    assert_eq!(w.drop_target(), None);
}

#[test]
fn drag_out_of_wall_cancels() {
    let mut w = wall();
    let p = center(&w, 0, 0);
    w.press(p);
    w.drag(Point::new(-50.0, -50.0));
    assert_eq!(w.release(Point::new(-50.0, -50.0)), WallRelease::Cancelled);
}

#[test]
fn size_cycles_small_wide_large() {
    let mut w = wall();
    assert_eq!(w.cycle_size(0, 0, VP), Some(TileWallSize::Wide));
    assert_eq!(w.cycle_size(0, 0, VP), Some(TileWallSize::Large));
    assert_eq!(w.cycle_size(0, 0, VP), Some(TileWallSize::Small));
    assert_eq!(w.cycle_size(9, 9, VP), None, "越界不 panic");
}

#[test]
fn keyboard_walks_across_groups() {
    let mut w = wall();
    assert_eq!(w.focus_by(1), Some((0, 0)));
    assert_eq!(w.focus_by(1), Some((0, 1)));
    assert_eq!(w.focus_by(1), Some((1, 0)), "组内走到头进下一组");
    assert_eq!(w.focus_by(1), Some((1, 0)), "末尾夹紧");
    assert_eq!(w.focus_by(-3), Some((0, 0)), "向前夹紧到首个");
}

#[test]
fn jump_to_group_reports_offset() {
    let w = wall();
    let off = w.jump_to_group(1).expect("B 组必须在场");
    let gl = group_layout(w.layout(), 1).expect("组布局");
    assert!(off <= gl.rect.origin.x);
    assert!(w.jump_to_group(9).is_none());
}

#[test]
fn render_draws_group_titles_and_tiles() {
    let w = wall();
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    w.render(&theme, &engine, &mut scene);
    let texts = scene
        .commands
        .iter()
        .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { .. }))
        .count();
    assert_eq!(texts, 2, "两个组标题");
    let fills = scene
        .commands
        .iter()
        .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
        .count();
    assert_eq!(fills, 5, "2 个标题带 + 3 块磁贴位");
}

#[test]
fn empty_wall_renders_nothing() {
    let w = MetroTileWall::new();
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    w.render(&theme, &engine, &mut scene);
    assert!(scene.commands.is_empty());
}

// ── 提供方补丁（2026-11-18）：滚动偏移 / 视口外预取 / 拖动基准 ──────────────

#[test]
fn scroll_offsets_hit_and_layout_public_api() {
    let mut w = wall();
    assert_eq!(w.scroll(), 0.0);
    let before = center(&w, 0, 0);
    assert_eq!(
        w.layout().hit(before).map(|s| (s.group, s.index)),
        Some((0, 0))
    );

    w.set_scroll(60.0);
    assert!((w.layout().scroll - 60.0).abs() < 0.01, "滚动写回布局");
    // 内容左移 60：指针同点不再命中原来的格，左移 60 才命中同一块磁贴。
    assert_eq!(
        w.layout()
            .hit(Point::new(before.x - 60.0, before.y))
            .map(|s| (s.group, s.index)),
        Some((0, 0))
    );
}

#[test]
fn overscan_adds_offscreen_group_public_api() {
    let mut w = MetroTileWall::new();
    let vp = Rect::new(0.0, 0.0, 1000.0, 600.0);
    w.set_groups(
        vec![
            WallGroup::new(
                "A",
                vec![
                    WallTile::new("a1", "A1", TileWallSize::Small),
                    WallTile::new("a2", "A2", TileWallSize::Small),
                ],
            ),
            WallGroup::new("B", vec![WallTile::new("b1", "B1", TileWallSize::Small)]),
        ],
        vp,
    );
    assert_eq!(w.layout().groups.len(), 1, "B 组在视口外，旧行为不返回");
    assert!(!w.layout().slots.iter().any(|s| s.id == "b1"));

    w.set_overscan_cols(1);
    assert!(w.layout().groups.len() >= 2, "预取左右各 1 列");
    assert!(w.layout().slots.iter().any(|s| s.id == "b1"));

    w.set_overscan_cols(0);
    assert!(
        !w.layout().slots.iter().any(|s| s.id == "b1"),
        "关掉即回旧输出"
    );
}

#[test]
fn drop_target_at_uses_baseline_without_dragged_tile() {
    let mut w = wall();
    let from = center(&w, 0, 0);
    w.press(from);
    w.drag(Point::new(from.x + 20.0, from.y));
    assert!(w.dragging());

    // 基准布局里 a2 补到首格；点它左半 = 插回原位 → 按点击回报。
    let a2 = center(&w, 0, 1);
    assert_eq!(
        w.drop_target_at(Point::new(130.0, a2.y)),
        Some(WallDropTarget::Activated("a1".to_string()))
    );
    // 组内空白（a2 右缘之外）= 追加组尾 → 插到第 1 位。
    assert_eq!(
        w.drop_target_at(Point::new(a2.x + 100.0, a2.y)),
        Some(WallDropTarget::InGroup { group: 0, index: 1 })
    );
}
