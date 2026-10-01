// acrylic.rs —— 亚克力背板：壁纸 → 等比裁满 → 模糊 → 主题着色。参 Ether docs/LAUNCHER_REBUILD.md §Ⅲ-bis 4。
//
// 用户裁定（2026-10-01）：Launcher 背景 = 用户桌面壁纸 + 可调模糊度的亚克力板，允许完全清晰。
// 只需要模糊**静态壁纸**（不是窗口实时内容），所以在客户端一次算好、壁纸或参数变了才重算，
// 不需要合成器的实时背景模糊协议，任何合成器下都一样。
//
// 做法：先按「封面」裁成目标宽高比；模糊时在 1/4 分辨率上做三遍盒式模糊（≈ 高斯，代价与半径无关），
// 交给渲染器双线性放大（模糊后的图放大不显块）；再按主题色混合（UWP 亚克力的 tint 层）。
// 噪点层暂不做：低分辨率上的噪点放大后成斑块，要做应在全分辨率另叠一层（登记为后续）。

use std::sync::Arc;

use kanesumi_core::{Color, ColorScheme, MetroTheme};

use crate::icon::Icon;

/// 亚克力参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Acrylic {
    /// 模糊半径（逻辑像素，约等于高斯 σ×2）。0 = 完全清晰。
    pub blur: f32,
    /// 着色（通常取主题背景色）。
    pub tint: Color,
    /// 着色强度 0..1（0 = 原图，1 = 纯色）。
    pub tint_opacity: f32,
}

impl Acrylic {
    /// 主题背板：着色取主题背景色；`blur` / `tint_opacity` 来自用户设置（Chorus theme.toml
    /// `acrylic_blur` / `acrylic_tint`），`tint_opacity = 0` 且 `blur = 0` 即原壁纸。
    pub fn for_theme(theme: &MetroTheme, blur: f32, tint_opacity: f32) -> Self {
        Self { blur: blur.max(0.0), tint: theme.colors.background, tint_opacity: tint_opacity.clamp(0.0, 1.0) }
    }

    /// 无用户设置时的主题默认着色强度（深色压得更重，保证白字可读）。
    pub fn default_tint(scheme: ColorScheme) -> f32 {
        match scheme {
            ColorScheme::Dark => 0.55,
            ColorScheme::Light => 0.45,
        }
    }
}

/// 生成背板图（直通 alpha，不透明）。`width × height` 是要铺满的逻辑尺寸；
/// 返回图的像素尺寸可能小于目标（模糊时降采样），绘制时按目标矩形拉伸即可。
pub fn backdrop(src: &Icon, width: f32, height: f32, spec: &Acrylic) -> Icon {
    let (tw, th) = (width.max(1.0), height.max(1.0));
    // 1. 封面裁切：取与目标同宽高比、居中的源矩形。
    let (sw, sh) = (src.width.max(1) as f32, src.height.max(1) as f32);
    let scale = (tw / sw).max(th / sh);
    let (cw, ch) = (tw / scale, th / scale);
    let (cx, cy) = ((sw - cw) / 2.0, (sh - ch) / 2.0);
    // 2. 输出分辨率：模糊时 1/4，否则不超过源裁切区分辨率（不放大）。
    let down = if spec.blur > 0.0 { 4.0 } else { 1.0 };
    let ow = ((tw / down).min(cw).round() as u32).max(1);
    let oh = ((th / down).min(ch).round() as u32).max(1);
    let mut px = area_resample(src, cx, cy, cw, ch, ow, oh);
    // 3. 模糊：半径换算到输出像素。
    if spec.blur > 0.0 {
        let r = ((spec.blur / down) * (ow as f32 / (tw / down))).round().max(1.0) as usize;
        let mut tmp = vec![0f32; px.len()];
        for _ in 0..3 {
            box_blur_h(&px, &mut tmp, ow as usize, oh as usize, r);
            box_blur_v(&tmp, &mut px, ow as usize, oh as usize, r);
        }
    }
    // 4. 着色 + 打包。
    let t = [spec.tint.r, spec.tint.g, spec.tint.b];
    let k = spec.tint_opacity.clamp(0.0, 1.0);
    let mut rgba = Vec::with_capacity((ow * oh * 4) as usize);
    for p in px.chunks_exact(3) {
        for c in 0..3 {
            let v = p[c] * (1.0 - k) + t[c] * k;
            rgba.push((v.clamp(0.0, 1.0) * 255.0).round() as u8);
        }
        rgba.push(255);
    }
    Icon { rgba: Arc::from(rgba), width: ow, height: oh }
}

