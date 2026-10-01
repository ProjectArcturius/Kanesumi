// frame_bench —— 元素树运行时基准（平台无关，Windows 可跑）。
//
// 对应任务 k-perf 第 1 步：先量后改。两个场景：
//   A「长列表」：根 = MetroScrollView + 2000 行（图标方块 MetroButton + 两行 Label）。
//   B「设置页」：MetroNavigationView + 一页 40 个控件（Switch / Slider / Button / TextBox）。
// 每个场景测：首帧 / 指针悬停移动 200 次（每次跨一行）/ 滚轮 100 次；
// 每次 = `Tree::frame` + `CpuRenderer::render`。输出 p50 / p95 / max（毫秒）与命令条数。
//
// 运行：cargo run --release -p kanesumi-harness --example frame_bench

use std::time::Instant;

use kanesumi_canvas::text::TextEngine;
use kanesumi_controls::{
    MetroButton, MetroNavigationView, MetroScrollView, MetroSlider, MetroSwitch, MetroTextBox,
    NavigationViewItem,
};
use kanesumi_core::{MetroTheme, Point, Size};
use kanesumi_element::testing::find_test_font;
use kanesumi_element::widgets::{Label, Stack};
use kanesumi_element::{Align, LayoutProps, Modifiers, Tree, WidgetId};
use kanesumi_harness::CpuRenderer;

const W: f32 = 1280.0;
const H: f32 = 800.0;
const DT: f64 = 1.0 / 60.0;

/// 铺满可用矩形。
fn full() -> LayoutProps {
    LayoutProps {
        h_align: Align::Stretch,
        v_align: Align::Stretch,
        ..LayoutProps::default()
    }
}

/// 靠左、按内容定尺寸。
fn start() -> LayoutProps {
    LayoutProps {
        h_align: Align::Start,
        ..LayoutProps::default()
    }
}

