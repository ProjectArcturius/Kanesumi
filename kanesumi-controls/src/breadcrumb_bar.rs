// MetroBreadcrumbBar —— 面包屑导航。参 CONTROL_SPEC §18。
//
// 移植自 microsoft-ui-xaml/dev/Breadcrumb（BreadcrumbBar.cpp + BreadcrumbBar.xaml）：
// - Item 14px Normal / LineHeight 20 / Padding 1,3；
// - 项间 chevron（E974 → 自绘）12px、Padding 2,0（占 ~16px）；
// - 当前项（末项）非按钮、无尾部 chevron、前景 on_surface；
// - 超宽折叠：前缀换 "…"（至少保留末项），点 "…" 弹出隐藏项下拉（MetroDropdownMenu）。
// 折叠与下拉复用 `MetroDropdownMenu`（隐藏项即其 items）。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size};
use kanesumi_element::{
    Event, EventCtx, Key, MeasureCtx, PaintCtx, PointerButton, PopupSpec, UpdateCtx, Widget,
    WidgetId,
};

use crate::dropdown_menu::{MenuItem, MetroDropdownMenu};
use crate::popup::{place_popup, popup_gap};

/// Item 字号（BreadcrumbBarItemThemeFontSize = ControlContentThemeFontSize 14）。
const ITEM_FONT: f32 = 14.0;
/// Item 水平 Padding（`1,3` → 左右各 1）。
const ITEM_PAD_X: f32 = 1.0;
/// Item 垂直 Padding（上下各 3）。
const ITEM_PAD_Y: f32 = 3.0;
/// Chevron 总占宽（12 glyph + 2×2 padding）。
const CHEVRON_GAP: f32 = 16.0;
/// Ellipsis 水平内边距（Padding 3 → 左右各 3）。
const ELLIPSIS_PAD: f32 = 6.0;

/// 面包屑点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreadcrumbClick {
    None,
    /// 命中面包屑项（索引；含下拉隐藏项）。
    Index(usize),
    /// 命中 Ellipsis（打开下拉）。
    Ellipsis,
}

/// 布局结果：可见起点 + 是否折叠 + 末段截断宽。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BreadcrumbLayout {
    /// 首个可见项在 `items` 的索引。
    pub start: usize,
    /// 是否渲染 Ellipsis（start > 0）。
    pub ellipsis: bool,
    /// 末段（当前项）可用宽：`Some(w)` = 前缀全部折叠后仍放不下，末段以省略号截断到 `w`；
    /// `None` = 完整显示。参 CONTROL_SPEC §18「收窄优先折叠前缀，仍放不下则末段截断」。
    pub last_width: Option<f32>,
}

/// MetroBreadcrumbBar —— 面包屑。参 CONTROL_SPEC §18。
#[derive(Debug, Clone)]
pub struct MetroBreadcrumbBar {
    pub items: Vec<String>,
    /// hover 的可见面包屑项（绝对索引）。
    pub hovered_item: Option<usize>,
    /// 是否 hover 在 Ellipsis 上。
    pub hovered_ellipsis: bool,
    /// Ellipsis 下拉（隐藏项）。
    pub menu: MetroDropdownMenu,
    /// 元素树下打开的溢出面板（`BreadcrumbOverflow` 节点）；旧路径不用。
    tree_popup: Option<WidgetId>,
    /// 元素树下最近一次按下的位置（`Event::Click` 不带坐标，用它换算命中项）。
    press_pos: Option<Point>,
}

impl Default for MetroBreadcrumbBar {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            hovered_item: None,
            hovered_ellipsis: false,
            menu: MetroDropdownMenu::new(Vec::new()),
            tree_popup: None,
            press_pos: None,
        }
    }
}

impl MetroBreadcrumbBar {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// Item 文本样式。
    pub fn item_style() -> TextStyle {
        TextStyle::new(ITEM_FONT, 20.0, FontWeight::Normal)
    }

