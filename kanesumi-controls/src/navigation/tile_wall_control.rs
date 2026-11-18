// tile_wall_control.rs —— 磁贴墙的**状态机**（几何复用 `navigation::tile_wall`）。
//
// 供应门 §Ⅰ：磁贴壁这一格「应用自己顶」——launcher 自造了 `tile_wall.rs`(2994 行) /
// `all_apps.rs`(1571) / `tiles.rs`(818)。控件库里有单块磁贴（`MetroTile`）与墙的纯几何
// （`navigation::tile_wall::TileWall::layout`），缺的是**拖动 / 尺寸档 / 键盘 / 索引跳转**
// 这一层状态。本文件补它，仍然不画磁贴本体（那归 `MetroTile`）。
//
// 语义（参 `docs/TILES_DESIGN.md`、`docs/INTERACTION_CANON.md`）：
// - 拖动必须**越阈值**才开始（阈值取交互正典，未写入即记临时值）；未越阈值松手 = 激活；
// - 拖动期间给出落点（组, 组内下标），松手才真正移动；拖出墙 = 取消；
// - 尺寸档循环：Small → Wide → Large → Small（4×2 的 ExtraLarge 不进循环）；
// - 键盘：左右在组内走、走到头进下一组；Enter 激活；
// - 字母索引跳转：返回该组顶部的滚动偏移（外壳据此滚过去，控件不做动画）。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::typography::{base_weight, TextStyle};
use kanesumi_core::{sizes, MetroTheme, Point, Rect};

use crate::navigation::tile_wall::{
    layout as wall_layout, layout_with_overscan as wall_layout_overscan,
    layout_without as wall_layout_without, TileWallGroup, TileWallGroupLayout, TileWallInput,
    TileWallLayout, TileWallMetrics, TileWallSize, TileWallSlot, TileWallTile, TILE_TITLE_GAP,
    TILE_TITLE_H,
};

/// 拖动阈值（逻辑像素）——引用交互正典的令牌，不在此另立门户。
pub const DRAG_THRESHOLD: f32 = kanesumi_core::interaction::DRAG_THRESHOLD_PX;

/// 墙里的一块磁贴（数据）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallTile {
    /// 稳定 id。
    pub id: String,
    /// 标签（磁贴上的字）。
    pub label: String,
    /// 尺寸档。
    pub size: TileWallSize,
}

impl WallTile {
    /// 新建。
    pub fn new(id: impl Into<String>, label: impl Into<String>, size: TileWallSize) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            size,
        }
    }
}

/// 一组。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WallGroup {
    /// 组名（标题带）。
    pub title: String,
    /// 组内磁贴。
    pub tiles: Vec<WallTile>,
}

impl WallGroup {
    /// 新建。
    pub fn new(title: impl Into<String>, tiles: Vec<WallTile>) -> Self {
        Self {
            title: title.into(),
            tiles,
        }
    }
}

/// 松手结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallRelease {
    /// 未越阈值 → 当成点击（激活该磁贴）。
    Activated(String),
    /// 越阈值且落点有效 → 移动。
    Moved {
        /// 源组。
        from_group: usize,
        /// 源组内下标。
        from_index: usize,
        /// 目标组。
        to_group: usize,
        /// 目标组内下标。
        to_index: usize,
    },
    /// 取消（拖出墙 / 无目标）。
    Cancelled,
}

/// 拖动落点（**基准布局**坐标：已摘除被拖项）。
///
/// 对照 launcher `shell/tiles.rs::DropTarget`；`InGroup` 的 `index` 是摘除被拖项之后的
/// 插入位，直接对基准序列做 `insert` 即落到指针所指的格子（不需要再按源 / 基准换算）。
/// 与旧 [`MetroTileWall::release`] 的区别：那个按「被拖项还在原位」的静息布局判落点，
/// 指针在自家磁贴上就是 `Cancelled`，落点差一格；新入口走 [`MetroTileWall::drop_target_at`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WallDropTarget {
    /// 落点就是原位：松手等于激活该磁贴，不换序。
    Activated(String),
    /// 插到 `group` 组内第 `index` 位。
    InGroup {
        /// 目标组（基准布局下标）。
        group: usize,
        /// 组内插入位（基准布局下标）。
        index: usize,
    },
    /// 在 `at` 处新建一组。
    NewGroup {
        /// 新组插到第几组之前（`== 组数` 即末尾）。
        at: usize,
    },
}

/// 拖动状态。
#[derive(Debug, Clone, Copy, PartialEq)]
enum DragState {
    Idle,
    /// 已按下，尚未越阈值。
    Armed {
        group: usize,
        index: usize,
        from: Point,
    },
    /// 已越阈值。
    Dragging {
        group: usize,
        index: usize,
        over: Option<(usize, usize)>,
    },
}

/// 磁贴墙控件。
#[derive(Debug, Clone)]
pub struct MetroTileWall {
    groups: Vec<WallGroup>,
    layout: TileWallLayout,
    /// 显式启用的提供方响应式度量；`None` = 沿用旧缺省 `BASE`。
    compat_metrics: Option<TileWallMetrics>,
    /// 当前滚动偏移（每次重排后写回夹紧值）。
    scroll: f32,
    /// 视口外预取的组列数；0 = 只回视口内几何（旧行为）。
    overscan_cols: usize,
    hover: Option<(usize, usize)>,
    focus: Option<(usize, usize)>,
    drag: DragState,
}

