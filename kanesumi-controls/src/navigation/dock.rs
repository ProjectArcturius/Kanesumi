// dock.rs —— Dock 控件的**状态机 + 渲染**（几何复用 `navigation::dock_rail`）。
//
// 供应门 §Ⅰ 判据：Dock 这一格原先 5 项「应用自己顶」——launcher 自造了 3842 行 `dock_rail.rs`
// 与 8273 行 `mod.rs`，hover / pressed / running 指示线 / 双区分界全手写。`kanesumi-controls`
// 里只有几何（`navigation::dock_rail::DockRail::layout`），没有状态与渲染。
// 本文件补这两层：
//   - 状态：`Rest / Hovered / Pressed`（+ 外部传入的 pinned / running / urgent / 窗口数）；
//   - 渲染：底板、图标位、入口格 / 计数格、双区定位线、紧急点、窗口计数、
//     关闭幽灵与启动呼吸（§185–§188 的瞬时态由调用方按 id 喂入）。
//
// 双区语义（参 `docs/LAUNCHER_REBUILD.md`）：左区 = 钉选未运行（灰线），右区 = 所有运行（强调色线）。
// 交互（参 `docs/INTERACTION_CANON.md`）：按下时「按下」外观跟随指针是否仍在目标上，移出即恢复；
// 松开必须在**同一槽**上才算激活（拖出去松手取消）。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect, ThemeColor, sizes};

use crate::navigation::dock_rail::{
    DockChipDirection, DockHit, DockLayout, DockRail, DockRailEntry, DockRailInput, DockSlotLayout,
    DockZone,
};

/// 关闭幽灵的图标不透明度（§185：关闭应用留下的淡出空位 = 25%）。
const GHOST_ICON_OPACITY: f32 = 0.25;

/// 让位空槽的哨兵 id。`DockRail` 不认识「空槽」这个输入，控件用一条合成条目让它
/// 参与同一套排序 / 底板 / 视口计算；`source` 在回填时改成 `usize::MAX`，任何按
/// `items[source]` 回查的绘制路径都会自然跳过它。
const PIN_GAP_ID: &str = "\u{0}pin-gap";

/// Dock 里的一项（数据由调用方给；控件不读系统状态）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockItem {
    /// 稳定 id（desktop id / app id）。
    pub id: String,
    /// 图标名（主题图标名；渲染侧按它取图，本控件只画位）。
    pub icon: String,
    /// 是否钉选。
    pub pinned: bool,
    /// 是否运行。
    pub running: bool,
    /// 是否紧急（需要用户注意；画一个语义点）。
    pub urgent: bool,
    /// 窗口数（>1 时出计数徽标）。
    pub windows: u32,
    /// 是否有**活动窗口**（焦点在该应用上）。与 `urgent` 是两件事：
    /// `active` = 你正在用它，`urgent` = 它需要你注意。
    pub active: bool,
}

impl DockItem {
    /// 新建一项。
    pub fn new(id: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            pinned: false,
            running: false,
            urgent: false,
            windows: 0,
            active: false,
        }
    }

    /// 钉选。
    pub fn pinned(mut self, on: bool) -> Self {
        self.pinned = on;
        self
    }

    /// 运行中。
    pub fn running(mut self, on: bool) -> Self {
        self.running = on;
        self
    }

    /// 紧急。
    pub fn urgent(mut self, on: bool) -> Self {
        self.urgent = on;
        self
    }

    /// 窗口数。
    pub fn windows(mut self, n: u32) -> Self {
        self.windows = n;
        self
    }

    /// 有活动窗口（焦点在本应用上）。
    pub fn active(mut self, on: bool) -> Self {
        self.active = on;
        self
    }
}

/// 单项交互态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockItemState {
    /// 静置。
    Rest,
    /// 指针在上。
    Hovered,
    /// 已按下且指针仍在本项上。
    Pressed,
}

