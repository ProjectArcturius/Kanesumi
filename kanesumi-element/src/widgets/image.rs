// Image —— 位图 / 图标元素（XAML Image + Stretch）。
//
// 持已光栅化的 `Icon`（RGBA）或 SVG 路径：**解码不进 paint**（参 docs/CANVAS_PLAN.md §Ⅴ 规则 1）。
// SVG 源一律走 Kanesumi 统一解码服务（`kanesumi_canvas::decode`，后台线程池 + 按字节 LRU）：
// paint 只查缓存 —— 命中就画；未命中则提交请求、本帧什么都不画（**尺寸照旧占位**，不跳），
// 并登记一个短定时器；就绪后 `update` 经元素树既有失效机制令本节点重画。失败结果被服务记住，
// 不反复重试（缺失图标不会每帧读盘）。对照 Windows：WIC / XAML `BitmapImage` 异步解码。
//
// `Stretch` 取 XAML 四值语义，目标矩形用 `fitted_rect` 纯函数算出，便于单测。
//
// 无子节点；`hit_test` 沿用默认（矩形内命中），与 Label 一致。
//
// 不透明度：`opacity`（`None` = 1.0）透传 `Scene::image_with_opacity`。用 `Option` 而非 `f32`：
// 本类型派生 `Default`，裸 `f32` 会默认成 0 → 整图不可见。

use std::path::PathBuf;

use kanesumi_canvas::decode::{self, DecodeKey, Peek};
use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::Scene;
use kanesumi_core::{Brush, Rect, Size};

use crate::widget::{MeasureCtx, PaintCtx, UpdateCtx, Widget};

/// 占位期间查询就绪的间隔（秒）。服务是下拉式的，元素树用既有定时器机制轮询：
/// 等待期间不占帧（`next_timer` 给外壳唤醒点），就绪即重画。
const DECODE_POLL_SECS: f64 = 1.0 / 60.0;

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
    /// SVG 光栅的显示缩放（逻辑 px → 目标像素），默认 1.0。
    pub scale: f32,
    /// 整图不透明度（0..1；`None` = 1.0）。
    pub opacity: Option<f32>,
    /// 在途解码键与目标像素（paint 提交时登记；就绪后由 `update` 消费并清空）。
    pending: Option<(DecodeKey, u32)>,
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
            pending: None,
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

    /// SVG 源在目标矩形下的光栅像素边长（最长边 × `scale`）。`Raster` 源无需解码 → `None`。
    fn target_px(&self, dest: Rect) -> Option<u32> {
        match &self.source {
            ImageSource::Raster(_) => None,
            ImageSource::Svg { .. } => Some(
                ((dest.size.width.max(dest.size.height)) * self.scale.max(0.01))
                    .round()
                    .max(1.0) as u32,
            ),
        }
    }

    /// 预热：按**声明尺寸**（显式宽高，否则固有尺寸）× `scale` 算目标像素，提交后台解码。
    ///
    /// 界面显现前调用（Launcher 升起前预热磁贴图标，参 docs/CANVAS_PLAN.md §Ⅳ C3）；
    /// 命中缓存即忽略，重复调用无代价。返回 false 表示本源无需解码（已是位图）。
    pub fn prefetch(&self) -> bool {
        let ImageSource::Svg { path, .. } = &self.source else {
            return false;
        };
        let intrinsic = self.intrinsic_size();
        let logical = Size::new(
            self.width.unwrap_or(intrinsic.width),
            self.height.unwrap_or(intrinsic.height),
        );
        let px = (logical.width.max(logical.height) * self.scale.max(0.01))
            .round()
            .max(1.0) as u32;
        decode::global().request_svg_longest(path.clone(), px);
        true
    }
}