/// 固定矩形。
fn fixed(w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

/// 场景 A：长列表（滚动容器 + 2000 行）。返回树与可悬停的图标方块 id。
fn build_long_list(theme: MetroTheme) -> (Tree, Vec<WidgetId>) {
    let mut tree = Tree::new(theme);
    let root = tree.root();
    let mut scroll = MetroScrollView::default();
    scroll.smooth_scroll = false;
    let sv = tree.insert_with(root, scroll, full());
    let col = tree.insert(sv, Stack::column());
    let mut icons = Vec::with_capacity(2000);
    for i in 0..2000 {
        let row = tree.insert_with(
            col,
            Stack::row().with_spacing(8.0),
            LayoutProps {
                height: Some(48.0),
                h_align: Align::Stretch,
                ..LayoutProps::default()
            },
        );
        // 图标方块用可交互的 MetroButton（空标签）—— 悬停触发视觉状态重画。
        let icon = tree.insert_with(row, MetroButton::accent(""), fixed(32.0, 32.0));
        icons.push(icon);
        let texts = tree.insert(row, Stack::column());
        tree.insert(texts, Label::new(format!("行 {i}")));
        tree.insert(texts, Label::new(format!("副标题 {i}")));
    }
    (tree, icons)
}

/// 场景 B：设置页（NavigationView + 40 个混合控件）。返回树与控件 id。
fn build_settings(theme: MetroTheme) -> (Tree, Vec<WidgetId>) {
    let mut tree = Tree::new(theme);
    let root = tree.root();
    let nav = tree.insert_with(
        root,
        MetroNavigationView::new(vec![
            NavigationViewItem::new("常规"),
            NavigationViewItem::new("显示"),
        ]),
        full(),
    );
    let page = tree.insert(nav, Stack::column().with_spacing(10.0));
    let mut controls = Vec::with_capacity(40);
    for i in 0..40 {
        let id = match i % 4 {
            0 => tree.insert_with(page, MetroSwitch::with_header(format!("开关 {i}")), start()),
            1 => tree.insert_with(
                page,
                MetroSlider::new().with_header(format!("滑杆 {i}")),
                start(),
            ),
            2 => tree.insert_with(page, MetroButton::new(format!("按钮 {i}")), start()),
            _ => tree.insert_with(
                page,
                MetroTextBox::with_placeholder(format!("输入 {i}")),
                start(),
            ),
        };
        controls.push(id);
    }
    (tree, controls)
}

/// 时延样本集（毫秒）。
#[derive(Default)]
struct Stats {
    v: Vec<f64>,
}

impl Stats {
    fn add(&mut self, x: f64) {
        self.v.push(x);
    }
    /// 返回 (p50, p95, max)。
    fn report(&mut self) -> (f64, f64, f64) {
        if self.v.is_empty() {
            return (0.0, 0.0, 0.0);
        }
        self.v.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let pct = |p: f64| -> f64 {
            let idx = ((self.v.len() as f64 - 1.0) * p).round() as usize;
            self.v[idx]
        };
        (pct(0.5), pct(0.95), *self.v.last().unwrap())
    }
    fn reset(&mut self) {
        self.v.clear();
    }
}

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

/// 跑一个场景：首帧 → 悬停 200（跨目标）→ 滚轮 100。每次 `frame` + `render`。
fn scenario(name: &str, tree: &mut Tree, engine: &TextEngine, targets: &[WidgetId]) {
    let size = Size::new(W, H);
    let mut cpu = CpuRenderer::new(W, H, 1.0);

    // 首帧（全量）。
    let t = Instant::now();
    let out = tree.frame(engine, size, DT);
    let first_tree = ms(t);
    let t = Instant::now();
    let _ = cpu.render(engine, &out.scene, out.damage);
    let first_raster = ms(t);
    println!(
        "{name}: 首帧 tree {first_tree:.2} ms | raster {first_raster:.2} ms | 命令 {} 条",
        out.scene.commands.len()
    );

    // 悬停目标：落在表面内的控件中心。
    let centers: Vec<Point> = targets
        .iter()
        .filter_map(|id| tree.rect(*id))
        .map(|r| r.center())
        .filter(|p| p.x >= 0.0 && p.y >= 0.0 && p.x < W && p.y < H)
        .collect();

    let mut frame_s = Stats::default();
    let mut raster_s = Stats::default();
    let mut cmds = Vec::new();
    for i in 0..200 {
        let p = if centers.is_empty() {
            Point::new(W / 2.0, H / 2.0)
        } else {
            centers[i % centers.len()]
        };
        tree.pointer_move(p);
        let t = Instant::now();
        let out = tree.frame(engine, size, DT);
        frame_s.add(ms(t));
        cmds.push(out.scene.commands.len());
        let t = Instant::now();
        let _ = cpu.render(engine, &out.scene, out.damage);
        raster_s.add(ms(t));
    }
    report("悬停200", &mut frame_s, &mut raster_s, &cmds);

    // 滚轮 100。
    frame_s.reset();
    raster_s.reset();
    cmds.clear();
    let viewport = Point::new(W / 2.0, H / 2.0);
    for _ in 0..100 {
        tree.scroll(viewport, 0.0, 50.0, Modifiers::NONE);
        let t = Instant::now();
        let out = tree.frame(engine, size, DT);
        frame_s.add(ms(t));
        cmds.push(out.scene.commands.len());
        let t = Instant::now();
        let _ = cpu.render(engine, &out.scene, out.damage);
        raster_s.add(ms(t));
    }
    report("滚轮100", &mut frame_s, &mut raster_s, &cmds);
}

fn report(label: &str, frame_s: &mut Stats, raster_s: &mut Stats, cmds: &[usize]) {
    let (f50, f95, fmax) = frame_s.report();
    let (r50, r95, rmax) = raster_s.report();
    let cmin = cmds.iter().copied().min().unwrap_or(0);
    let cmax = cmds.iter().copied().max().unwrap_or(0);
    let cavg = if cmds.is_empty() {
        0
    } else {
        cmds.iter().sum::<usize>() / cmds.len()
    };
    println!(
        "  {label}: tree p50 {f50:.2} p95 {f95:.2} max {fmax:.2} | \
         raster p50 {r50:.2} p95 {r95:.2} max {rmax:.2} | 命令 {cmin}/{cavg}/{cmax}"
    );
}

fn main() {
    let font = find_test_font().expect("找不到测试字体：设 KANESUMI_TEST_FONT");
    let engine = TextEngine::load(&font).expect("字体加载失败");
    println!("=== frame_bench === font: {}", font.display());
    let theme = MetroTheme::ether_dark();

    let (mut list, icons) = build_long_list(theme);
    scenario("long_list", &mut list, &engine, &icons);

    let (mut settings, controls) = build_settings(theme);
    scenario("settings", &mut settings, &engine, &controls);
}
