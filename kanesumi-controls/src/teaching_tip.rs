// MetroTeachingTip —— 新特性引导气泡。参 CONTROL_SPEC §26。
//
// 移植自 microsoft-ui-xaml/dev/TeachingTip（TeachingTip.xaml + TeachingTip_rs1/rs2_themeresources.xaml）：
// - 面板 MinW 320 / MaxW 336、MinH 40 / MaxH 520、ContentMargin 12、Border 1；
// - Title 14 SemiBold / Subtitle 14；Alternate Close 40×40（× 16）；
// - Row2 [Action][Close]；Tail 三角指向目标；
// - 放置：四方向取可用空间最大侧；开合淡入淡出（0.167s / 0.083s）。

use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size};
use kanesumi_element::{
    Event, EventCtx, Key, MeasureCtx, PaintCtx, PointerButton, PopupSpec, Widget, WidgetId,
};

/// 面板最小宽（320）。
pub const TEACHING_MIN_W: f32 = 320.0;
/// 面板最大宽（336）。
pub const TEACHING_MAX_W: f32 = 336.0;
/// ContentMargin（12）。
pub const TEACHING_CONTENT_MARGIN: f32 = 12.0;
/// Alternate Close 边长（40）。
pub const TEACHING_CLOSE_SIZE: f32 = 40.0;

/// 放置侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeachingTipSide {
    Top,
    Bottom,
    Left,
    Right,
}

/// 点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeachingTipClick {
    None,
    Action,
    Close,
}

/// 放置结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TeachingTipPlacement {
    pub rect: Rect,
    pub side: TeachingTipSide,
}

/// 放置：四方向取可用空间最大的一侧。面板 320×`panel_h`，贴目标侧、Tail 对齐目标中心。
pub fn place_teaching_tip(target: Rect, panel_h: f32, screen: Rect) -> TeachingTipPlacement {
    let w = TEACHING_MIN_W;
    let below = screen.bottom() - target.bottom();
    let above = target.origin.y - screen.origin.y;
    let right = screen.right() - target.right();
    let left = target.origin.x - screen.origin.x;
    // 空间最大侧
    let side = {
        let mut best = TeachingTipSide::Bottom;
        let mut best_space = below;
        if above > best_space {
            best = TeachingTipSide::Top;
            best_space = above;
        }
        if right > best_space {
            best = TeachingTipSide::Right;
            best_space = right;
        }
        if left > best_space {
            best = TeachingTipSide::Left;
        }
        best
    };
    let rect = match side {
        TeachingTipSide::Bottom => {
            let x = (target.center().x - w / 2.0).clamp(screen.origin.x, screen.right() - w);
            Rect::new(x, target.bottom(), w, panel_h)
        }
        TeachingTipSide::Top => {
            let x = (target.center().x - w / 2.0).clamp(screen.origin.x, screen.right() - w);
            Rect::new(x, target.origin.y - panel_h, w, panel_h)
        }
        TeachingTipSide::Left => Rect::new(
            target.origin.x - w,
            target.center().y - panel_h / 2.0,
            w,
            panel_h,
        ),
        TeachingTipSide::Right => Rect::new(
            target.right(),
            target.center().y - panel_h / 2.0,
            w,
            panel_h,
        ),
    };
    TeachingTipPlacement { rect, side }
}

/// MetroTeachingTip —— 引导气泡。参 CONTROL_SPEC §26。
#[derive(Debug, Clone)]
pub struct MetroTeachingTip {
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub action_label: Option<String>,
    /// 显示右上 ×。
    pub closable: bool,
    /// 是否打开。
    pub open: bool,
    /// 当前放置（None = 未放置/已关）。
    pub placement: Option<TeachingTipPlacement>,
    pub action_hovered: bool,
    pub close_hovered: bool,
    /// 淡入/淡出动画。
    anim: MetroAnim,
}

impl Default for MetroTeachingTip {
    fn default() -> Self {
        Self {
            title: String::new(),
            subtitle: String::new(),
            body: String::new(),
            action_label: None,
            closable: true,
            open: false,
            placement: None,
            action_hovered: false,
            close_hovered: false,
            anim: MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut),
        }
    }
}

