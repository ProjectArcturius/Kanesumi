use kanesumi_canvas::Scene;
use kanesumi_core::{Color, CornerRadius, MetroTheme, Rect, Size};

/// 控件形态 tokens —— 参 PLAN.md §4-5（Metro 形态：直角/极轻微圆角、无渐变纯色、内容优先）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlShape {
    pub corner_radius: CornerRadius,
    pub background: Color,
}

impl ControlShape {
    pub const fn new(corner_radius: CornerRadius, background: Color) -> Self {
        Self {
            corner_radius,
            background,
        }
    }
}

impl From<MetroTheme> for ControlShape {
    fn from(theme: MetroTheme) -> Self {
        Self {
            corner_radius: theme.tokens.corner_radius,
            background: theme.colors.surface,
        }
    }
}

/// MetroSurface —— 面板基底。可叠加交互 tint（hover/press）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroSurface {
    pub shape: ControlShape,
    /// 交互叠加 tint（悬停/按下）。无则不叠加。
    pub tint: Option<Color>,
}

impl MetroSurface {
    pub const fn new(shape: ControlShape) -> Self {
        Self { shape, tint: None }
    }

    pub const fn with_tint(mut self, tint: Color) -> Self {
        self.tint = Some(tint);
        self
    }

    /// 渲染到 `rect`。顺序：底色 → tint。使用 `shape.corner_radius`（参 V12：
    /// 原本 `fill_rect` 忽略 corner_radius，Slight/Capsule 面板全被拍直角）。
    pub fn render(&self, rect: Rect, scene: &mut Scene) {
        scene.fill_rounded_rect(self.shape.background, rect, self.shape.corner_radius);
        if let Some(tint) = self.tint {
            scene.fill_rounded_rect(tint, rect, self.shape.corner_radius);
        }
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）────────────────
//
// 容器外观：只画底 + tint，**不写 `arrange`** —— 框架默认把子节点铺满自身矩形
// （XAML `ContentControl` 语义），正好就是 MetroSurface 的内容容器语义。量测取子节点
// 期望尺寸的最大值，与 `kanesumi-element/src/widgets/border.rs` 同法。

impl kanesumi_element::Widget for MetroSurface {
    fn measure(&mut self, ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
        let mut s = Size::ZERO;
        for c in ctx.children() {
            let d = ctx.measure_child(c, available);
            s = Size::new(s.width.max(d.width), s.height.max(d.height));
        }
        s
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        self.render(ctx.rect(), scene);
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: String::new(),
            value: None,
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use crate::text::MetroText;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, LayoutProps, WidgetId};

    fn theme() -> MetroTheme {
        MetroTheme::ether_dark()
    }

    /// 在表面下挂一个文本子节点，验证「容器量测取子节点最大值」。
    fn harness(child: Option<MetroText>) -> (TestHarness, WidgetId, Option<WidgetId>) {
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroSurface::new(ControlShape::from(theme())),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let child = child.map(|c| {
            h.tree.insert_with(
                id,
                c,
                LayoutProps {
                    h_align: Align::Start,
                    v_align: Align::Start,
                    ..LayoutProps::default()
                },
            )
        });
        h.frame();
        (h, id, child)
    }

    #[test]
    fn measures_to_child_extent_and_passes_insurance_checks() {
        let (h, id, child) = harness(Some(MetroText::body(
            "一段用来撑开容器宽度的文本",
            Color::WHITE,
        )));
        let r = h.rect(id);
        assert!(r.size.width > 0.0 && r.size.height > 0.0, "应撑开：{r:?}");
        assert_eq!(
            r.size,
            h.rect(child.unwrap()).size,
            "表面尺寸 = 子节点期望尺寸"
        );
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn empty_surface_measures_zero() {
        let (h, id, _) = harness(None);
        assert_eq!(h.rect(id).size.width, 0.0);
        assert_eq!(h.rect(id).size.height, 0.0);
    }

    /// 压窄用例：表面被强制 40px 宽后子文本换行，三断言仍成立。
    #[test]
    fn squeezed_width_still_within() {
        let mut h = TestHarness::new(300.0, 200.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroSurface::new(ControlShape::from(theme())),
            LayoutProps {
                width: Some(40.0),
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.tree.insert_with(
            id,
            MetroText::body("很长的一行中文文本需要换行收束", Color::WHITE),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    #[test]
    fn renders_background() {
        let theme = MetroTheme::ether_dark();
        let surface = MetroSurface::new(ControlShape::from(theme));
        let mut scene = Scene::default();
        surface.render(Rect::new(0.0, 0.0, 100.0, 50.0), &mut scene);
        assert_eq!(scene.commands.len(), 1);
        assert!(matches!(scene.commands[0], SceneCommand::FillRect { .. }));
    }

    #[test]
    fn tint_adds_overlay() {
        let theme = MetroTheme::ether_dark();
        let surface =
            MetroSurface::new(ControlShape::from(theme)).with_tint(theme.indication.press_tint);
        let mut scene = Scene::default();
        surface.render(Rect::new(0.0, 0.0, 100.0, 50.0), &mut scene);
        assert_eq!(scene.commands.len(), 2);
    }
}
