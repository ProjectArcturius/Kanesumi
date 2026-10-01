// app.rs —— 计算器应用（元素树 TreeApp）。参 docs/ELEMENT_TREE.md §Ⅷ、Ether SYSTEM_APPS_PLAN §Ⅱ 顺序 1。
//
// 2026-10-01 由「旧 App（手写几何 + 手写命中）」移植到元素树，作为后续 TopBar / Settings /
// Librarian 移植的模板：布局由 Border / Stack / Label 与框架属性（LayoutProps）表达，按键交互
// 由 MetroButton 的动作驱动，命中 / 悬停 / 焦点 / Tab 顺序全部交给框架 ——
// 本文件不再有一个手写坐标、一个命中函数、一套 hovered / pressed 状态。
//
// 旧文件头「观察到的 runtime 缺口」两条在移植后均不成立，改写为移植结论：
// 1. 无 UniformGrid 布局原语 —— 等分网格改由 Stack 子节点的 grow 权重表达（0 键 grow 2.0
//    跨两列）；行高同理，窗口缩放时按键随之伸缩；
// 2. harness 无键盘输入 —— TreeApp::on_key 提供应用级快捷键（参 tree_host.rs），无焦点时
//    数字 / 运算符 / Enter / Backspace / Escape 直达计算状态机。

use kanesumi_canvas::TextAlign;
use kanesumi_controls::{ButtonClicked, ButtonKind, MetroButton};
use kanesumi_core::{FontWeight, MetroTheme, TextStyle};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{
    Action, Align, Insets, Key, LayoutProps, Modifiers, Tree, WidgetId,
};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

use crate::calc::{Calc, Op};

// ── 布局常量（逻辑像素）。参 KANESUMI_DESIGN §Ⅱ 贴边几何。 ──────────────────────────

const PAD: f32 = 8.0;
const GAP: f32 = 8.0;
/// 显示区槽位高度（数字贴底右对齐）。
const DISPLAY_H: f32 = 116.0;

// ── 键位 ────────────────────────────────────────────────────────────────────────────

/// 键身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyId {
    Clear,
    Sign,
    Percent,
    Div,
    D7,
    D8,
    D9,
    Mul,
    D4,
    D5,
    D6,
    Sub,
    D1,
    D2,
    D3,
    Add,
    D0,
    Dot,
    Eq,
}

impl KeyId {
    /// 数字键 0~9 的身份表（`on_key` 把字符映射到键）。
    const DIGITS: [KeyId; 10] = [
        KeyId::D0,
        KeyId::D1,
        KeyId::D2,
        KeyId::D3,
        KeyId::D4,
        KeyId::D5,
        KeyId::D6,
        KeyId::D7,
        KeyId::D8,
        KeyId::D9,
    ];

    fn label(&self) -> &'static str {
        match self {
            KeyId::Clear => "C",
            KeyId::Sign => "±",
            KeyId::Percent => "%",
            KeyId::Div => "÷",
            KeyId::D7 => "7",
            KeyId::D8 => "8",
            KeyId::D9 => "9",
            KeyId::Mul => "×",
            KeyId::D4 => "4",
            KeyId::D5 => "5",
            KeyId::D6 => "6",
            KeyId::Sub => "−",
            KeyId::D1 => "1",
            KeyId::D2 => "2",
            KeyId::D3 => "3",
            KeyId::Add => "+",
            KeyId::D0 => "0",
            KeyId::Dot => ".",
            KeyId::Eq => "=",
        }
    }

    /// 强调色键（沿用旧实现：仅等号）。参 CONTROL_SPEC §1 按钮种类。
    fn kind(&self) -> ButtonKind {
        match self {
            KeyId::Eq => ButtonKind::Accent,
            _ => ButtonKind::Standard,
        }
    }

    /// 作用到计算状态机。
    fn apply(&self, calc: &mut Calc) {
        match self {
            KeyId::Clear => calc.clear(),
            KeyId::Sign => calc.toggle_sign(),
            KeyId::Percent => calc.percent(),
            KeyId::Div => calc.apply_op(Op::Div),
            KeyId::D7 => calc.input_digit(7),
            KeyId::D8 => calc.input_digit(8),
            KeyId::D9 => calc.input_digit(9),
            KeyId::Mul => calc.apply_op(Op::Mul),
            KeyId::D4 => calc.input_digit(4),
            KeyId::D5 => calc.input_digit(5),
            KeyId::D6 => calc.input_digit(6),
            KeyId::Sub => calc.apply_op(Op::Sub),
            KeyId::D1 => calc.input_digit(1),
            KeyId::D2 => calc.input_digit(2),
            KeyId::D3 => calc.input_digit(3),
            KeyId::Add => calc.apply_op(Op::Add),
            KeyId::D0 => calc.input_digit(0),
            KeyId::Dot => calc.input_decimal(),
            KeyId::Eq => calc.equals(),
        }
    }
}

