// 元素树图层（G3-c）契约测试。参 layer.rs / Ether docs/GPU_COMPOSITION_PLAN.md G3。

use kanesumi_canvas::SceneCommand;
use kanesumi_core::{Color, Rect};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Border;
use kanesumi_element::{Align, LayerAnimSpec, LayerOp, LayoutProps};

fn fixed(x: f32, y: f32, w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        margin: kanesumi_element::Insets { left: x, top: y, right: 0.0, bottom: 0.0 },
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

const RED: Color = Color::rgb(1.0, 0.0, 0.0);
const BLUE: Color = Color::rgb(0.0, 0.0, 1.0);

fn fills(cmds: &[SceneCommand]) -> Vec<(Color, Rect)> {
    cmds.iter()
        .filter_map(|c| match c {
            SceneCommand::FillRect { color, rect, .. } => Some((*color, *rect)),
            _ => None,
        })
        .collect()
}

#[test]
fn layer_subtree_leaves_main_scene_and_is_committed_once() {
    let mut h = TestHarness::new(400.0, 300.0);
    let tile = h.tree.insert_with(h.root(), Border::new().background(RED), fixed(50.0, 60.0, 100.0, 80.0));
    h.tree.set_layer(tile, true);
    h.frame();

    // 主场景里不再有红块（交给图层子表面）。
    assert!(
        !fills(&h.last.scene.commands).iter().any(|(c, _)| *c == RED),
        "图层子树不得进入主场景"
    );
    let ops = h.tree.take_layer_ops();
    assert!(matches!(ops[0], LayerOp::Create { id, rect } if id == tile && rect == Rect::new(50.0, 60.0, 100.0, 80.0)));
    // 内容平移到图层自身坐标：红块从 (0,0) 起。
    let LayerOp::Content { id, scene } = &ops[1] else { panic!("应有 Content：{ops:?}") };
    assert_eq!(*id, tile);
    assert_eq!(fills(&scene.commands), vec![(RED, Rect::new(0.0, 0.0, 100.0, 80.0))]);

    // 内容未变的后续帧：不再提交。
    h.tree.invalidate_paint(h.root());
    h.frame();
    assert!(h.tree.take_layer_ops().is_empty(), "内容不变不得重复提交");
}

#[test]
fn content_change_resubmits_and_unmark_removes() {
    let mut h = TestHarness::new(400.0, 300.0);
    let tile = h.tree.insert_with(h.root(), Border::new().background(RED), fixed(0.0, 0.0, 40.0, 40.0));
    h.tree.set_layer(tile, true);
    h.frame();
    h.tree.take_layer_ops();

    h.tree.edit::<Border, _>(tile, |b, ctx| {
        *b = std::mem::take(b).background(BLUE);
        ctx.invalidate_paint();
    });
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(
        ops.iter().any(|o| matches!(o, LayerOp::Content { scene, .. } if fills(&scene.commands)[0].0 == BLUE)),
        "内容变了应重新提交：{ops:?}"
    );

    h.tree.set_layer(tile, false);
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(matches!(ops[0], LayerOp::Remove { id } if id == tile));
    // 取消图层后回到主场景。
    assert!(fills(&h.last.scene.commands).iter().any(|(c, _)| *c == BLUE));
}

#[test]
fn animate_is_forwarded_only_for_layers() {
    let mut h = TestHarness::new(200.0, 200.0);
    let a = h.tree.insert_with(h.root(), Border::new().background(RED), fixed(0.0, 0.0, 10.0, 10.0));
    let b = h.tree.insert_with(h.root(), Border::new().background(BLUE), fixed(20.0, 0.0, 10.0, 10.0));
    h.tree.set_layer(a, true);
    h.frame();
    h.tree.take_layer_ops();
    let spec = LayerAnimSpec {
        serial: 1,
        duration_ms: 350,
        delay_ms: 0,
        curve: [0.1, 0.9, 0.2, 1.0],
        to_x: 0.0,
        to_y: 0.0,
        to_opacity: 1.0,
    };
    h.tree.animate_layer(a, spec);
    h.tree.animate_layer(b, spec);
    let ops = h.tree.take_layer_ops();
    assert_eq!(ops.len(), 1, "非图层节点的动画请求应被忽略");
    assert!(matches!(ops[0], LayerOp::Animate { id, .. } if id == a));
}

#[test]
fn removed_node_drops_its_layer() {
    let mut h = TestHarness::new(200.0, 200.0);
    let a = h.tree.insert_with(h.root(), Border::new().background(RED), fixed(0.0, 0.0, 10.0, 10.0));
    h.tree.set_layer(a, true);
    h.frame();
    h.tree.take_layer_ops();
    h.tree.remove(a);
    h.frame();
    let ops = h.tree.take_layer_ops();
    assert!(matches!(ops[..], [LayerOp::Remove { id }] if id == a), "{ops:?}");
}
