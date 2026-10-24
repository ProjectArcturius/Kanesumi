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
//
// C2 一张画布：控制面板 / TopBar 与 Dock 菜单等小表面按 `KANESUMI_ONE_CANVAS=1` 走进程共享
// 的 v2 GPU 画布（逐表面 CPU 回退）；开关关闭维持既有 CPU 光栅路径。参 docs/CANVAS_PLAN.md §Ⅳ C2。

use std::time::Instant;

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Rect, Size};
use kanesumi_element::VisualAnim;
use smithay_client_toolkit::compositor::Surface;
use smithay_client_toolkit::shell::xdg::XdgPositioner;
use smithay_client_toolkit::shell::xdg::popup::{Popup, PopupConfigure, PopupHandler};
use wayland_client::protocol::wl_surface;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_positioner::{
    Anchor as PosAnchor, ConstraintAdjustment, Gravity,
};

use super::layers::proto::ether_visual_v1::{self, EtherVisualV1};
use super::{Shell, SurfaceOutput, SurfaceRenderer, guard};
use crate::app::{InputEvent, PopupRequest};
use crate::cpu_raster::CpuRenderer;
use crate::renderer_policy::{RendererKind, SurfaceClass, choose_renderer, default_expected_hz};
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
    /// CPU 光栅化器（子弹层走 SHM/dmabuf 提交）。与 `renderer` 二选一。
    cpu: Option<CpuRenderer>,
    /// wgpu 光栅化器（C2 一张画布：控制面板 / TopBar / Dock 菜单走 GPU 直出）。与 `cpu` 二选一。
    pub(super) renderer: Option<SurfaceRenderer>,
    scale: f32,
    pub(super) out: SurfaceOutput,
    dirty: bool,
    /// 下一次渲染必须整幅：首帧 / 换尺寸 / **合成器 configure 改了尺寸**。
    /// 新缓冲的像素是空的，按局部写会露出未初始化的区域（原点移动不需要 —— 场景以弹层
    /// 左上为原点，表面平移不改变缓冲像素）。
    needs_full: bool,
    /// 弹层表面的 `ether_visual_v1`（视觉偏移 + 不透明度）。挂在**弹层自己的表面**上：
    /// 合成器按 vsync 推进整块弹层的位移 / 淡出，弹层内容一帧都不重画（参 FEEL_PLAN §Ⅰ-4）。
    /// 合成器不支持 `ether_composition_v1` 时为 `None` → 直接到终值（旧行为）。
    visual: Option<EtherVisualV1>,
    /// 入场 / 退场视觉量（由元素树给出；入场起跑后清空）。
    enter: Option<VisualAnim>,
    exit: Option<VisualAnim>,
    /// 是否已退场（外壳据此停投输入、不再重复起跑）。
    closing: bool,
    /// 入场是否已起跑（首帧内容提交后起跑，只一次）。
    entered: bool,
    /// 帧节拍日志名（`弹层:<锚点角色>`，元素树给）。
    motion_name: String,
    /// 进行中的动画段名（`<motion_name> 入场|退场`）；`None` = 没有动画在跑。
    active_motion: Option<String>,
    /// 最近一次 animate 请求的时刻（诊断：合成器从请求到 Done 的真实耗时）。
    anim_sent_at: Option<Instant>,
}

/// 弹层表面 visual 的 Wayland 用户数据（key = 弹层 key，Done / Cancelled 按它回查）。
#[derive(Debug, Clone, Copy)]
pub(super) struct PopupVisual(pub(super) u64);

/// 弹层表面动画的序号：同一表面同一时刻只有一段动画，入场 / 退场各用固定值。
/// 合成器在被打断时回 `Cancelled{serial}`，据此区分「谁是过期的」。
const ENTER_SERIAL: u32 = 1;
const EXIT_SERIAL: u32 = 2;

/// 已存在弹层表面参与比对的判据（纯数据 —— 复用判据要能脱离 Wayland 单测）。
#[derive(Debug, Clone, Copy)]
pub(super) struct HostedMeta {
    key: u64,
    grab: bool,
    /// 上次同步时树申报的矩形。`rect.size` 变了 = 内容换了尺寸（不是换表面）。
    rect: Rect,
    /// 入场是否已起跑。表面复用期间必须保持 —— 归零就会重播入场。
    entered: bool,
    configured: bool,
    closing: bool,
}

/// 一个申报弹层参与比对的判据（`PopupRequest` 的裁剪）。
#[derive(Debug, Clone, Copy)]
pub(super) struct RequestMeta {
    key: u64,
    grab: bool,
    size: Size,
}

/// 一轮弹层表面同步的决策：前 `keep` 个表面复用，其余按 xdg-shell 逆序销毁后按请求新建。
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct PopupSync {
    keep: usize,
    /// 复用里尺寸变了的（同一表面换缓冲：不新建表面、不重播入场）。
    resized: Vec<usize>,
    /// 复用里该起跑入场的（只有「该 key 首次出现、且首帧内容已提交」那一轮）。
    enters: Vec<usize>,
}

