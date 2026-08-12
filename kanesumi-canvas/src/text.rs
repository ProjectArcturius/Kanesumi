use std::path::{Path, PathBuf};
use std::sync::Arc;

use fontdue::{Font, FontSettings};
use rustybuzz::{Direction, Face, UnicodeBuffer};
use unicode_bidi::ParagraphBidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// 文本越界策略。布局边界、绘制裁剪与内容取舍是三件独立的事。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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

#[derive(Clone)]
struct FontFace {
    raster: Font,
    bytes: Arc<[u8]>,
    collection_index: u32,
}

impl FontFace {
    fn from_bytes(bytes: Arc<[u8]>, collection_index: u32) -> Result<Self, TextLoadError> {
        let settings = FontSettings {
            collection_index,
            ..FontSettings::default()
        };
        let raster = Font::from_bytes(bytes.clone(), settings).map_err(TextLoadError::Parse)?;
        Face::from_slice(&bytes, collection_index).ok_or(TextLoadError::Parse("字体面无法塑形"))?;
        Ok(Self {
            raster,
            bytes,
            collection_index,
        })
    }

    fn shaper(&self) -> Face<'_> {
        Face::from_slice(&self.bytes, self.collection_index).expect("字体已在加载时验证")
    }
}

/// 文本引擎 —— OpenType shaping + Unicode BiDi + UAX #14 换行 + 字体回退。
/// Measure 与 Paint 消费同一塑形结果，禁止逐字符宽度近似。
#[derive(Clone)]
pub struct TextEngine {
    fonts: Vec<FontFace>,
    identity: u64,
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
        for path in fallbacks {
            let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
            if canonical == canonical_primary || !path.exists() {
                continue;
            }
            let Ok(bytes) = std::fs::read(path) else {
                continue;
            };
            let bytes: Arc<[u8]> = Arc::from(bytes);
            if let Ok(face) = FontFace::from_bytes(bytes, 0) {
                engine.fonts.push(face);
            }
        }
        engine.refresh_identity();
        Ok(engine)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextLoadError> {
        let face = FontFace::from_bytes(Arc::from(bytes.to_vec()), 0)?;
        let mut engine = Self {
            fonts: vec![face],
            identity: 0,
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
        for font in &self.fonts {
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
            .position(|font| {
                grapheme.chars().all(|c| {
                    is_default_ignorable(c) || c.is_whitespace() || font.raster.has_glyph(c)
                })
            })
            .unwrap_or(0)
    }

    /// 塑形一行，输出视觉顺序 glyph。BiDi run 由 UAX #9 决定，run 内由 rustybuzz 处理
    /// 连字、组合附标、上下文形态与字偶距。
    pub fn shape_line(&self, text: &str, size: f32, letter_spacing_em: f32) -> Vec<ShapedGlyph> {
        if text.is_empty() || size <= 0.0 || !size.is_finite() {
            return Vec::new();
        }
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
                let font_id = self.font_for_grapheme(grapheme);
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

    /// 整段文本宽度（不换行，含 OpenType 塑形）。
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        self.measure_with_spacing(text, size, 0.0)
    }

    pub fn measure_with_spacing(&self, text: &str, size: f32, letter_spacing_em: f32) -> f32 {
        self.shape_line(text, size, letter_spacing_em)
            .iter()
            .map(|glyph| glyph.x_advance)
            .sum::<f32>()
            .abs()
    }

    pub fn line_height(&self, size: f32) -> f32 {
        self.fonts[0]
            .raster
            .horizontal_line_metrics(size)
            .map(|m| m.ascent - m.descent)
            .unwrap_or(size * 1.2)
    }

    pub fn ascent(&self, size: f32) -> f32 {
        self.fonts[0]
            .raster
            .horizontal_line_metrics(size)
            .map(|m| m.ascent)
            .unwrap_or(size * 0.8)
    }

    /// 兼容单字符光标估算；真实段落绘制必须走 `shape_line`。
    pub fn glyph_metrics(&self, c: char, size: f32) -> fontdue::Metrics {
        let mut buffer = [0; 4];
        let font = self.font_for_grapheme(c.encode_utf8(&mut buffer));
        self.fonts[font].raster.metrics(c, size)
    }

    pub fn rasterize_glyph(
        &self,
        font_id: u32,
        glyph_id: u16,
        size: f32,
    ) -> (fontdue::Metrics, Vec<u8>) {
        self.fonts
            .get(font_id as usize)
            .unwrap_or(&self.fonts[0])
            .raster
            .rasterize_indexed(glyph_id, size)
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
            if self.measure_with_spacing(&candidate, size, letter_spacing_em) <= max_width {
                current.push_str(segment);
            } else if current.is_empty() {
                self.hard_break_graphemes(
                    segment,
                    size,
                    letter_spacing_em,
                    max_width,
                    &mut lines,
                    &mut current,
                );
            } else {
                self.push_line(&mut lines, &mut current, size, letter_spacing_em);
                if self.measure_with_spacing(segment, size, letter_spacing_em) <= max_width {
                    current.push_str(segment);
                } else {
                    self.hard_break_graphemes(
                        segment,
                        size,
                        letter_spacing_em,
                        max_width,
                        &mut lines,
                        &mut current,
                    );
                }
            }
            if mandatory {
                self.push_line(&mut lines, &mut current, size, letter_spacing_em);
            }
        }
        if !current.is_empty() {
            self.push_line(&mut lines, &mut current, size, letter_spacing_em);
        }
        lines
    }

    pub fn layout_box(&self, text: &str, size: f32, options: TextLayoutOptions) -> TextLayout {
        let line_height = options.line_height.max(0.0);
        let mut lines = if options.wrap {
            self.layout_with_spacing(text, size, options.letter_spacing_em, options.max_width)
        } else if text.is_empty() || options.max_width <= 0.0 {
            Vec::new()
        } else {
            vec![Line {
                content: text.replace(['\n', '\r'], " "),
                width: self.measure_with_spacing(text, size, options.letter_spacing_em),
            }]
        };
        let height_limit = if line_height > 0.0 && options.max_height.is_finite() {
            (options.max_height / line_height).floor().max(0.0) as usize
        } else {
            usize::MAX
        };
        let line_limit = options.max_lines.unwrap_or(usize::MAX).min(height_limit);
        let mut truncated = lines.len() > line_limit;
        lines.truncate(line_limit);
        if let Some(last) = lines.last_mut()
            && options.overflow == TextOverflow::Ellipsis
            && (truncated || last.width > options.max_width)
        {
            *last = self.ellipsize(last, size, options.letter_spacing_em, options.max_width);
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

    fn ellipsize(&self, line: &Line, size: f32, spacing: f32, max_width: f32) -> Line {
        let ellipsis = "…";
        if self.measure_with_spacing(ellipsis, size, spacing) > max_width {
            return Line {
                content: String::new(),
                width: 0.0,
            };
        }
        let mut graphemes: Vec<&str> = line.content.graphemes(true).collect();
        loop {
            let content = format!("{}{}", graphemes.concat().trim_end(), ellipsis);
            let width = self.measure_with_spacing(&content, size, spacing);
            if width <= max_width || graphemes.is_empty() {
                return Line { content, width };
            }
            graphemes.pop();
        }
    }

    fn hard_break_graphemes(
        &self,
        segment: &str,
        size: f32,
        spacing: f32,
        max_width: f32,
        lines: &mut Vec<Line>,
        current: &mut String,
    ) {
        for grapheme in segment.graphemes(true) {
            let candidate = format!("{current}{grapheme}");
            if !current.is_empty()
                && self.measure_with_spacing(&candidate, size, spacing) > max_width
            {
                self.push_line(lines, current, size, spacing);
            }
            current.push_str(grapheme);
        }
    }

    fn push_line(&self, lines: &mut Vec<Line>, current: &mut String, size: f32, spacing: f32) {
        let content = current.trim_end().to_string();
        if !content.is_empty() {
            lines.push(Line {
                width: self.measure_with_spacing(&content, size, spacing),
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

    #[test]
    fn layout_box_rejects_partially_visible_line() {
        let Some(engine) = engine() else { return };
        let options = TextLayoutOptions::wrapped(80.0, 28.0, 22.0);
        let layout = engine.layout_box("one two three four", 15.0, options);
        assert_eq!(layout.lines.len(), 1);
        assert_eq!(layout.size.height, 22.0);
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
}
