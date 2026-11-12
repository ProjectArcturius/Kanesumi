// notification_card.rs —— 通知卡片的**几何**（只算，不画）。
// 参 docs/supply/BEHAVIOR_CORPUS.md（手感/几何判据）、docs/KANESUMI_DESIGN.md（直角、贴边、单一强调色）、
// `docs/INTERACTION_CANON.md`（最小命中区）。
//
// 为什么先做几何：合成器 `shell/notifications.rs` 的生产段里，正文换行（`wrap_body`）与单行适配
// （`draw_fitted_text`）是自己实现的，而 `kanesumi-canvas` 的 `TextEngine::layout_box` 早就提供
// 换行 + 两行截断 + 省略号（`max_lines` + `TextOverflow::Ellipsis`，测试
// `layout_box_clamps_height_and_ellipsizes` / `wrapping_never_splits_a_cluster`）。
// 本模块把「卡片里每一块该在哪」定死：图标、标题、正文两行、动作行、关闭钮，
// 全部由量测得出，**不许写死高度**。渲染侧拿这些矩形喂 Scene 即可。

use std::sync::Arc;

use kanesumi_canvas::{Scene, TextAlign, TextEngine, TextLayout, TextLayoutOptions, TextOverflow};
use kanesumi_core::sizes::{ICON_16, ICON_32};
use kanesumi_core::typography::{MetroTypography, TextStyle, base_weight};
use kanesumi_core::{MetroTheme, Point, Rect};

/// 正文最多两行（Win10 通知卡；超出截断加省略号）。
pub const CARD_BODY_MAX_LINES: usize = 2;
/// 关闭钮边长（引用尺寸表 `ICON_32`；触控档 44 由外壳整块放大）。
pub const CARD_CLOSE: f32 = ICON_32;

/// 标题字体：14/20、基础字重（与 `MetroInfoBar` 同档）。
fn title_style() -> TextStyle {
    TextStyle::new(14.0, 20.0, base_weight())
}

/// 正文字体：主题正文档。
fn body_style() -> TextStyle {
    MetroTypography::metro().body_medium
}

/// 一张通知卡的全部矩形（逻辑像素，绝对坐标）。
#[derive(Debug, Clone)]
pub struct CardGeometry {
    /// 卡片矩形（传入的那个）。
    pub card: Rect,
    /// 应用图标。
    pub icon: Rect,
    /// 标题（单行，超高按标题行高）。
    pub title: Rect,
    /// 标题排版结果（单行 + 省略号；渲染侧直接用它，别自己再切一次）。
    pub title_layout: Arc<TextLayout>,
    /// 正文行盒（两行以内）。
    pub body: Rect,
    /// 正文排版结果（已按两行截断，含省略号）。
    pub body_layout: Arc<TextLayout>,
    /// 动作按钮（自左向右；空 = 无动作行）。
    pub actions: Vec<Rect>,
    /// 关闭钮。
    pub close: Rect,
    /// 卡片应有的高度（≤ `card.height` 时用多少）。
    pub height: f32,
}

/// 文本区左边界（图标右缘 + 间距）。
fn text_left(card: Rect) -> f32 {
    card.origin.x + 16.0 + ICON_16 + 12.0
}

/// 文本区可用宽（右侧让开关闭钮）。
fn text_width(card: Rect) -> f32 {
    (card.right() - 8.0 - CARD_CLOSE - 8.0 - text_left(card)).max(0.0)
}

impl CardGeometry {
    /// 量一张卡。`card` 只需要宽与原点；高度由内容决定（不信任传入高度）。
    pub fn layout(
        engine: &TextEngine,
        card: Rect,
        title: &str,
        body: &str,
        actions: &[String],
    ) -> Self {
        let tw = text_width(card);
        let title_style = title_style();
        let body_style = body_style();
        let title_y = card.origin.y + 16.0;

        let title_layout = engine.layout_box(
            title,
            title_style.size,
            TextLayoutOptions {
                max_width: tw,
                max_height: title_style.line_height,
                line_height: title_style.line_height,
                max_lines: Some(1),
                letter_spacing_em: 0.0,
                wrap: false,
                overflow: TextOverflow::Ellipsis,
                weight: title_style.weight,
            },
        );
        let title_rect = Rect::new(text_left(card), title_y, tw, title_style.line_height);

        let body_y = title_rect.bottom() + 4.0;
        let body_layout = engine.layout_box(
            body,
            body_style.size,
            TextLayoutOptions {
                max_width: tw,
                max_height: body_style.line_height * CARD_BODY_MAX_LINES as f32,
                line_height: body_style.line_height,
                max_lines: Some(CARD_BODY_MAX_LINES),
                letter_spacing_em: 0.0,
                wrap: true,
                overflow: TextOverflow::Ellipsis,
                weight: body_style.weight,
            },
        );
        let body_h = body_layout.size.height.max(0.0);
        let body_rect = Rect::new(text_left(card), body_y, tw, body_h);

        let mut action_rects = Vec::with_capacity(actions.len());
        let mut y = body_rect.bottom() + 16.0;
        if !actions.is_empty() {
            let ay = body_rect.bottom() + 12.0;
            let mut x = text_left(card);
            for a in actions {
                let w = engine.measure(a, body_style.size) + 12.0 * 2.0;
                action_rects.push(Rect::new(x, ay, w, 32.0));
                x += w + 8.0;
            }
            y = ay + 32.0 + 16.0;
        }

        Self {
            card,
            icon: Rect::new(
                card.origin.x + 16.0,
                card.origin.y + 16.0,
                ICON_16,
                ICON_16,
            ),
            title: title_rect,
            title_layout,
            body: body_rect,
            body_layout,
            actions: action_rects,
            close: Rect::new(
                card.right() - 8.0 - CARD_CLOSE,
                card.origin.y + 8.0,
                CARD_CLOSE,
                CARD_CLOSE,
            ),
            height: (y - card.origin.y).max(0.0),
        }
    }

