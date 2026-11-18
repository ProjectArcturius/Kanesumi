// tile_wall.rs —— 磁贴墙的纯几何提炼。参 docs/supply/TILE_WALL_FILE_VIEWS.md。
//
// 提炼自 launcher 的 `shell/tile_wall.rs`（`geometry` / `compute_map` / `realize_span` /
// `max_scroll`）与 `shell/tiles.rs`（组内宏单元装箱）。输入分组、磁贴度量、视口矩形与
// 滚动偏移；输出**可见**磁贴的矩形 / 命中区、视口裁剪结果与横向翻页边界。
//
// **只算几何**：不画、不读主题、不吃输入事件、不持交互状态。分组策略由调用方给
// （组名 + 磁贴序列），控件不复制 launcher 的 `tiles.toml` 业务枚举。
//
// 形状冻结（参 Kanesumi docs/API_STABILITY.md §三）：现有字段不改名、不改语义；新增
// 字段必须带 `Default`，枚举只许加变体（`TileWallSize` 已标 `#[non_exhaustive]`）。

use kanesumi_core::metrics::{SPACE_32, SPACE_8};
use kanesumi_core::{Point, Rect};

/// 组宽（1×1 单元列数）：12 = 6 个标准方 = Win8.1 实测组宽 950。
/// 与 launcher `shell/tiles.rs` 的 `GROUP_COLS` 同值；调用方可按度量表覆盖。
pub const TILE_COLUMNS: usize = 12;

/// 组块至少占的单元行数：6 = 3 行标准方（磁贴少时组块不缩，参 `tile_metrics::STD_ROWS`）。
pub const TILE_MIN_ROWS: usize = 6;

/// 兼容组头文字矩形高（逻辑像素），对应应用 `tile_wall.rs::TITLE_H`。
///
/// 40 = `SPACE_32 + SPACE_8`：引用既有间距令牌，避免在尺寸表外另立裸常量
/// （`kanesumi-controls/tests/size_discipline.rs` 守门）。
pub const TILE_TITLE_H: f32 = SPACE_32 + SPACE_8;

/// 兼容组头与首行磁贴的固定间距，对应应用 `tile_wall.rs::TITLE_GAP`。
pub const TILE_TITLE_GAP: f32 = SPACE_8;

/// 磁贴尺寸档：只表达跨几个 1×1 单元，不携带任何应用状态。
///
/// `Small` = 1×1、`Wide` = 2×1、`Large` = 2×2、`ExtraLarge` = 4×2。应用
/// `TileSize::Mini` → `Small`、`Standard` → `Large`、`Large` → `ExtraLarge`。
/// 2×1 是库内旧档，应用没有对应尺寸；旧三档含义与缺省循环不变。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TileWallSize {
    /// 1×1。
    Small,
    /// 2×1（横跨两列、一行）。
    Wide,
    /// 2×2。
    Large,
    /// 4×2（横跨四列、两行）。对应应用 `TileSize::Large`；不是把旧 `Wide` 改义。
    ExtraLarge,
}

impl TileWallSize {
    /// 跨单元数 `(列, 行)`。
    pub const fn units(self) -> (usize, usize) {
        match self {
            TileWallSize::Small => (1, 1),
            TileWallSize::Wide => (2, 1),
            TileWallSize::Large => (2, 2),
            TileWallSize::ExtraLarge => (4, 2),
        }
    }
}

/// 一枚待布局磁贴。`id` 只随命中结果回传，控件不解析图标 / 标题。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileWallTile {
    pub id: String,
    pub size: TileWallSize,
}

impl TileWallTile {
    pub fn new(id: impl Into<String>, size: TileWallSize) -> Self {
        Self {
            id: id.into(),
            size,
        }
    }
}

/// 一个磁贴组：组名 + 磁贴序列（顺序即组内放置顺序）。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TileWallGroup {
    pub title: String,
    pub tiles: Vec<TileWallTile>,
}

impl TileWallGroup {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            tiles: Vec::new(),
        }
    }

    pub fn with_tiles(mut self, tiles: Vec<TileWallTile>) -> Self {
        self.tiles = tiles;
        self
    }
}

/// 磁贴墙度量（逻辑像素）。缺省值逐项照抄 launcher 的 Windows 8.1 实测表
/// （`launcher/src/shell/tile_metrics.rs`，出处 docs/research/n1108_tiles/REPORT.md §2）。
///
/// `columns` 必须是偶数（宏单元 = 2×2）；奇数按向下取偶归一化，0 / 1 按 2 处理。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileWallMetrics {
    /// 1×1 单元边长。
    pub cell: f32,
    /// 相邻单元间隙。
    pub gap: f32,
    /// 组间距。
    pub group_gap: f32,
    /// 组标题带高（组块顶 → 首行磁贴）。
    pub title_band: f32,
    /// 首列左缘 / 末列右缘留白。
    pub margin_x: f32,
    /// 每组 1×1 单元列数（偶数）。
    pub columns: usize,
    /// 组块至少占的单元行数。
    pub min_rows: usize,
    /// 底部让位带：组块在「视口 + 此带」里垂直居中（launcher 的「所有应用」按钮带）。
    pub bottom_band: f32,
}

