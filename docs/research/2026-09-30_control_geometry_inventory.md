# 控件几何契约盘点（2026-09-30，M2-3 前置调研）

**共 46 个控件 / 45 个文件。**

目的：`docs/ROADMAP.md` M2「布局接管」要求所有控件的矩形由布局引擎产出；本盘点记录
`kanesumi-controls/src/` 下每个公开控件**现在**的几何相关 API，供调度者设计统一控件契约。
布局引擎目标契约见 `kanesumi-canvas/src/layout.rs:80-92`（`LayoutLeaf`：
`measure(engine, available) -> Size` + `render(theme, engine, rect, scene)`）。

## 〇、统计口径与命令

- 范围：`kanesumi-controls/src/*.rs`，排除 `lib.rs`（模块清单）。
- 控件判定：`pub struct Metro*`，或 `pub struct` 且有 `render` 方法（后者本仓为零，全部控件均带 `Metro` 前缀）。
- `MetroTab`（`tab_row.rs`）是标签数据模型而非控件，不计入 46；`MetroSurface`（`surface.rs`）有 `render`，计入。
- 逐文件“生产段”指该文件首个 `#[cfg(test)]` 之前的行；`Rect::new` 计数即统计此段。

用到的核对命令（在 `kanesumi-controls/src/` 下执行）：

```bash
# render / measure / 命中函数签名（截断多行签名的首行）
rg -n "pub fn (render|measure|hit)" *.rs

# 生产段 Rect::new 计数（首个 #[cfg(test)] 之前的行）
for f in *.rs; do t=$(rg -n '#\[cfg\(test\)\]' "$f" | head -1 | cut -d: -f1); \
  [ -z "$t" ] && t=$(wc -l < "$f"); \
  c=$(awk -v t="$t" 'NR<t && /Rect::new/ {n++} END{print n+0}' "$f"); \
  printf "%-28s prodRect=%s\n" "$f" "$c"; done

# 生产段 push_clip（每种 clip 成对，计数即对数）
rg -c "push_clip" *.rs
```

表列约定：`render 签名` 原样抄参数列表（去 `&self`）；`measure`/`hit_test` 有则抄签名；
`其他命中` 只列名字；`Rect::new` 为该文件生产段计数（`progress.rs` 的 Bar/Ring 共用同一文件，故两行同值）；
`画出 rect 之外` 指 render 是否可能画到传入矩形之外；`clip` 指生产段是否 `push_clip`。

## 一、逐控件表

