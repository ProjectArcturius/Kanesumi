use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect, Size, TextStyle};
use kanesumi_element::{
    ArrangeCtx, Event, EventCtx, Key, MeasureCtx, PaintCtx, PointerButton, PopupSpec, Widget,
    WidgetId,
};

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
        // V18：SPEC 要求 Linear（sokuou 无 Linear 变体，Power(1.0)=t^1=t 等价线性）。
        // 原 Quadratic/EaseIn 曲线开头慢、末尾快 —— dialog 消失感官"拖尾"。
        self.opacity = MetroAnim::new(0.083, UwpEasing::Power(1.0), EasingMode::EaseOut);
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
        self.hit_button_in(self.box_rect(screen), p)
    }

    /// 命中测试（给定盒体矩形）——元素树里盒体矩形由框架 arrange 决定，故与 `screen`
    /// 版本分开；旧 `hit_button` 转发到此。
    fn hit_button_in(&self, box_rect: Rect, p: kanesumi_core::Point) -> Option<DialogButton> {
        self.button_layout(box_rect)
            .into_iter()
            .find(|(_, _, rect)| rect.contains(p))
            .map(|(slot, _, _)| slot)
    }

    /// 按钮区布局 —— **唯一实现**：渲染、命中、无障碍三处都从这里取。
    ///
    /// 此前渲染与命中各写了一遍「从右到左、宽 130、间距 2」，两处一旦有一位不一致，
    /// 症状是「看见的按钮点不到、点得到的地方没有按钮」，而且只在某个偏门组合下复现。
    /// 返回顺序 = 从右到左（Close → Secondary → Primary），只含已配置的按钮。
    fn button_layout(&self, box_rect: Rect) -> Vec<(DialogButton, String, Rect)> {
        const PAD: f32 = 24.0;
        const BUTTON_W: f32 = 130.0;
        const BUTTON_H: f32 = 32.0;
        const GAP: f32 = 2.0;
        let button_y = box_rect.origin.y + box_rect.size.height - PAD - BUTTON_H;
        // 从右往左**紧挨着排**：只有已配置的按钮占位（缺 Secondary 时 Close 与 Primary 相邻）。
        // 按槽位序号排会留出空档 —— 第一版就这样把「确定」推到了对话框最左边。
        let mut occupied = 0usize;
        [
            (DialogButton::Close, self.buttons.close.as_deref()),
            (DialogButton::Secondary, self.buttons.secondary.as_deref()),
            (DialogButton::Primary, self.buttons.primary.as_deref()),
        ]
        .into_iter()
        .filter_map(|(slot, label)| {
            let label = label?;
            let x = box_rect.origin.x + box_rect.size.width
                - PAD
                - BUTTON_W
                - occupied as f32 * (BUTTON_W + GAP);
            occupied += 1;
            Some((
                slot,
                label.to_string(),
                Rect::new(x, button_y, BUTTON_W, BUTTON_H),
            ))
        })
        .collect()
    }

    /// 渲染：遮罩 + 盒体（标题 / 内容 / 按钮区）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        // 遮罩
        let scrim = theme
            .overlay_color
            .with_alpha(theme.overlay_color.a * self.overlay_alpha());
        scene.fill_rect(scrim, screen);
        self.render_box(theme, engine, self.box_rect(screen), scene, false);
    }

    /// 画盒体（标题 / 内容 / 按钮区）——不含遮罩。元素树面板 Widget 复用（盒体矩形由框架给出）。
    ///
    /// `has_content_children`：节点下有子节点（内容槽）时不画文本正文，画面让给子节点 ——
    /// 与按钮 / 勾选框同一套规则（参 docs/DEV_GUIDE.md §2.6）。
    fn render_box(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        box_rect: Rect,
        scene: &mut Scene,
        has_content_children: bool,
    ) {
        let colors = &theme.colors;

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

        // 标题（20px，最多 2 行）——CONTROL_SPEC §9 Title FontSize 20，标题类字重（G-67），MaxLines 2。
        let title_style = TextStyle::new(20.0, 26.0, kanesumi_core::typography::heading_weight());
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

        // 内容 —— 矩形与内容子树共用 `content_area`（有子节点时这段文字不画）。
        // 旧 bug：只加 title_gap 没加 title_h → 正文与标题重叠 26px。参 CONTROL_SPEC §9。
        if !self.content.is_empty() && !has_content_children {
            let content_style = kanesumi_core::MetroTypography::metro().body_medium;
            scene.text(
                self.content.clone(),
                self.content_area(box_rect),
                colors.on_surface,
                content_style,
                TextAlign::Left,
            );
        }

        // 按钮区（右下）：布局与命中、无障碍共用 `button_layout`，不在此另算一遍。
        for (slot, label, btn_rect) in self.button_layout(box_rect) {
            let default = matches!(
                (slot, self.buttons.default_button),
                (DialogButton::Primary, DialogDefaultButton::Primary)
                    | (DialogButton::Secondary, DialogDefaultButton::Secondary)
                    | (DialogButton::Close, DialogDefaultButton::Close)
            );
            let mut btn = if default {
                MetroButton::accent(label)
            } else {
                MetroButton::new(label)
            };
            btn.state = ControlState::Normal;
            btn.render(theme, engine, btn_rect, scene);
        }
    }
}

