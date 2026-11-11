use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::list_view::{
    KeyOutcome, ListSelection, ListWindow, MeasureCounter, ScrollAnchor, TYPE_AHEAD_TIMEOUT_MS,
    TypeAhead, find_anchor, list_window, wheel_step_px,
};
use crate::repeater::{MetroRepeater, RepeaterOrientation};

/// MetroList —— 垂直列表。行高下限 40（UWP ListViewItem MinHeight，参 CONTROL_SPEC §7）。
///
/// 虚拟化：经 `list_view::list_window` 只量测视口内行（可见 + 每侧 2 行 overscan，参
/// CONTROL_SPEC §41）——长列表不遍历全部行，避免掉帧；单帧量测次数可用
/// [`MetroList::measures_last_frame`] 读（`list_view.rs` 的验收用例钉住 1000 行时的上界）。
///
/// 行为核心拆在 `list_view.rs`：选中集合 / Shift 锚点 / type-ahead / 滚动锚。
/// **只有这一套列表**，本结构仍是唯一控件（绘制、命中、元素树接入都在本文件）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroList {
    pub rows: Vec<String>,
    /// 焦点行（lead）：键盘光标与最后点击项。单选时它同时是选中项；Ctrl+点到已选项
    /// 会把它从 [`MetroList::selection`] 里去掉而保留焦点（Windows 资源管理器同款）。
    pub selected: Option<usize>,
    /// 悬停行（PointerOver 中性高亮，非强调色，参 CONTROL_SPEC §5 规律 5）。
    pub hovered: Option<usize>,
    /// 整表禁用：行降透明度（CONTROL_SPEC §7 禁用 = 整行 Opacity 0.55）。
    pub disabled: bool,
    /// 滚动偏移（px）。正值 = 内容上移（显示更靠后行）。由滚轮驱动（`scroll_by`）。
    pub scroll: f32,
    /// 行内边距（水平）。UWP 为 12。
    pub padding_x: f32,
    /// 选中集合 + Shift 锚点（多选状态机，参 INTERACTION_CANON §Ⅶ）。
    pub selection: ListSelection,
    /// type-ahead 前缀与超时窗口。
    pub type_ahead: TypeAhead,
    /// 已向树登记 type-ahead 超时定时器（到期在 `update` 里清前缀）。
    typeahead_armed: bool,
    /// 上一帧量测行数（验收计数器；瞬态，不参与相等）。
    measures: MeasureCounter,
}

impl MetroList {
    pub fn new(rows: Vec<String>) -> Self {
        Self {
            rows,
            selected: None,
            hovered: None,
            disabled: false,
            scroll: 0.0,
            padding_x: 12.0,
            selection: ListSelection::default(),
            type_ahead: TypeAhead::default(),
            typeahead_armed: false,
            measures: MeasureCounter::default(),
        }
    }

    /// 单选（清掉多选集合），锚点移到该项。
    pub fn select(&mut self, index: Option<usize>) {
        let index = index.filter(|i| *i < self.rows.len());
        self.selected = index;
        match index {
            Some(i) => self.selection.select_only(i),
            None => self.selection.clear(),
        }
    }

    /// 当前选中项（升序；多选时不止一项）。
    pub fn selected_indices(&self) -> &[usize] {
        self.selection.items()
    }

    pub fn is_selected(&self, index: usize) -> bool {
        self.selection.contains(index)
    }

    /// 上一帧量测行数（虚拟化验收：≤ 可见行数 + 4）。
    pub fn measures_last_frame(&self) -> usize {
        self.measures.get()
    }

    /// 内容总高（行数 × 行高）。
    pub fn content_height(&self, theme: &MetroTheme) -> f32 {
        self.rows.len() as f32 * self.row_height(theme)
    }

    /// 最大滚动偏移：内容总高 − 视口高（不小于 0）。
    pub fn max_scroll(&self, theme: &MetroTheme, viewport_h: f32) -> f32 {
        (self.content_height(theme) - viewport_h).max(0.0)
    }

