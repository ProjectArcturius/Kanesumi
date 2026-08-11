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
    /// 徽标角标（1×1 迷你 / 叠加右上角）。
    Badge(u32),
    /// 单条预览（2×2 标准）。
    Preview(String),
    /// 内容行（4×2 更大）：最近邮件主题 / 最近照片 caption。
    Lines(Vec<String>),
}

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

    /// 渲染到 `rect`。顺序：基调色底 → 交互 tint → 图标 → 标题 → 动态内容 → 徽标。
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

        // 徽标（叠加右上角，1×1 或任一档）。
        if let TileLive::Badge(count) = &self.live {
            self.render_badge(theme, engine, rect, *count, scene);
        }
    }

    /// 迷你 1×1：图标居中（40×40），无标题。
    fn render_mini(&self, rect: Rect, scene: &mut Scene) {
        let icon_size = 40.0;
        let ir = Rect::new(
            rect.origin.x + (rect.size.width - icon_size) / 2.0,
            rect.origin.y + (rect.size.height - icon_size) / 2.0,
            icon_size,
            icon_size,
        );
        self.render_icon(scene, ir);
    }

    /// 标准 2×2：图标 40 左上，标题（body）在下，单条预览（caption）再下。
    fn render_standard(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let pad = 8.0;
        let body = theme.typography.body;
        let caption = theme.typography.caption;

        let icon = Rect::new(rect.origin.x + pad, rect.origin.y + pad, 40.0, 40.0);
        self.render_icon(scene, icon);

        let text_x = rect.origin.x + pad;
        let content_w = rect.size.width - pad * 2.0;
        // 标题（body）：icon 之下。
        scene.text(
            self.label.clone(),
            Rect::new(text_x, icon.bottom() + 6.0, content_w, body.line_height),
            Color::WHITE,
            body,
            TextAlign::Left,
        );
        // 单条预览（caption）：标题之下。
        if let TileLive::Preview(text) = &self.live {
            let y = icon.bottom() + 6.0 + body.line_height + 2.0;
            scene.text(
                text.clone(),
                Rect::new(text_x, y, content_w, caption.line_height),
                Color::WHITE.with_alpha(0.8),
                caption,
                TextAlign::Left,
            );
        }
    }

    /// 更大 4×2：图标 48 左上，标题 + 内容行在右侧。
    fn render_large(&self, theme: &MetroTheme, rect: Rect, scene: &mut Scene) {
        let pad = 8.0;
        let body = theme.typography.body;
        let caption = theme.typography.caption;

        let icon = Rect::new(rect.origin.x + pad, rect.origin.y + pad, 48.0, 48.0);
        self.render_icon(scene, icon);

        let text_x = icon.right() + 12.0;
        let content_w = rect.right() - pad - text_x;
        // 标题（body）。
        scene.text(
            self.label.clone(),
            Rect::new(text_x, icon.origin.y, content_w, body.line_height),
            Color::WHITE,
            body,
            TextAlign::Left,
        );
        // 内容行（caption）：标题之下，最多 3 行。
        let rows = match &self.live {
            TileLive::Lines(lines) => lines.clone(),
            TileLive::Preview(p) => vec![p.clone()],
            _ => Vec::new(),
        };
        for (i, line) in rows.iter().take(3).enumerate() {
            let y = icon.origin.y + body.line_height + 4.0 + i as f32 * caption.line_height;
            scene.text(
                line.clone(),
                Rect::new(text_x, y, content_w, caption.line_height),
                Color::WHITE.with_alpha(0.8),
                caption,
                TextAlign::Left,
            );
        }
    }

    /// 图标渲染：染白 glyph 居中缩放（不裁切），无图标则跳过。
    fn render_icon(&self, scene: &mut Scene, rect: Rect) {
        let Some(icon) = &self.icon else { return };
        // 等比缩放至 rect 内（透明 glyph 按目标边缩放）。
        let scale = (rect.size.width / icon.width as f32)
            .min(rect.size.height / icon.height as f32)
            .min(1.0);
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

    /// 徽标：右上角小方块 + 白字数字。
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
        let br = Rect::new(rect.right() - badge - pad, rect.origin.y + pad, badge, badge);
        scene.fill_rounded_rect(theme.colors.primary, br, CornerRadius::Slight);
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
