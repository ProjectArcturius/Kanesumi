// MetroTreeView —— 层级树。参 CONTROL_SPEC §27。
//
// 移植自 microsoft-ui-xaml/dev/TreeView（TreeViewItem.cpp + TreeView_themeresources.xaml）：
// - Item MinHeight 28；缩进 depth×16（UpdateIndentation `depth * 16`）；
// - chevron 16px：折叠 chevron_right / 展开 chevron_down（翻转 0.1s）；
// - Selected/PointerOver 底 = 白 15%（SubtleFillColorSecondary）。
// 子项展开显示逻辑由 `visible_rows` 展平。

use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size};

/// 行高（TreeViewItemMinHeight = 28）。
pub const TREE_ITEM_H: f32 = 28.0;
/// 每级缩进（depth×16）。
pub const TREE_INDENT: f32 = 16.0;
/// chevron 边长（16）。
pub const TREE_CHEVRON: f32 = 16.0;

/// 树节点。
#[derive(Debug, Clone, PartialEq)]
pub struct TreeViewNode {
    pub label: String,
    pub children: Vec<TreeViewNode>,
    pub expanded: bool,
}

impl TreeViewNode {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            children: Vec::new(),
            expanded: false,
        }
    }

    pub fn with_children(label: impl Into<String>, children: Vec<TreeViewNode>) -> Self {
        Self {
            label: label.into(),
            children,
            expanded: false,
        }
    }
}

/// 可见行（展平后）。
#[derive(Debug, Clone, PartialEq)]
pub struct TreeRow {
    pub label: String,
    pub depth: usize,
    pub has_children: bool,
    pub expanded: bool,
    /// 该行在根下的路径（索引序列）。
    pub path: Vec<usize>,
}

/// 树点击结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeAction {
    None,
    /// 选中行（返回路径）。
    Select(Vec<usize>),
    /// 展开/收起（返回路径）。
    Toggle(Vec<usize>),
}

/// MetroTreeView —— 层级树。参 CONTROL_SPEC §27。
#[derive(Debug, Clone)]
pub struct MetroTreeView {
    pub root: TreeViewNode,
    /// 选中路径。
    pub selected: Option<Vec<usize>>,
    /// 最近 toggle 的行路径（供 chevron 翻转动画）。
    pub toggled: Option<Vec<usize>>,
    /// hover 行路径。
    pub hovered: Option<Vec<usize>>,
    toggle_anim: MetroAnim,
    /// 滚动偏移（px）。正值 = 内容上移（显示更靠后行）。
    /// 元素树接入新增（自绘虚拟化的滚动由控件自持），旧 API 默认为 0 不受影响。
    scroll: f32,
}

impl Default for MetroTreeView {
    fn default() -> Self {
        Self {
            root: TreeViewNode::new(""),
            selected: None,
            toggled: None,
            hovered: None,
            toggle_anim: MetroAnim::new(0.1, UwpEasing::Quadratic, EasingMode::EaseOut),
            scroll: 0.0,
        }
    }
}

impl MetroTreeView {
    pub fn new(root: TreeViewNode) -> Self {
        Self {
            root,
            ..Self::default()
        }
    }

    pub fn update(&mut self, dt: f64) {
        self.toggle_anim.update(dt);
    }

