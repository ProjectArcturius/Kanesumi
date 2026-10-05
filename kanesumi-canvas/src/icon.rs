// 图标管线 —— PNG / JPEG / SVG → RGBA 统一解码（对应 ASSETS.md / ether-assets 同款管线，Kanesumi 自足）。
//
// 输出直通 RGBA（非预乘），供 `Scene::image` 上传纹理。Metro 风格：纯色图标可用 tint 染色。
// 壁纸接线（参 Ether WALLPAPER_DESIGN.md §Ⅲ/§Ⅴ）：光栅三张是 JPEG（3840×2400）、矢量三张
// 是 SVG，统一走 `rasterize_image`：给了 `target` 时栅格大图先盒式降采样到 ≤ 2× 目标，
// SVG 按 target 物理像素以 Cover 方式光栅一次——4K 源图不再占满缩略图内存。

use std::path::Path;
use std::sync::Arc;

/// 统一解码入口的格式种类（按扩展名识别，参 `kind_from_extension`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    /// PNG（tiny-skia 解码，带透明通道）。
    Png,
    /// JPEG（zune-jpeg 解码，输出不透明）。
    Jpeg,
    /// SVG（resvg 光栅；`target` 为输出物理像素，Cover 适配）。
    Svg,
}

/// 按文件扩展名识别格式（不区分大小写）：`png` / `jpg` / `jpeg` / `svg`。
/// 未知或无扩展名返回 None —— 统一入口不做魔数猜测（SVG 是文本，本就无魔数可判）。
fn kind_from_extension(path: &Path) -> Option<ImageKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "png" => Some(ImageKind::Png),
        "jpg" | "jpeg" => Some(ImageKind::Jpeg),
        "svg" => Some(ImageKind::Svg),
        _ => None,
    }
}

/// 已光栅化的图标（直通 RGBA 像素）。
/// `rgba` 用 `Arc<[u8]>` 共享：`Scene::image` 每帧 clone 只 bump 引用计数，
/// 不深拷贝整张位图（参 egui texture atlas 引用共享思路）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Icon {
    pub rgba: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
}

impl Icon {
    pub fn size(&self) -> kanesumi_core::Size {
        kanesumi_core::Size::new(self.width as f32, self.height as f32)
    }

    /// 从 SVG 文件加载图标（等同 `rasterize_svg` 的便捷方法）。
    pub fn load_svg(path: impl AsRef<Path>, target_size: u32) -> Option<Self> {
        rasterize_svg(path, target_size)
    }

    /// 裁剪为圆形（用户头像用）：圆外像素 alpha 置 0，正圆。
    pub fn circle_crop(mut self) -> Self {
        let (w, h) = (self.width as f32, self.height as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let r = (w.min(h) / 2.0).max(1.0);
        // 构造期唯一持有 → make_mut 就地改（不触发整表拷贝）。
        let rgba = Arc::make_mut(&mut self.rgba);
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy > r * r {
                    let i = ((y * self.width + x) * 4) as usize;
                    rgba[i + 3] = 0;
                }
            }
        }
        self
    }
}

/// tiny-skia 输出 premultiplied RGBA → 直通 RGBA（与 ether-assets 同款去预乘）。
/// PNG 与 SVG 光栅共用；alpha=0 的像素 RGB 置 0（预乘语义下 RGB 本就无意义）。
fn unpremultiply(raw: &[u8]) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(raw.len());
    for chunk in raw.chunks_exact(4) {
        let (r, g, b, a) = (chunk[0], chunk[1], chunk[2], chunk[3]);
        let (r, g, b) = if a == 0 {
            (0u8, 0u8, 0u8)
        } else {
            let af = a as f32 / 255.0;
            (
                (r as f32 / af).round().min(255.0) as u8,
                (g as f32 / af).round().min(255.0) as u8,
                (b as f32 / af).round().min(255.0) as u8,
            )
        };
        rgba.extend_from_slice(&[r, g, b, a]);
    }
    rgba
}

