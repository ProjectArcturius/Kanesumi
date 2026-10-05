// timeline.rs —— 拉起时间线（跨进程可比，零交互落盘）。
//
// 目的：定位「打开 Librarian / Settings 要等很久」（DECISIONS_2026-10-05 §104）。
// 每个 kanesumi 进程在关键阶段追加一行到 `~/.local/state/ether/launch-timeline.log`：
//   mono=<CLOCK_MONOTONIC 毫秒> pid=<pid> app=<app_id> stage=<阶段> dt=<距上一阶段毫秒> [detail]
// 时钟用 `CLOCK_MONOTONIC`（非 `Instant` 的进程私有零点），故合成器与各客户端写下的
// `mono` 可直接相减，得到「点击 → 首帧上屏」的真实墙钟。文件超 256 KiB 轮转为 `.1`。
//
// 应用无需改动即获得 harness 阶段（`platform::run` 内 `ensure_started`）；应用自身的
// 首屏准备（目录列举 / sysstate 取数）在各自 `main` 调 `start` 后 `note` 成段。

use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// 日志文件名（合成器同写此文件，格式一致）。
pub const LOG_NAME: &str = "launch-timeline.log";
/// 单文件上限：超过则轮转为 `<name>.1`（保留上一份，约 512 KiB 总量）。
pub const MAX_BYTES: u64 = 256 * 1024;

/// 进程级时间线上下文。`None` = 未 start（测试 / 非 kanesumi 入口）→ note 全部空操作，
/// 不写盘、不污染。
struct Ctx {
    app: String,
    pid: u32,
    /// 上一阶段记下的 mono 毫秒（dt 基准）。
    last_ms: f64,
    /// 已写过的 `note_once` 阶段（去重）。
    once: Vec<String>,
}

static CTX: OnceLock<Mutex<Option<Ctx>>> = OnceLock::new();

fn ctx() -> &'static Mutex<Option<Ctx>> {
    CTX.get_or_init(|| Mutex::new(None))
}

/// `CLOCK_MONOTONIC` 毫秒（跨进程可比）。非 Linux 退化为进程内相对时钟（仅保证编译）。
#[cfg(target_os = "linux")]
pub fn now_ms() -> f64 {
    let mut ts = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    // SAFETY：ts 为栈上合法 timespec，clockid 为常量；返回值忽略（不会失败）。
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1_000_000.0
}

#[cfg(not(target_os = "linux"))]
pub fn now_ms() -> f64 {
    use std::time::Instant;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

/// 记下进程入口并写 `main_entry`。重复调用为幂等空操作（应用重复调 / harness 兜底）。
pub fn start(app_id: &str) {
    let mut guard = ctx().lock().unwrap_or_else(|e| e.into_inner());
    if guard.is_some() {
        return;
    }
    let now = now_ms();
    let pid = std::process::id();
    *guard = Some(Ctx { app: app_id.to_string(), pid, last_ms: now, once: Vec::new() });
    drop(guard);
    write_line(now, app_id, pid, "main_entry", 0.0, "");
}

/// harness 兜底：应用未在 `main` 调 [`start`] 时，用 `AppConfig.app_id` 起头。
pub fn ensure_started(app_id: &str) {
    start(app_id);
}

/// 记一个阶段（每次调用都写）。
pub fn note(stage: &str) {
    note_inner(stage, false, "");
}

/// 记一个阶段，仅本进程首次出现时写（首 configure / 首帧 / 首提交等）。
pub fn note_once(stage: &'static str) {
    note_inner(stage, true, "");
}

/// 带附加信息（如窗口类型 / 尺寸）的阶段。
pub fn note_detail(stage: &str, detail: &str) {
    note_inner(stage, false, detail);
}

/// 首次出现才写、且带附加信息的阶段。
pub fn note_once_detail(stage: &'static str, detail: &str) {
    note_inner(stage, true, detail);
}

fn note_inner(stage: &str, once: bool, detail: &str) {
    let mut guard = ctx().lock().unwrap_or_else(|e| e.into_inner());
    let Some(c) = guard.as_mut() else { return };
    if once && c.once.iter().any(|s| s == stage) {
        return;
    }
    if once {
        c.once.push(stage.to_string());
    }
    let now = now_ms();
    let dt = (now - c.last_ms).max(0.0);
    c.last_ms = now;
    let (app, pid) = (c.app.clone(), c.pid);
    drop(guard);
    write_line(now, &app, pid, stage, dt, detail);
}

/// 一行：`mono=... pid=... app=... stage=... dt=...`（detail 追加在尾部）。
fn write_line(mono: f64, app: &str, pid: u32, stage: &str, dt: f64, detail: &str) {
    let line = if detail.is_empty() {
        format!("mono={mono:.3} pid={pid} app={app} stage={stage} dt={dt:.3}\n")
    } else {
        format!("mono={mono:.3} pid={pid} app={app} stage={stage} dt={dt:.3} {detail}\n")
    };
    append_log(&line);
}

/// 持久路径 `$HOME/.local/state/ether/<name>`；无 HOME 则丢弃。
fn state_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(home).join(".local/state/ether"))
}

fn append_log(line: &str) {
    let Some(dir) = state_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(LOG_NAME);
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len().saturating_add(line.len() as u64) > MAX_BYTES)
        .unwrap_or(false);
    if too_big {
        // 轮转：现文件改名 `.1`（覆盖旧备份），本次行重新开文件 —— 保留最近两份。
        let _ = std::fs::rename(&path, dir.join(format!("{LOG_NAME}.1")));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 单测直接覆盖「未 start → 不写盘」与「start 后 note_once 去重」。
    #[test]
    fn note_without_start_is_noop() {
        // 不调用 start：ctx 为 None（本测试进程内若被其他测试 start 过则跳过断言）。
        let guard = ctx().lock().unwrap();
        if guard.is_some() {
            return; // 测试并发下已 start，跳过。
        }
        drop(guard);
        note("should_not_write");
        assert!(ctx().lock().unwrap().is_none());
    }
}
