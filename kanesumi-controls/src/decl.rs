// 声明式 DSL —— Kanesumi 应用 UI 的声明式描述。
//
// 参 Ether-main PLAN.md §4（声明式控件树 + reconciler，参考 windows-reactor）。
// 状态驱动：`state → progress → render`，声明式描述是每帧从状态产出的**纯数据树**，
// reconciler 把它布局展开为 Scene 命令（无隐藏控件：元素不产生额外原生控件）。
//
// **布局唯一真源（2026-08-12 重构）**：本模块不再自算布局（旧 `collect_hits` 均分
// vs `layout_children` 内在尺寸两套算法 → 点击错位），而是把 `Decl` 树转换为
// `kanesumi_canvas::layout::LayoutNode`，交给 Measure/Arrange 引擎产出 `LaidTree`。
// 渲染命令、命中表、裁剪矩形全部从同一棵树派生（参 canvas/layout.rs）。
//
// 设计要点：
// - 纯数据、跨平台可测（不持 GPU/文本状态）；
// - 编译期类型检查（Rust 枚举/宏，非字符串 DSL）；
// - 与现有控件对接：元素最终调用 `MetroButton::render` 等（无重复实现）。

use kanesumi_canvas::layout::{CrossAlign, LayoutLeaf, LayoutNode, layout};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{MetroTheme, Rect, Size, TextStyle};

use crate::button::MetroButton;

/// 元素动作 —— 声明式 UI 与 App 逻辑的接线点。
/// 由 App 消费：命中元素 → 执行对应动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclAction {
    /// 无动作（纯展示）。
    None,
    /// 打开对话框。
    OpenDialog,
    /// 切换开关。
    ToggleSwitch,
    /// 选中列表项（携带索引）。
    SelectItem(usize),
    /// 自定义动作（App 侧匹配 id）。
    Custom(u32),
}

/// 声明式元素树（纯数据）。
#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    /// 水平布局容器。
    Row {
        /// 子元素间距。
        spacing: f32,
        children: Vec<Decl>,
    },
    /// 垂直布局容器。
    Column { spacing: f32, children: Vec<Decl> },
    /// 按钮。
    Button {
        label: String,
        accent: bool,
        action: DeclAction,
    },
    /// 文本。
    Text { content: String },
    /// 占位矩形（调试 / 布局测试）。
    Box { width: f32, height: f32 },
    /// 弹性占位 —— 在 Row/Column 中按 `grow` 比例分配剩余空间。参 V8。
    /// grow=0 时等价于零宽（无占位效果）；正数与其他 Spacer 按比例分。
    Spacer { grow: f32 },
}

impl Decl {
    /// 行容器。
    pub fn row(children: Vec<Decl>) -> Self {
        Decl::Row {
            spacing: 8.0,
            children,
        }
    }

    /// 列容器。
    pub fn column(children: Vec<Decl>) -> Self {
        Decl::Column {
            spacing: 8.0,
            children,
        }
    }

    /// 按钮。
    pub fn button(label: impl Into<String>, action: DeclAction) -> Self {
        Decl::Button {
            label: label.into(),
            accent: false,
            action,
        }
    }

    /// 强调按钮。
    pub fn accent_button(label: impl Into<String>, action: DeclAction) -> Self {
        Decl::Button {
            label: label.into(),
            accent: true,
            action,
        }
    }

    /// 文本。
    pub fn text(content: impl Into<String>) -> Self {
        Decl::Text {
            content: content.into(),
        }
    }

    /// 弹性占位（默认 grow=1）。参 V8。
    pub fn spacer(grow: f32) -> Self {
        Decl::Spacer { grow }
    }
}

/// 命中测试结果：元素动作 + 命中矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeclHit {
    pub action: DeclAction,
    pub rect: Rect,
}