impl TileWallMetrics {
    /// 1× 基准档（1366×768 实测值）。
    pub const BASE: Self = Self {
        cell: 70.0,
        gap: 10.0,
        group_gap: 70.0,
        title_band: 72.0,
        margin_x: 120.0,
        columns: TILE_COLUMNS,
        min_rows: TILE_MIN_ROWS,
        bottom_band: 48.0,
    };

    /// 归一化：非法长度归零，列数取偶数且至少 2。
    pub fn normalized(self) -> Self {
        let len = |v: f32| if v.is_finite() && v > 0.0 { v } else { 0.0 };
        Self {
            cell: len(self.cell),
            gap: len(self.gap),
            group_gap: len(self.group_gap),
            title_band: len(self.title_band),
            margin_x: len(self.margin_x),
            columns: (self.columns.max(2)) & !1,
            min_rows: self.min_rows.max(1),
            bottom_band: len(self.bottom_band),
        }
    }

    /// 从应用响应式度量构造：参数逐项对应 `launcher/src/shell/tile_metrics.rs` 的
    /// `Metrics`（`cell` / `gap` / `group_gap` / `title_band` / `margin_x`）。
    ///
    /// 只搬运字段，不复制 `metrics_for` 的屏高缩放算术 —— 那是应用侧策略，控件只消费结果。
    /// 列数、最小行数、底部让位带应用不掌握，仍取 [`TileWallMetrics::BASE`]。
    pub fn from_provider(
        cell: f32,
        gap: f32,
        group_gap: f32,
        title_band: f32,
        margin_x: f32,
    ) -> Self {
        Self {
            cell,
            gap,
            group_gap,
            title_band,
            margin_x,
            ..Self::BASE
        }
    }
}

impl Default for TileWallMetrics {
    fn default() -> Self {
        Self::BASE
    }
}

/// 布局输入：分组 + 度量 + 视口 + 滚动偏移。
///
/// `scroll` 会在布局里夹到 `[0, max_scroll]` 后回传；视口为空时一切可见集合为空。
#[derive(Debug, Clone, PartialEq)]
pub struct TileWallInput {
    pub groups: Vec<TileWallGroup>,
    pub viewport: Rect,
    pub scroll: f32,
    pub metrics: TileWallMetrics,
}

impl TileWallInput {
    pub fn new(groups: Vec<TileWallGroup>, viewport: Rect) -> Self {
        Self {
            groups,
            viewport,
            scroll: 0.0,
            metrics: TileWallMetrics::BASE,
        }
    }
}

/// 一枚**可见**磁贴：完整矩形 + 视口裁剪 + 命中区。
///
/// `rect` 是布局矩形（可能一部分在视口外）；`clip` / `hit` 是它与视口的交集。
/// `hit` 单独留一个字段（而不是直接复用 `clip`）是为了将来命中区与绘制区分离
/// （例如给触控放大 2 pt）时不改结构。
#[derive(Debug, Clone, PartialEq)]
pub struct TileWallSlot {
    /// 所属组（输入下标）。
    pub group: usize,
    /// 组内下标（输入下标）。
    pub index: usize,
    pub id: String,
    pub size: TileWallSize,
    /// 完整磁贴矩形（绝对坐标）。
    pub rect: Rect,
    /// 视口裁剪后的绘制矩形。
    pub clip: Rect,
    /// 命中区（当前 = `clip`）。
    pub hit: Rect,
    /// 是否被视口切掉了一部分。
    pub partial: bool,
}

/// 一个可见组的几何：组块 + 标题带。
#[derive(Debug, Clone, PartialEq)]
pub struct TileWallGroupLayout {
    /// 输入组下标。
    pub index: usize,
    pub title: String,
    /// 组块矩形（标题带 + 网格；高度按全墙最大行数统一）。
    pub rect: Rect,
    /// 标题带矩形（组块顶部 `title_band` 高）。
    pub title_rect: Rect,
    /// 与视口的交集。
    pub clip: Rect,
}

/// 磁贴墙一次布局的产物。
#[derive(Debug, Clone, PartialEq)]
pub struct TileWallLayout {
    /// 可见磁贴（[`layout_with_overscan`] 开启时另含视口外预取的列），按
    /// (组, 组内下标) 有序；视口外预取的槽位 `clip` / `hit` 为空矩形。
    pub slots: Vec<TileWallSlot>,
    /// 可见组（预取开启时另含视口外两侧的组；拖动基准为全量组）。
    pub groups: Vec<TileWallGroupLayout>,
    /// 全墙内容宽（不含左右留白）。
    pub content_width: f32,
    /// 滚动上界：`content_width + 2 × margin_x − 视口宽`，不小于 0。
    pub max_scroll: f32,
    /// 夹紧后的滚动偏移。
    pub scroll: f32,
    /// 翻页步长 = 视口宽。
    pub page_width: f32,
    /// 翻页边界（每页的滚动偏移，首项 0，末项 = `max_scroll`；无溢出时只有 0）。
    pub pages: Vec<f32>,
    /// 组块宽 / 高（所有组同高，与 launcher 的组块居中一致）。
    pub group_width: f32,
    pub group_height: f32,
    /// 全墙统一的行数（1×1 单元行，已取 `min_rows` 下限）。
    pub rows: usize,
    /// 本帧视口。
    pub viewport: Rect,
}

