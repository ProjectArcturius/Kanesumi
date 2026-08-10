// 壳布局 —— 把窗口 rect 划分为 AppBar / 内容区（可选左侧导航栏）。
//
// 状态驱动布局：`MetroShell::layout(window)` 一次求出各区域矩形，控件在其内自布局。
// 对应 UWP Frame 内容区 + AppBar / NavigationView 的首次近似（无 retained 视觉树）。

use kanesumi_core::Rect;

/// AppBar 高度（Metro 常规 48px，与 TabRow 头一致，参 CONTROL_SPEC §6）。
pub const APP_BAR_HEIGHT: f32 = 48.0;

/// 壳布局结果 —— 一次划分，控件消费。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShellLayout {
    /// 顶部命令栏区域。
    pub app_bar: Rect,
    /// 内容区（AppBar 之下；若启用左导航栏则为右侧剩余）。
    pub content: Rect,
    /// 左侧导航栏区域（未启用为 `None`）。
    pub nav_rail: Option<Rect>,
}

impl ShellLayout {
    /// 把 `window` 划分为 AppBar + 内容区。
    pub fn of(window: Rect) -> Self {
        Self {
            app_bar: Rect::new(
                window.origin.x,
                window.origin.y,
                window.size.width,
                APP_BAR_HEIGHT.min(window.size.height),
            ),
            content: Rect::new(
                window.origin.x,
                window.origin.y + APP_BAR_HEIGHT.min(window.size.height),
                window.size.width,
                window.size.height - APP_BAR_HEIGHT.min(window.size.height),
            ),
            nav_rail: None,
        }
    }

    /// 在内容区左侧再切出导航栏（NavigationView 式）。`rail_width` 为逻辑像素。
    pub fn with_nav_rail(mut self, rail_width: f32) -> Self {
        let w = rail_width.min(self.content.size.width);
        self.nav_rail = Some(Rect::new(
            self.content.origin.x,
            self.content.origin.y,
            w,
            self.content.size.height,
        ));
        self.content = Rect::new(
            self.content.origin.x + w,
            self.content.origin.y,
            self.content.size.width - w,
            self.content.size.height,
        );
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::{Point, Size};

    #[test]
    fn app_bar_sits_on_top() {
        let w = Rect::new(0.0, 0.0, 400.0, 300.0);
        let l = ShellLayout::of(w);
        assert_eq!(l.app_bar.size.height, 48.0);
        assert_eq!(l.app_bar.origin.y, 0.0);
        assert_eq!(l.content.origin.y, 48.0);
        assert_eq!(l.content.size.height, 252.0);
    }

    #[test]
    fn app_bar_clamps_to_window_height() {
        let w = Rect::new(0.0, 0.0, 400.0, 20.0);
        let l = ShellLayout::of(w);
        assert_eq!(l.app_bar.size.height, 20.0);
        assert_eq!(l.content.size.height, 0.0);
    }

    #[test]
    fn nav_rail_splits_content() {
        let w = Rect::new(10.0, 0.0, 400.0, 300.0);
        let l = ShellLayout::of(w).with_nav_rail(120.0);
        let rail = l.nav_rail.unwrap();
        assert_eq!(rail.size, Size::new(120.0, 252.0));
        assert_eq!(rail.origin, Point::new(10.0, 48.0));
        assert_eq!(l.content.origin.x, 130.0);
        assert_eq!(l.content.size.width, 280.0);
    }

    #[test]
    fn nav_rail_clamps_to_content_width() {
        let w = Rect::new(0.0, 0.0, 80.0, 300.0);
        let l = ShellLayout::of(w).with_nav_rail(120.0);
        let rail = l.nav_rail.unwrap();
        assert_eq!(rail.size.width, 80.0);
        assert_eq!(l.content.size.width, 0.0);
    }
}
