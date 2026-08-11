// MetroPersonPicture —— 圆形头像（TopBar 用户头像）。参 CONTROL_SPEC §16。
//
// 移植自 microsoft-ui-xaml/dev/PersonPicture（PersonPicture.cpp + InitialsGenerator.cpp）：
// - 圆 = 方形 rect 的 Capsule 圆角（CornerRadius::Capsule 在方形上即正圆）；
// - Initials 字号 = 42% of 边长，SemiBold；
// - Badge 圆 = 50% of 边长，右上角（Margin 0,-4,-4,0 外溢 4px）；
// - Badge 字号 = 60% of badge 圆；数字 >99 → "99+"。
// 首字母生成：Standard 字符集才产出（CJK/字形名返回空 → 宿主可回退默认图形）。

use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, CornerRadius, FontWeight, MetroTheme, Rect};

/// 名字字符分类。参 InitialsGenerator::GetCharacterType。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NameType {
    Other,
    Glyph,
    Symbolic,
    Standard,
}

/// 单个字符分类。
fn char_type(c: char) -> NameType {
    let u = c as u32;
    // Glyph —— IPA / 阿拉伯 / 天城文 / 孟加拉 / 泰米尔 / 泰文 / 老挝 …
    let glyph = (0x0250..=0x02AF).contains(&u)
        || (0x0600..=0x06FF).contains(&u)
        || (0x0750..=0x077F).contains(&u)
        || (0x08A0..=0x08FF).contains(&u)
        || (0x0900..=0x097F).contains(&u)
        || (0x0980..=0x09FF).contains(&u)
        || (0x0A00..=0x0A7F).contains(&u)
        || (0x0A80..=0x0AFF).contains(&u)
        || (0x0B00..=0x0B7F).contains(&u)
        || (0x0B80..=0x0BFF).contains(&u)
        || (0x0C00..=0x0C7F).contains(&u)
        || (0x0C80..=0x0CFF).contains(&u)
        || (0x0D00..=0x0D7F).contains(&u)
        || (0x0D80..=0x0DFF).contains(&u)
        || (0x0E00..=0x0E7F).contains(&u)
        || (0x0E80..=0x0EFF).contains(&u);
    if glyph {
        return NameType::Glyph;
    }
    // Symbolic —— CJK / 希腊 / 希伯来 / 亚美尼亚。
    let symbolic = (0x4E00..=0x9FFF).contains(&u)
        || (0x3400..=0x4DBF).contains(&u)
        || (0x20000..=0x2A6DF).contains(&u)
        || (0x2E80..=0x2EFF).contains(&u)
        || (0x3000..=0x303F).contains(&u)
        || (0x31C0..=0x31EF).contains(&u)
        || (0x0370..=0x03FF).contains(&u)
        || (0x0590..=0x05FF).contains(&u)
        || (0x0530..=0x058F).contains(&u);
    if symbolic {
        return NameType::Symbolic;
    }
    // Standard —— 拉丁 / 西里尔 / 组合变音符。
    let standard = (u > 0x0000 && u <= 0x007F)
        || (0x0080..=0x00FF).contains(&u)
        || (0x0100..=0x017F).contains(&u)
        || (0x0180..=0x024F).contains(&u)
        || (0x2C60..=0x2C7F).contains(&u)
        || (0xA720..=0xA7FF).contains(&u)
        || (0xAB30..=0xAB6F).contains(&u)
        || (0x1E00..=0x1EFF).contains(&u)
        || (0x0400..=0x04FF).contains(&u)
        || (0x0500..=0x052F).contains(&u)
        || (0x0300..=0x036F).contains(&u);
    if standard {
        return NameType::Standard;
    }
    NameType::Other
}

/// 名字整体类型：前 3 字符按 字形 > 表意 > 拉丁 优先级取最大。
fn name_type(name: &str) -> NameType {
    let mut result = NameType::Other;
    for c in name.chars().take(3) {
        let t = char_type(c);
        match t {
            NameType::Glyph => result = NameType::Glyph,
            NameType::Symbolic if result != NameType::Glyph => result = NameType::Symbolic,
            NameType::Standard if result != NameType::Glyph && result != NameType::Symbolic => {
                result = NameType::Standard
            }
            _ => {}
        }
    }
    result
}

/// 去尾随括号对（`(…)` / `[…]` / `{…}`）。参 InitialsGenerator::StripTrailingBrackets。
fn strip_trailing_brackets(name: &str) -> String {
    const DELIMS: [(&str, &str); 3] = [("{", "}"), ("(", ")"), ("[", "]")];
    for (open, close) in DELIMS {
        if name.ends_with(close)
            && let Some(start) = name.rfind(open)
        {
            return name[..start].to_string();
        }
    }
    name.to_string()
}

