use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ab_glyph_rasterizer::{Rasterizer, point};
use kanesumi_core::typography::FontWeight;
use rustybuzz::ttf_parser;
use rustybuzz::{Direction, Face, UnicodeBuffer};
use unicode_bidi::ParagraphBidiInfo;
use unicode_segmentation::UnicodeSegmentation;

/// 无字体时的编辑簇边界（字符下标），共用画布既有的 Unicode 字素切分。
/// 首项为 0，末项为字符总数；参 Ether docs/research/k_text_shaping/DESIGN.md §簇映射。
pub fn grapheme_char_boundaries(text: &str) -> Vec<usize> {
    let mut boundaries = vec![0];
    let mut chars = 0;
    for grapheme in text.graphemes(true) {
        chars += grapheme.chars().count();
        boundaries.push(chars);
    }
    boundaries
}

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

/// 排版结果 —— 单行（逻辑内容 + 实际塑形宽度 + 簇边界与光标坐标）。
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub content: String,
    pub width: f32,
    pub cluster_offsets: Vec<usize>,
    pub cluster_carets: Vec<f32>,
}

impl Line {
    pub fn new(content: impl Into<String>, width: f32) -> Self {
        let content = content.into();
        let len = content.len();
        Self {
            content,
            width,
            cluster_offsets: vec![0, len],
            cluster_carets: vec![0.0, width],
        }
    }

    /// 簇边界字节偏移列表。首项恒为 0，末项恒为 `content.len()`。
    pub fn cluster_boundaries(&self) -> &[usize] {
        &self.cluster_offsets
    }

    /// 字节偏移处的光标 x 坐标。若未落在簇边界上，夹紧到最近的簇边界。
    pub fn caret_x(&self, byte_offset: usize) -> f32 {
        if self.cluster_offsets.is_empty() || self.cluster_carets.is_empty() {
            return 0.0;
        }
        let byte = byte_offset.min(self.content.len());
        let idx = match self.cluster_offsets.binary_search(&byte) {
            Ok(i) => i,
            Err(i) => {
                if i == 0 {
                    0
                } else if i >= self.cluster_offsets.len() {
                    self.cluster_offsets.len() - 1
                } else {
                    let prev = self.cluster_offsets[i - 1];
                    let next = self.cluster_offsets[i];
                    if byte - prev <= next - byte {
                        i - 1
                    } else {
                        i
                    }
                }
            }
        };
        self.cluster_carets.get(idx).copied().unwrap_or(0.0)
    }

    /// 命中测试：根据 x 坐标返回最近的簇边界字节偏移。恰在中点时偏向后一个。
    pub fn hit_test(&self, x: f32) -> usize {
        if self.cluster_offsets.is_empty() || self.cluster_carets.is_empty() {
            return 0;
        }
        let mut best_idx = 0;
        let mut best_dist = f32::INFINITY;
        for (i, &caret) in self.cluster_carets.iter().enumerate() {
            let dist = (x - caret).abs();
            if dist <= best_dist {
                best_dist = dist;
                best_idx = i;
            }
        }
        self.cluster_offsets[best_idx]
    }

