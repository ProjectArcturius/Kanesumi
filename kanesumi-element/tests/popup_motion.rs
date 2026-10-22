// 弹层表面动画契约测试。参 docs/FEEL_PLAN.md §Ⅰ-2 / §Ⅰ-4、transitions.rs。
//
// 分工在这里被钉死：元素树只产出「播什么」（方位 → Edge、时长 / 曲线、中断接续的起点），
// 外壳把视觉量交给合成器。故元素侧能测的是**计划**与**状态机**：
//   · 方向映射（四方位 + 点锚定的右键菜单）；
//   · 退场：节点留到 Done、期间不接输入、Done 才移除；
//   · 退场中重开：从当前呈现值接续，不跳回起点；
//   · `motion: None` 完全没有动画计划（工具提示缺省）；
//   · Done 丢失的兜底（不能让弹层永久挡在界面上）。

use kanesumi_core::{Color, Point, Rect};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Border;
use kanesumi_element::transitions::visual_at_ms;
use kanesumi_element::{
    LayoutProps, PopupMotion, PopupSide, PopupSpec, TransitionParams, WidgetId,
};

fn panel() -> Border {
    Border::new().background(Color::rgb(0.3, 0.3, 0.3))
}

fn fixed(w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        ..LayoutProps::default()
    }
}

/// 分离模式的树（外壳另开 xdg_popup 承载弹层）—— 只有这种形态才有「弹层自己的表面」可动。
fn detached() -> TestHarness {
    let mut h = TestHarness::new(400.0, 320.0);
    h.tree.set_detached_popups(true);
    h.tree.set_popup_bounds(Some(Rect::new(0.0, 0.0, 400.0, 320.0)));
    h
}

fn anchor(h: &mut TestHarness) -> WidgetId {
    let a = h.tree.insert_with(h.root(), panel(), fixed(40.0, 20.0));
    h.frame();
    a
}

/// 开一个带锚点的弹层（内部放一个可命中的子控件，用于「退场不接输入」断言）。
fn open(h: &mut TestHarness, a: WidgetId, side: PopupSide, at: Option<Point>) -> WidgetId {
    let spec = PopupSpec {
        anchor: Some(a),
        at,
        side,
        ..PopupSpec::default()
    };
    let p = h.tree.open_popup(panel(), spec);
    h.tree.insert_with(p, panel(), fixed(80.0, 60.0));
    h.frame();
    p
}

/// 四个方位 + 点锚定：入场方向由「面板在锚点哪一侧」推出（面板在下方 → 从上方滑下来）。
#[test]
fn enter_direction_follows_popup_side() {
    let mut h = detached();
    let a = anchor(&mut h);
    let d = TransitionParams::POP_IN.offset;

    for (side, want) in [
        (PopupSide::Bottom, (0.0, -d)),
        (PopupSide::Top, (0.0, d)),
        (PopupSide::Left, (d, 0.0)),
        (PopupSide::Right, (-d, 0.0)),
    ] {
        let p = open(&mut h, a, side, None);
        let enter = h
            .tree
            .popup_motion(p)
            .and_then(|m| m.enter)
            .unwrap_or_else(|| panic!("{side:?} 应有入场动画"));
        assert_eq!(
            (enter.from_x, enter.from_y),
            want,
            "{side:?} 入场起跑偏移应为 {want:?}"
        );
        // 终值一律原位、不透明；时长 / 曲线取 UWP 一手源（不得在这里另立数值）。
        assert_eq!((enter.to_x, enter.to_y, enter.to_opacity), (0.0, 0.0, 1.0));
        assert_eq!(enter.duration_ms, 300);
        assert_eq!(enter.curve, TransitionParams::POP_IN.curve);
        h.tree.close_popup(p);
        h.tree.on_popup_motion_done(p);
    }

    // 点锚定（右键菜单）：无方位可言，缺省自上向下。
    let p = open(&mut h, a, PopupSide::Bottom, Some(Point::new(120.0, 140.0)));
    let enter = h.tree.popup_motion(p).and_then(|m| m.enter).unwrap();
    assert_eq!((enter.from_x, enter.from_y), (0.0, -d), "右键菜单自上向下");
}

/// 居中弹层（对话框）没有方位：只淡入淡出，不做方向性位移。
#[test]
fn centered_popup_only_fades() {
    let mut h = detached();
    let root = h.root();
    let p = open(&mut h, root, PopupSide::Center, None);
    let enter = h.tree.popup_motion(p).and_then(|m| m.enter).unwrap();
    assert_eq!((enter.from_x, enter.from_y), (0.0, 0.0), "居中弹层不位移");
    assert_eq!(enter.from_opacity, 0.0, "但仍要淡入");
}