impl MetroTeachingTip {
    pub fn new() -> Self {
        Self::default()
    }

    /// 面板高度（内容驱动，Min 40 / Max 520 之外夹紧）。
    pub fn panel_height(&self, engine: &TextEngine) -> f32 {
        let style = Self::body_style();
        let mut h = TEACHING_CONTENT_MARGIN * 2.0;
        if !self.title.is_empty() {
            h += style.line_height + 4.0;
        }
        if !self.subtitle.is_empty() {
            h += style.line_height;
        }
        if !self.body.is_empty() {
            h += engine
                .layout(
                    &self.body,
                    style.size,
                    TEACHING_MAX_W - TEACHING_CONTENT_MARGIN * 2.0,
                )
                .len() as f32
                * style.line_height;
        }
        if self.action_label.is_some() {
            h += 32.0 + 12.0;
        }
        h.clamp(40.0, 520.0)
    }

    fn title_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Semibold)
    }

    fn body_style() -> TextStyle {
        MetroTypography::metro().body_medium
    }

    /// 打开：计算放置 + 淡入。
    pub fn open(&mut self, engine: &TextEngine, target: Rect, screen: Rect) {
        let panel_h = self.panel_height(engine);
        self.placement = Some(place_teaching_tip(target, panel_h, screen));
        self.open = true;
        self.anim = MetroAnim::new(0.167, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.anim.set_target(1.0);
    }

    /// 关闭：淡出（动画后 `open=false`）。
    pub fn close(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.anim = MetroAnim::new(0.083, UwpEasing::Quadratic, EasingMode::EaseOut);
        self.anim.set_target(0.0);
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
        if self.anim.is_steady() && !self.open {
            self.placement = None;
        }
    }

    pub fn is_visible(&self) -> bool {
        self.open || self.anim.value() > 0.0
    }

    /// 当前淡入进度 [0,1]。
    pub fn alpha(&self) -> f32 {
        self.anim.value() as f32
    }

    /// 关闭按钮 rect（面板右上）。
    pub fn close_rect(&self) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        let p = self.placement?;
        Some(Rect::new(
            p.rect.right() - TEACHING_CLOSE_SIZE,
            p.rect.origin.y,
            TEACHING_CLOSE_SIZE,
            TEACHING_CLOSE_SIZE,
        ))
    }

    /// 操作按钮 rect（面板左下）。
    pub fn action_rect(&self) -> Option<Rect> {
        let _ = self.action_label.as_ref()?;
        let p = self.placement?;
        Some(Rect::new(
            p.rect.origin.x + TEACHING_CONTENT_MARGIN,
            p.rect.bottom() - TEACHING_CONTENT_MARGIN - 32.0,
            TEACHING_MAX_W / 2.0 - TEACHING_CONTENT_MARGIN,
            32.0,
        ))
    }

    /// 命中。
    pub fn hit(&self, pos: Point) -> TeachingTipClick {
        if let Some(c) = self.close_rect()
            && c.contains(pos)
        {
            return TeachingTipClick::Close;
        }
        if let Some(a) = self.action_rect()
            && a.contains(pos)
        {
            return TeachingTipClick::Action;
        }
        TeachingTipClick::None
    }

    /// 悬停路由。
    pub fn hover(&mut self, pos: Point) {
        self.action_hovered = self.hit(pos) == TeachingTipClick::Action;
        self.close_hovered = self.hit(pos) == TeachingTipClick::Close;
    }

    /// 处理点击：Action / Close → 关闭并返回。
    pub fn handle_click(&mut self, pos: Point) -> TeachingTipClick {
        let click = self.hit(pos);
        if click != TeachingTipClick::None {
            self.close();
        }
        click
    }

    /// 渲染气泡（面板 + Tail + 内容）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, scene: &mut Scene) {
        if !self.is_visible() {
            return;
        }
        let Some(p) = self.placement else { return };
        let colors = &theme.colors;
        let a = self.alpha();

        // 面板（半透明 surface）
        let bg = colors.surface_variant.with_alpha(a);
        scene.fill_rounded_rect(bg, p.rect, theme.tokens.corner_radius);
        scene.stroke_rounded_rect(
            colors.divider.with_alpha(a),
            p.rect,
            1.0,
            theme.tokens.corner_radius,
        );

        let title = Self::title_style();
        let body = Self::body_style();
        let mut y = p.rect.origin.y + TEACHING_CONTENT_MARGIN;

        // Title
        if !self.title.is_empty() {
            scene.text(
                self.title.clone(),
                Rect::new(
                    p.rect.origin.x + TEACHING_CONTENT_MARGIN,
                    y,
                    p.rect.size.width - TEACHING_CONTENT_MARGIN * 2.0 - 28.0,
                    title.line_height,
                ),
                colors.on_surface.with_alpha(a),
                title,
                TextAlign::Left,
            );
            y += title.line_height + 4.0;
        }
        // Subtitle
        if !self.subtitle.is_empty() {
            scene.text(
                self.subtitle.clone(),
                Rect::new(
                    p.rect.origin.x + TEACHING_CONTENT_MARGIN,
                    y,
                    p.rect.size.width - TEACHING_CONTENT_MARGIN * 2.0,
                    body.line_height,
                ),
                colors.on_surface.with_alpha(a),
                body,
                TextAlign::Left,
            );
            y += body.line_height;
        }
        // Body
        if !self.body.is_empty() {
            scene.text(
                self.body.clone(),
                Rect::new(
                    p.rect.origin.x + TEACHING_CONTENT_MARGIN,
                    y,
                    p.rect.size.width - TEACHING_CONTENT_MARGIN * 2.0,
                    body.line_height,
                ),
                colors.on_surface_variant.with_alpha(a),
                body,
                TextAlign::Left,
            );
        }

        // Action 按钮
        if let Some(label) = &self.action_label
            && let Some(ar) = self.action_rect()
        {
            if self.action_hovered {
                scene.fill_rounded_rect(
                    theme
                        .indication
                        .press_subtle_tint
                        .with_alpha(theme.indication.press_subtle_tint.a * a),
                    ar,
                    theme.tokens.corner_radius,
                );
            }
            let style = body;
            scene.text(
                label.clone(),
                Rect::new(
                    ar.origin.x + 8.0,
                    ar.origin.y + (ar.size.height - style.line_height) / 2.0,
                    ar.size.width - 16.0,
                    style.line_height,
                ),
                colors.primary.with_alpha(a),
                style,
                TextAlign::Left,
            );
        }

        // Close（× 自绘）
        if let Some(c) = self.close_rect() {
            if self.close_hovered {
                scene.fill_rounded_rect(
                    theme
                        .indication
                        .press_subtle_tint
                        .with_alpha(theme.indication.press_subtle_tint.a * a),
                    c,
                    theme.tokens.corner_radius,
                );
            }
            draw_tip_close(scene, c, colors.on_surface_variant.with_alpha(a));
        }

        // Tail（指向目标）
        let tail = self.tail_points(p);
        scene.triangle(tail.0, tail.1, tail.2, bg);
        let _ = Size::ZERO;
    }

    /// Tail 三角（根据侧向）。
    fn tail_points(&self, p: TeachingTipPlacement) -> (Point, Point, Point) {
        let cx = p.rect.center().x;
        let cy = p.rect.center().y;
        const S: f32 = 8.0;
        match p.side {
            TeachingTipSide::Bottom => (
                Point::new(cx - S, p.rect.origin.y),
                Point::new(cx + S, p.rect.origin.y),
                Point::new(cx, p.rect.origin.y - S),
            ),
            TeachingTipSide::Top => (
                Point::new(cx - S, p.rect.bottom()),
                Point::new(cx + S, p.rect.bottom()),
                Point::new(cx, p.rect.bottom() + S),
            ),
            TeachingTipSide::Left => (
                Point::new(p.rect.right(), cy - S),
                Point::new(p.rect.right(), cy + S),
                Point::new(p.rect.right() + S, cy),
            ),
            TeachingTipSide::Right => (
                Point::new(p.rect.origin.x, cy - S),
                Point::new(p.rect.origin.x, cy + S),
                Point::new(p.rect.origin.x - S, cy),
            ),
        }
    }
}

