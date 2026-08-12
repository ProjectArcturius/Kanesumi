// layout.rs —— UWP Measure/Arrange 两遍布局引擎（对应 UWP LayoutManager）。
//
// 参 PLAN.md §4.5 与 decl.rs：本引擎是**布局唯一真源**。渲染、命中、裁剪全部
// 从 `layout()` 产出的 LaidTree 派生 —— 消灭「画的位置」与「点得到的位置」
// 两套数值的漂移（Ether-main 开发史：collect_hits 均分 vs render_decl 内在尺寸，
// 两套算法 → 点击错位；控件 measure 与 emit_text 各自换行 → 文字溢出/换行 bug）。
//
// UWP 契约（参 SD/AnimationRules）：
// 1. **Measure 一遍**：父给子可用约束 `available`，子返回期望尺寸 `DesiredSize`；
// 2. **Arrange 一遍**：父给子最终矩形 `rect`，子（递归）展开自身子树；
// 3. 两遍均以 `TextEngine` 排版（`TextEngine::layout` 唯一真源）→ 量测即排版；
// 4. 布局树产物同时驱动：Scene 命令 / 命中表 / 裁剪矩形（box 语义）。

use kanesumi_core::geometry::{Point, Rect, Size};
use kanesumi_core::theme::MetroTheme;

use crate::scene::Scene;
use crate::text::TextEngine;

/// 双轴布局约束。对应 UWP `availableSize` 与 macOS intrinsic size 的上下界。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min: Size,
    pub max: Size,
}

impl Constraints {
    pub const fn new(min: Size, max: Size) -> Self {
        Self { min, max }
    }

    pub const fn loose(max: Size) -> Self {
        Self::new(Size::ZERO, max)
    }

    pub const fn tight(size: Size) -> Self {
        Self::new(size, size)
    }

    pub const fn unbounded() -> Self {
        Self::loose(Size::new(f32::INFINITY, f32::INFINITY))
    }

    pub fn normalized(self) -> Self {
        let min = self.min.normalized();
        let max = self.max.normalized();
        Self::new(
            Size::new(min.width.min(max.width), min.height.min(max.height)),
            max,
        )
    }

    pub fn constrain(self, size: Size) -> Size {
        let constraints = self.normalized();
        let size = size.normalized();
        Size::new(
            size.width
                .clamp(constraints.min.width, constraints.max.width),
            size.height
                .clamp(constraints.min.height, constraints.max.height),
        )
    }
}

/// 交叉轴对齐 —— 等价 UWP `VerticalAlignment`/`HorizontalAlignment` 简化版。
/// 容器子元素沿交叉轴（Row → Y，Column → X）的对齐方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossAlign {
    /// 撑满交叉轴（等价 `Stretch`，UWP 默认）。Kanesumi 默认。
    #[default]
    Stretch,
    /// 起始端对齐（Row → 顶部，Column → 左缘）。
    Start,
    /// 交叉轴居中。
    Center,
    /// 末端对齐（Row → 底部，Column → 右缘）。
    End,
}

/// 布局叶子 —— 宿主（控件层）实现的可布局可绘制单元。
///
/// 等价 UWP `FrameworkElement` 的 Measure/Arrange 契约：
/// - `measure` 返回给定约束下的期望尺寸（内容 + padding）；
/// - `render` 在 arrange 后的最终矩形内自绘。
///
/// 叶子不自知位置 —— 位置由容器布局决定，命中由 LaidTree 统一派生。
pub trait LayoutLeaf: Clone + PartialEq {
    /// Measure：给定可用约束，返回期望尺寸。
    fn measure(&self, engine: &TextEngine, available: Size) -> Size;
    /// Arrange 后的渲染：在 `rect` 内绘制到 scene。
    fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene);
}

/// 布局节点 —— 声明式控件树（纯数据）。`L` 为叶子类型。
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutNode<L: LayoutLeaf> {
    /// 水平容器：主轴 = X（左→右）。
    Row {
        spacing: f32,
        cross: CrossAlign,
        children: Vec<LayoutNode<L>>,
    },
    /// 垂直容器：主轴 = Y（上→下）。
    Column {
        spacing: f32,
        cross: CrossAlign,
        children: Vec<LayoutNode<L>>,
    },
    /// 叶子（Button/Text/…）。
    Leaf(L),
    /// 弹性占位：arrange 时按 `grow` 权重分掉容器主轴剩余空间。
    Spacer { grow: f32 },
    /// 子树弹性策略。`grow` 消费剩余空间，`shrink` 在不足时参与压缩，`min_main`
    /// 是压缩下限。对应 UWP Star sizing 与 AppKit compression resistance 的合流。
    Flexible {
        grow: f32,
        shrink: f32,
        min_main: f32,
        child: Box<LayoutNode<L>>,
    },
}

