// nav_focus_snapshot —— 焦点组 + 键盘焦点快照（任务 k-keyboard-nav，正典 §Ⅳ）。
//
// 四个场景各出一张图：焦点从组内首项出发，用**真实方向键**（`Tree::key_down`）移到
// 目标项后渲染 —— 图里焦点框的位置就是焦点组导航的产物，兼作端到端冒烟：
//   sidebar  侧栏纵向组：Down 一次，焦点在第 2 行（行文字贴左右缘，验焦点框不裁字）
//   content  内容纵向组：Down 一次，焦点在第 2 行（开关 / 滑杆 / 按钮混排的抽象）
//   grid     主页 4 列网格组（深色）：Right Right Down，焦点在第 2 行第 3 格
//   popup    弹层纵向组：开弹层焦点进首项，Down 一次到第 2 项，锚点按钮在上
//
//   cargo run -p kanesumi-gallery --example nav_focus_snapshot -- \
//     --scenario sidebar --scheme light --snapshot out.png 2
// 无 --snapshot 则按普通 GUI 应用运行（Linux Wayland）。

use kanesumi_controls::MetroButton;
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{
    Action, Align, FocusGroupSpec, Insets, Key, LayoutProps, PopupSpec, Tree, WidgetId,
};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

struct NavPage {
    config: AppConfig,
    theme: MetroTheme,
    scenario: &'static str,
}

impl TreeApp for NavPage {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        match self.scenario {
            "content" => self.build_content(tree),
            "grid" => self.build_grid(tree),
            "popup" => self.build_popup(tree),
            _ => self.build_sidebar(tree),
        }
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

impl NavPage {
    /// 侧栏纵向组：左栏 200 宽 + 5 行，Down 一次落第 2 行。
    fn build_sidebar(&mut self, tree: &mut Tree) {
        let row = tree.insert(tree.root(), Stack::row());
        let pane = tree.insert_with(
            row,
            Border::new()
                .background(ThemeColor::Surface)
                .padding(Insets::all(8.0)),
            LayoutProps {
                width: Some(200.0),
                ..LayoutProps::default()
            },
        );
        let col = tree.insert(pane, Stack::column());
        tree.set_focus_group(col, Some(FocusGroupSpec::vertical()));
        let names = ["个性化", "背景", "颜色", "任务栏", "开始菜单"];
        let mut first = None;
        for n in names {
            let b = tree.insert(col, MetroButton::new(n));
            first.get_or_insert(b);
        }
        assert!(tree.focus(first.unwrap(), true), "首行聚焦失败");
        assert!(tree.key_down(Key::Down, Default::default()), "组内 Down 应消费");
        tree.insert_with(
            row,
            Border::new().background(ThemeColor::Background),
            LayoutProps::default(),
        );
    }

    /// 内容纵向组：设置页混排的抽象（开关 / 滑杆 / 按钮），Down 一次落第 2 行。
    fn build_content(&mut self, tree: &mut Tree) {
        let root = tree.insert_with(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(12.0)),
            LayoutProps::default(),
        );
        let col = tree.insert(root, Stack::column());
        tree.set_focus_group(col, Some(FocusGroupSpec::vertical()));
        let names = ["透明效果 开关", "模糊强度 滑杆", "浏览…"];
        let mut first = None;
        for n in names {
            let b = tree.insert(col, MetroButton::new(n));
            first.get_or_insert(b);
        }
        assert!(tree.focus(first.unwrap(), true), "首行聚焦失败");
        assert!(tree.key_down(Key::Down, Default::default()), "组内 Down 应消费");
    }

