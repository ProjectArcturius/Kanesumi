// app.rs —— 计算器应用：Kanesumi 控件构建（App trait 契约，参 harness app.rs）。
//
// 用 MetroButton（Standard/Accent）+ 手排网格布局 + 右对齐大号结果显示。
// 观察到的 runtime 缺口（狗粮化产出，参 Ether-main TILES_DESIGN.md §8 精神）：
// 1. 无 UniformGrid 布局原语 —— 键盘/磁贴墙的等分网格要手算 rect；
// 2. harness 无键盘输入 —— 计算器目前纯鼠标驱动。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_controls::{ButtonKind, ControlState, MetroButton};
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, Size, TextStyle};
use kanesumi_harness::{App, AppConfig, EtherRole, InputEvent, Key, PointerButton};
use kanesumi_structure::UniformGrid;

use crate::calc::{Calc, Op};

// ── 布局常量（逻辑像素）。参 KANESUMI_DESIGN §Ⅱ 贴边几何。 ──────────────────────────

const PAD: f32 = 8.0;
const GAP: f32 = 8.0;
/// 显示区高度（右对齐结果行）。
const DISPLAY_H: f32 = 116.0;
/// 键盘列数（4 列 × 方形单元，0 键跨 2 列）。
const COLS: usize = 4;

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

/// 键位表（行序）。`0` 在第 5 行跨 2 列。
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

/// 计算器应用。
pub struct CalculatorApp {
    theme: MetroTheme,
    config: AppConfig,
    calc: Calc,
    /// 每个键的 MetroButton（state 每帧从 hovered/pressed 同步）。
    buttons: Vec<MetroButton>,
    /// 上一帧键位矩形（输入路由用）。
    rects: Vec<Rect>,
    /// 上一帧显示区矩形。
    display_rect: Rect,
    hovered: Option<usize>,
    pressed: Option<usize>,
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
        let buttons = KEYS
            .iter()
            .map(|id| {
                let mut b = MetroButton::new(id.label());
                b.kind = id.kind();
                b
            })
            .collect();
        Self {
            theme,
            config: AppConfig::new(
                "org.ether.calculator",
                "计算器",
                EtherRole::Browser,
                320.0,
                // 4 列方形单元(70) × 5 行 + 4 间隔 + 显示区 + 外边距：8+116+8+382+8 = 522。
                522.0,
            ),
            calc: Calc::new(),
            buttons,
            rects: Vec::new(),
            display_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            hovered: None,
            pressed: None,
        }
    }

    // ── 布局（每帧从窗口尺寸计算） ──────────────────────────────────────────────────

    /// 键盘区：4 列均匀网格（UniformGrid），0 键跨 2 列；单元方形由列数反推。
    fn layout(&self, size: Size) -> (Rect, Vec<Rect>) {
        let display = Rect::new(PAD, PAD, size.width - PAD * 2.0, DISPLAY_H);
        let keypad_y = PAD + DISPLAY_H + GAP;
        let keypad = Rect::new(
            PAD,
            keypad_y,
            size.width - PAD * 2.0,
            (size.height - keypad_y - PAD).max(0.0),
        );
        let mut grid = UniformGrid::new(keypad, COLS, GAP);
        let rects = KEYS
            .iter()
            .map(|id| {
                let span = if *id == KeyId::D0 { (2, 1) } else { (1, 1) };
                grid.allocate(span)
            })
            .collect();
        (display, rects)
    }

    /// 命中测试：返回键索引。
    fn key_at(&self, p: Point) -> Option<usize> {
        self.rects
            .iter()
            .position(|r| r.contains(p))
    }

    /// 显示文本样式：按长度自适应字号，避免溢出显示区。
    fn display_style(&self, len: usize) -> TextStyle {
        let size = match len {
            0..=7 => 46.0,
            8..=10 => 36.0,
            11..=13 => 28.0,
            _ => 22.0,
        };
        TextStyle::new(size, size * 1.25, FontWeight::Semilight)
    }

    // ── 输入路由 ────────────────────────────────────────────────────────────────────

    fn update_hover(&mut self, p: Point) {
        let hit = self.key_at(p);
        if hit != self.hovered {
            self.hovered = hit;
            self.sync_states();
        }
    }

    fn press(&mut self, p: Point) {
        self.pressed = self.key_at(p);
        self.sync_states();
    }

    fn release(&mut self, p: Point) {
        let Some(pidx) = self.pressed else { return };
        self.pressed = None;
        // 弹起仍落在同一键上才触发。
        if self.key_at(p) == Some(pidx) {
            let id = KEYS[pidx];
            id.apply(&mut self.calc);
        }
        self.update_hover(p);
    }

    fn clear_hover(&mut self) {
        self.hovered = None;
        self.pressed = None;
        self.sync_states();
    }

    /// 键盘 → 计算状态机（映射与键位表独立，字符语义对齐计算器习惯）。
    fn key_press(&mut self, key: Key) {
        match key {
            Key::Char(c) => match c {
                '0'..='9' => self.calc.input_digit(c as u8 - b'0'),
                '.' => self.calc.input_decimal(),
                '+' => self.calc.apply_op(Op::Add),
                '-' => self.calc.apply_op(Op::Sub),
                '*' | '×' => self.calc.apply_op(Op::Mul),
                '/' | '÷' => self.calc.apply_op(Op::Div),
                '%' => self.calc.percent(),
                'c' | 'C' => self.calc.clear(),
                _ => {}
            },
            Key::Enter => self.calc.equals(),
            Key::Backspace => self.calc.delete_last(),
            Key::Escape => self.calc.clear(),
            _ => {}
        }
    }

    /// 每帧同步键状态（hover/pressed 覆盖 Normal）。
    fn sync_states(&mut self) {
        for (i, btn) in self.buttons.iter_mut().enumerate() {
            btn.state = if self.pressed == Some(i) {
                ControlState::Pressed
            } else if self.hovered == Some(i) {
                ControlState::Hovered
            } else {
                ControlState::Normal
            };
        }
    }
}

