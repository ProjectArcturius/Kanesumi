// metro_tile_sheet —— MetroTile Win10 版式快照（无窗）：Mini / Standard / Large × 深浅两档，
// 外加 image opacity 0.5 对照。存 docs/research/metro_tile/。
//   cargo run -p kanesumi-gallery --example metro_tile_sheet

use std::path::PathBuf;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, rasterize_svg};
use kanesumi_controls::{MetroTile, TileLive, TileSize};
use kanesumi_core::{Accent, Color, ColorScheme, MetroTheme, Rect};

/// 快照四周留白（磁贴外仍见背景色，便于核对居中与内边距）。
const MARGIN: f32 = 24.0;

fn main() {
    let out_dir = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../docs/research/metro_tile"
    ));
    std::fs::create_dir_all(&out_dir).expect("建快照目录");
    let font = find_font().expect("需要字体（设 KANESUMI_TEST_FONT）");
    let engine = TextEngine::load(&font).expect("字体加载");
    // 磁贴 glyph：MetroTile 的 tint 语义是「白 = 保留原色」，故源 SVG 必须本身是白色，
    // 快照才能得到 Win10 式白图标（黑色 glyph + 白 tint 会原样输出黑色）。
    let glyph = r##"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 16 16">
      <g fill="#FFFFFF"><rect x="2" y="6" width="6" height="6"/><rect x="8" y="1" width="6" height="6"/><rect x="8" y="9" width="6" height="6"/></g>
    </svg>"##;
    let glyph_path = std::env::temp_dir().join("kanesumi_tile_glyph.svg");
    std::fs::write(&glyph_path, glyph).expect("写 glyph");
    let icon = rasterize_svg(&glyph_path, 160).expect("glyph 图标光栅化");

    // 三档尺寸 × 深浅两档，各一张（Mini/Standard/Large 的 Win10 版式核对）。
    for (scheme, sname) in [(ColorScheme::Dark, "dark"), (ColorScheme::Light, "light")] {
        let theme = MetroTheme::for_scheme(scheme, Accent::default());
        for (size, kind, tw, th) in [
            (TileSize::Mini, "mini", 64.0, 64.0),
            (TileSize::Standard, "standard", 136.0, 136.0),
            (TileSize::Large, "large", 280.0, 136.0),
        ] {
            let cw = tw + MARGIN * 2.0;
            let ch = th + MARGIN * 2.0;
            let mut tile = MetroTile::new(tile_label(kind), size, tile_color(kind));
            tile.icon = Some(icon.clone());
            tile.live = tile_live(kind);

            let mut scene = Scene::default();
            scene.fill_rect(theme.colors.background, Rect::new(0.0, 0.0, cw, ch));
            tile.render(
                &theme,
                &engine,
                Rect::new(MARGIN, MARGIN, tw, th),
                &mut scene,
            );

            let out = out_dir.join(format!("{kind}_{sname}.png"));
            save_scene(&engine, &scene, cw, ch, &out);
            println!("wrote {}", out.display());
        }

        // opacity 对照：同一图标叠在强调色底上，左 opacity 1.0、右 opacity 0.5。
        let (cw, ch) = (308.0 + MARGIN * 2.0, 72.0 + MARGIN * 2.0);
        let mut scene = Scene::default();
        scene.fill_rect(theme.colors.background, Rect::new(0.0, 0.0, cw, ch));
        scene.fill_rect(theme.colors.primary, Rect::new(MARGIN, MARGIN, 308.0, 72.0));
        scene.image_with_opacity(
            &icon,
            Rect::new(MARGIN + 16.0, MARGIN + 12.0, 48.0, 48.0),
            Some(Color::WHITE),
            1.0,
        );
        scene.image_with_opacity(
            &icon,
            Rect::new(MARGIN + 116.0, MARGIN + 12.0, 48.0, 48.0),
            Some(Color::WHITE),
            0.5,
        );
        scene.text(
            "opacity 1.0 / 0.5".into(),
            Rect::new(MARGIN + 180.0, MARGIN + 24.0, 120.0, 24.0),
            theme.colors.on_primary,
            theme.typography.body,
            TextAlign::Left,
        );
        let out = out_dir.join(format!("opacity_{sname}.png"));
        save_scene(&engine, &scene, cw, ch, &out);
        println!("wrote {}", out.display());
    }
}

/// 快照用标题 / 基调色 / 动态内容（仅用于展示版式，无业务含义）。
fn tile_label(kind: &str) -> &'static str {
    match kind {
        "mini" => "音乐",
        "standard" => "邮件",
        _ => "相册",
    }
}

fn tile_color(kind: &str) -> Color {
    match kind {
        "mini" => Color::from_hex(0x4C_A0_5E),
        "standard" => Color::from_hex(0xC8_42_3B),
        _ => Color::from_hex(0x3B_8F_C8),
    }
}

fn tile_live(kind: &str) -> TileLive {
    match kind {
        "mini" => TileLive::Badge(3),
        "standard" => TileLive::Preview("季度账单已生成".into()),
        _ => TileLive::Lines(vec!["年度报告".into(), "季度账单".into()]),
    }
}

fn save_scene(engine: &TextEngine, scene: &Scene, w: f32, h: f32, out: &std::path::Path) {
    let mut cpu = kanesumi_harness::CpuRenderer::new(w, h, 1.0);
    let (pw, ph) = cpu.physical_size();
    let rgba = cpu.render(engine, scene, None).to_vec();
    let pix = resvg::tiny_skia::Pixmap::from_vec(
        rgba,
        resvg::tiny_skia::IntSize::from_wh(pw, ph).unwrap(),
    )
    .unwrap();
    pix.save_png(out).unwrap();
}

fn find_font() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}