    /// 计算折叠布局。参 CONTROL_SPEC §18 折叠语义。
    pub fn layout(&self, engine: &TextEngine, rect: Rect) -> BreadcrumbLayout {
        let avail = rect.size.width;
        let style = Self::item_style();
        let widths: Vec<f32> = self
            .items
            .iter()
            .map(|i| engine.measure(i, style.size) + ITEM_PAD_X * 2.0)
            .collect();
        let n = self.items.len();
        if n == 0 {
            return BreadcrumbLayout {
                start: 0,
                ellipsis: false,
                last_width: None,
            };
        }
        // 全展示所需宽度
        let total: f32 = widths.iter().sum::<f32>() + (n as f32 - 1.0) * CHEVRON_GAP;
        if total <= avail {
            return BreadcrumbLayout {
                start: 0,
                ellipsis: false,
                last_width: None,
            };
        }
        // 只有一项且放不下：无可折叠前缀，直接末段截断。
        if n == 1 {
            return BreadcrumbLayout {
                start: 0,
                ellipsis: false,
                last_width: Some(avail.max(0.0)),
            };
        }
        // 折叠：优先把前缀折进已有的 Ellipsis 溢出按钮，保留尽量多的后缀（start 最小）。
        // 折叠后首项前让位 Ellipsis + 一个 chevron。
        let ellipsis_w = engine.measure("…", style.size) + ELLIPSIS_PAD;
        let prefix = ellipsis_w + CHEVRON_GAP;
        let mut start = n - 1; // 至少保留末项
        for candidate in 1..n {
            let kept: f32 = widths[candidate..].iter().sum();
            let gaps = (n - 1 - candidate) as f32 * CHEVRON_GAP;
            if prefix + kept + gaps <= avail {
                start = candidate;
                break;
            }
        }
        // 末段可用宽 = 总宽减去它之前的全部占位；不足其完整宽度时以省略号截断。
        let before_last = prefix
            + widths[start..n - 1].iter().sum::<f32>()
            + (n - 1 - start) as f32 * CHEVRON_GAP;
        let last_avail = (avail - before_last).max(0.0);
        let last_width = (last_avail + 0.01 < widths[n - 1]).then_some(last_avail);
        BreadcrumbLayout {
            start,
            ellipsis: true,
            last_width,
        }
    }

    /// 单个面包屑项 rect（含 padding）。
    fn item_rect(&self, engine: &TextEngine, rect: Rect, index: usize) -> Rect {
        let style = Self::item_style();
        let x = self.item_x(engine, rect, index);
        Rect::new(
            x,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
            engine.measure(&self.items[index], style.size) + ITEM_PAD_X * 2.0,
            style.line_height + ITEM_PAD_Y * 2.0,
        )
    }

    /// 面包屑项 x 起点（考虑 Ellipsis 与 chevron）。
    fn item_x(&self, engine: &TextEngine, rect: Rect, index: usize) -> f32 {
        let layout = self.layout(engine, rect);
        let style = Self::item_style();
        let mut x = rect.origin.x;
        if layout.ellipsis {
            x += engine.measure("…", style.size) + ELLIPSIS_PAD + CHEVRON_GAP;
        }
        for i in layout.start..index {
            x += engine.measure(&self.items[i], style.size) + ITEM_PAD_X * 2.0;
            if i < self.items.len() - 1 {
                x += CHEVRON_GAP;
            }
        }
        x
    }

    /// Ellipsis rect（折叠时）。
    pub fn ellipsis_rect(&self, engine: &TextEngine, rect: Rect) -> Option<Rect> {
        let layout = self.layout(engine, rect);
        if !layout.ellipsis {
            return None;
        }
        let style = Self::item_style();
        let w = engine.measure("…", style.size) + ELLIPSIS_PAD;
        Some(Rect::new(
            rect.origin.x,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
            w,
            style.line_height + ITEM_PAD_Y * 2.0,
        ))
    }

