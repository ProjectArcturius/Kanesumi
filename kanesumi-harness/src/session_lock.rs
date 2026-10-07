// session_lock.rs —— 会话锁客户端外壳（ext-session-lock-v1）。
//
// 参 docs/LOCKSCREEN_DESIGN.md §Ⅱ/§Ⅳ/§Ⅵ L2。锁屏客户端（ether-lock）经 `ETHER_ROLE=lock`
// 走本外壳，而非 platform::run 的 xdg/layer 单主表面模型：锁协议要求
// `lock()` → 等 `locked` → 为**每个**输出建 `ext_session_lock_surface_v1`。
//
// 安全不变量（本模块负责）：
//   1. 收到 `locked` 之前**不建表面、不提交任何缓冲**（不画内容）。
//   2. `finished`（合成器拒绝 / 强制解锁）→ 立即退出，**不**发 unlock。
//   3. 解锁唯一路径 = 应用经 `App::lock_unlock_requested()` 请求 → `unlock()` →
//      roundtrip → 退出（认证成功前应用恒返回 false）。
//   4. 锁未确认前不主动退出（等 `locked` / `finished`，或连接错误）。
//
// 渲染：沿用现有主表面 CPU 光栅器（`CpuRenderer`）+ wl_shm（Argb8888，R/B 交换，同
// `platform::commit_shm_buffers`）。每输出独立缓冲；主输出画完整界面，副输出只画背板。
// wgpu 锁屏表面不在本任务范围（`choose_renderer` 的 GPU 分支需自建 wgpu Surface，
// 属后续；锁屏为低频静态界面，CPU 光栅足够）。

use std::time::{Duration, Instant};

use kanesumi_canvas::text::TextEngine;
use kanesumi_core::Size;
use smithay_client_toolkit::reexports::{
    calloop::EventLoop, calloop_wayland_source::WaylandSource,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_session_lock, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        Capability, SeatHandler, SeatState,
        keyboard::{KeyEvent as SctkKeyEvent, KeyboardHandler, Keysym, Modifiers as SctkModifiers},
        pointer::{BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, PointerEvent, PointerEventKind, PointerHandler},
    },
    session_lock::{
        SessionLock, SessionLockHandler, SessionLockState, SessionLockSurface,
        SessionLockSurfaceConfigure,
    },
    shm::{Shm, ShmHandler, slot::SlotPool},
};
use wayland_client::{
    Connection, QueueHandle,
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
};

use crate::app::{App, InputEvent, LockEvent, Modifiers, PointerButton};
use crate::cpu_raster::CpuRenderer;

/// 锁确认状态机（纯逻辑；不变量 1/2 的单测目标，无需真实合成器）。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct LockGate {
    /// 已收到协议 `locked`。
    locked: bool,
    /// 已收到协议 `finished`。
    finished: bool,
    /// 已发出 unlock_and_destroy。
    unlock_sent: bool,
}

impl LockGate {
    /// 不变量 1：未确认 `locked` 前不得画任何锁屏内容。
    fn may_render(&self) -> bool {
        self.locked
    }

    /// 收到 `locked`。
    fn on_locked(&mut self) {
        self.locked = true;
    }

    /// 不变量 2：`finished` → 立即退出且不锁。
    fn on_finished(&mut self) {
        self.finished = true;
    }

    /// 不变量 3：仅「已确认 locked + 未 finished + 应用请求了 + 尚未发过」才允许解锁。
    fn may_unlock(&self, app_requested: bool) -> bool {
        app_requested && self.locked && !self.finished && !self.unlock_sent
    }

    /// 是否应立即退出（`finished` = 合成器已终止本次锁）。
    fn should_exit(&self) -> bool {
        self.finished
    }
}

