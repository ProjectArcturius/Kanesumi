// event.rs —— 元素级路由事件。参 ELEMENT_TREE §Ⅵ。
//
// 外壳事件（harness `InputEvent`）由 `TreeHost` 翻译成本枚举，再由树路由：
// 指针类按命中目标冒泡，键盘 / 文本类按焦点冒泡。控件只看到「与自己有关的」事件，
// 不再自己做命中测试。
//
// `Key` / `Modifiers` / `PointerButton` 在此定义（本 crate 不依赖 harness）；
// harness 侧以 `pub use` 重导出同一类型，避免两套键定义（E2 接线时收敛）。

use kanesumi_core::Point;
use kanesumi_core::interaction::WHEEL_STEP_PX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// 逻辑键。可打印字符 → `Char`；控制键 → 具名变体；未分类 → `Unknown`（原始 keysym）。
///
/// 具名变体是应用与控件的稳定语义面：应用不应再比 X11 keysym 原始码
/// （参 `Librarian` 的 F2 重命名 / F5 刷新 / Space 快速查看）。可打印空格仍走
/// `Char(' ')`（外壳需保留文本输入语义，TextBox 与所有「Space 激活」控件都按
/// `Char(' ')` 识别，参 `docs/ELEMENT_MIGRATION.md` §3）；`Space` 供合成事件或
/// 无 IME 组字时的语义化按键使用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Escape,
    Tab,
    /// 语义化空格。外壳对可打印空格仍投 `Char(' ')`，本变体供合成 / 特殊场景。
    Space,
    /// Insert（X11 `0xff63`）。
    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,
    Left,
    Right,
    Up,
    Down,
    /// 功能键 F1..F12；更高级 F13+ 落 `Unknown`。
    F(u8),
    Unknown(u32),
}

/// 修饰键状态（事件发生瞬间）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };
}

/// 滚动来源。参 `INTERACTION_CANON.md` §Ⅴ（滚轮离散 / 触控板连续 / 高精度连续）。
///
/// 区分来源是「跟手 + 松手惯性」的前提：滚轮走离散平滑追踪，触控板走像素精确直跟，
/// 并在 `End` 后按估出的速度启动惯性（参 `docs/ELEMENT_TREE.md` 事件一节）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScrollSource {
    /// 鼠标滚轮 / 高精度滚轮：`steps` 以「格」计（正 = 向下 / 向右）。
    /// 像素增量仍以 `dx`/`dy` 携带（Wheel 来源时 = `steps × WHEEL_STEP_PX`）。
    Wheel { steps: f32 },
    /// 触控板 / 触摸屏手指拖动：像素精确、连续、跟手。
    Finger,
    /// 其它连续源（合成器上报 `axis_source = continuous`）。
    Continuous,
}

/// 滚动阶段。`End` = 手指离开（Wayland `wl_pointer.axis_stop`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollPhase {
    /// 滚动进行中（位置增量）。
    Update,
    /// 手指离开 / 连续滚动结束 —— 宿主据此决定是否续惯性。
    End,
}

/// 元素树滚动输入（来源 + 阶段 + 像素增量）。`Tree::scroll_ex` 的入参。
///
/// 旧 `Tree::scroll(pos, dx, dy, modifiers)` 等价于
/// `Tree::scroll_ex(pos, ScrollInput::wheel(dx, dy, modifiers))`（`Wheel` / `Update`）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollInput {
    /// 水平像素增量（正 = 向右）。
    pub dx: f32,
    /// 垂直像素增量（正 = 向下）。
    pub dy: f32,
    /// 来源。
    pub source: ScrollSource,
    /// 阶段。
    pub phase: ScrollPhase,
    /// 修饰键。
    pub modifiers: Modifiers,
}

impl ScrollInput {
    /// 完整构造。
    pub fn new(
        dx: f32,
        dy: f32,
        source: ScrollSource,
        phase: ScrollPhase,
        modifiers: Modifiers,
    ) -> Self {
        Self {
            dx,
            dy,
            source,
            phase,
            modifiers,
        }
    }

    /// 滚轮离散：由像素增量反推格数（主轴优先取 `dy`，横向取 `dx`）。
    pub fn wheel(dx: f32, dy: f32, modifiers: Modifiers) -> Self {
        let px = if dy != 0.0 { dy } else { dx };
        Self::new(
            dx,
            dy,
            ScrollSource::Wheel {
                steps: px / WHEEL_STEP_PX,
            },
            ScrollPhase::Update,
            modifiers,
        )
    }