impl<L: LayoutLeaf> LayoutNode<L> {
    pub fn row(children: Vec<LayoutNode<L>>) -> Self {
        LayoutNode::Row {
            spacing: 8.0,
            cross: CrossAlign::Stretch,
            children,
        }
    }

    pub fn column(children: Vec<LayoutNode<L>>) -> Self {
        LayoutNode::Column {
            spacing: 8.0,
            cross: CrossAlign::Stretch,
            children,
        }
    }

    pub fn row_with(spacing: f32, cross: CrossAlign, children: Vec<LayoutNode<L>>) -> Self {
        LayoutNode::Row {
            spacing,
            cross,
            children,
        }
    }

    pub fn column_with(spacing: f32, cross: CrossAlign, children: Vec<LayoutNode<L>>) -> Self {
        LayoutNode::Column {
            spacing,
            cross,
            children,
        }
    }

    pub fn leaf(l: L) -> Self {
        LayoutNode::Leaf(l)
    }

    pub fn spacer(grow: f32) -> Self {
        LayoutNode::Spacer { grow }
    }

    pub fn flexible(child: LayoutNode<L>, grow: f32, shrink: f32, min_main: f32) -> Self {
        LayoutNode::Flexible {
            grow: grow.max(0.0),
            shrink: shrink.max(0.0),
            min_main: min_main.max(0.0),
            child: Box::new(child),
        }
    }
}

/// 布局树产物 —— 单次 `layout()` 的扁平节点表（DFS 先序）。
///
/// **唯一真源**：`render`（绘制命令）、`hit_at`（命中）、容器 Clip（box 语义）
/// 全部从本树派生，不存在第二套 rect 计算。
#[derive(Debug, Clone, PartialEq)]
pub struct LaidTree<L: LayoutLeaf> {
    nodes: Vec<LaidNode<L>>,
    root: usize,
}

/// 布局树节点：arrange 后的最终矩形 + 种类（容器/叶子）。
#[derive(Debug, Clone, PartialEq)]
pub struct LaidNode<L: LayoutLeaf> {
    /// arrange 结果 —— 渲染 / 命中 / 裁剪共用。
    pub rect: Rect,
    pub kind: LaidKind<L>,
}

/// 布局树节点种类。
#[derive(Debug, Clone, PartialEq)]
pub enum LaidKind<L: LayoutLeaf> {
    /// 容器：`children` 为直接子节点在扁平表中的索引。
    Container { children: Vec<usize> },
    /// 叶子（可命中）。
    Leaf(L),
    /// 弹性占位（无绘制、不可命中，但占有已分配矩形）。
    Spacer,
}

/// 布局：两遍（Measure → Arrange）产出布局树。
///
/// `root` 在 `root_rect` 内展开。容器约束沿树下传（子可用空间 = 父分配矩形），
/// 期望尺寸沿树回收（父按内在尺寸 + spacing + Spacer.grow 分配主轴）。
pub fn layout<L: LayoutLeaf>(
    root: &LayoutNode<L>,
    engine: &TextEngine,
    root_rect: Rect,
) -> LaidTree<L> {
    let root_rect = root_rect.normalized();
    let mut nodes = Vec::new();
    let root_idx = arrange(root, engine, root_rect, &mut nodes);
    LaidTree {
        nodes,
        root: root_idx,
    }
}

