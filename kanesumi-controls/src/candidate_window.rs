// MetroCandidateWindow —— IME 候选窗。参 CONTROL_SPEC §44 / CEYBOARD_SPEC §Ⅲ/§Ⅳ。
//
// 纯展示控件：内容（preedit / candidates / highlighted / page）由引擎层（Ceyboard）
// 注入，控件只负责画 + 命中测试，不产生候选、不持输入法状态。
// 参 CEYBOARD_SPEC §Ⅷ「Kanesumi 只负责画，Ceyboard 负责想」。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{MetroTheme, Point, Rect, Size};

/// 候选行高（CEYBOARD_SPEC §Ⅲ.3，对齐 Slider 32 / 列表行高惯例）。
pub const CANDIDATE_ROW_H: f32 = 32.0;
/// 序号列宽。
pub const CANDIDATE_LABEL_W: f32 = 20.0;
/// 面板左右内边距。
pub const CANDIDATE_PAD_X: f32 = 8.0;
/// 面板上下内边距。
pub const CANDIDATE_PAD_Y: f32 = 4.0;
/// 面板最大宽度（超出省略号截断）。
pub const CANDIDATE_MAX_W: f32 = 400.0;
/// 页脚高（翻页指示，可选）。
pub const CANDIDATE_FOOTER_H: f32 = 24.0;
/// 每页候选数（数字键 1–9）。
pub const CANDIDATES_PER_PAGE: usize = 9;

/// IME 候选窗。纯展示（引擎注入内容 + 命中测试回馈）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetroCandidateWindow {
    /// 未提交拼音串（preedit 行）。
    pub preedit: String,
    /// 组合态光标（preedit 内字节偏移，None = 无光标）。
    pub preedit_cursor: Option<usize>,
    /// 候选词（一页，≤ 9）。
    pub candidates: Vec<String>,
    /// 高亮候选下标。
    pub highlighted: Option<usize>,
    /// 当前页（0-based）。
    pub page: usize,
    /// 是否有上一页。
    pub has_prev: bool,
    /// 是否有下一页。
    pub has_next: bool,
    /// 可见性（弹层开关）。false = 不渲染。
    pub open: bool,
}

impl MetroCandidateWindow {
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否存在可展示内容（preedit 或候选）。
    pub fn is_empty(&self) -> bool {
        self.preedit.is_empty() && self.candidates.is_empty()
    }

    /// 候选词字号（CEYBOARD_SPEC §Ⅲ.3：微软拼音候选字号偏大 → body_large）。
    fn candidate_style(&self, theme: &MetroTheme) -> kanesumi_core::typography::TextStyle {
        theme.typography.body_large
    }

    /// 面板内容尺寸（不含外层 anchor 偏移），供 layer-shell 定位用。
    /// 高 = 上下内边距 + preedit 行高（若有）+ 候选行 × 行高 + 页脚（若翻页）。
    pub fn popup_size(&self) -> Size {
        let row_h = CANDIDATE_ROW_H;
        let mut h = CANDIDATE_PAD_Y * 2.0;
        if !self.preedit.is_empty() {
            h += row_h;
        }
        h += self.candidates.len() as f32 * row_h;
        if self.has_prev || self.has_next {
            h += CANDIDATE_FOOTER_H;
        }
        Size::new(CANDIDATE_MAX_W, h)
    }

    /// 命中候选行（返回下标；preedit 行 / 页脚不返回）。引擎层据此提交。
    pub fn hit_candidate(&self, rect: Rect, pos: Point) -> Option<usize> {
        if !self.open || !rect.contains(pos) {
            return None;
        }
        let mut y = rect.origin.y + CANDIDATE_PAD_Y;
        if !self.preedit.is_empty() {
            y += CANDIDATE_ROW_H; // preedit 行占一行
        }
        for (i, _) in self.candidates.iter().enumerate() {
            let row = Rect::new(rect.origin.x, y + i as f32 * CANDIDATE_ROW_H, rect.size.width, CANDIDATE_ROW_H);
            if row.contains(pos) {
                return Some(i);
            }
        }
        None
    }

