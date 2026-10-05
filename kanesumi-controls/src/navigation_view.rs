// MetroNavigationView —— 侧边导航。参 CONTROL_SPEC §28。
//
// 移植自 microsoft-ui-xaml/dev/NavigationView（NavigationView.xaml + NavigationView_themeresources.xaml）：
// - Left 模式：Expanded Pane 320 / Compact 48；Toggle 40×40；Item 高 40、icon 16、字 14；
//   选中指示条 3×16 强调色；Header Margin 56,44,0,0；
// - Top 模式：顶栏 48，项横排。
// 子项级联 / flyout / 动画式收窄暂略（Phase 3 续）。

use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::glyph;
use kanesumi_canvas::icon::Icon;
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{FontWeight, MetroTheme, Point, Rect};

/// Expanded Pane 宽（320）。
pub const NAV_PANE_EXPANDED: f32 = 320.0;
/// Compact Pane 宽（48）。
pub const NAV_PANE_COMPACT: f32 = 48.0;
/// Top Pane 高（48）。
pub const NAV_TOP_HEIGHT: f32 = 48.0;
/// Toggle 按钮 40×40。
pub const NAV_TOGGLE: f32 = 40.0;
/// Item 高（40）。
pub const NAV_ITEM_H: f32 = 40.0;
/// Header Margin（56,44,0,0 → 左 56、上 44）。
pub const NAV_HEADER_MARGIN: (f32, f32) = (56.0, 44.0);
/// 选中指示条 3×16。
pub const NAV_INDICATOR: (f32, f32) = (3.0, 16.0);
/// Top 模式 item 起始 padding —— 与 render 内 `x = r.origin.x + 16.0` 一致。
pub const NAV_TOP_ITEM_PAD_LEFT: f32 = 16.0;
/// Top 模式 item 右 padding —— 与 render 内 label `right - 12` 一致。
pub const NAV_TOP_ITEM_PAD_RIGHT: f32 = 12.0;
/// Top 模式 item 最小宽 —— 老硬编码值（40 + 16），保证 icon-only / 短 label 观感稳定。
pub const NAV_TOP_ITEM_MIN_W: f32 = NAV_ITEM_H + 16.0;
/// Top 模式 item 最大宽 —— 对齐 UWP NavigationViewItem 顶模式默认上限，防止长中文标签独占顶栏。
pub const NAV_TOP_ITEM_MAX_W: f32 = 240.0;
/// Top 模式带图标项的 icon 槽宽（含 icon 16 + 与 label 间距 12，与 render 内 `x += 28` 对齐）。
const NAV_TOP_ICON_SLOT: f32 = 28.0;
/// Top 模式无图标项额外左移（保持与带图标项 label 视觉起点接近，对齐 render `else x += 16`）。
const NAV_TOP_NO_ICON_PAD: f32 = 16.0;

/// 导航模式（对应 WinUI `NavigationViewPaneDisplayMode` 的左侧四态 + 顶部）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationPaneMode {
    /// 左侧展开 / 收起（WinUI `Left`）：pane 常驻并**推挤**内容，宽 320↔48 随动画插值。
    Left,
    /// 左侧图标栏（WinUI `LeftCompact`）：收起时 48 图标栏推挤内容；打开时 pane 以 320
    /// **覆盖**内容（内容不随 pane 展开而移动）。
    LeftCompact,
    /// 左侧最小（WinUI `LeftMinimal`）：仅汉堡键，内容占满；打开时 pane **覆盖**内容。
    LeftMinimal,
    /// 顶部（WinUI `Top`）：顶栏横排，内容在顶栏下。
    Top,
}

impl NavigationPaneMode {
    pub fn is_left(self) -> bool {
        !matches!(self, NavigationPaneMode::Top)
    }

    pub fn is_top(self) -> bool {
        matches!(self, NavigationPaneMode::Top)
    }
}

/// 导航项。
#[derive(Debug, Clone, PartialEq)]
pub struct NavigationViewItem {
    pub label: String,
    /// 图标文本（16px；None = 无图标）。图像图标 `icon_image` 优先。
    pub icon: Option<String>,
    /// 图像图标（SVG 光栅化，16×16 呈现）。None = 无图像图标，回退文本 `icon`。
    pub icon_image: Option<Icon>,
    /// 子项（Left 模式缩进展示）。
    pub children: Vec<NavigationViewItem>,
}

impl NavigationViewItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: None,
            icon_image: None,
            children: Vec::new(),
        }
    }

    pub fn with_icon(label: impl Into<String>, icon: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            icon: Some(icon.into()),
            icon_image: None,
            children: Vec::new(),
        }
    }

    pub fn with_image_icon(label: impl Into<String>, icon: Icon) -> Self {
        Self {
            label: label.into(),
            icon: None,
            icon_image: Some(icon),
            children: Vec::new(),
        }
    }
}

