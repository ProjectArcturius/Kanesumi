// Gallery 骨架 smoke：跨平台运行，输出设计 tokens 一览 + 首套控件渲染命令统计。
// Wayland+wgpu 外壳 Phase 3 接入（参 Ether-main PLAN.md §4.4 三层测试阶梯）。

use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Size};
use kanesumi_gallery::{command_summary, page_tree, palette, render_demo_scene};

fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    for p in [
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ] {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn main() {
    let theme = MetroTheme::ether_dark();
    println!("Kanesumi Gallery — 页树");
    for page in page_tree() {
        println!("  {}: {}", page as usize, page.title());
    }

    println!("\n设计 tokens");
    for entry in palette(&theme) {
        let (r, g, b) = (
            (entry.color.r * 255.0) as u8,
            (entry.color.g * 255.0) as u8,
            (entry.color.b * 255.0) as u8,
        );
        println!("  #{:<16} #{r:02X}{g:02X}{b:02X}", entry.name);
    }

    let Some(font_path) = find_font() else {
        println!("\n未找到测试字体（设 KANESUMI_TEST_FONT 可指定），跳过控件演示。");
        return;
    };
    let engine = TextEngine::load(font_path).expect("加载字体失败");
    let scene = render_demo_scene(&theme, &engine, Size::new(800.0, 600.0));
    let (fill, stroke, text, arc) = command_summary(&scene);
    println!(
        "\n控件演示 Scene —— FillRect: {fill}, StrokeRect: {stroke}, Text: {text}, Arc: {arc}"
    );
}
