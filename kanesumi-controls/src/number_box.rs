// MetroNumberBox —— 数字输入框。参 CONTROL_SPEC §37（NumberBox 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/NumberBox/`（NumberBox.cpp + NumberBox.xaml）：
// - 本质 = TextBox + 右侧两个 SpinButton（上/下，MinWidth 32）+ 校验；
// - SpinButtonsVisible 时 `SpinButtonsColumn.Width = 72`（两按钮 + 分隔），
//   Compact 模式两按钮并排；Default 模式 = Compact（xaml 默认 `SpinButtonPlacementMode=Compact`）；
// - MinWidth 120（NumberBoxMinWidth）；
// - 上/下箭头：RepeatButton 语义（按住重复），Kanesumi 自绘 chevron（V7）；
// - Value 变化 = `SmallChange` 步进，夹紧到 [Minimum, Maximum]；越界输入 clamp；
// - 无界时 Minimum/Maximum = ±∞（UWP 默认 NaN）。
//
// Kanesumi 实现：编辑核心复用 `TextField`（数字过滤），Value 提交语义 = 聚焦失焦/Enter。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::state::ControlState;
use crate::text_box::MetroTextBox;
use crate::text_field::{TextInputKey, TextField};

/// SpinButton 区宽（SpinButtonsColumn = 72）。
pub const NUMBERBOX_SPIN_COLUMN_W: f32 = 72.0;
/// 单个 SpinButton 最小宽（NumberBoxSpinButtonStyle MinWidth 32）。
pub const NUMBERBOX_SPIN_BUTTON_W: f32 = 32.0;
/// 控件 MinWidth（NumberBoxMinWidth 120）。
pub const NUMBERBOX_MIN_WIDTH: f32 = 120.0;
/// 分隔线厚度（NumberBoxSpinButtonBorderThickness 0,1,1,1 → 上/下按钮间竖线 1px）。
pub const NUMBERBOX_SEPARATOR: f32 = 1.0;

/// SpinButton 摆放模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinPlacement {
    /// 并排显示（默认，72 宽两按钮）。
    Compact,
    /// 折叠：仅弹层触发（Popup 模式）。Kanesumi 首期实现 Compact。
    Collapsed,
}

/// MetroNumberBox —— 数字输入。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroNumberBox {
    /// 编辑核心（数字内容）。
    pub field: TextField,
    /// 顶部标题（可选）。
    pub header: String,
    /// 最小值（None = 无界）。
    pub minimum: Option<f64>,
    /// 最大值（None = 无界）。
    pub maximum: Option<f64>,
    /// 步进（UWP SmallChange 默认 1）。
    pub small_change: f64,
    /// 控件交互状态。
    pub state: ControlState,
    /// 是否聚焦。
    pub focused: bool,
    /// SpinButton 摆放（首期仅 Compact）。
    pub placement: SpinPlacement,
    /// 上按钮悬停。
    pub up_hovered: bool,
    /// 下按钮悬停。
    pub down_hovered: bool,
}

impl Default for MetroNumberBox {
    fn default() -> Self {
        Self {
            field: TextField::new(),
            header: String::new(),
            minimum: None,
            maximum: None,
            small_change: 1.0,
            state: ControlState::Normal,
            focused: false,
            placement: SpinPlacement::Compact,
            up_hovered: false,
            down_hovered: false,
        }
    }
}

impl MetroNumberBox {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带标题构造。
    pub fn with_header(text: impl Into<String>) -> Self {
        Self {
            header: text.into(),
            ..Self::default()
        }
    }

    /// 设置数值（夹紧到 [min, max]），同步文本。
    pub fn set_value(&mut self, v: f64) {
        let v = self.clamp(v);
        self.field.set_text(self.format(v));
    }

    /// 当前数值（解析失败/空 → None）。
    pub fn value(&self) -> Option<f64> {
        let t = self.field.text();
        if t.trim().is_empty() {
            return None;
        }
        t.trim().parse::<f64>().ok().map(|v| self.clamp(v))
    }

    /// 步进限制。
    pub fn with_min(mut self, min: f64) -> Self {
        self.minimum = Some(min);
        if let Some(v) = self.value() {
            self.set_value(v);
        }
        self
    }