/// 激活事件（松手在**同一槽**上）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockActivated {
    /// 模型 id。
    pub id: String,
    /// 模型下标（`items` 里的位置，不是呈现序号）。
    pub source: usize,
}

/// Dock 控件：状态机 + 渲染。几何每帧按可用矩形重算。
#[derive(Debug, Clone)]
pub struct MetroDock {
    items: Vec<DockItem>,
    layout: DockLayout,
    /// 最近一次 `relayout` 的可用矩形 —— 输入类 setter 改字段后据此重算。
    available: Rect,
    hovered: Option<usize>,
    pressed: Option<usize>,
    /// 指针位置原样留存：入口格 / 计数格的悬停底与 `DockRailInput.hover` 都要它。
    hover_pos: Option<Point>,
    /// 当前命中的完整目标（槽位 / 入口格 / 计数格；幽灵已过滤）。
    hovered_hit: Option<DockHit>,
    scale: f32,
    show_entry: bool,
    scroll: [usize; 2],
    pin_gap: Option<usize>,
    ghosts: Vec<String>,
    starting: Vec<String>,
    /// 启动呼吸 / 轻跳的相位（0..1 一个周期）。控件不自带时钟：由调用方用 Sokuou
    /// 进度驱动，测试与外壳拿到的是同一份确定值。
    starting_progress: f32,
}

impl Default for MetroDock {
    fn default() -> Self {
        Self::new()
    }
}

impl MetroDock {
    /// 空 Dock。
    pub fn new() -> Self {
        let available = Rect::new(0.0, 0.0, 0.0, 0.0);
        Self {
            items: Vec::new(),
            layout: DockRail::layout(&DockRailInput::new(Vec::new(), available)),
            available,
            hovered: None,
            pressed: None,
            hover_pos: None,
            hovered_hit: None,
            scale: 1.0,
            show_entry: false,
            scroll: [0, 0],
            pin_gap: None,
            ghosts: Vec::new(),
            starting: Vec::new(),
            starting_progress: 0.0,
        }
    }

    /// 整体替换条目（保持 hover / press 只在仍存在的项上）。
    pub fn set_items(&mut self, items: Vec<DockItem>, available: Rect) {
        self.items = items;
        if self.hovered.is_some_and(|i| i >= self.items.len()) {
            self.hovered = None;
        }
        if self.pressed.is_some_and(|i| i >= self.items.len()) {
            self.pressed = None;
        }
        self.relayout(available);
    }

    /// 轨道缩放（1× / 1.5× / 2× …）。非法值由 `DockRail` 退回 1×。
    pub fn set_scale(&mut self, scale: f32) {
        self.scale = scale;
        self.relayout_available();
    }

    /// 是否显示左端入口格（§187）。打开后整组按含入口格的宽度重新居中。
    pub fn set_show_entry(&mut self, on: bool) {
        self.show_entry = on;
        self.relayout_available();
    }

    /// 每区已滚出的整槽数（§186 溢出视口）。超限值由 `DockRail` 夹紧后回传。
    pub fn set_scroll(&mut self, scroll: [usize; 2]) {
        self.scroll = scroll;
        self.relayout_available();
    }

    /// 拖动钉选时的让位空槽（§188）：在第 `n` 个左区槽前插一格。
    ///
    /// 只影响非溢出布局（与 launcher 一致：溢出时左区视口、计数格、底板宽三处会联动，
    /// 拖动钉选走到溢出的情形极窄）。`n` 超界按左区槽数夹紧。
    pub fn set_pin_gap(&mut self, gap: Option<usize>) {
        self.pin_gap = gap;
        self.relayout_available();
    }

    /// 关闭幽灵（§185）：这些 id 的槽位图标按 25% 画、定位线回灰、不参与命中。
    pub fn set_ghosts(&mut self, ghosts: &[String]) {
        self.ghosts = ghosts.to_vec();
        self.refresh_hover();
    }

