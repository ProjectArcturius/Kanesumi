// errors.rs —— App 回调 panic 的结构化落盘（`~/.local/state/ether/kanesumi-errors.log`）。
// 参 BEHAVIOR_CORPUS §一-1。
//
// 背景：外壳早就有 `catch_unwind`（App panic 不杀进程），但它只写一行
// `App::render panic，已隔离` —— 没有 app、没有帧号、没有当时在点哪个元素、没有最近的命令。
// 真机上「TopBar 少了一半」这种现象，靠那一行日志根本定不到位置。本模块补上这些上下文，
// 并且**折叠连续重复**：稳态里某帧开始每帧 panic，日志不能按 60 行/秒写爆磁盘。
//
// 行格式（一行一个 JSON 对象）：
//   {"ts":…,"app_id":…,"phase":…,"event":…,"tree_path":…,"message":…,
//    "frame_seq":…,"last_cmds":[…],"repeat":…}
// `repeat` 是折叠计数（同 `(phase,message)` 连续第几次），不是外来字段的替代品：
// 前八项是任务书钉死的形状，`repeat` 是折叠必须的计数。
//
// 折叠怎么做：同一 `(phase,message)` 连续出现时不追加新行，而是**回写最后一行**并把
// `repeat` 加一（行尾截断重写）。这样一次连续崩溃在文件里只占一行，条数 = 出事次数。

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// 落盘路径的环境变量（诊断 / 测试用；未设时写 `~/.local/state/ether/`）。
pub const LOG_ENV: &str = "KANESUMI_ERROR_LOG";

/// 出错的阶段。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Update,
    Render,
    HandleInput,
    /// 其它 App 回调（IME、焦点、菜单…）：同一套落盘，只是阶段名不同。
    Other(&'static str),
}

impl Phase {
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Update => "update",
            Phase::Render => "render",
            Phase::HandleInput => "handle_input",
            Phase::Other(name) => name,
        }
    }
}

/// 出错时能拿到的上下文（由 [`crate::App::panic_context`] 提供）。
///
/// 只在捕获到 panic 后取一次 —— 平时不构造，稳态帧不为它分配一个字节。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PanicContext {
    /// 最近一次命中的元素路径（无命中 / 非元素树应用 → None，日志写 `-`）。
    pub tree_path: Option<String>,
    /// 最近一帧的命令摘要（条数 + 首末命令）；非元素树应用为空。
    pub last_cmds: Vec<String>,
}

/// 折叠状态：上一行的键、在文件里的字节偏移与长度、已连续出现次数。
struct Last {
    key: (String, String),
    offset: u64,
    /// 上一行的字节长度（含换行），回写时用来截断残留。
    len: u64,
    repeat: u64,
}

static LAST: Mutex<Option<Last>> = Mutex::new(None);
/// 测试用的路径覆盖（生产走环境变量 / HOME）。
static PATH_OVERRIDE: Mutex<Option<PathBuf>> = Mutex::new(None);

/// 测试用：把错误日志指到临时文件；`None` 恢复默认。
#[doc(hidden)]
pub fn set_log_path_for_test(path: Option<PathBuf>) {
    *PATH_OVERRIDE.lock().unwrap_or_else(|e| e.into_inner()) = path;
    // 换文件后折叠状态必须清掉，否则会拿旧偏移去截新文件。
    *LAST.lock().unwrap_or_else(|e| e.into_inner()) = None;
}

/// 错误日志路径：测试覆盖 → `KANESUMI_ERROR_LOG` → `~/.local/state/ether/kanesumi-errors.log`。
///
/// 为什么默认写 home 而不是 `/tmp`：Debian 会话里崩溃 / 黑屏后无法开终端，只能重启回主
/// 系统读盘（参 AGENTS.md 铁律），tmpfs 会随重启丢。
pub fn log_path() -> Option<PathBuf> {
    if let Some(p) = PATH_OVERRIDE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
    {
        return Some(p);
    }
    if let Some(p) = std::env::var_os(LOG_ENV) {
        if !p.is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(Path::new(&home).join(".local/state/ether/kanesumi-errors.log"))
}

/// 从 `catch_unwind` 的 `Box<dyn Any>` 里取人类可读的 panic 消息。
pub fn payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
        .unwrap_or_else(|| "未知 panic（非字符串 payload）".to_string())
}

