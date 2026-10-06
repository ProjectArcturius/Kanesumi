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

/// 单表面三段耗时 + GPU 耗时 + 帧数（一窗）。
#[derive(Default, Clone, Copy)]
pub struct SurfacePerf {
    pub render: Ring,
    pub raster: Ring,
    pub commit: Ring,
    /// GPU 时间戳实测（毫秒；仅 WGPU 路径且设备支持时间戳查询时有样本）。
    /// 空环 = 不支持 / 尚未回读到 → 日志写 `gpu=n/a`。
    pub gpu: Ring,
    pub frames: u64,
}

impl SurfacePerf {
    pub const fn new() -> Self {
        Self {
            render: Ring::new(),
            raster: Ring::new(),
            commit: Ring::new(),
            gpu: Ring::new(),
            frames: 0,
        }
    }

    pub fn record(&mut self, render_ms: f32, raster_ms: f32, commit_ms: f32) {
        self.render.record(render_ms);
        self.raster.record(raster_ms);
        self.commit.record(commit_ms);
        self.frames += 1;
    }

    /// 记一个已回读到的 GPU 帧耗时（异步、滞后数帧，故与 `record` 分开调用）。
    pub fn record_gpu(&mut self, gpu_ms: f32) {
        self.gpu.record(gpu_ms);
    }

    pub fn is_empty(&self) -> bool {
        self.frames == 0
    }

    /// 清空本窗口（下次续累）。
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// 一行日志：行首 = 进程名 + 角色 + 时间（unix 秒），后接帧数与 render / raster / commit /
/// gpu 四段的 p50/p95/max。GPU 无样本（设备不支持时间戳查询，或尚未回读）写 `gpu=n/a`。
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
    let gpu = if p.gpu.is_empty() { "n/a".to_string() } else { seg(&p.gpu) };
    format!(
        "{proc} {role} t={secs} surface={surface} frames={} render={} raster={} commit={} gpu={gpu}\n",
        p.frames,
        seg(&p.render),
        seg(&p.raster),
        seg(&p.commit),
    )
}

/// 进程启动自证行（每进程一次性写在 perf 日志首行）：实际 MSAA 采样数与 GPU 计时是否开启。
/// `msaa = None` 表示本进程无 GPU 光栅器（layer-shell / CPU 光栅角色）。
/// 参 Ether docs/research/gpu_t1（任务 gpu-t1：MSAA 4 vs 1 与 GPU 帧耗时的 A/B 必须能从日志分辨档位）。
pub fn format_header(msaa_samples: Option<u32>, gpu_supported: bool) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let msaa = msaa_samples
        .map(|n| n.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let gpu = if gpu_supported { "ts" } else { "n/a" };
    format!("# kanesumi-harness t={secs} msaa={msaa} gpu={gpu}\n")
}

// ── GPU 时间戳环形缓冲的索引与换算（纯函数，单测覆盖）──
//
// 每帧写一对起止时间戳到第 `frame % SLOTS` 槽；回读滞后 [`TIMESTAMP_LAG`] 帧，
// 使查询结果有足够时间由 GPU 落定，**绝不阻塞当帧**。参 Ether docs/GPU_COMPOSITION_PLAN.md §Ⅲ。

/// 时间戳查询槽位数（每槽一对起止时间戳 + 一份回读缓冲）。
pub const TIMESTAMP_SLOTS: u64 = 4;
/// 回读滞后帧数：第 N 帧读第 N-LAG 帧写下的槽。
pub const TIMESTAMP_LAG: u64 = 2;

/// 第 `frame` 帧占用的槽位（纯函数）。
pub fn timestamp_slot(frame: u64, slots: u64) -> usize {
    (frame % slots.max(1)) as usize
}

/// 第 `frame` 帧应当尝试回读的槽位（写在第 `frame - TIMESTAMP_LAG` 帧）。
/// 帧号不足 `lag`（刚启动）→ None。
pub fn readback_slot_for(frame: u64, slots: u64, lag: u64) -> Option<usize> {
    frame.checked_sub(lag).map(|f| timestamp_slot(f, slots))
}

