// interaction.rs —— 交互正典的数值单一来源（Ether `docs/INTERACTION_CANON.md`，2026-10-02）。
//
// 正典 §Ⅰ：交互数值集中为本模块的具名常量与 `InteractionSettings`；控件、harness、应用
// **只引用**，不得各写各的。用户可调项（双击时长 / 滚轮行数 / 提示延迟）由 harness 从
// `~/.config/ether/input.toml` 覆盖，见 [`InteractionSettings`]。
//
// 所有长度为逻辑像素（2× 屏由渲染层翻倍，参 `DECISIONS` §J）。

use crate::accent::ColorScheme;
use crate::color::Color;

// ── Ⅱ 指针 ──────────────────────────────────────────────────────────────────

/// 双击时长：两次按下间隔 ≤ 500 ms（正典 §Ⅱ，Windows `GetDoubleClickTime` 系统实测）。
pub const DOUBLE_CLICK_MS: u32 = 500;
/// 双击容差框 4 × 4 px 的半边长（±2 px；正典 §Ⅱ，`SM_CXDOUBLECLK`）。
pub const DOUBLE_CLICK_TOLERANCE_PX: f32 = 2.0;
/// 拖拽起始阈值：按下后移动超过 4 px 才进入拖拽（正典 §Ⅱ，`SM_CXDRAG`）。
pub const DRAG_THRESHOLD_PX: f32 = 4.0;
/// 触控拖拽阈值 10 px（正典 §Ⅱ，标注「待核」，登记于 `docs/CANON_VS_TEMPORARY.md`）。
pub const DRAG_THRESHOLD_TOUCH_PX: f32 = 10.0;

// ── Ⅲ 悬停与提示 ────────────────────────────────────────────────────────────

/// 悬停触发的**行为**延迟 400 ms，容差 4 × 4 px（正典 §Ⅲ，`SPI_GETMOUSEHOVERTIME`）。
/// 注意：悬停**视觉**立即显示（0 ms），此值只用于「指针静止满时长才触发」的行为。
pub const HOVER_TRIGGER_MS: u32 = 400;
/// 悬停容差框半边长（±2 px；正典 §Ⅲ）。
pub const HOVER_TOLERANCE_PX: f32 = 2.0;
/// 工具提示初始延迟 500 ms（正典 §Ⅲ）。
pub const TOOLTIP_DELAY_MS: u64 = 500;
/// 工具提示再现延迟 100 ms（已有提示时移到相邻目标；正典 §Ⅲ）。
pub const TOOLTIP_RESHOW_MS: u64 = 100;
/// 工具提示消失 5 s（正典 §Ⅲ，`SPI_GETMESSAGEDURATION`）。
pub const TOOLTIP_HIDE_MS: u64 = 5000;
/// 子菜单悬停展开延迟 400 ms（正典 §Ⅲ，`SPI_GETMENUSHOWDELAY`）。
pub const SUBMENU_DELAY_MS: u64 = 400;
/// 子菜单关闭宽限 400 ms（指针朝子菜单方向移动时不关闭；正典 §Ⅲ）。
pub const SUBMENU_GRACE_MS: u64 = 400;

// ── Ⅳ 键盘与焦点 ────────────────────────────────────────────────────────────

/// 按键重复延迟 500 ms、速率约 30 次/秒（正典 §Ⅳ，系统实测）。
pub const KEY_REPEAT_DELAY_MS: u32 = 500;
/// 按键重复速率（次/秒；正典 §Ⅳ）。
pub const KEY_REPEAT_RATE_HZ: u32 = 30;
/// 文本光标闪烁周期 530 ms（正典 §Ⅳ，系统实测）。
pub const CARET_BLINK_PERIOD_MS: u32 = 530;
/// 文本光标宽度 1 px（正典 §Ⅳ；2× 下为 2 物理像素）。
pub const CARET_WIDTH_PX: f32 = 1.0;
/// 焦点框外圈 2 px（正典 §Ⅳ，`SPI_GETFOCUSBORDERWIDTH`；框架统一绘制）。
pub const FOCUS_OUTER_PX: f32 = 2.0;
/// 焦点框内圈 1 px 对比色（正典 §Ⅳ，UWP 双层焦点视觉；取值登记为临时项 T23）。
pub const FOCUS_INNER_PX: f32 = 1.0;

// ── Ⅴ 滚动 ──────────────────────────────────────────────────────────────────

