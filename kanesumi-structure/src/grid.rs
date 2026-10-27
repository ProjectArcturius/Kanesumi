// grid.rs —— 网格布局原语（MetroGrid + UniformGrid + TileWall）。
//
// 狗粮化缺口（早期计算器键盘区手算 rect 暴露；该应用 2026-10-04 已迁往 Ether apps/calculator）：
// 补均匀网格布局器。
// 应用：键盘（0 键跨 2 列）、磁贴墙（TILES_DESIGN §2/§3：1×1 / 2×2 / 4×2）、
// Settings 面板（MetroGrid，UWP Grid 复刻，参 CONTROL_SPEC §33）。

use kanesumi_core::{Rect, Size};

/// 网格轨道长度 —— UWP `RowDefinition`/`ColumnDefinition` 的三种尺寸。
/// - `Fixed(px)`：固定像素（如按钮高 32）；
/// - `Auto`：内容自适应，宿主量测后经 [`MetroGrid::resolve`] 传入；
/// - `Star(w)`：比例分配剩余空间（`*` 默认 1，`2*` = 权重 2）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GridLength {
    Fixed(f32),
    Auto,
    Star(f32),
}

impl GridLength {
    /// `*` 便捷构造（权重 1）。
    pub const fn star() -> Self {
        Self::Star(1.0)
    }

    /// `Auto` 便捷构造。
    pub const fn auto() -> Self {
        Self::Auto
    }
}

/// 子单元声明 —— 所在行列 + 跨度（UWP `Grid.Row/Column/RowSpan/ColumnSpan`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridChild {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
}

impl GridChild {
    /// 单格单元（row, col）。
    pub const fn at(row: usize, col: usize) -> Self {
        Self {
            row,
            col,
            row_span: 1,
            col_span: 1,
        }
    }

    /// 设跨度（行/列）。
    pub const fn with_span(mut self, row_span: usize, col_span: usize) -> Self {
        self.row_span = row_span;
        self.col_span = col_span;
        self
    }
}

/// 二维网格布局 —— UWP `Grid` 复刻。行/列定义 + 子单元（含跨度）。
///
/// **纯布局**：不持控件状态、不自绘（同 UniformGrid），`resolve` 只算轨道尺寸，
/// `child_rect` 由宿主据此放置内容。UWP Grid 无 gap（间距靠子元素 Margin），
/// Kanesumi 以 `gap`（row, col）扩展可选统一间距。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroGrid {
    /// 行定义（数量 = 行数）。
    pub rows: Vec<GridLength>,
    /// 列定义（数量 = 列数）。
    pub cols: Vec<GridLength>,
    /// 统一间距 (row_gap, col_gap)。UWP 无，Kanesumi 可选扩展。
    pub gap: (f32, f32),
}

impl MetroGrid {
    /// 构造。**空行/空列不再 panic**（2026-09-22 审计 P0-3）：无轨道 = 不布局任何子单元，
    /// `resolve` 返回空尺寸表、`child_rect` 夹紧到空矩形。退化输入由「绘制/命中为空」表达，
    /// 不由进程 panic 表达。
    pub fn new(rows: Vec<GridLength>, cols: Vec<GridLength>) -> Self {
        Self {
            rows,
            cols,
            gap: (0.0, 0.0),
        }
    }

    /// 设统一间距。
    pub fn with_gap(mut self, row_gap: f32, col_gap: f32) -> Self {
        self.gap = (row_gap, col_gap);
        self
    }