impl Default for MetroTileWall {
    fn default() -> Self {
        Self::new()
    }
}

impl MetroTileWall {
    /// 空墙。
    pub fn new() -> Self {
        Self {
            groups: Vec::new(),
            layout: wall_layout(&TileWallInput::new(
                Vec::new(),
                Rect::new(0.0, 0.0, 0.0, 0.0),
            )),
            compat_metrics: None,
            scroll: 0.0,
            overscan_cols: 0,
            hover: None,
            focus: None,
            drag: DragState::Idle,
        }
    }

    /// 启用 / 关闭提供方兼容度量。`Some` 时重排立即改用该表，`None` 恢复旧缺省 `BASE`。
    ///
    /// 这是兼容档的唯一开关：不调用时 `set_groups` / `relayout` 的行为与旧版逐字相同。
    pub fn set_compat_metrics(&mut self, metrics: Option<TileWallMetrics>) {
        self.compat_metrics = metrics.map(TileWallMetrics::normalized);
        let viewport = self.layout.viewport;
        self.relayout(viewport);
    }

    /// 本帧实际使用的度量（兼容档或旧缺省）。
    pub fn metrics(&self) -> TileWallMetrics {
        self.compat_metrics.unwrap_or(TileWallMetrics::BASE)
    }

    /// 设置滚动偏移并重排。夹到 `[0, max_scroll]`，非有限值归零。
    ///
    /// 命中与绘制消费同一份 `layout`：磁贴 `rect` / `clip` / `hit` 一起平移，控件不做
    /// 「按指针平移去模拟滚动」那套补偿（2026-11-18 卡的反例就是命中与绘制各算一套）。
    pub fn set_scroll(&mut self, scroll: f32) {
        // 负偏移在布局层也会夹回 0，这里先归零只是让写回值从一开始就是规范形。
        self.scroll = if scroll.is_finite() {
            scroll.max(0.0)
        } else {
            0.0
        };
        let viewport = self.layout.viewport;
        self.relayout(viewport);
    }

    /// 当前滚动偏移（重排后的夹紧值）。
    pub fn scroll(&self) -> f32 {
        self.scroll
    }

    /// 设置视口外预取的组列数（1 列 = 组块宽 + 组间距）。0 = 只回视口内几何（旧行为）。
    pub fn set_overscan_cols(&mut self, cols: usize) {
        self.overscan_cols = cols;
        let viewport = self.layout.viewport;
        self.relayout(viewport);
    }

    /// 当前预取列数。
    pub fn overscan_cols(&self) -> usize {
        self.overscan_cols
    }

    /// 替换分组并重排。
    pub fn set_groups(&mut self, groups: Vec<WallGroup>, viewport: Rect) {
        self.groups = groups;
        self.drag = DragState::Idle;
        if self.hover.is_some_and(|(g, i)| !self.exists(g, i)) {
            self.hover = None;
        }
        if self.focus.is_some_and(|(g, i)| !self.exists(g, i)) {
            self.focus = None;
        }
        self.relayout(viewport);
    }

    /// 分组（只读）。
    pub fn groups(&self) -> &[WallGroup] {
        &self.groups
    }

    /// 布局。
    pub fn layout(&self) -> &TileWallLayout {
        &self.layout
    }

    /// 按视口重排。兼容档启用时消费 `set_compat_metrics` 传入的响应式度量；
    /// 滚动偏移与预取列数沿用当前设置。重排后把夹紧后的滚动值写回，调用方读
    /// [`Self::scroll`] 得到的永远是实际生效值。
    pub fn relayout(&mut self, viewport: Rect) {
        let mut input = TileWallInput::new(self.wall_groups(), viewport);
        input.metrics = self.metrics();
        input.scroll = self.scroll;
        self.layout = if self.overscan_cols == 0 {
            wall_layout(&input)
        } else {
            wall_layout_overscan(&input, self.overscan_cols)
        };
        self.scroll = self.layout.scroll;
    }

    /// 摘除一枚磁贴后的基准布局（全量、不按视口过滤），供拖动落点判定与预览。
    ///
    /// 组索引与当前模型一致（摘空的组保留）；`scroll` / 度量 / 视口沿用当前值。
    /// 越界下标返回未摘除的全量布局，不 panic。
    pub fn layout_without(&self, group: usize, index: usize) -> TileWallLayout {
        let mut input = TileWallInput::new(self.wall_groups(), self.layout.viewport);
        input.metrics = self.metrics();
        input.scroll = self.scroll;
        wall_layout_without(&input, group, index)
    }

    /// 拖动中的基准布局（已摘除被拖项）；未进入拖动时为 `None`。
    pub fn drag_baseline(&self) -> Option<TileWallLayout> {
        match self.drag {
            DragState::Dragging { group, index, .. } => Some(self.layout_without(group, index)),
            _ => None,
        }
    }

