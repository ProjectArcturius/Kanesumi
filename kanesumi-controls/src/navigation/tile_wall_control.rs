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
    layout as wall_layout, TileWallGroup, TileWallGroupLayout, TileWallInput, TileWallLayout,
    TileWallMetrics, TileWallSize, TileWallTile, TILE_TITLE_GAP, TILE_TITLE_H,
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

    /// 按视口重排。兼容档启用时消费 `set_compat_metrics` 传入的响应式度量。
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
        let mut input = TileWallInput::new(groups, viewport);
        input.metrics = self.metrics();
        self.layout = wall_layout(&input);
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
    pub fn render_group_title(&self, theme: &MetroTheme, scene: &mut Scene, group: usize) -> bool {
        let Some(g) = self.groups.get(group) else {
            return false;
        };
        if g.title.is_empty() {
            return false;
        }
        if !self.layout.groups.iter().any(|x| x.index == group) {
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
}
