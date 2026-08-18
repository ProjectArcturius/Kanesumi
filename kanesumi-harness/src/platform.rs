// Linux 外壳：Wayland 客户端（sctk）+ wgpu 渲染循环。
//
// 对应 §4.2 三层握手的 Runtime 侧：普通 Wayland 客户端（xdg-shell / layer-shell），
// 动画由 frame callback 推进（参 HANDOVER §1 主循环）。
// 职责：连 Wayland → 按 `EtherRole::surface_kind()` 建表面（xdg-shell / layer-shell）→
// wgpu 附着 → frame callback 驱动 `App::update(dt)` / `App::render(engine, size)` → 光栅化 Scene。

use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{Rect, Size};
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
// 虚拟键盘：重放引擎未消费的按键给焦点客户端（fcitx5 同款透传）。
use wayland_protocols_misc::zwp_virtual_keyboard_v1::client::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1,
};

use crate::app::{
    AnchorKind, App, FloatingLayer, ImeAction, ImeContentHint, ImeContext, InputEvent, Key,
    LayerKind, Modifiers, PendingImeBatch, PointerButton, compute_ime_action,
};
use crate::appmenu::AppMenuHandle;
use crate::context_menu::ContextMenuAction;
use crate::cpu_raster::CpuRenderer;
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
        let busy = shell.dirty || shell.floating_dirty.iter().any(|d| *d);
        let timeout = if busy {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(100)
        };
        event_loop
            .dispatch(timeout, &mut shell)
            .map_err(|e| format!("事件循环 dispatch 失败：{e}"))?;
        // 引擎宿主幂等绑定（input_method.is_none 才建，seat 就绪后即生效）。
        shell.ensure_ime_engine(&qh);
        // 推进步：update / 定时器 / 菜单命令 / IME / 尺寸同步（与渲染解耦，I-4）。
        shell.step(&qh);
        // 脏 → 渲染 + commit（I-1：CPU 缓冲恒就绪，无条件成功）。
        if shell.dirty {
            shell.render_and_commit(&qh);
            shell.dirty = false;
        }
        for i in 0..shell.floating.len() {
            if shell.floating_dirty[i] {
                shell.floating_dirty[i] = false;
                shell.render_floating_frame(i, &qh);
            }
        }
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
    renderer: Option<Renderer>,
    /// layer-shell CPU 光栅化器（Scene → SHM 像素，零 GPU 同步点）。
    /// 参 TOPBAR_RENDER_REFACTOR §4.1 / cpu_raster.rs。
    cpu: Option<CpuRenderer>,

    /// 主表面脏标记（I-3：输入 / 定时器 / 动画 / 尺寸变化显式置位，commit 后清除）。
    /// frame 回调仅作 vsync 提示（置位 dirty），不驱动渲染（I-2）。
    dirty: bool,
    /// 各浮层表面脏标记（与 `floating` 等长）。
    floating_dirty: Vec<bool>,
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

    // ── SHM 输出（layer-shell 角色 CPU 光栅化 → wl_shm 提交）─────
    /// wl_shm 全局（layer-shell 表面用 SHM 提交；合成器未提供 → None，退化直接 present）。
    shm: Option<wl_shm::WlShm>,
    /// 主表面 SHM 缓冲状态。
    main_shm: ShmBuffers,
    /// 各浮层表面的 SHM 缓冲状态（与 `floating` 等长）。
    floating_shm: Vec<ShmBuffers>,
    /// 上次 update 的时刻（合成器时钟：dt 限幅防卡顿后跳变，§4.1 不变量 2）。
    last_update: Instant,
    /// 主表面 Scene 复用缓冲（egui PaintList）：每帧 `render_into` 就地清空重建，
    /// 复用 Vec 容量，避免每帧 `Scene::default()` + push 重分配。
    scene_buf: Scene,

    // ── dmabuf 直通（layer-shell CPU 角色可选输出；未开 → 维持 SHM）─────
    /// 客户端 dmabuf 全局（合成器提供 → Some）。参 linux-dmabuf 协议。
    dmabuf: DmabufState,
    /// 是否启用 dmabuf 直通：`ETHER_DMABUF=1` 且 gbm device 可用且 dmabuf 全局存在。
    /// 默认关（SHM 保底）；DRM 会话验证通过后于全面切换时翻默认。参 LINUX_DMABUF_PLAN M2。
    dmabuf_enabled: bool,
    /// 主表面 dmabuf 输出缓冲（layer-shell CPU 角色；xdg 角色闲置）。
    dmabuf_out: crate::dmabuf::DmabufBuffers,
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
    /// CPU 光栅化器（popup surface 走 SHM 提交；resize 复用，不重建）。
    cpu: Option<CpuRenderer>,
    shm: ShmBuffers,
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
    /// CPU 光栅化器（浮层恒为 layer-shell → SHM 提交）。
    cpu: Option<CpuRenderer>,
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
        let main_shm = ShmBuffers::default();

        // 客户端 dmabuf 全局（linux-dmabuf-feedback 主设备协商见 M5）。
        // ⚠ DMABUF 属性：非 XRGB8888（无 alpha）→ Alpha 通道读 0 → 整个 buffer 透明；
        //   bo 用 ARGB8888（has_alpha），合成器按 alpha 合成。
        let dmabuf_state = DmabufState::new(globals, qh);
        let dmabuf_present = dmabuf_state.version().is_some();
        let dmabuf_out = crate::dmabuf::DmabufBuffers::default();
        // 开 dmabuf 直通：`ETHER_DMABUF=1` + 合成器提供 global + 客户端能开 gbm device。
        // 默认关（SHM 保底，防 DRM 会话未验证前回归；全面切换时再翻）。参 LINUX_DMABUF_PLAN §4 回退策略。
        let dmabuf_enabled = std::env::var("ETHER_DMABUF").as_deref() == Ok("1")
            && dmabuf_present
            && dmabuf_out.ready();
        if dmabuf_enabled {
            log::info!("dmabuf 直通启用（ETHER_DMABUF=1）：合成器 global OK + gbm device 就绪");
        } else {
            log::info!(
                "SHM 提交路径：ETHER_DMABUF={:?} global={} gbm_ready={}",
                std::env::var("ETHER_DMABUF").unwrap_or_default(),
                dmabuf_present,
                dmabuf_out.ready()
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
            cpu: None,
            dirty: true,
            floating_dirty: vec![false; floating.len()],
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
            shm,
            main_shm,
            floating_shm,
            last_update: Instant::now(),
            scene_buf: Scene::default(),
            dmabuf: dmabuf_state,
            dmabuf_enabled,
            dmabuf_out,
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
            self.floating_dirty[i] = true; // 尺寸变化 → 呈现新高度（I-3）。
            if let Some(viewport) = f.viewport.as_ref()
                && h > 1.0
            {
                viewport
                    .set_destination(f.width.round().max(1.0) as i32, h.round().max(1.0) as i32);
            }
        }
    }

    /// 渲染浮层帧：ensure CPU 光栅器（透明底）→ App::render_floating → 光栅化 → SHM 提交。
    fn render_floating_frame(&mut self, idx: usize, qh: &QueueHandle<Self>) {
        if !self.app.floating_visible(idx) {
            // 浮层隐藏：不请求下一帧 → 表面空闲（零成本）。
            return;
        }
        let (app, floating) = (&mut self.app, &mut self.floating);
        let Some(f) = floating.get_mut(idx) else {
            return;
        };
        if !f.configured {
            return;
        }
        // ⚠ 光栅器惰性创建前先同步当前 App 请求高度：面板打开时 floating_height 返回
        //   面板高，f.height 可能仍是收起值 → 光栅器用 0 高度创建 → 浮层永远不可见。
        let h = app.floating_height(idx);
        if (h - f.height).abs() >= 0.5 {
            let h = h.max(1.0);
            f.layer_surface.set_size(f.width as u32, h as u32);
            f.height = h;
            if let Some(cpu) = f.cpu.as_mut() {
                cpu.resize(f.width, h, f.scale);
            }
            if let Some(viewport) = f.viewport.as_ref()
                && h > 1.0
            {
                viewport
                    .set_destination(f.width.round().max(1.0) as i32, h.round().max(1.0) as i32);
            }
        }
        if f.cpu.is_none() {
            f.cpu = Some(CpuRenderer::new(f.width, f.height, f.scale));
            log::info!("浮层 CPU 光栅化器已创建（{:.0}x{:.0}）", f.width, f.height);
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
        // 按需重绘：浮层动画跑完即停（floating_needs_redraw false → 不请求下一帧）。
        if app.floating_needs_redraw(idx) {
            s.frame(qh, s.clone());
        }
        if let Some(cpu) = f.cpu.as_mut() {
            let (pw, ph) = cpu.physical_size();
            let rgba = cpu.render(&self.engine, &scene, None);
            if let Some(shm) = self.shm.clone() {
                commit_shm_buffers(
                    &shm,
                    qh,
                    &f.surface,
                    &mut self.floating_shm[idx],
                    pw,
                    ph,
                    rgba,
                    f.scale,
                    None,
                );
            }
        }
    }

    /// 浮层输入事件错误边界。事件到达即置脏（I-4）。
    fn emit_floating_input(&mut self, idx: usize, event: InputEvent) {
        let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.floating_input(idx, event);
        }))
        .is_ok();
        if !ok {
            log::error!("App::floating_input panic，已隔离");
        }
        if idx < self.floating_dirty.len() {
            self.floating_dirty[idx] = true;
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
            d = Some(match d {
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

    /// 确保渲染器已创建（首个 configure 后调用；surface 已配置、尺寸已知）。
    /// 失败记日志并置 running=false（App 退出）。
    ///
    /// 按表面类型分派（TOPBAR_RENDER_REFACTOR §3.2）：
    /// - xdg-shell（Settings 窗口等）→ wgpu `Renderer`（直出 present）。
    /// - layer-shell（TopBar/Dock/Launcher/Ceyboard）→ `CpuRenderer`（Scene 直接
    ///   光栅化进 SHM；零 GPU 同步点、零读回）。
    fn ensure_renderer(&mut self) {
        if self.renderer.is_some() || self.cpu.is_some() {
            return;
        }
        if matches!(self.role.surface_kind(), SurfaceKind::XdgShell) {
            let wl_surface = if let Some(w) = &self.window {
                w.wl_surface()
            } else {
                &self.surface
            };
            match Renderer::new(&self.conn, wl_surface, self.width, self.height, self.scale, false) {
                Ok(r) => {
                    self.renderer = Some(r);
                    log::info!("wgpu 渲染器已创建（{:.0}x{:.0}）", self.width, self.height);
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
        } else {
            // layer-shell → CPU 光栅化。无失败模式：Vec 分配即就绪（I-1）。
            let cpu = CpuRenderer::new(self.width, self.height, self.scale);
            log::info!(
                "CPU 光栅化器已创建（{:.0}x{:.0}，scale {}）",
                self.width,
                self.height,
                self.scale,
            );
            self.cpu = Some(cpu);
        }
    }

    /// 推进步（每循环迭代；与渲染解耦，TOPBAR_RENDER_REFACTOR I-4）：
    /// update / 定时器 / 菜单命令 / IME / 尺寸同步。渲染完全由 `dirty` 驱动，
    /// 本步只推进状态并显式置位脏标记（I-3）。
    fn step(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        // 合成器时钟（PLAN §4.2）：dt 限幅防卡顿后跳变。§4.1 不变量 2。
        let now = Instant::now();
        let dt = now.duration_since(self.last_update).as_secs_f64().min(0.05);
        self.last_update = now;

        // 全局菜单命令（App::on_menu_command）：在 App::update 之前派发，
        // 保证菜单触发的状态变更当帧生效。
        self.drain_menu_commands();

        // 错误边界：App update panic 不杀进程（§4.1 鲁棒性）。
        let update_ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.app.update(dt);
        }))
        .is_ok();
        if !update_ok {
            log::error!("App::update panic，跳过本迭代");
            return;
        }
        // 右键菜单动画 tick（弹出/关闭轨道，与 App 状态解耦）。
        self.ctx_menu.update(dt);

        // 动态高度同步：App update 后可能请求展开/收起（TopBar 面板）。
        self.sync_preferred_height();
        self.sync_floating_heights();

        // 浮层可见性翻转（Launcher 开/合）→ 置浮层脏（翻转前不可见 → 不渲染）。
        for i in 0..self.floating.len() {
            let visible = self.app.floating_visible(i);
            if visible && !self.floating_visible_cache[i] {
                self.floating_dirty[i] = true;
            }
            self.floating_visible_cache[i] = visible;
        }

        // App 状态可能已变（焦点/文本/光标）→ 幂等 reconcile IME（无变化零成本）。
        self.reconcile_ime();

        // App 请求关闭（文件选择器等交付结果后）→ 退出主循环（进程正常收尾）。
        if self.app.should_close() {
            self.running = false;
            return;
        }

        // IME 候选窗 popup 刷新（内部脏门控：im_popup_dirty）。
        self.refresh_im_popup(qh);

        // I-3 定时器/动画脏位：App 自报「内容脏」→ 置主表面 dirty。
        if self.app.needs_redraw() {
            self.dirty = true;
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
        // 诊断：帧计数 + needs_redraw + frame_pending，写 /tmp 供会话外排查。
        self.frame_count += 1;
        if self.frame_count <= 20 || self.frame_count % 30 == 0 {
            let _ = std::fs::write(
                "/tmp/ether-harness-trace.log",
                format!(
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
            let _ = std::fs::write("/tmp/ether-kanesumi-diag.txt", lines.as_bytes());
            log::info!("{}", lines.trim_end());
        }

        let size = self.size();
        // 主表面 Scene 复用缓冲（egui PaintList）：App::render_into 就地清空重建，
        // 复用 Vec 容量，不做每帧 `Scene::default()` + 逐 push 重分配。
        // `mem::take` 移动出旧缓冲（容量保留），渲染后再放回（绕过 &mut self 分裂借用）。
        let mut scene_buf = std::mem::take(&mut self.scene_buf);
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

        // 渲染后仍在动画 → 请求下一帧（vsync 提示，I-2）。App 在 render() 内清除
        // 自身脏标记（契约），此处的 needs_redraw = 动画推进中。
        if self.app.needs_redraw() {
            self.request_next_frame(qh);
        }

        // 输出分派：layer-shell → CPU 光栅化 + SHM 提交；xdg-shell → wgpu 直出。
        // S4：本帧局部损坏矩形（CPU 光栅只重绘该区；GPU 直出全量，恒定消费）。
        let damage = self.take_damage();
        if let Some(cpu) = self.cpu.as_mut() {
            let (pw, ph) = cpu.physical_size();
            let rgba = cpu.render(&self.engine, &self.scene_buf, damage);
            if self.dmabuf_enabled {
                // dmabuf 直通：CpuRenderer → gbm bo mmap → fd → 合成器 EGLImage（零上传）。
                // 参 LINUX_DMABUF_PLAN §1。依赖合成器 dmabuf global + 客户端 gbm device。
                self.dmabuf_out
                    .commit(qh, &self.surface, &self.dmabuf, pw, ph, rgba, self.scale, damage);
            } else if let Some(shm) = self.shm.clone() {
                commit_shm_buffers(
                    &shm,
                    qh,
                    &self.surface,
                    &mut self.main_shm,
                    pw,
                    ph,
                    rgba,
                    self.scale,
                    damage,
                );
            }
        } else if let Some(r) = self.renderer.as_mut() {
            r.render(&self.engine, &self.scene_buf);
        }
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

    /// 应用新缩放：重配渲染器物理尺寸 + buffer_scale。
    fn apply_scale(&mut self, scale: f32) {
        if scale <= 0.0 || !scale.is_finite() {
            return;
        }
        self.scale = scale;
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
        if let Some(cpu) = floating.cpu.as_mut() {
            cpu.resize(floating.width, floating.height, scale);
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
        // vsync 提示（I-2）：frame 回调**不驱动渲染**，只置脏标记 → 主循环下一迭代
        // 渲染 + commit。回调丢失绝不冻结：主循环 16ms 超时兜底（TOPBAR_RENDER_REFACTOR §4.6）。
        if *surface == self.surface {
            self.frame_pending = false;
            self.dirty = true;
        } else if let Some(idx) = self.floating_idx(surface) {
            self.floating_dirty[idx] = true;
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
        if self.renderer.is_none() && self.cpu.is_none() {
            self.ensure_renderer();
            if !self.running {
                return;
            }
        }
        if !self.configured {
            self.configured = true;
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
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        self.ime_focus_surface = false;
        self.pending_ime = PendingImeBatch::default();
        self.reconcile_ime();
        // App 通知：失焦 → 关闭右键菜单 / 弹窗（失焦残留修复）。
        self.notify_focus_changed(false);
        self.dirty = true;
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
delegate_dmabuf!(Shell);

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
        _feedback: smithay_client_toolkit::dmabuf::DmabufFeedback,
    ) {
        // M5：按主设备协商 / 格式表校验（异构 GPU 安全）。当前单 GPU 直用 bo.modifier()。
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
        // create_immed 失败：合成器不接受该 fd/格式 → 本槽作废，等下一次尺寸/重建再试。
        // 不在此降级（保持简单）；严重不兼容时用户可 ETHER_DMABUF=0 回 SHM。
        log::warn!("dmabuf create_immed 失败（合成器可能未接受该格式/修饰符）");
    }

    fn released(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        buffer: &wl_buffer::WlBuffer,
    ) {
        self.dmabuf_out.mark_released(buffer);
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
                // 合成器转发的硬件按键 → 引擎。仅按下（state==Pressed）且引擎激活时处理。
                use wayland_client::WEnum;
                let is_pressed = matches!(
                    kstate,
                    WEnum::Value(wayland_client::protocol::wl_keyboard::KeyState::Pressed)
                );
                if !is_pressed || !state.im_active {
                    return;
                }
                let Some(xkb) = state.im_xkb.as_ref() else {
                    return;
                };
                let (sym, utf8) = xkb.keycode_to_sym(key);
                let logical = map_key(xkeysym::Keysym::new(sym), utf8);
                // 引擎处理按键 → 更新 preedit/commit，随即 flush 上屏。
                // 返回 false = 引擎未消费 → 经虚拟键盘重放给焦点客户端（fcitx5 同款
                // 透传：arrow/backspace/Home 等导航键须到焦点应用）。
                let consumed = state.app.ime_engine_key(logical, state.im_modifiers);
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
        while let Some(text) = self.app.ime_engine_take_commit() {
            im.commit_string(text);
            committed = true;
        }
        // 2. 待删除周边文本（退格）。double-buffered → 须 commit 才生效。
        //    App 以「字符数」请求（before=1 = 删光标前一字符）；协议按字节，
        //    CJK 字符 3 字节，据周边文本缓存换算字节数（整字符删，不劈码点）。
        let (before_chars, after_chars) = self.app.ime_engine_take_delete();
        if before_chars > 0 || after_chars > 0 {
            let before_bytes = self.chars_to_bytes_before(before_chars);
            let after_bytes = self.chars_to_bytes_after(after_chars);
            im.delete_surrounding_text(before_bytes, after_bytes);
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

    /// 光标前 `n` 字符 → 字节数（据周边文本缓存）。缺缓存时退化为 n 字节。
    fn chars_to_bytes_before(&self, n: u32) -> u32 {
        let Some((text, cursor, _anchor)) = self.im_surrounding.as_ref() else {
            return n;
        };
        let cursor = (*cursor as usize).min(text.len());
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
        let cursor = (*cursor as usize).min(text.len());
        let suffix = &text[cursor..];
        let n = (n as usize).min(suffix.chars().count());
        suffix.chars().take(n).map(|c| c.len_utf8()).sum::<usize>() as u32
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
            self.im_popup = Some(ImPopupSurface {
                surface: surface.clone(),
                popup,
                cpu: None,
                shm: ShmBuffers::default(),
                width: pw,
                height: ph,
            });
            self.im_popup_dirty = true; // 新 surface 首帧须渲染。
        }

        let Some(im_popup) = self.im_popup.as_mut() else {
            return;
        };
        if im_popup.cpu.is_none() {
            im_popup.cpu = Some(CpuRenderer::new(pw, ph, self.scale));
        } else if size_changed {
            // 尺寸变化：resize 复用（避免重建闪烁）。
            if let Some(cpu) = im_popup.cpu.as_mut() {
                cpu.resize(pw, ph, self.scale);
            }
        }
        // 候选窗内容仅在 dirty（key 变化 / 尺寸变化 / 首次）时渲染提交——
        // 每帧无条件 SHM 提交会致候选窗闪烁。
        if !self.im_popup_dirty {
            return;
        }
        self.im_popup_dirty = false;
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
        if let Some(cpu) = im_popup.cpu.as_mut() {
            let (srw, srh) = cpu.physical_size();
            let rgba = cpu.render(&self.engine, &scene, None);
            if let Some(shm) = self.shm.clone() {
                commit_shm_buffers(
                    &shm,
                    qh,
                    &im_popup.surface,
                    &mut im_popup.shm,
                    srw,
                    srh,
                    rgba,
                    self.scale,
                    None,
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
        state.pool.take().map(|p| p.destroy());
        for b in state.buffers.iter_mut() {
            b.take().map(|b| b.destroy());
        }
        state.mmap = None;
        state.in_flight = [false, false];
        state.next = 0;
        state.needs_full = [true, true];
        state.partial = [None, None];
        let fd = shm_open(expected * 2);
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
            if state.main_shm.mark_released(proxy) {
                return;
            }
            for slot in &mut state.floating_shm {
                if slot.mark_released(proxy) {
                    return;
                }
            }
            // IME 候选窗 popup 的 SHM 缓冲也须处理 release，否则双缓冲耗尽后候选窗冻结。
            if let Some(popup) = state.im_popup.as_mut()
                && popup.shm.mark_released(proxy)
            {
                return;
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
}
