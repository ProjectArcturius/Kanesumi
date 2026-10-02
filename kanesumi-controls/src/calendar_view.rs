// MetroCalendarView —— 日历视图（UWP `CalendarView` 移植，元素树实现）。
//
// 一手源：OS UWP 主题字典 `themeresources.xaml`（`CalendarView*` 笔刷键，深浅两段）与
// `generic.xaml` 的 `CalendarViewDayItemRevealStyle` / `CalendarViewRevealStyle` 模板：
// 日格 MinWidth/MinHeight 40、Margin 1、`CalendarItemBorderThickness` 2；模板头行 40、
// 星期行 38、日格区 6×7。参 docs/CONTROL_SPEC.md「CalendarView」。
//
// 日期算法自带（proleptic Gregorian，1970-01-01 为周四），**不引依赖**；「今天」由调用方传入，
// 控件不读系统时钟（便于测试）。三级 DisplayMode 与 UWP 相同：Month（日格）→ Year（12 月）
// → Decade（12 年），选中逐级返回。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{CornerRadius, MetroTheme, Point, Rect, Size};

/// 日格边长（UWP `CalendarViewDayItem` MinWidth/MinHeight 40）。
pub const DAY_SIZE: f32 = 40.0;
/// 日格内缩（UWP `Margin` 1）—— 网格外缘留 1px，绘制落在 38×38 内。
pub const DAY_MARGIN: f32 = 1.0;
/// 星期行高（模板 `RowDefinition Height="38"`）。
pub const WEEKDAY_H: f32 = 38.0;
/// 头行高（模板 `RowDefinition Height="40"`）。
pub const HEADER_H: f32 = 40.0;
/// 日格列 / 行数。
pub const CAL_COLS: usize = 7;
pub const CAL_ROWS: usize = 6;
/// 年 / 十年视图的列 / 行数（UWP `CalendarPanel` 12 项，4×3）。
pub const PICKER_COLS: usize = 4;
pub const PICKER_ROWS: usize = 3;
/// 视图固有宽（7 × 40）与日格区高（6 × 40）。
pub const CAL_W: f32 = DAY_SIZE * CAL_COLS as f32;
pub const CAL_GRID_H: f32 = DAY_SIZE * CAL_ROWS as f32;
/// 视图固有高 = 头行 + 星期行 + 日格区。
pub const CAL_H: f32 = HEADER_H + WEEKDAY_H + CAL_GRID_H;

/// 公历日期（最小模型）。「今天」由调用方提供，控件不读系统时钟。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Date {
    pub const fn new(year: i32, month: u8, day: u8) -> Self {
        Self { year, month, day }
    }

    /// 闰年判定（Gregorian）。
    pub const fn is_leap(year: i32) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    /// 某月天数。非法月份回退 30（调用方不应传入）。
    pub const fn days_in_month(year: i32, month: u8) -> u8 {
        match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 => {
                if Self::is_leap(year) {
                    29
                } else {
                    28
                }
            }
            _ => 30,
        }
    }

    /// 距 1970-01-01 的天数（可为负）。
    pub fn epoch_days(self) -> i64 {
        days_from_civil(self.year, self.month, self.day)
    }

    /// 从距 1970-01-01 的天数还原。
    pub fn from_epoch_days(days: i64) -> Self {
        let (y, m, d) = civil_from_days(days);
        Self::new(y, m, d)
    }

    /// 按天数平移（跨月 / 跨年正确）。
    pub fn add_days(self, delta: i64) -> Self {
        Self::from_epoch_days(self.epoch_days() + delta)
    }

    /// 星期：0 = 周一 … 6 = 周日（1970-01-01 为周四 → 3）。
    pub fn weekday(self) -> u8 {
        (self.epoch_days() + 3).rem_euclid(7) as u8
    }

    /// 按月平移（日超出目标月天数时夹到月末）。
    pub fn add_months(self, delta: i32) -> Self {
        let total = self.year * 12 + self.month as i32 - 1 + delta;
        let year = total.div_euclid(12);
        let month = (total.rem_euclid(12) + 1) as u8;
        Self::new(year, month, self.day.min(Self::days_in_month(year, month)))
    }
}

/// Howard Hinnant `days_from_civil` —— proleptic Gregorian，1970-01-01 = 0。
fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = year as i64 - if month <= 2 { 1 } else { 0 };
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// `days_from_civil` 的逆运算。
fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u8, d as u8)
}

