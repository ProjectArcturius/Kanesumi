// diagnostics.rs —— 元素树侧的行为诊断：严格模式开关 + 几何不变量 + 场景命令浮点校验。
// 参 BEHAVIOR_CORPUS §一-2（严格模式）、§一-4（几何不变量）。
//
// 为什么开关放在元素树而不是外壳：几何（arrange 产出的 rect）与场景命令都在这里生成，
// 外壳拿到的已经是成品 —— 一个 NaN 矩形传到外壳时光栅器只会悄悄丢图元（画面上少一块），
// 现场已经离成因很远了。这里能在**产生点**给出元素路径。
//
// 默认（未设 `KANESUMI_STRICT=1`）行为与从前逐字节一致：几何检查只在 debug 构建打印
// 错误日志、不 panic；命令浮点校验整段跳过（不改热路径，也不改生产语义）。
// 严格模式下违例一律 **panic**（外壳的 `catch_unwind` 会把它连同上下文写进错误日志，
// 参 kanesumi-harness `errors.rs`），这样「静默回退」在夜班里会变成一条可查的崩溃记录。

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

use kanesumi_canvas::SceneCommand;
use kanesumi_core::{Color, Rect};

/// 严格模式开关的环境变量名（与 harneess 侧同名同义）。
pub const STRICT_ENV: &str = "KANESUMI_STRICT";

/// 测试用覆盖：0 = 跟随环境变量，1 = 强制开，2 = 强制关。
///
/// 为什么要有它：`std::env::set_var` 在 Rust 2024 是 `unsafe`（多线程下改环境不安全），
/// 测试并行跑时更不能用。单测走这个原子量，生产走环境变量，两条路互不干扰。
static STRICT_OVERRIDE: AtomicU8 = AtomicU8::new(0);

/// 环境变量只读一次：指针事件热路径上不得每次 `env::var`（会分配一个 String）。
static STRICT_ENV_CACHE: OnceLock<bool> = OnceLock::new();

/// 严格模式是否开启。
pub fn strict() -> bool {
    match STRICT_OVERRIDE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => *STRICT_ENV_CACHE.get_or_init(|| {
            std::env::var(STRICT_ENV).is_ok_and(|v| v == "1")
        }),
    }
}

/// 测试用：串行化「改全局严格开关」的测试。
///
/// 开关是进程级 `static`：`cargo test` 默认多线程，两个测试各自 set/reset 会互相踩
/// （实测 NaN 几何用例在并行下偶发「没 panic」）。凡是要改开关的测试都先拿这把锁。
#[cfg(test)]
pub(crate) fn strict_test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// 测试用：强制开 / 关 / 恢复跟随环境变量。只应在 `#[cfg(test)]` 里调用。
#[doc(hidden)]
pub fn set_strict_for_test(on: Option<bool>) {
    STRICT_OVERRIDE.store(match on {
        Some(true) => 1,
        Some(false) => 2,
        None => 0,
    }, Ordering::Relaxed);
}

/// 不变量：`ok` 为假且处于严格模式 → 带上下文 panic；否则什么都不做。
///
/// `what` 用闭包：非严格模式下连错误字符串都不拼（热路径 / 每帧检查不产生分配）。
pub(crate) fn require(ok: bool, what: impl FnOnce() -> String) {
    if !ok && strict() {
        panic!("{}: {}", STRICT_ENV, what());
    }
}

/// 几何不变量：尺寸不得为负、坐标与尺寸不得为 NaN / 无穷。
///
/// debug 构建下**总是**调用（违例先打印元素路径），严格模式下再 panic —— 这样日构建
/// （debug 夜班）能留下现场，而 release 生产保持原行为。
pub(crate) fn check_rect(what: &str, rect: Rect, path: impl FnOnce() -> String) {
    if rect_is_sane(rect) {
        return;
    }
    let path = path();
    // debug 构建的观察口：不 panic 也要留下「哪个元素、错成什么样」。
    log::error!("kanesumi-element: {what} 几何违例 {rect:?} 路径 {path}");
    if strict() {
        panic!("{STRICT_ENV}: {what} 几何违例 {rect:?} 路径 {path}");
    }
}

/// 尺寸非负且四个分量都是有限值。
pub fn rect_is_sane(rect: Rect) -> bool {
    let r = rect;
    r.origin.x.is_finite()
        && r.origin.y.is_finite()
        && r.size.width.is_finite()
        && r.size.height.is_finite()
        && r.size.width >= 0.0
        && r.size.height >= 0.0
}

