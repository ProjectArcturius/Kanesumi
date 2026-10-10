// Linux 外壳：Wayland 客户端（sctk）+ wgpu 渲染循环。
//
// 对应 §4.2 三层握手的 Runtime 侧：普通 Wayland 客户端（xdg-shell / layer-shell），
// 动画由 frame callback 推进（参 HANDOVER §1 主循环）。
// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(engine, size)` → 光栅化 Scene。

use std::sync::Arc;
use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Rect, Size};
use smithay_client_toolkit::reexports::{
    calloop::EventLoop, calloop_wayland_source::WaylandSource,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_dmabuf, delegate_keyboard, delegate_layer, delegate_output,
    delegate_pointer, delegate_registry, delegate_seat, delegate_xdg_shell, delegate_xdg_window,
    dmabuf::{DmabufHandler, DmabufState},
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent as SctkKeyEvent, KeyboardHandler, Keysym},
        pointer::{
            AxisScroll, BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent, PointerEventKind,
            PointerHandler,
        },
    },
    shell::{
        WaylandSurface,
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        xdg::XdgShell,
        xdg::window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
    },
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_buffer, wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_shm_pool, wl_surface},
};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{Event as FractionalScaleEvent, WpFractionalScaleV1},
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{ContentHint, ContentPurpose, Event as TextInputEvent, ZwpTextInputV3},
};
use wayland_protocols::wp::viewporter::client::{
    wp_viewport::WpViewport, wp_viewporter::WpViewporter,
};
use wayland_protocols::wp::presentation_time::client::{
    wp_presentation, wp_presentation_feedback,
};
use crate::frame_clock::{FrameClock, InputCoalescer, PacingThrottle};
// input-method-v2 引擎宿主（Ceyboard 作为 IME 引擎连接合成器）。参 CEYBOARD_SPEC §Ⅴ。
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{
        Event as ImGrabEvent, ZwpInputMethodKeyboardGrabV2,
    },
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{Event as ImEvent, ZwpInputMethodV2},
    zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
};
// 虚拟键盘：重放引擎未消费的按键给焦点客户端（fcitx5 同款透传）。
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use crate::app::{
    AnchorKind, App, FloatingLayer, ImeAction, ImeContentHint, ImeContext, InputEvent, Key,
    LayerKind, Modifiers, PendingImeBatch, PointerButton, ScrollInput, ScrollPhase, ScrollSource,
    compute_ime_action,
};
use crate::appmenu::AppMenuHandle;
use crate::context_menu::ContextMenuAction;
use crate::cpu_raster::CpuRenderer;
use crate::render::{GpuContext, GpuContextExt, Renderer};

/// 画布 v1/v2 选择：`KANESUMI_CANVAS=2` 走实例化批渲染画布（CanvasV2，无 MSAA），
/// 其余（未设 / 其他值）保持 v1 `Renderer`（MSAA 4）。真机验证后由调度者翻缺省。
/// 参 Ether docs/CANVAS_PLAN.md §Ⅳ C1、任务 c1-canvas-core。
pub fn canvas_v2_enabled() -> bool {
    std::env::var("KANESUMI_CANVAS").ok().as_deref() == Some("2")
}

/// 表面渲染器枚举：v1 / v2 对外接口对齐，创建处统一经 [`SurfaceRenderer::with_context`]。
/// allow：v1 `Renderer` 自身 1560 字节（顶点缓冲句柄等），v2 已 Box 仍超 clippy 的
/// 变体尺寸差阈值；v1 属存量实现不动，此处维持直存。
#[allow(clippy::large_enum_variant)]
pub enum SurfaceRenderer {
    V1(Renderer),
    /// Box：CanvasV2（图集 / 缓存字段多）远大于 V1，包一层减小枚举尺寸差。
    V2(Box<crate::canvas_v2::CanvasV2>),
}

impl SurfaceRenderer {
    #[allow(clippy::too_many_arguments)]
    pub fn with_context(
        ctx: std::sync::Arc<GpuContext>,
        conn: &Connection,
        wl_surface: &wl_surface::WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
    ) -> Result<Self, crate::render::RendererError> {
        if crate::renderer_policy::one_canvas() || canvas_v2_enabled() {
            crate::canvas_v2::CanvasV2::with_context(
                ctx, conn, wl_surface, width, height, scale, transparent,
            )
            .map(|r| SurfaceRenderer::V2(Box::new(r)))
        } else {
            Renderer::with_context(ctx, conn, wl_surface, width, height, scale, transparent)
                .map(SurfaceRenderer::V1)
        }
    }

    pub fn render(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        scene: &kanesumi_canvas::Scene,
    ) {
        match self {
            SurfaceRenderer::V1(r) => r.render(engine, scene),
            SurfaceRenderer::V2(r) => r.render(engine, scene),
        }
    }

    pub fn render_with_damage(
        &mut self,
        engine: &kanesumi_canvas::text::TextEngine,
        scene: &kanesumi_canvas::Scene,
        damage: Option<kanesumi_core::Rect>,
    ) {
        match self {
            SurfaceRenderer::V1(r) => r.render_with_damage(engine, scene, damage),
            SurfaceRenderer::V2(r) => r.render_with_damage(engine, scene, damage),
        }
    }

    pub fn drain_gpu_samples(&mut self) -> Vec<f32> {
        match self {
            SurfaceRenderer::V1(r) => r.drain_gpu_samples(),
            SurfaceRenderer::V2(r) => r.drain_gpu_samples(),
        }
    }

    pub fn take_acquire_ms(&mut self) -> Option<f32> {
        match self {
            SurfaceRenderer::V1(r) => r.take_acquire_ms(),
            SurfaceRenderer::V2(r) => r.take_acquire_ms(),
        }
    }

    pub fn gpu_timing_supported(&self) -> bool {
        match self {
            SurfaceRenderer::V1(r) => r.gpu_timing_supported(),
            SurfaceRenderer::V2(r) => r.gpu_timing_supported(),
        }
    }

    pub fn physical_size(&self) -> (u32, u32) {
        match self {
            SurfaceRenderer::V1(r) => r.physical_size(),
            SurfaceRenderer::V2(r) => r.physical_size(),
        }
    }

    pub fn msaa_samples(&self) -> u32 {
        match self {
            SurfaceRenderer::V1(r) => r.msaa_samples(),
            SurfaceRenderer::V2(r) => r.msaa_samples(),
        }
    }

    pub fn resize(&mut self, width: f32, height: f32, scale: f32) {
        match self {
            SurfaceRenderer::V1(r) => r.resize(width, height, scale),
            SurfaceRenderer::V2(r) => r.resize(width, height, scale),
        }
    }

    pub fn diagnostics(&self) -> String {
        match self {
            SurfaceRenderer::V1(r) => r.diagnostics(),
            SurfaceRenderer::V2(r) => r.diagnostics(),
        }
    }

    /// 最近一帧绘制调用数（v2 记 `draws=`；v1 不统计 → None）。
    pub fn last_draws(&self) -> Option<u32> {
        match self {
            SurfaceRenderer::V1(_) => None,
            SurfaceRenderer::V2(r) => Some(r.last_draws()),
        }
    }

    /// 最近一帧 C1.5 保留画布统计（v2 记 (inc, dmg_pct, insts)；v1 → None）。
    pub fn last_c15_stats(&self) -> Option<(bool, f32, u32)> {
        match self {
            SurfaceRenderer::V1(_) => None,
            SurfaceRenderer::V2(r) => Some(r.last_c15_stats()),
        }
    }

    /// 目标离屏缓冲 target 是否就绪且内容有效（v2 增量绘制使用；v1 恒 false）。
    pub fn is_target_valid(&self) -> bool {
        match self {
            SurfaceRenderer::V1(_) => false,
            SurfaceRenderer::V2(r) => r.is_target_valid(),
        }
    }
}

use crate::renderer_policy::{
    RendererKind, SurfaceClass, choose_renderer, default_expected_hz, gpu_kill_switch,
};
use crate::role::{EtherRole, SurfaceKind};
use kanesumi_canvas::set_surface_scale;

/// 启动 harness 主循环（Linux）。阻塞运行，不返回。
///
/// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
/// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(size)` → 光栅化 Scene。
/// 诊断文件双写：/tmp + $HOME（LightDM 多会话 /tmp 可能隔离，home 跨 session 共享）。
/// 诊断落盘。
///
/// **主路径必须持久**（`~/.local/state/ether/`）—— Debian 会话里崩溃/黑屏后无法开终端，
/// 只能重启回主系统读盘；只写 tmpfs（`/tmp`）会随重启丢失（参 `AGENTS.md` 铁律）。
/// 另外在 `$XDG_RUNTIME_DIR` 留一份便于会话内即时查看（无持久性要求）。
mod layers;
mod popups;

fn write_diag(name: &str, content: &str) {
    if let Ok(home) = std::env::var("HOME") {
        let dir = std::path::Path::new(&home).join(".local/state/ether");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join(name), content);
    }
    let session_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let _ = std::fs::write(std::path::Path::new(&session_dir).join(name), content);
}

/// 把字节下标向下夹到最近的 UTF-8 字符边界。
///
/// IME 协议里的光标/删除偏移来自客户端，可能落在多字节字符中间；直接切片会 panic
/// （2026-09-22 审计 P0-4）。`str::floor_char_boundary` 尚未稳定，故自带实现。
fn floor_char_boundary(text: &str, index: usize) -> usize {
    if index >= text.len() {
        return text.len();
    }
    let mut i = index;
    while i > 0 && !text.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// 在错误边界内调用 App 回调：panic → 记日志 + 落盘 + 返回 `None`，**绝不杀进程**。
///
/// 2026-09-22 审计 P0-4：`update`/`render`/`handle_input` 之外的回调（`ime_engine_*`、
/// `focus_changed`、`context_menu` …）原本裸调，App 一 panic 就直达顶层 `exit(2)`，
/// 表现为「TopBar/候选窗整个进程消失」。凡是每帧或每键都会走到的 App 回调都必须过这里。
fn guard<T>(what: &str, f: impl FnOnce() -> T) -> Option<T> {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(v) => Some(v),
        Err(_) => {
            log::error!("App::{what} panic，已隔离（本帧跳过）");
            write_diag(
                "ether-app-panic.log",
                &format!("App::{what} panic —— 已隔离，进程存活\n"),
            );
            None
        }
    }
}

impl Shell {
    /// 读取系统主题（Chorus）并推给 App。
    ///
    /// 启动时一次；此后由 [`maybe_reload_system_theme`](Self::maybe_reload_system_theme)
    /// 在检测到 `theme.toml` 变更时再次调用。
    fn apply_system_theme(&mut self) {
        self.system_theme = crate::system_theme::load();
        self.theme_fingerprint = crate::system_theme::fingerprint();
        let theme = self.system_theme;
        guard("set_theme", || self.app.set_theme(theme));
        self.dirty = true;
    }

    /// 读取交互数值（`~/.config/ether/input.toml`，正典 §Ⅰ.2）并应用到双击检测器。
    ///
    /// 启动时一次；此后由 [`maybe_reload_system_theme`](Self::maybe_reload_system_theme)
    /// 在同一节流点检测变更。滚轮步长在事件处理时实时读 `self.interaction`。
    fn apply_input_settings(&mut self) {
        self.interaction = crate::input_config::load();
        self.interaction_fingerprint = crate::input_config::fingerprint();
        self.click_tracker.set_settings(self.interaction);
    }

    /// 节流检测 `theme.toml` / `input.toml` 变更（约每 0.5s 一次 stat），变化则重推。
    /// 用挂钟节流（而非帧计数）：空闲唤醒变稀疏后仍保持稳定的检测间隔。
    fn maybe_reload_system_theme(&mut self) {
        let now = Instant::now();
        if now < self.next_theme_check {
            return;
        }
        self.next_theme_check = now + crate::idle::THEME_POLL;
        if crate::system_theme::fingerprint() != self.theme_fingerprint {
            self.apply_system_theme();
            log::info!("系统主题已重载：accent/scheme 变更");
        }
        if crate::input_config::fingerprint() != self.interaction_fingerprint {
            self.apply_input_settings();
            log::info!("交互数值已重载：input.toml 变更");
        }
    }
}

pub fn run(app: &mut dyn App) -> ! {
    // `run` 永不返回（`-> !`）：`&mut dyn App` 借用可安全提升为 'static。
    let app: &'static mut dyn App = unsafe { std::mem::transmute(app) };
    // 会话内无日志 UI：panic / Err 都写文件供排查（Ether 下客户端启动失败定位）。
    // 用裸指针穿透闭包生命周期（app 已在 run 入口 unsafe 提升为 'static）。
    let app_ptr = app as *mut dyn App;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        move || -> Result<(), String> {
            let app: &'static mut dyn App = unsafe { &mut *app_ptr };
            run_inner(app)
        },
    ));
    match result {
        Ok(Ok(())) => std::process::exit(0),
        Ok(Err(e)) => {
            eprintln!("kanesumi-harness 异常退出: {e}");
            write_diag("ether-harness-crash.log", &format!("异常退出: {e}\n"));
            std::process::exit(1);
        }
        Err(panic) => {
            let msg = panic
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "未知 panic".into());
            eprintln!("kanesumi-harness panic: {msg}");
            write_diag(
                "ether-harness-crash.log",
                &format!("panic: {msg}\nbacktrace: 见 stderr\n"),
            );
            std::process::exit(2);
        }
    }
}

/// 主逻辑。错误以 String 上报（调用方 exit）。
fn run_inner(app: &'static mut dyn App) -> Result<(), String> {
    // dmabuf gbm 探测子进程入口（由 crate::dmabuf::probe_gbm_crash_safe 拉起）：
    // 只做一次 gbm 打开即退出 —— 段错误/失败 = 该驱动上直通不可用 → 父进程回退 SHM。
    // ⚠ 必须在任何 Wayland/字体/日志初始化之前：探测子进程不留副作用。
    // 应用 `main` **第一行**还应调 `kanesumi_harness::dmabuf_probe_entry()` 把入口再前移
    // （避免子进程重执行时先跑应用前半段，把段错误锅扣到 gbm 头上）。
    crate::dmabuf::dmabuf_probe_entry();
    // 拉起时间线：应用未在 `main` 调 `start` 时以此兜底（所有 kanesumi 应用自动获得）。
    // 参 DECISIONS_2026-10-05 §104、crate::timeline。
    crate::timeline::ensure_started(app.config().app_id);
    env_logger::init();

    let conn = Connection::connect_to_env()
        .map_err(|e| format!("Wayland 连接失败（确认 WAYLAND_DISPLAY）：{e}"))?;
    let (globals, event_queue) =
        registry_queue_init(&conn).map_err(|e| format!("registry_queue_init 失败：{e}"))?;
    let qh = event_queue.handle();

    let mut event_loop: EventLoop<Shell> =
        EventLoop::try_new().map_err(|e| format!("calloop EventLoop 初始化失败：{e}"))?;
    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("WaylandSource 插入失败：{e}"))?;

    // 解码就绪事件唤醒（F3 第 6 条）：calloop ping 桥接工作线程完成信号，唤醒后 poll 并通知元素树。
    let (ping, ping_source) =
        smithay_client_toolkit::reexports::calloop::ping::make_ping()
            .map_err(|e| format!("calloop ping source 创建失败：{e}"))?;
    event_loop
        .handle()
        .insert_source(ping_source, |(), _, shell: &mut Shell| {
            let ready = kanesumi_canvas::decode::poll_ready();
            if !ready.is_empty() {
                shell.app.on_decode_ready(&ready);
                shell.dirty = true;
            }
        })
        .map_err(|e| format!("ping_source 插入失败：{e}"))?;

    let ping_clone = ping.clone();
    kanesumi_canvas::decode::set_ready_hook(Box::new(move || {
        ping_clone.ping();
    }));
    struct ReadyHookGuard;
    impl Drop for ReadyHookGuard {
        fn drop(&mut self) {
            kanesumi_canvas::decode::clear_ready_hook();
        }
    }
    let _hook_guard = ReadyHookGuard;

    let role = EtherRole::from_env();

    // 字体：App 指定优先，否则环境变量 / 系统字体（SD §IX 禁止静默回退）。
    // 字体栈含字重字面（Light / Medium / Bold，T7）；TTC 集合选 SC 字面（N-40）。
    let font_source = app
        .font_path()
        .map(|p| kanesumi_canvas::text::FontSource {
            path: p,
            collection_tag: None,
        })
        .or_else(find_font_source)
        .ok_or_else(|| "未找到字体：设 KANESUMI_TEST_FONT 或提供 App::font_path()".to_string())?;
    let font_label = font_source.path.clone();
    // 字体栈整文件读入 + 解码：放后台线程，与下方 GPU 上下文初始化并行 —— 两者都是
    // 启动期大头，串行执行会把两段相加（参 DECISIONS_2026-10-05 §104 拉起时间线）。
    let font_extra = extra_sources(&font_source.path);
    let font_thread =
        std::thread::spawn(move || TextEngine::load_stack(&font_source, &font_extra));

    // xdg-shell 窗口恒走 wgpu 直出（`Renderer`）→ 在等字体的同时预建共享 GPU 上下文
    // （instance / adapter / device）并挂在 Shell 上，首个 configure 免再等。
    // layer 角色可能按面积/刷新率选 CPU 光栅，保持惰性（不预载无谓的 Vulkan 设备）。
    // `KANESUMI_NO_GPU_PRELOAD=1` 关预载（A/B 实测与排障用；不影响功能）。
    let preload_enabled = role.surface_kind() == SurfaceKind::XdgShell
        && std::env::var_os("KANESUMI_NO_GPU_PRELOAD").is_none();
    let preloaded_gpu = if preload_enabled {
        match CompositorState::bind(&globals, &qh) {
            Ok(comp) => {
                // 临时表面仅用于挑适配器 / 选格式，与 `GpuContext::new` 内部同款；
                // 真实表面由 `Renderer::with_context` 另行创建。
                let tmp = comp.create_surface(&qh);
                match crate::render::GpuContext::new(
                    &conn,
                    &tmp,
                    &[wgpu::Backends::VULKAN, wgpu::Backends::GL],
                ) {
                    Ok(ctx) => {
                        log::info!("GPU 上下文预载完成（与字体加载并行）");
                        Some(ctx)
                    }
                    Err(e) => {
                        log::warn!("GPU 上下文预载失败（{e:?}），首个 configure 时重试");
                        None
                    }
                }
            }
            Err(e) => {
                log::warn!("GPU 预载跳过（wl_compositor 绑定失败）：{e}");
                None
            }
        }
    } else {
        None
    };

    let engine = font_thread
        .join()
        .map_err(|_| "字体加载线程 panic".to_string())?
        .map_err(|e| format!("加载字体失败 {}：{e}", font_label.display()))?;
    // 字体栈整文件读入 + 解码完成（与 GPU 预载并行，故耗时不再是首帧关键路径）。
    crate::timeline::note("fonts_loaded");
    // §112:800 无可变字体时静态回落 Bold 700,落一行持久字体诊断(零交互,重启不丢)。
    // (fw1;engine 由并行线程 join 取得,诊断在其后判定。
    if !engine.extra_bold_is_variable() {
        write_diag(
            "ether-fonts.log",
            "800 回落 Bold(无可变字体;参 docs/DECISIONS_2026-10-05.md §112)
",
        );
    }

    // 锁屏角色：ext-session-lock-v1 与 xdg/layer 单主表面模型不同 —— 交 `session_lock`
    // 外壳（每输出一个锁屏表面，锁确认前不画内容）。参 docs/LOCKSCREEN_DESIGN.md §Ⅱ/§Ⅵ。
    // `session_lock::run` 不返回（退出由锁协议事件驱动）。
    if role.surface_kind() == SurfaceKind::SessionLock {
        crate::session_lock::run(app, engine);
    }

    let mut shell = Shell::new(app, engine, &conn, &globals, &qh, role)?;
    // 预载的 GPU 上下文交给外壳（None → 首个 configure 照旧惰性创建）。
    shell.gpu = preloaded_gpu;
    // 表面 / 角色 / 协议绑定完成，随即等首个 configure。
    crate::timeline::note("shell_ready");
    // 系统主题：读 Chorus 的 theme.toml 并推给 App（accent / scheme）。
    // 这是「用户在 Chorus 改 accent、应用却仍是写死橙色」那条断链的接回点。
    shell.apply_system_theme();
    // 交互数值：读 `~/.config/ether/input.toml`（正典 §Ⅰ.2；缺失 / 非法 → 默认）。
    shell.apply_input_settings();
    // 引擎宿主兜底：主循环内幂等绑定（每帧，seat 异步 announce 后自动创建）。
    // 绕过 new_capability 竞态——ceyboard 连接时 seat keyboard 能力可能已就绪，
    // 能力事件不触发 → grab 未建立 → 合成器转发的键收不到。

    // 主循环（TOPBAR_RENDER_REFACTOR §4.6 按需提交 + 唤醒重构）：
    // - 渲染完全由 `dirty` 驱动（输入 / 定时器 / 动画 / 尺寸变化显式置位，I-3）；
    // - frame 回调仅作 vsync 提示（到达 → 置 dirty），绝不驱动渲染（I-2）；
    // - dispatch timeout：脏时 16ms（动画兜底），空闲 100ms（定时器推进节流）。
    loop {
        if !shell.running {
            break;
        }
        // 空闲唤醒：仅在确有待渲染内容（脏 / 浮层脏 / 动画推进中）时保留 16ms 帧兜底
        // （I-2 不冻结）；否则阻塞到「最近定时器」与「主题检测节流点」较早者，
        // 两者皆无则交给 Wayland 事件唤醒。参 crate::idle。
        let now = Instant::now();
        let main_anim = shell.app.needs_redraw();
        let (can_render_main, _) = shell.main_throttle.can_render(now, main_anim);
        let can_render_any_floating = shell.floating.iter().enumerate().any(|(i, _)| {
            let anim = shell.app.floating_needs_redraw(i);
            let (can, _) = shell.floating_throttle.get_mut(i).map(|t| t.can_render(now, anim)).unwrap_or((true, false));
            can && (shell.floating_dirty[i] || anim)
        });
        let busy = (shell.dirty || main_anim) && can_render_main || can_render_any_floating;
        let theme_after = shell
            .next_theme_check
            .saturating_duration_since(Instant::now());
        let timeout = crate::idle::next_wake(busy, shell.app.next_wake_hint(), Some(theme_after));
        event_loop
            .dispatch(timeout, &mut shell)
            .map_err(|e| format!("事件循环 dispatch 失败：{e}"))?;
        crate::timeline::note_once("dispatch_done");
        // 引擎宿主幂等绑定（input_method.is_none 才建，seat 就绪后即生效）。
        shell.ensure_ime_engine(&qh);
        crate::timeline::note_once("ime_ready");
        // 推进步：update / 定时器 / 菜单命令 / IME / 尺寸同步（与渲染解耦，I-4）。
        shell.step(&qh);
        // 掉帧计数：渲染前声明各表面的动画状态（边沿开始 / 结束）。参 perf.rs。
        shell.sync_pacing();
        // 脏 → 渲染 + commit（I-1：CPU 缓冲恒就绪，无条件成功）。
        if shell.dirty {
            shell.render_and_commit(&qh);
        }
        for i in 0..shell.floating.len() {
            if shell.floating_dirty[i] {
                shell.render_floating_frame(i, &qh);
            }
        }
        shell.sync_popups(&qh);
        shell.render_popups(&qh);
        // 渲染期元素树产出的图层命令（G3-c：树图层的建层 / 内容在 frame 内才知道）同帧送出，不拖到下一帧。
        // 内容就绪门控的就绪汇报在 `TreeHost::render_into` 内完成（紧随本主循环的提交点）；
        // 本文件为 Linux/Wayland 专用 —— Windows 未编译，待 Arch 侧无头会话验证（fp5 验证第 3 条）。
        shell.process_layer_commands(&qh);
        // 帧耗时自记录：每 10s 且有新帧时追加一行持久日志（默认开启，零交互）。
        shell.maybe_flush_perf();
    }
    shell.flush_perf(true);
    Ok(())
}

