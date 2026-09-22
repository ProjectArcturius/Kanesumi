# Kanesumi 框架路线图（2026-09-22 立）

> **与 `PORT_ROADMAP.md` 的分工**：那份管**控件层**（WinUI 控件移植，已清空开源项）；
> 本文件管**框架层**（保险机制、协调、优化）。两者不重叠。
>
> **配套文档**：`MATURITY_AUDIT_2026-09-22.md`（缺口证据与优先级）、
> `REFERENCE.md`（微软栈源清单与对照表）、`CANON_VS_TEMPORARY.md`（临时值登记）、
> `COMPOSITION.md`（构图强制契约）。
>
> **本文件是「接下来做什么」的唯一真源。** 每完成一批，先改本表状态，再动其他文档。

---

## §Ⅰ 定位：做什么、不做什么

判断依据（维护者 2026-09-22）：UWP 真正强在**保险机制**（开发者不必预防"按钮飞出窗口"
"文本大于文本框"）与**极其好的优化**；一体动画与设计语言 Kanesumi + Sokuou **已有**。

**做**：保险机制、元件协调、优化纵深。

**不做**（明确写下来，防止反复）：
1. **不重建动画架构** —— Sokuou 是唯一真源，本路线只补词汇表覆盖与"接线"。
2. **不照搬依赖属性 / 附加属性** —— Rust 侧代价大于收益，只借其**失效传播**语义。
3. **不引入 Fluent 外观** —— 大圆角、Acrylic、长动效一律不取（参 `REFERENCE.md` §Ⅰ）。
4. **不为"原创"另起炉灶** —— 有权威实现可读就先读（WPF/WinUI 3 均已开源可读）。

---

## §Ⅱ 三条攻坚线

| 线 | 缺口本质 | 对应阶段 |
|---|---|---|
| **A 保险机制** | 结构上仍可能写出"飞出窗口 / 字大于框 / 点不到" | M1、M2 |
| **B 协调** | 无焦点管理器、无指针捕获、无右键目标语义 | M3 |
| **C 优化** | 无保留树、无布局失效、xdg 角色不吃损伤、无容器回收 | M4、M5 |

M6 是 A/B/C 的收尾（动画词汇补齐 + 优化可见化）。M7 是独立长线（无障碍）。

---

## §Ⅲ 阶段与批次

状态记号：⬜ 未开始 ｜ 🔵 进行中 ｜ ✅ 已完成 ｜ ⏸ 待裁定阻塞

### M1 · 令牌纪律（保险机制线）—— **进行中**

**目标**：控件不可能再写出错颜色；未知令牌必须报错（对标 WinUI 的
`XamlResourceReferenceFailed`：写错必须报错，不能静默回退成另一个样子）。

| 批次 | 内容 | 状态 |
|---|---|---|
| M1-1 | **令牌分类学 + 来源对齐**：把魔法 alpha 按语义归族（中性交互 / 列表高亮 / 强调衬底 / 不透明度乘数 / 遮罩），每族对齐权威来源；**并裁定 `CONTROL_SPEC` 的一处自相矛盾**（§65 说 PointerOver 白 10%，§215 说列表 PointerOver ≈30%） | 🔵 |
| M1-2 | **迁移**：剩余内联字面量 6 处 / 5 文件；魔法 alpha 约 30 处 / 20 文件 → 令牌 | ⬜ |
| M1-3 | **静态检查**：测试扫描 `kanesumi-controls/src/*.rs` 生产段，禁止 `Color::from_hex/rgb/new/from_rgba` 与裸 `with_alpha(<数字>)` | ⬜ |
| M1-4 | **对比度自检扩展**：新增令牌纳入 WCAG 断言（正文 ≥4.5 / 次级 ≥3.0） | ⬜ |

**验收**：M1-3 的静态检查通过 + 全量测试绿 + `CANON_VS_TEMPORARY` 临时项减少。

