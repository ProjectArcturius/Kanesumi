// text_density.rs —— 文字浓度证据 spike 样张生成器（tx1，2026-10-04）。
//
// 同一组 UI 文本（中英混排）按 逻辑字号 13/15/20/28 × 字重 Normal/Bold × 深底/浅底 ×
// 2×（主）/1×（辅）渲染；每个「浓度旋钮档 × 底色 × 缩放」出一张联系表（1:1 物理像素）
// + 一张 3× 最近邻放大裁切；另出一张 6 档横向对照总表。
//
// 只产证据、不进生产：所有旋钮经 `TextRenderTuning` 显式传入，默认（不设）即现行为；
// 本程序全部输出到仓库无关的 PNG，不触碰任何线上路径。
//
// 运行：cargo run --offline -p kanesumi-harness --example text_density [输出目录]
// 默认输出：docs/research/tx1/rust

use std::fs;
use std::path::{Path, PathBuf};

use kanesumi_canvas::text::{FontSource, TextEngine};
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{Color, FontWeight, Rect, TextStyle};
use kanesumi_harness::{CpuRenderer, TextRenderTuning};

/// 样张四周内边距（逻辑 px）。
const PAD: f32 = 12.0;
/// 行首标签列宽（逻辑 px）。
const LABEL_W: f32 = 52.0;
/// 相邻字号的间隔（逻辑 px）。
const ROW_GAP: f32 = 12.0;
/// 标签字号（逻辑 px）。
const LABEL_SIZE: f32 = 12.0;
/// 逻辑字号档。
const SIZES: [f32; 4] = [13.0, 15.0, 20.0, 28.0];
/// 文本按表意空格拆成三行：各变体换行点一致，便于逐像素对照。
const TEXT_LINES: [&str; 3] = [
    "设置　网络和 Internet　蓝牙和其他设备　个性化",
    "Settings　Network & Internet　Bluetooth & devices",
    "0123456789　文件资源管理器　The quick brown fox",
];

/// 一个浓度旋钮档（名字 + 参数）。
struct Arm {
    name: &'static str,
    tuning: TextRenderTuning,
}

/// 全部实验档。「base」即现状（旋钮默认值，逐像素等价于改动前）。
const ARMS: [Arm; 7] = [
    Arm {
        name: "base",
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "ctr",
        tuning: TextRenderTuning {
            contrast: 1.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "gam",
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.8,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "stem25",
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.25,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "stem50",
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.5,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "both-md",
        tuning: TextRenderTuning {
            contrast: 0.5,
            gamma: 1.4,
            stem_darken_px: 0.25,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "both-hi",
        tuning: TextRenderTuning {
            contrast: 1.0,
            gamma: 1.8,
            stem_darken_px: 0.5,
            outline_embolden_px: 0.0,
        },
    },
];

/// 横向对照总表选用的 6 档（顺序即面板自左至右）。
const MATRIX_ARMS: [&str; 6] = ["base", "ctr", "gam", "stem25", "both-md", "both-hi"];

/// RGBA 位图（sRGB 编码，直通 alpha），与 `CpuRenderer` 输出同布局。
struct Sheet {
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

impl Sheet {
    fn blank(w: u32, h: u32, fill: [u8; 4]) -> Self {
        let mut rgba = vec![0u8; w as usize * h as usize * 4];
        for px in rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&fill);
        }
        Self { w, h, rgba }
    }
}

/// 写 PNG（父目录自动创建）。
fn save_png(sheet: &Sheet, path: &Path) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("建目录 {}: {e}", dir.display()))?;
    }
    let pixmap = resvg::tiny_skia::Pixmap::from_vec(
        sheet.rgba.clone(),
        resvg::tiny_skia::IntSize::from_wh(sheet.w, sheet.h).ok_or("尺寸为零")?,
    )
    .ok_or("像素缓冲尺寸不符")?;
    pixmap
        .save_png(path)
        .map_err(|e| format!("写 {}: {e}", path.display()))
}

/// 把 `src` 整块贴到 `dst` 的 (ox, oy)（越界裁剪，直通拷贝 —— 两张都是不透明底）。
fn paste(dst: &mut Sheet, src: &Sheet, ox: u32, oy: u32) {
    for y in 0..src.h {
        let dy = oy + y;
        if dy >= dst.h {
            break;
        }
        let copy = src.w.min(dst.w.saturating_sub(ox)) as usize * 4;
        if copy == 0 {
            continue;
        }
        let di = ((dy * dst.w + ox) as usize) * 4;
        let si = (y * src.w) as usize * 4;
        dst.rgba[di..di + copy].copy_from_slice(&src.rgba[si..si + copy]);
    }
}

/// 裁切 (x, y, w, h)（物理像素，越界自动收敛）。
fn crop(sheet: &Sheet, x: u32, y: u32, w: u32, h: u32) -> Sheet {
    let x = x.min(sheet.w);
    let y = y.min(sheet.h);
    let w = w.min(sheet.w - x).max(1);
    let h = h.min(sheet.h - y).max(1);
    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    let row = w as usize * 4;
    for yy in 0..h {
        let src = ((y + yy) * sheet.w + x) as usize * 4;
        let dst = (yy * w) as usize * 4;
        rgba[dst..dst + row].copy_from_slice(&sheet.rgba[src..src + row]);
    }
    Sheet { w, h, rgba }
}