impl PopupSync {
    /// 同一性判据 = `(key, grab)`。
    ///
    /// `rect` **不**进比对：内容变高 / 变矮只是同一个表面换缓冲尺寸 —— xdg_popup 没有 resize
    /// 请求，但「同一表面提交不同尺寸的缓冲」是合法的，合成器按缓冲尺寸绘制。旧判据把 `rect`
    /// 算进同一性，tb-net 展开一行就把整块面板连同上各层销毁重建，`entered` 随之归零 →
    /// 下一轮重播 300 ms 入场（用户看到「点一条 Wi-Fi，面板消失又重新滑进来」）。
    ///
    /// `grab` 仍进比对：抓取弹层的父子关系与输入路由随它变，且 xdg-shell 只许自顶向下销毁，
    /// 故 grab 一变就从该层起全部拆掉重建 —— 这是正确行为。
    ///
    /// `closing` 同样不进比对（先例）：退场是同一个表面上的动画，重建就断了接续。
    fn plan(hosted: &[HostedMeta], reqs: &[RequestMeta]) -> Self {
        let mut plan = Self::default();
        for (i, (h, r)) in hosted.iter().zip(reqs.iter()).enumerate() {
            if h.key != r.key || h.grab != r.grab {
                break;
            }
            plan.keep = i + 1;
            if h.rect.size != r.size {
                plan.resized.push(i);
            }
            // 入场只在「该 key 首次出现」时起跑；表面复用（含尺寸变化）一律保持 `entered`。
            if !h.closing && !h.entered && h.configured {
                plan.enters.push(i);
            }
        }
        plan
    }
}

impl HostedPopup {
    /// 起跑一段表面动画（先由创建时设好的起点出发；`animate` 从当前呈现值过渡到终值）。
    ///
    /// `ether_visual_v1` 的请求是**双缓冲**的：必须提交弹层表面才被合成器采纳。
    fn start_visual_anim(&mut self, a: &VisualAnim, serial: u32) -> bool {
        let Some(v) = self.visual.as_ref() else {
            return false;
        };
        let [x1, y1, x2, y2] = a.curve;
        v.animate(
            serial,
            a.duration_ms,
            a.delay_ms,
            x1 as f64,
            y1 as f64,
            x2 as f64,
            y2 as f64,
            a.to_x as f64,
            a.to_y as f64,
            a.to_opacity.clamp(0.0, 1.0) as f64,
        );
        self.popup.wl_surface().commit();
        self.anim_sent_at = Some(Instant::now());
        true
    }

    /// 动画段收尾（落一行帧节拍日志；被打断也照落，便于对照）。
    fn finish_motion(&mut self) {
        if let Some(name) = self.active_motion.take() {
            crate::perf::pacing_finish(&name, Instant::now());
        }
    }

    /// 销毁 visual（弹层表面即将销毁）：先显式 destroy，合成器把表面视觉复位。
    fn drop_visual(&mut self) {
        if let Some(v) = self.visual.take() {
            v.destroy();
        }
    }
}

impl HostedPopup {
    fn wl_surface(&self) -> &wl_surface::WlSurface {
        self.popup.wl_surface()
    }

