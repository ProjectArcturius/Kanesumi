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

        // 底
        scene.fill_rounded_rect(colors.surface, rect, theme.tokens.corner_radius);
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

        // Primary 标签
        let label_w = engine.measure(&self.label, style.size);
        scene.text(
            self.label.clone(),
            Rect::new(
                rect.origin.x + 8.0,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                label_w,
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