impl Widget for Image {
    /// 显式宽高优先，否则图源固有尺寸。**占位不改变量测结果**：尺寸只由图源固有尺寸与
    /// 显式宽高决定，与解码是否就绪无关（参 §Ⅴ 规则 1「先占好尺寸」）。
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
        // 取图：**只查缓存**，绝不在 paint 里解码 / 读盘。
        let mut requested: Option<(DecodeKey, u32)> = None;
        let icon: Option<Icon> = match &self.source {
            ImageSource::Raster(i) => Some(i.clone()),
            ImageSource::Svg { path, .. } => {
                let px = self.target_px(dest).unwrap_or(1);
                let key = DecodeKey::svg_longest(path, px);
                match decode::global().peek(&key) {
                    Peek::Ready(out) => Some(out.icon.clone()),
                    // 已确认失败：保持占位，不反复重试（服务已记住负面结果）。
                    Peek::Failed => None,
                    // 未缓存：提交后台请求（只入队，不在此解码），本帧留占位。
                    Peek::Missing => {
                        decode::global().request_svg_longest(path.clone(), px);
                        requested = Some((key, px));
                        None
                    }
                    Peek::Pending => {
                        requested = Some((key, px));
                        None
                    }
                }
            }
        };
        let Some(icon) = icon else {
            // 未就绪：本帧什么都不画（尺寸已由 measure 占好，不跳）。
            // 登记等待解码键；若已安装钩子则由钩子唤醒，未安装则退回短定时器轮询（参 SMOOTHNESS_PLAN §Ⅲ-3、裁定 §122）。
            self.pending = requested;
            if let Some((ref key, _)) = self.pending {
                ctx.tree.wait_for_decode(ctx.id, key.clone());
            }
            if !decode::has_ready_hook() {
                ctx.request_timer(DECODE_POLL_SECS);
            }
            return;
        };
        self.pending = None;
        let fitted = fitted_rect(intrinsic, dest, stretch);
        // 裁剪到自身矩形：UniformToFill / None 的目标矩形可越出容器。
        scene.push_clip(dest);
        scene.image_with_opacity(&icon, fitted, tint, opacity);
        scene.pop_clip();
    }

    /// 就绪轮询：解码好了就令自己重画（元素树既有失效机制）；没好继续等一个间隔（无钩子退回路径）。
    fn update(&mut self, ctx: &mut UpdateCtx, _dt: f64) {
        let Some((key, px)) = self.pending.clone() else {
            return;
        };
        match decode::global().peek(&key) {
            Peek::Ready(_) | Peek::Failed => {
                self.pending = None;
                ctx.invalidate_paint();
            }
            Peek::Pending => {
                if !decode::has_ready_hook() {
                    ctx.request_timer(DECODE_POLL_SECS);
                }
            }
            // 缓存被清（主题切换 / 显式 clear）→ 重新提交，别永远等下去。
            Peek::Missing => {
                if let ImageSource::Svg { path, .. } = &self.source {
                    decode::global().request_svg_longest(path.clone(), px);
                }
                if !decode::has_ready_hook() {
                    ctx.request_timer(DECODE_POLL_SECS);
                }
            }
        }
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

    static HOOK_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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

    /// SVG 光栅像素随 `scale` 变化（scope 改变 → 服务按新像素出图，2× 下不糊）。
    #[test]
    fn svg_rerasterizes_on_scale_change() {
        let _lock = HOOK_TEST_LOCK.lock().unwrap();
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
        h.wait_decodes();
        assert_eq!(image_cmd(&h, id).0, 30, "scale=1 → 目标 30px 光栅");
        h.tree.edit::<Image, _>(id, |img, _| img.scale = 2.0);
        h.frame();
        h.wait_decodes();
        assert_eq!(image_cmd(&h, id).0, 60, "scale=2 → 重光栅到 60px");
    }

    /// 占位：解码就绪之前不画图，**但尺寸照旧占好**（measure 与解码无关，不跳）。
    #[test]
    fn placeholder_paints_nothing_but_keeps_measure() {
        let _lock = HOOK_TEST_LOCK.lock().unwrap();
        let Some(path) = svg_24() else { return };
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            Image::svg(&path),
            LayoutProps {
                width: Some(40.0),
                height: Some(24.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        // 首帧：请求已提交、结果未到 → 无图像命令，但矩形已按显式尺寸占好。
        let _ = kanesumi_canvas::decode::global().poll_ready();
        h.frame();
        assert_eq!(h.rect(id).size, Size::new(40.0, 24.0), "占位不改变量测结果");
        if kanesumi_canvas::decode::global().pending_count() > 0 {
            assert!(
                h.tree
                    .painted(id)
                    .iter()
                    .all(|c| !matches!(c, SceneCommand::Image { .. })),
                "未就绪时不得画图"
            );
        }
        // 就绪后同尺寸仍不变。
        h.wait_decodes();
        assert_eq!(h.rect(id).size, Size::new(40.0, 24.0));
    }

    /// 就绪后节点经元素树既有失效机制重画（定时器 → update → invalidate_paint）。
    #[test]
    fn node_repaints_after_decode_ready() {
        let _lock = HOOK_TEST_LOCK.lock().unwrap();
        let Some(path) = svg_24() else { return };
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            Image::svg(&path).stretch(Stretch::Fill),
            LayoutProps {
                width: Some(24.0),
                height: Some(24.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        // 后台完成 → 定时器到期那一帧必须已经画出图。
        let svc = kanesumi_canvas::decode::global();
        assert!(svc.wait_idle(std::time::Duration::from_secs(10)));
        h.frame();
        let (w, hgt, _) = image_cmd(&h, id);
        assert_eq!((w, hgt), (24, 24), "就绪后应重画并出 24pt 图");
    }

    /// `prefetch`：SVG 源提交后台解码（不阻塞），随后首帧 paint 直接命中。
    #[test]
    fn prefetch_makes_first_paint_a_cache_hit() {
        let Some(path) = svg_24() else { return };
        let img = Image::svg(&path).size(24.0, 24.0);
        assert!(img.prefetch(), "SVG 源应可预热");
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            img,
            LayoutProps {
                width: Some(24.0),
                height: Some(24.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.wait_decodes();
        assert_eq!(image_cmd(&h, id).0, 24);
        // 位图源无需解码 → 返回 false。
        let icon = Icon {
            rgba: std::sync::Arc::from(vec![0u8; 4].into_boxed_slice()),
            width: 1,
            height: 1,
        };
        assert!(!Image::raster(icon).prefetch());
    }

    /// 缺失 SVG：服务记住失败，反复出帧不反复读盘（每次都保持占位、不 panic）。
    #[test]
    fn missing_svg_keeps_placeholder_without_retry() {
        let mut h = TestHarness::new(200.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            Image::svg("/nonexistent/kanesumi-c3-missing.svg"),
            LayoutProps {
                width: Some(16.0),
                height: Some(16.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.wait_decodes();
        h.frame();
        h.frame();
        assert!(
            h.tree
                .painted(id)
                .iter()
                .all(|c| !matches!(c, SceneCommand::Image { .. })),
            "缺失图标不得画出任何图"
        );
        assert_eq!(h.rect(id).size, Size::new(16.0, 16.0));
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

    /// 解码就绪钩子触发：只失效等待该键的节点（参 SMOOTHNESS_PLAN §Ⅲ-3、裁定 §122）。
    #[test]
    fn hook_triggers_invalidation_only_for_waiting_node() {
        struct HookGuard;
        impl Drop for HookGuard {
            fn drop(&mut self) {
                decode::clear_ready_hook();
            }
        }

        let _lock = HOOK_TEST_LOCK.lock().unwrap();
        decode::set_ready_hook(Box::new(|| {}));
        let _guard = HookGuard;

        let path1 = std::env::temp_dir().join("kanesumi_hook_test_1.svg");
        let path2 = std::env::temp_dir().join("kanesumi_hook_test_2.svg");
        let _ = std::fs::write(&path1, r#"<svg width="20" height="20"></svg>"#);
        let _ = std::fs::write(&path2, r#"<svg width="20" height="20"></svg>"#);

        let mut h = TestHarness::new(200.0, 200.0);
        let props = LayoutProps {
            width: Some(20.0),
            height: Some(20.0),
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        };
        let id1 = h.tree.insert_with(
            h.root(),
            Image::svg(&path1).size(20.0, 20.0),
            props,
        );
        let id2 = h.tree.insert_with(
            h.root(),
            Image::svg(&path2).size(20.0, 20.0),
            props,
        );

        // 渲染首帧：两个 Image 都未就绪；有钩子安装时，不得设置轮询定时器。
        h.frame();
        assert!(h.tree.next_timer().is_none(), "安装钩子后不得设置轮询定时器");
        assert!(!h.tree.is_paint_dirty(id1));
        assert!(!h.tree.is_paint_dirty(id2));

        // 仅通知 key1 就绪：只有 id1 被标记待绘，id2 保持不动。
        let key1 = DecodeKey::svg_longest(&path1, 20);
        h.tree.on_decode_ready(&[key1]);
        assert!(h.tree.is_paint_dirty(id1), "等待该键的节点必须失效");
        assert!(!h.tree.is_paint_dirty(id2), "未等待该键的节点不得失效");

        let _ = std::fs::remove_file(&path1);
        let _ = std::fs::remove_file(&path2);
    }

    /// 无钩子退回路径：未安装钩子时退回既有定时器轮询（参 SMOOTHNESS_PLAN §Ⅲ-3）。
    #[test]
    fn fallback_to_timer_when_no_hook() {
        let _lock = HOOK_TEST_LOCK.lock().unwrap();
        decode::clear_ready_hook();
        let path = std::env::temp_dir().join("kanesumi_fallback_timer.svg");
        let _ = std::fs::write(&path, r#"<svg width="20" height="20"></svg>"#);

        let mut h = TestHarness::new(200.0, 200.0);
        let _id = h.tree.insert_with(
            h.root(),
            Image::svg(&path).size(20.0, 20.0),
            LayoutProps::default(),
        );
        h.frame();

        // 无钩子时必须登记定时器轮询
        assert!(h.tree.next_timer().is_some(), "无钩子时必须退回定时器");

        let _ = std::fs::remove_file(&path);
    }
}
