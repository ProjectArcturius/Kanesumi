// MetroDropDownButton —— 下拉按钮（Button + chevron + MenuFlyout）。参 CONTROL_SPEC §17。
//
// 移植自 microsoft-ui-xaml/dev/DropDownButton（DropDownButton.cpp + DropDownButton_v1.xaml）：
// - 结构 = MetroButton Standard + 右侧 chevron（E70D → 自绘，FontSize 12，Margin 6,0,0,0）；
// - 点击 → toggle 关联 MetroDropdownMenu（MenuFlyout）。
// Flyout 打开时按钮呈 Pressed 亮度（对齐 MenuBar Selected 语义）。

use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::dropdown_menu::{MenuItem, MetroDropdownMenu};
use crate::popup::{place_popup, popup_gap};
use crate::state::ControlState;

/// 按钮 Padding 右侧给 chevron 让位（Margin 6,0,0,0）。
const CHEVRON_SPACE: f32 = 18.0;

/// MetroDropDownButton —— 下拉按钮。参 CONTROL_SPEC §17。
#[derive(Debug, Clone)]
pub struct MetroDropDownButton {
    pub label: String,
    pub state: ControlState,
    /// 关联的下拉菜单（MenuFlyout）。
    pub menu: MetroDropdownMenu,
}

