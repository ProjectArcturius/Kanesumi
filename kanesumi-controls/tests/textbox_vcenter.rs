//! textbox_vcenter.rs —— 无头离屏像素门（任务 k-textbox-vcenter，契约按裁定 §145）。
//!
//! 用真实控件产出的 Scene（只取文本命令与其裁剪，去掉底色 / 边框 / 图标等非文本图元），
//! 经 `kanesumi-render` 的 `CanvasV2` 离屏（`GpuContext::headless()`，与金样同设施）光栅成
//! 像素，在**最终像素**上测量，逐条打印机器可读的 `GATE` / `SAME` 行，供
//! `tools/perf/textbox_vcenter.sh` 汇总判定：
//!   - `dy`  = `H` 墨迹底行与按契约算出的基线行之差（物理 px）≤ 1；
//!   - `gap` = 无裁剪参考的墨迹上下界与输入框内（经 PushClip 后）的墨迹上下界之差（px）＝ 0；
//!   - `same_line`：caret / 选区 / 预编辑下划线 / 占位符与文字同一行盒基准（逐项数值）。
//!
//! 无头 GPU 不可用时**直接 panic**（像素门必须在真实像素上测量），不得静默跳过。
//! 参 docs/research/k_textbox_vcenter/ 与 CONTROL_SPEC §34。

use std::sync::Mutex;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_controls::text_box::TEXTBOX_CARET_W;
use kanesumi_controls::{MetroAutoSuggestBox, MetroNumberBox, MetroPasswordBox, MetroTextBox};
use kanesumi_core::{MetroTheme, Rect};
use kanesumi_element::testing::{TestHarness, test_engine};
use kanesumi_element::{Align, LayoutProps, Widget};
use kanesumi_render::{CanvasV2, GpuContext};

/// 多线程并行创建无头 GPU 设备会让 Mesa 段错误（与金样对拍同因），各用例串行取设备。
static GPU_SERIAL: Mutex<()> = Mutex::new(());

const TEXT: &str = "Hgjpqy 中文汉字";
/// 离屏画布逻辑尺寸（覆盖控件与行盒）。
const CANVAS_W: f32 = 560.0;
const CANVAS_H: f32 = 64.0;
/// 墨迹判定阈值（alpha）：低于此视为透明背景 / 反走样淡边。
const INK_ALPHA: u8 = 16;

/// 只保留文本命令与其裁剪（`keep_clip`）：去掉底色 / 边框 / 光标 / 图标等非文本图元。
fn filter_text(scene: &Scene, keep_clip: bool) -> Scene {
    let mut out = Scene::default();
    for cmd in &scene.commands {
        match cmd {
            SceneCommand::PushClip { rect } if keep_clip => {
                out.commands.push(SceneCommand::PushClip { rect: *rect });
            }
            SceneCommand::PopClip if keep_clip => out.commands.push(SceneCommand::PopClip),
            SceneCommand::Text { content, .. } if content != "✕" => {
                out.commands.push(cmd.clone());
            }
            _ => {}
        }
    }
    out
}

/// 首条正文 Text 命令的矩形（行盒顶 = `rect.origin.y`）。
fn first_text_rect(scene: &Scene) -> Rect {
    scene
        .commands
        .iter()
        .find_map(|c| match c {
            SceneCommand::Text { rect, .. } => Some(*rect),
            _ => None,
        })
        .expect("场景应含正文 Text 命令")
}

/// 像素 alpha ≥ `INK_ALPHA` 的墨迹包围盒（物理像素，闭区间 x0,y0,x1,y1）。
fn ink_bbox(bytes: &[u8], pw: u32, ph: u32) -> Option<(u32, u32, u32, u32)> {
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
    any.then_some((x0, y0, x1, y1))
}

