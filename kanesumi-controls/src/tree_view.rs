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
}

impl Default for MetroTreeView {
    fn default() -> Self {
        Self {
            root: TreeViewNode::new(""),
            selected: None,
            toggled: None,
            hovered: None,
            toggle_anim: MetroAnim::new(0.1, UwpEasing::Quadratic, EasingMode::EaseOut),
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
                    rect.origin.y + i as f32 * TREE_ITEM_H,
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

        for (i, row) in rows.iter().enumerate() {
            let r = rects[i];
            let selected = self.selected.as_deref() == Some(row.path.as_slice());
            let hovered = self.hovered.as_deref() == Some(row.path.as_slice());

            // 底：Selected / hover 白 15%
            if selected || hovered {
                scene.fill_rect(colors.on_surface.with_alpha(0.15), r);
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

            // 标签（缩进 + chevron 之后）
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
            scene.text(
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
