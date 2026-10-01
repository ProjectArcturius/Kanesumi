// MetroTwoPaneView —— 双面板自适应容器。参 CONTROL_SPEC §22。
//
// 移植自 microsoft-ui-xaml/dev/TwoPaneView（TwoPaneView.cpp + TwoPaneView.xaml）：
// - MinWideModeWidth 641 / MinTallModeHeight 641；
// - 宽切 Wide（LeftRight/RightLeft），高切 Tall（TopBottom/BottomTop），否则 SinglePane；
// - Kanesumi 为纯布局容器：`pane_rects(rect)` 返回两面板矩形，宿主渲染内容。
// 多显示区域（折叠屏 hinge）逻辑不移植（Ether 桌面单屏）。

use kanesumi_core::{Rect, Size};

/// 双面板优先级（SinglePane 模式显示哪个）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoPanePriority {
    Pane1,
    Pane2,
}

/// 宽屏并排配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoPaneWideConfig {
    SinglePane,
    LeftRight,
    RightLeft,
}

/// 高屏堆叠配置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoPaneTallConfig {
    SinglePane,
    TopBottom,
    BottomTop,
}

/// 当前模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwoPaneMode {
    SinglePane,
    Wide,
    Tall,
}

/// 默认宽屏切换阈值（MinWideModeWidth）。
pub const DEFAULT_MIN_WIDE: f32 = 641.0;
/// 默认高屏切换阈值（MinTallModeHeight）。
pub const DEFAULT_MIN_TALL: f32 = 641.0;

/// MetroTwoPaneView —— 双面板容器。参 CONTROL_SPEC §22。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroTwoPaneView {
    pub min_wide_width: f32,
    pub min_tall_height: f32,
    pub wide_config: TwoPaneWideConfig,
    pub tall_config: TwoPaneTallConfig,
    pub pane_priority: TwoPanePriority,
    /// Pane1 占主轴向比例（0..1，默认 0.5）。
    pub pane1_ratio: f32,
    /// 最近一次 `arrange` 时的模式（供 App 查询；`mode(rect)` 仍是纯函数）。
    current: TwoPaneMode,
}

impl Default for MetroTwoPaneView {
    fn default() -> Self {
        Self {
            min_wide_width: DEFAULT_MIN_WIDE,
            min_tall_height: DEFAULT_MIN_TALL,
            wide_config: TwoPaneWideConfig::LeftRight,
            tall_config: TwoPaneTallConfig::TopBottom,
            pane_priority: TwoPanePriority::Pane1,
            pane1_ratio: 0.5,
            current: TwoPaneMode::SinglePane,
        }
    }
}

impl MetroTwoPaneView {
    pub fn new() -> Self {
        Self::default()
    }

    /// 最近一次 `arrange` 记录的模式（等宽切换无需动作，App 据此按模式调整内容）。
    pub fn current_mode(&self) -> TwoPaneMode {
        self.current
    }

    /// 当前模式（UpdateMode 单区域判据）。
    pub fn mode(&self, rect: Rect) -> TwoPaneMode {
        if rect.size.width > self.min_wide_width
            && self.wide_config != TwoPaneWideConfig::SinglePane
        {
            TwoPaneMode::Wide
        } else if rect.size.height > self.min_tall_height
            && self.tall_config != TwoPaneTallConfig::SinglePane
        {
            TwoPaneMode::Tall
        } else {
            TwoPaneMode::SinglePane
        }
    }