    pub fn with_max(mut self, max: f64) -> Self {
        self.maximum = Some(max);
        if let Some(v) = self.value() {
            self.set_value(v);
        }
        self
    }

    pub fn with_step(mut self, step: f64) -> Self {
        self.small_change = step;
        self
    }

    /// 夹紧到 [min, max]。
    fn clamp(&self, v: f64) -> f64 {
        let mut v = v;
        if let Some(min) = self.minimum {
            v = v.max(min);
        }
        if let Some(max) = self.maximum {
            v = v.min(max);
        }
        v
    }

    /// 数字格式化（整数去小数点）。
    fn format(&self, v: f64) -> String {
        if v.fract() == 0.0 {
            format!("{}", v as i64)
        } else {
            format!("{v}")
        }
    }

    /// 上一步。
    pub fn step_up(&mut self) -> Option<f64> {
        let cur = self.value().unwrap_or(0.0);
        let next = self.clamp(cur + self.small_change);
        self.set_value(next);
        Some(next)
    }

    /// 下一步。
    pub fn step_down(&mut self) -> Option<f64> {
        let cur = self.value().unwrap_or(0.0);
        let next = self.clamp(cur - self.small_change);
        self.set_value(next);
        Some(next)
    }

    /// 聚焦进入（全选，UWP TextBox 行为）。
    pub fn focus(&mut self) {
        self.focused = true;
        self.state = ControlState::Focused;
        self.field.select_all();
    }

    /// 失焦（提交文本 → clamp 回值域）。
    pub fn blur(&mut self) {
        self.focused = false;
        self.state = ControlState::Normal;
        if let Some(v) = self.value() {
            self.set_value(v);
        }
    }

    /// 处理编辑键（数字/控制键）。非数字字符被拒。
    pub fn handle_key(&mut self, key: TextInputKey) -> bool {
        // 数字输入过滤：只允许数字、小数点、负号（开头）、控制键
        match key {
            TextInputKey::Char(c) => {
                let cur = self.field.text();
                let allowed = c.is_ascii_digit()
                    || (c == '.' && !cur.contains('.'))
                    || (c == '-' && cur.is_empty());
                if !allowed {
                    return false;
                }
                self.field.insert_char(c)
            }
            TextInputKey::Enter => {
                if let Some(v) = self.value() {
                    self.set_value(v);
                }
                true
            }
            _ => self.field.handle_key(key),
        }
    }

    // ── 几何 ──────────────────────────────────────────────────

