// render.rs — wgpu Scene 光栅化（painter's algorithm）。参 HANDOVER §1 Scene 命令光栅化。
//
// 纯色无渐变（SD §II）：所有形状 CPU 侧三角化后走单一 color pipeline；
// 文本用 fontdue 光栅化字形 → R8 覆盖纹理 → textured quad。
// 坐标约定：Scene 逻辑像素（原点左上，y 向下）；物理像素 = 逻辑 × scale。
// 注：本模块仅 Linux（wgpu 表面需 Wayland wl_surface）。

use std::collections::HashMap;

use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};
use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Point, Rect, TextStyle};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};
// 顶点持久化使用 write_buffer；无 create_buffer_init（DeviceExt 不再需要）。

// ── 顶点类型 ─────────────────────────────────────────────────────────────

/// 形状顶点：NDC 位置 + 直通 RGBA（fragment 内预乘）。
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SolidVertex {
    pos: [f32; 2],
    color: [f32; 4],
}

/// 文本顶点：NDC 位置 + UV + 直通 RGBA。
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

// ── WGSL ─────────────────────────────────────────────────────────────────

const SOLID_SHADER: &str = r#"
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / vec3<f32>(12.92);
    let high = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
};
@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.color = color;
    return out;
}
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(srgb_to_linear(in.color.rgb) * in.color.a, in.color.a);
}
"#;

const TEXT_SHADER: &str = r#"
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / vec3<f32>(12.92);
    let high = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
@group(0) @binding(0) var glyph_tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let cov = textureSample(glyph_tex, samp, in.uv).r;
    return vec4<f32>(srgb_to_linear(in.color.rgb) * in.color.a * cov, in.color.a * cov);
}
"#;

/// 图标管线：RGBA8 纹理采样。`in.color` 承载 tint：白色 (1,1,1) = 原色；
/// 其他 = 染色（以图标 alpha 为形状蒙版替换颜色）。输出预乘供混合。
const IMAGE_SHADER: &str = r#"
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let low = c / vec3<f32>(12.92);
    let high = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(high, low, c <= vec3<f32>(0.04045));
}
@group(0) @binding(0) var img_tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};
@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) uv: vec2<f32>, @location(2) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.pos = vec4<f32>(pos, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}
@fragment
fn fs(in: VsOut) -> @location(0) vec4<f32> {
    let src = textureSample(img_tex, samp, in.uv);
    let is_tint = in.color.r != 1.0 || in.color.g != 1.0 || in.color.b != 1.0;
    let rgb = select(src.rgb * src.a, srgb_to_linear(in.color.rgb) * src.a, is_tint);
    return vec4<f32>(rgb, src.a);
}
"#;

// ── 字形缓存 ─────────────────────────────────────────────────────────────

