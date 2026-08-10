// render.rs — wgpu Scene 光栅化（painter's algorithm）。参 HANDOVER §1 Scene 命令光栅化。
//
// 纯色无渐变（SD §II）：所有形状 CPU 侧三角化后走单一 color pipeline；
// 文本用 fontdue 光栅化字形 → R8 覆盖纹理 → textured quad。
// 坐标约定：Scene 逻辑像素（原点左上，y 向下）；物理像素 = 逻辑 × scale。
// 注：本模块仅 Linux（wgpu 表面需 Wayland wl_surface）。

use std::collections::HashMap;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand, TextAlign};
use kanesumi_core::{Color, Point, Rect, TextStyle};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};
use wgpu::util::DeviceExt;

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
    return vec4<f32>(in.color.rgb * in.color.a, in.color.a);
}
"#;

const TEXT_SHADER: &str = r#"
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
    return vec4<f32>(in.color.rgb * in.color.a * cov, in.color.a * cov);
}
"#;

/// 图标管线：RGBA8 纹理采样。`in.color` 承载 tint：白色 (1,1,1) = 原色；
/// 其他 = 染色（以图标 alpha 为形状蒙版替换颜色）。输出预乘供混合。
const IMAGE_SHADER: &str = r#"
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
    let rgb = select(src.rgb * src.a, in.color.rgb * src.a, is_tint);
    return vec4<f32>(rgb, src.a);
}
"#;

// ── 字形缓存 ─────────────────────────────────────────────────────────────

/// 单个字形：R8 覆盖纹理 + 绑定组 + 度量。key = (char, 物理字号取整)。
struct GlyphEntry {
    #[allow(dead_code)]
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

/// 一段文本字形绘制区间：glyph key + 顶点范围（6 顶点/quad）。
struct TextRun {
    glyph_key: u32,
    start: u32,
    count: u32,
}

// ── 光栅化器 ─────────────────────────────────────────────────────────────

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
    glyphs: HashMap<u32, GlyphEntry>,
    /// 图标纹理缓存：key = (width,height) + rgba 内容 FNV 哈希。同一图标去重复用。
    images: HashMap<u32, GlyphEntry>,
    /// 逻辑 → 物理缩放（整数，通常 1 或 2）。
    scale: f32,
    /// 逻辑尺寸。
    width: f32,
    height: f32,
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
    pub fn new(
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
    ) -> Result<Self, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
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
        .ok_or(RendererError::Adapter)?;

        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .map_err(RendererError::Device)?;

