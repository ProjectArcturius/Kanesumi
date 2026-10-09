// context.rs —— 进程共享的 wgpu 上下文与渲染错误。
use std::sync::Arc;

/// 渲染器初始化错误。
#[derive(Debug)]
pub enum RendererError {
    Surface(wgpu::CreateSurfaceError),
    Adapter,
    Device(wgpu::RequestDeviceError),
    /// 共享上下文的表面格式不被目标表面支持。
    IncompatibleFormat { wanted: wgpu::TextureFormat },
}

impl std::fmt::Display for RendererError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Surface(e) => write!(f, "表面创建失败：{e}"),
            Self::Adapter => write!(f, "无可用适配器"),
            Self::Device(e) => write!(f, "设备请求失败：{e}"),
            Self::IncompatibleFormat { wanted } => write!(f, "不支持的表面格式：{wanted:?}"),
        }
    }
}

impl std::error::Error for RendererError {}

/// 进程共享的 wgpu 上下文：instance / adapter / device / queue 与选定的表面格式。
pub struct GpuContext {
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// 共享管线与配置统一使用的表面格式（sRGB 优先）。
    pub format: wgpu::TextureFormat,
}

impl GpuContext {
    /// 纯无表面构造（离屏与测试）：适配器按 LowPower 且无 surface 约束。
    pub fn headless() -> Result<Arc<Self>, RendererError> {
        Self::headless_with_backends(&[wgpu::Backends::all()])
    }

    /// 指定后端的无表面构造。
    pub fn headless_with_backends(backends: &[wgpu::Backends]) -> Result<Arc<Self>, RendererError> {
        for (i, backend) in backends.iter().enumerate() {
            match Self::headless_with_backend(*backend) {
                Ok(c) => return Ok(c),
                Err(e) if i + 1 < backends.len() => {
                    log::warn!("wgpu 无头后端 {:?} 初始化失败（{e:?}），尝试下一候选", backend);
                }
                Err(e) => return Err(e),
            }
        }
        Err(RendererError::Adapter)
    }

    fn headless_with_backend(backend: wgpu::Backends) -> Result<Arc<Self>, RendererError> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: backend,
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .or_else(|| {
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
        })
        .ok_or(RendererError::Adapter)?;

        let want = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        let (device, queue) = match pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("kanesumi-render-device"),
                required_features: want,
                ..Default::default()
            },
            None,
        )) {
            Ok(dq) => dq,
            Err(_) => pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor::default(),
                None,
            ))
            .map_err(RendererError::Device)?,
        };

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

        Ok(Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            format,
        }))
    }

    /// 由调用方提供 surface 闭包创建（适配特定平台窗口系统）。
    pub fn with_surface_creator<F>(
        backends: &[wgpu::Backends],
        create_surface: F,
    ) -> Result<Arc<Self>, RendererError>
    where
        F: Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
    {
        for (i, backend) in backends.iter().enumerate() {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: *backend,
                ..Default::default()
            });
            let surface = match create_surface(&instance) {
                Ok(s) => s,
                Err(e) if i + 1 < backends.len() => {
                    log::warn!("wgpu surface 创建失败（{e:?}），尝试下一候选");
                    continue;
                }
                Err(e) => return Err(RendererError::Surface(e)),
            };
            let adapter = match pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }))
            .or_else(|| {
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: wgpu::PowerPreference::LowPower,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                }))
            }) {
                Some(a) => a,
                None if i + 1 < backends.len() => continue,
                None => return Err(RendererError::Adapter),
            };

            let want = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
            let (device, queue) = match pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kanesumi-device"),
                    required_features: want,
                    ..Default::default()
                },
                None,
            )) {
                Ok(dq) => dq,
                Err(e) => {
                    log::info!("时间戳查询特性请求失败（{e}），GPU 计时关闭");
                    match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)) {
                        Ok(dq) => dq,
                        Err(_e) if i + 1 < backends.len() => continue,
                        Err(e) => return Err(RendererError::Device(e)),
                    }
                }
            };

            let caps = surface.get_capabilities(&adapter);
            let format = caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(caps.formats[0]);
            drop(surface);

            return Ok(Arc::new(Self {
                instance,
                adapter,
                device,
                queue,
                format,
            }));
        }
        Err(RendererError::Adapter)
    }

    pub fn from_parts(
        instance: wgpu::Instance,
        adapter: wgpu::Adapter,
        device: wgpu::Device,
        queue: wgpu::Queue,
        format: wgpu::TextureFormat,
    ) -> Arc<Self> {
        Arc::new(Self {
            instance,
            adapter,
            device,
            queue,
            format,
        })
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }
}
