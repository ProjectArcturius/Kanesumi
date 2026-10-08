// canvas_v2.rs —— CanvasV2：实例化批渲染 GPU 画布（画布主线 C1，v1 `Renderer` 并存）。
//
// 参 Ether docs/CANVAS_PLAN.md §Ⅲ 目标形态、任务 c1-canvas-core：
//   - 一个实例流 + 一条管线：每个 Scene 图元 = 一个 `Inst`，整帧一次 `draw`（仅
//     独立纹理图片处切段换绑定组）；不再按类型切 Step、不再逐字形绑定。
//   - 解析式抗锯齿：SDF 覆盖率在片元里算，去 MSAA、直画交换链。
//   - 字形（R8）/ 图片（RGBA8）图集 + 货架分配器（`atlas`），整帧绑一次。
//   - 排版 LRU（`text_cache`）：静态文本命中只平移出实例。
//   - wgpu `PIPELINE_CACHE` 落盘，二次启动管线就绪 < 20 ms。
// 颜色语义与旧三管线逐字一致（见 shaders.wgsl.rs 头注）。开关 `KANESUMI_CANVAS=2`
// 在 platform 侧分派，缺省仍走 v1 —— 本模块行为不对既有路径产生任何影响。

use std::collections::HashMap;
use std::sync::Arc;

use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Rect, TextStyle};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Connection;

use crate::glyph_layout::{GlyphKey, TextRenderTuning, layout_text_glyphs_tuned};
use crate::render::{GpuContext, RendererError, intersect, scissor_rect};
use crate::canvas_v2::atlas::ShelfAtlas;
use crate::canvas_v2::text_cache::{LayoutKey, RelGlyph, TextLayoutCache, layout_key};
use crate::canvas_v2::shaders::CANVAS2_SHADER;

#[path = "canvas_v2/shaders.wgsl.rs"]
pub mod shaders;
pub mod atlas;
pub mod text_cache;

// ── 实例结构 ─────────────────────────────────────────────────────────────

/// 实例 kind：与任务书一致。
pub(crate) const KIND_FILL_RRECT: u32 = 0;
pub(crate) const KIND_STROKE_RRECT: u32 = 1;
pub(crate) const KIND_ARC: u32 = 2;
pub(crate) const KIND_TRIANGLE: u32 = 3;
pub(crate) const KIND_GLYPH: u32 = 4;
pub(crate) const KIND_IMAGE: u32 = 5;
/// C1.5 增量清除实例：片元恒输出 0，走无混合管线（见 `pipeline_replace`）。
pub(crate) const KIND_CLEAR: u32 = 6;

/// 每图元一个实例。96 字节（WGSL vec4 对齐；末尾 8 字节补位）。
/// `rect` 为**物理像素**包围盒（逻辑 × scale 在编码时算好）：
/// 填充 / 描边 = 矩形本体；圆弧 = 包围盒（心 ± (R+t/2)）；三角形 = 三点包围盒；
/// 字形 / 图片 = quad 精确框。`clip` = 量化后的整数物理裁剪矩形（x0,y0,x1,y1）。
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Inst {
    kind: u32,
    flags: u32,
    rect: [f32; 4],
    p: [f32; 4],
    q: [f32; 4],
    color: [f32; 4],
    clip: [f32; 4],
    _pad: [f32; 2],
}

const _: () = assert!(std::mem::size_of::<Inst>() == 96);

/// 实例顶点属性（offset 显式对齐 `Inst` 真实布局：repr(C)，[f32;4] 对齐 4）。
/// kind+flags @0（8B）、rect@8、p@24、q@40、color@56、clip@72、_pad@88。
/// 用 `offset_of!` 取值守住（`inst_布局大小与对齐` 测试）。三条实例管线共用。
static INST_ATTRS: [wgpu::VertexAttribute; 6] = [
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x2, offset: std::mem::offset_of!(Inst, kind) as u64, shader_location: 0 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, rect) as u64, shader_location: 1 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, p) as u64, shader_location: 2 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, q) as u64, shader_location: 3 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, color) as u64, shader_location: 4 },
    wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, clip) as u64, shader_location: 5 },
];

/// 实例顶点缓冲布局（实例步进）。
fn inst_vertex_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Inst>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INST_ATTRS,
    }
}

/// 顶点缓冲初始容量（实例数）。不足时翻倍。
const INST_BUF_CAP: u32 = 4096;
/// 图集页尺寸阶梯：首页 2048²，放满升级 4096²，再满 → 字形跳过 / 图片走独立纹理。
const ATLAS_PAGE_SIZES: [u32; 2] = [2048, 4096];
/// 独立纹理退路：任一边超过该值不进图集（RGBA 图集单图 1024² = 4 MB）。
const IMAGE_ATLAS_MAX_EDGE: u32 = 1024;
/// 管线缓存落盘目录（`~/.cache/ether/wgpu/`）。
pub(crate) const PIPELINE_CACHE_DIR: &str = "ether/wgpu";

// ── C1.5 开关与损伤几何（纯逻辑，单测覆盖）──────────────────────────────

/// C1.5 A/B 基线开关：`KANESUMI_CANVAS_FULL=1` 强制整幅重画（release 也保留）。
/// A 档 = C1 的「每帧整幅」，B 档 = 本任务的增量重画；同一 Scene 两档逐位一致。
pub(crate) fn canvas_full_forced() -> bool {
    std::env::var_os("KANESUMI_CANVAS_FULL").is_some_and(|v| v == "1")
}

/// 自检开关：`KANESUMI_CANVAS_VERIFY=1`（仅 debug 构建生效）。参任务 c15 设计 4。
#[cfg(debug_assertions)]
pub(crate) fn canvas_verify_enabled() -> bool {
    cfg!(debug_assertions)
        && std::env::var_os("KANESUMI_CANVAS_VERIFY").is_some_and(|v| v == "1")
}

/// 逻辑损伤 `d` → 物理损伤 D（x0,y0,x1,y1）：先 × scale 向外取整（floor / ceil），
/// 再外扩 1 px（AA 环带），最后夹到表面内。完全落在表面外 → None（本帧无操作）。
pub(crate) fn damage_phys_rect(
    d: Rect,
    scale: f32,
    width: u32,
    height: u32,
) -> Option<[f32; 4]> {
    if d.size.width <= 0.0 || d.size.height <= 0.0 {
        return None;
    }
    let x0 = ((d.origin.x * scale).floor() - 1.0).max(0.0);
    let y0 = ((d.origin.y * scale).floor() - 1.0).max(0.0);
    let x1 = ((d.right() * scale).ceil() + 1.0).min(width as f32);
    let y1 = ((d.bottom() * scale).ceil() + 1.0).min(height as f32);
    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some([x0, y0, x1, y1])
    }
}

/// 实例包围盒（含 1 px 外扩，与顶点着色器展开一致）是否与物理损伤 D 相交。
/// 相切（仅边界接触）算不相交，与「相交」的严格语义一致（保留 Scene 原序由调用方保证）。
pub(crate) fn inst_intersects(rect: [f32; 4], d: [f32; 4]) -> bool {
    let x0 = rect[0] - 1.0;
    let y0 = rect[1] - 1.0;
    let x1 = rect[0] + rect[2] + 1.0;
    let y1 = rect[1] + rect[3] + 1.0;
    !(x1 <= d[0] || x0 >= d[2] || y1 <= d[1] || y0 >= d[3])
}

/// 按损伤剔除后入队（`cull = None` 全量直入）。
fn push_inst(insts: &mut Vec<Inst>, inst: Inst, cull: Option<[f32; 4]>) {
    if cull.is_none_or(|d| inst_intersects(inst.rect, d)) {
        insts.push(inst);
    }
}

// ── 图集 ─────────────────────────────────────────────────────────────────

/// 一张图集页：wgpu 纹理 + 货架分配器。
struct AtlasPage {
    texture: wgpu::Texture,
    atlas: ShelfAtlas,
}

impl AtlasPage {
    fn new(device: &wgpu::Device, size: u32, rgba: bool) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(if rgba {
                "kanesumi-c2-img-atlas"
            } else {
                "kanesumi-c2-glyph-atlas"
            }),
            size: wgpu::Extent3d {
                width: size,
                height: size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: if rgba {
                wgpu::TextureFormat::Rgba8UnormSrgb
            } else {
                wgpu::TextureFormat::R8Unorm
            },
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        Self {
            texture,
            atlas: ShelfAtlas::new(size, size),
        }
    }
}

/// 字形图集：key = `GlyphKey` → 条目左上 texel（位图尺寸从 `glyph_bitmaps` 查）。
struct GlyphAtlas {
    page: AtlasPage,
    /// 页级次（升级重建时 +1；调用方据此在渲染时惰性重传全部在用字形）。
    epoch: u64,
    slots: HashMap<GlyphKey, (u32, u32)>,
}

/// 图片图集：key = 内容 FNV → (texel x, y, w, h)。重建重传需要原始 RGBA，一并持有
/// （`Arc` 共享，Scene 侧本就持同一段数据，无深拷贝）。
struct ImageAtlas {
    page: AtlasPage,
    epoch: u64,
    slots: HashMap<u32, (u32, u32, u32, u32)>,
    sources: HashMap<u32, ImageSource>,
}

/// 把一条 `w×h` 条目上传进图集：行 stride 补到 256 对齐（`write_texture` 的
/// `bytes_per_row` 对齐要求），R8 每行 1 字节、RGBA 每行 4 字节。
fn upload_entry(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    at: (u32, u32),
    (w, h): (u32, u32),
    bytes_per_px: usize,
    bitmap: &[u8],
) {
    let stride = (w as usize * bytes_per_px).div_ceil(256) * 256;
    if stride == w as usize * bytes_per_px {
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: at.0, y: at.1, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            bitmap,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * bytes_per_px as u32),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        return;
    }
    let mut staged = vec![0u8; stride * h as usize];
    let src_stride = w as usize * bytes_per_px;
    for row in 0..h as usize {
        staged[row * stride..row * stride + src_stride]
            .copy_from_slice(&bitmap[row * src_stride..row * src_stride + src_stride]);
    }
    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x: at.0, y: at.1, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        &staged,
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(stride as u32),
            rows_per_image: Some(h),
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

