// 弹层表面动画在外壳侧的接线契约。参 docs/FEEL_PLAN.md §Ⅰ-4、platform/popups.rs。
//
// 这里不碰 Wayland（没有会话也能跑）：验证的是**元素树 → 外壳请求**这一段 ——
// 外壳要从 `PopupRequest` 里读出「入场动画 / 正在退场 / 退场动画」三件事，
// 并且退场期间不把输入投给正在消失的弹层。

use kanesumi_core::{Color, Point, Rect, Size};
use kanesumi_harness::app::{App, AppConfig, InputEvent};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, LayoutProps, PopupSide, PopupSpec, Tree, WidgetId};
use kanesumi_harness::role::EtherRole;
use kanesumi_harness::tree_host::{TreeApp, TreeHost};

struct PopupApp {
    config: AppConfig,
    anchor: Option<WidgetId>,
    popup: Option<WidgetId>,
}

impl PopupApp {
    fn new() -> Self {
        Self {
            config: AppConfig::new("org.ether.test", "popup-motion", EtherRole::Browser, 400.0, 300.0),
            anchor: None,
            popup: None,
        }
    }

    fn open(&mut self, tree: &mut Tree, side: PopupSide) {
        let a = self.anchor.unwrap();
        let p = tree.open_popup(
            Border::new().background(Color::rgb(0.3, 0.3, 0.3)),
            PopupSpec {
                anchor: Some(a),
                side,
                ..PopupSpec::default()
            },
        );
        // 弹层里放一个会自绘的子项：内容就绪 / 命中都需要真实绘制产物。
        tree.insert_with(
            p,
            Border::new().background(Color::rgb(0.4, 0.4, 0.4)),
            LayoutProps {
                width: Some(120.0),
                height: Some(80.0),
                ..LayoutProps::default()
            },
        );
        self.popup = Some(p);
    }
}

impl TreeApp for PopupApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn build(&mut self, tree: &mut Tree) {
        let col = tree.insert(tree.root(), Stack::column());
        self.anchor = Some(tree.insert_with(
            col,
            Border::new().background(Color::rgb(0.2, 0.2, 0.2)),
            LayoutProps {
                width: Some(60.0),
                height: Some(24.0),
                ..LayoutProps::default()
            },
        ));
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn host() -> TreeHost<PopupApp> {
    let mut h = TreeHost::new(PopupApp::new());
    // 外壳启动时（layer 主表面就绪后）会把放置区告诉 App —— 弹层分离由此开启。
    assert!(h.enable_popups(Rect::new(0.0, 0.0, 400.0, 300.0)));
    frame(&mut h);
    h
}

fn frame<A: TreeApp>(h: &mut TreeHost<A>) {
    h.advance_clock(1.0 / 60.0);
    h.update(1.0 / 60.0);
    let mut scene = kanesumi_canvas::Scene::default();
    h.render_into(&kanesumi_harness::element::testing::test_engine(), Size::new(400.0, 300.0), &mut scene);
}

/// 打开 → PopupRequest 带入场动画（方向与 side 对应）；关闭 → 同一 key 变 `closing` 且带退场动画。
#[test]
fn popup_request_carries_enter_and_exit_motion() {
    let mut h = host();
    let side = PopupSide::Bottom;
    let (app, tree) = h.parts_mut();
    app.open(tree, side);
    frame(&mut h);

    let reqs = h.popups();
    let req = reqs.first().expect("应申报一个弹层表面");
    assert!(!req.closing, "新弹层没有退场");
    let enter = req.enter.expect("缺省应带入场动画");
    assert_eq!(
        (enter.from_x, enter.from_y),
        (0.0, -24.0),
        "面板在下方 → 从上方滑下来"
    );
    assert!(req.exit.is_none());
    assert!(req.motion_name.starts_with("弹层:"), "{}", req.motion_name);
    let key = req.key;

    // Esc 路径等价于 close_popup：节点留到 Done，但请求立刻变 closing。
    let id = h.tree().popups().next().unwrap();
    h.parts_mut().1.close_popup(id);
    let reqs = h.popups();
    let req = reqs.first().expect("退场中仍要申报（表面还在放动画）");
    assert!(req.closing, "退场中 closing=true");
    assert!(req.enter.is_none(), "退场中不再播入场");
    let exit = req.exit.expect("应带退场动画");
    assert_eq!(exit.duration_ms, 150);
    assert_eq!(key, req.key, "复用同一表面：key 不得变");

    h.popup_motion_done(key);
    assert!(h.popups().is_empty(), "Done 后不再申报 → 外壳销毁表面");
    assert!(h.tree().popups().next().is_none());
}

/// 退场期间弹层表面上的输入一律丢弃（合成器抓取还没撤，事件仍会投进来）。
#[test]
fn popup_input_is_dropped_while_closing() {
    let mut h = host();
    let (app, tree) = h.parts_mut();
    app.open(tree, PopupSide::Bottom);
    frame(&mut h);

    let key = h.popups()[0].key;
    let id = h.tree().popups().next().unwrap();
    // 弹层内的一个点（弹层左上 + 一点），退场前应当能被树接收。
    let o = h.tree().popup_extent(id).unwrap().origin;
    let inside = Point::new(o.x + 10.0, o.y + 10.0);
    let hover_before = h.tree().hit(inside);
    assert!(hover_before.is_some(), "退场前弹层可命中");

    h.parts_mut().1.close_popup(id);
    // 退场中投给该弹层表面的输入整条丢弃：连树的指针位置都不该被这条事件改。
    // （合成器的抓取还没撤，按下 / 移动仍会投到这里；键盘事件同样走这条路，
    //  放进去会让 Esc / Enter 作用到别的弹层上。）
    let pointer_before = h.tree().pointer();
    h.popup_input(key, InputEvent::PointerMoved { x: 5.0, y: 5.0 });
    h.popup_input(
        key,
        InputEvent::PointerPressed {
            x: 5.0,
            y: 5.0,
            button: kanesumi_harness::element::PointerButton::Left,
            modifiers: kanesumi_harness::element::Modifiers::NONE,
        },
    );
    assert_eq!(
        h.tree().pointer(),
        pointer_before,
        "退场期间弹层表面的输入被整条丢弃"
    );
    assert!(
        h.tree().hit(inside).is_none(),
        "退场中的弹层不参与命中"
    );
}

/// 合成器销毁表面时（`popup_dismissed`）：不留退场残影，弹层立刻结算。
#[test]
fn dismissed_popup_settles_without_waiting_for_animation() {
    let mut h = host();
    let (app, tree) = h.parts_mut();
    app.open(tree, PopupSide::Bottom);
    frame(&mut h);
    let key = h.popups()[0].key;

    h.popup_dismissed(key);
    assert!(
        h.popups().is_empty(),
        "表面已被合成器销毁 → 不得再申报（否则会重建出残影）"
    );
    assert_eq!(h.tree().popups().count(), 0);
}
