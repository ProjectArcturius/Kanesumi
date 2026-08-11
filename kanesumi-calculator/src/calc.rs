// calc.rs —— 计算器纯逻辑（无 UI，跨平台可测）。
//
// 状态机：显示字符串 + 累加器 + 待定运算符 + 新条目标志 + 错误标志。
// 输入语义对齐 Windows 10 计算器（标准型）：连等、链式运算符、CE 语义按 C 处理。

/// 运算符。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
}

/// 计算状态机。
#[derive(Debug, Clone, PartialEq)]
pub struct Calc {
    display: String,
    acc: f64,
    op: Option<Op>,
    fresh: bool,
    error: bool,
}

/// 数字显示精度上限（超长输入拒绝，避免排版溢出）。
const MAX_DIGITS: usize = 15;

impl Calc {
    pub fn new() -> Self {
        Self {
            display: "0".into(),
            acc: 0.0,
            op: None,
            fresh: true,
            error: false,
        }
    }
}

impl Default for Calc {
    fn default() -> Self {
        Self::new()
    }
}

/// 当前显示文本（含运算符符号 / 错误文案）。
impl Calc {
    pub fn display(&self) -> &str {
        &self.display
    }

    /// 当前条目数值（错误态为 0，输入会先清错误）。
    fn cur(&self) -> f64 {
        self.display.parse().unwrap_or(0.0)
    }

    fn set_value(&mut self, v: f64) {
        self.display = fmt(v);
        self.error = v.is_nan() || v.is_infinite();
    }

    /// 数字键 0-9。
    pub fn input_digit(&mut self, d: u8) {
        if self.error {
            self.clear();
        }
        if self.fresh {
            self.display = d.to_string();
            self.fresh = false;
        } else if self.display.len() < MAX_DIGITS {
            if self.display == "0" {
                self.display = d.to_string();
            } else {
                self.display.push(char::from(b'0' + d));
            }
        }
    }

    /// 小数点键。
    pub fn input_decimal(&mut self) {
        if self.error {
            self.clear();
        }
        if self.fresh {
            self.display = "0.".into();
            self.fresh = false;
        } else if !self.display.contains('.') {
            self.display.push('.');
        }
    }

    /// 运算符键：若有待定运算符先结算，再存累加器 + 新运算符。
    pub fn apply_op(&mut self, op: Op) {
        if self.error {
            return;
        }
        let cur = self.cur();
        if let Some(prev) = self.op {
            self.acc = apply(prev, self.acc, cur);
        } else {
            self.acc = cur;
        }
        if self.acc.is_nan() || self.acc.is_infinite() {
            self.error = true;
            self.display = fmt(self.acc);
            return;
        }
        self.op = Some(op);
        self.fresh = true;
        self.display = fmt(self.acc);
    }

    /// 等号：结算待定运算符。
    pub fn equals(&mut self) {
        if self.error {
            return;
        }
        if let Some(prev) = self.op.take() {
            let r = apply(prev, self.acc, self.cur());
            self.acc = 0.0;
            self.fresh = true;
            self.set_value(r);
        }
    }

    /// 全部清除（C）。
    pub fn clear(&mut self) {
        *self = Self::new();
    }

    /// 清当前条目（CE）—— 显示归零，保留累加器与运算符。
    pub fn clear_entry(&mut self) {
        self.display = "0".into();
        self.fresh = true;
    }

    /// 退格：删显示末位（Backspace）。
    pub fn delete_last(&mut self) {
        if self.error || self.fresh {
            return;
        }
        self.display.pop();
        if self.display.is_empty() || self.display == "-" || self.display == "." || self.display == "-." {
            self.display = "0".into();
        }
        if self.display == "-0" {
            self.display = "0".into();
        }
    }

    /// 正负号。
    pub fn toggle_sign(&mut self) {
        if self.error {
            return;
        }
        if self.display.starts_with('-') {
            self.display.remove(0);
        } else if self.display != "0" {
            self.display.insert(0, '-');
        }
        if self.display == "-0" {
            self.display = "0".into();
        }
    }

    /// 百分号：当前条目 ÷100。
    pub fn percent(&mut self) {
        if self.error {
            return;
        }
        let v = self.cur() / 100.0;
        self.fresh = false;
        self.set_value(v);
    }
}

/// 结算一对 (a op b)。除零 → NaN（上层转错误态）。
fn apply(op: Op, a: f64, b: f64) -> f64 {
    match op {
        Op::Add => a + b,
        Op::Sub => a - b,
        Op::Mul => a * b,
        Op::Div => {
            if b == 0.0 {
                f64::NAN
            } else {
                a / b
            }
        }
    }
}

