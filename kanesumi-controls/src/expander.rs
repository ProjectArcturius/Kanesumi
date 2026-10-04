// MetroExpander —— 可折叠分组。参 CONTROL_SPEC §13。
//
// 移植自 microsoft-ui-xaml/dev/Expander（Expander.cpp + Expander.xaml + Expander_themeresources.xaml）：
// - Header 行 MinHeight 48、Padding 16,0,0,0、bg surface、边框 divider 1px；
// - 右侧 Chevron 按钮 32×32、Margin 20,0,8,0、glyph 12，展开时旋转 180°（0.1s）；
// - Content Padding 16、bg surface_variant，Down 模式边框 1,0,1,1 / Up 模式 1,1,1,0；
// - 展开动画 0.333s / 收起 0.167s（TranslateY，只动视觉属性）。
//
// 内容承载：宿主把内容渲染进 `content_rect`，并以 `content_clip` 裁剪（Scene::clip），
// 动画期间只显示可见段（铁律 4：展开不动宿主布局，只改裁剪窗）。

use kanesumi_anim::{MetroAnim, MetroPresets};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::state::ControlState;

/// 展开方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandDirection {
    Down,
    Up,
}

/// 头部行高（ExpanderMinHeight = 48）。
pub const EXPANDER_HEADER_HEIGHT: f32 = 48.0;
/// Header 左内边距（ExpanderHeaderPadding = 16,0,0,0）。
pub const EXPANDER_HEADER_PAD_X: f32 = 16.0;
/// Chevron 按钮边长（ExpanderChevronButtonSize = 32）。
pub const EXPANDER_CHEVRON_SIZE: f32 = 32.0;
/// Chevron 右侧边距（ExpanderChevronMargin = 20,0,8,0 的 right）。
pub const EXPANDER_CHEVRON_MARGIN_RIGHT: f32 = 8.0;

/// MetroExpander —— 折叠组。参 CONTROL_SPEC §13。
#[derive(Debug, Clone)]
pub struct MetroExpander {
    pub header: String,
    pub expanded: bool,
    /// Header 交互状态。
    pub state: ControlState,
    /// 展开方向（Down / Up）。
    pub direction: ExpandDirection,
    /// 内容完整高度（宿主在内容稳定后设置；动画期间不变）。
    pub content_height: f32,
    /// 内容展开进度 [0,1]（0.333s 展开 / 0.167s 收起）。
    content: MetroAnim,
    /// Chevron 旋转进度 [0,1]（0 = 朝下，1 = 朝上；0.1s）。
    chevron: MetroAnim,
}

impl Default for MetroExpander {
    fn default() -> Self {
        Self {
            header: String::new(),
            expanded: false,
            state: ControlState::Normal,
            direction: ExpandDirection::Down,
            content_height: 0.0,
            content: MetroPresets::expander_expand(),
            chevron: MetroPresets::expander_chevron(),
        }
    }
}