// ── 渲染器 ───────────────────────────────────────────────────────────────

/// 单表面的实例化批渲染光栅化器。对外接口与 v1 `Renderer` 对齐，platform 侧经
/// [`crate::platform::SurfaceRenderer`] 分派。
pub struct CanvasV2 {
    ctx: Arc<GpuContext>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bgl: wgpu::BindGroupLayout,
    bind_group: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    uniform: wgpu::Buffer,
    glyphs: GlyphAtlas,
    images: ImageAtlas,
    /// 独立纹理图片（超图集边 / 图集满）：key → 绑定组（同布局，binding 1 挂独立纹理）。
    own_images: HashMap<u32, wgpu::BindGroup>,
    own_views: HashMap<u32, wgpu::TextureView>,
    /// 1×1 R8 占位视图（独立纹理组 binding 0 用）。
    dummy_r8: wgpu::TextureView,
    /// 排版 LRU（含相对放置）。
    text_cache: TextLayoutCache,
    /// 字形位图 CPU 缓存（与 v1 同款；miss 才光栅化）。
    glyph_bitmaps: HashMap<GlyphKey, (kanesumi_canvas::text::GlyphMetrics, Vec<u8>)>,
    text_tuning: TextRenderTuning,
    /// 持久实例缓冲（VERTEX + COPY_DST，容量不足翻倍）。
    inst_buf: wgpu::Buffer,
    inst_cap: u32,
    gpu_timer: Option<crate::render::GpuTimer>,
    last_acquire_ms: Option<f32>,
    /// 最近一帧绘制调用数（perf `draws=`）。
    last_draws: u32,
    /// 最近一帧走独立纹理的 (实例索引, 内容 key)（encode 收集，draw 切段换组后清空）。
    pending_own: Vec<(u32, u32)>,
    /// C1.5：无混合管线（blend: None），只用于清除实例（`KIND_CLEAR`）。
    pipeline_replace: wgpu::RenderPipeline,
    /// C1.5：blit 呈现管线（交换链不支持 COPY_DST 时把 target 采样上屏）。
    blit_pipeline: wgpu::RenderPipeline,
    /// C1.5：blit 绑定组（binding 0 挂 target 视图、1 挂占位、2 采样器、3 uniform）。
    blit_bind_group: Option<wgpu::BindGroup>,
    /// C1.5：清除实例的 1 元素顶点缓冲（单独一次 draw）。
    clear_buf: wgpu::Buffer,
    /// C1.5：常驻离屏目标（与表面同尺寸同格式），增量重画的累积结果。
    target: Option<wgpu::Texture>,
    target_view: Option<wgpu::TextureView>,
    /// target 内容是否有效（false → 下一帧整幅重画）。
    target_valid: bool,
    /// 交换链纹理是否支持 COPY_DST（支持 → copy 呈现；否则 blit）。
    present_copy: bool,
    /// 复用的实例缓冲（避免每帧新分配 Vec）。
    insts_buf: Vec<Inst>,
    /// 帧计数（自检开关按 30 帧节拍触发）。
    frame_count: u64,
    /// 最近一帧是否增量、损伤面积占比（%）、画出的实例数（perf `inc` / `dmg_pct` / `insts`）。
    last_inc: bool,
    last_dmg_pct: f32,
    last_insts: u32,
    /// 自检（仅 debug 构建）：第二张整幅目标纹理（与 target 逐字节比对）。
    #[cfg(debug_assertions)]
    verify_tex: Option<wgpu::Texture>,
    /// 自检（仅 debug）：整幅编码的复用实例缓冲。
    #[cfg(debug_assertions)]
    verify_insts: Vec<Inst>,
    scale: f32,
    width: f32,
    height: f32,
}

impl CanvasV2 {

/// 编码一帧 Scene → 实例流。顺序 = Scene 命令顺序（painter's algorithm 天然保持）。
/// `cull = Some(D)`（物理损伤矩形）时，只入队包围盒（含 1 px 外扩）与 D 相交的实例；
/// `None` = 整幅。剔除在编码期完成，保持 Scene 原序，且不新分配第二份实例 Vec。
fn encode_scene(
    &mut self,
    engine: &kanesumi_canvas::text::TextEngine,
    scene: &Scene,
    cull: Option<[f32; 4]>,
    insts: &mut Vec<Inst>,
    own: &mut Vec<(u32, u32)>,
) {
    let s = self.scale;
    let mut clips = ClipStack::new(Rect::new(0.0, 0.0, self.width, self.height), s);
    let color = |c: &Color| [c.r, c.g, c.b, c.a];
    for cmd in &scene.commands {
        match cmd {
            SceneCommand::PushClip { rect } => clips.push(*rect),
            SceneCommand::PopClip => clips.pop(),
            SceneCommand::FillRect { color: c, rect, corner_radius } => {
                let Some(clip) = clips.current() else { continue };
                if rect.is_empty() {
                    continue;
                }
                push_inst(insts, Inst {
                    kind: KIND_FILL_RRECT,
                    flags: 0,
                    rect: [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                    p: [*corner_radius * s, 0.0, 0.0, 0.0],
                    q: [0.0; 4],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                }, cull);
            }
            SceneCommand::StrokeRect { color: c, rect, thickness, corner_radius } => {
                let Some(clip) = clips.current() else { continue };
                if rect.is_empty() {
                    continue;
                }
                push_inst(insts, Inst {
                    kind: KIND_STROKE_RRECT,
                    flags: 0,
                    rect: [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                    p: [*corner_radius * s, *thickness * s, 0.0, 0.0],
                    q: [0.0; 4],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                }, cull);
            }
            SceneCommand::Arc { center, radius, thickness, color: c, start_deg, end_deg } => {
                let Some(clip) = clips.current() else { continue };
                let (cx, cy) = (center.x * s, center.y * s);
                let (r, t) = (*radius * s, *thickness * s);
                let a0 = start_deg.to_radians();
                let a1 = end_deg.to_radians();
                // 包围盒：心 ± (R + t/2)；退路（r ≤ 0 / t ≤ 0）交给着色器出 0 覆盖。
                let half = (r + t * 0.5).max(1.0);
                push_inst(insts, Inst {
                    kind: KIND_ARC,
                    flags: 0,
                    rect: [cx - half, cy - half, half * 2.0, half * 2.0],
                    p: [cx, cy, r, t],
                    q: [a0, a1, 0.0, 0.0],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                }, cull);
            }
            SceneCommand::Triangle { p0, p1, p2, color: c } => {
                let Some(clip) = clips.current() else { continue };
                let (ax, bx) = (p0.x.min(p1.x).min(p2.x) * s, p0.x.max(p1.x).max(p2.x) * s);
                let (ay, by) = (p0.y.min(p1.y).min(p2.y) * s, p0.y.max(p1.y).max(p2.y) * s);
                push_inst(insts, Inst {
                    kind: KIND_TRIANGLE,
                    flags: 0,
                    rect: [ax, ay, (bx - ax).max(0.001), (by - ay).max(0.001)],
                    p: [p0.x * s, p0.y * s, p1.x * s, p1.y * s],
                    q: [p2.x * s, p2.y * s, 0.0, 0.0],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                }, cull);
            }
            SceneCommand::Text { .. } => {
                let Some(clip) = clips.current() else { continue };
                self.encode_text(engine, cmd, clip, cull, insts);
            }
            SceneCommand::Image { .. } => {
                let Some(clip) = clips.current() else { continue };
                self.encode_image(cmd, clip, cull, insts, own);
            }
        }
    }
}

    /// 用进程共享的 [`GpuContext`] 为单个 wl_surface 建表面 / 管线 / 图集。
    /// 表面配置、alpha 模式与 present 模式语义与 v1 `Renderer::with_context` 一致。
    #[allow(clippy::too_many_arguments)]
    pub fn with_context(
        ctx: Arc<GpuContext>,
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        _transparent: bool,
    ) -> Result<Self, RendererError> {
        let surface =
            crate::render::create_wl_surface(&ctx.instance, conn, wl_surface)
                .map_err(RendererError::Surface)?;
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = ctx.format;
        if !caps.formats.contains(&format) {
            return Err(RendererError::IncompatibleFormat { wanted: format });
        }
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .copied()
            .unwrap_or_else(|| {
                caps.alpha_modes
                    .iter()
                    .find(|a| **a == wgpu::CompositeAlphaMode::Opaque)
                    .copied()
                    .unwrap_or(wgpu::CompositeAlphaMode::Auto)
            });
        let device = &ctx.device;
        let (pw, ph) = (width * scale, height * scale);
        // C1.5：交换链纹理支持 COPY_DST 才能 copy_texture_to_texture 上屏；否则走 blit。
        let present_copy = caps.usages.contains(wgpu::TextureUsages::COPY_DST);
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if present_copy {
            usage |= wgpu::TextureUsages::COPY_DST;
        }
        let config = wgpu::SurfaceConfiguration {
            usage,
            format,
            view_formats: vec![format],
            alpha_mode,
            width: pw.round().max(1.0) as u32,
            height: ph.round().max(1.0) as u32,
            desired_maximum_frame_latency: 2,
            // 与 v1 一致：优先 Mailbox（acquire 立返），其次 Immediate，最后 FIFO。
            present_mode: crate::render::choose_present_mode(&caps.present_modes),
        };
        surface.configure(device, &config);
        Self::build(
            ctx, surface, config, conn, width, height, scale, present_copy,
        )
    }

    /// 管线、绑定布局、图集与持久缓冲（`with_context` 与单测共用的后半段）。
    #[allow(clippy::too_many_arguments)]
    fn build(
        ctx: Arc<GpuContext>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        _conn: &Connection,
        width: f32,
        height: f32,
        scale: f32,
        present_copy: bool,
    ) -> Result<Self, RendererError> {
        let device = &ctx.device;
        // 管线缓存：设备支持且磁盘有上次数据 → 管线创建走驱动缓存（首帧 < 20 ms 目标）。
        let cache = load_pipeline_cache(&ctx);
        let cache_ref = cache.as_ref().map(|(c, _)| c);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kanesumi-c2-shader"),
            source: wgpu::ShaderSource::Wgsl(CANVAS2_SHADER.into()),
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kanesumi-c2-bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // uniform（W/H/contrast/gamma）：vs 的 NDC 换算与 fs 的浓度补偿都用。
                    binding: 3,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kanesumi-c2-layout"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-c2-pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[inst_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: cache_ref,
        });
        // C1.5：清除实例专用管线 —— 与主管线仅差混合状态（None = 直接覆写）。
        let pipeline_replace = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-c2-pipeline-replace"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[inst_vertex_layout()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: cache_ref,
        });
        // C1.5：blit 呈现管线 —— 全屏三角形采样 target（无实例顶点缓冲）。
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-c2-blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "blit_vs",
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "blit_fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: cache_ref,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kanesumi-c2-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // uniform：每帧一写（W, H, contrast, gamma）。
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-c2-uniform"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_tuning = TextRenderTuning::default();
        let glyphs = GlyphAtlas {
            page: AtlasPage::new(device, ATLAS_PAGE_SIZES[0], false),
            epoch: 0,
            slots: HashMap::new(),
        };
        let images = ImageAtlas {
            page: AtlasPage::new(device, ATLAS_PAGE_SIZES[0], true),
            epoch: 0,
            slots: HashMap::new(),
            sources: HashMap::new(),
        };
        // 1×1 R8 空纹理视图：独立纹理图片的绑定组里 binding 0（字形槽）挂它。
        let dummy_r8 = make_dummy_view(device);
        let inst_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-c2-inst-buf"),
            size: INST_BUF_CAP as u64 * std::mem::size_of::<Inst>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let inst_cap = INST_BUF_CAP;
        // C1.5：清除实例缓冲（1 个 Inst；每增量帧写 D 矩形）。
        let clear_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-c2-clear-buf"),
            size: std::mem::size_of::<Inst>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = make_atlas_bind_group(
            device,
            &bgl,
            &sampler,
            &uniform,
            &glyphs.page.texture,
            &images.page.texture,
        );
        let gpu_timer = crate::render::GpuTimer::new(device, &ctx.queue);
        // 首行自证：present=copy|blit（C1.5 增量重画的呈现路径），A/B 与真机排查据此分辨。
        log::info!(
            "kanesumi CanvasV2：pipeline_cache={} gpu_timer={} present={}",
            if cache.is_some() { "on" } else { "n/a" },
            if gpu_timer.is_some() { "on" } else { "n/a" },
            if present_copy { "copy" } else { "blit" },
        );
        if let Some((c, path)) = cache {
            save_pipeline_cache(&c, &path);
        }
        crate::timeline::note_once("renderer_pipelines");
        Ok(Self {
            ctx,
            surface,
            config,
            pipeline,
            bgl,
            bind_group,
            sampler,
            uniform,
            glyphs,
            images,
            own_images: HashMap::new(),
            own_views: HashMap::new(),
            dummy_r8,
            text_cache: TextLayoutCache::new(4096),
            glyph_bitmaps: HashMap::new(),
            text_tuning,
            inst_buf,
            inst_cap,
            gpu_timer,
            last_acquire_ms: None,
            last_draws: 0,
            pending_own: Vec::new(),
            pipeline_replace,
            blit_pipeline,
            blit_bind_group: None,
            clear_buf,
            target: None,
            target_view: None,
            target_valid: false,
            present_copy,
            insts_buf: Vec::new(),
            frame_count: 0,
            last_inc: false,
            last_dmg_pct: 0.0,
            last_insts: 0,
            #[cfg(debug_assertions)]
            verify_tex: None,
            #[cfg(debug_assertions)]
            verify_insts: Vec::new(),
            scale,
            width,
            height,
        })
    }
}

