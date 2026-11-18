// notice_row.rs —— 通知行（MetroNoticeRow）：标题一行 + 正文换行 + 关闭叉 + 整行悬停 / 按下底。
//
// 缺口（Ether `docs/research/m1115_panels/REPORT.md` §四-2）：`action_center.rs::NoteRow` 是
// **四段卡片**（整行底 / 标题 / 最多两行正文 / 24 的关闭叉），旧 `MetroPanelRow` 只有
// 「单行主文字 + 同行右对齐次文字 + 右侧控件」，表示不了正文换行与独立的关闭命中。
// 本控件把这条行定死，正文的换行与省略**由控件做**（复用 `TextEngine::layout` 与实际字形量测，
// 不在应用里估字符宽度）。参 `docs/CONTROL_SPEC.md` §55.3。
//
// 关闭叉自绘（对角四三角形，与 `tab_view::draw_close_x` 同款）：Kanesumi 不假设 Segoe MDL2
// 存在，也不为了一个叉把 SVG 管线拖进控件；控件的图像输入只有调用方给的那一枚行首图标。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign, TextOverflow};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::state::ControlState;

// 行内几何全部来自 Ether `settings/src/topbar/action_center.rs::NoteRow` 的实测值：
// 左右 16、上 8、下 10（`.nt` 的 `padding: 8px 16px 10px`）、标题行高 20（`body_medium`）、
// 标题到正文 2（`.nt .b` 的 `margin-top`）、正文行高 18（`caption`）、叉命中 24 且上探 2、
// 标题到叉 8（旧代码是标题到时间列 8，时间列没进本控件）、叉图形 12。
//
// **不立 `const f32`**：尺寸的定义归 `kanesumi_core::sizes`，而
// `kanesumi-controls/tests/size_discipline.rs` 的白名单只减不增（本卡不改 core）。
// 这些数在 `measure` / `render` 里是具名局部量，出处就在上面这行注释。
// 参 `docs/CONTROL_SPEC.md` §55.3。

/// 正文缺省行数上限（任务书 §2：最多两行）。行数是计数不是尺寸，留作公开常量。
pub const NOTICE_MAX_LINES: usize = 2;

/// 通知行命中：关闭叉**优先**（点叉只关这一条，不顺带触发整行）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoticeHit {
    /// 整行（正文 / 标题 / 空白处）。
    Row,
    /// 关闭叉。
    Close,
}

/// 一条通知行。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroNoticeRow {
    title: String,
    body: String,
    max_lines: usize,
    close_visible: bool,
    hovered: bool,
    close_hovered: bool,
    pressed: bool,
}

