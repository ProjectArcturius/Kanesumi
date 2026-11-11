// dock_rail.rs —— Dock 轨道的纯几何提炼。参 docs/supply/DOCK_RAIL.md。
//
// 提炼自 launcher 的 `shell/dock_rail.rs`（arrange 的槽位 / 视口 / 计数格、`slot_at` 命中、
// 定位线段与双区分界）。**只做计算**：不碰 Scene、不读主题、不持有事件状态；这样合成器侧
// 桥、元素树或别的外壳都能拿同一份布局产物。
//
// 为什么输入只要「钉选 / 运行 / 激活」三个布尔 + 可用矩形：launcher 的两区顺序由模型决定
// （左区钉选未运行、右区钉选运行在前、其余运行按出现顺序），这里按同一规则**稳定重排**，
// 输入顺序只影响区内次序，调用方不必先算分区。
//
// 形状冻结约定（参 Kanesumi docs/API_STABILITY.md §三）：现有字段不改名、不改语义；
// 新增字段必须带 `Default` 或由 `DockRailInput::new` 填好，枚举只许加变体。

use kanesumi_core::{Point, Rect, ThemeColor};

/// 图标边长（§158：48 为 200% 输出的最终值）。
pub const DOCK_ICON_SIZE: f32 = 48.0;
/// 槽宽（图标居中，槽间无缝）。
pub const DOCK_SLOT_WIDTH: f32 = 60.0;
/// 板高 = 图标 + 板内上下各 10。
pub const DOCK_PLATE_HEIGHT: f32 = 68.0;
/// 悬浮：板底距屏底 8；这 8 pt 仍属 Dock 表面，命中延伸到屏幕底边（§158）。
pub const DOCK_FLOAT_GAP: f32 = 8.0;
/// 悬浮档的 Dock 主表面高（板 + 间隙），与合成器 `dock_visibility::DOCK_HEIGHT` 一致。
pub const DOCK_SURFACE_HEIGHT: f32 = DOCK_PLATE_HEIGHT + DOCK_FLOAT_GAP;
/// 定位线高（每槽一段、贴板内底沿）。
pub const DOCK_LINE_HEIGHT: f32 = 3.0;
/// 底板左右内边距（居中直角底板只包图标区）。
pub const DOCK_PLATE_PAD: f32 = 12.0;
/// 溢出判定：可用宽 = 输出逻辑宽 − 2 × 72（§186）。
pub const DOCK_SIDE_MARGIN: f32 = 72.0;
/// 计数格宽（§186：「‹ N」/「N ›」）。
pub const DOCK_CHIP_WIDTH: f32 = 36.0;

/// 一档缩放后的全部长度。
///
/// `scale` 乘到每个长度上（含命中区与定位线）；`available` 必须与结果在同一坐标空间。
/// 非有限值或 ≤ 0 按 1 处理（与 launcher 的「尺寸只进光栅化」不同：这里是整条轨道缩放）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockMetrics {
    pub scale: f32,
    pub icon: f32,
    pub slot_width: f32,
    pub plate_height: f32,
    pub float_gap: f32,
    pub line_height: f32,
    pub plate_pad: f32,
    pub side_margin: f32,
    pub chip_width: f32,
}

impl DockMetrics {
    /// 1× 基准档（§158 悬浮档）。
    pub const BASE: Self = Self {
        scale: 1.0,
        icon: DOCK_ICON_SIZE,
        slot_width: DOCK_SLOT_WIDTH,
        plate_height: DOCK_PLATE_HEIGHT,
        float_gap: DOCK_FLOAT_GAP,
        line_height: DOCK_LINE_HEIGHT,
        plate_pad: DOCK_PLATE_PAD,
        side_margin: DOCK_SIDE_MARGIN,
        chip_width: DOCK_CHIP_WIDTH,
    };

    /// 按系数缩放全部长度；非法系数退回 1×（不静默改变调用方的 rect）。
    pub fn scaled(scale: f32) -> Self {
        let s = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let b = Self::BASE;
        Self {
            scale: s,
            icon: b.icon * s,
            slot_width: b.slot_width * s,
            plate_height: b.plate_height * s,
            float_gap: b.float_gap * s,
            line_height: b.line_height * s,
            plate_pad: b.plate_pad * s,
            side_margin: b.side_margin * s,
            chip_width: b.chip_width * s,
        }
    }

    /// Dock 主表面高（板 + 悬浮间隙）。
    pub fn dock_height(self) -> f32 {
        self.plate_height + self.float_gap
    }
}

impl Default for DockMetrics {
    fn default() -> Self {
        Self::BASE
    }
}

/// 槽位归属区。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockZone {
    /// 左区：钉选且未运行。
    Pinned,
    /// 右区：运行中。
    Running,
}

/// 轨道方向。`Rtl` 当前**尚未实现**：几何与 `Ltr` 相同，只回显在快照里。
///
/// TODO(n1111-dock-rail): 镜像排布（入口格在右、定位线从右往左、计数格方向取反）需要
/// 先定「RTL 下图标是否也镜像」与溢出视口的取舍；launcher 现状无 RTL 路径，未凭空实现。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DockDirection {
    #[default]
    Ltr,
    Rtl,
}

/// 一个待布局条目。`id` / `icon` 只随快照回传，本控件不解析图标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockRailEntry {
    pub id: String,
    pub icon: String,
    pub running: bool,
    pub pinned: bool,
    /// 运行区里最近激活的窗口；决定定位线亮档（`PrimaryHover`）。
    pub activated: bool,
}

