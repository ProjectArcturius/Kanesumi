// file_views.rs —— 文件视图三件套的纯几何提炼。参 docs/supply/TILE_WALL_FILE_VIEWS.md。
//
// 提炼自 librarian 的 `window/views/list.rs`（`cols` / `name_parts`）、
// `window/sidebar.rs`（`row_geometry` / `eject_rect` / 选中条）与 `window/pathbar.rs`
// （`seg_layout` / 状态右对齐）。输入视口、条目度量与（调用方量好的）文本宽度；输出
// 行 / 列 / 分段矩形与选中、悬停、缩进槽位。
//
// **只算几何**：不画、不读主题、不量字（文本宽度由调用方的 TextEngine 量好传入），
// 不吃输入事件。业务状态（哪些项是书签 / 是否回收站 / 选中集合）一律由调用方给。
//
// 形状冻结（参 Kanesumi docs/API_STABILITY.md §三）：现有字段不改名、不改语义；新增
// 字段必须带 `Default`，枚举只许加变体（`FileListColumn` 已标 `#[non_exhaustive]`）。

use kanesumi_core::{Point, Rect};

/// 列表行高（librarian `spec::ROW_H`）。
pub const FILE_ROW_H: f32 = 28.0;
/// 列表表头高（librarian `spec::LIST_HEADER_H`）。
pub const FILE_HEADER_H: f32 = 28.0;
/// 列表左右内边距（librarian `spec::ROW_PAD_H`）。
pub const FILE_PAD_H: f32 = 12.0;
/// 行内图标边长（librarian `spec::ROW_ICON`）。
pub const FILE_ICON: f32 = 16.0;
/// 行内图文间距（librarian `spec::ROW_GAP`）。
pub const FILE_GAP: f32 = 8.0;
/// 展开三角宽（librarian `spec::TRI_W`）。
pub const FILE_TRI_W: f32 = 12.0;
/// 子项每层缩进（librarian `spec::INDENT`）。
pub const FILE_INDENT: f32 = 20.0;
/// 名称列至少留多宽；再窄就丢掉右侧次要列（多列自适应）。
pub const FILE_MIN_NAME_W: f32 = 120.0;

/// 列表列身份。顺序即从右往左的丢弃顺序：`Kind` 先让位，`Name` 永不丢。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FileListColumn {
    Name,
    Date,
    Size,
    Kind,
    Location,
    Deleted,
}

/// 列表度量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileListMetrics {
    pub row_height: f32,
    pub header_height: f32,
    pub pad_h: f32,
    pub icon: f32,
    pub gap: f32,
    pub tri_width: f32,
    pub indent: f32,
    pub min_name_width: f32,
}

impl FileListMetrics {
    pub const BASE: Self = Self {
        row_height: FILE_ROW_H,
        header_height: FILE_HEADER_H,
        pad_h: FILE_PAD_H,
        icon: FILE_ICON,
        gap: FILE_GAP,
        tri_width: FILE_TRI_W,
        indent: FILE_INDENT,
        min_name_width: FILE_MIN_NAME_W,
    };

    pub fn normalized(self) -> Self {
        let len = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        Self {
            row_height: len(self.row_height).max(1.0),
            header_height: len(self.header_height),
            pad_h: len(self.pad_h),
            icon: len(self.icon),
            gap: len(self.gap),
            tri_width: len(self.tri_width),
            indent: len(self.indent),
            min_name_width: len(self.min_name_width),
        }
    }
}

impl Default for FileListMetrics {
    fn default() -> Self {
        Self::BASE
    }
}

/// 右侧次要列宽（名称列吃余量）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileListColumns {
    pub date: f32,
    pub size: f32,
    pub kind: f32,
    pub location: f32,
    pub deleted: f32,
}

impl FileListColumns {
    pub const BASE: Self = Self {
        date: 180.0,
        size: 90.0,
        kind: 140.0,
        location: 180.0,
        deleted: 110.0,
    };

    pub fn normalized(self) -> Self {
        let len = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        Self {
            date: len(self.date),
            size: len(self.size),
            kind: len(self.kind),
            location: len(self.location),
            deleted: len(self.deleted),
        }
    }
}

impl Default for FileListColumns {
    fn default() -> Self {
        Self::BASE
    }
}

/// 列表一项的**几何输入**：名称自然宽（调用方量好）+ 缩进层级 + 是否有展开三角。
///
/// 控件不保存文件名：名称超宽时只回 `name_elided`，省略号由绘制层按同一矩形画。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileListItem {
    pub name_width: f32,
    pub depth: u8,
    pub expandable: bool,
}

impl FileListItem {
    pub fn new(name_width: f32) -> Self {
        Self {
            name_width: if name_width.is_finite() {
                name_width.max(0.0)
            } else {
                0.0
            },
            depth: 0,
            expandable: false,
        }
    }

    pub fn depth(mut self, depth: u8) -> Self {
        self.depth = depth;
        self
    }

    pub fn expandable(mut self, on: bool) -> Self {
        self.expandable = on;
        self
    }
}

/// 列表布局输入。`area` 含表头；表头固定，滚动只作用于行区。
#[derive(Debug, Clone, PartialEq)]
pub struct FileListInput {
    pub area: Rect,
    pub items: Vec<FileListItem>,
    pub columns: FileListColumns,
    /// 回收站视图：原位置 / 删除日期两列参与布局。
    pub trash: bool,
    pub scroll: f32,
    pub metrics: FileListMetrics,
    pub selected: Option<usize>,
    pub hover: Option<usize>,
}

