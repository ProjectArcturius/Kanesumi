// census.rs —— 图像内存归因计数（runtime 轨 B0，只量不修）。
//
// 参 Ether 主仓 tools/dispatch/tasks/w1029/rt-image-census.md §2-2：在图像解码、模糊、
// 亚克力背板的分配点记一行「进程、用途、宽×高、格式、字节数、分配/释放」，把 R0 复核里
// 「36,004 kB 独立匿名段用途未证」的推断变成实测。
//
// 缺省关：`KANESUMI_IMAGE_CENSUS=1` 才启用；关闭时 `record` 只做一次 OnceLock 原子读即
// 返回，不格式化、不 IO。启用后追加写 `~/.local/state/ether/image-census.log`
// （超 1 MiB 轮转为 `.1`，同 decode.rs `write_perf_log` 的做法）。
//
// 只加记录、不改任何分配 / 缓存 / 解码参数 —— 所有接线点均为旁路观察。

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// 日志轮转上限（字节）。
const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// `KANESUMI_IMAGE_CENSUS` 取值语义：仅 `1` / `true` 视为开（与 `ETHER_NOTIFY_STATS` 同口径）。
/// 纯函数抽出便于单测钉住，避免测试碰进程级环境变量。
pub fn parse_enabled(v: Option<&str>) -> bool {
    matches!(v, Some("1") | Some("true"))
}

/// 进程内缓存的开关判定（首次读环境变量，此后零系统调用）。
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| parse_enabled(std::env::var("KANESUMI_IMAGE_CENSUS").ok().as_deref()))
}

/// 事件种类：位图缓冲的产生与归还。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// 分配 / 开始持有。
    Alloc,
    /// 释放 / 缓存淘汰。
    Release,
}

/// 日志路径：`$HOME/.local/state/ether/image-census.log`。HOME 缺失 → None（不写）。
/// 判读脚本以隔离 HOME 跑无头会话，日志落在各自输出目录（tools/perf/image_census.sh）。
fn log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/state/ether").join("image-census.log"))
}

/// 一行 census 记录（字段顺序固定，`k=v` 空格分隔，脚本按此解析）。
/// `thread` 取当前线程名（解码池 / 主线程 / 应用线程即归因线索，任务书 §1-1）。
pub fn format_line(event: Event, use_: &str, fmt: &str, w: u32, h: u32, bytes: usize) -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let ev = match event {
        Event::Alloc => "alloc",
        Event::Release => "release",
    };
    let pid = std::process::id();
    let current = std::thread::current();
    let thread = current.name().unwrap_or("<unnamed>");
    format!("t={t} pid={pid} thread={thread} event={ev} use={use_} fmt={fmt} w={w} h={h} bytes={bytes}\n")
}

/// 记录一条（统一入口；关闭时零开销直返）。
pub fn record(event: Event, use_: &str, fmt: &str, w: u32, h: u32, bytes: usize) {
    if !enabled() {
        return;
    }
    let Some(path) = log_path() else { return };
    append(&path, &format_line(event, use_, fmt, w, h, bytes));
}

// ── 字形 census（A4 决策材料，夜 3 卡；缺省关）────────────────────────────
//
// `KANESUMI_GLYPH_CENSUS=1` 才启用（与图像 census 同一 `parse_enabled` 口径）；关闭时
// `record_glyph` 一次 OnceLock 原子读即返，不格式化、不 IO。启用后追加写
// `~/.local/state/ether/glyph-census.log`（同 1 MiB 轮转）。行：
//   t= pid= path=cpu|gpu font=<栈身份> font_id= glyph= size=<物理px 0.25 粒度> scale= w= h= bytes= event=raster|cache-hit|upload
// 只加旁路记录：不改字形缓存策略、不改光栅参数、不改任何渲染行为。

/// 字形 census 开关判定（复用图像 census 的 `1`/`true` 口径）。
pub fn glyph_enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| parse_enabled(std::env::var("KANESUMI_GLYPH_CENSUS").ok().as_deref()))
}

/// 字形 census 日志路径：`$HOME/.local/state/ether/glyph-census.log`。
fn glyph_log_path() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".local/state/ether").join("glyph-census.log"))
}

/// 字号取整到 0.25 物理 px（判读侧按同值聚合；浮点尾差不再拆分字形）。
fn snap_size(size: f32) -> f32 {
    (size * 4.0).round() / 4.0
}

/// 一条字形 census 行（字段顺序固定；`path` = cpu|gpu 标记产生侧）。
/// 行字段是外部判读契约，保持扁平参数（allow too_many_arguments）。
#[allow(clippy::too_many_arguments)]
pub fn format_line_glyph(
    path: &str,
    font: u64,
    font_id: u32,
    glyph_id: u16,
    size: f32,
    scale: f32,
    w: u32,
    h: u32,
    bytes: usize,
    event: &str,
) -> String {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    format!(
        "t={t} pid={pid} path={path} font={font} font_id={font_id} glyph={glyph_id} \
         size={:.2} scale={scale:.2} w={w} h={h} bytes={bytes} event={event}\n",
        snap_size(size)
    )
}

/// 核心写出行：`enabled`/`path` 参数化便于单测钉住「关时不写文件」。
#[allow(clippy::too_many_arguments)]
fn write_glyph_row(
    enabled: bool,
    path: Option<PathBuf>,
    path_label: &str,
    font: u64,
    font_id: u32,
    glyph_id: u16,
    size: f32,
    scale: f32,
    w: u32,
    h: u32,
    bytes: usize,
    event: &str,
) {
    if !enabled {
        return;
    }
    let Some(path) = path else { return };
    append(
        &path,
        &format_line_glyph(path_label, font, font_id, glyph_id, size, scale, w, h, bytes, event),
    );
}

