// MetroRepeater —— 虚拟化布局引擎。参 CONTROL_SPEC §41（Repeater 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/Repeater/`（FlowLayout.cpp + ItemsRepeater.cpp）。
// WinUI Repeater 是元素工厂 + 回收复用；Kanesumi 状态驱动渲染无保留视觉树，
// 移植其**虚拟化核心**：给定视口 + 滚动偏移，只计算可见项的范围与矩形——
// MetroList / TabView / TreeView / Grid 长列表共用，避免全量渲染掉帧。
//
// 支持 StackLayout（单轴堆叠，横/纵）+ UniformGridLayout（等宽网格）。
// 纯数据、跨平台可测：不依赖 Scene / 字体，只算几何。

use kanesumi_core::{Point, Rect, Size};

/// 布局方向（FlowLayout 主轴）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterOrientation {
    Vertical,
    Horizontal,
}

/// 布局模式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeaterLayout {
    /// StackLayout：单轴等尺寸堆叠（List / 单选列表）。
    Stack,
    /// UniformGridLayout：等宽网格（GridView / 磁贴）。
    UniformGrid,
}

/// MetroRepeater —— 虚拟化布局引擎。
///
/// 状态：条目数 + 单元尺寸 + 布局模式 + 主轴间距。纯几何：`visible_range` 返回
/// 应渲染的首末索引，`item_rect` 返回任意条目绝对矩形，`render` 前调用者只画可见项。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroRepeater {
    /// 条目总数。
    pub item_count: usize,
    /// 主轴单元尺寸（Stack：item_height；UniformGrid：cell 边长）。
    pub main_extent: f32,
    /// 次轴单元尺寸（Stack：撑满视口；UniformGrid：cell 边长）。
    pub cross_extent: f32,
    /// 条目间距（主轴）。WinUI StackLayout `Spacing`，默认 0。
    pub spacing: f32,
    /// 布局模式。
    pub layout: RepeaterLayout,
    /// 主轴方向。
    pub orientation: RepeaterOrientation,
    /// UniformGrid 列数（Stack 模式忽略）。
    pub columns: usize,
}

impl Default for MetroRepeater {
    fn default() -> Self {
        Self {
            item_count: 0,
            main_extent: 40.0,
            cross_extent: 0.0,
            spacing: 0.0,
            layout: RepeaterLayout::Stack,
            orientation: RepeaterOrientation::Vertical,
            columns: 1,
        }
    }
}

impl MetroRepeater {
    /// 纵向 Stack 便捷构造。
    pub fn stack_vertical(count: usize, item_height: f32) -> Self {
        Self {
            item_count: count,
            main_extent: item_height,
            layout: RepeaterLayout::Stack,
            orientation: RepeaterOrientation::Vertical,
            ..Self::default()
        }
    }

    /// 横向 Stack 便捷构造。
    pub fn stack_horizontal(count: usize, item_width: f32) -> Self {
        Self {
            item_count: count,
            main_extent: item_width,
            layout: RepeaterLayout::Stack,
            orientation: RepeaterOrientation::Horizontal,
            ..Self::default()
        }
    }

    /// 均匀网格构造（`columns` 列）。
    pub fn grid(count: usize, cell: f32, columns: usize, spacing: f32) -> Self {
        Self {
            item_count: count,
            main_extent: cell,
            cross_extent: cell,
            spacing,
            layout: RepeaterLayout::UniformGrid,
            orientation: RepeaterOrientation::Vertical,
            columns: columns.max(1),
        }
    }

    /// 条目的主轴跨度（含间距）。
    fn item_stride(&self) -> f32 {
        self.main_extent + self.spacing
    }

    /// 内容总主轴长。
    pub fn content_length(&self) -> f32 {
        match self.layout {
            RepeaterLayout::Stack => {
                self.item_count as f32 * self.item_stride() - self.spacing
            }
            RepeaterLayout::UniformGrid => {
                let rows = self.item_count.div_ceil(self.columns);
                rows as f32 * self.item_stride() - self.spacing
            }
        }
        .max(0.0)
    }

    /// 内容总次轴长（Stack = cross_extent；Grid = 满列宽）。
    pub fn content_cross(&self) -> f32 {
        match self.layout {
            RepeaterLayout::Stack => self.cross_extent,
            RepeaterLayout::UniformGrid => {
                self.columns as f32 * self.main_extent
                    + (self.columns as f32 - 1.0) * self.spacing
            }
        }
    }

