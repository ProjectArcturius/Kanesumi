// MetroCheckBox —— 三态复选框。参 CONTROL_SPEC §36（CheckBox 参考，闭源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/CheckBox_themeresources_v1.xaml`：
// - 勾选框 20×20、列 0 宽 20；Padding `8,5,0,0`；MinWidth 120 / MinHeight 32；
// - 勾选 = 强调色实心 + 白色 ✓（glyph E73E）；未选 = 透明 + `on_surface_variant` 1px 描边；
// - 不确定态 = 强调色实心 + 白色 —（glyph E73C）；
// - 悬停描边转 `on_surface`；按下填充转 `on_surface_variant`；禁用降透明度；
// - 颜色硬切换（DiscreteObjectKeyFrame 0），无过渡动画。
//
// Kanesumi 适配：勾与减走 L0 图形层（`kanesumi_canvas::icons`），不依赖字体里有没有
// `✓` / `—`（参 V7 与资产仓 docs/GLYPH_ASSETS.md）；强调色前景恒白；禁用降 38%。

use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::sizes::ICON_12;
use kanesumi_core::{Color, CornerRadius, MetroTheme, Point, Rect};

use crate::state::ControlState;

/// 勾选框边长（20）。取值入尺寸表 `kanesumi_core::sizes::ICON_20`（N18 图标尺寸档）。
pub const CHECKBOX_SIZE: f32 = kanesumi_core::sizes::ICON_20;
/// 描边厚度（CheckBoxBorderThemeThickness 1）。
pub const CHECKBOX_STROKE: f32 = 1.0;
/// 列 0 宽（20）+ 内容 Padding 左 8。
pub const CHECKBOX_BOX_GAP: f32 = 8.0;
/// 控件 MinHeight（32）。
pub const CHECKBOX_MIN_HEIGHT: f32 = 32.0;
/// 控件 MinWidth（120）。
pub const CHECKBOX_MIN_WIDTH: f32 = 120.0;

/// 三态值。None = 不确定（Indeterminate）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Checked,
    Unchecked,
    Indeterminate,
}

/// MetroCheckBox —— 三态复选框。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroCheckBox {
    pub label: String,
    pub state: CheckState,
    /// 交互状态（Hovered/Pressed/Disabled/Focused）。
    pub interact: ControlState,
    /// 是否允许点击进入不确定态（UWP 默认点击循环 Checked↔Unchecked，不含不确定）。
    pub allow_indeterminate: bool,
}

impl Default for MetroCheckBox {
    fn default() -> Self {
        Self {
            label: String::new(),
            state: CheckState::Unchecked,
            interact: ControlState::Normal,
            allow_indeterminate: false,
        }
    }
}