    /// 按增量滚动（`dy` 正 = 向下，同 `InputEvent::Scroll`）。夹紧到 [0, max]。
    pub fn scroll_by(&mut self, theme: &MetroTheme, viewport_h: f32, dy: f32) {
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll(theme, viewport_h));
    }

    /// 滚动到指定偏移。夹紧到 [0, max]。
    pub fn scroll_to(&mut self, theme: &MetroTheme, viewport_h: f32, offset: f32) {
        self.scroll = offset.clamp(0.0, self.max_scroll(theme, viewport_h));
    }

    /// 按行高算最大滚动偏移（不依赖主题；锚定与验收用）。
    pub fn max_scroll_with(&self, row_h: f32, viewport_h: f32) -> f32 {
        let content = self.rows.len() as f32 * row_h;
        (content - viewport_h).max(0.0)
    }

    /// 视口 / 量测窗口（不计数）：无障碍与命中只认可见段。
    pub fn window(&self, row_h: f32, viewport_h: f32) -> Option<ListWindow> {
        list_window(self.rows.len(), row_h, viewport_h, self.scroll)
    }

    /// 一帧的量测窗口（计数）：本帧量测行数落到 [`MetroList::measures_last_frame`]。
    pub fn measure_window(&self, row_h: f32, viewport_h: f32) -> Option<ListWindow> {
        self.measures.reset();
        let window = self.window(row_h, viewport_h)?;
        self.measures.add(window.rows());
        Some(window)
    }

    /// 点选一行：普通 / Ctrl / Shift 三种模式（INTERACTION_CANON §Ⅶ）。
    ///
    /// 返回是否有可观察变化（lead 或选中集合变了）。`index` 越界返回 false。
    pub fn click_row(&mut self, index: usize, ctrl: bool, shift: bool) -> bool {
        let before_lead = self.selected;
        let before_items = self.selection.items().to_vec();
        let Some(lead) = self.selection.click(index, ctrl, shift, self.rows.len()) else {
            return false;
        };
        self.selected = Some(lead);
        before_lead != self.selected || before_items != self.selection.items()
    }

    /// 把 lead 移到 `index`：`extend` = Shift 连选（锚点不动）。
    fn move_lead(&mut self, index: usize, extend: bool, row_h: f32, viewport_h: f32) -> bool {
        let before_lead = self.selected;
        let before_items = self.selection.items().to_vec();
        if extend {
            let anchor = self
                .selection
                .anchor()
                .or(before_lead)
                .unwrap_or(index)
                .min(self.rows.len().saturating_sub(1));
            self.selection.set_anchor_if_none(anchor);
            self.selection.select_range(anchor, index);
        } else {
            self.selection.select_only(index);
        }
        self.selected = Some(index);
        self.ensure_selected_visible_with(row_h, viewport_h);
        before_lead != self.selected || before_items != self.selection.items()
    }

    /// 分页步长 = 视口高 − 一行（INTERACTION_CANON §Ⅴ PageUp / PageDown）。
    pub fn page_rows(row_h: f32, viewport_h: f32) -> usize {
        if row_h <= 0.0 {
            return 1;
        }
        ((viewport_h / row_h).floor() as usize).saturating_sub(1).max(1)
    }

    /// 滚轮：按格数归一到正典步长（一格 48 px）后夹紧，返回是否真的滚了。
    ///
    /// 到端后返回 false —— 事件层据此不消费，让外层容器接着滚（ScrollChaining），
    /// 同时保证偏移绝不越界 / 不回弹。
    pub fn scroll_wheel(&mut self, dy: f32, steps: f32, row_h: f32, viewport_h: f32) -> bool {
        let before = self.scroll;
        let delta = wheel_step_px(dy, steps);
        self.scroll =
            (self.scroll + delta).clamp(0.0, self.max_scroll_with(row_h, viewport_h));
        self.scroll != before
    }

    /// 键盘：↑↓ / PageUp / PageDown / Home / End / type-ahead。
    ///
    /// 返回是否消费（空列表上的导航键也消费：不该冒泡给外层去挪焦点）。
    /// `now_ms` 由调用方注入（状态转录显式给时刻）；控件在元素树里传 0，
    /// 超时由登记的定时器在 `update` 里清前缀 —— 不读系统钟，等待期不占帧。
    pub fn handle_key(
        &mut self,
        key: kanesumi_element::Key,
        modifiers: kanesumi_element::Modifiers,
        row_h: f32,
        viewport_h: f32,
        now_ms: u64,
    ) -> KeyOutcome {
        use kanesumi_element::Key as K;
        if !matches!(key, K::Char(_)) {
            // 导航键打断 type-ahead（Windows 同款：敲完字按方向键，下一次敲字重开前缀）。
            self.type_ahead.reset();
        }
        if self.rows.is_empty() {
            // 空列表：导航键也消费，否则外层会把它当成「挪焦点」。
            let handled = matches!(
                key,
                K::Down | K::Up | K::Home | K::End | K::PageUp | K::PageDown
            );
            return KeyOutcome { handled, changed: false };
        }
        let last = self.rows.len() - 1;
        let page = Self::page_rows(row_h, viewport_h);
        let extend = modifiers.shift;
        let next = match key {
            K::Down => Some(self.selected.map_or(0, |i| (i + 1).min(last))),
            K::Up => Some(self.selected.map_or(last, |i| i.saturating_sub(1))),
            K::Home => Some(0),
            K::End => Some(last),
            K::PageDown => Some(self.selected.map_or(0, |i| (i + page).min(last))),
            K::PageUp => Some(self.selected.map_or(last, |i| i.saturating_sub(page))),
            K::Char(ch)
                if !modifiers.ctrl
                    && !modifiers.alt
                    && !modifiers.super_key
                    && !ch.is_control() =>
            {
                self.type_ahead.push(ch, now_ms);
                self.type_ahead.matching(&self.rows, self.selected)
            }
            _ => None,
        };
        let Some(index) = next else {
            return KeyOutcome::IGNORED;
        };
        let changed = self.move_lead(index, extend, row_h, viewport_h);
        KeyOutcome { handled: true, changed }
    }

    /// 把 lead 行滚进视口（最小滚动）。已完全可见时不动。
    fn ensure_selected_visible_with(&mut self, row_h: f32, viewport_h: f32) {
        let Some(i) = self.selected else { return };
        let top = i as f32 * row_h;
        let bottom = top + row_h;
        let max = self.max_scroll_with(row_h, viewport_h);
        if top < self.scroll {
            self.scroll = top.clamp(0.0, max);
        } else if bottom > self.scroll + viewport_h {
            self.scroll = (bottom - viewport_h).clamp(0.0, max);
        }
    }

    /// 内容变化：换行数据，并按滚动锚把视口里那一行留在原处（内容变短则夹到末尾）。
    ///
    /// 只按行标签锚定（`rows` 没有稳定 id）：重名行取离旧索引最近的一处；
    /// 锚那行被删掉时退回旧索引夹紧 —— 这是控件能给出的最强保证，不假装有 id。
    pub fn set_rows(&mut self, rows: Vec<String>, row_h: f32, viewport_h: f32) {
        let anchor = self.capture_anchor(row_h);
        self.rows = rows;
        self.restore_anchor(&anchor, row_h, viewport_h);
        self.normalize_selection();
    }

    /// 记下当前视口首行（内容变化前调用）。
    pub fn capture_anchor(&self, row_h: f32) -> ScrollAnchor {
        if self.rows.is_empty() || row_h <= 0.0 {
            return ScrollAnchor {
                top_index: 0,
                within_row: 0.0,
                label: None,
            };
        }
        let top = (self.scroll / row_h).floor().max(0.0) as usize;
        let top = top.min(self.rows.len() - 1);
        ScrollAnchor {
            top_index: top,
            within_row: (self.scroll - top as f32 * row_h).max(0.0),
            label: Some(self.rows[top].clone()),
        }
    }

    /// 内容变化后：把锚行放回视口首行，再夹紧到 [0, max]（不回弹、不留空屏）。
    pub fn restore_anchor(&mut self, anchor: &ScrollAnchor, row_h: f32, viewport_h: f32) {
        if self.rows.is_empty() || row_h <= 0.0 {
            self.scroll = 0.0;
            return;
        }
        let top = anchor
            .label
            .as_deref()
            .and_then(|label| find_anchor(&self.rows, label, anchor.top_index))
            .unwrap_or_else(|| anchor.top_index.min(self.rows.len() - 1));
        let max = self.max_scroll_with(row_h, viewport_h);
        self.scroll = (top as f32 * row_h + anchor.within_row).clamp(0.0, max);
    }

    /// 内容变短后收尾：丢掉越界选中项，lead 夹到最后一行。
    fn normalize_selection(&mut self) {
        let len = self.rows.len();
        self.selection.retain_within(len);
        if len == 0 {
            self.selected = None;
            self.selection.clear();
            return;
        }
        if self.selected.is_some_and(|i| i >= len) {
            let last = len - 1;
            self.selected = Some(last);
            if self.selection.is_empty() {
                self.selection.select_only(last);
            }
        }
        if self.hovered.is_some_and(|i| i >= len) {
            self.hovered = None;
        }
    }

    /// 行高：body 行高 + 上下 8px，下限 40（UWP MinHeight）。下限入尺寸表
    /// `kanesumi_core::sizes::LIST_ROW_MIN_HEIGHT`（N18 列表行高）。
    pub fn row_height(&self, theme: &MetroTheme) -> f32 {
        (theme.typography.body.line_height + 16.0).max(kanesumi_core::sizes::LIST_ROW_MIN_HEIGHT)
    }

    /// 虚拟化布局器（MetroRepeater StackLayout，参 CONTROL_SPEC §41）。
    pub fn virtualizer(&self, theme: &MetroTheme) -> MetroRepeater {
        MetroRepeater {
            item_count: self.rows.len(),
            main_extent: self.row_height(theme),
            layout: crate::repeater::RepeaterLayout::Stack,
            orientation: RepeaterOrientation::Vertical,
            ..MetroRepeater::default()
        }
    }

    /// 渲染到视口 `rect`。顺序：选中底 → 悬停底（两者并存时叠加）→ 行文字。
    /// 只量测视口内行（可见 + 每侧 overscan，计数落 [`MetroList::measures_last_frame`]）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let style = theme.typography.body;
        let row_height = self.row_height(theme);
        let colors = &theme.colors;
        let alpha = if self.disabled {
            0.55 // CONTROL_SPEC §7：禁用 = 整行 Opacity 0.55
        } else {
            1.0
        };

        let Some(window) = self.measure_window(row_height, rect.size.height) else {
            return;
        };
        // 偏移防御性夹紧：直接写 `scroll` 字段（旧 App / 内容变短的那一拍）也不会画出空白帧。
        let scroll = self
            .scroll
            .clamp(0.0, self.max_scroll_with(row_height, rect.size.height));

        // 容器语义 = 裁到自身矩形（2026-09-22 审计 P0-2）：虚拟化只保证「不渲染视口外的行」，
        // 但部分滚出的行（半行）其高亮/文字仍会画到列表 rect 之外。
        scene.push_clip(rect);

        for i in window.span() {
            let y = rect.origin.y - scroll + i as f32 * row_height;
            let row_rect = Rect::new(rect.origin.x, y, rect.size.width, row_height);
            let selected = self.selection.contains(i);
            let hovered = self.hovered == Some(i);

            if selected {
                // 选中高亮 = 强调色 AccentLow（暗 0.6 / 亮 0.4，一手源 themeresources L304/L4220）。
                scene.fill_rect(
                    theme
                        .colors
                        .selection_tint
                        .with_alpha(theme.colors.selection_tint.a * alpha),
                    row_rect,
                );
            }
            if hovered && !self.disabled {
                // 悬停 = 中性高亮。**一手源更正**：ListView 行 PointerOver 用的就是
                // `SystemControlHighlightListLowBrush`（= 通用悬停强度 10%），与 AppBarButton 同值；
                // 旧记录「≈30%」在一手字典里没有对应档（L1783 → L307 → L228）。
                //
                // 与选中**并存**（UWP `PointerOverSelected`）：先画选中强调底，再叠中性悬停。
                // 旧实现是 `else if`，选中行上悬停完全不显形 —— hover 与 selected 分不开。
                scene.fill_rect(theme.indication.hover_tint, row_rect);
            }

            let base = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let fg = base.with_alpha(base.a * alpha);
            let label_rect = Rect::new(
                rect.origin.x + self.padding_x,
                y + 8.0,
                rect.size.width - self.padding_x * 2.0,
                style.line_height,
            );
            scene.label(self.rows[i].clone(), label_rect, fg, style, TextAlign::Left);
        }

        scene.pop_clip();
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// 自绘虚拟化列表：行由控件按数据自画（旧 `render` 只画可见行），**不**把每行变成子节点。
// 旧 API（`select` / `scroll_by` / `scroll_to` / `render`）原样保留给未迁移的 App。
// `Click` 不带坐标 → 点击在 `PointerUp` 用 pos 映射到行；方向键 / Home / End 移动选中并用
// 旧 `scroll_to` 保证选中行可见。滚轮由自身持有偏移，偏移未变（已到端 / 内容不足）时不截停，
// 让外层容器接着滚（XAML ScrollChaining）。