/// 数值 → 显示文本。整数不带小数点；小数最多 10 位并去尾零；
/// 极大极小走科学计数（保 6 位有效）。错误 → 中文文案。
fn fmt(v: f64) -> String {
    if v.is_nan() {
        return "错误".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "∞".into() } else { "-∞".into() };
    }
    if v == 0.0 {
        return "0".into();
    }
    if v == v.trunc() && v.abs() < 1e15 {
        return format!("{}", v as i64);
    }
    if v.abs() >= 1e12 || v.abs() < 1e-9 {
        return format!("{:.6e}", v);
    }
    let s = format!("{:.10}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press_calc() -> Calc {
        Calc::new()
    }

    #[test]
    fn starts_at_zero() {
        let c = press_calc();
        assert_eq!(c.display(), "0");
    }

    #[test]
    fn digit_entry_replaces_zero() {
        let mut c = press_calc();
        c.input_digit(7);
        assert_eq!(c.display(), "7");
        c.input_digit(5);
        assert_eq!(c.display(), "75");
    }

    #[test]
    fn basic_addition() {
        let mut c = press_calc();
        for d in [1, 2] {
            c.input_digit(d);
        }
        c.apply_op(Op::Add);
        for d in [3, 4] {
            c.input_digit(d);
        }
        c.equals();
        assert_eq!(c.display(), "46");
    }

    #[test]
    fn subtraction_multiplication_division() {
        let mut c = press_calc();
        c.input_digit(9);
        c.apply_op(Op::Sub);
        c.input_digit(4);
        c.equals();
        assert_eq!(c.display(), "5");

        let mut c = press_calc();
        c.input_digit(6);
        c.apply_op(Op::Mul);
        c.input_digit(7);
        c.equals();
        assert_eq!(c.display(), "42");

        let mut c = press_calc();
        c.input_digit(8);
        c.apply_op(Op::Div);
        c.input_digit(2);
        c.equals();
        assert_eq!(c.display(), "4");
    }

    #[test]
    fn chained_operators_evaluate_left_to_right() {
        let mut c = press_calc();
        c.input_digit(2);
        c.apply_op(Op::Add);
        c.input_digit(3);
        c.apply_op(Op::Mul); // 结算 2+3=5，待定 ×
        c.input_digit(4);
        c.equals();
        assert_eq!(c.display(), "20", "2+3×4 = (2+3)×4 = 20（无优先级，左结合）");
    }

    #[test]
    fn equals_repeats_nothing_when_no_op() {
        let mut c = press_calc();
        c.input_digit(5);
        c.equals();
        assert_eq!(c.display(), "5");
    }

    #[test]
    fn decimal_entry() {
        let mut c = press_calc();
        c.input_digit(3);
        c.input_decimal();
        c.input_digit(1);
        c.input_digit(4);
        assert_eq!(c.display(), "3.14");
        // 已含小数点再点无效
        c.input_decimal();
        c.input_digit(5);
        assert_eq!(c.display(), "3.145");
    }

    #[test]
    fn clear_resets() {
        let mut c = press_calc();
        c.input_digit(9);
        c.apply_op(Op::Add);
        c.input_digit(1);
        c.clear();
        assert_eq!(c.display(), "0");
        c.input_digit(2);
        assert_eq!(c.display(), "2", "清除后重新输入");
    }

    #[test]
    fn toggle_sign() {
        let mut c = press_calc();
        c.input_digit(4);
        c.toggle_sign();
        assert_eq!(c.display(), "-4");
        c.toggle_sign();
        assert_eq!(c.display(), "4");
    }

    #[test]
    fn percent_scales_by_hundred() {
        let mut c = press_calc();
        c.input_digit(5);
        c.input_digit(0);
        c.percent();
        assert_eq!(c.display(), "0.5");
    }

    #[test]
    fn division_by_zero_shows_error_then_recovers() {
        let mut c = press_calc();
        c.input_digit(8);
        c.apply_op(Op::Div);
        c.input_digit(0);
        c.equals();
        assert_eq!(c.display(), "错误");
        // 错误态下输入数字自动恢复
        c.input_digit(9);
        assert_eq!(c.display(), "9");
    }

    #[test]
    fn decimal_result_strips_trailing_zeros() {
        let mut c = press_calc();
        c.input_digit(1);
        c.apply_op(Op::Div);
        c.input_digit(8);
        c.equals();
        assert_eq!(c.display(), "0.125");
    }

    #[test]
    fn fresh_entry_after_equals() {
        let mut c = press_calc();
        c.input_digit(2);
        c.apply_op(Op::Add);
        c.input_digit(2);
        c.equals();
        assert_eq!(c.display(), "4");
        c.input_digit(3);
        assert_eq!(c.display(), "3", "等号后输入新条目，不拼在结果后");
    }
}