/// 单个字形：R8 覆盖纹理 + 绑定组。缓存身份含字体、glyph ID 与精确物理字号。
struct GlyphEntry {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// 一段文本字形绘制区间：glyph key + 顶点范围（6 顶点/quad）。
struct TextRun {
    glyph_key: GlyphKey,
    start: u32,
    count: u32,
}

struct ImageRun {
    image_key: u32,
    start: u32,
    count: u32,
}

/// 一帧 Scene 的顶点/绘制步数据（`build_frame` 产物，`render`/`render_to_shm` 共用）。
struct FrameData {
    solid: Vec<SolidVertex>,
    text: Vec<TextVertex>,
    text_runs: Vec<TextRun>,
    image: Vec<TextVertex>,
    image_runs: Vec<ImageRun>,
    steps: Vec<Step>,
    pending_glyphs: Vec<GlyphKey>,
    pending_images: Vec<(u32, Vec<u8>, u32, u32)>,
}

/// 按 Scene 命令原始顺序记录绘制步（保 painter's algorithm 跨类型）。
/// 同类型连续命令合成一个 Step，异类之间切 Step；draw 阶段按 Step 顺序切 pipeline。
/// Step 携带裁剪（Draw 阶段 scissor 用）——同一裁剪上下文内的同类命令才合并。
#[derive(Clone, Copy)]
enum Step {
    Solid {
        start: u32,
        count: u32,
        clip: Option<Rect>,
    },
    Text {
        run_start: u32,
        run_end: u32,
        clip: Option<Rect>,
    },
    Image {
        run_start: u32,
        run_end: u32,
        clip: Option<Rect>,
    },
}

// 追加或延长同类型末尾 Step。类型不同或裁剪不同 → 结算旧 Step、开新 Step。
fn push_solid(steps: &mut Vec<Step>, before: u32, after: u32, clip: Option<Rect>) {
    if after == before {
        return;
    }
    if let Some(Step::Solid { count, clip: c, .. }) = steps.last_mut()
        && *c == clip
    {
        *count += after - before;
    } else {
        steps.push(Step::Solid {
            start: before,
            count: after - before,
            clip,
        });
    }
}

fn push_text(steps: &mut Vec<Step>, before: u32, after: u32, clip: Option<Rect>) {
    if after == before {
        return;
    }
    if let Some(Step::Text {
        run_end, clip: c, ..
    }) = steps.last_mut()
        && *c == clip
    {
        *run_end = after;
    } else {
        steps.push(Step::Text {
            run_start: before,
            run_end: after,
            clip,
        });
    }
}

fn push_image(steps: &mut Vec<Step>, before: u32, after: u32, clip: Option<Rect>) {
    if after == before {
        return;
    }
    if let Some(Step::Image {
        run_end, clip: c, ..
    }) = steps.last_mut()
        && *c == clip
    {
        *run_end = after;
    } else {
        steps.push(Step::Image {
            run_start: before,
            run_end: after,
            clip,
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    engine_id: u64,
    font_id: u32,
    glyph_id: u16,
    size_bits: u32,
}

fn scissor_rect(
    clip: Option<Rect>,
    scale: f32,
    buffer_width: u32,
    buffer_height: u32,
) -> (u32, u32, u32, u32) {
    let max_x = buffer_width.max(1) as f32;
    let max_y = buffer_height.max(1) as f32;
    match clip {
        Some(clip) => {
            let x = (clip.origin.x * scale).floor().clamp(0.0, max_x - 1.0) as u32;
            let y = (clip.origin.y * scale).floor().clamp(0.0, max_y - 1.0) as u32;
            let right = (clip.right() * scale).ceil().clamp(x as f32 + 1.0, max_x) as u32;
            let bottom = (clip.bottom() * scale).ceil().clamp(y as f32 + 1.0, max_y) as u32;
            (x, y, right - x, bottom - y)
        }
        None => (0, 0, buffer_width.max(1), buffer_height.max(1)),
    }
}

// ── 光栅化器 ─────────────────────────────────────────────────────────────

/// MSAA 采样数。4× 是桌面 GPU 上「几何抗锯齿的甜蜜点」——
/// 圆角矩形/弧线的斜边 staircase 显著减少，成本一次 resolve pass 可接受。
/// 参 A1（本次会话新增）：40×20 胶囊在 2× 缩放下每角 6 段 tessellation 产生的边缘锯齿。
const MSAA_SAMPLES: u32 = 4;

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    solid_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    text_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
    /// 图标纹理缓存：key = (width,height) + rgba 内容 FNV 哈希。同一图标去重复用。
    images: HashMap<u32, GlyphEntry>,
    /// 字形位图 CPU 缓存（fontdue 光栅化结果）。key = `GlyphKey`。静态文本每帧复用，
    /// 避免重复光栅化（全链路最贵的 CPU 操作）。GPU 侧已有字形纹理缓存，此处补 CPU 侧。
    glyph_bitmaps: HashMap<GlyphKey, (fontdue::Metrics, Vec<u8>)>,
    /// 持久顶点缓冲（避免每帧 create_buffer 的 GPU 分配开销，§4.1 保留视觉树）。
    solid_buf: wgpu::Buffer,
    text_buf: wgpu::Buffer,
    image_buf: wgpu::Buffer,
    /// 缓冲容量（顶点数），不足时翻倍重建。
    solid_cap: u32,
    text_cap: u32,
    image_cap: u32,
    /// MSAA 中间纹理 view（sample_count=MSAA_SAMPLES）。render pass attachment 用它，
    /// swapchain view 作 resolve_target。resize 时重建。
    msaa_view: wgpu::TextureView,
    /// SHM 输出模式：渲染到离屏纹理并读回 RGBA → wl_shm 提交（R/B 交换在 commit 侧）。
    /// ⚠ Ether 合成器对 layer-shell 的 wgpu dmabuf 渲染不可见（Known Issue #8），
    ///   SHM buffer（collect_layer_draws → import_memory）可靠。参 ETHER_RENDER_LESSONS.md。
    shm_output: bool,
    /// 离屏解析纹理（SHM 模式）：MSAA resolve target + readback 源。resize 时重建。
    shm_tex: Option<wgpu::Texture>,
    /// 读回缓冲（双缓冲，避免每帧 `create_buffer` 的 GPU 分配）。容量不足时重建。
    readback_bufs: [Option<wgpu::Buffer>; 2],
    /// 本帧提交副本的目标缓冲索引（0/1，ping-pong）。
    readback_idx: usize,
    /// 上一帧 map_async 的接收端（本帧读回上一帧缓冲用；None = 首帧尚未预热）。
    readback_rx: Option<std::sync::mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    /// 首帧预热：首帧无上一帧数据，须阻塞读回一次以提交内容（否则无提交 → 无 frame
    /// callback → 渲染循环停摆）。
    readback_warmed: bool,
    /// 逻辑 → 物理缩放（整数，通常 1 或 2）。
    scale: f32,
    /// 逻辑尺寸。
    width: f32,
    height: f32,
}

/// 创建与当前 surface 同尺寸/格式的 MSAA 中间纹理 view（`MSAA_SAMPLES` 采样）。
fn create_msaa_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kanesumi-msaa"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: MSAA_SAMPLES,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

/// 创建离屏解析纹理（SHM 模式）：单采样、同表面格式、RENDER_ATTACHMENT | COPY_SRC。
/// 作为 MSAA resolve target 渲染，随后 copy_texture_to_buffer 读回 RGBA → wl_shm。
fn create_shm_tex(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
) -> Option<wgpu::Texture> {
    if config.width < 1 || config.height < 1 {
        return None;
    }
    Some(device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kanesumi-shm-offscreen"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    }))
}

/// 渲染器初始化错误。
#[derive(Debug)]
pub enum RendererError {
    Surface(wgpu::CreateSurfaceError),
    Adapter,
    Device(wgpu::RequestDeviceError),
}

impl Renderer {
    /// 从 wl_surface 建 wgpu 表面与管线。`conn` 用于取 wl_display 指针。
    ///
    /// 后端选择：优先 Vulkan；`request_adapter` 失败（含 `SURFACE_LOST_KHR`，常见于
    /// GLES 合成器下）时回退 GL（Mesa llvmpipe/lavapipe 软件路径），仍失败再试
    /// `force_fallback_adapter`。参 Known Issue #8 —— Ether DRM 合成器（GlesRenderer）
    /// 下 Vulkan 客户端 surface 可能失效，GL 软件回退保证 TopBar/Dock 可渲染。
    /// 新建渲染器。`transparent` = 表面需透明底（浮层）：alpha_mode 选 PreMultiplied，
    /// clear 为透明，画布上只画半透明面板/控件（参 Ether 合成器对 alpha 表面的混合）。
    /// `false` = 不透明表面（Kanesumi 主表面，背景实体）。
    /// `shm_output` = true 时启用离屏读回 → wl_shm（Ether 合成器 dmabuf 不可见，参
    ///   ETHER_RENDER_LESSONS.md）；false = 直接 present 到 wgpu surface（xdg-shell 可用）。
    pub fn new(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
        shm_output: bool,
    ) -> Result<Self, RendererError> {
        Self::new_with_backends(
            conn,
            wl_surface,
            width,
            height,
            scale,
            transparent,
            shm_output,
            // GL 优先：Ether 合成器（smithay GlesRenderer）自动 bind EGL Wayland display，
            // wgpu GL 后端经 EGL_WL_bind_wayland_display 可直接连；Vulkan 在 Ether DRM
            // 下 get_physical_device_surface_capabilities 报 SURFACE_LOST（客户端 Wayland
            // WSI 与合成器不兼容），且 fallback 会落到 lavapipe 软件 Vulkan 卡死。参 session.log。
            &[wgpu::Backends::GL, wgpu::Backends::VULKAN],
        )
    }

    /// 显式指定后端候选（主→备）。遍历首个能成功创建 adapter 的后端。
    pub fn new_with_backends(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
        shm_output: bool,
        backends: &[wgpu::Backends],
    ) -> Result<Self, RendererError> {
        for (i, backend) in backends.iter().enumerate() {
            match Self::new_with_backend(
                conn,
                wl_surface,
                width,
                height,
                scale,
                transparent,
                shm_output,
                *backend,
            ) {
                Ok(r) => return Ok(r),
                Err(e) if i + 1 < backends.len() => {
                    log::warn!("wgpu 后端 {:?} 初始化失败（{e:?}），尝试下一候选", backend);
                }
                Err(e) => return Err(e),
            }
        }
        unreachable!("backends 非空")
    }

    fn new_with_backend(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        _transparent: bool,
        shm_output: bool,
        backend: wgpu::Backends,
    ) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..Default::default()
        });

        // 提取 wl_display / wl_surface 原始指针（同 launcher render.rs 模式）。
        use raw_window_handle::{
            RawDisplayHandle, RawWindowHandle, WaylandDisplayHandle, WaylandWindowHandle,
        };
        use std::ptr::NonNull;

        let backend = conn.backend();
        let display_ptr = backend.display_ptr() as *mut std::ffi::c_void;
        let raw_display_handle = RawDisplayHandle::Wayland(WaylandDisplayHandle::new(
            NonNull::new(display_ptr).expect("wl_display 指针为空"),
        ));
        let surface_ptr = wl_surface.id().as_ptr() as *mut std::ffi::c_void;
        let raw_window_handle = RawWindowHandle::Wayland(WaylandWindowHandle::new(
            NonNull::new(surface_ptr).expect("wl_surface 指针为空"),
        ));
        let surface = unsafe {
            instance
                .create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
                    raw_display_handle,
                    raw_window_handle,
                })
                .map_err(RendererError::Surface)?
        };

        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            // 兼容 surface 失败（SURFACE_LOST）→ 试无 surface 约束的适配器。
            // ⚠ 不用 force_fallback_adapter=true：会选 lavapipe 软件 Vulkan，request_device
            //   慢到卡死（settings 在 Ether 里等几分钟无输出的元凶）。
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        })
        .ok_or(RendererError::Adapter)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(RendererError::Device)?;

        let caps = surface.get_capabilities(&adapter);
        // 用 sRGB 格式：与 eframe（librarian 可见）对齐。⚠ 合成器（GLES）import 非 sRGB
        // dmabuf（XRGB8888，无 alpha）时 alpha 通道读 0 → 整个 buffer 透明（背景消失、
        // 文字浮空）；sRGB（ARGB8888，有 alpha）→ 可见。参 session.log + Known Issue #8。
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        // alpha_mode：优先 PreMultiplied —— 保留 alpha 通道（背景不透明像素 a=1 完全
        // 覆盖，浮层透明区 a=0 透出桌面）。⚠ Opaque 时 wgpu 可能把 alpha 通道写 0
        // （或选无 alpha 格式），合成器 GLES 用 (ONE, ONE_MINUS_SRC_ALPHA) 预乘混合
        // 会把暗背景"加亮混入"桌面 → 看起来背景透明（Ether Known Issue #8 同源）。
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

        let (pw, ph) = (width * scale, height * scale);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            view_formats: vec![format],
            alpha_mode,
            width: pw.round().max(1.0) as u32,
            height: ph.round().max(1.0) as u32,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::Fifo,
        };
        surface.configure(&device, &config);

        // 形状管线（预乘混合）
        let solid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kanesumi-solid"),
            source: wgpu::ShaderSource::Wgsl(SOLID_SHADER.into()),
        });
        let solid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kanesumi-solid-layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let solid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-solid-pipeline"),
            layout: Some(&solid_layout),
            vertex: wgpu::VertexState {
                module: &solid_shader,
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SolidVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &solid_shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 文本管线（字形 R8 纹理 + 采样器）
        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kanesumi-text"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
        });
        let text_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kanesumi-text-bgl"),
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let text_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kanesumi-text-layout"),
            bind_group_layouts: &[&text_bgl],
            push_constant_ranges: &[],
        });
        let text_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-text-pipeline"),
            layout: Some(&text_layout),
            vertex: wgpu::VertexState {
                module: &text_shader,
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &text_shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("kanesumi-glyph-sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // 图标管线（RGBA8 纹理 + tint，与文本共享同一顶点布局 / 绑定布局）
        let image_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("kanesumi-image"),
            source: wgpu::ShaderSource::Wgsl(IMAGE_SHADER.into()),
        });
        let image_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("kanesumi-image-layout"),
            bind_group_layouts: &[&text_bgl],
            push_constant_ranges: &[],
        });
        let image_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("kanesumi-image-pipeline"),
            layout: Some(&image_layout),
            vertex: wgpu::VertexState {
                module: &image_shader,
                entry_point: "vs",
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &image_shader,
                entry_point: "fs",
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: MSAA_SAMPLES,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 持久顶点缓冲（初始容量，不足时翻倍）。§4.1 不变量 1：静态内容保留，避免每帧重建。
        let mk_vert_buf = |label: &str, cap: u64| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: cap * std::mem::size_of::<TextVertex>() as u64,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let (solid_cap, text_cap, image_cap) = (1024u32, 1024u32, 128u32);
        let solid_buf = mk_vert_buf("kanesumi-solid-buf", 1024);
        let text_buf = mk_vert_buf("kanesumi-text-buf", 1024);
        let image_buf = mk_vert_buf("kanesumi-image-buf", 128);

        let msaa_view = create_msaa_view(&device, &config);
        let shm_tex = if shm_output { create_shm_tex(&device, &config) } else { None };

        Ok(Self {
            surface,
            device,
            queue,
            config,
            solid_pipeline,
            text_pipeline,
            image_pipeline,
            text_bgl,
            sampler,
            glyphs: HashMap::new(),
            images: HashMap::new(),
            glyph_bitmaps: HashMap::new(),
            solid_buf,
            text_buf,
            image_buf,
            solid_cap,
            text_cap,
            image_cap,
            msaa_view,
            shm_output,
            shm_tex,
            readback_bufs: [None, None],
            readback_idx: 0,
            readback_rx: None,
            readback_warmed: false,
            scale,
            width,
            height,
        })
    }

    /// 重配尺寸（逻辑）与缩放。configure 事件触发。
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        self.width = width;
        self.height = height;
        self.scale = scale;
        let (pw, ph) = (width * scale, height * scale);
        self.config.width = pw.round().max(1.0) as u32;
        self.config.height = ph.round().max(1.0) as u32;
        self.surface.configure(&self.device, &self.config);
        self.msaa_view = create_msaa_view(&self.device, &self.config);
        self.shm_tex = if self.shm_output {
            create_shm_tex(&self.device, &self.config)
        } else {
            None
        };
    }

    /// 诊断：当前表面格式 / alpha_mode / buffer 物理尺寸（排查合成器下显示透明）。
    pub fn diagnostics(&self) -> String {
        format!(
            "format={:?} alpha_mode={:?} buffer={}x{} (逻辑 {:.0}x{:.0}, scale {:.0})",
            self.config.format,
            self.config.alpha_mode,
            self.config.width,
            self.config.height,
            self.width,
            self.height,
            self.scale,
        )
    }

    /// 物理像素尺寸（读回 / SHM 提交用，与 config 同步）。
    pub fn physical_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// 把一帧 Scene 光栅化到当前表面并提交（present 模式）。
    pub fn render(&mut self, engine: &TextEngine, scene: &Scene) {
        let (pw, ph) = (self.config.width as f32, self.config.height as f32);
        if pw < 1.0 || ph < 1.0 {
            return;
        }
        let frame = self.build_frame(engine, scene);
        let surface_texture = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(e) => {
                log::warn!("kanesumi-harness 获取帧纹理失败：{e}");
                return;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_frame(&frame, &view);
        surface_texture.present();
    }

    /// 把一帧 Scene 光栅化到离屏纹理并读回 RGBA（wl_shm Argb8888 内存序为 BGRA，转换在 commit 侧）。
    /// ⚠ Ether 合成器对 layer-shell 的 wgpu dmabuf 渲染不可见 → 走 SHM 提交。
    ///   返回 None = 尺寸未就绪 / 读回失败（调用方跳过本帧）。
    pub fn render_to_shm(&mut self, engine: &TextEngine, scene: &Scene) -> Option<Vec<u8>> {
        let (pw, ph) = (self.config.width as f32, self.config.height as f32);
        // ⚠ 高度 ≤1 = 浮层收起态（sync_floating_heights 对关闭面板取 max(1.0)）。此时跳过
        //   读回 + 提交（1px 不可见），避免每帧 device.poll(Wait) 全量 GPU 同步拖慢输入
        //   （TopBar 3 浮层 × 60fps 的 GPU 同步是顶栏延迟主因）。
        if pw < 1.0 || ph <= 1.0 {
            return None;
        }
        let tex = match self.shm_tex.take() {
            Some(t) => t,
            None => return None,
        };
        let frame = self.build_frame(engine, scene);
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_frame(&frame, &view);
        let out = self.readback(&tex);
        // 归还离屏纹理（draw_frame / readback 不触碰 shm_tex，take/restore 安全）。
        self.shm_tex = Some(tex);
        out
    }

    /// 构建一帧顶点与绘制步（Scene → 顶点）。render / render_to_shm 共用。
    fn build_frame(&mut self, engine: &TextEngine, scene: &Scene) -> FrameData {
        let (px, py) = (self.width, self.height);

        // 逻辑 → NDC（y 翻转）。
        let ndc = |x: f32, y: f32| -> [f32; 2] { [(x / px) * 2.0 - 1.0, 1.0 - (y / py) * 2.0] };

        let mut solid: Vec<SolidVertex> = Vec::new();
        let mut text: Vec<TextVertex> = Vec::new();
        let mut text_runs: Vec<TextRun> = Vec::new();
        let mut pending_glyphs: Vec<GlyphKey> = Vec::new();
        let mut image: Vec<TextVertex> = Vec::new();
        let mut image_runs: Vec<ImageRun> = Vec::new();
        let mut pending_images: Vec<(u32, Vec<u8>, u32, u32)> = Vec::new();
        // 裁剪栈（box 语义，嵌套容器用）：`PushClip` / `PopClip` 严格配对。
        // 有效裁剪 = 栈内全部矩形的交集（子容器不放大父裁剪，只收窄）。
        let mut clip_stack: Vec<Option<Rect>> = Vec::new();

        // V18：按 Scene 命令原始顺序记录绘制步（保 painter's algorithm 跨类型）。
        // 旧代码把所有 FillRect 汇入一次 solid draw、所有 Text 汇入一次 text draw、
        // 所有 Image 汇入一次 image draw；顺序仅在同类内保持 → 底层控件文本会画在
        // 后来的对话框/下拉菜单面板之上（用户视觉：面板"透视"、文字浮顶）。
        //
        // 新方案：同类型连续命令合成一个 Step，异类之间切 Step；draw 阶段按 Step
        // 顺序切 pipeline + 画对应区间。多几次 set_pipeline，换来正确 z-order。
        // 2026-08-12：Step 携带裁剪（`clip` 为 Draw 阶段 scissor 用）—— 同一裁剪
        // 上下文内的同类命令才合并；裁剪切换（含嵌套 push/pop）自动切 Step，
        // 保证 Text/Triangle 也被 scissor 裁进容器（修复文字溢出，参 layout.rs）。
        // Step 枚举与 push_* 助手已上提模块级（render / render_to_shm 共用）。
        let mut steps: Vec<Step> = Vec::new();

        let surface_bounds = Rect::new(0.0, 0.0, self.width, self.height);
        let effective_clip = |stack: &[Option<Rect>]| {
            stack
                .last()
                .copied()
                .flatten()
                .and_then(|clip| intersect(clip, surface_bounds))
        };
        let is_fully_clipped = |stack: &[Option<Rect>]| {
            matches!(stack.last(), Some(None))
                || stack
                    .last()
                    .copied()
                    .flatten()
                    .is_some_and(|clip| intersect(clip, surface_bounds).is_none())
        };

        for cmd in &scene.commands {
            match cmd {
                SceneCommand::PushClip { rect } => {
                    let effective = match clip_stack.last().copied() {
                        Some(Some(parent)) => intersect(parent, *rect),
                        Some(None) => None,
                        None => Some(*rect),
                    };
                    clip_stack.push(effective);
                }
                SceneCommand::PopClip => {
                    if clip_stack.pop().is_none() {
                        log::warn!("Kanesumi Scene 收到未配对的 PopClip");
                    }
                }
                SceneCommand::FillRect {
                    color,
                    rect,
                    corner_radius,
                } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    let before = solid.len() as u32;
                    let clip = effective_clip(&clip_stack);
                    emit_fill(&mut solid, &ndc, *rect, *corner_radius, *color);
                    push_solid(&mut steps, before, solid.len() as u32, clip);
                }
                SceneCommand::StrokeRect {
                    color,
                    rect,
                    thickness,
                    corner_radius,
                } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    let before = solid.len() as u32;
                    let clip = effective_clip(&clip_stack);
                    emit_stroke(&mut solid, &ndc, *rect, *corner_radius, *thickness, *color);
                    push_solid(&mut steps, before, solid.len() as u32, clip);
                }
                SceneCommand::Arc {
                    center,
                    radius,
                    thickness,
                    color,
                    start_deg,
                    end_deg,
                } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    let before = solid.len() as u32;
                    emit_arc(
                        &mut solid, &ndc, *center, *radius, *thickness, *color, *start_deg,
                        *end_deg,
                    );
                    push_solid(
                        &mut steps,
                        before,
                        solid.len() as u32,
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
                    if is_fully_clipped(&clip_stack) || rect.is_empty() {
                        continue;
                    }
                    let text_clip = match effective_clip(&clip_stack) {
                        Some(parent) => intersect(parent, *rect),
                        None => intersect(surface_bounds, *rect),
                    };
                    let Some(text_clip) = text_clip else { continue };
                    let before = text_runs.len() as u32;
                    self.emit_text(
                        engine,
                        &ndc,
                        &mut text,
                        &mut text_runs,
                        &mut pending_glyphs,
                        content,
                        *rect,
                        *color,
                        *style,
                        *align,
                        *wrap,
                        *max_lines,
                        *overflow,
                    );
                    push_text(&mut steps, before, text_runs.len() as u32, Some(text_clip));
                }
                SceneCommand::Image {
                    rgba,
                    width,
                    height,
                    rect,
                    tint,
                } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    let before = image_runs.len() as u32;
                    let clip = effective_clip(&clip_stack);
                    emit_image(
                        &ndc,
                        &mut image,
                        &mut image_runs,
                        &mut pending_images,
                        rgba,
                        *width,
                        *height,
                        *rect,
                        *tint,
                    );
                    push_image(&mut steps, before, image_runs.len() as u32, clip);
                }
                SceneCommand::Triangle { p0, p1, p2, color } => {
                    if is_fully_clipped(&clip_stack) {
                        continue;
                    }
                    // 自绘几何 glyph（Metro chevron/箭头/收合指示等）——三顶点直接入 solid。
                    // 裁剪经 Draw 阶段 scissor（Step.clip）统一处理。
                    let before = solid.len() as u32;
                    let c = [color.r, color.g, color.b, color.a];
                    solid.push(SolidVertex {
                        pos: ndc(p0.x, p0.y),
                        color: c,
                    });
                    solid.push(SolidVertex {
                        pos: ndc(p1.x, p1.y),
                        color: c,
                    });
                    solid.push(SolidVertex {
                        pos: ndc(p2.x, p2.y),
                        color: c,
                    });
                    push_solid(
                        &mut steps,
                        before,
                        solid.len() as u32,
                        effective_clip(&clip_stack),
                    );
                }
            }
        }

        FrameData {
            solid,
            text,
            text_runs,
            image,
            image_runs,
            steps,
            pending_glyphs,
            pending_images,
        }
    }

    /// 绘制已构建帧：MSAA pass → resolve 到 `resolve` 视图 → submit。
    /// `resolve` = swapchain 视图（present 模式）或离屏纹理视图（SHM 模式）。
    fn draw_frame(&mut self, frame: &FrameData, resolve: &wgpu::TextureView) {
        // 先建字形纹理（借用分离）
        for key in &frame.pending_glyphs {
            self.ensure_glyph(*key);
        }
        // 再建图标纹理（借用分离）
        for (key, rgba, w, h) in &frame.pending_images {
            self.ensure_image(*key, rgba, *w, *h);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kanesumi-frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-pass"),
                // MSAA：多重采样纹理作 attachment，resolve 视图作 resolve_target。
                // pass 结束时硬件自动 4→1 downsample 到 resolve（swapchain 或离屏）。
                // store=Discard 因为 MSAA 中间纹理不再使用（resolve 已完成）。
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.msaa_view,
                    resolve_target: Some(resolve),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // V18：按 Scene 命令原始顺序走 Step，逐步 set_pipeline + 画对应区间。
            // 三个持久顶点缓冲一次性上传全量顶点（避免每步反复 upload）；draw 区间
            // 由 Step 携带。painter's algorithm 跨类型正确 —— 对话框/下拉菜单面板
            // 之后的命令不再被之前控件的文本盖住。
            let solid_buf = if !frame.solid.is_empty() {
                Some(upload_vertices_solid(
                    &self.device,
                    &self.queue,
                    &mut self.solid_buf,
                    &mut self.solid_cap,
                    &frame.solid,
                ))
            } else {
                None
            };
            let text_buf = if !frame.text.is_empty() {
                Some(upload_vertices(
                    &self.device,
                    &self.queue,
                    &mut self.text_buf,
                    &mut self.text_cap,
                    &frame.text,
                ))
            } else {
                None
            };
            let image_buf = if !frame.image.is_empty() {
                Some(upload_vertices(
                    &self.device,
                    &self.queue,
                    &mut self.image_buf,
                    &mut self.image_cap,
                    &frame.image,
                ))
            } else {
                None
            };

            // 裁剪矩形（逻辑）→ wgpu scissor（物理像素）。clip 为空 = 全表面。
            // 四步 clip 全覆盖：Solid/Text/Image 统一裁剪 —— 文字/几何 glyph 也被
            // 裁进容器（修复文字溢出、字体出框，参 layout.rs box 语义）。
            let set_scissor = |pass: &mut wgpu::RenderPass, clip: Option<Rect>| {
                let (x, y, width, height) =
                    scissor_rect(clip, self.scale, self.config.width, self.config.height);
                pass.set_scissor_rect(x, y, width, height);
            };

            for step in &frame.steps {
                match step {
                    Step::Solid { start, count, clip } => {
                        let Some(buf) = solid_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.solid_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, *clip);
                        pass.draw(*start..*start + *count, 0..1);
                    }
                    Step::Text {
                        run_start,
                        run_end,
                        clip,
                    } => {
                        let Some(buf) = text_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, *clip);
                        for run in &frame.text_runs[*run_start as usize..*run_end as usize] {
                            let Some(glyph) = self.glyphs.get(&run.glyph_key) else {
                                continue;
                            };
                            pass.set_bind_group(0, &glyph.bind_group, &[]);
                            pass.draw(run.start..run.start + run.count, 0..1);
                        }
                    }
                    Step::Image {
                        run_start,
                        run_end,
                        clip,
                    } => {
                        let Some(buf) = image_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, *clip);
                        for run in &frame.image_runs[*run_start as usize..*run_end as usize] {
                            let Some(tex) = self.images.get(&run.image_key) else {
                                continue;
                            };
                            pass.set_bind_group(0, &tex.bind_group, &[]);
                            pass.draw(run.start..run.start + run.count, 0..1);
                        }
                    }
                }
            }
            // wgpu-hal GLES 在 MSAA resolve 的 glBlitFramebuffer 前不会重置动态 scissor。
            // pass 结束前恢复全表面，否则 resolve 只复制最后一个文本框的区域。
            pass.set_scissor_rect(0, 0, self.config.width, self.config.height);
        }

        self.queue.submit(Some(encoder.finish()));
    }

    /// 读回离屏纹理 → RGBA bytes（bytes_per_row 256 对齐，去 padding）。
    /// wl_shm Argb8888 = 内存 [B,G,R,A]（little-endian），与 Bgra8UnormSrgb readback 一致。
    ///
    /// **双缓冲**：本帧把副本提交到 `bufs[idx]`，读回 `bufs[1-idx]`（上一帧副本，已就绪），
    /// 避免每帧 `device.poll(Maintain::Wait)` 全量 GPU 同步阻塞输入分发（TopBar 延迟主因）。
    /// 首帧（无上一帧）预热：阻塞读回一次，保证有提交 → frame callback → 渲染循环启动。
    fn readback(&mut self, tex: &wgpu::Texture) -> Option<Vec<u8>> {
        let w = tex.width();
        let h = tex.height();
        if w == 0 || h == 0 {
            return None;
        }
        let bpr = (w * 4).div_ceil(256) * 256;
        let needed = bpr as u64 * h as u64;
        let idx = self.readback_idx;
        if self.readback_bufs[idx]
            .as_ref()
            .is_none_or(|b| b.size() < needed)
        {
            self.readback_bufs[idx] = Some(self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("kanesumi-shm-readback"),
                size: needed,
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            }));
        }
        let rb = self.readback_bufs[idx].as_ref().expect("读回缓冲已建");
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("kanesumi-shm-readback-enc"),
        });
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: rb,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(h),
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit(Some(encoder.finish()));

        if !self.readback_warmed {
            // 首帧预热：阻塞 map + 读，但**保持映射**（供下一帧读回后 unmap）。
            // ⚠ 若此处 unmap，下一帧读回读不到上一帧数据 → 无提交 → 渲染循环停摆。
            self.readback_warmed = true;
            let slice = rb.slice(..);
            let (tx, rx) = std::sync::mpsc::channel();
            slice.map_async(wgpu::MapMode::Read, move |r| {
                let _ = tx.send(r);
            });
            self.device.poll(wgpu::Maintain::Wait);
            let out = match rx.recv() {
                Ok(Ok(())) => Some(Self::copy_range(&slice, w, h, bpr)),
                _ => None,
            };
            self.readback_idx = 1 - idx;
            return out;
        }

        // 常规帧：map_async 本帧缓冲（供下一帧读回），读回上一帧缓冲（已就绪）。
        let new_slice = rb.slice(..);
        let (new_tx, new_rx) = std::sync::mpsc::channel();
        new_slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = new_tx.send(r);
        });

        let prev = 1 - idx;
        let out = match self.readback_rx.take() {
            Some(rx) => {
                self.device.poll(wgpu::Maintain::Poll);
                match rx.try_recv() {
                    Ok(Ok(())) => {
                        let pb = self.readback_bufs[prev].as_ref().expect("读回缓冲已建");
                        let pslice = pb.slice(..);
                        Some(Self::de_read(&pslice, pb, w, h, bpr))
                    }
                    // 罕见：上一帧 map 尚未就绪（GPU 过忙）→ 阻塞兜底，保证正确性。
                    _ => {
                        self.device.poll(wgpu::Maintain::Wait);
                        match rx.recv() {
                            Ok(Ok(())) => {
                                let pb = self.readback_bufs[prev].as_ref().expect("读回缓冲已建");
                                let pslice = pb.slice(..);
                                Some(Self::de_read(&pslice, pb, w, h, bpr))
                            }
                            _ => None,
                        }
                    }
                }
            }
            // 第二帧：上一帧缓冲已被预热保持映射（无 rx）→ 直接读 + unmap。
            None => {
                let pb = self.readback_bufs[prev].as_ref().expect("读回缓冲已建");
                let pslice = pb.slice(..);
                Some(Self::de_read(&pslice, pb, w, h, bpr))
            }
        };
        self.readback_idx = prev;
        self.readback_rx = Some(new_rx);
        out
    }

    /// 从已映射的读回缓冲复制数据（去除 bpr padding），**不解除映射**。
    /// 首帧预热用：保持映射供下一帧读回后 unmap。
    fn copy_range(slice: &wgpu::BufferSlice, w: u32, h: u32, bpr: u32) -> Vec<u8> {
        let row_bytes = (w as usize) * 4;
        let mut o = Vec::with_capacity(row_bytes * h as usize);
        let data = slice.get_mapped_range();
        for row in 0..h as usize {
            let start = row * bpr as usize;
            o.extend_from_slice(&data[start..start + row_bytes]);
        }
        o
    }

    /// 从已映射的读回缓冲复制数据并解除映射（去除 bpr padding）。
    /// ⚠ 持久读回缓冲必须 unmap，否则下一帧 queue.submit 报 "is still mapped" → panic →
    /// 全部 SHM 客户端第二帧崩溃循环（Debian 实测，参 5e4f788 引入）。
    fn de_read(
        slice: &wgpu::BufferSlice,
        buf: &wgpu::Buffer,
        w: u32,
        h: u32,
        bpr: u32,
    ) -> Vec<u8> {
        let o = Self::copy_range(slice, w, h, bpr);
        buf.unmap();
        o
    }

    /// 排版一段文本并产出字形 quad。
    #[allow(clippy::too_many_arguments)]
    fn emit_text(
        &mut self,
        engine: &TextEngine,
        ndc: &dyn Fn(f32, f32) -> [f32; 2],
        verts: &mut Vec<TextVertex>,
        runs: &mut Vec<TextRun>,
        pending: &mut Vec<GlyphKey>,
        content: &str,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
        wrap: bool,
        max_lines: Option<usize>,
        overflow: kanesumi_canvas::TextOverflow,
    ) {
        // 光栅化用物理字号（保字形清晰），quad 坐标用逻辑（与 fill_rect 同坐标系）。
        // 修复：scale>1 时若混用物理坐标进逻辑 NDC，文字会放大错位。
        let size_phys = style.size * self.scale;
        // V17：布局须知字距，否则负字距（TabRow −0.025em）会把可容纳文本判为"装不下"
        // → 单词硬断 / CJK 一字一行 → 第二行溢出 rect 底部（"字体出框"）。
        let mut options =
            TextLayoutOptions::wrapped(rect.size.width, rect.size.height, style.line_height);
        options.letter_spacing_em = style.letter_spacing_em;
        options.max_lines = max_lines;
        options.wrap = wrap;
        options.overflow = overflow;
        let layout = engine.layout_box(content, style.size, options);
        let line_advance = style.line_height;
        let ascent_log = engine.ascent(size_phys) / self.scale;

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
                // 字形位图缓存：静态文本每帧重复光栅化是最大 CPU 浪费。命中仅复用度量
                // （零分配），miss 才 rasterize 并入库。bitmap 只在 GPU 纹理首次创建时
                // 需要，由 `ensure_glyph` 直接从缓存取，不再每帧 clone 位图。
                let metrics = if let Some((m, _)) = self.glyph_bitmaps.get(&key) {
                    *m
                } else {
                    let (m, b) =
                        engine.rasterize_glyph(glyph.font_id, glyph.glyph_id, size_phys);
                    if m.width > 0 && m.height > 0 {
                        self.glyph_bitmaps.insert(key, (m, b));
                    }
                    m
                };
                if metrics.width == 0 || metrics.height == 0 {
                    pen += glyph.x_advance;
                    continue;
                }
                // 物理 metrics → 逻辑坐标（÷ scale）。
                // fontdue: ymin = 字形底相对基线的偏移，fontdue Y+ 向上（PostScript 惯例）。
                //   descender 字母（y/p/g）ymin < 0（底在基线下方）；只有 ascender 的字母 ymin = 0。
                // 屏幕 Y+ 向下：字形顶 y0 = baseline − ymin − height。
                let inv = 1.0 / self.scale;
                let x0 = pen + glyph.x_offset + metrics.xmin as f32 * inv;
                let y0 = baseline
                    - glyph.y_offset
                    - metrics.ymin as f32 * inv
                    - metrics.height as f32 * inv;
                let (w, h) = (metrics.width as f32 * inv, metrics.height as f32 * inv);
                let (x1, y1) = (x0 + w, y0 + h);
                let start = verts.len() as u32;
                push_quad(
                    verts,
                    ndc,
                    [x0, y0],
                    [x1, y1],
                    [0.0, 0.0],
                    [1.0, 1.0],
                    color,
                );
                runs.push(TextRun {
                    glyph_key: key,
                    start,
                    count: 6,
                });
                pending.push(key);
                pen += glyph.x_advance;
            }
            line_y += line_advance;
        }
    }

    /// 确保字形纹理存在。bitmap/metrics 从 `glyph_bitmaps` CPU 缓存取（`emit_text`
    /// 已在 miss 时入库），不再通过帧数据传递——避免每帧 clone 位图。
    fn ensure_glyph(&mut self, key: GlyphKey) {
        if self.glyphs.contains_key(&key) {
            return;
        }
        let Some((metrics, bitmap)) = self.glyph_bitmaps.get(&key) else {
            return;
        };
        let (w, h) = (metrics.width as u32, metrics.height as u32);
        if w == 0 || h == 0 {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kanesumi-glyph"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            bitmap,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kanesumi-glyph-bg"),
            layout: &self.text_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.glyphs.insert(
            key,
            GlyphEntry {
                texture,
                bind_group,
            },
        );
    }

    /// 上传图标纹理（直通 RGBA → RGBA8UnormSrgb）。key = rgba 内容 FNV 哈希。
    fn ensure_image(&mut self, key: u32, rgba: &[u8], width: u32, height: u32) {
        if self.images.contains_key(&key) {
            return;
        }
        if width == 0 || height == 0 {
            return;
        }
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("kanesumi-image"),
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
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kanesumi-image-bg"),
            layout: &self.text_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        self.images.insert(
            key,
            GlyphEntry {
                texture,
                bind_group,
            },
        );
    }
}