/// 把 PNG 文件解码为直通 RGBA 图标（用户头像 `~/.face` 等）。失败返回 None。
/// 保留原签名：内部走 `rasterize_image` 统一入口的 PNG 分支。
pub fn rasterize_png(path: impl AsRef<Path>) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    rasterize_png_bytes(&data)
}

/// 把 PNG 字节解码为直通 RGBA 图标。参 rasterize_png。
pub fn rasterize_png_bytes(data: &[u8]) -> Option<Icon> {
    decode_png(data, None)
}

/// 统一入口 PNG 分支：tiny-skia 解 PNG，给了 `target` 且源图大于 2× 目标时盒式降采样。
fn decode_png(data: &[u8], target: Option<(u32, u32)>) -> Option<Icon> {
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(data).ok()?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let icon = Icon {
        rgba: Arc::from(unpremultiply(pixmap.data())),
        width: w,
        height: h,
    };
    downsample_to_target(icon, target)
}

/// 统一图像解码入口：按**扩展名**（不区分大小写）分派 `png` / `jpg` / `jpeg` / `svg`。
/// 用于壁纸等外部图片；失败 / 未知扩展名返回 None，不 panic。
///
/// `target` 为输出用途的物理像素尺寸（如缩略图 (128, 80)）：
/// - 栅格图（PNG / JPEG）：源图任一边超过 2× 目标时，先盒式降采样到 ≤ 2× 目标再交出；
/// - SVG：直接按 target 尺寸以 Cover 方式光栅（无 target 时用 viewBox 原尺寸）。
pub fn rasterize_image(path: impl AsRef<Path>, target: Option<(u32, u32)>) -> Option<Icon> {
    let path = path.as_ref();
    let kind = kind_from_extension(path)?;
    let data = std::fs::read(path).ok()?;
    rasterize_image_kind(&data, kind, target)
}

/// 统一图像解码入口的字节版：格式由调用方以 `ImageKind` 显式给定。参 rasterize_image。
pub fn rasterize_image_bytes(data: &[u8], kind: ImageKind) -> Option<Icon> {
    rasterize_image_kind(data, kind, None)
}

/// 三格式分派核心（无 target 时 SVG 用原尺寸，栅格图不降采样）。
fn rasterize_image_kind(data: &[u8], kind: ImageKind, target: Option<(u32, u32)>) -> Option<Icon> {
    match kind {
        ImageKind::Png => decode_png(data, target),
        ImageKind::Jpeg => decode_jpeg(data, target),
        ImageKind::Svg => render_svg(data, target),
    }
}

