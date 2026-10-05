// 图标与图像管线 —— PNG / JPEG / SVG 统一解码入口（对应 ASSETS.md / ether-assets 同款
// 管线，Kanesumi 自足）。JPEG 用 zune-jpeg、SVG 用 resvg（wp1-kanesumi-image-decode）。
//
// 输出直通 RGBA（非预乘），供 `Scene::image` 上传纹理。Metro 风格：纯色图标可用 tint 染色。

use std::path::Path;
use std::sync::Arc;

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

/// 把 PNG 文件解码为直通 RGBA 图标（用户头像 `~/.face` 等）。失败返回 None。
/// 统一入口的 PNG 分支包装，参 `rasterize_image`。
pub fn rasterize_png(path: impl AsRef<Path>) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    rasterize_png_bytes(&data)
}

/// 把 PNG 字节解码为直通 RGBA 图标。参 rasterize_png。
pub fn rasterize_png_bytes(data: &[u8]) -> Option<Icon> {
    decode_png(data, None)
}

/// 统一图像解码入口：按扩展名（不区分大小写）分派 `png` / `jpg` / `jpeg` / `svg`。
/// `target` 给定时：SVG 按该物理像素尺寸 Cover 光栅（等比放大到覆盖后居中裁剪，
/// `preserveAspectRatio` 照文件、由 usvg 处理）；PNG / JPEG 源图 ≥ 2× 目标时先盒式
/// 降采样到 ≤ 2× 目标再交出（3840×2400 的壁纸出 1280×800 缩略图不该占满内存）。
/// 失败返回 None 不 panic。参 WALLPAPER_DESIGN §Ⅲ、wp2-wallpaper-wiring。
pub fn rasterize_image(path: impl AsRef<Path>, target: Option<(u32, u32)>) -> Option<Icon> {
    let path = path.as_ref();
    let kind = ImageKind::from_extension(path.extension()?.to_str()?)?;
    let data = std::fs::read(path).ok()?;
    rasterize_image_bytes(&data, kind, target)
}

/// 同 [`rasterize_image`]，但格式由调用方显式给出（字节流没有扩展名可查）。
pub fn rasterize_image_bytes(data: &[u8], kind: ImageKind, target: Option<(u32, u32)>) -> Option<Icon> {
    match kind {
        ImageKind::Png => decode_png(data, target),
        ImageKind::Jpeg => decode_jpeg(data, target),
        ImageKind::Svg => decode_svg(data, target),
    }
}

/// 图片格式（统一解码入口的分派依据）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageKind {
    Png,
    Jpeg,
    Svg,
}

impl ImageKind {
    /// 按扩展名判别，不区分大小写；未知扩展名返回 None。
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(ImageKind::Png),
            "jpg" | "jpeg" => Some(ImageKind::Jpeg),
            "svg" => Some(ImageKind::Svg),
            _ => None,
        }
    }
}

/// PNG 解码：tiny-skia 出预乘 RGBA，按需盒式降采样后统一去预乘。
fn decode_png(data: &[u8], target: Option<(u32, u32)>) -> Option<Icon> {
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(data).ok()?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let premul = match target {
        Some(t) => {
            let f = downsample_factor(w, h, t);
            if f >= 2 {
                let (buf, nw, nh) = box_downsample(pixmap.data(), w, h, f);
                return Some(Icon {
                    rgba: Arc::from(premul_to_straight(&buf)),
                    width: nw,
                    height: nh,
                });
            }
            pixmap.data().to_vec()
        }
        None => pixmap.data().to_vec(),
    };
    Some(Icon {
        rgba: Arc::from(premul_to_straight(&premul)),
        width: w,
        height: h,
    })
}

/// JPEG 解码：zune-jpeg 默认输出 RGB（某些灰度图输出单通道 Luma），按输出长度判定
/// 通道数；输出不透明 RGBA（alpha 255），按需盒式降采样。失败返回 None。
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
    let (buf, w, h) = match target {
        Some(t) => {
            let f = downsample_factor(w, h, t);
            if f >= 2 {
                box_downsample(&rgba, w, h, f)
            } else {
                (rgba, w, h)
            }
        }
        None => (rgba, w, h),
    };
    Some(Icon {
        rgba: Arc::from(buf),
        width: w,
        height: h,
    })
}

