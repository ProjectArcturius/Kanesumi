// perf.rs —— harness 帧耗时自记录（默认开启，零交互给 Debian 实装排查）。
//
// 每个表面累计「render_into / raster / commit」三段耗时与帧数；每 10 秒且有新帧时
// 追加一行到持久路径 `~/.local/state/ether/ether-harness-perf.log`（复用 platform
// `write_diag` 的目录约定，非 tmpfs）。固定大小环形缓冲，热路径零分配。
//
// 本模块跨平台，可在 Windows / CI 单测；platform.rs 仅在 Linux 侧喂数据。

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 环形缓冲容量（每段固定样本数；p50/p95/max 由窗口内样本算）。
pub const RING: usize = 128;
/// 追加间隔。
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(10);
/// 日志超过此大小则截断重写。
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;

/// 固定大小环形样本缓冲（i.e. 不分配）。
#[derive(Clone, Copy)]
pub struct Ring {
    samples: [f32; RING],
    len: usize,
    next: usize,
    total: u64,
}

impl Default for Ring {
    fn default() -> Self {
        Self::new()
    }
}

impl Ring {
    pub const fn new() -> Self {
        Self {
            samples: [0.0; RING],
            len: 0,
            next: 0,
            total: 0,
        }
    }

    /// 记录一个样本（毫秒）。
    pub fn record(&mut self, ms: f32) {
        self.samples[self.next] = ms;
        self.next = (self.next + 1) % RING;
        if self.len < RING {
            self.len += 1;
        }
        self.total += 1;
    }

    /// 累计记录过的样本数（含被环形覆盖的）。
    pub fn total(&self) -> u64 {
        self.total
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// 最近一窗内第 `p` 分位（nearest-rank；`p ∈ [0,1]`）。空 → 0。
    pub fn percentile(&self, p: f32) -> f32 {
        if self.len == 0 {
            return 0.0;
        }
        let mut scratch = [0.0f32; RING];
        scratch[..self.len].copy_from_slice(&self.samples[..self.len]);
        let s = &mut scratch[..self.len];
        s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let idx = (((self.len as f32 - 1.0) * p.clamp(0.0, 1.0)).round() as usize).min(self.len - 1);
        s[idx]
    }

    pub fn max(&self) -> f32 {
        self.samples[..self.len].iter().copied().fold(0.0f32, f32::max)
    }
}

/// 单表面三段耗时 + 帧数（一窗）。
#[derive(Default, Clone, Copy)]
pub struct SurfacePerf {
    pub render: Ring,
    pub raster: Ring,
    pub commit: Ring,
    pub frames: u64,
}

impl SurfacePerf {
    pub const fn new() -> Self {
        Self {
            render: Ring::new(),
            raster: Ring::new(),
            commit: Ring::new(),
            frames: 0,
        }
    }

    pub fn record(&mut self, render_ms: f32, raster_ms: f32, commit_ms: f32) {
        self.render.record(render_ms);
        self.raster.record(raster_ms);
        self.commit.record(commit_ms);
        self.frames += 1;
    }

    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }

    /// 清空本窗口（下次续累）。
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// 一行日志：行首 = 进程名 + 角色 + 时间（unix 秒），后接帧数与三段 p50/p95/max。
pub fn format_line(proc: &str, role: &str, surface: &str, p: &SurfacePerf) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let seg = |r: &Ring| {
        format!(
            "{:.2}/{:.2}/{:.2}",
            r.percentile(0.5),
            r.percentile(0.95),
            r.max()
        )
    };
    format!(
        "{proc} {role} t={secs} surface={surface} frames={} render={} raster={} commit={}\n",
        p.frames,
        seg(&p.render),
        seg(&p.raster),
        seg(&p.commit),
    )
}

/// 持久日志路径：`$HOME/.local/state/ether/<name>`（与 `platform::write_diag` 同目录约定）。
pub fn state_log_path(name: &str) -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(Path::new(&home).join(".local/state/ether").join(name))
}

/// 追加日志；文件超 `MAX_LOG_BYTES` 时截断重写（保留本次内容）。目录不存在则创建。
pub fn write_log(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let too_big = std::fs::metadata(path)
        .map(|m| m.len().saturating_add(content.len() as u64) > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_big {
        let _ = std::fs::write(path, content);
    } else {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
            let _ = f.write_all(content.as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_percentiles_are_nearest_rank() {
        let mut r = Ring::new();
        for i in 1..=100 {
            r.record(i as f32);
        }
        assert_eq!(r.total(), 100);
        // nearest-rank: idx = round((99)*p)
        assert_eq!(r.percentile(0.5), 51.0); // round(49.5)=50 → 值 51
        assert_eq!(r.percentile(0.95), 95.0); // round(94.05)=94 → 值 95
        assert_eq!(r.max(), 100.0);
    }

    #[test]
    fn ring_wraps_but_keeps_total() {
        let mut r = Ring::new();
        for i in 0..(RING * 3) {
            r.record(i as f32);
        }
        assert_eq!(r.total(), (RING * 3) as u64, "累计计数含被覆盖样本");
        assert!(!r.is_empty());
        // 窗口内只剩最后 RING 个样本。
        assert_eq!(r.max(), (RING * 3 - 1) as f32);
    }

    #[test]
    fn empty_ring_is_zero() {
        let r = Ring::new();
        assert_eq!(r.percentile(0.5), 0.0);
        assert_eq!(r.max(), 0.0);
        assert!(r.is_empty());
    }

    #[test]
    fn surface_perf_counts_frames() {
        let mut p = SurfacePerf::new();
        assert!(p.is_empty());
        p.record(1.0, 2.0, 3.0);
        p.record(4.0, 5.0, 6.0);
        assert_eq!(p.frames, 2);
        assert_eq!(p.render.max(), 4.0);
        assert_eq!(p.raster.max(), 5.0);
        assert_eq!(p.commit.max(), 6.0);
        p.reset();
        assert!(p.is_empty());
        assert_eq!(p.frames, 0);
    }

    #[test]
    fn format_line_has_proc_role_time_and_fields() {
        let mut p = SurfacePerf::new();
        p.record(1.0, 2.0, 3.0);
        let line = format_line("ether-topbar", "TopBar", "main", &p);
        assert!(line.starts_with("ether-topbar TopBar t="), "行首进程 + 角色 + 时间：{line}");
        assert!(line.contains("surface=main"));
        assert!(line.contains("frames=1"));
        assert!(line.contains("render=1.00/1.00/1.00"));
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn write_log_truncates_over_cap() {
        let dir = std::env::temp_dir().join(format!("kperf-perf-{}", std::process::id()));
        let path = dir.join("ether-harness-perf.log");
        let _ = std::fs::remove_file(&path);
        // 先灌一个超过上限的文件，再一次小追加 → 截断重写为仅本次内容。
        let big = "x".repeat((MAX_LOG_BYTES + 10) as usize);
        write_log(&path, &big);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), big.len() as u64);
        write_log(&path, "line\n");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "line\n");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
