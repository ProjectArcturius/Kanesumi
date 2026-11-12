// list_row_model.rs —— MetroList 行模型扩展的契约（图标 / 次要文字 / 多列，只增不改）。

use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::list::{ListRowMeta, MetroList};
use kanesumi_core::{MetroTheme, Rect};
use kanesumi_element::testing::test_engine;

const RECT: Rect = Rect {
    origin: kanesumi_core::Point { x: 0.0, y: 0.0 },
    size: kanesumi_core::Size { width: 400.0, height: 200.0 },
};

fn rows() -> Vec<String> {
    vec!["报告.pdf".to_string(), "照片.png".to_string()]
}

fn labels(scene: &Scene) -> Vec<String> {
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
fn plain_list_behaves_as_before() {
    let list = MetroList::new(rows());
    assert_eq!(list.meta().len(), 0);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    list.render(&theme, &engine, RECT, &mut scene);
    assert_eq!(labels(&scene).len(), 2, "纯文字列表只出两行文字");
    assert_eq!(fills(&scene), 0, "没有图标就不该有图标底");
}

#[test]
fn meta_without_icon_changes_nothing_structural() {
    let list = MetroList::with_meta(
        rows(),
        vec![
            ListRowMeta::default().secondary("12 KB"),
            ListRowMeta::default().secondary("48 KB"),
        ],
    );
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    list.render(&theme, &engine, RECT, &mut scene);
    assert_eq!(fills(&scene), 0);
    let l = labels(&scene);
    assert_eq!(l.len(), 4, "两行文字 + 两个次要文字");
    assert!(l.contains(&"12 KB".to_string()));
}

#[test]
fn icon_reserves_a_slot_and_shifts_text() {
    let plain = MetroList::new(rows());
    let with_icon = MetroList::with_meta(
        rows(),
        vec![ListRowMeta::default().icon("file"), ListRowMeta::default()],
    );
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let (mut s1, mut s2) = (Scene::default(), Scene::default());
    plain.render(&theme, &engine, RECT, &mut s1);
    with_icon.render(&theme, &engine, RECT, &mut s2);
    assert_eq!(fills(&s2), 1, "只有第一行有图标位");
    let x_of = |s: &Scene, want: &str| {
        s.commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::Text { content, rect, .. } if content == want => Some(rect.origin.x),
                _ => None,
            })
            .expect("文字必须在场")
    };
    assert!(
        x_of(&s2, "报告.pdf") > x_of(&s1, "报告.pdf"),
        "有图标时文字右移"
    );
    assert_eq!(x_of(&s2, "照片.png"), x_of(&s1, "照片.png"), "无图标的行不动");
}

#[test]
fn cells_are_right_aligned_and_bounded() {
    let list = MetroList::with_meta(
        rows(),
        vec![
            ListRowMeta::default().cells(vec!["12 KB".into(), "昨天".into()]),
            ListRowMeta::default(),
        ],
    );
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    list.render(&theme, &engine, RECT, &mut scene);
    let l = labels(&scene);
    assert!(l.contains(&"12 KB".to_string()));
    assert!(l.contains(&"昨天".to_string()));
    for c in scene.commands.iter() {
        if let SceneCommand::Text { rect, .. } = c {
            assert!(rect.right() <= RECT.right() + 0.5, "文字不得溢出列表");
        }
    }
}

#[test]
fn meta_shorter_than_rows_does_not_panic() {
    let list = MetroList::with_meta(rows(), vec![ListRowMeta::default().icon("file")]);
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    list.render(&theme, &engine, RECT, &mut scene);
    assert_eq!(labels(&scene).len(), 2, "缺元数据的行按纯文字画");
}

#[test]
fn set_meta_replaces_and_row_meta_reads() {
    let mut list = MetroList::new(rows());
    assert!(list.row_meta(0).is_none());
    list.set_meta(vec![ListRowMeta::default().secondary("x")]);
    assert_eq!(list.row_meta(0).unwrap().secondary, "x");
    assert!(list.row_meta(9).is_none());
}

#[test]
fn hover_changes_color_not_element_count() {
    let mut list = MetroList::with_meta(
        rows(),
        vec![ListRowMeta::default().icon("a"), ListRowMeta::default().icon("b")],
    );
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut plain = Scene::default();
    list.render(&theme, &engine, RECT, &mut plain);
    list.hovered = Some(0);
    let mut hovered = Scene::default();
    list.render(&theme, &engine, RECT, &mut hovered);
    assert_eq!(
        labels(&plain).len(),
        labels(&hovered).len(),
        "悬停不改变文字元素数"
    );
}

#[test]
fn empty_list_renders_nothing() {
    let list = MetroList::new(Vec::new());
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();
    let mut scene = Scene::default();
    list.render(&theme, &engine, RECT, &mut scene);
    assert!(labels(&scene).is_empty());
}