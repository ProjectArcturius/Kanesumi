// Linux 外壳：Wayland 客户端（sctk）+ wgpu 渲染循环。
//
// 对应 §4.2 三层握手的 Runtime 侧：普通 Wayland 客户端（xdg-shell / layer-shell），
// 动画由 frame callback 推进（参 HANDOVER §1 主循环）。
// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(engine, size)` → 光栅化 Scene。

use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Rect, Size};
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
// input-method-v2 引擎宿主（Ceyboard 作为 IME 引擎连接合成器）。参 CEYBOARD_SPEC §Ⅴ。
use wayland_protocols_misc::zwp_input_method_v2::client::{
    zwp_input_method_keyboard_grab_v2::{
        Event as ImGrabEvent, ZwpInputMethodKeyboardGrabV2,
    },
    zwp_input_method_manager_v2::ZwpInputMethodManagerV2,
    zwp_input_method_v2::{Event as ImEvent, ZwpInputMethodV2},
    zwp_input_popup_surface_v2::ZwpInputPopupSurfaceV2,
};

use crate::app::{
    AnchorKind, App, FloatingLayer, ImeAction, ImeContentHint, ImeContext, InputEvent, Key,
    LayerKind, Modifiers, PendingImeBatch, PointerButton, compute_ime_action,
};
use crate::appmenu::AppMenuHandle;
use crate::context_menu::ContextMenuAction;
use crate::render::Renderer;
use crate::role::{EtherRole, SurfaceKind};