fn glyph_key(engine_id: u64, font_id: u32, glyph_id: u16, size_px: f32) -> GlyphKey {
    GlyphKey {
        engine_id,
        font_id,
        glyph_id,
        size_bits: size_px.to_bits(),
    }
}

// ── 形状三角化（逻辑坐标 → 已转 NDC 的顶点）────────────────────────────

/// 圆角矩形边界点（逆时针，含所有角弧）。`segs` 每角弧段数。
///
/// **总是**输出 `segs × 4` 个点（即使 r=0），以便 emit_stroke 里的外 / 内多边形
/// 点数一致（V10 修复：过去 r ≤ 0.5 时每角只 1 点，与 r=2 的 6 点/角不匹配 →
/// n = min(4, 24) = 4 → 4 大三角形跨越整个矩形，把 stroke 变成 fill）。
fn rounded_rect_polygon(rect: Rect, radius: f32, segs: usize) -> Vec<(f32, f32)> {
    let r = radius.clamp(0.0, rect.size.width.min(rect.size.height) / 2.0);
    let (x0, y0) = (rect.origin.x, rect.origin.y);
    let (x1, y1) = (rect.right(), rect.bottom());
    let mut pts = Vec::new();
    // 四个角弧，逆时针：左上→右上→右下→左下
    for (cx, cy, a_start) in [
        (x0 + r, y0 + r, 180.0_f32),
        (x1 - r, y0 + r, 270.0),
        (x1 - r, y1 - r, 0.0),
        (x0 + r, y1 - r, 90.0),
    ] {
        // 即便 r ≤ 0.5 也发 segs 点（都聚在 (cx,cy)），保持点数一致。
        if r <= 0.5 {
            for _ in 0..segs {
                pts.push((cx, cy));
            }
            continue;
        }
        for i in 0..segs {
            let a = (a_start + 90.0 * i as f32 / segs as f32).to_radians();
            pts.push((cx + r * a.cos(), cy + r * a.sin()));
        }
    }
    pts
}

