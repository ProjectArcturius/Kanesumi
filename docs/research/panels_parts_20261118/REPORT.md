# k-panels-parts 报告：为设置面板补四件控件（组头 / 图标位 / 通知行 / 方块网格）

> 卡：Ether `tools/dispatch/tasks/m1115/k-panels-parts.md`。上游 `m-panels-c0` 的快照与
> `docs/research/m1115_panels/REPORT.md`（在 Ether 仓，只读参考，未改一字）。
> 分支 `oc/k-panels-parts`，基线 `main@0ac06861`，工作树 `~/.oc/wt/k-panels-parts`。
> 执行机：Debian GNU/Linux 13、cargo / rustc 1.97.1、`CARGO_TARGET_DIR=~/.oc/target/k-panels-parts`。
> **卡面 §四 写的是 Fedora 门**；本机不是 Fedora，实际执行记录见 §六（差异与影响写在那里，
> 没有假装在 Fedora 上跑过）。只提交、不推送；未改 Ether 任何文件、未改 6 份 C0 转录、
> 未做应用迁移。

## 〇 · 结论

四件都是**新模块**，`panel_row.rs` 一个字未改（`git diff HEAD -- kanesumi-controls/src/panel_row.rs`
为空；卡面要求「旧出口不变」的 `PanelRow` / `MetroPanelRow` 与 Ether 侧 `SliderRow` 都不在
本件改动面内）。

| 件 | 模块 | 一句话 |
|---|---|---|
| `MetroSectionHeader` | `kanesumi-controls/src/section_header.rs` | caption 小字 + 次级墨色，行高 = 18 + 10 + 4 = 32 |
| `MetroIconSlot` | `kanesumi-controls/src/icon_slot.rs` | 槽内居中的 20 pt 方框；无图零命令 |
| `MetroNoticeRow` | `kanesumi-controls/src/notice_row.rs` | 标题 + 换行正文 + 关闭叉 + 悬停 / 按下底，四段逐段损伤 |
| `MetroTileGrid` | `kanesumi-controls/src/tile_grid.rs` | 等分格 + 标签 + 逐格命令区间与损伤，图标归调用方 |

门的实际读数（§六 有完整日志）：

- `cargo test --offline -p kanesumi-controls --lib` → **1024 passed / 0 failed / 0 ignored**
  （基线 997 + 新增 27）；
- `cargo test --offline -p kanesumi-element --test api_surface` → **1 passed**；
- `cargo clippy --offline -p kanesumi-controls --all-targets` → **退出 0**，告警行数与基线
  逐条相同（44 行），四个新文件零告警；
- `! grep -q "#\[ignore\]" kanesumi-controls/src/panel_row.rs` → **退出 0**。

`docs/API_SURFACE.txt`：**新增 58 行、移除 0 行**（纯增量，见 §三-3.4）。

## 一 · API 形状

### 1.1 共同形状：几何值对象 + 渲染 + 命中

四件都照 `panel_row.rs` 的既有分工：`measure` / `layout` 产出**几何值对象**，`render` 只消费它，
命中与逐段损伤也从同一份几何取。不引入隐藏的「上次量测」状态，也不让绘制与命中各算一遍。

```text
MetroSectionHeader::measure(origin, width)            -> SectionHeaderGeometry { rect, text }
MetroNoticeRow::measure(engine, origin, width)        -> NoticeRowGeometry { row, title, body, close }
MetroTileGrid::layout(area)                           -> TileGridGeometry { rect, cells }
MetroIconSlot::rect(slot)                             -> Rect（居中方框，无需几何对象）
```

### 1.2 `section_header.rs`

```rust
pub struct MetroSectionHeader { /* text, pad_top: 10, pad_bottom: 4 */ }
impl MetroSectionHeader {
    pub fn new(text: impl Into<String>) -> Self;
    pub fn with_padding(mut self, top: f32, bottom: f32) -> Self; // 负值夹 0、NaN 回落缺省
    pub fn text(&self) -> &str;
    pub fn is_empty(&self) -> bool;      // trim 后为空
    pub fn height(&self) -> f32;          // caption.line_height + pads
    pub fn measure(&self, origin: Point, width: f32) -> SectionHeaderGeometry;
    pub fn render(&self, theme: &MetroTheme, geo: &SectionHeaderGeometry, scene: &mut Scene) -> Option<Rect>;
}

pub struct SectionHeaderGeometry { pub rect: Rect, pub text: Rect }
impl SectionHeaderGeometry {
    pub fn rect(&self) -> Rect;       // 整行盒 —— 损伤
    pub fn text_rect(&self) -> Rect;  // 文本行盒
}
```

