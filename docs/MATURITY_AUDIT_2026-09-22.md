# Kanesumi 成熟度审计与 P0 整改 —— 2026-09-22

> 触发：在 `arc-deck`（UWP/WinUI 2 与 Rust 双实现）里感到「C# 那边更舒服」，怀疑 Kanesumi 不成熟。
> 本文记录**只读全仓审计**的结论与**第一批 P0 整改**，以及后续路线与需维护者裁定的事项。
> 审计基线 `8604304`（98 个 `.rs` / 40,196 行 / 672 个 `#[test]`）。

## §Ⅰ 审计结论（先纠正前提）

**引擎不弱，弱在「接不上」。** 这是本次审计最重要的一句话。

| 维度 | 实情 |
|---|---|
| 排版 | `kanesumi-canvas/src/text.rs`（887 行）是真实现：rustybuzz OpenType 塑形 + unicode-bidi + UAX #14 换行 + CJK 行首禁则 + 字体 fallback + `Clip`/`Ellipsis` + 双缓存。**全仓最成熟的部分。** |
| 布局 | `kanesumi-canvas/src/layout.rs`（771 行）是真两遍 Measure/Arrange：`Constraints` / `Row`/`Column`/`Leaf`/`Spacer`/`Flexible` / `CrossAlign` / 容器 clip / `hit_at` 与渲染共用同一棵树。 |
| 渲染/时效 | dirty 驱动 + frame callback 作 vsync 提示（16ms 忙 / 100ms 闲兜底）、CPU 局部重绘、dt 限幅。`dmabuf` 惰性化 + 子进程探测（对症 2026-08-19 事故）。 |
| 鲁棒性 | 生产路径 unwrap/panic 约 16 处 / 40k 行；整仓 **0 unsafe、0 `todo!`**。远超文档给人的印象。 |

**病灶是采用率与强制力**：布局引擎只有 `decl.rs`（DSL）与 gallery 两页在用（47 个控件里 13 个有 `measure()`、11 个有 `hit_test()`，261 处字面量 `Rect::new`）；`TextOverflow::Ellipsis` 全仓仅 1 处；`FontWeight` 是死字段（5 种字重视觉全同）；`RetainedScene` 每帧两次全量 `render_decl`；`MetroGrid` 零消费者；三套布局（`canvas/layout`、`structure/ui`、`structure/grid`）并存。

用户列的五项痛点，根因映射：

| 痛点 | 根因 |
|---|---|
| 开发效率 | 无控件树 / 无模板 / 无绑定；一个屏幕的坐标在「布局常量、控件树、事件路由」三处重复；`App` trait 要求 App 自己维护 `needs_redraw` / `hover_signature` / `damage_hint` |
| 动画流畅度 | 动画词汇表缺口（文档自承）；17 个交互控件硬状态切换（按钮连 UWP 100ms 下沉都没有）；xdg-shell 角色不吃损伤重绘 |
| 稳健性 | `catch_unwind` 覆盖不全；`shm_open` 两处 unwrap；IME 字节切片未校验 char boundary；surface/device-loss 无恢复 |
| 元件协调 | 无焦点管理器 / 无 Tab 遍历 / 无 z-index / 无 keyed reconciliation / 弹层非真 `xdg_popup`；每个控件自持 `hit_test`，`render` 与命中是两份算术 |
| 文字/元件防溢出 | 省略号未启用（硬裁切无 "…"）；多个控件把「量测宽度」当绘制宽度（文字画到控件外）；容器无 clip |

## §Ⅱ 本批 P0 整改（已落地，686 测试全绿）

### P0-1 溢出契约：默认省略号 + 显式意图 + 不得静默消失

- `Scene::text()` 默认 `overflow` 由 `Clip` 改为 **`Ellipsis`**（`kanesumi-canvas/src/scene.rs`）。
  理由：该 API 的调用方绝大多数是**单行标签**；旧默认让超长标签被硬裁掉半个字且无提示。
