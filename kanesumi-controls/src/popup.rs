use kanesumi_anim::{EasingMode, MetroAnim, MetroPresets, UwpEasing};
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Rect, Size};

/// 弹层展开方向。参 CONTROL_SPEC §8（ComboBoxHelper 判据：弹出容器相对触发器 Top>0 即向下）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupDirection {
    Down,
    Up,
}

/// 弹层定位结果 —— 方向 + 面板矩形。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PopupPlacement {
    pub direction: PopupDirection,
    pub rect: Rect,
}

/// 弹层与触发器间的固定间隙（px）。
pub const POPUP_GAP: f32 = 4.0;

/// 弹层间隙访问器（供外部与 `place_popup` 对齐）。
pub const fn popup_gap() -> f32 {
    POPUP_GAP
}

/// 计算面板位置：优先向下（触发器下方空间足够）；不足则向上翻转。
///
/// 判据（ComboBoxHelper）：触发器下缘到屏幕底的空间 ≥ 面板高 + 间隙 → Down；否则 Up。
/// 水平方向：面板与触发器左缘对齐（右缘超出屏幕时收拢）。
#[must_use]
pub fn place_popup(trigger: Rect, panel_size: Size, screen: Rect, gap: f32) -> PopupPlacement {
    let below = screen.bottom() - trigger.bottom();
    let panel_h = panel_size.height;
    let direction = if below >= panel_h + gap {
        PopupDirection::Down
    } else {
        PopupDirection::Up
    };
    let x = match direction {
        PopupDirection::Down => trigger.origin.x,
        PopupDirection::Up => {
            let w = panel_size.width.min(screen.size.width);
            let left = trigger.origin.x;
            if left + w > screen.right() {
                (screen.right() - w).max(screen.origin.x)
            } else {
                left
            }
        }
    };
    let y = match direction {
        PopupDirection::Down => trigger.bottom() + gap,
        PopupDirection::Up => trigger.origin.y - panel_h - gap,
    };
    PopupPlacement {
        direction,
        rect: Rect::new(x, y, panel_size.width, panel_size.height),
    }
}

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
    /// 关闭态。`open()`/`close()` 会用 `MetroPresets::*` 立即覆盖 anim，
    /// 所以此处用零时长占位（V20：原实现调 `default_metro()` 再 `jump_to(0.0)`
    /// 两次构造纯浪费）。
    fn default() -> Self {
        let zero = MetroAnim::new(0.0, UwpEasing::Quadratic, EasingMode::EaseOut);
        Self {
            state: PopupState::Closed,
            overlay: zero.clone(),
            panel: zero,
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
    use kanesumi_core::Point;

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
        // 深色主题上遮罩必须足够强才能视觉可见（参 V9：0.45 在 #1E1E1E 上只暗 10%）
        assert!(
            color.a >= 0.6,
            "遮罩 alpha 应 ≥ 0.6 才在深色底上可见，实际 {}",
            color.a
        );
    }

    #[test]
    fn place_popup_down_when_space() {
        let trigger = Rect::new(100.0, 200.0, 200.0, 32.0);
        let size = Size::new(200.0, 160.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let p = place_popup(trigger, size, screen, 4.0);
        assert_eq!(p.direction, PopupDirection::Down);
        assert_eq!(p.rect.origin, Point::new(100.0, 236.0));
    }

    #[test]
    fn place_popup_up_when_no_space_below() {
        // 触发器贴近屏幕底，下方只有 20px < 160 + 4
        let trigger = Rect::new(100.0, 540.0, 200.0, 32.0);
        let size = Size::new(200.0, 160.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let p = place_popup(trigger, size, screen, 4.0);
        assert_eq!(p.direction, PopupDirection::Up);
        assert_eq!(p.rect.origin.y, 540.0 - 160.0 - 4.0, "面板上翻");
    }

    #[test]
    fn place_popup_up_clamps_right_edge() {
        let trigger = Rect::new(700.0, 540.0, 200.0, 32.0);
        let size = Size::new(200.0, 160.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let p = place_popup(trigger, size, screen, 4.0);
        assert_eq!(p.direction, PopupDirection::Up);
        assert!(
            p.rect.origin.x + p.rect.size.width <= screen.right() + 0.01,
            "面板右缘不超出屏幕"
        );
    }
}
