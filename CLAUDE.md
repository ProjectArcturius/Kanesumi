# CLAUDE.md — Kanesumi（Ether 扇区）

## 构建与测试

所有命令从本仓根目录执行：

```bash
cargo check          # 检查所有 crate
cargo test           # 单元测试（当前 404 个）
cargo clippy
cargo fmt
```

### Gallery（daily driver，Linux）

```bash
# 需要在 Plasma（或任意支持 xdg-shell/layer-shell 的合成器）中运行
cargo run -p kanesumi-gallery                # xdg-shell 窗口（Browser 角色）
ETHER_ROLE=topbar cargo run -p kanesumi-gallery   # layer-shell TOP（测试 role 分派）
KANESUMI_TEST_FONT=/path/to/font.ttf cargo run -p kanesumi-gallery  # 指定字体
```

> 网络提示：`kanesumi-anim` 以 git 依赖拉取 sokuou。若环境代理导致 cargo 直连 git 失败，
> 在 `.cargo/config.toml` 设 `git-fetch-with-cli = true`（本仓已配置）。

## 架构

本仓是 **Ether 扇区·主仓（mainline）**，Rust workspace，8 个 crate：

```
kanesumi-core       (无依赖 — 设计 tokens / 主题 / MetroText / 交互指示 / 几何原语)
    ├── kanesumi-anim      (dep: sokuou(git) — Kanesumi 动画预设，消费 Sokuou)
    ├── kanesumi-canvas    (dep: kanesumi-core — 2D 图形：Scene 渲染命令 + TextEngine 排版)
    ├── kanesumi-structure (dep: kanesumi-core + canvas — 导航状态机 + 壳布局)
    ├── kanesumi-controls  (dep: core+canvas+anim — Kanesumi 控件库)
    ├── kanesumi-harness   (dep: core+canvas+anim+structure+controls+appmenu — 应用壳：App trait /
    │                       角色解析 / Linux Wayland+wgpu 外壳：platform.rs + render.rs)
    ├── kanesumi-appmenu   (Linux-gated dep: wayland-client/wayland-backend/wayland-scanner/zbus5 —
    │                       全局应用菜单：dbusmenu 服务 + org_kde_kwin_appmenu 绑定 + Registrar；
    │                       独立可用，eframe 应用（ether-librarian）直接依赖)
    └── kanesumi-gallery   (dep: core+canvas+anim+structure+controls+harness — Gallery 应用，daily driver)
```

> **Scene/TextEngine 归属（2026-08-10 拆分完成，参 Ether-main PLAN.md §6.1）**：渲染命令模型
> （`Scene`/`SceneCommand`/`TextAlign`）与字体度量（`TextEngine`/`Line`）驻 `kanesumi-canvas`
> （对应 UWP Win2D）。core 回归纯运行时（tokens/主题/排版/几何）。依赖方向
> `core ← canvas ← controls/harness/gallery`；controls 产出 Scene，harness 光栅化 Scene。

### 图标管线（kanesumi-canvas，2026-08-10）

- **`icon.rs`**：`rasterize_svg(path, size) -> Option<Icon>` —— resvg → tiny_skia → 直通 RGBA
  （与 Ether monorepo 侧 `ether-assets` 同款管线，Kanesumi 自足）。`Icon` = RGBA + 尺寸。
- **`scene.rs`**：`SceneCommand::Image { rgba, width, height, rect, tint }` + `Scene::image`。
- **harness `render.rs`**：Image 管线 —— RGBA8 纹理缓存（FNV 内容去重）+ IMAGE_SHADER
  （tint 白色=原色，其他=按 alpha 蒙版染色）+ 独立 draw pass。
- **controls `MetroIconButton`**：`with_svg` / `set_svg_icon` —— SVG 位图优先，否则 font glyph 回退。
- Gallery 的 Share 按钮用 SVG 图标演示全链路；资产 `kanesumi-gallery/assets/icons/`。

### 页面结构（kanesumi-structure，2026-08-10 填充）