/// 记一条结构化错误。写盘失败不 panic（诊断本身不能成为新的崩溃源）。
pub fn report(
    phase: Phase,
    app_id: &str,
    event: &str,
    ctx: &PanicContext,
    message: &str,
    frame_seq: u64,
) {
    let Some(path) = log_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let tree_path = ctx.tree_path.as_deref().unwrap_or("-");
    let key = (phase.as_str().to_string(), message.to_string());
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());

    // 折叠：与上一行同键且文件仍是那份（偏移有效）→ 回写计数。
    if let Some(prev) = last.as_mut() {
        if prev.key == key {
            prev.repeat += 1;
            let line = format!(
                "{}\n",
                json_line(
                    phase,
                    app_id,
                    event,
                    tree_path,
                    message,
                    frame_seq,
                    &ctx.last_cmds,
                    prev.repeat
                )
            );
            if rewrite(&path, prev.offset, prev.len, &line) {
                prev.len = line.len() as u64;
                return;
            }
            // 回写失败（文件被删 / 权限）→ 退回追加，不丢这条。
        }
    }

    let repeat = 1;
    let line = format!(
        "{}\n",
        json_line(phase, app_id, event, tree_path, message, frame_seq, &ctx.last_cmds, repeat)
    );
    let offset = append(&path, &line);
    *last = offset.map(|offset| Last {
        key,
        offset,
        len: line.len() as u64,
        repeat,
    });
}

/// 追加一行，返回该行起始偏移（失败 → None，折叠状态随之清空）。
fn append(path: &Path, line: &str) -> Option<u64> {
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .ok()?;
    let offset = f.metadata().ok()?.len();
    f.write_all(line.as_bytes()).ok()?;
    Some(offset)
}

/// 从 `offset` 起覆盖写 `line`，并把文件截到 `offset + line.len()`（清掉上一行更长的残留）。
fn rewrite(path: &Path, offset: u64, prev_len: u64, line: &str) -> bool {
    let Ok(mut f) = OpenOptions::new().write(true).open(path) else {
        return false;
    };
    if f.metadata().map(|m| m.len()).unwrap_or(0) != offset + prev_len {
        // 文件在这期间被别的进程动过：不要按旧偏移写，交给追加路径。
        return false;
    }
    if f.seek(SeekFrom::Start(offset)).is_err() {
        return false;
    }
    if f.write_all(line.as_bytes()).is_err() {
        return false;
    }
    f.set_len(offset + line.len() as u64).is_ok()
}

fn json_line(
    phase: Phase,
    app_id: &str,
    event: &str,
    tree_path: &str,
    message: &str,
    frame_seq: u64,
    last_cmds: &[String],
    repeat: u64,
) -> String {
    let cmds = if last_cmds.is_empty() {
        "[]".to_string()
    } else {
        let items: Vec<String> = last_cmds.iter().map(|c| format!("\"{}\"", escape(c))).collect();
        format!("[{}]", items.join(","))
    };
    format!(
        "{{\"ts\":\"{}\",\"app_id\":\"{}\",\"phase\":\"{}\",\"event\":\"{}\",\"tree_path\":\"{}\",\"message\":\"{}\",\"frame_seq\":{},\"last_cmds\":{},\"repeat\":{}}}",
        timestamp_iso8601(),
        escape(app_id),
        escape(phase.as_str()),
        escape(event),
        escape(tree_path),
        escape(message),
        frame_seq,
        cmds,
        repeat
    )
}

/// JSON 字符串转义（只做必需项：引号 / 反斜杠 / 控制字符）。
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// 当前 UTC 时间的 ISO-8601（毫秒精度）：`2026-11-11T20:31:05.123Z`。
///
/// 不引 chrono：只为一行时间戳加一个依赖不值得。civil-from-days 是 Howard Hinnant
/// 的公开算法，1970~2400 年区间内不会错。
pub fn timestamp_iso8601() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let millis = now.subsec_millis();
    let days = secs.div_euclid(86_400);
    let secs_of_day = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        y,
        m,
        d,
        secs_of_day / 3600,
        (secs_of_day % 3600) / 60,
        secs_of_day % 60,
        millis
    )
}

