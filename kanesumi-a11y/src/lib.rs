// kanesumi-a11y —— 无障碍平台桥：元素树快照 → AccessKit 树。
//
// 分工（参 docs/ELEMENT_TREE.md §Ⅸ）：
//
//   kanesumi-element  Tree::access_snapshot()   ← 纯数据快照（E6 地基，已有多轮验收）
//          ↓  本 crate：快照 → accesskit::TreeUpdate（纯数据 → 纯数据）
//   accesskit_unix（AT-SPI）/ accesskit_windows（UIA）  ← 平台适配器，把树交给读屏
//
// 为什么把「映射」单独成 crate 而不是塞进 element：
// 1. 依赖方向。element 是控件平台的地基，不该为了无障碍拖进一个跨平台协议库；
//    反过来把映射放在最上层，任何外壳（harness / gallery / 未来的 kanesumi-shell）都按需接。
// 2. **可验性**。映射是纯函数（快照 → 树），在任何平台都能单测；平台适配器只能在目标
//    平台验。两者拆开后，Windows 上就能把「角色 / 名字 / 子节点 / 焦点 / 合成项」全部钉死，
//    Linux 上只剩「把这份树接上 D-Bus」这一小步。
//
// 本轮范围（**只读**）：读屏能枚举整棵树、读到名字 / 值 / 状态 / 矩形与当前焦点。
// 动作回灌（读屏按「点击」→ 树里发事件）尚未接：AccessKit 的 `Action` 一旦声明，适配器就会
// 把它转给可访问性客户端，而我们还没有把动作转成树事件的通路 —— 声明了却做不到，
// 比不声明更糟（读屏会显示一堆按了没反应的按钮）。动作回灌与 `Action::Focus` / `Click`
// 一起在下一步做，届时不改本 crate 的现有签名（只加函数与映射）。

use accesskit::{Node, NodeId, Role, Toggled, TreeInfo, TreeUpdate};
use kanesumi_core::Rect as KRect;
use kanesumi_element::{AccessNode, AccessRole, Tree as ElementTree, WidgetId};

/// Linux 读屏（AT-SPI）适配器。**默认关**：feature `at-spi` 同时用于「接线」与「编译
/// 验证」—— 在任意平台跑 `cargo check -p kanesumi-a11y --features at-spi` 都能把这份
/// 代码编一遍（`accesskit_unix` 本身是纯 Rust），于是 Linux 适配器不必等 Linux 机器才能
/// 确认类型与 API 没用错。运行时它只在 Linux 上真的连得上 AT-SPI。
#[cfg(feature = "at-spi")]
pub mod atspi;

/// 工具名（AccessKit 用来向读屏自我介绍）。
pub const TOOLKIT_NAME: &str = "Kanesumi";

/// 最近一帧的树更新 —— 无障碍适配器在**别的线程**上要树，而元素树只活在 UI 线程。
///
/// 这是两边唯一的交接点：UI 线程每帧 `publish`，适配器线程 `current` 取一份克隆。
/// 为什么不让适配器直接读元素树：`Tree` 不是 `Sync`（控件以 `&mut` 调用），跨线程读它
/// 要么拿锁把整棵树锁住（每帧与读屏抢锁），要么把快照改成 `Arc` 结构 —— 后者等于在本就
/// 纯数据的快照上再包一层。存一份**已经生成好的** `TreeUpdate` 最省事，也最不容易错。
#[derive(Debug, Clone, Default)]
pub struct SnapshotSlot(std::sync::Arc<std::sync::Mutex<Option<TreeUpdate>>>);

impl SnapshotSlot {
    pub fn new() -> Self {
        Self::default()
    }

    /// 换成本帧的树（覆盖旧的）。
    pub fn publish(&self, update: TreeUpdate) {
        *self.0.lock().expect("无障碍快照锁中毒") = Some(update);
    }

    /// 取最近一帧的树（无则 `None` —— 适配器自身会撑过「首帧还没来」的窗口）。
    pub fn current(&self) -> Option<TreeUpdate> {
        self.0.lock().expect("无障碍快照锁中毒").clone()
    }

    /// 是否已有可用的树。
    pub fn is_ready(&self) -> bool {
        self.0.lock().expect("无障碍快照锁中毒").is_some()
    }
}

/// 合成项（菜单条目 / 列表行）的节点 id 从这里**往下**分配。
///
/// 为什么不用 `WidgetId::to_u64()` 之外的另一段空间：那个编码是 `gen << 32 | index`，
/// 真实 id 远小于 `u64::MAX`。合成项不是框架节点、没有 `WidgetId`，从最大值往下取可以保证
/// 与真节点不撞号，同时保持「同一结构 → 同一 id」（确定性分配，AT 侧的对象身份才稳定）。
const SYNTHETIC_ID_BASE: u64 = u64::MAX;

/// 框架节点 id → AccessKit 节点 id（同一份 id 空间，事件与命中都靠它对上）。
pub fn node_id(id: WidgetId) -> NodeId {
    NodeId(id.to_u64())
}

/// 元素的坐标 → AccessKit 的矩形（同一种坐标系：都是逻辑像素、y 向下、绝对位置）。
fn rect(r: KRect) -> accesskit::Rect {
    accesskit::Rect {
        x0: r.origin.x as f64,
        y0: r.origin.y as f64,
        x1: (r.origin.x + r.size.width) as f64,
        y1: (r.origin.y + r.size.height) as f64,
    }
}