    /// 展平可见行（深度优先，折叠节点不展开子项）。
    ///
    /// **性能取舍**：`collect_rows` 每行 `path.clone()`（`Vec<usize>` 深拷贝）——
    /// 深度 D、可见行 R 时总代价 `O(R · D)` 分配。典型 UWP TreeView 深度 ≤ 20，
    /// 可见行 ≤ 100 → 微不足道。深度 > 100 的极端场景应考虑 `Rc<[usize]>` 共享
    /// 或改为「借用 &[usize] + 索引对」以省去克隆。当前 API 直接返回 `Vec<TreeRow>`
    /// 对调用方最友好（无生命周期束缚），故保留此权衡。
    pub fn visible_rows(&self) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.collect_rows(&self.root, &mut Vec::new(), 0, &mut rows);
        rows
    }

    fn collect_rows(
        &self,
        node: &TreeViewNode,
        path: &mut Vec<usize>,
        depth: usize,
        out: &mut Vec<TreeRow>,
    ) {
        // 根节点 label 空 → 直接展平其子项（顶级项 depth = 0）。
        let is_root = path.is_empty() && node.label.is_empty();
        if is_root {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                self.collect_rows(child, path, depth, out);
                path.pop();
            }
            return;
        }
        out.push(TreeRow {
            label: node.label.clone(),
            depth,
            has_children: !node.children.is_empty(),
            expanded: node.expanded,
            path: path.clone(),
        });
        if node.expanded {
            for (i, child) in node.children.iter().enumerate() {
                path.push(i);
                self.collect_rows(child, path, depth + 1, out);
                path.pop();
            }
        }
    }

    /// 行几何：总尺寸 + 每行 rect。
    pub fn layout(&self, rect: Rect) -> (Size, Vec<Rect>) {
        let rows = self.visible_rows();
        let rects = rows
            .iter()
            .enumerate()
            .map(|(i, _)| {
                Rect::new(
                    rect.origin.x,
                    rect.origin.y + i as f32 * TREE_ITEM_H - self.scroll,
                    rect.size.width,
                    TREE_ITEM_H,
                )
            })
            .collect();
        (
            Size::new(rect.size.width, rows.len() as f32 * TREE_ITEM_H),
            rects,
        )
    }

    /// chevron rect（行内）。
    fn chevron_rect(&self, row_rect: Rect, depth: usize, has_children: bool) -> Option<Rect> {
        if !has_children {
            return None;
        }
        let x = row_rect.origin.x + depth as f32 * TREE_INDENT + TREE_INDENT;
        Some(Rect::new(
            x,
            row_rect.origin.y + (row_rect.size.height - TREE_CHEVRON) / 2.0,
            TREE_CHEVRON,
            TREE_CHEVRON,
        ))
    }

    /// 命中：先 chevron（toggle），再行（select）。
    pub fn hit(&self, rect: Rect, pos: Point) -> TreeAction {
        let rows = self.visible_rows();
        let (_, rects) = self.layout(rect);
        for (i, row) in rows.iter().enumerate() {
            let r = rects[i];
            if let Some(c) = self.chevron_rect(r, row.depth, row.has_children)
                && c.contains(pos)
            {
                return TreeAction::Toggle(row.path.clone());
            }
            if r.contains(pos) {
                return TreeAction::Select(row.path.clone());
            }
        }
        TreeAction::None
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.hovered = match self.hit(rect, pos) {
            TreeAction::Select(p) => Some(p),
            TreeAction::Toggle(p) => Some(p),
            TreeAction::None => None,
        };
    }

    /// 展开/收起某路径的节点。
    fn toggle_node(&mut self, path: &[usize]) -> bool {
        let mut node = &mut self.root;
        for &i in path {
            if i >= node.children.len() {
                return false;
            }
            node = &mut node.children[i];
        }
        node.expanded = !node.expanded;
        self.toggled = Some(path.to_vec());
        self.toggle_anim = MetroAnim::new(0.1, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.toggle_anim.set_target(1.0);
        true
    }

    /// 应用点击。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> TreeAction {
        match self.hit(rect, pos) {
            TreeAction::Toggle(path) => {
                self.toggle_node(&path);
                TreeAction::Toggle(path)
            }
            TreeAction::Select(path) => {
                self.selected = Some(path.clone());
                TreeAction::Select(path)
            }
            TreeAction::None => TreeAction::None,
        }
    }

    /// 渲染全部可见行。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let rows = self.visible_rows();
        let (_, rects) = self.layout(rect);
        let style = TextStyle::new(14.0, 20.0, FontWeight::Normal);

        // 容器语义 = 裁到自身矩形（2026-09-22 审计 P0-2）：展平行自 rect.origin.y 向下排，
        // 无视口上限时超出行高总和的项会画到控件之外。`rects` 应与 `rows` 等长，
        // 仍用 get() 兜底 —— 不一致输入不得 panic（审计 P0-3）。
        scene.push_clip(rect);

        for (i, row) in rows.iter().enumerate() {
            let Some(&r) = rects.get(i) else { break };
            let selected = self.selected.as_deref() == Some(row.path.as_slice());
            let hovered = self.hovered.as_deref() == Some(row.path.as_slice());

            // 底：选中用强调色 AccentLow，悬停用中性 ListLow（一手源 themeresources L1838-1841：
            // `TreeViewItemBackgroundPointerOver` = `SystemControlHighlightListLowBrush` = 10%，
            // `TreeViewItemBackgroundSelected` = `SystemControlHighlightAccent3RevealBrush`）。
            // 旧实现两态都用「白 15%」（§936 的 SubtleFill 说法），一手字典里 TreeView 行没有该档。
            if selected {
                scene.fill_rect(theme.colors.selection_tint, r);
            } else if hovered {
                scene.fill_rect(theme.indication.hover_tint, r);
            }

            // chevron
            if let Some(c) = self.chevron_rect(r, row.depth, row.has_children) {
                // 折叠 = 向右；展开 = 向下（翻转动画 0.1s 由 toggle_anim 驱动）。
                if row.expanded {
                    glyph::chevron_down(scene, c, colors.on_surface_variant);
                } else {
                    glyph::chevron_right(scene, c, colors.on_surface_variant);
                }
            }

            // 标签（缩进 + chevron 之后）：可用宽由行矩形派生，单行省略号（审计 P0-1）。
            let indent = row.depth as f32 * TREE_INDENT;
            let label_x = r.origin.x
                + indent
                + if row.has_children {
                    TREE_INDENT + TREE_CHEVRON
                } else {
                    TREE_INDENT
                };
            let label_w = (r.right() - label_x - 8.0).max(0.0);
            let fg = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            scene.label(
                row.label.clone(),
                Rect::new(
                    label_x,
                    r.origin.y + (r.size.height - style.line_height) / 2.0,
                    label_w,
                    style.line_height,
                ),
                fg,
                style,
                TextAlign::Left,
            );
        }

        scene.pop_clip();
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// 自绘虚拟化树：可见行由控件展平自画，**不**把每行变成子节点。点击行 / 方向键选中并发
// `TreeItemInvoked`；点 chevron / Right / Left 展开收起（复用旧 `handle_click` / `toggle_node`），
// 展开改变可见行数 → 量测失效。滚轮同 list：偏移未变（已到端 / 内容不足）时不截停，
// 让外层容器接着滚（XAML ScrollChaining）。

