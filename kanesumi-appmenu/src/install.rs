// 安装入口 —— 启动 D-Bus 菜单服务线程 + Wayland 绑定 + Registrar 兜底。
//
// 单一入口 install()：传入 Wayland 连接 / 表面 / 菜单树 / app_id，
// 返回 (AppMenuHandle, 命令接收端)。命令接收端由调用方持有，每帧 try_recv 排干，
// 路由到应用命令处理。eframe 应用（无自有 Wayland 连接）用
// `install_from_foreign_handles` 从 raw-window-handle 原始指针接入。
//
// 服务线程流程：
//   1. zbus 阻塞连接（内部线程跑对象服务器），/MenuBar 挂 com.canonical.dbusmenu；
//   2. request_name(app_id) 得稳定服务名（被占用则退回 unique_name）；
//   3. com.canonical.AppMenu.Registrar RegisterWindow(pid, /MenuBar) 兜底；
//   4. org_kde_kwin_appmenu set_address(service, /MenuBar)（合成器原生路径）；
//   5. 阻塞 update_rx：AppMenuHandle 推送运行时菜单更新（整树替换 / 勾选切换），
//      改状态 + revision 递增 + 发 dbusmenu 信号（勾选刷新）。
//
// 失败不致命：D-Bus 不可用 / 合成器不支持 appmenu 协议 → 记日志，其余路径照常。

use std::sync::{Arc, Mutex};
use std::thread;

use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy};
use zbus::blocking::Connection as DbConnection;

use crate::dbusmenu::{DbMenuIface, DbMenuState, emit_layout_updated, emit_toggle_updated};
use crate::tree::MenuTree;
use crate::{AppMenuHandle, MENUBAR_OBJECT_PATH, MenuUpdate, wayland};

/// 安装全局应用菜单。返回 (句柄, 命令接收端)。
///
/// - `conn` / `surface`：调用方的 Wayland 连接与主表面（set_address 关联用）。
/// - `tree`：初始菜单树。
/// - `app_id`：D-Bus 服务名候选（request_name(app_id)）。建议 `org.ether.*` 反向域名。
pub fn install(
    conn: &Connection,
    surface: &WlSurface,
    tree: MenuTree,
    app_id: &str,
) -> (AppMenuHandle, std::sync::mpsc::Receiver<i32>) {
    let (command_tx, command_rx) = std::sync::mpsc::channel::<i32>();
    let (update_tx, update_rx) = std::sync::mpsc::channel::<MenuUpdate>();

    let wl_conn = conn.clone();
    let wl_surface = surface.clone();
    let app_id = app_id.to_owned();

    thread::Builder::new()
        .name("kanesumi-appmenu".into())
        .spawn(move || run_service(wl_conn, wl_surface, tree, app_id, command_tx, update_rx))
        .expect("appmenu 服务线程启动失败");

    (AppMenuHandle { update_tx }, command_rx)
}

/// 从 eframe/winit 的原始 Wayland 句柄安装（eframe 应用集成，参 PezMax-One
/// `WaylandHandles`）。复用调用方（winit）已建好的 wl_display 连接 —— appmenu
/// 只对同连接的 wl_surface 生效。
///
/// # Safety
/// `display` / `surface` 必须为窗口生命周期内有效的 `wl_display` / `wl_surface`
/// 指针，且属于同一 Wayland 连接。调用方从 `eframe::CreationContext`
/// `display_handle()` / `window_handle()` 提取（raw-window-handle）。
pub unsafe fn install_from_foreign_handles(
    display: *mut core::ffi::c_void,
    surface: *mut core::ffi::c_void,
    tree: MenuTree,
    app_id: &str,
) -> Option<(AppMenuHandle, std::sync::mpsc::Receiver<i32>)> {
    use wayland_backend::sys::client::{Backend, ObjectId};

    // SAFETY: 调用方保证指针在窗口生命周期内有效（见函数文档）。
    let backend = unsafe { Backend::from_foreign_display(display as *mut _) };
    let conn = Connection::from_backend(backend);
    // SAFETY: 调用方保证 surface 是同一连接的 wl_proxy 指针。
    let surface_id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface as *mut _) }.ok()?;
    let surface = WlSurface::from_id(&conn, surface_id).ok()?;
    Some(install(&conn, &surface, tree, app_id))
}