/// 退场：节点留到 Done、期间 `popups()` 仍列出它（外壳还要画）、但不接输入、Done 才移除。
#[test]
fn close_defers_removal_until_done_and_ignores_input() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    let inner = h.tree.children(p)[0];
    let center = h.rect(inner).center();

    h.tree.close_popup(p);
    let plan = h.tree.popup_motion(p).expect("退场计划");
    assert!(plan.closing(), "关闭后进入退场");
    let exit = plan.exit.expect("应有退场动画");
    assert_eq!(exit.duration_ms, 150, "退场 150 ms（UWP 一手源）");
    assert_eq!((exit.to_x, exit.to_y, exit.to_opacity), (0.0, -24.0, 0.0));
    assert!(plan.enter.is_none(), "退场中不再有入场计划");

    // 结束后才移除：几帧之内仍在 `popups()` 里，`contains` 也为真。
    for _ in 0..3 {
        h.frame();
        assert!(
            h.tree.popups().any(|x| x == p),
            "退场未结束 → 弹层仍在 popups()（外壳还要画）"
        );
        assert!(h.tree.contains(p));
    }

    // 期间不接输入：命中跳过退场弹层，点击落不到它的子树。
    assert_eq!(h.tree.hit(center), None, "退场中的弹层不参与命中");
    assert!(!h.tree.contains(inner) || h.tree.hit(center) != Some(inner));

    h.tree.on_popup_motion_done(p);
    assert!(!h.tree.popups().any(|x| x == p), "Done 后从 popups() 移除");
    assert!(!h.tree.contains(p));
}

/// 退场中重开同一弹层：新弹层从**当前呈现值**接续（库已支持打断接续），不跳回起点。
#[test]
fn reopen_during_exit_continues_from_presented_value() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p1 = open(&mut h, a, PopupSide::Bottom, None);
    h.tree.close_popup(p1);
    let exit = h.tree.popup_motion(p1).unwrap().exit.unwrap();

    // 走到退场的中段（动画 150 ms；退场计时是墙钟，故这里真等约 70 ms）。
    std::thread::sleep(std::time::Duration::from_millis(70));
    let plan = h.tree.popup_motion(p1).expect("退场中");
    let expect = visual_at_ms(&exit, plan.exit_ms);
    assert!(
        plan.exit_ms > 40.0 && plan.exit_ms < 140.0,
        "应落在退场中段（实得 {:.1} ms）",
        plan.exit_ms
    );

    let p2 = open(&mut h, a, PopupSide::Bottom, None);
    let enter = h.tree.popup_motion(p2).and_then(|m| m.enter).unwrap();
    assert!(
        (enter.from_y - expect.0.y).abs() < 0.5,
        "接续起点 {:?} 应等于退场当前呈现值 {:?}",
        (enter.from_x, enter.from_y),
        expect
    );
    assert!(
        (enter.from_opacity - expect.1).abs() < 0.02,
        "接续不透明度 {} 应等于 {}",
        enter.from_opacity,
        expect.1
    );
    assert!(
        enter.from_y > -24.0 + 0.5,
        "不得从 PopIn 起点重播（实得 from_y={}）",
        enter.from_y
    );
    assert!(!h.tree.contains(p1), "被接续的旧弹层立即摘掉，不留两层叠着淡出");
}

/// `motion: None`：没有任何表面动画，关闭即移除（保留旧行为）。
#[test]
fn motion_none_has_no_animation_and_closes_immediately() {
    let mut h = detached();
    let a = anchor(&mut h);
    let spec = PopupSpec {
        anchor: Some(a),
        motion: PopupMotion::None,
        ..PopupSpec::default()
    };
    let p = h.tree.open_popup(panel(), spec);
    h.frame();
    assert!(h.tree.popup_motion(p).is_none(), "不动画的弹层没有计划");
    h.tree.close_popup(p);
    assert!(!h.tree.contains(p), "立即移除");
    assert!(!h.tree.popups().any(|x| x == p));
}

/// 工具提示缺省不动画：提示是即时反馈，不能让人等 300 ms 才看到。
#[test]
fn tooltip_defaults_to_no_motion() {
    let mut h = detached();
    let a = anchor(&mut h);
    h.tree.set_tooltip(a, "提示文字");
    // 指针移到锚点上并等满提示延迟（tests 用 idle 推进定时器）。
    h.tree.pointer_move(h.rect(a).center());
    h.idle(1.0);
    let tip = h.tree.tooltip_popup().expect("延迟满应显示提示");
    let spec = h.tree.popup_spec(tip).expect("提示是弹层");
    assert_eq!(spec.motion, PopupMotion::None, "提示缺省不动画");
    assert!(spec.passthrough);
    assert!(h.tree.popup_motion(tip).is_none());
}

