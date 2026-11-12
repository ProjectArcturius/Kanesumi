// panel_row.rs —— 面板行控件（TopBar 面板 / 设置列表的公共行原语）。
//
// 供应门 §Ⅰ：TopBar 一格 125 处 `Rect::new`，其中 **48 处**集中在 `action_center` /
// `net_panel` / `panel_rows` 三个面板文件（`docs/research/2026-11-13_topbar_rect_audit.md`）——
// 它们画的是同一类东西：图标槽 + 主文字 + 次要文字 + 右侧控件，各自手算。
// 本控件把这一行定死：几何由量测得出、命中分「整行」与「右侧控件」两区、
// 状态走命名表，右侧控件复用 `MetroSwitch`（开关行）或画箭头（动作行）。
//
// 语义（参 `docs/INTERACTION_CANON.md`）：
// - 整行可点，高 ≥ 32pt（pointer 档）；
// - 右侧控件命中**优先**：点开关只切开关，不触发整行；
// - 禁用行不响应按下，文字降透明度。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

use crate::switch::MetroSwitch;

/// 行右侧放什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelRowKind {
    /// 动作行：右侧画箭头，点行 = 执行。
    Action,
    /// 开关行：右侧放开关，点开关只切开关。
    Toggle {
        /// 当前是否打开。
        on: bool,
    },
    /// 值行：右侧显示一段文字（只读）。
    Value {
        /// 值文本。
        text: String,
    },
}

/// 面板行数据。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelRow {
    /// 稳定 id。
    pub id: String,
    /// 图标名（可选）。
    pub icon: Option<String>,
    /// 主文字。
    pub title: String,
    /// 次要文字（主文字右侧、弱化色；空 = 不画）。
    pub secondary: String,
    /// 右侧内容。
    pub kind: PanelRowKind,
    /// 是否禁用。
    pub disabled: bool,
}

impl PanelRow {
    /// 动作行。
    pub fn action(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: None,
            title: title.into(),
            secondary: String::new(),
            kind: PanelRowKind::Action,
            disabled: false,
        }
    }

    /// 开关行。
    pub fn toggle(id: impl Into<String>, title: impl Into<String>, on: bool) -> Self {
        Self {
            kind: PanelRowKind::Toggle { on },
            ..Self::action(id, title)
        }
    }

    /// 值行。
    pub fn value(
        id: impl Into<String>,
        title: impl Into<String>,
        text: impl Into<String>,
    ) -> Self {
        Self {
            kind: PanelRowKind::Value { text: text.into() },
            ..Self::action(id, title)
        }
    }

    /// 带图标。
    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    /// 带次要文字。
    pub fn secondary(mut self, text: impl Into<String>) -> Self {
        self.secondary = text.into();
        self
    }

    /// 禁用。
    pub fn disabled(mut self, on: bool) -> Self {
        self.disabled = on;
        self
    }
}

/// 行状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelRowState {
    /// 静置。
    Rest,
    /// 悬停。
    Hovered,
    /// 按下。
    Pressed,
    /// 禁用。
    Disabled,
}

/// 命中结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelRowHit {
    /// 右侧控件（开关 / 值区）。
    Control,
    /// 整行。
    Row,
}

/// 一行几何（绝对坐标）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PanelRowGeometry {
    /// 整行矩形。
    pub row: Rect,
    /// 图标槽（无图标 = 零尺寸，位置与文字起点一致）。
    pub icon: Rect,
    /// 主文字。
    pub title: Rect,
    /// 次要文字（空 = 零尺寸）。
    pub secondary: Rect,
    /// 右侧控件区（动作行 = 箭头区；开关 / 值行 = 控件区）。
    pub control: Rect,
}

impl PanelRowGeometry {
    /// 量一行。`width` 给宽度，高由字体行高 + 上下内边距得出（**不写死**）。
    pub fn measure(engine: &TextEngine, row: &PanelRow, origin: Point, width: f32) -> Self {
        let style = body_style();
        let height = (style.line_height + 20.0).max(32.0);
        let rect = Rect::new(origin.x, origin.y, width.max(0.0), height);
        let pad = 12.0;
        let mut x = rect.origin.x + pad;
        let icon = if row.icon.is_some() {
            let r = Rect::new(x, rect.origin.y + (height - sizes::ICON_16) / 2.0, sizes::ICON_16, sizes::ICON_16);
            x += sizes::ICON_16 + 12.0;
            r
        } else {
            Rect::new(x, rect.origin.y, 0.0, 0.0)
        };
        // 右侧：动作行给箭头区 24，开关行给开关 36×20，值行按值文本量测。
        let control_w = match &row.kind {
            PanelRowKind::Action => 24.0,
            PanelRowKind::Toggle { .. } => 36.0,
            PanelRowKind::Value { text } => engine.measure(text, style.size) + 8.0,
        };
        let control = Rect::new(
            rect.right() - pad - control_w,
            rect.origin.y + (height - 20.0) / 2.0,
            control_w,
            20.0,
        );
        let text_w = (control.origin.x - 12.0 - x).max(0.0);
        let title = Rect::new(x, rect.origin.y + (height - style.line_height) / 2.0, text_w, style.line_height);
        let secondary = if row.secondary.is_empty() {
            Rect::new(title.right(), title.origin.y, 0.0, 0.0)
        } else {
            let w = (engine.measure(&row.secondary, style.size) + 8.0).min(text_w);
            Rect::new(title.right() - w, title.origin.y, w, style.line_height)
        };
        Self {
            row: rect,
            icon,
            title,
            secondary,
            control,
        }
    }

