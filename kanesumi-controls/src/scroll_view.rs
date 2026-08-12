// MetroScrollView —— 滚动容器。参 CONTROL_SPEC §42（ScrollView / ScrollPresenter 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/ScrollView/`（ScrollView.idl + ScrollPresenter.cpp）：
// - ScrollableHeight = ExtentHeight − ViewportHeight（内容超视口才可滚）；
// - ScrollMode（Auto/Enabled/Disabled）、ScrollBarVisibility（Auto/Visible/Hidden）；
// - 滚轮 = 逻辑滚动（Kanesumi 离散步 50px，对齐合成器 Axis discrete）；
// - 平滑滚动 = Kanesumi 以 sokuou SpringAnim 实现（UWP 用 Composition 惯性）。
//
// Kanesumi 移植：**纯状态 + 几何**（不持视觉树）。offset 夹紧、scrollbar 拇指/轨道几何、
// 滚轮路由、可选弹簧平滑滚动。宿主渲染内容时以 `content_offset` 平移 + 视口裁剪。

use kanesumi_anim::{MetroPresets, SpringAnim};
use kanesumi_core::{Rect, Size};

/// 滚轮离散步（合成器 Axis discrete ≈ 50px/格）。
pub const SCROLL_WHEEL_STEP: f32 = 50.0;
/// 滚动条宽度（UWP ScrollBar 常规 8px，桌面 hover 展开 16px；Kanesumi 取 8）。
pub const SCROLLBAR_THICKNESS: f32 = 8.0;
/// 滚动条拇指最小长度（避免内容极长时拇指缩为点）。
pub const SCROLLBAR_MIN_THUMB: f32 = 24.0;

/// 滚动模式。对齐 ScrollingScrollMode。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollMode {
    Auto,
    Enabled,
    Disabled,
}

/// 滚动条可见性。对齐 ScrollingScrollBarVisibility。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollBarVisibility {
    Auto,
    Visible,
    Hidden,
}

/// MetroScrollView —— 滚动容器状态机。
///
/// 单轴滚动（主轴 = 传入内容/视口尺寸的对应轴）。`content_size` 由宿主更新；
/// `offset` 由 `scroll_by`/`scroll_to` 驱动（可平滑）；滚动条几何由 `vertical_scrollbar_rect`
/// 计算。内容渲染：宿主平移 `-offset` 后绘制，再以视口裁剪。
pub struct MetroScrollView {
    /// 内容尺寸（宿主维护）。
    pub content_size: Size,
    /// 视口尺寸。
    pub viewport_size: Size,
    /// 滚动偏移（主轴）。
    pub offset: f32,
    /// 滚动模式。
    pub mode: ScrollMode,
    /// 滚动条可见性。
    pub scrollbar_visibility: ScrollBarVisibility,
    /// 是否使用弹簧平滑滚动（默认开）。
    pub smooth_scroll: bool,
    /// 平滑滚动弹簧。
    spring: SpringAnim,
}

impl PartialEq for MetroScrollView {
    fn eq(&self, other: &Self) -> bool {
        self.content_size == other.content_size
            && self.viewport_size == other.viewport_size
            && self.offset == other.offset
            && self.mode == other.mode
            && self.scrollbar_visibility == other.scrollbar_visibility
            && self.smooth_scroll == other.smooth_scroll
    }
}

impl Default for MetroScrollView {
    fn default() -> Self {
        Self {
            content_size: Size::ZERO,
            viewport_size: Size::ZERO,
            offset: 0.0,
            mode: ScrollMode::Auto,
            scrollbar_visibility: ScrollBarVisibility::Auto,
            smooth_scroll: true,
            spring: MetroPresets::standard_interaction(),
        }
    }
}

impl MetroScrollView {
    pub fn new(content: Size, viewport: Size) -> Self {
        Self {
            content_size: content,
            viewport_size: viewport,
            ..Self::default()
        }
    }