    /// 命中面包屑项（含 Ellipsis）。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> BreadcrumbClick {
        if let Some(e) = self.ellipsis_rect(engine, rect)
            && e.contains(pos)
        {
            return BreadcrumbClick::Ellipsis;
        }
        let layout = self.layout(engine, rect);
        for i in layout.start..self.items.len() {
            let r = self.item_rect(engine, rect, i);
            if r.contains(pos) {
                return BreadcrumbClick::Index(i);
            }
        }
        BreadcrumbClick::None
    }

    /// 打开/关闭 Ellipsis 下拉（隐藏项）。
    pub fn toggle_ellipsis(&mut self, engine: &TextEngine, rect: Rect, screen: Rect) {
        let layout = self.layout(engine, rect);
        let hidden = self.items[0..layout.start].to_vec();
        self.menu.items = hidden.into_iter().map(MenuItem::new).collect();
        self.menu.invalidate_layout();
        if self.menu.anim.is_open() {
            self.menu.close();
        } else if let Some(er) = self.ellipsis_rect(engine, rect) {
            let size = self.menu.panel_size(engine);
            let placement = place_popup(er, size, screen, popup_gap());
            self.menu.open(placement.rect);
        }
    }

    /// 综合命中处理：Ellipsis toggle / 下拉项 / 面包屑项。
    pub fn handle_click(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        pos: Point,
    ) -> BreadcrumbClick {
        // 下拉已开：项 → 返回；外 → 关闭。
        if self.menu.anim.is_open() {
            if let Some(i) = self.menu.item_at(pos) {
                self.menu.close();
                return BreadcrumbClick::Index(i);
            }
            if !rect.contains(pos) {
                self.menu.close();
                return BreadcrumbClick::None;
            }
        }
        match self.hit(engine, rect, pos) {
            BreadcrumbClick::Ellipsis => {
                self.toggle_ellipsis(engine, rect, screen);
                BreadcrumbClick::Ellipsis
            }
            other => other,
        }
    }

    /// 悬停路由：Ellipsis / 项 / 下拉项。
    pub fn hover(&mut self, engine: &TextEngine, rect: Rect, pos: Point) {
        self.hovered_ellipsis = self
            .ellipsis_rect(engine, rect)
            .map(|r| r.contains(pos))
            .unwrap_or(false);
        self.hovered_item = match self.hit(engine, rect, pos) {
            BreadcrumbClick::Index(i) => Some(i),
            _ => None,
        };
        if self.menu.anim.is_open() {
            self.menu.hovered = self.menu.item_at(pos);
        }
    }

    /// 每帧推进下拉动画。
    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
    }

    /// 固有尺寸（不折叠时的完整宽度）。
    pub fn measure(&self, engine: &TextEngine) -> Size {
        let style = Self::item_style();
        let widths: f32 = self
            .items
            .iter()
            .map(|i| engine.measure(i, style.size) + ITEM_PAD_X * 2.0)
            .sum();
        let n = self.items.len().saturating_sub(1) as f32;
        Size::new(
            widths + n * CHEVRON_GAP,
            style.line_height + ITEM_PAD_Y * 2.0,
        )
    }

    /// 渲染面包屑 +（折叠时）Ellipsis 下拉。
    pub fn render(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let style = Self::item_style();
        let layout = self.layout(engine, rect);

        // 容器语义 = 裁到自身矩形（参 docs/COMPOSITION.md 契约 12）：折叠布局「至少保留
        // 末项」在极窄宽度下连一项也放不下，不裁就会画到 rect 之外（2026-10-01 迁移发现）。
        // 下拉菜单画在 rect 之外，故在它之前 pop_clip。
        scene.push_clip(rect);

        let mut x = rect.origin.x;
        // Ellipsis
        if layout.ellipsis {
            let w = engine.measure("…", style.size) + ELLIPSIS_PAD;
            let er = Rect::new(
                x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
                w,
                style.line_height + ITEM_PAD_Y * 2.0,
            );
            if self.hovered_ellipsis {
                scene.fill_rounded_rect(
                    theme.indication.hover_tint,
                    er,
                    theme.tokens.corner_radius,
                );
            }
            scene.text(
                "…".into(),
                Rect::new(
                    x + ELLIPSIS_PAD / 2.0,
                    rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                    w - ELLIPSIS_PAD,
                    style.line_height,
                ),
                colors.on_surface,
                style,
                TextAlign::Center,
            );
            x += w + CHEVRON_GAP;
        }

        // 项
        for (i, label) in self.items.iter().enumerate().skip(layout.start) {
            let is_last = i == self.items.len() - 1;
            let w = engine.measure(label, style.size) + ITEM_PAD_X * 2.0;
            // 末段截断：前缀折叠完仍放不下时，只给末段其可用宽（`scene.text` 以省略号收束）。
            let draw_w = if is_last {
                layout.last_width.unwrap_or(w)
            } else {
                w
            };
            let hovered = self.hovered_item == Some(i);
            let bg = if hovered && !is_last {
                theme.indication.hover_tint
            } else {
                kanesumi_core::Color::TRANSPARENT
            };
            let item_rect = Rect::new(
                x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0 - ITEM_PAD_Y,
                draw_w,
                style.line_height + ITEM_PAD_Y * 2.0,
            );
            if bg.a > 0.0 {
                scene.fill_rounded_rect(bg, item_rect, theme.tokens.corner_radius);
            }
            let fg = if is_last || hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.text(
                label.clone(),
                Rect::new(
                    x + ITEM_PAD_X,
                    rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                    (draw_w - ITEM_PAD_X * 2.0).max(0.0),
                    style.line_height,
                ),
                fg,
                style,
                TextAlign::Left,
            );
            x += w;
            // Chevron（非末项）
            if !is_last {
                let chevron_rect = Rect::new(
                    x + 2.0,
                    rect.origin.y + (rect.size.height - 12.0) / 2.0,
                    12.0,
                    12.0,
                );
                glyph::chevron_right(scene, chevron_rect, colors.on_surface_variant);
                x += CHEVRON_GAP;
            }
        }

        scene.pop_clip();

        // Ellipsis 下拉
        if self.menu.anim.is_visible() {
            self.menu.render(theme, engine, screen, scene);
        }
    }
}

