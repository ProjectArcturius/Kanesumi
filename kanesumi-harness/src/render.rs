// render.rs — wgpu Scene 光栅化（painter's algorithm）。参 HANDOVER §1 Scene 命令光栅化。
//
// 纯色无渐变（`KANESUMI_DESIGN.md` §Ⅲ.1 铁律 6，L102；旧注释引用的「SD §II」是错误溯源）：
// 所有形状 CPU 侧三角化后走单一 color pipeline；
// 文本用 fontdue 光栅化字形 → R8 覆盖纹理 → textured quad。
// 坐标约定：Scene 逻辑像素（原点左上，y 向下）；物理像素 = 逻辑 × scale。
// 注：本模块仅 Linux（wgpu 表面需 Wayland wl_surface）。

use std::collections::HashMap;
use std::sync::Arc;

use kanesumi_canvas::geometry::{Triangle, triangulate_arc, triangulate_fill, triangulate_stroke};
use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};
use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Rect, TextStyle};

use crate::glyph_layout::{
    GlyphKey, PlacedGlyph, TextRenderTuning, glyph_key, layout_text_glyphs,
    layout_text_glyphs_tuned,
};
use crate::perf;
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
// 文字浓度补偿（§112 重调；原 G-67）。与 CPU `TextRenderTuning::tune_coverage` 同一条公式；
// 改动必须两处同步，由 harness `gpu_formula_matches_cpu_lut` 守住。
// x = contrast，y = gamma，zw 为 16 字节对齐填充。
@group(0) @binding(2) var<uniform> tune: vec4<f32>;
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
    var cov = textureSample(glyph_tex, samp, in.uv).r;
    // 恒等档短路（与 CPU 查表返回 None 一致），保证旧行为逐像素不变。
    if (tune.x != 0.0 || tune.y != 1.0) {
        let luma = clamp(0.2126 * in.color.r + 0.7152 * in.color.g + 0.0722 * in.color.b, 0.0, 1.0);
        let gamma_eff = max(tune.y * (1.0 + 0.30 * (luma - 0.5)), 0.05);
        let g = pow(cov, 1.0 / gamma_eff);
        cov = clamp(0.5 + (g - 0.5) * (1.0 + tune.x), 0.0, 1.0);
    }
    return vec4<f32>(srgb_to_linear(in.color.rgb) * in.color.a * cov, in.color.a * cov);
}
"#;

/// 图标管线：RGBA8 纹理采样。`in.color` 的 rgb 承载 tint：白色 (1,1,1) = 原色；
/// 其他 = 染色（以图标 alpha 为形状蒙版替换颜色）；`in.color.a` 承载 opacity 乘子
/// （0..=1，默认 1），叠加到输出 rgb 与 alpha。输出预乘供混合。
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
    let opacity = in.color.a;
    return vec4<f32>(rgb * opacity, src.a * opacity);
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
    pending_images: Vec<(u32, Arc<[u8]>, u32, u32)>,
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

