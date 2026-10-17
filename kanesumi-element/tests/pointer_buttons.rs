// 扩展按钮走原生事件冒泡，不得触发左键点击。参 event.rs PointerButton。
use kanesumi_canvas::Scene;
use kanesumi_core::{Point, Size};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::{Event, EventCtx, MeasureCtx, Modifiers, PaintCtx, PointerButton, Widget};

struct ButtonProbe;

#[derive(Debug, PartialEq)]
enum Observed {
    Released(PointerButton),
    Click,
    Context,
}

impl Widget for ButtonProbe {
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        available
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    fn has_visual_states(&self) -> bool {
        true
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerUp { button, .. } => ctx.emit(Observed::Released(*button)),
            Event::Click => ctx.emit(Observed::Click),
            Event::ContextRequested { .. } => ctx.emit(Observed::Context),
            _ => {}
        }
    }
}

#[test]
fn auxiliary_buttons_keep_identity_without_activating_controls() {
    let mut h = TestHarness::new(100.0, 100.0);
    h.tree.insert(h.root(), ButtonProbe);
    h.frame();
    let pos = Point::new(50.0, 50.0);
    for code in [0x113, 0x114, 0x115, 0x116, 0x117, 0xffff] {
        let button = PointerButton::from_evdev(code);
        h.tree.pointer_down(pos, button, Modifiers::NONE);
        h.tree.pointer_up(pos, button, Modifiers::NONE);
        let events: Vec<_> = h.take::<Observed>().into_iter().map(|(_, event)| event).collect();
        assert_eq!(events, [Observed::Released(button)]);
    }
    h.tree.pointer_down(pos, PointerButton::Left, Modifiers::NONE);
    h.tree.pointer_up(pos, PointerButton::Left, Modifiers::NONE);
    let events: Vec<_> = h.take::<Observed>().into_iter().map(|(_, event)| event).collect();
    assert_eq!(events, [Observed::Released(PointerButton::Left), Observed::Click]);
}
