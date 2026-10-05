// acrylic_preview —— 亚克力背板预览：壁纸 PNG → 指定模糊 → 叠一段文字 → 写 PNG（跨平台，无窗）。
//   cargo run -p kanesumi-gallery --example acrylic_preview -- <wallpaper.png> <out.png> <blur> [light]

use std::path::Path;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Acrylic, Scene, TextAlign, backdrop, rasterize_image};
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Rect, Size};

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (src, out, blur) = (&a[0], &a[1], a[2].parse::<f32>().unwrap_or(0.0));
    let scheme = if a.get(3).map(String::as_str) == Some("light") { ColorScheme::Light } else { ColorScheme::Dark };
    let theme = MetroTheme::for_scheme(scheme, Accent::default());
    // 壁纸多为 JPEG / SVG：按扩展名分派（rasterize_image）；预览全尺寸即可。
    let wall = rasterize_image(src, None).expect("读壁纸失败");
    let (w, h) = (1280.0, 800.0);
    let t0 = std::time::Instant::now();
    let bg = backdrop(&wall, w, h, &Acrylic::for_theme(&theme, blur, Acrylic::default_tint(scheme)));
    let ms = t0.elapsed().as_secs_f32() * 1000.0;
    let mut scene = Scene::default();
    scene.image(&bg, Rect::new(0.0, 0.0, w, h), None);
    let style = theme.typography.title;
    scene.text(format!("亚克力 blur={blur}（{ms:.0} ms）"), Rect::new(48.0, 48.0, 900.0, 60.0), theme.colors.on_background, style, TextAlign::Left);
    let font = std::env::var("KANESUMI_TEST_FONT").unwrap_or_else(|_| "C:/Windows/Fonts/msyh.ttc".into());
    let engine = TextEngine::load(&font).expect("字体");
    let mut cpu = kanesumi_harness::CpuRenderer::new(w, h, 1.0);
    let (pw, ph) = cpu.physical_size();
    let rgba = cpu.render(&engine, &scene, None).to_vec();
    let pix = resvg::tiny_skia::Pixmap::from_vec(rgba, resvg::tiny_skia::IntSize::from_wh(pw, ph).unwrap()).unwrap();
    pix.save_png(Path::new(out)).unwrap();
    println!("{out} 背板 {}x{} 用时 {ms:.1} ms", bg.width, bg.height);
    let _ = Size::ZERO;
}