impl Step {
    /// 该步的裁剪矩形（逻辑像素）；None = 无裁剪。
    fn clip(&self) -> Option<Rect> {
        match self {
            Step::Solid { clip, .. } | Step::Text { clip, .. } | Step::Image { clip, .. } => *clip,
        }
    }
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


pub(crate) fn scissor_rect(
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

/// 默认 MSAA 采样数。4× 是桌面 GPU 上「几何抗锯齿的甜蜜点」——
/// 圆角矩形/弧线的斜边 staircase 显著减少，成本一次 resolve pass 可接受。
/// 参 A1（本次会话新增）：40×20 胶囊在 2× 缩放下每角 6 段 tessellation 产生的边缘锯齿。
pub const MSAA_SAMPLES_DEFAULT: u32 = 4;

/// 实际 MSAA 采样数：`KANESUMI_MSAA=1` → 单采样（不建 MSAA 纹理，直画交换链），
/// 其余（未设 / 其它值）→ [`MSAA_SAMPLES_DEFAULT`]。行为默认不变。
/// 只读一次并缓存 —— 同进程内管线 / 纹理 / 交换链配置必须用同一个值。
pub fn msaa_samples() -> u32 {
    static N: std::sync::OnceLock<u32> = std::sync::OnceLock::new();
    *N.get_or_init(|| match std::env::var("KANESUMI_MSAA") {
        Ok(v) if v.trim() == "1" => 1,
        _ => MSAA_SAMPLES_DEFAULT,
    })
}

/// 进程共享的 wgpu 上下文（G1）：instance / adapter / device / queue 与选定的表面格式。
///
/// 主表面与各浮层共用一份 —— G0 实测「每个用 wgpu 的进程 RSS +55~75 MB」，共享设备
/// 把这份开销从「每表面一份」降为「每进程一份」。参
/// Ether docs/GPU_COMPOSITION_PLAN.md §Ⅲ「G1 壳层 GPU 光栅」与 §Ⅳ「内存」。
/// 惰性创建、初始化失败永久回落 CPU（platform 侧持有）。
pub struct GpuContext {
    pub(crate) instance: wgpu::Instance,
    pub(crate) adapter: wgpu::Adapter,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    /// 共享管线与配置统一使用的表面格式（sRGB 优先）。
    pub(crate) format: wgpu::TextureFormat,
}

/// 单个表面的 wgpu 光栅化器（present 直出）。持有自己的一份表面/顶点缓冲/字形缓存，
/// 但 device / queue 来自进程共享的 [`GpuContext`]。
pub struct Renderer {
    ctx: Arc<GpuContext>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    solid_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,
    image_pipeline: wgpu::RenderPipeline,
    text_bgl: wgpu::BindGroupLayout,
    /// 图标绑定布局（纹理 + 采样器，无文字浓度 uniform）。
    image_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    /// 文字浓度补偿 uniform（contrast / gamma）。GPU 片元着色器消费，见 `TEXT_SHADER`。
    tuning_buf: wgpu::Buffer,
    /// 当前文字浓度旋钮。默认 = §112 生产档。
    text_tuning: TextRenderTuning,
    glyphs: HashMap<GlyphKey, GlyphEntry>,
    /// 图标纹理缓存：key = (width,height) + rgba 内容 FNV 哈希。同一图标去重复用。
    images: HashMap<u32, GlyphEntry>,
    /// 字形位图 CPU 缓存（fontdue 光栅化结果）。key = `GlyphKey`。静态文本每帧复用，
    /// 避免重复光栅化（全链路最贵的 CPU 操作）。GPU 侧已有字形纹理缓存，此处补 CPU 侧。
    glyph_bitmaps: HashMap<GlyphKey, (kanesumi_canvas::text::GlyphMetrics, Vec<u8>)>,
    /// 持久顶点缓冲（避免每帧 create_buffer 的 GPU 分配开销，§4.1 保留视觉树）。
    solid_buf: wgpu::Buffer,
    text_buf: wgpu::Buffer,
    image_buf: wgpu::Buffer,
    /// 缓冲容量（顶点数），不足时翻倍重建。
    solid_cap: u32,
    text_cap: u32,
    image_cap: u32,
    /// MSAA 中间纹理 view（sample_count = [`msaa_samples`]）。render pass attachment 用它，
    /// swapchain view 作 resolve_target。resize 时重建。单采样（`KANESUMI_MSAA=1`）时 None ——
    /// 直接画到交换链，无中间纹理、无 resolve。
    msaa_view: Option<wgpu::TextureView>,
    /// 本渲染器实际 MSAA 采样数（1 或 4）。
    msaa_samples: u32,
    /// GPU 时间戳计时（设备支持时 Some；否则 None → 日志 `gpu=n/a`）。
    gpu_timer: Option<GpuTimer>,
    /// 最近一次 `get_current_texture` 的阻塞时长（毫秒）。真机上取帧会等合成器释放缓冲 / vblank，
    /// 这段墙钟不是绘制成本——单独记，免得与光栅混为一谈。参 Ether docs/CANVAS_PLAN.md §Ⅰ。
    last_acquire_ms: Option<f32>,
    /// 逻辑 → 物理缩放（整数，通常 1 或 2）。
    scale: f32,
    /// 逻辑尺寸。
    width: f32,
    height: f32,
}

/// 创建与当前 surface 同尺寸/格式的 MSAA 中间纹理 view（`samples` 采样）。
fn create_msaa_view(
    device: &wgpu::Device,
    config: &wgpu::SurfaceConfiguration,
    samples: u32,
) -> wgpu::TextureView {
    let tex = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("kanesumi-msaa"),
        size: wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: samples,
        dimension: wgpu::TextureDimension::D2,
        format: config.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    tex.create_view(&wgpu::TextureViewDescriptor::default())
}

// ── GPU 时间戳计时（WGPU 路径）── 参 Ether docs/research/gpu_t1（任务 gpu-t1-frame-timing）。
//
// 设备支持 `TIMESTAMP_QUERY_INSIDE_ENCODERS` 时启用：每帧在 render pass 前后各写一个时间戳，
// 环形 query set（[`perf::TIMESTAMP_SLOTS`] 槽 × 2 时间戳），把查询结果 resolve 进 GPU 缓冲、
// 再复制到 MAP_READ 缓冲，**滞后 [`perf::TIMESTAMP_LAG`] 帧**异步回读 —— 绝不 `Wait` 当帧。
// 结果落到 `SurfacePerf::gpu`（p50/p95/max），写 `ether-harness-perf.log` 的 `gpu=` 字段。
pub(crate) struct GpuTimer {
    query_set: wgpu::QuerySet,
    /// QUERY_RESOLVE | COPY_SRC；每槽 256 字节对齐（QUERY_RESOLVE_BUFFER_ALIGNMENT）。
    resolve_buf: wgpu::Buffer,
    /// 每槽一份 MAP_READ | COPY_DST 回读缓冲（16 字节 = 两个 u64 时间戳）。
    read_bufs: Vec<Arc<wgpu::Buffer>>,
    /// 每 tick 纳秒数（`Queue::get_timestamp_period`）。
    period_ns: f32,
    /// 已提交的帧序号（决定当前写槽）。
    frame: u64,
    /// 槽状态：None = 空闲可写；Some(frame) = 该槽已写、等待回读。
    slot_frame: [Option<u64>; perf::TIMESTAMP_SLOTS as usize],
    /// 槽是否已挂上 `map_async`（挂上后同一槽不得重复挂，否则 wgpu 断言已映射）。
    slot_mapped: [bool; perf::TIMESTAMP_SLOTS as usize],
    /// 槽在回读后是否计入样本（跳过帧记 false，只回收槽不记数）。
    slot_keep: [bool; perf::TIMESTAMP_SLOTS as usize],
    /// 本帧是否真的写入了时间戳（槽被占用时跳过）。
    active: bool,
    /// 回读结果的发送端（回调线程 → 主线程）。
    tx: std::sync::mpsc::Sender<(usize, f32)>,
    rx: std::sync::mpsc::Receiver<(usize, f32)>,
    /// 已完成、待取走的样本（毫秒）。有上限，后端不取也不无界增长。
    pending: Vec<f32>,
}

/// 回读样本上限（每槽一次一帧，够 10 s 窗口用；后端取走后清零）。
const GPU_PENDING_CAP: usize = 4096;
/// 每槽 resolve 区大小（256 字节对齐要求；实际用 16 字节）。
const SLOT_STRIDE: u64 = 256;

impl GpuTimer {
    /// 设备支持时间戳查询则建计时器；否则 None（静默不开）。
    pub(crate) fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let need =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !device.features().contains(need) {
            return None;
        }
        let slots = perf::TIMESTAMP_SLOTS as u32;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("kanesumi-gpu-timer"),
            ty: wgpu::QueryType::Timestamp,
            count: slots * 2,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-gpu-timer-resolve"),
            size: SLOT_STRIDE * slots as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_bufs = (0..slots)
            .map(|i| {
                Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("kanesumi-gpu-timer-read-{i}")),
                    size: 16,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }))
            })
            .collect();
        let (tx, rx) = std::sync::mpsc::channel();
        Some(Self {
            query_set,
            resolve_buf,
            read_bufs,
            period_ns: queue.get_timestamp_period(),
            frame: 0,
            slot_frame: [None; perf::TIMESTAMP_SLOTS as usize],
            slot_mapped: [false; perf::TIMESTAMP_SLOTS as usize],
            slot_keep: [true; perf::TIMESTAMP_SLOTS as usize],
            active: false,
            tx,
            rx,
            pending: Vec::new(),
        })
    }

    /// 本帧的写槽（空闲时才写；被占用则该帧不采样）。
    pub(crate) fn begin(&mut self) -> Option<usize> {
        let s = perf::timestamp_slot(self.frame, perf::TIMESTAMP_SLOTS);
        if self.slot_frame[s].is_some() {
            self.active = false;
            return None;
        }
        self.slot_frame[s] = Some(self.frame);
        self.active = true;
        Some(s)
    }

    /// 在 render pass 前后各写一个时间戳。
    pub(crate) fn write_first(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        encoder.write_timestamp(&self.query_set, slot as u32 * 2);
    }
    pub(crate) fn write_last(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        encoder.write_timestamp(&self.query_set, slot as u32 * 2 + 1);
    }

    /// 把本帧两枚时间戳 resolve 到回读缓冲（须在 `write_last` 之后、提交之前调用）。
    pub(crate) fn encode_readback(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let base = slot as u64 * 2;
        encoder.resolve_query_set(
            &self.query_set,
            base as u32..(base + 2) as u32,
            &self.resolve_buf,
            slot as u64 * SLOT_STRIDE,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve_buf,
            slot as u64 * SLOT_STRIDE,
            &self.read_bufs[slot],
            0,
            16,
        );
    }

    /// 提交后推进帧号：为滞后帧挂上非阻塞 `map_async`（回调把毫秒送回 `rx`），再看有无到位结果。
    /// `keep` = 本帧是否计入样本（跳过帧传 false，只回收槽）。
    pub(crate) fn end(&mut self, device: &wgpu::Device, keep: bool) {
        if self.active {
            let s = perf::timestamp_slot(self.frame, perf::TIMESTAMP_SLOTS);
            self.slot_keep[s] = keep;
        }
        self.frame += 1;
        // 为 LAG 帧前写下的槽挂回读（每个槽在回到可写前只挂一次）。
        if let Some(s) = perf::readback_slot_for(self.frame, perf::TIMESTAMP_SLOTS, perf::TIMESTAMP_LAG)
            && let Some(written) = self.slot_frame[s]
            && !self.slot_mapped[s]
            && self.frame >= written + perf::TIMESTAMP_LAG
        {
            self.slot_mapped[s] = true;
            let buf = self.read_bufs[s].clone();
            let reader = buf.clone();
            let period = self.period_ns;
            let tx = self.tx.clone();
            buf.slice(0..16).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    let data = reader.slice(0..16).get_mapped_range();
                    let start = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8]));
                    let end = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0; 8]));
                    drop(data);
                    let _ = tx.send((s, perf::ticks_to_ms(start, end, period)));
                } else {
                    let _ = tx.send((s, f32::NAN));
                }
            });
        }
        // 非阻塞轮询：让已完成的 map 回调跑起来（不等待当帧 GPU）。
        let _ = device.poll(wgpu::Maintain::Poll);
        // 收结果：槽 → 空闲；keep 的样本计入。
        while let Ok((s, ms)) = self.rx.try_recv() {
            let keep = std::mem::replace(&mut self.slot_keep[s], true);
            self.slot_frame[s] = None;
            self.slot_mapped[s] = false;
            self.read_bufs[s].unmap();
            if keep && ms.is_finite() && self.pending.len() < GPU_PENDING_CAP {
                self.pending.push(ms);
            }
        }
    }

    /// 取走已完成样本（平台层每帧收取，记进对应表面的 `SurfacePerf::gpu`）。
    pub(crate) fn drain(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.pending)
    }
}

