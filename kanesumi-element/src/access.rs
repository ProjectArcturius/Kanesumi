// access.rs —— 无障碍快照与契约检查（E6 地基）。参 docs/ELEMENT_TREE.md §Ⅸ、卡面 n1104-e3-controls。
//
// `accessibility()`（widget.rs）自 E2 起只定接口、无消费者 —— 本文件是第一个：
// `Tree::access_snapshot()`（tree.rs 末段的入口）把内容树导出成一份**纯数据**快照，
// 这是 WinUI UIA 对等物的数据层，也是后续 AccessKit / AT-SPI 桥的唯一输入。
// 不接 AT-SPI（需新依赖，未授权）。
//
// 覆盖层弹层（菜单族 / 对话框 / 提示）自 n1104 批已入树，故本快照把它们按**打开顺序**
// 挂在根节点的子节点之后（后开的在更后面 = 更靠上，与绘制次序一致）；这是「顶层元素」
// 的对等物，不是视觉父子关系 —— AT 侧的父子也与是否视觉包含无关。
//
// 契约（框架强制，测试挡住；对应 WinUI AutomationPeer 核心属性，只提取契约）：
//   1. 可聚焦必有非空角色与名字      —— role ← ControlType，name ← AutomationProperties.Name
//   2. 报告 bounds 与命中区一致      —— bounds ← BoundingRectangle，IsOffscreen 的反面（隐藏不出现）
//   3. 禁用态反映在 state            —— disabled ← !IsEnabledProperty
//   4. 隐藏元素不出现（走树时剪除，含其子树）
// 每条契约都有反向自检测试（故意造违例树，断言检查报错），防止检查形同虚设。
//
// 命中一致性（契约 2）按 `rect ⊕ paint_overflow` 判：级联子菜单、溢出面板这类
// **声明过**外扩的控件本就该在自己的声明区域里被命中，拿裸 rect 判会误报。

use std::collections::HashMap;
use std::fmt::Write as _;

use kanesumi_core::{Point, Rect};

use crate::id::WidgetId;
use crate::props::Insets;
use crate::tree::Tree;
use crate::widget::{AccessInfo, AccessItem, AccessRole};

/// 无障碍快照节点 —— 纯数据（不含控件引用，不回指树）。
///
/// `id` 是框架节点 id（`WidgetId::to_u64` 的注释即「无障碍节点 id」）：
/// AccessKit 接入时树 id 直接作为 AT 节点 id（ELEMENT_TREE §Ⅸ）。
/// `expanded` / `selected` 已入快照形状但控件侧暂无数据源（expander 现把展开态放在
/// `checked`、list 把选中行放在 `value`），待 AccessInfo 扩展后接入 —— 见报告。
#[derive(Debug, Clone, PartialEq)]
pub struct AccessNode {
    pub id: WidgetId,
    /// `None` = 控件未实现 `accessibility()`（契约 1 对可聚焦元素判它）。
    pub role: Option<AccessRole>,
    /// 原样保留（含空串）；契约 1 要求可聚焦元素 `trim()` 后非空。
    pub name: Option<String>,
    pub value: Option<String>,
    pub checked: Option<bool>,
    pub expanded: Option<bool>,
    pub selected: Option<bool>,
    /// 框架判定的有效禁用（自身或任一祖先被禁用，同 `ControlStates::disabled`）。
    pub disabled: bool,
    /// `Widget::focusable()` 原值（不含禁用 / 可见过滤 —— Tab 序由框架另行派生）。
    pub focusable: bool,
    /// arrange 后的矩形（与命中所用的同一 rect）。
    pub bounds: Rect,
    /// 控件声明的绘制外扩（`Widget::paint_overflow`）。级联子菜单这类内容在矩形之外，
    /// 合成项的可交互范围按 `bounds ⊕ overflow` 判。
    pub overflow: Insets,
    pub children: Vec<AccessNode>,
    /// 控件自己报告的**合成项**（菜单条目、列表行这类画在单节点内的内容）。
    /// 与 `children` 的区别：合成项不参与命中 / 焦点 / 布局，是 AT 的可枚举视图。
    pub items: Vec<AccessItem>,
}

