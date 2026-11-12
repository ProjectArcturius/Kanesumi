// dock.rs —— Dock 控件的**状态机 + 渲染**（几何复用 `navigation::dock_rail`）。
//
// 供应门 §Ⅰ 判据：Dock 这一格原先 5 项「应用自己顶」——launcher 自造了 3842 行 `dock_rail.rs`
// 与 8273 行 `mod.rs`，hover / pressed / running 指示线 / 双区分界全手写。`kanesumi-controls`
// 里只有几何（`navigation::dock_rail::DockRail::layout`），没有状态与渲染。
// 本文件补这两层：
//   - 状态：`Rest / Hovered / Pressed`（+ 外部传入的 pinned / running / urgent / 窗口数）；
//   - 渲染：底板、图标位、双区定位线（灰 = 钉选未运行，强调色 = 运行中）、紧急点、窗口计数。
//
// 双区语义（参 `docs/LAUNCHER_REBUILD.md`）：左区 = 钉选未运行（灰线），右区 = 所有运行（强调色线）。
// 交互（参 `docs/INTERACTION_CANON.md`）：按下时「按下」外观跟随指针是否仍在目标上，移出即恢复；
// 松开必须在**同一槽**上才算激活（拖出去松手取消）。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::typography::{TextStyle, base_weight};
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

use crate::navigation::dock_rail::{
    DockLayout, DockRail, DockRailEntry, DockRailInput, DockSlotLayout, DockZone,
};

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
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl Default for MetroDock {
    fn default() -> Self {
        Self::new()
    }
}

impl MetroDock {
    /// 空 Dock。
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            layout: DockRail::layout(&DockRailInput::new(Vec::new(), Rect::new(0.0, 0.0, 0.0, 0.0))),
            hovered: None,
            pressed: None,
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

    /// 条目。
    pub fn items(&self) -> &[DockItem] {
        &self.items
    }

    /// 当前布局。
    pub fn layout(&self) -> &DockLayout {
        &self.layout
    }

    /// 按可用矩形重算几何。
    pub fn relayout(&mut self, available: Rect) {
        let entries: Vec<DockRailEntry> = self
            .items
            .iter()
            .map(|it| {
                DockRailEntry::new(it.id.clone(), it.icon.clone())
                    .pinned(it.pinned)
                    .running(it.running)
                    .activated(it.urgent)
            })
            .collect();
        self.layout = DockRail::layout(&DockRailInput::new(entries, available));
    }

    /// 指针移动：更新悬停项；按下期间同步「按下」外观（移出即恢复）。
    pub fn hover(&mut self, pos: Point) -> bool {
        let target = self.source_at(pos);
        let changed = target != self.hovered;
        self.hovered = target;
        changed
    }

    /// 按下：在项上则进入 Pressed。
    pub fn press(&mut self, pos: Point) -> bool {
        let target = self.source_at(pos);
        self.hovered = target;
        self.pressed = target;
        target.is_some()
    }

    /// 松手：仍在该项上才算激活；否则取消。
    pub fn release(&mut self, pos: Point) -> Option<DockActivated> {
        let target = self.source_at(pos);
        let pressed = self.pressed.take();
        self.hovered = target;
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

    fn source_at(&self, pos: Point) -> Option<usize> {
        self.layout
            .slots
            .iter()
            .find(|s| s.hit.is_some_and(|h| contains(h, pos)))
            .map(|s| s.source)
    }

    /// 渲染：底板 → 每槽图标位 → 双区定位线 → 紧急点 → 窗口计数。
    ///
    /// 颜色规则（Kanesumi 铁律：直角、单一强调色）：
    /// - 底板 `surface`；悬停项图标位 `surface_variant`；按下项 `primary`；
    /// - 定位线：右区（运行中）= `primary`，左区（钉选未运行）= `on_surface_variant`；
    /// - 紧急点 = `status.critical`；窗口计数徽标文字用 `on_primary` 叠 `primary`。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let c = &theme.colors;
        if let Some(plate) = self.layout.plate {
            scene.fill_rect(c.surface, plate);
        }
        for slot in &self.layout.slots {
            if slot.hit.is_none() {
                continue;
            }
            let state = self.state_of(slot.source);
            let fill = match state {
                DockItemState::Pressed => c.primary,
                DockItemState::Hovered => c.surface_variant,
                DockItemState::Rest => c.divider,
            };
            scene.fill_rect(fill, slot.icon_rect);
            // 双区定位线：贴在图标位下沿，高 DOCK_LINE。
            let line_color = match slot.zone {
                DockZone::Pinned => c.on_surface_variant,
                DockZone::Running => c.primary,
            };
            scene.fill_rect(
                line_color,
                Rect::new(
                    slot.icon_rect.origin.x,
                    slot.icon_rect.bottom() + 2.0,
                    slot.icon_rect.size.width,
                    sizes::DOCK_LINE,
                ),
            );
            let item = self.items.get(slot.source);
            if item.is_some_and(|i| i.urgent) {
                let d = 8.0;
                scene.fill_rect(
                    theme.status.critical,
                    Rect::new(slot.icon_rect.right() - d, slot.icon_rect.origin.y, d, d),
                );
            }
            if let Some(i) = item
                && i.windows > 1
            {
                let label = format!("{}", i.windows);
                let style = TextStyle::new(11.0, 16.0, base_weight());
                let w = engine.measure(&label, style.size) + 8.0;
                let badge = Rect::new(
                    slot.icon_rect.right() - w,
                    slot.icon_rect.bottom() - 16.0,
                    w,
                    16.0,
                );
                scene.fill_rect(c.primary, badge);
                scene.text(label, badge, c.on_primary, style, TextAlign::Center);
            }
        }
    }
}

fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x && p.x < r.right() && p.y >= r.origin.y && p.y < r.bottom()
}

/// 供测试与调用方读取：某个槽位的定位线矩形（渲染与断言同源）。
pub fn indicator_rect(slot: &DockSlotLayout) -> Rect {
    Rect::new(
        slot.icon_rect.origin.x,
        slot.icon_rect.bottom() + 2.0,
        slot.icon_rect.size.width,
        sizes::DOCK_LINE,
    )
}
