// ssd_chrome.rs —— 服务端窗口装饰（SSD）几何控件（MetroSsdChrome）。
//
// 缺口：Ether `compositor/src/wm/ssd.rs` 里五处自造的布局 / 叠色算术
// （`ssd_rects_logical` / `ssd_rects` / `title_row` / `border_thickness` / `over`）只长在合成器里，
// 判据 2「合成器三块自绘走同一元件层」缺最后一块（前两块是面板部件与 Dock 呈现）。本控件把那
// 五处的算术逐字收进来，合成器侧只留接线（本卡不改 `compositor/` 一行）。
//
// 本件**只算不画**：没有 `render`，产出逻辑几何 / 物理标题行 / 线宽 / 叠色值，由合成器落成
// 自绘命令。逻辑坐标是 pt，物理量由调用方给 `scale` —— 与 `ssd.rs` 同一口径。
// 参 `docs/CONTROL_SPEC.md` §56、`docs/research/m1115_ssd_chrome/REPORT.md`。

use kanesumi_core::{Color, Point, Rect, Size};

/// SSD 几何参数。缺省值全部来自 Ether `compositor/src/config.rs:31-46`，逐字段注出处行。
///
/// 它们只住在字段缺省里，**不立 `const f32`**：尺寸的定义归 `kanesumi_core::sizes`，而
/// `kanesumi-controls/tests/size_discipline.rs` 的裸尺寸白名单只减不增（本卡不改 core）。
/// 与 `section_header.rs` / `tile_grid.rs` 同做法。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SsdChromeMetrics {
    pub titlebar_h: f32,     // 28  config.rs:31 WIN_TITLEBAR_HEIGHT
    pub btn_w: f32,          // 44  config.rs:33 WIN_BTN_WIDTH
    pub btn_h: f32,          // 28  config.rs:34 WIN_BTN_HEIGHT
    pub title_pad_left: f32, // 12  config.rs:36 WIN_TITLE_PAD_LEFT
    pub app_icon_gap: f32,   // 8   config.rs:40 WIN_APP_ICON_GAP
    pub app_name_gap: f32,   // 12  config.rs:42 WIN_APP_NAME_GAP
    pub sep_w: f32,          // 1   config.rs:46 WIN_DIVIDER（分隔线按 px(1).max(1) 走）
    pub sep_gap: f32,        // 8   ssd.rs:216 的 px(8.0)（分隔线后的间隔）
    pub right_pad: f32,      // 8   ssd.rs:205 的 px(8.0)（标题栏右缘留白）
    pub divider: f32,        // 1   config.rs:46 WIN_DIVIDER
    pub position_line: f32,  // 2   config.rs:44 WIN_POSITION_LINE
}

impl Default for SsdChromeMetrics {
    fn default() -> Self {
        Self {
            titlebar_h: 28.0,
            btn_w: 44.0,
            btn_h: 28.0,
            title_pad_left: 12.0,
            app_icon_gap: 8.0,
            app_name_gap: 12.0,
            sep_w: 1.0,
            sep_gap: 8.0,
            right_pad: 8.0,
            divider: 1.0,
            position_line: 2.0,
        }
    }
}

/// 逻辑坐标（pt）的 SSD 几何。`origin` 与 `size` 与 `ssd.rs::SsdRectsLogical` 同义：
/// 标题栏 `y0 = loc.y − 标题栏高`，三按钮从内容右缘倒数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SsdGeometry {
    pub titlebar: Rect,
    pub minimize: Rect,
    pub maximize: Rect,
    pub close: Rect,
    /// 窗口完整外框（标题栏顶边 → 内容底边），逻辑坐标；物理外框由调用方按
    /// `display::scale_rect` 的两端点取整得出（`ssd.rs:149-158` 那段）。
    pub outer: Rect,
}

/// `Rect` 没有 `Default`（`kanesumi_core::geometry` 只给 `Size::ZERO`），而本卡不改 core 的
/// 尺寸层，故手写零矩形 —— 语义与 `SsdRectsLogical::default()` 相同：窗口未提交首帧时的空几何。
impl Default for SsdGeometry {
    fn default() -> Self {
        let zero = Rect::new(0.0, 0.0, 0.0, 0.0);
        Self {
            titlebar: zero,
            minimize: zero,
            maximize: zero,
            close: zero,
            outer: zero,
        }
    }
}