`render` 有文本返回 `Some(geo.rect)`，空文本返回 `None` 且零命令。

### 1.3 `icon_slot.rs`

```rust
pub struct MetroIconSlot { /* size: 20 */ }   // Default = ICON_20
impl MetroIconSlot {
    pub fn new() -> Self;
    pub fn with_size(mut self, pt: f32) -> Self;  // 负值夹 0、NaN 回落 20
    pub fn size(&self) -> f32;
    pub fn rect(&self, slot: Rect) -> Rect;       // 居中方形；槽小则夹到短边
    pub fn render(&self, theme: &MetroTheme, slot: Rect, icon: Option<&Icon>, scene: &mut Scene) -> Option<Rect>;
}
```

`render` 无图返回 `None`、零命令；有图在 `rect(slot)` 上 `scene.image`（染色 `on_surface`）
并返回该矩形。控件不接文件路径、不解码 SVG。

### 1.4 `notice_row.rs`

```rust
pub enum NoticeHit { Row, Close }

pub struct MetroNoticeRow { /* title, body, max_lines, close_visible, hovered, close_hovered, pressed */ }
impl MetroNoticeRow {
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self;
    pub fn with_max_lines(mut self, lines: usize) -> Self;  // 0 = 收起（只有标题）
    pub fn with_close(mut self, visible: bool) -> Self;
    pub fn title(&self) -> &str;
    pub fn body(&self) -> &str;
    pub fn max_lines(&self) -> usize;
    pub fn close_visible(&self) -> bool;
    pub fn set_hovered(&mut self, on: bool);
    pub fn set_close_hovered(&mut self, on: bool);
    pub fn set_pressed(&mut self, on: bool);
    pub fn state(&self) -> ControlState;   // 按下 > 悬停 > 常态
    pub fn is_empty(&self) -> bool;        // 标题与正文都无墨迹
    pub fn measure(&self, engine: &TextEngine, origin: Point, width: f32) -> NoticeRowGeometry;
    pub fn render(&self, theme: &MetroTheme, geo: &NoticeRowGeometry, scene: &mut Scene);
}

pub struct NoticeRowGeometry { pub row: Rect, pub title: Rect, pub body: Option<Rect>, pub close: Option<Rect> }
impl NoticeRowGeometry {
    pub fn rect(&self) -> Rect;
    pub fn title_rect(&self) -> Rect;
    pub fn body_rect(&self) -> Option<Rect>;
    pub fn close_rect(&self) -> Option<Rect>;
    pub fn damage(&self) -> Vec<Rect>;                    // 逐段损伤（跳过空段）
    pub fn hit(&self, pos: Point) -> Option<NoticeHit>;  // Close 优先
}
```

正文行数在 `measure` 里用 `TextEngine::layout` 数（实际字形），`render` 用
`scene.text_with_options(wrap = true, max_lines, Ellipsis)`；应用侧不再 `wrap` 或估字符宽。

### 1.5 `tile_grid.rs`

