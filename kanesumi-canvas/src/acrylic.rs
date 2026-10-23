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

/// 生成背板图（直通 alpha，不透明）。`width × height` 是要铺满的**逻辑**尺寸；
/// 输出像素尺寸 = 该尺寸取整（1× 表面下 1:1 铺图，paint 不重采样）。
pub fn backdrop(src: &Icon, width: f32, height: f32, spec: &Acrylic) -> Icon {
    let target = (
        width.max(1.0).round() as u32,
        height.max(1.0).round() as u32,
    );
    backdrop_at(src, width, height, target, spec)
}

/// 目标**物理像素**尺寸版：返回图的像素尺寸 = `target`，绘制时按同尺寸铺开即 1:1 拷贝。
///
/// 为什么需要它（任务 lx2-backdrop）：整数 2× 输出下背板图层是 3072×1920，而模糊域只有
/// 768×480；若把升采样留给 paint，`CpuRenderer` 每帧要对 590 万像素做双线性（真机首开
/// `encode` 152 ms 的来源，参 Ether docs/CANVAS_PLAN.md §Ⅴ-1「paint 不做解码 / 重采样」）。
/// 升采样放在**工作线程**、随缓存只做一次：降采样域做模糊（省时），再升回目标尺寸。
pub fn backdrop_at(
    src: &Icon,
    width: f32,
    height: f32,
    target: (u32, u32),
    spec: &Acrylic,
) -> Icon {
    let (tw, th) = (width.max(1.0), height.max(1.0));
    let (ow_t, oh_t) = (target.0.max(1), target.1.max(1));
    // 1. 封面裁切：取与目标同宽高比、居中的源矩形（源像素坐标）。
    let (sw, sh) = (src.width.max(1) as f32, src.height.max(1) as f32);
    let scale = (tw / sw).max(th / sh);
    let (cw, ch) = (tw / scale, th / scale);
    let (cx, cy) = ((sw - cw) / 2.0, (sh - ch) / 2.0);
    // 2. 模糊域：模糊时目标 /4（盒式模糊代价与半径无关）；不模糊时取裁切区原生分辨率，
    //    但不超过目标（放大交给第 4 步的一次升采样）。
    let (bw, bh) = blur_domain(cw, ch, (ow_t, oh_t), spec);
    let mut px = area_resample(src, cx, cy, cw, ch, bw, bh);
    // 3. 模糊：半径从目标像素换算到模糊域像素。
    if spec.blur > 0.0 {
        let r = ((spec.blur * (bw as f32 / tw)).round()).max(1.0) as usize;
        let mut tmp = vec![0f32; px.len()];
        for _ in 0..3 {
            box_blur_h(&px, &mut tmp, bw as usize, bh as usize, r);
            box_blur_v(&tmp, &mut px, bw as usize, bh as usize, r);
        }
    }
    // 4. 着色（在模糊域做，线性混合与升采样可交换）→ 一次升采样到目标像素 + 打包。
    let t = [spec.tint.r, spec.tint.g, spec.tint.b];
    let k = spec.tint_opacity.clamp(0.0, 1.0);
    if k > 0.0 {
        for p in px.chunks_exact_mut(3) {
            for c in 0..3 {
                p[c] = (p[c] * (1.0 - k) + t[c] * k).clamp(0.0, 1.0);
            }
        }
    }
    let rgba = upsample_rgba(&px, bw, bh, ow_t, oh_t);
    Icon { rgba: Arc::from(rgba), width: ow_t, height: oh_t }
}

/// 模糊域尺寸：模糊时目标 /4；不模糊时取裁切区原生分辨率（不超过目标，放大只做一次）。
fn blur_domain(cw: f32, ch: f32, target: (u32, u32), spec: &Acrylic) -> (u32, u32) {
    if spec.blur > 0.0 {
        ((target.0 / 4).max(1), (target.1 / 4).max(1))
    } else {
        (
            (cw.round() as u32).min(target.0).max(1),
            (ch.round() as u32).min(target.1).max(1),
        )
    }
}