/// Done 丢失（合成器不支持 / 事件丢了）时兜底移除：弹层不能永久挡在界面上。
#[test]
fn missing_done_falls_back_to_timeout() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    h.tree.close_popup(p);
    // 退场计时用墙钟：150 ms 动画 + 1000 ms 余量 = 1150 ms。
    std::thread::sleep(std::time::Duration::from_millis(200));
    h.frame();
    assert!(
        h.tree.popups().any(|x| x == p),
        "余量未到不得提前移除（动画可能仍在合成器里跑）"
    );
    std::thread::sleep(std::time::Duration::from_millis(1100));
    h.frame();
    assert!(
        !h.tree.popups().any(|x| x == p),
        "超过动画时长 + 余量后必须移除"
    );
}

/// 弹层表面动画**不要求外壳逐帧出帧**：动画在合成器里跑，客户端空转只会白烧 CPU
/// （无头实测：主表面无损伤时拿不到 frame callback，逐帧出帧会变成 250 Hz 自旋）。
/// 兜底收尾靠定时器（`next_timer` 给出截止时刻，外壳到点唤醒）。
#[test]
fn popup_motion_never_spins_the_client() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    assert!(!h.tree.needs_frame(), "入场在合成器里跑，客户端不必出帧");
    assert!(
        h.tree.next_timer().is_some(),
        "兜底截止时刻要交给外壳（空闲唤醒），否则 Done 丢了会永远挂着"
    );

    // 入场 Done（外壳回报）：计划摘掉，弹层照常显示。
    h.tree.on_popup_motion_done(p);
    assert!(h.tree.popup_motion(p).is_none(), "入场结束即摘计划");
    assert!(h.tree.contains(p), "弹层本身保留");

    // 入场 Done 丢失（合成器不支持动画 / 事件丢了）：超时兜底摘计划。
    let q = open(&mut h, a, PopupSide::Top, None);
    // 300 ms 动画 + 1000 ms 余量 = 1300 ms（墙钟）。
    std::thread::sleep(std::time::Duration::from_millis(1400));
    h.frame();
    assert!(
        h.tree.popup_motion(q).is_none(),
        "入场兜底超时后摘掉计划（实得仍在）"
    );
    assert!(h.tree.contains(q), "兜底只摘计划，不拆弹层");

    // 退场同理：期间不出帧，到点收尾。
    h.tree.close_popup(q);
    assert!(!h.tree.needs_frame(), "退场也在合成器里跑");
    assert!(h.tree.next_timer().is_some() || !h.tree.popups().any(|x| x == q));
}

/// 非分离模式（弹层画在主表面覆盖层里，没有自己的表面）不开表面动画 —— 否则会白等 150 ms
/// 且期间点不到弹层。既有应用（窗口内的对话框 / 菜单）因此零行为变化。
#[test]
fn attached_popups_keep_immediate_close() {
    let mut h = TestHarness::new(300.0, 300.0);
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    assert!(h.tree.popup_motion(p).is_none(), "非分离模式没有表面可动");
    h.tree.close_popup(p);
    assert!(!h.tree.contains(p), "照旧立即移除");
}

/// 帧节拍日志名带锚点角色（`弹层:<锚点控件类型>`），供真机验收按名字取记录。
#[test]
fn motion_name_carries_anchor_role() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    let name = h.tree.popup_motion_name(p).expect("有名字");
    assert!(name.starts_with("弹层:"), "前缀固定：{name}");
    assert!(name.len() > "弹层:".len(), "角色段不得为空：{name}");
    assert!(!name.contains("::"), "只取类型名末段：{name}");
}

/// 早期退场（打开后立刻关闭）也要真的消失：`popups()` 变空、无残留动画状态。
#[test]
fn close_before_first_frame_still_settles() {
    let mut h = detached();
    let a = anchor(&mut h);
    let p = open(&mut h, a, PopupSide::Bottom, None);
    h.tree.close_popup(p);
    h.tree.close_popup(p); // 二次关闭：不得重复发 PopupClosed / 重置退场计时
    let first = h.tree.popup_motion(p).unwrap().exit_ms;
    h.frame();
    assert!(h.tree.popup_motion(p).is_some(), "退场仍在进行");
    assert!(h.tree.popup_motion(p).unwrap().exit_ms > first);
    h.tree.on_popup_motion_done(p);
    assert_eq!(
        h.tree.popups().count(),
        0,
        "全部弹层已结算（无残留退场状态）"
    );
}
