use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use ab_glyph_rasterizer::{Rasterizer, point};
use kanesumi_core::typography::FontWeight;
use rustybuzz::ttf_parser;
use rustybuzz::{Direction, Face, UnicodeBuffer};
use unicode_bidi::ParagraphBidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// 文本越界策略。布局边界、绘制裁剪与内容取舍是三件独立的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TextOverflow {
    /// 保留完整内容，绘制阶段仍裁进文本框。
    #[default]
    Clip,
    /// 最后一行以省略号收束。
    Ellipsis,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub letter_spacing_em: f32,
    pub max_lines: Option<usize>,
    pub wrap: bool,
    pub overflow: TextOverflow,
    /// 字重：塑形与量测按它选字面（T7）。默认 Normal。
    pub weight: FontWeight,
}

impl TextLayoutOptions {
    pub fn wrapped(max_width: f32, max_height: f32, line_height: f32) -> Self {
        Self {
            max_width,
            max_height,
            line_height,
            letter_spacing_em: 0.0,
            max_lines: None,
            wrap: true,
            overflow: TextOverflow::Clip,
            weight: FontWeight::Normal,
        }
    }
}

/// 排版结果 —— 单行（逻辑内容 + 实际塑形宽度）。
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub content: String,
    pub width: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub lines: Vec<Line>,
    pub size: kanesumi_core::Size,
    pub truncated: bool,
}

/// OpenType 塑形后的单个 glyph。位置和推进量均为逻辑像素。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapedGlyph {
    pub font_id: u32,
    pub glyph_id: u16,
    pub cluster: u32,
    pub rtl: bool,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VisualCluster {
    char_start: usize,
    char_end: usize,
    x0: f32,
    x1: f32,
    rtl: bool,
}

/// 单行塑形几何。光标和选区由视觉 cluster 推导，不再按标量字符宽度近似。
#[derive(Debug, Clone, PartialEq)]
pub struct TextLineGeometry {
    pub width: f32,
    carets: Vec<f32>,
    clusters: Vec<VisualCluster>,
}

impl TextLineGeometry {
    pub fn caret_x(&self, char_index: usize) -> f32 {
        self.carets
            .get(char_index)
            .copied()
            .or_else(|| self.carets.last().copied())
            .unwrap_or(0.0)
    }

    pub fn caret_positions(&self) -> &[f32] {
        &self.carets
    }

    /// 命中最近光标；恰在中点时偏向后一个逻辑位置，与既有 TextBox 点按语义一致。
    pub fn caret_at_x(&self, x: f32) -> usize {
        self.carets
            .iter()
            .enumerate()
            .fold((0, f32::INFINITY), |best, (index, caret)| {
                let distance = (x - *caret).abs();
                if distance <= best.1 {
                    (index, distance)
                } else {
                    best
                }
            })
            .0
    }

    /// 返回逻辑字符区间在视觉行上的不相交水平片段。
    pub fn selection_spans(&self, start: usize, end: usize) -> Vec<(f32, f32)> {
        let lo = start.min(end).min(self.carets.len().saturating_sub(1));
        let hi = start.max(end).min(self.carets.len().saturating_sub(1));
        if lo == hi {
            return Vec::new();
        }

        let mut spans = Vec::new();
        for cluster in &self.clusters {
            let selected_start = lo.max(cluster.char_start);
            let selected_end = hi.min(cluster.char_end);
            if selected_start >= selected_end {
                continue;
            }
            let count = (cluster.char_end - cluster.char_start).max(1) as f32;
            let start_t = (selected_start - cluster.char_start) as f32 / count;
            let end_t = (selected_end - cluster.char_start) as f32 / count;
            let width = cluster.x1 - cluster.x0;
            let (a, b) = if cluster.rtl {
                (cluster.x1 - end_t * width, cluster.x1 - start_t * width)
            } else {
                (cluster.x0 + start_t * width, cluster.x0 + end_t * width)
            };
            spans.push((a.min(b), a.max(b)));
        }
        spans.sort_by(|a, b| a.0.total_cmp(&b.0));

        let mut merged: Vec<(f32, f32)> = Vec::new();
        for span in spans {
            if let Some(last) = merged.last_mut()
                && span.0 <= last.1 + 0.001
            {
                last.1 = last.1.max(span.1);
            } else {
                merged.push(span);
            }
        }
        merged
    }
}

/// 字体加载错误。
#[derive(Debug)]
pub enum TextLoadError {
    Io(std::io::Error),
    Parse(&'static str),
}

impl std::fmt::Display for TextLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextLoadError::Io(e) => write!(f, "读取字体失败: {e}"),
            TextLoadError::Parse(e) => write!(f, "解析字体失败: {e}"),
        }
    }
}

impl std::error::Error for TextLoadError {}

/// 字体来源：文件路径 + 可选 TTC 集合内语言标签（如 `"SC"`，按 name 表含该串选字面）。
/// Noto CJK 的 TTC 每档字重含多语言字面，须选 SC（裁定 N-40）。
#[derive(Debug, Clone)]
pub struct FontSource {
    pub path: PathBuf,
    pub collection_tag: Option<&'static str>,
}

/// 在 TTC 集合里找族名（name ID 1 / 16）含 `tag` 的字面下标；非集合或未命中返回 None。
/// 只匹配族名记录 —— 任意记录宽松匹配会在 JP 字面上误中（如版本串含 SC，2026-10-03 实测）。
fn collection_index_matching(bytes: &[u8], tag: &str) -> Option<u32> {
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    (0..count).find(|&i| {
        ttf_parser::Face::parse(bytes, i).is_ok_and(|face| {
            face.names().into_iter().any(|name| {
                (name.name_id == 1 || name.name_id == 16)
                    && name
                        .to_string()
                        .is_some_and(|text| text.to_ascii_uppercase().contains(tag))
            })
        })
    })
}

#[derive(Clone)]
struct FontFace {
    bytes: Arc<[u8]>,
    collection_index: u32,
    units_per_em: f32,
    /// OS/2 `usWeightClass`（表缺失时 ttf_parser 回落 Regular=400）。字重选字面用（T7）。
    weight: u16,
}

/// 字形度量（像素）。语义与原 fontdue `Metrics` 一致，外壳按它摆放位图：
/// 位图左上角 = (笔位 + `xmin`, 基线 − `ymin` − `height`)；`ymin` 为字形底相对基线（Y+ 向上）。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GlyphMetrics {
    pub xmin: i32,
    pub ymin: i32,
    pub width: usize,
    pub height: usize,
    pub advance_width: f32,
}

impl FontFace {
    /// 按 `FontSource` 加载：带 TTC 集合标签则选匹配字面，否则下标 0。
    fn from_source(bytes: &[u8], src: &FontSource) -> Result<Self, TextLoadError> {
        let bytes: Arc<[u8]> = Arc::from(bytes.to_vec());
        let index = src
            .collection_tag
            .and_then(|tag| collection_index_matching(&bytes, tag))
            .unwrap_or(0);
        Self::from_bytes(bytes, index)
    }

    /// 只校验、不预解析字形。
    ///
    /// 2026-09-30（KANESUMI_RUNTIME.md R1）：原先用 fontdue，加载时**预解析全部字形轮廓** ——
    /// CJK 字体（16~19 MiB 文件）每进程常驻约 330 MiB、加载约 365 ms（Arch 实测 release）。
    /// 现改为按需：塑形用 rustybuzz、光栅化用 ttf-parser 轮廓 + ab_glyph_rasterizer，
    /// 字形只在第一次被画时解码（外壳另有位图缓存），常驻内存 ≈ 字体文件本身。
    fn from_bytes(bytes: Arc<[u8]>, collection_index: u32) -> Result<Self, TextLoadError> {
        let face = ttf_parser::Face::parse(&bytes, collection_index)
            .map_err(|_| TextLoadError::Parse("字体无法解析"))?;
        let units_per_em = f32::from(face.units_per_em().max(1));
        let weight = face.weight().to_number();
        Face::from_slice(&bytes, collection_index).ok_or(TextLoadError::Parse("字体面无法塑形"))?;
        Ok(Self {
            bytes,
            collection_index,
            units_per_em,
            weight,
        })
    }

