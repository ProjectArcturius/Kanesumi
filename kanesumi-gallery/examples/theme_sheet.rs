// theme_sheet —— 主题样张（Wave A4 浅色调色板审计 · 2026-10-01）。
//
// 把所有已接入元素树的控件按分类摆一页，每类给 Normal 与（可做到的）Disabled / Checked /
// Selected 实例，用 `--scheme light|dark` 切换方案，`--snapshot <out.png>` 无窗渲染一帧。
//
// 运行（任意平台）：
//   cargo run -p kanesumi-gallery --example theme_sheet -- --scheme light --snapshot out.png 2
//
// 页面颜色一律走 `ThemeColor` 令牌 / 控件内部按 `MetroTheme` 取色，**不写字面量**
// （Kanesumi 铁律：主题切换时颜色必须跟随，参 `kanesumi-core/src/brush.rs` 抬头注释）。

use kanesumi_controls::{
    MenuItem, MetroButton, MetroCheckBox, MetroDropDownButton, MetroInfoBar,
    MetroList, MetroNavigationView, MetroNumberBox, MetroPasswordBox, MetroProgressBar,
    MetroProgressRing, MetroRadioButtons, MetroRatingControl, MetroSlider, MetroSwitch,
    MetroTabView, MetroTextBox, NavigationViewItem,
};
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Accent, ColorScheme, MetroTheme, Size, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Label, Stack};
use kanesumi_harness::element::{Action, Align, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp, TreeHost};
#[cfg(not(target_os = "linux"))]
use kanesumi_harness::App;

// ── 无窗快照：非 Linux 无 `kanesumi-harness::snapshot`（该模块 cfg 限 Linux），
// 故经 `#[path]` 复用 harness 的 CPU 光栅器（cpu_raster.rs 只依赖 `crate::render` 的
// 三个纯文本排版符号，本地补一个同名小模块即可），PNG 编码在下方自足实现。
#[cfg(not(target_os = "linux"))]
#[path = "../../kanesumi-harness/src/cpu_raster.rs"]
#[allow(clippy::all)]
mod cpu_raster;

/// harness `render.rs` 中 CPU 光栅器所需的最小文本排版接口（原样搬运，保持放置一致）。
#[cfg(not(target_os = "linux"))]
mod render {
    use std::collections::HashMap;

    use kanesumi_canvas::TextOverflow;
    use kanesumi_canvas::text::{TextEngine, TextLayoutOptions};
    use kanesumi_core::{Rect, TextStyle};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct GlyphKey {
        pub engine_id: u64,
        pub font_id: u32,
        pub glyph_id: u16,
        pub size_bits: u32,
    }

    pub struct PlacedGlyph {
        pub key: GlyphKey,
        pub x: f32,
        pub y: f32,
        pub w: f32,
        pub h: f32,
    }

