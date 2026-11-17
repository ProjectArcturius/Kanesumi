// MetroRatingControl —— 星级评分。参 CONTROL_SPEC §24。
//
// 移植自 microsoft-ui-xaml/dev/RatingControl（RatingControl.cpp + RatingControl.xaml）：
// - MaxRating 默认 5；star cell ≈ 24×24；实星取强调色、空星取 `on_surface_variant` 压低不透明度；
// - Value 支持小数（部分星 = 前景星按比例裁剪，用 Scene::clip）；
// - PointerOver 预览（hover_value）；IsClearEnabled 点当前值星清零；IsReadOnly 只读。
//
// Kanesumi 适配：星形走 L0 图形层（`kanesumi_canvas::icons`），不再写字体星形 ——
// 字体里缺那两枚字符时会渲染成方框（参 docs/VISUAL_ISSUES.md V7 与资产仓
// `docs/GLYPH_ASSETS.md`）。L0 只有一枚实心星：空星压低不透明度、半星裁剪复用同一份光栅。

use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, SceneCommand};
use kanesumi_core::sizes::ICON_32;
use kanesumi_core::typography::TextStyle;
use kanesumi_core::{Color, FontWeight, MetroTheme, Point, Rect};

/// star cell 边长（≈24）。取值入尺寸表 `kanesumi_core::sizes::ICON_24`（N18 图标尺寸档）。
pub const RATING_ITEM_SIZE: f32 = kanesumi_core::sizes::ICON_24;
/// 星间距（4）。
pub const RATING_ITEM_SPACING: f32 = 4.0;

/// MetroRatingControl —— 星级评分。参 CONTROL_SPEC §24。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroRatingControl {
    /// 当前值（0..max_rating，可小数）。
    pub value: f64,
    pub max_rating: usize,
    pub is_read_only: bool,
    /// 点当前值星清零。
    pub is_clear_enabled: bool,
    /// PointerOver 预览值（None = 无预览）。
    pub hover_value: Option<f64>,
    /// 星字号。
    pub item_size: f32,
    pub item_spacing: f32,
}

impl Default for MetroRatingControl {
    fn default() -> Self {
        Self {
            value: 0.0,
            max_rating: 5,
            is_read_only: false,
            is_clear_enabled: false,
            hover_value: None,
            item_size: RATING_ITEM_SIZE,
            item_spacing: RATING_ITEM_SPACING,
        }
    }
}

impl MetroRatingControl {
    pub fn new() -> Self {
        Self::default()
    }

    /// 星字形样式（item_size）。星形图标字形保持 Normal —— 加粗会让星角糊成团。
    pub fn star_style(&self) -> TextStyle {
        TextStyle::new(self.item_size, self.item_size, FontWeight::Normal)
    }

    /// 固有尺寸。
    pub fn measure(&self) -> kanesumi_core::Size {
        kanesumi_core::Size::new(
            self.max_rating as f32 * self.item_size
                + (self.max_rating.saturating_sub(1)) as f32 * self.item_spacing,
            self.item_size,
        )
    }

    /// 第 k 个星 cell rect（1 基）。
    pub fn star_rect(&self, rect: Rect, k: usize) -> Rect {
        let x = rect.origin.x + (k as f32 - 1.0) * (self.item_size + self.item_spacing);
        Rect::new(x, rect.origin.y, self.item_size, self.item_size)
    }

    /// 命中星（1 基）。0 = 无。
    fn star_at(&self, rect: Rect, pos: Point) -> usize {
        for k in 1..=self.max_rating {
            if self.star_rect(rect, k).contains(pos) {
                return k;
            }
        }
        0
    }

