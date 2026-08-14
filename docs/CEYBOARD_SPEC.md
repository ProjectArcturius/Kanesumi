# Ceyboard 规格（CEYBOARD_SPEC）

> 输入法系统产品规格。参 `ETHER_PRODUCTS.md` §4.3、`PLAN.md` §2.4、`IME_WIRING_PLAN.md`、
> `CONTROL_SPEC.md` §34（TextBox IME 组合态）/ §35（PasswordBox）、`compositor/docs/SD.md` §语言环境。
>
> 状态：**草案（2026-08-13 定）**。视觉参考 Win10 微软拼音「新体验」候选窗；无平台公开规格
> （微软未发布几何文档），Kanesumi 定夺值标注「**定**」，参考值标注「参考」，需 Gallery 逆推
> 确认的标注「待逆推」。

---

# Ⅰ · 定位与职责

## 1. 产品一句话

**Ceyboard 是 Ether 的输入法系统**：以 fcitx5 核心 + libime 为引擎（LGPL，动态链接），
Kanesumi 为 UI（`ETHER_ROLE=candidate`），提供中日韩等东亚语言输入——候选词窗口 + 状态指示。

## 2. 产品边界（参 ETHER_PRODUCTS.md §4.3）

| 项 | 定夺 |
|---|---|
| 引擎基座 | fcitx5 核心库 + libime（**LGPL-2.1-or-later**，动态链接无传染） |
| UI 实现 | **Kanesumi**，候选窗/状态指示用 Kanesumi 控件，**不重写 UI** |
| 角色 | `ETHER_ROLE=candidate`，layer-shell **OVERLAY**（参 harness `role.rs`） |
| 引擎关系 | **不 fork fcitx5 仓库**；上游经 Debian 系统库升级维护 |
| 词库/引擎插件 | 引擎链全 LGPL（fcitx5/libime/chinese-addons，实证见 `reference/fcitx5/NOTES.md` §六）；`pinyin.lua`（GPL 脚本）作数据文件入 `ether-langpack-zh-ime` |
| 平台 | **仅 Wayland**（input-method-v2 引擎宿主 + layer-shell 候选窗；无 X11/XWayland 目标） |
| 状态 | 计划（Kanban 待立项） |

## 3. 职责清单

- 候选词窗口：组合态拼音 → 候选词列表，位置跟随光标（文本字段），选词上屏。
- 状态指示：当前输入法 / 中英状态切换指示。
- 输入法切换与管理：多输入法（中文/日文/韩文）切换、配置。
- 引擎桥接：经 `zwp_input_method_v2` 连接合成器（引擎侧），经 `zwp_text_input_v3` 与文本字段互操作（合成器桥接）。

## 4. 非目标

- 不做软键盘 / 屏幕键盘。
- 不做语音 / 手写输入。
- 不内嵌浏览器等 Chromium 输入法实现。
- 不重写 fcitx5 引擎——只做 UI 宿主 + 引擎链接。

---

# Ⅱ · 角色契约（`ETHER_ROLE=candidate`）

## 1. 角色归属（参 harness `role.rs`、PLAN.md §4.3）

`EtherRole` 当前无 `candidate` 变体（只有 Desktop/Browser/TopBar/Dock/Launcher），Ceyboard
立项时新增：

```rust
/// 候选窗 / 状态指示（layer-shell OVERLAY，跟随光标）。参 CEYBOARD_SPEC §Ⅱ。
Candidate,
```

- `FromStr`: `"candidate" => Ok(EtherRole::Candidate)`
- `surface_kind()`: `Candidate => SurfaceKind::LayerOverlay`
- 合成器 `spawn_apps`（`compositor/src/state/companion.rs`）以 `ETHER_ROLE=candidate` 拉起。

## 2. 表面模型

