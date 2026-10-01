// Image —— 位图 / 图标元素（XAML Image + Stretch）。
//
// 持已光栅化的 `Icon`（RGBA）或 SVG 路径：SVG 按**目标尺寸 × 缩放**惰性光栅并缓存，
// 尺寸变化时重光栅（保证高 DPI 下清晰，而非把一张固定小图拉大）。`Stretch` 取 XAML 四值语义，
// 目标矩形用 `fitted_rect` 纯函数算出，便于单测。
//
// 无子节点；`hit_test` 沿用默认（矩形内命中），与 Label 一致。
//
// 不透明度：`opacity`（`None` = 1.0）透传 `Scene::image_with_opacity`。用 `Option` 而非 `f32`：
// 本类型派生 `Default`，裸 `f32` 会默认成 0 → 整图不可见。

use std::path::PathBuf;

use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::{Scene, rasterize_svg};
use kanesumi_core::{Brush, Rect, Size};

use crate::widget::{MeasureCtx, PaintCtx, Widget};

/// 缩放模式（XAML `Stretch`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Stretch {
    /// 固有尺寸、居中（可越出容器，绘制时裁剪）。
    None,
    /// 等比缩放至完整放进容器（letterbox）。
    #[default]
    Uniform,
    /// 等比缩放至铺满容器（超出部分裁剪）。
    UniformToFill,
    /// 非等比拉伸铺满容器。
    Fill,
}

/// 图源。
#[derive(Debug, Clone, PartialEq)]
pub enum ImageSource {
    /// 已光栅化的位图（PNG / 内存图标）。
    Raster(Icon),
    /// SVG 路径 + 固有尺寸（用于 measure；光栅在 paint 时按目标尺寸惰性做）。
    Svg { path: PathBuf, natural: Size },
}

impl Default for ImageSource {
    fn default() -> Self {
        Self::Raster(Icon::default())
    }
}

/// 已光栅化缓存（键 = 由目标矩形算出的像素边长）。
#[derive(Debug, Clone, PartialEq)]
struct RasterCache {
    px: u32,
    icon: Icon,
}

/// 图像元素。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Image {
    pub source: ImageSource,
    pub stretch: Stretch,
    /// 单色图标染色（用 `ThemeColor::*` 才会随主题变化）。
    pub tint: Option<Brush>,
    /// 显式宽（优先于图源固有尺寸）。
    pub width: Option<f32>,
    /// 显式高（优先于图源固有尺寸）。
    pub height: Option<f32>,
    /// SVG 重光栅的显示缩放（逻辑 px → 目标像素），默认 1.0。
    pub scale: f32,
    /// 整图不透明度（0..1；`None` = 1.0）。
    pub opacity: Option<f32>,
    raster: Option<RasterCache>,
}

impl Image {
    pub fn new(source: ImageSource) -> Self {
        Self {
            source,
            stretch: Stretch::default(),
            tint: None,
            width: None,
            height: None,
            scale: 1.0,
            opacity: None,
            raster: None,
        }
    }

    /// 位图源。
    pub fn raster(icon: Icon) -> Self {
        Self::new(ImageSource::Raster(icon))
    }

    /// SVG 源；固有尺寸从 SVG 头部（`width`/`height` 或 `viewBox`）解析，解析不出则 24×24。
    pub fn svg(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let natural = svg_natural_size(&path).unwrap_or(Size::new(24.0, 24.0));
        Self::new(ImageSource::Svg { path, natural })
    }

    /// 整图不透明度（0..1）。
    pub fn opacity(mut self, o: f32) -> Self {
        self.opacity = Some(o.clamp(0.0, 1.0));
        self
    }

    pub fn stretch(mut self, s: Stretch) -> Self {
        self.stretch = s;
        self
    }

    pub fn tint(mut self, c: impl Into<Brush>) -> Self {
        self.tint = Some(c.into());
        self
    }

    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.width = Some(width);
        self.height = Some(height);
        self
    }

    /// 图源固有尺寸（显式宽高优先的 measure 基准）。
    pub fn intrinsic_size(&self) -> Size {
        match &self.source {
            ImageSource::Raster(i) => i.size(),
            ImageSource::Svg { natural, .. } => *natural,
        }
    }

    /// 取（必要时重光栅化）的位图。SVG 目标像素边长 = `ceil(目标长边 × scale)`。
    fn rasterized(&mut self, dest: Rect) -> Option<&Icon> {
        match &self.source {
            ImageSource::Raster(i) => Some(i),
            ImageSource::Svg { path, .. } => {
                let scale = self.scale.max(0.01);
                let px = ((dest.size.width.max(dest.size.height)) * scale)
                    .round()
                    .max(1.0) as u32;
                let stale = self.raster.as_ref().is_none_or(|c| c.px != px);
                if stale {
                    self.raster = rasterize_svg(path, px)
                        .map(|icon| RasterCache { px, icon });
                }
                self.raster.as_ref().map(|c| &c.icon)
            }
        }
    }
}