    /// 选区高亮水平片段：给定起止字节偏移，返回视觉行上的水平区间片段列表。
    pub fn selection_spans(&self, start_byte: usize, end_byte: usize) -> Vec<(f32, f32)> {
        let lo = start_byte.min(end_byte).min(self.content.len());
        let hi = start_byte.max(end_byte).min(self.content.len());
        if lo == hi || self.cluster_offsets.len() < 2 {
            return Vec::new();
        }
        let mut spans = Vec::new();
        for i in 0..self.cluster_offsets.len() - 1 {
            let b0 = self.cluster_offsets[i];
            let b1 = self.cluster_offsets[i + 1];
            if b0 >= hi || b1 <= lo {
                continue;
            }
            let c0 = self.cluster_carets[i];
            let c1 = self.cluster_carets[i + 1];
            spans.push((c0.min(c1), c0.max(c1)));
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

#[derive(Debug, Clone, PartialEq)]
pub struct TextLayout {
    pub lines: Vec<Line>,
    pub size: kanesumi_core::Size,
    pub truncated: bool,
}

impl TextLayout {
    /// 簇边界字节偏移列表（单行/首行）。
    pub fn cluster_boundaries(&self) -> &[usize] {
        self.lines.first().map(|l| l.cluster_boundaries()).unwrap_or(&[])
    }

    /// 根据 x 坐标返回最近的簇边界字节偏移（单行/首行）。
    pub fn hit_test(&self, x: f32) -> usize {
        self.lines.first().map(|l| l.hit_test(x)).unwrap_or(0)
    }

    /// 字节偏移处的光标 x 坐标（单行/首行）。
    pub fn caret_x(&self, byte_offset: usize) -> f32 {
        self.lines.first().map(|l| l.caret_x(byte_offset)).unwrap_or(0.0)
    }

    /// 选区水平片段（单行/首行）。
    pub fn selection_spans(&self, start_byte: usize, end_byte: usize) -> Vec<(f32, f32)> {
        self.lines.first().map(|l| l.selection_spans(start_byte, end_byte)).unwrap_or_default()
    }
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

    /// 簇边界的字符下标列表。首项恒为 0，末项为字符总数。
    pub fn cluster_boundaries(&self) -> Vec<usize> {
        let mut boundaries = Vec::new();
        boundaries.push(0);
        let mut logical_clusters = self.clusters.clone();
        logical_clusters.sort_by_key(|c| c.char_start);
        for c in logical_clusters {
            if boundaries.last() != Some(&c.char_start) {
                boundaries.push(c.char_start);
            }
            if boundaries.last() != Some(&c.char_end) {
                boundaries.push(c.char_end);
            }
        }
        boundaries.dedup();
        boundaries
    }

    /// 命中最近光标；落在簇内时对齐到最近的簇边界，恰在中点时偏向后一个逻辑位置。
    pub fn caret_at_x(&self, x: f32) -> usize {
        let boundaries = self.cluster_boundaries();
        if boundaries.is_empty() {
            return 0;
        }
        boundaries
            .iter()
            .copied()
            .fold((0, f32::INFINITY), |best, index| {
                let caret = self.caret_x(index);
                let distance = (x - caret).abs();
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
pub(crate) fn collection_index_matching(bytes: &[u8], tag: &str) -> Option<u32> {
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    (0..count).find(|&i| {
        count_face_parse();
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

/// 字体字节句柄：堆内（`fs::read`）或外部注入的只读映射（harness 侧 mmap，
/// 参 harness `font_mmap`）。解析器按 `[u8]` 借用、无自引用，全程纯安全代码。
pub type FontBytes = Arc<dyn AsRef<[u8]> + Send + Sync>;

/// 码点覆盖表（w1029 text-coverage-cache §2-2）：cmap 全集的压缩闭区间表。
/// `has_glyph` 查表替代逐字 `Face::parse`——塑形覆盖查询曾对每字素整体走一遍
/// 「解析表目录 → 查 cmap」，代价按字数线性增长（A2 设计稿范围外发现 1 / §4.1 A3.3）。
/// 查表与 `glyph_index` 的覆盖判定同源，以旧路径的实际返回值过滤候选，
/// 由真机字体的3000码点回归测试逐点锁定（`coverage_matches_glyph_index_on_real_stack`）。
#[derive(Clone)]
pub(crate) struct CoverageTable {
    /// 升序闭区间 `(start, end)`，两两不相邻不相交。
    ranges: Arc<[(u32, u32)]>,
    /// 建表耗时（µs，cmap 遍历 + 区间合并），诊断口径。
    build_us: u128,
}

impl CoverageTable {
    /// 从已解析面构建。A3.1 起**不再**在载入期无条件调用：索引条目按
    /// `content_hash + collection_index` 命中时载入不建表，只有首次非索引用途
    ///（索引未命中 / 诊断统计）才经 [`FontFace::coverage_table`] 建一次；
    /// 字重实例条目经 `faces_from_source` 克隆共享同一份 `OnceLock`（`wght` 轴不影响 cmap）。
    ///
    /// 语义必须与 `Face::glyph_index`（旧 `has_glyph` 路径）严格同源：该函数按
    /// 「首个 Unicode 子表映射成功」判定，且格式层有段枚举之外的有效性过滤
    ///（如 `idRangeOffset=0xFFFF` 哨兵、gid 溢出 `u16` 时返 `None`）。故先按
    /// 子表 `codepoints()` 枚举候选，再逐点用 `Face::glyph_index` 复检——真机
    /// Noto CJK 上枚举 ~2.5 万候选、复检剔除个别无效码点（实测 U+0000），
    /// 覆盖面与旧路径逐点一致（回归测试锁定）。
    pub(crate) fn from_face(face: &ttf_parser::Face<'_>) -> Self {
        let t = std::time::Instant::now();
        let mut cps: Vec<u32> = Vec::new();
        if let Some(cmap) = face.tables().cmap {
            for sub in cmap.subtables {
                // ttf-parser 0.24 的 codepoints 是回调式枚举（位图 / 段表展开）。
                sub.codepoints(|cp| cps.push(cp));
            }
        }
        cps.sort_unstable();
        cps.dedup();
        // 复检按旧路径确切语义；超出 Unicode 标量区的候选（cmap 可含非 Unicode
        // 码点）进不了 char，一律不覆盖。
        cps.retain(|&cp| {
            char::from_u32(cp).is_some_and(|c| face.glyph_index(c).is_some())
        });
        let mut ranges: Vec<(u32, u32)> = Vec::with_capacity(cps.len() / 4 + 1);
        for &cp in &cps {
            match ranges.last_mut() {
                // cp=0 时 cp-1 会下溢，先判 cp>0。
                Some((_, end)) if cp > 0 && *end == cp - 1 => *end = cp,
                _ => ranges.push((cp, cp)),
            }
        }
        count_coverage_build();
        Self {
            ranges: ranges.into(),
            build_us: t.elapsed().as_micros(),
        }
    }

    /// 查表：码点落在任一区间内即覆盖。无 cmap 的字面 ranges 为空 → 全码点不覆盖，
    /// 与 `glyph_index` 对无 cmap 字面恒 `None` 一致。
    pub(crate) fn has(&self, c: char) -> bool {
        let cp = u32::from(c);
        let i = self.ranges.partition_point(|&(start, _)| start <= cp);
        i > 0 && self.ranges[i - 1].1 >= cp
    }

    /// 只读借用区间表（A3.0 `font_index` 写侧序列化用；不改建表与查询行为）。
    /// A3.0 阶段消费者只有 `font_index` 测试与（未来的）A3.1 构建器，故标 dead_code 豁免。
    #[allow(dead_code)]
    pub(crate) fn ranges(&self) -> &[(u32, u32)] {
        &self.ranges
    }

    /// 表内存成本（区间表本体字节数，不含 Arc 头）。
    fn memory_bytes(&self) -> usize {
        self.ranges.len() * std::mem::size_of::<(u32, u32)>()
    }

    /// 表实际覆盖的码点数（区间长度之和）。报告要写「真实覆盖码点数」，
    /// 光有区间条数看不出覆盖面——2032 个区间可能只覆盖 2 千点，也可能覆盖 4 万点。
    fn codepoints(&self) -> usize {
        self.ranges
            .iter()
            .map(|&(start, end)| (end - start) as usize + 1)
            .sum()
    }
}

/// 覆盖表统计（诊断口径，w1029 text-coverage-cache §3 报告用）。
#[derive(Debug, Clone, Copy)]
pub struct CoverageStat {
    /// 区间条数。
    pub ranges: usize,
    /// 实际覆盖码点数（各闭区间长度之和）。
    pub codepoints: usize,
    /// 表内存（区间表本体字节数）。
    pub memory_bytes: usize,
    /// 建表耗时（µs）。
    pub build_us: u128,
}

#[derive(Clone)]
struct FontFace {
    bytes: FontBytes,
    collection_index: u32,
    units_per_em: f32,
    /// 该条目的 OS/2 `usWeightClass`（表缺失时 ttf_parser 回落 Regular=400）。
    /// 可变字体实例条目存**请求字重目标值**（如 800），静态字面存真实 usWeightClass。
    weight: u16,
    /// 可变字体的 `wght` 轴坐标；`None` = 静态字面 / 不设轴（T7 原路径）。
    /// 塑形与光栅都据此把同一实例化面喂给 rustybuzz / ttf-parser，二者必须同值。
    wght: Option<f32>,
    /// 码点覆盖表（cmap 压缩区间）。A3.1 惰性：索引命中时不建，首次非索引用途
    ///（索引未命中 / 诊断统计）才建；同源多字重实例共享同一份 `Arc<OnceLock<..>>`。
    coverage: Arc<std::sync::OnceLock<Arc<CoverageTable>>>,
    /// A3.3/A3.1：开关开时解析的索引覆盖区段；`None` = 无索引 / 无效 / 未命中 → 回落内存表。
    /// 载入期命中即由 [`TextEngine::refresh_identity`] 预置，运行期 `has_glyph` 不再复算
    /// `content_hash`。
    index_cov: std::sync::OnceLock<Option<crate::font_index::CoverageRanges<'static>>>,
    /// 本字面整份字节的 FNV-1a（与索引 `content_hash` 同算法）。A3.1 起在
    /// [`TextEngine::refresh_identity`] 的身份遍历中顺带算出（同一趟内存访问），
    /// 供载入期按 `content_hash + collection_index` 判命中；字重实例共享同一份（同字节）。
    content_hash: Arc<std::sync::OnceLock<u64>>,
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
    /// 加载一个来源为若干字重条目（§112）：可变字体（带 `wght` 轴）按 [`WEIGHT_INSTANCES`]
    /// 实例化成多条，静态字面仍为一条。多条共享同一份字节句柄，常驻内存即字体文件本身
    ///（堆内或只读映射，不再复制）。
    fn faces_from_source(bytes: FontBytes, src: &FontSource) -> Result<Vec<Self>, TextLoadError> {
        let index = src
            .collection_tag
            .and_then(|tag| collection_index_matching(bytes.as_ref().as_ref(), tag))
            .unwrap_or(0);
        let base = Self::from_bytes(bytes, index)?;
        let Some((min, max)) = base.wght_axis_range() else {
            return Ok(vec![base]);
        };
        Ok(WEIGHT_INSTANCES
            .iter()
            .map(|w| {
                let target = weight_target(*w);
                let mut face = base.clone();
                face.weight = target;
                // 轴坐标夹进字体实际范围，但请求目标值留在 `weight` 供精确选面。
                face.wght = Some((target as f32).clamp(min, max));
                face
            })
            .collect())
    }

    /// `wght` 轴范围（可变字体）。非可变字体 / 无该轴返回 `None`。
    fn wght_axis_range(&self) -> Option<(f32, f32)> {
        let face = self.parser();
        face.variation_axes().into_iter().find_map(|axis| {
            (axis.tag == ttf_parser::Tag::from_bytes(b"wght"))
                .then_some((axis.min_value, axis.max_value))
        })
    }

    /// 只校验、不预解析字形。
    ///
    /// 2026-09-30（KANESUMI_RUNTIME.md R1）：原先用 fontdue，加载时**预解析全部字形轮廓** ——
    /// CJK 字体（16~19 MiB 文件）每进程常驻约 330 MiB、加载约 365 ms（Arch 实测 release）。
    /// 现改为按需：塑形用 rustybuzz、光栅化用 ttf-parser 轮廓 + ab_glyph_rasterizer，
    /// 字形只在第一次被画时解码（外壳另有位图缓存），常驻内存 ≈ 字体文件本身。
    fn from_bytes(bytes: FontBytes, collection_index: u32) -> Result<Self, TextLoadError> {
        // A3.1：索引存在时先不建覆盖表 —— 命中判定要 `content_hash`，而身份遍历
        //（`refresh_identity`）本来就要读完整份字节；在那一趟里顺带算哈希后再决策，
        // 避免为判命中再走一整趟。无索引 / 开关关 → `pending=false`，旧路径载入即建。
        let pending = font_index_switch() && shared_index().is_some();
        Self::load_validated(bytes, collection_index, pending)
    }

    /// 测试注入版：`idx` 可为非 `'static` 夹具（进程级开关不可按用例隔离）。
    /// 命中谓词与载入期相同（`content_hash + collection_index`），但此处直接算完
    ///（测试不经 [`TextEngine::refresh_identity`] 的身份遍历）；区段不缓存，
    /// 由 [`Self::has_glyph_with`] 传入。
    #[cfg(test)]
    fn from_bytes_with_index(
        bytes: FontBytes,
        collection_index: u32,
        idx: Option<&crate::font_index::OwnedIndex>,
    ) -> Result<Self, TextLoadError> {
        let hit = idx.is_some_and(|idx| {
            index_entry_coverage_in(idx, bytes.as_ref().as_ref(), collection_index).is_some()
        });
        Self::load_validated(bytes, collection_index, hit)
    }

    /// 校验字面并装配。`skip_coverage = true` 时先不建覆盖表（A3.1 载入期命中待判），
    /// 由 [`TextEngine::refresh_identity`] 在一趟身份遍历后决策；否则旧路径载入即建
    ///（w1029 §2-2）。
    fn load_validated(
        bytes: FontBytes,
        collection_index: u32,
        skip_coverage: bool,
    ) -> Result<Self, TextLoadError> {
        let data: &[u8] = bytes.as_ref().as_ref();
        count_face_parse();
        let face = ttf_parser::Face::parse(data, collection_index)
            .map_err(|_| TextLoadError::Parse("字体无法解析"))?;
        let units_per_em = f32::from(face.units_per_em().max(1));
        let weight = face.weight().to_number();
        Face::from_slice(data, collection_index).ok_or(TextLoadError::Parse("字体面无法塑形"))?;
        let coverage = std::sync::OnceLock::new();
        if !skip_coverage {
            let _ = coverage.set(Arc::new(CoverageTable::from_face(&face)));
        }
        Ok(Self {
            bytes,
            collection_index,
            units_per_em,
            weight,
            wght: None,
            coverage: Arc::new(coverage),
            index_cov: std::sync::OnceLock::new(),
            content_hash: Arc::new(std::sync::OnceLock::new()),
        })
    }

    /// A3.1 载入期决策（`refresh_identity` 算完 `content_hash` 后调用）：命中的字面
    /// 缓存索引区段、覆盖表保持未建；未命中则补建表，回到旧路径。
    fn resolve_index_coverage(&self, idx: &'static crate::font_index::OwnedIndex) {
        if self.coverage.get().is_some() || self.index_cov.get().is_some() {
            return;
        }
        let Some(hash) = self.content_hash.get().copied() else {
            return;
        };
        match index_entry_coverage_by_hash(idx, hash, self.collection_index) {
            Some(ranges) => {
                let _ = self.index_cov.set(Some(ranges));
            }
            None => {
                // 未命中：补建内存表回旧路径，并把「无索引条目」一并缓存——
                // 否则后续回落查询（`has_glyph` → `index_entry_coverage`）会对整份
                // 字节再付一次 FNV，与载入期已判的事实重复。
                let _ = self.coverage_table();
                let _ = self.index_cov.set(None);
            }
        }
    }

    /// 覆盖表：首次非索引用途才建（A3.1）。解析面在载入时已校验，这里只补一次
    /// `Face::parse`（进 `face_parse_calls` 诊断口径）。
    fn coverage_table(&self) -> &Arc<CoverageTable> {
        self.coverage.get_or_init(|| {
            count_face_parse();
            let face = ttf_parser::Face::parse(self.data(), self.collection_index)
                .expect("字体已在加载时验证");
            Arc::new(CoverageTable::from_face(&face))
        })
    }

    /// 字体字节视图。句柄统一走这里借 `[u8]`（堆内 / 映射两种来源同一用法）。
    fn data(&self) -> &[u8] {
        self.bytes.as_ref().as_ref()
    }

    /// 塑形面：可变字体实例条目先设 `wght` 轴，再交给 rustybuzz。
    fn shaper(&self) -> Face<'_> {
        let mut face = Face::from_slice(self.data(), self.collection_index)
            .expect("字体已在加载时验证");
        if let Some(value) = self.wght {
            face.set_variations(&[rustybuzz::Variation {
                tag: ttf_parser::Tag::from_bytes(b"wght"),
                value,
            }]);
        }
        face
    }

    /// 解析面（度量 / 轮廓）：与 [`Self::shaper`] 施加同一 `wght` 轴坐标。
    /// 每次调用重建，免去自引用结构。重建次数进诊断计数（w1029 text-coverage-cache
    /// §2-1）：塑形覆盖查询每字素走一次本函数，是该计数的主体。
    fn parser(&self) -> ttf_parser::Face<'_> {
        count_parse_rebuild();
        count_face_parse();
        let mut face = ttf_parser::Face::parse(self.data(), self.collection_index)
            .expect("字体已在加载时验证");
        if let Some(value) = self.wght {
            face.set_variation(ttf_parser::Tag::from_bytes(b"wght"), value);
        }
        face
    }

    /// 是否含该码点字形：开关开且本字面对上索引条目 → 查索引覆盖区段
    /// （`CoverageRanges::has`）；否则查内存覆盖表（w1029 §2-2）。A3.1 起该表可能
    /// 尚未建（索引命中路径），此处才惰性建表。可变字体实例与基条共享同一表 / 同一条目
    ///（`wght` 轴不影响 cmap）。
    fn has_glyph(&self, c: char) -> bool {
        let idx = if font_index_switch() { shared_index() } else { None };
        self.has_glyph_with(self.index_entry_coverage(idx), c)
    }

    /// 查询纯核：`Some(区段)` → 索引；`None` → 内存表（首次调用建表）。测试注入
    /// 非 `'static` 夹具区段，避免改进程级开关。
    fn has_glyph_with(
        &self,
        index_cov: Option<crate::font_index::CoverageRanges<'_>>,
        c: char,
    ) -> bool {
        match index_cov {
            Some(r) => r.has(c),
            None => self.coverage_table().has(c),
        }
    }

    /// 本字面在索引中的覆盖区段（`content_hash` + `collection_index` 匹配，OnceLock 缓存）。
    /// A3.1 载入期已定：命中预置区段、未命中连「无条目」一并缓存（见
    /// [`Self::resolve_index_coverage`]）；仅开关关 / 无索引的旧路径字面，首次查询
    /// 才对整份字节做一次 FNV（与 E04 全栈身份哈希同量级）。
    fn index_entry_coverage(
        &self,
        idx: Option<&'static crate::font_index::OwnedIndex>,
    ) -> Option<crate::font_index::CoverageRanges<'static>> {
        *self.index_cov.get_or_init(|| {
            index_entry_coverage_in(idx?, self.data(), self.collection_index)
        })
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

/// `FontWeight` → OS/2 `usWeightClass` 目标值。参 §112 界面三档：
/// Light 300 / Normal 400 / ExtraBold 800；其余为历史档（第三方保留）。
fn weight_target(weight: FontWeight) -> u16 {
    match weight {
        FontWeight::Light => 300,
        FontWeight::Normal => 400,
        FontWeight::Semilight => 350,
        FontWeight::Medium => 500,
        FontWeight::Semibold => 600,
        FontWeight::Bold => 700,
        FontWeight::ExtraBold => 800,
    }
}

/// 可变字体按 `wght` 轴实例化的字重档（覆盖枚举全部取值，任何请求都有精确条目）。
const WEIGHT_INSTANCES: [FontWeight; 7] = [
    FontWeight::Light,
    FontWeight::Normal,
    FontWeight::Semilight,
    FontWeight::Medium,
    FontWeight::Semibold,
    FontWeight::Bold,
    FontWeight::ExtraBold,
];

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
            let handle: FontBytes = Arc::new(bytes);
            if let Ok(face) = FontFace::from_bytes(handle, 0) {
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
        Self::load_stack_with(primary, extra, &|path| {
            std::fs::read(path).map(|bytes| -> FontBytes { Arc::new(bytes) })
        })
    }

    /// 带读取器的字体栈加载（runtime A1）：`read` 决定每个字体文件的字节来源
    /// （堆内 `fs::read` / 只读映射），字形与位图输出与 [`Self::load_stack`] 完全
    /// 一致（等价性由单测锁定）。主字面读取失败仍报错，附加字面失败照旧跳过。
    pub fn load_stack_with(
        primary: &FontSource,
        extra: &[FontSource],
        read: &dyn Fn(&Path) -> std::io::Result<FontBytes>,
    ) -> Result<Self, TextLoadError> {
        let mut engine = Self::load_tagged_with(primary, read)?;
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
            let Ok(handle) = read(&src.path) else {
                continue;
            };
            if let Ok(faces) = FontFace::faces_from_source(handle, src) {
                Arc::make_mut(&mut engine.fonts).extend(faces);
            }
        }
        engine.refresh_identity();
        Ok(engine)
    }

    fn load_tagged_with(
        src: &FontSource,
        read: &dyn Fn(&Path) -> std::io::Result<FontBytes>,
    ) -> Result<Self, TextLoadError> {
        let handle = read(&src.path).map_err(TextLoadError::Io)?;
        let faces = FontFace::faces_from_source(handle, src)?;
        Ok(Self {
            fonts: Arc::new(faces),
            identity: 0,
            shape_cache: Arc::new(Mutex::new(HashMap::new())),
            layout_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextLoadError> {
        let face = FontFace::from_bytes(Arc::new(bytes.to_vec()), 0)?;
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
        let idx = if font_index_switch() { shared_index() } else { None };
        // A3 身份采信（§7 客户端采信条款，裁定 (a)）：索引有效且全部字面按位 content
        // 命中 ⇒ 采信 header.stack_identity，跳过整趟栈 FNV；任一不中（缺失/损坏/
        // 数目不符/content 不中）回退下方自算，值逐位一致。缺省关档不走此径。
        if let Some(idx) = idx
            && self.adopt_identity_with(idx)
        {
            for font in self.fonts.iter() {
                font.resolve_index_coverage(idx);
            }
            return;
        }
        let mut hash = 0xcbf29ce484222325_u64;
        for (font_index, font) in self.fonts.iter().enumerate() {
            // A3.1：同源字重实例共享同一份 `content_hash`，只有首个实例在身份遍历里
            // 顺带算（与索引 `content_hash` 同算法）；无索引时不付这份额外运算。
            // 采信未中时 content 可能已被预置（留用），此处自然落栈-only 分支。
            let fresh = idx.is_some() && font.content_hash.get().is_none();
            if fresh && font_index == 0 {
                // 首字面的身份累加器仍是 FNV 初值，结果与纯文件哈希完全相同，
                // 复用一次计算；后续字面仍按原栈序逐字节累加，身份协议不变。
                // 真机无头实测载入 p50 410.6 → 367.2 ms（首字面 19 MB 少一路乘法）。
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
                let _ = font.content_hash.set(hash);
            } else if fresh {
                let mut content = 0xcbf29ce484222325_u64;
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                    content ^= u64::from(*byte);
                    content = content.wrapping_mul(0x100000001b3);
                }
                let _ = font.content_hash.set(content);
            } else {
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
            hash ^= u64::from(font.collection_index);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        self.identity = hash;
        // A3.1 载入期决策：`content_hash + collection_index` 命中 → 运行期查索引区段；
        // 未命中 → 此刻补建覆盖表（仍属载入期，行为与旧路径一致）。
        if let Some(idx) = idx {
            for font in self.fonts.iter() {
                font.resolve_index_coverage(idx);
            }
        }
    }

    /// 全命中采信尝试（§7 客户端采信条款，裁定 (a)）：全部字面按位 content_hash +
    /// `collection_index` 命中 ⇒ 置身份为 `idx.header().stack_identity` 并返回 true，
    /// 跳过整趟栈 FNV。content 按源算一次（同源实例共享句柄）；任一不中 / 索引缺失 /
    /// 损坏 / 数目不符返回 false——身份保持未动，调用方回退逐字节自算；已预置的
    /// content_hash 留用（栈哈希不依赖 content，回退值与不采信时逐位一致）。
    /// 单独成函数以便测试注入夹具索引（进程级开关不可按用例隔离）。
    fn adopt_identity_with(&mut self, idx: &crate::font_index::OwnedIndex) -> bool {
        let n = usize::try_from(idx.header().font_count).unwrap_or(0);
        if n == 0 || n != self.fonts.len() {
            return false;
        }
        for font in self.fonts.iter() {
            if font.content_hash.get().is_none() {
                let _ = font.content_hash.set(crate::font_index::fnv1a64(font.data()));
            }
        }
        for (i, font) in self.fonts.iter().enumerate() {
            let Some(e) = idx.entry(i) else {
                return false;
            };
            let Some(h) = font.content_hash.get().copied() else {
                return false;
            };
            if e.content_hash != h || u32::from(e.collection_index) != font.collection_index {
                return false;
            }
        }
        self.identity = idx.header().stack_identity;
        true
    }

    fn font_for_grapheme(&self, grapheme: &str) -> usize {
        count_font_for_grapheme();
        let idx = if font_index_switch() { shared_index() } else { None };
        self.font_for_grapheme_with(idx, grapheme)
    }

    /// 纯核：`idx` 为 Some 且对本栈有效（栈身份相等 + 条目数与字面数一致）→ 覆盖查询
    /// 走索引区段（`CoverageRanges::has`，与内存表同语义、A3.0 3000 码点对拍锁定）；
    /// 否则完全走旧路径（加载时内存覆盖表）。两路对无覆盖字素都回落 0。
    /// 测试经此注入 A3.0 写侧夹具（进程级 OnceLock 开关不可按用例隔离，环境只在外壳读）。
    fn font_for_grapheme_with(
        &self,
        idx: Option<&crate::font_index::OwnedIndex>,
        grapheme: &str,
    ) -> usize {
        if self.index_valid(idx) {
            return (0..self.fonts.len())
                .find(|&i| {
                    index_ranges_in(idx, i).is_some_and(|r| {
                        grapheme.chars()
                            .all(|c| is_default_ignorable(c) || c.is_whitespace() || r.has(c))
                    })
                })
                .unwrap_or(0);
        }
        self.fonts
            .iter()
            .position(|font| font.grapheme_covered(grapheme))
            .unwrap_or(0)
    }

    /// 索引对本栈有效：栈身份相等（A3.0 锁定同算法）且条目数与字面数一致。
    fn index_valid(&self, idx: Option<&crate::font_index::OwnedIndex>) -> bool {
        match idx {
            Some(idx) => {
                idx.header().stack_identity == self.identity()
                    && usize::try_from(idx.header().font_count).ok() == Some(self.fonts.len())
            }
            None => false,
        }
    }

    /// 按请求字重选字面下标（T7 + §112）：可变字体精确实例 → 任何精确 →
    /// ExtraBold(800) 宁轻勿重（取不重于 800 的最大档，即静态 Bold 700）→ 最近且不轻于 → 最近。
    /// 请求字重的字面缺字时，调用方再走 `font_for_grapheme` 的覆盖回退。
    fn face_for_weight(&self, weight: FontWeight) -> usize {
        if self.fonts.len() <= 1 {
            return 0;
        }
        let target = weight_target(weight);
        // 可变字体：优先精确实例条目（多条同源，仅 wght 轴不同）。
        if let Some(i) = self
            .fonts
            .iter()
            .position(|f| f.wght.is_some() && f.weight == target)
        {
            return i;
        }
        if let Some(i) = self.fonts.iter().position(|f| f.weight == target) {
            return i;
        }
        // §112 静态回退：ExtraBold(800) 宁轻勿重，取不重于 800 的最大档（Bold 700 优先于 Black 900）。
        if weight == FontWeight::ExtraBold
            && let Some((i, _)) = self
                .fonts
                .iter()
                .enumerate()
                .filter(|(_, f)| f.weight <= target)
                .max_by_key(|(_, f)| f.weight)
        {
            return i;
        }
        if let Some((i, _)) = self
            .fonts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.weight >= target)
            .min_by_key(|(_, f)| f.weight)
        {
            return i;
        }
        self.fonts
            .iter()
            .enumerate()
            .filter(|(_, f)| f.weight < target)
            .max_by_key(|(_, f)| f.weight)
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// ExtraBold(800) 是否由可变字体精确实例化。`false` = 无可变字体，静态回退到
    /// Bold 700 / 最近档；平台层据此在 `~/.local/state/ether/` 的字体诊断日志落一行（§112）。
    pub fn extra_bold_is_variable(&self) -> bool {
        let target = weight_target(FontWeight::ExtraBold);
        self.fonts
            .iter()
            .any(|f| f.wght.is_some() && f.weight == target)
    }

    /// 各字面覆盖表统计（诊断口径，w1029 §3 报告用）：区间数、覆盖码点数、内存（字节）与建表耗时。
    /// A3.1 起覆盖表惰性：本调用是「非索引用途」，会补齐索引命中路径未建的表；
    /// 数值口径与旧路径相同，`build_us` 是首次建表本身的耗时。
    pub fn diag_coverage_stats(&self) -> Vec<CoverageStat> {
        self.fonts
            .iter()
            .map(|f| {
                // A3.1：诊断是「非索引用途」，首次调用才建表（索引命中时载入期为 0）。
                let coverage = f.coverage_table();
                CoverageStat {
                    ranges: coverage.ranges.len(),
                    codepoints: coverage.codepoints(),
                    memory_bytes: coverage.memory_bytes(),
                    build_us: coverage.build_us,
                }
            })
            .collect()
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
        let cluster_segments = self.cluster_segments(text, size, letter_spacing_em, weight);
        let mut cluster_bounds = Vec::with_capacity(cluster_segments.len() + 1);
        let mut acc = 0usize;
        cluster_bounds.push(0);
        for s in &cluster_segments {
            acc += s.len();
            cluster_bounds.push(acc);
        }

        let mut lines = Vec::new();
        let mut current = String::new();
        let mut prev_end = 0usize;

        for (byte_idx, opportunity) in unicode_linebreak::linebreaks(text) {
            // 确保换行绝不切入簇内：非簇边界的折行点外扩对齐到下一个簇边界
            let valid_end = match cluster_bounds.binary_search(&byte_idx) {
                Ok(i) => cluster_bounds[i],
                Err(i) => {
                    if i < cluster_bounds.len() {
                        cluster_bounds[i]
                    } else {
                        byte_idx
                    }
                }
            };
            if valid_end <= prev_end {
                continue;
            }
            let segment = &text[prev_end..valid_end];
            prev_end = valid_end;
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
                self.hard_break_clusters(
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
                    self.hard_break_clusters(
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
            vec![self.create_line(
                &text.replace(['\n', '\r'], " "),
                size,
                options.letter_spacing_em,
                weight,
            )]
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
            return self.create_line("", size, spacing, weight);
        }
        let mut clusters = self.cluster_segments(&line.content, size, spacing, weight);
        loop {
            let content = format!("{}{}", clusters.concat().trim_end(), ellipsis);
            let width = self.measure_with_spacing_weighted(&content, size, spacing, weight);
            if width <= max_width || clusters.is_empty() {
                return self.create_line(&content, size, spacing, weight);
            }
            clusters.pop();
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn hard_break_clusters(
        &self,
        segment: &str,
        size: f32,
        spacing: f32,
        max_width: f32,
        weight: FontWeight,
        lines: &mut Vec<Line>,
        current: &mut String,
    ) {
        for cluster in self.cluster_segments(segment, size, spacing, weight) {
            let candidate = format!("{current}{cluster}");
            if !current.is_empty()
                && self.measure_with_spacing_weighted(&candidate, size, spacing, weight)
                    > max_width + FIT_EPSILON
            {
                self.push_line(lines, current, size, spacing, weight);
            }
            current.push_str(cluster);
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
            lines.push(self.create_line(&content, size, spacing, weight));
        }
        current.clear();
    }

    /// 为一段单行文本构建排版结果，包含精确的簇边界与各簇光标坐标。
    pub fn create_line(
        &self,
        text: &str,
        size: f32,
        letter_spacing_em: f32,
        weight: FontWeight,
    ) -> Line {
        if text.is_empty() {
            return Line {
                content: String::new(),
                width: 0.0,
                cluster_offsets: vec![0],
                cluster_carets: vec![0.0],
            };
        }
        let glyphs = self.shape_line_weighted(text, size, letter_spacing_em, weight);
        let mut visual_groups = Vec::<(usize, bool, f32, f32)>::new();
        let mut pen = 0.0_f32;
        for glyph in &glyphs {
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

        let mut glyph_cluster_starts: Vec<usize> = visual_groups
            .iter()
            .map(|(cluster, _, _, _)| *cluster)
            .collect();
        glyph_cluster_starts.sort_unstable();
        glyph_cluster_starts.dedup();

        let grapheme_bounds: Vec<usize> = text
            .grapheme_indices(true)
            .map(|(byte, _)| byte)
            .chain(std::iter::once(text.len()))
            .collect();

        // 簇边界：以字素簇为基准，若塑形合并为连字（跨字素），中间字素边界剔除
        let mut cluster_offsets = Vec::new();
        cluster_offsets.push(0);
        for &g in &grapheme_bounds[1..] {
            if (g == text.len() || glyph_cluster_starts.contains(&g))
                && cluster_offsets.last() != Some(&g)
            {
                cluster_offsets.push(g);
            }
        }
        if cluster_offsets.last() != Some(&text.len()) {
            cluster_offsets.push(text.len());
        }

        let mut cluster_carets = vec![f32::NAN; cluster_offsets.len()];
        cluster_carets[0] = 0.0;
        let last_idx = cluster_offsets.len() - 1;
        cluster_carets[last_idx] = pen.abs();

        for i in 0..cluster_offsets.len() - 1 {
            let c_start = cluster_offsets[i];
            let c_end = cluster_offsets[i + 1];
            let matching: Vec<_> = visual_groups
                .iter()
                .filter(|(start, _, _, _)| *start >= c_start && *start < c_end)
                .collect();
            if !matching.is_empty() {
                let is_rtl = matching[0].1;
                let min_x = matching.iter().map(|(_, _, x0, _)| *x0).fold(f32::INFINITY, f32::min);
                let max_x = matching.iter().map(|(_, _, _, x1)| *x1).fold(f32::NEG_INFINITY, f32::max);
                if is_rtl {
                    cluster_carets[i] = max_x;
                    cluster_carets[i + 1] = min_x;
                } else {
                    cluster_carets[i] = min_x;
                    cluster_carets[i + 1] = max_x;
                }
            }
        }

        let mut prev = 0.0;
        for c in &mut cluster_carets {
            if c.is_finite() {
                prev = *c;
            } else {
                *c = prev;
            }
        }

        Line {
            content: text.to_string(),
            width: pen.abs(),
            cluster_offsets,
            cluster_carets,
        }
    }

    /// 返回文本切分后的簇片段（每个片段在簇边界处切开，不在簇内断开）。
    pub fn cluster_segments<'a>(
        &self,
        text: &'a str,
        size: f32,
        letter_spacing_em: f32,
        weight: FontWeight,
    ) -> Vec<&'a str> {
        if text.is_empty() {
            return Vec::new();
        }
        let line = self.create_line(text, size, letter_spacing_em, weight);
        let mut segments = Vec::new();
        for i in 0..line.cluster_offsets.len() - 1 {
            let start = line.cluster_offsets[i];
            let end = line.cluster_offsets[i + 1];
            segments.push(&text[start..end]);
        }
        segments
    }
}

fn is_default_ignorable(c: char) -> bool {
    matches!(c, '\u{200C}' | '\u{200D}' | '\u{FE0E}' | '\u{FE0F}')
}

// ---- A3.3 开关：`KANESUMI_FONT_INDEX=1`（与 KANESUMI_TEXT_DIAG 同口径读取一次）----

/// A3.3 开关：`KANESUMI_FONT_INDEX=1`（进程内一次性读取，与 KANESUMI_TEXT_DIAG 同口径）。
fn font_index_switch() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var_os("KANESUMI_FONT_INDEX").is_some_and(|v| v == "1"))
}

/// 进程内唯一索引句柄：开关关 → None；开但文件缺失 / 损坏 → None（完全走旧路径）。
/// 路径沿用设计稿 A3.1 口径：`$XDG_RUNTIME_DIR/kanesumi/fonts-v1.idx`（未设即无索引）。
/// 缺失 / 损坏静默回退（kanesumi-canvas 无日志依赖，不新增）；细节归 A3.2 失效件。
fn shared_index() -> Option<&'static crate::font_index::OwnedIndex> {
    static IDX: std::sync::OnceLock<Option<crate::font_index::OwnedIndex>> =
        std::sync::OnceLock::new();
    IDX.get_or_init(|| {
        if !font_index_switch() {
            return None;
        }
        let dir = std::env::var_os("XDG_RUNTIME_DIR").filter(|d| !d.is_empty())?;
        let path = std::path::PathBuf::from(dir).join("kanesumi").join("fonts-v1.idx");
        crate::font_index::OwnedIndex::open(&path).ok()
    })
    .as_ref()
}

/// 条目 `i` 的覆盖区段（借用传入索引）。
fn index_ranges_in<'i>(
    idx: Option<&'i crate::font_index::OwnedIndex>,
    i: usize,
) -> Option<crate::font_index::CoverageRanges<'i>> {
    let idx = idx?;
    idx.coverage(i)
}

/// 自由函数（测试可注入非 `'static` 夹具）：按 `content_hash`（纯文件字节 FNV-1a）+
/// `collection_index` 匹配索引条目，取其覆盖区段。
fn index_entry_coverage_in<'i>(
    idx: &'i crate::font_index::OwnedIndex,
    data: &[u8],
    collection_index: u32,
) -> Option<crate::font_index::CoverageRanges<'i>> {
    index_entry_coverage_by_hash(idx, crate::font_index::fnv1a64(data), collection_index)
}

/// 同上，但 `content_hash` 由调用方给（A3.1 载入期在 `refresh_identity` 的身份遍历里
/// 顺带算出，避免为判命中再读一整趟字体字节）。
fn index_entry_coverage_by_hash<'i>(
    idx: &'i crate::font_index::OwnedIndex,
    hash: u64,
    collection_index: u32,
) -> Option<crate::font_index::CoverageRanges<'i>> {
    (0..usize::try_from(idx.header().font_count).unwrap_or(0))
        .filter_map(|i| idx.entry(i).map(|e| (i, e)))
        .find(|(_, e)| {
            e.content_hash == hash && u32::from(e.collection_index) == collection_index
        })
        .and_then(|(i, _)| idx.coverage(i))
}

// ---- 文本诊断计数（w1029 text-coverage-cache §2-1）----
//
// 缺省关闭：`KANESUMI_TEXT_DIAG=1` 或 A3.3 的 `KANESUMI_FONT_INDEX=1` 才计数、
// 启动线程与落盘——每 10 s 一行追加到 `~/.local/state/ether/kanesumi-text.log`
// （XDG_STATE_HOME 可重定向），进程退出路径可调 [`text_diag_flush`] 立即补一行。
// 零交互排障口径，参 AGENTS.md 铁律 3。

static FACE_PARSE_REBUILDS: AtomicU64 = AtomicU64::new(0);
static FONT_FOR_GRAPHEME_CALLS: AtomicU64 = AtomicU64::new(0);
/// 每进程 `Face::parse` 调用总数（A3.3 诊断：族名探测 / 加载校验 / 解析面重建三处）。
static FACE_PARSE_CALLS: AtomicU64 = AtomicU64::new(0);
/// 覆盖表建表次数（A3.1 惰性化门诊断：索引命中时载入后应为 0）。
static COVERAGE_BUILDS: AtomicU64 = AtomicU64::new(0);

thread_local! {
    /// 当前线程建表次数。进程级计数在并行用例下无法判「增量为 0」，
    /// 门用线程内计数（同一用例的载入与快照必在同一线程）。
    static COVERAGE_BUILDS_TLS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn count_coverage_build() {
    COVERAGE_BUILDS.fetch_add(1, Ordering::Relaxed);
    COVERAGE_BUILDS_TLS.with(|c| c.set(c.get() + 1));
}

/// 进程累计覆盖表建表次数（A3.1 诊断口径）。
pub fn coverage_builds() -> u64 {
    COVERAGE_BUILDS.load(Ordering::Relaxed)
}

/// 当前线程累计建表次数快照（测试判「载入后建表计数 = 0」用）。
#[cfg(test)]
fn coverage_builds_on_thread() -> u64 {
    COVERAGE_BUILDS_TLS.with(std::cell::Cell::get)
}

fn count_parse_rebuild() {
    if !diag_enabled() {
        return;
    }
    FACE_PARSE_REBUILDS.fetch_add(1, Ordering::Relaxed);
    diag_spawn_writer();
}

fn count_font_for_grapheme() {
    if !diag_enabled() {
        return;
    }
    FONT_FOR_GRAPHEME_CALLS.fetch_add(1, Ordering::Relaxed);
    diag_spawn_writer();
}

/// `Face::parse` 计数无条件（原子加远小于解析本身），打印由 [`diag_enabled`] 门控。
fn count_face_parse() {
    FACE_PARSE_CALLS.fetch_add(1, Ordering::Relaxed);
}

/// `Face::parse` 调用总数快照（A3.3；测试与基准取轮内增量）。
pub fn face_parse_calls() -> u64 {
    FACE_PARSE_CALLS.load(Ordering::Relaxed)
}

fn diag_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var_os("KANESUMI_TEXT_DIAG").is_some_and(|v| v == "1") || font_index_switch()
    })
}

/// (解析面重建次数，`font_for_grapheme` 调用次数) 累计快照，供基准取轮内增量。
pub fn text_diag_snapshot() -> (u64, u64) {
    (
        FACE_PARSE_REBUILDS.load(Ordering::Relaxed),
        FONT_FOR_GRAPHEME_CALLS.load(Ordering::Relaxed),
    )
}

fn diag_log_path() -> Option<PathBuf> {
    let dir = match std::env::var_os("XDG_STATE_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d).join("ether"),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/state/ether"),
    };
    Some(dir.join("kanesumi-text.log"))
}

fn diag_write_line() {
    let Some(path) = diag_log_path() else { return };
    let (parses, ffg) = text_diag_snapshot();
    let calls = face_parse_calls();
    let line = format!(
        "kanesumi-text-diag face_parse_rebuilds={parses} font_for_grapheme_calls={ffg} face_parse_calls={calls}\n"
    );
    let Some(dir) = path.parent() else { return };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

/// 立即落一行诊断（进程退出路径；基准 example 收尾时调用）。未开开关时空操作。
pub fn text_diag_flush() {
    if diag_enabled() {
        diag_write_line();
    }
}

fn diag_spawn_writer() {
    use std::sync::Once;
    static SPAWN: Once = Once::new();
    SPAWN.call_once(|| {
        if diag_enabled() {
            // 周期写线程起不来就放弃：诊断是辅助，不影响主流程。
            let _ = std::thread::Builder::new().name("text-diag".into()).spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_secs(10));
                    diag_write_line();
                }
            });
        }
    });
}

