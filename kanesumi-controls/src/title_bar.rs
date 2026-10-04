// MetroTitleBar —— 应用 SSD 标题栏。参 CONTROL_SPEC §23。
//
// 移植自 microsoft-ui-xaml/dev/TitleBar（TitleBar.cpp + TitleBar.xaml + TitleBar_themeresources.xaml）：
// - Compact 高 32 / Expanded 高 48；
// - Back 按钮 44×H（glyph E72B → 自绘 chevron_left 16px）；Icon 16px（Margin 4,0,0,0）；
// - Title Caption 12px（Margin 16,0,16,2，MinWidth 48）；
// - Activated = on_surface；Deactivated = on_surface_variant；Back hover/press 中性 tint。

use kanesumi_canvas::glyph;
use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::{body_weight, TextStyle};
use kanesumi_core::{MetroTheme, Point, Rect};

/// Compact 高度（TitleBarCompactHeight = 32）。
pub const TITLEBAR_COMPACT: f32 = 32.0;
/// Expanded 高度（TitleBarExpandedHeight = 48）。
pub const TITLEBAR_EXPANDED: f32 = 48.0;
/// Back 按钮宽（44）。
pub const TITLEBAR_BACK_WIDTH: f32 = 44.0;

/// TitleBar 点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBarClick {
    None,
    Back,
}

/// MetroTitleBar —— 应用标题栏。参 CONTROL_SPEC §23。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTitleBar {
    pub title: String,
    /// 应用图标（16px 位图）。
    pub icon: Option<Icon>,
    /// 是否显示 Back 按钮。
    pub back_enabled: bool,
    /// 窗口激活态（Deactivated 转暗）。
    pub activated: bool,
    /// Expanded 高（48）vs Compact 高（32）。
    pub expanded: bool,
    /// 右侧自定义内容区宽（宿主注入）。
    pub custom_content_width: f32,
    pub back_hovered: bool,
    pub back_pressed: bool,
}

impl Default for MetroTitleBar {
    fn default() -> Self {
        Self {
            title: String::new(),
            icon: None,
            back_enabled: false,
            activated: true,
            expanded: false,
            custom_content_width: 0.0,
            back_hovered: false,
            back_pressed: false,
        }
    }
}

impl MetroTitleBar {
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前高度。
    pub fn height(&self) -> f32 {
        if self.expanded {
            TITLEBAR_EXPANDED
        } else {
            TITLEBAR_COMPACT
        }
    }

    /// Back 按钮 rect（44×H，左缘）。
    pub fn back_rect(&self, rect: Rect) -> Option<Rect> {
        if !self.back_enabled {
            return None;
        }
        Some(Rect::new(
            rect.origin.x,
            rect.origin.y,
            TITLEBAR_BACK_WIDTH,
            rect.size.height,
        ))
    }

    /// Icon rect（16px；Back 隐藏时 Margin 16,0,0,0）。
    pub fn icon_rect(&self, rect: Rect) -> Option<Rect> {
        let _ = self.icon.as_ref()?;
        let x = if self.back_enabled {
            rect.origin.x + TITLEBAR_BACK_WIDTH + 4.0
        } else {
            rect.origin.x + 16.0
        };
        let h = self.height();
        Some(Rect::new(x, rect.origin.y + (h - 16.0) / 2.0, 16.0, 16.0))
    }

    /// Title rect（Icon 之后，右侧留给 custom content）。
    pub fn title_rect(&self, rect: Rect) -> Rect {
        let h = self.height();
        let x = self
            .icon_rect(rect)
            .map(|i| i.right() + 16.0)
            .unwrap_or_else(|| {
                if self.back_enabled {
                    rect.origin.x + TITLEBAR_BACK_WIDTH + 16.0
                } else {
                    rect.origin.x + 16.0
                }
            });
        let w =
            (rect.size.width - (x - rect.origin.x) - self.custom_content_width - 16.0).max(48.0);
        Rect::new(x, rect.origin.y + (h - 14.0) / 2.0, w, 14.0)
    }

    /// Title 样式（Caption 12px）。无同字号令牌（12/14），只取正文字重（G-67）。
    pub fn title_style() -> TextStyle {
        TextStyle::new(12.0, 14.0, body_weight())
    }

    /// 命中：Back 按钮。
    pub fn hit(&self, rect: Rect, pos: Point) -> TitleBarClick {
        if let Some(b) = self.back_rect(rect)
            && b.contains(pos)
        {
            return TitleBarClick::Back;
        }
        TitleBarClick::None
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.back_hovered = self.hit(rect, pos) == TitleBarClick::Back && !self.back_pressed;
    }