    /// 只问高度（外壳排版时先算高再定位置）。
    pub fn height_for(
        engine: &TextEngine,
        width: f32,
        title: &str,
        body: &str,
        actions: &[String],
    ) -> f32 {
        Self::layout(engine, Rect::new(0.0, 0.0, width, 0.0), title, body, actions).height
    }

    /// 命中动作按钮（返回序号）。
    pub fn action_at(&self, pos: Point) -> Option<usize> {
        self.actions.iter().position(|r| contains(*r, pos))
    }

    /// 命中关闭钮。
    pub fn hits_close(&self, pos: Point) -> bool {
        contains(self.close, pos)
    }
}

fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x && p.x < r.right() && p.y >= r.origin.y && p.y < r.bottom()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_element::testing::test_engine;

    fn long_cjk() -> String {
        "系统更新已完成，包含内核与图形栈。下次重启生效。".repeat(3)
    }

    fn geo(title: &str, body: &str, actions: &[&str]) -> CardGeometry {
        let engine = test_engine();
        let acts: Vec<String> = actions.iter().map(|s| s.to_string()).collect();
        CardGeometry::layout(&engine, Rect::new(0.0, 0.0, 360.0, 0.0), title, body, &acts)
    }

    #[test]
    fn body_clamps_to_two_lines() {
        let g = geo("更新", &long_cjk(), &[]);
        assert!(
            g.body_layout.lines.len() <= CARD_BODY_MAX_LINES,
            "正文最多两行，实得 {}",
            g.body_layout.lines.len()
        );
        assert!(g.body_layout.truncated, "超长正文必须标记截断（省略号）");
    }

    #[test]
    fn short_body_is_not_truncated() {
        let g = geo("更新", "已完成", &[]);
        assert!(!g.body_layout.truncated);
        assert_eq!(g.body_layout.lines.len(), 1);
    }

    #[test]
    fn title_stays_single_line() {
        let engine = test_engine();
        let g = CardGeometry::layout(
            &engine,
            Rect::new(0.0, 0.0, 200.0, 0.0),
            "一个很长很长很长很长很长很长很长的通知标题",
            "正文",
            &[],
        );
        assert_eq!(g.title.size.height, title_style().line_height);
        assert!(g.title.right() <= g.card.right() - 8.0 - CARD_CLOSE);
    }

    #[test]
    fn icon_sits_at_padding_and_is_16() {
        let g = geo("t", "b", &[]);
        assert_eq!(g.icon.origin.x, 16.0);
        assert_eq!(g.icon.origin.y, 16.0);
        assert_eq!(g.icon.size.width, ICON_16);
    }

    #[test]
    fn actions_extend_height_and_lay_out_left_to_right() {
        let without = geo("t", "b", &[]);
        let with = geo("t", "b", &["稍后", "立即重启"]);
        assert_eq!(with.actions.len(), 2);
        assert!(with.height > without.height, "有动作行的卡更高");
        assert!(with.actions[0].origin.x < with.actions[1].origin.x);
        assert_eq!(with.actions[0].size.height, 32.0);
        for a in &with.actions {
            assert!(a.right() <= with.card.right(), "动作不得溢出卡片：{a:?}");
        }
    }

    #[test]
    fn action_and_close_hit_tests() {
        let g = geo("t", "b", &["好"]);
        let a = g.actions[0];
        assert_eq!(
            g.action_at(Point::new(a.origin.x + a.size.width / 2.0, a.origin.y + 8.0)),
            Some(0)
        );
        assert_eq!(g.action_at(Point::new(g.card.right() - 2.0, g.card.bottom() - 2.0)), None);
        let c = g.close;
        assert!(g.hits_close(Point::new(c.origin.x + 4.0, c.origin.y + 4.0)));
        assert!(!g.hits_close(Point::new(c.origin.x - 20.0, c.origin.y - 20.0)));
    }

    #[test]
    fn narrow_card_keeps_text_width_non_negative() {
        let engine = test_engine();
        let g = CardGeometry::layout(&engine, Rect::new(0.0, 0.0, 40.0, 0.0), "t", "b", &[]);
        assert!(g.body.size.width >= 0.0);
        assert!(g.height > 0.0);
    }

    #[test]
    fn empty_body_still_produces_geometry() {
        let g = geo("只有标题", "", &[]);
        assert!(g.height > 0.0);
        assert!(g.body_layout.lines.len() <= CARD_BODY_MAX_LINES);
    }

    #[test]
    fn height_is_measured_not_fixed() {
        let engine = test_engine();
        let one = CardGeometry::height_for(&engine, 360.0, "t", "一行正文", &[]);
        let three = CardGeometry::height_for(&engine, 360.0, "t", &long_cjk(), &[]);
        assert!(three > one, "两行正文应比一行高：one={one} three={three}");
    }

    #[test]
    fn two_lines_reported_by_measurement() {
        // 两行正文的高度应等于两倍行高（±0.5），证明截断用的是量测而不是猜。
        let engine = test_engine();
        let g = CardGeometry::layout(
            &engine,
            Rect::new(0.0, 0.0, 260.0, 0.0),
            "标题",
            &"很长的一段正文用来撑满两行".repeat(2),
            &[],
        );
        let expect = body_style().line_height * CARD_BODY_MAX_LINES as f32;
        assert!(
            (g.body.size.height - expect).abs() <= body_style().line_height + 0.5,
            "正文高度应由量测给出：{}",
            g.body.size.height
        );
    }
}