/// 每输出槽位。
struct OutputSlot {
    output: wl_output::WlOutput,
    /// configure 后的逻辑尺寸（0 = 尚未 configure）。
    logical: (i32, i32),
    scale: i32,
    primary: bool,
    surface: Option<SessionLockSurface>,
    cpu: Option<CpuRenderer>,
    /// 共享内存池（首帧按物理尺寸建立，容纳多块在飞缓冲）。
    pool: Option<SlotPool>,
    /// 最近提交的缓冲（须存活到合成器释放；下次提交时替换）。
    buffer: Option<smithay_client_toolkit::shm::slot::Buffer>,
    /// 上一帧光栅尺寸 (逻辑宽, 逻辑高, scale)，变化才 resize。
    raster_dims: Option<(i32, i32, i32)>,
    /// 已提交过至少一帧。
    presented: bool,
}

/// 锁屏外壳状态。
struct LockShell {
    app: &'static mut dyn App,
    engine: TextEngine,
    registry_state: RegistryState,
    compositor_state: CompositorState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    /// ext-session-lock 协议状态（持有 manager 代理，存活即有效）。
    #[allow(dead_code)]
    lock_state: SessionLockState,
    /// 已创建的锁对象（`lock()` 起持有；`finished` / 解锁后丢弃）。
    lock: Option<SessionLock>,
    gate: LockGate,
    outputs: Vec<OutputSlot>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    modifiers: Modifiers,
    /// 最近一次大写锁定状态（变化才通知应用）。
    caps: bool,
    /// 启动时刻。
    started: Instant,
    /// `lock()` 调用时刻（锁确认耗时起点）。
    lock_call_at: Instant,
    /// 有内容变化待渲染（锁确认 / 输出变化 / 应用脏）。
    dirty: bool,
    /// 已上报过首帧。
    first_frame_reported: bool,
    /// 主循环退出标志。
    exit: bool,
    /// 退出码（0 正常解锁；2 `finished`）。
    exit_code: i32,
    /// 上次 `step` 时刻（dt）。
    last: Instant,
}

/// 锁屏外壳入口：接管进程直到解锁 / 被终止（不返回）。
pub fn run(app: &'static mut dyn App, engine: TextEngine) -> ! {
    let code = match run_inner(app, engine) {
        Ok(code) => code,
        Err(e) => {
            log::error!("锁屏外壳失败：{e}");
            1
        }
    };
    std::process::exit(code);
}

fn run_inner(app: &'static mut dyn App, engine: TextEngine) -> Result<i32, String> {
    let conn = Connection::connect_to_env()
        .map_err(|e| format!("Wayland 连接失败（WAYLAND_DISPLAY）：{e}"))?;
    let (globals, event_queue) = registry_queue_init::<LockShell>(&conn)
        .map_err(|e| format!("registry_queue_init 失败：{e}"))?;
    let qh = event_queue.handle();
    let mut event_loop: EventLoop<LockShell> =
        EventLoop::try_new().map_err(|e| format!("calloop EventLoop 初始化失败：{e}"))?;
    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| format!("WaylandSource 插入失败：{e}"))?;

    let compositor_state =
        CompositorState::bind(&globals, &qh).map_err(|e| format!("wl_compositor 不可用：{e}"))?;
    let output_state = OutputState::new(&globals, &qh);
    let seat_state = SeatState::new(&globals, &qh);
    let shm = Shm::bind(&globals, &qh).map_err(|e| format!("wl_shm 不可用：{e}"))?;
    let lock_state = SessionLockState::new(&globals, &qh);
    let registry_state = RegistryState::new(&globals);

    let started = Instant::now();
    let lock_call_at = Instant::now();
    // 请求锁定；`locked` / `finished` 在事件循环中到达。缺 manager → 无法锁定（环境错误）。
    let lock = lock_state
        .lock(&qh)
        .map_err(|e| format!("ext_session_lock_manager_v1 不可用：{e}"))?;

    let mut shell = LockShell {
        app,
        engine,
        registry_state,
        compositor_state,
        output_state,
        seat_state,
        shm,
        lock_state,
        lock: Some(lock),
        gate: LockGate::default(),
        outputs: Vec::new(),
        keyboard: None,
        pointer: None,
        modifiers: Modifiers::NONE,
        caps: false,
        started,
        lock_call_at,
        dirty: false,
        first_frame_reported: false,
        exit: false,
        exit_code: 0,
        last: Instant::now(),
    };

    while !shell.exit {
        event_loop
            .dispatch(Duration::from_millis(16), &mut shell)
            .map_err(|e| format!("事件循环 dispatch 失败：{e}"))?;
        shell.step(&qh, &conn);
    }
    Ok(shell.exit_code)
}

