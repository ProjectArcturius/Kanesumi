// MetroCommandBarFlyout —— 选中文本浮出命令条。参 CONTROL_SPEC §40（CommandBarFlyout 参考，开源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/CommandBarFlyout/`（CommandBarFlyout.cpp + 模板）：
// - 本质 = Flyout 包一个横向 CommandBar（AppBarButton 序列），文本选区选中时浮出；
// - 按钮 40×40（CommandBarFlyoutAppBarButtonStyleBase Width/Height=40），图标 16；
// - BorderThickness 1（CommandBarFlyoutBorderThemeThickness）；底色系统 chrome；
// - TextCommandBarFlyout 默认命令：Copy / Cut / Paste / Select All；
// - 轻量 dismiss（无遮罩，UWP 浮出工具栏不压暗背景）。
//
// Kanesumi 实现：命令 = 图标字形（思源黑体）+ 名称 + 动作回传。无遮罩（不同于 DropdownMenu）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::popup::{PopupAnim, PopupState};

/// 命令按钮尺寸（40×40）。
pub const COMMANDBAR_BUTTON_SIZE: f32 = 40.0;
/// 图标字号（16）。
pub const COMMANDBAR_ICON_SIZE: f32 = 16.0;
/// 边框厚度（1）。
pub const COMMANDBAR_BORDER: f32 = 1.0;
/// 文本选区命令（TextCommandBarFlyout 默认四命令）。
pub const TEXT_COMMANDS: [&str; 4] = ["复制", "剪切", "粘贴", "全选"];

/// 命令条动作 —— 宿主据此执行对应文本操作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandBarAction {
    Copy,
    Cut,
    Paste,
    SelectAll,
    Custom(usize),
}

/// 单个命令按钮。
#[derive(Debug, Clone, PartialEq)]
pub struct CommandButton {
    /// 图标字形（思源黑体字符，无 MDL2 依赖，参 V7）。
    pub glyph: String,
    /// 名称（tooltip / 无障碍）。
    pub name: String,
    /// 动作。
    pub action: CommandBarAction,
}

impl CommandButton {
    pub fn new(glyph: impl Into<String>, name: impl Into<String>, action: CommandBarAction) -> Self {
        Self {
            glyph: glyph.into(),
            name: name.into(),
            action,
        }
    }

    /// 标准文本命令构造（Copy/Cut/Paste/SelectAll）。
    pub fn text_command(idx: usize) -> Self {
        let (glyph, name, action) = match idx {
            0 => ("⧉", "复制", CommandBarAction::Copy), // 双矩形（Copy）
            1 => ("✂", "剪切", CommandBarAction::Cut),
            2 => ("📋", "粘贴", CommandBarAction::Paste), // 占位，可用字形替换
            _ => ("■", "全选", CommandBarAction::SelectAll),
        };
        Self::new(glyph, name, action)
    }

    /// 默认文本命令序列（Copy / Cut / Paste / Select All）。
    pub fn text_command_bar() -> Vec<CommandButton> {
        (0..4).map(Self::text_command).collect()
    }
}

/// MetroCommandBarFlyout —— 选中文本浮出命令条。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroCommandBarFlyout {
    /// 命令按钮序列。
    pub commands: Vec<CommandButton>,
    /// 命令条面板矩形（相对屏幕）。
    pub panel_rect: Rect,
    /// 悬停按钮。
    pub hovered: Option<usize>,
    /// 动画。
    pub anim: PopupAnim,
}

impl MetroCommandBarFlyout {
    pub fn new(commands: Vec<CommandButton>) -> Self {
        Self {
            commands,
            panel_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            hovered: None,
            anim: PopupAnim::new(),
        }
    }

    /// 默认文本命令条（Copy/Cut/Paste/SelectAll）。
    pub fn text_commands() -> Self {
        Self::new(CommandButton::text_command_bar())
    }

    /// 在 `anchor`（选中矩形）下方打开命令条。命令条宽 = 按钮数 × 40 + 边框。
    pub fn open(&mut self, anchor: Rect, screen: Rect) {
        self.panel_rect = self.place(anchor, screen);
        self.anim.open();
    }

