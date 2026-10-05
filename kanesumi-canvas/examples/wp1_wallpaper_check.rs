// wp1_wallpaper_check —— 统一解码入口实测（wp1-kanesumi-image-decode 验收用）。
// 用真实壁纸 JPEG / SVG 各解一次：打印全尺寸与采样像素，并存 1280×800 结果 PNG 供读图核对。
//   cargo run -p kanesumi-canvas --example wp1_wallpaper_check -- <壁纸路径> <out.png>
// 产物尺寸语义（参 WALLPAPER_DESIGN.md §Ⅲ 消费端一律 Cover）：
// - 栅格图（JPEG/PNG）：target 传缩略图逻辑尺寸的一半 (640,400)，入口降采样上限 2× → 1280×800；
// - SVG：target 即输出物理像素，直接传 (1280,800)，Cover 光栅一次。

use std::path::Path;

use kanesumi_canvas::rasterize_image;

/// 打印图标的尺寸与左上 / 中心像素（hex RGBA，直通语义）。
fn probe(label: &str, icon: &kanesumi_canvas::Icon) {
    let px = |x: u32, y: u32| {
        let i = ((y * icon.width + x) * 4) as usize;
        format!("#{:02X}{:02X}{:02X}{:02X}", icon.rgba[i], icon.rgba[i + 1], icon.rgba[i + 2], icon.rgba[i + 3])
    };
    println!(
        "{label}: {}×{} 左上={} 中心={}",
        icon.width,
        icon.height,
        px(0, 0),
        px(icon.width / 2, icon.height / 2)
    );
}

/// 直通 RGBA → 预乘（tiny-skia Pixmap 要求）→ 存 PNG。
fn save_png(icon: &kanesumi_canvas::Icon, out: &str) {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(icon.width, icon.height).unwrap();
    for (dst, c) in pixmap.pixels_mut().iter_mut().zip(icon.rgba.chunks_exact(4)) {
        let (r, g, b, a) = (c[0], c[1], c[2], c[3]);
        let af = a as u16;
        let premul = |v: u8| ((v as u16 * af + 127) / 255) as u8;
        *dst = resvg::tiny_skia::PremultipliedColorU8::from_rgba(premul(r), premul(g), premul(b), a)
            .unwrap();
    }
    pixmap.save_png(out).expect("存 PNG 失败");
    println!("已存 {out}（{}×{}）", icon.width, icon.height);
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (src, out) = (a[0].clone(), a[1].clone());
    let path = Path::new(&src);
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let is_svg = ext == "svg";

    // 全尺寸解码（栅格图不降采样 / SVG 用 viewBox 原尺寸）。
    let full = rasterize_image(path, None).expect("全尺寸解码失败");
    probe(&format!("{src}（全尺寸）"), &full);

    // 1280×800 交付档：栅格图走 2× 上限降采样，SVG 按物理像素 Cover 光栅。
    let thumb_target = if is_svg { (1280, 800) } else { (640, 400) };
    let thumb = rasterize_image(path, Some(thumb_target)).expect("缩略档解码失败");
    probe(&format!("{src}（1280×800 档）"), &thumb);
    save_png(&thumb, &out);
}