    /// 当前拖动在 `pos` 的落点（基准布局坐标）；未进入拖动时为 `None`。
    ///
    /// 判定照 launcher `shell/tiles.rs::drop_target`：指针 x 落在某组 x 区间时，命中磁贴
    /// 按左右半插前 / 后，组内空白（含标题带）追加组尾；落在组间缝隙或墙两侧空白时在
    /// 相应位置新建组。落点等于原位时回 [`WallDropTarget::Activated`]。
    pub fn drop_target_at(&self, pos: Point) -> Option<WallDropTarget> {
        let (group, index) = match self.drag {
            DragState::Dragging { group, index, .. } => (group, index),
            _ => return None,
        };
        let base = self.layout_without(group, index);
        Some(drop_target_in(
            &base,
            pos,
            (group, index),
            &self.id_at(group, index),
        ))
    }

    /// 悬停更新（返回是否变化）。
    pub fn hover(&mut self, pos: Point) -> bool {
        let t = self.hit(pos);
        let changed = t != self.hover;
        self.hover = t;
        changed
    }

    /// 悬停项。
    pub fn hovered(&self) -> Option<(usize, usize)> {
        self.hover
    }

    /// 焦点项。
    pub fn focused(&self) -> Option<(usize, usize)> {
        self.focus
    }

    /// 按下：记 `Armed`，等移动越阈值。
    pub fn press(&mut self, pos: Point) -> bool {
        match self.hit(pos) {
            Some((g, i)) => {
                self.drag = DragState::Armed {
                    group: g,
                    index: i,
                    from: pos,
                };
                self.hover = Some((g, i));
                self.focus = Some((g, i));
                true
            }
            None => {
                self.drag = DragState::Idle;
                false
            }
        }
    }

    /// 拖动：越阈值后记录落点。
    pub fn drag(&mut self, pos: Point) -> bool {
        let prev = self.drag;
        self.drag = match prev {
            DragState::Idle => DragState::Idle,
            DragState::Armed { group, index, from } => {
                let dx = pos.x - from.x;
                let dy = pos.y - from.y;
                if dx.hypot(dy) < DRAG_THRESHOLD {
                    prev
                } else {
                    DragState::Dragging {
                        group,
                        index,
                        over: self.hit(pos),
                    }
                }
            }
            DragState::Dragging {
                group,
                index,
                over: _,
            } => DragState::Dragging {
                group,
                index,
                over: self.hit(pos),
            },
        };
        self.hover = self.hit(pos);
        self.drag != prev
    }

    /// 是否正在拖。
    pub fn dragging(&self) -> bool {
        matches!(self.drag, DragState::Dragging { .. })
    }

    /// 落点（拖到哪）。
    pub fn drop_target(&self) -> Option<(usize, usize)> {
        match self.drag {
            DragState::Dragging { over, .. } => over,
            _ => None,
        }
    }

    /// 松手。
    pub fn release(&mut self, pos: Point) -> WallRelease {
        let state = std::mem::replace(&mut self.drag, DragState::Idle);
        let out = match state {
            DragState::Idle => WallRelease::Cancelled,
            DragState::Armed { group, index, .. } => match self.hit(pos) {
                Some((g, i)) if (g, i) == (group, index) => {
                    WallRelease::Activated(self.id_at(group, index))
                }
                // 未越阈值但移到了别的格子：按交互正典视为取消（不是移动）。
                _ => WallRelease::Cancelled,
            },
            DragState::Dragging { group, index, .. } => match self.hit(pos) {
                Some((g, i)) if (g, i) != (group, index) => WallRelease::Moved {
                    from_group: group,
                    from_index: index,
                    to_group: g,
                    to_index: i,
                },
                _ => WallRelease::Cancelled,
            },
        };
        self.hover = self.hit(pos);
        out
    }

    /// 把 `Moved` 真正应用（幂等：越界则不动）。
    pub fn apply_move(&mut self, m: &WallRelease, viewport: Rect) -> bool {
        let WallRelease::Moved {
            from_group,
            from_index,
            to_group,
            to_index,
        } = m.clone()
        else {
            return false;
        };
        if from_group >= self.groups.len()
            || from_index >= self.groups[from_group].tiles.len()
            || to_group >= self.groups.len()
            || to_index >= self.groups[to_group].tiles.len()
        {
            return false;
        }
        let tile = self.groups[from_group].tiles.remove(from_index);
        let insert = if from_group == to_group && from_index < to_index {
            to_index
        } else {
            to_index
        };
        self.groups[to_group].tiles.insert(insert, tile);
        self.relayout(viewport);
        true
    }

    /// 尺寸档循环：Small → Wide → Large → Small。
    ///
    /// 旧三档顺序与含义不变；4×2 的 `ExtraLarge` 不进循环，落到它时回到 `Small`。
    pub fn cycle_size(&mut self, group: usize, index: usize, viewport: Rect) -> Option<TileWallSize> {
        let tile = self.groups.get_mut(group)?.tiles.get_mut(index)?;
        tile.size = match tile.size {
            TileWallSize::Small => TileWallSize::Wide,
            TileWallSize::Wide => TileWallSize::Large,
            TileWallSize::Large => TileWallSize::Small,
            TileWallSize::ExtraLarge => TileWallSize::Small,
        };
        let size = tile.size;
        self.relayout(viewport);
        Some(size)
    }

