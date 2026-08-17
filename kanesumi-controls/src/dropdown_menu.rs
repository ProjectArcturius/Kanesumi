use std::cell::Cell;

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect, TextStyle};

use crate::popup::{PopupAnim, PopupState, place_submenu, popup_gap, render_overlay};

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
    /// 嵌套子菜单（MenuFlyoutSubItem 语义，参 CONTROL_SPEC §39）。
    pub submenu: Vec<MenuItem>,
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
            submenu: Vec::new(),
        }
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
        }
    }

    /// 清空 `panel_size` 缓存 —— 修改 `items` / `item_height` / 字体大小后必须调。
    pub fn invalidate_layout(&self) {
        self.panel_size_cache.set(None);
    }

    pub fn open(&mut self, at: Rect) {
        self.panel_rect = at;
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

    /// 面板尺寸：宽 = 最宽项（含图标/快捷键占位）+ 边距，高 = 项数 × 项高 + 分隔线。
    ///
    /// **V21 缓存**：结果按 `items` / `item_height` 稳定，首次计算后走 `panel_size_cache`。
    /// 外部修改字段后需 [`Self::invalidate_layout`]（Cell 无法拦截 pub 字段写入）。
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
        let separators = self.items.iter().filter(|i| i.separator_after).count() as f32;
        let width = (icon_w + text_w + shortcut_w + 22.0 + 24.0).max(120.0);
        let height = self.items.len() as f32 * self.item_height + separators * 2.0;
        let size = kanesumi_core::Size::new(width, height);
        self.panel_size_cache.set(Some(size));
        size
    }

    /// 项命中测试（相对面板原点，面板以 `panel_rect.origin` 起排）。
    /// 布局与渲染一致：每项占 `item_height`，项后分隔线再占 2px（参 `render_panel`）。
    pub fn item_at(&self, pos: Point) -> Option<usize> {
        if pos.x < self.panel_rect.origin.x
            || pos.x >= self.panel_rect.origin.x + self.panel_rect.size.width
        {
            return None;
        }
        let local_y = pos.y - self.panel_rect.origin.y;
        let mut y = 0.0;
        for (i, item) in self.items.iter().enumerate() {
            let h = self.item_height;
            if local_y >= y && local_y < y + h {
                return Some(i);
            }
            y += h;
            if item.separator_after {
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
    pub fn item_rect(&self, i: usize) -> Rect {
        let mut y = self.panel_rect.origin.y;
        for (k, item) in self.items.iter().enumerate() {
            if k == i {
                break;
            }
            y += self.item_height;
            if item.separator_after {
                y += 2.0;
            }
        }
        Rect::new(
            self.panel_rect.origin.x,
            y,
            self.panel_rect.size.width,
            self.item_height,
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
        self.hovered = self.item_at(pos);
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
        let sub_size = MetroDropdownMenu::new(items.clone()).panel_size(engine);
        let panel = place_submenu(parent_rect, sub_size, screen, popup_gap().min(2.0));
        let mut menu = MetroDropdownMenu::new(items);
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
    pub fn path_at(&self, pos: Point) -> Option<MenuPath> {
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
        let mut y = self.panel_rect.origin.y;
        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(
                self.panel_rect.origin.x,
                y,
                self.panel_rect.size.width,
                self.item_height,
            );

            // 悬停高亮（中性）+ 级联展开项也高亮
            if self.hovered == Some(i)
                || self.submenu.as_ref().map(|s| s.parent) == Some(i)
            {
                scene.fill_rect(colors.on_surface.with_alpha(0.10), item_rect);
            }

            let mut x = self.panel_rect.origin.x + 11.0;
            // 图标
            if let Some(icon) = &item.icon {
                let icon_rect = Rect::new(x, y + (self.item_height - 16.0) / 2.0, 16.0, 16.0);
                scene.text(
                    icon.clone(),
                    icon_rect,
                    colors.on_surface,
                    icon_style(),
                    TextAlign::Center,
                );
                x += 28.0;
            }
            // 文本 —— 宽度 = 面板右缘（含右内边距 11）到当前笔位。
            // 注意：panel_rect.size.width 是相对宽度，不能直接减 x（x 是绝对坐标）。
            // 旧 bug：`panel_rect.size.width - x` 得到巨大负值 → engine.layout 触发
            // CJK 单字硬断 → 菜单项变成字塔（每个字一个字）。
            let text_right = self.panel_rect.right() - 11.0;
            let text_w = (text_right - x).max(0.0);
            let text_rect = Rect::new(
                x,
                y + (self.item_height - style.line_height) / 2.0,
                text_w,
                style.line_height,
            );
            scene.text(
                item.label.clone(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
            );
            // 子菜单指示（chevron right）
            if item.is_submenu() {
                let chevron_rect = Rect::new(
                    self.panel_rect.right() - 22.0,
                    y + (self.item_height - 12.0) / 2.0,
                    12.0,
                    12.0,
                );
                kanesumi_canvas::glyph::chevron_right(scene, chevron_rect, colors.on_surface_variant);
            }
            // 快捷键（右侧）
            if let Some(sc) = &item.shortcut {
                let sc_w = engine.measure(sc, style.size);
                let sc_rect = Rect::new(
                    self.panel_rect.origin.x + self.panel_rect.size.width - sc_w - 24.0,
                    y + (self.item_height - style.line_height) / 2.0,
                    sc_w,
                    style.line_height,
                );
                scene.text(
                    sc.clone(),
                    sc_rect,
                    colors.on_surface_variant,
                    style,
                    TextAlign::Right,
                );
            }

            y += self.item_height;
            // 分隔线
            if item.separator_after {
                let sep_rect = Rect::new(
                    self.panel_rect.origin.x + 12.0,
                    y,
                    self.panel_rect.size.width - 24.0,
                    1.0,
                );
                scene.fill_rect(colors.divider, sep_rect);
                y += 2.0;
            }
        }
    }

    /// 渲染子菜单（若展开）。画在顶层面板之上。
    pub fn render_submenu(&self, theme: &MetroTheme, _engine: &TextEngine, scene: &mut Scene) {
        let Some(sub) = &self.submenu else {
            return;
        };
        crate::popup::render_panel_base(theme, sub.panel, sub.menu.anim.panel_progress(), scene);
        let style = menu_item_style();
        let colors = &theme.colors;
        let mut y = sub.panel.origin.y;
        for (i, item) in sub.menu.items.iter().enumerate() {
            let item_rect = Rect::new(
                sub.panel.origin.x,
                y,
                sub.panel.size.width,
                self.item_height,
            );
            if sub.menu.hovered == Some(i) {
                scene.fill_rect(colors.on_surface.with_alpha(0.10), item_rect);
            }
            let mut x = sub.panel.origin.x + 11.0;
            if let Some(icon) = &item.icon {
                let icon_rect = Rect::new(x, y + (self.item_height - 16.0) / 2.0, 16.0, 16.0);
                scene.text(
                    icon.clone(),
                    icon_rect,
                    colors.on_surface,
                    icon_style(),
                    TextAlign::Center,
                );
                x += 28.0;
            }
            let text_right = sub.panel.right() - 11.0;
            let text_w = (text_right - x).max(0.0);
            let text_rect = Rect::new(
                x,
                y + (self.item_height - style.line_height) / 2.0,
                text_w,
                style.line_height,
            );
            scene.text(
                item.label.clone(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
            );
            if item.is_submenu() {
                let chevron_rect = Rect::new(
                    sub.panel.right() - 22.0,
                    y + (self.item_height - 12.0) / 2.0,
                    12.0,
                    12.0,
                );
                kanesumi_canvas::glyph::chevron_right(scene, chevron_rect, colors.on_surface_variant);
            }
            y += self.item_height;
            if item.separator_after {
                scene.fill_rect(
                    colors.divider,
                    Rect::new(
                        sub.panel.origin.x + 12.0,
                        y,
                        sub.panel.size.width - 24.0,
                        1.0,
                    ),
                );
                y += 2.0;
            }
        }
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
        let item = &mut sub.menu.items[path.index];
        if item.checked {
            return false;
        }
        item.checked = true;
        for (k, it) in sub.menu.items.iter_mut().enumerate() {
            if k != path.index && it.radio_group.as_deref() == Some(group.as_str()) {
                it.checked = false;
            }
        }
        true
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

/// 菜单项文本样式：14px 正常。
fn menu_item_style() -> TextStyle {
    TextStyle::new(14.0, 20.0, kanesumi_core::FontWeight::Normal)
}

/// 菜单图标样式：16px。
fn icon_style() -> TextStyle {
    TextStyle::new(16.0, 16.0, kanesumi_core::FontWeight::Normal)
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
            menu.path_at(child_center),
            Some(MenuPath { parent: Some(1), index: 0 })
        );
        // 顶层项 → parent=None
        let r0 = menu.item_rect(0);
        assert_eq!(
            menu.path_at(Point::new(r0.origin.x + 10.0, r0.origin.y + 10.0)),
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
}
