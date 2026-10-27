// adaptive.rs —— 按**实际可用宽**改变排布的两个容器。
//
// 起因（2026-10-27 settings-adaptive-layout §2-2）：Settings 窗口缩到 1200 以下时
// 「内容列 600 + 间隔 48 + 相关设置 240」放不下，而行容器只会把子项压扁（`Stack` 无换行、
// 无断点），结果右栏从 240 一路压到 0、内容列还越出父矩形。WinUI 的解法是
// `AdaptiveTrigger` 在断点上换模板；元素树没有模板切换，故把断点下沉到容器本身：
//
// - [`AdaptiveStack`]：一行放不下子项的**最小宽**之和（+ 间隔）时整行改成纵向堆叠
//   —— WinUI `AdaptiveTrigger` 的等价物，最小宽取子项 `width` / `min.width` / `min_main`；
// - [`AdaptiveGrid`]：等宽网格，列数 / 单元宽由 `kanesumi-structure` 的
//   `adaptive_columns` 求解器按可用宽算（最小单元宽 + 最大列数），不按窗口常数反推。
//
// 两者都在 measure / arrange **当帧**按可用宽决策：可用宽一变就必然重排。
// `Tree` 的量测缓存键是（失效标记, 上次可用宽），可用宽变了即重新量测 —— 这条由
// `tests` 里的计数控件钉住（跨断点来回切不残留旧布局）。
//
// ⚠ 两者都需要**有限宽度**（父为列 / 滚动视图 / 页面根）。行容器的量测给的是
// `(∞, 交叉轴)`，此时沿用上一次有限宽的决策（`AdaptiveStack`）或 0（`AdaptiveGrid`）。

use kanesumi_canvas::Scene;
use kanesumi_core::{Point, Rect, Size};
use kanesumi_structure::grid::adaptive_columns;

use crate::id::WidgetId;
use crate::props::LayoutProps;
use crate::widget::{ArrangeCtx, MeasureCtx, PaintCtx, Widget};

/// 量测 / 排列上下文都能给的「可见子项 + 属性」（两个上下文的取法同名，故用小子 trait 收口）。
trait KidCtx {
    fn kid_ids(&self) -> Vec<WidgetId>;
    fn kid_props(&self, child: WidgetId) -> LayoutProps;
}

impl KidCtx for MeasureCtx<'_> {
    fn kid_ids(&self) -> Vec<WidgetId> {
        self.children()
    }
    fn kid_props(&self, child: WidgetId) -> LayoutProps {
        self.child_props(child)
    }
}

impl KidCtx for ArrangeCtx<'_> {
    fn kid_ids(&self) -> Vec<WidgetId> {
        self.children()
    }
    fn kid_props(&self, child: WidgetId) -> LayoutProps {
        self.child_props(child)
    }
}

/// 可见子项（隐藏项不参与排布，与 `Stack` 同口径）。
fn visible_children(ctx: &impl KidCtx) -> Vec<(WidgetId, LayoutProps)> {
    ctx.kid_ids()
        .into_iter()
        .map(|c| (c, ctx.kid_props(c)))
        .filter(|(_, p)| p.visible)
        .collect()
}

/// 子项的「最小宽」：它不能被压到比这更窄。
///
/// 取值顺序（与 `Tree::arrange_node` 的解析一致）：固定 `width` → `min.width` →
/// `min_main`（主轴最小尺寸，行里即宽度）→ 0（可换行 / 可压缩到底）。
pub fn min_width(props: &LayoutProps) -> f32 {
    if let Some(w) = props.width {
        return w.max(0.0);
    }
    if props.min.width > 0.0 {
        return props.min.width;
    }
    props.min_main.max(0.0)
}

/// 行高取值：子项声明了固定高（`props.height`）就用它，否则用其期望高。
///
/// 为什么不能一律用期望高：主页格声明了 `height: 104`，但格子里的说明文字在窄格里会换行，
/// 期望高能到 250 —— 用期望高会让行高翻倍、格与格之间出现大空档（AFTER 首轮实测：主页
/// 3 行占 770 而非 328）。
fn row_height(props: &LayoutProps, desired: f32) -> f32 {
    props.height.unwrap_or(desired).max(props.min.height).min(props.max.height)
}

