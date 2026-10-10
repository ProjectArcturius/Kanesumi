//! golden_offscreen.rs —— GPU 离屏渲染（CanvasV2）vs CPU 光栅器（CpuRenderer）金样逐像素对拍测试。
//!
//! 覆盖 8 个几何与文本/图片场景在 1× 与 2× 下的对比，
//! 断言各场景通道差 > 8 的像素比例 ≤ 0.5%（0.005）。

use std::sync::{Arc, Mutex};

// 多线程并行创建无头 GPU 设备会让 Mesa 段错误（2026-10-10 Arch 实测 SIGSEGV，单线程全过）；
// 各用例串行取设备，与 cargo test 的线程数无关。
static GPU_SERIAL: Mutex<()> = Mutex::new(());
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand, TextAlign, TextOverflow};
use kanesumi_core::{Color, FontWeight, Point, Rect, TextStyle};
use kanesumi_render::{CanvasV2, CpuRenderer, GpuContext};

fn get_test_engine() -> TextEngine {
    let font_path = [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc",
        "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
        "/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .into_iter()
    .find(|p| std::path::Path::new(p).exists())
    .expect("缺少用于金样对拍测试的中西文字体");
    TextEngine::load(font_path).expect("加载字体引擎失败")
}

/// 场景 1：纯色矩形（直角填充）。
fn scene_solid_rect() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.12, 0.12, 0.14, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.9, 0.2, 0.2, 1.0),
        rect: Rect::new(24.0, 24.0, 80.0, 60.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.2, 0.8, 0.3, 1.0),
        rect: Rect::new(130.0, 30.0, 100.0, 50.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.5, 0.9, 1.0),
        rect: Rect::new(30.0, 110.0, 90.0, 90.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.85, 0.85, 0.88, 1.0),
        rect: Rect::new(150.0, 120.0, 110.0, 100.0),
        corner_radius: 0.0,
    });
    s
}

/// 场景 2：圆角矩形。
fn scene_rounded_rect() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.12, 0.12, 0.14, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.2, 0.4, 0.8, 1.0),
        rect: Rect::new(30.0, 30.0, 100.0, 60.0),
        corner_radius: 6.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.8, 0.3, 0.2, 1.0),
        rect: Rect::new(150.0, 30.0, 120.0, 70.0),
        corner_radius: 12.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.7, 0.4, 1.0),
        rect: Rect::new(40.0, 130.0, 110.0, 90.0),
        corner_radius: 18.0,
    });
    s
}

/// 场景 3：描边。
fn scene_stroke_rect() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.1, 0.12, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    // 直角细描边
    s.commands.push(SceneCommand::StrokeRect {
        color: Color::new(0.9, 0.3, 0.3, 1.0),
        rect: Rect::new(30.0, 30.0, 90.0, 60.0),
        thickness: 1.0,
        corner_radius: 0.0,
    });
    // 直角粗描边
    s.commands.push(SceneCommand::StrokeRect {
        color: Color::new(0.2, 0.6, 0.9, 1.0),
        rect: Rect::new(150.0, 30.0, 100.0, 70.0),
        thickness: 3.0,
        corner_radius: 0.0,
    });
    // 圆角描边
    s.commands.push(SceneCommand::StrokeRect {
        color: Color::new(0.3, 0.8, 0.3, 1.0),
        rect: Rect::new(40.0, 140.0, 100.0, 80.0),
        thickness: 2.0,
        corner_radius: 8.0,
    });
    s
}

/// 场景 4：圆弧与胶囊。
fn scene_arc_capsule() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.1, 0.12, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    // 胶囊形填充（高度 32，圆角 16）
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.25, 0.5, 0.85, 1.0),
        rect: Rect::new(40.0, 50.0, 100.0, 32.0),
        corner_radius: 16.0,
    });
    // 胶囊形描边
    s.commands.push(SceneCommand::StrokeRect {
        color: Color::new(0.85, 0.45, 0.2, 1.0),
        rect: Rect::new(40.0, 110.0, 100.0, 32.0),
        thickness: 2.0,
        corner_radius: 16.0,
    });
    // 圆环（360°）
    s.commands.push(SceneCommand::Arc {
        center: Point::new(210.0, 90.0),
        radius: 24.0,
        thickness: 3.0,
        color: Color::new(0.2, 0.8, 0.6, 1.0),
        start_deg: 0.0,
        end_deg: 360.0,
    });
    s
}

/// 场景 5：半透明叠加。
fn scene_translucent_blend() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.95, 0.95, 0.95, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.9, 0.1, 0.1, 0.5),
        rect: Rect::new(40.0, 50.0, 110.0, 110.0),
        corner_radius: 8.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.8, 0.2, 0.5),
        rect: Rect::new(90.0, 80.0, 110.0, 110.0),
        corner_radius: 8.0,
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.3, 0.9, 0.6),
        rect: Rect::new(60.0, 120.0, 120.0, 90.0),
        corner_radius: 8.0,
    });
    s
}

