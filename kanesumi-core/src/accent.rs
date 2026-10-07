// accent.rs —— 强调色系统（Kanesumi Design §Ⅲ.3「深底 + 单一强调色」的机制层）。
//
// **真源分工**（这是本节的关键）：
// - **配置真源**：Ether 的 Chorus（`chorus/src/theme.rs`）拥有主题状态，
//   落盘于 `~/.config/ether/theme.toml` 的 `accent`（RRGGBB hex）与 `scheme`（dark/light）。
// - **渲染真源**：Kanesumi 把**一个基色**派生为完整色阶，供全部控件消费；
//   控件不得再出现任何写死的强调色。
//
// 该派生此前只存在于 Chorus，而 Kanesumi 侧用写死的橙色（`#E57812`）顶替 ——
// 于是用户在 Chorus 里把 accent 改成青绿（`#00897B`），全部应用仍然显示橙色。
// 这就是「临时方案被当成设计」的典型：机制缺失被误读为「Kanesumi 就是橙色」。
//
// 档位改用 Win10 的**HSV 明度 V 缩放**模型（2026-10-07 落地 `docs/CANON_VS_TEMPORARY.md` T20 ①，
// 取代旧的 RGB 向白/黑 lerp）：Light1/2/3 = V×1.17 / ×1.56 / ×1.925，Dark1/2/3 = V×0.905 / ×0.689 / ×0.49。
// 该模型对 `#0078D4` 的**明度（最大分量）与下限分量**与 OS 真值逐档精确吻合
// （OS：Light1 `#0091F8` / Dark1 `#0067C0` / Dark2 `#003E92` / Dark3 `#001A68`）。
//
// **已知边界**：OS 生成色阶时**还会调整色相**（微软声明该算法闭源、非纯明度缩放），
// 故**中间分量**（蓝档的 G）与真值有偏差；本实现保持色相不变，只承诺明度 / 下限分量对齐。
// 实测数据与探针脚本：`docs/WINDOWS_RESEARCH_BACKLOG.md` §B4；偏差登记于 T20。
//
// 派生参数与 Chorus `derive_accent()` 保持一致（同源算法，两处必须同步修改）。

use crate::color::Color;

/// 颜色方案 —— 仅 dark/light 二态（参 `KANESUMI_DESIGN.md` §Ⅲ.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ColorScheme {
    Dark,
    #[default]
    Light,
}

impl ColorScheme {
    pub const fn is_dark(self) -> bool {
        matches!(self, ColorScheme::Dark)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ColorScheme::Dark => "dark",
            ColorScheme::Light => "light",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "dark" => Some(ColorScheme::Dark),
            "light" => Some(ColorScheme::Light),
            _ => None,
        }
    }
}

/// 强调色色阶 —— 由单个基色派生，是「一屏一色」的全部来源。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Accent {
    /// 基色（用户 / 系统所选）。
    pub base: Color,
    /// 向白阶梯（依次更亮）。
    pub light1: Color,
    pub light2: Color,
    pub light3: Color,
    /// 向黑阶梯（依次更暗）。
    pub dark1: Color,
    pub dark2: Color,
    pub dark3: Color,
    /// 基色之上的前景：按 WCAG 相对亮度自动取黑或白。
    pub on_accent: Color,
}

/// HSV 明度缩放系数（Light1/2/3）。与 Win10 `UISettings` 真值反推所得一致，与 Chorus 同值。
const LIGHT_FACTORS: [f32; 3] = [1.17, 1.56, 1.925];
/// HSV 明度缩放系数（Dark1/2/3）。与 Win10 `UISettings` 真值反推所得一致，与 Chorus 同值。
const DARK_FACTORS: [f32; 3] = [0.905, 0.689, 0.49];