    /// 本轮比对用的判据快照（见 [`PopupSync::plan`]）。
    fn meta(&self) -> HostedMeta {
        HostedMeta {
            key: self.key,
            grab: self.grab,
            rect: self.rect,
            entered: self.entered,
            configured: self.configured,
            closing: self.closing,
        }
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
        // 判据 = `(key, grab)`：`rect` / `closing` 不进比对（见 `PopupSync::plan`）。
        let hosted: Vec<HostedMeta> = self.popups.iter().map(HostedPopup::meta).collect();
        let req_meta: Vec<RequestMeta> = reqs
            .iter()
            .map(|r| RequestMeta {
                key: r.key,
                grab: r.grab,
                size: r.rect.size,
            })
            .collect();
        let plan = PopupSync::plan(&hosted, &req_meta);
        let keep = plan.keep;
        // 保留者里刚被标退场的：在**同一个** visual 上起跑 PopOut（不重建表面）。
        let mut done_now: Vec<u64> = Vec::new();
        for (i, r) in reqs.iter().take(keep).enumerate() {
            self.popups[i].motion_name = r.motion_name.clone();
            // 退场动画随「closing 翻转」的那一帧才下发：先收进表面状态，再起跑。
            if !self.popups[i].closing {
                self.popups[i].exit = r.exit;
            }
            // 位置记账跟着树走：rect 原点客户端改不了（positioner 固定），但在这一层之上新建
            // 抓取子弹层时父偏移取的就是它 —— 不更新会按旧原点算子层落点。
            //
            // ⚠ 原点变化**不**触发整幅重画：Scene 以弹层左上为原点（`popup_scene` 已按
            // extent 平移），表面移动不改变缓冲里的像素。旧实现把「rect 变了」一律当整幅，
            // 而行展开动画期间布局原点每帧抖动 < 0.01 pt → 每个稳定尺寸的帧也整块重光栅
            //（2026-10-24 逐帧日志实测）。尺寸变化另有 `plan.resized` 走换缓冲。
            self.popups[i].rect = r.rect;
            // 尺寸变化走同一条表面：换缓冲 + 重画，表面身份（entered / 动画 / 抓取）一格不动。
            //
            // 判据量化到**物理像素**：行展开动画末端的高度会以 0.1 pt 级步进逼近终值，
            // 逻辑尺寸每帧都「变了」但缓冲尺寸一个像素都没变 —— 换缓冲就意味着整块重光栅，
            // 而画面其实没动（2026-10-24 逐帧日志：稳定 360×280 的帧被逐帧换缓冲）。
            if plan.resized.contains(&i) {
                let sc = self.popups[i].scale.max(1.0);
                let old = self.popups[i].size;
                let quant = |v: f32| (v * sc).round() as i32;
                if quant(old.width) != quant(r.rect.size.width)
                    || quant(old.height) != quant(r.rect.size.height)
                {
                    self.resize_popup(i, r.rect);
                } else {
                    // 逻辑尺寸跟上（排版 / 场景坐标用），缓冲与光栅器不动。
                    self.popups[i].size = r.rect.size;
                }
            }
            // 入场：首帧内容**上一轮**已提交（本轮之前合成器已经导入过这个表面）→ 现在起跑。
            // 晚一帧是有意的：新表面的首次导入 / 上传要花掉一整帧，若与动画首帧挤在一起，
            // 入场的前两帧就会迟到（实测 drops=1~2）。
            if plan.enters.contains(&i) {
                self.start_popup_enter(i);
            }
            if r.closing && !self.popups[i].closing && !self.start_popup_exit(i) {
                // 没有 visual（合成器不支持图层动画）：直接到终值，立刻回报，树随即摘掉弹层。
                done_now.push(r.key);
            }
        }
        while self.popups.len() > keep {
            if let Some(mut p) = self.popups.pop() {
                log::debug!("子弹层销毁 key={}", p.key);
                // 动画没收尾就销毁（例如退场被新的开合打断）：先落日志行，再撤 visual。
                p.finish_motion();
                p.drop_visual();
                drop(p);
                self.dirty = true;
            }
        }
        for r in reqs.into_iter().skip(keep) {
            if r.closing {
                // 开合发生在同一帧内（打开即关闭）：不值得建表面再播退场 —— 直接回报，
                // 否则用户会看到一块「一闪而过」的弹层。
                done_now.push(r.key);
                continue;
            }
            self.create_popup(qh, r);
        }
        for key in done_now {
            let app = &mut self.app;
            let _ = guard("popup_motion_done", || app.popup_motion_done(key));
        }
    }

    /// 同一表面换内容尺寸：只换缓冲与光栅器，表面身份（`entered` / 动画 / 抓取）一格不动。
    ///
    /// 位置由创建时的 positioner 固定 —— xdg_popup 没有「改已有弹层几何」的请求（
    /// `xdg_popup.reposition` 落在合成器的 `reposition_request`，本仓合成器目前是空实现，
    /// 参 compositor/src/state/xdg_shell.rs），故这里只换**尺寸**：内容随高度长 / 收，
    /// 顶边留在创建时的位置。
    ///
    /// 尺寸变化那一帧落一行 `弹层:<名字> 尺寸 <w>×<h>`（名字沿用 `motion_name`）：
    /// 无头与真机都靠它核对「没建新表面、只是同一表面换了缓冲」。
    fn resize_popup(&mut self, i: usize, rect: Rect) {
        let size = rect.size;
        let p = &mut self.popups[i];
        if p.size != size {
            p.size = size;
            // 渲染器可能还没建（惰性创建读 `p.size`）；已建的按新尺寸重设缓冲。
            if let Some(cpu) = p.cpu.as_mut() {
                cpu.resize(size.width, size.height, p.scale);
            }
            if let Some(r) = p.renderer.as_mut() {
                r.resize(size.width, size.height, p.scale);
            }
        }
        // 整幅重画（不等 damage）：缩小时尤其重要 —— 新缓冲必须先画满再提交，
        // 否则露出来的那块是上一帧的旧像素（收起一行会闪一帧残影）。
        p.dirty = true;
        p.needs_full = true;
        self.dirty = true;
        // 帧节拍日志：与入场 / 退场同一份（`ether-harness-pacing.log`），按时间排在一起。
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        crate::perf::write_pacing(&format!(
            "t={ms} {} 尺寸 {}×{}\n",
            p.motion_name,
            size.width.round() as i32,
            size.height.round() as i32
        ));
    }

