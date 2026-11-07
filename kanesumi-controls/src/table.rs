// table.rs —— 表格行：可交互列表行（列定义 / 行状态 / 展开三角 / 逐列命中 / 拖拽阈值）。
//
// 来源：`librarian/src/window/views/list.rs`（847 行整块自绘）。它与设置页的行不是一回事：
// 设置页一行是「标签在上、控件在下」的纵向块，表格行是**横向多列 + 可交互**（选择态 / 展开
// 三角 / 逐列命中 / 拖拽阈值 / 右键）。套 `Form::list_item` 会把这些交互全部丢掉，故单列一节。
// 参 `docs/CONTROL_SPEC.md` §51、`docs/INTERACTION_CANON.md` §Ⅱ（拖拽阈值 4pt）。
//
// 本文件只做**行**与**列几何**；虚拟化容器仍是 `ItemsRepeater`，横向滚动与键盘上下移动归
// 容器（行不知道自己的兄弟），故行内只处理自身矩形内能判定的交互。

use std::sync::Arc;

use kanesumi_canvas::{Icon, Scene, TextAlign, TextOverflow};
use kanesumi_core::typography::FontWeight;
use kanesumi_core::{Point, Rect, Size, TextStyle, ThemeColor, sizes};
use kanesumi_element::{
    AccessInfo, AccessRole, Event, EventCtx, MeasureCtx, PaintCtx, PointerButton, Widget,
};

use crate::state::{ControlState, control_state};

// ── 语义尺寸（只引用尺寸表，不在此另立值）──────────────────────────────────

/// 行高。
pub const ROW_H: f32 = sizes::TABLE_ROW_H;
/// 左右内边距。
pub const PAD_H: f32 = sizes::TABLE_PAD_H;
/// 行首三角 / 图标 / 文本之间的间距。
pub const GAP: f32 = sizes::TABLE_GAP;
/// 展开三角框宽。
pub const TRI_W: f32 = sizes::TABLE_TRI_W;
/// 行首图标档。
pub const ICON: f32 = sizes::TABLE_ICON;
/// 子项每层缩进。
pub const INDENT: f32 = sizes::TABLE_INDENT;

/// 拖拽阈值（pt）：按下后位移超过它即视为拖动，不是点选。参 `INTERACTION_CANON.md` §Ⅱ。
pub const DRAG_THRESHOLD: f32 = kanesumi_core::interaction::DRAG_THRESHOLD_PX;

/// 单元格文本 14/20（librarian `spec.rs:120` 的 `s_body`）。
fn s_cell() -> TextStyle {
    TextStyle::new(14.0, 20.0, FontWeight::Normal)
}

/// 单元格右侧内缩：两列紧邻时文字会粘在一起（「12 KB」+「Markdown 文档」读成一串），
/// 故每列文本右侧留一个图文间距。命中仍按整列矩形算，缩进只影响绘制。
const CELL_PAD_R: f32 = GAP;

/// 列内文本盒：垂直居中一行，右侧留出列间距。
fn cell_box(rect: Rect, style: TextStyle) -> Rect {
    let inner = Rect::new(
        rect.origin.x,
        rect.origin.y,
        (rect.size.width - CELL_PAD_R).max(0.0),
        rect.size.height,
    );
    text_box(inner, style)
}

// ── 列 ──────────────────────────────────────────────────────────────────────

/// 列宽策略。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
    /// 固定逻辑像素宽。
    Px(f32),
    /// 吃余量：剩余宽度在所有 `Star` 列之间平分（一列都没有 `Star` 时右侧就是留白）。
    Star,
}

/// 列内文本对齐（单元格文本按列矩形对齐，列内额外的内缩由列宽自己带）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnAlign {
    #[default]
    Start,
    Center,
    End,
}

