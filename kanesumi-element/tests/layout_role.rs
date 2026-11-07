// 布局角色（`LayoutRole` 附着属性）的契约测试。
//
// 为什么要有这层：对话框要同时装「正文（内容槽）」与「按钮行」，两类子节点在同一个父之下，
// 容器只看 `LayoutProps` 无法区分。有了角色，应用把按钮插进对话框即可，对话框不必自己画
// 按钮 —— 那是按钮成为框架节点、能被 Tab 走到的前提（参 docs/ELEMENt_TREE §Ⅳ.2）。
//
// 本测试钉住三件事：
//   1. 角色随子节点存放、经 `child_props` 原样读回（附着属性不是容器旁表）；
//   2. 分区容器确实按角色排到不同矩形（内容区在上、命令区在下）；
//   3. 未声明角色（默认 `Normal`）的子节点不被分区容器误吞进任一区。

use kanesumi_core::Size;
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Border;
use kanesumi_core::Rect;
use kanesumi_element::{
    ArrangeCtx, LayoutProps, LayoutRole, MeasureCtx, PaintCtx, Widget, WidgetId,
};

/// 极简分区容器：上半给 `Content`，下半给 `Commands`；`Normal` 子节点排到左上零尺寸，
/// 好让测试看出「没被误排」。
struct SplitPane;

impl Widget for SplitPane {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let w = available.width.clamp(0.0, 200.0);
        let h = available.height.clamp(0.0, 200.0);
        let each = Size::new(w, h / 2.0);
        for c in ctx.children().to_vec() {
            ctx.measure_child(c, each);
        }
        Size::new(w, h)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let top = Rect::new(rect.origin.x, rect.origin.y, rect.size.width, rect.size.height / 2.0);
        let bottom = Rect::new(
            rect.origin.x,
            rect.origin.y + rect.size.height / 2.0,
            rect.size.width,
            rect.size.height / 2.0,
        );
        for c in ctx.children().to_vec() {
            let role = ctx.child_props(c).role;
            let slot = match role {
                LayoutRole::Content => top,
                LayoutRole::Commands => bottom,
                LayoutRole::Normal => Rect::new(rect.origin.x, rect.origin.y, 0.0, 0.0),
            };
            ctx.arrange_child(c, slot);
        }
    }

    fn paint(&mut self, _: &mut PaintCtx, _: &mut kanesumi_canvas::Scene) {}
}

#[test]
fn role_is_an_attached_property() {
    let mut h = TestHarness::new(300.0, 300.0);
    let pane = h.tree.insert(h.root(), SplitPane);
    let content = h.tree.insert_with(
        pane,
        Border::new(),
        LayoutProps::default().in_role(LayoutRole::Content),
    );
    let commands = h.tree.insert_with(
        pane,
        Border::new(),
        LayoutProps::default().in_role(LayoutRole::Commands),
    );
    h.frame();

    // 角色随节点存放，容器经 child_props 读回 —— 不是容器自己记的旁表。
    assert_eq!(
        h.tree.props(content).unwrap().role,
        LayoutRole::Content,
        "内容角色读回"
    );
    assert_eq!(
        h.tree.props(commands).unwrap().role,
        LayoutRole::Commands,
        "命令角色读回"
    );

    // 分区排布生效：内容在上半、命令在下半。
    let pane_rect = h.rect(pane);
    let c = h.rect(content);
    let m = h.rect(commands);
    assert!((c.origin.y - pane_rect.origin.y).abs() < 0.5, "内容在上半：{c:?}");
    assert!(
        (m.origin.y - (pane_rect.origin.y + pane_rect.size.height / 2.0)).abs() < 0.5,
        "命令在下半：{m:?}"
    );
    assert!(c.size.height > 0.0 && m.size.height > 0.0, "两区都有高度");
}

#[test]
fn normal_role_is_not_captured_by_either_area() {
    let mut h = TestHarness::new(300.0, 300.0);
    let pane: WidgetId = h.tree.insert(h.root(), SplitPane);
    let plain = h.tree.insert_with(pane, Border::new(), LayoutProps::default());
    h.frame();

    assert_eq!(
        h.tree.props(plain).unwrap().role,
        LayoutRole::Normal,
        "默认角色是 Normal"
    );
    let r = h.rect(plain);
    assert_eq!(r.size, Size::ZERO, "Normal 子节点不被分区容器排进任一区：{r:?}");
}
