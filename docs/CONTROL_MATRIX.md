# Kanesumi 控件功能对照表（验收标准）

> 对照 `CONTROL_SPEC.md` 各章，一一对应到 Rust 实现。**验收标准：每一行「规格」均有对应「实现」与「测试」**。
> 状态列：✅ 已实现并有测试；🔶 已实现核心、部分能力待 Phase 3 续；❌ 未实现。

## 逐控件对照

| CONTROL_SPEC § | 控件 | Rust 模块 | 规格要点 | 实现 | 测试 |
|---|---|---|---|---|---|
| §1 Button | MetroButton | `button.rs` | Padding 8/5/8/6、四态硬切换、Accent 前景恒白、焦点外框 | ✅ | ✅ `renders_surface_and_label` / `focus_adds_stroke` / `disabled_reduces_alpha` / `hit_test_contains` |
| §2 IconButton | MetroIconButton | `icon_button.rs` | 68×56（纯图标 48）、图标 16+标签 12、常态透明底、hover 10%/press 20% 白 | ✅ | ✅ `measure_follows_label` / `renders_icon_and_label` / `hover_adds_tint` / `disabled_lowers_fg` |
| §3 Switch | MetroSwitch | `switch.rs` | 轨道 40×20 胶囊、knob 20、行程 20px、**0.15s** 滑动、On 轨道强调色 | ✅ | ✅ `travel_is_track_minus_knob` / `toggle_animates_to_target` / `toggle_is_interruptible` / `disabled_lowers_alpha` |
| §4 ProgressBar | MetroProgressBar | `progress.rs` | 确定 0.15s 滑动、不确定 **2.0s** 两波脉冲、Paused 0.6、Error 色 | ✅ | ✅ `determinate_slides_to_value` / `determinate_renders_indicator` / `indeterminate_cycles_two_pulses` / `indeterminate_phase_wraps` |
| §5 ProgressRing | MetroProgressRing | `progress.rs` | 32×32、线宽 4、确定 value×360、不确定 **2.0s** 旋转 0→900°、Inactive 隐藏 | ✅ | ✅ `ring_determinate_sweep_maps_value` / `ring_indeterminate_rotates` / `ring_inactive_hides` |
| §6 TabRow | MetroTabRow | `tab_row.rs` | 头高 48、24 SemiLight、选中管道 2px 强调色贴底、文字色选中最深 | ✅ | ✅ `header_height_matches_spec` / `tab_at_maps_x` / `selected_draws_pipe_only_on_selected` |
| §7 List | MetroList | `list.rs` | 选中强调色 0.60、行高下限 40、padding 12、滚动裁剪 | ✅ | ✅ `emits_row_texts` / `selection_highlights_row` / `scroll_skips_out_of_view_rows` |
| §8 ComboBox | MetroSelectorFlyout | `selector_flyout.rs` | 触发器高 32/箭头 32px、面板 MaxH 504、选中强调低透、遮罩 0.383/0.216s | ✅ | ✅ `toggle_opens_closes` / `panel_height_capped` / `item_at_maps_rows` / `renders_trigger_when_closed` |
| §8 MenuFlyout | MetroDropdownMenu | `dropdown_menu.rs` | 项高 32、图标 16、快捷键右、分隔线 1px、PointerOver 中性高亮、遮罩+面板动画 | ✅ | ✅ `toggle_open_close` / `item_at_maps_y` / `panel_size_grows_with_items` / `renders_panel_when_open` |
| §9 Dialog | MetroDialog | `dialog.rs` | 遮罩+盒体、淡入 0.167/淡出 0.083s、缩放 1.05→1.0 @0.5s、三按钮 P→S→C、默认按钮 Accent | ✅ | ✅ `show_hide_cycle` / `scale_animates_1_05_to_1` / `open_renders_scrim_box_and_buttons` / `box_rect_centers_and_clamps` |
| §8.5 MenuBar | MetroMenuBar | `menu_bar.rs` | 行高 40、header padding 10,4、Selected/Pressed 0.10 α、hover-swap（flyout 开时 hover 其它 header 自动切换）、点外关闭、点项返 (h,i) | ✅ | ✅ `header_at_maps_x` / `click_header_toggles_flyout` / `hover_swaps_flyout_when_open` / `click_flyout_item_returns_indices_and_closes` / `click_outside_flyout_closes_it` + 3 |
| §12 InfoBar | MetroInfoBar | `info_bar.rs` | MinHeight 48、ContentRoot Padding 16、图标 16px 方块+白色字形、横排/纵排判据、Close 38×38、Severity 四色深底面板、open/close 无动画 | ✅ | ✅ `close_hides` / `wide_bar_uses_horizontal_layout` / `narrow_bar_uses_vertical_layout` / `hit_close_and_action` / `handle_click_close_closes` / `closed_bar_renders_nothing` / `closable_false_hides_close_button` |
| §13 Expander | MetroExpander | `expander.rs` | Header 高 48、Padding 16、chevron 32×32（Margin 20,0,8,0）、Content Padding 16、展开 0.333s / 收起 0.167s、chevron 旋转 0.1s、Down/Up 方向 | ✅ | ✅ `toggle_flips_expanded` / `expand_animates_to_steady` / `expand_then_collapse_is_interruptible` / `visible_content_grows_with_progress` / `header_at_bottom_for_up` / `render_content_emits_only_when_visible` / `chevron_points_down_when_collapsed` |
| §14 InfoBadge | MetroInfoBadge | `info_badge.rs` | Value ≥0 数字（>99→"99+"）、FontSize 11、Padding 4,0,4,2、全胶囊（高/2）、W<H 方形、强调色底、Dot 4×4 | ✅ | ✅ `dot_is_minimum_and_square` / `value_clamps_to_99_plus` / `value_renders_accent_fill_and_text` / `dot_renders_no_text` / `kind_colors_map` |
| §15 PipsPager | MetroPipsPager | `pips_pager.rs` | Pip 命中区 12×20/20×12、正常胶囊高 4、选中高 6、选中强调色（Kanesumi 适配）、Nav 20×20 hover 显示、首/尾隐藏、滚动窗口 | ✅ | ✅ `clamp_selection_validates` / `visible_count_respects_max` / `click_selects_pip` / `nav_buttons_hidden_by_default` / `prev_next_edges_hidden` / `handle_prev_next_changes` / `render_emits_pills` |
| §16 PersonPicture | MetroPersonPicture | `person_picture.rs` | 方形圆（min 短边）、Initials 字号 42%、SemiBold、Badge 50% 右上外溢、Badge 字号 60%、>99→"99+"、首字母生成器（括号剥离/空格拆分/首末词） | ✅ | ✅ `initials_two_words` / `initials_strips_trailing_brackets` / `initials_cjk_returns_empty` / `circle_maintains_square` / `initials_font_is_42_percent` / `badge_is_half_and_top_right` / `badge_text_clamps` |
| §17 DropDownButton | MetroDropDownButton | `drop_down_button.rs` | Button + 右侧 chevron（E70D 自绘，12px，Margin 6,0,0,0）、点击 toggle MenuFlyout、flyout 开时 Pressed 亮度、点外关闭 | ✅ | ✅ `measure_reserves_chevron` / `toggle_opens_flyout` / `release_on_button_toggles` / `release_on_item_returns_index` / `click_outside_closes` / `hover_tracks_button_and_item` / `render_emits_button_and_flyout` / `disabled_lowers_alpha` |
| §18 BreadcrumbBar | MetroBreadcrumbBar | `breadcrumb_bar.rs` | Item 14px / chevron 12px（Padding 2,0）、当前项非按钮、超宽折叠为 "…" + 隐藏项下拉、点项返索引 | ✅ | ✅ `wide_fits_no_collapse` / `narrow_collapses_prefix` / `last_item_always_kept` / `hit_maps_items_and_ellipsis` / `hit_ellipsis_when_collapsed` / `toggle_ellipsis_opens_menu` / `handle_click_ellipsis_then_hidden_item` / `render_emits_items_and_chevrons` / `render_collapsed_has_ellipsis` |
| §19 SplitButton | MetroSplitButton | `split_button.rs` | Primary（*，MinWidth 35）│ 1px 分隔线 │ Secondary（35px chevron）、点 Primary 返主命令、点 Secondary toggle flyout、FlyoutOpen 两区 Pressed | ✅ | ✅ `geometry_splits_primary_secondary` / `hit_detects_parts` / `primary_click_returns_command` / `secondary_toggles_flyout` / `flyout_item_returns_index` / `click_outside_closes` / `render_emits_label_chevron_separator` |
| §20 PagerControl | MetroPagerControl | `pager_control.rs` | Nav 40×40（◀◀◀▶▶▶）、数字按钮 MinW 32/间距 5、选中强调色 + 2px 下条、窗口逻辑（前/中/后四页）、边缘隐藏 Nav | ✅ | ✅ `small_pages_show_all` / `start_window_shows_first_five` / `end_window_shows_last_five` / `center_window_shows_neighbors` / `selected_item_index_located` / `click_page_selects` / `nav_changes_pages` / `first_last_nav` / `edges_hide_nav` / `render_emits_selected_indicator` |
| §21 RadioButtons | MetroRadioButtons | `radio_buttons.rs` | Header 可选（Margin 0,0,0,8）、单选圆 20×20 描边 2px、选中 10px 强调圆点、ColSpacing 7/RowSpacing 8、MaxColumns 网格、单选不取消 | ✅ | ✅ `default_vertical_single_column` / `grid_layout_with_max_columns` / `select_updates_index` / `re_select_same_keeps` / `hit_outside_none` / `render_emits_circles_and_dot` / `header_renders_when_set` |
| §3 轨道/Knob | MetroSurface | `surface.rs` | 底色 + tint 叠加 | ✅ | ✅ |
| §7 列表项 | — | `list.rs` | 悬停/禁用态（待输入层） | 🔶 | — |
| §8.5 MenuBar 键盘遍历 | MetroMenuBar | `menu_bar.rs` | Alt-加速键 / Arrow Left/Right / Arrow Up/Down / Enter / Esc | 🔶 未接 | — |
| §8 MenuFlyoutSubItem | MetroDropdownMenu | `dropdown_menu.rs` | 二级级联（MenuItem.submenu 占位字段已在，render/命中未消费） | 🔶 未接 | — |

