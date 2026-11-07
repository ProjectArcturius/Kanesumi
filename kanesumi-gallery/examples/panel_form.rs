// panel_form —— 面板（顶栏控制中心）参照页：MetroSurface + 页面语法行 + 命令行。
//
// 这张卡回答的是「顶栏那五个整块自绘的面板要不要新控件」。答案是不用：面板的内容行与设置页
// 同形（标签 + 控件 + 数值），差别只在容器 —— 浮层表面而非滚动页。于是
//
//     MetroSurface（画底 / 圆角 / 裁剪）
//       └─ Form::section / slider / toggle   ← 页面语法
//            └─ 命令行（Stack::row + MetroButton）
//
// 命中、悬停、焦点、Tab、无障碍全交给元素树，面板不必再手算 volume_block(rect) 这类矩形，
// 也不必自持 PanelRegion 枚举。参 docs/CONTROL_SPEC.md §50。
//
// 出图：cargo run -p kanesumi-gallery --example panel_form -- --snapshot out.png 1

use kanesumi_controls::form::{self, Form};
use kanesumi_controls::{MetroButton, SwitchToggled};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

/// 面板宽（逻辑）。
const PANEL_W: f32 = 360.0;
/// 画布尺寸。
const PAGE_W: f32 = 400.0;
/// 画布高。
const PAGE_H: f32 = 620.0;

struct ControlCenter {
    config: AppConfig,
    theme: MetroTheme,
    wifi: bool,
    bluetooth: bool,
}

impl ControlCenter {
    fn new() -> Self {
        Self {
            config: AppConfig::new("org.ether.panel-form-demo", "控制中心", EtherRole::Browser, PAGE_W, PAGE_H),
            theme: MetroTheme::ether_dark(),
            wifi: true,
            bluetooth: false,
        }
    }
}

impl TreeApp for ControlCenter {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));

        // 画布留白：面板本身由合成器摆位置，示例里给一圈边距好看清边界。
        let pad = tree.insert_with(
            page,
            Stack::column(),
            LayoutProps {
                margin: Insets::new(20.0, 20.0, 20.0, 20.0),
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        // 面板容器：一条 form::panel 搞定（表面 + 内边距 + 一列），尺寸按内容撑开。
        let col = form::panel(tree, pad, self.theme, PANEL_W);

        let mut f = Form::new(col);
        f.section(tree, "音量", true);
        f.slider(tree, "扬声器", "60", 0.6, 0.0, 1.0, Some(0.01), None);

        f.section(tree, "亮度", false);
        f.slider(tree, "屏幕", "80", 0.8, 0.0, 1.0, Some(0.01), None);

        f.section(tree, "快速开关", false);
        f.toggle(tree, "Wi-Fi", self.wifi, None);
        f.toggle(tree, "蓝牙", self.bluetooth, None);

        // 命令行：面板底部的动作，靠右排。
        let row = f.row_block(tree, form::ITEM_MB);
        let line = tree.insert_with(
            row,
            Stack::row().with_spacing(8.0),
            LayoutProps {
                h_align: Align::End,
                ..LayoutProps::default()
            },
        );
        tree.insert(line, MetroButton::new("设置"));
        tree.insert(line, MetroButton::new("锁屏"));
        tree.insert(line, MetroButton::new("关机"));
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(t) = action.downcast_ref::<SwitchToggled>() {
            // 面板按行身份路由；示範用「与当前值不同」判定。
            if self.wifi != t.0 {
                self.wifi = t.0;
            } else {
                self.bluetooth = t.0;
            }
        }
    }
}

/// 字体查找（跨平台）。
fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        return Some(std::path::PathBuf::from(p));
    }
    [
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|p| p.exists())
}

fn main() {
    let mut host = kanesumi_harness::TreeHost::new(ControlCenter::new());
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).map(String::as_str).unwrap_or("panel_form.png");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(PAGE_W, PAGE_H),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }
    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
    #[cfg(not(target_os = "linux"))]
    {
        let _ = &mut host;
        println!("panel_form 实时窗口需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
    }
}