// ── 元素树接入（弹层类控件，参 docs/ELEMENT_MIGRATION.md §8）────────────────────
//
// 模态弹层：`PopupSpec { side: Center, modal: true, light_dismiss: false, scrim: true }` ——
// 表面居中、点外部不关闭（吞掉点击）、Tab 被限制在弹层内（焦点陷阱）、框架在弹层之下
// 铺主题遮罩（`overlay_color`）。盒体自身即弹层节点（`MetroDialog` 实现 `Widget`）。

/// 元素树动作：对话框按钮被激活（点击 / Esc = Close / Enter = Primary）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DialogResult(pub DialogButton);

/// 盒体固有高（旧 `box_rect` 的 240，受表面高夹紧；无内容子树时沿用）。
const DIALOG_BOX_H: f32 = 240.0;

// 盒体内边距与行高（CONTROL_SPEC §9：Padding `24,18,24,24`、按钮高 32）。
const DIALOG_PAD_X: f32 = 24.0;
const DIALOG_PAD_TOP: f32 = 18.0;
const DIALOG_PAD_BOTTOM: f32 = 24.0;
const DIALOG_TITLE_GAP: f32 = 12.0;
const DIALOG_CONTENT_GAP: f32 = 8.0;
const DIALOG_BUTTON_ROW: f32 = 32.0;
/// 高度范围（CONTROL_SPEC §9 Min/Max 184/756）。
const DIALOG_MIN_H: f32 = 184.0;
const DIALOG_MAX_H: f32 = 756.0;

impl MetroDialog {
    /// 内容区（标题之下、按钮行之上的矩形）——文本正文与内容子树共用同一个矩形，
    /// 免得「有子节点」和「只有文字」两种对话框的正文起点差几像素。
    pub fn content_area(&self, box_rect: Rect) -> Rect {
        let inner = Rect::new(
            box_rect.origin.x + DIALOG_PAD_X,
            box_rect.origin.y + DIALOG_PAD_TOP,
            (box_rect.size.width - DIALOG_PAD_X * 2.0).max(0.0),
            (box_rect.size.height - DIALOG_PAD_TOP - DIALOG_PAD_BOTTOM).max(0.0),
        );
        let title_h = if self.title.is_empty() { 0.0 } else { 26.0 };
        let title_gap = if self.title.is_empty() {
            0.0
        } else {
            DIALOG_TITLE_GAP
        };
        let y_off = title_h + title_gap;
        Rect::new(
            inner.origin.x,
            inner.origin.y + y_off,
            inner.size.width,
            (inner.size.height - DIALOG_BUTTON_ROW - DIALOG_CONTENT_GAP - y_off).max(0.0),
        )
    }

