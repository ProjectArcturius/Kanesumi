use std::cell::Cell;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::sizes::ICON_12;
use kanesumi_core::{MetroTheme, Point, Rect, Size, TextStyle, TextVAlign};

use kanesumi_element::{
    AccessInfo, AccessRole, ArrangeCtx, Event, EventCtx, Insets, Key, MeasureCtx, PaintCtx,
    PointerButton, UpdateCtx, Widget,
};

use crate::popup::{PopupAnim, PopupState, place_submenu, popup_gap, render_overlay, snap_edge};

/// 菜单面板的无障碍名字（菜单自身没有标题，AT 需要**类型名级**的落点）。
///
/// 不取第一项标签当名字：菜单被 AT 读成「新建，菜单」会把条目错当控件名，
/// 而条目在第一期还只是面板内绘制的内容（见 `access.rs` 头注）。
pub(crate) const MENU_ACCESS_NAME: &str = "菜单";

/// 留白式分组间距（逻辑 pt）：以空白表达组间分界，**不画线**。
/// 参 docs/DECISIONS_2026-10-05.md §100（TopBar 不要分割线，分组改用留白）。
pub const MENU_GROUP_GAP: f32 = 8.0;

/// 勾选列宽（逻辑 pt）：菜单有可勾选项时，左缘固定留出这一列画勾，标签统一右移 ——
/// 勾选与否**不改变文字起点**（旧做法是在标签里拼「✓」前缀，勾了的行文字整体右移一格，
/// 一列命令的下缘看得出来错位）。参 CONTROL_SPEC §39「视觉」。
pub const MENU_CHECK_GUTTER: f32 = 16.0;

/// 面板最大高度（逻辑 pt）。超过则面板封顶、内容内部滚动。
///
/// WinUI 的 `MenuFlyoutPresenter` 没有公开的 `MaxHeight`（模板里零声明）；504 取自同一族的
/// ComboBox 下拉 `MaxDropDownHeight`（CONTROL_SPEC §8 一手源），**属推断值**、登记在
/// `CANON_VS_TEMPORARY` 待真机对照。不封顶的后果是长菜单直接长到屏幕外，
/// `place_popup` 只会平移，用户看到的是一个被裁掉一半、又没有滚动条的菜单。
pub const MENU_MAX_HEIGHT: f32 = 504.0;

/// 菜单项。参 CONTROL_SPEC §8（MenuFlyout）：
/// - 项高 ≈32（Padding `11,9,11,10` + 14px 字）；图标 16px；快捷键右侧；
/// - PointerOver = 中性高亮，瞬时；分隔线高 1、左右 12 留白。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub label: String,
    pub icon: Option<String>,
    pub shortcut: Option<String>,
    /// Toggle 项勾选状态。
    pub checked: bool,
    /// 单选组名（RadioMenuFlyoutItem 语义：同组内勾选互斥，参 CONTROL_SPEC §39）。
    pub radio_group: Option<String>,
    /// 项后加分隔线。
    pub separator_after: bool,
    /// 项后加**留白式**分组间距（不画线）。与 `separator_after` 独立；同时置位时
    /// 以留白为准（不画线）——TopBar 依 §100 用它取代分割线。
    pub gap_after: bool,
    /// 嵌套子菜单（MenuFlyoutSubItem 语义，参 CONTROL_SPEC §39）。
    pub submenu: Vec<MenuItem>,
    /// 可用（XAML `MenuFlyoutItem.IsEnabled`）。`false` = 不响应悬停 / 键盘 / 点击，
    /// 文字按禁用透明度画。剪切 / 粘贴这类「现在做不了」的命令必须有这个状态，
    /// 否则应用只能把项删掉或让它点了没反应。
    pub enabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            shortcut: None,
            checked: false,
            radio_group: None,
            separator_after: false,
            gap_after: false,
            submenu: Vec::new(),
            enabled: true,
        }
    }

    /// 禁用项（灰显、不响应悬停与点击、键盘导航跳过）。
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// 右侧快捷键提示（纯展示，不注册加速键；加速键归应用 `on_key`）。
    pub fn with_shortcut(mut self, text: impl Into<String>) -> Self {
        self.shortcut = Some(text.into());
        self
    }

    pub fn with_icon(label: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            icon: Some(icon.into()),
            ..Self::new(label)
        }
    }

    pub fn separator(mut self) -> Self {
        self.separator_after = true;
        self
    }

    /// 项后加留白式分组间距（不画线，参 [`MENU_GROUP_GAP`]）。
    /// 与 [`Self::separator`] 二选一：TopBar 依 §100 用留白取代分割线。
    pub fn gap(mut self) -> Self {
        self.gap_after = true;
        self
    }

    /// 带嵌套子菜单（MenuFlyoutSubItem）。
    pub fn with_submenu(mut self, items: Vec<MenuItem>) -> Self {
        self.submenu = items;
        self
    }

    /// 单选组项（RadioMenuFlyoutItem）。
    pub fn radio(mut self, group: impl Into<String>) -> Self {
        self.radio_group = Some(group.into());
        self
    }

    /// 是否嵌套项（有子菜单）。
    pub fn is_submenu(&self) -> bool {
        !self.submenu.is_empty()
    }
}

/// 子菜单状态 —— 顶层菜单某嵌套项展开的二级菜单。
#[derive(Debug, Clone)]
pub struct SubmenuState {
    /// 顶层父项索引（触发展开的项）。
    pub parent: usize,
    /// 子菜单面板矩形（父项右侧，垂直对齐）。
    pub panel: Rect,
    /// 子菜单自身（递归复用 MetroDropdownMenu；Box 打破无限递归尺寸）。
    pub menu: Box<MetroDropdownMenu>,
}

impl PartialEq for SubmenuState {
    fn eq(&self, other: &Self) -> bool {
        self.parent == other.parent
            && self.panel == other.panel
            && self.menu.items == other.menu.items
            && self.menu.hovered == other.menu.hovered
    }
}

/// MetroDropdownMenu —— 下拉菜单（MenuFlyout 参考）。参 CONTROL_SPEC §8：
/// - 弹出 = 遮罩淡入 0.383s + 面板 0.30s 展开；
/// - 项高 32、图标 16、快捷键右对齐；PointerOver 中性高亮。
/// - 二级级联（§39）：悬停嵌套项展开子菜单；点击子菜单项返回 `(parent, child)`。
///
/// **V21 缓存**：`panel_size` 遍历 items 逐条 `engine.measure` —— items 稳定时可缓存。
/// 外部修改 `items` / `item_height` 后需调 [`Self::invalidate_layout`]（Rust 无法拦截
/// pub 字段写）。缓存不区分 engine 实例，隐含约定：应用生命周期内 TextEngine 单例。
#[derive(Debug, Clone)]
pub struct MetroDropdownMenu {
    pub items: Vec<MenuItem>,
    pub hovered: Option<usize>,
    /// 展开的子菜单（Some = 级联打开中）。
    pub submenu: Option<SubmenuState>,
    /// 项高（UWP 32）。修改后调 `invalidate_layout`。
    pub item_height: f32,
    pub anim: PopupAnim,
    /// 锚点（面板相对触发器的弹出位置）。顶级弹层方向由外部 [`crate::popup::place_popup`]
    /// 决定；二级子菜单方向由 [`Self::open_submenu`] 内部走 [`crate::popup::place_submenu`]。
    pub panel_rect: Rect,
    /// `panel_size` 结果缓存（Size: Copy，直接 Cell）。None = 需重算。
    panel_size_cache: Cell<Option<kanesumi_core::Size>>,
    /// 树弹层的锚点控件（元素树路径，[`Widget::event`] 据此发 `MenuInvoked`）。
    /// `None` = 旧路径纯绘用法，不发动作。参 `menu_flyout.rs` / `context_menu.rs`。
    pub(crate) owner: Option<kanesumi_element::WidgetId>,
    /// 内容滚动偏移（逻辑 pt）。面板封顶 [MENU_MAX_HEIGHT] 后靠它看到后面的项。
    pub scroll: f32,
    /// 最近一次已知的指针位置（滚轮事件不带坐标，靠它判断指针是否在面板上）。
    last_pointer: Option<Point>,
    /// 面板高度上限（默认 [MENU_MAX_HEIGHT]；子菜单 / 测试可改小）。
    pub max_height: f32,
    /// 级联子菜单与父面板的水平间距（逻辑 pt；UWP 为 2，可配）。
    pub submenu_gap: f32,
    /// 正在拖动右侧滚动指示条（拖动期间不改悬停项、不触发项激活）。
    hint_drag: bool,
}

impl MetroDropdownMenu {
    pub fn new(items: Vec<MenuItem>) -> Self {
        Self {
            items,
            hovered: None,
            submenu: None,
            item_height: 32.0,
            anim: PopupAnim::new(),
            panel_rect: Rect::new(0.0, 0.0, 0.0, 0.0),
            panel_size_cache: Cell::new(None),
            owner: None,
            scroll: 0.0,
            last_pointer: None,
            max_height: MENU_MAX_HEIGHT,
            submenu_gap: 2.0,
            hint_drag: false,
        }
    }

    /// 清空 `panel_size` 缓存 —— 修改 `items` / `item_height` / 字体大小后必须调。
    pub fn invalidate_layout(&self) {
        self.panel_size_cache.set(None);
    }

    pub fn open(&mut self, at: Rect) {
        self.panel_rect = at;
        // 每次打开都从头看：保留上次的滚动位置会让菜单「莫名其妙停在中间」。
        self.scroll = 0.0;
        self.anim.open();
    }

    pub fn close(&mut self) {
        self.anim.close();
        self.close_submenu();
    }

    pub fn toggle(&mut self, at: Rect) {
        if self.anim.is_open() {
            self.close();
        } else {
            self.open(at);
        }
    }

    pub fn update(&mut self, dt: f64) {
        self.anim.update(dt);
        self.update_submenu(dt);
    }

    pub fn state(&self) -> PopupState {
        self.anim.state()
    }

    /// 悬停语义签名：`(顶层 hovered, 子菜单 (父项索引, 子菜单 hovered))`。
    /// S1 输入门控：宿主在「指针纯 Move + 无按键」前后比对签名，未变化则不置脏。
    pub fn interaction_signature(&self) -> MenuInteractionSignature {
        Some((
            self.hovered,
            self.submenu.as_ref().map(|s| (s.parent, s.menu.hovered)),
        ))
    }

