// MenuFlyout —— 元素树的菜单弹层（XAML MenuFlyout）。参 docs/ELEMENT_TREE.md §Ⅴ.2、
// docs/ELEMENT_MIGRATION.md §8（弹层类控件模板）。
//
// 这是所有「点一下弹出菜单」的控件（DropDownButton / SplitButton / MenuBar / 右键菜单 /
// CommandBarFlyout 的溢出……）共用的弹层本体。它包一个既有的 `MetroDropdownMenu`
// （面板绘制、级联子菜单、单选组逻辑全部复用），只补元素树侧的职责：
//
// - 位置：挂在覆盖层，框架按锚点放置（下方放不下上翻、夹进表面），自身矩形 = 面板；
// - 级联子菜单：画在面板之外，经 `paint_overflow` 声明、`hit_test` 一并命中；
// - 交互：指针悬停 / 点击、键盘 Up/Down/Enter/Space/Right/Left（Esc 由框架关闭弹层）；
// - 结果：选中项发 `MenuInvoked { owner, path, .. }`（owner = 打开它的控件），随即关闭自身；
//   任何原因的关闭都会让 owner 收到 `Event::PopupClosed`（框架保证），焦点交还 owner。

use kanesumi_canvas::Scene;
use kanesumi_core::{Point, Rect, Size};
use kanesumi_element::{
    ArrangeCtx, Event, EventCtx, Insets, MeasureCtx, PaintCtx, PopupSpec, UpdateCtx, Widget,
    WidgetId,
};

use crate::dropdown_menu::{MenuPath, MenuItem, MetroDropdownMenu};

/// 元素树动作：菜单项被选中。来源 id 是弹层自身；按 `owner` 区分是哪个控件的菜单。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuInvoked {
    /// 打开这个菜单的控件。
    pub owner: WidgetId,
    pub path: MenuPath,
    /// 被选中项的标签（便于日志与简单分派）。
    pub label: String,
    /// 选中后该项的勾选状态（单选组项为 true；普通项为其 `checked` 原值）。
    pub checked: bool,
}

pub struct MenuFlyout {
    /// 面板本体即弹层节点；`owner` 记在 `menu.owner`（选中动作与分派用）。
    menu: MetroDropdownMenu,
}

impl MenuFlyout {
    pub fn new(owner: WidgetId, items: Vec<MenuItem>) -> Self {
        let mut menu = MetroDropdownMenu::new(items);
        menu.anim.open();
        // 面板本体承担交互：选中动作带 owner（App 按 owner 分派），由本菜单发。
        menu.owner = Some(owner);
        Self { menu }
    }

    /// 由控件在自己的事件回调里调用：以自身为锚点打开菜单。`keyboard` 为真时把焦点移入菜单
    /// 并预选第一项（键盘打开的菜单必须能直接 Up/Down，不必先动鼠标）。返回弹层 id。
    pub fn open(ctx: &mut EventCtx, items: Vec<MenuItem>, keyboard: bool) -> WidgetId {
        let owner = ctx.id();
        let mut flyout = MenuFlyout::new(owner, items);
        if keyboard && !flyout.menu.items.is_empty() {
            flyout.menu.hovered = Some(0);
        }
        let id = ctx.open_popup(
            flyout,
            PopupSpec {
                anchor: Some(owner),
                // 贴住触发控件：按钮 / 头像菜单下拉不留缝（TopBar 用户菜单曾透出 4pt 壁纸）。
                // 指针锚定的右键菜单走 `open_at`，那里仍是 `PopupSpec::default()` 的 0 缝。
                gap: 0.0,
                ..PopupSpec::default()
            },
        );
        ctx.focus_widget(id, keyboard);
        id
    }

    /// 右键菜单：在指针位置打开（点锚定，右 / 下放不下时翻转）。`owner` 仍是本控件。
    pub fn open_at(ctx: &mut EventCtx, items: Vec<MenuItem>, at: Point, keyboard: bool) -> WidgetId {
        let owner = ctx.id();
        let mut flyout = MenuFlyout::new(owner, items);
        if keyboard && !flyout.menu.items.is_empty() {
            flyout.menu.hovered = Some(0);
        }
        let id = ctx.open_popup(
            flyout,
            PopupSpec {
                anchor: Some(owner),
                at: Some(at),
                ..PopupSpec::default()
            },
        );
        ctx.focus_widget(id, keyboard);
        id
    }

}