    #[allow(clippy::too_many_arguments)]
    pub fn layout_text_glyphs(
        engine: &TextEngine,
        glyph_bitmaps: &mut HashMap<GlyphKey, (kanesumi_canvas::text::GlyphMetrics, Vec<u8>)>,
        content: &str,
        rect: Rect,
        style: TextStyle,
        align: kanesumi_canvas::TextAlign,
        wrap: bool,
        max_lines: Option<usize>,
        overflow: TextOverflow,
        scale: f32,
    ) -> Vec<PlacedGlyph> {
        let size_phys = style.size * scale;
        let mut options =
            TextLayoutOptions::wrapped(rect.size.width, rect.size.height, style.line_height);
        options.letter_spacing_em = style.letter_spacing_em;
        options.max_lines = max_lines;
        options.wrap = wrap;
        options.overflow = overflow;
        let layout = engine.layout_box(content, style.size, options);
        let line_advance = style.line_height;
        let ascent_log = engine.ascent(size_phys) / scale;

        let mut out = Vec::new();
        let mut line_y = rect.origin.y;
        for line in &layout.lines {
            let line_w = line.width;
            let x_log = match align {
                kanesumi_canvas::TextAlign::Left => rect.origin.x,
                kanesumi_canvas::TextAlign::Center => rect.origin.x + (rect.size.width - line_w) / 2.0,
                kanesumi_canvas::TextAlign::Right => rect.origin.x + rect.size.width - line_w,
            };
            let baseline = line_y + ascent_log;
            let mut pen = x_log;
            for glyph in engine.shape_line(&line.content, style.size, style.letter_spacing_em) {
                let key = GlyphKey {
                    engine_id: engine.identity(),
                    font_id: glyph.font_id,
                    glyph_id: glyph.glyph_id,
                    size_bits: size_phys.to_bits(),
                };
                let metrics = if let Some((m, _)) = glyph_bitmaps.get(&key) {
                    *m
                } else {
                    let (m, b) = engine.rasterize_glyph(glyph.font_id, glyph.glyph_id, size_phys);
                    if m.width > 0 && m.height > 0 {
                        glyph_bitmaps.insert(key, (m, b));
                    }
                    m
                };
                if metrics.width == 0 || metrics.height == 0 {
                    pen += glyph.x_advance;
                    continue;
                }
                let inv = 1.0 / scale;
                let x0 = pen + glyph.x_offset + metrics.xmin as f32 * inv;
                let y0 = baseline
                    - glyph.y_offset
                    - metrics.ymin as f32 * inv
                    - metrics.height as f32 * inv;
                out.push(PlacedGlyph {
                    key,
                    x: x0,
                    y: y0,
                    w: metrics.width as f32 * inv,
                    h: metrics.height as f32 * inv,
                });
                pen += glyph.x_advance;
            }
            line_y += line_advance;
        }
        out
    }
}

/// 最小 PNG 编码器（RGBA8，zlib「存储」块，无压缩）—— 免新增依赖。
#[cfg(not(target_os = "linux"))]
mod png {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                let mask = (crc & 1).wrapping_neg();
                crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
            }
        }
        !crc
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    fn chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut with_tag = tag.to_vec();
        with_tag.extend_from_slice(data);
        out.extend_from_slice(&with_tag);
        out.extend_from_slice(&crc32(&with_tag).to_be_bytes());
    }

    /// RGBA 像素（直通 alpha）→ PNG 字节。
    pub fn encode_rgba(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        // 原始扫描线：每行前置 filter=0。
        let mut raw = Vec::with_capacity((w as usize * 4 + 1) * h as usize);
        for y in 0..h as usize {
            raw.push(0u8);
            let start = y * w as usize * 4;
            raw.extend_from_slice(&rgba[start..start + w as usize * 4]);
        }
        // zlib：头 + 存储块（每块 ≤ 65535）+ Adler-32。
        let mut z = vec![0x78, 0x01];
        let mut offset = 0;
        while offset < raw.len() {
            let n = (raw.len() - offset).min(65535);
            let last = offset + n >= raw.len();
            z.push(if last { 0x01 } else { 0x00 });
            z.extend_from_slice(&(n as u16).to_le_bytes());
            z.extend_from_slice(&(!(n as u16)).to_le_bytes());
            z.extend_from_slice(&raw[offset..offset + n]);
            offset += n;
        }
        z.extend_from_slice(&adler32(&raw).to_be_bytes());

        let mut out = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&h.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &ihdr);
        chunk(&mut out, b"IDAT", &z);
        chunk(&mut out, b"IEND", &[]);
        out
    }
}

// ── 页面 ────────────────────────────────────────────────────────────────────

const SHEET_W: f32 = 960.0;
const SHEET_H: f32 = 1720.0;

struct Sheet {
    config: AppConfig,
    theme: MetroTheme,
}

