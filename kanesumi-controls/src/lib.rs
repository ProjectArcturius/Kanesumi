// Kanesumi（矩隅）· Metro 标准控件库（骨架）
//
// 对应 Kanesumi-sec-a 的 `:kanesumi-controls`。Phase 3 填充控件实现，参 PLAN.md §5。
// 此阶段仅固定控件形态数据模型（直角 / 极轻微圆角 / 无渐变纯色），供 Gallery 页树引用。

use kanesumi_core::{Color, MetroTheme};

/// 控件形态 tokens —— 参 PLAN.md §4-5（Metro 形态：直角/极轻微圆角、无渐变纯色、内容优先）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlShape {
    pub corner_radius: f32,
    pub background: Color,
}

impl ControlShape {
    pub const fn new(corner_radius: f32, background: Color) -> Self {
        Self {
            corner_radius,
            background,
        }
    }
}

/// MetroSurface —— 面板基底。骨架占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroSurface {
    pub shape: ControlShape,
}

impl MetroSurface {
    pub const fn new(shape: ControlShape) -> Self {
        Self { shape }
    }
}

/// MetroButton —— 命令按钮。骨架占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroButton {
    pub label: &'static str,
    pub background: Color,
    pub foreground: Color,
}

impl MetroButton {
    pub const fn new(label: &'static str, background: Color, foreground: Color) -> Self {
        Self {
            label,
            background,
            foreground,
        }
    }
}

/// MetroList —— 列表。骨架占位（行控件 Phase 3）。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroList {
    pub rows: Vec<&'static str>,
}

impl MetroList {
    pub fn new(rows: Vec<&'static str>) -> Self {
        Self { rows }
    }
}

/// Phase 3 待实现的控件清单（对照 Kanesumi-sec-a 同名控件 + WinUI-Gallery）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlKind {
    MetroSurface,
    MetroButton,
    MetroIconButton,
    MetroListRow,
    MetroSwitch,
    MetroProgressIndicator,
    MetroTabRow,
    MetroDialog,
    MetroDropdownMenu,
    MetroSelectorFlyout,
    MetroDivider,
    MetroBottomSheet,
}

impl ControlKind {
    /// 全部计划控件。
    pub const ALL: [ControlKind; 12] = [
        ControlKind::MetroSurface,
        ControlKind::MetroButton,
        ControlKind::MetroIconButton,
        ControlKind::MetroListRow,
        ControlKind::MetroSwitch,
        ControlKind::MetroProgressIndicator,
        ControlKind::MetroTabRow,
        ControlKind::MetroDialog,
        ControlKind::MetroDropdownMenu,
        ControlKind::MetroSelectorFlyout,
        ControlKind::MetroDivider,
        ControlKind::MetroBottomSheet,
    ];
}

impl From<MetroTheme> for ControlShape {
    fn from(t: MetroTheme) -> Self {
        Self {
            corner_radius: t.tokens.corner_radius,
            background: t.colors.surface,
        }
    }
}
