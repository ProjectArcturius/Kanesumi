// MetroCandidateWindow —— IME 候选窗。参 CONTROL_SPEC §44 / CEYBOARD_SPEC §Ⅲ/§Ⅳ。
//
// 纯展示控件：内容（candidates / highlighted / page）由引擎层（Ceyboard）注入，
// 控件只负责画 + 命中测试，不产生候选、不持输入法状态。
// 参 CEYBOARD_SPEC §Ⅷ「Kanesumi 只负责画，Ceyboard 负责想」。
//
// 视觉：微软拼音「新体验」横排候选——单行横向延伸，候选词横向排列
// （`1.你好 2.尼豪 3.泥蒿 …`），无 preedit 行（拼音内联在文本字段，由合成器
// text-input 桥接显示）。高亮项以强调色块包裹。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{MetroTheme, Point, Rect, Size};

/// 候选行高（微软拼音横排候选窗行高偏大，清晰易点）。
pub const CANDIDATE_ROW_H: f32 = 44.0;
/// 面板左右内边距。
pub const CANDIDATE_PAD_X: f32 = 12.0;
/// 面板上下内边距。
pub const CANDIDATE_PAD_Y: f32 = 6.0;
/// 序号 + 词之间间隔。
pub const CANDIDATE_LABEL_GAP: f32 = 2.0;
/// 相邻候选间隔（适度宽松：块间留缝但不过分，视觉透气）。
pub const CANDIDATE_ITEM_GAP: f32 = 10.0;
/// 高亮块额外内边距（序号左侧留白，词右侧留白）。
pub const CANDIDATE_HL_PAD: f32 = 6.0;
/// 面板最大宽度（超过则溢出省略当前页尾项）。
pub const CANDIDATE_MAX_W: f32 = 640.0;
/// 每页候选数（数字键 1–9）。
pub const CANDIDATES_PER_PAGE: usize = 9;

