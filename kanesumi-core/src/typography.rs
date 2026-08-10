/// 字重。Ether 默认字体为思源黑体（Source Han Sans SC）。参 ASSETS.md。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FontWeight {
    Normal,
    Semilight,
    Medium,
    Semibold,
    Bold,
}

/// 文本样式：尺寸为逻辑像素（display.rs 逻辑/物理分离的同一原则）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub weight: FontWeight,
}

impl TextStyle {
    pub const fn new(size: f32, line_height: f32, weight: FontWeight) -> Self {
        Self {
            size,
            line_height,
            weight,
        }
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
    pub const fn metro() -> Self {
        use FontWeight::*;
        Self {
            page_heading: TextStyle::new(34.0, 42.0, Normal),
            title: TextStyle::new(22.0, 28.0, Normal),
            body: TextStyle::new(15.0, 22.0, Normal),
            caption: TextStyle::new(13.0, 18.0, Normal),
            label: TextStyle::new(11.0, 14.0, Normal),
            headline_medium: TextStyle::new(28.0, 36.0, Normal),
            title_large: TextStyle::new(22.0, 28.0, Normal),
            title_medium: TextStyle::new(16.0, 24.0, Medium),
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
}
