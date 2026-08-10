use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Size};

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

/// 指针按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// 输入事件 —— 纯数据、跨平台。`x/y` 为表面本地逻辑坐标（指针进入表面后有效）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputEvent {
    /// 指针移动。
    PointerMoved { x: f32, y: f32 },
    /// 按下。
    PointerPressed {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    /// 释放。
    PointerReleased {
        x: f32,
        y: f32,
        button: PointerButton,
    },
    /// 滚轮 / 触摸板滚动。`dx`/`dy` 为逻辑像素增量；**正方向 = 表面坐标 +y（下）**，
    /// 即向下滚为正。外壳把 Wayland Axis 的 `discrete`（整格 ~50px）或 `absolute`
    /// （触摸板连续）转换为像素增量。
    Scroll { x: f32, y: f32 },
    /// 指针离开表面。
    PointerLeft,
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

    /// 字体路径。外壳据此加载 `TextEngine` 注入 `render`（排版唯一真源，SD §IX 禁止静默回退）。
    /// 默认 `None` → 外壳按 KANESUMI_TEST_FONT → 系统字体顺序查找。
    fn font_path(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// 每帧 tick。`dt` 单位为秒（外壳从 frame callback 计算，参 PLAN.md §4.2 合成器时钟）。
    fn update(&mut self, _dt: f64) {}

    /// 输入事件（指针位置为表面本地逻辑坐标）。控件命中测试由 App 负责（参 HANDOVER §2 输入层）。
    fn handle_input(&mut self, _event: InputEvent) {}

    /// 渲染一帧：把当前状态解析为绘制命令。
    /// `engine` 为外壳注入的 TextEngine（排版唯一真源），App 用它量测文本、外壳用它光栅化。
    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene;
}