## 通用能力对照

| 能力 | 状态 | 说明 |
|---|---|---|
| 状态驱动渲染（state → Scene） | ✅ | 全部控件 `render(theme, engine, rect, scene)` |
| 进度驱动动画（`update(dt)`） | ✅ | Switch/ProgressBar/ProgressRing/Dialog/DropdownMenu/SelectorFlyout 均带 `update` |
| 动画预设 | ✅ | `toggle_flip` 0.15s、`progress_indeterminate` 2.0s、`overlay_open/close`、`dialog_scale`（参 CONTROL_SPEC §10） |
| 命中测试（hit_test / item_at / tab_at） | ✅ | Button/IconButton/List/List/ TabRow/Menu/Selector 提供 |
| 输入驱动状态切换（pointer→state） | 🔶 | 控件提供 `set_state`/`set_checked`/`hovered` 字段；由 harness 外壳（Linux）从指针事件调用 |
| 焦点环 | 🔶 | Button/IconButton `Focused` 态自绘描边（Kanesumi 适配，非 Metro 状态） |
| 弹层遮罩 | ✅ | `PopupAnim` + `render_overlay`；主题 `overlay_color`（黑 45%） |
| 弹层方向自适应 | 🔶 | 判据已记录（Top>0 向下），Phase 3 续 |
| 图标系统（SVG/自绘） | ✅ | `kanesumi-canvas` `icon::rasterize_svg` → `Scene::image`（直通 RGBA + tint 染色）→ harness Image 管线（RGBA8 纹理）；`MetroIconButton::with_svg` 接入 |
| 圆角光栅化 | 🔶 | Scene `corner_radius` 已携带，外壳实现圆角绘制时生效 |
| 弧线光栅化 | 🔶 | Scene `Arc` 命令已携带，外壳实现 |

## 验收说明

- **「功能对照表一一对应」判定**：每章规格要点在「实现」列有对应模块与行为，且至少一个单元测试验证关键数值/状态（时长、行程、尺寸、命令序列）。
- **可弃参考**：`CONTROL_SPEC.md` + 本表自足覆盖行为、尺寸、动画、主题；`reference/` 可整体删除，实现不再依赖它。
- **遗留**（不阻塞弃用）：输入层事件接线、方向自适应、图标管线、圆角/弧光栅化 —— 均 ✅ 已完成（输入层/方向自适应/图标管线 2026-08-10；圆角/弧光栅化 harness 外壳已实现）。