/// RGB → HSV（h/s/v ∈ [0,1]）。派生内部用，避免在 `color.rs` 暴露第二套转换。
fn rgb_to_hsv(c: Color) -> (f32, f32, f32) {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    let d = max - min;
    let v = max;
    let s = if max <= 0.0 { 0.0 } else { d / max };
    let h = if d <= f32::EPSILON {
        0.0
    } else if max == c.r {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if max == c.g {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    } / 6.0;
    (h, s, v)
}

/// 单档明度缩放：V' = V×factor。V 越界（> 1）时按 Win10「V 饱和 + 饱和度下降」：
/// S' = S × (2 − V')，V 夹到 1。
///
/// S × (2 − V') 的推导：以 `#0078D4`（S=1）的两个越界点反推 —— Light2 真值 R=76
/// 要求 1 − S' = 76/255 → S' = 0.702 = 2 − 1.297（V'=×1.56）；Light3 真值 R=153
/// 要求 S' = 0.400 = 2 − 1.601（V'=×1.925），两点都精确落在 `2 − V'` 上。
/// 该式等价于对饱和色（S=1）直接缩放 HSL 明度 L，是无量纲的对称形式。
fn scale_value(base: Color, factor: f32) -> Color {
    let (h, s, v) = rgb_to_hsv(base);
    let scaled = v * factor;
    if scaled <= 1.0 {
        Color::hsv(h, s, scaled)
    } else {
        Color::hsv(h, (s * (2.0 - scaled)).clamp(0.0, 1.0), 1.0)
    }
}

impl Accent {
    /// Ether 默认强调色（暮蓝，裁定 `docs/DECISIONS_2026-10-04.md` §M-87）。
    /// 同时也是 `theme.toml` 缺失时的回退值。
    pub const DEFAULT_HEX: u32 = 0x2D_6F_E0;

    /// 由基色派生全阶（HSV 明度缩放，见文件头）。
    pub fn from_base(base: Color) -> Self {
        Self {
            base,
            light1: scale_value(base, LIGHT_FACTORS[0]),
            light2: scale_value(base, LIGHT_FACTORS[1]),
            light3: scale_value(base, LIGHT_FACTORS[2]),
            dark1: scale_value(base, DARK_FACTORS[0]),
            dark2: scale_value(base, DARK_FACTORS[1]),
            dark3: scale_value(base, DARK_FACTORS[2]),
            // 前景不按「相对亮度 > 0.5」这类固定阈值判（Chorus `derive_accent()` 现用此法），
            // 而是**直接比较黑 / 白各自的 WCAG 对比度、取大者**：
            // 固定阈值在中等亮度 accent 上会不达标 —— 例如青绿 #00897B 的亮度 0.19 < 0.5
            // 会判给白字，而白字在其上仅 4.36:1（低于正文阈值 4.5）。
            // 取大者则恒有 max(黑, 白) ≥ 4.58（两种判据的可行区间互补，覆盖全部亮度）。
            // 该判据已抽成 `Color::most_readable_on`，语义色块上的字形共用同一机制，
            // 避免第二处「写死白字」重演。这是对 Chorus 的**有意偏离**，
            // 理由登记于 docs/CANON_VS_TEMPORARY.md，Chorus 侧应对齐。
            on_accent: Color::most_readable_on(base),
        }
    }

    /// 由 `0xRRGGBB` 构造。
    pub fn from_hex(hex: u32) -> Self {
        Self::from_base(Color::from_hex(hex))
    }

    /// 由 `RRGGBB` 十六进制串构造（Chorus `theme.toml` 的格式）。非法输入回退默认。
    pub fn parse(hex: &str) -> Self {
        let hex = hex.trim().trim_start_matches('#');
        if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            match u32::from_str_radix(hex, 16) {
                Ok(v) => return Self::from_hex(v),
                Err(_) => {}
            }
        }
        Self::default()
    }

    /// 该方案使用的主强调色。
    ///
    /// 亮底直接用基色；暗底改用派生**亮档 Light1** —— 基色落在深底上偏暗，
    /// 人们期望图与预览页里深色方案的暮蓝是更亮的 `#4A86F0` 一档（四份工人报告一致指出）。
    /// 经比对：暮蓝 `#2D6FE0` 的 Light1 = `#3982FF`、Light2 = `#7FAEFF`，前者更接近期望。
    pub const fn primary_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light1,
            ColorScheme::Light => self.base,
        }
    }

    /// 主强调色之上的前景 —— 由 [`Color::most_readable_on`] 按对比度自动取黑 / 白，
    /// 而非写死；暗色主档换成 Light1 后仍走同一机制，保证换档不改变可读性保证。
    pub fn on_primary_for(self, scheme: ColorScheme) -> Color {
        Color::most_readable_on(self.primary_for(scheme))
    }

    /// 该方案下的悬停强调色（按钮等强调面）。
    ///
    /// 反馈方向与主档一致（都比主档更「亮眼」）：暗色主档已是 Light1，悬停再上 Light2；
    /// 亮色主档是基色，悬停压暗到 Dark1。两种方案下「悬停 = 朝远离背景的方向再走一档」。
    pub const fn hover_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light2,
            ColorScheme::Light => self.dark1,
        }
    }

    /// 按下态强调色（比悬停更进一档，方向同悬停）。
    pub const fn pressed_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light3,
            ColorScheme::Light => self.dark2,
        }
    }

    /// 焦点描边 —— 需在两种方案下都清楚可辨，故暗底取更亮的档、亮底取更暗的档。
    pub const fn focus_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light2,
            ColorScheme::Light => self.dark1,
        }
    }
}

