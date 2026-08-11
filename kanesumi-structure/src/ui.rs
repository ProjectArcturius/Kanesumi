// ui.rs —— V22 (A2) 布局器·渐进 A（egui-style 最小模型）
//
// 参 SESSION_HANDOVER §4.2：不动现有控件，`Ui` 是新叠加层。控件仍以 `Rect`
// 为输入，Gallery/App 层用 `Ui::allocate` 分配矩形替代硬编码常量（
// `PAD` / `CTRL_Y0` / `switch_rect` 之类）。逐控件迁移，一次一小步。
//
// 与 egui 差异：本层无「样式 / 视觉」概念，纯几何 —— 主轴 cursor 沿 direction
// 前进，`allocate(size)` 切出一块 `Rect`。子域（`horizontal`/`vertical`）
// 在剩余空间里开新 `Ui`，用完后按 `min_rect` 大小推进父 cursor。

use kanesumi_core::{Rect, Size};

/// 主轴方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutDirection {
    /// 主轴 = X（左→右）。等价 egui `Layout::left_to_right`。
    Horizontal,
    /// 主轴 = Y（上→下）。等价 egui `Layout::top_down`。
    Vertical,
}

/// 布局上下文 —— egui 风格最小模型。
///
/// - `max_rect`：可用总空间边界；子域调用时会收窄。
/// - `cursor`：主轴上下一次 `allocate` 起点（坐标绝对，非相对 `max_rect`）。
/// - `min_rect`：已放置内容的紧致包围盒；主轴长 = 起点 → 最后 rect 结束，
///   次轴长 = 所有 rect 次轴分量的 max。空 `Ui` 的 `min_rect` = 起点 0×0。
/// - `direction`：主轴方向。
/// - `spacing`：`allocate` 后主轴自动追加的间距（默认 0）。尾部 spacing 不计入 `min_rect`。
#[derive(Debug, Clone, PartialEq)]
pub struct Ui {
    pub max_rect: Rect,
    pub cursor: f32,
    pub min_rect: Rect,
    pub direction: LayoutDirection,
    pub spacing: f32,
}

impl Ui {
    /// 以给定 `max_rect` + 方向创建 Ui。`cursor` 起点 = max_rect 相应主轴边。
    pub fn new(max_rect: Rect, direction: LayoutDirection) -> Self {
        let cursor = match direction {
            LayoutDirection::Horizontal => max_rect.origin.x,
            LayoutDirection::Vertical => max_rect.origin.y,
        };
        let min_rect = Rect::new(max_rect.origin.x, max_rect.origin.y, 0.0, 0.0);
        Self {
            max_rect,
            cursor,
            min_rect,
            direction,
            spacing: 0.0,
        }
    }

    /// 设 `spacing`（builder 风格）。每次 `allocate` 后主轴追加该距离。
    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// 从 `cursor` 切出 `size` 大小的矩形，推进主轴 `size + spacing`。
    /// 次轴锚定 `max_rect` 相应边（Horizontal → top；Vertical → left）；次轴长度取
    /// `size` 的相应分量（可小于 `max_rect`，如按钮不必占满行高）。
    pub fn allocate(&mut self, size: Size) -> Rect {
        let rect = match self.direction {
            LayoutDirection::Horizontal => Rect::new(
                self.cursor,
                self.max_rect.origin.y,
                size.width,
                size.height,
            ),
            LayoutDirection::Vertical => Rect::new(
                self.max_rect.origin.x,
                self.cursor,
                size.width,
                size.height,
            ),
        };
        let advance = match self.direction {
            LayoutDirection::Horizontal => size.width,
            LayoutDirection::Vertical => size.height,
        };
        self.cursor += advance + self.spacing;
        // 内容真实结束（不含尾 spacing）
        let end = self.cursor - self.spacing;
        self.min_rect = match self.direction {
            LayoutDirection::Horizontal => Rect::new(
                self.min_rect.origin.x,
                self.min_rect.origin.y,
                (end - self.min_rect.origin.x).max(0.0),
                self.min_rect.size.height.max(size.height),
            ),
            LayoutDirection::Vertical => Rect::new(
                self.min_rect.origin.x,
                self.min_rect.origin.y,
                self.min_rect.size.width.max(size.width),
                (end - self.min_rect.origin.y).max(0.0),
            ),
        };
        rect
    }

