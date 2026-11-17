// dock_control.rs —— Dock 控件状态机与渲染的契约（Scene 命令断言，不看像素）。
//
// 判据来自 `docs/LAUNCHER_REBUILD.md`（双区：左区钉选未运行=灰线，右区运行=强调色线）
// 与 `docs/INTERACTION_CANON.md`（按下外观跟随目标、松开必须在同一槽上）。

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::navigation::{
    DockHit, DockIndicatorKind, DockItem, DockItemState, MetroDock, indicator_rect,
};
use kanesumi_core::{Color, MetroTheme, Point, Rect, ThemeColor};
use kanesumi_element::testing::test_engine;

fn dock(items: Vec<DockItem>) -> MetroDock {
    let mut d = MetroDock::new();
    d.set_items(items, Rect::new(0.0, 0.0, 800.0, 76.0));
    d
}

fn three() -> Vec<DockItem> {
    vec![
        DockItem::new("org.ether.settings", "settings").pinned(true),
        DockItem::new("org.ether.librarian", "files")
            .pinned(true)
            .running(true),
        DockItem::new("org.ether.launcher", "launcher").running(true),
    ]
}

fn center_of(d: &MetroDock, display: usize) -> Point {
    let s = &d.layout().slots[display];
    let r = s.hit.expect("槽位必须可点");
    Point::new(
        r.origin.x + r.size.width / 2.0,
        r.origin.y + r.size.height / 2.0,
    )
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

/// 测试夹具的合成口径与控件同一实现（`Color::over`）。
fn over(src: Color, dst: Color) -> Color {
    src.over(dst)
}

/// 场景在 `p` 处合成后的颜色（按 FillRect 顺序叠；裁剪不在本夹具的断言范围）。
fn fill_at(scene: &Scene, p: Point) -> Color {
    let mut out = None;
    for (rect, color) in fills(scene) {
        if rect.contains(p) {
            out = Some(match out {
                Some(dst) => over(color, dst),
                None => color,
            });
        }
    }
    out.unwrap_or_else(|| panic!("{p:?} 处没有任何填充"))
}

fn close(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.005
}

fn close_color(a: Color, b: Color) -> bool {
    close(a.r, b.r) && close(a.g, b.g) && close(a.b, b.b) && close(a.a, b.a)
}

/// C0 转录的三项输入（`docs/research/m1115_dock/REPORT.md` §三）：
/// librarian / nvim 钉选未运行，kitty 运行且激活。
fn c0_items() -> Vec<DockItem> {
    vec![
        DockItem::new("org.ether.librarian", "files").pinned(true),
        DockItem::new("nvim", "nvim").pinned(true),
        DockItem::new("kitty", "kitty").running(true).active(true),
    ]
}

/// C0 现场：1536 宽 × 76 高、入口格开、1×。真值 plate x 636 w 264。
fn c0_dock() -> MetroDock {
    let mut d = MetroDock::new();
    d.set_show_entry(true);
    d.set_items(c0_items(), Rect::new(0.0, 0.0, 1536.0, 76.0));
    d
}

fn hit_source(d: &MetroDock, pos: Point) -> Option<usize> {
    match d.hit(pos)? {
        DockHit::Slot(display) => d.layout().slot(display).map(|s| s.source),
        _ => None,
    }
}

fn render(d: &MetroDock) -> Scene {
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    scene
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
    assert_eq!(
        d.state_of(d.layout().slots[1].source),
        DockItemState::Hovered
    );
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
    assert_eq!(
        d.state_of(d.layout().slots[1].source),
        DockItemState::Hovered
    );
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
        let r = indicator_rect(&slots[display]).expect("可点槽必须有定位线");
        fills.iter().find(|(rect, _)| *rect == r).map(|(_, c)| *c)
    };
    // 颜色取 `DockIndicatorKind::token()`（C0 真值：Divider / Primary / PrimaryHover）。
    for (i, slot) in d.layout().slots.iter().enumerate() {
        let color = line_of(i).expect("每个可点槽都要有定位线");
        assert_eq!(
            color,
            slot.indicator_kind.token().resolve(&theme),
            "槽 {i} 的线色必须走 token"
        );
    }
    assert_eq!(
        d.layout().slots[0].indicator_kind,
        DockIndicatorKind::Pinned
    );
    assert_eq!(line_of(0), Some(theme.colors.divider), "左区灰线");
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
    // active = 正在用（图标位叠 10% hover tint）；urgent = 需要你注意（角上语义点）。
    // 旧用例断言「元素数不变」：新真值里静置格不铺底，active 会多一条叠色填充，
    // 因此改为断言**颜色**与**语义点**，而不是命令条数。
    let mut active_items = three();
    active_items[1].active = true;
    let plain = dock(three());
    let active_dock = dock(active_items);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    plain.render(&theme, &engine, &mut s1);
    active_dock.render(&theme, &engine, &mut s2);
    let p = active_dock.layout().slots[1].icon_rect.center();
    assert_eq!(fill_at(&s1, p), theme.colors.surface, "静置格不铺底");
    let tinted = over(theme.indication.hover_tint, theme.colors.surface);
    assert!(close_color(fill_at(&s2, p), tinted), "active 格叠 10%");

    // urgent 仍是独立的语义点：多一个 critical 色填充。
    let mut urgent_items = three();
    urgent_items[1].urgent = true;
    let urgent = dock(urgent_items);
    let mut s3 = Scene::default();
    urgent.render(&theme, &engine, &mut s3);
    assert!(
        fills(&s3).iter().any(|(_, c)| *c == theme.status.critical),
        "urgent 应画 critical 点"
    );
}

