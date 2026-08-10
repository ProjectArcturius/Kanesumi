use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, MetroTheme, Rect, Size};

/// MetroText —— 文本控件。内容 + 样式 + 对齐。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroText {
    pub content: String,
    pub style: TextStyle,
    pub color: Color,
    pub align: TextAlign,
}

impl MetroText {
    pub fn new(content: impl Into<String>, style: TextStyle, color: Color) -> Self {
        Self {
            content: content.into(),
            style,
            color,
            align: TextAlign::Left,
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

    /// 在 `max_width` 内排版，返回内容尺寸（宽度 = 行宽上限，高度 = 行数 × 行高）。
    pub fn measure(&self, engine: &TextEngine, max_width: f32) -> Size {
        let lines = engine.layout(&self.content, self.style.size, max_width);
        let width = lines
            .iter()
            .map(|l| l.width)
            .fold(0.0, f32::max)
            .min(max_width);
        let height = lines.len() as f32 * self.style.line_height;
        Size::new(width, height)
    }

    /// 在 `block` 内渲染。行内对齐按 `self.align`，行间垂直方向自上而下排布。
    pub fn render(&self, engine: &TextEngine, block: Rect, scene: &mut Scene) {
        let style = self.style;
        let lines = engine.layout(&self.content, style.size, block.size.width);
        for (i, line) in lines.iter().enumerate() {
            let x = match self.align {
                TextAlign::Left => block.origin.x,
                TextAlign::Center => block.origin.x + (block.size.width - line.width) / 2.0,
                TextAlign::Right => block.origin.x + block.size.width - line.width,
            };
            let rect = Rect::new(
                x,
                block.origin.y + i as f32 * style.line_height,
                line.width,
                style.line_height,
            );
            scene.text(
                line.content.clone(),
                rect,
                self.color,
                style,
                TextAlign::Left,
            );
        }
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
