use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect, TextStyle};

use crate::button::MetroButton;
use crate::state::ControlState;

/// 对话框状态机。参 CONTROL_SPEC §9。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogState {
    Closed,
    Showing,
    Open,
    Hiding,
}

/// 默认按钮（Enter 触发，用 Accent 样式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogDefaultButton {
    None,
    Primary,
    Secondary,
    Close,
}

/// 对话框按钮身份（命中测试 / 回调路由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogButton {
    Primary,
    Secondary,
    Close,
}

/// 对话框按钮组。
#[derive(Debug, Clone, PartialEq)]
pub struct DialogButtons {
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub close: Option<String>,
    pub default_button: DialogDefaultButton,
}

impl Default for DialogButtons {
    fn default() -> Self {
        Self {
            primary: None,
            secondary: None,
            close: None,
            default_button: DialogDefaultButton::None,
        }
    }
}

/// MetroDialog —— 对话框（ContentDialog 参考）。参 CONTROL_SPEC §9：
/// - 尺寸 320–548 宽 / 184–756 高；Padding `24,18,24,24`；
/// - 遮罩 + 盒体分离：淡入 0.167s / 淡出 0.083s（线性近似），缩放 1.05→1.0 @0.5s；
/// - 按钮 Primary → Secondary → Close（Close 恒最右）；默认按钮 = Accent；
/// - Esc = 解除（`hide`），点击遮罩不关闭。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroDialog {
    pub title: String,
    pub content: String,
    pub buttons: DialogButtons,
    pub state: DialogState,
    /// 对话框盒体尺寸（Phase 3 续做自适应测量）。
    pub width: f32,
    pub min_width: f32,
    pub max_width: f32,
    opacity: MetroAnim,
    scale: MetroAnim,
}

/// 出现前的初始缩放（略微放大，配 opacity 淡入完成"从远处收进"的观感）。
const SCALE_INITIAL: f64 = 1.05;
/// 稳态缩放。
const SCALE_STEADY: f64 = 1.0;

impl Default for MetroDialog {
    fn default() -> Self {
        let mut opacity = MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut);
        opacity.jump_to(0.0);
        // 初始 scale = 1.05（预备态）；show 从此值收缩到 1.0，参 CONTROL_SPEC §9。
        let mut scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        scale.jump_to(SCALE_INITIAL);
        Self {
            title: String::new(),
            content: String::new(),
            buttons: DialogButtons::default(),
            state: DialogState::Closed,
            width: 448.0,
            min_width: 320.0,
            max_width: 548.0,
            opacity,
            scale,
        }
    }
}

