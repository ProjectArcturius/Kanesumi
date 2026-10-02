// tree_minimal —— 最小元素树应用（docs/DEV_GUIDE.md §二 逐行同源，改文档请同步改此处）。
//
// 不写一个坐标、不写一个命中函数：布局由 Stack 与 LayoutProps 决定，交互由控件动作驱动。
//
// 运行（Linux，任意支持 xdg-shell 的合成器）：
//   cargo run -p kanesumi-gallery --example tree_minimal
// 出快照 PNG（任意平台）：
//   cargo run -p kanesumi-gallery --example tree_minimal -- --snapshot out.png 1

use kanesumi_controls::{ButtonClicked, MetroButton};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Insets, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

struct Counter {
    config: AppConfig,
    theme: MetroTheme,
    value: u32,
    label: Option<WidgetId>,
}

impl TreeApp for Counter {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert(
            col,
            Label::new("最小元素树应用").style(self.theme.typography.title),
        );
        self.label = Some(tree.insert(
            col,
            Label::new(format!("计数：{}", self.value)).color(ThemeColor::OnSurfaceVariant),
        ));
        tree.insert(col, MetroButton::accent("加一"));
    }

    fn on_action(&mut self, tree: &mut Tree, _from: WidgetId, action: Action) {
        if action.is::<ButtonClicked>() {
            self.value += 1;
            if let Some(id) = self.label {
                tree.edit::<Label, _>(id, |l, _| l.text = format!("计数：{}", self.value));
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let app = Counter {
        config: AppConfig::new(
            "org.ether.kanesumi.treeminimal",
            "最小元素树应用",
            EtherRole::Browser,
            420.0,
            300.0,
        ),
        theme: MetroTheme::ether_dark(),
        value: 0,
        label: None,
    };
    let mut host = kanesumi_harness::TreeHost::new(app);
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::platform::find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(420.0, 300.0),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    let _ = Counter {
        config: AppConfig::new(
            "org.ether.kanesumi.treeminimal",
            "",
            EtherRole::Browser,
            1.0,
            1.0,
        ),
        theme: MetroTheme::ether_dark(),
        value: 0,
        label: None,
    };
    println!("tree_minimal 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
}
