// 全局应用菜单树 —— 纯数据、跨平台。
//
// App 用声明式 API 构建菜单树（MenuTree），外壳（Linux）自动完成 D-Bus 服务、
// Wayland 绑定与 Registrar 注册；点击事件经 id 路由回 App::on_menu_command。
// 本模块不依赖 D-Bus / Wayland，任何平台可测试。
//
// 参 Ether 合成器 global_menu.rs / appmenu.rs、com.canonical.dbusmenu 规范
// （PeZMax-One app_menu 参考实现：proto.rs 序列化 + tree.rs 声明式）。

/// 菜单项开关类型（映射 dbusmenu `toggle-type` 属性）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToggleType {
    /// 普通项，无勾选。
    None,
    /// 复选（`toggle-type=checkmark`）。
    Checkmark,
    /// 单选（`toggle-type=radio`）。同一父节点内的多个 radio 天然互斥。
    Radio,
}

/// 菜单项 —— 声明式树节点。`id` 全局唯一且稳定（命令路由 / 勾选定位用）。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub id: i32,
    pub label: String,
    /// 分隔线：无 label，`type=separator`。子项为普通项时忽略。
    pub is_separator: bool,
    pub enabled: bool,
    pub visible: bool,
    pub toggle_type: ToggleType,
    /// 仅 `toggle_type != None` 时有意义。
    pub toggle_state: bool,
    pub children: Vec<MenuItem>,
}

impl MenuItem {
    /// 普通菜单项。
    pub fn item(id: i32, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            is_separator: false,
            enabled: true,
            visible: true,
            toggle_type: ToggleType::None,
            toggle_state: false,
            children: Vec::new(),
        }
    }

    /// 分隔线。
    pub fn separator(id: i32) -> Self {
        Self {
            id,
            label: String::new(),
            is_separator: true,
            enabled: true,
            visible: true,
            toggle_type: ToggleType::None,
            toggle_state: false,
            children: Vec::new(),
        }
    }

    /// 子菜单（含子项的顶层项 / 二级父项）。
    pub fn submenu(id: i32, label: impl Into<String>) -> Self {
        Self::item(id, label)
    }

    /// 复选项（checkmark）。
    pub fn check(id: i32, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            label: label.into(),
            is_separator: false,
            enabled: true,
            visible: true,
            toggle_type: ToggleType::Checkmark,
            toggle_state: checked,
            children: Vec::new(),
        }
    }

    /// 单选组项（radio）。
    pub fn radio(id: i32, label: impl Into<String>, checked: bool) -> Self {
        Self {
            id,
            label: label.into(),
            is_separator: false,
            enabled: true,
            visible: true,
            toggle_type: ToggleType::Radio,
            toggle_state: checked,
            children: Vec::new(),
        }
    }

    /// 追加子项（构建器风格：消费自身，返回 Self 支持链式）。
    pub fn push(mut self, child: MenuItem) -> Self {
        self.children.push(child);
        self
    }

    /// 深度优先查找（含自身）。
    pub fn find(&self, id: i32) -> Option<&MenuItem> {
        if self.id == id {
            return Some(self);
        }
        for c in &self.children {
            if let Some(found) = c.find(id) {
                return Some(found);
            }
        }
        None
    }

    /// 深度优先可变查找（含自身）。勾选状态更新用。
    pub fn find_mut(&mut self, id: i32) -> Option<&mut MenuItem> {
        if self.id == id {
            return Some(self);
        }
        for c in &mut self.children {
            if let Some(found) = c.find_mut(id) {
                return Some(found);
            }
        }
        None
    }

    /// 遍历所有节点（深度优先，含自身）。`dyn` 参数避免递归泛型实例化
    /// （`impl FnMut` 会随树深度嵌套类型，触发编译递归上限）。调用：`node.walk(&mut |n| …)`。
    pub fn walk(&self, f: &mut dyn FnMut(&MenuItem)) {
        f(self);
        for c in &self.children {
            c.walk(f);
        }
    }
}

/// 菜单树 —— 顶层即根节点（id 固定 0，dbusmenu 约定）。App 向 `push` 顶层子菜单。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuTree {
    /// 根节点。id 恒为 0，label 空。
    pub root: MenuItem,
}

impl MenuTree {
    /// 根节点 id（dbusmenu 顶层约定 0）。
    pub const ROOT_ID: i32 = 0;

    pub fn new() -> Self {
        Self {
            root: MenuItem::submenu(Self::ROOT_ID, ""),
        }
    }

    /// 追加顶层子菜单（File/Edit/View/Go…）。
    pub fn push(&mut self, child: MenuItem) -> &mut Self {
        self.root.children.push(child);
        self
    }

    /// 深度优先查找（含根）。
    pub fn find(&self, id: i32) -> Option<&MenuItem> {
        self.root.find(id)
    }

    /// 深度优先可变查找（含根）。勾选状态更新用。
    pub fn find_mut(&mut self, id: i32) -> Option<&mut MenuItem> {
        self.root.find_mut(id)
    }

    /// 遍历所有节点（深度优先，含根）。
    pub fn walk(&self, f: &mut dyn FnMut(&MenuItem)) {
        self.root.walk(f);
    }
}

impl Default for MenuTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MenuTree {
        let mut t = MenuTree::new();
        t.push(
            MenuItem::submenu(1, "File")
                .push(MenuItem::item(10, "New Window"))
                .push(MenuItem::separator(11))
                .push(MenuItem::item(12, "Close")),
        )
        .push(
            MenuItem::submenu(2, "View")
                .push(MenuItem::radio(20, "List", true))
                .push(MenuItem::radio(21, "Icon", false)),
        );
        t
    }

    #[test]
    fn tree_root_id_is_zero() {
        assert_eq!(MenuTree::ROOT_ID, 0);
        assert_eq!(MenuTree::new().root.id, 0);
    }

    #[test]
    fn find_looks_into_nested_children() {
        let t = sample();
        assert!(t.find(10).is_some(), "顶层子菜单的子项可找到");
        assert!(t.find(21).is_some(), "radio 子项可找到");
        assert!(t.find(0).is_some(), "根节点可找到");
        assert!(t.find(999).is_none());
    }

    #[test]
    fn find_mut_updates_toggle_state() {
        let mut t = sample();
        let n = t.find_mut(20).unwrap();
        n.toggle_state = false;
        let n = t.find_mut(21).unwrap();
        n.toggle_state = true;
        assert!(!t.find(20).unwrap().toggle_state);
        assert!(t.find(21).unwrap().toggle_state);
    }

    #[test]
    fn walk_visits_all_nodes_depth_first() {
        let t = sample();
        let mut ids = Vec::new();
        t.walk(&mut |n| ids.push(n.id));
        // 深度优先：根 → 1 → 10 → 11 → 12 → 2 → 20 → 21
        assert_eq!(ids, vec![0, 1, 10, 11, 12, 2, 20, 21]);
    }

    #[test]
    fn separator_has_no_label() {
        let s = MenuItem::separator(99);
        assert!(s.is_separator);
        assert!(s.label.is_empty());
    }

    #[test]
    fn radio_and_check_flags() {
        assert_eq!(MenuItem::radio(1, "a", true).toggle_type, ToggleType::Radio);
        assert_eq!(
            MenuItem::check(2, "b", false).toggle_type,
            ToggleType::Checkmark
        );
        assert_eq!(MenuItem::item(3, "c").toggle_type, ToggleType::None);
    }
}