/// IME 候选窗（横排单行）。纯展示（引擎注入内容 + 命中测试回馈）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MetroCandidateWindow {
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

    /// 是否存在可展示内容。
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }

    /// 候选词字号（微软拼音候选字号偏大 → 显式 18px，横排清晰）。
    fn candidate_style(&self, _theme: &MetroTheme) -> kanesumi_core::typography::TextStyle {
        kanesumi_core::typography::TextStyle::new(18.0, 26.0, kanesumi_core::FontWeight::Normal)
    }

    /// 单候选「序号 + 词」的宽度（估算：汉字 ≈ 字号宽，拉丁 ≈ 字号×0.6）。
    /// 自适应：每块 = 自身内容宽（不撑宽整体）。
    pub fn item_width(&self, i: usize) -> f32 {
        let Some(cand) = self.candidates.get(i) else {
            return 0.0;
        };
        let size = 18.0; // 候选字号
        let mut text_w = 0.0;
        for ch in cand.chars() {
            text_w += if ch.is_ascii() { size * 0.6 } else { size };
        }
        let label_w = 12.0;
        label_w + CANDIDATE_LABEL_GAP + text_w + CANDIDATE_HL_PAD * 2.0
    }

    /// 面板内容尺寸（横排单行，宽 = 各候选自适应宽累加 + 间距，高 = 单行）。
    /// 供 popup surface 定位用。宽上限 CANDIDATE_MAX_W。
    pub fn popup_size(&self) -> Size {
        if self.candidates.is_empty() {
            return Size::new(0.0, 0.0);
        }
        let mut w = 0.0;
        for i in 0..self.candidates.len() {
            if i > 0 {
                w += CANDIDATE_ITEM_GAP;
            }
            w += self.item_width(i);
        }
        Size::new(
            w.min(CANDIDATE_MAX_W).max(1.0),
            CANDIDATE_PAD_Y * 2.0 + CANDIDATE_ROW_H,
        )
    }

    /// 命中候选项（横排自适应宽 + 间距）。返回下标。
    pub fn hit_candidate(&self, rect: Rect, pos: Point) -> Option<usize> {
        if !self.open || self.candidates.is_empty() || !rect.contains(pos) {
            return None;
        }
        // 横向遍历：y 须在候选行带内。
        let row_y0 = rect.origin.y + CANDIDATE_PAD_Y;
        if pos.y < row_y0 || pos.y >= row_y0 + CANDIDATE_ROW_H {
            return None;
        }
        let start = rect.origin.x;
        let mut x = start;
        for i in 0..self.candidates.len() {
            let iw = self.item_width(i);
            if pos.x >= x && pos.x < x + iw {
                return Some(i);
            }
            x += iw + CANDIDATE_ITEM_GAP;
            if x > rect.right() {
                break;
            }
        }
        None
    }

    /// 渲染候选窗到 `rect`（横排单行）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        if !self.open || self.is_empty() {
            return;
        }
        let colors = &theme.colors;
        let style = self.candidate_style(theme);

        // 面板底（直角、无边框、不透明，CEYBOARD_SPEC §Ⅲ.1）。
        scene.fill_rect(colors.surface, rect);

        let row_y = rect.origin.y + CANDIDATE_PAD_Y;
        let start = rect.origin.x; // 贴面板左缘 → 高亮块贴边
        let mut x = start;

        for (i, cand) in self.candidates.iter().enumerate() {
            let iw = self.item_width(i);
            if x >= rect.right() {
                break;
            }
            let highlighted = self.highlighted == Some(i);
            let text_h = style.line_height;
            let text_y = row_y + (CANDIDATE_ROW_H - text_h) / 2.0;

            if highlighted {
                // 高亮：该项自适应宽色块（垂直铺满面板，左右贴块边界）。
                scene.fill_rect(
                    colors.primary,
                    Rect::new(x, rect.origin.y, iw, rect.size.height),
                );
            }

            let fg = if highlighted {
                colors.on_primary
            } else {
                colors.on_surface
            };
            let label_fg = if highlighted {
                colors.on_primary
            } else {
                colors.on_surface.with_alpha(0.5)
            };

            // 块内内容 = 「序号 + 词」整体居中于块宽。
            let label_w = 12.0;
            let content_w = (iw - CANDIDATE_HL_PAD * 2.0).max(0.0);
            let content_x = x + ((iw - content_w) / 2.0).max(0.0);

            // 序号。
            scene.text(
                format!("{}", i + 1),
                Rect::new(content_x, text_y, label_w, text_h),
                label_fg,
                style,
                TextAlign::Left,
            );
            // 候选词（超出块右缘省略）。
            let cand_x = content_x + label_w + CANDIDATE_LABEL_GAP;
            let cand_avail = (x + iw - cand_x).max(0.0);
            scene.text_with_options(
                cand.clone(),
                Rect::new(cand_x, text_y, cand_avail, text_h),
                fg,
                style,
                TextAlign::Left,
                false,
                Some(1),
                TextOverflow::Ellipsis,
            );

            x += iw + CANDIDATE_ITEM_GAP;
        }

        // 翻页指示（右下角，仅多页时）。
        if self.has_prev || self.has_next {
            let indicator = if self.has_prev && self.has_next {
                "‹ ›".to_string()
            } else if self.has_prev {
                "‹".to_string()
            } else {
                "›".to_string()
            };
            scene.text(
                indicator,
                Rect::new(
                    rect.origin.x + CANDIDATE_PAD_X,
                    rect.origin.y + CANDIDATE_PAD_Y,
                    (rect.size.width - CANDIDATE_PAD_X * 2.0).max(0.0),
                    style.line_height,
                ),
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
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 40.0), &mut scene);
        assert!(scene.is_empty());
    }

    #[test]
    fn renders_horizontal_candidates() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let cw = sample();
        let mut scene = Scene::default();
        let sz = cw.popup_size();
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, sz.width, sz.height), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        // 3 候选 × (序号 + 词) = 6 文本（无 preedit 行）+ 翻页指示 1 = 7
        assert_eq!(texts, 7);
    }

    #[test]
    fn highlight_fills_primary_block() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let cw = sample();
        let mut scene = Scene::default();
        let sz = cw.popup_size();
        cw.render(&theme, &engine, Rect::new(0.0, 0.0, sz.width, sz.height), &mut scene);
        let primary_fills = scene
            .commands
            .iter()
            .filter(|c| match c {
                SceneCommand::FillRect { color, .. } => color == &theme.colors.primary,
                _ => false,
            })
            .count();
        assert_eq!(primary_fills, 1, "仅高亮项有 primary 底");
    }

    #[test]
    fn hit_candidate_maps_horizontal_item() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let cw = sample();
        let sz = cw.popup_size();
        let rect = Rect::new(0.0, 0.0, sz.width, sz.height);
        // 第一块中点（贴面板左缘起，等宽块中点）。
        let item0_x = rect.origin.x + cw.item_width(0) / 2.0;
        let mid_y = rect.origin.y + CANDIDATE_PAD_Y + CANDIDATE_ROW_H / 2.0;
        assert_eq!(cw.hit_candidate(rect, Point::new(item0_x, mid_y)), Some(0));
        // 面板外不命中。
        assert_eq!(cw.hit_candidate(rect, Point::new(-5.0, mid_y)), None);
    }

    #[test]
    fn popup_size_single_line() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let cw = sample();
        let sz = cw.popup_size();
        // 横排单行：高 = 上下边距 + 单行高。
        assert_eq!(sz.height, CANDIDATE_PAD_Y * 2.0 + CANDIDATE_ROW_H);
        // 宽 = 3 项等宽总和（无间距，贴边）。
        assert!(sz.width <= CANDIDATE_MAX_W);
        assert!(sz.width > 0.0);
    }
}
