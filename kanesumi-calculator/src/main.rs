// 计算器入口。
//
// Linux：CalculatorApp → harness Wayland+wgpu 外壳（在 Plasma / Ether 上运行）。
// 字体由 harness 按 KANESUMI_TEST_FONT → 系统字体顺序加载（参 harness platform::find_font）。
// 非 Linux：纯逻辑 smoke（计算状态机自检），保持跨平台可测。

#[cfg(target_os = "linux")]
fn main() {
    let app = kanesumi_calculator::CalculatorApp::new();
    // Box::leak → &'static mut CalculatorApp → &mut dyn App（run 永不返回，生命周期合法）。
    kanesumi_harness::platform::run(Box::leak(Box::new(app)));
}

#[cfg(not(target_os = "linux"))]
fn main() {
    use kanesumi_calculator::Calc;
    let mut calc = Calc::new();
    for d in [2, 5] {
        calc.input_digit(d);
    }
    calc.apply_op(kanesumi_calculator::Op::Add);
    for d in [1, 7] {
        calc.input_digit(d);
    }
    calc.equals();
    println!("Kanesumi Calculator —— 25 + 17 = {}", calc.display());
}