| 文件 | 类型 | render 签名（去 &self） | measure | hit_test | 其他命中函数 | Rect::new | 画出 rect 之外？ | clip |
|---|---|---|---|---|---|---|---|---|
| animated_icon.rs | MetroAnimatedIcon | `theme: &MetroTheme, rect: Rect, scene: &mut Scene` | 无 | 无 | — | 0 | 否：三角由 rect 中心 ±30% 推导 | 否 |
| auto_suggest_box.rs | MetroAutoSuggestBox | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene`（`&mut self`） | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | `hit_item` | 9 | 是：建议弹层画在输入框 rect 下方 | 是 |
| breadcrumb_bar.rs | MetroBreadcrumbBar | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, screen: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine) -> Size` | 无 | `hit`, `hover` | 7 | 是：折叠省略号下拉菜单画在 bar rect 之外 | 否 |
| button.rs | MetroButton | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine, style: TextStyle) -> Size` | `hit_test(&self, rect: Rect, pos: kanesumi_core::Point) -> bool` | — | 1 | 否：底/交互/标签夹在 rect 内 | 否 |
| candidate_window.rs | MetroCandidateWindow | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（有 `popup_size`） | 无 | `hit_candidate` | 4 | 是：内容宽超面板时右侧候选越界（仅按起点 break） | 否 |
| check_box.rs | MetroCheckBox | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | — | 2 | 否 | 否 |
| color_picker.rs | MetroColorPicker | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self) -> kanesumi_core::Size` | 无 | `hover`, `press`, `handle_click`, `drag_to`, `release` | 11 | 是：满值滑块拇指右缘越出 rect 约 5px | 否 |
| command_bar_flyout.rs | MetroCommandBarFlyout | `theme: &MetroTheme, _engine: &TextEngine, scene: &mut Scene`（无 rect） | 无（有 `panel_size`） | 无 | `hit_command`, `hover` | 3 | 是：无 rect，仅按 `panel_rect` 画浮层 | 否 |
| context_menu.rs | MetroContextMenu | `theme: &MetroTheme, engine: &TextEngine, scene: &mut Scene`（无 rect） | 无（有 `panel_size`） | 无 | `hover`, `path_at` | 0 | 是：指针锚定面板与级联子菜单在触发控件之外 | 否 |
| dialog.rs | MetroDialog | `theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene` | 无（有 `box_rect`） | 无 | `hit_button` | 6 | 否：收整屏 screen，遮罩与盒体都在 screen 内 | 否 |
| drop_down_button.rs | MetroDropDownButton | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, screen: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine, style: TextStyle) -> kanesumi_core::Size` | `hit_test(&self, rect: Rect, pos: Point) -> bool` | `item_at`, `hover`, `press`, `release` | 2 | 是：MenuFlyout 弹层画在按钮 rect 之外 | 否 |
| dropdown_menu.rs | MetroDropdownMenu | `render`: `theme: &MetroTheme, engine: &TextEngine, screen: Rect, scene: &mut Scene`；另 `render_panel`/`render_submenu` 无 rect，用 `panel_rect`/`sub.panel` | 无（有 `panel_size`） | 无 | `item_at`, `path_at`, `hover` | 14 | 是：面板画在 `panel_rect`，子菜单再画在父面板之外 | 是 |
| expander.rs | MetroExpander | 无总 render；`render_header`: `theme, engine: &TextEngine, rect: Rect, scene`；`render_content`: `theme, rect: Rect, scene` | 无 | 无 | `hit_header` | 14 | 否：越界裁剪交宿主 `content_clip` | 否 |
| icon_button.rs | MetroIconButton | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self) -> Size` | `hit_test(&self, rect: Rect, pos: kanesumi_core::Point) -> bool` | — | 3 | 否 | 否 |
| info_badge.rs | MetroInfoBadge | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine) -> Size` | 无 | — | 2 | 否 | 否 |
| info_bar.rs | MetroInfoBar | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（私有 `layout`） | 无 | `hit`, `handle_click` | 11 | 否：仅当 rect 过矮时内容高理论上向下延伸 | 否 |

| list.rs | MetroList | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（有 `content_height`/`row_height`） | 无 | — | 2 | 否：`push_clip(rect)` 裁掉半滚出的行 | 是 |
| menu_bar.rs | MetroMenuBar | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, _screen: Rect, scene: &mut Scene` | 无（有 `header_width`/`total_width`/`flyout_height`） | 无 | `header_at`, `item_at`, `hover`, `press`, `release` | 2 | 是：flyout 在 `pop_clip` 之后画在 bar rect 之外 | 是 |
| metro_tile.rs | MetroTile | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | — | 10 | 否 | 否 |
| navigation_view.rs | MetroNavigationView | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（有 `pane_width`/`effective_pane_width`） | 无 | `hit`, `handle_click`, `hover` | 17 | 否：`push_clip(rect)` 包住 Toggle/项/子项 | 是 |
| number_box.rs | MetroNumberBox | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | `hit_spin` | 9 | 否 | 否 |
| pager_control.rs | MetroPagerControl | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine) -> Size` | 无 | `hit`, `handle_click`, `hover` | 16 | 否：`geom` 按 measure 结果在 rect 内居中 | 否 |
| parallax_view.rs | MetroParallaxView | 无（无 render） | 无 | 无 | — | 1 | N/A（非 render 控件） | 无 |
| password_box.rs | MetroPasswordBox | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene`（`&mut self`） | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | — | 0 | 否：委托 `MetroTextBox` 用同一 rect | 否 |
| person_picture.rs | MetroPersonPicture | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | 无 | — | 4 | 是：Badge 右上外溢 4px（`right+4`/`top-4`） | 否 |
| pips_pager.rs | MetroPipsPager | `theme: &MetroTheme, rect: Rect, scene: &mut Scene`（无 engine） | `measure(&self) -> (f32, f32)` | 无 | `click`, `handle_click` | 9 | 是：两侧 Nav 按钮由中心外扩，窄 rect 下越出 | 否 |
| progress.rs | MetroProgressBar | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | 无 | — | 3 | 是：高不足时轨道按最小高纵向越出 | 是 |
| progress.rs | MetroProgressRing | `theme: &MetroTheme, rect: Rect, scene: &mut Scene`（无 engine） | 无 | 无 | — | 3（与 Bar 共用文件） | 否：`size = min(self.size, 短边)` 后居中 | 否 |
| radio_buttons.rs | MetroRadioButtons | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine) -> Size` | 无 | `hit`, `handle_click`, `hover` | 6 | 是：列宽由 engine 排版决定，窄 rect 向右越出 | 否 |
| rating_control.rs | MetroRatingControl | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self) -> kanesumi_core::Size` | 无 | `hover`, `click` | 3 | 是：星自 rect.origin 按项目宽排布，窄 rect 向右越出 | 是 |
| repeater.rs | MetroRepeater | 无（无 render） | 无 | 无 | `item_at` | 4 | N/A（非 render 控件） | 无 |
| scroll_view.rs | MetroScrollView | 无（无 render） | 无 | 无 | — | 3 | N/A（非 render 控件） | 无 |