    /// 主体（Header 之下）。复用 TextBox 布局（正文 + 边框）。
    fn body_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let style = theme.typography.body;
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            style.line_height + 4.0
        };
        Rect::new(
            rect.origin.x,
            rect.origin.y + header_h,
            rect.size.width,
            (rect.size.height - header_h).max(0.0),
        )
    }

    /// 文本编辑区（主体左侧，扣除 Spin 区）。
    pub fn text_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let body = self.body_rect(theme, rect);
        let spin_w = if self.placement == SpinPlacement::Compact {
            NUMBERBOX_SPIN_COLUMN_W
        } else {
            0.0
        };
        Rect::new(
            body.origin.x,
            body.origin.y,
            (body.size.width - spin_w).max(0.0),
            body.size.height,
        )
    }

    /// 上按钮矩形（右半）。
    pub fn up_button_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let body = self.body_rect(theme, rect);
        let w = NUMBERBOX_SPIN_BUTTON_W;
        Rect::new(body.right() - w, body.origin.y, w, body.size.height)
    }

    /// 下按钮矩形（左半）。
    pub fn down_button_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let body = self.body_rect(theme, rect);
        let w = NUMBERBOX_SPIN_BUTTON_W;
        Rect::new(
            body.right() - NUMBERBOX_SPIN_COLUMN_W,
            body.origin.y,
            w,
            body.size.height,
        )
    }

    /// 命中 Spin 按钮：返回 Up/Down/None。
    pub fn hit_spin(&self, theme: &MetroTheme, rect: Rect, pos: Point) -> Option<SpinButton> {
        if self.placement != SpinPlacement::Compact {
            return None;
        }
        if self.up_button_rect(theme, rect).contains(pos) {
            Some(SpinButton::Up)
        } else if self.down_button_rect(theme, rect).contains(pos) {
            Some(SpinButton::Down)
        } else {
            None
        }
    }

    /// 整控件命中。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染。顺序：Header → 边框底 → 文本区（委托精简渲染）→ Spin 按钮。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = theme.typography.body;
        let disabled = self.state == ControlState::Disabled;
        let alpha = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };

        // Header
        if !self.header.is_empty() {
            scene.text(
                self.header.clone(),
                Rect::new(
                    rect.origin.x,
                    rect.origin.y,
                    rect.size.width,
                    style.line_height,
                ),
                colors.on_surface.with_alpha(colors.on_surface.a * alpha),
                style,
                TextAlign::Left,
            );
        }

        let body = self.body_rect(theme, rect);
        let b = if self.focused { 2.0 } else { 1.0 };
        let inner = Rect::new(
            body.origin.x,
            body.origin.y,
            (body.size.width - 2.0 * b).max(0.0),
            (body.size.height - 2.0 * b).max(0.0),
        );

        // 底色
        scene.fill_rounded_rect(
            colors.surface.with_alpha(colors.surface.a * alpha),
            inner,
            theme.tokens.corner_radius,
        );

        // 文本（左内边距 10）
        let text_rect = self.text_rect(theme, rect);
        let content_x = text_rect.origin.x + b + 10.0;
        let fg = colors.on_surface.with_alpha(colors.on_surface.a * alpha);
        let text = self.field.display_text();
        scene.text(
            text,
            Rect::new(
                content_x,
                text_rect.origin.y + b + 6.0,
                (text_rect.size.width - 16.0).max(0.0),
                style.line_height,
            ),
            fg,
            style,
            TextAlign::Left,
        );

        // Spin 按钮（Compact）
        if self.placement == SpinPlacement::Compact && !disabled {
            self.render_spin_button(
                theme,
                engine,
                self.up_button_rect(theme, rect),
                true,
                scene,
            );
            self.render_spin_button(
                theme,
                engine,
                self.down_button_rect(theme, rect),
                false,
                scene,
            );
            // 分隔竖线（Up/Down 间）
            let sep = self.down_button_rect(theme, rect);
            let sep_x = sep.right();
            scene.fill_rect(
                colors.divider.with_alpha(alpha),
                Rect::new(
                    sep_x,
                    body.origin.y + 4.0,
                    NUMBERBOX_SEPARATOR,
                    (body.size.height - 8.0).max(0.0),
                ),
            );
            // 左分隔（Spin 区与文本区）
            let spin_left = self.down_button_rect(theme, rect);
            scene.fill_rect(
                colors.divider.with_alpha(alpha),
                Rect::new(
                    spin_left.origin.x,
                    body.origin.y + 4.0,
                    NUMBERBOX_SEPARATOR,
                    (body.size.height - 8.0).max(0.0),
                ),
            );
        }

        // 边框
        let (stroke, stroke_w) = if self.focused {
            (colors.focus_stroke.with_alpha(alpha), 2.0)
        } else if self.state == ControlState::Hovered {
            (colors.on_surface_variant.with_alpha(0.9 * alpha), 1.0)
        } else {
            (colors.divider.with_alpha(alpha), 1.0)
        };
        scene.stroke_rounded_rect(stroke, inner, stroke_w, theme.tokens.corner_radius);
    }

    /// 单个 SpinButton 渲染（上/下 chevron 自绘）。
    fn render_spin_button(
        &self,
        theme: &MetroTheme,
        _engine: &TextEngine,
        btn: Rect,
        up: bool,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let hovered = if up { self.up_hovered } else { self.down_hovered };
        if hovered {
            scene.fill_rect(theme.indication.hover_tint, btn);
        }
        // chevron 三角（V7 自绘）：16px 等腰三角
        let c = btn.center();
        let s = 6.0;
        let color = colors.on_surface_variant.with_alpha(0.9);
        if up {
            scene.triangle(
                Point::new(c.x - s, c.y + s * 0.5),
                Point::new(c.x + s, c.y + s * 0.5),
                Point::new(c.x, c.y - s * 0.8),
                color,
            );
        } else {
            scene.triangle(
                Point::new(c.x - s, c.y - s * 0.5),
                Point::new(c.x + s, c.y - s * 0.5),
                Point::new(c.x, c.y + s * 0.8),
                color,
            );
        }
    }
}