impl Default for Accent {
    fn default() -> Self {
        Self::from_hex(Self::DEFAULT_HEX)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_accent_is_dusk_blue() {
        let a = Accent::default();
        assert_eq!(a.base, Color::from_hex(0x2D_6F_E0));
    }

    #[test]
    fn parse_accepts_hex_with_and_without_hash() {
        assert_eq!(Accent::parse("00897B").base, Color::from_hex(0x00_89_7B));
        assert_eq!(Accent::parse("#00897B").base, Color::from_hex(0x00_89_7B));
    }

    #[test]
    fn parse_rejects_garbage_and_falls_back_to_default() {
        assert_eq!(Accent::parse("zzz").base, Accent::default().base);
        assert_eq!(Accent::parse("12345").base, Accent::default().base);
        assert_eq!(Accent::parse("").base, Accent::default().base);
    }

    #[test]
    fn tiers_move_toward_white_and_black() {
        let a = Accent::from_hex(0x00_89_7B); // 青绿
        let l = a.base.relative_luminance();
        assert!(a.light3.relative_luminance() > a.light1.relative_luminance());
        assert!(a.light1.relative_luminance() > l);
        assert!(a.dark1.relative_luminance() < l);
        assert!(a.dark3.relative_luminance() < a.dark1.relative_luminance());
    }

    /// 关键回归：强调色改了，派生出的主色必须跟着改 ——
    /// 这正是「用户在 Chorus 改 accent、应用仍旧橙色」的那个 bug。
    #[test]
    fn derived_primary_tracks_base() {
        let teal = Accent::parse("00897B");
        assert_ne!(
            teal.base,
            Accent::default().base,
            "换基色必须改变主色，否则 accent 再次沦为死信"
        );
        // 亮色主档 = 基色；暗色主档 = 派生亮档 Light1（不再是基色）。
        assert_eq!(teal.primary_for(ColorScheme::Light), teal.base);
        assert_eq!(teal.primary_for(ColorScheme::Dark), teal.light1);
        assert_ne!(teal.primary_for(ColorScheme::Dark), teal.base);
    }

    /// T20 ① 回归：`#0078D4` 六档的**明度（最大分量）与下限分量**必须与 Win10
    /// `UISettings` 真值逐档吻合（±2）。
    ///
    /// ⚠ 已知边界：OS 生成色阶时还会调整**色相**（微软声明算法闭源），本实现保持色相，
    /// 故**中间分量**与真值有偏差（蓝档 G，最大 33），该偏差登记于
    /// `docs/CANON_VS_TEMPORARY.md` T20，不在本断言内。
    #[test]
    fn uwp_value_tiers_match_os_on_0078d4() {
        let a = Accent::from_hex(0x00_78_D4);
        let cases = [
            (a.light1, 0x00_91_F8u32),
            (a.light2, 0x4C_C2_FF),
            (a.light3, 0x99_EB_FF),
            (a.dark1, 0x00_67_C0),
            (a.dark2, 0x00_3E_92),
            (a.dark3, 0x00_1A_68),
        ];
        for (derived, os_hex) in cases {
            let os = Color::from_hex(os_hex);
            let hi = |c: Color| c.r.max(c.g).max(c.b) * 255.0;
            let lo = |c: Color| c.r.min(c.g).min(c.b) * 255.0;
            assert!(
                (hi(derived) - hi(os)).abs() <= 2.0,
                "明度分量偏差过大：派生 {derived:?} vs OS {os:?}"
            );
            assert!(
                (lo(derived) - lo(os)).abs() <= 2.0,
                "下限分量偏差过大：派生 {derived:?} vs OS {os:?}"
            );
        }
    }

    /// 深色方案主档取自派生亮档 —— 期望图 / 预览页里深色暮蓝是 `#4A86F0` 一档，
    /// 而基色 `#2D6FE0` 落在近黑底上偏暗（四份工人报告一致指出）。
    #[test]
    fn dark_primary_is_the_light_tier() {
        let a = Accent::default();
        assert_eq!(a.primary_for(ColorScheme::Dark), a.light1);
        // 亮档必须比基色亮（否则「深底更醒目」的裁定落空）。
        assert!(
            a.light1.relative_luminance() > a.base.relative_luminance(),
            "暗色主档必须亮于基色"
        );
    }

    /// 深色 accent 上的前景必须仍达正文对比度阈值（换档不得破坏可读性）。
    #[test]
    fn on_primary_meets_contrast_on_dark_accent() {
        for accent in [
            Accent::default(),
            Accent::parse("00897B"),
            Accent::parse("0078D4"),
        ] {
            let fg = accent.on_primary_for(ColorScheme::Dark);
            let ratio = fg.contrast_ratio(accent.primary_for(ColorScheme::Dark));
            assert!(ratio >= 4.5, "深色 accent 前景对比度仅 {ratio}，低于 4.5");
        }
    }

    /// 库级缺省方案改浅色（裁定 `docs/CANON_VS_TEMPORARY.md` T20 附带）。
    #[test]
    fn default_scheme_is_light() {
        assert_eq!(ColorScheme::default(), ColorScheme::Light);
    }

    /// 前瞻必须自动选黑/白，不能写死 —— 写死白色在浅色强调色上读不出。
    #[test]
    fn on_accent_is_chosen_by_luminance() {
        let dark_accent = Accent::from_hex(0x00_00_00);
        assert_eq!(dark_accent.on_accent, Color::WHITE);
        let light_accent = Accent::from_hex(0xFF_FF_FF);
        assert_eq!(light_accent.on_accent, Color::BLACK);
        // 深色强调色上白字必须达正文对比度阈值。
        assert!(dark_accent.on_accent.contrast_ratio(dark_accent.base) >= 4.5);
    }

    /// 守卫：自动前景必须**恒**达 4.5:1，覆盖整个色域。
    /// 旧判据（相对亮度 > 0.5）在青绿 #00897B 上只给 4.36:1 —— 该断言会失败。
    #[test]
    fn on_accent_always_meets_text_contrast_across_gamut() {
        let mut checked = 0;
        for r in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            for g in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                for b in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
                    let a = Accent::from_base(Color::rgb(r, g, b));
                    let ratio = a.on_accent.contrast_ratio(a.base);
                    assert!(
                        ratio >= 4.5,
                        "accent #{r}/{g}/{b} 的前景对比度仅 {ratio}，低于 4.5"
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 125);
    }

    /// 焦点描边在两种方案下都必须与背景可分辨（≥ 3:1 非文本阈值）。
    #[test]
    fn focus_stroke_contrasts_with_both_scheme_bases() {
        let a = Accent::parse("00897B");
        let dark_bg = Color::from_hex(0x1A_1A_1A);
        let light_bg = Color::from_hex(0xFA_FA_FA);
        assert!(a.focus_for(ColorScheme::Dark).contrast_ratio(dark_bg) >= 3.0);
        assert!(a.focus_for(ColorScheme::Light).contrast_ratio(light_bg) >= 3.0);
    }

    #[test]
    fn scheme_parses_and_round_trips() {
        assert_eq!(ColorScheme::parse("light"), Some(ColorScheme::Light));
        assert_eq!(ColorScheme::parse("dark"), Some(ColorScheme::Dark));
        assert_eq!(ColorScheme::parse("nope"), None);
        assert_eq!(ColorScheme::Light.as_str(), "light");
        assert!(ColorScheme::Dark.is_dark());
    }
}
