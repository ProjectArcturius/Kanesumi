use kanesumi_canvas::glyph;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect, TextStyle};

use crate::popup::{PopupAnim, PopupState, render_overlay};

/// MetroSelectorFlyout —— 下拉选择器（ComboBox 参考）。参 CONTROL_SPEC §8：
/// - 触发器 MinHeight 32、箭头区右 32px、glyph `E70D` 12px；
/// - 面板 MaxDropDownHeight 504（或 15 项）；选中项强调色低透，悬停中性；
/// - 遮罩 0.383s 入 / 0.216s 出，面板 `sheet_appear` 展开。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSelectorFlyout {
    pub items: Vec<String>,
    pub selected: Option<usize>,
    pub hovered: Option<usize>,
    /// 触发器聚焦。
    pub focused: bool,
    /// 触发器内标题文本（占位，未选时显示）。
    pub placeholder: String,
    pub anim: PopupAnim,
    /// 下拉面板矩形（Phase 3 续做方向自适应：Top>0 向下展开，参 ComboBoxHelper）。
    pub panel_rect: Rect,
    /// 面板最大高。
    pub max_dropdown_height: f32,
}

impl Default for MetroSelectorFlyout {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            selected: None,
            hovered: None,
            focused: false,
            placeholder: String::new(),
            anim: PopupAnim::new(),
            panel_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            max_dropdown_height: 504.0,
        }
    }
}

impl MetroSelectorFlyout {
    pub fn new(items: Vec<String>) -> Self {
        Self {
            items,
            ..Self::default()
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

    /// 面板高度：min(项数×项高, max_dropdown_height)。
    pub fn panel_height(&self) -> f32 {
        let item_h = 32.0;
        (self.items.len() as f32 * item_h).min(self.max_dropdown_height)
    }

    /// 项命中测试（相对面板）。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.x < self.panel_rect.origin.x
            || pos.x >= self.panel_rect.origin.x + self.panel_rect.size.width
        {
            return None;
        }
        let local_y = pos.y - self.panel_rect.origin.y;
        if local_y < 0.0 || local_y >= self.panel_height() {
            return None;
        }
        Some((local_y / 32.0) as usize)
    }

    /// 渲染触发器 +（可见时）遮罩与面板。
    pub fn render(
        &self,
        theme: &MetroTheme,
        _engine: &TextEngine,
        trigger: Rect,
        screen: Rect,
        scene: &mut Scene,
    ) {
        // 触发器
        let colors = &theme.colors;
        let bg = if self.focused {
            colors.primary.with_alpha(0.24)
        } else {
            colors.surface
        };
        scene.fill_rounded_rect(bg, trigger, theme.tokens.corner_radius);
        if self.focused {
            scene.stroke_rounded_rect(colors.primary, trigger, 1.0, theme.tokens.corner_radius);
        }

        let style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
        let text = self
            .selected
            .and_then(|i| self.items.get(i))
            .cloned()
            .unwrap_or_else(|| self.placeholder.clone());
        let text_rect = Rect::new(
            trigger.origin.x + 12.0,
            trigger.origin.y + (trigger.size.height - style.line_height) / 2.0,
            trigger.size.width - 32.0 - 10.0,
            style.line_height,
        );
        let fg = if self.selected.is_some() {
            colors.on_surface
        } else {
            colors.on_surface_variant
        };
        scene.text(text, text_rect, fg, style, TextAlign::Left);

        // 箭头 —— Metro 自绘 chevron（不依赖 Fluent codepoint，参 V7）。
        let arrow_rect = Rect::new(
            trigger.origin.x + trigger.size.width - 22.0,
            trigger.origin.y + (trigger.size.height - 12.0) / 2.0,
            12.0,
            12.0,
        );
        glyph::chevron_down(scene, arrow_rect, colors.on_surface);

        // 弹层
        if !self.anim.is_visible() {
            return;
        }
        render_overlay(theme, &self.anim, screen, scene);
        crate::popup::render_panel_base(theme, self.panel_rect, self.anim.panel_progress(), scene);

        let mut y = self.panel_rect.origin.y;
        for (i, item) in self.items.iter().enumerate() {
            if y - self.panel_rect.origin.y >= self.panel_height() {
                break;
            }
            let item_rect = Rect::new(
                self.panel_rect.origin.x,
                y,
                self.panel_rect.size.width,
                32.0,
            );
            let selected = self.selected == Some(i);
            if selected {
                scene.fill_rect(colors.primary.with_alpha(0.24), item_rect);
            } else if self.hovered == Some(i) {
                scene.fill_rect(colors.on_surface.with_alpha(0.10), item_rect);
            }
            let fg = if selected {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let text_rect = Rect::new(
                self.panel_rect.origin.x + 11.0,
                y + (32.0 - style.line_height) / 2.0,
                self.panel_rect.size.width - 22.0,
                style.line_height,
            );
            scene.text(item.clone(), text_rect, fg, style, TextAlign::Left);
            y += 32.0;
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
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn toggle_opens_closes() {
        let mut sel = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        sel.toggle(Rect::new(0.0, 40.0, 160.0, 160.0));
        for _ in 0..120 {
            sel.update(1.0 / 60.0);
        }
        assert!(sel.anim.is_open());
        sel.toggle(Rect::new(0.0, 40.0, 160.0, 160.0));
        for _ in 0..120 {
            sel.update(1.0 / 60.0);
        }
        assert_eq!(sel.state(), PopupState::Closed);
    }

    #[test]
    fn panel_height_capped() {
        let many: Vec<String> = (0..30).map(|i| format!("Item {i}")).collect();
        let sel = MetroSelectorFlyout::new(many);
        assert_eq!(sel.panel_height(), 504.0, "30 项截断到 504");
        let few = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        assert_eq!(few.panel_height(), 64.0);
    }

    #[test]
    fn item_at_maps_rows() {
        let sel = MetroSelectorFlyout::new(vec!["A".into(), "B".into()]);
        let rect = Rect::new(10.0, 100.0, 160.0, 64.0);
        // 模拟 panel_rect
        let mut sel = sel;
        sel.panel_rect = rect;
        assert_eq!(sel.item_at(Point::new(20.0, 110.0)), Some(0));
        assert_eq!(sel.item_at(Point::new(20.0, 142.0)), Some(1));
        assert_eq!(sel.item_at(Point::new(20.0, 200.0)), None, "面板外");
    }

    #[test]
    fn renders_trigger_when_closed() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut sel = MetroSelectorFlyout::new(vec!["Alpha".into()]);
        sel.selected = Some(0);
        let mut scene = Scene::default();
        sel.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 160.0, 32.0),
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 触发器底 + 选中文本 + 自绘 chevron（关闭时无遮罩，无 Text 箭头）
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        let triangles = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(texts, 1, "只有选中文本，箭头改自绘");
        assert_eq!(triangles, 1, "chevron 是 Triangle");
    }
}
