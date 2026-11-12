// notification_card_render.rs —— 通知卡渲染的契约（Scene 命令断言，不看像素）。
//
// 为什么用命令断言：Kanesumi 的判据是「几何与状态由框架给出，外壳只光栅化」，
// 所以正确的验证对象是 SceneCommand 序列（矩形 / 颜色 / max_lines / 文字），而不是截图。
// 参 `docs/supply/BEHAVIOR_CORPUS.md` §三（手感与形态只能用状态转录测）。

use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_controls::notification_card::{
    CARD_BODY_MAX_LINES, CardContent, CardGeometry, CardVisual, render_card,
};
use kanesumi_core::MetroTheme;
use kanesumi_core::{Color, Point, Rect};
use kanesumi_element::testing::test_engine;

fn geo_for(content: &CardContent<'_>) -> CardGeometry {
    let engine = test_engine();
    CardGeometry::layout(
        &engine,
        Rect::new(0.0, 0.0, 360.0, 0.0),
        content.title,
        content.body,
        content.actions,
    )
}

fn render(content: &CardContent<'_>, visual: CardVisual) -> (CardGeometry, Scene) {
    let theme = MetroTheme::ether_dark();
    let geo = geo_for(content);
    let mut scene = Scene::default();
    render_card(&theme, &geo, content, visual, &mut scene);
    (geo, scene)
}

fn fills(scene: &Scene) -> Vec<(Rect, Color, f32)> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect {
                color,
                rect,
                corner_radius,
            } => Some((*rect, *color, *corner_radius)),
            _ => None,
        })
        .collect()
}

fn texts(scene: &Scene) -> Vec<(String, Rect, Color, Option<usize>, TextAlign)> {
    scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::Text {
                content,
                rect,
                color,
                max_lines,
                align,
                ..
            } => Some((content.clone(), *rect, *color, *max_lines, *align)),
            _ => None,
        })
        .collect()
}

fn labels(actions: &[&str]) -> Vec<String> {
    actions.iter().map(|s| s.to_string()).collect()
}

#[test]
fn card_background_is_square_and_full_width() {
    let acts: Vec<String> = Vec::new();
    let c = CardContent {
        app_name: "系统",
        title: "更新已就绪",
        body: "下次重启生效。",
        actions: &acts,
    };
    let (geo, scene) = render(&c, CardVisual::default());
    let first = fills(&scene).into_iter().next().expect("至少一个填充");
    assert_eq!(first.2, 0.0, "卡片必须是直角（Kanesumi 铁律：圆角 0）");
    assert_eq!(first.0, geo.card);
}

#[test]
fn title_is_one_line_and_body_two() {
    let acts: Vec<String> = Vec::new();
    let c = CardContent {
        app_name: "系统",
        title: "更新已就绪",
        body: "包含内核与图形栈，下次重启生效。",
        actions: &acts,
    };
    let (_geo, scene) = render(&c, CardVisual::default());
    let t = texts(&scene);
    assert!(
        t.iter()
            .any(|(s, _, _, m, _)| s == "更新已就绪" && *m == Some(1)),
        "标题必须 max_lines=1：{t:?}"
    );
    assert!(
        t.iter()
            .any(|(s, _, _, m, _)| s.contains("内核") && *m == Some(CARD_BODY_MAX_LINES)),
        "正文必须 max_lines={CARD_BODY_MAX_LINES}：{t:?}"
    );
}

#[test]
fn each_action_gets_its_own_fill_and_label() {
    let acts = labels(&["稍后", "立即重启"]);
    let c = CardContent {
        app_name: "系统",
        title: "更新已就绪",
        body: "正文",
        actions: &acts,
    };
    let (geo, scene) = render(&c, CardVisual::default());
    let t = texts(&scene);
    for a in &acts {
        assert!(t.iter().any(|(s, _, _, _, _)| s == a), "缺动作文字：{a}");
    }
    let action_fills = fills(&scene)
        .into_iter()
        .filter(|(r, _, _)| geo.actions.iter().any(|a| a == r))
        .count();
    assert_eq!(action_fills, acts.len(), "每个动作一块底");
    // 动作文字居中
    for (s, _, _, _, align) in &t {
        if acts.iter().any(|a| a == s) {
            assert_eq!(*align, TextAlign::Center);
        }
    }
}

