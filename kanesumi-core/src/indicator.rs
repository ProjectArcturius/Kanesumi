use crate::accent::{Accent, ColorScheme};
use crate::color::Color;

/// 交互指示 —— 悬停 / 按下 / 禁用 / 焦点四态。
/// Metro 的即时反馈以 tint 与描边表达，不做模糊 / 投影等重特效。
///
/// tint 方向必须随方案翻转：暗底叠白、亮底叠黑。旧实现把「白 10%」写死，
/// 在亮色主题下等于**在浅背景上再叠白** —— 悬停完全不可见。
///
/// **强度取值的一手来源**（2026-09-22 在 Windows 上实测，清单见 `docs/UWP_PRIMARY_SOURCES.md`）：
/// `…\Windows Kits\10\DesignTime\…\UAP\10.0.26100.0\Generic\themeresources.xaml` 的
/// `Default`（暗，L4-1961）/ `Light`（亮，L3920-5878）两个 ThemeDictionary ——
/// `SystemListLowColor` = `#19FFFFFF` / `#19000000`（指针悬停，≈10%，L228/L4144）、
/// `SystemListMediumColor` = `#33FFFFFF` / `#33000000`（按下，20%，L229/L4145）、
/// `SystemControlHighlightListAccentLowBrush` = accent 0.6（暗）/ 0.4（亮）（L304/L4220）、
/// Base* 族 = 100/80/60/40/20%（L210-214 / L4126-4130）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroIndication {
    /// 悬停 tint（叠加在表面上）：UWP `SystemListLowColor` = **10%**（暗白 / 亮黑）。
    ///
    /// 来源：`themeresources.xaml` L228 / L4144；使用它的状态见
    /// `AppBarButtonBackgroundPointerOver`（L1680）、`ListViewItemBackgroundPointerOver`（L1783）、
    /// `ComboBoxItemBackgroundPointerOver`（L646）、`TreeViewItemBackgroundPointerOver`（L1838）。
    pub hover_tint: Color,
    /// 按下 tint（叠加在表面上）：UWP `SystemListMediumColor` = **20%**（暗白 / 亮黑）。
    ///
    /// 来源：`themeresources.xaml` L229 / L4145；状态见 `AppBarButtonBackgroundPressed`（L1681）、
    /// `ListViewItemBackgroundPressed`（L1784）、`SwipeItem` Pressed（`generic.xaml` L29495 区段）。
    ///
    /// ⚠ 旧值 22% 无权威依据：全文件 `#38FFFFFF` 只出现在 `MediaDownloadProgressIndicatorThemeBrush`
    /// （L1117，与交互 tint 无关），疑为当年误抄，已按 ListMedium 更正为 20%。
    pub press_tint: Color,
    /// **次要按压**底色：大面积按压或次要控件的按压反馈（滑块拇指、TopBar 芯片）。
    ///
    /// 与 [`press_tint`](Self::press_tint)（20%）不是同一个角色 —— 与 hover 族同构：
    /// 强按压用于按钮类，弱按压用于大面积/次要控件。两者此前分别叫
    /// `MetroColors::press_tint`（10%）与 `MetroIndication::press_tint`（22%），同名不同义，
    /// 已按角色改名收敛（参 `ROADMAP.md` M1-1 分类学）。
    ///
    /// 取值 10% = UWP `SystemListLowColor`：**这个强度在 UWP 里属于「悬停」档**，
    /// 用作「弱按压」是 Kanesumi 的角色选择（大面积元件上 20% 过重），非 UWP 对应项。
    pub press_subtle_tint: Color,
    /// 极轻的中性底色（分组底 / 次级容器 / 弱分隔 / WinUI 2.x 新控件悬停）。
    ///
    /// 来源（一手）：WinUI 2.x `Common_themeresources_any.xaml` 的
    /// `SubtleFillColorSecondary` = 暗 `#0FFFFFFF`（白 **5.9%**）/ 亮 `#09000000`（黑 3.5%）；
    /// 消费它的状态见 WinUI 2.8 `TabView_themeresources.xaml` L26-27/51-52
    /// （`TabViewButtonBackgroundPointerOver` / `TabViewItemHeaderCloseButtonBackgroundPointerOver`）。
    ///
    /// ⚠ 本令牌一度被拆成 `subtle_hover_tint`（15%）/ `subtle_press_tint`（25%）两个「浅叠」令牌，
    /// 依据是 `CONTROL_SPEC` §471/§790/§864/§936 写的「白 15% / 25%」。一手源证明那是
    /// **笔刷名对、百分比错**：Secondary/Tertiary 真值是 5.9% / 3.9%（暗）、3.5% / 2.4%（亮），
    /// 而且按下用的 Tertiary 比悬停用的 Secondary **更淡**。那两个令牌已删除
    /// （标题栏按钮改用 `hover_tint`/`press_tint`、标签页关闭键改用本令牌、滑动项改用 `hover_tint`）。
    pub subtle_tint: Color,
    /// 禁用态不透明度。UWP 惯例 0.38。
    ///
    /// ⚠ 一手源说明：OS UWP 里**没有**通用控件禁用不透明度这一档 ——
    /// `SystemControlDisabledBaseMediumLowBrush` / `…DisabledListMediumBrush`（L253/L258）是
    /// 纯字面转发、不带 Opacity；唯一明写的是 `ListViewItemDisabledThemeOpacity` = **0.55**（L1772）。
    /// 故 0.38 仍属 Kanesumi 取值（登记于 `docs/CANON_VS_TEMPORARY.md` T4）。
    pub disabled_opacity: f32,

    // ── 前景强度档 ──────────────────────────────────────────────────────────
    // 下面这组**不是色调**（参 `ROADMAP.md` M1-1 分类学）：叠加色仍是 `on_surface` /
    // `on_surface_variant` 一等令牌，本族只声明「同一个颜色用多强」。
    // 命名沿用 UWP `SystemControlForegroundBase*Brush` 家族 —— `CONTROL_SPEC` 多处
    // 就是按 BaseMediumHigh / BaseMedium / BaseMediumLow 描述边框与字形强度的。
    //
    // 三个档位的权威值（`themeresources.xaml` L210-214 暗 / L4126-4130 亮）：
    // BaseHigh `#FF…` = 100%、BaseMediumHigh `#CC…` = **80%**、BaseMedium `#99…` = 60%、
    // BaseMediumLow `#66…` = **40%**、BaseLow `#33…` = 20%。
    // 旧注释与 CONTROL_SPEC 写的「BaseMediumHigh 90% / BaseMediumLow 35%」是错的，已按实测更正。
    /// BaseMediumHigh（**80%**）：文本光标、自绘字形、次级前景。
    pub base_medium_high: f32,
    /// BaseMedium（**60%**）：弱化前景（preedit 下划线 `CONTROL_SPEC` §1154、已填充轨道）、
    /// 以及控件**悬停边框**（`SystemControlHighlightBaseMediumBrush`，L298）。
    pub base_medium: f32,
    /// BaseMediumLow（**40%**）：中灰实心（CheckBox UncheckedPressed，`CONTROL_SPEC` §1216）。
    pub base_medium_low: f32,
    /// 次级前景（0.8）：有色底上的次级行 / 次级字形 / 次级描边。
    ///
    /// 取值属 Kanesumi（UWP 无独立的 0.8 档；`BaseMediumHigh` 也恰是 0.8，但那作用于
    /// `on_surface`，本档作用于有色底），登记于 `docs/CANON_VS_TEMPORARY.md`。
    pub secondary_opacity: f32,
    /// 非激活 / 只读前景（0.5）：只读评分星、候选窗未选中项的序号。
    ///
    /// ⚠ 一手源候选：UWP `ListViewItemDisabledThemeOpacity` = 0.55（L1772，列表行专用）。
    /// 取值仍属 Kanesumi（`CONTROL_SPEC` §819 只写「低透」未给数），登记于
    /// `docs/CANON_VS_TEMPORARY.md` T16。
    pub inactive_opacity: f32,
    /// 聚焦态占位文本（0.7）：UWP `TextControlPlaceholderForegroundFocused` 的「略暗」观感。
    ///
    /// ⚠ 一手源显示这条更复杂：OS 的 Focused 占位笔刷是
    /// `SystemControlPageTextChromeBlackMediumLowBrush` = `SystemChromeBlackMediumLowColor`
    /// = `#66000000`（**黑 40%**，L218/L4134）——因为暗色方案下 UWP 聚焦文本框会变成**白底黑字**
    /// （`TextControlBackgroundFocused` = `SystemControlBackgroundChromeWhiteBrush` = `#FFFFFFFF`，L852）。
    /// 本库不采用「聚焦变白纸」这条 UWP 行为，故该档仍是待裁定项（登记于 `CANON_VS_TEMPORARY` T16）。
    pub placeholder_focused_opacity: f32,
    /// 焦点描边（由 accent 派生）。
    pub focus_stroke: Color,
}