    /// 解析轨道尺寸。
    ///
    /// - Fixed 轨道占自身像素；Auto 轨道取 `auto_rows`/`auto_cols` 对应宿主量测值；
    /// - 剩余空间（rect − 固定 − auto − 间距）按 Star 权重比例分配；
    /// - 无 Star 轨道时剩余空间留空（UWP 行为）。
    ///
    /// 返回 `(row_heights, col_widths)`，长度与定义数一致。
    pub fn resolve(
        &self,
        rect: Rect,
        auto_rows: &[f32],
        auto_cols: &[f32],
    ) -> (Vec<f32>, Vec<f32>) {
        let mut row_heights = vec![0.0f32; self.rows.len()];
        let mut col_widths = vec![0.0f32; self.cols.len()];

        for (i, len) in self.rows.iter().enumerate() {
            match len {
                GridLength::Fixed(h) => row_heights[i] = *h,
                GridLength::Auto => row_heights[i] = auto_rows.get(i).copied().unwrap_or(0.0),
                GridLength::Star(_) => {}
            }
        }
        for (i, len) in self.cols.iter().enumerate() {
            match len {
                GridLength::Fixed(w) => col_widths[i] = *w,
                GridLength::Auto => col_widths[i] = auto_cols.get(i).copied().unwrap_or(0.0),
                GridLength::Star(_) => {}
            }
        }

        let gap_w = self.gap.0 * self.rows.len().saturating_sub(1) as f32;
        let gap_h = self.gap.1 * self.cols.len().saturating_sub(1) as f32;
        let used_w: f32 = col_widths.iter().sum();
        let used_h: f32 = row_heights.iter().sum();
        let star_w_total: f32 = self
            .cols
            .iter()
            .filter_map(|c| match c {
                GridLength::Star(s) => Some(*s),
                _ => None,
            })
            .sum();
        let star_h_total: f32 = self
            .rows
            .iter()
            .filter_map(|r| match r {
                GridLength::Star(s) => Some(*s),
                _ => None,
            })
            .sum();

        let avail_w = (rect.size.width - used_w - gap_h).max(0.0);
        let avail_h = (rect.size.height - used_h - gap_w).max(0.0);
        for (i, len) in self.cols.iter().enumerate() {
            if let GridLength::Star(s) = len {
                col_widths[i] = if star_w_total > 0.0 {
                    avail_w * s / star_w_total
                } else {
                    0.0
                };
            }
        }
        for (i, len) in self.rows.iter().enumerate() {
            if let GridLength::Star(s) = len {
                row_heights[i] = if star_h_total > 0.0 {
                    avail_h * s / star_h_total
                } else {
                    0.0
                };
            }
        }

        (row_heights, col_widths)
    }

    /// 子单元矩形。跨度合并多轨道 + 其间间距。
    ///
    /// 入参 `heights`/`widths` 来自 [`Self::resolve`]。索引一律夹到可用轨道范围 ——
    /// `rows`/`cols` 是 pub 字段，宿主可能给出与 `GridChild` 不匹配的定义（审计 P0-3）。
    pub fn child_rect(
        &self,
        rect: Rect,
        heights: &[f32],
        widths: &[f32],
        child: GridChild,
    ) -> Rect {
        let col = child.col.min(widths.len());
        let row = child.row.min(heights.len());
        let col_end = col.saturating_add(child.col_span).min(widths.len());
        let row_end = row.saturating_add(child.row_span).min(heights.len());

        let x0 = rect.origin.x + widths[..col].iter().sum::<f32>() + self.gap.1 * col as f32;
        let y0 = rect.origin.y + heights[..row].iter().sum::<f32>() + self.gap.0 * row as f32;
        let w = widths[col..col_end].iter().sum::<f32>()
            + self.gap.1 * col_end.saturating_sub(col).saturating_sub(1) as f32;
        let h = heights[row..row_end].iter().sum::<f32>()
            + self.gap.0 * row_end.saturating_sub(row).saturating_sub(1) as f32;
        Rect::new(x0, y0, w.max(0.0), h.max(0.0))
    }
}

/// 均匀网格 —— 等尺寸方形单元、行优先排布、支持跨单元（span）。
///
/// 单元尺寸由 rect + columns + gap 反推（方形）；`allocate(sw, sh)` 按行优先推进，
/// 当前行剩余列数不足时自动换行（对齐 UWP Grid 的 Auto 流式排布）。
/// 网格只管几何分配，不持控件状态 —— 纯数据、跨平台可测。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UniformGrid {
    rect: Rect,
    columns: usize,
    gap: f32,
    row: usize,
    col: usize,
}

impl UniformGrid {
    /// 在 `rect` 内以 `columns` 列 + `gap` 间隔建网格。单元为方形。
    pub fn new(rect: Rect, columns: usize, gap: f32) -> Self {
        assert!(columns > 0, "网格列数必须 > 0");
        Self {
            rect,
            columns,
            gap,
            row: 0,
            col: 0,
        }
    }

    /// 单元边长（方形）：(可用宽 − 总间隔) / 列数。
    pub fn cell_size(&self) -> f32 {
        let w = self.rect.size.width - self.gap * (self.columns as f32 - 1.0);
        (w / self.columns as f32).max(0.0)
    }

