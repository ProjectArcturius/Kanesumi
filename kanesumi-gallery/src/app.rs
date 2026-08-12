// GalleryApp —— 三层测试阶梯的 daily driver（参 Ether-main PLAN.md §4.4）。
//
// 实现 `App` trait：状态驱动渲染 + 输入路由（参 HANDOVER §2 输入层）。
// 事件路由：顶层弹层优先（Dialog/DropdownMenu/SelectorFlyout）→ 常规控件。
// 控件状态切换：set_state / set_checked / hovered / show / hide / toggle。

use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_controls::{
    CommandBarAction, ControlState, MenuItem, MetroAutoSuggestBox, MetroButton, MetroCheckBox,
    MetroCommandBarFlyout, MetroDialog, MetroDropdownMenu, MetroIconButton, MetroList,
    MetroNumberBox, MetroPasswordBox, MetroProgressBar, MetroProgressRing, MetroRepeater,
    MetroScrollView, MetroSelectorFlyout, MetroSwitch, MetroTab, MetroTabRow, MetroTextBox, MetroTile,
    TextInputKey, TileSize,
};
use kanesumi_core::{Color, MetroTheme, Point, Rect, Size, TextStyle};
use kanesumi_harness::{App, AppConfig, EtherRole, InputEvent, PointerButton};
use kanesumi_structure::TileWall;

use crate::pages::{GalleryPage, page_tree, palette};

/// 布局常量（逻辑像素）。
const PAD: f32 = 16.0;
/// 标题 y 起点。
const TITLE_Y: f32 = 20.0;
/// 标题 rect 高度 —— page_heading (34/42) 行高，与 emit_text 实际排版一致
/// （原本用 36 与 line_height 42 不一致，视觉上标题会溢出 rect 6px）。
const TITLE_H: f32 = 42.0;
/// 页导航栏（TabRow, UWP NavigationView Top 模式的等价物）。
const NAV_Y: f32 = TITLE_Y + TITLE_H + 8.0;
const NAV_H: f32 = 48.0;
/// 内容区起点 = 导航栏底 + 12 gap。
const CTRL_Y0: f32 = NAV_Y + NAV_H + 12.0;
/// footer（声明式区）高度。
const FOOTER_H: f32 = 48.0;

// ── Tiles 页常量（TILES_DESIGN §2/§3） ───────────────────────────────────

/// 磁贴单元边长。
const TILE_CELL: f32 = 64.0;
/// 磁贴单元间隔。
const TILE_GAP: f32 = 8.0;
/// 磁贴墙每页列数。
const TILE_COLS: usize = 8;

/// 磁贴墙页数上限。
const TILE_MAX_PAGE: usize = 1;

/// 演示磁贴（三档尺寸 + Live 内容，TILES_DESIGN §4）。
fn demo_tiles() -> Vec<MetroTile> {
    use kanesumi_controls::TileLive;
    let share = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/share.svg");
    let mut tiles = vec![
        MetroTile::new("音乐", TileSize::Mini, Color::from_hex(0xFF_4C_A0_5E)),
        MetroTile::new("日历", TileSize::Mini, Color::from_hex(0xFF_D2_A1_3B)),
        MetroTile::new("邮件", TileSize::Standard, Color::from_hex(0xFF_C8_42_3B)),
        MetroTile::new("相册", TileSize::Large, Color::from_hex(0xFF_3B_8F_C8)),
        MetroTile::new("笔记", TileSize::Mini, Color::from_hex(0xFF_7A_4C_A8)),
        MetroTile::new("天气", TileSize::Standard, Color::from_hex(0xFF_3B_A8_A8)),
        MetroTile::new("时钟", TileSize::Mini, Color::from_hex(0xFF_6E_7A_8A)),
        MetroTile::new("设置", TileSize::Mini, Color::from_hex(0xFF_3A_4A_6B)),
    ];
    tiles[0].live = TileLive::Badge(3);
    tiles[2].live = TileLive::Preview("季度账单已生成".into());
    tiles[2].icon = Icon::load_svg(share, 40);
    tiles[3].live = TileLive::Lines(vec![
        "海边黄昏".into(),
        "城市夜景".into(),
        "周末野餐".into(),
    ]);
    tiles[3].icon = Icon::load_svg(share, 48);
    tiles[5].live = TileLive::Preview("多云 24°".into());
    tiles
}

/// 磁贴布局槽位：(page, row, col)，与 `demo_tiles()` 索引一一对应。
/// 页 0 展示三档 + 填充；页 1 演示翻页。
fn demo_tile_slots() -> Vec<(usize, usize, usize)> {
    vec![
        (0, 0, 0), // 音乐 Mini
        (0, 1, 0), // 日历 Mini
        (0, 0, 1), // 邮件 Standard（2×2 → 行 0-1 列 1-2）
        (0, 0, 3), // 相册 Large（4×2 → 行 0-1 列 3-6）
        (0, 1, 7), // 笔记 Mini
        (1, 0, 0), // 天气 Standard
        (1, 0, 2), // 时钟 Mini
        (1, 1, 2), // 设置 Mini
    ]
}

/// `GalleryPage` → `page_tree()` 索引。用于 nav.select 同步。
fn page_index(p: GalleryPage) -> usize {
    page_tree().iter().position(|&x| x == p).unwrap_or(0)
}

/// 索引 → `GalleryPage`。用于 nav 点击 → page 切换。
fn page_from_index(i: usize) -> GalleryPage {
    page_tree()[i.min(page_tree().len() - 1)]
}

/// 交互目标 —— 常规控件命中标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// 页导航栏（顶部 TabRow，UWP NavigationView Top）。切换 GalleryPage。
    Nav,
    Button,
    Accent,
    Icon,
    Switch,
    Tabs,
    List,
    Dropdown,
    Selector,
    // ── Input 页 ──
    TextBox,
    PasswordBox,
    CheckBox,
    NumberUp,
    NumberDown,
    AutoSuggest,
    /// CommandBarFlyout 命令按钮（命中时按按钮映射动作）。
    CommandBar,
    /// 虚拟化长列表（MetroRepeater 引擎演示）。
    VirtualList,
}

/// 聚焦的输入控件（键盘路由）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedInput {
    None,
    TextBox,
    Password,
    Number,
    AutoSuggest,
}

/// Gallery 应用状态。
pub struct GalleryApp {
    theme: MetroTheme,
    engine: TextEngine,
    config: AppConfig,

    /// 当前页（UWP NavigationView 选中项的等价）。
    page: GalleryPage,
    /// 页导航栏 —— TabRow 作为 top-mode NavigationView。
    nav: MetroTabRow,

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

    // ── Input 页控件 ──
    /// 文本输入框。
    textbox: MetroTextBox,
    /// 密码框。
    password: MetroPasswordBox,
    /// 复选框（三态）。
    checkbox: MetroCheckBox,
    /// 数字输入框。
    number: MetroNumberBox,
    /// 自动建议框。
    suggest: MetroAutoSuggestBox,
    /// 选中文本浮出命令条（TextBox 内选中时显示）。
    command_bar: MetroCommandBarFlyout,
    /// 命令条是否打开（供命中路由）。
    command_bar_open: bool,
    /// CommandBar 最近动作（演示文本操作）。
    command_result: Option<String>,

    // ── 长列表虚拟化演示（MetroRepeater + MetroScrollView，参 CONTROL_SPEC §41/§42） ──
    /// 虚拟化长列表滚动容器。
    virtual_list: MetroScrollView,
    /// 虚拟化列表选中项。
    virtual_selected: Option<usize>,

    // 输入状态
    hovered: Option<Target>,
    pressed: Option<Target>,
    /// 最近一次指针位置（滚轮路由需要，因为 Scroll 不带坐标）。
    pointer: Point,
    /// 最近一次对话框按钮动作（Primary/Secondary/Close），供应用响应。
    dialog_result: Option<kanesumi_controls::DialogButton>,

    // ── Tiles 页（MetroTile + TileWall 演示） ──
    /// 磁贴集合（TILES_DESIGN 三档尺寸 + Live 内容）。
    tiles: Vec<MetroTile>,
    /// 磁贴布局槽位：(page, row, col)，与 `tiles` 索引一一对应。
    tile_slots: Vec<(usize, usize, usize)>,
    /// 当前磁贴页（0 起）。
    tile_page: usize,
    /// 上一帧当前页磁贴矩形（输入路由用）。
    tile_rects: Vec<Rect>,
    /// 磁贴 hover / pressed 索引。
    tile_hovered: Option<usize>,
    tile_pressed: Option<usize>,
    /// 页导航按钮 pressed（None=无，Some(true)=上一页，Some(false)=下一页）。
    tile_nav_pressed: Option<bool>,