    /// 是否处于开/关动画推进中（Opening/Closing）。静态 Open 不需要逐帧重绘
    /// （宿主 needs_redraw 脏标记语义用，参 TOPBAR_RENDER_REFACTOR §4.6）。
    pub fn is_animating(&self) -> bool {
        matches!(self.anim.state(), PopupState::Opening | PopupState::Closing)
            || self.submenu.as_ref().is_some_and(|s| s.menu.is_animating())
    }

    /// 当前悬停项矩形（顶层 + 子菜单各一），无悬停返回空。S4 局部损坏重绘用
    /// （宿主取「旧 + 新」并集作为 damage 矩形 —— 悬停高亮只重绘这一小区域）。
    pub fn hovered_rects(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        if let Some(i) = self.hovered {
            out.push(self.item_rect(i));
        }
        if let Some(s) = self.submenu.as_ref()
            && let Some(k) = s.menu.hovered
        {
            out.push(s.menu.item_rect(k));
        }
        out
    }

    /// 面板尺寸：宽 = 最宽项（含图标/快捷键占位）+ 边距，高 = 项数 × 项高 + 分隔线。
    ///
    /// **V21 缓存**：结果按 `items` / `item_height` 稳定，首次计算后走 `panel_size_cache`。
    /// 外部修改字段后需 [`Self::invalidate_layout`]（Cell 无法拦截 pub 字段写入）。
    /// 勾选列宽：菜单里存在**可勾选项**（勾选态或单选组）时预留，否则为 0。
    ///
    /// 判据是「有没有可勾选的项」而不是「当前有没有勾」——按当前勾选态决定会让
    /// 取消勾选的一瞬间整列文字左移，看起来像菜单自己抖了一下。
    pub fn check_gutter(&self) -> f32 {
        let checkable = self
            .items
            .iter()
            .any(|i| i.checked || i.radio_group.is_some());
        if checkable { MENU_CHECK_GUTTER } else { 0.0 }
    }

    pub fn panel_size(&self, engine: &TextEngine) -> kanesumi_core::Size {
        if let Some(cached) = self.panel_size_cache.get() {
            return cached;
        }
        let style = menu_item_style();
        let text_w = self
            .items
            .iter()
            .map(|i| engine.measure(&i.label, style.size))
            .fold(0.0, f32::max);
        let icon_w = if self.items.iter().any(|i| i.icon.is_some()) {
            28.0
        } else {
            0.0
        };
        let shortcut_w = self
            .items
            .iter()
            .filter_map(|i| i.shortcut.as_ref())
            .map(|s| engine.measure(s, style.size))
            .fold(0.0, f32::max);
        let width = (self.check_gutter() + icon_w + text_w + shortcut_w + 22.0 + 24.0).max(120.0);
        let size = kanesumi_core::Size::new(width, self.content_height().min(self.max_height));
        self.panel_size_cache.set(Some(size));
        size
    }

    /// 内容总高（项 + 分隔线 + 分组留白），不受面板最大高度限制。
    pub fn content_height(&self) -> f32 {
        let separators = self
            .items
            .iter()
            .filter(|i| i.separator_after && !i.gap_after)
            .count() as f32;
        let gaps = self.items.iter().filter(|i| i.gap_after).count() as f32;
        self.items.len() as f32 * self.item_height + separators * 2.0 + gaps * MENU_GROUP_GAP
    }

    /// 可滚动距离（内容高 − 面板高）。面板装得下时为 0。
    pub fn max_scroll(&self) -> f32 {
        (self.content_height() - self.panel_rect.size.height).max(0.0)
    }

    /// 滚动（`dy` 逻辑像素，正 = 内容上移 = 看到更靠后的项），夹在 `[0, max_scroll]`。
    /// 返回是否真的动了（宿主据此决定是否重绘）。
    pub fn scroll_by(&mut self, dy: f32) -> bool {
        let before = self.scroll;
        self.scroll = (self.scroll + dy).clamp(0.0, self.max_scroll());
        (self.scroll - before).abs() > 0.01
    }

    /// 把第 `i` 项滚进可见区（键盘导航用；已可见时不动）。
    fn scroll_into_view(&mut self, i: usize) {
        let r = self.item_rect(i);
        let top = self.panel_rect.origin.y;
        let bottom = self.panel_rect.bottom();
        if r.origin.y < top {
            self.scroll = (self.scroll - (top - r.origin.y)).max(0.0);
        } else if r.bottom() > bottom {
            self.scroll = (self.scroll + (r.bottom() - bottom)).min(self.max_scroll());
        }
    }

