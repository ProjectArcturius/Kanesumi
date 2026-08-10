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
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
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
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_surface},
};

use crate::app::{App, InputEvent, PointerButton};
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

/// 查找字体：KANESUMI_TEST_FONT → 常见系统字体。
pub fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    for p in [
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

        // wgpu 渲染器附着。
        let wl_surface = if let Some(w) = &window {
            w.wl_surface()
        } else if let Some(l) = &layer_surface {
            l.wl_surface()
        } else {
            &surface
        };
        let renderer = Renderer::new(conn, wl_surface, width, height, 1.0)
            .map_err(|e| format!("wgpu 初始化失败：{e:?}"))?;

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
            renderer: Some(renderer),
            scale: 1,
            width,
            height,
            configured: false,
            running: true,
            last_frame: Instant::now(),
            pointer_pos: (-1.0, -1.0),
        })
    }

    /// 逻辑尺寸。
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }

    /// 渲染一帧：update + render + 光栅化 + present。
    /// 调用前须请求 frame callback（vsync 推进动画）。
    fn render_frame(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.05);
        self.last_frame = now;

        self.app.update(dt);
        let size = self.size();
        let scene = self.app.render(&self.engine, size);

        // 请求下一帧 callback（须在 present 之前，与本次提交对应）。
        let s = self.surface.clone();
        s.frame(qh, s.clone());

        if let Some(r) = self.renderer.as_mut() {
            r.render(&self.engine, &scene);
        }
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
        if h > 0 {
            self.height = h as f32;
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
                    self.app
                        .handle_input(InputEvent::PointerMoved { x: pos.0, y: pos.1 });
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = (-1.0, -1.0);
                    self.app.handle_input(InputEvent::PointerLeft);
                }
                PointerEventKind::Press { button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.app.handle_input(InputEvent::PointerPressed {
                        x: pos.0,
                        y: pos.1,
                        button,
                    });
                }
                PointerEventKind::Release { button, .. } => {
                    self.pointer_pos = pos;
                    let button = map_button(*button);
                    self.app.handle_input(InputEvent::PointerReleased {
                        x: pos.0,
                        y: pos.1,
                        button,
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
                        self.app.handle_input(InputEvent::Scroll { x: dx, y: dy });
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

// ── 委派宏 ───────────────────────────────────────────────────────────────

delegate_compositor!(Shell);
delegate_output!(Shell);
delegate_seat!(Shell);
delegate_pointer!(Shell);
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
