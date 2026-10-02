// Label —— 文本元素（XAML TextBlock）。
//
// 溢出显式（COMPOSITION 契约 5）：单行（默认）= 不换行 + 省略号；`wrap()` = 换行 + 可选最大行数。
// 量测与绘制消费同一 `TextEngine::layout` 结果（契约 7），故文字不会画出量得的矩形。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{Brush, Size, TextStyle};

use crate::widget::{AccessInfo, AccessRole, MeasureCtx, PaintCtx, Widget};

#[derive(Debug, Clone, PartialEq)]
pub struct Label {
    pub text: String,
    /// `None` = 主题正文样式。
    pub style: Option<TextStyle>,
    /// `None` = 主题 `on_surface`。
    /// 文字色。None = 主题 `on_surface`；用 `ThemeColor::*` 才随主题变化。
    pub color: Option<Brush>,
    pub align: TextAlign,
    pub wrap: bool,
    pub max_lines: Option<usize>,
}

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: None,
            color: None,
            align: TextAlign::Left,
            wrap: false,
            max_lines: None,
        }
    }

    pub fn style(mut self, style: TextStyle) -> Self {
        self.style = Some(style);
        self
    }

    pub fn color(mut self, color: impl Into<Brush>) -> Self {
        self.color = Some(color.into());
        self
    }

    pub fn align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    /// 换行段落（`max_lines` 为 None = 不限）。
    pub fn wrap(mut self, max_lines: Option<usize>) -> Self {
        self.wrap = true;
        self.max_lines = max_lines;
        self
    }

    fn line_count(&self, engine: &TextEngine, style: TextStyle, width: f32) -> usize {
        if !self.wrap {
            return 1;
        }
        let n = engine
            .layout_with_spacing_weighted(&self.text, style.size, style.letter_spacing_em, width, style.weight)
            .len()
            .max(1);
        self.max_lines.map_or(n, |m| n.min(m.max(1)))
    }
}

impl Widget for Label {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let style = self.style.unwrap_or(ctx.theme().typography.body);
        let engine = ctx.engine();
        // 量测与绘制必须同字面（T7）：字重影响字宽，窄量测会让绘制被省略号截断
        // （2026-10-03 TopBar 应用名 "Librarian" → "Librar…" 实测）。
        let natural = engine.measure_with_spacing_weighted(
            &self.text,
            style.size,
            style.letter_spacing_em,
            style.weight,
        );
        let width = if self.wrap {
            let lines = engine.layout_with_spacing_weighted(
                &self.text,
                style.size,
                style.letter_spacing_em,
                available.width,
                style.weight,
            );
            lines.iter().map(|l| l.width).fold(0.0, f32::max)
        } else {
            natural
        };
        let lines = self.line_count(engine, style, available.width);
        Size::new(width.min(available.width), lines as f32 * style.line_height)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let style = self.style.unwrap_or(ctx.theme().typography.body);
        let theme = *ctx.theme();
        let color = self.color.map_or(theme.colors.on_surface, |b| b.resolve(&theme));
        let overflow = if self.wrap {
            TextOverflow::Clip
        } else {
            TextOverflow::Ellipsis
        };
        scene.text_with_options(
            self.text.clone(),
            ctx.rect(),
            color,
            style,
            self.align,
            self.wrap,
            if self.wrap { self.max_lines } else { Some(1) },
            overflow,
        );
    }

    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::Label,
            name: self.text.clone(),
            value: None,
            checked: None,
        })
    }
}
