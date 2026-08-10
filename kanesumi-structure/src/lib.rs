// Kanesumi（矩隅）· 页面结构（骨架）
//
// 对应 Kanesumi-sec-a 的 `:kanesumi-structure`（MetroShell.kt / MetroAppBar.kt /
// MetroDetailScaffold.kt）。Phase 3 填充控件实现，参 PLAN.md §5。
// 此阶段仅固定公共数据模型，供 Gallery 页树引用。

use kanesumi_core::{Color, MetroTheme};

/// 应用主壳。骨架占位 —— 层级 / 导航模型 Phase 3 定稿。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroShell {
    pub theme: MetroTheme,
}

impl MetroShell {
    pub fn new(theme: MetroTheme) -> Self {
        Self { theme }
    }
}

/// 顶部命令栏（AppBar）。骨架占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroAppBar {
    pub background: Color,
    pub height: f32,
}

impl MetroAppBar {
    pub const fn new(background: Color, height: f32) -> Self {
        Self { background, height }
    }
}

/// 页面脚手架（Scaffold）。骨架占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroScaffold {
    pub background: Color,
    pub padding: f32,
}

impl MetroScaffold {
    pub const fn new(background: Color, padding: f32) -> Self {
        Self {
            background,
            padding,
        }
    }
}