impl TileWallLayout {
    /// 命中可见磁贴：只看裁剪后的命中区，指针在视口外必然落空。
    pub fn hit(&self, pos: Point) -> Option<&TileWallSlot> {
        if !self.viewport.contains(pos) {
            return None;
        }
        self.slots.iter().find(|s| s.hit.contains(pos))
    }

    /// 滚动偏移落在第几页（0 基）。无溢出时恒为 0。
    pub fn page_of(&self, scroll: f32) -> usize {
        if self.page_width <= 0.0 || self.pages.len() <= 1 {
            return 0;
        }
        let s = if scroll.is_finite() {
            scroll.max(0.0)
        } else {
            0.0
        };
        // 按边界表定位：末页可能不足一整页（如 3250 落在第 5 段），不能用
        // `floor(scroll / page_width)` —— 那会把最后一段算回上一页。
        self.pages.iter().rposition(|p| *p <= s + 0.01).unwrap_or(0)
    }

    /// 最后一页下标。
    pub fn last_page(&self) -> usize {
        self.pages.len().saturating_sub(1)
    }
}

/// 组内放置结果：`(列, 行, 跨列, 跨行)`，单位 = 1×1 单元。
type Placement = (usize, usize, usize, usize);

/// 布局的可见性策略。
#[derive(Clone, Copy)]
enum Visibility {
    /// 只回与视口相交的组与磁贴（旧行为，缺省）。
    Viewport,
    /// 额外回视口左右各 `n` 个组列的几何（列 = 组块宽 + 组间距）。
    Overscan(usize),
    /// 全量几何：不按视口过滤，拖动基准用（落点判定要全部组矩形）。
    All,
}

/// 由分组 + 度量 + 视口 + 滚动偏移算出一次布局。纯函数，无副作用。
///
/// 放置规则与 launcher 的宏单元装箱一致（参 `launcher/src/shell/tiles.rs::place_cells`）：
/// 组宽固定 `columns` 个单元，以 2×2 宏单元行优先扫描；1×1 填宏单元内的四个子格，
/// 2×2 占一个整宏单元，2×1 占宏单元的一整行（先上后下，尽量填洞）。
pub fn layout(input: &TileWallInput) -> TileWallLayout {
    layout_with_visibility(input, Visibility::Viewport)
}

/// 视口外预取：左右各多回 `cols` 个**组列**的几何，供列图层虚拟化预渲染。
///
/// 1 列 = 一个组列步距（`group_width + group_gap`）。launcher 的 `PREFETCH_COLS = 2`
/// 是 2 个宏单元列（≈ 320 px @cell70，参 Ether `docs/research/n1109_tiles/REPORT.md`
/// §3.1）；本卡要求「12 组 @1366 回 ≥ 3 组」，320 px 的窗口只装得下 2 组，故这里按
/// 组列计步距（`cols = 2` 回 4 组、`cols = 1` 回 3 组）。步距大的代价是替更多列付
/// 图层缓冲，调用方可用更小的 `cols` 换。
/// `cols = 0` 与 [`layout`] 逐字段相同；预取槽位 `clip` / `hit` 为空矩形，不会命中。
pub fn layout_with_overscan(input: &TileWallInput, cols: usize) -> TileWallLayout {
    if cols == 0 {
        return layout(input);
    }
    layout_with_visibility(input, Visibility::Overscan(cols))
}

/// 拖动基准布局：摘除 `(group, index)` 那一枚后的**全量**布局（不按视口过滤）。
///
/// 对照 launcher `shell/tiles.rs::drop_target` 的基准几何：落点判定要所有组矩形，
/// 视口外的组与磁贴也得在场。组索引与输入一致 —— 摘空的组保留，删组是应用策略，
/// 控件模型没有「空组即删」的约定（这也是与 launcher `remove_tile` 的唯一差别）。
pub fn layout_without(input: &TileWallInput, group: usize, index: usize) -> TileWallLayout {
    let mut trimmed = input.clone();
    if let Some(g) = trimmed.groups.get_mut(group)
        && index < g.tiles.len()
    {
        g.tiles.remove(index);
    }
    layout_with_visibility(&trimmed, Visibility::All)
}