impl Widget for Image {
    /// 显式宽高优先，否则图源固有尺寸。
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        let i = self.intrinsic_size();
        Size::new(
            self.width.unwrap_or(i.width).max(0.0),
            self.height.unwrap_or(i.height).max(0.0),
        )
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let dest = ctx.rect();
        if dest.size.width <= 0.0 || dest.size.height <= 0.0 {
            return;
        }
        let intrinsic = self.intrinsic_size();
        let stretch = self.stretch;
        let tint = self.tint.map(|b| b.resolve(ctx.theme()));
        let opacity = self.opacity.unwrap_or(1.0);
        let Some(icon) = self.rasterized(dest) else {
            return;
        };
        let fitted = fitted_rect(intrinsic, dest, stretch);
        // 裁剪到自身矩形：UniformToFill / None 的目标矩形可越出容器。
        scene.push_clip(dest);
        scene.image_with_opacity(icon, fitted, tint, opacity);
        scene.pop_clip();
    }
}

/// 把图源放进 `dest`（XAML Stretch 语义）；结果居中。纯函数，供单测。
pub fn fitted_rect(source: Size, dest: Rect, stretch: Stretch) -> Rect {
    let (sw, sh) = (source.width, source.height);
    match stretch {
        Stretch::Fill => dest,
        Stretch::None => center_in(dest, source),
        Stretch::Uniform => {
            if sw <= 0.0 || sh <= 0.0 {
                return center_in(dest, source);
            }
            let s = (dest.size.width / sw).min(dest.size.height / sh).max(0.0);
            center_in(dest, Size::new(sw * s, sh * s))
        }
        Stretch::UniformToFill => {
            if sw <= 0.0 || sh <= 0.0 {
                return dest;
            }
            let s = (dest.size.width / sw)
                .max(dest.size.height / sh)
                .max(0.0);
            center_in(dest, Size::new(sw * s, sh * s))
        }
    }
}

fn center_in(dest: Rect, size: Size) -> Rect {
    Rect::new(
        dest.origin.x + (dest.size.width - size.width) / 2.0,
        dest.origin.y + (dest.size.height - size.height) / 2.0,
        size.width,
        size.height,
    )
}

/// 从 SVG 文本解析固有尺寸：优先 `width`/`height`（非百分比），否则 `viewBox` 的第 3/4 值。
fn svg_natural_size(path: &std::path::Path) -> Option<Size> {
    let data = std::fs::read_to_string(path).ok()?;
    let w = attr_number(&data, "width");
    let h = attr_number(&data, "height");
    if let (Some(w), Some(h)) = (w, h) {
        return Some(Size::new(w, h));
    }
    let vb = attr_str(&data, "viewBox")?;
    let nums: Vec<f32> = vb
        .split([' ', ',', '\t', '\n', '\r'])
        .filter_map(|t| t.parse::<f32>().ok())
        .collect();
    if nums.len() >= 4 {
        return Some(Size::new(nums[2], nums[3]));
    }
    None
}

/// 取属性值（`name="..."` 或 `name='...'`）；属性名前的字符须非标识符字符（避开 `stroke-width`）。
fn attr_str(data: &str, name: &str) -> Option<String> {
    let bytes = data.as_bytes();
    let name_bytes = name.as_bytes();
    let mut i = 0;
    while i + name_bytes.len() < bytes.len() {
        if &bytes[i..i + name_bytes.len()] == name_bytes {
            let before_ok = i == 0 || !is_ident(bytes[i - 1]);
            if before_ok {
                let mut j = i + name_bytes.len();
                while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                    j += 1;
                }
                if j < bytes.len() && bytes[j] == b'=' {
                    j += 1;
                    while j < bytes.len() && bytes[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
                        let quote = bytes[j];
                        let start = j + 1;
                        let end = bytes[start..].iter().position(|&b| b == quote)? + start;
                        return Some(data[start..end].to_string());
                    }
                }
            }
        }
        i += 1;
    }
    None
}