/// 两个 GPU 时间戳之差 → 毫秒。`period_ns` = 每 tick 纳秒数（`Queue::get_timestamp_period`）。
/// `end <= start`（查询无效 / 计数器回绕）→ 0。
pub fn ticks_to_ms(start: u64, end: u64, period_ns: f32) -> f32 {
    if end <= start {
        return 0.0;
    }
    ((end - start) as f64 * period_ns as f64 / 1e6) as f32
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

// ── 动画掉帧计数（落盘）── 参 Ether docs/DECISIONS_2026-10-05.md §106。
//
// 客户端自驱动动画（控件过渡 / 页面导航 / 列表滚动惯性）的验收量同为「动画期间的掉帧数」：
// 相邻两次提交间隔 > 1.5 × 刷新周期即计一次掉帧，只在 `App::needs_redraw()`（动画推进中）计。
// 每段动画结束追加一行到 `~/.local/state/ether/ether-harness-pacing.log`（1 MB 轮转）；
// 与合成器侧同格式，便于两份日志一起判读。

/// 掉帧判定倍率：间隔严格大于 1.5 × 刷新周期才算。
pub const DROP_FACTOR: f32 = 1.5;

/// 刷新周期毫秒（默认 60 Hz；`ETHER_REFRESH_HZ` 可覆盖）。
pub fn refresh_period_ms() -> f32 {
    use std::sync::OnceLock;
    static HZ: OnceLock<f32> = OnceLock::new();
    let hz = *HZ.get_or_init(|| {
        std::env::var("ETHER_REFRESH_HZ")
            .ok()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|h| *h > 0.0)
            .unwrap_or(60.0)
    });
    1000.0 / hz
}

/// 单次提交间隔是否算掉帧（纯函数）。阈值严格大于：恰好 1.5× 不算。
pub fn is_dropped(interval_ms: f32, refresh_ms: f32) -> bool {
    interval_ms > refresh_ms * DROP_FACTOR
}

/// 统计一组相邻提交间隔的掉帧次数（纯函数）。
///
/// 取舍：一次超长间隔（哪怕跨 2 个刷新周期）只计 1 次 —— 验收看「卡了几次」而非「丢了几帧」，
/// 与 §106 一致；等间隔（含恰好 1.5×）不计。
pub fn count_dropped(intervals_ms: &[f32], refresh_ms: f32) -> u32 {
    intervals_ms.iter().filter(|&&d| is_dropped(d, refresh_ms)).count() as u32
}

/// 一段进行中的动画。`trigger` = 触发时刻（动画推进开始）。
#[derive(Debug, Clone)]
struct Activity {
    name: String,
    trigger: std::time::Instant,
    first_present: Option<std::time::Instant>,
    /// 本段动画自己的上一帧时刻（各段独立，避免把空闲期间隔算进新动画首帧）。
    last_present: Option<std::time::Instant>,
    frames: u32,
    drops: u32,
    max_interval_ms: f32,
}

/// 客户端动画掉帧追踪器（纯逻辑，便于单测；运行期由 thread_local 持有）。
#[derive(Debug)]
pub struct FramePacing {
    activities: Vec<Activity>,
    refresh_ms: f32,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self::new()
    }
}

impl FramePacing {
    pub fn new() -> Self {
        Self { activities: Vec::new(), refresh_ms: refresh_period_ms() }
    }

    #[cfg(test)]
    fn with_refresh(refresh_ms: f32) -> Self {
        Self { refresh_ms, ..Self::new() }
    }

    /// 开始一段动画。同名已存在则重置计数（重新起跑）。
    pub fn begin(&mut self, name: &str, trigger: std::time::Instant) {
        if let Some(a) = self.activities.iter_mut().find(|a| a.name == name) {
            a.trigger = trigger;
            a.first_present = None;
            a.last_present = None;
            a.frames = 0;
            a.drops = 0;
            a.max_interval_ms = 0.0;
            return;
        }
        self.activities.push(Activity {
            name: name.to_string(),
            trigger,
            first_present: None,
            last_present: None,
            frames: 0,
            drops: 0,
            max_interval_ms: 0.0,
        });
    }

    /// 声明动画源状态：由假变真开始、由真变假结束（结束返回日志行）。
    pub fn mark(
        &mut self,
        name: &str,
        active: bool,
        trigger: std::time::Instant,
        now: std::time::Instant,
    ) -> Option<String> {
        let registered = self.activities.iter().any(|a| a.name == name);
        match (active, registered) {
            (true, false) => {
                self.begin(name, trigger);
                None
            }
            (false, true) => self.finish(name, now),
            _ => None,
        }
    }

    /// 一帧提交：更新所有进行中动画的帧数 / 掉帧 / 最长间隔。首帧（本段尚无上一帧）不计间隔。
    pub fn on_present(&mut self, now: std::time::Instant) {
        let names: Vec<String> = self.activities.iter().map(|a| a.name.clone()).collect();
        for n in &names {
            self.record_present(n, now);
        }
    }

