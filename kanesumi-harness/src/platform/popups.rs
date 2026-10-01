// platform/popups.rs —— 子弹层宿主：把 App 申报的弹层（元素树覆盖层上的菜单 / 面板）
// 各开一个 xdg_popup 表面承载。参 Ether docs/POPUP_PLAN.md。
//
// 为什么：30px 高的 TopBar（layer-shell）画不下下拉菜单。旧做法是「把主表面临时拉高」或
// 「另开 layer 浮层 + 合成器按 namespace 硬编码路由 + /tmp socket 发 dismiss」。标准做法是
// `xdg_popup`（layer 父表面经 `zwlr_layer_surface_v1.get_popup`）：
// - 位置由 App（元素树）在「放置区 = 整个输出」里算好，positioner 只做精确落点 + 越界滑动兜底；
// - `light_dismiss` 弹层 `grab` → 合成器点外部发 `popup_done` → `App::popup_dismissed`；
// - 弹层表面的指针事件平移回主表面坐标（树坐标）交给 App，命中 / 焦点 / hover 照旧由树负责。
//
// xdg-shell 约束：抓取弹层必须叠在最上层抓取弹层之上（父 = 当前最上层抓取弹层），
// 且只能自顶向下销毁 —— 所以任何一层失效时，连同其上各层一起按逆序销毁再重建。

use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Rect, Size};
use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::shell::xdg::XdgPositioner;
use smithay_client_toolkit::shell::xdg::popup::{Popup, PopupConfigure, PopupHandler};
use wayland_client::protocol::wl_surface;
use wayland_client::{Connection, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_positioner::{
    Anchor as PosAnchor, ConstraintAdjustment, Gravity,
};

use super::{Shell, SurfaceOutput, guard};
use crate::app::{InputEvent, PopupRequest};
use crate::cpu_raster::CpuRenderer;
use crate::role::SurfaceKind;

/// 一个已开出的子弹层表面。
pub(super) struct HostedPopup {
    pub(super) key: u64,
    popup: Popup,
    /// 申报时的矩形（主表面局部 = 树坐标）。变化 → 重建。
    rect: Rect,
    /// 实际绘制尺寸（configure 给的；通常 = rect.size）。
    size: Size,
    grab: bool,
    configured: bool,
    cpu: Option<CpuRenderer>,
    scale: f32,
    pub(super) out: SurfaceOutput,
    dirty: bool,
}

impl HostedPopup {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        self.popup.wl_surface()
    }
}

impl Shell {
    /// 指针 / 键盘事件所在表面 → 弹层 key。
    pub(super) fn popup_key(&self, surface: &wl_surface::WlSurface) -> Option<u64> {
        self.popups
            .iter()
            .find(|p| p.wl_surface() == surface)
            .map(|p| p.key)
    }

    /// 是否有本进程的抓取弹层开着（键盘焦点移进弹层时，主表面的 leave 不算「失焦」）。
    pub(super) fn has_grabbing_popup(&self) -> bool {
        self.popups.iter().any(|p| p.grab)
    }

    /// 放置区（主表面局部坐标）：主表面在输出上的位置可由角色推出时 = 整个输出。
    /// 目前只为顶 / 底条（TopBar、Dock）开放 —— 窗口不知道自己在屏幕何处，弹层仍留在窗口内。
    fn popup_bounds(&self) -> Option<Rect> {
        let (ow, oh) = self.output_logical_size()?;
        let (ow, oh) = (ow as f32, oh as f32);
        match self.role.surface_kind() {
            SurfaceKind::LayerTop => Some(Rect::new(0.0, 0.0, ow, oh)),
            SurfaceKind::LayerBottom => Some(Rect::new(0.0, -(oh - self.height), ow, oh)),
            _ => None,
        }
    }

