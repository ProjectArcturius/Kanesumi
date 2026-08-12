use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Size};

use crate::role::EtherRole;

/// IME 上下文 / 内容提示 —— 定义于控件库（依赖方向 core ← controls ← harness）。
pub use kanesumi_controls::{ImeContentHint, ImeContext};

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
/// 非 Copy（IME 变体含 `String`）；App 消费时按值 move。
#[derive(Debug, Clone, PartialEq)]
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
    /// IME 组合态更新。`cursor_byte` 为组合态内光标字节偏移（None = 光标在尾部，
    /// 对应协议 cursor_begin/end = -1 的隐藏光标）。文本为空 = 清除组合态。
    Preedit {
        text: String,
        cursor_byte: Option<usize>,
    },
    /// IME 提交 —— 以提交文本替换光标处组合态（原子编辑）。
    Commit { text: String },
    /// IME 周边删除 —— 删光标前/后字节数（UTF-8，控件层外扩夹紧）。
    DeleteSurrounding { before_bytes: u32, after_bytes: u32 },
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

    /// IME 焦点上下文。返回 `Some(ImeContext)` = 当前有文本输入焦点（TextBox/PasswordBox 等），
    /// 外壳据此 enable text-input 并灌周边文本/光标矩形；`None` = 无 IME（默认）。
    fn ime_focus(&self) -> Option<ImeContext> {
        None
    }

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

/// IME 使能动作 —— `compute_ime_action` 的产物（幂等 reconcile 的决策核心，
/// 参 IME_WIRING_PLAN 阶段 D）。纯逻辑，无 Wayland 依赖，跨平台可测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeAction {
    /// 表面持键盘焦点 + App 有文本输入焦点 → enable + 灌上下文。
    Enable,
    /// 任一焦点缺失 → disable。
    Disable,
}

/// 幂等决策：根据当前期望（`focus_surface && focus_control`）与已发送状态比较，
/// 返回需要执行的动作。**只在状态翻转时返回动作**（不翻转 = None，无协议流量）。
pub fn compute_ime_action(
    focus_surface: bool,
    focus_control: bool,
    currently_enabled: bool,
) -> Option<ImeAction> {
    let want = focus_surface && focus_control;
    if want == currently_enabled {
        None
    } else if want {
        Some(ImeAction::Enable)
    } else {
        Some(ImeAction::Disable)
    }
}

/// 一帧待应用 IME 事件批 —— done 事件到达前累积的 pending 状态。
/// 协议批次 = 一帧内 Preedit/Commit/DeleteSurrounding 的积压，`done` 触发应用。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingImeBatch {
    pub preedit: Option<String>,
    pub cursor_begin: i32,
    pub cursor_end: i32,
    pub commit: Option<String>,
    pub delete_before: u32,
    pub delete_after: u32,
}

impl PendingImeBatch {
    /// 按协议 done 序列展开为 InputEvent 流：`DeleteSurrounding → Commit → Preedit`
    /// （参 text-input-unstable-v3 done 事件说明）。空字段不产生事件；
    /// 空 preedit 产生 `Preedit { text: "", … }`（= 清除组合态）。
    pub fn apply(&self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        if self.delete_before > 0 || self.delete_after > 0 {
            out.push(InputEvent::DeleteSurrounding {
                before_bytes: self.delete_before,
                after_bytes: self.delete_after,
            });
        }
        if let Some(text) = &self.commit
            && !text.is_empty()
        {
            out.push(InputEvent::Commit { text: text.clone() });
        }
        if let Some(text) = &self.preedit {
            // cursor_begin = -1（隐藏光标）→ None；否则取 begin 字节。
            let cursor_byte = if self.cursor_begin >= 0 {
                Some(self.cursor_begin as usize)
            } else {
                None
            };
            out.push(InputEvent::Preedit {
                text: text.clone(),
                cursor_byte,
            });
        }
        out
    }

    /// done 应用：`serial == current_serial`（非 stale 帧）才返回事件流，否则 None。
    pub fn apply_done(&self, serial: u32, current_serial: u32) -> Option<Vec<InputEvent>> {
        if serial != current_serial {
            return None;
        }
        Some(self.apply())
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

    #[test]
    fn compute_ime_action_only_flips_on_change() {
        // 期望 = focus_surface && focus_control
        assert_eq!(
            compute_ime_action(true, true, false),
            Some(ImeAction::Enable),
            "两焦点齐 → enable"
        );
        assert_eq!(
            compute_ime_action(true, true, true),
            None,
            "已 enable 且仍期望 → 幂等无动作"
        );
        assert_eq!(
            compute_ime_action(true, false, true),
            Some(ImeAction::Disable),
            "控件失焦 → disable"
        );
        assert_eq!(
            compute_ime_action(false, true, true),
            Some(ImeAction::Disable),
            "表面离开 → disable"
        );
        assert_eq!(
            compute_ime_action(false, false, false),
            None,
            "已 disable → 幂等无动作"
        );
    }

    #[test]
    fn pending_batch_applies_in_protocol_order() {
        let b = PendingImeBatch {
            preedit: Some("nǐ".into()),
            cursor_begin: 3,
            cursor_end: 3,
            commit: Some("你好".into()),
            delete_before: 3,
            delete_after: 6,
            ..Default::default()
        };
        let events = b.apply();
        assert!(matches!(
            events[0],
            InputEvent::DeleteSurrounding {
                before_bytes: 3,
                after_bytes: 6
            }
        ));
        assert!(matches!(&events[1], InputEvent::Commit { text } if text == "你好"));
        assert!(
            matches!(&events[2], InputEvent::Preedit { text, cursor_byte } if text == "nǐ" && *cursor_byte == Some(3))
        );
    }

    #[test]
    fn pending_batch_stale_serial_is_dropped() {
        let b = PendingImeBatch {
            commit: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(b.apply_done(1, 1).unwrap().len(), 1, "匹配 serial 生效");
        assert_eq!(b.apply_done(0, 1), None, "stale serial 丢弃");
    }

    #[test]
    fn pending_batch_empty_fields_produce_no_events() {
        let b = PendingImeBatch::default();
        assert!(b.apply().is_empty());
    }

    #[test]
    fn pending_batch_cursor_neg_one_is_hidden() {
        let b = PendingImeBatch {
            preedit: Some("ab".into()),
            cursor_begin: -1,
            cursor_end: -1,
            ..Default::default()
        };
        let events = b.apply();
        assert!(matches!(
            &events[0],
            InputEvent::Preedit {
                cursor_byte: None,
                ..
            }
        ));
    }

    #[test]
    fn pending_batch_empty_preedit_clears() {
        let b = PendingImeBatch {
            preedit: Some(String::new()),
            ..Default::default()
        };
        let events = b.apply();
        assert!(matches!(&events[0], InputEvent::Preedit { text, .. } if text.is_empty()));
    }
}