    /// 可滚主轴长（内容超视口的部分）。对齐 `ScrollableHeight = Extent − Viewport`。
    /// Kanesumi 单轴垂直滚动（主轴 = height）。
    pub fn max_offset(&self) -> f32 {
        (self.content_size.height - self.viewport_size.height).max(0.0)
    }

    /// 内容是否可滚（超视口）。
    pub fn is_scrollable(&self) -> bool {
        self.max_offset() > 0.0 && self.mode != ScrollMode::Disabled
    }

    /// 是否应显示滚动条（Auto = 可滚时显示）。
    pub fn scrollbar_visible(&self) -> bool {
        match self.scrollbar_visibility {
            ScrollBarVisibility::Visible => true,
            ScrollBarVisibility::Hidden => false,
            ScrollBarVisibility::Auto => self.is_scrollable(),
        }
    }

    /// 滚轮滚动（主轴；正 = 向下）。离散步 50px。Disabled 模式不滚。
    pub fn scroll_wheel(&mut self, dy: f32) {
        if self.mode == ScrollMode::Disabled {
            return;
        }
        self.scroll_to(self.offset + dy, false);
    }

    /// 增量滚动（带平滑）。
    pub fn scroll_by(&mut self, delta: f32) {
        let target = (self.offset + delta).clamp(0.0, self.max_offset());
        if self.smooth_scroll {
            self.spring.set_target(target as f64);
        } else {
            self.offset = target;
        }
    }

    /// 直接滚动到目标偏移（夹紧）。`animate=true` 时平滑过渡。
    pub fn scroll_to(&mut self, offset: f32, animate: bool) {
        let target = offset.clamp(0.0, self.max_offset());
        if animate && self.smooth_scroll {
            self.spring.set_target(target as f64);
        } else {
            self.offset = target;
            self.spring.snap();
            self.spring.set_target(target as f64);
        }
    }

    /// 跳到指定项（`item_main_pos` = 条目主轴起点，`item_extent` = 条目主轴长）。
    pub fn scroll_into_view(&mut self, item_main_pos: f32, item_extent: f32, animate: bool) {
        let viewport = self.viewport_size.height;
        let cur = self.offset;
        let target = if item_main_pos < cur {
            item_main_pos
        } else if item_main_pos + item_extent > cur + viewport {
            item_main_pos + item_extent - viewport
        } else {
            cur
        };
        self.scroll_to(target, animate);
    }

    /// 每帧推进平滑滚动。
    pub fn update(&mut self, dt: f64) {
        if self.smooth_scroll {
            self.spring.update(dt);
            self.offset = self.spring.value() as f32;
        }
    }

    /// 是否正在平滑滚动。
    pub fn is_animating(&self) -> bool {
        !self.spring.is_steady()
    }

    /// 滚动条轨道矩形（主轴 = 垂直滚动条，右缘 8px 宽）。
    pub fn scrollbar_track_rect(&self) -> Rect {
        let v = self.viewport_size;
        Rect::new(
            v.width - SCROLLBAR_THICKNESS,
            0.0,
            SCROLLBAR_THICKNESS,
            v.height,
        )
    }

    /// 滚动条拇指矩形。大小 = 视口/内容比例 × 轨道长，下限 SCROLLBAR_MIN_THUMB；
    /// 位置 = 偏移/内容长 × 轨道长。
    pub fn scrollbar_thumb_rect(&self) -> Rect {
        let track = self.scrollbar_track_rect();
        let max = self.max_offset();
        if max <= 0.0 {
            return Rect::new(track.origin.x, track.origin.y, track.size.width, 0.0);
        }
        let ratio = track.size.height / (self.content_size.height.max(1.0));
        let thumb_h = (ratio * track.size.height).max(SCROLLBAR_MIN_THUMB);
        let travel = (track.size.height - thumb_h).max(0.0);
        let y = travel * (self.offset / max);
        Rect::new(
            track.origin.x + 1.0,
            track.origin.y + y,
            track.size.width - 2.0,
            thumb_h,
        )
    }

