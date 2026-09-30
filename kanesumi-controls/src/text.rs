use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextLayoutOptions, TextOverflow};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, MetroTheme, Rect, Size};

/// MetroText —— 文本控件。内容 + 样式 + 对齐。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroText {
    pub content: String,
    pub style: TextStyle,
    pub color: Color,
    pub align: TextAlign,
    pub wrap: bool,
    pub max_lines: Option<usize>,
    pub overflow: TextOverflow,
}

impl MetroText {
    pub fn new(content: impl Into<String>, style: TextStyle, color: Color) -> Self {
        Self {
            content: content.into(),
            style,
            color,
            align: TextAlign::Left,
            wrap: true,
            max_lines: None,
            overflow: TextOverflow::Clip,
        }
    }

    /// 正文样式便捷构造。
    pub fn body(content: impl Into<String>, color: Color) -> Self {
        Self::new(content, MetroTheme::default().typography.body, color)
    }

    pub fn with_align(mut self, align: TextAlign) -> Self {
        self.align = align;
        self
    }

    pub fn single_line(mut self) -> Self {
        self.wrap = false;
        self.max_lines = Some(1);
        self
    }

    pub fn with_max_lines(mut self, max_lines: usize) -> Self {
        self.max_lines = Some(max_lines);
        self
    }

    pub fn with_overflow(mut self, overflow: TextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// 在 `max_width` 内排版，返回内容尺寸（宽度 = 行宽上限，高度 = 行数 × 行高）。
    pub fn measure(&self, engine: &TextEngine, max_width: f32) -> Size {
        let mut options =
            TextLayoutOptions::wrapped(max_width, f32::INFINITY, self.style.line_height);
        options.letter_spacing_em = self.style.letter_spacing_em;
        options.max_lines = self.max_lines;
        options.wrap = self.wrap;
        options.overflow = self.overflow;
        engine
            .layout_box(&self.content, self.style.size, options)
            .size
    }

    /// 在 `block` 内渲染。行内对齐按 `self.align`，行间垂直方向自上而下排布。
    pub fn render(&self, _engine: &TextEngine, block: Rect, scene: &mut Scene) {
        scene.text_with_options(
            self.content.clone(),
            block,
            self.color,
            self.style,
            self.align,
            self.wrap,
            self.max_lines,
            self.overflow,
        );
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）────────────────
//
// 纯展示控件：不可聚焦、不发动作。`Widget` 实现只做量测转发与绘制转发 —— 文本的换行 /
// 越界策略仍由控件自身的 `wrap` / `max_lines` / `overflow` 决定（COMPOSITION 强制契约）。
// 命中保持默认（整矩形），因为展示文本在 Kanesumi 里就是一块可见图元。

impl kanesumi_element::Widget for MetroText {
    /// 固有尺寸 = 按自身样式的排版结果。换行时以 `available.width` 为行宽上限，
    /// 宽度取最宽行；无界轴退化为单行量测（`f32::INFINITY` 交给排版器）。
    fn measure(&mut self, ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        let max_width = if available.width.is_finite() {
            available.width
        } else {
            f32::INFINITY
        };
        MetroText::measure(self, ctx.engine(), max_width)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.engine(), ctx.rect(), scene);
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Label,
            name: self.content.clone(),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps, WidgetId};

    fn harness(text: MetroText) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            text,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn wraps_and_passes_insurance_checks() {
        let (h, id) = harness(MetroText::body(
            "the quick brown fox jumps over the lazy dog",
            Color::WHITE,
        ));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    /// 压窄用例：强制 40px 宽后三断言仍成立（文本按自身策略换行 / 收束在矩形内）。
    #[test]
    fn squeezed_width_still_within() {
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroText::body("很长的一行中文文本需要按可用宽度换行收束", Color::WHITE),
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn narrow_measure_grows_height() {
        let (mut h, id) = harness(MetroText::body(
            "the quick brown fox jumps over the lazy dog",
            Color::WHITE,
        ));
        let wide = h.rect(id).size.height;
        h.tree.update_props(id, |p| p.width = Some(60.0));
        h.frame();
        let narrow = h.rect(id).size.height;
        assert!(
            narrow > wide,
            "压窄后应换行更高：wide={wide} narrow={narrow}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::{Color, Rect};

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    #[test]
    fn measure_multiline_height() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let text = MetroText::body("the quick brown fox jumps", Color::WHITE);
        let narrow = text.measure(&engine, 60.0);
        let wide = text.measure(&engine, 400.0);
        assert!(narrow.height > wide.height, "窄宽应多行，高更大");
        assert!(narrow.width <= 60.0 + f32::EPSILON);
    }

    #[test]
    fn render_emits_text_commands() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let text = MetroText::new("Ether", MetroTheme::default().typography.body, Color::WHITE);
        let mut scene = Scene::default();
        text.render(&engine, Rect::new(0.0, 0.0, 200.0, 22.0), &mut scene);
        assert!(matches!(
            scene.commands[0],
            kanesumi_canvas::SceneCommand::Text { .. }
        ));
    }
}
