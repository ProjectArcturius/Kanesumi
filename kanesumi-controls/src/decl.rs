// 声明式 DSL —— Kanesumi 应用 UI 的声明式描述（原型）。
//
// 参 Ether-main PLAN.md §4（声明式控件树 + reconciler，参考 windows-reactor）。
// 状态驱动：`state → progress → render`，声明式描述是每帧从状态产出的**纯数据树**，
// reconciler 把它布局展开为 Scene 命令（无隐藏控件：元素不产生额外原生控件）。
//
// 设计要点：
// - 纯数据、跨平台可测（不持 GPU/文本状态）；
// - 编译期类型检查（Rust 枚举/宏，非字符串 DSL）；
// - 与现有控件对接：元素最终调用 `MetroButton::render` 等（无重复实现）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect};

use crate::button::MetroButton;
use crate::state::ControlState;

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

/// 把声明式树按给定矩形布局展开，并收集可命中元素（动作 + 矩形）。
///
/// 返回命中表：App 在输入事件时查表触发动作。布局为简单顺序排布
/// （Row = 横向均分，Column = 纵向堆叠），reconciler 展开为 Scene 由 App/控件渲染。
///
/// # 布局语义（原型）
/// - `Row`：children 在给定矩形内**水平均分**，每个子元素等宽。
/// - `Column`：children 在给定矩形内**纵向均分**，每个子元素等高。
/// - `Box`：固定尺寸（在 Row/Column 中占位）。
#[must_use]
pub fn collect_hits(root: &Decl, rect: Rect) -> Vec<DeclHit> {
    let mut hits = Vec::new();
    collect_hits_in(root, rect, &mut hits);
    hits
}

fn collect_hits_in(node: &Decl, rect: Rect, out: &mut Vec<DeclHit>) {
    match node {
        Decl::Row { spacing, children } | Decl::Column { spacing, children } => {
            let is_row = matches!(node, Decl::Row { .. });
            // 无 engine 环境：按等分布局（无内在尺寸测量）。Spacer 与 spacing 忽略。
            // 真实布局（Text/Button 内在宽度 + Spacer + spacing）走 `render_decl`
            // → `RetainedScene::hits()`；`collect_hits` 仅为无字体测试保留。
            let _ = spacing;
            let n = children.len().max(1) as f32;
            for (i, child) in children.iter().enumerate() {
                let child_rect = if is_row {
                    let w = rect.size.width / n;
                    Rect::new(rect.origin.x + i as f32 * w, rect.origin.y, w, rect.size.height)
                } else {
                    let h = rect.size.height / n;
                    Rect::new(rect.origin.x, rect.origin.y + i as f32 * h, rect.size.width, h)
                };
                collect_hits_in(child, child_rect, out);
            }
        }
        Decl::Button { action, .. } => {
            if *action != DeclAction::None {
                out.push(DeclHit {
                    action: *action,
                    rect,
                });
            }
        }
        Decl::Text { .. } | Decl::Box { .. } | Decl::Spacer { .. } => {}
    }
}

/// reconciler：把声明式树渲染为 Scene 命令（无隐藏控件）。
///
/// 布局与 `collect_hits` 一致（Row/Column 均分），元素用现有控件渲染：
/// `Button` → `MetroButton`，`Text` → 场景文本。返回 (Scene, hits) —— App 一次
/// 声明 → 渲染 + 命中表，状态驱动每帧重建。
#[must_use]
pub fn render_decl(
    theme: &MetroTheme,
    engine: &TextEngine,
    root: &Decl,
    rect: Rect,
) -> (Scene, Vec<DeclHit>) {
    let mut scene = Scene::default();
    let mut hits = Vec::new();
    render_in(theme, engine, root, rect, &mut scene, &mut hits);
    (scene, hits)
}

fn render_in(
    theme: &MetroTheme,
    engine: &TextEngine,
    node: &Decl,
    rect: Rect,
    scene: &mut Scene,
    hits: &mut Vec<DeclHit>,
) {
    match node {
        Decl::Row { spacing, children } | Decl::Column { spacing, children } => {
            let is_row = matches!(node, Decl::Row { .. });
            let child_rects = layout_children(theme, engine, children, *spacing, rect, is_row);
            for (child, r) in children.iter().zip(child_rects) {
                render_in(theme, engine, child, r, scene, hits);
            }
        }
        Decl::Button {
            label,
            accent,
            action,
        } => {
            let mut btn = if *accent {
                MetroButton::accent(label.clone())
            } else {
                MetroButton::new(label.clone())
            };
            btn.state = ControlState::Normal;
            btn.render(theme, engine, rect, scene);
            if *action != DeclAction::None {
                hits.push(DeclHit {
                    action: *action,
                    rect,
                });
            }
        }
        Decl::Text { content } => {
            let style = theme.typography.body;
            let text_rect = Rect::new(
                rect.origin.x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                rect.size.width,
                style.line_height,
            );
            scene.text(
                content.clone(),
                text_rect,
                theme.colors.on_surface,
                style,
                TextAlign::Left,
            );
        }
        Decl::Box { width, height } => {
            let _ = (width, height);
            scene.fill_rect(theme.colors.surface_variant, rect);
        }
        Decl::Spacer { .. } => {
            // 无绘制 —— 仅在 Row/Column 布局中吃剩余空间。
        }
    }
}