// ── 元素树接入（弹层类控件，参 docs/ELEMENT_MIGRATION.md §8）────────────────────
//
// 触发器（本控件）只负责「打开溢出面板」与命中面包屑项；被折叠的层级放进覆盖层上的
// 独立节点 `BreadcrumbOverflow`（包一个 `MetroDropdownMenu` 负责面板绘制与命中），
// 面板直接以**被折叠层级的原索引**发 `BreadcrumbClicked` 并关闭自身 —— App 只需处理
// 一种动作，不必把菜单索引换算回层级索引。
//
// 「展开中」用私有字段 `tree_popup` 记；面板因任何原因关闭（选中 / 点外部 / Esc）都会
// 收到 `Event::PopupClosed`，在那里复位，不自己猜关闭时机。

/// 元素树动作：某级面包屑被激活。`index` = 层级索引（溢出面板里的项也是原索引）；
/// `owner` = 面包屑控件（溢出面板里点选时，动作来源 id 是会被销毁的面板，App 按 owner 分派）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreadcrumbClicked {
    pub owner: kanesumi_element::WidgetId,
    pub index: usize,
}

impl MetroBreadcrumbBar {
    /// 元素树：打开被折叠层级的溢出面板（锚在 Ellipsis 下缘）。`keyboard` 为真时
    /// 焦点进面板并预选首项。返回弹层 id（首帧前无排版引擎时返回 None）。
    fn open_overflow(&mut self, ctx: &mut EventCtx, keyboard: bool) -> Option<WidgetId> {
        let engine = ctx.engine().cloned()?;
        let layout = self.layout(&engine, ctx.rect());
        let indices: Vec<usize> = (0..layout.start).collect();
        let labels: Vec<String> = indices.iter().map(|i| self.items[*i].clone()).collect();
        if indices.is_empty() {
            return None; // 无折叠项（未触发命中判定时的兜底）
        }
        let panel = BreadcrumbOverflow::new(ctx.id(), indices, labels, keyboard);
        let at = self
            .ellipsis_rect(&engine, ctx.rect())
            .map(|er| Point::new(er.origin.x, er.bottom()));
        let id = ctx.open_popup(
            panel,
            PopupSpec {
                at,
                gap: popup_gap(),
                ..PopupSpec::default()
            },
        );
        ctx.focus_widget(id, keyboard);
        Some(id)
    }
}