/// 从 wl_display / wl_surface 原始指针创建 wgpu 表面（同 launcher render.rs 模式）。
/// 供 `GpuContext` 选适配器与 `Renderer` 建各自表面共用。
pub(crate) fn create_wl_surface(
    instance: &wgpu::Instance,
    conn: &Connection,
    wl_surface: &WlSurface,
) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError> {
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
    unsafe {
        instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle {
            raw_display_handle,
            raw_window_handle,
        })
    }
}

/// 渲染器初始化错误。
#[derive(Debug)]
pub enum RendererError {
    Surface(wgpu::CreateSurfaceError),
    Adapter,
    Device(wgpu::RequestDeviceError),
    /// 共享上下文的表面格式不被目标表面支持（同一合成器下不应发生）。
    IncompatibleFormat { wanted: wgpu::TextureFormat },
}

impl GpuContext {
    /// 进程共享上下文：挨个后端候选（主→备）建 instance/device/queue 与选定格式。
    /// `wl_surface` 仅用于挑一个与该表面兼容的适配器，函数返回后临时表面即丢弃。
    pub fn new(
        conn: &Connection,
        wl_surface: &WlSurface,
        backends: &[wgpu::Backends],
    ) -> Result<Arc<Self>, RendererError> {
        for (i, backend) in backends.iter().enumerate() {
            match Self::new_with_backend(conn, wl_surface, *backend) {
                Ok(c) => return Ok(c),
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
        backend: wgpu::Backends,
    ) -> Result<Arc<Self>, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..Default::default()
        });
        // 拉起时间线：Vulkan/WGPU 实例创建（ICD 扫描）。
        crate::timeline::note_once("gpu_instance");
        let surface =
            create_wl_surface(&instance, conn, wl_surface).map_err(RendererError::Surface)?;

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
        // 拉起时间线：适配器选定（此前的实例 / 表面 / 适配器枚举一起计时）。
        crate::timeline::note_once("gpu_adapter");

