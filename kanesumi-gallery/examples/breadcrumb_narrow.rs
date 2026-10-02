// breadcrumb_narrow —— 窄宽 BreadcrumbBar 快照（CONTROL_SPEC §18：前缀优先折进溢出按钮，
// 仍放不下时末段省略号截断）。参 docs/research/k_tooltip/。
//
//   cargo run -p kanesumi-gallery --example breadcrumb_narrow -- --snapshot docs/research/k_tooltip/breadcrumb_narrow.png 3
//
// 无 `--snapshot` 则按普通 GUI 应用运行（Linux Wayland）。

use kanesumi_controls::MetroBreadcrumbBar;
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

const W: f32 = 300.0;
const H: f32 = 120.0;
/// 面包屑被外部夹到的宽度（模拟 40px 工具栏 / 窄面板）；窄到前缀折叠后末段仍放不下，
/// 以触发「末段省略号截断」。
const BAR_W: f32 = 140.0;

struct NarrowPage {
    config: AppConfig,
    theme: MetroTheme,
}

impl TreeApp for NarrowPage {
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
                .padding(Insets::all(16.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert_with(
            col,
            MetroBreadcrumbBar::new(
                ["首页", "文档", "项目", "一个很长的当前页面名称"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            LayoutProps {
                width: Some(BAR_W),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn theme_for(scheme: &str) -> MetroTheme {
    match ColorScheme::parse(scheme) {
        Some(ColorScheme::Light) => MetroTheme::light(Accent::default()),
        _ => MetroTheme::dark(Accent::default()),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scheme = args
        .iter()
        .position(|a| a == "--scheme")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "dark".into());
    let page = NarrowPage {
        config: AppConfig::new(
            "org.ether.kanesumi.breadcrumbnarrow",
            "窄宽面包屑",
            EtherRole::Browser,
            W,
            H,
        ),
        theme: theme_for(&scheme),
    };
    let mut host = TreeHost::new(page);

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::element::testing::find_test_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            Size::new(W, H),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!(
            "snapshot {out} {w}x{h} scheme={scheme} font={}",
            font.display()
        );
        return;
    }

    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (host, W, H);
        println!(
            "breadcrumb_narrow 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。"
        );
    }
}
