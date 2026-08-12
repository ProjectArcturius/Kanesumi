// Linux 外壳：Wayland 客户端（sctk）+ wgpu 渲染循环。
//
// 对应 §4.2 三层握手的 Runtime 侧：普通 Wayland 客户端（xdg-shell / layer-shell），
// 动画由 frame callback 推进（参 HANDOVER §1 主循环）。
// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(engine, size)` → 光栅化 Scene。

use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_core::Size;
use smithay_client_toolkit::reexports::{
    calloop::EventLoop, calloop_wayland_source::WaylandSource,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent as SctkKeyEvent, KeyboardHandler, Keysym},
        pointer::{
            BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent, PointerEventKind, PointerHandler,
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
    Connection, Dispatch, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_surface},
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{ContentHint, ContentPurpose, Event as TextInputEvent, ZwpTextInputV3},
};

use crate::app::{
    App, ImeAction, ImeContentHint, ImeContext, InputEvent, Key, Modifiers, PendingImeBatch,
    PointerButton, compute_ime_action,
};
use crate::render::Renderer;
use crate::role::{EtherRole, SurfaceKind};

/// 启动 harness 主循环（Linux）。阻塞运行，不返回。
///
/// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
/// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(size)` → 光栅化 Scene。
pub fn run(app: &mut dyn App) -> ! {
    // `run` 永不返回（`-> !`）：`&mut dyn App` 借用可安全提升为 'static。
    let app: &'static mut dyn App = unsafe { std::mem::transmute(app) };
    if let Err(e) = run_inner(app) {
        eprintln!("kanesumi-harness 异常退出: {e}");
        std::process::exit(1);
    }
    std::process::exit(0);
}

/// 主逻辑。错误以 String 上报（调用方 exit）。
fn run_inner(app: &'static mut dyn App) -> Result<(), String> {
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

    let role = EtherRole::from_env();

    // 字体：App 指定优先，否则环境变量 / 系统字体（SD §IX 禁止静默回退）。
    let font_path = app
        .font_path()
        .or_else(find_font)
        .ok_or_else(|| "未找到字体：设 KANESUMI_TEST_FONT 或提供 App::font_path()".to_string())?;
    let engine = TextEngine::load(&font_path)
        .map_err(|e| format!("加载字体失败 {}：{e}", font_path.display()))?;

    let mut shell = Shell::new(app, engine, &conn, &globals, &qh, role)?;

    // 主循环：持续 dispatch（frame callback 驱动渲染，动画由 vsync 推进）。
    loop {
        if !shell.running {
            break;
        }
        event_loop
            .dispatch(std::time::Duration::from_millis(16), &mut shell)
            .map_err(|e| format!("事件循环 dispatch 失败：{e}"))?;
    }
    Ok(())
}