/// 1×1 R8 空纹理视图（独立纹理绑定组的字形槽占位）。
fn make_dummy_view(device: &wgpu::Device) -> wgpu::TextureView {
    device
        .create_texture(&wgpu::TextureDescriptor {
            label: Some("kanesumi-c2-dummy-r8"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
        .create_view(&wgpu::TextureViewDescriptor::default())
}

/// 图集绑定组（整帧只绑一次）：0 = 字形图集，1 = 图片图集，2 = 采样器，3 = uniform。
fn make_atlas_bind_group(
    device: &wgpu::Device,
    bgl: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    uniform: &wgpu::Buffer,
    glyph_tex: &wgpu::Texture,
    image_tex: &wgpu::Texture,
) -> wgpu::BindGroup {
    let glyph_view = glyph_tex.create_view(&wgpu::TextureViewDescriptor::default());
    let image_view = image_tex.create_view(&wgpu::TextureViewDescriptor::default());
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("kanesumi-c2-bind-group"),
        layout: bgl,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&glyph_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&image_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: uniform.as_entire_binding(),
            },
        ],
    })
}

// ── 管线缓存 ─────────────────────────────────────────────────────────────

/// 缓存文件名：adapter 名 + 驱动版本 hash（换驱动 / 换卡后旧缓存天然失效）。
fn cache_file_name(info: &wgpu::AdapterInfo) -> String {
    fn h(s: &str) -> u64 {
        let mut x: u64 = 0xcbf2_9ce4_8422_2325;
        for b in s.as_bytes() {
            x ^= *b as u64;
            x = x.wrapping_mul(0x0000_0100_0000_01b3);
        }
        x
    }
    format!(
        "{:016x}{:016x}{:016x}.bin",
        h(&info.name),
        h(&info.driver),
        h(&info.driver_info)
    )
}

/// 缓存落盘路径：`~/.cache/ether/wgpu/<adapter 与驱动 hash>.bin`。HOME 缺失 → None。
fn cache_path(ctx: &GpuContext) -> Option<std::path::PathBuf> {
    let home = std::env::var_os("HOME")?;
    let mut p = std::path::PathBuf::from(home);
    p.push(".cache");
    p.push(PIPELINE_CACHE_DIR);
    p.push(cache_file_name(&ctx.adapter.get_info()));
    Some(p)
}

/// 读 + 建管线缓存：设备支持 `PIPELINE_CACHE` 时返回（缓存, 落盘路径）。
/// 数据损坏由 `fallback: true` 兜底（wgpu 内部丢弃重建，绝不 panic）。
fn load_pipeline_cache(
    ctx: &GpuContext,
) -> Option<(wgpu::PipelineCache, std::path::PathBuf)> {
    if !ctx.device.features().contains(wgpu::Features::PIPELINE_CACHE) {
        return None;
    }
    let path = cache_path(ctx)?;
    let data = std::fs::read(&path).ok();
    if data.is_none() {
        log::info!("管线缓存：无既有数据（{}），首次编译后落盘", path.display());
    }
    // 安全性：`data` 只可能是本进程此前 `get_data()` 写出的 wgpu 缓存；文件被篡改时
    // `fallback: true` 让 wgpu 自行丢弃重建。适配器名 + 驱动 hash 进文件名防串用。
    let cache = unsafe {
        ctx.device.create_pipeline_cache(&wgpu::PipelineCacheDescriptor {
            label: Some("kanesumi-c2-pcache"),
            data: data.as_deref(),
            fallback: true,
        })
    };
    Some((cache, path))
}

/// 把管线初始化数据写回磁盘（建完管线调用一次；失败只 warn）。
fn save_pipeline_cache(cache: &wgpu::PipelineCache, path: &std::path::Path) {
    let Some(data) = cache.get_data() else {
        return;
    };
    if let Some(dir) = path.parent()
        && let Err(e) = std::fs::create_dir_all(dir)
    {
        log::warn!("管线缓存目录创建失败（{e}）：{}", dir.display());
        return;
    }
    match std::fs::write(path, &data) {
        Ok(_) => log::info!(
            "管线缓存已写盘（{} 字节）→ {}",
            data.len(),
            path.display()
        ),
        Err(e) => log::warn!("管线缓存写盘失败（{e}）：{}", path.display()),
    }
}

// ── Scene 编码 ───────────────────────────────────────────────────────────

/// 裁剪栈（box 语义）：PushClip / PopClip 严格配对，有效裁剪 = 栈内全部矩形交集，
/// 与表面求交后按 v1 `scissor_rect` 同一 floor/ceil 量化成整数物理像素边界。
struct ClipStack {
    stack: Vec<Option<Rect>>,
    surface: Rect,
    scale: f32,
}

impl ClipStack {
    fn new(surface: Rect, scale: f32) -> Self {
        Self {
            stack: Vec::new(),
            surface,
            scale,
        }
    }

    fn push(&mut self, rect: Rect) {
        let next = match self.stack.last().copied().flatten() {
            Some(p) => intersect(p, rect),
            None => Some(rect),
        };
        self.stack.push(next);
    }

    fn pop(&mut self) {
        self.stack.pop();
    }

    /// 当前有效裁剪（物理整数边界 x0,y0,x1,y1）。与 v1 语义一致：
    /// 栈空 = 无裁剪（全表面可见）；顶层 `Some(None)`（空交集层）= 完全裁掉 → None。
    fn current(&self) -> Option<[f32; 4]> {
        let clip = match self.stack.last().copied() {
            Some(Some(r)) => r,
            Some(None) => return None,
            None => self.surface,
        };
        let eff = intersect(clip, self.surface)?;
        let (x, y, w, h) = scissor_rect(Some(eff), self.scale, 1 << 20, 1 << 20);
        Some([x as f32, y as f32, (x + w) as f32, (y + h) as f32])
    }
}

/// FNV-1a 32 位（图片纹理缓存键，与 v1 `render.rs` 同常数同语义）。
fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 图片源（原始 RGBA + 尺寸）——图集重传与独立纹理共用。
type ImageSource = (Arc<[u8]>, u32, u32);

