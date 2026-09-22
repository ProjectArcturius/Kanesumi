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
// 档位对齐 Win10 `SystemAccentColor` 的 Light1~3 / Dark1~3；派生参数与 Chorus
// `derive_accent()` 保持一致（同源算法，两处必须同步修改）。

use crate::color::Color;

/// 颜色方案 —— 仅 dark/light 二态（参 `KANESUMI_DESIGN.md` §Ⅲ.3）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ColorScheme {
    #[default]
    Dark,
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

/// 向白混合的比例（Light1/2/3）。与 Chorus 同值。
const LIGHT_MIX: [f32; 3] = [0.25, 0.45, 0.65];
/// 向黑混合的比例（Dark1/2/3）。与 Chorus 同值。
const DARK_MIX: [f32; 3] = [0.20, 0.35, 0.50];

impl Accent {
    /// Ether 默认强调色（橙金）。同时也是 `theme.toml` 缺失时的回退值。
    pub const DEFAULT_HEX: u32 = 0xE5_78_12;

    /// 由基色派生全阶。
    pub fn from_base(base: Color) -> Self {
        let mix = |target: Color, t: f32| base.lerp(target, t as f64);
        Self {
            base,
            light1: mix(Color::WHITE, LIGHT_MIX[0]),
            light2: mix(Color::WHITE, LIGHT_MIX[1]),
            light3: mix(Color::WHITE, LIGHT_MIX[2]),
            dark1: mix(Color::BLACK, DARK_MIX[0]),
            dark2: mix(Color::BLACK, DARK_MIX[1]),
            dark3: mix(Color::BLACK, DARK_MIX[2]),
            // 阈值 0.5 与 Chorus 一致。相对亮度是 gamma 校正后的量，
            // 不能用「(r+g+b)/3 > 0.5」这类朴素判据代替。
            on_accent: if base.relative_luminance() > 0.5 {
                Color::BLACK
            } else {
                Color::WHITE
            },
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

    /// 暗色方案使用的主强调色。
    ///
    /// 暗底上直接用基色即可（基色在深底上已足够醒目）。
    pub const fn primary_for(self, _scheme: ColorScheme) -> Color {
        self.base
    }

    /// 该方案下的悬停 / 按下强调色（按钮等强调面）。
    ///
    /// 暗底悬停向白、按下向更深的基色；亮底反之 —— 保证两种方案下反馈方向一致（都更「亮眼」）。
    pub const fn hover_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light1,
            ColorScheme::Light => self.dark1,
        }
    }

    /// 按下态强调色。
    pub const fn pressed_for(self, scheme: ColorScheme) -> Color {
        match scheme {
            ColorScheme::Dark => self.light2,
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
    fn default_accent_is_ether_orange() {
        let a = Accent::default();
        assert_eq!(a.base, Color::from_hex(0xE5_78_12));
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
        assert_eq!(teal.primary_for(ColorScheme::Dark), teal.base);
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
