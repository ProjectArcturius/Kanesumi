// visual_state.rs —— 视觉状态过渡（VisualStateManager 的 Kanesumi 等价物）。参 ELEMENT_TREE §Ⅶ。
//
// 一个 `VisualState` = 一条 0↔1 的过渡进度（Sokuou `Progress`，动画唯一真源）。
// 控件在 `paint` 里用 `drive(on, ctx)` 设定目标并取当前进度画插值色，
// 在 `update` 里用 `tick(ctx, dt)` 推进 —— 未到稳态时自动续帧、到稳态自动停。
// 进入 / 离开可用不同时长（Ncrust KANESUMI_XAML「交互反馈」：进入 100ms / 离开 200ms）。

use kanesumi_anim::Progress;

use crate::widget::{PaintCtx, UpdateCtx};

#[derive(Debug, Clone)]
pub struct VisualState {
    progress: Progress,
    enter_secs: f64,
    exit_secs: f64,
}

impl VisualState {
    /// 进入 / 离开时长（秒）。
    pub fn new(enter_secs: f64, exit_secs: f64) -> Self {
        Self {
            progress: Progress::new(enter_secs),
            enter_secs,
            exit_secs,
        }
    }

    /// 指针反馈的默认时长：进入 100ms、离开 200ms。
    pub fn pointer() -> Self {
        Self::new(0.1, 0.2)
    }

    /// 设定目标并返回当前进度 `[0, 1]`。未到稳态时请求下一帧。
    pub fn drive(&mut self, on: bool, ctx: &mut PaintCtx) -> f32 {
        let target = if on { 1.0 } else { 0.0 };
        if self.progress.target() != target {
            self.progress.set_duration(if on {
                self.enter_secs
            } else {
                self.exit_secs
            });
            self.progress.set_target(target);
        }
        if !self.progress.is_steady() {
            ctx.request_anim_frame();
        }
        self.progress.value() as f32
    }

    /// 推进进度；有变化就重画，未到稳态就续帧。
    pub fn tick(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        if self.progress.is_steady() {
            return;
        }
        self.progress.update(dt);
        ctx.invalidate_paint();
        if !self.progress.is_steady() {
            ctx.request_anim_frame();
        }
    }

    /// 直接跳到稳态（无动画偏好 / 首帧）。
    pub fn jump(&mut self, on: bool) {
        self.progress.jump_to(if on { 1.0 } else { 0.0 });
    }

    pub fn value(&self) -> f32 {
        self.progress.value() as f32
    }
}
