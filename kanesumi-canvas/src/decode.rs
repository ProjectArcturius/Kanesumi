// decode.rs —— Kanesumi 统一后台解码服务（C3）。
//
// 契约（参 docs/CANVAS_PLAN.md §Ⅴ 规则 1）：**paint 里不解码、不读盘、不光栅 SVG**。
// 绘制代码只查本服务缓存：命中就画；未命中则提交后台请求、本帧画占位（尺寸照旧占位，不跳），
// 就绪后由消费方经元素树既有失效机制令该节点重画（`kanesumi-element` 的 `Image` 即如此）。
// 对照 Windows：WIC / XAML `BitmapImage` 异步解码，好了才显示。
//
// 结构（进程内单例 `global()`，懒启动）：
//   · 工作线程池 2..=4（`available_parallelism` 封顶 4，下限 2）；
//   · LRU 缓存按**字节**计上限，默认 64 MiB（`ETHER_DECODE_CACHE_MB` 可覆盖）；
//   · 同键在途请求合并（`pending`），不重复解码；
//   · **失败结果也缓存**（缺失 / 损坏文件不反复重试、不反复读盘）；
//   · `poll_ready()` 下拉式就绪通知（谁提交谁取）；`wait_idle()` 供测试与首帧自证。
//
// 性能自证（参 §Ⅴ 规则 4）：`enable_perf_log(proc)` 之后，每 10 s 追加一行到
// `~/.local/state/ether/ether-harness-perf.log`（与 kanesumi-harness `perf.rs` 同格式风格：
// `proc 角色 t=秒 … 字段=…`）。未启用时**一行不写** —— 测试进程不污染真机日志。

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::icon::{Icon, ImageKind, rasterize_image, rasterize_image_bytes, rasterize_svg};

/// 默认缓存上限（字节，64 MiB）。图标 / 缩略图量级下足够；壁纸级单图按需覆盖。
pub const DEFAULT_CACHE_BYTES: u64 = 64 * 1024 * 1024;
/// 日志追加间隔（与 harness perf 一致）。
pub const FLUSH_INTERVAL: Duration = Duration::from_secs(10);
/// 日志超过此大小则轮转（重命名为 `.1`）。
pub const MAX_LOG_BYTES: u64 = 1024 * 1024;
/// 解码耗时样本窗口上限（p50/p95/max 用；超出丢最旧）。
pub const LATENCY_WINDOW: usize = 512;

/// 解码产物：位图 + （管线提供时的）源图像素尺寸。
///
/// `source` 给「缩略图 + 原图信息」这类消费方用（Librarian 信息表要显示原图尺寸）；
/// 按目标直接光栅的管线（SVG 最长边）没有「源图」概念，为 `None`。
#[derive(Debug, Clone, PartialEq)]
pub struct DecodeOutput {
    pub icon: Icon,
    /// 缩放到目标之前的源图像素尺寸；未知为 `None`。
    pub source: Option<(u32, u32)>,
}

impl DecodeOutput {
    pub fn new(icon: Icon) -> Self {
        Self { icon, source: None }
    }

    pub fn with_source(icon: Icon, width: u32, height: u32) -> Self {
        Self {
            icon,
            source: Some((width, height)),
        }
    }
}

/// 缓存键：**管线标签** + 源标识 + 目标物理尺寸。
///
/// 管线标签必须区分像素语义不同的管线（例如 Ether 图标链与 Kanesumi 统一入口对
/// 非等比 SVG 的落点不同）—— 同一标签下同键必须像素等价，否则缓存会串味。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DecodeKey {
    pub op: &'static str,
    /// 源标识：文件路径（`to_string_lossy`）或字节摘要。
    pub source: String,
    /// 目标物理尺寸 `(宽, 高)`；一维语义（最长边）时高记 0。
    pub target: (u32, u32),
}

impl DecodeKey {
    /// `rasterize_svg` 语义：按最长边像素等比光栅。
    pub fn svg_longest(path: &Path, px: u32) -> Self {
        Self {
            op: "svg-longest",
            source: path.to_string_lossy().into_owned(),
            target: (px, 0),
        }
    }

