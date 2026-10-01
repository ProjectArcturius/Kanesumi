// MetroParallaxView —— 视差滚动。参 CONTROL_SPEC §30。
//
// 移植自 microsoft-ui-xaml/dev/ParallaxView（ParallaxView.cpp + ParallaxView.idl）：
// - shift = scroll_offset × ratio，clamp 到 [−MaxShift, +MaxShift]；
// - MaxShift = MaxShiftRatio × 视口主轴。
// 纯布局/位移辅助（宿主渲染内容），无自绘。

use kanesumi_core::{Rect, Size};

/// 默认视差系数（0.5）。
pub const DEFAULT_PARALLAX_RATIO: f32 = 0.5;

/// MetroParallaxView —— 视差容器。参 CONTROL_SPEC §30。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroParallaxView {
    /// 视差系数（0..1）。
    pub ratio: f32,
    /// 视口主轴尺寸。
    pub viewport: f32,
    /// 内容主轴尺寸。
    pub content: f32,
    /// 上限比率（× viewport）。
    pub max_shift_ratio: f32,
    /// 是否夹紧位移。
    pub clamped: bool,
    /// 当前滚动偏移（元素树接入新增；App 经 `set_scroll_offset` 写入）。
    scroll: f32,
}

impl Default for MetroParallaxView {
    fn default() -> Self {
        Self {
            ratio: DEFAULT_PARALLAX_RATIO,
            viewport: 0.0,
            content: 0.0,
            max_shift_ratio: 1.0,
            clamped: true,
            scroll: 0.0,
        }
    }
}

impl MetroParallaxView {
    pub fn new() -> Self {
        Self::default()
    }

    /// 最大位移（MaxShift = MaxShiftRatio × viewport）。
    pub fn max_shift(&self) -> f32 {
        self.max_shift_ratio * self.viewport
    }

    /// 给定滚动偏移 → 内容位移。
    pub fn shift(&self, scroll_offset: f32) -> f32 {
        let s = scroll_offset * self.ratio;
        if self.clamped {
            let m = self.max_shift();
            s.clamp(-m, m)
        } else {
            s
        }
    }

    /// 设置当前滚动偏移（元素树接入）。App 经 `tree.edit` 调用后须 `invalidate_arrange`，
    /// 子节点槽位才会按新的 `shift` 平移。
    pub fn set_scroll_offset(&mut self, v: f32) {
        self.scroll = v;
    }

    /// 当前滚动偏移。
    pub fn scroll_offset(&self) -> f32 {
        self.scroll
    }

    /// 内容视口窗口 rect（水平视差）。
    pub fn content_rect(&self, rect: Rect, scroll_offset: f32) -> Rect {
        let s = self.shift(scroll_offset);
        Rect::new(
            rect.origin.x + s,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        )
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 navigation_view.rs）────────
//
// **单子节点容器**：子节点排进 `content_rect(rect, scroll_offset)`（按 `shift` 水平平移）。
// `clips_children = true` 把越出视口的部分裁掉；`scrolls_children = true` 告诉框架
// 「子节点按偏移排在自身矩形之外」豁免包含性断言（同 ScrollViewer 语义）。`hit_test` 恒 false
// （空白穿透，命中交给子节点）。

impl kanesumi_element::Widget for MetroParallaxView {
    /// 视口铺满可用尺寸；无界轴取子节点期望尺寸。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: Size,
    ) -> Size {
        let mut content = Size::ZERO;
        for c in ctx.children() {
            let d = ctx.measure_child(c, available);
            content = Size::new(content.width.max(d.width), content.height.max(d.height));
        }
        Size::new(
            if available.width.is_finite() {
                available.width.max(0.0)
            } else {
                content.width
            },
            if available.height.is_finite() {
                available.height.max(0.0)
            } else {
                content.height
            },
        )
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        let slot = self.content_rect(rect, self.scroll);
        for c in ctx.children() {
            // 先量测再排版：非拉伸子节点要靠期望尺寸定位。
            ctx.measure_child(c, slot.size);
            ctx.arrange_child(c, slot);
        }
    }

    fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut kanesumi_canvas::Scene) {}

    fn hit_test(&self, _rect: Rect, _pos: kanesumi_core::Point) -> bool {
        false
    }

    fn clips_children(&self) -> bool {
        true
    }

    fn scrolls_children(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Label;
    use kanesumi_element::{Align, Insets, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroParallaxView {
                clamped: false,
                ..MetroParallaxView::default()
            },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let child = h.tree.insert(id, Label::new("内容"));
        h.frame();
        (h, id, child)
    }

    #[test]
    fn offset_translates_child_by_shift() {
        let (mut h, id, child) = harness();
        let before = h.rect(child).origin.x;
        h.tree.edit::<MetroParallaxView, _>(id, |p, ctx| {
            p.set_scroll_offset(100.0);
            ctx.invalidate_arrange();
        });
        h.frame();
        assert_eq!(
            h.rect(child).origin.x - before,
            50.0,
            "ratio 0.5 × offset 100 = 平移 50"
        );
    }

    #[test]
    fn blank_area_passes_through() {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroParallaxView::default(),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(
            id,
            Label::new("小"),
            LayoutProps {
                width: Some(40.0),
                height: Some(20.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        // 容器自身不命中，空白处穿透。
        assert_ne!(h.tree.hit(kanesumi_core::Point::new(200.0, 200.0)), Some(id));
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id, _) = harness();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_scales_with_ratio() {
        let p = MetroParallaxView {
            ratio: 0.5,
            viewport: 400.0,
            content: 800.0,
            ..MetroParallaxView::default()
        };
        assert_eq!(p.shift(100.0), 50.0);
        assert_eq!(p.shift(200.0), 100.0);
    }

    #[test]
    fn shift_clamped() {
        let p = MetroParallaxView {
            ratio: 1.0,
            viewport: 400.0,
            content: 1000.0,
            max_shift_ratio: 1.0,
            clamped: true,
            ..MetroParallaxView::default()
        };
        assert_eq!(p.max_shift(), 400.0);
        assert_eq!(p.shift(999.0), 400.0, "夹紧到 MaxShift");
        assert_eq!(p.shift(-999.0), -400.0);
    }

    #[test]
    fn clamped_false_passes_through() {
        let p = MetroParallaxView {
            ratio: 1.0,
            viewport: 400.0,
            content: 800.0,
            clamped: false,
            ..MetroParallaxView::default()
        };
        assert_eq!(p.shift(999.0), 999.0);
    }

    #[test]
    fn content_rect_translates() {
        let p = MetroParallaxView {
            ratio: 0.5,
            viewport: 400.0,
            content: 800.0,
            ..MetroParallaxView::default()
        };
        let r = Rect::new(0.0, 0.0, 400.0, 300.0);
        let shifted = p.content_rect(r, 100.0);
        assert_eq!(shifted.origin.x, 50.0, "视差位移");
    }
}
