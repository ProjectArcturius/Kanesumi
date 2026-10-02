// MetroCalendarDatePicker —— 日期选择器（UWP `CalendarDatePicker` 移植，元素树实现）。
//
// 一手源：`generic.xaml` 的 `CalendarDatePicker` 模板（`BorderThickness` 2、MinHeight 32、
// 日历字形 E787 12px 列宽 32、`DateText` Padding `12,0,0,2`）与 `themeresources.xaml` 的
// `CalendarDatePicker*` 笔刷键；弹层里放一个 `MetroCalendarView`。参 docs/CONTROL_SPEC.md
// 「CalendarDatePicker」。
//
// 元素树里弹层是覆盖层上的独立节点（参 docs/ELEMENT_MIGRATION.md §8）：触发器只负责打开与
// 「展开中外观」，「今天 / 选中」由调用方传入；选中后面板发 `DatePicked { owner, date }` 并
// 关闭自身，框架把焦点交还触发器。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Rect, Size, TextStyle};
use kanesumi_element::{
    Event, Key, MeasureCtx, PaintCtx, PointerButton, PopupSpec, Widget, WidgetId,
};

use crate::calendar_view::{CalendarResponse, Date, MetroCalendarView};
use crate::popup::popup_gap;
use crate::state::{ControlState, control_state};

/// 触发器最小高（UWP `MinHeight` 32）。
pub const PICKER_MIN_H: f32 = 32.0;
/// 边框粗细（UWP `CalendarDatePickerBorderThemeThickness` 2）。
pub const PICKER_BORDER: f32 = 2.0;
/// 日历字形区宽（模板第 3 列 `Width="32"`）。
pub const PICKER_GLYPH_W: f32 = 32.0;
/// 未选占位文本。
pub const PICKER_PLACEHOLDER: &str = "选择日期";

/// 元素树动作：面板选中了某日。来源 id 是弹层自身；按 `owner` 区分是哪个触发器（同 §8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DatePicked {
    /// 打开弹层的触发器。
    pub owner: WidgetId,
    pub date: Date,
}

/// MetroCalendarDatePicker —— 日期选择器。参 CONTROL_SPEC「CalendarDatePicker」。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroCalendarDatePicker {
    /// 当前选中日（None = 显示占位）。
    pub selected: Option<Date>,
    /// 「今天」（调用方传入）。
    pub today: Date,
    pub placeholder: String,
    /// 旧路径交互状态。
    pub state: ControlState,
    /// 元素树下当前展开的日历弹层（旧路径不用）。
    tree_popup: Option<WidgetId>,
}

impl MetroCalendarDatePicker {
    pub fn new(today: Date) -> Self {
        Self {
            selected: None,
            today,
            placeholder: PICKER_PLACEHOLDER.into(),
            state: ControlState::Normal,
            tree_popup: None,
        }
    }

    /// 指定初始选中日。
    #[must_use]
    pub fn with_selected(mut self, date: Date) -> Self {
        self.selected = Some(date);
        self
    }

    pub fn set_selected(&mut self, date: Date) {
        self.selected = Some(date);
    }

    /// 触发器显示文本：选中 = 「YYYY年M月D日」，否则占位。
    pub fn display_text(&self) -> String {
        match self.selected {
            Some(d) => format!("{}年{}月{}日", d.year, d.month, d.day),
            None => self.placeholder.clone(),
        }
    }

    /// 固有尺寸：文本宽 + 左 12 + 右 8 + 字形区 32；高 ≥ 32。
    pub fn measure(&self, engine: &TextEngine, style: TextStyle) -> Size {
        let text_w = engine.measure(&self.display_text(), style.size);
        let width = text_w + 12.0 + 8.0 + PICKER_GLYPH_W;
        let height = PICKER_MIN_H.max(style.line_height + 12.0);
        Size::new(width, height)
    }

    /// 渲染触发器。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let indication = &theme.indication;
        let style = theme.typography.body;