    /// `rasterize_image` 语义：按扩展名分派，`target` 为输出物理尺寸框。
    pub fn image(path: &Path, target: Option<(u32, u32)>) -> Self {
        Self {
            op: "image",
            source: path.to_string_lossy().into_owned(),
            target: target.unwrap_or((0, 0)),
        }
    }

    /// 调用方自有管线：`tag` 区分管线，`source` 由调用方给出（须能唯一标识内容）。
    pub fn custom(tag: &'static str, source: impl Into<String>, target: (u32, u32)) -> Self {
        Self {
            op: tag,
            source: source.into(),
            target,
        }
    }

    /// 内存字节：摘要 + 长度（同内容同键，不同内容几乎不可能碰撞）。
    pub fn bytes(kind: ImageKind, data: &[u8]) -> Self {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut h);
        Self {
            op: "bytes",
            source: format!("{:?}:{}:{:x}", kind, data.len(), h.finish()),
            target: (0, 0),
        }
    }
}

/// 缓存查询结果（一次查表区分四种状态，避免「未命中」与「已失败」混淆）。
#[derive(Debug, Clone)]
pub enum Peek {
    /// 既未缓存也不在途 —— 应当提交请求。
    Missing,
    /// 已提交、后台解码中 —— 应当继续等（占位）。
    Pending,
    /// 已就绪。
    Ready(Arc<DecodeOutput>),
    /// 已解码但失败（缺失 / 损坏）—— 已缓存，**不要反复重试**。
    Failed,
}

/// 后台任务：在工作线程上执行，`None` = 解码失败（负面结果同样入缓存）。
pub type Job = Arc<dyn Fn() -> Option<DecodeOutput> + Send + Sync>;

/// 统计快照（性能自证 / 测试断言用）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct DecodeStats {
    /// 提交过的请求数（含去重后丢弃的在途重复）。
    pub requests: u64,
    /// 提交时命中缓存的次数。
    pub hits: u64,
    /// 实际执行过的解码次数（含失败）。
    pub decodes: u64,
    /// 失败的解码次数。
    pub failures: u64,
    /// 缓存占用字节。
    pub cache_bytes: u64,
    /// 缓存条目数。
    pub cache_entries: u64,
    /// 在途请求数。
    pub pending: u64,
    /// 本窗口解码耗时 p50/p95/max（毫秒）。
    pub latency: (f32, f32, f32),
}

struct Entry {
    /// `None` = 已确认失败（负面缓存）。
    out: Option<Arc<DecodeOutput>>,
    bytes: u64,
    /// LRU 序号（命中 / 插入时取新值）。
    seq: u64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<DecodeKey, Entry>,
    pending: HashSet<DecodeKey>,
    /// 已完成、待消费方取走的键（下拉式通知）。
    ready: Vec<DecodeKey>,
    bytes: u64,
    seq: u64,
}

impl Inner {
    /// 插入结果并做 LRU 淘汰（按字节；至少保留一条，避免单个超限条目反复解码）。
    fn insert(&mut self, key: DecodeKey, out: Option<Arc<DecodeOutput>>, cap: u64) {
        let bytes = out
            .as_ref()
            .map(|o| o.icon.rgba.len() as u64)
            .unwrap_or(0);
        self.seq += 1;
        let seq = self.seq;
        if let Some(old) = self.map.insert(key, Entry { out, bytes, seq }) {
            self.bytes = self.bytes.saturating_sub(old.bytes);
        }
        self.bytes += bytes;
        self.trim(cap);
    }

