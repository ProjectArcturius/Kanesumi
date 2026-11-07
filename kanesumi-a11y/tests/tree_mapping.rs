// tree_mapping —— 快照 → AccessKit 树的映射验收。
//
// 这一层的价值在于「可在任意平台验」：映射是纯函数，所以角色 / 名字 / 值 / 三态 /
// 子节点 / 合成项 / 焦点全部能在 Windows 上钉死，Linux 侧只剩「把这份树接上 AT-SPI」。
// 参 kanesumi-a11y/src/lib.rs 头注、docs/ELEMENT_TREE.md §Ⅸ。

use accesskit::{NodeId, Role, Toggled};
use kanesumi_a11y::{node_id, update, update_for_tree};
use kanesumi_core::{Rect, Size};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::{AccessInfo, AccessItem, AccessRole, Widget};
use kanesumi_element::widgets::{Border, Label};

/// 可聚焦夹具：一个按钮形状的控件，读屏应看到 `Role::Button` + 名字。
struct FixtureButton {
    name: &'static str,
    focused: bool,
}

impl Widget for FixtureButton {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: Size,
    ) -> Size {
        Size::new(120.0, 32.0)
    }
    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut kanesumi_canvas::Scene) {
        scene.fill_rect(
            ctx.theme().colors.control_fill,
            ctx.rect(),
        );
    }
    fn focusable(&self) -> bool {
        true
    }
    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::Button,
            name: self.name.to_string(),
            value: self.focused.then(|| "当前".to_string()),
            checked: None,
        })
    }
}

fn by_label<'a>(
    update: &'a accesskit::TreeUpdate,
    label: &str,
) -> Option<&'a (NodeId, accesskit::Node)> {
    update.nodes.iter().find(|(_, n)| n.label() == Some(label))
}

#[test]
fn real_tree_maps_roles_names_and_structure() {
    let mut h = TestHarness::new(320.0, 240.0);
    let col = h.tree.insert(
        h.root(),
        Border::new().background(kanesumi_core::ThemeColor::Background),
    );
    let button = h.tree.insert(col, FixtureButton { name: "确定", focused: false });
    h.tree.insert(col, Label::new("说明文字"));
    h.frame();

    let update = update_for_tree(&h.tree);
    let tree = update.tree.as_ref().expect("首帧必须带树信息");
    assert_eq!(tree.toolkit_name.as_deref(), Some("Kanesumi"));
    assert_eq!(tree.root, node_id(h.root()), "树根就是元素树根");

    let (_, button_node) = update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id(button))
        .expect("按钮节点在更新里");
    assert_eq!(button_node.role(), Role::Button);
    assert_eq!(button_node.label(), Some("确定"));
    let bounds = button_node.bounds().expect("必须给矩形（读屏靠它画高亮框）");
    assert!(bounds.x1 > bounds.x0 && bounds.y1 > bounds.y0, "矩形非退化：{bounds:?}");

    assert!(by_label(&update, "说明文字").is_some_and(|(_, n)| n.role() == Role::Label));
    // 没有焦点的树：AccessKit 要求焦点字段恒有值，无焦点时落到根。
    assert_eq!(update.focus, node_id(h.root()), "无焦点时焦点落根");
}

#[test]
fn focus_follows_the_tree_and_falls_back_when_stale() {
    let mut h = TestHarness::new(320.0, 240.0);
    let a = h.tree.insert(h.root(), FixtureButton { name: "甲", focused: true });
    let b = h.tree.insert(h.root(), FixtureButton { name: "乙", focused: false });
    h.frame();
    h.tree.focus(a, true);
    h.frame();
    assert_eq!(update_for_tree(&h.tree).focus, node_id(a), "焦点跟着框架焦点走");

    // 抓一个「已经不在树里」的 id：删掉 b 再把它当焦点传进去。
    let stale = b;
    h.tree.remove(b);
    h.frame();
    assert_eq!(
        update_for_tree(&h.tree).focus,
        node_id(a),
        "陈旧焦点 id 不得写进更新（AccessKit 会 panic）"
    );
    let snapshot = h.tree.access_snapshot();
    assert_eq!(
        update(&snapshot, Some(stale)).focus,
        node_id(h.root()),
        "快照里没有的焦点 id 退回根"
    );
}