    /// 网格根矩形。
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// 光标位置（row, col）。
    pub fn cursor(&self) -> (usize, usize) {
        (self.row, self.col)
    }

    /// 下一格（跨 `span.0` 列 × `span.1` 行）。当前行剩余列数不足时换行。
    /// 返回格矩形；光标推进到 `col + span.0`。
    pub fn allocate(&mut self, span: (usize, usize)) -> Rect {
        let (sw, sh) = span;
        assert!(sw >= 1 && sh >= 1, "span 必须 ≥ 1×1");
        if self.col + sw > self.columns {
            self.row += 1;
            self.col = 0;
        }
        let cell = self.cell_size();
        let x = self.rect.origin.x + self.col as f32 * (cell + self.gap);
        let y = self.rect.origin.y + self.row as f32 * (cell + self.gap);
        let w = cell * sw as f32 + self.gap * (sw as f32 - 1.0);
        let h = cell * sh as f32 + self.gap * (sh as f32 - 1.0);
        self.col += sw;
        Rect::new(x, y, w, h)
    }

    /// 强制换行（跳至下一行行首）。
    pub fn new_line(&mut self) {
        self.row += 1;
        self.col = 0;
    }

    /// 已占用的行数（含当前行）。
    pub fn rows_used(&self) -> usize {
        self.row + 1
    }

    /// 网格内容尺寸（满列宽 × 已占行高）。
    pub fn content_size(&self) -> Size {
        let cell = self.cell_size();
        Size::new(
            cell * self.columns as f32 + self.gap * (self.columns as f32 - 1.0),
            cell * self.rows_used() as f32 + self.gap * self.row as f32,
        )
    }
}

/// 自适应等宽网格的求解结果：列数 + 等宽单元宽。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AdaptiveColumns {
    /// 列数（≥ 1，≤ 调用方给的上限）。
    pub columns: usize,
    /// 单元宽（等宽；可用宽不足一格时 = 可用宽）。
    pub cell_width: f32,
}

/// 按**实际可用宽**求解等宽网格的列数与单元宽（最小单元宽 + 最大列数 → 列数）。
///
/// 起因（2026-10-27 settings-adaptive-layout §2-2b）：主页大类格栅原本写死 4 列、格宽按
/// **窗口常数** 1200 反推（`(1200 − 2·56)/4`），窗口一变就与真实可用宽脱节 —— 窄窗格宽越界、
/// 宽窗留下右侧空档。列数 / 格宽必须是可用宽的函数，而不是窗口常数的函数。
///
/// 规则（与 WinUI `ItemsWrapGrid` 的 `MaximumRowsOrColumns` + 最小项宽同语义）：
/// `列数 = clamp(floor((可用宽 + 列间隔) / (最小单元宽 + 列间隔)), 1, 最大列数)`，
/// `单元宽 = (可用宽 − 列间隔 ×(列数 − 1)) / 列数`。
/// 退化输入（可用宽 ≤ 0、最小宽 / 上限非正）不 panic：返回 1 列 + 夹到 0 的宽。
pub fn adaptive_columns(available: f32, min_cell: f32, max_columns: usize, gap: f32) -> AdaptiveColumns {
    let max_columns = max_columns.max(1);
    let gap = gap.max(0.0);
    let min_cell = min_cell.max(0.0);
    if available.is_nan() || available <= 0.0 {
        return AdaptiveColumns {
            columns: 1,
            cell_width: 0.0,
        };
    }
    // 列间隔也占宽：能塞 k 列的条件是 k·min + (k−1)·gap ≤ 可用宽。
    let fit = if min_cell > 0.0 {
        ((available + gap) / (min_cell + gap)).floor()
    } else {
        max_columns as f32
    };
    let columns = (fit.max(1.0) as usize).clamp(1, max_columns);
    let cell_width = ((available - gap * (columns as f32 - 1.0)) / columns as f32).max(0.0);
    AdaptiveColumns {
        columns,
        cell_width,
    }
}