/// TTC 集合内选 SC 字面的标签（Noto Sans CJK 每档字重含多语言字面；与思源黑体同源，裁定 N-40）。
const SC_TAG: Option<&str> = Some("SC");

/// 思源 SC OTF（在场优先）→ 系统 Noto CJK TTC（SC 字面）的某字重候选链。
fn weight_candidates(weight: &str) -> Vec<kanesumi_canvas::text::FontSource> {
    [
        (
            format!("/usr/local/share/fonts/s/SourceHanSansSC-{weight}.otf"),
            None,
        ),
        (
            format!("/usr/share/fonts/noto-cjk/NotoSansCJK-{weight}.ttc"),
            SC_TAG,
        ),
        (
            format!("/usr/share/fonts/opentype/noto/NotoSansCJK-{weight}.ttc"),
            SC_TAG,
        ),
    ]
    .into_iter()
    .map(|(path, collection_tag)| kanesumi_canvas::text::FontSource {
        path: std::path::PathBuf::from(path),
        collection_tag,
    })
    .collect()
}

/// 查找字体来源：KANESUMI_TEST_FONT → Ether 正体（思源黑体 / Noto CJK SC）→ CJK → 常见拉丁。
/// 中文/日文/韩文须 CJK 字体（DejaVu/Liberation 无 CJK 字形，会渲染为方框）。
pub fn find_font_source() -> Option<kanesumi_canvas::text::FontSource> {
    use kanesumi_canvas::text::FontSource;
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(FontSource { path: p, collection_tag: None });
        }
    }
    regular_candidates().into_iter().find(|s| s.path.exists())
}

fn regular_candidates() -> Vec<kanesumi_canvas::text::FontSource> {
    [
        // §112 可变字体优先：带 wght 轴，按请求字重（300 / 400 / 800）实例化；
        // 在场时静态字重字面不再加载（见 extra_sources）。
        ("/usr/local/share/fonts/s/NotoSansSC-VF.ttf", None),
        ("/usr/local/share/fonts/s/NotoSansCJK-VF.otf.ttc", SC_TAG),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-VF.otf.ttc", SC_TAG),
        ("/usr/share/fonts/noto-cjk/NotoSansCJK-VF.otf.ttc", SC_TAG),
        // Ether 正体字体：思源黑体 SC（合成器同款，SD §IX 唯一字体）。
        ("/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf", None),
        // 系统 Noto CJK（与思源黑体同源；TTC 集合须选 SC 字面，否则静默落到 JP）。
        ("/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc", SC_TAG),
        ("/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc", SC_TAG),
        // 旧部署遗留的 TC Regular（繁体字形，仅保运行）。
        ("/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf", None),
        ("/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf", None),
        // 回退：拉丁（无中文，仅保运行）。
        (
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            None,
        ),
        ("/usr/share/fonts/TTF/DejaVuSans.ttf", None),
        (
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
            None,
        ),
    ]
    .into_iter()
    .map(|(path, collection_tag)| kanesumi_canvas::text::FontSource {
        path: std::path::PathBuf::from(path),
        collection_tag,
    })
    .collect()
}

/// 主字面之外的附加字面：Light / Medium / Bold（T7 字重）+ 脚本回退（阿拉伯 / 希伯来 / 符号）。
/// 缺整条候选链的字重记 warn（零交互诊断；该字重渲染时自动回落 Regular）。
/// 主字体为可变字体（文件名含 `-VF`）时跳过静态字重字面 —— 轴实例已覆盖全档，
/// 再加载数份 20 MB 的 TTC 只增常驻内存（参 Kanesumi R1）。
fn extra_sources(primary: &std::path::Path) -> Vec<kanesumi_canvas::text::FontSource> {
    use kanesumi_canvas::text::FontSource;
    let primary_canonical = primary.canonicalize().unwrap_or_else(|_| primary.to_path_buf());
    let mut out = Vec::new();
    let primary_is_vf = primary
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.contains("-VF"));
    if primary_is_vf {
        log::info!("主字体为可变字体（{}），跳过静态字重字面加载", primary.display());
    } else {
        for weight in ["Light", "Medium", "Bold"] {
            let Some(src) = weight_candidates(weight).into_iter().find(|s| s.path.exists())
            else {
                log::warn!("字重 {weight} 字面缺失（思源 SC / Noto CJK 均不在场），该字重将渲染为 Regular");
                continue;
            };
            out.push(src);
        }
    }
    for path in [
        "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansHebrew-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/TTF/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ] {
        let path = std::path::PathBuf::from(path);
        if path.exists() {
            out.push(FontSource { path, collection_tag: None });
        }
    }
    out.retain(|s| {
        s.path.canonicalize().unwrap_or_else(|_| s.path.clone()) != primary_canonical
    });
    out
}

/// 兼容包装：仅要主字面路径的调用方（ceyboard 等）。
pub fn find_font() -> Option<std::path::PathBuf> {
    find_font_source().map(|s| s.path)
}

/// 外壳状态：sctk 协议状态 + App + wgpu 渲染器。
// 注：以下字段保留以维持 Wayland 协议对象存活（Drop 即销毁表面/全局）：
// compositor_state / layer_shell / xdg_shell / window / layer_surface / role。
#[allow(dead_code)]
/// 单个 layer-shell 表面的共享状态：App、引擎、表面、输入、IME、SHM/dmabuf 输出。
/// 由 platform::run 创建；`Shell` 对 dmabuf 子模块（QueueHandle 类型）可见。
pub(crate) struct Shell {
    app: &'static mut dyn App,
    engine: TextEngine,
    role: EtherRole,

    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    seat_state: SeatState,
    layer_shell: Option<LayerShell>,
    xdg_shell: Option<XdgShell>,

    surface: wl_surface::WlSurface,
    window: Option<Window>,
    layer_surface: Option<LayerSurface>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard: Option<wl_keyboard::WlKeyboard>,

    /// xdg-shell 直出渲染器（present 路径）。layer-shell 角色为 None（走 CPU）。
    renderer: Option<SurfaceRenderer>,
    /// layer-shell CPU 光栅化器（Scene → SHM 像素，零 GPU 同步点）。
    /// 参 TOPBAR_RENDER_REFACTOR §4.1 / cpu_raster.rs。
    cpu: Option<CpuRenderer>,

    // ── GPU 上下文（G1：进程内共享一份 wgpu device/queue）─────
    /// 进程共享 wgpu 上下文（主表面 / 各浮层共用）。惰性创建；`gpu_failed` 后不再尝试。
    gpu: Option<Arc<GpuContext>>,
    /// wgpu 初始化是否已永久失败（不再重试，一律回落 CPU）。
    gpu_failed: bool,
    /// kill-switch 快照（`~/.config/ether/gpu-off` 或 `KANESUMI_GPU=0`）→ 一律 CPU。
    gpu_kill: bool,

    /// 主表面脏标记（I-3：输入 / 定时器 / 动画 / 尺寸变化显式置位，commit 后清除）。
    /// frame 回调仅作 vsync 提示（置位 dirty），不驱动渲染（I-2）。
    dirty: bool,
    /// 各浮层表面脏标记（与 `floating` 等长）。
    floating_dirty: Vec<bool>,
    /// 浮层下一帧须整幅（新建 / 重新显示）。参 `render_floating_frame` 局部光栅。
    floating_full: Vec<bool>,
    /// 浮层上一帧光栅的物理尺寸（变了 → 整幅）。
    floating_raster_size: Vec<Option<(u32, u32)>>,
    /// 上一迭代各浮层可见性（翻转检测 → 置脏）。
    floating_visible_cache: Vec<bool>,

    /// 主表面光栅化缩放。布局仍使用逻辑像素；支持 1.25 / 1.5 等分数比例。
    scale: f32,
    _fractional_scale_manager: Option<WpFractionalScaleManagerV1>,
    _viewporter: Option<WpViewporter>,
    _fractional_scale: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
    /// 逻辑尺寸（configure 后有效）。
    width: f32,
    height: f32,
    configured: bool,
    running: bool,
    /// 已请求主表面 frame callback 但尚未到达（去重，避免一帧多请求）。
    frame_pending: bool,
    /// wp_presentation 全局对象（合成器提供 → Some；F1/F3 时钟采样）。
    presentation: Option<wp_presentation::WpPresentation>,
    /// 客户端呈现帧时钟（F3：按 wp_presentation 回执 / frame 回调预测下一次呈现）。
    frame_clock: FrameClock,
    /// 主表面出帧节流状态机（F3：一个回调一帧 / 100ms 超时防冻结）。
    main_throttle: PacingThrottle,
    /// 各浮层表面出帧节流状态机（与 `floating` 等长）。
    floating_throttle: Vec<PacingThrottle>,
    /// 输入按帧合并队列（移动 / 滚动合并，出帧前一次性交付）。
    coalescer: InputCoalescer,
    pointer_pos: (f32, f32),
    /// 当前按下的指针键数（S1 输入门控：按键期间 Move 恒置脏，拖拽/滑动逐帧回馈）。
    pointer_buttons: u32,
    /// S4 局部损坏矩形（本帧 CPU 光栅只重绘该区，其余像素保留上帧；None = 全量）。
    /// 由菜单悬停 / App::damage_hint 累积，`render_and_commit` 消费后清除。
    pending_damage: Option<Rect>,
    /// 双击检测器（Press 判定 → 追加 `InputEvent::DoubleClick`）。
    click_tracker: crate::app::ClickTracker,
    /// 右键菜单状态机（harness 接管右键路由，参 CONTEXT_MENU_SPEC §Ⅵ.2）。
    /// 主表面右键 → `App::context_menu` → 菜单开在主表面内；点选 → `App::on_context_command`。
    ctx_menu: crate::context_menu::ContextMenuState,

    // ── IME（zwp_text_input_v3，参 IME_WIRING_PLAN 阶段 D） ─────
    /// text-input manager 全局（合成器未提供 → None，App 降级走裸 KeyPressed）。
    text_input_manager: Option<ZwpTextInputManagerV3>,
    /// per-seat text-input 对象。
    text_input: Option<ZwpTextInputV3>,
    /// 上次发送的 enable 状态（reconcile 幂等判定）。
    ime_enabled: bool,
    /// wl_keyboard / text-input enter 标记：表面是否持键盘焦点。
    ime_focus_surface: bool,
    /// 单调 commit 计数 —— 每次实际 commit() 后 +1，done serial 与之匹配才生效。
    commit_serial: u32,
    /// done 到达前累积的事件批。
    pending_ime: PendingImeBatch,
    /// 上次发送的 IME 上下文缓存（无变化不重发，避免每帧灌上下文 + commit 抖动）。
    ime_context_cache: Option<ImeContext>,

    // ── IME 引擎宿主（zwp_input_method_v2，Ceyboard 作为引擎）。参 CEYBOARD_SPEC §Ⅴ ─────
    /// input-method manager 全局（合成器提供 + App 声明引擎宿主 → Some）。
    input_method_manager: Option<ZwpInputMethodManagerV2>,
    /// per-seat input-method 对象（引擎侧）。
    input_method: Option<ZwpInputMethodV2>,
    /// grab_keyboard 返回的键盘 grab（接收合成器转发的按键）。
    im_keyboard_grab: Option<ZwpInputMethodKeyboardGrabV2>,
    /// 引擎是否激活（activate 后 true；此时按键进引擎）。
    im_active: bool,
    /// done 事件计数（serial = 已收到的 done 数；commit 时回传）。
    im_done_serial: u32,
    /// xkbcommon keymap 状态（keymap 事件建立，key 事件语义化）。
    im_xkb: Option<ImXkb>,
    /// 上次发送的 preedit（幂等：无变化不重发 set_preedit_string）。
    im_preedit_cache: Option<String>,
    /// 引擎键盘的修饰键状态（grab modifiers 事件维护，注入 ime_engine_key）。
    im_modifiers: Modifiers,
    /// 焦点文本字段周边文本（`ImEvent::SurroundingText` 缓存，退格字符边界用）。
    /// `(text, cursor, anchor)` 字节偏移。
    im_surrounding: Option<(String, u32, u32)>,
    /// 候选窗 popup surface（引擎激活时创建，deactivate 释放）。
    im_popup: Option<ImPopupSurface>,
    /// 候选窗内容脏标记（key 事件置位；refresh 消费后清除）。避免每帧 SHM 提交闪烁。
    im_popup_dirty: bool,
    /// 虚拟键盘 manager 全局（重放未消费按键给焦点客户端）。
    vk_manager: Option<ZwpVirtualKeyboardManagerV1>,
    /// per-seat 虚拟键盘对象。
    virtual_keyboard: Option<ZwpVirtualKeyboardV1>,
    /// 重放按键时间戳（单调递增）。
    im_key_time: u32,

    /// 当前修饰键状态（`update_modifiers` 维护，注入每个输入事件）。
    modifiers: Modifiers,
    /// Wayland 连接（延迟渲染器初始化用：首 configure 后才创建 wgpu surface）。
    conn: Connection,
    /// App 请求但尚未被 configure 确认的动态高度（layer-shell 展开用）。
    pending_height: Option<f32>,
    /// 浮层表面（独立 layer-shell，透明底控件浮层）。
    floating: Vec<FloatingSurface>,
    /// 全局应用菜单句柄（运行时更新：勾选 / 结构）。None = 未启用。
    appmenu: Option<AppMenuHandle>,
    /// 全局菜单命令接收端（服务线程推送点击 id）。
    appmenu_rx: Option<std::sync::mpsc::Receiver<i32>>,
    /// 首帧诊断日志是否已输出。
    diag_logged: bool,
    /// 渲染帧计数（诊断：验证静止唤醒是否重启渲染）。
    frame_count: u64,

    // ── 帧耗时自记录（默认开启；每 10s 追加一行持久日志）参 crate::perf ─────
    /// 主表面三段耗时（render_into / raster / commit）环形样本。
    perf_main: crate::perf::SurfacePerf,
    /// 各浮层表面三段耗时（与 `floating` 等长）。
    perf_floating: Vec<crate::perf::SurfacePerf>,
    /// 候选窗 popup 耗时样本（C2：perf 日志补齐）。
    perf_im_popup: crate::perf::SurfacePerf,
    /// 下次写 perf 日志的时刻（挂钟节流）。
    perf_flush_at: Instant,
    /// 进程名（日志行首）。
    perf_proc: String,
    /// 进程启动自证行（msaa / gpu 计时）是否已写 —— perf 日志首行只写一次。
    perf_header_done: bool,

    // ── 系统主题（Chorus）─────
    /// 当前生效的系统主题。外壳拥有并在启动 / 配置变更时推给 App（`App::set_theme`）。
    system_theme: MetroTheme,
    /// `theme.toml` 的 mtime 指纹 —— 变化才重载，避免每帧读盘。
    theme_fingerprint: Option<std::time::SystemTime>,
    /// 下次主题变更检测的时刻（节流 stat 调用；空闲唤醒的候选点之一）。
    next_theme_check: Instant,

    // ── 交互数值（`~/.config/ether/input.toml`，正典 §Ⅰ.2）─────
    /// 当前生效的交互设置（双击时长 / 滚轮行数 / 提示延迟）。
    interaction: kanesumi_core::InteractionSettings,
    /// `input.toml` 的 mtime 指纹 —— 变化才重载（与主题同一节流点）。
    interaction_fingerprint: Option<std::time::SystemTime>,

    // ── 输出缓冲（SHM 回退 + dmabuf 直通；主表面 / 各浮层 / IME 候选窗各一份）─────
    /// wl_shm 全局（dmabuf 不可用时的回退；合成器未提供 → None）。
    shm: Option<wl_shm::WlShm>,
    /// 主表面输出（layer-shell CPU 角色 / xdg 角色闲置）。
    main_out: SurfaceOutput,
    /// 各浮层表面的输出缓冲（与 `floating` 等长）。
    floating_out: Vec<SurfaceOutput>,
    /// 上次 update 的时刻（合成器时钟：dt 限幅防卡顿后跳变，§4.1 不变量 2）。
    last_update: Instant,
    /// 主表面 Scene 复用缓冲（egui PaintList）：每帧 `render_into` 就地清空重建，
    /// 复用 Vec 容量，避免每帧 `Scene::default()` + push 重分配。
    scene_buf: Scene,

    // ── dmabuf 直通（所有 CPU 光栅化表面默认走直通，SHM 为回退）─────
    /// 客户端 dmabuf 全局（合成器提供 → Some）。参 linux-dmabuf 协议。
    dmabuf: DmabufState,
    /// dmabuf 直通是否可用（合成器提供 global + 未 `ETHER_DMABUF=0` + gbm 探测通过）。
    /// 默认**开**；`ETHER_DMABUF=0` 可强制回退 SHM（保底）。参 LINUX_DMABUF_PLAN §4 M6。
    dmabuf_allowed: bool,
    /// 合成器 `zwp_linux_dmabuf_feedback_v1` 快照（主设备 + 格式表）—— 浮层/popup 后建时补灌。
    dmabuf_feedback: Option<(libc::dev_t, Vec<(u32, u64)>)>,
    /// 进程级共享 gbm device 句柄（所有表面 clone 同一个 → 只开一次 DRM fd/Mesa screen）。
    dmabuf_device: crate::dmabuf::DmabufDevice,
    /// feedback 协议对象（保活：drop 即销毁对象，后续 done/格式事件不再到达）。
    #[allow(dead_code)]
    dmabuf_feedback_obj: Option<
        wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
    >,

    // ── 子弹层（xdg_popup）。参 platform/popups.rs ─────
    /// 已开出的子弹层（自底向上）。
    popups: Vec<popups::HostedPopup>,
    /// 合成图层（G3-b）：子表面 + ether_visual_v1。参 platform/layers.rs。
    comp: layers::Composition,
    /// App 接受了子弹层（enable_popups 返回 true）。
    popups_enabled: bool,
    /// 上次告知 App 的放置区（变化才重发）。
    popup_bounds_sent: Option<Rect>,
    /// 当前 seat（xdg_popup.grab 需要）。
    seat: Option<wl_seat::WlSeat>,
    /// 最近一次输入事件 serial（指针按下 / 按键）—— grab 必须带触发它的输入 serial。
    last_input_serial: Option<u32>,
}

/// 单个表面的输出缓冲：SHM 双缓冲（回退）+ dmabuf 双缓冲（直通）+ 模式决策。
/// 主表面 / 各浮层 / IME 候选窗各一份 —— 直通覆盖**所有 CPU 光栅化表面**（2026-09-18）。
/// 提交优先 dmabuf；不可用/失败则回退 SHM，**绝不丢帧**。
struct SurfaceOutput {
    shm: ShmBuffers,
    dmabuf: crate::dmabuf::DmabufBuffers,
    /// 是否允许尝试 dmabuf（进程级门控：合成器 global + `ETHER_DMABUF!=0` + gbm 探测）。
    allow: bool,
}

impl SurfaceOutput {
    /// `device` 为**进程共享**句柄（所有表面共用一个 gbm device，避免每表面一次 Mesa screen）。
    /// 探测证明可用的组合随共享 device 传递，后建的浮层/popup 表面自动继承。
    fn new(allow: bool, device: crate::dmabuf::DmabufDevice) -> Self {
        let recipe = device.recipe();
        let mut dmabuf = crate::dmabuf::DmabufBuffers::default();
        dmabuf.set_device(device);
        dmabuf.set_probe_recipe(recipe);
        Self {
            shm: ShmBuffers::default(),
            dmabuf,
            allow,
        }
    }

    /// 灌合成器 feedback（主设备 + 格式表）；后建的表面用 `set_feedback` 补上。
    fn set_feedback(&mut self, main_device: libc::dev_t, formats: &[(u32, u64)]) {
        self.dmabuf.set_feedback(main_device, formats.to_vec());
    }

    /// 提交一帧：dmabuf 优先，失败回退 SHM。
    #[allow(clippy::too_many_arguments)]
    fn commit(
        &mut self,
        qh: &QueueHandle<Shell>,
        shm: Option<&wl_shm::WlShm>,
        dmabuf_state: &DmabufState,
        surface: &wl_surface::WlSurface,
        width: u32,
        height: u32,
        rgba: &[u8],
        scale: f32,
        damage: Option<kanesumi_core::Rect>,
    ) {
        use crate::dmabuf::CommitOutcome;
        if self.allow {
            match self
                .dmabuf
                .commit(qh, surface, dmabuf_state, width, height, rgba, scale, damage)
            {
                CommitOutcome::Committed | CommitOutcome::Skipped => return,
                CommitOutcome::Unavailable => {
                    // 单表面失败 → 该表面永久回退 SHM（其它表面不受影响）。
                    self.allow = false;
                    log::warn!("dmabuf 直通在该表面不可用 → 回退 SHM（不丢帧）");
                }
            }
        }
        if let Some(shm) = shm {
            commit_shm_buffers(shm, qh, surface, &mut self.shm, width, height, rgba, scale, damage);
        }
    }

    /// `wl_buffer.release` → 标记对应槽位可复用（SHM 与 dmabuf 槽都要认）。
    fn mark_released(&mut self, buffer: &wl_buffer::WlBuffer) -> bool {
        self.shm.mark_released(buffer) || self.dmabuf.mark_released(buffer)
    }
}

/// 单个 layer-shell 表面的 SHM 缓冲（双缓冲；尺寸变化时重建 pool/buffer）。
/// SHM 缓冲状态（主表面 + 各浮层各一份）。
/// ⚠ 双缓冲：smithay 对单缓冲客户端不发 wl_buffer.release（只在 buffer 被替换时
///   释放），单缓冲复用会导致 in_flight 永不复位 → 提交一帧后冻结。双缓冲交替
///   提交不同 buffer，触发 release，动画/悬停才能持续刷新。
struct ShmBuffers {
    pool: Option<wl_shm_pool::WlShmPool>,
    mmap: Option<memmap2::MmapMut>,
    width: u32,
    height: u32,
    /// 两个槽位的 buffer（同一 pool 的两半）。
    buffers: [Option<wl_buffer::WlBuffer>; 2],
    /// 每个槽位是否已 attach 且未收到 release。
    in_flight: [bool; 2],
    /// 下一个使用的槽位索引。
    next: usize,
    /// 槽位内容是否不可用（pool 新建/重建）→ 下次写入必须全量（否则零填充区域透明）。
    needs_full: [bool; 2],
    /// 槽位自上次写入后累积的局部损伤（物理像素）—— 该槽未写期间其它区域变化过，
    /// 下次写该槽须一并回补（buffer-age 语义，参 compositor render/damage.rs）。
    partial: [Option<Rect>; 2],
}