/// 取数值属性；百分比 / 解析失败 → None。
fn attr_number(data: &str, name: &str) -> Option<f32> {
    let v = attr_str(data, name)?;
    if v.trim().ends_with('%') {
        return None;
    }
    let head: String = v
        .trim()
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    head.parse().ok()
}

fn is_ident(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::{Align, LayoutProps};
    use crate::testing::TestHarness;
    use kanesumi_canvas::SceneCommand;
    use kanesumi_core::{Color, MetroTheme, ThemeColor};

    fn dest() -> Rect {
        Rect::new(10.0, 20.0, 100.0, 60.0)
    }

    #[test]
    fn stretch_fill_takes_dest_verbatim() {
        let r = fitted_rect(Size::new(20.0, 20.0), dest(), Stretch::Fill);
        assert_eq!(r, dest());
    }

    #[test]
    fn stretch_none_keeps_intrinsic_centered() {
        let src = Size::new(20.0, 30.0);
        let r = fitted_rect(src, dest(), Stretch::None);
        assert_eq!((r.size.width, r.size.height), (20.0, 30.0));
        assert_eq!(r.center(), dest().center());
    }

    #[test]
    fn stretch_uniform_letterboxes() {
        // 源 100×50 放进 100×60 → 取 min(1.0, 1.2)=1.0 → 100×50，上下居中。
        let r = fitted_rect(Size::new(100.0, 50.0), dest(), Stretch::Uniform);
        assert_eq!((r.size.width, r.size.height), (100.0, 50.0));
        assert!((r.origin.y - (20.0 + 5.0)).abs() < 1e-4);
    }

    #[test]
    fn stretch_uniform_to_fill_covers() {
        // 源 100×50 放进 100×60 → 取 max(1.0, 1.2)=1.2 → 120×60，左右越出。
        let r = fitted_rect(Size::new(100.0, 50.0), dest(), Stretch::UniformToFill);
        assert!((r.size.width - 120.0).abs() < 1e-3 && (r.size.height - 60.0).abs() < 1e-3);
        assert!(r.size.width > dest().size.width, "应覆盖并越出容器");
    }

    fn svg_24() -> Option<PathBuf> {
        let path = std::env::temp_dir().join("kanesumi_img_test_24.svg");
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                 <rect width="24" height="24" fill="#E57812"/></svg>"##;
        std::fs::write(&path, svg).ok()?;
        Some(path)
    }

    fn image_cmd(h: &TestHarness, id: crate::id::WidgetId) -> (u32, u32, Option<Color>) {
        h.tree
            .painted(id)
            .iter()
            .find_map(|c| match c {
                SceneCommand::Image { width, height, tint, .. } => {
                    Some((*width, *height, *tint))
                }
                _ => None,
            })
            .expect("应产生 Image 命令")
    }

    #[test]
    fn svg_rerasterizes_on_scale_change() {
        let Some(path) = svg_24() else { return };
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            Image::svg(&path).stretch(Stretch::Fill),
            LayoutProps {
                width: Some(30.0),
                height: Some(30.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(image_cmd(&h, id).0, 30, "scale=1 → 目标 30px 光栅");
        h.tree.edit::<Image, _>(id, |img, _| img.scale = 2.0);
        h.frame();
        assert_eq!(image_cmd(&h, id).0, 60, "scale=2 → 重光栅到 60px");
    }

    #[test]
    fn tint_resolves_theme_color() {
        let theme = MetroTheme::ether_dark();
        let icon = Icon {
            rgba: std::sync::Arc::from(vec![0u8; 4 * 4 * 4].into_boxed_slice()),
            width: 4,
            height: 4,
        };
        let mut h = TestHarness::with_theme(200.0, 200.0, theme);
        let id = h.tree.insert_with(
            h.root(),
            Image::raster(icon)
                .stretch(Stretch::Fill)
                .tint(ThemeColor::OnSurface),
            LayoutProps {
                width: Some(16.0),
                height: Some(16.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let tint = image_cmd(&h, id).2.expect("tint 应透传");
        assert_eq!(tint, theme.colors.on_surface, "令牌按当前主题解析");
    }

    #[test]
    fn explicit_size_overrides_intrinsic() {
        let icon = Icon {
            rgba: std::sync::Arc::from(Vec::<u8>::new().into_boxed_slice()),
            width: 64,
            height: 64,
        };
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            Image::raster(icon),
            LayoutProps {
                width: Some(12.0),
                height: Some(9.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(id).size, Size::new(12.0, 9.0));
    }
}
