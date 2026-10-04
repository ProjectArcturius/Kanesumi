// glyph_layout.rs —— 文本排版 → 字形放置（GPU 渲染器与 CPU 光栅器共用，平台无关）。
//
// 自 render.rs 拆出（2026-10-01）：CPU 光栅器（快照 / layer 表面）此前因依赖 wgpu 模块里的这段
// 排版代码而只能在 Linux 编译，Windows 上无法出样张。拆出后 cpu_raster / snapshot 跨平台。

use std::collections::HashMap;

use kanesumi_canvas::TextAlign;
use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};
use kanesumi_core::{Rect, TextStyle};

/// 文字浓度实验旋钮（tx1 spike，2026-10-04）。
///
/// 默认值 = 现状（全关），生产路径不受影响。仅 CPU 快照路径消费；参 Ether
/// `docs/TYPE_ENGINE_PLAN.md` §一·五 与 `docs/DECISIONS_2026-10-04.md` §E-63。
/// 三个旋钮都作用于字形「进入混合之前」：
/// - `contrast` / `gamma`：覆盖率 → 覆盖率的预混合查表（`coverage_lut`）。
/// - `stem_darken_px`：覆盖率掩码的亚像素膨胀（横 / 纵最大值滤波按比例混合）—— tx1
///   的旧途径，产生灰色光晕、边缘发虚，调度者审阅判定**不可用**，仅留作反例对照。
/// - `outline_embolden_px`：轮廓沿法线外扩（tx2 新增）—— 在轮廓进光栅器前加粗，
///   边缘仍是干净反走样阶梯，是推荐的加粗途径。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextRenderTuning {
    /// 对比度增强强度（0 = 关）。以 0.5 为轴做 S 形拉伸。
    pub contrast: f32,
    /// gamma（1.0 = 关）。>1 提亮中间调，细笔画变浓。
    pub gamma: f32,
    /// 笔画加粗基准量（物理像素，2× 下的档位 0 / 0.25 / 0.5）。实际量按字号递减。
    pub stem_darken_px: f32,
    /// 轮廓外扩基准量（物理像素，2× 下的档位 0 / 0.15 / 0.30）。实际量按字号递减。
    pub outline_embolden_px: f32,
}

impl Default for TextRenderTuning {
    fn default() -> Self {
        Self {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        }
    }
}

impl TextRenderTuning {
    /// 全默认 → 现行为。混合路径据此短路，保证默认逐像素不变。
    pub const fn is_default(&self) -> bool {
        self.contrast == 0.0
            && self.gamma == 1.0
            && self.stem_darken_px == 0.0
            && self.outline_embolden_px == 0.0
    }

    /// 该物理字号下的实际加粗量（物理像素）。
    ///
    /// 参照 Adobe CFF stem darkening「小字号加得多」的曲线形状：以 30 px
    /// （逻辑 15 @2×）为基准，量随物理字号反比缩放，并夹在 [0.5, 2.0] 倍之间
    /// 防止极端字号发散。这是形状近似，非 Adobe 逐值复刻。
    pub fn stem_darken_for_size(&self, size_px: f32) -> f32 {
        darken_amount_for_size(self.stem_darken_px, size_px)
    }

    /// 该物理字号下的实际轮廓外扩量（物理像素）。曲线与 `stem_darken_for_size` 同款。
    pub fn outline_embolden_for_size(&self, size_px: f32) -> f32 {
        darken_amount_for_size(self.outline_embolden_px, size_px)
    }

