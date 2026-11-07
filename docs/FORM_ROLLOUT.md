# 页面语法（form）落地进度与接手须知

本文件是「把 Kanesumi 从控件库推进到应用运行时」这条线的**唯一接手入口**。规格在
`docs/CONTROL_SPEC.md` §49 / §50 / §51，写法在 `docs/DEV_GUIDE.md` §二·四 / §二·五。

## 已完成（可直接用）

| 能力 | 位置 | 验证 |
|---|---|---|
| 页面语法 14 个语义块 | `kanesumi-controls/src/form.rs` | `tests/form_grammar.rs` 5 条；三道静态门（size / token / api）全绿 |
| `Form::block` 底级原语 | 同上 | settings 的 `Col` 已退化成它的别名 |
| `Form::list_item` 列表行（图标槽 + 主文本 + 右侧次要文本） | 同上 | 行高与列位单测；参照页 `form_page` 出图 |
| `form::panel` 面板容器（表面 + 内边距 + 一列） | 同上 | 参照页 `panel_form` 出图 |
| `MetroSurface::panel(theme)` | `kanesumi-controls/src/surface.rs` | API 表面快照 |
| CLI 工具链 `kanesumi` | `kanesumi-cli/` | 12 条单测；deb 经真机 `dpkg-deb -I/-c` 验过 |

参照页三张（都已看图确认）：`form_page`（12 块全览）、`form_theme_page`（settings 主题页原样迁移）、
`panel_form`（顶栏控制中心形态）。

## 已完成（应用侧）

- **settings**：36 个页面的结构调用全部走 `Form`；私有 `window/grammar.rs` 从 755 行降到 315 行
  （−58%），自绘控件从 36 个降到 20 个，且剩下的 20 个都是应用专属（`HomeCell` / `WifiRow` /
  `Thumbnail`…）。改动在 Ether 主仓分支 `oc/settings-form`（6 个提交）与 `oc/librarian-fixes`。
- **librarian**：两处修复（XDG 桌面目录、选择器双击=确认）编译通过，未部署。

## 未完成（按此顺序）

### 1. 顶栏面板行结构（控制中心先做）

994 行的 `settings/src/topbar/control_panel.rs` 要按
`settings/docs/PANEL_FORM_MIGRATION.md` 改：`form::panel` 建容器，滑杆/开关换成 `Form::slider` /
`Form::toggle`，删掉 `PanelRegion` 枚举与 `pressed` / `mute_hover` / `panel_w()` / `panel_h()`。
音频（1575）与电源（1240）同构，可照抄。网络面板（2698）需要连接列表——`Form::list_item` 可覆盖；
通知中心（1213）需要通知卡片，尚无规格。

三个已记录的坑：命令按钮别走 `Form::button`（它带 120 最小宽，三枚并排撑爆 360 面板）、
首块 `first = true`、面板高度变化会移动合成器的浮层锚点。

### 2. `MetroTableRow`（librarian 用）

规格已在 `docs/CONTROL_SPEC.md` §51：列定义（`Px` / `Star` / `Fit`）、行状态（含 `SelectionTint`
选中高亮）、展开三角、4pt 拖拽阈值、逐列命中、键盘导航、排序头。

**在它落地前，librarian 的列表不要改用 `Form::list_item`** —— 那个是静态设置行，硬套会丢选择态、
拖拽与逐列命中三样交互。

### 3. 已知小项

- `bg_work.rs` 里 `Job::Wallpaper` 的 match 分支写了两遍（第二份不可达），应删；
- `kanesumi_canvas::rasterize_svg` 对「无 `fill` 属性的 SVG」能正常栅格化，已有回归用例；
  gallery 某示例图标不显示的问题在调用侧（路径），与管线无关。

## 验收命令

