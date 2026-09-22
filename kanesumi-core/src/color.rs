/// SRGBA 颜色类型。纯色、无渐变 —— 出处 `KANESUMI_DESIGN.md` §Ⅲ.1 铁律 6（L102）；
/// 参 `PLAN.md` §4-5。⚠ 旧注释写「参 SD §II」属错误溯源（SD.md 不含「渐变」），2026-09-22 更正。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    /// 0xRRGGBB / 0xRRGGBBAA 启发式构造（≤ `0x00FFFFFF` 视为 RGB，其它视为 RGBA）。
    ///
    /// **⚠ 阈值坑（V19）**：`0x0100_0000` 会被当 RGBA（几乎全透明黑），
    /// 无法表达"RGB=(1,0,0)"这种低值 RGB。要显式指定 alpha 时用 [`Color::from_rgba`]。
    pub const fn from_hex(hex: u32) -> Self {
        let (r, g, b, a) = match hex {
            0..=0x00FF_FFFF => (
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
                1.0,
            ),
            _ => (
                ((hex >> 24) & 0xFF) as f32 / 255.0,
                ((hex >> 16) & 0xFF) as f32 / 255.0,
                ((hex >> 8) & 0xFF) as f32 / 255.0,
                (hex & 0xFF) as f32 / 255.0,
            ),
        };
        Self { r, g, b, a }
    }

    /// 0xRRGGBBAA 显式构造（V19）—— 无 alpha 推断，全部 32 位按 RGBA 拆。
    /// 用它避免 [`from_hex`] 的阈值歧义（如 `0x0100_0000` 想表达"极暗红全不透明"时）。
    pub const fn from_rgba(hex: u32) -> Self {
        Self {
            r: ((hex >> 24) & 0xFF) as f32 / 255.0,
            g: ((hex >> 16) & 0xFF) as f32 / 255.0,
            b: ((hex >> 8) & 0xFF) as f32 / 255.0,
            a: (hex & 0xFF) as f32 / 255.0,
        }
    }

    pub const fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    /// 线性插值。t ∈ [0, 1]，越界自动夹紧。
    pub fn lerp(self, other: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0) as f32;
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub const TRANSPARENT: Color = Color::new(0.0, 0.0, 0.0, 0.0);
    pub const BLACK: Color = Color::new(0.0, 0.0, 0.0, 1.0);
    pub const WHITE: Color = Color::new(1.0, 1.0, 1.0, 1.0);

    /// WCAG 2.x 相对亮度（sRGB 线性化后按人眼灵敏度加权）。
    ///
    /// 用于「基色上该用黑字还是白字」的自动判定，以及令牌对比度自检 ——
    /// 手写死白/死黑正是「浅色主题下强调色上文字读不出」的根因。
    pub fn relative_luminance(self) -> f32 {
        fn linear(v: f32) -> f32 {
            let v = v.clamp(0.0, 1.0);
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        }
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// WCAG 对比度（1.0 ~ 21.0）。用于断言「文字与其背景不得同色」。
    ///
    /// 阈值参考：正文 ≥ 4.5，大字 / 图标 / 描边 ≥ 3.0。
    pub fn contrast_ratio(self, other: Color) -> f32 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        let (hi, lo) = if a >= b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// 给定底色上**可读**的前景：比较黑 / 白各自的对比度，取大者。
    ///
    /// **为什么必须有这个机制**：把前景写死成白（或黑），在另一半色域上必然读不出 ——
    /// 同一处代码要在暗色 `#FF99A4`（WinUI `SystemFillColorCritical`）与亮色 `#C42B1C`
    /// 两种语义块上都成立，写死任何一个都在另一态翻车（InfoBar 图标方块即此例：
    /// 白字画在暗色的 `#6CCB5F` 成功绿上只有 1.9:1）。
    ///
    /// 取大者保证**恒 ≥ 4.58:1**：两种判据的可行区间互补，覆盖全部色域
    /// （与 `Accent::on_accent` 同源判据，参 `CANON_VS_TEMPORARY.md` D1）。
    pub fn most_readable_on(bg: Color) -> Color {
        if Color::BLACK.contrast_ratio(bg) >= Color::WHITE.contrast_ratio(bg) {
            Color::BLACK
        } else {
            Color::WHITE
        }
    }

    /// HSV → RGB（ColorPicker Spectrum 等）。h/s/v ∈ [0,1]。
    pub fn hsv(h: f32, s: f32, v: f32) -> Color {
        let h = (h.fract() + 1.0) % 1.0;
        let i = (h * 6.0).floor() as i32;
        let f = h * 6.0 - i as f32;
        let p = v * (1.0 - s);
        let q = v * (1.0 - f * s);
        let t = v * (1.0 - (1.0 - f) * s);
        let (r, g, b) = match i % 6 {
            0 => (v, t, p),
            1 => (q, v, p),
            2 => (p, v, t),
            3 => (p, q, v),
            4 => (t, p, v),
            _ => (v, p, q),
        };
        Color::rgb(r, g, b)
    }
}