#[test]
fn close_glyph_always_present_and_brightens_on_hover() {
    let acts: Vec<String> = Vec::new();
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: "b",
        actions: &acts,
    };
    let (_g1, plain) = render(&c, CardVisual::default());
    let (_g2, hover) = render(
        &c,
        CardVisual {
            close_hovered: true,
            ..CardVisual::default()
        },
    );
    let pick = |s: &Scene| {
        texts(s)
            .into_iter()
            .find(|(t, _, _, _, _)| t == "✕")
            .map(|(_, r, color, _, _)| (r, color))
            .expect("关闭字形必须始终存在（元素数稳定）")
    };
    let (r1, c1) = pick(&plain);
    let (r2, c2) = pick(&hover);
    assert_eq!(r1, r2, "悬停不改几何");
    assert_ne!(c1, c2, "悬停应提亮关闭字形");
}

#[test]
fn hovered_card_raises_background_color() {
    let acts: Vec<String> = Vec::new();
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: "b",
        actions: &acts,
    };
    let (_g1, plain) = render(&c, CardVisual::default());
    let (_g2, hover) = render(
        &c,
        CardVisual {
            hovered: true,
            ..CardVisual::default()
        },
    );
    let bg = |s: &Scene| fills(s).into_iter().next().unwrap().1;
    assert_ne!(bg(&plain), bg(&hover), "悬停抬一档底色");
}

#[test]
fn no_actions_means_no_action_labels() {
    let acts: Vec<String> = Vec::new();
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: "b",
        actions: &acts,
    };
    let (geo, scene) = render(&c, CardVisual::default());
    assert!(geo.actions.is_empty());
    // 应用名 + 标题 + 正文 + 关闭
    assert_eq!(texts(&scene).len(), 4);
}

#[test]
fn text_count_is_stable_across_hover_states() {
    let acts = labels(&["好"]);
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: "b",
        actions: &acts,
    };
    let (_g1, plain) = render(&c, CardVisual::default());
    let (_g2, busy) = render(
        &c,
        CardVisual {
            hovered: true,
            close_hovered: true,
            hovered_action: Some(0),
        },
    );
    assert_eq!(
        texts(&plain).len(),
        texts(&busy).len(),
        "悬停不得增减文字元素（否则损伤判定整片位移）"
    );
}

#[test]
fn body_rect_matches_measured_two_line_box() {
    let acts: Vec<String> = Vec::new();
    let body = "很长的一段正文用来撑满两行并触发省略号".repeat(2);
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: &body,
        actions: &acts,
    };
    let (geo, scene) = render(&c, CardVisual::default());
    let emitted = texts(&scene)
        .into_iter()
        .find(|(s, _, _, _, _)| s.starts_with("很长的一段正文"))
        .expect("正文命令必须在场");
    assert_eq!(emitted.1, geo.body, "命令矩形必须与量测矩形一致");
    assert!(geo.body.size.height > 0.0);
}

#[test]
fn hit_tests_follow_geometry() {
    let acts = labels(&["好"]);
    let c = CardContent {
        app_name: "系统",
        title: "t",
        body: "b",
        actions: &acts,
    };
    let (geo, _scene) = render(&c, CardVisual::default());
    let a = geo.actions[0];
    let inside = Point::new(a.origin.x + 2.0, a.origin.y + 2.0);
    assert_eq!(geo.action_at(inside), Some(0));
    assert!(geo.hits_close(Point::new(
        geo.close.origin.x + 2.0,
        geo.close.origin.y + 2.0
    )));
}
