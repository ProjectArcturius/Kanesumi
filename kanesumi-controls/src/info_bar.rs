// MetroInfoBar —— 信息横幅。参 CONTROL_SPEC §12。
//
// 移植自 microsoft-ui-xaml/dev/InfoBar（InfoBar.cpp + InfoBarPanel.cpp +
// InfoBar.xaml + InfoBar_themeresources.xaml）：
// - ContentRoot MinHeight 48、Padding 16,0,0,0、边框 1px；
// - [Icon] | InfoBarPanel（Title 14 SemiBold / Message 14 / Action）| [× 38×38]；
// - 横排/纵排判据（InfoBarPanel::MeasureOverride）：仅 1 项 / 超宽 / 单项超 48 → 纵排；
// - Severity 四色映射为深色纯色面板（铁律 6）。
// 无开合动画（模板 Collapsed 直接改 Visibility）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, FontWeight, MetroTheme, Point, Rect};

/// 严重级别。参 CONTROL_SPEC §12。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoBarSeverity {
    Informational,
    Success,
    Warning,
    Error,
}

impl InfoBarSeverity {
    /// 面板底色 + 图标方块色（Kanesumi 深色适配，参 CONTROL_SPEC §12 表）。
    fn colors(self) -> (Color, Color) {
        match self {
            InfoBarSeverity::Informational => {
                (Color::from_hex(0x1E_2A_38), Color::from_hex(0x4F_C1_FF))
            }
            InfoBarSeverity::Success => (Color::from_hex(0x1E_33_28), Color::from_hex(0x4C_C3_8A)),
            InfoBarSeverity::Warning => (Color::from_hex(0x33_2B_1E), Color::from_hex(0xE5_A9_4E)),
            InfoBarSeverity::Error => (Color::from_hex(0x33_1E_1E), Color::from_hex(0xE5_53_4A)),
        }
    }

    /// 默认图标字形。
    pub fn icon_glyph(self) -> &'static str {
        match self {
            InfoBarSeverity::Informational => "i",
            InfoBarSeverity::Success => "✓",
            InfoBarSeverity::Warning => "!",
            InfoBarSeverity::Error => "✕",
        }
    }
}

/// InfoBar 点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoBarClick {
    None,
    Close,
    Action,
}

/// 布局方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InfoBarLayout {
    Horizontal,
    Vertical,
}

/// 面板子项几何。
#[derive(Debug, Clone, Copy)]
struct PanelGeom {
    title: Rect,
    message: Rect,
    action: Rect,
    height: f32,
}

/// MetroInfoBar —— 信息横幅。参 CONTROL_SPEC §12。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroInfoBar {
    pub title: String,
    pub message: String,
    pub severity: InfoBarSeverity,
    /// 可见性（IsOpen）。
    pub open: bool,
    /// 是否可关闭（显示 ×）。
    pub closable: bool,
    /// 是否显示图标。
    pub is_icon_visible: bool,
    /// 操作按钮文本（可选）。
    pub action_label: Option<String>,
    /// 自定义图标字形（None = severity 默认）。
    pub icon_glyph: Option<String>,
    /// Close 按钮 hover 态。
    pub close_hovered: bool,
    /// Action 按钮 hover 态。
    pub action_hovered: bool,
}

impl Default for MetroInfoBar {
    fn default() -> Self {
        Self {
            title: String::new(),
            message: String::new(),
            severity: InfoBarSeverity::Informational,
            open: true,
            closable: true,
            is_icon_visible: true,
            action_label: None,
            icon_glyph: None,
            close_hovered: false,
            action_hovered: false,
        }
    }
}

