//! weight_preview —— T7 字重映射候选联系表（无头，纯 canvas，不碰生产默认）。
//!
//! 运行：`cargo run --offline -p kanesumi-canvas --example weight_preview`
//! 输出：`docs/research/t7_fontweight/candidates.png`（深浅两组 × 每块下附 3× 放大裁片）。
//!
//! 候选（映射策略 = 全局把「正文 / 标题」声明到哪一档字重）：
//! - C 基线：全部 Normal（T7 之前的观感）
//! - A：正文 Normal / 标题 Bold（样式声明即所得）
//! - B：正文 Medium / 标题 Bold（小字号加一档，向 Lumia 观感靠）

use std::path::PathBuf;

use kanesumi_canvas::text::{FontSource, TextEngine};
use kanesumi_core::typography::FontWeight;

/// 字体栈（N-40）：Noto CJK TTC 的 SC 字面，四档字重。
fn load_engine() -> TextEngine {
    let dir = "/usr/share/fonts/noto-cjk";
    let src = |w: &str| FontSource {
        path: PathBuf::from(format!("{dir}/NotoSansCJK-{w}.ttc")),
        collection_tag: Some("SC"),
    };
    TextEngine::load_stack(&src("Regular"), &[src("Light"), src("Medium"), src("Bold")])
        .expect("Noto CJK TTC 缺失：pacman -S noto-fonts-cjk 或 apt install fonts-noto-cjk")
}

struct Row {
    text: &'static str,
    size: f32,
    weight: FontWeight,
}

fn rows_for(body: FontWeight, heading: FontWeight) -> Vec<Row> {
    vec![
        Row { text: "显示器 · 亮度与色温（标题 20px）", size: 20.0, weight: heading },
        Row { text: "当光线变更时，自动变更亮度。The quick brown fox 0123 —— 正文 15px。", size: 15.0, weight: body },
        Row { text: "从已安装应用中选择，写入用户 autostart 目录。13px 副标题行。", size: 13.0, weight: body },
        Row { text: "此页面尚在建设中 11px 标注 Ether 以太", size: 11.0, weight: body },
    ]
}

/// 塑形 + 逐字形光栅化 → 整行覆盖度条带（行内基线对齐；返回 (宽, 高, 覆盖度)）。
/// 字形摆放与 glyph_layout.rs 同约定：top = −y_offset − ymin − height（Y+ 向上度量转图下行）。
fn strip(engine: &TextEngine, text: &str, size: f32, weight: FontWeight) -> (usize, usize, Vec<u8>) {
    let glyphs = engine.shape_line_weighted(text, size, 0.0, weight);
    let mut pen = 0.0_f32;
    let mut width = 0.0_f32;
    let mut bottom_max = 0.0_f32;
    let mut placed = Vec::new();
    for g in &glyphs {
        let (m, cover) = engine.rasterize_glyph(g.font_id, g.glyph_id, size);
        if m.width == 0 || m.height == 0 {
            pen += g.x_advance;
            continue;
        }
        let x0 = (pen + g.x_offset + m.xmin as f32).round();
        let top = (-g.y_offset - m.ymin as f32 - m.height as f32).round();
        bottom_max = bottom_max.max(top + m.height as f32);
        width = width.max(x0 + m.width as f32);
        placed.push((x0 as usize, top as usize, m, cover));
        pen += g.x_advance;
    }
    let w = (width.ceil() as usize).max(1);
    let h = (bottom_max.ceil() as usize).max(1);
    let mut cover = vec![0u8; w * h];
    for (x0, top, m, c) in placed {
        for (i, &a) in c.iter().enumerate() {
            if a == 0 {
                continue;
            }
            let gx = i % m.width;
            let gy = i / m.width;
            let px = x0 + gx;
            let py = top + gy;
            if px < w && py < h {
                let o = py * w + px;
                cover[o] = cover[o].max(a);
            }
        }
    }
    (w, h, cover)
}

/// 一块候选画面（多行正文 + 行距），返回 (RGBA, 各行顶部 y)。
fn render_rows(engine: &TextEngine, rows: &[Row], fg: [u8; 3], bg: [u8; 3], w: usize, h: usize) -> (Vec<u8>, Vec<usize>) {
    let mut tops = Vec::new();
    let mut buf = vec![0u8; w * h * 4];
    for px in buf.chunks_exact_mut(4) {
        px.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
    }
    let mut y = 8.0_f32;
    for row in rows {
        tops.push(y as usize);
        let (sw, _sh, cover) = strip(engine, row.text, row.size, row.weight);
        for (i, &a) in cover.iter().enumerate() {
            if a == 0 {
                continue;
            }
            let px = 10 + i % sw;
            let py = y as usize + i / sw;
            if px >= w || py >= h {
                continue;
            }
            let o = (py * w + px) * 4;
            let af = a as f32 / 255.0;
            for c in 0..3 {
                buf[o + c] = (fg[c] as f32 * af + buf[o + c] as f32 * (1.0 - af)).round() as u8;
            }
        }
        y += row.size * 1.55;
    }
    (buf, tops)
}

