// grid_form —— 两列表单参照页（`Grid` + 元件调用，零手写坐标）。
//
// 这张卡证明的是「开发者只需调用元件」：整页没有一个 `Rect::new`、没有一个命中函数、
// 没有一处 margin 对齐凑列 —— 行列由 `Grid::form` + `LayoutProps::in_cell` 决定，
// 交互由控件动作驱动，Tab 顺序由树序派生。设置页要照抄的就是这个结构。
//
// 运行（Linux，任意支持 xdg-shell 的合成器）：
//   cargo run -p kanesumi-gallery --example grid_form
// 出快照 PNG（任意平台）：
//   cargo run -p kanesumi-gallery --example grid_form -- --snapshot out.png 1

use kanesumi_controls::{
    ButtonClicked, CheckState, CheckToggled, MetroButton, MetroCheckBox, MetroDropDownButton,
    MetroTextBox, TextChanged,
};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Grid, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

/// 保存下来的节点 id（要回写的控件）。
#[derive(Default)]
struct Ids {
    name: Option<WidgetId>,
    bluetooth: Option<WidgetId>,
    summary: Option<WidgetId>,
}

struct Form {
    config: AppConfig,
    theme: MetroTheme,
    ids: Ids,
    name: String,
    bluetooth: bool,
}

impl Form {
    /// 一行表单：左列标签、右列控件。**这里是整页唯一的「列」概念**，且它只声明一次。
    fn row(tree: &mut Tree, form: WidgetId, r: usize, label: &str, control: impl kanesumi_harness::element::Widget) -> WidgetId {
        tree.insert_with(
            form,
            Label::new(label).color(ThemeColor::OnSurfaceVariant),
            LayoutProps::in_cell(r, 0),
        );
        tree.insert_with(form, control, LayoutProps::in_cell(r, 1))
    }

    fn refresh_summary(&self, tree: &mut Tree) {
        let Some(id) = self.ids.summary else {
            return;
        };
        let name = if self.name.is_empty() {
            "（未填写）"
        } else {
            self.name.as_str()
        };
        let bt = if self.bluetooth { "开" } else { "关" };
        tree.edit::<Label, _>(id, |l, _| {
            l.text = format!("当前：设备名 = {name}，蓝牙 = {bt}");
        });
    }
}

impl TreeApp for Form {
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
        let col = tree.insert(page, Stack::column().with_spacing(16.0));
        tree.insert(
            col,
            Label::new("设备设置").style(self.theme.typography.title),
        );

        // 左列固定 140，右列吃满剩余；行高随内容，行距 8、列距 16。
        let form = tree.insert(col, Grid::form(140.0).with_gap(8.0, 16.0));
        self.ids.name = Some(Self::row(
            tree,
            form,
            0,
            "设备名称",
            MetroTextBox::with_placeholder("例如：书房台式机"),
        ));
        self.ids.bluetooth = Some(Self::row(
            tree,
            form,
            1,
            "蓝牙",
            MetroCheckBox::new("启用蓝牙").with_checked(self.bluetooth),
        ));
        Self::row(
            tree,
            form,
            2,
            "打印质量",
            MetroDropDownButton::new(
                "标准",
                vec![
                    kanesumi_controls::MenuItem::new("草稿"),
                    kanesumi_controls::MenuItem::new("标准"),
                    kanesumi_controls::MenuItem::new("高质量"),
                ],
            ),
        );

        // 说明行跨两列：跨度同样只是属性。
        let summary = tree.insert_with(
            col,
            Label::new(String::new()).color(ThemeColor::OnSurfaceVariant),
            LayoutProps::default(),
        );
        self.ids.summary = Some(summary);
        self.refresh_summary(tree);

        // 操作行：靠右对齐，两个按钮；行内用 Stack，不再关心任何坐标。
        let actions = tree.insert_with(
            col,
            Stack::row().with_spacing(8.0),
            LayoutProps {
                h_align: Align::End,
                ..LayoutProps::default()
            },
        );
        tree.insert(actions, MetroButton::new("取消"));
        tree.insert(actions, MetroButton::accent("保存"));
    }

    fn on_action(&mut self, tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(changed) = action.downcast_ref::<TextChanged>() {
            self.name = changed.0.clone();
            self.refresh_summary(tree);
        } else if let Some(toggled) = action.downcast_ref::<CheckToggled>() {
            self.bluetooth = toggled.0 == CheckState::Checked;
            self.refresh_summary(tree);
        } else if action.is::<ButtonClicked>() {
            self.refresh_summary(tree);
        }
    }
}

fn app() -> Form {
    Form {
        config: AppConfig::new(
            "org.ether.kanesumi.gridform",
            "设备设置",
            EtherRole::Browser,
            560.0,
            420.0,
        ),
        theme: MetroTheme::ether_dark(),
        ids: Ids::default(),
        name: String::new(),
        bluetooth: false,
    }
}

/// 字体查找（跨平台）：`KANESUMI_TEST_FONT` 优先，其次各系统常见路径。
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
    let mut host = kanesumi_harness::TreeHost::new(app());
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).map(String::as_str).unwrap_or("grid_form.png");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(560.0, 420.0),
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
        println!("grid_form 实时窗口需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
    }
}
