// MetroSwipeControl —— 滑动手势操作项（Reveal 模式）。参 CONTROL_SPEC §32。
//
// 移植自 microsoft-ui-xaml/dev/SwipeControl（SwipeControl.cpp + SwipeControl.idl）：
// - LeftItems/RightItems 滑动露出；Mode Reveal（拖出操作项）vs Execute（拖出即触发）；
// - 释放：越过阈值吸合展开，否则回弹；
// - 点操作项 → Invoke；点内容 → Close。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect};

/// 操作项宽。
pub const SWIPE_ITEM_W: f32 = 64.0;
/// 吸合阈值（拖动超过项区一半 → 展开）。
pub const SWIPE_SNAP_THRESHOLD: f32 = 0.5;

/// 滑动模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeMode {
    /// 拖出操作项。
    Reveal,
    /// 拖出即触发首项。
    Execute,
}

/// 操作项。
#[derive(Debug, Clone, PartialEq)]
pub struct SwipeItem {
    pub label: String,
    pub action: SwipeItemAction,
}

/// 操作项类型（决定底色）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeItemAction {
    /// 默认（surface_variant）。
    Default,
    /// 强调（primary 底）。
    Accent,
    /// 危险（error 红）。
    Danger,
}

impl SwipeItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            action: SwipeItemAction::Default,
        }
    }

    pub fn with_action(label: impl Into<String>, action: SwipeItemAction) -> Self {
        Self {
            label: label.into(),
            action,
        }
    }
}

/// 点击结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeAction {
    None,
    /// 点操作项（返回其在对应列表中的索引）。
    Invoke(usize),
    /// 点内容 → 收起。
    Close,
}

/// MetroSwipeControl —— 滑动操作。参 CONTROL_SPEC §32。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSwipeControl {
    /// 当前露出距离（>0 = 露出左侧操作项）。
    pub offset: f32,
    pub left_items: Vec<SwipeItem>,
    pub right_items: Vec<SwipeItem>,
    pub mode: SwipeMode,
    /// 是否吸合展开。
    pub revealed: bool,
    /// hover 的操作项（Left/Right 列表索引）。
    pub hovered: Option<(bool, usize)>,
    drag_start: Option<f32>,
}

impl Default for MetroSwipeControl {
    fn default() -> Self {
        Self {
            offset: 0.0,
            left_items: Vec::new(),
            right_items: Vec::new(),
            mode: SwipeMode::Reveal,
            revealed: false,
            hovered: None,
            drag_start: None,
        }
    }
}