impl ShmBuffers {
    /// `wl_buffer.release` → 标记对应槽位可复用。返回是否命中。
    fn mark_released(&mut self, buffer: &wl_buffer::WlBuffer) -> bool {
        for (i, b) in self.buffers.iter().enumerate() {
            if b.as_ref() == Some(buffer) {
                self.in_flight[i] = false;
                return true;
            }
        }
        false
    }
}

/// IME 引擎宿主的 xkbcommon 状态 —— 把 grab keymap 的 keycode 语义化为 keysym/utf8。
/// 参 CEYBOARD_SPEC §Ⅴ（合成器把按键转发给 IME，IME 据此生成 preedit/commit）。
struct ImXkb {
    /// 保持 keymap 存活（State 引用它，drop 顺序在 state 之后）。
    #[allow(dead_code)]
    keymap: xkbcommon::xkb::Keymap,
    state: xkbcommon::xkb::State,
}

impl ImXkb {
    fn from_keymap(fd: std::os::fd::OwnedFd, size: usize) -> Result<Self, String> {
        let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
        let keymap = unsafe {
            xkbcommon::xkb::Keymap::new_from_fd(
                &context,
                fd,
                size,
                xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1,
                xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
            )
        }
        .map_err(|e| format!("keymap mmap 失败：{e}"))?
        .ok_or_else(|| "keymap 编译失败".to_string())?;
        let state = xkbcommon::xkb::State::new(&keymap);
        Ok(Self { keymap, state })
    }

    /// keycode → (keysym raw, utf8 文本)。
    fn keycode_to_sym(&self, keycode: u32) -> (u32, Option<String>) {
        let key = xkeysym::KeyCode::new(keycode + 8);
        let sym: xkeysym::Keysym = self.state.key_get_one_sym(key);
        let utf8 = self.state.key_get_utf8(key);
        let utf8 = utf8.trim_matches('\0');
        let utf8 = if utf8.is_empty() { None } else { Some(utf8.to_string()) };
        (sym.raw(), utf8)
    }

    fn update_mask(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        let _ = self.state.update_mask(depressed, latched, locked, 0, 0, group);
    }
}

/// IME 候选窗 popup surface（`zwp_input_popup_surface_v2`）。合成器渲染到 Layer 6
/// Overlay 并跟随光标（Section 1 `collect_im_popup_draws`）。参 CEYBOARD_SPEC §Ⅱ/§Ⅴ。
struct ImPopupSurface {
    surface: wl_surface::WlSurface,
    /// popup surface 对象（角色标记，保持存活）。
    #[allow(dead_code)]
    popup: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
    /// CPU 光栅化器（popup surface 走 SHM/dmabuf 提交；resize 复用，不重建）。与 `renderer` 二选一。
    cpu: Option<CpuRenderer>,
    /// wgpu 光栅化器（C2 一张画布：候选窗走 GPU 直出）。与 `cpu` 二选一。
    renderer: Option<SurfaceRenderer>,
    /// 输出缓冲（dmabuf 直通优先，SHM 回退）。
    out: SurfaceOutput,
    width: f32,
    height: f32,
}

impl Default for ShmBuffers {
    fn default() -> Self {
        Self {
            pool: None,
            mmap: None,
            width: 0,
            height: 0,
            buffers: [None, None],
            in_flight: [false, false],
            next: 0,
            needs_full: [true, true],
            partial: [None, None],
        }
    }
}

/// 浮层表面 —— 独立 wl_surface + layer-shell + CPU 光栅化（透明底）。
/// 内容由 `App::render_floating(idx)` 提供；输入按指针所在表面路由到 `floating_input`。
struct FloatingSurface {
    surface: wl_surface::WlSurface,
    layer_surface: LayerSurface,
    /// CPU 光栅化器（浮层走 layer-shell → SHM/dmabuf 提交）。与 `renderer` 二选一。
    cpu: Option<CpuRenderer>,
    /// wgpu 光栅化器（G1：大面积 / 高刷新浮层走 GPU 直出）。与 `cpu` 二选一。
    renderer: Option<SurfaceRenderer>,
    width: f32,
    height: f32,
    configured: bool,
    scale: f32,
    /// 全屏浮层（四边锚定，尺寸自适应）→ 不参与高度同步（floating_height 语义对
    /// 全屏表面无效；高度 0 = 铺满，非"收起"）。Launcher overlay 即此类。
    fullscreen: bool,
    _fractional_scale: Option<WpFractionalScaleV1>,
    viewport: Option<WpViewport>,
}

#[derive(Debug, Clone)]
struct FractionalScaleData {
    surface: wl_surface::WlSurface,
}

