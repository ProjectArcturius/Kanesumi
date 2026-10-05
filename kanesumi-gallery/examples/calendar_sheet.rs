// calendar_sheet —— 日历控件样张（MetroCalendarView / MetroCalendarDatePicker）。
//
// 深 / 浅各出一张，核对日格、今天 / 选中描边、三级视图与触发器配色是否随主题走
// （一律 `ThemeColor` / `MetroTheme`，不写字面量）。
//
// 运行（任意平台）：
//   cargo run -p kanesumi-gallery --example calendar_sheet -- --scheme light --snapshot out.png 2

use kanesumi_canvas::text::TextEngine;
use kanesumi_controls::{CalendarDisplayMode, Date, MetroCalendarDatePicker, MetroCalendarView};
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};

const SHEET_W: f32 = 1240.0;
const SHEET_H: f32 = 720.0;

struct Sheet {
    config: AppConfig,
    theme: MetroTheme,
}

fn start() -> LayoutProps {
    LayoutProps {
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

fn heading(tree: &mut Tree, parent: WidgetId, text: &str, sheet: &Sheet) {
    tree.insert(
        parent,
        Label::new(text)
            .style(sheet.theme.typography.body)
            .color(ThemeColor::Primary),
    );
}

fn calendar(mode: CalendarDisplayMode, selected: Option<Date>, today: Date) -> MetroCalendarView {
    let mut view = MetroCalendarView::new(today);
    if let Some(d) = selected {
        view = view.with_selected(d);
    }
    view.display_mode = mode;
    view
}

impl TreeApp for Sheet {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let t = self.theme.typography;
        let today = Date::new(2026, 10, 1);
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));
        tree.insert(
            col,
            Label::new(match self.theme.scheme {
                ColorScheme::Dark => "Kanesumi 日历样张 · Dark",
                ColorScheme::Light => "Kanesumi 日历样张 · Light",
            })
            .style(t.subheader),
        );

        let row = tree.insert(col, Stack::row().with_spacing(24.0));

        // Month（选中 10-15）。
        let month_col = tree.insert(row, Stack::column().with_spacing(6.0));
        heading(tree, month_col, "CalendarView · Month", self);
        tree.insert_with(
            month_col,
            calendar(
                CalendarDisplayMode::Month,
                Some(Date::new(2026, 10, 15)),
                today,
            ),
            start(),
        );

        // Year（12 月）。
        let year_col = tree.insert(row, Stack::column().with_spacing(6.0));
        heading(tree, year_col, "CalendarView · Year", self);
        tree.insert_with(
            year_col,
            calendar(CalendarDisplayMode::Year, None, today),
            start(),
        );

        // Decade（12 年）。
        let decade_col = tree.insert(row, Stack::column().with_spacing(6.0));
        heading(tree, decade_col, "CalendarView · Decade", self);
        tree.insert_with(
            decade_col,
            calendar(CalendarDisplayMode::Decade, None, today),
            start(),
        );

        // DatePicker：未选 / 已选。
        heading(tree, col, "CalendarDatePicker", self);
        let picker_row = tree.insert(col, Stack::row().with_spacing(16.0));
        tree.insert_with(picker_row, MetroCalendarDatePicker::new(today), start());
        tree.insert_with(
            picker_row,
            MetroCalendarDatePicker::new(today).with_selected(Date::new(2026, 3, 8)),
            start(),
        );
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
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
fn run_gui(host: TreeHost<Sheet>) {
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn run_gui(_host: TreeHost<Sheet>) {
    println!("calendar_sheet 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scheme = match args.iter().position(|a| a == "--scheme") {
        Some(i) if args.get(i + 1).map(String::as_str) == Some("light") => ColorScheme::Light,
        _ => ColorScheme::Dark,
    };
    let theme = MetroTheme::for_scheme(scheme, Accent::default());
    let app = Sheet {
        config: AppConfig::new(
            "org.ether.kanesumi.calendarsheet",
            "日历样张",
            EtherRole::Browser,
            SHEET_W,
            SHEET_H,
        ),
        theme,
    };
    let mut host = TreeHost::new(app);

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            Size::new(SHEET_W, SHEET_H),
            scale,
            3,
            std::path::Path::new(&out),
        )
        .expect("快照失败");
        println!(
            "snapshot {out} {w}x{h} scheme={scheme:?} font={}",
            font.display()
        );
        return;
    }
    run_gui(host);
}