impl MetroNoticeRow {
    /// 建一条通知：缺省正文最多 2 行、关闭叉可见。
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            max_lines: NOTICE_MAX_LINES,
            close_visible: true,
            hovered: false,
            close_hovered: false,
            pressed: false,
        }
    }

    /// 正文行数上限。`0` = 收起（只留标题行，行高 38），与旧 `NoteRow::collapsed` 同义。
    pub fn with_max_lines(mut self, lines: usize) -> Self {
        self.max_lines = lines;
        self
    }

    /// 关闭叉是否可见。关掉后**不占位**：标题宽度直接吃到右侧内距。
    pub fn with_close(mut self, visible: bool) -> Self {
        self.close_visible = visible;
        self
    }

    /// 标题。
    pub fn title(&self) -> &str {
        &self.title
    }

    /// 正文。
    pub fn body(&self) -> &str {
        &self.body
    }

    /// 行数上限。
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// 关闭叉是否可见。
    pub fn close_visible(&self) -> bool {
        self.close_visible
    }

    /// 悬停（整行）。
    pub fn set_hovered(&mut self, on: bool) {
        self.hovered = on;
    }

    /// 指针在关闭叉上（只提亮叉，不动行底）。
    pub fn set_close_hovered(&mut self, on: bool) {
        self.close_hovered = on;
    }

    /// 按下（整行）。
    pub fn set_pressed(&mut self, on: bool) {
        self.pressed = on;
    }

    /// 当前状态：按下 > 悬停 > 常态（与 `ControlState` 的通用优先级一致）。
    pub fn state(&self) -> ControlState {
        if self.pressed {
            ControlState::Pressed
        } else if self.hovered {
            ControlState::Hovered
        } else {
            ControlState::Normal
        }
    }

    /// 空输入：标题与正文都没有墨迹。此时不画任何东西（连悬停底也不画），
    /// 但行高仍按「只有标题」算，元素数目不变。
    pub fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.body.trim().is_empty()
    }

    /// 量一行。`width` 给可用宽，高由「内距 + 标题行 + 正文实际行数」得出（**不写死**）。
    pub fn measure(&self, engine: &TextEngine, origin: Point, width: f32) -> NoticeRowGeometry {
        let width = width.max(0.0);
        let (pad, top, bottom) = (16.0, 8.0, 10.0);
        let (title_h, gap, body_h) = (20.0, 2.0, 18.0);
        let (close_side, close_top, close_gap) = (24.0, 6.0, 8.0);
        let close = self.close_visible.then(|| {
            Rect::new(
                origin.x + width - pad - close_side,
                origin.y + close_top,
                close_side,
                close_side,
            )
        });
        let title_x = origin.x + pad;
        let title_right = match close {
            Some(c) => c.origin.x - close_gap,
            None => origin.x + width - pad,
        };
        let title = Rect::new(
            title_x,
            origin.y + top,
            (title_right - title_x).max(0.0),
            title_h,
        );
        let body_w = (width - 2.0 * pad).max(0.0);
        let lines = self.body_lines(engine, body_w);
        let body = (lines > 0).then(|| {
            Rect::new(
                origin.x + pad,
                origin.y + top + title_h + gap,
                body_w,
                body_h * lines as f32,
            )
        });
        let height = top + title_h + body.map_or(0.0, |b| gap + b.size.height) + bottom;
        NoticeRowGeometry {
            row: Rect::new(origin.x, origin.y, width, height),
            title,
            body,
            close,
        }
    }

    /// 画一行：行底 → 标题 → 正文 → 关闭叉。逐段矩形由 [`NoticeRowGeometry::damage`] 给。
    pub fn render(&self, theme: &MetroTheme, geo: &NoticeRowGeometry, scene: &mut Scene) {
        if self.is_empty() {
            return;
        }
        match self.state() {
            ControlState::Pressed => scene.fill_rect(theme.indication.press_tint, geo.row),
            ControlState::Hovered => scene.fill_rect(theme.indication.hover_tint, geo.row),
            _ => {}
        }
        scene.label(
            self.title.clone(),
            geo.title,
            theme.colors.on_surface,
            title_style(theme),
            TextAlign::Left,
        );
        if let Some(body) = geo.body {
            scene.text_with_options(
                self.body.clone(),
                body,
                theme.colors.on_surface_variant,
                body_style(theme),
                TextAlign::Left,
                true,
                Some(self.max_lines),
                TextOverflow::Ellipsis,
            );
        }
        if let Some(close) = geo.close {
            let tint = if self.close_hovered {
                theme.colors.on_surface
            } else {
                theme.colors.on_surface_variant
            };
            draw_close_x(scene, close, tint);
        }
    }

    /// 正文字数出来的行数：空正文 / 0 行上限 / 零宽 => 0。行数按 `caption` 与**实际字形**量测。
    fn body_lines(&self, engine: &TextEngine, width: f32) -> usize {
        if self.max_lines == 0 || self.body.trim().is_empty() || width <= 0.0 {
            return 0;
        }
        let lines = engine.layout(&self.body, measure_style().size, width).len();
        lines.clamp(1, self.max_lines)
    }
}