/// 场景 6：裁剪栈。
fn scene_clip_stack() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.15, 0.15, 0.18, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::PushClip {
        rect: Rect::new(40.0, 40.0, 200.0, 200.0),
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.9, 0.4, 0.1, 1.0),
        rect: Rect::new(20.0, 20.0, 100.0, 100.0),
        corner_radius: 0.0,
    });
    s.commands.push(SceneCommand::PushClip {
        rect: Rect::new(90.0, 90.0, 120.0, 120.0),
    });
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.2, 0.8, 0.7, 1.0),
        rect: Rect::new(70.0, 70.0, 100.0, 100.0),
        corner_radius: 12.0,
    });
    s.commands.push(SceneCommand::PopClip);
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.8, 0.2, 0.8, 1.0),
        rect: Rect::new(160.0, 150.0, 100.0, 100.0),
        corner_radius: 6.0,
    });
    s.commands.push(SceneCommand::PopClip);
    s
}

/// 场景 7：中文与拉丁文字。
fn scene_text_cjk_latin() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.1, 0.12, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 4.0,
    });
    s.commands.push(SceneCommand::Text {
        content: "Kanesumi 2026".to_string(),
        rect: Rect::new(30.0, 40.0, 260.0, 32.0),
        color: Color::WHITE,
        style: TextStyle::new(20.0, 24.0, FontWeight::Bold),
        align: TextAlign::Left,
        wrap: false,
        max_lines: Some(1),
        overflow: TextOverflow::Clip,
    });
    s.commands.push(SceneCommand::Text {
        content: "平台无关渲染内核".to_string(),
        rect: Rect::new(30.0, 90.0, 260.0, 28.0),
        color: Color::new(0.85, 0.85, 0.85, 1.0),
        style: TextStyle::new(16.0, 20.0, FontWeight::Normal),
        align: TextAlign::Left,
        wrap: false,
        max_lines: Some(1),
        overflow: TextOverflow::Clip,
    });
    s
}

/// 场景 7b：整形文本（连字 / 组合附加符 / 组合 emoji / CJK 混排）。
/// K3 后排版单位是「整形后的字形串」，本场景验证 CPU 与 GPU 两路对同一段
/// 整形结果逐像素一致（簇映射不改变放置契约）。参 Ether docs/research/k_text_shaping/DESIGN.md。
fn scene_text_shaped() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.1, 0.12, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 4.0,
    });
    // 连字（office / affluent）+ 字距对（AV To）。
    s.commands.push(SceneCommand::Text {
        content: "office affluent AV To".to_string(),
        rect: Rect::new(24.0, 32.0, 280.0, 36.0),
        color: Color::WHITE,
        style: TextStyle::new(26.0, 32.0, FontWeight::Normal),
        align: TextAlign::Left,
        wrap: false,
        max_lines: Some(1),
        overflow: TextOverflow::Clip,
    });
    // 组合附加符（é = e + U+0301）与组合 emoji（ZWJ 家庭、国旗）。
    s.commands.push(SceneCommand::Text {
        content: "e\u{301} e\u{301} \u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467} \u{1F1E8}\u{1F1F3}".to_string(),
        rect: Rect::new(24.0, 96.0, 280.0, 32.0),
        color: Color::new(0.9, 0.9, 0.9, 1.0),
        style: TextStyle::new(22.0, 28.0, FontWeight::Normal),
        align: TextAlign::Left,
        wrap: false,
        max_lines: Some(1),
        overflow: TextOverflow::Clip,
    });
    // 多行自动换行（换行点不得切入簇内）。
    s.commands.push(SceneCommand::Text {
        content: "系统设置 Kanesumi 文字整形 e\u{301}e\u{301}e\u{301} 换行不切簇".to_string(),
        rect: Rect::new(24.0, 150.0, 150.0, 120.0),
        color: Color::new(0.8, 0.85, 0.95, 1.0),
        style: TextStyle::new(16.0, 22.0, FontWeight::Normal),
        align: TextAlign::Left,
        wrap: true,
        max_lines: None,
        overflow: TextOverflow::Clip,
    });
    s
}

/// 场景 8：图片（包含原色与染色、透明度）。
fn scene_image() -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.1, 0.1, 0.12, 1.0),
        rect: Rect::new(10.0, 10.0, 300.0, 300.0),
        corner_radius: 0.0,
    });
    let img_w = 48u32;
    let img_h = 48u32;
    let mut rgba = Vec::with_capacity((img_w * img_h * 4) as usize);
    for y in 0..img_h {
        for x in 0..img_w {
            let cx = x as f32 - 24.0;
            let cy = y as f32 - 24.0;
            let d = (cx * cx + cy * cy).sqrt();
            let alpha = ((20.0 - d).clamp(0.0, 1.0) * 255.0) as u8;
            rgba.extend_from_slice(&[50, 120, 220, alpha]);
        }
    }
    let rgba_arc: Arc<[u8]> = Arc::from(rgba.into_boxed_slice());

    // 原色绘制
    s.commands.push(SceneCommand::Image {
        rgba: rgba_arc.clone(),
        width: img_w,
        height: img_h,
        rect: Rect::new(40.0, 50.0, 48.0, 48.0),
        tint: None,
        opacity: 1.0,
    });
    // 染色绘制（暖橙色 tint）带透明度
    s.commands.push(SceneCommand::Image {
        rgba: rgba_arc,
        width: img_w,
        height: img_h,
        rect: Rect::new(130.0, 50.0, 48.0, 48.0),
        tint: Some(Color::new(0.95, 0.45, 0.1, 1.0)),
        opacity: 0.85,
    });
    s
}

