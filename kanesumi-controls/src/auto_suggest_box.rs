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

use crate::ime::{ImeContentHint, ImeContext};
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
    /// 元素树下当前展开的建议弹层（`SuggestionList` 节点）；旧路径不用。
    tree_popup: Option<kanesumi_element::WidgetId>,
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
            tree_popup: None,
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

        // 边框：聚焦时正典 §Ⅳ 双层（外 2px 焦点色 + 内 1px 对比色）；其余态单层。
        if self.focused {
            crate::focus::draw_focus_ring(
                scene,
                colors.focus_stroke,
                theme.scheme,
                inner,
                theme.tokens.corner_radius,
            );
        } else {
            let (stroke, stroke_w) = if self.state == ControlState::Hovered {
                // 悬停边框 = BaseMedium 实色（一手源 themeresources L855 → L212），同 TextBox。
                (colors.on_surface_variant, 1.0)
            } else {
                (colors.divider, 1.0)
            };
            scene.stroke_rounded_rect(stroke, inner, stroke_w, theme.tokens.corner_radius);
        }

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
                    // 高亮 = 中性（参 CONTROL_SPEC §5 规律 5：悬停用中性）。
                    // 建议列表与 ListView 行同族；一手源显示 UWP 两处都用 ListLow（10%）。
                    scene.fill_rect(theme.indication.hover_tint, item);
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

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；输入框模板同 text_box.rs，
//    弹层模板参 docs/ELEMENT_MIGRATION.md §8 与 drop_down_button.rs）──────────────
//
// 旧路径把建议列表画在输入框自己的 Scene 里（宿主要传整屏、命中要手算）；元素树里
// 建议弹层是覆盖层上的独立节点 `SuggestionList`：输入框只负责「过滤 / 打开 / 更新 /
// 关闭」，弹层的放置、命中、点击由框架与弹层自身承担。弹层**不可聚焦**
// （`focusable() = false`），焦点始终留在输入框 —— 键入不因弹层出现而中断，
// 这正是 `EventCtx::edit` 的用例（参 `event_ctx_can_edit_another_widget_while_focus_stays`）。

/// 元素树动作：用户从建议弹层选中了一项。携带该项文本。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionChosen(pub String);

/// 元素树动作：回车提交当前文本（无高亮项时）。携带当前全文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuerySubmitted(pub String);

/// 建议弹层面板 —— 覆盖层上的独立节点。不可聚焦，点击某项即选中。
pub struct SuggestionList {
    /// 打开它的输入框（关闭通知 / 回写文本的目标）。
    owner: kanesumi_element::WidgetId,
    /// 当前显示的建议（过滤后）。
    shown: Vec<String>,
    /// 高亮项下标。
    highlighted: Option<usize>,
    /// 期望宽度（与输入框同宽）。
    width: f32,
}

impl SuggestionList {
    fn new(
        owner: kanesumi_element::WidgetId,
        shown: Vec<String>,
        highlighted: Option<usize>,
        width: f32,
    ) -> Self {
        Self {
            owner,
            shown,
            highlighted,
            width,
        }
    }

    fn set_items(&mut self, shown: Vec<String>, highlighted: Option<usize>) {
        self.shown = shown;
        self.highlighted = highlighted;
    }

    /// 第 i 项矩形（面板内）。
    fn item_rect(panel: Rect, i: usize) -> Rect {
        Rect::new(
            panel.origin.x + AUTOSUGGEST_BORDER,
            panel.origin.y + AUTOSUGGEST_BORDER + i as f32 * AUTOSUGGEST_ITEM_H,
            (panel.size.width - 2.0 * AUTOSUGGEST_BORDER).max(0.0),
            AUTOSUGGEST_ITEM_H,
        )
    }

    /// 命中项下标。
    fn hit_item(&self, panel: Rect, pos: Point) -> Option<usize> {
        if !panel.contains(pos) {
            return None;
        }
        (0..self.shown.len()).find(|i| Self::item_rect(panel, *i).contains(pos))
    }
}

