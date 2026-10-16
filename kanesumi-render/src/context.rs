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

/// 设备需请求的特性：GPU 计时（时间戳查询）＋ 管线缓存。
///
/// `PIPELINE_CACHE` 只在适配器确实支持时才加入 `required_features` —— 否则 `request_device`
/// 会整体失败、连时间戳一起丢。管线缓存让二次启动复用已编译管线（参任务 k-cold-start-gpu §2）。
/// 注意：仅表面路径启用；离屏 / 金样路径不请求，避免测试往真实 `~/.cache/ether` 写缓存。
fn device_required_features(adapter: &wgpu::Adapter) -> wgpu::Features {
    let mut want =
        wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if adapter.features().contains(wgpu::Features::PIPELINE_CACHE) {
        want |= wgpu::Features::PIPELINE_CACHE;
    }
    want
}

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
        Self::with_surface_creator_noted(backends, create_surface, |_| {})
    }

    /// 同 [`Self::with_surface_creator`]，但在各子步骤后回调 `note(&'static str)`。
    /// 供上层把 `gpu_create`（`gpu_instance`→`gpu_device`）拆成「实例 / 表面 / 适配器 / 设备」
    /// 四段记入拉起时间线。参任务书 k-cold-start-gpu §要做的事 1；回调只收静态阶段名，
    /// 不引入依赖。各段同机实测值见 docs/research/k_cold_start_gpu/ARCDECK_2026-10-16.md §Ⅱ。
    pub fn with_surface_creator_noted<F, N>(
        backends: &[wgpu::Backends],
        create_surface: F,
        note: N,
    ) -> Result<Arc<Self>, RendererError>
    where
        F: Fn(&wgpu::Instance) -> Result<wgpu::Surface<'static>, wgpu::CreateSurfaceError>,
        N: Fn(&'static str),
    {
        for (i, backend) in backends.iter().enumerate() {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: *backend,
                ..Default::default()
            });
            note("gpu_instance_new");
            let surface = match create_surface(&instance) {
                Ok(s) => s,
                Err(e) if i + 1 < backends.len() => {
                    log::warn!("wgpu surface 创建失败（{e:?}），尝试下一候选");
                    continue;
                }
                Err(e) => return Err(RendererError::Surface(e)),
            };
            note("gpu_surface_created");
            // 适配器选择（冷启动关键路径，参任务 §要做的事 1 / 3）：
            // 先按「无表面约束」请求 LowPower 适配器，随后用 `is_surface_supported` 核对能否
            // 呈现到本表面；不通过再退回带表面请求（多 GPU 场景下「无表面」可能选到不能呈现
            // 的卡）。末段 `force_fallback` 与原实现一致，保证软渲染兜底。
            // ⚠ 归因纠正（2026-10-16）：本条原注「带 `compatible_surface` 时 Vulkan WSI 在
            // 枚举期对每个物理设备做表面兼容探测，真机实测每次多付约 0.42 s」是唤醒修复前的
            // 无头 I/O 节拍伪影（`headless-io-wake` `1e569f32`），不是枚举成本；修后同机实测
            // `gpu_adapter` 次启 p50 3.7 ms / 首启 5.4 ms。快路径仍保留 —— `is_surface_supported`
            // 是多 GPU 选卡的核对；本轮未做「带表面请求」对照，故不宣称其省时收益。
            let fast = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
            .filter(|a| a.is_surface_supported(&surface));
            let adapter = match fast
                .or_else(|| {
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: false,
                    }))
                })
                .or_else(|| {
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::LowPower,
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: true,
                    }))
                }) {
                Some(a) => a,
                None if i + 1 < backends.len() => continue,
                None => return Err(RendererError::Adapter),
            };
            note("gpu_adapter_ready");

            let (device, queue) = match pollster::block_on(adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("kanesumi-device"),
                    required_features: device_required_features(&adapter),
                    ..Default::default()
                },
                None,
            )) {
                Ok(dq) => dq,
                Err(e) => {
                    log::info!("设备特性请求失败（{e}），回落无特性设备");
                    match pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None)) {
                        Ok(dq) => dq,
                        Err(_e) if i + 1 < backends.len() => continue,
                        Err(e) => return Err(RendererError::Device(e)),
                    }
                }
            };
            note("gpu_request_device");

            let caps = surface.get_capabilities(&adapter);
            note("gpu_caps_done");
            let format = caps
                .formats
                .iter()
                .find(|f| f.is_srgb())
                .copied()
                .unwrap_or(caps.formats[0]);
            drop(surface);
            note("gpu_surface_dropped");

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