    /// 在已有表面上升起退场动画。返回 false = 该弹层没有 visual / 没有退场动画（调用方直接回报 Done）。
    fn start_popup_exit(&mut self, i: usize) -> bool {
        let now = Instant::now();
        let predicted = self.frame_clock.current_predicted_present();
        let name = self.popups[i].motion_name.clone();
        self.popups[i].finish_motion();
        let p = &mut self.popups[i];
        p.closing = true;
        p.enter = None;
        let Some(a) = p.exit.take() else {
            log::debug!("弹层退场：请求里没有退场动画 → 直接到终值");
            return false;
        };
        if !p.start_visual_anim(&a, EXIT_SERIAL) {
            log::debug!("弹层退场：没有 visual（合成器不支持图层动画）→ 直接到终值");
            return false;
        }
        log::debug!("弹层 {name} 起跑（{} ms，visual={}）", a.duration_ms, p.visual.is_some());
        p.active_motion = Some(format!("{name} 退场"));
        let seg = format!("{name} 退场");
        crate::perf::pacing_begin(&seg, now);
        // 起跑这一帧本身也是「动画的一帧」：记一帧，段才不会因客户端随后空闲而帧数为 0
        // （`FramePacing::finish` 对零帧的段不落行 —— 那样退场段会整条消失）。
        crate::perf::pacing_present_named(&seg, now, predicted);
        true
    }

    /// 动画段结束（合成器回报 Done / Cancelled）。
    fn finish_popup_motion(&mut self, i: usize, serial: u32, done: bool) {
        let closing = self.popups[i].closing;
        // 只认「当前阶段」的回报：入场被退场打断时，合成器会补一个 `Cancelled{ENTER}`，
        // 而退场段已经在跑（`start_popup_exit` 已把入场段收尾）—— 此时若照收，
        // 会把退场的统计截断成几帧，验收看到的「退场掉了帧」就是假的。
        let phase_matches = (serial == EXIT_SERIAL) == closing;
        if !phase_matches {
            return;
        }
        // 两段动画都要告诉树「这段结束了」：入场结束 → 树摘掉计划（不再逐帧推进）；
        // 退场结束 → 树真正移除弹层节点（下一轮 `popups()` 比对销毁表面）。
        self.popups[i].finish_motion();
        if closing && !done {
            // 退场被打断（理论上只有表面即将销毁）：等树的兜底收尾。
            return;
        }
        let key = self.popups[i].key;
        let app = &mut self.app;
        let _ = guard("popup_motion_done", || app.popup_motion_done(key));
    }