/// 导航点击结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationAction {
    None,
    /// 选中项（索引路径）。
    Select(Vec<usize>),
    /// Pane toggle（Left 模式展开/收窄）。
    Toggle,
}

/// MetroNavigationView —— 侧边导航。参 CONTROL_SPEC §28。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroNavigationView {
    pub items: Vec<NavigationViewItem>,
    pub footer_items: Vec<NavigationViewItem>,
    /// 选中路径（索引）。
    pub selected: Option<Vec<usize>>,
    pub mode: NavigationPaneMode,
    /// Left 模式 Pane 展开态（320）vs 收窄（48）—— 逻辑目标。
    pub pane_expanded: bool,
    /// Pane 展开/收窄进度 [0,1]（0 = 收窄，1 = 展开），`pane_width()` 据此插值。
    /// 参 CONTROL_SPEC §28「动画式展开收窄」—— sokuou 时长驱动，可中断。
    pane: MetroAnim,
    /// 头部标题（Header 区，宿主也可自绘）。
    pub header: String,
    pub toggle_hovered: bool,
    pub footer_selected: Option<usize>,
}

impl Default for MetroNavigationView {
    fn default() -> Self {
        // 初始展开（进度 1.0）。
        let mut pane = MetroAnim::new(0.25, UwpEasing::Quadratic, EasingMode::EaseOut);
        pane.jump_to(1.0);
        Self {
            items: Vec::new(),
            footer_items: Vec::new(),
            selected: None,
            mode: NavigationPaneMode::Left,
            pane_expanded: true,
            pane,
            header: String::new(),
            toggle_hovered: false,
            footer_selected: None,
        }
    }
}

impl MetroNavigationView {
    pub fn new(items: Vec<NavigationViewItem>) -> Self {
        Self {
            items,
            ..Self::default()
        }
    }

    /// 当前 Pane 宽（Left 模式）—— 展开/收窄进度插值。
    ///
    /// `Left` / `LeftCompact`：48↔320（有一根常驻图标栏，展开沿其加宽）。
    /// `LeftMinimal`：0↔320（无图标栏，展开即整块浮出）。
    pub fn pane_width(&self) -> f32 {
        let p = self.pane.value() as f32;
        match self.mode {
            NavigationPaneMode::LeftMinimal => NAV_PANE_EXPANDED * p,
            _ => NAV_PANE_COMPACT + (NAV_PANE_EXPANDED - NAV_PANE_COMPACT) * p,
        }
    }

    /// 内容区被 pane **推挤**的宽度（参 CONTROL_SPEC §28）。
    ///
    /// - `Left`：随 pane 动画插值（展开推挤、收起让位）。
    /// - `LeftCompact`：恒为图标栏宽 48；展开的 pane 覆盖其上，内容不移动。
    /// - `LeftMinimal`：恒为 0；展开的 pane 覆盖内容。
    fn content_push(&self, rect: Rect) -> f32 {
        match self.mode {
            NavigationPaneMode::Left => self.effective_pane_width(rect),
            NavigationPaneMode::LeftCompact => NAV_PANE_COMPACT.min(rect.size.width.max(0.0)),
            NavigationPaneMode::LeftMinimal | NavigationPaneMode::Top => 0.0,
        }
    }

    /// Pane 展开进度 [0,1]（0 = 收窄，1 = 展开）。
    pub fn pane_progress(&self) -> f32 {
        self.pane.value() as f32
    }

    /// 设置 Pane 展开态并启动展开/收窄动画（可中断）。幂等（目标不变则无操作）。
    pub fn set_pane_expanded(&mut self, expanded: bool) {
        if self.pane_expanded == expanded {
            return;
        }
        self.pane_expanded = expanded;
        self.pane.set_target(if expanded { 1.0 } else { 0.0 });
    }

    /// 直接设定 Pane 展开进度 [0,1]（快照 / 测试用，不启动动画）。
    pub fn set_pane_progress(&mut self, progress: f32) {
        self.pane.jump_to(progress.clamp(0.0, 1.0) as f64);
    }

    /// Pane 展开/收窄动画是否进行中。
    pub fn is_animating(&self) -> bool {
        !self.pane.is_steady()
    }

    /// 每帧推进 Pane 展开/收窄动画。宿主（App::update）调用。
    pub fn update(&mut self, dt: f64) {
        self.pane.update(dt);
    }

    /// 给定宿主内实际可用 Pane 宽。窄窗口绝不把 320px 内在宽画出宿主边界。
    pub fn effective_pane_width(&self, rect: Rect) -> f32 {
        self.pane_width().min(rect.size.width.max(0.0))
    }

    /// Toggle 按钮 rect（Left / Top 均在原点起 40×40）。
    pub fn toggle_rect(&self, rect: Rect) -> Rect {
        Rect::new(rect.origin.x, rect.origin.y, NAV_TOGGLE, NAV_TOGGLE)
    }

