/// 字重。Ether 默认字体为思源黑体（Source Han Sans SC）。参 ASSETS.md。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Semilight,
    Medium,
    Semibold,
    Bold,
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
/// - **语义命名**（page_heading / title / body / caption / label）—— Metro/UWP 风格，
///   表达"这段字在页面里承担什么角色"。新代码优先用这套。
/// - **尺度命名**（headline_medium / title_large / …）—— 对齐 Material 3 惯例，
///   方便 M3 代码迁入时逐个替换。
///
/// 所有样式都定 line_height，避免默认行距让中文段落过松。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroTypography {
    // 语义命名
    pub page_heading: TextStyle,
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
    /// 默认样式阶梯。参 `docs/DECISIONS_2026-10-04.md` §G-67（修订 N-41）：
    /// **标题类 Bold、正文类 Medium**。T7（2026-10-03）裁定「声明即所得」后，
    /// 2026-10-04 用户据 tx2 1:1 样张把正文由 Normal 提到 Medium —— 真字重是唯一在
    /// 实际尺寸下可见地「强劲」而不产生灰晕的提浓方式（参 Kanesumi
    /// `docs/research/tx2/REPORT.md`）。
    pub const fn metro() -> Self {
        use FontWeight::*;
        Self {
            // 标题类：Bold。
            page_heading: TextStyle::new(34.0, 42.0, Bold),
            title: TextStyle::new(22.0, 28.0, Bold),
            headline_medium: TextStyle::new(28.0, 36.0, Bold),
            title_large: TextStyle::new(22.0, 28.0, Bold),
            title_medium: TextStyle::new(16.0, 24.0, Bold),
            // 正文类：Medium。
            body: TextStyle::new(15.0, 22.0, Medium),
            caption: TextStyle::new(13.0, 18.0, Medium),
            label: TextStyle::new(11.0, 14.0, Medium),
            body_large: TextStyle::new(16.0, 24.0, Medium),
            body_medium: TextStyle::new(14.0, 20.0, Medium),
            body_small: TextStyle::new(12.0, 16.0, Medium),
        }
    }
}

impl Default for MetroTypography {
    fn default() -> Self {
        Self::metro()
    }
}

/// 正文字重 —— G-67 规定正文 / 标签 / 说明类统一用正文族字重。
///
/// 供少数「字号 / 行高必须保持写死值、不能整体换成具名样式」的控件与页面使用。
/// 不直接写 `FontWeight::Medium`，是为了让令牌改档时这些地方同步生效。
/// 参 `docs/DECISIONS_2026-10-04.md` §G-67。
pub const fn body_weight() -> FontWeight {
    MetroTypography::metro().body.weight
}

/// 标题字重 —— G-67 规定标题类 Bold。用途同 [`body_weight`]。
pub const fn heading_weight() -> FontWeight {
    MetroTypography::metro().title.weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_sizes_descend() {
        let t = MetroTypography::metro();
        assert!(t.page_heading.size > t.title.size);
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

    /// G-67（修订 N-41）：标题类 Bold、正文类 Medium。
    /// 回归守卫：旧映射正文 Normal / 标题 Normal 会让本测试失败，防止回退。
    #[test]
    fn g67_body_medium_heading_bold() {
        let t = MetroTypography::metro();
        for (name, style) in [
            ("page_heading", t.page_heading),
            ("title", t.title),
            ("headline_medium", t.headline_medium),
            ("title_large", t.title_large),
            ("title_medium", t.title_medium),
        ] {
            assert_eq!(style.weight, FontWeight::Bold, "{name} 应为 Bold");
        }
        for (name, style) in [
            ("body", t.body),
            ("caption", t.caption),
            ("label", t.label),
            ("body_large", t.body_large),
            ("body_medium", t.body_medium),
            ("body_small", t.body_small),
        ] {
            assert_eq!(style.weight, FontWeight::Medium, "{name} 应为 Medium");
        }
    }

    /// G-67：字号与令牌不一致时的字重辅助函数必须与令牌同源，
    /// 否则改了令牌而写死处不跟随，正文 Medium 又会被绕过。
    #[test]
    fn weight_helpers_track_tokens() {
        let t = MetroTypography::metro();
        assert_eq!(body_weight(), FontWeight::Medium);
        assert_eq!(heading_weight(), FontWeight::Bold);
        assert_eq!(body_weight(), t.body.weight);
        assert_eq!(heading_weight(), t.title.weight);
    }
}
