// strict.rs —— `KANESUMI_STRICT=1` 严格模式：把静默回退变成硬失败。
// 参 BEHAVIOR_CORPUS §一-2。
//
// 为什么需要：静默回退在正常使用里「看起来没事」—— 字符落到方框、图标没画、纹理没上传、
// 帧缓冲取不到就跳过一帧。用户看到的是「莫名其妙出错」，日志里一片安静。严格模式把这些
// 分岔点全部改成 **panic**（不是 `Err`：单帧路径上没有调用方愿意逐层转播错误，而 panic 会
// 被外壳的 `catch_unwind` 接住、连同元素路径与帧号写进 `kanesumi-errors.log`）。夜班 / CI
// 用 `KANESUMI_STRICT=1` 跑一遍，任何「降级运行」都会变成一条可查的崩溃记录。
//
// 默认（未设该变量）**行为一个字节都不变**：本模块的 `require` 在非严格模式下只是
// 一次原子读 + 一个分支，不打印、不 panic、不改返回值。
//
// 覆盖面（与 BEHAVIOR_CORPUS §一-2 一一对应）：
//   1. 字体缺失回退            —— `platform::find_font_source`（主字面落到拉丁 / 繁体回退组）
//                                 与 `extra_sources`（字重字面缺失 → 回落 Regular）；
//   2. 纹理上传失败            —— `render::ensure_glyph` / `ensure_image` 的静默跳过；
//   3. 几何 NaN / 负尺寸       —— `kanesumi-element`（arrange 处，同一开关）；
//   4. 命中表为空却收到指针事件 —— `kanesumi-element`（指针入口，同一开关）；
//   5. 命令里出现非有限浮点     —— `kanesumi-element`（拼接处，同一开关）。
// 3~5 在元素树里实现（产生点才有元素路径），开关同名同义，两边读同一个环境变量。

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// 严格模式环境变量名（与 `kanesumi-element` 侧一致）。
pub const STRICT_ENV: &str = "KANESUMI_STRICT";

/// 测试用覆盖：0 = 跟随环境，1 = 强制开，2 = 强制关（`env::set_var` 在 Rust 2024 是 unsafe）。
static STRICT_OVERRIDE: AtomicU8 = AtomicU8::new(0);
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

/// 测试用：强制开 / 关 / 恢复跟随环境变量。
#[doc(hidden)]
pub fn set_strict_for_test(on: Option<bool>) {
    STRICT_OVERRIDE.store(
        match on {
            Some(true) => 1,
            Some(false) => 2,
            None => 0,
        },
        Ordering::Relaxed,
    );
}

/// 不变量：`ok` 为假且处于严格模式 → 带上下文 panic；否则什么都不做。
///
/// `what` 用闭包：非严格模式下连消息都不拼（这是每帧 / 每纹理都可能走到的点）。
pub fn require(ok: bool, what: impl FnOnce() -> String) {
    if !ok && strict() {
        panic!("{}: {}", STRICT_ENV, what());
    }
}

/// 记录一次「本来会被静默吞掉」的降级；严格模式下直接 panic。
pub fn require_or_log(ok: bool, what: impl FnOnce() -> String) {
    if ok {
        return;
    }
    if strict() {
        panic!("{}: {}", STRICT_ENV, what());
    }
    log::warn!("降级运行：{}", what());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strict_defaults_off_and_can_be_overridden() {
        set_strict_for_test(Some(false));
        assert!(!strict());
        require(false, || "不该 panic".into()); // 非严格 → 什么都不做

        set_strict_for_test(Some(true));
        assert!(strict());
        let hit = std::panic::catch_unwind(|| require(false, || "字体缺失".into()));
        assert!(hit.is_err(), "严格模式下违例必须 panic");

        // 通过的情形不 panic（严格模式不是「一律 panic」）。
        require(true, || "不该走到".into());
        set_strict_for_test(None);
    }

    #[test]
    fn violation_message_carries_the_strict_marker() {
        set_strict_for_test(Some(true));
        let err = std::panic::catch_unwind(|| require_or_log(false, || "纹理上传失败".into()))
            .unwrap_err();
        set_strict_for_test(None);
        let msg = crate::errors::payload_message(&*err);
        assert!(msg.contains(STRICT_ENV), "panic 消息必须带开关名：{msg}");
        assert!(msg.contains("纹理上传失败"), "panic 消息必须带成因：{msg}");
    }
}