/// 一组子项能否并排放下：最小宽之和 + 间隔 ≤ 可用宽。
///
/// 阈值判定单独成函数，让**不能靠容器换行**的地方（需要换父节点的场景）复用同一条规则，
/// 避免应用侧另写一套魔法数断点。
pub fn fits_in_row(min_widths: &[f32], spacing: f32, available: f32) -> bool {
    if min_widths.is_empty() {
        return true;
    }
    let spacing = spacing.max(0.0);
    let total: f32 = min_widths.iter().map(|w| w.max(0.0)).sum::<f32>()
        + spacing * (min_widths.len() - 1) as f32;
    // 0.01 与框架的矩形包含判定同量级：恰好等于可用宽算「放得下」。
    total <= available + 0.01
}

/// 宽度不足即纵向堆叠的行（WinUI `AdaptiveTrigger` 的容器版）。
///
/// 并排时等价 [`Stack::row`](crate::widgets::Stack)，堆叠时等价 `Stack::column`
/// （主轴分配沿用同一套 grow / shrink 规则，故子项的 `grow` 在两种模式下都有效）。
pub struct AdaptiveStack {
    spacing: f32,
    row: crate::widgets::Stack,
    column: crate::widgets::Stack,
    /// 上一次**有限宽**下的决策（true = 纵向）。行父容器量测给 ∞ 时沿用它。
    stacked: bool,
}

impl AdaptiveStack {
    pub fn new() -> Self {
        Self::with_spacing(0.0)
    }

    pub fn with_spacing(spacing: f32) -> Self {
        Self {
            spacing,
            row: crate::widgets::Stack::row().with_spacing(spacing),
            column: crate::widgets::Stack::column().with_spacing(spacing),
            stacked: false,
        }
    }

    /// 当前是否纵向堆叠（上一帧决策；调试 / 测试与焦点组列数同步用）。
    pub fn is_stacked(&self) -> bool {
        self.stacked
    }

    /// 决策：有有限可用宽就按它算，否则沿用上次。
    fn decide(&self, widths: &[f32], available: f32) -> bool {
        if available.is_finite() {
            !fits_in_row(widths, self.spacing, available)
        } else {
            self.stacked
        }
    }
}

impl Default for AdaptiveStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for AdaptiveStack {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let widths: Vec<f32> = visible_children(ctx)
            .iter()
            .map(|(_, p)| min_width(p))
            .collect();
        self.stacked = self.decide(&widths, available.width);
        if self.stacked {
            self.column.measure(ctx, available)
        } else {
            self.row.measure(ctx, available)
        }
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let widths: Vec<f32> = visible_children(ctx)
            .iter()
            .map(|(_, p)| min_width(p))
            .collect();
        self.stacked = self.decide(&widths, rect.size.width);
        if self.stacked {
            self.column.arrange(ctx, rect);
        } else {
            self.row.arrange(ctx, rect);
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 无背景容器不命中：空白处穿透（与 `Stack` 同契约）。
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
}

/// 等宽自适应格栅：给定最小单元宽与最大列数，列数 / 单元宽由**实际可用宽**求解。
///
/// 与 `UniformGrid`（方形单元、外部记账）的区别：本控件是容器本身 —— 自己量测子项、
/// 自己排成行主序网格，故窗口一变就跟着变，不需要宿主按窗口常数反推格宽
/// （旧主页格栅即栽在 `(1200 − 2·56)/4` 这种算法上）。
///
/// 单元一律**等宽**（`cell_width`），不受各格内容影响；行高取该行子项期望高的最大值，
/// 子项按自身 `height` / `v_align` 在行槽内解析（`Some(h)` 固定高、Stretch 拉满行高）。
pub struct AdaptiveGrid {
    min_cell: f32,
    max_columns: usize,
    row_gap: f32,
    col_gap: f32,
    /// 上一次求解结果（`columns()` / `cell_width()` 供应用同步焦点组列数、供测试断言）。
    columns: usize,
    cell_width: f32,
}

impl AdaptiveGrid {
    /// 最小单元宽 + 列数上限（列间隔缺省 0，行间隔缺省 0）。
    pub fn new(min_cell: f32, max_columns: usize) -> Self {
        Self {
            min_cell,
            max_columns: max_columns.max(1),
            row_gap: 0.0,
            col_gap: 0.0,
            columns: 1,
            cell_width: 0.0,
        }
    }