impl kanesumi_element::Widget for SuggestionList {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let max_items = (AUTOSUGGEST_LIST_MAX_H / AUTOSUGGEST_ITEM_H).floor() as usize;
        let n = self.shown.len().min(max_items).max(1) as f32;
        let h = n * AUTOSUGGEST_ITEM_H + 2.0 * AUTOSUGGEST_BORDER;
        kanesumi_core::Size::new(
            self.width.min(available.width.max(0.0)),
            h.min(available.height.max(0.0)),
        )
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        let colors = &theme.colors;
        let style = theme.typography.body;
        let panel = ctx.rect();
        scene.fill_rounded_rect(colors.surface_variant, panel, theme.tokens.corner_radius);
        scene.stroke_rect(colors.divider, panel, AUTOSUGGEST_BORDER);
        for (i, s) in self.shown.iter().enumerate() {
            let item = Self::item_rect(panel, i);
            if item.bottom() > panel.bottom() {
                break;
            }
            if self.highlighted == Some(i) {
                // 高亮 = 中性（参 CONTROL_SPEC §5 规律 5：悬停用中性）。
                scene.fill_rect(theme.indication.hover_tint, item);
            }
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

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        if let Event::PointerUp {
            pos,
            button: PointerButton::Left,
            ..
        } = event
            && let Some(i) = self.hit_item(ctx.rect(), *pos)
            && let Some(item) = self.shown.get(i).cloned()
        {
            ctx.emit(SuggestionChosen(item.clone()));
            // 回写文本由弹层发起 —— 它知道被点的是哪一项；`PopupClosed` 不携带这项信息。
            ctx.edit::<MetroAutoSuggestBox, _>(self.owner, |b, e| {
                b.accept_suggestion(&item);
                e.invalidate_paint();
            });
            ctx.close_popup(ctx.id());
            ctx.set_handled();
        }
    }

    /// 弹层不可聚焦：焦点始终留在输入框，键入不因弹层出现而中断。
    fn focusable(&self) -> bool {
        false
    }
}

/// 元素树 `Key` → 编辑核心 `TextInputKey`（与 text_box.rs 同表；控件层不能依赖 harness，
/// 故在此另持一份）。
fn edit_key(key: kanesumi_element::Key) -> Option<TextInputKey> {
    use kanesumi_element::Key;
    Some(match key {
        Key::Char(c) => TextInputKey::Char(c),
        Key::Space => TextInputKey::Char(' '),
        Key::Enter => TextInputKey::Enter,
        Key::Backspace => TextInputKey::Backspace,
        Key::Delete => TextInputKey::Delete,
        Key::Left => TextInputKey::Left,
        Key::Right => TextInputKey::Right,
        Key::Up => TextInputKey::Up,
        Key::Down => TextInputKey::Down,
        Key::Home => TextInputKey::Home,
        Key::End => TextInputKey::End,
        Key::Escape => TextInputKey::Escape,
        Key::Tab => TextInputKey::Tab,
        Key::Insert | Key::PageUp | Key::PageDown | Key::F(_) => return None,
        Key::Unknown(_) => return None,
    })
}

impl MetroAutoSuggestBox {
    /// 采纳一项建议：写回文本、关弹层、清高亮。弹层节点由调用方负责移除。
    fn accept_suggestion(&mut self, s: &str) {
        self.field.set_text(s);
        self.field.set_cursor(s.chars().count());
        self.popup_open = false;
        self.highlighted = None;
        self.tree_popup = None;
        self.last_text = s.to_string();
    }

    /// 依据 `shown` 打开 / 更新 / 关闭建议弹层。**不抢焦点**（弹层不可聚焦）。
    fn sync_popup(&mut self, ctx: &mut kanesumi_element::EventCtx) {
        if self.shown.is_empty() {
            if let Some(p) = self.tree_popup.take() {
                ctx.close_popup(p);
            }
            return;
        }
        match self.tree_popup {
            Some(p) => {
                let shown = self.shown.clone();
                let hl = self.highlighted;
                ctx.edit::<SuggestionList, _>(p, |list, e| {
                    // 项数变 → 面板高变：必须重量测（只重画会沿用旧高度）。
                    let resized = list.shown.len() != shown.len();
                    list.set_items(shown, hl);
                    if resized {
                        e.invalidate_measure();
                    } else {
                        e.invalidate_paint();
                    }
                });
            }
            None => {
                let width = ctx.rect().size.width;
                let popup =
                    SuggestionList::new(ctx.id(), self.shown.clone(), self.highlighted, width);
                let id = ctx.open_popup(
                    popup,
                    kanesumi_element::PopupSpec {
                        anchor: Some(ctx.id()),
                        side: kanesumi_element::PopupSide::Bottom,
                        gap: crate::popup::popup_gap(),
                        ..kanesumi_element::PopupSpec::default()
                    },
                );
                self.tree_popup = Some(id);
            }
        }
    }

