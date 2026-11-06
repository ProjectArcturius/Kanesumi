// dialog_content_snapshot —— 对话框内容槽（`Dialog.Content` 对等物）的视觉证据。
//
// 为什么单独一张：对话框以前只能吃一段纯文本，「重命名」这类「标题 + 输入框 + 提示」的对话框
// 只能由应用自造 —— 而自造的那一份不会跟随对话框的 Padding / 按钮布局 / 模态行为。
// 本图证明内容槽可用：对话框里放一个真实输入框与一行提示，整页仍然零手写坐标。
//
//   cargo run -p kanesumi-gallery --example dialog_content_snapshot -- <输出.png> [scale]

#![allow(deprecated)]

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_controls::{DialogDefaultButton, MetroDialog, MetroTextBox};
use kanesumi_core::{Size, ThemeColor};
use kanesumi_harness::cpu_raster::CpuRenderer;
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{
    Action, AccessInfo, AccessRole, Align, Event, EventCtx, Insets, LayoutProps, MeasureCtx,
    PaintCtx, Tree, Widget, WidgetId,
};
use kanesumi_harness::{
    App, AppConfig, EtherRole, InputEvent, Modifiers, PointerButton, TreeApp, TreeHost,
};

/// 触发器：点击 → 打开「重命名」对话框（内容由应用随后挂上）。
struct RenameOpener;

impl Widget for RenameOpener {
    fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
        Size::new(120.0, 32.0)
    }
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let fill = ctx.theme().colors.control_fill;
        scene.fill_rect(fill, ctx.rect());
    }
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            let mut dlg = MetroDialog::new("重命名", "");
            dlg.buttons.primary = Some("确定".into());
            dlg.buttons.close = Some("取消".into());
            dlg.buttons.default_button = DialogDefaultButton::Primary;
            // 走 `show_modal`：它会打开弹层、把焦点移进去，并把「当前按钮」预置成默认按钮
            // （因此截图里能看到焦点环）。
            dlg.show_modal(ctx);
            ctx.set_handled();
        }
    }
    fn focusable(&self) -> bool {
        true
    }
    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::Button,
            name: "重命名".into(),
            value: None,
            checked: None,
        })
    }
}

struct Page {
    config: AppConfig,
}

impl TreeApp for Page {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(16.0)),
        );
        tree.insert(page, RenameOpener);
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn engine() -> TextEngine {
    kanesumi_harness::element::testing::test_engine()
}

fn frame(host: &mut TreeHost<Page>, engine: &TextEngine, size: Size) -> Scene {
    host.advance_clock(1.0 / 60.0);
    host.update(1.0 / 60.0);
    let mut s = Scene::default();
    host.render_into(engine, size, &mut s);
    s
}

fn save(engine: &TextEngine, scene: &Scene, size: Size, scale: f32, out: &str) {
    let mut cpu = CpuRenderer::new(size.width, size.height, scale);
    let rgba = cpu.render(engine, scene, None).to_vec();
    let (pw, ph) = cpu.physical_size();
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        rgba,
        resvg::tiny_skia::IntSize::from_wh(pw, ph).expect("尺寸合法"),
    )
    .expect("像素缓冲尺寸不符");
    pixmap
        .save_png(std::path::Path::new(out))
        .expect("写 PNG 失败");
    println!("snapshot {out} {pw}x{ph}");
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args
        .next()
        .unwrap_or_else(|| "dialog_content.png".to_string());
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2.0);

    let engine = engine();
    let size = Size::new(560.0, 420.0);
    let mut host = TreeHost::new(Page {
        config: AppConfig::new(
            "org.ether.gallery.dialogcontent",
            "对话框内容槽",
            EtherRole::Browser,
            size.width,
            size.height,
        ),
    });

    frame(&mut host, &engine, size);

    // 点触发器（左上 16pt 内边距内，中心约 (76, 32)）→ 对话框打开。
    for ev in [
        InputEvent::PointerPressed {
            x: 76.0,
            y: 32.0,
            button: PointerButton::Left,
            modifiers: Modifiers::NONE,
        },
        InputEvent::PointerReleased {
            x: 76.0,
            y: 32.0,
            button: PointerButton::Left,
            modifiers: Modifiers::NONE,
        },
    ] {
        host.handle_input(ev);
    }
    frame(&mut host, &engine, size);

    // 内容槽：把「输入框 + 提示」挂到对话框节点下（弹层打开后由应用挂内容）。
    let popup = host.tree().popups().next().expect("对话框弹层已打开");
    {
        let (_, tree) = host.parts_mut();
        let col = tree.insert(popup, Stack::column().with_spacing(8.0));
        tree.insert(col, MetroTextBox::with_placeholder("新名称"));
        tree.insert_with(
            col,
            Label::new("扩展名保持不变").color(ThemeColor::OnSurfaceVariant),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
    }

    let scene = frame(&mut host, &engine, size);
    save(&engine, &scene, size, scale, &out);
}