/// 填充圆角矩形 → 三角扇（从多边形重心）。
fn emit_fill(
    verts: &mut Vec<SolidVertex>,
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    rect: Rect,
    radius: f32,
    color: Color,
) {
    if radius <= 0.5 {
        // 直矩形：两个三角形
        let (x0, y0, x1, y1) = (rect.origin.x, rect.origin.y, rect.right(), rect.bottom());
        let c = [color.r, color.g, color.b, color.a];
        verts.push(SolidVertex {
            pos: ndc(x0, y0),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(x1, y0),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(x1, y1),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(x0, y0),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(x1, y1),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(x0, y1),
            color: c,
        });
        return;
    }
    let pts = rounded_rect_polygon(rect, radius, 12);
    let (cx, cy) = (
        rect.origin.x + rect.size.width / 2.0,
        rect.origin.y + rect.size.height / 2.0,
    );
    let c = [color.r, color.g, color.b, color.a];
    for i in 0..pts.len() {
        let j = (i + 1) % pts.len();
        verts.push(SolidVertex {
            pos: ndc(cx, cy),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(pts[i].0, pts[i].1),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(pts[j].0, pts[j].1),
            color: c,
        });
    }
}

/// 描边圆角矩形 → 内外多边形连接成的环。
fn emit_stroke(
    verts: &mut Vec<SolidVertex>,
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    rect: Rect,
    radius: f32,
    thickness: f32,
    color: Color,
) {
    if thickness <= 0.0 {
        return;
    }
    if radius <= 0.5 {
        // 四条矩形边
        let (x0, y0) = (rect.origin.x, rect.origin.y);
        let (x1, y1) = (rect.right(), rect.bottom());
        let t = thickness;
        let inner_h = (y1 - y0).max(t) - 2.0 * t;
        let inner_w = (x1 - x0).max(t) - 2.0 * t;
        for r in [
            Rect::new(x0, y0, x1 - x0, t),
            Rect::new(x0, y1 - t, x1 - x0, t),
            Rect::new(x0, y0 + t, t, inner_h.max(0.0)),
            Rect::new(x1 - t, y0 + t, t, inner_h.max(0.0)),
        ] {
            emit_fill(verts, ndc, r, 0.0, color);
        }
        let _ = inner_w;
        return;
    }
    let outer = rounded_rect_polygon(rect, radius, 12);
    let inner_r = (radius - thickness).max(0.0);
    let inner_rect = Rect::new(
        rect.origin.x + thickness,
        rect.origin.y + thickness,
        rect.size.width - 2.0 * thickness,
        rect.size.height - 2.0 * thickness,
    );
    if inner_rect.size.width <= 0.0 || inner_rect.size.height <= 0.0 {
        emit_fill(verts, ndc, rect, radius, color);
        return;
    }
    let inner = rounded_rect_polygon(inner_rect, inner_r, 12);
    let c = [color.r, color.g, color.b, color.a];
    let n = outer.len().min(inner.len());
    for i in 0..n {
        let j = (i + 1) % n;
        let (ox, oy) = outer[i];
        let (ox2, oy2) = outer[j];
        let (ix, iy) = inner[i];
        let (ix2, iy2) = inner[j];
        verts.push(SolidVertex {
            pos: ndc(ox, oy),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(ix, iy),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(ox2, oy2),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(ix, iy),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(ix2, iy2),
            color: c,
        });
        verts.push(SolidVertex {
            pos: ndc(ox2, oy2),
            color: c,
        });
    }
}