/// 靠左、按内容定宽。
fn start() -> LayoutProps {
    LayoutProps {
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

/// 靠左、定宽（输入类控件）。
fn fixed(w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        h_align: Align::Start,
        v_align: Align::Start,
        width: Some(w),
        height: Some(h),
        ..LayoutProps::default()
    }
}

/// 分类标题。
fn heading(tree: &mut Tree, parent: WidgetId, text: &str, sheet: &Sheet) {
    tree.insert(
        parent,
        Label::new(text)
            .style(sheet.theme.typography.body)
            .color(ThemeColor::Primary),
    );
}

impl TreeApp for Sheet {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let t = self.theme.typography;
        let page = tree.insert(
            tree.root(),
            Border::new()
                .background(ThemeColor::Background)
                .padding(Insets::all(24.0)),
        );
        let col = tree.insert(page, Stack::column().with_spacing(10.0));
        tree.insert(
            col,
            Label::new(match self.theme.scheme {
                ColorScheme::Dark => "Kanesumi 主题样张 · Dark",
                ColorScheme::Light => "Kanesumi 主题样张 · Light",
            })
            .style(t.page_heading),
        );

        // Button：标准 / 强调 / 禁用。
        heading(tree, col, "Button", self);
        let row = tree.insert(col, Stack::row().with_spacing(10.0));
        tree.insert_with(row, MetroButton::new("标准"), start());
        tree.insert_with(row, MetroButton::accent("强调"), start());
        let b = tree.insert_with(row, MetroButton::new("禁用"), start());
        tree.set_enabled(b, false);

        // CheckBox：未选 / 已选 / 禁用。
        heading(tree, col, "CheckBox", self);
        let row = tree.insert(col, Stack::row().with_spacing(10.0));
        tree.insert_with(row, MetroCheckBox::new("未选"), start());
        tree.insert_with(row, MetroCheckBox::new("已选").with_checked(true), start());
        let c = tree.insert_with(row, MetroCheckBox::new("禁用").with_checked(true), start());
        tree.set_enabled(c, false);

        // RadioButtons：默认选中第二项。
        heading(tree, col, "RadioButton", self);
        let mut radio = MetroRadioButtons::new(vec!["一".into(), "二".into(), "三".into()]);
        radio.selected_index = Some(1);
        tree.insert_with(col, radio, start());

        // ToggleSwitch：关 / 开 / 禁用。
        heading(tree, col, "ToggleSwitch", self);
        let row = tree.insert(col, Stack::row().with_spacing(16.0));
        tree.insert_with(row, MetroSwitch::new().with_state_text("开", "关"), start());
        let mut on = MetroSwitch::new().with_state_text("开", "关");
        on.set_checked(true);
        tree.insert_with(row, on, start());
        let mut off = MetroSwitch::new().with_state_text("开", "关");
        off.set_checked(true);
        let s = tree.insert_with(row, off, start());
        tree.set_enabled(s, false);

        // 文本类：TextBox / PasswordBox / NumberBox（禁用各一）。
        heading(tree, col, "TextBox / PasswordBox / NumberBox", self);
        let row = tree.insert(col, Stack::row().with_spacing(10.0));
        tree.insert_with(row, MetroTextBox::new().with_text("ether-dev"), fixed(220.0, 40.0));
        tree.insert_with(row, MetroPasswordBox::with_placeholder("密码"), fixed(180.0, 40.0));
        let mut num = MetroNumberBox::new();
        num.set_value(42.0);
        tree.insert_with(row, num, fixed(160.0, 40.0));
        let tb = tree.insert_with(row, MetroTextBox::new().with_text("禁用"), fixed(160.0, 40.0));
        tree.set_enabled(tb, false);

        // Slider。
        heading(tree, col, "Slider", self);
        let mut slider = MetroSlider::new().with_range(0.0, 100.0);
        slider.set_value(60.0);
        tree.insert_with(col, slider, fixed(360.0, 40.0));

        // List：选中第二行。
        heading(tree, col, "ListView", self);
        let mut list = MetroList::new(vec![
            "第一行".into(),
            "第二行（已选）".into(),
            "第三行".into(),
            "第四行".into(),
        ]);
        list.select(Some(1));
        tree.insert_with(col, list, fixed(320.0, 160.0));

        // TabView。
        heading(tree, col, "TabView", self);
        let mut tabs = MetroTabView::new(vec!["邮件".into(), "日历".into(), "设置".into()]);
        tabs.selected_index = 1;
        tree.insert_with(col, tabs, fixed(440.0, 48.0));

        // InfoBar。
        heading(tree, col, "InfoBar", self);
        tree.insert_with(
            col,
            MetroInfoBar::error("出错了", "无法连接到服务器，请稍后重试。"),
            fixed(560.0, 60.0),
        );

        // DropDownButton（ComboBox 等价物）。
        heading(tree, col, "DropDownButton", self);
        tree.insert_with(
            col,
            MetroDropDownButton::new(
                "选择一个选项",
                vec![
                    MenuItem::new("选项一"),
                    MenuItem::new("选项二"),
                    MenuItem::new("选项三"),
                ],
            ),
            start(),
        );

        // RatingControl。
        heading(tree, col, "RatingControl", self);
        let mut rating = MetroRatingControl::new();
        rating.value = 3.0;
        tree.insert_with(col, rating, start());

        // NavigationView 窗格（展开态，第二项选中）。
        heading(tree, col, "NavigationView", self);
        let mut nav = MetroNavigationView::new(vec![
            NavigationViewItem::new("主页"),
            NavigationViewItem::new("收藏"),
            NavigationViewItem::new("设置"),
        ]);
        nav.selected = Some(vec![1]);
        tree.insert_with(col, nav, fixed(240.0, 200.0));

        // ProgressBar / ProgressRing。
        heading(tree, col, "Progress", self);
        let row = tree.insert(col, Stack::row().with_spacing(16.0));
        let mut bar = MetroProgressBar::new();
        bar.set_value(0.6);
        tree.insert_with(row, bar, fixed(260.0, 8.0));
        let mut ring = MetroProgressRing::new();
        ring.set_value(0.7);
        tree.insert_with(row, ring, fixed(40.0, 40.0));
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

// ── 入口 ────────────────────────────────────────────────────────────────────

/// 字体查找（跨平台）：环境变量优先，其次已知的 Windows / Linux 路径。
fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    for p in [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ] {
        let p = std::path::PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// 无窗快照。Linux 走 harness 官方 `snapshot`；其它平台走本地 CPU 光栅器 + 自足 PNG。
#[cfg(target_os = "linux")]
fn render_snapshot(
    host: &mut TreeHost<Sheet>,
    engine: &TextEngine,
    size: Size,
    scale: f32,
    out: &str,
) -> Result<(u32, u32), String> {
    kanesumi_harness::snapshot::render_png(host, engine, size, scale, 3, std::path::Path::new(out))
}

#[cfg(not(target_os = "linux"))]
fn render_snapshot(
    host: &mut TreeHost<Sheet>,
    engine: &TextEngine,
    size: Size,
    scale: f32,
    out: &str,
) -> Result<(u32, u32), String> {
    let mut scene = kanesumi_canvas::Scene::default();
    for _ in 0..3 {
        host.update(1.0 / 60.0);
        host.render_into(engine, size, &mut scene);
    }
    let mut cpu = cpu_raster::CpuRenderer::new(size.width, size.height, scale);
    let (w, h) = cpu.physical_size();
    let rgba = cpu.render(engine, &scene, None).to_vec();
    std::fs::write(out, png::encode_rgba(w, h, &rgba)).map_err(|e| e.to_string())?;
    Ok((w, h))
}

#[cfg(target_os = "linux")]
fn run_gui(host: TreeHost<Sheet>) {
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
}

#[cfg(not(target_os = "linux"))]
fn run_gui(_host: TreeHost<Sheet>) {
    println!("theme_sheet 需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let scheme = match args.iter().position(|a| a == "--scheme") {
        Some(i) if args.get(i + 1).map(String::as_str) == Some("light") => ColorScheme::Light,
        _ => ColorScheme::Dark,
    };
    let theme = MetroTheme::for_scheme(scheme, Accent::default());
    let app = Sheet {
        config: AppConfig::new(
            "org.ether.kanesumi.themesheet",
            "主题样张",
            EtherRole::Browser,
            SHEET_W,
            SHEET_H,
        ),
        theme,
    };
    let mut host = TreeHost::new(app);

    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).expect("--snapshot 需要输出路径");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = render_snapshot(&mut host, &engine, Size::new(SHEET_W, SHEET_H), scale, out)
            .expect("快照失败");
        println!(
            "snapshot {out} {w}x{h} scheme={scheme:?} font={}",
            font.display()
        );
        return;
    }
    run_gui(host);
}
