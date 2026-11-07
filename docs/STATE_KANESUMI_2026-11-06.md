# Kanesumi 实用化 · 收尾状态（2026-11-06，10 轮推进）

> 交接入口仍是 `docs/NIGHT_SHIFT_2026-11-06.md`（含 Fedora 落地命令与剩余任务卡）。
> 本文只回答一件事：**这一轮到底把哪些「判据」变成了可验证的事实**。

## 一、分支

| 项 | 值 |
|---|---|
| 分支 | `oc/k-menu-a11y`（Kanesumi 子模块） |
| 基线 | `integration/n1104` @ `389c3c3` |
| 提交 | 53（`git log --oneline 389c3c3..HEAD`） |
| 推送 / 指针 | **未推送、未改主仓 gitlink、未部署、未碰真机会话** |

## 二、对判据（`ROADMAP.md` §Ⅰ）的逐条交代

### ① 内容槽：控件可容纳任意子节点
- **机制**：`PaintCtx::children()`（绘制期判断有无内容）、`Widget` 默认 `arrange` 铺满、
  `EventCtx::insert_child` / `insert_child_with`（事件回调里挂子树）。
- **控件**：`MetroButton`、`MetroCheckBox`、`MetroDialog`（`show_modal_with` + 高度 184..756 自适应）。
- **样板**：`kanesumi-gallery/examples/grid_form.rs`（整页零 `Rect::new` 手算、零命中函数）、
  `dialog_content_snapshot.rs`。
- **判据达成**：图标+文字按钮 / 富标签勾选框 / 重命名对话框，均**不需要自造控件**。

### ② 右键菜单手感 + 菜单条目级无障碍
- 手感：禁用项（不亮/不展开/键盘跳过/点击不动作且不关）、勾选列（16pt，勾与不勾文字起点一致）、
  ↑↓ 循环跳过禁用、Home/End、→ 预选首个可用、Esc 关闭并交还焦点、贴边翻折（既有测试覆盖）、
  长菜单**封顶 504 + 滚动 + 可拖指示条**、`submenu_gap` 可配。
  **悬停延迟**：三家参考（WinUI / egui / iced）都是悬停即开，本仓同样即开（登记为对照结论，不作缺口）。
- 无障碍：菜单每条命令报 `MenuItem`（名字 / 勾选 / 禁用 / 快捷键，级联挂父项下）。
- 参考库对照：`docs/research/2026-11-06_menu_feel.md`（egui `3c2531c` / iced `8ff48c4` 浅克隆只读）。

### ③ 参考库对照
已按上条落地；结论写进研究文档（采纳 / 不采纳 / 仍缺三段），未复制其代码风格。

## 三、门（收尾实测）

| 命令 | 结果 |
|---|---|
| `cargo test -p kanesumi-element --lib` | 85 passed / 0 failed |
| `cargo test -p kanesumi-element --tests` | tree 53、scroller 8、layer 11、popup_motion 5、pointer_buttons 1、event_insert 2 全绿 |
| `cargo test -p kanesumi-controls --lib` | **862 passed / 2 failed**（下列两条 Windows 沙箱环境，非代码） |
| `cargo test -p kanesumi-canvas --lib glyph::tests` | 5 passed |
| clippy（element + controls + gallery，`--all-targets`） | 无新增告警（仅既有测试助手告警） |

两条环境失败（Fedora 上应绿，已请夜班核实）：
1. `icon_button::tests::svg_bitmap_renders_image_command` —— 临时目录 `PermissionDenied`；
2. `popup::tests::export_progress_series_csv` —— 测试写源码树 `../target` 被拒。

## 四、像素回归（收尾实测，逐字节比对）

| 图 | 对照 | 结果 |
|---|---|---|
| `grid_form`（内容槽 + 图标文字按钮） | 首次引入时 | **sha256 相同** |
| `dialog_content_snapshot`（对话框内容槽 + 焦点环） | 首次引入时 | **sha256 相同** |
| `menu_states_snapshot`（勾选列 / 禁用 / 长菜单） | 首次引入时 | **sha256 相同** |

即：十轮改动里**视觉零漂移**，新增的都是能力与交互，不是观感变化。

## 五、本轮顺手修掉的真缺陷（都由示例/契约逼出来）

1. 子菜单渲染是第二份排版 → 合一（否则勾选列 / 禁用透明度到不了子菜单）；
2. 列表 / 树 / 菜单 / 页签合成项越出宿主 → 一律与宿主矩形求交；
3. **滑块命中越界**（压扁时命中区伸出矩形，「点上面那行」会命中滑块）；
4. 导航视图 / 搜索框 Header 为空时可聚焦但无名 → 类型名级兜底。

## 六、仍未做（不进本轮，需你定或需硬件）

1. **对话框按钮做成真框架节点**（Tab 可达；现在 ←→ 可选、可读，但 Tab 到不了）—— 任务卡 W1；
2. **AT-SPI 桥**（需新依赖 `accesskit` / `atspi`，**未获授权**）；
3. **真机手感复测**（1×/1.5×/2× 菜单开合、列表/树滚动、对话框焦点环）—— 全部 NotRun；
4. **`integration/n1104`（实用化）与 `oc/scale-f1b`（分数缩放）两条线未合并**（动主仓 gitlink，需你拍板）；
5. W3 剩余：CommandBar / Breadcrumb 溢出面板的建议项（自动补全已接）。
