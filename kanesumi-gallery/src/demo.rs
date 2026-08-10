use kanesumi_controls::{
    MetroButton, MetroIconButton, MetroList, MetroProgressBar, MetroProgressRing, MetroSwitch,
    MetroTab, MetroTabRow,
};
use kanesumi_core::text::TextEngine;
use kanesumi_core::{MetroTheme, Rect, Scene, SceneCommand, Size};

/// 控件集成演示 —— 把首套控件渲染进一个 Scene（App::render 的产物形态）。
/// 验证 controls → Scene 管线端到端成立；GPU 光栅化由 harness 外壳承担。
/// 覆盖：Button / IconButton / Switch / ProgressBar / ProgressRing / TabRow / List。
pub fn render_demo_scene(theme: &MetroTheme, engine: &TextEngine, size: Size) -> Scene {
    let mut scene = Scene::default();

    // Button（Accent）
    let accent = MetroButton::accent("确定");
    accent.render(theme, engine, Rect::new(16.0, 16.0, 96.0, 38.0), &mut scene);

    // IconButton（纯图标）
    let icon = MetroIconButton::new("\u{E72D}");
    icon.render(
        theme,
        engine,
        Rect::new(128.0, 12.0, 48.0, 48.0),
        &mut scene,
    );

    // Switch（已开，动画跑完）
    let mut sw = MetroSwitch::with_label("飞行模式");
    sw.set_checked(true);
    sw.update(1.0);
    sw.render(
        theme,
        engine,
        Rect::new(200.0, 16.0, 160.0, 40.0),
        &mut scene,
    );

    // ProgressBar（不确定，相位 0.5s）
    let mut bar = MetroProgressBar::indeterminate();
    bar.update(0.5);
    bar.render(theme, engine, Rect::new(16.0, 70.0, 200.0, 4.0), &mut scene);

    // ProgressRing（不确定）
    let mut ring = MetroProgressRing::new();
    ring.indeterminate = true;
    ring.update(0.25);
    ring.render(theme, Rect::new(240.0, 60.0, 32.0, 32.0), &mut scene);

    // TabRow
    let tabs = MetroTabRow::new(vec![
        MetroTab::new("邮件"),
        MetroTab::new("日历"),
        MetroTab::new("人脉"),
    ]);
    tabs.render(
        theme,
        engine,
        Rect::new(16.0, 110.0, 320.0, 48.0),
        &mut scene,
    );

    // List（选中行 2）
    let mut list = MetroList::new(
        ["Alpha", "Beta", "Gamma", "Delta", "Epsilon"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    list.select(Some(2));
    list.render(
        theme,
        engine,
        Rect::new(16.0, 170.0, 240.0, size.height - 180.0),
        &mut scene,
    );

    scene
}

/// 命令统计 —— 供 smoke 输出。
pub fn command_summary(scene: &Scene) -> (usize, usize, usize, usize) {
    let (mut fill, mut stroke, mut text, mut arc) = (0, 0, 0, 0);
    for c in &scene.commands {
        match c {
            SceneCommand::FillRect { .. } => fill += 1,
            SceneCommand::StrokeRect { .. } => stroke += 1,
            SceneCommand::Text { .. } => text += 1,
            SceneCommand::Arc { .. } => arc += 1,
        }
    }
    (fill, stroke, text, arc)
}