```bash
cargo test -p kanesumi-controls --test form_grammar \
           --test size_discipline --test token_discipline --test idioms_enforce
cargo test -p kanesumi-cli
cargo test -p kanesumi-element --test api_surface   # 改公开 API 后先重出快照
cargo run -p kanesumi-gallery --example form_page -- --snapshot out.png 1
cargo run -p kanesumi-gallery --example panel_form -- --snapshot out.png 1
```
## 重做计划（2026-11-07 用户裁定：清障完成，重做系统应用，全部完成后再部署）

清障阶段已结束，`form` 侧不再缺构件。重做顺序按「一次能完整验证」排列：

| # | 目标 | 量 | 前置（都已就绪） |
|---|---|---|---|
| 1 | 顶栏任务面板 `TaskStackPanel` | ~150 行 | `form::panel_in`（弹层由应用层开）+ `Form::row_button`（带行号） |
| 2 | 顶栏控制中心 `ControlPanel` | ~450 行 | 同上 + `Form::slider` / `Form::labeled` / `Form::actions` |
| 3 | 音频 `AudioPanel` / 电源 `PowerPanel` | 各 ~500–600 行 | 同 2，照抄 |
| 4 | 网络 `NetPanel`（连接列表） | ~800 行 | 同 1（行按钮） |
| 5 | 通知中心 `ActionCenter` | ~700 行 | 需通知卡片构件（尚无规格，先补 §52） |
| 6 | librarian 窗口列表 / 桌面格 | 大 | 需 `MetroTableRow`（规格 §51） |

第 1 项的四步（下一轮直接照做）：`TaskStackButton::event` 从「自己 `open_popup`」改为
`ctx.emit(TaskStackOpenRequested(tasks.clone()))`；TopBar `on_action` 里接住它，用
`tree.open_popup(MetroSurface::panel(theme), spec)` + `form::panel_in` + 逐行 `Form::row_button(index, 标题, 应用名)`；
接住 `RowButtonClicked(index)` 做激活；删掉 `TaskStackPanel` 与其单测。

## 部署（用户裁定：全部完成后一次性部署）

现阶段一律不装。等上面 1~5 完成、`cargo check -p ether-settings` 与出图对照通过后，
再把 `ether` / `ether-settings` / `ether-librarian` 一起部署到 e495。
已编好但**未部署**的旧候选：`e495:~/verify/backup/ether-librarian.new-20261107`（该版只含 librarian 两修复，
重做完成后会重新构建覆盖它）。

## 进度更新（2026-11-07，顶栏第一个面板已迁完）

| 项 | 状态 | 证据 |
|---|---|---|
| 顶栏任务面板（`TaskStackPanel` ~150 行） | ✅ 已迁，整块自绘删除 | `settings` 分支 `d190a9c6`；`cargo check/test` 通过 |
| 顶栏控制中心（~450 行） | ⬜ 未开始 | 清单见 `settings/docs/PANEL_FORM_MIGRATION.md` |
| 音频 / 电源 / 网络 / 通知中心 | ⬜ 未开始 | 同上 |
| librarian | ⬜ 未开始 | 需 `MetroTableRow`（§51） |

**迁移范式（已被真实代码验证一遍，照抄即可）**：

1. 控件只发请求动作（`Widget::event` 里拿不到 `Tree`，不能自己开弹层）；
2. 应用层 `on_action` 里 `tree.open_popup(MetroSurface::panel(theme), spec)`；
3. `form::panel_in(tree, panel, 宽度)` 铺内容列；
4. `Form::section / slider / toggle / labeled / row_button` 建行；行按钮带回行号；
5. 删除原面板的 `impl Widget`、手算矩形函数与命中枚举。

**验证基线与纪律**（血的教训，务必遵守）：

- Linux 侧 `cargo test -p ether-settings` 基线：lib **132 passed**；main **464 passed / 4 failed**
  （4 项是既有的对话框确认类失败，已用「回退 topbar 后重跑」A/B 证明与迁移无关）；
- **Windows 侧 `cargo check -p ether-settings` 的 exit=0 不可作为验收证据**：批量替换后它曾长时间
  复用陈旧产物，把一个编译不过的分支报成绿。一律以 T14p 全量编译为准。