/// 查找字体：KANESUMI_TEST_FONT → Ether 正体（思源黑体）→ CJK → 常见拉丁。
/// 中文/日文/韩文须 CJK 字体（DejaVu/Liberation 无 CJK 字形，会渲染为方框）。
pub fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    for p in [
        // Ether 正体字体：思源黑体 SC（合成器同款，SD §IX 唯一字体）。
        "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
        "/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf",
        // 系统 CJK（含中文）。
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        // 回退：拉丁（无中文，仅保运行）。
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ] {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 外壳状态：sctk 协议状态 + App + wgpu 渲染器。
// 注：以下字段保留以维持 Wayland 协议对象存活（Drop 即销毁表面/全局）：
// compositor_state / layer_shell / xdg_shell / window / layer_surface / role。
#[allow(dead_code)]
struct Shell {
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

    renderer: Option<Renderer>,

    /// 逻辑缩放（通常 1 或 2）。
    scale: u32,
    /// 逻辑尺寸（configure 后有效）。
    width: f32,
    height: f32,
    configured: bool,
    running: bool,
    last_frame: Instant,
    pointer_pos: (f32, f32),

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
    /// 当前修饰键状态（`update_modifiers` 维护，注入每个输入事件）。
    modifiers: Modifiers,
    /// Wayland 连接（延迟渲染器初始化用：首 configure 后才创建 wgpu surface）。
    conn: Connection,
    /// App 请求但尚未被 configure 确认的动态高度（layer-shell 展开用）。
    pending_height: Option<f32>,
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
                win.set_min_size(Some((width as u32, height as u32)));
                win.commit();
                window = Some(win);
                xdg_shell = Some(shell);
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
                        ls.set_exclusive_zone(-1);
                        ls.set_size(0, 0);
                    }
                }
                ls.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
                ls.commit();
                layer_surface = Some(ls);
                layer_shell = Some(shell);
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
            scale: 1,
            width,
            height,
            configured: false,
            running: true,
            last_frame: Instant::now(),
            pointer_pos: (-1.0, -1.0),
            text_input_manager,
            text_input: None,
            ime_enabled: false,
            ime_focus_surface: false,
            commit_serial: 0,
            pending_ime: PendingImeBatch::default(),
            ime_context_cache: None,
            modifiers: Modifiers::NONE,
            conn: conn.clone(),
            pending_height: None,
        })
    }

    /// 同步 App 请求的动态高度：layer-shell 角色下 `preferred_height` 与当前高度
    /// 不一致 → `set_size` 并立即生效（不等 configure 往返，参旧 topbar.rs `set_height`）。
    /// 合成器按 cached_state.size.h 扩大命中与渲染区域，无需等协议确认。
    fn sync_preferred_height(&mut self) {
        let Some(h) = self.app.preferred_height() else {
            return;
        };
        if (h - self.height).abs() < 0.5 {
            return;
        }
        if let Some(ls) = self.layer_surface.as_ref() {
            ls.set_size(0, h as u32);
        }
        self.height = h;
        self.pending_height = Some(h);
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale as f32);
        }
    }

    /// 逻辑尺寸。
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// 确保渲染器已创建（首个 configure 后调用；surface 已配置、尺寸已知）。
    /// 失败记日志并置 running=false（App 退出）。
    fn ensure_renderer(&mut self) {
        if self.renderer.is_some() {
            return;
        }
        let wl_surface = if let Some(w) = &self.window {
            w.wl_surface()
        } else if let Some(l) = &self.layer_surface {
            l.wl_surface()
        } else {
            &self.surface
        };
        match Renderer::new(&self.conn, wl_surface, self.width, self.height, self.scale as f32) {
            Ok(r) => {
                self.renderer = Some(r);
                log::info!("wgpu 渲染器已创建（{:.0}x{:.0}）", self.width, self.height);
            }
            Err(e) => {
                log::error!("wgpu 渲染器初始化失败（{e:?}），退出");
                self.running = false;
            }
        }
    }

    /// 渲染一帧：update + render + 光栅化 + present。
    /// 调用前须请求 frame callback（vsync 推进动画）。
    fn render_frame(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        // 合成器时钟（PLAN §4.2）：frame callback 驱动，dt 限幅防卡顿后跳变。
        // §4.1 不变量 2 —— 动画由合成器 vsync 时钟推进，不依赖逻辑帧。
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.05);
        self.last_frame = now;

        // 错误边界：App update/render panic 不杀进程，降级为跳过本帧（§4.1 鲁棒性）。
        // &mut dyn App 非 UnwindSafe，用 AssertUnwindSafe 显式声明（App 是单线程消费）。
        let update_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.update(dt);
        }))
        .is_ok();
        if !update_ok {
            log::error!("App::update panic，跳过本帧");
            self.request_next_frame(qh);
            return;
        }

        // 动态高度同步：App update 后可能请求展开/收起（TopBar 面板），先同步表面尺寸。
        self.sync_preferred_height();

        // App 状态可能已变（焦点/文本/光标）→ 幂等 reconcile IME（无变化零成本）。
        self.reconcile_ime();

        let size = self.size();
        let scene_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.render(&self.engine, size)
        }));
        let scene = match scene_result {
            Ok(s) => s,
            Err(_) => {
                log::error!("App::render panic，跳过本帧");
                self.request_next_frame(qh);
                return;
            }
        };

        self.request_next_frame(qh);

        if let Some(r) = self.renderer.as_mut() {
            r.render(&self.engine, &scene);
        }
    }

    /// 请求下一帧 callback（须在 present 之前，与本次提交对应）。
    fn request_next_frame(&mut self, qh: &QueueHandle<Self>) {
        let s = self.surface.clone();
        s.frame(qh, s.clone());
    }

    /// 输入事件错误边界：App handle_input panic 不杀进程，仅记日志。
    fn emit_input(&mut self, event: InputEvent) {
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.handle_input(event);
        }))
        .is_ok();
        if !ok {
            log::error!("App::handle_input panic，已隔离");
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
        let focus_control = self.app.ime_focus().is_some();
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
        let ctx = self.app.ime_focus().unwrap_or_default();
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

    /// 应用新缩放：重配 wgpu 表面物理尺寸 + buffer_scale。
    fn apply_scale(&mut self, scale: u32) {
        if scale == 0 {
            return;
        }
        self.scale = scale;
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, scale as f32);
        }
        self.surface.set_buffer_scale(scale as i32);
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
            r.resize(self.width, self.height, self.scale as f32);
        }
    }
}