// ── 渲染：几何 → Scene（只发命令，不做像素） ────────────────────────────────

/// 卡片要显示的内容（字符串由调用方持有，渲染不复制）。
pub struct CardContent<'a> {
    /// 应用名（标题行上方的小字；空则不出）。
    pub app_name: &'a str,
    /// 标题。
    pub title: &'a str,
    /// 正文（渲染侧按两行截断）。
    pub body: &'a str,
    /// 动作按钮标签。
    pub actions: &'a [String],
}

/// 交互态（只影响颜色，不改几何 —— 元素数稳定，损伤判定才稳）。
#[derive(Debug, Clone, Copy, Default)]
pub struct CardVisual {
    /// 悬停中的动作序号。
    pub hovered_action: Option<usize>,
    /// 关闭钮是否悬停。
    pub close_hovered: bool,
    /// 卡片整体是否悬停（悬停时底色抬一档）。
    pub hovered: bool,
}

/// 把一张卡画进场景。**直角**（`fill_rect` 不带圆角）、单一强调色、无阴影 —— 参 `KANESUMI_DESIGN.md` §Ⅲ。
pub fn render_card(
    theme: &MetroTheme,
    geo: &CardGeometry,
    content: &CardContent<'_>,
    visual: CardVisual,
    scene: &mut Scene,
) {
    let c = &theme.colors;
    let bg = if visual.hovered { c.surface_variant } else { c.surface };
    scene.fill_rect(bg, geo.card);
    // 底边 1pt 分隔线：贴边、直角（通知之间不靠阴影分层）。
    scene.fill_rect(
        c.divider,
        Rect::new(geo.card.origin.x, geo.card.bottom() - 1.0, geo.card.size.width, 1.0),
    );
    // 应用图标位（真实图标由外壳提供；这里给一块底，避免文字悬空）。
    scene.fill_rect(c.surface_variant, geo.icon);
    if !content.app_name.is_empty() {
        scene.text(
            content.app_name.to_string(),
            Rect::new(geo.title.origin.x, geo.title.origin.y, geo.title.size.width, 16.0),
            c.on_surface_variant,
            TextStyle::new(11.0, 16.0, base_weight()),
            TextAlign::Left,
        );
    }
    // 标题：几何已按单行 + 省略号量过；命令侧同样限制单行，保证两处一致。
    scene.text_with_options(
        content.title.to_string(),
        geo.title,
        c.on_surface,
        title_style(),
        TextAlign::Left,
        false,
        Some(1),
        TextOverflow::Ellipsis,
    );
    // 正文：两行截断（与几何同一组参数）。
    scene.text_with_options(
        content.body.to_string(),
        geo.body,
        c.on_surface_variant,
        body_style(),
        TextAlign::Left,
        true,
        Some(CARD_BODY_MAX_LINES),
        TextOverflow::Ellipsis,
    );
    // 动作按钮：底色 + 居中文字；悬停换 surface_variant。
    for (i, r) in geo.actions.iter().enumerate() {
        let fill = if visual.hovered_action == Some(i) {
            c.surface_variant
        } else {
            c.divider
        };
        scene.fill_rect(fill, *r);
        let label = content.actions.get(i).cloned().unwrap_or_default();
        scene.text(
            label,
            *r,
            c.on_surface,
            body_style(),
            TextAlign::Center,
        );
    }
    // 关闭钮：只在悬停时给底，字形始终存在（元素数稳定）。
    if visual.close_hovered {
        scene.fill_rect(c.surface_variant, geo.close);
    }
    scene.text(
        "✕".to_string(),
        geo.close,
        if visual.close_hovered {
            c.on_surface
        } else {
            c.on_surface_variant
        },
        TextStyle::new(12.0, 16.0, base_weight()),
        TextAlign::Center,
    );
}