/// 取首字符（跳过开头标点与后续组合变音符）。
fn first_full_char(word: &str) -> String {
    let mut chars = word.chars();
    // 跳过开头标点：! " # $ % & ' ( ) * + , - . /  | : ; < = > ? @ | { | } ~
    let mut start_char = None;
    for c in chars.by_ref() {
        let u = c as u32;
        let punct = (0x0021..=0x002F).contains(&u)
            || (0x003A..=0x0040).contains(&u)
            || (0x007B..=0x007E).contains(&u);
        if !punct {
            start_char = Some(c);
            break;
        }
    }
    let Some(first) = start_char else {
        // 全标点 → 从 0 起取（上游回退 index 0）。
        return word.chars().next().map(String::from).unwrap_or_default();
    };
    // 吞掉紧随的组合变音符（0x0300..=0x036F）。
    let mut out = String::from(first);
    for c in chars {
        let u = c as u32;
        if (0x0300..=0x036F).contains(&u) {
            out.push(c);
        } else {
            break;
        }
    }
    out
}

/// 从 DisplayName 生成首字母。Standard 名字 → 首词首字符 + 末词首字符（大写）；
/// 单词 → 单字符；CJK / 字形名 → 空串。参 InitialsGenerator::InitialsFromDisplayName。
pub fn initials_from_display_name(name: &str) -> String {
    if name_type(name) != NameType::Standard {
        return String::new();
    }
    let stripped = strip_trailing_brackets(name);
    let words: Vec<&str> = stripped.split(' ').filter(|w| !w.is_empty()).collect();
    let result = match words.len() {
        0 => String::new(),
        1 => first_full_char(words[0]),
        _ => {
            let mut r = first_full_char(words[0]);
            r.push_str(&first_full_char(words[words.len() - 1]));
            r
        }
    };
    result.to_uppercase()
}

/// MetroPersonPicture —— 圆形头像 + 可选角标。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetroPersonPicture {
    /// 显示名（用于自动生成首字母）。
    pub display_name: String,
    /// 显式首字母（优先于 `display_name` 自动生成）。
    pub initials: Option<String>,
    /// 头像位图（UniformToFill 铺满方形；未做圆形裁剪，宿主可用 clip 近似）。
    pub image: Option<Icon>,
    /// 角标数字：0 或负 = 无角标；>99 渲染 "99+"。
    pub badge_number: i32,
}

impl MetroPersonPicture {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_name(name: impl Into<String>) -> Self {
        Self {
            display_name: name.into(),
            ..Self::default()
        }
    }

    /// 头像边 = min(rect 宽高)（维持方形，PersonPicture::OnSizeChanged 语义）。
    pub fn side(rect: Rect) -> f32 {
        rect.size.width.min(rect.size.height).max(1.0)
    }

    /// 头像方形 rect（以给定 rect 左上为原点，短边取齐）。
    pub fn circle_rect(rect: Rect) -> Rect {
        let side = Self::side(rect);
        Rect::new(rect.origin.x, rect.origin.y, side, side)
    }

    /// 最终显示的初始文字。
    pub fn actual_initials(&self) -> String {
        if let Some(i) = &self.initials
            && !i.is_empty()
        {
            return i.clone();
        }
        initials_from_display_name(&self.display_name)
    }

    /// Initials 文本样式：42% of 边长，SemiBold。
    pub fn initials_style(side: f32) -> TextStyle {
        let size = (side * 0.42).max(1.0);
        TextStyle::new(size, size * 1.4, FontWeight::Semibold)
    }

    /// Badge 圆 rect（50% of 边长，右上角，Margin 0,-4,-4,0 外溢）。
    pub fn badge_rect(rect: Rect) -> Option<Rect> {
        let side = Self::side(rect);
        let badge_size = side * 0.5;
        if badge_size <= 0.0 {
            return None;
        }
        Some(Rect::new(
            rect.right() - badge_size + 4.0,
            rect.origin.y - 4.0,
            badge_size,
            badge_size,
        ))
    }

    /// Badge 文本：>99 → "99+"。
    pub fn badge_text(&self) -> Option<String> {
        if self.badge_number <= 0 {
            return None;
        }
        Some(if self.badge_number > 99 {
            "99+".to_string()
        } else {
            self.badge_number.to_string()
        })
    }

    /// 渲染：头像圆（位图或底色+首字母）+ 角标。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let circle = Self::circle_rect(rect);
        let side = Self::side(rect);
        let _ = engine;

