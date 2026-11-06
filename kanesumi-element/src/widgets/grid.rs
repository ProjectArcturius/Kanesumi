// Grid —— 二维网格容器（XAML `Grid` + `RowDefinition` / `ColumnDefinition`）。
//
// 为什么要有它：`Stack` 只能排一维。设置页那种「标签一列、控件一列、右侧说明对齐」的表单，
// 用 `Stack` 拼就得靠固定宽 + margin 对齐，等于把几何重新手写一遍 —— 这正是元素树要消灭的东西。
// 有了 `Grid`，应用只声明轨道与「子节点在第几行第几列」（附着属性 `LayoutProps::grid_cell`，
// 对应 XAML `Grid.Row/Column/RowSpan/ColumnSpan`），位置全部由框架算。
//
// 与 UWP 的差异：UWP 的 `Grid` 无间距，间距靠子元素 Margin；本实现另给 `with_gap`
// （矩阵统一间距，省掉每个子节点重复写 margin），默认 0 时与 UWP 行为一致。
// 轨道求解复用 `kanesumi-structure::MetroGrid`（`CONTROL_SPEC` §33 的求解器），
// 本文件只负责「按树取子节点、量测、排入单元」，不另写一套算法。

use kanesumi_canvas::Scene;
use kanesumi_core::{Point, Rect, Size};
use kanesumi_structure::{GridLength, MetroGrid};

use crate::id::WidgetId;
use crate::props::LayoutProps;
use crate::widget::{ArrangeCtx, MeasureCtx, PaintCtx, Widget};

/// 二维网格容器。轨道定义 + 子节点单元（单元来自子节点的 `LayoutProps::grid_cell`）。
///
/// 量测语义：`Fixed` 取定义值，`Auto` 取该轨道内单跨子节点的最大期望尺寸，
/// `Star` 在量测期按轨道内内容的自然尺寸贡献（不撑满可用），排布期再按权重瓜分真实空间。
/// 因此把 `Grid` 放进 `Stack` 时不会因为 `Star` 而抢满整行。
#[derive(Debug, Clone, PartialEq)]
pub struct Grid {
    pub grid: MetroGrid,
}

impl Grid {
    pub fn new(rows: Vec<GridLength>, cols: Vec<GridLength>) -> Self {
        Self {
            grid: MetroGrid::new(rows, cols),
        }
    }

    /// 常见表单：左列固定标签宽、右列吃满剩余。**行数不必声明** —— 子节点落在声明范围外时
    /// 按隐式 `Auto` 轨道逐行补出（见 [`Self::effective_tracks`]），所以 `.form(140.0)` 一行写完，
    /// 内容有多少行就是多少行。
    pub fn form(label_width: f32) -> Self {
        Self::new(
            vec![GridLength::Auto],
            vec![GridLength::Fixed(label_width), GridLength::star()],
        )
    }

    /// 统一间距 `(行间距, 列间距)`。
    pub fn with_gap(mut self, row_gap: f32, col_gap: f32) -> Self {
        self.grid = self.grid.with_gap(row_gap.max(0.0), col_gap.max(0.0));
        self
    }

    /// 行数 / 列数（子节点轨道越界时由 `MetroGrid::child_rect` 夹紧，不 panic）。
    pub fn tracks(&self) -> (usize, usize) {
        (self.grid.rows.len(), self.grid.cols.len())
    }

    /// 有效轨道 = 声明轨道 + **隐式补出的 `Auto` 轨道**。
    ///
    /// 子节点落在声明范围之外时（`Grid::form(140.0)` 只声明一行，却写了五行内容），
    /// 按 CSS 网格的隐式轨道语义补 `Auto`，而不是像 XAML 那样把越界子节点丢进最后一格
    /// 叠在一起 —— 叠在一起是最难查的一类布局 bug，静默重叠比「明显多一行」糟得多。
    /// 应用因此不必数行数：`form()` 写一次，行数由内容决定。
    fn effective_tracks(&self, kids: &[(WidgetId, LayoutProps)]) -> MetroGrid {
        let mut g = self.grid.clone();
        let (mut rows, mut cols) = (g.rows.len(), g.cols.len());
        for (_, p) in kids {
            let c = p.grid_cell;
            rows = rows.max(c.row + c.row_span.max(1));
            cols = cols.max(c.col + c.col_span.max(1));
        }
        while g.rows.len() < rows {
            g.rows.push(GridLength::Auto);
        }
        while g.cols.len() < cols {
            g.cols.push(GridLength::Auto);
        }
        g
    }