    /// 键盘：左右在组内走，到头进下一组 / 上一组。
    pub fn focus_by(&mut self, delta: i32) -> Option<(usize, usize)> {
        let all: Vec<(usize, usize)> = self
            .groups
            .iter()
            .enumerate()
            .flat_map(|(g, grp)| (0..grp.tiles.len()).map(move |i| (g, i)))
            .collect();
        if all.is_empty() {
            return None;
        }
        let cur = self
            .focus
            .and_then(|f| all.iter().position(|x| *x == f))
            .map(|p| p as i32)
            .unwrap_or(-1);
        let next = (cur + delta).clamp(0, all.len() as i32 - 1) as usize;
        self.focus = Some(all[next]);
        self.focus
    }

    /// 字母索引跳转：返回该组块顶部的滚动偏移（外壳据此滚动）。
    pub fn jump_to_group(&self, group: usize) -> Option<f32> {
        self.layout
            .groups
            .iter()
            .find(|g| g.index == group)
            .map(|g| (g.rect.origin.x - self.layout.scroll).max(0.0))
    }

    /// 渲染：旧组标题带 + 磁贴底（磁贴本体由 `MetroTile` 负责，这里只画位与高亮）。
    ///
    /// 这是旧默认输出：15/20、`base_weight()`、`on_surface`、标题框高 20 且贴组块顶。
    /// 兼容组头走 [`Self::render_compat`] / [`Self::render_group_title`]，不在这里暗改。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        self.render_legacy_titles(theme, scene);
        self.render_slots(theme, engine, scene);
    }

    /// 旧组头：15/20、`base_weight()`、`on_surface`、框高 20 且贴组块顶。
    /// 单独抽出来只为让「默认输出未变」有可断言的形状，渲染顺序仍与旧 `render` 一致。
    fn render_legacy_titles(&self, theme: &MetroTheme, scene: &mut Scene) {
        let c = &theme.colors;
        let title_style = TextStyle::new(15.0, 20.0, base_weight());
        for g in &self.layout.groups {
            // 视口外预取的组不发绘制命令（几何仍可由 layout 取到）。
            if g.clip.is_empty() {
                continue;
            }
            scene.fill_rect(c.background, with_height(g.title_rect, 20.0));
            scene.text(
                g.title.clone(),
                with_height(g.title_rect, 20.0),
                c.on_surface,
                title_style,
                TextAlign::Left,
            );
        }
    }

    /// 兼容渲染：按组画 `caption` / `on_surface_variant` 组头，再画与旧 `render` 相同的磁贴底。
    ///
    /// 消费组为 `layout.groups`（视口内可见组）；组名仍取模型，空标题不画。
    pub fn render_compat(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        for g in &self.layout.groups {
            self.render_group_title(theme, scene, g.index);
        }
        self.render_slots(theme, engine, scene);
    }

    /// 按组单独画兼容组头，供保留树局部损伤重绘。返回是否真的画了文本。
    ///
    /// 样式固定为 `theme.typography.caption` + `theme.colors.on_surface_variant`，
    /// 文本在 `TILE_TITLE_H` 高标题带内按 caption 行高垂直居中；组名只来自模型。
    /// 视口外预取回来的组不在这里画（几何可用，屏幕外不发命令）。
    pub fn render_group_title(&self, theme: &MetroTheme, scene: &mut Scene, group: usize) -> bool {
        let Some(g) = self.groups.get(group) else {
            return false;
        };
        if g.title.is_empty() {
            return false;
        }
        let Some(placed) = self.layout.groups.iter().find(|x| x.index == group) else {
            return false;
        };
        if placed.clip.is_empty() {
            return false;
        }
        let style = theme.typography.caption;
        let Some(rect) = self.compat_title_rect(group, style.line_height) else {
            return false;
        };
        scene.label(
            g.title.clone(),
            rect,
            theme.colors.on_surface_variant,
            style,
            TextAlign::Left,
        );
        true
    }

    /// 兼容标题带：组内最上磁贴顶减 `TILE_TITLE_H + TILE_TITLE_GAP`。
    ///
    /// 无磁贴的组回落到组块顶（与应用 `title_top` 的 `unwrap_or` 一致）；组不在视口内返回 `None`。
    pub fn compat_title_band(&self, group: usize) -> Option<Rect> {
        let g = self.layout.groups.iter().find(|g| g.index == group)?;
        let empty = self
            .groups
            .get(group)
            .is_none_or(|group| group.tiles.is_empty());
        let tile_top = if empty {
            g.rect.origin.y
        } else {
            self.layout
                .slots
                .iter()
                .filter(|s| s.group == group)
                .map(|s| s.rect.origin.y)
                .reduce(f32::min)
                .unwrap_or(g.rect.origin.y + self.metrics().title_band)
        };
        Some(Rect::new(
            g.rect.origin.x,
            tile_top - TILE_TITLE_H - TILE_TITLE_GAP,
            g.rect.size.width,
            TILE_TITLE_H,
        ))
    }

    /// 兼容组头文本矩形：标题带内按给定行高垂直居中。
    pub fn compat_title_rect(&self, group: usize, line_height: f32) -> Option<Rect> {
        let band = self.compat_title_band(group)?;
        let lh = if line_height.is_finite() && line_height > 0.0 {
            line_height
        } else {
            0.0
        };
        Some(Rect::new(
            band.origin.x,
            band.origin.y + (TILE_TITLE_H - lh) / 2.0,
            band.size.width,
            lh,
        ))
    }

    /// 磁贴底 / 悬停 / 拖拽高亮；旧 `render` 与兼容 `render_compat` 共用。
    fn render_slots(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let c = &theme.colors;
        for slot in &self.layout.slots {
            // 视口外的预取槽位没有裁剪矩形，画了也是空白。
            if slot.clip.is_empty() {
                continue;
            }
            let hovered = self.hover == Some((slot.group, slot.index));
            let dragging = matches!(
                self.drag,
                DragState::Dragging { group, index, .. } if (group, index) == (slot.group, slot.index)
            );
            let fill = if dragging {
                c.primary
            } else if hovered {
                c.surface_variant
            } else {
                c.divider
            };
            scene.fill_rect(fill, slot.clip);
            let _ = (engine, sizes::ICON_32);
        }
    }

    fn hit(&self, pos: Point) -> Option<(usize, usize)> {
        self.layout.hit(pos).map(|s| (s.group, s.index))
    }

    fn exists(&self, group: usize, index: usize) -> bool {
        self.groups
            .get(group)
            .is_some_and(|g| index < g.tiles.len())
    }

    /// 模型 → 几何输入。组顺序与组索引严格一致，几何层不再重排。
    fn wall_groups(&self) -> Vec<TileWallGroup> {
        self.groups
            .iter()
            .map(|g| {
                TileWallGroup::new(g.title.clone()).with_tiles(
                    g.tiles
                        .iter()
                        .map(|t| TileWallTile::new(t.id.clone(), t.size))
                        .collect(),
                )
            })
            .collect()
    }

    fn id_at(&self, group: usize, index: usize) -> String {
        self.groups
            .get(group)
            .and_then(|g| g.tiles.get(index))
            .map(|t| t.id.clone())
            .unwrap_or_default()
    }
}