/// 记录一条字形 census（统一入口；关闭时零开销直返）。
/// `cache-hit` 行 bytes=0（字节只记 raster 行，避免同一位图重复计数）。
#[allow(clippy::too_many_arguments)]
pub fn record_glyph(
    path_label: &str,
    font: u64,
    font_id: u32,
    glyph_id: u16,
    size: f32,
    scale: f32,
    w: u32,
    h: u32,
    bytes: usize,
    event: &str,
) {
    if !glyph_enabled() {
        return;
    }
    write_glyph_row(
        true,
        glyph_log_path(),
        path_label,
        font,
        font_id,
        glyph_id,
        size,
        scale,
        w,
        h,
        bytes,
        event,
    );
}

/// 追加一行；超限先轮转（rename → `.1`）。IO 失败静默——归因日志不得反噬渲染路径。
fn append(path: &std::path::Path, line: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let too_big = std::fs::metadata(path)
        .map(|m| m.len().saturating_add(line.len() as u64) > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_big {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = f.write_all(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 开关语义：仅 `1` / `true` 开；未设 / 空串 / 其他值全关。
    #[test]
    fn parse_enabled_only_accepts_one_or_true() {
        assert!(parse_enabled(Some("1")));
        assert!(parse_enabled(Some("true")));
        assert!(!parse_enabled(Some("0")));
        assert!(!parse_enabled(Some("yes")));
        assert!(!parse_enabled(Some("")));
        assert!(!parse_enabled(None));
    }

    /// 行字段齐全且顺序固定（判读脚本按 `k=v` 逐字段 grep）。
    #[test]
    fn format_line_has_all_fields() {
        let line = format_line(Event::Alloc, "lock-wallpaper", "jpeg", 3840, 2400, 36_864_000);
        assert!(line.starts_with("t="), "单调毫秒时间戳在前: {line}");
        assert!(line.contains(" pid="), "{line}");
        assert!(line.contains(" thread="), "{line}");
        assert!(line.contains(" event=alloc"), "{line}");
        assert!(line.contains(" use=lock-wallpaper"), "{line}");
        assert!(line.contains(" fmt=jpeg"), "{line}");
        assert!(line.contains(" w=3840"), "{line}");
        assert!(line.contains(" h=2400"), "{line}");
        assert!(line.contains(" bytes=36864000"), "{line}");
        assert!(line.ends_with('\n'), "{line}");
        let rel = format_line(Event::Release, "decode-cache", "rgba", 2, 2, 16);
        assert!(rel.contains(" event=release"), "{rel}");
    }

    /// 追加 + 轮转：写两行可读回；超限后改名 `.1`、新文件重新起算。
    #[test]
    fn append_writes_and_rotates() {
        let dir = std::env::temp_dir().join(format!("k-census-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("image-census.log");
        append(&path, "one\n");
        append(&path, "two\n");
        let body = std::fs::read_to_string(&path).unwrap();
        assert_eq!(body, "one\ntwo\n");
        // 触发轮转（阈值按字节累计）。
        let big = "x".repeat(MAX_LOG_BYTES as usize);
        append(&path, &big);
        assert!(dir.join("image-census.log.1").exists(), "超限轮转为 .1");
        let fresh = std::fs::read_to_string(&path).unwrap();
        assert_eq!(fresh, big, "轮转后新文件只含本轮内容");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 字形行字段齐全且顺序固定（size 已取整到 0.25 粒度）。
    #[test]
    fn format_line_glyph_has_all_fields() {
        let line = format_line_glyph("cpu", 0xdeadbeef, 3, 72, 31.98, 2.0, 42, 46, 1932, "raster");
        assert!(line.starts_with("t="), "{line}");
        assert!(line.contains(" pid="), "{line}");
        assert!(line.contains(" path=cpu"), "{line}");
        assert!(line.contains(" font=3735928559"), "{line}");
        assert!(line.contains(" font_id=3"), "{line}");
        assert!(line.contains(" glyph=72"), "{line}");
        assert!(line.contains(" size=32.00"), "{line}");
        assert!(line.contains(" scale=2.00"), "{line}");
        assert!(line.contains(" w=42"), "{line}");
        assert!(line.contains(" h=46"), "{line}");
        assert!(line.contains(" bytes=1932"), "{line}");
        assert!(line.contains(" event=raster"), "{line}");
        assert!(line.ends_with('\n'), "{line}");
    }

    /// 开关关时不写文件（夜 3 卡门：关时零格式化零 IO）。
    #[test]
    fn glyph_row_disabled_writes_nothing() {
        let dir = std::env::temp_dir().join(format!("k-glyph-off-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("glyph-census.log");
        write_glyph_row(
            false,
            Some(path.clone()),
            "cpu",
            1,
            0,
            1,
            16.0,
            1.0,
            8,
            8,
            64,
            "raster",
        );
        assert!(!path.exists(), "关闭时不得创建/写入日志文件");
        // 开时写出行且格式正确。
        write_glyph_row(
            true,
            Some(path.clone()),
            "gpu",
            0x1234,
            2,
            33,
            24.11,
            2.0,
            30,
            34,
            1020,
            "upload",
        );
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains(" path=gpu"), "{body}");
        assert!(body.contains(" event=upload"), "{body}");
        assert!(body.contains(" size=24.00"), "{body}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