impl App for CalculatorApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn handle_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMoved { x, y } => self.update_hover(Point::new(x, y)),
            InputEvent::PointerPressed { x, y, button, .. } => {
                if button == PointerButton::Left {
                    self.press(Point::new(x, y));
                }
            }
            InputEvent::PointerReleased { x, y, button, .. } => {
                if button == PointerButton::Left {
                    self.release(Point::new(x, y));
                }
            }
            InputEvent::PointerLeft => self.clear_hover(),
            InputEvent::Scroll { .. } => {}
            InputEvent::KeyPressed { key, .. } => self.key_press(key),
            // IME 事件：计算器无文本输入，忽略（组合态/提交均不适用）。
            InputEvent::Preedit { .. } | InputEvent::Commit { .. } | InputEvent::DeleteSurrounding { .. } => {
            }
        }
    }

    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene {
        let mut scene = Scene::default();

        // 布局（输入路由同步上一帧 rects）
        let (display_rect, rects) = self.layout(size);
        self.display_rect = display_rect;
        self.rects = rects;
        self.sync_states();
        let colors = &self.theme.colors;

        // 背景
        scene.fill_rect(colors.background, Rect::new(0.0, 0.0, size.width, size.height));

        // 显示区：右对齐结果行 + 底部分隔线
        let disp = self.calc.display().to_string();
        let style = self.display_style(disp.len());
        let text_w = engine.measure(&disp, style.size);
        let x = (display_rect.right() - text_w).max(0.0);
        let y = display_rect.origin.y
            + (display_rect.size.height - style.line_height) / 2.0
            - 8.0;
        scene.text(
            disp,
            Rect::new(x, y, text_w, style.line_height),
            colors.on_background,
            style,
            TextAlign::Left,
        );
        scene.fill_rect(
            colors.divider,
            Rect::new(
                display_rect.origin.x,
                display_rect.bottom(),
                display_rect.size.width,
                1.0,
            ),
        );

        // 键区
        for (rect, btn) in self.rects.iter().zip(self.buttons.iter()) {
            btn.render(&self.theme, engine, *rect, &mut scene);
        }

        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::text::TextEngine;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn app() -> CalculatorApp {
        CalculatorApp::new()
    }

    /// 渲染一帧（同步 rects）→ 点键 → 返回显示文本。
    fn click_key(app: &mut CalculatorApp, key: KeyId) {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let _ = app.render(&engine, Size::new(320.0, 522.0));
        let idx = KEYS.iter().position(|&k| k == key).unwrap();
        let center = app.rects[idx].center();
        app.handle_input(InputEvent::PointerPressed {
            x: center.x,
            y: center.y,
            button: PointerButton::Left,
            modifiers: kanesumi_harness::Modifiers::NONE,
        });
        app.handle_input(InputEvent::PointerReleased {
            x: center.x,
            y: center.y,
            button: PointerButton::Left,
            modifiers: kanesumi_harness::Modifiers::NONE,
        });
    }

    #[test]
    fn digit_keys_enter_display() {
        let mut a = app();
        click_key(&mut a, KeyId::D1);
        assert_eq!(a.calc.display(), "1");
        click_key(&mut a, KeyId::D2);
        assert_eq!(a.calc.display(), "12");
    }

    #[test]
    fn arithmetic_via_keypad() {
        let mut a = app();
        click_key(&mut a, KeyId::D9);
        click_key(&mut a, KeyId::Add);
        click_key(&mut a, KeyId::D3);
        click_key(&mut a, KeyId::Eq);
        assert_eq!(a.calc.display(), "12");
    }

    #[test]
    fn clear_key_resets() {
        let mut a = app();
        click_key(&mut a, KeyId::D5);
        click_key(&mut a, KeyId::Clear);
        assert_eq!(a.calc.display(), "0");
    }

    #[test]
    fn render_produces_display_and_keys() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let mut a = app();
        let scene = a.render(&engine, Size::new(320.0, 522.0));
        assert!(!scene.commands.is_empty());
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 19, "显示 + 至少 19 个键标签，实际 {texts}");
    }

    #[test]
    fn hover_tracks_key() {
        let mut a = app();
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let _ = a.render(&engine, Size::new(320.0, 522.0));
        let center = a.rects[0].center();
        a.handle_input(InputEvent::PointerMoved {
            x: center.x,
            y: center.y,
        });
        assert_eq!(a.hovered, Some(0));
        assert_eq!(a.buttons[0].state, ControlState::Hovered);
    }

    #[test]
    fn keyboard_enters_digits_and_operators() {
        let mut a = app();
        for c in ['1', '2', '3'] {
            a.handle_input(InputEvent::KeyPressed {key: Key::Char(c), modifiers: kanesumi_harness::Modifiers::NONE});
        }
        assert_eq!(a.calc.display(), "123");
        a.handle_input(InputEvent::KeyPressed {key: Key::Char('+'), modifiers: kanesumi_harness::Modifiers::NONE});
        a.handle_input(InputEvent::KeyPressed {key: Key::Char('4'), modifiers: kanesumi_harness::Modifiers::NONE});
        a.handle_input(InputEvent::KeyPressed {key: Key::Enter, modifiers: kanesumi_harness::Modifiers::NONE});
        assert_eq!(a.calc.display(), "127", "123+4=127");
    }

    #[test]
    fn keyboard_backspace_deletes_last_digit() {
        let mut a = app();
        a.handle_input(InputEvent::KeyPressed {key: Key::Char('9'), modifiers: kanesumi_harness::Modifiers::NONE});
        a.handle_input(InputEvent::KeyPressed {key: Key::Char('7'), modifiers: kanesumi_harness::Modifiers::NONE});
        a.handle_input(InputEvent::KeyPressed {key: Key::Backspace, modifiers: kanesumi_harness::Modifiers::NONE});
        assert_eq!(a.calc.display(), "9");
    }

    #[test]
    fn keyboard_escape_clears() {
        let mut a = app();
        a.handle_input(InputEvent::KeyPressed {key: Key::Char('5'), modifiers: kanesumi_harness::Modifiers::NONE});
        a.handle_input(InputEvent::KeyPressed {key: Key::Escape, modifiers: kanesumi_harness::Modifiers::NONE});
        assert_eq!(a.calc.display(), "0");
    }
}