    /// 项命中测试（相对面板原点，面板以 `panel_rect.origin` 起排）。
    /// 布局与渲染一致：每项占 `item_height`，项后分隔线再占 2px（参 `render_panel`）。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.x < self.panel_rect.origin.x
            || pos.x >= self.panel_rect.origin.x + self.panel_rect.size.width
        {
            return None;
        }
        let local_y = pos.y - self.panel_rect.origin.y + self.scroll;
        let mut y = 0.0;
        for (i, item) in self.items.iter().enumerate() {
            let h = self.item_height;
            if local_y >= y && local_y < y + h {
                return Some(i);
            }
            y += h;
            if item.gap_after {
                // 留白式分组间距：不画线，间隙不可命中。
                y += MENU_GROUP_GAP;
            } else if item.separator_after {
                // 分隔线本身不可命中；+2px 计入下一项起点。
                if local_y >= y - 1.0 && local_y < y + 1.0 {
                    return None;
                }
                y += 2.0;
            }
        }
        None
    }

    /// 顶层第 `i` 项的矩形（面板内）。
    ///
    /// 纵向边缘吸附到物理像素网格：相邻项的悬停底共用边缘，2× 下不会互相露 1 px 缝
    /// （真机已知问题 #18）。面板原点本身也可能带小数，所以从吸附后的原点起算。
    pub fn item_rect(&self, i: usize) -> Rect {
        let mut y = snap_edge(self.panel_rect.origin.y - self.scroll);
        for (k, item) in self.items.iter().enumerate() {
            if k == i {
                break;
            }
            y = snap_edge(y + self.item_height);
            if item.gap_after {
                y = snap_edge(y + MENU_GROUP_GAP);
            } else if item.separator_after {
                y = snap_edge(y + 2.0);
            }
        }
        let next = snap_edge(y + self.item_height);
        Rect::new(
            self.panel_rect.origin.x,
            y,
            self.panel_rect.size.width,
            (next - y).max(0.0),
        )
    }

    /// 悬停路由：命中顶层项；若该项有子菜单且未展开 → 自动展开（级联）。
    /// 子菜单展开时悬停其它项 → 收起当前子菜单（hover-swap 语义，对齐 MenuBar §8.5）。
    ///
    /// `screen` 供子菜单方向自适应（右侧越屏翻左 / 下缘越屏上移），参 [`place_submenu`]。
    ///
    /// **跨缝隙保持**：指针从父项移入已开子菜单时，会经过父项↔子菜单之间的桥接带
    /// （水平条：顶层面板左缘 → 子菜单右缘，覆盖子菜单 y 范围）。在桥接带内保持
    /// 展开，否则移动途中子菜单会闪没（旧 bug）。
    ///
    /// 返回本次调用是否改变了悬停语义（顶层 `hovered` / 子菜单开合 / 子菜单 `hovered`）——
    /// 宿主（harness）据此做 S1 输入门控：菜单静止时纯 Move 不再触发整帧重绘。
    pub fn hover(&mut self, engine: &TextEngine, screen: Rect, pos: Point) -> bool {
        let before = self.interaction_signature();
        self.hover_inner(engine, screen, pos);
        before != self.interaction_signature()
    }

    /// `hover` 主体（额外包装用于前后签名比对）。
    fn hover_inner(&mut self, engine: &TextEngine, screen: Rect, pos: Point) {
        // 子菜单已开：指针在子菜单面板内 → 子菜单接管（悬停其项高亮）；
        // 在桥接带内 → 保持展开（不收起、不改父项高亮）。
        if let Some(s) = self.submenu.as_ref() {
            if s.panel.contains(pos) {
                self.submenu.as_mut().map(|m| m.menu.hover(engine, screen, pos));
                return;
            }
            let bridge = Rect::new(
                self.panel_rect.origin.x,
                s.panel.origin.y,
                s.panel.right() - self.panel_rect.origin.x,
                s.panel.size.height,
            );
            if bridge.contains(pos) {
                return;
            }
        }
        self.hovered = self.item_at(pos).filter(|i| {
            // 禁用项不亮、也不展开子菜单：指针扫过时高亮停在上一项才是「坏手感」，
            // 所以这里把 hovered 清成 None（Windows 同款：禁用行不出现悬停底）。
            self.items.get(*i).is_some_and(|it| it.enabled)
        });
        let submenu_target = self.hovered.filter(|i| self.items[*i].is_submenu());
        if let Some(i) = submenu_target {
            if self.submenu.as_ref().map(|s| s.parent) != Some(i) {
                self.open_submenu(engine, screen, i);
            }
        } else {
            self.submenu = None;
        }
    }

    /// 展开第 `i` 项的二级子菜单（若该项有 submenu）。子菜单面板方向由
    /// [`place_submenu`] 决定：默认父项右侧展开；右缘越屏 → 翻左；下缘越屏 → 上移。
    pub fn open_submenu(&mut self, engine: &TextEngine, screen: Rect, idx: usize) {
        let items = self.items.get(idx).map(|it| it.submenu.clone());
        let Some(items) = items else {
            return;
        };
        if items.is_empty() {
            self.submenu = None;
            return;
        }
        let parent_rect = self.item_rect(idx);
        let mut menu = MetroDropdownMenu::new(items);
        // 行高与高度上限跟随父菜单：级联面板不能比父面板高，也不能用另一套行高。
        menu.item_height = self.item_height;
        menu.max_height = self.max_height;
        let sub_size = menu.panel_size(engine);
        // 间距可配（默认 2，UWP 值）；配 0 时退回随缩放取整的通用间距。
        let gap = if self.submenu_gap > 0.0 {
            self.submenu_gap
        } else {
            popup_gap().min(2.0)
        };
        let panel = place_submenu(parent_rect, sub_size, screen, gap);
        menu.panel_rect = panel;
        menu.anim.open();
        self.submenu = Some(SubmenuState {
            parent: idx,
            panel,
            menu: Box::new(menu),
        });
    }

    /// 关闭子菜单。
    pub fn close_submenu(&mut self) {
        if let Some(mut s) = self.submenu.take() {
            s.menu.close();
        }
    }

    /// 命中测试（含子菜单）：先查子菜单项，再查顶层项。
    /// 返回 `MenuPath { parent: Option<usize>, index: usize }` —— `parent=None` 为顶层项。
    ///
    /// 名字用 `try_*`：这是「可能查不到」的查询（空白处 / 已关闭 → `None`），
    /// 调用方从名字就该知道要处理失败。参 docs/DEV_GUIDE.md §八·五。
    pub fn try_path_at(&self, pos: Point) -> Option<MenuPath> {
        let sub_hit = self
            .submenu
            .as_ref()
            .filter(|s| s.panel.contains(pos))
            .and_then(|s| s.menu.item_at(pos).map(|c| (s.parent, c)));
        if let Some((parent, child)) = sub_hit {
            return Some(MenuPath {
                parent: Some(parent),
                index: child,
            });
        }
        self.item_at(pos).map(|i| MenuPath {
            parent: None,
            index: i,
        })
    }

    /// 旧名（`try_*` 约定之前）。保留一个周期供下游迁移，之后删除。
    #[deprecated(note = "改用 try_path_at：可能查不到的查询统一 try_* 前缀")]
    pub fn path_at(&self, pos: Point) -> Option<MenuPath> {
        self.try_path_at(pos)
    }

    /// 子菜单悬停项（渲染用）。
    pub fn submenu_hovered(&self) -> Option<usize> {
        self.submenu.as_ref().and_then(|s| s.menu.hovered)
    }

    /// 子菜单渲染用：当前子菜单引用。
    pub fn submenu_state(&self) -> Option<&SubmenuState> {
        self.submenu.as_ref()
    }

    /// 渲染：遮罩 + 面板（项 + 分隔线 + 悬停高亮）。（子菜单由 [`Self::render_submenu`] 单独渲染）
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene) {
        if !self.anim.is_visible() {
            return;
        }
        render_overlay(theme, &self.anim, screen, scene);
        self.render_panel(theme, engine, scene);
    }

    /// 渲染面板本体（**无遮罩**）——右键菜单等无遮罩弹层复用。参 CONTEXT_MENU_SPEC §Ⅳ。
    /// 可见性仍受 `anim.is_visible()` 门控（关闭动画期间照常渲染直到收起）。
    pub fn render_panel(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        if !self.anim.is_visible() {
            return;
        }
        crate::popup::render_panel_base(theme, self.panel_rect, self.anim.panel_progress(), scene);

        let style = menu_item_style();
        let colors = &theme.colors;
        // 容器语义 = 裁到面板矩形（2026-09-22 审计 P0-2）：开启动画期间面板只露出一部分，
        // 各项仍按最终坐标绘制 → 不裁会画到面板外。
        scene.push_clip(self.panel_rect);
        // 边缘吸附到物理像素网格，与 `item_rect` 同源：相邻项的悬停底共用边缘，
        // 2x 下不会露 1 px 缝（真机已知问题 #18）。
        let mut y = snap_edge(self.panel_rect.origin.y - self.scroll);
        for (i, item) in self.items.iter().enumerate() {
            let next = snap_edge(y + self.item_height);
            let item_rect = Rect::new(
                self.panel_rect.origin.x,
                y,
                self.panel_rect.size.width,
                (next - y).max(0.0),
            );

            // 悬停高亮（中性）+ 级联展开项也高亮（禁用项不亮，见 `hover_inner`）。
            if self.hovered == Some(i)
                || self.submenu.as_ref().map(|s| s.parent) == Some(i)
            {
                scene.fill_rect(theme.indication.hover_tint, item_rect);
            }

            // 禁用项整体降透明度：文字 / 图标 / 快捷键 / chevron 一起暗，不单暗一个。
            let dim = |c: kanesumi_core::Color| {
                if item.enabled {
                    c
                } else {
                    c.with_alpha(theme.indication.disabled_opacity)
                }
            };
            let mut x = self.panel_rect.origin.x + 11.0;
            // 勾选列：有可勾选项时左缘留一列，勾在这里画、标签统一起点（勾与不勾不位移）。
            if self.check_gutter() > 0.0 {
                if item.checked {
                    let check_rect = Rect::new(
                        self.panel_rect.origin.x + 11.0,
                        y + (self.item_height - ICON_12) / 2.0,
                        ICON_12,
                        ICON_12,
                    );
                    kanesumi_canvas::glyph::check(scene, check_rect, dim(colors.on_surface));
                }
                x += MENU_CHECK_GUTTER;
            }            // 图标
            if let Some(icon) = &item.icon {
                let icon_rect = Rect::new(x, y + (self.item_height - 16.0) / 2.0, 16.0, 16.0);
                scene.text(
                    icon.clone(),
                    icon_rect,
                    dim(colors.on_surface),
                    icon_style(),
                    TextAlign::Center,
                );
                x += 28.0;
            }
            // 快捷键（右侧，右对齐）：**先量它，再让标签避让** —— 两者不得重叠。
            // 旧实现 rect 宽 = 量测宽且标签不知情 → 超长快捷键会压到标签上，且永远不会省略
            // （2026-09-22 审计 P0-1）。
            let shortcut = item.shortcut.clone();
            let sc_avail = (self.panel_rect.size.width - 24.0).max(0.0);
            let sc_w = shortcut
                .as_ref()
                .map(|s| engine.measure(s, style.size).min(sc_avail))
                .unwrap_or(0.0);
            // 文本 —— 宽度 = 面板右缘（含右内边距 11）到当前笔位，再为快捷键留出 sc_w + 12。
            // 注意：panel_rect.size.width 是相对宽度，不能直接减 x（x 是绝对坐标）。
            // 旧 bug：`panel_rect.size.width - x` 得到巨大负值 → engine.layout 触发
            // CJK 单字硬断 → 菜单项变成字塔（每个字一个字）。
            let reserve = if shortcut.is_some() { sc_w + 12.0 } else { 0.0 };
            let text_right = self.panel_rect.right() - 11.0 - reserve;
            let text_w = (text_right - x).max(0.0);
            // 纵向交给 TextVAlign::Center（行盒中线对齐 item 中线，与手算 (h−lh)/2
            // 逐位同值）。参 o4 纵向对齐。
            let text_rect = Rect::new(x, y, text_w, self.item_height);
            scene.label(
                item.label.clone(),
                text_rect,
                dim(colors.on_surface),
                style.with_v_align(TextVAlign::Center),
                TextAlign::Left,
            );
            // 子菜单指示（chevron right）
            if item.is_submenu() {
                let chevron_rect = Rect::new(
                    self.panel_rect.right() - 22.0,
                    y + (self.item_height - ICON_12) / 2.0,
                    ICON_12,
                    ICON_12,
                );
                kanesumi_canvas::glyph::chevron_right(
                    scene,
                    chevron_rect,
                    dim(colors.on_surface_variant),
                );
            }
            // 快捷键（右侧）：矩形由面板右缘派生并夹到可用宽，单行省略号。
            // 纵向同上交给 TextVAlign::Center。参 o4 纵向对齐。
            if let Some(sc) = shortcut {
                let sc_rect = Rect::new(
                    self.panel_rect.right() - 11.0 - sc_w,
                    y,
                    sc_w,
                    self.item_height,
                );
                scene.label(
                    sc,
                    sc_rect,
                    dim(colors.on_surface_variant),
                    style.with_v_align(TextVAlign::Center),
                    TextAlign::Right,
                );
            }

            y = next;
            if item.gap_after {
                // 留白式分组间距：只留空，不画线。
                y = snap_edge(y + MENU_GROUP_GAP);
            } else if item.separator_after {
                // 分隔线
                let sep_rect = Rect::new(
                    self.panel_rect.origin.x + 12.0,
                    y,
                    self.panel_rect.size.width - 24.0,
                    1.0,
                );
                scene.fill_rect(colors.divider, sep_rect);
                y = snap_edge(y + 2.0);
            }
        }
        
        // 滚动指示：面板封顶后右侧一条细条，thumb 高度按可见比例。没有指示时
        // 用户只看到「最后一项被切了一半」，不会想到还能滚。
        self.render_scroll_hint(theme, scene);
        scene.pop_clip();
    }

    /// 滚动指示条的 thumb 矩形（装得下时 `None`）。
    ///
    /// 渲染与拖动命中**共用这一处**几何：分开算的话，看到的位置和能拖的位置会差几个像素。
    fn scroll_hint_rect(&self) -> Option<Rect> {
        let max = self.max_scroll();
        if max <= 0.0 {
            return None;
        }
        const BAR_W: f32 = 3.0;
        const INSET: f32 = 4.0;
        let panel = self.panel_rect;
        let track_h = (panel.size.height - INSET * 2.0).max(0.0);
        let visible_ratio = (panel.size.height / self.content_height()).clamp(0.08, 1.0);
        let thumb_h = (track_h * visible_ratio).max(24.0).min(track_h);
        let progress = (self.scroll / max).clamp(0.0, 1.0);
        let thumb_y = panel.origin.y + INSET + (track_h - thumb_h) * progress;
        Some(Rect::new(
            panel.right() - INSET - BAR_W,
            snap_edge(thumb_y),
            BAR_W,
            thumb_h,
        ))
    }

    /// 滚动指示条的**命中区**：可见条只有 3pt 宽，直接拿来判命中等于点不中。
    /// 向左扩 8pt（面板右缘那一条），纵向用 thumb 的范围。
    fn scroll_hint_hit_rect(&self) -> Option<Rect> {
        let thumb = self.scroll_hint_rect()?;
        Some(Rect::new(
            thumb.origin.x - 8.0,
            thumb.origin.y,
            thumb.size.width + 8.0,
            thumb.size.height,
        ))
    }
    /// 拖动 thumb 到指针纵向位置（`pos.y`），换算成滚动偏移并夹紧。
    fn drag_hint_to(&mut self, pos: Point) -> bool {
        let max = self.max_scroll();
        if max <= 0.0 {
            return false;
        }
        const INSET: f32 = 4.0;
        let Some(thumb) = self.scroll_hint_rect() else {
            return false;
        };
        let track_top = self.panel_rect.origin.y + INSET;
        let track_h = (self.panel_rect.size.height - INSET * 2.0).max(0.0);
        let free = (track_h - thumb.size.height).max(1.0);
        // 让指针落在 thumb 中央附近也无所谓：按「thumb 顶端 = 指针 − 半 thumb 高」算，
        // 手感更跟手（按下时不会突然跳一下）。
        let target_top = pos.y - thumb.size.height / 2.0;
        let progress = ((target_top - track_top) / free).clamp(0.0, 1.0);
        let before = self.scroll;
        self.scroll = progress * max;
        (self.scroll - before).abs() > 0.01
    }

    /// 右侧细滚动条（仅面板装不下时画）。
    fn render_scroll_hint(&self, theme: &MetroTheme, scene: &mut Scene) {
        let Some(thumb) = self.scroll_hint_rect() else {
            return;
        };
        let color = theme
            .colors
            .on_surface_variant
            .with_alpha(theme.indication.base_medium_low);
        scene.fill_rounded_rect(color, thumb, kanesumi_core::CornerRadius::Capsule);
    }

    /// 渲染子菜单（若展开）。画在顶层面板之上。
    ///
    /// **委托给子菜单自己的 [`Self::render_panel`]，不再另写一份排版**：
    /// 旧实现是逐行复制顶层面板的绘制代码，于是「顶层的改进」到不了子菜单 ——
    /// 勾选列、禁用降透明度、图标让位、快捷键避让在两级菜单里长相不同，
    /// 而这类复制粘贴的分叉正是自绘框架最贵的债（改一处忘一处，症状只在子菜单里出现）。
    pub fn render_submenu(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let Some(sub) = &self.submenu else {
            return;
        };
        sub.menu.render_panel(theme, engine, scene);
    }

    /// 子菜单 `update(dt)`（动画推进）。
    pub fn update_submenu(&mut self, dt: f64) {
        if let Some(s) = &mut self.submenu {
            s.menu.update(dt);
        }
    }

    /// 选中顶层项（RadioMenuFlyoutItem 单选组语义：同组内互斥取消其它勾选）。
    /// 返回是否发生了状态变化。
    pub fn select(&mut self, idx: usize) -> bool {
        let Some(item) = self.items.get_mut(idx) else {
            return false;
        };
        let Some(group) = item.radio_group.clone() else {
            return false;
        };
        if item.checked {
            return false;
        }
        item.checked = true;
        // 同组互斥：取消其它同组项勾选
        for (k, it) in self.items.iter_mut().enumerate() {
            if k != idx && it.radio_group.as_deref() == Some(group.as_str()) {
                it.checked = false;
            }
        }
        true
    }

    /// 选中子菜单项（`path` 定位到子菜单）。返回是否变化。
    pub fn select_submenu(&mut self, path: MenuPath) -> bool {
        let Some(sub) = &mut self.submenu else {
            return false;
        };
        let Some(parent_item) = self.items.get_mut(path.parent.unwrap_or(0)) else {
            return false;
        };
        let group = parent_item
            .submenu
            .get(path.index)
            .and_then(|it| it.radio_group.clone());
        let Some(group) = group else {
            return false;
        };
        let item = match sub.menu.items.get_mut(path.index) {
            Some(it) => it,
            None => return false,
        };
        if item.checked {
            return false;
        }
        item.checked = true;
        for (k, it) in sub.menu.items.iter_mut().enumerate() {
            if k != path.index && it.radio_group.as_deref() == Some(group.as_str()) {
                it.checked = false;
            }
        }
        // 回写到父项的子菜单定义：子菜单面板是展开时的临时副本，只改副本的话
        // 收起再展开选择就丢了（2026-09-30 MenuFlyout 键盘测试发现）。
        if let Some(parent) = path.parent.and_then(|p| self.items.get_mut(p)) {
            for (k, it) in parent.submenu.iter_mut().enumerate() {
                if it.radio_group.as_deref() == Some(group.as_str()) {
                    it.checked = k == path.index;
                }
            }
        }
        true
    }

    // ── 元素树弹层的共享交互（Widget 实现，参 `menu_flyout.rs` / `context_menu.rs`）──

    /// 键盘步进（↑ / ↓，跳过分隔线与禁用项、循环）。子菜单展开时作用于子菜单。
    pub fn step_hover(&mut self, delta: isize) {
        let mut next = None;
        {
            let (items, hovered) = match self.submenu.as_ref() {
                Some(s) => (s.menu.items.as_slice(), s.menu.hovered),
                None => (self.items.as_slice(), self.hovered),
            };
            let n = items.len() as isize;
            if n == 0 {
                return;
            }
            let mut cursor = match hovered {
                Some(i) => i as isize,
                None if delta > 0 => -1,
                None => n,
            };
            // 至多走一圈：全是禁用项时停在原位、不选中任何项。
            for _ in 0..n {
                cursor += delta.signum();
                let idx = cursor.rem_euclid(n) as usize;
                if items[idx].enabled {
                    next = Some(idx);
                    break;
                }
            }
        }
        if let Some(idx) = next {
            match self.submenu.as_mut() {
                Some(s) => { s.menu.hovered = Some(idx); s.menu.scroll_into_view(idx); }
                None => { self.hovered = Some(idx); self.scroll_into_view(idx); }
            }
        }
    }

    /// 跳到首个 / 末个可用项（Home / End，XAML 菜单同款）。子菜单展开时作用于子菜单。
    pub fn hover_edge(&mut self, last: bool) {
        let idx = {
            let items = match self.submenu.as_ref() {
                Some(s) => s.menu.items.as_slice(),
                None => self.items.as_slice(),
            };
            if last {
                items.iter().rposition(|it| it.enabled)
            } else {
                items.iter().position(|it| it.enabled)
            }
        };
        if let Some(idx) = idx {
            match self.submenu.as_mut() {
                Some(s) => { s.menu.hovered = Some(idx); s.menu.scroll_into_view(idx); }
                None => { self.hovered = Some(idx); self.scroll_into_view(idx); }
            }
        }
    }

    /// 当前键盘 / 悬停项路径（子菜单展开时为子菜单内路径）。
    pub fn hovered_path(&self) -> Option<MenuPath> {
        match self.submenu.as_ref() {
            Some(s) => s.menu.hovered.map(|i| MenuPath {
                parent: Some(s.parent),
                index: i,
            }),
            None => self.hovered.map(|i| MenuPath {
                parent: None,
                index: i,
            }),
        }
    }

    /// 元素树语义的「激活一项」（点选 / Enter）：嵌套父项只展开子菜单不算选中；
    /// 其余标记勾选（单选组互斥）并关闭弹层（本节点即弹层根）。`owner` 在场时发
    /// `MenuInvoked { owner, path, .. }`。
    pub fn activate_at(&mut self, ctx: &mut kanesumi_element::EventCtx, path: MenuPath) {
        // 禁用项：点了 / 回车都没有动作，菜单也不关（关闭等于假装执行成功）。
        if self.item(path).is_some_and(|it| !it.enabled) {
            ctx.set_handled();
            return;
        }
        if path.parent.is_none() && self.items.get(path.index).is_some_and(|it| it.is_submenu()) {
            if let Some(engine) = ctx.engine().cloned() {
                self.open_submenu(&engine, ctx.surface(), path.index);
            }
            ctx.invalidate_paint();
            return;
        }
        match path.parent {
            None => {
                self.select(path.index);
            }
            Some(_) => {
                self.select_submenu(path);
            }
        }
        let (label, checked) = self
            .item(path)
            .map(|it| (it.label.clone(), it.checked))
            .unwrap_or_default();
        if let Some(owner) = self.owner {
            ctx.emit(crate::menu_flyout::MenuInvoked {
                owner,
                path,
                label,
                checked,
            });
        }
        ctx.close_popup(ctx.id());
    }

    /// `item(path)` 的私有读取（顶层 / 子菜单统一）。
    fn item(&self, path: MenuPath) -> Option<&MenuItem> {
        match path.parent {
            None => self.items.get(path.index),
            Some(p) => self.items.get(p)?.submenu.get(path.index),
        }
    }
}