/// 场景命令里的浮点必须有限（严格模式才检查）。
///
/// 为什么放在 compose 之后而不是光栅器里：光栅器拿到 NaN 只会把图元丢掉或画到屏幕外，
/// 表现是「莫名其妙少一块」；在这里能同时给出元素路径，一眼看出是哪个控件算出来的。
pub(crate) fn check_command(cmd: &SceneCommand, path: impl FnOnce() -> String) {
    if !strict() || command_is_finite(cmd) {
        return;
    }
    let path = path();
    panic!("{STRICT_ENV}: 场景命令含非有限浮点 {cmd:?} 路径 {path}");
}

/// 单条命令的所有浮点分量是否有限（矩形 / 线宽 / 颜色）。
///
/// 公开是为了让**外层应用**（如设置树的全页扫描）能在自己的测试里按同一判据核查，
/// 而不必打开全局严格开关（那会在并行测试里串味）。
pub fn command_is_finite(cmd: &SceneCommand) -> bool {
    let finite = |v: f32| v.is_finite();
    let rect_ok = |r: &Rect| {
        finite(r.origin.x) && finite(r.origin.y) && finite(r.size.width) && finite(r.size.height)
    };
    let color_ok = |c: &Color| finite(c.r) && finite(c.g) && finite(c.b) && finite(c.a);
    match cmd {
        SceneCommand::PushClip { rect } => rect_ok(rect),
        SceneCommand::PopClip => true,
        SceneCommand::FillRect { color, rect, corner_radius } => {
            rect_ok(rect) && color_ok(color) && finite(*corner_radius)
        }
        SceneCommand::StrokeRect { color, rect, thickness, corner_radius } => {
            rect_ok(rect) && color_ok(color) && finite(*thickness) && finite(*corner_radius)
        }
        // 其余图元（文本 / 图像 / 弧 / 线）走保守路径：字段随画布扩展而变，
        // 用穷举 match 会逼着每次加图元都改这里 —— 先只校公共的矩形与颜色。
        other => command_extras_finite(other),
    }
}

/// 非穷举图元的兜底校验：`SceneCommand` 是 `#[non_exhaustive]`，新图元至少要能过这里。
///
/// 用 `Debug` 文本扫一遍 `NaN` / `inf` 不是严谨做法，但它**不会漏**非有限值（最坏是多报），
/// 且只在本函数被调用时（严格模式）付出代价。真正的逐字段校验等画布提供
/// `SceneCommand::bounds()` 一类公共访问器后再补。
fn command_extras_finite(cmd: &SceneCommand) -> bool {
    let text = format!("{cmd:?}");
    !text.contains("NaN") && !text.contains("inf")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn sane_rects_pass_and_bad_ones_are_caught() {
        let _guard = strict_test_lock();
        assert!(rect_is_sane(rect(0.0, 0.0, 10.0, 10.0)));
        assert!(rect_is_sane(rect(-5.0, -5.0, 0.0, 0.0)), "负坐标合法（可滚动内容）");
        assert!(!rect_is_sane(rect(0.0, 0.0, -1.0, 10.0)), "负宽必须判违例");
        assert!(!rect_is_sane(rect(f32::NAN, 0.0, 10.0, 10.0)));
        assert!(!rect_is_sane(rect(0.0, 0.0, f32::INFINITY, 10.0)));
    }

    /// 严格模式下必 panic；非严格模式下**不得** panic（默认路径不受影响）。
    #[test]
    fn geometry_violation_only_panics_in_strict_mode() {
        let _guard = strict_test_lock();
        let bad = rect(0.0, 0.0, -1.0, 10.0);
        set_strict_for_test(Some(false));
        check_rect("test", bad, || "/bad".into()); // 不 panic
        set_strict_for_test(Some(true));
        let hit = std::panic::catch_unwind(|| check_rect("test", bad, || "/bad".into()));
        set_strict_for_test(None);
        assert!(hit.is_err(), "严格模式下负尺寸必须 panic");
    }

    #[test]
    fn non_finite_command_is_detected() {
        let _guard = strict_test_lock();
        let cmd = SceneCommand::FillRect {
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            rect: rect(0.0, 0.0, f32::NAN, 4.0),
            corner_radius: 0.0,
        };
        assert!(!command_is_finite(&cmd));
        let ok = SceneCommand::FillRect {
            color: Color::new(1.0, 1.0, 1.0, 1.0),
            rect: rect(0.0, 0.0, 4.0, 4.0),
            corner_radius: 0.0,
        };
        assert!(command_is_finite(&ok));
        // 非严格模式：check_command 不 panic（默认路径不变）。
        set_strict_for_test(Some(false));
        check_command(&cmd, || "/bad".into());
        set_strict_for_test(None);
    }
}