    pub fn close(&mut self) {
        self.anim.close();
    }

    /// 命令条尺寸。
    pub fn panel_size(&self) -> kanesumi_core::Size {
        let w = self.commands.len() as f32 * COMMANDBAR_BUTTON_SIZE + 2.0 * COMMANDBAR_BORDER;
        let h = COMMANDBAR_BUTTON_SIZE + 2.0 * COMMANDBAR_BORDER;
        kanesumi_core::Size::new(w, h)
    }

    /// 定位：命令条在选中矩形**上方**（TextCommandBarFlyout 默认贴选区上缘），
    /// 水平居中于选区；左右收拢不越出屏幕。上方空间不足则翻到下方。
    pub fn place(&self, anchor: Rect, screen: Rect) -> Rect {
        let size = self.panel_size();
        let gap = 4.0;
        let above = anchor.origin.y - gap;
        let y = if above >= size.height {
            above - size.height
        } else {
            anchor.bottom() + gap
        };
        // 水平居中，收拢到屏幕内
        let center_x = anchor.origin.x + anchor.size.width / 2.0;
        let mut x = center_x - size.width / 2.0;
        if x < screen.origin.x {
            x = screen.origin.x;
        }
        if x + size.width > screen.right() {
            x = (screen.right() - size.width).max(screen.origin.x);
        }
        Rect::new(x, y, size.width, size.height)
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
    }

    pub fn state(&self) -> PopupState {
        self.anim.state()
    }

    pub fn is_visible(&self) -> bool {
        self.anim.is_visible()
    }

    /// 命中命令按钮 → 动作。
    pub fn hit_command(&self, pos: Point) -> Option<CommandBarAction> {
        if !self.panel_rect.contains(pos) {
            return None;
        }
        let local_x = pos.x - self.panel_rect.origin.x;
        let idx = ((local_x - COMMANDBAR_BORDER) / COMMANDBAR_BUTTON_SIZE).floor() as usize;
        self.commands.get(idx).map(|c| c.action)
    }

    /// 悬停路由。
    pub fn hover(&mut self, pos: Point) {
        self.hovered = if self.panel_rect.contains(pos) {
            let local_x = pos.x - self.panel_rect.origin.x;
            let idx = ((local_x - COMMANDBAR_BORDER) / COMMANDBAR_BUTTON_SIZE).floor() as usize;
            if idx < self.commands.len() {
                Some(idx)
            } else {
                None
            }
        } else {
            None
        };
    }

