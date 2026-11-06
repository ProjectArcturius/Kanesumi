// 事件回调里挂子树（`EventCtx::insert_child` / `insert_child_with`）的契约测试。
//
// 为什么需要这层 API：打开弹层后往里放内容（对话框塞输入框）是常态，而此前 `EventCtx`
// 只能 `open_popup` / `edit`，**不能插子节点** —— 应用只能先开、再回到外壳层拿 `parts_mut()`
// 去插，把树的可变引用暴露给了应用代码。本测试钉住：
//   · 事件回调里插的子节点确实挂在目标节点下；
//   · 布局属性生效（`insert_child_with` 的 props 不被忽略）；
//   · 插入的内容参与量测（父节点尺寸随之变化）。

use kanesumi_core::{Point, Rect, Size};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::{Border, Stack};
use kanesumi_element::{
    Event, EventCtx, LayoutProps, MeasureCtx, PaintCtx, PopupSpec, Widget, WidgetId,
};

/// 点一下就开弹层并在里面挂内容的触发器。
struct Opener;

impl Widget for Opener {
    fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
        Size::new(80.0, 32.0)
    }
    fn paint(&mut self, _: &mut PaintCtx, _: &mut kanesumi_canvas::Scene) {}
    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            let popup = ctx.open_popup(
                Border::new().background(kanesumi_core::Color::rgb(0.2, 0.2, 0.2)),
                PopupSpec::default(),
            );
            let col = ctx.insert_child_with(
                popup,
                Stack::column().with_spacing(4.0),
                LayoutProps::default(),
            );
            ctx.insert_child_with(
                col,
                Border::new().background(kanesumi_core::Color::rgb(0.5, 0.5, 0.5)),
                LayoutProps {
                    width: Some(120.0),
                    height: Some(40.0),
                    ..LayoutProps::default()
                },
            );
            ctx.set_handled();
        }
    }
    fn focusable(&self) -> bool {
        true
    }
}

fn harness() -> (TestHarness, WidgetId) {
    let mut h = TestHarness::new(400.0, 300.0);
    let opener = h.tree.insert(h.root(), Opener);
    h.frame();
    (h, opener)
}

#[test]
fn event_callback_can_insert_children_into_the_popup() {
    let (mut h, opener) = harness();
    h.click(opener);
    h.frame();

    let popup = h.tree.popups().next().expect("弹层已打开");
    let kids = h.tree.children(popup).to_vec();
    assert_eq!(kids.len(), 1, "事件回调里插的子节点挂在弹层下");
    let col = kids[0];
    assert_eq!(h.tree.children(col).len(), 1, "孙节点也插上了");

    let inner = h.tree.children(col)[0];
    let r = h.rect(inner);
    assert_eq!(
        r.size,
        Size::new(120.0, 40.0),
        "`insert_child_with` 的布局属性生效（不是默认铺满）"
    );
}

#[test]
fn inserted_content_participates_in_layout() {
    let (mut h, opener) = harness();
    h.click(opener);
    h.frame();

    let popup = h.tree.popups().next().expect("弹层已打开");
    let popup_rect: Rect = h.rect(popup);
    assert!(
        popup_rect.size.width >= 120.0 && popup_rect.size.height >= 40.0,
        "弹层尺寸按插入的内容量测：{popup_rect:?}"
    );

    // 内容在弹层内（不越界）。
    let col = h.tree.children(popup)[0];
    let inner = h.tree.children(col)[0];
    let ir = h.rect(inner);
    assert!(
        ir.origin.x >= popup_rect.origin.x - 0.5
            && ir.origin.y >= popup_rect.origin.y - 0.5
            && ir.right() <= popup_rect.right() + 0.5
            && ir.bottom() <= popup_rect.bottom() + 0.5,
        "插入的内容在弹层内：{ir:?} ⊄ {popup_rect:?}"
    );
    let _ = Point::new(0.0, 0.0);
}
