use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Rect, Size, TextStyle};
use kanesumi_element::{
    ArrangeCtx, Event, EventCtx, Key, LayoutProps, LayoutRole, MeasureCtx, PaintCtx, PointerButton,
    PopupSpec, Widget, WidgetId,
};

use crate::dialog_button::{DIALOG_BUTTON_H, DIALOG_BUTTON_W, MetroDialogButton};
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
    /// 「当前按钮」（键盘 ←/→ 或指针悬停选定）。打开时预置为默认按钮。
    ///
    /// **只服务即时模式**（`render` / `hit_button` 这条不走元素树的路径）。元素树模式下
    /// 按钮是框架节点（`MetroDialogButton`），当前按钮 = 框架焦点，本字段不参与绘制与
    /// 命中：两处都记一份就会出现「键盘亮一个、指针亮一个」以及「Enter 与高亮对不上」。
    pub focused_button: Option<DialogButton>,
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
            focused_button: None,
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

    /// 视觉顺序（左 → 右）的已配置按钮：Primary → Secondary → Close。
    ///
    /// ←/→ 按这个顺序走；反过来排会让「按 → 往右走」变成往左跳。
    fn ordered_buttons(&self) -> Vec<DialogButton> {
        [
            (DialogButton::Primary, self.buttons.primary.is_some()),
            (DialogButton::Secondary, self.buttons.secondary.is_some()),
            (DialogButton::Close, self.buttons.close.is_some()),
        ]
        .into_iter()
        .filter(|(_, configured)| *configured)
        .map(|(b, _)| b)
        .collect()
    }

    /// 在已配置按钮间移动当前按钮（`delta` = ±1），到端循环。
    pub fn step_button(&mut self, delta: isize) {
        let order = self.ordered_buttons();
        if order.is_empty() {
            return;
        }
        let n = order.len() as isize;
        let cur = self
            .focused_button
            .or_else(|| self.default_button())
            .and_then(|b| order.iter().position(|x| *x == b))
            .map(|i| i as isize)
            .unwrap_or(if delta > 0 { -1 } else { n });
        let next = (cur + delta.signum()).rem_euclid(n) as usize;
        self.focused_button = Some(order[next]);
    }

    /// 配置的默认按钮；未配置或指向未配置的按钮时退回首个已配置按钮。
    fn default_button(&self) -> Option<DialogButton> {
        let configured = |b: DialogButton| match b {
            DialogButton::Primary => self.buttons.primary.is_some(),
            DialogButton::Secondary => self.buttons.secondary.is_some(),
            DialogButton::Close => self.buttons.close.is_some(),
        };
        let preferred = match self.buttons.default_button {
            DialogDefaultButton::Primary => Some(DialogButton::Primary),
            DialogDefaultButton::Secondary => Some(DialogButton::Secondary),
            DialogDefaultButton::Close => Some(DialogButton::Close),
            DialogDefaultButton::None => None,
        };
        preferred
            .filter(|b| configured(*b))
            .or_else(|| self.ordered_buttons().into_iter().next())
    }

    /// Enter / Space 会激活的按钮：当前按钮 → 默认按钮。
    pub fn activated_button(&self) -> Option<DialogButton> {
        self.focused_button.or_else(|| self.default_button())
    }

    /// 按钮区布局 —— **唯一实现**：渲染、命中、无障碍三处都从这里取。
    ///
    /// 此前渲染与命中各写了一遍「从右到左、宽 130、间距 2」，两处一旦有一位不一致，
    /// 症状是「看见的按钮点不到、点得到的地方没有按钮」，而且只在某个偏门组合下复现。
    /// 返回顺序 = 从右到左（Close → Secondary → Primary），只含已配置的按钮。
    fn button_layout(&self, box_rect: Rect) -> Vec<(DialogButton, String, Rect)> {
        // 从右往左**紧挨着排**：只有已配置的按钮占位（缺 Secondary 时 Close 与 Primary 相邻）。
        // 按槽位序号排会留出空档 —— 第一版就这样把「确定」推到了对话框最左边。
        let slots: Vec<(DialogButton, String)> = [
            (DialogButton::Close, self.buttons.close.as_deref()),
            (DialogButton::Secondary, self.buttons.secondary.as_deref()),
            (DialogButton::Primary, self.buttons.primary.as_deref()),
        ]
        .into_iter()
        .filter_map(|(slot, label)| label.map(|l| (slot, l.to_string())))
        .collect();
        command_row_rects(box_rect, slots.len())
            .into_iter()
            .zip(slots)
            .map(|(rect, (slot, label))| (slot, label, rect))
            .collect()
    }

    /// 已配置按钮的视觉序（左 → 右：Primary → Secondary → Close），与 `button_layout`
    /// 的返回顺序（右 → 左）相反 —— 节点按这个顺序插入，Tab 才是从左往右走。
    fn declared_buttons(&self) -> Vec<MetroDialogButton> {
        [
            (DialogButton::Primary, self.buttons.primary.as_deref()),
            (DialogButton::Secondary, self.buttons.secondary.as_deref()),
            (DialogButton::Close, self.buttons.close.as_deref()),
        ]
        .into_iter()
        .filter_map(|(slot, label)| {
            let label = label?;
            let is_default = matches!(
                (slot, self.buttons.default_button),
                (DialogButton::Primary, DialogDefaultButton::Primary)
                    | (DialogButton::Secondary, DialogDefaultButton::Secondary)
                    | (DialogButton::Close, DialogDefaultButton::Close)
            );
            Some(MetroDialogButton::new(slot, label).default_button(is_default))
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
        self.render_box(theme, engine, self.box_rect(screen), scene, false, false);
    }

    /// 画盒体（标题 / 内容 / 按钮区）——不含遮罩。元素树面板 Widget 复用（盒体矩形由框架给出）。
    ///
    /// `has_content_children`：节点下有子节点（内容槽）时不画文本正文，画面让给子节点 ——
    /// 与按钮 / 勾选框同一套规则（参 docs/DEV_GUIDE.md §2.6）。
    /// `has_command_children`：命令区已是真节点（框架随后按树序绘制）时不画自绘按钮，
    /// 否则会画两遍 —— 一遍点不到（自绘）、一遍能点（节点）。
    fn render_box(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        box_rect: Rect,
        scene: &mut Scene,
        has_content_children: bool,
        has_command_children: bool,
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

        // 按钮区（右下）：命令区已经有真节点时不画（框架随后按树序绘制，再画一遍就是双份）。
        if has_command_children {
            return;
        }
        for (slot, label, btn_rect) in self.button_layout(box_rect) {
            let default = matches!(
                (slot, self.buttons.default_button),
                (DialogButton::Primary, DialogDefaultButton::Primary)
                    | (DialogButton::Secondary, DialogDefaultButton::Secondary)
                    | (DialogButton::Close, DialogDefaultButton::Close)
            );
            // 与节点模式共用 `MetroDialogButton::paint_at`：自绘与节点两条路径的按钮
            // 在底色 / 交互 tint / 标签位置上一个像素都不该差。
            MetroDialogButton::new(slot, label)
                .default_button(default)
                .paint_at(theme, engine, ControlState::Normal, btn_rect, scene);
            // 焦点环必须画在按钮**之后**：先画会被按钮自己的底色盖住（外圈只剩 1px，
            // 深色下几乎看不见）。这里是即时模式「当前按钮」唯一的焦点视觉；
            // 节点模式下由框架画在焦点节点上。
            if self.focused_button == Some(slot) {
                crate::focus::draw_focus_ring(
                    scene,
                    theme.indication.focus_stroke,
                    theme.scheme,
                    btn_rect,
                    theme.tokens.corner_radius,
                );
            }
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
/// 命令区相邻按钮的水平间距（CONTROL_SPEC §9）。
const DIALOG_BUTTON_GAP: f32 = 2.0;

/// 命令区矩形：从**右缘往左**紧挨着排，第 `i` 个矩形 = 从右数第 `i` 个命令子节点。
///
/// 几何只在这里算一次 —— 渲染、命中、无障碍、`arrange` 全从它取。此前渲染与命中各写了
/// 一遍同样的公式，一位不一致的症状就是「看得见的按钮点不到」。
fn command_row_rects(box_rect: Rect, count: usize) -> Vec<Rect> {
    let y = box_rect.origin.y + box_rect.size.height - DIALOG_PAD_X - DIALOG_BUTTON_H;
    (0..count)
        .map(|i| {
            let x = box_rect.origin.x + box_rect.size.width
                - DIALOG_PAD_X
                - DIALOG_BUTTON_W
                - i as f32 * (DIALOG_BUTTON_W + DIALOG_BUTTON_GAP);
            Rect::new(x, y, DIALOG_BUTTON_W, DIALOG_BUTTON_H)
        })
        .collect()
}

/// 子节点是否属于**命令区**（`LayoutProps::role == Commands`）。
///
/// 只按角色分区：`Normal` 与 `Content` 都按内容区处理，与「布局角色」引入之前的
/// 既有行为一致（老代码把全部子节点当正文），已有对话框不必改一行就能继续用。
fn is_command(props: &LayoutProps) -> bool {
    props.role == LayoutRole::Commands
}

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
        self.show_modal_with(ctx, |_, _| {})
    }

    /// 打开模态并按 `build` 把内容挂进对话框（弹层 id 已给出）。
    ///
    /// 内容槽的自然写法：一次调用同时完成「开」、「定位」、「填内容」。没有它，应用得先
    /// `show_modal` 再回头去树里插子节点 —— 为了放一个输入框而暴露树可变引用。
    ///
    /// 命令区的按钮由这里**自动插成真节点**（`LayoutProps::role = Commands`），应用不必
    /// 自己插，也不该自己插：按钮的槽位身份、默认标记与关闭弹层的语义都跟 `DialogButtons`
    /// 配置绑定，两处各插一次就会出现四个按钮。
    pub fn show_modal_with(
        &mut self,
        ctx: &mut EventCtx,
        build: impl FnOnce(&mut EventCtx, WidgetId),
    ) -> WidgetId {
        // 打开即把「当前按钮」定在默认按钮上：否则键盘用户先看到没有任何高亮的按钮区，
        // 而且第一次按 → 会跳过它。（即时模式字段；节点模式下焦点由框架持有。）
        self.focused_button = self.default_button();
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
        build(ctx, id);
        // 按视觉序（左 → 右：Primary → Secondary → Close）插入：树序即 Tab 序，
        // 声明序反了会让 Tab 从最右边的 Close 开始往左走。
        let mut default_id = None;
        let mut first_id = None;
        for button in self.declared_buttons() {
            let is_default = button.is_default;
            let cmd = ctx.insert_child_with(
                id,
                button,
                LayoutProps::default().in_role(LayoutRole::Commands),
            );
            if first_id.is_none() {
                first_id = Some(cmd);
            }
            if is_default {
                default_id = Some(cmd);
            }
        }
        // 焦点：默认按钮 → 首个命令按钮 → 对话框本体（一个按钮都没配时）。
        // 落在按钮上而不是盒体上，Enter 才直接生效（按钮自己处理激活）。
        // 注意不能用 `ctx.children()` 找：那是**本节点**（开窗者）的子节点，不是弹层的。
        let focus = default_id.or(first_id).unwrap_or(id);
        ctx.focus_widget(focus, true);
        id
    }
}

impl Widget for MetroDialog {
    /// 尺寸：宽按 `width` 夹在 MIN/MAX 宽；高在**有内容子树**时按内容算
    /// （18 上边距 + 标题行 + 12 间距 + 内容 + 8 间距 + 32 按钮行 + 24 下边距），
    /// 夹在 CONTROL_SPEC §9 的 184..756；完全没有子节点时沿用固定 240（纯文本对话框）。
    ///
    /// 命令子节点只量不占高：按钮行高恒为 32（规格），让内容少一个子节点就塌一截不是
    /// 我们想要的效果 —— 一行固定节拍的按钮不该因为标签短就变矮。
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let width = self.width.clamp(self.min_width, self.max_width);
        let kids = ctx.children();
        if kids.is_empty() {
            return Size::new(width, DIALOG_BOX_H);
        }
        let (content_kids, command_kids): (Vec<WidgetId>, Vec<WidgetId>) = kids
            .into_iter()
            .partition(|c| !is_command(&ctx.child_props(*c)));
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
        let inner_w = (width - DIALOG_PAD_X * 2.0).max(0.0);
        let avail = Size::new(inner_w, (available.height - chrome).max(0.0));
        let mut content = Size::new(0.0, style.line_height);
        for c in content_kids {
            let d = ctx.measure_child(c, avail);
            content = Size::new(content.width.max(d.width), content.height.max(d.height));
        }
        for c in command_kids {
            ctx.measure_child(c, Size::new(inner_w, DIALOG_BUTTON_ROW));
        }
        let height = (chrome + content.height).clamp(DIALOG_MIN_H, DIALOG_MAX_H);
        Size::new(width, height)
    }

    /// 按角色分区排布：命令子节点进按钮行（右→左紧排），其余进内容区。
    ///
    /// 声明序 = 视觉左→右，`command_row_rects` 的序 = 右→左，故**反序配对**：
    /// 这样「最后插入的按钮在最右边」（Close 恒最右，参 CONTROL_SPEC §9）。
    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let kids = ctx.children();
        let commands: Vec<WidgetId> = kids
            .iter()
            .copied()
            .filter(|c| is_command(&ctx.child_props(*c)))
            .collect();
        let contents: Vec<WidgetId> = kids
            .iter()
            .copied()
            .filter(|c| !is_command(&ctx.child_props(*c)))
            .collect();
        let rects = command_row_rects(rect, commands.len());
        for (i, c) in commands.into_iter().enumerate() {
            let r = rects[rects.len() - 1 - i];
            ctx.arrange_child(c, r);
        }
        let content_area = self.content_area(rect);
        for c in contents {
            ctx.arrange_child(c, content_area);
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        let kids = ctx.children();
        let has_content = kids.iter().any(|c| !is_command(&ctx.child_props(*c)));
        let has_command = kids.iter().any(|c| is_command(&ctx.child_props(*c)));
        self.render_box(&theme, ctx.engine(), ctx.rect(), scene, has_content, has_command);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        let commands: Vec<WidgetId> = ctx
            .children()
            .into_iter()
            .filter(|c| is_command(&ctx.child_props(*c)))
            .collect();
        let node_mode = !commands.is_empty();
        match event {
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                if node_mode {
                    // 按钮已真节点化：点按钮由按钮自己处理（命中优先于父节点），
                    // 这里只需吞掉落点在盒体空白处的点击（模态 light_dismiss=false）。
                    ctx.set_handled();
                } else if let Some(b) = self.hit_button_in(ctx.rect(), *pos) {
                    ctx.emit(DialogResult(b));
                    ctx.close_popup(ctx.id());
                    ctx.set_handled();
                } else {
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => match key {
                // Esc = Close（模态弹层 light_dismiss=false，框架不会代劳）。
                Key::Escape => {
                    ctx.emit(DialogResult(DialogButton::Close));
                    ctx.close_popup(ctx.id());
                    ctx.set_handled();
                }
                // ← / →：节点模式挪**框架焦点**（唯一真相），即时模式挪自己的当前按钮字段。
                Key::Left | Key::Right if node_mode => {
                    let delta = if *key == Key::Left { -1 } else { 1 };
                    step_command_focus(ctx, &commands, delta);
                    ctx.set_handled();
                }
                Key::Left => {
                    self.step_button(-1);
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
                Key::Right => {
                    self.step_button(1);
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
                // Enter / Space：节点模式下由焦点按钮自己处理（它先收到事件并截停），
                // 只有焦点落在盒体本身时才在这里兜底 —— 否则内容区里的文本框按空格
                // 会把对话框关掉。
                Key::Enter | Key::Char(' ') => {
                    if node_mode && ctx.focus_id() != Some(ctx.id()) {
                        return;
                    }
                    if let Some(b) = self.activated_button() {
                        ctx.emit(DialogResult(b));
                        ctx.close_popup(ctx.id());
                    }
                    ctx.set_handled();
                }
                _ => {}
            },
            // 指针移进按钮区：与键盘共用同一个「当前按钮」，避免两套高亮打架。
            // 节点模式下悬停视觉由框架状态驱动（按钮自己是交互目标），不在此另记一份。
            Event::PointerMove { pos } if !node_mode => {
                if let Some(b) = self.hit_button_in(ctx.rect(), *pos)
                    && self.focused_button != Some(b)
                {
                    self.focused_button = Some(b);
                    ctx.invalidate_paint();
                }
            }
            // 焦点回来时若还没选过按钮，补上默认按钮（避免「有焦点但按钮区无高亮」）。
            Event::FocusIn { .. } if !node_mode && self.focused_button.is_none() => {
                self.focused_button = self.default_button();
                ctx.invalidate_paint();
            }
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

    // 按钮的合成项（`access_items`）已删：`show_modal*` 现在把按钮插成真节点，
    // 框架会按树序给它们各自的 `accessibility()` —— 再合成一份就是读屏里出现两遍按钮。
    // 按钮的槽位身份 / 默认标记随 `MetroDialogButton` 走，名字与矩形都只有一处来源。
}

/// ← / → 在命令区子节点间挪焦点（到端循环），移动的是**框架焦点**。
///
/// 只挪焦点、不另记「当前按钮」：记两份的后果是键盘与指针各亮一个、Enter 与高亮对不上。
/// 焦点不在命令区（例如落在盒体或内容区）时，→ 落到首个、← 落到末个。
fn step_command_focus(ctx: &mut EventCtx, commands: &[WidgetId], delta: isize) {
    if commands.is_empty() {
        return;
    }
    let n = commands.len() as isize;
    let cur = ctx
        .focus_id()
        .and_then(|f| commands.iter().position(|c| *c == f));
    let next = match cur {
        Some(i) => (i as isize + delta).rem_euclid(n) as usize,
        None if delta < 0 => (n - 1) as usize,
        None => 0,
    };
    ctx.focus_widget(commands[next], true);
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
        // 按钮是真节点（命令区子节点），按树序报出 = 视觉左→右：Primary → Close。
        let names: Vec<&str> = node
            .children
            .iter()
            .filter(|c| c.role == Some(kanesumi_element::AccessRole::Button))
            .map(|c| c.name.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(
            names,
            vec!["确定（默认）", "取消"],
            "按钮按左→右报出，默认按钮带后缀"
        );
        let primary = node
            .children
            .iter()
            .find(|c| c.name.as_deref() == Some("确定（默认）"))
            .expect("默认按钮节点");
        assert!(
            primary.focusable,
            "按钮是 Tab 停靠位（真节点化之前 Tab 只能落在盒体上）"
        );
        assert_eq!(primary.bounds.size.height, 32.0, "按钮高 32");
        assert!(
            node.items.is_empty(),
            "按钮已是真节点，不得再合成一份假项（否则读屏里出现两遍）：{:?}",
            node.items
        );
        h.assert_access_contract();
    }

    /// 命令区里槽位为 `slot` 的按钮节点。
    fn button_node(h: &TestHarness, popup: WidgetId, slot: DialogButton) -> WidgetId {
        h.tree
            .children(popup)
            .iter()
            .copied()
            .find(|c| h.tree.get::<MetroDialogButton>(*c).is_some_and(|b| b.slot == slot))
            .unwrap_or_else(|| panic!("没有槽位 {slot:?} 的按钮节点"))
    }

    #[test]
    fn dialog_button_hit_matches_accessibility_bounds() {
        // 排布 / 命中 / 无障碍三处共用 `command_row_rects`，所以「看见的按钮」=「能点的位置」
        // =「报出去的矩形」。
        let (h, popup, _) = content_harness(Size::new(120.0, 40.0));
        let snap = h.access_snapshot();
        let node = snap.find(popup).expect("对话框在快照内");
        let buttons: Vec<&kanesumi_element::access::AccessNode> = node
            .children
            .iter()
            .filter(|c| c.role == Some(kanesumi_element::AccessRole::Button))
            .collect();
        assert_eq!(buttons.len(), 2, "两个已配置按钮都要报出");
        for item in buttons {
            let p = item.bounds.center();
            let hit = h.tree.hit(p).expect("按钮中心必须命中");
            assert_eq!(
                hit, item.id,
                "无障碍报出的按钮矩形中心必须命中该按钮节点本人：{:?}",
                item.name
            );
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

    /// 取本帧发出的对话框结果（只要槽位）。
    ///
    /// 不核对「发出者是否在对话框子树内」：结果发出后弹层已关闭、子树已回收，此时按 id
    /// 查祖先必然落空。发出者身份由「点哪个按钮就由哪个按钮报」那条用例单独钉住。
    fn take_results(h: &mut TestHarness) -> Vec<DialogButton> {
        h.take::<DialogResult>().into_iter().map(|(_, r)| r.0).collect()
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
        // 打开即聚焦**默认按钮**（不是盒体）：Enter 才直接生效。焦点仍在弹层子树内 = 焦点陷阱成立。
        let f = h.tree.focused().expect("打开即有焦点");
        assert!(
            h.tree.is_ancestor_or_self(d, f),
            "打开即聚焦弹层（或其按钮）"
        );
        assert_eq!(
            h.tree.get::<MetroDialogButton>(f).map(|b| b.slot),
            Some(DialogButton::Primary),
            "落点是默认按钮"
        );
        h.tab();
        h.tab();
        let f = h.tree.focused().expect("Tab 之后仍有焦点");
        assert!(h.tree.is_ancestor_or_self(d, f), "Tab 不得逃出对话框");
    }

    #[test]
    fn click_close_button_reports_result() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        let close = button_node(&h, d, DialogButton::Close);
        h.click(close);
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(close, DialogResult(DialogButton::Close))],
            "点哪个按钮就由哪个按钮报结果"
        );
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn enter_is_primary_and_escape_is_close() {
        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        let primary = button_node(&h, d, DialogButton::Primary);
        h.key(Key::Enter);
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(primary, DialogResult(DialogButton::Primary))],
            "Enter 由**焦点按钮**发出（不是盒体代发）"
        );
        assert!(h.tree.popups().next().is_none());

        let (mut h, opener) = harness();
        let d = open(&mut h, opener);
        h.key(Key::Escape);
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(d, DialogResult(DialogButton::Close))],
            "Esc 没有对应按钮，由对话框本体发 Close"
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

    // ── 按钮键盘导航（←/→ + Enter，第五批）──────────────────────────────────

    /// 三按钮对话框：Primary 默认、Secondary、Close 全部配置。
    fn three_button_dialog() -> (TestHarness, WidgetId) {
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
                ..LayoutProps::default()
            },
        );
        h.frame();
        let popup = open(&mut h, opener);
        h.frame();
        (h, popup)
    }

    /// 「当前按钮」= **框架焦点**落在哪个命令区按钮上（不再读 `MetroDialog::focused_button`：
    /// 那个字段只服务即时模式，节点模式下由框架焦点说了算）。
    fn focused_of(h: &TestHarness, popup: WidgetId) -> Option<DialogButton> {
        let f = h.tree.focused()?;
        if !h.tree.is_ancestor_or_self(popup, f) {
            return None;
        }
        h.tree.get::<MetroDialogButton>(f).map(|b| b.slot)
    }

    /// `show_modal_with`：开弹层 + 填内容一次写完（应用不再需要 `parts_mut`）。
    #[test]
    fn show_modal_with_builds_content_in_one_call() {
        struct Builder;
        impl Widget for Builder {
            fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
                Size::new(80.0, 32.0)
            }
            fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
            fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
                if let Event::Click = event {
                    let mut dlg = MetroDialog::new("重命名", "");
                    dlg.buttons.primary = Some("确定".into());
                    dlg.show_modal_with(ctx, |ctx, popup| {
                        ctx.insert_child_with(
                            popup,
                            kanesumi_element::widgets::Border::new().background(kanesumi_core::Color::rgb(0.4, 0.4, 0.4)),
                            LayoutProps {
                                width: Some(200.0),
                                height: Some(60.0),
                                ..LayoutProps::default()
                            },
                        );
                    });
                    ctx.set_handled();
                }
            }
            fn focusable(&self) -> bool {
                true
            }
            fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
                Some(kanesumi_element::AccessInfo {
                    role: kanesumi_element::AccessRole::Button,
                    name: "打开".into(),
                    value: None,
                    checked: None,
                })
            }
        }

        let mut h = TestHarness::new(600.0, 500.0);
        let opener = h.tree.insert(h.root(), Builder);
        h.frame();
        h.click(opener);
        h.frame();

        let popup = h.tree.popups().next().expect("对话框已打开");
        let kids = h.tree.children(popup).to_vec();
        let contents: Vec<WidgetId> = kids
            .iter()
            .copied()
            .filter(|c| !is_command(&h.tree.props(*c).unwrap_or_default()))
            .collect();
        assert_eq!(contents.len(), 1, "内容在闭包里挂进了对话框");
        assert_eq!(
            h.rect(contents[0]).size,
            Size::new(200.0, 60.0),
            "内容按给定尺寸排布"
        );
        // 按钮也在同一次调用里插好了，且**插在命令区**（否则它会挤进内容区）。
        assert_eq!(
            kids.len(),
            2,
            "内容一个 + 已配置按钮一个（按钮由 show_modal_with 自动插）"
        );
        h.assert_access_contract();
    }

    #[test]
    fn opens_with_default_button_selected() {
        let (h, popup) = three_button_dialog();
        assert_eq!(
            focused_of(&h, popup),
            Some(DialogButton::Primary),
            "打开即落在默认按钮上（否则按钮区没有任何高亮）"
        );
    }

    #[test]
    fn arrow_keys_move_between_configured_buttons_and_wrap() {
        let (mut h, popup) = three_button_dialog();
        h.key(Key::Right);
        assert_eq!(focused_of(&h, popup), Some(DialogButton::Secondary));
        h.key(Key::Right);
        assert_eq!(
            focused_of(&h, popup),
            Some(DialogButton::Close),
            "视觉顺序在右"
        );
        h.key(Key::Right);
        assert_eq!(focused_of(&h, popup), Some(DialogButton::Primary), "到尾循环");
        h.key(Key::Left);
        assert_eq!(focused_of(&h, popup), Some(DialogButton::Close), "反向循环");
    }

    #[test]
    fn arrow_keys_skip_unconfigured_buttons() {
        let mut h = TestHarness::new(600.0, 500.0);
        let mut dlg = MetroDialog::new("删除？", "不可撤销");
        dlg.buttons.primary = Some("删除".into());
        dlg.buttons.close = Some("取消".into());
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
        h.frame();
        h.key(Key::Right);
        assert_eq!(
            focused_of(&h, popup),
            Some(DialogButton::Close),
            "未配置 Secondary 时不占位（Primary 之后直接 Close）"
        );
    }

    #[test]
    fn enter_activates_the_focused_button() {
        let (mut h, popup) = three_button_dialog();
        h.key(Key::Right); // → Secondary
        assert_eq!(focused_of(&h, popup), Some(DialogButton::Secondary), "先确认焦点走到了 Secondary");
        h.key(Key::Enter);
        assert_eq!(
            take_results(&mut h),
            vec![DialogButton::Secondary],
            "Enter 激活当前按钮，而不是永远 Primary"
        );
        assert!(h.tree.popups().next().is_none(), "激活后关闭");
    }

    #[test]
    fn space_activates_and_esc_still_closes() {
        let (mut h, popup) = three_button_dialog();
        let primary = button_node(&h, popup, DialogButton::Primary);
        h.key(Key::Char(' '));
        assert_eq!(
            h.take::<DialogResult>(),
            vec![(primary, DialogResult(DialogButton::Primary))],
            "Space 同 Enter，由焦点按钮发出"
        );

        let (mut h2, p2) = three_button_dialog();
        h2.key(Key::Escape);
        assert_eq!(
            h2.take::<DialogResult>(),
            vec![(p2, DialogResult(DialogButton::Close))],
            "Esc 仍是关闭"
        );
        assert!(h2.tree.popups().next().is_none());
    }

    #[test]
    fn focused_button_is_drawn_and_reported_to_at() {
        let (mut h, popup) = three_button_dialog();
        // 指针开窗后焦点视觉处于「指针态」（不画环，与 UWP 一致）；按一次 → 转键盘态，
        // 焦点也走到 Secondary。
        h.key(Key::Right);
        let f = h.tree.focused().expect("有焦点");
        assert_eq!(
            h.tree.get::<MetroDialogButton>(f).map(|b| b.slot),
            Some(DialogButton::Secondary),
            "键盘焦点在命令区按钮上"
        );
        let snap = h.access_snapshot();
        assert_eq!(
            snap.find(f).and_then(|n| n.name.clone()).as_deref(),
            Some("不保存"),
            "读屏播报的是焦点按钮本人的名字"
        );
        let primary = button_node(&h, popup, DialogButton::Primary);
        assert_eq!(
            snap.find(primary).and_then(|n| n.name.clone()).as_deref(),
            Some("保存（默认）"),
            "默认按钮的名字带默认标记"
        );

        // 焦点环由框架画在**焦点节点自己**身上（真节点化后不再由盒体代画）：
        // 弹层表面 = 盒体描边 1 条 + 焦点环 2 条（外 2px + 内 1px）。
        let scene = h.tree.popup_scene(popup).expect("弹层表面");
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::StrokeRect { .. }))
            .count();
        assert!(strokes >= 3, "盒体描边 + 双层焦点环（实得 {strokes} 条描边）");
        h.assert_access_contract();
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