| selector_flyout.rs | MetroSelectorFlyout | `theme: &MetroTheme, _engine: &TextEngine, trigger: Rect, screen: Rect, scene: &mut Scene` | 无（有 `panel_height`） | 无 | `item_at` | 5 | 是：面板经 `render_panel_base` 画在触发器之外 | 是 |
| slider.rs | MetroSlider | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | `measure(&self, width: f32) -> Size` | `hit_test(&self, rect: Rect, pos: Point) -> bool` | `hover`, `press`, `drag_to`, `release` | 5 | 否：轨道左右内缩、拇指居中于轨道 | 否 |
| split_button.rs | MetroSplitButton | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, screen: Rect, scene: &mut Scene` | `measure(&self, engine: &TextEngine, style: TextStyle) -> Size` | 无 | `hit`, `hover`, `press`, `release` | 5 | 是：下拉 flyout 在按钮 rect 之外 | 否 |
| surface.rs | MetroSurface | `rect: Rect, scene: &mut Scene`（无 theme/engine） | 无 | 无 | — | 0 | 否：底与 tint 同一 rect | 否 |
| swipe_control.rs | MetroSwipeControl | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | 无 | `hit`, `hover`, `handle_click`, `press`, `drag_to`, `release` | 6 | 否：左右操作项矩形都在 rect 内 | 否 |
| switch.rs | MetroSwitch | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | `hit_test(&self, rect: Rect, theme: &MetroTheme, pos: Point) -> bool` | `press`, `drag_to`, `release`, `cancel` | 4 | 否：`right_avail` 夹到 `rect.right()` | 否 |
| tab_row.rs | MetroTabRow | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（有 `header_width`/`total_width`） | 无 | `tab_at` | 2 | 否：render 首尾 `push_clip(rect)` | 是 |
| tab_view.rs | MetroTabView | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | 无 | `hit`, `hover`, `handle_click` | 6 | 否：`push_clip(rect)` 包住页签 | 是 |
| teaching_tip.rs | MetroTeachingTip | `theme: &MetroTheme, _engine: &TextEngine, scene: &mut Scene`（无 rect） | 无（有 `panel_height`） | 无 | `hit`, `hover`, `handle_click` | 10 | 是：气泡按 `placement` 定点画在目标 rect 之外 | 否 |
| text.rs | MetroText | `_engine: &TextEngine, block: Rect, scene: &mut Scene`（无 theme） | `measure(&self, engine: &TextEngine, max_width: f32) -> Size` | 无 | — | 0 | 否：越界交 `TextOverflow`（默认 Clip） | 否 |
| text_box.rs | MetroTextBox | `theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene`（`&mut self`） | 无 | `hit_test(&self, rect: Rect, pos: Point) -> bool` | `place_caret_at` | 13 | 否：选区/文本 `push_clip(content)`，光标钳到 `body.right()` | 是 |
| title_bar.rs | MetroTitleBar | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无 | 无 | `hit`, `hover`, `handle_click` | 4 | 不确定：`title_rect` 的 `.max(48.0)` 极窄时可越右 | 否 |
| tree_view.rs | MetroTreeView | `theme: &MetroTheme, _engine: &TextEngine, rect: Rect, scene: &mut Scene` | 无（几何为 `layout`） | 无 | `hit`, `hover`, `handle_click` | 3 | 否：行绘制包在 `push_clip(rect)` 内 | 是 |
| two_pane_view.rs | MetroTwoPaneView | 无（无 render） | 无 | 无 | — | 5 | N/A（非 render 控件） | 无 |

合计 46 行（`progress.rs` 两行同文件）。

## 二、签名分类

### 2.1 render 参数形态（九类）

| 形态 | 成员 |
|---|---|
| `(theme, engine, rect, scene)` 标准四参 | auto_suggest_box、button、check_box、icon_button、info_badge、info_bar、list、metro_tile、navigation_view、number_box、pager_control、radio_buttons、rating_control、slider、switch、tab_row、text_box |
| `(theme, engine, rect, screen, scene)` 多一个 screen | breadcrumb_bar、drop_down_button、menu_bar（`_screen` 未用）、split_button |
| `(theme, engine, screen, scene)` 收 screen 不收 rect | dialog、dropdown_menu |
| `(theme, engine, trigger, screen, scene)` trigger+screen | selector_flyout |
| `(theme, engine, scene)` 完全无矩形 | command_bar_flyout、context_menu、teaching_tip |
| `(theme, rect, scene)` 无 engine | animated_icon、pips_pager、MetroProgressRing |
| `(engine, block, scene)` 无 theme | text |
| `(rect, scene)` 无 theme/engine | surface |
| 无总 render（分方法或纯几何） | expander（`render_header`+`render_content`）、parallax_view、repeater、scroll_view、two_pane_view |

补充：`&mut self` 的 render 有 3 个（auto_suggest_box、password_box、text_box），其余为 `&self`；
`password_box` 只是把同一 rect 转交内层 `MetroTextBox`。engine 参数存在但未使用的有 button、
candidate_window、color_picker、command_bar_flyout、icon_button、list、person_picture、MetroProgressBar、
selector_flyout、slider、swipe_control、tab_view、teaching_tip、text、title_bar、tree_view；
根本没有 engine 参数的是 animated_icon、pips_pager、MetroProgressRing、surface。

### 2.2 measure 签名形态

| 形态 | 成员 |
|---|---|
| `measure(&self, engine: &TextEngine) -> Size` | breadcrumb_bar、info_badge、pager_control、radio_buttons |
| `measure(&self, engine, style: TextStyle) -> Size` | button、drop_down_button、split_button |
| `measure(&self) -> Size` | icon_button、color_picker、rating_control |
| `measure(&self, width: f32) -> Size` | slider |
| `measure(&self, engine, max_width: f32) -> Size` | text |
| `measure(&self) -> (f32, f32)` | pips_pager |
| 无 measure，但有等价尺寸 API | dropdown_menu/context_menu/command_bar_flyout `panel_size`；selector_flyout/teaching_tip `panel_height`；list `content_height`/`row_height`；menu_bar/tab_row `header_width`/`total_width`/`flyout_height`；navigation_view `pane_width`；tree_view `layout`；two_pane_view `pane_rects` |
| 无任何尺寸 API | 其余（多数按传入 rect 直接自绘） |

### 2.3 命中函数形态

- `hit_test(..., pos) -> bool`（10 个）：auto_suggest_box、check_box、drop_down_button、metro_tile、
  number_box、password_box、slider、text_box（均 `(rect, pos)`）；button、icon_button（`pos: kanesumi_core::Point`）。
- 需要 theme 的 hit_test（1 个）：`MetroSwitch::hit_test(&self, rect, theme: &MetroTheme, pos) -> bool`。
- `hit(..., pos) -> 动作枚举/结构`（11 个）：breadcrumb_bar→BreadcrumbClick、info_bar→InfoBarClick、
  navigation_view→NavigationAction、pager_control→PagerAction、radio_buttons→Option<usize>、
  split_button→SplitButtonPart、swipe_control→SwipeAction、tab_view→TabViewAction、
  teaching_tip→TeachingTipClick、title_bar→TitleBarClick、tree_view→TreeAction。
- `hit_*` 细分命中：`hit_item`/`hit_candidate`/`hit_command`→Option<usize>/Option<CommandBarAction>，
  `hit_spin`→Option<SpinButton>，`hit_button`→Option<DialogButton>，`hit_header`→bool。
- 点/项查询：`item_at`、`path_at`、`tab_at`、`header_at`、`item_at`（repeater）→多为 `Option<usize>`/`Option<MenuPath>`。
- `hover(..., pos)` 普遍存在，返回 `()`、`bool` 或 `Option<usize>` 三种，无统一约定。
- 指针族辅助：`press`/`release`/`drag_to`/`click`/`handle_click`/`place_caret_at`/`cancel`，纯事件函数、不是纯命中。

## 三、会画出 rect 之外的控件清单（M2-1 `clips_children` 豁免候选）

判定依据是 render 相对**它实际收到的矩形**：收整屏 `screen` 的弹层（dialog/dropdown_menu）不算越界，
无 rect 形参、按自身锚点绘制的才算。共 **17 个「是」+ 1 个「不确定」**。

**甲类·弹层/浮层（几何本就由锚点决定，不该被父容器裁剪）**
- command_bar_flyout（无 rect，画 `panel_rect`）
- context_menu（无 rect，画指针锚定面板 + 级联子菜单）
- teaching_tip（无 rect，画 `placement` 气泡 + 尾巴）
- selector_flyout（画在 trigger 之外的面板）
- auto_suggest_box（建议弹层画在输入框下方）
- breadcrumb_bar（折叠省略号下拉菜单）
- drop_down_button / split_button（MenuFlyout 弹层）
- menu_bar（flyout 在 `pop_clip` 之后绘制）
- dropdown_menu（面板 + 二级子菜单，`render_panel`/`render_submenu`）

**乙类·内容按自身尺寸排布、窄 rect 下溢出（应裁剪或按约束收敛）**
- candidate_window（内容宽超面板时右侧候选越界）
- color_picker（满值滑块拇指右缘越出约 5px）
- person_picture（Badge 右上外溢 4px）
- pips_pager（两侧 Nav 按钮由中心外扩）
- MetroProgressBar（高不足时按最小高纵向越出，且 `push_clip` 只裁到 `bar_rect`）
- radio_buttons、rating_control（按排版/项目宽向右越出）

**不确定**：title_bar —— `title_rect` 的 `.max(48.0)` 在极窄 rect 下可能越过 `rect.right()`。

其余 24 个 render 控件或自带 `push_clip`（list、navigation_view、tab_row、tab_view、text_box、tree_view）、
或全部几何从 rect 派生并夹紧（button、check_box、icon_button、info_badge、metro_tile、number_box、
pager_control、password_box、slider、surface、swipe_control、switch、dialog、info_bar、MetroProgressRing、
text、expander、animated_icon），视为「否」。四个纯几何结构（parallax_view、repeater、scroll_view、
two_pane_view）无 render，记 N/A。

## 四、统一成一个 trait 时最难处理的 3 个控件

1. **MetroTeachingTip**：render 没有 rect 形参，位置存在 `placement` 里，本质是浮层而非布局叶子，
   无法满足 `render(rect)` 的「画在给定矩形内」契约。
2. **MetroDropdownMenu**：render 收整屏 `screen`，并拆出 `render_panel`/`render_submenu` 三套方法自管
   面板与级联子菜单，单一 rect 的 render 表达不了多面板与锚点关系。
3. **MetroTextBox**：render 取 `&mut self` 且在绘制前推进滚动（`ensure_caret_visible`），是有副作用的
   render，且要求量测、命中、IME caret 与绘制消费同一布局产物。

（同类难题还有 `MetroDialog`/`MetroContextMenu`/`MetroCommandBarFlyout` 等无 rect 弹层，此处只取前三。）

## 五、非控件模块简述

- `lib.rs`：模块声明与 `pub use` 清单，本盘点的排除项。
- `decl.rs`：声明式 `Decl` 元素树 + `view!` 宏 + `render_decl` 调和器。**它已经实现了 `LayoutLeaf`**
  （`DeclLeaf::measure`/`render`，见 decl.rs:207-226），并把叶子转交现有控件渲染与命中表 `DeclHit`，
  是 M2 统一契约的现成范例与首个实现者。
- `retained.rs`：`RetainedScene` 增量渲染（首帧全量，后续按 `diff_decl` 只重建变化段），持有 `DeclHit` 命中表。
- `focus.rs`：`FocusRing`/`FocusId` 焦点环（Tab 顺序 + 单焦点真源），不持有视觉树；控件用 `is_focused`
  决定是否画焦点环，是几何之外的关注点。
- `ime.rs`：`ImeContext`/`ImeContentHint` IME 状态，无几何。
- `state.rs`：`ControlState` 枚举（Normal/Hovered/Pressed/Focused/Disabled）+ 旧名别名，无几何。
- `text_field.rs`：`TextField`/`TextInputKey` 文本编辑内核（光标/选区/undo/IME 预编辑），无 render、
  非视觉控件；由 MetroTextBox/PasswordBox/NumberBox/AutoSuggestBox 复用。
- `popup.rs`：弹层摆放与动画辅助 —— `place_popup`/`place_submenu`/`PopupPlacement`/`PopupAnim`/
  `render_overlay`/`render_panel_base`，被多处弹层控件消费；本身不是控件。