    /// Set (行间隔, 列间隔)。
    pub fn with_gaps(mut self, row_gap: f32, col_gap: f32) -> Self {
        self.row_gap = row_gap.max(0.0);
        self.col_gap = col_gap.max(0.0);
        self
    }

    /// 给定可用宽时的列数（不需要先量测）。
    ///
    /// 建树时就要同步「网格焦点组列数」的调用方用它 —— 那一刻还没有量测结果。
    pub fn columns_for(available: f32, min_cell: f32, max_columns: usize, col_gap: f32) -> usize {
        adaptive_columns(available, min_cell, max_columns, col_gap).columns
    }

    /// 上一次求解出的列数（≥ 1）。
    pub fn columns(&self) -> usize {
        self.columns
    }

    /// 上一次求解出的单元宽。
    pub fn cell_width(&self) -> f32 {
        self.cell_width
    }

    /// 求解并把结果记下来。可用宽非有限时沿用上次单元宽（本控件须放在能给有限宽的宿主里）。
    fn solve(&mut self, available: f32) -> (usize, f32) {
        let avail = if available.is_finite() {
            available
        } else {
            self.cell_width * self.columns as f32
        };
        let solved = adaptive_columns(avail, self.min_cell, self.max_columns, self.col_gap);
        self.columns = solved.columns;
        self.cell_width = solved.cell_width;
        (solved.columns, solved.cell_width)
    }
}

impl Widget for AdaptiveGrid {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let (cols, cell) = self.solve(available.width);
        let kids = visible_children(ctx);
        let mut height = 0.0f32;
        let mut row_h = 0.0f32;
        for (i, (id, props)) in kids.iter().enumerate() {
            let d = ctx.measure_child(*id, Size::new(cell, f32::INFINITY));
            row_h = row_h.max(row_height(props, d.height));
            let row_end = (i + 1) % cols == 0 || i + 1 == kids.len();
            if row_end {
                height += row_h + if i + 1 < kids.len() { self.row_gap } else { 0.0 };
                row_h = 0.0;
            }
        }
        // 宽度就报可用宽（等宽格已吃满），被子项撑宽反而会让父容器误判。
        let width = if available.width.is_finite() {
            available.width
        } else {
            cell * cols as f32 + self.col_gap * (cols as f32 - 1.0)
        };
        Size::new(width, height)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let (cols, cell) = self.solve(rect.size.width);
        let kids = visible_children(ctx);
        let mut y = rect.origin.y;
        let mut i = 0;
        while i < kids.len() {
            let end = (i + cols).min(kids.len());
            // 行高 = 该行子项期望高的最大值（重量测同约束命中缓存，不额外排版）。
            let mut row_h = 0.0f32;
            for (id, props) in &kids[i..end] {
                let d = ctx.measure_child(*id, Size::new(cell, f32::INFINITY));
                row_h = row_h.max(row_height(props, d.height));
            }
            for (j, (id, _)) in kids[i..end].iter().enumerate() {
                let x = rect.origin.x + j as f32 * (cell + self.col_gap);
                ctx.arrange_child(*id, Rect::new(x, y, cell, row_h));
            }
            y += row_h + self.row_gap;
            i = end;
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 无背景容器不命中：格子之间的缝隙穿透到下层（与 `Stack` 同契约）。
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::props::Insets;
    use crate::testing::TestHarness;
    use std::cell::RefCell;
    use std::rc::Rc;

    /// 计数 / 记事控件：把每次量测时的**可用宽**记下来，并回报固定期望尺寸。
    ///
    /// 为什么需要它：断点类控件的正确性有一半在「可用宽变了有没有重量测」——
    /// 只看矩形对不对会在缓存命中时给出假绿。
    struct Probe {
        log: Rc<RefCell<Vec<f32>>>,
        size: Size,
        greedy: bool,
    }

    impl Probe {
        fn fixed(log: &Rc<RefCell<Vec<f32>>>, w: f32, h: f32) -> Self {
            Self {
                log: log.clone(),
                size: Size::new(w, h),
                greedy: false,
            }
        }
        fn greedy(log: &Rc<RefCell<Vec<f32>>>, h: f32) -> Self {
            Self {
                log: log.clone(),
                size: Size::new(0.0, h),
                greedy: true,
            }
        }
    }

    impl Widget for Probe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
            self.log.borrow_mut().push(available.width);
            let w = if self.greedy {
                available.width.max(0.0)
            } else {
                self.size.width
            };
            Size::new(w, self.size.height)
        }
        fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
    }

