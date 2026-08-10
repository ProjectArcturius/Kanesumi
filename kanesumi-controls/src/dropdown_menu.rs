use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect, TextStyle};

use crate::popup::{PopupAnim, PopupState, render_overlay};

/// 菜单项。参 CONTROL_SPEC §8（MenuFlyout）：
/// - 项高 ≈32（Padding `11,9,11,10` + 14px 字）；图标 16px；快捷键右侧；
/// - PointerOver = 中性高亮，瞬时；分隔线高 1、左右 12 留白。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    /// Toggle 项勾选状态。
    pub checked: bool,
    /// 项后加分隔线。
    pub separator_after: bool,
    /// 嵌套子菜单（占位，Phase 3 续实现级联）。
    pub submenu: Vec<MenuItem>,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            checked: false,
            separator_after: false,
            submenu: Vec::new(),
        }
    }

    pub fn with_icon(label: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            icon: Some(icon.into()),
            ..Self::new(label)
        }
    }

    pub fn separator(mut self) -> Self {
        self.separator_after = true;
        self
    }
}

/// MetroDropdownMenu —— 下拉菜单（MenuFlyout 参考）。参 CONTROL_SPEC §8：
/// - 弹出 = 遮罩淡入 0.383s + 面板 0.30s 展开；
/// - 项高 32、图标 16、快捷键右对齐；PointerOver 中性高亮。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroDropdownMenu {
    pub items: Vec<MenuItem>,
    pub hovered: Option<usize>,
    /// 项高（UWP 32）。
    pub item_height: f32,
    pub anim: PopupAnim,
    /// 锚点（面板相对触发器的弹出位置，Phase 3 续做方向自适应）。
    pub panel_rect: Rect,
}

impl MetroDropdownMenu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            hovered: None,
            item_height: 32.0,
            anim: PopupAnim::new(),
            panel_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub fn open(&mut self, at: Rect) {
        self.panel_rect = at;
        self.anim.open();
    }

    pub fn close(&mut self) {
        self.anim.close();
    }

    pub fn toggle(&mut self, at: Rect) {
        if self.anim.is_open() {
            self.close();
        } else {
            self.open(at);
        }
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
    }

    pub fn state(&self) -> PopupState {
        self.anim.state()
    }

    /// 面板尺寸：宽 = 最宽项（含图标/快捷键占位）+ 边距，高 = 项数 × 项高 + 分隔线。
    pub fn panel_size(&self, engine: &TextEngine) -> kanesumi_core::Size {
        let style = menu_item_style();
        let text_w = self
            .items
            .iter()
            .map(|i| engine.measure(&i.label, style.size))
            .fold(0.0, f32::max);
        let icon_w = if self.items.iter().any(|i| i.icon.is_some()) {
            28.0
        } else {
            0.0
        };
        let shortcut_w = self
            .items
            .iter()
            .filter_map(|i| i.shortcut.as_ref())
            .map(|s| engine.measure(s, style.size))
            .fold(0.0, f32::max);
        let separators = self.items.iter().filter(|i| i.separator_after).count() as f32;
        let width = (icon_w + text_w + shortcut_w + 22.0 + 24.0).max(120.0);
        let height = self.items.len() as f32 * self.item_height + separators * 2.0;
        kanesumi_core::Size::new(width, height)
    }

    /// 项命中测试（相对面板原点，面板以 `panel_rect.origin` 起排）。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.x < self.panel_rect.origin.x
            || pos.x >= self.panel_rect.origin.x + self.panel_rect.size.width
        {
            return None;
        }
        let local_y = pos.y - self.panel_rect.origin.y;
        let mut y = 0.0;
        for (i, item) in self.items.iter().enumerate() {
            let h = self.item_height;
            if item.separator_after {
                // 分隔线 +2px 计入下一项起点
                y += h;
                if local_y >= y - 1.0 && local_y < y + 1.0 {
                    return None; // 分隔线本身不可命中
                }
            }
            if local_y >= y && local_y < y + h {
                return Some(i);
            }
            y += h;
        }
        None
    }

    /// 渲染：遮罩 + 面板（项 + 分隔线 + 悬停高亮）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene) {
        if !self.anim.is_visible() {
            return;
        }
        render_overlay(theme, &self.anim, screen, scene);
        crate::popup::render_panel_base(theme, self.panel_rect, self.anim.panel_progress(), scene);

        let style = menu_item_style();
        let colors = &theme.colors;
        let mut y = self.panel_rect.origin.y;
        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                self.panel_rect.origin.x,
                y,
                self.panel_rect.size.width,
                self.item_height,
            );

            // 悬停高亮（中性）
            if self.hovered == Some(i) {
                scene.fill_rect(colors.on_surface.with_alpha(0.10), item_rect);
            }

            let mut x = self.panel_rect.origin.x + 11.0;
            // 图标
            if let Some(icon) = &item.icon {
                let icon_rect = Rect::new(x, y + (self.item_height - 16.0) / 2.0, 16.0, 16.0);
                scene.text(
                    icon.clone(),
                    icon_rect,
                    colors.on_surface,
                    icon_style(),
                    TextAlign::Center,
                );
                x += 28.0;
            }
            // 文本 —— 宽度 = 面板右缘（含右内边距 11）到当前笔位。
            // 注意：panel_rect.size.width 是相对宽度，不能直接减 x（x 是绝对坐标）。
            // 旧 bug：`panel_rect.size.width - x` 得到巨大负值 → engine.layout 触发
            // CJK 单字硬断 → 菜单项变成字塔（每个字一行）。
            let text_right = self.panel_rect.right() - 11.0;
            let text_w = (text_right - x).max(0.0);
            let text_rect = Rect::new(
                x,
                y + (self.item_height - style.line_height) / 2.0,
                text_w,
                style.line_height,
            );
            scene.text(
                item.label.clone(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
            );
            // 快捷键（右侧）
            if let Some(sc) = &item.shortcut {
                let sc_w = engine.measure(sc, style.size);
                let sc_rect = Rect::new(
                    self.panel_rect.origin.x + self.panel_rect.size.width - sc_w - 24.0,
                    y + (self.item_height - style.line_height) / 2.0,
                    sc_w,
                    style.line_height,
                );
                scene.text(
                    sc.clone(),
                    sc_rect,
                    colors.on_surface_variant,
                    style,
                    TextAlign::Right,
                );
            }

            y += self.item_height;
            // 分隔线
            if item.separator_after {
                let sep_rect = Rect::new(
                    self.panel_rect.origin.x + 12.0,
                    y,
                    self.panel_rect.size.width - 24.0,
                    1.0,
                );
                scene.fill_rect(colors.divider, sep_rect);
                y += 2.0;
            }
        }
    }
}