/// 3× 最近邻放大（观察 AA 边缘）。输入行主序 RGBA。
fn zoom3(buf: &[u8], w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (zw, zh) = (w * 3, h * 3);
    let mut out = vec![0u8; zw * zh * 4];
    for y in 0..zh {
        for x in 0..zw {
            let src = ((y / 3) * w + x / 3) * 4;
            let dst = (y * zw + x) * 4;
            out[dst..dst + 4].copy_from_slice(&buf[src..src + 4]);
        }
    }
    (out, zw, zh)
}

fn main() {
    let engine = load_engine();
    let candidates: [(&str, FontWeight, FontWeight); 3] = [
        ("C 基线：全部 Normal（T7 之前）", FontWeight::Normal, FontWeight::Normal),
        ("A：正文 Normal / 标题 Bold（声明即所得）", FontWeight::Normal, FontWeight::Bold),
        ("B：正文 Medium / 标题 Bold（小字号加一档）", FontWeight::Medium, FontWeight::Bold),
    ];
    let w = 900usize;
    let block_h = 140usize;
    let zoom_h = 26usize * 3;
    let gap = 18usize;
    let schemes: [(&str, [u8; 3], [u8; 3]); 2] = [
        ("深色", [14, 14, 14], [230, 230, 230]),
        ("浅色", [243, 243, 243], [26, 26, 26]),
    ];
    let mut blocks = Vec::new();
    for (sname, bg, fg) in schemes {
        for (label, body, heading) in candidates {
            let rows = rows_for(body, heading);
            let (buf, tops) = render_rows(&engine, &rows, fg, bg, w, block_h);
            // 裁第一条正文行（用行位真值，26px 高、宽 360）放大观察。
            let crop_y = tops[1];
            let mut crop = vec![0u8; 360 * 26 * 4];
            for r in 0..26 {
                for c in 0..360 {
                    let src = ((crop_y + r) * w + c) * 4;
                    let dst = (r * 360 + c) * 4;
                    crop[dst..dst + 4].copy_from_slice(&buf[src..src + 4]);
                }
            }
            let (zbuf, zw, zh) = zoom3(&crop, 360, 26);
            blocks.push((format!("{sname} · {label}"), buf, zbuf, zw, zh));
        }
    }
    let canvas_h = blocks.len() * (block_h + zoom_h + 34) + gap;
    let canvas_w = w.max(360 * 3 + 12);
    let mut img = vec![0u8; canvas_w * canvas_h * 4];
    for px in img.chunks_exact_mut(4) {
        px.copy_from_slice(&[40, 40, 40, 255]);
    }
    let mut y = 4usize;
    let mut block_tops = Vec::new();
    for (label, buf, zbuf, zw, zh) in &blocks {
        // 标签行（11px Normal，随块内容同色）。
        let (lw, lh, lcover) = strip(&engine, label, 11.0, FontWeight::Normal);
        let label_fg: [u8; 3] = if label.starts_with("深色") { [230, 230, 230] } else { [26, 26, 26] };
        for (i, &a) in lcover.iter().enumerate() {
            if a == 0 { continue; }
            let px = 6 + i % lw;
            let py = y + i / lw;
            if px < canvas_w && py < canvas_h {
                let o = (py * canvas_w + px) * 4;
                let af = a as f32 / 255.0;
                for c in 0..3 {
                    img[o + c] = (label_fg[c] as f32 * af + img[o + c] as f32 * (1.0 - af)).round() as u8;
                }
            }
        }
        y += lh + 4;
        block_tops.push(y);
        for r in 0..block_h {
            let src = (r * w) * 4;
            let dst = ((y + r) * canvas_w + 6) * 4;
            img[dst..dst + w * 4].copy_from_slice(&buf[src..src + w * 4]);
        }
        let zy = y + block_h + 4;
        for r in 0..*zh {
            let src = (r * zw) * 4;
            let dst = ((zy + r) * canvas_w + 6) * 4;
            img[dst..dst + zw * 4].copy_from_slice(&zbuf[src..src + zw * 4]);
        }
        y += block_h + zoom_h + 34;
    }
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        img,
        resvg::tiny_skia::IntSize::from_wh(canvas_w as u32, canvas_h as u32).unwrap(),
    )
    .unwrap();
    let out = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/research/t7_fontweight/candidates.png");
    std::fs::create_dir_all(out.parent().unwrap()).unwrap();
    pixmap.save_png(&out).unwrap();
    println!("written: {}", out.display());
    println!("layout: label_stride_sample={:?} block_tops={block_tops:?}", blocks.len());
}