        // 底：标准控件填充；悬停 / 展开浅底；禁用降不透明。
        let bg = match self.state {
            ControlState::Disabled => colors
                .control_fill
                .with_alpha(colors.control_fill.a * indication.disabled_opacity),
            ControlState::Hovered | ControlState::Pressed => colors.surface_variant,
            _ => colors.control_fill,
        };
        scene.fill_rounded_rect(bg, rect, theme.tokens.corner_radius);
        // 边框（UWP 2px）。
        scene.stroke_rounded_rect(
            colors.control_stroke,
            rect,
            PICKER_BORDER,
            theme.tokens.corner_radius,
        );
        if self.state == ControlState::Focused {
            crate::focus::draw_focus_ring(
                scene,
                indication.focus_stroke,
                theme.scheme,
                rect,
                theme.tokens.corner_radius,
            );
        }

        // 文本：未选 = BaseMedium（占位）；已选 = BaseHigh。
        let text_rect = Rect::new(
            rect.origin.x + 12.0,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0,
            (rect.size.width - 12.0 - PICKER_GLYPH_W).max(0.0),
            style.line_height,
        );
        let fg = if self.selected.is_some() {
            colors.on_surface
        } else {
            colors.on_surface_variant
        };
        scene.label(self.display_text(), text_rect, fg, style, TextAlign::Left);

        // 日历字形（E787 的 Kanesumi 自绘近似：外框 + 顶栏 + 两挂环）。
        let glyph = Rect::new(
            rect.right() - PICKER_GLYPH_W + (PICKER_GLYPH_W - 16.0) / 2.0,
            rect.origin.y + (rect.size.height - 16.0) / 2.0,
            16.0,
            16.0,
        );
        calendar_glyph(scene, glyph, colors.on_surface_variant);
    }
}

/// 日历字形：外框描边 + 顶部横条 + 两个挂环（全部落在 `rect` 内）。
fn calendar_glyph(scene: &mut Scene, rect: Rect, color: Color) {
    scene.stroke_rounded_rect(color, rect, 1.0, CornerRadius::Slight);
    let bar_h = 1.5;
    scene.fill_rect(
        color,
        Rect::new(
            rect.origin.x,
            rect.origin.y + rect.size.height * 0.30,
            rect.size.width,
            bar_h,
        ),
    );
    let tick_w = rect.size.width * 0.16;
    for fx in [
        rect.origin.x + rect.size.width * 0.26,
        rect.origin.x + rect.size.width * 0.74 - tick_w,
    ] {
        scene.fill_rect(color, Rect::new(fx, rect.origin.y + 1.0, tick_w, 3.0));
    }
}

/// 弹层面板：一个 `MetroCalendarView` + `owner`。面板是覆盖层上的独立节点。
struct CalendarPanel {
    owner: WidgetId,
    view: MetroCalendarView,
}

impl CalendarPanel {
    fn new(owner: WidgetId, today: Date, selected: Option<Date>) -> Self {
        let mut view = MetroCalendarView::new(today);
        if let Some(date) = selected {
            view = view.with_selected(date);
        }
        Self { owner, view }
    }

    /// 交互结果：选中则发 `DatePicked` 并关闭自身；否则重画。
    fn after(&mut self, ctx: &mut kanesumi_element::EventCtx, response: CalendarResponse) {
        if let CalendarResponse::DateSelected(date) = response {
            ctx.emit(DatePicked {
                owner: self.owner,
                date,
            });
            ctx.close_popup(ctx.id());
        } else {
            ctx.invalidate_paint();
        }
    }
}

