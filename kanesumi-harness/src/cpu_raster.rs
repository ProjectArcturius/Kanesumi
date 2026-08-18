// cpu_raster.rs — CpuRenderer：把 Scene 直接用 CPU 光栅化进 SHM 像素缓冲。
// 参 settings/docs/TOPBAR_RENDER_REFACTOR.md §4（TopBar 归 settings + CPU 光栅化 + 按需提交）。
//
// 为什么没有 GPU：旧「离屏 wgpu → 读回 → wl_shm」三方握手在 4 个关节上静默断裂
// （不渲染 / 静态 / 不可交互 / 延迟）。本实现零同步点、零 map/unmap、零首帧预热：
// 唯一失败模式是「没 commit = 没内容」，一眼可见。参 TOPBAR_RENDER_REFACTOR §一。
//
// 颜色契约（§4.4）：复刻 wgpu 管线逐像素语义 —— 混合在**线性**空间逐样本执行，
// 存储值 = sRGB 编码。不透明快路径（a==1）退化为编码空间 lerp（与 GPU MSAA resolve
// 语义一致，截图零容差）；半透明走完整预乘（±1 灰度容差）。
// 与 tiny_skia 方案的差异（有意偏离）：tiny_skia 在 sRGB 空间预乘，无法复刻
// 「线性空间预乘 → sRGB 编码」契约；此处自实现 4× 超采样光栅器与 GPU MSAA 4× 同构。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, OnceLock};

use kanesumi_canvas::geometry::{Triangle, rounded_rect_polygon, triangulate_arc, triangulate_stroke};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Point, Rect, TextStyle};

use crate::render::{GlyphKey, PlacedGlyph, layout_text_glyphs};

/// 每像素超采样数（与 GPU 路径 MSAA 4× 同构；样本位 (0.25,0.25)…(0.75,0.75)）。
const SAMPLES: f32 = 4.0;
/// 四个样本相对像素原点的偏移。
const SAMPLE_OFFSETS: [(f32, f32); 4] = [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)];

// ── sRGB 编解码 LUT ────────────────────────────────────────────────────────

fn srgb_decode_lut() -> &'static [f32; 256] {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut t = [0.0f32; 256];
        for (i, v) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *v = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        t
    })
}

