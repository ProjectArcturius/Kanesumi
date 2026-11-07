// table_rows —— 表格行参照页：表头 + 多列行（展开三角 / 缩进 / 图标 / 选中 / 排序箭头）。
//
// 这张卡回答的是「librarian 那种密集列表要不要新控件」。要，因为它与设置页的列表行不同形：
//
//     MetroTableHeader（列标题 + 排序箭头）
//     MetroTableRow × N（三角槽 + 图标槽 + 多列文本 + 选中底）
//
// 两处都消费同一个 `column_rects`，故表头与行的列边界永远一致 —— librarian 现状是把列几何
// 算两遍（`Cols` + 表头自己的 `divider_at`），列宽一改就得两处同改。
// 行内的命中、悬停、焦点、右键、双击由元素树投给调用方，行自己不算区域枚举。
// 参 docs/CONTROL_SPEC.md §51。
//
// 出图：cargo run -p kanesumi-gallery --example table_rows -- --snapshot out.png 1

use kanesumi_canvas::{Icon, rasterize_svg};
use kanesumi_controls::table::{self, Column, ColumnSet, ColumnWidth, MetroTableRow};
use kanesumi_controls::{
    MetroTableHeader, SortRequested, TableRowActivated, TableRowClicked, TableRowToggled,
};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

/// 列表宽（逻辑）。
const LIST_W: f32 = 640.0;
/// 画布尺寸。
const PAGE_W: f32 = 680.0;
const PAGE_H: f32 = 400.0;

/// 列：名称吃余量、修改日期 180、大小 90、类型 140（与 librarian `1fr 180px 90px 140px` 同）。
fn columns() -> ColumnSet {
    ColumnSet::new([
        Column::new("名称", ColumnWidth::Star),
        Column::new("修改日期", ColumnWidth::Px(180.0)),
        Column::new("大小", ColumnWidth::Px(90.0)).align(table::ColumnAlign::End),
        Column::new("类型", ColumnWidth::Px(140.0)),
    ])
}

/// 一行数据（示例用，真实形态由数据层给）。
struct Entry {
    name: &'static str,
    date: &'static str,
    size: &'static str,
    kind: &'static str,
    dir: bool,
    indent: u8,
    expand: Option<bool>,
    selected: bool,
}

struct TableSheet {
    config: AppConfig,
    theme: MetroTheme,
    folder: Option<Icon>,
    file: Option<Icon>,
    sort: (usize, bool),
    selection: usize,
    expanded: bool,
}

impl TableSheet {
    fn entries(&self) -> Vec<Entry> {
        vec![
            Entry {
                name: "项目",
                date: "2026-11-07 09:12",
                size: "--",
                kind: "文件夹",
                dir: true,
                indent: 0,
                expand: Some(self.expanded),
                selected: false,
            },
            Entry {
                name: "docs",
                date: "2026-11-06 21:40",
                size: "--",
                kind: "文件夹",
                dir: true,
                indent: 1,
                expand: None,
                selected: false,
            },
            Entry {
                name: "src",
                date: "2026-11-05 18:03",
                size: "--",
                kind: "文件夹",
                dir: true,
                indent: 1,
                expand: None,
                selected: false,
            },
            Entry {
                name: "笔记.md",
                date: "2026-11-07 08:55",
                size: "12 KB",
                kind: "Markdown 文档",
                dir: false,
                indent: 0,
                expand: None,
                selected: self.selection == 0,
            },
            Entry {
                name: "设计稿总览-第三轮修订.pdf",
                date: "2026-11-02 14:20",
                size: "3.4 MB",
                kind: "PDF 文档",
                dir: false,
                indent: 0,
                expand: None,
                selected: self.selection == 1,
            },
            Entry {
                name: "备份.tar.gz",
                date: "2026-10-30 23:59",
                size: "182 MB",
                kind: "压缩文件",
                dir: false,
                indent: 0,
                expand: None,
                selected: self.selection == 2,
            },
        ]
    }
}

impl TreeApp for TableSheet {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new().background(ThemeColor::Background),
        );
        let list = tree.insert_with(
            page,
            Stack::column(),
            LayoutProps {
                width: Some(LIST_W),
                margin: Insets::new(20.0, 20.0, 20.0, 20.0),
                ..LayoutProps::default()
            },
        );

        let header = MetroTableHeader::new(columns()).with_sort(self.sort.0, self.sort.1);
        tree.insert(list, header);
        let cols = columns();
        for (i, e) in self.entries().into_iter().enumerate() {
            let mut row = MetroTableRow::new(i, cols.clone(), [e.name, e.date, e.size, e.kind])
                .with_indent(e.indent)
                .with_selected(e.selected);
            let icon = if e.dir {
                self.folder.clone()
            } else {
                self.file.clone()
            };
            if let Some(icon) = icon {
                row = row.with_icon(icon);
            }
            if let Some(expanded) = e.expand {
                row = row.with_expandable(expanded);
            }
            tree.insert(list, row);
        }
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(s) = action.downcast_ref::<SortRequested>() {
            self.sort = (s.column, s.ascending);
        } else if let Some(c) = action.downcast_ref::<TableRowClicked>() {
            // 前 3 行是目录（不参与「文件选中」），点文件行改选择。真实应用按自己的路由分派。
            if c.index >= 3 {
                self.selection = c.index - 3;
            }
        } else if let Some(t) = action.downcast_ref::<TableRowToggled>() {
            if t.0 == 0 {
                self.expanded = !self.expanded;
            }
        } else if action.downcast_ref::<TableRowActivated>().is_some() {
            // 双击 = 打开（示例不真的开）。
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

fn load_icon(name: &str, size: u32) -> Option<Icon> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("assets/icons")
        .join(name);
    rasterize_svg(&path, size)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let theme = if args.iter().any(|a| a == "--light") {
        MetroTheme::ether_light()
    } else {
        MetroTheme::ether_dark()
    };
    let mut host = kanesumi_harness::TreeHost::new(TableSheet {
        config: AppConfig::new(
            "org.ether.table-rows-demo",
            "表格行",
            EtherRole::Browser,
            PAGE_W,
            PAGE_H,
        ),
        theme,
        folder: load_icon("folder.svg", 16),
        file: load_icon("file.svg", 16),
        sort: (1, true),
        selection: 0,
        expanded: true,
    });
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args
            .get(i + 1)
            .map(String::as_str)
            .unwrap_or("table_rows.png");
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
        println!(
            "table_rows 实时窗口需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。"
        );
    }
}