/// 键位表（行序，每 4 个一行）。`0` 在第 5 行首、跨 2 列。
const KEYS: [KeyId; 19] = [
    KeyId::Clear,
    KeyId::Sign,
    KeyId::Percent,
    KeyId::Div,
    KeyId::D7,
    KeyId::D8,
    KeyId::D9,
    KeyId::Mul,
    KeyId::D4,
    KeyId::D5,
    KeyId::D6,
    KeyId::Sub,
    KeyId::D1,
    KeyId::D2,
    KeyId::D3,
    KeyId::Add,
    KeyId::D0,
    KeyId::Dot,
    KeyId::Eq,
];

// ── 应用状态 ────────────────────────────────────────────────────────────────────────

/// 计算器应用（元素树）。
pub struct CalculatorApp {
    theme: MetroTheme,
    config: AppConfig,
    calc: Calc,
    /// 显示文本节点（每帧由计算状态刷新）。
    display: Option<WidgetId>,
    /// 键身份 → 节点（`on_action` 按来源 id 反查键）。
    keys: Vec<(KeyId, WidgetId)>,
}

impl Default for CalculatorApp {
    fn default() -> Self {
        Self::new()
    }
}

impl CalculatorApp {
    pub fn new() -> Self {
        Self::with_theme(MetroTheme::ether_dark())
    }

    pub fn with_theme(theme: MetroTheme) -> Self {
        Self {
            theme,
            config: AppConfig::new(
                "org.ether.calculator",
                "计算器",
                EtherRole::Browser,
                320.0,
                // 4 列 × 5 行方形单元(70) + 4 间隔 + 显示区 + 外边距：8+116+8+382+8 = 522。
                522.0,
            ),
            calc: Calc::new(),
            display: None,
            keys: Vec::new(),
        }
    }

    /// 显示文本样式：按长度自适应字号，避免溢出显示区。
    fn display_style(len: usize) -> TextStyle {
        let size = match len {
            0..=7 => 46.0,
            8..=10 => 36.0,
            11..=13 => 28.0,
            _ => 22.0,
        };
        TextStyle::new(size, size * 1.25, FontWeight::Semilight)
    }

    /// 把计算状态写回显示节点的文本与字号（元素树：`edit` 是改控件的唯一入口）。
    fn refresh_display(&self, tree: &mut Tree) {
        let Some(id) = self.display else { return };
        let text = self.calc.display().to_string();
        let style = Self::display_style(text.len());
        tree.edit::<Label, _>(id, |l, _| {
            l.text = text;
            l.style = Some(style);
        });
    }
}

// ── 元素树接线 ──────────────────────────────────────────────────────────────────────