    /// 指定动画的一帧提交（多表面进程须按表面分别记，否则别的表面的帧会污染间隔）。
    pub fn record_present(&mut self, name: &str, now: std::time::Instant) {
        let refresh_ms = self.refresh_ms;
        for a in &mut self.activities {
            if a.name != name {
                continue;
            }
            if a.first_present.is_none() {
                a.first_present = Some(now);
            }
            a.frames = a.frames.saturating_add(1);
            if let Some(last) = a.last_present {
                let d = now.saturating_duration_since(last).as_secs_f32() * 1000.0;
                if d > a.max_interval_ms {
                    a.max_interval_ms = d;
                }
                if is_dropped(d, refresh_ms) {
                    a.drops = a.drops.saturating_add(1);
                }
            }
            a.last_present = Some(now);
        }
    }

    /// 结束一段动画并返回日志行（未提交过帧时为 None）。
    pub fn finish(&mut self, name: &str, _now: std::time::Instant) -> Option<String> {
        let i = self.activities.iter().position(|a| a.name == name)?;
        let a = self.activities.remove(i);
        let first = a.first_present?;
        let latency = first.saturating_duration_since(a.trigger).as_secs_f32() * 1000.0;
        Some(format_pacing_line(&a.name, latency, a.frames, a.drops, a.max_interval_ms))
    }
}

/// 一行掉帧日志（纯函数）：动画名、起跑延迟、总帧数、掉帧数、最长帧间隔。与合成器同格式。
pub fn format_pacing_line(
    name: &str,
    start_latency_ms: f32,
    frames: u32,
    drops: u32,
    max_interval_ms: f32,
) -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!(
        "t={ms} name=\"{name}\" start_latency_ms={start_latency_ms:.1} frames={frames} drops={drops} max_interval_ms={max_interval_ms:.1}\n"
    )
}

thread_local! {
    static PACING: std::cell::RefCell<FramePacing> = std::cell::RefCell::new(FramePacing::new());
}

/// 开始一段客户端动画（harness 事件循环单线程）。
pub fn pacing_begin(name: &str, trigger: std::time::Instant) {
    PACING.with(|p| p.borrow_mut().begin(name, trigger));
}

/// 声明动画源状态，结束时落盘。
pub fn pacing_mark(name: &str, active: bool, trigger: std::time::Instant, now: std::time::Instant) {
    let line = PACING.with(|p| p.borrow_mut().mark(name, active, trigger, now));
    if let Some(line) = line {
        write_pacing(&line);
    }
}

/// 一帧提交（全部进行中动画；单表面进程可用）。
pub fn pacing_present(now: std::time::Instant) {
    PACING.with(|p| p.borrow_mut().on_present(now));
}

/// 指定动画的一帧提交（多表面进程按表面分别记，避免互相污染间隔）。
pub fn pacing_present_named(name: &str, now: std::time::Instant) {
    PACING.with(|p| p.borrow_mut().record_present(name, now));
}

