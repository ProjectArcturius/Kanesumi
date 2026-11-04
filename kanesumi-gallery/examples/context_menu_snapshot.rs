// context_menu_snapshot —— E4 菜单族入树的无头快照（n1104-e4 亲看证据）。
//
// 三个场景（<输出目录> 下各一张 PNG，CPU 光栅、真实元素树，不开任何窗口）：
//   context-menu-window.png   400×300 窗口：右键菜单点锚定弹出（无遮罩、翻折）；
//   context-menu-submenu.png  同窗口：悬停嵌套项展开级联子菜单；
//   context-menu-dock.png     1280×800 输出上的 1280×76 Dock 条：外壳放大放置区
//                             （`enable_popups`，LayerBottom 同款）后菜单以树弹层
//                             承载 —— 主表面（条）+ 弹层表面两张 Scene 合成一张图，
//                             菜单完整落在条上方，不再被 76px 条面裁剪（修前塌缩
//                             对照见 out/n1104-e4-containers/before-*.png）。
//
// 场景 3 同时走通 harness 右键路由的新契约：`TreeApp::context_menu` 提供内容 →
// `App::open_context_popup`（TreeHost 实现）开 `MetroContextMenu` 树弹层。
//
// 用法：
//   cargo run -p kanesumi-gallery --example context_menu_snapshot -- <输出目录>

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_controls::{MenuInvoked, MenuItem};
use kanesumi_core::{MetroTheme, Point, Rect, Size, ThemeColor};
use kanesumi_harness::cpu_raster::CpuRenderer;
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, Widget, WidgetId};
use kanesumi_harness::{App, AppConfig, EtherRole, InputEvent, Modifiers, PointerButton, TreeApp, TreeHost};

/// 窗口场景：一块可右键的面板，右键释放弹菜单（树内事件，非分离模式可直接快照）。
struct WindowApp {
    config: AppConfig,
    target: Option<WidgetId>,
    invoked: Vec<String>,
}

impl TreeApp for WindowApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn build(&mut self, tree: &mut Tree) {
        let bg = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));
        let col = tree.insert(bg, Stack::column().with_spacing(8.0));
        self.target = Some(tree.insert_with(
            col,
            ContextTarget {
                items: window_items(),
                opened: 0,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(60.0, 40.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        ));
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Ok(inv) = action.downcast::<MenuInvoked>() {
            self.invoked.push(inv.label.clone());
        }
    }
}

/// Dock 场景：实现 harness 右键钩子（`TreeApp::context_menu`）—— 外壳在右键释放时
/// 经 `App::context_menu` 取内容，再经 `App::open_context_popup` 开树弹层。
struct DockApp {
    config: AppConfig,
    invoked: Vec<String>,
}

impl TreeApp for DockApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn build(&mut self, tree: &mut Tree) {
        let bg = tree.insert(tree.root(), Border::new().background(ThemeColor::Surface));
        tree.insert(bg, Stack::row().with_spacing(12.0));
    }
    fn context_menu(&mut self, _tree: &mut Tree, _x: f32, _y: f32) -> Option<Vec<MenuItem>> {
        Some(dock_items())
    }
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Ok(inv) = action.downcast::<MenuInvoked>() {
            self.invoked.push(inv.label.clone());
        }
    }
}

/// 右键目标：正典 §Ⅱ/§Ⅵ —— 框架在右键**释放**时投 `ContextRequested`，此处开菜单。
struct ContextTarget {
    items: Vec<MenuItem>,
    opened: u32,
}

impl Widget for ContextTarget {
    fn measure(
        &mut self,
        _: &mut kanesumi_harness::element::MeasureCtx,
        _: Size,
    ) -> Size {
        Size::new(160.0, 40.0)
    }
    fn paint(&mut self, _: &mut kanesumi_harness::element::PaintCtx, _: &mut Scene) {}
    fn event(&mut self, ctx: &mut kanesumi_harness::element::EventCtx, event: &kanesumi_harness::element::Event) {
        use kanesumi_harness::element::Event;
        if let Event::ContextRequested { pos } = event {
            kanesumi_controls::MetroContextMenu::open(ctx, self.items.clone(), *pos, false);
            ctx.set_handled();
            self.opened += 1;
        }
    }
}

