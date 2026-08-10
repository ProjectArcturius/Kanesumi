use kanesumi_anim::{MetroAnim, MetroPresets};
use kanesumi_core::{MetroTheme, Rect, Scene};

/// 弹层状态机。参 CONTROL_SPEC §8/§9：
/// - 遮罩淡入 0.383s / 淡出 0.216s（OverlayOpening/ClosingAnimation）；
/// - 面板展开/收起 = `sheet_appear`(0.30s) / `sheet_dismiss`(0.26s)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupState {
    Closed,
    Opening,
    Open,
    Closing,
}

/// 弹层动画 —— 遮罩 + 面板两条并行动画轨道。
#[derive(Debug, Clone, PartialEq)]
pub struct PopupAnim {
    state: PopupState,
    overlay: MetroAnim,
    panel: MetroAnim,
}

impl Default for PopupAnim {
    fn default() -> Self {
        let mut overlay = MetroAnim::default_metro();
        overlay.jump_to(0.0);
        let mut panel = MetroAnim::default_metro();
        panel.jump_to(0.0);
        Self {
            state: PopupState::Closed,
            overlay,
            panel,
        }
    }
}

impl PopupAnim {
    pub fn new() -> Self {
        Self::default()
    }

    /// 打开：遮罩 0.383s 淡入 + 面板 0.30s 展开（可中断）。
    pub fn open(&mut self) {
        if matches!(self.state, PopupState::Opening | PopupState::Open) {
            return;
        }
        self.state = PopupState::Opening;
        self.overlay = MetroPresets::overlay_open();
        self.overlay.set_target(1.0);
        self.panel = MetroPresets::sheet_appear();
        self.panel.set_target(1.0);
    }

    /// 关闭：遮罩 0.216s 淡出 + 面板 0.26s 收起。
    pub fn close(&mut self) {
        if matches!(self.state, PopupState::Closing | PopupState::Closed) {
            return;
        }
        self.state = PopupState::Closing;
        self.overlay = MetroPresets::overlay_close();
        self.overlay.set_target(0.0);
        self.panel = MetroPresets::sheet_dismiss();
        self.panel.set_target(0.0);
    }

    /// 每帧推进；轨道稳态后转 Open/Closed。
    pub fn update(&mut self, dt: f64) {
        match self.state {
            PopupState::Opening | PopupState::Closing => {
                self.overlay.update(dt);
                self.panel.update(dt);
                if self.overlay.is_steady() && self.panel.is_steady() {
                    self.state = if self.state == PopupState::Opening {
                        PopupState::Open
                    } else {
                        PopupState::Closed
                    };
                }
            }
            _ => {}
        }
    }

    pub fn state(&self) -> PopupState {
        self.state
    }

    pub fn is_visible(&self) -> bool {
        matches!(
            self.state,
            PopupState::Opening | PopupState::Open | PopupState::Closing
        )
    }

    pub fn is_open(&self) -> bool {
        self.state == PopupState::Open
    }

    /// 遮罩当前不透明度 [0,1]。
    pub fn overlay_alpha(&self) -> f32 {
        self.overlay.value() as f32
    }

    /// 面板展开进度 [0,1]。
    pub fn panel_progress(&self) -> f64 {
        self.panel.value()
    }
}

/// 绘制弹层遮罩（全屏 `screen`）。
///
/// 遮罩色：UWP 经典 `#99FFFFFF`（白 60%）；Ether 深色空间桌面改用黑 45% 半透明
/// （参 CONTROL_SPEC §9 遮罩节 + LP_DIM 惯例），由 `theme.overlay_color` 提供。
pub fn render_overlay(theme: &MetroTheme, anim: &PopupAnim, screen: Rect, scene: &mut Scene) {
    if !anim.is_visible() {
        return;
    }
    let color = theme
        .overlay_color
        .with_alpha(theme.overlay_color.a * anim.overlay_alpha());
    scene.fill_rect(color, screen);
}

/// 弹层面板底座：surface 底 + 1px 边框。`progress` 用于展开位移（未启用布局位移，仅语义占位）。
pub fn render_panel_base(theme: &MetroTheme, rect: Rect, progress: f64, scene: &mut Scene) {
    let _ = progress;
    scene.fill_rounded_rect(theme.colors.surface, rect, theme.tokens.corner_radius);
    scene.stroke_rounded_rect(theme.colors.divider, rect, 1.0, theme.tokens.corner_radius);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_reaches_open() {
        let mut anim = PopupAnim::new();
        anim.open();
        assert_eq!(anim.state(), PopupState::Opening);
        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        assert_eq!(anim.state(), PopupState::Open);
        assert!((anim.overlay_alpha() - 1.0).abs() < 1e-3);
    }

    #[test]
    fn close_returns_closed() {
        let mut anim = PopupAnim::new();
        anim.open();
        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        anim.close();
        assert_eq!(anim.state(), PopupState::Closing);
        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        assert_eq!(anim.state(), PopupState::Closed);
        assert!((anim.overlay_alpha() - 0.0).abs() < 1e-3);
    }

    #[test]
    fn hidden_by_default() {
        let anim = PopupAnim::new();
        assert!(!anim.is_visible());
        assert!(!anim.is_open());
    }

    #[test]
    fn overlay_color_is_dark_scrim() {
        let color = MetroTheme::ether_dark().overlay_color;
        assert!(color.a > 0.0 && color.a < 1.0, "遮罩半透明");
        assert_eq!(color.r, 0.0, "深色遮罩");
    }
}