/// 面积平均重采样（降采样不混叠）。输出 RGB f32（0..1，sRGB 值域；alpha 视作不透明）。
fn area_resample(src: &Icon, x0: f32, y0: f32, w: f32, h: f32, ow: u32, oh: u32) -> Vec<f32> {
    let (sw, sh) = (src.width as usize, src.height as usize);
    let mut out = vec![0f32; (ow * oh * 3) as usize];
    let fx = w / ow as f32;
    let fy = h / oh as f32;
    for oy in 0..oh as usize {
        let ya = (y0 + oy as f32 * fy).floor().max(0.0) as usize;
        let yb = ((y0 + (oy + 1) as f32 * fy).ceil() as usize).clamp(ya + 1, sh);
        for ox in 0..ow as usize {
            let xa = (x0 + ox as f32 * fx).floor().max(0.0) as usize;
            let xb = ((x0 + (ox + 1) as f32 * fx).ceil() as usize).clamp(xa + 1, sw);
            let mut acc = [0f32; 3];
            let mut n = 0f32;
            for y in ya.min(sh - 1)..yb {
                let row = y * sw * 4;
                for x in xa.min(sw - 1)..xb {
                    let i = row + x * 4;
                    acc[0] += src.rgba[i] as f32;
                    acc[1] += src.rgba[i + 1] as f32;
                    acc[2] += src.rgba[i + 2] as f32;
                    n += 1.0;
                }
            }
            let o = (oy * ow as usize + ox) * 3;
            let n = n.max(1.0) * 255.0;
            out[o] = acc[0] / n;
            out[o + 1] = acc[1] / n;
            out[o + 2] = acc[2] / n;
        }
    }
    out
}

/// 水平盒式模糊（滑动窗口，边缘钳制）。
fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = y * w * 3;
        let at = |x: isize, c: usize| src[row + (x.clamp(0, w as isize - 1) as usize) * 3 + c];
        for c in 0..3 {
            let mut sum: f32 = (-(r as isize)..=r as isize).map(|x| at(x, c)).sum();
            for x in 0..w {
                dst[row + x * 3 + c] = sum * norm;
                sum += at(x as isize + r as isize + 1, c) - at(x as isize - r as isize, c);
            }
        }
    }
}

/// 垂直盒式模糊。
fn box_blur_v(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    let norm = 1.0 / (2 * r + 1) as f32;
    for x in 0..w {
        let at = |y: isize, c: usize| src[((y.clamp(0, h as isize - 1) as usize) * w + x) * 3 + c];
        for c in 0..3 {
            let mut sum: f32 = (-(r as isize)..=r as isize).map(|y| at(y, c)).sum();
            for y in 0..h {
                dst[(y * w + x) * 3 + c] = sum * norm;
                sum += at(y as isize + r as isize + 1, c) - at(y as isize - r as isize, c);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 黑白棋盘（8px 格）。
    fn checker(w: u32, h: u32) -> Icon {
        let mut v = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let on = ((x / 8) + (y / 8)) % 2 == 0;
                let c = if on { 255 } else { 0 };
                v.extend_from_slice(&[c, c, c, 255]);
            }
        }
        Icon { rgba: Arc::from(v), width: w, height: h }
    }

    fn variance(i: &Icon) -> f32 {
        let vals: Vec<f32> = i.rgba.chunks_exact(4).map(|p| p[0] as f32).collect();
        let m = vals.iter().sum::<f32>() / vals.len() as f32;
        vals.iter().map(|v| (v - m).powi(2)).sum::<f32>() / vals.len() as f32
    }

    #[test]
    fn zero_blur_is_clear_and_untinted_when_opacity_zero() {
        let src = checker(64, 64);
        let spec = Acrylic { blur: 0.0, tint: Color::BLACK, tint_opacity: 0.0 };
        let out = backdrop(&src, 64.0, 64.0, &spec);
        assert_eq!((out.width, out.height), (64, 64));
        assert_eq!(&out.rgba[..], &src.rgba[..], "模糊 0 + 不着色 = 原图");
    }

    #[test]
    fn blur_smooths_and_downsamples() {
        let src = checker(256, 256);
        let spec = Acrylic { blur: 40.0, tint: Color::BLACK, tint_opacity: 0.0 };
        let out = backdrop(&src, 256.0, 256.0, &spec);
        assert!(out.width <= 64 && out.height <= 64, "模糊时 1/4 降采样");
        assert!(variance(&out) < variance(&src) * 0.05, "棋盘被抹平");
    }

    #[test]
    fn tint_pulls_toward_color_and_cover_fit_keeps_aspect() {
        let src = checker(400, 100); // 宽图
        let spec = Acrylic { blur: 0.0, tint: Color::BLACK, tint_opacity: 1.0 };
        let out = backdrop(&src, 100.0, 100.0, &spec);
        assert_eq!((out.width, out.height), (100, 100), "方形目标裁成方形");
        assert!(out.rgba.chunks_exact(4).all(|p| p[0] == 0), "全着色 = 纯色");
    }
}
