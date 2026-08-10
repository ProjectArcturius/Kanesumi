// Kanesumi（矩隅）· 页面结构
//
// 对应 UWP 的 Frame/Page + AppBar + Scaffold，状态驱动实现（无 retained 视觉树）。
// 参 Ether-main PLAN.md §4（Runtime 架构）—— 结构层 = 导航状态机 + 壳布局划分。
// App 消费 `Navigation` 决定渲染哪页，`MetroShell::layout` 提供区域矩形。

pub mod layout;
pub mod navigation;

pub use layout::{APP_BAR_HEIGHT, ShellLayout};
pub use navigation::{DURATION_PAGE_TRANSITION, Navigation};

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, MetroTheme, Rect};

/// 应用主壳 —— 主题 + 导航 + AppBar 的宿主。
/// `PageId` 为应用自定义页标识（与 `Navigation` 同型）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroShell<PageId> {
    pub theme: MetroTheme,
    pub nav: Navigation<PageId>,
    pub app_bar: MetroAppBar,
}

impl<PageId: Clone + PartialEq> MetroShell<PageId> {
    /// 以首页创建主壳（默认 Ether 深色主题 + 标准 AppBar）。
    pub fn new(initial: PageId, title: impl Into<String>) -> Self {
        Self {
            theme: MetroTheme::ether_dark(),
            nav: Navigation::new(initial),
            app_bar: MetroAppBar::new(title),
        }
    }

    /// 划分窗口为 AppBar / 内容区。
    pub fn layout(&self, window: Rect) -> ShellLayout {
        ShellLayout::of(window)
    }

    /// 渲染背景 + AppBar，返回内容区矩形。页面内容由 App 渲染进返回矩形。
    pub fn render(&self, engine: &TextEngine, window: Rect, scene: &mut Scene) -> Rect {
        scene.fill_rect(self.theme.colors.background, window);
        let layout = self.layout(window);
        self.app_bar
            .render(&self.theme, engine, layout.app_bar, scene);
        layout.content
    }
}

/// 顶部命令栏（AppBar）。标题 + 高度 + 背景；后续扩展操作按钮区。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroAppBar {
    pub background: Color,
    pub height: f32,
    pub title: String,
}

impl MetroAppBar {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            background: Color::BLACK.with_alpha(0.0),
            height: APP_BAR_HEIGHT,
            title: title.into(),
        }
    }

    /// 渲染背景 + 标题（标题用 `typography.title`，靠左，留 16px 内边距）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        if self.background.a > 0.0 {
            scene.fill_rect(self.background, rect);
        }
        let style = theme.typography.title;
        let title_rect = Rect::new(
            rect.origin.x + 16.0,
            rect.origin.y + (rect.size.height - style.line_height) / 2.0,
            rect.size.width - 32.0,
            style.line_height,
        );
        scene.text(
            self.title.clone(),
            title_rect,
            theme.colors.on_surface,
            style,
            TextAlign::Left,
        );
    }
}

/// 页面脚手架 —— 内容区容器（背景 + 内边距）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroScaffold {
    pub background: Color,
    pub padding: f32,
}

impl MetroScaffold {
    pub fn new(background: Color, padding: f32) -> Self {
        Self {
            background,
            padding,
        }
    }

    /// 渲染背景，返回扣除内边距后的内容矩形。
    pub fn render(&self, scene: &mut Scene, rect: Rect) -> Rect {
        scene.fill_rect(self.background, rect);
        Rect::new(
            rect.origin.x + self.padding,
            rect.origin.y + self.padding,
            rect.size.width - 2.0 * self.padding,
            rect.size.height - 2.0 * self.padding,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::{Point, Size};

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    #[test]
    fn shell_render_returns_content_rect() {
        let Some(p) = find_font() else { return };
        let mut scene = Scene::default();
        let engine = TextEngine::load(p).unwrap();
        let shell: MetroShell<u32> = MetroShell::new(0, "Ether");
        let window = Rect::new(0.0, 0.0, 400.0, 300.0);
        let content = shell.render(&engine, window, &mut scene);
        assert_eq!(content.origin, Point::new(0.0, APP_BAR_HEIGHT));
        assert_eq!(content.size, Size::new(400.0, 300.0 - APP_BAR_HEIGHT));
        // 背景填充 + AppBar 标题
        assert!(scene.commands.len() >= 2);
    }

    #[test]
    fn app_bar_renders_title_text() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let bar = MetroAppBar::new("Ether");
        let mut scene = Scene::default();
        bar.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 400.0, 48.0),
            &mut scene,
        );
        assert!(scene.commands.iter().any(
            |c| matches!(c, kanesumi_canvas::SceneCommand::Text { content, .. } if content == "Ether")
        ));
    }

    #[test]
    fn scaffold_pads_content() {
        let mut scene = Scene::default();
        let s = MetroScaffold::new(Color::BLACK, 16.0);
        let inner = s.render(&mut scene, Rect::new(0.0, 0.0, 200.0, 100.0));
        assert_eq!(inner.origin, Point::new(16.0, 16.0));
        assert_eq!(inner.size, Size::new(168.0, 68.0));
        assert_eq!(scene.commands.len(), 1, "背景填充");
    }
}
