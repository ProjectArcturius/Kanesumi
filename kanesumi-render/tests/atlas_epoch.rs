//! atlas_epoch.rs —— 图集换代同帧旧 UV 回归（任务 k-atlas-epoch）。
//!
//! 缺陷：`render_with_damage` 在 `encode_scene` 之前算好 `is_incremental` 与
//! `cull_rect`。编码途中字形页 / 图片页放满会 `rebuild_glyph_page` / `rebuild_image_page`
//! —— 换新纹理页、epoch+1、`refresh_bind_group`。此前已编码实例的 UV 仍是旧页坐标，
//! 却用新页 bind group 采样 → 错位字形 / 图片，且仍按旧损伤只清损伤区。
//!
//! 每个用例：先在增量画布上把最小图集页（2048²）填到「只容一个条目」，
//! 再以窄损伤提交含旧条目 + 一个新条目的 Scene（新条目必然触发换页），
//! 断言该帧输出与「同一 Scene 在新建 CanvasV2 上全幅绘制」逐像素一致（阈值 0.5%）。

use std::sync::{Arc, Mutex};

// 多线程并行创建无头 GPU 设备会让 Mesa 段错误；用例串行取设备。
static GPU_SERIAL: Mutex<()> = Mutex::new(());

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand, TextAlign, TextOverflow};
use kanesumi_core::{Color, FontWeight, Rect, TextStyle};
use kanesumi_render::{CanvasV2, GpuContext};

// 表面需足够大以容纳「撑满最小图集页」的大字形，否则字形全在屏外、无法验像素。
const SURF: f32 = 1600.0;
/// 最小图集页边长（参 canvas_v2.rs `ATLAS_PAGE_SIZES`）。
const MIN_PAGE: u32 = 2048;
/// `IMAGE_ATLAS_MAX_EDGE`：超过此边的图片走独立纹理，不进图集。
const IMG_EDGE: u32 = 1024;

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
    .expect("缺少用于图集换代测试的中西文字体");
    TextEngine::load(font_path).expect("加载字体引擎失败")
}

/// 选一个字号：单字形位图接近最小图集页上限但能放下（外框含 1 px 空隙仍 ≤ 页高），
/// 从而一页只容一个该字号字形，第二个字形必然触发换页。测试不新增页尺寸接口，
/// 只按公开的 `glyph_metrics` 反推。
fn pick_glyph_size(engine: &TextEngine, ch: char) -> f32 {
    let mut best: Option<(f32, usize, usize)> = None;
    let mut size = 1200.0f32;
    while size <= 2600.0 {
        let m = engine.glyph_metrics(ch, size);
        if m.width + 2 <= MIN_PAGE as usize && m.height + 2 <= MIN_PAGE as usize {
            // 两维都够大才能保证第二个字形放不进同一页。
            if m.width >= 1500 && m.height >= 1500 {
                return size;
            }
            if best.is_none_or(|(_, bw, bh)| m.width.min(m.height) > bw.min(bh)) {
                best = Some((size, m.width, m.height));
            }
        }
        size += 25.0;
    }
    let (s, w, h) = best.expect("字号搜索区间内找不到可放入最小图集页的字形");
    panic!("找不到能撑满最小图集页的字号：best size={s} w={w} h={h}（需 w,h ∈ [1500,2044]）");
}

/// 背景 + 若干单行文本命令，均锚在左上（保证窄损伤与所有字形实例相交）。
fn scene_glyphs(texts: &[&str], size: f32) -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.05, 0.05, 0.05, 1.0),
        rect: Rect::new(0.0, 0.0, SURF, SURF),
        corner_radius: 0.0,
    });
    for t in texts {
        s.commands.push(SceneCommand::Text {
            content: (*t).to_string(),
            rect: Rect::new(0.0, 0.0, size + 64.0, size + 64.0),
            color: Color::WHITE,
            style: TextStyle::new(size, size, FontWeight::Normal),
            align: TextAlign::Left,
            wrap: false,
            max_lines: Some(1),
            overflow: TextOverflow::Clip,
        });
    }
    s
}

/// 1024×1024 的 RGBA 内容（左闭右开渐变 + seed 区分），逐张不同以占满图集页。
fn image_rgba(seed: u8) -> Arc<[u8]> {
    let n = IMG_EDGE as usize;
    let mut v = vec![0u8; n * n * 4];
    for y in 0..n {
        for x in 0..n {
            let i = (y * n + x) * 4;
            v[i] = (x as u8).wrapping_mul(3).wrapping_add(seed);
            v[i + 1] = (y as u8).wrapping_mul(5).wrapping_add(seed);
            v[i + 2] = ((x + y) as u8).wrapping_add(seed);
            v[i + 3] = 255;
        }
    }
    Arc::from(v.into_boxed_slice())
}

