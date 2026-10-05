// elem_gaps —— 元素树补缺样张（2026-10-01）：Image 四 Stretch / NavigationView 左侧四态 / TabRow 溢出。
//
// 跨平台出图（CPU 光栅，不开窗）：
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene image   --snapshot docs/research/k_elem/image_stretch.png
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene nav-left      --snapshot docs/research/k_elem/nav_left.png
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene nav-collapsed --snapshot docs/research/k_elem/nav_left_collapsed.png
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene nav-compact   --snapshot docs/research/k_elem/nav_compact.png
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene nav-minimal   --snapshot docs/research/k_elem/nav_minimal.png
//   cargo run -p kanesumi-gallery --example elem_gaps -- --scene taboverflow   --snapshot docs/research/k_elem/tab_row_overflow.png

use std::path::PathBuf;

use kanesumi_controls::{
    MetroNavigationView, MetroTab, MetroTabRow, NavigationPaneMode, NavigationViewItem,
};
use kanesumi_core::{MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Image, Label, Stack, Stretch};
use kanesumi_harness::element::{Action, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

struct Sheet {
    config: AppConfig,
    theme: MetroTheme,
    scene: String,
    icon: PathBuf,
}

/// 生成一张 24×24 的 SVG 图标（橙底白勾）。
fn write_icon() -> PathBuf {
    let path = std::env::temp_dir().join("kanesumi_elem_gaps_icon.svg");
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
      <rect x="2" y="2" width="20" height="20" rx="4" fill="#E57812"/>
      <path d="M7 12.5l3.2 3.2L17 8.5" stroke="#FFFFFF" stroke-width="2.4"
            fill="none" stroke-linecap="round" stroke-linejoin="round"/>
    </svg>"##;
    let _ = std::fs::write(&path, svg);
    path
}

impl Sheet {
    fn build_image(&self, tree: &mut Tree) {
        let t = self.theme.typography;
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(16.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert(col, Label::new("Image · Stretch 四值").style(t.subheader));
        let row = tree.insert(col, Stack::row().with_spacing(16.0));
        for (name, stretch) in [
            ("None", Stretch::None),
            ("Uniform", Stretch::Uniform),
            ("UniformToFill", Stretch::UniformToFill),
            ("Fill", Stretch::Fill),
        ] {
            let cell = tree.insert(
                row,
                Stack::column().with_spacing(6.0),
            );
            tree.insert(cell, Label::new(name));
            let box_id = tree.insert_with(
                cell,
                Border::new()
                    .background(ThemeColor::SurfaceVariant)
                    .stroke(ThemeColor::Divider, 1.0),
                LayoutProps {
                    width: Some(104.0),
                    height: Some(64.0),
                    ..LayoutProps::default()
                },
            );
            tree.insert(box_id, Image::svg(&self.icon).stretch(stretch));
        }
    }

    fn build_taboverflow(&self, tree: &mut Tree) {
        let t = self.theme.typography;
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(16.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(16.0));
        tree.insert(col, Label::new("TabRow · 溢出滚动").style(t.subheader));
        let tabs = ["常规", "显示", "声音", "网络", "隐私", "更新", "关于"]
            .iter()
            .map(|s| MetroTab::new(*s))
            .collect();
        tree.insert_with(
            col,
            MetroTabRow::new(tabs),
            LayoutProps {
                width: Some(360.0),
                height: Some(48.0),
                ..LayoutProps::default()
            },
        );
    }

    fn build_nav(&self, tree: &mut Tree) {
        let t = self.theme.typography;
        let items = vec![
            NavigationViewItem::with_icon("常规", "⚙"),
            NavigationViewItem::with_icon("显示", "◐"),
            NavigationViewItem::with_icon("网络", "◇"),
            NavigationViewItem::new("关于"),
        ];
        let mut nav = MetroNavigationView::new(items);
        match self.scene.as_str() {
            "nav-collapsed" => {
                nav.set_pane_expanded(false);
                nav.set_pane_progress(0.0);
            }
            "nav-compact" => {
                nav.mode = NavigationPaneMode::LeftCompact;
                nav.set_pane_expanded(true);
                nav.set_pane_progress(1.0);
            }
            "nav-minimal" => {
                nav.mode = NavigationPaneMode::LeftMinimal;
                nav.set_pane_expanded(false);
                nav.set_pane_progress(0.0);
            }
            _ => {
                nav.set_pane_progress(1.0);
            }
        }
        nav.selected = Some(vec![0]);
        let page = tree.insert(
            tree.root(),
            Border::new().background(ThemeColor::Background),
        );
        let nav_id = tree.insert(page, nav);
        let content = tree.insert(
            nav_id,
            Border::new()
                .background(ThemeColor::SurfaceVariant)
                .padding(Insets::all(16.0)),
        );
        let inner = tree.insert(content, Stack::column().with_spacing(8.0));
        tree.insert(inner, Label::new("页面内容区").style(t.title));
        tree.insert(
            inner,
            Label::new("内容从 pane 推挤宽之后起算，点左侧类目不会被内容截走。")
                .color(ThemeColor::OnSurfaceVariant)
                .wrap(Some(3)),
        );
    }
}

impl TreeApp for Sheet {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        match self.scene.as_str() {
            "taboverflow" => self.build_taboverflow(tree),
            "nav-left" | "nav-collapsed" | "nav-compact" | "nav-minimal" => self.build_nav(tree),
            _ => self.build_image(tree),
        }
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

/// 样张尺寸（按场景）。
fn scene_size(scene: &str) -> (f32, f32) {
    match scene {
        "taboverflow" => (420.0, 140.0),
        "nav-left" | "nav-collapsed" | "nav-compact" | "nav-minimal" => (520.0, 360.0),
        _ => (520.0, 160.0),
    }
}

/// 字体查找（环境变量优先，其次已知的 Windows / Linux 路径）。
fn find_font() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

#[cfg(target_os = "linux")]
fn run_gui(host: kanesumi_harness::TreeHost<Sheet>) {
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn run_gui(_host: kanesumi_harness::TreeHost<Sheet>) {
    println!("elem_gaps 需要 Linux Wayland 会话；用 --scene <name> --snapshot <out.png> 出图。");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scene = args
        .iter()
        .position(|a| a == "--scene")
        .and_then(|i| args.get(i + 1))
        .cloned()
        .unwrap_or_else(|| "image".to_string());
    let (w, h) = scene_size(&scene);
    let app = Sheet {
        config: AppConfig::new(
            "org.ether.kanesumi.elemgaps",
            "元素树补缺样张",
            EtherRole::Browser,
            w,
            h,
        ),
        theme: MetroTheme::ether_dark(),
        scene: scene.clone(),
        icon: write_icon(),
    };
    let mut host = kanesumi_harness::TreeHost::new(app);

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (pw, ph) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            Size::new(w, h),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {pw}x{ph} scene={scene} font={}", font.display());
        return;
    }
    run_gui(host);
}