#[test]
fn c0_truth_geometry_entry_and_hits() {
    let d = c0_dock();
    let plate = d.layout().plate.expect("三项应有底板");
    assert_eq!(plate, Rect::new(636.0, 0.0, 264.0, 68.0));
    let entry = d.layout().entry.expect("show_entry = true");
    assert_eq!(entry, Rect::new(648.0, 0.0, 60.0, 68.0));
    let xs: Vec<f32> = d.layout().slots.iter().map(|s| s.rect.origin.x).collect();
    assert_eq!(xs, vec![708.0, 768.0, 828.0], "librarian / nvim / kitty");
    assert_eq!(
        hit_source(&d, Point::new(738.0, 34.0)),
        Some(0),
        "738 = librarian"
    );
    assert_eq!(
        hit_source(&d, Point::new(858.0, 34.0)),
        Some(2),
        "858 = kitty"
    );
    assert_eq!(d.hit(Point::new(678.0, 34.0)), Some(DockHit::Entry));
}

#[test]
fn entry_off_keeps_previous_centering() {
    // 入口格关：整组回到 204 宽居中（旧行为，不许因本卡回退）。
    let mut d = MetroDock::new();
    d.set_items(c0_items(), Rect::new(0.0, 0.0, 1536.0, 76.0));
    assert_eq!(d.layout().plate, Some(Rect::new(666.0, 0.0, 204.0, 68.0)));
    let xs: Vec<f32> = d.layout().slots.iter().map(|s| s.rect.origin.x).collect();
    assert_eq!(xs, vec![678.0, 738.0, 798.0]);
    assert_eq!(d.layout().entry, None);
}