/// 菜单项文本样式：14px 正常。
fn menu_item_style() -> TextStyle {
    TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal)
}

/// 菜单图标样式：16px。
fn icon_style() -> TextStyle {
    TextStyle::new(16.0, 16.0, kanesumi_core::FontWeight::Normal)
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

    #[test]
    fn toggle_open_close() {
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("Reset")]);
        menu.toggle(Rect::new(0.0, 0.0, 120.0, 32.0));
        assert!(menu.anim.is_visible());
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert!(menu.anim.is_open());
        menu.toggle(Rect::new(0.0, 0.0, 120.0, 32.0));
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert_eq!(menu.state(), PopupState::Closed);
    }

    #[test]
    fn item_at_maps_y() {
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("A"), MenuItem::new("B")]);
        menu.open(Rect::new(10.0, 10.0, 120.0, 32.0));
        assert_eq!(menu.item_at(Point::new(20.0, 12.0)), Some(0));
        assert_eq!(menu.item_at(Point::new(20.0, 12.0 + 32.0)), Some(1));
        assert_eq!(menu.item_at(Point::new(5.0, 12.0)), None, "面板外");
    }

    #[test]
    fn panel_size_grows_with_items() {
        let Some(engine) = find_engine() else { return };
        let one = MetroDropdownMenu::new(vec![MenuItem::new("A")]);
        let two = MetroDropdownMenu::new(vec![MenuItem::new("A"), MenuItem::new("B")]);
        assert!(
            two.panel_size(&engine).height > one.panel_size(&engine).height,
            "项越多越高"
        );
    }

    #[test]
    fn renders_panel_when_open() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("Reset")]);
        menu.open(Rect::new(10.0, 10.0, 120.0, 32.0));
        menu.update(1.0);
        let mut scene = Scene::default();
        menu.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 遮罩 + 面板底 + 边框 + 项文本
        assert!(scene.commands.len() >= 4);
    }

    /// 回归 V1：文本 rect 宽度不可为负，且必须完全落在面板内。
    /// 曾经 bug：`panel_rect.size.width - x`（x 绝对坐标）→ 巨大负值 →
    /// `TextEngine::layout(max_width=负)` 触发 CJK 单字硬断 → 菜单碎字塔。
    #[test]
    fn item_text_rect_fits_panel() {
        use kanesumi_canvas::SceneCommand;
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let panel_at = Rect::new(600.0, 300.0, 140.0, 96.0);
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::with_icon("新建", "★"),
            MenuItem::with_icon("打开", "★"),
        ]);
        menu.open(panel_at);
        menu.update(1.0);
        let mut scene = Scene::default();
        menu.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 1024.0, 768.0),
            &mut scene,
        );
        // 每条 Text 命令的 rect 必须宽度 > 0 且 (x + width) <= panel_rect.right()。
        let text_rects: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        assert!(!text_rects.is_empty(), "菜单项应产出 Text 命令");
        for r in &text_rects {
            assert!(
                r.size.width > 0.0,
                "text_rect 宽必须正，实际 {}",
                r.size.width
            );
            assert!(
                r.right() <= panel_at.right() + 0.01,
                "text_rect 右缘越出面板：right={} panel.right={}",
                r.right(),
                panel_at.right()
            );
        }
    }
}