    // 声明式 footer（view! + render_decl 驱动的真实 UI 区域，参 decl.rs）
    /// 声明式按钮点击计数（演示 DSL 驱动状态）。
    decl_count: u32,
    /// 上一帧声明式命中表（输入路由用）。
    decl_hits: Vec<kanesumi_controls::DeclHit>,
    /// 声明式 footer 区域。
    decl_rect: Rect,
    /// retained 渲染器（增量：只重建变化元素命令，PLAN §4.1 不变量 1）。
    decl_retained: kanesumi_controls::RetainedScene,
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
        // 首启页 = Controls（含所有控件示范）。
        let initial_page = GalleryPage::Controls;
        // 页导航（UWP NavigationView Top）—— 4 页与 pages::page_tree 对齐。
        let mut nav = MetroTabRow::new(
            page_tree()
                .iter()
                .map(|p| MetroTab::new(p.title()))
                .collect(),
        );
        // nav 选中态与 page 同步（避免"标题选中 Controls 但视觉选中 Tokens"漂移）。
        nav.select(page_index(initial_page));
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
            page: initial_page,
            nav,
            button: MetroButton::new("Standard"),
            accent: MetroButton::accent("打开对话框"),
            icon: MetroIconButton::with_svg(
                concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/share.svg"),
                16,
                "Share",
            )
            // SVG 缺失时回退为纯标签（避免依赖 MDL2 codepoint 存在，参 V7）
            .unwrap_or_else(|| MetroIconButton::with_label("", "Share")),
            // A1 重做：Switch 用 header + state text（Lumia 布局），不再是左 label
            switch: MetroSwitch::with_header("飞行模式").with_state_text("开", "关"),
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
                // Metro 时代 MenuFlyout 项常无图标（Fluent 时代才普遍带 MDL2 图标）；
                // 我们既然选思源黑体而非 Segoe MDL2，就不假设 MDL2 codepoint 存在。参 V7。
                MenuItem::new("新建"),
                MenuItem::new("打开"),
                MenuItem::new("保存"),
                // 二级级联 + 单选组（RadioMenuFlyoutItem，参 CONTROL_SPEC §39）
                MenuItem::new("缩放").with_submenu(vec![
                    MenuItem::new("放大").radio("zoom"),
                    MenuItem::new("缩小").radio("zoom"),
                    MenuItem::new("重置").radio("zoom"),
                ]),
                MenuItem::new("另存为...").separator(),
                MenuItem::new("退出"),
            ]),
            selector: {
                let mut s = MetroSelectorFlyout::new(
                    ["紧凑", "舒适", "宽松"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                );
                // selector.render 内部会绘触发器；placeholder 是未选时的占位文字。
                s.placeholder = "选择".into();
                s
            },
            dialog: {
                let mut d = MetroDialog::new("保存工作？", "是否保存对当前文件的更改？");
                d.buttons.primary = Some("保存".into());
                d.buttons.secondary = Some("不保存".into());
                d.buttons.close = Some("取消".into());
                d.buttons.default_button = kanesumi_controls::DialogDefaultButton::Primary;
                d
            },
            textbox: MetroTextBox::with_placeholder("输入文本…").with_text("Kanesumi"),
            password: MetroPasswordBox::with_placeholder("••••••"),
            checkbox: MetroCheckBox::new("启用 Wi-Fi").with_checked(true),
            number: MetroNumberBox::with_header("音量").with_min(0.0).with_max(100.0).with_step(5.0),
            suggest: MetroAutoSuggestBox::with_placeholder("搜索水果…").with_suggestions(
                ["苹果", "香蕉", "菠萝", "橙子", "西瓜", "火龙果", "百香果", "芒果", "葡萄", "樱桃"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            ),
            command_bar: MetroCommandBarFlyout::text_commands(),
            command_bar_open: false,
            command_result: None,
            virtual_list: MetroScrollView::default(),
            virtual_selected: None,
            hovered: None,
            pressed: None,
            pointer: Point::ORIGIN,
            dialog_result: None,
            tiles: demo_tiles(),
            tile_slots: demo_tile_slots(),
            tile_page: 0,
            tile_rects: Vec::new(),
            tile_hovered: None,
            tile_pressed: None,
            tile_nav_pressed: None,
            decl_count: 0,
            decl_hits: Vec::new(),
            decl_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            decl_retained: kanesumi_controls::RetainedScene::new(),
        }
        .apply_visual_audit_state()
    }

    /// 视觉审计工具 —— 读 `KANESUMI_DEMO_STATE` 环境变量把 App 打到指定初态，
    /// 供 `docs/VISUAL_ISSUES.md` 逐项截图核对（无输入自动化环境的替代）。
    /// 支持：dialog / dropdown / selector / list-selected / switch-on /
    /// hover-button / focused / tab-1 / disabled。未设或值不匹配 → 默认初态。
    fn apply_visual_audit_state(mut self) -> Self {
        let Ok(state) = std::env::var("KANESUMI_DEMO_STATE") else {
            return self;
        };
        match state.as_str() {
            "dialog" => self.dialog.show(),
            "dropdown" => self.dropdown.toggle(self.dropdown_panel()),
            "selector" => self.selector.toggle(self.selector_panel()),
            "list-selected" => {
                self.list.selected = Some(2);
                self.list.hovered = Some(5);
            }
            "switch-on" => {
                self.switch.set_checked(true);
                for _ in 0..30 {
                    self.switch.update(1.0 / 60.0);
                }
            }
            "hover-button" => {
                self.button.set_state(ControlState::Hovered);
                self.accent.set_state(ControlState::Pressed);
                self.icon.set_state(ControlState::Hovered);
            }
            "focused" => {
                self.button.set_state(ControlState::Focused);
            }
            "tab-1" => {
                self.tabs.select(1);
            }
            "disabled" => {
                self.button.set_state(ControlState::Disabled);
                self.switch.state = ControlState::Disabled;
                self.list.disabled = true;
            }
            _ => {}
        }
        self
    }

    // ── 布局矩形 ────────────────────────────────────────────────────────

    /// 页导航矩形（顶部横向 TabRow）。宽 = 窗口去 padding；高 = NAV_H。
    fn nav_rect(&self) -> Rect {
        Rect::new(PAD, NAV_Y, self.config.width - PAD * 2.0, NAV_H)
    }

    /// 内容区矩形（导航栏下 → footer 上）。所有页在此区域内布局。
    fn content_rect(&self) -> Rect {
        Rect::new(
            PAD,
            CTRL_Y0,
            self.config.width - PAD * 2.0,
            self.config.height - CTRL_Y0 - FOOTER_H,
        )
    }

    /// 按钮宽度由内容驱动（CONTROL_SPEC §1「无 MinWidth，尺寸 = 内容 + Padding」）。
    /// 高度仍固定 38（Gallery 视觉一致），只让宽度跟随 `measure` —— 参 V6。
    fn button_rect(&self) -> Rect {
        let w = self.button.measure(&self.engine, self.theme.typography.body).width;
        Rect::new(PAD, CTRL_Y0, w, 38.0)
    }
    fn accent_rect(&self) -> Rect {
        let w = self.accent.measure(&self.engine, self.theme.typography.body).width;
        let x = self.button_rect().right() + 8.0;
        Rect::new(x, CTRL_Y0, w, 38.0)
    }
    fn icon_rect(&self) -> Rect {
        let x = self.accent_rect().right() + 16.0;
        Rect::new(x, CTRL_Y0 - 4.0, 68.0, 56.0)
    }
    /// Switch 需装下 Header（body.line_height 22）+ 8 gap + Track 行高 22 = 52，
    /// 上下各 4px 边距 → 60 —— 参 A1 重做（switch.rs 新布局）。
    fn switch_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 44.0, 200.0, 60.0)
    }
    fn tabs_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 128.0, 420.0, 48.0)
    }
    fn list_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 188.0, 260.0, 280.0)
    }
    fn dropdown_trigger(&self) -> Rect {
        Rect::new(PAD + 280.0, CTRL_Y0 + 188.0, 130.0, 32.0)
    }
    fn selector_trigger(&self) -> Rect {
        Rect::new(PAD + 280.0, CTRL_Y0 + 234.0, 180.0, 32.0)
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

    // ── Input 页布局 ──────────────────────────────────────────────

    /// TextBox（含标题）。
    fn textbox_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0, 280.0, 60.0)
    }
    fn password_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 68.0, 280.0, 60.0)
    }
    fn checkbox_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 136.0, 220.0, 40.0)
    }
    fn number_rect(&self) -> Rect {
        Rect::new(PAD + 300.0, CTRL_Y0, 220.0, 60.0)
    }
    fn suggest_rect(&self) -> Rect {
        Rect::new(PAD + 300.0, CTRL_Y0 + 68.0, 280.0, 60.0)
    }
    /// 命令条触发点（TextBox 选中区域下方 —— 演示浮出命令条）。
    fn command_bar_anchor(&self) -> Rect {
        let tb = self.textbox_rect();
        Rect::new(tb.origin.x + 40.0, tb.bottom() - 8.0, 60.0, 16.0)
    }
    /// 命令条面板（由 place 定位）。
    fn command_bar_rect(&self) -> Rect {
        self.command_bar.place(self.command_bar_anchor(), self.screen())
    }
    /// 命令结果提示（CommandBar 动作回显）。
    fn command_result_rect(&self) -> Rect {
        Rect::new(PAD + 300.0, CTRL_Y0 + 136.0, 360.0, 40.0)
    }

    /// 虚拟化长列表视口（Input 页下方）。
    fn virtual_list_rect(&self) -> Rect {
        Rect::new(PAD, CTRL_Y0 + 230.0, 320.0, 190.0)
    }

    /// 虚拟化长列表布局器（1000 项 × 40px）。
    fn virtual_repeater(&self) -> MetroRepeater {
        MetroRepeater::stack_vertical(1000, 40.0)
    }

    /// TextBox 主体矩形（含 header 扣除）。Gallery 层无 header，直接返回内容区。
    fn textbox_body_rect(&self) -> Rect {
        let r = self.textbox_rect();
        // MetroTextBox::body_rect 是私有；Gallery 用无 header 构造，主体 = 整个矩形
        r
    }

    /// 执行 CommandBar 动作（演示文本操作）。
    fn apply_command(&mut self, action: CommandBarAction) {
        use kanesumi_controls::TextField;
        let cur = self.textbox.field.text();
        self.command_result = match action {
            CommandBarAction::Copy => Some("已复制".into()),
            CommandBarAction::Cut => {
                if let Some((lo, hi)) = self.textbox.field.selection() {
                    let mut f = TextField::with_text(&cur);
                    f.set_cursor(lo);
                    for _ in lo..hi {
                        f.delete();
                    }
                    self.textbox.field = f;
                }
                Some("已剪切".into())
            }
            CommandBarAction::Paste => Some("已粘贴".into()),
            CommandBarAction::SelectAll => {
                self.textbox.field.select_all();
                Some("已全选".into())
            }
            CommandBarAction::Custom(_) => None,
        };
    }

    // ── Tiles 页布局（TileWall 演示） ───────────────────────────────────

    fn tile_wall(&self) -> TileWall {
        TileWall::new(TILE_CELL, TILE_GAP).with_columns_per_page(TILE_COLS)
    }

    /// 当前页磁贴矩形列表（顺序 = 页内槽位顺序）。
    fn tile_rects_for_page(&self, page: usize, origin: Rect) -> Vec<Rect> {
        let wall = self.tile_wall();
        self.tile_slots
            .iter()
            .enumerate()
            .filter(|(_, (p, _, _))| *p == page)
            .map(|(i, (_, row, col))| {
                wall.tile_rect(origin, 0, *row, *col, self.tiles[i].size.cells())
            })
            .collect()
    }

    /// 命中当前页磁贴：返回 tiles 索引。
    fn tile_at(&self, p: Point) -> Option<usize> {
        self.tile_rects
            .iter()
            .position(|r| r.contains(p))
    }

    /// 上一页按钮矩形。
    fn tile_nav_prev_rect(&self, content: Rect) -> Rect {
        Rect::new(content.origin.x, content.origin.y + self.tile_wall().page_height() + 16.0, 96.0, 32.0)
    }

    /// 下一页按钮矩形。
    fn tile_nav_next_rect(&self, content: Rect) -> Rect {
        Rect::new(content.origin.x + 104.0, content.origin.y + self.tile_wall().page_height() + 16.0, 96.0, 32.0)
    }

    /// 每帧同步磁贴状态（hover/pressed → Normal）。
    fn sync_tile_states(&mut self) {
        for (i, t) in self.tiles.iter_mut().enumerate() {
            t.state = if self.tile_pressed == Some(i) {
                ControlState::Pressed
            } else if self.tile_hovered == Some(i) {
                ControlState::Hovered
            } else {
                ControlState::Normal
            };
        }
    }

    /// 声明式 footer 元素树 —— 状态文本 + Spacer + 计数按钮
    /// （演示 view! 驱动真实 UI）。Spacer 把按钮推到右端，文字与按钮各按内在宽度渲染。
    /// 参 V8：原本 Row 强制等分导致按钮撑到半屏。
    fn decl_footer(&self) -> kanesumi_controls::Decl {
        use kanesumi_controls::{Decl, DeclAction};
        Decl::row(vec![
            Decl::text(format!("声明式区域 · 计数 {}", self.decl_count)),
            Decl::spacer(1.0),
            Decl::button("点我 +1", DeclAction::Custom(9001)),
        ])
    }

    /// Tiles 页：MetroTile + TileWall 磁贴墙演示（TILES_DESIGN §2/§3/§4/§6）。
    /// 页 0 展示三档尺寸 + Live 内容；页 1 演示整页翻页；下方翻页按钮。
    fn render_page_tiles(&mut self, engine: &TextEngine, content: Rect, scene: &mut Scene) {
        let origin = Rect::new(content.origin.x, content.origin.y, content.size.width, content.size.height);

        // 当前页磁贴矩形 + 状态同步
        self.tile_rects = self.tile_rects_for_page(self.tile_page, origin);
        self.sync_tile_states();

        let page_tile_indices: Vec<usize> = self
            .tile_slots
            .iter()
            .enumerate()
            .filter(|(_, (p, _, _))| *p == self.tile_page)
            .map(|(i, _)| i)
            .collect();
        for (i, rect) in page_tile_indices.iter().zip(self.tile_rects.iter()) {
            self.tiles[*i].render(&self.theme, engine, *rect, scene);
        }

        // 翻页按钮
        let prev = self.tile_nav_prev_rect(content);
        let next = self.tile_nav_next_rect(content);
        let style = self.theme.typography.body;
        for (rect, label, is_prev, enabled) in [
            (prev, "‹ 上一页", true, self.tile_page > 0),
            (next, "下一页 ›", false, self.tile_page < TILE_MAX_PAGE),
        ] {
            scene.fill_rounded_rect(
                self.theme.colors.surface,
                rect,
                self.theme.tokens.corner_radius,
            );
            if enabled {
                if self.tile_nav_pressed == Some(is_prev) {
                    scene.fill_rect(self.theme.indication.press_tint, rect);
                }
                scene.text(
                    label.into(),
                    Rect::new(
                        rect.origin.x + 12.0,
                        rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                        rect.size.width - 24.0,
                        style.line_height,
                    ),
                    self.theme.colors.on_surface,
                    style,
                    TextAlign::Left,
                );
            }
        }
    }

    /// 命中常规控件（弹层优先，由调用方处理）。
    /// 页导航栏永远可点；页内控件仅当当前页匹配时可点。
    fn hit_regular(&self, p: Point) -> Option<Target> {
        if self.nav_rect().contains(p) {
            return Some(Target::Nav);
        }
        // 页内控件仅当在 Controls 页时活跃（其它页只做展示）。
        if self.page == GalleryPage::Controls {
            if self.button_rect().contains(p) {
                return Some(Target::Button);
            } else if self.accent_rect().contains(p) {
                return Some(Target::Accent);
            } else if self.icon_rect().contains(p) {
                return Some(Target::Icon);
            } else if self.switch_rect().contains(p) {
                return Some(Target::Switch);
            } else if self.tabs_rect().contains(p) {
                return Some(Target::Tabs);
            } else if self.list_rect().contains(p) {
                return Some(Target::List);
            } else if self.dropdown_trigger().contains(p) {
                return Some(Target::Dropdown);
            } else if self.selector_trigger().contains(p) {
                return Some(Target::Selector);
            } else {
                return None;
            }
        }
        // Input 页
        if self.page == GalleryPage::Input {
            if self.textbox_rect().contains(p) {
                return Some(Target::TextBox);
            } else if self.password_rect().contains(p) {
                return Some(Target::PasswordBox);
            } else if self.checkbox_rect().contains(p) {
                return Some(Target::CheckBox);
            } else if self.number_rect().contains(p) {
                let theme = &self.theme;
                let r = self.number_rect();
                if self.number.up_button_rect(theme, r).contains(p) {
                    return Some(Target::NumberUp);
                } else if self.number.down_button_rect(theme, r).contains(p) {
                    return Some(Target::NumberDown);
                }
                return Some(Target::TextBox); // 数字文本区 → 聚焦编辑
            } else if self.suggest_rect().contains(p) {
                return Some(Target::AutoSuggest);
            } else if self.virtual_list_rect().contains(p) {
                return Some(Target::VirtualList);
            }
        }
        // 命令条（Input 页，任何位置之上）
        if self.command_bar_open && self.command_bar_rect().contains(p) {
            return Some(Target::CommandBar);
        }
        None
    }

    /// 更新悬停态（Motion / Enter）。
    fn update_hover(&mut self, p: Point) {
        self.pointer = p;

        // Tiles 页：磁贴墙优先（不含常规控件）。
        if self.page == GalleryPage::Tiles {
            let t = self.tile_at(p);
            if t != self.tile_hovered {
                self.tile_hovered = t;
                self.sync_tile_states();
            }
            return;
        }

        // A1：Switch 拖动 —— pressed 在 switch 上时，motion 直接喂 drag_to（不走 hover）
        if self.pressed == Some(Target::Switch) {
            self.switch.drag_to(p);
            return;
        }

        // 弹层悬停优先
        let target = if self.dropdown.anim.is_visible() && self.dropdown.item_at(p).is_some() {
            Some(Target::Dropdown)
        } else if self.selector.anim.is_visible() && self.selector.item_at(p).is_some() {
            Some(Target::Selector)
        } else {
            self.hit_regular(p)
        };

        // Dropdown 级联：即使 target 不变，移动指针也要刷新子菜单（hover-swap）。
        if self.dropdown.anim.is_visible() {
            let in_panel = self.dropdown.item_at(p).is_some();
            let in_sub = self
                .dropdown
                .submenu_state()
                .map(|s| s.panel.contains(p))
                .unwrap_or(false);
            if in_panel || in_sub {
                self.dropdown.hover(&self.engine, p);
                self.hovered = Some(Target::Dropdown);
                return;
            }
        }

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
                // 级联：hover 自动展开子菜单（子菜单项悬停由 path_at 命中）
                self.dropdown.hover(&self.engine, p);
            }
            Some(Target::Selector) => {
                self.selector.hovered = self.selector.item_at(p);
            }
            Some(Target::CheckBox) => {}
            Some(Target::CommandBar) => self.command_bar.hover(p),
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
        self.dropdown.close_submenu();
        self.selector.hovered = None;
        self.command_bar.hovered = None;
        self.command_bar.hover(Point::new(-1000.0, -1000.0));
    }

    /// 按下（常规控件）。
    fn press(&mut self, p: Point) {
        // V18：页导航栏 —— 恒为最高优先级，任何页面（含 Tiles）都能切换。
        // 旧 bug：Tiles 分支在 nav 检查之前 early-return，导致点 nav tab 无响应。
        if self.nav_rect().contains(p) {
            self.pressed = Some(Target::Nav);
            return;
        }

        // Tiles 页：磁贴 / 翻页按钮。
        if self.page == GalleryPage::Tiles {
            if let Some(i) = self.tile_at(p) {
                self.tile_pressed = Some(i);
                self.sync_tile_states();
            } else {
                let content = self.content_rect();
                self.tile_nav_pressed = if self.tile_nav_prev_rect(content).contains(p)
                    && self.tile_page > 0
                {
                    Some(true)
                } else if self.tile_nav_next_rect(content).contains(p)
                    && self.tile_page < TILE_MAX_PAGE
                {
                    Some(false)
                } else {
                    None
                };
            }
            return;
        }

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
            if let Some(path) = self.dropdown.path_at(p) {
                // 选中（RadioMenuFlyoutItem 单选组互斥）；普通项无副作用
                if path.parent.is_some() {
                    self.dropdown.select_submenu(path);
                } else if self.dropdown.items[path.index].radio_group.is_some() {
                    self.dropdown.select(path.index);
                }
                self.dropdown.close();
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

        // 命令条（Input 页浮出）：命中命令按钮 → 执行；点外关闭。
        if self.command_bar_open {
            if let Some(action) = self.command_bar.hit_command(p) {
                self.apply_command(action);
                self.command_bar.close();
                self.command_bar_open = false;
            } else if self.command_bar_rect().contains(p) {
                // 面板内但非按钮（边框区）→ 关闭
                self.command_bar.close();
                self.command_bar_open = false;
            } else {
                self.command_bar.close();
                self.command_bar_open = false;
            }
            return;
        }

        // 声明式 footer 命中路由（render_decl 产出的命中表）
        if let Some(hit) = self.decl_hits.iter().find(|h| h.rect.contains(p)) {
            if hit.action == kanesumi_controls::DeclAction::Custom(9001) {
                self.decl_count += 1;
            }
            self.pressed = None;
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
            Some(Target::Switch) => {
                // A1：Switch 支持拖动 —— press 记录起点，motion → drag_to，release → 提交
                let rect = self.switch_rect();
                self.switch.press(rect, &self.theme, p);
            }
            Some(Target::TextBox) => {
                // 聚焦文本框（点击定位光标）
                self.textbox.focus();
                self.password.blur();
                self.number.blur();
                self.suggest.blur();
                let body = self.textbox_body_rect();
                self.textbox
                    .place_caret_at(&self.theme, &self.engine, body, p);
                // 聚焦时全选（UWP 行为）→ 浮出命令条
                self.textbox.field.select_all();
                self.command_bar
                    .open(self.command_bar_anchor(), self.screen());
                self.command_bar_open = true;
            }
            Some(Target::PasswordBox) => {
                self.password.focus();
                self.textbox.blur();
                self.number.blur();
                self.suggest.blur();
            }
            Some(Target::NumberUp) => {
                self.number.focus();
                self.textbox.blur();
                self.password.blur();
                self.suggest.blur();
                self.number.step_up();
            }
            Some(Target::NumberDown) => {
                self.number.focus();
                self.textbox.blur();
                self.password.blur();
                self.suggest.blur();
                self.number.step_down();
            }
            Some(Target::AutoSuggest) => {
                self.suggest.focus();
                self.textbox.blur();
                self.password.blur();
                self.number.blur();
            }
            Some(Target::CheckBox) => {
                self.checkbox.toggle();
            }
            Some(Target::VirtualList) => {
                // 点选虚拟化条目
                let vrect = self.virtual_list_rect();
                let repeater = self.virtual_repeater();
                let rel = Point::new(p.x - vrect.origin.x, p.y - vrect.origin.y);
                self.virtual_selected = repeater.item_at(vrect.size, self.virtual_list.offset, rel);
            }
            Some(Target::CommandBar) => {
                // release 处理
            }
            _ => {}
        }
    }

    /// 释放（触发动作）。
    fn release(&mut self, p: Point) {
        // V18：Nav 恒优先 —— 若 press 记的是 Nav，先结算再判 Tiles 分支
        // （否则 Tiles 页永远切不出去，参 press 同款修复）。
        if self.pressed == Some(Target::Nav) {
            self.pressed = None;
            if self.nav_rect().contains(p) {
                if let Some(i) = self.nav.tab_at(&self.engine, self.nav_rect(), p) {
                    self.nav.select(i);
                    self.page = page_from_index(i);
                    self.dropdown.close();
                    self.selector.close();
                }
            }
            return;
        }

        // Tiles 页：磁贴按下反馈 + 翻页。
        if self.page == GalleryPage::Tiles {
            if let Some(i) = self.tile_pressed.take() {
                // 磁贴动作：暂无（Launcher 负责启动）；保持视觉反馈。
                let _ = self.tile_at(p) == Some(i);
            }
            if let Some(is_prev) = self.tile_nav_pressed.take() {
                let content = self.content_rect();
                let hit = if is_prev {
                    self.tile_nav_prev_rect(content).contains(p)
                } else {
                    self.tile_nav_next_rect(content).contains(p)
                };
                if hit {
                    if is_prev {
                        self.tile_page = self.tile_page.saturating_sub(1);
                    } else {
                        self.tile_page = (self.tile_page + 1).min(TILE_MAX_PAGE);
                    }
                }
            }
            self.sync_tile_states();
            self.update_hover(p);
            return;
        }

        let Some(t) = self.pressed else {
            return;
        };
        self.pressed = None;
        // 弹起位置仍在同一目标上才算触发
        let hit_now = match t {
            Target::Nav => self.nav_rect().contains(p),
            Target::Button => self.button_rect().contains(p),
            Target::Accent => self.accent_rect().contains(p),
            Target::Icon => self.icon_rect().contains(p),
            Target::Switch => self.switch_rect().contains(p),
            Target::Tabs => self.tabs_rect().contains(p),
            Target::List => self.list_rect().contains(p),
            Target::Dropdown => self.dropdown_trigger().contains(p),
            Target::Selector => self.selector_trigger().contains(p),
            Target::TextBox => self.textbox_rect().contains(p),
            Target::PasswordBox => self.password_rect().contains(p),
            Target::CheckBox => self.checkbox_rect().contains(p),
            Target::NumberUp => self.number.up_button_rect(&self.theme, self.number_rect()).contains(p),
            Target::NumberDown => self
                .number
                .down_button_rect(&self.theme, self.number_rect())
                .contains(p),
            Target::AutoSuggest => self.suggest_rect().contains(p),
            Target::CommandBar => self.command_bar_rect().contains(p),
            Target::VirtualList => self.virtual_list_rect().contains(p),
        };
        if !hit_now {
            // A1：Switch 释放到轨道外 = 取消拖动（不 commit）
            if t == Target::Switch {
                self.switch.cancel();
            }
            self.update_hover(p);
            return;
        }

        match t {
            Target::Nav => {
                // 页切换：nav.tab_at 命中 → 更新 selected + page；关闭任何弹层。
                if let Some(i) = self.nav.tab_at(&self.engine, self.nav_rect(), p) {
                    self.nav.select(i);
                    self.page = page_from_index(i);
                    // 页切换时收拢所有弹层（避免残留 dropdown 显示在错位置）。
                    self.dropdown.close();
                    self.selector.close();
                }
            }
            Target::Button | Target::Accent | Target::Icon => {
                // 还原悬停态（若仍在其上）
                self.clear_hover();
                self.update_hover(p);
                if t == Target::Accent {
                    self.dialog.show();
                }
            }
            Target::Switch => {
                // A1：release 提交拖动结果（未拖动 = 点动 toggle；拖动 = 按 knob 过半判）
                let _ = self.switch.release();
            }
            Target::Tabs => {
                if let Some(i) = self.tabs.tab_at(&self.engine, self.tabs_rect(), p) {
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
            Target::TextBox | Target::PasswordBox | Target::AutoSuggest => {
                // 已在 press 聚焦；release 无需动作
            }
            Target::CheckBox | Target::NumberUp | Target::NumberDown | Target::CommandBar => {}
            Target::VirtualList => {
                // press 已选中；release 无需额外动作
            }
        }
        self.update_hover(p);
    }

    // ── 分页渲染 ────────────────────────────────────────────────────────
    //
    // 参照 WinUI-Gallery / UWP NavigationView：Frame 内一次只渲染当前页内容，
    // 不同页承载不同关注点，避免"一屏塞满所有控件"的视觉混乱。
    //
    // Popups（Dropdown / Selector）触发器与弹层仅在 Controls 页画；Dialog 是
    // 应用级模态，独立于页栈，`render()` 顶层统一处理。

    /// DesignTokens 页：主题调色板 —— UWP DesignTokens 参考的最小可视化。
    /// 每个 token 一行：色块 + 名字 + hex。竖排，Ui 原语驱动。
    fn render_page_tokens(&self, _engine: &TextEngine, content: Rect, scene: &mut Scene) {
        use kanesumi_structure::{LayoutDirection, Ui};

        let entries = palette(&self.theme);
        let mut ui = Ui::new(content, LayoutDirection::Vertical).with_spacing(8.0);
        let row_h = 28.0;
        let swatch_w = 40.0;
        let label_style = self.theme.typography.body;

        for entry in &entries {
            let row = ui.allocate(Size::new(content.size.width, row_h));
            // 色块（左）+ token 名（中）
            let swatch = Rect::new(row.origin.x, row.origin.y + 4.0, swatch_w, row_h - 8.0);
            scene.fill_rect(entry.color, swatch);
            let name_rect = Rect::new(
                row.origin.x + swatch_w + 12.0,
                row.origin.y + (row_h - label_style.line_height) / 2.0,
                content.size.width - swatch_w - 12.0,
                label_style.line_height,
            );
            scene.text(
                entry.name.into(),
                name_rect,
                self.theme.colors.on_surface,
                label_style,
                TextAlign::Left,
            );
        }
    }

    /// Animation 页：ProgressBar（不确定）+ ProgressRing（不确定）—— 两个
    /// UWP 时代运动的代表控件，各自演示 sokuou 时长与 UWP 缓动。
    fn render_page_animation(
        &self,
        engine: &TextEngine,
        content: Rect,
        scene: &mut Scene,
    ) {
        use kanesumi_structure::{LayoutDirection, Ui};
        let mut ui = Ui::new(content, LayoutDirection::Vertical).with_spacing(28.0);

        // Bar 段：标签 + Bar
        let label_style = self.theme.typography.body;
        let bar_label = ui.allocate(Size::new(content.size.width, label_style.line_height));
        scene.text(
            "ProgressBar · 不确定态".into(),
            bar_label,
            self.theme.colors.on_surface,
            label_style,
            TextAlign::Left,
        );
        let bar_rect = ui.allocate(Size::new(300.0, 4.0));
        self.bar.render(&self.theme, engine, bar_rect, scene);

        // Ring 段：标签 + Ring
        let ring_label = ui.allocate(Size::new(content.size.width, label_style.line_height));
        scene.text(
            "ProgressRing · 不确定态".into(),
            ring_label,
            self.theme.colors.on_surface,
            label_style,
            TextAlign::Left,
        );
        let ring_rect = ui.allocate(Size::new(32.0, 32.0));
        self.ring.render(&self.theme, ring_rect, scene);
    }

    /// Controls 页：所有交互控件的示范（按钮 / 开关 / Tabs / List / 菜单）。
    /// 使用现有的 rect helpers（button_rect / accent_rect / …）以保持既有测试有效。
    fn render_page_controls(&mut self, engine: &TextEngine, size: Size, scene: &mut Scene) {
        let colors = &self.theme.colors;

        self.button
            .render(&self.theme, engine, self.button_rect(), scene);
        self.accent
            .render(&self.theme, engine, self.accent_rect(), scene);
        self.icon
            .render(&self.theme, engine, self.icon_rect(), scene);
        self.switch
            .render(&self.theme, engine, self.switch_rect(), scene);
        self.tabs
            .render(&self.theme, engine, self.tabs_rect(), scene);
        self.list
            .render(&self.theme, engine, self.list_rect(), scene);

        // Dropdown 触发器（自绘）+ 弹层
        let dt = self.dropdown_trigger();
        scene.fill_rounded_rect(colors.surface, dt, self.theme.tokens.corner_radius);
        let style = TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal);
        scene.text(
            "菜单".into(),
            Rect::new(
                dt.origin.x + 12.0,
                dt.origin.y + (dt.size.height - style.line_height) / 2.0,
                dt.size.width - 24.0 - 22.0,
                style.line_height,
            ),
            colors.on_surface,
            style,
            TextAlign::Left,
        );
        kanesumi_canvas::glyph::chevron_down(
            scene,
            Rect::new(
                dt.origin.x + dt.size.width - 22.0,
                dt.origin.y + (dt.size.height - 12.0) / 2.0,
                12.0,
                12.0,
            ),
            colors.on_surface,
        );
        self.dropdown.render(
            &self.theme,
            engine,
            Rect::new(0.0, 0.0, size.width, size.height),
            scene,
        );
        // 二级级联子菜单（画在顶层之上）
        self.dropdown.render_submenu(&self.theme, engine, scene);

        // Selector（触发器 + 弹层同 render 拥有）
        self.selector.render(
            &self.theme,
            engine,
            self.selector_trigger(),
            Rect::new(0.0, 0.0, size.width, size.height),
            scene,
        );
    }

    /// Input 页：文本输入控件全示范（TextBox / PasswordBox / CheckBox / NumberBox /
    /// AutoSuggestBox + CommandBarFlyout 浮出命令条）。参 CONTROL_SPEC §34–§38、§40。
    fn render_page_input(&mut self, engine: &TextEngine, scene: &mut Scene) {
        let style = self.theme.typography.body;
        let label_style = self.theme.typography.caption;
        let colors = &self.theme.colors;

        // 组标签
        scene.text(
            "文本输入".into(),
            Rect::new(PAD, CTRL_Y0 - 18.0, 200.0, label_style.line_height),
            colors.on_surface_variant,
            label_style,
            TextAlign::Left,
        );
        scene.text(
            "数值 / 建议".into(),
            Rect::new(PAD + 300.0, CTRL_Y0 - 18.0, 200.0, label_style.line_height),
            colors.on_surface_variant,
            label_style,
            TextAlign::Left,
        );

        // TextBox
        self.textbox
            .render(&self.theme, engine, self.textbox_rect(), scene);

        // PasswordBox
        self.password
            .render(&self.theme, engine, self.password_rect(), scene);

        // CheckBox（右侧补充标签）
        self.checkbox
            .render(&self.theme, engine, self.checkbox_rect(), scene);
        let cbs = format!("状态: {:?}", self.checkbox.state);
        scene.text(
            cbs,
            Rect::new(
                PAD + 200.0,
                self.checkbox_rect().origin.y + (40.0 - style.line_height) / 2.0,
                160.0,
                style.line_height,
            ),
            colors.on_surface_variant,
            style,
            TextAlign::Left,
        );

        // NumberBox
        self.number
            .render(&self.theme, engine, self.number_rect(), scene);

        // AutoSuggestBox
        self.suggest
            .render(&self.theme, engine, self.suggest_rect(), scene);

        // CommandBar 触发：选中文本框内容时，在选区下方浮出命令条
        let has_selection = self.textbox.focused && self.textbox.field.selection().is_some();
        if has_selection && !self.command_bar_open {
            // 提示：双击文本框全选后可看到命令条
        }
        if self.command_bar_open {
            self.command_bar.render(&self.theme, engine, scene);
        }

        // CommandBar 结果回显
        if let Some(msg) = &self.command_result {
            scene.text(
                msg.clone(),
                self.command_result_rect(),
                colors.on_surface,
                style,
                TextAlign::Left,
            );
        }

        // 提示文字
        let hint = if self.textbox.focused {
            "Tab 切换焦点 · 双击全选 → 命令条"
        } else {
            "点击文本框聚焦 · 双击全选弹出命令条"
        };
        scene.text(
            hint.into(),
            Rect::new(
                PAD,
                CTRL_Y0 + 200.0,
                500.0,
                style.line_height,
            ),
            colors.on_surface_variant,
            style,
            TextAlign::Left,
        );

        // 虚拟化长列表（MetroRepeater + MetroScrollView 引擎演示）
        let vrect = self.virtual_list_rect();
        self.virtual_list.viewport_size = vrect.size;
        self.virtual_list.content_size = Size::new(
            vrect.size.width,
            self.virtual_repeater().content_length(),
        );
        self.virtual_list.update(0.0);

        // 边框 + 视口裁剪
        scene.stroke_rect(colors.divider, vrect, 1.0);
        scene.clip(Some(vrect));
        let repeater = self.virtual_repeater();
        if let Some((first, last)) = repeater.visible_range(vrect.size.height, self.virtual_list.offset)
        {
            for i in first..=last {
                let mut row = repeater.item_rect(i, vrect.size, self.virtual_list.offset);
                row.origin.x += vrect.origin.x;
                row.origin.y += vrect.origin.y;
                let selected = self.virtual_selected == Some(i);
                if selected {
                    scene.fill_rect(colors.primary.with_alpha(0.60), row);
                }
                scene.text(
                    format!("项目 {i} · 虚拟化条目"),
                    Rect::new(
                        row.origin.x + 12.0,
                        row.origin.y + (40.0 - style.line_height) / 2.0,
                        row.size.width - 24.0,
                        style.line_height,
                    ),
                    colors.on_surface,
                    style,
                    TextAlign::Left,
                );
            }
        }
        scene.clip(None);
        // 滚动条（可滚时显示）
        if self.virtual_list.scrollbar_visible() {
            let track = self.virtual_list.scrollbar_track_rect();
            let track = Rect::new(
                vrect.origin.x + track.origin.x,
                vrect.origin.y + track.origin.y,
                track.size.width,
                track.size.height,
            );
            scene.fill_rect(colors.surface_variant, track);
            let thumb = self.virtual_list.scrollbar_thumb_rect();
            let thumb = Rect::new(
                vrect.origin.x + thumb.origin.x,
                vrect.origin.y + thumb.origin.y,
                thumb.size.width,
                thumb.size.height,
            );
            scene.fill_rect(colors.on_surface_variant.with_alpha(0.6), thumb);
        }
        // 标签
        scene.text(
            "虚拟化长列表（1000 项 · 只渲染可见行）".into(),
            Rect::new(vrect.origin.x, vrect.bottom() + 4.0, 320.0, label_style.line_height),
            colors.on_surface_variant,
            label_style,
            TextAlign::Left,
        );
    }

    /// Structure 页：MetroShell 微缩演示 —— 顶部 AppBar + 内容区。
    /// 用现成 `MetroShell::render` 在内容 rect 内画一个"页中页"。
    fn render_page_structure(
        &self,
        engine: &TextEngine,
        content: Rect,
        scene: &mut Scene,
    ) {
        use kanesumi_structure::MetroShell;
        let shell: MetroShell<GalleryPage> =
            MetroShell::new(GalleryPage::Structure, "MetroShell 示例");
        // 微缩：把 shell 画进 content 区域（inset 一圈以呈现"卡片"感）。
        let inner = Rect::new(
            content.origin.x + 8.0,
            content.origin.y + 8.0,
            content.size.width - 16.0,
            (content.size.height - 16.0).max(120.0),
        );
        // 卡片边框
        scene.stroke_rect(self.theme.colors.divider, inner, 1.0);
        let content_area = shell.render(engine, inner, scene);
        // 内容区加一段说明文字
        let style = self.theme.typography.body;
        let text_rect = Rect::new(
            content_area.origin.x + 16.0,
            content_area.origin.y + 16.0,
            content_area.size.width - 32.0,
            style.line_height * 3.0,
        );
        scene.text(
            "MetroShell = AppBar + Scaffold（内容区）。\n\
             AppBar 承载页标题；内容区由页负责填充。\n\
             参 kanesumi-structure/lib.rs。"
                .into(),
            text_rect,
            self.theme.colors.on_surface,
            style,
            TextAlign::Left,
        );
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
        // V17：TabRow 选中管道滑行 + 文字色 crossfade 需每帧推进
        self.nav.update(dt);
        self.tabs.update(dt);
        // 输入控件
        self.textbox.update(dt);
        self.password.update(dt);
        self.command_bar.update(dt);
        // 虚拟化长列表平滑滚动
        self.virtual_list.update(dt);
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
                if self.page == GalleryPage::Input && self.virtual_list_rect().contains(p) {
                    self.virtual_list.viewport_size = self.virtual_list_rect().size;
                    self.virtual_list.content_size = Size::new(
                        self.virtual_list_rect().size.width,
                        self.virtual_repeater().content_length(),
                    );
                    self.virtual_list.scroll_wheel(y);
                } else if self.list_rect().contains(p) {
                    self.list
                        .scroll_by(&self.theme, self.list_rect().size.height, y);
                }
            }
            InputEvent::KeyPressed { key } => {
                // 键盘路由：聚焦的输入控件优先（TextBox / PasswordBox / NumberBox / AutoSuggestBox）
                let focused: FocusedInput = if self.textbox.focused {
                    FocusedInput::TextBox
                } else if self.password.focused() {
                    FocusedInput::Password
                } else if self.number.focused {
                    FocusedInput::Number
                } else if self.suggest.focused {
                    FocusedInput::AutoSuggest
                } else {
                    FocusedInput::None
                };

                use kanesumi_harness::Key as HarnessKey;
                // 统一转 TextInputKey；Up/Down 保留给 suggest 导航，其余交给文本编辑。
                let mapped = match key {
                    HarnessKey::Char(c) => Some(TextInputKey::Char(c)),
                    HarnessKey::Enter => Some(TextInputKey::Enter),
                    HarnessKey::Backspace => Some(TextInputKey::Backspace),
                    HarnessKey::Escape => Some(TextInputKey::Escape),
                    HarnessKey::Tab => Some(TextInputKey::Tab),
                    HarnessKey::Left => Some(TextInputKey::Left),
                    HarnessKey::Right => Some(TextInputKey::Right),
                    HarnessKey::Up => Some(TextInputKey::Up),
                    HarnessKey::Down => Some(TextInputKey::Down),
                    HarnessKey::Home => Some(TextInputKey::Home),
                    HarnessKey::End => Some(TextInputKey::End),
                    HarnessKey::Delete => Some(TextInputKey::Delete),
                    HarnessKey::Unknown(_) => None,
                };
                let Some(mapped) = mapped else {
                    return;
                };

                match focused {
                    FocusedInput::TextBox => {
                        self.textbox.handle_key(mapped);
                    }
                    FocusedInput::Password => {
                        self.password.handle_key(mapped);
                    }
                    FocusedInput::Number => {
                        self.number.handle_key(mapped);
                    }
                    FocusedInput::AutoSuggest => {
                        if let Some(action) = self.suggest.handle_key(mapped) {
                            use kanesumi_controls::AutoSuggestAction;
                            match action {
                                AutoSuggestAction::Commit(s) => {
                                    self.command_result = Some(format!("建议: {s}"));
                                    self.suggest.blur();
                                }
                                AutoSuggestAction::Dismiss => {}
                                _ => {}
                            }
                        }
                    }
                    FocusedInput::None => {}
                }
            }
            InputEvent::PointerLeft => {
                // A1：指针离开窗口时若 Switch 正被拖动，取消（避免 knob 悬在中间）
                if self.pressed == Some(Target::Switch) {
                    self.switch.cancel();
                }
                self.clear_hover();
                self.pressed = None;
                self.tile_hovered = None;
                self.tile_pressed = None;
                self.tile_nav_pressed = None;
                self.sync_tile_states();
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
            Rect::new(PAD, TITLE_Y, size.width - PAD * 2.0, TITLE_H),
            colors.on_background,
            title_style,
            TextAlign::Left,
        );

        // 页导航（UWP NavigationView Top 等价）—— 永久可点，切换 GalleryPage。
        self.nav
            .render(&self.theme, engine, self.nav_rect(), &mut scene);
        // 导航栏底 1px 分割线（UWP NavigationView 分区）。
        scene.fill_rect(
            colors.divider,
            Rect::new(PAD, NAV_Y + NAV_H, size.width - PAD * 2.0, 1.0),
        );

        // 页内容：按当前 GalleryPage 分发。
        let content = self.content_rect();
        match self.page {
            GalleryPage::DesignTokens => self.render_page_tokens(engine, content, &mut scene),
            GalleryPage::Animation => self.render_page_animation(engine, content, &mut scene),
            GalleryPage::Controls => self.render_page_controls(engine, size, &mut scene),
            GalleryPage::Input => self.render_page_input(engine, &mut scene),
            GalleryPage::Structure => self.render_page_structure(engine, content, &mut scene),
            GalleryPage::Tiles => self.render_page_tiles(engine, content, &mut scene),
        }

        // ── 声明式 footer（view! + RetainedScene 增量渲染的真实 UI 区域） ──
        {
            let footer = Rect::new(
                PAD,
                size.height - FOOTER_H,
                size.width - PAD * 2.0,
                FOOTER_H - 16.0,
            );
            self.decl_rect = footer;
            let tree = self.decl_footer();
            let commands = self
                .decl_retained
                .update(&self.theme, engine, &tree, footer)
                .0
                .to_vec();
            self.decl_hits = self.decl_retained.hits().to_vec();
            scene.commands.extend(commands);
        }

        // 对话框（最上层）—— 任何页都可见。accent 按钮触发（Controls 页），
        // 但状态存活于页切换（若用户在 Controls 页打开 Dialog，切到其它页仍看得见，
        // 与 UWP `ContentDialog` 行为一致：Dialog 是应用级模态，独立于 Frame 页栈）。
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
        // A1：Switch 有 header 后 track 位于宿主 rect 顶部偏下，center 已不在 track 上
        // → 点击 track 的实际中心（switch.track_rect）
        let track = g.switch.track_rect(g.switch_rect(), &g.theme);
        click(&mut g, track);
        assert!(g.switch.checked, "点击开关应切换");
        let track = g.switch.track_rect(g.switch_rect(), &g.theme);
        click(&mut g, track);
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
        // 图标按钮应产出 Image 命令（SVG 位图管线）
        let images = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Image { .. }))
            .count();
        assert_eq!(images, 1, "Share 图标应为 SVG 位图");
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
    fn declarative_footer_routes_click() {
        let mut g = app();
        // 渲染一帧 → 填充命中表
        let engine = g.engine.clone();
        let _ = g.render(&engine, Size::new(960.0, 600.0));
        assert!(!g.decl_hits.is_empty(), "声明式 footer 应有命中项");
        // 找到计数按钮（Custom(9001)）
        let hit = g
            .decl_hits
            .iter()
            .find(|h| h.action == kanesumi_controls::DeclAction::Custom(9001))
            .expect("应有点我+1 按钮");
        let center = hit.rect.center();
        let before = g.decl_count;
        g.handle_input(InputEvent::PointerPressed {
            x: center.x,
            y: center.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: center.x,
            y: center.y,
            button: PointerButton::Left,
        });
        assert_eq!(g.decl_count, before + 1, "声明式按钮点击应计数");
    }

    #[test]
    fn tiles_page_renders_tile_fills() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let mut g = app();
        g.page = GalleryPage::Tiles;
        g.nav.select(page_index(GalleryPage::Tiles));
        let scene = g.render(&engine, Size::new(960.0, 600.0));
        // 页 0 有 5 个磁贴（含 mini/standard/large）+ 2 个翻页按钮
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
            .count();
        assert!(fills >= 7, "磁贴底色 + 翻页按钮，实际 {fills}");
        // 相册 Large 磁贴的 Live 行
        assert!(
            scene.commands.iter().any(|c| matches!(
                c,
                kanesumi_canvas::SceneCommand::Text { content, .. } if content == "海边黄昏"
            )),
            "4×2 磁贴应渲染 Live 内容行"
        );
    }

    #[test]
    fn tiles_page_nav_switches_page() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let mut g = app();
        g.page = GalleryPage::Tiles;
        g.nav.select(page_index(GalleryPage::Tiles));
        let _ = g.render(&engine, Size::new(960.0, 600.0));
        assert_eq!(g.tile_page, 0);
        // 点下一页按钮
        let next = g.tile_nav_next_rect(g.content_rect());
        let c = next.center();
        g.handle_input(InputEvent::PointerPressed {
            x: c.x,
            y: c.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: c.x,
            y: c.y,
            button: PointerButton::Left,
        });
        assert_eq!(g.tile_page, 1, "点下一页应翻到页 1");
    }

    #[test]
    fn tiles_hover_sets_tile_state() {
        let Some(p) = find_font() else { return };
        let engine = TextEngine::load(p).unwrap();
        let mut g = app();
        g.page = GalleryPage::Tiles;
        g.nav.select(page_index(GalleryPage::Tiles));
        let _ = g.render(&engine, Size::new(960.0, 600.0));
        assert_eq!(g.tile_hovered, None);
        let r = g.tile_rects[0];
        let c = r.center();
        g.handle_input(InputEvent::PointerMoved { x: c.x, y: c.y });
        assert_eq!(g.tile_hovered, Some(0));
        assert_eq!(g.tiles[0].state, ControlState::Hovered);
    }

    #[test]
    fn nav_click_switches_page_from_tiles() {
        // V18 回归：Tiles 页早期 return 挡住 nav 分支 → 一进 Tiles 就切不回其它页。
        let mut g = app();
        g.page = GalleryPage::Tiles;
        g.nav.select(page_index(GalleryPage::Tiles));
        // 点 nav 第一项（DesignTokens）应能切回
        let nav = g.nav_rect();
        let w0 = g.nav.header_width(&g.engine, 0);
        let p = Point::new(nav.origin.x + w0 / 2.0, nav.origin.y + nav.size.height / 2.0);
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
        assert_eq!(g.page, GalleryPage::DesignTokens, "从 Tiles 应能切到 DesignTokens");
    }

    #[test]
    fn nav_click_switches_page() {
        let mut g = app();
        assert_eq!(g.page, GalleryPage::Controls, "首启进 Controls 页");
        assert_eq!(g.nav.selected, page_index(GalleryPage::Controls));
        // 点击第一个 nav tab（DesignTokens）
        let nav = g.nav_rect();
        let w0 = g.nav.header_width(&g.engine, 0);
        let p = Point::new(nav.origin.x + w0 / 2.0, nav.origin.y + nav.size.height / 2.0);
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
        assert_eq!(g.page, GalleryPage::DesignTokens, "点 nav 第一项应切到 Tokens");
        assert_eq!(g.nav.selected, 0);
    }

    #[test]
    fn non_controls_page_ignores_widget_clicks() {
        let mut g = app();
        g.page = GalleryPage::DesignTokens;
        g.nav.select(page_index(GalleryPage::DesignTokens));
        // Button rect 是 Controls 页的控件；在 Tokens 页点它应无效。
        let rect = g.button_rect();
        // button 初态 = Normal
        let before = g.button.state;
        click(&mut g, rect);
        assert_eq!(g.button.state, before, "非 Controls 页 button 点击应不改状态");
    }

    // ── Input 页 ──────────────────────────────────────────────────

    fn input_page(g: &mut GalleryApp) {
        g.page = GalleryPage::Input;
        g.nav.select(page_index(GalleryPage::Input));
    }

    #[test]
    fn textbox_focus_and_type_via_keyboard() {
        let mut g = app();
        input_page(&mut g);
        // 点击 TextBox 聚焦
        let r = g.textbox_rect();
        click(&mut g, r);
        assert!(g.textbox.focused, "点击文本框应聚焦");
        // 键盘输入
        g.handle_input(InputEvent::KeyPressed {
            key: kanesumi_harness::Key::Char('X'),
        });
        g.handle_input(InputEvent::KeyPressed {
            key: kanesumi_harness::Key::Char('Y'),
        });
        // 聚焦全选 → 输入替换内容
        assert!(g.textbox.field.text().ends_with("XY"), "输入应生效");
    }

    #[test]
    fn password_typing_masks() {
        let mut g = app();
        input_page(&mut g);
        let r = g.password_rect();
        click(&mut g, r);
        assert!(g.password.focused());
        g.handle_input(InputEvent::KeyPressed {
            key: kanesumi_harness::Key::Char('a'),
        });
        g.handle_input(InputEvent::KeyPressed {
            key: kanesumi_harness::Key::Char('b'),
        });
        assert_eq!(g.password.password(), "ab");
        assert_eq!(g.password.boxed.field.display_text(), "●●");
    }

    #[test]
    fn checkbox_toggles() {
        let mut g = app();
        input_page(&mut g);
        let before = g.checkbox.state;
        let r = g.checkbox_rect();
        click(&mut g, r);
        assert_ne!(g.checkbox.state, before, "点击复选框应切换状态");
    }

    #[test]
    fn number_step_up_down() {
        let mut g = app();
        input_page(&mut g);
        g.number.set_value(50.0);
        let up = g.number.up_button_rect(&g.theme, g.number_rect());
        click(&mut g, up);
        assert_eq!(g.number.value(), Some(55.0), "步进 5");
        let down = g.number.down_button_rect(&g.theme, g.number_rect());
        click(&mut g, down);
        click(&mut g, down);
        assert_eq!(g.number.value(), Some(45.0), "两次步退");
    }

    #[test]
    fn suggest_filters_on_typing() {
        let mut g = app();
        input_page(&mut g);
        let r = g.suggest_rect();
        click(&mut g, r);
        assert!(g.suggest.focused);
        // 输入"苹"（聚焦全选 → 替换）
        g.handle_input(InputEvent::KeyPressed {
            key: kanesumi_harness::Key::Char('苹'),
        });
        assert_eq!(g.suggest.shown, vec!["苹果"], "过滤建议");
    }

    #[test]
    fn command_bar_opens_on_textbox_focus_and_routes() {
        let mut g = app();
        input_page(&mut g);
        assert!(!g.command_bar_open);
        let tb = g.textbox_rect();
        click(&mut g, tb);
        assert!(g.command_bar_open, "聚焦文本框应浮出命令条");
        // 点击 Copy 按钮（index 0）→ 记录动作
        let r = g.command_bar_rect();
        let copy_btn = Point::new(
            r.origin.x + kanesumi_controls::COMMANDBAR_BORDER + 0.5 * 40.0,
            r.origin.y + 20.0,
        );
        g.handle_input(InputEvent::PointerPressed {
            x: copy_btn.x,
            y: copy_btn.y,
            button: PointerButton::Left,
        });
        g.handle_input(InputEvent::PointerReleased {
            x: copy_btn.x,
            y: copy_btn.y,
            button: PointerButton::Left,
        });
        assert_eq!(g.command_result.as_deref(), Some("已复制"));
        assert!(!g.command_bar_open, "点命令按钮后关闭命令条");
    }

    #[test]
    fn dropdown_cascade_opens_submenu_on_hover() {
        let mut g = app();
        // Controls 页首启
        let tr = g.dropdown_trigger();
        click(&mut g, tr);
        g.update(1.0);
        assert!(g.dropdown.anim.is_visible());
        // 悬停"缩放"（index 3）→ 展开子菜单
        let r3 = g.dropdown.item_rect(3);
        let p = Point::new(r3.origin.x + 10.0, r3.origin.y + 10.0);
        g.handle_input(InputEvent::PointerMoved { x: p.x, y: p.y });
        assert!(
            g.dropdown.submenu_state().is_some(),
            "悬停嵌套项应展开子菜单"
        );
    }

    #[test]
    fn virtual_list_scrolls_and_selects() {
        let mut g = app();
        input_page(&mut g);
        let vrect = g.virtual_list_rect();
        let before = g.virtual_list.offset;
        // 滚轮滚动
        let center = vrect.center();
        g.handle_input(InputEvent::PointerMoved {
            x: center.x,
            y: center.y,
        });
        g.handle_input(InputEvent::Scroll { x: 0.0, y: 100.0 });
        assert!(g.virtual_list.offset > before, "虚拟列表应滚动");
        // 点击第一可见行 → 选中
        let rel = Point::new(20.0, 10.0);
        let p = Point::new(vrect.origin.x + rel.x, vrect.origin.y + rel.y);
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
        assert!(g.virtual_selected.is_some(), "点击应选中虚拟化条目");
    }

    #[test]
    fn virtual_list_render_clips_to_viewport() {
        let mut g = app();
        input_page(&mut g);
        let engine = g.engine.clone();
        let scene = g.render(&engine, Size::new(960.0, 600.0));
        // 1000 项列表只渲染 ~5 行可见 → Text 命令数远小于 1000
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { content, .. } if content.starts_with("项目 ")))
            .count();
        assert!(
            texts > 0 && texts < 20,
            "虚拟化只渲染可见行，实际 {texts}（应 ~5-10）"
        );
        // 滚动条出现（可滚）
        assert!(g.virtual_list.scrollbar_visible(), "1000 项应显示滚动条");
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