impl MetroIndication {
    /// 按方案派生。tint 强度取 UWP 一手字典：指针悬停 = `SystemListLowColor`（10%）、
    /// 按下 = `SystemListMediumColor`（20%），两方案**对称**（只有叠白 / 叠黑之别）。
    pub fn for_scheme(scheme: ColorScheme, accent: Accent) -> Self {
        match scheme {
            ColorScheme::Dark => Self {
                hover_tint: Color::from_hex(0xFF_FF_FF_19), // 白 9.8%（UWP ListLow #19）
                press_tint: Color::from_hex(0xFF_FF_FF_33), // 白 20%（UWP ListMedium #33）
                press_subtle_tint: Color::from_hex(0xFF_FF_FF_19), // 白 9.8%（= ListLow）
                subtle_tint: Color::from_hex(0xFF_FF_FF_0F), // 白 5.9%
                disabled_opacity: 0.38,
                base_medium_high: 0.8,
                base_medium: 0.6,
                base_medium_low: 0.4,
                secondary_opacity: 0.8,
                inactive_opacity: 0.5,
                placeholder_focused_opacity: 0.7,
                focus_stroke: accent.focus_for(ColorScheme::Dark),
            },
            ColorScheme::Light => Self {
                // 亮底叠黑。半透明黑必须 `from_rgba`（`from_hex` 对 ≤0x00FFFFFF 走 RGB 分支，
                // 会静默变成不透明黑 —— 参 V19 阈值坑）。
                // 强度与暗色**对称**：ListLow 10% / ListMedium 20%（UWP 亮色字典 L4144/L4145）。
                hover_tint: Color::from_rgba(0x00_00_00_19), // 黑 9.8%
                press_tint: Color::from_rgba(0x00_00_00_33), // 黑 20%
                press_subtle_tint: Color::from_rgba(0x00_00_00_19), // 黑 9.8%
                subtle_tint: Color::from_rgba(0x00_00_00_09), // 黑 3.5%
                // ⚠ 亮色禁用不透明度沿用暗色 0.38，**未实测**（登记于 CANON_VS_TEMPORARY T4）。
                disabled_opacity: 0.38,
                // 前景强度档与方案无关：叠加色（`on_surface`）已经翻转，强度再按方案变
                // 会让同一控件在两种方案下浓淡不一（回归测试固定这一点）。
                base_medium_high: 0.8,
                base_medium: 0.6,
                base_medium_low: 0.4,
                secondary_opacity: 0.8,
                inactive_opacity: 0.5,
                placeholder_focused_opacity: 0.7,
                focus_stroke: accent.focus_for(ColorScheme::Light),
            },
        }
    }

