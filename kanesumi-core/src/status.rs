// status.rs —— 语义状态色（success / caution / critical / neutral + attention）。
//
// **与「单一强调色」的关系**（2026-09-22 维护者裁定 = 方案 B）：
// 正典 §Ⅲ.3 的「单一强调色」限定为**非语义**配色 —— 装饰与强调只有一个 accent；
// 语义状态色属于另一类，必须显式成组存在。把二者混为一谈，等于让 InfoBar 用强调色
// 表达「错误」，那才是真正的不成熟。
//
// **attention 就是 accent 本身**（与 WinUI 一致）：WinUI 的 `SystemFillColorAttention`
// 直接绑定 `SystemAccentColor(Light2)`，不另设一套「注意色」。
//
// 数值来源（MIT，开源可读）：
// `microsoft-ui-xaml` → `controls/dev/CommonStyles/Common_themeresources_any.xaml`
// 的 ThemeDictionaries：`Default` = 暗色，`Light` = 亮色。
// 取数日期 2026-09-22；升级时按同一路径复核。

use crate::accent::ColorScheme;
use crate::color::Color;

/// 语义状态色组。**方案感知**：深浅两套，字段一一对应。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusColors {
    /// 成功（正面前景 / 图标）。
    pub success: Color,
    /// 成功面板底色。
    pub success_background: Color,
    /// 警告（注意但非错误）。
    pub caution: Color,
    pub caution_background: Color,
    /// 错误 / 危险。
    pub critical: Color,
    pub critical_background: Color,
    /// 中性（无强语义的提示）。
    pub neutral: Color,
    pub neutral_background: Color,
    /// 实心中性（用于对底色要求不透明的场合）。
    pub solid_neutral: Color,
    /// 「注意」面板底 —— attention 即 accent，故底色是强调色之外唯一的语义补充。
    pub attention_background: Color,
    /// 错误态**实心填充**：ProgressBar Error 指示条。
    ///
    /// 取值 `#E81123`（Windows 系统错误红），两方案同值 —— 它是「系统错误」这一语义的
    /// 固有标识，不随背景变浅深。`CONTROL_SPEC` §4 只写「Error：错误色」，未给笔刷名，
    /// 故登记于 `docs/CANON_VS_TEMPORARY.md`（T12），待取到权威值后改。
    pub error_fill: Color,
    /// 危险操作**实心底**：SwipeControl `SwipeItemAction::Danger` 项。
    ///
    /// 取值 `#E5534A` 属 Kanesumi（UWP SwipeItem 未在快照中给出 Danger 底色，
    /// 参 `CONTROL_SPEC` §32），登记于 `docs/CANON_VS_TEMPORARY.md`（T13）。
    /// 注意：它同时是**文字底**，与 `on_surface` 的对比度仅约 3.2:1（低于正文 4.5），
    /// 待实测后决定是换值还是改用自动前景（`Color::most_readable_on`）。
    pub danger_fill: Color,
}

impl StatusColors {
    /// 暗色方案（WinUI `Default` 主题字典）。
    pub fn dark() -> Self {
        Self {
            success: Color::from_hex(0x6C_CB_5F),
            success_background: Color::from_hex(0x39_3D_1B),
            caution: Color::from_hex(0xFC_E1_00),
            caution_background: Color::from_hex(0x43_35_19),
            critical: Color::from_hex(0xFF_99_A4),
            critical_background: Color::from_hex(0x44_27_26),
            // ⚠ WinUI 用 AARRGGBB，Kanesumi 的 `from_hex` 是 RRGGBBAA：半透明项必须换算，
            // 否则 #8BFFFFFF 会被解析成「r=8B 的不透明白」而不是「白 55%」（参 Color 的 V19 阈值坑）。
            neutral: Color::from_hex(0xFF_FF_FF_8B),
            neutral_background: Color::from_hex(0xFF_FF_FF_08),
            solid_neutral: Color::from_hex(0x9D_9D_9D),
            attention_background: Color::from_hex(0xFF_FF_FF_08),
            error_fill: Color::from_hex(0xE8_11_23),
            danger_fill: Color::from_hex(0xE5_53_4A),
        }
    }