impl<L: LayoutLeaf> LaidTree<L> {
    /// 渲染：DFS 遍历布局树。容器以自身矩形为 Clip（box 语义），叶子自绘。
    /// 与 `hit_at` 共用同一树 → 画在哪里，点就得在哪。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        self.render_rec(self.root, theme, engine, scene);
    }

    fn render_rec(&self, idx: usize, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let node = &self.nodes[idx];
        match &node.kind {
            LaidKind::Container { children } => {
                // box 语义：内容裁剪到容器矩形内（参 scene.rs PushClip）。
                scene.push_clip(node.rect);
                for &c in children {
                    self.render_rec(c, theme, engine, scene);
                }
                scene.pop_clip();
            }
            LaidKind::Leaf(l) => l.render(theme, engine, node.rect, scene),
            LaidKind::Spacer => {}
        }
    }

    /// 命中测试：从根递归，沿祖先 Clip 累积有效裁剪；**后画者优先**
    /// （容器内子元素在父之后，命中其矩形即命中）。返回被点中的叶子。
    pub fn hit_at(&self, pos: Point) -> Option<&L> {
        self.hit_rec(self.root, pos, None)
    }

    fn hit_rec(&self, idx: usize, pos: Point, clip: Option<Rect>) -> Option<&L> {
        let node = &self.nodes[idx];
        // 有效裁剪 = 祖先 Clip ∩ 本节点矩形（容器裁子内容）。
        let effective = match clip {
            Some(c) => c.intersect(node.rect),
            None => Some(node.rect),
        }?;
        if !effective.contains(pos) {
            return None;
        }
        match &node.kind {
            LaidKind::Container { children } => {
                for &c in children.iter().rev() {
                    if let Some(h) = self.hit_rec(c, pos, Some(effective)) {
                        return Some(h);
                    }
                }
                None
            }
            LaidKind::Leaf(l) => Some(l),
            LaidKind::Spacer => None,
        }
    }

    /// 根节点索引。
    pub fn root(&self) -> usize {
        self.root
    }

    /// 全部布局节点（渲染 / 调试用）。
    pub fn nodes(&self) -> &[LaidNode<L>] {
        &self.nodes
    }

    /// DFS 序可见叶子：矩形已与全部祖先容器裁剪求交。命中表不得包含不可见区域。
    pub fn leaves(&self) -> impl Iterator<Item = (Rect, &L)> {
        let mut leaves = Vec::new();
        self.collect_visible_leaves(self.root, None, &mut leaves);
        leaves.into_iter()
    }

    fn collect_visible_leaves<'a>(
        &'a self,
        idx: usize,
        clip: Option<Rect>,
        out: &mut Vec<(Rect, &'a L)>,
    ) {
        let node = &self.nodes[idx];
        let Some(effective) = (match clip {
            Some(parent) => parent.intersect(node.rect),
            None => Some(node.rect),
        }) else {
            return;
        };
        match &node.kind {
            LaidKind::Container { children } => {
                for &child in children {
                    self.collect_visible_leaves(child, Some(effective), out);
                }
            }
            LaidKind::Leaf(leaf) => out.push((effective, leaf)),
            LaidKind::Spacer => {}
        }
    }
}

