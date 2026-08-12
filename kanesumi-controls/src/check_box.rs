// MetroCheckBox —— 三态复选框。参 CONTROL_SPEC §36（CheckBox 参考，闭源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/CheckBox_themeresources_v1.xaml`：
// - 勾选框 20×20、列 0 宽 20；Padding `8,5,0,0`；MinWidth 120 / MinHeight 32；
// - 勾选 = 强调色实心 + 白色 ✓（glyph E73E）；未选 = 透明 + `on_surface_variant` 1px 描边；
// - 不确定态 = 强调色实心 + 白色 —（glyph E73C）；
// - 悬停描边转 `on_surface`；按下填充转 `on_surface_variant`；禁用降透明度；
// - 颜色硬切换（DiscreteObjectKeyFrame 0），无过渡动画。
//
// Kanesumi 适配：✓/— 用标准 Unicode（思源黑体包含，参 V7 不依赖 Segoe MDL2）；
// 强调色前景恒白；禁用降 38%。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect};

use crate::state::ControlState;

/// 勾选框边长（20）。
pub const CHECKBOX_SIZE: f32 = 20.0;
/// 描边厚度（CheckBoxBorderThemeThickness 1）。
pub const CHECKBOX_STROKE: f32 = 1.0;
/// 列 0 宽（20）+ 内容 Padding 左 8。
pub const CHECKBOX_BOX_GAP: f32 = 8.0;
/// 控件 MinHeight（32）。
pub const CHECKBOX_MIN_HEIGHT: f32 = 32.0;
/// 控件 MinWidth（120）。
pub const CHECKBOX_MIN_WIDTH: f32 = 120.0;

/// 三态值。None = 不确定（Indeterminate）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Checked,
    Unchecked,
    Indeterminate,
}

/// MetroCheckBox —— 三态复选框。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroCheckBox {
    pub label: String,
    pub state: CheckState,
    /// 交互状态（Hovered/Pressed/Disabled/Focused）。
    pub interact: ControlState,
    /// 是否允许点击进入不确定态（UWP 默认点击循环 Checked↔Unchecked，不含不确定）。
    pub allow_indeterminate: bool,
}

impl Default for MetroCheckBox {
    fn default() -> Self {
        Self {
            label: String::new(),
            state: CheckState::Unchecked,
            interact: ControlState::Normal,
            allow_indeterminate: false,
        }
    }
}

