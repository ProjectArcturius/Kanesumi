// platform/layers.rs —— 合成图层的 Wayland 实现（G3-b）。参 Ether docs/GPU_COMPOSITION_PLAN.md G3、`crate::layers`。
//
// 每个图层 = 父表面（主表面或浮层）下的一个 desync 子表面：内容由 CpuRenderer 光栅**一次**提交成缓冲，
// 之后的位移 / 不透明度动画经 `ether_visual_v1` 交给合成器按 vsync 推进，客户端不再重画、不再提交。
// 子表面设空输入区域：图层纯视觉，指针事件落到父表面，命中照旧由 App 按布局判定（COMPOSITION 契约 12）。
// 合成器不提供 `ether_composition_v1` 时回落：偏移用 set_position 模拟，不透明度忽略，动画直接跳终值并回报 Done。

use kanesumi_canvas::Scene;
use kanesumi_core::Rect;
use smithay_client_toolkit::subcompositor::SubcompositorState;
use wayland_client::protocol::{wl_subsurface::WlSubsurface, wl_surface};
use wayland_client::{Connection, Dispatch, QueueHandle};

use super::{Shell, SurfaceOutput};
use crate::cpu_raster::CpuRenderer;
use crate::layers::{LayerCommand, LayerEvent, LayerId, LayerParent};

// ── 协议客户端绑定（与 Ether compositor/protocol/ether-composition-v1.xml 同一份）──

pub mod proto {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocol/ether-composition-v1.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocol/ether-composition-v1.xml");
}

pub(super) use proto::ether_composition_manager_v1::EtherCompositionManagerV1;
use proto::ether_scroller_v1::EtherScrollerV1;
use proto::ether_visual_v1::{self, EtherVisualV1};

/// 一个已创建的图层。
pub(super) struct CompLayer {
    id: LayerId,
    parent: wl_surface::WlSurface,
    surface: wl_surface::WlSurface,
    subsurface: WlSubsurface,
    visual: Option<EtherVisualV1>,
    /// 滚动器（fp4b）：Some 时本图层是滚动器内容子表面，位置由合成器 visual offset 驱动。
    pub(super) scroller: Option<EtherScrollerV1>,
    pub(super) out: SurfaceOutput,
    cpu: Option<CpuRenderer>,
    rect: Rect,
    scale: f32,
    /// 回落模式下模拟的视觉偏移（有协议时不用）。
    offset: (f32, f32),
    /// 上次提交的内容（只用于算提交损伤；`None` = 还没有可比对的上一帧 → 整幅）。
    last_scene: Option<Scene>,
    /// 光栅好但未能提交的内容（两槽缓冲都在合成器手里 → 本次提交被跳过）。
    /// 图层内容只在变化时交一次，跳过即永久丢失 —— 2026-10-17 Launcher 背板全透明的根因：
    /// 背板第 3、4 次内容（不透明的那两次）都被跳过，合成器一直显示首帧暂隐时的透明内容。
    /// 缓冲释放时与每帧处理图层命令时补交。
    pending: Option<Scene>,
    /// 滚器实现窗口同理（fp4b）：提交被跳过时存 (realized, scene)，缓冲释放 / 每帧补交。
    pending_realize: Option<(Rect, Scene)>,
}

/// Shell 持有的图层总状态。
#[derive(Default)]
pub(super) struct Composition {
    pub(super) manager: Option<EtherCompositionManagerV1>,
    pub(super) subcompositor: Option<SubcompositorState>,
    pub(super) layers: Vec<CompLayer>,
    unsupported_reported: bool,
}

impl Composition {
    pub(super) fn new(manager: Option<EtherCompositionManagerV1>, subcompositor: Option<SubcompositorState>) -> Self {
        Self { manager, subcompositor, layers: Vec::new(), unsupported_reported: false }
    }

    /// 认领 `wl_buffer.release`（图层输出缓冲）。
    pub(super) fn mark_released(&mut self, buffer: &wayland_client::protocol::wl_buffer::WlBuffer) -> bool {
        self.layers.iter_mut().any(|l| l.out.mark_released(buffer))
    }
}

