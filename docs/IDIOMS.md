# Kanesumi IDIOMS —— 「类 .NET 手感」的写法契约

2026-11-05 · 监工卡 n1104-idioms（交付 2）。把散在各卡与规范里的约定收拢成可执行条款；
每条带出处（`文件:行号`），无出处标「推断」。配套强制层见 §Ⅸ（`clippy.toml` 草案 + 静态检查测试）。
能力对照的「为什么」见 `WINUI_REFERENCE_MAP.md`。

## §Ⅰ 命名

1. **Rust 通用**：类型 / 枚举 / trait / 常量 PascalCase，函数 / 变量 / 模块 snake_case——
   即 rustfmt / clippy 默认，本仓无例外（全仓类型如 `MetroButton`、方法如 `measure`）。
2. **类型 / 枚举 / 动作 / 视觉状态名：PascalCase 写成强制**（用户偏好，2026-11-04 夜裁定）。
   动作类型遵循 `ELEMENT_MIGRATION.md` §4 表（`ButtonClicked`、`CheckToggled(pub CheckState)`
   —— `controls/src/button.rs:131`、`controls/src/check_box.rs:201`、`controls/src/dialog.rs:365`）。
3. **公共方法保持 snake_case**：用户 2026-11-04 夜确认「保持现状」。PascalCase 方法名会触发
   `non_snake_case` 告警，且与 trait 方法（`measure/paint/event`，`element/src/widget.rs:59/71/78`）
   冲突。**不改现有 API**；「公共面是否改 PascalCase」若日后再议，须由调度者另开裁定，不在日常提交里顺手改。
4. 控件类型 `Metro` 前缀（UWP 对应物，`controls/src/` 全体）；元素树内置控件无前缀
   （`element/src/widgets/{Border,Stack,Label,Image}`）；动作结构体字段 `pub` 元组式携带载荷。
5. 模块文件名 = 类型 snake_case（`check_box.rs` ↔ `MetroCheckBox`）；测试模块固定名
   `tree_tests`（几何/树行为）与 `tests`（单元），参 `ELEMENT_MIGRATION.md` §5。

## §Ⅱ Widget 模板

新控件按 `ELEMENT_MIGRATION.md` §1-§3 的模板实现 `Widget`（`element/src/widget.rs:56`）：

- `measure` 返回期望尺寸（**不含 margin**，margin 由框架加，`widget.rs:57`）；`arrange` 默认铺满
  子节点（`widget.rs:64`）——带内容的控件不写 arrange 也不会让子节点落零矩形；
- `paint` 只画自己；叠层内容用 `paint_after`（滚动条、覆盖描边，`widget.rs:75`）；
- 命中默认矩形内（`widget.rs:129`）；无背景容器必须 `hit_test → false`（空白穿透，
  `widgets/stack.rs:131-134`、`controls/src/parallax_view.rs:131`）；
- 弹层类走 `open_popup` + `PopupSpec`（`widget.rs:406`；模板见 `ELEMENT_MIGRATION.md` §8）；
- 有展开/选中语义的控件在快照数据源落地前，勾选/展开态放 `AccessInfo.checked`、选中行放
  `value`（expander 先例 `controls/src/expander.rs:436`）——终态见 access.rs 结构体注释。
- **内容槽**：控件可以容纳任意子节点（XAML `Content`）。做法固定三条：① `measure` 里
  `ctx.children()` 非空就按子节点量测（`MeasureCtx::children`）；② `arrange` 把子节点排进
  **内边距内矩形**；③ `paint` 里用 `PaintCtx::children()` 判断，有子节点就只画 chrome、不画自己的
  标签，但标签仍作 `AccessInfo.name`。样板 `controls/src/button.rs`「内容槽」一节。
  不允许「新增一个专门吃内容的控件类型」来代替内容槽 —— 那会让每个控件各长一套。

## §Ⅲ 布局

- 界面一律元素树：**不手算 `Rect::new` 命中、不每帧重建树、不在渲染期读指针判悬停**
  （common 禁区；框架三失效 `tree.rs:335/338/341`，位置由 arrange 决定）；
- margin / 对齐 / min / max / grow / shrink / visible 全走 `LayoutProps`（`element/src/props.rs:99`），
  控件不自解释这层（§Ⅳ.2）；
- **一维用 `Stack`，二维用 `Grid`**：行列单元写附着属性 `LayoutProps::grid_cell`
  （`props.rs`，对应 XAML `Grid.Row/Column`），**不允许**用「固定宽 + margin」手工拼列 ——
  那等于把行列坐标手写一遍，正是框架要替开发者做的事（`element/src/widgets/grid.rs`）；
- **动画只动视觉**：`UpdateCtx` 不放量测失效（`widget.rs:457-460` 注释）；平移动画用
  `invalidate_arrange`（`widget.rs:459`）；