impl Shell {
    fn new(
        app: &'static mut dyn App,
        engine: TextEngine,
        conn: &Connection,
        globals: &wayland_client::globals::GlobalList,
        qh: &QueueHandle<Self>,
        role: EtherRole,
    ) -> Result<Self, String> {
        let compositor_state =
            CompositorState::bind(globals, qh).map_err(|e| format!("wl_compositor 不可用：{e}"))?;
        let output_state = OutputState::new(globals, qh);
        let seat_state = SeatState::new(globals, qh);

        let cfg = app.config();
        let surface = compositor_state.create_surface(qh);
        let width = cfg.width;
        let height = cfg.height;

        let fractional_scale_manager = globals
            .bind::<WpFractionalScaleManagerV1, Self, ()>(qh, 1..=1, ())
            .ok();
        let viewporter = globals.bind::<WpViewporter, Self, ()>(qh, 1..=1, ()).ok();
        let presentation = globals
            .bind::<wp_presentation::WpPresentation, Self, ()>(qh, 1..=1, ())
            .ok();
        if presentation.is_some() {
            log::info!("wp_presentation 协议已绑定");
        }
        let fractional_supported = fractional_scale_manager.is_some() && viewporter.is_some();
        let fractional_scale = fractional_supported.then(|| {
            fractional_scale_manager
                .as_ref()
                .unwrap()
                .get_fractional_scale(
                    &surface,
                    qh,
                    FractionalScaleData {
                        surface: surface.clone(),
                    },
                )
        });
        let viewport = fractional_supported
            .then(|| viewporter.as_ref().unwrap().get_viewport(&surface, qh, ()));
        if viewport.is_some() {
            surface.set_buffer_scale(1);
        }

        // 建表面：按角色分派 xdg-shell / layer-shell。
        let mut layer_shell = None;
        let mut xdg_shell = None;
        let mut window = None;
        let mut layer_surface = None;

        match role.surface_kind() {
            SurfaceKind::XdgShell => {
                let shell =
                    XdgShell::bind(globals, qh).map_err(|e| format!("xdg_wm_base 不可用：{e}"))?;
                let win =
                    shell.create_window(surface.clone(), WindowDecorations::RequestServer, qh);
                win.set_title(cfg.title);
                win.set_app_id(cfg.app_id);
                win.set_min_size(Some((
                    cfg.min_width.round().max(1.0) as u32,
                    cfg.min_height.round().max(1.0) as u32,
                )));
                win.commit();
                window = Some(win);
                xdg_shell = Some(shell);
            }
            SurfaceKind::LayerBackground => {
                // 外部布局：桌面/墙纸层表面。非窗口——无 SSD/关闭键，不受 Alt+F4。
                // 参 role.rs EtherRole::Desktop；Ether 合成器需将 Background 画在最底。
                let shell = LayerShell::bind(globals, qh)
                    .map_err(|e| format!("zwlr_layer_shell_v1 不可用：{e}"))?;
                let ls = shell.create_layer_surface(
                    qh,
                    surface.clone(),
                    Layer::Background,
                    Some(cfg.app_id),
                    None,
                );
                ls.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
                ls.set_exclusive_zone(0);
                ls.set_size(0, 0);
                // OnDemand：桌面点中后要能收键盘（Delete / F2 / Enter / 方向键 / 重命名输入）。
                // 曾为 None —— 桌面上一切键盘操作都无效（2026-10-01 桌面审计）。
                ls.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                ls.commit();
                layer_surface = Some(ls);
                layer_shell = Some(shell);
            }
            SurfaceKind::LayerTop | SurfaceKind::LayerBottom | SurfaceKind::LayerOverlay => {
                let shell = LayerShell::bind(globals, qh)
                    .map_err(|e| format!("zwlr_layer_shell_v1 不可用：{e}"))?;
                let layer = match role.surface_kind() {
                    SurfaceKind::LayerTop => Layer::Top,
                    SurfaceKind::LayerBottom => Layer::Bottom,
                    _ => Layer::Overlay,
                };
                let ls =
                    shell.create_layer_surface(qh, surface.clone(), layer, Some(cfg.app_id), None);
                let anchor = match role.surface_kind() {
                    SurfaceKind::LayerTop => Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
                    SurfaceKind::LayerBottom => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                    _ => Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
                };
                ls.set_anchor(anchor);
                match role.surface_kind() {
                    SurfaceKind::LayerTop | SurfaceKind::LayerBottom => {
                        ls.set_exclusive_zone(height as i32);
                        ls.set_size(0, height as u32);
                    }
                    _ => {
                        // Overlay 主表面（Launcher/Candidate）：四边锚定铺满。
                        // ⚠ 不用 exclusive_zone(-1) + set_size(0,0)（合成器强制 (lw,0)
                        //   时可能触发 InvalidSize「height 0 without top/bottom anchors」，
                        //   旧 Ceyboard 反复被 ProtocolError 杀）。四边锚 + 尺寸 0 = 全屏。
                        ls.set_exclusive_zone(0);
                        ls.set_size(0, 0);
                    }
                }
                ls.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                ls.commit();
                layer_surface = Some(ls);
                layer_shell = Some(shell);
                // 子弹层（菜单 / 面板）要 xdg_wm_base：顶 / 底条一并绑定（缺失 → 弹层留在表面内）。
                if matches!(role.surface_kind(), SurfaceKind::LayerTop | SurfaceKind::LayerBottom) {
                    xdg_shell = XdgShell::bind(globals, qh)
                        .map_err(|e| log::warn!("xdg_wm_base 不可用，子弹层停用：{e}"))
                        .ok();
                }
            }
            // 锁屏表面：不经 xdg/layer 单主表面模型，由 `crate::session_lock` 外壳接管。
            // run_inner 已在建 Shell 前分流（此处仅保证枚举穷尽）。
            SurfaceKind::SessionLock => {
                return Err("锁屏角色由 session_lock 外壳处理（run_inner 已分流）".into());
            }
        }

        // wgpu 渲染器**延迟**到首个 configure 后创建（Shell::new 时 surface 尚未
        // configure，尺寸 0，Vulkan surface 会报 SURFACE_LOST_KHR —— Known Issue #8）。
        // 由 ensure_renderer() 在 WindowHandler/LayerShellHandler::configure 里触发。
        let renderer = None;

        // IME：绑定 text-input manager（合成器缺失 → None，App 降级走裸 KeyPressed，记一次日志）。
        // ⚠ range 上限须 ≤ wayland-protocols 声明的接口最大版本。smithay 0.7（Ether 合成器）
        // 仅实现 zwp_text_input_v3 v1，故请求 1..=1；写 1..=2 会触发 globals.rs panic
        // （"Maximum version (2) was higher than the proxy's maximum version (1)"）。
        let text_input_manager = globals
            .bind::<ZwpTextInputManagerV3, Self, ()>(qh, 1..=1, ())
            .map_err(|e| {
                log::warn!("zwp_text_input_manager_v3 不可用，IME 降级：{e}");
            })
            .ok();

        // IME 引擎宿主：App 声明 ime_engine_host() 时绑定 zwp_input_method_manager_v2。
        // 合成器未提供 → None，Ceyboard 退化为无键盘引擎（仅 UI 展示）。
        let input_method_manager = if app.ime_engine_host() {
            let m = globals
                .bind::<ZwpInputMethodManagerV2, Self, ()>(qh, 1..=1, ())
                .map_err(|e| {
                    log::warn!("zwp_input_method_manager_v2 不可用，引擎宿主降级：{e}");
                })
                .ok();
            if m.is_some() {
                log::info!("引擎宿主：zwp_input_method_manager_v2 已绑定");
            }
            m
        } else {
            None
        };

        // 虚拟键盘 manager：引擎宿主重放未消费按键（arrow/backspace 透传）。参 CEYBOARD_SPEC §Ⅴ。
        let vk_manager = if app.ime_engine_host() {
            let m = globals
                .bind::<ZwpVirtualKeyboardManagerV1, Self, ()>(qh, 1..=1, ())
                .map_err(|e| {
                    log::warn!("zwp_virtual_keyboard_manager_v1 不可用，按键透传降级：{e}");
                })
                .ok();
            if m.is_some() {
                log::info!("引擎宿主：zwp_virtual_keyboard_manager_v1 已绑定");
            }
            m
        } else {
            None
        };

        // wl_shm 全局（layer-shell 角色 CPU 光栅化 → wl_shm 提交）。合成器未提供 → None，
        // 退化直接 present（xdg-shell 路径不受影响）。
        let shm = globals
            .bind::<wl_shm::WlShm, Self, ()>(qh, 1..=1, ())
            .map_err(|e| {
                log::warn!("wl_shm 不可用，SHM 提交降级为直接 present：{e}");
            })
            .ok();
        // 客户端 dmabuf 全局（linux-dmabuf-feedback 主设备协商见 M5）。
        // ⚠ DMABUF 属性：非 XRGB8888（无 alpha）→ Alpha 通道读 0 → 整个 buffer 透明；
        //   bo 用带 alpha 的 fourcc（ARGB8888/ABGR8888），合成器按 alpha 合成。
        let dmabuf_state = DmabufState::new(globals, qh);
        let dmabuf_present = dmabuf_state.version().is_some();
        // dmabuf 直通**默认开**（2026-09-18 M6）：合成器提供 global 且未显式 `ETHER_DMABUF=0`，
        // 且 **崩溃安全探测** 通过（gbm 打开在部分 Mesa 上段错误 —— Debian 2026-08-19 黑屏根因，
        // 段错误无法进程内捕获 → 子进程探测，见 crate::dmabuf::probe_gbm_crash_safe）。
        // ⚠ 探测与 gbm device 都不在此处无条件执行：探测在子进程里、device 仍惰性。
        let dmabuf_opt_out = std::env::var("ETHER_DMABUF").as_deref() == Ok("0");
        // 探测证明可用的组合（`None` = 未探测出 → 全 SHM，绝不在未验证组合上分配）。
        // ⚠ 此刻尚未收到 per-surface feedback（主设备 dev_t 异步到达）→ 传 None；
        //   「主设备节点」备选跳过，真实提交仍按已证明的组合分配。
        let dmabuf_recipe = if dmabuf_present && !dmabuf_opt_out {
            crate::dmabuf::probe_gbm_crash_safe(None)
        } else {
            None
        };
        let dmabuf_allowed = dmabuf_recipe.is_some();
        // 主动请求 per-surface dmabuf feedback（v4+）：拿主设备 dev_t + 格式表。
        // ⚠ 必须显式请求 —— 合成器只对 protocol version < 4 发 legacy format/modifier 事件，
        //   v5 global 下不请求 feedback 则一个格式事件都收不到（无法做设备/格式校验）。
        let dmabuf_feedback_obj = if dmabuf_present && dmabuf_state.version().unwrap_or(0) >= 4 {
            match dmabuf_state.get_surface_feedback(&surface, qh) {
                Ok(fb) => Some(fb),
                Err(e) => {
                    log::warn!("dmabuf feedback 请求失败（设备/格式校验降级为默认）：{e}");
                    None
                }
            }
        } else {
            None
        };
        if dmabuf_allowed {
            log::info!("dmabuf 直通启用（默认；ETHER_DMABUF=0 可回退 SHM）—— 覆盖主表面/浮层/候选窗");
        } else {
            log::info!(
                "SHM 提交路径：global={} opt_out={} probe_ok={}",
                dmabuf_present,
                dmabuf_opt_out,
                dmabuf_allowed
            );
        }

        // 浮层表面：独立 layer-shell surface（透明底控件浮层）。非 layer-shell 角色无浮层。
        let floating = match &layer_shell {
            Some(s) => app
                .floating_layers()
                .into_iter()
                .map(|spec| {
                    create_floating_surface(
                        &compositor_state,
                        s,
                        qh,
                        spec,
                        fractional_scale_manager.as_ref(),
                        viewporter.as_ref(),
                    )
                })
                .collect::<Result<Vec<_>, String>>()?,
            None => Vec::new(),
        };
        // 进程共享 gbm device 句柄（惰性：首次 dmabuf 提交才真正打开）。
        // 探测证明可用的组合存进共享 device → 所有表面（含后建浮层/popup）自动继承，
        // 无需在每个 `SurfaceOutput::new` 点重复传递。
        let dmabuf_device = crate::dmabuf::DmabufDevice::default();
        dmabuf_device.set_recipe(dmabuf_recipe);
        // 合成图层（G3-b）：wl_subcompositor + ether_composition_v1（Ether 私有，别的合成器没有 → None 回落）。
        let comp = layers::Composition::new(
            globals.bind::<layers::EtherCompositionManagerV1, Self, ()>(qh, 1..=1, ()).ok(),
            smithay_client_toolkit::subcompositor::SubcompositorState::bind(
                compositor_state.wl_compositor().clone(),
                globals,
                qh,
            )
            .ok(),
        );
        let floating_out = std::iter::repeat_with(|| {
            SurfaceOutput::new(dmabuf_allowed, dmabuf_device.clone())
        })
        .take(floating.len())
        .collect();
        let main_out = SurfaceOutput::new(dmabuf_allowed, dmabuf_device.clone());

        // 全局应用菜单：App 声明了菜单树 → 安装（D-Bus 服务 + Wayland 绑定 + Registrar）。
        // 服务线程在后台跑，命令经通道回主线程每帧排干（App::on_menu_command）。
        let (appmenu, appmenu_rx) = match app.app_menu() {
            Some(tree) => {
                let (handle, rx) = crate::appmenu::install(conn, &surface, tree, cfg.app_id);
                // 注入句柄：App 据此运行时更新勾选 / 结构（set_check / update_tree）。
                app.set_appmenu_handle(handle.clone());
                (Some(handle), Some(rx))
            }
            None => (None, None),
        };

        let floating_len = floating.len();
        let perf_proc = std::env::args()
            .next()
            .and_then(|a| {
                std::path::Path::new(&a)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "ether-harness".into());

        Ok(Self {
            app,
            engine,
            role,
            registry_state: RegistryState::new(globals),
            compositor_state,
            output_state,
            seat_state,
            layer_shell,
            xdg_shell,
            surface,
            window,
            layer_surface,
            pointer: None,
            keyboard: None,
            renderer,
            cpu: None,
            gpu: None,
            gpu_failed: false,
            gpu_kill: gpu_kill_switch(),
            dirty: true,
            floating_dirty: vec![false; floating.len()],
            floating_full: vec![true; floating.len()],
            floating_raster_size: vec![None; floating.len()],
            floating_visible_cache: vec![false; floating.len()],
            scale: 1.0,
            _fractional_scale_manager: fractional_scale_manager,
            _viewporter: viewporter,
            _fractional_scale: fractional_scale,
            viewport,
            width,
            height,
            configured: false,
            running: true,
            frame_pending: false,
            presentation,
            frame_clock: FrameClock::default(),
            main_throttle: PacingThrottle::new(),
            floating_throttle: vec![PacingThrottle::new(); floating.len()],
            coalescer: InputCoalescer::new(),
            pointer_pos: (-1.0, -1.0),
            pointer_buttons: 0,
            pending_damage: None,
            click_tracker: crate::app::ClickTracker::default(),
            ctx_menu: crate::context_menu::ContextMenuState::new(),
            text_input_manager,
            text_input: None,
            ime_enabled: false,
            ime_focus_surface: false,
            commit_serial: 0,
            pending_ime: PendingImeBatch::default(),
            ime_context_cache: None,
            input_method_manager,
            input_method: None,
            im_keyboard_grab: None,
            im_active: false,
            im_done_serial: 0,
            im_xkb: None,
            im_preedit_cache: None,
            im_modifiers: Modifiers::NONE,
            im_surrounding: None,
            im_popup: None,
            im_popup_dirty: false,
            vk_manager,
            virtual_keyboard: None,
            im_key_time: 0,
            modifiers: Modifiers::NONE,
            conn: conn.clone(),
            pending_height: None,
            floating,
            appmenu,
            appmenu_rx,
            diag_logged: false,
            frame_count: 0,
            perf_main: crate::perf::SurfacePerf::new(),
            perf_floating: vec![crate::perf::SurfacePerf::new(); floating_len],
            perf_im_popup: crate::perf::SurfacePerf::new(),
            perf_flush_at: Instant::now(),
            perf_proc,
            perf_header_done: false,
            system_theme: MetroTheme::ether_dark(),
            theme_fingerprint: None,
            next_theme_check: Instant::now(),
            interaction: kanesumi_core::InteractionSettings::default(),
            interaction_fingerprint: None,
            shm,
            main_out,
            floating_out,
            last_update: Instant::now(),
            scene_buf: Scene::default(),
            dmabuf: dmabuf_state,
            dmabuf_allowed,
            dmabuf_feedback: None,
            dmabuf_feedback_obj,
            dmabuf_device,
            popups: Vec::new(),
            comp,
            popups_enabled: false,
            popup_bounds_sent: None,
            seat: None,
            last_input_serial: None,
        })
    }

    /// 浮层表面索引（按 wl_surface 比对；PointerHandler / CompositorHandler 分发用）。
    fn floating_idx(&self, surface: &wl_surface::WlSurface) -> Option<usize> {
        self.floating.iter().position(|f| f.surface == *surface)
    }

    /// 浮层表面索引（按 layer surface 比对；LayerShellHandler configure 分发用）。
    fn floating_idx_by_layer(&self, layer: &LayerSurface) -> Option<usize> {
        self.floating.iter().position(|f| f.layer_surface == *layer)
    }

    /// 首个输出的逻辑尺寸（全屏浮层 fallback；Ether 合成器 configure 常给高度 0）。
    fn output_logical_size(&self) -> Option<(i32, i32)> {
        self.output_state
            .outputs()
            .next()
            .and_then(|o| self.output_state.info(&o))
            .and_then(|info| info.logical_size)
    }

    /// 浮层高度同步（面板展开/收起）：App::floating_height 与当前不符 → set_size 立即
    /// 生效（高度 0 = 收起，无命中无渲染）。在渲染帧内调用（App update 后）。
    fn sync_floating_heights(&mut self) {
        for (i, f) in self.floating.iter_mut().enumerate() {
            if f.fullscreen {
                // 全屏浮层（Launcher overlay）：尺寸自适应铺满，不参与高度同步。
                continue;
            }
            let h = self.app.floating_height(i);
            if (h - f.height).abs() < 0.5 {
                continue;
            }
            // ⚠ Bottom-only 锚定浮层高度 0 非法（需上下同时锚定才可 0）→ 至少 1。
            let h = h.max(1.0);
            f.layer_surface.set_size(f.width as u32, h as u32);
            f.height = h;
            if let Some(cpu) = f.cpu.as_mut() {
                cpu.resize(f.width, h, f.scale);
            }
            if let Some(r) = f.renderer.as_mut() {
                r.resize(f.width, h, f.scale);
            }
            self.floating_dirty[i] = true; // 尺寸变化 → 呈现新高度（I-3）。
            if let Some(viewport) = f.viewport.as_ref()
                && h > 1.0
            {
                viewport
                    .set_destination(f.width.round().max(1.0) as i32, h.round().max(1.0) as i32);
            }
        }
    }

    /// 浮层渲染器惰性创建（G1）：按策略选 GPU / CPU；GPU 不可用或创建失败 → CPU。
    fn ensure_floating_renderer(&mut self, idx: usize) {
        let Some((fw, fh, fscale, surf)) = self.floating.get(idx).and_then(|f| {
            (f.cpu.is_none() && f.renderer.is_none())
                .then(|| (f.width, f.height, f.scale, f.surface.clone()))
        }) else {
            return;
        };
        let class = SurfaceClass::Floating;
        let area = ((fw * fscale).max(0.0) * (fh * fscale).max(0.0)) as u64;
        let hz = default_expected_hz(class);
        let gpu_ok = self.gpu_available();
        let one_canvas = crate::renderer_policy::one_canvas();
        let kind = choose_renderer(class, area, hz, gpu_ok, one_canvas);
        log::info!(
            "渲染器选择：浮层 #{}（{:.0}x{:.0}，scale {}）area={}px² hz={:.1} → {:?}（{}）",
            idx,
            fw,
            fh,
            fscale,
            area,
            hz,
            kind,
            crate::renderer_policy::decision_reason(class, area, hz, gpu_ok, one_canvas),
        );
        if kind == RendererKind::Gpu {
            if let Some(ctx) = self.ensure_gpu_context(&surf) {
                match SurfaceRenderer::with_context(ctx, &self.conn, &surf, fw, fh, fscale, true) {
                    Ok(mut r) => {
                        // 预热：空场景提交一次，强制管线/着色器编译（消除 G0 测得的首帧 20~40 ms 尖峰）。
                        r.render(&self.engine, &Scene::default());
                        log::info!("浮层 wgpu 渲染器已创建（{}）", r.diagnostics());
                        self.floating[idx].renderer = Some(r);
                        return;
                    }
                    Err(e) => {
                        log::warn!("浮层 wgpu 渲染器创建失败（{e:?}），回退 CPU");
                    }
                }
            } else {
                log::warn!("浮层 wgpu 共享上下文不可用，回退 CPU");
            }
        }
        let cpu = CpuRenderer::new(fw, fh, fscale);
        log::info!("浮层 CPU 光栅化器已创建（{:.0}x{:.0}）", fw, fh);
        self.floating[idx].cpu = Some(cpu);
    }

    /// 渲染浮层帧：ensure 渲染器（透明底）→ App::render_floating → 光栅化 → 提交。
    /// GPU 路径消费损伤（只重画损伤矩形；无损伤不提交）；CPU 路径走 cpu_raster。
    fn render_floating_frame(&mut self, idx: usize, qh: &QueueHandle<Self>) {
        if !self.app.floating_visible(idx) {
            // 浮层隐藏：不请求下一帧 → 表面空闲（零成本）。
            return;
        }
        if self.floating.get(idx).map(|f| !f.configured).unwrap_or(true) {
            return;
        }
        let (can_render, is_timeout) = if idx < self.floating_throttle.len() {
            self.floating_throttle[idx]
                .can_render(Instant::now(), self.app.floating_needs_redraw(idx))
        } else {
            (true, false)
        };
        if !can_render {
            return;
        }
        if is_timeout {
            log::warn!("frame_cb_timeout: 浮层 {idx} 100ms 未收到 frame 回调，强制渲染");
            write_diag(
                "ether-harness-trace.log",
                &format!("frame_cb_timeout: floating {idx}\n"),
            );
        }
        self.floating_dirty[idx] = false;
        self.ensure_floating_renderer(idx);
        let (app, floating) = (&mut self.app, &mut self.floating);
        let Some(f) = floating.get_mut(idx) else {
            return;
        };
        // ⚠ 光栅器惰性创建前先同步当前 App 请求高度：面板打开时 floating_height 返回
        //   面板高，f.height 可能仍是收起值 → 光栅器用 0 高度创建 → 浮层永远不可见。
        //   全屏浮层（四边锚定）不参与：高度 0 = 铺满，按 0 同步会把它压成 1px（R1 Launcher 不出图）。
        let h = app.floating_height(idx);
        if !f.fullscreen && (h - f.height).abs() >= 0.5 {
            let h = h.max(1.0);
            f.layer_surface.set_size(f.width as u32, h as u32);
            f.height = h;
            if let Some(cpu) = f.cpu.as_mut() {
                cpu.resize(f.width, h, f.scale);
            }
            if let Some(r) = f.renderer.as_mut() {
                r.resize(f.width, h, f.scale);
            }
            if let Some(viewport) = f.viewport.as_ref()
                && h > 1.0
            {
                viewport
                    .set_destination(f.width.round().max(1.0) as i32, h.round().max(1.0) as i32);
            }
        }
        // 局部光栅：新建 / 重新显示 / 尺寸变了 → 整幅（先通知 App 整幅拼接）；否则按 App 帧损伤。
        let phys_now = if let Some(cpu) = f.cpu.as_ref() {
            Some(cpu.physical_size())
        } else {
            f.renderer.as_ref().map(|r| r.physical_size())
        };
        let full = self.floating_full[idx] || self.floating_raster_size[idx] != phys_now;
        if full {
            app.floating_full_repaint(idx);
        }
        let t_render = Instant::now();
        // 本表面缩放注入全局：应用在 paint 期光栅图标时读它出物理像素。
        set_surface_scale(f.scale);
        let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            app.render_floating(&self.engine, idx, Size::new(f.width, f.height))
        }));
        let scene = match scene {
            Ok(s) => s,
            Err(_) => {
                log::error!("App::render_floating panic，跳过本帧");
                let s = f.surface.clone();
                s.frame(qh, s.clone());
                return;
            }
        };
        let render_ms = t_render.elapsed().as_secs_f32() * 1000.0;
        let s = f.surface.clone();
        let mut raster_ms = 0.0f32;
        let mut commit_ms = 0.0f32;
        let mut gpu_samples: Vec<f32> = Vec::new();
        let mut draws: Option<u32> = None;
        let mut acquire_ms: Option<f32> = None;
        let mut c15_stats: Option<(bool, f32, u32)> = None;
        if let Some(cpu) = f.cpu.as_mut() {
            let (pw, ph) = cpu.physical_size();
            let t = Instant::now();
            let damage = if full { None } else { app.floating_damage(idx) };
            // 零面积 = 这一帧什么都没变：不光栅、不提交。
            if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
                return;
            }
            let rgba = cpu.render(&self.engine, &scene, damage);
            raster_ms = t.elapsed().as_secs_f32() * 1000.0;
            // 浮层同样走 dmabuf 直通（默认）—— 控制面板 / Launcher / 菜单等浮层一并受益。
            let scale = f.scale;
            s.frame(qh, s.clone());
            if idx < self.floating_throttle.len() {
                self.floating_throttle[idx].on_request_callback(Instant::now());
            }
            send_presentation_feedback(
                self.presentation.as_ref(),
                self.frame_clock.current_predicted_present(),
                idx + 1,
                &s,
                qh,
            );
            let t = Instant::now();
            self.floating_out[idx].commit(
                qh,
                self.shm.as_ref(),
                &self.dmabuf,
                &s,
                pw,
                ph,
                rgba,
                scale,
                damage,
            );
            commit_ms = t.elapsed().as_secs_f32() * 1000.0;
            if idx < self.floating_throttle.len() {
                self.floating_throttle[idx].on_frame_rendered();
            }
            self.floating_full[idx] = false;
            self.floating_raster_size[idx] = phys_now;
        } else if let Some(r) = f.renderer.as_mut() {
            // GPU（G1）：消费损伤 —— 全幅或只重画损伤矩形（scissor + MSAA Load 保留
            // 上一帧其余像素）；零面积 = 本帧无变化 → 不提交。
            let damage = if full { None } else { app.floating_damage(idx) };
            if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
                self.floating_full[idx] = false;
                return;
            }
            s.frame(qh, s.clone());
            if idx < self.floating_throttle.len() {
                self.floating_throttle[idx].on_request_callback(Instant::now());
            }
            send_presentation_feedback(
                self.presentation.as_ref(),
                self.frame_clock.current_predicted_present(),
                idx + 1,
                &s,
                qh,
            );
            let t = Instant::now();
            r.render_with_damage(&self.engine, &scene, damage);
            raster_ms = t.elapsed().as_secs_f32() * 1000.0;
            if idx < self.floating_throttle.len() {
                self.floating_throttle[idx].on_frame_rendered();
            }
            gpu_samples = r.drain_gpu_samples();
            draws = r.last_draws();
            acquire_ms = r.take_acquire_ms();
            c15_stats = r.last_c15_stats();
            self.floating_full[idx] = false;
            self.floating_raster_size[idx] = phys_now;
        }
        if let Some(p) = self.perf_floating.get_mut(idx) {
            p.record(render_ms, raster_ms, commit_ms);
            if let Some(n) = draws {
                p.record_draws(n);
            }
            if let Some(ms) = acquire_ms {
                p.record_acquire(ms);
            }
            for ms in gpu_samples {
                p.record_gpu(ms);
            }
            if let Some((inc, dmg_pct, insts)) = c15_stats {
                p.record_c15(inc, dmg_pct, insts);
            }
        }
        // 掉帧计数：浮层本帧已提交（零面积早退分支不计）；按表面分别记避免污染间隔。参 perf.rs。
        crate::perf::pacing_present_named(&self.pacing_floating_name(idx), Instant::now());
    }

    /// 浮层输入事件错误边界。事件到达即置脏（I-4）。
    fn emit_floating_input(&mut self, idx: usize, event: InputEvent) {
        let is_move = matches!(event, InputEvent::PointerMoved { .. });
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.floating_input(idx, event);
        }))
        .is_ok();
        if !ok {
            log::error!("App::floating_input panic，已隔离");
        }
        // 纯指针移动：App 说浮层没变就不重画（同主表面 S1 悬停门控）。此前每次移动都整屏重画 Launcher 浮层。
        // 旧 App 的 floating_needs_redraw 默认 true，行为不变。
        if idx < self.floating_dirty.len() && (!is_move || self.app.floating_needs_redraw(idx)) {
            self.floating_dirty[idx] = true;
        }
    }

    /// 输入路由：按帧排队合并（F3），出帧前统一交付。
    fn route_input(&mut self, idx: Option<usize>, event: InputEvent) {
        self.coalescer.push(idx, event);
    }

    /// 排干累积的指针移动 / 滚动输入到 App。
    fn drain_coalesced_inputs(&mut self) {
        let events = self.coalescer.drain();
        for (target, ev) in events {
            self.route_input_immediate(target, ev);
        }
    }

    /// 立即交付单个输入事件。
    fn route_input_immediate(&mut self, idx: Option<usize>, event: InputEvent) {
        match idx {
            Some(i) => self.emit_floating_input(i, event),
            None => self.emit_input(event),
        }
    }

    /// 同步 App 请求的动态高度：layer-shell 角色下 `preferred_height` 与当前高度
    /// 不一致 → `set_size` 并立即生效（不等 configure 往返，参旧 topbar.rs `set_height`）。
    /// 合成器按 cached_state.size.h 扩大命中与渲染区域，无需等协议确认。
    fn sync_preferred_height(&mut self) {
        let Some(h) = self.app.preferred_height() else {
            return;
        };
        // ⚠ Bottom-only 锚定主表面高度 0 非法（需上下同时锚定）→ 至少 1（Dock 收起用）。
        let h = h.max(1.0);
        if (h - self.height).abs() < 0.5 {
            return;
        }
        if let Some(ls) = self.layer_surface.as_ref() {
            ls.set_size(0, h as u32);
            // 排他区域随高度同步（Dock 收起 → 工作区让出全高）。
            if self.role.surface_kind() == SurfaceKind::LayerBottom {
                ls.set_exclusive_zone(h.round() as i32);
            }
        }
        self.height = h;
        self.pending_height = Some(h);
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale);
        }
        if let Some(cpu) = self.cpu.as_mut() {
            cpu.resize(self.width, self.height, self.scale);
        }
        self.dirty = true; // 尺寸变化 → 呈现新高度（I-3）。
        if let Some(viewport) = self.viewport.as_ref() {
            viewport.set_destination(
                self.width.round().max(1.0) as i32,
                self.height.round().max(1.0) as i32,
            );
        }
    }

    /// 逻辑尺寸。
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// S4：累积局部损坏矩形（取并集；用于菜单悬停高亮这类小区域变化）。
    fn accumulate_damage(&mut self, rect: Rect) {
        self.pending_damage = Some(match self.pending_damage {
            Some(p) => union_rect(p, rect),
            None => rect,
        });
    }

    /// S4：消费本帧损坏矩形 = pending ∪ App::damage_hint。任一为 None
    /// （App 未报局部变化 / 明确全量）→ 全量。消费后清 pending。
    fn take_damage(&mut self) -> Option<Rect> {
        let mut d = self.pending_damage.take();
        if let Some(app_d) = self.app.damage_hint() {
            let empty = app_d.size.width <= 0.0 || app_d.size.height <= 0.0;
            d = Some(match d {
                // App 报「没变」（零面积）：外壳自己的待重画区照旧，不并入原点处的空矩形。
                Some(p) if empty => p,
                Some(p) => union_rect(p, app_d),
                None => app_d,
            });
        }
        d
    }

    /// 排干全局菜单命令 → App::on_menu_command(id)。错误边界隔离（panic 不杀进程）。
    /// 服务线程在后台把点击 id 塞进通道，这里每帧非阻塞消费。
    fn drain_menu_commands(&mut self) {
        let cmds: Vec<i32> = self
            .appmenu_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for id in cmds {
            let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.app.on_menu_command(id);
            }))
            .is_ok();
            if !ok {
                log::error!("App::on_menu_command panic，已隔离");
            }
        }
    }

    /// GPU 是否可用：kill-switch 未命中且共享上下文尚未永久失败。
    fn gpu_available(&self) -> bool {
        !self.gpu_kill && !self.gpu_failed
    }

    /// 惰性创建进程共享 wgpu 上下文；已建则复用。失败 → `gpu_failed` 永久回落 CPU 并记日志。
    /// 不检查 kill-switch（xdg 窗口没有 CPU 路径，必须 GPU；kill-switch 只门控 layer 表面）。
    fn ensure_gpu_context(
        &mut self,
        wl_surface: &wl_surface::WlSurface,
    ) -> Option<Arc<GpuContext>> {
        if self.gpu_failed {
            return None;
        }
        // 故障注入：KANESUMI_GPU_INIT_FAIL=1 模拟 wgpu 初始化失败，验证「初始化失败 →
        // 永久回落 CPU」路径（参任务书验证第 4 条）。
        if std::env::var_os("KANESUMI_GPU_INIT_FAIL").is_some_and(|v| v == "1") {
            log::error!("KANESUMI_GPU_INIT_FAIL=1：模拟 wgpu 初始化失败，永久回落 CPU 光栅");
            self.gpu_failed = true;
            return None;
        }
        if let Some(ctx) = &self.gpu {
            return Some(ctx.clone());
        }
        // 后端候选与 `Renderer::new` 一致：Vulkan 优先，GL 回退（Known Issue #8）。
        match GpuContext::new(
            &self.conn,
            wl_surface,
            &[wgpu::Backends::VULKAN, wgpu::Backends::GL],
        ) {
            Ok(ctx) => {
                log::info!(
                    "wgpu 共享上下文已创建（format={:?}）—— 主表面/浮层共用一份 device",
                    ctx.format()
                );
                // 时间线：wgpu instance / adapter / device 就绪（怀疑的最大头）。
                crate::timeline::note_once_detail(
                    "gpu_device_ready",
                    &format!("format={:?}", ctx.format()),
                );
                self.gpu = Some(ctx.clone());
                Some(ctx)
            }
            Err(e) => {
                log::error!("wgpu 共享上下文初始化失败（{e:?}），本进程永久回落 CPU 光栅");
                self.gpu_failed = true;
                None
            }
        }
    }

    /// 确保渲染器已创建（首个 configure 后调用；surface 已配置、尺寸已知）。
    ///
    /// 按表面类型分派（G1，参 Ether docs/GPU_COMPOSITION_PLAN.md §Ⅲ）：
    /// - xdg-shell（Settings 窗口等）→ wgpu `Renderer`（直出 present，恒 GPU）。
    /// - layer-shell 主表面 → `choose_renderer` 按面积 × 刷新频率选 GPU 或 CPU；
    ///   `KANESUMI_LAYER_GPU=1` 为遗留调试开关（强制该主表面走 GPU）。
    ///
    /// wgpu 初始化失败 → 回落 CPU（layer）或退出（xdg）。
    fn ensure_renderer(&mut self) {
        if self.renderer.is_some() || self.cpu.is_some() {
            return;
        }
        if matches!(self.role.surface_kind(), SurfaceKind::XdgShell) {
            let wl_surface = if let Some(w) = &self.window {
                w.wl_surface().clone()
            } else {
                self.surface.clone()
            };
            let Some(ctx) = self.ensure_gpu_context(&wl_surface) else {
                log::error!("wgpu 共享上下文不可用，xdg 窗口退出");
                write_diag(
                    "ether-renderer-error.log",
                    "wgpu 共享上下文初始化失败（xdg 窗口）\n",
                );
                self.running = false;
                return;
            };
            match SurfaceRenderer::with_context(
                ctx,
                &self.conn,
                &wl_surface,
                self.width,
                self.height,
                self.scale,
                false,
            ) {
                Ok(r) => {
                    log::info!("wgpu 渲染器已创建（{:.0}x{:.0}）", self.width, self.height);
                    if matches!(r, SurfaceRenderer::V2(_)) && !crate::canvas_v2::canvas_full_forced() {
                        self.app.set_damage_cull(true);
                    }
                    self.renderer = Some(r);
                    crate::timeline::note_once("renderer_ready");
                }
                Err(e) => {
                    log::error!("wgpu 渲染器初始化失败（{e:?}），退出");
                    write_diag(
                        "ether-renderer-error.log",
                        &format!("wgpu 渲染器初始化失败：{e:?}\n"),
                    );
                    self.running = false;
                }
            }
            return;
        }

        // layer-shell 主表面：按「面积 × 预期刷新频率」选渲染器。
        let class = SurfaceClass::LayerMain;
        let area = ((self.width * self.scale).max(0.0) * (self.height * self.scale).max(0.0)) as u64;
        let hz = default_expected_hz(class);
        let gpu_ok = self.gpu_available();
        let one_canvas = crate::renderer_policy::one_canvas();
        let force = std::env::var_os("KANESUMI_LAYER_GPU").is_some_and(|v| v == "1");
        let mut kind = choose_renderer(class, area, hz, gpu_ok, one_canvas);
        if force && gpu_ok {
            kind = RendererKind::Gpu;
        }
        log::info!(
            "渲染器选择：主表面（{:?}）area={}px² hz={:.1} → {:?}（{}）",
            self.role,
            area,
            hz,
            kind,
            crate::renderer_policy::decision_reason(class, area, hz, gpu_ok, one_canvas),
        );
        if kind == RendererKind::Gpu {
            let surf = self.surface.clone();
            if let Some(ctx) = self.ensure_gpu_context(&surf) {
                match SurfaceRenderer::with_context(
                    ctx,
                    &self.conn,
                    &surf,
                    self.width,
                    self.height,
                    self.scale,
                    false,
                ) {
                    Ok(r) => {
                        log::info!("主表面 wgpu 渲染器已创建（{}）", r.diagnostics());
                        if matches!(r, SurfaceRenderer::V2(_)) && !crate::canvas_v2::canvas_full_forced() {
                            self.app.set_damage_cull(true);
                        }
                        self.renderer = Some(r);
                        crate::timeline::note_once("renderer_ready");
                        return;
                    }
                    Err(e) => {
                        log::warn!("主表面 wgpu 渲染器创建失败（{e:?}），回退 CPU");
                    }
                }
            } else {
                log::warn!("主表面 wgpu 共享上下文不可用，回退 CPU");
            }
        }
        // CPU 光栅化。无失败模式：Vec 分配即就绪（I-1）。
        let cpu = CpuRenderer::new(self.width, self.height, self.scale);
        log::info!(
            "CPU 光栅化器已创建（{:.0}x{:.0}，scale {}）",
            self.width,
            self.height,
            self.scale,
        );
        self.cpu = Some(cpu);
        crate::timeline::note_once("cpu_renderer_ready");
        // CPU 局部光栅只重画 damage 区 → App 可按 damage 剔除拼接（wgpu 整幅直出不开）。
        self.app.set_damage_cull(true);
    }

    /// 推进步（每循环迭代；与渲染解耦，TOPBAR_RENDER_REFACTOR I-4）：
    /// update / 定时器 / 菜单命令 / IME / 尺寸同步。渲染完全由 `dirty` 驱动，
    /// 本步只推进状态并显式置位脏标记（I-3）。
    fn step(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let now = Instant::now();
        let can_render_main = self.main_throttle.can_render(now, self.app.needs_redraw()).0;
        let can_render_floating = self.floating.iter().enumerate().any(|(i, _)| {
            self.floating_throttle
                .get_mut(i)
                .map(|t| t.can_render(now, self.app.floating_needs_redraw(i)).0)
                .unwrap_or(true)
        });
        if !can_render_main && !can_render_floating {
            return;
        }

        crate::timeline::note_once("step_start");
        // 呈现时钟采样（F3）：由 FrameClock 预测下一次呈现并计算 dt（夹在 [0, 0.05 s]），
        // 动画按这一帧将显示的时刻推进。
        let clock_dt = self.frame_clock.advance_frame(now);
        self.last_update = now;

        // 全局菜单命令（App::on_menu_command）：在 App::update 之前派发，
        // 保证菜单触发的状态变更当帧生效。
        self.drain_menu_commands();

        // 输入按帧合并（F3）：出帧前一次性将累积的指针移动 / 轴事件交付给 App。
        self.drain_coalesced_inputs();

        // 错误边界：App update panic 不杀进程（§4.1 鲁棒性）。
        let update_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.advance_clock(clock_dt);
            self.app.update(clock_dt);
            // 主题变更检测（节流）：Chorus 改了 accent / scheme 后自动跟随。
            self.maybe_reload_system_theme();
        }))
        .is_ok();
        if !update_ok {
            log::error!("App::update panic，跳过本迭代");
            return;
        }
        // 合成图层命令（G3-b）：App 本帧推的建层 / 内容 / 动画请求。
        self.process_layer_commands(qh);
        // 右键菜单动画 tick（弹出/关闭轨道，与 App 状态解耦）。
        self.ctx_menu.update(clock_dt);

        // 动态高度同步：App update 后可能请求展开/收起（TopBar 面板）。
        self.sync_preferred_height();
        self.sync_floating_heights();

        // 浮层可见性翻转（Launcher 开/合）→ 置浮层脏（翻转前不可见 → 不渲染）。
        for i in 0..self.floating.len() {
            let visible = self.app.floating_visible(i);
            if visible && !self.floating_visible_cache[i] {
                self.floating_dirty[i] = true;
                // 隐藏期间缓冲内容不可信 → 重新显示的首帧整幅。
                self.floating_full[i] = true;
            }
            self.floating_visible_cache[i] = visible;
        }

        // App 状态可能已变（焦点/文本/光标）→ 幂等 reconcile IME（无变化零成本）。
        self.reconcile_ime();

        // App 请求关闭（文件选择器等交付结果后）→ 退出主循环（进程正常收尾）。
        if guard("should_close", || self.app.should_close()).unwrap_or(false) {
            self.running = false;
            return;
        }

        // IME 候选窗 popup 刷新（内部脏门控：im_popup_dirty）。
        self.refresh_im_popup(qh);

        // I-3 定时器/动画脏位：App 自报「内容脏」→ 置主表面 dirty。
        if self.app.needs_redraw() {
            self.dirty = true;
        }
        // 浮层同理：App 在 tick 里改了浮层树（Launcher 升起动画等）而此刻没有挂起的帧回调时，
        // 只靠回调置脏会让浮层停在上一帧（快速开合后磁贴墙卡在半途，2026-10-01 实测）。
        for i in 0..self.floating.len() {
            if self.floating_visible_cache[i] && self.app.floating_needs_redraw(i) {
                self.floating_dirty[i] = true;
            }
        }
        if self.ctx_menu.is_animating() {
            // 右键菜单开/关动画推进中 → 逐帧呈现；静态 Open 不再锁帧（S1）。
            self.dirty = true;
        }
    }

    /// 渲染 + 提交（dirty 驱动；I-1：CPU 缓冲恒就绪，无条件成功）。
    /// 渲染后若仍有动画（needs_redraw 语义 = 内容脏）→ 请求下一帧回调作 vsync
    /// 提示（I-2）。回调丢失绝不冻结：主循环 16ms 兜底超时继续推进。
    fn render_and_commit(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let (can_render, is_timeout) =
            self.main_throttle.can_render(Instant::now(), self.app.needs_redraw());
        if !can_render {
            return;
        }
        if is_timeout {
            log::warn!("frame_cb_timeout: 主表面 100ms 未收到 frame 回调，强制渲染");
            write_diag("ether-harness-trace.log", "frame_cb_timeout\n");
        }
        self.dirty = false;
        // 诊断：帧计数 + needs_redraw + frame_pending。落持久路径（write_diag），不用裸 /tmp。
        self.frame_count += 1;
        if self.frame_count <= 20 || self.frame_count % 30 == 0 {
            write_diag(
                "ether-harness-trace.log",
                &format!(
                    "frame #{}, needs_redraw={}, frame_pending={}\n",
                    self.frame_count,
                    self.app.needs_redraw(),
                    self.frame_pending
                ),
            );
        }
        // 首帧诊断：确认逻辑尺寸与 scale（排查合成器下 TopBar 显示不全/卡一半）。
        // 写入 /tmp/ether-kanesumi-diag.txt 便于会话内查看（无日志 UI）。
        if !self.diag_logged {
            self.diag_logged = true;
            let mut lines = format!(
                "harness 主表面：逻辑 {:.0}x{:.0}，scale {}，buffer 物理 {:.0}x{:.0}\n",
                self.width,
                self.height,
                self.scale,
                self.width * self.scale,
                self.height * self.scale,
            );
            if let Some(r) = self.renderer.as_ref() {
                lines.push_str(&format!("renderer: {}\n", r.diagnostics()));
            }
            write_diag("ether-kanesumi-diag.txt", &lines);
            log::info!("{}", lines.trim_end());
        }

        let target_valid = match self.renderer.as_ref() {
            Some(SurfaceRenderer::V2(r)) => r.is_target_valid(),
            Some(SurfaceRenderer::V1(_)) => false,
            None => true,
        };
        // 参 ELEMENT_TREE「compose 剔除」。CanvasV2 由 canvas_v2 的 inst_intersects 做物理损伤剔除
        // （含 1 px AA 外扩）；若在 Tree 层提前按未外扩的逻辑损伤剔除，外扩环带内的邻接图元会被
        // 丢掉，增量清除 D 后环带出现空洞，与整幅重画对拍 mismatch。非 V2（CPU / V1）保持既有语义。
        let is_v2 = matches!(self.renderer.as_ref(), Some(SurfaceRenderer::V2(_)));
        let damage_cull_active =
            damage_cull_active_for(is_v2, target_valid, crate::canvas_v2::canvas_full_forced());
        self.app.set_damage_cull(damage_cull_active);

        let size = self.size();
        // 主表面 Scene 复用缓冲（egui PaintList）：App::render_into 就地清空重建，
        // 复用 Vec 容量，不做每帧 `Scene::default()` + 逐 push 重分配。
        // `mem::take` 移动出旧缓冲（容量保留），渲染后再放回（绕过 &mut self 分裂借用）。
        let mut scene_buf = std::mem::take(&mut self.scene_buf);
        // 本表面缩放注入全局：应用在 paint 期光栅图标时读它出物理像素。
        set_surface_scale(self.scale);
        let t_render = Instant::now();
        let scene_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.render_into(&self.engine, size, &mut scene_buf)
        }));
        if scene_result.is_err() {
            log::error!("App::render panic，跳过本帧");
            self.scene_buf = scene_buf;
            return;
        }
        self.scene_buf = scene_buf;

        // 右键菜单叠加（主表面渲染，App 内容之上）。
        if self.ctx_menu.is_visible() {
            self.ctx_menu
                .render(&self.app.theme(), &self.engine, &mut self.scene_buf);
        }
        let render_ms = t_render.elapsed().as_secs_f32() * 1000.0;
        // 时间线：首帧 Scene 渲染完成（App::render_into 返回）。
        crate::timeline::note_once("first_frame_rendered");

        // 输出分派：layer-shell → CPU 光栅化 + SHM 提交；xdg-shell → wgpu 直出。
        // S4：本帧局部损坏矩形（CPU 光栅只重绘该区；GPU 直出全量，恒定消费）。
        let mut damage = self.take_damage();
        if !target_valid {
            damage = None;
        }
        // 零面积 = 这一帧什么都没变（元素树报告）：CPU 表面不光栅、不提交。
        if self.cpu.is_some() && damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
            return;
        }
        self.request_next_frame(qh);
        self.request_presentation_feedback(0, &self.surface.clone(), qh);
        let mut raster_ms = 0.0f32;
        let mut commit_ms = 0.0f32;
        let mut gpu_samples: Vec<f32> = Vec::new();
        let mut acquire_ms: Option<f32> = None;
        let mut draws: Option<u32> = None;
        if let Some(cpu) = self.cpu.as_mut() {
            let (pw, ph) = cpu.physical_size();
            let t = Instant::now();
            let rgba = cpu.render(&self.engine, &self.scene_buf, damage);
            raster_ms = t.elapsed().as_secs_f32() * 1000.0;
            // dmabuf 直通优先（零 CPU 上传），不可用自动回退 SHM。参 LINUX_DMABUF_PLAN §1。
            let t = Instant::now();
            self.main_out.commit(
                qh,
                self.shm.as_ref(),
                &self.dmabuf,
                &self.surface.clone(),
                pw,
                ph,
                rgba,
                self.scale,
                damage,
            );
            commit_ms = t.elapsed().as_secs_f32() * 1000.0;
            self.main_throttle.on_frame_rendered();
        } else if let Some(r) = self.renderer.as_mut() {
            let t = Instant::now();
            r.render_with_damage(&self.engine, &self.scene_buf, damage);
            raster_ms = t.elapsed().as_secs_f32() * 1000.0;
            self.main_throttle.on_frame_rendered();
            gpu_samples = r.drain_gpu_samples();
            acquire_ms = r.take_acquire_ms();
            if let Some(n) = r.last_draws() {
                draws = Some(n);
            }
            if let Some((inc, dmg_pct, insts)) = r.last_c15_stats() {
                self.perf_main.record_c15(inc, dmg_pct, insts);
            }
            #[cfg(debug_assertions)]
            if crate::canvas_v2::canvas_verify_enabled() {
                match r {
                    SurfaceRenderer::V2(v2) if v2.should_verify() => {
                        v2.verify_target_against_full(&self.engine, &self.scene_buf, damage);
                    }
                    _ => {}
                }
            }
        }
        // 时间线：首次提交（CPU 主表面 → SHM/dmabuf；xdg → wgpu present）。
        crate::timeline::note_once("first_commit");
        self.perf_main.record(render_ms, raster_ms, commit_ms);
        if let Some(n) = draws {
            self.perf_main.record_draws(n);
        }
        if let Some(ms) = acquire_ms {
            self.perf_main.record_acquire(ms);
        }
        for ms in gpu_samples {
            self.perf_main.record_gpu(ms);
        }
        // 掉帧计数：主表面本帧已提交（零面积早退分支不计）。参 perf.rs。
        crate::perf::pacing_present_named(&self.pacing_main_name(), Instant::now());
    }

    /// 掉帧计数：声明主表面与各浮层的「动画进行中」状态（App::needs_redraw 语义 = 内容脏 /
    /// 动画推进中）。边沿触发：开始登记、结束落盘一行（未提交过帧则不落）。参 perf.rs。
    fn sync_pacing(&mut self) {
        let now = Instant::now();
        // 判据：本帧确实要渲染（dirty）且 App 报告动画推进中（needs_redraw）——否则 App 可能在
        // 空闲期持续报告 needs_redraw，把十几秒的空闲间隔误记成掉帧。
        let main_active = self.app.needs_redraw() && self.dirty;
        crate::perf::pacing_mark(&self.pacing_main_name(), main_active, now, now);
        for i in 0..self.floating.len() {
            let active = self.app.floating_needs_redraw(i) && self.floating_dirty[i];
            let name = self.pacing_floating_name(i);
            crate::perf::pacing_mark(&name, active, now, now);
        }
    }

    /// 主表面掉帧记录名（角色 + 表面）。
    fn pacing_main_name(&self) -> String {
        format!("{:?}:main", self.role)
    }

    /// 浮层掉帧记录名。
    fn pacing_floating_name(&self, idx: usize) -> String {
        format!("{:?}:floating{idx}", self.role)
    }

    /// 每 10s 且有新帧时把各表面三段耗时 p50/p95/max 追加到持久日志（默认开启）。
    fn maybe_flush_perf(&mut self) {
        self.flush_perf(false);
    }

    /// 刷新 perf 日志（`force = true` 用于外壳退出时无视 10s 节流落盘全部未写数据）。
    fn flush_perf(&mut self, force: bool) {
        let now = Instant::now();
        if !force && now.duration_since(self.perf_flush_at) < crate::perf::FLUSH_INTERVAL {
            return;
        }
        self.perf_flush_at = now;
        let role = format!("{:?}", self.role);
        let proc = self.perf_proc.clone();
        let mut out = String::new();
        if !self.perf_main.is_empty() {
            out.push_str(&crate::perf::format_line(&proc, &role, "main", &self.perf_main));
            self.perf_main.reset();
        }
        for i in 0..self.perf_floating.len() {
            if !self.perf_floating[i].is_empty() {
                let name = format!("floating{i}");
                out.push_str(&crate::perf::format_line(
                    &proc,
                    &role,
                    &name,
                    &self.perf_floating[i],
                ));
                self.perf_floating[i].reset();
            }
        }
        if !self.perf_im_popup.is_empty() {
            out.push_str(&crate::perf::format_line(
                &proc,
                &role,
                "im_popup",
                &self.perf_im_popup,
            ));
            self.perf_im_popup.reset();
        }
        if out.is_empty() {
            return;
        }
        // 首行自证：实际 MSAA 采样数 + GPU 计时是否开启 + one_canvas=on|off（A/B 必须能从日志分辨档位）。
        if !self.perf_header_done {
            self.perf_header_done = true;
            let gpu_ok = self
                .renderer
                .as_ref()
                .map(|r| r.gpu_timing_supported())
                .or_else(|| {
                    self.floating
                        .iter()
                        .find_map(|f| f.renderer.as_ref().map(|r| r.gpu_timing_supported()))
                })
                .or_else(|| {
                    self.im_popup
                        .as_ref()
                        .and_then(|p| p.renderer.as_ref().map(|r| r.gpu_timing_supported()))
                })
                .unwrap_or(false);
            let msaa = self
                .renderer
                .as_ref()
                .map(|r| r.msaa_samples())
                .or_else(|| {
                    self.floating
                        .iter()
                        .find_map(|f| f.renderer.as_ref().map(|r| r.msaa_samples()))
                })
                .or_else(|| {
                    self.im_popup
                        .as_ref()
                        .and_then(|p| p.renderer.as_ref().map(|r| r.msaa_samples()))
                });
            out.insert_str(
                0,
                &crate::perf::format_header(
                    &proc,
                    msaa,
                    gpu_ok,
                    crate::platform::canvas_v2_enabled(),
                    crate::renderer_policy::one_canvas(),
                ),
            );
        }
        if let Some(path) = crate::perf::state_log_path("ether-harness-perf.log") {
            crate::perf::write_log(&path, &out);
        }
    }

    /// 为表面请求下一次提交的 wp_presentation feedback。
    fn request_presentation_feedback(
        &self,
        surface_id: usize,
        surface: &wl_surface::WlSurface,
        qh: &QueueHandle<Self>,
    ) {
        send_presentation_feedback(
            self.presentation.as_ref(),
            self.frame_clock.current_predicted_present(),
            surface_id,
            surface,
            qh,
        );
    }

    /// 请求下一帧 callback（须在 commit 之前，与本次提交对应）。去重：一帧只注册
    /// 一个 callback，`frame_pending` 标记，`CompositorHandler::frame` 到达时清除。
    /// ⚠ 回调仅作 vsync 提示（置 dirty）；丢失不再致命（TOPBAR_RENDER_REFACTOR I-2）。
    fn request_next_frame(&mut self, qh: &QueueHandle<Self>) {
        if self.frame_pending {
            return;
        }
        let s = self.surface.clone();
        s.frame(qh, s.clone());
        self.frame_pending = true;
        self.main_throttle.on_request_callback(Instant::now());
    }

    /// 主表面输入：右键菜单优先路由（参 CONTEXT_MENU_SPEC §Ⅵ.2）→ 未消费才投给 App。
    /// - 菜单关着 + 右键按下 → `App::context_menu(x, y)` 取内容：Some → 开菜单并消费；
    ///   None → 右键照常投递（App 自处理）。
    /// - 菜单开着 → 事件喂状态机（悬停/点选/LightDismiss/Esc/再右键），点选 → `on_context_command`。
    fn emit_input(&mut self, event: InputEvent) {
        // 按键计数：按键按住期间 Move 恒置脏（拖拽/滑杆拖动需逐帧回馈，不套用门控）。
        if let InputEvent::PointerPressed { .. } = &event {
            self.pointer_buttons += 1;
        }
        if let InputEvent::PointerReleased { .. } | InputEvent::PointerLeft = &event {
            self.pointer_buttons = self.pointer_buttons.saturating_sub(1);
        }

        // S1 输入门控：纯 Move 且无按键 → 路由前后比对「悬停语义签名」——
        // 菜单激活时以菜单自身签名为准；否则用 App::hover_signature（None → 旧行为）。
        let is_move = matches!(event, InputEvent::PointerMoved { .. });
        let app_sig_before = if is_move { self.app.hover_signature() } else { None };
        let menu_sig_before = self.ctx_menu.interaction_signature();
        // S4：菜单悬停旧高亮矩形（损坏区 = 旧 ∪ 新，局部重绘）。
        let menu_damage_before = if is_move && self.ctx_menu.is_visible() {
            self.ctx_menu.hovered_rects()
        } else {
            Vec::new()
        };

        let action = {
            // 仅「菜单关着 + 右键按下」才请求 App 内容（&mut app 与 &mut ctx 分开借用）。
            let items = if !self.ctx_menu.is_visible() {
                if let InputEvent::PointerPressed {
                    x, y, button: PointerButton::Right, ..
                } = &event
                {
                    // 错误边界（P0-4）：App 构造菜单时 panic 不得杀掉整个客户端进程。
                    guard("context_menu", || self.app.context_menu(*x, *y)).flatten()
                } else {
                    None
                }
            } else {
                None
            };
            let screen = Rect::new(0.0, 0.0, self.width, self.height);
            self.ctx_menu
                .route_main_event(Some(&self.engine), &event, screen, items)
        };
        match action {
            ContextMenuAction::PassThrough => {
                // 未消费 → 正常投递 App（错误边界隔离）。
                let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.app.handle_input(event);
                }))
                .is_ok();
                if !ok {
                    log::error!("App::handle_input panic，已隔离");
                }
            }
            ContextMenuAction::Consumed => {}
            ContextMenuAction::Activate(path) => {
                let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    self.app.on_context_command(&path);
                }))
                .is_ok();
                if !ok {
                    log::error!("App::on_context_command panic，已隔离");
                }
            }
        }

        // S1 门控判定：纯 Move + 无按键时按签名决定是否置脏（I-4 兜底保留：
        // 输入独立于渲染循环，循环死亡不再导致输入失效 —— 签名无签名app fallback 置脏）。
        if is_move && self.pointer_buttons == 0 {
            let menu_active = self.ctx_menu.is_visible();
            let changed = if menu_active {
                menu_sig_before != self.ctx_menu.interaction_signature()
            } else if let Some(sig0) = app_sig_before {
                sig0 != self.app.hover_signature().unwrap_or(sig0)
            } else {
                true // App 未提供签名 → 保持旧行为（每次 Move 都重绘）。
            };
            if changed {
                self.dirty = true;
                // S4：菜单悬停变化 → 局部损坏 = 旧高亮 ∪ 新高亮（避免残影）。
                if menu_active {
                    for r in menu_damage_before {
                        self.accumulate_damage(r);
                    }
                    for r in self.ctx_menu.hovered_rects() {
                        self.accumulate_damage(r);
                    }
                }
            }
        } else {
            self.dirty = true;
        }
    }

    /// 表面焦点变化通知：`App::focus_changed`（失焦关闭弹层，错误边界隔离）。
    /// 同时关闭 harness 自持的右键菜单（失焦残留：Alt+Tab / 点击其他窗口后菜单不隐）。
    fn notify_focus_changed(&mut self, focused: bool) {
        if !focused {
            self.ctx_menu.close();
        }
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.focus_changed(focused);
        }))
        .is_ok();
        if !ok {
            log::error!("App::focus_changed panic，已隔离");
        }
    }

    /// IME 幂等 reconcile（参 IME_WIRING_PLAN 阶段 D）：
    /// 期望 = `ime_focus_surface && App::ime_focus().is_some()`；与已发送状态不一致才
    /// enable/disable；上下文（周边文本/内容类型/光标矩形）有变化才重灌 + commit。
    ///
    /// 调用点：每帧 `App::update` 后 + wl_keyboard/text-input enter/leave + done 派发后。
    /// 无 text-input 对象（合成器缺 manager）→ 空操作。
    fn reconcile_ime(&mut self) {
        let Some(ti) = self.text_input.clone() else {
            return;
        };
        let focus_control = guard("ime_focus", || self.app.ime_focus())
            .flatten()
            .is_some();
        let want = self.ime_focus_surface && focus_control;

        // 使能状态翻转才发 enable/disable（幂等，避免协议流量）。
        let action = compute_ime_action(self.ime_focus_surface, focus_control, self.ime_enabled);
        if let Some(action) = action {
            self.ime_enabled = matches!(action, ImeAction::Enable);
            match action {
                ImeAction::Enable => ti.enable(),
                ImeAction::Disable => {
                    ti.disable();
                    ti.commit();
                    self.commit_serial += 1;
                    self.ime_context_cache = None;
                    // 失能：清除组合态（App 可能仍显示 preedit）。
                    self.emit_input(InputEvent::Preedit {
                        text: String::new(),
                        cursor_byte: None,
                    });
                    return;
                }
            }
        }

        if !want {
            return;
        }

        // 上下文无变化 → 不重灌（避免每帧 set_surrounding_text + commit）。
        let ctx = guard("ime_focus", || self.app.ime_focus())
            .flatten()
            .unwrap_or_default();
        if self.ime_context_cache.as_ref() == Some(&ctx) {
            return;
        }
        self.ime_context_cache = Some(ctx.clone());
        self.push_ime_context(&ti, &ctx);
        ti.commit();
        self.commit_serial += 1;
    }

    /// 灌 IME 上下文：周边文本（密码不外发）+ 内容类型 + 光标矩形。
    fn push_ime_context(&mut self, ti: &ZwpTextInputV3, ctx: &ImeContext) {
        if !ctx.surrounding_before.is_empty() || !ctx.surrounding_after.is_empty() {
            let text = format!("{}{}", ctx.surrounding_before, ctx.surrounding_after);
            ti.set_surrounding_text(text, ctx.cursor_byte as i32, ctx.anchor_byte as i32);
        }
        // 内容提示 → content_hint / content_purpose（阶段 E：Password 自禁候选窗）。
        let (hint, purpose) = match ctx.content_hint {
            ImeContentHint::Normal => (ContentHint::None, ContentPurpose::Normal),
            ImeContentHint::Password => (
                ContentHint::SensitiveData | ContentHint::HiddenText,
                ContentPurpose::Password,
            ),
            ImeContentHint::Digits => (ContentHint::None, ContentPurpose::Digits),
        };
        ti.set_content_type(hint, purpose);
        let r = ctx.caret_rect;
        ti.set_cursor_rectangle(
            r.origin.x as i32,
            r.origin.y as i32,
            r.size.width as i32,
            r.size.height as i32,
        );
    }

    /// 应用新缩放：重配渲染器物理尺寸 + buffer_scale。
    fn apply_scale(&mut self, scale: f32) {
        if scale <= 0.0 || !scale.is_finite() {
            return;
        }
        self.scale = scale;
        // 图标等应用侧预光栅资产按此缩放出物理像素（参 kanesumi-canvas scale 模块）。
        set_surface_scale(scale);
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, scale);
        }
        if let Some(cpu) = self.cpu.as_mut() {
            cpu.resize(self.width, self.height, scale);
        }
        self.dirty = true; // 缩放变化 → 呈现新尺寸（I-3）。
        if let Some(viewport) = self.viewport.as_ref() {
            viewport.set_destination(
                self.width.round().max(1.0) as i32,
                self.height.round().max(1.0) as i32,
            );
            self.surface.set_buffer_scale(1);
        } else {
            self.surface.set_buffer_scale(scale.round().max(1.0) as i32);
        }
    }

    fn apply_surface_scale(&mut self, surface: &wl_surface::WlSurface, scale: f32) {
        if *surface == self.surface {
            self.apply_scale(scale);
            return;
        }
        if let Some(popup) = self.im_popup.as_mut()
            && popup.surface == *surface
        {
            popup.surface.set_buffer_scale(scale.round().max(1.0) as i32);
            if let Some(cpu) = popup.cpu.as_mut() {
                cpu.resize(popup.width, popup.height, scale);
            }
            if let Some(r) = popup.renderer.as_mut() {
                r.resize(popup.width, popup.height, scale);
            }
            self.im_popup_dirty = true;
            return;
        }
        let Some(index) = self.floating_idx(surface) else {
            return;
        };
        let floating = &mut self.floating[index];
        floating.scale = scale;
        set_surface_scale(scale);
        if let Some(viewport) = floating.viewport.as_ref() {
            viewport.set_destination(
                floating.width.round().max(1.0) as i32,
                floating.height.round().max(1.0) as i32,
            );
            floating.surface.set_buffer_scale(1);
        } else {
            floating
                .surface
                .set_buffer_scale(scale.round().max(1.0) as i32);
        }
        if let Some(cpu) = floating.cpu.as_mut() {
            cpu.resize(floating.width, floating.height, scale);
        }
        if let Some(r) = floating.renderer.as_mut() {
            r.resize(floating.width, floating.height, scale);
        }
    }

    /// 应用新逻辑尺寸。
    fn apply_size(&mut self, width: f32, height: f32) {
        if width > 0.0 {
            self.width = width;
        }
        if height > 0.0 {
            self.height = height;
        }
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale);
        }
        if let Some(cpu) = self.cpu.as_mut() {
            cpu.resize(self.width, self.height, self.scale);
        }
        if let Some(viewport) = self.viewport.as_ref() {
            viewport.set_destination(
                self.width.round().max(1.0) as i32,
                self.height.round().max(1.0) as i32,
            );
        }
    }
}

