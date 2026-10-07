#![cfg_attr(not(target_os = "linux"), allow(unused_imports, dead_code))]
// transition_demo —— 元素树主题转场库演示（Entrance stagger / Content / Reposition）。
// 参 ANIMATION_SPEC §Ⅱ / §Ⅴ、ELEMENT_TREE §Ⅴ-ter。
//
// 运行（Linux，Ether 合成器；图层动画由合成器推进，客户端不重画）：
//   ETHER_ROLE=browser cargo run -p kanesumi-gallery --example transition_demo
// 非 Linux 仅保证可编译 —— 转场动画需要合成器，命令序列由 `kanesumi-element` 单测覆盖。
//
// 交互：启动后磁贴错位入场；点「重播」再发一次 Entrance；点单个磁贴发 Content（内容变化）。
// 图层 `Done` / `Cancelled` 经 `TreeApp::on_layer_event` 转达 `Tree::on_layer_event` 撤层。

use kanesumi_controls::{ButtonClicked, MetroButton};
use kanesumi_core::ThemeColor;
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{
    Action, Align, Insets, LayerOutcome, LayoutProps, Transition, TransitionParams, Tree, WidgetId,
};
use kanesumi_harness::layers::LayerEvent;
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

struct Demo {
    config: AppConfig,
    column: Option<WidgetId>,
    tiles: Vec<WidgetId>,
    replay: Option<WidgetId>,
    played: bool,
}

fn tile_props() -> LayoutProps {
    LayoutProps {
        width: Some(320.0),
        height: Some(56.0),
        h_align: Align::Start,
        ..LayoutProps::default()
    }
}

impl Demo {
    fn entrance(&self, tree: &mut Tree) {
        if let Some(col) = self.column {
            tree.play_transition(
                col,
                Transition::Entrance { stagger: true },
                TransitionParams::ENTRANCE,
            );
        }
    }
}

impl TreeApp for Demo {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(8.0));
        self.column = Some(col);
        for i in 0..5 {
            let tile = tree.insert_with(
                col,
                Border::new()
                    .background(ThemeColor::Surface)
                    .padding(Insets::all(12.0)),
                tile_props(),
            );
            tree.insert(tile, Label::new(format!("磁贴 {}", i + 1)));
            self.tiles.push(tile);
        }
        self.replay = Some(tree.insert_with(
            page,
            MetroButton::new("重播"),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        ));
    }

    fn tick(&mut self, tree: &mut Tree, _dt: f64) {
        // 首帧后补一次入场（此时布局已就绪，图层能拿到正确矩形）。
        if !self.played {
            self.played = true;
            self.entrance(tree);
        }
    }

    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action) {
        if Some(from) == self.replay && action.is::<ButtonClicked>() {
            self.entrance(tree);
        } else if self.tiles.contains(&from) && action.is::<ButtonClicked>() {
            // 内容变化：Content 转场（位移更小、无 stagger）。
            tree.play_transition(from, Transition::Content, TransitionParams::CONTENT);
        }
    }

    fn on_layer_event(&mut self, tree: &mut Tree, widget: Option<WidgetId>, event: LayerEvent) {
        match event {
            LayerEvent::Done { serial, .. } | LayerEvent::Cancelled { serial, .. } => {
                if let Some(id) = widget {
                    tree.on_layer_event(id, serial, LayerOutcome::Done);
                }
            }
            LayerEvent::Unsupported => tree.on_layer_unsupported(),
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let app = Demo {
        config: AppConfig::new("org.ether.transition-demo", "transition demo", EtherRole::Browser, 640.0, 480.0),
        column: None,
        tiles: Vec::new(),
        replay: None,
        played: false,
    };
    let host = kanesumi_harness::TreeHost::new(app);
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("transition_demo 需要 Linux（Wayland + Ether 合成器）；命令序列见 kanesumi-element 单测。");
}
