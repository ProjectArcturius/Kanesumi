// ItemsRepeater —— 元素树虚拟化列表容器。参 docs/ELEMENT_TREE.md §Ⅳ-bis / §Ⅹ E4。
//
// 对照来源：`reference/microsoft-ui-xaml/dev/Repeater/`（ItemsRepeater.cpp + FlowLayout.cpp，
// 开源）。WinUI `ItemsRepeater` 在 measure 里按「有效视口」向 `ElementFactory` 要元素、
// 回收离开视口的元素。Kanesumi 元素树用 `Widget::realize` 钩子表达同一件事：容器在
// `Tree::frame` 的 measure 之前，按上一帧 `rect`（视口）**增删自己的子节点** ——
// 可见行成为节点，离开视口的行进回收池（不删，只隐藏），池上限 = 可见数。
//
// 几何复用 `MetroRepeater`（可见范围 / 条目矩形 / scroll_into_view），滚动状态复用
// `MetroScrollView`（offset / 弹簧 / 滚动条）—— 本控件只做「索引 → 节点」的簿记与回收。
// 与 XAML 同：`ItemsRepeater` 本身不管选择，只提供 `bring_into_view_index`。

use std::collections::{BTreeMap, HashSet};

use kanesumi_canvas::Scene;
use kanesumi_core::{Rect, Size};
use kanesumi_element::{
    ArrangeCtx, Event, EventCtx, MeasureCtx, PaintCtx, RealizeCtx, UpdateCtx, Widget, WidgetId,
};

use crate::repeater::MetroRepeater;
use crate::scroll_view::{MetroScrollView, ScrollOffsetChanged};

/// 条目工厂：数据源 + 元素创建 / 复用。参 ELEMENT_TREE §Ⅳ-bis。
///
/// `key` 必须**稳定且唯一**：数据重排 / 增删后用它在两次 realize 之间复用节点。
/// `bind` 在回收池命中或数据变化时调用，把某 `index` 的数据写进已有节点（`ctx.edit`）。
pub trait ItemFactory: 'static {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 稳定键（数据重排 / 增删后用来复用节点）。同一数据集内不得重复。
    fn key(&self, index: usize) -> u64;

    /// 新建一行元素（由 repeater 插入自身之下）。
    fn build(&mut self, index: usize) -> Box<dyn Widget>;

    /// 复用已有节点显示另一个 `index` 的数据。
    fn bind(&mut self, index: usize, ctx: &mut RealizeCtx, node: WidgetId);
}

/// 布局模式。只做等高（Stack）与等格（UniformGrid）；**不做**变高行（后续项）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ItemLayout {
    /// 纵向等高空栈（`spacing` = 主轴间距）。
    Stack { item_extent: f32, spacing: f32 },
    /// 等宽网格（`cell` 为单元尺寸，列数按视口宽自适应）。
    UniformGrid { cell: Size, spacing: f32 },
}

impl ItemLayout {
    pub fn stack(item_extent: f32) -> Self {
        Self::Stack {
            item_extent: item_extent.max(0.0),
            spacing: 0.0,
        }
    }

    pub fn grid(cell: Size, spacing: f32) -> Self {
        Self::UniformGrid {
            cell,
            spacing: spacing.max(0.0),
        }
    }
}

/// 已实现条目：数据键（复用依据）+ 当前索引 + 节点。
#[derive(Debug, Clone, Copy)]
struct Realized {
    index: usize,
    node: WidgetId,
}

/// 元素树虚拟化列表容器。参本文件头部说明。
pub struct ItemsRepeater {
    factory: Option<Box<dyn ItemFactory>>,
    layout: ItemLayout,
    /// 滚动状态机（自带滚动，不嵌 `MetroScrollView` 节点，避免两层协调）。
    scroll: MetroScrollView,
    /// 几何引擎（可见范围 / 内容长 / scroll_into_view）。
    repeater: MetroRepeater,
    /// 键 → 已实现条目。以**键**为索引，数据重排后节点自然沿用。
    realized: BTreeMap<u64, Realized>,
    /// 回收池：已建但不显示的行（`visible = false`，不删）。
    pool: Vec<WidgetId>,
    /// 网格列数（按视口宽自适应，`realize` 中更新）。
    columns: usize,
    /// 数据条数。
    count: usize,
    /// 上一帧视口（`realize` 判定与 `arrange` 一致性）。
    viewport: Size,
    /// 视口上下各预留的比例（默认半屏，参任务设计）。
    overscan: f32,
    /// 数据变化标记：下一次 realize 强制重绑全部保留节点。
    dirty: bool,
}

