// section_header.rs —— 面板里的分组小字标题（MetroSectionHeader）。
//
// 缺口：`settings/src/topbar/{action_center,net_panel,panel_rows}.rs` 三处各写一遍「小字标题行」，
// 行盒都要手算（`GROUP_ROW_H` / `HD_H` 都是 32）。本控件把这条行定死：文本走 caption、
// 次级墨色、左对齐；高度 = 行高 + 上下内距，由控件给出，调用方不再算一次
// （参 `docs/CONTROL_SPEC.md` §55.1）。
//
// 与 `panel_row.rs` 同构：`measure` 产出几何值对象，`render` 只消费它；命中 / 损伤也从
// 同一份几何取，不另算。

use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::MetroTypography;
use kanesumi_core::{MetroTheme, Point, Rect};

// 上下内距缺省 10 / 4（`action_center` 的 `.grp`：`padding: 10px 16px 4px`）住在
// `MetroSectionHeader::new` 的字段缺省里，**不立 `const f32`**：尺寸的定义归
// `kanesumi_core::sizes`，而 `kanesumi-controls/tests/size_discipline.rs` 的白名单只减不增
// （本卡不改 core，也不往白名单里加行）。参 `docs/CONTROL_SPEC.md` §55.1。

/// 分组小字标题。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroSectionHeader {
    text: String,
    pad_top: f32,
    pad_bottom: f32,
}

impl MetroSectionHeader {
    /// 建一条标题。文本原样保存（空串 / 全空白不画，参 [`Self::is_empty`]）。
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            pad_top: 10.0,
            pad_bottom: 4.0,
        }
    }

    /// 改上下内距。负值夹 0（负内距会让标题压到邻居身上）；非有限值回落到缺省。
    ///
    /// 存在理由：三处应用常量都是 32 高，但内距不同 —— `panel_rows::SectionHeader` 是
    /// `body_small`（12 / 16）配 `HD_TOP = 12`，本控件是 `caption`（18）配 10 / 4。
    /// 消费方要按自己的排版把行高钉回 32（例如 `with_padding(12, 2)`），不必改控件。
    pub fn with_padding(mut self, top: f32, bottom: f32) -> Self {
        self.pad_top = sane_pad(top, 10.0);
        self.pad_bottom = sane_pad(bottom, 4.0);
        self
    }

    /// 标题文本。
    pub fn text(&self) -> &str {
        &self.text
    }

    /// 空标题：不产出任何绘制命令（空白串一并算空，避免「看起来有行、其实零墨迹」的假行）。
    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }

    /// 行高 = caption 行高 + 上下内距。缺省 `18 + 10 + 4 = 32`，与三处应用常量同值。
    pub fn height(&self) -> f32 {
        style().line_height + self.pad_top + self.pad_bottom
    }

    /// 量一条标题：`width` 给可用宽，高由 [`Self::height`] 得出（**不写死 32**）。
    pub fn measure(&self, origin: Point, width: f32) -> SectionHeaderGeometry {
        let rect = Rect::new(origin.x, origin.y, width.max(0.0), self.height());
        let text = Rect::new(
            rect.origin.x,
            rect.origin.y + self.pad_top,
            rect.size.width,
            style().line_height,
        );
        SectionHeaderGeometry { rect, text }
    }

    /// 画标题：有文本返回损伤矩形（整行盒），空文本返回 `None` 且零命令。
    pub fn render(
        &self,
        theme: &MetroTheme,
        geo: &SectionHeaderGeometry,
        scene: &mut Scene,
    ) -> Option<Rect> {
        if self.is_empty() {
            return None;
        }
        scene.label(
            self.text.clone(),
            geo.text,
            theme.colors.on_surface_variant,
            theme.typography.caption,
            TextAlign::Left,
        );
        Some(geo.rect)
    }
}

/// 标题几何（绝对坐标）。损伤与绘制消费同一份。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SectionHeaderGeometry {
    /// 整行盒（含上下内距）—— 损伤用。
    pub rect: Rect,
    /// 文本行盒。
    pub text: Rect,
}

impl SectionHeaderGeometry {
    /// 整行盒。
    pub fn rect(&self) -> Rect {
        self.rect
    }

    /// 文本行盒。
    pub fn text_rect(&self) -> Rect {
        self.text
    }
}

fn style() -> kanesumi_core::typography::TextStyle {
    MetroTypography::metro().caption
}

fn sane_pad(v: f32, fallback: f32) -> f32 {
    if v.is_finite() { v.max(0.0) } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    #[test]
    fn default_height_is_line_height_plus_padding() {
        // 手算自 caption（13 pt / 行高 18）与 .grp 的 10 / 4：18 + 10 + 4 = 32。
        let header = MetroSectionHeader::new("选择播放设备");
        assert_eq!(header.height(), 32.0);
        // 换 body_small 的消费方（panel_rows 的 12 / 16 配 HD_TOP = 12）钉回同一行高。
        assert_eq!(header.clone().with_padding(12.0, 0.0).height(), 30.0);
        // 负内距夹 0，非有限值回落缺省。
        assert_eq!(header.clone().with_padding(-8.0, -8.0).height(), 18.0);
        assert_eq!(header.with_padding(f32::NAN, 4.0).height(), 32.0);
    }

    #[test]
    fn measure_puts_text_one_pad_below_origin() {
        let geo = MetroSectionHeader::new("电源模式").measure(Point::new(16.0, 40.0), 328.0);
        assert_eq!(geo.rect(), Rect::new(16.0, 40.0, 328.0, 32.0));
        // 文本行盒：上内距 10、行高 18，左右内距归容器（与 .grp 的 16 由面板给一致）。
        assert_eq!(geo.text_rect(), Rect::new(16.0, 50.0, 328.0, 18.0));
    }

    #[test]
    fn empty_text_paints_nothing() {
        let theme = MetroTheme::ether_dark();
        let header = MetroSectionHeader::new("   ");
        let geo = header.measure(Point::new(0.0, 0.0), 300.0);
        let mut scene = Scene::default();
        assert_eq!(header.render(&theme, &geo, &mut scene), None);
        assert!(scene.commands.is_empty(), "空标题不得产出命令");
        assert_eq!(header.height(), 32.0, "空标题仍占位，元素数目不变");
    }

    #[test]
    fn paints_one_caption_label_in_secondary_ink() {
        let theme = MetroTheme::ether_dark();
        let header = MetroSectionHeader::new("亮度");
        let geo = header.measure(Point::new(0.0, 0.0), 200.0);
        let mut scene = Scene::default();
        assert_eq!(header.render(&theme, &geo, &mut scene), Some(geo.rect));
        assert_eq!(scene.commands.len(), 1);
        let SceneCommand::Text {
            content,
            rect,
            color,
            style,
            align,
            wrap,
            max_lines,
            ..
        } = &scene.commands[0]
        else {
            panic!("小字标题应只有一条文本命令：{:?}", scene.commands);
        };
        assert_eq!(content, "亮度");
        assert_eq!(*rect, geo.text_rect());
        assert_eq!(*color, theme.colors.on_surface_variant);
        assert_eq!(style.size, 13.0);
        assert_eq!(style.line_height, 18.0);
        assert_eq!(*align, TextAlign::Left);
        assert!(!wrap, "标题单行，不换行");
        assert_eq!(*max_lines, Some(1));
    }
}