/// SVG 解码：无 `target` 用 viewBox 原尺寸；有 `target` 按 Cover——等比放大到完全
/// 覆盖目标矩形，再居中裁出 `target`（文件自身的 preserveAspectRatio 由 usvg 尊重）。
fn decode_svg(data: &[u8], target: Option<(u32, u32)>) -> Option<Icon> {
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(data, &options).ok()?;
    let svg_size = tree.size();
    let Some((tw, th)) = target else {
        let (px_w, px_h) = (svg_size.width().round() as u32, svg_size.height().round() as u32);
        if px_w == 0 || px_h == 0 {
            return None;
        }
        return render_tree(&tree, px_w, px_h, 1.0);
    };
    if tw == 0 || th == 0 {
        return None;
    }
    let scale = (tw as f32 / svg_size.width()).max(th as f32 / svg_size.height());
    let full_w = (svg_size.width() * scale).ceil() as u32;
    let full_h = (svg_size.height() * scale).ceil() as u32;
    if full_w == 0 || full_h == 0 {
        return None;
    }
    let full = render_tree(&tree, full_w, full_h, scale)?;
    // 居中裁剪（覆盖尺寸 ≥ 目标，余量摊到两侧）。
    let (ox, oy) = (full_w.saturating_sub(tw) / 2, full_h.saturating_sub(th) / 2);
    let mut rgba = Vec::with_capacity((tw as usize) * (th as usize) * 4);
    for y in oy..(oy + th).min(full_h) {
        let start = ((y * full_w + ox) * 4) as usize;
        let end = start + (tw as usize) * 4;
        rgba.extend_from_slice(&full.rgba[start..end]);
    }
    Some(Icon {
        rgba: Arc::from(rgba),
        width: tw,
        height: th,
    })
}

/// 把 SVG 文件栅格化为直通 RGBA 图标。`target_size` 为最长边像素。
/// 失败（文件缺失 / 解析错误）返回 `None` —— 图标缺失不 panic（SD §IX 只约束字体）。
pub fn rasterize_svg(path: impl AsRef<Path>, target_size: u32) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    let options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(&data, &options).ok()?;

    let svg_size = tree.size();
    let scale = target_size as f32 / svg_size.width().max(svg_size.height());
    let px_w = (svg_size.width() * scale).round() as u32;
    let px_h = (svg_size.height() * scale).round() as u32;
    if px_w == 0 || px_h == 0 {
        return None;
    }
    render_tree(&tree, px_w, px_h, scale)
}

/// 把 usvg 树以 `scale` 渲染到 `px_w × px_h` 的位图并转为直通 RGBA。
fn render_tree(
    tree: &resvg::usvg::Tree,
    px_w: u32,
    px_h: u32,
    scale: f32,
) -> Option<Icon> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(px_w, px_h)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Some(Icon {
        rgba: Arc::from(premul_to_straight(pixmap.data())),
        width: px_w,
        height: px_h,
    })
}