/// 启动 harness 主循环（Linux）。阻塞运行，不返回。
///
/// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
/// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(size)` → 光栅化 Scene。
/// 诊断文件双写：/tmp + $HOME（LightDM 多会话 /tmp 可能隔离，home 跨 session 共享）。
fn write_diag(name: &str, content: &str) {
    let _ = std::fs::write(format!("/tmp/{name}"), content);
    if let Ok(home) = std::env::var("HOME") {
        let _ = std::fs::write(std::path::Path::new(&home).join(name), content);
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
    let engine = TextEngine::load_with_fallbacks(&font_path, fallback_fonts(&font_path))
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
        "/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf",
        "/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf",
        "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
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

fn fallback_fonts(primary: &std::path::Path) -> Vec<std::path::PathBuf> {
    [
        "/usr/local/share/fonts/s/SourceHanSansSC-Regular.otf",
        "/usr/local/share/fonts/s/SourceHanSansTC_Regular.otf",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/noto/NotoSansArabic-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansHebrew-Regular.ttf",
        "/usr/share/fonts/noto/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/TTF/NotoSansSymbols2-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .filter(|path| path != primary && path.exists())
    .collect()
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
    last_frame: Instant,
    pointer_pos: (f32, f32),
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
    /// 候选窗 popup surface（引擎激活时创建，deactivate 释放）。
    im_popup: Option<ImPopupSurface>,

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

    // ── SHM 输出（Ether 合成器 dmabuf 不可见 → 离屏读回 wl_shm 提交）─────
    /// wl_shm 全局（layer-shell 表面用 SHM 提交；合成器未提供 → None，退化直接 present）。
    shm: Option<wl_shm::WlShm>,
    /// 主表面是否走 SHM 输出（layer-shell 角色 = true；xdg-shell = false）。
    shm_output: bool,
    /// 主表面 SHM 缓冲状态。
    main_shm: ShmBuffers,
    /// 各浮层表面的 SHM 缓冲状态（与 `floating` 等长）。
    floating_shm: Vec<ShmBuffers>,
}

/// 单个 layer-shell 表面的 SHM 缓冲（单缓冲复用；尺寸变化时重建 pool/buffer）。
struct ShmBuffers {
    pool: Option<wl_shm_pool::WlShmPool>,
    buffer: Option<wl_buffer::WlBuffer>,
    mmap: Option<memmap2::MmapMut>,
    width: u32,
    height: u32,
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
    renderer: Option<Renderer>,
    shm: ShmBuffers,
    width: f32,
    height: f32,
}

impl Default for ShmBuffers {
    fn default() -> Self {
        Self {
            pool: None,
            buffer: None,
            mmap: None,
            width: 0,
            height: 0,
        }
    }
}

/// 浮层表面 —— 独立 wl_surface + layer-shell + 透明底渲染器。
/// 内容由 `App::render_floating(idx)` 提供；输入按指针所在表面路由到 `floating_input`。
struct FloatingSurface {
    surface: wl_surface::WlSurface,
    layer_surface: LayerSurface,
    renderer: Option<Renderer>,
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
                ls.set_keyboard_interactivity(KeyboardInteractivity::None);
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

        // IME 引擎宿主：App 声明 ime_engine_host() 时绑定 zwp_input_method_manager_v2。
        // 合成器未提供 → None，Ceyboard 退化为无键盘引擎（仅 UI 展示）。
        let input_method_manager = if app.ime_engine_host() {
            globals
                .bind::<ZwpInputMethodManagerV2, Self, ()>(qh, 1..=1, ())
                .map_err(|e| {
                    log::warn!("zwp_input_method_manager_v2 不可用，引擎宿主降级：{e}");
                })
                .ok()
        } else {
            None
        };

        // wl_shm 全局（SHM 提交用）。Ether 合成器对 layer-shell wgpu dmabuf 渲染不可见，
        // layer-shell 角色一律走离屏读回 → wl_shm 提交。合成器未提供 → None，退化 present。
        let shm = globals
            .bind::<wl_shm::WlShm, Self, ()>(qh, 1..=1, ())
            .map_err(|e| {
                log::warn!("wl_shm 不可用，SHM 提交降级为直接 present：{e}");
            })
            .ok();
        // 全部角色一律 SHM 输出（离屏 wgpu 读回 → wl_shm 提交）：Ether 合成器下
        // wgpu dmabuf 直出不可见（ETHER_RENDER_LESSONS.md 验证矩阵），SHM 是唯一
        // 可靠路径（含 xdg-shell 浏览器窗口）。wl_shm 缺失时 render_frame 回退直出。
        let shm_output = true;
        let main_shm = ShmBuffers::default();

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
        let floating_shm = std::iter::repeat_with(ShmBuffers::default)
            .take(floating.len())
            .collect();

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
            scale: 1.0,
            _fractional_scale_manager: fractional_scale_manager,
            _viewporter: viewporter,
            _fractional_scale: fractional_scale,
            viewport,
            width,
            height,
            configured: false,
            running: true,
            last_frame: Instant::now(),
            pointer_pos: (-1.0, -1.0),
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
            im_popup: None,
            modifiers: Modifiers::NONE,
            conn: conn.clone(),
            pending_height: None,
            floating,
            appmenu,
            appmenu_rx,
            diag_logged: false,
            shm,
            shm_output,
            main_shm,
            floating_shm,
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
    }

    /// 渲染浮层帧：ensure renderer（透明底）→ App::render_floating → 光栅化 → present。
    fn render_floating_frame(&mut self, idx: usize, qh: &QueueHandle<Self>) {
        if !self.app.floating_visible(idx) {
            // 浮层隐藏：不请求下一帧 → 表面空闲（无 frame 回调，零成本）。
            return;
        }
        let (app, floating) = (&mut self.app, &mut self.floating);
        let Some(f) = floating.get_mut(idx) else {
            return;
        };
        if !f.configured {
            return;
        }
        if f.renderer.is_none() {
            match Renderer::new(&self.conn, &f.surface, f.width, f.height, f.scale, true, true) {
                Ok(r) => {
                    f.renderer = Some(r);
                    log::info!("浮层渲染器已创建（{:.0}x{:.0}，透明底，SHM 输出）", f.width, f.height);
                }
                Err(e) => {
                    log::error!("浮层渲染器初始化失败（{e:?}），退出");
                    write_diag(
                        "ether-renderer-error.log",
                        &format!("浮层渲染器初始化失败：{e:?}\n"),
                    );
                    self.running = false;
                    return;
                }
            }
        }
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
        let s = f.surface.clone();
        s.frame(qh, s.clone());
        let surface = f.surface.clone();
        if let Some(r) = f.renderer.as_mut() {
            // 浮层恒为 layer-shell → SHM 提交（Ether 合成器 dmabuf 不可见）。
            if let Some(bgra) = r.render_to_shm(&self.engine, &scene) {
                let (pw, ph) = r.physical_size();
                if let Some(shm) = self.shm.clone() {
                    commit_shm_buffers(
                        &shm,
                        qh,
                        &surface,
                        &mut self.floating_shm[idx],
                        pw,
                        ph,
                        &bgra,
                    );
                }
            }
        }
    }

    /// 浮层输入事件错误边界。
    fn emit_floating_input(&mut self, idx: usize, event: InputEvent) {
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.floating_input(idx, event);
        }))
        .is_ok();
        if !ok {
            log::error!("App::floating_input panic，已隔离");
        }
    }

    /// 输入路由：`Some(i)` → 浮层 `i`；`None` → 主表面。
    fn route_input(&mut self, idx: Option<usize>, event: InputEvent) {
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
        // 桌面（Background 层，外部布局）需透明底：让合成器基色/壁纸透出，
        // 否则离屏读回是整幅不透明黑（桌面黑屏而非 #1E1E1E）。参 role.rs Desktop。
        let transparent = self.role.surface_kind() == SurfaceKind::LayerBackground;
        match Renderer::new(
            &self.conn,
            wl_surface,
            self.width,
            self.height,
            self.scale,
            transparent,
            self.shm_output,
        ) {
            Ok(r) => {
                self.renderer = Some(r);
                log::info!(
                    "wgpu 渲染器已创建（{:.0}x{:.0}，SHM 输出 {}）",
                    self.width,
                    self.height,
                    self.shm_output,
                );
            }
            Err(e) => {
                log::error!("wgpu 渲染器初始化失败（{e:?}），退出");
                // 会话内无日志 UI：错误写文件供排查（Ether 下 wgpu 初始化兼容问题）。
                write_diag(
                    "ether-renderer-error.log",
                    &format!("wgpu 渲染器初始化失败：{e:?}\n"),
                );
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
            let _ = std::fs::write("/tmp/ether-kanesumi-diag.txt", lines.as_bytes());
            log::info!("{}", lines.trim_end());
        }
        // 合成器时钟（PLAN §4.2）：frame callback 驱动，dt 限幅防卡顿后跳变。
        // §4.1 不变量 2 —— 动画由合成器 vsync 时钟推进，不依赖逻辑帧。
        let now = Instant::now();
        let dt = now.duration_since(self.last_frame).as_secs_f64().min(0.05);
        self.last_frame = now;

        // 全局菜单命令（App::on_menu_command）：在 App::update 之前派发，
        // 保证菜单触发的状态变更当帧生效。
        self.drain_menu_commands();

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
        // 右键菜单动画 tick（弹出/关闭轨道，与 App 状态解耦）。
        self.ctx_menu.update(dt);

        // 动态高度同步：App update 后可能请求展开/收起（TopBar 面板），先同步表面尺寸。
        self.sync_preferred_height();
        self.sync_floating_heights();
        // 浮层渲染唤醒：App 打开浮层（floating_visible 变 true）→ 首帧直接渲染
        // （建渲染器 + SHM 提交，使刚请求的 frame callback 生效），此后由 vsync 驱动。
        for i in 0..self.floating.len() {
            if !self.app.floating_visible(i) || !self.floating[i].configured {
                continue;
            }
            if self.floating[i].renderer.is_none() {
                self.render_floating_frame(i, qh);
            } else {
                let s = self.floating[i].surface.clone();
                s.frame(qh, s.clone());
            }
        }

        // App 状态可能已变（焦点/文本/光标）→ 幂等 reconcile IME（无变化零成本）。
        self.reconcile_ime();

        // IME 候选窗 popup 刷新（引擎激活时；尺寸变化重建 surface，随后渲染提交）。
        self.refresh_im_popup(qh);

        let size = self.size();
        let scene_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.render(&self.engine, size)
        }));
        let mut scene = match scene_result {
            Ok(s) => s,
            Err(_) => {
                log::error!("App::render panic，跳过本帧");
                self.request_next_frame(qh);
                return;
            }
        };

        // 右键菜单叠加（主表面渲染，App 内容之上）。
        if self.ctx_menu.is_visible() {
            self.ctx_menu
                .render(&self.app.theme(), &self.engine, &mut scene);
        }

        self.request_next_frame(qh);

        if let Some(r) = self.renderer.as_mut() {
            // SHM 输出优先（Ether 合成器 dmabuf 不可见）；合成器无 wl_shm 时回退直出。
            if self.shm_output && self.shm.is_some() {
                // 离屏读回 → wl_shm 提交（ETHER_RENDER_LESSONS.md 唯一可靠路径）。
                if let Some(bgra) = r.render_to_shm(&self.engine, &scene) {
                    let (pw, ph) = r.physical_size();
                    if let Some(shm) = self.shm.clone() {
                        commit_shm_buffers(
                            &shm,
                            qh,
                            &self.surface,
                            &mut self.main_shm,
                            pw,
                            ph,
                            &bgra,
                        );
                    }
                }
            } else {
                r.render(&self.engine, &scene);
            }
        }
    }

    /// 请求下一帧 callback（须在 present 之前，与本次提交对应）。
    fn request_next_frame(&mut self, qh: &QueueHandle<Self>) {
        let s = self.surface.clone();
        s.frame(qh, s.clone());
    }

    /// 主表面输入：右键菜单优先路由（参 CONTEXT_MENU_SPEC §Ⅵ.2）→ 未消费才投给 App。
    /// - 菜单关着 + 右键按下 → `App::context_menu(x, y)` 取内容：Some → 开菜单并消费；
    ///   None → 右键照常投递（App 自处理）。
    /// - 菜单开着 → 事件喂状态机（悬停/点选/LightDismiss/Esc/再右键），点选 → `on_context_command`。
    fn emit_input(&mut self, event: InputEvent) {
        let action = {
            // 仅「菜单关着 + 右键按下」才请求 App 内容（&mut app 与 &mut ctx 分开借用）。
            let items = if !self.ctx_menu.is_visible() {
                if let InputEvent::PointerPressed {
                    x, y, button: PointerButton::Right, ..
                } = &event
                {
                    self.app.context_menu(*x, *y)
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
    fn apply_scale(&mut self, scale: f32) {
        if scale <= 0.0 || !scale.is_finite() {
            return;
        }
        self.scale = scale;
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, scale);
        }
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
        let Some(index) = self.floating_idx(surface) else {
            return;
        };
        let floating = &mut self.floating[index];
        floating.scale = scale;
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
        if let Some(renderer) = floating.renderer.as_mut() {
            renderer.resize(floating.width, floating.height, scale);
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
        qh: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        // vsync 到达 → 按表面分发渲染：主表面 / 浮层。
        if *surface == self.surface {
            self.render_frame(qh);
        } else if let Some(idx) = self.floating_idx(surface) {
            self.render_floating_frame(idx, qh);
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
                let s = f.surface.clone();
                s.frame(qh, s.clone());
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
        // 首 configure：延迟创建渲染器（surface 已配置、尺寸已知；此前 wgpu 会 SURFACE_LOST）。
        if self.renderer.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if let Some(r) = self.renderer.as_mut() {
            r.resize(self.width, self.height, self.scale);
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
        if !self.configured {
            self.configured = true;
            // 首帧：render_frame 内部会请求 frame callback 并渲染。
            self.render_frame(qh);
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
                        self.route_input(
                            target,
                            InputEvent::Scroll {
                                x: dx,
                                y: dy,
                                modifiers: self.modifiers,
                            },
                        );
                    }
                }
            }
        }
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
        let text = event
            .utf8
            .as_deref()
            .filter(|text| text.chars().count() > 1)
            .map(str::to_owned);
        let key = map_key(event.keysym, if text.is_some() { None } else { event.utf8 });
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
            _ => {}
        }
    }
}