impl MetroCheckBox {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            ..Self::default()
        }
    }

    /// 三态切换（UWP 语义：默认两态循环；`allow_indeterminate` 时三态循环）。
    /// 返回新的状态。
    pub fn toggle(&mut self) -> CheckState {
        self.state = match (self.state, self.allow_indeterminate) {
            (CheckState::Unchecked, _) => CheckState::Checked,
            (CheckState::Checked, true) => CheckState::Indeterminate,
            (CheckState::Checked, false) => CheckState::Unchecked,
            (CheckState::Indeterminate, _) => CheckState::Unchecked,
        };
        self.state
    }

    /// builder：初始勾选。
    pub fn with_checked(mut self, checked: bool) -> Self {
        self.state = if checked {
            CheckState::Checked
        } else {
            CheckState::Unchecked
        };
        self
    }

    /// 编程式设置状态。
    pub fn set_state(&mut self, s: CheckState) {
        self.state = s;
    }

    /// 勾选框矩形（相对宿主 rect，垂直居中）。
    pub fn box_rect(&self, rect: Rect) -> Rect {
        let cy = rect.origin.y + (rect.size.height - CHECKBOX_SIZE) / 2.0;
        Rect::new(rect.origin.x, cy, CHECKBOX_SIZE, CHECKBOX_SIZE)
    }

    /// 命中：整行（含标签）。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 渲染。顺序：勾选框（描边/填充/字形）→ 标签。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        self.render_chrome(theme, engine, rect, scene);

        // 标签（Padding 左 8 + 20 列）
        if !self.label.is_empty() {
            let colors = &theme.colors;
            let style = theme.typography.body;
            let alpha = if self.interact == ControlState::Disabled {
                theme.indication.disabled_opacity
            } else {
                1.0
            };
            let fg = colors.on_surface.with_alpha(colors.on_surface.a * alpha);
            let label_x = rect.origin.x + CHECKBOX_SIZE + CHECKBOX_BOX_GAP;
            let label_w = (rect.right() - label_x).max(0.0);
            let label_rect = Rect::new(
                label_x,
                rect.origin.y + (rect.size.height - style.line_height) / 2.0,
                label_w,
                style.line_height,
            );
            scene.text(self.label.clone(), label_rect, fg, style, TextAlign::Left);
        }
    }

    /// 只画勾选框本体（底 / 描边 / 勾选字形）。
    ///
    /// 内容槽模式复用这里：控件里放了子节点时，画完选框就把画面让给内容，
    /// **不画自己的标签** —— 与按钮同一套规则（参 `docs/DEV_GUIDE.md` §2.6）。
    pub fn render_chrome(
        &self,
        theme: &MetroTheme,
        engine: &TextEngine,
        rect: Rect,
        scene: &mut Scene,
    ) {
        let colors = &theme.colors;
        let disabled = self.interact == ControlState::Disabled;
        let alpha = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };
        let _ = engine;

        let box_rect = self.box_rect(rect);
        let hovered = self.interact == ControlState::Hovered;
        let pressed = self.interact == ControlState::Pressed;
        let checked = self.state != CheckState::Unchecked;

        // 勾选框底（强调色实心）或透明
        let (fill, stroke) = if checked {
            let f = if pressed {
                // CheckedPressed = SystemAccentColorDark1
                colors.primary.lerp(Color::BLACK, 0.18)
            } else if hovered {
                // CheckedPointerOver = SystemAccentColorLight1
                colors.primary.lerp(Color::WHITE, 0.18)
            } else {
                colors.primary
            };
            (f.with_alpha(f.a * alpha), Color::TRANSPARENT)
        } else if pressed {
            // UncheckedPressed = BaseMediumLow（中灰实心），无描边
            (
                colors
                    .on_surface_variant
                    .with_alpha(theme.indication.base_medium_low * alpha),
                Color::TRANSPARENT,
            )
        } else {
            // Unchecked：透明底 + 描边（悬停转 on_surface）
            let s = if hovered {
                colors.on_surface
            } else {
                // UWP `CheckBoxCheckBackgroundStrokeUnchecked` = BaseMediumHigh 80%
                // （亮 #CC000000）；旧取 on_surface_variant 在浅色下偏淡。
                colors.control_stroke_strong
            };
            (
                Color::TRANSPARENT,
                s.with_alpha(s.a * alpha),
            )
        };

        if fill.a > 0.0 {
            scene.fill_rounded_rect(fill, box_rect, CornerRadius::Square);
        }
        if stroke.a > 0.0 {
            scene.stroke_rounded_rect(stroke, box_rect, CHECKBOX_STROKE, CornerRadius::Square);
        }

        // 字形：勾 / 减（白）—— 走 L0 图形层，不再赌字体里有 ✓ / —。
        // 12 pt 落在 20 pt 的框里（内缩 4），与 UWP 模板的视觉比重一致。
        if checked {
            let glyph = match self.state {
                CheckState::Indeterminate => GlyphName::Minus,
                _ => GlyphName::Check,
            };
            let fg = Color::WHITE.with_alpha(alpha);
            let inset = (box_rect.size.width - ICON_12) / 2.0;
            icons::draw(
                scene,
                glyph,
                Rect::new(
                    box_rect.origin.x + inset,
                    box_rect.origin.y + (box_rect.size.height - ICON_12) / 2.0,
                    ICON_12,
                    ICON_12,
                ),
                fg,
            );
        }

    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E2；模板同 button.rs）──────────────────

/// 元素树动作：勾选状态被用户切换（点击 / Space）。携带切换后的新状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckToggled(pub CheckState);

