# Kanesumi（矩隅）—— 以直角丈量边缘

**Ether 扇区·主仓（mainline）**：用 Rust 编写的原生应用 Runtime，统一 Metro 设计语言 + Sokuou（即応エンジン）动画。

> "以直角丈量边缘"是设计语言级陈述，与平台无关。各平台为**扇区 Sector**，共享同一圆心（设计语言 + Sokuou 动画）。

## 扇区

| 扇区 | 仓库 | 平台 / 技术 |
|---|---|---|
| **Ether（本仓）** | `ProjectArcturius/Kanesumi` | Rust / Wayland 合成器式（保留视觉树 + 合成时钟 + 零重绘） |
| Sec-A（Android） | `GuitaristRin/Kanesumi-sec-a` | Kotlin / Compose（`graphicsLayer` 零重组） |

## Workspace

| Crate | 职责 | 状态 |
|---|---|---|
| `kanesumi-core` | 设计 tokens / 主题 / MetroText / 交互指示 / 几何原语 | ✅ 骨架（Phase 2） |
| `kanesumi-anim` | 动画层，消费 Sokuou（即応エンジン） | ✅ 骨架（Phase 2） |
| `kanesumi-canvas` | 2D 图形：Scene 渲染命令 + TextEngine 排版（对应 Win2D） | ✅ 拆分完成（2026-08-10） |
| `kanesumi-harness` | 应用壳：App trait / ETHER_ROLE 角色解析 / Scene 场景 + Linux Wayland+wgpu 外壳 | 🆕 核心完成，外壳 Phase 3 |
| `kanesumi-structure` | 页面结构（Navigation 状态机 / ShellLayout / AppBar / Scaffold） | ✅ 导航+壳布局（2026-08-10） |
| `kanesumi-controls` | Metro 标准控件库（MetroSurface / Button / List…） | ✅ 12 控件全量 + 输入层 |
| `kanesumi-gallery` | Gallery 应用 —— 三层测试阶梯的 daily driver | ✅ 全交互 + 滚轮/弹层方向 |

## 设计原则

- **Metro，而非 Fluent**：轻盈、短促、0.25s、60Hz 友好、Quadratic/EaseOut。WinUI 3 / Win11 的 Fluent 动画仅作反面教材。
- **Sokuou 是动画唯一真源**：`kanesumi-anim` 直接消费 `sokuou`（git 依赖，Ether monorepo 侧以 `[patch]` 收敛到本地 submodule）。
- **状态驱动渲染**：`state → progress → resolved spatial state → render`，不做 timeline 播放。
- **无隐藏控件**：reconciler 逻辑组件不产生额外原生控件。
- **纯色无渐变**：直角或极轻微圆角、强调色 + 半透明面板、内容优先。
- **GPU 零重绘铁律**：动画只动视觉属性（位移/缩放/透明），不动布局；静态内容保留为纹理。

## 构建

```bash
cargo check
cargo test
cargo clippy
cargo fmt
```

设计蓝图见 Ether monorepo `PLAN.md`（§4 架构、§5 实施路线、§6 命名规划）。