**已记账（M0，2026-09-22 完成）**：溢出契约（默认省略号 + `label/paragraph` + 框矮于一行不静默消失）、
容器裁剪 6 处、可达 panic 5 处、`shm_open`/IME 边界/`guard()` 鲁棒性、`Accent` 色阶、深浅双态令牌、
`StatusColors` 语义色、`FocusRing` + Tab 路由。

### M2 · 布局接管（保险机制线，最大的一块）

**目标**：一个屏幕的坐标只在**一处**定义；控件矩形由引擎产出；`child ⊆ parent` 可断言。

| 批次 | 内容 | 依赖 |
|---|---|---|
| M2-1 | `LayoutLeaf` 契约补 `clips_children()`（默认 true）+ `arrange` 强制 `Constraints::max` | — |
| M2-2 | 三套布局去留裁定后收敛（`canvas/layout`（两遍约束）/ `structure/ui`（单遍 cursor）/ `structure/grid`（零消费者）） | **⏸ 待裁定** |
| M2-3 | 控件补 `measure()` / `hit_test()`：现 13/47 有 measure、11/47 有 hit_test | M2-1 |
| M2-4 | `kanesumi-gallery` 三页全量迁移为**参照实现**（真实 rect 由引擎产出） | M2-2 |
| M2-5 | 通用回归：「控件矩形外 1px 不可命中」「所有绘制矩形 ⊆ 祖先裁剪」 | M2-1 |

**验收**：gallery 迁移后视觉回归通过；M2-5 两条通用测试覆盖全部已迁移控件。
**风险**：高。**不碰 Ether 侧应用**（那属另一轮，需单独评估）。

### M3 · 交互协调（协调线）

**目标**：纯键盘可操作；指针捕获与右键目标由框架记录，App 不再自己维护。

| 批次 | 内容 |
|---|---|
| M3-1 | `FocusRing` 接入各可聚焦控件（`is_tab_stop`）；方向键遍历 |
| M3-2 | 指针捕获（拖拽 / Slider 拖动 / 文本选择）—— 旧实现在 App 侧自持 |
| M3-3 | `PointerEntered` / `Left` 与**真实 Modifiers**（现恒 default，KeyReleased 缺失） |
| M3-4 | `ContextTarget` 一等语义：`PointerPressed` 时框架记录命中项，菜单只能读它（对应 arc-deck R15「右键作用于错误行」） |
| M3-5 | 输入模型补全：`KeyReleased`、`TextInput` 与 IME `Commit` 分立 |

**验收**：`kanesumi-gallery` 全控件可纯键盘走完；M3-4 有回归测试。

### M4 · 优化：保留树 + 失效传播（优化线，**收益最大**）

**目标**：静止零重绘；变更只重算受影响子树；「动画只动视觉属性」在结构上有保证。

| 批次 | 内容 |
|---|---|
| M4-1 | `RetainedScene` 真增量：去掉每帧两次全量 `render_decl`，命中表复用同一 `LaidTree` |
| M4-2 | `diff_decl` keyed reconciliation（现按位置路径匹配 → 列表增删导致后续全 `Changed`） |
| M4-3 | 布局失效标记（`invalidate_measure` / `invalidate_arrange`），命中即复用 |
| M4-4 | damage 由 diff 产出，接 `App::damage_hint`（不再由 App 手工算） |
| M4-5 | xdg-shell 角色吃损伤重绘（现只有 CPU 路径有） |

**验收**：A/B 基准（帧耗时 / 命令数 / `layout_misses`）有数字对比；
「增量结果与全量路径逐条等价」测试。
**风险**：中高。**前置**：M4-3 之前不得做 M5 的回收优化（流沙上优化）。

### M5 · 优化：滚动与虚拟化统一

**目标**：任何内容放不下就有滚动的**统一**容器，而不是每种控件各写一套。