impl AccessNode {
    /// 文本序列化（先序、每层缩进两格），供测试与报告比对。行格式：
    /// `[id] 角色 name="…" value="…" checked=… disabled focusable bounds=x,y 宽×高`；
    /// 角色缺失记 `-`，`Some` 但空的 name / value 原样输出（`name=""`），None 省略。
    /// 合成项以 `#序号` 代替 `[id]`，缩进比其宿主节点多一层。
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        self.write_to(&mut out, 0);
        out
    }

    fn write_to(&self, out: &mut String, depth: usize) {
        let _ = write!(out, "{}[{}] {}", "  ".repeat(depth), self.id.to_u64(), role_label(self.role));
        if let Some(name) = &self.name {
            let _ = write!(out, " name=\"{name}\"");
        }
        if let Some(value) = &self.value {
            let _ = write!(out, " value=\"{value}\"");
        }
        if let Some(v) = self.checked {
            let _ = write!(out, " checked={v}");
        }
        if let Some(v) = self.expanded {
            let _ = write!(out, " expanded={v}");
        }
        if let Some(v) = self.selected {
            let _ = write!(out, " selected={v}");
        }
        if self.disabled {
            out.push_str(" disabled");
        }
        if self.focusable {
            out.push_str(" focusable");
        }
        let b = self.bounds;
        let _ = write!(
            out,
            " bounds={},{} {}×{}",
            b.origin.x, b.origin.y, b.size.width, b.size.height
        );
        out.push('\n');
        for c in &self.children {
            c.write_to(out, depth + 1);
        }
        for (i, item) in self.items.iter().enumerate() {
            item.write_to(out, depth + 1, i);
        }
    }

    /// 按节点 id 查子树里的快照节点（测试与诊断用）。
    pub fn find(&self, id: WidgetId) -> Option<&AccessNode> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
    }

    /// 按名字查合成项（含子项；测试与诊断用）。
    pub fn find_item(&self, name: &str) -> Option<&AccessItem> {
        fn find_in<'a>(items: &'a [AccessItem], name: &str) -> Option<&'a AccessItem> {
            for it in items {
                if it.name == name {
                    return Some(it);
                }
                if let Some(found) = find_in(&it.children, name) {
                    return Some(found);
                }
            }
            None
        }
        find_in(&self.items, name).or_else(|| self.children.iter().find_map(|c| c.find_item(name)))
    }
}

fn role_label(role: Option<AccessRole>) -> &'static str {
    match role {
        Some(AccessRole::Button) => "Button",
        Some(AccessRole::CheckBox) => "CheckBox",
        Some(AccessRole::Switch) => "Switch",
        Some(AccessRole::TextInput) => "TextInput",
        Some(AccessRole::Label) => "Label",
        Some(AccessRole::List) => "List",
        Some(AccessRole::ListItem) => "ListItem",
        Some(AccessRole::TreeItem) => "TreeItem",
        Some(AccessRole::TabItem) => "TabItem",
        Some(AccessRole::Menu) => "Menu",
        Some(AccessRole::MenuItem) => "MenuItem",
        Some(AccessRole::Group) => "Group",
        Some(AccessRole::Other) => "Other",
        None => "-",
    }
}

// ── 合成项的序列化 ───────────────────────────────────────────────────────────

impl AccessItem {
    fn write_to(&self, out: &mut String, depth: usize, index: usize) {
        let _ = write!(
            out,
            "{}#{index} {} name=\"{}\"",
            "  ".repeat(depth),
            role_label(Some(self.role)),
            self.name
        );
        if let Some(v) = &self.value {
            let _ = write!(out, " value=\"{v}\"");
        }
        if let Some(c) = self.checked {
            let _ = write!(out, " checked={c}");
        }
        if let Some(v) = self.selected {
            let _ = write!(out, " selected={v}");
        }
        if let Some(v) = self.expanded {
            let _ = write!(out, " expanded={v}");
        }
        if self.focused {
            out.push_str(" focused");
        }
        if self.disabled {
            out.push_str(" disabled");
        }
        let b = self.bounds;
        let _ = write!(
            out,
            " bounds={},{} {}×{}",
            b.origin.x, b.origin.y, b.size.width, b.size.height
        );
        out.push('\n');
        for (i, c) in self.children.iter().enumerate() {
            c.write_to(out, depth + 1, i);
        }
    }
}

// ── 快照 ─────────────────────────────────────────────────────────────────────
/// 从 `root` 导出子树快照，并把覆盖层上打开的弹层按打开顺序追加为根节点的后续子节点。
/// 隐藏节点（含其子树）与控件已被取出的节点不出现；先序即树序，同一棵树两次快照逐字段相等。
pub(crate) fn snapshot_root(tree: &Tree, root: WidgetId) -> AccessNode {
    let mut snap = walk(tree, root);
    for popup in tree.popups() {
        if tree.is_visible(popup) && tree.access_widget(popup).is_some() {
            snap.children.push(walk(tree, popup));
        }
    }
    snap
}

fn walk(tree: &Tree, id: WidgetId) -> AccessNode {
    let widget = tree.access_widget(id);
    let info: Option<AccessInfo> = widget.and_then(|w| w.accessibility());
    let (role, name, value, checked) = match info {
        Some(i) => (Some(i.role), Some(i.name), i.value, i.checked),
        None => (None, None, None, None),
    };
    let bounds = tree.rect(id).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
    AccessNode {
        id,
        role,
        name,
        value,
        checked,
        // 数据源待 AccessInfo 扩展（见结构体注释）。
        expanded: None,
        selected: None,
        disabled: tree.states_of(id).disabled,
        focusable: widget.is_some_and(|w| w.focusable()),
        bounds,
        overflow: widget.map(|w| w.paint_overflow()).unwrap_or(Insets::ZERO),
        children: tree
            .children(id)
            .iter()
            .filter(|&&c| tree.is_visible(c) && tree.access_widget(c).is_some())
            .map(|&c| walk(tree, c))
            .collect(),
        // 合成项：控件把「画在自己矩形里的那些行」报成 AT 子元素（菜单条目这类）。
        items: widget.map(|w| w.access_items(bounds, tree.theme())).unwrap_or_default(),
    }
}

// ── 契约检查 ─────────────────────────────────────────────────────────────────

