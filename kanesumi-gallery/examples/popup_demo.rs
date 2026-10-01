// popup_demo —— 子弹层验收：TopBar（layer-shell TOP，30px）上的菜单栏，菜单落在表面之外。
//
// 元素树弹层在顶 / 底条上由外壳各开一个 xdg_popup（父 = layer 表面，经 get_popup）承载，
// 点外部由合成器 popup_done 关闭。参 Ether docs/POPUP_PLAN.md。
//
// 运行（Linux）：
//   cargo run -p kanesumi-gallery --example popup_demo
// `POPUP_DEMO_NAMESPACE` 覆盖 layer namespace（Ether 合成器按 namespace 路由顶栏指针，
// 无头验收时设为 `ether-settings-topbar` 冒充 TopBar）。

use kanesumi_controls::{MenuBarItem, MenuInvoked, MenuItem, MetroMenuBar};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

struct Demo {
    config: AppConfig,
    status: Option<WidgetId>,
}

impl TreeApp for Demo {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn preferred_height(&self) -> Option<f32> {
        Some(30.0)
    }

    fn build(&mut self, tree: &mut Tree) {
        let c = self.theme().colors;
        let bar = tree.insert(tree.root(), Border::new().background(c.surface));
        let row = tree.insert(bar, Stack::row().with_spacing(12.0));
        let center = LayoutProps {
            v_align: Align::Center,
            ..LayoutProps::default()
        };
        tree.insert_with(
            row,
            MetroMenuBar::new(vec![
                MenuBarItem::new(
                    "文件",
                    vec![
                        MenuItem::new("新建"),
                        MenuItem::new("打开…"),
                        MenuItem::new("退出"),
                    ],
                ),
                MenuBarItem::new(
                    "视图",
                    vec![
                        MenuItem::new("放大"),
                        MenuItem::new("缩小"),
                        MenuItem::new("全屏"),
                    ],
                ),
            ]),
            center,
        );
        self.status = Some(tree.insert_with(
            row,
            Label::new("就绪"),
            LayoutProps {
                margin: Insets::new(12.0, 0.0, 0.0, 0.0),
                ..center
            },
        ));
    }

    fn on_action(&mut self, tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(m) = action.downcast_ref::<MenuInvoked>()
            && let Some(id) = self.status
        {
            let text = format!("选了：{}", m.label);
            log::info!("popup_demo {text}");
            tree.edit::<Label, _>(id, |l, _| l.text = text);
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let ns: &'static str = Box::leak(
        std::env::var("POPUP_DEMO_NAMESPACE")
            .unwrap_or_else(|_| "org.ether.kanesumi.popupdemo".into())
            .into_boxed_str(),
    );
    let app = Demo {
        config: AppConfig::new(ns, "子弹层验收", EtherRole::TopBar, 800.0, 30.0),
        status: None,
    };
    kanesumi_harness::platform::run(Box::leak(Box::new(kanesumi_harness::TreeHost::new(app))));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    let _ = Demo {
        config: AppConfig::new(
            "org.ether.kanesumi.popupdemo",
            "",
            EtherRole::TopBar,
            1.0,
            1.0,
        ),
        status: None,
    };
    println!("popup_demo 需要 Linux Wayland 会话。");
}