/// 标题行「身份区 + 标题」的横向排布（物理 px，出参 f64）。与 `ssd.rs:179-190` 的
/// `TitleRow` 同义，只是把 i32 换成 f64 —— 控件不替调用方决定取整落点。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SsdTitleRow {
    pub icon_x: f64,
    pub name_x: f64,
    pub sep_x: Option<f64>,
    pub title_x: f64,
    pub title_avail: f64,
}

/// SSD 几何控件。持一份 metrics，方法全是纯函数（无隐藏状态）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MetroSsdChrome {
    metrics: SsdChromeMetrics,
}

impl Default for MetroSsdChrome {
    fn default() -> Self {
        Self::new()
    }
}

impl MetroSsdChrome {
    /// 用缺省 metrics（Ether `config.rs` 的当前值）。
    pub fn new() -> Self {
        Self { metrics: SsdChromeMetrics::default() }
    }

    /// 换一套 metrics（合成器配置可改时用）。
    pub fn with_metrics(m: SsdChromeMetrics) -> Self {
        Self { metrics: m }
    }

    /// 逻辑几何。`loc` = surface 在逻辑空间的原点；`geo_offset_x` = 内容在 surface buffer 内的
    /// X 偏移（`ssd.rs` 的 `geo.loc.x`）；`content` = 内容尺寸（surface − 标题栏）。
    ///
    /// content 宽或高 ≤ 0 → 全 default：configure 未回应时窗口尚未提交首帧，此时算出任何矩形
    /// 都会在屏幕上留一块「幽灵标题栏」（`ssd.rs:94-97` 的同一判据）。NaN 同此处理 —— 否则
    /// NaN 会顺着矩形流进渲染。
    pub fn geometry(&self, loc: Point, geo_offset_x: f64, content: Size) -> SsdGeometry {
        // NaN 显式挡掉：`width <= 0.0` 对 NaN 为 false，不挡的话 NaN 会顺着矩形流进渲染。
        if !content.width.is_finite() || !content.height.is_finite() {
            return SsdGeometry::default();
        }
        if content.width <= 0.0 || content.height <= 0.0 {
            return SsdGeometry::default();
        }
        let tb_h = self.metrics.titlebar_h as f64;
        let btn_w = self.metrics.btn_w as f64;
        let btn_h = self.metrics.btn_h as f64;
        let cw = content.width as f64;
        let ch = content.height as f64;
        let x0 = loc.x as f64 + geo_offset_x;
        let y0 = loc.y as f64 - tb_h;
        let right = x0 + cw;

        SsdGeometry {
            titlebar: rect_f32(x0, y0, cw, tb_h),
            // 三按钮相邻、贴右缘：关闭最右，然后最大化、最小化（参 preview.html .buttons/.btn）。
            close: rect_f32(right - btn_w, y0, btn_w, btn_h),
            maximize: rect_f32(right - 2.0 * btn_w, y0, btn_w, btn_h),
            minimize: rect_f32(right - 3.0 * btn_w, y0, btn_w, btn_h),
            outer: rect_f32(x0, y0, cw, tb_h + ch),
        }
    }

    /// 物理标题行：逐字复刻 `ssd.rs:194-222`，含 `px(pt) = (pt*scale).round()`。
    /// 宽度一律由调用方给已光栅纹理的像素宽（0 = 不存在），本方法不估算字宽。
    pub fn title_row(&self, titlebar: Rect, btn_w: f64, scale: f64, icon_w: f64, name_w: f64, has_title: bool) -> SsdTitleRow {
        let px = |pt: f64| (pt * scale).round();
        let mut x = titlebar.origin.x as f64 + px(self.metrics.title_pad_left as f64);
        // 可用右界 = 标题栏右缘 − 三按钮 − 右侧留白（留白也随倍率走）。
        let right = titlebar.origin.x as f64 + titlebar.size.width as f64
            - 3.0 * btn_w
            - px(self.metrics.right_pad as f64);
        let icon_x = x;
        if icon_w > 0.0 {
            x += icon_w + px(self.metrics.app_icon_gap as f64);
        }
        let name_x = x;
        if name_w > 0.0 {
            x += name_w + px(self.metrics.app_name_gap as f64);
        }
        // 只有「有应用名 + 有标题」才画分隔：否则留一根孤立竖线。
        let sep_x = if name_w > 0.0 && has_title {
            let at = x;
            x += px(self.metrics.sep_w as f64).max(1.0) + px(self.metrics.sep_gap as f64);
            Some(at)
        } else {
            None
        };
        SsdTitleRow { icon_x, name_x, sep_x, title_x: x, title_avail: right - x }
    }