impl ItemsRepeater {
    pub fn new(layout: ItemLayout, factory: impl ItemFactory) -> Self {
        let mut scroll = MetroScrollView::default();
        scroll.smooth_scroll = false;
        Self {
            factory: Some(Box::new(factory)),
            layout,
            scroll,
            repeater: MetroRepeater::default(),
            realized: BTreeMap::new(),
            pool: Vec::new(),
            columns: 1,
            count: 0,
            viewport: Size::ZERO,
            overscan: 0.5,
            dirty: true,
        }
    }

    /// 纵向等高列表便捷构造。
    pub fn stack(item_extent: f32, factory: impl ItemFactory) -> Self {
        Self::new(ItemLayout::stack(item_extent), factory)
    }

    /// 等宽网格便捷构造。
    pub fn grid(cell: Size, spacing: f32, factory: impl ItemFactory) -> Self {
        Self::new(ItemLayout::grid(cell, spacing), factory)
    }

    /// 设置上下 overscan（视口比例；0 = 只实现可见行）。
    pub fn with_overscan(mut self, ratio: f32) -> Self {
        self.overscan = ratio.max(0.0);
        self
    }

    /// 是否用弹簧平滑滚动（默认关；滚轮离散步即时生效）。
    pub fn with_smooth_scroll(mut self, on: bool) -> Self {
        self.scroll.smooth_scroll = on;
        self
    }

    pub fn set_layout(&mut self, layout: ItemLayout) {
        self.layout = layout;
        self.dirty = true;
    }

    /// 换数据源（全量重绑，按 key 尽量复用节点）。
    pub fn set_factory(&mut self, factory: impl ItemFactory) {
        self.factory = Some(Box::new(factory));
        self.dirty = true;
    }

    /// 数据原地变化（增删 / 重排 / 改内容）后调用：下一次 realize 全量重绑。
    /// 调用方须经 `tree.edit` 并 `ctx.invalidate_measure()` 触发下一帧 realize。
    pub fn items_changed(&mut self) {
        self.dirty = true;
    }

    pub fn offset(&self) -> f32 {
        self.scroll.offset
    }

    pub fn max_offset(&self) -> f32 {
        self.scroll.max_offset()
    }

    /// 滚动到指定偏移（夹紧；`animate` 走弹簧）。
    pub fn scroll_to(&mut self, offset: f32, animate: bool) {
        self.scroll.scroll_to(offset, animate);
        self.dirty = true;
    }

    /// 已实现的条目数（可见 + overscan，远小于总条目数）。
    pub fn realized_count(&self) -> usize {
        self.realized.len()
    }

    /// 回收池大小（已建未显示的行）。
    pub fn pool_len(&self) -> usize {
        self.pool.len()
    }

    pub fn is_realized(&self, index: usize) -> bool {
        self.realized_node(index).is_some()
    }

    /// 当前显示 `index` 数据的节点。
    pub fn realized_node(&self, index: usize) -> Option<WidgetId> {
        self.realized
            .values()
            .find(|r| r.index == index)
            .map(|r| r.node)
    }

    pub fn factory(&self) -> Option<&dyn ItemFactory> {
        self.factory.as_deref()
    }

    // ── 几何（复用 MetroRepeater 的公式）─────────────────────────────────────

    /// 主轴步长（条目跨度 + 间距）。
    fn stride(&self) -> f32 {
        match self.layout {
            ItemLayout::Stack {
                item_extent,
                spacing,
            } => item_extent + spacing,
            ItemLayout::UniformGrid { cell, spacing } => cell.height + spacing,
        }
    }