/// Row/Column 子元素矩形分配 —— 内在尺寸 + `spacing` + `Spacer.grow` 剩余分配。
///
/// 规则（对齐 CONTROL_SPEC「无 MinWidth，尺寸 = 内容 + Padding」）：
/// 1. 每个子沿主轴取内在尺寸（Text/Button 由 `TextEngine.measure` 决定，Box 由字段决定，
///    Spacer 与嵌套容器为 0）；
/// 2. 相邻子间加 `spacing`；
/// 3. 主轴剩余空间按 Spacer.grow 比例分给 Spacer；无 Spacer 则剩余留白（左/上对齐）；
/// 4. 交叉轴每个子占满 `rect` 交叉轴（简化：不做 cross-axis 对齐）。
///
/// 参 V8。
fn layout_children(
    theme: &MetroTheme,
    engine: &TextEngine,
    children: &[Decl],
    spacing: f32,
    rect: Rect,
    is_row: bool,
) -> Vec<Rect> {
    if children.is_empty() {
        return Vec::new();
    }
    let intrinsic: Vec<f32> = children
        .iter()
        .map(|c| main_axis_intrinsic(theme, engine, c, is_row))
        .collect();
    let sum_intrinsic: f32 = intrinsic.iter().sum();
    let gaps = spacing * (children.len() - 1) as f32;
    let main_len = if is_row { rect.size.width } else { rect.size.height };
    let remaining = (main_len - sum_intrinsic - gaps).max(0.0);
    let total_grow: f32 = children
        .iter()
        .map(|c| match c {
            Decl::Spacer { grow } => grow.max(0.0),
            _ => 0.0,
        })
        .sum();
    let mut out = Vec::with_capacity(children.len());
    let (mut cursor, cross_origin, cross_size) = if is_row {
        (rect.origin.x, rect.origin.y, rect.size.height)
    } else {
        (rect.origin.y, rect.origin.x, rect.size.width)
    };
    for (i, (child, &intr)) in children.iter().zip(&intrinsic).enumerate() {
        let extra = match child {
            Decl::Spacer { grow } if total_grow > 0.0 => remaining * (grow.max(0.0) / total_grow),
            _ => 0.0,
        };
        let size = intr + extra;
        let r = if is_row {
            Rect::new(cursor, cross_origin, size, cross_size)
        } else {
            Rect::new(cross_origin, cursor, cross_size, size)
        };
        out.push(r);
        cursor += size;
        if i + 1 < children.len() {
            cursor += spacing;
        }
    }
    out
}

/// 主轴内在尺寸（Text/Button 由字体度量、Box 由字段；Spacer/嵌套容器为 0）。
fn main_axis_intrinsic(theme: &MetroTheme, engine: &TextEngine, node: &Decl, is_row: bool) -> f32 {
    match node {
        Decl::Text { content } => {
            let style = theme.typography.body;
            if is_row {
                engine.measure(content, style.size)
            } else {
                style.line_height
            }
        }
        Decl::Button { label, accent, .. } => {
            let btn = if *accent {
                MetroButton::accent(label.clone())
            } else {
                MetroButton::new(label.clone())
            };
            let s = btn.measure(engine, theme.typography.body);
            if is_row { s.width } else { s.height }
        }
        Decl::Box { width, height } => {
            if is_row { *width } else { *height }
        }
        // 嵌套容器 / Spacer：主轴内在尺寸 0（Spacer 靠 grow 吃剩余；
        // 嵌套容器暂未支持精确内在测量，交给外层配 Spacer 或直接给足空间）。
        Decl::Row { .. } | Decl::Column { .. } | Decl::Spacer { .. } => 0.0,
    }
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

    #[test]
    fn row_allocates_equal_width() {
        let root = Decl::row(vec![
            Decl::button("A", DeclAction::Custom(1)),
            Decl::button("B", DeclAction::Custom(2)),
        ]);
        let hits = collect_hits(&root, Rect::new(0.0, 0.0, 200.0, 40.0));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].action, DeclAction::Custom(1));
        assert_eq!(hits[0].rect.size.width, 100.0);
        assert_eq!(hits[1].rect.origin.x, 100.0);
    }

    #[test]
    fn column_allocates_equal_height() {
        let root = Decl::column(vec![
            Decl::text("a"),
            Decl::button("b", DeclAction::OpenDialog),
        ]);
        let hits = collect_hits(&root, Rect::new(0.0, 0.0, 200.0, 80.0));
        assert_eq!(hits.len(), 1, "只有按钮可命中");
        assert_eq!(hits[0].action, DeclAction::OpenDialog);
        assert_eq!(hits[0].rect.size.height, 40.0);
    }

    #[test]
    fn nested_layout_collects_leaves() {
        let root = Decl::row(vec![
            Decl::text("label"),
            Decl::column(vec![
                Decl::button("open", DeclAction::OpenDialog),
                Decl::button("save", DeclAction::Custom(7)),
            ]),
        ]);
        let hits = collect_hits(&root, Rect::new(0.0, 0.0, 400.0, 80.0));
        assert_eq!(hits.len(), 2);
        // 右半：两个按钮上下均分
        assert_eq!(hits[0].rect.origin.x, 200.0);
        assert_eq!(hits[0].rect.size.height, 40.0);
        assert_eq!(hits[1].rect.origin.y, 40.0);
    }

    #[test]
    fn no_action_elements_not_hittable() {
        let root = Decl::row(vec![
            Decl::text("pure text"),
            Decl::Box {
                width: 10.0,
                height: 10.0,
            },
        ]);
        assert!(collect_hits(&root, Rect::new(0.0, 0.0, 200.0, 40.0)).is_empty());
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
