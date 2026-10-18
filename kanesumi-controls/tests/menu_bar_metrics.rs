// 菜单自然宽度与实际文字排版回归。参 Ether PLAN_DAILY_2026-10-17.md §全局放大。
use std::sync::Mutex;

use kanesumi_canvas::text::TextLayoutOptions;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::menu_bar::{MenuBarItem, MetroMenuBar};
use kanesumi_core::{MetroTheme, Rect};
use kanesumi_element::testing::test_engine;

static SCALE_LOCK: Mutex<()> = Mutex::new(());

fn menu(labels: &[&str]) -> MetroMenuBar {
    MetroMenuBar::new(
        labels
            .iter()
            .map(|s| MenuBarItem::new(*s, vec![]))
            .collect(),
    )
}

#[test]
fn natural_width_keeps_every_header_at_subpixel_origins() {
    let _guard = SCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = test_engine();
    for scale in [1.0, 1.25, 1.5, 2.0, 2.5, 3.0] {
        kanesumi_canvas::set_surface_scale(scale);
        for labels in [
            vec!["File", "Edit", "View", "Help"],
            vec!["文件", "编辑", "查看", "帮助"],
        ] {
            let bar = menu(&labels);
            for x in [0.0, 0.125, 0.25, 0.375, 83.3] {
                let rect = Rect::new(x, 0.0, bar.total_width(&engine), 40.0);
                let headers = bar.header_rects(&engine, rect);
                assert_eq!(
                    headers.len(),
                    labels.len(),
                    "scale={scale}, x={x}, labels={labels:?}"
                );
                for pair in headers.windows(2) {
                    assert_eq!(pair[0].right(), pair[1].origin.x, "相邻背景必须共边");
                }
                for header in headers {
                    for edge in [header.origin.x, header.right()] {
                        assert!((edge * scale - (edge * scale).round()).abs() < 0.001);
                    }
                }
            }
        }
    }
    kanesumi_canvas::set_surface_scale(1.0);
}

#[test]
fn complete_headers_keep_their_text_without_ellipsis() {
    let _guard = SCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let engine = test_engine();
    for scale in [1.0, 1.25, 1.5, 2.0, 2.5, 3.0] {
        kanesumi_canvas::set_surface_scale(scale);
        let labels = ["File", "Edit", "View", "Help", "文件", "编辑", "查看"];
        let bar = menu(&labels);
        let rect = Rect::new(83.3, 0.0, 900.0, 40.0);
        let mut scene = Scene::default();
        bar.render(&MetroTheme::ether_light(), &engine, rect, rect, &mut scene);
        let mut count = 0;
        for command in &scene.commands {
            if let SceneCommand::Text {
                content,
                rect,
                style,
                wrap,
                max_lines,
                overflow,
                ..
            } = command
            {
                let layout = engine.layout_box(
                    content,
                    style.size,
                    TextLayoutOptions {
                        max_width: rect.size.width,
                        max_height: rect.size.height,
                        line_height: style.line_height,
                        letter_spacing_em: style.letter_spacing_em,
                        max_lines: *max_lines,
                        wrap: *wrap,
                        overflow: *overflow,
                        weight: style.weight,
                    },
                );
                assert!(
                    !layout.truncated,
                    "scale={scale}: {content} 被错误省略为 {:?}",
                    layout.lines
                );
                assert_eq!(layout.lines[0].content, *content);
                count += 1;
            }
        }
        assert_eq!(count, labels.len());
    }
    kanesumi_canvas::set_surface_scale(1.0);
}