/// 通知行几何（绝对坐标）。绘制、命中、逐段损伤消费同一份。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoticeRowGeometry {
    /// 整行盒（行底）。
    pub row: Rect,
    /// 标题行盒。
    pub title: Rect,
    /// 正文盒（无正文 = `None`，不占位）。
    pub body: Option<Rect>,
    /// 关闭叉命中盒（叉隐藏 = `None`，不占位）。
    pub close: Option<Rect>,
}

impl NoticeRowGeometry {
    /// 整行盒。
    pub fn rect(&self) -> Rect {
        self.row
    }

    /// 标题行盒。
    pub fn title_rect(&self) -> Rect {
        self.title
    }

    /// 关闭叉命中盒。
    pub fn close_rect(&self) -> Option<Rect> {
        self.close
    }

    /// 正文盒。
    pub fn body_rect(&self) -> Option<Rect> {
        self.body
    }

    /// 逐段损伤矩形（行底 / 标题 / 正文 / 叉），跳过空段与零尺寸段。
    ///
    /// 行底与标题 / 正文天然重叠 —— 这不是缺陷：行底是整块背景，按段损伤只在**合并**时
    /// 去重，不在这里把行底切成三块（切了反而会让「悬停只重绘行底」变成三次填充）。
    pub fn damage(&self) -> Vec<Rect> {
        std::iter::once(self.row)
            .chain(std::iter::once(self.title))
            .chain(self.body)
            .chain(self.close)
            .filter(|r| r.size.width > 0.0 && r.size.height > 0.0)
            .collect()
    }

    /// 命中：关闭叉优先，其次整行。
    pub fn hit(&self, pos: Point) -> Option<NoticeHit> {
        if let Some(close) = self.close
            && contains(close, pos)
        {
            return Some(NoticeHit::Close);
        }
        contains(self.row, pos).then_some(NoticeHit::Row)
    }
}

fn title_style(theme: &MetroTheme) -> TextStyle {
    theme.typography.body_medium
}

fn body_style(theme: &MetroTheme) -> TextStyle {
    theme.typography.caption
}

/// 量测用的正文样式：主题的排版表恒为 `MetroTypography::metro()`（`MetroTheme` 的所有构造器
/// 都写死它），`measure` 拿不到主题时按同一份取，与 `render` 不会漂。
fn measure_style() -> TextStyle {
    MetroTypography::metro().caption
}

/// 自绘 ×（对角四三角形，与 `tab_view::draw_close_x` 同一配方）。
///
/// 为什么在这里复制一份：`tab_view` 的同名函数是私有实现，为一个叉把它提成公共 API 会扩大
/// 既有控件的接口面（本卡不改既有控件）。
fn draw_close_x(scene: &mut Scene, rect: Rect, color: kanesumi_core::Color) {
    let center = rect.center();
    let r = 6.0; // 叉图形 12，半径 6（命中盒 24 由 measure 给）
    let t = 1.6;
    let (cx, cy) = (center.x, center.y);
    scene.triangle(
        Point::new(cx - r, cy - r + t),
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r - t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx - r + t, cy - r),
        Point::new(cx + r, cy + r - t),
        Point::new(cx + r - t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx + r - t, cy - r),
        Point::new(cx + r, cy - r + t),
        Point::new(cx - r + t, cy + r),
        color,
    );
    scene.triangle(
        Point::new(cx - r, cy + r - t),
        Point::new(cx + r, cy - r + t),
        Point::new(cx + r - t, cy - r),
        color,
    );
}

