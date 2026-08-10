# CLAUDE.md — Kanesumi（Ether 扇区）

## 构建与测试

所有命令从本仓根目录执行：

```bash
cargo check          # 检查所有 crate
cargo test           # 单元测试（当前 101 个）
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

本仓是 **Ether 扇区·主仓（mainline）**，Rust workspace，6 个 crate：

```
kanesumi-core       (无依赖 — 设计 tokens / 主题 / MetroText / 交互指示 / 几何原语)
    ├── kanesumi-anim      (dep: sokuou(git) — Metro 动画预设，消费 Sokuou)
    ├── kanesumi-structure (dep: kanesumi-core — 导航状态机 + 壳布局)
    ├── kanesumi-controls  (dep: kanesumi-core, kanesumi-anim — Metro 控件库)
    ├── kanesumi-harness   (dep: core+anim+structure+controls — 应用壳：App trait / 角色解析 / Scene /
    │                        Linux Wayland+wgpu 外壳：platform.rs + render.rs)
    └── kanesumi-gallery   (dep: core+anim+structure+controls+harness — Gallery 应用，daily driver)
```

### 页面结构（kanesumi-structure，2026-08-10 填充）

- **`navigation.rs`**：`Navigation<PageId>` —— 对应 UWP `Frame.Navigate` 的状态驱动实现（无保留视觉树）。
  页栈 + 过渡进度（`navigate_to` / `go_back` / `can_go_back` / `is_transitioning` / `leaving_page`）。
  过渡进度可由应用层 Sokuou 动画驱动（`set_transition_progress`），本层不依赖 kanesumi-anim。
- **`layout.rs`**：`ShellLayout` —— `MetroShell::layout(window)` 一次划分 AppBar / 内容区（可选左侧导航栏）。
- **`lib.rs`**：`MetroShell<PageId>`（主题 + 导航 + AppBar 宿主）、`MetroAppBar`（标题 + 高度）、
  `MetroScaffold`（内容容器 + 内边距）。`render(engine, window)` 渲染背景 + AppBar，返回内容矩形。

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

1. **Metro 而非 Fluent** —— 轻盈短促、0.25s、Quadratic/EaseOut。Win11 Fluent 仅作反面教材。
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