impl Dispatch<WpFractionalScaleManagerV1, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &WpFractionalScaleManagerV1,
        _event: <WpFractionalScaleManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpFractionalScaleV1, FractionalScaleData> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &WpFractionalScaleV1,
        event: <WpFractionalScaleV1 as Proxy>::Event,
        data: &FractionalScaleData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        if let FractionalScaleEvent::PreferredScale { scale } = event {
            state.apply_surface_scale(&data.surface, scale as f32 / 120.0);
        }
    }
}

impl Dispatch<WpViewporter, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewporter,
        _event: <WpViewporter as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpViewport, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &WpViewport,
        _event: <WpViewport as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PresentationFeedbackData {
    pub surface_id: usize, // 0 = main, 1.. = floating (index + 1)
    pub target: Option<Instant>,
}

fn send_presentation_feedback(
    pres: Option<&wp_presentation::WpPresentation>,
    target: Option<Instant>,
    surface_id: usize,
    surface: &wl_surface::WlSurface,
    qh: &QueueHandle<Shell>,
) {
    if let Some(pres) = pres {
        let data = PresentationFeedbackData {
            surface_id,
            target,
        };
        pres.feedback(surface, qh, data);
    }
}

impl Dispatch<wp_presentation::WpPresentation, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &wp_presentation::WpPresentation,
        _event: <wp_presentation::WpPresentation as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wp_presentation_feedback::WpPresentationFeedback, PresentationFeedbackData> for Shell {
    fn event(
        st: &mut Self,
        _proxy: &wp_presentation_feedback::WpPresentationFeedback,
        ev: <wp_presentation_feedback::WpPresentationFeedback as Proxy>::Event,
        data: &PresentationFeedbackData,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match ev {
            wp_presentation_feedback::Event::Presented {
                tv_sec_hi,
                tv_sec_lo,
                tv_nsec,
                refresh,
                ..
            } => {
                let sec = ((tv_sec_hi as u64) << 32) | (tv_sec_lo as u64);
                let mono = std::time::Duration::new(sec, tv_nsec);
                let presented_at = st.frame_clock.mono_base().instant_of(mono);
                st.frame_clock.on_presented(presented_at, refresh);
                if refresh > 0 {
                    crate::perf::pacing_set_period(std::time::Duration::from_nanos(refresh as u64));
                }
                let err_us = if let Some(target) = data.target {
                    if presented_at >= target {
                        presented_at.duration_since(target).as_secs_f64() * 1_000_000.0
                    } else {
                        target.duration_since(presented_at).as_secs_f64() * 1_000_000.0
                    }
                } else {
                    0.0
                };
                let surface_name = if data.surface_id == 0 {
                    st.pacing_main_name()
                } else {
                    st.pacing_floating_name(data.surface_id - 1)
                };
                crate::perf::pacing_feedback(&surface_name, err_us as f32);
            }
            wp_presentation_feedback::Event::Discarded => {
                st.frame_clock.on_discarded();
            }
            _ => {}
        }
    }
}

