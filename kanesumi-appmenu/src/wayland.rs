// org_kde_kwin_appmenu —— KDE 私有 Wayland 协议的客户端绑定。
//
// 合成器（KWin / Ether compositor appmenu.rs）据此把 wl_surface 与 D-Bus 菜单地址
// 关联：焦点切换时从该地址拉取 com.canonical.dbusmenu 菜单树。参 KDE Global Menu 规范。
//
// 实现要点（复用 harness 已有的 Wayland 连接，无需 raw-window-handle）：
//   1. 在连接上开一条独立 event_queue（registry 扫描用；本协议无事件，之后不再派发）。
//   2. roundtrip 找 org_kde_kwin_appmenu_manager 全局 → create(surface) → set_address。
//   3. flush 一次把请求推到 wire；随后 appmenu/manager 对象长驻（leak，drop 走 release）。

use wayland_client::protocol::wl_registry::{Event as RegistryEvent, WlRegistry};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, QueueHandle};

/// 生成代码模块 —— 与 PezMax-One app_menu/linux/wayland.rs 同构（wayland-scanner 宏方式）。
pub mod appmenu_proto {
    use wayland_client;
    use wayland_client::protocol::*;

    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocol/appmenu.xml");
    }
    use self::__interfaces::*;

    wayland_scanner::generate_client_code!("protocol/appmenu.xml");
}

use appmenu_proto::org_kde_kwin_appmenu::OrgKdeKwinAppmenu;
use appmenu_proto::org_kde_kwin_appmenu_manager::OrgKdeKwinAppmenuManager;

/// 绑定 appmenu。成功返回后对象已长驻（进程退出前有效）。
/// 失败原因常见：非 KDE/Ether 合成器（`org_kde_kwin_appmenu_manager` 全局不存在）。
pub fn bind_appmenu(
    conn: &Connection,
    surface: &WlSurface,
    service_name: &str,
    object_path: &str,
) -> Result<(), String> {
    // 1. 独立 event_queue：扫描 registry 找 manager 全局。不干扰主队列（calloop 主循环）。
    let mut event_queue = conn.new_event_queue::<AppMenuState>();
    let qh = event_queue.handle();
    let _registry = conn.display().get_registry(&qh, ());
    let mut state = AppMenuState { manager: None };
    event_queue
        .roundtrip(&mut state)
        .map_err(|e| format!("registry roundtrip 失败: {e}"))?;

    let manager = state.manager.ok_or_else(|| {
        "org_kde_kwin_appmenu_manager 全局不存在（非 KDE / Ether 合成器会话？）".to_string()
    })?;

    // 2. create(surface) + set_address —— 把本应用 D-Bus 菜单地址关联到表面。
    let appmenu = manager.create(surface, &qh, ());
    appmenu.set_address(service_name.to_owned(), object_path.to_owned());

    // 3. flush 把请求推到 wire；此后每次主循环 dispatch 都会带出。
    conn.flush().map_err(|e| format!("wayland flush 失败: {e}"))?;

    log::info!("appmenu 已 set_address: service={} path={}", service_name, object_path);

    // 4. 长驻：leak 整套状态，保持 appmenu / manager / queue 对象生存。
    //    proxy drop 会走 release()（合成器随即清除关联），必须防止 Drop。
    let leaked = Box::new((event_queue, manager, appmenu, surface.clone()));
    let _static: &'static mut _ = Box::leak(leaked);
    Ok(())
}

// ── event dispatcher ───────────────────────────────────────────────────────

struct AppMenuState {
    manager: Option<OrgKdeKwinAppmenuManager>,
}

impl Dispatch<WlRegistry, ()> for AppMenuState {
    fn event(
        state: &mut Self,
        registry: &WlRegistry,
        event: RegistryEvent,
        _data: &(),
        _conn: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let RegistryEvent::Global { name, interface, version } = event
            && interface == "org_kde_kwin_appmenu_manager"
        {
            // version 上限 ≤ 1（协议声明 v1）。bind 失败返回 inert proxy。
            state.manager = Some(registry.bind::<OrgKdeKwinAppmenuManager, _, _>(
                name,
                version.min(1),
                qh,
                (),
            ));
        }
    }
}

// manager / appmenu 无事件，仅保证对象存活（空 Dispatch 实现）。
impl Dispatch<OrgKdeKwinAppmenuManager, ()> for AppMenuState {
    fn event(
        _state: &mut Self,
        _proxy: &OrgKdeKwinAppmenuManager,
        _event: <OrgKdeKwinAppmenuManager as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<OrgKdeKwinAppmenu, ()> for AppMenuState {
    fn event(
        _state: &mut Self,
        _proxy: &OrgKdeKwinAppmenu,
        _event: <OrgKdeKwinAppmenu as wayland_client::Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
    }
}