impl MetroInfoBar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn error(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            message: message.into(),
            severity: InfoBarSeverity::Error,
            ..Self::default()
        }
    }

    /// 关闭横幅（IsOpen = false）。
    pub fn close(&mut self) {
        self.open = false;
    }

    /// 显示（IsOpen = true）。
    pub fn show(&mut self) {
        self.open = true;
    }

    /// Title 样式：14px SemiBold。
    fn title_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Semibold)
    }

    /// Message / Action 样式：14px Normal。
    fn body_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Normal)
    }

    /// Close 按钮 rect（38×38，Margin 5，右上）。
    pub fn close_rect(rect: Rect) -> Rect {
        Rect::new(rect.right() - 38.0 - 5.0, rect.origin.y + 5.0, 38.0, 38.0)
    }

    /// 图标 rect（16px，Margin 0,16,14,16，ContentRoot Padding 16 之后）。
    fn icon_rect(rect: Rect) -> Rect {
        Rect::new(rect.origin.x + 16.0, rect.origin.y + 16.0, 16.0, 16.0)
    }

    /// 内容可用宽度（除去 Padding 16 与 Close 区）。
    fn content_width(rect: Rect) -> f32 {
        (rect.size.width - 16.0 - 16.0 - 38.0 - 5.0 - 5.0).max(0.0)
    }

    /// 布局几何。返回 Horizontal/Vertical + 各子项 rect（相对 rect 原点）。
    fn layout(&self, engine: &TextEngine, rect: Rect) -> (InfoBarLayout, PanelGeom) {
        let avail_w = Self::content_width(rect);
        let icon_w = if self.is_icon_visible { 30.0 } else { 0.0 }; // 16 icon + 14 right margin
        let panel_w = (avail_w - icon_w).max(0.0);
        let title_w = engine.measure(&self.title, Self::title_style().size);
        let message_w = engine.measure(&self.message, Self::body_style().size);
        let action_w = self
            .action_label
            .as_ref()
            .map(|a| engine.measure(a, Self::body_style().size))
            .unwrap_or(0.0);

        // 横排：Title + 12 + Message + 16 + Action（仅当放得下）。
        let horizontal_total = title_w + 12.0 + message_w + 16.0 + action_w;
        let vertical = horizontal_total > panel_w || panel_w <= 0.0;

        let panel_x = rect.origin.x + 16.0 + icon_w;
        if vertical {
            // 纵排 Padding 0,14,0,18
            let y0 = rect.origin.y + 14.0;
            let title = Rect::new(panel_x, y0, panel_w, Self::title_style().line_height);
            let msg_y = y0 + Self::title_style().line_height + 4.0;
            let message = Rect::new(panel_x, msg_y, panel_w, Self::body_style().line_height);
            let action = if self.action_label.is_some() {
                let ay = msg_y + Self::body_style().line_height + 12.0;
                Rect::new(panel_x, ay, action_w, Self::body_style().line_height)
            } else {
                Rect::new(0.0, 0.0, 0.0, 0.0)
            };
            let height = if self.action_label.is_some() {
                action.bottom() - rect.origin.y + 18.0
            } else {
                message.bottom() - rect.origin.y + 18.0
            };
            (
                InfoBarLayout::Vertical,
                PanelGeom {
                    title,
                    message,
                    action,
                    height,
                },
            )
        } else {
            // 横排 Padding 0,0,0,0；Title top 14、Message left 12、Action left 16
            let y0 = rect.origin.y + 14.0;
            let title = Rect::new(panel_x, y0, title_w, Self::title_style().line_height);
            let msg_x = title.right() + 12.0;
            let message = Rect::new(msg_x, y0, message_w, Self::body_style().line_height);
            let action = if self.action_label.is_some() {
                Rect::new(
                    message.right() + 16.0,
                    rect.origin.y + 8.0,
                    action_w,
                    Self::body_style().line_height,
                )
            } else {
                Rect::new(0.0, 0.0, 0.0, 0.0)
            };
            (
                InfoBarLayout::Horizontal,
                PanelGeom {
                    title,
                    message,
                    action,
                    height: 48.0,
                },
            )
        }
    }

    /// 命中：Close / Action 按钮。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> InfoBarClick {
        if !self.open {
            return InfoBarClick::None;
        }
        if self.closable && Self::close_rect(rect).contains(pos) {
            return InfoBarClick::Close;
        }
        let (_, geom) = self.layout(engine, rect);
        if self.action_label.is_some() && geom.action.size.width > 0.0 && geom.action.contains(pos)
        {
            return InfoBarClick::Action;
        }
        InfoBarClick::None
    }

    /// 处理点击 —— 命中 Close 则关闭并返回对应事件。
    pub fn handle_click(&mut self, engine: &TextEngine, rect: Rect, pos: Point) -> InfoBarClick {
        let click = self.hit(engine, rect, pos);
        if click == InfoBarClick::Close {
            self.close();
        }
        click
    }

    /// 渲染整个横幅（含 ContentRoot 边框 + 背景 + 图标 + 文本 + Close）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        if !self.open {
            return;
        }
        let colors = &theme.colors;
        let (bg, icon_color) = self.severity.colors();
        let (_, geom) = self.layout(engine, rect);
        let content_w = Self::content_width(rect);
        let content_h = (rect.size.height).max(geom.height);
        let content = Rect::new(rect.origin.x, rect.origin.y, content_w, content_h);

        // ContentRoot 底 + 边框
        scene.fill_rect(bg, content);
        scene.stroke_rect(colors.divider, content, 1.0);

        // 图标：方块 + 白色字形
        if self.is_icon_visible {
            let icon_rect = Self::icon_rect(rect);
            scene.fill_rounded_rect(icon_color, icon_rect, kanesumi_core::CornerRadius::Slight);
            let glyph = self
                .icon_glyph
                .clone()
                .unwrap_or_else(|| self.severity.icon_glyph().to_string());
            let style = TextStyle::new(11.0, 16.0, FontWeight::Bold);
            scene.text(
                glyph,
                icon_rect,
                Color::from_hex(0xFF_FF_FF),
                style,
                TextAlign::Center,
            );
        }

        // Title
        scene.text(
            self.title.clone(),
            geom.title,
            colors.on_surface,
            Self::title_style(),
            TextAlign::Left,
        );
        // Message
        scene.text(
            self.message.clone(),
            geom.message,
            colors.on_surface,
            Self::body_style(),
            TextAlign::Left,
        );
        // Action
        if let Some(label) = &self.action_label {
            let action_rect = geom.action;
            if self.action_hovered {
                scene.fill_rect(colors.on_surface.with_alpha(0.10), action_rect);
            }
            scene.text(
                label.clone(),
                action_rect,
                colors.primary,
                Self::body_style(),
                TextAlign::Left,
            );
        }

        // Close 按钮
        if self.closable {
            let close = Self::close_rect(rect);
            if self.close_hovered {
                scene.fill_rounded_rect(
                    colors.on_surface.with_alpha(0.10),
                    close,
                    kanesumi_core::CornerRadius::Slight,
                );
            }
            // × 自绘：两条对角三角形对 → 用两条细 rect 近似
            let cx = close.center().x;
            let cy = close.center().y;
            let r = 6.0;
            let t = 1.6;
            // 斜线 → 画两个细矩形（近似），或用 4 个三角形。用 4 三角形画斜十字。
            // 简化：两条水平/垂直不成立，改用两条对角细条（StrokeRect 不支持旋转）——
            // 用两个 Triangle 各拼成对角长条。
            // 方案：对角「乘」号用 4 个三角形（每臂 1 个）—— 视觉上足够。
            let cross = Color::from_hex(0xE5_E5_E5);
            // 左上臂
            scene.triangle(
                Point::new(cx - r, cy - r + t),
                Point::new(cx - r + t, cy - r),
                Point::new(cx + r - t, cy + r),
                cross,
            );
            scene.triangle(
                Point::new(cx - r + t, cy - r),
                Point::new(cx + r, cy + r - t),
                Point::new(cx + r - t, cy + r),
                cross,
            );
            // 右上臂
            scene.triangle(
                Point::new(cx + r - t, cy - r),
                Point::new(cx + r, cy - r + t),
                Point::new(cx - r + t, cy + r),
                cross,
            );
            scene.triangle(
                Point::new(cx + r, cy - r + t),
                Point::new(cx + r - t, cy + r),
                Point::new(cx - r, cy + r - t),
                cross,
            );
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

    fn bar() -> MetroInfoBar {
        MetroInfoBar::error("连接失败", "请检查网络后重试")
    }

    #[test]
    fn default_severity_is_informational() {
        assert_eq!(MetroInfoBar::new().severity, InfoBarSeverity::Informational);
    }

    #[test]
    fn close_hides() {
        let mut b = MetroInfoBar::new();
        assert!(b.open);
        b.close();
        assert!(!b.open);
        b.show();
        assert!(b.open);
    }

    #[test]
    fn close_rect_is_top_right_38() {
        let r = Rect::new(0.0, 0.0, 400.0, 48.0);
        let c = MetroInfoBar::close_rect(r);
        assert_eq!(c.size.width, 38.0);
        assert_eq!(c.size.height, 38.0);
        assert!((c.right() - (400.0 - 5.0)).abs() < 0.01, "右缘距右 5px");
        assert_eq!(c.origin.y, 5.0);
    }

    #[test]
    fn wide_bar_uses_horizontal_layout() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        b.action_label = Some("重试".into());
        let r = Rect::new(0.0, 0.0, 600.0, 48.0);
        let (mode, geom) = b.layout(&engine, r);
        assert_eq!(mode, InfoBarLayout::Horizontal);
        assert!(geom.action.size.width > 0.0);
        // 横排 message 在 title 右侧
        assert!(geom.message.origin.x >= geom.title.right());
    }

    #[test]
    fn narrow_bar_uses_vertical_layout() {
        let Some(engine) = find_engine() else { return };
        let b = bar();
        let r = Rect::new(0.0, 0.0, 200.0, 120.0);
        let (mode, geom) = b.layout(&engine, r);
        assert_eq!(mode, InfoBarLayout::Vertical);
        // 纵排 message 在 title 下方
        assert!(geom.message.origin.y > geom.title.origin.y);
    }

    #[test]
    fn hit_close_and_action() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        b.action_label = Some("重试".into());
        let r = Rect::new(0.0, 0.0, 600.0, 48.0);
        let close = MetroInfoBar::close_rect(r);
        assert_eq!(
            b.hit(&engine, r, Point::new(close.center().x, close.center().y)),
            InfoBarClick::Close
        );
        let (_, geom) = b.layout(&engine, r);
        assert_eq!(
            b.hit(
                &engine,
                r,
                Point::new(geom.action.center().x, geom.action.center().y)
            ),
            InfoBarClick::Action
        );
        assert_eq!(
            b.hit(&engine, r, Point::new(100.0, 40.0)),
            InfoBarClick::None
        );
    }

    #[test]
    fn handle_click_close_closes() {
        let Some(engine) = find_engine() else { return };
        let mut b = bar();
        let r = Rect::new(0.0, 0.0, 600.0, 48.0);
        let close = MetroInfoBar::close_rect(r);
        let click = b.handle_click(&engine, r, Point::new(close.center().x, close.center().y));
        assert_eq!(click, InfoBarClick::Close);
        assert!(!b.open, "点 × 后关闭");
    }

    #[test]
    fn render_emits_commands_when_open() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let b = bar();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 600.0, 48.0),
            &mut scene,
        );
        assert!(!scene.is_empty());
        // 文本：title + message = 2
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 2, "title + message，实际 {texts}");
    }

    #[test]
    fn closed_bar_renders_nothing() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut b = bar();
        b.close();
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 600.0, 48.0),
            &mut scene,
        );
        assert!(scene.is_empty());
    }

    #[test]
    fn severity_colors_are_distinct() {
        let (bg1, _) = InfoBarSeverity::Error.colors();
        let (bg2, _) = InfoBarSeverity::Success.colors();
        assert_ne!(bg1, bg2);
    }

    #[test]
    fn closable_false_hides_close_button() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut b = bar();
        b.closable = false;
        let mut scene = Scene::default();
        b.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 600.0, 48.0),
            &mut scene,
        );
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(tris, 0, "不可关闭 → 无 × 三角形");
    }
}