// ── CompositorHandler ────────────────────────────────────────────────────

impl CompositorHandler for Shell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        new_factor: i32,
    ) {
        if new_factor > 0 {
            self.apply_scale(new_factor as u32);
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
        qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // vsync 到达 → 渲染下一帧。
        self.render_frame(qh);
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
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output)
            && info.scale_factor > 0
        {
            self.apply_scale(info.scale_factor as u32);
        }
    }

    fn update_output(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        if let Some(info) = self.output_state.info(&output)
            && info.scale_factor > 0
        {
            self.apply_scale(info.scale_factor as u32);
        }
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
        if self.renderer.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if !self.configured {
            self.configured = true;
            // 首帧：render_frame 内部会请求 frame callback 并渲染。
            self.render_frame(_qh);
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
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
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
        // 首 configure：延迟创建渲染器（surface 已配置、尺寸已知；此前 wgpu 会 SURFACE_LOST）。
        if self.renderer.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale as f32);
        }
        self.surface.set_buffer_scale(self.scale as i32);
        if !self.configured {
            self.configured = true;
            // 首帧：render_frame 内部会请求 frame callback 并渲染。
            self.render_frame(_qh);
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
            match &event.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_pos = pos;
                    self.emit_input(InputEvent::PointerMoved { x: pos.0, y: pos.1 });
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = (-1.0, -1.0);
                    self.emit_input(InputEvent::PointerLeft);
                }
                PointerEventKind::Press { button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.emit_input(InputEvent::PointerPressed {
                        x: pos.0,
                        y: pos.1,
                        button,
                        modifiers: self.modifiers,
                    });
                }
                PointerEventKind::Release { button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.emit_input(InputEvent::PointerReleased {
                        x: pos.0,
                        y: pos.1,
                        button,
                        modifiers: self.modifiers,
                    });
                }
                PointerEventKind::Axis {
                    horizontal,
                    vertical,
                    ..
                } => {
                    // 滚轮：优先离散步（每格 ~50px），触摸板用连续像素。
                    // 正方向 = +y（表面坐标，下为正，与 motion 一致）；向下滚为正。
                    const WHEEL_STEP_PX: f32 = 50.0;
                    let dy = if vertical.discrete != 0 {
                        vertical.discrete as f32 * WHEEL_STEP_PX
                    } else {
                        vertical.absolute as f32
                    };
                    let dx = if horizontal.discrete != 0 {
                        horizontal.discrete as f32 * WHEEL_STEP_PX
                    } else {
                        horizontal.absolute as f32
                    };
                    if dx != 0.0 || dy != 0.0 {
                        self.pointer_pos = pos;
                        self.emit_input(InputEvent::Scroll {
                            x: dx,
                            y: dy,
                            modifiers: self.modifiers,
                        });
                    }
                }
            }
        }
    }
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
    }

    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        self.ime_focus_surface = false;
        self.pending_ime = PendingImeBatch::default();
        self.reconcile_ime();
    }

    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: SctkKeyEvent,
    ) {
        let key = map_key(event.keysym, event.utf8);
        self.emit_input(InputEvent::KeyPressed {
            key,
            modifiers: self.modifiers,
        });
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
fn map_key(keysym: Keysym, utf8: Option<String>) -> Key {
    use xkeysym::key;
    match keysym.raw() {
        key::Return | key::KP_Enter => return Key::Enter,
        key::BackSpace => return Key::Backspace,
        key::Escape => return Key::Escape,
        key::Tab => return Key::Tab,
        key::Left => return Key::Left,
        key::Right => return Key::Right,
        key::Up => return Key::Up,
        key::Down => return Key::Down,
        key::Home => return Key::Home,
        key::End => return Key::End,
        key::Delete => return Key::Delete,
        _ => {}
    }
    if let Some(c) = utf8.and_then(|s| s.chars().next()) {
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
delegate_layer!(Shell);

impl ProvidesRegistryState for Shell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
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