/// 统一入口 JPEG 分支：解码为不透明 RGBA（alpha 255）。zune-jpeg 默认输出 RGB（某些灰度图
/// 输出单通道 Luma），按输出长度判定通道数；失败返回 None。
/// 与 PNG 路径同约定：直通 RGBA（JPEG 无透明，直通与预乘在此等价）。
fn decode_jpeg(data: &[u8], target: Option<(u32, u32)>) -> Option<Icon> {
    let mut decoder = zune_jpeg::JpegDecoder::new(data);
    let pixels = decoder.decode().ok()?;
    let info = decoder.info()?;
    let (w, h) = (info.width as u32, info.height as u32);
    if w == 0 || h == 0 {
        return None;
    }
    let n = (w as usize) * (h as usize);
    let mut rgba = Vec::with_capacity(n * 4);
    if pixels.len() >= n * 4 {
        // 已是 RGBA：仅需保证 alpha（zune 默认不会走到这里，防御性保留）。
        for c in pixels[..n * 4].chunks_exact(4) {
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    } else if pixels.len() >= n * 3 {
        for c in pixels[..n * 3].chunks_exact(3) {
            rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    } else if pixels.len() >= n {
        for &g in &pixels[..n] {
            rgba.extend_from_slice(&[g, g, g, 255]);
        }
    } else {
        return None;
    }
    let icon = Icon {
        rgba: Arc::from(rgba),
        width: w,
        height: h,
    };
    downsample_to_target(icon, target)
}

/// 把 SVG 文件栅格化为直通 RGBA 图标。`target_size` 为最长边像素。
/// 失败（文件缺失 / 解析错误）返回 `None` —— 图标缺失不 panic（SD §IX 只约束字体）。
pub fn rasterize_svg(path: impl AsRef<Path>, target_size: u32) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    let tree = parse_svg(&data)?;
    let svg_size = tree.size();
    let scale = target_size as f32 / svg_size.width().max(svg_size.height());
    let px_w = (svg_size.width() * scale).round() as u32;
    let px_h = (svg_size.height() * scale).round() as u32;
    render_svg_scaled(&tree, px_w, px_h, scale)
}

/// 统一入口 SVG 分支：`target` 给定时按物理像素 **Cover** 光栅（等比放大铺满 + 居中裁剪），
/// 无 `target` 时用 viewBox 原尺寸。文件的 `preserveAspectRatio` 已由 usvg 解析进树
/// （内容映射到 `tree.size()` 视口），此处对整树等比缩放即照文件语义。
fn render_svg(data: &[u8], target: Option<(u32, u32)>) -> Option<Icon> {
    let tree = parse_svg(data)?;
    let svg_size = tree.size();
    match target {
        Some((tw, th)) if tw > 0 && th > 0 => {
            let scale = (tw as f32 / svg_size.width()).max(th as f32 / svg_size.height());
            render_svg_scaled(&tree, tw, th, scale)
        }
        _ => {
            let px_w = (svg_size.width().round() as u32).max(1);
            let px_h = (svg_size.height().round() as u32).max(1);
            render_svg_scaled(&tree, px_w, px_h, 1.0)
        }
    }
}

/// usvg 解析（跨 `rasterize_svg` 与统一入口共用）。
fn parse_svg(data: &[u8]) -> Option<resvg::usvg::Tree> {
    let options = resvg::usvg::Options::default();
    resvg::usvg::Tree::from_data(data, &options).ok()
}

/// SVG 光栅核心：画布 `px_w × px_h`，SVG 内容以 `scale` 等比缩放并**居中**放置
/// （Cover：缩放后必然铺满画布，溢出部分被画布裁掉），输出去预乘直通 RGBA。
fn render_svg_scaled(
    tree: &resvg::usvg::Tree,
    px_w: u32,
    px_h: u32,
    scale: f32,
) -> Option<Icon> {
    if px_w == 0 || px_h == 0 {
        return None;
    }
    let svg_size = tree.size();
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px_w, px_h)?;
    // 先缩放后平移：内容中心对齐画布中心（Transform 组合按右乘顺序应用）。
    let tx = (px_w as f32 - svg_size.width() * scale) / 2.0;
    let ty = (px_h as f32 - svg_size.height() * scale) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale)
        .post_translate(tx, ty);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Some(Icon {
        rgba: Arc::from(unpremultiply(pixmap.data())),
        width: px_w,
        height: px_h,
    })
}

/// 统一入口的降采样门槛：给了 `target` 且源图任一边超过 2× 目标时，
/// 盒式降采样到 ≤ 2× 目标（3840×2400 壁纸给 1280×800 缩略图 → 解出 2560×1600 而非全尺寸）。
fn downsample_to_target(icon: Icon, target: Option<(u32, u32)>) -> Option<Icon> {
    // 无 target（全屏用法）原样交出，不做降采样。
    let Some((tw, th)) = target else { return Some(icon) };
    if tw == 0 || th == 0 {
        return Some(icon);
    }
    let cap = (tw.saturating_mul(2), th.saturating_mul(2));
    if icon.width <= cap.0 && icon.height <= cap.1 {
        return Some(icon);
    }
    downsample_rgba(&icon.rgba, icon.width, icon.height, cap.0, cap.1)
}

