// MetroSplitButton —— 复合命令按钮（Primary 主命令 + Secondary 下拉）。参 CONTROL_SPEC §19。
//
// 移植自 microsoft-ui-xaml/dev/SplitButton（SplitButton.cpp + SplitButton_v1.xaml）：
// - Primary（*，MinWidth 35）│ 分隔线 1px │ Secondary（35px，chevron E70D → 自绘）；
// - 点 Primary → 返回主命令；点 Secondary → toggle MenuFlyout（复用 MetroDropdownMenu）；
// - FlyoutOpen = 两区全 Pressed（白 22%）。
// 与 MetroDropDownButton 同型，仅多一个 Primary 命中区。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::dropdown_menu::{MenuItem, MetroDropdownMenu};
use crate::popup::{place_popup, popup_gap};

/// Secondary 区宽（SplitButtonSecondaryButtonSize = 35）。
const SECONDARY_WIDTH: f32 = 35.0;
/// Separator 列宽（1px）。
const SEPARATOR_WIDTH: f32 = 1.0;

/// 命中部件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitButtonPart {
    None,
    Primary,
    Secondary,
}

/// 点击结果：Primary 主命令 / 下拉项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitButtonClick {
    None,
    Primary,
    Index(usize),
}

/// MetroSplitButton —— 复合命令按钮。参 CONTROL_SPEC §19。
#[derive(Debug, Clone)]
pub struct MetroSplitButton {
    pub label: String,
    /// 下拉（MenuFlyout）项。
    pub menu: MetroDropdownMenu,
    pub primary_hovered: bool,
    pub primary_pressed: bool,
    pub secondary_hovered: bool,
    pub secondary_pressed: bool,
    /// 元素树下当前展开的菜单弹层（`MenuFlyout` 节点）；旧路径不用。
    tree_popup: Option<kanesumi_element::WidgetId>,
    /// 元素树指针下命中的区（Primary / Secondary / None）。
    tree_hover: SplitButtonPart,
    /// 元素树按下时记录的区（Click 无坐标，据此在 PointerUp 分派）。
    tree_press: SplitButtonPart,
}

