// MetroMenuBar —— 横排菜单栏 + 单层 MenuFlyout。
//
// 移植自 microsoft-ui-xaml/dev/MenuBar（MenuBar.cpp / MenuBarItem.cpp）。
// 与 macOS 顶栏 File/Edit/View 在结构上等价：横排 header + 每 header 一个下拉 flyout；
// Ether TopBar 与常规应用窗口菜单栏本质同物，位置由宿主决定，控件本身位置无关。
//
// 参 CONTROL_SPEC §8.5（本次新增）：
// - 行高 40（MenuBarHeight）、header padding 10,4,10,4（MenuBarItemButtonPadding）；
// - 视觉状态：Normal / PointerOver / Pressed / Selected（flyout 打开时）；
// - 交互：
//   * 点未开 header → 打开其 flyout；点已开 header → 关闭；
//   * flyout 已开时 hover 其它 header → 自动切换到该 header（UWP hover-swap）；
//   * ESC / 点击外部 → close_all；
//   * 点击 flyout 项 → 关闭并返回 (header_idx, item_idx)。
// - 键盘遍历（Alt-加速键 / Arrow Left/Right / Enter）暂略，属 Phase 3 续做。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect, TextStyle};

use crate::dropdown_menu::{MenuItem, MetroDropdownMenu};
use crate::popup::{PopupState, place_popup};

/// MenuBar header 高（UWP MenuBarHeight = 40）。
const HEADER_HEIGHT: f32 = 40.0;
/// Header 左右内边距（UWP MenuBarItemButtonPadding = 10,4,10,4）。
const HEADER_PAD_X: f32 = 10.0;

/// MenuBar 内单条 header —— label + 一组菜单项。
///
/// `items` 直接消费 [`crate::dropdown_menu::MenuItem`]（含快捷键 / 分隔线 / 图标），
/// 避免为菜单栏再造一份 item 类型。子项 `submenu` 字段暂未启用（级联菜单待 Phase 3）。
#[derive(Debug, Clone, PartialEq)]
pub struct MenuBarItem {
    pub label: String,
    pub items: Vec<MenuItem>,
}

impl MenuBarItem {
    pub fn new(label: impl Into<String>, items: Vec<MenuItem>) -> Self {
        Self {
            label: label.into(),
            items,
        }
    }
}

/// MetroMenuBar —— 横排菜单栏。
///
/// 不 derive `PartialEq`：内嵌 `MetroDropdownMenu`（含 `Cell` 缓存 + 动画）本身
/// 不实现 `PartialEq`，控件状态比较也无实际用处。
#[derive(Debug, Clone)]
pub struct MetroMenuBar {
    pub items: Vec<MenuBarItem>,
    pub header_height: f32,
    /// 当前 hover 的 header 索引。
    pub hovered_header: Option<usize>,
    /// 已按下但未释放的 header 索引（用于 press/release 一致性判定）。
    pressed_header: Option<usize>,
    /// 当前打开 flyout 的 header 索引；None = 全闭。
    open_index: Option<usize>,
    /// 每 header 对应的 flyout（一一映射，`items.len()`）。渲染 / 命中 / 动画都走它。
    flyouts: Vec<MetroDropdownMenu>,
}

impl MetroMenuBar {
    pub fn new(items: Vec<MenuBarItem>) -> Self {
        let flyouts = items
            .iter()
            .map(|it| MetroDropdownMenu::new(it.items.clone()))
            .collect();
        Self {
            items,
            header_height: HEADER_HEIGHT,
            hovered_header: None,
            pressed_header: None,
            open_index: None,
            flyouts,
        }
    }

    /// Header 文字样式：14px Normal（对齐 UWP MenuBarItem 字号，同 MenuFlyout 项）。
    pub fn header_style() -> TextStyle {
        TextStyle::new(14.0, 20.0, FontWeight::Normal)
    }

    /// 单 header 宽度（label 宽 + 2×`HEADER_PAD_X`）。
    pub fn header_width(&self, engine: &TextEngine, index: usize) -> f32 {
        if index >= self.items.len() {
            return 0.0;
        }
        let style = Self::header_style();
        engine.measure(&self.items[index].label, style.size) + HEADER_PAD_X * 2.0
    }