        // 位图优先（UniformToFill 近似 = 铺满方形）。
        if let Some(img) = &self.image {
            scene.image(img, circle, None);
        } else {
            // 圆底 + 首字母。
            scene.fill_rounded_rect(theme.colors.surface_variant, circle, CornerRadius::Capsule);
            let initials = self.actual_initials();
            if !initials.is_empty() {
                let style = Self::initials_style(side);
                let text_rect = Rect::new(
                    circle.origin.x,
                    circle.origin.y + (circle.size.height - style.line_height) / 2.0,
                    circle.size.width,
                    style.line_height,
                );
                scene.text(
                    initials,
                    text_rect,
                    theme.colors.on_surface,
                    style,
                    TextAlign::Center,
                );
            }
        }

        // Badge。
        if let Some(text) = self.badge_text()
            && let Some(badge) = Self::badge_rect(rect)
        {
            let badge_size = badge.size.width;
            // 底圆（fill #1A1A1A）+ 2px 描边（divider）。
            let fill = Color::from_hex(0x1A_1A_1A).with_alpha(0.8);
            scene.fill_rounded_rect(fill, badge, CornerRadius::Capsule);
            scene.stroke_rounded_rect(theme.colors.divider, badge, 2.0, CornerRadius::Capsule);
            let style = TextStyle::new(
                (badge_size * 0.6).max(1.0),
                badge_size * 0.6 * 1.4,
                FontWeight::Semibold,
            );
            let text_rect = Rect::new(
                badge.origin.x,
                badge.origin.y + (badge_size - style.line_height) / 2.0,
                badge_size,
                style.line_height,
            );
            scene.text(
                text,
                text_rect,
                theme.colors.on_surface,
                style,
                TextAlign::Center,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_two_words() {
        assert_eq!(initials_from_display_name("John Smith"), "JS");
        assert_eq!(initials_from_display_name("takahashi rinta"), "TR");
    }

    #[test]
    fn initials_single_word() {
        assert_eq!(initials_from_display_name("Ether"), "E");
    }

    #[test]
    fn initials_strips_trailing_brackets() {
        // "John Smith (OSG)" → "John Smith " → 两词 → JS
        assert_eq!(initials_from_display_name("John Smith (OSG)"), "JS");
        // "John [Smith]" → "John " → 单词 → J（上游同行为）
        assert_eq!(initials_from_display_name("John [Smith]"), "J");
    }

    #[test]
    fn initials_skip_leading_punctuation() {
        assert_eq!(initials_from_display_name("-John -Smith"), "JS");
    }

    #[test]
    fn initials_cjk_returns_empty() {
        assert_eq!(initials_from_display_name("张三"), "");
        assert_eq!(initials_from_display_name("李四"), "");
    }

    #[test]
    fn initials_empty_for_blank() {
        assert_eq!(initials_from_display_name("   "), "");
    }

    #[test]
    fn actual_initials_prefers_explicit() {
        let pic = MetroPersonPicture::with_name("John Smith");
        assert_eq!(pic.actual_initials(), "JS");
        let pic2 = MetroPersonPicture {
            initials: Some("JT".into()),
            display_name: "John Smith".into(),
            ..MetroPersonPicture::default()
        };
        assert_eq!(pic2.actual_initials(), "JT");
    }

    #[test]
    fn circle_maintains_square() {
        let r = Rect::new(0.0, 0.0, 96.0, 64.0);
        let c = MetroPersonPicture::circle_rect(r);
        assert_eq!(c.size.width, 64.0);
        assert_eq!(c.size.height, 64.0);
    }

    #[test]
    fn initials_font_is_42_percent() {
        let style = MetroPersonPicture::initials_style(96.0);
        assert!((style.size - 96.0 * 0.42).abs() < 0.01);
    }

    #[test]
    fn badge_is_half_and_top_right() {
        let r = Rect::new(0.0, 0.0, 96.0, 96.0);
        let b = MetroPersonPicture::badge_rect(r).unwrap();
        assert_eq!(b.size.width, 48.0, "Badge = 50% of 边长");
        // 右上角 + 外溢 4px
        assert!((b.right() - (r.right() + 4.0)).abs() < 0.01);
        assert!((b.origin.y - (r.origin.y - 4.0)).abs() < 0.01);
    }

    #[test]
    fn badge_text_clamps() {
        let pic = MetroPersonPicture {
            badge_number: 150,
            ..MetroPersonPicture::default()
        };
        assert_eq!(pic.badge_text().as_deref(), Some("99+"));
        let pic2 = MetroPersonPicture {
            badge_number: 7,
            ..MetroPersonPicture::default()
        };
        assert_eq!(pic2.badge_text().as_deref(), Some("7"));
        let pic3 = MetroPersonPicture::default();
        assert_eq!(pic3.badge_text(), None);
    }
}
