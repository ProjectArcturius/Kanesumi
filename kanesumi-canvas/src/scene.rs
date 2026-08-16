use kanesumi_core::color::Color;
use kanesumi_core::geometry::{CornerRadius, Point, Rect};
use kanesumi_core::typography::TextStyle;

use crate::text::TextOverflow;

/// 文本对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
}

/// 场景绘制命令 —— 状态驱动渲染的产物（参 PLAN.md §4-1）。
/// 外壳（Linux Wayland+wgpu）按序光栅化：后画的命令叠在前者之上（painter's algorithm）。
#[derive(Debug, Clone, PartialEq)]
pub enum SceneCommand {
    /// 纯色填充矩形。参 SD §II —— 纯色、无渐变。`corner_radius` 为圆角（0 = 直角）。
    FillRect {
        color: Color,
        rect: Rect,
        corner_radius: f32,
    },
    /// 矩形描边（边框 / 焦点环）。
    StrokeRect {
        color: Color,
        rect: Rect,
        thickness: f32,
        corner_radius: f32,
    },
    /// 文本：内容 + 文本框 + 样式。排版（换行）由 `TextEngine::layout` 统一，外壳据此光栅化。
    Text {
        content: String,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
        wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    },
    /// 圆弧（ProgressRing）。角度为度数，0° = 正上，顺时针；`start_deg != end_deg` 为弧。
    Arc {
        center: Point,
        radius: f32,
        thickness: f32,
        color: Color,
        start_deg: f32,
        end_deg: f32,
    },
    /// 压入裁剪矩形。嵌套裁剪取交集，空交集保持为空，不得退化成“无裁剪”。
    PushClip { rect: Rect },
    /// 弹出一层裁剪。与 `PushClip` 必须成对。
    PopClip,
    /// 位图（SVG 光栅化的图标等）。`rgba` 为直通 RGBA（非预乘），`width`/`height` 为像素；
    /// `rect` 为绘制目标（逻辑坐标）；`tint` 为染色（None = 原色，Some = 用指定色替换非透明像素）。
    Image {
        rgba: Vec<u8>,
        width: u32,
        height: u32,
        rect: Rect,
        tint: Option<Color>,
    },
    /// 填充三角形 —— Metro 自绘几何 glyph 的最小原语（chevron/箭头/收合指示等）。
    /// 参 docs/VISUAL_ISSUES.md V7：Kanesumi 不假设 Fluent/Segoe MDL2 字体存在，
    /// 常用几何 glyph（▼/▶/×/+）由 canvas 侧自绘，不占 codepoint。
    Triangle {
        p0: Point,
        p1: Point,
        p2: Point,
        color: Color,
    },
}

/// 一帧场景 —— App / 控件的渲染产物，由 harness 外壳光栅化。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub commands: Vec<SceneCommand>,
}

impl Scene {
    pub fn fill_rect(&mut self, color: Color, rect: Rect) {
        self.commands.push(SceneCommand::FillRect {
            color,
            rect,
            corner_radius: 0.0,
        });
    }

    pub fn fill_rounded_rect(&mut self, color: Color, rect: Rect, corner: CornerRadius) {
        self.commands.push(SceneCommand::FillRect {
            color,
            rect,
            corner_radius: corner.px(rect.size),
        });
    }

    pub fn stroke_rect(&mut self, color: Color, rect: Rect, thickness: f32) {
        self.commands.push(SceneCommand::StrokeRect {
            color,
            rect,
            thickness,
            corner_radius: 0.0,
        });
    }

    pub fn stroke_rounded_rect(
        &mut self,
        color: Color,
        rect: Rect,
        thickness: f32,
        corner: CornerRadius,
    ) {
        self.commands.push(SceneCommand::StrokeRect {
            color,
            rect,
            thickness,
            corner_radius: corner.px(rect.size),
        });
    }