/// 最近邻放大 `factor` 倍。
fn upscale_nearest(sheet: &Sheet, factor: u32) -> Sheet {
    let (w, h) = (sheet.w * factor, sheet.h * factor);
    let mut rgba = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h {
        for x in 0..w {
            let si = (((y / factor) * sheet.w + (x / factor)) as usize) * 4;
            let di = ((y * w + x) as usize) * 4;
            rgba[di..di + 4].copy_from_slice(&sheet.rgba[si..si + 4]);
        }
    }
    Sheet { w, h, rgba }
}

/// 深底 / 浅底前景与背景（参 Ether tx1：`#1F1F1F` 白字 / `#FFFFFF` 黑字）。
fn pair(dark: bool) -> (Color, Color) {
    if dark {
        (
            Color::new(31.0 / 255.0, 31.0 / 255.0, 31.0 / 255.0, 1.0),
            Color::WHITE,
        )
    } else {
        (Color::WHITE, Color::new(0.0, 0.0, 0.0, 1.0))
    }
}

/// 渲染一个自定义 Scene 到 RGBA 位图。`build` 往已铺好底色的 Scene 追加命令。
fn render_scene(
    engine: &TextEngine,
    tuning: TextRenderTuning,
    lw: f32,
    lh: f32,
    scale: f32,
    bg: Color,
    build: &dyn Fn(&mut Scene),
) -> Sheet {
    let mut r = CpuRenderer::new(lw, lh, scale);
    r.set_text_tuning(tuning);
    let mut scene = Scene::default();
    scene.fill_rect(bg, Rect::new(0.0, 0.0, lw, lh));
    build(&mut scene);
    let (w, h) = r.physical_size();
    let rgba = r.render(engine, &scene, None).to_vec();
    Sheet { w, h, rgba }
}

/// 单行文本命令（不换行、裁切，行内无 `\n`）。
#[allow(clippy::too_many_arguments)] // 纯绘制参数（文本/位置/尺寸/颜色/样式），拆结构体反而更绕。
fn line(
    scene: &mut Scene,
    text: &str,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fg: Color,
    style: TextStyle,
) {
    scene.text_with_options(
        text.to_string(),
        Rect::new(x, y, w, h),
        fg,
        style,
        TextAlign::Left,
        false,
        Some(1),
        TextOverflow::Clip,
    );
}

/// 联系表：一个浓度档 × 一种底色 × 一种缩放，含 4 字号 × 2 字重共 8 行。
fn contact_sheet(
    engine: &TextEngine,
    tuning: TextRenderTuning,
    dark: bool,
    scale: f32,
    inner_w: f32,
) -> Sheet {
    let (bg, fg) = pair(dark);
    let label_style = TextStyle::new(LABEL_SIZE, LABEL_SIZE * 1.3, FontWeight::Normal);
    let lw = PAD + LABEL_W + inner_w + PAD;
    let mut content_h = 0.0;
    for &size in &SIZES {
        for _ in 0..2 {
            content_h += 3.0 * engine.line_height(size) + ROW_GAP;
        }
    }
    let lh_total = PAD + content_h + PAD;
    render_scene(engine, tuning, lw, lh_total, scale, bg, &|scene| {
        let mut y = PAD;
        for &size in &SIZES {
            let lh = engine.line_height(size);
            for weight in [FontWeight::Normal, FontWeight::Bold] {
                let tag = if weight == FontWeight::Bold { "B" } else { "N" };
                line(
                    scene,
                    &format!("{size:.0}{tag}"),
                    PAD,
                    y,
                    LABEL_W,
                    lh,
                    fg,
                    label_style,
                );
                let style = TextStyle::new(size, lh, weight);
                for (i, content) in TEXT_LINES.iter().enumerate() {
                    line(
                        scene,
                        content,
                        PAD + LABEL_W,
                        y + i as f32 * lh,
                        inner_w,
                        lh,
                        fg,
                        style,
                    );
                }
                y += 3.0 * lh + ROW_GAP;
            }
        }
    })
}

/// 内容列宽：以最大字号 Bold 的最长行定宽，保证任何变体都不折行。
fn inner_width(engine: &TextEngine) -> f32 {
    TEXT_LINES
        .iter()
        .map(|l| engine.measure_with_spacing_weighted(l, 28.0, 0.0, FontWeight::Bold))
        .fold(0.0, f32::max)
        + 4.0
}