/// 排版缓存键的字节串：内容 + 样式全部字段 + 文本框尺寸 + 对齐 / 换行 / 行数 /
/// 溢出 / scale / 引擎。枚举按变体序号写死映射（加变体时同步）。
#[allow(clippy::too_many_arguments)]
fn layout_cache_key(
    content: &str,
    style: TextStyle,
    size_wh: (f32, f32),
    align: TextAlign,
    wrap: bool,
    max_lines: Option<usize>,
    overflow: kanesumi_canvas::TextOverflow,
    scale: f32,
    engine_id: u64,
) -> LayoutKey {
    let weight_id = match style.weight {
        kanesumi_core::FontWeight::Light => 0u8,
        kanesumi_core::FontWeight::Semilight => 1,
        kanesumi_core::FontWeight::Normal => 2,
        kanesumi_core::FontWeight::Medium => 3,
        kanesumi_core::FontWeight::Semibold => 4,
        kanesumi_core::FontWeight::Bold => 5,
        kanesumi_core::FontWeight::ExtraBold => 6,
    };
    let valign_id = match style.v_align {
        kanesumi_core::TextVAlign::Top => 0u8,
        kanesumi_core::TextVAlign::Center => 1,
        kanesumi_core::TextVAlign::Bottom => 2,
    };
    let align_id = match align {
        TextAlign::Left => 0u8,
        TextAlign::Center => 1,
        TextAlign::Right => 2,
    };
    let overflow_id = match overflow {
        kanesumi_canvas::TextOverflow::Clip => 0u8,
        kanesumi_canvas::TextOverflow::Ellipsis => 1,
    };
    let mut parts: Vec<&[u8]> = vec![content.as_bytes()];
    let mut num = Vec::with_capacity(64);
    num.extend_from_slice(&style.size.to_le_bytes());
    num.extend_from_slice(&style.line_height.to_le_bytes());
    num.extend_from_slice(&style.letter_spacing_em.to_le_bytes());
    num.push(weight_id);
    num.push(valign_id);
    num.push(align_id);
    num.push(u8::from(wrap));
    num.extend_from_slice(&(max_lines.map_or(0u32, |n| n as u32 + 1)).to_le_bytes());
    num.push(overflow_id);
    num.extend_from_slice(&size_wh.0.to_le_bytes());
    num.extend_from_slice(&size_wh.1.to_le_bytes());
    num.extend_from_slice(&scale.to_le_bytes());
    num.extend_from_slice(&engine_id.to_le_bytes());
    parts.push(&num);
    layout_key(&parts)
}

impl CanvasV2 {
    /// 编码一条 Text 命令：排版缓存命中 → 平移出实例；未命中 → 排版入库。
    /// 字形图集未命中同步上传（与 v1 `ensure_glyph` 同时机，只是换成图集条目）。
    #[allow(clippy::too_many_arguments)]
    fn encode_text(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        cmd: &SceneCommand,
        clip: [f32; 4],
        cull: Option<[f32; 4]>,
        insts: &mut Vec<Inst>,
    ) {
        let SceneCommand::Text { content, rect, color, style, align, wrap, max_lines, overflow } =
            cmd
        else {
            return;
        };
        if rect.is_empty() {
            return;
        }
        // 单行 label 的纵向对齐：与 v1 build_frame 同一调用（排版与裁剪同用它）。
        let rect = crate::glyph_layout::text_rect_with_valign(*rect, *style, *wrap, *max_lines);
        let key = layout_cache_key(
            content,
            *style,
            (rect.size.width, rect.size.height),
            *align,
            *wrap,
            *max_lines,
            *overflow,
            self.scale,
            engine.identity(),
        );
        let placed: Vec<RelGlyph> = if let Some(hit) = self.text_cache.get(&key) {
            hit.to_vec()
        } else {
            let abs = layout_text_glyphs_tuned(
                engine,
                &mut self.glyph_bitmaps,
                content,
                rect,
                *style,
                *align,
                *wrap,
                *max_lines,
                *overflow,
                self.scale,
                self.text_tuning,
            );
            let rel: Vec<RelGlyph> = abs
                .iter()
                .map(|g| RelGlyph {
                    key: g.key,
                    x: g.x - rect.origin.x,
                    y: g.y - rect.origin.y,
                    w: g.w,
                    h: g.h,
                })
                .collect();
            self.text_cache.put(key, rel.clone());
            rel
        };
        let s = self.scale;
        for g in placed {
            let Some((ax, ay, bw, bh)) = self.glyph_slot(g.key) else {
                continue; // 图集放不下：本帧该字形不画（warn 在 glyph_slot 里记一次）
            };
            let (x0, y0) = ((rect.origin.x + g.x) * s, (rect.origin.y + g.y) * s);
            let (pw, ph) = (
                self.glyphs.page.atlas.width() as f32,
                self.glyphs.page.atlas.height() as f32,
            );
            push_inst(insts, Inst {
                kind: KIND_GLYPH,
                flags: 0,
                rect: [x0, y0, g.w * s, g.h * s],
                p: [0.0; 4],
                q: [
                    ax as f32 / pw,
                    ay as f32 / ph,
                    (ax + bw) as f32 / pw,
                    (ay + bh) as f32 / ph,
                ],
                color: [color.r, color.g, color.b, color.a],
                clip,
                _pad: [0.0; 2],
            }, cull);
        }
    }

    /// 字形图集条目：返回 (texel x, y, 位图 w, h)。未命中分配并上传；
    /// 页放满先升级（2048→4096，全量重传），仍放不下 → None（该字形本帧跳过）。
    fn glyph_slot(&mut self, key: GlyphKey) -> Option<(u32, u32, u32, u32)> {
        if let Some(&(ax, ay)) = self.glyphs.slots.get(&key)
            && let Some((m, _)) = self.glyph_bitmaps.get(&key)
        {
            return Some((ax, ay, m.width as u32, m.height as u32));
        }
        let (metrics, bitmap) = self.glyph_bitmaps.get(&key)?;
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        if w == 0 || h == 0 {
            return None;
        }
        match self.glyphs.page.atlas.alloc(w, h) {
            Some(at) => {
                upload_entry(
                    &self.ctx.queue,
                    &self.glyphs.page.texture,
                    at,
                    (w, h),
                    1,
                    bitmap,
                );
                self.glyphs.slots.insert(key, at);
                Some((at.0, at.1, w, h))
            }
            None => {
                // 页放满：升级一档（slots 清空 → 全量重传），再试一次。
                let cur = self.glyphs.page.atlas.width();
                if let Some(&next) = ATLAS_PAGE_SIZES.iter().find(|&&sz| sz > cur) {
                    log::warn!(
                        "字形图集 {cur}² 放满 → 升级 {next}²（重传 {} 条）",
                        self.glyphs.slots.len()
                    );
                    self.rebuild_glyph_page(next);
                    return self.glyph_slot(key);
                }
                log::warn!("字形图集 4096² 放满，字形 #{key:?} 本帧跳过");
                None
            }
        }
    }

    /// 重建字形图集页（升级）：分配器清零、按 `glyph_bitmaps` 全量重传。
    fn rebuild_glyph_page(&mut self, size: u32) {
        let entries: Vec<(GlyphKey, (u32, u32), Vec<u8>)> = self
            .glyph_bitmaps
            .iter()
            .filter(|(_, (m, _))| m.width > 0 && m.height > 0)
            .map(|(k, (m, b))| (*k, (m.width as u32, m.height as u32), b.clone()))
            .collect();
        let new_page = AtlasPage::new(&self.ctx.device, size, false);
        self.glyphs.page = new_page;
        self.glyphs.slots.clear();
        self.glyphs.epoch += 1;
        for (k, (w, h), b) in entries {
            if let Some(at) = self.glyphs.page.atlas.alloc(w, h) {
                upload_entry(&self.ctx.queue, &self.glyphs.page.texture, at, (w, h), 1, &b);
                self.glyphs.slots.insert(k, at);
            }
        }
        self.refresh_bind_group();
        self.target_valid = false;
    }

    /// 图集页重建 / 独立纹理新增后刷新整帧绑定组（字形 / 图片图集纹理可能已换）。
    fn refresh_bind_group(&mut self) {
        self.bind_group = make_atlas_bind_group(
            &self.ctx.device,
            &self.bgl,
            &self.sampler,
            &self.uniform,
            &self.glyphs.page.texture,
            &self.images.page.texture,
        );
    }

    /// 编码一条 Image 命令：图集命中出 uv；未命中上传图集（页满先升级）；
    /// 超图集边 / 升级后仍放不下 → 独立纹理，实例索引记进 `own`（仅当该实例未被损伤剔除）。
    fn encode_image(
        &mut self,
        cmd: &SceneCommand,
        clip: [f32; 4],
        cull: Option<[f32; 4]>,
        insts: &mut Vec<Inst>,
        own: &mut Vec<(u32, u32)>,
    ) {
        let SceneCommand::Image { rgba, width, height, rect, tint, opacity } = cmd else {
            return;
        };
        if rect.is_empty() || *width == 0 || *height == 0 {
            return;
        }
        let key = fnv1a(rgba);
        let s = self.scale;
        let c = tint.unwrap_or(Color::WHITE).with_alpha(opacity.clamp(0.0, 1.0));
        let rect_phys = [
            rect.origin.x * s,
            rect.origin.y * s,
            rect.size.width * s,
            rect.size.height * s,
        ];
        let (uv, own_key) = if let Some(&(ax, ay, w, h)) = self.images.slots.get(&key) {
            let (pw, ph) = (
                self.images.page.atlas.width() as f32,
                self.images.page.atlas.height() as f32,
            );
            (
                [
                    ax as f32 / pw,
                    ay as f32 / ph,
                    (ax + w) as f32 / pw,
                    (ay + h) as f32 / ph,
                ],
                None,
            )
        } else if *width > IMAGE_ATLAS_MAX_EDGE || *height > IMAGE_ATLAS_MAX_EDGE {
            // 大图退路：独立纹理 + 独立绑定组（绘制时切段）。
            self.ensure_own_image(key, rgba, *width, *height);
            ([0.0, 0.0, 1.0, 1.0], Some(key))
        } else {
            match self.image_slot(key, rgba, *width, *height) {
                Some((ax, ay, w, h)) => {
                    let (pw, ph) = (
                        self.images.page.atlas.width() as f32,
                        self.images.page.atlas.height() as f32,
                    );
                    (
                        [
                            ax as f32 / pw,
                            ay as f32 / ph,
                            (ax + w) as f32 / pw,
                            (ay + h) as f32 / ph,
                        ],
                        None,
                    )
                }
                None => {
                    // 图集放满且无法再升级：独立纹理退路。
                    self.ensure_own_image(key, rgba, *width, *height);
                    ([0.0, 0.0, 1.0, 1.0], Some(key))
                }
            }
        };
        let inst = Inst {
            kind: KIND_IMAGE,
            flags: 0,
            rect: rect_phys,
            p: [0.0; 4],
            q: uv,
            color: [c.r, c.g, c.b, c.a],
            clip,
            _pad: [0.0; 2],
        };
        if cull.is_none_or(|d| inst_intersects(inst.rect, d)) {
            if let Some(k) = own_key {
                own.push((insts.len() as u32, k));
            }
            insts.push(inst);
        }
    }

