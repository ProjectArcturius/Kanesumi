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
/// 相邻候选间隔（**无 gap**：候选块连续紧贴，微软拼音横排同款）。
pub const CANDIDATE_ITEM_GAP: f32 = 0.0;
/// 高亮块左右内边距（加宽 → 每块内左右留白更大，内容宽松不挤）。
pub const CANDIDATE_HL_PAD: f32 = 16.0;
/// 面板最大宽度。放不下的候选整项省略（不画半个），面板宽恒 ≤ 此值
/// （内容预算 = 此值 − `CANDIDATE_PAD_X`，省略时右缘补回内边距）。
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
        // 候选词无同字号令牌（18/26），只取正文字重，字号行高保持原位。
        kanesumi_core::typography::TextStyle::new(
            18.0,
            26.0,
            kanesumi_core::typography::body_weight(),
        )
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

    /// 本页实际可见条数：按每项实际宽度累加，第一个放不下的候选起**整项省略**
    /// （微软拼音行为：不画半个，翻页键可见；参 imp1 报告范围外第 1 条）。
    /// 预算 = `CANDIDATE_MAX_W − CANDIDATE_PAD_X`：省略时面板右缘补回右内边距，
    /// 总宽仍 ≤ `CANDIDATE_MAX_W`。至少显示 1 项（单项超宽时块内文本自身省略）。
    /// 引擎层（Ceyboard）以后可据此翻页；本控件不产生翻页。
    pub fn visible_count(&self) -> usize {
        if self.candidates.is_empty() {
            return 0;
        }
        let budget = CANDIDATE_MAX_W - CANDIDATE_PAD_X;
        let mut w = 0.0;
        for i in 0..self.candidates.len() {
            let iw = self.item_width(i);
            let gap = if i > 0 { CANDIDATE_ITEM_GAP } else { 0.0 };
            if i > 0 && w + gap + iw > budget {
                return i;
            }
            w += gap + iw;
        }
        self.candidates.len()
    }

    /// 面板内容尺寸（横排单行，宽 = 各候选自适应宽累加 + 间距，高 = 单行）。
    /// 供 popup surface 定位用。放不下的候选整项省略（`visible_count`），
    /// 此时宽 = 最后一个完整候选的右沿 + 右内边距。
    pub fn popup_size(&self) -> Size {
        if self.candidates.is_empty() {
            return Size::new(0.0, 0.0);
        }
        let vis = self.visible_count();
        let mut w = 0.0;
        for i in 0..vis {
            if i > 0 {
                w += CANDIDATE_ITEM_GAP;
            }
            w += self.item_width(i);
        }
        // 有省略：右缘补内边距收尾（不再贴内容）；全显：纯累加，与旧版逐像素一致。
        let width = if vis < self.candidates.len() {
            w + CANDIDATE_PAD_X
        } else {
            w.clamp(1.0, CANDIDATE_MAX_W)
        };
        Size::new(
            width.max(1.0),
            CANDIDATE_PAD_Y * 2.0 + CANDIDATE_ROW_H,
        )
    }

    /// 命中候选项（横排自适应宽 + 间距）。返回**页内**下标；只覆盖可见项
    /// （被省略的区域不命中，点击空白无动作）。
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
        let vis = self.visible_count();
        for i in 0..vis {
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

        // 放不下的候选整项省略（不画半个）；高亮项落在省略部分时同样不画
        // （翻页到可见处是引擎的责任，参 imp1 报告范围外第 1 条）。
        let vis = self.visible_count();
        for (i, cand) in self.candidates.iter().enumerate().take(vis) {
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
                // 序号是**辅助信息**（候选词才是内容），未选中时压到非激活档。
                colors
                    .on_surface
                    .with_alpha(theme.indication.inactive_opacity)
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

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）──────────────────
//
// **纯展示**：`measure` = `popup_size()`，`paint` = 旧 `render`。内容（candidates /
// highlighted / page）仍由引擎层注入。点候选项发 `CandidateChosen`（全局下标 = 页偏移 +
// 页内下标，照旧 `hit_candidate` 语义）。**不可聚焦** —— 焦点永远在文本框；点击不抓焦点。

/// 元素树动作：候选项被点击。携带全局下标（`page × CANDIDATES_PER_PAGE + 页内下标`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateChosen(pub usize);

impl kanesumi_element::Widget for MetroCandidateWindow {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: Size,
    ) -> Size {
        self.popup_size()
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        if let Event::PointerUp {
            pos,
            button: PointerButton::Left,
            ..
        } = event
            && let Some(i) = self.hit_candidate(ctx.rect(), *pos)
        {
            let global = self.page * CANDIDATES_PER_PAGE + i;
            ctx.emit(CandidateChosen(global));
            ctx.set_handled();
        }
    }

    /// 纯展示：焦点永远在文本框，候选窗不占 Tab 位。
    fn focusable(&self) -> bool {
        false
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: "候选".to_string(),
            value: self.highlighted.and_then(|i| self.candidates.get(i).cloned()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps, Widget, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCandidateWindow {
                candidates: vec!["你好".into(), "尼豪".into(), "泥蒿".into()],
                highlighted: Some(0),
                page: 0,
                has_prev: false,
                has_next: true,
                open: true,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn item_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let cw = h.tree.get::<MetroCandidateWindow>(id).unwrap();
        let rect = h.rect(id);
        let mut x = rect.origin.x;
        for j in 0..i {
            x += cw.item_width(j) + CANDIDATE_ITEM_GAP;
        }
        Point::new(
            x + cw.item_width(i) / 2.0,
            rect.origin.y + CANDIDATE_PAD_Y + CANDIDATE_ROW_H / 2.0,
        )
    }

    #[test]
    fn click_second_candidate_reports_index() {
        let (mut h, id) = harness();
        h.click_at(item_center(&h, id, 1));
        assert_eq!(
            h.take::<CandidateChosen>(),
            vec![(id, CandidateChosen(1))]
        );
    }

    #[test]
    fn global_index_includes_page_offset() {
        let (mut h, id) = harness();
        h.tree.edit::<MetroCandidateWindow, _>(id, |c, ctx| {
            c.page = 1;
            ctx.invalidate_paint();
        });
        h.frame();
        h.click_at(item_center(&h, id, 0));
        assert_eq!(
            h.take::<CandidateChosen>(),
            vec![(id, CandidateChosen(CANDIDATES_PER_PAGE))]
        );
    }

    #[test]
    fn click_does_not_move_focus() {
        let (mut h, id) = harness();
        assert!(!h.tree.get::<MetroCandidateWindow>(id).unwrap().focusable());
        assert_eq!(h.tree.focused(), None);
        h.click_at(item_center(&h, id, 1));
        assert_eq!(h.tree.focused(), None, "点击候选窗不改变焦点");
    }

    /// 9 个长候选的树（宽表面容纳省略后的窗口）。
    fn overflow_harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(900.0, 160.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCandidateWindow {
                candidates: (0..9).map(|i| format!("候选词长长长{i:02}")).collect(),
                highlighted: Some(0),
                page: 2,
                has_prev: true,
                has_next: false,
                open: true,
            },
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
    fn click_last_visible_item_reports_correct_global_index() {
        let (mut h, id) = overflow_harness();
        let vis = h.tree.get::<MetroCandidateWindow>(id).unwrap().visible_count();
        assert!(vis < 9);
        h.click_at(item_center(&h, id, vis - 1));
        let expected = 2 * CANDIDATES_PER_PAGE + vis - 1;
        assert_eq!(h.take::<CandidateChosen>(), vec![(id, CandidateChosen(expected))]);
    }

    #[test]
    fn click_on_omitted_area_does_nothing() {
        let (mut h, id) = overflow_harness();
        let cw = h.tree.get::<MetroCandidateWindow>(id).unwrap();
        let rect = h.rect(id);
        let mut content_w = 0.0;
        for i in 0..cw.visible_count() {
            content_w += cw.item_width(i) + if i > 0 { CANDIDATE_ITEM_GAP } else { 0.0 };
        }
        // 最后一个可见项右侧、面板右内边距内的空白 = 被省略区域。
        let pos = Point::new(
            rect.origin.x + content_w + CANDIDATE_PAD_X / 2.0,
            rect.origin.y + CANDIDATE_PAD_Y + CANDIDATE_ROW_H / 2.0,
        );
        assert_eq!(cw.hit_candidate(rect, pos), None, "省略区不命中");
        h.click_at(pos);
        assert_eq!(h.take::<CandidateChosen>(), Vec::new(), "省略区点击无动作");
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness();
        assert_eq!(h.rect(id).size, h.tree.get::<MetroCandidateWindow>(id).unwrap().popup_size());
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
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

    fn overflowing() -> MetroCandidateWindow {
        MetroCandidateWindow {
            candidates: (0..9).map(|i| format!("候选词长长长{i:02}")).collect(),
            highlighted: Some(0),
            page: 0,
            has_prev: false,
            has_next: true,
            open: true,
        }
    }

    /// 放不下 → 从第一个超预算的候选起整项省略；面板宽 ≤ 上限，
    /// 最后一个可见项完整（右沿 = 面板宽 − 右内边距）。
    #[test]
    fn overflow_omits_whole_tail_items() {
        let cw = overflowing();
        let vis = cw.visible_count();
        assert!(vis < 9, "长候选须整项省略，实际可见 {vis}");
        assert!(vis >= 1, "至少显示 1 项");
        let sz = cw.popup_size();
        assert!(sz.width <= CANDIDATE_MAX_W, "面板宽 {} 超上限", sz.width);
        let mut content_w = 0.0;
        for i in 0..vis {
            if i > 0 {
                content_w += CANDIDATE_ITEM_GAP;
            }
            content_w += cw.item_width(i);
        }
        assert_eq!(sz.width, content_w + CANDIDATE_PAD_X, "宽 = 最后完整项右沿 + 右内边距");
    }

    /// 省略场景不画半个：所有文本 / 高亮块右沿 ≤ 面板宽 − 右内边距。
    #[test]
    fn overflow_renders_no_partial_item() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let cw = overflowing();
        let sz = cw.popup_size();
        let rect = Rect::new(0.0, 0.0, sz.width, sz.height);
        let mut scene = Scene::default();
        cw.render(&theme, &engine, rect, &mut scene);
        let limit = rect.right() - CANDIDATE_PAD_X;
        for cmd in &scene.commands {
            match cmd {
                // 文本框（含 ellipsis 兜底）不得越出可见区。
                SceneCommand::Text { rect: r, .. } => {
                    assert!(r.origin.x + r.size.width <= limit + 0.5, "文本越出可见区 {r:?}");
                }
                // 高亮块不得越出可见区（面板底 surface 铺满 rect，不在此列）。
                SceneCommand::FillRect { color, rect: r, .. } if color == &theme.colors.primary => {
                    assert!(r.right() <= limit + 0.5, "高亮块越出可见区 {r:?}");
                }
                _ => {}
            }
        }
    }

    /// 全部放得下 → 全可见，宽度 = 逐项累加（与改前一致）。
    #[test]
    fn short_candidates_all_visible_width_unchanged() {
        let cw = sample();
        assert_eq!(cw.visible_count(), 3);
        let expect: f32 = (0..3).map(|i| cw.item_width(i)).sum();
        assert_eq!(cw.popup_size().width, expect);
    }

    /// 高亮项落在省略部分：不画高亮块（翻页是引擎的责任），不 panic。
    #[test]
    fn highlight_in_omitted_range_is_not_drawn() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = themed();
        let mut cw = overflowing();
        cw.highlighted = Some(8); // 最后一项必然被省略
        let sz = cw.popup_size();
        let rect = Rect::new(0.0, 0.0, sz.width, sz.height);
        let mut scene = Scene::default();
        cw.render(&theme, &engine, rect, &mut scene);
        let primary_fills = scene
            .commands
            .iter()
            .filter(|c| match c {
                SceneCommand::FillRect { color, .. } => color == &theme.colors.primary,
                _ => false,
            })
            .count();
        assert_eq!(primary_fills, 0, "省略区内的高亮项不得画出高亮块");
    }
}