impl LockShell {
    /// 每步推进：锁未确认不画不解锁（不变量 1、4）；首批渲染只在锁确认后。
    fn step(&mut self, qh: &QueueHandle<Self>, conn: &Connection) {
        // 不变量 2：finished → 立即退出，绝不发 unlock。
        if self.gate.should_exit() {
            self.exit = true;
            self.exit_code = 2;
            return;
        }
        let now = Instant::now();
        let dt = now.saturating_duration_since(self.last).as_secs_f64();
        self.last = now;
        // 不变量 1：锁未确认前不渲染任何内容（不建表面、不提交缓冲）。
        if !self.gate.may_render() {
            return;
        }
        self.app.advance_clock(dt);
        self.app.update(dt);
        // 不变量 3：仅应用请求（认证成功）且锁已确认才解锁。
        if self.gate.may_unlock(self.app.lock_unlock_requested()) {
            self.do_unlock(conn);
            return;
        }
        if self.dirty || self.app.needs_redraw() {
            log::debug!("锁屏 step：dirty={} needs_redraw={}", self.dirty, self.app.needs_redraw());
            for i in 0..self.outputs.len() {
                self.render_output(qh, i);
            }
            self.dirty = false;
        }
    }

    /// 解锁：`unlock_and_destroy` → roundtrip → 退出（不变量 3 唯一路径）。
    fn do_unlock(&mut self, conn: &Connection) {
        if let Some(lock) = self.lock.take() {
            lock.unlock();
        }
        // roundtrip：确保 unlock 请求送达合成器（随后 destroy 不再引用）。
        let _ = conn.roundtrip();
        self.gate.unlock_sent = true;
        log::info!("锁屏：认证通过 → unlock_and_destroy，退出");
        self.exit = true;
        self.exit_code = 0;
    }