    /// 编辑键后：刷新过滤 → 同步弹层 →（文本真变时）报 `TextChanged`。
    fn after_edit(&mut self, ctx: &mut kanesumi_element::EventCtx, changed: bool) {
        if changed {
            self.rebuild_shown();
            self.sync_popup(ctx);
            ctx.emit(crate::text_box::TextChanged(self.field.text()));
        }
        ctx.invalidate_paint();
    }

    /// Up / Down：在建议间循环移动高亮，并同步给弹层。
    fn step_highlight(&mut self, ctx: &mut kanesumi_element::EventCtx, delta: isize) {
        if self.tree_popup.is_none() || self.shown.is_empty() {
            return;
        }
        let n = self.shown.len();
        let cur = self
            .highlighted
            .map_or(0usize, |i| (i as isize + delta).rem_euclid(n as isize) as usize);
        self.highlighted = Some(cur);
        if let Some(p) = self.tree_popup {
            let hl = self.highlighted;
            ctx.edit::<SuggestionList, _>(p, |list, e| {
                list.highlighted = hl;
                e.invalidate_paint();
            });
        }
        ctx.invalidate_paint();
    }

    /// 点击定位光标（与 MetroTextBox::place_caret_at 同款，参 CONTROL_SPEC §34）。
    fn place_caret_at(
        &mut self,
        theme: &MetroTheme,
        engine: &TextEngine,
        body: Rect,
        pos: Point,
    ) {
        let content = self.content_rect(theme, body);
        let size = theme.typography.body.size;
        let click_x = (pos.x + self.scroll - content.origin.x).max(0.0);
        let text = self.field.display_text();
        let geometry = engine.line_geometry(&text, size, 0.0);
        self.field.set_cursor(geometry.caret_at_x(click_x));
    }

    /// 光标 x（相对 body 左缘，含滚动偏移与组合态光标）。
    fn caret_x(&self, theme: &MetroTheme, engine: &TextEngine, body: Rect) -> f32 {
        let content = self.content_rect(theme, body);
        let size = theme.typography.body.size;
        let text = self.field.display_text();
        let geometry = engine.line_geometry(&text, size, 0.0);
        let idx = self.field.cursor()
            + if self.field.has_preedit() {
                self.field.preedit_caret_char()
            } else {
                0
            };
        content.origin.x - self.scroll + geometry.caret_x(idx)
    }

    /// 光标矩形（表面绝对坐标，未夹右缘 —— IME 需要真实位置）。
    fn caret_rect_absolute(&self, theme: &MetroTheme, engine: &TextEngine, body: Rect) -> Rect {
        let content = self.content_rect(theme, body);
        Rect::new(
            self.caret_x(theme, engine, body),
            content.origin.y,
            2.0,
            content.size.height,
        )
    }

    /// 当前 IME 上下文（周边文本 + 光标矩形）。`body` 为控件主体矩形。
    fn ime_context(&self, theme: &MetroTheme, engine: &TextEngine, body: Rect) -> ImeContext {
        let (before, after, cursor_byte, anchor_byte) = self.field.surrounding_text(1000);
        ImeContext {
            surrounding_before: before,
            surrounding_after: after,
            cursor_byte: cursor_byte as u32,
            anchor_byte: anchor_byte as u32,
            caret_rect: self.caret_rect_absolute(theme, engine, body),
            content_hint: ImeContentHint::Normal,
        }
    }