    /// 悬停预览。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        if self.is_read_only {
            self.hover_value = None;
            return;
        }
        let k = self.star_at(rect, pos);
        self.hover_value = if k == 0 { None } else { Some(k as f64) };
    }

    /// 点击：设置值（含 Clear 语义）。返回新值（None = 未变化）。
    pub fn click(&mut self, rect: Rect, pos: Point) -> Option<f64> {
        if self.is_read_only {
            return None;
        }
        let k = self.star_at(rect, pos);
        if k == 0 {
            return None;
        }
        let new_value = if self.is_clear_enabled && (self.value.round() as usize) == k {
            0.0
        } else {
            k as f64
        };
        self.hover_value = None;
        if (new_value - self.value).abs() < 1e-9 {
            None
        } else {
            self.value = new_value;
            Some(new_value)
        }
    }

    /// 渲染：每星（空星底 + 实星覆盖，部分星裁剪）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        self.render_scaled(theme, engine, rect, scene, kanesumi_canvas::surface_scale());
    }

    /// [`render`](Self::render) 的显式倍率版本：2× 快照 / 导出走这里。
    ///
    /// `icons::draw` 内部即 `draw_scaled(.., surface_scale())`，只认外壳注入的当前表面缩放；
    /// 要在别的倍率下出图必须显式传 `scale`，否则 1× 光栅会被放大成 2×、边缘发虚。
    fn render_scaled(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
        scale: f32,
    ) {
        let colors = &theme.colors;
        // 预览优先，否则当前值。
        let shown = self.hover_value.unwrap_or(self.value);
        // 32 pt 的星形位图比 24 pt 星格大一圈（墨迹仍在格内），裁到控件矩形免得
        // 越界检查与相邻控件吃到它 —— 旧的字体星形本来就被文本框裁着。
        scene.push_clip(rect);
        for k in 1..=self.max_rating {
            let sr = self.star_rect(rect, k);
            // 星形按 ICON_32 出图、居中压进 24 pt 星格：母版墨迹 22.8×21.7，
            // 实测 alpha 包围盒 24×22（1×）/ 46×44（2×）。旧 e830107 的字体星形
            // 在 Noto CJK SC 24 pt 下是 22×16（DejaVu 19×17，Source Han 22×16）。
            // 用户裁定 C2 批准母版几何（参 Ether docs/DECISIONS_2026-11-17.md §265
            // 与 docs/CONTROL_SPEC.md §24）：满星 22×16 → 24×22 是明确的行为变更，
            // 不是等价门，也不是字号/线宽可调出来的。墨迹仍落在星格内。
            let glyph = Rect::new(
                sr.origin.x + (sr.size.width - ICON_32) / 2.0,
                sr.origin.y + (sr.size.height - ICON_32) / 2.0,
                ICON_32,
                ICON_32,
            );
            // 空星：L0 只有实心一枚，用不透明度表达「空」。旧版是字体空心星，轮廓线天然更淡；
            // 实心星压到 BaseMediumLow（0.4）才不至于看着像满星。只读态再乘非激活档
            // （CONTROL_SPEC §819「Disabled：低透」）。
            let dim = if self.is_read_only {
                theme.indication.inactive_opacity
            } else {
                1.0
            };
            draw_star(
                scene,
                glyph,
                colors.on_surface_variant,
                scale,
                theme.indication.base_medium_low * dim,
            );
            // 实星覆盖
            let fill_frac = (shown - (k as f64 - 1.0)).clamp(0.0, 1.0);
            if fill_frac > 0.0 {
                // 只读态连实星也压到非激活档；旧实现把 alpha 放在字体颜色里，
                // 位图路径必须改走命令的 opacity（见 `draw_star`）。
                let (filled_color, filled_opacity) = if self.is_read_only {
                    (colors.on_surface_variant, theme.indication.inactive_opacity)
                } else {
                    (colors.primary, 1.0)
                };
                // 全星：直接画
                if fill_frac >= 1.0 - 1e-6 {
                    draw_star(scene, glyph, filled_color, scale, filled_opacity);
                } else {
                    // 部分星：裁剪到 fill_frac 宽，复用同一枚光栅（缓存键不含 fill_frac）。
                    let clip = Rect::new(
                        sr.origin.x,
                        sr.origin.y,
                        sr.size.width * fill_frac as f32,
                        sr.size.height,
                    );
                    scene.push_clip(clip);
                    draw_star(scene, glyph, filled_color, scale, filled_opacity);
                    scene.pop_clip();
                }
            }
        }
        scene.pop_clip();
        let _ = engine;
    }
}

