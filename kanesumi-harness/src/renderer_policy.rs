// renderer_policy.rs — 按表面选渲染器（G1「客户端 GPU 光栅」的决策层）。
//
// 参 Ether docs/GPU_COMPOSITION_PLAN.md §Ⅲ「G1 壳层 GPU 光栅（按 G0 修订）」与
// docs/research/gpu_g0/REPORT.md（G0 实测）。G0 结论：
//   - 小表面 CPU 局部光栅便宜（TopBar 3072×30 ≈ 0.10 ms），wgpu 每帧固定约 1 ms；
//   - 破 16.7 ms 预算的是大浮层（Launcher 覆盖层 3072×1920 ≈ 5.9 Mpx，CPU 光栅 p50 ≈ 19 ms）；
//   - 每份 wgpu 设备 RSS +55~75 MB、首帧晚 1~2 s。
// 故本模块只回答「这个表面值不值得一份 GPU 光栅」，是纯逻辑，便于单测；
// 设备惰性创建、失败回落、kill-switch 门控在 platform 侧执行。

/// 表面种类（选渲染器的输入）。与 `platform` 的 `SurfaceKind` 解耦，保持本模块纯逻辑。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceClass {
    /// xdg-shell 普通窗口 —— 现状即 wgpu 直出。
    XdgWindow,
    /// layer-shell 主表面（TopBar / Dock / 桌面 / Launcher 主表面）。
    LayerMain,
    /// 浮层表面（Launcher 覆盖层 / 控制面板 / 菜单等独立 layer-shell 表面）。
    Floating,
    /// IME 候选窗 popup。
    ImePopup,
}

/// 渲染器种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RendererKind {
    /// CPU 光栅（`CpuRenderer` → dmabuf/SHM）。
    Cpu,
    /// wgpu GPU 光栅（直出 present）。
    Gpu,
}

/// 选 GPU 的最小物理面积（px²）。
///
/// 依据 G0 的 CPU 吞吐 ≈ 310k px/ms（5.9 Mpx / 19 ms）与 wgpu 每帧固定 ≈ 1 ms：
/// CPU 光栅约 1 ms 的面积为 ~310k px²，取 1.0 Mpx²（CPU ≈ 3.2 ms）留出明确收益余量。
/// TopBar（3072×30 = 92k）与 Dock（3072×64 = 197k）均远低于此，保持 CPU。
pub const GPU_MIN_AREA_PX2: u64 = 1_000_000;

/// 选 GPU 的最小「面积 × 预期刷新频率」工作因子（px²·Hz）。
///
/// 一份 wgpu 设备约 +60 MB RSS 且首帧晚 1~2 s，只有「大且会频繁变化」的表面才值得。
/// 5.0 Mpx²·Hz：Launcher 覆盖层（5.9 Mpx，内容变化约 10 Hz）= 59 M 通过；
/// 一个 1 Mpx 却每分钟才变一次的表面（≈ 0.017 Hz）= 17k 不通过。
pub const GPU_MIN_WORK_PX2_HZ: f32 = 5_000_000.0;

/// 表面预期刷新频率（Hz）的缺省估计。数值为量级估计，供策略与日志使用。
pub fn default_expected_hz(class: SurfaceClass) -> f32 {
    match class {
        // 普通窗口本就逐帧呈现（滚动 / 打字 / 动画）。
        SurfaceClass::XdgWindow => 60.0,
        // 主表面：桌面框选 / Launcher 主滚动是高频；TopBar 时钟低频 —— 取上限。
        SurfaceClass::LayerMain => 30.0,
        // 浮层：G3 后动画在合成器侧，内容变化（页滚动 / 悬停）约 10 Hz 量级。
        SurfaceClass::Floating => 10.0,
        // IME 候选窗：每键一次（打字峰值但表面很小）。
        SurfaceClass::ImePopup => 10.0,
    }
}

/// 纯决策函数：给定表面种类、物理面积、预期刷新频率、GPU 是否可用与是否开启一张画布（`KANESUMI_ONE_CANVAS`），返回渲染器种类。
///
/// `gpu_available = false` 表示 kill-switch 命中或 wgpu 初始化已失败 → 一律 CPU。
/// xdg 窗口在可用时一律 GPU（现状行为，不经阈值）。
/// `one_canvas = true`（KANESUMI_ONE_CANVAS=1）且 GPU 可用时，LayerMain / Floating / ImePopup 一律 GPU。
/// `one_canvas = false` 时与旧有按面积 × 刷新频率阈值决策逻辑逐项一致。
pub fn choose_renderer(
    class: SurfaceClass,
    physical_area_px2: u64,
    expected_hz: f32,
    gpu_available: bool,
    one_canvas: bool,
) -> RendererKind {
    if !gpu_available {
        return RendererKind::Cpu;
    }
    if one_canvas {
        return RendererKind::Gpu;
    }
    match class {
        SurfaceClass::XdgWindow => RendererKind::Gpu,
        SurfaceClass::LayerMain | SurfaceClass::Floating | SurfaceClass::ImePopup => {
            let hz = if expected_hz.is_finite() && expected_hz > 0.0 {
                expected_hz
            } else {
                0.0
            };
            let work = physical_area_px2 as f32 * hz;
            if physical_area_px2 >= GPU_MIN_AREA_PX2 && work >= GPU_MIN_WORK_PX2_HZ {
                RendererKind::Gpu
            } else {
                RendererKind::Cpu
            }
        }
    }
}

