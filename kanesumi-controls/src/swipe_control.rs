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
                let bg = item_bg(item.action, colors.primary, colors.surface_variant);
                scene.fill_rect(bg, r);
                if self.hovered == Some((true, i)) {
                    scene.fill_rect(colors.on_surface.with_alpha(0.15), r);
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
                let bg = item_bg(item.action, colors.primary, colors.surface_variant);
                scene.fill_rect(bg, r);
                if self.hovered == Some((false, i)) {
                    scene.fill_rect(colors.on_surface.with_alpha(0.15), r);
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

/// 操作项底色。
fn item_bg(
    action: SwipeItemAction,
    primary: kanesumi_core::Color,
    surface_variant: kanesumi_core::Color,
) -> kanesumi_core::Color {
    match action {
        SwipeItemAction::Default => surface_variant,
        SwipeItemAction::Accent => primary,
        SwipeItemAction::Danger => kanesumi_core::Color::from_hex(0xE5_53_4A),
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