// ---- 会话字体链唯一真源（A3 集成裁决 2026-10-31）----

/// CJK 候选的 TTC 字面标签（与 harness `platform.rs::SC_TAG` 同值；A 线随后改为
/// 调用本函数，标签表随之单源化）。
const CHAIN_SC_TAG: Option<&str> = Some("SC");

/// 主字面候选链（有序；与 harness `regular_candidates` 逐条一致——唯一真源落地期间
/// 两表并存，改动必须双侧同步并 bump `font_index::POLICY_VERSION`）。
const REGULAR_CANDIDATES: [(&str, Option<&str>); 12] = [
    ("/usr/local/share/fonts/s/NotoSansSC-VF.ttf", None),
    ("/usr/local/share/fonts/s/NotoSansCJK-VF.otf.ttc", CHAIN_SC_TAG),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-VF.otf.ttc", CHAIN_SC_TAG),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-VF.otf.ttc", CHAIN_SC_TAG),
    ("/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf", None),
    ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", CHAIN_SC_TAG),
    ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", CHAIN_SC_TAG),
    ("/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf", None),
    ("/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf", None),
    ("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", None),
    ("/usr/share/fonts/TTF/DejaVuSans.ttf", None),
    ("/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf", None),
];

/// 字重字面候选（Light / Medium / Bold 各自依序找首个存在；与 harness `weight_candidates` 一致）。
fn chain_weight_candidates(weight: &str) -> Vec<FontSource> {
    [
        (format!("/usr/local/share/fonts/s/SourceHanSansSC-{weight}.otf"), None),
        (format!("/usr/share/fonts/noto-cjk/NotoSansCJK-{weight}.ttc"), CHAIN_SC_TAG),
        (format!("/usr/share/fonts/opentype/noto/NotoSansCJK-{weight}.ttc"), CHAIN_SC_TAG),
    ]
    .into_iter()
    .map(|(path, tag)| FontSource { path: PathBuf::from(path), collection_tag: tag })
    .collect()
}