    /// 内容整体尺寸（给定视口次轴尺寸决定 Stack 的 cross_extent）。
    pub fn content_size(&self, viewport: Size) -> Size {
        match self.orientation {
            RepeaterOrientation::Vertical => Size::new(
                viewport.width,
                self.content_length(),
            ),
            RepeaterOrientation::Horizontal => Size::new(
                self.content_length(),
                viewport.height,
            ),
        }
    }

    /// 可见条目范围 `Some((first, last_inclusive))`；无可见项 → None（虚拟化核心）。
    ///
    /// `offset` = 滚动偏移（主轴；垂直 = +y 下滚）。滚过内容末尾 → None。
    pub fn visible_range(&self, viewport_len: f32, offset: f32) -> Option<(usize, usize)> {
        if self.item_count == 0 || viewport_len <= 0.0 {
            return None;
        }
        let stride = self.item_stride();
        let first = (offset / stride).floor() as usize;
        if first >= self.item_count {
            // 已滚过内容末尾（offset 超 content_length）→ 无可见项
            return None;
        }
        // 项 i 可见 ⟺ i*stride < offset+viewport；i_max = ceil((offset+viewport)/stride) − 1
        let last = ((offset + viewport_len) / stride).ceil() as usize;
        let last = last.saturating_sub(1).min(self.item_count.saturating_sub(1));
        Some((first, last.max(first)))
    }

    /// 条目矩形（绝对坐标，含滚动偏移）。主轴方向决定 x/y。
    pub fn item_rect(&self, index: usize, viewport: Size, offset: f32) -> Rect {
        let index = index.min(self.item_count.saturating_sub(1));
        match self.layout {
            RepeaterLayout::Stack => match self.orientation {
                RepeaterOrientation::Vertical => Rect::new(
                    0.0,
                    index as f32 * self.item_stride() - offset,
                    viewport.width,
                    self.main_extent,
                ),
                RepeaterOrientation::Horizontal => Rect::new(
                    index as f32 * self.item_stride() - offset,
                    0.0,
                    self.main_extent,
                    viewport.height,
                ),
            },
            RepeaterLayout::UniformGrid => {
                let col = index % self.columns;
                let row = index / self.columns;
                Rect::new(
                    col as f32 * (self.main_extent + self.spacing),
                    row as f32 * (self.main_extent + self.spacing) - offset,
                    self.main_extent,
                    self.main_extent,
                )
            }
        }
    }

    /// 把任意条目滚进视口（返回目标滚动偏移；已可见则返回当前 offset）。
    pub fn scroll_into_view(&self, index: usize, viewport_len: f32, offset: f32) -> f32 {
        let rect = self.item_rect(index, Size::new(0.0, 0.0), 0.0);
        let main = match self.orientation {
            RepeaterOrientation::Vertical => rect.origin.y,
            RepeaterOrientation::Horizontal => rect.origin.x,
        };
        let extent = self.main_extent;
        let max = (self.content_length() - viewport_len).max(0.0);
        if main < offset {
            main.clamp(0.0, max)
        } else if main + extent > offset + viewport_len {
            (main + extent - viewport_len).clamp(0.0, max)
        } else {
            offset
        }
    }

    /// 可见条目矩形（已裁剪到视口内）。
    pub fn visible_rects(&self, viewport: Size, offset: f32) -> Vec<Rect> {
        let Some((lo, hi)) = self.visible_range(
            match self.orientation {
                RepeaterOrientation::Vertical => viewport.height,
                RepeaterOrientation::Horizontal => viewport.width,
            },
            offset,
        ) else {
            return Vec::new();
        };
        (lo..=hi).map(|i| self.item_rect(i, viewport, offset)).collect()
    }