    /// 全部 header 累加总宽（不含 rect 起点）。
    pub fn total_width(&self, engine: &TextEngine) -> f32 {
        (0..self.items.len())
            .map(|i| self.header_width(engine, i))
            .sum()
    }

    /// 可见 header 矩形。只保留完整落在宿主内的项，渲染、命中与弹层锚点共用。
    pub fn header_rects(&self, engine: &TextEngine, rect: Rect) -> Vec<Rect> {
        let rect = rect.normalized();
        let mut out = Vec::with_capacity(self.items.len());
        let mut cursor = rect.origin.x;
        for i in 0..self.items.len() {
            let w = self.header_width(engine, i);
            if cursor + w > rect.right() + f32::EPSILON {
                break;
            }
            out.push(Rect::new(
                cursor,
                rect.origin.y,
                w,
                self.header_height.min(rect.size.height),
            ));
            cursor += w;
        }
        out
    }

    /// Header 命中：返回索引（`rect` = MenuBar 布局矩形，`p` = 绝对指针坐标）。
    pub fn header_at(&self, engine: &TextEngine, rect: Rect, p: Point) -> Option<usize> {
        if p.y < rect.origin.y || p.y >= rect.origin.y + self.header_height {
            return None;
        }
        for (i, header) in self.header_rects(engine, rect).iter().enumerate() {
            if header.contains(p) {
                return Some(i);
            }
        }
        None
    }

    /// 设置 header 的子菜单项并同步到对应 flyout（⚠ `items[idx].items` 与 `flyouts[idx]`
    /// 是两份快照，直接改 `items` 会导致 flyout 渲染陈旧/空项 —— 必须经此方法同步）。
    pub fn set_item_submenu(&mut self, index: usize, items: Vec<MenuItem>) {
        if index >= self.items.len() {
            return;
        }
        self.items[index].items = items.clone();
        self.flyouts[index].items = items;
        self.flyouts[index].invalidate_layout();
    }

    /// 打开指定 header 的 flyout（自动收拢其它）。`screen` 供 `place_popup` 方向自适应。
    pub fn open(&mut self, index: usize, engine: &TextEngine, rect: Rect, screen: Rect) {
        if index >= self.items.len() {
            return;
        }
        // 收拢其它
        for (i, f) in self.flyouts.iter_mut().enumerate() {
            if i != index {
                f.close();
            }
        }
        let Some(trigger) = self.header_rects(engine, rect).get(index).copied() else {
            return;
        };
        let size = self.flyouts[index].panel_size(engine);
        // gap=0：AppMenu flyout 贴 TopBar 下缘（macOS 菜单栏下拉无间隙，与 bar 连续面板）。
        let placement = place_popup(trigger, size, screen, 0.0);
        self.flyouts[index].open(placement.rect);
        self.open_index = Some(index);
    }

    pub fn close_all(&mut self) {
        for f in &mut self.flyouts {
            f.close();
        }
        self.open_index = None;
    }

    pub fn update(&mut self, dt: f64) {
        for f in &mut self.flyouts {
            f.update(dt);
        }
        // flyout 动画走完转 Closed 后，同步 open_index。
        if let Some(i) = self.open_index
            && !self.flyouts[i].anim.is_visible()
        {
            self.open_index = None;
        }
    }

    /// 是否有 flyout 可见（含淡出中）。用于宿主判定弹层优先级。
    pub fn is_flyout_visible(&self) -> bool {
        self.flyouts.iter().any(|f| f.anim.is_visible())
    }

    /// 是否有 flyout 动画推进中（Opening/Closing）。静态 Open 不需要每帧重绘
    /// （宿主 needs_redraw 脏标记语义用；参 TOPBAR_RENDER_REFACTOR §4.6）。
    pub fn is_animating(&self) -> bool {
        self.flyouts
            .iter()
            .any(|f| matches!(f.anim.state(), PopupState::Opening | PopupState::Closing))
    }

    /// 当前展开 flyout 的面板矩形（无展开返回 None）。供宿主 light dismiss：
    /// 点击面板外 → 关闭 flyout（AppMenu 菜单栏语义，点击别处收菜单）。
    pub fn flyout_panel_rect(&self) -> Option<Rect> {
        let i = self.open_index?;
        self.flyouts.get(i).map(|f| f.panel_rect)
    }

