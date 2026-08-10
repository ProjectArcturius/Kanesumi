// GalleryApp —— 三层测试阶梯的 daily driver（参 Ether-main PLAN.md §4.4）。
//
// 实现 `App` trait：状态驱动渲染 + 输入路由（参 HANDOVER §2 输入层）。
// 事件路由：顶层弹层优先（Dialog/DropdownMenu/SelectorFlyout）→ 常规控件。
// 控件状态切换：set_state / set_checked / hovered / show / hide / toggle。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_controls::{
    ControlState, MenuItem, MetroButton, MetroDialog, MetroDropdownMenu, MetroIconButton,
    MetroList, MetroProgressBar, MetroProgressRing, MetroSelectorFlyout, MetroSwitch, MetroTab,
    MetroTabRow,
};
use kanesumi_core::{MetroTheme, Point, Rect, Size, TextStyle};
use kanesumi_harness::{App, AppConfig, EtherRole, InputEvent, PointerButton};

/// 布局常量（逻辑像素）。
const PAD: f32 = 16.0;
const TITLE_H: f32 = 36.0;
const CTRL_Y0: f32 = 44.0;

/// 交互目标 —— 常规控件命中标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    Button,
    Accent,
    Icon,
    Switch,
    Tabs,
    List,
    Dropdown,
    Selector,
}

/// Gallery 应用状态。
pub struct GalleryApp {
    theme: MetroTheme,
    engine: TextEngine,
    config: AppConfig,

    // 控件
    button: MetroButton,
    accent: MetroButton,
    icon: MetroIconButton,
    switch: MetroSwitch,
    bar: MetroProgressBar,
    ring: MetroProgressRing,
    tabs: MetroTabRow,
    list: MetroList,
    dropdown: MetroDropdownMenu,
    selector: MetroSelectorFlyout,
    dialog: MetroDialog,

    // 输入状态
    hovered: Option<Target>,
    pressed: Option<Target>,
    /// 最近一次指针位置（滚轮路由需要，因为 Scroll 不带坐标）。
    pointer: Point,
    /// 最近一次对话框按钮动作（Primary/Secondary/Close），供应用响应。
    dialog_result: Option<kanesumi_controls::DialogButton>,
}

impl GalleryApp {
    /// 从字体路径构造。字体与外壳同源（App::font_path + 外壳 TextEngine）。
    pub fn new(font_path: impl AsRef<std::path::Path>) -> Self {
        let engine = TextEngine::load(font_path).expect("Gallery 字体加载失败");
        Self::with_engine(engine)
    }

