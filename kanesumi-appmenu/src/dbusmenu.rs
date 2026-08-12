// com.canonical.dbusmenu 服务 —— 全局应用菜单的 D-Bus 侧。
//
// 挂在 /MenuBar，供合成器 TopBar（Ether global_menu.rs）/ Plasma Global Menu 读取。
// 实现 dbusmenu 规范中被客户端实际调用的方法 + 勾选状态刷新信号。
//
// 布局序列化关键点（易错区，参 PezMax-One proto.rs）：
// - GetLayout 返回 (u32, (i32, a{sv}, av))；av 的每个元素必须 `Value::Value(Box<_>)`
//   装箱 (ia{sv}av) 递归结构，否则 Plasma / Ether 会静默丢弃整个菜单。
// - a{sv} 字典的 value 同样必须装箱（signature 是 v）。
// - `_` 是助记符，字面下划线须转义为 `__`。
//
// zbus 5 阻塞连接在内部线程运行对象服务器，方法调用直接同步派发（zbus::interface）。

use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use zbus::interface;
use zbus::object_server::SignalEmitter;
use zbus::zvariant::{Array, Dict, OwnedValue, Signature, StructureBuilder, Value};

use crate::tree::{MenuItem, MenuTree, ToggleType};

/// 菜单服务内部状态。
pub struct DbMenuState {
    pub tree: MenuTree,
    /// 每次结构或属性更新递增；GetLayout 返回此值供客户端做失效比较。
    pub revision: u32,
    /// 点击命令送到主线程（App::on_menu_command）的通道。
    pub tx: std::sync::mpsc::Sender<i32>,
}

/// zbus interface 实现。挂在 Arc<Mutex> 上，允许服务线程在别处修改状态并发信号。
pub struct DbMenuIface {
    pub state: Arc<Mutex<DbMenuState>>,
}

// ── dbusmenu 接口 ──────────────────────────────────────────────────────────

#[interface(name = "com.canonical.dbusmenu")]
impl DbMenuIface {
    // ── 属性 ─────────────────────────────────────────────

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn status(&self) -> String {
        "normal".to_string()
    }

    #[zbus(property)]
    fn text_direction(&self) -> String {
        "ltr".to_string()
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    // ── 方法 ─────────────────────────────────────────────

    /// 返回子树布局。`parent_id=0` 时返回整棵；`recursion_depth<0` 无限层。
    /// 返回类型由 dbusmenu 规范强制（u(ia{sv}av)），不允许简化。
    #[allow(clippy::type_complexity)]
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        property_names: Vec<String>,
    ) -> (u32, (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>)) {
        let state = self.state.lock().unwrap();
        let node = state.tree.find(parent_id).unwrap_or(&state.tree.root);
        let layout = node_layout(node, recursion_depth, &property_names);

        // 顶层 struct 拆开——根节点的 props 直接返回具体类型，av 里的子项须装箱。
        let children_variants: Vec<OwnedValue> = layout
            .children
            .iter()
            .map(layout_to_value)
            .collect();

        (state.revision, (layout.id, layout.props, children_variants))
    }