    /// 当前展开 flyout 的面板高度（无展开返回 0）。供宿主 `preferred_height` 精确扩展，
    /// 避免固定保守高度造成「全宽大黑区」。参 settings kanesumi_topbar。
    pub fn flyout_height(&self, engine: &TextEngine) -> f32 {
        match self.open_index {
            Some(i) if i < self.flyouts.len() => self.flyouts[i].panel_size(engine).height,
            _ => 0.0,
        }
    }

    /// 打开的 flyout 内当前悬停项 `(header 索引, 项索引)`；未开 → None。
    /// S2 悬停语义签名用（App::hover_signature：flyout 内悬停变化触发重绘）。
    pub fn hovered_item(&self) -> Option<(usize, Option<usize>)> {
        match self.open_index {
            Some(i) if i < self.flyouts.len() => Some((i, self.flyouts[i].hovered)),
            _ => None,
        }
    }

    /// 命中 flyout 项：仅当有 flyout 展开时有效，返回 (header_idx, item_idx)。
    pub fn item_at(&self, p: Point) -> Option<(usize, usize)> {
        let i = self.open_index?;
        if !self.flyouts[i].anim.is_open() {
            return None;
        }
        self.flyouts[i].item_at(p).map(|j| (i, j))
    }

    /// 悬停路由：
    /// - 更新 header hover 视觉；
    /// - **flyout hover-swap**：若已有 flyout 开且指针悬到别的 header → 切换到该 header
    ///   （UWP MenuBarItem::OnMenuBarItemPointerEntered 行为）；
    /// - flyout 内部 hover → 同步项 hover。
    pub fn hover(&mut self, engine: &TextEngine, rect: Rect, screen: Rect, p: Point) {
        // Header 命中
        let hit = self.header_at(engine, rect, p);
        self.hovered_header = hit;

        if let Some(idx) = hit {
            if let Some(open) = self.open_index
                && open != idx
            {
                // hover-swap
                self.open(idx, engine, rect, screen);
            }
            return;
        }

        // 指针在 flyout 面板内 → 更新项 hover
        if let Some(i) = self.open_index {
            self.flyouts[i].hovered = self.flyouts[i].item_at(p);
        }
    }

    /// 按下：
    /// - 命中 header → 记录 pressed，release 时切换（避免 press-drag 意外提交）；
    /// - 命中 flyout 项 → 不在此处提交（按 release 走）；
    /// - 命中弹层外空白 → close_all（模拟点击外部关闭）。
    ///
    /// 返回 `true` 表示 MenuBar 消费了该按下（宿主别再向下路由）。
    pub fn press(&mut self, engine: &TextEngine, rect: Rect, p: Point) -> bool {
        if let Some(idx) = self.header_at(engine, rect, p) {
            self.pressed_header = Some(idx);
            return true;
        }
        // Flyout 打开时，点面板外空白 → 关闭；点面板内交给 release 处理。
        if let Some(i) = self.open_index {
            if self.flyouts[i].anim.is_visible() && self.flyouts[i].item_at(p).is_none() {
                self.close_all();
                return true;
            }
            if self.flyouts[i].item_at(p).is_some() {
                return true; // 消费，release 决定
            }
        }
        false
    }

    /// 释放：
    /// - press 落在同 header 上 → toggle 该 header 的 flyout；
    /// - flyout 内命中项 → 关闭并返回 (header_idx, item_idx) 给宿主派发动作；
    /// - 其它 → 无操作，仅清 pressed。
    pub fn release(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        screen: Rect,
        p: Point,
    ) -> Option<(usize, usize)> {
        // Header 释放
        if let Some(pressed) = self.pressed_header.take() {
            let hit = self.header_at(engine, rect, p);
            if hit == Some(pressed) {
                // toggle
                if self.open_index == Some(pressed) {
                    self.close_all();
                } else {
                    self.open(pressed, engine, rect, screen);
                }
            }
            return None;
        }
        // Flyout 项释放
        if let Some(i) = self.open_index
            && let Some(j) = self.flyouts[i].item_at(p)
        {
            self.close_all();
            return Some((i, j));
        }
        None
    }