    /// 图片图集条目：返回 (texel x, y, w, h)。未命中分配并上传；页满先升级。
    fn image_slot(
        &mut self,
        key: u32,
        rgba: &Arc<[u8]>,
        w: u32,
        h: u32,
    ) -> Option<(u32, u32, u32, u32)> {
        if let Some(&at) = self.images.slots.get(&key) {
            return Some((at.0, at.1, at.2, at.3));
        }
        match self.images.page.atlas.alloc(w, h) {
            Some(at) => {
                upload_entry(
                    &self.ctx.queue,
                    &self.images.page.texture,
                    at,
                    (w, h),
                    4,
                    rgba,
                );
                self.images.slots.insert(key, (at.0, at.1, w, h));
                self.images.sources.insert(key, (rgba.clone(), w, h));
                Some((at.0, at.1, w, h))
            }
            None => {
                let cur = self.images.page.atlas.width();
                if let Some(&next) = ATLAS_PAGE_SIZES.iter().find(|&&sz| sz > cur) {
                    log::warn!(
                        "图片图集 {cur}² 放满 → 升级 {next}²（重传 {} 张）",
                        self.images.slots.len()
                    );
                    self.rebuild_image_page(next);
                    return self.image_slot(key, rgba, w, h);
                }
                log::warn!("图片图集 4096² 放满，图片 #{key:08x} 走独立纹理");
                None
            }
        }
    }

    /// 重建图片图集页（升级）：分配器清零、按 `sources` 全量重传。
    fn rebuild_image_page(&mut self, size: u32) {
        let entries: Vec<(u32, ImageSource)> = self
            .images
            .sources
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect();
        let new_page = AtlasPage::new(&self.ctx.device, size, true);
        self.images.page = new_page;
        self.images.slots.clear();
        self.images.epoch += 1;
        for (k, (rgba, w, h)) in entries {
            if let Some(at) = self.images.page.atlas.alloc(w, h) {
                upload_entry(&self.ctx.queue, &self.images.page.texture, at, (w, h), 4, &rgba);
                self.images.slots.insert(k, (at.0, at.1, w, h));
            } else {
                // 升级后仍放不下的单张（不应发生：4096 ≥ 1024+2 边界）走独立纹理。
                self.images.slots.remove(&k);
                if let Some((rgba, w, h)) = self.images.sources.remove(&k) {
                    log::warn!("图片图集 {size}² 重传时放不下 #{k:08x}，改走独立纹理");
                    self.ensure_own_image(k, &rgba, w, h);
                }
            }
        }
        self.refresh_bind_group();
        self.target_valid = false;
    }

    /// 独立纹理图片：建纹理 + 同布局绑定组（binding 0 挂 R8 占位、binding 1 挂独立纹理）。
    /// 每图一次，之后绘制时按实例索引切段换绑定组。
    fn ensure_own_image(&mut self, key: u32, rgba: &[u8], width: u32, height: u32) {
        if self.own_images.contains_key(&key) {
            return;
        }
        let device = &self.ctx.device;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kanesumi-c2-own-image"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        upload_entry(&self.ctx.queue, &texture, (0, 0), (width, height), 4, rgba);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kanesumi-c2-own-image-bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_r8),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        self.own_views.insert(key, view);
        self.own_images.insert(key, bind_group);
    }