#[test]
fn indicator_uses_layout_rect_and_token_colors() {
    let d = c0_dock();
    let theme = MetroTheme::ether_dark();
    let scene = render(&d);
    let lines = fills(&scene);
    for (i, slot) in d.layout().slots.iter().enumerate() {
        let r = indicator_rect(slot).expect("可见槽必须有定位线");
        assert!(close(r.origin.y, 65.0), "槽 {i} 线 y = 65");
        assert!(close(r.size.height, 3.0), "槽 {i} 线高 = 3");
        let color = lines
            .iter()
            .find(|(rect, _)| *rect == r)
            .map(|(_, c)| *c)
            .expect("渲染必须画在同一条线上");
        assert_eq!(color, slot.indicator_kind.token().resolve(&theme));
    }
    assert_eq!(
        d.layout().slots[0].indicator_kind,
        DockIndicatorKind::Pinned
    );
    assert_eq!(DockIndicatorKind::Pinned.token(), ThemeColor::Divider);
    assert_eq!(
        d.layout().slots[2].indicator_kind,
        DockIndicatorKind::Focused,
        "运行且激活 = 亮档"
    );
    assert_eq!(
        d.layout().slots[2].indicator_kind.token(),
        ThemeColor::PrimaryHover
    );
}

#[test]
fn rest_hover_press_and_active_fills() {
    let mut d = c0_dock();
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let tinted = over(theme.indication.hover_tint, theme.colors.surface);
    let librarian = d.layout().slots[0].icon_rect.center();
    let kitty = d.layout().slots[2].icon_rect.center();

    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    assert_eq!(
        fill_at(&scene, librarian),
        theme.colors.surface,
        "静置不铺底"
    );
    assert!(
        close_color(fill_at(&scene, kitty), tinted),
        "运行且激活 = 10% 叠 surface"
    );

    d.hover(librarian);
    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    assert!(close_color(fill_at(&scene, librarian), tinted), "悬停 10%");

    d.press(librarian);
    let mut scene = Scene::default();
    d.render(&theme, &engine, &mut scene);
    let pressed = fill_at(&scene, librarian);
    assert!(close_color(pressed, tinted), "按下与悬停同档");
    assert_ne!(pressed, theme.colors.primary, "按下不得变强调色实心");
}

#[test]
fn badge_is_top_right_capsule_fifteen_high() {
    let mut items = c0_items();
    items[2].windows = 2;
    let mut d = MetroDock::new();
    d.set_show_entry(true);
    d.set_items(items, Rect::new(0.0, 0.0, 1536.0, 76.0));
    let scene = render(&d);
    let (badge, _, corner) = scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::FillRect {
                rect,
                corner_radius,
                ..
            } if (rect.size.height - 15.0).abs() < 0.01 && *corner_radius > 0.0 => {
                Some((*rect, c, *corner_radius))
            }
            _ => None,
        })
        .expect("窗口数 2 应有 15 高胶囊徽标");
    let icon = d.layout().slots[2].icon_rect;
    assert!(close(badge.origin.y, icon.origin.y - 1.0), "徽标贴图标顶边");
    assert!(
        close(badge.right(), icon.right() + 3.0),
        "徽标右缘超出图标 3 pt（右上角）"
    );
    assert!(close(badge.size.height, 15.0));
    assert!(close(corner, 7.5), "15 高胶囊 = 短边一半");
}

#[test]
fn ghost_ignores_hit_and_grays_line() {
    let mut d = c0_dock();
    d.set_ghosts(&["kitty".to_string()]);
    assert_eq!(d.hit(Point::new(858.0, 34.0)), None, "幽灵不参与命中");
    assert!(close(d.icon_opacity("kitty"), 0.25), "幽灵图标 25%");
    assert!(close(d.icon_opacity("nvim"), 1.0));
    let theme = MetroTheme::ether_dark();
    let scene = render(&d);
    let line = indicator_rect(&d.layout().slots[2]).expect("幽灵仍有线");
    let color = fills(&scene)
        .iter()
        .find(|(rect, _)| *rect == line)
        .map(|(_, c)| *c)
        .expect("幽灵线必须画");
    assert_eq!(color, theme.colors.divider, "幽灵线回灰");
}