impl TreeApp for CalculatorApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    /// 建树：`Border`（背景）> `Stack::column`：显示区 + 5 行按键。
    ///
    /// 等分全靠 `LayoutProps::grow`，不再手算 `UniformGrid`：行间 `grow` 等分剩余高度，
    /// 行内按键 `grow` 等分宽度；窗口缩放时两者随之伸缩。0 键 `grow: 2.0` 跨两列 ——
    /// 与「两列宽 + 中间一个 gap」相差约一个 gap（列间距不再计入跨键宽度），
    /// 这一 gap 级误差是可接受的近似（参 SYSTEM_APPS_PLAN 的移植说明）。
    fn build(&mut self, tree: &mut Tree) {
        self.keys.clear();

        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(kanesumi_core::ThemeColor::Background)
                .padding(Insets::all(PAD)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(GAP));

        // 显示区：116px 高槽位；Label 自身一行高，靠 v_align 贴底、TextAlign 右对齐 ——
        // 与旧实现「底部大号右对齐结果行」一致（字号由 display_style 按长度自适应）。
        let display_area = tree.insert_with(
            col,
            Border::new(),
            LayoutProps {
                height: Some(DISPLAY_H),
                ..LayoutProps::default()
            },
        );
        self.display = Some(tree.insert_with(
            display_area,
            Label::new("0").align(TextAlign::Right),
            LayoutProps {
                v_align: Align::End,
                ..LayoutProps::default()
            },
        ));

        for row in KEYS.chunks(4) {
            let row_id = tree.insert_with(
                col,
                Stack::row().with_spacing(GAP),
                LayoutProps {
                    grow: 1.0,
                    ..LayoutProps::default()
                },
            );
            for &key in row {
                // 0 键跨两列：grow 2.0（间距造成的约一个 gap 误差可接受）。
                let grow = if key == KeyId::D0 { 2.0 } else { 1.0 };
                let mut btn = MetroButton::new(key.label());
                btn.kind = key.kind();
                let id = tree.insert_with(
                    row_id,
                    btn,
                    LayoutProps {
                        grow,
                        ..LayoutProps::default()
                    },
                );
                self.keys.push((key, id));
            }
        }
    }

    /// 控件动作：按钮点击 → 反查键身份 → 计算状态机 → 刷新显示。
    fn on_action(&mut self, tree: &mut Tree, from: WidgetId, action: Action) {
        if !action.is::<ButtonClicked>() {
            return;
        }
        let Some(&(key, _)) = self.keys.iter().find(|(_, id)| *id == from) else {
            return;
        };
        key.apply(&mut self.calc);
        self.refresh_display(tree);
    }

    /// 应用级快捷键：在任何控件都未消费按键时到达（映射沿用旧 `key_press`，另补 `=`）。
    fn on_key(&mut self, tree: &mut Tree, key: Key, _modifiers: Modifiers) -> bool {
        let id = match key {
            Key::Char(c) => match c {
                '0'..='9' => Some(KeyId::DIGITS[(c as u8 - b'0') as usize]),
                '.' => Some(KeyId::Dot),
                '+' => Some(KeyId::Add),
                '-' => Some(KeyId::Sub),
                '*' | '×' => Some(KeyId::Mul),
                '/' | '÷' => Some(KeyId::Div),
                '%' => Some(KeyId::Percent),
                'c' | 'C' => Some(KeyId::Clear),
                '=' => Some(KeyId::Eq),
                _ => None,
            },
            Key::Enter => Some(KeyId::Eq),
            Key::Escape => Some(KeyId::Clear),
            Key::Backspace => {
                self.calc.delete_last();
                self.refresh_display(tree);
                return true;
            }
            _ => None,
        };
        let Some(id) = id else { return false };
        id.apply(&mut self.calc);
        self.refresh_display(tree);
        true
    }
}

