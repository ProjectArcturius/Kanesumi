// interaction.rs —— 指针 / 焦点语义的框架侧单一来源。参 INTERACTION_CANON §Ⅱ/§Ⅲ/§Ⅳ。
//
// 这里只放**纯判定**与可注入时刻的状态机；树侧的挂载点在 `tree.rs`
// （`update_hover` / `pointer_*` / `set_focus` / `tooltip_*`）。分开的理由：
// 「什么算一次双击」「移动多少才算离开静止」「外点该不该吞」都是可单测的规则，
// 混在树的借用与绘制路径里只能靠整树夹具间接观察，改一个阈值要跑一屏用例。
//
// 状态转录（本文件 `mod tests`）：一行 = 一次合成输入 + 该输入之后的**可观察状态**
// （`hover` / `pressed` / `focus_visible` / 点击次数 / 弹层数 / 提示）。格式固定为
// `序号 输入 -> hover=… pressed=… focus_visible=…`，逐行写进断言。手感问题不许用
// 截图当判据：同一条输入序列必须产出逐字节相同的转录。

use kanesumi_core::Point;
use kanesumi_core::interaction::{
    DOUBLE_CLICK_TOLERANCE_PX, HOVER_TOLERANCE_PX, InteractionSettings,
};

use crate::event::PointerButton;

/// 双击检测器 —— 记录每次指针按下，判定「同按钮、间隔 ≤ 双击时长、位移在 4×4 px 框内」为双击。
///
/// 数值取自交互正典（参 INTERACTION_CANON §Ⅱ）：500 ms / 4×4 px 容差框；
/// 时长可由 `input.toml` 覆盖（[`InteractionSettings`]）。
///
/// 语义：第二次按下判定为双击（外壳随后下发 `InputEvent::DoubleClick`）；判定后复位，
/// 故三次快速点击 = 单击 + 双击 + 单击（第三次重新开始计数，Windows 惯例）。
///
/// 位置：原先在 `kanesumi-harness::app`，本件收进元素层 —— 双击窗口是交互正典的数值，
/// 框架与应用不能各写一份（迁移后 harness 只 `pub use` 本类型）。
#[derive(Debug, Clone, Default)]
pub struct ClickTracker {
    last_button: Option<PointerButton>,
    last_time_ms: u32,
    last_x: f32,
    last_y: f32,
    /// 可覆盖的双击时长（`input.toml`）。
    settings: InteractionSettings,
}

impl ClickTracker {
    /// 应用用户覆盖（`input.toml` 变更时由外壳调用）。
    pub fn set_settings(&mut self, settings: InteractionSettings) {
        self.settings = settings;
    }

    /// 记录一次按下（`time_ms` 为事件时间戳，同序单调）。返回 true = 构成双击。
    pub fn record(&mut self, button: PointerButton, x: f32, y: f32, time_ms: u32) -> bool {
        let tolerance = DOUBLE_CLICK_TOLERANCE_PX;
        let double = self.last_button == Some(button)
            && time_ms >= self.last_time_ms
            && time_ms.saturating_sub(self.last_time_ms) <= self.settings.double_click_ms
            && (x - self.last_x).abs() <= tolerance
            && (y - self.last_y).abs() <= tolerance;
        if double {
            // 判定后复位：第三次快速点击视为新单击（Windows 双击语义）。
            self.last_button = None;
            self.last_time_ms = 0;
            self.last_x = 0.0;
            self.last_y = 0.0;
        } else {
            self.last_button = Some(button);
            self.last_time_ms = time_ms;
            self.last_x = x;
            self.last_y = y;
        }
        double
    }

    /// 指针离开表面时复位（跨表面的快速点击不算双击）。
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// 指针是否已移出「静止」容差框（正典 §Ⅲ：4×4 px，即任一轴位移 > 2 px）。
///
/// 只判轴对齐的框，不判欧氏距离：正典给的是 `GetSystemMetrics` 的矩形容差，
/// 斜向 2 px / 2 px 仍在框内（欧氏会误判为移动）。
pub(crate) fn moved_beyond_tolerance(from: Point, to: Point) -> bool {
    (to.x - from.x).abs() > HOVER_TOLERANCE_PX || (to.y - from.y).abs() > HOVER_TOLERANCE_PX
}

/// 轻量关闭的来源。优先级：焦点控件先处理 Esc（逐层回退）→ 顶层弹层 → 外点 → 失焦。
///
/// 失焦不在本枚举里：它不是输入事件，没有「消费」语义，由 [`crate::Tree::dismiss_popups`]
/// 一次收掉所有非模态轻触弹层。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DismissCause {
    /// `Esc`：只关最内层可轻触关闭的弹层；被控件先消费时不关。
    Escape,
    /// 弹层外按下：关最内层可轻触关闭的弹层，且**这次按下 / 释放都不传给下层**。
    OutsideClick,
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::{Scene, SceneCommand};
    use kanesumi_core::{Rect, Size};