impl Widget for CalendarPanel {
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        self.view.measure(ctx.engine())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        scene.fill_rounded_rect(theme.colors.surface, ctx.rect(), theme.tokens.corner_radius);
        self.view.focused = ctx.state().focused;
        self.view.render(&theme, ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &Event) {
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.view.hover(rect, *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.view.hovered = None;
                self.view.hovered_nav = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let response = self.view.click(rect, *pos);
                self.after(ctx, response);
                ctx.set_handled();
            }
            Event::Scroll { dy, .. } => {
                self.view.scroll(*dy);
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => {
                // Esc / Tab 留给框架（关闭弹层 / 焦点遍历）。
                if matches!(key, Key::Tab | Key::Escape) {
                    return;
                }
                let response = self.view.key(*key);
                self.after(ctx, response);
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn focus_visual(&self) -> bool {
        false
    }
}

// ── 元素树接入（弹层类，参 docs/ELEMENT_MIGRATION.md §8）──────────────────────────

impl Widget for MetroCalendarDatePicker {
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        MetroCalendarDatePicker::measure(self, ctx.engine(), ctx.theme().typography.body)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let saved = self.state;
        // 展开期间保持「按下」外观（对齐 DropDownButton）。
        self.state = if self.tree_popup.is_some() && !ctx.state().disabled {
            ControlState::Pressed
        } else {
            control_state(ctx.state())
        };
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), ctx.rect(), scene);
        self.state = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &Event) {
        let keyboard = match event {
            Event::Click => false,
            Event::KeyDown {
                key: Key::Enter | Key::Char(' ') | Key::Down,
                ..
            } => true,
            Event::PopupClosed { popup } => {
                if self.tree_popup == Some(*popup) {
                    self.tree_popup = None;
                    ctx.invalidate_paint();
                }
                return;
            }
            _ => return,
        };
        // 已展开时再次激活 = 收起。
        match self.tree_popup.take() {
            Some(popup) => ctx.close_popup(popup),
            None => {
                let panel = CalendarPanel::new(ctx.id(), self.today, self.selected);
                let id = ctx.open_popup(
                    panel,
                    PopupSpec {
                        anchor: Some(ctx.id()),
                        gap: popup_gap(),
                        ..PopupSpec::default()
                    },
                );
                ctx.focus_widget(id, keyboard);
                self.tree_popup = Some(id);
            }
        }
        ctx.invalidate_paint();
        ctx.set_handled();
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Button,
            name: self.placeholder.clone(),
            value: self
                .selected
                .map(|d| format!("{}年{}月{}日", d.year, d.month, d.day)),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(520.0, 520.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCalendarDatePicker::new(Date::new(2026, 10, 1)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(10.0, 10.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_opens_below_and_selection_reports_owner() {
        let (mut h, id) = harness();
        h.click(id);
        let p = h.tree.popups().next().expect("点击打开日历弹层");
        assert!(
            h.rect(p).origin.y >= h.rect(id).bottom(),
            "弹层在触发器下方"
        );
        let cell = {
            let panel = h.tree.get::<CalendarPanel>(p).unwrap();
            panel
                .view
                .day_rect(h.rect(p), Date::new(2026, 10, 15))
                .expect("日格在面板内")
                .center()
        };
        h.click_at(cell);
        let acts = h.take::<DatePicked>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1.owner, id, "动作带 owner");
        assert_eq!(acts[0].1.date, Date::new(2026, 10, 15));
        assert!(h.tree.popups().next().is_none(), "选中后弹层关闭");
        assert_eq!(h.tree.focused(), Some(id), "焦点回触发器");
        // 触发器展开复位。
        assert!(
            h.tree
                .get::<MetroCalendarDatePicker>(id)
                .unwrap()
                .tree_popup
                .is_none()
        );
    }

    #[test]
    fn click_again_closes_and_resets_state() {
        let (mut h, id) = harness();
        h.click(id);
        h.click(id);
        assert!(h.tree.popups().next().is_none());
        assert!(
            h.tree
                .get::<MetroCalendarDatePicker>(id)
                .unwrap()
                .tree_popup
                .is_none()
        );
    }

    #[test]
    fn keyboard_opens_with_focus_inside_and_esc_returns() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Enter);
        let p = h.tree.popups().next().expect("键盘打开弹层");
        assert_eq!(h.tree.focused(), Some(p), "键盘打开焦点进面板");
        h.key(Key::Escape);
        assert!(h.tree.popups().next().is_none());
        assert_eq!(h.tree.focused(), Some(id), "Esc 关闭后焦点回触发器");
    }

    #[test]
    fn placeholder_until_selected() {
        let mut h = TestHarness::new(400.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCalendarDatePicker::new(Date::new(2026, 10, 1)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let w = h.tree.get::<MetroCalendarDatePicker>(id).unwrap();
        assert_eq!(w.display_text(), PICKER_PLACEHOLDER);

        let mut h2 = TestHarness::new(400.0, 200.0);
        let id2 = h2.tree.insert_with(
            h2.root(),
            MetroCalendarDatePicker::new(Date::new(2026, 10, 1))
                .with_selected(Date::new(2026, 3, 8)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h2.frame();
        assert_eq!(
            h2.tree
                .get::<MetroCalendarDatePicker>(id2)
                .unwrap()
                .display_text(),
            "2026年3月8日"
        );
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, id) = harness();
        h.click(id);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click(id);
        assert!(h.tree.popups().next().is_none());
    }
}