impl ColumnAlign {
    fn text_align(self) -> TextAlign {
        match self {
            ColumnAlign::Start => TextAlign::Left,
            ColumnAlign::Center => TextAlign::Center,
            ColumnAlign::End => TextAlign::Right,
        }
    }
}

/// 一列的定义（表头与行共用同一份）。
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    /// 表头文字（空串 = 该列表头不写字）。
    pub header: String,
    /// 列宽策略。
    pub width: ColumnWidth,
    /// 列内文本对齐。
    pub align: ColumnAlign,
}

impl Column {
    pub fn new(header: &str, width: ColumnWidth) -> Self {
        Self {
            header: header.to_string(),
            width,
            align: ColumnAlign::Start,
        }
    }

    pub fn align(mut self, align: ColumnAlign) -> Self {
        self.align = align;
        self
    }
}

/// 一组列：所有行与表头共享。内部 `Arc`，每行克隆只加引用计数，不复制表头字符串。
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnSet(Arc<[Column]>);

impl ColumnSet {
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self(columns.into_iter().collect())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Column> {
        self.0.get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Column> {
        self.0.iter()
    }
}

/// 列矩形：把 `area` 按列宽策略从左到右切开（`Star` 平分剩余）。
///
/// 绘制、命中、表头排序箭头都消费这一个函数 —— 「同一布局产物」是元素树的硬契约
/// （参 `COMPOSITION.md`），行内绝不允许再写第二套 `x` 累加。
pub fn column_rects(columns: &ColumnSet, area: Rect) -> Vec<Rect> {
    if columns.is_empty() {
        return Vec::new();
    }
    let fixed: f32 = columns
        .iter()
        .map(|c| match c.width {
            ColumnWidth::Px(w) => w.max(0.0),
            ColumnWidth::Star => 0.0,
        })
        .sum();
    let stars = columns
        .iter()
        .filter(|c| c.width == ColumnWidth::Star)
        .count() as f32;
    let star_w = if stars > 0.0 {
        ((area.size.width - fixed) / stars).max(0.0)
    } else {
        0.0
    };
    let mut x = area.origin.x;
    columns
        .iter()
        .map(|c| {
            let w = match c.width {
                ColumnWidth::Px(w) => w.max(0.0),
                ColumnWidth::Star => star_w,
            };
            let r = Rect::new(x, area.origin.y, w, area.size.height);
            x += w;
            r
        })
        .collect()
}

// ── 行投出的动作 ─────────────────────────────────────────────────────────────
//
// 行的每种交互都要带回**身份**（行号）与**位置信息**，因为点哪一行的哪一列决定了后果：
// 名称列点 = 选择并打开预览，大小列点 = 只选择；右键要给菜单锚点；三角只发展开。
// 只投一个不带载荷的 `ButtonClicked` 是分不出来的（与 `Form::row_button` 同一教训）。

/// 行被点：`column` = 命中的列号（点行首三角不会投本动作，改投 [`TableRowToggled`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowClicked {
    pub index: usize,
    pub column: usize,
}

/// 行被双击：打开（目录进入 / 文件交给默认应用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowActivated(pub usize);

/// 行上按了右键：`pos` = 表面坐标的松开点，直接当菜单锚点用。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableRowContextRequested {
    pub index: usize,
    pub pos: Point,
}

/// 展开三角被点：调用方改自己的展开集合，再用 `tree.edit::<MetroTableRow, _>` 同步 `expanded`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRowToggled(pub usize);

/// 按下后位移超过 [`DRAG_THRESHOLD`] 再松开 —— 这是拖动，**不是**点选。
///
/// 拖动不投 [`TableRowClicked`]：否则「把文件拖到侧栏」会顺带把选择改掉，librarian 现状正是
/// 靠这条判据避免误选（参 `INTERACTION_CANON.md` §Ⅱ）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TableRowDragged {
    pub index: usize,
    pub pos: Point,
}

// ── 行 ──────────────────────────────────────────────────────────────────────

