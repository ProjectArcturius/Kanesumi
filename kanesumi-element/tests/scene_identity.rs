// 验证身份与绘制同源，不按全场景索引或可变内容猜身份。
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Size};
use kanesumi_element::testing::test_engine;
use kanesumi_element::widgets::Stack;
use kanesumi_element::{
    LayoutProps, MeasureCtx, PaintCtx, SceneCommandKey, Tree, Widget, WidgetId,
};

struct Block(bool);
impl Widget for Block {
    fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
        Size::new(24.0, 24.0)
    }
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let color = if self.0 {
            ctx.theme().colors.primary_hover
        } else {
            ctx.theme().colors.primary
        };
        scene.fill_rect(color, ctx.rect());
    }
    fn paint_after(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        scene.stroke_rect(ctx.theme().colors.divider, ctx.rect(), 1.0);
    }
    fn focusable(&self) -> bool {
        true
    }
}
fn setup() -> (Tree, WidgetId, WidgetId) {
    let mut tree = Tree::new(MetroTheme::ether_dark());
    let row = tree.insert(tree.root(), Stack::row().with_spacing(8.0));
    let props = LayoutProps {
        width: Some(24.0),
        height: Some(24.0),
        ..LayoutProps::default()
    };
    let a = tree.insert_with(row, Block(false), props.clone());
    let b = tree.insert_with(row, Block(false), props);
    (tree, a, b)
}
fn keyed(tree: &mut Tree) -> (Scene, Vec<SceneCommandKey>) {
    let mut scene = Scene::default();
    tree.frame_keyed_into(&test_engine(), Size::new(128.0, 64.0), 0.016, &mut scene);
    let keys = tree.scene_command_keys().to_vec();
    assert_eq!(scene.commands.len(), keys.len(), "每条命令都必须有身份");
    (scene, keys)
}
fn owned(keys: &[SceneCommandKey], node: WidgetId) -> Vec<SceneCommandKey> {
    keys.iter()
        .copied()
        .filter(|key| key.widget_id() == node)
        .collect()
}

#[test]
fn keyed_and_legacy_scene_commands_are_identical() {
    let (mut tree, _, _) = setup();
    let mut legacy = Scene::default();
    tree.frame_into(&test_engine(), Size::new(128.0, 64.0), 0.016, &mut legacy);
    let (scene, _) = keyed(&mut tree);
    assert_eq!(legacy, scene);
}
#[test]
fn removing_earlier_sibling_does_not_rekey_survivor() {
    let (mut tree, a, b) = setup();
    let (_, before) = keyed(&mut tree);
    assert_eq!(owned(&before, b).len(), 2);
    tree.remove(a);
    let (_, after) = keyed(&mut tree);
    assert_eq!(owned(&before, b), owned(&after, b));
}
#[test]
fn appending_sibling_does_not_rekey_existing_nodes() {
    let (mut tree, a, b) = setup();
    let (_, before) = keyed(&mut tree);
    tree.insert(tree.parent(a).unwrap(), Block(false));
    let (_, after) = keyed(&mut tree);
    assert_eq!(owned(&before, a), owned(&after, a));
    assert_eq!(owned(&before, b), owned(&after, b));
}
#[test]
fn recycled_arena_slot_has_a_new_identity() {
    let (mut tree, a, _) = setup();
    let parent = tree.parent(a).unwrap();
    let (_, before) = keyed(&mut tree);
    tree.remove(a);
    let replacement = tree.insert(parent, Block(false));
    let (_, after) = keyed(&mut tree);
    assert_ne!(a, replacement);
    assert!(
        !owned(&after, replacement)
            .iter()
            .any(|key| before.contains(key))
    );
}
#[test]
fn content_change_preserves_keys() {
    let (mut tree, a, _) = setup();
    let (before, keys_before) = keyed(&mut tree);
    tree.edit::<Block, _>(a, |block, _| block.0 = true);
    let (after, keys_after) = keyed(&mut tree);
    assert_ne!(before, after);
    assert_eq!(keys_before, keys_after);
}
#[test]
fn clips_after_paint_and_keyboard_focus_have_distinct_keys() {
    let (mut tree, a, _) = setup();
    keyed(&mut tree);
    assert!(tree.focus(a, true));
    let (scene, keys) = keyed(&mut tree);
    assert_eq!(
        owned(&keys, a).len(),
        4,
        "paint、after 与两条焦点描边都要导出"
    );
    assert_eq!(
        keys.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        keys.len()
    );
    assert!(
        scene
            .commands
            .iter()
            .any(|c| matches!(c, kanesumi_canvas::SceneCommand::PushClip { .. }))
    );
}