/// 主字面之外的脚本回退（有序；与 harness `extra_sources` 的固定表一致）。
const CHAIN_EXTRA_PATHS: [&str; 5] = [
    "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
    "/usr/share/fonts/noto/NotoSansHebrew-Regular.ttf",
    "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/TTF/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
];

/// 会话字体链：主字面 →（主字面非 VF 时的）Light/Medium/Bold → 脚本回退。
/// 与 harness `find_font_source`（去掉 `KANESUMI_TEST_FONT` 旁路）+ `extra_sources(primary)`
/// 语义完全一致：只含存在的路径、canonical 去重、**条目序 = 回退链序**。
/// 索引构建器（`ether-font-index`）按本链枚举条目；harness 为第二消费方（A 线接线）。
pub fn system_font_chain() -> Vec<FontSource> {
    let Some(primary) = REGULAR_CANDIDATES
        .iter()
        .map(|(path, tag)| FontSource { path: PathBuf::from(path), collection_tag: *tag })
        .find(|s| s.path.exists())
    else {
        return Vec::new();
    };
    let mut out = vec![primary.clone()];
    let primary_is_vf = primary
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains("-VF"));
    if !primary_is_vf {
        for weight in ["Light", "Medium", "Bold"] {
            if let Some(src) = chain_weight_candidates(weight).into_iter().find(|s| s.path.exists())
            {
                out.push(src);
            }
        }
    }
    for path in CHAIN_EXTRA_PATHS {
        let path = PathBuf::from(path);
        if path.exists() {
            out.push(FontSource { path, collection_tag: None });
        }
    }
    // canonical 去重（与 `load_stack_with` 的 seen 语义一致；主字面排首位保序）。
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| {
        seen.insert(s.path.canonicalize().unwrap_or_else(|_| s.path.clone()))
    });
    out
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

    /// 新旧入口等价（runtime A1）：`load_stack_with` 与 `load_stack` 对同一字体
    /// 排出完全相同的字形与光栅位图——字节句柄只换来源，不换解析结果。
    #[test]
    fn load_stack_with_matches_load_stack() {
        let Some(path) = find_font() else {
            return; // 既有约定：无系统字体时跳过（同 engine() 系测试）
        };
        let primary = FontSource { path: path.clone(), collection_tag: None };
        let old = TextEngine::load_stack(&primary, &[]).expect("旧入口应加载成功");
        let new = TextEngine::load_stack_with(&primary, &[], &|p| {
            std::fs::read(p).map(|bytes| -> FontBytes { Arc::new(bytes) })
        })
        .expect("新入口应加载成功");
        assert_eq!(old.font_count(), new.font_count());
        assert_eq!(old.identity(), new.identity(), "同一字节的字体栈身份必须一致");

        let text = "Ether 悬停 Ether 123abg";
        let a = old.shape_line(text, 24.0, 0.0);
        let b = new.shape_line(text, 24.0, 0.0);
        assert_eq!(a, b, "塑形输出必须逐字段一致");
        if let Some(g) = a.first() {
            let (ma, ba) = old.rasterize_glyph(g.font_id, g.glyph_id, 24.0);
            let (mb, bb) = new.rasterize_glyph(g.font_id, g.glyph_id, 24.0);
            assert_eq!(ma, mb, "字形度量必须一致");
            assert_eq!(ba, bb, "同字形位图必须逐字节一致");
        }
    }

    /// 覆盖表与旧 `glyph_index` 路径逐点相等（w1029 text-coverage-cache §2-3）：
    /// 真机 Noto CJK 栈抽3000个不同码点（CJK / 拉丁 / 符号 / 缺字 PUA / emoji / 上界），
    /// 每字面逐一比对；另对混排段落 `font_for_grapheme` 新旧选择逐字素相等。
    /// 缺字体则跳过并打印原因——判门机（Debian）必有，跳过不算通过。
    #[test]
    fn coverage_matches_glyph_index_on_real_stack() {
        let noto = [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
        ];
        let present: Vec<PathBuf> = noto
            .iter()
            .filter(|p| Path::new(p).exists())
            .map(PathBuf::from)
            .collect();
        if present.len() != noto.len() {
            eprintln!("跳过原因：真机 Noto CJK Regular/Bold字体栈不完整（{noto:?}）；不计验收通过");
            return;
        }
        let primary = FontSource { path: present[0].clone(), collection_tag: Some("SC") };
        let extra: Vec<FontSource> = present[1..]
            .iter()
            .map(|p| FontSource { path: p.clone(), collection_tag: Some("SC") })
            .collect();
        let engine = TextEngine::load_stack(&primary, &extra).expect("字体栈加载失败");
        assert!(engine.font_count() >= 1, "SC 字面应至少加载一条");
        assert_eq!(engine.font_count(), 2, "Regular/Bold的SC字面必须均参与比较");

        // 保留原有边界点，去重后从互不重叠的扩展A区补足3000个不同码点。
        let mut cps: Vec<u32> = Vec::new();
        cps.extend((0x4E00u32..0x9FFF).step_by(2).take(2000));
        cps.extend(0x20u32..0x7F);
        cps.extend(0x2000u32..0x206F);
        cps.extend(0x3000u32..0x303F);
        cps.extend((0xFF00u32..0xFFEF).step_by(2));
        cps.extend((0x2E80u32..0x2FDF).step_by(7));
        cps.extend((0xE000u32..0xF8FF).step_by(13));
        cps.extend([0x1F600, 0x10FFFF, 0x0, 0x10_FFFF]);
        cps.sort_unstable();
        cps.dedup();
        assert!(cps.len() <= 3000);
        cps.extend(0x3400..0x3400 + (3000 - cps.len()) as u32);
        cps.sort_unstable();
        cps.dedup();
        assert_eq!(cps.len(), 3000, "缺样本或重复样本不能计作3000码点");
        let mut mismatches: Vec<(u32, u32, char, bool, bool)> = Vec::new();
        for font in engine.fonts.iter() {
            for &cp in &cps {
                let Some(c) = char::from_u32(cp) else { continue };
                // 旧路径参照：has_glyph 原实现 = 解析面 glyph_index 命中。
                let old = ttf_parser::Face::parse(font.data(), font.collection_index)
                    .expect("加载时已验证")
                    .glyph_index(c)
                    .is_some();
                let new = font.coverage_table().has(c);
                if old != new {
                    mismatches.push((font.collection_index, cp, c, old, new));
                }
            }
        }
        assert!(
            mismatches.is_empty(),
            "覆盖表与 glyph_index 不一致 {} 例，样例 {:?}",
            mismatches.len(),
            &mismatches[..mismatches.len().min(8)]
        );

        // font_for_grapheme 段落逐字素相等：旧选择 = 逐字面「全字符 glyph_index 命中」。
        let para = "Ether 悬停渲染，中英混排 123 abg——字形覆盖缓存。passim…e\u{301}👩\u{200D}💻\u{E000}\u{10FFFF}";
        let old_covered = |f: &FontFace, grapheme: &str| {
            grapheme.chars().all(|c| {
                is_default_ignorable(c)
                    || c.is_whitespace()
                    || ttf_parser::Face::parse(f.data(), f.collection_index)
                        .expect("加载时已验证")
                        .glyph_index(c)
                        .is_some()
            })
        };
        for grapheme in para.graphemes(true) {
            let old_pick = engine
                .fonts
                .iter()
                .position(|f| old_covered(f, grapheme))
                .unwrap_or(0);
            let new_pick = engine.font_for_grapheme(grapheme);
            assert_eq!(old_pick, new_pick, "字素 {grapheme:?} 选面必须一致");
        }
        println!("COVERAGE_CHECK faces=2 codepoints=3000 glyph_checks=6000 graphemes={}",
                 para.graphemes(true).count());
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
/// T7 字重选择：精确定位（含 §112 静态回退）。用系统 Noto CJK TTC（与思源同源，
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
    assert_eq!(ids(FontWeight::Light), 0, "Light(300) 无 300 → 最近不轻于 400");
    assert_eq!(ids(FontWeight::Normal), 0, "Normal → 400 字面");
    assert_eq!(ids(FontWeight::Semilight), 0, "Semilight(350) → 最近不轻于 400");
    assert_eq!(ids(FontWeight::Medium), 1, "Medium(500) → 精确 500 字面");
    assert_eq!(ids(FontWeight::Semibold), 1, "Semibold(600) 无 600 → 最近 500");
    assert_eq!(ids(FontWeight::Bold), 1, "Bold(700) 无 700 → 最近 500");
    // 塑形缓存按字重隔离：同文本同字号不同字重 → 不同字面来源。
    assert_ne!(ids(FontWeight::Normal), ids(FontWeight::Medium));
}