- **`navigation.rs`**：`Navigation<PageId>` —— 对应 UWP `Frame.Navigate` 的状态驱动实现（无保留视觉树）。
  页栈 + 过渡进度（`navigate_to` / `go_back` / `can_go_back` / `is_transitioning` / `leaving_page`）。
  过渡进度可由应用层 Sokuou 动画驱动（`set_transition_progress`），本层不依赖 kanesumi-anim。
- **`layout.rs`**：`ShellLayout` —— `MetroShell::layout(window)` 一次划分 AppBar / 内容区（可选左侧导航栏）。
- **`lib.rs`**：`MetroShell<PageId>`（主题 + 导航 + AppBar 宿主）、`MetroAppBar`（标题 + 高度）、
  `MetroScaffold`（内容容器 + 内边距）。`render(engine, window)` 渲染背景 + AppBar，返回内容矩形。

### 稳固构图（2026-08-12）

完整契约见 `docs/COMPOSITION.md`。布局统一遵循约束 Measure/Arrange；文本走 BiDi +
OpenType shaping + fallback，显式 wrap/max-lines/ellipsis；Scene 裁剪只用成对
`PushClip`/`PopClip`；Wayland 外壳按 surface 支持整数与分数缩放。绘制、命中、弹层锚点
和 IME caret 必须消费同一布局产物。

### 声明式 DSL（kanesumi-controls/src/decl.rs，2026-08-10）

- **`Decl`** 元素树（纯数据、跨平台）：`Row`/`Column`（布局容器）/`Button`/`Text`/`Box`。
- **`view!` 宏**：Rust 原生声明式语法糖 → `Decl` 树（编译期检查，非字符串 DSL）。
- **`render_decl`**（reconciler）：声明式树 → 布局均分展开 → 现有控件渲染 → `Scene` +
  命中表（`DeclHit`）。App 消费命中表路由动作（`DeclAction`），无隐藏控件。
- **`diff_decl`**：按树位置路径匹配两帧声明式树，输出 `DeclChange`（Added/Removed/
  Changed/Replaced）——「保留视觉树 + damage 重绘」（PLAN §4.1 不变量 1/4）的逻辑基础。
- **`RetainedScene`**（retained.rs）：声明式树增量渲染 —— 首帧全量，后续 diff 驱动只
  重建变化元素命令段；输出命令序列 + 变化报告（damage hint）。为 harness 侧「只重绘
  变化区域」打基础。
- 状态驱动：App 每帧从状态产出 `Decl` 树，reconciler 展开为 Scene。增量 diff 已实现（`diff_decl`）。

### 错误边界（harness，2026-08-10）

`platform.rs`：`App::update`/`render`/`handle_input` 均用 `catch_unwind` 隔离 —— App
panic 不杀进程（记日志 + 跳过本帧）。合成器时钟 dt 限幅 50ms（§4.1 不变量 2）。

### 输入层（2026-08-10 完成）

- `InputEvent`（harness app.rs）增 `Scroll { x, y }` 变体；`platform.rs` 接线 Wayland Axis
  （离散步 50px/格 + 触摸板连续像素，+y 为正）。
- `MetroList` 增 `scroll_by`/`scroll_to`/`max_scroll` + 夹紧；`MetroDialog` 增 `hit_button`
  （Primary/Secondary/Close 身份命中）。
- 弹层方向自适应：`popup.rs` 的 `place_popup(trigger, size, screen, gap)` —— 下方空间不足时
  面板上翻（ComboBoxHelper 判据），上翻时右缘超出屏幕自动收拢。

### Linux 外壳（kanesumi-harness，Phase 3 续完成 2026-08-10）

- **`platform.rs`**：sctk 主循环。`EtherRole::surface_kind()` 分派 xdg-shell（Browser/Desktop）/
  layer-shell（TopBar/Dock/Launcher）；frame callback 驱动 `App::update(dt)` / `App::render(engine, size)`
  → `Renderer::render`；指针事件 → `App::handle_input`。
