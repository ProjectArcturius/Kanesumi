// 全局应用菜单（AppMenu）—— Kanesumi 应用壳的全局菜单接入。
//
// App 声明式构建菜单树（MenuTree，跨平台），一条调用自动完成：
//   1. com.canonical.dbusmenu D-Bus 服务（挂在 /MenuBar）；
//   2. org_kde_kwin_appmenu_manager Wayland 绑定（KWin / Ether 合成器原生路径）；
//   3. com.canonical.AppMenu.Registrar 兜底注册（合成器按 client PID 匹配）；
//   4. 点击事件经 mpsc 送回主线程（命令接收端）。
//
// 参考实现：PeZMax-One src/app_menu（成功对接 Plasma Global Menu + Ether TopBar）。
// 与 Ether 合成器侧 global_menu.rs / appmenu.rs / dbus_server.rs §Registrar 对应。
//
// 服务线程模型：zbus 5 阻塞连接在内部线程跑对象服务器（方法调用同步派发），
// 服务线程自身阻塞在 update_rx（AppMenuHandle 推送运行时菜单更新）上。
// Wayland 绑定复用调用方已有的连接 + 表面；eframe 应用经
// `install_from_foreign_handles` 用 raw-window-handle 原始指针接入。

pub mod tree;

#[cfg(target_os = "linux")]
mod dbusmenu;
#[cfg(target_os = "linux")]
mod install;
#[cfg(target_os = "linux")]
mod wayland;

pub use tree::{MenuItem, MenuTree, ToggleType};

/// D-Bus 上服务菜单的对象路径。合成器 / Plasma Global Menu 约定挂在 /MenuBar。
pub const MENUBAR_OBJECT_PATH: &str = "/MenuBar";

/// 运行时菜单更新 —— AppMenuHandle 推送，服务线程应用。
#[derive(Debug, Clone)]
pub enum MenuUpdate {
    /// 整体替换菜单树（结构/勾选变化均可，revision 递增 + layout_updated 信号）。
    Replace(MenuTree),
    /// 切换单个项勾选（items_properties_updated + layout_updated 信号）。
    SetCheck { id: i32, checked: bool },
}

/// 菜单句柄 —— App 持有，用于运行时更新菜单（勾选状态 / 结构）。
///
/// 纯 mpsc 发送端，跨平台可构造；Linux 下由 `install` / `install_from_foreign_handles`
/// 返回。`update_tx` 关闭（句柄全部 drop）时服务线程退出、菜单随之注销。
#[derive(Clone)]
pub struct AppMenuHandle {
    update_tx: std::sync::mpsc::Sender<MenuUpdate>,
}

impl AppMenuHandle {
    /// 整体替换菜单树。勾选状态变化（主题 / 视图模式等）用此接口。
    pub fn update_tree(&self, tree: MenuTree) {
        let _ = self.update_tx.send(MenuUpdate::Replace(tree));
    }

    /// 切换单个菜单项勾选（radio / checkmark 的 toggle-state）。
    pub fn set_check(&self, id: i32, checked: bool) {
        let _ = self.update_tx.send(MenuUpdate::SetCheck { id, checked });
    }
}

/// 安装全局应用菜单（Linux，harness 应用：传入已有 Wayland 连接与表面）。
///
/// - `conn` / `surface`：调用方（harness）的 Wayland 连接与主表面（set_address 关联用）。
/// - `tree`：初始菜单树。
/// - `app_id`：D-Bus 服务名候选（request_name(app_id)）。建议 `org.ether.*` 反向域名。
///
/// 返回 (句柄, 命令接收端)；命令接收端由调用方持有，每帧 try_recv 排干。
#[cfg(target_os = "linux")]
pub use install::install;

/// 安装全局应用菜单（Linux，eframe 应用：从 raw-window-handle 原始指针接入）。
///
/// # Safety
/// `display` / `surface` 必须为窗口生命周期内有效的 `wl_display` / `wl_surface`
/// 指针，且属于同一 Wayland 连接（调用方从 `CreationContext::display_handle` /
/// `window_handle` 提取，参 PezMax-One `extract_wayland_handles`）。
#[cfg(target_os = "linux")]
pub use install::install_from_foreign_handles;