/// 运行单个对拍测试，断言通道差 > 8 的像素比例 ≤ 0.5%（0.005）。
fn assert_golden_match(
    ctx: &Arc<GpuContext>,
    engine: &TextEngine,
    name: &str,
    scale: f32,
    scene: &Scene,
) {
    let (w, h) = (320.0, 320.0);
    let mut canvas = CanvasV2::offscreen(ctx.clone(), w, h, scale)
        .expect("创建 CanvasV2 离屏画布失败");
    canvas.render(engine, scene);
    let gpu_bytes = canvas.read_back();

    let mut cpu = CpuRenderer::new(w, h, scale);
    let cpu_bytes = cpu.render(engine, scene, None);

    assert_eq!(
        gpu_bytes.len(),
        cpu_bytes.len(),
        "[{name} @ {scale}x] GPU 与 CPU 读回字节长度不一致"
    );

    let total_pixels = ((w * scale) * (h * scale)) as usize;
    let mut bad_pixels = 0usize;
    let mut max_diff = 0i32;

    for i in 0..total_pixels {
        let (gr, gg, gb, ga) = (
            gpu_bytes[i * 4] as i32,
            gpu_bytes[i * 4 + 1] as i32,
            gpu_bytes[i * 4 + 2] as i32,
            gpu_bytes[i * 4 + 3] as i32,
        );
        let (cr, cg, cb, ca) = (
            cpu_bytes[i * 4] as i32,
            cpu_bytes[i * 4 + 1] as i32,
            cpu_bytes[i * 4 + 2] as i32,
            cpu_bytes[i * 4 + 3] as i32,
        );
        let dr = (gr - cr).abs();
        let dg = (gg - cg).abs();
        let db = (gb - cb).abs();
        let da = (ga - ca).abs();
        let d = dr.max(dg).max(db).max(da);
        if d > max_diff {
            max_diff = d;
        }
        if d > 8 {
            bad_pixels += 1;
        }
    }

    let bad_ratio = bad_pixels as f64 / total_pixels as f64;
    let bad_pct = bad_ratio * 100.0;
    println!(
        "[golden_offscreen] {name:<20} @ {scale:.0}x: bad_pixels={bad_pixels:>5}/{total_pixels} ({bad_pct:>6.3}%), max_diff={max_diff:>3}"
    );

    assert!(
        bad_ratio <= 0.005,
        "[{name} @ {scale}x] 通道差 > 8 的像素比例 {bad_ratio:.4}（{bad_pixels}/{total_pixels}）超过 0.5% 阈值！max_diff={max_diff}"
    );
}

fn run_for_both_scales<F>(name: &str, make_scene: F)
where
    F: Fn() -> Scene,
{
    let _serial = GPU_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let ctx = match GpuContext::headless() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("环境不支持无头 GPU（{e:?}），跳过金样对照测试");
            return;
        }
    };
    let engine = get_test_engine();
    let scene = make_scene();
    for scale in [1.0f32, 2.0f32] {
        assert_golden_match(&ctx, &engine, name, scale, &scene);
    }
}

#[test]
fn test_golden_solid_rect() {
    run_for_both_scales("solid_rect", scene_solid_rect);
}

#[test]
fn test_golden_rounded_rect() {
    run_for_both_scales("rounded_rect", scene_rounded_rect);
}

#[test]
fn test_golden_stroke_rect() {
    run_for_both_scales("stroke_rect", scene_stroke_rect);
}

#[test]
fn test_golden_arc_capsule() {
    run_for_both_scales("arc_capsule", scene_arc_capsule);
}

#[test]
fn test_golden_translucent_blend() {
    run_for_both_scales("translucent_blend", scene_translucent_blend);
}

#[test]
fn test_golden_clip_stack() {
    run_for_both_scales("clip_stack", scene_clip_stack);
}

#[test]
fn test_golden_text_cjk_latin() {
    run_for_both_scales("text_cjk_latin", scene_text_cjk_latin);
}

#[test]
fn test_golden_text_shaped() {
    run_for_both_scales("text_shaped", scene_text_shaped);
}

#[test]
fn test_golden_image() {
    run_for_both_scales("image", scene_image);
}
