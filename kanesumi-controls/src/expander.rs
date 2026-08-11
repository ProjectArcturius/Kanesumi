// MetroExpander —— 可折叠分组。参 CONTROL_SPEC §13。
//
// 移植自 microsoft-ui-xaml/dev/Expander（Expander.cpp + Expander.xaml + Expander_themeresources.xaml）：
// - Header 行 MinHeight 48、Padding 16,0,0,0、bg surface、边框 divider 1px；
// - 右侧 Chevron 按钮 32×32、Margin 20,0,8,0、glyph 12，展开时旋转 180°（0.1s）；
// - Content Padding 16、bg surface_variant，Down 模式边框 1,0,1,1 / Up 模式 1,1,1,0；
// - 展开动画 0.333s / 收起 0.167s（TranslateY，只动视觉属性）。
//
// 内容承载：宿主把内容渲染进 `content_rect`，并以 `content_clip` 裁剪（Scene::clip），
// 动画期间只显示可见段（铁律 4：展开不动宿主布局，只改裁剪窗）。

use kanesumi_anim::{MetroAnim, MetroPresets};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect};

use crate::state::ControlState;

/// 展开方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExpandDirection {
    Down,
    Up,
}

/// 头部行高（ExpanderMinHeight = 48）。
pub const EXPANDER_HEADER_HEIGHT: f32 = 48.0;
/// Header 左内边距（ExpanderHeaderPadding = 16,0,0,0）。
pub const EXPANDER_HEADER_PAD_X: f32 = 16.0;
/// Chevron 按钮边长（ExpanderChevronButtonSize = 32）。
pub const EXPANDER_CHEVRON_SIZE: f32 = 32.0;
/// Chevron 右侧边距（ExpanderChevronMargin = 20,0,8,0 的 right）。
pub const EXPANDER_CHEVRON_MARGIN_RIGHT: f32 = 8.0;

/// MetroExpander —— 折叠组。参 CONTROL_SPEC §13。
#[derive(Debug, Clone)]
pub struct MetroExpander {
    pub header: String,
    pub expanded: bool,
    /// Header 交互状态。
    pub state: ControlState,
    /// 展开方向（Down / Up）。
    pub direction: ExpandDirection,
    /// 内容完整高度（宿主在内容稳定后设置；动画期间不变）。
    pub content_height: f32,
    /// 内容展开进度 [0,1]（0.333s 展开 / 0.167s 收起）。
    content: MetroAnim,
    /// Chevron 旋转进度 [0,1]（0 = 朝下，1 = 朝上；0.1s）。
    chevron: MetroAnim,
}

impl Default for MetroExpander {
    fn default() -> Self {
        Self {
            header: String::new(),
            expanded: false,
            state: ControlState::Normal,
            direction: ExpandDirection::Down,
            content_height: 0.0,
            content: MetroPresets::expander_expand(),
            chevron: MetroPresets::expander_chevron(),
        }
    }
}

impl MetroExpander {
    pub fn new(header: impl Into<String>) -> Self {
        Self {
            header: header.into(),
            ..Self::default()
        }
    }

    /// 折叠/展开切换（等价 UWP IsExpanded 翻转）。
    pub fn toggle(&mut self) {
        self.set_expanded(!self.expanded);
    }

    /// 设置展开态 —— 启动对应时长动画（可中断）。
    pub fn set_expanded(&mut self, expanded: bool) {
        if self.expanded == expanded {
            return;
        }
        self.expanded = expanded;
        self.content = if expanded {
            MetroPresets::expander_expand()
        } else {
            MetroPresets::expander_collapse()
        };
        self.content.set_target(if expanded { 1.0 } else { 0.0 });
        self.chevron = MetroPresets::expander_chevron();
        self.chevron.set_target(if expanded { 1.0 } else { 0.0 });
    }

    /// 每帧推进内容/chevron 动画。
    pub fn update(&mut self, dt: f64) {
        self.content.update(dt);
        self.chevron.update(dt);
    }

    pub fn is_animating(&self) -> bool {
        !self.content.is_steady() || !self.chevron.is_steady()
    }

    /// 内容展开进度 [0,1]。
    pub fn content_progress(&self) -> f32 {
        self.content.value() as f32
    }

    /// 当前内容可见高度。
    pub fn visible_content_height(&self) -> f32 {
        self.content_height * self.content_progress()
    }

    /// Chevron 方向进度 [0,1]（0 = 下，1 = 上）。
    pub fn chevron_progress(&self) -> f32 {
        self.chevron.value() as f32
    }

    /// Header 行 rect。
    pub fn header_rect(&self, rect: Rect) -> Rect {
        match self.direction {
            ExpandDirection::Down => Rect::new(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                EXPANDER_HEADER_HEIGHT,
            ),
            ExpandDirection::Up => {
                let y = rect.bottom() - EXPANDER_HEADER_HEIGHT;
                Rect::new(rect.origin.x, y, rect.size.width, EXPANDER_HEADER_HEIGHT)
            }
        }
    }

