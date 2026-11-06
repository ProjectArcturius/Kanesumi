// context_menu.rs —— 右键菜单（ContextMenu）。参 CONTEXT_MENU_SPEC.md。
//
// 语义：右键按下 → 在指针位置弹出菜单（锚定指针点，四象限翻折，无遮罩）。
// 复用 `MetroDropdownMenu`（项规格/级联/单选组/悬停高亮全部复用），差异：
// - 锚点 = 指针坐标（`place_context_menu`），非触发器矩形（`place_popup`）；
// - 无遮罩（LightDismiss，点菜单外关闭）；
// - 按「按下触发」打开（非 DropdownMenu 的 toggle）。
//
// Kanesumi 只负责画：菜单内容（哪些命令/图标/勾选态）由 App 提供（`Vec<MenuItem>` 纯数据），
// 本控件不持目标/命令语义。参 CONTEXT_MENU_SPEC §Ⅰ.3。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::Scene;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use kanesumi_element::{
    Event, EventCtx, MeasureCtx, PaintCtx, PopupSpec, UpdateCtx, Widget, WidgetId,
};

use crate::dropdown_menu::{MenuInteractionSignature, MenuItem, MenuPath, MetroDropdownMenu};
use crate::popup::{PopupState, place_context_menu};

/// 右键菜单控件 —— 无遮罩弹层，锚定指针。
///
/// 内部持一个 [`MetroDropdownMenu`] 承担项/级联/悬停；本控件加右键语义层：
/// - `anchor`：右键按下点（定位基准）；
/// - 打开 = `open_at`（锚点 + 内容 + 屏幕矩形），关闭 = `close`（LightDismiss）。
#[derive(Debug, Clone)]
pub struct MetroContextMenu {
    menu: MetroDropdownMenu,
    /// 右键按下点（面板定位基准）。
    pub anchor: Point,
}

impl MetroContextMenu {
    pub fn new() -> Self {
        Self {
            menu: MetroDropdownMenu::new(Vec::new()),
            anchor: Point::new(0.0, 0.0),
        }
    }

    /// 元素树弹层构造：`owner` = 打开菜单的锚点控件（选中动作的 `MenuInvoked.owner`、
    /// 关闭通知与焦点交还对象）。面板即刻为展开态（入场动画由弹层表面承担）。
    pub fn for_tree(owner: WidgetId, items: Vec<MenuItem>) -> Self {
        let mut menu = MetroDropdownMenu::new(items);
        menu.anim.open();
        menu.owner = Some(owner);
        Self {
            menu,
            anchor: Point::new(0.0, 0.0),
        }
    }

    /// 在指针位置打开右键菜单（元素树路径，参 CONTEXT_MENU_SPEC §Ⅲ / ELEMENT_MIGRATION §8）。
    /// 点锚定：面板左上角落在 `at`，右 / 下放不下翻左 / 上（框架 `popup_slot`），
    /// 放置区 = `popup_bounds`（矮表面由外壳放大到整个输出，菜单不再被条面裁剪）。
    /// 无遮罩（LightDismiss 点外关闭，框架负责）。返回弹层 id。
    pub fn open(ctx: &mut EventCtx, items: Vec<MenuItem>, at: Point, keyboard: bool) -> WidgetId {
        let owner = ctx.id();
        let mut flyout = MetroContextMenu::for_tree(owner, items);
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

    /// 在 `anchor`（指针坐标）打开右键菜单。`screen` 为可显示区域（输出/表面矩形）。
    ///
    /// 面板经 [`place_context_menu`] 四象限翻折定位；动画按菜单弹出轨道（无遮罩）。
    pub fn open_at(
        &mut self,
        engine: &TextEngine,
        anchor: Point,
        items: Vec<MenuItem>,
        screen: Rect,
    ) {
        self.anchor = anchor;
        self.menu.items = items;
        self.menu.invalidate_layout();
        let size = self.menu.panel_size(engine);
        let rect = place_context_menu(anchor, size, screen);
        self.menu.open(rect);
    }

    /// 关闭（LightDismiss / 点选 / Esc）。回收级联。
    pub fn close(&mut self) {
        self.menu.close();
    }

    /// 每帧动画 tick。
    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
    }

