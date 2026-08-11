// MetroParallaxView —— 视差滚动。参 CONTROL_SPEC §30。
//
// 移植自 microsoft-ui-xaml/dev/ParallaxView（ParallaxView.cpp + ParallaxView.idl）：
// - shift = scroll_offset × ratio，clamp 到 [−MaxShift, +MaxShift]；
// - MaxShift = MaxShiftRatio × 视口主轴。
// 纯布局/位移辅助（宿主渲染内容），无自绘。

use kanesumi_core::Rect;

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
}

impl Default for MetroParallaxView {
    fn default() -> Self {
        Self {
            ratio: DEFAULT_PARALLAX_RATIO,
            viewport: 0.0,
            content: 0.0,
            max_shift_ratio: 1.0,
            clamped: true,
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