| 表面 | 协议 | 层 | 说明 |
|---|---|---|---|
| 候选窗 | `zwp_input_method_v2` popup（`InputMethodHandler::new_popup` 关联 Layer 6 Overlay） | Overlay | 组合态出现，随光标定位；键盘焦点**不在本表面** |
| 状态指示 | layer-shell OVERLAY（或并入 TopBar Control Gate，Phase 2 定） | Overlay | 常驻 / 切换瞬间浮现 |

> **关键不变量**：候选窗不抢键盘焦点。键盘事件仍进文本字段（合成器把按键同时转发给 IME——
> input-method-v2 的 key 事件路径），IME 返回 preedit/候选/commit。

## 3. 生命周期

```
合成器 spawn（ETHER_ROLE=candidate）
  → 连 Wayland，绑定 zwp_input_method_v2（引擎侧，fcitx5 核心）
  → 绑定 zwp_text_input_v3? 否 —— 文本中继由合成器桥接（text-input-v3 ↔ input-method-v2）
  → 等待合成器 new_popup（文本字段获焦 + 组合态开始）
  → popup 管理（位置 / 内容 / 翻页）
  → commit 上屏 / 取消 → 候选窗消失
```

---

# Ⅲ · 候选窗视觉规格（核心）

> 参考：Win10 微软拼音「新体验」候选窗。微软闭源无公开几何，Kanesumi 定夺如下。

## 1. 形状

| 项 | 值 | 依据 |
|---|---|---|
| 圆角 | **`Square`（直角）** | Kanesumi 铁律：直角切一切（参 KANESUMI_DESIGN §1） |
| 边框 | **无**（border 占位 transparent，同 Button §1 惯例） | 贴边无边框 |
| 阴影 | **无** | 铁律 6：深度靠明暗不靠阴影 |
| 面板底色 | `surface`（不透明）或 `surface_variant`（深色桌面用） | 同 Control 惯例；深浅色跟随 Chorus |
| 透明度 | 不透明（Kanesumi 深色空间桌面惯例，不做磨砂） | CONTROL_SPEC §通用规律 |
| 圆角过渡 | 无（纯直角，无渐变过渡） | — |

## 2. 结构（横排单行，2026-08-14 修订）

```
┌──────────────────────────────────────────────┐
│ 1.你好  2.尼豪  3.泥蒿  4.逆好  …    ›        │
└──────────────────────────────────────────────┘
```

| 区 | 说明 |
|---|---|
| **候选横排** | 单行横向延伸，每候选 = 序号 + 词（`1.你好 2.尼豪 …`）；微软拼音「新体验」横排模式 |
| **preedit** | **不显示在候选窗**——拼音内联在文本字段（合成器 text-input 桥接），候选框只显示候选词 |
| **高亮** | 当前选中项以 `primary` 色块包裹（序号 + 词一体） |
| **翻页** | 超出面板右缘 → 省略截断 + 右下角 `›` 翻页指示；键盘 +/- 或 PageUp/Down 翻页 |

## 3. 尺寸

| 项 | 值 | 状态 |
|---|---|---|
| 面板左右内边距 | 8px | **定** |
| 面板上下内边距 | 4px | **定** |
| 候选行高 | 32px | **定**（对齐 Slider 32 / 列表行高惯例） |
| 序号 | 约 10px 宽，+2px 间隙 | **定** |
| 候选词 FontSize | 16px（微软拼音候选字号偏大） | 参考，待逆推 |
| 候选间距 | 10px | **定** |
| 高亮块内边距 | 序号左 4px / 词右 4px | **定** |
| 面板最大宽度 | 依内容，上限 480px（超出省略截断） | 参考，待逆推 |
| 面板高度 | 单行 = 上下边距 4+4 + 行高 32 = 40px | **定** |

## 4. 颜色与状态

### 4.1 候选项状态

| 状态 | 序号 | 候选词前景 | 项背景 | 依据 |
|---|---|---|---|---|
| Normal | `on_surface` 50% | `on_surface` | 透明 | — |
| Highlight（当前选中） | `on_primary`（白） | `on_primary`（白） | **`primary`（强调色）** | 列表类「选中用强调色」（通用规律 5）；微软拼音高亮蓝底白字 → Kanesumi primary |
| Pressed（按下，可选） | — | — | `primary` + press_tint | 通用规律 2 按压位移，Kanesumi 用 tint |

