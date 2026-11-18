// tile_grid.rs —— 快捷方块网格（MetroTileGrid）：等分格 + 标签 + 逐格损伤，图标由调用方 blit。
//
// 缺口（Ether `docs/research/m1115_panels/REPORT.md` §五）：`action_center.rs::TilesGrid` 的
// 2×4 网格是手算的 `tile_rects`（4 列等宽、间隔 2、格高 56），命中再算一遍，损伤记到
// `hover: Option<usize>` 一格；`net_panel` 的三块方块又是一套。本控件把「等分 + 命中 + 逐格
// 损伤」收成一份几何：绘制、命中、损伤消费同一个 `TileGridGeometry`。
//
// **不画图标**：图标解码与 blit 归调用方（`icon_rect` 只给方框），本控件不接 `Icon`。
// 参 `docs/CONTROL_SPEC.md` §55.4。

use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::MetroTypography;
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

// 缺省几何来自 Ether `action_center.rs::TilesGrid` 的实测值：间隔 2（`.tiles` 的 `gap`）、
// 格高 56（`.tile` 的 56）、标签内距 8 / 6（`.tile` 的 `padding: 6px 8px`）、图标位 8 / 8 / 20
// （`net_panel::TILE_ICON` 20；`action_center` 是 18 / 8 / 6，走 `with_icon_box`）。
//
// 它们是 `MetroTileGrid` 的字段缺省（「自身默认参数」），**不立 `const f32`**：
// 尺寸的定义归 `kanesumi_core::sizes`，而 `kanesumi-controls/tests/size_discipline.rs`
// 的白名单只减不增（本卡不改 core）。参 `docs/CONTROL_SPEC.md` §55.4。

/// 图标位缺省边长 —— 语义别名引用尺寸表（`net_panel::TILE_ICON` 也是 20）。
pub const TILE_ICON_SIZE: f32 = sizes::ICON_20;

/// 方块网格。`rows` / `cols` 由调用方给（action_center 是 2 行 4 列）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTileGrid {
    rows: usize,
    cols: usize,
    gap: f32,
    tile_h: f32,
    labels: Vec<String>,
    on: Vec<bool>,
    disabled: Vec<bool>,
    hovered: Option<usize>,
    icon_size: f32,
    icon_pad_x: f32,
    icon_pad_y: f32,
}