    fn shaper(&self) -> Face<'_> {
        Face::from_slice(&self.bytes, self.collection_index).expect("字体已在加载时验证")
    }

    /// 解析表目录（只读头部，微秒级）。每次调用重建，免去自引用结构。
    fn parser(&self) -> ttf_parser::Face<'_> {
        ttf_parser::Face::parse(&self.bytes, self.collection_index).expect("字体已在加载时验证")
    }

    fn has_glyph(&self, c: char) -> bool {
        self.parser().glyph_index(c).is_some()
    }

    /// 字素（所有字符）是否都在本字面内。空白与缺省可忽略字符视为覆盖。
    fn grapheme_covered(&self, grapheme: &str) -> bool {
        grapheme
            .chars()
            .all(|c| is_default_ignorable(c) || c.is_whitespace() || self.has_glyph(c))
    }

    /// (ascent, descent)，像素；descent 为负。
    fn line_metrics(&self, size: f32) -> (f32, f32) {
        let face = self.parser();
        let scale = size / self.units_per_em;
        (
            f32::from(face.ascender()) * scale,
            f32::from(face.descender()) * scale,
        )
    }

    fn metrics_for(
        &self,
        face: &ttf_parser::Face<'_>,
        gid: ttf_parser::GlyphId,
        size: f32,
    ) -> GlyphMetrics {
        let scale = size / self.units_per_em;
        let advance_width = face
            .glyph_hor_advance(gid)
            .map_or(0.0, |a| f32::from(a) * scale);
        let Some(bb) = face.glyph_bounding_box(gid) else {
            return GlyphMetrics {
                advance_width,
                ..GlyphMetrics::default()
            };
        };
        let x0 = (f32::from(bb.x_min) * scale).floor();
        let y0 = (f32::from(bb.y_min) * scale).floor();
        let x1 = (f32::from(bb.x_max) * scale).ceil();
        let y1 = (f32::from(bb.y_max) * scale).ceil();
        GlyphMetrics {
            xmin: x0 as i32,
            ymin: y0 as i32,
            width: (x1 - x0).max(0.0) as usize,
            height: (y1 - y0).max(0.0) as usize,
            advance_width,
        }
    }

    fn metrics(&self, c: char, size: f32) -> GlyphMetrics {
        let face = self.parser();
        let gid = face.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
        self.metrics_for(&face, gid, size)
    }

    /// 光栅化一个字形为 8 位覆盖率位图（行主序，宽 × 高 = metrics.width × height）。
    fn rasterize(&self, glyph_id: u16, size: f32) -> (GlyphMetrics, Vec<u8>) {
        let face = self.parser();
        let gid = ttf_parser::GlyphId(glyph_id);
        let m = self.metrics_for(&face, gid, size);
        if m.width == 0 || m.height == 0 {
            return (m, Vec::new());
        }
        let mut sink = OutlineSink {
            raster: Rasterizer::new(m.width, m.height),
            scale: size / self.units_per_em,
            origin_x: m.xmin as f32,
            // 位图顶边（像素，Y+ 向上）= ymin + height。
            top: (m.ymin + m.height as i32) as f32,
            start: point(0.0, 0.0),
            last: point(0.0, 0.0),
        };
        if face.outline_glyph(gid, &mut sink).is_none() {
            return (m, vec![0; m.width * m.height]);
        }
        let mut out = vec![0u8; m.width * m.height];
        sink.raster.for_each_pixel(|i, a| {
            out[i] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        });
        (m, out)
    }

    /// 光栅化一个字形，先在**轮廓**上沿法线外扩 `embolden_px`（物理像素）再进光栅器。
    ///
    /// 与 `rasterize` 的区别：加粗发生在轮廓进入覆盖率光栅器之前，边缘仍是干净的反走样
    /// 阶梯，不产生掩码膨胀的灰色光晕。`embolden_px <= 0` 时直接走原路径，逐像素不变。
    /// 加粗后的字形包围盒由外扩后的轮廓实算，故 `GlyphMetrics` 已含四周溢出，调用方
    /// 无需再补 pad。参 Ether `docs/research/tx1/review/REVIEW.md`（掩码膨胀不可用）。
    fn rasterize_emboldened(
        &self,
        glyph_id: u16,
        size: f32,
        embolden_px: f32,
    ) -> (GlyphMetrics, Vec<u8>) {
        if embolden_px <= 0.0 || !embolden_px.is_finite() || size <= 0.0 || !size.is_finite() {
            return self.rasterize(glyph_id, size);
        }
        let face = self.parser();
        let gid = ttf_parser::GlyphId(glyph_id);
        let scale = size / self.units_per_em;
        // 曲线细分容差 0.1 物理像素，换算到字体单位；夹取防极小字号下失控。
        let tol = (0.1 / scale).clamp(0.5, 16.0);
        let mut collector = OutlineCollector::new(tol);
        if face.outline_glyph(gid, &mut collector).is_none() {
            let m = self.metrics_for(&face, gid, size);
            return (m, vec![0; m.width * m.height]);
        }
        collector.finish_contour();
        // 物理像素 → 字体单位。
        embolden_contours(&mut collector.contours, embolden_px / scale);
        // 外扩后轮廓的实算包围盒 → 像素 metrics（floor / ceil，与 metrics_for 同规则）。
        let (mut min_x, mut min_y) = (f32::INFINITY, f32::INFINITY);
        let (mut max_x, mut max_y) = (f32::NEG_INFINITY, f32::NEG_INFINITY);
        for contour in &collector.contours {
            for p in contour {
                min_x = min_x.min(p[0]);
                min_y = min_y.min(p[1]);
                max_x = max_x.max(p[0]);
                max_y = max_y.max(p[1]);
            }
        }
        if !min_x.is_finite() || !max_x.is_finite() {
            return self.rasterize(glyph_id, size);
        }
        let x0 = (min_x * scale).floor();
        let y0 = (min_y * scale).floor();
        let x1 = (max_x * scale).ceil();
        let y1 = (max_y * scale).ceil();
        let m = GlyphMetrics {
            xmin: x0 as i32,
            ymin: y0 as i32,
            width: (x1 - x0).max(0.0) as usize,
            height: (y1 - y0).max(0.0) as usize,
            advance_width: face
                .glyph_hor_advance(gid)
                .map_or(0.0, |a| f32::from(a) * scale),
        };
        if m.width == 0 || m.height == 0 {
            return (m, Vec::new());
        }
        let mut raster = Rasterizer::new(m.width, m.height);
        let origin_x = m.xmin as f32;
        let top = (m.ymin + m.height as i32) as f32;
        let map = |x: f32, y: f32| point(x * scale - origin_x, top - y * scale);
        for contour in &collector.contours {
            if contour.len() < 3 {
                continue;
            }
            let first = map(contour[0][0], contour[0][1]);
            let mut last = first;
            for p in &contour[1..] {
                let q = map(p[0], p[1]);
                raster.draw_line(last, q);
                last = q;
            }
            if last != first {
                raster.draw_line(last, first);
            }
        }
        let mut out = vec![0u8; m.width * m.height];
        raster.for_each_pixel(|i, a| {
            out[i] = (a.clamp(0.0, 1.0) * 255.0).round() as u8;
        });
        (m, out)
    }
}

/// ttf-parser 轮廓 → 覆盖率光栅器。字体单位（Y+ 向上）→ 位图像素（Y+ 向下）。
struct OutlineSink {
    raster: Rasterizer,
    scale: f32,
    origin_x: f32,
    top: f32,
    start: ab_glyph_rasterizer::Point,
    last: ab_glyph_rasterizer::Point,
}