    /// 内容平移偏移（渲染内容前应用）。
    pub fn content_offset(&self) -> f32 {
        -self.offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn max_offset_clamps_to_zero() {
        let sv = MetroScrollView::new(Size::new(200.0, 100.0), Size::new(200.0, 100.0));
        assert_eq!(sv.max_offset(), 0.0, "内容 ≤ 视口不可滚");
        let sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        assert_eq!(sv.max_offset(), 200.0, "内容 300 视口 100 → 可滚 200");
    }

    #[test]
    fn scroll_wheel_steps_by_50() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_wheel(50.0);
        assert_eq!(sv.offset, 50.0);
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 150.0);
        // 超界夹紧
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 200.0, "夹紧到 max_offset");
    }

    #[test]
    fn scroll_to_clamps() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(500.0, false);
        assert_eq!(sv.offset, 200.0);
        sv.scroll_to(-10.0, false);
        assert_eq!(sv.offset, 0.0);
    }

    #[test]
    fn smooth_scroll_animates_then_settles() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        assert!(sv.smooth_scroll);
        sv.scroll_to(100.0, true);
        assert!(sv.is_animating(), "平滑滚动进行中");
        for _ in 0..600 {
            sv.update(1.0 / 60.0);
        }
        assert!(!sv.is_animating());
        assert!((sv.offset - 100.0).abs() < 1.0, "稳定到目标，实际 {}", sv.offset);
    }

    #[test]
    fn scroll_into_view_brings_item_into_viewport() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 300.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        // 条目 250..290 不在 [0,100) → 滚到 250+40-100=190
        sv.scroll_into_view(250.0, 40.0, false);
        assert_eq!(sv.offset, 190.0);
        // 条目在视口内 → 不滚
        sv.scroll_into_view(200.0, 40.0, false);
        assert_eq!(sv.offset, 190.0, "已在视口内不滚动");
        // 条目在视口上方 → 回滚到其上缘
        sv.scroll_into_view(30.0, 40.0, false);
        assert_eq!(sv.offset, 30.0, "视口上方条目回滚到上缘");
    }

    #[test]
    fn scrollbar_thumb_geometry() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(0.0, false);
        let thumb0 = sv.scrollbar_thumb_rect();
        assert_eq!(thumb0.size.width, SCROLLBAR_THICKNESS - 2.0);
        assert_eq!(thumb0.origin.y, 0.0, "offset 0 拇指在顶");
        // 滚到中间 → 拇指下移
        sv.scroll_to(150.0, false);
        let thumb_mid = sv.scrollbar_thumb_rect();
        assert!(
            thumb_mid.origin.y > thumb0.origin.y,
            "滚动后拇指下移"
        );
        // 拇指高度 = 100/400 * 100 = 25（> min 24）
        assert!((thumb_mid.size.height - 25.0).abs() < 1.0);
    }

    #[test]
    fn scrollbar_thumb_min_size() {
        // 极长内容：拇指缩到最小
        let mut sv = MetroScrollView::new(Size::new(200.0, 5000.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        let thumb = sv.scrollbar_thumb_rect();
        assert_eq!(thumb.size.height, SCROLLBAR_MIN_THUMB, "拇指不小于 24");
    }

    #[test]
    fn auto_scrollbar_only_when_scrollable() {
        let small = MetroScrollView::new(Size::new(200.0, 80.0), Size::new(200.0, 100.0));
        assert!(!small.scrollbar_visible(), "内容不足不显示滚动条");
        let big = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        assert!(big.scrollbar_visible());
    }

    #[test]
    fn disabled_mode_blocks_scroll() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.mode = ScrollMode::Disabled;
        sv.smooth_scroll = false;
        sv.scroll_wheel(100.0);
        assert_eq!(sv.offset, 0.0, "Disabled 不滚动");
    }

    #[test]
    fn content_offset_negates_scroll() {
        let mut sv = MetroScrollView::new(Size::new(200.0, 400.0), Size::new(200.0, 100.0));
        sv.smooth_scroll = false;
        sv.scroll_to(100.0, false);
        assert_eq!(sv.content_offset(), -100.0);
    }
}