    /// 命中条目：把视口内点映射到条目索引（含 offset 还原）。
    pub fn item_at(&self, viewport: Size, offset: f32, pos: Point) -> Option<usize> {
        if !Rect::new(0.0, 0.0, viewport.width, viewport.height).contains(pos) {
            return None;
        }
        let main = match self.orientation {
            RepeaterOrientation::Vertical => pos.y + offset,
            RepeaterOrientation::Horizontal => pos.x + offset,
        };
        if main < 0.0 {
            return None;
        }
        let idx = (main / self.item_stride()) as usize;
        if idx >= self.item_count {
            return None;
        }
        // 校验在单元范围内（命中空白间距 → None）
        let within = main - idx as f32 * self.item_stride() <= self.main_extent;
        within.then_some(idx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_content_length() {
        let r = MetroRepeater::stack_vertical(5, 40.0);
        assert_eq!(r.content_length(), 200.0);
        let r = MetroRepeater::stack_vertical(5, 40.0);
        assert_eq!(r.content_length(), 200.0);
    }

    #[test]
    fn stack_with_spacing() {
        let mut r = MetroRepeater::stack_vertical(5, 40.0);
        r.spacing = 8.0;
        assert_eq!(r.content_length(), 5.0 * 48.0 - 8.0);
    }

    #[test]
    fn visible_range_partial_overlap() {
        let r = MetroRepeater::stack_vertical(100, 40.0);
        // 视口高 100，offset 30 → 覆盖 [30, 130) → 条目 0(0-40) 到 3(120-160)
        assert_eq!(r.visible_range(100.0, 30.0), Some((0, 3)));
    }

    #[test]
    fn visible_range_scrolled_middle() {
        let r = MetroRepeater::stack_vertical(100, 40.0);
        assert_eq!(r.visible_range(100.0, 200.0), Some((5, 7)), "offset 200 → 项 5-7");
    }

    #[test]
    fn visible_range_empty() {
        let r = MetroRepeater::stack_vertical(0, 40.0);
        assert_eq!(r.visible_range(100.0, 0.0), None);
    }

    #[test]
    fn visible_range_past_end() {
        let r = MetroRepeater::stack_vertical(3, 40.0);
        assert_eq!(r.visible_range(200.0, 500.0), None, "滚过内容末尾无可见项");
    }

    #[test]
    fn item_rect_offsets_by_scroll() {
        let r = MetroRepeater::stack_vertical(10, 40.0);
        let rect = r.item_rect(3, Size::new(200.0, 100.0), 50.0);
        assert_eq!(rect.origin.y, 3.0 * 40.0 - 50.0);
        assert_eq!(rect.size.height, 40.0);
    }

    #[test]
    fn horizontal_stack() {
        let r = MetroRepeater::stack_horizontal(10, 60.0);
        let rect = r.item_rect(2, Size::new(200.0, 100.0), 30.0);
        assert_eq!(rect.origin.x, 2.0 * 60.0 - 30.0);
        assert_eq!(rect.size.width, 60.0);
        assert_eq!(r.content_length(), 600.0);
    }

    #[test]
    fn uniform_grid_rows() {
        let r = MetroRepeater::grid(9, 32.0, 4, 8.0);
        // 9 项 4 列 → 3 行
        assert_eq!(r.content_length(), 3.0 * 40.0 - 8.0);
        let rect = r.item_rect(5, Size::new(200.0, 200.0), 0.0);
        // 第 1 行（row=1, col=1）
        assert_eq!(rect.origin.x, 1.0 * 40.0);
        assert_eq!(rect.origin.y, 1.0 * 40.0);
    }

    #[test]
    fn scroll_into_view_brings_item_in() {
        let r = MetroRepeater::stack_vertical(100, 40.0);
        // 项 80（[3200,3240)）不在 [0,100) → 最小滚动使其完全可见：底边贴视口底
        let target = r.scroll_into_view(80, 100.0, 0.0);
        assert_eq!(target, 80.0 * 40.0 + 40.0 - 100.0, "底边贴视口底");
        // 部分可见（项 2 [80,120) 底边越出 [0,100)）→ 滚到贴底
        assert_eq!(r.scroll_into_view(2, 100.0, 0.0), 20.0);
        // 完全可见（项 1 [40,80) ⊂ [0,100)）→ 不滚
        assert_eq!(r.scroll_into_view(1, 100.0, 0.0), 0.0);
    }

    #[test]
    fn item_at_maps_position() {
        let r = MetroRepeater::stack_vertical(10, 40.0);
        let viewport = Size::new(200.0, 100.0);
        // offset 30 → 视口 y=10 对应内容 y=40 → 项 1
        assert_eq!(r.item_at(viewport, 30.0, Point::new(10.0, 10.0)), Some(1));
        // 视口外
        assert_eq!(r.item_at(viewport, 30.0, Point::new(10.0, 200.0)), None);
    }

    #[test]
    fn visible_rects_matches_range() {
        let r = MetroRepeater::stack_vertical(100, 40.0);
        let rects = r.visible_rects(Size::new(200.0, 100.0), 200.0);
        assert_eq!(rects.len(), 3, "offset 200 视口 100 → 3 项");
        assert_eq!(rects[0].origin.y, 5.0 * 40.0 - 200.0);
    }
}
