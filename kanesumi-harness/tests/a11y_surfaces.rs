#![cfg(feature = "a11y")]
// 本测试必须覆盖外壳仍在调用的 App 兼容钩子，弃用不表示该发布路径可以漏验。
#![allow(deprecated)]
// 非主表面也必须发布本帧的真实树；多输出锁屏只发布主输出。
use kanesumi_controls::MetroButton;
use kanesumi_core::Size;
use kanesumi_harness::element::{Action, Tree, WidgetId};
use kanesumi_harness::{App, AppConfig, EtherRole, TreeApp, TreeHost};

struct Surfaces {
    config: AppConfig,
    button: Option<WidgetId>,
    popup: (f32, f32),
}
impl TreeApp for Surfaces {
    fn config(&self) -> &AppConfig {
        &self.config
    }
    fn build(&mut self, tree: &mut Tree) {
        self.button = Some(tree.insert(tree.root(), MetroButton::new("确定")));
    }
    fn on_action(&mut self, _: &mut Tree, _: WidgetId, _: Action) {}
    fn ime_engine_host(&self) -> bool {
        true
    }
    fn ime_engine_popup_size(&self) -> (f32, f32) {
        self.popup
    }
    fn lock_output(&mut self, tree: &mut Tree, _: Size, primary: bool) {
        tree.update_props(self.button.unwrap(), |p| p.visible = primary);
    }
}
fn host() -> TreeHost<Surfaces> {
    TreeHost::new(Surfaces {
        config: AppConfig::new(
            "org.kanesumi.a11y.surfaces",
            "surfaces",
            EtherRole::Browser,
            200.0,
            80.0,
        ),
        button: None,
        popup: (200.0, 80.0),
    })
}
fn names(host: &TreeHost<Surfaces>) -> Vec<String> {
    host.a11y_update()
        .expect("表面出帧必须发布")
        .nodes
        .iter()
        .filter_map(|(_, node)| node.label().map(str::to_owned))
        .collect()
}
#[test]
fn ime_popup_publishes_current_tree() {
    let mut host = host();
    host.enable_a11y();
    host.ime_engine_popup_scene(&kanesumi_harness::element::testing::test_engine());
    let update = host.a11y_update().expect("候选窗首帧必须发布");
    let button = update
        .nodes
        .iter()
        .find(|(_, n)| n.label() == Some("确定"))
        .unwrap();
    let bounds = button.1.bounds().unwrap();
    assert!(bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0);
}
#[test]
fn empty_ime_popup_clears_previous_content() {
    let mut host = host();
    host.enable_a11y();
    let engine = kanesumi_harness::element::testing::test_engine();
    host.ime_engine_popup_scene(&engine);
    assert!(names(&host).iter().any(|name| name == "确定"));
    let (app, tree) = host.parts_mut();
    tree.remove(app.button.take().unwrap());
    app.popup = (0.0, 0.0);
    assert!(host.ime_engine_popup_scene(&engine).is_empty());
    assert!(!names(&host).iter().any(|name| name == "确定"));
}
#[test]
fn lock_primary_publishes_and_secondary_preserves_primary() {
    let mut host = host();
    host.enable_a11y();
    let engine = kanesumi_harness::element::testing::test_engine();
    host.render_lock(&engine, Size::new(200.0, 80.0), true);
    let before = names(&host);
    assert!(before.iter().any(|name| name == "确定"));
    host.render_lock(&engine, Size::new(400.0, 200.0), false);
    assert_eq!(names(&host), before, "副屏背板不能覆盖主屏控件快照");
}
#[test]
fn surface_a11y_remains_off_without_optin() {
    let mut host = host();
    let engine = kanesumi_harness::element::testing::test_engine();
    host.ime_engine_popup_scene(&engine);
    host.render_lock(&engine, Size::new(200.0, 80.0), true);
    assert!(!host.a11y_enabled());
    assert!(host.a11y_update().is_none());
}

#[test]
fn platform_hidden_notification_clears_and_reopen_restores_candidates() {
    let mut host = host();
    host.enable_a11y();
    let engine = kanesumi_harness::element::testing::test_engine();
    host.ime_engine_popup_scene(&engine);
    assert!(names(&host).iter().any(|name| name == "确定"));
    host.ime_engine_popup_hidden();
    let hidden = host.a11y_update().unwrap();
    assert_eq!(hidden.nodes.len(), 1, "实际表面释放后只保留根");
    assert_eq!(hidden.focus.0, host.tree().root().to_u64());
    assert!(hidden.nodes[0].1.children().is_empty());
    host.ime_engine_popup_scene(&engine);
    assert!(
        names(&host).iter().any(|name| name == "确定"),
        "重新显示恢复真实候选"
    );
}
