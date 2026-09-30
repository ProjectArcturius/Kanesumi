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

use crate::popup::{PopupAnim, PopupState, popup_gap};

/// 命令按钮尺寸（40×40）。
pub const COMMANDBAR_BUTTON_SIZE: f32 = 40.0;
/// 图标字号（16）。
pub const COMMANDBAR_ICON_SIZE: f32 = 16.0;
/// 边框厚度（1）。
pub const COMMANDBAR_BORDER: f32 = 1.0;
/// 文本选区命令（TextCommandBarFlyout 默认四命令）—— 保留供宿主拼 tooltip。
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
    ///
    /// **字形选择铁律**：图标必须落在 SourceHan（Ether 正体）已覆盖的 codepoint，
    /// 否则渲染成 .notdef 方框。历史 bug：`⧉` (U+29C9 数学符号) / `📋` (U+1F4CB emoji)
    /// 在 SourceHan 里没有 → gallery 命令条两块方框（用户报告 V22）。
    ///
    /// 现用 `C / X / V / A` 键盘助记字母：
    /// - ASCII 单字符，任何字体（含 fallback）保证渲染；
    /// - 与实际 Ctrl+X/C/V/A 快捷键完全对应，用户无需记图形；
    /// - 40×40 按钮里 14~16px 单字母居中，视觉重心稳定。
    pub fn text_command(idx: usize) -> Self {
        let (glyph, name, action) = match idx {
            0 => ("C", "复制", CommandBarAction::Copy),
            1 => ("X", "剪切", CommandBarAction::Cut),
            2 => ("V", "粘贴", CommandBarAction::Paste),
            _ => ("A", "全选", CommandBarAction::SelectAll),
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
                scene.fill_rect(theme.indication.hover_tint, btn);
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

// ── 元素树接入（弹层类面板，参 docs/ELEMENT_MIGRATION.md §8；模板同 menu_flyout.rs）──
//
// 旧路径里命令条是自持浮层：调用方 `open(anchor, screen)` 先算好 `panel_rect`，再自行渲染与命中，
// 宿主还得把整屏 `screen` 传进来。元素树里它是覆盖层上的「面板 Widget」：`measure` 报面板尺寸、
// `arrange` 记下自身矩形、`paint` 画面板；命令点击发 `CommandInvoked(idx)` 后 `close_popup` 自身。
// 由其它控件经 `open_below`（锚下缘）/ `open_at`（点锚定）打开。旧 `open(&mut self, …)` API 保留不动。

/// 元素树动作：命令条上第 `idx` 个命令被点击（对应 `commands[idx]`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInvoked(pub usize);

impl MetroCommandBarFlyout {
    /// 命中命令按钮索引（面板边框之内）。面板外或越过按钮序列末尾返回 None。
    fn command_index_at(&self, pos: Point) -> Option<usize> {
        if !self.panel_rect.contains(pos) {
            return None;
        }
        let local_x = pos.x - self.panel_rect.origin.x - COMMANDBAR_BORDER;
        if local_x < 0.0 {
            return None;
        }
        let idx = (local_x / COMMANDBAR_BUTTON_SIZE).floor() as usize;
        (idx < self.commands.len()).then_some(idx)
    }

    /// 由其它控件调用：锚在本控件下缘打开命令条（`keyboard` = 打开后把焦点移入面板）。
    pub fn open_below(
        ctx: &mut kanesumi_element::EventCtx,
        commands: Vec<CommandButton>,
        keyboard: bool,
    ) -> kanesumi_element::WidgetId {
        Self::open_with(ctx, commands, None, keyboard)
    }

    /// 由其它控件调用：在表面坐标 `at`（面板左上角）打开命令条 —— 调用方按选区自行定位。
    pub fn open_at(
        ctx: &mut kanesumi_element::EventCtx,
        commands: Vec<CommandButton>,
        at: Point,
        keyboard: bool,
    ) -> kanesumi_element::WidgetId {
        Self::open_with(ctx, commands, Some(at), keyboard)
    }

    fn open_with(
        ctx: &mut kanesumi_element::EventCtx,
        commands: Vec<CommandButton>,
        at: Option<Point>,
        keyboard: bool,
    ) -> kanesumi_element::WidgetId {
        let owner = ctx.id();
        let mut bar = Self::new(commands);
        bar.anim.open();
        let id = ctx.open_popup(
            bar,
            kanesumi_element::PopupSpec {
                anchor: Some(owner),
                at,
                gap: popup_gap(),
                ..kanesumi_element::PopupSpec::default()
            },
        );
        ctx.focus_widget(id, keyboard);
        id
    }
}

impl kanesumi_element::Widget for MetroCommandBarFlyout {
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        _available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        self.panel_size()
    }

    fn arrange(&mut self, _ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        self.panel_rect = rect;
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        self.render(&theme, ctx.engine(), scene);
        if matches!(self.state(), PopupState::Opening | PopupState::Closing) {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, dt: f64) {
        self.anim.update(dt);
        ctx.invalidate_paint();
        if matches!(self.state(), PopupState::Opening | PopupState::Closing) {
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                let before = self.hovered;
                self.hover(*pos);
                if self.hovered != before {
                    ctx.invalidate_paint();
                }
            }
            Event::PointerLeave if self.hovered.is_some() => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                // 命令点击：发动作并关闭自身（关闭后 owner 收 PopupClosed、焦点交还）。
                if let Some(idx) = self.command_index_at(*pos) {
                    ctx.emit(CommandInvoked(idx));
                    ctx.close_popup(ctx.id());
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 面板以悬停高亮表示当前命令，不画框架焦点框。
    fn focus_visual(&self) -> bool {
        false
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: String::from("命令条"),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_core::Size;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{
        Align, Event, EventCtx, Insets, Key, LayoutProps, MeasureCtx, PaintCtx, Widget, WidgetId,
    };

    /// 最小打开者：点击 / Enter 时打开命令条。`at` 为 Some 走点锚定，否则锚下缘。
    struct Opener {
        at: Option<Point>,
    }

    impl Widget for Opener {
        fn measure(&mut self, _: &mut MeasureCtx, _: Size) -> Size {
            Size::new(120.0, 32.0)
        }
        fn paint(&mut self, _: &mut PaintCtx, _: &mut Scene) {}
        fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
            let open = |ctx: &mut EventCtx, keyboard: bool, at: Option<Point>| {
                let cmds = CommandButton::text_command_bar();
                match at {
                    Some(p) => {
                        MetroCommandBarFlyout::open_at(ctx, cmds, p, keyboard);
                    }
                    None => {
                        MetroCommandBarFlyout::open_below(ctx, cmds, keyboard);
                    }
                }
            };
            match event {
                Event::Click => open(ctx, false, self.at),
                Event::KeyDown {
                    key: Key::Enter, ..
                } => {
                    open(ctx, true, self.at);
                    ctx.set_handled();
                }
                _ => {}
            }
        }
        fn focusable(&self) -> bool {
            true
        }
    }

    fn harness(at: Option<Point>) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(600.0, 400.0);
        let id = h.tree.insert_with(
            h.root(),
            Opener { at },
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                margin: Insets::new(20.0, 20.0, 0.0, 0.0),
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    fn only_popup(h: &TestHarness) -> WidgetId {
        let mut it = h.tree.popups();
        let p = it.next().expect("命令条应已打开");
        assert!(it.next().is_none(), "同一时刻只应有一个弹层");
        p
    }

    fn first_button_center(panel: Rect) -> Point {
        Point::new(
            panel.origin.x + COMMANDBAR_BORDER + COMMANDBAR_BUTTON_SIZE / 2.0,
            panel.origin.y + COMMANDBAR_BORDER + COMMANDBAR_BUTTON_SIZE / 2.0,
        )
    }

    #[test]
    fn click_opens_below_and_command_click_emits_and_closes() {
        let (mut h, owner) = harness(None);
        h.click(owner);
        let p = only_popup(&h);
        assert!(h.rect(p).origin.y >= h.rect(owner).bottom(), "命令条在锚点下方");
        h.click_at(first_button_center(h.rect(p)));
        assert_eq!(h.take::<CommandInvoked>(), vec![(p, CommandInvoked(0))]);
        assert!(h.tree.popups().next().is_none(), "点命令后关闭");
    }

    #[test]
    fn open_at_places_panel_top_left_at_point() {
        let at = Point::new(300.0, 120.0);
        let (mut h, owner) = harness(Some(at));
        h.click(owner);
        let p = only_popup(&h);
        let r = h.rect(p);
        assert!((r.origin.x - at.x).abs() < 0.5 && (r.origin.y - at.y).abs() < 0.5, "点锚定 {r:?}");
    }

    #[test]
    fn keyboard_open_focus_inside_escape_returns_focus() {
        let (mut h, owner) = harness(None);
        h.tab();
        assert_eq!(h.tree.focused(), Some(owner));
        h.key(Key::Enter);
        let p = only_popup(&h);
        assert_eq!(h.tree.focused(), Some(p), "键盘打开焦点入面板");
        h.key(Key::Escape);
        assert_eq!(h.tree.focused(), Some(owner), "Esc 关闭后焦点回打开者");
        assert!(h.tree.popups().next().is_none());
    }

    #[test]
    fn outside_click_dismisses() {
        let (mut h, owner) = harness(None);
        h.click(owner);
        assert!(h.tree.popups().next().is_some());
        h.click_at(Point::new(580.0, 390.0));
        assert!(h.tree.popups().next().is_none(), "点外部关闭");
    }

    #[test]
    fn passes_insurance_checks_while_open() {
        let (mut h, owner) = harness(None);
        h.click(owner);
        let p = only_popup(&h);
        h.assert_contained();
        h.assert_no_hit_outside(owner);
        h.assert_paint_within(owner, Insets::ZERO);
        h.assert_paint_within(p, Insets::ZERO);
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

    /// 选区贴左缘：面板左缘应夹紧到 screen.origin.x，不越屏左。
    #[test]
    fn place_clamps_left_edge() {
        let bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        // 选区紧贴左缘，中心 x=20（远小于半个命令条宽 82）
        let anchor = Rect::new(0.0, 300.0, 40.0, 20.0);
        let r = bar.place(anchor, screen);
        assert!(r.origin.x >= screen.origin.x - 0.01, "命令条不越屏左");
        assert_eq!(r.origin.x, 0.0, "贴屏左");
    }

    /// 屏幕比面板窄：min-clamp 生效（origin.x = screen.origin.x），面板宽保持。
    #[test]
    fn place_clamps_when_panel_wider_than_screen() {
        let bar = MetroCommandBarFlyout::text_commands();
        // 面板 4×40 + 2 = 162；屏 100 → 无法容纳
        let screen = Rect::new(0.0, 0.0, 100.0, 600.0);
        let anchor = Rect::new(20.0, 300.0, 40.0, 20.0);
        let r = bar.place(anchor, screen);
        assert_eq!(r.origin.x, 0.0, "面板宽 > 屏 → 靠左夹紧");
        assert_eq!(r.size.width, bar.panel_size().width, "宽保持（宿主负责裁剪 / 缩放）");
    }

    /// 多按钮布局（8 个自定义命令）—— panel_size 线性增长，命中覆盖全部按钮。
    #[test]
    fn many_buttons_layout_and_hit() {
        let cmds: Vec<CommandButton> = (0..8)
            .map(|i| CommandButton::new("●", format!("cmd{i}"), CommandBarAction::Custom(i)))
            .collect();
        let mut bar = MetroCommandBarFlyout::new(cmds);
        assert_eq!(bar.panel_size().width, 8.0 * 40.0 + 2.0);
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(300.0, 300.0, 200.0, 20.0);
        bar.panel_rect = bar.place(anchor, screen);
        // 命中每个按钮中心 —— 应各自返回对应 Custom(i)
        for i in 0..8 {
            let cx = bar.panel_rect.origin.x + COMMANDBAR_BORDER + (i as f32 + 0.5) * COMMANDBAR_BUTTON_SIZE;
            let p = Point::new(cx, bar.panel_rect.origin.y + 20.0);
            assert_eq!(
                bar.hit_command(p),
                Some(CommandBarAction::Custom(i)),
                "第 {i} 个按钮命中失败"
            );
        }
    }

    /// 命中越过按钮序列末尾（点面板内但超出最后一个按钮的 x 范围）应返回 None。
    /// 场景：面板宽含边框补偿，最右侧 1px 属于边框而非按钮。
    #[test]
    fn hit_command_returns_none_past_last_button() {
        let mut bar = MetroCommandBarFlyout::text_commands();
        let screen = Rect::new(0.0, 0.0, 800.0, 600.0);
        let anchor = Rect::new(200.0, 300.0, 120.0, 20.0);
        bar.panel_rect = bar.place(anchor, screen);
        // 越过最后按钮末尾（面板 right - 边框 = 命令条按钮区末尾）
        let past = Point::new(bar.panel_rect.right() + 5.0, bar.panel_rect.origin.y + 20.0);
        assert_eq!(bar.hit_command(past), None, "面板外应无命中");
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