// ── CompositorHandler ────────────────────────────────────────────────────

impl CompositorHandler for Shell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if new_factor <= 0 {
            return;
        }
        if *surface == self.surface && self.viewport.is_none() {
            self.apply_surface_scale(surface, new_factor as f32);
        } else if let Some(index) = self.floating_idx(surface)
            && self.floating[index].viewport.is_none()
        {
            self.apply_surface_scale(surface, new_factor as f32);
        }
    }

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // vsync 提示（I-2 / F3 帧时钟采样与节流）：
        // - 记录回调时刻供 FrameClock 预测下一次呈现；
        // - 状态机置位 callback_received 放行下一帧，一个回调最多一帧。
        let now = Instant::now();
        if *surface == self.surface {
            self.frame_pending = false;
            self.frame_clock.on_frame_callback(now);
            self.main_throttle.on_frame_callback();
            self.dirty = self.dirty || self.app.needs_redraw() || !self.coalescer.is_empty();
        } else if let Some(idx) = self.floating_idx(surface) {
            if idx < self.floating_throttle.len() {
                self.floating_throttle[idx].on_frame_callback();
            }
            self.floating_dirty[idx] = self.floating_dirty[idx]
                || self.app.floating_needs_redraw(idx)
                || !self.coalescer.is_empty();
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

// ── OutputHandler ────────────────────────────────────────────────────────

impl OutputHandler for Shell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

// ── WindowHandler（xdg-shell）────────────────────────────────────────────

impl WindowHandler for Shell {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        crate::timeline::note_once("configure_enter");
        let w = configure
            .new_size
            .0
            .map(|v| v.get() as f32)
            .unwrap_or(self.width);
        let h = configure
            .new_size
            .1
            .map(|v| v.get() as f32)
            .unwrap_or(self.height);
        self.apply_size(w, h);
        // 首 configure：延迟创建渲染器（surface 已配置；此前 wgpu 会 SURFACE_LOST）。
        if self.renderer.is_none() && self.cpu.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if !self.configured {
            self.configured = true;
            crate::timeline::note_once("first_configure");
            // 首帧：置脏 → 主循环渲染 + commit（I-1：无条件成功）。
            self.dirty = true;
        } else {
            // 尺寸变化 → 呈现新内容（I-3）。
            self.dirty = true;
        }
    }
}

// ── LayerShellHandler（layer-shell）──────────────────────────────────────

impl LayerShellHandler for Shell {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        // 浮层 surface configure。
        if let Some(idx) = self.floating_idx_by_layer(layer) {
            let (w, h) = configure.new_size;
            // 全屏浮层：合成器 configure 常给高度 0（强制 (lw,0)）→ 用输出逻辑尺寸。
            let out_size = if h == 0 { self.output_logical_size() } else { None };
            let f = &mut self.floating[idx];
            // ⚠ 固定宽度浮层（Dock 右键菜单）保持 spec.width，不用合成器强制宽度
            //   （Ether 合成器对所有 layer surface 强制 (lw,0)）→ 否则菜单被拉成全宽。
            if w > 0 && f.fullscreen {
                f.width = w as f32;
            }
            if h > 0 {
                f.height = h as f32;
            } else if f.fullscreen {
                if let Some((ow, oh)) = out_size {
                    f.width = ow as f32;
                    f.height = oh as f32;
                }
            }
            if let Some(cpu) = f.cpu.as_mut() {
                cpu.resize(f.width, f.height, f.scale);
            }
            if let Some(r) = f.renderer.as_mut() {
                r.resize(f.width, f.height, f.scale);
            }
            if let Some(viewport) = f.viewport.as_ref()
                && f.width > 0.0
                && f.height > 0.0
            {
                viewport.set_destination(
                    f.width.round().max(1.0) as i32,
                    f.height.round().max(1.0) as i32,
                );
            }
            if !f.configured {
                f.configured = true;
                // 首帧：浮层脏 → 主循环渲染 + 提交。
                self.floating_dirty[idx] = true;
                let s = f.surface.clone();
                s.frame(qh, s.clone());
            } else {
                // 尺寸变化 → 呈现新内容（I-3）。
                self.floating_dirty[idx] = true;
            }
            return;
        }
        // 主 surface configure。
        let (w, h) = configure.new_size;
        if w > 0 {
            self.width = w as f32;
        }
        // 优先已主动请求的高度（面板展开/收起无需等合成器 configure 往返，
        // 参旧 topbar.rs pending_height 模式）。
        if let Some(ph) = self.pending_height.take() {
            self.height = ph;
        } else if h > 0 {
            self.height = h as f32;
        }
        // 首 configure：延迟创建渲染器（surface 已配置、尺寸已知）。
        if self.renderer.is_none() && self.cpu.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale);
        }
        if let Some(cpu) = self.cpu.as_mut() {
            cpu.resize(self.width, self.height, self.scale);
        }
        if let Some(viewport) = self.viewport.as_ref() {
            viewport.set_destination(
                self.width.round().max(1.0) as i32,
                self.height.round().max(1.0) as i32,
            );
            self.surface.set_buffer_scale(1);
        } else {
            self.surface
                .set_buffer_scale(self.scale.round().max(1.0) as i32);
        }
        // 首帧或尺寸变化：置脏 → 主循环渲染 + commit（I-1）。
        self.dirty = true;
        if !self.configured {
            self.configured = true;
            crate::timeline::note_once("first_configure");
        }
    }
}

// ── SeatHandler / PointerHandler ─────────────────────────────────────────

impl SeatHandler for Shell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}

    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if self.seat.is_none() {
            self.seat = Some(seat.clone());
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            let pointer = self
                .seat_state
                .get_pointer(qh, &seat)
                .expect("指针设备获取失败");
            self.pointer = Some(pointer);
        }
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            // 绑定键盘（xkbcommon keymap）→ 表面持焦点时 KeyPressed 事件推进。
            if let Ok(keyboard) = self.seat_state.get_keyboard(qh, &seat, None) {
                self.keyboard = Some(keyboard);
            }
            // per-seat text-input 对象（合成器缺 manager 时 None，App 降级裸 KeyPressed）。
            if self.text_input.is_none()
                && let Some(manager) = self.text_input_manager.as_ref()
            {
                let ti = manager.get_text_input(&seat, qh, ());
                self.text_input = Some(ti);
            }
            // per-seat input-method 对象（引擎宿主）+ grab keyboard。
            if self.input_method.is_none()
                && let Some(manager) = self.input_method_manager.as_ref()
            {
                let im = manager.get_input_method(&seat, qh, ());
                // grab_keyboard：引擎接收合成器转发的硬件键盘。
                let grab = im.grab_keyboard(qh, ());
                self.im_keyboard_grab = Some(grab);
                self.input_method = Some(im);
            }
            // per-seat 虚拟键盘（重放未消费按键给焦点客户端）。
            if self.virtual_keyboard.is_none()
                && let Some(manager) = self.vk_manager.as_ref()
            {
                let vk = manager.create_virtual_keyboard(&seat, qh, ());
                self.virtual_keyboard = Some(vk);
            }
        }
    }

    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer
            && let Some(ptr) = self.pointer.take()
        {
            ptr.release();
        }
        if capability == Capability::Keyboard
            && let Some(kb) = self.keyboard.take()
        {
            kb.release();
        }
    }

    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {
    }
}

/// 把 SCTK 的一次轴事件（含 `axis_source` / `axis_stop` / 离散步）汇成外壳 [`InputEvent`]。
///
/// 分流：离散滚轮 → 旧 [`InputEvent::Scroll`]（传统 App 行为不变，元素树 `TreeHost` 等价转
/// `Wheel`/`Update` 平滑）；触控板 / 连续源 → [`InputEvent::ScrollInput`] 携带
/// [`ScrollSource`]（跟手）；`axis_stop`（手指离开）→ `ScrollInput` 的 `End` 阶段（惯性起点）。
/// 返回 `None` = 无位移且非停止，忽略。
///
/// 注：SCTK 0.19 未暴露 `wl_pointer.axis_value120`（高精度滚轮），高精度设备只能经
/// `axis` 的 `absolute` / `discrete` 近似。本文件仅 Linux，**未在 Windows 编译**，待 Arch 验证。
fn axis_to_input(
    horizontal: &AxisScroll,
    vertical: &AxisScroll,
    source: Option<wl_pointer::AxisSource>,
    modifiers: Modifiers,
    step: f32,
) -> Option<InputEvent> {
    let has_discrete = vertical.discrete != 0 || horizontal.discrete != 0;
    // `axis_source` 缺省（None）：带离散步按滚轮，否则按触控板连续。
    let scroll_source = match source {
        Some(wl_pointer::AxisSource::Finger) => ScrollSource::Finger,
        Some(wl_pointer::AxisSource::Continuous) => ScrollSource::Continuous,
        Some(_) => ScrollSource::Wheel { steps: 0.0 },
        None if has_discrete => ScrollSource::Wheel { steps: 0.0 },
        None => ScrollSource::Continuous,
    };
    let scroll_source = match scroll_source {
        ScrollSource::Wheel { .. } => ScrollSource::Wheel {
            steps: vertical.discrete as f32 + horizontal.discrete as f32,
        },
        other => other,
    };
    // 离散步优先（每格 = wheel_lines × 16，正典默认 48px），否则用连续像素。
    let dy = if vertical.discrete != 0 {
        vertical.discrete as f32 * step
    } else {
        vertical.absolute as f32
    };
    let dx = if horizontal.discrete != 0 {
        horizontal.discrete as f32 * step
    } else {
        horizontal.absolute as f32
    };
    if vertical.stop || horizontal.stop {
        return Some(InputEvent::ScrollInput(ScrollInput::end(
            scroll_source,
            modifiers,
        )));
    }
    if dx == 0.0 && dy == 0.0 {
        return None;
    }
    match scroll_source {
        ScrollSource::Wheel { .. } => Some(InputEvent::Scroll {
            x: dx,
            y: dy,
            modifiers,
        }),
        _ => Some(InputEvent::ScrollInput(ScrollInput::new(
            dx,
            dy,
            scroll_source,
            ScrollPhase::Update,
            modifiers,
        ))),
    }
}

impl PointerHandler for Shell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let pos = (event.position.0 as f32, event.position.1 as f32);
            if let PointerEventKind::Press { serial, .. } = &event.kind {
                self.last_input_serial = Some(*serial);
            }
            // 子弹层表面：事件原样（弹层局部坐标）交给 App，App 平移回树坐标。
            if let Some(key) = self.popup_key(&event.surface) {
                let ev = match &event.kind {
                    PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                        Some(InputEvent::PointerMoved { x: pos.0, y: pos.1 })
                    }
                    PointerEventKind::Leave { .. } => Some(InputEvent::PointerLeft),
                    PointerEventKind::Press { button, .. } => Some(InputEvent::PointerPressed {
                        x: pos.0,
                        y: pos.1,
                        button: map_button(*button),
                        modifiers: self.modifiers,
                    }),
                    PointerEventKind::Release { button, .. } => Some(InputEvent::PointerReleased {
                        x: pos.0,
                        y: pos.1,
                        button: map_button(*button),
                        modifiers: self.modifiers,
                    }),
                    PointerEventKind::Axis {
                        vertical,
                        horizontal,
                        source,
                        ..
                    } => {
                        let step = self.interaction.wheel_step_px();
                        axis_to_input(horizontal, vertical, *source, self.modifiers, step)
                    }
                };
                if let Some(ev) = ev {
                    self.emit_popup_input(key, ev);
                }
                continue;
            }
            // 按指针所在表面路由：主表面 / 浮层。
            let target = if event.surface == self.surface {
                None
            } else {
                self.floating_idx(&event.surface)
            };
            match &event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_pos = pos;
                    self.route_input(target, InputEvent::PointerMoved { x: pos.0, y: pos.1 });
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = (-1.0, -1.0);
                    // 离开表面复位双击跟踪（跨表面快速点击不算双击）。
                    self.click_tracker.reset();
                    self.route_input(target, InputEvent::PointerLeft);
                }
                PointerEventKind::Press { time, button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.route_input(
                        target,
                        InputEvent::PointerPressed {
                            x: pos.0,
                            y: pos.1,
                            button,
                            modifiers: self.modifiers,
                        },
                    );
                    // 双击判定：第二次快速按下追加 DoubleClick（Press 照常投递，单击语义不丢）。
                    if self.click_tracker.record(button, pos.0, pos.1, *time) {
                        self.route_input(
                            target,
                            InputEvent::DoubleClick {
                                x: pos.0,
                                y: pos.1,
                                button,
                                modifiers: self.modifiers,
                            },
                        );
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.route_input(
                        target,
                        InputEvent::PointerReleased {
                            x: pos.0,
                            y: pos.1,
                            button,
                            modifiers: self.modifiers,
                        },
                    );
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    source,
                    ..
                } => {
                    // 滚轮 / 触控板分流见 `axis_to_input`：正方向 = +y（表面坐标，下为正）。
                    let step = self.interaction.wheel_step_px();
                    if let Some(ev) =
                        axis_to_input(horizontal, vertical, *source, self.modifiers, step)
                    {
                        self.pointer_pos = pos;
                        self.route_input(target, ev);
                    }
                }
            }
        }
        // 指针事件很大一部分是纯 Move —— 脏标记由 emit_input 的 S1 门控决定
        // （签名变化才置位），此处不再无条件置脏。
    }
}

/// 创建浮层 layer-shell surface。排他区域 -1（Neutral，不占工作区）。
fn create_floating_surface(
    compositor_state: &CompositorState,
    shell: &LayerShell,
    qh: &QueueHandle<Shell>,
    spec: FloatingLayer,
    fractional_scale_manager: Option<&WpFractionalScaleManagerV1>,
    viewporter: Option<&WpViewporter>,
) -> Result<FloatingSurface, String> {
    let layer = match spec.layer {
        LayerKind::Top => Layer::Top,
        LayerKind::Overlay => Layer::Overlay,
    };
    let anchor = match spec.anchor {
        AnchorKind::TopRight => Anchor::TOP | Anchor::RIGHT,
        AnchorKind::TopLeft => Anchor::TOP | Anchor::LEFT,
        AnchorKind::BottomCenter => Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        AnchorKind::Fullscreen => Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
        AnchorKind::Bottom => Anchor::BOTTOM,
    };
    let surface = compositor_state.create_surface(qh);
    let fractional_supported = fractional_scale_manager.is_some() && viewporter.is_some();
    let fractional_scale = fractional_supported.then(|| {
        fractional_scale_manager.unwrap().get_fractional_scale(
            &surface,
            qh,
            FractionalScaleData {
                surface: surface.clone(),
            },
        )
    });
    let viewport = fractional_supported.then(|| viewporter.unwrap().get_viewport(&surface, qh, ()));
    if viewport.is_some() {
        surface.set_buffer_scale(1);
    }
    let ls = shell.create_layer_surface(qh, surface.clone(), layer, Some(spec.app_id), None);
    ls.set_anchor(anchor);
    // 上边距：让 Top 锚定浮层贴 TopBar 下边（不盖住 bar）。参 FloatingLayer::top_margin。
    if spec.top_margin > 0.0 {
        ls.set_margin(spec.top_margin.round() as i32, 0, 0, 0);
    }
    // ⚠ exclusive_zone 仅全屏表面（四边锚定）可用 -1；部分表面（如固定宽度右键菜单）
    //   设 -1 会触发 zwlr_layer_surface_v1 ERROR_INVALID_SURFACE_STATE（协议错误 → 客户端
    //   被杀）。非全屏浮层用 0（不占排他区域）。
    if matches!(spec.anchor, AnchorKind::Fullscreen) {
        ls.set_exclusive_zone(-1);
    } else {
        ls.set_exclusive_zone(0);
    }
    // ⚠ 高度 0 仅在「上下同时锚定」（全屏）合法；Bottom-only 浮层（右键菜单）设 0 →
    //   zwlr_layer_surface ERROR_INVALID_SURFACE_STATE。非全屏浮层初始高度至少 1。
    let init_h = if matches!(spec.anchor, AnchorKind::Fullscreen) {
        spec.height
    } else {
        spec.height.max(1.0)
    };
    ls.set_size(spec.width as u32, init_h as u32);
    ls.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    ls.commit();
    Ok(FloatingSurface {
        surface,
        layer_surface: ls,
        cpu: None,
        renderer: None,
        width: spec.width,
        height: spec.height,
        configured: false,
        scale: 1.0,
        fullscreen: matches!(spec.anchor, AnchorKind::Fullscreen),
        _fractional_scale: fractional_scale,
        viewport,
    })
}

/// Wayland 按钮 → PointerButton。
fn map_button(button: u32) -> PointerButton {
    match button {
        BTN_LEFT => PointerButton::Left,
        BTN_RIGHT => PointerButton::Right,
        BTN_MIDDLE => PointerButton::Middle,
        _ => PointerButton::Left,
    }
}

// ── KeyboardHandler ────────────────────────────────────────────────────

impl KeyboardHandler for Shell {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        // 表面获得键盘焦点 → text-input 焦点标记 + 幂等 reconcile。
        self.ime_focus_surface = true;
        self.reconcile_ime();
        // App 通知：获焦（关闭弹层路径之外的正向通知）。
        self.notify_focus_changed(true);
        self.dirty = true;
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        self.ime_focus_surface = false;
        self.pending_ime = PendingImeBatch::default();
        self.reconcile_ime();
        // 键盘焦点移进本进程的抓取弹层（合成器在 grab 时转移焦点）→ 不是失焦，弹层不能因此关闭。
        if *surface == self.surface && self.has_grabbing_popup() {
            self.dirty = true;
            return;
        }
        // App 通知：失焦 → 关闭右键菜单 / 弹窗（失焦残留修复）。
        self.notify_focus_changed(false);
        self.dirty = true;
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        serial: u32,
        event: SctkKeyEvent,
    ) {
        self.last_input_serial = Some(serial);
        let text = event
            .utf8
            .as_deref()
            .filter(|text| text.chars().count() > 1)
            .map(str::to_owned);
        let key = map_key(event.keysym, if text.is_some() { None } else { event.utf8 });
        // Tab / Shift+Tab：框架级焦点推进（App 可选实现 `App::focus_move`）。
        // 仅在 IME 未接管键盘时抢路由 —— 否则会破坏输入法内的候选选择。
        if key == Key::Tab && !self.ime_enabled {
            let backward = self.modifiers.shift;
            let consumed = guard("focus_move", || self.app.focus_move(backward)).unwrap_or(false);
            if consumed {
                self.dirty = true;
                return;
            }
        }
        self.emit_input(InputEvent::KeyPressed {
            key,
            modifiers: self.modifiers,
        });
        // xkb 可产生多标量文本（组合序列等）；不能静默丢掉首字符后的内容。
        if let Some(text) = text
            && !self.ime_enabled
        {
            self.emit_input(InputEvent::Commit { text });
        }
        self.dirty = true;
    }

    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: SctkKeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        modifiers: smithay_client_toolkit::seat::keyboard::Modifiers,
        _layout: u32,
    ) {
        // 缓存修饰键状态，注入后续输入事件（App 据此组合快捷键/范围选）。
        self.modifiers = Modifiers {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            super_key: modifiers.logo,
        };
    }

    fn update_repeat_info(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _repeat: smithay_client_toolkit::seat::keyboard::RepeatInfo,
    ) {
    }
}