/// tiny-skia 输出 premultiplied RGBA → 直通 RGBA（与 ether-assets 同款去预乘）。
/// a=0 的像素 RGB 置 0（预乘下颜色本就无意义）。
fn premul_to_straight(raw: &[u8]) -> Vec<u8> {
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

/// 源图 ≥ 2× 目标时所需的盒式降采样倍数（两维分别看，取大者）；不足 2× 返回 1（不降采样）。
/// 3840×2400 给 (1280, 800)：ceil(3840/2560)=2 → f=2 → 产出 1920×1200 ≤ 2× 目标。
fn downsample_factor(w: u32, h: u32, target: (u32, u32)) -> u32 {
    let (tw, th) = (target.0.max(1), target.1.max(1));
    let fx = w.div_ceil(2 * tw);
    let fy = h.div_ceil(2 * th);
    let f = fx.max(fy).max(1);
    if f < 2 {
        1
    } else {
        f
    }
}

/// f 倍盒式降采样（f×f 块取平均；末尾不足一块的余数行 / 列裁掉）。
/// 在预乘域平均：半透明像素的颜色权重随 alpha 正确衰减，平均后再统一去预乘。
fn box_downsample(premul: &[u8], w: u32, h: u32, f: u32) -> (Vec<u8>, u32, u32) {
    let (nw, nh) = (w / f, h / f);
    if nw == 0 || nh == 0 {
        return (premul.to_vec(), w, h);
    }
    let mut out = vec![0u8; (nw as usize) * (nh as usize) * 4];
    for oy in 0..nh {
        for ox in 0..nw {
            let (mut sr, mut sg, mut sb, mut sa) = (0u32, 0u32, 0u32, 0u32);
            for y in (oy * f)..((oy + 1) * f) {
                for x in (ox * f)..((ox + 1) * f) {
                    let i = ((y * w + x) * 4) as usize;
                    sr += premul[i] as u32;
                    sg += premul[i + 1] as u32;
                    sb += premul[i + 2] as u32;
                    sa += premul[i + 3] as u32;
                }
            }
            let n = (f * f) as u32;
            let o = ((oy * nw + ox) * 4) as usize;
            out[o] = (sr / n) as u8;
            out[o + 1] = (sg / n) as u8;
            out[o + 2] = (sb / n) as u8;
            out[o + 3] = (sa / n) as u8;
        }
    }
    (out, nw, nh)
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

    /// JPEG 夹具（16×16 纯橙 #E57812，224 字节）解码尺寸 / 像素正确。
    #[test]
    fn rasterizes_jpeg_bytes() {
        let data = include_bytes!("../assets/test_icon.jpg");
        let icon = rasterize_image_bytes(data, ImageKind::Jpeg, None).expect("JPEG 应解码");
        assert_eq!((icon.width, icon.height), (16, 16));
        assert_eq!(icon.rgba.len(), 16 * 16 * 4);
        let px = &icon.rgba[..4];
        assert_eq!(px[3], 255, "JPEG 输出 alpha 恒 255");
        // 纯橙 #E57812 经 JPEG 有损编码 → 实测 e3 78 10，留容差（防通道错位）。
        assert!((px[0] as i32 - 227).abs() <= 16, "R≈227: {}", px[0]);
        assert!((px[1] as i32 - 120).abs() <= 24, "G≈120: {}", px[1]);
        assert!((px[2] as i32 - 16).abs() <= 24, "B≈16: {}", px[2]);
    }

    /// 扩展名分派（不区分大小写）：同一份 JPEG 字节按 .JPG / .jpeg 都能读出；
    /// 未知扩展名即使内容是合法 PNG 也返回 None。
    #[test]
    fn dispatches_by_extension_case_insensitive() {
        let data = include_bytes!("../assets/test_icon.jpg");
        let dir = std::env::temp_dir();
        for name in ["kanesumi_wp1_a.JPG", "kanesumi_wp1_b.jpeg"] {
            let path = dir.join(name);
            std::fs::write(&path, data).unwrap();
            let icon = rasterize_image(&path, None).expect("扩展名分派应成功");
            assert_eq!((icon.width, icon.height), (16, 16), "{}", name);
            std::fs::remove_file(&path).ok();
        }
        let path = dir.join("kanesumi_wp1_c.bmp");
        std::fs::write(&path, b"\x89PNG\r\n\x1a\nnot really").unwrap();
        assert!(rasterize_image(&path, None).is_none(), "未知扩展名应 None");
        std::fs::remove_file(&path).ok();
    }

    /// 损坏文件返回 None 不 panic（PNG 魔数后是垃圾）。
    #[test]
    fn corrupt_png_returns_none() {
        assert!(rasterize_image_bytes(b"\x89PNG\r\n\x1a\ngarbage", ImageKind::Png, None).is_none());
        assert!(rasterize_image_bytes(b"not an image", ImageKind::Jpeg, None).is_none());
        assert!(rasterize_image_bytes(&[], ImageKind::Svg, None).is_none());
    }

    /// 盒式降采样：256×256 PNG 给 (64,64) 目标 → f=2 → 128×128（≤ 2× 目标）。
    #[test]
    fn downsamples_large_png_to_target_bound() {
        let mut pm = resvg::tiny_skia::Pixmap::new(256, 256).unwrap();
        pm.fill(resvg::tiny_skia::Color::from_rgba8(30, 60, 90, 255));
        let png = pm.encode_png().unwrap();
        let icon = rasterize_image_bytes(&png, ImageKind::Png, Some((64, 64))).expect("解码应成功");
        assert_eq!((icon.width, icon.height), (128, 128));
        assert_eq!(icon.rgba.len(), 128 * 128 * 4);
        // 无目标时不降采样。
        let full = rasterize_image_bytes(&png, ImageKind::Png, None).unwrap();
        assert_eq!((full.width, full.height), (256, 256));
    }

    /// SVG Cover：24×24 视图给 (48,32) 目标 → 放大到 48×48 后居中裁 48×32。
    #[test]
    fn svg_cover_crops_to_target() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                 <rect width="24" height="24" fill="#E57812"/>
               </svg>"##;
        let icon = rasterize_image_bytes(svg.as_bytes(), ImageKind::Svg, Some((48, 32)))
            .expect("Cover 光栅应成功");
        assert_eq!((icon.width, icon.height), (48, 32));
        assert_eq!(icon.rgba.len(), 48 * 32 * 4);
        assert!(icon.rgba[0] > 200, "裁剪窗口内仍是橙色");
        // 无 target 用 viewBox 原尺寸。
        let natural = rasterize_image_bytes(svg.as_bytes(), ImageKind::Svg, None).unwrap();
        assert_eq!((natural.width, natural.height), (24, 24));
    }
}