    /// 批量返回节点属性。property_names 为空 = 返回全部。
    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        property_names: Vec<String>,
    ) -> Vec<(i32, HashMap<String, OwnedValue>)> {
        let state = self.state.lock().unwrap();
        ids.into_iter()
            .filter_map(|id| {
                state
                    .tree
                    .find(id)
                    .map(|node| (id, node_properties(node, &property_names)))
            })
            .collect()
    }

    /// 返回单个节点的单个属性。未知返回空字符串（避免客户端断连）。
    fn get_property(&self, id: i32, name: String) -> OwnedValue {
        let state = self.state.lock().unwrap();
        if let Some(node) = state.tree.find(id) {
            let props = node_properties(node, std::slice::from_ref(&name));
            if let Some(v) = props.get(&name) {
                return v.clone();
            }
        }
        str_val("")
    }

    /// 菜单事件——点击 / hover。仅处理 clicked，把 id 送主线程。
    fn event(&self, id: i32, event_id: String, _data: OwnedValue, _timestamp: u32) {
        if event_id != "clicked" {
            return;
        }
        let state = self.state.lock().unwrap();
        let _ = state.tx.send(id);
    }

    /// 批量事件。返回未找到的 id 列表（永不失败，返回空）。
    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        for (id, event_id, data, ts) in events {
            self.event(id, event_id, data, ts);
        }
        Vec::new()
    }

    /// 客户端展开子菜单前的钩子。菜单结构静态，返回 false（无需重拉）。
    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    // ── 信号 ─────────────────────────────────────────────
    // ItemsPropertiesUpdated / LayoutUpdated 由服务线程通过 SignalEmitter 触发。

    #[zbus(signal)]
    pub async fn items_properties_updated(
        emitter: &SignalEmitter<'_>,
        updated: Vec<(i32, HashMap<String, OwnedValue>)>,
        removed: Vec<(i32, Vec<String>)>,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    pub async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

// ── 布局序列化 ─────────────────────────────────────────────────────────────

/// 一层布局节点。序列化时经 `layout_to_value` 装成 D-Bus 递归 variant。
pub struct LayoutItem {
    pub id: i32,
    pub props: HashMap<String, OwnedValue>,
    pub children: Vec<LayoutItem>,
}

/// 节点属性 → dbusmenu a{sv} 字典。filter 非空时只含指定键。
/// 易错区：字典 value signature 是 v，所有值必须装箱 `Value::Value(Box<_>)`。
fn node_properties(node: &MenuItem, filter: &[String]) -> HashMap<String, OwnedValue> {
    let mut map: HashMap<String, OwnedValue> = HashMap::new();
    let want = |k: &str| filter.is_empty() || filter.iter().any(|f| f == k);

    if node.is_separator {
        if want("type") {
            map.insert("type".to_string(), str_val("separator"));
        }
    } else {
        if want("label") {
            // dbusmenu 用 `_` 作助记符；无助记符需求，把裸 `_` 转义为 `__`。
            let escaped = node.label.replace('_', "__");
            map.insert("label".to_string(), str_val(&escaped));
        }
        if !node.children.is_empty() && want("children-display") {
            map.insert("children-display".to_string(), str_val("submenu"));
        }
        match node.toggle_type {
            ToggleType::Checkmark => {
                if want("toggle-type") {
                    map.insert("toggle-type".to_string(), str_val("checkmark"));
                }
                if want("toggle-state") {
                    map.insert(
                        "toggle-state".to_string(),
                        i32_val(if node.toggle_state { 1 } else { 0 }),
                    );
                }
            }
            ToggleType::Radio => {
                if want("toggle-type") {
                    map.insert("toggle-type".to_string(), str_val("radio"));
                }
                if want("toggle-state") {
                    map.insert(
                        "toggle-state".to_string(),
                        i32_val(if node.toggle_state { 1 } else { 0 }),
                    );
                }
            }
            ToggleType::None => {}
        }
    }

    if want("enabled") && !node.enabled {
        map.insert("enabled".to_string(), bool_val(false));
    }
    if want("visible") && !node.visible {
        map.insert("visible".to_string(), bool_val(false));
    }

    map
}

/// 递归打包 GetLayout 返回类型 (i32, a{sv}, av)。
/// depth：-1 无限；0 仅当前节点；>0 剩余层数。
fn node_layout(node: &MenuItem, depth: i32, filter: &[String]) -> LayoutItem {
    let props = node_properties(node, filter);
    let children = if depth == 0 {
        Vec::new()
    } else {
        let next = if depth < 0 { -1 } else { depth - 1 };
        node.children
            .iter()
            .map(|c| node_layout(c, next, filter))
            .collect()
    };
    LayoutItem {
        id: node.id,
        props,
        children,
    }
}

/// 把 LayoutItem 递归编码成 zvariant Value。
/// av 的每个元素必须是 variant 装箱的 (ia{sv}av)；a{sv} 的 value 同样装箱。
pub fn layout_to_value(item: &LayoutItem) -> OwnedValue {
    let variant_sig = Signature::from_str("v").expect("v signature");
    let key_sig = Signature::from_str("s").expect("s signature");
    let val_sig = Signature::from_str("v").expect("v signature");

    let mut av = Array::new(&variant_sig);
    for child in &item.children {
        let child_value: Value<'static> = Value::from(layout_to_value(child));
        if let Err(e) = av.append(Value::Value(Box::new(child_value))) {
            log::error!("append child variant 失败: {e}");
        }
    }

    let mut props = Dict::new(&key_sig, &val_sig);
    for (k, v) in &item.props {
        let key_value: Value<'static> = Value::from(k.clone());
        let val_inner: Value<'static> = Value::from(v.clone());
        if let Err(e) = props.append(key_value, Value::Value(Box::new(val_inner))) {
            log::error!("append prop {k} 失败: {e}");
        }
    }

    let structure = StructureBuilder::new()
        .add_field(item.id)
        .append_field(Value::from(props))
        .append_field(Value::from(av))
        .build()
        .expect("layout struct 构建不会失败");

    Value::from(structure)
        .try_to_owned()
        .expect("layout Value 不会失败")
}

// ── 信号发射辅助（服务线程调用，block_on 驱动 async 信号）──────────────────

/// 发射 layout_updated(revision, parent)。菜单结构/勾选变更后调用。
pub fn emit_layout_updated(conn: &zbus::blocking::Connection, revision: u32, parent: i32) {
    let iface_ref = conn
        .object_server()
        .interface::<_, DbMenuIface>(crate::MENUBAR_OBJECT_PATH);
    let Ok(iface_ref) = iface_ref else {
        log::warn!("dbusmenu: 找不到 /MenuBar 接口，跳过 layout_updated 信号");
        return;
    };
    let _ = async_io::block_on(DbMenuIface::layout_updated(
        iface_ref.signal_emitter(),
        revision,
        parent,
    ));
}

/// 发射 items_properties_updated(updated, removed)。勾选状态变更后调用。
pub fn emit_items_properties_updated(
    conn: &zbus::blocking::Connection,
    updated: Vec<(i32, HashMap<String, OwnedValue>)>,
    removed: Vec<(i32, Vec<String>)>,
) {
    let iface_ref = conn
        .object_server()
        .interface::<_, DbMenuIface>(crate::MENUBAR_OBJECT_PATH);
    let Ok(iface_ref) = iface_ref else {
        log::warn!("dbusmenu: 找不到 /MenuBar 接口，跳过 items_properties_updated 信号");
        return;
    };
    let _ = async_io::block_on(DbMenuIface::items_properties_updated(
        iface_ref.signal_emitter(),
        updated,
        removed,
    ));
}

/// 发射单个节点 toggle-state 变化信号（SetCheck 用）。只带 toggle-state 键即可，
/// 客户端增量合并。参 dbusmenu spec ItemsPropertiesUpdated。
pub fn emit_toggle_updated(conn: &zbus::blocking::Connection, id: i32, checked: bool) {
    let mut props = HashMap::new();
    props.insert("toggle-state".to_string(), i32_val(if checked { 1 } else { 0 }));
    emit_items_properties_updated(conn, vec![(id, props)], Vec::new());
}

// ── zvariant 辅助 ─────────────────────────────────────────────────────────

fn str_val(s: &str) -> OwnedValue {
    Value::from(s.to_owned())
        .try_to_owned()
        .expect("str Value 不会失败")
}

fn bool_val(b: bool) -> OwnedValue {
    Value::from(b).try_to_owned().expect("bool Value 不会失败")
}

fn i32_val(v: i32) -> OwnedValue {
    Value::from(v).try_to_owned().expect("i32 Value 不会失败")
}
