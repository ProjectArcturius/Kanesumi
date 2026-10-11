// textbox_vcenter.rs —— 无头像素门（任务 k-textbox-vcenter）。
//
// 目的：在无显示会话下，对 TextBox / PasswordBox / AutoSuggestBox / NumberBox 给定主体矩形，
// 用 `TextEngine` 的字形度量与真实放置公式（与 `kanesumi_render::glyph_layout` 同源：基线 =
// 行盒顶 + ascent，逐字形 metrics 摆放）算出输入文字的**墨迹包围盒**（物理像素），
// 逐条打印机器可读的 `GATE` 行，供 `tools/perf/textbox_vcenter.sh` 汇总判定：
//   - `dy`        = 墨迹垂直中心与边框内侧矩形中心之差（物理 px，2× 缩放）；
//   - `gap`       = 无裁剪墨迹高度 − 输入框内（经 PushClip 后）墨迹高度（物理 px）。
//
// 本测试自身断言：
//   1. `gap == 0`（下伸部没被切 —— 本次修复的主目标）；
//   2. 文字行盒中心 = 内容区中心（CONTROL_SPEC §34 / 任务书要求 1）。
// `dy ≤ 1` 的像素门由脚本按原书判定（不在测试里放宽）。
//
// 说明：本测试用字形位图 metrics 的空域（即光栅后的非背景像素范围）做像素域断言，
// 不重走 wgpu。参 `docs/research/k_textbox_vcenter/` 与 CONTROL_SPEC §34。

use kanesumi_canvas::SceneCommand;
use kanesumi_canvas::text::TextEngine;
use kanesumi_controls::text_box::{
    text_input_content_rect, text_input_inner_rect, text_input_line_rect,
};
use kanesumi_controls::{MetroAutoSuggestBox, MetroNumberBox, MetroPasswordBox, MetroTextBox};
use kanesumi_core::{MetroTheme, Rect, TextStyle};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::{Align, LayoutProps, Widget, WidgetId};

const SCALE: f32 = 2.0;
const TEXT: &str = "Hgjpqy 中文汉字";
/// 删除按钮字形（不计入正文墨迹）。
const DELETE_GLYPH: &str = "✕";

/// 单条 Text 命令的字形墨迹包围盒（物理像素）。
fn glyph_ink(engine: &TextEngine, content: &str, rect: Rect, style: TextStyle) -> Option<Rect> {
    let size_phys = style.size * SCALE;
    let ascent_log = engine.ascent(size_phys) / SCALE;
    let baseline_phys = (rect.origin.y + ascent_log) * SCALE;
    let glyphs =
        engine.shape_line_weighted(content, style.size, style.letter_spacing_em, style.weight);
    let mut pen = rect.origin.x;
    let (mut minx, mut miny, mut maxx, mut maxy) =
        (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    let mut any = false;
    for g in &glyphs {
        let (m, _) = engine.rasterize_glyph(g.font_id, g.glyph_id, size_phys);
        if m.width == 0 || m.height == 0 {
            pen += g.x_advance;
            continue;
        }
        let x0 = (pen + g.x_offset) * SCALE + m.xmin as f32;
        let y0 = baseline_phys - g.y_offset * SCALE - m.ymin as f32 - m.height as f32;
        minx = minx.min(x0);
        miny = miny.min(y0);
        maxx = maxx.max(x0 + m.width as f32);
        maxy = maxy.max(y0 + m.height as f32);
        any = true;
        pen += g.x_advance;
    }
    any.then(|| Rect::new(minx, miny, maxx - minx, maxy - miny))
}

fn union(a: Option<Rect>, b: Rect) -> Option<Rect> {
    Some(match a {
        None => b,
        Some(a) => {
            let x0 = a.origin.x.min(b.origin.x);
            let y0 = a.origin.y.min(b.origin.y);
            let x1 = a.right().max(b.right());
            let y1 = a.bottom().max(b.bottom());
            Rect::new(x0, y0, x1 - x0, y1 - y0)
        }
    })
}

/// 正文墨迹：未裁剪（`ink`）与经当前 PushClip 裁剪后（`clipped`）。
struct Measurement {
    ink: Rect,
    clipped: Rect,
}

fn measure_ink(h: &TestHarness) -> Measurement {
    let engine = &h.engine;
    let mut clips: Vec<Rect> = Vec::new();
    let mut ink: Option<Rect> = None;
    let mut clipped: Option<Rect> = None;
    for cmd in &h.last.scene.commands {
        match cmd {
            SceneCommand::PushClip { rect } => clips.push(Rect::new(
                rect.origin.x * SCALE,
                rect.origin.y * SCALE,
                rect.size.width * SCALE,
                rect.size.height * SCALE,
            )),
            SceneCommand::PopClip => {
                clips.pop();
            }
            SceneCommand::Text {
                content, rect, style, ..
            } => {
                if content == DELETE_GLYPH {
                    continue;
                }
                let Some(bbox) = glyph_ink(engine, content, *rect, *style) else {
                    continue;
                };
                ink = union(ink, bbox);
                let clip = clips
                    .iter()
                    .copied()
                    .reduce(|a, b| a.intersect(b).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)));
                let vis = match clip {
                    Some(c) => bbox.intersect(c),
                    None => Some(bbox),
                };
                if let Some(v) = vis {
                    clipped = union(clipped, v);
                }
            }
            _ => {}
        }
    }
    Measurement {
        ink: ink.expect("应至少有一条正文 Text 命令"),
        clipped: clipped.expect("正文墨迹未被完全裁掉"),
    }
}