#[cfg(test)]
mod decl_integration {
    use super::tests::find_font;
    use kanesumi_canvas::text::TextEngine;
    use kanesumi_controls::{Decl, DeclAction, view};
    use kanesumi_core::MetroTheme;

    /// 声明式 UI 端到端：view! → render_decl → 命中 → 动作。
    #[test]
    fn declarative_button_renders_and_hits() {
        let Some(p) = find_font() else {
            return;
        };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();

        let tree: Decl = view! {
            Column {
                view!(Button { label: "打开对话框".to_string(), accent: true, action: DeclAction::OpenDialog }),
                view!(Button { label: "取消".to_string(), accent: false, action: DeclAction::Custom(1) })
            }
        };

        let (scene, hits) = kanesumi_controls::render_decl(
            &theme,
            &engine,
            &tree,
            kanesumi_core::Rect::new(0.0, 0.0, 200.0, 80.0),
        );
        assert!(!scene.commands.is_empty(), "渲染出命令");
        assert_eq!(hits.len(), 2, "两个按钮都可命中");
        assert_eq!(hits[0].action, DeclAction::OpenDialog);
        assert_eq!(hits[1].action, DeclAction::Custom(1));
        // V8 修复后：按钮取内在高度（line_height + 11 = 22 + 11 = 33），
        // 相邻按钮间加 Column 默认 spacing=8 —— 不再等分 80/2=40。
        let intr_h = kanesumi_controls::MetroButton::accent("")
            .measure(&engine, theme.typography.body)
            .height;
        assert!(
            (hits[0].rect.size.height - intr_h).abs() < 0.5,
            "按钮内在高度，实际 {}",
            hits[0].rect.size.height
        );
        assert!(
            (hits[1].rect.origin.y - (intr_h + 8.0)).abs() < 0.5,
            "第二按钮起点 = 第一按钮高 + spacing 8，实际 {}",
            hits[1].rect.origin.y
        );
    }

    /// 动作路由：命中表 → 触发（验证声明式 UI 与 App 逻辑的接线）。
    #[test]
    fn action_routes_to_app_behavior() {
        let Some(p) = find_font() else {
            return;
        };
        let engine = TextEngine::load(p).unwrap();
        let theme = MetroTheme::ether_dark();
        let tree: Decl = view! {
            Row {
                view!(Button { label: "打开".to_string(), accent: true, action: DeclAction::OpenDialog })
            }
        };
        let (_, hits) = kanesumi_controls::render_decl(
            &theme,
            &engine,
            &tree,
            kanesumi_core::Rect::new(0.0, 0.0, 200.0, 40.0),
        );
        // 模拟点击第一个命中矩形中心 → 应路由到 OpenDialog
        let hit = &hits[0];
        let center = hit.rect.center();
        let triggered = hit.rect.contains(center);
        assert!(triggered, "命中矩形应包含其中心");
        assert_eq!(hit.action, DeclAction::OpenDialog);
    }
}