```rust
pub const TILE_ICON_SIZE: f32 = sizes::ICON_20; // 唯一公开的尺寸常量：语义别名引用尺寸表

pub struct MetroTileGrid { /* rows, cols, gap: 2, tile_h: 56, labels, on, disabled,
                             hovered, icon: 20 / 8 / 8 */ }
impl MetroTileGrid {
    pub fn new(rows: usize, cols: usize) -> Self;
    pub fn with_gap(mut self, gap: f32) -> Self;
    pub fn with_tile_height(mut self, h: f32) -> Self;
    pub fn with_labels(mut self, labels: impl IntoIterator<Item = impl Into<String>>) -> Self;
    pub fn with_icon_box(mut self, size: f32, pad_x: f32, pad_y: f32) -> Self;
    pub fn with_on(mut self, index: usize, on: bool) -> Self;
    pub fn with_disabled(mut self, index: usize, disabled: bool) -> Self;
    pub fn rows(&self) -> usize;          // 同形：cols / gap / tile_height / count / height
    pub fn label(&self, index: usize) -> Option<&str>;
    pub fn is_on(&self, index: usize) -> bool;
    pub fn is_disabled(&self, index: usize) -> bool;
    pub fn set_on(&mut self, index: usize, on: bool);
    pub fn set_disabled(&mut self, index: usize, disabled: bool);
    pub fn set_hovered(&mut self, index: Option<usize>);  // 越界归 None
    pub fn hovered(&self) -> Option<usize>;
    pub fn layout(&self, area: Rect) -> TileGridGeometry;
    pub fn icon_rect(&self, cell: Rect) -> Rect;   // 调用方 blit 的方框
    pub fn label_rect(&self, cell: Rect) -> Rect;  // body_small 一行，左下
    pub fn render(&self, theme: &MetroTheme, geo: &TileGridGeometry, scene: &mut Scene) -> Vec<TileCellDraw>;
}

pub struct TileGridGeometry { pub rect: Rect, pub cells: Vec<Rect> }
impl TileGridGeometry {
    pub fn rect(&self) -> Rect;
    pub fn rects(&self) -> &[Rect];                 // 格数 = rows × cols，与面积无关
    pub fn cell(&self, index: usize) -> Option<Rect>;
    pub fn hit(&self, pos: Point) -> Option<usize>; // 格缝 / 格外 / 零尺寸格 → None
    pub fn damage(&self, index: usize) -> Option<Rect>;
}

pub struct TileCellDraw { pub index: usize, pub rect: Rect, pub damage: Rect,
                          pub commands: std::ops::Range<usize> }
```

### 1.6 导出方式

四个模块按 `panel_row` 的既有风格只 `pub mod`，不做根重导出（`lib.rs` 只加四行
`pub mod …;`）。理由：`panel_row.rs` 就是面板族的现状 —— `MetroPanelRow` /
`PanelRowGeometry` 也只能走 `kanesumi_controls::panel_row::…`，四个新件与它同族，
没有理由换一种访问方式（要根重导出应连带 `panel_row` 一起做，那是另一件）。

## 二 · 改动清单

| 文件 | 改动 |
|---|---|
| `kanesumi-controls/src/section_header.rs` | 新增 `MetroSectionHeader` / `SectionHeaderGeometry` + 4 条回归 |
| `kanesumi-controls/src/icon_slot.rs` | 新增 `MetroIconSlot` + 6 条回归 |
| `kanesumi-controls/src/notice_row.rs` | 新增 `MetroNoticeRow` / `NoticeRowGeometry` / `NoticeHit` + 9 条回归 |
| `kanesumi-controls/src/tile_grid.rs` | 新增 `MetroTileGrid` / `TileGridGeometry` / `TileCellDraw` + 8 条回归 |
| `kanesumi-controls/src/lib.rs` | 只加 4 行 `pub mod …;`（按字母序插入），无重导出、无其他改动 |
| `docs/CONTROL_SPEC.md` | 追加 §55（四件规格 + 共同约定 + 未做 / 边界） |
| `docs/API_SURFACE.txt` | 重新生成（+58 / −0） |
| `docs/research/panels_parts_20261118/REPORT.md` | 本报告 |

`kanesumi-core` / `kanesumi-canvas` / `kanesumi-element` 未改：颜色全用现存令牌，文本放置全走
`Scene::label` / `Scene::text_with_options` / `TextEngine::layout`，没有为了四件下沉新原语。
`panel_row.rs` **零改动**（`git diff HEAD -- kanesumi-controls/src/panel_row.rs` 为空）。
没有新增依赖（`kanesumi-controls/Cargo.toml` 未动）。

## 三 · 真值取证

### 3.1 取证口径

期望值只来自两处，都是**独立取**的只读材料：

- 应用侧源码常量（Ether 仓 `settings/src/topbar/{action_center,net_panel,panel_rows}.rs`）：
  `GRP_TOP/GRP_BOTTOM`、`NOTE_TOP/NOTE_BOTTOM/NOTE_TITLE_H/NOTE_GAP/NOTE_BODY_H/NOTE_CLOSE`、
  `TILE_H/TILE_GAP/TILE_PAD/TILE_PAD_Y/TILE_ICON`、`ICON_PT/ICON_BTN/ROW_H/HD_H`；
