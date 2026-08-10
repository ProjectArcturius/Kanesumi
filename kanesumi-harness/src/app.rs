use kanesumi_core::{MetroTheme, Scene, Size};

use crate::role::EtherRole;

/// 应用配置 —— 身份 + 启动尺寸。app_id 命名空间 `org.ether.*`（ENCS §XI）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppConfig {
    pub app_id: &'static str,
    pub title: &'static str,
    pub role: EtherRole,
    pub width: f32,
    pub height: f32,
}

impl AppConfig {
    pub const fn new(
        app_id: &'static str,
        title: &'static str,
        role: EtherRole,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            app_id,
            title,
            role,
            width,
            height,
        }
    }
}

/// Kanesumi 应用入口 trait —— 把 Kanesumi 变成应用 SDK 的契约。
///
/// 状态驱动渲染（参 PLAN.md §4-1 / AnimationRules.md §III）：
/// `state → progress → resolved spatial state → render`。
/// App 只产出 `Scene` 绘制命令，GPU 光栅化由 harness 外壳承担——保持纯逻辑、跨平台可测。
pub trait App {
    fn config(&self) -> &AppConfig;

    /// 应用主题（默认 Ether 深色空间桌面）。
    fn theme(&self) -> MetroTheme {
        MetroTheme::ether_dark()
    }

    /// 每帧 tick。`dt` 单位为秒（外壳从 frame callback 计算，参 PLAN.md §4.2 合成器时钟）。
    fn update(&mut self, _dt: f64) {}

    /// 渲染一帧：把当前状态解析为绘制命令。
    fn render(&mut self, size: Size) -> Scene;
}