    fn create_popup(&mut self, qh: &QueueHandle<Self>, req: PopupRequest) {
        let Some(wm_base) = self.xdg_shell.as_ref() else {
            return;
        };
        // dk3-perf：新建弹层表面要付 wgpu 表面 / 管线 / 首帧上传，尖峰日志用它归因。
        self.surface_created = true;
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
        // 表面视觉：整块弹层的位移 / 淡入淡出交给合成器。起点在**首帧内容提交之前**设好，
        // 合成器第一帧就按起点呈现 —— 不会先闪一帧终值再跳回起点。
        let visual = if req.enter.is_some() || req.exit.is_some() {
            self.comp
                .manager
                .as_ref()
                .map(|m| m.get_visual(popup.wl_surface(), qh, PopupVisual(req.key)))
        } else {
            None
        };
        if let (Some(v), Some(a)) = (visual.as_ref(), req.enter) {
            v.set_offset(a.from_x as f64, a.from_y as f64);
            v.set_opacity(a.from_opacity.clamp(0.0, 1.0) as f64);
        }
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
            renderer: None,
            scale,
            out,
            dirty: true,
            needs_full: true,
            visual,
            enter: req.enter,
            exit: req.exit,
            closing: false,
            entered: false,
            motion_name: req.motion_name,
            active_motion: None,
            anim_sent_at: None,
        });
    }

    /// 子弹层渲染器惰性创建（C2）：按策略选 GPU / CPU；GPU 不可用或创建失败 → CPU。
    ///
    /// 子弹层 = 控制面板 / TopBar 菜单 / Dock 子菜单等**小表面**。缺省（开关关）维持既有 CPU
    /// 光栅；`KANESUMI_ONE_CANVAS=1` 且 GPU 可用时与主表面 / 浮层 / 候选窗同走一份 v2 GPU
    /// 画布（见下方 gpu_ok 门控）。参 docs/CANVAS_PLAN.md §Ⅳ C2。
    fn ensure_popup_renderer(&mut self, i: usize) {
        let Some((pw, ph, pscale, surf)) = self.popups.get(i).and_then(|p| {
            (p.cpu.is_none() && p.renderer.is_none())
                .then(|| (p.size.width, p.size.height, p.scale, p.popup.wl_surface().clone()))
        }) else {
            return;
        };
        let class = SurfaceClass::Floating;
        let area = ((pw * pscale).max(0.0) * (ph * pscale).max(0.0)) as u64;
        let hz = default_expected_hz(class);
        // 子弹层（控制面板 / 菜单）是 C2 授权扩展的第四类表面：此前固定 CPU 光栅，不参与
        // 面积阈值决策；开关关闭时必须逐字节保持既有 CPU 路径，故把「开关开启」与 GPU 可用
        // 相与后传给决策函数（关闭 → gpu_available=false → 必返 CPU）。参 CANVAS_PLAN §Ⅳ C2。
        let gpu_ok = self.gpu_available();
        let one_canvas = crate::renderer_policy::one_canvas();
        let kind = choose_renderer(class, area, hz, gpu_ok && one_canvas, one_canvas);
        log::info!(
            "渲染器选择：子弹层 #{}（{:.0}x{:.0}，scale {}）area={}px² hz={:.1} → {:?}（{}）",
            i,
            pw,
            ph,
            pscale,
            area,
            hz,
            kind,
            if one_canvas {
                crate::renderer_policy::decision_reason(class, area, hz, gpu_ok, one_canvas)
            } else {
                "子弹层缺省 CPU 光栅（KANESUMI_ONE_CANVAS 关闭）".to_string()
            },
        );
        if kind == RendererKind::Gpu {
            if let Some(ctx) = self.ensure_gpu_context(&surf) {
                match SurfaceRenderer::with_context(ctx, &self.conn, &surf, pw, ph, pscale, true) {
                    Ok(mut r) => {
                        // 预热：空场景提交一次，强制管线/着色器编译（消除首帧尖峰）。
                        r.render(&self.engine, &Scene::default());
                        log::info!("子弹层 wgpu 渲染器已创建（{}）", r.diagnostics());
                        self.popups[i].renderer = Some(r);
                        return;
                    }
                    Err(e) => {
                        log::warn!("子弹层 wgpu 渲染器创建失败（{e:?}），回退 CPU");
                    }
                }
            } else {
                log::warn!("子弹层 wgpu 共享上下文不可用，回退 CPU");
            }
        }
        log::info!("子弹层 CPU 光栅化器已创建（{:.0}x{:.0}）", pw, ph);
        self.popups[i].cpu = Some(CpuRenderer::new(pw, ph, pscale));
    }

    /// 渲染脏弹层：App::render_popup → 光栅（GPU 直出 / CPU→dmabuf）→ 提交。
    pub(super) fn render_popups(&mut self, qh: &QueueHandle<Self>) {
        // TextEngine::clone 零拷贝（共享字体栈与塑形缓存）：先借出一份，避免与 `&mut self.app`
        // / `ensure_popup_renderer(&mut self)` 抢 self 的借用。
        let engine_owned = self.engine.clone();
        let engine: &TextEngine = &engine_owned;
        for i in 0..self.popups.len() {
            let key = self.popups[i].key;
            let wants = {
                let app = &self.app;
                guard("popup_needs_redraw", || app.popup_needs_redraw(key)).unwrap_or(false)
            };
            if !self.popups[i].configured || !(self.popups[i].dirty || wants) {
                continue;
            }
            self.popups[i].dirty = false;
            // 先要 Scene，再问损伤：损伤由 App 比对**本帧 Scene 与上一帧**得出（元素树外壳
            // 的 `scene_damage`）—— 整块自绘的弹层根（网络卡这类）只有这样才知道「变了哪一块」。
            // 合成 Scene 只是拼已有绘制命令（不重排、不光栅），稳态 0.02 ms 量级。
            let needs_full = std::mem::take(&mut self.popups[i].needs_full);
            let size = self.popups[i].size;
            let t_render = Instant::now();
            let app = &mut self.app;
            let scene = match guard("render_popup", || app.render_popup(engine, key, size)) {
                Some(s) => s,
                None => {
                    // 没画成：整幅标记留给下一轮，别把首帧弄成局部。
                    self.popups[i].needs_full = needs_full;
                    continue;
                }
            };
            let render_ms = t_render.elapsed().as_secs_f32() * 1000.0;
            let app_damage = {
                let app = &mut self.app;
                guard("popup_damage", || app.popup_damage(key)).unwrap_or(None)
            };
            // 首帧 / 换尺寸 → 整幅；零面积 = 本帧什么都没变 → 不光栅、不提交。
            let forced = popup_full_forced();
            let damage = if needs_full || forced { None } else { app_damage };
            if damage.is_some_and(|d| d.size.width <= 0.0 || d.size.height <= 0.0) {
                // 输入 / 时钟之类的唤醒没有改动任何视觉：不产生弹层帧。
                // Scene 已作为下一帧的差异基准留存（App 侧），像素与上一帧相同。
                continue;
            }
            // 惰性创建渲染器：GPU 一张画布优先，失败回退 CPU（逐表面）。
            self.ensure_popup_renderer(i);
            // 逐帧损伤（RUST_LOG=kanesumi_harness::platform=debug）：判断「这一帧为什么画」
            // 与「局部占比多少」的唯一直接证据，真机与无头同一行格式。
            log::debug!(
                "弹层帧 key={key} size={:.0}×{:.0} dmg={} src={}",
                size.width,
                size.height,
                match damage {
                    None => "全幅".to_string(),
                    Some(d) => format!(
                        "{:.0}×{:.0}@{:.0},{:.0}（{:.1}%）",
                        d.size.width,
                        d.size.height,
                        d.origin.x,
                        d.origin.y,
                        100.0 * d.size.width * d.size.height
                            / (size.width * size.height).max(1.0)
                    ),
                },
                if forced {
                    "整幅(KANESUMI_POPUP_FULL)"
                } else if needs_full {
                    "整幅(首帧/换尺寸)"
                } else if app_damage.is_none() {
                    "整幅(场景差异)"
                } else {
                    "Scene差异"
                },
            );
            let p = &mut self.popups[i];
            let mut raster_ms = 0.0f32;
            let mut commit_ms = 0.0f32;
            let mut gpu_samples = Vec::new();
            let mut draws = None;
            let mut acquire_ms = None;
            let mut c15_stats = None;
            // dk3-perf：弹层表面同样逐帧记分段（与主表面 / 浮层同口径）。
            let mut gpu_timing: Option<crate::perf::FrameTiming> = None;
            let mut cpu_upload_bytes = 0u64;
            if let Some(cpu) = p.cpu.as_mut() {
                let (pw, ph) = cpu.physical_size();
                let t = Instant::now();
                // 局部光栅：只重画损伤区，其余像素保留上一帧（与主表面 / 浮层同一条路径）。
                let rgba = cpu.render(engine, &scene, damage);
                raster_ms = t.elapsed().as_secs_f32() * 1000.0;
                // 上传量 = 本帧写进 dmabuf / SHM 的像素字节（损伤区；None = 整幅）。
                cpu_upload_bytes = damage.map_or_else(
                    || (pw as u64) * (ph as u64) * 4,
                    |d| {
                        let sc = p.scale.max(1.0);
                        (d.size.width * sc).round().max(0.0) as u64
                            * (d.size.height * sc).round().max(0.0) as u64
                            * 4
                    },
                );
                // 子弹层同主表面 / 浮层走 dmabuf 直通（SHM 为回退）。
                let t = Instant::now();
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
                    damage,
                );
                commit_ms = t.elapsed().as_secs_f32() * 1000.0;
            } else if let Some(r) = p.renderer.as_mut() {
                // GPU（C2）：v2 增量画布 —— 部分损伤走 scissor + 保留上一帧其余像素
                //（与主表面同一条路径；整幅帧传 None）。
                let t = Instant::now();
                r.render_with_damage(engine, &scene, damage);
                raster_ms = t.elapsed().as_secs_f32() * 1000.0;
                gpu_timing = Some(r.take_frame_timing());
                gpu_samples = r.drain_gpu_samples();
                draws = r.last_draws();
                acquire_ms = r.take_acquire_ms();
                c15_stats = r.last_c15_stats();
            }
            self.perf_popup.record(render_ms, raster_ms, commit_ms);
            if let Some(n) = draws {
                self.perf_popup.record_draws(n);
            }
            if let Some(ms) = acquire_ms {
                self.perf_popup.record_acquire(ms);
            }
            for ms in gpu_samples {
                self.perf_popup.record_gpu(ms);
            }
            if let Some((inc, dmg_pct, insts)) = c15_stats {
                self.perf_popup.record_c15(inc, dmg_pct, insts);
            }
            // dk3-perf：分段耗时 + 尖峰日志（弹层往往正是「建表面那一下」的尖峰来源）。
            let cpu_path = gpu_timing.is_none();
            let mut timing = gpu_timing.unwrap_or_default();
            timing.scene_ms = render_ms;
            timing.scene_cmds = scene.commands.len() as u32;
            if cpu_path {
                timing.encode_ms = raster_ms;
                timing.submit_ms = commit_ms;
                timing.upload_bytes = cpu_upload_bytes;
            }
            timing.new_surface = std::mem::take(&mut self.surface_created);
            // 内容入场：行重画发生在主树 `frame()`（面板自绘），这里取该弹层最近一帧的
            // content 段；无入场 / 未重画 → 0。参 kanesumi_element::ContentFrameStats。
            let content = kanesumi_element::tree::popup_content_stats(key);
            // content 段 = 面板自绘（行重画）+ 该弹层表面光栅，**不含** commit。
            // 首帧常含表面/管线创建，稳态帧才是内容入场本身的代价（README 判读口径）。
            timing.content_ms = content.map_or(0.0, |s| s.content_ms + raster_ms);
            timing.content_rows = content.map_or(0, |s| s.rows);
            self.perf_popup.record_phases(&timing);
            crate::perf::maybe_spike(
                &self.perf_proc,
                &format!("{:?}", self.role),
                "popup",
                &timing,
            );
            crate::perf::pacing_present_named(
                &format!("{:?}:popup", self.role),
                Instant::now(),
                self.frame_clock.current_predicted_present(),
            );
        }
    }

    /// 首帧内容提交后起跑入场动画（只起一次；合成器不支持图层动画时立即回报 Done）。
    fn start_popup_enter(&mut self, i: usize) {
        if self.popups.get(i).is_none_or(|p| p.entered) {
            return;
        }
        let now = Instant::now();
        let p = &mut self.popups[i];
        p.entered = true;
        let Some(a) = p.enter.take() else {
            return;
        };
        let (key, animated) = (p.key, p.start_visual_anim(&a, ENTER_SERIAL));
        if !animated {
            // 没有 visual（合成器不支持 ether_composition_v1）：直接到终值，并告诉树
            // 「这段动画结束了」—— 否则树会一直以为动画在跑，白白逐帧推进。
            let app = &mut self.app;
            let _ = guard("popup_motion_done", || app.popup_motion_done(key));
            return;
        }
        let name = format!("{} 入场", p.motion_name);
        log::debug!("弹层 {name} 起跑（{} ms）", a.duration_ms);
        p.active_motion = Some(name.clone());
        let predicted = self.frame_clock.current_predicted_present();
        crate::perf::pacing_begin(&name, now);
        crate::perf::pacing_present_named(&name, now, predicted);
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

impl Dispatch<EtherVisualV1, PopupVisual> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &EtherVisualV1,
        event: ether_visual_v1::Event,
        data: &PopupVisual,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        let key = data.0;
        log::debug!(
            "弹层表面 visual 事件 key={key} {event:?}（请求后 {} ms）",
            state
                .popups
                .iter()
                .find(|p| p.key == key)
                .and_then(|p| p.anim_sent_at)
                .map_or(-1.0, |t| t.elapsed().as_secs_f32() * 1000.0) as i64
        );
        let Some(i) = state.popups.iter().position(|p| p.key == key) else {
            return;
        };
        match event {
            ether_visual_v1::Event::Done { serial } => {
                state.finish_popup_motion(i, serial, true)
            }
            ether_visual_v1::Event::Cancelled { serial } => {
                state.finish_popup_motion(i, serial, false)
            }
        }
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
                if let Some(r) = p.renderer.as_mut() {
                    r.resize(size.width, size.height, p.scale);
                }
            }
        }
        p.configured = true;
        p.dirty = true;
        // 首次 configure / 合成器给的尺寸与申报不同都要先画满一块新缓冲。
        p.needs_full = true;
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