    /// 前景色（sRGB 直通 rgba）→ 覆盖率映射表；全默认时返回 `None`（保持原样）。
    ///
    /// 公式（参 Skia `SkScalerContext` luminance preblend / DirectWrite 增强对比度思路）：
    /// 1. 按前景亮度选择 gamma：`gamma_eff = gamma × (1 + 0.3·(luma − 0.5))` —— 亮字
    ///    （深底）最多 ×1.15，暗字（浅底）最多 ×0.85。
    /// 2. gamma：`c₁ = c^(1/gamma_eff)`（gamma>1 提亮中间调）。
    /// 3. contrast：`c₂ = 0.5 + (c₁ − 0.5)·(1 + contrast)`（以 0.5 为轴拉伸后再夹取）。
    pub fn coverage_lut(&self, color: [f32; 4]) -> Option<[u8; 256]> {
        if self.contrast == 0.0 && self.gamma == 1.0 {
            return None;
        }
        let luma = 0.2126 * color[0] + 0.7152 * color[1] + 0.0722 * color[2];
        let gamma_eff = (self.gamma * (1.0 + 0.30 * (luma.clamp(0.0, 1.0) - 0.5))).max(0.05);
        let gain = 1.0 + self.contrast;
        let mut lut = [0u8; 256];
        for (i, slot) in lut.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            let g = c.powf(1.0 / gamma_eff);
            let s = (0.5 + (g - 0.5) * gain).clamp(0.0, 1.0);
            *slot = (s * 255.0).round() as u8;
        }
        Some(lut)
    }
}

/// 加粗 / 外扩量随物理字号递减的公共曲线（tx1 沿用）：以 30 px 为基准反比缩放，
/// 夹在 [0.5, 2.0] 倍之间防极端字号发散。
fn darken_amount_for_size(base_px: f32, size_px: f32) -> f32 {
    if base_px <= 0.0 || size_px <= 0.0 || !size_px.is_finite() {
        return 0.0;
    }
    const REFERENCE_PX: f32 = 30.0;
    let factor = (REFERENCE_PX / size_px).clamp(0.5, 2.0);
    base_px * factor
}

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
/// 生产（GPU / CPU）路径用本入口 —— 等价于「全默认旋钮」。
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
    layout_text_glyphs_tuned(
        engine,
        glyph_bitmaps,
        content,
        rect,
        style,
        align,
        wrap,
        max_lines,
        overflow,
        scale,
        TextRenderTuning::default(),
    )
}

