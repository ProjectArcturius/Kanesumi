//! text_coverage_bench —— 4096 字中英混排段落塑形基准（w1029 text-coverage-cache §2-1）。
//!
//! 取字思路沿 R0 探针（docs/research/runtime_r0/REPORT.md §八-3）：内嵌常用字表
//! （examples/text_corpus.txt，随仓提交）+ U+4E00 起按码点补足，来源拆分如实打印。
//! 真机字体栈（R0 独立复核 §二-2）：Noto CJK Regular + Bold，TTC 内选 SC 字面（裁定 N-40）。
//!
//! 用法：`cargo run --release --offline -p kanesumi-canvas --example text_coverage_bench`
//! 输出 `BENCH_*` / `DIAG_TOTAL` 行供 `tools/perf/text_coverage.sh` 判门；
//! `KANESUMI_TEXT_DIAG=1` 时退出前向 kanesumi-text.log 补一行诊断。
//! 语料只含 CJK 表意字符、ASCII 词与空格，无组合符 / ZWJ，字素数 = 字符数。

use std::hash::{Hash, Hasher};
use std::time::Instant;

use kanesumi_canvas::text::{FontSource, TextEngine, text_diag_flush, text_diag_snapshot};

const PRIMARY: &str = "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc";
const EXTRA: &str = "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc";
const TARGET_CHARS: usize = 4096;

/// 拉丁词表：中英混排的「英」。够短，够常见。
const LATIN_WORDS: &[&str] = &[
    "ether", "wayland", "shaping", "glyph", "cache", "text", "font", "runtime", "compositor",
    "latency", "pixel", "damage",
];

fn build_paragraph() -> (String, usize, usize) {
    // 常用字表去重（表由执行模型凭常用字频书写，非标准表引用——沿 R0 口径如实打印）。
    let mut common: Vec<char> = Vec::new();
    for c in include_str!("text_corpus.txt").chars() {
        if !c.is_whitespace() && !common.contains(&c) {
            common.push(c);
        }
    }
    let mut out = String::with_capacity(TARGET_CHARS * 3);
    let mut common_taken = 0usize;
    let mut latin_taken = 0usize;
    let mut supplemental = 0usize;
    let mut len = 0usize;
    while len < TARGET_CHARS {
        // 每 32 字素落一个拉丁词 + 空格（CJK 为主的混排），其余用 CJK；
        // 表尽则 U+4E00 起按码点补足。
        if len % 32 == 31 {
            out.push_str(LATIN_WORDS[latin_taken % LATIN_WORDS.len()]);
            out.push(' ');
            latin_taken += 1;
            len += LATIN_WORDS[(latin_taken - 1) % LATIN_WORDS.len()].len() + 1;
        } else {
            let c = if common_taken < common.len() {
                common_taken += 1;
                common[common_taken - 1]
            } else {
                supplemental += 1;
                char::from_u32(0x4E00 + (supplemental - 1) as u32).unwrap_or('一')
            };
            out.push(c);
            len += 1;
        }
    }
    // 词插入按 len 跳变，收尾可能越过 4096：按字符截到恰好 TARGET_CHARS。
    // 最后一字用已知缺字，保证两侧确实经过回退选择分支。
    let mut out = out.chars().take(TARGET_CHARS - 1).collect::<String>();
    out.push('\u{10FFFF}');
    (out, common_taken, supplemental)
}

fn median_of(mut xs: Vec<f64>) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).expect("无 NaN"));
    xs[xs.len() / 2]
}

fn main() {
    let (para, common_used, supplemental_used) = build_paragraph();
    let graphemes = para.chars().count(); // 语料无组合符，字素数 = 字符数
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    para.hash(&mut hash);
    println!(
        "CORPUS total_chars={graphemes} common_table={} common_used={common_used} supplemental_used={supplemental_used} hash={:016x}",
        include_str!("text_corpus.txt").chars().filter(|c| !c.is_whitespace()).count(),
        hash.finish(),
    );
    let primary = FontSource {
        path: PRIMARY.into(),
        collection_tag: Some("SC"),
    };
    let extra = FontSource {
        path: EXTRA.into(),
        collection_tag: Some("SC"),
    };
    let mut round_ms: Vec<f64> = Vec::new();
    let (mut parse_total, mut ffg_total) = (0u64, 0u64);
    for round in 0..3 {
        let t = Instant::now();
        let engine = TextEngine::load_stack(&primary, &[extra.clone()]).expect("字体栈加载失败");
        let load_ms = t.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(engine.font_count(), 2, "Regular与Bold的SC字面必须都加载");
        // 只量塑形段的逐字解析，加载期的轴探测不属于每字素重建。
        let (p0, f0) = text_diag_snapshot();
        let t = Instant::now();
        let glyphs = engine.shape_line(&para, 24.0, 0.0);
        let shape_ms = t.elapsed().as_secs_f64() * 1000.0;
        let (p1, f1) = text_diag_snapshot();
        parse_total += p1 - p0;
        ffg_total += f1 - f0;
        round_ms.push(shape_ms);
        println!(
            "BENCH_ROUND round={round} shape_ms={shape_ms:.6} glyphs={} load_ms={load_ms:.6} parse_delta={} ffg_delta={} font_count={}",
            glyphs.len(),
            p1 - p0,
            f1 - f0,
            engine.font_count(),
        );
        if round == 2 {
            for (face, stat) in engine.diag_coverage_stats().iter().enumerate() {
                println!("COVERAGE face={face} ranges={} memory_bytes={} build_us={}",
                         stat.ranges, stat.memory_bytes, stat.build_us);
            }
        }
    }
    println!("BENCH_MEDIAN shape_ms={:.6}", median_of(round_ms.clone()));
    println!("BENCH_ROUNDS_MDetail {}", format_list(&round_ms));
    println!(
        "DIAG_TOTAL face_parse_rebuilds={parse_total} font_for_grapheme_calls={ffg_total}"
    );
    text_diag_flush();
}

fn format_list(xs: &[f64]) -> String {
    xs.iter()
        .map(|x| format!("{x:.1}"))
        .collect::<Vec<_>>()
        .join("/")
}