impl MetroSplitButton {
    pub fn new(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            menu: MetroDropdownMenu::new(items),
            primary_hovered: false,
            primary_pressed: false,
            secondary_hovered: false,
            secondary_pressed: false,
            tree_popup: None,
            tree_hover: SplitButtonPart::None,
            tree_press: SplitButtonPart::None,
        }
    }

    /// 固有尺寸：标签 + Primary/Secondary 区。
    pub fn measure(&self, engine: &TextEngine, style: TextStyle) -> Size {
        let width =
            engine.measure(&self.label, style.size) + 16.0 + SECONDARY_WIDTH + SEPARATOR_WIDTH;
        let height = style.line_height + 11.0;
        Size::new(width, height)
    }

    /// Primary 区 rect。
    pub fn primary_rect(&self, rect: Rect) -> Rect {
        let w = (rect.size.width - SECONDARY_WIDTH - SEPARATOR_WIDTH).max(35.0);
        Rect::new(rect.origin.x, rect.origin.y, w, rect.size.height)
    }

    /// Secondary 区 rect（最右 35px）。
    pub fn secondary_rect(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.right() - SECONDARY_WIDTH,
            rect.origin.y,
            SECONDARY_WIDTH,
            rect.size.height,
        )
    }

    /// Separator 线 rect（1px 竖线）。
    pub fn separator_rect(&self, rect: Rect) -> Rect {
        let s = self.secondary_rect(rect);
        Rect::new(
            s.origin.x - SEPARATOR_WIDTH,
            rect.origin.y,
            SEPARATOR_WIDTH,
            rect.size.height,
        )
    }

    /// 命中：Primary / Secondary / None。
    pub fn hit(&self, rect: Rect, pos: Point) -> SplitButtonPart {
        if self.secondary_rect(rect).contains(pos) {
            return SplitButtonPart::Secondary;
        }
        if self.primary_rect(rect).contains(pos) {
            return SplitButtonPart::Primary;
        }
        SplitButtonPart::None
    }

    pub fn is_flyout_open(&self) -> bool {
        matches!(
            self.menu.anim.state(),
            crate::popup::PopupState::Opening | crate::popup::PopupState::Open
        )
    }

    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
        if !self.menu.anim.is_visible() {
            self.primary_pressed = false;
            self.secondary_pressed = false;
        }
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) -> bool {
        let part = self.hit(rect, pos);
        self.primary_hovered = part == SplitButtonPart::Primary && !self.primary_pressed;
        self.secondary_hovered = part == SplitButtonPart::Secondary && !self.secondary_pressed;
        if self.menu.anim.is_visible() {
            self.menu.hovered = self.menu.item_at(pos);
        }
        part != SplitButtonPart::None
    }

    /// 按下。
    pub fn press(&mut self, rect: Rect, pos: Point) -> bool {
        if self.is_flyout_open() {
            if self.menu.item_at(pos).is_some() {
                return true;
            }
            if self.hit(rect, pos) == SplitButtonPart::None {
                self.menu.close();
                return true;
            }
        }
        match self.hit(rect, pos) {
            SplitButtonPart::Primary => {
                self.primary_pressed = true;
                true
            }
            SplitButtonPart::Secondary => {
                self.secondary_pressed = true;
                true
            }
            SplitButtonPart::None => false,
        }
    }

    /// 释放：Primary → 主命令；Secondary → toggle 下拉；下拉项 → Index。
    pub fn release(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        pos: Point,
    ) -> SplitButtonClick {
        if self.is_flyout_open()
            && let Some(i) = self.menu.item_at(pos)
        {
            self.menu.close();
            self.primary_pressed = false;
            self.secondary_pressed = false;
            return SplitButtonClick::Index(i);
        }
        let part = self.hit(rect, pos);
        match part {
            SplitButtonPart::Primary if self.primary_pressed => {
                self.primary_pressed = false;
                SplitButtonClick::Primary
            }
            SplitButtonPart::Secondary if self.secondary_pressed => {
                self.secondary_pressed = false;
                self.toggle_flyout(engine, rect, screen);
                SplitButtonClick::None
            }
            _ => {
                self.primary_pressed = false;
                self.secondary_pressed = false;
                SplitButtonClick::None
            }
        }
    }

    /// 打开/收起下拉。
    pub fn toggle_flyout(&mut self, engine: &TextEngine, rect: Rect, screen: Rect) {
        if self.is_flyout_open() {
            self.menu.close();
        } else {
            let size = self.menu.panel_size(engine);
            let placement = place_popup(rect, size, screen, popup_gap());
            self.menu.open(placement.rect);
        }
    }

    /// 渲染：底 + 边框 + Primary 标签 + 分隔线 + Secondary chevron +（下拉）。
    pub fn render(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let indication = &theme.indication;
        let style = theme.typography.body;

        let primary = self.primary_rect(rect);
        let secondary = self.secondary_rect(rect);

        // 底：标准按钮填充（UWP `ButtonBackground` = BaseLow 20%），不得取 `surface`。
        scene.fill_rounded_rect(colors.control_fill, rect, theme.tokens.corner_radius);
        // 交互 tint
        if self.primary_pressed {
            scene.fill_rect(indication.press_tint, primary);
        } else if self.primary_hovered {
            scene.fill_rect(indication.hover_tint, primary);
        }
        if self.secondary_pressed {
            scene.fill_rect(indication.press_tint, secondary);
        } else if self.secondary_hovered {
            scene.fill_rect(indication.hover_tint, secondary);
        }
        // 边框
        scene.stroke_rounded_rect(colors.divider, rect, 1.0, theme.tokens.corner_radius);

        // Primary 标签：可用宽取自 primary 区（非量测宽度），单行省略号。
        // 2026-09-22 审计 P0-1：量测宽度只用于固有尺寸，绘制矩形必须由布局矩形派生。
        scene.label(
            self.label.clone(),
            Rect::new(
                primary.origin.x + 8.0,
                primary.origin.y + (primary.size.height - style.line_height) / 2.0,
                (primary.size.width - 16.0).max(0.0),
                style.line_height,
            ),
            colors.on_surface,
            style,
            TextAlign::Left,
        );

        // 分隔线
        scene.fill_rect(colors.divider, self.separator_rect(rect));

        // Secondary chevron（右侧，Pad 0,0,9,0 → 右 9px）
        let chevron = Rect::new(
            secondary.right() - 9.0 - 12.0,
            rect.origin.y + (rect.size.height - 12.0) / 2.0,
            12.0,
            12.0,
        );
        glyph::chevron_down(scene, chevron, colors.on_surface_variant);

        // 下拉
        if self.menu.anim.is_visible() {
            self.menu.render(theme, engine, screen, scene);
        }
    }
}

// ── 元素树接入（弹层类，参 docs/ELEMENT_MIGRATION.md §8；模板同 drop_down_button.rs）──
//
// 旧路径里下拉画在按钮自己的 Scene 里（需宿主传整屏 `screen`）。元素树里菜单是覆盖层上的
// 独立节点 `MenuFlyout`：本控件只做「两个命中区的分派」与「展开中外观」，放置 / 命中 /
// 键盘 / 关闭 / 焦点交还都交给框架与 MenuFlyout。Primary 区发主动作，Secondary 区开菜单。
// PointerUp 用坐标判区（`Click` 无坐标）：`PointerDown` 记区，`PointerUp` 按记录分派。

/// 元素树动作：Primary（主命令）区被激活。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SplitButtonInvoked;