    /// 渲染单个输出并提交。主输出画完整界面，副输出只画背板。
    fn render_output(&mut self, _qh: &QueueHandle<Self>, i: usize) {
        let Some(slot) = self.outputs.get_mut(i) else {
            return;
        };
        let (lw, lh) = slot.logical;
        if lw <= 0 || lh <= 0 {
            return;
        }
        let Some(surface) = slot.surface.as_ref() else {
            return;
        };
        let scale = slot.scale.max(1);
        let primary = slot.primary;
        let size = Size::new(lw as f32, lh as f32);
        let scene = self.app.render_lock(&self.engine, size, primary);
        let dims = (lw, lh, scale);
        let cpu = slot
            .cpu
            .get_or_insert_with(|| CpuRenderer::new(size.width, size.height, scale as f32));
        if slot.raster_dims != Some(dims) {
            cpu.resize(size.width, size.height, scale as f32);
            slot.raster_dims = Some(dims);
        }
        let rgba = cpu.render(&self.engine, &scene, None).to_vec();
        let (pw, ph) = cpu.physical_size();
        if pw == 0 || ph == 0 {
            return;
        }
        if slot.pool.is_none() {
            // 池容纳约 4 块在飞缓冲（锁屏内容低频变化；SlotPool 按需增长）。
            let len = (pw as usize * ph as usize * 4).saturating_mul(4).max(1 << 20);
            match SlotPool::new(len, &self.shm) {
                Ok(p) => slot.pool = Some(p),
                Err(e) => {
                    log::error!("锁屏：wl_shm 池创建失败：{e}");
                    return;
                }
            }
        }
        let pool = slot.pool.as_mut().expect("pool 已建");
        let (buffer, canvas) = match pool.create_buffer(
            pw as i32,
            ph as i32,
            (pw * 4) as i32,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                log::error!("锁屏：缓冲创建失败：{e}");
                return;
            }
        };
        // RGBA → BGRA（Argb8888 内存序 [B,G,R,A]；与 platform::commit_shm_buffers 同款）。
        for (dst, src) in canvas.chunks_exact_mut(4).zip(rgba.chunks_exact(4)) {
            dst[0] = src[2];
            dst[1] = src[1];
            dst[2] = src[0];
            dst[3] = src[3];
        }
        let wl = surface.wl_surface();
        wl.set_buffer_scale(scale);
        if buffer.attach_to(wl).is_err() {
            log::error!("锁屏：缓冲 attach 失败");
            return;
        }
        wl.damage_buffer(0, 0, pw as i32, ph as i32);
        wl.commit();
        log::debug!("锁屏渲染输出 #{i}：{lw}x{lh}@{scale} 命令 {} 条", scene.commands.len());
        slot.buffer = Some(buffer);
        if !slot.presented {
            slot.presented = true;
            if !self.first_frame_reported {
                self.first_frame_reported = true;
                let ms = self.started.elapsed().as_millis() as u64;
                self.app.on_lock_event(LockEvent::FirstFrame { first_frame_ms: ms });
            }
        }
    }

    /// 收集输出（`locked` / 热插拔时）。锁已确认才建锁屏表面（不变量 1）。
    fn add_output(&mut self, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        if self.outputs.iter().any(|s| s.output == output) {
            return;
        }
        let info = self.output_state.info(&output);
        let name = info
            .as_ref()
            .and_then(|i| i.name.clone())
            .unwrap_or_else(|| format!("out{}", self.outputs.len()));
        let scale = info.as_ref().map(|i| i.scale_factor.max(1)).unwrap_or(1);
        // 渲染尺寸**只**由 configure 决定（协议要求先 ack_configure 再提交缓冲）；
        // 此处仅取诊断用逻辑尺寸（输出信息），不写入槽位的渲染尺寸。
        let info_logical = info.as_ref().and_then(|i| i.logical_size).unwrap_or((0, 0));
        let primary = self.outputs.is_empty();
        let index = self.outputs.len() as u32;
        let surface = if self.gate.may_render() {
            self.lock.as_ref().map(|lock| {
                let s = self.compositor_state.create_surface(qh);
                lock.create_lock_surface(s, &output, qh)
            })
        } else {
            None
        };
        self.outputs.push(OutputSlot {
            output,
            // 0,0 = 尚未 configure → 不提交缓冲（不变量 1）。
            logical: (0, 0),
            scale,
            primary,
            surface,
            cpu: None,
            pool: None,
            buffer: None,
            raster_dims: None,
            presented: false,
        });
        log::info!("锁屏输出 #{index} {name}（逻辑 {info_logical:?} scale {scale}，主={primary}）");
        self.app
            .on_lock_event(LockEvent::OutputAdded {
                index,
                name,
                logical: info_logical,
                scale,
                primary,
            });
        self.dirty = true;
    }

    fn remove_output(&mut self, output: &wl_output::WlOutput) {
        if let Some(pos) = self.outputs.iter().position(|s| &s.output == output) {
            self.outputs.remove(pos);
            for (i, s) in self.outputs.iter_mut().enumerate() {
                s.primary = i == 0;
            }
            self.app.on_lock_event(LockEvent::OutputRemoved { index: pos as u32 });
            self.dirty = true;
        }
    }

    fn primary_index(&self) -> usize {
        self.outputs.iter().position(|s| s.primary).unwrap_or(0)
    }

    /// configure：记录逻辑尺寸，标记脏（下一帧按该尺寸出缓冲）。
    fn on_configure(&mut self, surface: &SessionLockSurface, cfg: &SessionLockSurfaceConfigure) {
        let Some(slot) = self.outputs.iter_mut().find(|s| {
            s.surface
                .as_ref()
                .is_some_and(|x| x.wl_surface() == surface.wl_surface())
        }) else {
            return;
        };
        slot.logical = (cfg.new_size.0 as i32, cfg.new_size.1 as i32);
        self.dirty = true;
    }
}