/// AccessKit 的硬约束：更新里除根以外的每个节点，必须是**某一个**节点 `children` 里的 id。
/// 违反会被适配器当损坏树拒收 —— 这里按结构遍历一次，防止解析式展平写漏分支。
#[test]
fn every_node_is_child_of_exactly_one_parent() {
    let mut h = TestHarness::new(320.0, 240.0);
    let col = h.tree.insert(h.root(), Border::new());
    for i in 0..4 {
        let row = h.tree.insert(col, Border::new());
        h.tree.insert(
            row,
            Label::new(format!("第 {i} 行")),
        );
    }
    h.frame();

    let update = update_for_tree(&h.tree);
    let mut parents: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
    for (_, node) in &update.nodes {
        for child in node.children() {
            *parents.entry(*child).or_default() += 1;
        }
    }
    let root = update.tree.as_ref().unwrap().root;
    let known: std::collections::HashSet<NodeId> =
        update.nodes.iter().map(|(id, _)| *id).collect();
    for id in &known {
        if *id == root {
            assert!(
                !parents.contains_key(id),
                "根节点不得是别人的子节点"
            );
        } else {
            assert_eq!(
                parents.get(id).copied(),
                Some(1),
                "节点 {id:?} 必须恰好被一个父节点引用"
            );
        }
    }
    for (child, count) in &parents {
        assert!(known.contains(child), "引用了不存在的子节点 {child:?}（出现 {count} 次）");
    }
    assert_eq!(known.len(), update.nodes.len(), "id 不得重复");
}

#[test]
fn synthesized_items_become_real_child_nodes() {
    /// 面板夹具：把两行报成合成项（菜单 / 列表这类控件的最小形状）。
    struct Panel;
    impl Widget for Panel {
        fn measure(
            &mut self,
            _ctx: &mut kanesumi_element::MeasureCtx,
            _available: Size,
        ) -> Size {
            Size::new(200.0, 64.0)
        }
        fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut kanesumi_canvas::Scene) {}
        fn accessibility(&self) -> Option<AccessInfo> {
            Some(AccessInfo {
                role: AccessRole::Menu,
                name: "文件".to_string(),
                value: None,
                checked: None,
            })
        }
        fn access_items(
            &self,
            rect: Rect,
            _theme: &kanesumi_core::MetroTheme,
        ) -> Vec<AccessItem> {
            let mut rows = Vec::new();
            for (i, name) in ["新建", "打开"].into_iter().enumerate() {
                let mut item = AccessItem::new(
                    AccessRole::MenuItem,
                    name,
                    Rect::new(rect.origin.x, rect.origin.y + i as f32 * 32.0, rect.size.width, 32.0),
                );
                if i == 1 {
                    item.checked = Some(true);
                }
                rows.push(item);
            }
            rows
        }
    }

    let mut h = TestHarness::new(320.0, 240.0);
    let panel = h.tree.insert(h.root(), Panel);
    h.frame();

    let update = update_for_tree(&h.tree);
    let (_, panel_node) = update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id(panel))
        .expect("面板节点");
    assert_eq!(panel_node.children().len(), 2, "两条合成项成为子节点");

    let item_ids: Vec<NodeId> = panel_node.children().to_vec();
    for id in &item_ids {
        assert!(
            !update.nodes.iter().any(|(wid, _)| *wid != *id && wid.0 == id.0),
            "合成项 id 唯一"
        );
    }
    let item = update
        .nodes
        .iter()
        .find(|(id, _)| *id == item_ids[1])
        .map(|(_, n)| n)
        .expect("第二条合成项");
    assert_eq!(item.role(), Role::MenuItem);
    assert_eq!(item.label(), Some("打开"));
    assert_eq!(item.toggled(), Some(Toggled::True), "勾选态要能读到");
    // 合成项 id 不占用框架节点 id 空间（`WidgetId` 编码远小于 u64::MAX）。
    assert!(item_ids.iter().all(|id| id.0 > u32::MAX as u64), "合成项 id 在高位段");

    // 结构约束照旧：合成项也必须是合法子节点。
    let known: Vec<NodeId> = update.nodes.iter().map(|(id, _)| *id).collect();
    assert!(known.contains(&item_ids[0]) && known.contains(&item_ids[1]));
}

