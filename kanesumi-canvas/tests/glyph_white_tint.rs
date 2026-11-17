// 单色 glyph 是 alpha 蒙版：白 tint 在既有 CPU/GPU 后端表示保留纹理原色。
// 因此蒙版 RGB 必须中性白，不能把资产的黑墨色当成最终前景。
use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::{Color, Rect};

#[test]
fn mask_pixels_allow_white_foreground_at_both_scales() {
    for scale in [1.0, 2.0] {
        for name in GlyphName::ALL {
            let mut scene = Scene::default();
            icons::draw_scaled(&mut scene, name, Rect::new(0.0, 0.0, 32.0, 32.0), Color::WHITE, scale);
            for command in &scene.commands {
                if let SceneCommand::Image { rgba, tint, width, height, .. } = command {
                    assert_eq!(*tint, Some(Color::WHITE));
                    assert_eq!((*width, *height), ((32.0 * scale) as u32, (32.0 * scale) as u32));
                    let mut ink = 0;
                    for px in rgba.chunks_exact(4).filter(|px| px[3] > 0) {
                        ink += 1;
                        assert_eq!(&px[..3], &[255, 255, 255], "{name:?} @ {scale}x must be a neutral mask");
                    }
                    assert!(ink > 0, "{name:?} has no ink");
                }
            }
        }
    }
}

#[test]
fn normalization_preserves_source_alpha_and_destination() {
    for name in [GlyphName::Hamburger, GlyphName::Star, GlyphName::Close] {
        let raw = kanesumi_canvas::icon::rasterize_svg_bytes(name.svg().as_bytes(), 64).unwrap();
        let target = Rect::new(4.0, 7.0, 32.0, 32.0);
        let mut scene = Scene::default();
        icons::draw_scaled(&mut scene, name, target, Color::WHITE, 2.0);
        let SceneCommand::Image { rgba, rect, .. } = &scene.commands[0] else {
            panic!("expected bitmap mask");
        };
        assert_eq!(*rect, target);
        assert_eq!(rgba.len(), raw.rgba.len());
        for (mask, source) in rgba.chunks_exact(4).zip(raw.rgba.chunks_exact(4)) {
            assert_eq!(mask[3], source[3], "{name:?} alpha geometry changed");
        }
    }
}

#[test]
fn normalized_mask_remains_shared_in_the_cache() {
    let draw = |color| {
        let mut scene = Scene::default();
        icons::draw_scaled(&mut scene, GlyphName::Hamburger, Rect::new(0.0, 0.0, 32.0, 32.0), color, 2.0);
        let SceneCommand::Image { rgba, .. } = scene.commands.remove(0) else {
            panic!("expected bitmap mask");
        };
        rgba
    };
    let first = draw(Color::WHITE);
    let second = draw(Color::BLACK);
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}