/// 表格行：横向多列 + 行首展开三角 / 图标，整行可点，三角与行身分开命中。
#[derive(Debug, Clone)]
pub struct MetroTableRow {
    index: usize,
    columns: ColumnSet,
    cells: Vec<String>,
    icon: Option<Icon>,
    indent: u8,
    expandable: bool,
    expanded: bool,
    selected: bool,
    /// 按下点（判定拖拽阈值用，也供不带坐标的 `Click` 定位列）。
    press: Option<Point>,
    /// 本次按下已进入拖拽（松开时投 [`TableRowDragged`]）。
    dragging: bool,
    /// 拖拽后的那次 `Click` 要吞掉（框架在 `PointerUp` 之后还会合成一次 `Click`）。
    suppress_click: bool,
}

impl MetroTableRow {
    /// 建一行：`cells` 与 `columns` 一一对应，不足按空串补、多出的忽略。
    pub fn new(
        index: usize,
        columns: ColumnSet,
        cells: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            index,
            columns,
            cells: cells.into_iter().map(Into::into).collect(),
            icon: None,
            indent: 0,
            expandable: false,
            expanded: false,
            selected: false,
            press: None,
            dragging: false,
            suppress_click: false,
        }
    }

    /// 行首图标（16 档，已光栅好的位图）。
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// 缩进层级（每层 [`INDENT`]）；目录树展开后的子项用。
    pub fn with_indent(mut self, indent: u8) -> Self {
        self.indent = indent;
        self
    }

    /// 可展开：行首出现三角，`expanded` 决定朝向。**不可展开的行同样占三角槽**，
    /// 否则树里「有子项 / 无子项」两种行的名称左缘会错开（librarian `name_parts` 同款）。
    pub fn with_expandable(mut self, expanded: bool) -> Self {
        self.expandable = true;
        self.expanded = expanded;
        self
    }

    /// 当前项：画选中底（[`ThemeColor::SelectionTint`]）。
    pub fn with_selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn index(&self) -> usize {
        self.index
    }

    /// 改展开态（调用方收到 [`TableRowToggled`] 后用）。
    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    /// 改选中态。
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    /// 列矩形（已扣左右内边距）。绘制与命中都走它。
    fn rects(&self, rect: Rect) -> Vec<Rect> {
        column_rects(&self.columns, self.inner(rect))
    }

    fn inner(&self, rect: Rect) -> Rect {
        Rect::new(
            rect.origin.x + PAD_H,
            rect.origin.y,
            (rect.size.width - PAD_H * 2.0).max(0.0),
            rect.size.height,
        )
    }

    /// 名称列（第 0 列）内部几何：缩进 → 三角槽 → 图标槽 → 文本。
    ///
    /// 两个槽**恒占位**，不因该行无三角 / 无图标而省掉：一列里混排时左侧对齐比省 20pt 重要。
    fn name_parts(&self, rect: Rect, cols: &[Rect]) -> (Rect, Rect, Rect) {
        let col = cols.first().copied().unwrap_or(rect);
        let mut x = col.origin.x + self.indent as f32 * INDENT;
        let tri = Rect::new(
            x,
            col.origin.y + (col.size.height - TRI_W) / 2.0,
            TRI_W.min((col.right() - x).max(0.0)),
            TRI_W,
        );
        x += TRI_W + GAP;
        let icon = Rect::new(
            x,
            col.origin.y + (col.size.height - ICON) / 2.0,
            ICON.min((col.right() - x).max(0.0)),
            ICON,
        );
        x += ICON + GAP;
        let text = Rect::new(x, col.origin.y, (col.right() - x).max(0.0), col.size.height);
        (tri, icon, text)
    }

    /// `pos` 落在第几列（列外夹到两端）。
    fn column_at(&self, rect: Rect, pos: Point) -> usize {
        index_at(&self.rects(rect), pos.x)
    }

    /// `pos` 是否落在展开三角上（只有可展开的行才有这块命中区）。
    fn tri_hit(&self, rect: Rect, pos: Point) -> bool {
        if !self.expandable {
            return false;
        }
        let (tri, ..) = self.name_parts(rect, &self.rects(rect));
        pos.x >= tri.origin.x && pos.x < tri.right() && rect.contains(pos)
    }
}

