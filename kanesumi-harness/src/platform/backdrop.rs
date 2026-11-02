// 背板代理生命周期与能力汇总。参 Ether docs/research/backdrop_m3/PROTOCOL.md。
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Dispatch, Proxy, QueueHandle};

use super::layers::EtherCompositionManagerV1;
use super::layers::proto::ether_backdrop_v1::{self, EtherBackdropV1};
use crate::layers::{BackdropMaterial, BackdropTarget, LayerEvent};

struct SurfaceBackdrop {
    surface: WlSurface,
    proxy: EtherBackdropV1,
    available: Option<bool>,
}

struct TargetBackdrop {
    target: BackdropTarget,
    material: BackdropMaterial,
    surfaces: Vec<SurfaceBackdrop>,
}

#[derive(Default)]
pub(crate) struct Backdrops {
    targets: Vec<TargetBackdrop>,
}

impl Backdrops {
    pub(crate) fn contains(&self, target: BackdropTarget) -> bool {
        self.targets.iter().any(|t| t.target == target)
    }

    pub(crate) fn remove(&mut self, target: BackdropTarget) {
        if let Some(i) = self.targets.iter().position(|t| t.target == target) {
            for s in self.targets.remove(i).surfaces {
                s.proxy.destroy();
            }
        }
    }

    pub(crate) fn set<D>(
        &mut self,
        manager: Option<&EtherCompositionManagerV1>,
        target: BackdropTarget,
        material: Option<BackdropMaterial>,
        surfaces: Vec<WlSurface>,
        qh: &QueueHandle<D>,
    ) -> Option<LayerEvent>
    where
        D: Dispatch<EtherBackdropV1, BackdropTarget> + 'static,
    {
        let Some(material) = material else {
            self.remove(target);
            return Some(event(target, false));
        };
        let Some(manager) = manager.filter(|m| m.version() >= 4) else {
            self.remove(target);
            return Some(event(target, false));
        };
        let index = match self.targets.iter().position(|t| t.target == target) {
            Some(i) => i,
            None => {
                self.targets.push(TargetBackdrop {
                    target,
                    material,
                    surfaces: Vec::new(),
                });
                self.targets.len() - 1
            }
        };
        let entry = &mut self.targets[index];
        let was_available = aggregate(entry) == Some(true);
        entry.material = material;
        // 热插拔或目标重建先销毁旧代理，禁止排队旧事件污染新对象。
        entry.surfaces.retain(|s| {
            if surfaces.contains(&s.surface) {
                true
            } else {
                s.proxy.destroy();
                false
            }
        });
        for surface in surfaces {
            if !entry.surfaces.iter().any(|s| s.surface == surface) {
                let proxy = manager.get_backdrop(&surface, qh, target);
                entry.surfaces.push(SurfaceBackdrop {
                    surface,
                    proxy,
                    available: None,
                });
            }
        }
        for s in &entry.surfaces {
            s.proxy.set_material(
                material.blur as f64,
                material.tint_rgb,
                material.tint_opacity as f64,
            );
            if let Some(r) = material.region {
                s.proxy.set_region(
                    r.origin.x.round() as i32,
                    r.origin.y.round() as i32,
                    r.size.width.round() as i32,
                    r.size.height.round() as i32,
                );
            } else {
                s.proxy.unset_region();
            }
            s.surface.commit();
        }
        aggregate(entry)
            .or_else(|| was_available.then_some(false))
            .map(|available| event(target, available))
    }

    pub(crate) fn sync<D>(
        &mut self,
        manager: Option<&EtherCompositionManagerV1>,
        target: BackdropTarget,
        surfaces: Vec<WlSurface>,
        qh: &QueueHandle<D>,
    ) -> Option<LayerEvent>
    where
        D: Dispatch<EtherBackdropV1, BackdropTarget> + 'static,
    {
        let entry = self.targets.iter().find(|t| t.target == target)?;
        if entry.surfaces.len() == surfaces.len()
            && entry.surfaces.iter().all(|s| surfaces.contains(&s.surface))
        {
            return None;
        }
        self.set(manager, target, Some(entry.material), surfaces, qh)
    }

    pub(crate) fn availability(
        &mut self,
        proxy: &EtherBackdropV1,
        target: BackdropTarget,
        state: wayland_client::WEnum<ether_backdrop_v1::Availability>,
    ) -> Option<LayerEvent> {
        let entry = self.targets.iter_mut().find(|t| t.target == target)?;
        let surface = entry.surfaces.iter_mut().find(|s| s.proxy == *proxy)?;
        surface.available = Some(matches!(
            state,
            wayland_client::WEnum::Value(ether_backdrop_v1::Availability::Available)
        ));
        aggregate(entry).map(|available| event(target, available))
    }
}

impl Drop for Backdrops {
    fn drop(&mut self) {
        for entry in &self.targets {
            for s in &entry.surfaces {
                s.proxy.destroy();
            }
        }
    }
}

fn aggregate(entry: &TargetBackdrop) -> Option<bool> {
    availability_state(entry.surfaces.iter().map(|s| s.available))
}

fn availability_state(states: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut any = false;
    let mut pending = false;
    for state in states {
        any = true;
        match state {
            Some(false) => return Some(false),
            None => pending = true,
            Some(true) => {}
        }
    }
    if !any {
        Some(false)
    } else if pending {
        None
    } else {
        Some(true)
    }
}

fn event(target: BackdropTarget, available: bool) -> LayerEvent {
    LayerEvent::BackdropAvailability { target, available }
}

#[cfg(test)]
mod tests {
    use super::availability_state;

    #[test]
    fn primary_available_must_wait_for_secondary() {
        assert_eq!(availability_state([Some(true), None].into_iter()), None);
        assert_eq!(
            availability_state([Some(true), Some(true)].into_iter()),
            Some(true)
        );
    }

    #[test]
    fn any_failed_output_requires_client_fallback() {
        assert_eq!(
            availability_state([Some(true), Some(false)].into_iter()),
            Some(false)
        );
        assert_eq!(
            availability_state([None, Some(false)].into_iter()),
            Some(false)
        );
        assert_eq!(availability_state([].into_iter()), Some(false));
    }
}