    /// 项区（Left：Pane 内 Toggle 下方；Top：Toggle 右侧横排）。
    fn item_area(&self, rect: Rect) -> Rect {
        match self.mode {
            NavigationPaneMode::Left
            | NavigationPaneMode::LeftCompact
            | NavigationPaneMode::LeftMinimal => Rect::new(
                rect.origin.x,
                rect.origin.y + NAV_TOGGLE,
                self.effective_pane_width(rect),
                (rect.size.height - NAV_TOGGLE).max(0.0),
            ),
            NavigationPaneMode::Top => Rect::new(
                rect.origin.x + NAV_TOGGLE,
                rect.origin.y,
                (rect.size.width - NAV_TOGGLE).max(0.0),
                NAV_TOP_HEIGHT,
            ),
        }
    }

    /// Top 模式单项宽度：`pad_left + icon_slot + label_measure + pad_right`，夹紧 [MIN, MAX]。
    /// 与 render 内部 label 起点 / 右缘一致，避免超长中文标签被裁掉。
    fn top_item_width(&self, engine: &TextEngine, index: usize) -> f32 {
        let Some(item) = self.items.get(index) else {
            return NAV_TOP_ITEM_MIN_W;
        };
        let style = Self::item_style();
        let label_w = if item.label.is_empty() {
            0.0
        } else {
            engine.measure_with_spacing_weighted(&item.label, style.size, style.letter_spacing_em, style.weight)
        };
        let icon_w = if item.icon.is_some() {
            NAV_TOP_ICON_SLOT
        } else {
            NAV_TOP_NO_ICON_PAD
        };
        let raw = NAV_TOP_ITEM_PAD_LEFT + icon_w + label_w + NAV_TOP_ITEM_PAD_RIGHT;
        raw.clamp(NAV_TOP_ITEM_MIN_W, NAV_TOP_ITEM_MAX_W)
    }

    /// 顶层项 rect（按模式排布）。
    ///
    /// Top 模式按 label 测量算宽（[MIN, MAX] 夹紧）；Left 模式无需测量，`engine` 忽略。
    pub fn top_item_rects(&self, engine: &TextEngine, rect: Rect) -> Vec<Rect> {
        let area = self.item_area(rect);
        match self.mode {
            NavigationPaneMode::Left
            | NavigationPaneMode::LeftCompact
            | NavigationPaneMode::LeftMinimal => {
                let mut y = area.origin.y;
                self.items
                    .iter()
                    .map(|item| {
                        let rect = Rect::new(area.origin.x, y, area.size.width, NAV_ITEM_H);
                        y += NAV_ITEM_H;
                        if self.pane_expanded && !item.children.is_empty() {
                            y += item.children.len() as f32 * NAV_ITEM_H;
                        }
                        rect
                    })
                    .collect()
            }
            NavigationPaneMode::Top => {
                let mut x = area.origin.x;
                (0..self.items.len())
                    .map(|i| {
                        let w = self.top_item_width(engine, i);
                        let r = Rect::new(x, area.origin.y, w, NAV_ITEM_H);
                        x += w;
                        r
                    })
                    .collect()
            }
        }
    }

    /// Header rect（Left：**pane 推挤宽 + Header Margin**、y=44；Top：顶栏下）。
    ///
    /// 2026-10-01 修正：旧实现固定从 `origin + (56,44)` 起算，pane 展开（320）时内容区压住
    /// 左侧类目、点类目被内容子节点截走。现按 WinUI `NavigationView` 语义从 pane 当前推挤宽之后起算
    /// —— `Left` 随展开/收窄动画平移（只动 arrange，不重量测），`LeftCompact` / `LeftMinimal`
    /// 打开时 pane 覆盖、内容不移动。
    pub fn header_rect(&self, rect: Rect) -> Rect {
        match self.mode {
            NavigationPaneMode::Left
            | NavigationPaneMode::LeftCompact
            | NavigationPaneMode::LeftMinimal => {
                let push = self.content_push(rect);
                // 夹到父右缘：极端窄窗口下 pane 已占满，内容退化为零宽，绝不越出父矩形。
                let x = (rect.origin.x + push + NAV_HEADER_MARGIN.0).min(rect.right());
                Rect::new(
                    x,
                    rect.origin.y + NAV_HEADER_MARGIN.1,
                    (rect.right() - x).max(0.0),
                    NAV_ITEM_H,
                )
            }
            NavigationPaneMode::Top => Rect::new(
                rect.origin.x + NAV_TOGGLE,
                rect.origin.y + NAV_TOP_HEIGHT,
                (rect.size.width - NAV_TOGGLE).max(0.0),
                NAV_ITEM_H,
            ),
        }
    }

    /// Content rect（剩余区域）。
    pub fn content_rect(&self, rect: Rect) -> Rect {
        let hr = self.header_rect(rect);
        Rect::new(
            hr.origin.x,
            hr.bottom(),
            hr.size.width,
            (rect.bottom() - hr.bottom()).max(0.0),
        )
    }