impl MetroTileGrid {
    /// 建一个 `rows × cols` 的网格。任一维为 0 = 空格（不出像素，元素数目 = 0）。
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            rows,
            cols,
            gap: 2.0,
            tile_h: 56.0,
            labels: Vec::new(),
            on: Vec::new(),
            disabled: Vec::new(),
            hovered: None,
            icon_size: TILE_ICON_SIZE,
            icon_pad_x: 8.0,
            icon_pad_y: 8.0,
        }
    }

    /// 格间距。负值夹 0，非有限值回落缺省。
    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = sane(gap, 2.0).max(0.0);
        self
    }

    /// 格高。负值夹 0，非有限值回落缺省。
    pub fn with_tile_height(mut self, h: f32) -> Self {
        self.tile_h = sane(h, 56.0).max(0.0);
        self
    }

    /// 逐格标签（按格序；缺的格子不画标签）。空串同样不画。
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.labels = labels.into_iter().map(Into::into).collect();
        self
    }

    /// 图标位：边长与左 / 上内距（`action_center` 的 18 / 8 / 6）。
    pub fn with_icon_box(mut self, size: f32, pad_x: f32, pad_y: f32) -> Self {
        self.icon_size = sane(size, TILE_ICON_SIZE).max(0.0);
        self.icon_pad_x = sane(pad_x, 8.0).max(0.0);
        self.icon_pad_y = sane(pad_y, 8.0).max(0.0);
        self
    }

    /// 第 `i` 格的开态（开 = 强调底 + 强调前景）。越界忽略。
    pub fn with_on(mut self, index: usize, on: bool) -> Self {
        self.set_on(index, on);
        self
    }

    /// 第 `i` 格是否禁用（禁用 = 底色与前景降不透明度、不画悬停描边）。越界忽略。
    pub fn with_disabled(mut self, index: usize, disabled: bool) -> Self {
        self.set_disabled(index, disabled);
        self
    }

    /// 行数。
    pub fn rows(&self) -> usize {
        self.rows
    }

    /// 列数。
    pub fn cols(&self) -> usize {
        self.cols
    }

    /// 格间距。
    pub fn gap(&self) -> f32 {
        self.gap
    }

    /// 格高。
    pub fn tile_height(&self) -> f32 {
        self.tile_h
    }

    /// 格数 = `rows × cols`（与面积无关：面积 0 时格数也不变）。
    pub fn count(&self) -> usize {
        self.rows.saturating_mul(self.cols)
    }

    /// 网格高 = `rows × 格高 + (rows − 1) × 间隔`。
    pub fn height(&self) -> f32 {
        if self.rows == 0 {
            return 0.0;
        }
        self.tile_h * self.rows as f32 + self.gap * (self.rows - 1) as f32
    }

    /// 第 `i` 格的标签。
    pub fn label(&self, index: usize) -> Option<&str> {
        self.labels.get(index).map(String::as_str)
    }

    /// 第 `i` 格是否开。
    pub fn is_on(&self, index: usize) -> bool {
        self.on.get(index).copied().unwrap_or(false)
    }

    /// 第 `i` 格是否禁用。
    pub fn is_disabled(&self, index: usize) -> bool {
        self.disabled.get(index).copied().unwrap_or(false)
    }

    /// 设某格开态（越界忽略）。
    pub fn set_on(&mut self, index: usize, on: bool) {
        if index < self.count() {
            grow(&mut self.on, index);
            self.on[index] = on;
        }
    }

    /// 设某格禁用（越界忽略）。
    pub fn set_disabled(&mut self, index: usize, disabled: bool) {
        if index < self.count() {
            grow(&mut self.disabled, index);
            self.disabled[index] = disabled;
        }
    }

    /// 指针悬停的格子（`None` = 无悬停）。越界值归 `None`。
    pub fn set_hovered(&mut self, index: Option<usize>) {
        self.hovered = index.filter(|i| *i < self.count());
    }

    /// 当前悬停格。
    pub fn hovered(&self) -> Option<usize> {
        self.hovered
    }

    /// 量网格：`area` 的宽参与等分，高由 [`Self::height`] 决定（调用方按它铺 `area`）。
    pub fn layout(&self, area: Rect) -> TileGridGeometry {
        let area = area.normalized();
        let cells = if self.rows == 0 || self.cols == 0 {
            Vec::new()
        } else {
            let total_gap = self.gap * (self.cols - 1) as f32;
            let tile_w = ((area.size.width - total_gap) / self.cols as f32).max(0.0);
            // 宽吃不下（含面积为 0）：所有格塌到网格原点成零矩形 —— 不让 gap 把零宽格推到
            // 网格右缘之外，否则「零面积」会产出一串位置各异的空损伤。
            if tile_w <= 0.0 {
                vec![Rect::new(area.origin.x, area.origin.y, 0.0, self.tile_h); self.count()]
            } else {
                (0..self.count())
                    .map(|i| {
                        let (col, row) = (i % self.cols, i / self.cols);
                        Rect::new(
                            area.origin.x + col as f32 * (tile_w + self.gap),
                            area.origin.y + row as f32 * (self.tile_h + self.gap),
                            tile_w,
                            self.tile_h,
                        )
                    })
                    .collect()
            }
        };
        TileGridGeometry {
            rect: Rect::new(area.origin.x, area.origin.y, area.size.width, self.height()),
            cells,
        }
    }

    /// 图标位（调用方往这里 blit）。缺省 `(cell.x + 8, cell.y + 8, 20, 20)`。
    pub fn icon_rect(&self, cell: Rect) -> Rect {
        Rect::new(
            cell.origin.x + self.icon_pad_x,
            cell.origin.y + self.icon_pad_y,
            self.icon_size
                .min((cell.size.width - self.icon_pad_x).max(0.0)),
            self.icon_size
                .min((cell.size.height - self.icon_pad_y).max(0.0)),
        )
    }

    /// 标签位（左下，`body_small` 一行）。
    pub fn label_rect(&self, cell: Rect) -> Rect {
        let line = MetroTypography::metro().body_small.line_height;
        // `.tile` 的 `padding: 6px 8px`：左右 8、距底 6。
        let (pad, bottom) = (8.0, 6.0);
        Rect::new(
            cell.origin.x + pad,
            cell.bottom() - bottom - line,
            (cell.size.width - 2.0 * pad).max(0.0),
            line,
        )
    }

    /// 逐格绘制：返回每格 `{ index, rect, damage, commands }`。
    ///
    /// `commands` 是该格命令在 `scene.commands` 里的半开区间（本格命令连续，不跨格）；
    /// `damage` 只盖本格。零宽 / 零高的格**不产出**（命令与条目都不加）。
    pub fn render(
        &self,
        theme: &MetroTheme,
        geo: &TileGridGeometry,
        scene: &mut Scene,
    ) -> Vec<TileCellDraw> {
        let mut out = Vec::with_capacity(geo.cells.len());
        for (index, cell) in geo.cells.iter().copied().enumerate() {
            if cell.size.width <= 0.0 || cell.size.height <= 0.0 {
                continue;
            }
            let start = scene.commands.len();
            let on = self.is_on(index);
            let disabled = self.is_disabled(index);
            let alpha = if disabled {
                theme.indication.disabled_opacity
            } else {
                1.0
            };
            let bg = if on {
                theme.colors.primary
            } else {
                theme.colors.surface_variant
            };
            let fg = if on {
                theme.colors.on_primary
            } else {
                theme.colors.on_surface
            };
            // 开 / 关与禁用都只换 token 的透明度，不新增颜色常量（参 CONTROL_SPEC §55.4）。
            scene.fill_rect(bg.with_alpha(bg.a * alpha), cell);
            if !disabled && self.hovered == Some(index) {
                // 预览 `.tile:hover` 是 2 px 内描边，不换底。
                scene.stroke_rect(
                    theme
                        .colors
                        .on_surface
                        .with_alpha(theme.indication.base_medium),
                    cell,
                    2.0,
                );
            }
            if let Some(label) = self.label(index)
                && !label.trim().is_empty()
            {
                scene.label(
                    label.to_string(),
                    self.label_rect(cell),
                    fg.with_alpha(fg.a * alpha),
                    theme.typography.body_small,
                    TextAlign::Left,
                );
            }
            out.push(TileCellDraw {
                index,
                rect: cell,
                damage: cell,
                commands: start..scene.commands.len(),
            });
        }
        out
    }
}