impl Widget for MetroBreadcrumbBar {
    /// 固有尺寸：不折叠时的完整宽度（转发旧 `measure`）。
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        MetroBreadcrumbBar::measure(self, ctx.engine())
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        // 遮罩不画（覆盖层目前无全屏遮罩机制，详见本批报告）；旧 `render` 里的下拉
        // 在树路径恒关闭（`menu.anim` 未开），不会重复绘制。
        MetroBreadcrumbBar::render(self, &theme, ctx.engine(), ctx.rect(), ctx.surface(), scene);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerMove { pos } => {
                if let Some(engine) = ctx.engine().cloned() {
                    self.hover(&engine, ctx.rect(), *pos);
                    ctx.invalidate_paint();
                }
            }
            Event::PointerLeave => {
                let had_item = self.hovered_item.take().is_some();
                let had_ellipsis = std::mem::replace(&mut self.hovered_ellipsis, false);
                if had_item || had_ellipsis {
                    ctx.invalidate_paint();
                }
            }
            // 记下按下位置：`Event::Click` 不带坐标，而命中哪一级必须按坐标算。
            // 弹层打开时的「点触发器」在 `pointer_down` 就被框架轻触关闭并按吞掉处理，
            // 届时不会投递本事件 —— 天然避免「关掉又立刻重开」。
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                self.press_pos = Some(*pos);
            }
            Event::Click => {
                let Some(pos) = self.press_pos.take() else {
                    return;
                };
                let Some(engine) = ctx.engine().cloned() else {
                    return;
                };
                match self.hit(&engine, ctx.rect(), pos) {
                    BreadcrumbClick::Index(i) => {
                        ctx.emit(BreadcrumbClicked {
                            owner: ctx.id(),
                            index: i,
                        });
                    }
                    BreadcrumbClick::Ellipsis => match self.tree_popup.take() {
                        Some(p) => ctx.close_popup(p),
                        None => self.tree_popup = self.open_overflow(ctx, false),
                    },
                    BreadcrumbClick::None => return,
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => {
                match key {
                    // Down / Enter / Space 打开溢出面板（键盘用户 Tab 到本控件后可直接开）。
                    Key::Down | Key::Enter | Key::Char(' ') => match self.tree_popup.take() {
                        Some(p) => ctx.close_popup(p),
                        None => self.tree_popup = self.open_overflow(ctx, true),
                    },
                    // Tab / Esc 留给框架（焦点遍历 / 弹层关闭）。
                    _ => return,
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::PopupClosed { popup } if self.tree_popup == Some(*popup) => {
                self.tree_popup = None;
                ctx.invalidate_paint();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: self
                .items
                .last()
                .cloned()
                .unwrap_or_else(|| "面包屑".to_string()),
            value: Some(self.items.join(" / ")),
            checked: None,
        })
    }
}

/// 溢出面板（覆盖层节点）：被折叠的层级即菜单项，选中时按原索引发动作。
struct BreadcrumbOverflow {
    /// 打开面板的面包屑控件。
    owner: kanesumi_element::WidgetId,
    /// 菜单项 → `MetroBreadcrumbBar::items` 的原始索引（一一对应）。
    indices: Vec<usize>,
    menu: MetroDropdownMenu,
}

impl BreadcrumbOverflow {
    /// `preselect` = 键盘打开时预选首项（焦点入面板后可直接 Enter）。
    fn new(
        owner: kanesumi_element::WidgetId,
        indices: Vec<usize>,
        labels: Vec<String>,
        preselect: bool,
    ) -> Self {
        let mut menu = MetroDropdownMenu::new(labels.into_iter().map(MenuItem::new).collect());
        menu.anim.open();
        if preselect {
            menu.hovered = (!menu.items.is_empty()).then_some(0);
        }
        Self {
            owner,
            indices,
            menu,
        }
    }

    /// 菜单项索引 → 被折叠层级的原索引。
    fn original_index(&self, menu_index: usize) -> Option<usize> {
        self.indices.get(menu_index).copied()
    }

    /// Up/Down 环状移动悬停项。
    fn step(&mut self, delta: isize) {
        let n = self.menu.items.len();
        if n == 0 {
            return;
        }
        self.menu.hovered = Some(match self.menu.hovered {
            Some(i) => (i as isize + delta).rem_euclid(n as isize) as usize,
            None if delta > 0 => 0,
            None => n - 1,
        });
    }

    /// 选中菜单第 `menu_index` 项：按原索引发动作并关闭自身。
    fn invoke(&mut self, ctx: &mut EventCtx, menu_index: usize) {
        if let Some(original) = self.original_index(menu_index) {
            ctx.emit(BreadcrumbClicked {
                owner: self.owner,
                index: original,
            });
            ctx.close_popup(ctx.id());
        }
    }
}

impl Widget for BreadcrumbOverflow {
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        self.menu.panel_size(ctx.engine())
    }

    fn arrange(&mut self, _ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.menu.panel_rect = rect;
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        self.menu.render_panel(&theme, ctx.engine(), scene);
        if self.menu.is_animating() {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        self.menu.update(dt);
        ctx.invalidate_paint();
        if self.menu.is_animating() {
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerMove { pos } => {
                if let Some(engine) = ctx.engine().cloned()
                    && self.menu.hover(&engine, ctx.surface(), *pos)
                {
                    ctx.invalidate_paint();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                if let Some(i) = self.menu.item_at(*pos) {
                    self.invoke(ctx, i);
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => {
                match key {
                    Key::Down => self.step(1),
                    Key::Up => self.step(-1),
                    Key::Enter | Key::Char(' ') => {
                        if let Some(i) = self.menu.hovered {
                            self.invoke(ctx, i);
                        }
                    }
                    // Tab / Esc 留给框架（焦点遍历 / 弹层关闭）。
                    _ => return,
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

    /// 面板以悬停高亮表示当前项，不要框架焦点框。
    fn focus_visual(&self) -> bool {
        false
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: "折叠层级".to_string(),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps};

    const ITEMS: [&str; 4] = ["首页", "文档", "项目", "Ether"];

    fn harness(width: Option<f32>) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(600.0, 300.0);
        let bar = MetroBreadcrumbBar::new(ITEMS.iter().map(|s| s.to_string()).collect());
        let id = h.tree.insert_with(
            h.root(),
            bar,
            LayoutProps {
                width,
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(20.0, 20.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    /// 折叠后 Ellipsis 中心（窄宽夹具用）。
    fn ellipsis_center(h: &TestHarness, id: WidgetId) -> Point {
        let r = h.rect(id);
        let bar = h.tree.get::<MetroBreadcrumbBar>(id).unwrap();
        bar.ellipsis_rect(&h.engine, r)
            .expect("折叠布局应有 Ellipsis")
            .center()
    }

    /// 可见面包屑项中心。
    fn level_center(h: &TestHarness, id: WidgetId, index: usize) -> Point {
        let r = h.rect(id);
        let bar = h.tree.get::<MetroBreadcrumbBar>(id).unwrap();
        bar.item_rect(&h.engine, r, index).center()
    }

    #[test]
    fn click_level_emits_breadcrumb_clicked() {
        let (mut h, id) = harness(None);
        h.click_at(level_center(&h, id, 0));
        assert_eq!(
            h.take::<BreadcrumbClicked>(),
            vec![(
                id,
                BreadcrumbClicked {
                    owner: id,
                    index: 0
                }
            )]
        );
        h.click_at(level_center(&h, id, 2));
        assert_eq!(
            h.take::<BreadcrumbClicked>(),
            vec![(
                id,
                BreadcrumbClicked {
                    owner: id,
                    index: 2
                }
            )]
        );
        // 末项（当前层级）仍是可点层级。
        h.click_at(level_center(&h, id, 3));
        assert_eq!(
            h.take::<BreadcrumbClicked>(),
            vec![(
                id,
                BreadcrumbClicked {
                    owner: id,
                    index: 3
                }
            )]
        );
    }

    #[test]
    fn ellipsis_opens_panel_and_pick_reports_original_index() {
        let (mut h, id) = harness(Some(120.0));
        let start = {
            let bar = h.tree.get::<MetroBreadcrumbBar>(id).unwrap();
            bar.layout(&h.engine, h.rect(id)).start
        };
        assert!(start >= 2, "窄宽应折叠出至少两级，实际 start={start}");
        h.click_at(ellipsis_center(&h, id));
        let p = h.tree.popups().next().expect("点 Ellipsis 应打开溢出面板");
        let pr = h.rect(p);
        assert!(
            pr.origin.y >= h.rect(id).bottom() - 0.5,
            "面板在锚点下方 {pr:?}"
        );
        assert_eq!(
            h.tree.get::<MetroBreadcrumbBar>(id).unwrap().tree_popup,
            Some(p),
            "触发器记下弹层"
        );
        // 点面板第二项：应发被折叠层级的**原索引**（= 1），不是菜单索引。
        h.click_at(Point::new(pr.origin.x + 20.0, pr.origin.y + 32.0 + 16.0));
        assert_eq!(
            h.take::<BreadcrumbClicked>(),
            vec![(
                p,
                BreadcrumbClicked {
                    owner: id,
                    index: 1
                }
            )]
        );
        assert!(h.tree.popups().next().is_none(), "选中后面板关闭");
        assert!(
            h.tree
                .get::<MetroBreadcrumbBar>(id)
                .unwrap()
                .tree_popup
                .is_none(),
            "PopupClosed 后复位"
        );
    }

    #[test]
    fn keyboard_opens_panel_focuses_and_returns() {
        let (mut h, id) = harness(Some(120.0));
        h.tab();
        assert_eq!(h.tree.focused(), Some(id), "Tab 聚焦面包屑");
        h.key(Key::Down); // 键盘打开：预选首项
        let p = h.tree.popups().next().expect("Down 应打开溢出面板");
        assert_eq!(h.tree.focused(), Some(p), "键盘打开焦点进面板");
        h.key(Key::Down); // 首项 → 次项
        h.key(Key::Enter);
        assert_eq!(
            h.take::<BreadcrumbClicked>(),
            vec![(
                p,
                BreadcrumbClicked {
                    owner: id,
                    index: 1
                }
            )]
        );
        assert_eq!(h.tree.focused(), Some(id), "关闭后焦点回触发器");
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn escape_and_outside_click_reset_tree_popup() {
        // Esc
        let (mut h, id) = harness(Some(120.0));
        h.click_at(ellipsis_center(&h, id));
        assert!(h.tree.popups().next().is_some());
        h.key(Key::Escape);
        assert!(h.tree.popups().next().is_none(), "Esc 关闭面板");
        assert!(h.take::<BreadcrumbClicked>().is_empty());
        assert!(
            h.tree
                .get::<MetroBreadcrumbBar>(id)
                .unwrap()
                .tree_popup
                .is_none()
        );
        // 点外部
        let (mut h, id) = harness(Some(120.0));
        h.click_at(ellipsis_center(&h, id));
        h.click_at(Point::new(580.0, 280.0));
        assert!(h.tree.popups().next().is_none(), "点外部关闭面板");
        assert!(h.take::<BreadcrumbClicked>().is_empty());
        assert!(
            h.tree
                .get::<MetroBreadcrumbBar>(id)
                .unwrap()
                .tree_popup
                .is_none()
        );
    }

    #[test]
    fn disabled_bar_does_not_open_panel() {
        let (mut h, id) = harness(Some(120.0));
        h.tree.set_enabled(id, false);
        h.click_at(ellipsis_center(&h, id));
        assert!(h.tree.popups().next().is_none(), "禁用后点击不打开面板");
        assert!(h.take::<BreadcrumbClicked>().is_empty());
    }

    #[test]
    fn passes_insurance_checks_closed_and_open() {
        let (mut h, id) = harness(Some(120.0));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
        h.click_at(ellipsis_center(&h, id));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
        let p = h.tree.popups().next().expect("面板应打开");
        h.assert_paint_within(p, Insets::ZERO);
    }

    #[test]
    fn squeezed_width_still_passes_insurance_checks() {
        let (h, id) = harness(Some(40.0));
        assert!(h.rect(id).size.width <= 40.0, "夹具确实压窄");
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn bar() -> MetroBreadcrumbBar {
        MetroBreadcrumbBar::new(
            ["首页", "文档", "项目", "Ether"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        )
    }

    #[test]
    fn wide_fits_no_collapse() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 800.0, 32.0);
        let l = b.layout(&engine, r);
        assert_eq!(l.start, 0);
        assert!(!l.ellipsis);
    }

    #[test]
    fn narrow_collapses_prefix() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let l = b.layout(&engine, r);
        assert!(l.ellipsis, "窄宽应折叠");
        assert!(l.start > 0);
        // 至少保留末项
        assert!(l.start <= 3);
    }

    #[test]
    fn last_item_always_kept() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 60.0, 32.0);
        let l = b.layout(&engine, r);
        assert!(l.ellipsis);
        assert_eq!(l.start, 3, "极窄也保留末项");
    }

    /// 前缀全折叠后仍放不下：末段以省略号截断（CONTROL_SPEC §18）。
    #[test]
    fn narrow_truncates_last_segment_with_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let r = Rect::new(0.0, 0.0, 60.0, 32.0);
        let l = b.layout(&engine, r);
        let last_w = l.last_width.expect("末段放不下应给截断宽");
        let full = b.item_rect(&engine, r, 3).size.width;
        assert!(last_w > 0.0 && last_w < full, "截断宽 {last_w} < 完整 {full}");
        // 末段文本框宽 = 截断宽 − padding，省略号由 `Scene::text` 收束。
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            r,
            Rect::new(0.0, 0.0, 400.0, 400.0),
            &mut scene,
        );
        let last = scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::Text { content, rect, .. } if content.as_str() == "Ether" => {
                    Some(*rect)
                }
                _ => None,
            })
            .expect("末段应渲染");
        assert!(
            (last.size.width - (last_w - ITEM_PAD_X * 2.0)).abs() < 0.5,
            "末段文本框应被夹到截断宽（{} vs {}）",
            last.size.width,
            last_w - ITEM_PAD_X * 2.0
        );
        // 完整放得下时不得截断。
        let wide = b.layout(&engine, Rect::new(0.0, 0.0, 800.0, 32.0));
        assert!(wide.last_width.is_none());
    }

    #[test]
    fn hit_maps_items_and_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 800.0, 32.0);
        // 第一项中心
        let first = b.item_rect(&engine, r, 0);
        assert_eq!(
            b.hit(&engine, r, Point::new(first.center().x, first.center().y)),
            BreadcrumbClick::Index(0)
        );
        // 无折叠 → 无 ellipsis
        assert_eq!(b.ellipsis_rect(&engine, r), None);
    }

    #[test]
    fn hit_ellipsis_when_collapsed() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let er = b.ellipsis_rect(&engine, r);
        assert!(er.is_some());
        assert_eq!(
            b.hit(
                &engine,
                r,
                Point::new(er.unwrap().center().x, er.unwrap().center().y)
            ),
            BreadcrumbClick::Ellipsis
        );
    }

    #[test]
    fn toggle_ellipsis_opens_menu() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let screen = Rect::new(0.0, 0.0, 400.0, 400.0);
        b.toggle_ellipsis(&engine, r, screen);
        assert!(!b.menu.items.is_empty(), "隐藏项应进入下拉");
        assert!(b.menu.anim.is_visible(), "下拉应打开（含 Opening）");
    }

    #[test]
    fn handle_click_ellipsis_then_hidden_item() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        let r = Rect::new(0.0, 0.0, 120.0, 32.0);
        let screen = Rect::new(0.0, 0.0, 400.0, 400.0);
        // 点 ellipsis
        let er = b.ellipsis_rect(&engine, r).unwrap();
        let c = b.handle_click(&engine, r, screen, Point::new(er.center().x, er.center().y));
        assert_eq!(c, BreadcrumbClick::Ellipsis);
        // 点下拉第一项
        b.menu.update(1.0);
        let panel = b.menu.panel_rect;
        let item = Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0);
        let c = b.handle_click(&engine, r, screen, item);
        assert!(matches!(c, BreadcrumbClick::Index(0)));
        b.menu.update(1.0);
        assert!(!b.menu.anim.is_open());
    }

    #[test]
    fn render_emits_items_and_chevrons() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 32.0),
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 4, "4 个面包屑项");
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 3, "3 个 chevron");
    }

    #[test]
    fn render_collapsed_has_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 120.0, 32.0),
            Rect::new(0.0, 0.0, 400.0, 400.0),
            &mut scene,
        );
        let texts: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "…"), "折叠应渲染 Ellipsis");
    }
}