    /// 面板逻辑尺寸（量测，不定位）。浮层表面高度 / 布局预留用。
    pub fn panel_size(&self, engine: &TextEngine) -> kanesumi_core::Size {
        self.menu.panel_size(engine)
    }

    /// 面板是否可见（含开/关动画期间）。
    pub fn is_visible(&self) -> bool {
        self.menu.anim.is_visible()
    }

    /// 面板是否已完全打开（稳定 Open 态）。
    pub fn is_open(&self) -> bool {
        self.menu.anim.is_open()
    }

    /// 悬停路由（复用 DropdownMenu 级联语义）：命中项高亮，悬停嵌套项自动展开子菜单。
    /// 返回悬停语义是否改变（S1 输入门控：无变化 → 宿主不置脏）。
    pub fn hover(&mut self, engine: &TextEngine, screen: Rect, pos: Point) -> bool {
        self.menu.hover(engine, screen, pos)
    }

    /// 悬停语义签名（S1 输入门控用。参 `MetroDropdownMenu::interaction_signature`）。
    pub fn interaction_signature(&self) -> MenuInteractionSignature {
        self.menu.interaction_signature()
    }

    /// 是否处于开/关动画推进中（静态 Open 不需要逐帧重绘）。
    pub fn is_animating(&self) -> bool {
        self.menu.is_animating()
    }

    /// 当前悬停项矩形（顶层 + 子菜单），S4 局部损坏重绘用。
    pub fn hovered_rects(&self) -> Vec<Rect> {
        self.menu.hovered_rects()
    }

    /// 命中测试（含级联）：返回 `(parent, child)` 路径或顶层索引。
    pub fn path_at(&self, pos: Point) -> Option<MenuPath> {
        self.menu.path_at(pos)
    }

    /// 菜单项点选后标记勾选态（单选组互斥）。
    pub fn select(&mut self, path: MenuPath) -> bool {
        match path.parent {
            None => self.menu.select(path.index),
            Some(_) => self.menu.select_submenu(path),
        }
    }

    /// 渲染：面板 + 级联（**无遮罩**）。关态渲染空（透明不可见）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        self.menu.render_panel(theme, engine, scene);
        self.menu.render_submenu(theme, engine, scene);
    }

    /// 面板矩形（渲染/命中参考）。
    pub fn panel_rect(&self) -> Rect {
        self.menu.panel_rect
    }

    /// 顶层第 `i` 项的矩形（面板内）。供悬停/点选定位。
    pub fn item_rect(&self, i: usize) -> Rect {
        self.menu.item_rect(i)
    }

    /// 当前展开的子菜单（父项 + 面板 + 子菜单控件）。
    pub fn submenu_state(&self) -> Option<&crate::dropdown_menu::SubmenuState> {
        self.menu.submenu_state()
    }

    /// 子菜单当前悬停项索引。
    pub fn submenu_hovered(&self) -> Option<usize> {
        self.menu.submenu_hovered()
    }

    /// 指定路径是否为嵌套父项（带子菜单）。点击父项应保持/展开子菜单，
    /// **不触发命令**（如「压缩为」父项 = 展开 ZIP/TAR 子菜单，而非直接压缩）。
    pub fn is_submenu_parent(&self, path: &[usize]) -> bool {
        match path {
            [i] => self.menu.items.get(*i).is_some_and(|it| it.is_submenu()),
            _ => false,
        }
    }

    /// 菜单状态（供外部判断/测试）。
    pub fn state(&self) -> PopupState {
        self.menu.state()
    }
}

/// 面板即弹层节点：全部交互在 [`MetroDropdownMenu` 的 `Widget` 实现](MetroDropdownMenu)，
/// 本包装只补「右键 = 点锚定 + 无遮罩」的打开入口（[`Self::open`]）。
impl Widget for MetroContextMenu {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        Widget::measure(&mut self.menu, ctx, available)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
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

