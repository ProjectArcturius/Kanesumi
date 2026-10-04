// Tooltip —— 工具提示气泡（框架 chrome，非应用控件）。参 CONTROL_SPEC «ToolTip»。
//
// 由框架在 `Tree` 的提示计时到期时自动挂到覆盖层，应用只经 `Tree::set_tooltip` 声明文字。
// 视觉取自 UWP `ToolTip` 样式（`A2:13245-13261`）：12px 正文、内边距 `8,5,8,7`、最大宽 320、
// 1px 描边、直角/轻圆角。颜色复用当前主题令牌（core 不属本任务改动范围）：
// 底 `surface_variant` ≈ 暗 `#2E2E2E` / 亮 `#F0F0F0`，字 `on_surface`，描边 `divider`。
//
// 提示**不吃输入、不抢焦点**：`hit_test` 恒 false，框架对该弹层另设 `PopupSpec::passthrough`。

use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::{Size, TextStyle};

use crate::widget::{AccessInfo, AccessRole, MeasureCtx, PaintCtx, Widget};

/// 气泡最大宽度（UWP `MaxWidth="320"`，`A2:13261`）。
pub const MAX_WIDTH: f32 = 320.0;
/// 气泡与锚点之间的间隙。
pub const TOOLTIP_GAP: f32 = 4.0;
/// 内边距（UWP `ToolTipBorderThemePadding = 8,5,8,7`，`A1:726`）。
const PAD_L: f32 = 8.0;
const PAD_T: f32 = 5.0;
const PAD_R: f32 = 8.0;
const PAD_B: f32 = 7.0;
/// 气泡正文样式：12px / 行高 16（UWP `ToolTipContentThemeFontSize = 12`，`A1:101`），
/// 直接套正文小号令牌（参 `docs/DECISIONS_2026-10-04.md` §G-67）。
pub fn tooltip_style() -> TextStyle {
    kanesumi_core::MetroTypography::metro().body_small
}

/// 提示气泡（覆盖层节点）。参本文件头。
#[derive(Debug, Clone, PartialEq)]
pub struct Tooltip {
    text: String,
}

impl Tooltip {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

impl Widget for Tooltip {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let style = tooltip_style();
        // 文本换行宽 = min(320 − 内边距, 可用宽 − 内边距)。
        let wrap = (available.width - PAD_L - PAD_R).clamp(1.0, MAX_WIDTH - PAD_L - PAD_R);
        let lines = ctx.engine().layout_with_spacing_weighted(
            &self.text,
            style.size,
            style.letter_spacing_em,
            wrap,
            style.weight,
        );
        let text_w = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        let text_w = text_w.min(wrap);
        Size::new(
            text_w + PAD_L + PAD_R,
            lines.len().max(1) as f32 * style.line_height + PAD_T + PAD_B,
        )
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        let rect = ctx.rect();
        let corner = theme.tokens.corner_radius;
        scene.fill_rounded_rect(theme.colors.surface_variant, rect, corner);
        scene.stroke_rounded_rect(theme.colors.divider, rect, 1.0, corner);
        let inner = kanesumi_core::Rect::new(
            rect.origin.x + PAD_L,
            rect.origin.y + PAD_T,
            (rect.size.width - PAD_L - PAD_R).max(0.0),
            (rect.size.height - PAD_T - PAD_B).max(0.0),
        );
        scene.text_with_options(
            self.text.clone(),
            inner,
            theme.colors.on_surface,
            tooltip_style(),
            TextAlign::Left,
            true,
            None,
            TextOverflow::Clip,
        );
    }

    /// 提示不参与命中（`PopupSpec::passthrough` 之外的第二道保险）。
    fn hit_test(&self, _rect: kanesumi_core::Rect, _pos: kanesumi_core::Point) -> bool {
        false
    }

    fn accessibility(&self) -> Option<AccessInfo> {
        Some(AccessInfo {
            role: AccessRole::Label,
            name: self.text.clone(),
            value: None,
            checked: None,
        })
    }
}