    /// 4 列网格组（深色）：行主序 12 格，Right Right Down 落第 2 行第 3 格。
    fn build_grid(&mut self, tree: &mut Tree) {
        let grid = tree.insert_with(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(16.0)),
            LayoutProps::default(),
        );
        let col = tree.insert(grid, Stack::column());
        tree.set_focus_group(col, Some(FocusGroupSpec::grid(4)));
        let names = [
            "系统", "设备", "电话", "网络", "个性化", "应用", "账户", "时间",
            "游戏", "轻松使用", "搜索", "隐私",
        ];
        let mut first = None;
        let mut current_row = None;
        for (i, n) in names.iter().enumerate() {
            if i % 4 == 0 {
                current_row = Some(tree.insert(col, Stack::row()));
            }
            // 行主序：先序 = 第 1 行 4 格 → 第 2 行 4 格 → …（组导航按先序走）。
            let b = tree.insert_with(
                current_row.unwrap(),
                MetroButton::new(*n),
                LayoutProps {
                    width: Some(130.0),
                    ..LayoutProps::default()
                },
            );
            first.get_or_insert(b);
        }
        assert!(tree.focus(first.unwrap(), true), "首格聚焦失败");
        assert!(tree.key_down(Key::Right, Default::default()));
        assert!(tree.key_down(Key::Right, Default::default()));
        assert!(tree.key_down(Key::Down, Default::default()), "网格 Down 应消费");
    }

    /// 弹层纵向组：锚点按钮在上，弹层面板挂其下，Down 一次到第 2 项。
    fn build_popup(&mut self, tree: &mut Tree) {
        let root = tree.insert_with(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(12.0)),
            LayoutProps::default(),
        );
        let btn = tree.insert_with(
            root,
            MetroButton::new("日期和时间设置（锚点）"),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let pop = tree.open_popup(
            Border::new()
                .background(ThemeColor::Surface)
                .padding(Insets::all(8.0)),
            PopupSpec {
                anchor: Some(btn),
                gap: 4.0,
                ..PopupSpec::default()
            },
        );
        let inner = tree.insert(pop, Stack::column());
        tree.set_focus_group(inner, Some(FocusGroupSpec::vertical()));
        let items = ["自动设置时间", "立即同步", "时区：本地时间"];
        let mut first = None;
        for n in items {
            let b = tree.insert(inner, MetroButton::new(n));
            first.get_or_insert(b);
        }
        assert!(tree.focus(first.unwrap(), true), "弹层首项聚焦失败");
        assert!(tree.key_down(Key::Down, Default::default()), "弹层组内 Down 应消费");
    }
}

fn theme_for(scheme: &str) -> MetroTheme {
    match ColorScheme::parse(scheme) {
        Some(ColorScheme::Light) => MetroTheme::light(Accent::default()),
        _ => MetroTheme::dark(Accent::default()),
    }
}

/// 跨平台字体探测：优先带 CJK 的面（界面文案是中文），回退到 Linux 常见路径。
fn find_font() -> Option<std::path::PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(w) = std::env::var("WINDIR") {
        let fonts = std::path::PathBuf::from(w).join("Fonts");
        candidates.push(fonts.join("msyh.ttc"));
        candidates.push(fonts.join("segoeui.ttf"));
    }
    candidates.push(std::path::PathBuf::from(
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ));
    candidates.push(std::path::PathBuf::from(
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ));
    candidates.into_iter().find(|p| p.exists())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let arg = |name: &str| -> Option<String> {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let scenario: &'static str = match arg("--scenario").as_deref() {
        Some("content") => "content",
        Some("grid") => "grid",
        Some("popup") => "popup",
        _ => "sidebar",
    };
    let scheme = arg("--scheme").unwrap_or_else(|| "light".into());
    let w: f32 = if scenario == "grid" { 640.0 } else { 480.0 };
    let app = NavPage {
        config: AppConfig::new(
            "org.ether.kanesumi.navfocussnapshot",
            "焦点组快照",
            EtherRole::Browser,
            w,
            320.0,
        ),
        theme: theme_for(&scheme),
        scenario,
    };
    let mut host = TreeHost::new(app);
    if let Some(out) = arg("--snapshot") {
        let scale: f32 = args.last().and_then(|s| s.parse().ok()).unwrap_or(2.0);
        let font = find_font().expect("未找到可用字体（msyh / Noto CJK / DejaVu）");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (pw, ph) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            Size::new(w, 320.0),
            scale,
            3,
            std::path::Path::new(&out),
        )
        .expect("快照失败");
        println!("snapshot {out} {pw}x{ph} scenario={scenario} scheme={scheme}");
        return;
    }
    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
    #[cfg(not(target_os = "linux"))]
    println!("GUI 运行需要 Linux Wayland；用 --snapshot <out.png> [scale] 出图。");
}