        // GPU 时间戳查询（帧耗时实测）：请求失败则静默重试无特性设备，GPU 计时关闭。
        // ⚠ 必须同时请求 `TIMESTAMP_QUERY`（查询类型许可）与 `..._INSIDE_ENCODERS`（允许在
        //   命令编码器里写时间戳）—— 二者是独立的特性位，只请求后者会在 create_query_set
        //   时校验失败（Features(TIMESTAMP_QUERY) are required but not enabled）。
        // 参 Ether docs/research/gpu_t1（任务 gpu-t1-frame-timing）。
        let want =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let (device, queue) = match pollster::block_on(
            adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kanesumi-device"),
                    required_features: want,
                    ..Default::default()
                },
                None,
            ),
        ) {
            Ok(dq) => dq,
            Err(e) => {
                log::info!("时间戳查询特性请求失败（{e}），GPU 计时关闭");
                pollster::block_on(
                    adapter.request_device(&wgpu::DeviceDescriptor::default(), None),
                )
                .map_err(RendererError::Device)?
            }
        };
        crate::timeline::note_once("gpu_device");

        let caps = surface.get_capabilities(&adapter);
        // 诊断：surface capabilities 的 alpha_modes / formats（排查「主表面无法透明」）。
        log::warn!(
            "kanesumi surface caps: alpha_modes={:?} formats={:?}",
            caps.alpha_modes,
            caps.formats,
        );
        // 用 sRGB 格式：与 eframe（librarian 可见）对齐。⚠ 合成器（GLES）import 非 sRGB
        // dmabuf（XRGB8888，无 alpha）时 alpha 通道读 0 → 整个 buffer 透明（背景消失、
        // 文字浮空）；sRGB（ARGB8888，有 alpha）→ 可见。参 session.log + Known Issue #8。
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        // 临时表面仅用于挑适配器；真实表面由各 Renderer 自建（`with_context`）。
        drop(surface);

        Ok(Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            format,
        }))
    }

    /// 共享管线/配置使用的表面格式。
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
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
    /// ⚠ 本渲染器只服务 xdg-shell 直出（present）。layer-shell 角色走 CpuRenderer
    ///   （cpu_raster.rs）→ wl_shm，不再有离屏读回路径。参 TOPBAR_RENDER_REFACTOR。
    pub fn new(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
    ) -> Result<Self, RendererError> {
        Self::new_with_backends(
            conn,
            wl_surface,
            width,
            height,
            scale,
            transparent,
            // ⚠ 实验：Vulkan 优先（支持 PreMultiplied alpha，主表面可透明）。
            // 之前 Vulkan 在 Ether 下 SURFACE_LOST，故 GL 优先；GL 复位实验后重试 Vulkan。
            &[wgpu::Backends::VULKAN, wgpu::Backends::GL],
        )
    }

    /// 显式指定后端候选（主→备）。建进程共享上下文，再为本表面建表面/管线。
    pub fn new_with_backends(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
        backends: &[wgpu::Backends],
    ) -> Result<Self, RendererError> {
        let ctx = GpuContext::new(conn, wl_surface, backends)?;
        Self::with_context(ctx, conn, wl_surface, width, height, scale, transparent)
    }

    /// 用进程共享的 [`GpuContext`] 为单个 wl_surface 建表面 / 管线 / 顶点缓冲。
    /// 共享 device/queue 与选定格式 —— 每进程一份 wgpu 设备而非每表面一份（G1 RSS）。
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
            create_wl_surface(&ctx.instance, conn, wl_surface).map_err(RendererError::Surface)?;
        let caps = surface.get_capabilities(&ctx.adapter);
        let format = ctx.format;
        if !caps.formats.contains(&format) {
            return Err(RendererError::IncompatibleFormat { wanted: format });
        }
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
        // device 借用共享上下文（wgpu 22 的 Device 非 Clone，用引用即可）。
        let device = &ctx.device;
        // 实际 MSAA 采样数（进程内一致；`KANESUMI_MSAA=1` → 单采样）。
        let msaa = msaa_samples();

        let (pw, ph) = (width * scale, height * scale);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            view_formats: vec![format],
            alpha_mode,
            width: pw.round().max(1.0) as u32,
            height: ph.round().max(1.0) as u32,
            desired_maximum_frame_latency: 2,
            // 优先非阻塞呈现（G1 修复）：内容变化帧由损伤驱动，若用 FIFO，`acquire`
            // 会等合成器释放上一帧缓冲（~16.7 ms），把「每帧光栅耗时」从真实 GPU 工作
            // （<1 ms）抬高到 ~19 ms，G1 的浮层 GPU 收益被 vsync 等待吃掉。
            // Mailbox 可丢旧帧、acquire 立返；合成器不广告时回落 Immediate / FIFO。
            present_mode: {
                let chosen = choose_present_mode(&caps.present_modes);
                log::info!(
                    "kanesumi surface present_modes={:?} → 选 {:?}",
                    caps.present_modes,
                    chosen
                );
                chosen
            },
        };
        surface.configure(device, &config);

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
                count: msaa,
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
        // 图标绑定布局：纹理 + 采样器（与文本共用顶点布局，但不需文字浓度 uniform）。
        let image_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("kanesumi-image-bgl"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
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
                count: msaa,
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
            bind_group_layouts: &[&image_bgl],
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
                count: msaa,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        // 文字浓度补偿 uniform（16 字节 = 4×f32，满足 uniform 最小绑定大小）。
        let tuning_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-text-tuning"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let text_tuning = TextRenderTuning::default();
        ctx.queue.write_buffer(
            &tuning_buf,
            0,
            bytemuck::cast_slice(&[text_tuning.contrast, text_tuning.gamma, 0.0f32, 0.0f32]),
        );

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

        // 单采样（KANESUMI_MSAA=1）不建中间纹理，直接画到交换链。
        let msaa_view = if msaa > 1 {
            Some(create_msaa_view(device, &config, msaa))
        } else {
            None
        };
        let gpu_timer = GpuTimer::new(device, &ctx.queue);
        log::info!(
            "kanesumi 光栅器：msaa={msaa} gpu_timer={}",
            if gpu_timer.is_some() { "on" } else { "n/a" }
        );
        // 拉起时间线：三条管线 / 绑定布局 / 缓冲建完（首帧前的最后一段 GPU 初始化）。
        crate::timeline::note_once("renderer_pipelines");

        Ok(Self {
            ctx,
            surface,
            config,
            solid_pipeline,
            text_pipeline,
            image_pipeline,
            text_bgl,
            image_bgl,
            sampler,
            tuning_buf,
            text_tuning,
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
            msaa_samples: msaa,
            gpu_timer,
            last_acquire_ms: None,
            scale,
            width,
            height,
        })
    }

    /// 设定文字浓度旋钮。变化时写 uniform 并清空字形位图 / 纹理缓存 ——
    /// 加粗会改变字形位图与放置几何，旧缓存对新旋钮不再有效。
    /// 设为 `TextRenderTuning::identity()` 即恢复改动前行为（覆盖率不过表、字形原样）。
    pub fn set_text_tuning(&mut self, tuning: TextRenderTuning) {
        if self.text_tuning == tuning {
            return;
        }
        self.text_tuning = tuning;
        self.ctx.queue.write_buffer(
            &self.tuning_buf,
            0,
            bytemuck::cast_slice(&[tuning.contrast, tuning.gamma, 0.0f32, 0.0f32]),
        );
        self.glyphs.clear();
        self.glyph_bitmaps.clear();
    }

    /// 诊断：当前文字浓度旋钮。
    pub fn text_tuning(&self) -> TextRenderTuning {
        self.text_tuning
    }

    /// 重配尺寸（逻辑）与缩放。configure 事件触发。
    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        self.width = width;
        self.height = height;
        self.scale = scale;
        let (pw, ph) = (width * scale, height * scale);
        self.config.width = pw.round().max(1.0) as u32;
        self.config.height = ph.round().max(1.0) as u32;
        self.surface.configure(&self.ctx.device, &self.config);
        if self.msaa_samples > 1 {
            self.msaa_view = Some(create_msaa_view(
                &self.ctx.device,
                &self.config,
                self.msaa_samples,
            ));
        }
    }

    /// 诊断：当前表面格式 / alpha_mode / buffer 物理尺寸（排查合成器下显示透明）。
    pub fn diagnostics(&self) -> String {
        format!(
            "format={:?} alpha_mode={:?} buffer={}x{} (逻辑 {:.0}x{:.0}, scale {:.0}) msaa={} gpu_timer={}",
            self.config.format,
            self.config.alpha_mode,
            self.config.width,
            self.config.height,
            self.width,
            self.height,
            self.scale,
            self.msaa_samples,
            if self.gpu_timer.is_some() { "on" } else { "n/a" },
        )
    }

    /// 实际 MSAA 采样数（1 / 4）。
    pub fn msaa_samples(&self) -> u32 {
        self.msaa_samples
    }

    /// GPU 时间戳计时是否可用（不可用时 perf 日志写 `gpu=n/a`）。
    /// 取走最近一帧取交换链纹理的阻塞时长（毫秒；未取过为 None）。
    pub fn take_acquire_ms(&mut self) -> Option<f32> {
        self.last_acquire_ms.take()
    }

    pub fn gpu_timing_supported(&self) -> bool {
        self.gpu_timer.is_some()
    }

    /// 取走已回读到的 GPU 帧耗时样本（毫秒）。平台层每帧调用并记进对应表面的
    /// `SurfacePerf::gpu`；异步滞后回读，故与绘制调用分开。参 perf.rs。
    pub fn drain_gpu_samples(&mut self) -> Vec<f32> {
        self.gpu_timer.as_mut().map(|t| t.drain()).unwrap_or_default()
    }

    /// 物理像素尺寸（读回 / SHM 提交用，与 config 同步）。
    pub fn physical_size(&self) -> (u32, u32) {
        (self.config.width, self.config.height)
    }

    /// 把一帧 Scene 光栅化到当前表面并提交（present 模式，全幅）。
    pub fn render(&mut self, engine: &TextEngine, scene: &Scene) {
        self.render_with_damage(engine, scene, None);
    }

    /// 损伤感知绘制（G1）：`damage = None` 全幅重画（clear）；
    /// `Some(rect)`（逻辑像素）只重画该矩形 —— scissor 与各绘制步裁剪求交，
    /// MSAA 纹理以 `Load` 保留上一帧其余内容。零面积损伤 = 无变化，完全不提交。
    ///
    /// 参 Ether docs/GPU_COMPOSITION_PLAN.md §Ⅲ「GPU 路径损伤感知」。
    pub fn render_with_damage(&mut self, engine: &TextEngine, scene: &Scene, damage: Option<Rect>) {
        let (pw, ph) = (self.config.width as f32, self.config.height as f32);
        if pw < 1.0 || ph < 1.0 {
            return;
        }
        // 零面积 = 本帧无变化：不取帧纹理、不提交。
        if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
            return;
        }
        let frame = self.build_frame(engine, scene);
        let acquire_at = std::time::Instant::now();
        let acquired = self.surface.get_current_texture();
        self.last_acquire_ms = Some(acquire_at.elapsed().as_secs_f32() * 1000.0);
        let surface_texture = match acquired {
            Ok(t) => t,
            Err(e) => {
                log::warn!("kanesumi-harness 获取帧纹理失败：{e}");
                return;
            }
        };
        let view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.draw_frame(&frame, &view, damage);
        surface_texture.present();
    }

    /// 构建一帧顶点与绘制步（Scene → 顶点）。present 路径专用。
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
        let mut pending_images: Vec<(u32, Arc<[u8]>, u32, u32)> = Vec::new();
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
                    push_triangles(
                        &mut solid,
                        &ndc,
                        &triangulate_fill(*rect, *corner_radius),
                        *color,
                    );
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
                    push_triangles(
                        &mut solid,
                        &ndc,
                        &triangulate_stroke(*rect, *corner_radius, *thickness),
                        *color,
                    );
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
                    push_triangles(
                        &mut solid,
                        &ndc,
                        &triangulate_arc(*center, *radius, *thickness, *start_deg, *end_deg),
                        *color,
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
                    // 单行 label 的纵向对齐：Top 恒等；Center / Bottom 换成一行行盒矩形，
                    // 排版与裁剪同用它（矩形矮于一行时行盒反而更大，字不被裁）。参 o4 纵向对齐。
                    let rect =
                        crate::glyph_layout::text_rect_with_valign(*rect, *style, *wrap, *max_lines);
                    let text_clip = match effective_clip(&clip_stack) {
                        Some(parent) => intersect(parent, rect),
                        None => intersect(surface_bounds, rect),
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
                        rect,
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
                    opacity,
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
                        rgba.as_ref(),
                        *width,
                        *height,
                        *rect,
                        *tint,
                        *opacity,
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
                    push_triangles(
                        &mut solid,
                        &ndc,
                        &[Triangle::new(*p0, *p1, *p2)],
                        *color,
                    );
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
    /// `resolve` = swapchain 视图（present 模式）。`damage = Some(rect)` 时只重画该矩形
    /// （scissor），MSAA 纹理 `Load` 上一帧内容 → 其余像素保留；`None` 全幅 clear。
    fn draw_frame(
        &mut self,
        frame: &FrameData,
        resolve: &wgpu::TextureView,
        damage: Option<Rect>,
    ) {
        // 先建字形纹理（借用分离）
        for key in &frame.pending_glyphs {
            self.ensure_glyph(*key);
        }
        // 再建图标纹理（借用分离）
        for (key, rgba, w, h) in &frame.pending_images {
            self.ensure_image(*key, rgba.as_ref(), *w, *h);
        }

        let mut encoder = self
            .ctx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kanesumi-frame"),
            });

        // GPU 计时：pass 前写起始时间戳（槽被占用则该帧不采样）。
        let timer_slot = self.gpu_timer.as_mut().and_then(|t| t.begin());
        if let (Some(t), Some(slot)) = (self.gpu_timer.as_ref(), timer_slot) {
            t.write_first(&mut encoder, slot);
        }

        {
            // 全幅 clear；损伤帧 Load 保留 MSAA 上一帧内容（store=Store 使其跨帧存活）。
            // ⚠ 单采样（KANESUMI_MSAA=1）时交换链内容未定义，无法 Load → 损伤帧也全幅重画
            //   （不做「持久单采样纹理 + 复制到交换链」，取舍见 docs/research/gpu_t1/REPORT.md）。
            let single = self.msaa_samples <= 1;
            let load = if single {
                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
            } else {
                match damage {
                    Some(_) => wgpu::LoadOp::Load,
                    None => wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                }
            };
            let (view, resolve_target): (&wgpu::TextureView, Option<&wgpu::TextureView>) = if single {
                (resolve, None)
            } else {
                (
                    self.msaa_view.as_ref().expect("MSAA 采样 >1 但中间纹理缺失"),
                    Some(resolve),
                )
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-pass"),
                // MSAA：多重采样纹理作 attachment，resolve 视图作 resolve_target。
                // pass 结束时硬件自动 4→1 downsample 到 resolve（swapchain）。
                // store=Store（非 Discard）是为损伤帧 Load 保留上一帧内容服务。
                // 单采样：直接画到交换链，无 resolve。
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target,
                    ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
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
                    &self.ctx.device,
                    &self.ctx.queue,
                    &mut self.solid_buf,
                    &mut self.solid_cap,
                    &frame.solid,
                ))
            } else {
                None
            };
            let text_buf = if !frame.text.is_empty() {
                Some(upload_vertices(
                    &self.ctx.device,
                    &self.ctx.queue,
                    &mut self.text_buf,
                    &mut self.text_cap,
                    &frame.text,
                ))
            } else {
                None
            };
            let image_buf = if !frame.image.is_empty() {
                Some(upload_vertices(
                    &self.ctx.device,
                    &self.ctx.queue,
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
                // 损伤裁剪（G1）：与步骤自身裁剪求交；空交集 → 整步跳过（不动颜色）。
                let Some(clip) = damage_clip(damage, step.clip()) else {
                    continue;
                };
                match step {
                    Step::Solid { start, count, .. } => {
                        let Some(buf) = solid_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.solid_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, clip);
                        pass.draw(*start..*start + *count, 0..1);
                    }
                    Step::Text {
                        run_start, run_end, ..
                    } => {
                        let Some(buf) = text_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.text_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, clip);
                        for run in &frame.text_runs[*run_start as usize..*run_end as usize] {
                            let Some(glyph) = self.glyphs.get(&run.glyph_key) else {
                                continue;
                            };
                            pass.set_bind_group(0, &glyph.bind_group, &[]);
                            pass.draw(run.start..run.start + run.count, 0..1);
                        }
                    }
                    Step::Image {
                        run_start, run_end, ..
                    } => {
                        let Some(buf) = image_buf.as_ref() else {
                            continue;
                        };
                        pass.set_pipeline(&self.image_pipeline);
                        pass.set_vertex_buffer(0, buf.slice(..));
                        set_scissor(&mut pass, clip);
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

        // GPU 计时：pass 后写结束时间戳，并把本帧两枚时间戳 resolve 到回读缓冲。
        if let (Some(t), Some(slot)) = (self.gpu_timer.as_ref(), timer_slot) {
            t.write_last(&mut encoder, slot);
            t.encode_readback(&mut encoder, slot);
        }

        self.ctx.queue.submit(Some(encoder.finish()));

        // 推进计时器帧号并非阻塞收结果（滞后数帧，绝不阻塞当帧）。
        // 每帧都算有效帧（本函数只在确有内容要画时调用）→ keep = true。
        if let Some(t) = self.gpu_timer.as_mut() {
            t.end(&self.ctx.device, true);
        }
    }

    /// 排版一段文本并产出字形 quad。placement 与 CPU 光栅器共用
    /// `layout_text_glyphs`（参 cpu_raster.rs），两后端几何同源。
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
        let placed = if self.text_tuning.needs_glyph_tuning() {
            layout_text_glyphs_tuned(
                engine,
                &mut self.glyph_bitmaps,
                content,
                rect,
                style,
                align,
                wrap,
                max_lines,
                overflow,
                self.scale,
                self.text_tuning,
            )
        } else {
            layout_text_glyphs(
                engine,
                &mut self.glyph_bitmaps,
                content,
                rect,
                style,
                align,
                wrap,
                max_lines,
                overflow,
                self.scale,
            )
        };
        for g in placed {
            let (x1, y1) = (g.x + g.w, g.y + g.h);
            let start = verts.len() as u32;
            push_quad(
                verts,
                ndc,
                [g.x, g.y],
                [x1, y1],
                [0.0, 0.0],
                [1.0, 1.0],
                color,
            );
            runs.push(TextRun {
                glyph_key: g.key,
                start,
                count: 6,
            });
            pending.push(g.key);
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
        let texture = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
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
        self.ctx.queue.write_texture(
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
        let bind_group = self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.tuning_buf.as_entire_binding(),
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
        let texture = self.ctx.device.create_texture(&wgpu::TextureDescriptor {
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
        self.ctx.queue.write_texture(
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
        let bind_group = self.ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("kanesumi-image-bg"),
            layout: &self.image_bgl,
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

/// 三角形列表 → solid 顶点（NDC 变换 + 直通色）。
fn push_triangles(
    verts: &mut Vec<SolidVertex>,
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    tris: &[Triangle],
    color: Color,
) {
    let c = [color.r, color.g, color.b, color.a];
    for t in tris {
        for p in [t.p0, t.p1, t.p2] {
            verts.push(SolidVertex {
                pos: ndc(p.x, p.y),
                color: c,
            });
        }
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
pub(crate) fn intersect(a: Rect, b: Rect) -> Option<Rect> {
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

/// 选呈现模式（G1）：优先 `Mailbox`（可丢旧帧、acquire 立返，避免 FIFO 的并发等待
/// 把内容变化帧的「光栅耗时」抬到 ~16.7 ms），其次 `Immediate`，最后回落 `Fifo`。
pub(crate) fn choose_present_mode(available: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    if available.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if available.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        wgpu::PresentMode::Fifo
    }
}

/// 合并全局损伤裁剪与步骤裁剪（G1 损伤感知）。
/// 返回 `Some(None)` = 全表面（无损伤且步骤无裁剪）；`Some(Some(r))` = 裁剪到 r；
/// `None` = 两者无交集 → 该步整步跳过（不动颜色）。
fn damage_clip(damage: Option<Rect>, clip: Option<Rect>) -> Option<Option<Rect>> {
    match (damage, clip) {
        (None, c) => Some(c),
        (Some(d), None) => Some(Some(d)),
        (Some(d), Some(c)) => intersect(c, d).map(Some),
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
/// 无 tint（None）→ 白（原色）；有 tint → 染色。`opacity` 存入顶点色 alpha（shader 乘子）。
/// key = rgba 内容 FNV 哈希。纹理上传去重由 `Renderer::ensure_image`（`contains_key`）负责，这里只排队。
#[allow(clippy::too_many_arguments)]
fn emit_image(
    ndc: &dyn Fn(f32, f32) -> [f32; 2],
    verts: &mut Vec<TextVertex>,
    runs: &mut Vec<ImageRun>,
    pending: &mut Vec<(u32, Arc<[u8]>, u32, u32)>,
    rgba: &[u8],
    width: u32,
    height: u32,
    rect: Rect,
    tint: Option<Color>,
    opacity: f32,
) {
    let key = fnv1a(rgba);
    let c = tint.unwrap_or(Color::WHITE).with_alpha(opacity.clamp(0.0, 1.0));
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
    pending.push((key, Arc::from(rgba), width, height));
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

    #[test]
    fn damage_clip_无损伤时保留步骤裁剪() {
        assert_eq!(damage_clip(None, None), Some(None));
        let c = Rect::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(damage_clip(None, Some(c)), Some(Some(c)));
    }

    #[test]
    fn damage_clip_有损伤时限制到损伤区() {
        let d = Rect::new(10.0, 10.0, 20.0, 20.0);
        // 步骤无裁剪 → 裁剪到损伤。
        assert_eq!(damage_clip(Some(d), None), Some(Some(d)));
        // 步骤裁剪完全包含损伤 → 交集仍是损伤。
        let outer = Rect::new(0.0, 0.0, 100.0, 100.0);
        assert_eq!(damage_clip(Some(d), Some(outer)), Some(Some(d)));
        // 部分重叠 → 交集。
        let part = Rect::new(15.0, 15.0, 40.0, 40.0);
        assert_eq!(
            damage_clip(Some(d), Some(part)),
            Some(Some(Rect::new(15.0, 15.0, 15.0, 15.0)))
        );
    }

    #[test]
    fn damage_clip_无交集则整步跳过() {
        let d = Rect::new(0.0, 0.0, 5.0, 5.0);
        let far = Rect::new(50.0, 50.0, 10.0, 10.0);
        assert_eq!(damage_clip(Some(d), Some(far)), None);
    }

    #[test]
    fn present_mode_优先非阻塞() {
        use wgpu::PresentMode::{Fifo, Immediate, Mailbox};
        // 合成器广告 Mailbox → 选 Mailbox（避免 FIFO 并发等待）。
        assert_eq!(choose_present_mode(&[Mailbox, Fifo]), Mailbox);
        assert_eq!(choose_present_mode(&[Fifo, Mailbox]), Mailbox);
        // 无 Mailbox 有 Immediate → Immediate。
        assert_eq!(choose_present_mode(&[Fifo, Immediate]), Immediate);
        // 只有 Fifo → 回落 Fifo（保底可用）。
        assert_eq!(choose_present_mode(&[Fifo]), Fifo);
        // 空表（异常输入）→ Fifo。
        assert_eq!(choose_present_mode(&[]), Fifo);
    }
}