/// 光栅一个 Scene 到离屏画布并读回像素（物理尺寸）。
fn raster(canvas: &mut CanvasV2, engine: &TextEngine, scale: f32, scene: &Scene) -> (Vec<u8>, u32, u32) {
    canvas.render(engine, scene);
    let bytes = canvas.read_back();
    let pw = (CANVAS_W * scale).round() as u32;
    let ph = (CANVAS_H * scale).round() as u32;
    (bytes, pw, ph)
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

/// 基线门：`H` 墨迹底行 vs 契约基线（行盒顶 + 行内主字体 ascent）。返回物理 px 偏差。
fn gate_baseline(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    theme: &MetroTheme,
    label: &str,
    h_scene: &Scene,
    scale: f32,
) {
    let input = filter_text(h_scene, true);
    let trect = first_text_rect(&input);
    let baseline_log = trect.origin.y + engine.ascent(theme.typography.body.size);
    let (bytes, pw, ph) = raster(canvas, engine, scale, &input);
    let (_, _, _, y1) = ink_bbox(&bytes, pw, ph).expect("H 应有墨迹");
    let ink_bottom = (y1 + 1) as f32;
    let baseline_phys = baseline_log * scale;
    let dy = (ink_bottom - baseline_phys).abs();
    println!(
        "GATE {label} scale={scale:.0} kind=baseline dy={dy:.2} baseline_phys={baseline_phys:.2} ink_bottom={ink_bottom:.2}"
    );
    assert!(
        dy <= 1.0,
        "[{label}@{scale}x] H 墨迹底行 {ink_bottom} 与契约基线 {baseline_phys} 差 {dy} 物理 px > 1"
    );
}

/// 零裁切门：输入框内（经 PushClip）与无裁剪参考的墨迹上下界之差（须为 0）。
fn gate_zero_clip(
    canvas: &mut CanvasV2,
    engine: &TextEngine,
    label: &str,
    scene: &Scene,
    scale: f32,
) {
    let input = filter_text(scene, true);
    let reference = filter_text(scene, false);
    let (ib, pw, ph) = raster(canvas, engine, scale, &input);
    let (ix0, iy0, ix1, iy1) = ink_bbox(&ib, pw, ph).expect("输入框内应有墨迹");
    let (rb, _, _) = raster(canvas, engine, scale, &reference);
    let (rx0, ry0, rx1, ry1) = ink_bbox(&rb, pw, ph).expect("无裁剪参考应有墨迹");
    let d_top = (iy0 as i64 - ry0 as i64).abs();
    let d_bottom = ((iy1 + 1) as i64 - (ry1 + 1) as i64).abs();
    let d_left = (ix0 as i64 - rx0 as i64).abs();
    let d_right = ((ix1 + 1) as i64 - (rx1 + 1) as i64).abs();
    let gap = d_top.max(d_bottom).max(d_left).max(d_right);
    println!(
        "GATE {label} scale={scale:.0} kind=zero_clip gap={gap} ink_top={iy0} ink_bottom={} ref_top={ry0} ref_bottom={}",
        iy1 + 1,
        ry1 + 1
    );
    assert_eq!(
        gap, 0,
        "[{label}@{scale}x] 输入框内墨迹与无裁剪参考不一致（某一边被裁）"
    );
}

/// 同一行盒基准门：caret / 选区 / 预编辑下划线贴同一行盒；占位符同一行盒顶。
fn gate_same_line(theme: &MetroTheme) {
    let engine = test_engine();
    let body = Rect::new(0.0, 0.0, 240.0, 35.0);
    let mut tb = MetroTextBox::from_text("Ether");
    tb.focus(); // 全选 + 聚焦边框
    tb.field.set_preedit("nǐ", Some(0));
    let mut scene = Scene::default();
    tb.render(theme, &engine, body, &mut scene);

    let line = tb.line_rect(theme, body);
    let baseline = line.origin.y + engine.ascent(theme.typography.body.size);
    let text_y = first_text_rect(&scene).origin.y;
    let caret: Vec<Rect> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { rect, .. } if rect.size.width == TEXTBOX_CARET_W => Some(*rect),
            _ => None,
        })
        .collect();
    let sel_tint = theme.colors.text_selection_tint;
    let sels: Vec<Rect> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { rect, color, .. } if *color == sel_tint => Some(*rect),
            _ => None,
        })
        .collect();
    let underlines: Vec<Rect> = scene
        .commands
        .iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { rect, .. } if rect.size.height == 1.0 => Some(*rect),
            _ => None,
        })
        .collect();

    assert_eq!(text_y, line.origin.y, "文字在行盒顶部");
    assert_eq!(caret.len(), 1, "聚焦时恰好一个光标");
    assert_eq!(caret[0].origin.y, line.origin.y, "光标贴行盒顶");
    assert_eq!(caret[0].size.height, line.size.height, "光标跨整行盒");
    assert!(!sels.is_empty(), "全选应有选区高亮");
    for s in &sels {
        assert_eq!(s.origin.y, line.origin.y, "选区贴行盒顶");
        assert_eq!(s.size.height, line.size.height, "选区跨整行盒");
    }
    assert!(!underlines.is_empty(), "组合态应有下划线");
    for u in &underlines {
        assert!(
            (u.origin.y - (baseline + 2.0)).abs() < 0.05,
            "下划线在基线下方 2px：{} vs {}",
            u.origin.y,
            baseline + 2.0
        );
    }

    // 占位符：单独一帧（空内容 + 未聚焦），文字行盒与正文同基准。
    let mut ph = MetroTextBox::with_placeholder("搜索");
    let mut ph_scene = Scene::default();
    ph.render(theme, &engine, body, &mut ph_scene);
    let ph_line = ph.line_rect(theme, body);
    let ph_text_y = first_text_rect(&ph_scene).origin.y;
    assert_eq!(ph_text_y, ph_line.origin.y, "占位符贴行盒顶");

    println!(
        "SAME TextBox line_top={:.2} text_y={:.2} caret_y={:.2} caret_h={:.2} sel_y={:.2} underline_y={:.2} baseline={:.2} ph_y={:.2}",
        line.origin.y, text_y, caret[0].origin.y, caret[0].size.height,
        sels[0].origin.y, underlines[0].origin.y, baseline, ph_text_y,
    );
}