/// Spin 按钮身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpinButton {
    Up,
    Down,
}

/// 便捷占位：保留 TextBox 类型链接（NumberBox 未来可开放文本部分复用）。
#[allow(dead_code)]
fn _textbox_bridge(_tb: &MetroTextBox, _k: TextInputKey) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    #[test]
    fn set_value_formats() {
        let mut nb = MetroNumberBox::new();
        nb.set_value(3.0);
        assert_eq!(nb.field.text(), "3");
        nb.set_value(3.5);
        assert_eq!(nb.field.text(), "3.5");
    }

    #[test]
    fn step_respects_small_change() {
        let mut nb = MetroNumberBox::new();
        nb.set_value(5.0);
        nb.step_up();
        assert_eq!(nb.value(), Some(6.0));
        nb.step_down();
        nb.step_down();
        assert_eq!(nb.value(), Some(4.0));
    }

    #[test]
    fn clamp_to_min_max() {
        let mut nb = MetroNumberBox::new()
            .with_min(0.0)
            .with_max(10.0)
            .with_step(2.0);
        nb.set_value(9.0);
        nb.step_up(); // 11 → clamp 10
        assert_eq!(nb.value(), Some(10.0));
        nb.step_down(); // 8
        nb.step_down(); // 6
        assert_eq!(nb.value(), Some(6.0));
        nb.set_value(-5.0); // clamp 0
        assert_eq!(nb.value(), Some(0.0));
    }

    #[test]
    fn focus_selects_all_blur_commits() {
        let mut nb = MetroNumberBox::new();
        nb.set_value(3.0);
        nb.focus();
        assert_eq!(nb.field.selection(), Some((0, 1)));
        nb.field.set_cursor(0);
        nb.field.insert_char('9');
        assert_eq!(nb.field.text(), "93");
        nb.blur();
        // blur 时 clamp + 提交
        assert_eq!(nb.value(), Some(93.0));
        assert!(!nb.focused);
    }

    #[test]
    fn non_numeric_chars_rejected() {
        let mut nb = MetroNumberBox::new();
        nb.handle_key(TextInputKey::Char('a'));
        assert_eq!(nb.field.text(), "");
        nb.handle_key(TextInputKey::Char('1'));
        nb.handle_key(TextInputKey::Char('2'));
        assert_eq!(nb.field.text(), "12");
        // 重复小数点拒绝
        nb.handle_key(TextInputKey::Char('.'));
        nb.handle_key(TextInputKey::Char('.'));
        assert_eq!(nb.field.text(), "12.");
    }

    #[test]
    fn spin_buttons_geometry() {
        let theme = MetroTheme::ether_dark();
        let nb = MetroNumberBox::new();
        let rect = Rect::new(0.0, 0.0, 200.0, 40.0);
        let up = nb.up_button_rect(&theme, rect);
        let down = nb.down_button_rect(&theme, rect);
        assert_eq!(up.size.width, NUMBERBOX_SPIN_BUTTON_W);
        assert_eq!(down.size.width, NUMBERBOX_SPIN_BUTTON_W);
        assert!(up.origin.x > down.origin.x, "Up 在右、Down 在左（72 区）");
        assert_eq!(up.right(), rect.right());
    }

    #[test]
    fn hit_spin_detects() {
        let theme = MetroTheme::ether_dark();
        let nb = MetroNumberBox::new();
        let rect = Rect::new(0.0, 0.0, 200.0, 40.0);
        let up = nb.up_button_rect(&theme, rect);
        let down = nb.down_button_rect(&theme, rect);
        assert_eq!(
            nb.hit_spin(&theme, rect, up.center()),
            Some(SpinButton::Up)
        );
        assert_eq!(
            nb.hit_spin(&theme, rect, down.center()),
            Some(SpinButton::Down)
        );
        assert_eq!(
            nb.hit_spin(&theme, rect, Point::new(10.0, 20.0)),
            None
        );
    }

    #[test]
    fn render_emits_spin_chevrons() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let nb = MetroNumberBox::new();
        let mut scene = Scene::default();
        nb.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 40.0), &mut scene);
        let triangles = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(triangles, 2, "上/下两个 chevron");
    }
}
