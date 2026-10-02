// tooltip_demo —— 工具提示演示（CONTROL_SPEC §48）。元素树应用只需 `tree.set_tooltip`，
// 显隐 / 计时 / 定位由框架负责（不抢焦点、不吃输入）。同时用于出深 / 浅快照：
//
//   cargo run -p kanesumi-gallery --example tooltip_demo -- --snapshot docs/research/k_tooltip/tooltip_dark.png  3
//   cargo run -p kanesumi-gallery --example tooltip_demo -- --scheme light --snapshot docs/research/k_tooltip/tooltip_light.png 3
//
// 无 `--snapshot` 则按普通 GUI 应用运行（Linux Wayland）。

use kanesumi_controls::MetroButton;
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{App as _, AppConfig, EtherRole, InputEvent, TreeApp, TreeHost};

const W: f32 = 360.0;
const H: f32 = 200.0;

struct TooltipPage {
    config: AppConfig,
    theme: MetroTheme,
    /// 快照时悬停的目标（长提示，演示换行）。
    hover_id: Option<WidgetId>,
}

impl TreeApp for TooltipPage {
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
            Label::new("工具提示").style(self.theme.typography.title),
        );
        let start = LayoutProps {
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        };
        let short = tree.insert_with(col, MetroButton::new("重置"), start);
        tree.set_tooltip(short, "把名称恢复为默认值");
        let long = tree.insert_with(col, MetroButton::new("应用"), start);
        tree.set_tooltip(
            long,
            "应用设置并打开确认对话框：这段文字较长，用来演示最大宽度 320 与自动换行。",
        );
        self.hover_id = Some(long);
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
    let app = TooltipPage {
        config: AppConfig::new(
            "org.ether.kanesumi.tooltipdemo",
            "工具提示演示",
            EtherRole::Browser,
            W,
            H,
        ),
        theme: theme_for(&scheme),
        hover_id: None,
    };
    let mut host = TreeHost::new(app);
    let hover = host.app().hover_id;

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::element::testing::find_test_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        // 先出一帧拿到布局，再把指针移到目标上；随后 render_png 的帧数足以越过提示延迟。
        let size = Size::new(W, H);
        let mut scene = kanesumi_canvas::Scene::default();
        host.advance_clock(1.0 / 60.0);
        host.update(1.0 / 60.0);
        host.render_into(&engine, size, &mut scene);
        if let Some(id) = hover {
            let c = host.tree().rect(id).expect("目标已布局").center();
            host.handle_input(InputEvent::PointerMoved { x: c.x, y: c.y });
        }
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            size,
            scale,
            60,
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
        let _ = (hover, host, W, H);
        println!("tooltip_demo 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
    }
}