    fn stack_of(h: &mut TestHarness, spacing: f32, widths: &[f32]) -> WidgetId {
        let log = Rc::new(RefCell::new(Vec::new()));
        let id = h
            .tree
            .insert_with(h.root(), AdaptiveStack::with_spacing(spacing), LayoutProps::default());
        for w in widths {
            h.tree.insert_with(
                id,
                Probe::fixed(&log, *w, 40.0),
                LayoutProps {
                    width: Some(*w),
                    height: Some(40.0),
                    v_align: crate::props::Align::Start,
                    ..LayoutProps::default()
                },
            );
        }
        id
    }

    /// 并排 / 堆叠两侧 + **恰在阈值**（最小宽之和 + 间隔 = 可用宽 → 并排）。
    #[test]
    fn adaptive_stack_flips_at_min_width_sum() {
        let mut h = TestHarness::new(700.0, 300.0);
        let id = stack_of(&mut h, 10.0, &[300.0, 300.0]);
        h.frame();
        let kids = h.tree.children(id).to_vec();
        assert_eq!(h.rect(kids[0]).origin.y, h.rect(kids[1]).origin.y, "700 宽应并排");
        assert_eq!(h.rect(kids[1]).origin.x, 310.0);

        // 恰好 610 = 300 + 10 + 300 → 仍并排。
        h.resize(610.0, 300.0);
        h.frame();
        assert_eq!(h.rect(kids[0]).origin.y, h.rect(kids[1]).origin.y, "恰在阈值应并排");

        // 差 1 → 堆叠：同一 x，y 递增。
        h.resize(609.0, 300.0);
        h.frame();
        assert_eq!(h.rect(kids[0]).origin.x, h.rect(kids[1]).origin.x, "609 宽应堆叠");
        assert_eq!(h.rect(kids[1]).origin.y, 50.0, "堆叠时按间隔下移");
        assert!(h.tree.get::<AdaptiveStack>(id).is_some_and(|s| s.is_stacked()));
    }