/// 三级显示模式（UWP `CalendarViewDisplayMode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarDisplayMode {
    /// 日格（月）。
    Month,
    /// 12 个月。
    Year,
    /// 12 年。
    Decade,
}

impl CalendarDisplayMode {
    /// 下一级视图（Month → Year → Decade；Decade 已是最上级）。
    pub fn next(self) -> Self {
        match self {
            Self::Month => Self::Year,
            Self::Year => Self::Decade,
            Self::Decade => Self::Decade,
        }
    }
}

/// 元素树动作：选中某日（点日格 / Enter）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateSelected(pub Date);

/// 一次交互的结果（元素树接入与 DatePicker 面板共用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalendarResponse {
    None,
    /// 选中了某日（点日格 / Enter/Space）。
    DateSelected(Date),
    /// DisplayMode 改变（点头部 / 选中月、年返回上级）。
    ViewChanged,
    /// 显示月 / 年 / 十年改变（翻页按钮 / PageUp/PageDown / 滚轮）。
    MonthChanged,
}

/// MetroCalendarView —— 日历视图。参 CONTROL_SPEC「CalendarView」。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetroCalendarView {
    /// 当前显示的月（`day` 无意义，惯例取 1）。
    pub displayed: Date,
    /// 选中日（None = 未选）。
    pub selected: Option<Date>,
    /// 「今天」（调用方传入；None = 不高亮今天）。
    pub today: Option<Date>,
    /// 显示模式。
    pub display_mode: CalendarDisplayMode,
    /// 周起始日：0 = 周一 … 6 = 周日（默认周一）。
    pub week_start: u8,
    /// 键盘游标日。
    pub cursor: Date,
    /// 指针悬停的日格。
    pub hovered: Option<Date>,
    /// 指针悬停的头部区（0 标题 / 1 上月 / 2 下月）。
    pub hovered_nav: Option<u8>,
    /// 本控件是否持键盘焦点（`paint` 时由框架状态写入，用于画游标框）。
    pub focused: bool,
}

impl MetroCalendarView {
    /// 以「今天」构造：显示今天所在月、游标落在今天、无选中。
    pub fn new(today: Date) -> Self {
        Self {
            displayed: Date::new(today.year, today.month, 1),
            selected: None,
            today: Some(today),
            display_mode: CalendarDisplayMode::Month,
            week_start: 0,
            cursor: today,
            hovered: None,
            hovered_nav: None,
            focused: false,
        }
    }

    /// 指定初始选中日。
    #[must_use]
    pub fn with_selected(mut self, date: Date) -> Self {
        self.selected = Some(date);
        self.cursor = date;
        self.displayed = Date::new(date.year, date.month, 1);
        self
    }

    /// 配置周起始日（0 = 周一 … 6 = 周日）。
    #[must_use]
    pub fn with_week_start(mut self, start: u8) -> Self {
        self.week_start = start % 7;
        self
    }

    /// 固有尺寸（宽 = 7 日格，高 = 头行 + 星期行 + 6 日格）。
    pub fn measure(&self, _engine: &TextEngine) -> Size {
        Size::new(CAL_W, CAL_H)
    }

    /// 42 个日格日期（从周起始日对齐，含上下月溢出）。
    pub fn month_grid(&self) -> Vec<Date> {
        let first = Date::new(self.displayed.year, self.displayed.month, 1);
        let lead = (first.weekday() as i32 - self.week_start as i32).rem_euclid(7) as i64;
        let start = first.add_days(-lead);
        (0..(CAL_COLS * CAL_ROWS) as i64)
            .map(|i| start.add_days(i))
            .collect()
    }

    /// 头部三区矩形 `(标题, 上月, 下月)`（模板列宽 5:1:1）。
    pub fn nav_rects(&self, rect: Rect) -> (Rect, Rect, Rect) {
        let nav = rect.size.width / 7.0;
        let title = Rect::new(
            rect.origin.x,
            rect.origin.y,
            (rect.size.width - 2.0 * nav).max(0.0),
            HEADER_H,
        );
        let prev = Rect::new(rect.right() - 2.0 * nav, rect.origin.y, nav, HEADER_H);
        let next = Rect::new(rect.right() - nav, rect.origin.y, nav, HEADER_H);
        (title, prev, next)
    }

