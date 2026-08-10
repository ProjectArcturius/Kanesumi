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
| §3 轨道/Knob | MetroSurface | `surface.rs` | 底色 + tint 叠加 | ✅ | ✅ |
| §7 列表项 | — | `list.rs` | 悬停/禁用态（待输入层） | 🔶 | — |

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
