// 计算器入口。
//
// Linux：CalculatorApp（元素树 TreeApp）→ TreeHost → harness Wayland+wgpu 外壳
// （在 Plasma / Ether 上运行）。参 docs/ELEMENT_TREE.md §Ⅷ。
// 字体由 harness 按 KANESUMI_TEST_FONT → 系统字体顺序加载（参 harness platform::find_font）。
// `--snapshot <out.png> [scale]`：不开窗，经 CPU 光栅器渲染一帧到 PNG（视觉核对用）。
// 非 Linux：纯逻辑 smoke（计算状态机自检），保持跨平台可测。

#[cfg(target_os = "linux")]
fn main() {
    let app = kanesumi_calculator::CalculatorApp::new();
    let mut host = kanesumi_harness::TreeHost::new(app);
    // `--snapshot <out.png> [scale]`：照 tree_demo 的写法，不开窗写一张 PNG。
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = kanesumi_harness::platform::find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(320.0, 522.0),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }
    // Box::leak → &'static mut TreeHost → &mut dyn App（run 永不返回，生命周期合法）。
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
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