impl FileListInput {
    pub fn new(area: Rect, items: Vec<FileListItem>) -> Self {
        Self {
            area,
            items,
            columns: FileListColumns::BASE,
            trash: false,
            scroll: 0.0,
            metrics: FileListMetrics::BASE,
            selected: None,
            hover: None,
        }
    }
}

/// 一列的表头 / 行共用几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileListColumnSlot {
    pub column: FileListColumn,
    pub rect: Rect,
}

/// 一行可见行。`clip` 是行与行区的交集；`name` 是名称文本可用的矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileListRowSlot {
    pub index: usize,
    pub rect: Rect,
    pub clip: Rect,
    pub triangle: Option<Rect>,
    pub icon: Rect,
    pub name: Rect,
    /// 名称自然宽超过名称矩形 → 绘制层应画省略号。
    pub name_elided: bool,
}

/// 列表一次布局的产物。
#[derive(Debug, Clone, PartialEq)]
pub struct FileListLayout {
    /// 本帧实际显示的列（名称列恒在首位）。
    pub columns: Vec<FileListColumnSlot>,
    /// 只含可见行。
    pub rows: Vec<FileListRowSlot>,
    pub header: Rect,
    pub body: Rect,
    pub content_height: f32,
    pub max_scroll: f32,
    pub scroll: f32,
    /// 行总数（含视口外的行）。
    pub count: usize,
    pub row_height: f32,
    /// 选中 / 悬停行的整行槽位（已裁到行区；不可见为 `None`）。
    pub selected_slot: Option<Rect>,
    pub hover_slot: Option<Rect>,
}

impl FileListLayout {
    /// 任意行的整行矩形（可能在行区外）；用于命中与滚动定位。
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.count {
            return None;
        }
        Some(Rect::new(
            self.body.origin.x,
            self.body.origin.y - self.scroll + index as f32 * self.row_height,
            self.body.size.width,
            self.row_height,
        ))
    }

    /// 任意行可绘制 / 可悬停的槽位（行 ∩ 行区）；不可见返回 `None`。
    pub fn slot_rect(&self, index: usize) -> Option<Rect> {
        self.row_rect(index)?.intersect(self.body)
    }

    /// 行区坐标 → 行下标（表头与空白返回 `None`）。
    pub fn row_at(&self, pos: Point) -> Option<usize> {
        if !self.body.contains(pos) || self.count == 0 {
            return None;
        }
        let idx = ((pos.y - self.body.origin.y + self.scroll) / self.row_height).floor();
        (idx >= 0.0 && (idx as usize) < self.count).then_some(idx as usize)
    }
}

/// 列表布局：表头固定，行区按 `scroll` 虚拟化；只回可见行。
///
/// 多列自适应：名称列吃余量，右侧次要列按 `Kind → Size → Date → Deleted → Location`
/// 的顺序让位（回收站视图同样先丢 Kind / Size）——名称列窄于 `min_name_width` 就再丢
/// 一列，直到只剩名称。参 librarian `window/views/list.rs::cols`（原实现不丢列，窄窗
/// 会把名称压到 0，这里补上退化策略）。
pub fn file_list_layout(input: &FileListInput) -> FileListLayout {
    let m = input.metrics.normalized();
    let area = input.area.normalized();
    let cw = input.columns.normalized();
    let header = Rect::new(
        area.origin.x,
        area.origin.y,
        area.size.width,
        m.header_height,
    );
    let body = Rect::new(
        area.origin.x,
        area.origin.y + m.header_height,
        area.size.width,
        (area.size.height - m.header_height).max(0.0),
    );

    let inner = (area.size.width - 2.0 * m.pad_h).max(0.0);
    let candidates: [(FileListColumn, f32); 4] = if input.trash {
        [
            (FileListColumn::Location, cw.location),
            (FileListColumn::Deleted, cw.deleted),
            (FileListColumn::Size, cw.size),
            (FileListColumn::Kind, cw.kind),
        ]
    } else {
        [
            (FileListColumn::Date, cw.date),
            (FileListColumn::Size, cw.size),
            (FileListColumn::Kind, cw.kind),
            (FileListColumn::Kind, 0.0),
        ]
    };
    // 普通视图只有三项候选；回收站四项。用 `take` 切掉填充项。
    let candidate_count = if input.trash { 4 } else { 3 };
    let mut keep = candidate_count;
    let mut fixed: f32 = candidates[..keep].iter().map(|(_, w)| *w).sum();
    while keep > 0 && inner - fixed < m.min_name_width {
        keep -= 1;
        fixed -= candidates[keep].1;
    }

    let name_w = (inner - fixed).max(0.0);
    let mut columns = vec![FileListColumnSlot {
        column: FileListColumn::Name,
        rect: Rect::new(
            area.origin.x + m.pad_h,
            header.origin.y,
            name_w,
            m.header_height,
        ),
    }];
    let mut right = area.right() - m.pad_h;
    let mut secondary = Vec::with_capacity(keep);
    for (col, w) in candidates[..keep].iter().rev() {
        right -= *w;
        secondary.push(FileListColumnSlot {
            column: *col,
            rect: Rect::new(right, header.origin.y, *w, m.header_height),
        });
    }
    secondary.reverse();
    columns.extend(secondary);

    let count = input.items.len();
    let content_height = count as f32 * m.row_height;
    let max_scroll = (content_height - body.size.height).max(0.0);
    let scroll = if input.scroll.is_finite() {
        input.scroll.clamp(0.0, max_scroll)
    } else {
        0.0
    };

    let mut rows = Vec::new();
    if count > 0 && !body.is_empty() {
        let first = (scroll / m.row_height).floor() as usize;
        let last = (((scroll + body.size.height) / m.row_height).ceil() as usize).min(count);
        for index in first..last {
            let rect = Rect::new(
                body.origin.x,
                body.origin.y - scroll + index as f32 * m.row_height,
                body.size.width,
                m.row_height,
            );
            let Some(clip) = rect.intersect(body) else {
                continue;
            };
            let item = input.items[index];
            let name_x = columns[0].rect.origin.x;
            let base = name_x + item.depth as f32 * m.indent;
            let (triangle, content_x) = if item.expandable {
                (
                    Some(Rect::new(
                        base,
                        rect.origin.y + (m.row_height - m.tri_width) / 2.0,
                        m.tri_width,
                        m.tri_width,
                    )),
                    base + m.tri_width + m.gap,
                )
            } else {
                (None, base)
            };
            let icon = Rect::new(
                content_x,
                rect.origin.y + (m.row_height - m.icon) / 2.0,
                m.icon,
                m.icon,
            );
            let text_x = content_x + m.icon + m.gap;
            let text_w = (name_x + columns[0].rect.size.width - text_x).max(0.0);
            rows.push(FileListRowSlot {
                index,
                rect,
                clip,
                triangle,
                icon,
                name: Rect::new(text_x, rect.origin.y, text_w, m.row_height),
                name_elided: item.name_width > text_w + 0.01,
            });
        }
    }

    let slot = |index: Option<usize>| {
        index.filter(|i| *i < count).and_then(|i| {
            let y = body.origin.y - scroll + i as f32 * m.row_height;
            Rect::new(body.origin.x, y, body.size.width, m.row_height).intersect(body)
        })
    };
    let selected_slot = slot(input.selected);
    let hover_slot = slot(input.hover);

    FileListLayout {
        columns,
        rows,
        header,
        body,
        content_height,
        max_scroll,
        scroll,
        count,
        row_height: m.row_height,
        selected_slot,
        hover_slot,
    }
}

