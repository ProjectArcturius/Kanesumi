// icon_slot.rs —— 面板行首的图标位（MetroIconSlot）。
//
// 缺口：`settings/src/topbar/panel_rows.rs::IconSlot` 手算「32 宽的列里居中一枚 20 pt 图标」
// （`(rect.size.width - pt) / 2.0`），`action_center` / `net_panel` 里同样的居中算式又各写一遍。
// 本控件只做两件事：算居中方框、在有图时把那枚已光栅化的位图画进去。
//
// **不解码**：SVG → RGBA 归调用方（`kanesumi_canvas::icon::rasterize_svg` / `Icon::load_svg`），
// 本控件不碰文件、不碰 `resvg`。参 `docs/CONTROL_SPEC.md` §55.2。

use kanesumi_canvas::Scene;
use kanesumi_canvas::icon::Icon;
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

/// 行首图标位：给定槽矩形，算出居中的正方形（损伤盒与 blit 目标同一份）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroIconSlot {
    size: f32,
}

impl Default for MetroIconSlot {
    fn default() -> Self {
        Self {
            size: sizes::ICON_20,
        }
    }
}

impl MetroIconSlot {
    /// 缺省 20 pt（`panel_rows::ICON_PT` / `net_panel::ROW_ICON` 同值）。
    pub fn new() -> Self {
        Self::default()
    }

    /// 改边长。负值夹 0；非有限值回落 20 pt（尺寸错不该让整行消失）。
    pub fn with_size(mut self, pt: f32) -> Self {
        self.size = if pt.is_finite() {
            pt.max(0.0)
        } else {
            sizes::ICON_20
        };
        self
    }

    /// 当前边长。
    pub fn size(&self) -> f32 {
        self.size
    }

    /// 居中方框：槽内水平 / 垂直居中；槽的短边小于边长时**夹到短边**再居中
    /// （照 `panel_rows::IconSlot::paint` 的 `(w - pt) / 2`，另加「不许溢出槽」的夹紧）。
    /// 槽或边长为 0 时返回零矩形 —— 调用方据此跳过损伤。
    pub fn rect(&self, slot: Rect) -> Rect {
        let size = slot.normalized();
        let pt = self
            .size
            .min(size.size.width)
            .min(size.size.height)
            .max(0.0);
        if pt <= 0.0 {
            return Rect::new(size.origin.x, size.origin.y, 0.0, 0.0);
        }
        Rect::new(
            size.origin.x + (size.size.width - pt) / 2.0,
            size.origin.y + (size.size.height - pt) / 2.0,
            pt,
            pt,
        )
    }

    /// 画图标：`icon = None` 返回 `None` 且**零命令**；有图返回损伤矩形。
    ///
    /// 染色固定 `on_surface`（与 `panel_rows::IconSlot` 一致）：图标是行首的非交互标识，
    /// 不跟随强调色，免得与行尾控件抢注意力。
    pub fn render(
        &self,
        theme: &MetroTheme,
        slot: Rect,
        icon: Option<&Icon>,
        scene: &mut Scene,
    ) -> Option<Rect> {
        let icon = icon?;
        let rect = self.rect(slot);
        if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            return None;
        }
        scene.image(icon, rect, Some(theme.colors.on_surface));
        Some(rect)
    }
}

/// 状态簇的前置、中间与贴右槽位。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StripSlot {
    Leading,
    Item(usize),
    Trailing,
}

/// 固定数量、允许不同宽度的槽位条。参 CONTROL_SPEC §55.2。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroSlotStrip<const N: usize> {
    bounds: Rect,
    pub leading: Option<Rect>,
    pub items: [Rect; N],
    pub trailing: Rect,
}

impl<const N: usize> MetroSlotStrip<N> {
    pub fn new(bounds: Rect, leading: Option<f32>, widths: [f32; N], trailing: f32) -> Self {
        let leading = leading
            .map(|width| Rect::new(bounds.origin.x, bounds.origin.y, width, bounds.size.height));
        let mut x = leading.map_or(bounds.origin.x, Rect::right);
        let items = widths.map(|width| {
            let cell = Rect::new(x, bounds.origin.y, width, bounds.size.height);
            x = cell.right();
            cell
        });
        let trailing = Rect::new(
            bounds.right() - trailing,
            bounds.origin.y,
            trailing,
            bounds.size.height,
        );
        Self {
            bounds,
            leading,
            items,
            trailing,
        }
    }

    pub fn slot(&self, slot: StripSlot) -> Option<Rect> {
        match slot {
            StripSlot::Leading => self.leading,
            StripSlot::Item(index) => self.items.get(index).copied(),
            StripSlot::Trailing => Some(self.trailing),
        }
    }