    /// 可见子节点及其布局属性。
    fn kids(ctx_children: Vec<WidgetId>, props: impl Fn(WidgetId) -> LayoutProps) -> Vec<(WidgetId, LayoutProps)> {
        ctx_children
            .into_iter()
            .map(|c| (c, props(c)))
            .filter(|(_, p)| p.visible)
            .collect()
    }

    /// 量测所有子节点，按轨道汇总 Auto 尺寸与 Star 的「内容贡献」。
    ///
    /// 返回 `(auto_rows, auto_cols)`：两者都已把 Star 轨道的**内容自然尺寸**填进去，
    /// 量测期据此求总尺寸；排布期 `MetroGrid::resolve` 只在 Star 轨道上覆盖这两列值。
    fn measure_tracks(
        &self,
        tracks: &MetroGrid,
        kids: &[(WidgetId, LayoutProps)],
        available: Size,
        measure: &mut dyn FnMut(WidgetId, Size) -> Size,
    ) -> (Vec<f32>, Vec<f32>) {
        let mut auto_rows = vec![0.0f32; tracks.rows.len()];
        let mut auto_cols = vec![0.0f32; tracks.cols.len()];
        for (c, p) in kids {
            let d = measure(*c, available);
            let cell = p.grid_cell;
            // 跨轨道子节点不参与单轨道 Auto 求解（与 UWP 一样：跨轨道时 Auto 只按单跨内容算，
            // 跨轨道内容超出由夹紧兜住），避免一条内容撑大整排轨道。
            if cell.row < auto_rows.len()
                && (cell.row_span == 1 || !matches!(tracks.rows[cell.row], GridLength::Auto))
            {
                auto_rows[cell.row] = auto_rows[cell.row].max(d.height);
            }
            if cell.col < auto_cols.len()
                && (cell.col_span == 1 || !matches!(tracks.cols[cell.col], GridLength::Auto))
            {
                auto_cols[cell.col] = auto_cols[cell.col].max(d.width);
            }
        }
        (auto_rows, auto_cols)
    }
}

impl Widget for Grid {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let kids = Self::kids(ctx.children(), |c| ctx.child_props(c));
        if kids.is_empty() {
            return Size::ZERO;
        }
        let tracks = self.effective_tracks(&kids);
        let (rows, cols) = (tracks.rows.len(), tracks.cols.len());
        let (auto_rows, auto_cols) = self.measure_tracks(&tracks, &kids, available, &mut |c, avail| {
            ctx.measure_child(c, avail)
        });

