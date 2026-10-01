// virtual_list —— 元素树虚拟化列表演示（ItemsRepeater，参 docs/ELEMENT_TREE.md §Ⅹ E4）。
//
// 一万行：每行 = 图标块 + 两行文字，悬停变色。整个列表只实现视口 + overscan 的行，
// 滚轮 / 键盘 / 点击全部走元素树，无一手写坐标、无一命中函数。
//
// 出快照 PNG（任意平台）：
//   cargo run -p kanesumi-gallery --example virtual_list -- --snapshot out.png 1
//
// 计时（release）：加 `--bench` 打印首帧与滚轮 100 次 `Tree::frame` 的 p50 / p95。

use std::cell::RefCell;
use std::rc::Rc;

use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_controls::{ItemFactory, ItemsRepeater};
use kanesumi_core::{MetroTheme, Rect, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{
    Action, Event, EventCtx, Insets, LayoutProps, MeasureCtx, PaintCtx, RealizeCtx, Widget,
    WidgetId,
};
use kanesumi_harness::element::tree::Tree;
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

const ROWS: usize = 10_000;
const ROW_H: f32 = 56.0;
const SHEET_W: f32 = 560.0;
const SHEET_H: f32 = 640.0;

/// 行数据。
#[derive(Debug, Clone)]
struct RowData {
    title: String,
    subtitle: String,
}

fn sample_data(count: usize) -> Vec<RowData> {
    (0..count)
        .map(|i| RowData {
            title: format!("条目 {i} —— Librarian 目录项"),
            subtitle: format!("/home/ether/documents/项目/{i:05}/说明.txt · {i} B"),
        })
        .collect()
}

/// 元素树动作：某行被激活。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RowActivated(pub usize);

/// 一行：图标块 + 标题 + 副标题；悬停变色。
struct Row {
    index: usize,
    title: String,
    subtitle: String,
}

impl Widget for Row {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        Size::new(0.0, ROW_H)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let rect = ctx.rect();
        let theme = ctx.theme();
        let hovered = ctx.state().hovered;
        if hovered {
            scene.fill_rect(theme.colors.surface_variant, rect);
        } else {
            scene.fill_rect(theme.colors.surface, rect);
        }
        // 图标块（28×28），纵向居中。
        let icon = Rect::new(
            rect.origin.x + 16.0,
            rect.origin.y + (rect.size.height - 28.0) / 2.0,
            28.0,
            28.0,
        );
        scene.fill_rect(theme.colors.primary, icon);
        // 两行文字。
        let text_x = icon.right() + 14.0;
        let text_w = (rect.right() - text_x - 12.0).max(0.0);
        scene.label(
            self.title.clone(),
            Rect::new(text_x, rect.origin.y + 9.0, text_w, 20.0),
            theme.colors.on_surface,
            theme.typography.body,
            TextAlign::Left,
        );
        scene.label(
            self.subtitle.clone(),
            Rect::new(text_x, rect.origin.y + 31.0, text_w, 16.0),
            theme.colors.on_surface_variant,
            theme.typography.caption,
            TextAlign::Left,
        );
        // 行分隔线（底部 1px）。
        scene.fill_rect(
            theme.colors.divider,
            Rect::new(rect.origin.x, rect.bottom() - 1.0, rect.size.width, 1.0),
        );
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if matches!(event, Event::Click) {
            ctx.emit(RowActivated(self.index));
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        true
    }
}

/// 一万行数据源（键 = 索引）。
struct SampleFactory {
    data: Rc<RefCell<Vec<RowData>>>,
}

impl ItemFactory for SampleFactory {
    fn len(&self) -> usize {
        self.data.borrow().len()
    }

    fn key(&self, index: usize) -> u64 {
        index as u64
    }

    fn build(&mut self, index: usize) -> Box<dyn Widget> {
        let data = self.data.borrow();
        let row = Row {
            index,
            title: data[index].title.clone(),
            subtitle: data[index].subtitle.clone(),
        };
        Box::new(row)
    }

    fn bind(&mut self, index: usize, ctx: &mut RealizeCtx, node: WidgetId) {
        let data = self.data.borrow();
        let title = data[index].title.clone();
        let subtitle = data[index].subtitle.clone();
        ctx.edit::<Row, _>(node, |row, _| {
            row.index = index;
            row.title = title;
            row.subtitle = subtitle;
        });
    }
}

struct Demo {
    config: AppConfig,
    theme: MetroTheme,
    data: Rc<RefCell<Vec<RowData>>>,
    status: Option<WidgetId>,
    list: Option<WidgetId>,
}

impl Demo {
    fn new() -> Self {
        let theme = MetroTheme::ether_dark();
        Self {
            config: AppConfig::new(
                "org.ether.kanesumi.virtuallist",
                "虚拟化列表",
                EtherRole::Browser,
                SHEET_W,
                SHEET_H,
            ),
            theme,
            data: Rc::new(RefCell::new(sample_data(ROWS))),
            status: None,
            list: None,
        }
    }

    fn set_status(&self, tree: &mut Tree, text: String) {
        if let Some(id) = self.status {
            tree.edit::<Label, _>(id, |l, _| l.text = text);
        }
    }
}

impl TreeApp for Demo {
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
        let col = tree.insert(page, Stack::column().with_spacing(10.0));
        tree.insert(
            col,
            Label::new(format!("虚拟化列表 · {ROWS} 行"))
                .style(self.theme.typography.title),
        );
        self.status = Some(tree.insert(
            col,
            Label::new("只实现视口 + overscan 的行；滚轮 / 方向键 / 点击走元素树")
                .color(ThemeColor::OnSurfaceVariant),
        ));
        let factory = SampleFactory {
            data: self.data.clone(),
        };
        self.list = Some(tree.insert_with(
            col,
            ItemsRepeater::stack(ROW_H, factory),
            LayoutProps {
                // grow = 吃掉标题之外的全部高度（Stack 主轴分配），列表得到有界视口。
                grow: 1.0,
                ..LayoutProps::default()
            },
        ));
    }

    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action) {
        if let Some(RowActivated(i)) = action.downcast_ref::<RowActivated>() {
            self.set_status(tree, format!("点击：条目 {i}（来源 {from:?}）"));
        }
    }
}

fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    for p in [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn run_gui(host: TreeHost<Demo>) {
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn run_gui(_host: TreeHost<Demo>) {
    println!("virtual_list 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
}

/// 首帧 + 滚轮 100 次的 `Tree::frame` 耗时（release 才有意义）。
fn bench(engine: &kanesumi_canvas::text::TextEngine) {
    use kanesumi_harness::{App, InputEvent, Modifiers};
    use std::time::Instant;

    let size = Size::new(SHEET_W, SHEET_H);
    let mut host = TreeHost::new(Demo::new());
    // 首帧：首次 realize（实现可见行）+ 全量布局 + 全量绘制。
    let t = Instant::now();
    host.update(1.0 / 60.0);
    let mut scene = Scene::default();
    host.render_into(engine, size, &mut scene);
    let first = t.elapsed();

    // 滚轮事件不带坐标，外壳用最近指针位置命中 → 先把指针移到列表内。
    let at = InputEvent::PointerMoved {
        x: 120.0,
        y: 300.0,
    };
    host.handle_input(at);
    let mut times = Vec::with_capacity(100);
    for _ in 0..100 {
        host.handle_input(InputEvent::Scroll {
            x: 0.0,
            y: 50.0,
            modifiers: Modifiers::NONE,
        });
        let t = Instant::now();
        host.update(1.0 / 60.0);
        let mut scene = Scene::default();
        host.render_into(engine, size, &mut scene);
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    let offset = host
        .app()
        .list
        .and_then(|id| host.tree().get::<ItemsRepeater>(id))
        .map_or(0.0, |r| r.offset());
    let nodes = host
        .app()
        .list
        .map_or(0, |id| host.tree().children(id).len());
    times.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let pick = |q: f64| times[((q * times.len() as f64) as usize).min(times.len() - 1)];
    println!(
        "bench {ROWS} 行：首帧 {:.2} ms；滚轮100次 frame p50 {:.2} ms p95 {:.2} ms（offset {offset:.0}，存活行节点 {nodes}）",
        first.as_secs_f64() * 1000.0,
        pick(0.50),
        pick(0.95)
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let font = find_font().expect("未找到字体");
    let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let mut host = TreeHost::new(Demo::new());
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            Size::new(SHEET_W, SHEET_H),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }
    if args.iter().any(|a| a == "--bench") {
        bench(&engine);
        return;
    }
    run_gui(TreeHost::new(Demo::new()));
}


