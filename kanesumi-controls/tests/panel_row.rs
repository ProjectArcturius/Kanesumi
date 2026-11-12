// panel_row.rs —— 面板行控件的契约（TopBar 三面板 48 处手算布局的替代品）。

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::panel_row::{
    MetroPanelRow, PanelRow, PanelRowGeometry, PanelRowHit, PanelRowState,
};
use kanesumi_core::{MetroTheme, Point, Rect};
use kanesumi_element::testing::test_engine;

const W: f32 = 360.0;

fn geo(row: &PanelRow) -> PanelRowGeometry {
    PanelRowGeometry::measure(&test_engine(), row, Point::new(0.0, 0.0), W)
}

#[test]
fn height_comes_from_font_not_a_constant() {
    let g = geo(&PanelRow::action("a", "音量"));
    assert!(g.row.size.height >= 32.0, "命中高度下限 32");
    assert_eq!(g.row.size.width, W);
}

#[test]
fn icon_shifts_text_right() {
    let plain = geo(&PanelRow::action("a", "音量"));
    let with_icon = geo(&PanelRow::action("a", "音量").icon("volume"));
    assert_eq!(with_icon.icon.size.width, kanesumi_core::sizes::ICON_16);
    assert!(
        with_icon.title.origin.x > plain.title.origin.x,
        "有图标时主文字必须右移"
    );
    assert_eq!(plain.icon.size.width, 0.0);
}

#[test]
fn control_area_width_depends_on_kind() {
    let action = geo(&PanelRow::action("a", "打开设置"));
    let toggle = geo(&PanelRow::toggle("t", "Wi-Fi", true));
    let value = geo(&PanelRow::value("v", "版本", "1.0.3"));
    assert_eq!(action.control.size.width, 24.0, "动作行 = 箭头区");
    assert_eq!(toggle.control.size.width, 36.0, "开关行 = 开关宽");
    assert!(value.control.size.width > 0.0);
    for g in [action, toggle, value] {
        assert!(g.control.right() <= g.row.right(), "右侧控件不得溢出");
        assert!(g.title.right() <= g.control.origin.x, "文字不得压到控件");
    }
}

#[test]
fn control_hit_takes_priority_over_row() {
    let g = geo(&PanelRow::toggle("t", "蓝牙", false));
    let c = g.control;
    let in_control = Point::new(c.origin.x + 4.0, c.origin.y + 4.0);
    assert_eq!(g.hit(in_control), Some(PanelRowHit::Control));
    let in_row = Point::new(g.title.origin.x + 4.0, g.title.origin.y + 4.0);
    assert_eq!(g.hit(in_row), Some(PanelRowHit::Row));
    assert_eq!(g.hit(Point::new(-10.0, -10.0)), None);
}

#[test]
fn states_follow_hover_and_press() {
    let mut r = MetroPanelRow::new(PanelRow::action("a", "x"));
    assert_eq!(r.state(), PanelRowState::Rest);
    r.hover(true);
    assert_eq!(r.state(), PanelRowState::Hovered);
    assert!(r.press());
    assert_eq!(r.state(), PanelRowState::Pressed);
    assert_eq!(r.release(Some(PanelRowHit::Row)), Some(true));
    // 松手后指针仍在行上 → 回到 Hovered（不是 Rest）：状态只描述交互，不猜指针去哪了。
    assert_eq!(r.state(), PanelRowState::Hovered);
    r.hover(false);
    assert_eq!(r.state(), PanelRowState::Rest);
}

#[test]
fn disabled_row_ignores_press_and_release() {
    let mut r = MetroPanelRow::new(PanelRow::action("a", "x").disabled(true));
    assert_eq!(r.state(), PanelRowState::Disabled);
    assert!(!r.press());
    assert_eq!(r.release(Some(PanelRowHit::Row)), None);
}

#[test]
fn control_release_does_not_trigger_row() {
    let mut r = MetroPanelRow::new(PanelRow::toggle("t", "Wi-Fi", false));
    r.press();
    assert_eq!(r.release(Some(PanelRowHit::Control)), Some(false));
}

#[test]
fn render_draws_title_secondary_and_control() {
    let row = PanelRow::toggle("t", "Wi-Fi", true)
        .icon("wifi")
        .secondary("已连接");
    let r = MetroPanelRow::new(row.clone());
    let g = geo(&row);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    r.render(&theme, &engine, &g, &mut scene);
    let texts: Vec<String> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"Wi-Fi".to_string()));
    assert!(texts.contains(&"已连接".to_string()));
    let fills = scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count();
    assert!(fills >= 2, "图标位 + 开关轨道至少两块");
}

#[test]
fn value_row_prints_value() {
    let row = PanelRow::value("v", "版本", "1.0.3");
    let r = MetroPanelRow::new(row.clone());
    let g = geo(&row);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    r.render(&theme, &engine, &g, &mut scene);
    let texts: Vec<String> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect();
    assert!(texts.contains(&"1.0.3".to_string()), "值行必须显示值：{texts:?}");
}

#[test]
fn hover_changes_color_not_element_count() {
    let row = PanelRow::action("a", "打开设置").icon("settings");
    let g = geo(&row);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut plain = MetroPanelRow::new(row.clone());
    let mut hovered = MetroPanelRow::new(row);
    hovered.hover(true);
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    plain.render(&theme, &engine, &g, &mut s1);
    hovered.render(&theme, &engine, &g, &mut s2);
    assert_eq!(s1.commands.len() + 1, s2.commands.len(), "悬停只多一层底色");
}

#[test]
fn narrow_width_does_not_panic_or_overlap() {
    let engine = test_engine();
    let g = PanelRowGeometry::measure(
        &engine,
        &PanelRow::toggle("t", "一个很长很长的开关行标题", true).secondary("次要"),
        Point::new(0.0, 0.0),
        80.0,
    );
    assert!(g.title.size.width >= 0.0);
    assert!(g.row.size.width == 80.0);
}

#[test]
fn geometry_is_reusable_for_same_width() {
    let engine = test_engine();
    let row = PanelRow::action("a", "同宽");
    let a = PanelRowGeometry::measure(&engine, &row, Point::new(0.0, 0.0), W);
    let b = PanelRowGeometry::measure(&engine, &row, Point::new(0.0, 0.0), W);
    assert_eq!(a, b, "同输入同输出（外壳可放心缓存）");
    let _: Rect = a.row;
}