impl MetroCheckBox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    /// 三态切换（UWP 语义：默认两态循环；`allow_indeterminate` 时三态循环）。
    /// 返回新的状态。
    pub fn toggle(&mut self) -> CheckState {
        self.state = match (self.state, self.allow_indeterminate) {
            (CheckState::Unchecked, _) => CheckState::Checked,
            (CheckState::Checked, true) => CheckState::Indeterminate,
            (CheckState::Checked, false) => CheckState::Unchecked,
            (CheckState::Indeterminate, _) => CheckState::Unchecked,
        };
        self.state
    }

    /// builder：初始勾选。
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.state = if checked {
            CheckState::Checked
        } else {
            CheckState::Unchecked
        };
        self
    }

    /// 编程式设置状态。
    pub fn set_state(&mut self, s: CheckState) {
        self.state = s;
    }

    /// 勾选框矩形（相对宿主 rect，垂直居中）。
    pub fn box_rect(&self, rect: Rect) -> Rect {
        let cy = rect.origin.y + (rect.size.height - CHECKBOX_SIZE) / 2.0;
        Rect::new(rect.origin.x, cy, CHECKBOX_SIZE, CHECKBOX_SIZE)
    }

    /// 命中：整行（含标签）。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染。顺序：勾选框（描边/填充/字形）→ 标签。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = theme.typography.body;
        let disabled = self.interact == ControlState::Disabled;
        let alpha = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };
        let _ = engine;

        let box_rect = self.box_rect(rect);
        let hovered = self.interact == ControlState::Hovered;
        let pressed = self.interact == ControlState::Pressed;
        let checked = self.state != CheckState::Unchecked;

        // 勾选框底（强调色实心）或透明
        let (fill, stroke) = if checked {
            let f = if pressed {
                // CheckedPressed = SystemAccentColorDark1
                colors.primary.lerp(Color::BLACK, 0.18)
            } else if hovered {
                // CheckedPointerOver = SystemAccentColorLight1
                colors.primary.lerp(Color::WHITE, 0.18)
            } else {
                colors.primary
            };
            (f.with_alpha(f.a * alpha), Color::TRANSPARENT)
        } else if pressed {
            // UncheckedPressed = BaseMediumLow（中灰实心），无描边
            (
                colors.on_surface_variant.with_alpha(0.35 * alpha),
                Color::TRANSPARENT,
            )
        } else {
            // Unchecked：透明底 + 描边（悬停转 on_surface）
            let s = if hovered {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            (
                Color::TRANSPARENT,
                s.with_alpha(s.a * alpha),
            )
        };

        if fill.a > 0.0 {
            scene.fill_rounded_rect(fill, box_rect, CornerRadius::Square);
        }
        if stroke.a > 0.0 {
            scene.stroke_rounded_rect(stroke, box_rect, CHECKBOX_STROKE, CornerRadius::Square);
        }

        // 字形：✓ / —（白）
        if checked {
            let glyph = match self.state {
                CheckState::Indeterminate => "—",
                _ => "✓",
            };
            let fg = Color::WHITE.with_alpha(alpha);
            scene.text(
                glyph.into(),
                box_rect,
                fg,
                style,
                TextAlign::Center,
            );
        }

        // 标签（Padding 左 8 + 20 列）
        if !self.label.is_empty() {
            let fg = colors.on_surface.with_alpha(colors.on_surface.a * alpha);
            let label_x = rect.origin.x + CHECKBOX_SIZE + CHECKBOX_BOX_GAP;
            let label_w = (rect.right() - label_x).max(0.0);
            let label_rect = Rect::new(
                label_x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                label_w,
                style.line_height,
            );
            scene.text(self.label.clone(), label_rect, fg, style, TextAlign::Left);
        }
    }
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

    fn render(cb: &MetroCheckBox) -> Scene {
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        cb.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 40.0), &mut scene);
        scene
    }

    #[test]
    fn two_state_toggle_cycles() {
        let mut cb = MetroCheckBox::new("启用");
        assert_eq!(cb.toggle(), CheckState::Checked);
        assert_eq!(cb.toggle(), CheckState::Unchecked, "默认两态循环");
    }

    #[test]
    fn three_state_toggle_with_allow() {
        let mut cb = MetroCheckBox::new("全选");
        cb.allow_indeterminate = true;
        assert_eq!(cb.toggle(), CheckState::Checked);
        assert_eq!(cb.toggle(), CheckState::Indeterminate);
        assert_eq!(cb.toggle(), CheckState::Unchecked);
        assert_eq!(cb.toggle(), CheckState::Checked);
    }

    #[test]
    fn checked_renders_accent_fill_and_glyph() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("启用");
        cb.state = CheckState::Checked;
        let scene = render(&cb);
        // 强调色填充
        let accent = scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::FillRect { color, .. } if *color == MetroTheme::ether_dark().colors.primary));
        assert!(accent, "Checked 应有强调色填充");
        // ✓ 字形
        let glyph = scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "✓"));
        assert!(glyph, "Checked 渲染 ✓");
    }

    #[test]
    fn unchecked_renders_stroke_not_fill() {
        if !font_available() {
            return;
        }
        let cb = MetroCheckBox::new("启用");
        assert_eq!(cb.state, CheckState::Unchecked);
        let scene = render(&cb);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::StrokeRect { .. })),
            "Unchecked 应有描边"
        );
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 0, "Unchecked 无填充");
    }

    #[test]
    fn indeterminate_renders_dash() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("全选");
        cb.state = CheckState::Indeterminate;
        let scene = render(&cb);
        let glyph = scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "—"));
        assert!(glyph, "Indeterminate 渲染 —");
    }

    #[test]
    fn hit_test_contains() {
        let cb = MetroCheckBox::new("启用");
        let rect = Rect::new(0.0, 0.0, 200.0, 40.0);
        assert!(cb.hit_test(rect, Point::new(30.0, 20.0)));
        assert!(!cb.hit_test(rect, Point::new(300.0, 20.0)));
    }

    #[test]
    fn disabled_lowers_alpha() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("启用");
        cb.state = CheckState::Checked;
        cb.interact = ControlState::Disabled;
        let scene = render(&cb);
        // 强调色填充 alpha 应 < 1
        let a = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, .. } => Some(color.a),
                _ => None,
            })
            .fold(1.0f32, f32::min);
        assert!(a < 1.0, "禁用降透明度，实际 a={}", a);
    }
}