    /// 主轴剩余空间（cursor 到 max_rect 相应边）。
    pub fn available_main(&self) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => (self.max_rect.right() - self.cursor).max(0.0),
            LayoutDirection::Vertical => (self.max_rect.bottom() - self.cursor).max(0.0),
        }
    }

    /// 次轴长度（Horizontal → max_rect.height；Vertical → max_rect.width）。
    pub fn available_cross(&self) -> f32 {
        match self.direction {
            LayoutDirection::Horizontal => self.max_rect.size.height,
            LayoutDirection::Vertical => self.max_rect.size.width,
        }
    }

    /// 剩余可用矩形（当前 cursor 之后的所有空间）。
    fn remaining_rect(&self) -> Rect {
        match self.direction {
            LayoutDirection::Horizontal => Rect::new(
                self.cursor,
                self.max_rect.origin.y,
                (self.max_rect.right() - self.cursor).max(0.0),
                self.max_rect.size.height,
            ),
            LayoutDirection::Vertical => Rect::new(
                self.max_rect.origin.x,
                self.cursor,
                self.max_rect.size.width,
                (self.max_rect.bottom() - self.cursor).max(0.0),
            ),
        }
    }

    /// 在剩余空间开一个 Horizontal 子 Ui，闭包内布局；返回 `(消耗的 min_rect, 闭包结果)`。
    /// 闭包结束后按子 `min_rect` 大小 `allocate` 推进父 cursor。
    pub fn horizontal<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> (Rect, R) {
        let child_rect = self.remaining_rect();
        let mut child = Ui::new(child_rect, LayoutDirection::Horizontal);
        let r = f(&mut child);
        let consumed = child.min_rect;
        self.allocate(Size::new(consumed.size.width, consumed.size.height));
        (consumed, r)
    }

    /// 在剩余空间开一个 Vertical 子 Ui。同 [`Self::horizontal`]。
    pub fn vertical<R>(&mut self, f: impl FnOnce(&mut Ui) -> R) -> (Rect, R) {
        let child_rect = self.remaining_rect();
        let mut child = Ui::new(child_rect, LayoutDirection::Vertical);
        let r = f(&mut child);
        let consumed = child.min_rect;
        self.allocate(Size::new(consumed.size.width, consumed.size.height));
        (consumed, r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn max() -> Rect {
        Rect::new(10.0, 20.0, 400.0, 300.0)
    }

    #[test]
    fn allocate_horizontal_advances_x() {
        let mut ui = Ui::new(max(), LayoutDirection::Horizontal);
        let a = ui.allocate(Size::new(100.0, 40.0));
        assert_eq!(a, Rect::new(10.0, 20.0, 100.0, 40.0));
        let b = ui.allocate(Size::new(60.0, 30.0));
        assert_eq!(b, Rect::new(110.0, 20.0, 60.0, 30.0), "x 推进到 10+100");
        assert_eq!(ui.cursor, 170.0, "cursor = 10 + 100 + 60");
    }

    #[test]
    fn allocate_vertical_advances_y() {
        let mut ui = Ui::new(max(), LayoutDirection::Vertical);
        let a = ui.allocate(Size::new(200.0, 40.0));
        assert_eq!(a.origin.y, 20.0);
        let b = ui.allocate(Size::new(200.0, 30.0));
        assert_eq!(b.origin.y, 60.0, "y 推进到 20+40");
    }

    #[test]
    fn spacing_appended_between_allocations_but_not_after_last() {
        let mut ui = Ui::new(max(), LayoutDirection::Horizontal).with_spacing(8.0);
        ui.allocate(Size::new(50.0, 10.0));
        let b = ui.allocate(Size::new(50.0, 10.0));
        assert_eq!(b.origin.x, 68.0, "10 + 50 + 8 spacing");
        // min_rect 主轴长不含尾 spacing
        assert_eq!(ui.min_rect.size.width, 108.0, "内容边界不算末尾 spacing");
    }

    #[test]
    fn min_rect_tracks_content_bounding_box() {
        let mut ui = Ui::new(max(), LayoutDirection::Horizontal);
        ui.allocate(Size::new(40.0, 60.0));
        ui.allocate(Size::new(40.0, 30.0));
        assert_eq!(ui.min_rect.size, Size::new(80.0, 60.0), "次轴取 max");
    }

    #[test]
    fn available_main_shrinks() {
        let mut ui = Ui::new(max(), LayoutDirection::Horizontal);
        assert_eq!(ui.available_main(), 400.0);
        ui.allocate(Size::new(150.0, 20.0));
        assert_eq!(ui.available_main(), 250.0);
    }

    #[test]
    fn horizontal_scope_advances_parent_by_child_min_rect() {
        let mut ui = Ui::new(max(), LayoutDirection::Vertical);
        ui.allocate(Size::new(0.0, 10.0)); // 占 10px 高
        let (consumed, _) = ui.horizontal(|row| {
            row.allocate(Size::new(80.0, 32.0));
            row.allocate(Size::new(80.0, 32.0));
        });
        assert_eq!(consumed.size, Size::new(160.0, 32.0));
        // 父 cursor：起点 20 (max.origin.y) + 10 (init allocate) + 32（子 min_rect 高） = 62
        assert_eq!(ui.cursor, 62.0);
    }

    #[test]
    fn empty_ui_has_zero_min_rect() {
        let ui = Ui::new(max(), LayoutDirection::Horizontal);
        assert_eq!(ui.min_rect.size, Size::new(0.0, 0.0));
    }
}
