use fontdue::{Font, FontSettings};

/// 排版结果 —— 单行（内容 + 像素宽度）。
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub content: String,
    pub width: f32,
}

/// 字体加载错误。
#[derive(Debug)]
pub enum TextLoadError {
    Io(std::io::Error),
    Parse(&'static str),
}

impl std::fmt::Display for TextLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextLoadError::Io(e) => write!(f, "读取字体失败: {e}"),
            TextLoadError::Parse(e) => write!(f, "解析字体失败: {e}"),
        }
    }
}

impl std::error::Error for TextLoadError {}

/// 文本引擎 —— fontdue 度量 + 贪心换行。参 ether-settings/topbar.rs 同款管线。
///
/// 排版唯一真源：控件 `measure`/`render` 与外壳光栅化都用 `layout`，保证量测一致。
#[derive(Clone)]
pub struct TextEngine {
    font: Font,
}

impl TextEngine {
    /// 从字体文件加载。Ether 唯一字体为思源黑体，禁止静默回退（SD §IX）。
    pub fn load(path: impl AsRef<std::path::Path>) -> Result<Self, TextLoadError> {
        let bytes = std::fs::read(path).map_err(TextLoadError::Io)?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, TextLoadError> {
        let font =
            Font::from_bytes(bytes, FontSettings::default()).map_err(TextLoadError::Parse)?;
        Ok(Self { font })
    }

    /// 单字符像素宽度。
    fn char_width(&self, c: char, size: f32) -> f32 {
        self.font.metrics(c, size).advance_width
    }

    /// 整段文本宽度（不换行）。
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        text.chars().map(|c| self.char_width(c, size)).sum()
    }

    /// 行高（ascent - descent）。
    pub fn line_height(&self, size: f32) -> f32 {
        self.font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent - m.descent)
            .unwrap_or(size * 1.2)
    }

    /// 贪心换行布局。
    ///
    /// - 优先在空白断行；
    /// - 单个词超宽时按字符硬断（覆盖中文等无空格文本）。
    /// - 连续空白折叠为单个空格。
    pub fn layout(&self, text: &str, size: f32, max_width: f32) -> Vec<Line> {
        let mut lines = Vec::new();
        let mut current = String::new();
        let mut current_width = 0.0;
        let space_width = self.char_width(' ', size);

        for token in text.split_ascii_whitespace() {
            let token_width = self.measure(token, size);
            let separator = if current.is_empty() { 0.0 } else { space_width };

            if current_width + separator + token_width <= max_width {
                if !current.is_empty() {
                    current.push(' ');
                    current_width += separator;
                }
                current.push_str(token);
                current_width += token_width;
            } else if current.is_empty() {
                // 单个词超宽：按字符硬断
                let mut seg = String::new();
                let mut seg_width = 0.0;
                for c in token.chars() {
                    let cw = self.char_width(c, size);
                    if seg_width + cw > max_width && !seg.is_empty() {
                        lines.push(Line {
                            content: std::mem::take(&mut seg),
                            width: seg_width,
                        });
                        seg_width = 0.0;
                    }
                    seg.push(c);
                    seg_width += cw;
                }
                if !seg.is_empty() {
                    lines.push(Line {
                        content: seg,
                        width: seg_width,
                    });
                }
            } else {
                lines.push(Line {
                    content: std::mem::take(&mut current),
                    width: current_width,
                });
                current.push_str(token);
                current_width = token_width;
            }
        }

        if !current.is_empty() {
            lines.push(Line {
                content: current,
                width: current_width,
            });
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试字体定位：环境变量优先，其次常见系统字体。无字体时跳过（跨平台 CI）。
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
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    #[test]
    fn measure_is_monotonic() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        assert!(engine.measure("a", 15.0) > 0.0);
        assert!(engine.measure("abc", 15.0) > engine.measure("a", 15.0));
    }

    #[test]
    fn layout_wraps_at_whitespace() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let text = "the quick brown fox jumps over the lazy dog";
        let lines = engine.layout(text, 15.0, 80.0);
        assert!(lines.len() >= 2, "宽 80px 应折多行，实际 {}", lines.len());
        for l in &lines {
            assert!(l.width <= 80.0 + f32::EPSILON, "行超宽: {}", l.width);
        }
        let joined: String = lines
            .iter()
            .map(|l| l.content.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        assert_eq!(joined, text);
    }

    #[test]
    fn layout_hard_breaks_long_word() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let long = "日本語の文章".repeat(30);
        let lines = engine.layout(&long, 15.0, 60.0);
        assert!(lines.len() >= 2);
        for l in &lines {
            assert!(l.width <= 60.0 + f32::EPSILON);
        }
    }

    #[test]
    fn single_line_within_width() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let lines = engine.layout("Ether", 15.0, 500.0);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].content, "Ether");
    }

    #[test]
    fn load_missing_font_errors() {
        assert!(TextEngine::load("C:/no/such/font.ttf").is_err());
    }
}