> preedit 不显示在候选窗（拼音内联文本字段）；无独立 preedit 行。组合态光标在
> 文本字段由 text-input-v3 桥接呈现（CONTROL_SPEC §34）。

### 4.3 颜色切换

**瞬时硬切换**（通用规律 1：`DiscreteObjectKeyFrame`，无颜色过渡动画）。高亮随候选
翻页/按键即时跳变，不做渐入渐出。

## 5. 动画

| 场景 | 动画 | 参数 |
|---|---|---|
| 候选窗弹出 | `Progress` 驱动（SettleIn/Fade 平移，CONTROL_SPEC §9/§10 动画表） | 0.25s Quadratic/EaseOut（Metro 时代轻盈短促） |
| 高亮切换 | 瞬时 | — |
| 翻页换内容 | 瞬时 | — |
| 候选窗关闭 | Fade out（可选） | 0.2s |

> 铁律：动画只动视觉属性（位移/透明），不动布局；进度驱动，无时间线（AnimRules §III）。

## 6. 状态指示（中/英 切换）

| 项 | 值 |
|---|---|
| 形态 | 小直角面板：图标 + 文字（如「中」「英」或输入法名缩写） |
| 形状 | `Square`，无边框无阴影，同候选窗铁律 |
| 位置 | Phase 1：跟随候选窗底部浮现；Phase 2：并入 TopBar Control Gate |
| 切换 | 点击切换中/英（Ctrl+Space 或 Super+Space 快捷键，Phase 2 定） |
| 动画 | 浮现 Fade in 0.2s，进度驱动 |

---

# Ⅳ · 行为与交互

## 1. 键盘（焦点在文本字段，合成器转发）

| 键 | 行为 |
|---|---|
| 数字 1–9 | 直接选第 N 候选并上屏 |
| 空格 | 选高亮候选（中文输入典型：空格选第一候） |
| 回车 | 提交高亮候选 |
| ↑ / ↓ | 高亮上移/下移（翻页边界自动翻页） |
| PageUp / PageDown 或 + / - | 翻页 |
| Tab | 候选窗内移动高亮（可选） |
| Esc | 取消组合态，清 preedit（CONTROL_SPEC §34：Escape 打断组合态） |
| 直接键入非候选键 | 打断组合态（CONTROL_SPEC §34 同款） |

## 2. 鼠标（候选窗表面内）

| 操作 | 行为 |
|---|---|
| 左键点击候选行 | 提交该候选上屏 |
| 滚轮 | 翻页 |
| 左键点击 preedit 行 | 无操作（或移动组合态光标，Phase 2 定） |

## 3. 位置

- 候选窗锚定**文本字段光标矩形**（`parent_geometry`：合成器 `input_method.rs` 返回焦点窗口
  内容矩形，见 `compositor/src/state/input_method.rs`）。
- 空间不足 → 候选窗向上翻（同 `popup.rs` `place_popup` 自适应，参 kanesumi popup）。
- 位置变化随光标移动即时更新（`popup_repositioned`）。

## 4. 密码字段

- `ime_focus()` 返回 `content_hint = Password` → harness 映射 `content_purpose = password |
  content_hint = sensitive_data | hidden_text`（fcitx5 自禁候选窗，CONTROL_SPEC §35）。
- 密码字段组合态**不弹候选窗**，周边文本不外发。

---

# Ⅴ · 协议接线（合成器 + 客户端）

## 1. 合成器侧（2026-08-14 已补全）