impl kanesumi_element::Widget for MetroSplitButton {
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        _available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        MetroSplitButton::measure(self, ctx.engine(), ctx.theme().typography.body)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let saved = (
            self.primary_hovered,
            self.primary_pressed,
            self.secondary_hovered,
            self.secondary_pressed,
        );
        let hovered = ctx.state().hovered;
        // 展开期间 chevron 区保持按下外观（对齐旧 FlyoutOpen 语义）。
        self.primary_pressed = self.tree_press == SplitButtonPart::Primary;
        self.secondary_pressed =
            self.tree_press == SplitButtonPart::Secondary || self.tree_popup.is_some();
        self.primary_hovered =
            hovered && self.tree_hover == SplitButtonPart::Primary && !self.primary_pressed;
        self.secondary_hovered =
            hovered && self.tree_hover == SplitButtonPart::Secondary && !self.secondary_pressed;
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), ctx.rect(), ctx.surface(), scene);
        (
            self.primary_hovered,
            self.primary_pressed,
            self.secondary_hovered,
            self.secondary_pressed,
        ) = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                let part = self.hit(ctx.rect(), *pos);
                if part != self.tree_hover {
                    self.tree_hover = part;
                    ctx.invalidate_paint();
                }
            }
            Event::PointerLeave => {
                self.tree_hover = SplitButtonPart::None;
                ctx.invalidate_paint();
            }
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let part = self.hit(ctx.rect(), *pos);
                if part != SplitButtonPart::None {
                    self.tree_press = part;
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let pressed = self.tree_press;
                self.tree_press = SplitButtonPart::None;
                if pressed != SplitButtonPart::None && self.hit(ctx.rect(), *pos) == pressed {
                    match pressed {
                        SplitButtonPart::Primary => ctx.emit(SplitButtonInvoked),
                        SplitButtonPart::Secondary => {
                            // 已展开再点 = 收起；否则打开。与旧 `toggle_flyout` 语义一致。
                            if let Some(p) = self.tree_popup.take() {
                                ctx.close_popup(p);
                            } else {
                                let items = self.menu.items.clone();
                                self.tree_popup =
                                    Some(crate::menu_flyout::MenuFlyout::open(ctx, items, false));
                            }
                        }
                        SplitButtonPart::None => {}
                    }
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::KeyDown {
                key: Key::Enter | Key::Char(' '),
                ..
            } => {
                ctx.emit(SplitButtonInvoked);
                ctx.set_handled();
            }
            Event::KeyDown { key: Key::Down, .. } => {
                // Down / Alt+Down 打开菜单（键盘打开预选首项）。
                if self.tree_popup.is_none() {
                    let items = self.menu.items.clone();
                    self.tree_popup =
                        Some(crate::menu_flyout::MenuFlyout::open(ctx, items, true));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            Event::PopupClosed { popup } if self.tree_popup == Some(*popup) => {
                self.tree_popup = None;
                ctx.invalidate_paint();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Button,
            name: self.label.clone(),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::menu_flyout::MenuInvoked;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(500.0, 400.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroSplitButton::new(
                "保存",
                vec![MenuItem::new("另存为…"), MenuItem::new("导出…")],
            ),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(10.0, 10.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn part_center(h: &TestHarness, id: WidgetId, part: SplitButtonPart) -> Point {
        let r = h.rect(id);
        match part {
            SplitButtonPart::Primary => h
                .tree
                .get::<MetroSplitButton>(id)
                .unwrap()
                .primary_rect(r)
                .center(),
            SplitButtonPart::Secondary => h
                .tree
                .get::<MetroSplitButton>(id)
                .unwrap()
                .secondary_rect(r)
                .center(),
            SplitButtonPart::None => r.center(),
        }
    }

    #[test]
    fn primary_click_emits_action_without_menu() {
        let (mut h, id) = harness();
        h.click_at(part_center(&h, id, SplitButtonPart::Primary));
        assert_eq!(h.take::<SplitButtonInvoked>().len(), 1);
        assert!(h.tree.popups().next().is_none(), "主命令不应开菜单");
        assert!(
            h.tree.get::<MetroSplitButton>(id).unwrap().tree_popup.is_none()
        );
    }

    #[test]
    fn secondary_click_opens_below_and_selection_reports_owner() {
        let (mut h, id) = harness();
        h.click_at(part_center(&h, id, SplitButtonPart::Secondary));
        let p = h.tree.popups().next().expect("chevron 区开菜单");
        assert!(h.rect(p).origin.y >= h.rect(id).bottom(), "菜单落在按钮下方");
        let pr = h.rect(p);
        h.click_at(Point::new(pr.origin.x + 20.0, pr.origin.y + 16.0));
        let acts = h.take::<MenuInvoked>();
        assert_eq!(acts.len(), 1);
        assert_eq!(acts[0].1.owner, id);
        assert_eq!(acts[0].1.label, "另存为…");
        assert!(
            h.tree.get::<MetroSplitButton>(id).unwrap().tree_popup.is_none(),
            "选中后复位"
        );
    }

    #[test]
    fn down_opens_with_focus_inside_and_escape_returns_focus() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Down);
        let p = h.tree.popups().next().expect("Down 键开菜单");
        assert_eq!(h.tree.focused(), Some(p), "键盘打开焦点入菜单");
        h.key(Key::Escape);
        assert_eq!(h.tree.focused(), Some(id), "Esc 关闭后焦点回按钮");
        assert!(
            h.tree.get::<MetroSplitButton>(id).unwrap().tree_popup.is_none()
        );
    }

    #[test]
    fn outside_click_dismisses_and_resets() {
        let (mut h, id) = harness();
        h.click_at(part_center(&h, id, SplitButtonPart::Secondary));
        assert!(h.tree.popups().next().is_some());
        h.click_at(Point::new(480.0, 390.0));
        assert!(h.tree.popups().next().is_none());
        assert!(
            h.tree.get::<MetroSplitButton>(id).unwrap().tree_popup.is_none(),
            "点外部关闭后复位"
        );
    }

    #[test]
    fn enter_activates_primary_action() {
        let (mut h, _id) = harness();
        h.tab();
        h.key(Key::Enter);
        assert_eq!(h.take::<SplitButtonInvoked>().len(), 1);
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, id) = harness();
        h.click_at(part_center(&h, id, SplitButtonPart::Secondary));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
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

    fn btn() -> MetroSplitButton {
        MetroSplitButton::new(
            "保存",
            vec![MenuItem::new("另存为…"), MenuItem::new("导出…")],
        )
    }

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 140.0, 36.0)
    }

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    #[test]
    fn geometry_splits_primary_secondary() {
        let b = btn();
        let r = rect();
        let primary = b.primary_rect(r);
        let secondary = b.secondary_rect(r);
        let sep = b.separator_rect(r);
        assert_eq!(secondary.size.width, 35.0);
        assert_eq!(sep.size.width, 1.0);
        assert_eq!(primary.right(), sep.origin.x);
        assert_eq!(sep.right(), secondary.origin.x);
    }

    #[test]
    fn hit_detects_parts() {
        let b = btn();
        let r = rect();
        let primary = b.primary_rect(r);
        assert_eq!(b.hit(r, Point::new(10.0, 18.0)), SplitButtonPart::Primary);
        let secondary = b.secondary_rect(r);
        assert_eq!(
            b.hit(r, Point::new(secondary.center().x, 18.0)),
            SplitButtonPart::Secondary
        );
        assert_eq!(b.hit(r, Point::new(500.0, 500.0)), SplitButtonPart::None);
    }

    #[test]
    fn primary_click_returns_command() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        let r = rect();
        let primary = b.primary_rect(r);
        let p = Point::new(primary.center().x, primary.center().y);
        assert!(b.press(r, p));
        assert_eq!(
            b.release(&engine, r, screen(), p),
            SplitButtonClick::Primary
        );
    }

    #[test]
    fn secondary_toggles_flyout() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        let r = rect();
        let secondary = b.secondary_rect(r);
        let p = Point::new(secondary.center().x, secondary.center().y);
        assert!(b.press(r, p));
        b.release(&engine, r, screen(), p);
        assert!(b.is_flyout_open());
        b.press(r, p);
        b.release(&engine, r, screen(), p);
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        assert!(!b.is_flyout_open(), "再点关闭");
    }

    #[test]
    fn flyout_item_returns_index() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        let r = rect();
        let secondary = b.secondary_rect(r);
        let p = Point::new(secondary.center().x, secondary.center().y);
        b.press(r, p);
        b.release(&engine, r, screen(), p);
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        let panel = b.menu.panel_rect;
        let item_p = Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0);
        assert_eq!(
            b.release(&engine, r, screen(), item_p),
            SplitButtonClick::Index(0)
        );
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        assert!(!b.is_flyout_open());
    }

    #[test]
    fn click_outside_closes() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        let r = rect();
        let secondary = b.secondary_rect(r);
        let p = Point::new(secondary.center().x, secondary.center().y);
        b.press(r, p);
        b.release(&engine, r, screen(), p);
        assert!(b.is_flyout_open());
        assert!(b.press(r, Point::new(700.0, 500.0)), "点外部消费并关闭");
        assert!(!b.is_flyout_open());
    }

    #[test]
    fn render_emits_label_chevron_separator() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = btn();
        let mut scene = kanesumi_canvas::Scene::default();
        b.render(&theme, &engine, rect(), screen(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 1, "Secondary chevron");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 1, "Primary 标签");
    }
}
