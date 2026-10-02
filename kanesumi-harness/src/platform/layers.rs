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
use proto::ether_visual_v1::{self, EtherVisualV1};

/// 一个已创建的图层。
pub(super) struct CompLayer {
    id: LayerId,
    parent: wl_surface::WlSurface,
    surface: wl_surface::WlSurface,
    subsurface: WlSubsurface,
    visual: Option<EtherVisualV1>,
    pub(super) out: SurfaceOutput,
    cpu: Option<CpuRenderer>,
    rect: Rect,
    scale: f32,
    /// 回落模式下模拟的视觉偏移（有协议时不用）。
    offset: (f32, f32),
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
            LayerCommand::Destroy { id } => {
                let Some(i) = self.layer_index(id) else { return };
                let l = self.comp.layers.remove(i);
                if let Some(v) = l.visual {
                    v.destroy();
                }
                l.subsurface.destroy();
                l.surface.destroy();
                l.parent.commit();
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
            out,
            cpu: None,
            rect,
            scale,
            offset: (0.0, 0.0),
        };
        place(&mut l);
        self.comp.layers.push(l);
    }

    /// 光栅内容并提交（只在内容变化时由 App 触发；动画期间不会走到这里）。
    fn set_layer_content(&mut self, id: LayerId, scene: &Scene, qh: &QueueHandle<Shell>) {
        let Some(i) = self.layer_index(id) else { return };
        let Shell { engine, comp, shm, dmabuf, .. } = self;
        let l = &mut comp.layers[i];
        let (w, h) = (l.rect.size.width.max(1.0), l.rect.size.height.max(1.0));
        let cpu = l.cpu.get_or_insert_with(|| CpuRenderer::new(w, h, l.scale));
        let (pw, ph) = cpu.physical_size();
        let rgba = cpu.render(engine, scene, None);
        l.out.commit(qh, shm.as_ref(), dmabuf, &l.surface, pw, ph, rgba, l.scale, None);
    }
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
        let e = match event {
            ether_visual_v1::Event::Done { serial } => LayerEvent::Done { id: *id, serial },
            ether_visual_v1::Event::Cancelled { serial } => LayerEvent::Cancelled { id: *id, serial },
        };
        state.app.on_layer_event(e);
    }
}