    /// `(分隔线, 激活定位线)` 的物理像素厚度，各自 `.max(1)`：2× 下描边若只有一半厚，
    /// 与标题栏 / 按钮矩形的 2× 放大不一致（`ssd.rs:637-643`）。
    pub fn border_thickness(&self, scale: f64) -> (f64, f64) {
        let divider = (self.metrics.divider as f64 * scale).round().max(1.0);
        let position = (self.metrics.position_line as f64 * scale).round().max(1.0);
        (divider, position)
    }

    /// 物理坐标下的顶、底、左、右分隔线及激活定位线。参 CONTROL_SPEC §57。
    pub fn border_rects(&self, outer: Rect, scale: f64) -> [Rect; 5] {
        let (border, line) = self.border_thickness(scale);
        let (border, line) = (border.round() as f32, line.round() as f32);
        let (x, y) = (outer.origin.x, outer.origin.y);
        let (w, h) = (outer.size.width, outer.size.height);
        [
            Rect::new(x, y, w, border),
            Rect::new(x, outer.bottom() - border, w, border),
            Rect::new(x, y, border, h),
            Rect::new(outer.right() - border, y, border, h),
            Rect::new(x, y, w, line),
        ]
    }

    /// 已光栅化的按钮符号在物理按钮框内居中，半像素按旧整数除法截断。
    pub fn symbol_rect(&self, button: Rect, raster: Size) -> Option<Rect> {
        if raster.is_empty() {
            return None;
        }
        Some(Rect::new(
            button.origin.x + ((button.size.width - raster.width) / 2.0).trunc(),
            center_y(button, raster.height),
            raster.width,
            raster.height,
        ))
    }

    /// 已光栅化的身份或标题纹理，右边界之前最多保留可见的物理像素。
    pub fn title_part_rect(&self, titlebar: Rect, raster: Size, x: i32, right: i32, dy: i32) -> Option<Rect> {
        let width = raster.width.min((right - x) as f32);
        if raster.is_empty() || width <= 0.0 {
            return None;
        }
        Some(Rect::new(x as f32, center_y(titlebar, raster.height) + dy as f32, width, raster.height))
    }

    /// 应用名与窗口标题之间的竖分隔线。参 Ether SSD_DESIGN §Ⅱ-2。
    pub fn title_separator_rect(&self, titlebar: Rect, x: i32, scale: f64) -> Rect {
        let h = (12.0 * scale).round() as f32;
        let w = (self.metrics.sep_w as f64 * scale).round().max(1.0) as f32;
        Rect::new(x as f32, center_y(titlebar, h), w, h)
    }

    /// 不透明底叠上色（按钮悬停 / 按下硬切），alpha 强制 1.0。
    pub fn over(&self, base: Color, top: Color, a: f32) -> Color {
        Color {
            r: base.r + (top.r - base.r) * a,
            g: base.g + (top.g - base.g) * a,
            b: base.b + (top.b - base.b) * a,
            a: 1.0,
        }
    }
}

fn center_y(rect: Rect, height: f32) -> f32 {
    rect.origin.y + ((rect.size.height - height) / 2.0).trunc()
}

/// 单行标题：放得下就原样，放不下按**字符**从尾部收、末尾补省略号；连省略号都放不下
/// （`avail` ≤ 0，或空串）就返回 None —— 不画，而不是画半个字。
///
/// `measure` 给出候选串的光栅尺寸（生产上与上传纹理使用同一文字引擎；
/// 单测里是假测量），故「空串不画」「超宽加省略号」能在无 GPU、无字体的单测里锁住。
/// 取前缀而非裁位图：位图硬裁会把最后一个汉字切成半笔，读起来像「没有字」。
pub fn fit_ssd_title(
    text: &str,
    avail: i32,
    measure: &mut dyn FnMut(&str) -> (u32, u32),
) -> Option<(String, u32, u32)> {
    if text.is_empty() || avail <= 0 {
        return None;
    }
    let (w, h) = measure(text);
    if w == 0 || h == 0 {
        return None;
    }
    if w as i32 <= avail {
        return Some((text.to_string(), w, h));
    }
    // 二分最大可放下的前缀。宽度对前缀长度单调不减（字距调整最多 ±1 px，不影响判据）。
    let chars: Vec<char> = text.chars().collect();
    let (mut lo, mut hi) = (0usize, chars.len());
    let mut best: Option<(String, u32, u32)> = None;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let mut cand: String = chars[..mid].iter().collect();
        cand.truncate(cand.trim_end().len());
        cand.push('…');
        let (cw, ch) = measure(&cand);
        if cw == 0 || ch == 0 {
            break;
        }
        if cw as i32 <= avail {
            best = Some((cand, cw, ch));
            lo = mid + 1;
        } else if mid == 0 {
            break;
        } else {
            hi = mid - 1;
        }
    }
    best
}