/// 自绘 ×（右上关闭）。
fn draw_tip_close(scene: &mut Scene, rect: Rect, color: kanesumi_core::Color) {
    let cx = rect.center().x;
    let cy = rect.center().y;
    let r = 6.0;
    let t = 1.6;
    scene.triangle(
        Point::new(cx - r, cy - r + t),
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r - t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r, cy + r - t),
        Point::new(cx + r - t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx + r - t, cy - r),
        Point::new(cx + r, cy - r + t),
        Point::new(cx - r + t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx + r, cy - r + t),
        Point::new(cx + r - t, cy + r),
        Point::new(cx - r, cy + r - t),
        color,
    );
}

// ── 元素树接入（弹层类控件，参 docs/ELEMENT_MIGRATION.md §8）────────────────────
//
// 气泡本身即弹层节点（`MetroTeachingTip` 实现 `Widget`）：以目标控件为锚点、非模态
// 挂到覆盖层。关闭键 / 动作键各发一个动作并关闭自身。`light_dismiss` 按旧语义 ——
// 有关闭键的气泡要显式关（不可轻触关闭），没有关闭键的才允许点外部关闭。
//
// 放置：框架 `PopupSpec` 只支持「锚点下缘 + 放不下上翻」，故气泡只实现「下方」这一侧；
// 旧 `place_teaching_tip` 的四向择空能力（上 / 左 / 右）待框架补齐后接入（详见本批报告）。