impl OutlineSink {
    fn map(&self, x: f32, y: f32) -> ab_glyph_rasterizer::Point {
        point(x * self.scale - self.origin_x, self.top - y * self.scale)
    }
}

impl ttf_parser::OutlineBuilder for OutlineSink {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.start = p;
        self.last = p;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.map(x, y);
        self.raster.draw_line(self.last, p);
        self.last = p;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let (c, p) = (self.map(x1, y1), self.map(x, y));
        self.raster.draw_quad(self.last, c, p);
        self.last = p;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let (c1, c2, p) = (self.map(x1, y1), self.map(x2, y2), self.map(x, y));
        self.raster.draw_cubic(self.last, c1, c2, p);
        self.last = p;
    }

    fn close(&mut self) {
        if self.last != self.start {
            self.raster.draw_line(self.last, self.start);
        }
        self.last = self.start;
    }
}

// ── 轮廓加粗（tx2 spike）───────────────────────────────────────────────────
//
// 与 `OutlineSink`（直接喂光栅器）并列：本采集器把 ttf-parser 的段收集成闭合折线，
// 以便在进光栅器前对轮廓做几何外扩。曲线按物理像素容差递归细分。

/// 采集字形轮廓为闭合折线（字体单位，Y+ 向上）。直线原样收点；二次 / 三次曲线按
/// `tol`（字体单位）细分。
struct OutlineCollector {
    contours: Vec<Vec<[f32; 2]>>,
    current: Vec<[f32; 2]>,
    start: [f32; 2],
    last: [f32; 2],
    tol: f32,
}

impl OutlineCollector {
    fn new(tol: f32) -> Self {
        Self {
            contours: Vec::new(),
            current: Vec::new(),
            start: [0.0, 0.0],
            last: [0.0, 0.0],
            tol: tol.max(1e-4),
        }
    }

    /// 收尾当前轮廓；点数不足 3（围不出面积）则丢弃。
    fn finish_contour(&mut self) {
        if self.current.len() >= 3 {
            self.contours.push(std::mem::take(&mut self.current));
        } else {
            self.current.clear();
        }
    }
}

fn mid_point(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

/// 控制点到弦的垂距是否小于容差（细分终止判据）。
fn control_is_flat(p0: [f32; 2], c: [f32; 2], p1: [f32; 2], tol: f32) -> bool {
    let dx = p1[0] - p0[0];
    let dy = p1[1] - p0[1];
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-6 {
        return true;
    }
    ((c[0] - p0[0]) * dy - (c[1] - p0[1]) * dx).abs() / len <= tol
}

/// 二次贝塞尔递归细分：`p0` 已在 `out` 中，只在叶子压入终点。
fn flatten_quad(
    out: &mut Vec<[f32; 2]>,
    p0: [f32; 2],
    c: [f32; 2],
    p1: [f32; 2],
    tol: f32,
    depth: u32,
) {
    if depth >= 16 || control_is_flat(p0, c, p1, tol) {
        out.push(p1);
        return;
    }
    let a = mid_point(p0, c);
    let b = mid_point(c, p1);
    let m = mid_point(a, b);
    flatten_quad(out, p0, a, m, tol, depth + 1);
    flatten_quad(out, m, b, p1, tol, depth + 1);
}

/// 三次贝塞尔递归细分（de Casteljau 中点拆分）。
fn flatten_cubic(
    out: &mut Vec<[f32; 2]>,
    p0: [f32; 2],
    c1: [f32; 2],
    c2: [f32; 2],
    p1: [f32; 2],
    tol: f32,
    depth: u32,
) {
    if depth >= 18 || (control_is_flat(p0, c1, p1, tol) && control_is_flat(p1, c2, p0, tol)) {
        out.push(p1);
        return;
    }
    let a = mid_point(p0, c1);
    let b = mid_point(c1, c2);
    let d = mid_point(c2, p1);
    let m0 = mid_point(a, b);
    let m1 = mid_point(b, d);
    let m = mid_point(m0, m1);
    flatten_cubic(out, p0, a, m0, m, tol, depth + 1);
    flatten_cubic(out, m, m1, d, p1, tol, depth + 1);
}

impl ttf_parser::OutlineBuilder for OutlineCollector {
    fn move_to(&mut self, x: f32, y: f32) {
        self.finish_contour();
        self.start = [x, y];
        self.last = [x, y];
        self.current.push([x, y]);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.push([x, y]);
        self.last = [x, y];
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let last = self.last;
        flatten_quad(&mut self.current, last, [x1, y1], [x, y], self.tol, 0);
        self.last = [x, y];
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let last = self.last;
        flatten_cubic(
            &mut self.current,
            last,
            [x1, y1],
            [x2, y2],
            [x, y],
            self.tol,
            0,
        );
        self.last = [x, y];
    }

    fn close(&mut self) {
        self.finish_contour();
        self.last = self.start;
    }
}

/// 闭合轮廓的有符号面积（鞋带公式，字体单位）。Y+ 向上时逆时针为正。
fn signed_area(contour: &[[f32; 2]]) -> f32 {
    let mut sum = 0.0;
    for i in 0..contour.len() {
        let a = contour[i];
        let b = contour[(i + 1) % contour.len()];
        sum += a[0] * b[1] - b[0] * a[1];
    }
    sum * 0.5
}

/// 边 `a → b` 的单位法线，`orient` 为全局环绕符号。`orient > 0`（外环逆时针）
/// 时法线取 `(dy, -dx)`，指向外环外侧；`orient < 0` 时取反向。
fn edge_normal(a: [f32; 2], b: [f32; 2], orient: f32) -> Option<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-9 {
        return None;
    }
    Some([orient * dy / len, -orient * dx / len])
}

/// 把闭合轮廓沿法线外扩 `d`（调用方坐标系单位）。外环向外、内环向内：
/// 环绕方向由面积最大的轮廓（外环）统一决定，故非零环绕结构（含「口」等内框）保持。
///
/// 顶点位移取相邻两边法线的斜接解 `d·(n1+n2)/(1+n1·n2)`；`n1·n2 → -1` 的尖角 /
/// 回头角退化为单位角平分方向，避免斜接长度发散（尖角因此被削平，已知局限）。
pub(crate) fn embolden_contours(contours: &mut [Vec<[f32; 2]>], d: f32) {
    if d <= 0.0 || !d.is_finite() {
        return;
    }
    let orient = contours
        .iter()
        .map(|c| signed_area(c))
        .reduce(|a, b| if b.abs() > a.abs() { b } else { a })
        .map(|a| if a < 0.0 { -1.0 } else { 1.0 })
        .unwrap_or(1.0);
    for contour in contours.iter_mut() {
        let n = contour.len();
        if n < 3 {
            continue;
        }
        let orig = contour.clone();
        for i in 0..n {
            let prev = orig[(i + n - 1) % n];
            let cur = orig[i];
            let next = orig[(i + 1) % n];
            let (Some(n1), Some(n2)) = (
                edge_normal(prev, cur, orient),
                edge_normal(cur, next, orient),
            ) else {
                continue;
            };
            let denom = 1.0 + n1[0] * n2[0] + n1[1] * n2[1];
            let (ox, oy) = if denom >= 0.25 {
                let k = d / denom;
                ((n1[0] + n2[0]) * k, (n1[1] + n2[1]) * k)
            } else {
                let (ax, ay) = (n1[0] + n2[0], n1[1] + n2[1]);
                let len = (ax * ax + ay * ay).sqrt();
                if len < 1e-6 {
                    (0.0, 0.0)
                } else {
                    (ax / len * d, ay / len * d)
                }
            };
            contour[i] = [cur[0] + ox, cur[1] + oy];
        }
    }
}

/// `FontWeight` → OS/2 `usWeightClass` 目标值（思源无 Semibold 字面，按 600 就重选 Bold）。
fn weight_target(weight: FontWeight) -> u16 {
    match weight {
        FontWeight::Semilight => 350,
        FontWeight::Normal => 400,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
    }
}