// ── 协议处理器 ────────────────────────────────────────────────────────────

impl SessionLockHandler for LockShell {
    fn locked(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, session_lock: SessionLock) {
        self.gate.on_locked();
        self.lock = Some(session_lock);
        let ms = self.lock_call_at.elapsed().as_millis() as u64;
        log::info!("锁屏：locked 已收到（{ms} ms）→ 为每个输出建锁屏表面");
        self.app.on_lock_event(LockEvent::Locked { lock_confirm_ms: ms });
        self.ensure_outputs(qh);
        self.dirty = true;
    }

    fn finished(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _session_lock: SessionLock) {
        self.gate.on_finished();
        log::warn!("锁屏：收到 finished（拒绝 / 被强制解锁）→ 退出（不解锁）");
        self.app.on_lock_event(LockEvent::Finished);
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: SessionLockSurface,
        configure: SessionLockSurfaceConfigure,
        _serial: u32,
    ) {
        self.on_configure(&surface, &configure);
    }
}

impl LockShell {
    /// 锁确认后：补建缺失的锁屏表面（`locked` 前注册的输出此处在建表面）。
    fn ensure_outputs(&mut self, qh: &QueueHandle<Self>) {
        let outputs: Vec<_> = self.output_state.outputs().collect();
        for o in outputs {
            if let Some(info) = self.output_state.info(&o)
                && let Some(slot) = self.outputs.iter_mut().find(|s| s.output == o)
            {
                // 渲染尺寸只由 configure 决定（先 ack 再提交）；此处只更新 scale。
                slot.scale = info.scale_factor.max(1);
                if slot.surface.is_none()
                    && let Some(lock) = self.lock.as_ref()
                {
                    let s = self.compositor_state.create_surface(qh);
                    slot.surface = Some(lock.create_lock_surface(s, &o, qh));
                }
                continue;
            }
            self.add_output(qh, o);
        }
    }
}

impl CompositorHandler for LockShell {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
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
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
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

impl OutputHandler for LockShell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _conn: &Connection, qh: &QueueHandle<Self>, output: wl_output::WlOutput) {
        self.add_output(qh, output);
    }
    fn update_output(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        // 输出信息变化（尺度 / 尺寸）：锁屏表面存在则更新，缺失则补建。
        self.ensure_outputs(qh);
        let _ = output;
    }
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: wl_output::WlOutput,
    ) {
        self.remove_output(&output);
    }
}

impl SeatHandler for LockShell {
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
        if capability == Capability::Keyboard
            && self.keyboard.is_none()
            && let Ok(kb) = self.seat_state.get_keyboard(qh, &seat, None)
        {
            self.keyboard = Some(kb);
        }
        if capability == Capability::Pointer
            && self.pointer.is_none()
            && let Ok(p) = self.seat_state.get_pointer(qh, &seat)
        {
            self.pointer = Some(p);
        }
    }
    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        _capability: Capability,
    ) {
    }
    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: wl_seat::WlSeat) {}
}

impl KeyboardHandler for LockShell {
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
    }
    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: SctkKeyEvent,
    ) {
        // 锁未确认前不收输入（不画、不响应）。
        if !self.gate.may_render() {
            return;
        }
        let key = crate::platform::map_key(event.keysym, event.utf8);
        self.app.handle_input(InputEvent::KeyPressed {
            key,
            modifiers: self.modifiers,
        });
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
        modifiers: SctkModifiers,
        _layout: u32,
    ) {
        self.modifiers = Modifiers {
            ctrl: modifiers.ctrl,
            alt: modifiers.alt,
            shift: modifiers.shift,
            super_key: modifiers.logo,
        };
        if modifiers.caps_lock != self.caps {
            self.caps = modifiers.caps_lock;
            self.app.lock_caps_lock_changed(self.caps);
            self.dirty = true;
        }
    }
}