/// 命中结果 —— `parent=None` = 顶层项；`parent=Some(p)` = 顶层 p 的子菜单第 `index` 项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPath {
    pub parent: Option<usize>,
    pub index: usize,
}

/// 悬停语义签名（S1 输入门控）：`(顶层 hovered, 子菜单 (父项索引, 子菜单 hovered))`。
pub type MenuInteractionSignature = Option<(Option<usize>, Option<(usize, Option<usize>)>)>;

/// 菜单项文本样式：14px 正文（走令牌，参 §G-67）。
fn menu_item_style() -> TextStyle {
    kanesumi_core::MetroTypography::metro().body_medium
}

/// 菜单图标样式：16px。图标字形保持 Normal —— 符号笔画本就细，加粗会糊。
fn icon_style() -> TextStyle {
    TextStyle::new(16.0, 16.0, kanesumi_core::FontWeight::Normal)
}

// ── 元素树弹层（E4 菜单族入树，参 docs/ELEMENT_MIGRATION.md §8）────────────────
//
// 面板本体就是弹层节点：挂在覆盖层，框架按 `PopupSpec` 放置（`at` 点锚定 = 右键
// 菜单的四象限翻折，`anchor` 下缘锚定 = 下拉菜单），自身矩形 = 面板。级联子菜单画在
// 面板之外（`paint_overflow` 声明、`hit_test` 一并命中）；选中发 `MenuInvoked`（owner
// = 打开它的控件）并关闭自身；任何原因的关闭都由框架交还焦点（`close_popup` 契约）。
impl Widget for MetroDropdownMenu {
    fn measure(&mut self, ctx: &mut MeasureCtx, _available: Size) -> Size {
        self.panel_size(ctx.engine())
    }

    fn arrange(&mut self, _ctx: &mut ArrangeCtx, rect: Rect) {
        self.panel_rect = rect;
        // 面板变矮 / 项变少后旧滚动量可能越界，就地夹回。
        self.scroll = self.scroll.clamp(0.0, self.max_scroll());
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        self.render_panel(&theme, ctx.engine(), scene);
        self.render_submenu(&theme, ctx.engine(), scene);
        if self.is_animating() {
            ctx.request_anim_frame();
        }
    }

    fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
        self.update(dt);
        ctx.invalidate_paint();
        if self.is_animating() {
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            // 按下滚动指示条 → 开始拖动（命中区比 3pt 的可见条宽，见 `scroll_hint_hit_rect`）。
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                self.last_pointer = Some(*pos);
                if self
                    .scroll_hint_hit_rect()
                    .is_some_and(|r| r.contains(*pos))
                {
                    self.hint_drag = true;
                    if self.drag_hint_to(*pos) {
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                }
            }
            Event::PointerMove { pos } => {
                self.last_pointer = Some(*pos);
                // 拖动中：只滚内容，不改悬停项（否则一路扫过去会把悬停高亮带跑）。
                if self.hint_drag {
                    if self.drag_hint_to(*pos) {
                        ctx.invalidate_paint();
                    }
                    ctx.set_handled();
                    return;
                }
                if let Some(engine) = ctx.engine().cloned()
                    && self.hover(&engine, ctx.surface(), *pos)
                {
                    ctx.invalidate_paint();
                }
            }
            // 滚轮 / 触控板：面板封顶后靠它看后面的项。指针在面板内或菜单有焦点时才吃。
            Event::Scroll { dy, dx, .. } if *dx == 0.0 => {
                // 滚轮事件不带坐标：用最近指针位置判断「指针在面板上」；菜单有焦点时也吃。
                let over_panel = self
                    .last_pointer
                    .is_some_and(|p| self.panel_rect.contains(p));
                if (over_panel || ctx.state().focused) && self.scroll_by(*dy) {
                    ctx.invalidate_paint();
                    ctx.set_handled();
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                // 拖着滚动条松手：只结束拖动，不激活下面的项。
                if self.hint_drag {
                    self.hint_drag = false;
                    ctx.set_handled();
                    return;
                }
                if let Some(path) = self.try_path_at(*pos) {
                    self.activate_at(ctx, path);
                }
                ctx.set_handled();
            }
            Event::KeyDown { key, .. } => {
                match key {
                    Key::Down => self.step_hover(1),
                    Key::Up => self.step_hover(-1),
                    // Home / End：跳到首 / 末个可用项（XAML 菜单同款；只有 ↑↓ 时，
                    // 长菜单要按十几次才能到头）。
                    Key::Home => self.hover_edge(false),
                    Key::End => self.hover_edge(true),
                    Key::Right => {
                        if let (Some(i), Some(engine)) = (self.hovered, ctx.engine().cloned())
                            && self.items.get(i).is_some_and(|it| it.is_submenu() && it.enabled)
                        {
                            self.open_submenu(&engine, ctx.surface(), i);
                            if let Some(s) = self.submenu.as_mut() {
                                s.menu.hovered = s.menu.items.iter().position(|it| it.enabled);
                            }
                        }
                    }
                    Key::Left => self.close_submenu(),
                    Key::Enter | Key::Char(' ') => {
                        if let Some(path) = self.hovered_path() {
                            self.activate_at(ctx, path);
                        }
                    }
                    // Esc / Tab 留给框架（关闭弹层 / 焦点遍历）。
                    _ => return,
                }
                ctx.invalidate_paint();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    /// 菜单以悬停高亮表示当前项，不要框架焦点框。
    fn focus_visual(&self) -> bool {
        false
    }

    /// 无障碍：角色 `Menu`；名字用类型名级落点（[`MENU_ACCESS_NAME`]）；
    /// 当前项（悬停 / 键盘预选）放 `value` —— 条目在快照里是**合成项**（见 `access_items`），
    /// 播报「停在哪个命令」还要靠这一档。
    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::Menu,
            name: MENU_ACCESS_NAME.to_string(),
            value: self
                .hovered
                .and_then(|i| self.items.get(i))
                .map(|it| it.label.clone()),
            checked: None,
        })
    }

    /// 无障碍合成项：把每一行菜单命令报成 AT 子元素（UIA 的「项 Peer」）。
    ///
    /// 菜单把几十行画在**一个**框架节点里，读屏要能逐条枚举命令、逐条读名字与勾选态，
    /// 只能由控件自己上报。级联子菜单的条目挂在父项之下（`children`），不摊平 ——
    /// 摊平会让读屏以为「名称」是一级命令。
    ///
    /// **只报视口内的项**：菜单封顶后靠滚动看后面的项，滚动出面板的项不能再报（契约 5
    /// 会抓住越界的合成项，而且读屏也不该读到屏幕上没有的命令）。
    fn access_items(&self, rect: Rect, _theme: &kanesumi_core::MetroTheme) -> Vec<kanesumi_element::AccessItem> {
        let _ = rect;
        self.items
            .iter()
            .enumerate()
            .filter(|(i, _)| {
                let r = self.item_rect(*i);
                r.bottom() > self.panel_rect.origin.y && r.origin.y < self.panel_rect.bottom()
            })
            .map(|(i, item)| {
                // 被面板边缘裁到一半的项：bounds 取与面板的交集（读屏读的是看得见的那段），
                // 否则契约 5「合成项须在宿主范围内」会当场报违例。
                let visible = self
                    .item_rect(i)
                    .intersect(self.panel_rect)
                    .unwrap_or(self.panel_rect);
                let mut node =
                    kanesumi_element::AccessItem::new(AccessRole::MenuItem, item.label.clone(), visible);
                // 快捷键读出来（UIA 的 AcceleratorKey 语义）：只有键名，不注册。
                node.value = item.shortcut.clone();
                node.checked = (item.checked || item.radio_group.is_some()).then_some(item.checked);
                node.disabled = !item.enabled;
                // 子菜单已展开时，把它的条目挂在父项之下。
                if let Some(sub) = self.submenu.as_ref().filter(|s| s.parent == i) {
                    node.children = sub
                        .menu
                        .items
                        .iter()
                        .enumerate()
                        .map(|(k, child)| {
                            let r = Rect::new(
                                sub.panel.origin.x,
                                sub.panel.origin.y + k as f32 * self.item_height,
                                sub.panel.size.width,
                                self.item_height,
                            );
                            let mut c = kanesumi_element::AccessItem::new(
                                AccessRole::MenuItem,
                                child.label.clone(),
                                r,
                            );
                            c.value = child.shortcut.clone();
                            c.checked = (child.checked || child.radio_group.is_some())
                                .then_some(child.checked);
                            c.disabled = !child.enabled;
                            c
                        })
                        .collect();
                }
                node
            })
            .collect()
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
            || self
                .submenu_state()
                .is_some_and(|s| s.panel.contains(pos))
    }