/// 诊断 / 回退开关：`KANESUMI_POPUP_FULL=1` 时弹层表面每帧整幅重画（= 改前的提交行为）。
///
/// 两个用途：① 无头验收拿它当「改前基线」做逐字节对照（局部重画与整幅重画必须像素一致）；
/// ② 真机上局部损伤若出现残影，可一键回到整幅，不必回滚版本。
fn popup_full_forced() -> bool {
    static FORCED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FORCED.get_or_init(|| std::env::var_os("KANESUMI_POPUP_FULL").is_some_and(|v| v != "0"))
}

#[cfg(test)]
mod tests {
    //! 弹层表面**同一性判据**的单元测试（不碰 Wayland：判据是纯函数）。
    //!
    //! 回归对象：tb-net 面板展开一行会让整块面板重建表面并重播 300 ms 入场。
    //! 判据一旦把 `rect` 算进同一性就会复现；判据一旦收掉 `grab` 就会把抓取弹层的
    //! 父子关系 / 输入路由拆错。

    use super::*;

    fn hosted(key: u64, grab: bool, w: f32, h: f32) -> HostedMeta {
        HostedMeta {
            key,
            grab,
            rect: Rect::new(0.0, 30.0, w, h),
            entered: false,
            configured: true,
            closing: false,
        }
    }