    /// 某日在当前 42 格中的矩形（不在格内返回 None）。
    pub fn day_rect(&self, rect: Rect, date: Date) -> Option<Rect> {
        let idx = self.month_grid().iter().position(|d| *d == date)?;
        let row = idx / CAL_COLS;
        let col = idx % CAL_COLS;
        let top = rect.origin.y + HEADER_H + WEEKDAY_H;
        Some(Rect::new(
            rect.origin.x + col as f32 * DAY_SIZE,
            top + row as f32 * DAY_SIZE,
            DAY_SIZE,
            DAY_SIZE,
        ))
    }

    /// 月 / 十年视图中第 `idx`（0..12）项的矩形。
    pub fn picker_cell_rect(rect: Rect, idx: usize) -> Rect {
        let cw = rect.size.width / PICKER_COLS as f32;
        let ch = CAL_GRID_H / PICKER_ROWS as f32;
        let top = rect.origin.y + HEADER_H + WEEKDAY_H;
        Rect::new(
            rect.origin.x + (idx % PICKER_COLS) as f32 * cw,
            top + (idx / PICKER_COLS) as f32 * ch,
            cw,
            ch,
        )
    }

    /// 月视图：指针下的日期（含上下月溢出格）。
    pub fn day_at(&self, rect: Rect, pos: Point) -> Option<Date> {
        let top = rect.origin.y + HEADER_H + WEEKDAY_H;
        if pos.x < rect.origin.x || pos.x >= rect.right() || pos.y < top {
            return None;
        }
        let col = ((pos.x - rect.origin.x) / DAY_SIZE).floor() as usize;
        let row = ((pos.y - top) / DAY_SIZE).floor() as usize;
        if col >= CAL_COLS || row >= CAL_ROWS {
            return None;
        }
        self.month_grid().get(row * CAL_COLS + col).copied()
    }

    /// 年视图：指针下的月份（1..12）。
    pub fn month_at(rect: Rect, pos: Point) -> Option<u8> {
        let idx = Self::picker_index_at(rect, pos)?;
        Some(idx as u8 + 1)
    }

    /// 十年视图：指针下的年份（以十年基址 + idx）。
    pub fn year_at(&self, rect: Rect, pos: Point) -> Option<i32> {
        let idx = Self::picker_index_at(rect, pos)?;
        let base = self.displayed.year.div_euclid(10) * 10;
        Some(base + idx as i32)
    }

    fn picker_index_at(rect: Rect, pos: Point) -> Option<usize> {
        let top = rect.origin.y + HEADER_H + WEEKDAY_H;
        if pos.y < top || pos.y >= top + CAL_GRID_H {
            return None;
        }
        (0..(PICKER_COLS * PICKER_ROWS))
            .find(|idx| Self::picker_cell_rect(rect, *idx).contains(pos))
    }

    /// 十年基址（`2026 → 2020`）。
    pub fn decade_base(&self) -> i32 {
        self.displayed.year.div_euclid(10) * 10
    }

    /// 头部标题文本（UWP `TemplateSettings.HeaderText`）。
    pub fn header_text(&self) -> String {
        match self.display_mode {
            CalendarDisplayMode::Month => {
                format!("{}年{}月", self.displayed.year, self.displayed.month)
            }
            CalendarDisplayMode::Year => format!("{}年", self.displayed.year),
            CalendarDisplayMode::Decade => {
                let base = self.decade_base();
                format!("{} – {}年", base, base + 11)
            }
        }
    }

    /// 显示月平移。
    fn shift_month(&mut self, delta: i32) {
        self.displayed = self.displayed.add_months(delta);
        self.cursor = Date::new(
            self.displayed.year,
            self.displayed.month,
            self.cursor.day.min(Date::days_in_month(
                self.displayed.year,
                self.displayed.month,
            )),
        );
    }

    /// 键盘游标平移（跨月时同步显示月）。
    fn move_cursor(&mut self, days: i64) {
        self.cursor = self.cursor.add_days(days);
        if self.cursor.year != self.displayed.year || self.cursor.month != self.displayed.month {
            self.displayed = Date::new(self.cursor.year, self.cursor.month, 1);
        }
    }

    /// 选中某日；溢出日格会同步显示月。
    fn select_day(&mut self, date: Date) -> CalendarResponse {
        self.selected = Some(date);
        self.cursor = date;
        if date.year != self.displayed.year || date.month != self.displayed.month {
            self.displayed = Date::new(date.year, date.month, 1);
        }
        CalendarResponse::DateSelected(date)
    }