    /// 按字节上限淘汰最久未用者（先淘有字节的条目）。至少保留一条 ——
    /// 单个超限条目（大壁纸）若被自己淘掉就会反复解码。
    fn trim(&mut self, cap: u64) {
        while self.bytes > cap && self.map.len() > 1 {
            let victim = self
                .map
                .iter()
                .filter(|(_, e)| e.bytes > 0)
                .min_by_key(|(_, e)| e.seq)
                .or_else(|| self.map.iter().min_by_key(|(_, e)| e.seq))
                .map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            if let Some(e) = self.map.remove(&victim) {
                self.bytes = self.bytes.saturating_sub(e.bytes);
            }
        }
    }

    fn peek(&mut self, key: &DecodeKey) -> Peek {
        if let Some(e) = self.map.get_mut(key) {
            self.seq += 1;
            e.seq = self.seq;
            return match &e.out {
                Some(o) => Peek::Ready(o.clone()),
                None => Peek::Failed,
            };
        }
        if self.pending.contains(key) {
            return Peek::Pending;
        }
        Peek::Missing
    }
}

/// 统一后台解码服务。全局单例见 [`global`]。
pub struct DecodeService {
    inner: Mutex<Inner>,
    /// `pending` 变空时唤醒 `wait_idle`。
    idle: Condvar,
    tx: OnceLock<Sender<(DecodeKey, Job)>>,
    /// 缓存上限（字节）；`set_cache_cap_bytes` 可改（测试用小值验证淘汰）。
    cap: AtomicU64,
    requests: AtomicU64,
    hits: AtomicU64,
    decodes: AtomicU64,
    failures: AtomicU64,
    /// 已落盘的累计请求数（判断本窗口是否有新活动）。
    flushed_requests: AtomicU64,
    /// 解码耗时窗口（毫秒）；监视线程每 10 s 消费一次并清空。
    latency: Mutex<Vec<f32>>,
    /// 落盘进程名；`None` = 不写日志（测试进程默认如此）。
    perf_proc: Mutex<Option<&'static str>>,
    /// 解码完成通知钩子（进程内一个钩子，工作线程完成一项即调用）。
    ready_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl Default for DecodeService {
    fn default() -> Self {
        Self::new()
    }
}

impl DecodeService {
    pub fn new() -> Self {
        let cap = std::env::var("ETHER_DECODE_CACHE_MB")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(|mb| mb.saturating_mul(1024 * 1024))
            .unwrap_or(DEFAULT_CACHE_BYTES);
        Self {
            inner: Mutex::new(Inner::default()),
            idle: Condvar::new(),
            tx: OnceLock::new(),
            cap: AtomicU64::new(cap),
            requests: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            decodes: AtomicU64::new(0),
            failures: AtomicU64::new(0),
            flushed_requests: AtomicU64::new(0),
            latency: Mutex::new(Vec::new()),
            perf_proc: Mutex::new(None),
            ready_hook: Mutex::new(None),
        }
    }

    /// 设置解码就绪回调（进程内单个钩子，工作线程完成一项即调用）。参 SMOOTHNESS_PLAN §Ⅲ-3。
    pub fn set_ready_hook(&self, hook: Box<dyn Fn() + Send + Sync>) {
        let arc: Arc<dyn Fn() + Send + Sync> = Arc::from(hook);
        *self.ready_hook.lock().unwrap() = Some(arc);
    }

    /// 是否已安装就绪钩子。
    pub fn has_ready_hook(&self) -> bool {
        self.ready_hook.lock().unwrap().is_some()
    }

    /// 清空就绪回调。
    pub fn clear_ready_hook(&self) {
        *self.ready_hook.lock().unwrap() = None;
    }

    /// 缓存上限（字节）。
    pub fn set_cache_cap_bytes(&self, bytes: u64) {
        self.cap.store(bytes.max(1), Ordering::Relaxed);
        let cap = self.cap.load(Ordering::Relaxed);
        self.inner.lock().unwrap().trim(cap);
    }

    pub fn cache_cap_bytes(&self) -> u64 {
        self.cap.load(Ordering::Relaxed)
    }

    /// 清空缓存与在途登记（测试 / 主题切换后按需重解）。在途任务完成后会重新写入结果。
    pub fn clear(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.map.clear();
        inner.ready.clear();
        inner.bytes = 0;
    }