impl MetroDialog {
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            content: content.into(),
            ..Self::default()
        }
    }

    /// 显示：遮罩 0.167s 淡入 + 盒体 1.05→1.0 @0.5s（CONTROL_SPEC §9）。
    /// 关键：sokuou 的 MetroAnim 不支持原地改时长/缓动，重建后必须 jump_to 保留
    /// 当前 value（否则新建即 value=0，盒体会从零"长"出来而不是 1.05→1.0 收缩）。
    pub fn show(&mut self) {
        if matches!(self.state, DialogState::Showing | DialogState::Open) {
            return;
        }
        // 从 Closed 起：opacity 0 / scale 1.05（预备态）。从 Hiding 中断：保当前值。
        let (from_o, from_s) = if self.state == DialogState::Closed {
            (0.0, SCALE_INITIAL)
        } else {
            (self.opacity.value(), self.scale.value())
        };
        self.state = DialogState::Showing;
        self.opacity = MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.opacity.jump_to(from_o);
        self.opacity.set_target(1.0);
        self.scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        self.scale.jump_to(from_s);
        self.scale.set_target(SCALE_STEADY);
    }

    /// 隐藏（Esc / 按钮）：opacity 0.083s 先行熄灭，盒体缩放 1.0→1.05 随后。
    /// 同样必须 jump_to 保当前 value —— 从 Open 起 scale=1.0，从 Showing 中断则任意。
    pub fn hide(&mut self) {
        if matches!(self.state, DialogState::Hiding | DialogState::Closed) {
            return;
        }
        let (from_o, from_s) = (self.opacity.value(), self.scale.value());
        self.state = DialogState::Hiding;
        self.opacity = MetroAnim::new(0.083, UwpEasing::Quadratic, EasingMode::EaseIn);
        self.opacity.jump_to(from_o);
        self.opacity.set_target(0.0);
        self.scale = MetroAnim::new(0.5, UwpEasing::Cubic, EasingMode::EaseOut);
        self.scale.jump_to(from_s);
        self.scale.set_target(SCALE_INITIAL);
    }

    /// 每帧推进；轨道稳态后转 Open/Closed。
    pub fn update(&mut self, dt: f64) {
        match self.state {
            DialogState::Showing | DialogState::Hiding => {
                self.opacity.update(dt);
                self.scale.update(dt);
                if self.opacity.is_steady() && self.scale.is_steady() {
                    self.state = if self.state == DialogState::Showing {
                        DialogState::Open
                    } else {
                        DialogState::Closed
                    };
                }
            }
            _ => {}
        }
    }

    pub fn is_visible(&self) -> bool {
        matches!(
            self.state,
            DialogState::Showing | DialogState::Open | DialogState::Hiding
        )
    }

    /// 遮罩不透明度 [0,1]。
    pub fn overlay_alpha(&self) -> f32 {
        self.opacity.value() as f32
    }

    /// 盒体缩放（1.05→1.0）。
    pub fn scale_value(&self) -> f64 {
        self.scale.value()
    }

    /// 盒体矩形：居中于 `screen`，宽度受 scale 影响。
    pub fn box_rect(&self, screen: Rect) -> Rect {
        let w = self
            .width
            .clamp(self.min_width, self.max_width)
            .min(screen.size.width - 32.0);
        let h = 240.0_f32.min(screen.size.height - 32.0);
        let s = self.scale_value() as f32;
        let sw = w * s;
        let sh = h * s;
        Rect::new(
            screen.origin.x + (screen.size.width - sw) / 2.0,
            screen.origin.y + (screen.size.height - sh) / 2.0,
            sw,
            sh,
        )
    }

    /// 命中测试：`p` 命中的对话框按钮（盒体内右下角按钮区）。
    /// `None` = 未命中按钮（遮罩 / 盒体空白 / 内容区）。
    pub fn hit_button(&self, screen: Rect, p: kanesumi_core::Point) -> Option<DialogButton> {
        if !self.is_visible() {
            return None;
        }
        let box_rect = self.box_rect(screen);
        let pad = 24.0;
        let button_w = 130.0_f32.min(202.0);
        let button_y = box_rect.origin.y + box_rect.size.height - 24.0 - 32.0;
        // 与 render 相同的布局：Close(最右) → Secondary → Primary
        let mut x = box_rect.origin.x + box_rect.size.width - pad - button_w;
        for slot in [
            DialogButton::Close,
            DialogButton::Secondary,
            DialogButton::Primary,
        ] {
            let rect = Rect::new(x, button_y, button_w, 32.0);
            if rect.contains(p) {
                // 仅返回已配置的按钮
                let configured = match slot {
                    DialogButton::Primary => self.buttons.primary.is_some(),
                    DialogButton::Secondary => self.buttons.secondary.is_some(),
                    DialogButton::Close => self.buttons.close.is_some(),
                };
                if configured {
                    return Some(slot);
                }
            }
            x -= button_w + 2.0;
        }
        None
    }

    /// 渲染：遮罩 + 盒体（标题 / 内容 / 按钮区）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        let colors = &theme.colors;

        // 遮罩
        let scrim = theme
            .overlay_color
            .with_alpha(theme.overlay_color.a * self.overlay_alpha());
        scene.fill_rect(scrim, screen);

        let box_rect = self.box_rect(screen);
        // 盒体（chrome）
        scene.fill_rounded_rect(colors.surface, box_rect, theme.tokens.corner_radius);
        scene.stroke_rounded_rect(colors.divider, box_rect, 1.0, theme.tokens.corner_radius);

        let pad = 24.0;
        let inner = Rect::new(
            box_rect.origin.x + pad,
            box_rect.origin.y + 18.0,
            box_rect.size.width - pad * 2.0,
            box_rect.size.height - 18.0 - 24.0,
        );

        // 标题（20px，最多 2 行）——CONTROL_SPEC §9 Title FontSize 20 / Normal，MaxLines 2。
        let title_style = TextStyle::new(20.0, 26.0, kanesumi_core::FontWeight::Normal);
        let title_h = if self.title.is_empty() {
            0.0
        } else {
            title_style.line_height
        };
        if !self.title.is_empty() {
            let title_rect = Rect::new(
                inner.origin.x,
                inner.origin.y,
                inner.size.width,
                title_style.line_height,
            );
            scene.text(
                self.title.clone(),
                title_rect,
                colors.on_surface,
                title_style,
                TextAlign::Left,
            );
        }

        // 内容 —— 起点 = 标题下沿 + TitleMargin(12)。
        // 旧 bug：只加 title_gap 没加 title_h → 正文与标题重叠 26px。参 CONTROL_SPEC §9。
        if !self.content.is_empty() {
            let content_style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
            let title_margin = if self.title.is_empty() { 0.0 } else { 12.0 };
            let y_off = title_h + title_margin;
            let content_rect = Rect::new(
                inner.origin.x,
                inner.origin.y + y_off,
                inner.size.width,
                (inner.size.height - 32.0 - y_off).max(0.0),
            );
            scene.text(
                self.content.clone(),
                content_rect,
                colors.on_surface,
                content_style,
                TextAlign::Left,
            );
        }

        // 按钮区（右下，Primary → Secondary → Close）
        let buttons = [
            (self.buttons.close.as_deref(), 0),
            (self.buttons.secondary.as_deref(), 1),
            (self.buttons.primary.as_deref(), 2),
        ];
        let button_w = 130.0_f32.min(202.0);
        let mut x = box_rect.origin.x + box_rect.size.width - pad;
        for (label, slot) in buttons {
            let Some(label) = label else { continue };
            x -= button_w;
            let default = match slot {
                0 => self.buttons.default_button == DialogDefaultButton::Close,
                1 => self.buttons.default_button == DialogDefaultButton::Secondary,
                _ => self.buttons.default_button == DialogDefaultButton::Primary,
            };
            let mut btn = if default {
                MetroButton::accent(label.to_string())
            } else {
                MetroButton::new(label.to_string())
            };
            btn.state = ControlState::Normal;
            let btn_rect = Rect::new(
                x,
                box_rect.origin.y + box_rect.size.height - 24.0 - 32.0,
                button_w,
                32.0,
            );
            btn.render(theme, engine, btn_rect, scene);
            x -= 2.0; // 按钮间距
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

    #[test]
    fn show_hide_cycle() {
        let mut dlg = MetroDialog::new("Save?", "Keep changes?");
        dlg.show();
        assert_eq!(dlg.state, DialogState::Showing);
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert_eq!(dlg.state, DialogState::Open);
        dlg.hide();
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert_eq!(dlg.state, DialogState::Closed);
    }

    #[test]
    fn scale_animates_1_05_to_1() {
        let mut dlg = MetroDialog::new("T", "C");
        // 关键：show 前应从 SCALE_INITIAL=1.05 起（预备态），而不是 0。
        // 老 bug：MetroAnim::new 重建后 value=0，动画变成 0→1.0（盒体从零"长"出来）。
        // 新版：default() jump_to(1.05) + show() jump_to(1.05) + set_target(1.0)。
        assert!(
            (dlg.scale_value() - 1.05).abs() < 1e-6,
            "default scale 应为 1.05，实际 {}",
            dlg.scale_value()
        );
        dlg.show();
        // 首帧后仍在 1.05 附近（尚未 tick）
        assert!(
            dlg.scale_value() > 1.0 && dlg.scale_value() <= 1.05,
            "show 首刻 scale 应 ∈ (1.0, 1.05]，实际 {}",
            dlg.scale_value()
        );
        // 走一小段，应向 1.0 收缩，且不应低于 1.0（不越过目标）
        dlg.update(0.05);
        let mid = dlg.scale_value();
        assert!(
            mid < 1.05 && mid >= 1.0,
            "中期 scale 应 ∈ [1.0, 1.05)，实际 {mid}"
        );
        // 走满，稳态 1.0
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert!(
            (dlg.scale_value() - 1.0).abs() < 1e-3,
            "终态 scale ≈ 1.0，实际 {}",
            dlg.scale_value()
        );
    }

    /// 回归 V3：hide 时 scale 从当前 1.0 起飘到 1.05，中途 opacity 不应突变。
    #[test]
    fn hide_scale_starts_from_steady_not_zero() {
        let mut dlg = MetroDialog::new("T", "C");
        dlg.show();
        // 走满进入 Open
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        assert!((dlg.scale_value() - 1.0).abs() < 1e-3);
        dlg.hide();
        // hide 首刻 scale 仍应 ≈ 1.0（从稳态起飘，不能被重置到 0）
        let s = dlg.scale_value();
        assert!(
            s >= 0.99 && s <= 1.06,
            "hide 首刻 scale 应从 1.0 起，实际 {s}"
        );
    }

    #[test]
    fn hidden_renders_nothing() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let dlg = MetroDialog::new("T", "C");
        let mut scene = Scene::default();
        dlg.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        assert!(scene.is_empty());
    }

    #[test]
    fn open_renders_scrim_box_and_buttons() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut dlg = MetroDialog::new("Save your work?", "Do you want to save?");
        dlg.buttons.primary = Some("Save".into());
        dlg.buttons.secondary = Some("Don't Save".into());
        dlg.buttons.close = Some("Cancel".into());
        dlg.buttons.default_button = DialogDefaultButton::Primary;
        dlg.show();
        for _ in 0..120 {
            dlg.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        dlg.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 遮罩 + 盒体底 + 盒体边框 + 3 按钮底 + 标题/内容/按钮文本…
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert!(fills >= 5, "遮罩 + 盒体 + 3 按钮底，实际 {fills}");
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count();
        assert!(strokes >= 1, "盒体边框");
    }

    #[test]
    fn box_rect_centers_and_clamps() {
        let mut dlg = MetroDialog::new("T", "C");
        dlg.width = 400.0;
        dlg.show();
        dlg.update(1.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let r = dlg.box_rect(screen);
        assert!(
            (r.origin.x + r.size.width / 2.0 - 400.0).abs() < 1.0,
            "水平居中"
        );
        assert!(
            (r.origin.y + r.size.height / 2.0 - 300.0).abs() < 1.0,
            "垂直居中"
        );
    }

    #[test]
    fn hit_button_maps_configured_slots() {
        let mut dlg = MetroDialog::new("Save your work?", "Do you want to save?");
        dlg.buttons.primary = Some("Save".into());
        dlg.buttons.secondary = Some("Don't Save".into());
        dlg.buttons.close = Some("Cancel".into());
        dlg.show();
        dlg.update(1.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let box_rect = dlg.box_rect(screen);
        let button_y = box_rect.origin.y + box_rect.size.height - 24.0 - 32.0 + 16.0;
        let right = box_rect.origin.x + box_rect.size.width - 24.0;

        // Close 最右
        let close = dlg
            .hit_button(screen, kanesumi_core::Point::new(right - 65.0, button_y))
            .unwrap();
        assert_eq!(close, DialogButton::Close);
        // Secondary 居中
        let sec = dlg
            .hit_button(
                screen,
                kanesumi_core::Point::new(right - 130.0 - 67.0, button_y),
            )
            .unwrap();
        assert_eq!(sec, DialogButton::Secondary);
        // Primary 最左
        let pri = dlg
            .hit_button(
                screen,
                kanesumi_core::Point::new(right - 260.0 - 69.0, button_y),
            )
            .unwrap();
        assert_eq!(pri, DialogButton::Primary);
        // 遮罩区未命中
        assert!(
            dlg.hit_button(screen, kanesumi_core::Point::new(10.0, 10.0))
                .is_none()
        );
        // 隐藏时未命中
        dlg.hide();
        dlg.update(1.0);
        assert!(
            dlg.hit_button(screen, kanesumi_core::Point::new(right - 65.0, button_y))
                .is_none()
        );
    }

    /// 回归 V2：内容 rect 的 y 必须落在标题 rect 下方，不重叠。
    /// 曾经 bug：`inner.y + title_gap(12)` 忘了加 title 的 line_height(26) →
    /// 正文 y 只比标题下移 12，实际压在标题上，视觉塌成一坨。
    #[test]
    fn content_does_not_overlap_title() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut dlg = MetroDialog::new("保存工作？", "是否保存对当前文件的更改？");
        dlg.show();
        dlg.update(1.0);
        let mut scene = Scene::default();
        dlg.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        let text_rects: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { rect, content, .. } => Some((*rect, content.clone())),
                _ => None,
            })
            .collect();
        // 找到标题和正文
        let title = text_rects
            .iter()
            .find(|(_, c)| c == "保存工作？")
            .expect("标题应产出 Text 命令");
        let content = text_rects
            .iter()
            .find(|(_, c)| c == "是否保存对当前文件的更改？")
            .expect("正文应产出 Text 命令");
        assert!(
            content.0.origin.y >= title.0.bottom(),
            "正文 y({}) 必须在标题下沿({})之下",
            content.0.origin.y,
            title.0.bottom()
        );
    }
}