/// 文本行盒：在给定矩形里垂直居中一行（`Scene::label` 是顶对齐的）。
fn text_box(rect: Rect, style: TextStyle) -> Rect {
    let h = style.line_height.min(rect.size.height);
    Rect::new(
        rect.origin.x,
        rect.origin.y + (rect.size.height - h) / 2.0,
        rect.size.width,
        h,
    )
}

/// `x` 落在第几列（列外夹到两端）。行与表头共用。
fn index_at(cols: &[Rect], x: f32) -> usize {
    if cols.is_empty() {
        return 0;
    }
    for (i, c) in cols.iter().enumerate() {
        if x < c.right() {
            return i;
        }
    }
    cols.len() - 1
}

/// 展开三角：收起 = pan-end（朝右），展开 = pan-down（朝下）。几何照抄 librarian 现状
/// （`list.rs` 的 10×10 实心三角落在 12 宽槽里居中）。
fn paint_tri(scene: &mut Scene, slot: Rect, expanded: bool, color: kanesumi_core::Color) {
    let glyph = 10.0;
    let x = slot.origin.x + (slot.size.width - glyph).max(0.0) / 2.0;
    let cy = slot.origin.y + slot.size.height / 2.0;
    if expanded {
        scene.triangle(
            Point::new(x, cy - 2.0),
            Point::new(x + glyph, cy - 2.0),
            Point::new(x + glyph / 2.0, cy + 4.0),
            color,
        );
    } else {
        scene.triangle(
            Point::new(x + 2.0, cy - 5.0),
            Point::new(x + 2.0, cy + 5.0),
            Point::new(x + 8.0, cy),
            color,
        );
    }
}

impl Widget for MetroTableRow {
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        // 行宽由容器给（虚拟化只决定可见行，不决定列宽），高恒为行高档。
        Size::new(available.width.max(0.0), ROW_H)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let rect = ctx.rect();
        let theme = *ctx.theme();
        let state = control_state(ctx.state());
        let cols = self.rects(rect);
        let (tri, icon_rect, text_rect) = self.name_parts(rect, &cols);

        // 容器语义：本行是自己的边界，半行滚出视口时文字不得画到行外。
        scene.push_clip(rect);

        if self.selected {
            scene.fill_rect(ThemeColor::SelectionTint.resolve(&theme), rect);
        } else if matches!(state, ControlState::Hovered | ControlState::Pressed) {
            // 悬停 = 中性高亮（与 `MetroList` 同值，一手源 `SystemControlHighlightListLowBrush`）。
            scene.fill_rect(theme.indication.hover_tint, rect);
        }

        let ink = ThemeColor::OnSurface.resolve(&theme);
        // 选中行整行压深色底，次级色在强调色上会糊掉 —— 选中时统一用主前景
        // （librarian 现状同款：选中行一律白字）。
        let ink2 = if self.selected {
            ink
        } else {
            ThemeColor::OnSurfaceVariant.resolve(&theme)
        };

        if self.expandable {
            paint_tri(scene, tri, self.expanded, ink2);
        }
        if let Some(icon) = &self.icon {
            scene.image(icon, icon_rect, Some(ink2));
        }