    /// 查缓存（命中同时刷新 LRU 次序）。
    pub fn peek(&self, key: &DecodeKey) -> Peek {
        self.inner.lock().unwrap().peek(key)
    }

    /// 命中缓存的产物（未命中 / 已失败 / 在途均为 `None`）。
    pub fn get(&self, key: &DecodeKey) -> Option<Arc<DecodeOutput>> {
        match self.peek(key) {
            Peek::Ready(o) => Some(o),
            _ => None,
        }
    }

    /// 提交后台请求。同键已缓存（含失败）或已在途时直接返回（在途合并）。
    ///
    /// 接收者取 `&Arc<Self>`：工作线程需要共享同一实例（[`global`] 返回 `&Arc`，
    /// 一次性实例由调用方 `Arc::new(DecodeService::new())` 持有）。
    pub fn request(self: &Arc<Self>, key: DecodeKey, job: Job) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.map.contains_key(&key) {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return;
            }
            if !inner.pending.insert(key.clone()) {
                return;
            }
        }
        self.ensure_pool();
        let tx = self.tx.get().expect("工作池已启动");
        // 收端已断开不可能（tx 常驻本单例）；忽略发送失败即可。
        let _ = tx.send((key, job));
    }

    /// 同步取：命中即返回；未命中就地解码并写入缓存（失败同样入缓存）。
    ///
    /// **不得在 paint / 每帧路径调用** —— 这是给 update 期一次性准备与「预热未覆盖的兜底」
    /// 用的窄口（参 CANVAS_PLAN §Ⅴ.1）。同键已在后台在途时会重复解一次，两者像素等价。
    pub fn get_or_decode(&self, key: &DecodeKey, job: impl FnOnce() -> Option<DecodeOutput>) -> Option<Arc<DecodeOutput>> {
        match self.peek(key) {
            Peek::Ready(o) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return Some(o);
            }
            // 负面结果同样算命中：不重试（同键反复失败会反复读盘，正是本服务要避免的）。
            Peek::Failed => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                return None;
            }
            Peek::Missing | Peek::Pending => {}
        }
        let started = Instant::now();
        let out = job().map(Arc::new);
        self.record_decode(started.elapsed().as_secs_f32() * 1000.0, out.is_none());
        let mut inner = self.inner.lock().unwrap();
        // 在途的后台任务可能刚写进同样结果：这里以先到者为准（像素等价，覆盖亦无害）。
        inner.insert(key.clone(), out.clone(), self.cap.load(Ordering::Relaxed));
        inner.pending.remove(key);
        inner.ready.push(key.clone());
        self.idle.notify_all();
        drop(inner);
        let hook = self.ready_hook.lock().unwrap().clone();
        if let Some(h) = hook {
            h();
        }
        out
    }

    /// 取走本窗口内已完成的键（下拉式就绪通知；取走后不再重复给出）。
    pub fn poll_ready(&self) -> Vec<DecodeKey> {
        std::mem::take(&mut self.inner.lock().unwrap().ready)
    }

    /// 在途请求数。
    pub fn pending_count(&self) -> usize {
        self.inner.lock().unwrap().pending.len()
    }

    /// 等待全部在途请求结束（测试 / 首帧自证用）。超时返回 false。
    pub fn wait_idle(&self, timeout: Duration) -> bool {
        let start = Instant::now();
        let mut inner = self.inner.lock().unwrap();
        while !inner.pending.is_empty() {
            let left = timeout.saturating_sub(start.elapsed());
            if left.is_zero() {
                return false;
            }
            let (g, _) = self.idle.wait_timeout(inner, left).unwrap();
            inner = g;
        }
        true
    }

    pub fn stats(&self) -> DecodeStats {
        let inner = self.inner.lock().unwrap();
        let lat = self.latency.lock().unwrap();
        DecodeStats {
            requests: self.requests.load(Ordering::Relaxed),
            hits: self.hits.load(Ordering::Relaxed),
            decodes: self.decodes.load(Ordering::Relaxed),
            failures: self.failures.load(Ordering::Relaxed),
            cache_bytes: inner.bytes,
            cache_entries: inner.map.len() as u64,
            pending: inner.pending.len() as u64,
            latency: percentiles(&lat),
        }
    }

    /// 启用性能日志（写 `~/.local/state/ether/ether-harness-perf.log`）。
    /// 未调用时一行不写 —— 测试与临时进程不得污染真机日志。
    pub fn enable_perf_log(self: &Arc<Self>, proc: &'static str) {
        *self.perf_proc.lock().unwrap() = Some(proc);
        self.ensure_pool();
    }

    // ── 常用管线便捷入口（键与任务同源，避免调用点各写各的）────────────────────

    /// 提交 `rasterize_svg` 语义的后台光栅（`Image` 控件 SVG 源用）。
    pub fn request_svg_longest(self: &Arc<Self>, path: impl Into<PathBuf>, px: u32) {
        let path = path.into();
        let key = DecodeKey::svg_longest(&path, px);
        let job_path = path.clone();
        self.request(
            key,
            Arc::new(move || rasterize_svg(&job_path, px).map(DecodeOutput::new)),
        );
    }

    /// `rasterize_svg` 语义的同步取（封装在 update 期一次性准备）。
    pub fn svg_longest(&self, path: &Path, px: u32) -> Option<Arc<DecodeOutput>> {
        let key = DecodeKey::svg_longest(path, px);
        self.get_or_decode(&key, || rasterize_svg(path, px).map(DecodeOutput::new))
    }

    /// 提交 `rasterize_image` 语义（按扩展名分派）的后台解码。
    pub fn request_image(self: &Arc<Self>, path: impl Into<PathBuf>, target: Option<(u32, u32)>) {
        let path = path.into();
        let key = DecodeKey::image(&path, target);
        let job_path = path.clone();
        self.request(
            key,
            Arc::new(move || rasterize_image(&job_path, target).map(DecodeOutput::new)),
        );
    }

    /// 提交内存字节（`ImageKind` 显式）的后台解码。
    pub fn request_bytes(self: &Arc<Self>, kind: ImageKind, data: Arc<[u8]>) -> DecodeKey {
        let key = DecodeKey::bytes(kind, &data);
        let job_data = data.clone();
        self.request(
            key.clone(),
            Arc::new(move || rasterize_image_bytes(&job_data, kind).map(DecodeOutput::new)),
        );
        key
    }

    // ── 内部 ────────────────────────────────────────────────────────────────

    fn record_decode(&self, ms: f32, failed: bool) {
        self.decodes.fetch_add(1, Ordering::Relaxed);
        if failed {
            self.failures.fetch_add(1, Ordering::Relaxed);
        }
        let mut w = self.latency.lock().unwrap();
        if w.len() >= LATENCY_WINDOW {
            w.remove(0);
        }
        w.push(ms);
    }

    /// 启动工作池与监视线程（幂等）。
    fn ensure_pool(self: &Arc<Self>) {
        self.tx.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<(DecodeKey, Job)>();
            let rx = Arc::new(Mutex::new(rx));
            let workers = std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(2)
                .clamp(2, 4);
            for i in 0..workers {
                let rx: Arc<Mutex<Receiver<(DecodeKey, Job)>>> = rx.clone();
                let svc = Arc::clone(self);
                std::thread::Builder::new()
                    .name(format!("kanesumi-decode-{i}"))
                    .spawn(move || worker_loop(&svc, rx))
                    .ok();
            }
            let svc = Arc::clone(self);
            std::thread::Builder::new()
                .name("kanesumi-decode-perf".into())
                .spawn(move || perf_loop(&svc))
                .ok();
            tx
        });
    }

    /// 全局单例（工作池与监视线程按首个请求 / `enable_perf_log` 启动）。
    pub fn global() -> &'static Arc<DecodeService> {
        static GLOBAL: OnceLock<Arc<DecodeService>> = OnceLock::new();
        GLOBAL.get_or_init(|| Arc::new(DecodeService::new()))
    }
}

