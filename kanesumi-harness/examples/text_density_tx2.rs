// text_density_tx2.rs —— 文字浓度证据第二轮样张生成器（tx2，2026-10-04）。
//
// 在 tx1（掩码膨胀产生光晕、不可用）之后，改从**真字重**（Noto Sans CJK 设计笔画）与
// **轮廓加粗**（沿法线外扩，边缘干净）里找浓度。8 个档各自绑定一个字重与一组旋钮，
// 同 tx1 的排版（4 逻辑字号 × 深/浅底 × 2×/1×，各档一张联系表 + 3× 放大），
// 另出一页「1:1 实际尺寸对照」供在 ThinkPad 上按 100% 查看。
//
// 只产证据、不进生产：所有旋钮经 `TextRenderTuning` 显式传入，默认（不设）即现行为。
//
// 运行：cargo run --offline -p kanesumi-harness --example text_density_tx2 [输出目录]
// 默认输出：docs/research/tx2/rust

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
/// 1:1 页的基准逻辑字号（2× 下 = 30 物理 px，接近正文 15）。
const ONESHEET_SIZE: f32 = 15.0;
/// 1:1 页左侧档名字号（逻辑 13 → 26 物理 px，≥ 任务要求的 24）。
const ONESHEET_LABEL: f32 = 13.0;
/// 文本按表意空格拆成三行：各变体换行点一致，便于逐像素对照（同 tx1）。
const TEXT_LINES: [&str; 3] = [
    "设置　网络和 Internet　蓝牙和其他设备　个性化",
    "Settings　Network & Internet　Bluetooth & devices",
    "0123456789　文件资源管理器　The quick brown fox",
];

/// 一个浓度档：档名 + 该档的字重 + 旋钮。
struct Arm {
    name: &'static str,
    weight: FontWeight,
    tuning: TextRenderTuning,
}

/// 全部实验档。`base` 即现状（Normal + 旋钮默认值）。`dil25` 是 tx1 的掩码膨胀反例。
const ARMS: [Arm; 8] = [
    Arm {
        name: "base",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "med",
        weight: FontWeight::Medium,
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "med-cg",
        weight: FontWeight::Medium,
        tuning: TextRenderTuning {
            contrast: 0.5,
            gamma: 1.4,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "reg-cg",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.5,
            gamma: 1.4,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.0,
        },
    },
    Arm {
        name: "ol15",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.15,
        },
    },
    Arm {
        name: "ol30",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.30,
        },
    },
    Arm {
        name: "ol15-cg",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.5,
            gamma: 1.4,
            stem_darken_px: 0.0,
            outline_embolden_px: 0.15,
        },
    },
    Arm {
        name: "dil25",
        weight: FontWeight::Normal,
        tuning: TextRenderTuning {
            contrast: 0.0,
            gamma: 1.0,
            stem_darken_px: 0.25,
            outline_embolden_px: 0.0,
        },
    },
];

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

/// 字重短标签（N/M/B）。
fn weight_tag(weight: FontWeight) -> &'static str {
    match weight {
        FontWeight::Bold => "B",
        FontWeight::Medium => "M",
        _ => "N",
    }
}

/// 深底 / 浅底前景与背景（同 tx1：`#1F1F1F` 白字 / `#FFFFFF` 黑字）。
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