/// 画一枚星，并把不透明度写进 `SceneCommand::Image.opacity`。
///
/// 为什么不把 alpha 放进 tint：两条 Image 渲染路径都用
/// `tint.unwrap_or(WHITE).with_alpha(opacity)` 构造顶点色（参
/// `kanesumi-harness/src/render.rs` 的 `emit_image` 与
/// `kanesumi-render/src/cpu_raster.rs` 的同名函数），`tint.a` 会被覆盖；
/// 空星的 0.4 与只读态的非激活档只能落在 opacity 字段，否则空星与满星一样实。
fn draw_star(scene: &mut Scene, rect: Rect, color: Color, scale: f32, opacity: f32) {
    let start = scene.commands.len();
    icons::draw_scaled(scene, GlyphName::Star, rect, color, scale);
    if let Some(SceneCommand::Image { opacity: o, .. }) = scene.commands.get_mut(start) {
        *o = (*o * opacity.clamp(0.0, 1.0)).clamp(0.0, 1.0);
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）──────────────────
//
// 旧 API（`measure` / `hover` / `click` / `render`）原样保留给未迁移的 App。
// `PointerMove` 调 `hover` 预览，`PointerUp` 调 `click` 提交；`PointerLeave` 清悬停
// （旧 API 无专门方法，直接清 `hover_value`）。方向键 Left/Right 以整星步进。
// 命中与焦点交给框架。

/// 元素树动作：评分值被用户改变（点击 / 方向键）。携带新值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RatingValueChanged(pub f64);

impl MetroRatingControl {
    /// 方向键步进：从当前值（四舍五入）±1 星，夹紧 [0, max_rating]。只读时不变。
    fn step_by(&mut self, delta: f64) -> Option<f64> {
        if self.is_read_only {
            return None;
        }
        let old = self.value;
        self.value = (self.value.round() + delta).clamp(0.0, self.max_rating as f64);
        if (self.value - old).abs() < 1e-9 {
            None
        } else {
            Some(self.value)
        }
    }
}

impl kanesumi_element::Widget for MetroRatingControl {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        MetroRatingControl::measure(self)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                self.hover(ctx.rect(), *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hover_value = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                if let Some(v) = self.click(ctx.rect(), *pos) {
                    ctx.emit(RatingValueChanged(v));
                }
                ctx.invalidate_paint();
            }
            Event::KeyDown { key, .. } => {
                let delta = match key {
                    Key::Left => -1.0,
                    Key::Right => 1.0,
                    _ => return,
                };
                if let Some(v) = self.step_by(delta) {
                    ctx.emit(RatingValueChanged(v));
                    ctx.invalidate_paint();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        !self.is_read_only
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Other,
            name: String::new(),
            value: Some(format!("{}", self.value)),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(320.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroRatingControl::new(),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn star_center(h: &TestHarness, id: WidgetId, k: usize) -> Point {
        h.tree
            .get::<MetroRatingControl>(id)
            .unwrap()
            .star_rect(h.rect(id), k)
            .center()
    }

    #[test]
    fn click_sets_value_and_reports() {
        let (mut h, id) = harness();
        h.click_at(star_center(&h, id, 3));
        assert_eq!(
            h.take::<RatingValueChanged>(),
            vec![(id, RatingValueChanged(3.0))]
        );
        assert_eq!(h.tree.get::<MetroRatingControl>(id).unwrap().value, 3.0);
    }

    #[test]
    fn hover_previews_and_leave_clears() {
        let (mut h, id) = harness();
        h.move_to(star_center(&h, id, 4));
        h.frame();
        assert_eq!(
            h.tree.get::<MetroRatingControl>(id).unwrap().hover_value,
            Some(4.0)
        );
        h.tree.pointer_leave();
        h.frame();
        assert_eq!(
            h.tree.get::<MetroRatingControl>(id).unwrap().hover_value,
            None
        );
    }

    #[test]
    fn arrow_keys_step_stars_and_tab_focuses() {
        let (mut h, id) = harness();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Right);
        assert_eq!(h.tree.get::<MetroRatingControl>(id).unwrap().value, 1.0);
        h.key(Key::Right);
        h.key(Key::Left);
        assert_eq!(h.tree.get::<MetroRatingControl>(id).unwrap().value, 1.0);
        assert_eq!(h.take::<RatingValueChanged>().len(), 3);
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness();
        assert!(h.rect(id).size.width > 0.0 && h.rect(id).size.height > 0.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness();
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(star_center(&h, id, 2));
        assert!(h.take::<RatingValueChanged>().is_empty());
        assert_eq!(h.tree.get::<MetroRatingControl>(id).unwrap().value, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;
    use kanesumi_core::Color;
    use kanesumi_render::CpuRenderer;
    use kanesumi_render::glyph_layout::TextRenderTuning;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn rating() -> MetroRatingControl {
        MetroRatingControl::default()
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 200.0, 24.0)
    }

    #[test]
    fn defaults() {
        let r = rating();
        assert_eq!(r.max_rating, 5);
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn star_geometry() {
        let r = rating();
        let a = area();
        let s1 = r.star_rect(a, 1);
        let s2 = r.star_rect(a, 2);
        assert_eq!(s1.origin.x, 0.0);
        assert_eq!(s1.size.width, 24.0);
        assert_eq!(s2.origin.x, 24.0 + 4.0, "星间距 4");
    }

    #[test]
    fn click_sets_value() {
        let mut r = rating();
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), Some(3.0));
        assert_eq!(r.value, 3.0);
    }

    #[test]
    fn clear_enabled_resets() {
        let mut r = rating();
        r.is_clear_enabled = true;
        r.value = 3.0;
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), Some(0.0), "点当前值星清零");
        assert_eq!(r.value, 0.0);
    }

    #[test]
    fn read_only_no_change() {
        let mut r = rating();
        r.is_read_only = true;
        r.value = 2.0;
        let a = area();
        let s4 = r.star_rect(a, 4);
        assert_eq!(r.click(a, s4.center()), None);
        assert_eq!(r.value, 2.0);
    }

    #[test]
    fn click_same_value_no_change() {
        let mut r = rating();
        r.value = 3.0;
        let a = area();
        let s3 = r.star_rect(a, 3);
        assert_eq!(r.click(a, s3.center()), None, "同值不触发变更");
    }

    #[test]
    fn hover_sets_preview() {
        let mut r = rating();
        let a = area();
        let s4 = r.star_rect(a, 4);
        r.hover(a, s4.center());
        assert_eq!(r.hover_value, Some(4.0));
        r.hover(a, Point::new(1000.0, 1000.0));
        assert_eq!(r.hover_value, None);
    }

    /// 状态转录：2.5 星 → 5 张空星位图 + 3 张实星位图，半星裁剪，颜色/落格正确。
    #[test]
    fn render_emits_star_glyphs() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut r = rating();
        r.value = 2.5;
        let mut scene = Scene::default();
        r.render_scaled(&theme, &engine, area(), &mut scene, 1.0);

        // 不许再有字体星形 —— 字形本身是 L0 位图。
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 0, "评分不再写字体字形");

        let images: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Image {
                    rect,
                    tint,
                    width,
                    height,
                    opacity,
                    ..
                } => Some((*rect, *tint, *width, *height, *opacity)),
                _ => None,
            })
            .collect();
        assert_eq!(images.len(), 8, "5 空星 + 3 实星（2 整 + 1 半）");

        // 空星的颜色与不透明度都落在命令上：tint 是 on_surface_variant（不再塞 alpha，
        // Image 后端会覆盖 tint.a），0.4 走 opacity 字段。
        let empty = theme.colors.on_surface_variant;
        let empty_count = images
            .iter()
            .filter(|i| i.1 == Some(empty) && (i.4 - theme.indication.base_medium_low).abs() < 1e-6)
            .count();
        assert_eq!(empty_count, 5, "每星都先垫一张空星（opacity 0.4）");
        let filled_count = images
            .iter()
            .filter(|i| i.1 == Some(theme.colors.primary) && (i.4 - 1.0).abs() < 1e-6)
            .count();
        assert_eq!(filled_count, 3, "2 整星 + 1 半星取强调色、不透明");

        for (rect, _, width, height, _) in &images {
            assert_eq!(
                (rect.size.width, rect.size.height),
                (ICON_32, ICON_32),
                "按 32 母版出图"
            );
            assert_eq!((*width, *height), (32, 32), "1× 位图按 32 px 光栅");
        }
        // 1 基第 3 颗星位于 x=56，半星裁剪到一半宽。
        let clips: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::PushClip { rect } => Some(*rect),
                _ => None,
            })
            .collect();
        assert_eq!(clips.len(), 2, "外层控件 + 半星");
        assert_eq!(clips[0], area(), "控件不画出自己的矩形");
        assert_eq!(
            clips[1],
            Rect::new(56.0, 0.0, 12.0, 24.0),
            "裁剪到 fill_frac=0.5"
        );
    }

    /// 只读态：空星在 0.4 上再乘非激活档，实星也压到非激活档；两条都必须走 opacity。
    #[test]
    fn read_only_dims_both_star_layers() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let r = MetroRatingControl {
            max_rating: 1,
            value: 1.0,
            is_read_only: true,
            ..MetroRatingControl::default()
        };
        let mut scene = Scene::default();
        r.render_scaled(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 24.0, 24.0),
            &mut scene,
            1.0,
        );
        let ops: Vec<f32> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Image { opacity, .. } => Some(*opacity),
                _ => None,
            })
            .collect();
        assert_eq!(ops.len(), 2, "空星底 + 实星覆盖");
        assert!(
            (ops[0] - theme.indication.base_medium_low * theme.indication.inactive_opacity).abs()
                < 1e-6,
            "空星 = 0.4 × 非激活档：{}",
            ops[0]
        );
        assert!(
            (ops[1] - theme.indication.inactive_opacity).abs() < 1e-6,
            "实星 = 非激活档：{}",
            ops[1]
        );
    }

    /// 2× 输出：逻辑落格不变，位图按物理像素出（draw_scaled，不是 1× 放大）。
    #[test]
    fn render_scaled_at_two_x_keeps_logical_rect() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut r = rating();
        r.value = 1.0;
        let mut scene = Scene::default();
        r.render_scaled(&theme, &engine, area(), &mut scene, 2.0);

        let images: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Image {
                    rect,
                    width,
                    height,
                    ..
                } => Some((*rect, *width, *height)),
                _ => None,
            })
            .collect();
        assert!(!images.is_empty());
        for (rect, width, height) in images {
            assert_eq!(rect.size.width, ICON_32, "逻辑尺寸不随倍率变");
            assert_eq!((width, height), (64, 64), "2× 出 64 px 位图");
        }
    }

    /// 空星必须真的比满星淡，且颜色分别是中性灰与强调蓝。
    ///
    /// 只断言 `SceneCommand::Image` 存在/尺寸不够：两条 Image 后端都用
    /// `opacity` 而不是 `tint.a`，把 0.4 塞进 tint 会让空星与满星一样实。
    /// 这里在黑底上光栅，直接看像素。
    #[test]
    fn empty_star_is_dimmed_and_gray_in_pixels() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let rect = Rect::new(0.0, 0.0, 24.0, 24.0);
        let peak = |value: f64| -> ([u8; 4], u32) {
            let r = MetroRatingControl {
                max_rating: 1,
                value,
                ..MetroRatingControl::default()
            };
            let mut scene = Scene::default();
            scene.fill_rect(Color::BLACK, rect);
            r.render_scaled(&theme, &engine, rect, &mut scene, 1.0);
            let mut cpu = CpuRenderer::new(24.0, 24.0, 1.0);
            cpu.set_text_tuning(TextRenderTuning::identity());
            let bytes = cpu.render(&engine, &scene, None).to_vec();
            let mut best = ([0u8; 4], 0u32);
            for px in bytes.chunks_exact(4) {
                // 3/6/1 权重与探针 png_ascii 的亮度口径一致，只为挑最亮样本。
                let lum = 3 * px[0] as u32 + 6 * px[1] as u32 + px[2] as u32;
                if lum > best.1 {
                    best = ([px[0], px[1], px[2], px[3]], lum);
                }
            }
            best
        };

        let (empty, empty_lum) = peak(0.0);
        let (filled, _filled_lum) = peak(1.0);
        assert!(empty_lum > 0, "空星应有可见像素");
        // 0.4 是线性光的不透明度：on_surface_variant（154,160,166）压到 0.4 后
        // sRGB 约 100，而不是 154×0.4=62。若 tint.a 被忽略（满不透明），这里会是 154+。
        assert!(
            empty[0] < 130 && empty[1] < 135 && empty[2] < 140,
            "空星应压到约 0.4 线性不透明度：{empty:?}"
        );
        assert!(
            (empty[2] as i32 - empty[0] as i32).abs() <= 20,
            "空星应是中性灰：{empty:?}"
        );
        assert!(
            filled[2] >= 200 && filled[2] as i32 - filled[0] as i32 >= 120,
            "满星应是强调蓝且不透明：{filled:?}"
        );
    }

    /// 只保留指定 tint 的 Image 与其裁剪命令（颜色分离测半星）。
    fn filter_tint(scene: &Scene, tint: Color) -> Scene {
        let mut out = Scene::default();
        for cmd in &scene.commands {
            match cmd {
                SceneCommand::PushClip { .. } | SceneCommand::PopClip => {
                    out.commands.push(cmd.clone());
                }
                SceneCommand::Image { tint: Some(c), .. } if *c == tint => {
                    out.commands.push(cmd.clone())
                }
                _ => {}
            }
        }
        out
    }

    /// 指定逻辑区域里 alpha ≥ 16 的墨迹包围盒（物理像素，闭区间）。
    fn ink_bbox_in(
        bytes: &[u8],
        pw: u32,
        ph: u32,
        region: Rect,
        scale: f32,
    ) -> Option<(u32, u32, u32, u32)> {
        let x0 = (region.origin.x * scale).floor().max(0.0) as u32;
        let y0 = (region.origin.y * scale).floor().max(0.0) as u32;
        let x1 = ((region.right() * scale).ceil() as u32).min(pw);
        let y1 = ((region.bottom() * scale).ceil() as u32).min(ph);
        let (mut bx0, mut by0, mut bx1, mut by1) = (u32::MAX, u32::MAX, 0, 0);
        let mut any = false;
        for y in y0..y1 {
            for x in x0..x1 {
                if bytes[((y * pw + x) * 4 + 3) as usize] >= 16 {
                    any = true;
                    bx0 = bx0.min(x);
                    by0 = by0.min(y);
                    bx1 = bx1.max(x);
                    by1 = by1.max(y);
                }
            }
        }
        any.then_some((bx0, by0, bx1, by1))
    }

    /// 真实像素门：半星裁剪只作用本星，不污染后续命令；1×/2× 的半星宽度是整星一半。
    ///
    /// 3 星、值 1.5：第 1 颗满星、第 2 颗半星、第 3 颗空星。把「实星色」与「空星色」
    /// 分开光栅 —— 若半星的 PushClip 漏到后面，第 3 颗空星会被裁成半宽且第 3 颗实星
    /// 会画出半颗。
    #[test]
    fn half_star_clip_is_scoped_to_its_own_cell() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let r = MetroRatingControl {
            max_rating: 3,
            value: 1.5,
            ..MetroRatingControl::default()
        };
        let full = Rect::new(0.0, 0.0, 80.0, 24.0);
        let cell = |k: usize| Rect::new((k - 1) as f32 * 28.0, 0.0, 24.0, 24.0);
        let empty_color = theme.colors.on_surface_variant;

        for (scale, full_w, full_h, half_w) in
            [(1.0f32, 24u32, 22u32, 12u32), (2.0, 46, 44, 23)]
        {
            let mut scene = Scene::default();
            r.render_scaled(&theme, &engine, full, &mut scene, scale);

            let mut cpu = CpuRenderer::new(full.size.width, full.size.height, scale);
            cpu.set_text_tuning(TextRenderTuning::identity());
            let (pw, ph) = cpu.physical_size();

            let filled = filter_tint(&scene, theme.colors.primary);
            let bytes = cpu.render(&engine, &filled, None).to_vec();
            let b1 = ink_bbox_in(&bytes, pw, ph, cell(1), scale).expect("第 1 颗满星有墨");
            let b2 = ink_bbox_in(&bytes, pw, ph, cell(2), scale).expect("第 2 颗半星有墨");
            let b3 = ink_bbox_in(&bytes, pw, ph, cell(3), scale);
            assert_eq!(b1.2 - b1.0 + 1, full_w, "{scale}× 满星宽");
            assert_eq!(b1.3 - b1.1 + 1, full_h, "{scale}× 满星高");
            assert_eq!(b2.2 - b2.0 + 1, half_w, "{scale}× 半星只占半格");
            assert_eq!(b3, None, "{scale}× 半星裁剪不得漏到第 3 颗实星");

            let empty = filter_tint(&scene, empty_color);
            let bytes = cpu.render(&engine, &empty, None).to_vec();
            let e3 = ink_bbox_in(&bytes, pw, ph, cell(3), scale).expect("第 3 颗空星有墨");
            let e3_w = e3.2 - e3.0 + 1;
            // 空星带 0.4 不透明度，alpha≥16 的阈值会削掉最外圈抗锯齿，故允许比满星窄 ≤2 px；
            // 关键是绝不能是半星宽（12/23）。
            assert!(
                e3_w + 2 >= full_w && e3_w <= full_w,
                "{scale}× 后续空星不得被半星裁剪（宽 {e3_w}，满星 {full_w}）"
            );
        }
    }
}
