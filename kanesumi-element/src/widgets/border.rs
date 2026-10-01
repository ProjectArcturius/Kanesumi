// Border —— 背景 / 描边 / 内边距容器（XAML Border）。子节点 Z 叠放于内边距矩形内。
//
// 命中语义随背景：无背景 = 空白处穿透（XAML `Background="{x:Null}"`），有背景 = 挡住下层。

use kanesumi_canvas::Scene;
use kanesumi_core::{Brush, Point, Rect, Size};

use crate::props::Insets;
use crate::widget::{ArrangeCtx, MeasureCtx, PaintCtx, Widget};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Border {
    /// 背景。用主题令牌（`ThemeColor::*`）才会随深浅方案 / accent 变化。
    pub background: Option<Brush>,
    /// 描边（颜色, 粗细）。画在矩形内侧，不外扩。
    pub stroke: Option<(Brush, f32)>,
    pub padding: Insets,
}

impl Border {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn background(mut self, c: impl Into<Brush>) -> Self {
        self.background = Some(c.into());
        self
    }

    pub fn stroke(mut self, c: impl Into<Brush>, thickness: f32) -> Self {
        self.stroke = Some((c.into(), thickness.max(0.0)));
        self
    }

    pub fn padding(mut self, p: Insets) -> Self {
        self.padding = p;
        self
    }
}

impl Widget for Border {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let inner = self.padding.shrink(available);
        let mut s = Size::ZERO;
        for c in ctx.children() {
            let d = ctx.measure_child(c, inner);
            s = Size::new(s.width.max(d.width), s.height.max(d.height));
        }
        self.padding.grow(s)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let inner = self.padding.deflate(rect);
        for c in ctx.children() {
            ctx.arrange_child(c, inner);
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let rect = ctx.rect();
        let theme = *ctx.theme();
        if let Some(bg) = self.background {
            scene.fill_rect(bg.resolve(&theme), rect);
        }
        if let Some((c, t)) = self.stroke
            && t > 0.0
        {
            scene.stroke_rect(c.resolve(&theme), rect, t);
        }
    }

    fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        self.background.is_some() && rect.contains(pos)
    }
}