/// 侧栏行高（librarian `spec::SB_ROW_H`）。
pub const SIDEBAR_ROW_H: f32 = 32.0;
/// 侧栏分组头高（上 12 + 16 行 + 下 4，librarian `spec::SB_HEADER_*`）。
pub const SIDEBAR_HEADER_H: f32 = 32.0;
/// 侧栏左内边距（librarian `spec::SB_PAD_L`）。
pub const SIDEBAR_PAD_L: f32 = 16.0;
/// 侧栏图标边长（librarian `spec::SB_ICON`）。
pub const SIDEBAR_ICON: f32 = 16.0;
/// 侧栏图文间距（librarian `spec::SB_GAP`）。
pub const SIDEBAR_GAP: f32 = 12.0;
/// 选中条尺寸与上距（librarian `spec::SB_SEL_BAR_*`）。
pub const SIDEBAR_SEL_BAR_W: f32 = 4.0;
pub const SIDEBAR_SEL_BAR_H: f32 = 16.0;
pub const SIDEBAR_SEL_BAR_TOP: f32 = 8.0;
/// 设备行尾弹出钮（librarian `spec::SB_EJECT_*`）。
pub const SIDEBAR_EJECT_W: f32 = 24.0;
pub const SIDEBAR_EJECT_PAD_R: f32 = 8.0;

/// 侧栏度量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarMetrics {
    pub pad_t: f32,
    pub pad_l: f32,
    pub header_height: f32,
    pub row_height: f32,
    pub icon: f32,
    pub gap: f32,
    /// 每级缩进（librarian 现状无嵌套，默认 16 供书签分组使用）。
    pub indent: f32,
    pub sel_bar_w: f32,
    pub sel_bar_h: f32,
    pub sel_bar_top: f32,
    pub eject_w: f32,
    pub eject_pad_r: f32,
}

impl SidebarMetrics {
    pub const BASE: Self = Self {
        pad_t: 4.0,
        pad_l: SIDEBAR_PAD_L,
        header_height: SIDEBAR_HEADER_H,
        row_height: SIDEBAR_ROW_H,
        icon: SIDEBAR_ICON,
        gap: SIDEBAR_GAP,
        indent: 16.0,
        sel_bar_w: SIDEBAR_SEL_BAR_W,
        sel_bar_h: SIDEBAR_SEL_BAR_H,
        sel_bar_top: SIDEBAR_SEL_BAR_TOP,
        eject_w: SIDEBAR_EJECT_W,
        eject_pad_r: SIDEBAR_EJECT_PAD_R,
    };

    pub fn normalized(self) -> Self {
        let len = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        Self {
            pad_t: len(self.pad_t),
            pad_l: len(self.pad_l),
            header_height: len(self.header_height),
            row_height: len(self.row_height),
            icon: len(self.icon),
            gap: len(self.gap),
            indent: len(self.indent),
            sel_bar_w: len(self.sel_bar_w),
            sel_bar_h: len(self.sel_bar_h),
            sel_bar_top: len(self.sel_bar_top),
            eject_w: len(self.eject_w),
            eject_pad_r: len(self.eject_pad_r),
        }
    }
}

impl Default for SidebarMetrics {
    fn default() -> Self {
        Self::BASE
    }
}

/// 侧栏一项的几何输入。`header` = 分组标题（不可点、不缩进）；`indent` 只作用于行内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SidebarEntry {
    pub header: bool,
    pub indent: u8,
    pub ejectable: bool,
}