    /// 无障碍：右键菜单的面板与下拉菜单同构，整体委托（角色 / 名字 / 当前项 / 合成条目同源）。
    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Widget::accessibility(&self.menu)
    }

    /// 合成条目同理委托：右键菜单的条目与下拉菜单是同一套语义。
    fn access_items(&self, rect: Rect, theme: &MetroTheme) -> Vec<kanesumi_element::AccessItem> {
        Widget::access_items(&self.menu, rect, theme)
    }

    fn hit_test(
        &self,
        rect: Rect,
        pos: Point,
    ) -> bool {
        Widget::hit_test(&self.menu, rect, pos)
    }

    fn paint_overflow(&self) -> kanesumi_element::Insets {
        Widget::paint_overflow(&self.menu)
    }
}

impl Default for MetroContextMenu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCREEN: Rect = Rect::new(0.0, 0.0, 1024.0, 768.0);

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            return TextEngine::load(p).ok();
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn open_at_anchors_at_pointer() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(300.0, 300.0),
            vec![MenuItem::new("Reset")],
            TEST_SCREEN,
        );
        assert!(menu.is_visible());
        // 屏幕中央，右侧/下方空间足 → 面板右上角 = 指针。
        assert_eq!(menu.panel_rect().origin, Point::new(300.0, 300.0));
        // 无遮罩：不渲染全屏遮罩矩形（render_panel 只画面板）。
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        menu.render(&theme, &engine, &mut scene);
        let panel = menu.panel_rect();
        assert!(
            scene.commands.iter().all(|c| match c {
                kanesumi_canvas::SceneCommand::FillRect { rect, .. } => {
                    // 全屏遮罩 = 覆盖整个 TEST_SCREEN 的 FillRect；面板内矩形允许。
                    let full = rect.origin.x <= 0.5
                        && rect.origin.y <= 0.5
                        && (rect.origin.x + rect.size.width) >= TEST_SCREEN.right() - 0.5
                        && (rect.origin.y + rect.size.height) >= TEST_SCREEN.bottom() - 0.5;
                    !full || (rect.origin.x >= panel.origin.x - 0.5
                        && rect.origin.y >= panel.origin.y - 0.5)
                }
                _ => true,
            }),
            "右键菜单不渲染全屏遮罩"
        );
    }

    #[test]
    fn open_at_flips_to_fit_screen_corner() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(1000.0, 750.0),
            vec![MenuItem::new("A"), MenuItem::new("B"), MenuItem::new("C")],
            TEST_SCREEN,
        );
        let r = menu.panel_rect();
        assert!(
            r.origin.x + r.size.width <= TEST_SCREEN.right() + 0.01,
            "面板右缘不越屏"
        );
        assert!(
            r.origin.y + r.size.height <= TEST_SCREEN.bottom() + 0.01,
            "面板下缘不越屏"
        );
    }

    #[test]
    fn closes_to_invisible() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(300.0, 300.0),
            vec![MenuItem::new("Reset")],
            TEST_SCREEN,
        );
        menu.close();
        assert!(menu.is_visible(), "关闭动画期间仍可见");
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert!(!menu.is_visible());
        assert_eq!(menu.state(), PopupState::Closed);
    }

    #[test]
    fn default_is_invisible() {
        let menu = MetroContextMenu::new();
        assert!(!menu.is_visible());
        assert!(!menu.is_open());
    }

    #[test]
    fn path_at_hits_items_after_open() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroContextMenu::new();
        menu.open_at(
            &engine,
            Point::new(100.0, 100.0),
            vec![MenuItem::new("A"), MenuItem::new("B")],
            TEST_SCREEN,
        );
        menu.update(1.0);
        let r = menu.panel_rect();
        // 第一项：面板内顶部。
        let hit = menu.path_at(Point::new(r.origin.x + 20.0, r.origin.y + 10.0));
        assert_eq!(hit, Some(MenuPath { parent: None, index: 0 }));
        // 面板外 → None。
        assert_eq!(menu.path_at(Point::new(5.0, 5.0)), None);
    }
}

