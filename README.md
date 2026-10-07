# Kanesumi（矩隅）—— 以直角丈量边缘

**Ether 扇区·主仓（mainline）**：Rust 原生应用 Runtime，统一 Kanesumi Design 设计语言 + Sokuou（即応エンジン）动画。

> 「以直角丈量边缘」是设计语言级陈述，与平台无关。各平台为**扇区 Sector**，共享同一圆心（设计语言 + Sokuou 动画）。

## 扇区

| 扇区 | 仓库 | 平台 |
|---|---|---|
| **Ether（本仓）** | `ProjectArcturius/Kanesumi` | Rust / Wayland（视觉树 + 合成时钟 + 零重绘） |
| Sec-A（Android） | `GuitaristRin/Kanesumi-sec-a` | Kotlin / Compose（`graphicsLayer` 零重组） |

## Workspace

| Crate | 职责 |
|---|---|
| `kanesumi-core` | 设计 tokens / 主题 / MetroText / 交互正典常量 / 几何原语 |
| `kanesumi-anim` | 动画层，消费 Sokuou |
| `kanesumi-element` | **元素树**（TreeApp / TreeHost）：声明式界面 + 合成时钟 + 按损伤重绘 |
| `kanesumi-canvas` | 2D 图形：Scene 渲染命令 + TextEngine 排版 + CanvasV2 批渲染 |
| `kanesumi-structure` | 页面结构（Navigation / ShellLayout / AppBar / Scaffold） |
| `kanesumi-controls` | 标准控件库（Button / List / 候选窗 / 工具提示…）+ 输入层 |
| `kanesumi-harness` | 应用壳：App trait / Scene / Linux Wayland+wgpu 外壳 / 快照 |
| `kanesumi-appmenu` | 全局菜单（dbusmenu 客户端） |
| `kanesumi-gallery` | Gallery 应用 —— 三层测试阶梯的 daily driver |

## 设计原则

- **正典**：Kanesumi Design（`KANESUMI_DESIGN.md`，Ether monorepo 仓根）——取代历史称谓「Metro Design」。
- **轻盈短促**：0.25s、60Hz 友好、Quadratic / EaseOut；参考 UWP 时代，Win11 Fluent 动画仅作反面教材。
- **Sokuou 是动画唯一真源**：`kanesumi-anim` 直接消费 `sokuou`（Ether monorepo 侧以 `[patch]` 收敛到本地 submodule）。
- **状态驱动渲染**：`state → progress → resolved spatial state → render`，不做 timeline 播放。
- **GPU 零重绘铁律**：动画只动视觉属性（位移 / 缩放 / 透明），不动布局；静态内容保留为纹理。
- **纯色无渐变**：直角或极轻微圆角、强调色 + 半透明面板、内容优先。

## 构建

```bash
cargo check
cargo test
cargo clippy
cargo fmt
```

界面开发看 `docs/DEV_GUIDE.md`（元素树写法）；临时值登记看 `docs/CANON_VS_TEMPORARY.md`。
设计蓝图见 Ether monorepo `PLAN.md`。