/// 服务线程主体。阻塞运行直至 AppMenuHandle 被 drop（update_tx 关闭）。
fn run_service(
    wl_conn: Connection,
    wl_surface: WlSurface,
    tree: MenuTree,
    app_id: String,
    command_tx: std::sync::mpsc::Sender<i32>,
    update_rx: std::sync::mpsc::Receiver<MenuUpdate>,
) {
    // ── 1. zbus 阻塞连接 + /MenuBar 服务 ──────────────────────────────
    let state = Arc::new(Mutex::new(DbMenuState {
        tree,
        revision: 1,
        tx: command_tx,
    }));
    let iface = DbMenuIface { state: state.clone() };

    let conn = match zbus::blocking::connection::Builder::session()
        .map_err(|e| format!("D-Bus session 不可达: {e}"))
        .and_then(|b| {
            b.serve_at(MENUBAR_OBJECT_PATH, iface)
                .map_err(|e| format!("serve_at /MenuBar 失败: {e}"))
                .and_then(|b| b.build().map_err(|e| format!("build 失败: {e}")))
        }) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("appmenu: {e}（无全局菜单，进程照常运行）");
            return;
        }
    };

    // ── 2. 服务名：优先 request_name(app_id)，被占用退回 unique_name ─────
    let service_name = if conn.request_name(app_id.as_str()).is_ok() {
        app_id
    } else {
        let uniq = conn
            .unique_name()
            .map(|n| n.to_string())
            .unwrap_or_else(|| ":kanesumi.appmenu".into());
        log::debug!("appmenu: request_name({app_id}) 被占用，退回 {uniq}");
        uniq
    };

    // ── 3. Registrar 兜底（合成器按 client PID 匹配）────────────────────
    register_with_registrar(&conn);

    // ── 4. Wayland set_address（合成器原生路径，最高优先）───────────────
    match wayland::bind_appmenu(&wl_conn, &wl_surface, &service_name, MENUBAR_OBJECT_PATH) {
        Ok(_) => {}
        Err(e) => log::warn!("appmenu: Wayland 绑定跳过（{e}）"),
    }

    log::info!(
        "appmenu 已就绪: service={} path={}（Registrar + Wayland 双路径）",
        service_name,
        MENUBAR_OBJECT_PATH
    );

    // ── 5. 主循环：消费运行时菜单更新 ──────────────────────────────────
    while let Ok(update) = update_rx.recv() {
        match update {
            MenuUpdate::Replace(new_tree) => {
                let mut s = state.lock().unwrap();
                s.tree = new_tree;
                s.revision = s.revision.wrapping_add(1);
                let rev = s.revision;
                drop(s);
                emit_layout_updated(&conn, rev, 0);
                log::debug!("appmenu: 菜单树已替换（revision={rev}）");
            }
            MenuUpdate::SetCheck { id, checked } => {
                let mut s = state.lock().unwrap();
                let found = s
                    .tree
                    .find_mut(id)
                    .is_some_and(|node| {
                        node.toggle_state = checked;
                        true
                    });
                if !found {
                    drop(s);
                    continue;
                }
                s.revision = s.revision.wrapping_add(1);
                let rev = s.revision;
                drop(s);
                emit_toggle_updated(&conn, id, checked);
                emit_layout_updated(&conn, rev, 0);
                log::debug!("appmenu: 勾选 {id} -> {checked}（revision={rev}）");
            }
        }
    }
    // update_tx 关闭（句柄 drop）→ 退出线程。
}

/// com.canonical.AppMenu.Registrar RegisterWindow(pid, /MenuBar) 兜底。
/// window_id 传本进程 PID——Ether 合成器按 surface 所属 client 的 PID 匹配
/// （参 compositor/dbus_server.rs get_menu_by_pid / state/appmenu.rs 路径②）。
fn register_with_registrar(conn: &DbConnection) {
    let result = (|| -> zbus::Result<()> {
        use zbus::blocking::Proxy;
        use zbus::zvariant::ObjectPath;
        let proxy = Proxy::new(
            conn,
            "com.canonical.AppMenu.Registrar",
            "/com/canonical/AppMenu/Registrar",
            "com.canonical.AppMenu.Registrar",
        )?;
        let path = ObjectPath::try_from(MENUBAR_OBJECT_PATH)?;
        let pid = std::process::id();
        let _: () = proxy.call("RegisterWindow", &(pid, &path))?;
        Ok(())
    })();

    match result {
        Ok(()) => log::debug!("appmenu: Registrar RegisterWindow(pid) 已注册"),
        Err(e) => log::debug!("appmenu: Registrar 注册跳过（{e}）"),
    }
}
