// glyph_layout.rs —— 文本排版 → 字形放置（GPU 渲染器与 CPU 光栅器共用，平台无关）。
//
// 自 render.rs 拆出（2026-10-01）：CPU 光栅器（快照 / layer 表面）此前因依赖 wgpu 模块里的这段
// 排版代码而只能在 Linux 编译，Windows 上无法出样张。拆出后 cpu_raster / snapshot 跨平台。

use std::collections::HashMap;

use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};
use kanesumi_canvas::TextAlign;
use kanesumi_core::{Rect, TextStyle};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct GlyphKey {
    pub(crate) engine_id: u64,
    pub(crate) font_id: u32,
    pub(crate) glyph_id: u16,
    pub(crate) size_bits: u32,
}

pub(crate) fn glyph_key(engine_id: u64, font_id: u32, glyph_id: u16, size_px: f32) -> GlyphKey {
    GlyphKey {
        engine_id,
        font_id,
        glyph_id,
        size_bits: size_px.to_bits(),
    }
}

/// 一段排版后的字形放置记录（逻辑坐标）。GPU（emit_text）与 CPU（cpu_raster）共用。
pub(crate) struct PlacedGlyph {
    pub key: GlyphKey,
    /// 字形位图左上角（逻辑坐标）。
    pub x: f32,
    pub y: f32,
    /// 字形位图尺寸（逻辑坐标）。
    pub w: f32,
    pub h: f32,
}

/// 排版一段文本 → 字形放置列表（placement 与旧 emit_text 完全一致）。
/// 字形位图缓存：miss 才 rasterize 入库（静态文本零重栅格化）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_text_glyphs(
    engine: &TextEngine,
    glyph_bitmaps: &mut HashMap<GlyphKey, (kanesumi_canvas::text::GlyphMetrics, Vec<u8>)>,
    content: &str,
    rect: Rect,
    style: TextStyle,
    align: TextAlign,
    wrap: bool,
    max_lines: Option<usize>,
    overflow: kanesumi_canvas::TextOverflow,
    scale: f32,
) -> Vec<PlacedGlyph> {
    // 光栅化用物理字号（保字形清晰），放置坐标用逻辑。
    let size_phys = style.size * scale;
    let mut options =
        TextLayoutOptions::wrapped(rect.size.width, rect.size.height, style.line_height);
    options.letter_spacing_em = style.letter_spacing_em;
    options.max_lines = max_lines;
    options.wrap = wrap;
    options.overflow = overflow;
    let layout = engine.layout_box(content, style.size, options);
    let line_advance = style.line_height;
    let ascent_log = engine.ascent(size_phys) / scale;

    let mut out = Vec::new();
    let mut line_y = rect.origin.y;
    for line in &layout.lines {
        // 对齐决定行首 x（逻辑）
        let line_w = line.width;
        let x_log = match align {
            TextAlign::Left => rect.origin.x,
            TextAlign::Center => rect.origin.x + (rect.size.width - line_w) / 2.0,
            TextAlign::Right => rect.origin.x + rect.size.width - line_w,
        };
        let baseline = line_y + ascent_log;
        let mut pen = x_log;
        for glyph in engine.shape_line(&line.content, style.size, style.letter_spacing_em) {
            let key = glyph_key(engine.identity(), glyph.font_id, glyph.glyph_id, size_phys);
            let metrics = if let Some((m, _)) = glyph_bitmaps.get(&key) {
                *m
            } else {
                let (m, b) = engine.rasterize_glyph(glyph.font_id, glyph.glyph_id, size_phys);
                if m.width > 0 && m.height > 0 {
                    glyph_bitmaps.insert(key, (m, b));
                }
                m
            };
            if metrics.width == 0 || metrics.height == 0 {
                pen += glyph.x_advance;
                continue;
            }
            // 物理 metrics → 逻辑坐标（÷ scale）。ymin 为字形底相对基线偏移（Y+ 向上）。
            let inv = 1.0 / scale;
            let x0 = pen + glyph.x_offset + metrics.xmin as f32 * inv;
            let y0 = baseline
                - glyph.y_offset
                - metrics.ymin as f32 * inv
                - metrics.height as f32 * inv;
            out.push(PlacedGlyph {
                key,
                x: x0,
                y: y0,
                w: metrics.width as f32 * inv,
                h: metrics.height as f32 * inv,
            });
            pen += glyph.x_advance;
        }
        line_y += line_advance;
    }
    out
}

// ── 形状三角化（逻辑坐标 → 已转 NDC 的顶点）────────────────────────────
// 三角化本身驻 kanesumi-canvas::geometry（GPU / CPU 两后端同源，参 TOPBAR_RENDER_REFACTOR §4.3）。