| 项 | 现状 | 待办 |
|---|---|---|
| `text_input_manager` / `input_method_manager` | ✅ 已注册（`compositor/src/state/mod.rs`） | — |
| `InputMethodHandler` | ✅ 已补全（`input_method.rs`）：`new_popup` 存列表、渲染时叠 Layer 6 Overlay、`parent_geometry` 匹配焦点窗口矩形 | 候选窗 popup 关联渲染已验证（`render/mod.rs` `collect_im_popup_draws`） |
| `delegate_text_input_manager!` / `delegate_input_method_manager!` | ✅ 已注册 | — |
| 合成器 spawn Ceyboard | ✅ 切换为 `ceyboard` + `ETHER_ROLE=candidate`（`companion.rs`） | — |

## 2. 客户端侧（kanesumi-harness，已完成）

`IME_WIRING_PLAN` 阶段 A–E 已全部落地：

- `text_field.rs`：preedit 状态、`commit_ime`/`delete_surrounding`/`surrounding_text`（含单测）。
- `text_box.rs`：preedit 渲染 + 虚线下划线 + 光标位移 + `ime_context()`。
- `ime.rs`：`ImeContext`/`ImeContentHint`。
- `app.rs`：`InputEvent::{Preedit, Commit, DeleteSurrounding}` + `App::ime_focus()`。
- `platform.rs`：`reconcile_ime` 幂等 + `Dispatch<ZwpTextInputV3>` 事件批处理 + serial 防 stale。

## 3. Ceyboard 进程（2026-08-14 已落）

Ceyboard crate（`ceyboard/`，bin `ceyboard`）：

- `engine.rs` — `ImeEngine` trait + `EngineCommand`/`EngineEvent`/`EngineSnapshot` 契约。
- `mock.rs` — 纯 Rust 模拟引擎（无 libime 环境跑通全链路）。
- `app.rs` — `CeyboardApp`（App trait：候选窗驱动 + 键盘/鼠标路由 + 中英切换）。
- `ffi/` + `libime_engine.rs` — libime 真实拼音引擎（cxx 桥接，`libime` feature）。
  `create_libime_engine` 加载 `/usr/share/libime/sc.dict`（Binary）+ zh_CN.lm（resolver）；
  拼音候选 / 选词上屏 / 退格 / 翻页全部真实 libime 语义。

> 引擎桥接（libime LGPL 动态链接）已通：`nihao → 你好` 候选链实测。

## 4. 引擎宿主协议绑定（2026-08-14 已落）

harness 作为 `zwp_input_method_v2` 引擎宿主的客户端绑定已落地：

- `App` trait 增引擎宿主钩子：`ime_engine_host()` / `ime_engine_key()` /
  `ime_engine_preedit()` / `ime_engine_take_commit()` / `ime_engine_take_delete()`。
- `platform.rs`：`Shell` 绑定 `zwp_input_method_manager_v2` → `get_input_method(seat)` →
  `grab_keyboard`；`Dispatch<ZwpInputMethodV2>`（activate/deactivate/done）+ 键盘 grab
  keymap/key/modifiers（xkbcommon 语义化）→ `App::ime_engine_key` → 引擎。
- `flush_engine()`：引擎 preedit → `set_preedit_string`（幂等缓存）、commit →
  `commit_string`、`delete_surrounding_text`，最后 `commit(serial)` 上屏。
- CeyboardApp 实现全部引擎宿主钩子（libime 引擎驱动）。

> 候选窗视觉：引擎进程经 layer-shell OVERLAY 渲染候选窗（当前路径）；
> 输入法 popup surface（`get_input_popup_surface`）的候选窗渲染留待后续（合成器
> `collect_im_popup_draws` 已就绪）。

---

# Ⅵ · 语言包与分发

## 1. 词库与引擎插件（PLAN §2.4，2026-08-14 修订）

```
ether-langpack-zh-ime   # 词库数据 + Ceyboard pinyin 配置 + pinyin.lua（GPL 数据脚本）
```

**许可证（2026-08-14 实证，见 `reference/fcitx5/NOTES.md` §六）：**