    use crate::event::{Event, Key};
    use crate::id::WidgetId;
    use crate::props::{Align, Insets, LayoutProps};
    use crate::testing::TestHarness;
    use crate::tree::{PopupMotion, PopupSpec};
    use crate::widget::{ArrangeCtx, EventCtx, MeasureCtx, PaintCtx, Widget};

    /// 测试控件记录到框架动作队列的事件（测试按序断言，不读控件内部）。
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Seen {
        Enter,
        Leave,
        Down,
        Up,
        Click,
        Double,
        Context,
        KeyDown,
    }

    /// 可命中 / 可聚焦的探针。`open_menu` = 右键请求时打开一个锚定自身的弹层。
    struct Probe {
        size: Size,
        focusable: bool,
        open_menu: bool,
    }

    impl Probe {
        fn new(w: f32, h: f32) -> Self {
            Self {
                size: Size::new(w, h),
                focusable: true,
                open_menu: false,
            }
        }

        fn menu(w: f32, h: f32) -> Self {
            Self {
                size: Size::new(w, h),
                focusable: true,
                open_menu: true,
            }
        }
    }

    impl Widget for Probe {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            self.size
        }

        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            scene.fill_rect(kanesumi_core::Color::WHITE, ctx.rect());
        }

        fn focusable(&self) -> bool {
            self.focusable
        }

        fn has_visual_states(&self) -> bool {
            true
        }

        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::PointerEnter => ctx.emit(Seen::Enter),
                Event::PointerLeave => ctx.emit(Seen::Leave),
                Event::PointerDown { .. } => ctx.emit(Seen::Down),
                Event::PointerUp { .. } => ctx.emit(Seen::Up),
                Event::Click => ctx.emit(Seen::Click),
                Event::DoubleTapped { .. } => ctx.emit(Seen::Double),
                Event::ContextRequested { pos } if self.open_menu => {
                    ctx.emit(Seen::Context);
                    // 模拟应用：右键释放时打开上下文菜单。`motion: None` 让关闭是即时的，
                    // 测试不依赖墙钟动画。
                    ctx.open_popup(
                        Probe::new(100.0, 80.0),
                        PopupSpec {
                            at: Some(*pos),
                            motion: PopupMotion::None,
                            ..PopupSpec::default()
                        },
                    );
                }
                Event::ContextRequested { .. } => ctx.emit(Seen::Context),
                Event::KeyDown { .. } => ctx.emit(Seen::KeyDown),
                _ => {}
            }
        }
    }

    /// 透明容器：只做父子悬停链的父级，自身不命中。
    struct Pane;

    impl Widget for Pane {
        fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
            let mut w = 0.0;
            let mut h: f32 = 0.0;
            for c in ctx.children() {
                let d = ctx.measure_child(c, Size::new(f32::INFINITY, f32::INFINITY));
                w += d.width;
                h = h.max(d.height);
            }
            Size::new(w.max(200.0), h.max(40.0))
        }

        fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
            let n = ctx.children().len().max(1) as f32;
            let cw = rect.size.width / n;
            for (i, c) in ctx.children().into_iter().enumerate() {
                ctx.arrange_child(
                    c,
                    Rect::new(
                        rect.origin.x + i as f32 * cw,
                        rect.origin.y,
                        cw,
                        rect.size.height,
                    ),
                );
            }
        }

        fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

        fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
            false
        }

        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::PointerEnter => ctx.emit(Seen::Enter),
                Event::PointerLeave => ctx.emit(Seen::Leave),
                _ => {}
            }
        }
    }

    fn layout(x: f32, y: f32, w: f32, h: f32) -> LayoutProps {
        LayoutProps {
            margin: Insets::new(x, y, 0.0, 0.0),
            width: Some(w),
            height: Some(h),
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        }
    }

    fn probe_at(h: &mut TestHarness, x: f32, y: f32, w: f32, hh: f32, probe: Probe) -> WidgetId {
        h.tree.insert_with(h.root(), probe, layout(x, y, w, hh))
    }

    fn seen(h: &mut TestHarness) -> Vec<Seen> {
        h.take::<Seen>().into_iter().map(|(_, a)| a).collect()
    }

    /// 焦点框是框架在 compose 阶段加的 StrokeRect；指针聚焦应为 0 条。
    fn focus_strokes(h: &TestHarness) -> usize {
        h.last
            .scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count()
    }

    /// 一段状态转录：每行 = 输入 + 该输入之后的可观察状态。
    #[derive(Default)]
    struct Trace(Vec<String>);

    impl Trace {
        fn state(&mut self, seq: u32, input: &str, h: &TestHarness, id: WidgetId) {
            let s = h.tree.states_of(id);
            self.0.push(format!(
                "{seq:02} {input:<26} hover={} pressed={} focus_visible={}",
                s.hovered as u8, s.pressed as u8, s.keyboard_focused as u8
            ));
        }

        fn note(&mut self, seq: u32, input: &str, observed: &str) {
            self.0.push(format!("{seq:02} {input:<26} {observed}"));
        }

        fn expect(&self, expected: &str) {
            // 报告要贴逐段转录：`KANESUMI_TRANSCRIPT=1 cargo test … -- --nocapture` 取原文。
            if std::env::var_os("KANESUMI_TRANSCRIPT").is_some() {
                println!("----\n{}", self.0.join("\n"));
            }
            // 对齐只影响可读性：逐行压掉多余空白再比，数值与顺序仍逐字节钉死。
            let squash = |s: &str| {
                s.lines()
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert_eq!(
                squash(&self.0.join("\n")),
                squash(expected),
                "状态转录与钉死的行为不符"
            );
        }
    }

    // ── ① 进入 / 离开：父级在子节点间移动时不闪断 ─────────────────────────────

    #[test]
    fn transcript_enter_leave() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.move_to(Point::new(250.0, 150.0));
        t.state(1, "move(outside)", &h, a);
        h.move_to(h.center(a));
        t.state(2, "move(inside)", &h, a);
        h.tree.pointer_leave();
        t.state(3, "pointer_leave", &h, a);
        t.expect(
            "01 move(outside)            hover=0 pressed=0 focus_visible=0\n\
             02 move(inside)             hover=1 pressed=0 focus_visible=0\n\
             03 pointer_leave            hover=0 pressed=0 focus_visible=0",
        );
    }

    #[test]
    fn transcript_parent_hover_survives_child_switch() {
        let mut h = TestHarness::new(300.0, 200.0);
        let pane = h
            .tree
            .insert_with(h.root(), Pane, layout(10.0, 10.0, 220.0, 40.0));
        let a = h.tree.insert_with(
            pane,
            Probe::new(80.0, 30.0),
            LayoutProps {
                width: Some(80.0),
                height: Some(30.0),
                ..LayoutProps::default()
            },
        );
        let b = h.tree.insert_with(
            pane,
            Probe::new(80.0, 30.0),
            LayoutProps {
                width: Some(80.0),
                height: Some(30.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        let mut t = Trace::default();
        h.move_to(h.center(a));
        t.state(1, "move(child_a)", &h, pane);
        t.state(2, "move(child_a)", &h, a);
        h.move_to(h.center(b));
        t.state(3, "move(child_b)", &h, pane);
        t.state(4, "move(child_b)", &h, a);
        t.state(5, "move(child_b)", &h, b);
        t.expect(
            "01 move(child_a)            hover=1 pressed=0 focus_visible=0\n\
             02 move(child_a)            hover=1 pressed=0 focus_visible=0\n\
             03 move(child_b)            hover=1 pressed=0 focus_visible=0\n\
             04 move(child_b)            hover=0 pressed=0 focus_visible=0\n\
             05 move(child_b)            hover=1 pressed=0 focus_visible=0",
        );
        // 父级只进一次、没有离开：子节点切换不产生 Enter/Leave 抖动。
        assert_eq!(
            seen(&mut h),
            [Seen::Enter, Seen::Enter, Seen::Leave, Seen::Enter]
        );
    }

    // ── ② 按下 / 取消：移出边界后释放不触发 click ─────────────────────────────

    #[test]
    fn transcript_press_then_release_inside_clicks() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.move_to(h.center(a));
        t.state(1, "move(a)", &h, a);
        h.press_at(h.center(a), PointerButton::Left);
        t.state(2, "press(a)", &h, a);
        h.release_at(h.center(a), PointerButton::Left);
        t.state(3, "release(a)", &h, a);
        t.expect(
            "01 move(a)                  hover=1 pressed=0 focus_visible=0\n\
             02 press(a)                 hover=1 pressed=1 focus_visible=0\n\
             03 release(a)               hover=1 pressed=0 focus_visible=0",
        );
        assert_eq!(
            seen(&mut h),
            [Seen::Enter, Seen::Down, Seen::Up, Seen::Click]
        );
    }

    #[test]
    fn transcript_press_cancelled_by_moving_out() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.press_at(h.center(a), PointerButton::Left);
        t.state(1, "press(a)", &h, a);
        h.move_to(Point::new(250.0, 150.0));
        t.state(2, "move(outside)", &h, a);
        h.release_at(Point::new(250.0, 150.0), PointerButton::Left);
        t.state(3, "release(outside)", &h, a);
        t.expect(
            "01 press(a)                 hover=1 pressed=1 focus_visible=0\n\
             02 move(outside)            hover=0 pressed=0 focus_visible=0\n\
             03 release(outside)         hover=0 pressed=0 focus_visible=0",
        );
        // 松开事件仍投给捕获者（拖拽语义），但**没有** Click。
        assert_eq!(
            seen(&mut h),
            [Seen::Enter, Seen::Down, Seen::Leave, Seen::Up]
        );
    }

    // ── ③ 循环回调：按住移回原元素恢复 pressed（Qt QAbstractButton）────────────

    #[test]
    fn transcript_press_cycle_back_inside() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.press_at(h.center(a), PointerButton::Left);
        t.state(1, "press(a)", &h, a);
        h.move_to(Point::new(250.0, 150.0));
        t.state(2, "move(outside)", &h, a);
        h.move_to(h.center(a));
        t.state(3, "move(back_to_a)", &h, a);
        h.release_at(h.center(a), PointerButton::Left);
        t.state(4, "release(a)", &h, a);
        t.expect(
            "01 press(a)                 hover=1 pressed=1 focus_visible=0\n\
             02 move(outside)            hover=0 pressed=0 focus_visible=0\n\
             03 move(back_to_a)          hover=1 pressed=1 focus_visible=0\n\
             04 release(a)               hover=1 pressed=0 focus_visible=0",
        );
        assert_eq!(
            seen(&mut h),
            [
                Seen::Enter,
                Seen::Down,
                Seen::Leave,
                Seen::Enter,
                Seen::Up,
                Seen::Click
            ]
        );
    }

    // ── ④ 焦点可见性：键盘 = 1，鼠标 = 0 ──────────────────────────────────────

    #[test]
    fn transcript_focus_visibility_keyboard_vs_pointer() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        let b = probe_at(&mut h, 10.0, 60.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.tab();
        t.state(1, "Tab", &h, a);
        t.state(1, "Tab", &h, b);
        h.click(b);
        t.state(2, "click(b)", &h, b);
        t.state(2, "click(b)", &h, a);
        assert_eq!(focus_strokes(&h), 0, "指针聚焦不画焦点框");
        h.click(b);
        t.state(3, "click(b_again)", &h, b);
        h.tab();
        t.state(4, "Tab", &h, a);
        t.state(4, "Tab", &h, b);
        assert_eq!(focus_strokes(&h), 2, "键盘焦点画外 2px + 内 1px");
        t.expect(
            "01 Tab                      hover=0 pressed=0 focus_visible=1\n\
             01 Tab                      hover=0 pressed=0 focus_visible=0\n\
             02 click(b)                 hover=1 pressed=0 focus_visible=0\n\
             02 click(b)                 hover=0 pressed=0 focus_visible=0\n\
             03 click(b_again)           hover=1 pressed=0 focus_visible=0\n\
             04 Tab                      hover=0 pressed=0 focus_visible=1\n\
             04 Tab                      hover=1 pressed=0 focus_visible=0",
        );
        assert_eq!(h.tree.focused(), Some(a));
        assert!(h.tree.focus_is_keyboard(), "Tab 重新取得键盘焦点");
    }

    #[test]
    fn transcript_tab_order_forward_and_backward() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        let b = probe_at(&mut h, 10.0, 50.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        let c = probe_at(&mut h, 10.0, 90.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let names = |h: &TestHarness| match h.tree.focused() {
            Some(id) if id == a => "A",
            Some(id) if id == b => "B",
            Some(id) if id == c => "C",
            None => "none",
            _ => "?",
        };
        let mut t = Trace::default();
        h.tab();
        t.note(1, "Tab", &format!("focus={}", names(&h)));
        h.tab();
        t.note(2, "Tab", &format!("focus={}", names(&h)));
        h.tab();
        t.note(3, "Tab", &format!("focus={}", names(&h)));
        h.tab();
        t.note(4, "Tab(wrap)", &format!("focus={}", names(&h)));
        h.shift_tab();
        t.note(5, "Shift+Tab", &format!("focus={}", names(&h)));
        h.shift_tab();
        t.note(6, "Shift+Tab", &format!("focus={}", names(&h)));
        t.expect(
            "01 Tab                      focus=A\n\
             02 Tab                      focus=B\n\
             03 Tab                      focus=C\n\
             04 Tab(wrap)                focus=A\n\
             05 Shift+Tab                focus=C\n\
             06 Shift+Tab                focus=B",
        );
        assert!(h.tree.focus_is_keyboard(), "Tab 导航始终是键盘焦点");
    }

    // ── ⑤ 双击窗口：500 ms / ±2 px（时刻由测试注入，不读系统钟）────────────────

    #[test]
    fn transcript_double_click_window() {
        let mut c = ClickTracker::default();
        let mut t = Trace::default();
        let step = |t: &mut Trace,
                    seq: u32,
                    label: &str,
                    c: &mut ClickTracker,
                    x: f32,
                    y: f32,
                    ms: u32| {
            let double = c.record(PointerButton::Left, x, y, ms);
            t.note(seq, label, &format!("double={}", double as u8));
        };
        step(&mut t, 1, "press(t=1000,x=10)", &mut c, 10.0, 10.0, 1000);
        step(&mut t, 2, "press(t=1500,x=12)", &mut c, 12.0, 10.0, 1500);
        step(&mut t, 3, "press(t=2001,x=10)", &mut c, 10.0, 10.0, 2001);
        step(&mut t, 4, "press(t=2002,x=12.5)", &mut c, 12.5, 10.0, 2002);
        // `input.toml` 把窗口改到 300 ms 后，旧边界随之改变。
        c.set_settings(InteractionSettings {
            double_click_ms: 300,
            ..Default::default()
        });
        c.reset();
        step(
            &mut t,
            5,
            "press(t=3000,cfg=300ms)",
            &mut c,
            10.0,
            10.0,
            3000,
        );
        step(
            &mut t,
            6,
            "press(t=3300,cfg=300ms)",
            &mut c,
            10.0,
            10.0,
            3300,
        );
        t.expect(
            "01 press(t=1000,x=10)       double=0\n\
             02 press(t=1500,x=12)       double=1\n\
             03 press(t=2001,x=10)       double=0\n\
             04 press(t=2002,x=12.5)     double=0\n\
             05 press(t=3000,cfg=300ms)  double=0\n\
             06 press(t=3300,cfg=300ms)  double=1",
        );
    }

    #[test]
    fn transcript_double_tap_still_clicks_once() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.press_at(h.center(a), PointerButton::Left);
        t.state(1, "press(a)", &h, a);
        h.tree.pointer_double(h.center(a), PointerButton::Left);
        t.state(2, "double(a)", &h, a);
        h.release_at(h.center(a), PointerButton::Left);
        h.frame();
        t.state(3, "release(a)", &h, a);
        let events = seen(&mut h);
        t.note(4, "events", &format!("{events:?}"));
        t.expect(
            "01 press(a)                 hover=1 pressed=1 focus_visible=0\n\
             02 double(a)                hover=1 pressed=1 focus_visible=0\n\
             03 release(a)               hover=1 pressed=0 focus_visible=0\n\
             04 events                   [Enter, Down, Double, Up, Click]",
        );
        assert_eq!(
            events,
            [Seen::Enter, Seen::Down, Seen::Double, Seen::Up, Seen::Click],
            "双击不丢单击，且 DoubleTapped 在 PointerUp 之前"
        );
    }

    // ── ⑥ 弹层轻消失：Esc → 外点 → 失焦 → 再次右键打开同一弹层 ─────────────────

    fn open_popup(h: &mut TestHarness, anchor: WidgetId, at: Point, modal: bool) -> WidgetId {
        h.tree.open_popup(
            Probe::new(100.0, 80.0),
            PopupSpec {
                anchor: Some(anchor),
                at: Some(at),
                modal,
                motion: PopupMotion::None,
                ..PopupSpec::default()
            },
        )
    }

    #[test]
    fn transcript_popup_escape_consumes_and_reopens() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        let p = open_popup(&mut h, a, Point::new(120.0, 60.0), false);
        t.note(
            1,
            "open_popup",
            &format!("popups={}", h.tree.popups().count()),
        );
        assert!(h.tree.focus(p, true), "弹层内可聚焦");
        t.note(
            2,
            "focus_inside",
            &format!("focus_visible={}", h.tree.focus_is_keyboard() as u8),
        );
        let consumed = h.key(Key::Escape);
        t.note(
            3,
            "Esc",
            &format!(
                "consumed={} popups={}",
                consumed as u8,
                h.tree.popups().count()
            ),
        );
        let p2 = open_popup(&mut h, a, Point::new(120.0, 60.0), false);
        t.note(
            4,
            "right_click_reopen",
            &format!(
                "popups={} fresh={}",
                h.tree.popups().count(),
                (p2 != p) as u8
            ),
        );
        t.expect(
            "01 open_popup               popups=1\n\
             02 focus_inside             focus_visible=1\n\
             03 Esc                      consumed=1 popups=0\n\
             04 right_click_reopen       popups=1 fresh=1",
        );
        assert_eq!(h.tree.focused(), Some(a), "关闭后焦点回到锚点");
    }

    #[test]
    fn transcript_popup_escape_closes_one_layer_at_a_time() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        open_popup(&mut h, a, Point::new(120.0, 60.0), false);
        open_popup(&mut h, a, Point::new(160.0, 100.0), false);
        t.note(
            1,
            "open_two",
            &format!("popups={}", h.tree.popups().count()),
        );
        let first = h.key(Key::Escape);
        t.note(
            2,
            "Esc(top)",
            &format!(
                "consumed={} popups={}",
                first as u8,
                h.tree.popups().count()
            ),
        );
        let second = h.key(Key::Escape);
        t.note(
            3,
            "Esc(next)",
            &format!(
                "consumed={} popups={}",
                second as u8,
                h.tree.popups().count()
            ),
        );
        let third = h.key(Key::Escape);
        t.note(
            4,
            "Esc(empty)",
            &format!(
                "consumed={} popups={}",
                third as u8,
                h.tree.popups().count()
            ),
        );
        t.expect(
            "01 open_two                 popups=2\n\
             02 Esc(top)                 consumed=1 popups=1\n\
             03 Esc(next)                consumed=1 popups=0\n\
             04 Esc(empty)               consumed=0 popups=0",
        );
    }

    #[test]
    fn transcript_popup_outside_click_is_swallowed() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        h.move_to(h.center(a));
        h.frame();
        // 悬停 Enter 是上一动作的产物，先清空，外点断言只看这次点击。
        assert_eq!(seen(&mut h), [Seen::Enter]);
        let mut t = Trace::default();
        open_popup(&mut h, a, Point::new(200.0, 100.0), false);
        t.note(
            1,
            "open_popup",
            &format!("popups={}", h.tree.popups().count()),
        );
        h.press_at(h.center(a), PointerButton::Left);
        h.release_at(h.center(a), PointerButton::Left);
        t.note(
            2,
            "click_outside",
            &format!("popups={}", h.tree.popups().count()),
        );
        t.expect(
            "01 open_popup               popups=1\n\
             02 click_outside            popups=0",
        );
        assert!(
            seen(&mut h).is_empty(),
            "外点整次点击不传给下层（含按下、释放、Click）"
        );
    }

    #[test]
    fn transcript_popup_focus_loss_keeps_modal() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        open_popup(&mut h, a, Point::new(120.0, 60.0), false);
        open_popup(&mut h, a, Point::new(160.0, 100.0), true);
        t.note(
            1,
            "open_two",
            &format!("popups={}", h.tree.popups().count()),
        );
        h.tree.dismiss_popups();
        t.note(
            2,
            "window_blur",
            &format!("popups={}", h.tree.popups().count()),
        );
        open_popup(&mut h, a, Point::new(120.0, 60.0), false);
        t.note(
            3,
            "reopen_normal",
            &format!("popups={}", h.tree.popups().count()),
        );
        t.expect(
            "01 open_two                 popups=2\n\
             02 window_blur              popups=1\n\
             03 reopen_normal            popups=2",
        );
        assert!(
            h.tree
                .popups()
                .any(|p| h.tree.popup_spec(p).is_some_and(|s| s.modal)),
            "模态弹层在失焦时保留"
        );
    }

    #[test]
    fn transcript_right_click_reopens_same_popup() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = probe_at(&mut h, 10.0, 10.0, 80.0, 24.0, Probe::menu(80.0, 24.0));
        h.frame();
        let mut t = Trace::default();
        h.right_click(a);
        t.note(
            1,
            "right_click",
            &format!("popups={}", h.tree.popups().count()),
        );
        assert!(h.key(Key::Escape));
        t.note(2, "Esc", &format!("popups={}", h.tree.popups().count()));
        h.right_click(a);
        t.note(
            3,
            "right_click",
            &format!("popups={}", h.tree.popups().count()),
        );
        h.click_at(Point::new(260.0, 160.0));
        t.note(
            4,
            "click_outside",
            &format!("popups={}", h.tree.popups().count()),
        );
        h.right_click(a);
        t.note(
            5,
            "right_click",
            &format!("popups={}", h.tree.popups().count()),
        );
        t.expect(
            "01 right_click              popups=1\n\
             02 Esc                      popups=0\n\
             03 right_click              popups=1\n\
             04 click_outside            popups=0\n\
             05 right_click              popups=1",
        );
        let p = h.tree.popups().next().expect("菜单已重开");
        assert_eq!(
            h.tree.popup_spec(p).unwrap().anchor,
            Some(a),
            "重开的还是同一锚点"
        );
    }

    // ── ⑦ 悬停提示延迟：静止满 500 ms 才出，移动超出 4×4 px 框即撤回 ────────────

    fn tooltip_probe(h: &mut TestHarness) -> WidgetId {
        let a = probe_at(h, 10.0, 10.0, 80.0, 24.0, Probe::new(80.0, 24.0));
        h.tree.set_tooltip(a, "提示");
        h.frame();
        a
    }

    #[test]
    fn transcript_tooltip_delay_and_movement_withdraw() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = tooltip_probe(&mut h);
        let at = h.center(a);
        let mut t = Trace::default();
        h.move_to(at);
        t.note(
            1,
            "move(a)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.idle(0.40);
        t.note(
            2,
            "idle(0.40)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.idle(0.11);
        t.note(
            3,
            "idle(0.51)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.move_to(Point::new(at.x + 2.0, at.y));
        t.note(
            4,
            "move(+2px)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.move_to(Point::new(at.x + 5.0, at.y));
        t.note(
            5,
            "move(+5px)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.idle(0.40);
        t.note(
            6,
            "idle(0.40)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.idle(0.11);
        t.note(
            7,
            "idle(0.51)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        t.expect(
            "01 move(a)                  tip=0\n\
             02 idle(0.40)               tip=0\n\
             03 idle(0.51)               tip=1\n\
             04 move(+2px)               tip=1\n\
             05 move(+5px)               tip=0\n\
             06 idle(0.40)               tip=0\n\
             07 idle(0.51)               tip=1",
        );
    }

    #[test]
    fn transcript_tooltip_movement_resets_still_timer() {
        let mut h = TestHarness::new(300.0, 200.0);
        let a = tooltip_probe(&mut h);
        let at = h.center(a);
        let mut t = Trace::default();
        h.move_to(at);
        h.idle(0.30);
        t.note(
            1,
            "move+0.30s",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.move_to(Point::new(at.x + 5.0, at.y));
        h.idle(0.30);
        t.note(
            2,
            "move(+5px)+0.30s",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        h.idle(0.21);
        t.note(
            3,
            "idle(0.51)",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        // 容差内的 2 px 抖动不重置计时：再等 0.30 s 仍应显示（起始于上一次 +5px）。
        h.move_to(Point::new(at.x + 7.0, at.y));
        h.idle(0.30);
        t.note(
            4,
            "jitter(+2px)+0.30s",
            &format!("tip={}", h.tree.tooltip_popup().is_some() as u8),
        );
        t.expect(
            "01 move+0.30s              tip=0\n\
             02 move(+5px)+0.30s        tip=0\n\
             03 idle(0.51)              tip=1\n\
             04 jitter(+2px)+0.30s      tip=1",
        );
    }
}