impl kanesumi_element::Widget for MetroCheckBox {
    /// 固有尺寸：勾选框 20 + 间距 8 + 标签；不小于 MinWidth 120 × MinHeight 32（CONTROL_SPEC §36）。
    ///
    /// 有子节点时按**内容槽**量测（子节点 = 标签位置的任意元素），标签不再计入尺寸。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let style = ctx.theme().typography.body;
        let kids = ctx.children();
        if !kids.is_empty() {
            let avail = kanesumi_core::Size::new(
                (available.width - CHECKBOX_SIZE - CHECKBOX_BOX_GAP).max(0.0),
                available.height,
            );
            let mut content = kanesumi_core::Size::ZERO;
            for c in kids {
                let d = ctx.measure_child(c, avail);
                content = kanesumi_core::Size::new(
                    content.width.max(d.width),
                    content.height.max(d.height),
                );
            }
            return kanesumi_core::Size::new(
                (CHECKBOX_SIZE + CHECKBOX_BOX_GAP + content.width).max(CHECKBOX_MIN_WIDTH),
                content.height.max(style.line_height).max(CHECKBOX_MIN_HEIGHT),
            );
        }
        let label_w = if self.label.is_empty() {
            0.0
        } else {
            CHECKBOX_BOX_GAP + ctx.engine().measure(&self.label, style.size)
        };
        kanesumi_core::Size::new(
            (CHECKBOX_SIZE + label_w).max(CHECKBOX_MIN_WIDTH),
            style.line_height.max(CHECKBOX_MIN_HEIGHT),
        )
    }

    /// 内容排进勾选框右侧的区域（与标签同一列起点）。
    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: Rect) {
        let inner = Rect::new(
            rect.origin.x + CHECKBOX_SIZE + CHECKBOX_BOX_GAP,
            rect.origin.y,
            (rect.size.width - CHECKBOX_SIZE - CHECKBOX_BOX_GAP).max(0.0),
            rect.size.height,
        );
        for c in ctx.children() {
            ctx.arrange_child(c, inner);
        }
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        let saved = self.interact;
        self.interact = crate::state::control_state(ctx.state());
        if ctx.children().is_empty() {
            self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        } else {
            // 内容槽模式：只画勾选框，标签位置留给子节点。
            self.render_chrome(ctx.theme(), ctx.engine(), ctx.rect(), scene);
        }
        self.interact = saved;
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key};
        if matches!(
            event,
            Event::Click
                | Event::KeyDown {
                    key: Key::Char(' '),
                    ..
                }
        ) {
            let s = self.toggle();
            ctx.emit(CheckToggled(s));
            ctx.invalidate_paint();
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::CheckBox,
            name: self.label.clone(),
            value: None,
            checked: match self.state {
                CheckState::Checked => Some(true),
                CheckState::Unchecked => Some(false),
                CheckState::Indeterminate => None,
            },
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(cb: MetroCheckBox) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            cb,
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        (h, id)
    }

    #[test]
    fn click_toggles_and_reports_new_state() {
        let (mut h, id) = harness(MetroCheckBox::new("启用蓝牙"));
        h.click(id);
        assert_eq!(h.take::<CheckToggled>(), vec![(id, CheckToggled(CheckState::Checked))]);
        assert_eq!(h.tree.get::<MetroCheckBox>(id).unwrap().state, CheckState::Checked);
        h.tab();
        h.key(Key::Char(' '));
        assert_eq!(h.take::<CheckToggled>(), vec![(id, CheckToggled(CheckState::Unchecked))]);
    }

    #[test]
    fn respects_min_size_and_passes_insurance_checks() {
        let (h, id) = harness(MetroCheckBox::new("短"));
        let r = h.rect(id);
        assert!(r.size.width >= CHECKBOX_MIN_WIDTH && r.size.height >= CHECKBOX_MIN_HEIGHT);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
        h.assert_access_contract();
    }

    /// 无障碍契约（E3）：勾选态进快照 checked；禁用反映在快照（参 access.rs）。
    #[test]
    fn access_contract_reports_checked_and_disabled() {
        let (mut h, id) = harness(MetroCheckBox::new("启用蓝牙"));
        let snap = h.access_snapshot();
        let node = snap.find(id).expect("复选框在快照内");
        assert_eq!(node.checked, Some(false), "未勾选 → checked=false");
        h.click(id);
        let snap = h.access_snapshot();
        assert_eq!(
            snap.find(id).unwrap().checked,
            Some(true),
            "勾选后 checked=true（勾选态反映在快照）"
        );
        h.tree.set_enabled(id, false);
        h.frame();
        let snap = h.access_snapshot();
        assert!(snap.find(id).unwrap().disabled);
        h.assert_access_contract();
    }

    #[test]
    fn long_label_grows_width() {
        let (h, id) = harness(MetroCheckBox::new("一个相当长的复选框标签，用来确认宽度随内容增长"));
        assert!(h.rect(id).size.width > CHECKBOX_MIN_WIDTH);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    fn render(cb: &MetroCheckBox) -> Scene {
        let engine = TextEngine::load(find_font().unwrap()).unwrap();
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        cb.render(&theme, &engine, Rect::new(0.0, 0.0, 200.0, 40.0), &mut scene);
        scene
    }

    #[test]
    fn two_state_toggle_cycles() {
        let mut cb = MetroCheckBox::new("启用");
        assert_eq!(cb.toggle(), CheckState::Checked);
        assert_eq!(cb.toggle(), CheckState::Unchecked, "默认两态循环");
    }

    #[test]
    fn three_state_toggle_with_allow() {
        let mut cb = MetroCheckBox::new("全选");
        cb.allow_indeterminate = true;
        assert_eq!(cb.toggle(), CheckState::Checked);
        assert_eq!(cb.toggle(), CheckState::Indeterminate);
        assert_eq!(cb.toggle(), CheckState::Unchecked);
        assert_eq!(cb.toggle(), CheckState::Checked);
    }

    #[test]
    fn checked_renders_accent_fill_and_glyph() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("启用");
        cb.state = CheckState::Checked;
        let scene = render(&cb);
        // 强调色填充
        let accent = scene
            .commands
            .iter()
            .any(|c| matches!(c, SceneCommand::FillRect { color, .. } if *color == MetroTheme::ether_dark().colors.primary));
        assert!(accent, "Checked 应有强调色填充");
        // 勾：几何快路径的四条三角（不再写字体字形 —— 字体里有没有 ✓ 不该决定界面对不对）
        let strokes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::Triangle { .. }))
            .count();
        assert_eq!(strokes, 4, "Checked 的勾应是 4 条三角");
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "✓")),
            "不许再写字体字形 ✓"
        );
    }

    #[test]
    fn unchecked_renders_stroke_not_fill() {
        if !font_available() {
            return;
        }
        let cb = MetroCheckBox::new("启用");
        assert_eq!(cb.state, CheckState::Unchecked);
        let scene = render(&cb);
        assert!(
            scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::StrokeRect { .. })),
            "Unchecked 应有描边"
        );
        let fills = scene
            .commands
            .iter()
            .filter(|c| matches!(c, SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(fills, 0, "Unchecked 无填充");
    }

    #[test]
    fn indeterminate_renders_dash() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("全选");
        cb.state = CheckState::Indeterminate;
        let scene = render(&cb);
        // 减号是位图 glyph（非几何快路径），按白色染色、12 pt 落格。
        let dash = scene.commands.iter().find_map(|c| match c {
            SceneCommand::Image { rect, tint, .. } => Some((*rect, *tint)),
            _ => None,
        });
        let (rect, tint) = dash.expect("Indeterminate 应画出减号位图");
        assert_eq!(tint, Some(Color::WHITE), "减号染白");
        assert_eq!(rect.size.width, ICON_12, "12 pt 落格");
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::Text { content, .. } if content == "—")),
            "不许再写字体字形 —"
        );
    }

    #[test]
    fn hit_test_contains() {
        let cb = MetroCheckBox::new("启用");
        let rect = Rect::new(0.0, 0.0, 200.0, 40.0);
        assert!(cb.hit_test(rect, Point::new(30.0, 20.0)));
        assert!(!cb.hit_test(rect, Point::new(300.0, 20.0)));
    }

    #[test]
    fn disabled_lowers_alpha() {
        if !font_available() {
            return;
        }
        let mut cb = MetroCheckBox::new("启用");
        cb.state = CheckState::Checked;
        cb.interact = ControlState::Disabled;
        let scene = render(&cb);
        // 强调色填充 alpha 应 < 1
        let a = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { color, .. } => Some(color.a),
                _ => None,
            })
            .fold(1.0f32, f32::min);
        assert!(a < 1.0, "禁用降透明度，实际 a={}", a);
    }

    // ── 内容槽（勾选框里放富标签，2026-11-06）────────────────────────────────

    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, LayoutProps, WidgetId};

    /// 勾选框里放内容：子节点占位在勾选框右侧（与标签同一列）。
    fn content_harness() -> (TestHarness, WidgetId, WidgetId) {
        use kanesumi_element::widgets::Label;
        let mut h = TestHarness::new(400.0, 200.0);
        let cb = h.tree.insert_with(
            h.root(),
            MetroCheckBox::new("启用蓝牙（读屏名字）"),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let label = h.tree.insert(
            cb,
            Label::new("启用蓝牙"),
        );
        h.frame();
        (h, cb, label)
    }

    #[test]
    fn content_slot_places_child_right_of_the_box() {
        let (h, cb, label) = content_harness();
        let r = h.rect(cb);
        let lr = h.rect(label);
        assert!(
            lr.origin.x >= r.origin.x + CHECKBOX_SIZE + CHECKBOX_BOX_GAP - 0.5,
            "内容在勾选框右侧：勾选框 {r:?}，内容 {lr:?}"
        );
        assert!(lr.right() <= r.right() + 0.5, "内容不越出控件");
        h.assert_contained();
    }

    #[test]
    fn content_slot_suppresses_label_text() {
        let (h, cb, _) = content_harness();
        let texts: Vec<String> = h
            .tree
            .painted(cb)
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !texts.iter().any(|t| t.contains("读屏名字")),
            "内容槽模式下不再画自己的标签：{texts:?}"
        );
    }

    #[test]
    fn content_slot_keeps_label_as_accessibility_name() {
        let (h, cb, _) = content_harness();
        let snap = h.access_snapshot();
        assert_eq!(
            snap.find(cb).and_then(|n| n.name.clone()).as_deref(),
            Some("启用蓝牙（读屏名字）")
        );
        h.assert_access_contract();
    }
}