impl SidebarEntry {
    pub const fn row() -> Self {
        Self {
            header: false,
            indent: 0,
            ejectable: false,
        }
    }

    pub const fn group() -> Self {
        Self {
            header: true,
            indent: 0,
            ejectable: false,
        }
    }

    pub const fn indent(mut self, level: u8) -> Self {
        self.indent = level;
        self
    }

    pub const fn ejectable(mut self, on: bool) -> Self {
        self.ejectable = on;
        self
    }
}

/// 侧栏布局输入。`collapsed` = 整栏折叠（宽 0、无行），供外壳做收起动画的落定格。
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarInput {
    pub area: Rect,
    pub entries: Vec<SidebarEntry>,
    pub selected: Option<usize>,
    pub hover: Option<usize>,
    pub collapsed: bool,
    pub metrics: SidebarMetrics,
}

impl SidebarInput {
    pub fn new(area: Rect, entries: Vec<SidebarEntry>) -> Self {
        Self {
            area,
            entries,
            selected: None,
            hover: None,
            collapsed: false,
            metrics: SidebarMetrics::BASE,
        }
    }
}

/// 一行侧栏的槽位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SidebarSlot {
    pub index: usize,
    pub header: bool,
    pub rect: Rect,
    pub clip: Rect,
    pub icon: Option<Rect>,
    pub text: Rect,
    pub eject: Option<Rect>,
    /// 行内容的缩进像素（分组头恒 0）。
    pub indent: f32,
    pub selected: bool,
    pub hovered: bool,
}

/// 侧栏一次布局的产物。
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarLayout {
    /// 只含可见行。
    pub rows: Vec<SidebarSlot>,
    /// 选中行左侧的强调条（不可见 / 折叠时为 `None`）。
    pub selected_bar: Option<Rect>,
    pub content_height: f32,
    /// 折叠时宽 0，否则 = 输入区域宽。
    pub width: f32,
    pub collapsed: bool,
}

impl SidebarLayout {
    /// 行区坐标 → 行下标（分组头不可点，返回 `None`）。
    pub fn row_at(&self, pos: Point) -> Option<usize> {
        self.rows
            .iter()
            .find(|r| !r.header && r.clip.contains(pos))
            .map(|r| r.index)
    }
}

/// 侧栏布局：分组头 32、行 32；选中条贴行左缘，缩进只挪图标 / 文字。
///
/// 与 librarian `window/sidebar.rs::row_geometry` 同高同序；`indent` 是新增能力（现状
/// 侧栏不嵌套），默认 0 时几何与现状逐像素一致。
pub fn sidebar_layout(input: &SidebarInput) -> SidebarLayout {
    let m = input.metrics.normalized();
    let area = input.area.normalized();
    if input.collapsed {
        return SidebarLayout {
            rows: Vec::new(),
            selected_bar: None,
            content_height: 0.0,
            width: 0.0,
            collapsed: true,
        };
    }

    let mut y = area.origin.y + m.pad_t;
    let mut rows = Vec::new();
    for (index, entry) in input.entries.iter().enumerate() {
        let h = if entry.header {
            m.header_height
        } else {
            m.row_height
        };
        let rect = Rect::new(area.origin.x, y, area.size.width, h);
        y += h;
        let Some(clip) = rect.intersect(area) else {
            continue;
        };
        let indent = if entry.header {
            0.0
        } else {
            entry.indent as f32 * m.indent
        };
        let base_x = area.origin.x + m.pad_l + indent;
        let row = SidebarSlot {
            index,
            header: entry.header,
            rect,
            clip,
            icon: None,
            text: Rect::new(
                area.origin.x + m.pad_l,
                rect.origin.y,
                (area.size.width - 2.0 * m.pad_l).max(0.0),
                h,
            ),
            eject: None,
            indent,
            selected: input.selected == Some(index) && !entry.header,
            hovered: input.hover == Some(index) && !entry.header,
        };
        let row = if entry.header {
            row
        } else {
            let icon = Rect::new(base_x, rect.origin.y + (h - m.icon) / 2.0, m.icon, m.icon);
            let eject = entry.ejectable.then(|| {
                Rect::new(
                    area.right() - m.eject_pad_r - m.eject_w,
                    rect.origin.y + (h - m.eject_w) / 2.0,
                    m.eject_w,
                    m.eject_w,
                )
            });
            let text_x = base_x + m.icon + m.gap;
            let text_right = eject
                .map(|e| e.origin.x - m.gap)
                .unwrap_or(area.right() - m.pad_l);
            SidebarSlot {
                icon: Some(icon),
                text: Rect::new(text_x, rect.origin.y, (text_right - text_x).max(0.0), h),
                eject,
                ..row
            }
        };
        rows.push(row);
    }

    let content_height = (y - area.origin.y).max(0.0);
    let selected_bar = input
        .selected
        .filter(|i| *i < input.entries.len())
        .filter(|i| !input.entries[*i].header)
        .and_then(|i| {
            let y = area.origin.y
                + m.pad_t
                + input.entries[..i]
                    .iter()
                    .map(|e| {
                        if e.header {
                            m.header_height
                        } else {
                            m.row_height
                        }
                    })
                    .sum::<f32>();
            Rect::new(area.origin.x, y + m.sel_bar_top, m.sel_bar_w, m.sel_bar_h).intersect(area)
        });

    SidebarLayout {
        rows,
        selected_bar,
        content_height,
        width: area.size.width,
        collapsed: false,
    }
}

