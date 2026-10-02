// tree_demo —— 元素树参照页（ELEMENT_TREE §Ⅹ E2 验收）。
//
// 整页**没有一个手写坐标、没有一个命中函数、没有焦点登记**：布局由 Stack/Border 与框架属性
// 决定，交互由控件动作驱动，Tab 顺序由树序派生。这是 E5 移植 Ether 应用时的写法样板。
//
// 运行（Linux，任意支持 xdg-shell 的合成器）：
//   cargo run -p kanesumi-gallery --example tree_demo

use kanesumi_controls::{
    ButtonClicked, CheckState, CheckToggled, MetroButton, MetroCheckBox, MetroTextBox,
    TextChanged, TextSubmitted,
};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{
    Action, Align, Insets, LayoutProps, PopupDismissed, PopupSpec, Tree, WidgetId,
};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

#[derive(Default)]
struct Ids {
    name: Option<WidgetId>,
    bluetooth: Option<WidgetId>,
    auto: Option<WidgetId>,
    cancel: Option<WidgetId>,
    apply: Option<WidgetId>,
    status: Option<WidgetId>,
    dialog: Option<WidgetId>,
    dialog_ok: Option<WidgetId>,
}

struct Demo {
    config: AppConfig,
    theme: MetroTheme,
    ids: Ids,
    name: String,
}

/// 靠左、按内容定宽（行内控件的常用属性）。
fn start() -> LayoutProps {
    LayoutProps {
        h_align: Align::Start,
        ..LayoutProps::default()
    }
}

impl Demo {
    fn set_status(&self, tree: &mut Tree, text: String) {
        if let Some(id) = self.ids.status {
            tree.edit::<Label, _>(id, |l, _| l.text = text);
        }
    }

    fn open_dialog(&mut self, tree: &mut Tree) {
        let panel = tree.open_popup(
            Border::new()
                .background(ThemeColor::Surface)
                .stroke(ThemeColor::Divider, 1.0)
                .padding(Insets::all(20.0)),
            PopupSpec {
                modal: true,
                ..PopupSpec::default()
            },
        );
        let col = tree.insert(panel, Stack::column().with_spacing(16.0));
        tree.insert(
            col,
            Label::new("设置已应用").style(self.theme.typography.title),
        );
        tree.insert(
            col,
            Label::new(format!("设备名称：{}", if self.name.is_empty() { "（未填写）" } else { &self.name })),
        );
        let ok = tree.insert_with(
            col,
            MetroButton::accent("确定"),
            LayoutProps {
                h_align: Align::End,
                ..LayoutProps::default()
            },
        );
        tree.focus(ok, true);
        self.ids.dialog = Some(panel);
        self.ids.dialog_ok = Some(ok);
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
        let t = self.theme.typography;
        let page = tree.insert(
            tree.root(),
            Border::new().background(ThemeColor::Background).padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(12.0));

        tree.insert(col, Label::new("元素树参照页").style(t.page_heading));
        tree.insert(
            col,
            Label::new(
                "这一页没有手写坐标、没有命中函数、没有焦点登记。缩放窗口看布局重排，\
                 Tab / Shift+Tab 走焦点，Enter / Space 激活，长标签自动省略。",
            )
            .color(ThemeColor::OnSurfaceVariant)
            .wrap(Some(3)),
        );

        self.ids.name = Some(tree.insert_with(
            col,
            MetroTextBox::with_header("设备名称").with_text("ether-dev"),
            LayoutProps {
                max: kanesumi_core::Size::new(360.0, f32::INFINITY),
                ..LayoutProps::default()
            },
        ));
        self.name = "ether-dev".into();
        self.ids.bluetooth = Some(tree.insert_with(col, MetroCheckBox::new("启用蓝牙"), start()));
        self.ids.auto = Some(tree.insert_with(
            col,
            MetroCheckBox::new("开机时自动连接到上次使用的网络（这是一个故意写得很长的标签）")
                .with_checked(true),
            start(),
        ));

        // 弹性占位把按钮行推到底部（grow 吃掉剩余高度）。
        tree.insert_with(
            col,
            Stack::column(),
            LayoutProps {
                grow: 1.0,
                ..LayoutProps::default()
            },
        );

        let row = tree.insert_with(col, Stack::row().with_spacing(8.0), start());
        self.ids.cancel = Some(tree.insert(row, MetroButton::new("重置")));
        self.ids.apply = Some(tree.insert(row, MetroButton::accent("应用")));
        self.ids.status = Some(tree.insert(
            col,
            Label::new("状态：就绪").color(ThemeColor::OnSurfaceVariant),
        ));

        // 工具提示（框架行为，CONTROL_SPEC §48）：指针静止 / 键盘焦点停留满延迟即显示。
        if let Some(id) = self.ids.cancel {
            tree.set_tooltip(id, "把名称恢复为默认值");
        }
        if let Some(id) = self.ids.apply {
            tree.set_tooltip(id, "应用设置并打开确认对话框 (Enter)");
        }
    }

    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action) {
        if let Some(t) = action.downcast_ref::<TextChanged>() {
            self.name = t.0.clone();
            self.set_status(tree, format!("状态：名称改为「{}」", t.0));
        } else if action.is::<TextSubmitted>() || (action.is::<ButtonClicked>() && Some(from) == self.ids.apply) {
            self.open_dialog(tree);
        } else if action.is::<ButtonClicked>() && Some(from) == self.ids.cancel {
            if let Some(id) = self.ids.name {
                tree.edit::<MetroTextBox, _>(id, |b, ctx| {
                    b.field.set_text("ether-dev");
                    ctx.invalidate_paint();
                });
            }
            self.name = "ether-dev".into();
            self.set_status(tree, "状态：已重置".into());
        } else if action.is::<ButtonClicked>() && Some(from) == self.ids.dialog_ok {
            if let Some(d) = self.ids.dialog.take() {
                tree.close_popup(d);
            }
            if let Some(a) = self.ids.apply {
                tree.focus(a, true);
            }
            self.set_status(tree, "状态：已应用".into());
        } else if let Some(CheckToggled(s)) = action.downcast_ref::<CheckToggled>() {
            let which = if Some(from) == self.ids.bluetooth { "蓝牙" } else { "自动连接" };
            let on = *s == CheckState::Checked;
            self.set_status(tree, format!("状态：{which}{}", if on { "开启" } else { "关闭" }));
        } else if action.is::<PopupDismissed>() {
            self.ids.dialog = None;
        }
    }
}

#[cfg(target_os = "linux")]
fn main() {
    let app = Demo {
        config: AppConfig::new(
            "org.ether.kanesumi.treedemo",
            "元素树参照页",
            EtherRole::Browser,
            560.0,
            480.0,
        )
        .with_min_size(320.0, 320.0),
        theme: MetroTheme::ether_dark(),
        ids: Ids::default(),
        name: String::new(),
    };
    let mut host = kanesumi_harness::TreeHost::new(app);
    // `--snapshot <out.png> [scale]`：不开窗，经 CPU 光栅器渲染一帧到 PNG（视觉核对用）。
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::platform::find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(560.0, 480.0),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    // 非 Linux：无外壳，仅保证示例可编译。页面行为由 TreeHost 的单元测试覆盖。
    let _ = (Demo {
        config: AppConfig::new("org.ether.kanesumi.treedemo", "", EtherRole::Browser, 1.0, 1.0),
        theme: MetroTheme::ether_dark(),
        ids: Ids::default(),
        name: String::new(),
    })
    .config;
    println!("tree_demo 需要 Linux Wayland 会话。");
}
