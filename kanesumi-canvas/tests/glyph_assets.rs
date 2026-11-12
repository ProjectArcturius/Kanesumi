// 控件底纹图形的资产合同 —— 三件事必须成立：
//
// 1. **枚举 ↔ 文件双向相等**：`GlyphName::ALL` 与资产仓 `glyphs/*.svg` 一一对应。
//    加图不加枚举、加枚举不加图，都在这里红（这一条替代了「生成物勿手改」那把锁）。
// 2. **每枚都画得出来**：落格、染色、像素尺寸正确，且不依赖字体。
// 3. **缓存按 (名字, 像素) 生效**：同尺寸第二次不重复光栅，2× 不会复用 1× 的位图。

use std::path::PathBuf;

use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::{Color, Point, Rect};

/// 资产真源目录（本仓 `assets/kanesumi-assets/glyphs`，submodule）。
fn glyph_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../assets/kanesumi-assets/glyphs")
}

fn rect() -> Rect {
    Rect::new(10.0, 20.0, 16.0, 16.0)
}

fn inside(rect: Rect, point: Point) -> bool {
    const EPS: f32 = 1.0;
    point.x >= rect.origin.x - EPS
        && point.x <= rect.right() + EPS
        && point.y >= rect.origin.y - EPS
        && point.y <= rect.bottom() + EPS
}

#[test]
fn all_names_are_sixteen() {
    assert_eq!(GlyphName::ALL.len(), 16, "词表定的是 16 枚");
}

#[test]
fn names_and_files_match_both_ways() {
    let dir = glyph_dir();
    let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("读不到资产目录 {}：{error}（submodule 没初始化？）", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".svg"))
        .map(|name| name.trim_end_matches(".svg").to_string())
        .collect();
    on_disk.sort();

    let mut declared: Vec<String> = GlyphName::ALL.iter().map(|name| name.file().to_string()).collect();
    declared.sort();

    assert_eq!(
        declared, on_disk,
        "枚举与资产文件不一致：加图要同时加 GlyphName，加枚举也要同时加 SVG"
    );
}

#[test]
fn file_names_are_kebab_case_and_unique() {
    let mut seen = std::collections::BTreeSet::new();
    for name in GlyphName::ALL {
        let file = name.file();
        assert!(
            file.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "{} 不是 kebab-case",
            file
        );
        assert!(seen.insert(file), "{} 重复", file);
    }
}

