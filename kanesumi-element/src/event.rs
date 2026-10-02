// event.rs —— 元素级路由事件。参 ELEMENT_TREE §Ⅵ。
//
// 外壳事件（harness `InputEvent`）由 `TreeHost` 翻译成本枚举，再由树路由：
// 指针类按命中目标冒泡，键盘 / 文本类按焦点冒泡。控件只看到「与自己有关的」事件，
// 不再自己做命中测试。
//
// `Key` / `Modifiers` / `PointerButton` 在此定义（本 crate 不依赖 harness）；
// harness 侧以 `pub use` 重导出同一类型，避免两套键定义（E2 接线时收敛）。

use kanesumi_core::Point;

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
    Scroll { dx: f32, dy: f32, modifiers: Modifiers },
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