/// 追加掉帧日志并 1 MB 轮转（现文件改名 `.1`）。
pub fn write_pacing(line: &str) {
    let Some(path) = state_log_path("ether-harness-pacing.log") else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len().saturating_add(line.len() as u64) > MAX_LOG_BYTES)
        .unwrap_or(false);
    if too_big {
        let _ = std::fs::rename(&path, path.with_extension("log.1"));
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = f.write_all(line.as_bytes());
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
        assert!(p.gpu.is_empty(), "reset 一并清 GPU 样本");
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
    fn format_line_gpu_is_n_a_without_samples() {
        let mut p = SurfacePerf::new();
        p.record(1.0, 2.0, 3.0);
        let line = format_line("ether-settings", "Settings", "main", &p);
        assert!(line.contains("gpu=n/a"), "无 GPU 样本写 n/a：{line}");
    }

    #[test]
    fn format_line_gpu_shows_percentiles_with_samples() {
        let mut p = SurfacePerf::new();
        p.record(1.0, 2.0, 3.0);
        for v in [1.0, 2.0, 3.0, 4.0] {
            p.record_gpu(v);
        }
        let line = format_line("ether-settings", "Settings", "main", &p);
        assert!(line.contains("gpu=3.00/4.00/4.00"), "有样本写 p50/p95/max：{line}");
        assert!(!line.contains("gpu=n/a"));
    }

    #[test]
    fn header_records_msaa_and_gpu_support() {
        let h = format_header(Some(1), false);
        assert!(h.starts_with('#'), "首行以 # 开标注：{h}");
        assert!(h.contains("msaa=1"), "{h}");
        assert!(h.contains("gpu=n/a"), "{h}");
        let h = format_header(Some(4), true);
        assert!(h.contains("msaa=4") && h.contains("gpu=ts"), "{h}");
        assert!(h.ends_with('\n'));
        // 无 GPU 光栅器（layer-shell / CPU 角色）→ msaa=n/a。
        let h = format_header(None, false);
        assert!(h.contains("msaa=n/a"), "{h}");
    }

    #[test]
    fn timestamp_slot_cycles_and_readback_lags() {
        // 槽位按帧号取模循环。
        assert_eq!(timestamp_slot(0, TIMESTAMP_SLOTS), 0);
        assert_eq!(timestamp_slot(3, TIMESTAMP_SLOTS), 3);
        assert_eq!(timestamp_slot(4, TIMESTAMP_SLOTS), 0);
        assert_eq!(timestamp_slot(9, TIMESTAMP_SLOTS), 1);
        // 回读滞后 LAG 帧，且与写入槽互不碰撞（LAG ≤ SLOTS）。
        for f in TIMESTAMP_LAG..(TIMESTAMP_LAG + 8) {
            let w = timestamp_slot(f, TIMESTAMP_SLOTS);
            let r = readback_slot_for(f, TIMESTAMP_SLOTS, TIMESTAMP_LAG).unwrap();
            assert_eq!(r, timestamp_slot(f - TIMESTAMP_LAG, TIMESTAMP_SLOTS));
            assert_ne!(r, w, "第 {f} 帧的回读槽不得与写入槽相同");
        }
        // 启动前几帧无可回读槽。
        assert_eq!(readback_slot_for(0, TIMESTAMP_SLOTS, TIMESTAMP_LAG), None);
        assert_eq!(readback_slot_for(1, TIMESTAMP_SLOTS, TIMESTAMP_LAG), None);
        assert_eq!(readback_slot_for(2, TIMESTAMP_SLOTS, TIMESTAMP_LAG), Some(0));
    }

    #[test]
    fn ticks_to_ms_scales_and_guards_wraparound() {
        // 1 tick = 1 ns（多数 Vulkan 驱动）→ 10_000 tick = 0.01 ms。
        assert!((ticks_to_ms(0, 10_000, 1.0) - 0.01).abs() < 1e-6);
        // 1 tick = 1 µs（period=1000 ns）→ 1000 tick = 1 ms。
        assert!((ticks_to_ms(0, 1000, 1000.0) - 1.0).abs() < 1e-4);
        // end ≤ start 视为无效样本（计数器回绕 / 查询未写）→ 0。
        assert_eq!(ticks_to_ms(5, 5, 1.0), 0.0);
        assert_eq!(ticks_to_ms(9, 3, 1.0), 0.0);
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

    #[test]
    fn dropped_none_at_or_below_threshold() {
        let r = 1000.0 / 60.0;
        assert_eq!(count_dropped(&[r, r, r], r), 0);
        assert_eq!(count_dropped(&[r * 1.5], r), 0, "恰好 1.5× 不算");
        assert!(is_dropped(r * 1.5 + 0.01, r));
    }

    #[test]
    fn long_interval_counts_once_not_per_period() {
        let r = 1000.0 / 60.0;
        assert_eq!(count_dropped(&[r * 2.0], r), 1);
        assert_eq!(count_dropped(&[r * 3.0], r), 1, "跨两个周期也只计 1");
        assert_eq!(count_dropped(&[r, r * 2.5, r, r * 4.0], r), 2);
    }

    #[test]
    fn pacing_line_reports_latency_drops_and_max() {
        use std::time::{Duration, Instant};
        let r = 1000.0 / 60.0;
        let mut p = FramePacing::with_refresh(r);
        let t0 = Instant::now();
        p.begin("TopBar:main", t0);
        p.on_present(t0 + Duration::from_millis(12));
        p.on_present(t0 + Duration::from_millis(42));
        p.on_present(t0 + Duration::from_millis(58));
        let line = p.finish("TopBar:main", t0 + Duration::from_millis(58)).unwrap();
        assert!(line.contains("name=\"TopBar:main\""), "{line}");
        assert!(line.contains("start_latency_ms=12.0"), "{line}");
        assert!(line.contains("frames=3"), "{line}");
        assert!(line.contains("drops=1"), "{line}");
        assert!(line.contains("max_interval_ms=30.0"), "{line}");
        assert!(line.ends_with('\n'));
    }

    #[test]
    fn pacing_finish_without_frames_is_none() {
        use std::time::{Duration, Instant};
        let mut p = FramePacing::with_refresh(16.67);
        let t0 = Instant::now();
        p.begin("x", t0);
        assert!(p.finish("x", t0 + Duration::from_millis(5)).is_none());
    }
}