fn contains(r: Rect, p: Point) -> bool {
    r.size.width > 0.0
        && r.size.height > 0.0
        && p.x >= r.origin.x
        && p.x < r.right()
        && p.y >= r.origin.y
        && p.y < r.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    /// 字体引擎：缺字体时跳过（与库内既有用例同法；本机装了 DejaVu）。
    fn engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT")
            && let Ok(e) = TextEngine::load(p)
        {
            return Some(e);
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    /// 一行装得下的正文（3 个汉字 ≈ 39 pt，远小于正文宽 328）。
    const SHORT: &str = "已保存";

    /// 两行以上的正文（7 汉字 × 8 遍 ≈ 728 pt，超过正文宽 328）。
    fn long_body() -> String {
        "构建完成的正文。".repeat(8)
    }

    #[test]
    fn two_line_row_is_76_one_line_is_58_and_collapsed_is_38() {
        let Some(engine) = engine() else { return };
        // 8（上）+ 20（标题）+ 2（间隔）+ 18 × 2 + 10（下）。
        let two = MetroNoticeRow::new("构建完成", long_body()).measure(
            &engine,
            Point::new(0.0, 0.0),
            360.0,
        );
        assert_eq!(two.rect().size.height, 76.0);
        assert_eq!(two.body_rect().unwrap().size.height, 36.0);
        // 8 + 20 + 2 + 18 + 10 —— 与 C0 转录的「展开 58」同值。
        let one =
            MetroNoticeRow::new("构建完成", SHORT).measure(&engine, Point::new(0.0, 0.0), 360.0);
        assert_eq!(one.rect().size.height, 58.0);
        assert_eq!(one.body_rect().unwrap().size.height, 18.0);
        // 8 + 20 + 10 —— 与 C0 转录的「收起 38」同值。
        let collapsed = MetroNoticeRow::new("构建完成", long_body())
            .with_max_lines(0)
            .measure(&engine, Point::new(0.0, 0.0), 360.0);
        assert_eq!(collapsed.rect().size.height, 38.0);
        assert_eq!(collapsed.body_rect(), None);
    }

    #[test]
    fn segments_match_the_app_constants() {
        let Some(engine) = engine() else { return };
        let row = MetroNoticeRow::new("构建完成", long_body());
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);
        // 行底 (0,0,360,76)；标题 x 16、y 8、宽 = 320 − 8 − 16 = 296、行高 20；
        // 叉 (320,6,24,24)；正文 x 16、y 30、宽 328、两行 36。
        assert_eq!(geo.rect(), Rect::new(0.0, 0.0, 360.0, 76.0));
        assert_eq!(geo.title_rect(), Rect::new(16.0, 8.0, 296.0, 20.0));
        assert_eq!(geo.close_rect(), Some(Rect::new(320.0, 6.0, 24.0, 24.0)));
        assert_eq!(geo.body_rect(), Some(Rect::new(16.0, 30.0, 328.0, 36.0)));
        // 逐段损伤：行底 / 标题 / 正文 / 叉四段，顺序固定。
        assert_eq!(
            geo.damage(),
            vec![
                Rect::new(0.0, 0.0, 360.0, 76.0),
                Rect::new(16.0, 8.0, 296.0, 20.0),
                Rect::new(16.0, 30.0, 328.0, 36.0),
                Rect::new(320.0, 6.0, 24.0, 24.0),
            ]
        );
    }

    #[test]
    fn hidden_close_reclaims_the_title_width() {
        let Some(engine) = engine() else { return };
        let row = MetroNoticeRow::new("构建完成", SHORT).with_close(false);
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);
        // 叉不占位：标题右缘从 320 − 8 扩到 360 − 16 = 344，宽 344 − 16 = 328。
        assert_eq!(geo.close_rect(), None);
        assert_eq!(geo.title_rect(), Rect::new(16.0, 8.0, 328.0, 20.0));
        assert_eq!(geo.damage().len(), 3, "行底 / 标题 / 正文，无叉段");
    }

    #[test]
    fn hit_prefers_close_then_row() {
        let Some(engine) = engine() else { return };
        let geo =
            MetroNoticeRow::new("构建完成", SHORT).measure(&engine, Point::new(0.0, 0.0), 360.0);
        assert_eq!(
            geo.hit(Point::new(332.0, 18.0)),
            Some(NoticeHit::Close),
            "叉内优先 Close"
        );
        assert_eq!(
            geo.hit(Point::new(320.0, 6.0)),
            Some(NoticeHit::Close),
            "叉左上角含"
        );
        assert_eq!(geo.hit(Point::new(100.0, 18.0)), Some(NoticeHit::Row));
        assert_eq!(geo.hit(Point::new(400.0, 18.0)), None, "行外");
        assert_eq!(geo.hit(Point::new(100.0, 58.0)), None, "行底之下");
        // 叉隐藏时同一个点落到整行。
        let no_close = MetroNoticeRow::new("构建完成", SHORT)
            .with_close(false)
            .measure(&engine, Point::new(0.0, 0.0), 360.0);
        assert_eq!(no_close.hit(Point::new(332.0, 18.0)), Some(NoticeHit::Row));
    }

    #[test]
    fn hover_and_press_paint_the_row_background() {
        let Some(engine) = engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroNoticeRow::new("构建完成", SHORT);
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);

        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { .. })),
            "静置不铺行底（C0 首行底色 = 面板底）"
        );

        row.set_hovered(true);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        let first = &scene.commands[0];
        assert_eq!(
            first,
            &SceneCommand::FillRect {
                color: theme.indication.hover_tint,
                rect: geo.rect(),
                corner_radius: 0.0,
            }
        );

        row.set_pressed(true);
        assert_eq!(row.state(), ControlState::Pressed);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        assert_eq!(
            scene.commands[0],
            SceneCommand::FillRect {
                color: theme.indication.press_tint,
                rect: geo.rect(),
                corner_radius: 0.0,
            },
            "按下压过悬停"
        );
    }

    #[test]
    fn body_wraps_with_ellipsis_and_max_lines() {
        let Some(engine) = engine() else { return };
        let theme = MetroTheme::ether_dark();
        let row = MetroNoticeRow::new("构建完成", long_body());
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        let body: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text {
                    content,
                    wrap,
                    max_lines,
                    ..
                } if content.starts_with("构建完成") && *wrap => Some((*max_lines, *wrap)),
                _ => None,
            })
            .collect();
        assert_eq!(body, vec![(Some(2), true)], "正文换行 + 上限 2 行");
    }

    #[test]
    fn close_glyph_is_four_triangles_and_brightens_on_hover() {
        let Some(engine) = engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroNoticeRow::new("构建完成", SHORT);
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        let xs: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Triangle { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert_eq!(xs.len(), 4, "叉 = 对角四三角形");
        assert!(xs.iter().all(|c| *c == theme.colors.on_surface_variant));

        row.set_close_hovered(true);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        let xs: Vec<_> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Triangle { color, .. } => Some(*color),
                _ => None,
            })
            .collect();
        assert!(
            xs.iter().all(|c| *c == theme.colors.on_surface),
            "悬停叉提亮到主墨色"
        );
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { .. })),
            "悬停在叉上不动行底"
        );
    }

    #[test]
    fn empty_row_paints_nothing_but_keeps_its_slot() {
        let Some(engine) = engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroNoticeRow::new("  ", "");
        let geo = row.measure(&engine, Point::new(0.0, 0.0), 360.0);
        assert_eq!(geo.rect().size.height, 38.0, "空行仍占「只有标题」的高度");
        row.set_hovered(true);
        let mut scene = Scene::default();
        row.render(&theme, &geo, &mut scene);
        assert!(scene.commands.is_empty(), "空输入连悬停底也不画");
    }

    #[test]
    fn zero_width_has_no_body_but_still_measures() {
        let Some(engine) = engine() else { return };
        let geo = MetroNoticeRow::new("构建完成", long_body()).measure(
            &engine,
            Point::new(0.0, 0.0),
            20.0,
        );
        assert_eq!(geo.body_rect(), None, "宽度吃不下正文时不产正文段");
        assert_eq!(geo.rect().size.height, 38.0);
        assert_eq!(geo.title_rect().size.width, 0.0, "标题宽夹到 0 而不是负数");
    }
}