fn layout_with_visibility(input: &TileWallInput, vis: Visibility) -> TileWallLayout {
    let m = input.metrics.normalized();
    let viewport = input.viewport.normalized();
    let placements: Vec<Vec<Placement>> = input
        .groups
        .iter()
        .map(|g| place_group(&g.tiles, m.columns))
        .collect();

    let content_rows = placements
        .iter()
        .map(|p| p.iter().map(|(_, r, _, rs)| *r + *rs).max().unwrap_or(0))
        .max()
        .unwrap_or(0);
    let rows = content_rows.max(m.min_rows);
    let group_width = m.columns as f32 * m.cell + (m.columns.saturating_sub(1)) as f32 * m.gap;
    let grid_h = rows as f32 * m.cell + (rows.saturating_sub(1)) as f32 * m.gap;
    let group_height = m.title_band + grid_h;
    let n = input.groups.len();
    let content_width = if n == 0 {
        0.0
    } else {
        n as f32 * group_width + (n - 1) as f32 * m.group_gap
    };

    // 首列固定贴左缘 + 留白（不随内容居中）；末组滚到右缘留白为止。
    // 视口为空时没有可滚的参照，上界归零，避免「空视口也能翻页」。
    let max_scroll = if n == 0 || viewport.is_empty() {
        0.0
    } else {
        (content_width + 2.0 * m.margin_x - viewport.size.width).max(0.0)
    };
    let scroll = if input.scroll.is_finite() {
        input.scroll.clamp(0.0, max_scroll)
    } else {
        0.0
    };

    let full_h = viewport.size.height + m.bottom_band;
    let y0 = viewport.origin.y + (full_h - group_height) / 2.0;
    let grid_y = y0 + m.title_band;
    let x0 = viewport.origin.x + m.margin_x - scroll;

    // 预取把视口按组列步距向左右扩；全量策略不参与过滤。`Viewport` 分支与旧代码
    // 同一条判据（`rect.intersect(viewport)`），保证未开启时输出逐字段不变。
    let visible = match vis {
        Visibility::Viewport | Visibility::All => viewport,
        Visibility::Overscan(pad_cols) => {
            let pad = pad_cols as f32 * (group_width + m.group_gap);
            Rect::new(
                viewport.origin.x - pad,
                viewport.origin.y,
                viewport.size.width + 2.0 * pad,
                viewport.size.height,
            )
        }
    };
    let include = |r: Rect| match vis {
        Visibility::All => true,
        _ => r.intersect(visible).is_some(),
    };
    // 裁剪与命中始终只认视口：预取回来的视口外几何不参与命中，绘制矩形为空。
    let clip_of = |r: Rect| {
        r.intersect(viewport)
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))
    };

    let mut slots = Vec::new();
    let mut groups = Vec::new();
    for (g, group) in input.groups.iter().enumerate() {
        let gx = x0 + g as f32 * (group_width + m.group_gap);
        let group_rect = Rect::new(gx, y0, group_width, group_height);
        if include(group_rect) {
            groups.push(TileWallGroupLayout {
                index: g,
                title: group.title.clone(),
                rect: group_rect,
                title_rect: Rect::new(gx, y0, group_width, m.title_band),
                clip: clip_of(group_rect),
            });
        }
        for (t, (col, row, cols, col_rows)) in placements[g].iter().enumerate() {
            let rect = Rect::new(
                gx + *col as f32 * (m.cell + m.gap),
                grid_y + *row as f32 * (m.cell + m.gap),
                *cols as f32 * m.cell + cols.saturating_sub(1) as f32 * m.gap,
                *col_rows as f32 * m.cell + col_rows.saturating_sub(1) as f32 * m.gap,
            );
            if !include(rect) {
                continue;
            }
            let clip = clip_of(rect);
            let tile = &group.tiles[t];
            slots.push(TileWallSlot {
                group: g,
                index: t,
                id: tile.id.clone(),
                size: tile.size,
                rect,
                clip,
                hit: clip,
                // 完全在视口外的预取槽位不是「被切掉一部分」。
                partial: !clip.is_empty() && clip != rect,
            });
        }
    }

    TileWallLayout {
        slots,
        groups,
        content_width,
        max_scroll,
        scroll,
        page_width: viewport.size.width,
        pages: page_offsets(viewport.size.width, max_scroll),
        group_width,
        group_height,
        rows,
        viewport,
    }
}

/// 横向翻页边界：每页的滚动偏移。末页贴 `max_scroll`，不越界。
fn page_offsets(page_width: f32, max_scroll: f32) -> Vec<f32> {
    if max_scroll <= 0.0 || page_width <= 0.0 {
        return vec![0.0];
    }
    let count = (max_scroll / page_width).ceil() as usize;
    let mut out = Vec::with_capacity(count + 1);
    out.push(0.0);
    for i in 1..=count {
        let v = (i as f32 * page_width).min(max_scroll);
        if out.last().copied() != Some(v) {
            out.push(v);
        }
    }
    out
}