/// 同 `layout_text_glyphs`，但按 `tuning` 在光栅化时施加笔画加粗（覆盖率膨胀）。
/// 加粗会改变字形位图与放置用的 metrics（四周补偿 pad），故旋钮变化时调用方必须
/// 清空字形与布局缓存（见 `CpuRenderer::set_text_tuning`）。
#[allow(clippy::too_many_arguments)]
pub(crate) fn layout_text_glyphs_tuned(
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
    tuning: TextRenderTuning,
) -> Vec<PlacedGlyph> {
    // 光栅化用物理字号（保字形清晰），放置坐标用逻辑。
    let size_phys = style.size * scale;
    let mut options =
        TextLayoutOptions::wrapped(rect.size.width, rect.size.height, style.line_height);
    options.letter_spacing_em = style.letter_spacing_em;
    options.max_lines = max_lines;
    options.wrap = wrap;
    options.overflow = overflow;
    // 字重随样式生效（T7）：排版与塑形同字面，字形缓存按 font_id 天然隔离。
    options.weight = style.weight;
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
        for glyph in engine.shape_line_weighted(
            &line.content,
            style.size,
            style.letter_spacing_em,
            style.weight,
        ) {
            let key = glyph_key(engine.identity(), glyph.font_id, glyph.glyph_id, size_phys);
            let metrics = if let Some((m, _)) = glyph_bitmaps.get(&key) {
                *m
            } else {
                // 轮廓外扩在光栅化之前施加（`embolden == 0` 时内部短路回原路径）；
                // 掩码膨胀（stem_darken）仍在位图上做，二者可独立开关。
                let embolden = tuning.outline_embolden_for_size(size_phys);
                let (m, b) = engine.rasterize_glyph_emboldened(
                    glyph.font_id,
                    glyph.glyph_id,
                    size_phys,
                    embolden,
                );
                let (m, b) = tune_glyph_bitmap(m, b, size_phys, tuning);
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
            let y0 =
                baseline - glyph.y_offset - metrics.ymin as f32 * inv - metrics.height as f32 * inv;
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

/// 对单个字形位图施加浓度旋钮的加粗变换；默认旋钮或空位图时原样返回。
///
/// 先在四周补 `pad = ceil(amount)` 像素零边，再做膨胀 —— 否则膨胀会在字形
/// bbox 边缘被削掉。补偿后的 metrics 同步平移 / 放大，放置坐标因此仍正确。
fn tune_glyph_bitmap(
    mut m: kanesumi_canvas::text::GlyphMetrics,
    b: Vec<u8>,
    size_px: f32,
    tuning: TextRenderTuning,
) -> (kanesumi_canvas::text::GlyphMetrics, Vec<u8>) {
    let amount = tuning.stem_darken_for_size(size_px);
    if amount <= 0.0 || m.width == 0 || m.height == 0 || b.len() < m.width * m.height {
        return (m, b);
    }
    let pad = amount.ceil() as usize;
    let (nw, nh) = (m.width + 2 * pad, m.height + 2 * pad);
    let mut padded = vec![0u8; nw * nh];
    for y in 0..m.height {
        let src = y * m.width;
        let dst = (y + pad) * nw + pad;
        padded[dst..dst + m.width].copy_from_slice(&b[src..src + m.width]);
    }
    let dilated = dilate_coverage(&padded, nw, nh, amount);
    m.xmin -= pad as i32;
    m.ymin -= pad as i32;
    m.width = nw;
    m.height = nh;
    (m, dilated)
}

/// 覆盖率掩码的亚像素膨胀：横 / 纵最大值滤波取逐像素最大（十字结构元），
/// 半径 `r = floor(amount)` 与 `r+1` 的结果按小数部分线性混合。
fn dilate_coverage(src: &[u8], w: usize, h: usize, amount: f32) -> Vec<u8> {
    if amount <= 0.0 || w == 0 || h == 0 {
        return src.to_vec();
    }
    let r = amount.floor() as usize;
    let frac = (amount - r as f32).clamp(0.0, 1.0);
    let full = cross_max(src, w, h, r);
    if frac <= 0.0 {
        return full;
    }
    let next = cross_max(src, w, h, r + 1);
    full.iter()
        .zip(next.iter())
        .map(|(&a, &b)| {
            (a as f32 + (b as f32 - a as f32) * frac)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect()
}

/// 十字（横 ∪ 纵）最大值滤波，半径 `r`。`r == 0` 时为原样。
fn cross_max(src: &[u8], w: usize, h: usize, r: usize) -> Vec<u8> {
    let mut out = src.to_vec();
    if r == 0 || w == 0 || h == 0 {
        return out;
    }
    for y in 0..h {
        let row = y * w;
        for x in 0..w {
            let mut m = src[row + x];
            let lo = x.saturating_sub(r);
            let hi = (x + r).min(w - 1);
            for xx in lo..=hi {
                m = m.max(src[row + xx]);
            }
            out[row + x] = m;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut m = out[y * w + x];
            let lo = y.saturating_sub(r);
            let hi = (y + r).min(h - 1);
            for yy in lo..=hi {
                m = m.max(src[yy * w + x]);
            }
            out[y * w + x] = m;
        }
    }
    out
}

// ── 形状三角化（逻辑坐标 → 已转 NDC 的顶点）────────────────────────────
// 三角化本身驻 kanesumi-canvas::geometry（GPU / CPU 两后端同源，参 TOPBAR_RENDER_REFACTOR §4.3）。

#[cfg(test)]
mod tests {
    use super::*;

    fn metrics(w: usize, h: usize) -> kanesumi_canvas::text::GlyphMetrics {
        kanesumi_canvas::text::GlyphMetrics {
            xmin: 0,
            ymin: 0,
            width: w,
            height: h,
            advance_width: w as f32,
        }
    }

    /// 默认旋钮：覆盖率查表为 None；加粗量为 0；位图原样返回。
    #[test]
    fn default_tuning_is_identity() {
        let t = TextRenderTuning::default();
        assert!(t.is_default());
        assert!(t.coverage_lut([1.0, 1.0, 1.0, 1.0]).is_none());
        assert_eq!(t.stem_darken_for_size(30.0), 0.0);
        let src = vec![0u8, 255, 128, 0];
        let (_, out) = tune_glyph_bitmap(metrics(2, 2), src.clone(), 30.0, t);
        assert_eq!(out, src);
    }

    /// 加粗量随字号递减，基准 30 px 处等于旋钮值。
    #[test]
    fn stem_darken_decreases_with_size() {
        let t = TextRenderTuning {
            stem_darken_px: 0.5,
            ..TextRenderTuning::default()
        };
        assert!((t.stem_darken_for_size(30.0) - 0.5).abs() < 1e-6, "基准");
        assert!(
            t.stem_darken_for_size(26.0) > t.stem_darken_for_size(56.0),
            "小字号加得多"
        );
        // 夹取上界：极小字号不超过 2× 基准。
        assert!(t.stem_darken_for_size(1.0) <= 1.0 + 1e-6);
    }

    /// 对比度 / gamma 查表：端点固定（0→0、255→255）、单调不减、中间调被抬高。
    #[test]
    fn coverage_lut_endpoints_monotonic_and_denser() {
        let t = TextRenderTuning {
            contrast: 1.0,
            gamma: 1.4,
            ..TextRenderTuning::default()
        };
        let lut = t.coverage_lut([1.0, 1.0, 1.0, 1.0]).expect("非默认应有表");
        assert_eq!(lut[0], 0);
        assert_eq!(lut[255], 255);
        assert!(
            lut.windows(2).all(|w| w[0] <= w[1]),
            "覆盖率映射必须单调不减"
        );
        assert!(lut[128] > 128, "gamma>1 / contrast>0 应抬高中间调");
    }

    /// 深底亮字与浅底暗字走不同 gamma 档（暗字档更弱）。
    #[test]
    fn coverage_lut_selects_gamma_by_foreground_luminance() {
        let t = TextRenderTuning {
            contrast: 0.0,
            gamma: 1.4,
            ..TextRenderTuning::default()
        };
        let dark_fg = t.coverage_lut([0.0, 0.0, 0.0, 1.0]).unwrap();
        let light_fg = t.coverage_lut([1.0, 1.0, 1.0, 1.0]).unwrap();
        assert!(light_fg[128] > dark_fg[128], "亮字档更浓");
    }

    /// 单像素经 1 px 膨胀后覆盖到十字邻域；默认旋钮不改变。
    #[test]
    fn dilation_grows_single_pixel() {
        // 3×3 中心 1 像素。
        let src = vec![0u8, 0, 0, 0, 255, 0, 0, 0, 0];
        let grown = dilate_coverage(&src, 3, 3, 1.0);
        assert_eq!(grown[0], 0, "对角不扩张（十字结构元）");
        assert!(grown[1] > 0 && grown[3] > 0 && grown[5] > 0 && grown[7] > 0);
        assert_eq!(grown[4], 255);
        let same = dilate_coverage(&src, 3, 3, 0.0);
        assert_eq!(same, src);
    }

    /// 亚像素量在整数膨胀之间线性过渡（半径 0 → 半径 1 的中点）。
    #[test]
    fn dilation_subpixel_blends_between_radii() {
        let mut src = vec![0u8; 25];
        src[12] = 255; // 5×5 中心。
        let half = dilate_coverage(&src, 5, 5, 0.5);
        let full = dilate_coverage(&src, 5, 5, 1.0);
        let near = 3 + 2 * 5; // 距中心 1 的右邻。
        assert_eq!(half[near], 128, "0.5 px = 半径 0 与半径 1 各半");
        assert_eq!(full[near], 255, "1.0 px = 半径 1 全量");
        let far = 4 + 2 * 5; // 距中心 2：半径 1 够不到。
        assert_eq!(half[far], 0);
        assert_eq!(full[far], 0);
    }
}