/// 进程内统一解码服务（见 [`DecodeService`]）。
pub fn global() -> &'static Arc<DecodeService> {
    DecodeService::global()
}

/// 设置解码就绪回调（进程内单个钩子，工作线程完成一项即调用）。
pub fn set_ready_hook(hook: Box<dyn Fn() + Send + Sync>) {
    global().set_ready_hook(hook);
}

/// 下拉式取走全局就绪队列中已完成的键（取走后不再重复给出）。
pub fn poll_ready() -> Vec<DecodeKey> {
    global().poll_ready()
}

/// 是否已安装解码就绪钩子。
pub fn has_ready_hook() -> bool {
    global().has_ready_hook()
}

/// 清除解码就绪钩子。
pub fn clear_ready_hook() {
    global().clear_ready_hook();
}

fn worker_loop(svc: &Arc<DecodeService>, rx: Arc<Mutex<Receiver<(DecodeKey, Job)>>>) {
    loop {
        let job = { rx.lock().unwrap().recv() };
        let Ok((key, job)) = job else { return };
        let started = Instant::now();
        let out = job().map(Arc::new);
        svc.record_decode(started.elapsed().as_secs_f32() * 1000.0, out.is_none());
        let mut inner = svc.inner.lock().unwrap();
        inner.pending.remove(&key);
        inner.insert(key.clone(), out, svc.cap.load(Ordering::Relaxed));
        inner.ready.push(key);
        svc.idle.notify_all();
        drop(inner);
        let hook = svc.ready_hook.lock().unwrap().clone();
        if let Some(h) = hook {
            h();
        }
    }
}