    /// 启动中的 id（§185）：定位线在运行色与灰之间呼吸，图标位轻跳。
    pub fn set_starting(&mut self, starting: &[String]) {
        self.starting = starting.to_vec();
    }

    /// 启动相位（0..1，一个呼吸周期）。非有限值按 0 处理，越界夹紧。
    pub fn set_starting_progress(&mut self, progress: f32) {
        self.starting_progress = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// 指针位置（`None` = 离开轨道）。返回悬停目标是否变化。
    pub fn set_hover(&mut self, pos: Option<Point>) -> bool {
        let before = self.hovered_hit;
        self.hover_pos = pos;
        self.refresh_hover();
        before != self.hovered_hit
    }

    /// 条目。
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    /// 当前布局。
    pub fn layout(&self) -> &DockLayout {
        &self.layout
    }

    /// 当前命中的完整目标（幽灵槽已过滤）。入口格 / 计数格的命中从这里取。
    pub fn hit(&self, pos: Point) -> Option<DockHit> {
        match self.layout.hit(pos)? {
            DockHit::Slot(display) => {
                let slot = self.layout.slot(display)?;
                if slot.id == PIN_GAP_ID || self.is_ghost(&slot.id) {
                    None
                } else {
                    Some(DockHit::Slot(display))
                }
            }
            other => Some(other),
        }
    }

    /// 让位空槽的矩形（§188；无空槽 / 溢出时 `None`）。
    pub fn pin_gap_rect(&self) -> Option<Rect> {
        let slot = self.layout.slots.iter().find(|s| s.id == PIN_GAP_ID)?;
        Some(Rect::new(
            slot.rect.origin.x,
            slot.rect.origin.y,
            slot.rect.size.width,
            self.layout.metrics.plate_height.min(slot.rect.size.height),
        ))
    }

    /// 入口格图标位（32 居中；未开启入口格为 `None`）。控件不解码图标，调用方按此位 blit。
    pub fn entry_icon_rect(&self) -> Option<Rect> {
        let entry = self.layout.entry?;
        let plate_h = self.layout.metrics.plate_height.min(entry.size.height);
        Some(Rect::new(
            entry.origin.x + (entry.size.width - sizes::ICON_32) / 2.0,
            entry.origin.y + ((plate_h - sizes::ICON_32) / 2.0).max(0.0),
            sizes::ICON_32,
            sizes::ICON_32,
        ))
    }

    /// 图标不透明度：关闭幽灵 = 25%，其余 1.0。图标层按它乘一次即可。
    pub fn icon_opacity(&self, id: &str) -> f32 {
        if self.is_ghost(id) {
            GHOST_ICON_OPACITY
        } else {
            1.0
        }
    }

    /// 启动中图标的纵向轻跳偏移（pt，正值 = 上移；非启动项 = 0）。
    ///
    /// 与定位线呼吸共用 `starting_progress`：0 / 1 落回原位，半个周期抬到 3 pt。
    pub fn icon_jump(&self, id: &str) -> f32 {
        if !self.is_starting(id) {
            return 0.0;
        }
        let phase = self.starting_progress * std::f32::consts::TAU;
        sizes::DOCK_START_HOP * (0.5 - 0.5 * phase.cos())
    }

    /// 按可用矩形重算几何。
    pub fn relayout(&mut self, available: Rect) {
        self.available = available;
        let entries: Vec<DockRailEntry> = self
            .items
            .iter()
            .map(|it| {
                DockRailEntry::new(it.id.clone(), it.icon.clone())
                    .pinned(it.pinned)
                    .running(it.running)
                    .activated(it.active)
            })
            .collect();
        let input = self.rail_input(entries.clone());
        let mut layout = DockRail::layout(&input);
        // 让位空槽走「合成一条条目」：`DockRail` 的排序 / 底板 / 视口全部照旧，
        // 空槽自然占一格并把左区后续槽右移。溢出时不让位（与 launcher 同判据）。
        let left_len = self.left_len();
        if let Some(gap) = self.pin_gap.map(|n| n.min(left_len))
            && !layout.overflow
        {
            let insert_at = self.gap_insert_index(gap);
            let mut with_gap = entries;
            with_gap.insert(insert_at, DockRailEntry::new(PIN_GAP_ID, "").pinned(true));
            let input = self.rail_input(with_gap);
            layout = DockRail::layout(&input);
            for slot in &mut layout.slots {
                if slot.source == insert_at {
                    slot.source = usize::MAX;
                    slot.id = PIN_GAP_ID.to_string();
                } else if slot.source > insert_at {
                    slot.source -= 1;
                }
            }
        }
        self.layout = layout;
        self.refresh_hover();
    }

    fn relayout_available(&mut self) {
        let available = self.available;
        self.relayout(available);
    }

    fn rail_input(&self, entries: Vec<DockRailEntry>) -> DockRailInput {
        let mut input = DockRailInput::new(entries, self.available);
        input.scale = self.scale;
        input.show_entry = self.show_entry;
        input.scroll = self.scroll;
        input.hover = self.hover_pos;
        input
    }

    fn left_len(&self) -> usize {
        self.items.iter().filter(|i| i.pinned && !i.running).count()
    }

    /// 左区第 `gap` 个槽在输入里的插入位置（0..=左区槽数）。
    fn gap_insert_index(&self, gap: usize) -> usize {
        let mut seen = 0;
        for (i, item) in self.items.iter().enumerate() {
            if item.pinned && !item.running {
                if seen == gap {
                    return i;
                }
                seen += 1;
            }
        }
        self.items.len()
    }

    fn refresh_hover(&mut self) {
        self.hovered_hit = self.hover_pos.and_then(|p| self.hit(p));
        self.hovered = match self.hovered_hit {
            Some(DockHit::Slot(display)) => self.layout.slot(display).map(|s| s.source),
            _ => None,
        };
        // 快照与控件查询同源：`DockRailInput.hover` 在 relayout 时喂过一份，但随后
        // 幽灵过滤 / 指针移动都会改命中，这里统一回写。
        self.layout.hovered = self.hovered_hit;
    }

    fn is_ghost(&self, id: &str) -> bool {
        self.ghosts.iter().any(|g| g == id)
    }

    fn is_starting(&self, id: &str) -> bool {
        self.starting.iter().any(|s| s == id)
    }

    /// 指针移动：更新悬停项；按下期间同步「按下」外观（移出即恢复）。
    pub fn hover(&mut self, pos: Point) -> bool {
        self.set_hover(Some(pos))
    }

    /// 按下：在项上则进入 Pressed。
    pub fn press(&mut self, pos: Point) -> bool {
        self.set_hover(Some(pos));
        let target = self.hovered;
        self.pressed = target;
        target.is_some()
    }

    /// 松手：仍在该项上才算激活；否则取消。
    pub fn release(&mut self, pos: Point) -> Option<DockActivated> {
        self.set_hover(Some(pos));
        let target = self.hovered;
        let pressed = self.pressed.take();
        match (pressed, target) {
            (Some(a), Some(b)) if a == b => Some(DockActivated {
                id: self.items.get(a)?.id.clone(),
                source: a,
            }),
            _ => None,
        }
    }

    /// 取消按下（Esc / 失焦）。
    pub fn cancel(&mut self) {
        self.pressed = None;
    }

    /// 某项的交互态。
    pub fn state_of(&self, source: usize) -> DockItemState {
        if self.pressed == Some(source) && self.hovered == Some(source) {
            DockItemState::Pressed
        } else if self.hovered == Some(source) {
            DockItemState::Hovered
        } else {
            DockItemState::Rest
        }
    }

    /// 悬停项的模型下标。
    pub fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    /// 当前悬停目标（含入口格 / 计数格）。
    pub fn hovered_hit(&self) -> Option<DockHit> {
        self.hovered_hit
    }

    /// 渲染：底板 → 入口格悬停底 → 每槽状态底 / 定位线 → 计数格 → 紧急点 / 徽标。
    ///
    /// 颜色规则（C0 转录的真值，参 `docs/research/m1115_dock/REPORT.md` §三）：
    /// - 底板 `surface`；静置格**不铺底**（露底板）；
    /// - 悬停 / 按下 / 运行且激活 = `hover_tint` 10% 叠 `surface`（按下不再变强调色实心）；
    /// - 定位线用 `slot.indicator` 与 `DockIndicatorKind::token()`：Pinned→Divider、
    ///   Running→Primary、Focused→PrimaryHover；
    /// - 关闭幽灵的线回 Divider；启动中的线在 Primary↔Divider 间呼吸。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let c = &theme.colors;
        if let Some(plate) = self.layout.plate {
            scene.fill_rect(c.surface, plate);
        }
        // 10% 叠色先算成实色：合成算术在 `Color::over`（core 只提供算法，不新增
        // 颜色令牌；卡 §二 C 允许控件内算）。叠在 surface 上即真值 #2e2e2e。
        let hover_fill = theme.indication.hover_tint.over(c.surface);
        let plate_h = self.layout.metrics.plate_height;

        // 入口格（§187）：只画悬停底；图标位由 `entry_icon_rect()` 交给调用方 blit。
        if let Some(entry) = self.layout.entry
            && self.hover_pos.is_some_and(|p| entry.contains(p))
        {
            scene.fill_rect(
                hover_fill,
                Rect::new(
                    entry.origin.x,
                    entry.origin.y,
                    entry.size.width,
                    plate_h.min(entry.size.height),
                ),
            );
        }

        for slot in &self.layout.slots {
            // 让位空槽（§188）：只占位、只画左区灰线，不画图标 / 底 / 徽标。
            if slot.id == PIN_GAP_ID {
                if let Some(line) = try_indicator_rect(slot) {
                    scene.fill_rect(c.divider, line);
                }
                continue;
            }
            if !slot.visible || slot.hit.is_none() {
                continue;
            }
            let Some(item) = self.items.get(slot.source) else {
                continue;
            };
            let state = self.state_of(slot.source);
            let ghost = self.is_ghost(&item.id);
            let highlighted = matches!(state, DockItemState::Hovered | DockItemState::Pressed)
                || (item.running && item.active);
            if highlighted {
                scene.fill_rect(
                    hover_fill,
                    Rect::new(
                        slot.rect.origin.x,
                        slot.rect.origin.y,
                        slot.rect.size.width,
                        plate_h.min(slot.rect.size.height),
                    ),
                );
            }
            if let Some(line) = try_indicator_rect(slot) {
                let color = if ghost {
                    c.divider
                } else if self.is_starting(&item.id) {
                    self.breath_color(theme)
                } else {
                    slot.indicator_kind.token().resolve(theme)
                };
                scene.fill_rect(color, line);
            }
            if item.urgent {
                let d = 8.0;
                scene.fill_rect(
                    theme.status.critical,
                    Rect::new(slot.icon_rect.right() - d, slot.icon_rect.origin.y, d, d),
                );
            }
            if item.windows > 1 {
                let jump = self.icon_jump(&item.id);
                let icon = Rect::new(
                    slot.icon_rect.origin.x,
                    slot.icon_rect.origin.y - jump,
                    slot.icon_rect.size.width,
                    slot.icon_rect.size.height,
                );
                self.paint_count_badge(
                    theme,
                    scene,
                    icon,
                    item.windows,
                    self.icon_opacity(&item.id),
                );
            }
        }

        // 计数格（§186）：格底 + 区基色线 + 数字。数字走 `Scene::label`（engine 排版）。
        for zone in &self.layout.zones {
            for chip in &zone.chips {
                if self.hover_pos.is_some_and(|p| chip.rect.contains(p)) {
                    scene.fill_rect(
                        hover_fill,
                        Rect::new(
                            chip.rect.origin.x,
                            chip.rect.origin.y,
                            chip.rect.size.width,
                            plate_h.min(chip.rect.size.height),
                        ),
                    );
                }
                let line_color = match zone.zone {
                    DockZone::Pinned => c.divider,
                    DockZone::Running => c.primary,
                };
                scene.fill_rect(line_color, chip.line);
                let text = match chip.direction {
                    DockChipDirection::Prev => format!("‹ {}", chip.count),
                    DockChipDirection::Next => format!("{} ›", chip.count),
                };
                let style = theme.typography.body;
                let lh = engine.line_height(style.size);
                let h = plate_h.min(chip.rect.size.height);
                scene.label(
                    text,
                    Rect::new(
                        chip.rect.origin.x,
                        chip.rect.origin.y + ((h - lh) / 2.0).max(0.0),
                        chip.rect.size.width,
                        lh,
                    ),
                    c.on_surface,
                    style,
                    TextAlign::Center,
                );
            }
        }
    }