    /// 亮色方案（WinUI `Light` 主题字典）。
    pub fn light() -> Self {
        Self {
            success: Color::from_hex(0x0F_7B_0F),
            success_background: Color::from_hex(0xDF_F6_DD),
            caution: Color::from_hex(0x9D_5D_00),
            caution_background: Color::from_hex(0xFF_F4_CE),
            critical: Color::from_hex(0xC4_2B_1C),
            critical_background: Color::from_hex(0xFD_E7_E9),
            // 同样是 AARRGGBB → RRGGBBAA 的换算。半透明**黑**必须用 from_rgba：
            // 其数值 ≤ 0x00FFFFFF，走 from_hex 会被当成不透明 RGB（V19 阈值坑）。
            neutral: Color::from_rgba(0x00_00_00_72),
            neutral_background: Color::from_rgba(0x00_00_00_06),
            solid_neutral: Color::from_hex(0x8A_8A_8A),
            attention_background: Color::from_hex(0xF6_F6_F6_80),
            // 实心语义填充两方案同值（见字段文档：系统错误红是语义固有标识）。
            error_fill: Color::from_hex(0xE8_11_23),
            danger_fill: Color::from_hex(0xE5_53_4A),
        }
    }

    pub fn for_scheme(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::Dark => Self::dark(),
            ColorScheme::Light => Self::light(),
        }
    }
}

impl Default for StatusColors {
    fn default() -> Self {
        Self::dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 语义色不得随 accent 变化 —— 换主题不该让「错误」变成另一种颜色。
    #[test]
    fn status_colors_are_independent_of_accent() {
        // 该性质由类型签名保证：StatusColors 不接受 accent 入参。
        // 此测试固定住这一契约，防止日后有人「顺手」把 accent 塞进来。
        let a = StatusColors::dark();
        let b = StatusColors::for_scheme(ColorScheme::Dark);
        assert_eq!(a, b);
    }

    #[test]
    fn success_caution_critical_are_distinct_and_ordered_by_severity_hue() {
        let s = StatusColors::dark();
        assert_ne!(s.success, s.caution);
        assert_ne!(s.caution, s.critical);
        assert_ne!(s.success, s.critical);
    }

    /// 面板底与正文（`on_surface`）的对比度必须达标 —— 语义色最容易「浅底浅字」。
    ///
    /// 半透明底（neutral_*）先按其承载面（`surface`）做 source-over 合成再判 —— 直接拿
    /// 未合成的 RGB 判会得到假结论（白 3% 会被当成纯白）。
    #[test]
    fn severity_backgrounds_meet_text_contrast() {
        use crate::accent::Accent;
        use crate::color::over;
        use crate::colors::MetroColors;

        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let status = StatusColors::for_scheme(scheme);
            let colors = MetroColors::for_scheme(scheme, Accent::default());
            let text = colors.on_surface;
            for (name, bg) in [
                ("success", status.success_background),
                ("caution", status.caution_background),
                ("critical", status.critical_background),
                ("neutral", status.neutral_background),
                ("attention", status.attention_background),
            ] {
                let effective = over(bg, colors.surface);
                let ratio = text.contrast_ratio(effective);
                assert!(ratio >= 3.0, "{scheme:?}/{name} 面板底与正文对比度仅 {ratio}");
            }
        }
    }

    /// 实心语义填充（错误指示条 / 危险操作底）必须实心、且在两种方案的承载面上可辨
    /// （非文本元素阈值 3.0，WCAG 1.4.11）。
    #[test]
    fn solid_semantic_fills_are_opaque_and_visible_on_surfaces() {
        use crate::accent::Accent;
        use crate::colors::MetroColors;

        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            let status = StatusColors::for_scheme(scheme);
            let colors = MetroColors::for_scheme(scheme, Accent::default());
            for (name, fill) in [
                ("error_fill", status.error_fill),
                ("danger_fill", status.danger_fill),
            ] {
                assert_eq!(fill.a, 1.0, "{scheme:?}/{name} 必须实心，不得透出下层");
                for (carrier, surface) in [
                    ("surface", colors.surface),
                    ("background", colors.background),
                ] {
                    let ratio = fill.contrast_ratio(surface);
                    assert!(
                        ratio >= 3.0,
                        "{scheme:?}/{name} 在 {carrier} 上对比度仅 {ratio}，低于 3.0"
                    );
                }
            }
        }
    }

    /// 实心语义填充是「系统语义固有标识」：不随方案、也不随 accent 变。
    #[test]
    fn solid_semantic_fills_are_scheme_invariant() {
        let dark = StatusColors::dark();
        let light = StatusColors::light();
        assert_eq!(dark.error_fill, light.error_fill);
        assert_eq!(dark.danger_fill, light.danger_fill);
    }
}