fn window_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("打开"),
        MenuItem::new("排序").with_submenu(vec![
            MenuItem::new("名称").radio("sort"),
            MenuItem::new("日期").radio("sort"),
        ]),
        MenuItem::new("删除"),
    ]
}

/// Dock 五项（与 launcher i18n dock 菜单同构）。
fn dock_items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("新建窗口"),
        MenuItem::new("固定"),
        MenuItem::new("关闭"),
        MenuItem::new("设置").with_submenu(vec![MenuItem::new("自动隐藏"), MenuItem::new("居中")]),
        MenuItem::new("从 Dock 移除"),
    ]
}

fn engine() -> TextEngine {
    kanesumi_harness::element::testing::test_engine()
}

/// 一帧：推进时钟 + update + render（与 snapshot::render_png 同构，单帧）。
fn frame(host: &mut TreeHost<impl TreeApp>, engine: &TextEngine, size: Size) -> Scene {
    host.advance_clock(1.0 / 60.0);
    host.update(1.0 / 60.0);
    let mut s = Scene::default();
    host.render_into(engine, size, &mut s);
    s
}

fn save_png(path: &std::path::Path, rgba: &[u8], w: u32, h: u32) {
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        rgba.to_vec(),
        resvg::tiny_skia::IntSize::from_wh(w, h).expect("尺寸合法"),
    )
    .expect("像素缓冲尺寸不符");
    pixmap.save_png(path).expect("写 PNG 失败");
    println!("snapshot {} {w}x{h}", path.display());
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .expect("用法：context_menu_snapshot <输出目录>");
    let out_dir = std::path::PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir).expect("创建输出目录");
    let engine = engine();

    // ── 场景 1 / 2：窗口右键菜单（树内弹层直接进主 Scene）────────────────────
    let mut host = TreeHost::new(WindowApp {
        config: AppConfig::new("org.ether.gallery.snapshot", "右键菜单快照", EtherRole::Browser, 400.0, 300.0),
        target: None,
        invoked: Vec::new(),
    });
    let size = Size::new(400.0, 300.0);
    frame(&mut host, &engine, size);
    let target = host.app().target.expect("已建目标");
    // 右键按下 + 释放（正典：释放才弹）。
    host.handle_input(InputEvent::PointerPressed {
        x: 140.0,
        y: 60.0,
        button: PointerButton::Right,
        modifiers: Modifiers::NONE,
    });
    host.handle_input(InputEvent::PointerReleased {
        x: 140.0,
        y: 60.0,
        button: PointerButton::Right,
        modifiers: Modifiers::NONE,
    });
    let scene = frame(&mut host, &engine, size);
    let mut cpu = CpuRenderer::new(size.width, size.height, 1.0);
    let rgba = cpu.render(&engine, &scene, None).to_vec();
    let (w, h) = cpu.physical_size();
    save_png(&out_dir.join("context-menu-window.png"), &rgba, w, h);

    // 场景 2：悬停「排序」（第 2 项：面板原点 (140,60)，项高 32 → y = 60+32+16）
    // 展开级联子菜单（带子菜单的亲看证据）。
    host.handle_input(InputEvent::PointerMoved { x: 160.0, y: 108.0 });
    let scene = frame(&mut host, &engine, size);
    let rgba = cpu.render(&engine, &scene, None).to_vec();
    save_png(&out_dir.join("context-menu-submenu.png"), &rgba, w, h);
    let _ = target;

    // ── 场景 3：Dock 条（1280×76）+ 放大放置区 → 树弹层承载，双表面合成 ──────
    let mut host = TreeHost::new(DockApp {
        config: AppConfig::new("ether-launcher-dock", "Dock", EtherRole::Dock, 1280.0, 76.0),
        invoked: Vec::new(),
    });
    // 外壳 `sync_popups` 同款：LayerBottom 的放置区 = 整个输出（条上方为主）。
    assert!(host.enable_popups(Rect::new(0.0, -(800.0 - 76.0), 1280.0, 800.0)));
    let strip = Size::new(1280.0, 76.0);
    frame(&mut host, &engine, strip);
    // 外壳路由（`Shell::emit_input` 释放分支）：右键按下照常投树（记命中目标），
    // 释放取内容 → `open_context_popup` 开树弹层。
    host.handle_input(InputEvent::PointerPressed {
        x: 640.0,
        y: 40.0,
        button: PointerButton::Right,
        modifiers: Modifiers::NONE,
    });
    let items = host
        .context_menu(640.0, 40.0)
        .expect("App 提供右键菜单内容");
    assert!(host.open_context_popup(items, 640.0, 40.0), "树弹层承载");
    frame(&mut host, &engine, strip);
    let popup = host.tree().popups().next().expect("菜单弹层应存在");
    let extent = host.tree().popup_extent(popup).expect("弹层有范围");

    // 主表面（条）+ 弹层表面 → 合成一张 1280×800 输出图（与合成器同构：条贴底、
    // 弹层表面按 `popup_extent` 相对条原点摆放）。
    let scene = frame(&mut host, &engine, strip);
    let mut cpu_strip = CpuRenderer::new(strip.width, strip.height, 1.0);
    let strip_rgba = cpu_strip.render(&engine, &scene, None).to_vec();
    let popup_scene = host.tree().popup_scene(popup).expect("弹层 Scene");
    let mut cpu_popup = CpuRenderer::new(extent.size.width, extent.size.height, 1.0);
    let popup_rgba = cpu_popup.render(&engine, &popup_scene, None).to_vec();
    let (pw, ph) = cpu_popup.physical_size();

    let (ow, oh) = (1280u32, 800u32);
    let mut out = vec![0u8; (ow * oh * 4) as usize];
    // 背板：暗色桌面（与深色主题对比可辨）。
    for y in 0..oh {
        for x in 0..ow {
            let i = ((y * ow + x) * 4) as usize;
            out[i] = 24;
            out[i + 1] = 26;
            out[i + 2] = 30;
            out[i + 3] = 255;
        }
    }
    // 条贴底（Dock 全局位置 y = 800 − 76）。
    let dock_y = oh - 76;
    for y in 0..76u32 {
        for x in 0..1280u32 {
            let src = ((y * 1280 + x) * 4) as usize;
            let dst = (((dock_y + y) * ow + x) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&strip_rgba[src..src + 4]);
        }
    }
    // 弹层表面：dock 本地 (x, y) → 全局 (x, dock_y + y)。弹层在放置区（条上方），
    // y 为负 → 全局 y < dock_y，即菜单完整落在条上方（不被条面裁剪）。
    let px = extent.origin.x as i32;
    let py = dock_y as i32 + extent.origin.y as i32;
    assert!(py >= 0 && py + (ph as i32) <= oh as i32, "弹层完整落在输出内：py={py} ph={ph}");
    for y in 0..ph {
        for x in 0..pw {
            let src = ((y * pw + x) * 4) as usize;
            let gx = (px + x as i32) as u32;
            let gy = (py + y as i32) as u32;
            let dst = ((gy * ow + gx) * 4) as usize;
            out[dst..dst + 4].copy_from_slice(&popup_rgba[src..src + 4]);
        }
    }
    save_png(&out_dir.join("context-menu-dock.png"), &out, ow, oh);
    println!("选中动作（Dock）：{:?}", host.app().invoked);
    let _ = MetroTheme::ether_dark();
}
