# CLAUDE.md — Kanesumi（Ether 扇区）

## 构建与测试

所有命令从本仓根目录执行：

```bash
cargo check          # 检查所有 crate
cargo test           # 单元测试（Phase 2 起每 crate 自带）
cargo clippy
cargo fmt
```

## 架构

本仓是 **Ether 扇区·主仓（mainline）**，Rust workspace，5 个 crate：

```
kanesumi-core       (无依赖 — 设计 tokens / 主题 / MetroText / 交互指示)
    ├── kanesumi-anim      (dep: sokuou(git) — Metro 动画预设，消费 Sokuou)
    ├── kanesumi-structure (dep: kanesumi-core — 页面结构)
    ├── kanesumi-controls  (dep: kanesumi-core, kanesumi-anim — Metro 控件库)
    └── kanesumi-gallery   (dep: core+anim+structure+controls — Gallery 应用)
```

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
6. **纯色无渐变** —— 直角或极轻微圆角、强调色 + 半透明面板。

## 约定

- 注释使用中文 Vintage Words 风格，`参 <doc> §<chapter>` 交叉引用。参 Ether-main `CLAUDE.md`。
- 颜色为 `kanesumi_core::Color`（SRGBA f32）；尺寸为逻辑像素。
- 所有 crate 为纯 Rust、无 `unsafe`、跨平台（`#[cfg(target_os)]` 仅限渲染/输入外壳）。
- 新增控件前先读 `kanesumi-sec-a` 同名实现 + `microsoft-ui-xaml` 控件源码。