// ── 元素树弹层（E4）：右键释放打开 / 点外关 / 键盘 / 焦点回锚 / 尺寸=量测值 ──────

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, LayoutProps};

    /// 最小右键目标：右键（释放）→ `MetroContextMenu::open`，记录 PopupClosed 次数。
    struct Target {
        items: Vec<MenuItem>,
        closed: u32,
    }

    impl Widget for Target {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(100.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            match event {
                Event::ContextRequested { pos } => {
                    MetroContextMenu::open(ctx, self.items.clone(), *pos, false);
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
                name: "右键目标".to_string(),
                value: None,
                checked: None,
            })
        }
    }

    fn items() -> Vec<MenuItem> {
        vec![
            MenuItem::new("新建"),
            MenuItem::new("排序").with_submenu(vec![
                MenuItem::new("名称").radio("sort"),
                MenuItem::new("日期").radio("sort"),
            ]),
            MenuItem::new("复制"),
            MenuItem::new("粘贴"),
        ]
    }

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(600.0, 400.0);
        let id = h.tree.insert_with(
            h.root(),
            Target {
                items: items(),
                closed: 0,
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: kanesumi_element::Insets::new(20.0, 20.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn popup(h: &TestHarness) -> WidgetId {
        h.tree.popups().next().expect("右键菜单应已打开")
    }

    /// 右键（按下+释放）→ 菜单弹层；按正典 §Ⅱ 在**释放**时打开（框架保证）。
    fn right_click(h: &mut TestHarness, id: WidgetId) {
        let c = h.center(id);
        h.press_at(c, kanesumi_element::PointerButton::Right);
        h.release_at(c, kanesumi_element::PointerButton::Right);
        h.frame();
    }

    /// 菜单弹层尺寸 = 内容测量值（`panel_size`），点锚定落在指针处。
    #[test]
    fn opens_point_anchored_with_measured_size() {
        let (mut h, target) = harness();
        right_click(&mut h, target);
        let p = popup(&h);
        let expected = {
            let m = h.tree.get::<MetroContextMenu>(p).unwrap();
            let mut probe = m.clone();
            probe.anchor = Point::new(0.0, 0.0);
            probe.menu.panel_size(&h.engine)
        };
        let pr = h.rect(p);
        assert_eq!(pr.size, expected, "弹层矩形 = 面板量测值");
        assert_eq!(pr.origin, Point::new(70.0, 36.0), "面板左上角 = 指针点");
        // 无遮罩：弹层自身之外不画整面 FillRect（render_panel 只画面板）。
        // 无遮罩（CONTEXT_MENU_SPEC §Ⅳ）：面板之外不得有整面压暗 FillRect。
        let cmds = h.tree.painted(p);
        let mask = cmds.iter().any(|c| {
            matches!(
                c,
                kanesumi_canvas::SceneCommand::FillRect { rect, .. }
                if rect.size.width >= 599.0 && rect.size.height >= 399.0
            )
        });
        assert!(!mask, "右键菜单无全屏遮罩");
    }

    /// 矮表面（Dock 条同款 76px）：弹层不裁进条面 —— 外壳放大 `popup_bounds` 后，
    /// 弹层完整尺寸 = 量测值（旧叠加渲染在 76px 表面内必然塌缩，E4 入树的意义）。
    #[test]
    fn full_size_on_short_surface_with_expanded_bounds() {
        let (mut h, target) = harness();
        h.resize(400.0, 76.0);
        // Shell 对 LayerBottom 的同款放置区：整个输出（条上方为主）。
        h.tree
            .set_popup_bounds(Some(Rect::new(0.0, -(800.0 - 76.0), 400.0, 800.0)));
        h.frame();
        right_click(&mut h, target);
        let p = popup(&h);
        let extent = h.tree.popup_extent(p).expect("弹层有范围");
        let expected = {
            let m = h.tree.get::<MetroContextMenu>(p).unwrap().clone();
            m.menu.panel_size(&h.engine)
        };
        assert_eq!(extent.size, expected, "矮表面上弹层仍 = 内容量测值");
        assert!(
            extent.size.height > 76.0,
            "菜单高度 {} 超出条面 76px 也不得被裁",
            extent.size.height
        );
    }

    /// 点外关闭（LightDismiss）+ 焦点交还锚点 + PopupClosed 通知。
    #[test]
    fn outside_click_dismisses_and_focus_returns() {
        let (mut h, target) = harness();
        right_click(&mut h, target);
        assert_eq!(h.tree.focused(), Some(popup(&h)), "打开后焦点在弹层");
        h.click_at(Point::new(590.0, 390.0));
        assert!(h.tree.popups().next().is_none(), "点外关闭");
        assert_eq!(h.tree.focused(), Some(target), "焦点回锚点");
        assert_eq!(h.tree.get::<Target>(target).unwrap().closed, 1);
        assert!(h.take::<crate::menu_flyout::MenuInvoked>().is_empty());
    }

    /// Esc 关闭：焦点回锚点（正典 §Ⅳ）。
    #[test]
    fn esc_closes_and_focus_returns() {
        let (mut h, target) = harness();
        right_click(&mut h, target);
        assert!(h.key(kanesumi_element::Key::Escape), "Esc 关闭弹层");
        assert!(h.tree.popups().next().is_none());
        assert_eq!(h.tree.focused(), Some(target));
        assert_eq!(h.tree.get::<Target>(target).unwrap().closed, 1);
    }

    /// 方向键 / Enter / 子菜单：Down 至嵌套项 → Right 展开 → Down → Enter 激活
    /// 子菜单项（路径 [parent, child]，单选组勾选），随后关闭、焦点回锚点。
    #[test]
    fn keyboard_navigates_submenu_and_activates() {
        let (mut h, target) = harness();
        right_click(&mut h, target);
        let p = popup(&h);
        assert_eq!(h.tree.focused(), Some(p));
        h.key(kanesumi_element::Key::Down); // → 新建
        h.key(kanesumi_element::Key::Down); // → 排序（嵌套项）
        h.key(kanesumi_element::Key::Right); // 展开子菜单，预选「名称」
        h.key(kanesumi_element::Key::Down); // → 日期
        h.key(kanesumi_element::Key::Enter);
        let acts = h.take::<crate::menu_flyout::MenuInvoked>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1.owner, target);
        assert_eq!(
            acts[0].1.path,
            MenuPath {
                parent: Some(1),
                index: 1
            }
        );
        assert!(acts[0].1.checked, "单选组项选中后为勾选");
        assert!(h.tree.popups().next().is_none(), "选中后关闭");
        assert_eq!(h.tree.focused(), Some(target), "关闭后焦点回锚点");
        assert_eq!(h.tree.get::<Target>(target).unwrap().closed, 1);
    }

    /// 级联子菜单区域可命中、并声明为绘制外扩（三条保险断言在打开态仍成立）。
    #[test]
    fn submenu_hit_and_overflow_declared() {
        let (mut h, target) = harness();
        right_click(&mut h, target);
        let p = popup(&h);
        let pr = h.rect(p);
        // 悬停「排序」（第 2 项：一项 32 + 分隔线 2 之后）展开子菜单。
        h.move_to(Point::new(pr.origin.x + 20.0, pr.origin.y + 32.0 + 16.0));
        h.frame();
        let sub = h
            .tree
            .get::<MetroContextMenu>(p)
            .and_then(|m| m.menu.submenu_state().map(|s| s.panel))
            .expect("悬停嵌套项应展开子菜单");
        assert_eq!(h.tree.hit(sub.center()), Some(p), "子菜单区域命中菜单弹层");
        let ov = h
            .tree
            .get::<MetroContextMenu>(p)
            .unwrap()
            .menu
            .paint_overflow();
        assert!(ov != kanesumi_element::Insets::ZERO, "子菜单声明外扩");
        h.assert_paint_within(p, ov);
        h.assert_no_hit_outside(p);
        h.assert_contained();
    }

    /// 无障碍（E6）：右键菜单入快照后报告 `Menu` 角色与非空名字，打开态契约成立。
    #[test]
    fn access_snapshot_reports_menu_role() {
        use kanesumi_element::AccessRole;
        let (mut h, target) = harness();
        right_click(&mut h, target);
        let p = popup(&h);
        let snap = h.access_snapshot();
        let node = snap.find(p).expect("右键菜单在无障碍快照内");
        assert_eq!(node.role, Some(AccessRole::Menu));
        assert_eq!(node.name.as_deref(), Some("菜单"), "名字是类型名级落点");
        assert!(node.focusable);
        h.assert_access_contract();
    }
}