/// Keysym + utf8 → 逻辑键。控制键优先（Backspace 的 utf8 是控制字符，不能当 Char）；
/// 其余可打印键走 utf8 字符（含 shift 符号 / 小键盘）。未分类透传原始 keysym。
///
/// Ctrl 组合的 utf8 是控制字符（Ctrl+F → `\u{6}`、Ctrl+Space → `\u{0}`），而 `Char`
/// 变体语义是可打印字符（参 DEV_GUIDE.md §2.6）：此时回退 keysym 的可打印字符，
/// 修饰键状态由 `Modifiers` 另行携带；不回退会使全部 Ctrl+字母 加速键（以及
/// ceyboard 的 Ctrl+Space 切换）失配（2026-10-02 l3 报告）。
///
/// 空格键**不**走具名变体：其 utf8 是 `" "`，必须落 `Char(' ')` 才能让 TextBox 与
/// 所有「Space 激活」控件照旧工作（参 `Key::Space` 注释）。PageUp/PageDown/Insert/
/// F1..F12 无 utf8，由本表语义化；F13+ 落 `Unknown`。
pub(crate) fn map_key(keysym: Keysym, utf8: Option<String>) -> Key {
    use xkeysym::key;
    match keysym.raw() {
        key::Return | key::KP_Enter => return Key::Enter,
        key::BackSpace => return Key::Backspace,
        key::Escape => return Key::Escape,
        key::Tab => return Key::Tab,
        key::Insert => return Key::Insert,
        key::Delete => return Key::Delete,
        key::Home => return Key::Home,
        key::End => return Key::End,
        key::Page_Up => return Key::PageUp,
        key::Page_Down => return Key::PageDown,
        key::Left => return Key::Left,
        key::Right => return Key::Right,
        key::Up => return Key::Up,
        key::Down => return Key::Down,
        key::F1 => return Key::F(1),
        key::F2 => return Key::F(2),
        key::F3 => return Key::F(3),
        key::F4 => return Key::F(4),
        key::F5 => return Key::F(5),
        key::F6 => return Key::F(6),
        key::F7 => return Key::F(7),
        key::F8 => return Key::F(8),
        key::F9 => return Key::F(9),
        key::F10 => return Key::F(10),
        key::F11 => return Key::F(11),
        key::F12 => return Key::F(12),
        _ => {}
    }
    if let Some(c) = utf8.and_then(|s| s.chars().next()) {
        if c.is_control()
            && let Some(printable) = keysym.key_char()
            && !printable.is_control()
        {
            return Key::Char(printable);
        }
        return Key::Char(c);
    }
    Key::Unknown(keysym.raw())
}

// ── 委派宏 ───────────────────────────────────────────────────────────────

delegate_compositor!(Shell);
delegate_output!(Shell);
delegate_seat!(Shell);
delegate_pointer!(Shell);
delegate_keyboard!(Shell);
delegate_registry!(Shell);
delegate_xdg_shell!(Shell);
delegate_xdg_window!(Shell);
smithay_client_toolkit::delegate_xdg_popup!(Shell);
delegate_layer!(Shell);
delegate_dmabuf!(Shell);
smithay_client_toolkit::delegate_subcompositor!(Shell);

impl ProvidesRegistryState for Shell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

// ── 客户端 dmabuf（linux-dmabuf-v1）────
// create_immed 产出的 wl_buffer release → 合成器用毕，标记 dmabuf 槽可复用。
// 参 LINUX_DMABUF_PLAN §3 客户端端。
impl DmabufHandler for Shell {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf
    }

    fn dmabuf_feedback(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _proxy: &wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_feedback_v1::ZwpLinuxDmabufFeedbackV1,
        feedback: smithay_client_toolkit::dmabuf::DmabufFeedback,
    ) {
        // M5：合成器主设备 + 格式表协商（多 GPU 安全 + 免 R/B 交换的快路径）。
        // 客户端据此 ① 优先打开与主设备同 dev_t 的 DRM 节点；② 只在广告表内选 fourcc，
        // 未广告 → 回退 SHM（避免合成器导入失败导致表面不可见）。
        let dev = feedback.main_device();
        let formats: Vec<(u32, u64)> = feedback
            .format_table()
            .iter()
            .map(|f| (f.format, f.modifier))
            .collect();
        log::info!(
            "dmabuf feedback：main_device=0x{dev:x} 格式表 {} 项",
            formats.len()
        );
        self.dmabuf_feedback = Some((dev, formats.clone()));
        self.main_out.set_feedback(dev, &formats);
        for out in self.floating_out.iter_mut() {
            out.set_feedback(dev, &formats);
        }
        if let Some(popup) = self.im_popup.as_mut() {
            popup.out.set_feedback(dev, &formats);
        }
    }

    fn created(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _params: &wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
        _buffer: wl_buffer::WlBuffer,
    ) {
        // 仅在异步 create（非 create_immed）路径触发；我们只用 create_immed，不处理。
    }

    fn failed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _params: &wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1,
    ) {
        // 合成器拒绝了该缓冲（格式/修饰符/设备不匹配）→ 本进程 dmabuf 判定不可用，
        // **所有表面立即回退 SHM**（无法归属到具体 params，取保守全局降级），不丢帧。
        log::warn!("dmabuf 被合成器拒绝 → 全表面回退 SHM（ETHER_DMABUF=0 可显式回退）");
        self.dmabuf_allowed = false;
        self.main_out.dmabuf.mark_unavailable();
        for out in self.floating_out.iter_mut() {
            out.dmabuf.mark_unavailable();
        }
        if let Some(popup) = self.im_popup.as_mut() {
            popup.out.dmabuf.mark_unavailable();
        }
    }

    fn released(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        buffer: &wl_buffer::WlBuffer,
    ) {
        if self.main_out.mark_released(buffer) {
            return;
        }
        for out in self.floating_out.iter_mut() {
            if out.mark_released(buffer) {
                return;
            }
        }
        if self.popup_buffer_released(buffer) {
            return;
        }
        if let Some(popup) = self.im_popup.as_mut() {
            popup.out.mark_released(buffer);
        }
    }
}

// 无主用的 wl_region 事件（避免缺 Dispatch）。
impl wayland_client::Dispatch<wayland_client::protocol::wl_region::WlRegion, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &wayland_client::protocol::wl_region::WlRegion,
        _event: wayland_client::protocol::wl_region::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// ── IME（zwp_text_input_v3，参 IME_WIRING_PLAN 阶段 D） ────────────────────

// manager 无事件，仅保证对象存活。
impl Dispatch<ZwpTextInputManagerV3, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpTextInputManagerV3,
        _event: wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpTextInputV3, ()> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &ZwpTextInputV3,
        event: TextInputEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            // 表面获得/失去 text-input 焦点 → 幂等 reconcile。
            TextInputEvent::Enter { surface } => {
                if surface == state.surface {
                    state.ime_focus_surface = true;
                }
                state.reconcile_ime();
            }
            TextInputEvent::Leave { .. } => {
                state.ime_focus_surface = false;
                state.pending_ime = PendingImeBatch::default();
                // 协议要求 leave 时重置 preedit（清 App 组合态）。
                state.emit_input(InputEvent::Preedit {
                    text: String::new(),
                    cursor_byte: None,
                });
                state.reconcile_ime();
            }
            // done 前累积进 pending 批。
            TextInputEvent::PreeditString {
                text,
                cursor_begin,
                cursor_end,
            } => {
                state.pending_ime.preedit = text;
                state.pending_ime.cursor_begin = cursor_begin;
                state.pending_ime.cursor_end = cursor_end;
            }
            TextInputEvent::CommitString { text } => {
                state.pending_ime.commit = text;
            }
            TextInputEvent::DeleteSurroundingText {
                before_length,
                after_length,
            } => {
                state.pending_ime.delete_before = before_length;
                state.pending_ime.delete_after = after_length;
            }
            TextInputEvent::Done { serial } => {
                // 仅 serial 匹配生效（stale 帧丢弃，参 IME_WIRING_PLAN 风险 1）。
                match state.pending_ime.apply_done(serial, state.commit_serial) {
                    Some(events) => {
                        for ev in events {
                            state.emit_input(ev);
                        }
                    }
                    None => {
                        log::debug!(
                            "text-input done serial {serial}（当前 {}）stale，丢弃",
                            state.commit_serial
                        );
                    }
                }
                state.pending_ime = PendingImeBatch::default();
                // 派发后文本/光标已变 → 重新灌上下文。
                state.reconcile_ime();
            }
            _ => {}
        }
    }
}

// ── IME 引擎宿主（zwp_input_method_v2，Ceyboard 作为引擎）。参 CEYBOARD_SPEC §Ⅴ ─────

// manager 无事件，仅保证对象存活。
impl Dispatch<ZwpInputMethodManagerV2, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpInputMethodManagerV2,
        _event: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_method_manager_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpInputMethodV2, ()> for Shell {
    fn event(
        state: &mut Self,
        proxy: &ZwpInputMethodV2,
        event: ImEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        match event {
            // 文本字段获焦 → 引擎激活；重置组合态缓存。
            ImEvent::Activate => {
                log::info!("IME 引擎激活（文本字段获焦，im_active=true）");
                state.im_active = true;
                state.im_preedit_cache = None;
                state.emit_input(InputEvent::Preedit {
                    text: String::new(),
                    cursor_byte: None,
                });
                state.flush_engine(proxy);
            }
            // 失焦 → 引擎失活；清组合态。
            ImEvent::Deactivate => {
                log::info!("IME 引擎失活（im_active=false）");
                state.im_active = false;
                state.im_preedit_cache = None;
                state.emit_input(InputEvent::Preedit {
                    text: String::new(),
                    cursor_byte: None,
                });
            }
            // done 事件：serial 计数（commit 请求需回传该 serial）。
            ImEvent::Done => {
                state.im_done_serial += 1;
            }
            // 周边文本缓存（退格字符边界用：delete_surrounding_text 按字节，
            // CJK 字符 3 字节，须整字符删避免劈码点）。
            ImEvent::SurroundingText {
                text,
                cursor,
                anchor,
            } => {
                state.im_surrounding = Some((text, cursor, anchor));
            }
            _ => {}
        }
    }
}

/// 抓取按键是否原样放行（不经引擎）。纯函数，便于回归测试。
///
/// `im_active == false`（焦点表面 text-input 未启用）或 keymap 未建立时必须放行：
/// 合成器在 grab 有效期内可能仍转发按键（防御：门控未生效 / 旧合成器），若吞掉则整个
/// 键盘静默失效（参 docs/CEYBOARD_FIX_2026-09-18.md）。放行路径走虚拟键盘重放，
/// **绝不进入 `ime_engine_key`**，故不会触发 Shift 点按判定等引擎逻辑。
fn grab_key_passthrough(im_active: bool, has_keymap: bool) -> bool {
    !im_active || !has_keymap
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &ZwpInputMethodKeyboardGrabV2,
        event: ImGrabEvent,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            ImGrabEvent::Keymap { format, fd, size } => {
                // xkbcommon keymap → ImXkb（key 事件语义化）。
                use wayland_client::WEnum;
                let fmt_ok = match format {
                    WEnum::Value(f) => {
                        matches!(f, wayland_client::protocol::wl_keyboard::KeymapFormat::XkbV1)
                    }
                    _ => false,
                };
                if !fmt_ok {
                    log::warn!("input-method keymap 格式非 xkb_v1，忽略");
                    return;
                }
                // 虚拟键盘须先有 keymap 才能重放按键（协议要求）——把同一 keymap 也发过去。
                if let Some(vk) = state.virtual_keyboard.clone() {
                    use std::os::fd::AsFd;
                    if let Ok(fd_clone) = fd.try_clone() {
                        vk.keymap(1, fd_clone.as_fd(), size);
                    }
                }
                match ImXkb::from_keymap(fd, size as usize) {
                    Ok(xkb) => state.im_xkb = Some(xkb),
                    Err(e) => log::warn!("input-method keymap 建立失败：{e}"),
                }
            }
            ImGrabEvent::Key {
                key,
                state: kstate,
                ..
            } => {
                // 合成器转发的硬件按键 → 引擎。仅按下（state==Pressed）时处理。
                use wayland_client::WEnum;
                let is_pressed = matches!(
                    kstate,
                    WEnum::Value(wayland_client::protocol::wl_keyboard::KeyState::Pressed)
                );
                if !is_pressed {
                    // 松开事件：引擎激活期交给 App —— Shift 点按在**松开时**判定
                    // （按住期间无其它键、且按住 < 300 ms 才切中英，参 docs/IME_PLAN.md §Ⅱ-2）。
                    // 非激活期与按下同样门控，不介入（IME1）。松开不重放：按下时已随键透传。
                    if !grab_key_passthrough(state.im_active, state.im_xkb.is_some())
                        && let Some(xkb) = state.im_xkb.as_ref()
                    {
                        let (sym, utf8) = xkb.keycode_to_sym(key);
                        let logical = map_key(xkeysym::Keysym::new(sym), utf8);
                        let _ = guard("ime_engine_key_release", || {
                            state.app.ime_engine_key_release(logical, state.im_modifiers)
                        });
                    }
                    return;
                }
                // ⚠ 引擎不可用（文本字段未聚焦 / keymap 未建立）时**绝不吞键**：直接经虚拟
                // 键盘重放给焦点客户端。旧实现在此 `return`，而合成器在 grab 有效期内**始终**
                // 把按键转发给 IME（smithay `active_text_input_serial_or_default` 恒回调）→
                // IME 一失活整个键盘静默失效（2026-09-18 Ceyboard 故障复盘发现）。
                // 安全性：本事件只可能来自 grab（客户端此时收不到原始按键），故重放不会重复投递。
                if grab_key_passthrough(state.im_active, state.im_xkb.is_some()) {
                    // 零交互排障留痕：非激活期按键只应来自「门控未生效 / 旧合成器」，
                    // 记录证明放行而非吞键（参 docs/IME_PLAN.md §Ⅲ IME1）。
                    log::debug!("IME 抓取按键 key={key} im_active={} → 放行", state.im_active);
                    if let Some(vk) = state.virtual_keyboard.clone() {
                        vk.key(state.im_key_time, key, 1); // 按下
                        vk.key(state.im_key_time, key, 0); // 释放（透传完整按键）
                    }
                    state.im_key_time += 1;
                    return;
                }
                let Some(xkb) = state.im_xkb.as_ref() else {
                    return;
                };
                let (sym, utf8) = xkb.keycode_to_sym(key);
                let logical = map_key(xkeysym::Keysym::new(sym), utf8);
                log::debug!("IME 抓取按键 key={key} im_active=true → 引擎");
                // 引擎处理按键 → 更新 preedit/commit，随即 flush 上屏。
                // 返回 false = 引擎未消费 → 经虚拟键盘重放给焦点客户端（fcitx5 同款
                // 透传：arrow/backspace/Home 等导航键须到焦点应用）。
                let consumed = guard("ime_engine_key", || {
                    state.app.ime_engine_key(logical, state.im_modifiers)
                })
                .unwrap_or(false);
                if let Some(im) = state.input_method.clone() {
                    state.flush_engine(&im);
                }
                if !consumed
                    && let Some(vk) = state.virtual_keyboard.clone()
                {
                    vk.key(state.im_key_time, key, 1); // 按下
                    vk.key(state.im_key_time, key, 0); // 释放（透传完整按键）
                }
                state.im_key_time += 1;
                // 候选窗 popup 刷新（key 即时置脏，下一帧 refresh 提交）。
                state.im_popup_dirty = true;
                state.refresh_im_popup(qh);
            }
            ImGrabEvent::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(xkb) = state.im_xkb.as_mut() {
                    xkb.update_mask(mods_depressed, mods_latched, mods_locked, group);
                }
                state.im_modifiers = Modifiers {
                    ctrl: mods_depressed & 4 != 0,   // Control_L = 0x04
                    alt: mods_depressed & 8 != 0,    // Alt_L = 0x08
                    shift: mods_depressed & 1 != 0,  // Shift_L = 0x01
                    super_key: mods_depressed & 64 != 0, // Super_L = 0x40
                };
                // 同步修饰键到虚拟键盘（重放的组合键如 Ctrl+C 须带修饰状态）。
                if let Some(vk) = state.virtual_keyboard.clone() {
                    vk.modifiers(mods_depressed, mods_latched, mods_locked, group);
                }
            }
            _ => {}
        }
    }
}

// popup surface：接收 text_input_rectangle（光标矩形提示）。无请求需处理。
impl Dispatch<ZwpInputPopupSurfaceV2, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpInputPopupSurfaceV2,
        _event: wayland_protocols_misc::zwp_input_method_v2::client::zwp_input_popup_surface_v2::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