impl DockRailEntry {
    pub fn new(id: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            running: false,
            pinned: false,
            activated: false,
        }
    }

    pub fn pinned(mut self, on: bool) -> Self {
        self.pinned = on;
        self
    }

    pub fn running(mut self, on: bool) -> Self {
        self.running = on;
        self
    }

    pub fn activated(mut self, on: bool) -> Self {
        self.activated = on;
        self
    }
}

/// 布局输入。`scroll` 是每区已滚出的整槽数（溢出视口），会被夹到本区上限后回传。
#[derive(Debug, Clone, PartialEq)]
pub struct DockRailInput {
    pub entries: Vec<DockRailEntry>,
    /// Dock 主表面矩形（逻辑或已缩放的同一空间；高通常 = `metrics.dock_height()`）。
    pub available: Rect,
    /// 缩放系数（1× / 1.5× / 2× …）。
    pub scale: f32,
    /// 左端入口格（§187 开始格）：固定一格、不属任何区、下面不画定位线。
    pub show_entry: bool,
    /// 指针位置（用于 `hovered` 快照）；`None` = 不在轨道上。
    pub hover: Option<Point>,
    /// 每区已滚出的槽数（呈现序号不是模型序号，调用方按 `slots[].source` 回查）。
    pub scroll: [usize; 2],
    pub direction: DockDirection,
}

impl DockRailInput {
    pub fn new(entries: Vec<DockRailEntry>, available: Rect) -> Self {
        Self {
            entries,
            available,
            scale: 1.0,
            show_entry: false,
            hover: None,
            scroll: [0, 0],
            direction: DockDirection::Ltr,
        }
    }
}

/// 定位线语义档 —— 几何由本控件给，颜色由绘制层用主题令牌解析。
///
/// 映射与 launcher 现状一致（`zone_color` / `seg_color`）：左区 `Divider`、运行 `Primary`、
/// 运行且激活 `PrimaryHover`。启动呼吸 / 关闭幽灵这类**瞬时应用状态**不在输入里，由调用方
/// 在绘制期覆盖颜色（它们不是稳态布局的一部分）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DockIndicatorKind {
    Pinned,
    Running,
    Focused,
}

impl DockIndicatorKind {
    pub fn token(self) -> ThemeColor {
        match self {
            Self::Pinned => ThemeColor::Divider,
            Self::Running => ThemeColor::Primary,
            Self::Focused => ThemeColor::PrimaryHover,
        }
    }
}

/// 计数格方向：`Prev` = 「‹ N」在本区前端，`Next` = 「N ›」在本区末端。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockChipDirection {
    Prev,
    Next,
}

/// 一个计数格（§186 溢出时按区翻页）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockChipLayout {
    pub zone: DockZone,
    pub direction: DockChipDirection,
    /// 整格矩形（板高，含本格悬停底）。
    pub rect: Rect,
    /// 格下定位线（整格宽，颜色取本区基色）。
    pub line: Rect,
    /// 格上显示的数字：`Prev` = 前方被挡槽数，`Next` = 后方被挡槽数。
    pub count: usize,
}

/// 一个槽位的布局产物。
#[derive(Debug, Clone, PartialEq)]
pub struct DockSlotLayout {
    /// 输入 `entries` 的下标 —— 点击 / 菜单应按这个回查模型，不要拿呈现序号当模型序号。
    pub source: usize,
    /// 呈现序号（槽数组下标；与 launcher 的 `DockSlotInvoked.index` 同义）。
    pub display: usize,
    pub zone: DockZone,
    pub id: String,
    pub icon: String,
    /// 整槽矩形：宽 = 槽宽，高 = 轨道高（含下方悬浮间隙）。
    pub rect: Rect,
    /// 可点区域。溢出时按区视口水平裁剪；被完全挡住 = `None`。
    pub hit: Option<Rect>,
    /// 槽内图标（48 居中，板内上下各 10）。
    pub icon_rect: Rect,
    /// 本帧是否在区视口内（决定画不画图标 / 线）。
    pub visible: bool,
    /// 定位线段（贴板内底沿，高 = `line_height`）。
    pub indicator: Option<Rect>,
    pub indicator_kind: DockIndicatorKind,
}

/// 一个区的布局产物。空区 `rect.width == 0`（与 launcher 的 `zone_rects` 同）。
#[derive(Debug, Clone, PartialEq)]
pub struct DockZoneLayout {
    pub zone: DockZone,
    pub rect: Rect,
    /// 本区首槽在 `slots` 里的下标（可能等于 `slots.len()`）。
    pub first_display: usize,
    /// 视口可容的槽数。
    pub visible: usize,
    /// 前方被挡、已滚出的槽数（夹到本区上限后）。
    pub before: usize,
    /// 后方被挡、未滚到的槽数。
    pub after: usize,
    /// 0..2 枚计数格（`before > 0` 一枚、`after > 0` 一枚）。
    pub chips: Vec<DockChipLayout>,
}

/// 命中结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DockHit {
    /// 左端入口格（§187）。
    Entry,
    /// 槽位（呈现序号；用 `DockLayout::slots[display].source` 回查模型）。
    Slot(usize),
    /// 计数格：翻本区一页。
    Chip {
        zone: DockZone,
        direction: DockChipDirection,
    },
}