#[inline]
fn srgb_decode_f32(c: f32) -> f32 {
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

#[inline]
fn srgb_encode_f32(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

#[inline]
fn srgb_decode_u8(v: u8) -> f32 {
    srgb_decode_lut()[v as usize]
}

#[inline]
fn lerp_u8(v0: u8, v1: f32, t: f32) -> u8 {
    (v0 as f32 * (1.0 - t) + v1 * t).round().clamp(0.0, 255.0) as u8
}

/// 逐像素混合（复刻 GPU MSAA 语义）：
/// - `color`：sRGB 直通 + alpha（Scene 命令颜色）。
/// - `cov`：覆盖率 0..=1（超采样计数 / 4 或字形位图 / 255）。
/// 不透明（a==1）：编码空间 lerp（= MSAA resolve 对编码值的平均）。
/// 半透明：逐样本线性预乘 → sRGB 编码 → 与未覆盖样本平均。
#[inline]
fn blend_px(px: &mut [u8; 4], color: [f32; 4], cov: f32) {
    if cov <= 0.0 {
        return;
    }
    let a = color[3];
    if a >= 1.0 {
        px[0] = lerp_u8(px[0], color[0] * 255.0, cov);
        px[1] = lerp_u8(px[1], color[1] * 255.0, cov);
        px[2] = lerp_u8(px[2], color[2] * 255.0, cov);
        px[3] = lerp_u8(px[3], 255.0, cov);
        return;
    }
    // 预乘线性（复刻 shader：rgb = decode(color.rgb) * color.a）。
    let p = [
        srgb_decode_f32(color[0]) * a,
        srgb_decode_f32(color[1]) * a,
        srgb_decode_f32(color[2]) * a,
    ];
    let d = [
        srgb_decode_u8(px[0]),
        srgb_decode_u8(px[1]),
        srgb_decode_u8(px[2]),
    ];
    // 覆盖样本存储值 = 线性混合后 sRGB 编码（GPU 逐样本混合 + 存储编码）。
    let s = [
        srgb_encode_f32(p[0] + d[0] * (1.0 - a)),
        srgb_encode_f32(p[1] + d[1] * (1.0 - a)),
        srgb_encode_f32(p[2] + d[2] * (1.0 - a)),
    ];
    px[0] = lerp_u8(px[0], s[0] * 255.0, cov);
    px[1] = lerp_u8(px[1], s[1] * 255.0, cov);
    px[2] = lerp_u8(px[2], s[2] * 255.0, cov);
    let a_stored = a + (px[3] as f32 / 255.0) * (1.0 - a);
    px[3] = lerp_u8(px[3], a_stored * 255.0, cov);
}

// ── 光栅化器 ───────────────────────────────────────────────────────────────

/// CPU 光栅化器：物理像素 RGBA（sRGB 编码，与 wgpu Bgra8UnormSrgb 附件存储同语义）。
/// 输出缓冲为 RGBA 序（复刻旧 wgpu 读回），commit_shm_buffers 的 R/B 交换路径不变。
pub struct CpuRenderer {
    /// 物理像素宽高。
    w: u32,
    h: u32,
    /// 逻辑尺寸。
    lw: f32,
    lh: f32,
    scale: f32,
    /// 像素缓冲（RGBA，sRGB 编码 + 直通 alpha）。
    buf: Vec<u8>,
    /// 字形位图缓存：key = GlyphKey。静态文本每帧零重栅格化。
    glyph_bitmaps: HashMap<GlyphKey, (fontdue::Metrics, Vec<u8>)>,
    /// 图标缓存（FNV 内容去重）。`Arc<[u8]>` 共享字节，命中仅增引用计数——
    /// 旧 `(Vec<u8>,..).clone()` 每帧整张拷一遍，缓存形同虚设（参 egui texture atlas）。
    images: HashMap<u32, (Arc<[u8]>, u32, u32)>,
    /// 文本布局缓存（egui GalleyCache 同构思想，参 reference/egui fonts.rs 1061-1292）：
    /// key = 布局参数打包（内容 fnv1a + rect/style/align/wrap/max_lines/overflow/scale）。
    /// 静态文本每帧零重排版 —— 命中只 blit（字形位图已在 glyph_bitmaps）。
    /// generation GC：保留「本帧 ∪ 上帧」使用过的条目，其余淘汰（局部 damage 帧
    /// 未重绘的静态文本因上帧使用而保留，避免下个全量帧重排）。
    layout_cache: HashMap<u64, LayoutEntry>,
    layout_used: HashSet<u64>,
    layout_prev_used: HashSet<u64>,
    /// 布局 miss 计数（诊断/测试：静态文本重复渲染应不增长）。
    layout_misses: u64,
}

/// 布局缓存条目：内容（精确比较防 hash 碰撞误用）+ 放置结果。
struct LayoutEntry {
    content: Arc<str>,
    placed: Vec<PlacedGlyph>,
}

impl CpuRenderer {
    /// 新建渲染器。`width/height` 为逻辑尺寸，`scale` 为逻辑 → 物理缩放。
    pub fn new(width: f32, height: f32, scale: f32) -> Self {
        let mut r = Self {
            w: 0,
            h: 0,
            lw: width,
            lh: height,
            scale,
            buf: Vec::new(),
            glyph_bitmaps: HashMap::new(),
            images: HashMap::new(),
            layout_cache: HashMap::new(),
            layout_used: HashSet::new(),
            layout_prev_used: HashSet::new(),
            layout_misses: 0,
        };
        r.resize(width, height, scale);
        r
    }

    /// 重配尺寸：零填充重建，无 map/unmap/warmup 任何状态（§4.1）。
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        self.lw = width;
        self.lh = height;
        self.scale = scale;
        self.w = (width * scale).round().max(1.0) as u32;
        self.h = (height * scale).round().max(1.0) as u32;
        self.buf = vec![0u8; self.w as usize * self.h as usize * 4];
    }

    /// 只清逻辑 `d` 覆盖的物理像素（局部重绘用，其余像素保留上帧）。
    fn clear_rect(&mut self, d: Rect) {
        let x0 = (d.origin.x * self.scale).floor().clamp(0.0, self.w as f32) as u32;
        let y0 = (d.origin.y * self.scale).floor().clamp(0.0, self.h as f32) as u32;
        let x1 = (d.right() * self.scale).ceil().clamp(0.0, self.w as f32) as u32;
        let y1 = (d.bottom() * self.scale).ceil().clamp(0.0, self.h as f32) as u32;
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let row_bytes = (x1 - x0) as usize * 4;
        for py in y0..y1 {
            let row = (py * self.w + x0) as usize * 4;
            self.buf[row..row + row_bytes].fill(0);
        }
    }

    /// 物理像素尺寸（SHM 提交用）。
    pub fn physical_size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    /// 把一帧 Scene 光栅化进像素缓冲，返回 RGBA bytes。
    ///
    /// S4 局部重绘：`damage = Some(d)` 时只清/绘 `d` 内的像素，缓冲区其余部分
    /// **保留上一帧**（命令经 `d` 裁剪）。调用方必须保证 `d` 覆盖本帧全部变化；
    /// 场景存在未知变化（时钟/面板整体变动等）传 `None` → 全量重绘（每帧零填充）。
    pub fn render(&mut self, engine: &TextEngine, scene: &Scene, damage: Option<Rect>) -> &[u8] {
        self.render_inner(Some(engine), scene, damage)
    }

    /// 命令遍历实现。`engine = None` 时跳过 Text 命令（形状单测无需字体）。
    fn render_inner(
        &mut self,
        engine: Option<&TextEngine>,
        scene: &Scene,
        damage: Option<Rect>,
    ) -> &[u8] {
        match damage {
            // S4：仅清损坏矩形，其余保留上帧（配合命令裁剪 = 只重绘变化区）。
            Some(d) => self.clear_rect(d),
            // 全量：每帧零填充（透明底）。面板扩展区空白透出桌面（透明契约）。
            None => self.buf.fill(0),
        }
        // 命令裁剪：全量 → 原样；局部 → 与 damage 求交（None 栈顶 = 视口即 damage）。
        let clip_through_damage = |clip: Option<Rect>| -> Option<Rect> {
            match damage {
                Some(d) => match clip {
                    Some(c) => intersect_logical(c, d),
                    None => Some(d),
                },
                None => clip,
            }
        };
        let mut clip_stack: Vec<Option<Rect>> = Vec::new();
        for cmd in &scene.commands {
            match cmd {
                SceneCommand::PushClip { rect } => {
                    let effective = match clip_stack.last().copied() {
                        Some(Some(parent)) => intersect_logical(parent, *rect),
                        Some(None) => None,
                        None => Some(*rect),
                    };
                    clip_stack.push(effective);
                }
                SceneCommand::PopClip => {
                    if clip_stack.pop().is_none() {
                        log::warn!("Kanesumi CPU 光栅：未配对的 PopClip");
                    }
                }
                SceneCommand::FillRect { color, rect, corner_radius } => {
                    let clip = clip_through_damage(effective_clip(&clip_stack));
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_rect(*rect, *corner_radius, *color, clip);
                }
                SceneCommand::StrokeRect { color, rect, thickness, corner_radius } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_triangles(
                        triangulate_stroke(*rect, *corner_radius, *thickness),
                        *color,
                        clip_through_damage(effective_clip(&clip_stack)),
                    );
                }
                SceneCommand::Arc { center, radius, thickness, color, start_deg, end_deg } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_triangles(
                        triangulate_arc(*center, *radius, *thickness, *start_deg, *end_deg),
                        *color,
                        clip_through_damage(effective_clip(&clip_stack)),
                    );
                }
                SceneCommand::Triangle { p0, p1, p2, color } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_triangles(
                        vec![Triangle::new(*p0, *p1, *p2)],
                        *color,
                        clip_through_damage(effective_clip(&clip_stack)),
                    );
                }
                SceneCommand::Text {
                    content,
                    rect,
                    color,
                    style,
                    align,
                    wrap,
                    max_lines,
                    overflow,
                } => {
                    let Some(engine) = engine else { continue };
                    if is_fully_clipped(&clip_stack) || rect.is_empty() {
                        continue;
                    }
                    // 文字裁剪 = (有效裁剪 ∩ damage) ∩ rect。None = 无裁剪上下文
                    // （全量帧、无 PushClip）→ 回退表面边界（镜像 GPU 路径 surface_bounds
                    // 语义，参 render.rs emit_text）——⚠ 曾把 None 当「不相交」跳过：
                    // 全量帧无 clip 文本永不绘制，仅局部 damage 帧与损坏区相交才显示
                    // （静止时中文全部缺失、悬停一闪即出，S4 回归）。
                    let text_clip = match clip_through_damage(effective_clip(&clip_stack)) {
                        Some(parent) => intersect_logical(parent, *rect),
                        None => intersect_logical(Rect::new(0.0, 0.0, self.lw, self.lh), *rect),
                    };
                    let Some(text_clip) = text_clip else { continue };
                    self.emit_text(
                        engine, content, *rect, *color, *style, *align, *wrap, *max_lines,
                        *overflow, Some(text_clip),
                    );
                }
                SceneCommand::Image { rgba, width, height, rect, tint } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.emit_image(
                        rgba.as_ref(),
                        *width,
                        *height,
                        *rect,
                        *tint,
                        clip_through_damage(effective_clip(&clip_stack)),
                    );
                }
            }
        }
        // 布局缓存 generation GC：保留「本帧 ∪ 上帧」使用的条目（egui GalleyCache
        // 同款）；局部 damage 帧未重绘的静态文本因上帧使用而保留。淘汰条目的字形
        // 位图仍在 glyph_bitmaps（只增不减），重排时直接复用。
        let keep: HashSet<u64> = self
            .layout_used
            .union(&self.layout_prev_used)
            .copied()
            .collect();
        self.layout_cache.retain(|k, _| keep.contains(k));
        self.layout_prev_used = std::mem::take(&mut self.layout_used);
        &self.buf
    }

    // ── 形状填充 ────────────────────────────────────────────────────────────

    /// 轴对齐矩形快路径：解析覆盖率（分数边缘），逐行/逐像素混合。
    /// 顶栏背景等大面积不透明矩形走这里，避免 4× 超采样。
    fn fill_rect(&mut self, rect: Rect, radius: f32, color: Color, clip: Option<Rect>) {
        if radius <= 0.5 {
            let x0 = (rect.origin.x * self.scale) as f32;
            let y0 = (rect.origin.y * self.scale) as f32;
            let x1 = (rect.right() * self.scale) as f32;
            let y1 = (rect.bottom() * self.scale) as f32;
            self.fill_rect_aa(x0, y0, x1, y1, color, clip);
        } else {
            let pts = rounded_rect_polygon(rect, radius, 12);
            let tris = {
                let center = Point::new(
                    rect.origin.x + rect.size.width / 2.0,
                    rect.origin.y + rect.size.height / 2.0,
                );
                let mut t = Vec::with_capacity(pts.len());
                for i in 0..pts.len() {
                    let j = (i + 1) % pts.len();
                    t.push(Triangle::new(center, pts[i], pts[j]));
                }
                t
            };
            self.fill_triangles(tris, color, clip);
        }
    }

    /// 轴对齐矩形解析 AA 填充（物理浮点坐标）。
    fn fill_rect_aa(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, color: Color, clip: Option<Rect>) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let (mut cx0, mut cy0, mut cx1, mut cy1) = (0.0f32, 0.0f32, self.w as f32, self.h as f32);
        if let Some(c) = clip {
            cx0 = c.origin.x * self.scale;
            cy0 = c.origin.y * self.scale;
            cx1 = c.right() * self.scale;
            cy1 = c.bottom() * self.scale;
        }
        let x0c = x0.max(cx0);
        let y0c = y0.max(cy0);
        let x1c = x1.min(cx1);
        let y1c = y1.min(cy1);
        if x1c <= x0c || y1c <= y0c {
            return;
        }
        let c = [color.r, color.g, color.b, color.a];
        let py0 = y0c.floor().max(0.0) as u32;
        let py1 = (y1c.ceil().min(self.h as f32)) as u32;
        let px0 = x0c.floor().max(0.0) as u32;
        let px1 = (x1c.ceil().min(self.w as f32)) as u32;
        for py in py0..py1 {
            // 行覆盖率 = [py, py+1] 与 [y0c, y1c] 重叠长度。
            let cov_y = (y1c.min((py + 1) as f32) - y0c.max(py as f32)).clamp(0.0, 1.0);
            if cov_y <= 0.0 {
                continue;
            }
            for px in px0..px1 {
                let cov_x = (x1c.min((px + 1) as f32) - x0c.max(px as f32)).clamp(0.0, 1.0);
                if cov_x <= 0.0 {
                    continue;
                }
                let cov = cov_x * cov_y;
                if cov <= 0.0 {
                    continue;
                }
                let idx = (py * self.w + px) as usize * 4;
                let mut px4 = [self.buf[idx], self.buf[idx + 1], self.buf[idx + 2], self.buf[idx + 3]];
                blend_px(&mut px4, c, cov);
                self.buf[idx..idx + 4].copy_from_slice(&px4);
            }
        }
    }

    /// 三角形列表 4× 超采样填充（圆角矩形 / 环 / 弧 / 三角）。与 GPU MSAA 4× 同构。
    ///
    /// 性能（参考：reference/tiny-skia scan/path.rs、reference/raqote rasterizer.rs ——
    /// 扫描线区间裁剪，替代「每像素 × 每三角形 × 4 样本」）：
    /// 逐行收集覆盖区间 [xl, xr]（行中心扫描线 + 边斜率插值），只遍历区间 ± 斜率
    /// 外扩的像素；整像素完全位于区间内部（距边界 ≥ EXT，EXT = ⌈max|dx/dy|·¼⌉+1）
    /// 且行距每个相交三角形带边界 ≥ 0.5 → 4 样本必然全中 → cov 直写，跳过逐样本判定。
    /// 输出与逐像素 4× 超采样**逐位一致**：内部像素数学严格全中；边缘像素判定不变；
    /// 范围外像素（样本 x 距行中心区间 ≥ EXT-0.75 > max|dx/dy|·¼）严格不命中任何三角形。
    fn fill_triangles(&mut self, tris: Vec<Triangle>, color: Color, clip: Option<Rect>) {
        if tris.is_empty() {
            return;
        }
        let c = [color.r, color.g, color.b, color.a];
        let scale = self.scale;
        // 三角形 → 物理坐标 + 三条边（斜率 dx/dy + y 半开带 [lo, hi)）。
        struct TriE {
            pts: [[f32; 2]; 3],
            y_lo: f32,
            y_hi: f32,
            edges: [[f32; 5]; 3],
        }
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut phys: Vec<TriE> = Vec::with_capacity(tris.len());
        for t in &tris {
            let pts = [
                [t.p0.x * scale, t.p0.y * scale],
                [t.p1.x * scale, t.p1.y * scale],
                [t.p2.x * scale, t.p2.y * scale],
            ];
            for p in &pts {
                min_x = min_x.min(p[0]);
                min_y = min_y.min(p[1]);
                max_x = max_x.max(p[0]);
                max_y = max_y.max(p[1]);
            }
            let mut edges = [[0.0f32; 5]; 3];
            for (k, (a, b)) in [(0, 1), (1, 2), (2, 0)].into_iter().enumerate() {
                let (p, q) = (pts[a], pts[b]);
                let (y_lo, y_hi) = if p[1] <= q[1] { (p[1], q[1]) } else { (q[1], p[1]) };
                // 水平边（dy==0）不参与扫描线求交，斜率置 0（该边无覆盖率贡献）。
                let dx_dy = if q[1] != p[1] { (q[0] - p[0]) / (q[1] - p[1]) } else { 0.0 };
                edges[k] = [p[0], p[1], dx_dy, y_lo, y_hi];
            }
            let y_lo = pts[0][1].min(pts[1][1]).min(pts[2][1]);
            let y_hi = pts[0][1].max(pts[1][1]).max(pts[2][1]);
            phys.push(TriE {
                pts,
                y_lo,
                y_hi,
                edges,
            });
        }
        // 裁剪：包围盒 ∩ clip（物理）。
        let (mut cx0, mut cy0, mut cx1, mut cy1) = (0.0f32, 0.0f32, self.w as f32, self.h as f32);
        if let Some(cl) = clip {
            cx0 = cl.origin.x * scale;
            cy0 = cl.origin.y * scale;
            cx1 = cl.right() * scale;
            cy1 = cl.bottom() * scale;
        }
        let bx0 = min_x.max(cx0);
        let by0 = min_y.max(cy0);
        let bx1 = max_x.min(cx1);
        let by1 = max_y.min(cy1);
        if bx1 <= bx0 || by1 <= by0 {
            return;
        }
        let py0 = by0.floor().max(0.0) as u32;
        let py1 = (by1.ceil().min(self.h as f32)) as u32;
        let px0 = bx0.floor().max(0.0) as u32;
        let px1 = (bx1.ceil().min(self.w as f32)) as u32;
        // 每行：收集投影区间（像素遍历范围）与行中心区间（内部快路径），分别合并。
        // 投影区间 = 带∩行中点处覆盖 ± ½·max|dx/dy|（覆盖端点随 y 线性变化，带∩行
        // 半宽 ≤0.5 → 保守包含该行所有可能命中的样本 x —— 关键：带与像素行部分重叠
        // 时行中心 y_c 可在带外而样本 y 在带内，必须用带∩行投影，否则漏像素）。
        for py in py0..py1 {
            let y_c = py as f32 + 0.5;
            let row_top = py as f32;
            let row_bot = py as f32 + 1.0;
            let mut spans: Vec<(f32, f32, f32)> = Vec::new();
            let mut centers: Vec<(f32, f32, f32)> = Vec::new();
            let mut band_margin = f32::MAX;
            for t in &phys {
                if t.y_lo >= row_bot || t.y_hi <= row_top {
                    continue;
                }
                let y_lo = t.y_lo.max(row_top);
                let y_hi = t.y_hi.min(row_bot);
                let y_m = (y_lo + y_hi) * 0.5;
                let mut xl = f32::MAX;
                let mut xr = f32::MIN;
                let mut n = 0usize;
                let mut ms = 0.0f32;
                for e in &t.edges {
                    if y_m >= e[3] && y_m < e[4] {
                        let x = e[0] + (y_m - e[1]) * e[2];
                        xl = xl.min(x);
                        xr = xr.max(x);
                        n += 1;
                        ms = ms.max(e[2].abs());
                    }
                }
                if n >= 2 && xr > xl {
                    let half = ms * 0.5;
                    spans.push((xl - half, xr + half, ms));
                }
                // 行中心区间：仅带覆盖行中心时可用作内部快路径。
                if y_c >= t.y_lo && y_c < t.y_hi {
                    band_margin = band_margin.min((y_c - t.y_lo).min(t.y_hi - y_c));
                    let mut xl = f32::MAX;
                    let mut xr = f32::MIN;
                    let mut n = 0usize;
                    let mut ms = 0.0f32;
                    for e in &t.edges {
                        if y_c >= e[3] && y_c < e[4] {
                            let x = e[0] + (y_c - e[1]) * e[2];
                            xl = xl.min(x);
                            xr = xr.max(x);
                            n += 1;
                            ms = ms.max(e[2].abs());
                        }
                    }
                    if n >= 2 && xr > xl {
                        centers.push((xl, xr, ms));
                    }
                }
            }
            if spans.is_empty() {
                continue;
            }
            // 合并重叠/相邻区间（并集语义），斜率取 max。
            let merge = |v: &mut Vec<(f32, f32, f32)>| {
                v.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                let mut out: Vec<(f32, f32, f32)> = Vec::new();
                for s in v.drain(..) {
                    if let Some(last) = out.last_mut() {
                        if s.0 <= last.1 {
                            last.1 = last.1.max(s.1);
                            last.2 = last.2.max(s.2);
                            continue;
                        }
                    }
                    out.push(s);
                }
                *v = out;
            };
            merge(&mut spans);
            merge(&mut centers);
            let row_cov_y = (cy1.min((py + 1) as f32) - cy0.max(py as f32)).clamp(0.0, 1.0);
            if row_cov_y <= 0.0 {
                continue;
            }
            // 内部行：样本 y（py+0.25 / py+0.75）仍在每个「带覆盖行中心」的三角形带内。
            let inside_row = band_margin >= 0.5;
            for (pl, pr, _ms) in &spans {
                // 像素范围：命中像素样本 x ∈ [px+0.25, px+0.75] 必与投影区间相交。
                let px_lo = ((pl.ceil() as i64 - 1).max(cx0.floor() as i64)).max(px0 as i64);
                let px_hi = ((pr.floor() as i64 + 1).min(cx1.ceil() as i64)).min(px1 as i64);
                for px in px_lo..px_hi {
                    // 裁剪覆盖率（解析；clip 为轴对齐矩形）。
                    let clip_cov_x =
                        (cx1.min((px + 1) as f32) - cx0.max(px as f32)).clamp(0.0, 1.0);
                    let clip_cov = clip_cov_x * row_cov_y;
                    if clip_cov <= 0.0 {
                        continue;
                    }
                    let idx = (py * self.w + px as u32) as usize * 4;
                    let mut px4 = [self.buf[idx], self.buf[idx + 1], self.buf[idx + 2], self.buf[idx + 3]];
                    // 内部像素快路径：在行中心合并区间内部（距边界 ≥ EXT）且内部行
                    // → 4 样本必然全中（数学严格，见函数注释），cov = clip_cov 直写。
                    let mut internal = false;
                    if inside_row {
                        for (xl, xr, ms) in &centers {
                            let ext = ((ms * 0.25).ceil() as i64 + 1).max(1);
                            if (px as i64) >= (xl.ceil() as i64) + ext
                                && (px as i64) + 1 <= (xr.floor() as i64) - ext
                            {
                                internal = true;
                                break;
                            }
                        }
                    }
                    if internal {
                        blend_px(&mut px4, c, clip_cov);
                    } else {
                        // 4× 超采样计数（并集语义：任一三角形命中即 +1）。
                        let mut hits = 0u8;
                        for (ox, oy) in SAMPLE_OFFSETS {
                            let sx = px as f32 + ox;
                            let sy = py as f32 + oy;
                            for t in &phys {
                                if point_in_triangle(sx, sy, &t.pts[0], &t.pts[1], &t.pts[2]) {
                                    hits += 1;
                                    break;
                                }
                            }
                        }
                        if hits == 0 {
                            continue;
                        }
                        blend_px(&mut px4, c, hits as f32 / SAMPLES * clip_cov);
                    }
                    self.buf[idx..idx + 4].copy_from_slice(&px4);
                }
            }
        }
    }

    // ── 文本 / 图标 ──────────────────────────────────────────────────────────

    /// 文本命令 → 字形位图覆盖 blit。placement 与 GPU emit_text 共用 layout_text_glyphs。
    ///
    /// 布局缓存（egui GalleyCache 思想）：静态文本（菜单标题/活跃应用名/Dock 标签等）
    /// 每帧命中缓存零重排版，只做字形 blit（clip 每次独立 —— 布局结果不依赖 clip）。
    #[allow(clippy::too_many_arguments)]
    fn emit_text(
        &mut self,
        engine: &TextEngine,
        content: &str,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
        wrap: bool,
        max_lines: Option<usize>,
        overflow: kanesumi_canvas::TextOverflow,
        clip: Option<Rect>,
    ) {
        let scale = self.scale;
        let c = [color.r, color.g, color.b, color.a];
        let key = layout_key(content, rect, style, align, wrap, max_lines, overflow, scale);
        self.layout_used.insert(key);
        // 缓存与字形位图移出 self（避免与 self.buf 借用冲突），零克隆。
        let mut cache = std::mem::take(&mut self.layout_cache);
        let mut glyphs = std::mem::take(&mut self.glyph_bitmaps);
        // 命中：直接 blit（字形位图已在 glyph_bitmaps，静态文本零重排版）。
        let hit = match cache.get(&key) {
            Some(e) if e.content.as_ref() == content => {
                for g in &e.placed {
                    if let Some((_, bitmap)) = glyphs.get(&g.key) {
                        self.blit_coverage(bitmap, g.x, g.y, g.w, g.h, c, clip);
                    }
                }
                true
            }
            _ => false,
        };
        if !hit {
            // miss：layout + 光栅化字形 + 入缓存。
            self.layout_misses += 1;
            let placed = layout_text_glyphs(
                engine,
                &mut glyphs,
                content,
                rect,
                style,
                align,
                wrap,
                max_lines,
                overflow,
                scale,
            );
            for g in &placed {
                if let Some((_, bitmap)) = glyphs.get(&g.key) {
                    self.blit_coverage(bitmap, g.x, g.y, g.w, g.h, c, clip);
                }
            }
            cache.insert(key, LayoutEntry { content: Arc::from(content), placed });
        }
        self.layout_cache = cache;
        self.glyph_bitmaps = glyphs;
    }

    /// 覆盖位图 blit：`bitmap` 为灰度覆盖（0..=255），目标左上角 / 尺寸为逻辑坐标。
    fn blit_coverage(
        &mut self,
        bitmap: &[u8],
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [f32; 4],
        clip: Option<Rect>,
    ) {
        let scale = self.scale;
        let bw = (w * scale).round() as u32;
        let bh = (h * scale).round() as u32;
        if bw == 0 || bh == 0 || bitmap.is_empty() {
            return;
        }
        let x0 = (x * scale).round() as i64;
        let y0 = (y * scale).round() as i64;
        // 裁剪边界（物理）。
        let (mut cx0, mut cy0, mut cx1, mut cy1) = (0i64, 0i64, self.w as i64, self.h as i64);
        if let Some(cl) = clip {
            cx0 = (cl.origin.x * scale).floor() as i64;
            cy0 = (cl.origin.y * scale).floor() as i64;
            cx1 = (cl.right() * scale).ceil() as i64;
            cy1 = (cl.bottom() * scale).ceil() as i64;
        }
        for py in y0..y0 + bh as i64 {
            if py < cy0 || py >= cy1 {
                continue;
            }
            for px in x0..x0 + bw as i64 {
                if px < cx0 || px >= cx1 {
                    continue;
                }
                let tx = (px - x0).clamp(0, bw as i64 - 1) as u32;
                let ty = (py - y0).clamp(0, bh as i64 - 1) as u32;
                let cov = bitmap[(ty * bw + tx) as usize] as f32 / 255.0;
                if cov <= 0.0 {
                    continue;
                }
                let idx = ((py as u32) * self.w + px as u32) as usize * 4;
                let mut px4 = [self.buf[idx], self.buf[idx + 1], self.buf[idx + 2], self.buf[idx + 3]];
                blend_px(&mut px4, color, cov);
                self.buf[idx..idx + 4].copy_from_slice(&px4);
            }
        }
    }

    /// Image 命令 → 双线性采样 blit（sRGB 解码后线性插值，同 GPU 采样器语义）。
    /// `tint` 复刻 IMAGE_SHADER：None/白 = 原色；其他 = 以图标 alpha 蒙版染色。
    fn emit_image(
        &mut self,
        rgba: &[u8],
        sw: u32,
        sh: u32,
        rect: Rect,
        tint: Option<Color>,
        clip: Option<Rect>,
    ) {
        if sw == 0 || sh == 0 || rgba.len() < (sw * sh * 4) as usize {
            return;
        }
        let key = fnv1a(rgba);
        let cached = self
            .images
            .entry(key)
            .or_insert_with(|| (Arc::from(rgba), sw, sh))
            .clone();
        let (src, sw, sh) = cached;
        let scale = self.scale;
        let x0 = rect.origin.x * scale;
        let y0 = rect.origin.y * scale;
        let x1 = rect.right() * scale;
        let y1 = rect.bottom() * scale;
        let (mut cx0, mut cy0, mut cx1, mut cy1) = (0.0f32, 0.0f32, self.w as f32, self.h as f32);
        if let Some(cl) = clip {
            cx0 = cl.origin.x * scale;
            cy0 = cl.origin.y * scale;
            cx1 = cl.right() * scale;
            cy1 = cl.bottom() * scale;
        }
        let x0c = x0.max(cx0);
        let y0c = y0.max(cy0);
        let x1c = x1.min(cx1);
        let y1c = y1.min(cy1);
        if x1c <= x0c || y1c <= y0c {
            return;
        }
        let tint_is_white = tint
            .map(|t| t.r == 1.0 && t.g == 1.0 && t.b == 1.0)
            .unwrap_or(true);
        let tint_rgb = tint.map(|t| {
            [srgb_decode_f32(t.r), srgb_decode_f32(t.g), srgb_decode_f32(t.b)]
        });
        let dst_w = x1 - x0;
        let dst_h = y1 - y0;
        let px0 = x0c.floor().max(0.0) as u32;
        let py0 = y0c.floor().max(0.0) as u32;
        let px1 = (x1c.ceil().min(self.w as f32)) as u32;
        let py1 = (y1c.ceil().min(self.h as f32)) as u32;
        for py in py0..py1 {
            for px in px0..px1 {
                // 目标像素中心 → 源坐标（双线性）。
                let u = ((px as f32 + 0.5 - x0) / dst_w * sw as f32 - 0.5).clamp(0.0, sw as f32 - 1.0);
                let v = ((py as f32 + 0.5 - y0) / dst_h * sh as f32 - 0.5).clamp(0.0, sh as f32 - 1.0);
                let (u0, v0) = (u.floor() as u32, v.floor() as u32);
                let (u1, v1) = ((u0 + 1).min(sw - 1), (v0 + 1).min(sh - 1));
                let (fu, fv) = (u - u0 as f32, v - v0 as f32);
                let s00 = texel(&src[..], sw, u0, v0);
                let s10 = texel(&src[..], sw, u1, v0);
                let s01 = texel(&src[..], sw, u0, v1);
                let s11 = texel(&src[..], sw, u1, v1);
                // 逐通道双线性（sRGB 解码后线性插值）。
                let mut lin = [0.0f32; 4];
                for ch in 0..4 {
                    let c00 = srgb_decode_u8(s00[ch]);
                    let c10 = srgb_decode_u8(s10[ch]);
                    let c01 = srgb_decode_u8(s01[ch]);
                    let c11 = srgb_decode_u8(s11[ch]);
                    lin[ch] = c00 * (1.0 - fu) * (1.0 - fv)
                        + c10 * fu * (1.0 - fv)
                        + c01 * (1.0 - fu) * fv
                        + c11 * fu * fv;
                }
                // 预乘线性（复刻 IMAGE_SHADER：rgb × alpha；tint 时以 tint 替换颜色）。
                let a = lin[3];
                let rgb = match (&tint_rgb, tint_is_white) {
                    (Some(t), false) => [t[0] * a, t[1] * a, t[2] * a],
                    _ => [lin[0] * a, lin[1] * a, lin[2] * a],
                };
                if a <= 0.0 {
                    continue;
                }
                let idx = (py * self.w + px) as usize * 4;
                let mut px4 = [self.buf[idx], self.buf[idx + 1], self.buf[idx + 2], self.buf[idx + 3]];
                let mut c = [0.0f32; 4];
                for ch in 0..3 {
                    c[ch] = srgb_encode_f32(rgb[ch]).clamp(0.0, 1.0);
                }
                c[3] = a;
                blend_px(&mut px4, c, 1.0);
                self.buf[idx..idx + 4].copy_from_slice(&px4);
            }
        }
    }
}