- 新增 `Scene::label()`（单行不换行 + 省略号 = UWP `TextWrapping=NoWrap` + `CharacterEllipsis`）
  与 `Scene::paragraph()`（换行 + 裁切）。意图进类型，不再靠调用方记三个参数。
- `TextEngine::layout_box`：**框矮于一行时不得把唯一一行也截掉**（旧实现 `floor(h/line_height)=0`
  → `truncate(0)` → 文字整段消失且无日志）；零高框仍不画。
- 控件侧修正「量测宽当绘制宽」：`button`（最典型：长标签画到按钮外）、`tab_row`、`tab_view`、
  `drop_down_button`、`split_button`、`menu_bar`、`radio_buttons`、`dropdown_menu`（标签 + 快捷键避让）、
  `list`、`tree_view`、`selector_flyout`。

### P0-2 容器裁剪成为容器语义

`list`（虚拟化但半行越界仍画）、`tab_view`、`tree_view`、`dropdown_menu`（含子面板开启动画期）、
`selector_flyout`、`tab_row` 补成对 `push_clip/pop_clip`；`tab_view::hit/hover` 增加
「控件矩形外一律不命中」（旧实现能从控件外点中滚出视口的页签）。

### P0-3 消除可达 panic

| 位置 | 旧行为 | 现行为 |
|---|---|---|
| `tab_row::render` | `MetroTabRow::default()`（空表）→ `geoms[self.selected]` 越界 | 空表/空矩形直接不画；`selected`/`prev_selected` 夹紧 |
| `dropdown_menu::select_submenu` | `sub.menu.items[path.index]` 索引的是**另一份列表** | `get_mut` 守卫 |
| `menu_bar::render` | `items` pub / `flyouts` 私有 → 长度失配越界 | `flyouts.get(i)` |
| `repeater` | `columns` 可置 0 → `div_ceil(0)` / `% 0` | 一律经 `columns_clamped()` |
| `MetroGrid::new` | 空行/空列 `assert!` | 允许空定义；`child_rect` 索引夹紧 + span 饱和减 |

### P0-4 稳健性

- `shm_open`：`/dev/shm` 满或权限异常时**丢帧 + 落盘诊断**，不再 `unwrap` 杀进程；重建 pool 改为
  「先建新、成功后再拆旧」。
- IME 周边文本切片：`floor_char_boundary` 夹到 UTF-8 字符边界（客户端给的字节偏移可能落在多字节
  字符中间 → 旧实现直接 panic）。
- 新增 `guard(what, f)` 错误边界，并接入此前裸调的 App 回调：`ime_engine_key` / `ime_engine_preedit` /
  `ime_engine_take_commit` / `ime_engine_take_delete` / `ime_engine_popup_size` / `context_menu` /
  `focus_changed` 同路径 / `ime_focus` / `should_close`。
- 诊断改**持久路径**：`write_diag` 主写 `~/.local/state/ether/`，会话内副本写 `$XDG_RUNTIME_DIR`；
  帧 trace / 尺寸 diag 不再裸写 `/tmp`（参 `AGENTS.md` 铁律）。
- 文档漂移：`CLAUDE.md` 测试数 404 → 686；`README.md` 控件数 12 → 47。

## §Ⅲ 后续路线（未做，按优先级）

**P0（余）**
1. **布局引擎落地**：47 控件里 13 个有 `measure()`、11 个有 `hit_test()` —— 先迁 gallery 三页，让真实
   rect 由引擎产出；同时明确 `canvas/layout` 与 `structure/ui`、`structure/grid` 三套布局的去留。
2. **焦点管理器 + Tab 遍历**（新增 `controls/focus.rs`，harness 侧统一处理 Tab/Shift+Tab）。
3. **一个通用 ScrollViewer 容器**（clip + 虚拟化 + 命中三合一）：修 `SelectorFlyout` 504px 截断不可达、
   `DropdownMenu` 可画到屏外、`TreeView` 无滚动。