impl PointerHandler for LockShell {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        if !self.gate.may_render() {
            return;
        }
        let primary = self.primary_index();
        let Some(primary_surface) = self
            .outputs
            .get(primary)
            .and_then(|s| s.surface.as_ref())
            .map(|s| s.wl_surface().clone())
        else {
            return;
        };
        let mut changed = false;
        for ev in events {
            if ev.surface != primary_surface {
                continue;
            }
            let (x, y) = (ev.position.0 as f32, ev.position.1 as f32);
            let input = match &ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    Some(InputEvent::PointerMoved { x, y })
                }
                PointerEventKind::Leave { .. } => Some(InputEvent::PointerLeft),
                PointerEventKind::Press { button, .. } => map_pointer_button(*button)
                    .map(|button| InputEvent::PointerPressed {
                        x,
                        y,
                        button,
                        modifiers: self.modifiers,
                    }),
                PointerEventKind::Release { button, .. } => map_pointer_button(*button)
                    .map(|button| InputEvent::PointerReleased {
                        x,
                        y,
                        button,
                        modifiers: self.modifiers,
                    }),
                PointerEventKind::Axis { vertical, .. } => {
                    let dy = if vertical.discrete != 0 {
                        vertical.discrete as f32 * 48.0
                    } else {
                        vertical.absolute as f32
                    };
                    Some(InputEvent::Scroll {
                        x,
                        y: dy,
                        modifiers: self.modifiers,
                    })
                }
            };
            if let Some(input) = input {
                self.app.handle_input(input);
                changed = true;
            }
        }
        if changed {
            self.dirty = true;
        }
    }
}

/// 指针按钮 → 元素树按钮（未识别返回 None）。
fn map_pointer_button(button: u32) -> Option<PointerButton> {
    match button {
        BTN_LEFT => Some(PointerButton::Left),
        BTN_RIGHT => Some(PointerButton::Right),
        BTN_MIDDLE => Some(PointerButton::Middle),
        _ => None,
    }
}

impl ProvidesRegistryState for LockShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

impl ShmHandler for LockShell {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(LockShell);
delegate_output!(LockShell);
delegate_seat!(LockShell);
delegate_keyboard!(LockShell);
delegate_pointer!(LockShell);
delegate_shm!(LockShell);
delegate_session_lock!(LockShell);
delegate_registry!(LockShell);

// ── 单测：锁确认状态机（不变量 1/2/3，无需真实合成器） ─────────────────────

#[cfg(test)]
mod tests {
    use super::LockGate;

    /// 不变量 1：收到 `locked` 前不得渲染。
    #[test]
    fn no_render_before_locked() {
        let mut gate = LockGate::default();
        assert!(!gate.may_render(), "未确认前不得画任何内容");
        gate.on_locked();
        assert!(gate.may_render(), "确认后方可画");
    }

    /// 不变量 2：`finished` → 退出且**不**解锁。
    #[test]
    fn finished_exits_without_unlock() {
        let mut gate = LockGate::default();
        gate.on_finished();
        assert!(gate.should_exit(), "finished 必须立即退出");
        assert!(!gate.may_unlock(true), "finished 后即使应用请求也不得解锁");
        // 锁未确认即 finished（合成器拒绝）也不得解锁 / 不得渲染。
        assert!(!gate.may_render());
    }

    /// 不变量 3：仅「应用请求 + 已确认 + 未 finished + 未发过」才解锁，且只发一次。
    #[test]
    fn unlock_only_after_locked_and_request() {
        let mut gate = LockGate::default();
        assert!(!gate.may_unlock(true), "未确认前不得解锁");
        gate.on_locked();
        assert!(!gate.may_unlock(false), "应用未请求不得解锁");
        assert!(gate.may_unlock(true), "应用请求后允许解锁");
        gate.unlock_sent = true;
        assert!(!gate.may_unlock(true), "不得重复发 unlock");
    }

    /// 锁未确认前不主动退出（不变量 4）：默认门即不退出。
    #[test]
    fn no_exit_before_locked() {
        let gate = LockGate::default();
        assert!(!gate.should_exit());
    }
}