fn with_height(r: Rect, h: f32) -> Rect {
    Rect::new(r.origin.x, r.origin.y, r.size.width, h)
}

/// 基准布局上的落点判定，逐条对照 launcher `shell/tiles.rs::drop_target`（2026-11-18 读码）。
///
/// 1. 指针 x 落在某组 x 区间：命中磁贴按左右半插前 / 后，组内空白（标题带、组块底部）
///    追加组尾；命中判据是**完整磁贴矩形**（含 y），所以纵向离开磁贴就按组尾处理；
/// 2. 落在组间缝隙或首组左缘外：在右侧那一组前新建组；
/// 3. 落在末组右缘外：末尾新建组。
fn drop_target_in(
    base: &TileWallLayout,
    pos: Point,
    from: (usize, usize),
    from_id: &str,
) -> WallDropTarget {
    if base.groups.is_empty() {
        return WallDropTarget::NewGroup { at: 0 };
    }
    for g in &base.groups {
        if pos.x >= g.rect.origin.x && pos.x < g.rect.right() {
            let tiles: Vec<&TileWallSlot> =
                base.slots.iter().filter(|s| s.group == g.index).collect();
            for s in &tiles {
                if s.rect.contains(pos) {
                    let index = if pos.x >= s.rect.origin.x + s.rect.size.width / 2.0 {
                        s.index + 1
                    } else {
                        s.index
                    };
                    return settle(g.index, index, from, from_id);
                }
            }
            let end = tiles.iter().map(|s| s.index + 1).max().unwrap_or(0);
            return settle(g.index, end, from, from_id);
        }
    }
    for g in &base.groups {
        if pos.x < g.rect.origin.x {
            return WallDropTarget::NewGroup { at: g.index };
        }
    }
    WallDropTarget::NewGroup {
        at: base.groups.len(),
    }
}

/// 基准下标等于原位即「不换序」，按点击（Activated）回报。
fn settle(group: usize, index: usize, from: (usize, usize), from_id: &str) -> WallDropTarget {
    if (group, index) == from {
        WallDropTarget::Activated(from_id.to_string())
    } else {
        WallDropTarget::InGroup { group, index }
    }
}