    pub fn text(
        &mut self,
        content: String,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
    ) {
        self.text_with_options(
            content,
            rect,
            color,
            style,
            align,
            true,
            None,
            TextOverflow::Clip,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_with_options(
        &mut self,
        content: String,
        rect: Rect,
        color: Color,
        style: TextStyle,
        align: TextAlign,
        wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
    ) {
        self.commands.push(SceneCommand::Text {
            content,
            rect: rect.normalized(),
            color,
            style,
            align,
            wrap,
            max_lines,
            overflow,
        });
    }

    pub fn arc(
        &mut self,
        center: Point,
        radius: f32,
        thickness: f32,
        color: Color,
        start_deg: f32,
        end_deg: f32,
    ) {
        self.commands.push(SceneCommand::Arc {
            center,
            radius,
            thickness,
            color,
            start_deg,
            end_deg,
        });
    }

    /// 绘制位图（图标等）。`icon` 为已光栅化的直通 RGBA；`rect` 为逻辑目标矩形；
    /// `tint` 为染色（None = 原色）。
    pub fn image(&mut self, icon: &super::icon::Icon, rect: Rect, tint: Option<Color>) {
        self.commands.push(SceneCommand::Image {
            rgba: icon.rgba.clone(),
            width: icon.width,
            height: icon.height,
            rect,
            tint,
        });
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.commands.push(SceneCommand::PushClip {
            rect: rect.normalized(),
        });
    }

    pub fn pop_clip(&mut self) {
        self.commands.push(SceneCommand::PopClip);
    }

    /// 填充三角形。三点按任意顺序（外壳不做正面/背面区分）。
    pub fn triangle(&mut self, p0: Point, p1: Point, p2: Point, color: Color) {
        self.commands
            .push(SceneCommand::Triangle { p0, p1, p2, color });
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    /// 平移本场景所有命令（面板本地坐标 → 主表面坐标）。子面板（会话菜单/控制面板/
    /// 日历）在本地 (0,0) 布局，合并到主表面时整体平移 offset。参 kanesumi_topbar。
    pub fn translate(&mut self, offset: Point) {
        for cmd in &mut self.commands {
            match cmd {
                SceneCommand::FillRect { rect, .. }
                | SceneCommand::StrokeRect { rect, .. }
                | SceneCommand::Text { rect, .. }
                | SceneCommand::PushClip { rect }
                | SceneCommand::Image { rect, .. } => {
                    rect.origin = Point::new(rect.origin.x + offset.x, rect.origin.y + offset.y);
                }
                SceneCommand::Arc { center, .. } => {
                    center.x += offset.x;
                    center.y += offset.y;
                }
                SceneCommand::Triangle { p0, p1, p2, .. } => {
                    p0.x += offset.x;
                    p0.y += offset.y;
                    p1.x += offset.x;
                    p1.y += offset.y;
                    p2.x += offset.x;
                    p2.y += offset.y;
                }
                SceneCommand::PopClip => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_by_default() {
        assert!(Scene::default().is_empty());
    }

    #[test]
    fn fill_rect_appends() {
        let mut scene = Scene::default();
        scene.fill_rect(Color::BLACK, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], SceneCommand::FillRect { .. }));
    }

    #[test]
    fn stroke_and_text_append() {
        let mut scene = Scene::default();
        scene.stroke_rect(Color::WHITE, Rect::new(0.0, 0.0, 8.0, 8.0), 1.0);
        scene.text(
            "Ether".into(),
            Rect::new(0.0, 0.0, 40.0, 16.0),
            Color::WHITE,
            TextStyle::new(15.0, 22.0, kanesumi_core::FontWeight::Normal),
            TextAlign::Left,
        );
        assert_eq!(scene.commands.len(), 2);
    }

    #[test]
    fn rounded_and_arc_append() {
        let mut scene = Scene::default();
        scene.fill_rounded_rect(
            Color::BLACK,
            Rect::new(0.0, 0.0, 40.0, 20.0),
            CornerRadius::Capsule,
        );
        scene.arc(Point::new(16.0, 16.0), 14.0, 4.0, Color::WHITE, 0.0, 270.0);
        assert!(matches!(
            scene.commands[0],
            SceneCommand::FillRect { corner_radius, .. } if corner_radius == 10.0
        ));
        assert!(matches!(
            scene.commands[1],
            SceneCommand::Arc { end_deg, .. } if end_deg == 270.0
        ));
    }

    #[test]
    fn corner_radius_px_resolves_spec_values() {
        let size = kanesumi_core::geometry::Size::new(40.0, 20.0);
        assert_eq!(CornerRadius::Square.px(size), 0.0);
        assert_eq!(CornerRadius::Slight.px(size), 2.0);
        assert_eq!(CornerRadius::Capsule.px(size), 10.0, "胶囊 = 短边一半");
    }

    #[test]
    fn image_appends_with_tint() {
        use super::super::icon::Icon;
        let mut scene = Scene::default();
        let icon = Icon {
            rgba: vec![0; 16],
            width: 2,
            height: 2,
        };
        scene.image(&icon, Rect::new(0.0, 0.0, 16.0, 16.0), Some(Color::WHITE));
        assert!(matches!(
            scene.commands[0],
            SceneCommand::Image {
                width: 2,
                height: 2,
                tint: Some(_),
                ..
            }
        ));
        assert_eq!(scene.commands.len(), 1);
    }

    #[test]
    fn clip_commands_are_explicit_and_paired() {
        let mut scene = Scene::default();
        scene.push_clip(Rect::new(0.0, 0.0, 10.0, 10.0));
        scene.pop_clip();
        assert!(matches!(scene.commands[0], SceneCommand::PushClip { .. }));
        assert!(matches!(scene.commands[1], SceneCommand::PopClip));
    }
}
