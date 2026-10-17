//! textbox_vcenter.rs —— 无头离屏像素门（任务 k-textbox-vcenter，契约按裁定 §145）。
//!
//! 用真实控件产出的 Scene（只取文本 / 光标 / 选区 / 下划线命令与其裁剪，去掉底色 / 边框 /
//! 图标等非文本图元），经 `kanesumi-render` 的 `CanvasV2` 离屏（`GpuContext::headless()`，
//! 与金样同设施）光栅成像素，在**最终像素**上测量，逐条打印机器可读的 `GATE` / `SAME` 行，
//! 供 `tools/perf/textbox_vcenter.sh` 汇总判定（四条）：
//!
//! - `baseline`：`H` 墨迹底行 vs **按契约长写算出**的基线（行盒顶 + ascent）≤ 1 物理 px。
//! - `baseline_masked`：掩码字形墨迹底行 vs 无裁剪 Label 在同一契约行盒内的墨迹底行 ≤ 1。
//! - `zero_clip`：输入框内（经 PushClip）墨迹四界 = 独立无裁剪 Label 在同契约行盒内的墨迹四界（差 0 px）。
//! - `clip`：正文命令生效时的最内层 `PushClip` 恰为契约的边框内侧矩形。
//! - `SAME`：caret / 选区 / 预编辑下划线 / 占位符的**最终像素**范围落在契约行盒的对应位置上。
//!
//! 三条契约的**期望值一律在测试侧独立长写**（由主体矩形 + 边框厚度 + 样式行高算出），
//! 不取自被测控件的命令几何 —— 否则整行同步错移无法被识别（2026-10-12 复核 §Ⅳ）。
//! 无头 GPU 不可用时**直接 panic**（像素门必须在真实像素上测量），不得静默跳过。
//! 参 docs/research/k_textbox_vcenter/ 与 CONTROL_SPEC §34。

use std::sync::Mutex;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::number_box::NUMBERBOX_SPIN_COLUMN_W;
use kanesumi_controls::text_box::{TEXTBOX_CARET_W, TEXT_INPUT_PAD_L, TEXT_INPUT_PAD_R};
use kanesumi_controls::{MetroAutoSuggestBox, MetroNumberBox, MetroPasswordBox, MetroTextBox};
use kanesumi_core::{Color, MetroTheme, Rect, ThemeColor};
use kanesumi_element::testing::{TestHarness, test_engine};
use kanesumi_element::widgets::Label;
use kanesumi_element::{Align, Insets, LayoutProps, Widget};
use kanesumi_render::{CanvasV2, GpuContext};

/// 多线程并行创建无头 GPU 设备会让 Mesa 段错误（与金样对拍同因），各用例串行取设备。
static GPU_SERIAL: Mutex<()> = Mutex::new(());

/// 正文串：拉丁 + CJK + 下伸部（g j p q y）。
const TEXT: &str = "Hgjpqy 中文汉字";
/// 基线探针：`H` 的墨迹底行即基线。
const PROBE: &str = "H";
/// 占位符串。
const PLACEHOLDER: &str = "搜索";
/// 离屏画布逻辑尺寸（覆盖控件与行盒）。
const CANVAS_W: f32 = 560.0;
const CANVAS_H: f32 = 64.0;
/// 墨迹判定阈值（alpha）：低于此视为透明背景 / 反走样淡边。
const INK_ALPHA: u8 = 16;

/// 像素墨迹包围盒（物理像素，闭区间）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ink {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

/// 只保留目标命令与其裁剪（`keep_clip`）。正文筛选滤掉删除按钮的 `✕` 字形 —— 它不是正文，
/// 且字体缺字形时是方框，会污染墨迹包围盒。
///
/// 光标 / 选区 / 下划线都只由 `FillRect` 承载（边框走 `StrokeRect`，不参与），故可按
/// 几何或色值唯一识别：光标宽 = `TEXTBOX_CARET_W`；选区色 = 选区令牌；下划线是
/// 高 1、宽 ≤ 4 的短划。
fn filter_scene(scene: &Scene, keep_clip: bool, kind: Filter) -> Scene {
    let mut out = Scene::default();
    for cmd in &scene.commands {
        match cmd {
            SceneCommand::PushClip { rect } if keep_clip => {
                out.commands.push(SceneCommand::PushClip { rect: *rect });
            }
            SceneCommand::PopClip if keep_clip => out.commands.push(SceneCommand::PopClip),
            _ if kind.wants(cmd) => out.commands.push(cmd.clone()),
            _ => {}
        }
    }
    out
}