- fcitx5 核心、libime、fcitx5-chinese-addons 引擎链**全部为 LGPL-2.1-or-later**
  （chinese-addons 的 metainfo `GPL-2.0+` 系模板遗留，逐文件核查无引用）。
- pinyin 引擎插件（`.so`，LGPL）**直接随 Ceyboard 动态链接分发**，不再隔离进语言包。
- **唯一 GPL 文件 `im/pinyin/pinyin.lua`**（时间/日期/符号转换脚本）作**数据文件**入语言包，
  不随 Ceyboard 二进制编译/链接，规避传染争议（与 fcitx 官方「lua 扩展与核心分离」惯例一致）。
- Ceyboard 本体（Apache-2.0）动态链接 LGPL 库（fcitx5 核心 + libime + pinyin 插件），**不受传染**。
- LGPL-2.1 §6 义务：动态 `.so` 分发、保留版权声明、附许可证文本、允许 relink。

## 2. 与语言包的关系（SD §语言环境）

Ceyboard 是「语言环境」（Locale + 字体 + 输入法 + Fallback + UI 对照 + Typographic Policy）
的输入法消费端。装 `ether-langpack-zh-ime` → 中文可输入（词库 + pinyin.lua 数据 + 配置）；
不装 → 仅拉丁输入（引擎基座仍在，只是无中文词库）。

---

# Ⅶ · 验收标准（对应 XDG_PLAN §7）

- `konsole`（或任意 Kanesumi TextBox 应用）聚焦后按拼音 → 候选窗弹出，跟随光标。
- 数字选词 / 空格选高亮 / 方向键 / 翻页全部可用。
- 候选词经 commit 上屏，进入文本字段（无方框）。
- 深/浅主题切换 → 候选窗颜色跟随 Chorus。
- 密码字段不弹候选窗，周边文本不外发。
- 中/英切换状态指示可见、可切换。
- 候选窗不抢键盘焦点（焦点始终在文本字段）。

---

# Ⅷ · 与 Kanesumi 的关系

| 项 | 归属 |
|---|---|
| 候选窗 UI | **新 Kanesumi 控件**（候选窗控件，CONTROL_SPEC 新增 §44）——纯视觉 + 状态，引擎无关 |
| 状态指示 UI | 新控件（或并入 Control Gate） |
| 角色 | harness `role.rs` 增 `Candidate` |
| 引擎桥接 | Ceyboard 进程内部（fcitx5 核心动态链接），不经 Kanesumi |
| 主题 | 跟随 `MetroTheme` / Chorus（深浅色 token） |

> **Kanesumi 只负责画，Ceyboard 负责想**：候选窗控件是纯展示（内容注入、高亮态、翻页态），
> 候选生成/选词逻辑全在 Ceyboard 引擎层，控件不持输入法状态。

---

# Ⅸ · 施工顺序建议（2026-08-14 已全部完成）

1. ✅ **合成器轨 P2-1 补全**（`input_method.rs`：popup 关联 Overlay + `parent_geometry` 精确化）。
2. ✅ **候选窗控件**（CONTROL_SPEC §44，`MetroCandidateWindow`）。
3. ✅ **harness `Candidate` 角色** + `companion.rs` spawn 切换。
4. ✅ **Ceyboard 进程**（引擎抽象 + mock 引擎 + 候选窗驱动 + libime 真实引擎 FFI 桥接）。
5. ✅ **状态指示**（中/英切换，Phase 1 候选窗底部）。
6. ✅ **语言包 `ether-langpack-zh-ime`**（manifest + staging + 打包说明）。
7. ✅ **引擎宿主协议绑定**（`zwp_input_method_v2` 客户端：grab keyboard + preedit/commit 上屏）。

**剩余（后续单独立项）：**

- 输入法 popup surface（`get_input_popup_surface`）候选窗渲染（当前 layer-shell OVERLAY 路径）。
- 状态指示 Phase 2 并入 TopBar Control Gate。
- 语言包实际词库数据落位 + `pinyin.lua` 数据脚本分发。