/// 每 10 s 落一行解码性能（仅当 `enable_perf_log` 启用且本窗口有活动）。
fn perf_loop(svc: &Arc<DecodeService>) {
    loop {
        std::thread::sleep(FLUSH_INTERVAL);
        let proc = *svc.perf_proc.lock().unwrap();
        let Some(proc) = proc else {
            svc.latency.lock().unwrap().clear();
            continue;
        };
        let requests = svc.requests.load(Ordering::Relaxed);
        if requests == svc.flushed_requests.load(Ordering::Relaxed) {
            svc.latency.lock().unwrap().clear();
            continue;
        }
        svc.flushed_requests.store(requests, Ordering::Relaxed);
        let lat = std::mem::take(&mut *svc.latency.lock().unwrap());
        let s = svc.stats();
        let line = format_decode_line(proc, &s, &lat);
        write_perf_log(&line);
    }
}

/// 一行解码性能日志（与 harness `perf::format_line` 同格式风格：`proc 角色 t=秒 字段=值`）。
/// `hit_rate` = 命中 / 请求（累计）；`decode` = 本窗口解码耗时 p50/p95/max（毫秒）。
pub fn format_decode_line(proc: &str, s: &DecodeStats, window: &[f32]) -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let rate = if s.requests == 0 {
        0.0
    } else {
        s.hits as f64 / s.requests as f64
    };
    let (p50, p95, max) = percentiles(window);
    format!(
        "{proc} Decode t={secs} surface=decode requests={} hit_rate={rate:.3} decode={p50:.2}/{p95:.2}/{max:.2} cache_bytes={} pending={} failures={}\n",
        s.requests, s.cache_bytes, s.pending, s.failures,
    )
}

