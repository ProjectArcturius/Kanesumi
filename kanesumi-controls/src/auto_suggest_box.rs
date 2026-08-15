// MetroAutoSuggestBox —— 自动建议输入框。参 CONTROL_SPEC §38（AutoSuggestBox 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/AutoSuggestBox/`（AutoSuggestBoxHelper.cpp + 模板）：
// - 主体 = TextBox + SuggestionsPopup（Border + ListView，MaxHeight AutoSuggestListMaxHeight）；
// - 输入变化 → 触发建议更新（`TextChanged`）；下拉展示过滤结果；
// - 键盘导航：Up/Down 在建议间移动（Helper 职责），Enter 提交选中；
// - 建议列表项高 40、Padding 12（ListView 语义，参 CONTROL_SPEC §7）。
//
// Kanesumi 实现：复用 `TextField` 编辑 + 建议列表（`MetroList` 式渲染）。
// 建议数据由宿主经 `set_suggestions` 注入（纯逻辑过滤）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::state::ControlState;
use crate::text_box::MetroTextBox;
use crate::text_field::{TextInputKey, TextField};

/// 建议列表最大高（UWP AutoSuggestListMaxHeight 300，OS 值）。
pub const AUTOSUGGEST_LIST_MAX_H: f32 = 300.0;
/// 建议项行高（ListViewItem 40，参 CONTROL_SPEC §7）。
pub const AUTOSUGGEST_ITEM_H: f32 = 40.0;
/// 建议项水平内边距（ListView Padding 12）。
pub const AUTOSUGGEST_ITEM_PAD: f32 = 12.0;
/// 面板边框（AutoSuggestListBorderThemeThickness 1）。
pub const AUTOSUGGEST_BORDER: f32 = 1.0;

/// MetroAutoSuggestBox —— 自动建议输入框。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroAutoSuggestBox {
    /// 编辑核心。
    pub field: TextField,
    /// 占位文本。
    pub placeholder: String,
    /// 顶部标题（可选）。
    pub header: String,
    /// 建议源（宿主注入，未过滤全量）。
    pub suggestions: Vec<String>,
    /// 当前显示的建议（过滤后）。
    pub shown: Vec<String>,
    /// 选中建议下标（键盘 Up/Down 移动）。
    pub highlighted: Option<usize>,
    /// 面板是否展开。
    pub popup_open: bool,
    /// 交互状态。
    pub state: ControlState,
    /// 是否聚焦。
    pub focused: bool,
    /// 水平滚动偏移（内容超宽时，保持末尾/光标可见）。单行输入框，长文本左移。
    pub scroll: f32,
    /// 上次文本（检测变化触发过滤）。
    last_text: String,
}

impl Default for MetroAutoSuggestBox {
    fn default() -> Self {
        Self {
            field: TextField::new(),
            placeholder: String::new(),
            header: String::new(),
            suggestions: Vec::new(),
            shown: Vec::new(),
            highlighted: None,
            popup_open: false,
            state: ControlState::Normal,
            focused: false,
            scroll: 0.0,
            last_text: String::new(),
        }
    }
}

impl MetroAutoSuggestBox {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带占位文本构造。
    pub fn with_placeholder(text: impl Into<String>) -> Self {
        Self {
            placeholder: text.into(),
            ..Self::default()
        }
    }

    /// 带标题构造。
    pub fn with_header(text: impl Into<String>) -> Self {
        Self {
            header: text.into(),
            ..Self::default()
        }
    }

    /// 注入建议源 + 初始内容（触发一次过滤）。
    pub fn with_suggestions(mut self, items: Vec<String>) -> Self {
        self.suggestions = items;
        self.rebuild_shown();
        self
    }

    /// 内容。
    pub fn text(&self) -> String {
        self.field.text()
    }

    /// 聚焦进入。
    pub fn focus(&mut self) {
        self.focused = true;
        self.state = ControlState::Focused;
        self.field.select_all();
        self.rebuild_shown();
    }

    /// 失焦（关闭弹层）。
    pub fn blur(&mut self) {
        self.focused = false;
        self.state = ControlState::Normal;
        self.popup_open = false;
    }