/// `view!` 宏 —— 声明式 UI 语法糖，展开为 `Decl` 树。
///
/// 用法（children 为 `view!(...)` 嵌套调用）：
/// ```rust
/// use kanesumi_controls::{view, Decl, DeclAction};
/// let tree: Decl = view! {
///     Row {
///         view!(Button { label: "打开".to_string(), accent: true, action: DeclAction::OpenDialog }),
///         view!(Text { content: "就绪".to_string() })
///     }
/// };
/// ```
///
/// 支持：`Row { ... }`、`Column { ... }`、`Button { label, accent, action }`、
/// `Text { content }`、`Box { width, height }`。
#[macro_export]
macro_rules! view {
    // Row / Column 容器（children 为逗号分隔的 view! 表达式）
    (Row { $($child:expr),* $(,)? }) => {
        $crate::decl::Decl::Row {
            spacing: 8.0,
            children: vec![ $($child),* ],
        }
    };
    (Column { $($child:expr),* $(,)? }) => {
        $crate::decl::Decl::Column {
            spacing: 8.0,
            children: vec![ $($child),* ],
        }
    };
    // Button
    (Button { label: $label:expr, accent: $accent:expr, action: $action:expr }) => {
        $crate::decl::Decl::Button {
            label: $label,
            accent: $accent,
            action: $action,
        }
    };
    // Text
    (Text { content: $content:expr }) => {
        $crate::decl::Decl::Text { content: $content }
    };
    // Box
    (Box { width: $w:expr, height: $h:expr }) => {
        $crate::decl::Decl::Box { width: $w, height: $h }
    };
    // Spacer
    (Spacer { grow: $grow:expr }) => {
        $crate::decl::Decl::Spacer { grow: $grow }
    };
}

// ── 布局叶子（Decl 的 Leaf 种类 → LayoutLeaf） ──────────────────────────────

/// 声明式叶子的引擎适配：Button / Text / Box。
/// 样式在构建时从 theme 解析（`LayoutLeaf::measure` 无 theme 入参，故先固化）。
#[derive(Debug, Clone, PartialEq)]
enum DeclLeaf {
    Button {
        label: String,
        accent: bool,
        action: DeclAction,
        style: TextStyle,
    },
    Text {
        content: String,
        style: TextStyle,
    },
    Box {
        width: f32,
        height: f32,
    },
}

impl DeclLeaf {
    /// 命中动作（None = 纯展示不可命中）。
    fn action(&self) -> Option<DeclAction> {
        match self {
            DeclLeaf::Button {
                action, style: _, ..
            } if *action != DeclAction::None => Some(*action),
            _ => None,
        }
    }
}

impl LayoutLeaf for DeclLeaf {
    fn measure(&self, engine: &TextEngine, available: Size) -> Size {
        match self {
            DeclLeaf::Button { label, style, .. } => {
                MetroButton::new(label.clone()).measure(engine, *style)
            }
            DeclLeaf::Text { content, style } => {
                let lines = engine.layout(content, style.size, available.width);
                let width = lines
                    .iter()
                    .map(|l| l.width)
                    .fold(0.0, f32::max)
                    .min(available.width);
                Size::new(width, lines.len() as f32 * style.line_height)
            }
            DeclLeaf::Box { width, height } => Size::new(*width, *height),
        }
    }

    fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let style = match self {
            DeclLeaf::Button { style, .. } | DeclLeaf::Text { style, .. } => *style,
            DeclLeaf::Box { .. } => theme.typography.body,
        };
        match self {
            DeclLeaf::Button { label, accent, .. } => {
                let btn = if *accent {
                    MetroButton::accent(label.clone())
                } else {
                    MetroButton::new(label.clone())
                };
                btn.render(theme, engine, rect, scene);
            }
            DeclLeaf::Text { content, .. } => {
                // 与 `measure` 同源换行（canvas TextEngine::layout 唯一真源），
                // 逐行下发 → 量测与绘制永远一致（参 layout.rs「量测即排版」）。
                scene.text_with_options(
                    content.clone(),
                    rect,
                    theme.colors.on_surface,
                    style,
                    TextAlign::Left,
                    true,
                    None,
                    TextOverflow::Clip,
                );
            }
            DeclLeaf::Box { .. } => {
                scene.fill_rect(theme.colors.surface_variant, rect);
            }
        }
    }
}

// ── Decl → LayoutNode 转换 ──────────────────────────────────────────────