/// 加载系统 Noto Sans CJK SC：Regular（400）+ Bold（700），TTC 选 SC 字面（裁定 N-40）。
fn load_engine() -> Result<TextEngine, String> {
    let dir = PathBuf::from("/usr/share/fonts/noto-cjk");
    let regular = dir.join("NotoSansCJK-Regular.ttc");
    let bold = dir.join("NotoSansCJK-Bold.ttc");
    if !regular.exists() || !bold.exists() {
        return Err(format!(
            "缺 Noto CJK 字体：{} / {}",
            regular.display(),
            bold.display()
        ));
    }
    let primary = FontSource {
        path: regular,
        collection_tag: Some("SC"),
    };
    let extra = [FontSource {
        path: bold,
        collection_tag: Some("SC"),
    }];
    TextEngine::load_stack(&primary, &extra).map_err(|e| e.to_string())
}

/// 6 档横向对照总表（size=15 Normal 深底 2×）：上排标签、下排各档面板并排。
fn matrix_sheet(engine: &TextEngine, inner_w: f32) -> Result<Sheet, String> {
    let (bg, fg) = pair(true);
    let size = 15.0f32;
    let lh = engine.line_height(size);
    let (plw, plh) = (PAD + inner_w + PAD, PAD + 3.0 * lh + PAD);
    let (seam, label_h) = (10.0f32, 26.0f32);
    let n = MATRIX_ARMS.len() as f32;
    let (tw, th) = (n * plw + (n - 1.0) * seam, label_h + plh);
    let mut out = Sheet::blank((tw * 2.0) as u32, (th * 2.0) as u32, [31, 31, 31, 255]);
    // 标签条用现状档渲染，保证文字可读。
    let label_style = TextStyle::new(13.0, 18.0, FontWeight::Normal);
    let strip = render_scene(
        engine,
        TextRenderTuning::default(),
        tw,
        label_h,
        2.0,
        bg,
        &|scene| {
            for (i, name) in MATRIX_ARMS.iter().enumerate() {
                let x = i as f32 * (plw + seam) + PAD;
                line(scene, name, x, 4.0, plw, label_h - 4.0, fg, label_style);
            }
        },
    );
    paste(&mut out, &strip, 0, 0);
    for (i, name) in MATRIX_ARMS.iter().enumerate() {
        let arm = ARMS
            .iter()
            .find(|a| a.name == *name)
            .ok_or_else(|| format!("未知档 {name}"))?;
        let panel = render_scene(engine, arm.tuning, plw, plh, 2.0, bg, &|scene| {
            let style = TextStyle::new(size, lh, FontWeight::Normal);
            for (k, content) in TEXT_LINES.iter().enumerate() {
                line(
                    scene,
                    content,
                    PAD,
                    PAD + k as f32 * lh,
                    inner_w,
                    lh,
                    fg,
                    style,
                );
            }
        });
        paste(
            &mut out,
            &panel,
            ((i as f32 * (plw + seam)) * 2.0) as u32,
            (label_h * 2.0) as u32,
        );
    }
    Ok(out)
}

fn run(out_dir: &Path) -> Result<(), String> {
    let engine = load_engine()?;
    let inner_w = inner_width(&engine);
    let mut index = String::new();
    for arm in &ARMS {
        for &dark in &[true, false] {
            for &scale in &[2.0f32, 1.0] {
                let sheet = contact_sheet(&engine, arm.tuning, dark, scale, inner_w);
                let (bg_tag, sc_tag) = (
                    if dark { "dark" } else { "light" },
                    if scale == 2.0 { "2x" } else { "1x" },
                );
                let name = format!("rd_{}_{}_{}.png", arm.name, bg_tag, sc_tag);
                save_png(&sheet, &out_dir.join(&name))?;
                index.push_str(&format!(
                    "{name}\tarm={} bg={bg_tag} scale={sc_tag}\n",
                    arm.name
                ));
                // 3× 最近邻放大：左上角（标签列 + 首行 13px）。
                let zx = ((PAD + LABEL_W + 360.0) * scale) as u32;
                let zh = ((PAD + engine.line_height(13.0)) * scale) as u32;
                let zoom = upscale_nearest(&crop(&sheet, 0, 0, zx, zh), 3);
                let zname = format!("rd_{}_{}_{}_zoom3.png", arm.name, bg_tag, sc_tag);
                save_png(&zoom, &out_dir.join(&zname))?;
                index.push_str(&format!("{zname}\t{name} 左上裁切 3× 最近邻\n"));
            }
        }
    }
    let matrix = matrix_sheet(&engine, inner_w)?;
    let mname = "matrix_15N_dark_2x.png";
    save_png(&matrix, &out_dir.join(mname))?;
    index.push_str(&format!(
        "{mname}\t6 档横向对照 size=15 Normal dark 2x 顺序={:?}\n",
        MATRIX_ARMS
    ));
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    fs::write(out_dir.join("index.txt"), &index).map_err(|e| e.to_string())?;
    println!(
        "text_density: 输出 {} 个 PNG 到 {}",
        ARMS.len() * 2 * 2 * 2 + 1,
        out_dir.display()
    );
    Ok(())
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/research/tx1/rust".to_string());
    let out_dir = PathBuf::from(out_dir);
    if let Err(e) = run(&out_dir) {
        eprintln!("text_density 失败: {e}");
        std::process::exit(1);
    }
}
