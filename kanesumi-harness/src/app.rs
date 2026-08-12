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

/// 逻辑键 —— 键盘事件（wl_keyboard 经 xkbcommon 语义化）的跨平台契约。
/// 可打印字符（含 shift 符号 / 小键盘）→ `Char`；控制键 → 具名变体；未分类 → `Unknown`（原始 keysym 透传）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// 可打印字符（utf8 语义，如 `'+'`、`'%'`、`'7'`）。
    Char(char),
    Enter,
    Backspace,
    Escape,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Delete,
    /// 未分类 keysym（原始值透传，App 可自行处理）。
    Unknown(u32),
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
    /// 键按下（表面持有键盘焦点时）。释放事件不推（App 一般只关心按下）。
    KeyPressed { key: Key },
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

/// harness `Key` → 控件层 `TextInputKey` 转换（TextBox 等文本控件路由用）。
///
/// **契约：仅做键身份（identity）映射，不处理修饰键。** 修饰键状态由宿主在调用处
/// 自行维护并组合，`key_to_text_input` 完全不感知 Shift/Ctrl/Alt/Super。典型模式：
///
/// - **Shift + 方向 = 选区扩展**：宿主检测 Shift 按下，把
///   `TextField::move_left/right/…` 的 `select=true` 传下去。
/// - **Ctrl + A / Ctrl + C / Ctrl + Z / Ctrl + V**：宿主检测 Ctrl，不走本函数，
///   直接调 `TextField::select_all` / `copy` / `undo` / `insert`。
/// - **Ctrl + Home/End = 跳到首/尾字符**：同上，由宿主组合。
///
/// 键身份来自 wl_keyboard 经 xkbcommon 语义化后的 keysym，已考虑键盘布局
/// （Dvorak / QWERTY / IME latin），不受物理 scancode 影响。
///
/// `Unknown` / 无法映射的键 → `None`（宿主可不消费）。
pub fn key_to_text_input(key: Key) -> Option<kanesumi_controls::TextInputKey> {
    use kanesumi_controls::TextInputKey as K;
    match key {
        Key::Char(c) => Some(K::Char(c)),
        Key::Enter => Some(K::Enter),
        Key::Backspace => Some(K::Backspace),
        Key::Escape => Some(K::Escape),
        Key::Tab => Some(K::Tab),
        Key::Left => Some(K::Left),
        Key::Right => Some(K::Right),
        Key::Up => Some(K::Up),
        Key::Down => Some(K::Down),
        Key::Home => Some(K::Home),
        Key::End => Some(K::End),
        Key::Delete => Some(K::Delete),
        Key::Unknown(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_maps_to_text_input() {
        assert_eq!(
            key_to_text_input(Key::Char('a')),
            Some(kanesumi_controls::TextInputKey::Char('a'))
        );
        assert_eq!(
            key_to_text_input(Key::Backspace),
            Some(kanesumi_controls::TextInputKey::Backspace)
        );
        assert_eq!(
            key_to_text_input(Key::Up),
            Some(kanesumi_controls::TextInputKey::Up)
        );
        assert_eq!(key_to_text_input(Key::Unknown(0x00ff)), None);
    }
}