    /// 元素树：模态打开（表面居中、焦点陷阱、点外部不关闭）。返回弹层 id。
    pub fn show_modal(&mut self, ctx: &mut EventCtx) -> WidgetId {
        let id = ctx.open_popup(
            self.clone(),
            PopupSpec {
                side: kanesumi_element::PopupSide::Center,
                modal: true,
                light_dismiss: false,
                scrim: true,
                ..PopupSpec::default()
            },
        );
        ctx.focus_widget(id, true);
        id
    }
}

impl Widget for MetroDialog {
    /// 尺寸：宽按 `width` 夹在 MIN/MAX 宽；高在**有内容子树**时按内容算
    /// （18 上边距 + 标题行 + 12 间距 + 内容 + 8 间距 + 32 按钮行 + 24 下边距），
    /// 夹在 CONTROL_SPEC §9 的 184..756；无内容子树时沿用固定 240（纯文本对话框）。
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let width = self.width.clamp(self.min_width, self.max_width);
        let kids = ctx.children();
        if kids.is_empty() {
            return Size::new(width, DIALOG_BOX_H);
        }
        let style = kanesumi_core::MetroTypography::metro().body_medium;
        let title_h = if self.title.is_empty() {
            0.0
        } else {
            let title_style =
                TextStyle::new(20.0, 26.0, kanesumi_core::typography::heading_weight());
            title_style.line_height
        };
        let title_margin = if self.title.is_empty() { 0.0 } else { 12.0 };
        let chrome = DIALOG_PAD_TOP + title_h + title_margin + DIALOG_CONTENT_GAP
            + DIALOG_BUTTON_ROW
            + DIALOG_PAD_BOTTOM;
        let avail = Size::new(
            (width - DIALOG_PAD_X * 2.0).max(0.0),
            (available.height - chrome).max(0.0),
        );
        let mut content = Size::new(0.0, style.line_height);
        for c in kids {
            let d = ctx.measure_child(c, avail);
            content = Size::new(content.width.max(d.width), content.height.max(d.height));
        }
        let height = (chrome + content.height).clamp(DIALOG_MIN_H, DIALOG_MAX_H);
        Size::new(width, height)
    }

    /// 内容子树排进标题之下、按钮行之上的内容区。
    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let inner = self.content_area(rect);
        for c in ctx.children() {
            ctx.arrange_child(c, inner);
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        let has_content = !ctx.children().is_empty();
        self.render_box(&theme, ctx.engine(), ctx.rect(), scene, has_content);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                if let Some(b) = self.hit_button_in(ctx.rect(), *pos) {
                    ctx.emit(DialogResult(b));
                    ctx.close_popup(ctx.id());
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => match key {
                // Esc = Close（模态弹层 light_dismiss=false，框架不会代劳）。
                Key::Escape => {
                    ctx.emit(DialogResult(DialogButton::Close));
                    ctx.close_popup(ctx.id());
                    ctx.set_handled();
                }
                // Enter = Primary（未配置 Primary 时无动作）。
                Key::Enter => {
                    if self.buttons.primary.is_some() {
                        ctx.emit(DialogResult(DialogButton::Primary));
                        ctx.close_popup(ctx.id());
                    }
                    ctx.set_handled();
                }
                _ => {}
            },
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 模态盒体不画框架焦点框（整框描边无意义；按钮动作由 Enter/Esc 承担）。
    fn focus_visual(&self) -> bool {
        false
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: self.title.clone(),
            value: Some(self.content.clone()),
            checked: None,
        })
    }

    /// 合成项：对话框按钮（UIA 里按钮是该 Group 的子元素）。
    ///
    /// 按钮是控件自绘的（不是框架节点），所以 Tab 走到不了它们 —— 读屏至少要能枚举、
    /// 能读到名字与是否默认按钮（默认按钮在名字后缀「（默认）」）。把按钮做成真节点是
    /// 另一张卡（要动焦点陷阱与 Enter 语义），这里先补可读性。
    fn access_items(
        &self,
        rect: Rect,
        _theme: &kanesumi_core::MetroTheme,
    ) -> Vec<kanesumi_element::AccessItem> {
        self.button_layout(rect)
            .into_iter()
            .map(|(slot, label, bounds)| {
                let is_default = matches!(
                    (slot, self.buttons.default_button),
                    (DialogButton::Primary, DialogDefaultButton::Primary)
                        | (DialogButton::Secondary, DialogDefaultButton::Secondary)
                        | (DialogButton::Close, DialogDefaultButton::Close)
                );
                let name = if is_default {
                    format!("{label}（默认）")
                } else {
                    label
                };
                kanesumi_element::AccessItem::new(
                    kanesumi_element::AccessRole::Button,
                    name,
                    bounds,
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_core::Point;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps};

    // ── 内容槽与按钮无障碍（2026-11-06）────────────────────────────────────

    /// 带内容子树的对话框：标题 + 一个固定尺寸的「内容」。
    fn content_harness(child: Size) -> (TestHarness, WidgetId, WidgetId) {
        struct Probe {
            size: Size,
        }
        impl Widget for Probe {
            fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
                self.size
            }
            fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        }
        let mut h = TestHarness::new(600.0, 700.0);
        let mut dlg = MetroDialog::new("重命名", "占位正文");
        dlg.buttons.primary = Some("确定".into());
        dlg.buttons.close = Some("取消".into());
        dlg.buttons.default_button = DialogDefaultButton::Primary;
        let opener = h.tree.insert_with(
            h.root(),
            Opener { dlg },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        let popup = open(&mut h, opener);
        let content = h
            .tree
            .insert_with(popup, Probe { size: child }, LayoutProps::default());
        h.frame();
        (h, popup, content)
    }

    #[test]
    fn content_slot_sizes_dialog_by_its_child() {
        let (h, popup, content) = content_harness(Size::new(200.0, 300.0));
        let r = h.rect(popup);
        assert!(
            r.size.height > DIALOG_BOX_H,
            "内容 300 高时对话框要比固定 240 高：{}",
            r.size.height
        );
        assert!(r.size.height <= DIALOG_MAX_H, "仍夹在最大高内：{}", r.size.height);
        let cr = h.rect(content);
        let area = {
            let dlg = h.tree.get::<MetroDialog>(popup).expect("对话框");
            dlg.content_area(r)
        };
        assert!(
            (cr.origin.y - area.origin.y).abs() < 0.5
                && (cr.origin.x - area.origin.x).abs() < 0.5,
            "内容排在 content_area：{cr:?} vs {area:?}"
        );
        h.assert_contained();
    }

    #[test]
    fn content_slot_suppresses_text_but_keeps_title() {
        let (h, popup, _) = content_harness(Size::new(120.0, 40.0));
        let texts: Vec<String> = h
            .tree
            .painted(popup)
            .iter()
            .filter_map(|c| match c {
                kanesumi_canvas::SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "重命名"), "标题照画：{texts:?}");
        assert!(
            !texts.iter().any(|t| t == "占位正文"),
            "有内容子树时不画文本正文：{texts:?}"
        );
    }

    #[test]
    fn dialog_buttons_are_accessibility_items() {
        let (h, popup, _) = content_harness(Size::new(120.0, 40.0));
        let snap = h.access_snapshot();
        let node = snap.find(popup).expect("对话框在快照内");
        let names: Vec<&str> = node.items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, vec!["取消", "确定（默认）"], "按钮按从右到左报出，默认按钮带后缀");
        let primary = node.items.iter().find(|i| i.name.contains("确定")).unwrap();
        assert_eq!(primary.role, kanesumi_element::AccessRole::Button);
        assert_eq!(primary.bounds.size.height, 32.0, "按钮高 32");
        h.assert_access_contract();
    }

    #[test]
    fn dialog_button_hit_matches_accessibility_bounds() {
        // 渲染 / 命中 / 无障碍共用 `button_layout`，所以「看见的按钮」= 「能点的位置」= 「报出去的矩形」。
        let (h, popup, _) = content_harness(Size::new(120.0, 40.0));
        let snap = h.access_snapshot();
        let node = snap.find(popup).expect("对话框在快照内");
        let dlg = h.tree.get::<MetroDialog>(popup).expect("对话框");
        for item in &node.items {
            let p = item.bounds.center();
            let hit = dlg.hit_button_in(h.rect(popup), p);
            assert!(hit.is_some(), "无障碍报出的按钮矩形中心必须能命中：{} {p:?}", item.name);
        }
    }
    /// 触发器：点击模态打开对话框。
    struct Opener {
        dlg: MetroDialog,
    }

    impl Widget for Opener {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(80.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Click = event {
                self.dlg.show_modal(ctx);
                ctx.set_handled();
            }
        }
        fn focusable(&self) -> bool {
            true
        }
        /// 夹具也是可聚焦控件：无障碍契约要求有角色与名字。
        fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
            Some(kanesumi_element::AccessInfo {
                role: kanesumi_element::AccessRole::Button,
                name: "打开对话框".to_string(),
                value: None,
                checked: None,
            })
        }
    }

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(600.0, 500.0);
        let mut dlg = MetroDialog::new("保存工作？", "是否保存对当前文件的更改？");
        dlg.buttons.primary = Some("保存".into());
        dlg.buttons.secondary = Some("不保存".into());
        dlg.buttons.close = Some("取消".into());
        dlg.buttons.default_button = DialogDefaultButton::Primary;
        let opener = h.tree.insert_with(
            h.root(),
            Opener { dlg },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(20.0, 20.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, opener)
    }

    fn open(h: &mut TestHarness, opener: WidgetId) -> WidgetId {
        h.click(opener);
        h.tree.popups().next().expect("对话框应打开")
    }

    #[test]
    fn modal_centers_and_traps_focus() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        let r = h.rect(d);
        assert!(
            (r.origin.x + r.size.width / 2.0 - h.size.width / 2.0).abs() < 1.5,
            "水平居中 {r:?}"
        );
        assert!(
            (r.origin.y + r.size.height / 2.0 - h.size.height / 2.0).abs() < 1.5,
            "垂直居中 {r:?}"
        );
        assert_eq!(h.tree.focused(), Some(d), "打开即聚焦弹层");
        h.tab();
        h.tab();
        assert_eq!(h.tree.focused(), Some(d), "Tab 不得逃出对话框");
    }

    #[test]
    fn click_close_button_reports_result() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        let r = h.rect(d);
        let bw = 130.0_f32.min(202.0);
        let by = r.origin.y + r.size.height - 24.0 - 32.0 + 16.0;
        let cx = r.origin.x + r.size.width - 24.0 - bw / 2.0; // Close 最右
        h.click_at(Point::new(cx, by));
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(d, DialogResult(DialogButton::Close))]
        );
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn enter_is_primary_and_escape_is_close() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        h.key(Key::Enter);
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(d, DialogResult(DialogButton::Primary))]
        );
        assert!(h.tree.popups().next().is_none());

        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        h.key(Key::Escape);
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(d, DialogResult(DialogButton::Close))]
        );
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn outside_click_does_not_close_modal() {
        let (mut h, opener) = harness();
        let _d = open(&mut h, opener);
        h.click_at(Point::new(10.0, 10.0));
        assert!(h.tree.popups().next().is_some(), "模态点外部不关闭");
        assert!(h.take::<DialogResult>().is_empty());
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        h.assert_contained();
        h.assert_no_hit_outside(d);
        h.assert_paint_within(d, Insets::ZERO);
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