/// 弧线（ProgressRing）→ 内外圆环扇形三角带。
/// 0° = 正上，顺时针（screen y 向下）。
#[allow(clippy::too_many_arguments)]
fn emit_arc(
    verts: &mut Vec<SolidVertex>,
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    center: Point,
    radius: f32,
    thickness: f32,
    color: Color,
    start_deg: f32,
    end_deg: f32,
) {
    let sweep = end_deg - start_deg;
    if sweep.abs() <= 0.5 || thickness <= 0.0 {
        return;
    }
    let r_out = radius + thickness / 2.0;
    let r_in = (radius - thickness / 2.0).max(0.0);
    let n = ((sweep.abs() / 8.0).ceil() as usize).clamp(1, 256);
    let c = [color.r, color.g, color.b, color.a];
    let mut prev_o: Option<(f32, f32)> = None;
    let mut prev_i: Option<(f32, f32)> = None;
    for k in 0..=n {
        let a = (start_deg + sweep * k as f32 / n as f32).to_radians();
        // 0° 向上、顺时针：x = r*sin, y = -r*cos
        let (s, cs) = a.sin_cos();
        let (ox, oy) = (center.x + r_out * s, center.y - r_out * cs);
        let (ix, iy) = (center.x + r_in * s, center.y - r_in * cs);
        if let (Some((px, py)), Some((qx, qy))) = (prev_o, prev_i) {
            verts.push(SolidVertex {
                pos: ndc(px, py),
                color: c,
            });
            verts.push(SolidVertex {
                pos: ndc(qx, qy),
                color: c,
            });
            verts.push(SolidVertex {
                pos: ndc(ox, oy),
                color: c,
            });
            verts.push(SolidVertex {
                pos: ndc(qx, qy),
                color: c,
            });
            verts.push(SolidVertex {
                pos: ndc(ix, iy),
                color: c,
            });
            verts.push(SolidVertex {
                pos: ndc(ox, oy),
                color: c,
            });
        }
        prev_o = Some((ox, oy));
        prev_i = Some((ix, iy));
    }
}