/// 全方法委托 [`MetroDropdownMenu` 的 `Widget` 实现](MetroDropdownMenu)。
/// `owner` 已随构造写进面板（`menu.owner`），选中动作由面板发出。
impl Widget for MenuFlyout {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        Widget::measure(&mut self.menu, ctx, available)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        Widget::arrange(&mut self.menu, ctx, rect)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        Widget::paint(&mut self.menu, ctx, scene)
    }

    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        Widget::update(&mut self.menu, ctx, dt)
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        Widget::event(&mut self.menu, ctx, event)
    }

    fn focusable(&self) -> bool {
        Widget::focusable(&self.menu)
    }

    fn focus_visual(&self) -> bool {
        Widget::focus_visual(&self.menu)
    }

    /// 无障碍：菜单弹层本体即面板，整体委托（角色 `Menu` / 名字 / 当前项同源）。
    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Widget::accessibility(&self.menu)
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        Widget::hit_test(&self.menu, rect, pos)
    }

    fn paint_overflow(&self) -> Insets {
        Widget::paint_overflow(&self.menu)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_element::Key;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, LayoutProps};

    /// 最小锚点：点击打开菜单，记录 PopupClosed。
    struct Opener {
        items: Vec<MenuItem>,
        closed: u32,
    }

    impl Widget for Opener {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(100.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::Click => {
                    MenuFlyout::open(ctx, self.items.clone(), false);
                }
                Event::KeyDown {
                    key: Key::Enter, ..
                } => {
                    MenuFlyout::open(ctx, self.items.clone(), true);
                    ctx.set_handled();
                }
                Event::PopupClosed { .. } => self.closed += 1,
                _ => {}
            }
        }
        fn focusable(&self) -> bool {
            true
        }
        /// 夹具也是「真实的可聚焦控件」：整树无障碍契约要求可聚焦元件有角色与名字，
        /// 夹具缺这两项会让契约断言红在夹具上，掩盖产品件的真实违例。
        fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
            Some(kanesumi_element::AccessInfo {
                role: kanesumi_element::AccessRole::Button,
                name: "打开菜单".to_string(),
                value: None,
                checked: None,
            })
        }
    }

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::new("新建"),
            MenuItem::new("打开").separator(),
            MenuItem::new("排序").with_submenu(vec![
                MenuItem::new("名称").radio("sort"),
                MenuItem::new("日期").radio("sort"),
            ]),
        ]
    }

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(600.0, 400.0);
        let id = h.tree.insert_with(
            h.root(),
            Opener {
                items: items(),
                closed: 0,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(20.0, 20.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn popup(h: &TestHarness) -> WidgetId {
        h.tree.popups().next().expect("菜单应已打开")
    }

    #[test]
    fn click_opens_below_owner_and_item_click_invokes_and_closes() {
        let (mut h, owner) = harness();
        h.click(owner);
        let p = popup(&h);
        let pr = h.rect(p);
        assert!(pr.origin.y >= h.rect(owner).bottom(), "菜单在锚点下方 {pr:?}");
        let item0 = Point::new(pr.origin.x + 20.0, pr.origin.y + 16.0);
        h.click_at(item0);
        let acts = h.take::<MenuInvoked>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1.owner, owner);
        assert_eq!(acts[0].1.label, "新建");
        assert!(h.tree.popups().next().is_none(), "选中后关闭");
    }

    #[test]
    fn keyboard_open_navigate_submenu_and_select_radio() {
        let (mut h, owner) = harness();
        h.tree.focus(owner, true);
        h.key(Key::Enter); // 键盘打开：焦点入菜单、预选第一项
        let p = popup(&h);
        assert_eq!(h.tree.focused(), Some(p));
        h.key(Key::Down);
        h.key(Key::Down); // → 排序
        h.key(Key::Right); // 展开子菜单，预选「名称」
        h.key(Key::Down); // → 日期
        h.key(Key::Enter);
        let acts = h.take::<MenuInvoked>();
        assert_eq!(acts.len(), 1);
        assert_eq!(
            acts[0].1.path,
            MenuPath {
                parent: Some(2),
                index: 1
            }
        );
        assert!(acts[0].1.checked, "单选组项选中后为勾选");
        assert_eq!(h.tree.focused(), Some(owner), "关闭后焦点回锚点");
        assert_eq!(h.tree.get::<Opener>(owner).unwrap().closed, 1);
    }

    #[test]
    fn outside_click_dismisses_and_notifies_owner() {
        let (mut h, owner) = harness();
        h.click(owner);
        h.click_at(Point::new(590.0, 390.0));
        assert!(h.tree.popups().next().is_none());
        assert_eq!(h.tree.get::<Opener>(owner).unwrap().closed, 1);
        assert!(h.take::<MenuInvoked>().is_empty());
    }

    #[test]
    fn submenu_area_is_hit_and_declared_as_overflow() {
        let (mut h, owner) = harness();
        h.click(owner);
        let p = popup(&h);
        let pr = h.rect(p);
        // 悬停「排序」（第 3 项：两项 + 分隔线 2px 之后）展开子菜单。
        h.move_to(Point::new(pr.origin.x + 20.0, pr.origin.y + 32.0 * 2.0 + 2.0 + 16.0));
        h.frame();
        let sub = h
            .tree
            .get::<MenuFlyout>(p)
            .and_then(|m| m.menu.submenu_state().map(|s| s.panel))
            .expect("悬停子菜单父项应展开子菜单");
        assert_eq!(h.tree.hit(sub.center()), Some(p), "子菜单区域命中菜单弹层");
        let ov = h.tree.get::<MenuFlyout>(p).unwrap().paint_overflow();
        h.assert_paint_within(p, ov);
    }

    /// 无障碍（E6）：菜单弹层入快照后报告 `Menu` 角色与非空名字，打开态契约成立。
    #[test]
    fn access_snapshot_reports_menu_role() {
        use kanesumi_element::AccessRole;
        let (mut h, owner) = harness();
        h.click(owner);
        let p = popup(&h);
        let snap = h.access_snapshot();
        let node = snap.find(p).expect("菜单弹层在无障碍快照内");
        assert_eq!(node.role, Some(AccessRole::Menu));
        assert_eq!(node.name.as_deref(), Some("菜单"), "名字是类型名级落点");
        assert!(node.focusable);
        h.assert_access_contract();
    }
}