/// 双线性升采样到目标尺寸并打包直通 RGBA（alpha 恒 255）。
///
/// 与 `CpuRenderer` 的 `resample_bilinear` 同语义（sRGB 解码后线性插值、再编码），
/// 否则「worker 升采样」与「paint 升采样」会有肉眼可见的亮度差（验收要求 ≤ 2 级色差）。
fn upsample_rgba(src: &[f32], sw: u32, sh: u32, tw: u32, th: u32) -> Vec<u8> {
    // 先把模糊域的浮点值量化回 u8：paint 路径当年解码的就是这份 8 位结果，量化点必须一致。
    let quant: Vec<u8> = src.iter().map(|v| (v * 255.0).round() as u8).collect();
    let mut out = vec![255u8; (tw * th * 4) as usize];
    let inv_w = sw as f32 / tw as f32;
    let inv_h = sh as f32 / th as f32;
    for y in 0..th {
        let v = ((y as f32 + 0.5) * inv_h - 0.5).clamp(0.0, sh as f32 - 1.0);
        let v0 = v.floor() as u32;
        let v1 = (v0 + 1).min(sh - 1);
        let fv = v - v0 as f32;
        for x in 0..tw {
            let u = ((x as f32 + 0.5) * inv_w - 0.5).clamp(0.0, sw as f32 - 1.0);
            let u0 = u.floor() as u32;
            let u1 = (u0 + 1).min(sw - 1);
            let fu = u - u0 as f32;
            let o = ((y * tw + x) * 4) as usize;
            for c in 0..3 {
                let at = |sx: u32, sy: u32| {
                    let i = ((sy * sw + sx) * 3 + c) as usize;
                    srgb_decode_byte(quant[i])
                };
                let l = at(u0, v0) * (1.0 - fu) * (1.0 - fv)
                    + at(u1, v0) * fu * (1.0 - fv)
                    + at(u0, v1) * (1.0 - fu) * fv
                    + at(u1, v1) * fu * fv;
                out[o + c as usize] = (srgb_encode(l) * 255.0 + 0.5) as u8;
            }
        }
    }
    out
}

/// sRGB 8 位 → 线性 0..1（与 `kanesumi-render` 同公式）。
fn srgb_decode_byte(v: u8) -> f32 {
    let c = v as f32 / 255.0;
    if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
}

/// 线性 0..1 → sRGB 0..1（与 `kanesumi-render` 同公式）。
fn srgb_encode(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.0031308 { c * 12.92 } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
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
        assert_eq!((out.width, out.height), (256, 256), "输出尺寸 = 目标尺寸（1:1 铺图）");
        assert!(variance(&out) < variance(&src) * 0.05, "棋盘被抹平");
    }

    /// 模糊仍在降采样域做（任务 lx2-backdrop §1）：2× 目标 512×512 时模糊域是 128×128。
    #[test]
    fn blur_runs_in_downsampled_domain_then_upsamples_to_target() {
        let src = checker(1024, 1024);
        let spec = Acrylic { blur: 30.0, tint: Color::BLACK, tint_opacity: 0.0 };
        assert_eq!(blur_domain(512.0, 512.0, (512, 512), &spec), (128, 128));
        // 物理 2× 的目标（1024×1024）同样是 1/4 模糊域，输出回到 1024×1024。
        assert_eq!(blur_domain(512.0, 512.0, (1024, 1024), &spec), (256, 256));
        let out = backdrop_at(&src, 512.0, 512.0, (1024, 1024), &spec);
        assert_eq!((out.width, out.height), (1024, 1024));
        // 升采样后仍平滑：相邻像素差远小于 1/4 分辨率直接糊上去的块状边界。
        let row: Vec<i32> = (0..out.width as usize)
            .map(|x| out.rgba[(512 * out.width as usize + x) * 4] as i32)
            .collect();
        assert!(row.windows(2).all(|w| (w[1] - w[0]).abs() <= 3), "升采样后无块状接缝");
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
