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

use kanesumi_core::{Point, Rect, ThemeColor, sizes};

/// 图标边长（§158：48 为 200% 输出的最终值）。
pub const DOCK_ICON_SIZE: f32 = sizes::DOCK_ICON;
/// 槽宽（图标居中，槽间无缝）。
pub const DOCK_SLOT_WIDTH: f32 = sizes::DOCK_SLOT;
/// 板高 = 图标 + 板内上下各 10。
pub const DOCK_PLATE_HEIGHT: f32 = sizes::DOCK_PLATE_HEIGHT;
/// 悬浮：板底距屏底 8；这 8 pt 仍属 Dock 表面，命中延伸到屏幕底边（§158）。
pub const DOCK_FLOAT_GAP: f32 = sizes::DOCK_FLOAT_GAP;
/// 悬浮档的 Dock 主表面高（板 + 间隙），与合成器 `dock_visibility::DOCK_HEIGHT` 一致。
pub const DOCK_SURFACE_HEIGHT: f32 = DOCK_PLATE_HEIGHT + DOCK_FLOAT_GAP;
/// 定位线高（每槽一段、贴板内底沿）。
pub const DOCK_LINE_HEIGHT: f32 = sizes::DOCK_LINE;
/// 底板左右内边距（居中直角底板只包图标区）。
pub const DOCK_PLATE_PAD: f32 = sizes::DOCK_PLATE_PAD;
/// 溢出判定：可用宽 = 输出逻辑宽 − 2 × 72（§186）。
pub const DOCK_SIDE_MARGIN: f32 = sizes::DOCK_SIDE_MARGIN;
/// 计数格宽（§186：「‹ N」/「N ›」）。
pub const DOCK_CHIP_WIDTH: f32 = sizes::DOCK_CHIP_WIDTH;

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

/// 呈现输入：两帧之间的**中间态**。旧 launcher 的 `disp_x` / `start_off`（逐槽位移）、
/// `plate_shown`（板宽插值）、`scroll_pos`（分数滚动）与 `line_from`（线色插值）都收敛到这里。
///
/// 所有字段都是**可选覆盖**：`None` / 空表 = 用落定几何。只有 `MetroDock::set_presentation`
/// 显式喂入时才参与新绘制入口；旧的 `MetroDock::render` 始终走落定路径，逐像素不变。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DockPresentation {
    /// 完整板宽（含左右内边距，逻辑 pt）。`None` = 落定板宽；`0` = 收成无板。
    pub plate_width: Option<f32>,
    /// 板顶 y。`None` = 落定板顶。
    pub plate_top: Option<f32>,
    /// 板高。`None` = 落定板高（并夹到轨道高）。
    pub plate_height: Option<f32>,
    /// 逐槽水平位移（正值向右）。键先按稳定 id 查，再按 `#<source>` 查；两处都命中则相加。
    /// 覆盖让位（+一格）与换位过渡的起点偏移；轻跳是纵向，由控件按启动相位产出。
    pub slot_offsets: Vec<(String, f32)>,
    /// 逐区滚动位置（单位 = 槽，可含小数）。整数落定值来自 `DockRailInput.scroll`，
    /// 这里给的是动画中的**当前**位置；`None` = 取落定值。
    pub scroll: Option<[f32; 2]>,
    /// 逐槽定位线颜色进度：`0` = `Divider`、`0.5` = `Primary`、`1` = `PrimaryHover`，
    /// 两段内线性插值。键规则同 `slot_offsets`；缺省按槽的 `indicator_kind` 取端点。
    pub line_progress: Vec<(String, f32)>,
}

impl DockPresentation {
    /// 只改板宽（板顶 / 高沿用落定值）。
    pub fn with_plate_width(mut self, width: f32) -> Self {
        self.plate_width = Some(width);
        self
    }