    /// 处理编辑键。Up/Down 在建议间导航，Enter 提交。
    /// 返回 `AutoSuggestAction`（宿主据此路由）。
    pub fn handle_key(&mut self, key: TextInputKey) -> Option<AutoSuggestAction> {
        match key {
            TextInputKey::Up | TextInputKey::Down if self.popup_open && !self.shown.is_empty() => {
                let n = self.shown.len();
                let delta = if key == TextInputKey::Up { -1 } else { 1 };
                let cur = self.highlighted.map_or(0usize, |i| (i as isize + delta).rem_euclid(n as isize) as usize);
                self.highlighted = Some(cur);
                Some(AutoSuggestAction::Highlight(cur))
            }
            TextInputKey::Enter => {
                if let Some(i) = self.highlighted {
                    let s = self.shown.get(i).cloned();
                    if let Some(s) = s {
                        self.field.set_text(s.clone());
                        self.popup_open = false;
                        self.highlighted = None;
                        return Some(AutoSuggestAction::Commit(s));
                    }
                }
                self.popup_open = false;
                Some(AutoSuggestAction::SubmitText)
            }            TextInputKey::Char(_)
            | TextInputKey::Backspace
            | TextInputKey::Delete
            | TextInputKey::Left
            | TextInputKey::Right
            | TextInputKey::Home
            | TextInputKey::End => {
                let changed = self.field.handle_key(key);
                if changed {
                    self.rebuild_shown();
                    return Some(AutoSuggestAction::TextChanged);
                }
                None
            }
            TextInputKey::Up | TextInputKey::Down => {
                // 弹层未开时上下键无建议导航。
                None
            }
            TextInputKey::Escape => {
                self.popup_open = false;
                Some(AutoSuggestAction::Dismiss)
            }
            TextInputKey::Tab => None,
        }
    }