/// 无头 GPU 像素门主用例：真实控件 → CanvasV2 离屏 → 最终像素上测三条契约。
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

    for scale in [1.0f32, 2.0f32] {
        // TextBox：拉丁 + CJK + 下伸部（g j p q y）。
        let (ht, _) = harness(MetroTextBox::from_text(TEXT));
        let (hh, _) = harness(MetroTextBox::from_text("H"));
        gate_baseline(canvas_for(scale, &mut c1, &mut c2), &hh.engine, &theme, "TextBox", &hh.last.scene, scale);
        gate_zero_clip(canvas_for(scale, &mut c1, &mut c2), &ht.engine, "TextBox", &ht.last.scene, scale);

        // PasswordBox：掩码 / 明文两态；基线取明文 `H`（几何与掩码态同）。
        let (hp, _) = harness(MetroPasswordBox::with_text(TEXT)); // 掩码
        let (hp_h, _) = harness(plain_password("H"));
        let (hp_plain, _) = harness(plain_password(TEXT));
        gate_baseline(canvas_for(scale, &mut c1, &mut c2), &hp_h.engine, &theme, "PasswordBox", &hp_h.last.scene, scale);
        gate_zero_clip(canvas_for(scale, &mut c1, &mut c2), &hp.engine, "PasswordBox.masked", &hp.last.scene, scale);
        gate_zero_clip(canvas_for(scale, &mut c1, &mut c2), &hp_plain.engine, "PasswordBox.plain", &hp_plain.last.scene, scale);

        // AutoSuggestBox。
        let (has, _) = harness(auto_suggest(TEXT));
        let (has_h, _) = harness(auto_suggest("H"));
        gate_baseline(canvas_for(scale, &mut c1, &mut c2), &has_h.engine, &theme, "AutoSuggestBox", &has_h.last.scene, scale);
        gate_zero_clip(canvas_for(scale, &mut c1, &mut c2), &has.engine, "AutoSuggestBox", &has.last.scene, scale);

        // NumberBox。
        let (hn, _) = harness(number_box(TEXT));
        let (hn_h, _) = harness(number_box("H"));
        gate_baseline(canvas_for(scale, &mut c1, &mut c2), &hn_h.engine, &theme, "NumberBox", &hn_h.last.scene, scale);
        gate_zero_clip(canvas_for(scale, &mut c1, &mut c2), &hn.engine, "NumberBox", &hn.last.scene, scale);
    }

    gate_same_line(&theme);

    // 离屏截图（供调度者目视；worker 不读图，只落路径）。仅设 `KANESUMI_TEXTBOX_SHOTS` 时。
    if let Ok(dir) = std::env::var("KANESUMI_TEXTBOX_SHOTS") {
        let dir = std::path::PathBuf::from(dir);
        let _ = std::fs::create_dir_all(&dir);
        let (ht, _) = harness(MetroTextBox::from_text(TEXT));
        dump_ppm(&dir, "textbox.ppm", &mut c2, &ht.engine, 2.0, &ht.last.scene);
        let (hp, _) = harness(MetroPasswordBox::with_text(TEXT));
        dump_ppm(&dir, "password_masked.ppm", &mut c2, &hp.engine, 2.0, &hp.last.scene);
        let (hpp, _) = harness(plain_password(TEXT));
        dump_ppm(&dir, "password_plain.ppm", &mut c2, &hpp.engine, 2.0, &hpp.last.scene);
        let (hn, _) = harness(number_box(TEXT));
        dump_ppm(&dir, "numberbox.ppm", &mut c2, &hn.engine, 2.0, &hn.last.scene);
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

/// 明文密码框（清掩码）——用于测量几何基线。
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