/// 候选窗接线保留旧的恒 CPU 路径；仅显式开启一张画布后才允许 GPU。
/// 不能直接用面积策略替代旧接线，否则大候选窗会在缺省关闭时新建 GPU。
/// 参 Ether tools/dispatch/tasks/c2-one-canvas.md §设计。
pub(crate) fn choose_ime_popup_renderer(
    physical_area_px2: u64,
    expected_hz: f32,
    gpu_available: bool,
    one_canvas: bool,
) -> RendererKind {
    if !one_canvas {
        return RendererKind::Cpu;
    }
    choose_renderer(
        SurfaceClass::ImePopup,
        physical_area_px2,
        expected_hz,
        gpu_available,
        one_canvas,
    )
}

/// 决策的人类可读理由（日志用；与选择结果一并记一次）。
pub fn decision_reason(
    class: SurfaceClass,
    physical_area_px2: u64,
    expected_hz: f32,
    gpu_available: bool,
    one_canvas: bool,
) -> String {
    if !gpu_available {
        return "GPU 不可用（kill-switch 命中或初始化失败）→ CPU".to_string();
    }
    if one_canvas {
        return "KANESUMI_ONE_CANVAS=1 强制一张画布 → GPU".to_string();
    }
    match class {
        SurfaceClass::XdgWindow => "xdg-shell 窗口默认 GPU 直出".to_string(),
        _ => {
            let work = physical_area_px2 as f32 * expected_hz.max(0.0);
            format!(
                "area={physical_area_px2}px²（阈值 {GPU_MIN_AREA_PX2}）× {expected_hz:.1}Hz = {work:.0}px²·Hz（阈值 {GPU_MIN_WORK_PX2_HZ:.0}）"
            )
        }
    }
}

/// kill-switch 纯判定：`~/.config/ether/gpu-off` 存在，或 `KANESUMI_GPU=0` → 一律 CPU。
///
/// 与真实环境分离以便单测；真实读取见 [`gpu_kill_switch`]。
pub fn kill_switch_active(env_canesumi_gpu: Option<&str>, gpu_off_file: bool) -> bool {
    gpu_off_file || env_canesumi_gpu == Some("0")
}

/// 真实 kill-switch：读 `KANESUMI_GPU` 与 `~/.config/ether/gpu-off`。
pub fn gpu_kill_switch() -> bool {
    let env = std::env::var("KANESUMI_GPU").ok();
    let off = config_dir_flag_exists("gpu-off");
    kill_switch_active(env.as_deref(), off)
}

/// `~/.config/ether/<name>` 是否存在（HOME 缺失时视为不存在）。
fn config_dir_flag_exists(name: &str) -> bool {
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    std::path::Path::new(&home)
        .join(".config/ether")
        .join(name)
        .exists()
}

/// KANESUMI_ONE_CANVAS 开关纯判定：env 为 Some("1") 时激活。
///
/// 与真实环境分离以便单测；真实读取见 [`one_canvas`]。
pub fn one_canvas_active(env_one_canvas: Option<&str>) -> bool {
    env_one_canvas == Some("1")
}

