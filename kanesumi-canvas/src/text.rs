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

    /// 字体 ascent（基线以上高度，逻辑像素）。光栅化定位基线用。
    pub fn ascent(&self, size: f32) -> f32 {
        self.font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent)
            .unwrap_or(size * 0.8)
    }

    /// 单字符度量（advance/xmin/ymin/width/height）。光栅化定位字形用。
    pub fn glyph_metrics(&self, c: char, size: f32) -> fontdue::Metrics {
        self.font.metrics(c, size)
    }

    /// 光栅化单字符为 alpha 覆盖位图（`len == width * height`）。
    /// 外壳用它生成字形纹理（参 HANDOVER §1 Scene Text 光栅化）。
    pub fn rasterize(&self, c: char, size: f32) -> (fontdue::Metrics, Vec<u8>) {
        self.font.rasterize(c, size)
    }

    /// 贪心换行布局。参 V14：用 UAX #14 断行机会（`unicode_linebreak`）替代 ASCII 空白，
    /// 让 CJK 文本 / 混合文本在正确位置断行（不会把 "，" "。" 推到行首等禁则）。
    ///
    /// 步骤：
    /// 1. `unicode_linebreak::linebreaks(text)` → (byte_idx_after_segment, BreakOpportunity)
    /// 2. 段间贪心累加宽度，超宽即换行；
    /// 3. 单段本身超宽 → 按字符硬断（兜底）；
    /// 4. `Mandatory`（如 `\n`）强制换行；
    /// 5. Line.content 剥尾部空白（避免"the quick  brown" 双空格）。
    pub fn layout(&self, text: &str, size: f32, max_width: f32) -> Vec<Line> {
        let mut lines = Vec::new();
        if text.is_empty() || max_width <= 0.0 {
            return lines;
        }

        let mut current = String::new();
        let mut current_width = 0.0;
        let mut prev_end = 0usize;

        let opportunities: Vec<(usize, unicode_linebreak::BreakOpportunity)> =
            unicode_linebreak::linebreaks(text).collect();

        for (byte_idx, opp) in opportunities {
            let segment = &text[prev_end..byte_idx];
            prev_end = byte_idx;
            if segment.is_empty() {
                continue;
            }
            let is_mandatory =
                matches!(opp, unicode_linebreak::BreakOpportunity::Mandatory);
            // 强制断行前先剥掉段末的换行符（\n / \r），它们不进入渲染。
            let render_segment = if is_mandatory {
                segment.trim_end_matches(['\n', '\r'])
            } else {
                segment
            };
            let seg_width = self.measure(render_segment, size);

            if current_width + seg_width <= max_width {
                current.push_str(render_segment);
                current_width += seg_width;
            } else if current.is_empty() {
                // 单段超宽 —— 按字符硬断（保底）
                self.hard_break_into_lines(render_segment, size, max_width, &mut lines);
            } else {
                // 换行：先 flush 当前，再放新段（新段仍超宽再硬断）
                Self::push_trimmed(&mut lines, &mut current, current_width, size, self);
                current_width = 0.0;
                if seg_width <= max_width {
                    current.push_str(render_segment);
                    current_width = seg_width;
                } else {
                    self.hard_break_into_lines(render_segment, size, max_width, &mut lines);
                }
            }

            if is_mandatory && !current.is_empty() {
                Self::push_trimmed(&mut lines, &mut current, current_width, size, self);
                current_width = 0.0;
            }
        }

        if !current.is_empty() {
            Self::push_trimmed(&mut lines, &mut current, current_width, size, self);
        }
        lines
    }

    /// 单段（无法在其内部找到 UAX #14 断行点）超宽时按字符硬断。
    fn hard_break_into_lines(
        &self,
        seg: &str,
        size: f32,
        max_width: f32,
        out: &mut Vec<Line>,
    ) {
        let mut buf = String::new();
        let mut buf_w = 0.0;
        for c in seg.chars() {
            let cw = self.char_width(c, size);
            if buf_w + cw > max_width && !buf.is_empty() {
                let trimmed = buf.trim_end();
                let w = self.measure(trimmed, size);
                out.push(Line {
                    content: trimmed.to_string(),
                    width: w,
                });
                buf.clear();
                buf_w = 0.0;
            }
            buf.push(c);
            buf_w += cw;
        }
        if !buf.is_empty() {
            let trimmed = buf.trim_end();
            let w = self.measure(trimmed, size);
            out.push(Line {
                content: trimmed.to_string(),
                width: w,
            });
        }
    }

    /// 把 current 剥尾空白后压入 lines，清空 current。
    fn push_trimmed(
        lines: &mut Vec<Line>,
        current: &mut String,
        _current_width: f32,
        size: f32,
        engine: &TextEngine,
    ) {
        let trimmed = current.trim_end();
        let w = engine.measure(trimmed, size);
        lines.push(Line {
            content: trimmed.to_string(),
            width: w,
        });
        current.clear();
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

    #[test]
    fn layout_respects_cjk_line_start_prohibition() {
        // V14: 中文标点（如「，」「。」）不应出现在行首（UAX #14 CL 类禁则）。
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        // 构造：让"，"处刚好在某行末尾/开头附近
        let text = "你好世界你好世界，世界你好";
        let lines = engine.layout(text, 15.0, 90.0);
        for l in &lines {
            let first_char = l.content.chars().next();
            // "，" "。" "！" "？" 等中文标点不应在行首
            let prohibited = ['，', '。', '！', '？', '：', '；', '、', '）', '】', '」', '』'];
            assert!(
                first_char.map(|c| !prohibited.contains(&c)).unwrap_or(true),
                "行首不应是禁则字符：{:?}",
                l.content
            );
        }
    }

    #[test]
    fn layout_handles_mandatory_break_from_newline() {
        // \n 强制断行
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let text = "line1\nline2\nline3";
        let lines = engine.layout(text, 15.0, 500.0);
        assert_eq!(lines.len(), 3, "3 行");
        assert_eq!(lines[0].content, "line1");
        assert_eq!(lines[1].content, "line2");
        assert_eq!(lines[2].content, "line3");
    }

    #[test]
    fn layout_trims_trailing_whitespace() {
        // V14 副作用：不能有尾空白（旧代码在段末拼空格，join 后双空格）
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let text = "hello world foo bar baz";
        let lines = engine.layout(text, 15.0, 60.0);
        for l in &lines {
            assert_eq!(
                l.content.trim_end(),
                l.content.as_str(),
                "Line.content 不应有尾空白：{:?}",
                l.content
            );
        }
    }
}