/// 元素树动作：树行被选中（点击 / 方向键）。按旧代码的节点路径（根下索引序列）表示。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeItemInvoked(pub Vec<usize>);

impl MetroTreeView {
    /// 可见行内容总高。
    fn content_height(&self) -> f32 {
        self.visible_rows().len() as f32 * TREE_ITEM_H
    }

    /// 最大滚动偏移（内容高 − 视口高，下限 0）。
    fn max_scroll(&self, viewport_h: f32) -> f32 {
        (self.content_height() - viewport_h).max(0.0)
    }

    /// 选中行在展平可见行中的序号。
    fn selected_row_index(&self) -> Option<usize> {
        let sel = self.selected.as_deref()?;
        self.visible_rows()
            .iter()
            .position(|r| r.path.as_slice() == sel)
    }

    /// 把选中行滚进视口（视口高 = 控件矩形高）。已可见时不滚。
    fn ensure_selected_visible(&mut self, viewport_h: f32) {
        let Some(i) = self.selected_row_index() else { return };
        let top = i as f32 * TREE_ITEM_H;
        let bottom = top + TREE_ITEM_H;
        let target = if top < self.scroll {
            top
        } else if bottom > self.scroll + viewport_h {
            bottom - viewport_h
        } else {
            return;
        };
        let max = self.max_scroll(viewport_h);
        self.scroll = target.clamp(0.0, max);
    }

