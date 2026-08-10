use kanesumi_core::{Color, Rect};

/// 场景绘制命令 —— 状态驱动渲染的产物。参 PLAN.md §4-1。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SceneCommand {
    /// 纯色填充矩形。参 SD §II —— 纯色、无渐变。
    FillRect { color: Color, rect: Rect },
}

/// 一帧场景 —— App 的渲染产物，由 harness 外壳（Linux Wayland+wgpu）光栅化。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Scene {
    pub commands: Vec<SceneCommand>,
}

impl Scene {
    pub fn fill_rect(&mut self, color: Color, rect: Rect) {
        self.commands.push(SceneCommand::FillRect { color, rect });
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_rect_appends() {
        let mut scene = Scene::default();
        assert!(scene.is_empty());
        scene.fill_rect(Color::BLACK, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], SceneCommand::FillRect { .. }));
    }
}