impl MetroDropDownButton {
    pub fn new(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            state: ControlState::Normal,
            menu: MetroDropdownMenu::new(items),
        }
    }

    /// 固有尺寸：文本宽 + 左右 padding（8）+ chevron 区（18）。
    pub fn measure(&self, engine: &TextEngine, style: TextStyle) -> kanesumi_core::Size {
        let width = engine.measure(&self.label, style.size) + 8.0 + 8.0 + CHEVRON_SPACE;
        let height = style.line_height + 11.0;
        kanesumi_core::Size::new(width, height)
    }

    /// 命中：整个按钮区。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 面板尺寸（转发 menu）。
    pub fn panel_size(&self, engine: &TextEngine) -> kanesumi_core::Size {
        self.menu.panel_size(engine)
    }

    pub fn is_flyout_open(&self) -> bool {
        matches!(
            self.menu.anim.state(),
            crate::popup::PopupState::Opening | crate::popup::PopupState::Open
        )
    }

    pub fn is_flyout_visible(&self) -> bool {
        self.menu.anim.is_visible()
    }

    pub fn update(&mut self, dt: f64) {
        self.menu.update(dt);
        // flyout 关闭动画走完后复位按钮态。
        if !self.menu.anim.is_visible() && self.state == ControlState::Pressed {
            self.state = ControlState::Normal;
        }
    }

    /// 打开/收起 flyout（面板位置 = place_popup 自动定向）。
    pub fn toggle(&mut self, engine: &TextEngine, rect: Rect, screen: Rect) {
        if self.is_flyout_open() {
            self.menu.close();
            self.state = ControlState::Normal;
        } else {
            let trigger = rect;
            let size = self.panel_size(engine);
            let placement = place_popup(trigger, size, screen, popup_gap());
            self.menu.open(placement.rect);
            self.state = ControlState::Pressed;
        }
    }

    /// 命中 flyout 项：仅 flyout 展开时有效，返回项索引。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if !self.is_flyout_open() {
            return None;
        }
        self.menu.item_at(pos)
    }

    /// 指针悬停路由：按钮 hover + flyout 项 hover。
    pub fn hover(&mut self, rect: Rect, pos: Point) -> bool {
        if self.is_flyout_visible() {
            if let Some(i) = self.menu.item_at(pos) {
                self.menu.hovered = Some(i);
                return true;
            }
            self.menu.hovered = None;
        }
        let on_button = self.hit_test(rect, pos);
        if on_button && self.state != ControlState::Pressed {
            self.state = ControlState::Hovered;
        } else if !on_button && self.state == ControlState::Hovered {
            self.state = ControlState::Normal;
        }
        on_button
    }

    /// 按下：按钮或 flyout 项。返回 true = 消费事件。
    pub fn press(&mut self, rect: Rect, pos: Point) -> bool {
        if self.is_flyout_open() {
            // flyout 打开：面板内交给 release，面板外关闭。
            if self.menu.item_at(pos).is_some() {
                return true;
            }
            if !self.hit_test(rect, pos) {
                self.menu.close();
                self.state = ControlState::Normal;
                return true;
            }
        }
        if self.hit_test(rect, pos) {
            self.state = ControlState::Pressed;
            return true;
        }
        false
    }

    /// 释放：按钮 toggle；flyout 项返回 (item_idx)。
    pub fn release(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        pos: Point,
    ) -> Option<usize> {
        // flyout 项
        if self.is_flyout_open()
            && let Some(i) = self.menu.item_at(pos)
        {
            self.menu.close();
            self.state = ControlState::Normal;
            return Some(i);
        }
        // 按钮
        if self.state == ControlState::Pressed && self.hit_test(rect, pos) {
            self.toggle(engine, rect, screen);
        } else if self.state == ControlState::Pressed {
            self.state = ControlState::Normal;
        }
        None
    }

    /// 渲染按钮（Standard）+ flyout（若开）。
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

        // 底
        let bg = if self.state == ControlState::Disabled {
            colors
                .surface
                .with_alpha(colors.surface.a * indication.disabled_opacity)
        } else {
            colors.surface
        };
        scene.fill_rounded_rect(bg, rect, theme.tokens.corner_radius);

        match self.state {
            ControlState::Hovered => scene.fill_rect(indication.hover_tint, rect),
            ControlState::Pressed => scene.fill_rect(indication.press_tint, rect),
            _ => {}
        }

        if self.state == ControlState::Focused {
            scene.stroke_rounded_rect(
                indication.focus_stroke,
                rect,
                2.0,
                theme.tokens.corner_radius,
            );
        }

        // 标签（左侧，留出 chevron 区）
        let label_w = engine.measure(&self.label, style.size);
        let text_rect = Rect::new(
            rect.origin.x + 8.0,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0,
            label_w,
            style.line_height,
        );
        scene.text(
            self.label.clone(),
            text_rect,
            colors.on_surface,
            style,
            TextAlign::Left,
        );

        // Chevron（右侧，Margin 6,0,0,0，12px）
        let chevron = Rect::new(
            rect.right() - 8.0 - 12.0,
            rect.origin.y + (rect.size.height - 12.0) / 2.0,
            12.0,
            12.0,
        );
        glyph::chevron_down(scene, chevron, colors.on_surface_variant);

        // Flyout
        if self.menu.anim.is_visible() {
            self.menu.render(theme, engine, screen, scene);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

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

    fn btn() -> MetroDropDownButton {
        MetroDropDownButton::new(
            "保存",
            vec![MenuItem::new("另存为…"), MenuItem::new("导出…")],
        )
    }

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 120.0, 36.0)
    }

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    #[test]
    fn measure_reserves_chevron() {
        let Some(engine) = find_engine() else { return };
        let style = MetroTheme::ether_dark().typography.body;
        let b = btn();
        let w = b.measure(&engine, style).width;
        let label_w = engine.measure(&b.label, style.size);
        assert!(
            w > label_w + 16.0 + 8.0,
            "measure 应含 padding + chevron 区"
        );
    }

    #[test]
    fn toggle_opens_flyout() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        assert!(!b.is_flyout_open());
        b.toggle(&engine, rect(), screen());
        assert!(b.is_flyout_open());
        assert_eq!(b.state, ControlState::Pressed, "打开时呈 Pressed");
        b.toggle(&engine, rect(), screen());
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        assert!(!b.is_flyout_open());
        assert_eq!(b.state, ControlState::Normal);
    }

    #[test]
    fn release_on_button_toggles() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        let p = rect().center();
        b.press(rect(), p);
        let hit = b.release(&engine, rect(), screen(), p);
        assert_eq!(hit, None);
        assert!(b.is_flyout_open());
        b.press(rect(), p);
        let hit = b.release(&engine, rect(), screen(), p);
        assert_eq!(hit, None);
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        assert!(!b.is_flyout_open(), "再点关闭");
    }

    #[test]
    fn release_on_item_returns_index() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        b.toggle(&engine, rect(), screen());
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        let panel = b.menu.panel_rect;
        let item_p = Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0);
        let hit = b.release(&engine, rect(), screen(), item_p);
        assert_eq!(hit, Some(0));
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        assert!(!b.is_flyout_open());
    }

    #[test]
    fn click_outside_closes() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        b.toggle(&engine, rect(), screen());
        let outside = Point::new(700.0, 500.0);
        let consumed = b.press(rect(), outside);
        assert!(consumed, "flyout 开时点外部应消费并关闭");
        assert!(!b.is_flyout_open());
    }

    #[test]
    fn hover_tracks_button_and_item() {
        let Some(engine) = find_engine() else { return };
        let mut b = btn();
        b.hover(rect(), Point::new(10.0, 18.0));
        assert_eq!(b.state, ControlState::Hovered);
        b.hover(rect(), Point::new(500.0, 500.0));
        assert_eq!(b.state, ControlState::Normal);
        // flyout 内 hover → 项 hover
        b.toggle(&engine, rect(), screen());
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        let panel = b.menu.panel_rect;
        b.hover(
            rect(),
            Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0),
        );
        assert_eq!(b.menu.hovered, Some(0));
    }

    #[test]
    fn render_emits_button_and_flyout() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut b = btn();
        b.toggle(&engine, rect(), screen());
        for _ in 0..120 {
            b.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        b.render(&theme, &engine, rect(), screen(), &mut scene);
        // 遮罩 + 面板底 + 边框 + 按钮底 + 2 项文本 + 按钮文本
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 1, "按钮 chevron 三角");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 3, "按钮标签 + flyout 项，实际 {texts}");
    }

    #[test]
    fn disabled_lowers_alpha() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut b = btn();
        b.state = ControlState::Disabled;
        let mut scene = Scene::default();
        b.render(&theme, &engine, rect(), screen(), &mut scene);
        let Some(SceneCommand::FillRect { color, .. }) = scene.commands.first() else {
            panic!("首命令应为按钮底色");
        };
        assert!(color.a < 1.0, "禁用降 alpha");
    }
}