/// f64 逻辑坐标 → `Rect`（`kanesumi_core::Rect` 的字段是 f32）。逻辑空间里的坐标是像素级
/// 整数 / 半整数，远在 f32 的 24 位精确整数范围内；分数倍率只影响物理换算（`px`），
/// 不在这一步取整。
fn rect_f32(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(x as f32, y as f32, w as f32, h as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试内复刻 `compositor/src/display.rs:257-266` 的 `scale_rect`（两端点取整），
    /// 只为把旧 `physical_geometry_scales_at_two` 的期望原样搬过来 —— 物理换算归合成器，
    /// 本控件不提供该 API。
    fn scale_rect(r: Rect, scale: f64) -> Rect {
        let x0 = (r.origin.x as f64 * scale).round();
        let y0 = (r.origin.y as f64 * scale).round();
        let x1 = ((r.origin.x + r.size.width) as f64 * scale).round();
        let y1 = ((r.origin.y + r.size.height) as f64 * scale).round();
        Rect::new(x0 as f32, y0 as f32, (x1 - x0).max(0.0) as f32, (y1 - y0).max(0.0) as f32)
    }

    #[test]
    fn default_metrics_match_compositor_config() {
        // 逐字段对齐 Ether `compositor/src/config.rs:31-46`（与 ssd.rs 的两处 px(8.0)）。
        let m = SsdChromeMetrics::default();
        assert_eq!(m.titlebar_h, 28.0);
        assert_eq!(m.btn_w, 44.0);
        assert_eq!(m.btn_h, 28.0);
        assert_eq!(m.title_pad_left, 12.0);
        assert_eq!(m.app_icon_gap, 8.0);
        assert_eq!(m.app_name_gap, 12.0);
        assert_eq!(m.sep_w, 1.0);
        assert_eq!(m.sep_gap, 8.0);
        assert_eq!(m.right_pad, 8.0);
        assert_eq!(m.divider, 1.0);
        assert_eq!(m.position_line, 2.0);
        // `new()` 与 `default()` 同一套值。
        assert_eq!(MetroSsdChrome::new(), MetroSsdChrome::default());
    }

    #[test]
    fn geometry_matches_spec() {
        // 28 pt 标题栏；三按钮 44×28 相邻、贴右缘；关闭最右。旧 `logical_geometry_matches_spec`。
        let r = MetroSsdChrome::new().geometry(
            Point::new(100.0, 200.0),
            0.0,
            Size::new(800.0, 600.0),
        );
        assert_eq!(r.titlebar.origin, Point::new(100.0, 172.0));
        assert_eq!(r.titlebar.size, Size::new(800.0, 28.0));
        assert_eq!(r.close.origin, Point::new(856.0, 172.0));
        assert_eq!(r.maximize.origin, Point::new(812.0, 172.0));
        assert_eq!(r.minimize.origin, Point::new(768.0, 172.0));
        for b in [r.close, r.maximize, r.minimize] {
            assert_eq!(b.size, Size::new(44.0, 28.0));
        }
    }

    #[test]
    fn geometry_applies_content_offset() {
        // geo_offset_x 是内容在 buffer 内的 X 偏移：几何整体右移，右缘随之右移（ssd.rs:101-103）。
        let r = MetroSsdChrome::new().geometry(
            Point::new(100.0, 200.0),
            7.0,
            Size::new(800.0, 600.0),
        );
        assert_eq!(r.titlebar.origin, Point::new(107.0, 172.0));
        assert_eq!(r.close.origin, Point::new(863.0, 172.0));
        assert_eq!(r.outer.origin, Point::new(107.0, 172.0));
    }

    #[test]
    fn physical_geometry_scales_at_two() {
        // 旧 `physical_geometry_scales_at_two`：逻辑几何按合成器的两端点取整放大 2×。
        let chrome = MetroSsdChrome::new();
        let logical = chrome.geometry(Point::new(100.0, 200.0), 0.0, Size::new(800.0, 600.0));
        let titlebar = scale_rect(logical.titlebar, 2.0);
        assert_eq!(titlebar.origin, Point::new(200.0, 344.0));
        assert_eq!(titlebar.size, Size::new(1600.0, 56.0));
        assert_eq!(scale_rect(logical.close, 2.0).size, Size::new(88.0, 56.0));
        // outer = 标题栏 + 内容（含 28pt 标题栏）。
        assert_eq!(logical.outer.size, Size::new(800.0, 628.0));
        let outer = scale_rect(logical.outer, 2.0);
        assert_eq!(outer.origin, Point::new(200.0, 344.0));
        assert_eq!(outer.size, Size::new(1600.0, 1256.0));
    }

    #[test]
    fn no_content_yields_empty_rects() {
        // configure 未回应（content = 0）→ 全 default，等待首帧。旧 `zero_content_yields_empty_rects`。
        let r = MetroSsdChrome::new().geometry(Point::new(10.0, 10.0), 0.0, Size::new(0.0, 100.0));
        assert_eq!(r, SsdGeometry::default());
        assert_eq!(r.titlebar.size, Size::new(0.0, 0.0));
        // 高为 0 或 NaN 同样不出几何（NaN 不满足 `> 0`）。
        for h in [0.0, -5.0, f32::NAN] {
            assert_eq!(
                MetroSsdChrome::new().geometry(Point::new(10.0, 10.0), 0.0, Size::new(800.0, h)),
                SsdGeometry::default()
            );
        }
    }

    #[test]
    fn border_and_line_thickness_scales_at_two() {
        // 1 px 分隔线 / 2 pt 定位线按输出 scale 换算：2× 下为 2 / 4 物理像素。
        // 否则 2× 上描边只有一半厚。旧 `border_and_line_thickness_scales_at_two`。
        let chrome = MetroSsdChrome::new();
        assert_eq!(chrome.border_thickness(1.0), (1.0, 2.0));
        assert_eq!(chrome.border_thickness(2.0), (2.0, 4.0));
        // 分数倍率：1.5× 下 divider 1.5→round 2、定位线 3；且任何倍率都不低于 1 px。
        assert_eq!(chrome.border_thickness(1.5), (2.0, 3.0));
        assert_eq!(chrome.border_thickness(0.0), (1.0, 1.0));
    }

    #[test]
    fn title_row_matches_measured_layout() {
        // 800 pt 标题栏（scale 1）：左距 12 → 图标 16 + 8 → 名称 63 + 12 → 分隔 1 + 8 → 标题，
        // 右界 = 右缘 − 三按钮 132 − 留白 8。数字取自 1× 无头实测（n1108_ssd/diagnosis.md 表一）。
        // 旧 `title_row_matches_measured_layout`。
        let tb = Rect::new(100.0, 52.0, 800.0, 28.0);
        let row = MetroSsdChrome::new().title_row(tb, 44.0, 1.0, 16.0, 63.0, true);
        assert_eq!(row.icon_x, 112.0);
        assert_eq!(row.name_x, 136.0);
        assert_eq!(row.sep_x, Some(211.0));
        assert_eq!(row.title_x, 220.0);
        assert_eq!(row.title_avail, 760.0 - 220.0);
        // 只有应用名、没有标题时不画分隔（不然留一根孤立竖线）。
        assert_eq!(MetroSsdChrome::new().title_row(tb, 44.0, 1.0, 16.0, 63.0, false).sep_x, None);
    }

    #[test]
    fn narrow_titlebar_leaves_no_room_for_title() {
        // 标题栏被三按钮与身份区吃满时，标题可用宽 ≤ 0 → 不画（而不是画到按钮底下）。
        // 旧 `narrow_titlebar_leaves_no_room_for_title`。
        let tb = Rect::new(100.0, 52.0, 240.0, 28.0);
        let row = MetroSsdChrome::new().title_row(tb, 44.0, 1.0, 16.0, 63.0, true);
        assert!(row.title_avail <= 0.0, "avail={}", row.title_avail);
        // 右界 100+240−132−8 = 200，标题起点 220 —— 起点已越过右界。
        assert!(row.title_x > 200.0, "title_x={}", row.title_x);
    }

    #[test]
    fn title_row_without_icon_starts_name_at_icon_slot() {
        // 无图标（icon_w = 0）时 name_x = icon_x，身份区不多占一段。
        let tb = Rect::new(0.0, 0.0, 800.0, 28.0);
        let row = MetroSsdChrome::new().title_row(tb, 44.0, 1.0, 0.0, 63.0, true);
        assert_eq!(row.icon_x, 12.0);
        assert_eq!(row.name_x, 12.0);
        assert_eq!(row.sep_x, Some(12.0 + 63.0 + 12.0));
    }

    #[test]
    fn title_row_scales_padding_at_fractional_scale() {
        // px(pt) = (pt*scale).round()：1.5× 下左距 12→18、图标间隔 8→12、名称间隔 12→18、
        // 分隔 1→max(1)=2（px(1)=2）、分隔后 8→12、右留白 8→12。旧实现逐项同一算式。
        let tb = Rect::new(0.0, 0.0, 800.0, 28.0);
        let row = MetroSsdChrome::new().title_row(tb, 44.0, 1.5, 16.0, 63.0, true);
        assert_eq!(row.icon_x, 18.0);
        assert_eq!(row.name_x, 18.0 + 16.0 + 12.0);
        assert_eq!(row.sep_x, Some(46.0 + 63.0 + 18.0));
        assert_eq!(row.title_x, 127.0 + 2.0 + 12.0);
        // titlebar 已是物理矩形（800 px 宽），右界不再乘 scale：800 − 132 − 12 = 656。
        assert_eq!(row.title_avail, 656.0 - 141.0);
    }

    #[test]
    fn over_forces_opaque_alpha() {
        // 不透明底叠上色：逐通道线性插值，输出 alpha 恒 1.0（ssd.rs:386-393）。
        let chrome = MetroSsdChrome::new();
        let base = Color::new(0.2, 0.4, 0.6, 0.5);
        let top = Color::new(0.9, 0.9, 0.9, 0.0);
        let half = chrome.over(base, top, 0.5);
        assert!((half.r - 0.55).abs() < 1e-6);
        assert!((half.g - 0.65).abs() < 1e-6);
        assert!((half.b - 0.75).abs() < 1e-6);
        assert_eq!(half.a, 1.0, "alpha 恒 1.0");
        // a = 0 保留底色 RGB、a = 1 取上色 RGB；两端也不把 alpha 带出来。
        assert_eq!(chrome.over(base, top, 0.0), Color::new(0.2, 0.4, 0.6, 1.0));
        assert_eq!(chrome.over(base, top, 1.0), Color::new(0.9, 0.9, 0.9, 1.0));
        // 现调用方传的悬停 / 按下比例（0.08 / 0.16）走同一式。
        let hover = chrome.over(Color::new(0.0, 0.0, 0.0, 0.0), Color::new(1.0, 1.0, 1.0, 0.0), 0.08);
        assert!((hover.r - 0.08).abs() < 1e-6);
        assert_eq!(hover.a, 1.0);
    }

    #[test]
    fn metrics_override_drives_geometry_and_lines() {
        // 合成器若改 config 值，几何与线宽跟着字段走，不需要改控件。
        let m = SsdChromeMetrics {
            titlebar_h: 32.0,
            btn_w: 40.0,
            btn_h: 30.0,
            position_line: 3.0,
            ..SsdChromeMetrics::default()
        };
        let chrome = MetroSsdChrome::with_metrics(m);
        let r = chrome.geometry(Point::new(0.0, 100.0), 0.0, Size::new(400.0, 200.0));
        assert_eq!(r.titlebar.origin, Point::new(0.0, 68.0));
        assert_eq!(r.titlebar.size, Size::new(400.0, 32.0));
        assert_eq!(r.close.origin, Point::new(360.0, 68.0));
        assert_eq!(r.close.size, Size::new(40.0, 30.0));
        assert_eq!(r.outer.size, Size::new(400.0, 232.0));
        assert_eq!(chrome.border_thickness(1.0), (1.0, 3.0));
    }
}