        let caps = surface.get_capabilities(&adapter);
        // 用非 sRGB 格式：shader 直通写入的即 sRGB 色值（Metro 纯色）。
        // 若选 sRGB 表面格式，硬件会再做 linear→sRGB，颜色被提亮（双伽马）。
        let format = caps
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);
        let alpha_mode = caps
            .alpha_modes
            .iter()
            .find(|a| **a == wgpu::CompositeAlphaMode::PreMultiplied)
            .copied()
            .unwrap_or(wgpu::CompositeAlphaMode::Auto);

        let (pw, ph) = (width * scale, height * scale);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            view_formats: vec![format],
            alpha_mode,
            width: pw.max(1.0) as u32,
            height: ph.max(1.0) as u32,
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
            multisample: wgpu::MultisampleState::default(),
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
            multisample: wgpu::MultisampleState::default(),
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
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

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
        self.config.width = pw.max(1.0) as u32;
        self.config.height = ph.max(1.0) as u32;
        self.surface.configure(&self.device, &self.config);
    }

    /// 逻辑尺寸。
    pub fn size(&self) -> (f32, f32) {
        (self.width, self.height)
    }

    /// 把一帧 Scene 光栅化到当前表面并提交。
    pub fn render(&mut self, engine: &TextEngine, scene: &Scene) {
        let (pw, ph) = (self.width * self.scale, self.height * self.scale);
        if pw < 1.0 || ph < 1.0 {
            return;
        }
        let (px, py) = (self.width, self.height);

        // 逻辑 → NDC（y 翻转）。
        let ndc = |x: f32, y: f32| -> [f32; 2] { [(x / px) * 2.0 - 1.0, 1.0 - (y / py) * 2.0] };

        let mut solid: Vec<SolidVertex> = Vec::new();
        let mut text: Vec<TextVertex> = Vec::new();
        let mut text_runs: Vec<TextRun> = Vec::new();
        let mut pending_glyphs: Vec<(u32, Vec<u8>, fontdue::Metrics)> = Vec::new();
        let mut image: Vec<TextVertex> = Vec::new();
        let mut image_runs: Vec<TextRun> = Vec::new();
        let mut pending_images: Vec<(u32, Vec<u8>, u32, u32)> = Vec::new();

        for cmd in &scene.commands {
            match cmd {
                SceneCommand::FillRect {
                    color,
                    rect,
                    corner_radius,
                } => {
                    emit_fill(&mut solid, &ndc, *rect, *corner_radius, *color);
                }
                SceneCommand::StrokeRect {
                    color,
                    rect,
                    thickness,
                    corner_radius,
                } => {
                    emit_stroke(&mut solid, &ndc, *rect, *corner_radius, *thickness, *color);
                }
                SceneCommand::Arc {
                    center,
                    radius,
                    thickness,
                    color,
                    start_deg,
                    end_deg,
                } => {
                    emit_arc(
                        &mut solid, &ndc, *center, *radius, *thickness, *color, *start_deg,
                        *end_deg,
                    );
                }
                SceneCommand::Text {
                    content,
                    rect,
                    color,
                    style,
                    align,
                } => {
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
                    );
                }
                SceneCommand::Image {
                    rgba,
                    width,
                    height,
                    rect,
                    tint,
                } => {
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
                }
            }
        }

        // 先建字形纹理（借用分离）
        for (key, bitmap, metrics) in &pending_glyphs {
            self.ensure_glyph(*key, bitmap, metrics);
        }
        // 再建图标纹理（借用分离）
        for (key, rgba, w, h) in &pending_images {
            self.ensure_image(*key, rgba, *w, *h);
        }

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

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("kanesumi-frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("kanesumi-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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

            // 形状（单次 draw）
            if !solid.is_empty() {
                let buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("kanesumi-solid-verts"),
                        contents: bytemuck::cast_slice(&solid),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.solid_pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                pass.draw(0..solid.len() as u32, 0..1);
            }

            // 文本（逐字形，独立绑定纹理）
            if !text.is_empty() {
                let buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("kanesumi-text-verts"),
                        contents: bytemuck::cast_slice(&text),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.text_pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                for run in &text_runs {
                    let Some(glyph) = self.glyphs.get(&run.glyph_key) else {
                        continue;
                    };
                    pass.set_bind_group(0, &glyph.bind_group, &[]);
                    pass.draw(run.start..run.start + run.count, 0..1);
                }
            }

            // 图标（逐纹理，独立绑定）
            if !image.is_empty() {
                let buf = self
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("kanesumi-image-verts"),
                        contents: bytemuck::cast_slice(&image),
                        usage: wgpu::BufferUsages::VERTEX,
                    });
                pass.set_pipeline(&self.image_pipeline);
                pass.set_vertex_buffer(0, buf.slice(..));
                for run in &image_runs {
                    let Some(tex) = self.images.get(&run.glyph_key) else {
                        continue;
                    };
                    pass.set_bind_group(0, &tex.bind_group, &[]);
                    pass.draw(run.start..run.start + run.count, 0..1);
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        surface_texture.present();
    }

    /// 排版一段文本并产出字形 quad。
    #[allow(clippy::too_many_arguments)]
    fn emit_text(
        &mut self,
        engine: &TextEngine,
        ndc: &dyn Fn(f32, f32) -> [f32; 2],
        verts: &mut Vec<TextVertex>,
        runs: &mut Vec<TextRun>,
        pending: &mut Vec<(u32, Vec<u8>, fontdue::Metrics)>,
        content: &str,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
    ) {
        let size_phys = style.size * self.scale;
        let lines = engine.layout(content, style.size, rect.size.width);
        let line_advance = style.line_height * self.scale;
        let ascent_phys = engine.ascent(size_phys);

        let mut line_y = rect.origin.y * self.scale;
        for line in &lines {
            // 对齐决定行首 x（逻辑）
            let line_w = line.width;
            let x_log = match align {
                TextAlign::Left => rect.origin.x,
                TextAlign::Center => rect.origin.x + (rect.size.width - line_w) / 2.0,
                TextAlign::Right => rect.origin.x + rect.size.width - line_w,
            };
            let baseline = line_y + ascent_phys;
            let mut pen = x_log * self.scale;
            for c in line.content.chars() {
                let (metrics, bitmap) = engine.rasterize(c, size_phys);
                if metrics.width == 0 || metrics.height == 0 {
                    pen += metrics.advance_width;
                    continue;
                }
                let key = glyph_key(c, size_phys.round() as u32);
                let x0 = pen + metrics.xmin as f32;
                let y0 = baseline - metrics.ymin as f32 - metrics.height as f32;
                let (w, h) = (metrics.width as f32, metrics.height as f32);
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
                pending.push((key, bitmap, metrics));
                pen += metrics.advance_width;
            }
            line_y += line_advance;
        }
    }

    /// 确保字形纹理存在。
    fn ensure_glyph(&mut self, key: u32, bitmap: &[u8], metrics: &fontdue::Metrics) {
        if self.glyphs.contains_key(&key) {
            return;
        }
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

/// (char, 物理字号) → 稳定 u32 键。
fn glyph_key(c: char, size_px: u32) -> u32 {
    (c as u32) << 16 | (size_px & 0xFFFF)
}

// ── 形状三角化（逻辑坐标 → 已转 NDC 的顶点）────────────────────────────

/// 圆角矩形边界点（逆时针，含所有角弧）。`segs` 每角弧段数。
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
        if r <= 0.5 {
            pts.push((cx, cy));
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
    let pts = rounded_rect_polygon(rect, radius, 6);
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
    let outer = rounded_rect_polygon(rect, radius, 6);
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
    let inner = rounded_rect_polygon(inner_rect, inner_r, 6);
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
    runs: &mut Vec<TextRun>,
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
    runs.push(TextRun {
        glyph_key: key,
        start,
        count: 6,
    });
    pending.push((key, rgba.to_vec(), width, height));
}