// 虚拟键盘 manager / 对象：无事件需处理，仅保证存活。
impl Dispatch<ZwpVirtualKeyboardManagerV1, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardManagerV1,
        _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwpVirtualKeyboardV1, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &ZwpVirtualKeyboardV1,
        _event: wayland_protocols_misc::zwp_virtual_keyboard_v1::client::zwp_virtual_keyboard_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Shell {
    /// 主动绑定引擎宿主：遍历已有 seat，创建 input-method 对象 + grab keyboard + 虚拟键盘。
    /// 幂等（input_method.is_none 才建）。用于绕过 new_capability 竞态（ceyboard 连接时
    /// seat keyboard 能力可能已就绪，能力事件不触发 → grab 未建立 → 键收不到）。
    fn ensure_ime_engine(&mut self, qh: &QueueHandle<Self>) {
        if self.input_method.is_some() && self.virtual_keyboard.is_some() {
            return;
        }
        for seat in self.seat_state.seats() {
            if self.input_method.is_none()
                && let Some(manager) = self.input_method_manager.as_ref()
            {
                let im = manager.get_input_method(&seat, qh, ());
                let grab = im.grab_keyboard(qh, ());
                self.im_keyboard_grab = Some(grab);
                self.input_method = Some(im);
            }
            if self.virtual_keyboard.is_none()
                && let Some(manager) = self.vk_manager.as_ref()
            {
                let vk = manager.create_virtual_keyboard(&seat, qh, ());
                self.virtual_keyboard = Some(vk);
            }
            if self.input_method.is_some() && self.virtual_keyboard.is_some() {
                break;
            }
        }
    }

    /// 引擎宿主 flush：把 App 引擎的 preedit / commit / delete 通过 input-method-v2 上屏。
    /// 幂等：preedit 无变化不重发 set_preedit_string（避免光标抖动）。
    fn flush_engine(&mut self, im: &ZwpInputMethodV2) {
        // 1. 待提交文本（选词/空格/回车）。
        let mut committed = false;
        loop {
            let Some(text) = guard("ime_engine_take_commit", || self.app.ime_engine_take_commit())
                .flatten()
            else {
                break;
            };
            im.commit_string(text);
            committed = true;
        }
        // 2. 待删除周边文本（退格）。double-buffered → 须 commit 才生效。
        //    App 以「字符数」请求（before=1 = 删光标前一字符）；协议按字节，
        //    CJK 字符 3 字节，据周边文本缓存换算字节数（整字符删，不劈码点）。
        let Some((before_chars, after_chars)) =
            guard("ime_engine_take_delete", || self.app.ime_engine_take_delete())
        else {
            return;
        };
        if before_chars > 0 || after_chars > 0 {
            let before_bytes = self.chars_to_bytes_before(before_chars);
            let after_bytes = self.chars_to_bytes_after(after_chars);
            im.delete_surrounding_text(before_bytes, after_bytes);
            committed = true;
        }
        // 3. 组合态 preedit（变化才发）。double-buffered → 变化时 commit 才生效。
        let Some((preedit, cursor_byte)) =
            guard("ime_engine_preedit", || self.app.ime_engine_preedit())
        else {
            return;
        };
        let cursor = cursor_byte.map(|c| c as i32).unwrap_or(-1);
        if self.im_preedit_cache.as_deref() != Some(preedit.as_str()) {
            im.set_preedit_string(preedit.clone(), cursor, cursor);
            self.im_preedit_cache = Some(preedit);
            committed = true;
        }
        // 4. 提交（serial = 已收到 done 数）。
        if committed {
            im.commit(self.im_done_serial);
        }
    }

    /// 光标前 `n` 字符 → 字节数（据周边文本缓存）。缺缓存时退化为 n 字节。
    fn chars_to_bytes_before(&self, n: u32) -> u32 {
        let Some((text, cursor, _anchor)) = self.im_surrounding.as_ref() else {
            return n;
        };
        // 夹到字符边界：客户端给的光标可能是字节偏移且落在多字节字符中间（P0-4）。
        let cursor = floor_char_boundary(text, (*cursor as usize).min(text.len()));
        let prefix = &text[..cursor];
        let n = (n as usize).min(prefix.chars().count());
        // 取前缀最后 n 字符的字节长度。
        let chars: Vec<char> = prefix.chars().collect();
        let len = chars.len();
        chars[len - n..].iter().map(|c| c.len_utf8()).sum::<usize>() as u32
    }

    /// 光标后 `n` 字符 → 字节数。
    fn chars_to_bytes_after(&self, n: u32) -> u32 {
        let Some((text, cursor, _anchor)) = self.im_surrounding.as_ref() else {
            return n;
        };
        let cursor = floor_char_boundary(text, (*cursor as usize).min(text.len()));
        let suffix = &text[cursor..];
        let n = (n as usize).min(suffix.chars().count());
        suffix.chars().take(n).map(|c| c.len_utf8()).sum::<usize>() as u32
    }

    /// 候选窗 popup surface 每帧刷新：按引擎状态建/调整 surface，渲染候选窗 Scene 提交。
    /// 合成器把 `zwp_input_popup_surface_v2` 渲染到 Layer 6 Overlay（跟随光标）。
    fn refresh_im_popup(&mut self, qh: &QueueHandle<Self>) {
        let Some((pw, ph)) = guard("ime_engine_popup_size", || self.app.ime_engine_popup_size())
        else {
            return;
        };
        let active = self.im_active && pw > 0.0 && ph > 0.0;
        let has_input_method = self.input_method.is_some();

        // 引擎失活 / 无 popup 内容 → 释放 popup surface。
        if !active || !has_input_method {
            if let Some(popup) = self.im_popup.take() {
                // popup surface 无独立 destroy；wl_surface 销毁即消失。
                popup.surface.destroy();
                log::info!("IME 候选窗 popup 已释放");
            }
            return;
        }

        // 尺寸变化或首次 → 重建 popup surface（wl_surface 尺寸由 SHM buffer 决定）。
        // ⚠ CPU 光栅器不复建（resize 复用），避免候选内容变化时 surface 重建闪烁。
        let size_changed = self
            .im_popup
            .as_ref()
            .map(|p| (p.width - pw).abs() > 0.5 || (p.height - ph).abs() > 0.5)
            .unwrap_or(true);
        if size_changed && self.im_popup.is_some() {
            // 仅更新记录尺寸，不 destroy/recreate（surface 尺寸由 SHM buffer 驱动，
            // CPU 光栅器 resize 即可）。
            if let Some(p) = self.im_popup.as_mut() {
                p.width = pw;
                p.height = ph;
            }
            self.im_popup_dirty = true; // 尺寸变化须重渲染提交。
        }
        if self.im_popup.is_none() {
            let Some(im) = self.input_method.clone() else {
                return;
            };
            let surface = self.compositor_state.create_surface(qh);
            // SHM buffer 按合成器 scale 渲染（物理像素），须声明 buffer_scale 让合成器
            // 按 1x 逻辑缩放 —— 缺失则 HiDPI 下候选窗错位/模糊。
            surface.set_buffer_scale(self.scale.round().max(1.0) as i32);
            let popup = im.get_input_popup_surface(&surface, qh, ());
            log::info!("IME 候选窗 popup 创建：{pw:.0}×{ph:.0} scale={}", self.scale);
            let mut out =
                SurfaceOutput::new(self.dmabuf_allowed, self.dmabuf_device.clone());
            if let Some((dev, formats)) = self.dmabuf_feedback.clone() {
                out.set_feedback(dev, &formats);
            }
            self.im_popup = Some(ImPopupSurface {
                surface: surface.clone(),
                popup,
                cpu: None,
                renderer: None,
                out,
                width: pw,
                height: ph,
            });
            self.im_popup_dirty = true; // 新 surface 首帧须渲染。
        }

        let needs_init = self
            .im_popup
            .as_ref()
            .map(|p| p.cpu.is_none() && p.renderer.is_none())
            .unwrap_or(false);
        if needs_init {
            let surf = self.im_popup.as_ref().unwrap().surface.clone();
            let class = SurfaceClass::ImePopup;
            let area = ((pw * self.scale).max(0.0) * (ph * self.scale).max(0.0)) as u64;
            let hz = default_expected_hz(class);
            let gpu_ok = self.gpu_available();
            let one_canvas = crate::renderer_policy::one_canvas();
            let kind = choose_renderer(class, area, hz, gpu_ok, one_canvas);
            log::info!(
                "渲染器选择：候选窗（{:.0}x{:.0}，scale {}）area={}px² hz={:.1} → {:?}（{}）",
                pw,
                ph,
                self.scale,
                area,
                hz,
                kind,
                crate::renderer_policy::decision_reason(class, area, hz, gpu_ok, one_canvas),
            );
            if kind == RendererKind::Gpu {
                if let Some(ctx) = self.ensure_gpu_context(&surf) {
                    match SurfaceRenderer::with_context(
                        ctx,
                        &self.conn,
                        &surf,
                        pw,
                        ph,
                        self.scale,
                        true,
                    ) {
                        Ok(mut r) => {
                            // 预热：空场景提交一次，强制管线/着色器编译。
                            r.render(&self.engine, &Scene::default());
                            log::info!("候选窗 wgpu 渲染器已创建（{}）", r.diagnostics());
                            if let Some(p) = self.im_popup.as_mut() {
                                p.renderer = Some(r);
                            }
                        }
                        Err(e) => {
                            log::warn!("候选窗 wgpu 渲染器创建失败（{e:?}），回退 CPU");
                            if let Some(p) = self.im_popup.as_mut() {
                                p.cpu = Some(CpuRenderer::new(pw, ph, self.scale));
                            }
                        }
                    }
                } else {
                    log::warn!("候选窗 wgpu 共享上下文不可用，回退 CPU");
                    if let Some(p) = self.im_popup.as_mut() {
                        p.cpu = Some(CpuRenderer::new(pw, ph, self.scale));
                    }
                }
            } else if let Some(p) = self.im_popup.as_mut() {
                p.cpu = Some(CpuRenderer::new(pw, ph, self.scale));
            }
        } else if size_changed {
            // 尺寸变化：resize 复用（避免重建闪烁）。
            if let Some(p) = self.im_popup.as_mut() {
                if let Some(cpu) = p.cpu.as_mut() {
                    cpu.resize(pw, ph, self.scale);
                }
                if let Some(r) = p.renderer.as_mut() {
                    r.resize(pw, ph, self.scale);
                }
            }
        }
        // 候选窗内容仅在 dirty（key 变化 / 尺寸变化 / 首次）时渲染提交——
        // 每帧无条件 SHM 提交会致候选窗闪烁。
        if !self.im_popup_dirty {
            return;
        }
        self.im_popup_dirty = false;
        let t_render = Instant::now();
        // 候选窗 Scene（App 引擎驱动）。
        let scene = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.ime_engine_popup_scene(&self.engine)
        }));
        let scene = match scene {
            Ok(s) => s,
            Err(_) => {
                log::error!("App::ime_engine_popup_scene panic，跳过本帧");
                return;
            }
        };
        let render_ms = t_render.elapsed().as_secs_f32() * 1000.0;
        let mut raster_ms = 0.0f32;
        let mut commit_ms = 0.0f32;
        let mut gpu_samples = Vec::new();
        let mut draws = None;
        let mut acquire_ms = None;
        let mut c15_stats = None;

        if let Some(im_popup) = self.im_popup.as_mut() {
            if let Some(cpu) = im_popup.cpu.as_mut() {
                let (srw, srh) = cpu.physical_size();
                let t = Instant::now();
                let rgba = cpu.render(&self.engine, &scene, None);
                raster_ms = t.elapsed().as_secs_f32() * 1000.0;
                // 候选窗同样走 dmabuf 直通（打字时每键一提交 → 收益最直接的表面）。
                let t = Instant::now();
                let surface = im_popup.surface.clone();
                im_popup.out.commit(
                    qh,
                    self.shm.as_ref(),
                    &self.dmabuf,
                    &surface,
                    srw,
                    srh,
                    rgba,
                    self.scale,
                    None,
                );
                commit_ms = t.elapsed().as_secs_f32() * 1000.0;
            } else if let Some(r) = im_popup.renderer.as_mut() {
                let t = Instant::now();
                r.render_with_damage(&self.engine, &scene, None);
                raster_ms = t.elapsed().as_secs_f32() * 1000.0;
                gpu_samples = r.drain_gpu_samples();
                draws = r.last_draws();
                acquire_ms = r.take_acquire_ms();
                c15_stats = r.last_c15_stats();
            }
        }

        self.perf_im_popup.record(render_ms, raster_ms, commit_ms);
        if let Some(n) = draws {
            self.perf_im_popup.record_draws(n);
        }
        if let Some(ms) = acquire_ms {
            self.perf_im_popup.record_acquire(ms);
        }
        for ms in gpu_samples {
            self.perf_im_popup.record_gpu(ms);
        }
        if let Some((inc, dmg_pct, insts)) = c15_stats {
            self.perf_im_popup.record_c15(inc, dmg_pct, insts);
        }
        crate::perf::pacing_present_named(&format!("{:?}:im_popup", self.role), Instant::now());
    }
}

// ── SHM 提交（Ether 合成器 dmabuf 不可见 → 离屏读回 wl_shm 提交）─────────────

/// 创建可共享内存文件（/dev/shm 优先，回退 /tmp）供 wl_shm pool 使用。参 settings/topbar.rs。
///
/// 失败返回 `None` 而非 panic（2026-09-22 审计 P0-4）：`/dev/shm` 满或权限异常时，
/// 旧实现的 `unwrap()` 会直接杀死整个客户端进程。正确行为是**丢帧 + 落盘诊断**。
fn shm_open(size: usize) -> Option<std::fs::File> {
    let name = format!("ether-kanesumi-{}", std::process::id());
    let base = if std::path::Path::new("/dev/shm").exists() {
        "/dev/shm"
    } else {
        "/tmp"
    };
    let path = format!("{}/{}", base, name);
    let file = match std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            log::error!("shm_open 打开 {path} 失败: {e}");
            return None;
        }
    };
    std::fs::remove_file(&path).ok();
    if let Err(e) = file.set_len(size as u64) {
        log::error!("shm_open set_len({size}) 失败（{base} 是否已满？）: {e}");
        return None;
    }
    Some(file)
}

/// 用渲染读回的像素更新 SHM 表面（RGBA→BGRA R/B 交换；单缓冲复用；尺寸变化重建）。
/// wl_shm Argb8888 = 内存 [B,G,R,A]（little-endian），与 Bgra8UnormSrgb readback 一致。
#[allow(clippy::too_many_arguments)]
/// 两矩形并集（外接框）。S4 损坏矩形累积用。
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// 计算本槽位需写入区（物理像素）：全量帧 / 槽位内容不可用（新建）→ 全量（None）；
/// 局部帧 → 本帧 damage ∪ 自上次写该槽后的累积损伤（buffer-age 回补）。
/// 纯函数，单测覆盖回补逻辑。
fn compute_write_region(
    fresh_pool: bool,
    needs_full: bool,
    damage: Option<Rect>,
    partial: Option<Rect>,
) -> Option<Rect> {
    if fresh_pool || needs_full || damage.is_none() {
        None
    } else {
        let d = damage.unwrap();
        Some(match partial {
            Some(p) => union_rect(p, d),
            None => d,
        })
    }
}

/// 主表面是否开启元素树损伤剔除（`App::set_damage_cull`）。
///
/// 参 ELEMENT_TREE「compose 剔除」。CanvasV2 恒为 `false`：其损伤剔除在 `canvas_v2` 内按
/// **物理**损伤 `damage_phys_rect`（含 1 px AA 外扩）经 `inst_intersects` 完成；若在 Tree 层
/// 提前按**未外扩**的逻辑损伤剔除，外扩环带内的邻接图元会被丢掉 —— 增量清除 D 后该环带
/// 出现空洞，与整幅重画对拍 mismatch（参报告 §三.1、§七）。
/// 非 V2（CPU / V1）路径不涉及外扩，保持既有语义 `target_valid && !canvas_full_forced()` 不变。
fn damage_cull_active_for(is_v2: bool, target_valid: bool, full_forced: bool) -> bool {
    if is_v2 {
        false
    } else {
        target_valid && !full_forced
    }
}

fn commit_shm_buffers(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<Shell>,
    surface: &wl_surface::WlSurface,
    state: &mut ShmBuffers,
    width: u32,
    height: u32,
    bgra: &[u8],
    scale: f32,
    damage: Option<Rect>,
) {
    use std::os::fd::AsFd;

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if bgra.len() < expected || width == 0 || height == 0 {
        return;
    }
    // 尺寸变化或 pool 未建 → 重建（pool 大小 = 2×expected，容纳双缓冲）。
    let fresh_pool = state.pool.is_none() || state.width != width || state.height != height;
    if fresh_pool {
        // 先建新 pool 所需的 shm 文件，**成功后再拆旧的**：失败时保持旧 pool 原样并丢帧，
        // 下一帧再试（2026-09-22 审计 P0-4 —— 旧实现 unwrap 直接杀进程）。
        let Some(fd) = shm_open(expected * 2) else {
            write_diag(
                "ether-shm-error.log",
                "shm_open 失败：无法为 wl_shm 建立共享内存池（/dev/shm 是否已满？）\n",
            );
            return;
        };
        state.pool.take().map(|p| p.destroy());
        for b in state.buffers.iter_mut() {
            b.take().map(|b| b.destroy());
        }
        state.mmap = None;
        state.in_flight = [false, false];
        state.next = 0;
        state.needs_full = [true, true];
        state.partial = [None, None];
        let mmap = unsafe { memmap2::MmapMut::map_mut(&fd) }.ok();
        let pool = shm.create_pool(fd.as_fd(), (expected * 2) as i32, qh, ());
        for i in 0..2 {
            let buf = pool.create_buffer(
                (i * expected) as i32,
                width as i32,
                height as i32,
                (width * 4) as i32,
                wl_shm::Format::Argb8888,
                qh,
                (),
            );
            state.buffers[i] = Some(buf);
        }
        state.pool = Some(pool);
        state.mmap = mmap;
        state.width = width;
        state.height = height;
    }
    // 找一个空闲槽位（优先 next，其次另一个；双缓冲都飞则跳过本帧）。
    let idx = if !state.in_flight[state.next] {
        state.next
    } else if !state.in_flight[1 - state.next] {
        1 - state.next
    } else {
        return;
    };
    // 本槽位需写入区（物理像素）：
    // - 全量帧 / 槽位内容不可用（新建）→ 整面；
    // - 局部帧 → 本帧 damage ∪ 自上次写该槽后的累积损伤（buffer-age 回补：
    //   该槽可能两帧未写，其间其它区域变化过 —— 与合成器侧 damage 上传区间对齐）。
    // 参 compositor render/damage.rs 的 age 回补同款思路。
    let write_region = compute_write_region(fresh_pool, state.needs_full[idx], damage, state.partial[idx]);
    // 物理拷贝区（与下方 damage_buffer 发送区间一致 —— 合成器只上传该区，
    // mmap 须恰好写完该区，槽位经 partial 累积回补保持与 CpuRenderer buf 同步）。
    let (cx0, cy0, cw, ch) = match write_region {
        Some(d) => {
            let x0 = (d.origin.x * scale).floor().clamp(0.0, width as f32) as u32;
            let y0 = (d.origin.y * scale).floor().clamp(0.0, height as f32) as u32;
            let x1 = (d.right() * scale).ceil().clamp(0.0, width as f32) as u32;
            let y1 = (d.bottom() * scale).ceil().clamp(0.0, height as f32) as u32;
            (x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
        }
        None => (0, 0, width, height),
    };
    // 局部拷贝 + R/B 交换：只写写入区行（其余像素保留槽位上帧内容）。
    if let Some(mmap) = state.mmap.as_mut() {
        let base = idx * expected;
        let row_bytes = cw as usize * 4;
        for py in cy0..cy0 + ch {
            let src_start = (py * width + cx0) as usize * 4;
            let dst_start = base + src_start;
            if src_start + row_bytes > bgra.len() || dst_start + row_bytes > mmap.len() {
                break;
            }
            for (dst, src) in mmap[dst_start..dst_start + row_bytes]
                .chunks_exact_mut(4)
                .zip(bgra[src_start..src_start + row_bytes].chunks_exact(4))
            {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = src[3];
            }
        }
    }
    // 槽位状态更新（buffer-age 回补登记）。
    state.needs_full[idx] = false;
    state.partial[idx] = None;
    match damage {
        Some(d) => {
            // 另一槽未写期间本帧损伤发生 → 累积，下次写该槽时回补。
            if state.needs_full[1 - idx] {
                state.partial[1 - idx] = None;
            } else {
                state.partial[1 - idx] = Some(match state.partial[1 - idx] {
                    Some(p) => union_rect(p, d),
                    None => d,
                });
            }
        }
        None => {
            // 全量帧：另一槽内容相对当前帧整体过期。
            state.needs_full[1 - idx] = true;
            state.partial[1 - idx] = None;
        }
    }
    if let Some(buf) = state.buffers[idx].as_ref() {
        surface.attach(Some(buf), 0, 0);
        // damage：与 mmap 写入区一致（合成器只上传该区）；None = 全量。
        // 不报 damage 时 KWin 等合成器可能不重绘表面。
        if surface.version() >= 4 {
            surface.damage_buffer(cx0 as i32, cy0 as i32, cw as i32, ch as i32);
        } else if let Some(d) = write_region {
            // 旧协议 damage 用表面坐标（逻辑）；未提供 scale 换算时四舍五入。
            surface.damage(
                d.origin.x.round() as i32,
                d.origin.y.round() as i32,
                d.size.width.round() as i32,
                d.size.height.round() as i32,
            );
        } else {
            surface.damage(0, 0, width as i32, height as i32);
        }
        surface.commit();
        state.in_flight[idx] = true;
        state.next = 1 - idx;
    }
}

// ── SHM 相关空事件处理（wl_shm / pool / buffer，无事件需处理）──────────────

impl Dispatch<wl_shm::WlShm, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm::WlShm,
        _event: wl_shm::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_shm_pool::WlShmPool, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &wl_shm_pool::WlShmPool,
        _event: wl_shm_pool::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<wl_buffer::WlBuffer, ()> for Shell {
    fn event(
        state: &mut Self,
        proxy: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        // release：合成器用完了该缓冲 → 标记可复用（避免重复 attach 同缓冲触发 EBUSY）。
        if let wl_buffer::Event::Release = event {
            // 主表面 / 各浮层 / IME 候选窗 —— 每份输出缓冲（SHM 或 dmabuf 槽）都要认领，
            // 否则双缓冲耗尽后该表面冻结（dmabuf 槽同样靠 release 复位 in_flight）。
            if state.main_out.mark_released(proxy) {
                return;
            }
            if state.comp.mark_released(proxy) {
                return;
            }
            for slot in &mut state.floating_out {
                if slot.mark_released(proxy) {
                    return;
                }
            }
            if state.popup_buffer_released(proxy) {
                return;
            }
            if let Some(popup) = state.im_popup.as_mut() {
                popup.out.mark_released(proxy);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    /// 局部帧 + 无历史 → 写区 = 本帧 damage。
    #[test]
    fn write_region_plain_damage() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        assert_eq!(compute_write_region(false, false, d, None), d);
    }

    /// 局部帧 + 槽位累积损伤（buffer-age 回补）→ 写区 = 两区并集。
    #[test]
    fn write_region_backfills_partial() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        let p = Some(r(200.0, 5.0, 8.0, 8.0));
        let out = compute_write_region(false, false, d, p).unwrap();
        assert_eq!(out, r(10.0, 0.0, 198.0, 13.0)); // 并集外接框
    }

    /// 槽位内容不可用（新建 pool）→ 全量。
    #[test]
    fn write_region_fresh_pool_is_full() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        assert_eq!(compute_write_region(true, false, d, None), None);
    }

    /// 槽位标记全量过期 → 全量。
    #[test]
    fn write_region_needs_full_is_full() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        assert_eq!(compute_write_region(false, true, d, None), None);
    }

    /// 全量帧（damage None）→ 全量，忽略历史。
    #[test]
    fn write_region_full_frame_is_full() {
        assert_eq!(compute_write_region(false, false, None, Some(r(0.0, 0.0, 8.0, 8.0))), None);
    }

    /// 主表面 Tree 损伤剔除开关：V2 一律关闭（改由 canvas_v2 物理剔除 + 1 px AA 外扩承担）。
    #[test]
    fn damage_cull_v2_disabled() {
        assert!(!damage_cull_active_for(true, true, false), "V2 目标有效也必须关闭 Tree 剔除");
        assert!(!damage_cull_active_for(true, false, false), "V2 目标失效同样关闭");
        assert!(!damage_cull_active_for(true, true, true), "V2 强制整幅同样关闭");
        assert!(!damage_cull_active_for(true, false, true), "V2 强制整幅且失效同样关闭");
    }

    /// 非 V2（CPU / V1）路径保持既有语义 `target_valid && !canvas_full_forced()`，不受本任务影响。
    #[test]
    fn damage_cull_non_v2_unchanged() {
        // CPU 路径（target_valid = true）：仅强制整幅开关能关闭。
        assert!(damage_cull_active_for(false, true, false), "CPU 局部帧应开启 Tree 剔除");
        assert!(!damage_cull_active_for(false, true, true), "CPU 强制整幅应关闭 Tree 剔除");
        // V1 路径（target_valid = false）：恒关闭，与整幅开关无关。
        assert!(!damage_cull_active_for(false, false, false), "V1 恒关闭 Tree 剔除");
        assert!(!damage_cull_active_for(false, false, true), "V1 恒关闭 Tree 剔除");
    }

    /// 具名键映射：PageUp/PageDown（含 Prior/Next 别名）、Insert、F1..F12。
    #[test]
    fn map_key_named_keys() {
        use xkeysym::key;
        assert_eq!(map_key(Keysym::new(key::Page_Up), None), Key::PageUp);
        assert_eq!(map_key(Keysym::new(key::Prior), None), Key::PageUp);
        assert_eq!(map_key(Keysym::new(key::Page_Down), None), Key::PageDown);
        assert_eq!(map_key(Keysym::new(key::Next), None), Key::PageDown);
        assert_eq!(map_key(Keysym::new(key::Insert), None), Key::Insert);
        assert_eq!(map_key(Keysym::new(key::F1), None), Key::F(1));
        assert_eq!(map_key(Keysym::new(key::F2), None), Key::F(2));
        assert_eq!(map_key(Keysym::new(key::F5), None), Key::F(5));
        assert_eq!(map_key(Keysym::new(key::F12), None), Key::F(12));
        // F13+ 未纳入具名表 → 透传原始 keysym。
        assert_eq!(map_key(Keysym::new(key::F13), None), Key::Unknown(key::F13));
    }

    /// 空格键保留可打印字符路径（TextBox 与所有 Space 激活控件按 `Char(' ')` 识别）。
    #[test]
    fn map_key_space_is_printable_char() {
        use xkeysym::key;
        assert_eq!(
            map_key(Keysym::new(key::space), Some(" ".to_string())),
            Key::Char(' ')
        );
        // 无 utf8（合成 / 特殊路径）时落 Unknown，不误发具名 Space 破坏文本输入。
        assert_eq!(
            map_key(Keysym::new(key::space), None),
            Key::Unknown(key::space)
        );
    }

    /// Ctrl 组合：xkb 的 utf8 是控制字符（Ctrl+F → `\u{6}`、Ctrl+Space → `\u{0}`），
    /// 不得进 `Char`（其语义是可打印字符，参 DEV_GUIDE.md §2.6）——回退 keysym 的
    /// 可打印字符，ctrl 由 `Modifiers` 携带（2026-10-02 l3 报告：全部 Ctrl+字母
    /// 加速键因此失配）。
    #[test]
    fn map_key_ctrl_combo_falls_back_to_printable_keysym() {
        use xkeysym::key;
        // Ctrl+F：utf8 \u{6}（ACK）→ 加速键按 Char('f') + ctrl 匹配。
        assert_eq!(
            map_key(Keysym::new(key::f), Some("\u{6}".to_string())),
            Key::Char('f')
        );
        // Ctrl+Space（ceyboard 切中英）：utf8 \u{0} → Char(' ')。
        assert_eq!(
            map_key(Keysym::new(key::space), Some("\u{0}".to_string())),
            Key::Char(' ')
        );
        // 无 Ctrl 的可打印 utf8 照旧原样，不走回退。
        assert_eq!(
            map_key(Keysym::new(key::f), Some("f".to_string())),
            Key::Char('f')
        );
        // Backspace 仍由具名表接住（utf8 \u{8} 是控制字符），不受回退影响。
        assert_eq!(
            map_key(Keysym::new(key::BackSpace), Some("\u{8}".to_string())),
            Key::Backspace
        );
    }

    /// IME 未激活（文本字段未聚焦）时，抓取按键一律原样放行——不进引擎，
    /// 因此不会触发 Shift 点按判定等引擎逻辑。参 docs/IME_PLAN.md §Ⅲ IME1。
    #[test]
    fn grab_key_passthrough_when_ime_inactive() {
        assert!(grab_key_passthrough(false, true));
        assert!(grab_key_passthrough(false, false));
    }

    /// IME 激活且 keymap 就绪 → 交引擎处理。
    #[test]
    fn grab_key_goes_to_engine_when_ime_active() {
        assert!(!grab_key_passthrough(true, true));
    }

    /// keymap 未建立（无 xkb 可用）→ 无论激活与否都放行，绝不吞键。
    #[test]
    fn grab_key_passthrough_without_keymap() {
        assert!(grab_key_passthrough(true, false));
    }
}
