use std::str::FromStr;

/// 应用角色 —— `ETHER_ROLE` 环境变量约定。参 Ether-main PLAN.md §4.3。
/// 系统应用由合成器以 `Command(find_bin(harness)) + ETHER_ROLE=<role>` 启动。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            _ => Err(RoleParseError),
        }
    }
}

/// 表面类型 —— Linux 外壳据此选择 Wayland 协议。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    fn parse_all_roles() {
        assert_eq!("desktop".parse(), Ok(EtherRole::Desktop));
        assert_eq!("browser".parse(), Ok(EtherRole::Browser));
        assert_eq!("topbar".parse(), Ok(EtherRole::TopBar));
        assert_eq!("dock".parse(), Ok(EtherRole::Dock));
        assert_eq!("launcher".parse(), Ok(EtherRole::Launcher));
        assert_eq!("candidate".parse(), Ok(EtherRole::Candidate));
        assert_eq!("lock".parse(), Ok(EtherRole::Lock));
        assert_eq!("shot-select".parse(), Ok(EtherRole::ShotSelect));
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
