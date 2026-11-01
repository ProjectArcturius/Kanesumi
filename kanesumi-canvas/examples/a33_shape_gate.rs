//! A3.3 塑形门样例驱动（任务 rt-a33-shape-gate，只量不修）。
//!
//! 按生产同款系统字体栈（`text::system_font_chain` → `TextEngine::load_stack`）载入，
//! 对固定 CJK 段落样例（examples/a33_sample_text.txt，include_str! 入库）做两相计量：
//! - **塑形相**：`shape_line` 对样例的轮换窗口排版（窗口轮换破 shape 缓存，保证每遍都是
//!   真实塑形；同文重复会命中 `shape_cache`，量出来的就不是塑形 CPU）。这一相驱动
//!   逐字素覆盖查询（`font_for_grapheme` / `has_glyph`），是 A3.3 两条门的计量对象：
//!   font_for_grapheme_calls > 0；「载入完成后」Face::parse 增量 = 0。
//! - **光栅相**：对各批塑形结果逐字形 `rasterize_glyph`（RGBA 喂 FNV-1a 64 哈希）。
//!   光栅路径每次调用经 `parser()` 重建解析面（text.rs 既有行为），与覆盖查询无关，
//!   增量单列只如实报告、不进门。两相**先后**执行并各自快照，parse 增量不互相混算。
//!
//! 计数全部复用 text.rs 现有 pub 读取接口（`text_diag_snapshot` / `face_parse_calls`，
//! 不新增计数语义）；CPU 取 /proc/self/stat 的 utime+stime（进程 CPU，非墙钟）。
//! 输出：stdout 末行 `A33_RESULT {json}`（门脚本解析）；退出码 0 正常 / 3 字体链不可用。

use kanesumi_canvas::text::{system_font_chain, TextEngine};

/// 塑形相每批遍数（窗口轮换 × 间距微步共同破 shape 缓存；每遍真实塑形）。
const SHAPES_PER_BATCH: usize = 400;
/// 批数（中位 / p90 的样本量）。
const BATCHES: usize = 9;
/// 光栅相每批对当批末次塑形结果的光栅遍数。
const RASTER_PASSES_PER_BATCH: usize = 2;
/// 塑形窗口长度（逻辑字符）。
const WINDOW_CHARS: usize = 120;
/// 窗口起点步进。
const WINDOW_STRIDE: usize = 11;
const TEXT_PX: f32 = 18.0;