/// 元素树动作：气泡被关闭键关闭。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeachingTipClosed;

/// 元素树动作：气泡的动作键被激活。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TeachingTipAction;

/// 气泡与目标之间的间隙（= Tail 高度，让三角尖端顶到目标下缘）。
const TIP_GAP: f32 = 8.0;

impl MetroTeachingTip {
    /// 以 `target` 为锚点打开气泡（非模态）。返回弹层 id。
    ///
    /// 内容复制进弹层节点：`self` 仍是 App 持有的定义，`show` 可重复调用。
    pub fn show(&self, ctx: &mut EventCtx, target: WidgetId) -> WidgetId {
        let mut tip = self.clone();
        tip.open = true;
        // 元素树不做淡入（节点出现即终态）：直接把淡入轨道置到 1，避免首帧 alpha=0。
        tip.anim.jump_to(1.0);
        ctx.open_popup(
            tip,
            PopupSpec {
                anchor: Some(target),
                gap: TIP_GAP,
                // 有关闭键 → 需显式关闭；无关闭键 → 允许轻触关闭。
                light_dismiss: !self.closable,
                ..PopupSpec::default()
            },
        )
    }

    /// 触发一次结果：发对应动作并关闭弹层。`action` = 是否按住动作键语义。
    fn finish(&self, ctx: &mut EventCtx, action: bool) {
        if action {
            ctx.emit(TeachingTipAction);
        } else {
            ctx.emit(TeachingTipClosed);
        }
        ctx.close_popup(ctx.id());
    }
}