**P1**
4. keyed reconciliation + `RetainedScene` 真增量（当前每帧两次全量）。
5. 动画：`PointerDown/UpThemeAnimation`（按钮 100ms 下沉）→ stagger 入场 → AddDelete/Reorder。
6. 真 `xdg_popup` 弹层（当前弹层画在主 surface，Dock/TopBar 矮表面无法把菜单弹到自身之外）。
7. `FontWeight` 接真字面（多字重字体栈 / 可变字体）；`ShapeKey`/`LayoutKey` 纳入字体 identity 与 text-scale。
8. `needs_redraw()` 契约陷阱（契约要求 `render()` 末尾清脏，但查询发生在 render 之后 → 严格遵循者
   动画退化到 ~10Hz）。

**P2**
9. 无障碍导出（AT-SPI2）——对照 UWP 免费提供的 UIA，这是自绘路线最大的结构缺口。
10. 系统剪贴板桥接、拖放。

## §Ⅴ 续：M1 收官与「Windows 静态验证」通道（2026-09-22）

按 `ROADMAP.md` M1 收官（令牌纪律），并把「没有 Linux 会话也能验证」这件事做实：

1. **令牌层补齐**：`MetroIndication` 增前景强度档，`MetroColors` 增 `text_selection_tint` /
   `track_subtle`，`StatusColors` 增 `error_fill` / `danger_fill`，`Color` 增 `most_readable_on`（自动前景）。
   ⚠ **当日稍晚的「一手源取证轮」推翻了本批的三处取值与两个令牌**（浅叠族 15%/25% 的百分比、
   `list_hover_tint` 与 `accent_low_tint` 的存在本身、Base* 档位的 90%/35%），详见
   `docs/UWP_PRIMARY_SOURCES.md` §Ⅱ 与 `CANON_VS_TEMPORARY.md` §三。
2. **迁移**：控件生产段的 32 处魔法 alpha 与 6 处内联颜色字面量全部换成令牌；
   无权威来源的取值不猜也不丢 —— 令牌化后在 `CANON_VS_TEMPORARY.md` 登记（T12~T16）。
3. **静态守住**：`kanesumi-controls/tests/token_discipline.rs` 扫描生产段，禁止颜色构造器与
   裸 `with_alpha(<数字>)`；含反向自检与「扫描文件数 ≥40」断言，防「检查形同虚设」。
4. **自检扩展**：新增令牌纳入 WCAG 断言 —— tint 先 source-over 合成再判（直接拿未合成 RGB
   会把「浅底叠白」判成合格），实心语义件 ≥3.0，指示条 vs 轨道 ≥3.0（亮色 2.7 为登记缺口 T15）。
5. **测试通道修复**：全仓字体查找补 Windows 路径。此前 gallery 33 个交互测试直接 panic、
   calculator 2 个假失败、menu_bar / `canvas::layout` / `harness::context_menu` 静默跳过 ——
   静默跳过的测试比没有测试更危险（改坏了也不知道）。修复后本机 **710 个单元测试 + 2 个静态检查
   + 2 个 doctest 全绿**（合计 714），`cargo clippy --workspace --all-targets` 告警数与基线**逐条一致**（无新增）。


## §Ⅳ 需维护者裁定

1. `MetroGrid` / `structure::Ui` 是留是废？（决定 P0-1 的迁移终点）
2. `RetainedScene` 是「尚未完成」还是「已放弃」？若放弃，是否标注实验性并移出公开导出？
3. 「默认省略号」是否符合正典？（本批已按「安全默认」落地：装得下零影响，装不下给 "…"）
4. `arrange` 是否应强制 `Constraints::max`？是否需要 Min/Max 尺寸样式属性？
5. 弹层是否必须走真 `xdg_popup`？（会改动被 `CONTEXT_MENU_SPEC.md` 冻结的 `place_popup` 签名）
6. `FontWeight` 的意图：待接线，还是刻意留待可变字体？