    /// 键盘：Up / Down 导航弹层，Enter 选中 / 提交，Esc 关弹层；其余转编辑核心。
    fn on_key_down(
        &mut self,
        ctx: &mut kanesumi_element::EventCtx,
        key: kanesumi_element::Key,
        modifiers: &kanesumi_element::Modifiers,
    ) {
        use kanesumi_element::Key;
        // Tab 留给框架做焦点遍历。
        if key == Key::Tab {
            return;
        }
        if key == Key::Escape {
            // 弹层开着：由输入框关闭并截停；否则留给框架（关闭其它弹层 / 无操作）。
            if self.tree_popup.is_some() {
                if let Some(p) = self.tree_popup.take() {
                    ctx.close_popup(p);
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            return;
        }
        if key == Key::Enter {
            if let Some(i) = self.highlighted
                && let Some(s) = self.shown.get(i).cloned()
            {
                let popup = self.tree_popup.take();
                self.accept_suggestion(&s);
                if let Some(p) = popup {
                    ctx.close_popup(p);
                }
                ctx.emit(SuggestionChosen(s));
                ctx.invalidate_paint();
                ctx.set_handled();
                return;
            }
            ctx.emit(QuerySubmitted(self.field.text()));
            ctx.set_handled();
            return;
        }
        if matches!(key, Key::Up | Key::Down) {
            self.step_highlight(ctx, if key == Key::Up { -1 } else { 1 });
            // 弹层未开时上下键不消费（与旧 handle_key 一致）。
            if self.tree_popup.is_some() {
                ctx.set_handled();
            }
            return;
        }
        if modifiers.ctrl {
            match key {
                Key::Char('a' | 'A') => self.field.select_all(),
                Key::Char('z' | 'Z') => {
                    if self.field.undo() {
                        self.after_edit(ctx, true);
                    }
                }
                _ => return,
            }
            ctx.invalidate_paint();
            ctx.set_handled();
            return;
        }
        let before = self.field.text();
        if modifiers.shift {
            match key {
                Key::Left => self.field.move_left(true),
                Key::Right => self.field.move_right(true),
                Key::Home => self.field.move_home(true),
                Key::End => self.field.move_end(true),
                _ => {
                    if let Some(k) = edit_key(key) {
                        self.field.handle_key(k);
                    }
                }
            }
        } else if let Some(k) = edit_key(key) {
            self.field.handle_key(k);
        } else {
            return;
        }
        let changed = self.field.text() != before;
        self.after_edit(ctx, changed);
        ctx.set_handled();
    }
}

impl kanesumi_element::Widget for MetroAutoSuggestBox {
    /// 宽 = max(MinWidth 64, 占位 / 标题宽 + Padding 16)；高 = 标题行 + max(MinHeight 32,
    /// 行高 + 上下 Padding 11 + 边框 2)（与 MetroTextBox 对齐，无删除按钮列）。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let style = ctx.theme().typography.body;
        let engine = ctx.engine();
        let content_w = engine
            .measure(&self.placeholder, style.size)
            .max(engine.measure(&self.header, style.size));
        let width = (content_w + 16.0).max(64.0).min(available.width.max(0.0));
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            style.line_height + 4.0
        };
        let body_h = (style.line_height + 11.0 + 2.0).max(32.0);
        kanesumi_core::Size::new(width, header_h + body_h)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let s = ctx.state();
        self.state = if s.disabled {
            ControlState::Disabled
        } else if s.focused {
            ControlState::Focused
        } else {
            crate::state::control_state(s)
        };
        let rect = ctx.rect();
        let theme = *ctx.theme();
        // 建议列表已迁到独立弹层节点 `SuggestionList`：主体绘制不再画列表。
        let popup_open = self.popup_open;
        self.popup_open = false;
        self.render(&theme, ctx.engine(), rect, scene);
        self.popup_open = popup_open;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        match event {
            Event::FocusIn { .. } => {
                self.focus();
                ctx.invalidate_paint();
            }
            Event::FocusOut => {
                self.field.clear_preedit();
                self.blur();
                if let Some(p) = self.tree_popup.take() {
                    ctx.close_popup(p);
                }
                ctx.invalidate_paint();
            }
            Event::PopupClosed { popup } if self.tree_popup == Some(*popup) => {
                self.tree_popup = None;
                ctx.invalidate_paint();
            }
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let rect = ctx.rect();
                let theme = *ctx.theme();
                let body = self.body_rect(&theme, rect);
                if let Some(engine) = ctx.engine().cloned() {
                    self.place_caret_at(&theme, &engine, body, *pos);
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, modifiers } => {
                self.on_key_down(ctx, *key, modifiers);
            }
            Event::Preedit { text, cursor_byte } => {
                self.field.set_preedit(text, *cursor_byte);
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::Commit { text } => {
                let before = self.field.text();
                self.field.commit_ime(text);
                let changed = self.field.text() != before;
                self.after_edit(ctx, changed);
                ctx.set_handled();
            }
            Event::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => {
                let before = self.field.text();
                self.field.delete_surrounding(*before_bytes, *after_bytes);
                let changed = self.field.text() != before;
                self.after_edit(ctx, changed);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    /// 输入框自绘聚焦边框（2px），不要框架再叠焦点视觉。
    fn focusable(&self) -> bool {
        true
    }

    fn focus_visual(&self) -> bool {
        false
    }

    fn ime(
        &self,
        rect: Rect,
        theme: &MetroTheme,
        engine: &TextEngine,
    ) -> Option<kanesumi_element::ImeContext> {
        self.focused
            .then(|| self.ime_context(theme, engine, self.body_rect(theme, rect)))
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::TextInput,
            name: if self.header.is_empty() {
                self.placeholder.clone()
            } else {
                self.header.clone()
            },
            value: Some(self.field.text()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn boxed() -> MetroAutoSuggestBox {
        MetroAutoSuggestBox::new().with_suggestions(
            ["苹果", "香蕉", "菠萝", "橙子", "西瓜", "火龙果", "百香果"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            boxed(),
            LayoutProps {
                width: Some(240.0),
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(10.0, 10.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn text(h: &TestHarness, id: WidgetId) -> String {
        h.tree.get::<MetroAutoSuggestBox>(id).unwrap().field.text()
    }

    fn popup(h: &TestHarness) -> WidgetId {
        h.tree.popups().next().expect("建议弹层应已打开")
    }

    #[test]
    fn typing_opens_popup_and_focus_stays_in_input() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.type_text("果");
        let p = popup(&h);
        assert_eq!(h.tree.focused(), Some(id), "弹层出现后焦点仍在输入框");
        assert!(
            h.rect(p).origin.y >= h.rect(id).bottom(),
            "弹层贴在输入框下方"
        );
    }

    #[test]
    fn popup_height_follows_suggestion_count() {
        // 回归：项数变化只重画不重量测 → 面板沿用旧高度。
        let (mut h, _id) = harness();
        h.tab();
        h.type_text("香"); // 香蕉 / 百香果
        let p = popup(&h);
        let two = h.rect(p).size.height;
        h.type_text("果"); // 「香果」→ 百香果（弹层不关，原地更新）
        assert_eq!(popup(&h), p, "同一弹层原地更新");
        let one = h.rect(p).size.height;
        assert!(
            (two - one - AUTOSUGGEST_ITEM_H).abs() < 0.5,
            "少一项面板矮一行：{two} → {one}"
        );
    }

    #[test]
    fn down_enter_chooses_and_closes() {
        let (mut h, id) = harness();
        h.tab();
        h.type_text("果"); // 苹果 / 火龙果 / 百香果
        h.key(Key::Down); // 高亮 0 → 1
        h.key(Key::Enter);
        let acts = h.take::<SuggestionChosen>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1, SuggestionChosen("火龙果".into()));
        assert_eq!(text(&h, id), "火龙果");
        assert!(h.tree.popups().next().is_none(), "选中后弹层关闭");
        assert_eq!(h.tree.focused(), Some(id), "焦点仍在输入框");
    }

    #[test]
    fn click_item_chooses_and_closes() {
        let (mut h, id) = harness();
        h.tab();
        h.type_text("果");
        let p = popup(&h);
        let pr = h.rect(p);
        h.click_at(Point::new(
            pr.origin.x + 20.0,
            pr.origin.y + AUTOSUGGEST_ITEM_H / 2.0,
        ));
        let acts = h.take::<SuggestionChosen>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1, SuggestionChosen("苹果".into()));
        assert_eq!(text(&h, id), "苹果");
        assert!(h.tree.popups().next().is_none());
        assert_eq!(h.tree.focused(), Some(id));
    }

    #[test]
    fn no_match_closes_popup() {
        let (mut h, _id) = harness();
        h.tab();
        h.type_text("果");
        assert!(h.tree.popups().next().is_some());
        h.type_text("zzz");
        assert!(h.tree.popups().next().is_none(), "无匹配关闭弹层");
    }

    #[test]
    fn escape_closes_popup_and_keeps_focus() {
        let (mut h, id) = harness();
        h.tab();
        h.type_text("果");
        assert!(h.tree.popups().next().is_some());
        h.key(Key::Escape);
        assert!(h.tree.popups().next().is_none(), "Esc 关闭弹层");
        assert_eq!(h.tree.focused(), Some(id), "焦点仍在输入框");
    }

    #[test]
    fn enter_without_highlight_submits_query() {
        let (mut h, id) = harness();
        h.tab();
        h.type_text("无匹配项"); // 无候选 → 无高亮
        h.key(Key::Enter);
        assert_eq!(
            h.take::<QuerySubmitted>(),
            vec![(id, QuerySubmitted("无匹配项".into()))]
        );
    }

    #[test]
    fn insurance_checks_while_popup_open() {
        let (mut h, id) = harness();
        h.tab();
        h.type_text("果");
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn insurance_checks_when_squeezed() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            boxed(),
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(10.0, 10.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tab();
        h.type_text("果");
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