/// §112 静态回退：无可变字体时 ExtraBold(800) 宁轻勿重 → 静态 Bold 700，而非 Black 900。
/// 用系统 Noto CJK Regular + Bold（SC 字面）；机器无字体时跳过。
#[test]
fn static_fallback_extra_bold_picks_bold_not_black() {
    use crate::text::FontSource;
    let dir = std::path::PathBuf::from("/usr/share/fonts/noto-cjk");
    let regular = dir.join("NotoSansCJK-Regular.ttc");
    let bold = dir.join("NotoSansCJK-Bold.ttc");
    if !regular.exists() || !bold.exists() {
        return;
    }
    let engine = TextEngine::load_stack(
        &FontSource { path: regular, collection_tag: Some("SC") },
        &[FontSource { path: bold, collection_tag: Some("SC") }],
    )
    .unwrap();
    assert!(!engine.extra_bold_is_variable(), "静态栈不得报告可变字体");
    let ids = |w: FontWeight| {
        engine
            .shape_line_weighted("以太 Ether", 16.0, 0.0, w)
            .first()
            .map(|g| g.font_id)
            .unwrap()
    };
    assert_eq!(ids(FontWeight::Bold), 1, "Bold → 精确 700 字面");
    assert_eq!(ids(FontWeight::ExtraBold), 1, "ExtraBold(800) → 静态回落 Bold 700");
    // 回落选中的确实是 700 档，而非更重的 Black：字面 usWeightClass 由光栅路径决定。
    let selected = engine.fonts[ids(FontWeight::ExtraBold) as usize].weight;
    assert_eq!(selected, 700, "800 必须回落 Bold(700)，不是 Black(900)");
}

/// §112 可变字体：带 `wght` 轴的字体按 300 / 400 / 800 实例化后，墨迹面积单调递增。
/// 测试字体经 `KANESUMI_TEST_VF` 指定，或落在系统候选路径；不在场时跳过（既不误报也不阻塞 CI）。
#[test]
fn variable_font_weight_instances_grow_ink_monotonically() {
    let Some(path) = find_variable_font() else {
        return;
    };
    let engine = TextEngine::load_stack(
        &FontSource { path, collection_tag: None },
        &[],
    )
    .unwrap();
    assert!(engine.extra_bold_is_variable(), "带 wght 轴的字体应实例化 ExtraBold");

    // 同一 CJK 字在三个字重下光栅化，比较覆盖率之和（≈ 墨迹面积 / 笔宽代理）。
    let ink = |w: FontWeight| -> u64 {
        let glyphs = engine.shape_line_weighted("永", 64.0, 0.0, w);
        let g = glyphs.first().expect("应塑形出字形");
        let (m, bitmap) = engine.rasterize_glyph(g.font_id, g.glyph_id, 64.0);
        assert!(m.width > 0 && m.height > 0, "字形位图非空");
        bitmap.iter().map(|&b| u64::from(b)).sum()
    };
    let light = ink(FontWeight::Light);
    let normal = ink(FontWeight::Normal);
    let extra_bold = ink(FontWeight::ExtraBold);
    assert!(
        light < normal && normal < extra_bold,
        "300 / 400 / 800 墨量应严格递增：{light} / {normal} / {extra_bold}"
    );
}

/// 可变字体测试源：环境变量优先，其次系统 / 开发机常见路径。
fn find_variable_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_VF") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "/usr/local/share/fonts/s/NotoSansSC-VF.ttf",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-VF.otf.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-VF.otf.ttc",
        "/usr/share/fonts/truetype/noto/NotoSansSC-VF.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|p| p.exists())
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

// ── K3 文字整形地基（参 Ether docs/research/k_text_shaping/DESIGN.md）───────────────────
//
// 排版单位自「字符」改为「整形后的字形串」：同字体 / 方向 / 脚本的连续文本交 rustybuzz
// 整形，产出 `ShapedGlyph { glyph_id, cluster（UTF-8 字节）, x_advance, x_offset, y_offset }`。
// 以下测试锁死四条契约：① 中日韩 / ASCII 位置与「逐字符取度量」的旧结果一致（金样 ≤0.5px）；
// ② 连字、组合 emoji、组合附加符按簇整体处理；③ 换行与省略号不切入簇内；
// ④ CPU / GPU 两路对同一段整形文本逐像素一致（后者在 kanesumi-render 的 golden_offscreen）。

/// Settings 导航常用字串 20 条（K3 金样集）。覆盖 CJK、含拉丁混排与 `HiDPI` 等型号词。
const SETTINGS_GOLDEN_STRINGS: &[&str] = &[
    "系统", "设备", "个性化", "声音", "时间和语言", "隐私", "通知", "显示",
    "存储", "电源", "网络", "连接", "打印机", "用户", "辅助功能", "应用管理",
    "个性化 Chorus", "Wi-Fi", "缩放与 HiDPI", "提权认证 uniauth",
];

/// 逐字符取度量的「未整形」宽度（K3 之前的算法）：每字符独立前进量之和。
fn unshaped_width(engine: &TextEngine, text: &str, size: f32) -> f32 {
    text.chars()
        .map(|c| engine.glyph_metrics(c, size).advance_width)
        .sum()
}

/// 金样：Settings 20 条字串整形后字形位置与未整形参考的位置差 ≤ 0.5 px。
/// 这些串不含 `ff` / `fi` / `fl` 连字，字形与字符一一对应，故可逐位对齐比较。
#[test]
fn settings_golden_strings_shaping_within_half_px() {
    let Some(engine) = engine() else { return };
    let size = 14.0;
    for text in SETTINGS_GOLDEN_STRINGS {
        let glyphs = engine.shape_line(text, size, 0.0);
        let chars: Vec<char> = text.chars().collect();
        assert_eq!(
            glyphs.len(),
            chars.len(),
            "金样集不得含连字（{text}：{} 字形 / {} 字符）",
            glyphs.len(),
            chars.len()
        );
        let mut shaped_pen = 0.0_f32;
        let mut unshaped_pen = 0.0_f32;
        let mut max_diff = 0.0_f32;
        for (i, c) in chars.iter().enumerate() {
            max_diff = max_diff.max((shaped_pen - unshaped_pen).abs());
            shaped_pen += glyphs[i].x_advance;
            unshaped_pen += engine.glyph_metrics(*c, size).advance_width;
        }
        max_diff = max_diff.max((shaped_pen - unshaped_pen).abs());
        assert!(
            max_diff <= 0.5,
            "{text} 整形前后字形位置差 {max_diff:.3} px > 0.5 px"
        );
    }
}

/// 拉丁字距对（kerning）：整形应让它们**变窄或持平**，不构成金样失败。
/// 单列报告差值，供人核对（`cargo test ... -- --nocapture`）。
#[test]
fn kerning_pairs_shaped_not_wider_than_unshaped() {
    let Some(engine) = engine() else { return };
    let size = 14.0;
    for pair in ["AV", "To", "Va", "WA", "Settings"] {
        let shaped = engine.measure(pair, size);
        let unshaped = unshaped_width(&engine, pair, size);
        println!("[k3-kerning] {pair:<10} shaped={shaped:.3} unshaped={unshaped:.3} delta={:.3}", shaped - unshaped);
        assert!(
            shaped <= unshaped + 0.001,
            "{pair} 整形后不得比未整形更宽（shaped {shaped:.3} / unshaped {unshaped:.3}）"
        );
    }
}

/// 连字：`ffi` / `fl` 在支持 `liga` 的拉丁字体（DejaVu Sans）上合成单个字形，
/// 且该字形簇起点为 0（整个连字是一个簇）。无该字体时跳过。
#[test]
fn ligature_ffi_and_fl_merge_into_single_glyph() {
    let Some(path) = find_ligature_font() else { return };
    let engine = TextEngine::load(&path).unwrap();
    let ffi = engine.shape_line("ffi", 24.0, 0.0);
    assert_eq!(ffi.len(), 1, "ffi 应合成一个字形（字体 {path:?}）");
    assert_eq!(ffi[0].cluster, 0, "连字簇起点为 0");
    let fl = engine.shape_line("fl", 24.0, 0.0);
    assert_eq!(fl.len(), 1, "fl 应合成一个字形");
    assert_eq!(fl[0].cluster, 0);
}

/// 整形绝不增加字形数（合并只会更少）。SC 正体（思源 / Noto CJK）无拉丁连字，
/// 此时字形数 == 字符数；有连字的字体则更少。此断言跨字体恒成立。
#[test]
fn shaping_never_increases_glyph_count() {
    let Some(engine) = engine() else { return };
    for text in ["ffi", "fl", "office", "affluent", "系统", "Settings"] {
        let glyphs = engine.shape_line(text, 20.0, 0.0);
        let chars = text.chars().count();
        assert!(
            glyphs.len() <= chars,
            "{text}：{} 字形 > {chars} 字符",
            glyphs.len()
        );
    }
}

/// 连字内部不是簇边界：`office` 的 `ffi`（字符 1..4）整体一簇，
/// 字节偏移 2 / 3 处取光标必须夹紧到 1 或 4。
#[test]
fn ligature_interior_is_not_a_cluster_boundary() {
    let Some(engine) = engine() else { return };
    let line = engine.create_line("office", 20.0, 0.0, FontWeight::Normal);
    let bounds = line.cluster_boundaries();
    assert_eq!(bounds, &[0, 1, 4, 5, 6], "o | ffi | c | e 的字节边界");
    assert!(!bounds.contains(&2) && !bounds.contains(&3), "连字内不得有边界");
    let caret_after_o = line.caret_x(1);
    let caret_after_ffi = line.caret_x(4);
    assert!(caret_after_ffi > caret_after_o, "连字整体一步，占一段宽度");
    assert_eq!(line.caret_x(2), caret_after_o, "字节 2 夹紧到前一边界");
    assert_eq!(line.caret_x(3), caret_after_ffi, "字节 3 夹紧到后一边界");
    assert_eq!(line.hit_test(caret_after_ffi), 4, "命中回到簇边界");
}

/// 组合 emoji（ZWJ 家庭序列）与组合附加符（é）是单簇：光标一步跨过，
/// 命中只落在簇边界，绝不落进簇内部。
#[test]
fn shaped_clusters_step_over_emoji_and_combining_mark() {
    let Some(engine) = engine() else { return };
    // 👨👩👧 = 4 + 3 + 4 + 3 + 4 = 18 字节。
    let family = "a\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}b";
    let line = engine.create_line(family, 18.0, 0.0, FontWeight::Normal);
    assert_eq!(line.cluster_boundaries(), &[0, 1, 19, 20], "a | 家庭 | b");
    assert_eq!(line.caret_x(19), line.caret_x(1) + engine.measure("\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}", 18.0), "家庭簇占一整段前移");
    // 簇内部任意字节都夹紧到相邻边界。
    let mid = line.caret_x(10);
    assert!(mid == line.caret_x(1) || mid == line.caret_x(19));

    let marked = engine.create_line("e\u{301}x", 18.0, 0.0, FontWeight::Normal);
    assert_eq!(marked.cluster_boundaries(), &[0, 3, 4], "é | x");
    assert_eq!(marked.caret_x(1), marked.caret_x(0), "附加符内夹紧到 é 起点");
    assert_eq!(marked.caret_x(2), marked.caret_x(3), "附加符内夹紧到 é 终点");
    assert_eq!(marked.hit_test(marked.caret_x(3)), 3);
}

/// 换行绝不切入簇内：组合附加符与 ZWJ emoji 均整簇换行。
#[test]
fn wrapping_never_splits_a_cluster() {
    let Some(engine) = engine() else { return };
    let cluster = "e\u{301}";
    let width = engine.measure(cluster, 18.0) + 0.5;
    let lines = engine.layout(&cluster.repeat(3), 18.0, width);
    assert_eq!(lines.len(), 3, "每簇一行");
    assert!(lines.iter().all(|l| l.content == cluster), "不得劈开组合附加符");

    let emoji = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}";
    let emoji_w = engine.measure(emoji, 18.0) + 0.5;
    let lines = engine.layout(&emoji.repeat(2), 18.0, emoji_w);
    assert_eq!(lines.len(), 2, "ZWJ emoji 每簇一行");
    assert!(lines.iter().all(|l| l.content == emoji), "不得劈开 ZWJ 序列");
}