impl MetroSwipeControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// 左侧项区总宽。
    pub fn left_width(&self) -> f32 {
        self.left_items.len() as f32 * SWIPE_ITEM_W
    }

    /// 右侧项区总宽。
    pub fn right_width(&self) -> f32 {
        self.right_items.len() as f32 * SWIPE_ITEM_W
    }

    /// 露出距离夹紧范围：[-right_width, left_width]。
    pub fn clamp_offset(&self, o: f32) -> f32 {
        o.clamp(-self.right_width(), self.left_width())
    }

    /// 开始拖动。
    pub fn press(&mut self, pos: Point) {
        self.drag_start = Some(pos.x);
    }

    /// 拖动：露出距离随 dx 变化。
    pub fn drag_to(&mut self, pos: Point) {
        if let Some(start) = self.drag_start {
            let dx = pos.x - start;
            self.offset = self.clamp_offset(dx);
        }
    }

    /// 释放：越过阈值吸合，否则回弹。
    pub fn release(&mut self) {
        self.drag_start = None;
        let threshold = if self.offset > 0.0 {
            self.left_width() * SWIPE_SNAP_THRESHOLD
        } else {
            self.right_width() * SWIPE_SNAP_THRESHOLD
        };
        let target = if self.offset.abs() >= threshold && threshold > 0.0 {
            if self.offset > 0.0 {
                self.left_width()
            } else {
                -self.right_width()
            }
        } else {
            0.0
        };
        self.offset = target;
        self.revealed = self.offset != 0.0;
        // Execute 模式：拖出即触发并复位
        if self.mode == SwipeMode::Execute && self.offset != 0.0 {
            self.offset = 0.0;
            self.revealed = false;
        }
    }

    /// 收起。
    pub fn close(&mut self) {
        self.offset = 0.0;
        self.revealed = false;
        self.drag_start = None;
    }

    /// 左侧操作项 rect（内容左缘固定，内容右滑露出）。
    fn left_items_rects(&self, rect: Rect) -> Vec<Rect> {
        (0..self.left_items.len())
            .map(|i| {
                Rect::new(
                    rect.origin.x + i as f32 * SWIPE_ITEM_W,
                    rect.origin.y,
                    SWIPE_ITEM_W,
                    rect.size.height,
                )
            })
            .collect()
    }

    /// 命中：操作项（露出时）/ 内容。
    pub fn hit(&self, rect: Rect, pos: Point) -> SwipeAction {
        if self.offset > 0.0 {
            for (i, r) in self.left_items_rects(rect).iter().enumerate() {
                if r.contains(pos) {
                    return SwipeAction::Invoke(i);
                }
            }
        }
        if self.offset < 0.0 {
            // 右侧项：内容右缘固定
            let x0 = rect.right() - self.right_width();
            for i in 0..self.right_items.len() {
                let r = Rect::new(
                    x0 + i as f32 * SWIPE_ITEM_W,
                    rect.origin.y,
                    SWIPE_ITEM_W,
                    rect.size.height,
                );
                if r.contains(pos) {
                    return SwipeAction::Invoke(i);
                }
            }
        }
        if rect.contains(pos) {
            return SwipeAction::Close;
        }
        SwipeAction::None
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.hovered = match self.hit(rect, pos) {
            SwipeAction::Invoke(i) if self.offset > 0.0 => Some((true, i)),
            SwipeAction::Invoke(i) => Some((false, i)),
            _ => None,
        };
    }

    /// 应用点击。
    pub fn handle_click(&mut self, rect: Rect, pos: Point) -> SwipeAction {
        let action = self.hit(rect, pos);
        match action {
            SwipeAction::Invoke(_) | SwipeAction::Close => self.close(),
            SwipeAction::None => {}
        }
        action
    }

    /// 渲染：内容（宿主自绘占位）+ 露出的操作项。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = TextStyle::new(12.0, 16.0, FontWeight::Normal);

        if self.offset > 0.0 {
            for (i, item) in self.left_items.iter().enumerate() {
                let r = Rect::new(
                    rect.origin.x + i as f32 * SWIPE_ITEM_W,
                    rect.origin.y,
                    SWIPE_ITEM_W,
                    rect.size.height,
                );
                let bg = item_bg(item.action, theme);
                scene.fill_rect(bg, r);
                if self.hovered == Some((true, i)) {
                    // 悬停叠白 10%（通用悬停档）。⚠ 一手源：UWP `SwipeItem` 的 PointerOver 是
                    // **空视觉状态**（`generic.xaml` L29495 区段无 Setter）、Pressed 才是中性 40%
                    // （BaseMediumLow）——本库给鼠标加悬停反馈是**有意偏离**（桌面鼠标无按压预期），
                    // 登记于 `CANON_VS_TEMPORARY` T13 同条。
                    scene.fill_rect(theme.indication.hover_tint, r);
                }
                scene.text(
                    item.label.clone(),
                    Rect::new(
                        r.origin.x,
                        r.origin.y + (r.size.height - style.line_height) / 2.0,
                        r.size.width,
                        style.line_height,
                    ),
                    colors.on_surface,
                    style,
                    TextAlign::Center,
                );
            }
        } else if self.offset < 0.0 {
            let x0 = rect.right() - self.right_width();
            for (i, item) in self.right_items.iter().enumerate() {
                let r = Rect::new(
                    x0 + i as f32 * SWIPE_ITEM_W,
                    rect.origin.y,
                    SWIPE_ITEM_W,
                    rect.size.height,
                );
                let bg = item_bg(item.action, theme);
                scene.fill_rect(bg, r);
                if self.hovered == Some((false, i)) {
                    // 同上：悬停叠白 10%（通用悬停档；UWP 的 SwipeItem 无悬停态，本库有意加）。
                    scene.fill_rect(theme.indication.hover_tint, r);
                }
                scene.text(
                    item.label.clone(),
                    Rect::new(
                        r.origin.x,
                        r.origin.y + (r.size.height - style.line_height) / 2.0,
                        r.size.width,
                        style.line_height,
                    ),
                    colors.on_surface,
                    style,
                    TextAlign::Center,
                );
            }
        }
    }
}

