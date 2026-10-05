// fw1_ramp.rs —— §112 字重对照渲染器（证据样张，不进生产）。
//
// 以 kanesumi CPU 光栅在 2×（浅底黑字）下渲染 `docs/research/fw1/ramp.py` 的六行
// （Header / Title / Subtitle / Base / Body / Caption），供与期望图 Noto 列逐行比较
// 平均墨迹覆盖率，标定 TextRenderTuning 的 contrast / gamma。参
// Ether `docs/DECISIONS_2026-10-05.md` §112 与任务 `fw1-font-weights`。
//
// 运行：
//   KANESUMI_TEST_FONT=/path/NotoSansSC-VF.ttf \
//   FW1_CONTRAST=0.5 FW1_GAMMA=1.4 \
//   cargo run --offline -p kanesumi-harness --example fw1_ramp -- docs/research/fw1/rust
// 每行输出一个 PNG：`row_{i}_{scene}.png`（物理像素，2×）。

use std::fs;
use std::path::{Path, PathBuf};

use kanesumi_canvas::text::{FontSource, TextEngine};
use kanesumi_canvas::{Scene, TextOverflow};
use kanesumi_core::{Color, FontWeight, Rect, TextStyle};
use kanesumi_harness::{CpuRenderer, TextRenderTuning};

/// (行名, 逻辑字号, 字重, 示例文字)。与 `docs/research/fw1/ramp.py` 的 ROWS 一致。
const ROWS: [(&str, f32, FontWeight, &str); 6] = [
    ("header", 34.0, FontWeight::Light, "欢迎使用 Ether"),
    ("title", 28.0, FontWeight::Light, "输入法"),
    ("subtitle", 20.0, FontWeight::Normal, "中英切换"),
    ("base", 15.0, FontWeight::ExtraBold, "时间和语言"),
    ("body", 14.0, FontWeight::Normal, "为每个应用记住中英状态 Ceyboard 1.2 MB"),
    ("caption", 12.0, FontWeight::Normal, "终端、全屏应用与游戏默认英文。选中 1 项，共 11 项"),
];

/// 样张内边距（逻辑 px）。
const PAD: f32 = 8.0;
/// 渲染缩放（2×，与期望图同）。
const SCALE: f32 = 2.0;

fn env_f32(name: &str, default: f32) -> f32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .unwrap_or(default)
}

fn save_png(w: u32, h: u32, rgba: &[u8], path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("建目录 {}: {e}", dir.display()))?;
    }
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        rgba.to_vec(),
        resvg::tiny_skia::IntSize::from_wh(w, h).ok_or("尺寸为零")?,
    )
    .ok_or("像素缓冲尺寸不符")?;
    pixmap
        .save_png(path)
        .map_err(|e| format!("写 {}: {e}", path.display()))
}

/// 加载字体：`KANESUMI_TEST_FONT`（可指向可变字体）优先，其次常见静态栈。
fn load_engine() -> Result<TextEngine, String> {
    let env = std::env::var("KANESUMI_TEST_FONT").ok().map(PathBuf::from);
    let path = env
        .clone()
        .filter(|p| p.exists())
        .or_else(|| {
            [
                "/tmp/opencode/NotoSansSC-VF.ttf",
                "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
                "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
            ]
            .into_iter()
            .map(PathBuf::from)
            .find(|p| p.exists())
        })
        .ok_or("未找到字体：设 KANESUMI_TEST_FONT")?;
    let primary = FontSource { path, collection_tag: None };
    TextEngine::load_stack(&primary, &[]).map_err(|e| e.to_string())
}

fn main() -> Result<(), String> {
    let out_dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("docs/research/fw1/rust"));
    let contrast = env_f32("FW1_CONTRAST", 0.5);
    let gamma = env_f32("FW1_GAMMA", 1.4);
    let tuning = TextRenderTuning {
        contrast,
        gamma,
        stem_darken_px: 0.0,
        outline_embolden_px: 0.0,
    };
    let engine = load_engine()?;
    eprintln!(
        "contrast={contrast} gamma={gamma} vf={}",
        if engine.extra_bold_is_variable() { "yes" } else { "no" }
    );

    // 与期望图同前景色（ramp.html 正文 #1A1A1A），避免颜色差污染覆盖率对比。
    let black = Color::new(26.0 / 255.0, 26.0 / 255.0, 26.0 / 255.0, 1.0);
    let white = Color::WHITE;
    for (i, (name, size, weight, text)) in ROWS.iter().enumerate() {
        let lh = engine.line_height(*size);
        let text_w = engine.measure_with_spacing_weighted(text, *size, 0.0, *weight);
        let lw = PAD + text_w.ceil() + PAD;
        let lh_total = PAD + lh + PAD;
        let mut r = CpuRenderer::new(lw, lh_total, SCALE);
        r.set_text_tuning(tuning);
        let mut scene = Scene::default();
        scene.fill_rect(white, Rect::new(0.0, 0.0, lw, lh_total));
        scene.text_with_options(
            (*text).to_string(),
            Rect::new(PAD, PAD, text_w.ceil() + 1.0, lh),
            black,
            TextStyle::new(*size, lh, *weight),
            kanesumi_canvas::TextAlign::Left,
            false,
            Some(1),
            TextOverflow::Clip,
        );
        let (pw, ph) = r.physical_size();
        let rgba = r.render(&engine, &scene, None).to_vec();
        let path = out_dir.join(format!("row_{i}_{name}.png"));
        save_png(pw, ph, &rgba, &path)?;
        println!("{}", path.display());
    }
    Ok(())
}