        // 单元格：第 0 列吃三角与图标之后的剩余宽，其余列按列矩形对齐。
        let style = s_cell();
        let first = self.cells.first().map(String::as_str).unwrap_or("");
        scene.label(
            first.to_string(),
            cell_box(text_rect, style),
            ink,
            style,
            TextAlign::Left,
        );
        for (i, col) in cols.iter().enumerate().skip(1) {
            let text = self.cells.get(i).map(String::as_str).unwrap_or("");
            if text.is_empty() {
                continue;
            }
            let align = self.columns.get(i).map(|c| c.align).unwrap_or_default();
            scene.text_with_options(
                text.to_string(),
                cell_box(*col, style),
                ink2,
                style,
                align.text_align(),
                false,
                Some(1),
                TextOverflow::Ellipsis,
            );
        }

        scene.pop_clip();
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        use kanesumi_element::Key;
        match event {
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                self.press = Some(*pos);
                self.dragging = false;
            }
            Event::PointerMove { pos } => {
                // 拖拽判定只看**位移**，不看时长：超过 4pt 就是拖动（INTERACTION_CANON §Ⅱ）。
                if let Some(p) = self.press {
                    let dx = pos.x - p.x;
                    let dy = pos.y - p.y;
                    if !self.dragging && (dx * dx + dy * dy).sqrt() > DRAG_THRESHOLD {
                        self.dragging = true;
                    }
                }
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } if self.dragging => {
                // 已在拖拽：松开即投拖动，并标记吞掉随后那次合成 `Click`。
                self.suppress_click = true;
                ctx.emit(TableRowDragged {
                    index: self.index,
                    pos: *pos,
                });
                self.press = None;
            }
            Event::Click => {
                // 拖动后的那次合成点击不是点选：吞掉它，别顺手改掉选择。
                if self.suppress_click {
                    self.suppress_click = false;
                    self.press = None;
                    ctx.set_handled();
                    return;
                }
                let rect = ctx.rect();
                let pos = self.press.take().or_else(|| ctx.pointer());
                if let Some(pos) = pos {
                    if self.tri_hit(rect, pos) {
                        // 三角只展开，不选择 —— 「整行可点 + 行首按钮」的同一类分工。
                        self.expanded = !self.expanded;
                        ctx.emit(TableRowToggled(self.index));
                        ctx.set_handled();
                        return;
                    }
                    let column = self.column_at(rect, pos);
                    ctx.emit(TableRowClicked {
                        index: self.index,
                        column,
                    });
                    ctx.set_handled();
                }
            }
            Event::DoubleTapped { .. } => {
                ctx.emit(TableRowActivated(self.index));
                ctx.set_handled();
            }
            Event::ContextRequested { pos } => {
                ctx.emit(TableRowContextRequested {
                    index: self.index,
                    pos: *pos,
                });
                ctx.set_handled();
            }
            Event::KeyDown {
                key: Key::Enter,
                repeat: false,
                ..
            } => {
                ctx.emit(TableRowActivated(self.index));
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn has_visual_states(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::ListItem,
            name: self.cells.join("，"),
            value: None,
            checked: None,
        })
    }
}

// ── 表头 ────────────────────────────────────────────────────────────────────

/// 表头高。
pub const HEADER_H: f32 = sizes::TABLE_HEADER_H;

/// 点了列头：`column` 列要按 `ascending` 排序。
///
/// 行序由调用方的数据层决定（本层不持有集合，不知道排序结果），故这里只投**意图**。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortRequested {
    pub column: usize,
    pub ascending: bool,
}

/// 表头：列标题 + 排序箭头，点列头切排序（同一列再点反序，换列从升序起步）。
///
/// 与行共用 [`ColumnSet`] 与 [`column_rects`]，故表头与行的列边界永远一致 —— librarian 现状
/// 是把列几何算两遍（`Cols` + 表头自己的 `divider_at`），列宽一改就得两处同改。
#[derive(Debug, Clone)]
pub struct MetroTableHeader {
    columns: ColumnSet,
    sort: Option<(usize, bool)>,
}

impl MetroTableHeader {
    pub fn new(columns: ColumnSet) -> Self {
        Self {
            columns,
            sort: None,
        }
    }