    /// 每迭代：放置区告知 App；按 `App::popups` 增删弹层表面。
    pub(super) fn sync_popups(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured || self.xdg_shell.is_none() {
            return;
        }
        let Some(bounds) = self.popup_bounds() else {
            return;
        };
        if self.popup_bounds_sent != Some(bounds) {
            self.popup_bounds_sent = Some(bounds);
            let app = &mut self.app;
            self.popups_enabled =
                guard("enable_popups", || app.enable_popups(bounds)).unwrap_or(false);
            if self.popups_enabled {
                log::info!("子弹层已启用：放置区 {bounds:?}");
            }
        }
        if !self.popups_enabled {
            return;
        }
        let app = &self.app;
        let reqs = guard("popups", || app.popups()).unwrap_or_default();
        // 自底向上比对：第一处不一致起，其上各层全部逆序销毁（xdg-shell 只许销毁最上层）。
        let keep = self
            .popups
            .iter()
            .zip(reqs.iter())
            .take_while(|(h, r)| h.key == r.key && h.rect == r.rect && h.grab == r.grab)
            .count();
        while self.popups.len() > keep {
            if let Some(p) = self.popups.pop() {
                log::debug!("子弹层销毁 key={}", p.key);
                drop(p);
                self.dirty = true;
            }
        }
        for r in reqs.into_iter().skip(keep) {
            self.create_popup(qh, r);
        }
    }

    fn create_popup(&mut self, qh: &QueueHandle<Self>, req: PopupRequest) {
        let Some(wm_base) = self.xdg_shell.as_ref() else {
            return;
        };
        let w = req.rect.size.width.round().max(1.0) as i32;
        let h = req.rect.size.height.round().max(1.0) as i32;
        // 父表面：抓取弹层叠在最上层抓取弹层之上；否则挂主表面。
        let parent = if req.grab {
            self.popups.iter().rev().find(|p| p.grab)
        } else {
            None
        };
        let (px, py) = parent.map_or((0.0, 0.0), |p| (p.rect.origin.x, p.rect.origin.y));
        let Ok(positioner) = XdgPositioner::new(wm_base) else {
            log::warn!("子弹层：xdg_positioner 创建失败");
            return;
        };
        positioner.set_size(w, h);
        positioner.set_anchor_rect(
            (req.rect.origin.x - px).round() as i32,
            (req.rect.origin.y - py).round() as i32,
            1,
            1,
        );
        positioner.set_anchor(PosAnchor::TopLeft);
        positioner.set_gravity(Gravity::BottomRight);
        // 位置已由树在放置区内算好；滑动只兜底（输出尺寸与假设不符时不越界）。
        positioner
            .set_constraint_adjustment(ConstraintAdjustment::SlideX | ConstraintAdjustment::SlideY);

        let surface = match Surface::new(&self.compositor_state, qh) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("子弹层：wl_surface 创建失败：{e}");
                return;
            }
        };
        let parent_xdg = parent.map(|p| p.popup.xdg_surface().clone());
        let popup =
            match Popup::from_surface(parent_xdg.as_ref(), &positioner, qh, surface, wm_base) {
                Ok(p) => p,
                Err(e) => {
                    log::warn!("子弹层：xdg_popup 创建失败：{e}");
                    return;
                }
            };
        if parent_xdg.is_none() {
            // 父 = 主表面。layer 主表面经 get_popup 认父（须在首次 commit 前）。
            if let Some(ls) = self.layer_surface.as_ref() {
                ls.get_popup(popup.xdg_popup());
            } else {
                log::warn!("子弹层：主表面不是 layer 表面，无法认父");
                return;
            }
        }
        if req.grab {
            match (self.seat.as_ref(), self.last_input_serial) {
                (Some(seat), Some(serial)) => popup.xdg_popup().grab(seat, serial),
                _ => log::warn!("子弹层：无输入 serial，不抓取（点外部不会自动关闭）"),
            }
        }
        let scale = self.scale.ceil().max(1.0);
        popup.wl_surface().set_buffer_scale(scale as i32);
        popup.wl_surface().commit();
        log::debug!(
            "子弹层创建 key={} rect={:?} grab={}",
            req.key,
            req.rect,
            req.grab
        );
        let mut out = SurfaceOutput::new(self.dmabuf_allowed, self.dmabuf_device.clone());
        if let Some((dev, formats)) = self.dmabuf_feedback.as_ref() {
            out.set_feedback(*dev, formats);
        }
        self.popups.push(HostedPopup {
            key: req.key,
            popup,
            rect: req.rect,
            size: req.rect.size,
            grab: req.grab,
            configured: false,
            cpu: None,
            scale,
            out,
            dirty: true,
        });
    }

    /// 渲染脏弹层：App::render_popup → CPU 光栅 → 提交。
    pub(super) fn render_popups(&mut self, qh: &QueueHandle<Self>) {
        for i in 0..self.popups.len() {
            let key = self.popups[i].key;
            let wants = {
                let app = &self.app;
                guard("popup_needs_redraw", || app.popup_needs_redraw(key)).unwrap_or(false)
            };
            let p = &mut self.popups[i];
            if !p.configured || !(p.dirty || wants) {
                continue;
            }
            p.dirty = false;
            let size = p.size;
            let cpu = p
                .cpu
                .get_or_insert_with(|| CpuRenderer::new(size.width, size.height, p.scale));
            let engine: &TextEngine = &self.engine;
            let app = &mut self.app;
            let scene = match guard("render_popup", || app.render_popup(engine, key, size)) {
                Some(s) => s,
                None => continue,
            };
            let (pw, ph) = cpu.physical_size();
            let rgba = cpu.render(engine, &scene, None);
            let surface = p.popup.wl_surface().clone();
            p.out.commit(
                qh,
                self.shm.as_ref(),
                &self.dmabuf,
                &surface,
                pw,
                ph,
                rgba,
                p.scale,
                None,
            );
        }
    }

    /// 弹层表面输入 → App（App 自行平移回树坐标）。事件到达即置脏（I-4）。
    pub(super) fn emit_popup_input(&mut self, key: u64, event: InputEvent) {
        let app = &mut self.app;
        if guard("popup_input", || app.popup_input(key, event)).is_none() {
            log::error!("App::popup_input panic，已隔离");
        }
        if let Some(p) = self.popups.iter_mut().find(|p| p.key == key) {
            p.dirty = true;
        }
        self.dirty = true;
    }

    /// 弹层缓冲 release（双缓冲槽复位）。
    pub(super) fn popup_buffer_released(
        &mut self,
        buffer: &wayland_client::protocol::wl_buffer::WlBuffer,
    ) -> bool {
        self.popups.iter_mut().any(|p| p.out.mark_released(buffer))
    }
}