- **`render.rs`**：wgpu Scene 光栅化。形状 CPU 三角化（FillRect/StrokeRect 圆角、Arc 环形扇形）；
  文本 `TextEngine.rasterize` → R8 字形纹理 → textured quad；**非 sRGB 表面格式**（避免双伽马提亮）。
- **`app.rs` App trait 契约**：
  - `render(&mut self, engine: &TextEngine, size)` —— 外壳注入 TextEngine（排版唯一真源），App 用它量测、外壳用它光栅化；
  - `handle_input(InputEvent)` —— 指针事件（Moved/Pressed/Released/Left），控件命中测试由 App 负责；
  - `font_path()` —— 默认 `None` → 外壳按 KANESUMI_TEST_FONT → 系统字体查找。
- **App 接口测试**：`kanesumi-gallery/src/app.rs` 9 个交互测试覆盖 switch/tab/list/dialog/dropdown/selector 状态切换。

### 全局应用菜单（`kanesumi-appmenu` + harness 重导出，2026-08-12）

macOS 风格全局菜单接入（Ether TopBar / Plasma Global Menu）。App 声明式菜单树，一条调用自动完成
D-Bus 服务 + Wayland 绑定 + Registrar 注册 + 点击路由。参考实现：PezMax-One `src/app_menu`。

**独立 crate `kanesumi-appmenu`**（2026-08-12 自 harness 拆出，轻依赖：wayland-client/
wayland-backend/wayland-scanner/zbus5/async-io 全 Linux-gated），eframe 应用（不经 harness）
可直接依赖它（如 ether-librarian）：

- **`tree.rs`（跨平台）**：`MenuTree` / `MenuItem` / `ToggleType` 声明式树
  （item/separator/submenu/check/radio + `push` 链式 + `find`/`find_mut`/`walk`）。
- **`lib.rs`（跨平台）**：`AppMenuHandle`（`set_check` / `update_tree` 运行时更新，
  纯 mpsc）；`MENUBAR_OBJECT_PATH`（`/MenuBar`）。
- **`install.rs`（Linux）**：
  - `install(conn, surface, tree, app_id)` —— harness 应用（有自有 Wayland 连接）；
  - `install_from_foreign_handles(display, surface, tree, app_id)` —— **eframe 应用**，
    从 raw-window-handle 原始指针复用 winit 的 wl_display（参 PezMax-One `WaylandHandles`）。
  服务线程依次（1）zbus 阻塞连接挂 `/MenuBar` com.canonical.dbusmenu；（2）`request_name(app_id)`
  得服务名（占用则退回 unique_name）；（3）`com.canonical.AppMenu.Registrar`
  RegisterWindow(pid, `/MenuBar`) 兜底（合成器按 PID 匹配）；（4）`org_kde_kwin_appmenu`
  set_address（合成器原生路径）；（5）消费 `MenuUpdate` 更新勾选/结构 + 发 dbusmenu 信号。
- **`dbusmenu.rs`（Linux）**：zbus `#[interface]` 实现（GetLayout/Event/AboutToShow/
  信号）。序列化易错区已处理：`av` 子项与 `a{sv}` value 必须 variant 装箱、`_`→`__` 转义、
  radio/checkmark toggle-state。返回签名是协议强制的，`#[allow(clippy::type_complexity)]`。
- **`wayland.rs`（Linux）**：`wayland-scanner` 宏生成 KDE 私有协议客户端代码；
  在连接上开独立 event_queue 找 `org_kde_kwin_appmenu_manager` 全局 → create(surface) →
  set_address → flush → Box::leak 长驻（drop 会走 release() 清除关联）。

**harness 集成**（`kanesumi-harness` 仅 `pub use kanesumi_appmenu::*` 重导出）：
- App trait 钩子：`app_menu() -> Option<MenuTree>` / `set_appmenu_handle(AppMenuHandle)` /
  `on_menu_command(id)`；`platform.rs` 自动安装 + 每帧排干命令。
- Gallery 演示：File/View/Help 菜单 + View 勾选项运行时切换（`set_check` 发信号刷新）。