    /// 级联子菜单画在面板之外：声明外扩，框架据此计入损伤。
    fn paint_overflow(&self) -> Insets {
        let Some(s) = self.submenu_state() else {
            return Insets::ZERO;
        };
        let (p, r) = (s.panel, self.panel_rect);
        Insets::new(
            (r.origin.x - p.origin.x).max(0.0),
            (r.origin.y - p.origin.y).max(0.0),
            (p.right() - r.right()).max(0.0),
            (p.bottom() - r.bottom()).max(0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SCREEN: Rect = Rect::new(0.0, 0.0, 1024.0, 768.0);

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

    #[test]
    fn toggle_open_close() {
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("Reset")]);
        menu.toggle(Rect::new(0.0, 0.0, 120.0, 32.0));
        assert!(menu.anim.is_visible());
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert!(menu.anim.is_open());
        menu.toggle(Rect::new(0.0, 0.0, 120.0, 32.0));
        for _ in 0..120 {
            menu.update(1.0 / 60.0);
        }
        assert_eq!(menu.state(), PopupState::Closed);
    }

    #[test]
    fn item_at_maps_y() {
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("A"), MenuItem::new("B")]);
        menu.open(Rect::new(10.0, 10.0, 120.0, 32.0));
        assert_eq!(menu.item_at(Point::new(20.0, 12.0)), Some(0));
        assert_eq!(menu.item_at(Point::new(20.0, 12.0 + 32.0)), Some(1));
        assert_eq!(menu.item_at(Point::new(5.0, 12.0)), None, "面板外");
    }

    #[test]
    fn panel_size_grows_with_items() {
        let Some(engine) = find_engine() else { return };
        let one = MetroDropdownMenu::new(vec![MenuItem::new("A")]);
        let two = MetroDropdownMenu::new(vec![MenuItem::new("A"), MenuItem::new("B")]);
        assert!(
            two.panel_size(&engine).height > one.panel_size(&engine).height,
            "项越多越高"
        );
    }

    /// V21：panel_size 结果缓存到 Cell，重复调用命中缓存；invalidate 后重算。
    #[test]
    fn panel_size_is_cached_until_invalidate() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("A")]);
        let a = menu.panel_size(&engine);
        // 直接修改 items，不 invalidate → 缓存命中，返回旧尺寸。
        menu.items.push(MenuItem::new("BB longer"));
        let b_cached = menu.panel_size(&engine);
        assert_eq!(a, b_cached, "未 invalidate 时应命中缓存返回旧值");
        // invalidate 后重算 → 新尺寸更高（多一项）。
        menu.invalidate_layout();
        let b_fresh = menu.panel_size(&engine);
        assert!(
            b_fresh.height > a.height,
            "invalidate 后应重算，高度增加，实际 {} vs {}",
            b_fresh.height,
            a.height
        );
    }

    #[test]
    fn renders_panel_when_open() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("Reset")]);
        menu.open(Rect::new(10.0, 10.0, 120.0, 32.0));
        menu.update(1.0);
        let mut scene = Scene::default();
        menu.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 800.0, 600.0),
            &mut scene,
        );
        // 遮罩 + 面板底 + 项文本（Kanesumi 铁律：无边框，故无 border 命令）。
        assert!(scene.commands.len() >= 3);
    }

    /// 回归 V1：文本 rect 宽度不可为负，且必须完全落在面板内。
    /// 曾经 bug：`panel_rect.size.width - x`（x 绝对坐标）→ 巨大负值 →
    /// `TextEngine::layout(max_width=负)` 触发 CJK 单字硬断 → 菜单碎字塔。
    #[test]
    fn item_text_rect_fits_panel() {
        use kanesumi_canvas::SceneCommand;
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let panel_at = Rect::new(600.0, 300.0, 140.0, 96.0);
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::with_icon("新建", "★"),
            MenuItem::with_icon("打开", "★"),
        ]);
        menu.open(panel_at);
        menu.update(1.0);
        let mut scene = Scene::default();
        menu.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 1024.0, 768.0),
            &mut scene,
        );
        // 每条 Text 命令的 rect 必须宽度 > 0 且 (x + width) <= panel_rect.right()。
        let text_rects: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { rect, .. } => Some(*rect),
                _ => None,
            })
            .collect();
        assert!(!text_rects.is_empty(), "菜单项应产出 Text 命令");
        for r in &text_rects {
            assert!(
                r.size.width > 0.0,
                "text_rect 宽必须正，实际 {}",
                r.size.width
            );
            assert!(
                r.right() <= panel_at.right() + 0.01,
                "text_rect 右缘越出面板：right={} panel.right={}",
                r.right(),
                panel_at.right()
            );
        }
    }

    // ── 二级级联（§39 MenuFlyoutSubItem） ─────────────────────────────

    fn cascaded_menu() -> MetroDropdownMenu {
        MetroDropdownMenu::new(vec![
            MenuItem::new("视图"),
            MenuItem::new("缩放").with_submenu(vec![
                MenuItem::new("放大").radio("zoom"),
                MenuItem::new("缩小").radio("zoom"),
                MenuItem::new("重置").radio("zoom"),
            ]),
            MenuItem::new("帮助"),
        ])
    }

    #[test]
    fn hover_over_submenu_opens_it() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        menu.open(Rect::new(0.0, 0.0, 120.0, 96.0));
        // 悬停"缩放"（index 1）→ 展开子菜单
        let r1 = menu.item_rect(1);
        menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        assert_eq!(menu.hovered, Some(1));
        let sub = menu.submenu_state().expect("悬停嵌套项应展开子菜单");
        assert_eq!(sub.parent, 1);
        assert_eq!(sub.menu.items.len(), 3);
        // 子菜单面板在父项右侧
        assert!(sub.panel.origin.x >= r1.right() + 1.0, "子菜单位于父项右侧");
        assert!((sub.panel.origin.y - r1.origin.y).abs() < 1.0, "垂直对齐");
    }

    #[test]
    fn hover_other_item_swaps_submenu() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        menu.open(Rect::new(0.0, 0.0, 120.0, 96.0));
        let r1 = menu.item_rect(1);
        menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        assert!(menu.submenu.is_some());
        // 悬停普通项（index 0）→ 收起子菜单
        let r0 = menu.item_rect(0);
        menu.hover(&engine, TEST_SCREEN, Point::new(r0.origin.x + 10.0, r0.origin.y + 10.0));
        assert!(menu.submenu.is_none(), "悬停普通项应收起子菜单");
        assert_eq!(menu.hovered, Some(0));
    }

    #[test]
    fn hover_into_submenu_keeps_it_open() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        menu.open(Rect::new(0.0, 0.0, 120.0, 96.0));
        let r1 = menu.item_rect(1);
        menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        let sub = menu.submenu_state().unwrap().panel;
        // 指针移入子菜单内部 → 保持展开，且悬停高亮转移到子菜单项。
        let inside = Point::new(sub.origin.x + 10.0, sub.origin.y + 10.0);
        menu.hover(&engine, TEST_SCREEN, inside);
        assert!(menu.submenu.is_some(), "指针进入子菜单应收起");
        assert_eq!(menu.submenu_hovered(), Some(0), "子菜单首项应高亮");
        // 指针停在父项↔子菜单之间的桥接带（父项右缘+1，父项 y 中）→ 保持展开。
        let bridge = Point::new(r1.right() + 1.0, r1.origin.y + 10.0);
        menu.hover(&engine, TEST_SCREEN, bridge);
        assert!(menu.submenu.is_some(), "桥接带内应收起子菜单");
        // 离开面板（远处空白）→ 收起。
        menu.hover(&engine, TEST_SCREEN, Point::new(800.0, 700.0));
        assert!(menu.submenu.is_none(), "远处空白应收起");
    }

    #[test]
    fn path_at_hits_submenu_items() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        menu.open(Rect::new(0.0, 0.0, 120.0, 96.0));
        let r1 = menu.item_rect(1);
        menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        let sub = menu.submenu_state().unwrap();
        // 点子菜单第一项（index 0）→ 返回 (parent=1, index=0)
        let child_center = Point::new(
            sub.panel.origin.x + 10.0,
            sub.panel.origin.y + 10.0,
        );
        assert_eq!(
            menu.try_path_at(child_center),
            Some(MenuPath { parent: Some(1), index: 0 })
        );
        // 顶层项 → parent=None
        let r0 = menu.item_rect(0);
        assert_eq!(
            menu.try_path_at(Point::new(r0.origin.x + 10.0, r0.origin.y + 10.0)),
            Some(MenuPath { parent: None, index: 0 })
        );
    }

    #[test]
    fn radio_group_mutually_exclusive() {
        let mut menu = cascaded_menu();
        menu.open(Rect::new(0.0, 0.0, 120.0, 96.0));
        // 直接展开子菜单（hover 已测），操作顶层 select 前先测子菜单互斥
        let r1 = menu.item_rect(1);
        // 用 hover 打开子菜单后 select_submenu
        // （这里 engine 需要；open_submenu 内部用 panel_size 需要 engine）
        if let Some(engine) = find_engine() {
            menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
            assert!(menu.select_submenu(MenuPath { parent: Some(1), index: 0 }));
            assert!(menu.submenu_state().unwrap().menu.items[0].checked);
            assert!(menu.select_submenu(MenuPath { parent: Some(1), index: 2 }));
            // 0 被取消，2 勾选
            assert!(!menu.submenu_state().unwrap().menu.items[0].checked);
            assert!(menu.submenu_state().unwrap().menu.items[2].checked);
            // 再点已选中项 → 无变化
            assert!(!menu.select_submenu(MenuPath { parent: Some(1), index: 2 }));
        }
    }

    /// 子菜单右缘越屏 → `open_submenu` 应翻到父项左侧（参 §39 MenuFlyoutSubItem）。
    #[test]
    fn submenu_flips_left_when_right_overflows() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        // 面板贴屏右缘：panel_rect origin.x = 900, width=120 → 父项 right ≈ 1020 + 子菜单 → 越屏 1024
        menu.open(Rect::new(900.0, 0.0, 120.0, 96.0));
        let r1 = menu.item_rect(1);
        menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        let sub = menu.submenu_state().expect("应展开子菜单");
        assert!(
            sub.panel.origin.x < r1.origin.x,
            "子菜单应翻到父项左侧：sub.x={} vs parent.x={}",
            sub.panel.origin.x,
            r1.origin.x
        );
        assert!(
            sub.panel.right() <= TEST_SCREEN.right() + 0.01,
            "翻左后子菜单不越屏"
        );
    }

    /// 子菜单下缘越屏 → `open_submenu` 应上移到屏内。
    #[test]
    fn submenu_shifts_up_when_bottom_overflows() {
        let Some(engine) = find_engine() else { return };
        let mut menu = cascaded_menu();
        // 屏 200 高；父面板贴屏底
        let tiny_screen = Rect::new(0.0, 0.0, 800.0, 200.0);
        menu.open(Rect::new(0.0, 140.0, 120.0, 96.0));
        let r1 = menu.item_rect(1);
        menu.hover(&engine, tiny_screen, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        let sub = menu.submenu_state().expect("应展开子菜单");
        assert!(
            sub.panel.bottom() <= tiny_screen.bottom() + 0.01,
            "下缘上移到屏内：bottom={} screen.bottom={}",
            sub.panel.bottom(),
            tiny_screen.bottom()
        );
    }

    #[test]
    fn radio_group_at_top_level() {
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("小").radio("size"),
            MenuItem::new("中").radio("size"),
            MenuItem::new("大").radio("size"),
        ]);
        assert!(menu.select(0));
        assert!(menu.items[0].checked);
        assert!(menu.select(2));
        assert!(!menu.items[0].checked, "同组互斥取消");
        assert!(menu.items[2].checked);
        assert!(!menu.select(2), "重选已勾选项无变化");
    }

    #[test]
    fn item_at_and_rect_align_with_separators() {
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("打开"),
            MenuItem::new("复制"),
            MenuItem::new("压缩为"),
            MenuItem::new("移至回收站").separator(),
            MenuItem::new("永久删除").separator(),
            MenuItem::new("属性"),
        ]);
        menu.open(Rect::new(0.0, 0.0, 120.0, 0.0));
        let h = menu.item_height;
        // 布局：每项 h；分隔线画在项**之后** +2（与 render_panel 一致）。
        // 0 打开 [0,h)                      → y=0
        // 1 复制 [h,2h)                     → y=h
        // 2 压缩为 [2h,3h)                  → y=2h
        // 3 移至回收站(sep) [3h,4h)          → y=3h；线在 4h
        // 4 永久删除(sep) [4h+2,5h+2)        → y=4h+2；线在 5h+2
        // 5 属性 [5h+4,6h+4)                → y=5h+4
        let cases = [
            (0, 0.0),
            (1, h),
            (2, 2.0 * h),
            (3, 3.0 * h),
            (4, 4.0 * h + 2.0),
            (5, 5.0 * h + 4.0),
        ];
        for (idx, y) in cases {
            let hit = menu.item_at(Point::new(10.0, y + h * 0.5));
            assert_eq!(hit, Some(idx), "y={y} 应命中第 {idx} 项");
            let r = menu.item_rect(idx);
            assert!((r.origin.y - y).abs() < 1e-3, "第 {idx} 项 rect y={} ≠ 预期 {y}", r.origin.y);
        }
        // 分隔线本身不可命中（第 3 项后，y = 4h）。
        assert_eq!(menu.item_at(Point::new(10.0, 4.0 * h)), None, "分隔线不可命中");
    }

    /// 留白式分组间距：只占高、不画线（参 docs/DECISIONS_2026-10-05 §100）。
    #[test]
    fn gap_after_reserves_whitespace_and_draws_no_line() {
        use kanesumi_canvas::SceneCommand;
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("头部").gap(),
            MenuItem::new("打开"),
            MenuItem::new("退出"),
        ]);
        menu.open(Rect::new(0.0, 0.0, 160.0, 0.0));
        let h = menu.item_height;
        // 布局：项 0 [0,h)，间距 [h,h+GAP)，项 1 [h+GAP,2h+GAP)。
        assert!((menu.item_rect(1).origin.y - (h + MENU_GROUP_GAP)).abs() < 1e-3);
        assert_eq!(
            menu.item_at(Point::new(10.0, h + MENU_GROUP_GAP * 0.5)),
            None,
            "间距不可命中"
        );
        assert_eq!(
            menu.item_at(Point::new(10.0, h + MENU_GROUP_GAP + h * 0.5)),
            Some(1)
        );
        let Some(engine) = find_engine() else { return };
        let size = menu.panel_size(&engine);
        assert!(
            (size.height - (3.0 * h + MENU_GROUP_GAP)).abs() < 1e-3,
            "面板高应含间距，实际 {}",
            size.height
        );
        // 间距处不得出现 divider 色的分隔线。
        let theme = MetroTheme::ether_dark();
        let divider = theme.colors.divider;
        menu.update(1.0);
        let mut scene = Scene::default();
        menu.render(&theme, &engine, Rect::new(0.0, 0.0, 800.0, 600.0), &mut scene);
        let sep_at_gap = scene.commands.iter().any(|c| {
            matches!(
                c,
                SceneCommand::FillRect { color, rect, .. }
                    if *color == divider && (rect.origin.y - h).abs() < 1.5
            )
        });
        assert!(!sep_at_gap, "留白式间距处不得画分隔线");
    }

    /// 相邻项的悬停底必须共用边缘，且边缘落在物理像素网格上：
    /// 否则 2x 下两块高亮之间会露出 1 px 缝（真机已知问题 #18）。
    #[test]
    fn adjacent_item_rects_share_pixel_grid_edges() {
        kanesumi_canvas::set_surface_scale(2.0);
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("打开"),
            MenuItem::new("另存为"),
            MenuItem::new("退出"),
        ]);
        // 故意用带小数的面板原点：这正是 2x 下露缝的成因。
        menu.panel_rect = Rect::new(30.25, 100.4, 180.0, 96.0);
        let rects: Vec<_> = (0..3).map(|i| menu.item_rect(i)).collect();
        for pair in rects.windows(2) {
            assert!(
                (pair[0].bottom() - pair[1].origin.y).abs() < 1e-4,
                "相邻项必须首尾相接：{:?} / {:?}",
                pair[0],
                pair[1]
            );
        }
        for r in &rects {
            let y2 = r.origin.y * 2.0;
            let bottom2 = r.bottom() * 2.0;
            assert!(
                (y2 - y2.round()).abs() < 1e-4 && (bottom2 - bottom2.round()).abs() < 1e-4,
                "项边缘必须落在物理像素上：{:?}",
                r
            );
        }
        kanesumi_canvas::set_surface_scale(1.0);
    }

    /// 无障碍（E6）：菜单面板报告 `Menu` 角色、类型名级名字与当前项；打开态下契约成立。
    #[test]
    fn accessibility_reports_menu_role_and_current_item() {
        use kanesumi_element::testing::TestHarness;
        use kanesumi_element::{AccessRole as Role, PopupSpec};

        let mut h = TestHarness::new(420.0, 300.0);
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("新建"), MenuItem::new("打开")]);
        menu.anim.open();
        menu.hovered = Some(1);
        let id = h.tree.open_popup(menu, PopupSpec::default());
        h.frame();

        let snap = h.access_snapshot();
        let node = snap.find(id).expect("菜单弹层在快照内");
        assert_eq!(node.role, Some(Role::Menu));
        assert_eq!(node.name.as_deref(), Some(MENU_ACCESS_NAME));
        assert_eq!(node.value.as_deref(), Some("打开"), "当前项放 value");
        assert!(node.focusable, "菜单接收键盘，可聚焦");
        h.assert_access_contract();
    }

    /// 无障碍合成项：菜单命令逐条成为 AT 子元素（UIA 项 Peer 语义）。
    #[test]
    fn access_snapshot_exposes_menu_items() {
        use kanesumi_element::testing::TestHarness;
        use kanesumi_element::PopupSpec;

        let mut h = TestHarness::new(420.0, 320.0);
        let mut menu = menu_with_disabled();
        menu.items[2].submenu[0].checked = true;
        menu.items[0].shortcut = Some("Ctrl+C".into());
        menu.anim.open();
        let id = h.tree.open_popup(menu, PopupSpec::default());
        h.frame();

        let snap = h.access_snapshot();
        let node = snap.find(id).expect("菜单在快照内");
        assert_eq!(node.items.len(), 5, "五行命令各是一个合成项：{:?}", node.items.iter().map(|i| &i.name).collect::<Vec<_>>());
        let copy = snap.find_item("复制").expect("复制项");
        assert_eq!(copy.role, kanesumi_element::AccessRole::MenuItem);
        assert_eq!(copy.value.as_deref(), Some("Ctrl+C"), "快捷键读出来");
        assert!(!copy.disabled);
        let paste = snap.find_item("粘贴").expect("粘贴项");
        assert!(paste.disabled, "禁用项要报给 AT");
        let sort = snap.find_item("排序").expect("排序项");
        assert_eq!(sort.checked, None, "非勾选项不报勾选态");
        assert!(paste.bounds.size.height > 0.0, "合成项有几何：{:?}", paste.bounds);
        h.assert_access_contract();
    }

    /// 级联子菜单的条目挂在父项之下，不摊平成一级命令。
    #[test]
    fn access_snapshot_nests_submenu_items_under_parent() {
        use kanesumi_element::testing::TestHarness;
        use kanesumi_element::PopupSpec;

        let mut h = TestHarness::new(420.0, 320.0);
        let mut menu = menu_with_disabled();
        menu.items[2].submenu[0].checked = true;
        menu.anim.open();
        let id = h.tree.open_popup(menu, PopupSpec::default());
        h.frame();

        // 悬停「排序」展开子菜单。
        let row = menu_item_rect(&h, id, 2);
        h.move_to(Point::new(row.origin.x + 20.0, row.origin.y + 10.0));
        h.frame();

        let snap = h.access_snapshot();
        let node = snap.find(id).expect("菜单在快照内");
        let sort = node
            .items
            .iter()
            .find(|i| i.name == "排序")
            .expect("排序项");
        assert_eq!(sort.children.len(), 2, "子菜单两项挂在其下");
        assert_eq!(sort.children[0].name, "名称");
        assert_eq!(sort.children[0].checked, Some(true), "单选勾选态跟着走");
        assert!(
            !node.items.iter().any(|i| i.name == "名称"),
            "子菜单项不摊平到一级"
        );
        h.assert_access_contract();
    }

    /// 取弹层内第 `i` 行的屏幕矩形（用于把指针移到该行）。
    fn menu_item_rect(
        h: &kanesumi_element::testing::TestHarness,
        id: kanesumi_element::WidgetId,
        i: usize,
    ) -> Rect {
        let r = h.rect(id);
        Rect::new(r.origin.x, r.origin.y + i as f32 * 32.0, r.size.width, 32.0)
    }
    // ── 滚动指示条拖动 + 子菜单间距可配（2026-11-06 收尾）──────────────────────

    #[test]
    fn drag_hint_maps_pointer_to_scroll_and_clamps() {
        let Some(engine) = find_engine() else { return };
        let mut menu = long_menu(40);
        let size = menu.panel_size(&engine);
        menu.open(Rect::new(0.0, 0.0, size.width, size.height));
        let max = menu.max_scroll();
        assert!(max > 0.0);

        // 拖到面板底部 → 滚到底。
        assert!(menu.drag_hint_to(Point::new(10.0, size.height)));
        assert!((menu.scroll - max).abs() < 0.5, "拖到底 = 滚到底：{} vs {max}", menu.scroll);

        // 拖到顶部以上 → 夹回 0。
        menu.drag_hint_to(Point::new(10.0, -50.0));
        assert_eq!(menu.scroll, 0.0, "拖出上缘夹到 0");

        // 拖到中部 → 落在 (0, max) 之间。
        menu.drag_hint_to(Point::new(10.0, size.height / 2.0));
        assert!(menu.scroll > 0.0 && menu.scroll < max, "中部：{}", menu.scroll);
    }

    #[test]
    fn scroll_hint_hit_area_is_wider_than_the_visible_bar() {
        let Some(engine) = find_engine() else { return };
        let mut short = long_menu(3);
        let size = short.panel_size(&engine);
        short.open(Rect::new(0.0, 0.0, size.width, size.height));
        assert!(short.scroll_hint_hit_rect().is_none(), "装得下时没有可拖的条");

        let mut long = long_menu(40);
        let size = long.panel_size(&engine);
        long.open(Rect::new(0.0, 0.0, size.width, size.height));
        let bar = long.scroll_hint_rect().expect("有滚动条");
        let hit = long.scroll_hint_hit_rect().expect("有命中区");
        assert!(hit.contains(bar.center()), "命中区包含可见条");
        assert!(
            hit.size.width >= bar.size.width + 7.0,
            "命中区向左扩（3pt 的条点不准）：bar {:?} hit {:?}",
            bar,
            hit
        );
    }

    #[test]
    fn submenu_gap_is_configurable() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("视图"),
            MenuItem::new("排序").with_submenu(vec![MenuItem::new("名称")]),
        ]);
        menu.submenu_gap = 12.0;
        menu.open(Rect::new(0.0, 0.0, 120.0, 64.0));
        let r1 = menu.item_rect(1);
        menu.open_submenu(&engine, TEST_SCREEN, 1);
        let sub = menu.submenu_state().expect("子菜单已开");
        assert!(
            (sub.panel.origin.x - (r1.right() + 12.0)).abs() < 0.5,
            "间距按配置生效：panel {} vs 期望 {}",
            sub.panel.origin.x,
            r1.right() + 12.0
        );
    }
    // ── 长菜单滚动（面板封顶 MENU_MAX_HEIGHT，2026-11-06）────────────────────────

    fn long_menu(n: usize) -> MetroDropdownMenu {
        MetroDropdownMenu::new((0..n).map(|i| MenuItem::new(format!("命令 {i}"))).collect())
    }

    #[test]
    fn long_menu_panel_height_is_capped() {
        let Some(engine) = find_engine() else { return };
        let menu = long_menu(40);
        let size = menu.panel_size(&engine);
        assert_eq!(size.height, MENU_MAX_HEIGHT, "面板封顶在最大高度");
        assert!(
            menu.content_height() > MENU_MAX_HEIGHT + 100.0,
            "内容本身远高于上限：{}",
            menu.content_height()
        );
    }

    #[test]
    fn short_menu_is_not_capped_and_cannot_scroll() {
        let Some(engine) = find_engine() else { return };
        let mut menu = long_menu(3);
        let size = menu.panel_size(&engine);
        assert!(size.height < MENU_MAX_HEIGHT, "三项菜单不需要封顶");
        menu.open(Rect::new(0.0, 0.0, size.width, size.height));
        assert_eq!(menu.max_scroll(), 0.0);
        assert!(!menu.scroll_by(50.0), "装得下时滚动无效");
    }

    #[test]
    fn scroll_shifts_item_geometry_and_hit_testing() {
        let Some(engine) = find_engine() else { return };
        let mut menu = long_menu(40);
        let size = menu.panel_size(&engine);
        menu.open(Rect::new(0.0, 0.0, size.width, size.height));

        let before = menu.item_rect(2);
        assert!(menu.scroll_by(64.0), "有内容可滚");
        let after = menu.item_rect(2);
        assert!(
            (before.origin.y - after.origin.y - 64.0).abs() < 0.5,
            "滚动 64pt 后第 3 项上移 64pt：{} → {}",
            before.origin.y,
            after.origin.y
        );
        // 命中跟着几何走：现在落在面板顶部的那一行应是滚动后可见的首项。
        let hit = menu.item_at(Point::new(after.origin.x + 10.0, after.origin.y + 10.0));
        assert_eq!(hit, Some(2), "命中与渲染同一套滚动偏移");
    }

    #[test]
    fn scroll_is_clamped_to_bounds() {
        let Some(engine) = find_engine() else { return };
        let mut menu = long_menu(40);
        let size = menu.panel_size(&engine);
        menu.open(Rect::new(0.0, 0.0, size.width, size.height));
        assert!(!menu.scroll_by(-100.0), "顶部再往上滚无效");
        assert_eq!(menu.scroll, 0.0);
        menu.scroll_by(10_000.0);
        assert_eq!(menu.scroll, menu.max_scroll(), "滚到底停在 max_scroll");
    }

    #[test]
    fn keyboard_end_scrolls_last_item_into_view() {
        let Some(engine) = find_engine() else { return };
        let mut menu = long_menu(40);
        let size = menu.panel_size(&engine);
        menu.open(Rect::new(0.0, 0.0, size.width, size.height));

        menu.hover_edge(true); // End
        assert_eq!(menu.hovered, Some(39));
        assert!(menu.scroll > 0.0, "长菜单里跳到末项必须滚动");
        let r = menu.item_rect(39);
        assert!(
            r.bottom() <= menu.panel_rect.bottom() + 0.5 && r.origin.y >= menu.panel_rect.origin.y - 0.5,
            "末项要落在面板内：{r:?} vs 面板 {:?}",
            menu.panel_rect
        );
    }

    #[test]
    fn scroll_hint_only_when_panel_overflows() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let hint_bars = |menu: &MetroDropdownMenu| {
            let mut scene = Scene::default();
            menu.render_panel(&theme, &engine, &mut scene);
            scene
                .commands
                .iter()
                .filter(|c| match c {
                    kanesumi_canvas::SceneCommand::FillRect { rect, .. } => {
                        // 贴右缘、宽 3pt 的细条
                        (menu.panel_rect.right() - rect.right()).abs() < 8.0
                            && (rect.size.width - 3.0).abs() < 0.5
                    }
                    _ => false,
                })
                .count()
        };

        let mut short = long_menu(3);
        let size = short.panel_size(&engine);
        short.open(Rect::new(0.0, 0.0, size.width, size.height));
        short.anim.open();
        assert_eq!(hint_bars(&short), 0, "装得下时没有滚动条");

        let mut long = long_menu(40);
        let size = long.panel_size(&engine);
        long.open(Rect::new(0.0, 0.0, size.width, size.height));
        long.anim.open();
        assert_eq!(hint_bars(&long), 1, "封顶后出现一条滚动指示");
    }

    #[test]
    fn access_items_report_visible_items_only() {
        use kanesumi_element::testing::TestHarness;
        use kanesumi_element::PopupSpec;

        let mut h = TestHarness::new(420.0, 700.0);
        let mut menu = long_menu(40);
        menu.anim.open();
        let id = h.tree.open_popup(menu, PopupSpec::default());
        h.frame();

        let snap = h.access_snapshot();
        let node = snap.find(id).expect("菜单在快照内");
        assert!(!node.items.is_empty(), "有可见项");
        assert!(
            node.items.len() < 40,
            "只报视口内的项（实得 {}）",
            node.items.len()
        );
        for item in &node.items {
            assert!(
                item.bounds.origin.y >= node.bounds.origin.y - 0.5
                    && item.bounds.bottom() <= node.bounds.bottom() + 0.5,
                "合成项在面板内：{:?} ⊄ {:?}",
                item.bounds,
                node.bounds
            );
        }
        h.assert_access_contract();
    }
    /// RM-1028-5 回归（k-framework-fix1 消费方 1/3）：走真实控件对象，稳态开后关闭，    /// 第一帧面板进度 ≥ 0.9 —— open/close 重建轨道致跳 0 的缺陷在此必须红。
    #[test]
    fn close_from_steady_open_keeps_first_frame() {
        let mut menu = MetroDropdownMenu::new(vec![MenuItem::new("一"), MenuItem::new("二")]);
        menu.open(Rect::new(0.0, 0.0, 120.0, 32.0));
        for _ in 0..60 {
            menu.update(1.0 / 60.0);
        }
        assert!(menu.anim.is_open());
        menu.close();
        menu.update(1.0 / 60.0);
        assert!(
            menu.anim.panel_progress() >= 0.9,
            "稳态关闭第一帧面板进度 {}，不得跳 0",
            menu.anim.panel_progress()
        );
    }

    // ── 禁用项与勾选列（菜单手感 / 能力，2026-11-06）────────────────────────────

    /// 含禁用项与级联的菜单：0 普通 / 1 禁用 / 2 带子菜单 / 3 禁用 / 4 普通。
    fn menu_with_disabled() -> MetroDropdownMenu {
        MetroDropdownMenu::new(vec![
            MenuItem::new("复制"),
            MenuItem::new("粘贴").disabled(),
            MenuItem::new("排序").with_submenu(vec![
                MenuItem::new("名称").radio("sort"),
                MenuItem::new("日期").radio("sort"),
            ]),
            MenuItem::new("全选").disabled(),
            MenuItem::new("属性"),
        ])
    }

    #[test]
    fn keyboard_navigation_skips_disabled_items_and_wraps() {
        let mut menu = menu_with_disabled();
        menu.open(Rect::new(0.0, 0.0, 120.0, 160.0));
        menu.hovered = None;
        menu.step_hover(1);
        assert_eq!(menu.hovered, Some(0), "首个可用项");
        menu.step_hover(1);
        assert_eq!(menu.hovered, Some(2), "跳过禁用项 1");
        menu.step_hover(1);
        assert_eq!(menu.hovered, Some(4), "跳过禁用项 3");
        menu.step_hover(1);
        assert_eq!(menu.hovered, Some(0), "到尾后循环回首项");
        menu.step_hover(-1);
        assert_eq!(menu.hovered, Some(4), "反向同样跳过禁用项");
    }

    #[test]
    fn home_and_end_jump_to_first_and_last_enabled() {
        let mut menu = menu_with_disabled();
        menu.open(Rect::new(0.0, 0.0, 120.0, 160.0));
        menu.hovered = Some(2);
        menu.hover_edge(true);
        assert_eq!(menu.hovered, Some(4), "End 到末个可用项");
        menu.hover_edge(false);
        assert_eq!(menu.hovered, Some(0), "Home 到首个可用项");
    }

    #[test]
    fn hovering_disabled_item_clears_highlight_and_keeps_submenu_closed() {
        let Some(engine) = find_engine() else { return };
        let mut menu = menu_with_disabled();
        menu.open(Rect::new(0.0, 0.0, 120.0, 160.0));
        let r0 = menu.item_rect(0);
        menu.hover(&engine, TEST_SCREEN, Point::new(r0.origin.x + 10.0, r0.origin.y + 10.0));
        assert_eq!(menu.hovered, Some(0));
        // 扫过禁用项：不留高亮（否则上一项亮着，像是选中了它）
        let r1 = menu.item_rect(1);
        let changed = menu.hover(&engine, TEST_SCREEN, Point::new(r1.origin.x + 10.0, r1.origin.y + 10.0));
        assert!(changed, "悬停语义变化要通知宿主重绘");
        assert_eq!(menu.hovered, None, "禁用项不亮，且清掉上一项的高亮");
        assert!(menu.submenu.is_none());
    }

    #[test]
    fn disabled_item_does_not_open_its_submenu_on_hover() {
        let Some(engine) = find_engine() else { return };
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("排序")
                .with_submenu(vec![MenuItem::new("名称")])
                .disabled(),
            MenuItem::new("帮助"),
        ]);
        menu.open(Rect::new(0.0, 0.0, 120.0, 64.0));
        let r0 = menu.item_rect(0);
        menu.hover(&engine, TEST_SCREEN, Point::new(r0.origin.x + 10.0, r0.origin.y + 10.0));
        assert!(menu.submenu.is_none(), "禁用项的子菜单不应因悬停展开");
    }

    #[test]
    fn check_gutter_only_reserved_when_items_are_checkable() {
        let Some(engine) = find_engine() else { return };
        // 标签取长一些，越过面板 120pt 的最小宽度地板 —— 否则两种面板都是 120，量不出差。
        let long = "按修改时间从新到旧排序";
        let plain = MetroDropdownMenu::new(vec![MenuItem::new(long), MenuItem::new("属性")]);
        assert_eq!(plain.check_gutter(), 0.0, "无可勾选项不占列");
        let with_radio = MetroDropdownMenu::new(vec![
            MenuItem::new(long).radio("sort"),
            MenuItem::new("按名称排序").radio("sort"),
        ]);
        assert_eq!(with_radio.check_gutter(), MENU_CHECK_GUTTER);
        // 面板宽度要把它算进去，否则勾画在文字上。
        let plain_w = plain.panel_size(&engine).width;
        let radio_w = with_radio.panel_size(&engine).width;
        assert!(
            radio_w - plain_w >= MENU_CHECK_GUTTER - 0.5,
            "勾选列计入面板宽：{radio_w} vs {plain_w}"
        );
    }

    #[test]
    fn checked_item_draws_checkmark_and_labels_share_start_x() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("名称").radio("sort"),
            MenuItem::new("日期").radio("sort"),
        ]);
        menu.items[0].checked = true;
        menu.open(Rect::new(0.0, 0.0, 160.0, 64.0));
        menu.anim.open();
        let mut scene = Scene::default();
        menu.render_panel(&theme, &engine, &mut scene);

        let triangles = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Triangle { .. }))
            .count();
        assert!(triangles >= 4, "勾选标记 = 4 个三角（实得 {triangles}）");

        // 两个标签的左缘必须一致：勾选不改变文字起点。
        let label_x: Vec<f32> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                kanesumi_canvas::SceneCommand::Text { rect, content, .. } if content == "名称" || content == "日期" => {
                    Some(rect.origin.x)
                }
                _ => None,
            })
            .collect();
        assert_eq!(label_x.len(), 2, "两项文字都画了");
        assert_eq!(label_x[0], label_x[1], "勾选项与未勾选项文字起点相同");
    }

    #[test]
    fn disabled_items_render_dimmed() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut menu = MetroDropdownMenu::new(vec![
            MenuItem::new("复制"),
            MenuItem::new("粘贴").disabled(),
        ]);
        menu.open(Rect::new(0.0, 0.0, 160.0, 64.0));
        menu.anim.open();
        let mut scene = Scene::default();
        menu.render_panel(&theme, &engine, &mut scene);

        let alpha_of = |label: &str| {
            scene
                .commands
                .iter()
                .find_map(|c| match c {
                    kanesumi_canvas::SceneCommand::Text { content, color, .. } if content == label => {
                        Some(color.a)
                    }
                    _ => None,
                })
                .unwrap_or(1.0)
        };
        assert_eq!(alpha_of("复制"), 1.0, "可用项不降透明度");
        let dim = alpha_of("粘贴");
        assert!(
            dim < 1.0 && dim > 0.0,
            "禁用项按 disabled_opacity 降透明度（实得 {dim}）"
        );
    }
}