    /// 命中：`Control` 优先（右侧控件区）。
    pub fn hit(&self, pos: Point) -> Option<PanelRowHit> {
        if !contains(self.row, pos) {
            return None;
        }
        if contains(self.control, pos) {
            Some(PanelRowHit::Control)
        } else {
            Some(PanelRowHit::Row)
        }
    }
}

/// 面板行控件：状态 + 渲染。几何由 [`PanelRowGeometry::measure`] 给（外壳可缓存）。
#[derive(Debug, Clone)]
pub struct MetroPanelRow {
    row: PanelRow,
    hovered: bool,
    pressed: bool,
    switch: MetroSwitch,
}

impl MetroPanelRow {
    /// 新建。
    pub fn new(row: PanelRow) -> Self {
        Self {
            row,
            hovered: false,
            pressed: false,
            switch: MetroSwitch::new(),
        }
    }

    /// 数据。
    pub fn row(&self) -> &PanelRow {
        &self.row
    }

    /// 状态。
    pub fn state(&self) -> PanelRowState {
        if self.row.disabled {
            PanelRowState::Disabled
        } else if self.pressed {
            PanelRowState::Pressed
        } else if self.hovered {
            PanelRowState::Hovered
        } else {
            PanelRowState::Rest
        }
    }

    /// 悬停。
    pub fn hover(&mut self, on: bool) {
        self.hovered = on;
    }

    /// 按下（禁用行不响应）。
    pub fn press(&mut self) -> bool {
        if self.row.disabled {
            return false;
        }
        self.pressed = true;
        true
    }

    /// 松手：返回 `Some(true)` = 整行被点击，`Some(false)` = 右侧控件被点击，`None` = 取消。
    pub fn release(&mut self, hit: Option<PanelRowHit>) -> Option<bool> {
        let was = std::mem::take(&mut self.pressed);
        if !was || self.row.disabled {
            return None;
        }
        match hit {
            Some(PanelRowHit::Row) => Some(true),
            Some(PanelRowHit::Control) => Some(false),
            None => None,
        }
    }

    /// 渲染。`geo` 由调用方按当前宽度量好（同一帧内两处用同一份，避免两次量测）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, geo: &PanelRowGeometry, scene: &mut Scene) {
        let c = &theme.colors;
        let state = self.state();
        let alpha = if state == PanelRowState::Disabled { 0.55 } else { 1.0 };
        // 行底：悬停抬一档，按下用强调色。
        match state {
            PanelRowState::Hovered => scene.fill_rect(c.surface_variant, geo.row),
            PanelRowState::Pressed => scene.fill_rect(c.primary, geo.row),
            _ => {}
        }
        if self.row.icon.is_some() {
            scene.fill_rect(c.divider.with_alpha(c.divider.a * alpha), geo.icon);
        }
        let fg = c.on_surface.with_alpha(c.on_surface.a * alpha);
        scene.label(
            self.row.title.clone(),
            geo.title,
            fg,
            body_style(),
            TextAlign::Left,
        );
        if !self.row.secondary.is_empty() {
            scene.label(
                self.row.secondary.clone(),
                geo.secondary,
                c.on_surface_variant.with_alpha(c.on_surface_variant.a * alpha),
                body_style(),
                TextAlign::Right,
            );
        }
        match &self.row.kind {
            PanelRowKind::Action => {
                // 箭头：右下角两笔短斜线用三角形代替（直角几何，不用圆角）。
                let a = geo.control;
                let cx = a.origin.x + a.size.width / 2.0;
                let cy = a.origin.y + a.size.height / 2.0;
                scene.triangle(
                    Point::new(cx - 3.0, cy - 6.0),
                    Point::new(cx + 3.0, cy),
                    Point::new(cx - 3.0, cy + 6.0),
                    c.on_surface_variant.with_alpha(c.on_surface_variant.a * alpha),
                );
            }
            PanelRowKind::Toggle { on } => {
                self.switch
                    .render(theme, engine, geo.control, scene);
                let _ = on;
            }
            PanelRowKind::Value { text } => {
                scene.label(
                    text.clone(),
                    geo.control,
                    c.on_surface_variant.with_alpha(c.on_surface_variant.a * alpha),
                    body_style(),
                    TextAlign::Right,
                );
            }
        }
    }
}

fn body_style() -> TextStyle {
    MetroTypography::metro().body_medium
}

fn contains(r: Rect, p: Point) -> bool {
    r.size.width > 0.0
        && r.size.height > 0.0
        && p.x >= r.origin.x
        && p.x < r.right()
        && p.y >= r.origin.y
        && p.y < r.bottom()
}