| 批次 | 内容 |
|---|---|
| M5-1 | 通用 `ScrollHost`（clip + 虚拟化 + 命中三合一）；`MetroScrollView` 补 `render` |
| M5-2 | `SelectorFlyout` 504px 截断不可达 → 接滚动 |
| M5-3 | `DropdownMenu` 面板高度夹紧（现可画到屏外）；`place_popup` 契约变更需维护者确认 |
| M5-4 | `TreeView` 接滚动；`MetroRepeater` 容器回收（依赖 M4-2 的 keyed） |

### M6 · 动画词汇补齐 + 优化可见化

| 批次 | 内容 | 来源 |
|---|---|---|
| M6-1 | `PointerDown/UpThemeAnimation`：按钮 100ms Y 下沉（现 17 个交互控件硬切换） | `REFERENCE.md` §9.4 |
| M6-2 | stagger 入场（`EntranceThemeTransition` 语义） | `ANIMATION_SPEC.md` 处置表 |
| M6-3 | `ControlFastAnimationDuration = 0.167s` 对齐（权威值已取到，见 `REFERENCE.md` §9.4） | 同上 |
| M6-4 | 帧统计浮层 + 过度绘制可视化（对标 `EnableFrameRateCounter` / `IsOverdrawHeatMapEnabled`） | `REFERENCE.md` §Ⅲ.1 |

### M7 · 长线（P2）

无障碍语义树（对照 UIA，导出 AT-SPI）· 系统剪贴板 · 拖放 · `FontWeight` 真字面选择 ·
文本缓存失效 API（字体 identity / text-scale）。

---

## §Ⅳ 执行纪律（每批都必须满足）

1. **一个功能块一次 commit**，Conventional Commits 前缀、中文正文、句号结尾（参 `AGENTS.md`）。
2. **未经明确指示不得 push**。
3. 提交前 `cargo test --workspace` 全绿、`cargo clippy --workspace --all-targets` 无新增告警。
4. 行为变更（尤其默认值）**必须带回归测试**，并在 `CANON_VS_TEMPORARY.md` 或本文件留痕。
5. 数值来源必须可追（先读源再写规格再写码）；无源无实测时**登记为临时**，不伪装成正典。

---

## §Ⅴ 阻塞本路线的待裁定项

| # | 待裁定 | 阻塞 |
|---|---|---|
| 1 | 三套布局的去留（`canvas/layout` / `structure/ui` / `structure/grid`） | M2-2（进而 M2-4） |
| 2 | `place_popup` 契约是否可破（`CONTEXT_MENU_SPEC.md` 曾冻结其签名） | M5-3 |
| 3 | T3：WinUI 的 Base/Secondary/Tertiary 分层 ↔ 本项目 background/surface/surface_variant 的映射 | M1-2 的亮色中性色替换 |
| 4 | T8：两个 `press_tint`（`colors` 白 10% 与 `indication` 白 22%）收敛到哪一个 | M1-1 |
| 5 | `CONTROL_SPEC` §65（PointerOver 白 10%）与 §215（列表 PointerOver ≈30%）哪个是本意 | M1-1 |

---

## §Ⅵ 跨仓联动（Ether 侧，不属本路线但会互相等待）

记录在此，避免"Kanesumi 做完了但没人接"：

1. **Chorus 主题消费端接线**：harness 侧已就绪（`App::set_theme` + `system_theme`），
   需要 `settings` / `librarian` 实现 `set_theme` 并返回它，才能"改 accent 全应用变色"。
2. **设置模块反转**（`DEPENDENCY_MODEL.md` 规则 2）：Settings 不再硬编码产品类目。
3. **媒体键与快捷键服务**（`SETTINGS_DESIGN.md` 附 · B）：`org.ether.Shortcuts` + Settings 界面。
4. **Settings 类目内容**：先取 Windows / macOS / GNOME / KDE 并集，再按本机裁，最后 polish。