/// 一次布局的完整快照 —— 绘制层只读它，不再自己算几何。
#[derive(Debug, Clone, PartialEq)]
pub struct DockLayout {
    pub metrics: DockMetrics,
    /// 归一化后的输入矩形。
    pub dock: Rect,
    /// 居中底板（图标区 + 左右内边距）；零条目且无入口格时为 `None`。
    pub plate: Option<Rect>,
    /// 左端入口格（未启用为 `None`）。
    pub entry: Option<Rect>,
    /// `[左区, 右区]`；左区 = 钉选未运行，右区 = 所有运行。
    pub zones: [DockZoneLayout; 2],
    /// 按呈现顺序（左区 → 右区）排列的槽位。
    pub slots: Vec<DockSlotLayout>,
    /// 左区条目数 = 双区分界在 `slots` 里的下标。
    pub split: usize,
    /// 两区相接的 x（仅两区都有条目时 `Some`）。
    pub zone_boundary: Option<f32>,
    /// 定位线顶边（板内底沿）。
    pub line_y: f32,
    pub line_height: f32,
    /// 条目总宽（含入口格）超出可用宽：已按区开视口 + 计数格。
    pub overflow: bool,
    /// 回显输入方向；`Rtl` 目前与 `Ltr` 同几何（见 `DockDirection` 的 TODO）。
    pub direction: DockDirection,
    /// 输入 `hover` 落在哪个目标上（计数格优先，其次入口格，再次槽位）。
    pub hovered: Option<DockHit>,
}

impl DockLayout {
    /// 命中查询（与 `hovered` 同一套规则）；纯只读，供事件层复用。
    pub fn hit(&self, pos: Point) -> Option<DockHit> {
        for zone in &self.zones {
            for chip in &zone.chips {
                if chip.rect.contains(pos) {
                    return Some(DockHit::Chip {
                        zone: chip.zone,
                        direction: chip.direction,
                    });
                }
            }
        }
        if self.entry.is_some_and(|r| r.contains(pos)) {
            return Some(DockHit::Entry);
        }
        self.slots
            .iter()
            .find(|s| s.hit.is_some_and(|r| r.contains(pos)))
            .map(|s| DockHit::Slot(s.display))
    }

    /// 按呈现序号取槽位。
    pub fn slot(&self, display: usize) -> Option<&DockSlotLayout> {
        self.slots.get(display)
    }

    /// 指针落在哪个区（滚轮分流用；计数格也算本区，因为它在本区矩形内）。
    pub fn zone_at(&self, pos: Point) -> Option<DockZone> {
        self.zones
            .iter()
            .find(|z| z.rect.contains(pos))
            .map(|z| z.zone)
    }
}

/// 只按水平范围裁剪（纵向整列保留）：命中区要含槽下方的悬浮间隙（§158）。
fn clip_x(r: Rect, clip: Rect) -> Option<Rect> {
    let x0 = r.origin.x.max(clip.origin.x);
    let x1 = r.right().min(clip.right());
    (x1 - x0 > 0.01).then(|| Rect::new(x0, r.origin.y, x1 - x0, r.size.height))
}

/// Dock 轨道几何的纯计算入口。
///
/// 用法：`let layout = DockRail::layout(&input);` 然后绘制层读 `layout.slots` /
/// `layout.plate` / `layout.zones`，事件层调 `layout.hit(pos)`。同一份输入必得同一份输出。
pub struct DockRail;