    /// 启动呼吸色：相位 0 取运行色，半周期取灰（与 launcher 的 `breathe` 同式）。
    fn breath_color(&self, theme: &MetroTheme) -> Color {
        let t = 0.5 - 0.5 * (self.starting_progress * std::f32::consts::TAU).cos();
        ThemeColor::Primary
            .resolve(theme)
            .lerp(ThemeColor::Divider.resolve(theme), t as f64)
    }

    /// 多窗口计数徽标：图标右上角 15 pt 胶囊（§185 / launcher `paint_count_badge`）。
    ///
    /// 宽度按字数分三档（1 位 15 / 2 位 19 / 更多 24），不按 `engine.measure` 取 ——
    /// 保持与 C0 旧实现的形状逐像素一致；文字仍由 `Scene::label` 交给 engine 排版。
    fn paint_count_badge(
        &self,
        theme: &MetroTheme,
        scene: &mut Scene,
        icon: Rect,
        count: u32,
        opacity: f32,
    ) {
        let text = if count > 99 {
            "99+".to_string()
        } else {
            count.to_string()
        };
        let w = match text.len() {
            1 => 15.0,
            2 => 19.0,
            _ => 24.0,
        };
        let h = 15.0;
        let badge = Rect::new(
            icon.origin.x + icon.size.width - w + 3.0,
            icon.origin.y - 1.0,
            w,
            h,
        );
        scene.fill_rounded_rect(
            theme.colors.primary.with_alpha(opacity),
            badge,
            CornerRadius::Capsule,
        );
        scene.label(
            text,
            Rect::new(badge.origin.x, badge.origin.y + 0.5, w, h),
            theme.colors.on_primary.with_alpha(opacity),
            theme.typography.label,
            TextAlign::Center,
        );
    }
}

/// 旧定位线几何查询。保留返回类型与历史算术，避免已落地的调用方编译失败。
/// 新渲染只消费轨道的可见定位线，参 `docs/CONTROL_SPEC.md`「MetroDock 定位线查询的兼容契约」。
#[deprecated(note = "请改用 try_indicator_rect，读取轨道生成的可见定位线")]
pub fn indicator_rect(slot: &DockSlotLayout) -> Rect {
    Rect::new(
        slot.icon_rect.origin.x,
        slot.icon_rect.bottom() + 2.0,
        slot.icon_rect.size.width,
        sizes::DOCK_LINE,
    )
}

/// 轨道生成的可见定位线。被完全裁掉的槽没有线。
pub fn try_indicator_rect(slot: &DockSlotLayout) -> Option<Rect> {
    slot.indicator
}