    pub fn slots(&self) -> impl Iterator<Item = (StripSlot, Rect)> + '_ {
        self.leading
            .map(|rect| (StripSlot::Leading, rect))
            .into_iter()
            .chain(
                self.items
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(i, rect)| (StripSlot::Item(i), rect)),
            )
            .chain(std::iter::once((StripSlot::Trailing, self.trailing)))
    }

    /// 空间不足导致槽位重叠时，先判前置槽与贴右槽。
    pub fn at(&self, point: Point) -> Option<StripSlot> {
        if !self.bounds.contains(point) {
            return None;
        }
        if self.leading.is_some_and(|r| r.contains(point)) {
            return Some(StripSlot::Leading);
        }
        if self.trailing.contains(point) {
            return Some(StripSlot::Trailing);
        }
        self.items
            .iter()
            .position(|r| r.contains(point))
            .or_else(|| N.checked_sub(1))
            .map(StripSlot::Item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;
    use kanesumi_core::Point;

    fn icon_20() -> Icon {
        Icon {
            rgba: vec![0u8; 20 * 20 * 4].into(),
            width: 20,
            height: 20,
        }
    }

    #[test]
    fn strip_preserves_edge_priority_when_items_overlap() {
        let bounds = Rect::new(100.0, 20.0, 80.0, 30.0);
        let strip = MetroSlotStrip::new(bounds, Some(30.0), [26.0; 4], 34.0);
        assert_eq!(strip.at(Point::new(129.0, 25.0)), Some(StripSlot::Leading));
        assert_eq!(strip.at(Point::new(140.0, 25.0)), Some(StripSlot::Item(0)));
        assert_eq!(strip.at(Point::new(150.0, 25.0)), Some(StripSlot::Trailing));
        assert_eq!(strip.at(Point::new(180.0, 25.0)), None);

        let narrow = Rect::new(100.0, 20.0, 20.0, 30.0);
        let both = MetroSlotStrip::new(narrow, Some(30.0), [26.0; 4], 34.0);
        assert_eq!(both.at(Point::new(110.0, 25.0)), Some(StripSlot::Leading));
    }

    #[test]
    fn default_is_20_centered_in_a_32_slot() {
        // panel_rows 的图标列 32 × 行高 32：(32 - 20) / 2 = 6。
        let slot = Rect::new(16.0, 4.0, 32.0, 32.0);
        let s = MetroIconSlot::new();
        assert_eq!(s.size(), 20.0);
        assert_eq!(s.rect(slot), Rect::new(22.0, 10.0, 20.0, 20.0));
    }

    #[test]
    fn smaller_slot_clamps_and_recenters() {
        // 槽 12 × 12 装不下 20 pt：夹到 12 并居中（偏移 0）。
        let s = MetroIconSlot::new();
        assert_eq!(
            s.rect(Rect::new(0.0, 0.0, 12.0, 12.0)),
            Rect::new(0.0, 0.0, 12.0, 12.0)
        );
        // 非正方槽：短边定尺寸，两边各自居中。槽 40 × 28 → 20 pt，x +10、y +4。
        assert_eq!(
            s.rect(Rect::new(0.0, 0.0, 40.0, 28.0)),
            Rect::new(10.0, 4.0, 20.0, 20.0)
        );
    }

    #[test]
    fn custom_size_and_zero_size() {
        let slot = Rect::new(0.0, 0.0, 32.0, 32.0);
        assert_eq!(
            MetroIconSlot::new().with_size(24.0).rect(slot),
            Rect::new(4.0, 4.0, 24.0, 24.0)
        );
        // 0 边长 = 零矩形（不产出像素），非有限值回落 20。
        assert_eq!(
            MetroIconSlot::new().with_size(0.0).rect(slot),
            Rect::new(0.0, 0.0, 0.0, 0.0)
        );
        assert_eq!(MetroIconSlot::new().with_size(f32::NAN).size(), 20.0);
        // 负宽高的槽：`Rect::normalized` 把宽高夹 0（不回卷原点），于是零矩形。
        assert_eq!(
            MetroIconSlot::new().rect(Rect::new(10.0, 10.0, -4.0, -4.0)),
            Rect::new(10.0, 10.0, 0.0, 0.0)
        );
    }

    #[test]
    fn no_icon_paints_nothing() {
        let theme = MetroTheme::ether_dark();
        let mut scene = Scene::default();
        let slot = Rect::new(0.0, 0.0, 32.0, 32.0);
        assert_eq!(
            MetroIconSlot::new().render(&theme, slot, None, &mut scene),
            None
        );
        assert!(scene.commands.is_empty(), "无图不得产出命令");
    }

    #[test]
    fn icon_paints_one_tinted_image_at_the_centered_box() {
        let theme = MetroTheme::ether_dark();
        let slot = Rect::new(16.0, 4.0, 32.0, 32.0);
        let icon = icon_20();
        let mut scene = Scene::default();
        let damage = MetroIconSlot::new().render(&theme, slot, Some(&icon), &mut scene);
        assert_eq!(damage, Some(Rect::new(22.0, 10.0, 20.0, 20.0)));
        assert_eq!(scene.commands.len(), 1);
        let SceneCommand::Image {
            rect,
            tint,
            width,
            height,
            ..
        } = &scene.commands[0]
        else {
            panic!("图标位应只有一条图像命令：{:?}", scene.commands);
        };
        assert_eq!(*rect, Rect::new(22.0, 10.0, 20.0, 20.0));
        assert_eq!(*tint, Some(theme.colors.on_surface));
        assert_eq!((*width, *height), (20, 20));
        assert_eq!(damage.unwrap().origin, Point::new(22.0, 10.0));
    }

    #[test]
    fn zero_sized_slot_never_emits_an_image() {
        let theme = MetroTheme::ether_dark();
        let icon = icon_20();
        let mut scene = Scene::default();
        let s = MetroIconSlot::new();
        assert_eq!(
            s.render(
                &theme,
                Rect::new(4.0, 4.0, 0.0, 0.0),
                Some(&icon),
                &mut scene
            ),
            None
        );
        assert!(scene.commands.is_empty());
    }
}