    /// 板宽 / 板顶 / 板高一次给全。
    pub fn with_plate(mut self, width: f32, top: f32, height: f32) -> Self {
        self.plate_width = Some(width);
        self.plate_top = Some(top);
        self.plate_height = Some(height);
        self
    }

    /// 逐区当前滚动位置（可含小数槽）。
    pub fn with_scroll(mut self, scroll: [f32; 2]) -> Self {
        self.scroll = Some(scroll);
        self
    }

    /// 逐槽水平位移。
    pub fn with_slot_offset(mut self, id: impl Into<String>, dx: f32) -> Self {
        self.slot_offsets.push((id.into(), dx));
        self
    }

    /// 按模型下标给逐槽水平位移（内部键 = `#<source>`）。
    pub fn with_source_offset(mut self, source: usize, dx: f32) -> Self {
        self.slot_offsets.push((format!("#{source}"), dx));
        self
    }

    /// 逐槽定位线颜色进度（0 / 0.5 / 1 = Divider / Primary / PrimaryHover）。
    pub fn with_line_progress(mut self, id: impl Into<String>, progress: f32) -> Self {
        self.line_progress.push((id.into(), progress));
        self
    }

    /// 按模型下标给线色进度（内部键 = `#<source>`）。
    pub fn with_source_line_progress(mut self, source: usize, progress: f32) -> Self {
        self.line_progress.push((format!("#{source}"), progress));
        self
    }
}

/// 呈现后的一个槽：落定槽位 + 板宽插值 / 分数滚动 / 逐槽位移后的矩形。
#[derive(Debug, Clone, PartialEq)]
pub struct DockPresentedSlot {
    pub source: usize,
    pub display: usize,
    pub zone: DockZone,
    pub id: String,
    pub icon: String,
    /// 整槽矩形（宽 = 槽宽，高 = 轨道高，含下方悬浮间隙）。
    pub rect: Rect,
    /// 呈现后的可点区域；溢出时按区视口裁剪，完全挡住 = `None`。
    pub hit: Option<Rect>,
    pub icon_rect: Rect,
    pub visible: bool,
    /// 呈现后的定位线段；不可见 / 完全滚出视口 = `None`。宽度只到本槽，便于按槽记损伤。
    pub indicator: Option<Rect>,
    pub indicator_kind: DockIndicatorKind,
    /// 本槽线的颜色进度（0 / 0.5 / 1 = Divider / Primary / PrimaryHover）。
    pub line_progress: f32,
    /// 本槽实际生效的水平位移（含 id / source 两处命中之和）。
    pub dx: f32,
}

/// 呈现后的一个计数格。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockPresentedChip {
    pub zone: DockZone,
    pub direction: DockChipDirection,
    pub count: usize,
    pub rect: Rect,
    pub line: Rect,
}

/// 呈现后的一个区。
#[derive(Debug, Clone, PartialEq)]
pub struct DockPresentedZone {
    pub zone: DockZone,
    pub rect: Rect,
    /// 槽位视口（计数格之外的可见槽区）。
    pub viewport: Rect,
    pub before: usize,
    pub after: usize,
    pub visible: usize,
    pub chips: Vec<DockPresentedChip>,
}

/// 一次呈现的完整快照。几何真值仍以 `DockLayout` 为落定来源，本结构只描述**当前帧**。
#[derive(Debug, Clone, PartialEq)]
pub struct DockPresentedLayout {
    pub metrics: DockMetrics,
    pub dock: Rect,
    pub plate: Option<Rect>,
    pub entry: Option<Rect>,
    pub zones: [DockPresentedZone; 2],
    pub slots: Vec<DockPresentedSlot>,
    pub line_y: f32,
    pub line_height: f32,
    pub overflow: bool,
}

impl DockPresentedLayout {
    /// 呈现几何上的命中（计数格 → 入口格 → 槽位），与 `DockLayout::hit` 同序。
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
    pub fn slot(&self, display: usize) -> Option<&DockPresentedSlot> {
        self.slots.get(display)
    }
}