/// 操作项底色。参 `CONTROL_SPEC` §32 —— 规格未给色值，故一律取主题令牌：
/// 默认项 = 次级表面；强调项 = 强调色；危险项 = 语义危险底（`StatusColors::danger_fill`）。
fn item_bg(action: SwipeItemAction, theme: &MetroTheme) -> kanesumi_core::Color {
    match action {
        SwipeItemAction::Default => theme.colors.surface_variant,
        SwipeItemAction::Accent => theme.colors.primary,
        SwipeItemAction::Danger => theme.status.danger_fill,
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3）─────────────────────────────────
//
// 拖拽类：旧 API（`press` / `drag_to` / `release` / `handle_click` / `render`）原样保留给未迁移的
// App；`Widget` 实现把指针按下 / 移动 / 释放翻译成「显露 → 吸合」或「轻点触发操作项」。
// 本控件无键盘语义，`focusable()` 保持默认 false（不占 Tab 位）—— 副作用是框架不会把本节点当
// 「交互目标」、也就不会合成 `Click`，故触发判定放在 `PointerUp` 里自己做（同 §3 拖拽类约定）。
// 命中与焦点交给框架；`hit_test` 保持默认整矩形（旧 `hit` 对 rect 内一律返回 Close，等价）。

/// 轻点 / 拖动分界（水平位移像素）：≤ 此值视为轻点（触发操作项或收起），否则按拖动释放。
const SWIPE_TAP_SLOP: f32 = 4.0;

/// 元素树动作：滑动操作项被触发。携带该项的语义类型（`SwipeItemAction`）。
/// 元素树动作：某个操作项被触发。携带**哪一侧、第几项**（同语义的多个项靠它区分）与该项语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SwipeInvoked {
    /// `true` = 左侧项（向右滑露出），`false` = 右侧项。
    pub left: bool,
    /// 在对应列表（`left_items` / `right_items`）中的索引。
    pub index: usize,
    pub action: SwipeItemAction,
}

impl kanesumi_element::Widget for MetroSwipeControl {
    /// 固有尺寸：规格 §32 未给定量；宽随宿主铺满（无界轴退回 200），高取条目高 48。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let w = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            200.0
        };
        kanesumi_core::Size::new(w, 48.0)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // §6 修正：右侧项区从 `rect.right()` 往左展开，rect 窄于项区宽时左缘会画到 rect 之外
        // （旧行为：越界绘制 → 新行为：裁到 rect）。成对夹裁，不改旧 `render`。
        scene.push_clip(ctx.rect());
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        scene.pop_clip();
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        match event {
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                self.press(*pos);
                ctx.set_handled();
            }
            // 框架在按下后自动捕获指针，移出控件也会收到 Move。
            Event::PointerMove { pos } => {
                if self.drag_start.is_some() {
                    self.drag_to(*pos);
                }
                self.hover(ctx.rect(), *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                // 水平位移 ≤ 阈值 = 轻点：命中露出操作项 → 触发；命中内容 → 收起。
                let is_tap = self
                    .drag_start
                    .map(|s| (pos.x - s).abs() <= SWIPE_TAP_SLOP)
                    .unwrap_or(false);
                if is_tap {
                    let rect = ctx.rect();
                    // offset 的符号决定命中在左 / 右哪一侧（`handle_click` 内部会 close 归零）。
                    let side_left = self.offset > 0.0;
                    let invoked = match self.handle_click(rect, *pos) {
                        SwipeAction::Invoke(i) => {
                            let item = if side_left {
                                self.left_items.get(i)
                            } else {
                                self.right_items.get(i)
                            };
                            item.map(|it| SwipeInvoked {
                                left: side_left,
                                index: i,
                                action: it.action,
                            })
                        }
                        _ => None,
                    };
                    if let Some(a) = invoked {
                        ctx.emit(a);
                    }
                    ctx.invalidate_paint();
                    ctx.set_handled();
                } else {
                    // 拖动释放：吸合 / 回弹。Execute 模式越过阈值即触发该侧首项（旧 `release`
                    // 只做复位、不给身份，故在此先按同一阈值判定再转发）。
                    let side_left = self.offset > 0.0;
                    let threshold = if side_left {
                        self.left_width()
                    } else {
                        self.right_width()
                    } * SWIPE_SNAP_THRESHOLD;
                    let triggered = if self.mode == SwipeMode::Execute
                        && threshold > 0.0
                        && self.offset.abs() >= threshold
                    {
                        if side_left {
                            self.left_items.first()
                        } else {
                            self.right_items.first()
                        }
                        .map(|it| SwipeInvoked {
                            left: side_left,
                            index: 0,
                            action: it.action,
                        })
                    } else {
                        None
                    };
                    self.release();
                    if let Some(a) = triggered {
                        ctx.emit(a);
                    }
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            _ => {}
        }
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: String::from("滑动操作"),
            value: Some(if self.revealed {
                String::from("已展开")
            } else {
                String::from("已收起")
            }),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps, PointerButton, WidgetId};

    fn sample() -> MetroSwipeControl {
        MetroSwipeControl {
            left_items: vec![
                SwipeItem::with_action("收藏", SwipeItemAction::Accent),
                SwipeItem::with_action("删除", SwipeItemAction::Danger),
            ],
            right_items: vec![SwipeItem::new("更多")],
            ..MetroSwipeControl::default()
        }
    }

    fn harness(widget: MetroSwipeControl, props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            widget,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    /// 拖出左项并吸合（越过阈值）。
    fn reveal_left(h: &mut TestHarness, id: WidgetId) {
        let r = h.rect(id);
        let start = Point::new(r.origin.x + 40.0, r.center().y);
        h.press_at(start, PointerButton::Left);
        h.move_to(Point::new(start.x + 200.0, start.y));
        h.release_at(Point::new(start.x + 200.0, start.y), PointerButton::Left);
        h.frame();
    }

    #[test]
    fn drag_reveals_left_items_and_snaps() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        reveal_left(&mut h, id);
        let s = h.tree.get::<MetroSwipeControl>(id).unwrap();
        assert_eq!(s.offset, 128.0, "越过阈值吸合到左项区全宽");
        assert!(s.revealed);
    }

    #[test]
    fn drag_below_threshold_bounces_back() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        let r = h.rect(id);
        let start = Point::new(r.origin.x + 40.0, r.center().y);
        h.press_at(start, PointerButton::Left);
        h.move_to(Point::new(start.x + 10.0, start.y)); // dx=10 < 左项区一半
        h.release_at(Point::new(start.x + 10.0, start.y), PointerButton::Left);
        h.frame();
        let s = h.tree.get::<MetroSwipeControl>(id).unwrap();
        assert_eq!(s.offset, 0.0, "未过阈值回弹");
        assert!(h.take::<SwipeInvoked>().is_empty());
    }

    #[test]
    fn tap_revealed_item_emits_invoked_and_closes() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        reveal_left(&mut h, id);
        let r = h.rect(id);
        // 左项固定于左缘：item0 = [origin.x, origin.x+64)、item1 = 下一段。
        h.click_at(Point::new(r.origin.x + 32.0, r.center().y));
        assert_eq!(
            h.take::<SwipeInvoked>(),
            vec![(id, SwipeInvoked { left: true, index: 0, action: SwipeItemAction::Accent })]
        );
        assert_eq!(h.tree.get::<MetroSwipeControl>(id).unwrap().offset, 0.0);
    }

    #[test]
    fn tap_second_revealed_item_reports_its_action() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        reveal_left(&mut h, id);
        let r = h.rect(id);
        h.click_at(Point::new(r.origin.x + 96.0, r.center().y));
        assert_eq!(
            h.take::<SwipeInvoked>(),
            vec![(id, SwipeInvoked { left: true, index: 1, action: SwipeItemAction::Danger })]
        );
    }

    #[test]
    fn tap_content_closes_without_invoking() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        reveal_left(&mut h, id);
        let r = h.rect(id);
        h.click_at(Point::new(r.right() - 10.0, r.center().y));
        assert!(h.take::<SwipeInvoked>().is_empty());
        assert_eq!(h.tree.get::<MetroSwipeControl>(id).unwrap().offset, 0.0);
    }

    #[test]
    fn execute_mode_drag_triggers_first_item() {
        let widget = MetroSwipeControl {
            mode: SwipeMode::Execute,
            ..sample()
        };
        let (mut h, id) = harness(widget, LayoutProps::default());
        let r = h.rect(id);
        let start = Point::new(r.origin.x + 40.0, r.center().y);
        h.press_at(start, PointerButton::Left);
        h.move_to(Point::new(start.x + 200.0, start.y));
        h.release_at(Point::new(start.x + 200.0, start.y), PointerButton::Left);
        h.frame();
        assert_eq!(
            h.take::<SwipeInvoked>(),
            vec![(id, SwipeInvoked { left: true, index: 0, action: SwipeItemAction::Accent })]
        );
        assert_eq!(h.tree.get::<MetroSwipeControl>(id).unwrap().offset, 0.0);
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(sample(), LayoutProps::default());
        assert!(h.rect(id).size.width > 0.0 && h.rect(id).size.height > 0.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        // 右项区 64 宽 > 控件宽 40：右项会画到 rect 左缘之外，§6 在 paint 里裁掉。
        let (h, id) = harness(sample(), LayoutProps {
            width: Some(40.0),
            ..LayoutProps::default()
        });
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn no_tab_stop_without_keyboard_semantics() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        h.tab();
        assert_ne!(h.tree.focused(), Some(id), "无键盘语义 → 不占 Tab 位");
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(sample(), LayoutProps::default());
        h.tree.set_enabled(id, false);
        h.frame();
        reveal_left(&mut h, id);
        let s = h.tree.get::<MetroSwipeControl>(id).unwrap();
        assert_eq!(s.offset, 0.0, "禁用不响应拖动");
        assert!(!s.revealed);
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

    fn swipe() -> MetroSwipeControl {
        MetroSwipeControl {
            left_items: vec![
                SwipeItem::with_action("收藏", SwipeItemAction::Accent),
                SwipeItem::with_action("删除", SwipeItemAction::Danger),
            ],
            right_items: vec![SwipeItem::new("更多")],
            ..MetroSwipeControl::default()
        }
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 300.0, 48.0)
    }

    #[test]
    fn drag_reveals_left() {
        let mut s = swipe();
        s.press(Point::new(50.0, 24.0));
        s.drag_to(Point::new(150.0, 24.0)); // 右拖 → 露出左项
        assert_eq!(s.offset, 100.0);
        s.release();
        assert_eq!(s.offset, 128.0, "越过阈值吸合到项区全宽");
        assert!(s.revealed);
    }

    #[test]
    fn drag_below_threshold_bounces() {
        let mut s = swipe();
        s.press(Point::new(100.0, 24.0));
        s.drag_to(Point::new(110.0, 24.0)); // dx=10 < 项区一半
        s.release();
        assert_eq!(s.offset, 0.0, "未过阈值回弹");
    }

    #[test]
    fn click_invoke_returns_index() {
        let mut s = swipe();
        s.offset = 64.0;
        s.revealed = true;
        let r = Rect::new(0.0, 0.0, 300.0, 48.0);
        // 左项固定于左缘：item0 = [0,64)，item1 = [64,128)
        assert_eq!(
            s.handle_click(r, Point::new(32.0, 24.0)),
            SwipeAction::Invoke(0)
        );
        assert_eq!(s.offset, 0.0, "点操作项后收起");
    }

    #[test]
    fn click_content_closes() {
        let mut s = swipe();
        s.offset = 128.0;
        let r = area();
        assert_eq!(
            s.handle_click(r, Point::new(200.0, 24.0)),
            SwipeAction::Close
        );
        assert_eq!(s.offset, 0.0);
    }

    #[test]
    fn execute_mode_resets_after_drag() {
        let mut s = swipe();
        s.mode = SwipeMode::Execute;
        s.press(Point::new(50.0, 24.0));
        s.drag_to(Point::new(150.0, 24.0));
        s.release();
        assert_eq!(s.offset, 0.0, "Execute 拖出即触发并复位");
    }

    #[test]
    fn clamp_limits_offset() {
        let mut s = swipe();
        s.press(Point::new(100.0, 24.0));
        s.drag_to(Point::new(-500.0, 24.0));
        assert_eq!(s.offset, -64.0, "夹紧到右项区宽");
    }

    #[test]
    fn render_emits_revealed_items() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut s = swipe();
        s.offset = 128.0;
        let mut scene = Scene::default();
        s.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 2, "两个左操作项");
    }
}
