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
    }
}
