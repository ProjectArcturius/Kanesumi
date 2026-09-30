// font_mem —— 字体加载的内存 / 耗时探针（KANESUMI_RUNTIME.md R1 的基线数据）。
//
// 用法（Linux）：cargo run --release -p kanesumi-canvas --example font_mem -- <字体路径>
// 输出加载前后的 RSS（私有 + 共享）与耗时。每个 Kanesumi 进程都付一次这份代价。

use std::time::Instant;

fn rss() -> (u64, u64) {
    // (/proc/self/status VmRSS KiB, RssFile KiB)
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let get = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0)
    };
    (get("VmRSS:"), get("RssFile:"))
}

fn main() {
    let path = std::env::args().nth(1).expect("用法: font_mem <字体路径>");
    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    let (r0, f0) = rss();
    let t = Instant::now();
    let engine = kanesumi_canvas::text::TextEngine::load(&path).expect("加载失败");
    let dt = t.elapsed();
    let (r1, f1) = rss();
    // 触发一次排版，确认可用。
    let w = engine.measure("中文 English 混排", 15.0);
    println!("font={path} file={:.1}MiB", size as f64 / 1048576.0);
    println!(
        "load={:.0}ms  VmRSS +{:.1}MiB (file-backed +{:.1}MiB)  measure_ok={}",
        dt.as_secs_f64() * 1000.0,
        (r1 - r0) as f64 / 1024.0,
        (f1 as f64 - f0 as f64) / 1024.0,
        w > 0.0
    );
}
