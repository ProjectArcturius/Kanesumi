# Kanesumi 实用化交接 · 夜班（Fedora）· 2026-11-06

> 本文件是**交接真源**：留给远端 Fedora 夜班接力用。分支、门、任务卡、禁区都在这里。
> 写这份的人（Windows 会话）已完成 8 轮推进，**未推送、未改主仓指针、未部署、未碰真机会话**。

## 一、交接状态

| 项 | 值 |
|---|---|
| 仓 | Kanesumi（`shared/kanesumi` 子模块） |
| 分支 | `oc/k-menu-a11y` |
| 基线 | `integration/n1104`（`389c3c3` fix(controls): 修复合入尺寸表时的导入残留） |
| 顶部提交 | `b724199` docs(a11y): 页签条成为第六处合成项消费者 |
| 提交数 | **48**（`git log --oneline 389c3c3..HEAD`） |
| 改动面 | 32 文件，+4453 / −269 |
| 推送 | **未推送**。是否推送由用户定；夜班不推 |

工作树（Windows）：`Ether/target/wt/k-menu-a11y`（gitignored）。Fedora 侧请新建独立工作树，勿复用他人现场。

## 二、已完成（按主题，含验收证据）

### A. 内容槽（`ContentControl` 对等物）
- 框架：`PaintCtx::children()`；`EventCtx::insert_child` / `insert_child_with`（弹层内容一次写完）；
- 控件：`MetroButton`、`MetroCheckBox`、`MetroDialog`（＋`show_modal_with`，盒体高度 184..756 自适应）；
- 样板：`kanesumi-gallery/examples/grid_form.rs`（整页零手写坐标）、`dialog_content_snapshot.rs`；
- 文档：`docs/DEV_GUIDE.md` §2.6、`docs/IDIOMS.md` §Ⅱ、`docs/CONTROL_SPEC.md` §1/§36/§9。

### B. 菜单手感
- 禁用项（`MenuItem::disabled()`：不亮、不展开子菜单、键盘跳过、点击无动作且菜单不关、整体降 0.38 透明度）；
- 勾选列（`MENU_CHECK_GUTTER` 16pt + 自绘 `glyph::check`；勾与不勾文字起点一致）；
- 键盘：↑↓ 循环跳过禁用、Home/End、→ 预选首个可用；
- **长菜单封顶 + 滚动**：`MENU_MAX_HEIGHT = 504`（推断值，登记 `CANON_VS_TEMPORARY` T26）、滚轮/键盘滚动、右侧 3pt 胶囊指示；
- `render_submenu` 改为委托 `render_panel`（消掉第二份排版）；
- 文档：`docs/CONTROL_SPEC.md` §8/§39、`docs/research/2026-11-06_menu_feel.md`（含 egui / iced 对照）。

### C. 无障碍
- 框架：`AccessItem`（role/name/value/checked/selected/expanded/bounds/disabled/focused）、
  `Widget::access_items(rect, theme)`、快照并入覆盖层弹层、第 5 条契约（合成项须在宿主 `bounds ⊕ paint_overflow` 内）、命中一致性按外扩判；
- 角色词表：`Menu` / `MenuItem` / `TreeItem` / `TabItem`；
- **六处消费者**：菜单命令、`MetroList`（可见行）、`MetroDialog`（按钮 + 当前项）、`MetroTreeView`、`MetroNavigationView`、`MetroTabRow`；
- 对话框按钮键盘 ←→ + Enter（当前项有焦点环、报 `focused`）；
- 示例：`examples/access_snapshot.rs` 打印含合成项的快照并跑契约（非零退出即可见）。

### D. 顺手修掉的缺陷
- 列表/树/菜单/页签的合成项边界：一律与宿主矩形求交（否则契约 5 报越界、读屏读到屏幕外内容）；
- **滑块命中越界**：被压得比自然高度矮时命中区伸出矩形（`access_snapshot` 示例抓出）→ 改为与自身矩形求交；
- `MetroNavigationView` Header 为空时可聚焦但无名 → 退回「导航」；
- 菜单子菜单渲染与顶层不一致 → 合一。

## 三、本机门（Windows，可作为基线数字）

| 命令 | 结果 |
|---|---|
| `cargo test -p kanesumi-element --lib` | 85 passed / 0 failed |
| `cargo test -p kanesumi-element --tests` | tree 53、scroller 8、layer 11、popup_motion 5、pointer_buttons 1、event_insert 2 全绿 |
| `cargo test -p kanesumi-controls --lib` | **858 passed / 2 failed** |
| `cargo test -p kanesumi-canvas --lib glyph::tests` | 5 passed |
| `cargo clippy -p kanesumi-element -p kanesumi-controls --all-targets` | 无新增告警（仅既有测试助手告警） |

**两条失败是 Windows 沙箱环境，不是代码**（Fedora 上应绿，请核实并回报）：

1. `icon_button::tests::svg_bitmap_renders_image_command` —— 临时目录 `PermissionDenied`；
2. `popup::tests::export_progress_series_csv` —— 测试把 CSV 写进源码树 `../target`，子进程被拒。

## 四、Fedora 侧落地步骤