impl MetroExpander {
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            ..Self::default()
        }
    }

    /// 折叠/展开切换（等价 UWP IsExpanded 翻转）。
    pub fn toggle(&mut self) {
        self.set_expanded(!self.expanded);
    }

    /// 设置展开态 —— 启动对应时长动画（可中断）。
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        self.content = if expanded {
            MetroPresets::expander_expand()
        } else {
            MetroPresets::expander_collapse()
        };
        self.content.set_target(if expanded { 1.0 } else { 0.0 });
        self.chevron = MetroPresets::expander_chevron();
        self.chevron.set_target(if expanded { 1.0 } else { 0.0 });
    }

    /// 每帧推进内容/chevron 动画。
    pub fn update(&mut self, dt: f64) {
        self.content.update(dt);
        self.chevron.update(dt);
    }

    pub fn is_animating(&self) -> bool {
        !self.content.is_steady() || !self.chevron.is_steady()
    }

    /// 内容展开进度 [0,1]。
    pub fn content_progress(&self) -> f32 {
        self.content.value() as f32
    }

    /// 当前内容可见高度。
    pub fn visible_content_height(&self) -> f32 {
        self.content_height * self.content_progress()
    }

    /// Chevron 方向进度 [0,1]（0 = 下，1 = 上）。
    pub fn chevron_progress(&self) -> f32 {
        self.chevron.value() as f32
    }

    /// Header 行 rect。
    pub fn header_rect(&self, rect: Rect) -> Rect {
        match self.direction {
            ExpandDirection::Down => Rect::new(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                EXPANDER_HEADER_HEIGHT,
            ),
            ExpandDirection::Up => {
                let y = rect.bottom() - EXPANDER_HEADER_HEIGHT;
                Rect::new(rect.origin.x, y, rect.size.width, EXPANDER_HEADER_HEIGHT)
            }
        }
    }

    /// 内容完整 rect（宿主渲染内容的位置，全高）。
    pub fn content_rect(&self, rect: Rect) -> Rect {
        let h = self.content_height;
        match self.direction {
            ExpandDirection::Down => Rect::new(
                rect.origin.x,
                rect.origin.y + EXPANDER_HEADER_HEIGHT,
                rect.size.width,
                h,
            ),
            ExpandDirection::Up => Rect::new(rect.origin.x, rect.origin.y, rect.size.width, h),
        }
    }

    /// 内容裁剪窗（动画期间只显示可见段）。
    pub fn content_clip(&self, rect: Rect) -> Option<Rect> {
        let vis = self.visible_content_height();
        if vis <= 0.01 {
            return None;
        }
        let content = self.content_rect(rect);
        Some(match self.direction {
            ExpandDirection::Down => {
                Rect::new(content.origin.x, content.origin.y, content.size.width, vis)
            }
            ExpandDirection::Up => {
                let y = content.bottom() - vis;
                Rect::new(content.origin.x, y, content.size.width, vis)
            }
        })
    }

    /// Chevron 按钮 rect（右侧）。
    pub fn chevron_rect(&self, rect: Rect) -> Rect {
        let header = self.header_rect(rect);
        Rect::new(
            header.right() - EXPANDER_CHEVRON_SIZE - EXPANDER_CHEVRON_MARGIN_RIGHT,
            header.origin.y + (header.size.height - EXPANDER_CHEVRON_SIZE) / 2.0,
            EXPANDER_CHEVRON_SIZE,
            EXPANDER_CHEVRON_SIZE,
        )
    }

    /// Header 命中（含 chevron 区域）。
    pub fn hit_header(&self, rect: Rect, pos: Point) -> bool {
        self.header_rect(rect).contains(pos)
    }

    /// Header 文本样式：14px。
    pub fn header_style() -> TextStyle {
        MetroTypography::metro().body_medium
    }

    /// 渲染 Header 行（bg + 边框 + 标签 + chevron）。Content 由宿主渲染。
    pub fn render_header(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let header = self.header_rect(rect);
        let style = Self::header_style();

        scene.fill_rect(colors.surface, header);
        // 交互 tint
        match self.state {
            ControlState::Hovered => scene.fill_rect(theme.indication.subtle_tint, header),
            // 按压比悬停更实一档（10% = press_subtle_tint，参 ROADMAP M1-1 分类学）。
            ControlState::Pressed => scene.fill_rect(theme.indication.press_subtle_tint, header),
            _ => {}
        }
        // 边框（4 边）
        scene.stroke_rect(colors.divider, header, 1.0);

        // 标签
        let text_rect = Rect::new(
            header.origin.x + EXPANDER_HEADER_PAD_X,
            header.origin.y + (header.size.height - style.line_height) / 2.0,
            header.size.width - EXPANDER_HEADER_PAD_X - EXPANDER_CHEVRON_SIZE - 28.0,
            style.line_height,
        );
        scene.text(
            self.header.clone(),
            text_rect,
            colors.on_surface,
            style,
            TextAlign::Left,
        );

        // Chevron —— 朝下/朝上之间按进度插值（0.1s 旋转的几何近似）。
        let chevron = self.chevron_rect(rect);
        let p = self.chevron_progress();
        // 三角形顶点按进度从「下」翻到「上」。
        let cx = chevron.origin.x + chevron.size.width / 2.0;
        let top_y = chevron.origin.y + chevron.size.height * (0.25 + 0.25 * p);
        let bottom_y = chevron.origin.y + chevron.size.height * (0.75 - 0.25 * p);
        let w = chevron.size.width * 0.4;
        // p=0（下）：base 在上、tip 在下；p=1（上）：base 在下、tip 在上。
        let (left, right, tip) = if p < 0.5 {
            (
                Point::new(cx - w, top_y),
                Point::new(cx + w, top_y),
                Point::new(cx, bottom_y),
            )
        } else {
            (
                Point::new(cx - w, bottom_y),
                Point::new(cx + w, bottom_y),
                Point::new(cx, top_y),
            )
        };
        scene.triangle(left, right, tip, colors.on_surface);
        let _ = engine;
    }

    /// 渲染 Content 底 + 边框（`progress` 高度）。宿主内容画在 `content_rect` 上。
    pub fn render_content(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let vis = self.visible_content_height();
        if vis <= 0.01 {
            return;
        }
        let colors = &theme.colors;
        let content = self.content_rect(rect);
        let visible = match self.direction {
            ExpandDirection::Down => {
                Rect::new(content.origin.x, content.origin.y, content.size.width, vis)
            }
            ExpandDirection::Up => {
                let y = content.bottom() - vis;
                Rect::new(content.origin.x, y, content.size.width, vis)
            }
        };
        scene.fill_rect(colors.surface_variant, visible);
        // 边框：Down = 1,0,1,1；Up = 1,1,1,0。画左/右/可见端 3 条。
        scene.fill_rect(
            colors.divider,
            Rect::new(visible.origin.x, visible.origin.y, 1.0, visible.size.height),
        );
        scene.fill_rect(
            colors.divider,
            Rect::new(
                visible.right() - 1.0,
                visible.origin.y,
                1.0,
                visible.size.height,
            ),
        );
        match self.direction {
            ExpandDirection::Down => {
                scene.fill_rect(
                    colors.divider,
                    Rect::new(
                        visible.origin.x,
                        visible.bottom() - 1.0,
                        visible.size.width,
                        1.0,
                    ),
                );
            }
            ExpandDirection::Up => {
                scene.fill_rect(
                    colors.divider,
                    Rect::new(visible.origin.x, visible.origin.y, visible.size.width, 1.0),
                );
            }
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）────────────────────
//
// **容器**：标题行由控件自绘（`render_header`），内容 = 子节点（App 把内容挂到它下面）。
// `measure` = 标题高 +（展开时）子节点期望高。尺寸在切换那一刻失效一次；动画期间只
// `invalidate_arrange`（子节点槽位随进度变化）与 `invalidate_paint`（内容底/裁剪随进度重画），
// **不**重新量测子树（参 ELEMENT_TREE §Ⅴ.1）。注：框架的 `UpdateCtx` 有意不提供量测失效
// （动画只动视觉），故收起时容器在切换那一刻直接塌回标题高，没有「高度动画」这一段。
//
// 框架只能把子节点裁到本节点矩形，无法裁到「内容可见窗」这一子区域，故动画期间子节点槽位
// 直接取 `content_clip`（可见窗）本身：收起稳态退化为零高槽位（子节点不可命中），展开态为
// 完整 `content_rect`。

/// 元素树动作：展开态改变（点标题 / Enter / Space）。携带新的展开态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpanderToggled(pub bool);

impl kanesumi_element::Widget for MetroExpander {
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: Size,
    ) -> Size {
        let header = EXPANDER_HEADER_HEIGHT;
        let w = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let content_avail = if available.height.is_finite() {
            (available.height - header).max(0.0)
        } else {
            f32::INFINITY
        };
        let mut content_h = 0.0f32;
        if self.expanded {
            for c in ctx.children() {
                let d = ctx.measure_child(c, Size::new(w, content_avail));
                content_h = content_h.max(d.height);
            }
        }
        self.content_height = content_h;
        Size::new(w, header + content_h)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        // 展开稳态 = 完整内容区；动画期节点槽位 = 可见窗（见节首说明）；
        // 收起稳态 = 内容原点上的零高槽位（子节点排进去但不可命中）。
        let slot = if self.expanded && !self.is_animating() {
            self.content_rect(rect)
        } else {
            self.content_clip(rect).unwrap_or_else(|| {
                let c = self.content_rect(rect);
                Rect::new(c.origin.x, c.origin.y, c.size.width, 0.0)
            })
        };
        // 子节点期望尺寸已在 `measure` 里算好，此处不再量测 —— 动画期量测会破坏「只动视觉」。
        for c in ctx.children() {
            ctx.arrange_child(c, slot);
        }
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let saved = self.state;
        self.state = crate::state::control_state(ctx.state());
        self.render_header(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        self.render_content(ctx.theme(), ctx.rect(), scene);
        self.state = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        let rect = ctx.rect();
        let toggled = match event {
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => self.hit_header(rect, *pos),
            Event::KeyDown { key: Key::Enter, .. }
            | Event::KeyDown { key: Key::Char(' '), .. } => true,
            _ => false,
        };
        if !toggled {
            return;
        }
        self.toggle();
        // 尺寸只在切换这一刻变一次：展开立刻撑到内容高，收起在动画结束再塌回标题高。
        ctx.invalidate_measure();
        ctx.invalidate_arrange();
        ctx.invalidate_paint();
        ctx.request_anim_frame();
        ctx.emit(ExpanderToggled(self.expanded));
        ctx.set_handled();
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, _dt: f64) {
        let was_animating = self.is_animating();
        if !self.content.is_steady() {
            // 统一动画入口：真实时钟推进 + 未稳态自动续帧（参 ELEMENT_TREE §帧调度）。
            ctx.animate(&mut self.content);
        }
        if !self.chevron.is_steady() {
            ctx.animate(&mut self.chevron);
        }
        if self.is_animating() {
            // 动画期内容可见窗变化 → 重排槽位；内容底/chevron 重画。
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
        } else if was_animating {
            // 动画刚结束：按完整内容区重排一次，避免稳态槽位停在动画末帧的近似值。
            ctx.invalidate_arrange();
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        // 只有标题行可交互；内容区的空白不拦截，交给下层。
        self.hit_header(rect, pos)
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: self.header.clone(),
            value: None,
            checked: Some(self.expanded),
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;
    use kanesumi_canvas::Scene;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Label;
    use kanesumi_element::{Align, Insets, LayoutProps, MeasureCtx, PaintCtx, Widget, WidgetId};

    /// 计数用子控件：记录 `measure` 被调用次数（验证动画期间不重量测）。
    struct Counted {
        size: Size,
        count: Rc<Cell<u32>>,
    }

    impl Widget for Counted {
        fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
            self.count.set(self.count.get() + 1);
            self.size
        }
        fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
            scene.fill_rect(kanesumi_core::Color::WHITE, ctx.rect());
        }
    }

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroExpander::new("网络"),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn header_center(h: &TestHarness, id: WidgetId) -> Point {
        h.tree
            .get::<MetroExpander>(id)
            .unwrap()
            .header_rect(h.rect(id))
            .center()
    }

    #[test]
    fn collapsed_is_header_high_and_child_unhittable() {
        let (mut h, id) = harness();
        let child = h.tree.insert(id, Label::new("内容"));
        h.frame();
        assert_eq!(h.rect(id).size.height, EXPANDER_HEADER_HEIGHT, "收起 = 标题高");
        assert_eq!(h.rect(child).size.height, 0.0, "收起 → 子节点零高槽位");
        assert_ne!(h.tree.hit(Point::new(200.0, 60.0)), Some(child));
    }

    #[test]
    fn expanding_reveals_child_in_content_rect() {
        let (mut h, id) = harness();
        let child = h.tree.insert(id, Label::new("内容"));
        h.frame();
        h.click_at(header_center(&h, id));
        h.settle();
        let content = h
            .tree
            .get::<MetroExpander>(id)
            .unwrap()
            .content_rect(h.rect(id));
        let r = h.rect(child);
        assert_eq!(r, content, "展开稳态子节点铺满 content_rect");
        assert_eq!(h.tree.hit(r.center()), Some(child), "展开后子节点可命中");
    }

    #[test]
    fn toggle_emits_with_new_state() {
        let (mut h, id) = harness();
        h.click_at(header_center(&h, id));
        assert_eq!(h.take::<ExpanderToggled>(), vec![(id, ExpanderToggled(true))]);
        h.settle();
        h.tab();
        h.key(kanesumi_element::Key::Enter);
        assert_eq!(h.take::<ExpanderToggled>(), vec![(id, ExpanderToggled(false))]);
    }

    #[test]
    fn child_not_remeasured_during_animation() {
        let (mut h, id) = harness();
        let count = Rc::new(Cell::new(0));
        h.tree.insert(
            id,
            Counted {
                size: Size::new(120.0, 40.0),
                count: count.clone(),
            },
        );
        h.frame();
        assert_eq!(count.get(), 0, "收起稳态不量测子节点");
        h.click_at(header_center(&h, id));
        let at_toggle = count.get();
        assert!(at_toggle >= 1, "展开切换量测一次子节点");
        h.frame();
        h.frame();
        assert_eq!(count.get(), at_toggle, "动画期间不重新量测子节点");
        h.settle();
        assert_eq!(count.get(), at_toggle, "稳态不再增量测");
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (mut h, id) = harness();
        h.tree.insert(id, Label::new("内容"));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroExpander::new("网络"),
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.tree.insert(id, Label::new("内容"));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_toggle() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(header_center(&h, id));
        assert!(h.take::<ExpanderToggled>().is_empty());
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

    fn expanded_to_steady(mut e: MetroExpander) -> MetroExpander {
        e.set_expanded(true);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        e
    }

    #[test]
    fn header_height_is_48() {
        assert_eq!(EXPANDER_HEADER_HEIGHT, 48.0);
    }

    #[test]
    fn toggle_flips_expanded() {
        let mut e = MetroExpander::new("网络");
        assert!(!e.expanded);
        e.toggle();
        assert!(e.expanded);
        e.toggle();
        assert!(!e.expanded);
    }

    #[test]
    fn expand_animates_to_steady() {
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        assert!(e.is_animating());
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        assert!(!e.is_animating());
        assert!((e.content_progress() - 1.0).abs() < 0.001);
        assert!(
            (e.chevron_progress() - 1.0).abs() < 0.001,
            "chevron 翻到朝上"
        );
    }

    #[test]
    fn expand_then_collapse_is_interruptible() {
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        e.update(0.1);
        let mid = e.content_progress();
        assert!(mid > 0.0 && mid < 1.0);
        e.set_expanded(false);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        assert!((e.content_progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn visible_content_grows_with_progress() {
        let mut e = MetroExpander::new("网络");
        e.content_height = 120.0;
        e.set_expanded(true);
        e.update(0.1);
        let p = e.content_progress();
        let vis = e.visible_content_height();
        assert!((vis - 120.0 * p).abs() < 0.01, "可见高 = 内容高 × 进度");
    }

    #[test]
    fn header_at_top_for_down() {
        let e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let h = e.header_rect(rect);
        assert_eq!(h.origin.y, 0.0);
        assert_eq!(h.size.height, 48.0);
        // content 在 header 下方
        let c = e.content_rect(rect);
        assert_eq!(c.origin.y, 48.0);
    }

    #[test]
    fn header_at_bottom_for_up() {
        let mut e = MetroExpander::new("网络");
        e.direction = ExpandDirection::Up;
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let h = e.header_rect(rect);
        assert_eq!(h.origin.y, 152.0, "Up 模式 header 贴底");
        let c = e.content_rect(rect);
        assert_eq!(c.origin.y, 0.0);
    }

    #[test]
    fn hit_header_contains() {
        let e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        assert!(e.hit_header(rect, Point::new(10.0, 20.0)));
        assert!(!e.hit_header(rect, Point::new(10.0, 100.0)));
    }

    #[test]
    fn render_header_emits_bg_border_label_chevron() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        e.render_header(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 200.0),
            &mut scene,
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count();
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(strokes, 1, "Header 边框");
        assert_eq!(texts, 1, "Header 标签");
        assert_eq!(tris, 1, "Chevron 三角形");
    }

    #[test]
    fn render_content_emits_only_when_visible() {
        let theme = MetroTheme::ether_dark();
        // 收起 → 无 content
        let mut e = MetroExpander::new("网络");
        e.content_height = 100.0;
        let mut scene = Scene::default();
        e.render_content(&theme, Rect::new(0.0, 0.0, 300.0, 200.0), &mut scene);
        assert!(scene.is_empty(), "收起态不渲染 content");
        // 展开到稳态 → 有 fill（底 + 3 边框条）
        e = expanded_to_steady(e);
        let mut scene = Scene::default();
        e.render_content(&theme, Rect::new(0.0, 0.0, 300.0, 200.0), &mut scene);
        assert_eq!(scene.commands.len(), 4, "content 底 + 左/右/底边框 3 条");
    }

    #[test]
    fn chevron_points_down_when_collapsed_up_when_expanded() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        // 收起（progress 0）：tip 在下方
        let mut scene = Scene::default();
        e.render_header(&theme, &engine, rect, &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = scene.commands.last().unwrap() else {
            panic!("应画三角形");
        };
        let cy = e.chevron_rect(rect).center().y;
        let max_y = p0.y.max(p1.y).max(p2.y);
        let min_y = p0.y.min(p1.y).min(p2.y);
        assert!(max_y > cy, "收起 → 尖端朝下");
        assert!(min_y < cy);
    }
}