impl Widget for MetroTeachingTip {
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        Size::new(TEACHING_MIN_W, self.panel_height(ctx.engine()))
    }

    /// 记下自身矩形供旧 `render` 使用：气泡恒为「下方」放置（Tail 指向上方目标）。
    fn arrange(&mut self, _ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.placement = Some(TeachingTipPlacement {
            rect,
            side: TeachingTipSide::Bottom,
        });
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), scene);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::PointerMove { pos } => {
                self.hover(*pos);
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                match self.hit(*pos) {
                    TeachingTipClick::Action => self.finish(ctx, true),
                    TeachingTipClick::Close => self.finish(ctx, false),
                    TeachingTipClick::None => {}
                }
                ctx.set_handled();
            }
            Event::KeyDown {
                key: Key::Enter | Key::Char(' '),
                ..
            } => {
                self.finish(ctx, self.action_label.is_some());
                ctx.set_handled();
            }
            // Tab / Esc 留给框架（焦点遍历 / 轻触关闭）。
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 气泡不画框架焦点框（按钮由自身 hover / 强调色表达）。
    fn focus_visual(&self) -> bool {
        false
    }

    /// Tail 三角画在气泡上方 8px。
    fn paint_overflow(&self) -> kanesumi_element::Insets {
        kanesumi_element::Insets::new(0.0, 8.0, 0.0, 0.0)
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: self.title.clone(),
            value: Some(self.body.clone()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps};

    /// 目标锚点（气泡指向它）。
    struct Anchor;

    impl Widget for Anchor {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(100.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn hit_test(&self, rect: Rect, pos: Point) -> bool {
            rect.contains(pos)
        }
    }

    /// 触发器：点击时以 `target` 为锚点弹出气泡。
    struct Opener {
        tip: MetroTeachingTip,
        target: WidgetId,
    }

    impl Widget for Opener {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(80.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            if let Event::Click = event {
                self.tip.show(ctx, self.target);
                ctx.set_handled();
            }
        }
        fn focusable(&self) -> bool {
            true
        }
    }

    fn tip(action: bool) -> MetroTeachingTip {
        MetroTeachingTip {
            title: "新功能".into(),
            body: "试试这个引导气泡".into(),
            action_label: action.then(|| "知道了".to_string()),
            ..MetroTeachingTip::default()
        }
    }

    fn harness(action: bool, closable: bool) -> (TestHarness, WidgetId, WidgetId) {
        let mut h = TestHarness::new(500.0, 400.0);
        let target = h.tree.insert_with(
            h.root(),
            Anchor,
            LayoutProps {
                width: Some(100.0),
                height: Some(32.0),
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(40.0, 40.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        let mut t = tip(action);
        t.closable = closable;
        let opener = h.tree.insert_with(
            h.root(),
            Opener { tip: t, target },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(40.0, 200.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, opener, target)
    }

    #[test]
    fn opens_below_target_and_close_key_reports_and_closes() {
        let (mut h, opener, target) = harness(false, true);
        h.click(opener);
        let p = h.tree.popups().next().expect("点击打开气泡");
        let pr = h.rect(p);
        assert!(
            pr.origin.y >= h.rect(target).bottom(),
            "气泡在目标下方 {pr:?} vs {:?}",
            h.rect(target)
        );
        let close = h.tree.get::<MetroTeachingTip>(p).unwrap().close_rect().unwrap();
        h.click_at(close.center());
        assert_eq!(h.take::<TeachingTipClosed>(), vec![(p, TeachingTipClosed)]);
        assert!(h.tree.popups().next().is_none(), "关闭键后弹层移除");
    }

    #[test]
    fn action_key_reports_action_and_closes() {
        let (mut h, opener, _) = harness(true, true);
        h.click(opener);
        let p = h.tree.popups().next().unwrap();
        let action = h
            .tree
            .get::<MetroTeachingTip>(p)
            .unwrap()
            .action_rect()
            .unwrap();
        h.click_at(action.center());
        assert_eq!(h.take::<TeachingTipAction>(), vec![(p, TeachingTipAction)]);
        assert!(h.tree.popups().next().is_none());
    }

    /// 有关闭键（closable）→ 不可轻触关闭：点外部仍在。
    #[test]
    fn closable_tip_is_not_light_dismissed() {
        let (mut h, opener, _) = harness(false, true);
        h.click(opener);
        assert!(h.tree.popups().next().is_some());
        h.click_at(Point::new(480.0, 390.0));
        assert!(h.tree.popups().next().is_some(), "有关闭键不可轻触关闭");
    }

    /// 无关闭键 → 允许点外部关闭。
    #[test]
    fn non_closable_tip_is_light_dismissed() {
        let (mut h, opener, _) = harness(false, false);
        h.click(opener);
        assert!(h.tree.popups().next().is_some());
        h.click_at(Point::new(480.0, 390.0));
        assert!(h.tree.popups().next().is_none(), "无关闭键允许轻触关闭");
    }

    #[test]
    fn keyboard_enter_activates_action() {
        let (mut h, opener, _) = harness(true, true);
        h.click(opener);
        // 关闭键气泡打开后焦点在气泡（键盘聚焦）→ Enter 触发动作键。
        let p = h.tree.popups().next().unwrap();
        h.tree.focus(p, true);
        h.key(Key::Enter);
        assert_eq!(h.take::<TeachingTipAction>().len(), 1);
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, opener, _) = harness(true, true);
        h.click(opener);
        let p = h.tree.popups().next().unwrap();
        h.assert_contained();
        h.assert_paint_within(p, Insets::new(0.0, 8.0, 0.0, 0.0));
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

    fn tip() -> MetroTeachingTip {
        MetroTeachingTip {
            title: "新功能".into(),
            body: "试试这个引导气泡".into(),
            action_label: Some("知道了".into()),
            ..MetroTeachingTip::default()
        }
    }

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 1024.0, 768.0)
    }

    #[test]
    fn placement_bottom_by_default() {
        // 目标在上方 → 下方空间最大
        let target = Rect::new(412.0, 100.0, 100.0, 40.0);
        let p = place_teaching_tip(target, 120.0, screen());
        assert_eq!(p.side, TeachingTipSide::Bottom);
        assert_eq!(p.rect.origin.y, 140.0);
        assert_eq!(p.rect.size.width, 320.0);
    }

    #[test]
    fn placement_top_when_no_bottom() {
        let target = Rect::new(400.0, 700.0, 100.0, 40.0);
        let p = place_teaching_tip(target, 120.0, screen());
        assert_eq!(p.side, TeachingTipSide::Top);
        assert_eq!(p.rect.bottom(), 700.0);
    }

    #[test]
    fn placement_right_when_vertical_tight() {
        // 小屏：上下空间都不足 → 左右
        let target = Rect::new(100.0, 150.0, 50.0, 30.0);
        let small = Rect::new(0.0, 0.0, 500.0, 200.0);
        let p = place_teaching_tip(target, 120.0, small);
        assert_eq!(p.side, TeachingTipSide::Right);
    }

    #[test]
    fn open_sets_placement() {
        let Some(engine) = find_engine() else { return };
        let mut t = tip();
        t.open(&engine, Rect::new(400.0, 400.0, 100.0, 40.0), screen());
        assert!(t.open);
        assert!(t.placement.is_some());
        assert!(t.is_visible());
    }

    #[test]
    fn close_fades_then_hides() {
        let Some(engine) = find_engine() else { return };
        let mut t = tip();
        t.open(&engine, Rect::new(400.0, 400.0, 100.0, 40.0), screen());
        t.close();
        assert!(!t.open);
        for _ in 0..120 {
            t.update(1.0 / 60.0);
        }
        assert_eq!(t.placement, None, "动画结束后放置清空");
    }

    #[test]
    fn hit_action_and_close() {
        let Some(engine) = find_engine() else { return };
        let mut t = tip();
        t.open(&engine, Rect::new(400.0, 400.0, 100.0, 40.0), screen());
        let a = t.action_rect().unwrap();
        assert_eq!(t.hit(a.center()), TeachingTipClick::Action);
        let c = t.close_rect().unwrap();
        assert_eq!(t.hit(c.center()), TeachingTipClick::Close);
        assert_eq!(t.hit(Point::new(500.0, 200.0)), TeachingTipClick::None);
    }

    #[test]
    fn handle_click_closes() {
        let Some(engine) = find_engine() else { return };
        let mut t = tip();
        t.open(&engine, Rect::new(400.0, 400.0, 100.0, 40.0), screen());
        let c = t.close_rect().unwrap();
        assert_eq!(t.handle_click(c.center()), TeachingTipClick::Close);
        assert!(!t.open);
    }

    #[test]
    fn panel_height_grows_with_content() {
        let Some(engine) = find_engine() else { return };
        let empty = MetroTeachingTip::default();
        let h1 = empty.panel_height(&engine);
        let t = tip();
        let h2 = t.panel_height(&engine);
        assert!(h2 > h1, "有内容更高");
    }

    #[test]
    fn render_emits_when_open() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut t = tip();
        t.open(&engine, Rect::new(400.0, 400.0, 100.0, 40.0), screen());
        t.update(1.0);
        let mut scene = Scene::default();
        t.render(&theme, &engine, &mut scene);
        assert!(!scene.is_empty());
        // 文本 ≥ 2（title + body）
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 2, "title + body，实际 {texts}");
    }

    #[test]
    fn closed_renders_nothing() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let t = MetroTeachingTip::default();
        let mut scene = Scene::default();
        t.render(&theme, &engine, &mut scene);
        assert!(scene.is_empty());
    }
}