/// Arrange：把 `node` 展开进 `rect`，产出扁平节点表；返回节点索引。
fn arrange<L: LayoutLeaf>(
    node: &LayoutNode<L>,
    engine: &TextEngine,
    rect: Rect,
    out: &mut Vec<LaidNode<L>>,
) -> usize {
    let idx = out.len();
    match node {
        LayoutNode::Row {
            spacing,
            cross,
            children,
        } => {
            out.push(LaidNode {
                rect,
                kind: LaidKind::Container { children: vec![] },
            });
            // 量测：每个子以 (主轴无界, 交叉轴 = 父交叉) 量内在尺寸。
            let mut measured = Vec::with_capacity(children.len());
            for c in children {
                match c {
                    LayoutNode::Spacer { .. } => measured.push(Size::ZERO),
                    _ => {
                        measured.push(measure(
                            c,
                            engine,
                            Size::new(f32::INFINITY, rect.size.height),
                        ));
                    }
                }
            }
            let gaps = spacing * children.len().saturating_sub(1) as f32;
            let widths = distribute_main(
                children,
                &measured.iter().map(|s| s.width).collect::<Vec<_>>(),
                (rect.size.width - gaps).max(0.0),
            );

            let mut cursor = rect.origin.x;
            let mut child_indices = Vec::with_capacity(children.len());
            for (i, child) in children.iter().enumerate() {
                let w = widths[i];
                let arranged = measure(child, engine, Size::new(w, rect.size.height));
                let (ch, cy) = match cross {
                    CrossAlign::Stretch => (rect.size.height, rect.origin.y),
                    CrossAlign::Start => (arranged.height.min(rect.size.height), rect.origin.y),
                    CrossAlign::Center => (
                        arranged.height.min(rect.size.height),
                        rect.origin.y
                            + (rect.size.height - arranged.height.min(rect.size.height)) / 2.0,
                    ),
                    CrossAlign::End => (
                        arranged.height.min(rect.size.height),
                        rect.origin.y + rect.size.height - arranged.height.min(rect.size.height),
                    ),
                };
                let child_rect = Rect::new(cursor, cy, w, ch);
                child_indices.push(arrange(child, engine, child_rect, out));
                cursor += w + spacing;
            }
            if let LaidKind::Container { children } = &mut out[idx].kind {
                *children = child_indices;
            }
        }
        LayoutNode::Column {
            spacing,
            cross,
            children,
        } => {
            out.push(LaidNode {
                rect,
                kind: LaidKind::Container { children: vec![] },
            });
            let mut measured = Vec::with_capacity(children.len());
            for c in children {
                match c {
                    LayoutNode::Spacer { .. } => measured.push(Size::ZERO),
                    _ => {
                        measured.push(measure(
                            c,
                            engine,
                            Size::new(rect.size.width, f32::INFINITY),
                        ));
                    }
                }
            }
            let gaps = spacing * children.len().saturating_sub(1) as f32;
            let heights = distribute_main(
                children,
                &measured.iter().map(|s| s.height).collect::<Vec<_>>(),
                (rect.size.height - gaps).max(0.0),
            );

            let mut cursor = rect.origin.y;
            let mut child_indices = Vec::with_capacity(children.len());
            for (i, child) in children.iter().enumerate() {
                let h = heights[i];
                let arranged = measure(child, engine, Size::new(rect.size.width, h));
                let (cw, cx) = match cross {
                    CrossAlign::Stretch => (rect.size.width, rect.origin.x),
                    CrossAlign::Start => (arranged.width.min(rect.size.width), rect.origin.x),
                    CrossAlign::Center => (
                        arranged.width.min(rect.size.width),
                        rect.origin.x
                            + (rect.size.width - arranged.width.min(rect.size.width)) / 2.0,
                    ),
                    CrossAlign::End => (
                        arranged.width.min(rect.size.width),
                        rect.origin.x + rect.size.width - arranged.width.min(rect.size.width),
                    ),
                };
                let child_rect = Rect::new(cx, cursor, cw, h);
                child_indices.push(arrange(child, engine, child_rect, out));
                cursor += h + spacing;
            }
            if let LaidKind::Container { children } = &mut out[idx].kind {
                *children = child_indices;
            }
        }
        LayoutNode::Leaf(l) => {
            out.push(LaidNode {
                rect,
                kind: LaidKind::Leaf(l.clone()),
            });
        }
        LayoutNode::Spacer { .. } => {
            out.push(LaidNode {
                rect,
                kind: LaidKind::Spacer,
            });
        }
        LayoutNode::Flexible { child, .. } => return arrange(child, engine, rect, out),
    }
    idx
}

fn distribute_main<L: LayoutLeaf>(
    children: &[LayoutNode<L>],
    desired: &[f32],
    available: f32,
) -> Vec<f32> {
    let mut sizes: Vec<f32> = desired.iter().map(|v| v.max(0.0)).collect();
    let total: f32 = sizes.iter().sum();
    if total < available {
        let grow_total: f32 = children.iter().map(main_grow).sum();
        if grow_total > 0.0 {
            let extra = available - total;
            for (size, child) in sizes.iter_mut().zip(children) {
                *size += extra * main_grow(child) / grow_total;
            }
        }
        return sizes;
    }

    let mut deficit = total - available;
    let mut active = vec![true; children.len()];
    while deficit > 0.001 {
        let weight_total: f32 = children
            .iter()
            .enumerate()
            .filter(|(i, _)| active[*i])
            .map(|(i, child)| main_shrink(child) * (sizes[i] - main_min(child)).max(0.0))
            .sum();
        if weight_total <= 0.0 {
            break;
        }
        let before = deficit;
        for (i, child) in children.iter().enumerate() {
            if !active[i] {
                continue;
            }
            let capacity = (sizes[i] - main_min(child)).max(0.0);
            let weight = main_shrink(child) * capacity;
            let reduction = (before * weight / weight_total).min(capacity);
            sizes[i] -= reduction;
            deficit -= reduction;
            if capacity - reduction <= 0.001 {
                active[i] = false;
            }
        }
        if (before - deficit).abs() <= 0.001 {
            break;
        }
    }
    sizes
}

fn main_grow<L: LayoutLeaf>(node: &LayoutNode<L>) -> f32 {
    match node {
        LayoutNode::Spacer { grow } | LayoutNode::Flexible { grow, .. } => grow.max(0.0),
        _ => 0.0,
    }
}