/// 把声明式树转换为引擎布局节点。`style` 从 theme 解析（Measure/Arrange 一致）。
fn to_layout_node(root: &Decl, style: TextStyle) -> LayoutNode<DeclLeaf> {
    match root {
        Decl::Row { spacing, children } => LayoutNode::Row {
            spacing: *spacing,
            cross: CrossAlign::Stretch,
            children: children.iter().map(|c| to_layout_node(c, style)).collect(),
        },
        Decl::Column { spacing, children } => LayoutNode::Column {
            spacing: *spacing,
            cross: CrossAlign::Stretch,
            children: children.iter().map(|c| to_layout_node(c, style)).collect(),
        },
        Decl::Button {
            label,
            accent,
            action,
        } => LayoutNode::Leaf(DeclLeaf::Button {
            label: label.clone(),
            accent: *accent,
            action: *action,
            style,
        }),
        Decl::Text { content } => LayoutNode::Leaf(DeclLeaf::Text {
            content: content.clone(),
            style,
        }),
        Decl::Box { width, height } => LayoutNode::Leaf(DeclLeaf::Box {
            width: *width,
            height: *height,
        }),
        Decl::Spacer { grow } => LayoutNode::Spacer { grow: *grow },
    }
}

/// reconciler：把声明式树渲染为 Scene 命令 + 命中表（无隐藏控件）。
///
/// 走 Measure/Arrange 引擎：`layout()` 一次产出 LaidTree，渲染与命中从同一棵树派生。
/// 容器以自身矩形裁剪子内容（box 语义）—— 文本溢出、点击错位从此不可能发生。
#[must_use]
pub fn render_decl(
    theme: &MetroTheme,
    engine: &TextEngine,
    root: &Decl,
    rect: Rect,
) -> (Scene, Vec<DeclHit>) {
    let style = theme.typography.body;
    let node = to_layout_node(root, style);
    let tree = layout(&node, engine, rect);
    let mut scene = Scene::default();
    tree.render(theme, engine, &mut scene);
    let hits = tree
        .leaves()
        .filter_map(|(r, leaf)| leaf.action().map(|action| DeclHit { action, rect: r }))
        .collect();
    (scene, hits)
}

/// 命中表收集（与 `render_decl` 同源：同一棵 LaidTree 的叶子矩形）。
///
/// 不再有独立的均分算法 —— `collect_hits` 即 `render_decl` 的命中面，
/// 画在哪里，点就得在哪里（参 decl.rs 头部设计注记）。
#[must_use]
pub fn collect_hits(
    theme: &MetroTheme,
    engine: &TextEngine,
    root: &Decl,
    rect: Rect,
) -> Vec<DeclHit> {
    render_decl(theme, engine, root, rect).1
}

// ── reconciler 增量 diff ────────────────────────────────────────────────
//
// `diff_decl` 比较两帧的声明式树（旧 vs 新），按**树位置路径**匹配元素，
// 输出变化列表。这是「保留视觉树 + damage 重绘」（PLAN §4.1 不变量 1/4）的逻辑基础：
// App 保留上一帧 `Decl`，diff 后只重发变化元素的命令，静态内容复用。

/// 元素路径：自根向下，每层一个容器子索引。`[0, 1]` = 根容器第 1 个子元素。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct DeclPath(pub Vec<usize>);

/// 元素种类（用于区分「同一位置的按钮变成文本」等跨类变化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclKind {
    Row,
    Column,
    Button,
    Text,
    Box,
    Spacer,
}

fn kind_of(node: &Decl) -> DeclKind {
    match node {
        Decl::Row { .. } => DeclKind::Row,
        Decl::Column { .. } => DeclKind::Column,
        Decl::Button { .. } => DeclKind::Button,
        Decl::Text { .. } => DeclKind::Text,
        Decl::Box { .. } => DeclKind::Box,
        Decl::Spacer { .. } => DeclKind::Spacer,
    }
}

/// 单个元素的变更。
#[derive(Debug, Clone, PartialEq)]
pub enum DeclChange {
    /// 元素新增（旧树无此路径）。
    Added(DeclPath),
    /// 元素移除（新树无此路径）。
    Removed(DeclPath),
    /// 元素内容变化（同路径同种类，但 label/content 等变了）。
    Changed(DeclPath),
    /// 元素种类变化（同路径，按钮→文本）。
    Replaced(DeclPath),
}

/// 比较两帧声明式树，返回按路径排序的变化列表。
///
/// 匹配规则：**按树位置路径**（同索引容器内，第 i 个子元素互相对齐）。
/// 简化模型（不做 keyed reconciliation）—— 列表增删会导致后续元素整体
/// 被标记 Changed/Replaced，但这在保留视觉树原型下可接受（后续可加 key）。
#[must_use]
pub fn diff_decl(old: &Decl, new: &Decl) -> Vec<DeclChange> {
    let mut changes = Vec::new();
    diff_node(old, new, DeclPath(vec![]), &mut changes);
    changes
}