/// 单条契约违例（`detail` 为人读说明；测试按 `kind` + `id` 判定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessViolation {
    pub id: WidgetId,
    pub kind: AccessViolationKind,
    pub detail: String,
}

/// 契约违例种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessViolationKind {
    /// 可聚焦但 `accessibility()` 未提供（无角色）。
    FocusableNoRole,
    /// 可聚焦但名字缺失或空白。
    FocusableNoName,
    /// 自身命中测试接受了报告 bounds 外的点。
    HitOutsideBounds,
    /// 树命中的结果点不在其报告 bounds 内（快照与命中几何脱节）。
    HitResultOutsideBounds,
    /// 快照 disabled 与框架状态不一致（快照已过期）。
    DisabledMismatch,
    /// 合成项名字为空白。
    ItemNoName,
    /// 合成项 bounds 超出宿主可交互范围（bounds ⊕ paint_overflow）。
    ItemOutsideHost,
}

impl AccessViolationKind {
    /// 人读标签（断言消息用）。
    pub fn label(self) -> &'static str {
        match self {
            Self::FocusableNoRole => "可聚焦无角色",
            Self::FocusableNoName => "可聚焦无名字",
            Self::HitOutsideBounds => "命中越出 bounds",
            Self::HitResultOutsideBounds => "命中结果不在 bounds 内",
            Self::DisabledMismatch => "禁用态未反映",
            Self::ItemNoName => "合成项无名字",
            Self::ItemOutsideHost => "合成项越出宿主范围",
        }
    }
}

/// 全部契约：命名 + 命中一致 + 禁用态。返回全部违例（不短路），空 = 通过。
pub fn check_access_contract(tree: &Tree, root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = check_focusable_named(root);
    out.extend(check_bounds_hit(tree, root));
    out.extend(check_disabled_state(tree, root));
    out.extend(check_items(root));
    out
}

/// 契约一：每个可聚焦元素 role 非空且 name 非空。
pub fn check_focusable_named(root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = Vec::new();
    visit_named(root, &mut out);
    out
}

fn visit_named(n: &AccessNode, out: &mut Vec<AccessViolation>) {
    if n.focusable {
        if n.role.is_none() {
            out.push(AccessViolation {
                id: n.id,
                kind: AccessViolationKind::FocusableNoRole,
                detail: "可聚焦元素 accessibility() 未提供角色".into(),
            });
        }
        if n.name.as_deref().is_none_or(|s| s.trim().is_empty()) {
            out.push(AccessViolation {
                id: n.id,
                kind: AccessViolationKind::FocusableNoName,
                detail: "可聚焦元素名字缺失或空白（须取自控件自身文本 / 标签）".into(),
            });
        }
    }
    for c in &n.children {
        visit_named(c, out);
    }
}

/// 契约五：合成项必须有名字，且**落在宿主控件的可交互范围**内
/// （`bounds ⊕ paint_overflow`）—— 读屏点到的地方必须真能点到。
pub fn check_items(root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = Vec::new();
    visit_items(root, &mut out);
    out
}

fn visit_items(n: &AccessNode, out: &mut Vec<AccessViolation>) {
    if !n.items.is_empty() {
        // 合成项的范围 = 宿主 bounds ⊕ 控件的绘制外扩声明（菜单的级联子面板就在这里）。
        let area = n.overflow.inflate(n.bounds);
        for item in &n.items {
            visit_item(item, n.id, area, out);
        }
    }
    for c in &n.children {
        visit_items(c, out);
    }
}

fn visit_item(item: &AccessItem, host: WidgetId, area: Rect, out: &mut Vec<AccessViolation>) {
    if item.name.trim().is_empty() {
        out.push(AccessViolation {
            id: host,
            kind: AccessViolationKind::ItemNoName,
            detail: format!("合成项（{:?}）名字为空白", item.role),
        });
    }
    if !rect_within(item.bounds, area, 0.5) {
        out.push(AccessViolation {
            id: host,
            kind: AccessViolationKind::ItemOutsideHost,
            detail: format!(
                "合成项「{}」bounds {:?} 超出宿主可交互范围 {area:?}",
                item.name, item.bounds
            ),
        });
    }
    for c in &item.children {
        visit_item(c, host, area, out);
    }
}

fn rect_within(inner: Rect, outer: Rect, eps: f32) -> bool {
    inner.origin.x >= outer.origin.x - eps
        && inner.origin.y >= outer.origin.y - eps
        && inner.right() <= outer.right() + eps
        && inner.bottom() <= outer.bottom() + eps
}