// ── 辅助 ───────────────────────────────────────────────────────────────────

/// 有效裁剪 = 栈内全部矩形的交集（子容器只收窄）。
fn effective_clip(stack: &[Option<Rect>]) -> Option<Rect> {
    stack.last().copied().flatten()
}

/// 栈顶为 None（子与父不相交）→ 完全裁剪。
fn is_fully_clipped(stack: &[Option<Rect>]) -> bool {
    matches!(stack.last(), Some(None))
}

fn intersect_logical(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// 边函数符号一致性判定（三角形方向已归一为 CCW 时全 ≥ 0 即在内）。
fn point_in_triangle(px: f32, py: f32, a: &[f32; 2], b: &[f32; 2], c: &[f32; 2]) -> bool {
    let e = |p: &[f32; 2], q: &[f32; 2], r: (f32, f32)| -> f32 {
        (q[0] - p[0]) * (r.1 - p[1]) - (q[1] - p[1]) * (r.0 - p[0])
    };
    let ab = e(a, b, (px, py));
    let bc = e(b, c, (px, py));
    let ca = e(c, a, (px, py));
    (ab >= 0.0 && bc >= 0.0 && ca >= 0.0) || (ab <= 0.0 && bc <= 0.0 && ca <= 0.0)
}

fn texel(src: &[u8], sw: u32, x: u32, y: u32) -> [u8; 4] {
    let idx = (y * sw + x) as usize * 4;
    [src[idx], src[idx + 1], src[idx + 2], src[idx + 3]]
}

/// FNV-1a 32 位哈希 —— 图标内容去重（同 render.rs）。
fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 布局缓存 key：内容 fnv1a + 全部布局参数打包（u64 旋转混合）。参数值精确进 key
/// （内容相等但参数不同 → 不同 key）；命中后再比内容防碰撞误用。
#[allow(clippy::too_many_arguments)]
fn layout_key(
    content: &str,
    rect: Rect,
    style: TextStyle,
    align: TextAlign,
    wrap: bool,
    max_lines: Option<usize>,
    overflow: kanesumi_canvas::TextOverflow,
    scale: f32,
) -> u64 {
    let mut h = fnv1a(content.as_bytes()) as u64;
    h = h.rotate_left(13) ^ rect.origin.x.to_bits() as u64;
    h = h.rotate_left(13) ^ rect.origin.y.to_bits() as u64;
    h = h.rotate_left(13) ^ rect.size.width.to_bits() as u64;
    h = h.rotate_left(13) ^ rect.size.height.to_bits() as u64;
    h = h.rotate_left(13) ^ style.size.to_bits() as u64;
    h = h.rotate_left(13) ^ style.line_height.to_bits() as u64;
    h = h.rotate_left(13) ^ style.letter_spacing_em.to_bits() as u64;
    h = h.rotate_left(13) ^ style.weight as u64;
    h = h.rotate_left(13) ^ align as u64;
    h = h.rotate_left(13) ^ wrap as u64;
    h = h.rotate_left(13) ^ max_lines.unwrap_or(0) as u64;
    h = h.rotate_left(13) ^ overflow as u64;
    h = h.rotate_left(13) ^ scale.to_bits() as u64;
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    use kanesumi_canvas::text::TextEngine;
    use kanesumi_core::{Color, FontWeight, TextStyle};

    fn test_font_path() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        [
            "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
            "/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ]
        .into_iter()
        .map(std::path::PathBuf::from)
        .find(|p| p.exists())
    }

    /// 统计缓冲内非零像素数。
    fn painted_count(r: &CpuRenderer) -> usize {
        r.buf
            .chunks_exact(4)
            .filter(|px| px[0] != 0 || px[1] != 0 || px[2] != 0 || px[3] != 0)
            .count()
    }

    fn render_scene(scene: &Scene, w: f32, h: f32) -> CpuRenderer {
        let mut r = CpuRenderer::new(w, h, 1.0);
        // 形状测试不依赖字体：engine = None 跳过 Text 命令。
        r.render_inner(None, scene, None);
        r
    }

    fn render_scene_damaged(scene: &Scene, w: f32, h: f32, d: Rect) -> CpuRenderer {
        let mut r = CpuRenderer::new(w, h, 1.0);
        r.render_inner(None, scene, Some(d));
        r
    }

    fn px_at(r: &CpuRenderer, x: u32, y: u32) -> [u8; 4] {
        let idx = (y * r.w + x) as usize * 4;
        [r.buf[idx], r.buf[idx + 1], r.buf[idx + 2], r.buf[idx + 3]]
    }

    #[test]
    fn opaque_rect_interior_is_exact() {
        let mut scene = Scene::default();
        scene.fill_rect(Color::new(0.2, 0.4, 0.6, 1.0), Rect::new(0.0, 0.0, 8.0, 8.0));
        let r = render_scene(&scene, 8.0, 8.0);
        let p = px_at(&r, 4, 4);
        assert_eq!([p[0], p[1], p[2]], [51, 102, 153]); // 0.2/0.4/0.6 × 255
        assert_eq!(p[3], 255);
    }

    #[test]
    fn opaque_rect_aa_edge_is_encoded_lerp() {
        let mut scene = Scene::default();
        // 半像素覆盖：x∈[0, 0.5) 的矩形。
        scene.fill_rect(Color::WHITE, Rect::new(0.0, 0.0, 0.5, 8.0));
        let r = render_scene(&scene, 8.0, 8.0);
        let p = px_at(&r, 0, 4);
        assert_eq!(p[0], 128); // lerp(0, 255, 0.5) = 127.5 → 128（MSAA resolve 同语义）
        assert_eq!(p[3], 128);
        let p2 = px_at(&r, 1, 4);
        assert_eq!(p2, [0, 0, 0, 0]);
    }

    #[test]
    fn translucent_premul_matches_gpu_contract() {
        // 白 50% 叠透明：rgb = encode(decode(1.0)*0.5) = encode(0.5) = 188；a = 128。
        let mut px = [0u8, 0, 0, 0];
        blend_px(&mut px, [1.0, 1.0, 1.0, 0.5], 1.0);
        assert_eq!(px[0], 188);
        assert_eq!(px[3], 128);
    }

    #[test]
    fn translucent_over_opaque_uses_linear_blend() {
        // 白 50% 叠黑不透明（MSAA 覆盖样本语义）：样本 = encode(decode(1)*0.5 + 0*0.5) = 188。
        let mut px = [0u8, 0, 0, 255];
        blend_px(&mut px, [1.0, 1.0, 1.0, 0.5], 1.0);
        assert_eq!(px[0], 188);
        assert_eq!(px[3], 255); // a = 0.5 + 1*(1-0.5) = 1
    }

    #[test]
    fn rounded_rect_corner_has_partial_coverage() {
        let mut scene = Scene::default();
        scene.fill_rect(Color::WHITE, Rect::new(0.0, 0.0, 10.0, 10.0));
        // 加圆角：直接构造命令（scene.fill_rect 无圆角参数 → 手工推入）。
        let mut s2 = Scene::default();
        s2.commands.push(SceneCommand::FillRect {
            color: Color::WHITE,
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            corner_radius: 4.0,
        });
        let r = render_scene(&s2, 10.0, 10.0);
        // 角像素（0,0）覆盖率 < 1；中心像素不透明。
        assert!(px_at(&r, 0, 0)[3] < 255);
        assert_eq!(px_at(&r, 5, 5), [255, 255, 255, 255]);
        let _ = scene;
    }

    #[test]
    fn clip_rect_limits_fill() {
        let mut scene = Scene::default();
        scene.push_clip(Rect::new(0.0, 0.0, 4.0, 8.0));
        scene.fill_rect(Color::WHITE, Rect::new(0.0, 0.0, 8.0, 8.0));
        scene.pop_clip();
        let r = render_scene(&scene, 8.0, 8.0);
        assert_eq!(px_at(&r, 2, 4), [255, 255, 255, 255]);
        assert_eq!(px_at(&r, 6, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn triangle_fill_hits_interior() {
        let mut scene = Scene::default();
        scene.triangle(
            Point::new(0.0, 8.0),
            Point::new(4.0, 0.0),
            Point::new(8.0, 8.0),
            Color::WHITE,
        );
        let r = render_scene(&scene, 8.0, 8.0);
        assert_eq!(px_at(&r, 4, 4), [255, 255, 255, 255]);
        assert_eq!(px_at(&r, 0, 4), [0, 0, 0, 0]);
    }

    #[test]
    fn resize_rebuilds_buffer_cleanly() {
        let mut r = CpuRenderer::new(8.0, 8.0, 1.0);
        r.resize(16.0, 16.0, 2.0);
        assert_eq!(r.physical_size(), (32, 32));
        assert_eq!(r.buf.len(), 32 * 32 * 4);
        assert!(r.buf.iter().all(|&b| b == 0));
    }

    /// S4：局部重绘 —— 首帧全量画左半，第二帧 damage 右半 → 仅右半被当前命令
    /// 影响，左半保留首帧像素（未受影响区域不重光栅）。
    #[test]
    fn damage_redraw_keeps_untouched_region() {
        // 帧 1：左半边白。
        let mut scene1 = Scene::default();
        scene1.fill_rect(Color::new(1.0, 1.0, 1.0, 1.0), Rect::new(0.0, 0.0, 4.0, 8.0));
        let mut r = render_scene(&scene1, 8.0, 8.0);
        assert_eq!(px_at(&r, 2, 4), [255, 255, 255, 255], "帧1左半白");

        // 帧 2：右半边红，只报右侧 damage → 左半保留帧1白，右半重绘为红。
        let mut scene2 = Scene::default();
        scene2.fill_rect(Color::new(1.0, 0.0, 0.0, 1.0), Rect::new(4.0, 0.0, 4.0, 8.0));
        r.render_inner(None, &scene2, Some(Rect::new(4.0, 0.0, 4.0, 8.0)));
        assert_eq!(px_at(&r, 2, 4), [255, 255, 255, 255], "左侧未重光栅保持帧1");
        assert_eq!(px_at(&r, 6, 4), [255, 0, 0, 255], "右侧重绘为红");
    }

    /// S4：无 damage + 全量 → 整表面被当前命令覆盖（旧行为）。
    #[test]
    fn full_redraw_clears_all() {
        let mut scene1 = Scene::default();
        scene1.fill_rect(Color::new(1.0, 1.0, 1.0, 1.0), Rect::new(0.0, 0.0, 8.0, 8.0));
        let mut r = render_scene(&scene1, 8.0, 8.0);
        let mut scene2 = Scene::default();
        scene2.fill_rect(Color::new(0.0, 0.0, 1.0, 1.0), Rect::new(0.0, 0.0, 8.0, 8.0));
        r.render_inner(None, &scene2, None);
        assert_eq!(px_at(&r, 2, 4), [0, 0, 255, 255], "全量覆盖");
        assert_eq!(px_at(&r, 6, 4), [0, 0, 255, 255]);
    }

    /// S4 回归：无 PushClip 的文本命令在全量帧必须绘制。
    ///
    /// 曾把「无裁剪上下文（None）」误判为「与损坏区不相交 → 跳过」，导致全量帧
    /// 全部无 clip 文本（TopBar 时钟/活跃应用/菜单标题、Dock 图标标签等）永不
    /// 绘制 —— 静止时中文全部缺失，仅局部 damage 帧与损坏区相交才显示。
    #[test]
    fn text_without_clip_draws_in_full_frame() {
        let Some(path) = test_font_path() else {
            return;
        };
        let engine = TextEngine::load(path).unwrap();
        let style = TextStyle::new(20.0, 24.0, FontWeight::Normal);
        let mut scene = Scene::default();
        scene.text(
            "中文测试".to_string(),
            Rect::new(2.0, 4.0, 56.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        let mut r = CpuRenderer::new(64.0, 32.0, 1.0);
        r.render(&engine, &scene, None);
        let painted = painted_count(&r);
        assert!(painted > 0, "全量帧无 clip 文本必须绘制（曾整体跳过）");
        // 文本只落在给定矩形附近，不越界污染表面。
        let mut outside = 0;
        for py in 0..32u32 {
            for px in 0..64u32 {
                if !(px >= 2 && px < 58 && py >= 4 && py < 28) {
                    let idx = (py * 64 + px) as usize * 4;
                    if r.buf[idx + 3] != 0 {
                        outside += 1;
                    }
                }
            }
        }
        assert_eq!(outside, 0, "文本不得绘制在 rect 之外");
    }

    /// S4：局部 damage 与文本 rect 不相交 → 该项跳过（保留上帧像素）；
    /// 相交 → 只重绘相交区。配合 damage 语义不得引入「无 clip 全跳过」回归。
    #[test]
    fn text_damage_intersects_or_keeps_previous_frame() {
        let Some(path) = test_font_path() else {
            return;
        };
        let engine = TextEngine::load(path).unwrap();
        let style = TextStyle::new(20.0, 24.0, FontWeight::Normal);
        // 帧 1：全量，左半画中文。
        let mut scene1 = Scene::default();
        scene1.text(
            "中文".to_string(),
            Rect::new(2.0, 4.0, 40.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        let mut r = CpuRenderer::new(64.0, 32.0, 1.0);
        r.render(&engine, &scene1, None);
        let painted1 = painted_count(&r);
        assert!(painted1 > 0);

        // 帧 2：damage 右侧（不覆盖文本）→ 文本区像素保留帧 1。
        let mut scene2 = Scene::default();
        scene2.fill_rect(Color::new(1.0, 0.0, 0.0, 1.0), Rect::new(48.0, 0.0, 16.0, 32.0));
        r.render(&engine, &scene2, Some(Rect::new(48.0, 0.0, 16.0, 32.0)));
        assert_eq!(painted_count(&r), painted1 + 16 * 32, "文本区保留、右侧新增");

        // 帧 3：damage 覆盖文本区 → 新文本（右侧）只画在损坏区内。
        let mut scene3 = Scene::default();
        scene3.text(
            "字".to_string(),
            Rect::new(52.0, 4.0, 20.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        r.render(&engine, &scene3, Some(Rect::new(48.0, 0.0, 16.0, 32.0)));
        let mut in_damage = 0;
        for py in 0..32u32 {
            for px in 48..64u32 {
                let idx = (py * 64 + px) as usize * 4;
                if r.buf[idx + 3] != 0 {
                    in_damage += 1;
                }
            }
        }
        assert!(in_damage > 0, "损坏区内文本必须重绘");
        // 左半文本像素不得被破坏（上帧保留）。
        assert!(painted_count(&r) > in_damage, "文本区未损坏部分保留上帧");
    }

    /// 扫描线区间裁剪与朴素「每像素 × 每三角形 × 4 样本」参考实现逐位一致
    /// （零容差契约的硬保证，参函数注释的严格性推导）。
    #[test]
    fn fill_triangles_scanline_matches_naive() {
        use kanesumi_canvas::geometry::{rounded_rect_polygon, triangulate_arc, triangulate_stroke};
        use kanesumi_core::{Point, Rect};
        // 场景：圆角矩形 + 弧 + stroke 环 + 两个独立三角形 + 圆角 clip。
        let mut tris: Vec<kanesumi_canvas::geometry::Triangle> = Vec::new();
        // 圆角矩形（中心扇出 12 三角）。
        let rect = Rect::new(2.0, 3.0, 40.0, 28.0);
        let pts = rounded_rect_polygon(rect, 5.0, 12);
        let center = Point::new(rect.origin.x + rect.size.width / 2.0, rect.origin.y + rect.size.height / 2.0);
        for i in 0..pts.len() {
            let j = (i + 1) % pts.len();
            tris.push(kanesumi_canvas::geometry::Triangle::new(center, pts[i], pts[j]));
        }
        // 弧（环形扇带）。
        tris.extend(triangulate_arc(Point::new(50.0, 10.0), 8.0, 2.0, 20.0, 200.0));
        // stroke 环。
        tris.extend(triangulate_stroke(Rect::new(60.0, 2.0, 12.0, 12.0), 2.0, 1.5));
        // 独立三角形。
        tris.push(kanesumi_canvas::geometry::Triangle::new(
            Point::new(78.0, 4.0),
            Point::new(82.0, 14.0),
            Point::new(74.0, 16.0),
        ));
        let clip = Rect::new(0.0, 0.0, 84.0, 30.0);

        // 参考实现（朴素每像素 4 样本；保留旧 fill_triangles 语义）。
        let naive = |src: &[kanesumi_canvas::geometry::Triangle]| {
            let mut r = CpuRenderer::new(84.0, 30.0, 1.0);
            let c = [0.4f32, 0.8, 1.0, 0.7]; // 半透明覆盖 blend 全路径
            let phys: Vec<[[f32; 2]; 3]> = src
                .iter()
                .map(|t| {
                    [
                        [t.p0.x, t.p0.y],
                        [t.p1.x, t.p1.y],
                        [t.p2.x, t.p2.y],
                    ]
                })
                .collect();
            let clip_rect = [
                clip.origin.x,
                clip.origin.y,
                clip.right(),
                clip.bottom(),
            ];
            let (cx0, cy0, cx1, cy1) =
                (clip_rect[0], clip_rect[1], clip_rect[2], clip_rect[3]);
            let (bx0, by0, bx1, by1) = (cx0, cy0, cx1, cy1);
            for py in by0.floor().max(0.0) as u32..by1.ceil().min(r.h as f32) as u32 {
                for px in bx0.floor().max(0.0) as u32..bx1.ceil().min(r.w as f32) as u32 {
                    let clip_cov_x =
                        (cx1.min((px + 1) as f32) - cx0.max(px as f32)).clamp(0.0, 1.0);
                    let clip_cov_y =
                        (cy1.min((py + 1) as f32) - cy0.max(py as f32)).clamp(0.0, 1.0);
                    let clip_cov = clip_cov_x * clip_cov_y;
                    if clip_cov <= 0.0 {
                        continue;
                    }
                    let mut hits = 0u8;
                    for (ox, oy) in SAMPLE_OFFSETS {
                        let sx = px as f32 + ox;
                        let sy = py as f32 + oy;
                        for a in &phys {
                            if point_in_triangle(sx, sy, &a[0], &a[1], &a[2]) {
                                hits += 1;
                                break;
                            }
                        }
                    }
                    if hits == 0 {
                        continue;
                    }
                    let idx = (py * r.w + px) as usize * 4;
                    let mut px4 = [
                        r.buf[idx],
                        r.buf[idx + 1],
                        r.buf[idx + 2],
                        r.buf[idx + 3],
                    ];
                    blend_px(&mut px4, c, hits as f32 / SAMPLES * clip_cov);
                    r.buf[idx..idx + 4].copy_from_slice(&px4);
                }
            }
            r
        };

        let ref_r = naive(&tris);
        let mut fast = CpuRenderer::new(84.0, 30.0, 1.0);
        fast.fill_triangles(tris, Color::new(0.4, 0.8, 1.0, 0.7), Some(clip));
        let f = |r: &CpuRenderer| r.buf.iter().filter(|&&b| b != 0).count();
        if fast.buf != ref_r.buf {
            let mut first = None;
            for (i, (a, b)) in fast.buf.iter().zip(ref_r.buf.iter()).enumerate() {
                if a != b {
                    let px = (i / 4) as u32;
                    let (py, px) = (px / 84, px % 84);
                    first = Some((py, px, *a, *b));
                    break;
                }
            }
            panic!(
                "fast={} naive={} 首个差异 {:?}",
                f(&fast),
                f(&ref_r),
                first
            );
        }
    }

    /// 文本布局缓存（egui GalleyCache）：同内容同参数重复渲染零重排版。
    #[test]
    fn text_layout_cache_hits_repeat_render() {
        let Some(path) = test_font_path() else {
            return;
        };
        let engine = TextEngine::load(path).unwrap();
        let style = TextStyle::new(20.0, 24.0, FontWeight::Normal);
        let mut r = CpuRenderer::new(200.0, 50.0, 1.0);
        // 帧 1：miss（入缓存）。
        let mut s1 = Scene::default();
        s1.text(
            "缓存命中验证".to_string(),
            Rect::new(2.0, 4.0, 100.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        r.render(&engine, &s1, None);
        assert_eq!(r.layout_misses, 1, "首帧 miss");
        assert!(painted_count(&r) > 0, "首帧文本已绘制");
        // 帧 2：同内容 → 命中（miss 不增长）。
        let mut s2 = Scene::default();
        s2.text(
            "缓存命中验证".to_string(),
            Rect::new(2.0, 4.0, 100.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        r.render(&engine, &s2, None);
        assert_eq!(r.layout_misses, 1, "同内容重复渲染必须命中缓存（零重排版）");
        // 帧 3：不同内容 → miss（时钟等动态文本每变一次重排一次）。
        let mut s3 = Scene::default();
        s3.text(
            "另一段文本".to_string(),
            Rect::new(2.0, 4.0, 100.0, 24.0),
            Color::WHITE,
            style,
            TextAlign::Left,
        );
        r.render(&engine, &s3, None);
        assert_eq!(r.layout_misses, 2, "内容变化才 miss");
        // 局部 damage 帧：无文本命令。generation GC 保留「本帧 ∪ 上帧」使用条目。
        // 帧 3 用了 B（prev={B}）→ 帧 4 后 B 保留；A 连续两帧未用被淘汰。
        let mut s4 = Scene::default();
        s4.fill_rect(Color::new(1.0, 0.0, 0.0, 1.0), Rect::new(150.0, 0.0, 50.0, 50.0));
        r.render(&engine, &s4, Some(Rect::new(150.0, 0.0, 50.0, 50.0)));
        assert_eq!(r.layout_cache.len(), 1, "上帧使用窗口保留 B、淘汰久未用的 A");
        // 静态文本持续使用即持续命中：帧 5 再渲染 A → miss（被淘汰后重排一次）。
        r.render(&engine, &s1, None);
        assert_eq!(r.layout_misses, 3, "淘汰后的 A 重排一次后入缓存");
    }
}