    /// 指针悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        let (title, prev, next) = self.nav_rects(rect);
        self.hovered_nav = if prev.contains(pos) {
            Some(1)
        } else if next.contains(pos) {
            Some(2)
        } else if title.contains(pos) {
            Some(0)
        } else {
            None
        };
        self.hovered = match self.display_mode {
            CalendarDisplayMode::Month => self.day_at(rect, pos),
            _ => None,
        };
    }

    /// 单击。返回动作语义（点日格 → `DateSelected`）。
    pub fn click(&mut self, rect: Rect, pos: Point) -> CalendarResponse {
        if !rect.contains(pos) {
            return CalendarResponse::None;
        }
        let (title, prev, next) = self.nav_rects(rect);
        match self.display_mode {
            CalendarDisplayMode::Month => {
                if title.contains(pos) {
                    self.display_mode = CalendarDisplayMode::Year;
                    return CalendarResponse::ViewChanged;
                }
                if prev.contains(pos) {
                    self.shift_month(-1);
                    return CalendarResponse::MonthChanged;
                }
                if next.contains(pos) {
                    self.shift_month(1);
                    return CalendarResponse::MonthChanged;
                }
                if let Some(date) = self.day_at(rect, pos) {
                    return self.select_day(date);
                }
            }
            CalendarDisplayMode::Year => {
                if title.contains(pos) {
                    self.display_mode = CalendarDisplayMode::Decade;
                    return CalendarResponse::ViewChanged;
                }
                if prev.contains(pos) {
                    self.displayed = self.displayed.add_months(-12).with_day(1);
                    return CalendarResponse::MonthChanged;
                }
                if next.contains(pos) {
                    self.displayed = self.displayed.add_months(12).with_day(1);
                    return CalendarResponse::MonthChanged;
                }
                if let Some(month) = Self::month_at(rect, pos) {
                    self.displayed = Date::new(self.displayed.year, month, 1);
                    self.cursor = Date::new(self.displayed.year, month, 1);
                    self.display_mode = CalendarDisplayMode::Month;
                    return CalendarResponse::ViewChanged;
                }
            }
            CalendarDisplayMode::Decade => {
                if prev.contains(pos) {
                    self.displayed = Date::new(self.displayed.year - 10, self.displayed.month, 1);
                    return CalendarResponse::MonthChanged;
                }
                if next.contains(pos) {
                    self.displayed = Date::new(self.displayed.year + 10, self.displayed.month, 1);
                    return CalendarResponse::MonthChanged;
                }
                if let Some(year) = self.year_at(rect, pos) {
                    self.displayed = Date::new(year, self.displayed.month, 1);
                    self.cursor = Date::new(year, self.displayed.month, 1);
                    self.display_mode = CalendarDisplayMode::Year;
                    return CalendarResponse::ViewChanged;
                }
            }
        }
        CalendarResponse::None
    }

    /// 键盘。PageUp / PageDown 用框架具名变体（外壳把 keysym 语义化，控件不再比原始码）。
    pub fn key(&mut self, key: kanesumi_element::Key) -> CalendarResponse {
        use kanesumi_element::Key;
        match self.display_mode {
            CalendarDisplayMode::Month => match key {
                Key::Left => {
                    self.move_cursor(-1);
                    CalendarResponse::None
                }
                Key::Right => {
                    self.move_cursor(1);
                    CalendarResponse::None
                }
                Key::Up => {
                    self.move_cursor(-7);
                    CalendarResponse::None
                }
                Key::Down => {
                    self.move_cursor(7);
                    CalendarResponse::None
                }
                Key::Enter | Key::Char(' ') | Key::Space => self.select_day(self.cursor),
                Key::PageUp => {
                    self.shift_month(-1);
                    CalendarResponse::MonthChanged
                }
                Key::PageDown => {
                    self.shift_month(1);
                    CalendarResponse::MonthChanged
                }
                _ => CalendarResponse::None,
            },
            CalendarDisplayMode::Year => match key {
                Key::Left => {
                    self.displayed = self.displayed.add_months(-1).with_day(1);
                    CalendarResponse::None
                }
                Key::Right => {
                    self.displayed = self.displayed.add_months(1).with_day(1);
                    CalendarResponse::None
                }
                Key::Up => {
                    self.displayed = self.displayed.add_months(-4).with_day(1);
                    CalendarResponse::None
                }
                Key::Down => {
                    self.displayed = self.displayed.add_months(4).with_day(1);
                    CalendarResponse::None
                }
                Key::Enter | Key::Char(' ') | Key::Space => {
                    self.display_mode = CalendarDisplayMode::Month;
                    CalendarResponse::ViewChanged
                }
                Key::PageUp => {
                    self.displayed = self.displayed.add_months(-12).with_day(1);
                    CalendarResponse::MonthChanged
                }
                Key::PageDown => {
                    self.displayed = self.displayed.add_months(12).with_day(1);
                    CalendarResponse::MonthChanged
                }
                _ => CalendarResponse::None,
            },
            CalendarDisplayMode::Decade => match key {
                Key::Left => {
                    self.displayed = Date::new(self.displayed.year - 1, self.displayed.month, 1);
                    CalendarResponse::None
                }
                Key::Right => {
                    self.displayed = Date::new(self.displayed.year + 1, self.displayed.month, 1);
                    CalendarResponse::None
                }
                Key::Up => {
                    self.displayed = Date::new(self.displayed.year - 4, self.displayed.month, 1);
                    CalendarResponse::None
                }
                Key::Down => {
                    self.displayed = Date::new(self.displayed.year + 4, self.displayed.month, 1);
                    CalendarResponse::None
                }
                Key::Enter | Key::Char(' ') | Key::Space => {
                    self.display_mode = CalendarDisplayMode::Year;
                    CalendarResponse::ViewChanged
                }
                Key::PageUp => {
                    self.displayed = Date::new(self.displayed.year - 10, self.displayed.month, 1);
                    CalendarResponse::MonthChanged
                }
                Key::PageDown => {
                    self.displayed = Date::new(self.displayed.year + 10, self.displayed.month, 1);
                    CalendarResponse::MonthChanged
                }
                _ => CalendarResponse::None,
            },
        }
    }

    /// 滚轮：正 `dy`（向下）→ 下一月 / 年 / 十年。
    pub fn scroll(&mut self, dy: f32) -> CalendarResponse {
        if dy == 0.0 {
            return CalendarResponse::None;
        }
        match self.display_mode {
            CalendarDisplayMode::Month => self.shift_month(if dy > 0.0 { 1 } else { -1 }),
            CalendarDisplayMode::Year => {
                self.displayed = self
                    .displayed
                    .add_months(if dy > 0.0 { 12 } else { -12 })
                    .with_day(1);
            }
            CalendarDisplayMode::Decade => {
                self.displayed = Date::new(
                    self.displayed.year + if dy > 0.0 { 10 } else { -10 },
                    self.displayed.month,
                    1,
                );
            }
        }
        CalendarResponse::MonthChanged
    }
}