/// 元素筛选器：正文（含占位符）/ 光标 / 选区 / 下划线。
#[derive(Clone, Copy, Debug)]
enum Filter {
    /// 正文与占位符文本。
    Text,
    /// 光标（宽 = `TEXTBOX_CARET_W`）。
    Caret,
    /// 选区高亮（色 = 选区令牌）。
    Selection(Color),
    /// 预编辑下划线（高 1、宽 ≤ 4 的短划）。
    Underline,
}

impl Filter {
    fn wants(self, cmd: &SceneCommand) -> bool {
        match (self, cmd) {
            (Filter::Text, SceneCommand::Text { content, .. }) => content != "✕" && !content.is_empty(),
            (Filter::Caret, SceneCommand::FillRect { rect, .. }) => {
                (rect.size.width - TEXTBOX_CARET_W).abs() < 1e-4
            }
            (Filter::Selection(tint), SceneCommand::FillRect { color, .. }) => *color == tint,
            (Filter::Underline, SceneCommand::FillRect { rect, .. }) => {
                (rect.size.height - 1.0).abs() < 1e-4 && rect.size.width <= 4.0 + 1e-4
            }
            _ => false,
        }
    }
}

/// 像素 alpha ≥ `INK_ALPHA` 的墨迹包围盒（物理像素）。
fn ink_bbox(bytes: &[u8], pw: u32, ph: u32) -> Option<Ink> {
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut any = false;
    for y in 0..ph {
        for x in 0..pw {
            if bytes[((y * pw + x) * 4 + 3) as usize] >= INK_ALPHA {
                any = true;
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    any.then_some(Ink { x0, y0, x1, y1 })
}

/// 光栅一个 Scene 到离屏画布并读回像素（物理尺寸）。
fn raster(canvas: &mut CanvasV2, engine: &TextEngine, scale: f32, scene: &Scene) -> (Vec<u8>, u32, u32) {
    canvas.render(engine, scene);
    let bytes = canvas.read_back();
    let pw = (CANVAS_W * scale).round() as u32;
    let ph = (CANVAS_H * scale).round() as u32;
    (bytes, pw, ph)
}

/// 光栅并直接返回墨迹包围盒（无墨迹 → panic，带标签）。
fn ink_of(canvas: &mut CanvasV2, engine: &TextEngine, scale: f32, scene: &Scene, what: &str) -> Ink {
    let (bytes, pw, ph) = raster(canvas, engine, scale, scene);
    ink_bbox(&bytes, pw, ph).unwrap_or_else(|| panic!("{what} 应有墨迹"))
}

/// 用真实控件在固定布局（起点 0,0，宽 400）下出一帧，返回夹具与主体矩形。
fn harness<W: Widget + 'static>(widget: W) -> (TestHarness, Rect) {
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
    let body = h.rect(id);
    (h, body)
}

fn canvas_for<'a>(scale: f32, c1: &'a mut CanvasV2, c2: &'a mut CanvasV2) -> &'a mut CanvasV2 {
    if scale < 1.5 { c1 } else { c2 }
}

// ── 契约几何（测试侧独立长写，**不调用被测实现**）──────────────────────────────
//
// 裁定 §145：行框中心 = 边框内侧矩形中心；基线 = 行盒顶 + 行内主字体 ascent。
// 这里按主体矩形 + 边框厚度 + 样式行高把契约重写一遍 —— 若期望值取自被测控件的命令
// 几何，整行同步错移就无法被识别（2026-10-12 复核 §Ⅳ 指出的旧门缺陷）。

/// 契约：边框内侧矩形 = 文字区四边各内缩 `border`（原点同步平移）。
fn contract_inner(area: Rect, border: f32) -> Rect {
    Rect::new(
        area.origin.x + border,
        area.origin.y + border,
        (area.size.width - 2.0 * border).max(0.0),
        (area.size.height - 2.0 * border).max(0.0),
    )
}

/// 契约行盒：横向取文字区扣边框与 Padding（`10,6,6,5` 的左右两值），纵向在内侧矩形内居中。
fn contract_line(area: Rect, border: f32, line_height: f32) -> Rect {
    let inner = contract_inner(area, border);
    Rect::new(
        area.origin.x + border + TEXT_INPUT_PAD_L,
        inner.origin.y + (inner.size.height - line_height) / 2.0,
        (area.size.width - 2.0 * border - TEXT_INPUT_PAD_L - TEXT_INPUT_PAD_R).max(0.0),
        line_height,
    )
}

/// 一个被测控件用例。
struct Case {
    label: &'static str,
    /// 文字区（正文行盒与裁剪框所在矩形）。TextBox / PasswordBox / AutoSuggestBox = 主体；
    /// NumberBox = 主体去掉右侧 Spin 列（`NUMBERBOX_SPIN_COLUMN_W`）。
    area: Rect,
    /// 边框厚度（未聚焦 = 1）。
    border: f32,
    /// 契约行盒（独立长写；横向 = 文字区扣边框与 Padding，纵向 = 内侧矩形内居中）。
    line: Rect,
    /// 整帧 Scene。
    scene: Scene,
    /// 期望的显示串（掩码态 = 掩码字符重复；供无裁剪 Label 参照使用）。
    display: String,
}

/// 真实控件出一帧 → 用例（含契约行盒）。
fn make_case<W: Widget + 'static>(
    label: &'static str,
    theme: &MetroTheme,
    widget: W,
    spin_column: bool,
    display: impl Into<String>,
) -> Case {
    let (h, body) = harness(widget);
    let border = 1.0;
    let area = if spin_column {
        Rect::new(
            body.origin.x,
            body.origin.y,
            (body.size.width - NUMBERBOX_SPIN_COLUMN_W).max(0.0),
            body.size.height,
        )
    } else {
        body
    };
    Case {
        label,
        area,
        border,
        line: contract_line(area, border, theme.typography.body.line_height),
        scene: h.last.scene.clone(),
        display: display.into(),
    }
}

/// 独立无裁剪 Label 参照：同字体同字号同色、**无裁剪**、文字矩形恰为契约行盒。
/// 返回其墨迹包围盒（物理像素）—— 契约位置上的字形真实上下界。
fn label_ink(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    scale: f32,
    text: &str,
    line: Rect,
    color: ThemeColor,
) -> Ink {
    let mut h = TestHarness::new(CANVAS_W, CANVAS_H);
    let id = h.tree.insert_with(
        h.root(),
        Label::new(text.to_string()).color(color),
        LayoutProps {
            margin: Insets::new(line.origin.x, line.origin.y, 0.0, 0.0),
            width: Some(line.size.width),
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        },
    );
    h.frame();
    let rect = h.rect(id);
    assert!(
        (rect.origin.x - line.origin.x).abs() < 0.01
            && (rect.origin.y - line.origin.y).abs() < 0.01
            && (rect.size.height - line.size.height).abs() < 0.01,
        "参照 Label 未落在契约行盒上：{rect:?} vs {line:?}"
    );
    let scene = filter_scene(&h.last.scene, false, Filter::Text);
    ink_of(canvas, engine, scale, &scene, "参照 Label")
}

// ── 三条像素门 ────────────────────────────────────────────────────────────────

/// 基线门（明文）：`H` 墨迹底行 vs **契约基线**（契约行盒顶 + 行内主字体 ascent）。
/// 期望值来自测试侧长写，不取被测控件的命令几何 —— 整行同步错移会被识别。
fn gate_baseline(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    theme: &MetroTheme,
    case: &Case,
    scale: f32,
) {
    let baseline_log = case.line.origin.y + engine.ascent(theme.typography.body.size);
    let scene = filter_scene(&case.scene, true, Filter::Text);
    let ink = ink_of(canvas, engine, scale, &scene, case.label);
    let bottom = (ink.y1 + 1) as f32;
    let expected = baseline_log * scale;
    let dy = (bottom - expected).abs();
    println!(
        "GATE {} scale={scale:.0} kind=baseline dy={dy:.2} expected={expected:.2} ink_bottom={bottom:.2} line_top={:.2}",
        case.label, case.line.origin.y,
    );
    assert!(
        dy <= 1.0,
        "[{}@{scale}x] H 墨迹底行 {bottom} 与契约基线 {expected} 差 {dy} 物理 px > 1（行盒顶 {}）",
        case.label,
        case.line.origin.y,
    );
}

/// 基线门（掩码态）：掩码字形墨迹底行 vs 无裁剪 Label 在**同一契约行盒**内的墨迹底行。
/// 掩码字形（`●`）不落在基线上，故期望值取契约行盒内独立 Label 的实测像素，
/// 不以明文 `H` 代替（2026-10-12 复核 §Ⅳ 指出的旧门缺陷）。
fn gate_baseline_masked(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    case: &Case,
    scale: f32,
) {
    let expected = label_ink(canvas, engine, scale, &case.display, case.line, ThemeColor::OnSurface);
    let scene = filter_scene(&case.scene, true, Filter::Text);
    let ink = ink_of(canvas, engine, scale, &scene, case.label);
    let dy = ((ink.y1 + 1) as f32 - (expected.y1 + 1) as f32).abs();
    // `rel` = 墨迹中心相对主体矩形中心的偏移（物理 px）。行盒中心恒等于主体中心（边框对称、
    // 行盒在边框内侧矩形内居中），故该偏移与控件高矮、边框粗细无关，是字体 + 契约的常量 ——
    // 真链路夹具 B（无头锁屏截图）用它比对同一条契约。
    let body_center = (case.area.origin.y + case.area.size.height / 2.0) * scale;
    let ink_center = (ink.y0 as f32 + (ink.y1 + 1) as f32) / 2.0;
    let rel = ink_center - body_center;
    println!(
        "GATE {} scale={scale:.0} kind=baseline_masked dy={dy:.2} expected={} ink_bottom={} rel={rel:.2}",
        case.label,
        expected.y1 + 1,
        ink.y1 + 1,
    );
    assert!(
        dy <= 1.0,
        "[{}@{scale}x] 掩码墨迹底行 {} 与契约行盒内 Label 参照 {} 差 {dy} 物理 px > 1",
        case.label,
        ink.y1 + 1,
        expected.y1 + 1,
    );
}

/// 零裁切门：输入框内（经 PushClip）墨迹四界 = 独立无裁剪 Label 在**同一契约行盒**内的
/// 墨迹四界（差 0 px）。既证明没被裁，也证明行盒位置与契约一致。
fn gate_zero_clip(canvas: &mut CanvasV2, engine: &TextEngine, case: &Case, scale: f32) {
    let reference = label_ink(canvas, engine, scale, &case.display, case.line, ThemeColor::OnSurface);
    let scene = filter_scene(&case.scene, true, Filter::Text);
    let ink = ink_of(canvas, engine, scale, &scene, case.label);
    let d = [
        ink.x0.abs_diff(reference.x0),
        ink.y0.abs_diff(reference.y0),
        (ink.x1 + 1).abs_diff(reference.x1 + 1),
        (ink.y1 + 1).abs_diff(reference.y1 + 1),
    ];
    let gap = *d.iter().max().unwrap();
    println!(
        "GATE {} scale={scale:.0} kind=zero_clip gap={gap} ink=({},{},{},{}) ref=({},{},{},{})",
        case.label,
        ink.x0,
        ink.y0,
        ink.x1 + 1,
        ink.y1 + 1,
        reference.x0,
        reference.y0,
        reference.x1 + 1,
        reference.y1 + 1,
    );
    assert_eq!(
        gap, 0,
        "[{}@{scale}x] 输入框内墨迹 {:?} 与无裁剪 Label 参照 {:?} 不一致（某一边被裁 / 行盒错位）",
        case.label, ink, reference,
    );
}

/// 裁剪框门：正文命令生效时的**最内层** `PushClip` 必须等于契约的边框内侧矩形
/// （不含内边距）—— 契约要求「字形裁剪框为边框内侧矩形」，取内容区会把下伸部切掉，
/// 取整个控件矩形则等于没裁（长文本会压到 Spin 列上）。
fn gate_clip(case: &Case, scale: f32) {
    let mut stack: Vec<Rect> = Vec::new();
    let mut found: Option<Rect> = None;
    for cmd in &case.scene.commands {
        match cmd {
            SceneCommand::PushClip { rect } => stack.push(*rect),
            SceneCommand::PopClip => {
                stack.pop();
            }
            SceneCommand::Text { content, .. } if !content.is_empty() && content != "✕" => {
                found = stack.last().copied();
                break;
            }
            _ => {}
        }
    }
    let got = found.unwrap_or_else(|| panic!("[{}] 正文命令不在任何裁剪框内", case.label));
    let expected = contract_inner(case.area, case.border);
    let d = (got.origin.x - expected.origin.x)
        .abs()
        .max((got.origin.y - expected.origin.y).abs())
        .max((got.size.width - expected.size.width).abs())
        .max((got.size.height - expected.size.height).abs());
    println!(
        "GATE {} scale={scale:.0} kind=clip d={d:.2} got=({:.1},{:.1},{:.1},{:.1}) expected=({:.1},{:.1},{:.1},{:.1})",
        case.label,
        got.origin.x,
        got.origin.y,
        got.size.width,
        got.size.height,
        expected.origin.x,
        expected.origin.y,
        expected.size.width,
        expected.size.height,
    );
    assert!(
        d < 0.01,
        "[{}] 正文裁剪框 {got:?} 不等于契约的边框内侧矩形 {expected:?}",
        case.label,
    );
}

/// 只对确有该图元的控件调用（AutoSuggestBox / NumberBox 的 render 不画光标与选区）。
/// 同基线门（最终像素）：光标 / 选区 / 预编辑下划线的像素纵向范围 vs 契约行盒 / 契约基线。
/// 只对确有该图元的控件调用（AutoSuggestBox / NumberBox 的 render 不画光标与选区）。
fn gate_same_line(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    theme: &MetroTheme,
    scale: f32,
    label: &str,
    line: Rect,
    scene: &Scene,
) {
    let base = line.origin.y + engine.ascent(theme.typography.body.size);
    let bottom = line.origin.y + line.size.height;
    let checks = [
        ("caret", Filter::Caret, line.origin.y, bottom),
        (
            "selection",
            Filter::Selection(theme.colors.text_selection_tint),
            line.origin.y,
            bottom,
        ),
        ("underline", Filter::Underline, base + 2.0, base + 3.0),
    ];
    for (elem, filter, expect0, expect1) in checks {
        let sub = filter_scene(scene, true, filter);
        let ink = ink_of(canvas, engine, scale, &sub, elem);
        let y0 = ink.y0 as f32;
        let y1 = (ink.y1 + 1) as f32;
        let d = (y0 - expect0 * scale).abs().max((y1 - expect1 * scale).abs());
        println!(
            "SAME {label}@{scale:.0} {elem} y0={y0} y1={y1} expect0={:.2} expect1={:.2} d={d:.2}",
            expect0 * scale,
            expect1 * scale,
        );
        assert!(
            d <= 1.0,
            "[{label}@{scale}x] {elem} 像素范围 {y0}..{y1} 偏离契约 {:.2}..{:.2} 达 {d} 物理 px",
            expect0 * scale,
            expect1 * scale,
        );
    }
}

/// 占位符门（最终像素）：占位符墨迹四界 = 无裁剪 Label 在**同一契约行盒**内的墨迹四界。
/// 证明占位符与正文同一基线（不是自己另算一份位置）。
fn gate_placeholder(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    scale: f32,
    label: &str,
    line: Rect,
    scene: &Scene,
    text: &str,
) {
    let reference = label_ink(canvas, engine, scale, text, line, ThemeColor::OnSurfaceVariant);
    let sub = filter_scene(scene, true, Filter::Text);
    let ink = ink_of(canvas, engine, scale, &sub, label);
    let d = [
        ink.x0.abs_diff(reference.x0),
        ink.y0.abs_diff(reference.y0),
        (ink.x1 + 1).abs_diff(reference.x1 + 1),
        (ink.y1 + 1).abs_diff(reference.y1 + 1),
    ];
    let gap = *d.iter().max().unwrap();
    println!(
        "SAME {label}@{scale:.0} placeholder y0={} y1={} ref_y0={} ref_y1={} d={gap}",
        ink.y0,
        ink.y1 + 1,
        reference.y0,
        reference.y1 + 1,
    );
    assert_eq!(
        gap, 0,
        "[{label}@{scale}x] 占位符墨迹 {ink:?} 与契约行盒内 Label 参照 {reference:?} 不一致"
    );
}

// ── 用例构造 ─────────────────────────────────────────────────────────────────

/// 明文密码框（清掩码）—— 用于量测明文态几何。
fn plain_password(text: &str) -> MetroPasswordBox {
    let mut p = MetroPasswordBox::new();
    p.field_mut().set_mask(None);
    p.field_mut().set_text(text);
    p
}

fn auto_suggest(text: &str) -> MetroAutoSuggestBox {
    let mut a = MetroAutoSuggestBox::new();
    a.field.set_text(text);
    a
}

fn number_box(text: &str) -> MetroNumberBox {
    let mut n = MetroNumberBox::new();
    n.field.set_text(text);
    n
}

/// 掩码显示串（与控件 `display_chars` 同长同字符）。
fn mask_string(len: usize) -> String {
    kanesumi_controls::password_box::PASSWORD_MASK_CHAR.to_string().repeat(len)
}

/// 聚焦 + IME 组合态的 TextBox（光标 / 选区 / 预编辑下划线三者同时在场）。
fn focused_textbox(text: &str) -> MetroTextBox {
    let mut tb = MetroTextBox::from_text(text);
    tb.focus(); // 全选 + 聚焦边框
    tb.field.set_preedit("nǐ", Some(0));
    tb
}

/// 聚焦 + IME 组合态的 PasswordBox（掩码态，与 TextBox 同一条渲染路径）。
fn focused_password(text: &str) -> MetroPasswordBox {
    let mut pw = MetroPasswordBox::with_text(text);
    pw.focus();
    pw.field_mut().set_preedit("nǐ", Some(0));
    pw
}

/// 无头 GPU 像素门主用例：真实控件 → CanvasV2 离屏 → 最终像素上测三条契约。
///
/// 输出机器可读行：`GATE <label> scale=<n> kind=<baseline|baseline_masked|zero_clip> ...`
/// 与 `SAME <label>@<n> <elem> ...`，由 `tools/perf/textbox_vcenter.sh` 汇总判定。
#[test]
fn textbox_pixel_gates() {
    let _serial = GPU_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx =
        GpuContext::headless().expect("无头 GPU 不可用：像素门必须在最终像素上测量，不静默跳过");
    let mut c1 = CanvasV2::offscreen(ctx.clone(), CANVAS_W, CANVAS_H, 1.0)
        .expect("创建 1× 离屏画布失败");
    let mut c2 = CanvasV2::offscreen(ctx.clone(), CANVAS_W, CANVAS_H, 2.0)
        .expect("创建 2× 离屏画布失败");
    let theme = MetroTheme::ether_dark();
    let engine = test_engine();

    for scale in [1.0f32, 2.0f32] {
        let cv = canvas_for(scale, &mut c1, &mut c2);

        // 基线门（明文 `H` 探针）：TextBox / PasswordBox 明文 / AutoSuggestBox / NumberBox。
        for case in [
            make_case("TextBox", &theme, MetroTextBox::from_text(PROBE), false, PROBE),
            make_case(
                "PasswordBox.plain",
                &theme,
                plain_password(PROBE),
                false,
                PROBE,
            ),
            make_case("AutoSuggestBox", &theme, auto_suggest(PROBE), false, PROBE),
            make_case("NumberBox", &theme, number_box(PROBE), true, PROBE),
        ] {
            gate_baseline(cv, &engine, &theme, &case, scale);
        }

        // 基线门（掩码态）：掩码字形 vs 同一契约行盒内的无裁剪 Label 参照。
        let masked_probe = make_case(
            "PasswordBox.masked",
            &theme,
            MetroPasswordBox::with_text(PROBE),
            false,
            mask_string(PROBE.chars().count()),
        );
        gate_baseline_masked(cv, &engine, &masked_probe, scale);

        // 零裁切门：五态（含掩码 / 明文两态）的墨迹四界 vs 同一契约行盒内的 Label 参照。
        for case in [
            make_case("TextBox", &theme, MetroTextBox::from_text(TEXT), false, TEXT),
            make_case(
                "PasswordBox.masked",
                &theme,
                MetroPasswordBox::with_text(TEXT),
                false,
                mask_string(TEXT.chars().count()),
            ),
            make_case(
                "PasswordBox.plain",
                &theme,
                plain_password(TEXT),
                false,
                TEXT,
            ),
            make_case("AutoSuggestBox", &theme, auto_suggest(TEXT), false, TEXT),
            make_case("NumberBox", &theme, number_box(TEXT), true, TEXT),
        ] {
            gate_zero_clip(cv, &engine, &case, scale);
            gate_clip(&case, scale);
        }

        // 同基线门（最终像素）：光标 / 选区 / 下划线（TextBox 与掩码 PasswordBox 同路径）。
        let tb_case = make_case("TextBox", &theme, focused_textbox("Ether"), false, "Ether");
        gate_same_line(cv, &engine, &theme, scale, "TextBox", tb_case.line, &tb_case.scene);
        let pw_case = make_case("PasswordBox", &theme, focused_password("Ether"), false, "Ether");
        gate_same_line(
            cv,
            &engine,
            &theme,
            scale,
            "PasswordBox",
            pw_case.line,
            &pw_case.scene,
        );

        // 占位符门（最终像素）：与正文同一契约行盒。
        for (label, case) in [
            (
                "TextBox",
                make_case(
                    "TextBox",
                    &theme,
                    MetroTextBox::with_placeholder(PLACEHOLDER),
                    false,
                    PLACEHOLDER,
                ),
            ),
            (
                "PasswordBox",
                make_case(
                    "PasswordBox",
                    &theme,
                    MetroPasswordBox::with_placeholder(PLACEHOLDER),
                    false,
                    PLACEHOLDER,
                ),
            ),
            (
                "AutoSuggestBox",
                make_case(
                    "AutoSuggestBox",
                    &theme,
                    MetroAutoSuggestBox::with_placeholder(PLACEHOLDER),
                    false,
                    PLACEHOLDER,
                ),
            ),
        ] {
            gate_placeholder(cv, &engine, scale, label, case.line, &case.scene, PLACEHOLDER);
        }
    }

    // 离屏截图（供调度者目视）。仅设 `KANESUMI_TEXTBOX_SHOTS` 时。
    if let Ok(dir) = std::env::var("KANESUMI_TEXTBOX_SHOTS") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        for (name, scene) in [
            ("textbox", make_case("TextBox", &theme, MetroTextBox::from_text(TEXT), false, TEXT)),
            (
                "password_masked",
                make_case(
                    "PasswordBox.masked",
                    &theme,
                    MetroPasswordBox::with_text(TEXT),
                    false,
                    mask_string(TEXT.chars().count()),
                ),
            ),
            (
                "password_plain",
                make_case("PasswordBox.plain", &theme, plain_password(TEXT), false, TEXT),
            ),
            ("numberbox", make_case("NumberBox", &theme, number_box(TEXT), true, TEXT)),
        ] {
            dump_ppm(&dir, &format!("{name}.ppm"), &mut c2, &engine, 2.0, &scene.scene);
        }
    }
}

/// 落盘一张 PPM（P6）：最终像素（含底色 / 边框 / 文字）。仅在设了
/// `KANESUMI_TEXTBOX_SHOTS` 时调用；调度者据此目视，脚本再转 PNG 入库。
fn dump_ppm(
    dir: &std::path::Path,
    name: &str,
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    scale: f32,
    scene: &Scene,
) {
    let (bytes, pw, ph) = raster(canvas, engine, scale, scene);
    let mut out = format!("P6\n{pw} {ph}\n255\n").into_bytes();
    for px in bytes.chunks_exact(4) {
        out.extend_from_slice(&px[..3]);
    }
    let _ = std::fs::write(dir.join(name), out);
}
