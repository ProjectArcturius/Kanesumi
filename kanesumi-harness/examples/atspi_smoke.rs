// atspi_smoke —— 无障碍运行时冒烟：带控件与模态对话框的小界面，装上 AT-SPI 桥后存活若干秒。
//
// 为什么要有它：`kanesumi-a11y` 的映射有单测、适配器有编译验证，但「读屏真的连得上、
// 读得到」只能在 Linux 的 D-Bus 会话里跑一次才知道。本示例**不依赖合成器**（自己驱动帧），
// 所以 SSH 进无图形会话的机器也能跑：
//
//   dbus-run-session -- bash scripts/a11y_linux.sh
//
// 产出（零交互、持久路径）：
//   ~/.local/state/kanesumi/a11y-smoke.log    本进程看到的 AccessKit 树摘要
//   ~/.local/state/kanesumi/a11y-atspi.log    同一会话里从 AT-SPI 总线查到的登记项
//
// `A11Y_SMOKE_SECS` 控制存活秒数（默认 20；期间外部 AT 客户端可随时来查）。

#![allow(deprecated)]

use std::io::Write as _;

use kanesumi_controls::{DialogDefaultButton, MetroButton, MetroDialog, MetroTextBox};
use kanesumi_core::{MetroTheme, Size, ThemeColor};
use kanesumi_element::widgets::Stack;
use kanesumi_element::{
    Action, Align, Event, EventCtx, Insets, LayoutProps, Modifiers, PointerButton, Widget, WidgetId,
};
use kanesumi_harness::element::Tree;
use kanesumi_harness::element::widgets::{Border, Label};
use kanesumi_harness::{App, AppConfig, EtherRole, TreeApp};

/// 触发对话框的按钮：点它走**真实输入路径**（`show_modal*` 只在事件回调里拿得到 EventCtx）。
struct DialogOpener;

impl Widget for DialogOpener {
    fn measure(&mut self, _: &mut kanesumi_element::MeasureCtx, _: Size) -> Size {
        Size::new(120.0, 32.0)
    }
    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut kanesumi_canvas::Scene) {
        scene.fill_rect(ctx.theme().colors.control_fill, ctx.rect());
    }
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            let mut dlg = MetroDialog::new("保存更改？", "关闭前是否保存当前文件。");
            dlg.buttons.primary = Some("保存".into());
            dlg.buttons.secondary = Some("不保存".into());
            dlg.buttons.close = Some("取消".into());
            dlg.buttons.default_button = DialogDefaultButton::Primary;
            dlg.show_modal_with(ctx, |ctx, popup| {
                ctx.insert_child(popup, MetroTextBox::with_placeholder("文件名"));
            });
            ctx.set_handled();
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Button,
            name: "保存更改".to_string(),
            value: None,
            checked: None,
        })
    }
}

struct SmokeApp {
    config: AppConfig,
    theme: MetroTheme,
    opener: Option<WidgetId>,
}

impl TreeApp for SmokeApp {
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
                .padding(Insets::all(20.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(10.0));
        tree.insert(col, Label::new("无障碍冒烟").style(self.theme.typography.title));
        tree.insert_with(
            col,
            MetroTextBox::with_placeholder("名称"),
            LayoutProps { h_align: Align::Start, ..LayoutProps::default() },
        );
        tree.insert_with(
            col,
            MetroButton::accent("确定"),
            LayoutProps { h_align: Align::Start, ..LayoutProps::default() },
        );
        self.opener = Some(tree.insert(col, DialogOpener));
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn log_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::Path::new(&home).join(".local/state/kanesumi")
}

/// 把本进程看到的 AccessKit 树写成可读摘要（角色 / 名字 / 矩形 / 焦点）。
fn dump(host: &kanesumi_harness::TreeHost<SmokeApp>) -> String {
    let Some(update) = host.a11y_update() else {
        return "（还没有树）\n".to_string();
    };
    let mut out = String::new();
    out.push_str(&format!(
        "nodes={} focus={}\n",
        update.nodes.len(),
        update.focus.0
    ));
    // 按 id 排序，输出稳定，便于两次运行做 diff。
    let mut nodes: Vec<_> = update.nodes.iter().collect();
    nodes.sort_by_key(|(id, _)| id.0);
    for (id, n) in nodes {
        out.push_str(&format!(
            "{:>4} {:<16} {:?} label={:?} bounds={:?} disabled={} selected={:?} expanded={:?}\n",
            id.0,
            format!("{:?}", n.role()),
            n.role(),
            n.label().unwrap_or(""),
            n.bounds(),
            n.is_disabled(),
            n.is_selected(),
            n.is_expanded(),
        ));
    }
    out
}

fn main() {
    let secs: u64 = std::env::var("A11Y_SMOKE_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);

    let app = SmokeApp {
        config: AppConfig::new("org.ether.kanesumi.a11ysmoke", "无障碍冒烟", EtherRole::Browser, 480.0, 360.0),
        theme: MetroTheme::ether_dark(),
        opener: None,
    };
    let mut host = kanesumi_harness::TreeHost::new(app);

    #[cfg(feature = "a11y")]
    host.enable_a11y();
    #[cfg(not(feature = "a11y"))]
    {
        eprintln!("需要 --features a11y 才有无障碍桥；本次只跑界面本身。");
    }

    let engine = kanesumi_harness::element::testing::test_engine();
    let size = Size::new(480.0, 360.0);
    let mut scene = kanesumi_canvas::Scene::default();
    let frame = |host: &mut kanesumi_harness::TreeHost<SmokeApp>, scene: &mut kanesumi_canvas::Scene| {
        host.advance_clock(1.0 / 60.0);
        host.update(1.0 / 60.0);
        host.render_into(&engine, size, scene);
    };

    frame(&mut host, &mut scene);

    // 打开对话框：走真实输入路径，于是按钮由 `show_modal_with` 插成真节点。
    if let Some(opener) = host.parts_mut().0.opener {
        let (_, tree) = host.parts_mut();
        if let Some(rect) = tree.rect(opener) {
            let p = rect.center();
            tree.pointer_move(p);
            tree.pointer_down(p, PointerButton::Left, Modifiers::NONE);
            tree.pointer_up(p, PointerButton::Left, Modifiers::NONE);
        }
    }
    frame(&mut host, &mut scene);

    let summary = dump(&host);
    print!("{summary}");
    if let Err(e) = std::fs::create_dir_all(log_dir()) {
        eprintln!("建日志目录失败：{e}");
    }
    let log = log_dir().join("a11y-smoke.log");
    match std::fs::File::create(&log).and_then(|mut f| f.write_all(summary.as_bytes())) {
        Ok(()) => println!("\n摘要已写入 {}", log.display()),
        Err(e) => eprintln!("写 {} 失败：{e}", log.display()),
    }

    // 存活窗口：外部 AT 客户端（busctl / gdbus / Orca）在这段时间里查得到树。
    println!("保持存活 {secs}s（A11Y_SMOKE_SECS 可改）…");
    let started = std::time::Instant::now();
    while started.elapsed().as_secs() < secs {
        frame(&mut host, &mut scene);
        std::thread::sleep(std::time::Duration::from_millis(16));
    }
    println!("退出。");
}
