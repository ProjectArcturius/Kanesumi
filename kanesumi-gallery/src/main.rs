// Gallery 入口。
//
// Linux：加载字体 → GalleryApp → harness Wayland+wgpu 外壳（daily driver，参 PLAN.md §4.4）。
// 非 Linux：纯逻辑 smoke（页树 + tokens + 控件 Scene 统计），保持跨平台可测。

/// 非 Linux 平台的字体查找（Windows / macOS smoke test 用）。
/// Linux 侧一律走 `kanesumi_harness::platform::find_font`，避免
/// Gallery 预加载字体与外壳实际加载字体不同源（V18 直接根因：
/// Gallery 用 DejaVu 量按钮宽度、外壳用 SourceHan 光栅化 → CJK 溢出 box）。
#[cfg(not(target_os = "linux"))]
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
    // V18：Gallery 预加载字体必须与 harness 外壳同源。旧代码 main.rs 自己
    // find_font 只查 DejaVu，harness 优先查 SourceHan —— 两处不一致时按钮
    // 用 DejaVu 量宽（CJK 走 .notdef 极窄）、harness 用 SourceHan 光栅化（CJK 正常宽），
    // 结果 "打开对话框" 溢出 box。此处直接委派 harness 的查找顺序。
    let font_path = kanesumi_harness::platform::find_font()
        .expect("未找到字体：设 KANESUMI_TEST_FONT");
    let app = kanesumi_gallery::GalleryApp::new(font_path);
    // Box::leak → &'static mut GalleryApp → &mut dyn App（run 永不返回，生命周期合法）。
    kanesumi_harness::platform::run(Box::leak(Box::new(app)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    use kanesumi_canvas::text::TextEngine;
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
    let (fill, stroke, text, arc, image) = command_summary(&scene);
    println!(
        "\n控件演示 Scene —— FillRect: {fill}, StrokeRect: {stroke}, Text: {text}, Arc: {arc}, Image: {image}"
    );
}