/// 落定语义档 → 三档线色进度的自然端点。
fn natural_line_progress(kind: DockIndicatorKind) -> f32 {
    match kind {
        DockIndicatorKind::Pinned => 0.0,
        DockIndicatorKind::Running => 0.5,
        DockIndicatorKind::Focused => 1.0,
    }
}

/// 逐槽键查询：先按 id，再按 `#<source>`；非有限值忽略。
fn keyed_value(table: &[(String, f32)], id: &str, source: usize) -> Option<f32> {
    let source_key = format!("#{source}");
    table
        .iter()
        .filter(|(key, _)| key == id || key == &source_key)
        .map(|(_, value)| *value)
        .filter(|value| value.is_finite())
        .reduce(|a, b| a + b)
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

    /// 在落定布局上套用呈现输入。旧 `layout` 的签名与行为不变；本函数只读它。
    pub fn present(input: &DockRailInput, presentation: &DockPresentation) -> DockPresentedLayout {
        Self::present_layout(&Self::layout(input), presentation)
    }

    /// 在已有落定布局上套用呈现输入。给已经自行合成条目（如让位空槽）的调用方复用。
    pub fn present_layout(
        layout: &DockLayout,
        presentation: &DockPresentation,
    ) -> DockPresentedLayout {
        let m = layout.metrics;
        let dock = layout.dock;
        let landed_plate = layout.plate;

        let plate_h = presentation
            .plate_height
            .filter(|v| v.is_finite() && *v >= 0.0)
            .map(|h| h.min(dock.size.height))
            .or_else(|| landed_plate.map(|p| p.size.height))
            .unwrap_or_else(|| m.plate_height.min(dock.size.height));
        let plate_top = presentation
            .plate_top
            .filter(|v| v.is_finite())
            .or_else(|| landed_plate.map(|p| p.origin.y))
            .unwrap_or(dock.origin.y);
        let plate_w = presentation
            .plate_width
            .filter(|v| v.is_finite())
            .or_else(|| landed_plate.map(|p| p.size.width));
        let plate = plate_w.and_then(|w| {
            (w > 0.0).then(|| {
                Rect::new(
                    dock.origin.x + (dock.size.width - w) / 2.0,
                    plate_top,
                    w,
                    plate_h,
                )
            })
        });

        // 内容左沿从「呈现后的板左沿 + 内边距」重新起算：板宽插值时槽位随板一起收放，
        // 不是把所有槽平移同一个常量（旧 `plate_rect` + arrange 的原始关系）。
        let content_left = plate
            .map(|p| p.origin.x)
            .unwrap_or(dock.origin.x + (dock.size.width - m.plate_pad * 2.0) / 2.0)
            + m.plate_pad;
        let entry_w = if layout.entry.is_some() {
            m.slot_width
        } else {
            0.0
        };
        let entry = layout
            .entry
            .map(|_| Rect::new(content_left, plate_top, m.slot_width, plate_h));
        let line_y = plate_top + plate_h - m.line_height;

        let mut zones = [
            DockPresentedZone {
                zone: DockZone::Pinned,
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                viewport: Rect::new(0.0, 0.0, 0.0, 0.0),
                before: 0,
                after: 0,
                visible: 0,
                chips: Vec::new(),
            },
            DockPresentedZone {
                zone: DockZone::Running,
                rect: Rect::new(0.0, 0.0, 0.0, 0.0),
                viewport: Rect::new(0.0, 0.0, 0.0, 0.0),
                before: 0,
                after: 0,
                visible: 0,
                chips: Vec::new(),
            },
        ];
        let mut x = content_left + entry_w;
        for (z, presented_zone) in zones.iter_mut().enumerate() {
            let src = &layout.zones[z];
            let zw = src.rect.size.width;
            let vx = x + if src.before > 0 { m.chip_width } else { 0.0 };
            let viewport = Rect::new(vx, plate_top, src.visible as f32 * m.slot_width, plate_h);
            let mut chips = Vec::with_capacity(src.chips.len());
            for chip in &src.chips {
                let cx = match chip.direction {
                    DockChipDirection::Prev => x,
                    DockChipDirection::Next => x + zw - m.chip_width,
                };
                chips.push(DockPresentedChip {
                    zone: chip.zone,
                    direction: chip.direction,
                    count: chip.count,
                    rect: Rect::new(cx, plate_top, m.chip_width, plate_h),
                    line: Rect::new(cx, line_y, m.chip_width, m.line_height),
                });
            }
            *presented_zone = DockPresentedZone {
                zone: src.zone,
                rect: Rect::new(x, plate_top, zw, plate_h),
                viewport,
                before: src.before,
                after: src.after,
                visible: src.visible,
                chips,
            };
            x += zw;
        }

        let mut slots = Vec::with_capacity(layout.slots.len());
        for slot in &layout.slots {
            let z = usize::from(slot.display >= layout.split);
            let local = if z == 0 {
                slot.display
            } else {
                slot.display - layout.split
            };
            let before = layout.zones[z].before as f32;
            let scroll = presentation
                .scroll
                .map(|s| s[z])
                .filter(|v| v.is_finite())
                .unwrap_or(before)
                .clamp(
                    0.0,
                    (layout.zones[z].before + layout.zones[z].visible) as f32,
                );
            let scroll_shift = (before - scroll) * m.slot_width;
            let dx = keyed_value(&presentation.slot_offsets, &slot.id, slot.source).unwrap_or(0.0);
            let sx = zones[z].viewport.origin.x
                + (local as f32 - before) * m.slot_width
                + scroll_shift
                + dx;
            let rect = Rect::new(sx, plate_top, m.slot_width, dock.size.height);
            let hit = if layout.overflow {
                clip_x(rect, zones[z].viewport)
            } else {
                Some(rect)
            };
            let icon_rect = Rect::new(
                sx + (m.slot_width - m.icon) / 2.0,
                plate_top + ((plate_h - m.icon) / 2.0).max(0.0),
                m.icon,
                m.icon,
            );
            let indicator = if slot.visible {
                if layout.overflow {
                    let vp = zones[z].viewport;
                    let x0 = sx.max(vp.origin.x);
                    let x1 = (sx + m.slot_width).min(vp.right());
                    (x1 - x0 > 0.01).then(|| Rect::new(x0, line_y, x1 - x0, m.line_height))
                } else {
                    // 不溢出时不裁：与旧 `render` 的整排线一致，位移槽的线跟着槽走。
                    Some(Rect::new(sx, line_y, m.slot_width, m.line_height))
                }
            } else {
                None
            };
            let natural = natural_line_progress(slot.indicator_kind);
            let line_progress = keyed_value(&presentation.line_progress, &slot.id, slot.source)
                .map(|v| v.clamp(0.0, 1.0))
                .unwrap_or(natural);
            slots.push(DockPresentedSlot {
                source: slot.source,
                display: slot.display,
                zone: slot.zone,
                id: slot.id.clone(),
                icon: slot.icon.clone(),
                rect,
                hit,
                icon_rect,
                visible: slot.visible,
                indicator: if hit.is_some() { indicator } else { None },
                indicator_kind: slot.indicator_kind,
                line_progress,
                dx,
            });
        }

        DockPresentedLayout {
            metrics: m,
            dock,
            plate,
            entry,
            zones,
            slots,
            line_y,
            line_height: m.line_height,
            overflow: layout.overflow,
        }
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

    // ── 呈现几何（2026-11-17）────────────────────────────────────────────

    /// C0 现场（`docs/research/m1115_dock/REPORT.md` §三）：1536 × 76、入口格开、1×，
    /// librarian / nvim 钉选未运行、kitty 运行且激活。
    fn c0_input() -> DockRailInput {
        let entries = vec![
            entry("org.ether.librarian", true, false, false),
            entry("nvim", true, false, false),
            entry("kitty", false, true, true),
        ];
        let mut input = DockRailInput::new(entries, Rect::new(0.0, 0.0, 1536.0, 76.0));
        input.show_entry = true;
        input
    }

    /// 落定态：期望值全部由 §158 常量与 C0 转录推出，不从 `layout()` 反推。
    #[test]
    fn presentation_settled_matches_c0_constants() {
        let p = DockPresentation::default();
        let l = DockRail::present(&c0_input(), &p);
        // 板宽 = 入口 60 + 三槽 180 + 左右内边距 24 = 264；居中 x = (1536 − 264) / 2。
        assert_eq!(l.plate, Some(Rect::new(636.0, 0.0, 264.0, 68.0)));
        // 内容左沿 = 636 + 12 = 648；槽从入口格后开始，步距 60。
        assert_eq!(l.entry, Some(Rect::new(648.0, 0.0, 60.0, 68.0)));
        let xs: Vec<f32> = l.slots.iter().map(|s| s.rect.origin.x).collect();
        assert_eq!(xs, [708.0, 768.0, 828.0]);
        assert_eq!(l.slots[0].icon_rect, Rect::new(714.0, 10.0, 48.0, 48.0));
        // 定位线贴板内底沿：y = 68 − 3 = 65，高 3；每段只到本槽宽。
        for s in &l.slots {
            assert_eq!(
                s.indicator,
                Some(Rect::new(s.rect.origin.x, 65.0, 60.0, 3.0))
            );
        }
        assert_eq!(l.slots[0].line_progress, 0.0, "Pinned = Divider");
        assert_eq!(l.slots[1].line_progress, 0.0);
        assert_eq!(
            l.slots[2].line_progress, 1.0,
            "Running + active = PrimaryHover"
        );
        // 命中按呈现几何：738 / 858 / 678 仍是 C0 的三处。
        assert_eq!(l.hit(Point::new(738.0, 34.0)), Some(DockHit::Slot(0)));
        assert_eq!(l.hit(Point::new(858.0, 34.0)), Some(DockHit::Slot(2)));
        assert_eq!(l.hit(Point::new(678.0, 34.0)), Some(DockHit::Entry));
    }

    /// 板宽插值到一半（264 → 204，取 234）：内容随板左沿重算，槽整体右移 15。
    #[test]
    fn presentation_plate_halfway_reflows_content() {
        let p = DockPresentation::default().with_plate_width(234.0);
        let l = DockRail::present(&c0_input(), &p);
        // (1536 − 234) / 2 = 651；内容 651 + 12 = 663；首槽 663 + 60 = 723。
        assert_eq!(l.plate, Some(Rect::new(651.0, 0.0, 234.0, 68.0)));
        assert_eq!(l.entry, Some(Rect::new(663.0, 0.0, 60.0, 68.0)));
        let xs: Vec<f32> = l.slots.iter().map(|s| s.rect.origin.x).collect();
        assert_eq!(xs, [723.0, 783.0, 843.0]);
    }

    /// 让位偏移一格：板宽 +60、目标槽与右区一起右移一格（C0 值 798 / 858）。
    /// 同时覆盖 id 键（kitty）与 `#<source>` 键（nvim，source 1）。
    #[test]
    fn presentation_pin_gap_offsets_by_one_slot() {
        let p = DockPresentation::default()
            .with_plate_width(324.0)
            .with_source_offset(1, 60.0)
            .with_slot_offset("kitty", 60.0);
        let l = DockRail::present(&c0_input(), &p);
        let x = |id: &str| {
            l.slots
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.rect.origin.x)
                .unwrap()
        };
        assert_eq!(x("org.ether.librarian"), 678.0, "让位前不动");
        assert_eq!(x("nvim"), 798.0, "让出一格");
        assert_eq!(x("kitty"), 858.0, "右区跟随板宽重算");
    }

    /// 板顶 / 板高是独立覆盖：内容与定位线一起下移、线贴新板底。
    #[test]
    fn presentation_plate_top_and_height_move_line() {
        let p = DockPresentation::default().with_plate(264.0, 4.0, 60.0);
        let l = DockRail::present(&c0_input(), &p);
        assert_eq!(l.plate, Some(Rect::new(636.0, 4.0, 264.0, 60.0)));
        assert_eq!(l.entry, Some(Rect::new(648.0, 4.0, 60.0, 60.0)));
        assert_eq!(l.slots[0].rect.origin.y, 4.0);
        // 线 y = 4 + 60 − 3 = 61。
        assert_eq!(
            l.slots[0].indicator,
            Some(Rect::new(708.0, 61.0, 60.0, 3.0))
        );
    }

    /// 滚动 0.5 槽：整数目标由 `scroll` 给（1），呈现位置 0.5 把槽右移半格。
    #[test]
    fn presentation_fractional_scroll_moves_half_a_slot() {
        let mut entries = Vec::new();
        for i in 0..10 {
            entries.push(entry(&format!("p{i}"), true, false, false));
        }
        for i in 0..2 {
            entries.push(entry(&format!("r{i}"), false, true, false));
        }
        let mut input = DockRailInput::new(entries, Rect::new(0.0, 0.0, 800.0, 76.0));
        input.show_entry = true;
        input.scroll = [1, 0];
        let mut p = DockPresentation::default().with_scroll([0.5, 0.0]);
        let l = DockRail::present(&input, &p);
        // 可用宽 656；溢出后左区 6 槽 + 两枚计数格、右区 2 槽。
        // 板宽 = 入口 60 + 左区 432 + 右区 120 + 内边距 24 = 636，左沿 (800−636)/2 = 82；
        // 内容 94、入口 94..154、左区 154、Prev 格 154..190、视口 190。
        assert!(l.overflow);
        assert_eq!(l.zones[0].before, 1);
        assert_eq!(l.zones[0].viewport.origin.x, 190.0);
        // 落定第 0 槽 = 190 − 60 = 130；滚动 0.5 槽右移 30 → 160，第 1 槽 220。
        assert_eq!(l.slots[0].rect.origin.x, 160.0);
        assert_eq!(l.slots[1].rect.origin.x, 220.0);
        // 回到整数 1.0：第 0 槽 130（被 Prev 格挡住），第 1 槽 190。
        p.scroll = Some([1.0, 0.0]);
        let l = DockRail::present(&input, &p);
        assert_eq!(l.slots[0].rect.origin.x, 130.0);
        assert_eq!(l.slots[1].rect.origin.x, 190.0);
    }

    /// 三档线色进度的端点与中点：只换进度，不改落定几何。
    #[test]
    fn presentation_line_progress_endpoints() {
        let mut p = DockPresentation::default();
        let natural = DockRail::present(&c0_input(), &p);
        assert_eq!(natural.slots[0].line_progress, 0.0);
        assert_eq!(natural.slots[1].line_progress, 0.0);
        assert_eq!(natural.slots[2].line_progress, 1.0);
        p.line_progress = vec![
            ("org.ether.librarian".into(), 0.0),
            ("nvim".into(), 0.25),
            ("kitty".into(), 1.0),
        ];
        let l = DockRail::present(&c0_input(), &p);
        assert_eq!(l.slots[0].line_progress, 0.0);
        assert_eq!(l.slots[1].line_progress, 0.25);
        assert_eq!(l.slots[2].line_progress, 1.0);
        // 呈现几何不因线色改动。
        assert_eq!(l.plate, natural.plate);
        assert_eq!(
            l.slots.iter().map(|s| s.rect).collect::<Vec<_>>(),
            natural.slots.iter().map(|s| s.rect).collect::<Vec<_>>()
        );
    }
}