    /// 跨断点来回切：每次都要重量测，且不能残留上一模式的矩形。
    #[test]
    fn adaptive_stack_toggle_remeasures_and_leaves_no_stale_layout() {
        let mut h = TestHarness::new(800.0, 300.0);
        let log = Rc::new(RefCell::new(Vec::new()));
        let id = h
            .tree
            .insert_with(h.root(), AdaptiveStack::with_spacing(8.0), LayoutProps::default());
        let a = h.tree.insert_with(
            id,
            Probe::fixed(&log, 300.0, 40.0),
            LayoutProps { width: Some(300.0), ..LayoutProps::default() },
        );
        let b = h.tree.insert_with(
            id,
            Probe::fixed(&log, 300.0, 40.0),
            LayoutProps { width: Some(300.0), ..LayoutProps::default() },
        );
        h.frame();
        assert_eq!(h.rect(a).origin.y, h.rect(b).origin.y);
        let stacked_at_800 = log.borrow().len();

        // 800 → 400（堆叠）→ 800（并排）→ 400（堆叠）。
        for (w, want_stacked) in [(400.0f32, true), (800.0, false), (400.0, true)] {
            h.resize(w, 300.0);
            h.frame();
            let stacked = h.tree.get::<AdaptiveStack>(id).unwrap().is_stacked();
            assert_eq!(stacked, want_stacked, "{w} 宽的模式");
            let (ra, rb) = (h.rect(a), h.rect(b));
            if want_stacked {
                assert_eq!(ra.origin.x, rb.origin.x, "{w}: 堆叠应对齐左缘");
                assert!(rb.origin.y >= ra.bottom(), "{w}: 堆叠应上下相接");
            } else {
                assert_eq!(ra.origin.y, rb.origin.y, "{w}: 并排应对齐顶边");
                assert!(rb.origin.x >= ra.right(), "{w}: 并排应左右相接");
            }
        }
        // 每次跨断点都重量测过（可用宽变了 → 缓存不得复用）。
        assert!(
            log.borrow().len() > stacked_at_800,
            "跨断点未重量测：量测次数没增加"
        );
    }

    fn grid_of(h: &mut TestHarness, n: usize, min_cell: f32, max_cols: usize) -> (WidgetId, Vec<WidgetId>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let gid = h.tree.insert_with(
            h.root(),
            AdaptiveGrid::new(min_cell, max_cols).with_gaps(8.0, 0.0),
            LayoutProps::default(),
        );
        let cells = (0..n)
            .map(|_| {
                h.tree.insert_with(
                    gid,
                    Probe::greedy(&log, 104.0),
                    LayoutProps {
                        height: Some(104.0),
                        ..LayoutProps::default()
                    },
                )
            })
            .collect();
        (gid, cells)
    }

    /// 等宽格栅：阈值两侧 / 恰在阈值 / 上限钳制 / 格宽吃满可用宽 / 行间隔。
    #[test]
    fn adaptive_grid_solves_columns_from_available_width() {
        let mut h = TestHarness::new(1200.0, 400.0);
        let (gid, cells) = grid_of(&mut h, 5, 272.0, 4);
        h.frame();
        // 1200 → 4 列 300 宽；第 5 格换行，y = 104 + 8。
        assert_eq!(h.tree.get::<AdaptiveGrid>(gid).unwrap().columns(), 4);
        assert_eq!(h.rect(cells[0]).size.width, 300.0);
        assert_eq!(h.rect(cells[3]).origin.x, 900.0);
        assert_eq!((h.rect(cells[4]).origin.x, h.rect(cells[4]).origin.y), (0.0, 112.0));
        assert_eq!(h.rect(cells[4]).size.width, 300.0, "换行格仍等宽");

        // 恰在 4 列阈值 1088 → 4 列 × 272。
        h.resize(1088.0, 400.0);
        h.frame();
        assert_eq!(h.tree.get::<AdaptiveGrid>(gid).unwrap().columns(), 4);
        assert_eq!(h.rect(cells[0]).size.width, 272.0);

        // 1087 → 3 列，格宽吃满（1087/3）。
        h.resize(1087.0, 400.0);
        h.frame();
        assert_eq!(h.tree.get::<AdaptiveGrid>(gid).unwrap().columns(), 3);
        assert!((h.rect(cells[0]).size.width - 1087.0 / 3.0).abs() < 0.01);

        // 528 → 低于 2 列（2×272）→ 1 列，格宽 528；5 格各占一行。
        h.resize(528.0, 400.0);
        h.frame();
        assert_eq!(h.tree.get::<AdaptiveGrid>(gid).unwrap().columns(), 1);
        assert_eq!(h.rect(cells[0]).size.width, 528.0);
        for (i, c) in cells.iter().enumerate() {
            assert_eq!(h.rect(*c).origin.y, i as f32 * 112.0, "第 {i} 格行位");
        }
        // 等宽不变式：所有格同宽。
        let widths: Vec<f32> = cells.iter().map(|c| h.rect(*c).size.width).collect();
        assert!(widths.windows(2).all(|w| (w[0] - w[1]).abs() < 0.01), "{widths:?}");
    }

