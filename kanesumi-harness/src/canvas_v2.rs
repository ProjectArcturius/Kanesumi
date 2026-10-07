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

/// 顶点缓冲初始容量（实例数）。不足时翻倍。
const INST_BUF_CAP: u32 = 4096;
/// 图集页尺寸阶梯：首页 2048²，放满升级 4096²，再满 → 字形跳过 / 图片走独立纹理。
const ATLAS_PAGE_SIZES: [u32; 2] = [2048, 4096];
/// 独立纹理退路：任一边超过该值不进图集（RGBA 图集单图 1024² = 4 MB）。
const IMAGE_ATLAS_MAX_EDGE: u32 = 1024;
/// 管线缓存落盘目录（`~/.cache/ether/wgpu/`）。
pub(crate) const PIPELINE_CACHE_DIR: &str = "ether/wgpu";

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
    scale: f32,
    width: f32,
    height: f32,
}

impl CanvasV2 {

/// 编码一帧 Scene → 实例流。顺序 = Scene 命令顺序（painter's algorithm 天然保持）。
fn encode_scene(&mut self, engine: &kanesumi_canvas::text::TextEngine, scene: &Scene) -> Vec<Inst> {
    let s = self.scale;
    let mut insts: Vec<Inst> = Vec::with_capacity(scene.commands.len());
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
                insts.push(Inst {
                    kind: KIND_FILL_RRECT,
                    flags: 0,
                    rect: [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                    p: [*corner_radius * s, 0.0, 0.0, 0.0],
                    q: [0.0; 4],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                });
            }
            SceneCommand::StrokeRect { color: c, rect, thickness, corner_radius } => {
                let Some(clip) = clips.current() else { continue };
                if rect.is_empty() {
                    continue;
                }
                insts.push(Inst {
                    kind: KIND_STROKE_RRECT,
                    flags: 0,
                    rect: [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                    p: [*corner_radius * s, *thickness * s, 0.0, 0.0],
                    q: [0.0; 4],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                });
            }
            SceneCommand::Arc { center, radius, thickness, color: c, start_deg, end_deg } => {
                let Some(clip) = clips.current() else { continue };
                let (cx, cy) = (center.x * s, center.y * s);
                let (r, t) = (*radius * s, *thickness * s);
                let a0 = start_deg.to_radians();
                let a1 = end_deg.to_radians();
                // 包围盒：心 ± (R + t/2)；退路（r ≤ 0 / t ≤ 0）交给着色器出 0 覆盖。
                let half = (r + t * 0.5).max(1.0);
                insts.push(Inst {
                    kind: KIND_ARC,
                    flags: 0,
                    rect: [cx - half, cy - half, half * 2.0, half * 2.0],
                    p: [cx, cy, r, t],
                    q: [a0, a1, 0.0, 0.0],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                });
            }
            SceneCommand::Triangle { p0, p1, p2, color: c } => {
                let Some(clip) = clips.current() else { continue };
                let (ax, bx) = (p0.x.min(p1.x).min(p2.x) * s, p0.x.max(p1.x).max(p2.x) * s);
                let (ay, by) = (p0.y.min(p1.y).min(p2.y) * s, p0.y.max(p1.y).max(p2.y) * s);
                insts.push(Inst {
                    kind: KIND_TRIANGLE,
                    flags: 0,
                    rect: [ax, ay, (bx - ax).max(0.001), (by - ay).max(0.001)],
                    p: [p0.x * s, p0.y * s, p1.x * s, p1.y * s],
                    q: [p2.x * s, p2.y * s, 0.0, 0.0],
                    color: color(c),
                    clip,
                    _pad: [0.0; 2],
                });
            }
            SceneCommand::Text { .. } => {
                let Some(clip) = clips.current() else { continue };
                self.encode_text(engine, cmd, clip, &mut insts);
            }
            SceneCommand::Image { .. } => {
                let Some(clip) = clips.current() else { continue };
                self.encode_image(cmd, clip, &mut insts);
            }
        }
    }
    insts
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
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
            ctx, surface, config, conn, width, height, scale,
        )
    }

    /// 管线、绑定布局、图集与持久缓冲（`with_context` 与单测共用的后半段）。
    fn build(
        ctx: Arc<GpuContext>,
        surface: wgpu::Surface<'static>,
        config: wgpu::SurfaceConfiguration,
        _conn: &Connection,
        width: f32,
        height: f32,
        scale: f32,
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
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Inst>() as u64,
                    step_mode: wgpu::VertexStepMode::Instance,
                    // offset 显式对齐 `Inst` 真实布局（repr(C)，[f32;4] 对齐 4）：
                    // kind+flags @0（8B）、rect@8、p@24、q@40、color@56、clip@72、_pad@88。
                    // 用 `offset_of!` 断言守住（inst_布局大小与对齐 测试）。
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x2, offset: std::mem::offset_of!(Inst, kind) as u64, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, rect) as u64, shader_location: 1 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, p) as u64, shader_location: 2 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, q) as u64, shader_location: 3 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, color) as u64, shader_location: 4 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: std::mem::offset_of!(Inst, clip) as u64, shader_location: 5 },
                    ],
                }],
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
        let bind_group = make_atlas_bind_group(
            device,
            &bgl,
            &sampler,
            &uniform,
            &glyphs.page.texture,
            &images.page.texture,
        );
        let gpu_timer = crate::render::GpuTimer::new(device, &ctx.queue);
        log::info!(
            "kanesumi CanvasV2：pipeline_cache={} gpu_timer={}",
            if cache.is_some() { "on" } else { "n/a" },
            if gpu_timer.is_some() { "on" } else { "n/a" },
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
            insts.push(Inst {
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
            });
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
    /// 超图集边 / 升级后仍放不下 → 独立纹理，实例索引记进 `pending_own`。
    fn encode_image(
        &mut self,
        cmd: &SceneCommand,
        clip: [f32; 4],
        insts: &mut Vec<Inst>,
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
        let (inst_rect, uv) = if let Some(&(ax, ay, w, h)) = self.images.slots.get(&key) {
            let (pw, ph) = (
                self.images.page.atlas.width() as f32,
                self.images.page.atlas.height() as f32,
            );
            (
                [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                [
                    ax as f32 / pw,
                    ay as f32 / ph,
                    (ax + w) as f32 / pw,
                    (ay + h) as f32 / ph,
                ],
            )
        } else if *width > IMAGE_ATLAS_MAX_EDGE || *height > IMAGE_ATLAS_MAX_EDGE {
            // 大图退路：独立纹理 + 独立绑定组（绘制时切段）。
            self.ensure_own_image(key, rgba, *width, *height);
            let idx = insts.len() as u32;
            self.pending_own.push((idx, key));
            ([rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
             [0.0, 0.0, 1.0, 1.0])
        } else {
            match self.image_slot(key, rgba, *width, *height) {
                Some((ax, ay, w, h)) => {
                    let (pw, ph) = (
                        self.images.page.atlas.width() as f32,
                        self.images.page.atlas.height() as f32,
                    );
                    (
                        [rect.origin.x * s, rect.origin.y * s, rect.size.width * s, rect.size.height * s],
                        [
                            ax as f32 / pw,
                            ay as f32 / ph,
                            (ax + w) as f32 / pw,
                            (ay + h) as f32 / ph,
                        ],
                    )
                }
                None => {
                    // 图集放满且无法再升级：独立纹理退路。
                    self.ensure_own_image(key, rgba, *width, *height);
                    let idx = insts.len() as u32;
                    self.pending_own.push((idx, key));
                    ([
                        rect.origin.x * s,
                        rect.origin.y * s,
                        rect.size.width * s,
                        rect.size.height * s,
                    ], [0.0, 0.0, 1.0, 1.0])
                }
            }
        };
        insts.push(Inst {
            kind: KIND_IMAGE,
            flags: 0,
            rect: inst_rect,
            p: [0.0; 4],
            q: uv,
            color: [c.r, c.g, c.b, c.a],
            clip,
            _pad: [0.0; 2],
        });
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

    // ── 渲染入口（接口与 v1 对齐） ──────────────────────────────────────────

    /// 把一帧 Scene 光栅化到当前表面并提交（全幅）。
    pub fn render(&mut self, engine: &kanesumi_canvas::text::TextEngine, scene: &Scene) {
        self.render_with_damage(engine, scene, None);
    }

    /// 损伤感知绘制：语义与 v1 相同 —— 但 v2 直画交换链（无 MSAA 中间纹理），
    /// 交换链内容跨帧未定义，`Some(damage)` 也全幅重画、由实例的 clip 丢弃界外像素。
    /// 零面积损伤 = 无变化，完全不提交（与 v1 一致）。
    pub fn render_with_damage(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        scene: &Scene,
        damage: Option<Rect>,
    ) {
        let (pw, ph) = (self.config.width as f32, self.config.height as f32);
        if pw < 1.0 || ph < 1.0 {
            return;
        }
        if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
            return;
        }
        let insts = self.encode_scene(engine, scene);
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
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_frame(&insts, &view);
        surface_texture.present();
    }

    /// 绘制：全幅 clear → 一次 `draw(0..6, 0..n)`（独立纹理图片处切段换绑定组）。
    fn draw_frame(&mut self, insts: &[Inst], view: &wgpu::TextureView) {
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
        {
            let buf = upload_insts(
                &self.ctx.device,
                &self.ctx.queue,
                &mut self.inst_buf,
                &mut self.inst_cap,
                insts,
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-c2-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
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
            // 实例按 Scene 原序一次画完；独立纹理图片处切段：换该图绑定组画单实例再切回。
            let mut own = self.pending_own.iter().copied().peekable();
            let mut run_start: u32 = 0;
            let mut i: u32 = 0;
            while i < insts.len() as u32 {
                if own.peek().is_some_and(|&(oi, _)| oi == i) {
                    let Some((_, key)) = own.next() else { unreachable!() };
                    if i > run_start {
                        pass.draw(0..6, run_start..i);
                        draws += 1;
                    }
                    if let Some(bg) = self.own_images.get(&key) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, i..i + 1);
                        draws += 1;
                        pass.set_bind_group(0, &self.bind_group, &[]);
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
        }
        if let (Some(t), Some(slot)) = (self.gpu_timer.as_ref(), timer_slot) {
            t.write_last(&mut encoder, slot);
            t.encode_readback(&mut encoder, slot);
        }
        self.ctx.queue.submit(Some(encoder.finish()));
        if let Some(t) = self.gpu_timer.as_mut() {
            t.end(&self.ctx.device, true);
        }
        self.last_draws = draws;
        self.pending_own.clear();
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
    }

    pub fn text_tuning(&self) -> TextRenderTuning {
        self.text_tuning
    }

    /// 尺寸 / 缩放变化：重建表面配置（图集与排版缓存与尺寸无关，保留）。
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        self.width = width;
        self.height = height;
        self.scale = scale;
        let caps = self.surface.get_capabilities(&self.ctx.adapter);
        self.config.width = (width * scale).round().max(1.0) as u32;
        self.config.height = (height * scale).round().max(1.0) as u32;
        self.config.alpha_mode = caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .copied()
            .unwrap_or(self.config.alpha_mode);
        self.surface.configure(&self.ctx.device, &self.config);
    }

    /// 诊断串（日志用）。
    pub fn diagnostics(&self) -> String {
        format!(
            "CanvasV2 {}x{}@{}x inst_cap={} glyphs={} images={} layout_cache={} draws={}",
            self.width,
            self.height,
            self.scale,
            self.inst_cap,
            self.glyphs.slots.len(),
            self.images.slots.len() + self.own_images.len(),
            self.text_cache.len(),
            self.last_draws,
        )
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
        let two_pi = 6.283185307179586;
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
}

