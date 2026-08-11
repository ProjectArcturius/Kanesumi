// grid.rs —— 网格布局原语（UniformGrid + TileWall）。
//
// 狗粮化缺口（kanesumi-calculator 键盘区手算 rect 暴露）：补均匀网格布局器。
// 应用：键盘（0 键跨 2 列）、磁贴墙（TILES_DESIGN §2/§3：1×1 / 2×2 / 4×2）。

use kanesumi_core::{Rect, Size};

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
}