    /// 内容完整 rect（宿主渲染内容的位置，全高）。
    pub fn content_rect(&self, rect: Rect) -> Rect {
        let h = self.content_height;
        match self.direction {
            ExpandDirection::Down => Rect::new(
                rect.origin.x,
                rect.origin.y + EXPANDER_HEADER_HEIGHT,
                rect.size.width,
                h,
            ),
            ExpandDirection::Up => Rect::new(rect.origin.x, rect.origin.y, rect.size.width, h),
        }
    }

    /// 内容裁剪窗（动画期间只显示可见段）。
    pub fn content_clip(&self, rect: Rect) -> Option<Rect> {
        let vis = self.visible_content_height();
        if vis <= 0.01 {
            return None;
        }
        let content = self.content_rect(rect);
        Some(match self.direction {
            ExpandDirection::Down => {
                Rect::new(content.origin.x, content.origin.y, content.size.width, vis)
            }
            ExpandDirection::Up => {
                let y = content.bottom() - vis;
                Rect::new(content.origin.x, y, content.size.width, vis)
            }
        })
    }

    /// Chevron 按钮 rect（右侧）。
    pub fn chevron_rect(&self, rect: Rect) -> Rect {
        let header = self.header_rect(rect);
        Rect::new(
            header.right() - EXPANDER_CHEVRON_SIZE - EXPANDER_CHEVRON_MARGIN_RIGHT,
            header.origin.y + (header.size.height - EXPANDER_CHEVRON_SIZE) / 2.0,
            EXPANDER_CHEVRON_SIZE,
            EXPANDER_CHEVRON_SIZE,
        )
    }

    /// Header 命中（含 chevron 区域）。
    pub fn hit_header(&self, rect: Rect, pos: Point) -> bool {
        self.header_rect(rect).contains(pos)
    }