- 冻结 C0 转录（`settings/tests/transcripts/panels/*.txt`，6 份，只读未改）：
  「展开 58 / 收起 38」「首行静置底色 = 面板底」「ton 方块 = 强调色 / 禁用 = 0.38 叠色」。

**没有**用被测函数生成 expected：测试里的 32 / 18 / 30 / 58 / 76 / 38 / 296 / 320 / 328 /
80.5 / 148 / 64 / 136 / 72 全是按上表常量手算后写成的字面量（例：两行行高
`8 + 20 + 2 + 18×2 + 10 = 76`；2×4 网格首格 `(616 − 3×8)/4 = 148`）。四个模块合计
`Rect::new` 只出现在几何计算与断言里，没有第二份「按像素试出来」的数。

### 3.2 逐条真值与取证命令

下表每行的命令都是单测粒度，可逐条复跑（`cargo test --offline -p kanesumi-controls --lib <名字>`）。

| 要求 | 读数（字面量） | 取证命令（后缀） |
|---|---|---|
| 节头行高 | `18 + 10 + 4 = 32`；`with_padding(12,0)` → 30；`(0,0)` → 18；`NaN` → 32 | `section_header::tests::default_height_is_line_height_plus_padding` |
| 节头文本盒 | 原点 `(16,40)`、宽 328 → 整行 `(16,40,328,32)`、文本 `(16,50,328,18)` | `section_header::tests::measure_puts_text_one_pad_below_origin` |
| 节头视觉 | 1 条 Text：13 pt / 行高 18 / `on_surface_variant` / 左对齐 / `max_lines=1` / 不换行 | `section_header::tests::paints_one_caption_label_in_secondary_ink` |
| 节头空输入 | 零命令；行高仍 32 | `section_header::tests::empty_text_paints_nothing` |
| 图标位居中 | 32×32 槽 → `(22,10,20,20)`（`(32−20)/2 = 6`） | `icon_slot::tests::default_is_20_centered_in_a_32_slot` |
| 图标位夹紧 | 12×12 槽 → `(0,0,12,12)`；40×28 槽 → `(10,4,20,20)` | `icon_slot::tests::smaller_slot_clamps_and_recenters` |
| 图标位尺寸 | 24 pt → `(4,4,24,24)`；0 → 零矩形；`NaN` → 回 20 | `icon_slot::tests::custom_size_and_zero_size` |
| 图标位无图 | `None` + 零命令（含零尺寸槽） | `icon_slot::tests::no_icon_paints_nothing`、`icon_slot::tests::zero_sized_slot_never_emits_an_image` |
| 图标位出图 | 1 条 Image 在 `(22,10,20,20)`、染色 `on_surface`、20×20 | `icon_slot::tests::icon_paints_one_tinted_image_at_the_centered_box` |
| 通知行高 | 两行 **76** / 一行 **58** / `max_lines=0` **38**（与 C0 的 58 / 38 同值） | `notice_row::tests::two_line_row_is_76_one_line_is_58_and_collapsed_is_38` |
| 通知行四段（360 宽） | row `(0,0,360,76)`、title `(16,8,296,20)`、close `(320,6,24,24)`、body `(16,30,328,36)`；damage 四段按此序 | `notice_row::tests::segments_match_the_app_constants` |
| 叉不占位 | `with_close(false)` → title 宽 **328**、close `None`、damage 3 段 | `notice_row::tests::hidden_close_reclaims_the_title_width` |
| 命中优先 Close | `(332,18)` / `(320,6)` → Close；`(100,18)` → Row；`(400,18)` / `(100,58)` → None；叉隐藏时 `(332,18)` → Row | `notice_row::tests::hit_prefers_close_then_row` |
| 悬停 / 按下底 | 静置**不铺底**；悬停 → `indication.hover_tint` 铺整行；按下 → `press_tint` 且压过悬停 | `notice_row::tests::hover_and_press_paint_the_row_background` |
| 正文换行 / 省略 | 1 条 Text：`wrap=true`、`max_lines=Some(2)`、`Ellipsis` | `notice_row::tests::body_wraps_with_ellipsis_and_max_lines` |
| 关闭叉自绘 | 4 个 Triangle；默认 `on_surface_variant`，悬停叉 → `on_surface` 且不动行底 | `notice_row::tests::close_glyph_is_four_triangles_and_brightens_on_hover` |
| 通知行空输入 | 零命令（连悬停底也不画），行高仍 38 | `notice_row::tests::empty_row_paints_nothing_but_keeps_its_slot` |
| 通知行极窄 | 宽 20 → 无正文段、标题宽夹 0、行高 38 | `notice_row::tests::zero_width_has_no_body_but_still_measures` |
| 方块网格（卡面例） | 2×4、gap 8、格高 64、宽 616 → 8 格；首格 `(0,0,148,64)`、次 `(156,0,148,64)`、第 5 格 `(0,72,148,64)`、高 136 | `tile_grid::tests::two_by_four_gap_8_first_cell_is_148_by_64` |
| 方块网格（应用几何） | 328 宽、gap 2、格高 56 → 格宽 80.5、x `16 / 98.5 / 181 / 263.5`、第二行 y 158、高 114；图标 `(24,108,20,20)`、标签 `(24,134,64.5,16)`；18/8/6 图标 `(24,106,18,18)` | `tile_grid::tests::action_center_geometry_matches_the_app_constants` |
| 网格命中 | 格内 → 格号；格缝 `(148,10)`、行缝 `(10,70)`、格外、网格下缘 → None | `tile_grid::tests::hit_skips_gaps_and_outside` |
| 网格空 / 零面积 | 0 维 → 0 格 0 命令；面积 0 → 8 格（零宽，塌到原点）、0 命令、`damage(0)=None` | `tile_grid::tests::empty_and_zero_area_grids_stay_pixel_free` |
| 逐格损伤 | 8 条 draw：`damage = 本格`、`commands` 连续且并集 = 全部命令；8 底 + 2 标签 | `tile_grid::tests::each_cell_gets_its_own_damage_and_command_range` |
| 方块颜色 | 关 = `surface_variant` + `on_surface`；开 = `primary` + `on_primary`；标签 12 pt | `tile_grid::tests::on_and_off_colors_come_from_tokens` |
| 方块悬停 / 禁用 | 悬停 2 pt 描边（`on_surface` × `base_medium`）只描一格且不换底；禁用不描边、底色 alpha 0.38；越界悬停归 None | `tile_grid::tests::hover_stroke_only_on_enabled_cells` |
| 方块标签 | `body_small` 12 pt、左对齐、盒 `(8,34,84,16)`；空白标签不画 | `tile_grid::tests::labels_use_body_small_in_the_label_box` |

