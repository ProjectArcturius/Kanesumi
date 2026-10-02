#![cfg_attr(not(target_os = "linux"), allow(unused_imports, dead_code))]
// layer_probe —— 合成图层（G3-b）端到端探针：三个图层错开入场 → 停留 → 离场，动画全部由合成器推进。
// 参 Ether docs/GPU_COMPOSITION_PLAN.md G3。
//
// 运行（Linux，Ether 合成器；layer-shell 背景层角色，便于无头会话截图）：
//   ETHER_ROLE=desktop cargo run -p kanesumi-gallery --example layer_probe
// 日志：每次主表面 render、每条图层事件都打一行 —— 验收「动画期间客户端不重画」看 render 计数是否停住。

use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Color, Rect, Size};
use kanesumi_harness::layers::{
    LayerAnim, LayerCommand, LayerEvent, LayerId, LayerParent, LayerQueue, CURVE_UWP_CLOSE, CURVE_UWP_OPEN,
};
use kanesumi_harness::{App, AppConfig, EtherRole, Scene};

const COLORS: [Color; 3] = [
    Color::rgb(0.18, 0.45, 0.62),
    Color::rgb(0.80, 0.42, 0.10),
    Color::rgb(0.25, 0.55, 0.30),
];

struct Probe {
    config: AppConfig,
    queue: LayerQueue,
    started: Option<Instant>,
    phase: u8,
    done: u32,
    renders: u32,
}

impl Probe {
    fn tile(i: usize) -> Rect {
        Rect::new(120.0 + i as f32 * 220.0, 200.0, 200.0, 200.0)
    }

    fn animate_all(&mut self, entering: bool) {
        for i in 0..3 {
            let anim = LayerAnim {
                serial: (self.phase as u32) * 10 + i as u32,
                duration_ms: if entering { 350 } else { 200 },
                delay_ms: if entering { i as u32 * 60 } else { 0 },
                curve: if entering { CURVE_UWP_OPEN } else { CURVE_UWP_CLOSE },
                to_x: 0.0,
                to_y: if entering { 0.0 } else { 40.0 },
                to_opacity: if entering { 1.0 } else { 0.0 },
            };
            self.queue.animate(LayerId(i as u32), anim);
        }
    }
}

impl App for Probe {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn update(&mut self, _dt: f64) {
        let now = Instant::now();
        let started = *self.started.get_or_insert(now);
        let t = now.duration_since(started).as_millis();
        match self.phase {
            // 首帧之后建层：内容只画这一次；起点 = 下移 40、透明。
            0 if t > 300 => {
                for i in 0..3 {
                    let id = LayerId(i as u32);
                    let rect = Self::tile(i);
                    self.queue.create(id, LayerParent::Main, rect);
                    let mut scene = Scene::default();
                    scene.fill_rect(COLORS[i], Rect::new(0.0, 0.0, rect.size.width, rect.size.height));
                    self.queue.set_content(id, scene);
                    self.queue.push(LayerCommand::SetOffset { id, x: 0.0, y: 40.0 });
                    self.queue.push(LayerCommand::SetOpacity { id, opacity: 0.0 });
                }
                self.phase = 1;
            }
            1 if t > 1000 => {
                log::info!("probe：入场开始（主表面 render 计数 {}）", self.renders);
                self.animate_all(true);
                self.phase = 2;
            }
            3 if t > 2500 => {
                log::info!("probe：离场开始（主表面 render 计数 {}）", self.renders);
                self.animate_all(false);
                self.phase = 4;
            }
            _ => {}
        }
    }

    fn needs_redraw(&self) -> bool {
        // 只有建层前需要出首帧；之后主表面不再重画（动画全在合成器）。
        self.renders == 0
    }

    fn render(&mut self, _engine: &TextEngine, size: Size) -> Scene {
        self.renders += 1;
        let mut scene = Scene::default();
        scene.fill_rect(Color::new(0.08, 0.08, 0.09, 1.0), Rect::new(0.0, 0.0, size.width, size.height));
        scene
    }

    fn take_layer_commands(&mut self) -> Vec<LayerCommand> {
        self.queue.take()
    }

    fn on_layer_event(&mut self, event: LayerEvent) {
        log::info!("probe：图层事件 {event:?}（主表面 render 计数 {}）", self.renders);
        if let LayerEvent::Done { .. } = event {
            self.done += 1;
            if self.done == 3 && self.phase == 2 {
                self.phase = 3;
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let app = Probe {
        config: AppConfig::new("org.ether.layer-probe", "layer probe", EtherRole::Desktop, 1280.0, 800.0),
        queue: LayerQueue::new(),
        started: None,
        phase: 0,
        done: 0,
        renders: 0,
    };
    kanesumi_harness::platform::run(Box::leak(Box::new(app)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    let _ = Probe::tile(0);
    eprintln!("layer_probe 需要 Linux（Wayland + Ether 合成器）");
}