    /// 直接注入 TextEngine（外壳已加载同源字体）。
    pub fn with_engine(engine: TextEngine) -> Self {
        let theme = MetroTheme::ether_dark();
        Self {
            theme,
            engine,
            config: AppConfig::new(
                "org.ether.gallery",
                "Kanesumi Gallery",
                EtherRole::Browser,
                960.0,
                600.0,
            ),
            button: MetroButton::new("Standard"),
            accent: MetroButton::accent("打开对话框"),
            icon: MetroIconButton::with_label("\u{E72D}", "Share"),
            switch: MetroSwitch::with_label("飞行模式"),
            bar: MetroProgressBar::indeterminate(),
            ring: MetroProgressRing::new(),
            tabs: MetroTabRow::new(vec![
                MetroTab::new("邮件"),
                MetroTab::new("日历"),
                MetroTab::new("人脉"),
            ]),
            list: MetroList::new(
                [
                    "Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta", "Eta", "Theta", "Iota",
                    "Kappa",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            ),
            dropdown: MetroDropdownMenu::new(vec![
                MenuItem::new("新建"),
                MenuItem::with_icon("打开", "\u{E8E5}"),
                MenuItem::with_icon("保存", "\u{E74E}"),
                MenuItem::with_icon("另存为...", "\u{E792}").separator(),
                MenuItem::with_icon("退出", "\u{E7E8}"),
            ]),
            selector: MetroSelectorFlyout::new(
                ["紧凑", "舒适", "宽松"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            dialog: {
                let mut d = MetroDialog::new("保存工作？", "是否保存对当前文件的更改？");
                d.buttons.primary = Some("保存".into());
                d.buttons.secondary = Some("不保存".into());
                d.buttons.close = Some("取消".into());
                d.buttons.default_button = kanesumi_controls::DialogDefaultButton::Primary;
                d
            },
            hovered: None,
            pressed: None,
            pointer: Point::ORIGIN,
            dialog_result: None,
        }
    }

    // ── 布局矩形 ────────────────────────────────────────────────────────

    fn button_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0, 96.0, 38.0)
    }
    fn accent_rect(&self) -> Rect {
        Rect::new(PAD + 104.0, CTRL_Y0, 130.0, 38.0)
    }
    fn icon_rect(&self) -> Rect {
        Rect::new(PAD + 250.0, CTRL_Y0 - 4.0, 68.0, 56.0)
    }
    fn switch_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 44.0, 200.0, 40.0)
    }
    fn bar_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 92.0, 300.0, 4.0)
    }
    fn ring_rect(&self) -> Rect {
        Rect::new(PAD + 320.0, CTRL_Y0 + 84.0, 32.0, 32.0)
    }
    fn tabs_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 108.0, 420.0, 48.0)
    }
    fn list_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 168.0, 260.0, 280.0)
    }
    fn dropdown_trigger(&self) -> Rect {
        Rect::new(PAD + 280.0, CTRL_Y0 + 168.0, 130.0, 32.0)
    }
    fn selector_trigger(&self) -> Rect {
        Rect::new(PAD + 280.0, CTRL_Y0 + 214.0, 180.0, 32.0)
    }

    /// 弹层面板锚点（方向自适应：下方空间不足时上翻，参 CONTROL_SPEC §8）。
    fn dropdown_panel(&self) -> Rect {
        let t = self.dropdown_trigger();
        let size = self.dropdown.panel_size(&self.engine);
        kanesumi_controls::place_popup(t, size, self.screen(), kanesumi_controls::popup_gap()).rect
    }
    fn selector_panel(&self) -> Rect {
        let t = self.selector_trigger();
        let size = kanesumi_core::Size::new(t.size.width, self.selector.panel_height());
        kanesumi_controls::place_popup(t, size, self.screen(), kanesumi_controls::popup_gap()).rect
    }

    /// Gallery 全屏窗口（供弹层方向自适应用）。
    fn screen(&self) -> Rect {
        Rect::new(0.0, 0.0, self.config.width, self.config.height)
    }

    /// 命中常规控件（弹层优先，由调用方处理）。
    fn hit_regular(&self, p: Point) -> Option<Target> {
        if self.button_rect().contains(p) {
            Some(Target::Button)
        } else if self.accent_rect().contains(p) {
            Some(Target::Accent)
        } else if self.icon_rect().contains(p) {
            Some(Target::Icon)
        } else if self.switch_rect().contains(p) {
            Some(Target::Switch)
        } else if self.tabs_rect().contains(p) {
            Some(Target::Tabs)
        } else if self.list_rect().contains(p) {
            Some(Target::List)
        } else if self.dropdown_trigger().contains(p) {
            Some(Target::Dropdown)
        } else if self.selector_trigger().contains(p) {
            Some(Target::Selector)
        } else {
            None
        }
    }

    /// 更新悬停态（Motion / Enter）。
    fn update_hover(&mut self, p: Point) {
        self.pointer = p;
        // 弹层悬停优先
        let target = if self.dropdown.anim.is_visible() && self.dropdown.item_at(p).is_some() {
            Some(Target::Dropdown)
        } else if self.selector.anim.is_visible() && self.selector.item_at(p).is_some() {
            Some(Target::Selector)
        } else {
            self.hit_regular(p)
        };

        if target == self.hovered {
            return;
        }
        self.clear_hover();
        self.hovered = target;
        match target {
            Some(Target::Button) => self.button.set_state(ControlState::Hovered),
            Some(Target::Accent) => self.accent.set_state(ControlState::Hovered),
            Some(Target::Icon) => self.icon.set_state(ControlState::Hovered),
            Some(Target::List) => {
                let rh = self.list.row_height(&self.theme);
                let idx = ((p.y - self.list_rect().origin.y + self.list.scroll) / rh) as usize;
                self.list.hovered = Some(idx).filter(|i| *i < self.list.rows.len());
            }
            Some(Target::Dropdown) => {
                self.dropdown.hovered = self.dropdown.item_at(p);
            }
            Some(Target::Selector) => {
                self.selector.hovered = self.selector.item_at(p);
            }
            _ => {}
        }
    }

    /// 最近一次指针位置（逻辑坐标）。
    fn pointer_pos(&self) -> Point {
        self.pointer
    }

    fn clear_hover(&mut self) {
        self.hovered = None;
        self.button.set_state(ControlState::Normal);
        self.accent.set_state(ControlState::Normal);
        self.icon.set_state(ControlState::Normal);
        self.list.hovered = None;
        self.dropdown.hovered = None;
        self.selector.hovered = None;
    }

    /// 按下（常规控件）。
    fn press(&mut self, p: Point) {
        // 弹层优先
        if self.dialog.is_visible() {
            // 按钮路由：命中按钮 → 记录身份并关闭；未命中（遮罩/空白）仅关闭（简化）。
            let screen = Rect::new(0.0, 0.0, 960.0, 600.0);
            self.dialog_result = self.dialog.hit_button(screen, p);
            self.dialog.hide();
            self.pressed = None;
            return;
        }
        if self.dropdown.anim.is_visible() {
            if let Some(i) = self.dropdown.item_at(p) {
                self.dropdown.close();
                // 简单动作：无副作用（选中态可后续扩展）
                let _ = i;
            } else {
                self.dropdown.close();
            }
            return;
        }
        if self.selector.anim.is_visible() {
            if let Some(i) = self.selector.item_at(p) {
                self.selector.selected = Some(i);
                self.selector.close();
            } else {
                self.selector.close();
            }
            return;
        }

        let t = self.hit_regular(p);
        self.pressed = t;
        match t {
            Some(Target::Button) | Some(Target::Accent) | Some(Target::Icon) => {
                let state = ControlState::Pressed;
                if t == Some(Target::Button) {
                    self.button.set_state(state);
                } else if t == Some(Target::Accent) {
                    self.accent.set_state(state);
                } else {
                    self.icon.set_state(state);
                }
            }
            _ => {}
        }
    }

    /// 释放（触发动作）。
    fn release(&mut self, p: Point) {
        let Some(t) = self.pressed else {
            return;
        };
        self.pressed = None;
        // 弹起位置仍在同一目标上才算触发
        let hit_now = match t {
            Target::Button => self.button_rect().contains(p),
            Target::Accent => self.accent_rect().contains(p),
            Target::Icon => self.icon_rect().contains(p),
            Target::Switch => self.switch_rect().contains(p),
            Target::Tabs => self.tabs_rect().contains(p),
            Target::List => self.list_rect().contains(p),
            Target::Dropdown => self.dropdown_trigger().contains(p),
            Target::Selector => self.selector_trigger().contains(p),
        };
        if !hit_now {
            self.update_hover(p);
            return;
        }

        match t {
            Target::Button | Target::Accent | Target::Icon => {
                // 还原悬停态（若仍在其上）
                self.clear_hover();
                self.update_hover(p);
                if t == Target::Accent {
                    self.dialog.show();
                }
            }
            Target::Switch => {
                self.switch.set_checked(!self.switch.checked);
            }
            Target::Tabs => {
                if let Some(i) = self.tabs.tab_at(&self.engine, p.x) {
                    self.tabs.select(i);
                }
            }
            Target::List => {
                let rh = self.list.row_height(&self.theme);
                let idx = ((p.y - self.list_rect().origin.y + self.list.scroll) / rh) as usize;
                if idx < self.list.rows.len() {
                    self.list.select(Some(idx));
                }
            }
            Target::Dropdown => {
                self.dropdown.toggle(self.dropdown_panel());
            }
            Target::Selector => {
                self.selector.toggle(self.selector_panel());
            }
        }
        self.update_hover(p);
    }
}

