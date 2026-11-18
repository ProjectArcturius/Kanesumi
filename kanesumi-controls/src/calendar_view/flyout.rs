//! 与时钟入口配套的单层月历，参 CONTROL_SPEC §45。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::sizes::calendar_flyout as sizes;
use kanesumi_core::sizes::calendar_flyout::{
    CLOCK_TOP, DATE_GAP, DATE_H, GRID_BOTTOM, LINK_H, MONTH_BOTTOM, MONTH_H, MONTH_TOP, NAV_W, PAD,
    WEEK_H,
};
use kanesumi_core::{Color, FontWeight, MetroTheme, Point, Rect, TextStyle};

use super::{CAL_COLS, CAL_ROWS, Date, MetroCalendarView};

const CLOCK_STYLE: TextStyle = TextStyle::new(44.0, 48.0, FontWeight::Light);

/// 时钟弹层的布局。月历绘制和日期命中都使用这些坐标。
#[derive(Debug, Clone, Copy)]
pub struct CalendarFlyoutLayout {
    pub time: Rect,
    pub date: Rect,
    pub month_header: Rect,
    pub grid_top: f32,
    pub link: Rect,
    grid_left: f32,
    week_top: f32,
}

impl CalendarFlyoutLayout {
    pub const WIDTH: f32 = sizes::WIDTH;
    pub const GRID_PAD: f32 = sizes::GRID_PAD;
    pub const CELL_W: f32 = (Self::WIDTH - 2.0 * Self::GRID_PAD) / CAL_COLS as f32;
    pub const CELL_H: f32 = sizes::CELL_H;
    pub const HEIGHT: f32 = CLOCK_TOP
        + CLOCK_STYLE.line_height
        + DATE_GAP
        + DATE_H
        + MONTH_TOP
        + MONTH_H
        + MONTH_BOTTOM
        + WEEK_H
        + CAL_ROWS as f32 * Self::CELL_H
        + GRID_BOTTOM
        + LINK_H;

    pub fn of(rect: Rect) -> Self {
        let left = rect.origin.x + PAD;
        let inner_w = (rect.size.width - 2.0 * PAD).max(0.0);
        let time = Rect::new(
            left,
            rect.origin.y + CLOCK_TOP,
            inner_w,
            CLOCK_STYLE.line_height,
        );
        let date = Rect::new(left, time.bottom() + DATE_GAP, inner_w, DATE_H);
        let month_header = Rect::new(left, date.bottom() + MONTH_TOP, inner_w, MONTH_H);
        let week_top = month_header.bottom() + MONTH_BOTTOM;
        let grid_top = week_top + WEEK_H;
        let link = Rect::new(
            rect.origin.x,
            grid_top + CAL_ROWS as f32 * Self::CELL_H + GRID_BOTTOM,
            rect.size.width,
            LINK_H,
        );
        Self {
            time,
            date,
            month_header,
            grid_top,
            link,
            grid_left: rect.origin.x + Self::GRID_PAD,
            week_top,
        }
    }

    /// 上月和下月的点按区。
    pub fn nav_rects(self) -> (Rect, Rect) {
        let h = self.month_header;
        (
            Rect::new(h.origin.x, h.origin.y, NAV_W, MONTH_H),
            Rect::new(h.right() - NAV_W, h.origin.y, NAV_W, MONTH_H),
        )
    }

    fn cell(self, index: usize) -> Rect {
        Rect::new(
            self.grid_left + (index % CAL_COLS) as f32 * Self::CELL_W,
            self.grid_top + (index / CAL_COLS) as f32 * Self::CELL_H,
            Self::CELL_W,
            Self::CELL_H,
        )
    }

    fn nav_at(self, pos: Point) -> Option<i32> {
        let (prev, next) = self.nav_rects();
        if prev.contains(pos) {
            Some(-1)
        } else if next.contains(pos) {
            Some(1)
        } else {
            None
        }
    }
}

/// 由调用方更新的时钟、今日和设置入口文案。
pub struct CalendarFlyoutContent<'a> {
    pub time: &'a str,
    pub date: &'a str,
    pub today: Date,
    pub link: &'a str,
    pub link_hover: bool,
}

/// 复用 CalendarView 的月份与选中状态，保留时钟弹层的显示和点按契约。
pub struct MetroCalendarFlyout {
    view: MetroCalendarView,
    pressed: Option<i32>,
}

impl MetroCalendarFlyout {
    pub fn new(displayed: Date) -> Self {
        Self {
            view: MetroCalendarView::new(displayed),
            pressed: None,
        }
    }

    pub fn displayed(&self) -> Date {
        self.view.displayed
    }

    pub fn selected(&self) -> Option<Date> {
        self.view.selected
    }

    pub fn header_text(&self) -> String {
        self.view.header_text()
    }