/// 把顶点数据写入持久缓冲（容量不足时重建并翻倍）。
/// 返回缓冲引用。避免每帧 create_buffer 的 GPU 分配（§4.1 不变量 1）。
fn upload_vertices<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &'a mut wgpu::Buffer,
    cap: &mut u32,
    verts: &[TextVertex],
) -> &'a wgpu::Buffer {
    let needed = verts.len() as u32;
    if *cap < needed {
        while *cap < needed {
            *cap = (*cap * 2).max(1024);
        }
        *buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-vert-buf"),
            size: *cap as u64 * std::mem::size_of::<TextVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }
    if !verts.is_empty() {
        queue.write_buffer(buf, 0, bytemuck::cast_slice(verts));
    }
    buf
}

/// 把形状顶点数据写入持久缓冲（SolidVertex 大小同 TextVertex 布局，通用）。
fn upload_vertices_solid<'a>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    buf: &'a mut wgpu::Buffer,
    cap: &mut u32,
    verts: &[SolidVertex],
) -> &'a wgpu::Buffer {
    let needed = verts.len() as u32;
    if *cap < needed {
        while *cap < needed {
            *cap = (*cap * 2).max(1024);
        }
        *buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-solid-buf"),
            size: *cap as u64 * std::mem::size_of::<SolidVertex>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }
    if !verts.is_empty() {
        queue.write_buffer(buf, 0, bytemuck::cast_slice(verts));
    }
    buf
}