一次跑完四组：

```bash
cargo test --offline -p kanesumi-controls --lib -- section_header icon_slot notice_row tile_grid
# → test result: ok. 27 passed; 0 failed; 0 ignored; 0 measured; 997 filtered out
```

**读数的来源边界（如实说）**：`notice_row` 的「两行 / 一行」取决于本机字体度量（
`TextEngine::layout` 按实际字形换行），测试用「远短于 328 宽」与「约 728 pt 长」两段文本把
边界撑开，换字体也不该翻。行高公式本身与字体无关。

### 3.3 现有用例变更表

**无。** 没有删改任何既有用例的输入、断言、夹具或忽略项；只追加了 27 条（4 + 6 + 9 + 8）。
证据：`git diff --stat` 里没有 `kanesumi-controls/src/panel_row.rs`、没有既有测试文件；
`--lib` 总数 997（基线）→ 1024（+27，0 移除），`0 ignored` 两侧一致。
`#[ignore]` 未新增（验收命令单独检查 `panel_row.rs`，并按 §六 全仓复查四个新文件）。

### 3.4 API 增删

`cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt`：

- **新增 58 行、移除 0 行**（`diff <(git show HEAD:docs/API_SURFACE.txt) docs/API_SURFACE.txt`：
  `>` 58 行、`<` 0 行）。
- 新增 9 个公开类型：`MetroSectionHeader`、`SectionHeaderGeometry`、`MetroIconSlot`、
  `MetroNoticeRow`、`NoticeRowGeometry`、`NoticeHit`、`MetroTileGrid`、`TileGridGeometry`、
  `TileCellDraw`；其余 49 行是方法签名（去重后）。
- 旧签名一行未删：`PanelRow` / `MetroPanelRow` / `PanelRowGeometry` 等原样在快照里。

## 四 · 没做的 / 边界