/// 天数（自 1970-01-01）→ (年, 月, 日)。Hinnant 的 `civil_from_days`。
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "kanesumi-errors-{}-{}-{tag}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("建临时目录");
        dir.join("kanesumi-errors.log")
    }

    /// 落盘验证：一条 panic 写成一行 JSON，八个字段齐全。
    #[test]
    fn report_writes_one_json_line_with_all_fields() {
        let path = temp_path("fields");
        set_log_path_for_test(Some(path.clone()));
        let ctx = PanicContext {
            tree_path: Some("/ZStack[0]/Stack[1]/Label[2]".into()),
            last_cmds: vec!["7 条".into(), "首 FillRect(0,0,320,30)".into(), "末 Text(4,4)".into()],
        };
        report(
            Phase::Render,
            "org.ether.settings",
            "-",
            &ctx,
            "index out of bounds: the len is 3 but the index is 5",
            4242,
        );
        let text = std::fs::read_to_string(&path).expect("日志必须落盘");
        set_log_path_for_test(None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 1, "一次 panic 必须只写一行：{text}");
        let line = lines[0];
        for key in [
            "\"ts\":\"",
            "\"app_id\":\"org.ether.settings\"",
            "\"phase\":\"render\"",
            "\"event\":\"-\"",
            "\"tree_path\":\"/ZStack[0]/Stack[1]/Label[2]\"",
            "\"message\":\"index out of bounds: the len is 3 but the index is 5\"",
            "\"frame_seq\":4242",
            "\"last_cmds\":[\"7 条\",\"首 FillRect(0,0,320,30)\",\"末 Text(4,4)\"]",
        ] {
            assert!(line.contains(key), "缺字段 {key}：{line}");
        }
        // ts 形状：2026-11-11T20:31:05.123Z
        let ts_start = line.find("\"ts\":\"").unwrap() + 6;
        let ts = &line[ts_start..ts_start + 24];
        assert!(ts.ends_with('Z') && ts.contains('T'), "时间戳格式不对：{ts}");
    }

    /// 同一 (phase,message) 连续发生 → 折叠成一行，计数递增，不写爆。
    #[test]
    fn consecutive_identical_failures_fold_into_one_line() {
        let path = temp_path("fold");
        set_log_path_for_test(Some(path.clone()));
        let ctx = PanicContext::default();
        for i in 0..5 {
            report(Phase::Update, "org.ether.launcher", "-", &ctx, "boom", i);
        }
        // 换一条消息必须另起一行。
        report(Phase::Update, "org.ether.launcher", "-", &ctx, "bang", 9);
        let text = std::fs::read_to_string(&path).expect("日志必须落盘");
        set_log_path_for_test(None);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());

        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 2, "连续 5 次同键 panic 必须折叠成 1 行：{text}");
        assert!(lines[0].contains("\"repeat\":5"), "折叠计数应为 5：{}", lines[0]);
        assert!(lines[0].contains("\"frame_seq\":4"), "回写后的行应是最新一次：{}", lines[0]);
        assert!(lines[1].contains("\"repeat\":1"), "新消息计数从 1 开始：{}", lines[1]);
        // 折叠后行里不能留下上一行的尾巴（旧的 frame_seq 数字不得残留）。
        assert!(!lines[0].contains("\"frame_seq\":0"), "残留了上一行内容：{}", lines[0]);
    }

    /// 默认路径必须是持久目录（不是 /tmp）。
    #[test]
    fn default_path_is_persistent_state_dir() {
        set_log_path_for_test(None);
        // 测试进程的环境里可能有 KANESUMI_ERROR_LOG：有就按它，没有就核 home 路径。
        if std::env::var_os(LOG_ENV).is_none() {
            let p = log_path().expect("HOME 缺失时无法判定");
            assert!(
                p.ends_with(".local/state/ether/kanesumi-errors.log"),
                "默认路径必须落在持久状态目录：{p:?}"
            );
            assert!(!p.starts_with("/tmp"), "不得写 tmpfs：{p:?}");
        }
    }

    #[test]
    fn json_escaping_keeps_line_single_line() {
        let s = escape("a\"b\\c\nd");
        assert!(!s.contains('\n'));
        assert!(s.contains("\\\""));
        assert!(s.contains("\\\\"));
        assert!(s.contains("\\n"));
    }

    #[test]
    fn iso_timestamp_matches_known_epochs() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20_000), (2024, 10, 4));
        let ts = timestamp_iso8601();
        assert_eq!(ts.len(), 24, "ISO-8601 毫秒精度固定 24 字符：{ts}");
        assert!(ts > "2024".to_string());
    }
}
