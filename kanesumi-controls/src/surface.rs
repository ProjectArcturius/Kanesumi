use kanesumi_canvas::Scene;
use kanesumi_core::{Color, CornerRadius, MetroTheme, Rect};

/// 控件形态 tokens —— 参 PLAN.md §4-5（Metro 形态：直角/极轻微圆角、无渐变纯色、内容优先）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlShape {
    pub corner_radius: CornerRadius,
    pub background: Color,
}

impl ControlShape {
    pub const fn new(corner_radius: CornerRadius, background: Color) -> Self {
        Self {
            corner_radius,
            background,
        }
    }
}

impl From<MetroTheme> for ControlShape {
    fn from(theme: MetroTheme) -> Self {
        Self {
            corner_radius: theme.tokens.corner_radius,
            background: theme.colors.surface,
        }
    }
}

/// MetroSurface —— 面板基底。可叠加交互 tint（hover/press）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroSurface {
    pub shape: ControlShape,
    /// 交互叠加 tint（悬停/按下）。无则不叠加。
    pub tint: Option<Color>,
}

impl MetroSurface {
    pub const fn new(shape: ControlShape) -> Self {
        Self { shape, tint: None }
    }

    pub const fn with_tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    /// 渲染到 `rect`。顺序：底色 → tint。使用 `shape.corner_radius`（参 V12：
    /// 原本 `fill_rect` 忽略 corner_radius，Slight/Capsule 面板全被拍直角）。
    pub fn render(&self, rect: Rect, scene: &mut Scene) {
        scene.fill_rounded_rect(self.shape.background, rect, self.shape.corner_radius);
        if let Some(tint) = self.tint {
            scene.fill_rounded_rect(tint, rect, self.shape.corner_radius);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    #[test]
    fn renders_background() {
        let theme = MetroTheme::ether_dark();
        let surface = MetroSurface::new(ControlShape::from(theme));
        let mut scene = Scene::default();
        surface.render(Rect::new(0.0, 0.0, 100.0, 50.0), &mut scene);
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], SceneCommand::FillRect { .. }));
    }

    #[test]
    fn tint_adds_overlay() {
        let theme = MetroTheme::ether_dark();
        let surface =
            MetroSurface::new(ControlShape::from(theme)).with_tint(theme.indication.press_tint);
        let mut scene = Scene::default();
        surface.render(Rect::new(0.0, 0.0, 100.0, 50.0), &mut scene);
        assert_eq!(scene.commands.len(), 2);
    }
}
