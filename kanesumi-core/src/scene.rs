use crate::color::Color;
use crate::geometry::{Point, Rect};
use crate::typography::TextStyle;

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

    pub fn fill_rounded_rect(&mut self, color: Color, rect: Rect, corner_radius: f32) {
        self.commands.push(SceneCommand::FillRect {
            color,
            rect,
            corner_radius,
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
        corner_radius: f32,
    ) {
        self.commands.push(SceneCommand::StrokeRect {
            color,
            rect,
            thickness,
            corner_radius,
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
        self.commands.push(SceneCommand::Text {
            content,
            rect,
            color,
            style,
            align,
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

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
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
            TextStyle::new(15.0, 22.0, crate::FontWeight::Normal),
            TextAlign::Left,
        );
        assert_eq!(scene.commands.len(), 2);
    }

    #[test]
    fn rounded_and_arc_append() {
        let mut scene = Scene::default();
        scene.fill_rounded_rect(Color::BLACK, Rect::new(0.0, 0.0, 40.0, 20.0), 10.0);
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
}
