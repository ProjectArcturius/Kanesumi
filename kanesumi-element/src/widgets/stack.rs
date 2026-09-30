// Stack —— 行 / 列容器（XAML StackPanel + Star 分配的合流）。
//
// 主轴分配沿用 kanesumi-canvas `layout.rs` 的算法语义（grow 分剩余、shrink 按「权重 × 可压缩量」
// 压缩、min_main 为下限），只是权重改从子节点的框架属性 `LayoutProps` 读 ——
// 交叉轴对齐不在这里做，由框架按子节点的 h_align / v_align 在槽位内解析。

use kanesumi_canvas::Scene;
use kanesumi_core::{Point, Rect, Size};

use crate::id::WidgetId;
use crate::props::LayoutProps;
use crate::widget::{ArrangeCtx, MeasureCtx, PaintCtx, Widget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    /// 水平排列（主轴 X）。
    Row,
    /// 垂直排列（主轴 Y）。
    Column,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stack {
    pub axis: Axis,
    pub spacing: f32,
}

impl Stack {
    pub fn row() -> Self {
        Self {
            axis: Axis::Row,
            spacing: 0.0,
        }
    }

    pub fn column() -> Self {
        Self {
            axis: Axis::Column,
            spacing: 0.0,
        }
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing.max(0.0);
        self
    }

    fn main(&self, s: Size) -> f32 {
        match self.axis {
            Axis::Row => s.width,
            Axis::Column => s.height,
        }
    }

    fn cross(&self, s: Size) -> f32 {
        match self.axis {
            Axis::Row => s.height,
            Axis::Column => s.width,
        }
    }

    fn size(&self, main: f32, cross: f32) -> Size {
        match self.axis {
            Axis::Row => Size::new(main, cross),
            Axis::Column => Size::new(cross, main),
        }
    }

    /// 子节点量测约束：主轴无界、交叉轴 = 父交叉。
    fn child_available(&self, available: Size) -> Size {
        self.size(f32::INFINITY, self.cross(available))
    }

    fn visible_children(ids: Vec<WidgetId>, props: impl Fn(WidgetId) -> LayoutProps) -> Vec<(WidgetId, LayoutProps)> {
        ids.into_iter()
            .map(|c| (c, props(c)))
            .filter(|(_, p)| p.visible)
            .collect()
    }
}

impl Widget for Stack {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let kids = Self::visible_children(ctx.children(), |c| ctx.child_props(c));
        let child_avail = self.child_available(available);
        let mut main = 0.0f32;
        let mut cross = 0.0f32;
        for (i, (c, _)) in kids.iter().enumerate() {
            let d = ctx.measure_child(*c, child_avail);
            if i > 0 {
                main += self.spacing;
            }
            main += self.main(d);
            cross = cross.max(self.cross(d));
        }
        self.size(main, cross)
    }

    fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
        let kids = Self::visible_children(ctx.children(), |c| ctx.child_props(c));
        if kids.is_empty() {
            return;
        }
        let child_avail = self.child_available(rect.size);
        let desired: Vec<f32> = kids
            .iter()
            .map(|(c, _)| {
                let d = ctx.measure_child(*c, child_avail);
                self.main(d)
            })
            .collect();
        let gaps = self.spacing * (kids.len() - 1) as f32;
        let available = (self.main(rect.size) - gaps).max(0.0);
        let props: Vec<LayoutProps> = kids.iter().map(|(_, p)| *p).collect();
        let sizes = distribute(&props, &desired, available);

        let mut cursor = match self.axis {
            Axis::Row => rect.origin.x,
            Axis::Column => rect.origin.y,
        };
        for ((c, _), len) in kids.iter().zip(sizes) {
            let slot = match self.axis {
                Axis::Row => Rect::new(cursor, rect.origin.y, len, rect.size.height),
                Axis::Column => Rect::new(rect.origin.x, cursor, rect.size.width, len),
            };
            ctx.arrange_child(*c, slot);
            cursor += len + self.spacing;
        }
    }

    fn paint(&mut self, _ctx: &mut PaintCtx, _scene: &mut Scene) {}

    /// 无背景容器不命中：空白处穿透到下层（COMPOSITION 契约 12）。
    fn hit_test(&self, _rect: Rect, _pos: Point) -> bool {
        false
    }
}

/// 主轴分配（语义同 kanesumi-canvas `layout::distribute_main`）。
fn distribute(props: &[LayoutProps], desired: &[f32], available: f32) -> Vec<f32> {
    let mut sizes: Vec<f32> = desired.iter().map(|v| v.max(0.0)).collect();
    let total: f32 = sizes.iter().sum();
    if total < available {
        let grow_total: f32 = props.iter().map(|p| p.grow.max(0.0)).sum();
        if grow_total > 0.0 {
            let extra = available - total;
            for (size, p) in sizes.iter_mut().zip(props) {
                *size += extra * p.grow.max(0.0) / grow_total;
            }
        }
        return sizes;
    }
    let mut deficit = total - available;
    let mut active = vec![true; props.len()];
    while deficit > 0.001 {
        let weight_total: f32 = props
            .iter()
            .enumerate()
            .filter(|(i, _)| active[*i])
            .map(|(i, p)| p.shrink.max(0.0) * (sizes[i] - p.min_main.max(0.0)).max(0.0))
            .sum();
        if weight_total <= 0.0 {
            break;
        }
        let before = deficit;
        for (i, p) in props.iter().enumerate() {
            if !active[i] {
                continue;
            }
            let capacity = (sizes[i] - p.min_main.max(0.0)).max(0.0);
            let weight = p.shrink.max(0.0) * capacity;
            let reduction = (before * weight / weight_total).min(capacity);
            sizes[i] -= reduction;
            deficit -= reduction;
            if capacity - reduction <= 0.001 {
                active[i] = false;
            }
        }
        if (before - deficit).abs() <= 0.001 {
            break;
        }
    }
    sizes
}