    /// 处理点击。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> TitleBarClick {
        self.hover(rect, pos);
        if self.back_pressed && self.hit(rect, pos) == TitleBarClick::Back {
            self.back_pressed = false;
            TitleBarClick::Back
        } else {
            TitleBarClick::None
        }
    }

    /// 渲染：Back chevron + Icon + Title。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let fg = if self.activated {
            colors.on_surface
        } else {
            colors.on_surface_variant
        };

        // Back
        if let Some(b) = self.back_rect(rect) {
            // 底：悬停 / 按压 = 透明底控件的通用档（UWP `AppBarButtonBackgroundPointerOver/Pressed`
            // → `SystemControlHighlightListLow/MediumBrush` = 10% / 20%，themeresources L1680/1681）。
            // 旧值 15% / 25% 出自 `CONTROL_SPEC` §790/§791 的「SubtleFillColorSecondary/Tertiary
            // = 白 15% / 25%」——一手源证明笔刷名对、百分比错（真值 5.9% / 3.9%），
            // 且这里的返回键本就是透明底按钮，与 AppBarButton 同族。
            let bg = if self.back_pressed {
                theme.indication.press_tint
            } else if self.back_hovered {
                theme.indication.hover_tint
            } else {
                kanesumi_core::Color::TRANSPARENT
            };
            if bg.a > 0.0 {
                scene.fill_rect(bg, b);
            }
            let chevron = Rect::new(
                b.origin.x + (b.size.width - 16.0) / 2.0,
                b.origin.y + (b.size.height - 16.0) / 2.0,
                16.0,
                16.0,
            );
            glyph::chevron_left(scene, chevron, fg);
        }

        // Icon
        if let Some(img) = &self.icon
            && let Some(ir) = self.icon_rect(rect)
        {
            scene.image(img, ir, None);
        }

        // Title
        let tr = self.title_rect(rect);
        scene.text(
            self.title.clone(),
            tr,
            fg,
            Self::title_style(),
            TextAlign::Left,
        );
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3）─────────────────────────────────
//
// 应用标题栏：Back 按钮是唯一可交互部件。盘点：旧 `TitleBarClick` 只有 `Back` 一种。
// 指针按 `PointerDown` 置按压、`PointerUp` 调旧 `handle_click` 判定并发动作；悬停走旧 `hover`。
// `title_rect` 的 `.max(48.0)` 在极窄 rect 会越出右缘，按 §6 在 `paint` 里把自身绘制夹进 rect。

/// 元素树动作：标题栏按钮被激活。携带旧的 `TitleBarClick` 身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TitleBarAction(pub TitleBarClick);