/// 契约二：报告 bounds 与命中区一致。
///
/// 两层校验（两层都按 `bounds ⊕ paint_overflow` 判，见 [`hit_area`]）：
/// - 节点级：`Widget::hit_test` 不得接受声明区外的点（对照 `assert_no_hit_outside`
///   的 1px 外圈采样，直接调控件自己的 hit_test）；
/// - 树级：以 24px 网格采样表面，`Tree::hit` 的结果若在快照内，命中点必须落在其
///   报告范围内（命中与快照同帧几何）。不在快照内的命中结果（遮罩等框架自绘）跳过。
pub fn check_bounds_hit(tree: &Tree, root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = Vec::new();
    visit_hit(tree, root, &mut out);
    // 树级：网格采样。表面无效（未出帧）时无采样点，检查退化为节点级。
    let s = tree.surface();
    let mut index: HashMap<u64, &AccessNode> = HashMap::new();
    index_nodes(root, &mut index);
    let step = 24.0;
    let mut x = s.origin.x + step / 2.0;
    while x < s.right() {
        let mut y = s.origin.y + step / 2.0;
        while y < s.bottom() {
            if let Some(h) = tree.hit(Point::new(x, y))
                && let Some(node) = index.get(&h.to_u64())
                && !hit_area(tree, &h, node).contains(Point::new(x, y))
            {
                out.push(AccessViolation {
                    id: h,
                    kind: AccessViolationKind::HitResultOutsideBounds,
                    detail: format!("树命中点 ({x},{y}) 不在其报告 bounds {:?}", node.bounds),
                });
            }
            y += step;
        }
        x += step;
    }
    out
}

/// 控件允许被命中的报告范围 = 快照 bounds ⊕ 控件声明的 `paint_overflow`。
///
/// 级联子菜单、被折叠层级的溢出面板这类内容**本就在**自身矩形之外（`hit_test` 一并命中，
/// `paint_overflow` 也一并声明、`assert_paint_within` 同样放行）；拿裸 bounds 判命中一致性
/// 只会把正确实现判成违例。未声明外扩的控件退化为裸 bounds，判定不变。
fn hit_area(tree: &Tree, id: &WidgetId, node: &AccessNode) -> Rect {
    match tree.access_widget(*id) {
        Some(w) => w.paint_overflow().inflate(node.bounds),
        None => node.bounds,
    }
}

fn visit_hit(tree: &Tree, n: &AccessNode, out: &mut Vec<AccessViolation>) {
    if let Some(w) = tree.access_widget(n.id) {
        let b = n.bounds;
        // 外 1px 的探针只在**声明区之外**才算违例（声明区内的命中是正确的）。
        let area = hit_area(tree, &n.id, n);
        let (x0, y0, x1, y1) = (
            b.origin.x - 1.0,
            b.origin.y - 1.0,
            b.right() + 1.0,
            b.bottom() + 1.0,
        );
        let (cx, cy) = (b.center().x, b.center().y);
        for p in [
            Point::new(x0, cy),
            Point::new(x1, cy),
            Point::new(cx, y0),
            Point::new(cx, y1),
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x0, y1),
            Point::new(x1, y1),
        ] {
            if area.contains(p) {
                continue;
            }
            if w.hit_test(b, p) {
                out.push(AccessViolation {
                    id: n.id,
                    kind: AccessViolationKind::HitOutsideBounds,
                    detail: format!("hit_test 接受了 bounds 外的点 ({},{})", p.x, p.y),
                });
            }
        }
    }
    for c in &n.children {
        visit_hit(tree, c, out);
    }
}

fn index_nodes<'a>(n: &'a AccessNode, index: &mut HashMap<u64, &'a AccessNode>) {
    index.insert(n.id.to_u64(), n);
    for c in &n.children {
        index_nodes(c, index);
    }
}

/// 契约三：快照 disabled 与框架状态一致（快照过期时报错）。
pub fn check_disabled_state(tree: &Tree, root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = Vec::new();
    visit_disabled(tree, root, &mut out);
    out
}

fn visit_disabled(tree: &Tree, n: &AccessNode, out: &mut Vec<AccessViolation>) {
    if n.disabled != tree.states_of(n.id).disabled {
        out.push(AccessViolation {
            id: n.id,
            kind: AccessViolationKind::DisabledMismatch,
            detail: format!(
                "快照 disabled={} 与框架状态 disabled={} 不一致（快照已过期）",
                n.disabled,
                tree.states_of(n.id).disabled
            ),
        });
    }
    for c in &n.children {
        visit_disabled(tree, c, out);
    }
}