/// 网格几何（绝对坐标）。绘制、命中、逐格损伤消费同一份。
#[derive(Debug, Clone, PartialEq)]
pub struct TileGridGeometry {
    /// 整块网格盒（高按 [`MetroTileGrid::height`] 算，不含调用方的外内距）。
    pub rect: Rect,
    /// 逐格矩形，顺序 = 行优先（`index = row × cols + col`）。
    pub cells: Vec<Rect>,
}

impl TileGridGeometry {
    /// 整块网格盒。
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// 逐格矩形（只读切片）。
    pub fn rects(&self) -> &[Rect] {
        &self.cells
    }

    /// 第 `i` 格。
    pub fn cell(&self, index: usize) -> Option<Rect> {
        self.cells.get(index).copied()
    }

    /// 命中：点在格内（含边）返回格号；落在格缝、网格外、零尺寸格上都返回 `None`。
    pub fn hit(&self, pos: Point) -> Option<usize> {
        self.cells.iter().position(|r| contains(*r, pos))
    }

    /// 第 `i` 格的损伤矩形（就是它自己那一格）。
    pub fn damage(&self, index: usize) -> Option<Rect> {
        self.cell(index)
            .filter(|r| r.size.width > 0.0 && r.size.height > 0.0)
    }
}

/// 一格的绘制结果：几何 + 损伤 + 本格命令区间。
#[derive(Debug, Clone, PartialEq)]
pub struct TileCellDraw {
    /// 格号（行优先）。
    pub index: usize,
    /// 格矩形。
    pub rect: Rect,
    /// 本格损伤（= 格矩形）。
    pub damage: Rect,
    /// 本格命令在 `Scene::commands` 里的半开区间。
    pub commands: std::ops::Range<usize>,
}

