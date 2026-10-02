// animation.rs —— 框架统一动画入口的抽象（参 ELEMENT_TREE §帧调度）。
//
// 元素树的 `UpdateCtx::animate` / `PaintCtx::animate` 只依赖本 trait，不绑定具体原语：
// `Progress` / `MetroAnim` / `SpringAnim` 全部实现它。框架据此按**真实经过时间**推进动画，
// 并在未到稳态时自动登记动画帧 —— 控件不必再手写 `update(dt)` + `request_anim_frame()`。
//
// 「Sokuou 动画唯一真源」不变：本 trait 只是对 Sokuou 原语的转发，不引入新原语。

use sokuou::{MetroAnim, Progress, SpringAnim};

/// 可被元素树统一推进的动画原语。
pub trait Animation {
    /// 按真实经过时间 `dt`（秒）推进一步。
    fn advance(&mut self, dt: f64);
    /// 是否已到稳态（此后无需再出帧）。
    fn is_steady(&self) -> bool;
    /// 当前标量值。
    fn value(&self) -> f64;
}

impl Animation for Progress {
    fn advance(&mut self, dt: f64) {
        Progress::update(self, dt);
    }
    fn is_steady(&self) -> bool {
        Progress::is_steady(self)
    }
    fn value(&self) -> f64 {
        Progress::value(self)
    }
}

impl Animation for MetroAnim {
    fn advance(&mut self, dt: f64) {
        MetroAnim::update(self, dt);
    }
    fn is_steady(&self) -> bool {
        MetroAnim::is_steady(self)
    }
    fn value(&self) -> f64 {
        MetroAnim::value(self)
    }
}

impl Animation for SpringAnim {
    fn advance(&mut self, dt: f64) {
        SpringAnim::update(self, dt);
    }
    fn is_steady(&self) -> bool {
        SpringAnim::is_steady(self)
    }
    fn value(&self) -> f64 {
        SpringAnim::value(self)
    }
}
