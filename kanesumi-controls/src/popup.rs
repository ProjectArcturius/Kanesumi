use kanesumi_anim::{FlyoutTrack, MetroPresets};
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Point, Rect, Size};
use kanesumi_element::placement::{self, PopupSide};

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
/// 判据全部委托 [`kanesumi_element::placement::place`]（框架内唯一的定位实现）：
/// 下方优先 → 上翻 → 左右滑移 → 夹紧到 `screen`（= 锚点所在输出的 workarea）。
/// 本函数只剩「控件参数 → 定位请求」的翻译；签名与 M5-3 口径（面板不得画到屏外）不变。
///
/// 用于 ComboBox/DropDownButton/MenuBar 顶级 flyout —— 触发器 vs 面板是「下方 or 上方」二择。
/// 子菜单（父项右侧展开）请用 [`place_submenu`]。
#[must_use]
pub fn place_popup(trigger: Rect, panel_size: Size, screen: Rect, gap: f32) -> PopupPlacement {
    let p = placement::place(placement::PlacementRequest {
        gap,
        ..placement::PlacementRequest::new(trigger, panel_size, screen)
    });
    PopupPlacement {
        direction: match p.side {
            // 左右 / 居中不在本函数的语义里（触发器对齐弹层只有上下两择），按下方报。
            PopupSide::Top => PopupDirection::Up,
            _ => PopupDirection::Down,
        },
        rect: p.rect,
    }
}

/// 把坐标吸附到**物理像素网格**（1/scale）。
///
/// 相邻背景块（菜单栏标题、下拉菜单项）共用吸附后的边缘，2× 缩放下就不会互相露出一条
/// 1 px 缝（真机已知问题 #18）。文字量测出来的是小数宽，不吸附必然出现半像素边。
pub fn snap_edge(v: f32) -> f32 {
    let s = kanesumi_canvas::surface_scale().max(1.0);
    (v * s).round() / s
}

/// 右键菜单定位：锚定指针点，四象限翻折，始终屏内。参 CONTEXT_MENU_SPEC §Ⅲ。
///
/// 锚点 = 指针坐标（无触发器）；面板左上角即指针位置。下方放不下翻上；越右缘时**滑移**
/// 回屏内（不再镜像翻左 —— 统一规则见 [`kanesumi_element::placement`]，与 Qt
/// `QMenuPrivate::popup`「右缘越界 → 收拢」一致）。面板大于屏幕时贴屏边（不裁内容）。
pub fn place_context_menu(anchor: Point, panel_size: Size, screen: Rect) -> Rect {
    placement::place(placement::PlacementRequest::new(
        Rect::new(anchor.x, anchor.y, 0.0, 0.0),
        panel_size,
        screen,
    ))
    .rect
}

/// 子菜单（父菜单某项右侧展开）横向定位：优先父项右侧；右缘越屏 → 翻到父项左侧；
/// 两侧都放不下 → 取空间更大的一侧并夹紧。纵向以父项顶对齐，下缘越屏时上移（夹进屏内）。
///
/// UWP MenuFlyoutSubItem 行为参考：子面板不覆盖父项、遇边翻转、始终屏内。
/// 参 CONTROL_SPEC §39（MenuFlyoutSubItem）。
#[must_use]
pub fn place_submenu(parent: Rect, panel_size: Size, screen: Rect, gap: f32) -> Rect {
    placement::place(placement::PlacementRequest {
        side: PopupSide::Right,
        gap,
        ..placement::PlacementRequest::new(parent, panel_size, screen)
    })
    .rect
}

/// 弹层状态机。参 CONTROL_SPEC §8/§9：
/// - 遮罩淡入 0.383s / 淡出 0.216s（OverlayOpening/ClosingAnimation）；
/// - 面板展开/收起 = `sheet_appear`(0.30s) / `sheet_dismiss`(0.26s，裁定 §247)，正典 Bézier（§200）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupState {
    Closed,
    Opening,
    Open,
    Closing,
}

/// 弹层动画 —— 遮罩 + 面板两条并行动画轨道。
///
/// 两条轨道持有**整个生命周期**（`FlyoutTrack`），open / close 只换向（`retune_to`
/// 加 `set_target`），从当前呈现值接续，不再每次重建轨道。旧实现每次重建
/// `MetroPresets::*` 新对象（初值 0），稳态后 close 直接从 0 起（瞬间消失）、
/// 收起中再 open 从 0 重跑（RM-1028-5 缺陷 1）。
#[derive(Debug, Clone, PartialEq)]
pub struct PopupAnim {
    state: PopupState,
    overlay: FlyoutTrack,
    panel: FlyoutTrack,
}