**D-Bus 依赖收敛**：本 crate 用 zbus 5（默认 features = async-io + blocking-api）+
`async-io`（block_on 驱动信号）。Ether 合成器 / settings 仍用 zbus 3，二者可共存。

### 与 Ether monorepo 的关系

- 本仓是 `ProjectArcturius/Ether`（monorepo）的 submodule，位于 `shared/kanesumi/`。
- **Ether monorepo 不把本仓 crate 列为 workspace 成员**（嵌套 workspace 冲突），
  而是用 `[patch."https://github.com/ProjectArcturius/Kanesumi"]` 将 git 依赖收敛到
  本地 submodule 检出版。参 `PLAN.md` §6.3 同款 sokuou 模式。
- 反向：`kanesumi-anim` 以 git 依赖声明 sokuou（本仓独立可建）；Ether monorepo 用
  `[patch."https://github.com/GuitaristRin/Sokuou"]` 收敛到 `shared/sokuou`。

### 扇区对称

本仓（Ether 扇区）与 `GuitaristRin/Kanesumi-sec-a`（Android 扇区）共用同名同层骨架，
后端不同。实现控件时对照 sec-a 的同名 `.kt` 与 `microsoft-ui-xaml` 的 C++ 实现。

## 设计铁律

1. **Kanesumi Design，而非 Fluent** —— 轻盈短促、0.25s、Quadratic/EaseOut。参考 UWP/Metro 时代；Win11 Fluent 仅作反面教材。
2. **Sokuou 动画唯一真源** —— 动画一律走 `kanesumi-anim`（消费 sokuou），禁止自造动画原语。
3. **状态驱动渲染** —— `state → progress → resolved spatial state → render`。
4. **动画只动视觉属性，不动布局** —— 位移/缩放/透明走快速路径，绝不触发 Measure/Arrange。
5. **无隐藏控件** —— reconciler 逻辑组件不产生额外原生控件。
6. **纯色无渐变** —— 直角或极轻微圆角、强调色 + 半透明面板。圆角以 `CornerRadius` 枚举
   （`Square`/`Slight`/`Capsule`）类型级约束，**禁止 Fluent 式大圆角（4px/8px）**；`Capsule` 仅限
   全圆角形态（Switch 轨道/Knob、ProgressBar 指示条）。

## 控件实现规范

**控件行为一律以 `docs/CONTROL_SPEC.md` 为准**（Metro/UWP 时代规格提取，2026-08-10）。
来源为 Ether monorepo `reference/` 快照（microsoft-ui-xaml v2.8.7 + WinUI-Gallery winui2），
提取后参考目录可安全丢弃。规格含：尺寸、视觉状态表、动画时长/缓动、主题色值、
OS 闭源缺失项的处理（标注「未在快照中」）。新增/修改控件前先读对应章节。

## 约定

- 注释使用中文 Vintage Words 风格，`参 <doc> §<chapter>` 交叉引用。参 Ether-main `CLAUDE.md`。
- 颜色为 `kanesumi_core::Color`（SRGBA f32）；尺寸为逻辑像素。
- 所有 crate 为纯 Rust、无 `unsafe`、跨平台（`#[cfg(target_os)]` 仅限渲染/输入外壳）。
- 新增控件前先读 `kanesumi-sec-a` 同名实现 + `microsoft-ui-xaml` 控件源码。

## 术语约定（Kanesumi Design 定名）

本仓实现的设计语言为 **Kanesumi Design**（正典：Ether monorepo 仓根 `KANESUMI_DESIGN.md`，取代「Metro Design / 改良 Metro」）。**`Metro*` 标识符**（MetroText/MetroTheme/MetroIndication/MetroShell…）是代码/组件名，**保留不重命名**。写文档/注释：用「Kanesumi 风格 / Kanesumi 铁律 / Kanesumi 控件库」，不写「Metro 风格 / Metro 铁律」；缓动族称「UWP 缓动」（`UwpEasing`）；「Metro 时代」仅作历史指称。完整映射见 `KANESUMI_DESIGN.md` §Ⅴ。
