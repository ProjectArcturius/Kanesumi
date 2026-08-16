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

use std::collections::HashMap;
use std::sync::OnceLock;

use kanesumi_canvas::geometry::{Triangle, rounded_rect_polygon, triangulate_arc, triangulate_stroke};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Point, Rect, TextStyle};

use crate::render::{GlyphKey, layout_text_glyphs};

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
    /// 图标缓存（FNV 内容去重，同 render.rs 思路）。
    images: HashMap<u32, (Vec<u8>, u32, u32)>,
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

    /// 物理像素尺寸（SHM 提交用）。
    pub fn physical_size(&self) -> (u32, u32) {
        (self.w, self.h)
    }

    /// 把一帧 Scene 光栅化进像素缓冲，返回 RGBA bytes（每帧从透明开始）。
    pub fn render(&mut self, engine: &TextEngine, scene: &Scene) -> &[u8] {
        self.render_inner(Some(engine), scene)
    }

    /// 命令遍历实现。`engine = None` 时跳过 Text 命令（形状单测无需字体）。
    fn render_inner(&mut self, engine: Option<&TextEngine>, scene: &Scene) -> &[u8] {
        // 每帧零填充（透明底）。面板扩展区空白透出桌面（TopBar 主表面透明契约）。
        self.buf.fill(0);
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
                    let clip = effective_clip(&clip_stack);
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
                        effective_clip(&clip_stack),
                    );
                }
                SceneCommand::Arc { center, radius, thickness, color, start_deg, end_deg } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_triangles(
                        triangulate_arc(*center, *radius, *thickness, *start_deg, *end_deg),
                        *color,
                        effective_clip(&clip_stack),
                    );
                }
                SceneCommand::Triangle { p0, p1, p2, color } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    self.fill_triangles(
                        vec![Triangle::new(*p0, *p1, *p2)],
                        *color,
                        effective_clip(&clip_stack),
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
                    let text_clip = match effective_clip(&clip_stack) {
                        Some(parent) => intersect_logical(parent, *rect),
                        None => Some(*rect),
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
                        rgba,
                        *width,
                        *height,
                        *rect,
                        *tint,
                        effective_clip(&clip_stack),
                    );
                }
            }
        }
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
    fn fill_triangles(&mut self, tris: Vec<Triangle>, color: Color, clip: Option<Rect>) {
        if tris.is_empty() {
            return;
        }
        let c = [color.r, color.g, color.b, color.a];
        // 三角形 → 物理坐标 + 包围盒。
        let scale = self.scale;
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let phys: Vec<[[f32; 2]; 3]> = tris
            .iter()
            .map(|t| {
                let a = [t.p0.x * scale, t.p0.y * scale];
                let b = [t.p1.x * scale, t.p1.y * scale];
                let c2 = [t.p2.x * scale, t.p2.y * scale];
                min_x = min_x.min(a[0]).min(b[0]).min(c2[0]);
                min_y = min_y.min(a[1]).min(b[1]).min(c2[1]);
                max_x = max_x.max(a[0]).max(b[0]).max(c2[0]);
                max_y = max_y.max(a[1]).max(b[1]).max(c2[1]);
                [a, b, c2]
            })
            .collect();
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
        let px0 = bx0.floor().max(0.0) as u32;
        let py0 = by0.floor().max(0.0) as u32;
        let px1 = (bx1.ceil().min(self.w as f32)) as u32;
        let py1 = (by1.ceil().min(self.h as f32)) as u32;
        for py in py0..py1 {
            for px in px0..px1 {
                // 裁剪覆盖率（解析；clip 为轴对齐矩形）。
                let clip_cov_x =
                    (cx1.min((px + 1) as f32) - cx0.max(px as f32)).clamp(0.0, 1.0);
                let clip_cov_y =
                    (cy1.min((py + 1) as f32) - cy0.max(py as f32)).clamp(0.0, 1.0);
                let clip_cov = clip_cov_x * clip_cov_y;
                if clip_cov <= 0.0 {
                    continue;
                }
                // 4× 超采样计数。
                let mut hits = 0u8;
                for (ox, oy) in SAMPLE_OFFSETS {
                    let sx = px as f32 + ox;
                    let sy = py as f32 + oy;
                    for [a, b, c2] in &phys {
                        if point_in_triangle(sx, sy, a, b, c2) {
                            hits += 1;
                            break;
                        }
                    }
                }
                if hits == 0 {
                    continue;
                }
                let cov = hits as f32 / SAMPLES * clip_cov;
                let idx = (py * self.w + px) as usize * 4;
                let mut px4 = [self.buf[idx], self.buf[idx + 1], self.buf[idx + 2], self.buf[idx + 3]];
                blend_px(&mut px4, c, cov);
                self.buf[idx..idx + 4].copy_from_slice(&px4);
            }
        }
    }

    // ── 文本 / 图标 ──────────────────────────────────────────────────────────

    /// 文本命令 → 字形位图覆盖 blit。placement 与 GPU emit_text 共用 layout_text_glyphs。
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
        // 缓存移出 self：layout + blit 期间以局部变量借用（避免 HashMap 与 self 的
        // 借用冲突），零克隆。
        let mut cache = std::mem::take(&mut self.glyph_bitmaps);
        let placed = layout_text_glyphs(
            engine,
            &mut cache,
            content,
            rect,
            style,
            align,
            wrap,
            max_lines,
            overflow,
            scale,
        );
        let c = [color.r, color.g, color.b, color.a];
        for g in &placed {
            if let Some((_, bitmap)) = cache.get(&g.key) {
                self.blit_coverage(bitmap, g.x, g.y, g.w, g.h, c, clip);
            }
        }
        self.glyph_bitmaps = cache;
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
            .or_insert_with(|| (rgba.to_vec(), sw, sh))
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
                let s00 = texel(&src, sw, u0, v0);
                let s10 = texel(&src, sw, u1, v0);
                let s01 = texel(&src, sw, u0, v1);
                let s11 = texel(&src, sw, u1, v1);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn render_scene(scene: &Scene, w: f32, h: f32) -> CpuRenderer {
        let mut r = CpuRenderer::new(w, h, 1.0);
        // 形状测试不依赖字体：engine = None 跳过 Text 命令。
        r.render_inner(None, scene);
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
}