/// 测量 + 打印 `GATE` 行 + 断言「不裁切」与「行盒居中」两项。
fn check(label: &str, h: &TestHarness, body: Rect) {
    let m = measure_ink(h);
    let inner = text_input_inner_rect(body, 2.0);
    let inner_px = Rect::new(
        inner.origin.x * SCALE,
        inner.origin.y * SCALE,
        inner.size.width * SCALE,
        inner.size.height * SCALE,
    );

    let ink_center = m.ink.center().y;
    let inner_center = inner_px.center().y;
    let dy = (ink_center - inner_center).abs();
    let gap = (m.ink.size.height - m.clipped.size.height).abs();
    println!(
        "GATE {label} ink_center={ink_center:.2} inner_center={inner_center:.2} dy={dy:.2} ink_h={:.2} clipped_h={:.2} gap={gap:.2}",
        m.ink.size.height, m.clipped.size.height
    );

    // 主目标：下伸部不被内边距切掉。
    assert!(
        gap <= 0.01,
        "[{label}] 墨迹高度被裁剪：未裁 {:.2} vs 裁剪后 {:.2}",
        m.ink.size.height,
        m.clipped.size.height
    );

    // 设计契约：行盒在内容区内垂直居中（任务书要求 1）。
    let line_height = MetroTheme::ether_dark().typography.body.line_height;
    let content = text_input_content_rect(body, 2.0);
    let line = text_input_line_rect(content, line_height);
    assert!(
        (line.center().y - content.center().y).abs() < 0.01,
        "[{label}] 行盒中心 {} 应等于内容区中心 {}",
        line.center().y,
        content.center().y
    );
}

fn with_box<T: Widget + 'static>(widget: T) -> (TestHarness, WidgetId) {
    kanesumi_canvas::set_surface_scale(SCALE);
    let mut h = TestHarness::new(600.0, 200.0);
    let id = h.tree.insert_with(
        h.root(),
        widget,
        LayoutProps {
            width: Some(400.0),
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        },
    );
    h.frame();
    h.tab(); // 聚焦：边框 2px，与「输入文字」状态一致
    (h, id)
}

#[test]
fn textbox_ink_metrics() {
    let mut tb = MetroTextBox::from_text(TEXT);
    tb.field.set_cursor(tb.field.text().chars().count());
    let (h, id) = with_box(tb);
    let body = h.rect(id);
    check("TextBox", &h, body);
}

#[test]
fn passwordbox_ink_metrics() {
    let (h, id) = with_box(MetroPasswordBox::with_text(TEXT));
    let body = h.rect(id);
    check("PasswordBox", &h, body);
}

#[test]
fn autosuggestbox_ink_metrics() {
    let mut ab = MetroAutoSuggestBox::new();
    ab.field.set_text(TEXT);
    let (h, id) = with_box(ab);
    let body = h.rect(id);
    check("AutoSuggestBox", &h, body);
}

#[test]
fn numberbox_ink_metrics() {
    let mut nb = MetroNumberBox::new();
    nb.field.set_text(TEXT);
    let (h, id) = with_box(nb);
    let body = h.rect(id);
    check("NumberBox", &h, body);
}