    fn month_days(&self) -> impl Iterator<Item = (usize, Date)> + '_ {
        let displayed = self.view.displayed;
        self.view
            .month_grid()
            .into_iter()
            .enumerate()
            .filter(move |(_, day)| day.year == displayed.year && day.month == displayed.month)
    }

    /// 月外日期与空白格没有可命中的矩形。
    pub fn day_rect(&self, layout: CalendarFlyoutLayout, date: Date) -> Option<Rect> {
        self.month_days()
            .find(|(_, day)| *day == date)
            .map(|(index, _)| layout.cell(index))
    }

    pub fn day_at(&self, layout: CalendarFlyoutLayout, pos: Point) -> Option<Date> {
        self.month_days()
            .find(|(index, _)| layout.cell(*index).contains(pos))
            .map(|(_, date)| date)
    }

    pub fn select_at(&mut self, layout: CalendarFlyoutLayout, pos: Point) -> bool {
        let Some(date) = self.day_at(layout, pos) else {
            return false;
        };
        self.view.select_day(date);
        true
    }

    pub fn press_month(&mut self, layout: CalendarFlyoutLayout, pos: Point) -> bool {
        self.pressed = layout.nav_at(pos);
        self.pressed.is_some()
    }

    /// 在同一个翻月按钮上释放时返回改变后的月份。
    pub fn release_month(&mut self, layout: CalendarFlyoutLayout, pos: Point) -> Option<Date> {
        let delta = self.pressed.take()?;
        if layout.nav_at(pos) != Some(delta) {
            return None;
        }
        self.view.shift_month(delta);
        Some(self.view.displayed)
    }

    pub fn cancel_press(&mut self) {
        self.pressed = None;
    }

    pub fn render(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        layout: CalendarFlyoutLayout,
        content: CalendarFlyoutContent<'_>,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        scene.text(
            content.time.to_owned(),
            layout.time,
            colors.on_surface,
            CLOCK_STYLE,
            TextAlign::Left,
        );
        scene.text(
            content.date.to_owned(),
            layout.date,
            colors.primary,
            theme.typography.caption,
            TextAlign::Left,
        );
        scene.text(
            self.header_text(),
            layout.month_header,
            colors.on_surface,
            theme.typography.body_medium,
            TextAlign::Center,
        );
        let (prev, next) = layout.nav_rects();
        draw_arrow(scene, prev.center(), 4.0, colors.on_surface);
        draw_arrow(scene, next.center(), -4.0, colors.on_surface);
        self.render_days(theme, engine, layout, content.today, scene);
        if content.link_hover {
            scene.fill_rect(theme.indication.hover_tint, layout.link);
        }
        let body = theme.typography.body_medium;
        let link_text = Rect::new(
            layout.link.origin.x + PAD,
            layout.link.origin.y + 10.0,
            (layout.link.size.width - 2.0 * PAD).max(0.0),
            body.line_height,
        );
        scene.text(
            content.link.to_owned(),
            link_text,
            colors.primary,
            body,
            TextAlign::Left,
        );
    }

    fn render_days(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        layout: CalendarFlyoutLayout,
        today: Date,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let weekdays = ["一", "二", "三", "四", "五", "六", "日"];
        let small = theme.typography.body_small;
        for (i, weekday) in weekdays.into_iter().enumerate() {
            let cell = Rect::new(
                layout.cell(i).origin.x,
                layout.week_top,
                CalendarFlyoutLayout::CELL_W,
                WEEK_H,
            );
            scene.text(
                weekday.to_owned(),
                centered_text(engine, cell, weekday, small),
                colors.on_surface_variant,
                small,
                TextAlign::Left,
            );
        }

        let caption = theme.typography.caption;
        for (index, day) in self.month_days() {
            let cell = layout.cell(index);
            let color = if day == today {
                scene.fill_rect(colors.primary, cell);
                // 时钟预览的今日数字使用白字，兼容原来的时钟弹层。
                Color::WHITE
            } else {
                if self.view.selected == Some(day) {
                    scene.fill_rect(colors.surface_variant, cell);
                }
                colors.on_surface
            };
            let text = day.day.to_string();
            let bounds = centered_text(engine, cell, &text, caption);
            scene.text(text, bounds, color, caption, TextAlign::Left);
        }
    }
}

fn centered_text(engine: &TextEngine, cell: Rect, label: &str, style: TextStyle) -> Rect {
    let width = engine.measure(label, style.size);
    Rect::new(
        cell.origin.x + (cell.size.width - width) / 2.0,
        cell.origin.y + (cell.size.height - style.line_height) / 2.0,
        width,
        style.line_height,
    )
}

fn draw_arrow(scene: &mut Scene, center: Point, offset: f32, color: Color) {
    scene.triangle(
        Point::new(center.x - offset, center.y),
        Point::new(center.x + offset, center.y - 6.0),
        Point::new(center.x + offset, center.y + 6.0),
        color,
    );
}
