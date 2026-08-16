// 图标管线 —— SVG → RGBA 光栅化（对应 ASSETS.md / ether-assets 同款管线，Kanesumi 自足）。
//
// 输出直通 RGBA（非预乘），供 `Scene::image` 上传纹理。Metro 风格：纯色图标可用 tint 染色。

use std::path::Path;

/// 已光栅化的图标（直通 RGBA 像素）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Icon {
    pub rgba: Vec<u8>,
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
        for y in 0..self.height {
            for x in 0..self.width {
                let dx = x as f32 + 0.5 - cx;
                let dy = y as f32 + 0.5 - cy;
                if dx * dx + dy * dy > r * r {
                    let i = ((y * self.width + x) * 4) as usize;
                    self.rgba[i + 3] = 0;
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
    let pixmap = resvg::tiny_skia::Pixmap::decode_png(&data).ok()?;
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
        rgba,
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
        rgba,
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
}