impl Shell {
    /// 每帧（App::update 之后）取走 App 的图层命令并执行。
    pub(super) fn process_layer_commands(&mut self, qh: &QueueHandle<Shell>) {
        self.retry_pending_layers(qh);
        let cmds = self.app.take_layer_commands();
        if cmds.is_empty() {
            return;
        }
        if self.comp.manager.is_none() && !self.comp.unsupported_reported {
            self.comp.unsupported_reported = true;
            log::warn!("合成器不支持 ether_composition_v1 → 图层动画回落为直接跳终值");
            self.app.on_layer_event(LayerEvent::Unsupported);
        }
        for c in cmds {
            self.apply_layer_command(c, qh);
        }
    }

    /// 补交被跳过的图层内容（缓冲释放 / 每帧调用；无待补交时零成本）。
    pub(super) fn retry_pending_layers(&mut self, qh: &QueueHandle<Shell>) {
        let mut realize: Vec<(LayerId, Rect, Scene)> = Vec::new();
        for l in self.comp.layers.iter() {
            if let Some((r, s)) = &l.pending_realize {
                realize.push((l.id, *r, s.clone()));
            }
        }
        for (id, r, s) in realize {
            self.realize_scroller(id, &r, &s, qh);
        }
        let ids: Vec<LayerId> = self
            .comp
            .layers
            .iter()
            .filter(|l| l.pending.is_some())
            .map(|l| l.id)
            .collect();
        for id in ids {
            let Some(i) = self.layer_index(id) else { continue };
            if let Some(scene) = self.comp.layers[i].pending.take() {
                self.set_layer_content(id, &scene, qh);
            }
        }
    }

    fn layer_index(&self, id: LayerId) -> Option<usize> {
        self.comp.layers.iter().position(|l| l.id == id)
    }