fn fnv_feed(bytes: &[u8], hash: &mut u64) {
    for &b in bytes {
        *hash ^= u64::from(b);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

/// 进程 CPU 时间（ms）＝ /proc/self/stat 的 utime+stime（字段 14/15；CLK_TCK=100 → 10ms 粒度）。
fn process_cpu_ms() -> u64 {
    let stat = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let Some(rest) = stat.rsplit_once(')').map(|(_, r)| r) else {
        return 0;
    };
    let f: Vec<&str> = rest.split_whitespace().collect();
    let utime: u64 = f.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
    let stime: u64 = f.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
    (utime + stime) * 10
}

fn percentile(mut v: Vec<u64>, p: f64) -> u64 {
    if v.is_empty() {
        return 0;
    }
    v.sort_unstable();
    let idx = ((v.len() as f64 * p).ceil() as usize).clamp(1, v.len());
    v[idx - 1]
}

fn main() {
    let chain = system_font_chain();
    if chain.is_empty() {
        eprintln!("A33-ERR 会话字体链为空（无系统字体）");
        std::process::exit(3);
    }
    let engine = TextEngine::load_stack(&chain[0], &chain[1..]).expect("生产同款字体栈必须可加载");

    let sample: Vec<char> = include_str!("a33_sample_text.txt").chars().collect();
    assert!(
        sample.len() > WINDOW_CHARS,
        "样例过短（需 > {} 字符，实得 {}）",
        WINDOW_CHARS,
        sample.len()
    );

    // 载入基线（「载入完成后」单列；两相只看各自增量）。
    let (rebuilds_base, ffg_base) = kanesumi_canvas::text::text_diag_snapshot();
    let parse_base = kanesumi_canvas::text::face_parse_calls();

    // ── 塑形相（先全部跑完）：轮换窗口的真实塑形（缓存失效，逐遍走覆盖查询）──
    let mut per_batch_glyphs: Vec<Vec<kanesumi_canvas::text::ShapedGlyph>> = Vec::new();
    let mut shape_cpu: Vec<u64> = Vec::new();
    for batch in 0..BATCHES {
        let c0 = process_cpu_ms();
        let mut last_glyphs = Vec::new();
        for i in 0..SHAPES_PER_BATCH {
            // 窗口轮换 × 间距微步：ShapeKey 含 (text, size, spacing, weight)，两者任一
            // 变化即缓存失效；间距步长 1e-4em 对字形内容的扰动可忽略（哈希仍须稳定）。
            let seq = batch * SHAPES_PER_BATCH + i;
            let off = seq * WINDOW_STRIDE % (sample.len() - WINDOW_CHARS);
            let win: String = sample[off..off + WINDOW_CHARS].iter().collect();
            let spacing = (seq % 64) as f32 * 1e-4;
            last_glyphs = engine.shape_line(&win, TEXT_PX, spacing);
        }
        per_batch_glyphs.push(last_glyphs);
        shape_cpu.push(process_cpu_ms() - c0);
    }
    // 塑形相边界快照：三个门指标只覆盖塑形相；光栅相增量单列，不混算。
    let (rebuilds_mid, ffg_delta) = kanesumi_canvas::text::text_diag_snapshot();
    let parse_mid = kanesumi_canvas::text::face_parse_calls();
    let ffg_delta = ffg_delta - ffg_base;
    let shape_parse_delta = parse_mid - parse_base;
    let shape_rebuilds_delta = rebuilds_mid - rebuilds_base;

    // ── 光栅相（后全部跑完）：各批塑形结果逐字形光栅，RGBA 进哈希 ──
    let mut rgba_hash: u64 = 0xcbf29ce484222325;
    let mut glyph_rasters = 0usize;
    let mut raster_cpu: Vec<u64> = Vec::new();
    for glyphs in &per_batch_glyphs {
        let c0 = process_cpu_ms();
        for _ in 0..RASTER_PASSES_PER_BATCH {
            for g in glyphs {
                let (_metrics, rgba) = engine.rasterize_glyph(g.font_id, g.glyph_id, TEXT_PX);
                fnv_feed(&rgba, &mut rgba_hash);
                glyph_rasters += 1;
            }
        }
        raster_cpu.push(process_cpu_ms() - c0);
    }
    let raster_parse_delta = kanesumi_canvas::text::face_parse_calls() - parse_mid;

    let shape_med = percentile(shape_cpu.clone(), 0.5);
    let shape_p90 = percentile(shape_cpu.clone(), 0.9);
    let raster_med = percentile(raster_cpu.clone(), 0.5);
    println!(
        "A33_RESULT {{\"font_for_grapheme_calls\":{},\"shape_parse_delta\":{},\"shape_rebuilds_delta\":{},\"raster_parse_delta\":{},\"shape_cpu_ms_median\":{},\"shape_cpu_ms_p90\":{},\"raster_cpu_ms_median\":{},\"shape_cpu_ms_samples\":{:?},\"raster_cpu_ms_samples\":{:?},\"rgba_hash\":\"{:016x}\",\"glyph_rasters\":{},\"fonts\":{}}}",
        ffg_delta,
        shape_parse_delta,
        shape_rebuilds_delta,
        raster_parse_delta,
        shape_med,
        shape_p90,
        raster_med,
        shape_cpu,
        raster_cpu,
        rgba_hash,
        glyph_rasters,
        engine.font_count(),
    );
}
