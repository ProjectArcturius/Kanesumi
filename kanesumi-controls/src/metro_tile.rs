// metro_tile.rs —— MetroTile（磁贴）。
//
// 磁贴 = 应用入口容器 + 动态内容宿主（Live Tile 类比）。参 Ether-main TILES_DESIGN.md：
// - §3 尺寸档：Mini 1×1 / Standard 2×2（默认）/ Large 4×2，只横向延长；
// - §5 图标：磁贴专用 glyph（透明大图形），`icon_tint` 染白（Lumia 磁贴风格）；
// - §6 颜色：单一基调色 `base_color`（Chorus harmonize 前可直用），状态色走主题 tokens。
// 本控件只负责**在给定 rect 内渲染**；rect 由网格（UniformGrid / TileWall）分配。

use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect};

use crate::state::ControlState;

/// 磁贴尺寸档（TILES_DESIGN §3）。网格单元 (列, 行)。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileSize {
    /// 1×1 迷你 —— 图标（居中）+ 可选徽标角标。
    Mini,
    /// 2×2 标准（默认）—— 图标 + 标题 + 单条预览。
    Standard,
    /// 4×2 更大 —— 图标 + 标题 + 内容行（最近邮件 / 最近照片 caption 等）。
    Large,
}

impl TileSize {
    /// 网格跨单元数 (col, row)。高恒 ≤ 2（TILES_DESIGN §2 硬约束）。
    pub const fn cells(self) -> (usize, usize) {
        match self {
            TileSize::Mini => (1, 1),
            TileSize::Standard => (2, 2),
            TileSize::Large => (4, 2),
        }
    }
}

/// 磁贴动态内容（Live Tile 类比，TILES_DESIGN §4 模板集）。
#[derive(Debug, Clone, PartialEq)]
pub enum TileLive {
    /// 无动态内容。
    None,
    /// 徽标角标（右下角叠加）。
    Badge(u32),
    /// 单条预览（2×2 标准）。
    Preview(String),
    /// 内容行（4×2 更大）：最近邮件主题 / 最近照片 caption。
    Lines(Vec<String>),
}

/// Win10 开始屏幕磁贴图标边长占磁贴**短边**的比例（标准磁贴图标约 40%）。
const TILE_ICON_RATIO: f32 = 0.4;
/// 标题 / 内容的内边距（Win10 左、下各 8px）。
const TILE_PAD: f32 = 8.0;

/// MetroTile —— 磁贴。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTile {
    pub size: TileSize,
    /// 基调色（TILES_DESIGN §6：manifest 单一 base_color，Chorus harmonize 前可直用）。
    pub base_color: Color,
    /// 磁贴专用图形（透明 glyph，TILES_DESIGN §5）。
    pub icon: Option<Icon>,
    /// 图标染色（None = 保留原色；默认白 = Lumia 磁贴风格）。
    pub icon_tint: Option<Color>,
    /// 标题。
    pub label: String,
    pub state: ControlState,
    /// 动态内容（Live Tile）。
    pub live: TileLive,
}

impl MetroTile {
    pub fn new(label: impl Into<String>, size: TileSize, base_color: Color) -> Self {
        Self {
            size,
            base_color,
            icon: None,
            icon_tint: Some(Color::WHITE),
            label: label.into(),
            state: ControlState::Normal,
            live: TileLive::None,
        }
    }

    /// builder：装载 SVG 磁贴图形（栅格化失败 → 保留 None，不 panic）。
    pub fn with_svg(mut self, path: impl AsRef<std::path::Path>, target: u32) -> Self {
        self.icon = Icon::load_svg(path, target);
        self
    }

    pub fn set_state(&mut self, state: ControlState) {
        self.state = state;
    }

    /// 命中测试。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染到 `rect`。顺序：基调色底 → 交互 tint → 图标（居中）→ 标题（左下）→ 内容 → 徽标（右下）。
    /// 版式按 Win10 开始屏幕：图标居中（占短边 40%），标题左下（内边距 8px，单行省略）；
    /// 小磁贴（Mini）只有图标无标题；宽磁贴（Large）图标居中、标题左下。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let indication = &theme.indication;
        let corner = theme.tokens.corner_radius;

        // 基调色底（纯色无渐变，Kanesumi 铁律 6）+ 状态 tint。
        scene.fill_rounded_rect(self.base_color, rect, corner);
        match self.state {
            ControlState::Hovered => scene.fill_rounded_rect(indication.hover_tint, rect, corner),
            ControlState::Pressed => scene.fill_rounded_rect(indication.press_tint, rect, corner),
            _ => {}
        }

        match self.size {
            TileSize::Mini => self.render_mini(rect, scene),
            TileSize::Standard => self.render_standard(theme, rect, scene),
            TileSize::Large => self.render_large(theme, rect, scene),
        }