/// 磁贴墙（TILES_DESIGN §2）—— 固定高 2 单元、左贴边、无限向右、整页分页。
///
/// 行数固定 2（纵向延长被硬约束，TILES_DESIGN §2「上下固定」）；页宽由
/// `columns_per_page` 决定，页间水平整页平移。供 Launcher 磁贴主页 / Gallery 磁贴页消费。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TileWall {
    /// 一页可见列数。
    columns_per_page: usize,
    /// 单元间隔。
    gap: f32,
    /// 单元边长（方形）。
    cell: f32,
    /// 行数（固定 2）。
    rows: usize,
}

impl TileWall {
    /// 磁贴墙固定行数。
    pub const ROWS: usize = 2;

    /// 默认 4 列/页（一页恰容纳一个 4×2 大磁贴）。
    pub fn new(cell: f32, gap: f32) -> Self {
        Self {
            columns_per_page: 4,
            gap,
            cell,
            rows: Self::ROWS,
        }
    }

    /// builder：设一页列数。
    pub const fn with_columns_per_page(mut self, n: usize) -> Self {
        self.columns_per_page = n;
        self
    }

    /// 单元边长。
    pub const fn cell(&self) -> f32 {
        self.cell
    }

    /// 一页宽（columns_per_page 单元 + 间隔）。
    pub fn page_width(&self) -> f32 {
        self.cell * self.columns_per_page as f32
            + self.gap * (self.columns_per_page as f32 - 1.0)
    }

    /// 一页高（固定 2 单元 + 间隔）。
    pub fn page_height(&self) -> f32 {
        self.cell * self.rows as f32 + self.gap * (self.rows as f32 - 1.0)
    }