/// 塑形缓存键 —— 文本 + 字号 + 字距 + 字重。运行期字体栈不变（加载期一次性），
/// 故不含 `identity`；字体栈变化时 `load_with_fallbacks` 尚未被渲染消费，缓存为空。
#[derive(Hash, Eq, PartialEq, Clone)]
struct ShapeKey {
    text: String,
    size_bits: u32,
    spacing_bits: u32,
    /// 请求字重（判别值即可，选字面在实算路径）。
    weight: u8,
}

/// 排版缓存键 —— 覆盖 `layout_box` 全部输入（文本 + 字号 + 字重 + 换行选项）。
/// 静态文本（时钟 / 应用名 / 菜单项）每帧重复 UAX #14 换行 + 逐段测量是仅次于
/// 塑形与光栅化的 CPU 大头；命中直接返回缓存的 `Arc<TextLayout>`（参 egui GalleyCache）。
#[derive(Hash, Eq, PartialEq, Clone)]
struct LayoutKey {
    text: String,
    size_bits: u32,
    spacing_bits: u32,
    max_width_bits: u32,
    max_height_bits: u32,
    line_height_bits: u32,
    max_lines: Option<usize>,
    wrap: bool,
    overflow: TextOverflow,
    weight: u8,
}

/// 塑形/排版缓存容量上限。超出即整体清空 —— 纯加速缓存，命中与未命中结果等价，
/// 故清空无正确性风险，只约束长会话内存（时钟每分钟、应用名/菜单项均在产生新 key）。
/// 参 swash `FontCache` 的有界原则：缓存必须有界，否则长会话单调泄漏。
const SHAPE_CACHE_MAX: usize = 4096;
/// 容纳判定的容差（逻辑像素）。控件常以「量测宽 + 内边距」定容器、再以「容器 − 内边距」
/// 反推可用宽，f32 往返会比量测宽少 1 ULP，恰好放得下的文字被判溢出而省略
/// （2026-10-02 TopBar 全局菜单首项「F…」，Ether docs/research/topbar_t1/NOTES.md）。
/// 0.01 px 远小于任何可见差异，只吸收舍入误差，不放过真实溢出。
const FIT_EPSILON: f32 = 0.01;
const LAYOUT_CACHE_MAX: usize = 4096;

/// 文本引擎 —— OpenType shaping + Unicode BiDi + UAX #14 换行 + 字体回退。
/// Measure 与 Paint 消费同一塑形结果，禁止逐字符宽度近似。
#[derive(Clone)]
pub struct TextEngine {
    /// 字体栈共享后端：`Arc<Vec<..>>` 使 `clone()` 零拷贝（只 bump 引用计数），
    /// 消除每帧 `engine.clone()` 深拷贝整份字形表（fontdue `Font` 的 `Clone` 为
    /// `Vec<Glyph>` + `HashMap<char,..>` 深拷贝，CJK 字体数 MB）。参 egui 共享后端思路。
    fonts: Arc<Vec<FontFace>>,
    identity: u64,
    /// 塑形结果缓存（`Arc<Mutex<..>>` 使 Clone 共享同一缓存）。静态文本每帧重复
    /// BiDi 分析 + grapheme 切分 + rustybuzz 塑形是仅次于光栅化的 CPU 大头。
    shape_cache: Arc<Mutex<HashMap<ShapeKey, Arc<Vec<ShapedGlyph>>>>>,
    /// 排版结果缓存（换行 + 逐段测量）。与 `shape_cache` 互补：塑形缓存按行，
    /// 本缓存按整段 `layout_box` 调用。
    layout_cache: Arc<Mutex<HashMap<LayoutKey, Arc<TextLayout>>>>,
}

impl TextEngine {
    /// 从字体文件加载。调用方可用 `load_with_fallbacks` 提供脚本覆盖。
    pub fn load(path: impl AsRef<Path>) -> Result<Self, TextLoadError> {
        let bytes = std::fs::read(path).map_err(TextLoadError::Io)?;
        Self::from_bytes(&bytes)
    }

    /// 加载主字体和有序回退栈。坏掉或重复的回退字体不会替换主字体。
    pub fn load_with_fallbacks(
        primary: impl AsRef<Path>,
        fallbacks: impl IntoIterator<Item = PathBuf>,
    ) -> Result<Self, TextLoadError> {
        let primary = primary.as_ref();
        let mut engine = Self::load(primary)?;
        let canonical_primary = primary
            .canonicalize()
            .unwrap_or_else(|_| primary.to_path_buf());
        let mut seen = std::iter::once(canonical_primary).collect::<std::collections::HashSet<_>>();
        for path in fallbacks {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if !seen.insert(canonical) || !path.exists() {
                continue;
            }
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let bytes: Arc<[u8]> = Arc::from(bytes);
            if let Ok(face) = FontFace::from_bytes(bytes, 0) {
                Arc::make_mut(&mut engine.fonts).push(face);
            }
        }
        engine.refresh_identity();
        Ok(engine)
    }

    /// 加载字体栈（T7）：主字面 + 附加字面（字重变体 / 脚本回退）。
    /// 每项可带 TTC 集合标签（如 `"SC"`，按 name 表选字面）；重复路径与缺失文件跳过。
    pub fn load_stack(
        primary: &FontSource,
        extra: &[FontSource],
    ) -> Result<Self, TextLoadError> {
        let mut engine = Self::load_tagged(primary)?;
        let mut seen = std::collections::HashSet::new();
        seen.insert(
            primary
                .path
                .canonicalize()
                .unwrap_or_else(|_| primary.path.clone()),
        );
        for src in extra {
            let canonical = src.path.canonicalize().unwrap_or_else(|_| src.path.clone());
            if !seen.insert(canonical) || !src.path.exists() {
                continue;
            }
            let Ok(bytes) = std::fs::read(&src.path) else {
                continue;
            };
            if let Ok(face) = FontFace::from_source(&bytes, src) {
                Arc::make_mut(&mut engine.fonts).push(face);
            }
        }
        engine.refresh_identity();
        Ok(engine)
    }