    /// 输入变化 → 过滤建议 + 展开弹层。
    ///
    /// **复杂度**：每次按键 `O(N × avg_len)` —— 遍历 `suggestions` 全量做 `str::contains`。
    /// 上限 `take(50)` 只截结果条数，**不减少扫描**（依旧遍历全部 suggestions）。
    ///
    /// **适用**：N ≤ ~10k 且 avg_len 小时体感无卡。若宿主注入 100k+ 建议源，或用户
    /// 高频输入 CJK 长串，需在宿主侧提前建索引（trie / bigram / 前缀桶）并把过滤好的
    /// 子集塞回 `suggestions`，或未来在此改为增量 filter（前缀不变时复用上一帧结果）。
    fn rebuild_shown(&mut self) {
        let q = self.field.text();
        self.shown = if q.is_empty() {
            // UWP AutoSuggestBox 空文本默认不弹（IsSuggestionListOpen false）
            self.popup_open = false;
            Vec::new()
        } else {
            self.popup_open = true;
            self.suggestions
                .iter()
                .filter(|s| s.contains(&q))
                .take(50) // 结果条数上限，不影响扫描量
                .cloned()
                .collect()
        };
        if self.shown.is_empty() {
            self.popup_open = false;
        }
        self.highlighted = if self.shown.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    /// 点击建议项 → 提交。返回被选中项。
    pub fn select_item(&mut self, index: usize) -> Option<String> {
        let s = self.shown.get(index).cloned()?;
        self.field.set_text(s.clone());
        self.popup_open = false;
        self.highlighted = None;
        self.last_text = s.clone();
        Some(s)
    }

    /// 建议面板矩形（宿主 rect 下方，与文本框同宽）。
    pub fn popup_rect(&self, rect: Rect) -> Rect {
        let item_h = AUTOSUGGEST_ITEM_H;
        let max_items = (AUTOSUGGEST_LIST_MAX_H / item_h).floor() as usize;
        let n = self.shown.len().min(max_items).max(1);
        Rect::new(
            rect.origin.x,
            rect.bottom(),
            rect.size.width,
            n as f32 * item_h + 2.0 * AUTOSUGGEST_BORDER,
        )
    }

    /// 建议项矩形（面板内第 i 项）。
    pub fn item_rect(&self, rect: Rect, i: usize) -> Rect {
        let panel = self.popup_rect(rect);
        Rect::new(
            panel.origin.x + AUTOSUGGEST_BORDER,
            panel.origin.y + AUTOSUGGEST_BORDER + i as f32 * AUTOSUGGEST_ITEM_H,
            panel.size.width - 2.0 * AUTOSUGGEST_BORDER,
            AUTOSUGGEST_ITEM_H,
        )
    }

    /// 命中建议项（面板展开时）。
    pub fn hit_item(&self, rect: Rect, pos: Point) -> Option<usize> {
        if !self.popup_open {
            return None;
        }
        let panel = self.popup_rect(rect);
        if !panel.contains(pos) {
            return None;
        }
        self.shown
            .iter()
            .enumerate()
            .find(|(i, _)| self.item_rect(rect, *i).contains(pos))
            .map(|(i, _)| i)
    }

    /// 整控件命中（文本框区）。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 每帧（无动画，占位保持接口一致）。
    pub fn update(&mut self, _dt: f64) {}

    /// 渲染：TextBox（复用精简渲染）+ 建议弹层（展开时）。
    ///
    /// 自适应（单行输入）：文本以 `wrap=false` 单行排版，超出内容区时经 `PushClip`
    /// 裁剪进框内，`scroll` 保持末尾可见——修复长文本「文字不在框内」的自适应差问题
    /// （旧实现 `wrap=true` 会把超宽文本换行，第二行被框裁掉 / 溢出）。
    pub fn render(&mut self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = theme.typography.body;

        // Header
        if !self.header.is_empty() {
            scene.text(
                self.header.clone(),
                Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    style.line_height,
                ),
                colors.on_surface,
                style,
                TextAlign::Left,
            );
        }

        let body = self.body_rect(theme, rect);
        let b = self.border_thickness();
        let inner = Rect::new(
            body.origin.x,
            body.origin.y,
            (body.size.width - 2.0 * b).max(0.0),
            (body.size.height - 2.0 * b).max(0.0),
        );
        scene.fill_rounded_rect(colors.surface, inner, theme.tokens.corner_radius);

        let content = self.content_rect(theme, body);

        // 自适应滚动：单行文本超宽时，把 scroll 调至末尾可见（UWP 单行输入行为）。
        if !self.field.is_empty() {
            let text_w = engine.measure(&self.field.display_text(), style.size);
            let view_w = content.size.width;
            self.scroll = if text_w > view_w { text_w - view_w } else { 0.0 };
        } else {
            self.scroll = 0.0;
        }

        // 文本 / 占位 —— 单行不换行 + 裁剪进内容区（避免换行溢出框）。
        scene.push_clip(content);
        if !self.field.is_empty() {
            let text_rect = Rect::new(
                content.origin.x - self.scroll,
                content.origin.y,
                content.size.width + self.scroll,
                style.line_height,
            );
            scene.text_with_options(
                self.field.display_text(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
                false,
                Some(1),
                TextOverflow::Clip,
            );
        } else if !self.placeholder.is_empty() {
            let ph_rect = Rect::new(
                content.origin.x,
                content.origin.y,
                content.size.width,
                style.line_height,
            );
            scene.text_with_options(
                self.placeholder.clone(),
                ph_rect,
                colors.on_surface_variant,
                style,
                TextAlign::Left,
                false,
                Some(1),
                TextOverflow::Clip,
            );
        }
        scene.pop_clip();

        // 边框
        let (stroke, stroke_w) = if self.focused {
            (colors.focus_stroke, 2.0)
        } else if self.state == ControlState::Hovered {
            (colors.on_surface_variant.with_alpha(0.9), 1.0)
        } else {
            (colors.divider, 1.0)
        };
        scene.stroke_rounded_rect(stroke, inner, stroke_w, theme.tokens.corner_radius);

        // 建议弹层
        if self.popup_open && !self.shown.is_empty() {
            let panel = self.popup_rect(rect);
            scene.fill_rounded_rect(colors.surface_variant, panel, theme.tokens.corner_radius);
            scene.stroke_rect(colors.divider, panel, AUTOSUGGEST_BORDER);
            for (i, s) in self.shown.iter().enumerate() {
                let item = self.item_rect(rect, i);
                if item.bottom() > panel.bottom() {
                    break;
                }
                if self.highlighted == Some(i) {
                    // 高亮 = 中性（参 CONTROL_SPEC §5 规律 5：悬停用中性）
                    scene.fill_rect(colors.on_surface.with_alpha(0.30), item);
                }
                // 建议项单行不换行 + 裁剪（超宽项截断进 item，不溢出面板）。
                let text_rect = Rect::new(
                    item.origin.x + AUTOSUGGEST_ITEM_PAD,
                    item.origin.y + (AUTOSUGGEST_ITEM_H - style.line_height) / 2.0,
                    (item.size.width - 2.0 * AUTOSUGGEST_ITEM_PAD).max(0.0),
                    style.line_height,
                );
                scene.push_clip(text_rect);
                scene.text_with_options(
                    s.clone(),
                    text_rect,
                    colors.on_surface,
                    style,
                    TextAlign::Left,
                    false,
                    Some(1),
                    TextOverflow::Clip,
                );
                scene.pop_clip();
            }
        }
    }

    /// 边框厚度：聚焦 2px，其余 1px（与 MetroTextBox 对齐）。
    fn border_thickness(&self) -> f32 {
        if self.focused { 2.0 } else { 1.0 }
    }

    /// 内容区（在 `body` 内扣除边框 + Padding，与 MetroTextBox::content_rect 同款
    /// UWP Padding `10,6,6,5`）——文本/占位/滚动的自适应基准矩形。
    fn content_rect(&self, _theme: &MetroTheme, body: Rect) -> Rect {
        let b = self.border_thickness();
        let pad_l = 10.0;
        let pad_t = 6.0;
        let pad_r = 6.0;
        let pad_b = 5.0;
        Rect::new(
            body.origin.x + b + pad_l,
            body.origin.y + b + pad_t,
            (body.size.width - 2.0 * b - pad_l - pad_r).max(0.0),
            (body.size.height - 2.0 * b - pad_t - pad_b).max(0.0),
        )
    }

    /// 主体矩形（Header 之下）。
    fn body_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let style = theme.typography.body;
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            style.line_height + 4.0
        };
        Rect::new(
            rect.origin.x,
            rect.origin.y + header_h,
            rect.size.width,
            (rect.size.height - header_h).max(0.0),
        )
    }
}