    /// 触控板 / 连续源的 `End`（手指离开），无位置增量。
    pub fn end(source: ScrollSource, modifiers: Modifiers) -> Self {
        Self::new(0.0, 0.0, source, ScrollPhase::End, modifiers)
    }
}

/// 路由事件。坐标一律为表面本地逻辑坐标（控件用自身 `ctx.rect()` 换算局部坐标）。
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// 指针按下。按下即被框架捕获给命中目标，直到释放（参 §Ⅵ「按下即捕获」）。
    PointerDown {
        pos: Point,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// 双击（XAML `DoubleTapped`）：外壳判定同按钮、短间隔的第二次按下后，在该次
    /// `PointerDown` **之后**追加投递（单击语义不丢）。投给命中目标并冒泡。
    DoubleTapped { pos: Point, button: PointerButton },
    PointerUp {
        pos: Point,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// 指针移动（捕获期间投给捕获者，否则投给命中目标）。
    PointerMove { pos: Point },
    /// 指针进入 / 离开本元素（框架比对前后悬停链合成；不冒泡）。
    PointerEnter,
    PointerLeave,
    /// 框架合成的点击：同一交互元素上 左键按下 → 在其范围内释放。
    Click,
    /// 右键请求上下文菜单。目标 = 按下瞬间的命中元素（`ContextTarget` 一等语义，ROADMAP M3-4）。
    ContextRequested { pos: Point },
    /// 滚动（逻辑像素，正 = 向下 / 向右）。首个处理者截停。
    ///
    /// `source` 区分滚轮 / 触控板 / 连续源，`phase` 区分进行中与结束（手指离开）。
    /// `dx`/`dy` 保留原语义（Wheel 来源时 = `steps × WHEEL_STEP_PX`）。
    Scroll {
        dx: f32,
        dy: f32,
        source: ScrollSource,
        phase: ScrollPhase,
        modifiers: Modifiers,
    },
    /// 键按下（焦点冒泡）。
    KeyDown { key: Key, modifiers: Modifiers },
    /// IME 组合态（焦点）。空串 = 清除。
    Preedit { text: String, cursor_byte: Option<usize> },
    /// IME 提交 / 普通文本输入（焦点）。
    Commit { text: String },
    /// IME 周边删除（焦点）。
    DeleteSurrounding { before_bytes: u32, after_bytes: u32 },
    /// 本控件打开的弹层已关闭（被选中项关闭、点外部、Esc、失焦 —— 任何原因）。
    /// 投给弹层的锚点，不冒泡。控件据此复位「展开中」外观。
    PopupClosed { popup: crate::id::WidgetId },
    /// 获得 / 失去焦点（不冒泡）。`keyboard` = 由 Tab 等键盘操作获得（决定是否画焦点视觉）。
    FocusIn { keyboard: bool },
    FocusOut,
}

impl Event {
    /// 该事件是否沿祖先链冒泡。
    pub fn bubbles(&self) -> bool {
        !matches!(
            self,
            Event::PointerEnter
                | Event::PointerLeave
                | Event::FocusIn { .. }
                | Event::FocusOut
                | Event::PopupClosed { .. }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 滚轮像素增量反推格数：48px = 1 格（正典 §Ⅴ「滚轮一格 48px」）。
    #[test]
    fn wheel_scroll_input_counts_steps() {
        let w = ScrollInput::wheel(0.0, 96.0, Modifiers::NONE);
        assert_eq!(w.source, ScrollSource::Wheel { steps: 2.0 });
        assert_eq!(w.phase, ScrollPhase::Update);
        assert_eq!(w.dy, 96.0);
        // 纵向为 0 时按横向 dx 反推。
        let h = ScrollInput::wheel(48.0, 0.0, Modifiers::NONE);
        assert_eq!(h.source, ScrollSource::Wheel { steps: 1.0 });
    }

    /// `End` 无位置增量，阶段为结束（Wayland `axis_stop` / 手指离开）。
    #[test]
    fn end_has_no_delta_and_end_phase() {
        let e = ScrollInput::end(ScrollSource::Finger, Modifiers::NONE);
        assert_eq!(e.phase, ScrollPhase::End);
        assert_eq!((e.dx, e.dy), (0.0, 0.0));
        assert_eq!(e.source, ScrollSource::Finger);
    }
}