    fn load_tagged(src: &FontSource) -> Result<Self, TextLoadError> {
        let bytes = std::fs::read(&src.path).map_err(TextLoadError::Io)?;
        let face = FontFace::from_source(&bytes, src)?;
        Ok(Self {
            fonts: Arc::new(vec![face]),
            identity: 0,
            shape_cache: Arc::new(Mutex::new(HashMap::new())),
            layout_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextLoadError> {
        let face = FontFace::from_bytes(Arc::from(bytes.to_vec()), 0)?;
        let mut engine = Self {
            fonts: Arc::new(vec![face]),
            identity: 0,
            shape_cache: Arc::new(Mutex::new(HashMap::new())),
            layout_cache: Arc::new(Mutex::new(HashMap::new())),
        };
        engine.refresh_identity();
        Ok(engine)
    }

    pub fn font_count(&self) -> usize {
        self.fonts.len()
    }

    /// 字体栈身份。渲染与 retained cache 必须把它纳入环境键。
    pub fn identity(&self) -> u64 {
        self.identity
    }

    fn refresh_identity(&mut self) {
        let mut hash = 0xcbf29ce484222325_u64;
        for font in self.fonts.iter() {
            for byte in font.bytes.iter() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash ^= u64::from(font.collection_index);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.identity = hash;
    }

    fn font_for_grapheme(&self, grapheme: &str) -> usize {
        self.fonts
            .iter()
            .position(|font| font.grapheme_covered(grapheme))
            .unwrap_or(0)
    }

    /// 按请求字重选字面下标（T7）：精确匹配 → 最近且不轻于 → 最近 → `fonts[0]`。
    /// 请求字重的字面缺字时，调用方再走 `font_for_grapheme` 的覆盖回退。
    fn face_for_weight(&self, weight: FontWeight) -> usize {
        if self.fonts.len() <= 1 {
            return 0;
        }
        let target = weight_target(weight);
        let weights: Vec<u16> = self.fonts.iter().map(|f| f.weight).collect();
        if let Some(i) = weights.iter().position(|&w| w == target) {
            return i;
        }
        if let Some((i, _)) = weights
            .iter()
            .enumerate()
            .filter(|(_, w)| **w >= target)
            .min_by_key(|(_, w)| **w)
        {
            return i;
        }
        weights
            .iter()
            .enumerate()
            .filter(|(_, w)| **w < target)
            .max_by_key(|(_, w)| **w)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// 塑形一行，输出视觉顺序 glyph。BiDi run 由 UAX #9 决定，run 内由 rustybuzz 处理
    /// 连字、组合附标、上下文形态与字偶距。字重取 Normal。
    pub fn shape_line(&self, text: &str, size: f32, letter_spacing_em: f32) -> Vec<ShapedGlyph> {
        self.shape_line_weighted(text, size, letter_spacing_em, FontWeight::Normal)
    }

    /// 同 `shape_line`，按请求字重选字面（T7）：字重字面缺字时回退覆盖链。
    pub fn shape_line_weighted(
        &self,
        text: &str,
        size: f32,
        letter_spacing_em: f32,
        weight: FontWeight,
    ) -> Vec<ShapedGlyph> {
        if text.is_empty() || size <= 0.0 || !size.is_finite() {
            return Vec::new();
        }
        // 塑形缓存：静态文本每帧重复 BiDi + rustybuzz 塑形是主要 CPU 开销（仅次于
        // 已缓存的光栅化）。命中直接返回克隆（ShapedGlyph 为 Copy，浅拷贝极廉）。
        let key = ShapeKey {
            text: text.to_string(),
            size_bits: size.to_bits(),
            spacing_bits: letter_spacing_em.to_bits(),
            weight: weight as u8,
        };
        if let Some(hit) = self.shape_cache.lock().expect("塑形缓存锁中毒").get(&key) {
            return hit.as_ref().clone();
        }
        let weight_face = self.face_for_weight(weight);
        let bidi = ParagraphBidiInfo::new(text, None);
        let (levels, runs) = bidi.visual_runs(0..text.len());
        let spacing = letter_spacing_em * size;
        let mut out = Vec::new();

        for run in runs {
            if run.is_empty() {
                continue;
            }
            let rtl = levels[run.start].is_rtl();
            let run_text = &text[run.clone()];
            let mut font_runs = Vec::<(usize, usize, usize)>::new();
            for (local, grapheme) in run_text.grapheme_indices(true) {
                // 优先请求字重的字面；缺字再走覆盖链（脚本回退 / 主字面）。
                let font_id = if self.fonts[weight_face].grapheme_covered(grapheme) {
                    weight_face
                } else {
                    self.font_for_grapheme(grapheme)
                };
                let start = run.start + local;
                let end = start + grapheme.len();
                match font_runs.last_mut() {
                    Some((last_font, _, last_end)) if *last_font == font_id => *last_end = end,
                    _ => font_runs.push((font_id, start, end)),
                }
            }
            if rtl {
                font_runs.reverse();
            }
            for (font_id, start, end) in font_runs {
                let face = self.fonts[font_id].shaper();
                let units = face.units_per_em().max(1) as f32;
                let scale = size / units;
                let mut buffer = UnicodeBuffer::new();
                buffer.push_str(&text[start..end]);
                buffer.set_direction(if rtl {
                    Direction::RightToLeft
                } else {
                    Direction::LeftToRight
                });
                buffer.guess_segment_properties();
                let glyphs = rustybuzz::shape(&face, &[], buffer);
                for (info, pos) in glyphs.glyph_infos().iter().zip(glyphs.glyph_positions()) {
                    let mut advance = pos.x_advance as f32 * scale;
                    if advance.abs() > f32::EPSILON {
                        advance += spacing;
                    }
                    out.push(ShapedGlyph {
                        font_id: font_id as u32,
                        glyph_id: info.glyph_id as u16,
                        cluster: start as u32 + info.cluster,
                        rtl,
                        x_advance: advance,
                        y_advance: pos.y_advance as f32 * scale,
                        x_offset: pos.x_offset as f32 * scale,
                        y_offset: pos.y_offset as f32 * scale,
                    });
                }
            }
        }
        let mut cache = self.shape_cache.lock().expect("塑形缓存锁中毒");
        if cache.len() >= SHAPE_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, Arc::new(out.clone()));
        out
    }

    /// 塑形与编辑共用的单行视觉几何。返回值以字符下标索引，兼容 TextField 契约。
    pub fn line_geometry(&self, text: &str, size: f32, letter_spacing_em: f32) -> TextLineGeometry {
        let glyphs = self.shape_line(text, size, letter_spacing_em);
        let char_boundaries: Vec<usize> = text
            .char_indices()
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .collect();
        let mut visual_groups = Vec::<(usize, bool, f32, f32)>::new();
        let mut pen = 0.0_f32;
        for glyph in glyphs {
            let next = pen + glyph.x_advance;
            let x0 = pen.min(next);
            let x1 = pen.max(next);
            if let Some((cluster, rtl, _, group_x1)) = visual_groups.last_mut()
                && *cluster == glyph.cluster as usize
                && *rtl == glyph.rtl
            {
                *group_x1 = (*group_x1).max(x1);
            } else {
                visual_groups.push((glyph.cluster as usize, glyph.rtl, x0, x1));
            }
            pen = next;
        }

        let mut logical_starts: Vec<usize> = visual_groups
            .iter()
            .map(|(cluster, _, _, _)| *cluster)
            .collect();
        logical_starts.sort_unstable();
        logical_starts.dedup();

        let mut clusters = Vec::new();
        for (byte_start, rtl, x0, x1) in visual_groups {
            let byte_end = logical_starts
                .iter()
                .copied()
                .find(|start| *start > byte_start)
                .unwrap_or(text.len());
            let char_start = char_boundaries.partition_point(|byte| *byte < byte_start);
            let char_end = char_boundaries.partition_point(|byte| *byte < byte_end);
            clusters.push(VisualCluster {
                char_start,
                char_end: char_end.max(char_start + 1).min(char_boundaries.len() - 1),
                x0,
                x1,
                rtl,
            });
        }

        let char_count = char_boundaries.len().saturating_sub(1);
        let mut carets = vec![f32::NAN; char_count + 1];
        let mut logical_clusters = clusters.clone();
        logical_clusters.sort_by_key(|cluster| cluster.char_start);
        for cluster in &logical_clusters {
            let count = (cluster.char_end - cluster.char_start).max(1);
            for offset in 0..=count {
                let t = offset as f32 / count as f32;
                let x = if cluster.rtl {
                    cluster.x1 - t * (cluster.x1 - cluster.x0)
                } else {
                    cluster.x0 + t * (cluster.x1 - cluster.x0)
                };
                carets[cluster.char_start + offset] = x;
            }
        }
        let width = pen.abs();
        let mut previous = 0.0;
        for caret in &mut carets {
            if caret.is_finite() {
                previous = *caret;
            } else {
                *caret = previous;
            }
        }

        TextLineGeometry {
            width,
            carets,
            clusters,
        }
    }

    /// 整段文本宽度（不换行，含 OpenType 塑形）。字重取 Normal。
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        self.measure_with_spacing(text, size, 0.0)
    }

    pub fn measure_with_spacing(&self, text: &str, size: f32, letter_spacing_em: f32) -> f32 {
        self.measure_with_spacing_weighted(text, size, letter_spacing_em, FontWeight::Normal)
    }

    /// 同 `measure_with_spacing`，按请求字重选字面（T7）。
    pub fn measure_with_spacing_weighted(
        &self,
        text: &str,
        size: f32,
        letter_spacing_em: f32,
        weight: FontWeight,
    ) -> f32 {
        self.shape_line_weighted(text, size, letter_spacing_em, weight)
            .iter()
            .map(|glyph| glyph.x_advance)
            .sum::<f32>()
            .abs()
    }

    pub fn line_height(&self, size: f32) -> f32 {
        let (a, d) = self.fonts[0].line_metrics(size);
        if a > 0.0 { a - d } else { size * 1.2 }
    }

    pub fn ascent(&self, size: f32) -> f32 {
        let (a, _) = self.fonts[0].line_metrics(size);
        if a > 0.0 { a } else { size * 0.8 }
    }

    /// 兼容单字符光标估算；真实段落绘制必须走 `shape_line`。
    pub fn glyph_metrics(&self, c: char, size: f32) -> GlyphMetrics {
        let mut buffer = [0; 4];
        let font = self.font_for_grapheme(c.encode_utf8(&mut buffer));
        self.fonts[font].metrics(c, size)
    }

    pub fn rasterize_glyph(
        &self,
        font_id: u32,
        glyph_id: u16,
        size: f32,
    ) -> (GlyphMetrics, Vec<u8>) {
        self.fonts
            .get(font_id as usize)
            .unwrap_or(&self.fonts[0])
            .rasterize(glyph_id, size)
    }

    /// 轮廓加粗版光栅化（tx2 spike）：先在轮廓上沿法线外扩 `embolden_px` 物理像素。
    /// `embolden_px <= 0` 时与 `rasterize_glyph` 逐像素一致（生产路径不受影响）。
    pub fn rasterize_glyph_emboldened(
        &self,
        font_id: u32,
        glyph_id: u16,
        size: f32,
        embolden_px: f32,
    ) -> (GlyphMetrics, Vec<u8>) {
        self.fonts
            .get(font_id as usize)
            .unwrap_or(&self.fonts[0])
            .rasterize_emboldened(glyph_id, size, embolden_px)
    }

    pub fn layout(&self, text: &str, size: f32, max_width: f32) -> Vec<Line> {
        self.layout_with_spacing(text, size, 0.0, max_width)
    }

    pub fn layout_with_spacing(
        &self,
        text: &str,
        size: f32,
        letter_spacing_em: f32,
        max_width: f32,
    ) -> Vec<Line> {
        self.layout_with_spacing_weighted(text, size, letter_spacing_em, max_width, FontWeight::Normal)
    }

    /// 同 `layout_with_spacing`，按请求字重选字面（T7）。
    pub fn layout_with_spacing_weighted(
        &self,
        text: &str,
        size: f32,
        letter_spacing_em: f32,
        max_width: f32,
        weight: FontWeight,
    ) -> Vec<Line> {
        if text.is_empty() || max_width <= 0.0 || max_width.is_nan() {
            return Vec::new();
        }
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut prev_end = 0usize;

        for (byte_idx, opportunity) in unicode_linebreak::linebreaks(text) {
            let segment = &text[prev_end..byte_idx];
            prev_end = byte_idx;
            if segment.is_empty() {
                continue;
            }
            let mandatory = matches!(opportunity, unicode_linebreak::BreakOpportunity::Mandatory);
            let segment = if mandatory {
                segment.trim_end_matches(['\n', '\r'])
            } else {
                segment
            };
            let candidate = format!("{current}{segment}");
            if self.measure_with_spacing_weighted(&candidate, size, letter_spacing_em, weight)
                <= max_width + FIT_EPSILON
            {
                current.push_str(segment);
            } else if current.is_empty() {
                self.hard_break_graphemes(
                    segment,
                    size,
                    letter_spacing_em,
                    max_width,
                    weight,
                    &mut lines,
                    &mut current,
                );
            } else {
                self.push_line(&mut lines, &mut current, size, letter_spacing_em, weight);
                if self.measure_with_spacing_weighted(segment, size, letter_spacing_em, weight)
                    <= max_width + FIT_EPSILON
                {
                    current.push_str(segment);
                } else {
                    self.hard_break_graphemes(
                        segment,
                        size,
                        letter_spacing_em,
                        max_width,
                        weight,
                        &mut lines,
                        &mut current,
                    );
                }
            }
            if mandatory {
                self.push_line(&mut lines, &mut current, size, letter_spacing_em, weight);
            }
        }
        if !current.is_empty() {
            self.push_line(&mut lines, &mut current, size, letter_spacing_em, weight);
        }
        lines
    }

    /// 排版一段文本 → 换行 + 逐段测量结果（`Arc` 共享，命中零数据拷贝）。
    /// 参 egui `GalleyCache`：静态文本（时钟/应用名/菜单项）每帧重复 UAX #14 换行 +
    /// 逐段测量是仅次于塑形/光栅化的 CPU 大头，此处按完整排版输入缓存整段结果。
    pub fn layout_box(&self, text: &str, size: f32, options: TextLayoutOptions) -> Arc<TextLayout> {
        let key = LayoutKey {
            text: text.to_string(),
            size_bits: size.to_bits(),
            spacing_bits: options.letter_spacing_em.to_bits(),
            max_width_bits: options.max_width.to_bits(),
            max_height_bits: options.max_height.to_bits(),
            line_height_bits: options.line_height.to_bits(),
            max_lines: options.max_lines,
            wrap: options.wrap,
            overflow: options.overflow,
            weight: options.weight as u8,
        };
        if let Some(hit) = self.layout_cache.lock().expect("排版缓存锁中毒").get(&key) {
            return hit.clone();
        }
        let layout = Arc::new(self.layout_box_uncached(text, size, options));
        let mut cache = self.layout_cache.lock().expect("排版缓存锁中毒");
        if cache.len() >= LAYOUT_CACHE_MAX {
            cache.clear();
        }
        cache.insert(key, layout.clone());
        layout
    }

    /// `layout_box` 的实算路径（缓存 miss 时调用）。
    fn layout_box_uncached(&self, text: &str, size: f32, options: TextLayoutOptions) -> TextLayout {
        let weight = options.weight;
        let line_height = options.line_height.max(0.0);
        let mut lines = if options.wrap {
            self.layout_with_spacing_weighted(
                text,
                size,
                options.letter_spacing_em,
                options.max_width,
                weight,
            )
        } else if text.is_empty() || options.max_width <= 0.0 {
            Vec::new()
        } else {
            vec![Line {
                content: text.replace(['\n', '\r'], " "),
                width: self.measure_with_spacing_weighted(
                    text,
                    size,
                    options.letter_spacing_em,
                    weight,
                ),
            }]
        };
        // 高度上限折算成行数。两条防「文字静默消失」的铁律（2026-09-22 溢出审计）：
        // 1. `wrap == false`（单行标签）**至少保留一行** —— 调用方给的框比一行还矮时，
        //    旧实现 floor(h/line_height)=0 会把唯一一行也 truncate 掉，文字整段消失，
        //    且没有任何日志；绘制仍受 paint clip 约束，故「画出来再裁」永远优于「不画」。
        // 2. `wrap == true` 且框高 > 0 时同样至少一行 —— 半行可见也好过空白。
        let height_limit = if options.max_height <= 0.0 {
            0
        } else if !options.wrap {
            1
        } else if line_height > 0.0 && options.max_height.is_finite() {
            (options.max_height / line_height).floor().max(1.0) as usize
        } else {
            usize::MAX
        };
        let line_limit = options.max_lines.unwrap_or(usize::MAX).min(height_limit);
        let mut truncated = lines.len() > line_limit;
        lines.truncate(line_limit);
        if let Some(last) = lines.last_mut()
            && options.overflow == TextOverflow::Ellipsis
            && (truncated || last.width > options.max_width + FIT_EPSILON)
        {
            *last = self.ellipsize(
                last,
                size,
                options.letter_spacing_em,
                options.max_width,
                weight,
            );
            truncated = true;
        }
        let width = lines
            .iter()
            .map(|line| line.width)
            .fold(0.0, f32::max)
            .min(options.max_width.max(0.0));
        TextLayout {
            size: kanesumi_core::Size::new(width, lines.len() as f32 * line_height),
            lines,
            truncated,
        }
    }

    fn ellipsize(
        &self,
        line: &Line,
        size: f32,
        spacing: f32,
        max_width: f32,
        weight: FontWeight,
    ) -> Line {
        let ellipsis = "…";
        if self.measure_with_spacing_weighted(ellipsis, size, spacing, weight) > max_width {
            return Line {
                content: String::new(),
                width: 0.0,
            };
        }
        let mut graphemes: Vec<&str> = line.content.graphemes(true).collect();
        loop {
            let content = format!("{}{}", graphemes.concat().trim_end(), ellipsis);
            let width = self.measure_with_spacing_weighted(&content, size, spacing, weight);
            if width <= max_width || graphemes.is_empty() {
                return Line { content, width };
            }
            graphemes.pop();
        }
    }

    #[allow(clippy::too_many_arguments)] // 纯排版参数（字素 / 字号 / 字距 / 宽度 / 字重），拆结构体反而更绕。
    fn hard_break_graphemes(
        &self,
        segment: &str,
        size: f32,
        spacing: f32,
        max_width: f32,
        weight: FontWeight,
        lines: &mut Vec<Line>,
        current: &mut String,
    ) {
        for grapheme in segment.graphemes(true) {
            let candidate = format!("{current}{grapheme}");
            if !current.is_empty()
                && self.measure_with_spacing_weighted(&candidate, size, spacing, weight)
                    > max_width + FIT_EPSILON
            {
                self.push_line(lines, current, size, spacing, weight);
            }
            current.push_str(grapheme);
        }
    }

    fn push_line(
        &self,
        lines: &mut Vec<Line>,
        current: &mut String,
        size: f32,
        spacing: f32,
        weight: FontWeight,
    ) {
        let content = current.trim_end().to_string();
        if !content.is_empty() {
            lines.push(Line {
                width: self.measure_with_spacing_weighted(&content, size, spacing, weight),
                content,
            });
        }
        current.clear();
    }
}

fn is_default_ignorable(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_font() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        [
            "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "C:/Windows/Fonts/segoeui.ttf",
        ]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.exists())
    }

    fn engine() -> Option<TextEngine> {
        TextEngine::load(find_font()?).ok()
    }

    #[test]
    fn measure_is_monotonic() {
        let Some(engine) = engine() else { return };
        assert!(engine.measure("abc", 15.0) > engine.measure("a", 15.0));
    }

    #[test]
    fn shaping_forms_ligatures_or_kerning_without_scalar_cache_identity() {
        let Some(engine) = engine() else { return };
        let glyphs = engine.shape_line("office", 20.0, 0.0);
        assert!(!glyphs.is_empty());
        assert!(glyphs.iter().all(|glyph| glyph.glyph_id > 0));
    }

    #[test]
    fn combining_mark_stays_in_one_grapheme_when_wrapping() {
        let Some(engine) = engine() else { return };
        let text = "e\u{301}e\u{301}";
        let width = engine.measure("e\u{301}", 18.0) + 0.1;
        let lines = engine.layout(text, 18.0, width);
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|line| line.content == "e\u{301}"));
    }

    #[test]
    fn bidi_text_shapes_without_reversing_source() {
        let Some(engine) = engine() else { return };
        let glyphs = engine.shape_line("Ether مرحبا", 18.0, 0.0);
        assert!(!glyphs.is_empty());
        assert!(engine.measure("Ether مرحبا", 18.0) > 0.0);
    }

    #[test]
    fn rtl_line_geometry_places_logical_end_on_visual_left() {
        let Some(engine) = engine() else { return };
        let geometry = engine.line_geometry("אבג", 18.0, 0.0);
        assert!(geometry.caret_x(3) < geometry.caret_x(0));
        let spans = geometry.selection_spans(0, 3);
        assert_eq!(spans.len(), 1);
        assert!((spans[0].1 - spans[0].0 - geometry.width).abs() < 0.01);
    }

    #[test]
    fn caret_hit_midpoint_prefers_trailing_position() {
        let Some(engine) = engine() else { return };
        let geometry = engine.line_geometry("a", 18.0, 0.0);
        let midpoint = (geometry.caret_x(0) + geometry.caret_x(1)) / 2.0;
        assert_eq!(geometry.caret_at_x(midpoint), 1);
    }

    #[test]
    fn layout_wraps_and_respects_cjk_prohibition() {
        let Some(engine) = engine() else { return };
        let lines = engine.layout("你好世界你好世界，世界你好", 15.0, 90.0);
        assert!(lines.len() >= 2);
        let prohibited = ['，', '。', '！', '？', '：', '；', '、', '）', '】'];
        assert!(lines.iter().all(|line| {
            line.content
                .chars()
                .next()
                .is_none_or(|c| !prohibited.contains(&c))
        }));
    }

    #[test]
    fn mandatory_breaks_create_lines() {
        let Some(engine) = engine() else { return };
        let lines = engine.layout("line1\nline2\nline3", 15.0, 500.0);
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn layout_box_clamps_height_and_ellipsizes() {
        let Some(engine) = engine() else { return };
        let mut options = TextLayoutOptions::wrapped(80.0, 22.0, 22.0);
        options.overflow = TextOverflow::Ellipsis;
        let layout = engine.layout_box("the quick brown fox jumps", 15.0, options);
        assert_eq!(layout.lines.len(), 1);
        assert!(layout.truncated);
        assert!(layout.lines[0].content.ends_with('…'));
        assert!(layout.lines[0].width <= 80.0);
    }

    /// 回归（2026-10-02 TopBar「F…」）：可用宽因 f32 往返比量测宽少 1 ULP 时不得省略、不得折行。
    #[test]
    fn exact_fit_survives_one_ulp_rounding_loss() {
        let Some(engine) = engine() else { return };
        for label in ["File", "Edit", "Bookmarks", "文件", "设置"] {
            let measured = engine.measure(label, 14.0);
            let short = f32::from_bits(measured.to_bits() - 1);
            let mut options = TextLayoutOptions::wrapped(short, 22.0, 22.0);
            options.wrap = false;
            options.max_lines = Some(1);
            options.overflow = TextOverflow::Ellipsis;
            let layout = engine.layout_box(label, 14.0, options);
            assert!(
                !layout.truncated,
                "{label} 被省略（量测 {measured}，可用 {short}）"
            );
            assert_eq!(layout.lines[0].content, label);

            let wrapped =
                engine.layout_box(label, 14.0, TextLayoutOptions::wrapped(short, 44.0, 22.0));
            assert_eq!(wrapped.lines.len(), 1, "{label} 不应因 1 ULP 折行");
        }
    }

    #[test]
    fn layout_box_rejects_partially_visible_line() {
        let Some(engine) = engine() else { return };
        let options = TextLayoutOptions::wrapped(80.0, 28.0, 22.0);
        let layout = engine.layout_box("one two three four", 15.0, options);
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.size.height, 22.0);
    }

    /// 回归守卫（2026-09-22 溢出审计）：单行标签框矮于一行时不得静默消失。
    /// 旧实现 `floor(h/line_height)` = 0 → truncate(0) → 一行不剩，文字凭空不见。
    #[test]
    fn single_line_label_survives_box_shorter_than_one_line() {
        let Some(engine) = engine() else { return };
        let mut options = TextLayoutOptions::wrapped(40.0, 12.0, 22.0);
        options.wrap = false;
        options.max_lines = Some(1);
        options.overflow = TextOverflow::Ellipsis;
        let layout = engine.layout_box("hello world", 15.0, options);
        assert_eq!(layout.lines.len(), 1, "框矮于一行也不得静默消失");
        assert!(layout.lines[0].content.ends_with('…'));
        assert!(layout.lines[0].width <= 40.0);
    }

    /// 同一防线的换行分支：框高 > 0 但不足一行时保留一行（绘制阶段仍受裁剪）。
    #[test]
    fn wrapped_text_in_sub_line_box_keeps_one_line() {
        let Some(engine) = engine() else { return };
        let options = TextLayoutOptions::wrapped(60.0, 10.0, 22.0);
        let layout = engine.layout_box("one two three", 15.0, options);
        assert_eq!(layout.lines.len(), 1);
    }

    /// 零高框是「显式收起」：必须什么都不画（不能与上一条混为一谈）。
    #[test]
    fn zero_height_box_draws_nothing() {
        let Some(engine) = engine() else { return };
        let options = TextLayoutOptions::wrapped(60.0, 0.0, 22.0);
        let layout = engine.layout_box("one two three", 15.0, options);
        assert!(layout.lines.is_empty());
    }

    #[test]
    fn negative_spacing_measure_matches_layout() {
        let Some(engine) = engine() else { return };
        let target = engine.measure_with_spacing("Controls", 24.0, -0.025);
        let lines = engine.layout_with_spacing("Controls", 24.0, -0.025, target + 0.01);
        assert_eq!(lines.len(), 1);
        assert!((lines[0].width - target).abs() < 0.01);
    }

    #[test]
    fn load_missing_font_errors() {
        assert!(TextEngine::load("/definitely/missing/font.ttf").is_err());
    }

    /// clone 共享字体后端：`fonts` 为 `Arc`，clone 零拷贝（不重建字形表）。
    /// 改前（`Vec<FontFace>` 深拷贝）此断言必失败 —— 回归守卫。
    #[test]
    fn clone_shares_font_backing() {
        let Some(engine) = engine() else { return };
        let cloned = engine.clone();
        assert!(Arc::ptr_eq(&engine.fonts, &cloned.fonts));
    }