impl App for GalleryApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn font_path(&self) -> Option<std::path::PathBuf> {
        // 外壳从 KANESUMI_TEST_FONT 或系统字体查找；此处无需指定。
        None
    }

    fn update(&mut self, dt: f64) {
        self.switch.update(dt);
        self.bar.update(dt);
        self.ring.update(dt);
        self.dropdown.update(dt);
        self.selector.update(dt);
        self.dialog.update(dt);
    }

    fn handle_input(&mut self, event: InputEvent) {
        match event {
            InputEvent::PointerMoved { x, y } => self.update_hover(Point::new(x, y)),
            InputEvent::PointerPressed { x, y, button } => {
                if button == PointerButton::Left {
                    self.press(Point::new(x, y));
                }
            }
            InputEvent::PointerReleased { x, y, button } => {
                if button == PointerButton::Left {
                    self.release(Point::new(x, y));
                }
            }
            InputEvent::Scroll { y, .. } => {
                // 滚轮：指针在列表视口上时滚动列表；否则无操作
                let p = self.pointer_pos();
                if self.list_rect().contains(p) {
                    self.list
                        .scroll_by(&self.theme, self.list_rect().size.height, y);
                }
            }
            InputEvent::PointerLeft => {
                self.clear_hover();
                self.pressed = None;
            }
        }
    }

    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene {
        let mut scene = Scene::default();
        let colors = &self.theme.colors;

        // 背景
        scene.fill_rect(
            colors.background,
            Rect::new(0.0, 0.0, size.width, size.height),
        );

        // 标题
        let title_style = self.theme.typography.page_heading;
        scene.text(
            "Kanesumi Gallery".into(),
            Rect::new(PAD, 20.0, size.width - PAD * 2.0, TITLE_H),
            colors.on_background,
            title_style,
            TextAlign::Left,
        );

        // 控件
        self.button
            .render(&self.theme, engine, self.button_rect(), &mut scene);
        self.accent
            .render(&self.theme, engine, self.accent_rect(), &mut scene);
        self.icon
            .render(&self.theme, engine, self.icon_rect(), &mut scene);
        self.switch
            .render(&self.theme, engine, self.switch_rect(), &mut scene);
        self.bar
            .render(&self.theme, engine, self.bar_rect(), &mut scene);
        self.ring.render(&self.theme, self.ring_rect(), &mut scene);
        self.tabs
            .render(&self.theme, engine, self.tabs_rect(), &mut scene);
        self.list
            .render(&self.theme, engine, self.list_rect(), &mut scene);

        // 弹层（Dropdown / Selector 触发器始终画）
        let dt = self.dropdown_trigger();
        scene.fill_rounded_rect(colors.surface, dt, self.theme.tokens.corner_radius);
        let style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
        scene.text(
            "菜单 ▾".into(),
            Rect::new(
                dt.origin.x + 12.0,
                dt.origin.y + (dt.size.height - style.line_height) / 2.0,
                dt.size.width - 24.0,
                style.line_height,
            ),
            colors.on_surface,
            style,
            TextAlign::Left,
        );
        self.dropdown.render(
            &self.theme,
            engine,
            Rect::new(0.0, 0.0, size.width, size.height),
            &mut scene,
        );

        let st = self.selector_trigger();
        scene.fill_rounded_rect(colors.surface, st, self.theme.tokens.corner_radius);
        let sel_text = self
            .selector
            .selected
            .and_then(|i| self.selector.items.get(i))
            .cloned()
            .unwrap_or_else(|| self.selector.placeholder.clone());
        let sel_text = if sel_text.is_empty() {
            "选择 ▾".into()
        } else {
            sel_text
        };
        scene.text(
            sel_text,
            Rect::new(
                st.origin.x + 12.0,
                st.origin.y + (st.size.height - style.line_height) / 2.0,
                st.size.width - 24.0,
                style.line_height,
            ),
            colors.on_surface,
            style,
            TextAlign::Left,
        );
        self.selector.render(
            &self.theme,
            engine,
            st,
            Rect::new(0.0, 0.0, size.width, size.height),
            &mut scene,
        );

        // 对话框（最上层）
        self.dialog.render(
            &self.theme,
            engine,
            Rect::new(0.0, 0.0, size.width, size.height),
            &mut scene,
        );

        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
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

    fn app() -> GalleryApp {
        GalleryApp::with_engine(TextEngine::load(find_font().unwrap()).unwrap())
    }

    /// 点击矩形中心：press + release。
    fn click(app: &mut GalleryApp, rect: Rect) {
        let p = Point::new(
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0,
        );
        app.handle_input(InputEvent::PointerPressed {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
        app.handle_input(InputEvent::PointerReleased {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
    }

    #[test]
    fn switch_toggles_on_click() {
        let mut g = app();
        assert!(!g.switch.checked);
        let r = g.switch_rect();
        click(&mut g, r);
        assert!(g.switch.checked, "点击开关应切换");
        let r = g.switch_rect();
        click(&mut g, r);
        assert!(!g.switch.checked);
    }

    #[test]
    fn tabs_select_on_click() {
        let mut g = app();
        assert_eq!(g.tabs.selected, 0);
        // 第二个 tab 中心：需按 header 宽度计算
        let (tabs_origin, w0, w1) = {
            let g = &g;
            (
                g.tabs_rect().origin,
                g.tabs.header_width(&g.engine, 0),
                g.tabs.header_width(&g.engine, 1),
            )
        };
        let p = Point::new(tabs_origin.x + w0 + w1 / 2.0, tabs_origin.y + 20.0);
        g.handle_input(InputEvent::PointerPressed {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
        assert_eq!(g.tabs.selected, 1, "点击第二 tab 应选中");
    }

    #[test]
    fn list_selects_on_click() {
        let mut g = app();
        let (origin, rh) = (g.list_rect().origin, g.list.row_height(&g.theme));
        // 点击第 3 行（index 2）
        let p = Point::new(origin.x + 20.0, origin.y + 2.5 * rh);
        g.handle_input(InputEvent::PointerPressed {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: p.x,
            y: p.y,
            button: PointerButton::Left,
        });
        assert_eq!(g.list.selected, Some(2));
    }

    #[test]
    fn accent_button_opens_dialog() {
        let mut g = app();
        assert!(!g.dialog.is_visible());
        let r = g.accent_rect();
        click(&mut g, r);
        g.update(1.0);
        assert!(g.dialog.is_visible(), "点击 accent 按钮应打开对话框");
        // 对话框打开时点击任意处关闭（hide 后动画走完转 Closed）
        let r = g.switch_rect();
        click(&mut g, r);
        g.update(1.0);
        assert!(!g.dialog.is_visible());
    }

    #[test]
    fn dropdown_toggles_and_selects() {
        let mut g = app();
        assert!(!g.dropdown.anim.is_visible());
        let tr = g.dropdown_trigger();
        click(&mut g, tr);
        g.update(1.0);
        assert!(g.dropdown.anim.is_visible(), "点击触发器应展开菜单");
        // 点击面板第一项：关闭菜单（close 后动画走完转 Closed）
        let panel = g.dropdown_panel();
        let item = Point::new(panel.origin.x + 20.0, panel.origin.y + 10.0);
        g.handle_input(InputEvent::PointerPressed {
            x: item.x,
            y: item.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: item.x,
            y: item.y,
            button: PointerButton::Left,
        });
        g.update(1.0);
        assert!(!g.dropdown.anim.is_visible(), "点选菜单项应关闭");
    }

    #[test]
    fn selector_selects_item() {
        let mut g = app();
        let tr = g.selector_trigger();
        click(&mut g, tr);
        g.update(1.0);
        assert!(g.selector.anim.is_visible());
        let panel = g.selector_panel();
        let item = Point::new(panel.origin.x + 20.0, panel.origin.y + 40.0); // 第 2 行
        g.handle_input(InputEvent::PointerPressed {
            x: item.x,
            y: item.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: item.x,
            y: item.y,
            button: PointerButton::Left,
        });
        g.update(1.0);
        assert_eq!(g.selector.selected, Some(1), "应选中第 2 项");
        assert!(!g.selector.anim.is_visible());
    }

    #[test]
    fn render_produces_scene_with_text() {
        let mut g = app();
        let engine = g.engine.clone();
        let scene = g.render(&engine, Size::new(960.0, 600.0));
        assert!(!scene.commands.is_empty());
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { .. }))
            .count();
        assert!(texts >= 4, "标题 + 各控件文本");
    }

    #[test]
    fn scroll_over_list_scrolls_it() {
        let mut g = app();
        let before = g.list.scroll;
        // 指针移到列表视口内，向下滚动 2 格（100px）
        let center = g.list_rect().center();
        g.handle_input(InputEvent::PointerMoved {
            x: center.x,
            y: center.y,
        });
        g.handle_input(InputEvent::Scroll { x: 0.0, y: 100.0 });
        assert!(
            g.list.scroll > before,
            "滚轮应滚动列表，before={before} after={}",
            g.list.scroll
        );
    }

    #[test]
    fn list_hover_tracks_row() {
        let mut g = app();
        let (origin, rh) = (g.list_rect().origin, g.list.row_height(&g.theme));
        // 悬停第 2 行
        g.handle_input(InputEvent::PointerMoved {
            x: origin.x + 20.0,
            y: origin.y + 1.5 * rh,
        });
        assert_eq!(g.list.hovered, Some(1), "悬停应命中第 2 行");
        // 移到列表外 → 清除
        g.handle_input(InputEvent::PointerMoved { x: 5.0, y: 5.0 });
        assert_eq!(g.list.hovered, None, "离开列表应清除悬停");
    }

    #[test]
    fn scroll_outside_list_is_ignored() {
        let mut g = app();
        g.handle_input(InputEvent::Scroll { x: 0.0, y: 100.0 });
        assert_eq!(g.list.scroll, 0.0, "指针不在列表上时滚动无效");
    }

    #[test]
    fn dialog_button_press_records_result() {
        let mut g = app();
        // 打开对话框
        let r = g.accent_rect();
        click(&mut g, r);
        g.update(1.0);
        assert!(g.dialog.is_visible());
        assert_eq!(g.dialog_result, None);

        // 点击 Close 按钮（右下角）
        let screen = Rect::new(0.0, 0.0, 960.0, 600.0);
        let box_rect = g.dialog.box_rect(screen);
        let right = box_rect.origin.x + box_rect.size.width - 24.0;
        let button_y = box_rect.origin.y + box_rect.size.height - 24.0 - 32.0 + 16.0;
        let close_pos = Point::new(right - 65.0, button_y);
        g.handle_input(InputEvent::PointerPressed {
            x: close_pos.x,
            y: close_pos.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: close_pos.x,
            y: close_pos.y,
            button: PointerButton::Left,
        });
        assert_eq!(
            g.dialog_result,
            Some(kanesumi_controls::DialogButton::Close),
            "点击 Close 按钮应记录身份"
        );
        g.update(1.0);
        assert!(!g.dialog.is_visible(), "按钮点击后对话框关闭");
    }
}

#[cfg(test)]
mod structure_integration {
    use super::*;
    use crate::pages::GalleryPage;
    use kanesumi_structure::{MetroShell, Navigation};

    #[test]
    fn navigation_drives_shell_pages() {
        let mut nav: Navigation<GalleryPage> = Navigation::new(GalleryPage::DesignTokens);
        assert_eq!(*nav.current(), GalleryPage::DesignTokens);

        nav.navigate_to(GalleryPage::Controls);
        assert!(nav.is_transitioning());
        nav.set_transition_progress(1.0);
        assert_eq!(*nav.current(), GalleryPage::Controls);
        assert!(!nav.is_transitioning());

        assert!(nav.go_back());
        nav.set_transition_progress(1.0);
        assert_eq!(*nav.current(), GalleryPage::DesignTokens);
    }

    #[test]
    fn metro_shell_renders_chrome_with_theme() {
        let Some(p) = super::tests::find_font() else {
            return;
        };
        let engine = TextEngine::load(p).unwrap();
        let shell: MetroShell<GalleryPage> = MetroShell::new(GalleryPage::DesignTokens, "Ether");
        let mut scene = Scene::default();
        let content = shell.render(&engine, Rect::new(0.0, 0.0, 960.0, 600.0), &mut scene);
        assert_eq!(content.origin.x, 0.0);
        assert!(content.size.width > 0.0 && content.size.height > 0.0);
        assert!(scene.commands.iter().any(|c| matches!(
            c,
            kanesumi_canvas::SceneCommand::Text { content, .. } if content == "Ether"
        )));
    }
}