    fn item_style() -> TextStyle {
        MetroTypography::metro().body_medium
    }

    /// 命中：Toggle / 顶层项。`engine` 仅 Top 模式用来量 label 宽；Left 模式忽略。
    pub fn hit(&self, engine: &TextEngine, rect: Rect, pos: Point) -> NavigationAction {
        if self.toggle_rect(rect).contains(pos) {
            return NavigationAction::Toggle;
        }
        for (i, r) in self.top_item_rects(engine, rect).iter().enumerate() {
            if r.contains(pos) {
                return NavigationAction::Select(vec![i]);
            }
        }
        NavigationAction::None
    }

    /// 应用点击：Toggle / Select。
    pub fn handle_click(
        &mut self,
        engine: &TextEngine,
        rect: Rect,
        pos: Point,
    ) -> NavigationAction {
        match self.hit(engine, rect, pos) {
            NavigationAction::Select(path) => {
                self.selected = Some(path.clone());
                NavigationAction::Select(path)
            }
            NavigationAction::Toggle if self.mode.is_left() => {
                self.set_pane_expanded(!self.pane_expanded);
                NavigationAction::Toggle
            }
            NavigationAction::Toggle => NavigationAction::Toggle,
            NavigationAction::None => NavigationAction::None,
        }
    }

    /// 悬停路由。
    pub fn hover(&mut self, rect: Rect, pos: Point) {
        self.toggle_hovered = self.toggle_rect(rect).contains(pos);
    }

    /// 渲染 Pane / Top 栏（Toggle + 项列表 + Footer）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = Self::item_style();

        scene.push_clip(rect);
        // Toggle（☰ 自绘三横）
        let tr = self.toggle_rect(rect);
        if self.toggle_hovered {
            scene.fill_rounded_rect(
                theme.indication.hover_tint,
                tr,
                theme.tokens.corner_radius,
            );
        }
        let cx = tr.center().x;
        let cy = tr.center().y;
        let bar_w = 14.0;
        let t = 1.5;
        for dy in [-6.0, 0.0, 6.0] {
            scene.fill_rect(
                colors.on_surface,
                Rect::new(cx - bar_w / 2.0, cy + dy - t / 2.0, bar_w, t),
            );
        }

        // 项列表裁到 pane 区：LeftMinimal 收起（pane 宽 0）时不得让图标漏到内容上；
        // Top 模式裁到顶栏高。Toggle 已在此之外画好，始终可见。
        let pane_clip = match self.mode {
            NavigationPaneMode::Top => {
                Rect::new(rect.origin.x, rect.origin.y, rect.size.width, NAV_TOP_HEIGHT)
            }
            _ => Rect::new(
                rect.origin.x,
                rect.origin.y,
                self.effective_pane_width(rect),
                rect.size.height,
            ),
        };
        scene.push_clip(pane_clip);