    /// 渲染候选窗到 `rect`（rect = 面板整体区域，内容从内边距内排布）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        if !self.open || self.is_empty() {
            return;
        }
        let colors = &theme.colors;
        let style = self.candidate_style(theme);
        let row_h = CANDIDATE_ROW_H;

        // 面板底（直角、无边框、不透明，CEYBOARD_SPEC §Ⅲ.1）。
        scene.fill_rect(colors.surface, rect);

        let mut y = rect.origin.y + CANDIDATE_PAD_Y;

        // preedit 行（置顶，微软拼音同款）。
        if !self.preedit.is_empty() {
            let text_rect = Rect::new(
                rect.origin.x + CANDIDATE_PAD_X,
                y + (row_h - style.line_height) / 2.0,
                (rect.size.width - CANDIDATE_PAD_X * 2.0).max(0.0),
                style.line_height,
            );
            scene.text(
                self.preedit.clone(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
            );
            // 组合态光标：竖向 2px（CEYBOARD_SPEC §Ⅲ.4.2）。
            if let Some(cur) = self.preedit_cursor {
                let _ = cur;
                // 光标 x 需 TextEngine 度量（preedit 前段宽）。纯展示下省略，
                // 引擎层若需光标位置由 `ime_context` 提供 caret_rect，此处不重度量。
            }
            y += row_h;
        }

        // 候选行列表。
        for (i, cand) in self.candidates.iter().enumerate() {
            let row = Rect::new(rect.origin.x, y, rect.size.width, row_h);
            let highlighted = self.highlighted == Some(i);

            if highlighted {
                // 高亮 = primary 底 + on_primary 字（CEYBOARD_SPEC §Ⅲ.4.1）。
                scene.fill_rect(colors.primary, row);
            }

            let idx = format!("{}", i + 1);
            let fg = if highlighted {
                colors.on_primary
            } else {
                colors.on_surface
            };
            // 序号（Normal 用 on_surface 50%）。
            let label_fg = if highlighted {
                colors.on_primary
            } else {
                colors.on_surface.with_alpha(0.5)
            };
            let label_rect = Rect::new(
                rect.origin.x + CANDIDATE_PAD_X,
                y + (row_h - style.line_height) / 2.0,
                CANDIDATE_LABEL_W,
                style.line_height,
            );
            scene.text(idx, label_rect, label_fg, style, TextAlign::Left);

            // 候选词（超出省略）。
            let cand_rect = Rect::new(
                rect.origin.x + CANDIDATE_PAD_X + CANDIDATE_LABEL_W,
                y + (row_h - style.line_height) / 2.0,
                (rect.size.width - CANDIDATE_PAD_X * 2.0 - CANDIDATE_LABEL_W).max(0.0),
                style.line_height,
            );
            scene.text_with_options(
                cand.clone(),
                cand_rect,
                fg,
                style,
                TextAlign::Left,
                false,
                Some(1),
                TextOverflow::Ellipsis,
            );

            y += row_h;
        }

        // 页脚（翻页指示，可选）。
        if self.has_prev || self.has_next {
            let footer = Rect::new(
                rect.origin.x + CANDIDATE_PAD_X,
                y,
                (rect.size.width - CANDIDATE_PAD_X * 2.0).max(0.0),
                CANDIDATE_FOOTER_H,
            );
            let indicator = if self.has_prev && self.has_next {
                "‹ ›".to_string()
            } else if self.has_prev {
                "‹".to_string()
            } else {
                "›".to_string()
            };
            scene.text(
                indicator,
                footer,
                colors.on_surface_variant,
                theme.typography.caption,
                TextAlign::Right,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

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

    fn font_available() -> bool {
        find_font().is_some()
    }

    fn themed() -> MetroTheme {
        MetroTheme::ether_dark()
    }

    fn sample() -> MetroCandidateWindow {
        MetroCandidateWindow {
            preedit: "nihao".into(),
            preedit_cursor: Some(5),
            candidates: vec!["你好".into(), "尼豪".into(), "泥蒿".into()],
            highlighted: Some(0),
            page: 0,
            has_prev: false,
            has_next: true,
            open: true,
        }
    }

    #[test]
    fn closed_renders_nothing() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let mut cw = sample();
        cw.open = false;
        let mut scene = Scene::default();
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 200.0), &mut scene);
        assert!(scene.is_empty());
    }

    #[test]
    fn renders_preedit_plus_candidates() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let cw = sample();
        let mut scene = Scene::default();
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 200.0), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        // preedit 1 + 3 候选 × (序号 + 词) = 1 + 6 = 7 文本 + 页脚 1 = 8
        assert_eq!(texts, 8);
    }

    #[test]
    fn highlight_fills_primary_row() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let cw = sample();
        let mut scene = Scene::default();
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 200.0), &mut scene);
        let primary_fills = scene
            .commands
            .iter()
            .filter(|c| match c {
                SceneCommand::FillRect { color, .. } => color == &theme.colors.primary,
                _ => false,
            })
            .count();
        assert_eq!(primary_fills, 1, "仅高亮行有 primary 底");
    }

    #[test]
    fn hit_candidate_maps_row() {
        let theme = themed();
        let cw = sample();
        let rect = Rect::new(0.0, 0.0, 200.0, 200.0);
        // preedit 行占一行（32px），候选行从 y=36 开始
        let row0 = Rect::new(0.0, CANDIDATE_PAD_Y + CANDIDATE_ROW_H, 200.0, CANDIDATE_ROW_H);
        assert_eq!(cw.hit_candidate(rect, row0.center()), Some(0));
        // preedit 行不命中候选
        let preedit_row = Rect::new(0.0, CANDIDATE_PAD_Y, 200.0, CANDIDATE_ROW_H);
        assert_eq!(cw.hit_candidate(rect, preedit_row.center()), None);
    }

    #[test]
    fn popup_size_accounts_for_footer() {
        let theme = themed();
        let cw = sample();
        let sz = cw.popup_size();
        // 高 = 8 + 32(preedit) + 3×32 + 24(footer) = 164
        assert_eq!(sz.height, 8.0 + 32.0 + 96.0 + 24.0);
        assert_eq!(sz.width, CANDIDATE_MAX_W);
    }

    #[test]
    fn no_footer_when_no_paging() {
        let theme = themed();
        let mut cw = sample();
        cw.has_next = false;
        cw.has_prev = false;
        let sz = cw.popup_size();
        assert_eq!(sz.height, 8.0 + 32.0 + 96.0);
    }
}