    /// 单个条目的尺寸（Stack：撑满视口宽；Grid：单元）。
    fn item_size(&self, viewport: Size) -> Size {
        match self.layout {
            ItemLayout::Stack { item_extent, .. } => Size::new(viewport.width, item_extent),
            ItemLayout::UniformGrid { cell, .. } => cell,
        }
    }

    fn columns_clamped(&self) -> usize {
        self.columns.max(1)
    }

    /// 内容总主轴长（Stack / UniformGrid 公式同 `MetroRepeater`）。
    fn content_length(&self) -> f32 {
        match self.layout {
            ItemLayout::Stack {
                item_extent,
                spacing,
            } => (self.count as f32 * (item_extent + spacing) - spacing).max(0.0),
            ItemLayout::UniformGrid { cell, spacing } => {
                let rows = self.count.div_ceil(self.columns_clamped());
                (rows as f32 * (cell.height + spacing) - spacing).max(0.0)
            }
        }
    }

    /// 条目 `index` 在视口坐标系里的矩形（已扣滚动偏移）。
    fn item_rect(&self, index: usize, viewport: Size, offset: f32) -> Rect {
        match self.layout {
            ItemLayout::Stack {
                item_extent,
                spacing,
            } => Rect::new(
                0.0,
                index as f32 * (item_extent + spacing) - offset,
                viewport.width,
                item_extent,
            ),
            ItemLayout::UniformGrid { cell, spacing } => {
                let columns = self.columns_clamped();
                let col = index % columns;
                let row = index / columns;
                Rect::new(
                    col as f32 * (cell.width + spacing),
                    row as f32 * (cell.height + spacing) - offset,
                    cell.width,
                    cell.height,
                )
            }
        }
    }

    /// 视口（含上下 overscan）内需要实现的索引集。参任务设计「可见范围 + 上下各 overscan」。
    fn visible_indices(&self) -> Vec<usize> {
        if self.count == 0 {
            return Vec::new();
        }
        let vp = self.viewport.height;
        if vp <= 0.0 {
            return Vec::new();
        }
        let overscan = vp * self.overscan;
        let lo = (self.scroll.offset - overscan).max(0.0);
        let span = vp + overscan * 2.0;
        match self.layout {
            ItemLayout::Stack { .. } => {
                match self.repeater.visible_range(span, lo) {
                    Some((a, b)) => (a..=b).collect(),
                    None => Vec::new(),
                }
            }
            ItemLayout::UniformGrid { .. } => {
                let stride = self.stride();
                if stride <= 0.0 {
                    return Vec::new();
                }
                let columns = self.columns_clamped();
                let first_row = (lo / stride).floor() as usize;
                let last_row = ((self.scroll.offset + vp + overscan) / stride).ceil() as usize;
                let first = (first_row * columns).min(self.count - 1);
                let last = (last_row * columns).saturating_sub(1).min(self.count - 1);
                (first..=last).collect()
            }
        }
    }

    /// 同步 `MetroRepeater` / `MetroScrollView` 的几何状态并夹紧偏移。
    fn sync_geometry(&mut self, viewport: Size) {
        self.viewport = viewport;
        self.repeater.item_count = self.count;
        match self.layout {
            ItemLayout::Stack {
                item_extent,
                spacing,
            } => {
                self.repeater.main_extent = item_extent;
                self.repeater.cross_extent = viewport.width;
                self.repeater.spacing = spacing;
            }
            ItemLayout::UniformGrid { cell, spacing } => {
                self.repeater.main_extent = cell.height;
                self.repeater.cross_extent = cell.width;
                self.repeater.spacing = spacing;
                self.repeater.columns = self.columns_clamped();
            }
        }
        self.scroll.viewport_size = viewport;
        self.scroll.content_size = Size::new(viewport.width, self.content_length());
        let max = self.scroll.max_offset();
        if self.scroll.offset > max {
            self.scroll.offset = max;
        }
    }