fn diff_node(old: &Decl, new: &Decl, path: DeclPath, out: &mut Vec<DeclChange>) {
    // 种类不同 → Replaced（不再深入比较子元素）
    if kind_of(old) != kind_of(new) {
        out.push(DeclChange::Replaced(path.clone()));
        return;
    }
    // 内容不同 → Changed（仅对叶子内容类型；容器继续比较子元素）
    match (old, new) {
        (
            Decl::Button {
                label: o,
                accent: oa,
                action: oact,
            },
            Decl::Button {
                label: n,
                accent: na,
                action: nact,
            },
        ) if o != n || oa != na || oact != nact => {
            out.push(DeclChange::Changed(path.clone()));
            return;
        }
        (Decl::Text { content: o }, Decl::Text { content: n }) if o != n => {
            out.push(DeclChange::Changed(path.clone()));
            return;
        }
        (
            Decl::Box {
                width: ow,
                height: oh,
            },
            Decl::Box {
                width: nw,
                height: nh,
            },
        ) if ow != nw || oh != nh => {
            out.push(DeclChange::Changed(path.clone()));
            return;
        }
        (Decl::Spacer { grow: o }, Decl::Spacer { grow: n }) if o != n => {
            out.push(DeclChange::Changed(path.clone()));
            return;
        }
        _ => {}
    }
    // 容器：比较子元素数量与逐子内容
    let (o_children, n_children): (&[Decl], &[Decl]) = match (old, new) {
        (Decl::Row { children: o, .. }, Decl::Row { children: n, .. })
        | (Decl::Column { children: o, .. }, Decl::Column { children: n, .. }) => (o, n),
        // 叶子且内容相同 → 无变化
        _ => return,
    };
    let max = o_children.len().max(n_children.len());
    for i in 0..max {
        let mut child_path = path.clone();
        child_path.0.push(i);
        match (o_children.get(i), n_children.get(i)) {
            (None, Some(_)) => out.push(DeclChange::Added(child_path)),
            (Some(_), None) => out.push(DeclChange::Removed(child_path)),
            (Some(o), Some(n)) => diff_node(o, n, child_path, out),
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn row_places_children_by_intrinsic_width() {
        // 引擎布局：内在宽度 + spacing，不再均分（参 V8）。
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let root = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
        ]);
        let hits = collect_hits(&theme, &engine, &root, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].action, DeclAction::Custom(1));
        // 第二按钮 x = A 内在宽 + spacing 8
        assert!(
            (hits[1].rect.origin.x - (hits[0].rect.size.width + 8.0)).abs() < 0.5,
            "B.x = A.w + spacing 8，实际 {}",
            hits[1].rect.origin.x
        );
    }

    #[test]
    fn column_places_children_by_intrinsic_height() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let root = Decl::column(vec![
            Decl::text("a"),
            Decl::button("b", DeclAction::OpenDialog),
        ]);
        let hits = collect_hits(&theme, &engine, &root, Rect::new(0.0, 0.0, 200.0, 80.0));
        assert_eq!(hits.len(), 1, "只有按钮可命中");
        assert_eq!(hits[0].action, DeclAction::OpenDialog);
        // 按钮取内在高度（line_height + 11），非均分 80/2=40
        let intr = MetroButton::new("b")
            .measure(&engine, theme.typography.body)
            .height;
        assert!(
            (hits[0].rect.size.height - intr).abs() < 0.5,
            "按钮内在高度，实际 {}",
            hits[0].rect.size.height
        );
    }

    #[test]
    fn nested_layout_collects_leaves() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let root = Decl::row(vec![
            Decl::text("label"),
            Decl::column(vec![
                Decl::button("open", DeclAction::OpenDialog),
                Decl::button("save", DeclAction::Custom(7)),
            ]),
        ]);
        let hits = collect_hits(&theme, &engine, &root, Rect::new(0.0, 0.0, 400.0, 80.0));
        assert_eq!(hits.len(), 2);
        // 右半：两个按钮上下排列（spacing 8）
        assert!(
            (hits[1].rect.origin.y - (hits[0].rect.size.height + 8.0)).abs() < 0.5,
            "save.y = open.h + spacing 8，实际 {}",
            hits[1].rect.origin.y
        );
    }

    #[test]
    fn no_action_elements_not_hittable() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let root = Decl::row(vec![
            Decl::text("pure text"),
            Decl::Box {
                width: 10.0,
                height: 10.0,
            },
        ]);
        assert!(collect_hits(&theme, &engine, &root, Rect::new(0.0, 0.0, 200.0, 40.0)).is_empty());
    }

    #[test]
    fn view_macro_builds_decl_tree() {
        use crate::DeclAction;
        let tree: Decl = crate::view! {
            Row {
                view!(Button { label: "打开".to_string(), accent: true, action: DeclAction::OpenDialog }),
                view!(Text { content: "就绪".to_string() })
            }
        };
        let Decl::Row { children, .. } = &tree else {
            panic!("应为 Row");
        };
        assert_eq!(children.len(), 2);
        assert!(matches!(children[0], Decl::Button { accent: true, .. }));
        assert!(matches!(children[1], Decl::Text { .. }));
    }

    #[test]
    fn render_decl_produces_scene_and_hits() {
        use kanesumi_canvas::SceneCommand;
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let tree: Decl = crate::view! {
            Row {
                view!(Button { label: "打开".to_string(), accent: true, action: DeclAction::OpenDialog }),
                view!(Button { label: "保存".to_string(), accent: false, action: DeclAction::None })
            }
        };
        let (scene, hits) = render_decl(&theme, &engine, &tree, Rect::new(0.0, 0.0, 300.0, 40.0));
        // 两个按钮 → 至少 2 条 FillRect（底）
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert!(fills >= 2, "按钮底色");
        // 只有 OpenDialog 可命中（None 排除）
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].action, DeclAction::OpenDialog);
    }

    #[test]
    fn render_decl_emits_clip_for_container() {
        // 容器应发出 PushClip（box 语义）—— 文本溢出从此被裁剪。
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let tree = Decl::column(vec![Decl::text("超长文本超长文本超长文本超长文本")]);
        let (scene, _) = render_decl(&theme, &engine, &tree, Rect::new(0.0, 0.0, 80.0, 40.0));
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, kanesumi_canvas::SceneCommand::PushClip { .. })),
            "容器应发 PushClip"
        );
    }

    // ── diff_decl 测试 ──

    #[test]
    fn diff_identical_trees_is_empty() {
        let a = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::text("hi"),
        ]);
        let b = a.clone();
        assert!(diff_decl(&a, &b).is_empty());
    }

    #[test]
    fn diff_text_change_reports_changed() {
        let a = Decl::text("old");
        let b = Decl::text("new");
        assert_eq!(
            diff_decl(&a, &b),
            vec![DeclChange::Changed(DeclPath(vec![]))]
        );
    }

    #[test]
    fn diff_kind_change_reports_replaced() {
        let a = Decl::button("x", DeclAction::None);
        let b = Decl::text("x");
        assert_eq!(
            diff_decl(&a, &b),
            vec![DeclChange::Replaced(DeclPath(vec![]))]
        );
    }

    #[test]
    fn diff_added_and_removed_children() {
        let old = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
        ]);
        let new = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
            Decl::text("C"),
        ]);
        assert_eq!(
            diff_decl(&old, &new),
            vec![DeclChange::Added(DeclPath(vec![2]))]
        );
    }

    #[test]
    fn diff_nested_change_uses_path() {
        let old = Decl::row(vec![
            Decl::text("label"),
            Decl::column(vec![
                Decl::button("open", DeclAction::OpenDialog),
                Decl::button("save", DeclAction::Custom(7)),
            ]),
        ]);
        let new = Decl::row(vec![
            Decl::text("label"),
            Decl::column(vec![
                Decl::button("open", DeclAction::OpenDialog),
                Decl::button("save-as", DeclAction::Custom(8)),
            ]),
        ]);
        assert_eq!(
            diff_decl(&old, &new),
            vec![DeclChange::Changed(DeclPath(vec![1, 1]))],
            "路径 [1,1] = 列容器第 2 子元素"
        );
    }

    #[test]
    fn diff_animation_friendly_visual_change() {
        // 动画只动视觉属性（如按钮 accent 切换）→ Changed 而非 Replaced
        let old = Decl::button("save", DeclAction::Custom(1));
        let mut new = old.clone();
        if let Decl::Button { accent, .. } = &mut new {
            *accent = true;
        }
        assert_eq!(
            diff_decl(&old, &new),
            vec![DeclChange::Changed(DeclPath(vec![]))],
            "视觉属性变化 = Changed（供 retained 视觉树只更新此元素）"
        );
    }
}