// ── 测试（含全部契约的反向自检）─────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use kanesumi_core::Size;

    use super::*;
    use crate::props::{Align, Insets, LayoutProps};
    use crate::testing::TestHarness;
    use crate::widgets::Stack;

    /// 探针控件：固定尺寸，可配置焦点 / 名字 / 命中行为（反向自检的违例源）。
    struct Probe {
        size: Size,
        focusable: bool,
        info: Option<AccessInfo>,
        /// Rect = 默认矩形命中；Everywhere = 任何点都命中（故意违例）；Half = 只认左半。
        hit_everywhere: bool,
        hit_half: bool,
    }

    impl Probe {
        fn named(role: AccessRole, name: &str, size: Size) -> Self {
            Self {
                size,
                focusable: true,
                info: Some(AccessInfo {
                    role,
                    name: name.to_string(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: false,
                hit_half: false,
            }
        }

        /// 非聚焦叶子（Label）：不可聚焦 → 契约一不绑定。
        fn leaf(name: &str, size: Size) -> Self {
            Self {
                size,
                focusable: false,
                info: Some(AccessInfo {
                    role: AccessRole::Label,
                    name: name.to_string(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: false,
                hit_half: false,
            }
        }

        /// 故意违例：可聚焦但 `accessibility()` 缺失。
        fn unnamed_focusable(size: Size) -> Self {
            Self {
                size,
                focusable: true,
                info: None,
                hit_everywhere: false,
                hit_half: false,
            }
        }
    }

    impl Widget for Probe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            self.size
        }
        fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
        fn focusable(&self) -> bool {
            self.focusable
        }
        fn hit_test(&self, rect: Rect, pos: Point) -> bool {
            self.hit_everywhere
                || (rect.contains(pos) && (!self.hit_half || pos.x < rect.center().x))
        }
        fn accessibility(&self) -> Option<AccessInfo> {
            self.info.clone()
        }
    }

    use crate::widget::{MeasureCtx, PaintCtx, Widget};
    use kanesumi_canvas::Scene;

    /// 根下挂一列两个探针：`确定`（Button，可聚焦）+ `说明`（Label）。
    fn harness() -> (TestHarness, crate::id::WidgetId, crate::id::WidgetId) {
        let mut h = TestHarness::new(200.0, 100.0);
        let col = h.tree.insert(h.root(), Stack::column());
        let a = h.tree.insert_with(
            col,
            Probe::named(AccessRole::Button, "确定", Size::new(88.0, 32.0)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let b = h.tree.insert_with(
            col,
            Probe::leaf("说明", Size::new(88.0, 32.0)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, a, b)
    }

    #[test]
    fn snapshot_reports_role_name_bounds_children() {
        let (h, a, b) = harness();
        let snap = h.tree.access_snapshot();
        assert_eq!(snap.id, h.tree.root(), "快照根 = 内容根");
        assert_eq!(snap.children.len(), 1, "根下是 Stack 列");
        let col = &snap.children[0];
        assert_eq!(col.children.len(), 2);
        let na = col.children.first().expect("探针 A");
        let nb = col.children.last().expect("探针 B");
        assert_eq!(na.id, a);
        assert_eq!(nb.id, b);
        assert_eq!(na.role, Some(AccessRole::Button));
        assert_eq!(na.name.as_deref(), Some("确定"));
        assert!(na.focusable && !na.disabled);
        assert_eq!(na.value, None);
        assert_eq!(na.checked, None);
        // 树序（先序）：A 在 B 前。
        assert_eq!(col.children.iter().map(|c| c.id).collect::<Vec<_>>(), vec![a, b]);
    }

    #[test]
    fn snapshot_is_stable() {
        let (h, _, _) = harness();
        assert_eq!(h.tree.access_snapshot(), h.tree.access_snapshot(), "两次快照逐字段相等");
    }

    #[test]
    fn snapshot_bounds_match_arranged_rect() {
        let (h, a, _) = harness();
        let snap = h.tree.access_snapshot();
        let na = snap.find(a).expect("探针 A 在快照内");
        assert_eq!(na.bounds, h.rect(a), "快照 bounds = arrange 后矩形");
    }

    #[test]
    fn hidden_subtree_absent_from_snapshot() {
        let (mut h, _, b) = harness();
        // 隐藏 B：快照不再出现；恢复可见后回来（契约四：隐藏元素不出现）。
        h.tree.update_props(b, |p| p.visible = false);
        h.frame();
        let snap = h.tree.access_snapshot();
        assert!(snap.find(b).is_none(), "隐藏节点不出现");
        // 子树一并消失。
        let col = h.tree.children(h.tree.root())[0];
        let child = h.tree.insert(col, Probe::named(AccessRole::Button, "孙", Size::new(10.0, 10.0)));
        h.tree.update_props(col, |p| p.visible = false);
        h.frame();
        assert!(h.tree.access_snapshot().find(child).is_none(), "隐藏子树的节点不出现");
        h.tree.update_props(col, |p| p.visible = true);
        h.frame();
        assert!(h.tree.access_snapshot().find(child).is_some(), "恢复可见后出现");
    }

    #[test]
    fn disabled_reflected_in_snapshot() {
        let (mut h, a, b) = harness();
        h.tree.set_enabled(a, false);
        h.frame();
        let snap = h.tree.access_snapshot();
        assert!(snap.find(a).unwrap().disabled, "禁用反映在快照");
        assert!(!snap.find(b).unwrap().disabled, "未禁用节点不受影响");
        // 祖先禁用 → 后代有效禁用（ControlStates::disabled 语义）。
        let col = h.tree.children(h.tree.root())[0];
        h.tree.set_enabled(col, false);
        h.frame();
        let snap = h.tree.access_snapshot();
        assert!(snap.find(a).unwrap().disabled, "祖先禁用传给后代");
        // 契约检查在新快照上通过。
        assert!(check_access_contract(&h.tree, &snap).is_empty());
    }

    #[test]
    fn to_text_is_stable_and_lossless() {
        let (h, _, _) = harness();
        let text = h.tree.access_snapshot().to_text();
        // 槽位：0 = 内容根，1 = 覆盖层根（不进快照），2 起为内容节点。
        let expected = "\
[0] - bounds=0,0 200×100
  [2] - bounds=0,0 200×100
    [3] Button name=\"确定\" focusable bounds=0,0 88×32
    [4] Label name=\"说明\" bounds=0,32 88×32
";
        assert_eq!(text, expected, "文本序列化稳定（供报告比对）");
        // 空名字原样输出（契约一判定需要区分 Some("") 与 None）。
        let mut h = TestHarness::new(100.0, 50.0);
        h.tree.insert(
            h.root(),
            Probe {
                size: Size::new(40.0, 20.0),
                focusable: true,
                info: Some(AccessInfo {
                    role: AccessRole::Button,
                    name: String::new(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: false,
                hit_half: false,
            },
        );
        h.frame();
        assert!(h.tree.access_snapshot().to_text().contains("name=\"\""), "空名原样保留");
    }

    #[test]
    fn contract_flags_unnamed_focusable() {
        let mut h = TestHarness::new(100.0, 50.0);
        let id = h.tree.insert(h.root(), Probe::unnamed_focusable(Size::new(40.0, 20.0)));
        h.frame();
        let snap = h.tree.access_snapshot();
        let vs = check_access_contract(&h.tree, &snap);
        // 反向自检：无角色 + 无名字两条违例都报，且指到该节点。
        let kinds: Vec<_> = vs.iter().filter(|v| v.id == id).map(|v| v.kind).collect();
        assert!(kinds.contains(&AccessViolationKind::FocusableNoRole), "违例含可聚焦无角色：{kinds:?}");
        assert!(kinds.contains(&AccessViolationKind::FocusableNoName), "违例含可聚焦无名字：{kinds:?}");
        assert_eq!(kinds.len(), 2, "不应有其它违例：{kinds:?}");
    }

    #[test]
    fn contract_flags_empty_name_focusable() {
        let mut h = TestHarness::new(100.0, 50.0);
        let id = h.tree.insert(
            h.root(),
            Probe {
                size: Size::new(40.0, 20.0),
                focusable: true,
                info: Some(AccessInfo {
                    role: AccessRole::Button,
                    name: "   ".into(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: false,
                hit_half: false,
            },
        );
        h.frame();
        let snap = h.tree.access_snapshot();
        let vs = check_access_contract(&h.tree, &snap);
        let kinds: Vec<_> = vs.iter().map(|v| v.kind).collect();
        assert!(
            kinds == vec![AccessViolationKind::FocusableNoName],
            "空白名字算缺失，且只报名字：{kinds:?}"
        );
        let _ = id;
    }

    #[test]
    fn contract_flags_hit_outside_bounds() {
        let mut h = TestHarness::new(100.0, 50.0);
        let id = h.tree.insert_with(
            h.root(),
            Probe {
                size: Size::new(40.0, 20.0),
                focusable: true,
                info: Some(AccessInfo {
                    role: AccessRole::Button,
                    name: "全命中".into(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: true,
                hit_half: false,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let snap = h.tree.access_snapshot();
        let vs = check_access_contract(&h.tree, &snap);
        let kinds: Vec<_> = vs.iter().filter(|v| v.id == id).map(|v| v.kind).collect();
        assert!(kinds.contains(&AccessViolationKind::HitOutsideBounds), "节点级：接受 bounds 外的点：{kinds:?}");
        assert!(kinds.contains(&AccessViolationKind::HitResultOutsideBounds), "树级：命中结果不在 bounds 内：{kinds:?}");
    }

    #[test]
    fn contract_passes_for_subrect_hit_widget() {
        // hit_test 收窄（只认左半）合规：命中区 ⊆ bounds。
        let mut h = TestHarness::new(100.0, 50.0);
        let id = h.tree.insert_with(
            h.root(),
            Probe {
                size: Size::new(40.0, 20.0),
                focusable: true,
                info: Some(AccessInfo {
                    role: AccessRole::Button,
                    name: "左半命中".into(),
                    value: None,
                    checked: None,
                }),
                hit_everywhere: false,
                hit_half: true,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let snap = h.tree.access_snapshot();
        assert!(check_access_contract(&h.tree, &snap).is_empty());
        // 左半命中、右半不命中（半开区间：中心与右缘点不算）。
        let b = snap.find(id).unwrap().bounds;
        let (cx, cy) = (b.center().x, b.center().y);
        assert_eq!(h.tree.hit(Point::new(b.origin.x + 10.0, cy)), Some(id), "左半命中");
        assert_ne!(
            h.tree.hit(Point::new(cx, cy)),
            Some(id),
            "中心点（右半边界）不命中收窄的 hit_test"
        );
        assert_ne!(
            h.tree.hit(Point::new(b.right() - 0.5, cy)),
            Some(id),
            "右缘内点不命中收窄的 hit_test"
        );
    }

    #[test]
    fn contract_flags_stale_disabled_snapshot() {
        let (mut h, a, _) = harness();
        let snap = h.tree.access_snapshot();
        h.tree.set_enabled(a, false);
        h.frame();
        let vs = check_disabled_state(&h.tree, &snap);
        assert!(
            vs.iter().any(|v| v.id == a && v.kind == AccessViolationKind::DisabledMismatch),
            "过期快照被禁用检查抓住"
        );
        // 同理：布局过期时命中检查抓住 bounds 脱节（节点移位后快照仍报旧位置）。
        let snap = h.tree.access_snapshot();
        h.tree.update_props(a, |p| p.margin = Insets::new(120.0, 0.0, 0.0, 0.0));
        h.frame();
        let vs = check_bounds_hit(&h.tree, &snap);
        assert!(
            vs.iter().any(|v| v.id == a && v.kind == AccessViolationKind::HitResultOutsideBounds),
            "过期快照的 bounds 被命中检查抓住"
        );
    }

    #[test]
    #[should_panic(expected = "无障碍契约违例")]
    fn harness_assert_panics_on_unnamed_focusable() {
        // 反向自检（夹具级）：故意造无名可聚焦元素，断言检查报错而非静默通过。
        let mut h = TestHarness::new(100.0, 50.0);
        h.tree.insert(h.root(), Probe::unnamed_focusable(Size::new(40.0, 20.0)));
        h.frame();
        h.assert_access_contract();
    }

    #[test]
    fn harness_assert_passes_on_wellformed_tree() {
        let (h, _, _) = harness();
        h.assert_access_contract();
    }

    // ── 合成项（画在单节点里的行，如菜单条目）──────────────────────────────────

    /// 报告合成项的探针：行高 24，`items` 按 rect 生成，可故意越界 / 无名。
    struct ItemProbe {
        rows: usize,
        /// 每行名字；空串 = 故意违例。
        names: Vec<String>,
        /// 越出宿主矩形多少（0 = 规整）。
        overflow_rows: f32,
    }

    impl Widget for ItemProbe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            Size::new(120.0, 24.0 * self.rows as f32)
        }
        fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
        fn accessibility(&self) -> Option<AccessInfo> {
            Some(AccessInfo {
                role: AccessRole::Menu,
                name: "菜单".into(),
                value: None,
                checked: None,
            })
        }
        fn access_items(&self, rect: Rect, _theme: &kanesumi_core::MetroTheme) -> Vec<AccessItem> {
            (0..self.rows)
                .map(|i| {
                    AccessItem::new(
                        AccessRole::MenuItem,
                        self.names.get(i).cloned().unwrap_or_default(),
                        Rect::new(
                            rect.origin.x,
                            rect.origin.y + i as f32 * 24.0 + self.overflow_rows,
                            rect.size.width,
                            24.0,
                        ),
                    )
                })
                .collect()
        }
    }

    fn item_harness(probe: ItemProbe) -> (TestHarness, crate::id::WidgetId) {
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            probe,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn synthesized_items_are_in_snapshot_and_serialized() {
        let (h, id) = item_harness(ItemProbe {
            rows: 2,
            names: vec!["复制".into(), "粘贴".into()],
            overflow_rows: 0.0,
        });
        let snap = h.tree.access_snapshot();
        let node = snap.find(id).expect("节点在快照内");
        assert_eq!(node.items.len(), 2, "两行命令各是一个合成项");
        assert_eq!(node.items[0].name, "复制");
        assert_eq!(node.items[0].role, AccessRole::MenuItem);
        assert!(
            node.items[0].bounds.origin.y >= node.bounds.origin.y - 0.5,
            "合成项 bounds 在宿主坐标系内"
        );
        let text = snap.to_text();
        assert!(text.contains("#0 MenuItem name=\"复制\""), "文本序列化带 #序号：\n{text}");
        assert!(text.contains("#1 MenuItem name=\"粘贴\""), "{text}");
        h.assert_access_contract();
    }

    #[test]
    fn item_outside_host_is_flagged() {
        let (h, id) = item_harness(ItemProbe {
            rows: 2,
            names: vec!["复制".into(), "粘贴".into()],
            // 第二行整体下移一行高：越出宿主 bounds（且该探针没有声明外扩）。
            overflow_rows: 24.0,
        });
        let snap = h.tree.access_snapshot();
        let kinds: Vec<_> = check_items(&snap)
            .into_iter()
            .filter(|v| v.id == id)
            .map(|v| v.kind)
            .collect();
        assert!(
            kinds.contains(&AccessViolationKind::ItemOutsideHost),
            "越出宿主的合成项要被抓住：{kinds:?}"
        );
    }

    #[test]
    fn item_without_name_is_flagged() {
        let (h, id) = item_harness(ItemProbe {
            rows: 1,
            names: vec![String::new()],
            overflow_rows: 0.0,
        });
        let snap = h.tree.access_snapshot();
        let kinds: Vec<_> = check_items(&snap)
            .into_iter()
            .filter(|v| v.id == id)
            .map(|v| v.kind)
            .collect();
        assert!(
            kinds.contains(&AccessViolationKind::ItemNoName),
            "空白名字的合成项要被抓住：{kinds:?}"
        );
    }

    #[test]
    fn declared_overflow_covers_synthesized_items() {
        // 与菜单同构：级联子面板画在宿主矩形之外，但控件声明了外扩 → 子项合法。
        struct OverflowMenu;
        impl Widget for OverflowMenu {
            fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
                Size::new(80.0, 40.0)
            }
            fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
            fn paint_overflow(&self) -> Insets {
                Insets::new(0.0, 0.0, 80.0, 0.0)
            }
            fn access_items(&self, rect: Rect, _theme: &kanesumi_core::MetroTheme) -> Vec<AccessItem> {
                vec![AccessItem::new(
                    AccessRole::MenuItem,
                    "级联项",
                    Rect::new(rect.right() + 4.0, rect.origin.y, 60.0, 20.0),
                )]
            }
        }
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            OverflowMenu,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let snap = h.tree.access_snapshot();
        let vs = check_items(&snap);
        assert!(
            vs.iter().all(|v| v.id != id),
            "声明过外扩时，外扩区里的合成项合法：{vs:?}"
        );
    }

    // ── 覆盖层弹层入快照（菜单族无障碍的地基）──────────────────────────────────

    /// 从根上摘下弹层节点（弹层的父是覆盖层，不是内容根）。
    fn popup_children(snap: &AccessNode, root_count: usize) -> &[AccessNode] {
        &snap.children[root_count..]
    }

    #[test]
    fn popup_nodes_join_snapshot_after_content_in_open_order() {
        let (mut h, a, _) = harness();
        let content_children = h.tree.access_snapshot().children.len();
        let first = h.tree.open_popup(
            Probe::named(AccessRole::Menu, "菜单", Size::new(120.0, 80.0)),
            crate::PopupSpec::default(),
        );
        h.frame();
        let second = h.tree.open_popup(
            Probe::named(AccessRole::Menu, "菜单", Size::new(120.0, 80.0)),
            crate::PopupSpec::default(),
        );
        h.frame();
        let snap = h.tree.access_snapshot();

        let pops = popup_children(&snap, content_children);
        assert_eq!(
            pops.iter().map(|n| n.id).collect::<Vec<_>>(),
            vec![first, second],
            "弹层按打开顺序追加在内容节点之后（后开的更靠上）"
        );
        let node = snap.find(first).expect("弹层在快照内");
        assert_eq!(node.role, Some(AccessRole::Menu));
        assert_eq!(node.name.as_deref(), Some("菜单"));
        // 内容树部分不受影响：探针 A 仍在原位置。
        assert!(snap.find(a).is_some());
    }

    #[test]
    fn closed_popup_leaves_snapshot() {
        let (mut h, _, _) = harness();
        let content_children = h.tree.access_snapshot().children.len();
        let popup = h.tree.open_popup(
            Probe::named(AccessRole::Menu, "菜单", Size::new(100.0, 60.0)),
            crate::PopupSpec::default(),
        );
        h.frame();
        assert!(h.tree.access_snapshot().find(popup).is_some(), "打开后在快照内");
        h.tree.close_popup(popup);
        h.frame();
        let snap = h.tree.access_snapshot();
        assert!(snap.find(popup).is_none(), "关闭后离开快照");
        assert!(popup_children(&snap, content_children).is_empty());
    }

    /// 命中一致性按 `bounds ⊕ paint_overflow` 判：声明区内的命中不是违例。
    struct OverflowProbe {
        overflow: Insets,
        /// 命中范围比声明区更大 → 仍然违例（防止「允许外扩」退化成「什么都能命中」）。
        hit_beyond: bool,
        info: AccessInfo,
    }

    impl Widget for OverflowProbe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            Size::new(40.0, 20.0)
        }
        fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}
        fn focusable(&self) -> bool {
            true
        }
        fn hit_test(&self, rect: Rect, pos: Point) -> bool {
            let area = self.overflow.inflate(rect);
            if self.hit_beyond {
                // 声明区外再越 20pt：必须被抓。
                self.overflow
                    .inflate(Insets::all(20.0).inflate(rect))
                    .contains(pos)
            } else {
                area.contains(pos)
            }
        }
        fn paint_overflow(&self) -> Insets {
            self.overflow
        }
        fn accessibility(&self) -> Option<AccessInfo> {
            Some(self.info.clone())
        }
    }

    fn overflow_harness(overflow: Insets, hit_beyond: bool) -> (TestHarness, crate::id::WidgetId) {
        let mut h = TestHarness::new(200.0, 120.0);
        let id = h.tree.insert_with(
            h.root(),
            OverflowProbe {
                overflow,
                hit_beyond,
                info: AccessInfo {
                    role: AccessRole::Menu,
                    name: "级联".into(),
                    value: None,
                    checked: None,
                },
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn declared_overflow_hit_is_not_a_contract_violation() {
        let (h, id) = overflow_harness(Insets::new(0.0, 0.0, 60.0, 0.0), false);
        let snap = h.tree.access_snapshot();
        let vs = check_access_contract(&h.tree, &snap);
        assert!(vs.is_empty(), "声明过外扩的命中不算越界：{vs:?}");
        // 溢出区确实可命中（否则这条契约就没被真正检验）。
        let b = snap.find(id).unwrap().bounds;
        assert_eq!(
            h.tree.hit(Point::new(b.right() + 30.0, b.center().y)),
            Some(id),
            "声明区内的点命中该控件"
        );
    }

    #[test]
    fn hit_beyond_declared_overflow_is_still_flagged() {
        let (h, id) = overflow_harness(Insets::new(0.0, 0.0, 60.0, 0.0), true);
        let snap = h.tree.access_snapshot();
        let kinds: Vec<_> = check_bounds_hit(&h.tree, &snap)
            .into_iter()
            .filter(|v| v.id == id)
            .map(|v| v.kind)
            .collect();
        assert!(
            kinds.contains(&AccessViolationKind::HitOutsideBounds)
                || kinds.contains(&AccessViolationKind::HitResultOutsideBounds),
            "超出声明区的命中仍要报违例：{kinds:?}"
        );
    }
}