/// 联系表：一个档 × 一种底色 × 一种缩放，含 4 字号 × 2 字重（档字重 / Bold）共 8 行。
fn contact_sheet(engine: &TextEngine, arm: &Arm, dark: bool, scale: f32, inner_w: f32) -> Sheet {
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
    render_scene(engine, arm.tuning, lw, lh_total, scale, bg, &|scene| {
        let mut y = PAD;
        for &size in &SIZES {
            let lh = engine.line_height(size);
            for weight in [arm.weight, FontWeight::Bold] {
                line(
                    scene,
                    &format!("{size:.0}{}", weight_tag(weight)),
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

/// 加载系统 Noto Sans CJK SC 全字重：Regular（400）+ DemiLight（350）+ Medium（500）+
/// Bold（700），TTC 选 SC 字面（裁定 N-40）。缺任一档即报错，避免静默降级。
fn load_engine() -> Result<TextEngine, String> {
    let dir = PathBuf::from("/usr/share/fonts/noto-cjk");
    let regular = dir.join("NotoSansCJK-Regular.ttc");
    let demilight = dir.join("NotoSansCJK-DemiLight.ttc");
    let medium = dir.join("NotoSansCJK-Medium.ttc");
    let bold = dir.join("NotoSansCJK-Bold.ttc");
    for path in [&regular, &demilight, &medium, &bold] {
        if !path.exists() {
            return Err(format!("缺 Noto CJK 字重：{}", path.display()));
        }
    }
    let primary = FontSource {
        path: regular,
        collection_tag: Some("SC"),
    };
    let extra = [
        FontSource {
            path: demilight,
            collection_tag: Some("SC"),
        },
        FontSource {
            path: medium,
            collection_tag: Some("SC"),
        },
        FontSource {
            path: bold,
            collection_tag: Some("SC"),
        },
    ];
    TextEngine::load_stack(&primary, &extra).map_err(|e| e.to_string())
}

/// 8 档横向对照总表（size=15 各档字重，深底 2×）：上排标签、下排各档面板并排。
fn matrix_sheet(engine: &TextEngine, inner_w: f32) -> Result<Sheet, String> {
    let (bg, fg) = pair(true);
    let size = 15.0f32;
    let lh = engine.line_height(size);
    let (plw, plh) = (PAD + inner_w + PAD, PAD + 3.0 * lh + PAD);
    let (seam, label_h) = (10.0f32, 26.0f32);
    let n = ARMS.len() as f32;
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
            for (i, arm) in ARMS.iter().enumerate() {
                let x = i as f32 * (plw + seam) + PAD;
                line(scene, arm.name, x, 4.0, plw, label_h - 4.0, fg, label_style);
            }
        },
    );
    paste(&mut out, &strip, 0, 0);
    for (i, arm) in ARMS.iter().enumerate() {
        let panel = render_scene(engine, arm.tuning, plw, plh, 2.0, bg, &|scene| {
            let style = TextStyle::new(size, lh, arm.weight);
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

/// 1:1 实际尺寸对照页：2× 物理像素、**不放大**，8 档上下排列，左侧大号档名标注。
/// 供在 ThinkPad 上按 100% 原尺寸查看真实观感。
fn onesheet(engine: &TextEngine, dark: bool, inner_w: f32) -> Sheet {
    let (bg, fg) = pair(dark);
    let lh = engine.line_height(ONESHEET_SIZE);
    let label_style = TextStyle::new(ONESHEET_LABEL, lh, FontWeight::Normal);
    let lw = PAD + 96.0 + inner_w + PAD;
    let row_h = 3.0 * lh + ROW_GAP;
    let lh_total = PAD + ARMS.len() as f32 * row_h + PAD;
    render_scene(
        engine,
        TextRenderTuning::default(),
        lw,
        lh_total,
        2.0,
        bg,
        &|scene| {
            let mut y = PAD;
            for arm in ARMS.iter() {
                line(scene, arm.name, PAD, y, 96.0, lh, fg, label_style);
                let style = TextStyle::new(ONESHEET_SIZE, lh, arm.weight);
                for (i, content) in TEXT_LINES.iter().enumerate() {
                    line(
                        scene,
                        content,
                        PAD + 96.0,
                        y + i as f32 * lh,
                        inner_w,
                        lh,
                        fg,
                        style,
                    );
                }
                y += row_h;
            }
        },
    )
}

/// 1:1 页内容列宽：按 `ONESHEET_SIZE` Bold 的最长行定宽 + 余量。
fn onesheet_width(engine: &TextEngine) -> f32 {
    TEXT_LINES
        .iter()
        .map(|l| engine.measure_with_spacing_weighted(l, ONESHEET_SIZE, 0.0, FontWeight::Bold))
        .fold(0.0, f32::max)
        + 4.0
}

fn run(out_dir: &Path) -> Result<(), String> {
    let engine = load_engine()?;
    let inner_w = inner_width(&engine);
    let one_w = onesheet_width(&engine);
    let mut index = String::new();
    for arm in &ARMS {
        for &dark in &[true, false] {
            for &scale in &[2.0f32, 1.0] {
                let sheet = contact_sheet(&engine, arm, dark, scale, inner_w);
                let (bg_tag, sc_tag) = (
                    if dark { "dark" } else { "light" },
                    if scale == 2.0 { "2x" } else { "1x" },
                );
                let name = format!("rd_{}_{}_{}.png", arm.name, bg_tag, sc_tag);
                save_png(&sheet, &out_dir.join(&name))?;
                index.push_str(&format!(
                    "{name}\tarm={} weight={:?} bg={bg_tag} scale={sc_tag}\n",
                    arm.name, arm.weight
                ));
                // 3× 最近邻放大：左上角（标签列 + 首行 13px 档字重）。
                let zx = ((PAD + LABEL_W + 360.0) * scale) as u32;
                let zh = ((PAD + engine.line_height(13.0)) * scale) as u32;
                let zoom = upscale_nearest(&crop(&sheet, 0, 0, zx, zh), 3);
                let zname = format!("rd_{}_{}_{}_zoom3.png", arm.name, bg_tag, sc_tag);
                save_png(&zoom, &out_dir.join(&zname))?;
                index.push_str(&format!("{zname}\t{name} 左上裁切 3× 最近邻\n"));
                // 多内环字专项放大（仅 2×）：13px 块第 3 行（含「器」）内容列 3×。
                if scale == 2.0 {
                    let lh13 = engine.line_height(13.0);
                    let cy = ((PAD + 2.0 * lh13) * scale) as u32;
                    let cx = ((PAD + LABEL_W) * scale) as u32;
                    let cw = (420.0 * scale) as u32;
                    let ch = (lh13 * scale).ceil() as u32;
                    let holes = upscale_nearest(&crop(&sheet, cx, cy, cw, ch), 3);
                    let hname = format!("rd_{}_{}_{}_holes3x.png", arm.name, bg_tag, sc_tag);
                    save_png(&holes, &out_dir.join(&hname))?;
                    index.push_str(&format!("{hname}\t{name} 13px 第 3 行（含器）3× 最近邻\n"));
                }
            }
        }
    }
    let matrix = matrix_sheet(&engine, inner_w)?;
    let mname = "matrix_15_dark_2x.png";
    save_png(&matrix, &out_dir.join(mname))?;
    index.push_str(&format!(
        "{mname}\t8 档横向对照 size=15 dark 2x 顺序={:?}\n",
        ARMS.iter().map(|a| a.name).collect::<Vec<_>>()
    ));
    for dark in [true, false] {
        let bg_tag = if dark { "dark" } else { "light" };
        let sheet = onesheet(&engine, dark, one_w);
        let name = format!("onesheet_{bg_tag}.png");
        save_png(&sheet, &out_dir.join(&name))?;
        index.push_str(&format!(
            "{name}\t1:1 实际尺寸（2× 物理，不放大）8 档上下排列\n"
        ));
    }
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    fs::write(out_dir.join("index.txt"), &index).map_err(|e| e.to_string())?;
    let pngs = fs::read_dir(out_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.to_string_lossy() == "png")
                })
                .count()
        })
        .unwrap_or(0);
    println!("text_density_tx2: {} 个 PNG 在 {}", pngs, out_dir.display());
    Ok(())
}

fn main() {
    let out_dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "docs/research/tx2/rust".to_string());
    let out_dir = PathBuf::from(out_dir);
    if let Err(e) = run(&out_dir) {
        eprintln!("text_density_tx2 失败: {e}");
        std::process::exit(1);
    }
}