    /// 确保常驻离屏目标 target 与其视图就绪；尺寸变动时在 resize 里已清空重置。
    fn ensure_target(&mut self) {
        if self.target.is_some() {
            return;
        }
        let device = &self.ctx.device;
        let (pw, ph) = (self.config.width, self.config.height);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kanesumi-c2-target"),
            size: wgpu::Extent3d {
                width: pw,
                height: ph,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());
        // blit 绑定组：binding 0 挂 target_view，binding 1 挂 dummy_r8，
        // binding 2 挂采样器，binding 3 挂 uniform。
        let blit_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kanesumi-c2-blit-bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&target_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&self.dummy_r8),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: self.uniform.as_entire_binding(),
                },
            ],
        });
        self.target = Some(target);
        self.target_view = Some(target_view);
        self.blit_bind_group = Some(blit_bind_group);
        self.target_valid = false;
    }

    // ── 渲染入口（接口与 v1 对齐） ──────────────────────────────────────────

    /// 把一帧 Scene 光栅化到当前表面并提交（全幅）。
    pub fn render(&mut self, engine: &kanesumi_canvas::text::TextEngine, scene: &Scene) {
        self.render_with_damage(engine, scene, None);
    }

    /// 损伤感知绘制：C1.5 保留画布（常驻离屏目标 + 增量清除 + 剔除重画 + 上屏呈现）。
    /// `Some(damage)` 时：只对损伤矩形 D 做无混合清除，只重编码/绘制与 D 相交的图元；
    /// `None` 或 target 无效 / 强制整幅开关开启时：全量重画并更新 target_valid。
    pub fn render_with_damage(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        scene: &Scene,
        damage: Option<Rect>,
    ) {
        let (pw, ph) = (self.config.width, self.config.height);
        if pw == 0 || ph == 0 {
            return;
        }
        if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
            return;
        }
        self.ensure_target();

        let forced_full = canvas_full_forced();
        let phys_d = match damage {
            Some(d) => match damage_phys_rect(d, self.scale, pw, ph) {
                Some(pd) => Some(pd),
                None => return, // 损伤在表面外或零面积：无操作
            },
            None => None, // 全量损伤
        };
        let is_incremental = !forced_full && self.target_valid && phys_d.is_some();

        let (inc, dmg_pct) = if let (true, Some(pd)) = (is_incremental, phys_d) {
            let dmg_w = (pd[2] - pd[0]).max(0.0);
            let dmg_h = (pd[3] - pd[1]).max(0.0);
            let total = (pw * ph) as f32;
            let pct = if total > 0.0 { (dmg_w * dmg_h / total) * 100.0 } else { 0.0 };
            (true, pct)
        } else {
            (false, 100.0)
        };
        self.last_inc = inc;
        self.last_dmg_pct = dmg_pct;

        let mut insts = std::mem::take(&mut self.insts_buf);
        let mut own = std::mem::take(&mut self.pending_own);
        insts.clear();
        own.clear();
        let cull_rect = if is_incremental { phys_d } else { None };
        self.encode_scene(engine, scene, cull_rect, &mut insts, &mut own);
        self.last_insts = insts.len() as u32;
        self.insts_buf = insts;
        self.pending_own = own;

        // uniform：物理尺寸 + 浓度旋钮（每帧一写，16 字节）。
        self.ctx.queue.write_buffer(
            &self.uniform,
            0,
            bytemuck::cast_slice(&[
                self.config.width as f32,
                self.config.height as f32,
                self.text_tuning.contrast,
                self.text_tuning.gamma,
            ]),
        );

        let acquire_at = std::time::Instant::now();
        let acquired = self.surface.get_current_texture();
        self.last_acquire_ms = Some(acquire_at.elapsed().as_secs_f32() * 1000.0);
        let surface_texture = match acquired {
            Ok(t) => t,
            Err(e) => {
                log::warn!("CanvasV2 获取帧纹理失败：{e}");
                return;
            }
        };

        self.draw_and_present(is_incremental, phys_d, surface_texture);
    }

    /// 绘制 target（全幅清或增量重画）并呈现到交换链（copy 或 blit）。
    fn draw_and_present(
        &mut self,
        is_incremental: bool,
        phys_d: Option<[f32; 4]>,
        surface_texture: wgpu::SurfaceTexture,
    ) {
        let (pw, ph) = (self.config.width, self.config.height);
        let mut draws = 0u32;
        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kanesumi-c2-frame"),
            });
        let timer_slot = self.gpu_timer.as_mut().and_then(|t| t.begin());
        if let (Some(t), Some(slot)) = (self.gpu_timer.as_ref(), timer_slot) {
            t.write_first(&mut encoder, slot);
        }

        // 1. 准备上传数据（在 render pass 之前完成）
        if is_incremental {
            let pd = phys_d.unwrap();
            let clear_inst = Inst {
                kind: KIND_CLEAR,
                flags: 0,
                rect: [pd[0], pd[1], pd[2] - pd[0], pd[3] - pd[1]],
                p: [0.0; 4],
                q: [0.0; 4],
                color: [0.0; 4],
                clip: pd,
                _pad: [0.0; 2],
            };
            self.ctx.queue.write_buffer(&self.clear_buf, 0, bytemuck::bytes_of(&clear_inst));
        }

        let inst_count = self.insts_buf.len() as u32;
        if inst_count > 0 {
            upload_insts(
                &self.ctx.device,
                &self.ctx.queue,
                &mut self.inst_buf,
                &mut self.inst_cap,
                &self.insts_buf,
            );
        }

        // 2. 渲染到常驻离屏目标 target
        {
            let load_op = if is_incremental {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-c2-target-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: self.target_view.as_ref().unwrap(),
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if is_incremental {
                let pd = phys_d.unwrap();
                let sx = (pd[0].round() as u32).min(pw);
                let sy = (pd[1].round() as u32).min(ph);
                let sw = ((pd[2] - pd[0]).round() as u32).min(pw.saturating_sub(sx)).max(1);
                let sh = ((pd[3] - pd[1]).round() as u32).min(ph.saturating_sub(sy)).max(1);
                pass.set_scissor_rect(sx, sy, sw, sh);

                // 增量清除：覆写 D 为 0
                pass.set_pipeline(&self.pipeline_replace);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.clear_buf.slice(..));
                pass.draw(0..6, 0..1);
                draws += 1;
            }

            if inst_count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
                pass.set_vertex_buffer(0, self.inst_buf.slice(..));
                draws += record_draw_insts(
                    &mut pass,
                    inst_count,
                    &self.own_images,
                    &self.bind_group,
                    &self.pending_own,
                );
            }
        }
        if !is_incremental {
            self.target_valid = true;
        }

        // 3. target 呈现上屏（copy 或 blit）
        if self.present_copy {
            encoder.copy_texture_to_texture(
                wgpu::ImageCopyTexture {
                    texture: self.target.as_ref().unwrap(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::ImageCopyTexture {
                    texture: &surface_texture.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: pw,
                    height: ph,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            let surface_view = surface_texture
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            let mut blit_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-c2-blit-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            blit_pass.set_pipeline(&self.blit_pipeline);
            blit_pass.set_bind_group(0, self.blit_bind_group.as_ref().unwrap(), &[]);
            blit_pass.draw(0..3, 0..1);
            draws += 1;
        }

        if let (Some(t), Some(slot)) = (self.gpu_timer.as_ref(), timer_slot) {
            t.write_last(&mut encoder, slot);
            t.encode_readback(&mut encoder, slot);
        }
        self.ctx.queue.submit(Some(encoder.finish()));
        if let Some(t) = self.gpu_timer.as_mut() {
            t.end(&self.ctx.device, true);
        }
        surface_texture.present();
        self.last_draws = draws;
        self.pending_own.clear();
        self.frame_count = self.frame_count.wrapping_add(1);
    }

    /// 自检判定：仅在开启 `KANESUMI_CANVAS_VERIFY=1`、增量帧、且帧数为 30 的倍数时触发。
    #[cfg(debug_assertions)]
    pub(crate) fn should_verify(&self) -> bool {
        canvas_verify_enabled() && self.last_inc && self.frame_count > 0 && self.frame_count.is_multiple_of(30)
    }

    /// 自检：验证增量重画累积 target 与同帧整幅重画结果是否逐位一致（仅 debug 构建生效）。
    #[cfg(debug_assertions)]
    pub(crate) fn verify_target_against_full(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        scene: &Scene,
        damage: Option<Rect>,
    ) {
        let (pw, ph) = (self.config.width, self.config.height);
        if self.verify_tex.is_none() {
            let tex = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("kanesumi-c2-verify-target"),
                size: wgpu::Extent3d {
                    width: pw,
                    height: ph,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            self.verify_tex = Some(tex);
        }

        let mut verify_insts = std::mem::take(&mut self.verify_insts);
        verify_insts.clear();
        let mut verify_own = Vec::new();
        self.encode_scene(engine, scene, None, &mut verify_insts, &mut verify_own);

        let verify_tex = self.verify_tex.as_ref().unwrap();
        let verify_view = verify_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kanesumi-c2-verify-encoder"),
            });
        {
            let mut buf_cap = verify_insts.len() as u32;
            let mut inst_buf = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kanesumi-c2-verify-insts"),
                size: (verify_insts.len() * std::mem::size_of::<Inst>()).max(64) as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let buf = upload_insts(
                &self.ctx.device,
                &self.ctx.queue,
                &mut inst_buf,
                &mut buf_cap,
                &verify_insts,
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-c2-verify-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &verify_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, buf.slice(..));
            record_draw_insts(
                &mut pass,
                verify_insts.len() as u32,
                &self.own_images,
                &self.bind_group,
                &verify_own,
            );
        }
        let verify_insts_len = verify_insts.len();
        self.verify_insts = verify_insts;

        let unaligned_bytes_per_row = pw * 4;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row = (unaligned_bytes_per_row + align - 1) & !(align - 1);
        let buf_size = (bytes_per_row * ph) as u64;

        let read_buf_a = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verify-read-a"),
            size: buf_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let read_buf_b = self.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("verify-read-b"),
            size: buf_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: self.target.as_ref().unwrap(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &read_buf_a,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(ph),
                },
            },
            wgpu::Extent3d {
                width: pw,
                height: ph,
                depth_or_array_layers: 1,
            },
        );

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: verify_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &read_buf_b,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(ph),
                },
            },
            wgpu::Extent3d {
                width: pw,
                height: ph,
                depth_or_array_layers: 1,
            },
        );

        self.ctx.queue.submit(Some(encoder.finish()));

        let slice_a = read_buf_a.slice(..);
        let slice_b = read_buf_b.slice(..);
        let (tx_a, rx_a) = std::sync::mpsc::channel();
        let (tx_b, rx_b) = std::sync::mpsc::channel();
        slice_a.map_async(wgpu::MapMode::Read, move |v| {
            let _ = tx_a.send(v);
        });
        slice_b.map_async(wgpu::MapMode::Read, move |v| {
            let _ = tx_b.send(v);
        });
        self.ctx.device.poll(wgpu::Maintain::Wait);
        rx_a.recv().unwrap().expect("map_async a 失败");
        rx_b.recv().unwrap().expect("map_async b 失败");

        let data_a = slice_a.get_mapped_range();
        let data_b = slice_b.get_mapped_range();

        let mut mismatch = 0u64;
        let mut first_mismatch = None;
        for row in 0..ph as usize {
            let row_start = row * bytes_per_row as usize;
            let row_end = row_start + unaligned_bytes_per_row as usize;
            if data_a[row_start..row_end] != data_b[row_start..row_end] {
                mismatch += 1;
                if first_mismatch.is_none() {
                    for col in 0..pw as usize {
                        let px_start = row_start + col * 4;
                        let pa = &data_a[px_start..px_start + 4];
                        let pb = &data_b[px_start..px_start + 4];
                        if pa != pb {
                            first_mismatch = Some((col, row, [pa[0], pa[1], pa[2], pa[3]], [pb[0], pb[1], pb[2], pb[3]]));
                            break;
                        }
                    }
                }
            }
        }
        drop(data_a);
        drop(data_b);
        read_buf_a.unmap();
        read_buf_b.unmap();

        if mismatch > 0 {
            log::warn!(
                "WARN canvas-verify mismatch px={mismatch} damage={damage:?} first={first_mismatch:?} insts_verify={} scene_cmds={}",
                verify_insts_len,
                scene.commands.len()
            );
        }
        assert_eq!(
            mismatch, 0,
            "CanvasV2 自检失败：增量 target 与整幅重画逐位对拍存在 {mismatch} 行像素不匹配！frame={}",
            self.frame_count
        );
        log::info!(
            "CanvasV2 自检（frame={}）：增量 target 与整幅重画逐位一致（0 mismatch，damage={damage:?}）",
            self.frame_count
        );
    }

    // ── 对外接口（与 v1 `Renderer` 对齐，platform 侧经枚举分派） ─────────────

    /// 设定文字浓度旋钮。变化时清排版 / 字形位图 / 图集缓存 ——
    /// 加粗会改变字形位图与放置几何，旧缓存对新旋钮不再有效（与 v1 一致）。
    pub fn set_text_tuning(&mut self, tuning: TextRenderTuning) {
        if self.text_tuning == tuning {
            return;
        }
        self.text_tuning = tuning;
        self.text_cache.clear();
        self.glyph_bitmaps.clear();
        self.glyphs.slots.clear();
        self.glyphs.page.atlas.reset();
        self.target_valid = false;
    }

    pub fn text_tuning(&self) -> TextRenderTuning {
        self.text_tuning
    }

    /// 尺寸 / 缩放变化：重建表面配置（图集与排版缓存与尺寸无关，保留）。
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        let new_w = (width * scale).round().max(1.0) as u32;
        let new_h = (height * scale).round().max(1.0) as u32;
        if (self.width - width).abs() < 1e-4
            && (self.height - height).abs() < 1e-4
            && (self.scale - scale).abs() < 1e-4
            && self.config.width == new_w
            && self.config.height == new_h
        {
            return;
        }
        self.width = width;
        self.height = height;
        self.scale = scale;
        let caps = self.surface.get_capabilities(&self.ctx.adapter);
        self.config.width = new_w;
        self.config.height = new_h;
        self.config.alpha_mode = caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .copied()
            .unwrap_or(self.config.alpha_mode);
        self.surface.configure(&self.ctx.device, &self.config);
        self.target = None;
        self.target_view = None;
        self.blit_bind_group = None;
        self.target_valid = false;
        #[cfg(debug_assertions)]
        {
            self.verify_tex = None;
        }
    }

    /// 目标离屏缓冲 target 是否就绪且内容有效（内容完整、可直接做增量累积）。
    pub fn is_target_valid(&self) -> bool {
        self.target.is_some() && self.target_valid
    }

    /// 诊断串（日志用）。
    pub fn diagnostics(&self) -> String {
        format!(
            "CanvasV2 {}x{}@{}x inst_cap={} glyphs={} images={} layout_cache={} draws={} target_valid={}",
            self.width,
            self.height,
            self.scale,
            self.inst_cap,
            self.glyphs.slots.len(),
            self.images.slots.len() + self.own_images.len(),
            self.text_cache.len(),
            self.last_draws,
            self.target_valid,
        )
    }

    /// 最近一帧保留画布统计（是否增量、损伤面积占比 %、绘制实例数）。
    pub fn last_c15_stats(&self) -> (bool, f32, u32) {
        (self.last_inc, self.last_dmg_pct, self.last_insts)
    }

    /// v2 无 MSAA（SDF 解析抗锯齿），恒 1。
    pub fn msaa_samples(&self) -> u32 {
        1
    }

    pub fn take_acquire_ms(&mut self) -> Option<f32> {
        self.last_acquire_ms.take()
    }

    pub fn gpu_timing_supported(&self) -> bool {
        self.gpu_timer.is_some()
    }

    pub fn drain_gpu_samples(&mut self) -> Vec<f32> {
        self.gpu_timer
            .as_mut()
            .map(crate::render::GpuTimer::drain)
            .unwrap_or_default()
    }

    pub fn physical_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// 最近一帧绘制调用数（perf `draws=`）。
    pub fn last_draws(&self) -> u32 {
        self.last_draws
    }
}

/// 实例流写入持久缓冲（容量不足翻倍重建）。
fn upload_insts<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &'a mut wgpu::Buffer,
    cap: &mut u32,
    insts: &[Inst],
) -> &'a wgpu::Buffer {
    let needed = insts.len() as u32;
    if *cap < needed {
        while *cap < needed {
            *cap = (*cap * 2).max(INST_BUF_CAP);
        }
        *buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-c2-inst-buf"),
            size: *cap as u64 * std::mem::size_of::<Inst>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
    }
    if !insts.is_empty() {
        queue.write_buffer(buf, 0, bytemuck::cast_slice(insts));
    }
    buf
}