#[test]
fn separate_trees_do_not_alias_their_arena_keys() {
    let (mut first, a, _) = setup();
    let (mut second, b, _) = setup();
    assert_eq!(a, b, "两个 arena 恰好生成相同 WidgetId");
    let (_, first_keys) = keyed(&mut first);
    let (_, second_keys) = keyed(&mut second);
    assert_ne!(owned(&first_keys, a), owned(&second_keys, b));
}
#[test]
fn partial_damage_still_exports_complete_keyed_scene() {
    let (mut tree, a, b) = setup();
    tree.set_damage_cull(true);
    let (_, before) = keyed(&mut tree);
    tree.edit::<Block, _>(a, |block, _| block.0 = true);
    let (_, after) = keyed(&mut tree);
    assert_eq!(owned(&before, b), owned(&after, b));
    assert_eq!(
        owned(&after, b).len(),
        2,
        "未损伤的兄弟也必须留在完整 GPU 列表中"
    );
}
#[test]
fn legacy_recomposition_invalidates_old_identity_table() {
    let (mut tree, a, _) = setup();
    keyed(&mut tree);
    tree.edit::<Block, _>(a, |block, _| block.0 = true);
    let mut scene = Scene::default();
    tree.frame_into(&test_engine(), Size::new(128.0, 64.0), 0.016, &mut scene);
    assert!(tree.scene_command_keys().is_empty());
    let (_, keys) = keyed(&mut tree);
    assert!(!keys.is_empty());
}
#[test]
fn steady_frame_reuses_scene_and_identity_storage() {
    let (mut tree, _, _) = setup();
    let engine = test_engine();
    let mut scene = Scene::default();
    let mut reused = false;
    for _ in 0..8 {
        reused = tree
            .frame_keyed_into(&engine, Size::new(128.0, 64.0), 0.016, &mut scene)
            .reused;
    }
    assert!(reused);
    let before = scene.clone();
    let keys = tree.scene_command_keys().to_vec();
    let pointer = tree.scene_command_keys().as_ptr();
    let frame = tree.frame_keyed_into(&engine, Size::new(128.0, 64.0), 0.016, &mut scene);
    assert!(frame.reused);
    assert_eq!(before, scene);
    assert_eq!(keys, tree.scene_command_keys());
    assert_eq!(pointer, tree.scene_command_keys().as_ptr());
}
#[test]
fn detached_layer_is_not_duplicated_in_main_scene() {
    let (mut tree, a, b) = setup();
    tree.set_layer(a, true);
    let (_, keys) = keyed(&mut tree);
    assert!(owned(&keys, a).is_empty());
    assert_eq!(owned(&keys, b).len(), 2);
}
#[test]
fn popup_scrim_has_an_identity_without_hiding_popup_keys() {
    let (mut tree, _, _) = setup();
    let popup = tree.open_popup(
        Block(false),
        kanesumi_element::PopupSpec {
            scrim: true,
            ..Default::default()
        },
    );
    let (_, keys) = keyed(&mut tree);
    assert_eq!(owned(&keys, popup).len(), 2);
    assert_eq!(
        keys.iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
            .len(),
        keys.len()
    );
}