/// source-over 合成：`src` 覆盖在 `dst` 之上（仅测试用）。
///
/// **半透明令牌的对比度必须先合成再判** —— 直接拿未合成的 RGB 会得到假结论
/// （白 3% 会被当成纯白，于是「浅底叠白」也判成合格）。令牌自检统一走这里。
#[cfg(test)]
pub(crate) fn over(src: Color, dst: Color) -> Color {
    let a = src.a;
    Color::new(
        src.r * a + dst.r * (1.0 - a),
        src.g * a + dst.g * (1.0 - a),
        src.b * a + dst.b * (1.0 - a),
        1.0,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_rgb() {
        let c = Color::from_hex(0xE5_78_12);
        assert_eq!(c.r, 0xE5 as f32 / 255.0);
        assert_eq!(c.g, 0x78 as f32 / 255.0);
        assert_eq!(c.b, 0x12 as f32 / 255.0);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn hex_rgba() {
        let c = Color::from_hex(0xFF_FF_FF_1A);
        assert_eq!(c.a, 0x1A as f32 / 255.0);
    }

    #[test]
    fn lerp_clamps() {
        let a = Color::BLACK;
        let b = Color::WHITE;
        assert_eq!(a.lerp(b, 0.5).r, 0.5);
        assert_eq!(a.lerp(b, -1.0), a);
        assert_eq!(a.lerp(b, 2.0), b);
    }

    #[test]
    fn from_rgba_bypasses_hex_heuristic() {
        // V19：from_rgba 显式，不受 0x00FFFFFF 阈值影响
        let c = Color::from_rgba(0x0100_0000);
        // 期望：r = 1/255（极暗红），a = 0（透明）—— 明确 RGBA 解释
        assert!((c.r - 1.0 / 255.0).abs() < 1e-6);
        assert_eq!(c.g, 0.0);
        assert_eq!(c.b, 0.0);
        assert_eq!(c.a, 0.0);
        // from_hex 会走 RGBA 分支得到同样结果（此值恰好越阈值），但语义不明；
        // 而 0x00FF_FFFF 在 from_hex 会被 RGB 化：
        let opaque_white = Color::from_hex(0x00FF_FFFF);
        assert_eq!(opaque_white.a, 1.0);
        // from_rgba 同值则 alpha=0xFF 全不透明白
        let rgba_white = Color::from_rgba(0xFFFF_FFFF);
        assert_eq!(rgba_white.a, 1.0);
        assert_eq!(rgba_white.r, 1.0);
    }

    #[test]
    fn contrast_ratio_matches_wcag_known_values() {
        // WCAG 极值：黑白对比 = 21:1；同色 = 1:1。
        assert!((Color::WHITE.contrast_ratio(Color::BLACK) - 21.0).abs() < 0.01);
        assert!((Color::BLACK.contrast_ratio(Color::WHITE) - 21.0).abs() < 0.01, "对比度对称");
        assert!((Color::WHITE.contrast_ratio(Color::WHITE) - 1.0).abs() < 0.001);
    }

    #[test]
    fn relative_luminance_follows_eye_sensitivity() {
        let g = Color::rgb(0.0, 1.0, 0.0).relative_luminance();
        let r = Color::rgb(1.0, 0.0, 0.0).relative_luminance();
        let b = Color::rgb(0.0, 0.0, 1.0).relative_luminance();
        assert!(g > r && r > b, "人眼灵敏度 G > R > B，实际 g={g} r={r} b={b}");
    }

    /// 自动前景必须覆盖全色域且**恒**达正文阈值 —— 手写死白/死黑正是
    /// 「浅色语义块上字形读不出」的根因。数学下界是 √21 ≈ 4.58:1。
    #[test]
    fn most_readable_on_always_meets_text_contrast_across_gamut() {
        let mut checked = 0;
        for r in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            for g in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                for b in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    let bg = Color::rgb(r, g, b);
                    let fg = Color::most_readable_on(bg);
                    let ratio = fg.contrast_ratio(bg);
                    assert!(
                        ratio >= 4.58,
                        "底色 #{r}/{g}/{b} 的自动前景对比度仅 {ratio}，低于 4.58"
                    );
                    assert!(fg == Color::BLACK || fg == Color::WHITE, "只能取黑或白");
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 125);
    }

    #[test]
    fn most_readable_on_picks_by_contrast_not_by_threshold() {
        // 深底给白、浅底给黑。
        assert_eq!(Color::most_readable_on(Color::BLACK), Color::WHITE);
        assert_eq!(Color::most_readable_on(Color::WHITE), Color::BLACK);
        // 中等亮度（WinUI 暗色 critical 粉红 #FF99A4）上必须给黑字 ——
        // 这正是「写死白字」翻车的那种底色。
        let winui_critical_dark = Color::from_hex(0xFF_99_A4);
        assert_eq!(Color::most_readable_on(winui_critical_dark), Color::BLACK);
    }
}
