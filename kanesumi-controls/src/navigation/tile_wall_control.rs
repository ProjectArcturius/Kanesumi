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
// - 尺寸档三档循环：Small → Medium → Wide → Small；
// - 键盘：左右在组内走、走到头进下一组；Enter 激活；
// - 字母索引跳转：返回该组顶部的滚动偏移（外壳据此滚过去，控件不做动画）。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::typography::{TextStyle, base_weight};
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

use crate::navigation::tile_wall::{
    TileWallGroup, TileWallGroupLayout, TileWallInput, TileWallLayout, TileWallSize, TileWallTile,
    layout as wall_layout,
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

/// 拖动状态。
#[derive(Debug, Clone, Copy, PartialEq)]
enum DragState {
    Idle,
    /// 已按下，尚未越阈值。
    Armed { group: usize, index: usize, from: Point },
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
            layout: wall_layout(&TileWallInput::new(Vec::new(), Rect::new(0.0, 0.0, 0.0, 0.0))),
            hover: None,
            focus: None,
            drag: DragState::Idle,
        }
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

    /// 按视口重排。
    pub fn relayout(&mut self, viewport: Rect) {
        let groups: Vec<TileWallGroup> = self
            .groups
            .iter()
            .map(|g| {
                TileWallGroup::new(g.title.clone()).with_tiles(
                    g.tiles
                        .iter()
                        .map(|t| TileWallTile::new(t.id.clone(), t.size))
                        .collect(),
                )
            })
            .collect();
        self.layout = wall_layout(&TileWallInput::new(groups, viewport));
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
            DragState::Armed {
                group,
                index,
                from,
            } => {
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

    /// 尺寸档循环：Small → Medium → Wide → Small。
    pub fn cycle_size(&mut self, group: usize, index: usize, viewport: Rect) -> Option<TileWallSize> {
        let tile = self.groups.get_mut(group)?.tiles.get_mut(index)?;
        tile.size = match tile.size {
            TileWallSize::Small => TileWallSize::Wide,
            TileWallSize::Wide => TileWallSize::Large,
            TileWallSize::Large => TileWallSize::Small,
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

    /// 渲染：组标题带 + 磁贴底（磁贴本体由 `MetroTile` 负责，这里只画位与高亮）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene) {
        let c = &theme.colors;
        let title_style = TextStyle::new(15.0, 20.0, base_weight());
        for g in &self.layout.groups {
            scene.fill_rect(c.background, with_height(g.title_rect, 20.0));
            scene.text(
                g.title.clone(),
                with_height(g.title_rect, 20.0),
                c.on_surface,
                title_style,
                TextAlign::Left,
            );
        }
        for slot in &self.layout.slots {
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

/// 供调用方按组取布局（索引跳转后滚动到哪）。
pub fn group_layout(layout: &TileWallLayout, group: usize) -> Option<&TileWallGroupLayout> {
    layout.groups.iter().find(|g| g.index == group)
}
