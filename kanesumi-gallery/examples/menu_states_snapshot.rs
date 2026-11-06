// menu_states_snapshot —— 菜单状态视觉证据（禁用项 / 勾选列 / 级联 / 快捷键）。
//
// 为什么单独一张：菜单的「手感」里有一半是看出来的 —— 勾选有没有把文字顶歪、
// 禁用项是不是整行暗下去、快捷键与 chevron 有没有互相压。这些都是像素问题，
// 靠断言只能测到「画了」，测不到「看起来对」。
//
// 用法：
//   cargo run -p kanesumi-gallery --example menu_states_snapshot -- <输出.png> [scale]

// 快照必须走外壳契约（`App::handle_input` / `App::update`），与 `context_menu_snapshot` 同款豁免。
#![allow(deprecated)]

use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_controls::{MenuItem, MetroContextMenu};
use kanesumi_core::{Size, ThemeColor};
use kanesumi_harness::cpu_raster::CpuRenderer;
use kanesumi_harness::element::widgets::Border;
use kanesumi_harness::element::{
    Action, Event, EventCtx, MeasureCtx, PaintCtx, Tree, Widget, WidgetId,
};
use kanesumi_harness::{
    App, AppConfig, EtherRole, InputEvent, Modifiers, PointerButton, TreeApp, TreeHost,
};

/// 可右键的空白面板（右键释放 → 树弹层承载的右键菜单）。
struct Window {
    config: AppConfig,
    items: Vec<MenuItem>,
}

impl TreeApp for Window {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn build(&mut self, tree: &mut Tree) {
        tree.insert(
            tree.root(),
            Border::new().background(ThemeColor::Background),
        );
        tree.insert(
            tree.root(),
            RightClickTarget {
                items: self.items.clone(),
            },
        );
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

struct RightClickTarget {
    items: Vec<MenuItem>,
}

impl Widget for RightClickTarget {
    fn measure(&mut self, _: &mut MeasureCtx, available: Size) -> Size {
        available
    }
    fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::ContextRequested { pos } = event {
            MetroContextMenu::open(ctx, self.items.clone(), *pos, false);
            ctx.set_handled();
        }
    }
}

/// 一张典型的编辑器右键菜单：可用项 + 禁用项 + 单选勾选 + 级联 + 快捷键。
fn items() -> Vec<MenuItem> {
    // 「名称」为当前排序方式：勾选列要能看见勾，且勾了也不把文字顶歪。
    let mut sort_by_name = MenuItem::new("名称").radio("sort");
    sort_by_name.checked = true;
    vec![
        MenuItem::new("复制").with_shortcut("Ctrl+C"),
        MenuItem::new("剪切").with_shortcut("Ctrl+X"),
        MenuItem::new("粘贴").with_shortcut("Ctrl+V").disabled(),
        MenuItem::new("排序").with_submenu(vec![
            sort_by_name,
            MenuItem::new("修改时间").radio("sort"),
            MenuItem::new("大小").radio("sort").disabled(),
        ]),
        MenuItem::new("全选").with_shortcut("Ctrl+A").disabled(),
        MenuItem::new("属性"),
    ]
}

/// 40 条命令：面板必须封顶在 MENU_MAX_HEIGHT 内，而不是长到屏幕外。
fn long_items() -> Vec<MenuItem> {
    (0..40)
        .map(|i| {
            let item = MenuItem::new(format!("命令 {i:02}"));
            if i % 5 == 0 { item.disabled() } else { item }
        })
        .collect()
}

fn save(engine: &TextEngine, scene: &Scene, size: Size, scale: f32, out: &str) {
    let mut cpu = CpuRenderer::new(size.width, size.height, scale);
    let rgba = cpu.render(engine, scene, None).to_vec();
    let (w, h) = cpu.physical_size();
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        rgba,
        resvg::tiny_skia::IntSize::from_wh(w, h).expect("尺寸合法"),
    )
    .expect("像素缓冲尺寸不符");
    pixmap
        .save_png(std::path::Path::new(out))
        .expect("写 PNG 失败");
    println!("snapshot {out} {w}x{h}");
}
fn engine() -> TextEngine {
    kanesumi_harness::element::testing::test_engine()
}

fn frame(host: &mut TreeHost<impl TreeApp>, engine: &TextEngine, size: Size) -> Scene {
    host.advance_clock(1.0 / 60.0);
    host.update(1.0 / 60.0);
    let mut s = Scene::default();
    host.render_into(engine, size, &mut s);
    s
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "menu_states.png".to_string());
    let scale: f32 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2.0);

    let engine = engine();
    let size = Size::new(420.0, 320.0);
    let mut host = TreeHost::new(Window {
        config: AppConfig::new(
            "org.ether.gallery.menustates",
            "菜单状态",
            EtherRole::Browser,
            size.width,
            size.height,
        ),
        items: items(),
    });
    frame(&mut host, &engine, size);

    // 右键（释放才弹，正典）→ 菜单左上角落在 (120, 40)。
    for ev in [
        InputEvent::PointerPressed {
            x: 120.0,
            y: 40.0,
            button: PointerButton::Right,
            modifiers: Modifiers::NONE,
        },
        InputEvent::PointerReleased {
            x: 120.0,
            y: 40.0,
            button: PointerButton::Right,
            modifiers: Modifiers::NONE,
        },
    ] {
        host.handle_input(ev);
    }
    frame(&mut host, &engine, size);

    // 悬停「排序」（第 4 项，项高 32）→ 展开级联子菜单，子菜单里「名称」勾选。
    host.handle_input(InputEvent::PointerMoved {
        x: 140.0,
        y: 40.0 + 32.0 * 3.0 + 16.0,
    });
    let scene = frame(&mut host, &engine, size);
    save(&engine, &scene, size, scale, &out);

    // ── 第二张：长菜单（40 项）—— 面板必须封顶、不许长出屏幕 ─────────────────
    let long_size = Size::new(420.0, 620.0);
    let mut long = TreeHost::new(Window {
        config: AppConfig::new(
            "org.ether.gallery.menustates",
            "长菜单",
            EtherRole::Browser,
            long_size.width,
            long_size.height,
        ),
        items: long_items(),
    });
    frame(&mut long, &engine, long_size);
    for ev in [
        InputEvent::PointerPressed {
            x: 60.0,
            y: 30.0,
            button: PointerButton::Right,
            modifiers: Modifiers::NONE,
        },
        InputEvent::PointerReleased {
            x: 60.0,
            y: 30.0,
            button: PointerButton::Right,
            modifiers: Modifiers::NONE,
        },
    ] {
        long.handle_input(ev);
    }
    let long_scene = frame(&mut long, &engine, long_size);
    let long_out = {
        let p = std::path::Path::new(&out);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("menu_states");
        p.with_file_name(format!("{stem}-long.png"))
    };
    save(
        &engine,
        &long_scene,
        long_size,
        scale,
        &long_out.to_string_lossy(),
    );
}
