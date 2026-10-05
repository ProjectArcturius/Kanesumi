/// 字重。Ether 默认字体为 Noto Sans CJK SC（可变字体轴；静态档回退 Light / Regular / Bold）。
///
/// 参 `docs/DECISIONS_2026-10-05.md` §112：界面只用三档 —— 标题类 `Light`（≈ 微软雅黑
/// Light）、正文类 `Normal`（≈ 雅黑 Regular）、强调标签 `ExtraBold`（wght 800，≈ 雅黑 Bold）。
/// `Semilight` / `Medium` / `Semibold` 保留枚举值给第三方与历史调用，界面令牌不再引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    /// 300，界面标题类（Header / Subheader / Title）。
    Light,
    /// 400，界面正文 / 列表 / 菜单 / 说明（≈ 雅黑 Regular）。
    Normal,
    /// 350，历史档（第三方保留）。
    Semilight,
    /// 500，历史档（第三方保留）。
    Medium,
    /// 600，历史档（第三方保留）。
    Semibold,
    /// 700，静态字体回退档（无可变字体时 800 回落此档）。
    Bold,
    /// 800，界面强调标签（可变字体 wght 800；无可变字体时回落 Bold 700）。
    ExtraBold,
}

/// 单行文本在目标矩形内的纵向对齐（UWP `VerticalAlignment` 的一行行盒版）。
/// 只作用于单行标签（`Scene::label`）；多行段落始终从矩形上沿排。
/// 参 o4 纵向对齐（STATE 2026-10-02 §Ⅳ-11：消除调用方手算「一行高 + 纵向居中」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextVAlign {
    /// 行盒顶对齐矩形上沿（默认，改前行为逐像素不变）。
    Top,
    /// 一行行盒中线对齐矩形中线；矩形矮于一行时允许上下溢出（不裁字）。
    Center,
    /// 行盒下沿贴矩形下沿。
    Bottom,
}

/// 文本样式：尺寸为逻辑像素（display.rs 逻辑/物理分离的同一原则）。
/// `letter_spacing_em` = 字距（em 单位），负值收紧、正值放宽。UWP CharacterSpacing/1000。
/// 例：TabRow Header CharacterSpacing=−25 → letter_spacing_em = −0.025。参 V16。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: FontWeight,
    pub letter_spacing_em: f32,
    /// 单行纵向对齐（默认 Top）。参 o4 纵向对齐。
    pub v_align: TextVAlign,
}

impl TextStyle {
    /// 默认字距 0（普通排版）、纵向顶对齐。
    pub const fn new(size: f32, line_height: f32, weight: FontWeight) -> Self {
        Self {
            size,
            line_height,
            weight,
            letter_spacing_em: 0.0,
            v_align: TextVAlign::Top,
        }
    }

    /// Builder：设 em 单位字距（对齐 UWP CharacterSpacing）。参 V16。
    pub const fn with_letter_spacing_em(mut self, em: f32) -> Self {
        self.letter_spacing_em = em;
        self
    }

    /// Builder：设单行纵向对齐（默认 Top）。参 o4 纵向对齐。
    pub const fn with_v_align(mut self, a: TextVAlign) -> Self {
        self.v_align = a;
        self
    }

    /// 字距转逻辑像素。
    pub fn letter_spacing_px(&self) -> f32 {
        self.letter_spacing_em * self.size
    }
}

/// Metro 排版体系。两套命名共存：
///
/// - **语义命名**（header / subheader / title / subtitle / base / body / caption / label）——
///   Metro/UWP 风格，表达"这段字在页面里承担什么角色"。新代码优先用这套，参 §112。
/// - **尺度命名**（headline_medium / title_large / …）—— 对齐 Material 3 惯例，
///   方便 M3 代码迁入时逐个替换。
///
/// 所有样式都定 line_height，避免默认行距让中文段落过松。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroTypography {
    // 语义命名（参 §112 场景表）
    /// 大标题（Win10 Header，46）。
    pub header: TextStyle,
    /// 页标题 / 主标题（Win10 Subheader，34）。
    pub subheader: TextStyle,
    /// 分区标题 20。
    pub subtitle: TextStyle,
    /// 强调标签 / 左栏大类标题 15（ExtraBold）。
    pub base: TextStyle,
    pub title: TextStyle,
    pub body: TextStyle,
    pub caption: TextStyle,
    pub label: TextStyle,
    // 尺度命名（M3 命名，Kanesumi 定值）
    pub headline_medium: TextStyle,
    pub title_large: TextStyle,
    pub title_medium: TextStyle,
    pub body_large: TextStyle,
    pub body_medium: TextStyle,
    pub body_small: TextStyle,
}