impl kanesumi_element::Widget for MetroTitleBar {
    /// 标题栏按宿主给宽铺满；无界轴退回 Compact 默认宽，避免 NaN / 0 宽。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let w = if available.width.is_finite() {
            available.width
        } else {
            320.0
        };
        kanesumi_core::Size::new(w, self.height())
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // §6 修正：极窄 rect 下 `title_rect` 的 MinWidth 48 会越出右缘；成对把自身绘制夹进 rect。
        scene.push_clip(ctx.rect());
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        scene.pop_clip();
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                self.hover(ctx.rect(), *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                let changed = self.back_hovered;
                self.back_hovered = false;
                if changed {
                    ctx.invalidate_paint();
                }
            }
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let on_back = self.hit(ctx.rect(), *pos) == TitleBarClick::Back;
                if on_back {
                    self.back_pressed = true;
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let click = self.handle_click(ctx.rect(), *pos);
                if click != TitleBarClick::None {
                    ctx.emit(TitleBarAction(click));
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            Event::KeyDown {
                key: Key::Enter | Key::Char(' '),
                ..
            } if self.back_enabled => {
                ctx.emit(TitleBarAction(TitleBarClick::Back));
                ctx.set_handled();
            }
            _ => {}
        }
    }

    /// 只有存在 Back 按钮时才占 Tab 位。
    fn focusable(&self) -> bool {
        self.back_enabled
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: if self.title.is_empty() {
                String::from("标题栏")
            } else {
                self.title.clone()
            },
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(back: bool, props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(640.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroTitleBar {
                title: "Ether 设置".into(),
                back_enabled: back,
                ..MetroTitleBar::default()
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_back_emits_action() {
        let (mut h, id) = harness(true, LayoutProps::default());
        let back = {
            let b = h.tree.get::<MetroTitleBar>(id).unwrap();
            b.back_rect(h.rect(id)).unwrap().center()
        };
        h.click_at(back);
        assert_eq!(
            h.take::<TitleBarAction>(),
            vec![(id, TitleBarAction(TitleBarClick::Back))]
        );
    }

    #[test]
    fn click_title_area_emits_nothing() {
        let (mut h, id) = harness(true, LayoutProps::default());
        let r = h.rect(id);
        h.click_at(Point::new(r.right() - 20.0, r.center().y));
        assert!(h.take::<TitleBarAction>().is_empty());
    }

    #[test]
    fn keyboard_activates_back_when_enabled() {
        let (mut h, id) = harness(true, LayoutProps::default());
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Enter);
        assert_eq!(
            h.take::<TitleBarAction>(),
            vec![(id, TitleBarAction(TitleBarClick::Back))]
        );
    }

    #[test]
    fn without_back_is_not_focusable() {
        let (mut h, id) = harness(false, LayoutProps::default());
        h.tab();
        assert_ne!(h.tree.focused(), Some(id));
    }

    #[test]
    fn passes_insurance_checks_including_squeezed() {
        let (mut h, id) = harness(true, LayoutProps::default());
        h.move_to(h.center(id));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);

        // 极窄 rect：`title_rect` 的 MinWidth 48 会越出右缘，必须裁在 rect 内。
        let narrow = h.tree.insert_with(
            h.root(),
            MetroTitleBar {
                title: "很长很长的标题文本用于压窄测试".into(),
                back_enabled: true,
                ..MetroTitleBar::default()
            },
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(narrow);
        h.assert_paint_within(narrow, Insets::ZERO);
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

    fn bar() -> MetroTitleBar {
        MetroTitleBar {
            title: "Ether 设置".into(),
            back_enabled: true,
            ..MetroTitleBar::default()
        }
    }

    fn rect() -> Rect {
        Rect::new(0.0, 0.0, 640.0, TITLEBAR_COMPACT)
    }

    #[test]
    fn heights() {
        assert_eq!(MetroTitleBar::default().height(), 32.0);
        assert_eq!(
            MetroTitleBar {
                expanded: true,
                ..MetroTitleBar::default()
            }
            .height(),
            48.0
        );
    }

    #[test]
    fn back_rect_when_enabled() {
        let b = bar();
        let r = rect();
        let br = b.back_rect(r).unwrap();
        assert_eq!(br.size.width, 44.0);
        assert_eq!(br.size.height, 32.0);
        assert_eq!(MetroTitleBar::default().back_rect(r), None, "禁用时无 Back");
    }

    #[test]
    fn icon_rect_offsets() {
        let r = rect();
        // 无 icon
        assert_eq!(bar().icon_rect(r), None);
        // 有 icon + back
        let b = MetroTitleBar {
            icon: Some(Icon::default()),
            back_enabled: true,
            ..MetroTitleBar::default()
        };
        let ir = b.icon_rect(r).unwrap();
        assert_eq!(ir.origin.x, 44.0 + 4.0, "Back 后 Margin 4");
        // 有 icon 无 back → Margin 16
        let b2 = MetroTitleBar {
            icon: Some(Icon::default()),
            back_enabled: false,
            ..MetroTitleBar::default()
        };
        assert_eq!(b2.icon_rect(r).unwrap().origin.x, 16.0);
    }

    #[test]
    fn title_rect_after_icon() {
        let r = rect();
        let b = bar();
        let tr = b.title_rect(r);
        assert!(tr.origin.x >= 44.0 + 16.0, "Back 后 Title Margin 16");
        // 右侧留 custom content
        let b2 = MetroTitleBar {
            custom_content_width: 100.0,
            ..bar()
        };
        let tr2 = b2.title_rect(r);
        assert!(tr2.right() <= r.right() - 100.0 - 16.0 + 0.01);
    }

    #[test]
    fn hit_back_only() {
        let b = bar();
        let r = rect();
        assert_eq!(b.hit(r, Point::new(10.0, 16.0)), TitleBarClick::Back);
        assert_eq!(b.hit(r, Point::new(200.0, 16.0)), TitleBarClick::None);
    }

    #[test]
    fn deactivated_dims_foreground() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b2 = MetroTitleBar {
            activated: false,
            ..bar()
        };
        let mut scene = Scene::default();
        b2.render(&theme, &engine, rect(), &mut scene);
        let Some(SceneCommand::Text { color, .. }) = scene
            .commands
            .iter()
            .find(|c| matches!(c, SceneCommand::Text { .. }))
        else {
            panic!("应有标题文本");
        };
        assert_eq!(*color, theme.colors.on_surface_variant, "Deactivated 转暗");
    }

    #[test]
    fn back_hover_emits_tint() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut b = bar();
        b.back_hovered = true;
        let mut scene = Scene::default();
        b.render(&theme, &engine, rect(), &mut scene);
        // Back 底 tint（半透明 FillRect）
        let has_tint = scene.commands.iter().any(
            |c| matches!(c, SceneCommand::FillRect { color, .. } if color.a > 0.0 && color.a < 1.0),
        );
        assert!(has_tint);
    }
}