    /// 渲染：面板底 + 边框 + 各命令按钮（图标 + 悬停高亮）。
    pub fn render(&self, theme: &MetroTheme, _engine: &TextEngine, scene: &mut Scene) {
        if !self.anim.is_visible() {
            return;
        }
        let colors = &theme.colors;
        // 面板底（chrome → surface_variant，浅于页面）
        scene.fill_rounded_rect(
            colors.surface_variant,
            self.panel_rect,
            theme.tokens.corner_radius,
        );
        scene.stroke_rounded_rect(
            colors.divider,
            self.panel_rect,
            COMMANDBAR_BORDER,
            theme.tokens.corner_radius,
        );

        let style = theme.typography.body;
        for (i, cmd) in self.commands.iter().enumerate() {
            let btn = Rect::new(
                self.panel_rect.origin.x + COMMANDBAR_BORDER + i as f32 * COMMANDBAR_BUTTON_SIZE,
                self.panel_rect.origin.y + COMMANDBAR_BORDER,
                COMMANDBAR_BUTTON_SIZE,
                COMMANDBAR_BUTTON_SIZE,
            );
            if self.hovered == Some(i) {
                // AppBarButton PointerOver = HighlightListLow（白 10%）
                scene.fill_rect(colors.on_surface.with_alpha(0.10), btn);
            }
            // 图标（16px 居中）
            scene.text(
                cmd.glyph.clone(),
                btn,
                colors.on_surface,
                style,
                TextAlign::Center,
            );
        }
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

    fn font_available() -> bool {
        find_font().is_some()
    }

    #[test]
    fn default_text_commands() {
        let bar = MetroCommandBarFlyout::text_commands();
        assert_eq!(bar.commands.len(), 4);
        assert_eq!(bar.commands[0].action, CommandBarAction::Copy);
        assert_eq!(bar.commands[1].action, CommandBarAction::Cut);
        assert_eq!(bar.commands[2].action, CommandBarAction::Paste);
        assert_eq!(bar.commands[3].action, CommandBarAction::SelectAll);
    }

    #[test]
    fn panel_size_scales_with_commands() {
        let one = MetroCommandBarFlyout::new(vec![CommandButton::new("A", "a", CommandBarAction::Custom(0))]);
        let four = MetroCommandBarFlyout::text_commands();
        assert!(four.panel_size().width > one.panel_size().width);
        assert_eq!(four.panel_size().width, 4.0 * 40.0 + 2.0);
        assert_eq!(four.panel_size().height, 40.0 + 2.0);
    }

    #[test]
    fn place_above_anchor_centered() {
        let bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(200.0, 300.0, 120.0, 20.0);
        let r = bar.place(anchor, screen);
        // 居中：命令条中心 ≈ 选区中心
        assert!((r.center().x - anchor.center().x).abs() < 1.0, "水平居中");
        // 上方：下缘 ≈ 选区上缘 - 4
        assert!((r.bottom() - (anchor.origin.y - 4.0)).abs() < 1.0, "贴选区上缘");
        // 面板在屏幕内
        assert!(r.origin.x >= 0.0 && r.right() <= screen.right());
    }

    #[test]
    fn place_flips_below_when_no_room_above() {
        let bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(200.0, 10.0, 120.0, 20.0); // 紧贴顶部
        let r = bar.place(anchor, screen);
        assert!(r.origin.y >= anchor.bottom() + 4.0 - 0.01, "上方不足翻到下方");
    }

    #[test]
    fn place_clamps_right_edge() {
        let bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 300.0, 600.0);
        let anchor = Rect::new(250.0, 300.0, 40.0, 20.0); // 右缘
        let r = bar.place(anchor, screen);
        assert!(r.right() <= screen.right() + 0.01, "命令条右缘不越屏");
    }

    #[test]
    fn hit_command_maps_buttons() {
        let mut bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(200.0, 300.0, 120.0, 20.0);
        bar.panel_rect = bar.place(anchor, screen);
        let panel = bar.panel_rect;
        // 第 2 个按钮（index 1）中心
        let p = Point::new(
            panel.origin.x + 1.0 + 1.5 * 40.0,
            panel.origin.y + 20.0,
        );
        assert_eq!(bar.hit_command(p), Some(CommandBarAction::Cut));
        // 面板外
        assert_eq!(bar.hit_command(Point::new(5.0, 5.0)), None);
    }

    #[test]
    fn hover_tracks_button() {
        let mut bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(200.0, 300.0, 120.0, 20.0);
        bar.panel_rect = bar.place(anchor, screen);
        let p = Point::new(bar.panel_rect.origin.x + 1.0 + 0.5 * 40.0, bar.panel_rect.origin.y + 20.0);
        bar.hover(p);
        assert_eq!(bar.hovered, Some(0));
        bar.hover(Point::new(5.0, 5.0));
        assert_eq!(bar.hovered, None);
    }

    #[test]
    fn open_close_anim() {
        let mut bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        bar.open(Rect::new(100.0, 100.0, 80.0, 20.0), screen);
        assert!(bar.anim.is_visible());
        for _ in 0..120 {
            bar.update(1.0 / 60.0);
        }
        assert_eq!(bar.state(), PopupState::Open);
        bar.close();
        for _ in 0..120 {
            bar.update(1.0 / 60.0);
        }
        assert_eq!(bar.state(), PopupState::Closed);
    }

    #[test]
    fn render_emits_panel_and_icons() {
        if !font_available() {
            return;
        }
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        bar.open(Rect::new(200.0, 300.0, 120.0, 20.0), screen);
        bar.update(1.0);
        let mut scene = Scene::default();
        bar.render(&theme, &engine, &mut scene);
        // 面板底 + 边框 + 4 图标
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 1, "一个面板底");
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        assert_eq!(texts, 4, "4 个命令图标");
    }
}