#[test]
fn scroll_moves_left_zone_only() {
    let mut items = Vec::new();
    for i in 0..30 {
        items.push(DockItem::new(format!("p{i}"), "icon").pinned(true));
    }
    for i in 0..5 {
        items.push(DockItem::new(format!("r{i}"), "icon").running(true));
    }
    let mut d = MetroDock::new();
    d.set_show_entry(true);
    d.set_items(items, Rect::new(0.0, 0.0, 1536.0, 76.0));
    assert!(d.layout().overflow, "35 项在 1536 宽下应溢出");
    let first_left = |d: &MetroDock| {
        d.layout()
            .slots
            .iter()
            .find(|s| s.zone == kanesumi_controls::navigation::DockZone::Pinned && s.visible)
            .map(|s| s.source)
            .expect("左区应有可见槽")
    };
    let before_left = first_left(&d);
    let left_before = d.layout().zones[0].before;
    let right_sources_before: Vec<usize> = d
        .layout()
        .slots
        .iter()
        .filter(|s| s.zone == kanesumi_controls::navigation::DockZone::Running)
        .map(|s| s.source)
        .collect();
    let right_first = d.layout().zones[1].first_display;

    d.set_scroll([1, 0]);
    assert_eq!(d.layout().zones[0].before, left_before + 1, "左区滚出一槽");
    assert_eq!(d.layout().zones[1].before, 0, "右区不滚");
    assert_eq!(first_left(&d), before_left + 1, "左区首槽换人");
    let right_sources_after: Vec<usize> = d
        .layout()
        .slots
        .iter()
        .filter(|s| s.zone == kanesumi_controls::navigation::DockZone::Running)
        .map(|s| s.source)
        .collect();
    assert_eq!(right_sources_after, right_sources_before, "右区槽序不动");
    assert_eq!(d.layout().zones[1].first_display, right_first);
    assert_eq!(d.layout().zones[1].after, 0, "右区仍在末页");
}

#[test]
fn pin_gap_opens_a_slot_and_shifts_left_zone() {
    let mut d = c0_dock();
    let plate_before = d.layout().plate.expect("底板");
    let nvim_before = d
        .layout()
        .slots
        .iter()
        .find(|s| s.id == "nvim")
        .expect("nvim 槽")
        .rect
        .origin
        .x;
    d.set_pin_gap(Some(1));
    let gap = d.pin_gap_rect().expect("应让出一格");
    assert!(close(gap.size.width, 60.0));
    let plate_after = d.layout().plate.expect("底板");
    assert!(close(
        plate_after.size.width,
        plate_before.size.width + 60.0
    ));
    let nvim_after = d
        .layout()
        .slots
        .iter()
        .find(|s| s.id == "nvim")
        .expect("nvim 槽")
        .rect
        .origin
        .x;
    // 空槽让底板加宽 60 并重新居中：后续槽相对板右移一格，绝对位置只右移
    // 「一格 − 底板左移量」（60 − 30 = 30）。
    let shift = 60.0 - (plate_before.origin.x - plate_after.origin.x);
    assert!(close(nvim_after, nvim_before + shift), "左区后续槽让位一格");
    assert!(close(gap.right(), nvim_after));
    d.set_pin_gap(None);
    assert!(d.pin_gap_rect().is_none());
    assert!(close(
        d.layout().plate.expect("底板").size.width,
        plate_before.size.width
    ));
}

#[test]
fn scale_scales_line_and_plate() {
    let mut d = MetroDock::new();
    d.set_show_entry(true);
    d.set_scale(2.0);
    d.set_items(c0_items(), Rect::new(0.0, 0.0, 3072.0, 152.0));
    let plate = d.layout().plate.expect("底板");
    assert!(close(plate.size.width, 528.0), "2× 板宽");
    let line = indicator_rect(&d.layout().slots[0]).expect("线");
    assert!(close(line.origin.y, 130.0), "2× 线 y = 65 × 2");
    assert!(close(line.size.height, 6.0));
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
        act.map(|a| format!("Activated({})", a.id))
            .unwrap_or_else(|| "None".to_string())
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
