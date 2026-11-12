use std::str::FromStr;

/// 应用角色 —— `ETHER_ROLE` 环境变量约定。参 Ether-main PLAN.md §4.3。
/// 系统应用由合成器以 `Command(find_bin(harness)) + ETHER_ROLE=<role>` 启动。
///
/// `#[non_exhaustive]`：这 8 个角色是当前全部形态，但「通知表面」「系统对话框表面」
/// 会以**新增变体**的方式进来（只增不改）。下游按角色分派时必须留显式兜底分支
/// （明确降级 + 日志），不能把未知角色静默当成某个已知角色。参 Kanesumi
/// docs/API_STABILITY.md §三 / §五。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EtherRole {
    /// 桌面投影（Layer 1，无 SSD）。如 Librarian 桌面模式。
    Desktop,
    /// 普通窗口（Layer 2）。默认角色。
    Browser,
    /// 顶栏（layer-shell TOP）。如 Settings TopBar。
    TopBar,
    /// Dock（layer-shell BOTTOM）。
    Dock,
    /// 启动器（layer-shell OVERLAY）。
    Launcher,
    /// IME 候选窗 / 状态指示（layer-shell OVERLAY，跟随光标）。参 CEYBOARD_SPEC §Ⅱ。
    Candidate,
    /// 锁屏客户端（ext-session-lock-v1）。参 docs/LOCKSCREEN_DESIGN.md §Ⅱ/§Ⅵ L2。
    /// 不走 xdg/layer 主表面：经 `session_lock` 外壳为**每个输出**建锁屏表面。
    Lock,
    /// 截图选区覆盖层（layer-shell OVERLAY，四边锚定铺满，**独占键盘**）。
    /// 参 docs/SCREENSHOT_DESIGN.md §Ⅱ：冻结帧铺满 + 拖选 / 单击整窗，Esc 取消。
    ShotSelect,
    /// 通知表面（layer-shell OVERLAY，贴右上或指定角，**不抢键盘**）。
    ///
    /// 供应门 §Ⅰ 判据之一：通知这一格原先在框架里「一格都没有」。卡片本体在
    /// `kanesumi-controls::notification(_card)`，本角色只解决「它挂在哪一层」。
    /// 刻意**不新增** `SurfaceKind` 变体：表面类型是协议选择，通知用 OVERLAY 即正确；
    /// 新增变体只会让外壳多一条「未知类型显式降级」的路径，得不偿失。
    Notification,
}

/// layer-shell 键盘交互模式（与 `zwlr_layer_surface_v1.set_keyboard_interactivity` 同义）。
///
/// 为什么在本文件另立一个枚举而不直接用 sctk 的 `KeyboardInteractivity`：本文件是**跨平台**
/// 的角色解析层（Windows 下也编译、也跑单测），sctk 只在 Linux 外壳里可用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKeyboardInteractivity {
    /// 永不取键盘。
    None,
    /// 点击后取键盘（现状：所有既有 layer 主表面）。
    OnDemand,
    /// 表面映射即独占键盘（截图选区覆盖层：Esc 必须在图形出现后立刻可用）。
    Exclusive,
}

/// 角色解析失败。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoleParseError;

impl FromStr for EtherRole {
    type Err = RoleParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "desktop" => Ok(EtherRole::Desktop),
            "browser" => Ok(EtherRole::Browser),
            "topbar" => Ok(EtherRole::TopBar),
            "dock" => Ok(EtherRole::Dock),
            "launcher" => Ok(EtherRole::Launcher),
            "candidate" => Ok(EtherRole::Candidate),
            "lock" => Ok(EtherRole::Lock),
            "shot-select" => Ok(EtherRole::ShotSelect),
            "notification" => Ok(EtherRole::Notification),
            _ => Err(RoleParseError),
        }
    }
}