impl Dispatch<ZwpInputMethodKeyboardGrabV2, ()> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &ZwpInputMethodKeyboardGrabV2,
        event: ImGrabEvent,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
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
                // 合成器转发的硬件按键 → 引擎。仅按下（state==Pressed）且引擎激活时处理。
                use wayland_client::WEnum;
                let pressed = matches!(
                    kstate,
                    WEnum::Value(wayland_client::protocol::wl_keyboard::KeyState::Pressed)
                );
                if !pressed || !state.im_active {
                    return;
                }
                let Some(xkb) = state.im_xkb.as_ref() else {
                    return;
                };
                let (sym, utf8) = xkb.keycode_to_sym(key);
                let logical = map_key(
                    xkeysym::Keysym::new(sym),
                    utf8,
                );
                // 引擎处理按键 → 更新 preedit/commit，随即 flush 上屏。
                state.app.ime_engine_key(logical, state.im_modifiers);
                if let Some(im) = state.input_method.clone() {
                    state.flush_engine(&im);
                }
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

impl Shell {
    /// 引擎宿主 flush：把 App 引擎的 preedit / commit / delete 通过 input-method-v2 上屏。
    /// 幂等：preedit 无变化不重发 set_preedit_string（避免光标抖动）。
    fn flush_engine(&mut self, im: &ZwpInputMethodV2) {
        // 1. 待提交文本（选词/空格/回车）。
        let mut committed = false;
        while let Some(text) = self.app.ime_engine_take_commit() {
            im.commit_string(text);
            committed = true;
        }
        // 2. 待删除周边字节（退格）。double-buffered → 须 commit 才生效。
        let (before, after) = self.app.ime_engine_take_delete();
        if before > 0 || after > 0 {
            im.delete_surrounding_text(before, after);
            committed = true;
        }
        // 3. 组合态 preedit（变化才发）。double-buffered → 变化时 commit 才生效。
        let (preedit, cursor_byte) = self.app.ime_engine_preedit();
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

    /// 候选窗 popup surface 每帧刷新：按引擎状态建/调整 surface，渲染候选窗 Scene 提交。
    /// 合成器把 `zwp_input_popup_surface_v2` 渲染到 Layer 6 Overlay（跟随光标）。
    fn refresh_im_popup(&mut self, qh: &QueueHandle<Self>) {
        let (pw, ph) = self.app.ime_engine_popup_size();
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

        // 尺寸变化或首次 → 重建 popup surface + 渲染器。
        let size_changed = self
            .im_popup
            .as_ref()
            .map(|p| (p.width - pw).abs() > 0.5 || (p.height - ph).abs() > 0.5)
            .unwrap_or(true);
        if size_changed || self.im_popup.as_ref().map(|p| p.renderer.is_none()).unwrap_or(true) {
            if let Some(old) = self.im_popup.take() {
                old.surface.destroy();
            }
            let Some(im) = self.input_method.clone() else {
                return;
            };
            let surface = self.compositor_state.create_surface(qh);
            let popup = im.get_input_popup_surface(&surface, qh, ());
            log::info!("IME 候选窗 popup 创建：{pw:.0}×{ph:.0}");
            self.im_popup = Some(ImPopupSurface {
                surface: surface.clone(),
                popup,
                renderer: None,
                shm: ShmBuffers::default(),
                width: pw,
                height: ph,
            });
        }

        let Some(im_popup) = self.im_popup.as_mut() else {
            return;
        };
        if im_popup.renderer.is_none() {
            match Renderer::new(&self.conn, &im_popup.surface, pw, ph, 1.0, true, true) {
                Ok(r) => im_popup.renderer = Some(r),
                Err(e) => {
                    log::warn!("候选窗 popup 渲染器初始化失败：{e:?}");
                    return;
                }
            }
        }
        let Some(r) = im_popup.renderer.as_mut() else {
            return;
        };
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
        if let Some(bgra) = r.render_to_shm(&self.engine, &scene) {
            let (srw, srh) = r.physical_size();
            if let Some(shm) = self.shm.clone() {
                commit_shm_buffers(
                    &shm,
                    qh,
                    &im_popup.surface,
                    &mut im_popup.shm,
                    srw,
                    srh,
                    &bgra,
                );
            }
        }
    }
}

// ── SHM 提交（Ether 合成器 dmabuf 不可见 → 离屏读回 wl_shm 提交）─────────────

/// 创建可共享内存文件（/dev/shm 优先，回退 /tmp）供 wl_shm pool 使用。参 settings/topbar.rs。
fn shm_open(size: usize) -> std::fs::File {
    let name = format!("ether-kanesumi-{}", std::process::id());
    let base = if std::path::Path::new("/dev/shm").exists() {
        "/dev/shm"
    } else {
        "/tmp"
    };
    let path = format!("{}/{}", base, name);
    let file = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .open(&path)
        .unwrap();
    std::fs::remove_file(&path).ok();
    file.set_len(size as u64).unwrap();
    file
}

/// 用渲染读回的 BGRA 像素更新 SHM 表面（单缓冲复用；尺寸变化时重建 pool/buffer）。
/// wl_shm Argb8888 = 内存 [B,G,R,A]（little-endian），与 Bgra8UnormSrgb readback 一致。
#[allow(clippy::too_many_arguments)]
fn commit_shm_buffers(
    shm: &wl_shm::WlShm,
    qh: &QueueHandle<Shell>,
    surface: &wl_surface::WlSurface,
    state: &mut ShmBuffers,
    width: u32,
    height: u32,
    bgra: &[u8],
) {
    use std::os::fd::AsFd;

    let expected = (width as usize)
        .saturating_mul(height as usize)
        .saturating_mul(4);
    if bgra.len() < expected || width == 0 || height == 0 {
        return;
    }
    // 尺寸变化或 pool 未建 → 重建。
    if state.pool.is_none() || state.width != width || state.height != height {
        state.pool.take().map(|p| p.destroy());
        state.buffer.take().map(|b| b.destroy());
        state.mmap = None;
        let fd = shm_open(expected);
        let mmap = unsafe { memmap2::MmapMut::map_mut(&fd) }.ok();
        let pool = shm.create_pool(fd.as_fd(), expected as i32, qh, ());
        let buf = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            (width * 4) as i32,
            wl_shm::Format::Argb8888,
            qh,
            (),
        );
        state.pool = Some(pool);
        state.buffer = Some(buf);
        state.mmap = mmap;
        state.width = width;
        state.height = height;
    }
    if let Some(mut mmap) = state.mmap.take() {
        let n = bgra.len().min(mmap.len());
        mmap[..n].copy_from_slice(&bgra[..n]);
        state.mmap = Some(mmap);
    }
    if let Some(buf) = state.buffer.as_ref() {
        surface.attach(Some(buf), 0, 0);
        surface.commit();
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
        _state: &mut Self,
        _proxy: &wl_buffer::WlBuffer,
        _event: wl_buffer::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}