/// 省略号截断不切半个簇：省略号前的正文必是完整簇的整数倍。
#[test]
fn ellipsis_truncation_never_splits_a_cluster() {
    let Some(engine) = engine() else { return };
    let unit = "e\u{301}";
    let text = unit.repeat(4);
    // 逐档预算：从「只放省略号」到「放 3 个簇 + 省略号」，正文必须始终是整簇前缀。
    for k in 0..4usize {
        let budget = engine.measure(&format!("{}…", unit.repeat(k)), 18.0);
        let mut options = TextLayoutOptions::wrapped(budget, 22.0, 22.0);
        options.wrap = false;
        options.max_lines = Some(1);
        options.overflow = TextOverflow::Ellipsis;
        let layout = engine.layout_box(&text, 18.0, options);
        assert_eq!(layout.lines.len(), 1);
        let content = &layout.lines[0].content;
        if let Some(body) = content.strip_suffix('…') {
            let whole = (0..=3).any(|m| body == unit.repeat(m));
            assert!(whole, "k={k}：省略号前必须是整簇，实际 {body:?}");
        } else {
            assert_eq!(content.as_str(), text, "k={k}：预算够放下全文时不得截断");
        }
    }
}

/// 超过可用宽度的单行（省略号）不得切入连字内部：正文只能是完整连字簇。
/// 用带 `liga` 的拉丁字体；无该字体时跳过。
#[test]
fn ellipsis_does_not_split_ligature() {
    let Some(path) = find_ligature_font() else { return };
    let engine = TextEngine::load(&path).unwrap();
    let text = "offi";
    let ell = engine.measure("…", 20.0);
    for k in 0..2usize {
        let head = ["", "o"][k];
        let budget = engine.measure(&format!("{head}{}", if k == 0 { "" } else { "ffi" }), 20.0) + ell + 0.5;
        let mut options = TextLayoutOptions::wrapped(budget, 24.0, 24.0);
        options.wrap = false;
        options.max_lines = Some(1);
        options.overflow = TextOverflow::Ellipsis;
        let layout = engine.layout_box(text, 20.0, options);
        let content = &layout.lines[0].content;
        let body = content.trim_end_matches('…');
        assert!(
            body == "o" || body == "offi" || body.is_empty(),
            "k={k}：省略号前不得切一半连字，实际 {body:?}"
        );
    }
}

/// 排版缓存命中路径的耗时量级（K3 性能报告）。缓存本身在本件未改（`layout_box`
/// 命中仍是「加锁 + 哈希查找 + Arc 克隆」），此测试打印冷 / 热耗时供人核对命中确有增益。
#[test]
fn layout_cache_hit_timing_report() {
    use std::time::Instant;
    let Some(engine) = engine() else { return };
    let size = 14.0;
    let options = TextLayoutOptions::wrapped(200.0, 40.0, 20.0);

    // 冷：每串一次未命中（首帧场景）。
    let cold = Instant::now();
    for text in SETTINGS_GOLDEN_STRINGS {
        let _ = engine.layout_box(text, size, options);
    }
    let cold = cold.elapsed();

    // 热：已全部入缓存，100 轮命中。
    let warm = Instant::now();
    for _ in 0..100 {
        for text in SETTINGS_GOLDEN_STRINGS {
            let _ = engine.layout_box(text, size, options);
        }
    }
    let warm = warm.elapsed() / 100;
    println!(
        "[k3-perf] Settings 20 串：冷排版 {cold:?}（{:.1} µs/串），缓存命中 {warm:?}（{:.2} µs/串）",
        cold.as_secs_f64() * 1e6 / SETTINGS_GOLDEN_STRINGS.len() as f64,
        warm.as_secs_f64() * 1e6 / SETTINGS_GOLDEN_STRINGS.len() as f64,
    );
    assert!(warm < cold, "缓存命中应显著快于冷排版");
}

/// 带 `liga` 连字的拉丁字体（`ffi` / `fl`）。测试机常见路径，缺失则跳过。
fn find_ligature_font() -> Option<std::path::PathBuf> {
    [
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|p| p.exists())
}