/// 表面类型 —— Linux 外壳据此选择 Wayland 协议。
///
/// `#[non_exhaustive]`：理由同 [`EtherRole`]。新增表面类型只加变体；下游
/// （外壳分派、弹层放置等）对未知类型必须显式降级，不许落回 xdg-shell。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SurfaceKind {
    /// xdg-shell 普通窗口。
    XdgShell,
    /// layer-shell BACKGROUND（最底层，桌面/墙纸）。非窗口：不受窗口管理，
    /// 无 SSD / 关闭键 / Alt+F4，不可被当作窗口关闭。参 Ether PLAN.md §4.3「desktop 迁移」。
    LayerBackground,
    /// layer-shell TOP（排他工作区上界）。
    LayerTop,
    /// layer-shell BOTTOM。
    LayerBottom,
    /// layer-shell OVERLAY。
    LayerOverlay,
    /// ext-session-lock-v1 锁屏表面（每输出一个）。参 docs/LOCKSCREEN_DESIGN.md §Ⅱ。
    SessionLock,
}

impl EtherRole {
    /// 从 `ETHER_ROLE` 环境变量读取角色；未设置或非法时回退为 Browser。
    pub fn from_env() -> Self {
        std::env::var("ETHER_ROLE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(EtherRole::Browser)
    }

    /// 角色 → 表面类型。参 PLAN.md §4.3 表。
    /// Desktop → layer-shell Background（外部布局：非窗口，不被窗口管理/关闭；
    /// 解决「桌面被 Alt+F4 关闭」。合成器需将 Background 层画在最底（Ether 跟进项）。
    pub fn surface_kind(self) -> SurfaceKind {
        match self {
            EtherRole::Desktop => SurfaceKind::LayerBackground,
            EtherRole::Browser => SurfaceKind::XdgShell,
            EtherRole::TopBar => SurfaceKind::LayerTop,
            EtherRole::Dock => SurfaceKind::LayerBottom,
            EtherRole::Launcher => SurfaceKind::LayerOverlay,
            EtherRole::Candidate => SurfaceKind::LayerOverlay,
            EtherRole::ShotSelect => SurfaceKind::LayerOverlay,
            EtherRole::Lock => SurfaceKind::SessionLock,
            // 通知落在 OVERLAY：在所有窗口之上、不需要独占层，且不参与窗口管理。
            EtherRole::Notification => SurfaceKind::LayerOverlay,
        }
    }

    /// 角色的 layer-shell 键盘交互模式。
    ///
    /// **只有截图选区覆盖层要独占键盘**：它一出现用户就在拖选，Esc / 右键必须立刻生效，
    /// 不能等一次点击才取焦（`OnDemand` 语义）。其余角色保持原值（桌面 `OnDemand` 是
    /// 2026-10-01 桌面审计补的：点中桌面后 Delete / F2 / 方向键要能落到桌面）。
    ///
    /// 屏幕几何：选区的键鼠路由不依赖本值（合成器按协议缓存状态判独占 Overlay），
    /// 本值只决定合成器是否把键盘交给它。参 docs/SCREENSHOT_DESIGN.md §Ⅱ。
    pub fn keyboard_interactivity(self) -> LayerKeyboardInteractivity {
        match self {
            EtherRole::ShotSelect => LayerKeyboardInteractivity::Exclusive,
            // 通知条**永不**取键盘：Win10 的 toast 不抢焦点，用户正在打的字不能被打断。
            // 键盘关闭走外壳的全局热键，不靠给表面焦点。
            EtherRole::Notification => LayerKeyboardInteractivity::None,
            EtherRole::Desktop
            | EtherRole::Browser
            | EtherRole::TopBar
            | EtherRole::Dock
            | EtherRole::Launcher
            | EtherRole::Candidate
            | EtherRole::Lock => LayerKeyboardInteractivity::OnDemand,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_role_is_overlay_and_never_steals_keyboard() {
        assert_eq!(EtherRole::Notification.surface_kind(), SurfaceKind::LayerOverlay);
        assert_eq!(
            EtherRole::Notification.keyboard_interactivity(),
            LayerKeyboardInteractivity::None,
            "通知永不取键盘（Win10 toast 不抢焦点）"
        );
        // 与既有 OVERLAY 角色对比：候选窗 / 启动器都是 OnDemand，只有通知是 None。
        assert_eq!(
            EtherRole::Candidate.keyboard_interactivity(),
            LayerKeyboardInteractivity::OnDemand
        );
        assert_eq!(
            EtherRole::Launcher.keyboard_interactivity(),
            LayerKeyboardInteractivity::OnDemand
        );
    }

    #[test]
    fn parse_all_roles() {
        assert_eq!("desktop".parse(), Ok(EtherRole::Desktop));
        assert_eq!("browser".parse(), Ok(EtherRole::Browser));
        assert_eq!("topbar".parse(), Ok(EtherRole::TopBar));
        assert_eq!("dock".parse(), Ok(EtherRole::Dock));
        assert_eq!("launcher".parse(), Ok(EtherRole::Launcher));
        assert_eq!("candidate".parse(), Ok(EtherRole::Candidate));
        assert_eq!("lock".parse(), Ok(EtherRole::Lock));
        assert_eq!("shot-select".parse(), Ok(EtherRole::ShotSelect));
        assert_eq!("notification".parse(), Ok(EtherRole::Notification));
    }

    #[test]
    fn parse_error_on_unknown() {
        assert_eq!("settings".parse::<EtherRole>(), Err(RoleParseError));
        assert_eq!("".parse::<EtherRole>(), Err(RoleParseError));
    }

    #[test]
    fn from_env_falls_back_to_browser() {
        unsafe {
            std::env::remove_var("ETHER_ROLE");
        }
        assert_eq!(EtherRole::from_env(), EtherRole::Browser);
        unsafe {
            std::env::set_var("ETHER_ROLE", "topbar");
        }
        assert_eq!(EtherRole::from_env(), EtherRole::TopBar);
        unsafe {
            std::env::set_var("ETHER_ROLE", "nonsense");
        }
        assert_eq!(EtherRole::from_env(), EtherRole::Browser);
    }

    #[test]
    fn surface_kind_mapping() {
        assert_eq!(EtherRole::TopBar.surface_kind(), SurfaceKind::LayerTop);
        assert_eq!(EtherRole::Dock.surface_kind(), SurfaceKind::LayerBottom);
        assert_eq!(
            EtherRole::Launcher.surface_kind(),
            SurfaceKind::LayerOverlay
        );
        assert_eq!(EtherRole::Browser.surface_kind(), SurfaceKind::XdgShell);
        // 候选窗 = layer-shell Overlay（跟随光标）。
        assert_eq!(
            EtherRole::Candidate.surface_kind(),
            SurfaceKind::LayerOverlay
        );
        // 外部布局：桌面 = layer-shell Background（非窗口）。
        assert_eq!(
            EtherRole::Desktop.surface_kind(),
            SurfaceKind::LayerBackground
        );
        // 锁屏：ext-session-lock-v1（非 xdg/layer）。
        assert_eq!(EtherRole::Lock.surface_kind(), SurfaceKind::SessionLock);
        // 截图选区：铺满的 Overlay 层表面。
        assert_eq!(
            EtherRole::ShotSelect.surface_kind(),
            SurfaceKind::LayerOverlay
        );
    }

    /// 未来角色回归：下游按角色分派时，**未知角色必须落进显式降级分支**，不许被任何
    /// 已知分支吞掉（例如静默当成 Browser）。
    ///
    /// 为什么用 `Probe::Future` 当「未来变体」：本班只钉 API 稳定性、不加新角色
    /// （通知 / 系统对话框留给后续件）；而 `#[non_exhaustive]` 在定义它的 crate 内不
    /// 生效，本测试也无法凭空构造第 9 个 `EtherRole` 变体。夹具照消费方写法逐条列
    /// **真实**角色，剩下的走 `_` 兜底 —— 这正是五个应用里那条分支的形状。
    #[test]
    fn future_role_is_not_swallowed_by_known_arms() {
        /// 分派输入：真实角色，或一个「本版本还不认识的未来角色」。
        #[derive(Debug, Clone, Copy)]
        enum Probe {
            Known(EtherRole),
            Future,
        }

        /// 分派结果：已知角色各占一格，降级单独一格 —— 降级绝不能等于任何已知角色。
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        enum Route {
            Desktop,
            Browser,
            TopBar,
            Dock,
            Launcher,
            Candidate,
            Lock,
            ShotSelect,
            Degraded,
        }

        fn route(probe: Probe) -> Route {
            match probe {
                Probe::Known(EtherRole::Desktop) => Route::Desktop,
                Probe::Known(EtherRole::Browser) => Route::Browser,
                Probe::Known(EtherRole::TopBar) => Route::TopBar,
                Probe::Known(EtherRole::Dock) => Route::Dock,
                Probe::Known(EtherRole::Launcher) => Route::Launcher,
                Probe::Known(EtherRole::Candidate) => Route::Candidate,
                Probe::Known(EtherRole::Lock) => Route::Lock,
                Probe::Known(EtherRole::ShotSelect) => Route::ShotSelect,
                // 兜底 = 明确降级：新角色只落这里，不冒充任何已知角色。
                _ => Route::Degraded,
            }
        }

        // 已知 8 个角色各自落自己的分支（兜底不许反过来吞掉已知角色）。
        let known = [
            (EtherRole::Desktop, Route::Desktop),
            (EtherRole::Browser, Route::Browser),
            (EtherRole::TopBar, Route::TopBar),
            (EtherRole::Dock, Route::Dock),
            (EtherRole::Launcher, Route::Launcher),
            (EtherRole::Candidate, Route::Candidate),
            (EtherRole::Lock, Route::Lock),
            (EtherRole::ShotSelect, Route::ShotSelect),
        ];
        assert_eq!(known.len(), 8, "角色数变了：新角色必须在分派处与上表定档");
        for (role, want) in known {
            assert_eq!(route(Probe::Known(role)), want, "{role:?} 落错了分支");
        }

        // 未来角色：只许降级，且不能命中任何已知分支。
        assert_eq!(route(Probe::Future), Route::Degraded);
        assert!(
            !matches!(
                route(Probe::Future),
                Route::Desktop
                    | Route::Browser
                    | Route::TopBar
                    | Route::Dock
                    | Route::Launcher
                    | Route::Candidate
                    | Route::Lock
                    | Route::ShotSelect
            ),
            "未来角色被某个已知分支吞掉了"
        );
    }

    /// 每个角色的键盘交互模式逐个钉死：新增角色若忘了定档，这条测试先红。
    /// 参 docs/SCREENSHOT_DESIGN.md §Ⅱ（只有选区覆盖层独占键盘）。
    #[test]
    fn keyboard_interactivity_per_role_is_locked() {
        use LayerKeyboardInteractivity::{Exclusive, OnDemand};
        let table = [
            (EtherRole::Desktop, OnDemand),
            (EtherRole::Browser, OnDemand),
            (EtherRole::TopBar, OnDemand),
            (EtherRole::Dock, OnDemand),
            (EtherRole::Launcher, OnDemand),
            (EtherRole::Candidate, OnDemand),
            (EtherRole::Lock, OnDemand),
            (EtherRole::ShotSelect, Exclusive),
        ];
        assert_eq!(table.len(), 8, "角色数变了：新角色必须在上表定档");
        for (role, want) in table {
            assert_eq!(role.keyboard_interactivity(), want, "{role:?} 的键盘交互模式");
        }
    }
}