fn sane(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v } else { fallback }
}

fn grow(v: &mut Vec<bool>, index: usize) {
    if v.len() <= index {
        v.resize(index + 1, false);
    }
}

fn contains(r: Rect, p: Point) -> bool {
    r.size.width > 0.0
        && r.size.height > 0.0
        && p.x >= r.origin.x
        && p.x < r.right()
        && p.y >= r.origin.y
        && p.y < r.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    /// 卡面点名的字面量：2 行 × 4 列、间隔 8、格高 64、宽 616 → 首格 (0,0,148,64)。
    fn card_grid() -> MetroTileGrid {
        MetroTileGrid::new(2, 4)
            .with_gap(8.0)
            .with_tile_height(64.0)
    }

    #[test]
    fn two_by_four_gap_8_first_cell_is_148_by_64() {
        let grid = card_grid();
        let geo = grid.layout(Rect::new(0.0, 0.0, 616.0, 136.0));
        assert_eq!(grid.count(), 8);
        assert_eq!(geo.rects().len(), 8);
        assert_eq!(geo.rect(), Rect::new(0.0, 0.0, 616.0, 136.0));
        // (616 − 3 × 8) / 4 = 148；行步距 64 + 8 = 72。
        assert_eq!(geo.cell(0), Some(Rect::new(0.0, 0.0, 148.0, 64.0)));
        assert_eq!(geo.cell(1), Some(Rect::new(156.0, 0.0, 148.0, 64.0)));
        assert_eq!(geo.cell(4), Some(Rect::new(0.0, 72.0, 148.0, 64.0)));
        assert_eq!(geo.cell(7), Some(Rect::new(468.0, 72.0, 148.0, 64.0)));
        assert_eq!(grid.height(), 136.0);
    }

    #[test]
    fn action_center_geometry_matches_the_app_constants() {
        // 2 行 4 列、缺省间隔 2 / 格高 56；面板 360 减左右各 16 → 328 宽。
        let grid = MetroTileGrid::new(2, 4);
        assert_eq!(grid.gap(), 2.0);
        assert_eq!(grid.tile_height(), 56.0);
        let geo = grid.layout(Rect::new(16.0, 100.0, 328.0, 114.0));
        // (328 − 3 × 2) / 4 = 80.5；列步距 82.5；行步距 58。
        assert_eq!(geo.cell(0), Some(Rect::new(16.0, 100.0, 80.5, 56.0)));
        assert_eq!(geo.cell(1), Some(Rect::new(98.5, 100.0, 80.5, 56.0)));
        assert_eq!(geo.cell(2), Some(Rect::new(181.0, 100.0, 80.5, 56.0)));
        assert_eq!(geo.cell(3), Some(Rect::new(263.5, 100.0, 80.5, 56.0)));
        assert_eq!(geo.cell(4), Some(Rect::new(16.0, 158.0, 80.5, 56.0)));
        assert_eq!(grid.height(), 114.0);
        // 图标位缺省 8 / 8 / 20；标签左下 16 高、左右各让 8。
        assert_eq!(
            grid.icon_rect(geo.cell(0).unwrap()),
            Rect::new(24.0, 108.0, 20.0, 20.0)
        );
        assert_eq!(
            grid.label_rect(geo.cell(0).unwrap()),
            Rect::new(24.0, 134.0, 64.5, 16.0)
        );
        // action_center 的图标是 18 / 8 / 6。
        let ac = MetroTileGrid::new(2, 4).with_icon_box(18.0, 8.0, 6.0);
        assert_eq!(
            ac.icon_rect(geo.cell(0).unwrap()),
            Rect::new(24.0, 106.0, 18.0, 18.0)
        );
    }

    #[test]
    fn hit_skips_gaps_and_outside() {
        let geo = card_grid().layout(Rect::new(0.0, 0.0, 616.0, 136.0));
        assert_eq!(geo.hit(Point::new(0.0, 0.0)), Some(0), "左上角含");
        assert_eq!(geo.hit(Point::new(147.0, 63.0)), Some(0));
        assert_eq!(geo.hit(Point::new(148.0, 10.0)), None, "格缝不算命中");
        assert_eq!(geo.hit(Point::new(160.0, 10.0)), Some(1));
        assert_eq!(geo.hit(Point::new(10.0, 80.0)), Some(4));
        assert_eq!(geo.hit(Point::new(10.0, 70.0)), None, "行缝不算命中");
        assert_eq!(geo.hit(Point::new(-1.0, 0.0)), None);
        assert_eq!(geo.hit(Point::new(0.0, 136.0)), None, "网格之下");
    }

    #[test]
    fn empty_and_zero_area_grids_stay_pixel_free() {
        let theme = MetroTheme::ether_dark();
        // 维度 0：格数为 0，命令与条目都为空。
        for grid in [MetroTileGrid::new(0, 4), MetroTileGrid::new(2, 0)] {
            assert_eq!(grid.count(), 0);
            let geo = grid.layout(Rect::new(0.0, 0.0, 360.0, 100.0));
            assert!(geo.rects().is_empty());
            assert_eq!(geo.hit(Point::new(1.0, 1.0)), None);
            let mut scene = Scene::default();
            assert!(grid.render(&theme, &geo, &mut scene).is_empty());
            assert!(scene.commands.is_empty());
        }
        // 面积为 0：格数不变（元素数目 8），但没有一格产出命令。
        let grid = MetroTileGrid::new(2, 4);
        let geo = grid.layout(Rect::new(4.0, 4.0, 0.0, 0.0));
        assert_eq!(geo.rects().len(), 8, "元素数目不随面积变");
        assert!(
            geo.rects()
                .iter()
                .all(|r| r.origin == Point::new(4.0, 4.0) && r.size.width == 0.0),
            "零面积时所有格塌到原点成零宽格：{:?}",
            geo.rects()
        );
        let mut scene = Scene::default();
        assert!(grid.render(&theme, &geo, &mut scene).is_empty());
        assert!(scene.commands.is_empty());
        assert_eq!(geo.damage(0), None, "零尺寸格不产出损伤");
    }

    #[test]
    fn each_cell_gets_its_own_damage_and_command_range() {
        let theme = MetroTheme::ether_dark();
        let grid = card_grid().with_labels(["蓝牙", "夜灯"]);
        let geo = grid.layout(Rect::new(0.0, 0.0, 616.0, 136.0));
        let mut scene = Scene::default();
        let draws = grid.render(&theme, &geo, &mut scene);
        assert_eq!(draws.len(), 8);
        let mut covered = 0;
        for (i, d) in draws.iter().enumerate() {
            assert_eq!(d.index, i);
            assert_eq!(d.damage, geo.cell(i).unwrap(), "损伤 = 本格");
            assert_eq!(d.rect, d.damage);
            assert_eq!(d.commands.start, covered, "命令区间连续、不跨格");
            covered = d.commands.end;
        }
        assert_eq!(covered, scene.commands.len(), "区间并集 = 全部命令");
        // 只有前两格有标签：8 个底 + 2 条标签。
        assert_eq!(
            scene
                .commands
                .iter()
                .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
                .count(),
            8
        );
        assert_eq!(
            scene
                .commands
                .iter()
                .filter(|c| matches!(c, SceneCommand::Text { .. }))
                .count(),
            2
        );
    }

    #[test]
    fn on_and_off_colors_come_from_tokens() {
        let theme = MetroTheme::ether_dark();
        let grid = MetroTileGrid::new(1, 2)
            .with_on(1, true)
            .with_labels(["蓝牙", "勿扰"]);
        let geo = grid.layout(Rect::new(0.0, 0.0, 200.0, 56.0));
        let mut scene = Scene::default();
        grid.render(&theme, &geo, &mut scene);
        let fills: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, rect, .. } => Some((*color, *rect)),
                _ => None,
            })
            .collect();
        assert_eq!(
            fills[0],
            (theme.colors.surface_variant, geo.cell(0).unwrap())
        );
        assert_eq!(fills[1], (theme.colors.primary, geo.cell(1).unwrap()));
        let texts: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { color, style, .. } => Some((*color, style.size)),
                _ => None,
            })
            .collect();
        assert_eq!(
            texts[0],
            (theme.colors.on_surface, 12.0),
            "关 = 常规墨色 + body_small"
        );
        assert_eq!(
            texts[1],
            (theme.colors.on_primary, 12.0),
            "开 = 强调色之上的前景"
        );
    }

    #[test]
    fn hover_stroke_only_on_enabled_cells() {
        let theme = MetroTheme::ether_dark();
        let mut grid = MetroTileGrid::new(1, 2);
        let geo = grid.layout(Rect::new(0.0, 0.0, 200.0, 56.0));

        grid.set_hovered(Some(0));
        assert_eq!(grid.hovered(), Some(0));
        let mut scene = Scene::default();
        grid.render(&theme, &geo, &mut scene);
        let strokes: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::StrokeRect {
                    color,
                    rect,
                    thickness,
                    ..
                } => Some((*color, *rect, *thickness)),
                _ => None,
            })
            .collect();
        assert_eq!(strokes.len(), 1, "只描悬停那一格，且不换底");
        assert_eq!(strokes[0].1, geo.cell(0).unwrap());
        assert_eq!(strokes[0].2, 2.0);
        assert_eq!(
            strokes[0].0,
            theme
                .colors
                .on_surface
                .with_alpha(theme.indication.base_medium)
        );

        // 禁用格不描边，底色与前景降不透明度。
        grid.set_disabled(0, true);
        let mut scene = Scene::default();
        grid.render(&theme, &geo, &mut scene);
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::StrokeRect { .. })),
            "禁用格不画悬停描边"
        );
        let fill = match &scene.commands[0] {
            SceneCommand::FillRect { color, .. } => *color,
            other => panic!("首条应是底色：{other:?}"),
        };
        assert_eq!(
            fill,
            theme
                .colors
                .surface_variant
                .with_alpha(theme.indication.disabled_opacity)
        );
        // 越界悬停值归 None，不 panic。
        grid.set_hovered(Some(99));
        assert_eq!(grid.hovered(), None);
    }

    #[test]
    fn labels_use_body_small_in_the_label_box() {
        let theme = MetroTheme::ether_dark();
        let grid = MetroTileGrid::new(1, 1).with_labels(["所有设置"]);
        let geo = grid.layout(Rect::new(0.0, 0.0, 100.0, 56.0));
        let mut scene = Scene::default();
        grid.render(&theme, &geo, &mut scene);
        let text = scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::Text {
                    content,
                    rect,
                    style,
                    align,
                    ..
                } => Some((content.clone(), *rect, style.size, *align)),
                _ => None,
            })
            .expect("有标签应产出文本命令");
        assert_eq!(text.0, "所有设置");
        assert_eq!(text.1, Rect::new(8.0, 34.0, 84.0, 16.0));
        assert_eq!(text.2, 12.0);
        assert_eq!(text.3, TextAlign::Left);
        // 空标签不画。
        let blank = MetroTileGrid::new(1, 1).with_labels([" "]);
        let mut scene = Scene::default();
        blank.render(&theme, &geo, &mut scene);
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { .. }))
        );
    }
}