/// 组内装箱：宏单元（2×2）行优先，1×1 先填宏单元子格。
///
/// 返回的坐标是 1×1 单元。组宽固定，行数按内容生长（不截断、不丢弃磁贴）。
fn place_group(tiles: &[TileWallTile], columns: usize) -> Vec<Placement> {
    let cols = columns.max(2) & !1;
    let macro_cols = cols / 2;
    // 所有磁贴面积之和是所需行数的上界；多留 8 行给「先占洞后补」的散块。
    let total_units: usize = tiles
        .iter()
        .map(|t| {
            let (c, r) = t.size.units();
            c * r
        })
        .sum();
    let max_rows = (total_units / cols).max(1) + 8;
    let mut occ: Vec<Vec<bool>> = Vec::new();
    let mut out = Vec::with_capacity(tiles.len());
    for tile in tiles {
        let (w, h) = tile.size.units();
        let (x, y) = find_slot(&occ, cols, macro_cols, w, h, max_rows);
        occupy(&mut occ, x, y, w, h, cols);
        out.push((x, y, w, h));
    }
    out
}

/// 扫描首个可放位置。1×1 / 2×1 / 2×2 按 launcher 的宏单元顺序；其它尺寸走任意首适配。
fn find_slot(
    occ: &[Vec<bool>],
    cols: usize,
    macro_cols: usize,
    w: usize,
    h: usize,
    max_rows: usize,
) -> (usize, usize) {
    for mr in 0..max_rows {
        for mc in 0..macro_cols {
            let x = mc * 2;
            let y = mr * 2;
            let found = match (w, h) {
                (1, 1) => [(0, 0), (1, 0), (0, 1), (1, 1)]
                    .into_iter()
                    .find(|(dx, dy)| is_free(occ, x + dx, y + dy)),
                (2, 1) => {
                    if is_free(occ, x, y) && is_free(occ, x + 1, y) {
                        Some((0, 0))
                    } else if is_free(occ, x, y + 1) && is_free(occ, x + 1, y + 1) {
                        Some((0, 1))
                    } else {
                        None
                    }
                }
                (2, 2) => {
                    let free = (0..2).all(|dy| (0..2).all(|dx| is_free(occ, x + dx, y + dy)));
                    free.then_some((0, 0))
                }
                // 应用 `find_large`：4×2 必须占横向相邻两个完整宏单元，起点偶数。
                (4, 2) => {
                    let free = mc + 1 < macro_cols
                        && (0..2).all(|dy| (0..4).all(|dx| is_free(occ, x + dx, y + dy)));
                    free.then_some((0, 0))
                }
                _ => first_fit(occ, cols, y, w, h),
            };
            if let Some((dx, dy)) = found {
                return (x + dx, y + dy);
            }
        }
    }
    // 面积上界算错也不丢磁贴：落到已占用行之下，宁可组变高，也不让内容凭空消失。
    (0, occ.len())
}

/// 任意 `w×h` 在横扫一行里的首个空位（仅未来新增尺寸档的兜底）。
fn first_fit(
    occ: &[Vec<bool>],
    cols: usize,
    row: usize,
    w: usize,
    h: usize,
) -> Option<(usize, usize)> {
    if w > cols || h == 0 || w == 0 {
        return None;
    }
    for x in 0..=(cols - w) {
        if (0..h).all(|dy| (0..w).all(|dx| is_free(occ, x + dx, row + dy))) {
            return Some((x, row));
        }
    }
    None
}

/// 单元格是否空闲（未分配的行视为空闲）。
fn is_free(occ: &[Vec<bool>], x: usize, y: usize) -> bool {
    occ.get(y).and_then(|r| r.get(x)).is_none_or(|v| !*v)
}