#[test]
fn every_glyph_embeds_a_parseable_svg() {
    for name in GlyphName::ALL {
        let svg = name.svg();
        assert!(svg.contains("<svg"), "{} 没有内嵌 SVG", name.file());
        assert!(svg.contains(r#"viewBox="0 0 32 32""#), "{} 母版不是 32×32", name.file());
        assert!(!svg.contains("Gradient"), "{} 出现渐变", name.file());
        assert!(!svg.contains("opacity"), "{} 出现透明度", name.file());
    }
}

/// 每枚三件套：落格 / 染色 / 缓存。
///
/// 每枚给一个**独有像素尺寸**，测试并行跑时互不干扰（缓存按 (名字, 像素) 分键）。
macro_rules! glyph_suite {
    ($($module:ident => $variant:ident, $px:literal),* $(,)?) => {
        $(
            mod $module {
                use super::*;

                const PX: u32 = $px;

                fn scale() -> f32 {
                    PX as f32 / 16.0
                }

                #[test]
                fn draws_inside_rect() {
                    let target = rect();
                    let mut scene = Scene::default();
                    icons::draw(&mut scene, GlyphName::$variant, target, Color::WHITE);
                    assert!(!scene.commands.is_empty(), "{} 什么都没画", GlyphName::$variant.file());

                    let mut covered = false;
                    for command in &scene.commands {
                        match command {
                            SceneCommand::Image { rect: drawn, rgba, .. } => {
                                assert_eq!(*drawn, target, "位图必须画在给定矩形内");
                                // 「有命令」不等于「画出了东西」：坏的 path 会光栅成一张全透明图，
                                // 所以这里直接查覆盖。
                                let painted =
                                    rgba.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
                                assert!(
                                    painted > 0,
                                    "{} 光栅成了全透明图（path 写坏了？）",
                                    GlyphName::$variant.file()
                                );
                                covered = true;
                            }
                            SceneCommand::Triangle { p0, p1, p2, .. } => {
                                for point in [p0, p1, p2] {
                                    assert!(inside(target, *point), "笔迹越出矩形：{point:?}");
                                }
                                covered = true;
                            }
                            _ => {}
                        }
                    }
                    assert!(covered, "{} 没有可判定的绘制命令", GlyphName::$variant.file());
                }

                #[test]
                fn uses_the_given_color() {
                    let target = rect();
                    let color = Color::new(0.2, 0.4, 0.6, 1.0);
                    let mut scene = Scene::default();
                    icons::draw(&mut scene, GlyphName::$variant, target, color);
                    for command in &scene.commands {
                        match command {
                            SceneCommand::Image { tint, .. } => {
                                assert_eq!(*tint, Some(color), "位图必须按给定色染色");
                            }
                            SceneCommand::Triangle { color: drawn, .. } => {
                                assert_eq!(*drawn, color, "几何 glyph 必须用给定色");
                            }
                            _ => {}
                        }
                    }
                }

                #[test]
                fn caches_per_pixel_size() {
                    let target = rect();
                    let mut scene = Scene::default();
                    icons::draw_scaled(&mut scene, GlyphName::$variant, target, Color::WHITE, scale());
                    let first = icons::cached_pixels(GlyphName::$variant);
                    if GlyphName::$variant.is_geometric() {
                        assert!(first.is_empty(), "{} 走几何快路径，不该占纹理", GlyphName::$variant.file());
                        return;
                    }
                    assert!(
                        first.contains(&PX),
                        "{} 应缓存 {PX}px 这一份（实为 {first:?}）",
                        GlyphName::$variant.file()
                    );

                    let mut scene = Scene::default();
                    icons::draw_scaled(&mut scene, GlyphName::$variant, target, Color::WHITE, scale());
                    assert_eq!(
                        icons::cached_pixels(GlyphName::$variant),
                        first,
                        "同尺寸第二次调用不该再光栅一张"
                    );
                }
            }
        )*
    };
}

glyph_suite! {
    chevron_down => ChevronDown, 101,
    chevron_up => ChevronUp, 102,
    chevron_left => ChevronLeft, 103,
    chevron_right => ChevronRight, 104,
    check => Check, 105,
    minus => Minus, 106,
    close => Close, 107,
    hamburger => Hamburger, 108,
    more_horizontal => MoreHorizontal, 109,
    more_vertical => MoreVertical, 110,
    search => Search, 111,
    add => Add, 112,
    star => Star, 113,
    info => Info, 114,
    warning => Warning, 115,
    error => Error, 116,
}

#[test]
fn two_times_does_not_reuse_the_one_time_bitmap() {
    // 1× 与 2× 必须各存一份：拿 16px 图放大到 32px 会糊。
    let name = GlyphName::Search;
    let target = rect();
    let mut scene = Scene::default();
    icons::draw_scaled(&mut scene, name, target, Color::WHITE, 1.0);
    let mut scene = Scene::default();
    icons::draw_scaled(&mut scene, name, target, Color::WHITE, 2.0);
    let pixels = icons::cached_pixels(name);
    assert_eq!(pixels, vec![16, 32], "1× 与 2× 各一份，实为 {pixels:?}");
}

#[test]
fn zero_sized_rect_draws_nothing() {
    let mut scene = Scene::default();
    icons::draw(&mut scene, GlyphName::Star, Rect::new(0.0, 0.0, 0.0, 0.0), Color::WHITE);
    assert!(scene.commands.is_empty(), "零尺寸不该产生命令");
}

#[test]
fn geometric_glyphs_need_no_texture() {
    // chevron 与 check 走几何快路径：不占纹理，也就不进缓存。
    for name in [
        GlyphName::ChevronDown,
        GlyphName::ChevronUp,
        GlyphName::ChevronLeft,
        GlyphName::ChevronRight,
        GlyphName::Check,
    ] {
        let before = icons::cached_pixels(name);
        let mut scene = Scene::default();
        icons::draw_scaled(&mut scene, name, rect(), Color::WHITE, 1.0);
        assert_eq!(icons::cached_pixels(name), before, "{} 不该光栅", name.file());
        assert!(
            scene.commands.iter().all(|c| matches!(c, SceneCommand::Triangle { .. })),
            "{} 应只产出三角",
            name.file()
        );
    }
}