/// 真实读取环境变量 `KANESUMI_ONE_CANVAS`。
pub fn one_canvas() -> bool {
    let env = std::env::var("KANESUMI_ONE_CANVAS").ok();
    one_canvas_active(env.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_canvas_active_判定() {
        assert!(!one_canvas_active(None));
        assert!(!one_canvas_active(Some("0")));
        assert!(!one_canvas_active(Some("2")));
        assert!(!one_canvas_active(Some("")));
        assert!(one_canvas_active(Some("1")));
    }

    #[test]
    fn one_canvas_开启且_gpu_可用时一律_gpu() {
        for class in [
            SurfaceClass::XdgWindow,
            SurfaceClass::LayerMain,
            SurfaceClass::Floating,
            SurfaceClass::ImePopup,
        ] {
            // 即使面积与频率极小，开启一张画布后也选 GPU。
            assert_eq!(
                choose_renderer(class, 100, 1.0, true, true),
                RendererKind::Gpu,
                "{class:?} 在 one_canvas 开启且 GPU 可用时必须 GPU"
            );
        }
    }

    #[test]
    fn one_canvas_开启但_gpu_不可用时一律_cpu() {
        for class in [
            SurfaceClass::XdgWindow,
            SurfaceClass::LayerMain,
            SurfaceClass::Floating,
            SurfaceClass::ImePopup,
        ] {
            assert_eq!(
                choose_renderer(class, 5_898_240, 60.0, false, true),
                RendererKind::Cpu,
                "{class:?} 在 GPU 不可用时即使 one_canvas 开启也必须 CPU"
            );
        }
    }

    #[test]
    fn 候选窗关闭一张画布时保留恒_cpu_接线() {
        for area in [0, 200_000, GPU_MIN_AREA_PX2, 5_898_240, u64::MAX] {
            for gpu_available in [false, true] {
                assert_eq!(
                    choose_ime_popup_renderer(area, 10.0, gpu_available, false),
                    RendererKind::Cpu,
                    "缺省关闭时不得因候选窗面积 {area} 新建 GPU"
                );
            }
        }
    }

    #[test]
    fn 候选窗开启一张画布时仍尊重_gpu_可用性() {
        for area in [0, 200_000, GPU_MIN_AREA_PX2, 5_898_240] {
            assert_eq!(
                choose_ime_popup_renderer(area, 10.0, true, true),
                RendererKind::Gpu
            );
            assert_eq!(
                choose_ime_popup_renderer(area, 10.0, false, true),
                RendererKind::Cpu
            );
        }
    }

    #[test]
    fn 小表面留在_cpu() {
        // TopBar 3072×30 ≈ 92k px²，刷新 2 Hz。
        assert_eq!(
            choose_renderer(SurfaceClass::LayerMain, 92_160, 2.0, true, false),
            RendererKind::Cpu
        );
        // Dock 3072×64 ≈ 197k px²，即使高频也低于面积阈值。
        assert_eq!(
            choose_renderer(SurfaceClass::LayerMain, 196_608, 30.0, true, false),
            RendererKind::Cpu
        );
        // IME 候选窗（小）。
        assert_eq!(
            choose_renderer(SurfaceClass::ImePopup, 200_000, 10.0, true, false),
            RendererKind::Cpu
        );
    }

    #[test]
    fn 大浮层走_gpu() {
        // Launcher 覆盖层 3072×1920 ≈ 5.90 Mpx，10 Hz。
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, 5_898_240, 10.0, true, false),
            RendererKind::Gpu
        );
        // 桌面（大面积、高频框选）。
        assert_eq!(
            choose_renderer(SurfaceClass::LayerMain, 5_898_240, 60.0, true, false),
            RendererKind::Gpu
        );
    }

    #[test]
    fn 大而低频留在_cpu() {
        // 2 Mpx 但每分钟才变一次（≈ 0.0167 Hz）→ 不值一份设备。
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, 2_000_000, 0.0167, true, false),
            RendererKind::Cpu
        );
        // 面积过关 + 频率过关 → GPU。
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, 2_000_000, 30.0, true, false),
            RendererKind::Gpu
        );
    }

    #[test]
    fn 子弹层等小浮层关闭时留_cpu_开启时走_gpu() {
        // 控制面板 / TopBar / Dock 菜单的典型物理尺寸（≤ 1 Mpx）：开关关闭按阈值留 CPU……
        let area = 800 * 1000; // 0.8 Mpx
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, area, 10.0, true, false),
            RendererKind::Cpu,
            "子弹层尺寸的小浮层在开关关闭时必须留 CPU"
        );
        // ……开关开启 → GPU（与浮层同一规则）。
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, area, 10.0, true, true),
            RendererKind::Gpu
        );
    }

    #[test]
    fn gpu_不可用一律_cpu() {
        for class in [
            SurfaceClass::XdgWindow,
            SurfaceClass::LayerMain,
            SurfaceClass::Floating,
            SurfaceClass::ImePopup,
        ] {
            assert_eq!(
                choose_renderer(class, 5_898_240, 60.0, false, false),
                RendererKind::Cpu,
                "{class:?} 在 GPU 不可用时必须 CPU"
            );
        }
    }

    #[test]
    fn kill_switch_判定() {
        assert!(!kill_switch_active(None, false));
        assert!(kill_switch_active(Some("0"), false));
        assert!(kill_switch_active(None, true));
        // 非 "0" 的值不禁用（KANESUMI_GPU=1 或未设）。
        assert!(!kill_switch_active(Some("1"), false));
        assert!(!kill_switch_active(Some(""), false));
    }

    #[test]
    fn 非法频率按零处理() {
        // NaN / 负值 → 视为不动，留在 CPU。
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, 5_898_240, f32::NAN, true, false),
            RendererKind::Cpu
        );
        assert_eq!(
            choose_renderer(SurfaceClass::Floating, 5_898_240, -5.0, true, false),
            RendererKind::Cpu
        );
    }
}