        let (mut w, mut h) = (0.0f32, 0.0f32);
        for (i, len) in tracks.rows.iter().enumerate() {
            h += match len {
                GridLength::Fixed(v) => v.max(0.0),
                GridLength::Auto | GridLength::Star(_) => auto_rows[i],
            };
        }
        for (i, len) in tracks.cols.iter().enumerate() {
            w += match len {
                GridLength::Fixed(v) => v.max(0.0),
                GridLength::Auto | GridLength::Star(_) => auto_cols[i],
            };
        }
        h += self.grid.gap.0 * rows.saturating_sub(1) as f32;
        w += self.grid.gap.1 * cols.saturating_sub(1) as f32;
        Size::new(w.min(available.width.max(0.0)), h.min(available.height.max(0.0)))
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let kids = Self::kids(ctx.children(), |c| ctx.child_props(c));
        if kids.is_empty() {
            return;
        }
        let tracks = self.effective_tracks(&kids);
        let (auto_rows, auto_cols) = self.measure_tracks(&tracks, &kids, rect.size, &mut |c, avail| {
            ctx.measure_child(c, avail)
        });
        let (heights, widths) = tracks.resolve(rect, &auto_rows, &auto_cols);
        for (c, p) in &kids {
            let slot = tracks.child_rect(rect, &heights, &widths, p.grid_cell);
            ctx.arrange_child(*c, slot);
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 无背景容器不命中：单元之间的空白穿透到下层（COMPOSITION 契约 12，与 `Stack` 同）。
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::props::{Align, Insets};
    use crate::testing::TestHarness;
    use crate::widgets::Stack;
    use kanesumi_core::Color;
    use kanesumi_structure::GridChild;

    /// 固定尺寸探针：量测值即内容尺寸，命中 = 自身矩形。
    struct Probe {
        size: Size,
        focused: bool,
    }

    impl Probe {
        fn new(w: f32, h: f32) -> Self {
            Self {
                size: Size::new(w, h),
                focused: false,
            }
        }
    }

    impl Widget for Probe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            self.size
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            scene.fill_rect(Color::WHITE, ctx.rect());
        }
        fn focusable(&self) -> bool {
            self.focused
        }
    }

    /// 把网格挂在根下并固定尺寸（起点对齐，便于逐像素断言）。
    fn grid_harness(g: Grid, w: f32, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            g,
            LayoutProps {
                width: Some(w),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn cell(row: usize, col: usize) -> LayoutProps {
        LayoutProps::in_cell(row, col)
    }

    #[test]
    fn props_helpers_express_cells_and_spans() {
        // 附着属性的便捷写法与直接写 grid_cell 等价（应用侧不必自造 cell()）。
        let p = LayoutProps::in_cell(2, 1).spanning(1, 3);
        assert_eq!(p.grid_cell, GridChild::at(2, 1).with_span(1, 3));
        assert_eq!(LayoutProps::in_cell(0, 0).grid_cell, GridChild::at(0, 0));
        assert_eq!(
            LayoutProps::in_cell(0, 0).spanning(0, 0).grid_cell,
            GridChild::at(0, 0).with_span(1, 1),
            "跨度下限 1，不会被写成 0"
        );
    }

    #[test]
    fn fixed_tracks_place_children_in_their_cells() {
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::Fixed(40.0), GridLength::Fixed(60.0)],
                vec![GridLength::Fixed(100.0), GridLength::Fixed(200.0)],
            ),
            300.0,
            100.0,
        );
        let a = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(0, 0));
        let b = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(0, 1));
        let c = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(1, 1));
        h.frame();
        assert_eq!(h.rect(a), Rect::new(0.0, 0.0, 100.0, 40.0), "第 0 行第 0 列");
        assert_eq!(h.rect(b), Rect::new(100.0, 0.0, 200.0, 40.0), "第 0 行第 1 列");
        assert_eq!(h.rect(c), Rect::new(100.0, 40.0, 200.0, 60.0), "第 1 行第 1 列");
    }

    #[test]
    fn star_columns_share_remaining_width_by_weight() {
        // 表面 400 宽（夹具下限）：固定 100 + Star 1:3 瓜分剩余 300 → 75 / 225。
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::auto()],
                vec![GridLength::Fixed(100.0), GridLength::star(), GridLength::Star(3.0)],
            ),
            400.0,
            30.0,
        );
        let a = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(0, 1));
        let b = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(0, 2));
        h.frame();
        assert_eq!(h.rect(a).size.width, 75.0, "1* 得剩余 300 的四分之一");
        assert_eq!(h.rect(b).size.width, 225.0, "3* 得剩余 300 的四分之三");
        assert_eq!(h.rect(b).origin.x, 175.0);
    }

    #[test]
    fn auto_column_takes_widest_content() {
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::auto()],
                vec![GridLength::auto(), GridLength::star()],
            ),
            300.0,
            20.0,
        );
        let narrow = h.tree.insert_with(g, Probe::new(30.0, 10.0), cell(0, 0));
        let wide = h.tree.insert_with(g, Probe::new(80.0, 10.0), cell(1, 0));
        h.frame();
        assert_eq!(h.rect(narrow).size.width, 80.0, "Auto 列宽 = 该列最宽内容");
        assert_eq!(h.rect(wide).size.width, 80.0);
    }

    #[test]
    fn span_merges_tracks_and_gap() {
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::Fixed(20.0)],
                vec![GridLength::Fixed(50.0), GridLength::Fixed(70.0)],
            )
            .with_gap(0.0, 10.0),
            130.0,
            20.0,
        );
        let across = h.tree.insert_with(
            g,
            Probe::new(10.0, 10.0),
            LayoutProps {
                grid_cell: GridChild::at(0, 0).with_span(1, 2),
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(across).size.width, 130.0, "跨两列 = 50 + 10 间距 + 70");
    }

    #[test]
    fn gap_offsets_following_tracks() {
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::Fixed(20.0), GridLength::Fixed(20.0)],
                vec![GridLength::Fixed(40.0)],
            )
            .with_gap(12.0, 0.0),
            40.0,
            52.0,
        );
        let second = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(1, 0));
        h.frame();
        assert_eq!(h.rect(second).origin.y, 32.0, "第二行起点 = 20 + 12 行间距");
    }

    #[test]
    fn empty_tracks_layout_nothing_without_panic() {
        // 声明零轨道（审计 P0-3 的退化输入）：不 panic。按隐式轨道语义，子节点各占一列
        // 自动轨道 —— 会有布局，但绝不会静默重叠或落到零矩形。
        let (mut h, g) = grid_harness(Grid::new(Vec::new(), Vec::new()), 100.0, 50.0);
        let orphan = h.tree.insert_with(g, Probe::new(10.0, 10.0), cell(0, 0));
        h.frame();
        assert!(h.rect(orphan).size.width > 0.0 && h.rect(orphan).size.height > 0.0);
    }

    #[test]
    fn implicit_auto_rows_hold_form_rows_without_counting() {
        // `Grid::form` 只声明一行；三行内容各自落在一行里，不叠在一起。
        let (mut h, g) = grid_harness(Grid::form(140.0).with_gap(0.0, 16.0), 400.0, 200.0);
        let mut rows = Vec::new();
        for r in 0..3 {
            let label = h.tree.insert_with(g, Probe::new(60.0, 20.0), cell(r, 0));
            h.tree.insert_with(g, Probe::new(40.0, 20.0), cell(r, 1));
            rows.push(label);
        }
        h.frame();
        let ys: Vec<f32> = rows.iter().map(|id| h.rect(*id).origin.y).collect();
        assert_eq!(ys, vec![0.0, 20.0, 40.0], "隐式 Auto 行逐行累加，不重叠");
        assert_eq!(h.rect(rows[0]).origin.x, 0.0, "标签列贴左");
        assert_eq!(h.rect(rows[0]).size.width, 140.0, "固定标签列宽");
    }

    #[test]
    fn children_stay_inside_and_blank_space_does_not_hit() {
        let (mut h, g) = grid_harness(
            Grid::new(
                vec![GridLength::Fixed(40.0), GridLength::Fixed(40.0)],
                vec![GridLength::Fixed(60.0), GridLength::Fixed(60.0)],
            )
            .with_gap(8.0, 8.0),
            128.0,
            88.0,
        );
        let a = h.tree.insert_with(
            g,
            Probe {
                size: Size::new(20.0, 20.0),
                focused: true,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::all(8.0),
                grid_cell: GridChild::at(0, 0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(a);
        assert_eq!(h.tree.hit(Point::new(70.0, 44.0)), None, "单元间空白不命中");

        // 网格自己不吃指针（空白穿透）。
        assert_eq!(h.tree.hit(Point::new(126.0, 86.0)), None);
    }

    #[test]
    fn grid_inside_stack_does_not_steal_star_space() {
        // Star 在量测期只按内容贡献：放进 Stack 且靠左时，宽度取内容宽而非整行。
        let mut h = TestHarness::new(400.0, 200.0);
        let col = h.tree.insert(h.root(), Stack::column());
        let g = h.tree.insert_with(
            col,
            Grid::new(vec![GridLength::auto()], vec![GridLength::star()]),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(g, Probe::new(45.0, 12.0), cell(0, 0));
        h.frame();
        assert_eq!(h.rect(g).size.width, 45.0, "无界/未分配时 Star 退化为内容宽");
    }
}