impl PopupHandler for Shell {
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        popup: &Popup,
        config: PopupConfigure,
    ) {
        let Some(p) = self
            .popups
            .iter_mut()
            .find(|p| p.popup.wl_surface() == popup.wl_surface())
        else {
            return;
        };
        // 合成器给的尺寸若与申报不同（极少：输出比放置区小），按合成器尺寸画。
        if config.width > 0 && config.height > 0 {
            let size = Size::new(config.width as f32, config.height as f32);
            if size != p.size {
                p.size = size;
                if let Some(cpu) = p.cpu.as_mut() {
                    cpu.resize(size.width, size.height, p.scale);
                }
            }
        }
        p.configured = true;
        p.dirty = true;
    }

    fn done(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, popup: &Popup) {
        // 合成器关闭（点外部 / 抓取被夺）：其上各层一并销毁，App 关掉对应元素树弹层。
        let Some(i) = self
            .popups
            .iter()
            .position(|p| p.popup.wl_surface() == popup.wl_surface())
        else {
            return;
        };
        let mut closed = Vec::new();
        while self.popups.len() > i {
            if let Some(p) = self.popups.pop() {
                closed.push(p.key);
            }
        }
        for key in closed {
            let app = &mut self.app;
            let _ = guard("popup_dismissed", || app.popup_dismissed(key));
        }
        self.dirty = true;
    }
}