/// 标记占用并自动增高行表。
fn occupy(occ: &mut Vec<Vec<bool>>, x: usize, y: usize, w: usize, h: usize, cols: usize) {
    let need = (y + h).max(occ.len());
    while occ.len() < need {
        occ.push(vec![false; cols]);
    }
    for dy in 0..h {
        for dx in 0..w {
            if let Some(c) = occ.get_mut(y + dy).and_then(|r| r.get_mut(x + dx)) {
                *c = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(id: &str, size: TileWallSize) -> TileWallTile {
        TileWallTile::new(id, size)
    }

    fn one_group(tiles: Vec<TileWallTile>) -> TileWallGroup {
        TileWallGroup::new("组").with_tiles(tiles)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn empty_wall_has_no_slots_and_one_page() {
        let l = layout(&TileWallInput::new(
            vec![],
            Rect::new(0.0, 0.0, 1000.0, 700.0),
        ));
        assert!(l.slots.is_empty());
        assert!(l.groups.is_empty());
        assert_eq!(l.content_width, 0.0);
        assert_eq!(l.max_scroll, 0.0);
        assert_eq!(l.pages, vec![0.0]);
        assert_eq!(l.rows, TILE_MIN_ROWS);
        assert_eq!(l.last_page(), 0);
    }

    #[test]
    fn empty_group_keeps_title_band_and_min_rows() {
        let l = layout(&TileWallInput::new(
            vec![one_group(vec![])],
            Rect::new(0.0, 0.0, 1000.0, 700.0),
        ));
        assert!(l.slots.is_empty(), "空组不该凭空长出磁贴");
        assert_eq!(l.groups.len(), 1);
        let m = TileWallMetrics::BASE;
        let expect_h = m.title_band + m.min_rows as f32 * m.cell + (m.min_rows - 1) as f32 * m.gap;
        assert!(close(l.group_height, expect_h));
        assert!(close(l.groups[0].title_rect.size.height, m.title_band));
    }

    #[test]
    fn wide_and_large_tiles_span_cells() {
        let l = layout(&TileWallInput::new(
            vec![one_group(vec![
                tile("wide", TileWallSize::Wide),
                tile("large", TileWallSize::Large),
                tile("small", TileWallSize::Small),
            ])],
            Rect::new(0.0, 0.0, 2000.0, 800.0),
        ));
        let m = TileWallMetrics::BASE;
        let group_x = m.margin_x;
        let wide = l.slots.iter().find(|s| s.id == "wide").unwrap();
        let large = l.slots.iter().find(|s| s.id == "large").unwrap();
        let small = l.slots.iter().find(|s| s.id == "small").unwrap();
        // 2×1：横跨两列、一行。
        assert!(close(wide.rect.size.width, 2.0 * m.cell + m.gap));
        assert!(close(wide.rect.size.height, m.cell));
        // 2×2：占一个整宏单元（2×2 列之后）。
        assert!(close(large.rect.size.width, 2.0 * m.cell + m.gap));
        assert!(close(large.rect.size.height, 2.0 * m.cell + m.gap));
        assert!(close(large.rect.origin.x, group_x + 2.0 * (m.cell + m.gap)));
        // 1×1 填进首个宏单元剩下的左下子格，不与大磁贴重叠。
        assert!(close(small.rect.size.width, m.cell));
        assert!(close(small.rect.origin.x, group_x));
        assert!(close(
            small.rect.origin.y,
            wide.rect.origin.y + m.cell + m.gap
        ));
        for a in 0..l.slots.len() {
            for b in a + 1..l.slots.len() {
                let (ra, rb) = (l.slots[a].rect, l.slots[b].rect);
                assert!(
                    ra.intersect(rb).is_none(),
                    "磁贴 {} 与 {} 重叠",
                    l.slots[a].id,
                    l.slots[b].id
                );
            }
        }
    }

    #[test]
    fn compat_title_constants_match_provider() {
        assert_eq!(TILE_TITLE_H, 40.0, "应用 TITLE_H");
        assert_eq!(TILE_TITLE_GAP, 8.0, "应用 TITLE_GAP");
    }

    #[test]
    fn extra_large_tiles_take_four_by_two_macro_slots() {
        let l = layout(&TileWallInput::new(
            vec![one_group(vec![
                tile("a", TileWallSize::ExtraLarge),
                tile("b", TileWallSize::ExtraLarge),
                tile("c", TileWallSize::ExtraLarge),
            ])],
            Rect::new(0.0, 0.0, 2000.0, 800.0),
        ));
        let m = TileWallMetrics::BASE;
        let group_x = m.margin_x;
        let a = l.slots.iter().find(|s| s.id == "a").unwrap();
        let b = l.slots.iter().find(|s| s.id == "b").unwrap();
        let c = l.slots.iter().find(|s| s.id == "c").unwrap();
        // 4×2：宽 310、高 150，正好三枚铺满 950 宽的组。
        for s in [a, b, c] {
            assert!(close(s.rect.size.width, 4.0 * m.cell + 3.0 * m.gap));
            assert!(close(s.rect.size.height, 2.0 * m.cell + m.gap));
        }
        assert!(close(a.rect.origin.x, group_x));
        assert!(close(b.rect.origin.x, group_x + 4.0 * (m.cell + m.gap)));
        assert!(close(c.rect.origin.x, group_x + 8.0 * (m.cell + m.gap)));
        assert_eq!(l.rows, TILE_MIN_ROWS, "4×2 只占两行内容，组块仍保底 6 行");
        assert_eq!(
            TileWallSize::ExtraLarge.units(),
            (4, 2),
            "4×2 不得占用旧的 Wide/Large 含义"
        );
        assert_eq!(TileWallSize::Wide.units(), (2, 1));
        assert_eq!(TileWallSize::Large.units(), (2, 2));
    }

    #[test]
    fn large_tile_crosses_a_row_boundary() {
        let l = layout(&TileWallInput::new(
            vec![one_group(vec![tile("large", TileWallSize::Large)])],
            Rect::new(0.0, 0.0, 2000.0, 800.0),
        ));
        let m = TileWallMetrics::BASE;
        let large = &l.slots[0];
        let grid_top = large.rect.origin.y;
        // 150 高 = 两行单元 + 一道间隙，证明它跨行而不是被压进一行。
        assert!(close(large.rect.size.height, 2.0 * m.cell + m.gap));
        assert!(close(large.rect.bottom(), grid_top + 2.0 * m.cell + m.gap));
        assert!(l.rows >= 2);
    }

    #[test]
    fn viewport_cuts_half_a_row_of_tiles() {
        let groups = vec![one_group(vec![
            tile("a", TileWallSize::Wide),
            tile("b", TileWallSize::Wide),
            tile("c", TileWallSize::Wide),
        ])];
        // 先取完整视口下的几何，再把右缘挪到第三枚磁贴的中点。
        let full = layout(&TileWallInput::new(
            groups.clone(),
            Rect::new(0.0, 0.0, 2000.0, 700.0),
        ));
        let c = full.slots.iter().find(|s| s.id == "c").unwrap();
        let cut_x = c.rect.origin.x + c.rect.size.width / 2.0;
        let l = layout(&TileWallInput::new(
            groups,
            Rect::new(0.0, 0.0, cut_x, 700.0),
        ));
        let c2 = l.slots.iter().find(|s| s.id == "c").unwrap();
        assert!(c2.partial, "切掉一半的磁贴必须标 partial");
        assert!(close(c2.clip.size.width, c2.rect.size.width / 2.0));
        assert_eq!(c2.hit, c2.clip);
    }

    #[test]
    fn scroll_clamps_to_max_and_pages_end_at_max() {
        let groups = (0..4)
            .map(|g| one_group(vec![tile(&format!("t{g}"), TileWallSize::Small)]))
            .collect();
        let mut input = TileWallInput::new(groups, Rect::new(0.0, 0.0, 1000.0, 700.0));
        input.scroll = 99_999.0;
        let l = layout(&input);
        assert!(close(l.max_scroll, l.content_width + 240.0 - 1000.0));
        assert!(close(l.scroll, l.max_scroll));
        assert_eq!(l.pages.first().copied(), Some(0.0));
        assert!(close(*l.pages.last().unwrap(), l.max_scroll));
        assert_eq!(l.last_page(), l.pages.len() - 1);
        assert_eq!(l.page_of(l.max_scroll), l.last_page());
        // 负偏移夹回首页。
        input.scroll = -50.0;
        assert_eq!(layout(&input).scroll, 0.0);
    }

    #[test]
    fn hit_only_inside_viewport_and_clip() {
        let groups = vec![one_group(vec![
            tile("a", TileWallSize::Wide),
            tile("b", TileWallSize::Wide),
            tile("c", TileWallSize::Wide),
        ])];
        let full = layout(&TileWallInput::new(
            groups.clone(),
            Rect::new(0.0, 0.0, 2000.0, 700.0),
        ));
        let c = full.slots.iter().find(|s| s.id == "c").unwrap();
        let cut_x = c.rect.origin.x + c.rect.size.width / 2.0;
        let l = layout(&TileWallInput::new(
            groups,
            Rect::new(0.0, 0.0, cut_x, 700.0),
        ));
        let c2 = l.slots.iter().find(|s| s.id == "c").unwrap();
        let inside = Point::new(c2.clip.origin.x + 1.0, c2.clip.origin.y + 1.0);
        assert_eq!(l.hit(inside).map(|s| s.id.as_str()), Some("c"));
        // 在完整矩形里、但已被视口裁掉的那一半不算命中。
        let outside_clip = Point::new(c2.rect.right() - 1.0, c2.rect.origin.y + 1.0);
        assert!(l.hit(outside_clip).is_none());
        // 视口外。
        assert!(l.hit(Point::new(cut_x + 5.0, 10.0)).is_none());
    }

    #[test]
    fn thousand_tiles_only_report_visible_slots() {
        let groups = (0..10)
            .map(|g| {
                let tiles = (0..100)
                    .map(|t| tile(&format!("g{g}t{t}"), TileWallSize::Small))
                    .collect();
                TileWallGroup::new(format!("组{g}")).with_tiles(tiles)
            })
            .collect();
        let l = layout(&TileWallInput::new(
            groups,
            Rect::new(0.0, 0.0, 800.0, 600.0),
        ));
        assert!(!l.slots.is_empty());
        assert!(
            l.rows > TILE_MIN_ROWS,
            "每组 100 枚会撑高行数（组内溢出），实际 {} 行",
            l.rows
        );
        assert!(
            l.slots.len() < 100,
            "1000 枚里只该回可见的几十枚，实际 {}",
            l.slots.len()
        );
        for s in &l.slots {
            assert!(!s.clip.is_empty());
            assert!(s.clip.intersect(l.viewport).is_some());
        }
    }

    // ── 视口外预取（B3）与拖动基准（B1）的几何回归 ──────────────────────────

    /// C0 夹具：两组 18+2 枚 `Large`，视口 1280×752，提供方度量。
    fn c0_like_input() -> TileWallInput {
        let first = (1..=18)
            .map(|n| tile(&format!("app{n:02}"), TileWallSize::Large))
            .collect();
        let second = (19..=20)
            .map(|n| tile(&format!("app{n:02}"), TileWallSize::Large))
            .collect();
        let mut input = TileWallInput::new(
            vec![
                TileWallGroup::new("常用").with_tiles(first),
                TileWallGroup::new("").with_tiles(second),
            ],
            Rect::new(0.0, 0.0, 1280.0, 752.0),
        );
        input.metrics = TileWallMetrics::from_provider(70.0, 10.0, 70.0, 72.0, 112.4451);
        input
    }

    fn twelve_groups() -> Vec<TileWallGroup> {
        (0..12)
            .map(|g| {
                TileWallGroup::new(format!("组{g}"))
                    .with_tiles(vec![tile(&format!("g{g}"), TileWallSize::Small)])
            })
            .collect()
    }

    #[test]
    fn overscan_zero_is_field_by_field_identical() {
        let c0 = c0_like_input();
        assert_eq!(layout_with_overscan(&c0, 0), layout(&c0));

        let wide = TileWallInput::new(twelve_groups(), Rect::new(0.0, 0.0, 1366.0, 768.0));
        assert_eq!(layout_with_overscan(&wide, 0), layout(&wide));
    }

    #[test]
    fn overscan_returns_twentieth_tile_beyond_right_edge() {
        let input = c0_like_input();
        let base = layout(&input);
        assert_eq!(base.slots.len(), 19, "旧行为丢掉视口外的 app20");
        assert!(!base.slots.iter().any(|s| s.id == "app20"));

        let wide = layout_with_overscan(&input, 2);
        assert_eq!(wide.slots.len(), 20, "C0 的 20 枚应全部在场");
        let app20 = wide
            .slots
            .iter()
            .find(|s| s.id == "app20")
            .expect("app20 在预取列里");
        assert!(close(app20.rect.origin.x, 1292.4451), "app20 的几何不变");
        assert!(app20.clip.is_empty(), "视口外预取槽位没有裁剪矩形");
        assert!(app20.hit.is_empty());
        assert!(!app20.partial, "完全在视口外不算被切掉一部分");
        // 命中仍只认视口：预取回来的槽位不能被点到。
        assert!(wide.hit(Point::new(1300.0, 276.0)).is_none());
    }

    #[test]
    fn overscan_two_sides_include_whole_group_columns() {
        let input = TileWallInput::new(twelve_groups(), Rect::new(0.0, 0.0, 1366.0, 768.0));
        let base = layout(&input);
        assert_eq!(base.groups.len(), 2, "旧行为只回与视口相交的两组");

        let wide = layout_with_overscan(&input, 2);
        assert_eq!(
            wide.groups.len(),
            4,
            "两侧各 2 个组列：视口内 2 组 + 右侧 2 组（组 0..3）"
        );
        assert!(
            wide.groups
                .iter()
                .any(|g| g.rect.origin.x >= 1366.0 && g.clip.is_empty()),
            "右侧应有一组完全在视口外"
        );

        // 滚动到中段后，预取集合同时越过视口左右两侧。
        let mut scrolled = input.clone();
        scrolled.scroll = 500.0;
        let mid = layout_with_overscan(&scrolled, 2);
        assert!(mid.groups.iter().any(|g| g.rect.origin.x < 0.0), "左侧组列");
        assert!(
            mid.groups.iter().any(|g| g.rect.origin.x >= 1366.0),
            "右侧组列"
        );
    }

    #[test]
    fn layout_without_drops_source_tile_and_keeps_group_indices() {
        let input = c0_like_input();
        let base = layout_without(&input, 0, 0);
        assert_eq!(base.slots.len(), 19);
        assert_eq!(base.groups.len(), 2);
        assert_eq!(base.slots[0].id, "app02", "基准布局首格是被拖项的后一枚");
        assert!(
            close(base.slots[0].rect.origin.x, 112.4451),
            "摘除后被拖项的格子由后一枚补上"
        );
        assert_eq!(base.slots.iter().filter(|s| s.group == 0).count(), 17);
        let second = base
            .slots
            .iter()
            .find(|s| s.id == "app19")
            .expect("第二组仍在场");
        assert!(
            close(second.rect.origin.x, 1132.4451),
            "组索引不因摘除而漂移"
        );
        // 越界下标不 panic，也不改布局。
        assert_eq!(layout_without(&input, 9, 9).slots.len(), 20);
    }

    #[test]
    fn layout_without_returns_all_groups_for_drop_targets() {
        let input = TileWallInput::new(twelve_groups(), Rect::new(0.0, 0.0, 1366.0, 768.0));
        let base = layout_without(&input, 0, 0);
        assert_eq!(base.groups.len(), 12, "落点判定需要全部组矩形");
        assert!(base.groups.iter().skip(2).all(|g| g.clip.is_empty()));
        // 摘空的组保留（删组是应用策略）。
        assert!(base.slots.iter().all(|s| s.group != 0));
    }
}