/// 控件角色 → AccessKit 角色。
///
/// `AccessRole` 是 `#[non_exhaustive]`（角色表还在长）：未知角色给 `GenericContainer`，
/// 而不是 `Unknown` —— `Unknown` 会让读屏报「未知控件」，而未知角色其实是「我们新加了一种
/// 控件、读屏这边暂时当作容器」；`GenericContainer` 至少能继续读出它的子元素。
fn role_of(role: Option<AccessRole>) -> Role {
    match role {
        Some(AccessRole::Button) => Role::Button,
        Some(AccessRole::CheckBox) => Role::CheckBox,
        Some(AccessRole::Switch) => Role::Switch,
        Some(AccessRole::TextInput) => Role::TextInput,
        Some(AccessRole::Label) => Role::Label,
        Some(AccessRole::List) => Role::List,
        Some(AccessRole::ListItem) => Role::ListItem,
        Some(AccessRole::TreeItem) => Role::TreeItem,
        Some(AccessRole::TabItem) => Role::Tab,
        Some(AccessRole::Menu) => Role::Menu,
        Some(AccessRole::MenuItem) => Role::MenuItem,
        Some(AccessRole::Group) => Role::Group,
        Some(AccessRole::Other) => Role::GenericContainer,
        None => Role::GenericContainer,
        _ => Role::GenericContainer,
    }
}

/// 把一棵元素树的无障碍快照转成 AccessKit 的一帧更新（读屏侧唯一输入）。
///
/// 焦点取 `tree.focused()`：AccessKit 要求每次更新都给出焦点节点（没有焦点时给根）。
pub fn update_for_tree(tree: &ElementTree) -> TreeUpdate {
    update(&tree.access_snapshot(), tree.focused())
}

/// 快照 → 一帧 AccessKit 更新（纯函数：同一快照必得同一份更新，便于测试与重放）。
///
/// 直接产出**全量**节点表：AccessKit 允许增量，但增量需要「哪块变了」的差分，而元素树手里
/// 最便宜且最准确的那份信息就是快照本身。等真机量出瓶颈再谈增量（届时本函数签名不变，
/// 只多一个 `&mut AccessKitSession` 之类的入参，属新增）。
pub fn update(snapshot: &AccessNode, focus: Option<WidgetId>) -> TreeUpdate {
    let mut builder = Builder {
        nodes: Vec::new(),
        next_synthetic: SYNTHETIC_ID_BASE,
    };
    let root = builder.push_tree(snapshot);
    let focus = focus
        .map(node_id)
        .filter(|id| builder.nodes.iter().any(|(n, _)| n == id))
        .unwrap_or(root);
    let mut tree = TreeInfo::new(root);
    tree.toolkit_name = Some(TOOLKIT_NAME.to_string());
    tree.toolkit_version = Some(env!("CARGO_PKG_VERSION").to_string());
    TreeUpdate {
        nodes: builder.nodes,
        tree: Some(tree),
        tree_id: accesskit::TreeId::ROOT,
        focus,
    }
}

/// 深优先展平：先建子节点（拿到 id），再建自己（children 要写 id）。
struct Builder {
    nodes: Vec<(NodeId, Node)>,
    next_synthetic: u64,
}

impl Builder {
    /// 递归展平一个快照节点，返回它的 AccessKit id。
    fn push_tree(&mut self, snapshot: &AccessNode) -> NodeId {
        let id = node_id(snapshot.id);
        let mut children = Vec::with_capacity(snapshot.children.len() + snapshot.items.len());
        for child in &snapshot.children {
            children.push(self.push_tree(child));
        }
        // 合成项（菜单条目 / 列表行）：控件自己画在同一个节点里的行。它们在 AT 侧必须是
        // **该节点的子元素**，否则读屏无法逐条枚举（参 docs/ELEMENT_TREE.md §Ⅸ-2）。
        for item in &snapshot.items {
            children.push(self.push_item(item));
        }

        let mut node = Node::new(role_of(snapshot.role));
        // 名字为空串时不写：AccessKit 会把空 label 当成「有名字且为空」，而读屏对
        // 「有名字但空」和「没名字」的处理不同（后者会去翻子元素猜名字）。
        if let Some(name) = &snapshot.name
            && !name.trim().is_empty()
        {
            node.set_label(name.clone());
        }
        if let Some(value) = &snapshot.value {
            node.set_value(value.clone());
        }
        if snapshot.disabled {
            node.set_disabled();
        }
        // selected / xpanded 在 AccessKit 里是**三态**（Option<bool>）：None = 该概念
        // 不适用，Some(false) = 明确未选中 / 已折叠。两者必须分开传，读屏才不会把
        // 「不适用」读成「未选中」。
        if let Some(selected) = snapshot.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = snapshot.expanded {
            node.set_expanded(expanded);
        }
        if let Some(checked) = snapshot.checked {
            node.set_toggled(Toggled::from(checked));
        }
        node.set_bounds(rect(snapshot.bounds));
        node.set_children(children);
        self.nodes.push((id, node));
        id
    }

    /// 合成项 → 子节点。id 由括号里递增分配（确定性：同一位置永远同一个 id）。
    fn push_item(&mut self, item: &kanesumi_element::AccessItem) -> NodeId {
        let id = NodeId(self.next_synthetic);
        self.next_synthetic -= 1;
        let mut children = Vec::with_capacity(item.children.len());
        for child in &item.children {
            children.push(self.push_item(child));
        }
        let mut node = Node::new(role_of(Some(item.role)));
        if !item.name.trim().is_empty() {
            node.set_label(item.name.clone());
        }
        if let Some(value) = &item.value {
            node.set_value(value.clone());
        }
        if item.disabled {
            node.set_disabled();
        }
        if let Some(selected) = item.selected {
            node.set_selected(selected);
        }
        if let Some(expanded) = item.expanded {
            node.set_expanded(expanded);
        }
        if let Some(checked) = item.checked {
            node.set_toggled(Toggled::from(checked));
        }
        node.set_bounds(rect(item.bounds));
        node.set_children(children);
        self.nodes.push((id, node));
        id
    }
}
