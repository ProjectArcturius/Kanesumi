// access.rs —— 无障碍快照与契约检查（E6 地基）。参 docs/ELEMENT_TREE.md §Ⅸ、卡面 n1104-e3-controls。
//
// `accessibility()`（widget.rs）自 E2 起只定接口、无消费者 —— 本文件是第一个：
// `Tree::access_snapshot()`（tree.rs 末段的入口）把内容树导出成一份**纯数据**快照，
// 这是 WinUI UIA 对等物的数据层，也是后续 AccessKit / AT-SPI 桥的唯一输入。
// 不接 AT-SPI（需新依赖，未授权）。弹层在覆盖层上，不在本快照内（菜单族入树后再并入）。
//
// 契约（框架强制，测试挡住；对应 WinUI AutomationPeer 核心属性，只提取契约）：
//   1. 可聚焦必有非空角色与名字      —— role ← ControlType，name ← AutomationProperties.Name
//   2. 报告 bounds 与命中区一致      —— bounds ← BoundingRectangle，IsOffscreen 的反面（隐藏不出现）
//   3. 禁用态反映在 state            —— disabled ← !IsEnabledProperty
//   4. 隐藏元素不出现（走树时剪除，含其子树）
// 每条契约都有反向自检测试（故意造违例树，断言检查报错），防止检查形同虚设。

use std::collections::HashMap;
use std::fmt::Write as _;

use kanesumi_core::{Point, Rect};

use crate::id::WidgetId;
use crate::tree::Tree;
use crate::widget::{AccessInfo, AccessRole};

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
    pub children: Vec<AccessNode>,
}

impl AccessNode {
    /// 文本序列化（先序、每层缩进两格），供测试与报告比对。行格式：
    /// `[id] 角色 name="…" value="…" checked=… disabled focusable bounds=x,y 宽×高`；
    /// 角色缺失记 `-`，`Some` 但空的 name / value 原样输出（`name=""`），None 省略。
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
    }

    /// 按节点 id 查子树里的快照节点（测试与诊断用）。
    pub fn find(&self, id: WidgetId) -> Option<&AccessNode> {
        if self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find(id))
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
        Some(AccessRole::Group) => "Group",
        Some(AccessRole::Other) => "Other",
        None => "-",
    }
}

// ── 快照 ─────────────────────────────────────────────────────────────────────

/// 从 `root` 导出子树快照。隐藏节点（含其子树）与控件已被取出的节点不出现；
/// 先序即树序，同一棵树两次快照逐字段相等。
pub(crate) fn snapshot_root(tree: &Tree, root: WidgetId) -> AccessNode {
    walk(tree, root)
}

fn walk(tree: &Tree, id: WidgetId) -> AccessNode {
    let widget = tree.access_widget(id);
    let info: Option<AccessInfo> = widget.and_then(|w| w.accessibility());
    let (role, name, value, checked) = match info {
        Some(i) => (Some(i.role), Some(i.name), i.value, i.checked),
        None => (None, None, None, None),
    };
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
        bounds: tree.rect(id).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0)),
        children: tree
            .children(id)
            .iter()
            .filter(|&&c| tree.is_visible(c) && tree.access_widget(c).is_some())
            .map(|&c| walk(tree, c))
            .collect(),
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
        }
    }
}

/// 全部契约：命名 + 命中一致 + 禁用态。返回全部违例（不短路），空 = 通过。
pub fn check_access_contract(tree: &Tree, root: &AccessNode) -> Vec<AccessViolation> {
    let mut out = check_focusable_named(root);
    out.extend(check_bounds_hit(tree, root));
    out.extend(check_disabled_state(tree, root));
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

/// 契约二：报告 bounds 与命中区一致。
///
/// 两层校验：
/// - 节点级：`Widget::hit_test` 不得接受 bounds 外的点（对照 `assert_no_hit_outside`
///   的 1px 外圈采样，直接调控件自己的 hit_test）；
/// - 树级：以 24px 网格采样表面，`Tree::hit` 的结果若在快照内，命中点必须落在其
///   报告 bounds 中（命中与快照同帧几何）。不在快照内的命中结果（覆盖层弹层）跳过。
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
                && !node.bounds.contains(Point::new(x, y))
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

fn visit_hit(tree: &Tree, n: &AccessNode, out: &mut Vec<AccessViolation>) {
    if let Some(w) = tree.access_widget(n.id) {
        let b = n.bounds;
        let (x0, y0, x1, y1) = (b.origin.x - 1.0, b.origin.y - 1.0, b.right() + 1.0, b.bottom() + 1.0);
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
}