    /// 兼容别名：旧调用点（`MetroIndication::ether()`）= 暗色 + 默认 accent。
    pub fn ether() -> Self {
        Self::for_scheme(ColorScheme::Dark, Accent::default())
    }
}

impl Default for MetroIndication {
    fn default() -> Self {
        Self::ether()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disabled_opacity_uwp_convention() {
        assert_eq!(MetroIndication::ether().disabled_opacity, 0.38);
    }

    /// 关键回归：亮色方案下 tint 必须叠黑而不是叠白，否则悬停在浅底上不可见。
    #[test]
    fn light_scheme_tints_darken_instead_of_lighten() {
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert!(dark.hover_tint.r > 0.5, "暗底悬停叠白");
        assert!(light.hover_tint.r < 0.5, "亮底悬停叠黑");
        assert_eq!(
            light.hover_tint.a,
            25.0 / 255.0,
            "半透明黑不得退化成不透明（V19）；强度取 UWP ListLow #19"
        );
    }

    /// 悬停 / 按下强度由 UWP 一手字典固定：`SystemListLowColor` = 10%、
    /// `SystemListMediumColor` = 20%（`themeresources.xaml` L228/L229 暗、L4144/L4145 亮），
    /// 且两方案**对称**（旧值「亮色只有暗色一半」没有依据，22% 亦然）。
    #[test]
    fn interaction_tint_strengths_match_uwp_list_brushes() {
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert!((dark.hover_tint.a - 0.10).abs() < 0.01, "暗色悬停 = ListLow 10%");
        assert!((light.hover_tint.a - 0.10).abs() < 0.01, "亮色悬停 = ListLow 10%");
        assert!((dark.press_tint.a - 0.20).abs() < 0.01, "暗色按下 = ListMedium 20%");
        assert!((light.press_tint.a - 0.20).abs() < 0.01, "亮色按下 = ListMedium 20%");
        // 对称：同一角色的 alpha 必须相等，只有叠加色方向不同。
        assert_eq!(dark.hover_tint.a, light.hover_tint.a);
        assert_eq!(dark.press_tint.a, light.press_tint.a);
        assert_eq!(dark.press_subtle_tint.a, light.press_subtle_tint.a);
        assert!(dark.press_tint.a > dark.hover_tint.a, "按压必须比悬停实");
    }

    #[test]
    fn focus_stroke_follows_accent() {
        let orange = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let teal = MetroIndication::for_scheme(ColorScheme::Dark, Accent::parse("00897B"));
        assert_ne!(orange.focus_stroke, teal.focus_stroke);
    }

    /// 极轻底 = WinUI 2.x `SubtleFillColorSecondary`（暗 5.9% / 亮 3.5%），
    /// 且比通用悬停（10%）**更淡** —— 这正是它「极轻」的语义。
    #[test]
    fn subtle_tint_is_lighter_than_hover() {
        for (scheme, expected) in [(ColorScheme::Dark, 0.059), (ColorScheme::Light, 0.035)] {
            let i = MetroIndication::for_scheme(scheme, Accent::default());
            assert!(
                (i.subtle_tint.a - expected).abs() < 0.01,
                "{scheme:?} 极轻底应取 WinUI 2.x SubtleFillColorSecondary = {expected}"
            );
            assert!(
                i.subtle_tint.a < i.hover_tint.a,
                "{scheme:?} 极轻底必须比悬停更淡"
            );
        }
    }

    /// tint 是**半透明叠加**，其上的正文必须合成后再判对比度：
    /// 令牌自检的常见漏洞就是拿未合成的 RGB 判，于是「浅底叠白」也判成合格。
    #[test]
    fn text_on_interaction_tints_meets_contrast() {
        use crate::color::over;
        use crate::colors::MetroColors;

        for scheme in [ColorScheme::Dark, ColorScheme::Light] {
            for accent in [
                Accent::default(),
                Accent::parse("00897B"),
                Accent::parse("0078D7"),
            ] {
                let i = MetroIndication::for_scheme(scheme, accent);
                let colors = MetroColors::for_scheme(scheme, accent);
                let ctx = format!("{scheme:?}");
                for (name, tint) in [
                    ("hover_tint", i.hover_tint),
                    ("press_tint", i.press_tint),
                    ("press_subtle_tint", i.press_subtle_tint),
                    ("subtle_tint", i.subtle_tint),
                ] {
                    for (carrier, surface) in [
                        ("surface", colors.surface),
                        ("surface_variant", colors.surface_variant),
                    ] {
                        let effective = over(tint, surface);
                        let ratio = colors.on_surface.contrast_ratio(effective);
                        assert!(
                            ratio >= 4.5,
                            "{ctx}/{name} 叠在 {carrier} 上时正文对比度仅 {ratio}"
                        );
                    }
                }
            }
        }
    }

    /// 前景强度档是**不透明度乘数**，与方案无关：叠加色已经翻转，
    /// 强度再按方案变会让同一控件在两种方案下浓淡不一。
    #[test]
    fn foreground_opacity_tiers_are_scheme_independent() {
        let dark = MetroIndication::for_scheme(ColorScheme::Dark, Accent::default());
        let light = MetroIndication::for_scheme(ColorScheme::Light, Accent::default());
        assert_eq!(dark.disabled_opacity, light.disabled_opacity);
        assert_eq!(dark.base_medium_high, light.base_medium_high);
        assert_eq!(dark.base_medium, light.base_medium);
        assert_eq!(dark.base_medium_low, light.base_medium_low);
        assert_eq!(dark.secondary_opacity, light.secondary_opacity);
        assert_eq!(dark.inactive_opacity, light.inactive_opacity);
        assert_eq!(
            dark.placeholder_focused_opacity,
            light.placeholder_focused_opacity
        );
        // 档位取值由一手源固定（Base* 族等距 20 点）：High/Secondary 0.8 ≥ 聚焦占位 0.7 >
        // Medium 0.6 > Inactive 0.5 > MediumLow 0.4 > 禁用 0.38。
        let i = dark;
        assert_eq!(i.base_medium_high, 0.8, "BaseMediumHigh = #CC = 80%");
        assert_eq!(i.base_medium_low, 0.4, "BaseMediumLow = #66 = 40%");
        assert!(i.base_medium_high >= i.secondary_opacity);
        assert!(i.secondary_opacity > i.placeholder_focused_opacity);
        assert!(i.placeholder_focused_opacity > i.base_medium);
        assert!(i.base_medium > i.inactive_opacity);
        assert!(i.inactive_opacity > i.base_medium_low);
        assert!(i.base_medium_low > i.disabled_opacity);
    }
}