/// 路径栏高（librarian `spec::PATH_H`）。
pub const PATH_BAR_H: f32 = 28.0;
/// 路径栏左右内边距（librarian `spec::PATH_PAD_H`）。
pub const PATH_BAR_PAD_H: f32 = 12.0;
/// 段与分隔符的间距（librarian `spec::PATH_GAP`）。
pub const PATH_BAR_GAP: f32 = 4.0;
/// 盘符图标边长（librarian `spec::PATH_ICON`）。
pub const PATH_BAR_ICON: f32 = 12.0;
/// 段最小宽：再窄就丢前导段，用省略号代替。
pub const PATH_BAR_MIN_SEGMENT_W: f32 = 24.0;
/// 前导省略号宽（「…」占位）。
pub const PATH_BAR_ELLIPSIS_W: f32 = 12.0;

/// 路径栏度量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathBarMetrics {
    pub height: f32,
    pub pad_h: f32,
    pub gap: f32,
    pub icon: f32,
    /// 「›」分隔符的自然宽（调用方量好；默认 6）。
    pub separator_width: f32,
    pub min_segment_width: f32,
    pub ellipsis_width: f32,
}

impl PathBarMetrics {
    pub const BASE: Self = Self {
        height: PATH_BAR_H,
        pad_h: PATH_BAR_PAD_H,
        gap: PATH_BAR_GAP,
        icon: PATH_BAR_ICON,
        separator_width: 6.0,
        min_segment_width: PATH_BAR_MIN_SEGMENT_W,
        ellipsis_width: PATH_BAR_ELLIPSIS_W,
    };

    pub fn normalized(self) -> Self {
        let len = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        Self {
            height: len(self.height),
            pad_h: len(self.pad_h),
            gap: len(self.gap),
            icon: len(self.icon),
            separator_width: len(self.separator_width),
            min_segment_width: len(self.min_segment_width).max(1.0),
            ellipsis_width: len(self.ellipsis_width),
        }
    }
}

impl Default for PathBarMetrics {
    fn default() -> Self {
        Self::BASE
    }
}

/// 一个路径段：`label` 只随分段槽位回传；`width` 是调用方量好的自然宽。
#[derive(Debug, Clone, PartialEq)]
pub struct PathBarSegment {
    pub label: String,
    pub width: f32,
}

impl PathBarSegment {
    pub fn new(label: impl Into<String>, width: f32) -> Self {
        Self {
            label: label.into(),
            width: if width.is_finite() {
                width.max(0.0)
            } else {
                0.0
            },
        }
    }
}

/// 路径栏布局输入。`status_width` 是右侧状态文本的自然宽（0 = 不显示）。
#[derive(Debug, Clone, PartialEq)]
pub struct PathBarInput {
    pub area: Rect,
    pub segments: Vec<PathBarSegment>,
    pub icon: bool,
    pub status_width: f32,
    pub metrics: PathBarMetrics,
}

impl PathBarInput {
    pub fn new(area: Rect, segments: Vec<PathBarSegment>) -> Self {
        Self {
            area,
            segments,
            icon: false,
            status_width: 0.0,
            metrics: PathBarMetrics::BASE,
        }
    }
}

/// 一个实际绘制的路径段。`separator` = 它前面的「›」矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PathBarSegmentSlot {
    /// 输入段下标（被丢掉的段不出现）。
    pub index: usize,
    pub rect: Rect,
    pub separator: Option<Rect>,
    /// 宽度被压缩到自然宽以下 → 绘制层应画省略号。
    pub elided: bool,
}

/// 路径栏一次布局的产物。
#[derive(Debug, Clone, PartialEq)]
pub struct PathBarLayout {
    pub segments: Vec<PathBarSegmentSlot>,
    /// 前导段被丢弃时的「…」槽位（含其后的分隔符间距）。
    pub ellipsis: Option<Rect>,
    pub icon: Option<Rect>,
    pub status: Option<Rect>,
    /// 有任何段被压缩或丢弃。
    pub overflow: bool,
    /// 段区可用宽（扣掉图标与状态）。
    pub available: f32,
}

impl PathBarLayout {
    /// 命中某段（`pos` 在该段矩形内）。
    pub fn segment_at(&self, pos: Point) -> Option<&PathBarSegmentSlot> {
        self.segments.iter().find(|s| s.rect.contains(pos))
    }
}