1. **未迁应用**（`m-panels-c1` 的事）：Ether 的 `action_center.rs` / `net_panel.rs` /
   `panel_rows.rs` 一个字未改，46 处 `Rect::new` 仍是 46 处。本件只补提供方。
2. **通知行没有时间列**。卡面 §一 的最小契约只列「标题 / 正文 / 关闭叉 / 整行底」四段；
   C0 里 `NoteRow` 还有一条 44 宽的 `HH:MM` 时间列（`NOTE_TIME_W`）。本控件按卡面四段做，
   时间列留给 c1 决定（要么并进标题行右侧，要么另开一件补 `with_time`）。不假装它存在。
3. **按下态是新增行为，会让一条 C0 转录变**。旧 `NoteRow` 没有按下态
   （`action_row_pointer.txt` 的 `press(note_row)` 段「底色与文字都没变」）；本控件按下铺
   `indication.press_tint`。这是卡面「整行悬停/按下底」要求的，迁移时属 `行为变更：`，
   须由 C2 裁定放行 —— 本件没有改转录。
4. **图标解码仍归调用方**：`MetroIconSlot::render` 只接受已光栅化的 `&Icon`；
   `MetroTileGrid` 干脆只给 `icon_rect`，不产 Image 命令。SVG / 资产管线不在本件。
5. **单选 / 多选、键盘、无障碍树角色未做**：四件都只给几何、绘制、命中与状态，没有
   `Widget` 实现、没有 `AccessRole`、没有 Tab 顺序。这与 `panel_row` 现状一致（它也不是
   `Widget`），但意味着 c1 若要进元素树，需要另一次接线。
6. **RTL 未做**：四件都按 LTR 几何，不做镜像（与 `MetroDock` 呈现层同一 TODO）。
7. **`cargo fmt --all` 基线本来就红**，本件只对四个新文件跑
   `rustfmt --edition 2024`（`--check` 退出 0），没有格式化范围外文件。
8. **四件的「未启用」靠调用方不调用实现**：没有布尔开关，也没有 `set_enabled(false)`；
   空输入（空文本 / 无图 / 0 行 / 0 维度 / 零面积）已经保证零像素。

## 五 · 范围外发现

1. `tab_view.rs` 的 `draw_close_x` 是私有函数，`notice_row` 为画叉复制了同一份四三角形配方。
   把它提到一个共享位置（例如 `kanesumi-controls/src/glyph.rs`）会更省，但那要动既有控件的
   模块边界，超出本卡「不改既有控件」的约束。
2. `sizes` 里仍没有面板行的高 / 内距 / 图标槽 / 方块格高令牌（m1115 REPORT §四-5 已记）。
   本件**没有**在 `kanesumi-core` 另立 `sizes` 项（那会改动 core 的公开表，需另开件），也
   **没有**新增 `const f32`（`kanesumi-controls/tests/size_discipline.rs` 的裸尺寸白名单只减
   不增）：可配的数落成字段缺省，固定的布局数写成 `measure` 里的具名局部量并注明出处。
   于是这些数仍是「面板规格的副本」而不是「尺寸表的一员」—— core 补 token 时应回头替换。
3. `panel_row.rs` 自己的 12 / 20 / 24 / 36 仍是裸数字（它不在本卡「新增四件」的改动范围，
   且卡面要求「不改既有控件」）。若后续要把面板族常量统一，应从 `panel_row` 起改。
4. 通知行的 `measure` 需要 `&TextEngine`，而 `MetroSectionHeader` / `MetroTileGrid` 不需要。
   这不是疏漏：只有通知行的行高依赖正文实际行数。若 c1 的元素树 `measure` 拿不到引擎，
   会撞上一个已知的框架约束（`MeasureCtx` 不带 `TextEngine`），需要 c1 自己解决（缓存引擎
   或按最大行数取高）。

## 六 · 门执行记录（2026-11-18，本机 Debian 13）

环境：`export PATH=$HOME/.cargo/bin:$HOME/.local/bin:$PATH`、`CARGO_BUILD_JOBS=8`，
`CARGO_TARGET_DIR=~/.oc/target/k-panels-parts`（会话环境自带）。日志在
`~/.oc/out/k-panels-parts/`。