impl Default for PopupAnim {
    /// 关闭态：两条轨道用真实预设构造并 `jump_to(0)`，此后 open/close 只换向。
    fn default() -> Self {
        let mut overlay = MetroPresets::overlay_open();
        let mut panel = MetroPresets::sheet_appear();
        overlay.jump_to(0.0);
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

    /// 打开：遮罩 0.383s 淡入 + 面板 0.30s 正典开曲线展开（可中断，从当前值接续）。
    pub fn open(&mut self) {
        if matches!(self.state, PopupState::Opening | PopupState::Open) {
            return;
        }
        self.state = PopupState::Opening;
        self.overlay.retune_to(&MetroPresets::overlay_open());
        self.panel.retune_to(&MetroPresets::sheet_appear());
        self.overlay.set_target(1.0);
        self.panel.set_target(1.0);
    }

    /// 关闭：遮罩 0.216s 淡出 + 面板 0.26s（裁定 §247）正典关曲线收起（可中断，从当前值接续）。
    pub fn close(&mut self) {
        if matches!(self.state, PopupState::Closing | PopupState::Closed) {
            return;
        }
        self.state = PopupState::Closing;
        self.overlay.retune_to(&MetroPresets::overlay_close());
        self.panel.retune_to(&MetroPresets::sheet_dismiss());
        self.overlay.set_target(0.0);
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

/// 弹层面板底座：surface 纯色底（无边框，Kanesumi 铁律：无容器边框，靠色阶拉开层次）。
/// `progress` 用于展开位移（未启用布局位移，仅语义占位）。
pub fn render_panel_base(theme: &MetroTheme, rect: Rect, progress: f64, scene: &mut Scene) {
    let _ = progress;
    scene.fill_rounded_rect(theme.colors.surface, rect, theme.tokens.corner_radius);
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
    fn place_submenu_right_when_space() {
        let parent = Rect::new(100.0, 100.0, 160.0, 32.0);
        let size = Size::new(180.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_submenu(parent, size, screen, 2.0);
        assert_eq!(r.origin.x, parent.right() + 2.0, "默认父项右侧展开");
        assert_eq!(r.origin.y, 100.0);
    }

    #[test]
    fn place_submenu_flips_left_when_right_overflows() {
        // 父项 right = 700 + 160 = 860；右侧 862 + 180 = 1042 > 800 → 翻左
        let parent = Rect::new(700.0, 100.0, 100.0, 32.0);
        let size = Size::new(180.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_submenu(parent, size, screen, 2.0);
        assert!(r.origin.x < parent.origin.x, "翻到父项左侧");
        assert_eq!(r.origin.x, parent.origin.x - 2.0 - 180.0);
    }

    #[test]
    fn place_submenu_shifts_up_when_bottom_overflows() {
        // 父项贴屏底：y=560, h=32 → 底 592；子面板 y=560, h=120 → 底 680 > 600 → 上移
        let parent = Rect::new(100.0, 560.0, 160.0, 32.0);
        let size = Size::new(180.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_submenu(parent, size, screen, 2.0);
        assert_eq!(r.origin.y + r.size.height, screen.bottom(), "下缘齐屏底");
    }

    #[test]
    fn place_submenu_clamps_when_neither_side_fits() {
        // 屏幕仅 100 宽，面板 180 宽 —— 两侧都放不下，靠右夹紧
        let parent = Rect::new(20.0, 20.0, 60.0, 32.0);
        let size = Size::new(180.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 100.0, 600.0);
        let r = place_submenu(parent, size, screen, 2.0);
        assert!(r.origin.x >= screen.origin.x - 0.01, "不越屏左");
        // 面板宽 > 屏 → 靠右夹紧 = right - w = -80，被 max(origin.x=0) 拉回 0
        assert!(r.origin.x <= screen.right() - 1.0 || (screen.right() - size.width) < 0.0);
    }

    #[test]
    fn place_context_menu_defaults_to_lower_right() {
        // 屏幕中央锚点，右侧/下方空间足够 → 面板右上角即指针位置。
        let anchor = Point::new(300.0, 300.0);
        let size = Size::new(160.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.origin, Point::new(300.0, 300.0));
    }

    #[test]
    fn place_context_menu_slides_left_when_no_room_right() {
        // 右侧只剩 40px < 160 → 滑移回屏内（统一规则：滑移而不是镜像翻左；
        // 旧行为锚点右侧只剩 40px，面板会贴到 600，指针落在面板最后 40px 里）。
        let anchor = Point::new(760.0, 300.0);
        let size = Size::new(160.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.right(), screen.right());
        assert_eq!(r.origin.x, 800.0 - 160.0);
    }

    #[test]
    fn place_context_menu_flips_up_when_no_room_below() {
        // 下方只剩 20px < 120 → 翻上。
        let anchor = Point::new(300.0, 580.0);
        let size = Size::new(160.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.origin.y, 580.0 - 120.0);
    }

    #[test]
    fn place_context_menu_flips_up_and_slides_left_when_no_room() {
        // 右下两向都不够 → 上翻 + 左滑（y 翻到锚点上方，x 滑到屏右缘内）。
        let anchor = Point::new(780.0, 590.0);
        let size = Size::new(160.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.origin, Point::new(800.0 - 160.0, 590.0 - 120.0));
    }

    #[test]
    fn place_context_menu_clamps_to_screen_edges() {
        // 面板大于屏幕 → 贴屏角。
        let anchor = Point::new(400.0, 300.0);
        let size = Size::new(900.0, 700.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.origin.x, 0.0);
        assert_eq!(r.origin.y, 0.0);
        assert!(r.right() <= screen.right() + 0.01);
        assert!(r.bottom() <= screen.bottom() + 0.01);
    }

    #[test]
    fn place_context_menu_preserves_pointer_when_space() {
        // 锚点在屏幕角落仍有足够空间时，面板不偏移。
        let anchor = Point::new(10.0, 10.0);
        let size = Size::new(120.0, 80.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = place_context_menu(anchor, size, screen);
        assert_eq!(r.origin, Point::new(10.0, 10.0));
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

    /// M5-3 回归：**面板高于屏幕时不得画到屏幕外**（旧实现直接用 panel_size，
    /// 超长菜单的 y 可为负）。签名不变，夹紧在内部完成。
    #[test]
    fn place_popup_clamps_panel_taller_than_screen() {
        let trigger = Rect::new(40.0, 300.0, 120.0, 32.0);
        let size = Size::new(240.0, 1200.0); // 远高于屏幕
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let p = place_popup(trigger, size, screen, 4.0);
        assert!(p.rect.origin.y >= screen.origin.y - 0.01, "y 不得为负");
        assert!(
            p.rect.bottom() <= screen.bottom() + 0.01,
            "面板底不得越过屏幕底，实际 {}",
            p.rect.bottom()
        );
        assert!(p.rect.size.height <= screen.size.height + 0.01);
    }

    /// M5-3：面板宽于屏幕时同样夹到屏内。
    #[test]
    fn place_popup_clamps_panel_wider_than_screen() {
        let trigger = Rect::new(700.0, 100.0, 80.0, 32.0);
        let size = Size::new(1200.0, 120.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let p = place_popup(trigger, size, screen, 4.0);
        assert!(p.rect.origin.x >= screen.origin.x - 0.01);
        assert!(p.rect.right() <= screen.right() + 0.01);
    }

    // ── RM-1028-5 缺陷 1 回归（k-framework-fix1）：open/close 不得重建动画轨道 ──

    /// 正典收起曲线整段按 60fps 采样的单帧最大进度差（全摆幅）。
    /// 推导：曲线两端点固定，逐帧取样取最大相邻差；半程中断时实际摆幅减半，
    /// 阈值再放宽 2 倍，故这是宽松上界（任务书 §2-1「一帧应有步长的 2 倍」）。
    fn max_frame_step_of_close(fps: f64) -> f64 {
        let duration = kanesumi_anim::DURATION_SHEET_DISMISS;
        let n = (duration * fps).ceil() as usize;
        let mut prev = 0.0_f64;
        let mut max = 0.0_f64;
        for i in 1..=n {
            let t = ((i as f64 / fps) / duration).min(1.0);
            let y = kanesumi_anim::bezier_y(kanesumi_anim::CURVE_UWP_CLOSE, t as f32) as f64;
            max = max.max(y - prev);
            prev = y;
        }
        max
    }

    fn steady_open() -> PopupAnim {
        let mut anim = PopupAnim::new();
        anim.open();
        for _ in 0..120 {
            anim.update(1.0 / 60.0);
        }
        assert_eq!(anim.state(), PopupState::Open);
        anim
    }

    fn tick_until(anim: &mut PopupAnim, pred: impl Fn(f64) -> bool, max_frames: usize) -> f64 {
        for _ in 0..max_frames {
            anim.update(1.0 / 60.0);
            let v = anim.panel_progress();
            if pred(v) {
                return v;
            }
        }
        panic!("未在 {max_frames} 帧内到达目标进度，当前 {}", anim.panel_progress());
    }

    /// 场景 i（任务书 §2-1）：Open 稳态后 close，第一帧采样值 ≥ 0.9（不得跳到 0 附近）。
    #[test]
    fn close_from_steady_open_first_frame_stays_visible() {
        let mut anim = steady_open();
        anim.close();
        anim.update(1.0 / 60.0);
        assert!(
            anim.panel_progress() >= 0.9,
            "稳态关闭第一帧面板进度 {}，不得跳 0",
            anim.panel_progress()
        );
    }

    /// 场景 ii：开到约一半时 close，下一帧与上一帧差 ≤ 一帧应有步长的 2 倍。
    #[test]
    fn close_from_half_open_next_frame_within_step() {
        let mut anim = PopupAnim::new();
        anim.open();
        let v0 = tick_until(&mut anim, |v| v >= 0.5, 120);
        anim.close();
        anim.update(1.0 / 60.0);
        let v1 = anim.panel_progress();
        let bound = 2.0 * max_frame_step_of_close(60.0);
        assert!(
            (v1 - v0).abs() <= bound,
            "半开收起首帧差 {v1}-{v0} 超过 2×一帧步长 {bound}"
        );
        assert!(v1 >= 0.4, "半开收起首帧 {v1} 不得塌回 0 附近");
    }

    /// 场景 iii：收起一半时重新 open，呈现值不得回跳到中断之前之下
    /// （参 compositor/docs/AnimationRules.md：可打断、从当前呈现值接续）。
    #[test]
    fn reopen_from_half_close_continues_from_current() {
        let mut anim = PopupAnim::new();
        anim.open();
        let v_pre = tick_until(&mut anim, |v| v >= 0.5, 120);
        anim.close();
        let _v_mid = tick_until(&mut anim, |v| v <= 0.5, 120);
        anim.open();
        anim.update(1.0 / 60.0);
        assert!(
            anim.panel_progress() >= v_pre,
            "半收重开首帧 {} 低于中断前呈现值 {v_pre}",
            anim.panel_progress()
        );
    }

    // ── 逐帧曲线证据导出（任务书 §5：修前 / 修后同一剧本 CSV） ──

    fn step_log(anim: &mut PopupAnim, rows: &mut String, frame: &mut usize, t_ms: &mut f64) {
        anim.update(1.0 / 60.0);
        *t_ms += 1000.0 / 60.0;
        rows.push_str(&format!(
            "{},{:.1},{:?},{:.6},{:.6}\n",
            *frame,
            t_ms,
            anim.state(),
            anim.panel_progress(),
            anim.overlay_alpha()
        ));
        *frame += 1;
    }

    /// 剧本与 tools/perf/popup_continuity.sh 相同：开→半开收起→半收重开→稳态→收。
    /// 数据来自动画模型逐帧推进（`PopupAnim::update`），**不是屏幕读回**；夜班流程把它
    /// 复制为 docs/research/k_framework_fix1/progress_{before,after}.csv（Ether 仓）。
    #[test]
    fn export_progress_series_csv() {
        let mut anim = PopupAnim::new();
        let mut rows = String::from("frame,elapsed_ms,state,panel_progress,overlay_alpha\n");
        let mut frame = 0_usize;
        let mut t_ms = 0.0_f64;
        anim.open();
        while anim.panel_progress() < 0.5 && frame < 120 {
            step_log(&mut anim, &mut rows, &mut frame, &mut t_ms);
        }
        anim.close();
        while anim.panel_progress() > 0.5 && frame < 240 {
            step_log(&mut anim, &mut rows, &mut frame, &mut t_ms);
        }
        anim.open();
        while anim.state() != PopupState::Open && frame < 360 {
            step_log(&mut anim, &mut rows, &mut frame, &mut t_ms);
        }
        anim.close();
        while anim.state() != PopupState::Closed && frame < 480 {
            step_log(&mut anim, &mut rows, &mut frame, &mut t_ms);
        }
        assert!(frame < 480, "剧本未在 480 帧内走完");
        // 写到工作区根 target/（gitignored），夜班流程复制为 Ether 仓 docs 下的修前 / 修后 CSV。
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../target");
        std::fs::create_dir_all(&out).expect("创建 target 目录");
        std::fs::write(out.join("popup_progress_series.csv"), &rows).expect("写出 CSV");
    }
}
