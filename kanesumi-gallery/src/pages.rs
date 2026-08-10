use kanesumi_core::{Color, MetroTheme};

/// Gallery 页。每个页面展示一组控件 / 主题示例。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryPage {
    /// 设计 tokens / 主题 / MetroText / 交互指示
    DesignTokens,
    /// 动画（弹簧 / 缓动 / 时长预设）
    Animation,
    /// 标准控件
    Controls,
    /// 页面结构（Shell / AppBar / Scaffold）
    Structure,
}

impl GalleryPage {
    pub fn title(self) -> &'static str {
        match self {
            GalleryPage::DesignTokens => "Design Tokens",
            GalleryPage::Animation => "Animation",
            GalleryPage::Controls => "Controls",
            GalleryPage::Structure => "Structure",
        }
    }
}

/// 页树 —— 对照 WinUI-Gallery 的分组。Phase 3 扩展为嵌套页。
pub fn page_tree() -> [GalleryPage; 4] {
    [
        GalleryPage::DesignTokens,
        GalleryPage::Animation,
        GalleryPage::Controls,
        GalleryPage::Structure,
    ]
}

/// 设计 tokens 一览条目。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteEntry {
    pub name: &'static str,
    pub color: Color,
}

/// 当前主题下的颜色 token 快照（DesignTokens 页展示用）。
pub fn palette(theme: &MetroTheme) -> Vec<PaletteEntry> {
    use kanesumi_core::MetroColors;
    let c: MetroColors = theme.colors;
    vec![
        PaletteEntry {
            name: "background",
            color: c.background,
        },
        PaletteEntry {
            name: "surface",
            color: c.surface,
        },
        PaletteEntry {
            name: "surfaceVariant",
            color: c.surface_variant,
        },
        PaletteEntry {
            name: "divider",
            color: c.divider,
        },
        PaletteEntry {
            name: "primary",
            color: c.primary,
        },
        PaletteEntry {
            name: "focusStroke",
            color: c.focus_stroke,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_tree_is_complete() {
        assert_eq!(page_tree().len(), 4);
    }

    #[test]
    fn palette_derives_from_theme() {
        let entries = palette(&MetroTheme::ether_dark());
        assert_eq!(entries.len(), 6);
        assert!(entries.iter().any(|e| e.name == "primary"));
    }
}