    fn entered(mut h: HostedMeta) -> HostedMeta {
        h.entered = true;
        h
    }

    fn req(key: u64, grab: bool, w: f32, h: f32) -> RequestMeta {
        RequestMeta {
            key,
            grab,
            size: Size::new(w, h),
        }
    }

    /// 同一 key、同一 grab、**高度变了**：复用同一表面（不新建 / 不销毁 / 不重播入场）。
    #[test]
    fn rect_change_reuses_same_surface() {
        let h = [entered(hosted(7, true, 360.0, 400.0))];
        let r = [req(7, true, 360.0, 444.0)];
        let plan = PopupSync::plan(&h, &r);
        assert_eq!(plan.keep, 1, "同一 key + 同一 grab → 复用，不重建");
        assert_eq!(h.len() - plan.keep, 0, "销毁表面数 0");
        assert_eq!(r.len() - plan.keep, 0, "新建表面数 0");
        assert_eq!(plan.resized, vec![0], "尺寸变化走同一条表面");
        assert!(plan.enters.is_empty(), "复用不得再起跑入场（entered 保持）");
    }

    /// 收起一行（变矮）同样是「同一表面换尺寸」，不是重建。
    #[test]
    fn rect_shrink_also_reuses_same_surface() {
        let h = [entered(hosted(7, true, 360.0, 444.0))];
        let r = [req(7, true, 360.0, 400.0)];
        let plan = PopupSync::plan(&h, &r);
        assert_eq!((plan.keep, plan.resized.as_slice()), (1, &[0][..]));
        assert!(plan.enters.is_empty());
    }