    fn apply_layer_command(&mut self, c: LayerCommand, qh: &QueueHandle<Shell>) {
        match c {
            LayerCommand::Create { id, parent, rect } => self.create_layer(id, parent, rect, qh),
            LayerCommand::SetContent { id, scene } => self.set_layer_content(id, &scene, qh),
            LayerCommand::SetRect { id, rect } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &mut self.comp.layers[i];
                l.rect = rect;
                place(l);
                if let Some(cpu) = l.cpu.as_mut() {
                    cpu.resize(rect.size.width, rect.size.height, l.scale);
                }
            }
            LayerCommand::SetOffset { id, x, y } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &mut self.comp.layers[i];
                match &l.visual {
                    Some(v) => {
                        v.set_offset(x as f64, y as f64);
                        l.surface.commit();
                    }
                    None => {
                        l.offset = (x, y);
                        place(l);
                    }
                }
            }
            LayerCommand::SetOpacity { id, opacity } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &self.comp.layers[i];
                if let Some(v) = &l.visual {
                    v.set_opacity(opacity.clamp(0.0, 1.0) as f64);
                    l.surface.commit();
                }
            }
            LayerCommand::Animate { id, anim } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &mut self.comp.layers[i];
                match l.visual.clone() {
                    Some(v) => {
                        let [x1, y1, x2, y2] = anim.curve;
                        v.animate(
                            anim.serial,
                            anim.duration_ms,
                            anim.delay_ms,
                            x1 as f64,
                            y1 as f64,
                            x2 as f64,
                            y2 as f64,
                            anim.to_x as f64,
                            anim.to_y as f64,
                            anim.to_opacity.clamp(0.0, 1.0) as f64,
                        );
                        l.surface.commit();
                    }
                    None => {
                        l.offset = (anim.to_x, anim.to_y);
                        place(l);
                        for e in crate::layers::fallback_events(id, &anim) {
                            self.app.on_layer_event(e);
                        }
                    }
                }
            }
            LayerCommand::SetClip { id, insets } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &self.comp.layers[i];
                if let Some(v) = &l.visual
                    && wayland_client::Proxy::version(v) >= crate::layers::CLIP_SINCE_VERSION
                {
                    v.set_clip(insets.top as f64, insets.right as f64, insets.bottom as f64, insets.left as f64);
                    l.surface.commit();
                }
            }
            LayerCommand::AnimateClip { id, anim } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &self.comp.layers[i];
                match l.visual.clone() {
                    Some(v) if wayland_client::Proxy::version(&v) >= crate::layers::CLIP_SINCE_VERSION => {
                        let [x1, y1, x2, y2] = anim.curve;
                        let t = anim.to;
                        v.animate_clip(
                            anim.serial,
                            anim.duration_ms,
                            anim.delay_ms,
                            x1 as f64,
                            y1 as f64,
                            x2 as f64,
                            y2 as f64,
                            t.top as f64,
                            t.right as f64,
                            t.bottom as f64,
                            t.left as f64,
                        );
                        l.surface.commit();
                    }
                    // 合成器只有 v1 / 无合成图层：不裁，立即完成（图层完整可见，没有推开动画）。
                    _ => {
                        for e in crate::layers::fallback_clip_events(id, &anim) {
                            self.app.on_layer_event(e);
                        }
                    }
                }
            }
            LayerCommand::Destroy { id } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = self.comp.layers.remove(i);
                if let Some(v) = l.visual {
                    v.destroy();
                }
                if let Some(s) = l.scroller {
                    s.destroy();
                }
                l.subsurface.destroy();
                l.surface.destroy();
                l.parent.commit();
            }
            // ── 合成滚动（fp4b）。参 crate::layers 头注与 Ether fp4-compositor-scroll.md §Ⅲ ──
            LayerCommand::CreateScroller { id, parent, viewport, extent, background } => {
                self.create_scroller(id, parent, viewport, extent, background, qh)
            }
            LayerCommand::SetScrollerGeometry { id, viewport, extent, background } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = &self.comp.layers[i];
                let Some(s) = &l.scroller else { return };
                s.set_viewport(
                    viewport.origin.x.round() as i32,
                    viewport.origin.y.round() as i32,
                    viewport.size.width.round() as i32,
                    viewport.size.height.round() as i32,
                );
                s.set_extent(extent.0.round() as i32, extent.1.round() as i32);
                s.set_background(background);
                // 双缓冲请求随内容表面 commit 生效；子表面 desync，直接 commit。
                l.surface.commit();
            }
            LayerCommand::RealizeScroller { id, realized, scene } => {
                self.realize_scroller(id, &realized, &scene, qh)
            }
            LayerCommand::ScrollerScrollBy { id, serial, dy } => {
                let Some(i) = self.layer_index(id) else { return };
                if let Some(s) = &self.comp.layers[i].scroller {
                    s.scroll_by(serial, 0.0, dy as f64);
                }
            }
            LayerCommand::ScrollerScrollTo { id, serial, y, animate } => {
                let Some(i) = self.layer_index(id) else { return };
                if let Some(s) = &self.comp.layers[i].scroller {
                    s.scroll_to(serial, 0.0, y as f64, if animate { 1 } else { 0 });
                }
            }
        }
    }

    fn create_layer(&mut self, id: LayerId, parent: LayerParent, rect: Rect, qh: &QueueHandle<Shell>) {
        if self.layer_index(id).is_some() {
            log::warn!("图层 {id:?} 已存在，忽略重复 Create");
            return;
        }
        let Some(sub) = self.comp.subcompositor.as_ref() else {
            log::warn!("合成器无 wl_subcompositor，无法创建图层 {id:?}");
            return;
        };
        let (parent_surface, scale) = match parent {
            LayerParent::Main => (self.surface.clone(), self.scale),
            LayerParent::Floating(i) => match self.floating.get(i) {
                Some(f) => (f.surface.clone(), f.scale),
                None => {
                    log::warn!("图层 {id:?} 的父浮层 {i} 不存在");
                    return;
                }
            },
        };
        let (subsurface, surface) = sub.create_subsurface(parent_surface.clone(), qh);
        // desync：图层自己的 commit 立即生效（内容只交一次，动画请求不必等父表面提交）。
        subsurface.set_desync();
        // 空输入区域：指针穿透到父表面。
        let region = self.compositor_state.wl_compositor().create_region(qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        surface.set_buffer_scale(scale.round().max(1.0) as i32);
        let visual = self.comp.manager.as_ref().map(|m| m.get_visual(&surface, qh, id));
        let out = SurfaceOutput::new(self.dmabuf_allowed, self.dmabuf_device.clone());
        let mut l = CompLayer {
            id,
            parent: parent_surface,
            surface,
            subsurface,
            visual,
            scroller: None,
            out,
            cpu: None,
            rect,
            scale,
            offset: (0.0, 0.0),
            last_scene: None,
            pending: None,
            pending_realize: None,
        };
        place(&mut l);
        self.comp.layers.push(l);
        // dk3-perf：新建图层子表面（建 CpuRenderer / 首帧上传）单独标出来。
        self.surface_created = true;
    }

    /// 建滚动器图层（fp4b）：内容子表面位置固定 (0,0)，摆放全部由合成器 visual offset
    /// 驱动（`视口原点 + realized 原点 − 滚动位置`）；轴事件由合成器按视口认领。
    /// manager 版本 < 3（无 `get_scroller`）时不建并回发 `Unsupported` —— 树侧收到后
    /// 把全部滚动器标记撤下，控件回自绘路径。
    fn create_scroller(
        &mut self,
        id: LayerId,
        parent: LayerParent,
        viewport: Rect,
        extent: (f32, f32),
        background: u32,
        qh: &QueueHandle<Shell>,
    ) {
        if self.layer_index(id).is_some() {
            log::warn!("滚动器图层 {id:?} 已存在，忽略重复 CreateScroller");
            return;
        }
        let v3 = self
            .comp
            .manager
            .as_ref()
            .is_some_and(|m| wayland_client::Proxy::version(m) >= crate::layers::SCROLLER_SINCE_VERSION);
        if !v3 {
            if !self.comp.unsupported_reported {
                self.comp.unsupported_reported = true;
                log::warn!("合成器无 ether_composition_v1 v3 → 合成滚动回退为应用线程自绘");
                self.app.on_layer_event(LayerEvent::Unsupported);
            }
            return;
        }
        let Some(sub) = self.comp.subcompositor.as_ref() else {
            log::warn!("合成器无 wl_subcompositor，无法创建滚动器图层 {id:?}");
            return;
        };
        let (parent_surface, scale) = match parent {
            LayerParent::Main => (self.surface.clone(), self.scale),
            LayerParent::Floating(i) => match self.floating.get(i) {
                Some(f) => (f.surface.clone(), f.scale),
                None => return,
            },
        };
        let (subsurface, surface) = sub.create_subsurface(parent_surface.clone(), qh);
        // desync：滚动器的 set_viewport / set_realized 双缓冲随本表面 commit 原子生效。
        subsurface.set_desync();
        // 位置 (0,0)：摆放交给合成器（visual offset = 视口原点 + realized 原点 − 滚动位置）。
        subsurface.set_position(0, 0);
        // 空输入区域：指针事件照常落在父表面，命中由 App 按布局换算（协议约定）。
        let region = self.compositor_state.wl_compositor().create_region(qh, ());
        surface.set_input_region(Some(&region));
        region.destroy();
        surface.set_buffer_scale(scale.round().max(1.0) as i32);
        let Some(m) = self.comp.manager.clone() else { return };
        let visual = m.get_visual(&surface, qh, id);
        let scroller = m.get_scroller(&visual, qh, id);
        scroller.set_viewport(
            viewport.origin.x.round() as i32,
            viewport.origin.y.round() as i32,
            viewport.size.width.round() as i32,
            viewport.size.height.round() as i32,
        );
        scroller.set_extent(extent.0.round() as i32, extent.1.round() as i32);
        scroller.set_axes(1);
        scroller.set_background(background);
        let out = SurfaceOutput::new(self.dmabuf_allowed, self.dmabuf_device.clone());
        let l = CompLayer {
            id,
            parent: parent_surface,
            surface,
            subsurface,
            visual: Some(visual),
            scroller: Some(scroller),
            out,
            cpu: None,
            rect: viewport,
            scale,
            offset: (0.0, 0.0),
            last_scene: None,
            pending: None,
            pending_realize: None,
        };
        self.comp.layers.push(l);
        // 首帧几何（视口 / extent / 背景色）随这次空 commit 生效。
        self.comp.layers.last_mut().unwrap().surface.commit();
        self.surface_created = true;
    }

    /// 提交滚动器实现窗口：缓冲 resize → 光栅 → `set_realized` → commit（一次原子生效）。
    fn realize_scroller(&mut self, id: LayerId, realized: &Rect, scene: &Scene, qh: &QueueHandle<Shell>) {
        let Some(i) = self.layer_index(id) else { return };
        let t = std::time::Instant::now();
        let Shell { engine, comp, shm, dmabuf, .. } = self;
        let l = &mut comp.layers[i];
        if l.scroller.is_none() {
            return;
        }
        // 协议要求缓冲逻辑尺寸 == realized 尺寸：两边都取整数（round），一致。
        let (w, h) = (realized.size.width.round().max(1.0), realized.size.height.round().max(1.0));
        let cpu = l.cpu.get_or_insert_with(|| CpuRenderer::new(w, h, l.scale));
        cpu.resize(w, h, l.scale);
        let (pw, ph) = cpu.physical_size();
        let rgba = cpu.render(engine, scene, None);
        let encode_ms = t.elapsed().as_secs_f32() * 1000.0;
        l.last_scene = None;
        let t = std::time::Instant::now();
        // 先 set_realized（入 pending），attach + commit 时与新缓冲原子生效。
        let committed = l.out.commit(qh, shm.as_ref(), dmabuf, &l.surface, pw, ph, rgba, l.scale, None);
        if committed {
            if let Some(s) = &l.scroller {
                s.set_realized(
                    realized.origin.x.round() as i32,
                    realized.origin.y.round() as i32,
                    w as i32,
                    h as i32,
                );
            }
            // 双缓冲请求随这次 commit 生效（bad_realized 校验读的就是本次新缓冲）。
            l.surface.commit();
            l.pending_realize = None;
        } else {
            // 两槽缓冲都在合成器手里 → 本次跳过；树侧已把该 Scene 记为已提交，
            // 不补交就永久丢这一窗口。缓冲释放 / 下帧处理命令时重试（同图层内容补交）。
            l.pending_realize = Some((*realized, scene.clone()));
        }
        let submit_ms = t.elapsed().as_secs_f32() * 1000.0;
        let timing = crate::perf::FrameTiming {
            encode_ms,
            submit_ms,
            scene_cmds: scene.commands.len() as u32,
            upload_bytes: (pw as u64) * (ph as u64) * 4,
            new_surface: std::mem::take(&mut self.surface_created),
            ..Default::default()
        };
        let p = self.perf_layers.entry(id.0).or_default();
        p.record(0.0, encode_ms, submit_ms);
        p.record_phases(&timing);
        crate::perf::maybe_spike(
            &self.perf_proc,
            &format!("{:?}", self.role),
            &format!("scroller{}", id.0),
            &timing,
        );
    }

    /// 光栅内容并提交（只在内容变化时由 App 触发；动画期间不会走到这里）。
    fn set_layer_content(&mut self, id: LayerId, scene: &Scene, qh: &QueueHandle<Shell>) {
        let Some(i) = self.layer_index(id) else { return };
        let t = std::time::Instant::now();
        let Shell { engine, comp, shm, dmabuf, .. } = self;
        let l = &mut comp.layers[i];
        let (w, h) = (l.rect.size.width.max(1.0), l.rect.size.height.max(1.0));
        let cpu = l.cpu.get_or_insert_with(|| CpuRenderer::new(w, h, l.scale));
        let (pw, ph) = cpu.physical_size();
        let rgba = cpu.render(engine, scene, None);
        let encode_ms = t.elapsed().as_secs_f32() * 1000.0;
        // 内容只在变化时提交，但提交缓冲默认整幅拷贝：Dock 悬停只改一个图标的颜色，
        // 也要把 1.87 MB 全写一遍（真机尖峰里 submit 占了大头）。用场景差异算出损伤区，
        // 只写变化的行；差异算不出（结构变化 / 首帧）就退回整幅，语义不变。
        let damage = scene_damage(l.last_scene.as_ref(), scene, l.rect.size);
        l.last_scene = Some(scene.clone());
        let t = std::time::Instant::now();
        let committed = l.out.commit(
            qh,
            shm.as_ref(),
            dmabuf,
            &l.surface,
            pw,
            ph,
            rgba,
            l.scale,
            damage,
        );
        let submit_ms = t.elapsed().as_secs_f32() * 1000.0;
        // 跳过 → 记下最新内容，缓冲释放后补交（只留最新一份，旧的作废）。
        l.pending = if committed { None } else { Some(scene.clone()) };
        // dk3-perf：图层内容帧（Dock 图标 / 亚克力、Launcher 背板都在这里光栅）。
        // 与父表面分开记 —— 它是「Dock 内容重画」，不是父表面场景变慢。
        let timing = crate::perf::FrameTiming {
            encode_ms,
            submit_ms,
            scene_cmds: scene.commands.len() as u32,
            upload_bytes: (pw as u64) * (ph as u64) * 4,
            new_surface: std::mem::take(&mut self.surface_created),
            ..Default::default()
        };
        let p = self.perf_layers.entry(id.0).or_default();
        p.record(0.0, encode_ms, submit_ms);
        p.record_phases(&timing);
        crate::perf::maybe_spike(
            &self.perf_proc,
            &format!("{:?}", self.role),
            &format!("layer{}", id.0),
            &timing,
        );
    }
}