/// 追加解码性能日志；超 1 MiB 轮转为 `.1`。目录不存在则创建。
fn write_perf_log(line: &str) {
    let Ok(home) = std::env::var("HOME") else { return };
    let path = Path::new(&home)
        .join(".local/state/ether")
        .join("ether-harness-perf.log");
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

/// nearest-rank 百分位（与 harness `Ring::percentile` 同法）。空 → 全零。
pub fn percentiles(samples: &[f32]) -> (f32, f32, f32) {
    if samples.is_empty() {
        return (0.0, 0.0, 0.0);
    }
    let mut s = samples.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let at = |p: f32| -> f32 {
        let idx = (((s.len() as f32 - 1.0) * p.clamp(0.0, 1.0)).round() as usize).min(s.len() - 1);
        s[idx]
    };
    (at(0.5), at(0.95), s[s.len() - 1])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    /// 纯色小图（字节数 = w*h*4，LRU 用例据此定上限）。
    fn solid(w: u32, h: u32) -> DecodeOutput {
        DecodeOutput::new(Icon {
            rgba: Arc::from(vec![0u8; (w * h * 4) as usize].into_boxed_slice()),
            width: w,
            height: h,
        })
    }

    fn counter() -> Arc<AtomicU32> {
        Arc::new(AtomicU32::new(0))
    }

    /// 同一键重复提交：在途合并 + 就绪后命中 —— 解码只跑一次。
    #[test]
    fn same_key_requests_are_merged() {
        let svc = Arc::new(DecodeService::new());
        let calls = counter();
        let key = DecodeKey::custom("test", "merge", (0, 0));
        for _ in 0..3 {
            let c = Arc::clone(&calls);
            svc.request(
                key.clone(),
                Arc::new(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                    std::thread::sleep(Duration::from_millis(30));
                    Some(solid(2, 2))
                }),
            );
        }
        assert!(svc.wait_idle(Duration::from_secs(5)), "在途请求应全部结束");
        assert_eq!(calls.load(Ordering::SeqCst), 1, "同键在途应合并为一次解码");
        assert!(matches!(svc.peek(&key), Peek::Ready(_)));
        // 已就绪后再提交：命中缓存，不再入队。
        let c = Arc::clone(&calls);
        svc.request(
            key.clone(),
            Arc::new(move || {
                c.fetch_add(1, Ordering::SeqCst);
                Some(solid(2, 2))
            }),
        );
        let s = svc.stats();
        assert_eq!(s.hits, 1, "第三次提交命中缓存");
        assert_eq!(s.decodes, 1);
        assert_eq!(s.pending, 0);
    }

    /// LRU 按**字节**淘汰：超上限先淘最久未用；命中刷新次序。
    #[test]
    fn lru_evicts_least_recently_used_by_bytes() {
        let svc = Arc::new(DecodeService::new());
        svc.set_cache_cap_bytes(128); // 每张 4×4 = 64 字节，恰好放两张
        let key = |name: &'static str| DecodeKey::custom("test", name, (0, 0));
        let (a, b, c, d) = (key("a"), key("b"), key("c"), key("d"));
        svc.get_or_decode(&a, || Some(solid(4, 4)));
        svc.get_or_decode(&b, || Some(solid(4, 4)));
        assert!(matches!(svc.peek(&b), Peek::Ready(_)), "容量未超不淘汰");
        svc.get_or_decode(&c, || Some(solid(4, 4)));
        assert!(matches!(svc.peek(&a), Peek::Missing), "最久未用的 a 被淘汰");
        assert!(svc.stats().cache_bytes <= 128);
        // 命中 b 两次刷新次序 → 插入 d 时被淘的是 c。
        let _ = svc.peek(&b);
        let _ = svc.peek(&b);
        svc.get_or_decode(&d, || Some(solid(4, 4)));
        assert!(matches!(svc.peek(&b), Peek::Ready(_)), "最近用过的 b 保留");
        assert!(matches!(svc.peek(&c), Peek::Missing), "次序已刷新 → 淘 c");
    }

    /// 失败结果同样入缓存：同键不反复解码、不反复读盘。
    #[test]
    fn failures_are_cached_and_not_retried() {
        let svc = Arc::new(DecodeService::new());
        let calls = counter();
        let key = DecodeKey::custom("test", "missing", (0, 0));
        for _ in 0..3 {
            let c = Arc::clone(&calls);
            svc.request(
                key.clone(),
                Arc::new(move || {
                    c.fetch_add(1, Ordering::SeqCst);
                    None
                }),
            );
        }
        assert!(svc.wait_idle(Duration::from_secs(5)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(matches!(svc.peek(&key), Peek::Failed), "失败应被记住");
        assert_eq!(svc.stats().failures, 1);
        // 再次提交：走缓存（失败）分支，不解码。
        let c = Arc::clone(&calls);
        svc.request(key.clone(), Arc::new(move || { c.fetch_add(1, Ordering::SeqCst); None }));
        assert_eq!(calls.load(Ordering::SeqCst), 1, "失败不重试");
        // 同步取同样吃到失败缓存。
        assert!(svc.get_or_decode(&key, || Some(solid(1, 1))).is_none());
    }

    /// 真解码路径：SVG 最长边 + 就绪下拉通知。
    #[test]
    fn svg_longest_decodes_and_announces_ready() {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("kanesumi_decode_{}.svg", std::process::id()));
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                     <rect width="24" height="24" fill="#E57812"/></svg>"##;
        std::fs::write(&path, svg).unwrap();
        let svc = Arc::new(DecodeService::new());
        svc.request_svg_longest(path.clone(), 32);
        assert!(svc.wait_idle(Duration::from_secs(5)));
        let key = DecodeKey::svg_longest(&path, 32);
        let out = svc.get(&key).expect("应解码成功");
        assert_eq!((out.icon.width, out.icon.height), (32, 32));
        assert!(svc.poll_ready().contains(&key), "就绪键应可下拉取走");
        assert!(svc.poll_ready().is_empty(), "取走后不再重复给出");
        // 同步入口命中同一缓存。
        let again = svc.svg_longest(&path, 32).expect("命中缓存");
        assert!(svc.stats().hits >= 1);
        assert_eq!(again.icon.width, 32);
        let _ = std::fs::remove_file(&path);
    }

    /// 日志行含任务书要求的字段（同 harness `perf` 风格）。
    #[test]
    fn decode_log_line_has_required_fields() {
        let stats = DecodeStats {
            requests: 8,
            hits: 2,
            decodes: 6,
            failures: 1,
            cache_bytes: 4096,
            cache_entries: 3,
            pending: 0,
            latency: (0.0, 0.0, 0.0),
        };
        let line = format_decode_line("ether-launcher", &stats, &[1.0, 2.0, 9.0]);
        assert!(line.starts_with("ether-launcher Decode t="), "{line}");
        assert!(line.contains("requests=8"), "{line}");
        assert!(line.contains("hit_rate=0.250"), "{line}");
        assert!(line.contains("decode=2.00/9.00/9.00"), "{line}");
        assert!(line.contains("cache_bytes=4096"), "{line}");
        assert!(line.ends_with('\n'));
        assert_eq!(percentiles(&[]), (0.0, 0.0, 0.0));
    }

    /// 解码就绪钩子触发测试（参 SMOOTHNESS_PLAN §Ⅲ-3、裁定 §122）。
    #[test]
    fn ready_hook_called_on_decode_completion() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let svc = Arc::new(DecodeService::new());
        assert!(!svc.has_ready_hook());

        static HOOK_CALLS: AtomicUsize = AtomicUsize::new(0);
        HOOK_CALLS.store(0, Ordering::SeqCst);
        svc.set_ready_hook(Box::new(|| {
            HOOK_CALLS.fetch_add(1, Ordering::SeqCst);
        }));
        assert!(svc.has_ready_hook());

        let key = DecodeKey::custom("test", "hook_key", (10, 10));
        let out = svc.get_or_decode(&key, || {
            Some(DecodeOutput::new(Icon {
                rgba: vec![0xff; 400].into(),
                width: 10,
                height: 10,
            }))
        });
        assert!(out.is_some());
        assert_eq!(HOOK_CALLS.load(Ordering::SeqCst), 1, "get_or_decode 完成应触发钩子");

        svc.clear_ready_hook();
        assert!(!svc.has_ready_hook());
    }
}