    /// 行高取子项**声明的**固定高：说明文字在窄格里换行会撑高期望高，若按期望高排，
    /// 行高会翻倍（AFTER 首轮实测：主页 3 行占 770 而非 328）。
    #[test]
    fn adaptive_grid_row_height_honors_declared_height() {
        let mut h = TestHarness::new(1200.0, 800.0);
        let log = Rc::new(RefCell::new(Vec::new()));
        let gid = h
            .tree
            .insert_with(h.root(), AdaptiveGrid::new(272.0, 4), LayoutProps::default());
        let mut cells = Vec::new();
        for _ in 0..5 {
            // 期望高 250（模拟换行后的说明文字），但声明高 104。
            cells.push(h.tree.insert_with(
                gid,
                Probe::greedy(&log, 250.0),
                LayoutProps {
                    height: Some(104.0),
                    ..LayoutProps::default()
                },
            ));
        }
        h.frame();
        for c in &cells {
            assert_eq!(h.rect(*c).size.height, 104.0);
        }
        // 第 5 格换到第二行：y = 一行高（104），不是期望高（250）。
        assert_eq!(h.rect(cells[4]).origin.y, 104.0, "行高应取声明高");
    }

    /// 可用宽变化必须重测子项（缓存键含可用宽），来回切不留旧格宽。
    #[test]
    fn adaptive_grid_remeasures_when_available_width_changes() {
        let mut h = TestHarness::new(1200.0, 400.0);
        let log = Rc::new(RefCell::new(Vec::new()));
        let gid = h
            .tree
            .insert_with(h.root(), AdaptiveGrid::new(272.0, 4), LayoutProps::default());
        let mut cells = Vec::new();
        for _ in 0..2 {
            cells.push(h.tree.insert_with(
                gid,
                Probe::greedy(&log, 104.0),
                LayoutProps { height: Some(104.0), ..LayoutProps::default() },
            ));
        }
        h.frame();
        assert!(log.borrow().iter().any(|w| (*w - 300.0).abs() < 0.01), "1200: 4 列 → 每格 300");
        log.borrow_mut().clear();
        h.resize(1088.0, 400.0);
        h.frame();
        assert!(
            log.borrow().iter().any(|w| (*w - 272.0).abs() < 0.01),
            "1088: 应重量测为 272，实得 {:?}",
            log.borrow()
        );
        // 回到 1200：格宽必须回到 600，不留 272。
        h.resize(1200.0, 400.0);
        h.frame();
        assert_eq!(h.rect(cells[0]).size.width, 300.0);
    }

    /// `fits_in_row` / `min_width`：阈值语义与「最小宽」取值顺序。
    #[test]
    fn fits_in_row_and_min_width_contract() {
        assert!(fits_in_row(&[], 10.0, 0.0));
        assert!(fits_in_row(&[600.0, 240.0], 48.0, 888.0), "恰好放下算放得下");
        assert!(!fits_in_row(&[600.0, 240.0], 48.0, 887.0));
        assert!(fits_in_row(&[100.0], 0.0, 100.0));
        // 最小宽：固定宽 > min.width > min_main > 0。
        let fixed = LayoutProps { width: Some(320.0), min_main: 10.0, ..LayoutProps::default() };
        assert_eq!(min_width(&fixed), 320.0);
        let minw = LayoutProps {
            min: Size::new(180.0, 0.0),
            min_main: 10.0,
            ..LayoutProps::default()
        };
        assert_eq!(min_width(&minw), 180.0);
        let flexible = LayoutProps { min_main: 96.0, margin: Insets::default(), ..LayoutProps::default() };
        assert_eq!(min_width(&flexible), 96.0);
        assert_eq!(min_width(&LayoutProps::default()), 0.0);
    }
}