/// 一帧场景里一条命令的绘制外框（逻辑坐标）。裁剪命令返回 None（不直接绘制）。
fn cmd_bounds(c: &kanesumi_canvas::SceneCommand) -> Option<Rect> {
    use kanesumi_canvas::SceneCommand as C;
    match c {
        C::FillRect { rect, .. } | C::StrokeRect { rect, .. } | C::Text { rect, .. } | C::Image { rect, .. } => Some(*rect),
        C::Arc { center, radius, thickness, .. } => {
            let r = radius + thickness / 2.0;
            Some(Rect::new(center.x - r, center.y - r, r * 2.0, r * 2.0))
        }
        C::Triangle { p0, p1, p2, .. } => {
            let x0 = p0.x.min(p1.x).min(p2.x);
            let y0 = p0.y.min(p1.y).min(p2.y);
            let x1 = p0.x.max(p1.x).max(p2.x);
            let y1 = p0.y.max(p1.y).max(p2.y);
            Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
        }
        C::PushClip { .. } | C::PopClip => None,
    }
}

/// 命令是否等价（图像按 `Arc` 身份比较：`Image` 的 `PartialEq` 会逐字节比对整幅位图，
/// 全屏背板 23 MB，绝不能用在热路径上）。
fn cmd_same(a: &kanesumi_canvas::SceneCommand, b: &kanesumi_canvas::SceneCommand) -> bool {
    use kanesumi_canvas::SceneCommand as C;
    match (a, b) {
        (
            C::Image { rgba: ra, width: wa, height: ha, rect: r1, tint: t1, opacity: o1 },
            C::Image { rgba: rb, width: wb, height: hb, rect: r2, tint: t2, opacity: o2 },
        ) => {
            std::sync::Arc::ptr_eq(ra, rb)
                && wa == wb
                && ha == hb
                && r1 == r2
                && t1 == t2
                && o1 == o2
        }
        _ => a == b,
    }
}