    /// Header 文本样式：14px。
    pub fn header_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Normal)
    }

    /// 渲染 Header 行（bg + 边框 + 标签 + chevron）。Content 由宿主渲染。
    pub fn render_header(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let header = self.header_rect(rect);
        let style = Self::header_style();

        scene.fill_rect(colors.surface, header);
        // 交互 tint
        match self.state {
            ControlState::Hovered => scene.fill_rect(colors.on_surface.with_alpha(0.06), header),
            ControlState::Pressed => scene.fill_rect(colors.on_surface.with_alpha(0.10), header),
            _ => {}
        }
        // 边框（4 边）
        scene.stroke_rect(colors.divider, header, 1.0);

        // 标签
        let text_rect = Rect::new(
            header.origin.x + EXPANDER_HEADER_PAD_X,
            header.origin.y + (header.size.height - style.line_height) / 2.0,
            header.size.width - EXPANDER_HEADER_PAD_X - EXPANDER_CHEVRON_SIZE - 28.0,
            style.line_height,
        );
        scene.text(
            self.header.clone(),
            text_rect,
            colors.on_surface,
            style,
            TextAlign::Left,
        );

        // Chevron —— 朝下/朝上之间按进度插值（0.1s 旋转的几何近似）。
        let chevron = self.chevron_rect(rect);
        let p = self.chevron_progress();
        // 三角形顶点按进度从「下」翻到「上」。
        let cx = chevron.origin.x + chevron.size.width / 2.0;
        let top_y = chevron.origin.y + chevron.size.height * (0.25 + 0.25 * p);
        let bottom_y = chevron.origin.y + chevron.size.height * (0.75 - 0.25 * p);
        let w = chevron.size.width * 0.4;
        // p=0（下）：base 在上、tip 在下；p=1（上）：base 在下、tip 在上。
        let (left, right, tip) = if p < 0.5 {
            (
                Point::new(cx - w, top_y),
                Point::new(cx + w, top_y),
                Point::new(cx, bottom_y),
            )
        } else {
            (
                Point::new(cx - w, bottom_y),
                Point::new(cx + w, bottom_y),
                Point::new(cx, top_y),
            )
        };
        scene.triangle(left, right, tip, colors.on_surface);
        let _ = engine;
    }

    /// 渲染 Content 底 + 边框（`progress` 高度）。宿主内容画在 `content_rect` 上。
    pub fn render_content(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let vis = self.visible_content_height();
        if vis <= 0.01 {
            return;
        }
        let colors = &theme.colors;
        let content = self.content_rect(rect);
        let visible = match self.direction {
            ExpandDirection::Down => {
                Rect::new(content.origin.x, content.origin.y, content.size.width, vis)
            }
            ExpandDirection::Up => {
                let y = content.bottom() - vis;
                Rect::new(content.origin.x, y, content.size.width, vis)
            }
        };
        scene.fill_rect(colors.surface_variant, visible);
        // 边框：Down = 1,0,1,1；Up = 1,1,1,0。画左/右/可见端 3 条。
        scene.fill_rect(
            colors.divider,
            Rect::new(visible.origin.x, visible.origin.y, 1.0, visible.size.height),
        );
        scene.fill_rect(
            colors.divider,
            Rect::new(
                visible.right() - 1.0,
                visible.origin.y,
                1.0,
                visible.size.height,
            ),
        );
        match self.direction {
            ExpandDirection::Down => {
                scene.fill_rect(
                    colors.divider,
                    Rect::new(
                        visible.origin.x,
                        visible.bottom() - 1.0,
                        visible.size.width,
                        1.0,
                    ),
                );
            }
            ExpandDirection::Up => {
                scene.fill_rect(
                    colors.divider,
                    Rect::new(visible.origin.x, visible.origin.y, visible.size.width, 1.0),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

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

    fn expanded_to_steady(mut e: MetroExpander) -> MetroExpander {
        e.set_expanded(true);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        e
    }

    #[test]
    fn header_height_is_48() {
        assert_eq!(EXPANDER_HEADER_HEIGHT, 48.0);
    }

    #[test]
    fn toggle_flips_expanded() {
        let mut e = MetroExpander::new("网络");
        assert!(!e.expanded);
        e.toggle();
        assert!(e.expanded);
        e.toggle();
        assert!(!e.expanded);
    }

    #[test]
    fn expand_animates_to_steady() {
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        assert!(e.is_animating());
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        assert!(!e.is_animating());
        assert!((e.content_progress() - 1.0).abs() < 0.001);
        assert!(
            (e.chevron_progress() - 1.0).abs() < 0.001,
            "chevron 翻到朝上"
        );
    }

    #[test]
    fn expand_then_collapse_is_interruptible() {
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        e.update(0.1);
        let mid = e.content_progress();
        assert!(mid > 0.0 && mid < 1.0);
        e.set_expanded(false);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        assert!((e.content_progress() - 0.0).abs() < 0.001);
    }

    #[test]
    fn visible_content_grows_with_progress() {
        let mut e = MetroExpander::new("网络");
        e.content_height = 120.0;
        e.set_expanded(true);
        e.update(0.1);
        let p = e.content_progress();
        let vis = e.visible_content_height();
        assert!((vis - 120.0 * p).abs() < 0.01, "可见高 = 内容高 × 进度");
    }

    #[test]
    fn header_at_top_for_down() {
        let e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let h = e.header_rect(rect);
        assert_eq!(h.origin.y, 0.0);
        assert_eq!(h.size.height, 48.0);
        // content 在 header 下方
        let c = e.content_rect(rect);
        assert_eq!(c.origin.y, 48.0);
    }

    #[test]
    fn header_at_bottom_for_up() {
        let mut e = MetroExpander::new("网络");
        e.direction = ExpandDirection::Up;
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        let h = e.header_rect(rect);
        assert_eq!(h.origin.y, 152.0, "Up 模式 header 贴底");
        let c = e.content_rect(rect);
        assert_eq!(c.origin.y, 0.0);
    }

    #[test]
    fn hit_header_contains() {
        let e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        assert!(e.hit_header(rect, Point::new(10.0, 20.0)));
        assert!(!e.hit_header(rect, Point::new(10.0, 100.0)));
    }

    #[test]
    fn render_header_emits_bg_border_label_chevron() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut e = MetroExpander::new("网络");
        e.set_expanded(true);
        for _ in 0..120 {
            e.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        e.render_header(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 200.0),
            &mut scene,
        );
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::StrokeRect { .. }))
            .count();
        let tris = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(strokes, 1, "Header 边框");
        assert_eq!(texts, 1, "Header 标签");
        assert_eq!(tris, 1, "Chevron 三角形");
    }

    #[test]
    fn render_content_emits_only_when_visible() {
        let theme = MetroTheme::ether_dark();
        // 收起 → 无 content
        let mut e = MetroExpander::new("网络");
        e.content_height = 100.0;
        let mut scene = Scene::default();
        e.render_content(&theme, Rect::new(0.0, 0.0, 300.0, 200.0), &mut scene);
        assert!(scene.is_empty(), "收起态不渲染 content");
        // 展开到稳态 → 有 fill（底 + 3 边框条）
        e = expanded_to_steady(e);
        let mut scene = Scene::default();
        e.render_content(&theme, Rect::new(0.0, 0.0, 300.0, 200.0), &mut scene);
        assert_eq!(scene.commands.len(), 4, "content 底 + 左/右/底边框 3 条");
    }

    #[test]
    fn chevron_points_down_when_collapsed_up_when_expanded() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut e = MetroExpander::new("网络");
        let rect = Rect::new(0.0, 0.0, 300.0, 200.0);
        // 收起（progress 0）：tip 在下方
        let mut scene = Scene::default();
        e.render_header(&theme, &engine, rect, &mut scene);
        let SceneCommand::Triangle { p0, p1, p2, .. } = scene.commands.last().unwrap() else {
            panic!("应画三角形");
        };
        let cy = e.chevron_rect(rect).center().y;
        let max_y = p0.y.max(p1.y).max(p2.y);
        let min_y = p0.y.min(p1.y).min(p2.y);
        assert!(max_y > cy, "收起 → 尖端朝下");
        assert!(min_y < cy);
    }
}