impl DockRail {
    /// 一次稳态布局。**不做**过渡插值：launcher 的 0.2 s 换位 / 底板伸缩 / 滚动动画是
    /// 调用方按 `sokuou` 进度在两次快照之间插值，控件只给落定几何。
    pub fn layout(input: &DockRailInput) -> DockLayout {
        let m = DockMetrics::scaled(input.scale);
        let dock = input.available.normalized();
        let top = dock.origin.y;
        let plate_h = m.plate_height.min(dock.size.height);
        let line_y = top + plate_h - m.line_height;

        // 分区：左区（钉选未运行）→ 右区（钉选运行在前、其余运行按出现顺序）。
        // 两边都不是的条目不上轨道（未钉选也未运行）。
        let mut order: Vec<usize> = Vec::with_capacity(input.entries.len());
        for (i, e) in input.entries.iter().enumerate() {
            if e.pinned && !e.running {
                order.push(i);
            }
        }
        let split = order.len();
        for (i, e) in input.entries.iter().enumerate() {
            if e.pinned && e.running {
                order.push(i);
            }
        }
        for (i, e) in input.entries.iter().enumerate() {
            if !e.pinned && e.running {
                order.push(i);
            }
        }
        let n = order.len();
        let entry_w = if input.show_entry { m.slot_width } else { 0.0 };

        // 溢出判定（§186）：总槽宽（含入口格）超出可用宽 = 输出逻辑宽 − 2 × 72。
        // 溢出时每区各自视口：左区最多三分之一（至少 2），其余归右区；被挡的一侧留计数格。
        let avail = (dock.size.width - 2.0 * m.side_margin).max(m.slot_width);
        let overflow = entry_w + n as f32 * m.slot_width > avail + 0.5;
        let fit =
            |chips: f32| (((avail - entry_w - chips) / m.slot_width).floor()).max(0.0) as usize;
        let (vis_l, vis_r) = if overflow {
            let fit2 = fit(2.0 * m.chip_width);
            let vl = split.min((fit2 / 3).max(2));
            let vr = (n - split).min(fit2.saturating_sub(vl));
            let (vl, vr) = (split.min(fit2.saturating_sub(vr)), vr);
            // 两区都会被挡时前后各有一枚计数格（共四枚）：按四枚重算一次，
            // 否则「滚到中间」时底板会比可用宽还宽（与 launcher 同）。
            if split > vl && n - split > vr {
                let fit4 = fit(4.0 * m.chip_width);
                let l = split.min((fit4 / 3).max(2));
                let r = (n - split).min(fit4.saturating_sub(l));
                (split.min(fit4.saturating_sub(r)), r)
            } else {
                (vl, vr)
            }
        } else {
            (split, n - split)
        };
        let vis = [vis_l, vis_r];

        // 滚动位按新视口夹紧（整槽）；`before` 是夹紧后的值，回传到快照里。
        let before = [
            input.scroll[0].min(split.saturating_sub(vis_l)),
            input.scroll[1].min((n - split).saturating_sub(vis_r)),
        ];
        let after = [
            split.saturating_sub(vis_l + before[0]),
            (n - split).saturating_sub(vis_r + before[1]),
        ];

        let zone_w = |z: usize| {
            vis[z] as f32 * m.slot_width
                + if before[z] > 0 { m.chip_width } else { 0.0 }
                + if after[z] > 0 { m.chip_width } else { 0.0 }
        };
        let group_w = entry_w + zone_w(0) + zone_w(1);
        let plate = (group_w > 0.0).then(|| {
            let w = group_w + 2.0 * m.plate_pad;
            Rect::new(dock.origin.x + (dock.size.width - w) / 2.0, top, w, plate_h)
        });
        // 底板居中的同一条式子；无底板时只用来给空布局一个稳定起点。
        let plate_left = dock.origin.x + (dock.size.width - (group_w + 2.0 * m.plate_pad)) / 2.0;
        let mut x = plate_left + m.plate_pad;

        // 左端入口格（§187）：固定一格、不属任何区、下面不画定位线。
        let entry = if input.show_entry {
            let r = Rect::new(x, top, m.slot_width, plate_h);
            x += m.slot_width;
            Some(r)
        } else {
            None
        };

        let mut zones = [
            DockZoneLayout {
                zone: DockZone::Pinned,
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                first_display: 0,
                visible: vis[0],
                before: before[0],
                after: after[0],
                chips: Vec::new(),
            },
            DockZoneLayout {
                zone: DockZone::Running,
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                first_display: split,
                visible: vis[1],
                before: before[1],
                after: after[1],
                chips: Vec::new(),
            },
        ];
        for z in 0..2 {
            let w = zone_w(z);
            zones[z].rect = Rect::new(x, top, w, plate_h);
            if before[z] > 0 {
                zones[z].chips.push(DockChipLayout {
                    zone: zones[z].zone,
                    direction: DockChipDirection::Prev,
                    rect: Rect::new(x, top, m.chip_width, plate_h),
                    line: Rect::new(x, line_y, m.chip_width, m.line_height),
                    count: before[z],
                });
            }
            if after[z] > 0 {
                let cx = x + w - m.chip_width;
                zones[z].chips.push(DockChipLayout {
                    zone: zones[z].zone,
                    direction: DockChipDirection::Next,
                    rect: Rect::new(cx, top, m.chip_width, plate_h),
                    line: Rect::new(cx, line_y, m.chip_width, m.line_height),
                    count: after[z],
                });
            }
            x += w;
        }
        let zone_boundary = (split > 0 && split < n).then(|| zones[1].rect.origin.x);

        // 槽位矩形（呈现顺序）。溢出时命中按区视口水平裁剪，纵向保留悬浮间隙。
        let mut slots = Vec::with_capacity(n);
        for (display, &source) in order.iter().enumerate() {
            let e = &input.entries[source];
            let z = usize::from(display >= split);
            let local = if z == 0 { display } else { display - split };
            let zr = zones[z].rect;
            let zx = zr.origin.x + if before[z] > 0 { m.chip_width } else { 0.0 };
            let sx = zx + (local as f32 - before[z] as f32) * m.slot_width;
            let rect = Rect::new(sx, top, m.slot_width, dock.size.height);
            let vp = Rect::new(zx, top, vis[z] as f32 * m.slot_width, zr.size.height);
            let hit = if overflow {
                clip_x(rect, vp)
            } else {
                Some(rect)
            };
            let visible = local >= before[z] && local < before[z] + vis[z];
            let icon_rect = Rect::new(
                rect.origin.x + (m.slot_width - m.icon) / 2.0,
                top + ((plate_h - m.icon) / 2.0).max(0.0),
                m.icon,
                m.icon,
            );
            let indicator_kind = if z == 0 {
                DockIndicatorKind::Pinned
            } else if e.activated {
                DockIndicatorKind::Focused
            } else {
                DockIndicatorKind::Running
            };
            slots.push(DockSlotLayout {
                source,
                display,
                zone: zones[z].zone,
                id: e.id.clone(),
                icon: e.icon.clone(),
                rect,
                hit,
                icon_rect,
                visible,
                indicator: None,
                indicator_kind,
            });
        }

        // 定位线段：不溢出时整排按当帧 x 排序，「到下一段」定宽（末段 = 槽宽）；
        // 溢出时逐区拼「计数格 + 可见槽」，格与槽都按本区视口裁。
        if !overflow {
            let mut cells: Vec<(f32, f32, Option<usize>)> = slots
                .iter()
                .map(|s| (s.rect.origin.x, m.slot_width, Some(s.display)))
                .collect();
            cells.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            for k in 0..cells.len() {
                let (cx, nominal, display) = cells[k];
                let w = if k + 1 < cells.len() {
                    cells[k + 1].0 - cx
                } else {
                    nominal
                };
                if let Some(i) = display {
                    slots[i].indicator = Some(Rect::new(cx, line_y, w.max(0.0), m.line_height));
                }
            }
        } else {
            for z in 0..2 {
                if zones[z].rect.size.width <= 0.0 {
                    continue;
                }
                // (x, nominal 宽, 槽位呈现序号；计数格为 None)
                let mut cells: Vec<(f32, f32, Option<usize>)> = zones[z]
                    .chips
                    .iter()
                    .map(|c| (c.rect.origin.x, m.chip_width, None))
                    .collect();
                for s in &slots {
                    if s.zone != zones[z].zone || !s.visible {
                        continue;
                    }
                    let local = if z == 0 { s.display } else { s.display - split };
                    if local >= before[z] && local < before[z] + vis[z] {
                        cells.push((s.rect.origin.x, m.slot_width, Some(s.display)));
                    }
                }
                cells.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
                for k in 0..cells.len() {
                    let (cx, nominal, display) = cells[k];
                    let w = if k + 1 < cells.len() {
                        cells[k + 1].0 - cx
                    } else {
                        nominal
                    };
                    if let Some(i) = display {
                        slots[i].indicator = Some(Rect::new(cx, line_y, w.max(0.0), m.line_height));
                    }
                }
            }
        }

        let mut layout = DockLayout {
            metrics: m,
            dock,
            plate,
            entry,
            zones,
            slots,
            split,
            zone_boundary,
            line_y,
            line_height: m.line_height,
            overflow,
            direction: input.direction,
            hovered: None,
        };
        layout.hovered = input.hover.and_then(|p| layout.hit(p));
        layout
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: f32) -> Rect {
        Rect::new(0.0, 0.0, width, DOCK_SURFACE_HEIGHT)
    }