/// 供调用方按组取布局（索引跳转后滚动到哪）。
pub fn group_layout(layout: &TileWallLayout, group: usize) -> Option<&TileWallGroupLayout> {
    layout.groups.iter().find(|g| g.index == group)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;
    use kanesumi_core::{Accent, ColorScheme, MetroTheme};

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    fn theme() -> MetroTheme {
        MetroTheme::for_scheme(ColorScheme::Dark, Accent::default())
    }

    fn standard_group(title: &str, count: usize, first: usize) -> WallGroup {
        let tiles = (0..count)
            .map(|i| {
                let n = first + i;
                WallTile::new(
                    format!("app{n:02}"),
                    format!("App{n:02}"),
                    TileWallSize::Large,
                )
            })
            .collect();
        WallGroup::new(title, tiles)
    }

    fn c0_wall() -> MetroTileWall {
        let mut wall = MetroTileWall::new();
        // C0 的提供方度量：margin_x = 1280×120/1366，其余为 1366×768 基准值。
        wall.set_compat_metrics(Some(TileWallMetrics::from_provider(
            70.0, 10.0, 70.0, 72.0, 112.4451,
        )));
        wall.set_groups(
            vec![standard_group("常用", 18, 1), standard_group("", 2, 19)],
            Rect::new(0.0, 0.0, 1280.0, 752.0),
        );
        wall
    }

    fn slot_at(
        wall: &MetroTileWall,
        group: usize,
        index: usize,
    ) -> &crate::navigation::tile_wall::TileWallSlot {
        wall.layout()
            .slots
            .iter()
            .find(|s| s.group == group && s.index == index)
            .expect("槽位应可见")
    }

    #[test]
    fn compat_c0_geometry_matches_frozen_numbers() {
        let wall = c0_wall();
        let first = slot_at(&wall, 0, 0);
        assert!(close(first.rect.origin.x, 112.4451), "C0 首格 x");
        assert!(close(first.rect.origin.y, 201.0), "C0 首格 y");
        assert!(close(first.rect.size.width, 150.0), "C0 标准方宽");
        assert!(close(first.rect.size.height, 150.0), "C0 标准方高");

        let second = slot_at(&wall, 1, 0);
        assert!(close(second.rect.origin.x, 1132.4451), "C0 第二组首格 x");
        assert!(close(second.rect.origin.y, 201.0), "C0 第二组首格 y");

        // 兼容档关闭后必须回到旧缺省 120，而不是悄悄沿用过提供方值。
        let mut legacy = wall;
        legacy.set_compat_metrics(None);
        assert_eq!(legacy.metrics(), TileWallMetrics::BASE);
        assert!(close(slot_at(&legacy, 0, 0).rect.origin.x, 120.0));
    }

    #[test]
    fn compat_title_band_matches_c0_and_uses_caption_role() {
        let wall = c0_wall();
        let band = wall.compat_title_band(0).expect("首组标题带");
        assert!(close(band.origin.x, 112.4451));
        assert!(close(band.origin.y, 153.0), "201 − 40 − 8");
        assert!(close(band.size.width, 950.0));
        assert!(close(band.size.height, 40.0));

        let text = wall.compat_title_rect(0, 18.0).expect("caption 框");
        assert!(close(text.origin.y, 164.0), "153 + (40−18)/2");
        assert!(close(text.size.height, 18.0));
        assert!(
            text.contains(Point::new(200.0, 170.0)),
            "C0 的 (200,170) 必须落在组头文本内"
        );

        let theme = theme();
        let mut scene = Scene::default();
        assert!(wall.render_group_title(&theme, &mut scene, 0));
        let SceneCommand::Text {
            content,
            rect,
            color,
            style,
            ..
        } = &scene.commands[0]
        else {
            panic!("组头应发 Text 命令，实际 {:?}", scene.commands[0]);
        };
        assert_eq!(content, "常用");
        assert_eq!(*color, theme.colors.on_surface_variant);
        assert_eq!(*style, theme.typography.caption);
        assert!(close(rect.origin.y, 164.0));
        // 第二组无标题：不画，也不凭空造文案。
        assert!(!wall.render_group_title(&theme, &mut scene, 1));
        assert_eq!(scene.commands.len(), 1);
    }

    #[test]
    fn legacy_title_and_default_layout_stay_unchanged() {
        let mut wall = MetroTileWall::new();
        wall.set_groups(
            vec![standard_group("常用", 1, 1)],
            Rect::new(0.0, 0.0, 1000.0, 700.0),
        );
        assert_eq!(wall.metrics(), TileWallMetrics::BASE);
        assert!(close(slot_at(&wall, 0, 0).rect.origin.x, 120.0));

        let group = &wall.layout().groups[0];
        let theme = theme();
        let mut scene = Scene::default();
        wall.render_legacy_titles(&theme, &mut scene);

        let SceneCommand::FillRect { color, rect, .. } = &scene.commands[0] else {
            panic!("旧组头应先画标题底");
        };
        assert_eq!(*color, theme.colors.background);
        assert!(close(rect.origin.y, group.title_rect.origin.y));
        assert!(close(rect.size.height, 20.0));

        let SceneCommand::Text {
            content,
            rect,
            color,
            style,
            ..
        } = &scene.commands[1]
        else {
            panic!("旧组头文本命令");
        };
        assert_eq!(content, "常用");
        assert_eq!(*color, theme.colors.on_surface);
        assert_eq!(style.size, 15.0);
        assert_eq!(style.line_height, 20.0);
        assert_eq!(style.weight, base_weight());
        assert!(close(rect.origin.y, group.title_rect.origin.y));
        assert!(close(rect.size.height, 20.0));
    }

    #[test]
    fn extra_large_is_4x2_and_old_cycle_is_unchanged() {
        let viewport = Rect::new(0.0, 0.0, 1280.0, 752.0);
        let mut wall = MetroTileWall::new();
        wall.set_groups(
            vec![WallGroup::new(
                "",
                vec![WallTile::new("x", "X", TileWallSize::ExtraLarge)],
            )],
            viewport,
        );
        let s = slot_at(&wall, 0, 0);
        assert!(close(s.rect.size.width, 310.0));
        assert!(close(s.rect.size.height, 150.0));

        // ExtraLarge 不进旧循环：先回 Small，再走 Small → Wide → Large → Small。
        assert_eq!(wall.cycle_size(0, 0, viewport), Some(TileWallSize::Small));
        assert_eq!(wall.cycle_size(0, 0, viewport), Some(TileWallSize::Wide));
        assert_eq!(wall.cycle_size(0, 0, viewport), Some(TileWallSize::Large));
        assert_eq!(wall.cycle_size(0, 0, viewport), Some(TileWallSize::Small));
    }

    #[test]
    fn compat_narrow_and_double_metrics_scale_geometry() {
        let mut narrow = MetroTileWall::new();
        narrow.set_compat_metrics(Some(TileWallMetrics::from_provider(
            70.0, 10.0, 70.0, 72.0, 70.2782,
        )));
        narrow.set_groups(
            vec![standard_group("常用", 18, 1), standard_group("", 2, 19)],
            Rect::new(0.0, 0.0, 800.0, 600.0),
        );
        assert!(close(slot_at(&narrow, 0, 0).rect.origin.x, 70.2782));

        // 2× 逻辑度量 + 2× 视口：所有几何应是 C0 的两倍。
        let mut doubled = MetroTileWall::new();
        doubled.set_compat_metrics(Some(TileWallMetrics::from_provider(
            140.0, 20.0, 140.0, 144.0, 224.8902,
        )));
        doubled.set_groups(
            vec![standard_group("常用", 18, 1), standard_group("", 2, 19)],
            Rect::new(0.0, 0.0, 2560.0, 1504.0),
        );
        let first = slot_at(&doubled, 0, 0);
        assert!(close(first.rect.origin.x, 224.8902));
        // 底部让位带仍取库缺省 48（应用度量不含该字段），故 y 不是 C0 的严格两倍：
        // (1504+48−1084)/2 + 144 = 378。
        assert!(close(first.rect.origin.y, 378.0));
        assert!(close(first.rect.size.width, 300.0));
        assert!(close(first.rect.size.height, 300.0));
        assert!(close(slot_at(&doubled, 1, 0).rect.origin.x, 2264.8902));
    }

    // ── B1 拖动基准 / B2 滚动 / B3 预取 ────────────────────────────────────

    fn center_of(wall: &MetroTileWall, group: usize, index: usize) -> Point {
        slot_at(wall, group, index).rect.center()
    }

    /// 按下 app01 并越过阈值进入拖动；落点随后由 `drop_target_at` 单独判。
    fn start_dragging(wall: &mut MetroTileWall) {
        let from = center_of(wall, 0, 0);
        assert!(wall.press(from));
        assert!(wall.drag(Point::new(from.x + DRAG_THRESHOLD + 1.0, from.y)));
        assert!(wall.dragging());
    }

    #[test]
    fn drop_target_matches_legacy_launcher_table() {
        let mut wall = c0_wall();
        start_dragging(&mut wall);

        // 表中只给 x 的行，读数取被拖项所在行（y = 276）；旧 `drop_target` 的命中判据
        // 是完整磁贴矩形，所以 y 必须落在那一行，读数才与表一致。
        let y = 276.0;
        let rows = [
            (180.0, WallDropTarget::Activated("app01".to_string())),
            (200.0, WallDropTarget::InGroup { group: 0, index: 1 }),
            (250.0, WallDropTarget::InGroup { group: 0, index: 1 }),
            (312.0, WallDropTarget::InGroup { group: 0, index: 1 }),
            (350.0, WallDropTarget::InGroup { group: 0, index: 2 }),
            (400.0, WallDropTarget::InGroup { group: 0, index: 2 }),
        ];
        for (x, want) in rows {
            assert_eq!(wall.drop_target_at(Point::new(x, y)), Some(want), "x = {x}");
        }
        // 墙左缘外的两个实测点（Ether C0.md §3.6 / §3.6.1）→ 首位新建组。
        for pos in [Point::new(-10.0, 376.0), Point::new(50.0, 400.0)] {
            assert_eq!(
                wall.drop_target_at(pos),
                Some(WallDropTarget::NewGroup { at: 0 }),
                "pos = {pos:?}"
            );
        }
        // 另一组：x 相同、y 在该组首行 → InGroup{1,1}（表中要的读数）。
        assert_eq!(
            wall.drop_target_at(Point::new(1270.0, 276.0)),
            Some(WallDropTarget::InGroup { group: 1, index: 1 })
        );
        // 表里的 (1270,760) 纵向在组块（底 671）之外，旧算法按「组内空白」追加组尾，
        // 读数是 index 2；该点在实际应用里是 Dock 钉选带（Ether C0.md §3.6.2），
        // 不进磁贴墙落点判定。见 REPORT §一·B1 的差异说明。
        assert_eq!(
            wall.drop_target_at(Point::new(1270.0, 760.0)),
            Some(WallDropTarget::InGroup { group: 1, index: 2 })
        );
        // 未拖动时没有落点。
        let idle = c0_wall();
        assert!(idle.drop_target_at(Point::new(200.0, 276.0)).is_none());
    }

    #[test]
    fn drag_baseline_excludes_only_dragged_tile_and_keeps_groups() {
        let mut wall = c0_wall();
        assert!(wall.drag_baseline().is_none(), "未拖动时没有基准布局");
        start_dragging(&mut wall);
        let base = wall.drag_baseline().expect("拖动中有基准布局");
        assert_eq!(base.slots.len(), 19);
        assert_eq!(base.groups.len(), 2, "摘空的组也保留，组索引不漂移");
        assert!(base.slots.iter().all(|s| s.id != "app01"));
        assert_eq!(base.slots[0].id, "app02");
        assert!(close(base.slots[0].rect.origin.x, 112.4451));
        assert!(close(base.slots[0].rect.origin.y, 201.0));

        // 显式入口与拖动无关，同一份基准。
        let manual = wall.layout_without(0, 0);
        assert_eq!(manual, base);
    }

    #[test]
    fn scroll_shifts_hit_and_render_together() {
        let mut wall = c0_wall();
        let before = slot_at(&wall, 0, 5).clone();
        let p = before.rect.center();
        assert_eq!(wall.layout().hit(p).map(|s| s.id.as_str()), Some("app06"));

        wall.set_scroll(500.0);
        assert!(close(wall.scroll(), 500.0));
        assert!(close(wall.layout().scroll, 500.0));
        let after = slot_at(&wall, 0, 5);
        assert!(close(after.rect.origin.x, before.rect.origin.x - 500.0));
        assert!(close(after.rect.origin.y, before.rect.origin.y), "只横向滚");
        // 命中消费同一份几何：内容右移 500 后，指针左移 500 仍命中同一格。
        assert_eq!(
            wall.layout()
                .hit(Point::new(p.x - 500.0, p.y))
                .map(|s| s.id.as_str()),
            Some("app06")
        );

        // 绘制同样按滚动后的 clip：不再有「命中一套、画另一套」。
        let theme = theme();
        let engine = kanesumi_element::testing::test_engine();
        let mut scene = Scene::default();
        wall.render(&theme, &engine, &mut scene);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { rect, .. } if *rect == after.clip)),
            "磁贴底应按滚动后的裁剪矩形绘制"
        );

        // 重排不会把滚动丢掉。
        let vp = wall.layout().viewport;
        wall.relayout(vp);
        assert!(close(wall.scroll(), 500.0));
    }

    #[test]
    fn scroll_clamps_to_max_and_non_finite_resets() {
        let mut wall = c0_wall();
        let max = wall.layout().max_scroll;
        assert!(close(max, 914.8901), "C0 夹具本来就滚得动");

        wall.set_scroll(99_999.0);
        assert!(close(wall.scroll(), max));
        assert!(close(wall.layout().scroll, max));

        wall.set_scroll(-50.0);
        assert_eq!(wall.scroll(), 0.0);

        // 内容变少后重排：写回值被夹到新的上界。
        wall.set_scroll(500.0);
        wall.set_groups(
            vec![standard_group("常用", 2, 1)],
            Rect::new(0.0, 0.0, 1280.0, 752.0),
        );
        assert!(close(wall.scroll(), wall.layout().max_scroll));
        assert_eq!(wall.scroll(), 0.0);

        wall.set_scroll(f32::NAN);
        assert_eq!(wall.scroll(), 0.0);
    }

    #[test]
    fn explicit_zero_scroll_keeps_layout_and_render_identical() {
        let base = c0_wall();
        let mut zero = c0_wall();
        zero.set_scroll(0.0);
        assert_eq!(base.layout(), zero.layout());

        let theme = theme();
        let engine = kanesumi_element::testing::test_engine();
        let mut a = Scene::default();
        let mut b = Scene::default();
        base.render(&theme, &engine, &mut a);
        zero.render(&theme, &engine, &mut b);
        assert_eq!(a.commands, b.commands);
    }

    #[test]
    fn overscan_default_unchanged_and_two_columns_adds_app20() {
        let mut wall = c0_wall();
        let legacy = wall.layout().clone();
        assert_eq!(legacy.slots.len(), 19);
        assert_eq!(legacy.groups.len(), 2);

        wall.set_overscan_cols(2);
        assert_eq!(wall.layout().slots.len(), 20);
        assert!(wall.layout().slots.iter().any(|s| s.id == "app20"));

        wall.set_overscan_cols(0);
        assert_eq!(wall.layout(), &legacy, "关掉预取逐字段回到旧输出");
    }

    #[test]
    fn overscan_twelve_groups_at_1366_returns_viewport_plus_columns() {
        let groups = (0..12)
            .map(|g| {
                WallGroup::new(
                    format!("组{g}"),
                    vec![WallTile::new(format!("t{g}"), "T", TileWallSize::Small)],
                )
            })
            .collect();
        let mut wall = MetroTileWall::new();
        wall.set_groups(groups, Rect::new(0.0, 0.0, 1366.0, 768.0));
        assert_eq!(wall.layout().groups.len(), 2, "旧行为：只回视口内的两组");

        wall.set_overscan_cols(2);
        let groups = &wall.layout().groups;
        assert!(
            groups.len() >= 3,
            "至少含视口左右各一列，实际 {}",
            groups.len()
        );
        let outside = groups
            .iter()
            .find(|g| g.clip.is_empty())
            .expect("右侧预取列");
        assert!(outside.rect.origin.x >= 1366.0);
        // 预取槽位不参与命中（躲开纵向的钉选带）。
        assert!(
            wall.layout()
                .hit(Point::new(outside.rect.center().x, outside.rect.center().y))
                .is_none()
        );
    }
}