impl MetroTypography {
    /// 默认样式阶梯。参 `docs/DECISIONS_2026-10-05.md` §112（修订 §G-67 / §N-41）：
    /// **标题类 Light（300）、正文类 Regular（400）、强调标签 Base ExtraBold（800）。**
    /// 字重与 Win10 同场景实际落到的微软雅黑 UI 等粗（实测 `docs/research/fw1/`）；
    /// 界面不再使用 Medium / DemiLight / Semibold。字号仍由 sz1 任务统一校准，此处只改字重。
    pub const fn metro() -> Self {
        use FontWeight::*;
        Self {
            // 标题类：Light。
            header: TextStyle::new(46.0, 56.0, Light),
            subheader: TextStyle::new(34.0, 42.0, Light),
            title: TextStyle::new(22.0, 28.0, Light),
            headline_medium: TextStyle::new(28.0, 36.0, Light),
            title_large: TextStyle::new(22.0, 28.0, Light),
            title_medium: TextStyle::new(16.0, 24.0, Light),
            // 分区标题：Regular。
            subtitle: TextStyle::new(20.0, 28.0, Normal),
            // 强调标签：ExtraBold。
            base: TextStyle::new(15.0, 22.0, ExtraBold),
            // 正文类：Regular。
            body: TextStyle::new(15.0, 22.0, Normal),
            caption: TextStyle::new(13.0, 18.0, Normal),
            label: TextStyle::new(11.0, 14.0, Normal),
            body_large: TextStyle::new(16.0, 24.0, Normal),
            body_medium: TextStyle::new(14.0, 20.0, Normal),
            body_small: TextStyle::new(12.0, 16.0, Normal),
        }
    }
}

impl Default for MetroTypography {
    fn default() -> Self {
        Self::metro()
    }
}

/// 正文字重 —— §112 规定正文 / 标签 / 说明类统一用正文族字重（Regular）。
///
/// 供少数「字号 / 行高必须保持写死值、不能整体换成具名样式」的控件与页面使用。
/// 不直接写 `FontWeight::Normal`，是为了让令牌改档时这些地方同步生效。
/// 参 `docs/DECISIONS_2026-10-05.md` §112。
pub const fn body_weight() -> FontWeight {
    MetroTypography::metro().body.weight
}

/// 标题字重 —— §112 规定标题类 Light。用途同 [`body_weight`]。
/// 参 `docs/DECISIONS_2026-10-05.md` §112。
pub const fn heading_weight() -> FontWeight {
    MetroTypography::metro().title.weight
}

/// 强调标签字重 —— §112 规定 Base（左栏大类标题 / 强调标签）为 ExtraBold。
pub const fn base_weight() -> FontWeight {
    MetroTypography::metro().base.weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_sizes_descend() {
        let t = MetroTypography::metro();
        assert!(t.header.size > t.subheader.size);
        assert!(t.subheader.size > t.title.size);
        assert!(t.title.size > t.body.size);
        assert!(t.body.size > t.caption.size);
        assert!(t.caption.size > t.label.size);
    }

    #[test]
    fn line_heights_tight_for_cjk() {
        let t = MetroTypography::metro();
        // 中文需紧凑行距：行距/字号 < 1.5
        assert!(t.body.line_height / t.body.size < 1.5);
    }

    /// §112（修订 §G-67 / §N-41）：标题类 Light、正文类 Normal、Base ExtraBold。
    /// 回归守卫：旧映射（标题 Bold / 正文 Medium）会让本测试失败，防止回退。
    #[test]
    fn s112_weight_ramp() {
        let t = MetroTypography::metro();
        for (name, style) in [
            ("header", t.header),
            ("subheader", t.subheader),
            ("title", t.title),
            ("headline_medium", t.headline_medium),
            ("title_large", t.title_large),
            ("title_medium", t.title_medium),
        ] {
            assert_eq!(style.weight, FontWeight::Light, "{name} 应为 Light");
        }
        for (name, style) in [
            ("subtitle", t.subtitle),
            ("body", t.body),
            ("caption", t.caption),
            ("label", t.label),
            ("body_large", t.body_large),
            ("body_medium", t.body_medium),
            ("body_small", t.body_small),
        ] {
            assert_eq!(style.weight, FontWeight::Normal, "{name} 应为 Normal");
        }
        assert_eq!(t.base.weight, FontWeight::ExtraBold, "base 应为 ExtraBold");
        assert_eq!(t.base.size, 15.0, "base 为 §112 新增 15");
    }

    /// §112：字号与令牌不一致时的字重辅助函数必须与令牌同源，
    /// 否则改了令牌而写死处不跟随，正文档又会被绕过。
    #[test]
    fn weight_helpers_track_tokens() {
        let t = MetroTypography::metro();
        assert_eq!(body_weight(), FontWeight::Normal);
        assert_eq!(heading_weight(), FontWeight::Light);
        assert_eq!(base_weight(), FontWeight::ExtraBold);
        assert_eq!(body_weight(), t.body.weight);
        assert_eq!(heading_weight(), t.title.weight);
        assert_eq!(base_weight(), t.base.weight);
    }
}