    /// 选中行（若可展开收起）翻转展开态；返回是否变化。
    fn toggle_selected(&mut self, expand: bool) -> bool {
        let Some(path) = self.selected.clone() else { return false };
        let Some(row) = self.visible_rows().into_iter().find(|r| r.path == path) else {
            return false;
        };
        if !row.has_children || row.expanded == expand {
            return false;
        }
        self.toggle_node(&path)
    }
}

impl kanesumi_element::Widget for MetroTreeView {
    /// 宽取可用宽（无界则 0）；高 = min(内容高, 可用高)，可用高无界时取内容高。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: Size,
    ) -> Size {
        let width = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let content_h = self.content_height();
        let height = if available.height.is_finite() {
            content_h.min(available.height.max(0.0))
        } else {
            content_h
        };
        Size::new(width, height)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, dt: f64) {
        MetroTreeView::update(self, dt);
        if !self.toggle_anim.is_steady() {
            ctx.invalidate_paint();
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.hover(rect, *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => match self.handle_click(rect, *pos) {
                TreeAction::Select(path) => {
                    ctx.emit(TreeItemInvoked(path));
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
                TreeAction::Toggle(_) => {
                    ctx.invalidate_measure();
                    ctx.invalidate_paint();
                    ctx.request_anim_frame();
                    ctx.set_handled();
                }
                TreeAction::None => {}
            },
            Event::Scroll { dy, .. } => {
                let viewport_h = rect.size.height;
                let before = self.scroll;
                self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll(viewport_h));
                if self.scroll != before {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => match key {
                Key::Down | Key::Up => {
                    let rows = self.visible_rows();
                    if rows.is_empty() {
                        return;
                    }
                    let last = rows.len() - 1;
                    let next = match (key, self.selected_row_index()) {
                        (Key::Down, Some(i)) => (i + 1).min(last),
                        (Key::Down, None) => 0,
                        (Key::Up, Some(i)) => i.saturating_sub(1),
                        (Key::Up, None) => last,
                        _ => return,
                    };
                    let path = rows[next].path.clone();
                    let changed = self.selected.as_deref() != Some(path.as_slice());
                    self.selected = Some(path.clone());
                    if changed {
                        self.ensure_selected_visible(rect.size.height);
                        ctx.emit(TreeItemInvoked(path));
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                }
                Key::Right | Key::Left => {
                    if self.toggle_selected(*key == Key::Right) {
                        let max = self.max_scroll(rect.size.height);
                        self.scroll = self.scroll.min(max);
                        ctx.invalidate_measure();
                        ctx.invalidate_paint();
                        ctx.request_anim_frame();
                    }
                    ctx.set_handled();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::List,
            name: String::from("树"),
            value: self
                .selected
                .as_ref()
                .and_then(|p| self.visible_rows().into_iter().find(|r| &r.path == p))
                .map(|r| r.label),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(rows: usize, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let root = TreeViewNode::with_children(
            "",
            (0..rows)
                .map(|i| TreeViewNode::new(format!("节点 {i}")))
                .collect(),
        );
        let id = h.tree.insert_with(
            h.root(),
            MetroTreeView::new(root),
            LayoutProps {
                width: Some(200.0),
                height: Some(height),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn harness_children() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let root = TreeViewNode::with_children(
            "",
            vec![
                TreeViewNode::with_children(
                    "文档",
                    vec![TreeViewNode::new("项目"), TreeViewNode::new("报告")],
                ),
                TreeViewNode::new("图片"),
            ],
        );
        let id = h.tree.insert_with(
            h.root(),
            MetroTreeView::new(root),
            LayoutProps {
                width: Some(200.0),
                height: Some(120.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn row_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let scroll = h.tree.get::<MetroTreeView>(id).unwrap().scroll;
        let r = h.rect(id);
        Point::new(
            r.origin.x + 60.0,
            r.origin.y - scroll + i as f32 * TREE_ITEM_H + TREE_ITEM_H / 2.0,
        )
    }

    #[test]
    fn click_row_invokes_item() {
        let (mut h, id) = harness_children();
        h.click_at(row_center(&h, id, 0));
        assert_eq!(
            h.take::<TreeItemInvoked>(),
            vec![(id, TreeItemInvoked(vec![0]))]
        );
        assert_eq!(
            h.tree.get::<MetroTreeView>(id).unwrap().selected,
            Some(vec![0])
        );
    }

    #[test]
    fn click_chevron_toggles_expansion() {
        let (mut h, id) = harness_children();
        let r = h.rect(id);
        let row0 = Rect::new(r.origin.x, r.origin.y, r.size.width, TREE_ITEM_H);
        let chev = h
            .tree
            .get::<MetroTreeView>(id)
            .unwrap()
            .chevron_rect(row0, 0, true)
            .unwrap();
        h.click_at(chev.center());
        let t = h.tree.get::<MetroTreeView>(id).unwrap();
        assert!(t.root.children[0].expanded, "点箭头应展开");
        assert_eq!(t.visible_rows().len(), 4);
        // 再点一次收起。
        h.click_at(chev.center());
        let t = h.tree.get::<MetroTreeView>(id).unwrap();
        assert!(!t.root.children[0].expanded, "再点箭头应收起");
        assert_eq!(t.visible_rows().len(), 2);
    }

    #[test]
    fn keyboard_moves_selection_and_scrolls_into_view() {
        let (mut h, id) = harness(20, 100.0);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        for _ in 0..5 {
            assert!(h.key(Key::Down), "方向键应被树消费");
        }
        let t = h.tree.get::<MetroTreeView>(id).unwrap();
        assert_eq!(t.selected, Some(vec![4]));
        let top = 4.0 * TREE_ITEM_H;
        let bottom = top + TREE_ITEM_H;
        assert!(
            t.scroll <= top && bottom <= t.scroll + 100.0,
            "选中行应滚进视口：scroll={}",
            t.scroll
        );
        assert!(t.scroll > 0.0, "第 4 行视口外应滚动");
        assert_eq!(h.take::<TreeItemInvoked>().len(), 5);
    }

    #[test]
    fn arrow_right_expands_left_collapses() {
        let (mut h, id) = harness_children();
        h.tab();
        h.key(Key::Down); // 选中 [0] 文档
        assert_eq!(
            h.tree.get::<MetroTreeView>(id).unwrap().selected,
            Some(vec![0])
        );
        assert!(h.key(Key::Right), "Right 应被树消费");
        assert!(h.tree.get::<MetroTreeView>(id).unwrap().root.children[0].expanded);
        assert!(h.key(Key::Left));
        assert!(!h.tree.get::<MetroTreeView>(id).unwrap().root.children[0].expanded);
        h.settle();
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(20, 100.0);
        assert_eq!(h.rect(id).size.width, 200.0);
        assert_eq!(h.rect(id).size.height, 100.0, "高 = min(内容 560, 可用 100)");
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        let mut h = TestHarness::new(400.0, 300.0);
        let root = TreeViewNode::with_children(
            "",
            vec![TreeViewNode::with_children(
                "很长很长很长很长很长的节点",
                vec![TreeViewNode::new("子节点也很长很长很长")],
            )],
        );
        let id = h.tree.insert_with(
            h.root(),
            MetroTreeView::new(root),
            LayoutProps {
                width: Some(40.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn wheel_bubbles_when_content_fits() {
        // 内容不足一屏 → 滚轮不截停，外层接到（ScrollChaining）。
        let mut h = TestHarness::new(300.0, 300.0);
        let sv = h.tree.insert_with(
            h.root(),
            crate::scroll_view::MetroScrollView::default(),
            LayoutProps {
                width: Some(200.0),
                height: Some(100.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let col = h.tree.insert(sv, kanesumi_element::widgets::Stack::column());
        let tree = h.tree.insert_with(
            col,
            MetroTreeView::new(TreeViewNode::with_children(
                "",
                vec![TreeViewNode::new("唯一")],
            )),
            LayoutProps {
                height: Some(28.0),
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(
            col,
            crate::button::MetroButton::new("高内容"),
            LayoutProps {
                height: Some(300.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        let t = h.tree.get::<MetroTreeView>(tree).unwrap();
        assert_eq!(t.scroll, 0.0);
        h.tree.scroll(
            h.rect(tree).center(),
            0.0,
            50.0,
            kanesumi_element::Modifiers::NONE,
        );
        h.frame();
        assert_eq!(h.tree.get::<MetroTreeView>(tree).unwrap().scroll, 0.0);
        assert!(
            h.tree.get::<crate::scroll_view::MetroScrollView>(sv).unwrap().offset > 0.0,
            "滚动链应冒泡到外层"
        );
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness_children();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(row_center(&h, id, 0));
        assert!(h.take::<TreeItemInvoked>().is_empty());
        assert_eq!(h.tree.get::<MetroTreeView>(id).unwrap().selected, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn tree() -> MetroTreeView {
        MetroTreeView::new(TreeViewNode::with_children(
            "",
            vec![
                TreeViewNode::with_children(
                    "文档",
                    vec![TreeViewNode::new("项目"), TreeViewNode::new("报告")],
                ),
                TreeViewNode::new("图片"),
            ],
        ))
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 300.0, 200.0)
    }

    #[test]
    fn flattened_rows_collapsed() {
        let t = tree();
        let rows = t.visible_rows();
        // 根空 label → 直接子项：文档(折叠) + 图片，顶级 depth=0
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "文档");
        assert_eq!(rows[0].depth, 0);
        assert!(rows[0].has_children);
        assert_eq!(rows[1].label, "图片");
    }

    #[test]
    fn expand_reveals_children() {
        let mut t = tree();
        t.toggle_node(&[0]);
        let rows = t.visible_rows();
        assert_eq!(rows.len(), 4, "展开后 +2 子项");
        assert_eq!(rows[1].label, "项目");
        assert_eq!(rows[1].depth, 1);
    }

    #[test]
    fn select_returns_path() {
        let mut t = tree();
        t.toggle_node(&[0]);
        let (_, rects) = t.layout(area());
        let row2 = rects[1]; // 项目
        assert_eq!(
            t.handle_click(area(), row2.center()),
            TreeAction::Select(vec![0, 0])
        );
        assert_eq!(t.selected.as_deref(), Some(vec![0, 0].as_slice()));
    }

    #[test]
    fn toggle_via_chevron() {
        let mut t = tree();
        let (_, rects) = t.layout(area());
        let row0 = rects[0];
        let chevron = t.chevron_rect(row0, 0, true).unwrap();
        assert_eq!(
            t.handle_click(area(), chevron.center()),
            TreeAction::Toggle(vec![0])
        );
        assert_eq!(t.root.children[0].expanded, true);
        assert_eq!(t.visible_rows().len(), 4);
    }

    #[test]
    fn indent_scales_with_depth() {
        let row = TreeRow {
            label: "x".into(),
            depth: 2,
            has_children: false,
            expanded: false,
            path: vec![0, 0],
        };
        let r = Rect::new(0.0, 0.0, 300.0, 28.0);
        // label_x = 0 + 2*16 + 16 = 48
        let label_x = r.origin.x + row.depth as f32 * TREE_INDENT + TREE_INDENT;
        assert_eq!(label_x, 48.0, "缩进 depth×16");
    }

    #[test]
    fn render_emits_rows_and_chevrons() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut t = tree();
        t.toggle_node(&[0]);
        t.update(1.0);
        let mut scene = Scene::default();
        t.render(&theme, &engine, area(), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 4, "4 行标签");
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 1, "仅 文档 有 chevron（展开态 1 个三角形）");
    }

    #[test]
    fn collapsed_hides_subtree() {
        let t = tree();
        let (_, rects) = t.layout(area());
        assert_eq!(rects.len(), 2, "折叠时只渲染可见行");
    }
}