/// 背景 + `count` 张 1024×1024 图集图片，缩放到小目标矩形（源尺寸决定图集占用）。
fn scene_images(count: usize) -> Scene {
    let mut s = Scene::default();
    s.commands.push(SceneCommand::FillRect {
        color: Color::new(0.05, 0.05, 0.05, 1.0),
        rect: Rect::new(0.0, 0.0, SURF, SURF),
        corner_radius: 0.0,
    });
    for k in 0..count {
        let x = k as f32 * 300.0;
        let y = k as f32 * 240.0;
        s.commands.push(SceneCommand::Image {
            rgba: image_rgba(k as u8 + 1),
            width: IMG_EDGE,
            height: IMG_EDGE,
            rect: Rect::new(x, y, 256.0, 256.0),
            tint: None,
            opacity: 1.0,
        });
    }
    s
}

/// 逐像素比对，阈值沿用既有金样：通道差 > 8 的像素比例 ≤ 0.5%。
fn assert_same(inc: &[u8], full: &[u8], label: &str) {
    assert_eq!(inc.len(), full.len(), "[{label}] 读回字节长度不一致");
    let total = inc.len() / 4;
    let mut bad = 0usize;
    let mut max_diff = 0i32;
    for i in 0..total {
        let d = (0..4)
            .map(|c| (inc[i * 4 + c] as i32 - full[i * 4 + c] as i32).abs())
            .max()
            .unwrap();
        if d > 8 {
            bad += 1;
        }
        if d > max_diff {
            max_diff = d;
        }
    }
    let ratio = bad as f64 / total as f64;
    println!(
        "[atlas_epoch] {label}: bad={bad}/{total} ({:.4}%), max_diff={max_diff}",
        ratio * 100.0
    );
    assert!(
        ratio <= 0.005,
        "[{label}] 通道差>8 的像素比例 {ratio:.4}（{bad}/{total}）超过 0.5% 阈值！\
         max_diff={max_diff}（图集换代当帧用了旧 UV）"
    );
}

fn headless_ctx() -> Option<Arc<GpuContext>> {
    match GpuContext::headless() {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("环境不支持无头 GPU（{e:?}），跳过图集换代回归测试");
            None
        }
    }
}

#[test]
fn 字形图集换代当帧不得用旧坐标() {
    let _serial = GPU_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(ctx) = headless_ctx() else { return };
    let engine = get_test_engine();
    let size = pick_glyph_size(&engine, '永');

    // 增量画布：帧 1 只画「永」填满最小页；帧 2 加「文」并触发换页。
    let mut inc = CanvasV2::offscreen(ctx.clone(), SURF, SURF, 1.0).expect("离屏画布");
    inc.render(&engine, &scene_glyphs(&["永"], size));
    let scene2 = scene_glyphs(&["永", "文"], size);
    inc.render_with_damage(&engine, &scene2, Some(Rect::new(0.0, 0.0, SURF, 32.0)));
    let inc_px = inc.read_back();

    // 参照：同一 Scene 在新建 CanvasV2 上全幅绘制。
    let mut full = CanvasV2::offscreen(ctx.clone(), SURF, SURF, 1.0).expect("离屏画布");
    full.render(&engine, &scene2);
    let full_px = full.read_back();

    assert_same(&inc_px, &full_px, "glyph-atlas-epoch");
}

#[test]
fn 图片图集换代当帧不得用旧坐标() {
    let _serial = GPU_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(ctx) = headless_ctx() else { return };
    // 图片用例不排版文字，但 CanvasV2 接口需要 TextEngine。
    let engine = get_test_engine();

    let mut inc = CanvasV2::offscreen(ctx.clone(), SURF, SURF, 1.0).expect("离屏画布");
    inc.render(&engine, &scene_images(1));
    let scene2 = scene_images(2);
    inc.render_with_damage(&engine, &scene2, Some(Rect::new(0.0, 0.0, 64.0, 64.0)));
    let inc_px = inc.read_back();

    let mut full = CanvasV2::offscreen(ctx.clone(), SURF, SURF, 1.0).expect("离屏画布");
    full.render(&engine, &scene2);
    let full_px = full.read_back();

    assert_same(&inc_px, &full_px, "image-atlas-epoch");
}