#[test]
fn tri_state_and_disabled_round_trip() {
    struct TreeRowFixture;
    impl Widget for TreeRowFixture {
        fn measure(
            &mut self,
            _ctx: &mut kanesumi_element::MeasureCtx,
            _available: Size,
        ) -> Size {
            Size::new(160.0, 24.0)
        }
        fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut kanesumi_canvas::Scene) {}
        fn focusable(&self) -> bool {
            false
        }
        fn accessibility(&self) -> Option<AccessInfo> {
            Some(AccessInfo {
                role: AccessRole::TreeItem,
                name: "文档".to_string(),
                value: Some("3 项".to_string()),
                checked: Some(true),
            })
        }
    }

    let mut h = TestHarness::new(320.0, 240.0);
    let row = h.tree.insert(h.root(), TreeRowFixture);
    h.frame();
    let update = update_for_tree(&h.tree);
    let (_, node) = update
        .nodes
        .iter()
        .find(|(id, _)| *id == node_id(row))
        .expect("树行节点");
    assert_eq!(node.role(), Role::TreeItem);
    assert_eq!(node.label(), Some("文档"));
    assert_eq!(node.value(), Some("3 项"));
    assert_eq!(node.toggled(), Some(Toggled::True));
    assert_eq!(
        node.is_selected(),
        None,
        "不适用必须是 None（读成「未选中」是常见噪音）"
    );
    assert_eq!(node.is_expanded(), None, "不适用必须是 None");
    assert!(!node.is_disabled());
}

/// 未实现的动作**不要声明**：本轮是只读桥。声明了 `Action::Click` 而没有回灌通路，
/// 读屏会显示可按但按了没反应的按钮 —— 比不声明更糟。参 src/lib.rs 头注。
#[test]
fn no_actions_are_advertised_yet() {
    let mut h = TestHarness::new(320.0, 240.0);
    h.tree.insert(h.root(), FixtureButton { name: "保存", focused: false });
    h.frame();
    let update = update_for_tree(&h.tree);
    for (id, node) in &update.nodes {
        assert!(
            !node.supports_action(accesskit::Action::Click)
                && !node.supports_action(accesskit::Action::Focus),
            "本轮不声明动作（节点 {id:?}）"
        );
    }
}

#[test]
fn node_id_matches_framework_encoding() {
    // 命中与将来的动作回灌都要靠 id 对接，先把这层编码钉住。
    let mut h = TestHarness::new(100.0, 100.0);
    let id = h.tree.insert(h.root(), Label::new("x"));
    h.frame();
    assert_eq!(node_id(id).0, id.to_u64());
    assert!(
        update_for_tree(&h.tree)
            .nodes
            .iter()
            .any(|(nid, _)| *nid == node_id(id)),
        "框架 id 直接就是无障碍节点 id"
    );
}
#[test]
fn snapshot_slot_hands_the_latest_tree_across_threads() {
    use kanesumi_a11y::SnapshotSlot;

    let slot = SnapshotSlot::new();
    assert!(!slot.is_ready(), "首帧之前不该有树");
    assert!(slot.current().is_none());

    let mut h = TestHarness::new(160.0, 120.0);
    h.tree.insert(h.root(), Label::new("跨线程"));
    h.frame();
    let update = update_for_tree(&h.tree);
    let node_count = update.nodes.len();
    slot.publish(update);
    assert!(slot.is_ready());

    // 适配器线程就是这么用的：克隆槽位到别的线程再取。
    let worker = slot.clone();
    let seen = std::thread::spawn(move || worker.current().map(|u| u.nodes.len()))
        .join()
        .expect("线程不 panic");
    assert_eq!(seen, Some(node_count), "别的线程要能看到最近一帧");
    assert_eq!(slot.current().map(|u| u.nodes.len()), Some(node_count));
}