/// 元素树动作：列表选中行改变（点击 / 方向键 / Home / End）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListSelectionChanged(pub usize);

impl MetroList {
    /// 行命中：`pos` 在列表矩形内时按 `scroll` 映射到行索引。
    fn row_at(&self, theme: &MetroTheme, rect: Rect, pos: Point) -> Option<usize> {
        if !rect.contains(pos) {
            return None;
        }
        let row_h = self.row_height(theme);
        if row_h <= 0.0 {
            return None;
        }
        let scroll = self.scroll.clamp(0.0, self.max_scroll_with(row_h, rect.size.height));
        let idx = ((pos.y - rect.origin.y + scroll) / row_h).floor();
        if idx < 0.0 {
            return None;
        }
        let i = idx as usize;
        (i < self.rows.len()).then_some(i)
    }

}

impl kanesumi_element::Widget for MetroList {
    /// 宽取可用宽（无界则 0）；高 = min(内容高, 可用高)，可用高无界时取内容高。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let width = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let content_h = self.content_height(ctx.theme());
        let height = if available.height.is_finite() {
            content_h.min(available.height.max(0.0))
        } else {
            content_h
        };
        kanesumi_core::Size::new(width, height)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // 禁用态由框架维护（框架不给禁用节点投事件）→ 映射进自绘字段后再恢复。
        let saved = self.disabled;
        self.disabled = ctx.state().disabled;
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        self.disabled = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton, ScrollSource};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.hovered = self.row_at(ctx.theme(), rect, *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                modifiers,
                ..
            } => {
                let Some(i) = self.row_at(ctx.theme(), rect, *pos) else {
                    return;
                };
                if self.click_row(i, modifiers.ctrl, modifiers.shift) {
                    ctx.emit(ListSelectionChanged(self.selected.unwrap_or(i)));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            Event::Scroll { dy, source, .. } => {
                let viewport_h = rect.size.height;
                let row_h = self.row_height(ctx.theme());
                // 滚轮按格数归一到正典步长（INTERACTION_CANON §Ⅴ：一格 48 px）；
                // 触控板 / 连续源保留像素精确增量（§Ⅴ「跟手」）。
                let scrolled = match source {
                    ScrollSource::Wheel { steps } => {
                        self.scroll_wheel(*dy, *steps, row_h, viewport_h)
                    }
                    _ => {
                        let before = self.scroll;
                        self.scroll = (self.scroll + *dy)
                            .clamp(0.0, self.max_scroll_with(row_h, viewport_h));
                        self.scroll != before
                    }
                };
                if scrolled {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, modifiers, .. } => {
                let row_h = self.row_height(ctx.theme());
                // 时刻传 0：元素树里的超时由定时器负责（见 `update`），这里不需要钟。
                let outcome = self.handle_key(*key, *modifiers, row_h, rect.size.height, 0);
                if !outcome.handled {
                    return;
                }
                if matches!(key, kanesumi_element::Key::Char(_)) {
                    // 前缀窗口从首字符起算（Tree 定时器是 min 语义，逐键续期要新 API，
                    // 见 `TypeAhead` 注）；到期由 `update` 清前缀。
                    self.typeahead_armed = true;
                    ctx.request_timer(TYPE_AHEAD_TIMEOUT_MS as f64 / 1000.0);
                }
                if outcome.changed {
                    if let Some(i) = self.selected {
                        ctx.emit(ListSelectionChanged(i));
                    }
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    /// type-ahead 超时：定时器到期即清前缀（不读系统钟，等待期不占帧）。
    fn update(&mut self, _ctx: &mut kanesumi_element::UpdateCtx, _dt: f64) {
        if self.typeahead_armed {
            self.typeahead_armed = false;
            self.type_ahead.reset();
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: String::from("列表"),
            value: self.selected.and_then(|i| self.rows.get(i).cloned()),
            checked: None,
        })
    }

    /// 无障碍合成项：把**视口内的行**报成 AT 子项（UIA 虚拟化列表的项 Peer 语义）。
    ///
    /// 只报可见行，和绘制 / 命中用同一个可见窗口（`list_view::list_window` 的可见段）——
    /// 上报看不见的行会让读屏读到屏幕上不存在的条目，契约 5（合成项须落在宿主范围内）
    /// 也会当场抓住它。滚动后上报的行随之改变；overscan 段只量不报。
    fn access_items(
        &self,
        rect: Rect,
        theme: &kanesumi_core::MetroTheme,
    ) -> Vec<kanesumi_element::AccessItem> {
        let row_h = self.row_height(theme);
        let Some(window) = self.window(row_h, rect.size.height) else {
            return Vec::new();
        };
        let scroll = self.scroll.clamp(0.0, self.max_scroll_with(row_h, rect.size.height));
        window
            .visible_span()
            .filter_map(|i| {
                let name = self.rows.get(i)?;
                let y = rect.origin.y - scroll + i as f32 * row_h;
                // 视口边缘的半行：bounds 取与列表矩形的交集（读屏读的是看得见的那段）。
                // 不裁会直接违反契约 5「合成项须在宿主范围内」——
                // 这个洞是 `access_snapshot` 示例跑出来的：列表声明高 120、实际只分到 94，
                // 第三行整行越界。
                let bounds = Rect::new(rect.origin.x, y, rect.size.width, row_h).intersect(rect)?;
                let mut item = kanesumi_element::AccessItem::new(
                    kanesumi_element::AccessRole::ListItem,
                    name.clone(),
                    bounds,
                );
                item.selected = Some(self.selected == Some(i));
                item.disabled = self.disabled;
                Some(item)
            })
            .collect()
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::scroll_view::MetroScrollView;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Stack;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, Modifiers, WidgetId};

    fn harness(rows: usize, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let items = (0..rows).map(|i| format!("行 {i}")).collect();
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(items),
            LayoutProps {
                width: Some(200.0),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn row_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let list = h.tree.get::<MetroList>(id).unwrap();
        let row_h = list.row_height(h.tree.theme());
        let r = h.rect(id);
        Point::new(
            r.origin.x + 10.0,
            r.origin.y - list.scroll + i as f32 * row_h + row_h / 2.0,
        )
    }

    #[test]
    fn click_selects_row_and_reports() {
        let (mut h, id) = harness(10, 100.0);
        h.click_at(row_center(&h, id, 1));
        assert_eq!(
            h.take::<ListSelectionChanged>(),
            vec![(id, ListSelectionChanged(1))]
        );
        assert_eq!(
            h.tree.get::<MetroList>(id).unwrap().selected,
            Some(1)
        );
    }

    #[test]
    fn keyboard_moves_selection_and_scrolls_into_view() {
        let (mut h, id) = harness(10, 100.0);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        for _ in 0..5 {
            assert!(h.key(Key::Down), "方向键应被列表消费");
        }
        let list = h.tree.get::<MetroList>(id).unwrap();
        assert_eq!(list.selected, Some(4), "Down 五次从无选中到第 4 行");
        let row_h = list.row_height(h.tree.theme());
        let top = 4.0 * row_h;
        let bottom = top + row_h;
        assert!(
            list.scroll <= top && bottom <= list.scroll + 100.0,
            "选中行必须滚进视口：scroll={} row=({top},{bottom})",
            list.scroll
        );
        assert!(list.scroll > 0.0, "第 4 行在视口外 → 应滚动");
        h.key(Key::End);
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, Some(9));
        h.key(Key::Home);
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, Some(0));
        assert_eq!(h.take::<ListSelectionChanged>().len(), 7, "5 Down + End + Home");
    }

    #[test]
    fn wheel_bubbles_to_outer_when_not_scrollable() {
        // 列表内容不足一屏（不可滚）→ 滚轮不截停，外层 ScrollView 接着滚（ScrollChaining）。
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        let list = h.tree.insert_with(
            col,
            MetroList::new(vec!["A".into()]),
            LayoutProps {
                height: Some(40.0),
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(
            col,
            crate::button::MetroButton::new("高内容"),
            LayoutProps {
                height: Some(300.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tree.scroll(row_center(&h, list, 0), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(
            h.tree.get::<MetroList>(list).unwrap().scroll,
            0.0,
            "不可滚的列表自身不滚"
        );
        assert!(
            h.tree.get::<MetroScrollView>(sv).unwrap().offset > 0.0,
            "滚动链应冒泡到外层 ScrollView"
        );
    }

    #[test]
    fn wheel_at_end_bubbles_to_outer() {
        // 列表可滚但已到底 → 再向下滚轮不截停，外层接到。
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, Stack::column());
        let list = h.tree.insert_with(
            col,
            MetroList::new((0..10).map(|i| format!("行 {i}")).collect()),
            LayoutProps {
                height: Some(100.0),
                ..LayoutProps::default()
            },
        );
        // 兄弟高内容：让外层 ScrollView 可滚，列表到底后才能冒泡给它。
        h.tree.insert_with(
            col,
            crate::button::MetroButton::new("高内容"),
            LayoutProps {
                height: Some(300.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        let c = h.rect(list).center();
        // 先滚到列表底部。
        for _ in 0..20 {
            h.tree.scroll(c, 0.0, 50.0, Modifiers::NONE);
        }
        h.frame();
        let at_end = h.tree.get::<MetroList>(list).unwrap().scroll;
        assert!(at_end > 0.0 && at_end >= h.tree.get::<MetroList>(list).unwrap().max_scroll(h.tree.theme(), 100.0) - 0.01);
        // 到底后再滚：列表偏移不变，滚动链冒泡到外层。
        h.tree.scroll(c, 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(h.tree.get::<MetroList>(list).unwrap().scroll, at_end);
        let off = h.tree.get::<MetroScrollView>(sv).unwrap().offset;
        assert!(
            off > 0.0,
            "到底后滚动链应冒泡，列表 scroll={at_end} 外层 offset={off}"
        );
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(10, 100.0);
        assert_eq!(h.rect(id).size.width, 200.0);
        assert_eq!(h.rect(id).size.height, 100.0, "高 = min(内容高 400, 可用 100)");
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(vec!["很长很长很长的一行文本".into(); 10]),
            LayoutProps {
                width: Some(40.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(10, 100.0);
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(row_center(&h, id, 1));
        assert!(h.take::<ListSelectionChanged>().is_empty());
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::{Scene, SceneCommand};

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

    fn render(rows: Vec<String>, selected: Option<usize>, scroll: f32, rect: Rect) -> Scene {
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(rows);
        list.select(selected);
        list.scroll = scroll;
        let mut scene = Scene::default();
        list.render(&theme, &engine, rect, &mut scene);
        scene
    }

    #[test]
    fn emits_row_texts() {
        if !font_available() {
            return;
        }
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            None,
            0.0,
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 3);
    }

    #[test]
    fn selection_highlights_row() {
        if !font_available() {
            return;
        }
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            Some(1),
            0.0,
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let highlights = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(highlights, 1, "只有选中行有高亮底");
    }

    #[test]
    fn scroll_skips_out_of_view_rows() {
        if !font_available() {
            return;
        }
        // 100 行、偏移 200（首行 = 第 5 行）：前面的行不进量测窗口。
        let rows: Vec<String> = (0..100).map(|i| format!("行 {i}")).collect();
        let scene = render(rows, None, 200.0, Rect::new(0.0, 0.0, 200.0, 200.0));
        let texts: Vec<String> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(!texts.iter().any(|t| t == "行 0"), "首行已滚出视口");
        assert!(texts.iter().any(|t| t == "行 5"), "视口首行在量测窗口内");
        assert!(texts.len() <= 12, "量测窗口 = 可见 + overscan，实际 {}", texts.len());
    }

    /// 偏移越过内容末尾：按最大偏移夹紧后照常画，不留空白帧。
    ///
    /// 旧实现直接 `visible_range` → `None`，整表空白；内容变短的那一拍（夹紧尚未跑）
    /// 会闪黑。现在渲染用夹紧后的偏移，`set_rows` 的锚定再把状态夹回合法范围。
    #[test]
    fn overscroll_clamps_to_tail_instead_of_blank() {
        if !font_available() {
            return;
        }
        let scene = render(
            vec!["Alpha".into(), "Beta".into(), "Gamma".into()],
            None,
            500.0,
            Rect::new(0.0, 0.0, 200.0, 100.0),
        );
        let texts: Vec<String> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts.len(), 3, "夹到 max 偏移（20）后三行仍在窗口内");
        assert_eq!(texts.last().map(String::as_str), Some("Gamma"));
    }

    #[test]
    fn select_out_of_range_is_none() {
        let mut list = MetroList::new(vec!["A".into()]);
        list.select(Some(5));
        assert_eq!(list.selected, None);
    }

    #[test]
    fn scroll_by_clamps_to_content() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(), "B".into(), "C".into()]);
        let viewport_h = 100.0;
        let max = list.max_scroll(&theme, viewport_h);
        // 3 行 × 40 = 120，视口 100 → max 20
        assert_eq!(max, 20.0);

        list.scroll_by(&theme, viewport_h, 10.0);
        assert_eq!(list.scroll, 10.0);
        // 向下滚超出 → 夹紧到 max
        list.scroll_by(&theme, viewport_h, 100.0);
        assert_eq!(list.scroll, max);
        // 向上滚超出 → 夹紧到 0
        list.scroll_by(&theme, viewport_h, -100.0);
        assert_eq!(list.scroll, 0.0);
    }

    #[test]
    fn scroll_to_clamps_negative() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(); 10]);
        list.scroll_to(&theme, 200.0, -5.0);
        assert_eq!(list.scroll, 0.0);
        list.scroll_to(&theme, 200.0, 9999.0);
        assert_eq!(list.scroll, list.max_scroll(&theme, 200.0));
    }

    #[test]
    fn no_scroll_when_content_fits() {
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["A".into(), "B".into()]);
        list.scroll_by(&theme, 300.0, 50.0);
        assert_eq!(list.scroll, 0.0, "内容不足一屏不滚动");
    }

    #[test]
    fn hover_highlights_neutral_not_accent() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["Alpha".into(), "Beta".into()]);
        list.hovered = Some(1);
        let mut scene = Scene::default();
        list.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &mut scene,
        );
        // 悬停行用中性高亮（on_surface 白 30%），不用强调色
        let hover_fills = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, .. } if color.r > 0.0 => Some(color),
                _ => None,
            })
            .count();
        assert_eq!(hover_fills, 1, "只有悬停行有中性高亮");
    }

    #[test]
    fn disabled_lowers_row_alpha() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["Alpha".into()]);
        list.disabled = true;
        let mut scene = Scene::default();
        list.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 200.0, 200.0),
            &mut scene,
        );
        // 首命令现在是成对 PushClip（容器裁剪，P0-2），故按类型查行文本命令。
        let Some(SceneCommand::Text { color, .. }) = scene
            .commands
            .iter()
            .find(|c| matches!(c, SceneCommand::Text { .. }))
        else {
            panic!("应有行文本命令");
        };
        assert!(color.a < 1.0, "禁用态行文字应降透明度，实际 a={}", color.a);
    }

    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Key, LayoutProps, Modifiers, WidgetId};

    /// 行内一点（测试点击 / 悬停用）。
    fn row_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let list = h.tree.get::<MetroList>(id).unwrap();
        let row_h = list.row_height(h.tree.theme());
        let r = h.rect(id);
        Point::new(
            r.origin.x + 10.0,
            r.origin.y - list.scroll + i as f32 * row_h + row_h / 2.0,
        )
    }

    /// 列表夹具（本模块独立，不复用 `tree_tests::harness`）。
    fn list_harness(rows: usize, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let items = (0..rows).map(|i| format!("行 {i}")).collect();
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(items),
            LayoutProps {
                width: Some(200.0),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    /// 无障碍合成项：只报视口内的行，选中态跟着走（UIA 虚拟化列表的项 Peer）。
    #[test]
    fn access_items_report_visible_rows_only() {
        // 可见高 120、行高 ≥40 → 视口内约 3 行；共 50 行。
        let (h, id) = list_harness(50, 120.0);
        let snap = h.access_snapshot();
        let node = snap.find(id).expect("列表在快照内");
        assert!(
            !node.items.is_empty() && node.items.len() < 8,
            "只报视口内的行（{} 项，共 50 行）",
            node.items.len()
        );
        assert_eq!(node.items[0].name, "行 0");
        assert_eq!(
            node.items[0].role,
            kanesumi_element::AccessRole::ListItem
        );
        assert!(node.items.iter().all(|i| !*i.selected.as_ref().unwrap_or(&true)), "未选中");
        h.assert_access_contract();
    }

    /// 视口边缘的半行：合成项 bounds 必须裁到列表矩形内（契约 5）。
    ///
    /// 这条是 `access_snapshot` 示例跑出来的：列表声明高 120、实际只分到 94，
    /// 第三行整行越界 → 契约当场报违例。断言里刻意用「不整除」的高度。
    #[test]
    fn access_items_clip_partial_rows_at_viewport_edge() {
        let (h, id) = list_harness(3, 94.344826);
        let snap = h.access_snapshot();
        let node = snap.find(id).expect("列表在快照内");
        assert_eq!(node.items.len(), 3, "三行都在视口内（最后一行只露一部分）");
        for item in &node.items {
            assert!(
                item.bounds.origin.y >= node.bounds.origin.y - 0.5
                    && item.bounds.bottom() <= node.bounds.bottom() + 0.5,
                "合成项裁到列表矩形内：{} {:?} ⊄ {:?}",
                item.name,
                item.bounds,
                node.bounds
            );
        }
        assert!(
            node.items[2].bounds.size.height < 40.0,
            "第三行只剩可见那段：{:?}",
            node.items[2].bounds
        );
        h.assert_access_contract();
    }
    /// 滚动后上报的行随视口改变，且选中行带 selected=true。
    #[test]
    fn access_items_follow_scroll_and_selection() {
        let (mut h, id) = list_harness(50, 120.0);
        h.tree.edit::<MetroList, _>(id, |l, _| {
            l.select(Some(10));
            l.scroll = 400.0;
        });
        h.frame();
        let snap = h.access_snapshot();
        let node = snap.find(id).expect("列表在快照内");
        assert!(
            node.items.iter().all(|i| i.name != "行 0"),
            "滚出视口的行不再上报：{:?}",
            node.items.iter().map(|i| &i.name).collect::<Vec<_>>()
        );
        assert!(
            node.items.iter().any(|i| i.selected == Some(true)),
            "选中行（滚动后进入视口）带 selected"
        );
        h.assert_access_contract();
    }

    // ── 虚拟化量测 / 滚轮 / 多选 / type-ahead（元素树路径）────────────────────

    /// 1000 行、视口 300：真实一帧只量测可见 8 行 + overscan（≤ 4），不是 1000。
    #[test]
    fn frame_measures_only_visible_plus_overscan_for_1000_rows() {
        let (h, id) = list_harness(1000, 300.0);
        let list = h.tree.get::<MetroList>(id).unwrap();
        let row_h = list.row_height(h.tree.theme());
        let window = list.window(row_h, 300.0).expect("有窗口");
        let measured = list.measures_last_frame();
        assert_eq!(measured, window.rows(), "计数器 = 本帧量测窗口");
        assert!(
            measured <= window.visible_rows() + 2 * crate::list_view::LIST_OVERSCAN_ROWS,
            "单帧量测 {measured} 应 ≤ 可见 {} + overscan 4",
            window.visible_rows()
        );
        assert!(measured <= 12, "量测 {measured} 行；1000 行列表不该逐项量");
    }

    /// 滚轮事件按正典归一：外壳旧值 50 px/格 → 48 px/格（INTERACTION_CANON §Ⅴ）。
    #[test]
    fn wheel_notch_normalizes_to_canon_48px() {
        let (mut h, id) = list_harness(100, 200.0);
        let c = h.rect(id).center();
        h.tree.scroll(c, 0.0, 50.0, Modifiers::NONE);
        h.frame();
        assert_eq!(
            h.tree.get::<MetroList>(id).unwrap().scroll,
            48.0,
            "一格 = 3 行 × 16 = 48 px"
        );
    }

    /// Ctrl / Shift 点击经元素树投递，修饰键一路到选择状态机。
    #[test]
    fn ctrl_and_shift_clicks_reach_selection() {
        use kanesumi_element::PointerButton;
        let (mut h, id) = list_harness(10, 200.0);
        let click = |h: &mut TestHarness, i: usize, m: Modifiers| {
            let p = row_center(h, id, i);
            h.tree.pointer_move(p);
            h.tree.pointer_down(p, PointerButton::Left, m);
            h.tree.pointer_up(p, PointerButton::Left, m);
            h.frame();
        };
        click(&mut h, 0, Modifiers::NONE);
        click(&mut h, 2, Modifiers { ctrl: true, ..Modifiers::NONE });
        let list = h.tree.get::<MetroList>(id).unwrap();
        assert_eq!(list.selected_indices(), &[0, 2], "Ctrl 点选追加");
        click(&mut h, 4, Modifiers { shift: true, ..Modifiers::NONE });
        let list = h.tree.get::<MetroList>(id).unwrap();
        assert_eq!(list.selected_indices(), &[2, 3, 4], "Shift 从锚点 2 连选");
        assert_eq!(list.selected, Some(4), "焦点行落在点击项");
    }

    /// type-ahead 超时由树的定时器负责：等待期满后前缀重开。
    #[test]
    fn type_ahead_expires_via_tree_timer() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroList::new(vec!["Beta".into(), "Echo".into()]),
            LayoutProps {
                width: Some(200.0),
                height: Some(200.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Char('b'));
        assert_eq!(h.tree.get::<MetroList>(id).unwrap().selected, Some(0));
        h.idle(1.2);
        h.key(Key::Char('e'));
        assert_eq!(
            h.tree.get::<MetroList>(id).unwrap().selected,
            Some(1),
            "超时后 'e' 重开前缀 → Echo；旧前缀「be」会停在 Beta"
        );
    }

    /// 悬停与选中并存时画两层：选中强调底 + 中性悬停（hover ≠ selected）。
    #[test]
    fn hover_and_selected_render_two_distinct_washes() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut list = MetroList::new(vec!["Alpha".into(), "Beta".into(), "Gamma".into()]);
        list.select(Some(1));
        list.hovered = Some(1);
        let mut scene = Scene::default();
        list.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 200.0), &mut scene);
        let fills: Vec<kanesumi_core::Color> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(fills.len(), 2, "选中行悬停：选中底 + 悬停底");
        let near = |a: kanesumi_core::Color, b: kanesumi_core::Color| {
            (a.r - b.r).abs() < 1e-3
                && (a.g - b.g).abs() < 1e-3
                && (a.b - b.b).abs() < 1e-3
        };
        assert!(
            fills
                .iter()
                .any(|c| near(*c, theme.colors.selection_tint)),
            "一层是选中强调底"
        );
        assert!(
            fills.iter().any(|c| near(*c, theme.indication.hover_tint)),
            "另一层是中性悬停底"
        );
    }
}