fn main_shrink<L: LayoutLeaf>(node: &LayoutNode<L>) -> f32 {
    match node {
        LayoutNode::Spacer { .. } => 0.0,
        LayoutNode::Flexible { shrink, .. } => shrink.max(0.0),
        _ => 1.0,
    }
}

fn main_min<L: LayoutLeaf>(node: &LayoutNode<L>) -> f32 {
    match node {
        LayoutNode::Flexible { min_main, .. } => min_main.max(0.0),
        _ => 0.0,
    }
}

/// Measure：给定约束，返回期望尺寸（父布局用）。叶子委托 `LayoutLeaf::measure`。
fn measure<L: LayoutLeaf>(node: &LayoutNode<L>, engine: &TextEngine, available: Size) -> Size {
    let available = available.normalized();
    let measured = match node {
        LayoutNode::Row {
            spacing, children, ..
        } => {
            let mut w: f32 = 0.0;
            let mut h: f32 = 0.0;
            for (i, c) in children.iter().enumerate() {
                let s = measure(c, engine, Size::new(f32::INFINITY, available.height));
                if i > 0 {
                    w += spacing;
                }
                w += s.width;
                h = h.max(s.height);
            }
            Size::new(w, h)
        }
        LayoutNode::Column {
            spacing, children, ..
        } => {
            let mut w: f32 = 0.0;
            let mut h: f32 = 0.0;
            for (i, c) in children.iter().enumerate() {
                let s = measure(c, engine, Size::new(available.width, f32::INFINITY));
                if i > 0 {
                    h += spacing;
                }
                h += s.height;
                w = w.max(s.width);
            }
            Size::new(w, h)
        }
        LayoutNode::Leaf(l) => l.measure(engine, available),
        LayoutNode::Spacer { .. } => Size::ZERO,
        LayoutNode::Flexible { child, .. } => measure(child, engine, available),
    };
    Constraints::loose(available).constrain(measured)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::TextAlign;
    use kanesumi_core::Color;

    /// 测试叶子：固定尺寸 + 可选的标签宽度（模拟 Text）。
    #[derive(Debug, Clone, PartialEq)]
    struct FixedLeaf {
        w: f32,
        h: f32,
        tag: &'static str,
    }

    impl LayoutLeaf for FixedLeaf {
        fn measure(&self, _engine: &TextEngine, _available: Size) -> Size {
            Size::new(self.w, self.h)
        }
        fn render(&self, _theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
            scene.fill_rect(Color::WHITE, rect);
        }
    }

    fn leaf(w: f32, h: f32, tag: &'static str) -> LayoutNode<FixedLeaf> {
        LayoutNode::Leaf(FixedLeaf { w, h, tag })
    }

    fn engine_or_skip() -> Option<TextEngine> {
        for p in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn row_places_children_left_to_right() {
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::row(vec![leaf(40.0, 20.0, "a"), leaf(60.0, 20.0, "b")]);
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 200.0, 40.0));
        let leaves: Vec<&FixedLeaf> = tree
            .nodes()
            .iter()
            .filter_map(|n| match &n.kind {
                LaidKind::Leaf(l) if l.w > 0.0 => Some(l),
                _ => None,
            })
            .collect();
        assert_eq!(leaves.len(), 2);
        // 叶子 a 在 0..40，叶子 b 在 48..108（spacing 8）
        let a = tree.hit_at(Point::new(20.0, 20.0)).expect("点中 a");
        assert_eq!(a.tag, "a");
        let b = tree.hit_at(Point::new(70.0, 20.0)).expect("点中 b");
        assert_eq!(b.tag, "b");
        assert!(
            tree.hit_at(Point::new(150.0, 20.0)).is_none(),
            "spacing 空隙不可命中"
        );
    }

    #[test]
    fn column_places_children_top_down() {
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::column(vec![leaf(100.0, 20.0, "a"), leaf(100.0, 30.0, "b")]);
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 200.0, 100.0));
        let a = tree.hit_at(Point::new(50.0, 10.0)).expect("点中 a");
        assert_eq!(a.tag, "a");
        let b = tree
            .hit_at(Point::new(50.0, 40.0))
            .expect("点中 b（含 spacing）");
        assert_eq!(b.tag, "b");
    }

    #[test]
    fn container_clips_hidden_children() {
        // 容器宽 100，第一个子撑满 100，第二个子挤出去 —— 但容器 Clip 使其不可命中
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::Row {
            spacing: 0.0,
            cross: CrossAlign::Stretch,
            children: vec![leaf(100.0, 20.0, "a"), leaf(100.0, 20.0, "b")],
        };
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 100.0, 20.0));
        // a 在 0..100 可命中；b 在 100..200 被容器 rect 裁剪 → 不可命中
        assert!(tree.hit_at(Point::new(50.0, 10.0)).is_some());
        assert!(
            tree.hit_at(Point::new(150.0, 10.0)).is_none(),
            "容器裁剪子内容"
        );
    }

    #[test]
    fn spacer_distributes_remaining() {
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::Row {
            spacing: 0.0,
            cross: CrossAlign::Stretch,
            children: vec![
                leaf(40.0, 20.0, "a"),
                LayoutNode::spacer(1.0),
                leaf(40.0, 20.0, "b"),
            ],
        };
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 200.0, 20.0));
        // a: 0..40，spacer: 40..160，b: 160..200
        let a = tree.hit_at(Point::new(10.0, 10.0)).expect("a");
        assert_eq!(a.tag, "a");
        let b = tree.hit_at(Point::new(180.0, 10.0)).expect("b");
        assert_eq!(b.tag, "b");
        assert!(
            tree.hit_at(Point::new(100.0, 10.0)).is_none(),
            "spacer 不可命中"
        );
    }

    #[test]
    fn row_shrinks_children_instead_of_arranging_past_parent() {
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::Row {
            spacing: 8.0,
            cross: CrossAlign::Stretch,
            children: vec![leaf(100.0, 20.0, "a"), leaf(100.0, 20.0, "b")],
        };
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 120.0, 20.0));
        let leaves: Vec<_> = tree.leaves().collect();
        assert_eq!(leaves.len(), 2);
        assert!((leaves[0].0.size.width - 56.0).abs() < 0.01);
        assert!((leaves[1].0.size.width - 56.0).abs() < 0.01);
        assert!(leaves[1].0.right() <= 120.0);
    }

    #[test]
    fn flexible_minimum_models_compression_resistance() {
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let root = LayoutNode::row_with(
            0.0,
            CrossAlign::Stretch,
            vec![
                LayoutNode::flexible(leaf(100.0, 20.0, "resists"), 0.0, 1.0, 80.0),
                LayoutNode::flexible(leaf(100.0, 20.0, "shrinks"), 0.0, 1.0, 0.0),
            ],
        );
        let tree = layout(&root, &engine, Rect::new(0.0, 0.0, 120.0, 20.0));
        let leaves: Vec<_> = tree.leaves().collect();
        assert!(leaves[0].0.size.width >= 80.0, "高压缩阻力项不得越过下限");
        assert!(leaves[1].0.size.width < leaves[0].0.size.width);
        assert!((leaves.iter().map(|(rect, _)| rect.size.width).sum::<f32>() - 120.0).abs() < 0.01);
    }

    #[test]
    fn measure_reports_wrapped_height() {
        // 文本叶子在窄约束下折多行 → measure 高度随之增高（量测 = 排版）
        let Some(engine) = engine_or_skip() else {
            return;
        };
        let text_leaf = LayoutNode::Leaf(TextLeaf {
            content: "the quick brown fox jumps over the lazy dog".into(),
        });
        let narrow = measure(&text_leaf, &engine, Size::new(80.0, f32::INFINITY));
        let wide = measure(&text_leaf, &engine, Size::new(400.0, f32::INFINITY));
        assert!(narrow.height > wide.height, "窄约束应折多行，高更大");
    }

    /// 文本叶子：以 TextEngine::layout 量测高度（与渲染同源）。
    #[derive(Debug, Clone, PartialEq)]
    struct TextLeaf {
        content: String,
    }

    impl LayoutLeaf for TextLeaf {
        fn measure(&self, engine: &TextEngine, available: Size) -> Size {
            let style =
                kanesumi_core::TextStyle::new(15.0, 22.0, kanesumi_core::FontWeight::Normal);
            let lines = engine.layout(&self.content, style.size, available.width);
            let width = lines.iter().map(|l| l.width).fold(0.0, f32::max);
            Size::new(width, lines.len() as f32 * style.line_height)
        }
        fn render(&self, _theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
            scene.text(
                self.content.clone(),
                rect,
                Color::WHITE,
                kanesumi_core::TextStyle::new(15.0, 22.0, kanesumi_core::FontWeight::Normal),
                TextAlign::Left,
            );
        }
    }
}
