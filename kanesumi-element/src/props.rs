// props.rs —— 框架级布局属性。参 ELEMENT_TREE §Ⅳ.2（对应 XAML FrameworkElement）。
//
// 这些属性由框架解释，控件不必（也不应）自己处理 Margin / 对齐 / 夹紧 ——
// 否则就回到了「每个控件各算一套几何」的老路。

use kanesumi_core::{Rect, Size};

/// 四边距（外边距 / 内边距）。
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    pub const ZERO: Insets = Insets::all(0.0);

    pub const fn all(v: f32) -> Self {
        Self {
            left: v,
            top: v,
            right: v,
            bottom: v,
        }
    }

    /// 水平 / 垂直对称（XAML `Thickness(h, v)`）。
    pub const fn symmetric(h: f32, v: f32) -> Self {
        Self {
            left: h,
            top: v,
            right: h,
            bottom: v,
        }
    }

    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self {
            left,
            top,
            right,
            bottom,
        }
    }

    pub fn horizontal(self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(self) -> f32 {
        self.top + self.bottom
    }

    /// 从尺寸扣除（下限 0；无穷保持无穷）。
    pub fn shrink(self, size: Size) -> Size {
        Size::new(
            (size.width - self.horizontal()).max(0.0),
            (size.height - self.vertical()).max(0.0),
        )
    }

    pub fn grow(self, size: Size) -> Size {
        Size::new(size.width + self.horizontal(), size.height + self.vertical())
    }

    /// 矩形内缩（宽高下限 0，不翻转）。
    pub fn deflate(self, rect: Rect) -> Rect {
        let w = (rect.size.width - self.horizontal()).max(0.0);
        let h = (rect.size.height - self.vertical()).max(0.0);
        Rect::new(rect.origin.x + self.left, rect.origin.y + self.top, w, h)
    }

    /// 矩形外扩（绘制越界许可用）。
    pub fn inflate(self, rect: Rect) -> Rect {
        Rect::new(
            rect.origin.x - self.left,
            rect.origin.y - self.top,
            rect.size.width + self.horizontal(),
            rect.size.height + self.vertical(),
        )
    }
}

/// 在父分配槽位内的对齐（XAML `HorizontalAlignment` / `VerticalAlignment`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    /// 撑满槽位（XAML 默认）。
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

/// 框架级布局属性。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutProps {
    pub margin: Insets,
    /// 固定宽 / 高（XAML `Width` / `Height`）。
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min: Size,
    pub max: Size,
    pub h_align: Align,
    pub v_align: Align,
    /// 仅对 `Stack` 父有效：主轴剩余空间分配权重（Star）。
    pub grow: f32,
    /// 仅对 `Stack` 父有效：主轴不足时的压缩权重（0 = 不可压缩）。
    pub shrink: f32,
    /// 仅对 `Stack` 父有效：压缩下限。
    pub min_main: f32,
    /// `false` = 不量测、不绘制、不命中、不可聚焦（XAML `Collapsed`）。
    pub visible: bool,
}

impl Default for LayoutProps {
    fn default() -> Self {
        Self {
            margin: Insets::ZERO,
            width: None,
            height: None,
            min: Size::ZERO,
            max: Size::new(f32::INFINITY, f32::INFINITY),
            h_align: Align::Stretch,
            v_align: Align::Stretch,
            grow: 0.0,
            shrink: 1.0,
            min_main: 0.0,
            visible: true,
        }
    }
}

impl LayoutProps {
    /// 把控件内容尺寸按固定尺寸 / min / max 夹紧（不含 margin）。
    pub(crate) fn clamp_size(&self, size: Size) -> Size {
        let w = self.width.unwrap_or(size.width);
        let h = self.height.unwrap_or(size.height);
        Size::new(
            clamp_axis(w, self.min.width, self.max.width),
            clamp_axis(h, self.min.height, self.max.height),
        )
    }

    /// 量测时给控件的可用尺寸：扣 margin，再被固定尺寸 / max 收窄。
    pub(crate) fn inner_available(&self, available: Size) -> Size {
        let a = self.margin.shrink(available);
        let w = self.width.unwrap_or(a.width).min(self.max.width).min(a.width);
        let h = self.height.unwrap_or(a.height).min(self.max.height).min(a.height);
        Size::new(w.max(0.0), h.max(0.0))
    }
}

/// min 优先于 max（XAML 语义：MinWidth > MaxWidth 时取 MinWidth）；NaN / 负 → 0。
fn clamp_axis(v: f32, min: f32, max: f32) -> f32 {
    let v = if v.is_nan() { 0.0 } else { v.max(0.0) };
    v.min(max).max(min.max(0.0))
}
