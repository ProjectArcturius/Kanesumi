// osd.rs —— OSD 控件的契约（几何贴底居中 / 2s 自动消失 / 元素数稳定）。

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::osd::{MetroOsd, OSD_DURATION_MS, OsdGeometry, OsdKind};
use kanesumi_core::{MetroTheme, Point, Rect, Size};
use kanesumi_element::testing::test_engine;

const VP: Rect = Rect {
    origin: Point { x: 0.0, y: 0.0 },
    size: Size { width: 1280.0, height: 800.0 },
};

fn texts(scene: &Scene) -> Vec<String> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::Text { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

fn fills(scene: &Scene) -> usize {
    scene
        .commands
        .iter()
        .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
        .count()
}

#[test]
fn starts_hidden_and_renders_nothing() {
    let osd = MetroOsd::new();
    assert!(!osd.visible());
    let mut scene = Scene::default();
    osd.render(&MetroTheme::ether_dark(), &test_engine(), VP, &mut scene);
    assert!(scene.commands.is_empty());
}

#[test]
fn show_then_tick_keeps_then_hides() {
    let mut osd = MetroOsd::new();
    osd.show(OsdKind::Volume { level: 0.5, muted: false });
    assert!(osd.visible());
    assert!(!osd.tick(OSD_DURATION_MS - 1.0), "未到点不隐藏");
    assert!(osd.tick(2.0), "到点必须隐藏");
    assert!(!osd.visible());
}

#[test]
fn new_event_resets_timer() {
    let mut osd = MetroOsd::new();
    osd.show(OsdKind::Brightness { level: 0.2 });
    osd.tick(1500.0);
    osd.show(OsdKind::Brightness { level: 0.4 });
    assert!(osd.remaining_ms() > 1900.0, "新事件重置计时，不是排队");
}

#[test]
fn card_is_bottom_centered() {
    let engine = test_engine();
    let g = OsdGeometry::measure(&engine, &OsdKind::Volume { level: 0.5, muted: false }, VP);
    let left = g.card.origin.x - VP.origin.x;
    let right = VP.right() - g.card.right();
    assert!((left - right).abs() < 0.5, "水平居中：left={left} right={right}");
    assert!((VP.bottom() - g.card.bottom() - 48.0).abs() < 0.5, "离下沿 48");
    assert!(g.card.size.width >= 200.0);
}

#[test]
fn level_kinds_have_track_others_do_not() {
    let engine = test_engine();
    let vol = OsdGeometry::measure(&engine, &OsdKind::Volume { level: 0.3, muted: false }, VP);
    let bright = OsdGeometry::measure(&engine, &OsdKind::Brightness { level: 0.7 }, VP);
    let caps = OsdGeometry::measure(&engine, &OsdKind::CapsLock { on: true }, VP);
    assert!(vol.track.size.width > 0.0);
    assert!(bright.track.size.width > 0.0);
    assert_eq!(caps.track.size.width, 0.0, "CapsLock 无进度条");
    assert!(vol.card.size.height > caps.card.size.height, "有进度条的卡更高");
}

#[test]
fn fill_width_follows_level_and_clamps() {
    let engine = test_engine();
    let half = OsdGeometry::measure(&engine, &OsdKind::Volume { level: 0.5, muted: false }, VP);
    assert!((half.fill.size.width - half.track.size.width * 0.5).abs() < 0.5);
    let over = OsdGeometry::measure(&engine, &OsdKind::Brightness { level: 2.0 }, VP);
    assert!((over.fill.size.width - over.track.size.width).abs() < 0.5, "超过 1 夹紧");
    let under = OsdGeometry::measure(&engine, &OsdKind::Brightness { level: -1.0 }, VP);
    assert_eq!(under.fill.size.width, 0.0, "低于 0 夹紧");
}

#[test]
fn muted_volume_shows_muted_label_and_empty_fill() {
    let engine = test_engine();
    let muted = OsdGeometry::measure(&engine, &OsdKind::Volume { level: 0.8, muted: true }, VP);
    assert_eq!(muted.fill.size.width, 0.0, "静音时填充为 0");
    assert_eq!(OsdKind::Volume { level: 0.8, muted: true }.label(), "静音");
    assert_eq!(OsdKind::Volume { level: 0.5, muted: false }.label(), "音量 50%");
}

#[test]
fn element_count_is_stable_across_levels() {
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut low = MetroOsd::new();
    low.show(OsdKind::Volume { level: 0.1, muted: false });
    let mut high = MetroOsd::new();
    high.show(OsdKind::Volume { level: 0.9, muted: false });
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    low.render(&theme, &engine, VP, &mut s1);
    high.render(&theme, &engine, VP, &mut s2);
    assert_eq!(fills(&s1), fills(&s2), "只改颜色/宽度，不增减元素");
    assert_eq!(texts(&s1).len(), texts(&s2).len());
}

#[test]
fn render_emits_card_icon_label_and_track() {
    let osd = {
        let mut o = MetroOsd::new();
        o.show(OsdKind::Brightness { level: 0.6 });
        o
    };
    let mut scene = Scene::default();
    osd.render(&MetroTheme::ether_dark(), &test_engine(), VP, &mut scene);
    assert_eq!(fills(&scene), 4, "卡片 + 图标位 + 轨道 + 填充");
    assert_eq!(texts(&scene), vec!["亮度 60%".to_string()]);
    assert!(osd.icon_center(&test_engine(), VP).is_some());
}

#[test]
fn caps_lock_has_no_track_elements() {
    let osd = {
        let mut o = MetroOsd::new();
        o.show(OsdKind::CapsLock { on: true });
        o
    };
    let mut scene = Scene::default();
    osd.render(&MetroTheme::ether_dark(), &test_engine(), VP, &mut scene);
    assert_eq!(fills(&scene), 2, "卡片 + 图标位");
    assert_eq!(texts(&scene), vec!["大写锁定：开".to_string()]);
}

#[test]
fn media_label_follows_playing() {
    assert_eq!(OsdKind::Media { playing: true }.label(), "播放");
    assert_eq!(OsdKind::Media { playing: false }.label(), "暂停");
    assert_eq!(OsdKind::Media { playing: true }.glyph(), "play");
}