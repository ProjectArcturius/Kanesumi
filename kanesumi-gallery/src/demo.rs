use kanesumi_controls::{MetroButton, MetroList};
use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect, Scene, SceneCommand, Size};

/// 控件集成演示 —— 把首套控件渲染进一个 Scene（App::render 的产物形态）。
/// 验证 controls → Scene 管线端到端成立；GPU 光栅化由 harness 外壳承担。
pub fn render_demo_scene(theme: &MetroTheme, engine: &TextEngine, size: Size) -> Scene {
    let mut scene = Scene::default();

    let accent = MetroButton::accent("确定");
    accent.render(theme, engine, Rect::new(16.0, 16.0, 96.0, 38.0), &mut scene);

    let mut list = MetroList::new(
        ["Alpha", "Beta", "Gamma", "Delta"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    list.select(Some(1));
    list.render(
        theme,
        engine,
        Rect::new(16.0, 64.0, 240.0, size.height - 80.0),
        &mut scene,
    );

    scene
}

/// 命令统计 —— 供 smoke 输出。
pub fn command_summary(scene: &Scene) -> (usize, usize, usize) {
    let (mut fill, mut stroke, mut text) = (0, 0, 0);
    for c in &scene.commands {
        match c {
            SceneCommand::FillRect { .. } => fill += 1,
            SceneCommand::StrokeRect { .. } => stroke += 1,
            SceneCommand::Text { .. } => text += 1,
        }
    }
    (fill, stroke, text)
}