    /// 磁贴矩形：`origin` = 视口原点（第一页左缘）；`page` = 页偏移；
    /// `(row, col)` = 页内单元；`span` = 跨单元（TILES_DESIGN §3：Mini 1×1 / Standard 2×2 / Large 4×2）。
    pub fn tile_rect(
        &self,
        origin: Rect,
        page: usize,
        row: usize,
        col: usize,
        span: (usize, usize),
    ) -> Rect {
        let (sw, sh) = span;
        assert!(sw >= 1 && sh >= 1 && sh <= self.rows, "磁贴高不可超墙高");
        let x = origin.origin.x
            + page as f32 * self.page_width()
            + col as f32 * (self.cell + self.gap);
        let y = origin.origin.y + row as f32 * (self.cell + self.gap);
        Rect::new(
            x,
            y,
            self.cell * sw as f32 + self.gap * (sw as f32 - 1.0),
            self.cell * sh as f32 + self.gap * (sh as f32 - 1.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── MetroGrid ───────────────────────────────────────────────

    #[test]
    fn grid_resolves_fixed_and_star() {
        // 列：80px 固定 + `*` + 2`*`；行：`*`。宽 400 → star 分配 (400−80)=320 → 106.67 / 213.33
        let g = MetroGrid::new(
            vec![GridLength::star()],
            vec![GridLength::Fixed(80.0), GridLength::star(), GridLength::Star(2.0)],
        );
        let (rh, cw) = g.resolve(Rect::new(0.0, 0.0, 400.0, 100.0), &[], &[]);
        assert_eq!(cw[0], 80.0);
        assert!((cw[1] - 106.6667).abs() < 0.01);
        assert!((cw[2] - 213.3333).abs() < 0.01);
        assert_eq!(rh[0], 100.0);
    }

    #[test]
    fn grid_auto_uses_measured_sizes() {
        let g = MetroGrid::new(
            vec![GridLength::auto(), GridLength::Fixed(40.0)],
            vec![GridLength::star()],
        );
        let (rh, _) = g.resolve(Rect::new(0.0, 0.0, 200.0, 100.0), &[24.0], &[]);
        assert_eq!(rh[0], 24.0, "Auto 行取宿主量测值");
        assert_eq!(rh[1], 40.0);
    }

    #[test]
    fn grid_child_rect_honors_span() {
        let g = MetroGrid::new(
            vec![GridLength::Fixed(20.0), GridLength::Fixed(30.0)],
            vec![GridLength::Fixed(50.0), GridLength::Fixed(50.0)],
        );
        let (rh, cw) = g.resolve(Rect::new(0.0, 0.0, 100.0, 50.0), &[], &[]);
        let r = g.child_rect(
            Rect::new(0.0, 0.0, 100.0, 50.0),
            &rh,
            &cw,
            GridChild::at(0, 1).with_span(2, 1),
        );
        assert_eq!(r, Rect::new(50.0, 0.0, 50.0, 50.0), "跨 2 行占满高度");
    }

    #[test]
    fn grid_with_gap_pads_cells() {
        let g = MetroGrid::new(
            vec![GridLength::Fixed(20.0), GridLength::Fixed(20.0)],
            vec![GridLength::Fixed(20.0), GridLength::Fixed(20.0)],
        )
        .with_gap(4.0, 4.0);
        let (rh, cw) = g.resolve(Rect::new(0.0, 0.0, 100.0, 100.0), &[], &[]);
        let r = g.child_rect(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            &rh,
            &cw,
            GridChild::at(1, 1),
        );
        assert_eq!(r.origin.x, 24.0, "x = 列宽 20 + col_gap 4");
        assert_eq!(r.origin.y, 24.0, "y = 行高 20 + row_gap 4");
    }

    // ── UniformGrid ─────────────────────────────────────────────

    fn grid() -> UniformGrid {
        UniformGrid::new(Rect::new(0.0, 0.0, 304.0, 600.0), 4, 8.0)
    }

    #[test]
    fn cell_size_squares_from_columns() {
        let g = grid();
        let cell = g.cell_size();
        assert!((cell - 70.0).abs() < 1e-4, "4 列 304 宽 8 间隔 → 70，实际 {cell}");
    }

    #[test]
    fn allocate_flows_row_major() {
        let mut g = grid();
        let cell = g.cell_size();
        let a = g.allocate((1, 1));
        let b = g.allocate((1, 1));
        assert_eq!(a.origin.x, 0.0);
        assert_eq!(b.origin.x, cell + 8.0);
        assert_eq!(b.origin.y, 0.0);
    }

    #[test]
    fn allocate_wraps_when_row_full() {
        let mut g = grid();
        for _ in 0..4 {
            g.allocate((1, 1));
        }
        assert_eq!(g.cursor(), (0, 4));
        let next = g.allocate((1, 1));
        assert_eq!(next.origin.y, g.cell_size() + 8.0, "第 5 格换行到第 2 行");
        assert_eq!(g.cursor(), (1, 1));
    }

    #[test]
    fn allocate_span_advances_by_span_width() {
        let mut g = grid();
        let cell = g.cell_size();
        // 0 键跨 2 列（计算器键区同款）
        let wide = g.allocate((2, 1));
        assert_eq!(wide.size.width, cell * 2.0 + 8.0);
        assert_eq!(g.cursor(), (0, 2));
        let next = g.allocate((1, 1));
        assert_eq!(next.origin.x, (cell + 8.0) * 2.0);
    }

    #[test]
    fn allocate_wraps_before_span_overflow() {
        let mut g = grid();
        g.allocate((2, 1)); // col=2
        g.allocate((2, 1)); // col=4 (行满)
        let next = g.allocate((2, 1)); // 需 2 列，行满 → 换行
        assert_eq!(next.origin.y, g.cell_size() + 8.0);
        assert_eq!(next.origin.x, 0.0);
    }

    #[test]
    fn new_line_jumps_to_next_row_start() {
        let mut g = grid();
        g.allocate((1, 1));
        g.allocate((1, 1));
        g.new_line();
        assert_eq!(g.cursor(), (1, 0));
    }

    #[test]
    fn content_size_reflects_rows_used() {
        let mut g = grid();
        g.allocate((2, 1));
        g.allocate((2, 1)); // 行满
        g.allocate((2, 2)); // 换行到第 2 行，高 2 单元
        let cell = g.cell_size();
        assert_eq!(g.content_size().width, cell * 4.0 + 8.0 * 3.0);
        assert_eq!(g.rows_used(), 2);
        assert_eq!(g.content_size().height, cell * 2.0 + 8.0);
    }

    // ── TileWall ───────────────────────────────────────────────────────

    #[test]
    fn tile_wall_page_dimensions() {
        let wall = TileWall::new(64.0, 8.0).with_columns_per_page(4);
        assert_eq!(wall.page_width(), 64.0 * 4.0 + 8.0 * 3.0);
        assert_eq!(wall.page_height(), 64.0 * 2.0 + 8.0);
    }

    #[test]
    fn tile_wall_rects_offset_by_page() {
        let wall = TileWall::new(64.0, 8.0).with_columns_per_page(4);
        let origin = Rect::new(16.0, 24.0, 1000.0, 200.0);
        let p0 = wall.tile_rect(origin, 0, 0, 0, (2, 2));
        let p1 = wall.tile_rect(origin, 1, 0, 0, (2, 2));
        assert_eq!(p0.origin.x, 16.0);
        assert_eq!(p1.origin.x, 16.0 + wall.page_width(), "第 2 页整页平移");
        assert_eq!(p0.size, Size::new(64.0 * 2.0 + 8.0, 64.0 * 2.0 + 8.0));
    }

    #[test]
    fn tile_wall_vertical_span_beyond_wall_panics() {
        let wall = TileWall::new(64.0, 8.0).with_columns_per_page(4);
        let origin = Rect::new(0.0, 0.0, 1000.0, 200.0);
        let result = std::panic::catch_unwind(|| wall.tile_rect(origin, 0, 0, 0, (1, 3)));
        assert!(result.is_err(), "纵向延长被硬约束（TILES_DESIGN §2）");
    }

    /// P0-3 回归：空行/空列定义不得 panic（旧 `new` 用 assert!）。
    /// 退化输入应由「解析出空尺寸表 / 子矩形夹到空」表达。
    #[test]
    fn empty_definitions_do_not_panic() {
        let g = MetroGrid::new(Vec::new(), Vec::new());
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (rows, cols) = g.resolve(rect, &[], &[]);
        assert!(rows.is_empty() && cols.is_empty());
        let child = g.child_rect(rect, &rows, &cols, GridChild::at(0, 0));
        assert_eq!(child.size.width, 0.0);
        assert_eq!(child.size.height, 0.0);
    }

    /// 自适应等宽网格：阈值两侧 / 恰在阈值 / 上限钳制 / 退化输入。
    #[test]
    fn adaptive_columns_solves_from_available_width() {
        // 4 列恰好放得下（4×272 = 1088）—— 恰在阈值。
        let a = adaptive_columns(1088.0, 272.0, 4, 0.0);
        assert_eq!((a.columns, a.cell_width), (4, 272.0));
        // 差 0.1 就只放 3 列，格宽吃满可用宽（不是留空档）。
        let b = adaptive_columns(1087.9, 272.0, 4, 0.0);
        assert_eq!(b.columns, 3);
        assert!((b.cell_width - 1087.9 / 3.0).abs() < 0.01);
        // 更宽：仍 4 列（上限），格宽变大。
        let c = adaptive_columns(1424.0, 272.0, 4, 0.0);
        assert_eq!((c.columns, c.cell_width), (4, 356.0));
        // 窄窗：1 列，格宽 = 可用宽（低于最小格宽也不再分列）。
        let d = adaptive_columns(528.0, 272.0, 4, 0.0);
        assert_eq!((d.columns, d.cell_width), (1, 528.0));
        // 带宽间隔：能塞 3 列的条件是 3×270 + 2×10 = 830 ≤ 830。
        let e = adaptive_columns(830.0, 270.0, 4, 10.0);
        assert_eq!(e.columns, 3);
        assert!((e.cell_width - (830.0 - 20.0) / 3.0).abs() < 0.01);
        let f = adaptive_columns(829.0, 270.0, 4, 10.0);
        assert_eq!(f.columns, 2);
        // 退化输入不 panic。
        for (avail, min, max, gap) in [(0.0, 272.0, 4, 0.0), (-5.0, 272.0, 0, 0.0), (100.0, 0.0, 3, 8.0)] {
            let g = adaptive_columns(avail, min, max, gap);
            assert!(g.columns >= 1 && g.cell_width >= 0.0, "{avail}/{min}/{max}/{gap}");
        }
    }

    /// P0-3 回归：`GridChild` 索引越界时夹紧，不得 panic（rows/cols 是 pub 字段）。
    #[test]
    fn child_rect_clamps_out_of_range_child() {
        let g = MetroGrid::new(vec![GridLength::star()], vec![GridLength::star()]);
        let rect = Rect::new(0.0, 0.0, 100.0, 100.0);
        let (rows, cols) = g.resolve(rect, &[], &[]);
        let child = g.child_rect(rect, &rows, &cols, GridChild { row: 9, col: 9, row_span: 0, col_span: 0 });
        assert!(child.size.width <= 100.0 + 0.01);
        assert!(child.size.height <= 100.0 + 0.01);
    }
}