/// 两次图层内容之间的**提交损伤**（逻辑坐标）：`None` = 整幅（首帧 / 结构变化 / 裁剪变化）。
///
/// 只用于决定「写多少行缓冲」，不影响光栅结果；算不准就退回整幅，不会画错。
fn scene_damage(prev: Option<&Scene>, next: &Scene, size: kanesumi_core::Size) -> Option<Rect> {
    let prev = prev?;
    if prev.commands.len() != next.commands.len() {
        return None;
    }
    let mut out: Option<Rect> = None;
    for (a, b) in prev.commands.iter().zip(next.commands.iter()) {
        if cmd_same(a, b) {
            continue;
        }
        // 裁剪变了会影响其后所有命令的可见区 → 直接整幅，避免漏刷。
        let is_clip = |c: &kanesumi_canvas::SceneCommand| {
            matches!(c, kanesumi_canvas::SceneCommand::PushClip { .. } | kanesumi_canvas::SceneCommand::PopClip)
        };
        if is_clip(a) || is_clip(b) {
            return None;
        }
        for c in [a, b] {
            if let Some(r) = cmd_bounds(c) {
                out = Some(match out {
                    Some(u) => super::union_rect(u, r),
                    None => r,
                });
            }
        }
    }
    let d = out?;
    // 外扩 1 px：描边 / AA / 图标边缘不留接缝；裁到表面内。
    let d = Rect::new(d.origin.x - 1.0, d.origin.y - 1.0, d.size.width + 2.0, d.size.height + 2.0);
    let clipped = d.intersect(Rect::new(0.0, 0.0, size.width, size.height))?;
    // 大面积变化（> 60%）直接整幅：分行循环 + buffer-age 回补还不如一次整幅拷贝。
    let full = size.width * size.height;
    if full > 0.0 && clipped.size.width * clipped.size.height > full * 0.6 {
        return None;
    }
    Some(clipped)
}