    /// 两面板矩形（SinglePane 时另一面板为空）。返回 (pane1, pane2)。
    pub fn pane_rects(&self, rect: Rect) -> (Rect, Rect) {
        let mode = self.mode(rect);
        let w = rect.size.width;
        let h = rect.size.height;
        let r = self.pane1_ratio.clamp(0.0, 1.0);
        let empty = Rect::new(0.0, 0.0, 0.0, 0.0);
        match mode {
            TwoPaneMode::Wide => {
                let split = rect.origin.x + w * r;
                let pane1 = Rect::new(rect.origin.x, rect.origin.y, split - rect.origin.x, h);
                let pane2 = Rect::new(split, rect.origin.y, w - (split - rect.origin.x), h);
                if self.wide_config == TwoPaneWideConfig::RightLeft {
                    (pane2, pane1)
                } else {
                    (pane1, pane2)
                }
            }
            TwoPaneMode::Tall => {
                let split = rect.origin.y + h * r;
                let pane1 = Rect::new(rect.origin.x, rect.origin.y, w, split - rect.origin.y);
                let pane2 = Rect::new(rect.origin.x, split, w, h - (split - rect.origin.y));
                if self.tall_config == TwoPaneTallConfig::BottomTop {
                    (pane2, pane1)
                } else {
                    (pane1, pane2)
                }
            }
            TwoPaneMode::SinglePane => match self.pane_priority {
                TwoPanePriority::Pane1 => (rect, empty),
                TwoPanePriority::Pane2 => (empty, rect),
            },
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 navigation_view.rs）────────
//
// **纯布局容器**：前两个子节点分别排进 `pane_rects(rect)` 的两块；SinglePane 模式下被隐藏
// 的那块排进零尺寸槽位（不可见、不可命中）。无自绘、`hit_test` 恒 false（空白穿透）。
// 模式随尺寸变化不需要动作（下一次 arrange 自然按新尺寸重排），只在 arrange 里记下当前
// 模式供 App 经 `current_mode()` 查询。

impl kanesumi_element::Widget for MetroTwoPaneView {
    /// 容器铺满可用尺寸。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: Size,
    ) -> Size {
        let w = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let h = if available.height.is_finite() {
            available.height.max(0.0)
        } else {
            0.0
        };
        Size::new(w, h)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.current = self.mode(rect);
        let (pane1, pane2) = self.pane_rects(rect);
        let children = ctx.children();
        if let Some(&c) = children.first() {
            // 先量测再排版：非拉伸面板要靠期望尺寸定位（未量测 → 期望为 0）。
            ctx.measure_child(c, pane1.size);
            ctx.arrange_child(c, pane1);
        }
        if let Some(&c) = children.get(1) {
            ctx.measure_child(c, pane2.size);
            ctx.arrange_child(c, pane2);
        }
    }

    fn paint(&mut self, _ctx: &mut kanesumi_element::PaintCtx, _scene: &mut kanesumi_canvas::Scene) {}

    /// 空白穿透：面板内容由子节点命中，容器本身不拦截。
    fn hit_test(&self, _rect: Rect, _pos: kanesumi_core::Point) -> bool {
        false
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Label;
    use kanesumi_element::{Align, Insets, LayoutProps, WidgetId};

    fn harness(width: f32, height: f32) -> (TestHarness, WidgetId, WidgetId, WidgetId) {
        let mut h = TestHarness::new(width, height);
        let props = LayoutProps {
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        };
        let id = h.tree.insert_with(h.root(), MetroTwoPaneView::new(), props);
        let pane1 = h.tree.insert(id, Label::new("左"));
        let pane2 = h.tree.insert(id, Label::new("右"));
        h.frame();
        (h, id, pane1, pane2)
    }

    #[test]
    fn wide_splits_two_panes_without_overlap() {
        let (h, id, p1, p2) = harness(1000.0, 600.0);
        assert_eq!(
            h.tree.get::<MetroTwoPaneView>(id).unwrap().current_mode(),
            TwoPaneMode::Wide
        );
        let a = h.rect(p1);
        let b = h.rect(p2);
        assert!(a.size.width > 0.0 && b.size.width > 0.0, "两块都有面积");
        assert!(a.right() <= b.origin.x + 0.01, "左右不重叠");
    }

    #[test]
    fn narrow_shows_priority_pane_only() {
        let (h, id, p1, p2) = harness(500.0, 500.0);
        assert_eq!(
            h.tree.get::<MetroTwoPaneView>(id).unwrap().current_mode(),
            TwoPaneMode::SinglePane
        );
        assert!(h.rect(p1).size.width > 0.0, "优先面板 Pane1 有面积");
        assert_eq!(h.rect(p2), Rect::new(0.0, 0.0, 0.0, 0.0), "另一块零尺寸");
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id, _, _) = harness(1000.0, 600.0);
        assert_eq!(h.rect(id).size, Size::new(1000.0, 600.0));
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_uwp() {
        let t = MetroTwoPaneView::new();
        assert_eq!(t.min_wide_width, 641.0);
        assert_eq!(t.min_tall_height, 641.0);
        assert_eq!(t.wide_config, TwoPaneWideConfig::LeftRight);
        assert_eq!(t.tall_config, TwoPaneTallConfig::TopBottom);
    }

    #[test]
    fn narrow_is_single_pane_pane1() {
        let t = MetroTwoPaneView::new();
        let rect = Rect::new(0.0, 0.0, 500.0, 500.0);
        assert_eq!(t.mode(rect), TwoPaneMode::SinglePane);
        let (p1, p2) = t.pane_rects(rect);
        assert_eq!(p1, rect);
        assert_eq!(p2.size.width, 0.0);
    }

    #[test]
    fn wide_splits_horizontally() {
        let t = MetroTwoPaneView::new();
        let rect = Rect::new(0.0, 0.0, 1000.0, 600.0);
        assert_eq!(t.mode(rect), TwoPaneMode::Wide);
        let (p1, p2) = t.pane_rects(rect);
        assert!((p1.size.width - 500.0).abs() < 0.01, "pane1 左半");
        assert!((p2.size.width - 500.0).abs() < 0.01);
        assert_eq!(p1.right(), p2.origin.x);
    }

    #[test]
    fn wide_right_left_swaps() {
        let t = MetroTwoPaneView {
            wide_config: TwoPaneWideConfig::RightLeft,
            ..MetroTwoPaneView::default()
        };
        let rect = Rect::new(0.0, 0.0, 1000.0, 600.0);
        let (p1, p2) = t.pane_rects(rect);
        assert!(p1.origin.x > p2.origin.x, "RightLeft → Pane1 在右");
    }

    #[test]
    fn tall_splits_vertically() {
        let t = MetroTwoPaneView {
            min_wide_width: 9999.0,
            ..MetroTwoPaneView::default()
        };
        let rect = Rect::new(0.0, 0.0, 600.0, 1000.0);
        assert_eq!(t.mode(rect), TwoPaneMode::Tall);
        let (p1, p2) = t.pane_rects(rect);
        assert!((p1.size.height - 500.0).abs() < 0.01, "pane1 上半");
        assert_eq!(p1.bottom(), p2.origin.y);
    }

    #[test]
    fn ratio_adjusts_split() {
        let t = MetroTwoPaneView {
            pane1_ratio: 0.3,
            ..MetroTwoPaneView::default()
        };
        let rect = Rect::new(0.0, 0.0, 1000.0, 600.0);
        let (p1, _) = t.pane_rects(rect);
        assert!((p1.size.width - 300.0).abs() < 0.01);
    }

    #[test]
    fn pane_priority_selects_single() {
        let t = MetroTwoPaneView {
            pane_priority: TwoPanePriority::Pane2,
            ..MetroTwoPaneView::default()
        };
        let rect = Rect::new(0.0, 0.0, 500.0, 500.0);
        let (p1, p2) = t.pane_rects(rect);
        assert_eq!(p2, rect, "Pane2 优先级 → 单面板显示 Pane2");
        assert_eq!(p1.size.width, 0.0);
    }

    #[test]
    fn config_single_pane_forced() {
        let t = MetroTwoPaneView {
            wide_config: TwoPaneWideConfig::SinglePane,
            ..MetroTwoPaneView::default()
        };
        let rect = Rect::new(0.0, 0.0, 1000.0, 600.0);
        assert_eq!(t.mode(rect), TwoPaneMode::SinglePane, "强制单面板");
    }
}