// ── 测试（元素树夹具，参 docs/ELEMENT_TREE.md §Ⅹ M2 保险断言）────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_harness::element::testing::TestHarness;

    /// 建树 + 出一帧。
    fn setup(w: f32, h: f32) -> (CalculatorApp, TestHarness) {
        let mut app = CalculatorApp::new();
        let mut harness = TestHarness::new(w, h);
        app.build(&mut harness.tree);
        harness.frame();
        (app, harness)
    }

    /// 把腾起的控件动作喂给 App（等价 `TreeHost::drain_actions` 的最小实现）。
    fn pump(app: &mut CalculatorApp, harness: &mut TestHarness) {
        for (from, action) in harness.take_actions() {
            app.on_action(&mut harness.tree, from, action);
        }
    }

    fn key_id(app: &CalculatorApp, key: KeyId) -> WidgetId {
        app.keys
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, id)| *id)
            .expect("键在树中")
    }

    fn click(app: &mut CalculatorApp, harness: &mut TestHarness, key: KeyId) {
        let id = key_id(app, key);
        harness.click(id);
        pump(app, harness);
    }

    /// 显示节点当前文本（验证动作 / 键盘路径确实写回了树，而非只动了 `Calc`）。
    fn shown(app: &CalculatorApp, harness: &TestHarness) -> String {
        let id = app.display.expect("显示节点已建");
        harness
            .tree
            .get::<Label>(id)
            .expect("显示节点是 Label")
            .text
            .clone()
    }

    #[test]
    fn keypad_actions_evaluate_to_42() {
        let (mut app, mut harness) = setup(320.0, 522.0);
        for key in [
            KeyId::D2,
            KeyId::D5,
            KeyId::Add,
            KeyId::D1,
            KeyId::D7,
            KeyId::Eq,
        ] {
            click(&mut app, &mut harness, key);
        }
        assert_eq!(shown(&app, &harness), "42", "25 + 17 = 42，经动作路径");
    }

    #[test]
    fn keyboard_shortcuts_evaluate_to_42() {
        let (mut app, mut harness) = setup(320.0, 522.0);
        for key in [
            Key::Char('2'),
            Key::Char('5'),
            Key::Char('+'),
            Key::Char('1'),
            Key::Char('7'),
            Key::Enter,
        ] {
            assert!(app.on_key(&mut harness.tree, key, Modifiers::NONE));
        }
        assert_eq!(shown(&app, &harness), "42", "键盘 25+17 = 42");
    }

    #[test]
    fn insurance_assertions_hold_at_two_sizes() {
        for (w, h) in [(320.0, 480.0), (640.0, 960.0)] {
            let (app, harness) = setup(w, h);
            harness.assert_contained();
            for (_, id) in &app.keys {
                harness.assert_no_hit_outside(*id);
                harness.assert_paint_within(*id, Insets::ZERO);
            }
        }
    }

    #[test]
    fn zero_key_spans_two_columns() {
        let (app, harness) = setup(320.0, 480.0);
        let w0 = harness.rect(key_id(&app, KeyId::D0)).size.width;
        let w5 = harness.rect(key_id(&app, KeyId::D5)).size.width;
        assert!(
            (w0 - 2.0 * w5).abs() <= GAP + 0.5,
            "0 键宽 {w0} 应约为数字键宽 {w5} 的两倍（容差一个 gap {GAP}）"
        );
    }

    #[test]
    fn keyboard_backspace_deletes_last_digit() {
        let (mut app, mut harness) = setup(320.0, 522.0);
        for key in [Key::Char('9'), Key::Char('7')] {
            assert!(app.on_key(&mut harness.tree, key, Modifiers::NONE));
        }
        assert!(app.on_key(&mut harness.tree, Key::Backspace, Modifiers::NONE));
        assert_eq!(shown(&app, &harness), "9");
    }

    #[test]
    fn keyboard_escape_clears() {
        let (mut app, mut harness) = setup(320.0, 522.0);
        assert!(app.on_key(&mut harness.tree, Key::Char('5'), Modifiers::NONE));
        assert!(app.on_key(&mut harness.tree, Key::Escape, Modifiers::NONE));
        assert_eq!(shown(&app, &harness), "0");
    }

    /// 未映射的按键不消费、不改变显示（应用级快捷键契约）。
    #[test]
    fn unmapped_keys_report_unhandled() {
        let (mut app, mut harness) = setup(320.0, 522.0);
        assert!(!app.on_key(&mut harness.tree, Key::Char('z'), Modifiers::NONE));
        assert!(!app.on_key(&mut harness.tree, Key::Tab, Modifiers::NONE));
    }

    /// 移植后不再有手写几何：显示区在键盘之上，0 键贴左边距。
    #[test]
    fn layout_places_display_above_keypad_and_zero_at_left_pad() {
        let (app, harness) = setup(320.0, 522.0);
        let display = harness.rect(app.display.unwrap());
        let zero = harness.rect(key_id(&app, KeyId::D0));
        assert!(
            display.bottom() <= zero.origin.y + 0.01,
            "显示区应位于键盘之上"
        );
        assert!(
            (zero.origin.x - PAD).abs() < 0.51,
            "0 键左边距 = PAD，实际 {}",
            zero.origin.x
        );
    }
}