    /// 当前排序列与方向（画箭头用；数据层排完序后回调同步即可）。
    pub fn with_sort(mut self, column: usize, ascending: bool) -> Self {
        self.sort = Some((column, ascending));
        self
    }

    pub fn set_sort(&mut self, column: usize, ascending: bool) {
        self.sort = Some((column, ascending));
    }

    fn rects(&self, rect: Rect) -> Vec<Rect> {
        column_rects(
            &self.columns,
            Rect::new(
                rect.origin.x + PAD_H,
                rect.origin.y,
                (rect.size.width - PAD_H * 2.0).max(0.0),
                rect.size.height,
            ),
        )
    }
}

/// 排序箭头：升序朝上、降序朝下（10 档实心三角）。
fn paint_arrow(scene: &mut Scene, slot: Rect, ascending: bool, color: kanesumi_core::Color) {
    let g = sizes::TABLE_SORT_TRI;
    let cx = slot.origin.x + slot.size.width / 2.0;
    let cy = slot.origin.y + slot.size.height / 2.0;
    if ascending {
        scene.triangle(
            Point::new(cx - g / 2.0, cy + g / 2.0),
            Point::new(cx + g / 2.0, cy + g / 2.0),
            Point::new(cx, cy - g / 2.0),
            color,
        );
    } else {
        scene.triangle(
            Point::new(cx - g / 2.0, cy - g / 2.0),
            Point::new(cx + g / 2.0, cy - g / 2.0),
            Point::new(cx, cy + g / 2.0),
            color,
        );
    }
}

impl Widget for MetroTableHeader {
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        Size::new(available.width.max(0.0), HEADER_H)
    }

    /// 表头文本 12/20 次级色（librarian `spec.rs:126` 的 `s_caption`）。
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let rect = ctx.rect();
        let theme = *ctx.theme();
        let engine = ctx.engine();
        let style = TextStyle::new(12.0, 20.0, FontWeight::Normal);
        let ink = ThemeColor::OnSurfaceVariant.resolve(&theme);
        scene.push_clip(rect);
        for (i, col) in self.rects(rect).iter().enumerate() {
            let Some(c) = self.columns.get(i) else {
                continue;
            };
            if c.header.is_empty() {
                continue;
            }
            let avail = (col.size.width - CELL_PAD_R).max(0.0);
            let w = engine.measure(&c.header, style.size).min(avail);
            let x0 = match c.align {
                ColumnAlign::Start => col.origin.x,
                ColumnAlign::Center => col.origin.x + (avail - w) / 2.0,
                ColumnAlign::End => col.origin.x + avail - w,
            };
            let box_rect = Rect::new(col.origin.x, col.origin.y, avail, col.size.height);
            scene.label(
                c.header.clone(),
                text_box(box_rect, style),
                ink,
                style,
                c.align.text_align(),
            );
            if let Some((_, ascending)) = self.sort.filter(|(s, _)| *s == i) {
                // 箭头紧跟标题文字（librarian「排序小三角 10（表头名称后）」），不放到列尾。
                let g = sizes::TABLE_SORT_TRI;
                let ax = (x0 + w + 4.0).min(col.right() - g);
                let slot = Rect::new(ax, col.origin.y + (col.size.height - g) / 2.0, g, g);
                paint_arrow(scene, slot, ascending, ink);
            }
        }
        scene.pop_clip();
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            let rect = ctx.rect();
            let Some(pos) = ctx.pointer() else { return };
            let column = index_at(&self.rects(rect), pos.x);
            // 同一列再点 = 反序；换列 = 从升序起步（与 librarian 表头现状一致）。
            let ascending = match self.sort {
                Some((c, asc)) if c == column => !asc,
                _ => true,
            };
            self.sort = Some((column, ascending));
            ctx.emit(SortRequested { column, ascending });
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        false
    }

    fn has_visual_states(&self) -> bool {
        true
    }
}
