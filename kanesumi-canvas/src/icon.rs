// 图标管线 —— SVG → RGBA 光栅化（对应 ASSETS.md / ether-assets 同款管线，Kanesumi 自足）。
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
/// tiny-skia 输出 premultiplied RGBA → 去预乘直通 RGBA。参 rasterize_svg。
pub fn rasterize_png(path: impl AsRef<Path>) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    rasterize_png_bytes(&data)
}

/// 把 PNG 字节解码为直通 RGBA 图标。参 rasterize_png。
pub fn rasterize_png_bytes(data: &[u8]) -> Option<Icon> {
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(data).ok()?;
    let (w, h) = (pixmap.width(), pixmap.height());
    let raw = pixmap.data();
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
    Some(Icon {
        rgba: Arc::from(rgba),
        width: w,
        height: h,
    })
}

/// 按**文件头魔数**分派解码 PNG / JPEG（不信扩展名）。用于壁纸等外部图片。
/// 失败 / 未知格式返回 None。JPEG 输出不透明 RGBA（alpha 255）。
pub fn rasterize_image(path: impl AsRef<Path>) -> Option<Icon> {
    let data = std::fs::read(path).ok()?;
    rasterize_image_bytes(&data)
}

/// 按字节魔数分派：PNG（`\x89PNG\r\n\x1a\n`）/ JPEG（`FF D8 FF`）。参 rasterize_image。
pub fn rasterize_image_bytes(data: &[u8]) -> Option<Icon> {
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        rasterize_png_bytes(data)
    } else if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        rasterize_jpeg_bytes(data)
    } else {
        None
    }
}

/// 把 JPEG 字节解码为不透明 RGBA（alpha 255）。zune-jpeg 默认输出 RGB（某些灰度图输出
/// 单通道 Luma），按输出长度判定通道数；失败返回 None。
fn rasterize_jpeg_bytes(data: &[u8]) -> Option<Icon> {
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
    Some(Icon {
        rgba: Arc::from(rgba),
        width: w,
        height: h,
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

    let mut pixmap = resvg::tiny_skia::Pixmap::new(px_w, px_h)?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    // tiny-skia 输出 premultiplied RGBA → 直通 RGBA（与 ether-assets 同款去预乘）。
    let raw = pixmap.data();
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
    Some(Icon {
        rgba: Arc::from(rgba),
        width: px_w,
        height: px_h,
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

    /// 魔数分派：JPEG 夹具（16×16 纯橙 #E57812，224 字节）解码尺寸 / 像素正确。
    #[test]
    fn rasterizes_jpeg_by_magic() {
        let data = include_bytes!("../assets/test_icon.jpg");
        let icon = rasterize_image_bytes(data).expect("JPEG 应解码");
        assert_eq!((icon.width, icon.height), (16, 16));
        assert_eq!(icon.rgba.len(), 16 * 16 * 4);
        let px = &icon.rgba[..4];
        assert_eq!(px[3], 255, "JPEG 输出 alpha 恒 255");
        // 纯橙 #E57812 经 JPEG 有损编码 → 实测 e3 78 10，留容差（防通道错位）。
        assert!((px[0] as i32 - 227).abs() <= 16, "R≈227: {}", px[0]);
        assert!((px[1] as i32 - 120).abs() <= 24, "G≈120: {}", px[1]);
        assert!((px[2] as i32 - 16).abs() <= 24, "B≈16: {}", px[2]);
    }

    /// 未知格式（无 PNG/JPEG 魔数）返回 None，不 panic。
    #[test]
    fn unknown_image_bytes_return_none() {
        assert!(rasterize_image_bytes(b"not an image").is_none());
        assert!(rasterize_image_bytes(&[]).is_none());
    }
}