        // 徽标（叠加右下角，任一档）。
        if let TileLive::Badge(count) = &self.live {
            self.render_badge(theme, engine, rect, *count, scene);
        }
    }

    /// 磁贴内图标矩形：居中，边长 = 短边 × [`TILE_ICON_RATIO`]（Win10 约 40%）。
    fn icon_rect(rect: Rect) -> Rect {
        let s = rect.size.width.min(rect.size.height) * TILE_ICON_RATIO;
        Rect::new(
            rect.origin.x + (rect.size.width - s) / 2.0,
            rect.origin.y + (rect.size.height - s) / 2.0,
            s,
            s,
        )
    }

    /// 标题 / 内容左起始 x 与可用宽度（左右各 [`TILE_PAD`]）。
    fn text_bounds(rect: Rect) -> (f32, f32, f32) {
        let x = rect.origin.x + TILE_PAD;
        (
            x,
            rect.size.width - TILE_PAD * 2.0,
            rect.bottom() - TILE_PAD,
        )
    }

    /// 迷你 1×1：仅居中图标，无标题（Win10 小磁贴）。
    fn render_mini(&self, rect: Rect, scene: &mut Scene) {
        self.render_icon(scene, Self::icon_rect(rect));
    }

    /// 标准 2×2：图标居中；标题左下（caption，单行省略）；单条预览叠在标题之上。
    fn render_standard(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        self.render_icon(scene, Self::icon_rect(rect));
        let caption = theme.typography.caption;
        let (text_x, content_w, title_bottom) = Self::text_bounds(rect);
        // 标题（caption）：左下角。
        scene.label(
            self.label.clone(),
            Rect::new(
                text_x,
                title_bottom - caption.line_height,
                content_w,
                caption.line_height,
            ),
            Color::WHITE,
            caption,
            TextAlign::Left,
        );
        // 单条预览（caption）：标题之上，次级不透明度。
        if let TileLive::Preview(text) = &self.live {
            scene.label(
                text.clone(),
                Rect::new(
                    text_x,
                    title_bottom - caption.line_height * 2.0,
                    content_w,
                    caption.line_height,
                ),
                Color::WHITE.with_alpha(theme.indication.secondary_opacity),
                caption,
                TextAlign::Left,
            );
        }
    }

    /// 更大 4×2：图标居中；标题左下；内容行自标题向上堆叠（最多 3 行）。
    fn render_large(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        self.render_icon(scene, Self::icon_rect(rect));
        let caption = theme.typography.caption;
        let (text_x, content_w, title_bottom) = Self::text_bounds(rect);
        // 标题（caption）：左下角。
        scene.label(
            self.label.clone(),
            Rect::new(
                text_x,
                title_bottom - caption.line_height,
                content_w,
                caption.line_height,
            ),
            Color::WHITE,
            caption,
            TextAlign::Left,
        );
        // 内容行（caption）：标题之上，最多 3 行。
        let rows = match &self.live {
            TileLive::Lines(lines) => lines.clone(),
            TileLive::Preview(p) => vec![p.clone()],
            _ => Vec::new(),
        };
        for (i, line) in rows.iter().take(3).enumerate() {
            let y = title_bottom - caption.line_height * (i as f32 + 2.0);
            scene.label(
                line.clone(),
                Rect::new(text_x, y, content_w, caption.line_height),
                Color::WHITE.with_alpha(theme.indication.secondary_opacity),
                caption,
                TextAlign::Left,
            );
        }
    }

    /// 图标渲染：glyph 等比缩放至 `rect` 内并居中（不裁切），无图标则跳过。
    /// 缩放不设 1.0 上限——磁贴要求图标占短边固定比例，调用方应提供足够分辨率。
    fn render_icon(&self, scene: &mut Scene, rect: Rect) {
        let Some(icon) = &self.icon else { return };
        let scale =
            (rect.size.width / icon.width as f32).min(rect.size.height / icon.height as f32);
        let w = icon.width as f32 * scale;
        let h = icon.height as f32 * scale;
        let r = Rect::new(
            rect.origin.x + (rect.size.width - w) / 2.0,
            rect.origin.y + (rect.size.height - h) / 2.0,
            w,
            h,
        );
        scene.image(icon, r, self.icon_tint);
    }

    /// 徽标：右下角小方块 + 白字数字（Win10 磁贴角标位）。
    fn render_badge(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        count: u32,
        scene: &mut Scene,
    ) {
        let badge = 20.0;
        let pad = 6.0;
        let br = Rect::new(
            rect.right() - badge - pad,
            rect.bottom() - badge - pad,
            badge,
            badge,
        );
        scene.fill_rounded_rect(theme.colors.primary, br, CornerRadius::Square);
        let text = count.to_string();
        let label = theme.typography.label;
        let w = engine.measure(&text, label.size);
        scene.text(
            text,
            Rect::new(
                br.origin.x + (br.size.width - w) / 2.0,
                br.origin.y + (br.size.height - label.line_height) / 2.0,
                w,
                label.line_height,
            ),
            theme.colors.on_primary,
            label,
            TextAlign::Left,
        );
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）────────────────
//
// 磁贴是「可点击的应用入口」。盘点：旧 `MetroTile` 只有 `state` + `render`，无翻转 / 实时
// 动画（`update` 无需实现）。迁移只把框架状态映射到 `state`、把点击 / Enter / Space
// 翻译成 `TileClicked`；命中、聚焦、hover / pressed 维护交给框架。

/// 元素树动作：磁贴被激活（点击 / Enter / Space）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileClicked;

/// 磁贴单元边长（与 gallery `TILE_CELL` 一致；TILES_DESIGN §3 三档尺寸按单元网格定义）。
const TREE_TILE_CELL: f32 = 64.0;
/// 磁贴单元间隔（与 gallery `TILE_GAP` 一致）。
const TREE_TILE_GAP: f32 = 8.0;

impl kanesumi_element::Widget for MetroTile {
    /// 固有尺寸 = 尺寸档跨单元数 × 单元边长 + 单元间隔（Mini 64、Standard 136、Large 280）。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let (cols, rows) = self.size.cells();
        kanesumi_core::Size::new(
            cols as f32 * TREE_TILE_CELL + cols.saturating_sub(1) as f32 * TREE_TILE_GAP,
            rows as f32 * TREE_TILE_CELL + rows.saturating_sub(1) as f32 * TREE_TILE_GAP,
        )
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let saved = self.state;
        self.state = crate::state::control_state(ctx.state());
        // §6 修正：压窄到图标（40）+ 标题行高之下时，标题 / 内容行会画到 `rect` 外缘；
        // 成对把自身绘制夹进 `rect`（旧行为：越界绘制 → 新行为：裁剪）。
        scene.push_clip(ctx.rect());
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        scene.pop_clip();
        self.state = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key};
        if matches!(
            event,
            Event::Click
                | Event::KeyDown {
                    key: Key::Enter | Key::Char(' '),
                    ..
                }
        ) {
            ctx.emit(TileClicked);
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Button,
            name: self.label.clone(),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(size: TileSize, props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroTile::new("邮件", size, Color::from_hex(0xFF_C8_42_3B)),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_and_keyboard_activate() {
        let (mut h, id) = harness(TileSize::Standard, LayoutProps::default());
        h.click(id);
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Enter);
        h.key(Key::Char(' '));
        assert_eq!(h.take::<TileClicked>().len(), 3);
    }

    #[test]
    fn measure_follows_tile_tier() {
        let (h, mini) = harness(TileSize::Mini, LayoutProps::default());
        assert_eq!(h.rect(mini).size, kanesumi_core::Size::new(64.0, 64.0));
        let (h2, large) = harness(TileSize::Large, LayoutProps::default());
        assert_eq!(h2.rect(large).size, kanesumi_core::Size::new(280.0, 136.0));
    }

    #[test]
    fn stays_within_when_squeezed() {
        let (mut h, id) = harness(
            TileSize::Standard,
            LayoutProps {
                width: Some(40.0),
                height: Some(40.0),
                ..LayoutProps::default()
            },
        );
        h.move_to(h.center(id));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_tile_ignores_activation() {
        let (mut h, id) = harness(TileSize::Standard, LayoutProps::default());
        h.tree.set_enabled(id, false);
        h.frame();
        h.click(id);
        assert!(h.take::<TileClicked>().is_empty());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn engine() -> TextEngine {
        TextEngine::load(find_font().expect("测试字体缺失，请设 KANESUMI_TEST_FONT")).unwrap()
    }

    fn tile() -> MetroTile {
        MetroTile::new("邮件", TileSize::Standard, Color::from_hex(0xFF_C8_42_3B))
    }

    fn render_scene(tile: &MetroTile, rect: Rect) -> Scene {
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        tile.render(&theme, &engine(), rect, &mut scene);
        scene
    }

    #[test]
    fn cells_map_to_spec_sizes() {
        assert_eq!(TileSize::Mini.cells(), (1, 1));
        assert_eq!(TileSize::Standard.cells(), (2, 2));
        assert_eq!(TileSize::Large.cells(), (4, 2));
    }

    /// 测试用 8×8 不透明白图（几何断言只需尺寸）。
    fn white_icon() -> Icon {
        Icon {
            rgba: vec![255u8; 8 * 8 * 4].into(),
            width: 8,
            height: 8,
        }
    }

    /// Win10 版式：图标居中且占短边 40%；标题左下（左 / 下内边距各 8）。
    #[test]
    fn win10_layout_icon_centered_title_bottom_left() {
        let theme = MetroTheme::ether_dark();
        let engine = engine();
        let rect = Rect::new(0.0, 0.0, 136.0, 136.0);
        let mut t = tile();
        t.icon = Some(white_icon());
        let mut scene = Scene::default();
        t.render(&theme, &engine, rect, &mut scene);

        let img = scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::Image { rect, .. } => Some(*rect),
                _ => None,
            })
            .expect("有图标 → Image 命令");
        assert!(
            (img.center().x - 68.0).abs() < 0.01 && (img.center().y - 68.0).abs() < 0.01,
            "图标居中: {:?}",
            img
        );
        assert!(
            (img.size.width - 136.0 * 0.4).abs() < 0.5,
            "图标占短边 40%: {}",
            img.size.width
        );

        let title = scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::Text { content, rect, .. } if content == "邮件" => Some(*rect),
                _ => None,
            })
            .expect("标题");
        assert!((title.origin.x - 8.0).abs() < 0.01, "标题左内边距 8");
        assert!((title.bottom() - 128.0).abs() < 0.01, "标题底内边距 8");
    }

    /// 小磁贴（Mini）只有居中图标，无标题。
    #[test]
    fn mini_has_icon_only() {
        let theme = MetroTheme::ether_dark();
        let engine = engine();
        let mut t = MetroTile::new("音乐", TileSize::Mini, Color::from_hex(0xFF_4C_A0_5E));
        t.icon = Some(white_icon());
        let mut scene = Scene::default();
        t.render(&theme, &engine, Rect::new(0.0, 0.0, 64.0, 64.0), &mut scene);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Image { .. })),
            "Mini 有图标"
        );
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "音乐")),
            "Mini 无标题"
        );
    }

    /// 徽标角标在右下角（不再右上）。
    #[test]
    fn badge_sits_bottom_right() {
        let mut t = tile();
        t.live = TileLive::Badge(12);
        let scene = render_scene(&t, Rect::new(0.0, 0.0, 136.0, 136.0));
        // 徽标底块是最后一个 FillRect（括号角标 20px，右 / 下各留 6px）。
        let badge = scene
            .commands
            .iter()
            .rev()
            .find_map(|c| match c {
                SceneCommand::FillRect { rect, .. } => Some(*rect),
                _ => None,
            })
            .expect("徽标底块");
        assert!((badge.right() - (136.0 - 6.0)).abs() < 0.01, "右内边距 6");
        assert!((badge.bottom() - (136.0 - 6.0)).abs() < 0.01, "底内边距 6");
    }

    #[test]
    fn renders_base_fill_and_label() {
        let scene = render_scene(&tile(), Rect::new(0.0, 0.0, 136.0, 136.0));
        assert!(matches!(
            scene.commands[0],
            SceneCommand::FillRect { .. }
        ), "首命令为基调色底");
        assert!(
            scene.commands.iter().any(|c| matches!(
                c,
                SceneCommand::Text { content, .. } if content == "邮件"
            )),
            "标题文本"
        );
    }

    #[test]
    fn hover_state_overlays_tint() {
        let mut t = tile();
        t.set_state(ControlState::Hovered);
        let scene = render_scene(&t, Rect::new(0.0, 0.0, 136.0, 136.0));
        assert!(scene.commands.len() >= 3, "底 + hover tint + 标题");
    }

    #[test]
    fn badge_renders_count() {
        let mut t = tile();
        t.live = TileLive::Badge(12);
        let scene = render_scene(&t, Rect::new(0.0, 0.0, 136.0, 136.0));
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "12")),
            "徽标数字"
        );
    }

    #[test]
    fn large_renders_live_lines() {
        let mut t = tile();
        t.size = TileSize::Large;
        t.live = TileLive::Lines(vec![
            "年度报告".into(),
            "季度账单".into(),
            "活动邀请".into(),
        ]);
        let scene = render_scene(&t, Rect::new(0.0, 0.0, 280.0, 136.0));
        for line in ["年度报告", "季度账单", "活动邀请"] {
            assert!(
                scene.commands.iter().any(|c| matches!(
                    c,
                    SceneCommand::Text { content, .. } if content == line
                )),
                "内容行 {line}"
            );
        }
    }

    #[test]
    fn hit_test_contains() {
        let t = tile();
        let rect = Rect::new(10.0, 10.0, 136.0, 136.0);
        assert!(t.hit_test(rect, Point::new(50.0, 50.0)));
        assert!(!t.hit_test(rect, Point::new(200.0, 200.0)));
    }

    #[test]
    fn missing_icon_skips_image_command() {
        let scene = render_scene(&tile(), Rect::new(0.0, 0.0, 136.0, 136.0));
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Image { .. })),
            "无图标不产出 Image 命令"
        );
    }
}
