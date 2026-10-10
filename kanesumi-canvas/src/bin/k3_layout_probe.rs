// K3 排版前后对照：同一字体、同一字串、冷排版与热缓存分别计时。
// 参 Ether tools/dispatch/tasks/k-text-shaping.md §要做的事-3。
use std::hint::black_box;
use std::time::Instant;

use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};

const STRINGS: &[&str] = &[
    "系统", "设备", "个性化", "声音", "时间和语言", "隐私", "通知", "显示",
    "存储", "电源", "网络", "连接", "打印机", "用户", "辅助功能", "应用管理",
    "个性化 Chorus", "Wi-Fi", "缩放与 HiDPI", "提权认证 uniauth",
];

fn main() {
    let font = std::env::args().nth(1).expect("需要指定同一字体文件");
    let options = TextLayoutOptions::wrapped(200.0, 40.0, 20.0);
    let mut cold = Vec::new();
    let mut hit = Vec::new();
    for _ in 0..9 {
        // 字体载入不计入排版时长；每轮新引擎确保冷排版尚未命中。
        let engine = TextEngine::load(&font).expect("字体载入失败");
        let start = Instant::now();
        for text in STRINGS {
            black_box(engine.layout_box(black_box(text), 14.0, options));
        }
        cold.push(start.elapsed().as_nanos() as f64);
        // 单轮 20 万次命中，降低纳秒级计时噪声。
        let start = Instant::now();
        for _ in 0..10_000 {
            for text in STRINGS {
                black_box(engine.layout_box(black_box(text), 14.0, options));
            }
        }
        hit.push(start.elapsed().as_nanos() as f64 / 10_000.0);
    }
    cold.sort_by(f64::total_cmp);
    hit.sort_by(f64::total_cmp);
    println!("cold_20_median_ns={:.0} hit_20_median_ns={:.0} rounds=9 calls_per_round=200000",
             cold[4], hit[4]);
}
