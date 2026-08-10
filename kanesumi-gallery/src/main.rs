// Gallery 入口。
//
// Linux：加载字体 → GalleryApp → harness Wayland+wgpu 外壳（daily driver，参 PLAN.md §4.4）。
// 非 Linux：纯逻辑 smoke（页树 + tokens + 控件 Scene 统计），保持跨平台可测。

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

#[cfg(target_os = "linux")]
fn main() {
    let font_path = find_font()
        .or_else(kanesumi_harness::platform::find_font)
        .expect("未找到字体：设 KANESUMI_TEST_FONT");
    let app = kanesumi_gallery::GalleryApp::new(font_path);
    // Box::leak → &'static mut GalleryApp → &mut dyn App（run 永不返回，生命周期合法）。
    kanesumi_harness::platform::run(Box::leak(Box::new(app)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    use kanesumi_core::text::TextEngine;
    use kanesumi_core::{MetroTheme, Size};
    use kanesumi_gallery::{command_summary, page_tree, palette, render_demo_scene};

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