/// T7 字重选择：精确 → 最近且不轻于 → 最近。用系统 Noto CJK TTC（与思源同源，
/// Regular 400 / Medium 500 各一集合，SC 字面）；机器无 Noto CJK 时跳过。
/// （reference/ 下的测试字体是 LFS 指针，瘦 checkout 无内容，不可依赖。）
#[test]
fn weight_selection_prefers_exact_then_nearest() {
    use crate::text::FontSource;
    let dir = std::path::PathBuf::from("/usr/share/fonts/noto-cjk");
    let regular = dir.join("NotoSansCJK-Regular.ttc");
    let medium = dir.join("NotoSansCJK-Medium.ttc");
    if !regular.exists() || !medium.exists() {
        return;
    }
    let engine = TextEngine::load_stack(
        &FontSource { path: regular, collection_tag: Some("SC") },
        &[FontSource { path: medium, collection_tag: Some("SC") }],
    )
    .unwrap();
    let ids = |w: FontWeight| {
        engine
            .shape_line_weighted("以太 Ether", 16.0, 0.0, w)
            .first()
            .map(|g| g.font_id)
            .unwrap()
    };
    assert_eq!(ids(FontWeight::Normal), 0, "Normal → 400 字面");
    assert_eq!(ids(FontWeight::Semilight), 0, "Semilight(350) → 最近不轻于 400");
    assert_eq!(ids(FontWeight::Medium), 1, "Medium(500) → 精确 500 字面");
    assert_eq!(ids(FontWeight::Semibold), 1, "Semibold(600) 无 700 → 最近 500");
    assert_eq!(ids(FontWeight::Bold), 1, "Bold(700) 无 700 → 最近 500");
    // 塑形缓存按字重隔离：同文本同字号不同字重 → 不同字面来源。
    assert_ne!(ids(FontWeight::Normal), ids(FontWeight::Medium));
}

