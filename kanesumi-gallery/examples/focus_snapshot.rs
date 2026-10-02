// focus_snapshot —— 键盘焦点态快照（正典 §Ⅳ：外 2px 焦点色 + 内 1px 对比色）。
//
// 一个按钮置于页面左上，`tree.focus(btn, true)` 令其处于键盘焦点态；用小窗口 + 高 scale
// 放大，肉眼可查双层焦点框。生成 `docs/research/k_interact/` 下的深 / 浅两张图：
//
//   cargo run -p kanesumi-gallery --example focus_snapshot -- --scheme dark  --snapshot docs/research/k_interact/focus_dark.png  8
//   cargo run -p kanesumi-gallery --example focus_snapshot -- --scheme light --snapshot docs/research/k_interact/focus_light.png 8
//
// 无 `--snapshot` 则按普通 GUI 应用运行（Linux Wayland）。

use kanesumi_controls::MetroButton;
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

/// 逻辑窗口尺寸（放大靠 `scale`）。
const W: f32 = 160.0;
const H: f32 = 72.0;

struct FocusPage {
    config: AppConfig,
    theme: MetroTheme,
}

impl TreeApp for FocusPage {
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
                .padding(Insets::all(12.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(8.0));
        let start = LayoutProps {
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        };
        let btn = tree.insert_with(col, MetroButton::new("确定"), start);
        // 键盘焦点（正典 §Ⅳ：只有键盘交互后才显示焦点框）。
        tree.focus(btn, true);
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn theme_for(scheme: &str) -> MetroTheme {
    match ColorScheme::parse(scheme) {
        Some(ColorScheme::Light) => MetroTheme::light(Accent::default()),
        _ => MetroTheme::dark(Accent::default()),
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scheme = args
        .iter()
        .position(|a| a == "--scheme")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "dark".into());
    let app = FocusPage {
        config: AppConfig::new(
            "org.ether.kanesumi.focussnapshot",
            "焦点快照",
            EtherRole::Browser,
            W,
            H,
        ),
        theme: theme_for(&scheme),
    };
    let mut host = TreeHost::new(app);
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::platform::find_font().expect("未找到字体");
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
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    let _ = (theme_for("dark"), W, H);
    println!("focus_snapshot 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
}
