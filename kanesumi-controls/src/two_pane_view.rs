// MetroTwoPaneView —— 双面板自适应容器。参 CONTROL_SPEC §22。
//
// 移植自 microsoft-ui-xaml/dev/TwoPaneView（TwoPaneView.cpp + TwoPaneView.xaml）：
// - MinWideModeWidth 641 / MinTallModeHeight 641；
// - 宽切 Wide（LeftRight/RightLeft），高切 Tall（TopBottom/BottomTop），否则 SinglePane；
// - Kanesumi 为纯布局容器：`pane_rects(rect)` 返回两面板矩形，宿主渲染内容。
// 多显示区域（折叠屏 hinge）逻辑不移植（Ether 桌面单屏）。

use kanesumi_core::Rect;

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
        }
    }
}

impl MetroTwoPaneView {
    pub fn new() -> Self {
        Self::default()
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