/// TTC 集合按族名（name ID 1/16）选 SC 字面（N-40）。系统无 Noto CJK 时跳过。
#[test]
fn ttc_collection_selects_sc_face() {
    let ttc = std::path::PathBuf::from("/usr/share/fonts/noto-cjk/NotoSansCJK-Medium.ttc");
    if !ttc.exists() {
        return;
    }
    let bytes = std::fs::read(&ttc).unwrap();
    let idx = collection_index_matching(&bytes, "SC").expect("Noto TTC 应含 SC 族名字面");
    let face = ttf_parser::Face::parse(&bytes, idx).unwrap();
    let family = face
        .names()
        .into_iter()
        .filter(|n| n.name_id == 1 || n.name_id == 16)
        .find_map(|n| n.to_string())
        .unwrap_or_default();
    assert!(
        family.to_ascii_uppercase().contains("CJK SC"),
        "选中字面的族名应含 CJK SC，实际 {family}"
    );
    assert_eq!(face.weight().to_number(), 500, "Medium TTC 的 SC 字面应为 500");
}

/// 轮廓加粗量为 0 时必须与原光栅化逐像素一致（生产路径不受影响的守卫）。
/// 用系统 Noto CJK「口」字（含内框），同时覆盖外环 + 内环。无字体时跳过。
#[test]
fn outline_embolden_zero_matches_plain_raster() {
    let dir = std::path::PathBuf::from("/usr/share/fonts/noto-cjk");
    let regular = dir.join("NotoSansCJK-Regular.ttc");
    if !regular.exists() {
        return;
    }
    let engine = TextEngine::load_stack(
        &FontSource {
            path: regular,
            collection_tag: Some("SC"),
        },
        &[],
    )
    .unwrap();
    let glyphs = engine.shape_line("口", 30.0, 0.0);
    let Some(g) = glyphs.first() else { return };
    let plain = engine.rasterize_glyph(g.font_id, g.glyph_id, 30.0);
    let zero = engine.rasterize_glyph_emboldened(g.font_id, g.glyph_id, 30.0, 0.0);
    assert_eq!(plain.0, zero.0, "加粗 0 的 metrics 必须一致");
    assert_eq!(plain.1, zero.1, "加粗 0 的位图必须逐像素一致");
}