/// 滚轮一格 = 3 行（正典 §Ⅴ，`SPI_GETWHEELSCROLLLINES` = 3）。
pub const WHEEL_LINES: u32 = 3;
/// 逻辑行高（正典 §Ⅴ：48 px / 3 行）。
pub const WHEEL_LINE_HEIGHT_PX: f32 = 16.0;
/// 滚轮一格像素步长 = 3 行 × 16 = 48 px（正典 §Ⅴ）。
pub const WHEEL_STEP_PX: f32 = 48.0;
/// 横向滚轮 / Shift+滚轮一格 = 3 字符宽 = 48 px（正典 §Ⅴ，`SPI_GETWHEELSCROLLCHARS`）。
pub const WHEEL_STEP_X_PX: f32 = 48.0;

/// 焦点框内圈对比色：深色主题用黑、浅色主题用白（正典 §Ⅳ 双层焦点视觉）。
///
/// ⚠ 正典未给一手源，取值登记为临时项 T23（`docs/CANON_VS_TEMPORARY.md`）。备选方案是
/// 取 `on_accent` 的反色；先随主题翻转，等真机核对再定。
pub fn focus_inner_color(scheme: ColorScheme) -> Color {
    match scheme {
        ColorScheme::Dark => Color::BLACK,
        ColorScheme::Light => Color::WHITE,
    }
}

/// 交互数值的用户可调覆盖（正典 §Ⅰ.2）。`Default` = 正典值。
///
/// 由 harness 读取 `~/.config/ether/input.toml` 覆盖；控件层消费默认值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InteractionSettings {
    /// 双击时长（ms）。
    pub double_click_ms: u32,
    /// 滚轮一格行数。
    pub wheel_lines: u32,
    /// 工具提示初始延迟（ms）。
    pub tooltip_delay_ms: u64,
}

impl Default for InteractionSettings {
    fn default() -> Self {
        Self {
            double_click_ms: DOUBLE_CLICK_MS,
            wheel_lines: WHEEL_LINES,
            tooltip_delay_ms: TOOLTIP_DELAY_MS,
        }
    }
}

impl InteractionSettings {
    /// 滚轮一格像素步长 = 行数 × 行高（正典 §Ⅴ；默认 3 × 16 = 48）。
    pub fn wheel_step_px(&self) -> f32 {
        self.wheel_lines as f32 * WHEEL_LINE_HEIGHT_PX
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_canon() {
        let s = InteractionSettings::default();
        assert_eq!(s.double_click_ms, 500);
        assert_eq!(s.wheel_lines, 3);
        assert_eq!(s.tooltip_delay_ms, 500);
        assert_eq!(s.wheel_step_px(), 48.0);
    }

    /// 常量与正典逐项一致：防止有人只改其中一处。
    #[test]
    fn canon_values_are_pinned() {
        assert_eq!(DOUBLE_CLICK_MS, 500);
        assert_eq!(DOUBLE_CLICK_TOLERANCE_PX, 2.0, "4×4 框 = ±2");
        assert_eq!(DRAG_THRESHOLD_PX, 4.0);
        assert_eq!(HOVER_TRIGGER_MS, 400);
        assert_eq!(HOVER_TOLERANCE_PX, 2.0);
        assert_eq!(TOOLTIP_DELAY_MS, 500);
        assert_eq!(TOOLTIP_RESHOW_MS, 100);
        assert_eq!(TOOLTIP_HIDE_MS, 5000);
        assert_eq!(SUBMENU_DELAY_MS, 400);
        assert_eq!(CARET_BLINK_PERIOD_MS, 530);
        assert_eq!(CARET_WIDTH_PX, 1.0);
        assert_eq!(FOCUS_OUTER_PX, 2.0);
        assert_eq!(FOCUS_INNER_PX, 1.0);
        assert_eq!(WHEEL_STEP_PX, WHEEL_LINES as f32 * WHEEL_LINE_HEIGHT_PX);
    }

    #[test]
    fn wheel_step_follows_lines_override() {
        let s = InteractionSettings {
            wheel_lines: 5,
            ..Default::default()
        };
        assert_eq!(s.wheel_step_px(), 80.0);
    }

    #[test]
    fn focus_inner_color_flips_with_scheme() {
        assert_eq!(focus_inner_color(ColorScheme::Dark), Color::BLACK);
        assert_eq!(focus_inner_color(ColorScheme::Light), Color::WHITE);
    }
}