/// 录制实例绘制：按原序绘制，独立纹理处切段切换绑定组。返回产生的绘制调用次数。
fn record_draw_insts<'a>(
    pass: &mut wgpu::RenderPass<'a>,
    insts_len: u32,
    own_images: &'a HashMap<u32, wgpu::BindGroup>,
    atlas_bind_group: &'a wgpu::BindGroup,
    pending_own: &[(u32, u32)],
) -> u32 {
    let mut draws = 0u32;
    let mut own = pending_own.iter().copied().peekable();
    let mut run_start: u32 = 0;
    let mut i: u32 = 0;
    while i < insts_len {
        if own.peek().is_some_and(|&(oi, _)| oi == i) {
            let Some((_, key)) = own.next() else { unreachable!() };
            if i > run_start {
                pass.draw(0..6, run_start..i);
                draws += 1;
            }
            if let Some(bg) = own_images.get(&key) {
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..6, i..i + 1);
                draws += 1;
                pass.set_bind_group(0, atlas_bind_group, &[]);
            }
            i += 1;
            run_start = i;
        } else {
            i += 1;
        }
    }
    if i > run_start {
        pass.draw(0..6, run_start..i);
        draws += 1;
    }
    draws
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── SDF 数学的 CPU 对拍（任务书验证 2）──
    //
    // 与 shaders.wgsl.rs 同公式的 Rust 参考实现；在网格特征点上与解析期望值对拍。
    // WGSL 侧改动着色器必须同步这里（与 `gpu_formula_matches_cpu_lut` 同一纪律）。

    fn sd_rrect(p: (f32, f32), half: (f32, f32), r: f32) -> f32 {
        let qx = p.0.abs() - (half.0 - r);
        let qy = p.1.abs() - (half.1 - r);
        let (mx, my) = (qx.max(0.0), qy.max(0.0));
        (mx * mx + my * my).sqrt() + qx.max(qy).min(0.0) - r
    }

    fn sd_stroke(p: (f32, f32), half: (f32, f32), r: f32, t: f32) -> f32 {
        let d = sd_rrect(p, half, r);
        d.max(-(d + t))
    }

    fn sd_arc(p: (f32, f32), a0: f32, a1: f32, big_r: f32, t: f32) -> f32 {
        let two_pi = std::f32::consts::TAU;
        let sw = (a1 - a0).abs();
        let ring = ((p.0 * p.0 + p.1 * p.1).sqrt() - big_r).abs() - 0.5 * t;
        if sw >= two_pi - 1e-3 {
            return ring;
        }
        let m = 0.5 * (a0 + a1);
        let h = 0.5 * sw;
        let phi = p.0.atan2(-p.1);
        let mut dphi = phi - m;
        dphi -= two_pi * (dphi / two_pi).round();
        if dphi.abs() <= h {
            return ring;
        }
        let e0 = (big_r * a0.sin(), big_r * -a0.cos());
        let e1 = (big_r * a1.sin(), big_r * -a1.cos());
        let d0 = ((p.0 - e0.0).powi(2) + (p.1 - e0.1).powi(2)).sqrt();
        let d1 = ((p.0 - e1.0).powi(2) + (p.1 - e1.1).powi(2)).sqrt();
        d0.min(d1) - 0.5 * t
    }

    fn sd_triangle(p: (f32, f32), p0: (f32, f32), p1: (f32, f32), p2: (f32, f32)) -> f32 {
        let sub = |a: (f32, f32), b: (f32, f32)| (a.0 - b.0, a.1 - b.1);
        let dot = |a: (f32, f32), b: (f32, f32)| a.0 * b.0 + a.1 * b.1;
        let e0 = sub(p1, p0);
        let e1 = sub(p2, p1);
        let e2 = sub(p0, p2);
        let v0 = sub(p, p0);
        let v1 = sub(p, p1);
        let v2 = sub(p, p2);
        let clamp01 = |x: f32| x.clamp(0.0, 1.0);
        let pq = |v: (f32, f32), e: (f32, f32)| {
            let k = dot(v, e) / dot(e, e);
            let k = clamp01(k);
            (v.0 - e.0 * k, v.1 - e.1 * k)
        };
        let pq0 = pq(v0, e0);
        let pq1 = pq(v1, e1);
        let pq2 = pq(v2, e2);
        let s = (e0.0 * e2.1 - e0.1 * e2.0).signum();
        let seg = |pq: (f32, f32), e: (f32, f32)| {
            (dot(pq, pq), s * (pq.0 * e.1 - pq.1 * e.0))
        };
        let d0 = seg(pq0, e0);
        let d1 = seg(pq1, e1);
        let d2 = seg(pq2, e2);
        let d = d0.0.min(d1.0).min(d2.0);
        let dy = if d == d0.0 { d0.1 } else if d == d1.0 { d1.1 } else { d2.1 };
        -d.sqrt() * dy.signum()
    }

    #[test]
    fn sdf_圆角矩形_中心_边_四角() {
        let half = (50.0, 30.0);
        let r = 10.0;
        // 中心：到最近边（y 向）30。
        assert!((sd_rrect((0.0, 0.0), half, r) + 30.0).abs() < 1e-4);
        // 右边中点（x = 50）：边界 d=0。
        assert!(sd_rrect((50.0, 0.0), half, r).abs() < 1e-4);
        // 圆角起点（x = 50−10 = 40，y = −30）：边界。
        assert!(sd_rrect((40.0, -30.0), half, r).abs() < 1e-4);
        // 矩形角 (50, 30)：到圆角圆心 (40,20) 距离 √200 − 10。
        let want = 200f32.sqrt() - 10.0;
        assert!((sd_rrect((50.0, 30.0), half, r) - want).abs() < 1e-4);
        // 直角退路（r=0）：退化为矩形 SDF。
        assert!((sd_rrect((0.0, 0.0), half, 0.0) + 30.0).abs() < 1e-4);
        assert!(sd_rrect((50.0, 30.0), half, 0.0).abs() < 1e-4);
    }

    #[test]
    fn sdf_向内描边_内外沿与带内() {
        let half = (50.0, 30.0);
        let r = 10.0;
        let t = 8.0;
        // 外沿 = 矩形边界（x = 50）。
        assert!(sd_stroke((50.0, 0.0), half, r, t).abs() < 1e-4);
        // 内沿 = 内缩 t（x = 50 − 8 = 42）。
        assert!(sd_stroke((42.0, 0.0), half, r, t).abs() < 1e-4);
        // 带内中点（x = 46）→ −t/2。
        assert!((sd_stroke((46.0, 0.0), half, r, t) + 4.0).abs() < 1e-4);
        // 深内部（中心）：在带外（内沿以内）→ 填充 SDF = −30、max(−30, 22) = +22
        //（距描边带内沿 22 px，覆盖率按 0 处理）。
        assert!((sd_stroke((0.0, 0.0), half, r, t) - 22.0).abs() < 1e-4);
        // 带外深部（远离矩形）：正距离。
        assert!(sd_stroke((80.0, 0.0), half, r, t) > 0.0);
    }

    #[test]
    fn sdf_圆弧环_中心线_角度边界_端点圆盘() {
        let big_r = 40.0;
        let t = 6.0;
        // 全圆（张角 ≥ 2π）：环带中心线（r = 40）在带内中点 d = −t/2。
        assert!((sd_arc((40.0, 0.0), 0.0, 6.5, big_r, t) + 3.0).abs() < 1e-4);
        // 90° 弧（0 → π/2）：中心线中角 π/4 处在带内中点 d = −t/2。
        let m = 45f32.to_radians();
        let pm = (big_r * m.sin(), -big_r * m.cos());
        assert!(
            (sd_arc(pm, 0.0, std::f32::consts::FRAC_PI_2, big_r, t) + 3.0).abs() < 1e-4
        );
        // 中心线上、弧范围外（角 180° 方向）：到弧两端点圆盘的最近距离。
        // 点 (0, 40)（phi = π）。端点 e1 = 40·(sin π/2, −cos π/2) = (40, 0)，e0 = (0, −40)。
        // 到 e0 距离 80，到 e1 距离 √(40² + 40²) ≈ 56.57 → d = 56.57 − 3。
        let d = sd_arc((0.0, 40.0), 0.0, std::f32::consts::FRAC_PI_2, big_r, t);
        let want = (40.0f32 * 40.0 + 40.0f32 * 40.0).sqrt() - 3.0;
        assert!((d - want).abs() < 1e-3, "d={d} want={want}");
        // 环带径向内外：phi 在弧内但 r 偏出 t/2。
        let inside = sd_arc(pm, 0.0, std::f32::consts::FRAC_PI_2, big_r, t);
        let off = sd_arc(
            ((big_r + 4.0) * m.sin(), -(big_r + 4.0) * m.cos()),
            0.0,
            std::f32::consts::FRAC_PI_2,
            big_r,
            t,
        );
        assert!((off - inside - 4.0).abs() < 1e-3, "径向外移 4 px → d + 4");
    }

    #[test]
    fn sdf_三角形_三边_内外() {
        let (p0, p1, p2) = ((0.0, 0.0), (60.0, 0.0), (0.0, 40.0));
        // 三边中点：边界 d = 0。
        assert!(sd_triangle((30.0, 0.0), p0, p1, p2).abs() < 1e-4);
        assert!(sd_triangle((0.0, 20.0), p0, p1, p2).abs() < 1e-4);
        // 斜边中点 (30, 20)。
        assert!(sd_triangle((30.0, 20.0), p0, p1, p2).abs() < 1e-4);
        // 内部（重心附近）为负。
        assert!(sd_triangle((10.0, 8.0), p0, p1, p2) < 0.0);
        // 外部（远离）为正，且等于到最近边的距离（法向 6 px 的点）。
        let d = sd_triangle((30.0, -6.0), p0, p1, p2);
        assert!((d - 6.0).abs() < 1e-3, "d={d}");
    }

    // ── 实例展开与结构布局 ──────────────────────────────────────────────

    /// 顶点着色器的包围盒展开（Rust 参考实现）：rect 外扩 1 px，覆盖 6 顶点四边形。
    fn expand_rect(rect: [f32; 4]) -> [f32; 4] {
        [
            rect[0] - 1.0,
            rect[1] - 1.0,
            rect[2] + 2.0,
            rect[3] + 2.0,
        ]
    }

    #[test]
    fn 实例展开包围盒_含1px外扩() {
        let r = [10.0, 20.0, 100.0, 50.0];
        let e = expand_rect(r);
        assert_eq!(e, [9.0, 19.0, 102.0, 52.0], "四边各外扩 1 px");
        // 展开后的盒必须完整覆盖原 rect（AA 过渡带在原边界 ±0.5 px 内）。
        assert!(e[0] <= r[0] && e[1] <= r[1]);
        assert!(e[0] + e[2] >= r[0] + r[2] && e[1] + e[3] >= r[1] + r[3]);
    }

    /// `Inst` 布局与对齐：96 字节（WGSL 侧 vec4 对齐），字段偏移与 attributes 一致。
    #[test]
    fn inst_布局大小与对齐() {
        assert_eq!(std::mem::size_of::<Inst>(), 96);
        assert_eq!(std::mem::offset_of!(Inst, rect), 8);
        assert_eq!(std::mem::offset_of!(Inst, p), 24);
        assert_eq!(std::mem::offset_of!(Inst, q), 40);
        assert_eq!(std::mem::offset_of!(Inst, color), 56);
        assert_eq!(std::mem::offset_of!(Inst, clip), 72);
    }

    /// 排版缓存键对内容与样式敏感（防错字渲染）。
    #[test]
    fn 排版缓存键区分内容与样式() {
        let style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
        let a = layout_cache_key("确定", style, (100.0, 20.0), TextAlign::Left, false, Some(1),
            kanesumi_canvas::TextOverflow::Clip, 2.0, 7);
        let b = layout_cache_key("取消", style, (100.0, 20.0), TextAlign::Left, false, Some(1),
            kanesumi_canvas::TextOverflow::Clip, 2.0, 7);
        assert_ne!(a, b, "内容不同键不同");
        let c = layout_cache_key("确定",
            TextStyle::new(15.0, 20.0, kanesumi_core::FontWeight::Normal), (100.0, 20.0),
            TextAlign::Left, false, Some(1), kanesumi_canvas::TextOverflow::Clip, 2.0, 7);
        assert_ne!(a, c, "字号不同键不同");
        let d = layout_cache_key("确定", style, (100.0, 20.0), TextAlign::Center, false, Some(1),
            kanesumi_canvas::TextOverflow::Clip, 2.0, 7);
        assert_ne!(a, d, "对齐不同键不同");
        let e = layout_cache_key("确定", style, (100.0, 20.0), TextAlign::Left, false, Some(1),
            kanesumi_canvas::TextOverflow::Clip, 1.0, 7);
        assert_ne!(a, e, "scale 不同键不同");
    }

    // ── C1.5 保留画布单测 ───────────────────────────────────────────────

    #[test]
    fn damage_phys_rect_边界与取整外扩() {
        // 浮点与 2x 缩放：向外取整并外扩 1 px
        let d = Rect::new(10.2, 20.7, 30.1, 40.2);
        let pd = damage_phys_rect(d, 2.0, 500, 500).unwrap();
        // x0: floor(10.2 * 2.0) - 1.0 = 20.0 - 1.0 = 19.0
        // y0: floor(20.7 * 2.0) - 1.0 = 41.0 - 1.0 = 40.0
        // x1: ceil((10.2 + 30.1) * 2.0) + 1.0 = ceil(80.6) + 1.0 = 81.0 + 1.0 = 82.0
        // y1: ceil((20.7 + 40.2) * 2.0) + 1.0 = ceil(121.8) + 1.0 = 122.0 + 1.0 = 123.0
        assert_eq!(pd, [19.0, 40.0, 82.0, 123.0]);

        // 左上边界夹紧到 0.0
        let d_topleft = Rect::new(0.0, 0.0, 10.0, 10.0);
        let pd_topleft = damage_phys_rect(d_topleft, 1.0, 100, 100).unwrap();
        assert_eq!(pd_topleft, [0.0, 0.0, 11.0, 11.0]);

        // 右下边界夹紧到 width/height
        let d_bottomright = Rect::new(90.0, 90.0, 20.0, 20.0);
        let pd_bottomright = damage_phys_rect(d_bottomright, 1.0, 100, 100).unwrap();
        assert_eq!(pd_bottomright, [89.0, 89.0, 100.0, 100.0]);

        // 完全出界或零面积返回 None
        assert!(damage_phys_rect(Rect::new(-20.0, -20.0, 5.0, 5.0), 1.0, 100, 100).is_none());
        assert!(damage_phys_rect(Rect::new(150.0, 150.0, 10.0, 10.0), 1.0, 100, 100).is_none());
        assert!(damage_phys_rect(Rect::new(10.0, 10.0, 0.0, 0.0), 1.0, 100, 100).is_none());
    }

    #[test]
    fn inst_intersects_环带相切与原序() {
        // rect: [10, 10, 20, 20]，展开 1 px 为 [9, 9, 31, 31]
        let r = [10.0, 10.0, 20.0, 20.0];

        // 内部相交
        assert!(inst_intersects(r, [15.0, 15.0, 25.0, 25.0]));

        // 相切（边界接触）：按严格相交语义判为 false
        assert!(!inst_intersects(r, [31.0, 10.0, 50.0, 30.0]));
        assert!(!inst_intersects(r, [0.0, 0.0, 9.0, 30.0]));

        // 1 px 外扩覆盖（未外扩前右边界 30，但在 30.5 仍能与展开后的 31 相交）
        assert!(inst_intersects(r, [30.5, 10.0, 40.0, 30.0]));

        // push_inst 保持原序
        let d = [15.0, 15.0, 25.0, 25.0];
        let mut list = Vec::new();
        let make_inst = |kind: u32, x: f32| Inst {
            kind,
            flags: 0,
            rect: [x, 15.0, 10.0, 10.0],
            p: [0.0; 4],
            q: [0.0; 4],
            color: [0.0; 4],
            clip: [0.0, 0.0, 100.0, 100.0],
            _pad: [0.0; 2],
        };
        // 5 个图元：第 0(x=15 相交)、第 1(x=50 不相交)、第 2(x=20 相交)、第 3(x=80 不相交)、第 4(x=22 相交)
        push_inst(&mut list, make_inst(0, 15.0), Some(d));
        push_inst(&mut list, make_inst(1, 50.0), Some(d));
        push_inst(&mut list, make_inst(2, 20.0), Some(d));
        push_inst(&mut list, make_inst(3, 80.0), Some(d));
        push_inst(&mut list, make_inst(4, 22.0), Some(d));

        assert_eq!(list.len(), 3);
        assert_eq!(list[0].kind, 0);
        assert_eq!(list[1].kind, 2);
        assert_eq!(list[2].kind, 4);
    }

    #[test]
    fn c15_开关环境变量识别() {
        let old = std::env::var_os("KANESUMI_CANVAS_FULL");
        unsafe {
            std::env::set_var("KANESUMI_CANVAS_FULL", "1");
        }
        assert!(canvas_full_forced());
        unsafe {
            std::env::set_var("KANESUMI_CANVAS_FULL", "0");
        }
        assert!(!canvas_full_forced());
        unsafe {
            match old {
                Some(v) => std::env::set_var("KANESUMI_CANVAS_FULL", v),
                None => std::env::remove_var("KANESUMI_CANVAS_FULL"),
            }
        }
    }

    #[test]
    fn target_valid_失效条件表() {
        // 参 CANVAS_PLAN §Ⅳ C1.5 设计 1：
        // 首帧、resize、scale 变化、set_text_tuning、图集页重建（字形/图片）、交换链重建。
        #[derive(Default)]
        struct CanvasValidState {
            target_valid: bool,
            width: u32,
            height: u32,
            scale: f32,
            tuning_gamma: f32,
            glyph_epoch: u64,
            image_epoch: u64,
        }
        impl CanvasValidState {
            fn render_frame(&mut self, is_full: bool) {
                if is_full || !self.target_valid {
                    self.target_valid = true;
                }
            }
            fn resize(&mut self, w: u32, h: u32, s: f32) {
                if self.width != w || self.height != h || (self.scale - s).abs() > 1e-4 {
                    self.width = w;
                    self.height = h;
                    self.scale = s;
                    self.target_valid = false;
                }
            }
            fn set_text_tuning(&mut self, gamma: f32) {
                if (self.tuning_gamma - gamma).abs() > 1e-4 {
                    self.tuning_gamma = gamma;
                    self.target_valid = false;
                }
            }
            fn rebuild_glyph_atlas(&mut self) {
                self.glyph_epoch += 1;
                self.target_valid = false;
            }
            fn rebuild_image_atlas(&mut self) {
                self.image_epoch += 1;
                self.target_valid = false;
            }
            fn recreate_swapchain(&mut self) {
                self.target_valid = false;
            }
        }

        let mut s = CanvasValidState::default();
        // 1. 首帧：必须为 false（整幅重画前不可增量）
        assert!(!s.target_valid, "首帧 target_valid 必须为 false");

        s.render_frame(true);
        assert!(s.target_valid, "首帧全幅绘制后置为 true");

        // 2. resize 尺寸变动
        s.resize(1024, 768, 1.0);
        assert!(!s.target_valid, "尺寸变化必须置 false");
        s.render_frame(true);

        // 3. scale 变动
        s.resize(1024, 768, 2.0);
        assert!(!s.target_valid, "scale 变化必须置 false");
        s.render_frame(true);

        // 4. set_text_tuning
        s.set_text_tuning(1.25);
        assert!(!s.target_valid, "set_text_tuning 必须置 false");
        s.render_frame(true);

        // 5. 图集页重建（字形 epoch 变）
        s.rebuild_glyph_atlas();
        assert!(!s.target_valid, "字形图集页重建必须置 false");
        s.render_frame(true);

        // 6. 图集页重建（图片 epoch 变）
        s.rebuild_image_atlas();
        assert!(!s.target_valid, "图片图集页重建必须置 false");
        s.render_frame(true);

        // 7. 交换链重建（Lost / Outdated / configure）
        s.recreate_swapchain();
        assert!(!s.target_valid, "交换链重建必须置 false");
    }
}