/// 键盘/点击路由结果 —— 宿主据此执行动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutoSuggestAction {
    /// 文本变化（宿主可触发外部过滤/刷新）。
    TextChanged,
    /// 高亮移动（弹层内）。
    Highlight(usize),
    /// 提交选中的建议项。
    Commit(String),
    /// 回车但无选中 → 提交当前文本。
    SubmitText,
    /// Esc 关闭弹层。
    Dismiss,
}

/// 便捷占位：保留 TextBox 类型可见（AutoSuggestBox 主体即 TextBox 语义）。
#[allow(dead_code)]
fn _bridge(_tb: &MetroTextBox, _k: TextInputKey) -> bool {
    false
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

    fn boxed() -> MetroAutoSuggestBox {
        MetroAutoSuggestBox::new().with_suggestions(
            ["苹果", "香蕉", "菠萝", "橙子", "西瓜", "火龙果", "百香果"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    #[test]
    fn typing_filters_suggestions() {
        let mut ab = boxed();
        ab.focus();
        let act = ab.handle_key(TextInputKey::Char('香'));
        assert_eq!(act, Some(AutoSuggestAction::TextChanged));
        assert_eq!(ab.shown, vec!["香蕉", "百香果"]);
        assert!(ab.popup_open);
    }

    #[test]
    fn empty_query_closes_popup() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('苹'));
        assert!(ab.popup_open);
        ab.handle_key(TextInputKey::Backspace); // 删空 → 关弹层
        assert!(!ab.popup_open, "空文本不弹层");
    }

    #[test]
    fn arrow_keys_highlight() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('果'));
        assert_eq!(ab.highlighted, Some(0));
        ab.handle_key(TextInputKey::Down);
        assert_eq!(ab.highlighted, Some(1));
        ab.handle_key(TextInputKey::Down);
        assert_eq!(ab.highlighted, Some(2));
        // 循环
        ab.handle_key(TextInputKey::Up);
        assert_eq!(ab.highlighted, Some(1));
    }

    #[test]
    fn enter_commits_highlighted() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('苹'));
        ab.handle_key(TextInputKey::Down); // 高亮 1（苹果在 0）→ 从 0 到 1 → 香蕉? 不，"苹"只匹配苹果
        let act = ab.handle_key(TextInputKey::Enter);
        assert_eq!(act, Some(AutoSuggestAction::Commit("苹果".into())));
        assert!(!ab.popup_open);
        assert_eq!(ab.field.text(), "苹果");
    }

    #[test]
    fn click_selects_item() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('西'));
        let r = Rect::new(0.0, 0.0, 200.0, 32.0);
        let i = ab.hit_item(r, ab.item_rect(r, 0).center());
        assert_eq!(i, Some(0));
        let s = ab.select_item(i.unwrap());
        assert_eq!(s, Some("西瓜".into()));
        assert!(!ab.popup_open);
    }

    #[test]
    fn esc_dismisses() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('苹'));
        assert!(ab.popup_open);
        assert_eq!(ab.handle_key(TextInputKey::Escape), Some(AutoSuggestAction::Dismiss));
        assert!(!ab.popup_open);
    }

    #[test]
    fn popup_geometry_capped() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('子')); // 1 项
        let r = Rect::new(0.0, 0.0, 200.0, 32.0);
        let panel = ab.popup_rect(r);
        assert_eq!(panel.origin.y, r.bottom(), "弹层贴文本框下方");
        assert_eq!(panel.origin.x, r.origin.x);
        assert!(panel.size.height <= AUTOSUGGEST_LIST_MAX_H + 2.0);
    }

    #[test]
    fn render_emits_suggestion_rows() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('果'));
        let mut scene = Scene::default();
        ab.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 3, "文本 + 2 建议项，实际 {texts}");
    }

    #[test]
    fn highlight_first_by_default() {
        let mut ab = boxed();
        ab.focus();
        ab.handle_key(TextInputKey::Char('果'));
        assert_eq!(ab.highlighted, Some(0), "展开后默认高亮首项");
    }
}