    /// 渲染：header 行 + 命中 header 的 flyout（若开）。
    /// `screen` = 弹层可占区域（供 flyout `render` 画遮罩）。
    pub fn render(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        _screen: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let style = Self::header_style();
        let geoms = self.header_rects(engine, rect);

        scene.push_clip(rect);
        for (i, hrect) in geoms.iter().copied().enumerate() {
            // 背景高亮：Selected（flyout 开）≡ Pressed 亮度；PointerOver 更淡。
            // 用 on_surface 的低 alpha（Fluent SubtleFill 语义映射到 Kanesumi 纯色）。
            let bg_alpha = if self.open_index == Some(i) || self.pressed_header == Some(i) {
                Some(0.10)
            } else if self.hovered_header == Some(i) {
                Some(0.06)
            } else {
                None
            };
            if let Some(a) = bg_alpha {
                scene.fill_rect(colors.on_surface.with_alpha(a), hrect);
            }

            // Label —— 竖直居中，靠左（padding 10）。
            let label_w = engine.measure(&self.items[i].label, style.size);
            let text_rect = Rect::new(
                hrect.origin.x + HEADER_PAD_X,
                rect.origin.y + (self.header_height - style.line_height) / 2.0,
                label_w,
                style.line_height,
            );
            scene.text(
                self.items[i].label.clone(),
                text_rect,
                colors.on_surface,
                style,
                TextAlign::Left,
            );
        }
        scene.pop_clip();

        // Flyout —— 每帧渲染一个（open_index 唯一）。菜单栏 flyout 无遮罩
        // （render_panel 而非 render：render 会 render_overlay 黑 70% 盖满主表面整宽，
        // 把 bar 与扩展区全压黑 → 「屏幕等宽大黑区」）。参 dropdown_menu render_overlay。
        if let Some(i) = self.open_index {
            self.flyouts[i].render_panel(theme, engine, scene);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
            "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    fn menubar() -> MetroMenuBar {
        MetroMenuBar::new(vec![
            MenuBarItem::new(
                "文件",
                vec![
                    MenuItem::new("新建"),
                    MenuItem::new("打开"),
                    MenuItem::new("保存"),
                ],
            ),
            MenuBarItem::new(
                "编辑",
                vec![
                    MenuItem::new("撤销"),
                    MenuItem::new("重做"),
                    MenuItem::new("剪切"),
                    MenuItem::new("复制"),
                    MenuItem::new("粘贴"),
                ],
            ),
            MenuBarItem::new("视图", vec![MenuItem::new("全屏"), MenuItem::new("放大")]),
        ])
    }

    fn barrect() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 40.0)
    }

    fn screen() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    #[test]
    fn header_layout_grows_with_label() {
        let Some(engine) = find_engine() else { return };
        let bar = menubar();
        assert!(bar.header_width(&engine, 0) > 0.0);
        // 累加宽度 = 每 header 宽之和
        let sum: f32 = (0..bar.items.len())
            .map(|i| bar.header_width(&engine, i))
            .sum();
        assert!((bar.total_width(&engine) - sum).abs() < 1e-4);
    }

    #[test]
    fn narrow_host_only_exposes_fully_visible_headers() {
        let Some(engine) = find_engine() else { return };
        let bar = menubar();
        let first = bar.header_width(&engine, 0);
        let rects = bar.header_rects(&engine, Rect::new(10.0, 0.0, first + 2.0, 40.0));
        assert_eq!(rects.len(), 1);
        assert!(rects[0].right() <= 12.0 + first);
        assert_eq!(
            bar.header_at(
                &engine,
                Rect::new(10.0, 0.0, first + 2.0, 40.0),
                rects[0].center()
            ),
            Some(0)
        );
    }

    #[test]
    fn header_at_maps_x() {
        let Some(engine) = find_engine() else { return };
        let bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let w1 = bar.header_width(&engine, 1);
        // 第一 header 中心
        assert_eq!(
            bar.header_at(&engine, barrect(), Point::new(w0 / 2.0, 20.0)),
            Some(0)
        );
        // 第二 header 中心
        assert_eq!(
            bar.header_at(&engine, barrect(), Point::new(w0 + w1 / 2.0, 20.0)),
            Some(1)
        );
        // rect 外
        assert_eq!(
            bar.header_at(&engine, barrect(), Point::new(w0 / 2.0, 100.0)),
            None
        );
    }

    #[test]
    fn click_header_toggles_flyout() {
        let Some(engine) = find_engine() else { return };
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let p = Point::new(w0 / 2.0, 20.0);
        assert!(bar.open_index.is_none());
        bar.press(&engine, barrect(), p);
        bar.release(&engine, barrect(), screen(), p);
        assert_eq!(bar.open_index, Some(0), "第一次点开 header 0");
        // 再点同 header → 关
        bar.press(&engine, barrect(), p);
        bar.release(&engine, barrect(), screen(), p);
        // close 是动画（is_visible 会保 True 一段时间），但 open_index 需清零
        assert_eq!(bar.open_index, None, "再次点应关闭");
    }

    #[test]
    fn hover_swaps_flyout_when_open() {
        let Some(engine) = find_engine() else { return };
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let w1 = bar.header_width(&engine, 1);
        // 打开 header 0
        let p0 = Point::new(w0 / 2.0, 20.0);
        bar.press(&engine, barrect(), p0);
        bar.release(&engine, barrect(), screen(), p0);
        assert_eq!(bar.open_index, Some(0));
        // hover 到 header 1 → 自动切换
        let p1 = Point::new(w0 + w1 / 2.0, 20.0);
        bar.hover(&engine, barrect(), screen(), p1);
        assert_eq!(bar.open_index, Some(1), "hover-swap 到 header 1");
    }

    #[test]
    fn click_flyout_item_returns_indices_and_closes() {
        let Some(engine) = find_engine() else { return };
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        // 打开 header 0
        let head_p = Point::new(w0 / 2.0, 20.0);
        bar.press(&engine, barrect(), head_p);
        bar.release(&engine, barrect(), screen(), head_p);
        assert_eq!(bar.open_index, Some(0));
        // flyout 面板首项中心（面板 origin.x + 20, origin.y + item_height/2）
        let panel = bar.flyouts[0].panel_rect;
        let item_p = Point::new(panel.origin.x + 20.0, panel.origin.y + 16.0);
        bar.press(&engine, barrect(), item_p);
        let hit = bar.release(&engine, barrect(), screen(), item_p);
        assert_eq!(hit, Some((0, 0)), "点首项应返回 (header 0, item 0)");
        assert_eq!(bar.open_index, None, "点项后 flyout 关闭");
    }

    #[test]
    fn click_outside_flyout_closes_it() {
        let Some(engine) = find_engine() else { return };
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let head_p = Point::new(w0 / 2.0, 20.0);
        bar.press(&engine, barrect(), head_p);
        bar.release(&engine, barrect(), screen(), head_p);
        assert_eq!(bar.open_index, Some(0));
        // 空白（远离 header 和 panel）
        let outside = Point::new(700.0, 500.0);
        let handled = bar.press(&engine, barrect(), outside);
        assert!(handled, "flyout 开时点外部消费该事件");
        assert_eq!(bar.open_index, None, "外部点击应关闭");
    }

    #[test]
    fn render_emits_headers_and_flyout_when_open() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let head_p = Point::new(w0 / 2.0, 20.0);
        bar.press(&engine, barrect(), head_p);
        bar.release(&engine, barrect(), screen(), head_p);
        // 推动画到稳态
        for _ in 0..60 {
            bar.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        bar.render(&theme, &engine, barrect(), screen(), &mut scene);
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::Text { .. }))
            .count();
        // 3 header label + 3 flyout item + 1 selected header 高亮 fill 至少
        assert!(
            texts >= 6,
            "至少 3 header + 3 flyout item 文本，实际 {texts}"
        );
    }

    #[test]
    fn update_syncs_open_index_when_flyout_closes() {
        let Some(engine) = find_engine() else { return };
        let mut bar = menubar();
        let w0 = bar.header_width(&engine, 0);
        let p = Point::new(w0 / 2.0, 20.0);
        bar.press(&engine, barrect(), p);
        bar.release(&engine, barrect(), screen(), p);
        assert_eq!(bar.open_index, Some(0));
        bar.close_all();
        // update 推到 flyout 完全 Closed 后 open_index 应同步为 None
        for _ in 0..120 {
            bar.update(1.0 / 60.0);
        }
        assert_eq!(bar.open_index, None);
    }
}