/// 会话字体链唯一真源（集成裁决 2026-10-31）：链非空、每条都存在、
/// 主字面 == 主候选链首个存在项、canonical 无重复。
#[test]
fn system_font_chain_matches_candidates() {
    let chain = system_font_chain();
    assert!(!chain.is_empty(), "真机必有字体（链不得为空）");
    let first = REGULAR_CANDIDATES
        .iter()
        .map(|(path, tag)| FontSource { path: PathBuf::from(path), collection_tag: *tag })
        .find(|s| s.path.exists())
        .expect("同上");
    assert_eq!(chain[0].path, first.path, "主字面必须是首个存在候选");
    assert_eq!(chain[0].collection_tag, first.collection_tag);
    let mut seen = std::collections::HashSet::new();
    for s in &chain {
        assert!(s.path.exists(), "链只含存在路径: {}", s.path.display());
        assert!(
            seen.insert(s.path.canonicalize().unwrap_or_else(|_| s.path.clone())),
            "canonical 重复: {}",
            s.path.display()
        );
    }
    // 主字面为 VF 时不得出现静态字重字面（语义与 extra_sources 一致）。
    let primary_is_vf = chain[0]
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains("-VF"));
    if primary_is_vf {
        for s in &chain[1..] {
            let n = s.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            assert!(
                !["Light", "Medium", "Bold"].iter().any(|w| n.contains(&format!("-{w}"))),
                "VF 主字面时不得有静态字重字面: {n}"
            );
        }
    }
}
    // ---- A3.3：索引覆盖查询（KANESUMI_FONT_INDEX=1；夹具经 A3.0 写侧构造）----

    /// 判门机 / 本云同款 Noto CJK TTC（真机栈）；无则按既有约定跳过。
    fn cjk_font() -> Option<std::path::PathBuf> {
        [
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
    }

    /// 夹具：A3.0 写侧 `write_index` 构造引擎真实栈的索引 → 临时文件 →
    /// `OwnedIndex::open`（走真实读路径）。`perturb` 翻转首字面首字节，
    /// 使 `stack_identity` 必然与栈失配（有效性闸的判别样本）。
    fn fixture_index(engine: &TextEngine, perturb: bool) -> Option<crate::font_index::OwnedIndex> {
        let mut specs = Vec::new();
        let mut covs = Vec::new();
        let mut bytes: Vec<Vec<u8>> = Vec::new();
        for font in engine.fonts.iter() {
            let face = ttf_parser::Face::parse(font.data(), font.collection_index).ok()?;
            covs.push(CoverageTable::from_face(&face));
            let mut data = font.data().to_vec();
            if perturb {
                data[0] ^= 0xA5;
            }
            bytes.push(data);
        }
        for (font, cov) in engine.fonts.iter().zip(covs.iter()) {
            specs.push(crate::font_index::IndexEntrySpec {
                path: "fixture.ttf",
                family: "Fixture",
                collection_index: u16::try_from(font.collection_index).ok()?,
                weight: font.weight,
                slant: 0,
                flags: 0b100,
                ttc_faces: 1,
                coverage: cov.ranges(),
                wght_min: 0.0,
                wght_max: 0.0,
                mtime_ns: 0,
            });
        }
        let refs: Vec<&[u8]> = bytes.iter().map(|b| b.as_slice()).collect();
        let file = crate::font_index::write_index(&specs, &refs, &[], 1, 0, 0b0011);
        // 目录名带调用原子序号：旧实现只含 pid，同进程并行用例全部落在同一路径上
        // 互相覆写与提前 remove_file（50 轮 × 16 线程实测 9 次失败，均为
        // 「夹具不可构造」；01 交件报告的偶发 2 例即此竞态）。
        static FIXTURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "a33-{}-{}",
            std::process::id(),
            FIXTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).ok()?;
        let path = dir.join("fonts-v1.idx");
        std::fs::write(&path, &file).ok()?;
        let idx = crate::font_index::OwnedIndex::open(&path).ok();
        let _ = std::fs::remove_dir_all(&dir);
        idx
    }

    const PARAGRAPH: &str = "字体索引覆盖查询：Ether 123abc，常用汉字集。";

    /// A3.3 门：索引开且有效 → 同一 CJK 段落逐字素选择与旧路径一致，
    /// 且覆盖查询全程 `Face::parse` 计数增量 = 0。
    #[test]
    fn indexed_cjk_paragraph_zero_face_parse_and_matches_legacy() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        assert_eq!(
            idx.header().stack_identity,
            engine.identity(),
            "前置：夹具身份必须与栈一致"
        );
        let before = face_parse_calls();
        for g in PARAGRAPH.graphemes(true) {
            assert_eq!(
                engine.font_for_grapheme_with(Some(&idx), g),
                engine.font_for_grapheme_with(None, g),
                "字素 {g:?} 索引选择必须与旧路径一致"
            );
        }
        assert_eq!(face_parse_calls() - before, 0, "索引覆盖查询不得触发 Face::parse");
    }

    /// A3.3 门：身份失配的索引必须整体作废 → 完全走旧路径（逐字素一致）。
    #[test]
    fn identity_mismatched_index_falls_back_to_legacy_per_grapheme() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(bad) = fixture_index(&engine, true) else {
            panic!("扰动夹具必须可构造");
        };
        assert_ne!(
            bad.header().stack_identity,
            engine.identity(),
            "前置：扰动后身份必须失配"
        );
        for g in PARAGRAPH.graphemes(true) {
            assert_eq!(
                engine.font_for_grapheme_with(Some(&bad), g),
                engine.font_for_grapheme_with(None, g),
                "失配索引必须回落旧路径"
            );
        }
    }

    /// has_glyph 索引区段与内存覆盖表同值（CJK 覆盖 / 拉丁 / PUA 缺字 / 空白）。
    #[test]
    fn has_glyph_via_index_matches_legacy_table() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        for font in engine.fonts.iter() {
            let r = index_entry_coverage_in(&idx, font.data(), font.collection_index)
                .expect("字面必须命中索引条目");
            for &cp in &[0x4E00_u32, 0x9FA5, 0x0041, 0x00E9, 0xE000, 0x1F600, 0x0020] {
                let Some(c) = char::from_u32(cp) else { continue };
                assert_eq!(
                    r.has(c),
                    font.has_glyph(c),
                    "U+{cp:04X} 索引与内存覆盖表必须同值"
                );
            }
        }
    }

    /// A3.1 门：索引开关开且条目按 `content_hash + collection_index` 命中时，
    /// 载入**不建**覆盖表（线程内建表计数增量 = 0），且运行期查询走索引区段、
    /// 与旧 `glyph_index` 同值——不建表不等于查不到。
    #[test]
    fn index_hit_skips_coverage_table_build_at_load() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        let font = &engine.fonts[0];
        let before = coverage_builds_on_thread();
        let loaded =
            FontFace::from_bytes_with_index(font.bytes.clone(), font.collection_index, Some(&idx))
                .expect("命中索引的字面必须可加载");
        assert_eq!(
            coverage_builds_on_thread() - before,
            0,
            "索引命中时载入期不得建覆盖表"
        );
        assert!(
            loaded.coverage.get().is_none(),
            "载入后覆盖表必须保持未初始化"
        );

        let ranges = index_entry_coverage_in(&idx, loaded.data(), loaded.collection_index)
            .expect("字面必须命中索引条目");
        for c in ['A', '中', '一', '\u{E000}', '\u{10FFFF}'] {
            let via_index = loaded.has_glyph_with(Some(ranges), c);
            assert_eq!(via_index, ranges.has(c), "索引查询必须走区段");
            assert_eq!(
                via_index,
                ttf_parser::Face::parse(loaded.data(), loaded.collection_index)
                    .expect("加载时已验证")
                    .glyph_index(c)
                    .is_some(),
                "U+{:04X} 索引查询必须与旧 glyph_index 同值",
                c as u32
            );
        }
        assert!(
            loaded.coverage.get().is_none(),
            "索引查询不得顺带建表（否则惰性化失效）"
        );
    }

    /// A3.1 门：诊断（`coverage_stats`）首次调用才建表，且数值与旧路径建表逐项相同；
    /// 二次调用复用已建表，不再计建表。
    #[test]
    fn diagnostics_build_lazily_with_legacy_numbers() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        let font = &engine.fonts[0];
        let loaded =
            FontFace::from_bytes_with_index(font.bytes.clone(), font.collection_index, Some(&idx))
                .expect("命中索引的字面必须可加载");
        assert!(loaded.coverage.get().is_none());
        // 旧路径参照：同一 `from_face` 独立建一份表。
        let face = ttf_parser::Face::parse(font.data(), font.collection_index)
            .expect("加载时已验证");
        let legacy = CoverageTable::from_face(&face);
        let engine = TextEngine {
            fonts: Arc::new(vec![loaded]),
            identity: 0,
            shape_cache: Arc::new(Mutex::new(HashMap::new())),
            layout_cache: Arc::new(Mutex::new(HashMap::new())),
        };
        let before = coverage_builds_on_thread();
        let stats = engine.diag_coverage_stats();
        assert_eq!(
            coverage_builds_on_thread() - before,
            1,
            "诊断首次调用必须恰好建表一次"
        );
        assert_eq!(stats.len(), 1);
        assert_eq!(stats[0].ranges, legacy.ranges().len(), "区间数必须同值");
        assert_eq!(
            stats[0].codepoints,
            legacy.codepoints(),
            "覆盖码点数必须同值"
        );
        assert_eq!(
            stats[0].memory_bytes,
            legacy.memory_bytes(),
            "表内存必须同值"
        );
        let before = coverage_builds_on_thread();
        let _ = engine.diag_coverage_stats();
        assert_eq!(
            coverage_builds_on_thread() - before,
            0,
            "二次诊断不得重复建表"
        );
    }

    /// A3.1 真机路径：显式设 `KANESUMI_FONT_INDEX=1` 且
    /// `$XDG_RUNTIME_DIR/kanesumi/fonts-v1.idx` 存在时，整栈载入不建覆盖表，
    /// 诊断首次调用为每个未建表的覆盖句柄恰补建一次。常规 `cargo test`（未设开关）
    /// 直接跳过；手动验法：`KANESUMI_FONT_INDEX=1 XDG_RUNTIME_DIR=<含索引的目录>
    /// cargo test -p kanesumi-canvas real_env_index -- --exact`。
    #[test]
    fn real_env_index_hit_skips_coverage_build() {
        if !font_index_switch() || shared_index().is_none() {
            eprintln!("跳过：未开 KANESUMI_FONT_INDEX=1 或无索引文件（注入路径已覆盖）");
            return;
        }
        let chain = system_font_chain();
        let (primary, extra) = chain.split_first().expect("真机字体链不得为空");
        let before = coverage_builds_on_thread();
        let engine = TextEngine::load_stack(primary, extra).expect("真机字体栈必须可加载");
        assert_eq!(
            coverage_builds_on_thread() - before,
            0,
            "索引命中时整栈载入不得建覆盖表"
        );
        assert_eq!(
            shared_index().expect("前置已判有索引").header().stack_identity,
            engine.identity(),
            "前置：索引身份必须与栈一致（否则本用例没走到命中路径）"
        );
        // 诊断是首次「非索引用途」：每个未建表的覆盖句柄恰好补建一次。同源字重
        // 实例共享同一 `Arc<OnceLock<..>>` 句柄（`base.clone()`），共享组只建一次——
        // 真机 VF 主栈（7 实例同文件）建表数 = 去重句柄数，不等于 font_count。
        let mut handles: Vec<usize> = engine
            .fonts
            .iter()
            .filter(|f| f.coverage.get().is_none())
            .map(|f| Arc::as_ptr(&f.coverage) as usize)
            .collect();
        handles.sort_unstable();
        handles.dedup();
        assert!(!handles.is_empty(), "真机字体栈不得为空");
        let before = coverage_builds_on_thread();
        let stats = engine.diag_coverage_stats();
        assert_eq!(
            coverage_builds_on_thread() - before,
            handles.len() as u64,
            "诊断必须为每个未建表的覆盖句柄恰建一次（共享实例不重复计）"
        );
        assert!(
            stats.iter().all(|s| s.codepoints > 0),
            "诊断数值不得为空（与旧路径同口径）"
        );
    }

    /// A3.1 门：同一字面索引 `has_glyph` 与旧内存表**逐点**同值（~2600 码点，
    /// 覆盖 CJK / 拉丁 / 标点 / PUA / emoji / 码点上界 / U+0000）。
    #[test]
    fn index_has_glyph_matches_legacy_table_pointwise() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        let mut cps: Vec<u32> = Vec::new();
        cps.extend((0x4E00u32..0x9FFF).step_by(2).take(2000));
        cps.extend(0x20u32..0x7F);
        cps.extend(0x3000u32..0x303F);
        cps.extend((0xE000u32..0xF8FF).step_by(13));
        cps.extend([0x0, 0x1F600, 0x10FFFF]);
        cps.sort_unstable();
        cps.dedup();
        for font in engine.fonts.iter() {
            let ranges = index_entry_coverage_in(&idx, font.data(), font.collection_index)
                .expect("字面必须命中索引条目");
            let face = ttf_parser::Face::parse(font.data(), font.collection_index)
                .expect("加载时已验证");
            // 旧路径的载入期内存表：同一 `from_face`，独立建一份作参照。
            let legacy = CoverageTable::from_face(&face);
            for &cp in &cps {
                let Some(c) = char::from_u32(cp) else { continue };
                assert_eq!(
                    font.has_glyph_with(Some(ranges), c),
                    legacy.has(c),
                    "U+{cp:04X} 索引查询与旧内存表必须同值"
                );
            }
        }
    }

    /// A3.1 门：索引缺失 / 损坏 → 建表计数恢复（每字面载入期 1 次），查询回旧内存表。
    #[test]
    fn missing_or_corrupt_index_restores_load_time_coverage_build() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let font = &engine.fonts[0];

        // 缺失：没有索引可注入 → 旧路径载入期建表。
        let before = coverage_builds_on_thread();
        let loaded = FontFace::from_bytes_with_index(font.bytes.clone(), font.collection_index, None)
            .expect("无索引时必须可加载");
        assert_eq!(
            coverage_builds_on_thread() - before,
            1,
            "无索引时载入期必须建表一次"
        );
        assert!(loaded.coverage.get().is_some(), "旧路径载入后表必须已建");

        // 损坏：真实读侧拒绝坏索引 → 调用方拿不到索引 → 同上回落。
        // 同一竞态面：目录名带调用原子序号，避免与其他并行用例共享路径；用毕清目录。
        static CORRUPT_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "a31-corrupt-{}-{}",
            std::process::id(),
            CORRUPT_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("临时目录");
        let bad_path = dir.join("fonts-v1.idx");
        std::fs::write(&bad_path, b"not an index").expect("写坏索引");
        let bad_idx = crate::font_index::OwnedIndex::open(&bad_path);
        assert!(bad_idx.is_err(), "损坏索引必须被读侧拒绝");
        let before = coverage_builds_on_thread();
        let loaded = FontFace::from_bytes_with_index(
            font.bytes.clone(),
            font.collection_index,
            bad_idx.as_ref().ok(),
        )
        .expect("损坏索引必须回落旧路径");
        assert_eq!(
            coverage_builds_on_thread() - before,
            1,
            "损坏索引时载入期建表计数必须恢复"
        );
        let face = ttf_parser::Face::parse(font.data(), font.collection_index)
            .expect("加载时已验证");
        let legacy = CoverageTable::from_face(&face);
        for c in ['A', '中', '\u{E000}', '\u{10FFFF}'] {
            assert_eq!(
                loaded.has_glyph_with(None, c),
                legacy.has(c),
                "损坏索引必须回旧路径"
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ---- A3 身份采信门（裁定 (a)，§7 客户端采信条款回归）----

    /// 全命中采信：采信值 == 自算值 == 索引 stack_identity（三者逐位相等），
    /// 且采信尝试把 content_hash 预置齐（同源实例共享句柄）。
    #[test]
    fn adopted_identity_equals_self_computed_on_full_hit() {
        let Some(path) = cjk_font() else { return };
        let mut engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(idx) = fixture_index(&engine, false) else {
            panic!("夹具索引必须可构造");
        };
        let self_computed = engine.identity();
        assert_eq!(
            self_computed,
            idx.header().stack_identity,
            "前置：夹具索引身份必须与自算一致"
        );
        assert!(engine.adopt_identity_with(&idx), "全命中必须采信");
        assert_eq!(engine.identity(), self_computed, "采信值必须与自算值逐位相等");
        assert_eq!(
            engine.identity(),
            idx.header().stack_identity,
            "采信值必须等于索引 stack_identity"
        );
        assert!(
            engine.fonts.iter().all(|f| f.content_hash.get().is_some()),
            "采信尝试必须把各源 content_hash 预置齐"
        );
    }

    /// 任一字面 content 不中（扰动夹具）→ 拒绝采信，身份保持自算值不变；
    /// 已预置的 content_hash 留用，不改变回退值。
    #[test]
    fn content_mismatch_falls_back_to_self_computed_identity() {
        let Some(path) = cjk_font() else { return };
        let mut engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let Some(bad) = fixture_index(&engine, true) else {
            panic!("扰动夹具必须可构造");
        };
        let self_computed = engine.identity();
        assert_ne!(
            self_computed,
            bad.header().stack_identity,
            "前置：扰动索引身份必须与自算不同"
        );
        assert!(!engine.adopt_identity_with(&bad), "content 不中必须拒绝采信");
        assert_eq!(engine.identity(), self_computed, "拒绝采信后身份必须保持自算值");
        assert!(
            engine.fonts.iter().all(|f| f.content_hash.get().is_some()),
            "采信尝试预置的 content_hash 留用"
        );
    }

    /// 缺省关档（KANESUMI_FONT_INDEX 未设）：身份与旧逐字节实现逐位相等，
    /// 且不付 content 趟（采信路径完全不触达）。
    #[test]
    fn off_identity_matches_legacy_bytewise_replica() {
        let Some(path) = cjk_font() else { return };
        let engine = TextEngine::load(path).expect("Noto CJK 必须可加载");
        let mut hash = 0xcbf29ce484222325_u64;
        for font in engine.fonts.iter() {
            for byte in font.data().iter() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
            }
            hash ^= u64::from(font.collection_index);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        assert_eq!(engine.identity(), hash, "off 档身份必须与旧逐字节实现逐位相等");
        assert!(
            engine.fonts.iter().all(|f| f.content_hash.get().is_none()),
            "off 档不付 content 趟"
        );
    }

    // ---- A3 载入剩余成本分布探针（第 2 卡：只量不修，不在产品路径）----
    //
    // 五段口径（段值除 coverage 外为同字节同原语复刻计时，读后热字节，误差二阶）：
    //   io_ms       真实 fs::read（load_stack_with 计时读取闭包累计）
    //   parse_ms    载入期解析：tag 扫描（collection_index_matching）+ 校验 parse +
    //               rustybuzz from_slice + 轴探测 parse，与 face_parse_calls 增量对账
    //   coverage_ms 覆盖表建表：off = Σ 覆盖句柄 build_us（产品内部实测）；on = diag_ms
    //              （惰性建表窗口实测，含建表内的一次 Face::parse）
    //   identity_ms 身份 FNV：栈哈希 + 索引开时的 content_hash（单趟双累加复刻，
    //               与 engine.identity() 逐位对账）
    //   index_ms    索引查找（仅 on）：index_valid 判定 + 按预置 content_hash 的条目匹配
    //   other_ms    load_ms − io − parse − identity − index（装配 / 缓存 / 分配等余量）
    // 窗口：off = load_ms；on = load_ms + diag_ms（惰性建表在首次诊断触发）。
    // 无 KANESUMI_LOAD_COST_CSV 时直接跳过，不影响常规 cargo test；off/on 为进程级
    // 开关，不可同进程隔离，由外层分档各跑 5 次（每次独立进程 = 真实冷载入）。
    #[test]
    fn load_cost_probe_once() {
        let Some(csv) = std::env::var_os("KANESUMI_LOAD_COST_CSV") else { return };
        let run = std::env::var("KANESUMI_LOAD_COST_RUN").unwrap_or_else(|_| "0".into());
        let chain = system_font_chain();
        if chain.is_empty() {
            eprintln!("跳过：会话字体链为空");
            return;
        }
        let tag_of: HashMap<PathBuf, Option<&'static str>> =
            chain.iter().map(|s| (s.path.clone(), s.collection_tag)).collect();
        // 真实载入：读取闭包只计时 IO 并留字节；tag 匹配按产品语义发生在 faces_from_source，
        // 复刻段再计，避免双重计时。
        let io = std::cell::Cell::new(std::time::Duration::ZERO);
        let loaded: std::cell::RefCell<Vec<(PathBuf, FontBytes)>> = Default::default();
        let read = |p: &Path| -> std::io::Result<FontBytes> {
            let t = std::time::Instant::now();
            let bytes: FontBytes = Arc::new(std::fs::read(p)?);
            io.set(io.get() + t.elapsed());
            loaded.borrow_mut().push((p.to_path_buf(), bytes.clone()));
            Ok(bytes)
        };
        let (primary, extra) = chain.split_first().expect("链非空已判");
        let parses_before = face_parse_calls();
        let builds_before = coverage_builds();
        let t0 = std::time::Instant::now();
        let engine =
            TextEngine::load_stack_with(primary, extra, &read).expect("探针字体栈必须可加载");
        let load_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let idx = if font_index_switch() { shared_index() } else { None };
        // diag 前先记未建句柄数（on 档对账：惰性建表应恰好每未建句柄一次；部分命中时
        // resolve 已在载入内建掉一部分，不为零也不能误断）。
        let mut unset_handles: Vec<usize> = Vec::new();
        for font in engine.fonts.iter() {
            if font.coverage.get().is_none() {
                let h = Arc::as_ptr(&font.coverage) as usize;
                if !unset_handles.contains(&h) {
                    unset_handles.push(h);
                }
            }
        }
        // 诊断触发 on 档惰性建表（off 档零建表、diag 近零成本）。
        let td = std::time::Instant::now();
        let _ = engine.diag_coverage_stats();
        let diag_ms = td.elapsed().as_secs_f64() * 1000.0;
        let parse_delta = face_parse_calls() - parses_before;
        let builds_delta = coverage_builds() - builds_before;
        // coverage 段：off 按覆盖句柄去重取内部实测 build_us；on 取 diag 窗口实测。
        let coverage_ms = if idx.is_some() {
            diag_ms
        } else {
            let mut seen: Vec<usize> = Vec::new();
            let mut sum = 0.0f64;
            for font in engine.fonts.iter() {
                let h = Arc::as_ptr(&font.coverage) as usize;
                if seen.contains(&h) {
                    continue;
                }
                seen.push(h);
                sum += font.coverage_table().build_us as f64 / 1000.0;
            }
            sum
        };
        // identity 段：单趟双累加复刻（fresh 字面 = 栈哈希 + content 哈希同趟），逐位对账。
        let t = std::time::Instant::now();
        let mut hash = 0xcbf29ce484222325_u64;
        let mut seen: Vec<usize> = Vec::new();
        for font in engine.fonts.iter() {
            let h = Arc::as_ptr(&font.content_hash) as usize;
            let fresh = idx.is_some() && !seen.contains(&h);
            if fresh {
                seen.push(h);
            }
            let mut content = 0xcbf29ce484222325_u64;
            for byte in font.data().iter() {
                hash ^= u64::from(*byte);
                hash = hash.wrapping_mul(0x100000001b3);
                if fresh {
                    content ^= u64::from(*byte);
                    content = content.wrapping_mul(0x100000001b3);
                }
            }
            if fresh {
                assert_eq!(
                    Some(content),
                    font.content_hash.get().copied(),
                    "复刻 content_hash 必须与产品值一致"
                );
            }
            hash ^= u64::from(font.collection_index);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        assert_eq!(hash, engine.identity(), "复刻栈身份必须与产品一致");
        let identity_ms = t.elapsed().as_secs_f64() * 1000.0;
        // index 段（仅 on）：有效性判定 + 全字面条目匹配（与 resolve_index_coverage 同形）。
        let mut index_ms = 0.0f64;
        let mut index_valid = false;
        if let Some(idx) = idx {
            let t = std::time::Instant::now();
            index_valid = idx.header().stack_identity == engine.identity()
                && usize::try_from(idx.header().font_count).ok() == Some(engine.font_count());
            for font in engine.fonts.iter() {
                if let Some(h) = font.content_hash.get().copied() {
                    let _ = index_entry_coverage_by_hash(idx, h, font.collection_index);
                }
            }
            index_ms = t.elapsed().as_secs_f64() * 1000.0;
        }
        // parse 段：同字节同原语复刻（tag 扫描 + 校验 + rustybuzz + 轴探测；VF 实例克隆不
        // parse）。复刻自身也走 count_face_parse，故段内计数取进程计数器差值——tag 扫描
        // 的尝试次数不可手数，进程计数是最准的对账口径。
        let parses_before_replica = face_parse_calls();
        let t = std::time::Instant::now();
        for (path, bytes) in loaded.borrow().iter() {
            let data: &[u8] = bytes.as_ref().as_ref();
            let ci = tag_of
                .get(path)
                .cloned()
                .flatten()
                .and_then(|tag| collection_index_matching(data, tag))
                .unwrap_or(0);
            let face = ttf_parser::Face::parse(data, ci).expect("复刻 parse 必须成功");
            let _weight = face.weight();
            let _ = rustybuzz::Face::from_slice(data, ci);
            let probe = ttf_parser::Face::parse(data, ci).expect("复刻轴探测必须成功");
            let _axes = probe.variation_axes().into_iter().count();
        }
        let parse_ms = t.elapsed().as_secs_f64() * 1000.0;
        let replica_parses = face_parse_calls() - parses_before_replica;
        // 对账：产品载入（含 on 档惰性建表）的 parse 增量 == 复刻增量 + 2×源数（复刻的
        // 校验/轴探测为直接调用、不经 count_face_parse，只有 tag 扫描进进程计数）
        // + on 档建表 parse。不等只提示不中止——测量不因口径漂移而死，偏差写进 CSV。
        let expected_parses = replica_parses
            + 2 * loaded.borrow().len() as u64
            + if idx.is_some() { builds_delta } else { 0 };
        if parse_delta != expected_parses {
            eprintln!(
                "load_cost_probe 对账偏差: parse_delta={parse_delta} expected={expected_parses}"
            );
        }
        let other_ms =
            (load_ms - io.get().as_secs_f64() * 1000.0 - parse_ms - identity_ms - index_ms).max(0.0);
        let window_ms = load_ms + if idx.is_some() { diag_ms } else { 0.0 };
        if idx.is_some() {
            // on 档 coverage 段已取 diag_ms，这里仅对账：建表次数 = diag 前未建句柄数。
            assert_eq!(
                builds_delta,
                unset_handles.len() as u64,
                "on 档诊断必须每个未建句柄恰建表一次"
            );
        }
        // CSV 追加一行；首行写表头。持久路径由调用方经环境变量给到本槽 HOME。
        let write_header = !std::path::Path::new(&csv).exists();
        if let Some(dir) = std::path::Path::new(&csv).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&csv) {
            if write_header {
                let _ = writeln!(f, "mode,run,sources,font_count,index_valid,window_ms,load_ms,diag_ms,io_ms,parse_ms,coverage_ms,identity_ms,index_ms,other_ms,parse_delta,replica_parses,builds_delta");
            }
            let _ = writeln!(
                f,
                "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{}",
                if idx.is_some() { "on" } else { "off" },
                run,
                chain.len(),
                engine.font_count(),
                index_valid,
                window_ms,
                load_ms,
                diag_ms,
                io.get().as_secs_f64() * 1000.0,
                parse_ms,
                coverage_ms,
                identity_ms,
                index_ms,
                other_ms,
                parse_delta,
                replica_parses,
                builds_delta
            );
        }
        println!(
            "load_cost_probe mode={} run={run} window_ms={window_ms:.3} io={:.3} parse={parse_ms:.3} coverage={coverage_ms:.3} identity={identity_ms:.3} index={index_ms:.3} other={other_ms:.3}",
            if idx.is_some() { "on" } else { "off" },
            io.get().as_secs_f64() * 1000.0
        );
    }

    // ---- A3 身份哈希候选探针辅助（第 3 卡，只量不修；仅测试区）----
    //
    // b1：u64 分块 FNV 变体——小端 8 字节一块，h = (h ^ w) * FNV_PRIME，尾块逐字节，
    //     末尾一次雪崩（xorshift + 奇乘）把尾块差分扩散到全宽。吞吐收益来自每 8 字节
    //     一次乘法（原 FNV-1a 每字节一次）。
    fn fnv_chunked(data: &[u8]) -> u64 {
        const PRIME: u64 = 0x100000001b3;
        let mut h = 0xcbf29ce484222325_u64;
        let mut chunks = data.chunks_exact(8);
        for c in &mut chunks {
            let mut w = [0u8; 8];
            w.copy_from_slice(c);
            h = (h ^ u64::from_le_bytes(w)).wrapping_mul(PRIME);
        }
        for &b in chunks.remainder() {
            h ^= u64::from(b);
            h = h.wrapping_mul(PRIME);
        }
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h
    }

    // b2：8 路并行乘法混合——64 字节一轮、每路独立素数（8 条互不依赖的乘法链，
    //     摊平串行 FNV 的乘法延迟），尾块按路逐字节，末尾雪崩合并。
    fn mix8(data: &[u8]) -> u64 {
        const P: [u64; 8] = [
            0x100000001b3,
            0x9e3779b97f4a7c15,
            0xff51afd7ed558ccd,
            0xc4ceb9fe1a85ec53,
            0x2545f4914f6cdd1d,
            0x9e3779b185ebca87,
            0xc2b2ae3d27d4eb4f,
            0x165667b19e3779f9,
        ];
        let mut h = [0xcbf29ce484222325_u64; 8];
        let mut chunks = data.chunks_exact(64);
        for c in &mut chunks {
            for (k, lane) in h.iter_mut().enumerate() {
                let mut w = [0u8; 8];
                w.copy_from_slice(&c[k * 8..k * 8 + 8]);
                *lane = (*lane ^ u64::from_le_bytes(w)).wrapping_mul(P[k]);
            }
        }
        for (i, &b) in chunks.remainder().iter().enumerate() {
            let k = i % 8;
            h[k] ^= u64::from(b) << (((i / 8) % 8) * 8);
            h[k] = h[k].wrapping_mul(P[k]);
        }
        let mut acc = 0xcbf29ce484222325_u64;
        for (k, lane) in h.iter().enumerate() {
            acc = (acc ^ lane.rotate_left((k * 9) as u32)).wrapping_mul(P[k]);
        }
        acc ^= acc >> 33;
        acc = acc.wrapping_mul(0xff51afd7ed558ccd);
        acc ^= acc >> 29;
        acc
    }

    // ---- A3 身份哈希候选探针（第 3 卡：只量不修，裁定材料；不在产品路径）----
    //
    // 候选（口径沿 report-2：release off/on 各 5 次独立进程）：
    //   cur  现行 r3 身份遍历复刻（首字面单累加复用 + 其余双累加，栈序混 collection_index）
    //   a    全命中采信：不累栈哈希，逐源 content FNV；on 档与索引条目「按位」比对
    //        （§3 font_id==下标），全对上则身份取 idx.header().stack_identity；off 档
    //        无索引，采信不适用，仅测 content 趟成本
    //   b1   u64 分块 FNV 变体（fnv_chunked）替代字节主体
    //   b2   8 路并行乘法混合（mix8）替代字节主体
    // 各候选同字节计时；cur 与 engine.identity() / content_hash 逐位对账。
    #[test]
    fn identity_candidates_probe_once() {
        let Some(csv) = std::env::var_os("KANESUMI_IDCAND_CSV") else { return };
        let run = std::env::var("KANESUMI_IDCAND_RUN").unwrap_or_else(|_| "0".into());
        let chain = system_font_chain();
        if chain.is_empty() {
            eprintln!("跳过：会话字体链为空");
            return;
        }
        let io = std::cell::Cell::new(std::time::Duration::ZERO);
        let read = |p: &Path| -> std::io::Result<FontBytes> {
            let t = std::time::Instant::now();
            let bytes: FontBytes = Arc::new(std::fs::read(p)?);
            io.set(io.get() + t.elapsed());
            Ok(bytes)
        };
        let (primary, extra) = chain.split_first().expect("链非空已判");
        let engine =
            TextEngine::load_stack_with(primary, extra, &read).expect("探针字体栈必须可加载");
        let idx = if font_index_switch() { shared_index() } else { None };
        let bytes_total: usize = engine.fonts.iter().map(|f| f.data().len()).sum();
        // cur：现行身份遍历复刻（与 refresh_identity 逐分支同形，含首字面复用）。
        let t = std::time::Instant::now();
        let mut hash = 0xcbf29ce484222325_u64;
        for (i, font) in engine.fonts.iter().enumerate() {
            let fresh = idx.is_some() && font.content_hash.get().is_none();
            if fresh && i == 0 {
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            } else if fresh {
                let mut content = 0xcbf29ce484222325_u64;
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                    content ^= u64::from(*byte);
                    content = content.wrapping_mul(0x100000001b3);
                }
                assert_eq!(Some(content), font.content_hash.get().copied(), "复刻 content 必须同值");
            } else {
                for byte in font.data().iter() {
                    hash ^= u64::from(*byte);
                    hash = hash.wrapping_mul(0x100000001b3);
                }
            }
            hash ^= u64::from(font.collection_index);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        assert_eq!(hash, engine.identity(), "cur 复刻必须与产品身份逐位一致");
        let cur_ms = t.elapsed().as_secs_f64() * 1000.0;
        // a：逐源 content 趟（不累栈）；同源字面共享句柄只扫一趟（VF 栈收益主体）。
        let t = std::time::Instant::now();
        let mut content_by_handle: Vec<(usize, u64)> = Vec::new();
        for font in engine.fonts.iter() {
            let h = Arc::as_ptr(&font.content_hash) as usize;
            if !content_by_handle.iter().any(|(k, _)| *k == h) {
                content_by_handle.push((h, crate::font_index::fnv1a64(font.data())));
            }
        }
        let a_ms = t.elapsed().as_secs_f64() * 1000.0;
        let content_of = |font: &FontFace| -> u64 {
            let h = Arc::as_ptr(&font.content_hash) as usize;
            content_by_handle.iter().find(|(k, _)| *k == h).map(|(_, v)| *v).unwrap_or(0)
        };
        // a 采信判定（on）：font_count 对位 + 逐位 content 比对（§3 位置即身份的一半）。
        let mut a_val: u64 = 0;
        let mut a_match = false;
        if let Some(idx) = idx {
            let n = usize::try_from(idx.header().font_count).unwrap_or(0);
            a_match = n == engine.fonts.len()
                && engine
                    .fonts
                    .iter()
                    .enumerate()
                    .all(|(i, font)| match idx.entry(i) {
                        Some(e) => e.content_hash == content_of(font)
                            && u32::from(e.collection_index) == font.collection_index,
                        None => false,
                    });
            if a_match {
                a_val = idx.header().stack_identity;
            }
        }
        // b1/b2：栈序逐 entry 换宽哈希核（content 候选值 = 单文件哈希），混入 collection_index。
        let t = std::time::Instant::now();
        let mut s1 = 0xcbf29ce484222325_u64;
        for font in engine.fonts.iter() {
            s1 = (s1 ^ fnv_chunked(font.data())).wrapping_mul(0x100000001b3);
            s1 = (s1 ^ u64::from(font.collection_index)).wrapping_mul(0x100000001b3);
        }
        let b1_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = std::time::Instant::now();
        let mut s2 = 0xcbf29ce484222325_u64;
        for font in engine.fonts.iter() {
            s2 = (s2 ^ mix8(font.data())).wrapping_mul(0x100000001b3);
            s2 = (s2 ^ u64::from(font.collection_index)).wrapping_mul(0x100000001b3);
        }
        let b2_ms = t.elapsed().as_secs_f64() * 1000.0;
        let mode = if idx.is_some() { "on" } else { "off" };
        let write_header = !std::path::Path::new(&csv).exists();
        if let Some(dir) = std::path::Path::new(&csv).parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        use std::io::Write as _;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&csv) {
            if write_header {
                let _ = writeln!(f, "mode,run,sources,font_count,bytes_total,io_ms,cur_ms,a_ms,b1_ms,b2_ms,cur_val,a_val,a_match,b1_val,b2_val");
            }
            let _ = writeln!(
                f,
                "{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{}",
                mode,
                run,
                chain.len(),
                engine.font_count(),
                bytes_total,
                io.get().as_secs_f64() * 1000.0,
                cur_ms,
                a_ms,
                b1_ms,
                b2_ms,
                hash,
                a_val,
                a_match,
                s1,
                s2
            );
        }
        println!(
            "id_cand mode={mode} run={run} bytes={bytes_total} io={:.3} cur={cur_ms:.3} a={a_ms:.3} b1={b1_ms:.3} b2={b2_ms:.3} a_match={a_match}",
            io.get().as_secs_f64() * 1000.0
        );
    }
}