| 命令 | 结果 |
|---|---|
| `cargo test --offline -p kanesumi-controls --lib` | **通过**：1024 passed / 0 failed / 0 ignored（基线 997） |
| `cargo test --offline -p kanesumi-element --test api_surface` | **通过**：1 passed / 0 failed |
| `cargo clippy --offline -p kanesumi-controls --all-targets` | **通过**：退出 0；`warning` 行 44 条 = 基线 44 条，四个新文件 0 条 |
| `! grep -q "#\[ignore\]" kanesumi-controls/src/panel_row.rs` | **通过**：退出 0 |
| `grep -c "#\[ignore\]" kanesumi-controls/src/{section_header,icon_slot,notice_row,tile_grid}.rs` | 0 / 0 / 0 / 0 |
| `cargo test --offline -p kanesumi-controls`（整包：lib + 14 个集成测试目标） | **通过**：lib 1024 + 各集成目标全 ok（含 `size_discipline` 2、`token_discipline` 2） |
| `cargo test --offline --workspace --features kanesumi-harness/a11y` | **通过**：退出 0，全部 `test result: ok`（见 §6.2） |

### 6.1 workspace 门抓到的真问题（已修，留档）

第一次跑 workspace 门时红在：

```text
test no_new_bare_size_consts_in_production_code ... FAILED
error: test failed, to rerun pass `-p kanesumi-controls --test size_discipline`
```

`kanesumi-controls/tests/size_discipline.rs` 是 N18 静态检查：**生产段不得新增
`const NAME: f32 = <整值>;`**，白名单「只减不增」。我第一版四个模块里有一批
`pub const NOTICE_TOP: f32 = 8.0;` 这类常量（卡面「尺寸不许出现裸常量」的直观写法），
正好撞在枪口上。修法不是往白名单加行（那是让检查失效），而是：

- 可配的尺寸 → **字段缺省**（`MetroSectionHeader` 的 10 / 4、`MetroTileGrid` 的间隔 2 /
  格高 56 / 图标 8 / 8 / 20）；
- 与面板规格同源的固定布局数 → `measure` 里的**具名局部量**，模块顶部注释写明出处；
- 只留 `TILE_ICON_SIZE = sizes::ICON_20` 这一条**语义别名引用尺寸表**（规则明确放行）。

修后 `size_discipline` 2 passed、workspace 门全绿。这条是本卡三条验收命令**抓不到**的
（它们不跑集成测试），记在这里以免下一个人重犯。

### 6.2 与卡面的差异：本机不是 Fedora

卡面 §四 写「门（Fedora…）」并预告 workspace 门在 Fedora 上会因宿主字体栈红在
`kanesumi-canvas::font_index::tests::real_stack_identity_matches_text_engine`。本机是
**Debian 13 / rustc 1.97.1**（工人守则禁止换机器跑验收），所以：

- 上面各条门按卡面命令**原样**在本机执行（未改写、未合并成一条）；
- workspace 门在本机**全绿**（`~/.oc/out/k-panels-parts/workspace-a11y-2.log`，退出 0）。
  这只是「Debian 上绿」，**不能**据此宣称 Fedora 上也绿 —— 卡面说的那条红是宿主字体栈
  问题，与本次改动无关，本机没有复现。

## 七 · 格式与提交

- `cargo fmt --all -- --check` 基线本来就红（约 190 文件），未顺手格式化范围外文件。
- 本件只对四个新文件执行 `rustfmt --edition 2024`，`--check` 退出 0；`lib.rs` 只多了四行
  `pub mod`，未重排既有内容。
- 提交（只提交、不推送）：
  1. `feat(controls): 补面板部件四件（节头 / 图标位 / 通知行 / 方块网格）。`
     —— 四个新模块 + 27 条回归 + `docs/CONTROL_SPEC.md` §55 + `docs/API_SURFACE.txt`；
  2. `docs(controls): 记录面板部件四件的真值取证与门执行。` —— 本报告。
- 未改：`kanesumi-core` / `kanesumi-canvas` / `kanesumi-element` / `kanesumi-harness`、
  任何既有控件与测试、Ether 仓任何文件、6 份 C0 转录、`Cargo.toml` / `Cargo.lock`。
- 合入与指针前推由调度者做；之后另开 `m-panels-c1`（含 C2 裁定：哪些转录行允许变，
  首当其冲是 §四-3 的按下态那一行）。