    /// 尺寸没变 → 既不复用触发重画，也不入场。
    #[test]
    fn unchanged_rect_is_noop() {
        let h = [entered(hosted(7, true, 360.0, 400.0))];
        let r = [req(7, true, 360.0, 400.0)];
        let plan = PopupSync::plan(&h, &r);
        assert_eq!(plan.keep, 1);
        assert!(plan.resized.is_empty(), "尺寸没变不重设缓冲");
        assert!(plan.enters.is_empty());
    }

    /// 入场只在「该 key 首次出现、且首帧内容已提交」那一轮起跑一次。
    #[test]
    fn enter_starts_once_per_key() {
        let h = [hosted(7, true, 360.0, 400.0)];
        let r = [req(7, true, 360.0, 400.0)];
        assert_eq!(PopupSync::plan(&h, &r).enters, vec![0], "首次出现 → 起跑入场");

        // 入场已起跑 + 内容换尺寸：不得再起跑（这正是「面板重滑一次」的回归点）。
        let h2 = [entered(hosted(7, true, 360.0, 400.0))];
        let r2 = [req(7, true, 360.0, 444.0)];
        assert!(
            PopupSync::plan(&h2, &r2).enters.is_empty(),
            "尺寸变化那一轮不得有第二条入场"
        );
    }

    /// 首帧内容还没提交（未 configure）→ 晚一轮再起跑入场。
    #[test]
    fn enter_waits_for_first_commit() {
        let mut h = hosted(7, true, 360.0, 400.0);
        h.configured = false;
        let r = [req(7, true, 360.0, 400.0)];
        assert!(PopupSync::plan(&[h], &r).enters.is_empty());
        h.configured = true;
        assert_eq!(PopupSync::plan(&[h], &r).enters, vec![0]);
    }

    /// `grab` 变化仍按原判据重建（拆开 / 重建是正确行为：父子关系与输入路由都变了）。
    #[test]
    fn grab_change_rebuilds() {
        let h = [entered(hosted(7, true, 360.0, 400.0))];
        let r = [req(7, false, 360.0, 400.0)];
        let plan = PopupSync::plan(&h, &r);
        assert_eq!(plan.keep, 0, "grab 变化 → 该层起全部重建");
        assert_eq!(h.len() - plan.keep, 1, "销毁 1 个表面");
        assert_eq!(r.len() - plan.keep, 1, "新建 1 个表面");
        assert!(plan.resized.is_empty());
    }

    /// key 变化（换了另一个弹层）→ 重建。
    #[test]
    fn key_change_rebuilds() {
        let h = [entered(hosted(7, true, 360.0, 400.0))];
        let r = [req(8, true, 360.0, 400.0)];
        assert_eq!(PopupSync::plan(&h, &r).keep, 0);
    }

    /// xdg-shell 只许自顶向下销毁：下层尺寸变了照旧复用，只有**其上的**层拆掉重建。
    #[test]
    fn upper_layer_change_rebuilds_only_above_it() {
        let h = [
            entered(hosted(1, true, 200.0, 100.0)),
            entered(hosted(2, true, 120.0, 80.0)),
        ];
        let reqs = [req(1, true, 200.0, 140.0), req(3, true, 120.0, 80.0)];
        let plan = PopupSync::plan(&h, &reqs);
        assert_eq!(plan.keep, 1, "下层（key=1）仍复用");
        assert_eq!(2 - plan.keep, 1, "只销毁其上的第 2 层");
        assert_eq!(plan.resized, vec![0], "下层尺寸变化仍在同一表面上生效");
        assert!(plan.enters.is_empty(), "两层都已入场过");
    }

    /// 退场期间不改判据：closing 不进同一性，且退场中不再起跑入场。
    #[test]
    fn closing_not_part_of_identity() {
        let mut h = entered(hosted(7, true, 360.0, 400.0));
        h.closing = true;
        let r = [req(7, true, 360.0, 400.0)];
        let plan = PopupSync::plan(&[h], &r);
        assert_eq!(plan.keep, 1, "退场是同一表面上的动画");
        assert!(plan.enters.is_empty());
    }
}