impl Date {
    /// 取该日的月初（仅换显示月时用）。
    fn with_day(self, day: u8) -> Self {
        Self::new(self.year, self.month, day)
    }
}

/// 头部按钮（上月 / 下月）的箭头矩形。
fn nav_arrow_rect(button: Rect) -> Rect {
    Rect::new(button.center().x - 6.0, button.center().y - 6.0, 12.0, 12.0)
}

impl MetroCalendarView {
    /// 渲染到 `rect`（自身按 `CAL_W × CAL_H` 布局；小于该尺寸时裁到 rect）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            return;
        }
        scene.push_clip(rect);

        // 头行：标题 + 上月 / 下月按钮。
        let colors = &theme.colors;
        let body = theme.typography.body;
        let (title, prev, next) = self.nav_rects(rect);
        let title_y = title.origin.y + (HEADER_H - body.line_height) / 2.0;
        scene.label(
            self.header_text(),
            Rect::new(
                title.origin.x + 12.0,
                title_y,
                (title.size.width - 12.0).max(0.0),
                body.line_height,
            ),
            colors.on_surface,
            body,
            TextAlign::Left,
        );
        let nav_fg = |hovered: bool| {
            if hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            }
        };
        glyph::chevron_left(
            scene,
            nav_arrow_rect(prev),
            nav_fg(self.hovered_nav == Some(1)),
        );
        glyph::chevron_right(
            scene,
            nav_arrow_rect(next),
            nav_fg(self.hovered_nav == Some(2)),
        );

        match self.display_mode {
            CalendarDisplayMode::Month => self.render_month(theme, engine, rect, scene),
            CalendarDisplayMode::Year => self.render_picker(theme, engine, rect, scene, false),
            CalendarDisplayMode::Decade => self.render_picker(theme, engine, rect, scene, true),
        }
        scene.pop_clip();
    }

    /// 月视图：星期行 + 42 日格。
    fn render_month(
        &self,
        theme: &MetroTheme,
        _engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let body = theme.typography.body;
        let caption = theme.typography.caption;
        let names = ["一", "二", "三", "四", "五", "六", "日"];

        // 星期行（从周起始日轮转）。
        let week_top = rect.origin.y + HEADER_H;
        for col in 0..CAL_COLS {
            let name = names[(self.week_start as usize + col) % 7];
            let cell = Rect::new(
                rect.origin.x + col as f32 * DAY_SIZE,
                week_top,
                DAY_SIZE,
                WEEKDAY_H,
            );
            let y = cell.origin.y + (WEEKDAY_H - caption.line_height) / 2.0;
            scene.label(
                name.to_string(),
                Rect::new(cell.origin.x, y, cell.size.width, caption.line_height),
                colors.on_surface,
                caption,
                TextAlign::Center,
            );
        }

        // 日格。
        let grid_top = rect.origin.y + HEADER_H + WEEKDAY_H;
        for (i, date) in self.month_grid().iter().enumerate() {
            let row = i / CAL_COLS;
            let col = i % CAL_COLS;
            let cell = Rect::new(
                rect.origin.x + col as f32 * DAY_SIZE,
                grid_top + row as f32 * DAY_SIZE,
                DAY_SIZE,
                DAY_SIZE,
            );
            let inner = cell.inset(DAY_MARGIN, DAY_MARGIN, DAY_MARGIN, DAY_MARGIN);
            let in_month = date.year == self.displayed.year && date.month == self.displayed.month;
            let selected = self.selected == Some(*date);
            let is_today = self.today == Some(*date);
            let hovered = self.hovered == Some(*date);

            // 底：选中填充 / 悬停浅底 / 溢出月弱底。
            if selected {
                scene.fill_rounded_rect(colors.primary, inner, CornerRadius::Square);
            } else if hovered {
                scene.fill_rounded_rect(colors.surface_variant, inner, CornerRadius::Square);
            } else if !in_month {
                scene.fill_rounded_rect(colors.track_subtle, inner, CornerRadius::Square);
            }
            // 今天：强调色描边（UWP `IsTodayHighlighted`）。
            if is_today && !selected {
                scene.stroke_rect(colors.primary, inner, 2.0);
            }
            // 键盘游标：焦点描边（仅聚焦时画；正典 §Ⅳ 外 2px + 内 1px）。
            if self.focused && self.cursor == *date && in_month && !selected {
                crate::focus::draw_focus_ring(
                    scene,
                    colors.focus_stroke,
                    theme.scheme,
                    inner,
                    kanesumi_core::CornerRadius::Square,
                );
            }

            let fg = if selected {
                colors.on_primary
            } else if !in_month {
                colors.on_surface_variant
            } else if is_today {
                colors.primary
            } else {
                colors.on_surface
            };
            let y = inner.origin.y + (inner.size.height - body.line_height) / 2.0;
            scene.label(
                date.day.to_string(),
                Rect::new(inner.origin.x, y, inner.size.width, body.line_height),
                fg,
                body,
                TextAlign::Center,
            );
        }
    }

    /// 年 / 十年视图：12 格（`decade` = 十年视图）。
    fn render_picker(
        &self,
        theme: &MetroTheme,
        _engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
        decade: bool,
    ) {
        let colors = &theme.colors;
        let body = theme.typography.body;
        let base = self.decade_base();
        for idx in 0..(PICKER_COLS * PICKER_ROWS) {
            let cell = Self::picker_cell_rect(rect, idx);
            let inner = cell.inset(DAY_MARGIN, DAY_MARGIN, DAY_MARGIN, DAY_MARGIN);
            let (label, selected) = if decade {
                let year = base + idx as i32;
                (format!("{year}"), year == self.displayed.year)
            } else {
                let month = idx as u8 + 1;
                (format!("{month}月"), month == self.displayed.month)
            };
            if selected {
                scene.fill_rounded_rect(colors.primary, inner, CornerRadius::Square);
            }
            let fg = if selected {
                colors.on_primary
            } else {
                colors.on_surface
            };
            let y = inner.origin.y + (inner.size.height - body.line_height) / 2.0;
            scene.label(
                label,
                Rect::new(inner.origin.x, y, inner.size.width, body.line_height),
                fg,
                body,
                TextAlign::Center,
            );
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_MIGRATION.md §5）──────────────────────────────────
//
// 旧 API（`measure` / `hover` / `click` / `key` / `scroll` / `render`）保留为纯逻辑，供
// DatePicker 面板复用；`Widget` 实现只做事件映射与动作发射（点日格 → `DateSelected`）。

impl kanesumi_element::Widget for MetroCalendarView {
    fn measure(&mut self, ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        let size = MetroCalendarView::measure(self, ctx.engine());
        // 宽度可随可用宽收缩（挤小时由 `render` 裁到 rect，§6）。
        let width = if available.width.is_finite() {
            size.width.min(available.width.max(0.0))
        } else {
            size.width
        };
        Size::new(width, size.height)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.focused = ctx.state().focused;
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                self.hover(ctx.rect(), *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                self.hovered_nav = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let response = self.click(ctx.rect(), *pos);
                if let CalendarResponse::DateSelected(date) = response {
                    ctx.emit(DateSelected(date));
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::Scroll { dy, .. } => {
                if self.scroll(*dy) != CalendarResponse::None {
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => {
                let relevant = matches!(
                    key,
                    Key::Left
                        | Key::Right
                        | Key::Up
                        | Key::Down
                        | Key::Enter
                        | Key::Char(' ')
                        | Key::Space
                        | Key::PageUp
                        | Key::PageDown
                );
                if !relevant {
                    return;
                }
                let response = self.key(*key);
                if let CalendarResponse::DateSelected(date) = response {
                    ctx.emit(DateSelected(date));
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 焦点视觉 = 日格游标框（自绘），不叠加框架整框。
    fn focus_visual(&self) -> bool {
        false
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: String::from("日历"),
            value: self
                .selected
                .map(|d| format!("{}年{}月{}日", d.year, d.month, d.day)),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_years() {
        assert!(Date::is_leap(2000), "400 年闰");
        assert!(Date::is_leap(2024));
        assert!(!Date::is_leap(1900), "百年不闰");
        assert!(!Date::is_leap(2026));
    }

    #[test]
    fn month_lengths() {
        assert_eq!(Date::days_in_month(2000, 2), 29);
        assert_eq!(Date::days_in_month(2026, 2), 28);
        assert_eq!(Date::days_in_month(2026, 4), 30);
        assert_eq!(Date::days_in_month(2026, 12), 31);
    }

    #[test]
    fn known_weekdays() {
        // 0 = 周一 … 6 = 周日
        assert_eq!(Date::new(2026, 10, 1).weekday(), 3, "2026-10-01 星期四");
        assert_eq!(Date::new(2000, 2, 29).weekday(), 1, "2000-02-29 星期二");
        assert_eq!(Date::new(1970, 1, 1).weekday(), 3, "纪元日为周四");
    }

    #[test]
    fn epoch_roundtrip_and_month_math() {
        let d = Date::new(2026, 10, 1);
        assert_eq!(Date::from_epoch_days(d.epoch_days()), d);
        assert_eq!(d.add_days(31), Date::new(2026, 11, 1));
        assert_eq!(d.add_months(-1), Date::new(2026, 9, 1));
        assert_eq!(
            Date::new(2026, 1, 31).add_months(1),
            Date::new(2026, 2, 28),
            "日夹到目标月末"
        );
    }

    #[test]
    fn month_grid_starts_on_configured_weekday() {
        let v = MetroCalendarView::new(Date::new(2026, 10, 1));
        let grid = v.month_grid();
        assert_eq!(grid.len(), 42);
        assert_eq!(grid[0].weekday(), 0, "默认周一起始");
        // 2026-10-01 周四 → 前导 3 格（9/28 一、9/29 二、9/30 三）。
        assert_eq!(grid[0], Date::new(2026, 9, 28));
        assert_eq!(grid[3], Date::new(2026, 10, 1));

        let sunday = MetroCalendarView::new(Date::new(2026, 10, 1)).with_week_start(6);
        assert_eq!(sunday.month_grid()[0].weekday(), 6, "周日起始");
    }

    #[test]
    fn scroll_changes_month() {
        let mut v = MetroCalendarView::new(Date::new(2026, 10, 1));
        assert_eq!(v.scroll(10.0), CalendarResponse::MonthChanged);
        assert_eq!(v.displayed, Date::new(2026, 11, 1));
        assert_eq!(v.scroll(-10.0), CalendarResponse::MonthChanged);
        assert_eq!(v.displayed, Date::new(2026, 10, 1));
        assert_eq!(v.scroll(0.0), CalendarResponse::None);
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(500.0, 500.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCalendarView::new(Date::new(2026, 10, 1)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn cell_center(h: &TestHarness, id: WidgetId, date: Date) -> Point {
        h.tree
            .get::<MetroCalendarView>(id)
            .unwrap()
            .day_rect(h.rect(id), date)
            .expect("日在 42 格内")
            .center()
    }

    #[test]
    fn click_day_selects_and_reports() {
        let (mut h, id) = harness();
        h.click_at(cell_center(&h, id, Date::new(2026, 10, 15)));
        assert_eq!(
            h.take::<DateSelected>(),
            vec![(id, DateSelected(Date::new(2026, 10, 15)))]
        );
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().selected,
            Some(Date::new(2026, 10, 15))
        );
    }

    #[test]
    fn click_adjacent_month_cell_switches_month() {
        let (mut h, id) = harness();
        // 2026-11-01 在 10 月网格末行（溢出格），点击应切到 11 月。
        h.click_at(cell_center(&h, id, Date::new(2026, 11, 1)));
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().displayed,
            Date::new(2026, 11, 1)
        );
        assert_eq!(
            h.take::<DateSelected>(),
            vec![(id, DateSelected(Date::new(2026, 11, 1)))]
        );
    }

    #[test]
    fn page_down_and_up_change_month() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        // 外壳把 PageUp / PageDown 语义化为具名变体，控件直接比较语义键。
        h.key(Key::PageDown);
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().displayed,
            Date::new(2026, 11, 1)
        );
        h.key(Key::PageUp);
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().displayed,
            Date::new(2026, 10, 1)
        );
    }

    #[test]
    fn arrow_and_enter_selects_cursor() {
        let (mut h, id) = harness();
        h.tab();
        h.key(Key::Right); // 10-02
        h.key(Key::Down); // +7 → 10-09
        h.key(Key::Enter);
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().selected,
            Some(Date::new(2026, 10, 9))
        );
        assert_eq!(
            h.take::<DateSelected>(),
            vec![(id, DateSelected(Date::new(2026, 10, 9)))]
        );
    }

    #[test]
    fn three_level_view_round_trip() {
        let (mut h, id) = harness();
        let rect = h.rect(id);
        let (title, _, _) = h.tree.get::<MetroCalendarView>(id).unwrap().nav_rects(rect);
        h.click_at(title.center());
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().display_mode,
            CalendarDisplayMode::Year
        );
        h.click_at(title.center());
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().display_mode,
            CalendarDisplayMode::Decade
        );
        // 十年视图点 2026 → 回 Year。
        let idx = {
            let v = h.tree.get::<MetroCalendarView>(id).unwrap();
            (2026 - v.decade_base()) as usize
        };
        h.click_at(MetroCalendarView::picker_cell_rect(rect, idx).center());
        let v = h.tree.get::<MetroCalendarView>(id).unwrap();
        assert_eq!(v.display_mode, CalendarDisplayMode::Year);
        assert_eq!(v.displayed.year, 2026);
        // 年视图点 10 月 → 回 Month。
        h.click_at(MetroCalendarView::picker_cell_rect(rect, 9).center());
        let v = h.tree.get::<MetroCalendarView>(id).unwrap();
        assert_eq!(v.display_mode, CalendarDisplayMode::Month);
        assert_eq!(v.displayed, Date::new(2026, 10, 1));
    }

    #[test]
    fn hover_sets_and_leave_clears() {
        let (mut h, id) = harness();
        h.move_to(cell_center(&h, id, Date::new(2026, 10, 20)));
        h.frame();
        assert_eq!(
            h.tree.get::<MetroCalendarView>(id).unwrap().hovered,
            Some(Date::new(2026, 10, 20))
        );
        h.tree.pointer_leave();
        h.frame();
        assert_eq!(h.tree.get::<MetroCalendarView>(id).unwrap().hovered, None);
    }

    #[test]
    fn sizes_and_insurance_checks() {
        let (h, id) = harness();
        assert_eq!(h.rect(id).size, Size::new(CAL_W, CAL_H));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        // 挤到 120 宽时 render 必须裁到 rect（§6）。
        let mut h = TestHarness::new(200.0, 400.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroCalendarView::new(Date::new(2026, 10, 1)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                width: Some(120.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(id).size.width, 120.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(cell_center(&h, id, Date::new(2026, 10, 15)));
        assert!(h.take::<DateSelected>().is_empty());
        assert_eq!(h.tree.get::<MetroCalendarView>(id).unwrap().selected, None);
    }
}