    /// 某条目的目标偏移（已可见返回当前偏移）。
    fn target_offset_for(&self, index: usize, viewport_h: f32) -> f32 {
        let rect = self.item_rect(index, Size::new(self.viewport.width, viewport_h), 0.0);
        let top = rect.origin.y;
        let bottom = top + rect.size.height;
        let max = self.scroll.max_offset();
        if top < self.scroll.offset {
            top.clamp(0.0, max)
        } else if bottom > self.scroll.offset + viewport_h {
            (bottom - viewport_h).clamp(0.0, max)
        } else {
            self.scroll.offset
        }
    }

    /// 把条目 `index` 滚进视口并实现出来（XAML `ItemsRepeater.GetOrCreateElement` +
    /// `StartBringIntoView` 的合流）。调用方经 `tree.edit` 触发下一帧 realize。
    pub fn bring_into_view_index(&mut self, index: usize) {
        if self.count == 0 {
            return;
        }
        let index = index.min(self.count - 1);
        let vp = self.viewport.height.max(1.0);
        let target = self.target_offset_for(index, vp);
        self.scroll.scroll_to(target, false);
        self.dirty = true;
    }
}

impl Widget for ItemsRepeater {
    /// 宽取可用宽（无界则 0）；高 = min(内容高, 可用高)，可用高无界时取内容高。
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        self.count = self.factory.as_ref().map_or(0, |f| f.len());
        self.repeater.item_count = self.count;
        let width = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            self.viewport.width
        };
        let height = if available.height.is_finite() {
            available.height.max(0.0)
        } else {
            self.content_length()
        };
        let item = self.item_size(Size::new(width, height));
        for r in self.realized.values() {
            _ctx.measure_child(r.node, item);
        }
        Size::new(width, height)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let viewport_changed = rect.size != self.viewport;
        if viewport_changed {
            self.sync_geometry(rect.size);
        }
        let entries: Vec<(usize, WidgetId)> =
            self.realized.values().map(|r| (r.index, r.node)).collect();
        for (index, node) in entries {
            let r = self.item_rect(index, rect.size, self.scroll.offset);
            ctx.arrange_child(
                node,
                Rect::new(
                    rect.origin.x + r.origin.x,
                    rect.origin.y + r.origin.y,
                    r.size.width,
                    r.size.height,
                ),
            );
        }
        if viewport_changed {
            // 视口变了 → 下一帧按新视口重新实现（首帧 `realize` 曾以表面尺寸兜底）。
            ctx.invalidate_realize();
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 滚动条叠在行之上（几何取自 `MetroScrollView`，视口本地 → 表面坐标）。
    fn paint_after(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        if !self.scroll.scrollbar_visible() {
            return;
        }
        let rect = ctx.rect();
        let t = self.scroll.scrollbar_thumb_rect();
        let thumb = Rect::new(
            rect.origin.x + t.origin.x,
            rect.origin.y + t.origin.y,
            t.size.width,
            t.size.height,
        );
        let theme = ctx.theme();
        let color = theme
            .colors
            .on_surface_variant
            .with_alpha(theme.indication.base_medium_low);
        scene.fill_rect(color, thumb);
    }

    /// 实现钩子：按视口 + overscan 决定需要哪些索引，回收离开视口的行、为新索引取池 / 新建。
    /// 参 docs/ELEMENT_TREE.md §Ⅳ-bis。
    fn wants_realize(&self) -> bool {
        true
    }

    fn realize(&mut self, ctx: &mut RealizeCtx) {
        self.count = self.factory.as_ref().map_or(0, |f| f.len());
        // 视口：优先上一帧 rect；首帧零矩形时以表面尺寸兜底（根下容器首帧即可用），
        // 容器若嵌在更小的槽位里，arrange 会再标记一次 realize 校正。
        let r = ctx.rect();
        let vp = if r.size.width > 0.0 && r.size.height > 0.0 {
            r.size
        } else {
            ctx.surface().size
        };
        if vp.width <= 0.0 || vp.height <= 0.0 {
            return;
        }
        self.sync_geometry(vp);
        if let ItemLayout::UniformGrid { cell, spacing } = self.layout {
            let cols = if cell.width + spacing > 0.0 {
                ((vp.width + spacing) / (cell.width + spacing)).floor() as i64
            } else {
                1
            };
            self.columns = cols.max(1) as usize;
            self.repeater.columns = self.columns;
        }
        // 目标索引 → 目标键。
        let needed = self.visible_indices();
        let needed_keys: HashSet<u64> = match self.factory.as_ref() {
            Some(f) => needed.iter().map(|i| f.key(*i)).collect(),
            None => HashSet::new(),
        };
        // 离开视口的行进回收池（焦点行不回收，留在原位直到失焦）。
        let mut to_pool: Vec<u64> = Vec::new();
        for (&key, entry) in self.realized.iter() {
            if !needed_keys.contains(&key) && !ctx.is_focus_related(entry.node) {
                to_pool.push(key);
            }
        }
        for key in to_pool {
            if let Some(entry) = self.realized.remove(&key) {
                ctx.set_child_visible(entry.node, false);
                self.pool.push(entry.node);
            }
        }
        // 实现目标索引：同键保留节点（必要时重绑），否则取池 / 新建。
        let dirty = self.dirty;
        let mut needed_sorted = needed;
        needed_sorted.sort_unstable();
        needed_sorted.dedup();
        for index in needed_sorted {
            let key = match self.factory.as_ref() {
                Some(f) => f.key(index),
                None => break,
            };
            let existing = self.realized.get(&key).map(|e| e.node);
            let node = match existing {
                Some(node) => {
                    let index_changed = self
                        .realized
                        .get(&key)
                        .is_some_and(|e| e.index != index);
                    if let Some(e) = self.realized.get_mut(&key) {
                        e.index = index;
                    }
                    if (index_changed || dirty)
                        && let Some(f) = self.factory.as_mut()
                    {
                        f.bind(index, ctx, node);
                    }
                    node
                }
                None => {
                    let node = if let Some(n) = self.pool.pop() {
                        if let Some(f) = self.factory.as_mut() {
                            f.bind(index, ctx, n);
                        }
                        n
                    } else if let Some(f) = self.factory.as_mut() {
                        ctx.insert_child_boxed(f.build(index))
                    } else {
                        break;
                    };
                    ctx.set_child_visible(node, true);
                    self.realized.insert(key, Realized { index, node });
                    node
                }
            };
            if !ctx.child_visible(node) {
                ctx.set_child_visible(node, true);
            }
        }
        // 池上限 = 可见（已实现）数：超出的节点真正删除（走框架清理路径）。
        let cap = self.realized.len().max(1);
        while self.pool.len() > cap {
            let n = self.pool.remove(0);
            ctx.remove_child(n);
        }
        self.dirty = false;
        // 子节点 / 偏移变化 → 下一帧重排（并让新行被量测绘制）。
        ctx.invalidate_measure();
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Scroll { dy, .. } = event {
            // 不可滚不截停：滚动链交给外层容器（XAML ScrollChaining）。
            if self.count == 0 || self.content_length() <= self.viewport.height {
                return;
            }
            let before = self.scroll.offset;
            self.scroll.scroll_wheel(*dy);
            if self.scroll.offset != before {
                ctx.invalidate_realize();
                ctx.invalidate_arrange();
                ctx.invalidate_paint();
                ctx.emit(ScrollOffsetChanged(self.scroll.offset));
            }
            ctx.set_handled();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        let before = self.scroll.offset;
        self.scroll.update(dt);
        if self.scroll.offset != before {
            ctx.invalidate_realize();
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
        }
        if self.scroll.is_animating() {
            ctx.request_anim_frame();
        }
    }

    /// 后代（行或行内子控件）获得键盘焦点时把它滚进视口。
    fn bring_into_view(&mut self, ctx: &mut EventCtx, target: Rect) -> bool {
        let rect = ctx.rect();
        let vp = rect.size.height;
        if vp <= 0.0 || self.count == 0 {
            return false;
        }
        let stride = self.stride();
        if stride <= 0.0 {
            return false;
        }
        let content_y = target.origin.y - rect.origin.y + self.scroll.offset;
        let index = (content_y / stride).floor().max(0.0) as usize;
        let index = index.min(self.count - 1);
        let before = self.scroll.offset;
        let target_off = self.target_offset_for(index, vp);
        self.scroll.scroll_to(target_off, false);
        if self.scroll.offset != before {
            ctx.invalidate_realize();
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
            ctx.emit(ScrollOffsetChanged(self.scroll.offset));
            true
        } else {
            false
        }
    }

    fn scrolls_children(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: String::from("虚拟化列表"),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    use kanesumi_core::Point;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{LayoutProps, Modifiers};

    /// 行动作：点击行（携带行当前数据索引）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    struct RowClicked(pub usize);

    /// 测试行：固定高、可聚焦、点击发动作。
    struct ListRow {
        index: usize,
        height: f32,
    }

    impl Widget for ListRow {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            Size::new(0.0, self.height)
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            scene.fill_rect(ctx.theme().colors.surface, ctx.rect());
        }
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if matches!(event, Event::Click) {
                ctx.emit(RowClicked(self.index));
                ctx.set_handled();
            }
        }
        fn focusable(&self) -> bool {
            true
        }
    }

    /// 键可由外部重排的工厂（测 `items_changed` 后按 key 复用）。
    struct SharedFactory {
        keys: Rc<RefCell<Vec<u64>>>,
        row_h: f32,
    }

    impl ItemFactory for SharedFactory {
        fn len(&self) -> usize {
            self.keys.borrow().len()
        }
        fn key(&self, index: usize) -> u64 {
            self.keys.borrow()[index]
        }
        fn build(&mut self, index: usize) -> Box<dyn Widget> {
            Box::new(ListRow {
                index,
                height: self.row_h,
            })
        }
        fn bind(&mut self, index: usize, ctx: &mut RealizeCtx, node: WidgetId) {
            ctx.edit::<ListRow, _>(node, |row, _| row.index = index);
        }
    }

    const ROW_H: f32 = 40.0;

    /// 表面即视口（repeater 撑满根），首帧 realize 就拿到真实视口。
    fn harness(count: usize, vp_h: f32) -> (TestHarness, WidgetId, Rc<RefCell<Vec<u64>>>) {
        let mut h = TestHarness::new(200.0, vp_h);
        let keys = Rc::new(RefCell::new((0..count as u64).collect()));
        let factory = SharedFactory {
            keys: keys.clone(),
            row_h: ROW_H,
        };
        let id = h.tree.insert_with(
            h.root(),
            ItemsRepeater::stack(ROW_H, factory),
            LayoutProps::default(),
        );
        h.frame();
        (h, id, keys)
    }

    #[test]
    fn realizes_only_viewport_plus_overscan() {
        let (h, id, _) = harness(10_000, 300.0);
        let rep = h.tree.get::<ItemsRepeater>(id).unwrap();
        let n = rep.realized_count();
        assert!((8..=20).contains(&n), "只实现可见 + overscan，实际 {n}");
        assert_eq!(h.tree.children(id).len(), n, "子节点数 = 已实现数");
    }

    #[test]
    fn scrolling_does_not_grow_node_count() {
        let (mut h, id, _) = harness(10_000, 300.0);
        let before = h.tree.children(id).len();
        for _ in 0..100 {
            h.tree
                .scroll(Point::new(100.0, 150.0), 0.0, 50.0, Modifiers::NONE);
            h.frame();
        }
        let after = h.tree.children(id).len();
        assert!(after <= before + 4, "滚动后节点数不涨：{before} → {after}");
        assert!(h.tree.get::<ItemsRepeater>(id).unwrap().offset() > 0.0);
    }

    #[test]
    fn pool_reuses_nodes_instead_of_allocating() {
        let (mut h, id, _) = harness(10_000, 300.0);
        let initial = h.tree.children(id).len();
        let first_id = h
            .tree
            .get::<ItemsRepeater>(id)
            .unwrap()
            .realized_node(0)
            .unwrap();
        for _ in 0..200 {
            h.tree
                .scroll(Point::new(100.0, 150.0), 0.0, 50.0, Modifiers::NONE);
            h.frame();
        }
        assert!(h.tree.contains(first_id), "回收池保留节点，不删除");
        h.tree.edit::<ItemsRepeater, _>(id, |r, ctx| {
            r.scroll_to(0.0, false);
            ctx.invalidate_measure();
        });
        h.frame();
        let after = h.tree.children(id).len();
        assert!(
            after <= initial + 4,
            "回顶部复用池中节点、总数有界：{initial} → {after}"
        );
    }

    #[test]
    fn focused_row_is_not_recycled_when_scrolled_away() {
        let (mut h, id, _) = harness(10_000, 300.0);
        let row0 = h
            .tree
            .get::<ItemsRepeater>(id)
            .unwrap()
            .realized_node(0)
            .unwrap();
        assert!(h.tree.focus(row0, true), "行可聚焦");
        h.frame();
        for _ in 0..200 {
            h.tree
                .scroll(Point::new(100.0, 150.0), 0.0, 50.0, Modifiers::NONE);
            h.frame();
        }
        assert!(h.tree.contains(row0), "焦点行滚出视口不被回收");
        assert_eq!(h.tree.focused(), Some(row0));
        assert!(
            h.tree.get::<ItemsRepeater>(id).unwrap().is_realized(0),
            "焦点行仍在已实现集合"
        );
    }

    #[test]
    fn items_changed_reuses_nodes_by_key() {
        let (mut h, id, keys) = harness(100, 300.0);
        let node_for_key5 = {
            let rep = h.tree.get::<ItemsRepeater>(id).unwrap();
            assert!(rep.is_realized(5));
            rep.realized_node(5).unwrap()
        };
        // 重排：键 5 移到索引 1。
        keys.borrow_mut().swap(1, 5);
        h.tree.edit::<ItemsRepeater, _>(id, |r, ctx| {
            r.items_changed();
            ctx.invalidate_measure();
        });
        h.frame();
        let rep = h.tree.get::<ItemsRepeater>(id).unwrap();
        assert_eq!(
            rep.realized_node(1),
            Some(node_for_key5),
            "键 5 的节点复用到新索引 1"
        );
        assert_eq!(
            h.tree.get::<ListRow>(node_for_key5).unwrap().index,
            1,
            "节点已重绑显示索引 1 的数据"
        );
    }

    #[test]
    fn click_reaches_row_widget() {
        let (mut h, id, _) = harness(100, 300.0);
        let node = h
            .tree
            .get::<ItemsRepeater>(id)
            .unwrap()
            .realized_node(2)
            .unwrap();
        h.click(node);
        assert_eq!(h.take::<RowClicked>(), vec![(node, RowClicked(2))]);
    }

    #[test]
    fn bring_into_view_realizes_offscreen_target() {
        let (mut h, id, _) = harness(10_000, 300.0);
        assert!(
            !h.tree.get::<ItemsRepeater>(id).unwrap().is_realized(9000),
            "目标行初始未实现"
        );
        h.tree.edit::<ItemsRepeater, _>(id, |r, ctx| {
            r.bring_into_view_index(9000);
            ctx.invalidate_measure();
        });
        h.frame();
        let rep = h.tree.get::<ItemsRepeater>(id).unwrap();
        assert!(rep.is_realized(9000), "目标行已实现");
        let node = rep.realized_node(9000).unwrap();
        let view = h.rect(id);
        let r = h.rect(node);
        assert!(
            r.origin.y >= view.origin.y - 0.01 && r.bottom() <= view.bottom() + 0.01,
            "目标行在视口内 {r:?} ⊄ {view:?}"
        );
    }

    #[test]
    fn wheel_emits_scroll_offset_changed() {
        let (mut h, id, _) = harness(10_000, 300.0);
        h.tree
            .scroll(Point::new(100.0, 150.0), 0.0, 50.0, Modifiers::NONE);
        h.frame();
        let actions = h.take::<ScrollOffsetChanged>();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].0, id);
        assert!((actions[0].1.0 - 50.0).abs() < 0.01, "偏移 50");
    }
}