- 滚动容器声明 `scrolls_children`（`widget.rs:140`）豁免包含断言，裁剪由 `clips_children`（`widget.rs:134`）。

## §Ⅳ 事件 / 动作

- 控件只处理框架事件（`Event`，`element/src/event.rs`），`ctx.set_handled()` 截停冒泡（`widget.rs:374`）；
- 对外只发**类型化动作**：`ctx.emit(ActionStruct)`（`widget.rs:378`）→ App 在 `on_action` downcast
  （harness TreeApp，`tree_host.rs`）；动作命名见 §Ⅰ.2；
- 点击合成由框架负责（§Ⅵ `Click`）；控件不自己判「按下→抬起」；
- 长任务/计时用 `request_timer`（`widget.rs:319`）与 `tick`，不引线程。

## §Ⅴ 视觉状态

- 状态源 = 框架 `ControlStates`（`widget.rs:22`：hovered/pressed/focused/keyboard_focused/disabled），
  `paint` 里读 `ctx.state()` 设定目标；过渡插值用 `VisualState`（`visual_state.rs:13`）+ `ctx.animate`；
- **视觉状态名 PascalCase 强制**（本文件 §Ⅰ.2 同源裁定；对应 WinUI GoToState 的状态名，
  `WINUI_REFERENCE_MAP.md` #5）；
- 只有键盘焦点画焦点框（`widget.rs:26-28` 注释、`focus_visual` `widget.rs:124`）；
- 禁用态由框架传播（`ControlStates.disabled` = 自身或祖先，`tree.rs` `effectively_disabled`），
  控件不自判祖先链。

## §Ⅵ 尺寸与颜色

- **颜色只用 `ThemeColor`**（`core/src/brush.rs:16`；common 禁区）——禁裸色值；
- **尺寸只用尺寸正典（N18）或卡面允许的常量**；正典未覆盖的，定义具名 `const` 并注来源，
  禁裸魔数（强制层 §Ⅸ 挡新增代码的裸色值/裸尺寸）；
- 尺寸令牌表由 size-tokens 线建立（本批未合并，落地后此处引用其表）。

## §Ⅶ 错误 / 诊断

- 库层用 `log`（`error!` 带 `kanesumi-<crate>:` 前缀，如 `tree.rs:595` `insert` 父不存在改挂内容根）；
  不 panic——库不崩外壳；
- 测试里**panic 并说明**：字体缺失 panic 带修复提示（`element/src/testing.rs:9`、`:45`）——
  「静默跳过的测试比没有测试更危险」（testing.rs 头注）；
- `expect`/断言消息用中文写明语义与实际值（如 testing.rs `:236` 矩形越断言）；
- 诊断入口：`type_name`（`widget.rs:189`）+ `WidgetId::to_u64`（`id.rs:26`，无障碍节点 id / 日志用）。

## §Ⅷ 测试

- 每个控件必须有 `mod tree_tests`：**三断言**（`assert_contained` / `assert_no_hit_outside` /
  `assert_paint_within`，`testing.rs:222-323`）+ **无障碍契约** `assert_access_contract`
  （`testing.rs` 末段，e3 起）；弹层类另有「开着过保险」模式（`dialog.rs:573`、`teaching_tip.rs:719`）；
- 行为变更**先红后绿**，失败证据保留到 `out/<task>/`；不删测试、不改断言、不加 `#[ignore]`、
  不放宽阈值（common 禁区）；
- 契约检查必须带**反向自检**（故意造违例断言检查报错），先例 `element/src/access.rs` 测试组
  （`contract_flags_unnamed_focusable` 等）；
- 无外壳驱动用 `TestHarness`（`testing.rs:50`）；真机/无头截图证据放 `out/`，禁 `/tmp`。

## §Ⅸ 强制层（本卡落地的最小机械检查）

- `clippy.toml` `disallowed-methods` / `disallowed-types` 草案 + `tests/` 静态检查测试
  （禁裸色值 / 裸尺寸 / 应用层 `Rect::new` / 动画路径 `Instant::now`），**含反向自检**；
- 先只对**新增代码**生效（检查测试只扫本批白名单文件），不批量改旧代码——旧代码的
  历史豁免记 `CANON_VS_TEMPORARY`，退役节奏随 E5。

## §Ⅹ 提交与文档

- 一个控件/一件事一个提交；subject 格式 `feat(controls): <内容>`（仓库 log 惯例，中文正文）；
  正文列动作类型、事件映射、测试、顺带修复（`ELEMENT_MIGRATION.md` §7）；
- 告警数不得超过基线（同 §7：controls lib 2 / lib test 36——容器内逐 target 对照）；
- 文档：结论带 `文件:行号` 出处，查不到标「推断」；设计变更先更 `docs/` 再动代码；
- 证据（日志 / 截图 / report.md / proof.json）放 `out/<task-id>/`，不进仓、不进 `/tmp`；
- 不推送、不安装、不清理其他会话现场；同一错误三次停下写报告。