/// 矩形求交（box 语义：内容裁剪到盒内）。不相交返回 None。
fn intersect(a: Rect, b: Rect) -> Option<Rect> {
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

/// 推入一个 quad（两三角形）。
fn push_quad(
    verts: &mut Vec<TextVertex>,
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    p0: [f32; 2],
    p1: [f32; 2],
    uv0: [f32; 2],
    uv1: [f32; 2],
    color: Color,
) {
    let c = [color.r, color.g, color.b, color.a];
    let (x0, y0) = (p0[0], p0[1]);
    let (x1, y1) = (p1[0], p1[1]);
    verts.push(TextVertex {
        pos: ndc(x0, y0),
        uv: uv0,
        color: c,
    });
    verts.push(TextVertex {
        pos: ndc(x1, y0),
        uv: [uv1[0], uv0[1]],
        color: c,
    });
    verts.push(TextVertex {
        pos: ndc(x1, y1),
        uv: uv1,
        color: c,
    });
    verts.push(TextVertex {
        pos: ndc(x0, y0),
        uv: uv0,
        color: c,
    });
    verts.push(TextVertex {
        pos: ndc(x1, y1),
        uv: uv1,
        color: c,
    });
    verts.push(TextVertex {
        pos: ndc(x0, y1),
        uv: [uv0[0], uv1[1]],
        color: c,
    });
}

/// FNV-1a 32 位哈希 —— 图标纹理缓存键（内容去重）。
fn fnv1a(data: &[u8]) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 为一条 Image 命令生成纹理 quad：`rgba` 直通像素 → `rect` 目标矩形。
/// 无 tint（None）→ 白（原色）；有 tint → 染色。key = rgba 内容 FNV 哈希。
/// 纹理上传去重由 `Renderer::ensure_image`（`contains_key`）负责，这里只排队。
#[allow(clippy::too_many_arguments)]
fn emit_image(
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    verts: &mut Vec<TextVertex>,
    runs: &mut Vec<ImageRun>,
    pending: &mut Vec<(u32, Vec<u8>, u32, u32)>,
    rgba: &[u8],
    width: u32,
    height: u32,
    rect: Rect,
    tint: Option<Color>,
) {
    let key = fnv1a(rgba);
    let c = tint.unwrap_or(Color::WHITE);
    let start = verts.len() as u32;
    push_quad(
        verts,
        ndc,
        [rect.origin.x, rect.origin.y],
        [rect.right(), rect.bottom()],
        [0.0, 0.0],
        [1.0, 1.0],
        c,
    );
    runs.push(ImageRun {
        image_key: key,
        start,
        count: 6,
    });
    pending.push((key, rgba.to_vec(), width, height));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_scissor_uses_rounded_configured_buffer() {
        assert_eq!(scissor_rect(None, 1.5, 152, 77), (0, 0, 152, 77));
    }

    #[test]
    fn fractional_edge_clip_keeps_last_physical_pixel() {
        assert_eq!(
            scissor_rect(Some(Rect::new(100.75, 0.0, 0.25, 1.0)), 1.5, 152, 2),
            (151, 0, 1, 2),
        );
    }
}