```bash
# 1) 取源码：基线 + 增量 bundle（由用户/调度者把 bundle 放到 Fedora）
git -C ~/ether-night/src/Kanesumi cat-file -t 389c3c3          # 基线必须存在
git -C ~/ether-night/src/Kanesumi fetch /path/to/k-menu-a11y.bundle oc/k-menu-a11y:oc/k-menu-a11y

# 2) 独立工作树（勿复用既有现场）
git -C ~/ether-night/src/Kanesumi worktree add ~/ether-night/wt/k-menu-a11y oc/k-menu-a11y

# 3) 构建与门（容器镜像 localhost/arcturius-build:trixie，jobs=2）
~/ether-night/bin/ebuild ~/ether-night/wt/k-menu-a11y cargo test --workspace
~/ether-night/bin/ebuild ~/ether-night/wt/k-menu-a11y cargo clippy --workspace --all-targets
```

Windows 侧生成增量 bundle 的命令（只读，不改仓）：

```bash
git -C /path/to/Ether/target/wt/k-menu-a11y bundle create k-menu-a11y.bundle 389c3c3..oc/k-menu-a11y
```

## 五、夜班任务卡（按顺序，一次一张）

### W1 · 对话框按钮做成真框架节点（Tab 可达）
- **目的**：现在按钮是控件自绘 + ←→ 选择，Tab 到不了它们。Oct 卡面遗留的最高价值一项。
- **做法（二选一，先写一页设计再动手）**：
  A. 组合式：`show_modal_with` 打开 `Border(padding 24,18,24,24)` → `Grid(rows=[*, 32+8])`，行 0 = 标题 + 内容槽，行 1 = 真 `MetroButton` 子节点；`MetroDialog` 退化为「盒体 chrome + 内容区」；
  B. 角色标注：给 `Widget` 加 `fn layout_role() -> LayoutRole { Normal | Content | Commands }`，对话按角色排子节点。
- **文件面**：`kanesumi-controls/src/dialog.rs`、必要的 `kanesumi-element`（仅 A/B 选 B 时）。
- **门**：`cargo test -p kanesumi-controls --lib dialog`；Tab 顺序测试（三个按钮依次获得焦点）；Enter/Space 激活焦点按钮；Esc 关闭；焦点陷阱内循环。
- **禁区**：不改 `DialogResult` 语义、不改按钮视觉规格、不删既有 ←→ 选择（两套输入可共存，但要写明优先级）。

### ~~W2 · 菜单手感收尾~~ **已完成（2026-11-06 收尾批，提交 4cedd29）**
`submenu_gap` 可配、指示条可拖（命中区左扩 8pt、拖动期不改悬停项、松手不激活）；回弹登记为有意不做。**夜班跳过本卡。**

### W2（原卡，留档）
- 拖拽滚动指示 thumb、滚到尽头回弹（或明确不做并登记）、子菜单间距可配（`submenu_gap`）；
- **文件面**：`kanesumi-controls/src/dropdown_menu.rs`、`popup.rs`；
- **门**：现有菜单测试全绿 + 新增 ≥2 条（thumb 拖动改变偏移、间距配置生效）；`examples/menu_states_snapshot.rs` 出图（2×）并亲看。

### W3 · 无障碍第 5 条契约铺满剩余容器（**部分完成**：自动补全建议列表已接，提交 `b576d81`）
- `MetroCommandBar` / `MetroBreadcrumbBar` 溢出面板 / `MetroAutoSuggestBox` 建议列表 的 `access_items`（按 list/tree 的先例：可见项 + 交集裁切）；
- **门**：每处 1~2 条测试 + `assert_access_contract`；示例 `access_snapshot.rs` 至少覆盖一处。

### W4 · 全量门 + 像素回归（收尾卡）
- `cargo test --workspace`、`cargo clippy --workspace --all-targets`；核实第三节两条 Windows 失败在 Linux 上为绿；
- 三张图 1× / 1.5× / 2× 出图并亲看：`grid_form`（内容槽）、`menu_states_snapshot`（长菜单）、`dialog_content_snapshot`（对话框焦点环）；
- 产出：一份 `docs/research/2026-11-0X_night_review.md`，含每项命令与汇总行、图路径、未做项。

## 六、禁区（夜班一律遵守）

1. **不推送、不改主仓子模块指针、不部署、不安装、不碰真机会话**；
2. 不新增依赖（AT-SPI 桥等需要用户点头，**不在本夜班队列**）；
3. 不删测试、不改断言、不加 `#[ignore]`、不放宽阈值；测试红就修代码；
4. 不清理他人现场与用户数据；`/tmp` 不落任何诊断（持久路径才行）；
5. 合并只进集成分支，不进 `main`；每项一次提交（一个功能块）。

## 七、仍未做（留给用户决定）

1. **AT-SPI 桥**：需新依赖（`accesskit` / `atspi`），未获授权；
2. **真机手感复测**：1×/1.5×/2× 菜单开合、列表/树滚动、对话框焦点环 —— 全部 NotRun；
3. **`integration/n1104` 与 `oc/scale-f1b` 两条线未合并**（主仓 gitlink 决定，需用户拍板）；
4. 菜单条目 / 对话框按钮仍不是框架节点（读屏可枚举，但 Tab 与命中路由还到不了）。