        let rects = self.top_item_rects(engine, rect);
        // Pane 展开进度：label / chevron / 子项按进度淡入（Top 模式恒全显）。
        let pane_p = self.pane_progress();
        // 子项可见性（Left 展开 + 选中展开）——简化为展开态展示 children。
        for (i, item) in self.items.iter().enumerate() {
            let r = rects[i];
            let selected = self.selected.as_deref() == Some([i].as_slice());
            // 底
            if selected {
                scene.fill_rect(theme.indication.subtle_tint, r);
            }
            // 选中指示条（3×16 强调色，左侧）
            if selected {
                scene.fill_rect(
                    colors.primary,
                    Rect::new(
                        r.origin.x,
                        r.origin.y + (r.size.height - NAV_INDICATOR.1) / 2.0,
                        NAV_INDICATOR.0,
                        NAV_INDICATOR.1,
                    ),
                );
            }
            // icon（图像图标优先于文本 glyph；SVG 光栅化，缺失回退文本）
            let mut x = r.origin.x + 16.0;
            if let Some(img) = &item.icon_image {
                let icon_rect =
                    Rect::new(x, r.origin.y + (r.size.height - 16.0) / 2.0, 16.0, 16.0);
                scene.image(img, icon_rect, None);
                x += 16.0 + 12.0;
            } else if let Some(icon) = &item.icon {
                let icon_rect =
                    Rect::new(x, r.origin.y + (r.size.height - 16.0) / 2.0, 16.0, 16.0);
                // 图标字形保持 Normal —— 符号笔画本就细，加粗会糊。
                scene.text(
                    icon.clone(),
                    icon_rect,
                    colors.on_surface_variant,
                    TextStyle::new(16.0, 16.0, FontWeight::Normal),
                    TextAlign::Center,
                );
                x += 16.0 + 12.0;
            } else {
                x += 16.0; // 无 icon 时 label 顶到 padding 16
            }
            // label（收窄态隐藏；展开/收窄动画期间按进度淡入淡出）
            if pane_p > 0.0 || self.mode.is_top() {
                let alpha = if self.mode.is_top() { 1.0 } else { pane_p };
                let label_w = (r.right() - x - 12.0).max(0.0);
                let fg = if selected {
                    colors.on_surface
                } else {
                    colors.on_surface_variant
                }
                .with_alpha(alpha);
                scene.text(
                    item.label.clone(),
                    Rect::new(
                        x,
                        r.origin.y + (r.size.height - style.line_height) / 2.0,
                        label_w,
                        style.line_height,
                    ),
                    fg,
                    style,
                    TextAlign::Left,
                );
                // 子项 chevron
                if !item.children.is_empty() {
                    let chev = Rect::new(
                        r.right() - 28.0,
                        r.origin.y + (r.size.height - 12.0) / 2.0,
                        12.0,
                        12.0,
                    );
                    glyph::chevron_right(scene, chev, colors.on_surface_variant.with_alpha(alpha));
                }
            }

            // 子项（展开时）
            if self.pane_expanded && !item.children.is_empty() {
                let base_y = r.bottom();
                for (j, child) in item.children.iter().enumerate() {
                    let cr = Rect::new(
                        rect.origin.x + 16.0,
                        base_y + j as f32 * NAV_ITEM_H,
                        (self.effective_pane_width(rect) - 16.0).max(0.0),
                        NAV_ITEM_H,
                    );
                    let csel = self.selected.as_deref() == Some([i, j].as_slice());
                    if csel {
                        scene.fill_rect(theme.indication.subtle_tint, cr);
                    }
                    scene.text(
                        child.label.clone(),
                        Rect::new(
                            cr.origin.x + 16.0,
                            cr.origin.y + (cr.size.height - style.line_height) / 2.0,
                            cr.size.width - 16.0,
                            style.line_height,
                        ),
                        colors.on_surface_variant.with_alpha(pane_p),
                        style,
                        TextAlign::Left,
                    );
                }
            }
        }
        scene.pop_clip(); // pane 裁剪
        scene.pop_clip(); // 整体矩形裁剪
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// **容器**：导航栏（Toggle + 项列表 + Footer）由控件自绘，内容区放**子节点**（App 把页面挂
// 在它下面）。`arrange` 把每个子节点排进 `content_rect(rect)`，`clips_children` 保持 true。
// 点导航项发 `NavigationItemInvoked`；点汉堡键展开 / 收起（Left 模式）。展开 / 收窄动画期间
// 只 `invalidate_arrange`（内容区平移）与 `invalidate_paint`（导航栏随宽度重画），**不**重新
// 量测子树（参 ELEMENT_TREE §Ⅴ.1「动画只动视觉」）。

/// 元素树动作：导航项被选中（点击）。携带顶层项索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavigationItemInvoked(pub usize);

impl kanesumi_element::Widget for MetroNavigationView {
    /// 容器通常铺满窗口：可用尺寸全要（无界轴归 0 以防非有限矩形）。
    fn measure(
        &mut self,
        _ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let w = if available.width.is_finite() {
            available.width.max(0.0)
        } else {
            0.0
        };
        let h = if available.height.is_finite() {
            available.height.max(0.0)
        } else {
            0.0
        };
        kanesumi_core::Size::new(w, h)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        // 内容区随 Pane 宽度变化；子节点全部排进 content_rect。
        let content = self.content_rect(rect);
        for c in ctx.children() {
            // 先按内容区量测：非拉伸对齐的页面要靠期望尺寸定位（未量测 → 期望为 0）。
            ctx.measure_child(c, content.size);
            ctx.arrange_child(c, content);
        }
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    /// 插树前 / `edit` 改过 Pane 目标（未稳态）→ 请框架登记续帧（k-switch-init）。
    fn wants_anim(&self) -> bool {
        !self.pane.is_steady()
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, _dt: f64) {
        if !self.pane.is_steady() {
            // 统一动画入口：真实时钟推进 + 未稳态自动续帧（参 ELEMENT_TREE §帧调度）。
            ctx.animate(&mut self.pane);
            // 动画期内容区平移 → 重排；导航栏外观随 Pane 宽度变化 → 重画。
            ctx.invalidate_arrange();
            ctx.invalidate_paint();
        }
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, PointerButton};
        let rect = ctx.rect();
        match event {
            Event::PointerMove { pos } => {
                self.hover(rect, *pos);
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.toggle_hovered = false;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                // 旧 `handle_click` 需要排版引擎量 Top 模式标签宽；首帧之前引擎为 None。
                let action = match ctx.engine() {
                    Some(engine) => self.handle_click(engine, rect, *pos),
                    None => NavigationAction::None,
                };
                match action {
                    NavigationAction::Select(path) => {
                        if let Some(&i) = path.first() {
                            ctx.emit(NavigationItemInvoked(i));
                        }
                        ctx.invalidate_paint();
                        ctx.set_handled();
                    }
                    NavigationAction::Toggle => {
                        // 仅左侧模式真正翻转（Top 模式 handle_click 返回 Toggle 但不改状态）。
                        if self.mode.is_left() {
                            ctx.invalidate_arrange();
                            ctx.invalidate_paint();
                            ctx.request_anim_frame();
                        }
                        ctx.set_handled();
                    }
                    NavigationAction::None => {}
                }
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: self.header.clone(),
            value: self
                .selected
                .as_ref()
                .and_then(|p| p.first())
                .and_then(|i| self.items.get(*i))
                .map(|item| item.label.clone()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;

    #[test]
    fn start_aligned_page_gets_its_desired_size() {
        // 回归：arrange 前未量测子节点 → 非拉伸页面期望尺寸为 0（看不见）。
        let mut h = TestHarness::new(800.0, 600.0);
        let nav = h.tree.insert(h.root(), MetroNavigationView::new(vec![NavigationViewItem::new("一")]));
        let page = h.tree.insert_with(
            nav,
            Label::new("页面内容"),
            LayoutProps { h_align: Align::Start, v_align: Align::Start, ..LayoutProps::default() },
        );
        h.frame();
        let r = h.rect(page);
        assert!(r.size.width > 0.0 && r.size.height > 0.0, "页面应有内容尺寸 {r:?}");
        let content = h.tree.get::<MetroNavigationView>(nav).unwrap().content_rect(h.rect(nav));
        assert_eq!((r.origin.x, r.origin.y), (content.origin.x, content.origin.y));
    }
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::widgets::Label;
    use kanesumi_element::{Align, Insets, LayoutProps, WidgetId};

    fn harness(width: f32, height: f32) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(width, height);
        let nav = MetroNavigationView::new(vec![
            NavigationViewItem::with_icon("设置", "⚙"),
            NavigationViewItem::with_icon("外观", "◐"),
            NavigationViewItem::new("关于"),
        ]);
        let id = h.tree.insert_with(
            h.root(),
            nav,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn label_child_fills_content_rect() {
        let (mut h, id) = harness(800.0, 600.0);
        let label = h.tree.insert(id, Label::new("页面内容"));
        h.frame();
        let cr = h
            .tree
            .get::<MetroNavigationView>(id)
            .unwrap()
            .content_rect(h.rect(id));
        assert_eq!(h.rect(label), cr, "子节点应铺满 content_rect");
    }

    #[test]
    fn toggle_moves_child_but_keeps_it_inside() {
        let (mut h, id) = harness(800.0, 600.0);
        let label = h.tree.insert(id, Label::new("页面内容"));
        h.frame();
        let before = h.rect(label);
        let tr = h
            .tree
            .get::<MetroNavigationView>(id)
            .unwrap()
            .toggle_rect(h.rect(id));
        h.click_at(tr.center());
        h.settle();
        let after = h.rect(label);
        assert_ne!(before, after, "收起 Pane 后内容区应随之变化");
        assert!(after.size.width > before.size.width, "Pane 收窄后内容区变宽");
        let nr = h.rect(id);
        assert!(
            after.origin.x >= nr.origin.x && after.right() <= nr.right(),
            "子节点始终在 NavigationView 内：{after:?} ⊄ {nr:?}"
        );
        h.assert_contained();
    }

    #[test]
    fn click_item_invokes_with_index() {
        let (mut h, id) = harness(800.0, 600.0);
        let r = h.rect(id);
        let items = h
            .tree
            .get::<MetroNavigationView>(id)
            .unwrap()
            .top_item_rects(&h.engine, r);
        h.click_at(items[1].center());
        assert_eq!(
            h.take::<NavigationItemInvoked>(),
            vec![(id, NavigationItemInvoked(1))]
        );
        assert_eq!(
            h.tree.get::<MetroNavigationView>(id).unwrap().selected,
            Some(vec![1])
        );
    }

    /// 回归：内容区铺满后压在左侧类目上时，点类目被内容（会吞指针的控件）截走。
    /// 修好后内容从 pane 右起算，类目中心的点先命中导航控件本身。
    #[test]
    fn content_child_does_not_steal_category_clicks() {
        let (mut h, id) = harness(800.0, 600.0);
        // 内容子节点用一个会吞 PointerUp 的按钮铺满，模拟「滚动容器截走点击」。
        let content = h.tree.insert(id, crate::MetroButton::new("页面内容"));
        h.frame();
        let nr = h.rect(id);
        let items = h
            .tree
            .get::<MetroNavigationView>(id)
            .unwrap()
            .top_item_rects(&h.engine, nr);
        assert!(
            !h.rect(content).contains(items[1].center()),
            "内容子节点不应覆盖类目中心 {:?}",
            h.rect(content)
        );
        h.click_at(items[1].center());
        assert_eq!(
            h.take::<NavigationItemInvoked>(),
            vec![(id, NavigationItemInvoked(1))],
            "点类目应选中而非命中内容"
        );
        h.settle();
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(800.0, 600.0);
        assert_eq!(h.rect(id).size.width, 800.0);
        assert_eq!(h.rect(id).size.height, 600.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_with_child_still_passes_insurance_checks() {
        let (mut h, id) = harness(200.0, 150.0);
        let _label = h.tree.insert(id, Label::new("内容"));
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(800.0, 600.0);
        h.tree.set_enabled(id, false);
        h.frame();
        let r = h.rect(id);
        let items = h
            .tree
            .get::<MetroNavigationView>(id)
            .unwrap()
            .top_item_rects(&h.engine, r);
        h.click_at(items[1].center());
        assert!(h.take::<NavigationItemInvoked>().is_empty());
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

    fn nav() -> MetroNavigationView {
        MetroNavigationView::new(vec![
            NavigationViewItem::with_icon("设置", "⚙"),
            NavigationViewItem::with_icon("外观", "◐"),
            NavigationViewItem::new("关于"),
        ])
    }

    fn area() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    /// G-67：导航条目属正文类，字重必须跟随正文令牌，不得再写死 Normal 绕过令牌。
    #[test]
    fn item_style_uses_body_weight_token() {
        let item = MetroNavigationView::item_style();
        assert_eq!(item.weight, MetroTypography::metro().body.weight);
        assert_eq!(item.weight, kanesumi_core::typography::body_weight());
        // 字号 / 行高保持原值，本任务不改布局。
        assert_eq!(item.size, 14.0);
        assert_eq!(item.line_height, 20.0);
    }

    #[test]
    fn pane_widths() {
        let n = nav();
        assert_eq!(n.pane_width(), NAV_PANE_EXPANDED);
        let compact = MetroNavigationView {
            pane_expanded: false,
            ..nav()
        };
        // pane_expanded 仅为逻辑目标；宽度由 pane 动画进度决定（未推进 = 仍展开）。
        assert_eq!(compact.pane_width(), NAV_PANE_EXPANDED);
    }

    #[test]
    fn toggle_click_switches() {
        let Some(engine) = find_engine() else { return };
        let mut n = nav();
        let r = area();
        assert_eq!(
            n.handle_click(&engine, r, Point::new(20.0, 20.0)),
            NavigationAction::Toggle
        );
        assert!(!n.pane_expanded, "点 toggle 收窄（逻辑目标翻转）");
        assert!(n.is_animating(), "toggle 后进入展开/收窄动画");
        // 推进到稳态 → 宽度收敛到 48。
        for _ in 0..120 {
            n.update(1.0 / 60.0);
        }
        assert!(!n.is_animating());
        assert_eq!(n.pane_width(), NAV_PANE_COMPACT);
    }

    #[test]
    fn pane_width_interpolates_during_animation() {
        let Some(engine) = find_engine() else { return };
        let mut n = nav();
        let r = area();
        n.handle_click(&engine, r, Point::new(20.0, 20.0)); // 开始收窄
        n.update(0.1);
        let w = n.pane_width();
        assert!(
            w > NAV_PANE_COMPACT && w < NAV_PANE_EXPANDED,
            "中途宽度应插值，实际 {w}"
        );
        // 中断：再点 toggle 展开，应可反向推进。
        n.handle_click(&engine, r, Point::new(20.0, 20.0));
        assert!(n.pane_expanded);
        for _ in 0..120 {
            n.update(1.0 / 60.0);
        }
        assert_eq!(n.pane_width(), NAV_PANE_EXPANDED);
    }

    #[test]
    fn select_top_item() {
        let Some(engine) = find_engine() else { return };
        let mut n = nav();
        let r = area();
        let rects = n.top_item_rects(&engine, r);
        let second = rects[1];
        assert_eq!(
            n.handle_click(&engine, r, second.center()),
            NavigationAction::Select(vec![1])
        );
        assert_eq!(n.selected.as_deref(), Some([1].as_slice()));
    }

    #[test]
    fn content_rect_clears_expanded_pane() {
        // 展开（320）：内容从 pane 右 + Header Margin 起算，不再压住左侧类目。
        let n = nav();
        let r = area();
        let cr = n.content_rect(r);
        assert_eq!(cr.origin.x, NAV_PANE_EXPANDED + NAV_HEADER_MARGIN.0);
        assert_eq!(cr.size.width, r.size.width - NAV_PANE_EXPANDED - NAV_HEADER_MARGIN.0);
        assert_eq!(cr.origin.y, NAV_HEADER_MARGIN.1 + NAV_ITEM_H);
    }

    #[test]
    fn content_rect_follows_collapsed_rail() {
        // Left 收起（48）：内容随 pane 动画平移到 48 + Margin。
        let mut n = nav();
        n.set_pane_expanded(false);
        n.pane.jump_to(0.0);
        let cr = n.content_rect(area());
        assert_eq!(cr.origin.x, NAV_PANE_COMPACT + NAV_HEADER_MARGIN.0);
    }

    #[test]
    fn left_compact_opens_over_content() {
        // LeftCompact：常驻 48 图标栏推挤；打开（320）时 pane 覆盖，内容不移动。
        let mut n = MetroNavigationView {
            mode: NavigationPaneMode::LeftCompact,
            ..nav()
        };
        n.pane.jump_to(0.0);
        let closed = n.content_rect(area());
        n.set_pane_expanded(true);
        n.pane.jump_to(1.0);
        let opened = n.content_rect(area());
        assert_eq!(closed.origin.x, NAV_PANE_COMPACT + NAV_HEADER_MARGIN.0);
        assert_eq!(opened.origin.x, closed.origin.x, "覆盖不推挤内容");
    }

    #[test]
    fn left_minimal_content_full_width() {
        // LeftMinimal：仅汉堡键，内容从 Margin 起算；打开覆盖，内容不动。
        let mut n = MetroNavigationView {
            mode: NavigationPaneMode::LeftMinimal,
            ..nav()
        };
        n.pane.jump_to(0.0);
        let closed = n.content_rect(area());
        assert_eq!(closed.origin.x, NAV_HEADER_MARGIN.0);
        n.set_pane_expanded(true);
        n.pane.jump_to(1.0);
        assert_eq!(n.content_rect(area()).origin.x, NAV_HEADER_MARGIN.0);
    }

    #[test]
    fn top_mode_horizontal() {
        let Some(engine) = find_engine() else { return };
        let n = MetroNavigationView {
            mode: NavigationPaneMode::Top,
            ..nav()
        };
        let r = area();
        let rects = n.top_item_rects(&engine, r);
        // 横排：y 相同、x 递增
        assert_eq!(rects[0].origin.y, rects[1].origin.y);
        assert!(rects[1].origin.x > rects[0].origin.x);
        // 顶栏下 header
        assert_eq!(n.header_rect(r).origin.y, NAV_TOP_HEIGHT);
    }

    /// 长中文标签在 Top 模式下应扩宽（不被硬编码 56px 裁断）。
    #[test]
    fn top_mode_widens_for_long_labels() {
        let Some(engine) = find_engine() else { return };
        let n = MetroNavigationView {
            mode: NavigationPaneMode::Top,
            items: vec![
                NavigationViewItem::with_icon("设置", "⚙"),
                NavigationViewItem::with_icon("网络与共享中心", "◐"),
            ],
            ..MetroNavigationView::default()
        };
        let rects = n.top_item_rects(&engine, area());
        assert!(
            rects[1].size.width > rects[0].size.width,
            "长 label 应比短 label 宽：短={} 长={}",
            rects[0].size.width,
            rects[1].size.width
        );
        assert!(
            rects[0].size.width >= NAV_TOP_ITEM_MIN_W,
            "短 label 不低于 MIN"
        );
        assert!(
            rects[1].size.width <= NAV_TOP_ITEM_MAX_W + 0.01,
            "长 label 不超过 MAX（截断而非无限扩张）"
        );
    }

    /// 极端超长 label 被 MAX 夹紧。
    #[test]
    fn top_mode_clamps_to_max() {
        let Some(engine) = find_engine() else { return };
        let n = MetroNavigationView {
            mode: NavigationPaneMode::Top,
            items: vec![NavigationViewItem::with_icon(
                "这是一个非常非常非常非常非常长的导航标签用来测试上限夹紧",
                "◆",
            )],
            ..MetroNavigationView::default()
        };
        let rects = n.top_item_rects(&engine, area());
        assert!(
            (rects[0].size.width - NAV_TOP_ITEM_MAX_W).abs() < 0.01,
            "超长应夹到 MAX，实际 {}",
            rects[0].size.width
        );
    }

    #[test]
    fn indicator_emitted_on_selected() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut n = nav();
        n.selected = Some(vec![1]);
        let mut scene = Scene::default();
        n.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let indicator = scene.commands.iter().any(
            |c| matches!(c, SceneCommand::FillRect { color, .. } if *color == theme.colors.primary),
        );
        assert!(indicator, "选中项应有强调色指示条");
    }

    #[test]
    fn compact_hides_labels() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut n = nav();
        n.pane_expanded = false;
        n.pane.jump_to(0.0); // 收窄稳态（宽度 48、进度 0）
        let mut scene = Scene::default();
        n.render(&theme, &engine, area(), &mut scene);
        use kanesumi_canvas::SceneCommand;
        let texts = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Text { .. }))
            .count();
        // Compact：只留 icon 文本（2 个带图标项），无 label（"关于" 无图标 → 无文本）
        assert_eq!(texts, 2, "Compact 只显示图标，实际 {texts}");
    }
}