/// 路径栏布局：面包屑从右往左保空间，前导段放不下就折叠成「…」。
///
/// 省略策略（逐级退化）：
/// 1. 全放得下 → 按自然宽排；
/// 2. 放不下但都到最小宽还能放 → 每段先给 `min_segment_width`，余量**从右往左**
///    补给（当前目录优先拿到完整宽度）；
/// 3. 连最小宽都放不下 → 从左边丢段，用一个 `ellipsis_width` 的「…」占位；
/// 4. 只剩最后一段仍超宽 → 最后一段吃满剩余宽并标 `elided`。
///
/// 与 librarian `window/pathbar.rs::seg_layout` 的差别：现状不压缩、长路径直接与右侧
/// 状态重叠（源码注释已自认「r1 从简」），这里补的是退化策略本身。
pub fn path_bar_layout(input: &PathBarInput) -> PathBarLayout {
    let m = input.metrics.normalized();
    let area = input.area.normalized();
    let y = area.origin.y + (area.size.height - m.height).max(0.0) / 2.0;
    let line_h = m.height.min(area.size.height);
    let mut x = area.origin.x + m.pad_h;
    let icon = input.icon.then(|| {
        let r = Rect::new(x, y + (m.height - m.icon) / 2.0, m.icon, m.icon);
        x += m.icon + m.gap;
        r
    });
    let status_w = if input.status_width.is_finite() {
        input.status_width.max(0.0)
    } else {
        0.0
    };
    let status =
        (status_w > 0.0).then(|| Rect::new(area.right() - m.pad_h - status_w, y, status_w, line_h));
    let right_limit = status
        .map(|s| s.origin.x - m.gap)
        .unwrap_or(area.right() - m.pad_h);
    let available = (right_limit - x).max(0.0);

    let natural: Vec<f32> = input.segments.iter().map(|s| s.width).collect();
    let sep = m.separator_width + 2.0 * m.gap;
    let (start, widths) = elide_segments(&natural, &m, sep, available);

    let mut segments = Vec::with_capacity(widths.len());
    let mut ellipsis = None;
    if start > 0 {
        let rect = Rect::new(x, y, m.ellipsis_width, line_h);
        ellipsis = Some(rect);
        x += m.ellipsis_width;
        // 「…」之后、首段之前仍有一个分隔符；由下面的循环统一补 gap + sep + gap。
    }
    for (j, w) in widths.iter().enumerate() {
        let index = start + j;
        if index > start || start > 0 {
            // 分隔符画在段前的间隙里（与 librarian 的 `x - gap - measure("›")` 同式）。
            x += m.gap;
        }
        let separator =
            (index > start || start > 0).then(|| Rect::new(x, y, m.separator_width, line_h));
        if separator.is_some() {
            x += m.separator_width + m.gap;
        }
        let rect = Rect::new(x, y, *w, line_h);
        x += *w;
        segments.push(PathBarSegmentSlot {
            index,
            rect,
            separator,
            elided: *w + 0.01 < natural[index],
        });
    }

    let overflow = start > 0 || segments.iter().any(|s| s.elided);
    PathBarLayout {
        segments,
        ellipsis,
        icon,
        status,
        overflow,
        available,
    }
}

/// 返回 `(起始段下标, 每段宽)`。见 `path_bar_layout` 的四级退化。
fn elide_segments(natural: &[f32], m: &PathBarMetrics, sep: f32, avail: f32) -> (usize, Vec<f32>) {
    let n = natural.len();
    if n == 0 {
        return (0, Vec::new());
    }
    let total: f32 = natural.iter().sum::<f32>() + (n - 1) as f32 * sep;
    if total <= avail {
        return (0, natural.to_vec());
    }
    let min_total = n as f32 * m.min_segment_width + (n - 1) as f32 * sep;
    if min_total <= avail {
        return (0, distribute(natural, m.min_segment_width, sep, avail));
    }
    // 丢前导段：保留最后一个，省略号本身也占 `ellipsis + sep`。
    let lead = m.ellipsis_width + sep;
    for start in 1..n {
        let kept = n - start;
        let need = lead + kept as f32 * m.min_segment_width + (kept - 1) as f32 * sep;
        if need <= avail {
            let budget = (avail - lead).max(0.0);
            return (
                start,
                distribute(&natural[start..], m.min_segment_width, sep, budget),
            );
        }
    }
    // 只剩最后一段，仍超宽：吃满剩余宽并标省略。start 归 0 时没有省略号，
    // 不能再扣 `ellipsis + sep`。
    let lead = if n > 1 { m.ellipsis_width + sep } else { 0.0 };
    let budget = (avail - lead).max(0.0);
    (n - 1, vec![budget.min(natural[n - 1])])
}