/// 子表面位置 = 图层矩形原点（+ 回落模式下的模拟偏移）。位置随父表面下次提交生效 → 立即空提交父表面。
fn place(l: &mut CompLayer) {
    let (ox, oy) = l.offset;
    l.subsurface
        .set_position((l.rect.origin.x + ox).round() as i32, (l.rect.origin.y + oy).round() as i32);
    l.parent.commit();
}

// ── 协议分派 ──

impl Dispatch<EtherCompositionManagerV1, ()> for Shell {
    fn event(
        _state: &mut Self,
        _proxy: &EtherCompositionManagerV1,
        _event: proto::ether_composition_manager_v1::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<EtherVisualV1, LayerId> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &EtherVisualV1,
        event: ether_visual_v1::Event,
        id: &LayerId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        log::debug!("图层 visual 事件 id={:?} {event:?}", id);
        let e = match event {
            ether_visual_v1::Event::Done { serial } => LayerEvent::Done { id: *id, serial },
            ether_visual_v1::Event::Cancelled { serial } => LayerEvent::Cancelled { id: *id, serial },
        };
        state.app.on_layer_event(e);
    }
}

impl Dispatch<EtherScrollerV1, LayerId> for Shell {
    fn event(
        state: &mut Self,
        _proxy: &EtherScrollerV1,
        event: proto::ether_scroller_v1::Event,
        id: &LayerId,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use proto::ether_scroller_v1::Event as E;
        let e = match event {
            // 每输出帧至多一次：将要显示的位置 + 该帧目标呈现时刻（CLOCK_MONOTONIC ns）。
            E::Offset { x, y, present_ns_hi, present_ns_lo } => LayerEvent::ScrollerOffset {
                id: *id,
                x: x as f32,
                y: y as f32,
                present_ns: ((present_ns_hi as u64) << 32) | present_ns_lo as u64,
            },
            E::State { phase, serial } => LayerEvent::ScrollerState {
                id: *id,
                phase: u32::from(phase),
                serial,
            },
        };
        state.app.on_layer_event(e);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::{Scene, SceneCommand};
    use kanesumi_core::Color;

    fn fill(x: f32, y: f32, w: f32, h: f32, c: Color) -> SceneCommand {
        SceneCommand::FillRect { color: c, rect: Rect::new(x, y, w, h), corner_radius: 0.0 }
    }

    #[test]
    fn scene_damage_is_none_without_previous_frame() {
        let mut s = Scene::default();
        s.commands.push(fill(0.0, 0.0, 10.0, 10.0, Color::new(1.0, 0.0, 0.0, 1.0)));
        assert_eq!(scene_damage(None, &s, kanesumi_core::Size::new(100.0, 100.0)), None);
    }

    #[test]
    fn scene_damage_covers_only_changed_commands() {
        let size = kanesumi_core::Size::new(200.0, 100.0);
        let a = Color::new(1.0, 0.0, 0.0, 1.0);
        let b = Color::new(0.0, 1.0, 0.0, 1.0);
        let mut prev = Scene::default();
        prev.commands.push(fill(0.0, 0.0, 10.0, 10.0, a));
        prev.commands.push(fill(50.0, 20.0, 10.0, 10.0, a));
        let mut next = prev.clone();
        next.commands[1] = fill(50.0, 20.0, 10.0, 10.0, b);
        let d = scene_damage(Some(&prev), &next, size).expect("有损伤");
        // 只有第二条变：损伤 = 它的外框 + 1 px 外扩。
        assert!((d.origin.x - 49.0).abs() < 1e-4, "{d:?}");
        assert!((d.origin.y - 19.0).abs() < 1e-4, "{d:?}");
        assert!((d.size.width - 12.0).abs() < 1e-4, "{d:?}");
        assert!((d.size.height - 12.0).abs() < 1e-4, "{d:?}");
    }

    #[test]
    fn scene_damage_falls_back_to_full_on_structure_or_clip_change() {
        let size = kanesumi_core::Size::new(200.0, 100.0);
        let c = Color::new(1.0, 0.0, 0.0, 1.0);
        let mut prev = Scene::default();
        prev.commands.push(fill(0.0, 0.0, 10.0, 10.0, c));
        let mut more = prev.clone();
        more.commands.push(fill(20.0, 0.0, 10.0, 10.0, c));
        assert_eq!(scene_damage(Some(&prev), &more, size), None, "命令数变化 → 整幅");
        let mut clip = prev.clone();
        clip.commands[0] = SceneCommand::PushClip { rect: Rect::new(0.0, 0.0, 5.0, 5.0) };
        assert_eq!(scene_damage(Some(&prev), &clip, size), None, "裁剪变化 → 整幅");
    }

    #[test]
    fn scene_damage_compares_image_by_identity_not_bytes() {
        let size = kanesumi_core::Size::new(200.0, 100.0);
        let rgba: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![255u8; 4 * 4 * 4]);
        let mk = |rgba: std::sync::Arc<[u8]>, x: f32| SceneCommand::Image {
            rgba,
            width: 4,
            height: 4,
            rect: Rect::new(x, 0.0, 4.0, 4.0),
            tint: None,
            opacity: 1.0,
        };
        let mut prev = Scene::default();
        prev.commands.push(mk(rgba.clone(), 0.0));
        let mut same = prev.clone();
        same.commands[0] = mk(rgba, 0.0);
        assert_eq!(scene_damage(Some(&prev), &same, size), None, "同一份图像 → 无损伤");
        let other: std::sync::Arc<[u8]> = std::sync::Arc::from(vec![0u8; 4 * 4 * 4]);
        let mut moved = prev.clone();
        moved.commands[0] = mk(other, 100.0);
        let d = scene_damage(Some(&prev), &moved, size).expect("换图 + 移位 → 有损伤");
        assert!(d.size.width >= 100.0, "两处外框都要覆盖：{d:?}");
    }
}