    fn entry(id: &str, pinned: bool, running: bool, activated: bool) -> DockRailEntry {
        DockRailEntry::new(id, "icon")
            .pinned(pinned)
            .running(running)
            .activated(activated)
    }

    /// 与 launcher 测试的 `many(left, right)` 同构：先左侧钉选、再右侧运行。
    fn many(left: usize, right: usize) -> Vec<DockRailEntry> {
        let mut out = Vec::new();
        for i in 0..left {
            out.push(entry(&format!("p{i}"), true, false, false));
        }
        for i in 0..right {
            out.push(entry(&format!("r{i}"), false, true, false));
        }
        out
    }

    fn layout(width: f32, entries: Vec<DockRailEntry>, show_entry: bool) -> DockLayout {
        let mut input = DockRailInput::new(entries, area(width));
        input.show_entry = show_entry;
        DockRail::layout(&input)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    // ── 0 / 1 条 ─────────────────────────────────────────────────────────

    /// 空轨道、无入口格：没有底板、没有槽、没有线（launcher `empty_dock_draws_nothing`）。
    #[test]
    fn empty_dock_without_entry_has_no_plate() {
        let l = layout(1280.0, Vec::new(), false);
        assert!(l.plate.is_none(), "空 Dock 无底板");
        assert!(l.entry.is_none());
        assert!(l.slots.is_empty());
        assert!(!l.overflow);
        assert_eq!(l.zones[0].rect.size.width, 0.0);
        assert_eq!(l.zones[1].rect.size.width, 0.0);
        assert_eq!(l.zone_boundary, None);
        assert_eq!(l.line_height, DOCK_LINE_HEIGHT);
    }

    /// 空轨道 + 入口格：底板只包入口格（launcher `empty_dock_keeps_entry_cell`）。
    #[test]
    fn empty_dock_keeps_entry_cell_and_plate() {
        let l = layout(1280.0, Vec::new(), true);
        let plate = l.plate.expect("入口格应撑出底板");
        assert!(close(
            plate.size.width,
            DOCK_SLOT_WIDTH + 2.0 * DOCK_PLATE_PAD
        ));
        assert!(close(plate.origin.x, (1280.0 - plate.size.width) / 2.0));
        let e = l.entry.expect("空 Dock 仍有入口格");
        assert!(close(e.size.width, DOCK_SLOT_WIDTH));
        assert!(close(e.origin.x, plate.origin.x + DOCK_PLATE_PAD));
        assert!(l.slots.is_empty(), "入口格不是槽位");
        assert_eq!(l.hit(e.center()), Some(DockHit::Entry));
    }

    /// 一条槽：底板 = 槽宽 + 两侧内边距，水平居中；无计数格、无分界。
    #[test]
    fn single_slot_is_centered_with_padding() {
        let l = layout(1280.0, vec![entry("a", true, false, false)], false);
        assert_eq!(l.slots.len(), 1);
        let s = &l.slots[0];
        assert!(close(s.rect.origin.x, 610.0), "槽 x = 640 - 30");
        assert!(close(s.rect.size.width, DOCK_SLOT_WIDTH));
        assert!(
            close(s.rect.size.height, DOCK_SURFACE_HEIGHT),
            "命中含悬浮间隙"
        );
        assert_eq!(s.hit, Some(s.rect));
        let plate = l.plate.expect("一槽应有底板");
        assert_eq!(plate, Rect::new(598.0, 0.0, 84.0, 68.0));
        assert_eq!(l.zone_boundary, None, "只有左区，无分界");
        assert!(l.zones[1].chips.is_empty());
    }

    // ── 悬浮几何（抄 launcher `float_geometry_icon_line_and_bottom_hit`）──

    #[test]
    fn float_geometry_icon_line_and_bottom_hit() {
        let l = layout(1280.0, many(2, 2), false);
        let s = &l.slots[2];
        assert!(
            close(s.icon_rect.origin.y, 10.0),
            "图标距板顶 10：{:?}",
            s.icon_rect
        );
        assert!(close(
            s.icon_rect.origin.y + DOCK_ICON_SIZE + 10.0,
            DOCK_PLATE_HEIGHT
        ));
        assert!(
            close(s.icon_rect.origin.x, s.rect.origin.x + 6.0),
            "图标在槽内水平居中"
        );
        assert!(
            close(l.line_y, DOCK_PLATE_HEIGHT - DOCK_LINE_HEIGHT),
            "定位线贴板底"
        );
        // 指针贴屏底（悬浮间隙内）仍命中该槽（§158）。
        let bottom = Point::new(s.rect.center().x, DOCK_SURFACE_HEIGHT - 1.0);
        assert_eq!(l.hit(bottom), Some(DockHit::Slot(2)));
    }

    /// 底板包住整组图标 + 左右各 12（launcher `b_case_plate_fills_group_plus_padding`）。
    #[test]
    fn plate_fills_group_plus_padding() {
        let l = layout(1280.0, many(2, 2), false);
        let plate = l.plate.expect("应有底板");
        assert!(close(
            plate.size.width,
            4.0 * DOCK_SLOT_WIDTH + 2.0 * DOCK_PLATE_PAD
        ));
        assert!(close(plate.size.height, DOCK_PLATE_HEIGHT));
        assert_eq!(plate.origin.y, 0.0);
        assert!(close(plate.origin.x, (1280.0 - plate.size.width) / 2.0));
        assert!(close(
            l.zones[1].rect.right(),
            plate.right() - DOCK_PLATE_PAD
        ));
    }

    // ── 溢出 / 计数格 ────────────────────────────────────────────────────

    #[test]
    fn no_overflow_has_no_chips_and_all_visible() {
        let l = layout(1280.0, many(2, 2), false);
        assert!(!l.overflow);
        assert_eq!(l.zones[0].visible, 2);
        assert_eq!(l.zones[1].visible, 2);
        assert!(
            l.zones.iter().all(|z| z.chips.is_empty()),
            "不溢出时不画计数格"
        );
        assert!(l.slots.iter().all(|s| s.visible && s.hit.is_some()));
        assert_eq!(l.zone_boundary, Some(l.zones[1].rect.origin.x));
        assert!(
            close(l.zones[0].rect.right(), l.zones[1].rect.origin.x),
            "两区相接无缝"
        );
    }

    /// 溢出：左区可见 ≤ ⌊可用槽数/3⌋、两侧计数格数字正确（launcher
    /// `overflow_limits_left_zone_and_draws_count_chips`）。
    #[test]
    fn overflow_limits_left_zone_and_draws_count_chips() {
        let l = layout(1280.0, many(15, 15), true);
        // 可用宽 1280 − 144 = 1136；入口 60、四枚计数格 36 → 15 槽。
        assert!(l.overflow);
        assert_eq!(l.zones[0].visible, 5, "左区 = ⌊15/3⌋");
        assert_eq!(l.zones[1].visible, 10, "其余归右区");
        assert_eq!(l.zones[0].chips.len(), 1);
        assert_eq!(l.zones[1].chips.len(), 1);
        let c0 = l.zones[0].chips[0];
        let c1 = l.zones[1].chips[0];
        assert_eq!((c0.direction, c0.count), (DockChipDirection::Next, 10));
        assert_eq!((c1.direction, c1.count), (DockChipDirection::Next, 5));
        assert!(close(c0.rect.origin.x, 484.0));
        assert!(close(c1.rect.origin.x, 1120.0));
        assert_eq!(l.zones[0].rect, Rect::new(184.0, 0.0, 336.0, 68.0));
        assert_eq!(l.zones[1].rect, Rect::new(520.0, 0.0, 636.0, 68.0));
        assert_eq!(l.plate, Some(Rect::new(112.0, 0.0, 1056.0, 68.0)));
        // 计数格命中 → 翻本区一页。
        assert_eq!(
            l.hit(c0.rect.center()),
            Some(DockHit::Chip {
                zone: DockZone::Pinned,
                direction: DockChipDirection::Next
            })
        );
        // 计数格下的线画本区基色（几何整格宽）。
        assert!(close(c0.line.size.width, DOCK_CHIP_WIDTH));
        assert!(close(c0.line.origin.y, l.line_y));
        assert_eq!(l.zone_at(c0.rect.center()), Some(DockZone::Pinned));
        assert_eq!(l.zone_at(c1.rect.center()), Some(DockZone::Running));
    }

    /// 只有右区有槽时溢出：左区视口为 0、计数格全归右区（launcher
    /// `overflow_with_empty_left_zone_keeps_right_viewport`）。
    #[test]
    fn overflow_with_empty_left_zone_keeps_right_viewport() {
        let l = layout(1280.0, many(0, 30), true);
        assert_eq!(l.zones[0].visible, 0, "左区无槽");
        assert!(l.zones[1].visible >= 2, "右区至少 2 槽");
        assert_eq!(l.zones[0].rect.size.width, 0.0, "空左区不占宽");
        assert!(l.zones[0].chips.is_empty());
        assert_eq!(l.zones[1].chips.len(), 1);
        assert_eq!(l.zones[1].chips[0].zone, DockZone::Running);
        // 无槽的条目被完全挡住的也留在快照里（供外壳回查），但不可见、不可点。
        assert_eq!(l.slots.len(), 30);
        let vis = l.zones[1].visible;
        assert!(
            l.slots
                .iter()
                .skip(vis)
                .all(|s| !s.visible && s.hit.is_none())
        );
    }

    // ── 双区切换 ─────────────────────────────────────────────────────────

    /// 钉选项开始运行 → 从「左区钉选未运行」搬到右区（钉选运行排在其余运行之前）。
    #[test]
    fn zone_switch_moves_pinned_entry_between_zones() {
        let mut entries = vec![
            entry("a", true, false, false),
            entry("b", true, true, false),
            entry("c", false, true, false),
        ];
        let l = layout(1280.0, entries.clone(), false);
        assert_eq!(l.split, 1, "只有 a 在左区");
        assert_eq!(
            l.slots.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert_eq!(
            (l.slots[0].zone, l.slots[1].zone),
            (DockZone::Pinned, DockZone::Running)
        );
        assert_eq!(l.zone_boundary, Some(l.zones[1].rect.origin.x));
        assert_eq!(l.slot(2).map(|s| s.source), Some(2), "呈现序号 → 输入下标");

        // a 开始运行：左区清空，a 作为「钉选运行」排到右区最前。
        entries[0].running = true;
        let l = layout(1280.0, entries.clone(), false);
        assert_eq!(l.split, 0);
        assert_eq!(l.zone_boundary, None, "左区空 → 无分界");
        assert_eq!(
            l.slots.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["a", "b", "c"]
        );
        assert!(l.slots.iter().all(|s| s.zone == DockZone::Running));

        // a 取消钉选又不在运行：不上轨道（与 launcher 模型同）。
        entries[0].pinned = false;
        entries[0].running = false;
        let l = layout(1280.0, entries, false);
        assert_eq!(
            l.slots.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
            ["b", "c"]
        );
    }

    // ── 命中边缘 / 悬停 ──────────────────────────────────────────────────

    #[test]
    fn hover_at_slot_edges() {
        let l = layout(1280.0, vec![entry("a", true, false, false)], false);
        let s = l.slots[0].rect;
        assert_eq!(
            l.hit(Point::new(s.origin.x, s.origin.y)),
            Some(DockHit::Slot(0))
        );
        assert_eq!(
            l.hit(Point::new(s.right() - 0.01, s.bottom() - 0.01)),
            Some(DockHit::Slot(0)),
            "右下角内侧仍命中"
        );
        assert_eq!(
            l.hit(Point::new(s.right(), s.center().y)),
            None,
            "右缘是开区间"
        );
        assert_eq!(l.hit(Point::new(s.origin.x - 0.01, s.center().y)), None);
        assert_eq!(l.hit(Point::new(s.center().x, s.bottom())), None);
        // 悬浮间隙属于槽（§158）。
        assert_eq!(
            l.hit(Point::new(s.center().x, s.bottom() - 1.0)),
            Some(DockHit::Slot(0))
        );

        let mut input = DockRailInput::new(vec![entry("a", true, false, false)], area(1280.0));
        input.hover = Some(Point::new(s.center().x, 10.0));
        let l = DockRail::layout(&input);
        assert_eq!(l.hovered, Some(DockHit::Slot(0)), "hover 快照与 hit 同规则");
    }

    // ── 指示点跟随钉选 / 运行 / 激活 ─────────────────────────────────────

    #[test]
    fn indicator_follows_pin_run_and_focus() {
        let entries = vec![
            entry("a", true, false, true),
            entry("b", true, true, true),
            entry("c", false, true, false),
        ];
        let l = layout(1280.0, entries, false);
        assert_eq!(l.slots[0].indicator_kind, DockIndicatorKind::Pinned);
        assert_eq!(l.slots[0].indicator_kind.token(), ThemeColor::Divider);
        // 钉选 + 运行 + 激活 = 右区焦点亮档（不是左区灰）。
        assert_eq!(l.slots[1].indicator_kind, DockIndicatorKind::Focused);
        assert_eq!(l.slots[1].indicator_kind.token(), ThemeColor::PrimaryHover);
        assert_eq!(l.slots[2].indicator_kind, DockIndicatorKind::Running);
        assert_eq!(l.slots[2].indicator_kind.token(), ThemeColor::Primary);
        assert!(l.slots.iter().all(|s| s.indicator.is_some()), "每槽一段线");
    }

    /// 定位线段首尾相接、覆盖整组（launcher `non_overflow_reorder_keeps_line_seamless`
    /// 的落定态）。
    #[test]
    fn indicator_segments_are_seamless() {
        let l = layout(1280.0, many(3, 2), false);
        let mut segs: Vec<Rect> = l.slots.iter().filter_map(|s| s.indicator).collect();
        segs.sort_by(|a, b| a.origin.x.partial_cmp(&b.origin.x).unwrap());
        let plate = l.plate.unwrap();
        assert!(close(segs[0].origin.x, plate.origin.x + DOCK_PLATE_PAD));
        for pair in segs.windows(2) {
            assert!(
                close(pair[0].right(), pair[1].origin.x),
                "定位线留缝：{pair:?}"
            );
        }
        assert!(close(
            segs.last().unwrap().right(),
            plate.right() - DOCK_PLATE_PAD
        ));
        assert!(segs.iter().all(|r| close(r.size.height, DOCK_LINE_HEIGHT)));
    }

    /// 入口格不属任何区、下面不画线（launcher `entry_cell_has_no_line_and_emits_toggle`）。
    #[test]
    fn entry_cell_has_no_line() {
        let l = layout(1280.0, many(2, 1), true);
        let e = l.entry.expect("应有入口格");
        assert!(close(e.size.width, DOCK_SLOT_WIDTH));
        assert!(close(
            e.origin.x + DOCK_SLOT_WIDTH,
            l.slots[0].rect.origin.x
        ));
        assert!(
            l.slots
                .iter()
                .filter_map(|s| s.indicator)
                .all(|r| r.origin.x >= e.right() - 0.01),
            "入口格下面不画定位线"
        );
        assert_eq!(l.hit(e.center()), Some(DockHit::Entry));
    }

    // ── 溢出滚动输入 ────────────────────────────────────────────────────

    /// 滚动位是结构化输入：夹到本区上限，滚出视口的槽不可见 / 不可点。
    #[test]
    fn scroll_clamps_and_hides_leading_slots() {
        let mut input = DockRailInput::new(many(15, 15), area(1280.0));
        input.show_entry = true;
        input.scroll = [0, 1];
        let l = DockRail::layout(&input);
        assert_eq!(l.zones[1].before, 1);
        assert_eq!(l.zones[1].after, 4);
        let zx = l.zones[1].rect.origin.x;
        assert_eq!(l.zones[1].chips.len(), 2, "前后各一枚计数格");
        assert_eq!(l.zones[1].chips[0].direction, DockChipDirection::Prev);
        assert!(close(l.zones[1].chips[0].rect.origin.x, zx));
        assert_eq!(l.zones[1].chips[1].count, 4);
        let first = l.zones[1].first_display;
        assert_eq!(first, 15);
        assert!(
            !l.slots[first].visible && l.slots[first].hit.is_none(),
            "滚出的槽不可点"
        );
        let hit = l.slots[first + 1].hit.expect("第二槽顶到视口起点");
        assert!(close(hit.origin.x, zx + DOCK_CHIP_WIDTH));

        // 超限输入被夹紧，回传到快照。
        input.scroll = [99, 99];
        let l = DockRail::layout(&input);
        assert_eq!((l.zones[0].before, l.zones[1].before), (10, 5));
        assert_eq!((l.zones[0].after, l.zones[1].after), (0, 0));
    }

    // ── 缩放 1× / 1.5× / 2× ──────────────────────────────────────────────

    #[test]
    fn scale_multiplies_all_lengths() {
        for f in [1.0_f32, 1.5, 2.0] {
            let mut input = DockRailInput::new(
                many(2, 2),
                Rect::new(0.0, 0.0, 1280.0 * f, DOCK_SURFACE_HEIGHT * f),
            );
            input.scale = f;
            let l = DockRail::layout(&input);
            let m = l.metrics;
            assert!(close(m.slot_width, DOCK_SLOT_WIDTH * f));
            assert!(close(m.icon, DOCK_ICON_SIZE * f));
            assert!(close(m.plate_height, DOCK_PLATE_HEIGHT * f));
            assert!(close(m.line_height, DOCK_LINE_HEIGHT * f));
            assert!(close(m.plate_pad, DOCK_PLATE_PAD * f));
            assert!(close(m.dock_height(), DOCK_SURFACE_HEIGHT * f));
            let plate = l.plate.unwrap();
            assert!(close(plate.size.width, 264.0 * f), "{f}× 底板宽");
            assert!(close(plate.origin.x, 508.0 * f), "{f}× 底板居中");
            assert!(close(l.slots[0].rect.origin.x, 520.0 * f), "{f}× 首槽 x");
            assert!(
                close(l.slots[0].icon_rect.origin.y, 10.0 * f),
                "{f}× 图标 y"
            );
            assert!(close(l.line_y, 65.0 * f), "{f}× 定位线 y");
            assert!(close(l.slots[3].indicator.unwrap().size.width, 60.0 * f));
            // 命中区同步缩放：屏底内 1 pt 仍落在槽上。
            let p = Point::new(l.slots[0].rect.center().x, l.dock.size.height - 1.0);
            assert_eq!(l.hit(p), Some(DockHit::Slot(0)));
        }
        assert_eq!(
            DockMetrics::scaled(0.0),
            DockMetrics::BASE,
            "非法系数退回 1×"
        );
        assert_eq!(DockMetrics::scaled(f32::NAN), DockMetrics::BASE);
    }

    // ── RTL ─────────────────────────────────────────────────────────────

    /// RTL 尚未实现：方向被回显，但几何与 LTR 相同。
    ///
    /// TODO(n1111-dock-rail): 镜像排布（入口格在右、槽序从右往左、计数格方向取反）需要
    /// 先定图标是否镜像；launcher 现状无 RTL 路径，此处不凭空实现，只把缺口钉成用例。
    #[test]
    fn rtl_is_echoed_but_geometry_is_ltr() {
        let entries = many(2, 2);
        let mut ltr_input = DockRailInput::new(entries.clone(), area(1280.0));
        ltr_input.direction = DockDirection::Ltr;
        let ltr = DockRail::layout(&ltr_input);
        let mut rtl_input = DockRailInput::new(entries, area(1280.0));
        rtl_input.direction = DockDirection::Rtl;
        let rtl = DockRail::layout(&rtl_input);

        assert_eq!(rtl.direction, DockDirection::Rtl);
        assert_eq!(rtl.plate, ltr.plate);
        assert_eq!(rtl.entry, ltr.entry);
        assert_eq!(rtl.zones, ltr.zones);
        assert_eq!(rtl.slots, ltr.slots);
        assert_eq!(rtl.zone_boundary, ltr.zone_boundary);
    }
}