/// 盒式降采样（面积加权平均，含 alpha）：等比缩到 `max_w × max_h` 之内。
/// 相比最近邻能压掉 JPEG 高频颗粒，后续 GPU 缩半采样不再闪噪点。
fn downsample_rgba(rgba: &[u8], w: u32, h: u32, max_w: u32, max_h: u32) -> Option<Icon> {
    if w == 0 || h == 0 || rgba.len() != (w as usize) * (h as usize) * 4 {
        return None;
    }
    let scale = (max_w as f32 / w as f32).min(max_h as f32 / h as f32);
    if scale <= 0.0 {
        return None;
    }
    let dw = ((w as f32 * scale).round() as u32).clamp(1, w);
    let dh = ((h as f32 * scale).round() as u32).clamp(1, h);
    let mut out = vec![0u8; (dw as usize) * (dh as usize) * 4];
    // 每个目标像素映射回源矩形 [sx0,sx1)×[sy0,sy1)，对覆盖到的源像素按重叠面积加权。
    for dy in 0..dh {
        let sy0 = dy as f32 * h as f32 / dh as f32;
        let sy1 = (dy + 1) as f32 * h as f32 / dh as f32;
        for dx in 0..dw {
            let sx0 = dx as f32 * w as f32 / dw as f32;
            let sx1 = (dx + 1) as f32 * w as f32 / dw as f32;
            let mut acc = [0.0f32; 4];
            let mut sum = 0.0f32;
            for sy in (sy0.floor() as u32)..((sy1.ceil() as u32).min(h)) {
                let wy = (sy as f32 + 1.0).min(sy1) - (sy as f32).max(sy0);
                if wy <= 0.0 {
                    continue;
                }
                for sx in (sx0.floor() as u32)..((sx1.ceil() as u32).min(w)) {
                    let wx = (sx as f32 + 1.0).min(sx1) - (sx as f32).max(sx0);
                    if wx <= 0.0 {
                        continue;
                    }
                    let src = ((sy as usize) * (w as usize) + (sx as usize)) * 4;
                    let a = wx * wy;
                    acc[0] += rgba[src] as f32 * a;
                    acc[1] += rgba[src + 1] as f32 * a;
                    acc[2] += rgba[src + 2] as f32 * a;
                    acc[3] += rgba[src + 3] as f32 * a;
                    sum += a;
                }
            }
            if sum > 0.0 {
                let dst = ((dy as usize) * (dw as usize) + (dx as usize)) * 4;
                for c in 0..4 {
                    out[dst + c] = (acc[c] / sum).round().clamp(0.0, 255.0) as u8;
                }
            }
        }
    }
    Some(Icon {
        rgba: Arc::from(out),
        width: dw,
        height: dh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_svg_path() -> Option<std::path::PathBuf> {
        // 用临时目录写一个最小 SVG（仓库内不携带 SVG 测试资产）
        let dir = std::env::temp_dir();
        let path = dir.join("kanesumi_test_icon.svg");
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                 <rect width="24" height="24" fill="#E57812"/>
               </svg>"##;
        std::fs::write(&path, svg).ok()?;
        Some(path)
    }

    #[test]
    fn rasterizes_svg_to_rgba() {
        let Some(path) = test_svg_path() else { return };
        let icon = rasterize_svg(&path, 24).expect("光栅化应成功");
        assert_eq!(icon.width, 24);
        assert_eq!(icon.height, 24);
        assert_eq!(icon.rgba.len(), 24 * 24 * 4);
        // 首像素应为橙色（直通 RGBA）
        assert!(icon.rgba[0] > 200, "R 通道橙");
        assert!(icon.rgba[1] < 200, "G 通道低");
        assert!(icon.rgba[3] == 255, "完全不透明");
    }

    #[test]
    fn missing_svg_returns_none() {
        assert!(rasterize_svg("/nonexistent/icon.svg", 24).is_none());
    }

    /// JPEG 夹具（16×16 纯橙 #E57812，224 字节）：按 `ImageKind::Jpeg` 显式分派解码。
    #[test]
    fn rasterizes_jpeg_bytes() {
        let data = include_bytes!("../assets/test_icon.jpg");
        let icon = rasterize_image_bytes(data, ImageKind::Jpeg).expect("JPEG 应解码");
        assert_eq!((icon.width, icon.height), (16, 16));
        assert_eq!(icon.rgba.len(), 16 * 16 * 4);
        let px = &icon.rgba[..4];
        assert_eq!(px[3], 255, "JPEG 输出 alpha 恒 255");
        // 纯橙 #E57812 经 JPEG 有损编码 → 实测 e3 78 10，留容差（防通道错位）。
        assert!((px[0] as i32 - 227).abs() <= 16, "R≈227: {}", px[0]);
        assert!((px[1] as i32 - 120).abs() <= 24, "G≈120: {}", px[1]);
        assert!((px[2] as i32 - 16).abs() <= 24, "B≈16: {}", px[2]);
    }

    /// 损坏数据（PNG / JPEG 各一）返回 None，不 panic。
    #[test]
    fn corrupted_bytes_return_none() {
        assert!(rasterize_image_bytes(b"not an image", ImageKind::Png).is_none());
        assert!(rasterize_image_bytes(b"not an image", ImageKind::Jpeg).is_none());
        assert!(rasterize_image_bytes(b"not an image", ImageKind::Svg).is_none());
        assert!(rasterize_image_bytes(&[], ImageKind::Jpeg).is_none());
    }

    /// 统一入口用的 16:10 内联 SVG（左半品红 / 右半暮蓝，对齐壁纸色片 BRAND §Ⅱ-2）。
    const TEST_SVG_16_10: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1600 1000">
        <rect width="1600" height="1000" fill="#2D6FE0"/>
        <rect width="800" height="1000" fill="#E0457B"/>
    </svg>"##;

    /// 造一张 64×64 测试 PNG：上半橙 #E57812、下半蓝 #2D6FE0（全不透明，预乘 = 原值）。
    fn write_test_png(path: &std::path::Path) {
        let mut pixmap = resvg::tiny_skia::Pixmap::new(64, 64).unwrap();
        let orange =
            resvg::tiny_skia::PremultipliedColorU8::from_rgba(0xE5, 0x78, 0x12, 0xFF).unwrap();
        let blue =
            resvg::tiny_skia::PremultipliedColorU8::from_rgba(0x2D, 0x6F, 0xE0, 0xFF).unwrap();
        for (i, px) in pixmap.pixels_mut().iter_mut().enumerate() {
            *px = if i / 64 < 32 { orange } else { blue };
        }
        std::fs::write(path, pixmap.encode_png().unwrap()).unwrap();
    }

    /// 扩展名分派（不区分大小写）：.PNG / .JpG / .Svg 各解一次。
    #[test]
    fn dispatches_by_extension_case_insensitive() {
        let dir = std::env::temp_dir();
        let png_path = dir.join("kanesumi_wp1_case.PNG");
        write_test_png(&png_path);
        let icon = rasterize_image(&png_path, None).expect("大写 .PNG 应按 png 分派");
        assert_eq!((icon.width, icon.height), (64, 64));

        let jpg_path = dir.join("kanesumi_wp1_case.JpG");
        std::fs::write(&jpg_path, include_bytes!("../assets/test_icon.jpg")).unwrap();
        let icon = rasterize_image(&jpg_path, None).expect("混合大小写 .JpG 应按 jpeg 分派");
        assert_eq!((icon.width, icon.height), (16, 16));

        let svg_path = dir.join("kanesumi_wp1_case.SVG");
        std::fs::write(&svg_path, TEST_SVG_16_10).unwrap();
        let icon = rasterize_image(&svg_path, None).expect("大写 .SVG 应按 svg 分派");
        assert_eq!((icon.width, icon.height), (1600, 1000), "无 target 用 viewBox 原尺寸");
    }

    /// 未知扩展名 / 缺失文件返回 None，不 panic。
    #[test]
    fn unknown_extension_and_missing_file_return_none() {
        let dir = std::env::temp_dir();
        let bmp_path = dir.join("kanesumi_wp1_case.bmp");
        std::fs::write(&bmp_path, b"whatever").unwrap();
        assert!(rasterize_image(&bmp_path, None).is_none(), "未知扩展名 None");
        assert!(rasterize_image("/nonexistent/wallpaper.jpg", None).is_none(), "缺失文件 None");
    }

    /// PNG 大图降采样：64×64 给 target (16,16) → 上限 32×32，内容纯色区不混色。
    #[test]
    fn png_downsamples_to_2x_target() {
        let dir = std::env::temp_dir();
        let path = dir.join("kanesumi_wp1_ds.png");
        write_test_png(&path);
        let icon = rasterize_image(&path, Some((16, 16))).expect("应解码");
        assert_eq!((icon.width, icon.height), (32, 32), "降采样到 2× target");
        let px = |x: u32, y: u32| ((y * icon.width + x) * 4) as usize;
        assert!(icon.rgba[px(0, 4)] > 200, "上区应仍为橙 R");
        assert!(icon.rgba[px(0, 28) + 2] > 180, "下区应仍为蓝 B");
        // 未超门槛（64 < 2×48 = 96）→ 原尺寸交出。
        let full = rasterize_image(&path, Some((48, 48))).expect("应解码");
        assert_eq!((full.width, full.height), (64, 64), "未超 2× target 不降采样");
    }

    /// JPEG 大图降采样：testdata 480×480 壁纸样张给 target (100,100) → 解出 200×200。
    #[test]
    fn jpeg_downsamples_to_2x_target() {
        let dir = std::env::temp_dir();
        let path = dir.join("kanesumi_wp1_ds.jpg");
        std::fs::write(&path, include_bytes!("../testdata/wallpaper_480.jpg")).unwrap();
        let full = rasterize_image(&path, None).expect("JPEG 应解码");
        assert_eq!((full.width, full.height), (480, 480), "无 target 保持原尺寸");
        let small = rasterize_image(&path, Some((100, 100))).expect("JPEG 应解码");
        assert_eq!((small.width, small.height), (200, 200), "降采样到 2× target");
        assert_eq!(small.rgba.len(), 200 * 200 * 4);
    }

    /// SVG 统一入口：target 物理像素 Cover 光栅（16:10 → 16:10 等比铺满，无留白）。
    #[test]
    fn svg_target_covers_and_centers() {
        let icon = render_svg(TEST_SVG_16_10.as_bytes(), Some((800, 500))).expect("SVG 应光栅");
        assert_eq!((icon.width, icon.height), (800, 500));
        assert_eq!(icon.rgba.len(), 800 * 500 * 4);
        // 缩放对位：分界线在源 x=800 → 目标 x=400，左半品红 / 右半暮蓝。
        let px = |x: u32, y: u32| ((y * icon.width + x) * 4) as usize;
        let left = icon.rgba[px(100, 250)];
        assert!(left as i32 > 150 && icon.rgba[px(100, 250) + 2] < 140, "左半品红: R={left}");
        assert!(icon.rgba[px(700, 250) + 2] > 150, "右半暮蓝");
        // 非同比例 target：画布取 target，内容 Cover 居中、上下溢出裁掉。
        let cropped = render_svg(TEST_SVG_16_10.as_bytes(), Some((800, 400))).expect("SVG 应光栅");
        assert_eq!((cropped.width, cropped.height), (800, 400));
        // 上缘被裁：内容垂直居中，画布上缘对应源 y=50（内容 800×500，偏移 -50）。
        let top = (400 * 4) as usize;
        assert!(cropped.rgba[top + 2] > 150, "上缘应是暮蓝（Cover 裁剪，非留白）");
    }

    /// `rasterize_png` 兼容路径与统一入口 PNG 分支行为一致（同约定同像素）。
    #[test]
    fn rasterize_png_compat_uses_same_branch() {
        let dir = std::env::temp_dir();
        let path = dir.join("kanesumi_wp1_compat.png");
        write_test_png(&path);
        let a = rasterize_png(&path).expect("应解码");
        let b = rasterize_image(&path, None).expect("应解码");
        assert_eq!(a, b);
    }
}