/// 每段先给最小宽，余量从右往左补（右端 = 当前目录优先完整）。
fn distribute(natural: &[f32], min: f32, sep: f32, avail: f32) -> Vec<f32> {
    let n = natural.len();
    let mut widths = vec![min; n];
    let base = n as f32 * min + (n - 1) as f32 * sep;
    let mut extra = (avail - base).max(0.0);
    for i in (0..n).rev() {
        let add = (natural[i] - min).max(0.0).min(extra);
        widths[i] += add;
        extra -= add;
    }
    widths
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn item(name_width: f32) -> FileListItem {
        FileListItem::new(name_width)
    }

    #[test]
    fn file_list_zero_items_has_empty_body() {
        let l = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 800.0, 400.0),
            Vec::new(),
        ));
        assert_eq!(l.count, 0);
        assert!(l.rows.is_empty());
        assert_eq!(l.content_height, 0.0);
        assert_eq!(l.max_scroll, 0.0);
        assert_eq!(l.header.size.height, FILE_HEADER_H);
        assert!(close(l.body.origin.y, FILE_HEADER_H));
        assert!(l.selected_slot.is_none() && l.hover_slot.is_none());
        assert_eq!(l.row_at(Point::new(10.0, 40.0)), None);
    }

    #[test]
    fn file_list_thousand_items_only_lay_out_visible_rows() {
        let items = (0..1000).map(|_| item(60.0)).collect();
        let l = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 800.0, 400.0),
            items,
        ));
        assert_eq!(l.count, 1000);
        assert!(
            l.rows.len() < 20,
            "1000 行里只该回一屏，实际 {}",
            l.rows.len()
        );
        assert_eq!(l.rows.first().unwrap().index, 0);
        // 行矩形连续、每行 28。
        for w in l.rows.windows(2) {
            assert!(close(w[1].rect.origin.y - w[0].rect.origin.y, FILE_ROW_H));
        }
        // 滚到中段仍只回一屏，并从正确下标开始。
        let mut input = FileListInput::new(
            Rect::new(0.0, 0.0, 800.0, 400.0),
            (0..1000).map(|_| item(60.0)).collect(),
        );
        input.scroll = 1000.0;
        let l = file_list_layout(&input);
        assert_eq!(
            l.rows.first().unwrap().index,
            (1000.0f32 / FILE_ROW_H).floor() as usize
        );
        assert!(l.rows.len() < 20);
        assert!(l.max_scroll > 0.0);
    }

    #[test]
    fn long_file_name_is_elided_in_the_name_column() {
        let l = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 300.0, 400.0),
            vec![item(400.0).expandable(true)],
        ));
        assert_eq!(l.columns.len(), 1, "窄窗应只剩名称列");
        let row = &l.rows[0];
        assert!(row.name.size.width < 400.0);
        assert!(row.name_elided, "自然宽超过名称矩形必须标省略");
        assert!(row.name.origin.x >= row.icon.right());
    }

    #[test]
    fn narrow_window_drops_secondary_columns() {
        let wide = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 1000.0, 400.0),
            vec![item(60.0)],
        ));
        assert_eq!(wide.columns.len(), 4, "宽窗：名称 + 日期 + 大小 + 种类");
        let mid = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 500.0, 400.0),
            vec![item(60.0)],
        ));
        assert_eq!(mid.columns.len(), 3, "中等宽度：名称 + 日期 + 大小");
        let narrow = file_list_layout(&FileListInput::new(
            Rect::new(0.0, 0.0, 260.0, 400.0),
            vec![item(60.0)],
        ));
        assert_eq!(narrow.columns.len(), 1, "窄窗退化到只有名称列");
        assert_eq!(narrow.columns[0].column, FileListColumn::Name);
        // 列不重叠、名称列左起内边距。
        assert!(close(narrow.columns[0].rect.origin.x, FILE_PAD_H));
    }

    #[test]
    fn file_list_selection_and_hover_slots_follow_scroll() {
        let mut input = FileListInput::new(
            Rect::new(0.0, 0.0, 800.0, 400.0),
            (0..20).map(|_| item(40.0)).collect(),
        );
        input.selected = Some(6);
        input.hover = Some(2);
        let l = file_list_layout(&input);
        let sel = l.selected_slot.unwrap();
        assert!(close(sel.origin.y, FILE_HEADER_H + 6.0 * FILE_ROW_H));
        assert!(close(sel.size.height, FILE_ROW_H));
        assert!(close(
            l.hover_slot.unwrap().origin.y,
            FILE_HEADER_H + 2.0 * FILE_ROW_H
        ));
        assert_eq!(
            l.row_at(Point::new(100.0, FILE_HEADER_H + FILE_ROW_H + 1.0)),
            Some(1)
        );
        // 表头不算行。
        assert_eq!(l.row_at(Point::new(100.0, 2.0)), None);
        // 滚六行后选中槽位上移到表头下沿，滚出视口的前几行没有槽位。
        input.scroll = 6.0 * FILE_ROW_H;
        let l = file_list_layout(&input);
        assert!(close(l.selected_slot.unwrap().origin.y, FILE_HEADER_H));
        assert!(l.slot_rect(0).is_none(), "滚出十行后第 0 行已完全不可见");
    }

    #[test]
    fn file_rows_carry_depth_triangle_and_icon() {
        let mut input = FileListInput::new(
            Rect::new(0.0, 0.0, 800.0, 400.0),
            vec![item(60.0), item(60.0).depth(2).expandable(true)],
        );
        input.trash = false;
        let l = file_list_layout(&input);
        let first = &l.rows[0];
        assert!(first.triangle.is_none());
        assert!(close(first.icon.origin.x, FILE_PAD_H));
        let child = &l.rows[1];
        let tri = child.triangle.unwrap();
        assert!(close(tri.size.width, FILE_TRI_W));
        assert!(close(
            child.icon.origin.x,
            FILE_PAD_H + 2.0 * FILE_INDENT + FILE_TRI_W + FILE_GAP
        ));
        assert!(close(child.name.origin.x, child.icon.right() + FILE_GAP));
    }

    #[test]
    fn sidebar_groups_and_rows_stack_at_fixed_heights() {
        let entries = vec![
            SidebarEntry::group(),
            SidebarEntry::row(),
            SidebarEntry::row(),
        ];
        let l = sidebar_layout(&SidebarInput::new(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            entries,
        ));
        assert_eq!(l.rows.len(), 3);
        assert!(close(l.rows[0].rect.origin.y, 4.0));
        assert!(close(l.rows[0].rect.size.height, SIDEBAR_HEADER_H));
        assert!(close(l.rows[1].rect.origin.y, 36.0));
        assert!(close(l.rows[1].rect.size.height, SIDEBAR_ROW_H));
        assert!(close(l.content_height, 4.0 + 32.0 + 32.0 + 32.0));
        assert!(close(l.width, 200.0));
        // 分组标题不可点。
        assert_eq!(l.row_at(Point::new(100.0, 10.0)), None);
        assert_eq!(l.row_at(Point::new(100.0, 45.0)), Some(1));
    }

    #[test]
    fn sidebar_indent_moves_icon_and_text_not_the_row() {
        let base = SidebarInput::new(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            vec![SidebarEntry::row().indent(1)],
        );
        let l = sidebar_layout(&base);
        let row = &l.rows[0];
        assert!(close(row.indent, 16.0));
        assert!(close(row.icon.unwrap().origin.x, SIDEBAR_PAD_L + 16.0));
        assert!(close(
            row.text.origin.x,
            SIDEBAR_PAD_L + 16.0 + SIDEBAR_ICON + SIDEBAR_GAP
        ));
        assert!(close(row.rect.origin.x, 0.0), "行底仍从栏左缘起");
    }

    #[test]
    fn sidebar_selected_bar_sits_inside_the_row() {
        let mut input = SidebarInput::new(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            vec![
                SidebarEntry::group(),
                SidebarEntry::row(),
                SidebarEntry::row(),
            ],
        );
        input.selected = Some(2);
        let l = sidebar_layout(&input);
        let bar = l.selected_bar.unwrap();
        assert!(close(bar.origin.x, 0.0));
        assert!(close(bar.origin.y, 4.0 + 32.0 + 32.0 + SIDEBAR_SEL_BAR_TOP));
        assert!(close(bar.size.width, SIDEBAR_SEL_BAR_W));
        assert!(l.rows[2].selected);
        // 选中分组头不画条。
        input.selected = Some(0);
        assert!(sidebar_layout(&input).selected_bar.is_none());
    }

    #[test]
    fn sidebar_eject_hit_sits_at_right_edge() {
        let l = sidebar_layout(&SidebarInput::new(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            vec![SidebarEntry::row().ejectable(true)],
        ));
        let e = l.rows[0].eject.unwrap();
        assert!(close(
            e.origin.x,
            200.0 - SIDEBAR_EJECT_PAD_R - SIDEBAR_EJECT_W
        ));
        assert!(close(e.size.width, SIDEBAR_EJECT_W));
        assert!(close(l.rows[0].text.right(), e.origin.x - SIDEBAR_GAP));
    }

    #[test]
    fn sidebar_collapsed_has_no_rows_and_zero_width() {
        let mut input = SidebarInput::new(
            Rect::new(0.0, 0.0, 200.0, 300.0),
            vec![SidebarEntry::group(), SidebarEntry::row()],
        );
        input.collapsed = true;
        input.selected = Some(1);
        let l = sidebar_layout(&input);
        assert!(l.collapsed);
        assert!(l.rows.is_empty());
        assert_eq!(l.content_height, 0.0);
        assert_eq!(l.width, 0.0);
        assert!(l.selected_bar.is_none());
    }

    #[test]
    fn path_bar_natural_segments_keep_status_clear() {
        let mut input = PathBarInput::new(
            Rect::new(0.0, 0.0, 600.0, PATH_BAR_H),
            vec![
                PathBarSegment::new("home", 80.0),
                PathBarSegment::new("user", 80.0),
                PathBarSegment::new("docs", 80.0),
            ],
        );
        input.icon = true;
        input.status_width = 100.0;
        let l = path_bar_layout(&input);
        assert_eq!(l.segments.len(), 3);
        assert!(!l.overflow);
        assert_eq!(
            l.segments.iter().filter(|s| s.separator.is_some()).count(),
            2
        );
        let last = l.segments.last().unwrap();
        assert!(last.rect.right() <= l.status.unwrap().origin.x - PATH_BAR_GAP);
        assert!(close(
            l.status.unwrap().origin.x,
            600.0 - PATH_BAR_PAD_H - 100.0
        ));
        assert!(close(l.icon.unwrap().origin.x, PATH_BAR_PAD_H));
    }

    #[test]
    fn path_bar_drops_leading_segments_into_ellipsis() {
        let input = PathBarInput::new(
            Rect::new(0.0, 0.0, 120.0, PATH_BAR_H),
            vec![
                PathBarSegment::new("/", 80.0),
                PathBarSegment::new("home", 80.0),
                PathBarSegment::new("docs", 80.0),
            ],
        );
        let l = path_bar_layout(&input);
        assert!(l.overflow);
        assert!(l.ellipsis.is_some(), "前导段应折成省略号");
        assert_eq!(l.segments.len(), 2);
        assert_eq!(l.segments[0].index, 1);
        // 当前目录优先拿到余量：最后一段 >= 前一段。
        assert!(l.segments[1].rect.size.width >= l.segments[0].rect.size.width);
        // 段都在栏内，且省略号在首段左侧。
        assert!(l.segments[0].rect.origin.x > l.ellipsis.unwrap().origin.x);
        assert!(l.segments.last().unwrap().rect.right() <= 120.0 - PATH_BAR_PAD_H + 0.01);
    }

    #[test]
    fn path_bar_single_overwide_segment_is_compressed() {
        let input = PathBarInput::new(
            Rect::new(0.0, 0.0, 200.0, PATH_BAR_H),
            vec![PathBarSegment::new("a-very-long-directory-name", 500.0)],
        );
        let l = path_bar_layout(&input);
        assert_eq!(l.segments.len(), 1);
        assert!(l.segments[0].elided);
        assert!(l.segments[0].rect.size.width < 500.0);
        assert!(close(
            l.segments[0].rect.size.width,
            200.0 - 2.0 * PATH_BAR_PAD_H
        ));
        assert!(l.overflow);
    }

    #[test]
    fn path_bar_zero_segments_keeps_status_only() {
        let mut input = PathBarInput::new(Rect::new(0.0, 0.0, 400.0, PATH_BAR_H), Vec::new());
        input.status_width = 120.0;
        let l = path_bar_layout(&input);
        assert!(l.segments.is_empty());
        assert!(l.ellipsis.is_none());
        assert!(!l.overflow);
        assert!(close(
            l.status.unwrap().origin.x,
            400.0 - PATH_BAR_PAD_H - 120.0
        ));
        assert!(l.segment_at(Point::new(200.0, 10.0)).is_none());
    }
}