/// 正方形轮廓外扩 d：四边各外移 d，边长 +2d（容差）。
#[test]
fn embolden_square_grows_by_two_d() {
    let mut contours = vec![vec![
        [0.0f32, 0.0],
        [100.0, 0.0],
        [100.0, 100.0],
        [0.0, 100.0],
    ]];
    embolden_contours(&mut contours, 5.0);
    let xs = contours[0].iter().map(|p| p[0]);
    let ys = contours[0].iter().map(|p| p[1]);
    let min_x = xs.clone().fold(f32::INFINITY, f32::min);
    let max_x = xs.fold(f32::NEG_INFINITY, f32::max);
    let min_y = ys.clone().fold(f32::INFINITY, f32::min);
    let max_y = ys.fold(f32::NEG_INFINITY, f32::max);
    assert!((min_x + 5.0).abs() < 1e-3, "左边应外移 d：{min_x}");
    assert!((max_x - 105.0).abs() < 1e-3, "右边应外移 d：{max_x}");
    assert!((min_y + 5.0).abs() < 1e-3, "下边应外移 d：{min_y}");
    assert!((max_y - 105.0).abs() < 1e-3, "上边应外移 d：{max_y}");
}

/// 内环（与外环反环绕）在整体外扩时向内收：孔洞缩小而非扩大。
#[test]
fn embolden_hole_shrinks_inward() {
    let outer = vec![
        [0.0f32, 0.0],
        [300.0, 0.0],
        [300.0, 300.0],
        [0.0, 300.0],
    ]; // 逆时针外环。
    let hole = vec![
        [100.0f32, 100.0],
        [100.0, 200.0],
        [200.0, 200.0],
        [200.0, 100.0],
    ]; // 顺时针内环。
    let mut contours = vec![outer, hole];
    embolden_contours(&mut contours, 5.0);
    let h = &contours[1];
    let min_x = h.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let max_x = h.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    assert!((min_x - 105.0).abs() < 1e-3, "内环左边应内收：{min_x}");
    assert!((max_x - 195.0).abs() < 1e-3, "内环右边应内收：{max_x}");
}
}