// notification_card.rs —— 通知卡片（迁移版 `MetroNotificationCard`）。
//
// 几何逐值抄 Ether 合成器 `compositor/src/shell/notifications.rs`，正文行由调用方切好，
// 控件不量测、不换行。依据 docs/research/n1112_element_bridge/NOTIFY_PLAN.md §二 步骤 1；
// 规格见 `docs/CONTROL_SPEC.md` §56，映射与常量出处见
// `docs/research/m1115_notify/REPORT.md`。
//
// 历史上同文件还有一份「量测版」（`CardGeometry` / `render_card`，把换行与省略交给
// `TextEngine`），且它写在测试模块**之后**，从未被静态检查扫过。m1115_cleanup 已整体删除，
// 只留本实现；删因与证据见 `docs/research/m1115_cleanup/REPORT.md` §一。

use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::typography::TextVAlign;
use kanesumi_core::{Color, MetroTheme, Point, Rect, ThemeColor};

fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.origin.x && p.x < r.right() && p.y >= r.origin.y && p.y < r.bottom()
}

// ── MetroNotificationCard（k-notify-card） ──────────────────────────────────
//
// 几何逐值抄 Ether `compositor/src/shell/notifications.rs`，正文行由调用方切好，
// 控件不量测、不换行（参 NOTIFY_PLAN §二 步骤 1）。步骤 2 把换行交给 canvas 后，
// 本卡才补上换行与省略。
//
// 常量用 f64：源文件就是 f64，逐值对照不必往返转换。它们是**迁移契约**（步骤 2/3 会整块
// 删除），不是 Kanesumi 的新设计尺寸，所以不登记 `kanesumi_core::sizes`；`size_discipline`
// 只拦 f32 裸尺寸，这一点在 m1115_notify/REPORT.md 里如实记录，交调度者裁定。
// 参 docs/CONTROL_SPEC.md §56。

/// 卡片内边距（四边同值）—— 参 notifications.rs:42。
const NOTIFY_PAD: f64 = 12.0;
/// 左侧图片边长 —— 参 notifications.rs:44。
const NOTIFY_IMAGE_SIZE: f64 = 48.0;
/// 图片列与文字的间距 —— 参 notifications.rs:45。
const NOTIFY_IMAGE_GAP: f64 = 12.0;
/// 标题 / 正文行高 —— 参 notifications.rs:48。
const NOTIFY_LINE_H: f64 = 22.0;
/// 应用名行高 —— 参 notifications.rs:49。
const NOTIFY_APP_LINE_H: f64 = 16.0;
/// 正文最多行数（超出的行不进几何）—— 参 notifications.rs:52。
const NOTIFY_BODY_MAX_LINES: usize = 3;
/// 按钮高度 —— 参 notifications.rs:53。
const NOTIFY_BTN_H: f64 = 28.0;
/// 按钮间距 —— 参 notifications.rs:54。
const NOTIFY_BTN_GAP: f64 = 8.0;
/// 正文末行与按钮行的间距 —— 参 notifications.rs:55。
const NOTIFY_BTN_TOP_GAP: f64 = 10.0;
/// 按钮最多个数 —— 参 notifications.rs:56。
const NOTIFY_BTN_MAX: usize = 3;
/// 关闭叉命中边长 —— 参 notifications.rs:58。
const NOTIFY_CLOSE_SIZE: f64 = 18.0;
/// 关闭叉距卡片右上角的内缩 —— 参 notifications.rs:59。
const NOTIFY_CLOSE_INSET: f64 = 6.0;
/// 1 px 描边 —— 参 notifications.rs:1930-1938（draw_border 的四条边）。
const NOTIFY_BORDER: f64 = 1.0;
/// 源 `rect_from` 的最小边长（宽 / 高夹到至少 1）—— 参 notifications.rs:1613-1618。
const NOTIFY_MIN_EXTENT: f64 = 1.0;
/// 卡片底色不透明度：源 `notification_bg = surface_variant @ 0.97` —— 参 theme.rs:191。
const NOTIFY_BG_ALPHA: f32 = 0.97;
/// 按钮常态底：文字色叠 6% —— 参 notifications.rs:2064。
const NOTIFY_BTN_TINT: f32 = 0.06;
/// 按钮悬停底：文字色叠 12% —— 参 notifications.rs:2062-2064。
const NOTIFY_BTN_TINT_HOVER: f32 = 0.12;
const NOTIFY_BTN_TINT_PRESS: f32 = 0.18;
/// 关闭叉悬停底：文字色叠 12% —— 参 notifications.rs:2072-2076。
const NOTIFY_CLOSE_TINT: f32 = 0.12;

/// 一张通知卡的内容。只在 `new` 时借一次，控件把字段复制进卡片。
pub struct NotificationContent<'a> {
    /// 应用名（空 = 不出应用名行，也不占行高）。
    pub app_name: &'a str,
    /// 标题（空 = 不出标题行，也不占行高）。
    pub summary: &'a str,
    /// 已切好的正文行（本卡不做换行；换行仍在合成器，步骤 2 才迁到 canvas）。
    pub body_lines: &'a [String],
    /// 动作按钮标签（超过 3 个的部分不进几何）。
    pub buttons: &'a [&'a str],
    /// 是否有左侧图片（只影响文字列左移 48 + 12，不画图）。
    pub has_image: bool,
}

/// 通知卡命中目标。优先级：关闭叉 > 动作按钮 > 卡体。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotifyHit {
    /// 卡体（标题 / 正文 / 空白处）。
    Body,
    /// 右上角关闭叉。
    Close,
    /// 第 `i` 个动作按钮。
    Action(usize),
}

/// 一张通知卡的全部矩形（逻辑像素，**以卡左上为原点**）。
#[derive(Debug, Clone, PartialEq)]
pub struct NotifyCardGeometry {
    /// 卡体。
    pub card: Rect,
    /// 左侧图片位（无图 = `None`，文字列不左移）。
    pub icon: Option<Rect>,
    /// 应用名行（空应用名 = `None`，不占行高）。
    pub app: Option<Rect>,
    /// 标题行；空标题时为零高矩形（不占行高，槽位仍在）。
    pub title: Rect,
    /// 正文整体盒（行数为 0 时高度 0；逐行矩形以 `body_lines` 为准）。
    pub body: Rect,
    /// 正文逐行矩形（最多 3 行，每行高 22）。
    pub body_lines: Vec<Rect>,
    /// 关闭叉命中盒（18 × 18）。
    pub close: Rect,
    /// 动作按钮（最多 3 个，自左向右等分）。
    pub buttons: Vec<Rect>,
}

/// 通知卡四个文字槽的样式。几何沿用卡片原来的行盒。
#[derive(Clone, Copy)]
pub struct NotificationTextStyles {
    pub app: TextStyle,
    pub title: TextStyle,
    pub body: TextStyle,
    pub action: TextStyle,
}

/// 一张通知卡：内容 + 悬停态。几何与命中是纯函数，渲染只消费几何。
///
/// 内容在 `new` 时复制进控件：卡片要活过调用方的借期（外壳按通知 id 缓存一张卡，
/// 模型变更时再重建），把生命周期参数留在消费者身上没有收益。
pub struct MetroNotificationCard {
    app_name: String,
    summary: String,
    body_lines: Vec<String>,
    buttons: Vec<String>,
    has_image: bool,
    hover: Option<NotifyHit>,
    pressed_action: Option<usize>,
}

impl MetroNotificationCard {
    /// 建一张卡：内容复制进控件，悬停态从「无」开始。
    pub fn new(content: NotificationContent<'_>) -> Self {
        Self {
            app_name: content.app_name.to_string(),
            summary: content.summary.to_string(),
            body_lines: content.body_lines.to_vec(),
            buttons: content.buttons.iter().map(|s| (*s).to_string()).collect(),
            has_image: content.has_image,
            hover: None,
            pressed_action: None,
        }
    }

    /// 给定可用宽 → 卡高与全部子矩形（**以卡左上为原点**，逻辑像素）。
    ///
    /// 源 `layout_card`（notifications.rs:1859-1917）逐值等价：宽 / 高先按源
    /// `rect_from`（:1613）舍入成整数逻辑像素，再推子矩形。可见宽与 `CARD_W`
    /// （`config::NOTIFICATION_WIDTH = 320`）同值时，结果与合成器现有几何逐值相同。
    pub fn geometry(&self, width: f64) -> NotifyCardGeometry {
        let card_w = width.max(0.0).round().max(NOTIFY_MIN_EXTENT);
        let card_h = self.card_height().round().max(NOTIFY_MIN_EXTENT);
        let has_image = self.has_image;
        let ext = if has_image {
            NOTIFY_IMAGE_SIZE + NOTIFY_IMAGE_GAP
        } else {
            0.0
        };
        let text_x = NOTIFY_PAD + ext;
        let text_w = (card_w - NOTIFY_PAD - ext - NOTIFY_PAD).max(NOTIFY_MIN_EXTENT);
        let icon = has_image
            .then(|| px_rect(NOTIFY_PAD, NOTIFY_PAD, NOTIFY_IMAGE_SIZE, NOTIFY_IMAGE_SIZE));

        // 源 :1860-1888：应用名 / 标题各占一行，空串不占行高。
        let mut cy = NOTIFY_PAD;
        let app = if self.app_name.is_empty() {
            None
        } else {
            let r = px_rect(text_x, cy, text_w, NOTIFY_APP_LINE_H);
            cy += NOTIFY_APP_LINE_H;
            Some(r)
        };
        let title = if self.summary.is_empty() {
            // 零高矩形：标题槽位仍在（render 恒定发一条命令），但不占行高。
            Rect::new(text_x as f32, cy as f32, text_w as f32, 0.0)
        } else {
            let r = px_rect(text_x, cy, text_w, NOTIFY_LINE_H);
            cy += NOTIFY_LINE_H;
            r
        };

        let line_count = self.body_lines.len().min(NOTIFY_BODY_MAX_LINES);
        let body_top = cy;
        let mut body_lines = Vec::with_capacity(line_count);
        for _ in 0..line_count {
            body_lines.push(px_rect(text_x, cy, text_w, NOTIFY_LINE_H));
            cy += NOTIFY_LINE_H;
        }
        let body = Rect::new(
            text_x as f32,
            body_top as f32,
            text_w as f32,
            (NOTIFY_LINE_H * line_count as f64) as f32,
        );

        // 源 :1889-1898：按钮自左向右等分，贴卡底（卡高在源里是已舍入的 rh）。
        let button_count = self.buttons.len().min(NOTIFY_BTN_MAX);
        let mut buttons = Vec::with_capacity(button_count);
        if button_count > 0 {
            let area_w = card_w - NOTIFY_PAD * 2.0;
            let gaps = button_count.saturating_sub(1) as f64;
            let bw = (area_w - NOTIFY_BTN_GAP * gaps) / button_count as f64;
            let by = card_h - NOTIFY_PAD - NOTIFY_BTN_H;
            for k in 0..button_count {
                buttons.push(px_rect(
                    NOTIFY_PAD + (bw + NOTIFY_BTN_GAP) * k as f64,
                    by,
                    bw,
                    NOTIFY_BTN_H,
                ));
            }
        }

        let close = px_rect(
            card_w - NOTIFY_CLOSE_INSET - NOTIFY_CLOSE_SIZE,
            NOTIFY_CLOSE_INSET,
            NOTIFY_CLOSE_SIZE,
            NOTIFY_CLOSE_SIZE,
        );
        NotifyCardGeometry {
            card: Rect::new(0.0, 0.0, card_w as f32, card_h as f32),
            icon,
            app,
            title,
            body,
            body_lines,
            close,
            buttons,
        }
    }

    /// 悬停态。只改颜色，不改几何 —— 元素数目恒定，损伤判定才稳。
    pub fn set_hover(&mut self, hit: Option<NotifyHit>) {
        self.hover = hit;
    }

    /// 按下的动作序号。只有指针仍在同一按钮上时才显示按下反馈。
    pub fn set_pressed_action(&mut self, index: Option<usize>) {
        self.pressed_action = index;
    }

    /// 命中：关闭叉 > 动作按钮 > 卡体；卡外一律 `None`（源 hit :2146-2163 同序）。
    pub fn hit(&self, g: &NotifyCardGeometry, p: Point) -> Option<NotifyHit> {
        if !contains(g.card, p) {
            return None;
        }
        if contains(g.close, p) {
            return Some(NotifyHit::Close);
        }
        if let Some(i) = g.buttons.iter().position(|r| contains(*r, p)) {
            return Some(NotifyHit::Action(i));
        }
        Some(NotifyHit::Body)
    }

    /// 卡底 / 描边 / 应用名 / 标题 / 正文逐行 / 按钮底与标签 / 关闭叉。
    ///
    /// 每个槽位**恒定发命令**（空标题也发空文字），悬停只换颜色不增删 —— 与 SSD 按钮底同款。
    /// 颜色全部经 `ThemeColor` 令牌；卡片底色 0.97 与按钮叠色 0.06 / 0.12 / 0.18 是源配方常量。
    pub fn render(&self, g: &NotifyCardGeometry, theme: &MetroTheme, scene: &mut Scene) {
        self.render_card(
            g,
            theme,
            scene,
            NotificationTextStyles {
                app: theme.typography.body_small,
                title: theme.typography.base,
                body: theme.typography.body,
                action: theme.typography.body_small.with_v_align(TextVAlign::Center),
            },
            TextOverflow::Ellipsis,
        );
    }

    /// 使用调用方的字号。正文已经按目标字体折行，按原行裁剪，避免再次加省略号。
    pub fn render_with_text_styles(
        &self,
        g: &NotifyCardGeometry,
        theme: &MetroTheme,
        scene: &mut Scene,
        styles: NotificationTextStyles,
    ) {
        self.render_card(g, theme, scene, styles, TextOverflow::Clip);
    }

    fn render_card(
        &self,
        g: &NotifyCardGeometry,
        theme: &MetroTheme,
        scene: &mut Scene,
        styles: NotificationTextStyles,
        body_overflow: TextOverflow,
    ) {
        let bg = ThemeColor::SurfaceVariant
            .resolve(theme)
            .with_alpha(NOTIFY_BG_ALPHA);
        let text = ThemeColor::OnSurface.resolve(theme);
        let secondary = ThemeColor::OnSurfaceVariant.resolve(theme);
        let divider = ThemeColor::Divider.resolve(theme);

        scene.fill_rect(bg, g.card);
        draw_border(scene, g.card, divider);
        if let Some(app) = g.app {
            scene.label(
                self.app_name.clone(),
                app,
                secondary,
                styles.app,
                TextAlign::Left,
            );
        }
        scene.label(
            self.summary.clone(),
            g.title,
            text,
            styles.title,
            TextAlign::Left,
        );
        for (rect, line) in g.body_lines.iter().zip(self.body_lines.iter()) {
            scene.text_with_options(
                line.clone(),
                *rect,
                text,
                styles.body,
                TextAlign::Left,
                false,
                Some(1),
                body_overflow,
            );
        }
        for (i, rect) in g.buttons.iter().enumerate() {
            let tint = match (self.hover == Some(NotifyHit::Action(i)), self.pressed_action == Some(i)) {
                (true, true) => NOTIFY_BTN_TINT_PRESS,
                (true, false) => NOTIFY_BTN_TINT_HOVER,
                (false, _) => NOTIFY_BTN_TINT,
            };
            scene.fill_rect(blend(bg, text, tint), *rect);
            draw_border(scene, *rect, divider);
            if let Some(label) = self.buttons.get(i) {
                scene.label(
                    (*label).to_string(),
                    *rect,
                    text,
                    // 28 高的按钮盒里文字只有 16 高：源是垂直居中，这里显式声明。
                    styles.action,
                    TextAlign::Center,
                );
            }
        }
        // 源 :2068-2084：关闭叉垫底常驻；卡片任一悬停都点亮叉（合成器按整卡 hover 取 alpha）。
        let close_hot = self.hover == Some(NotifyHit::Close);
        let close_bg = if close_hot {
            blend(bg, text, NOTIFY_CLOSE_TINT)
        } else {
            bg
        };
        scene.fill_rect(close_bg, g.close);
        let glyph = if self.hover.is_some() {
            secondary
        } else {
            Color::TRANSPARENT
        };
        icons::draw(scene, GlyphName::Close, g.close, glyph);
    }

    /// 源 `card_height`（notifications.rs:1778-1792）逐值等价。
    fn card_height(&self) -> f64 {
        let mut h = NOTIFY_PAD;
        if !self.app_name.is_empty() {
            h += NOTIFY_APP_LINE_H;
        }
        if !self.summary.is_empty() {
            h += NOTIFY_LINE_H;
        }
        h += NOTIFY_LINE_H * self.body_lines.len().min(NOTIFY_BODY_MAX_LINES) as f64;
        if !self.buttons.is_empty() {
            h += NOTIFY_BTN_TOP_GAP + NOTIFY_BTN_H;
        }
        h += NOTIFY_PAD;
        h.max(NOTIFY_PAD * 2.0 + NOTIFY_IMAGE_SIZE)
    }
}

/// 源 `rect_from`（notifications.rs:1613-1618）的逐值等价：四边各自四舍五入，宽 / 高下限 1。
///
/// 合成器的卡片与子矩形都是整数逻辑像素（`Rectangle<i32, Logical>`），命中与绘制都消费它；
/// 保持同一舍入，迁移后同一输入才给出同一组矩形。
fn px_rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect::new(
        x.round() as f32,
        y.round() as f32,
        w.round().max(NOTIFY_MIN_EXTENT) as f32,
        h.round().max(NOTIFY_MIN_EXTENT) as f32,
    )
}

/// 源 `draw_border`（notifications.rs:1928-1940）：四条 1 px 细矩形。
///
/// 不用 `Scene::stroke_rect`：stroke 以边线为中心，会向外溢半像素 —— 那会改像素。
fn draw_border(scene: &mut Scene, rect: Rect, color: Color) {
    let (x, y, w, h) = (
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    );
    let t = NOTIFY_BORDER as f32;
    for edge in [
        Rect::new(x, y, w, t),
        Rect::new(x, y + h - t, w, t),
        Rect::new(x, y, t, h),
        Rect::new(x + w - t, y, t, h),
    ] {
        scene.fill_rect(color, edge);
    }
}

/// 源 `blend`（notifications.rs:1628-1635）：手工混合，alpha 取两者较大值。
///
/// 不用 `Color::lerp`：它连 alpha 一起插值，源配方是 `max` —— 逐像素对齐不能差这一点。
fn blend(base: Color, top: Color, a: f32) -> Color {
    Color {
        r: base.r + (top.r - base.r) * a,
        g: base.g + (top.g - base.g) * a,
        b: base.b + (top.b - base.b) * a,
        a: base.a.max(top.a),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 期望值按 notifications.rs 的常量手算，不调被测函数生成。

    fn two_lines() -> Vec<String> {
        vec!["第一行".to_string(), "第二行".to_string()]
    }

    fn fill_color(scene: &Scene, rect: Rect) -> Color {
        use kanesumi_canvas::SceneCommand;
        scene
            .commands
            .iter()
            .find_map(|c| match c {
                SceneCommand::FillRect { rect: r, color, .. } if *r == rect => Some(*color),
                _ => None,
            })
            .unwrap_or_else(|| panic!("缺少矩形 {rect:?} 的填充命令"))
    }

    fn close_glyph_tint(scene: &Scene) -> Option<Color> {
        use kanesumi_canvas::SceneCommand;
        scene.commands.iter().find_map(|c| match c {
            SceneCommand::Image { tint, .. } => *tint,
            _ => None,
        })
    }

    #[test]
    fn migrate_geometry_matches_source_without_and_with_image() {
        let lines = two_lines();
        let plain = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新已就绪",
            body_lines: &lines,
            buttons: &[],
            has_image: false,
        });
        let g = plain.geometry(320.0);
        // 12 + 16（应用名）+ 22（标题）+ 22×2（正文）+ 12 = 106。
        assert_eq!(g.card, Rect::new(0.0, 0.0, 320.0, 106.0));
        assert_eq!(g.icon, None);
        assert_eq!(g.app, Some(Rect::new(12.0, 12.0, 296.0, 16.0)));
        assert_eq!(g.title, Rect::new(12.0, 28.0, 296.0, 22.0));
        assert_eq!(g.body, Rect::new(12.0, 50.0, 296.0, 44.0));
        assert_eq!(
            g.body_lines,
            vec![
                Rect::new(12.0, 50.0, 296.0, 22.0),
                Rect::new(12.0, 72.0, 296.0, 22.0),
            ]
        );
        assert_eq!(g.close, Rect::new(296.0, 6.0, 18.0, 18.0));
        assert!(g.buttons.is_empty());

        let image = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新已就绪",
            body_lines: &lines,
            buttons: &[],
            has_image: true,
        });
        let g = image.geometry(320.0);
        assert_eq!(g.card, Rect::new(0.0, 0.0, 320.0, 106.0), "有图不改卡高");
        assert_eq!(g.icon, Some(Rect::new(12.0, 12.0, 48.0, 48.0)));
        // 文字列左移 48 + 12：x 72、宽 320 − 12 − 60 − 12 = 236。
        assert_eq!(g.app, Some(Rect::new(72.0, 12.0, 236.0, 16.0)));
        assert_eq!(g.body_lines[0], Rect::new(72.0, 50.0, 236.0, 22.0));
    }

    #[test]
    fn migrate_button_rows_for_zero_one_two_buttons() {
        let lines: Vec<String> = Vec::new();
        let none = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &[],
            has_image: false,
        });
        let g = none.geometry(320.0);
        // 12 + 16 + 22 + 0 + 12 = 62 → 源 card_height 的下限 72（12×2 + 48）。
        assert_eq!(g.card.size.height, 72.0);
        assert!(g.buttons.is_empty());

        let one = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &["立即重启"],
            has_image: false,
        });
        let g = one.geometry(320.0);
        // 12 + 16 + 22 + 0 +（10 + 28）+ 12 = 100；按钮贴着卡底：100 − 12 − 28 = 60。
        assert_eq!(g.card.size.height, 100.0);
        assert_eq!(g.buttons, vec![Rect::new(12.0, 60.0, 296.0, 28.0)]);

        let two = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &["稍后", "立即重启"],
            has_image: false,
        });
        let g = two.geometry(320.0);
        assert_eq!(g.card.size.height, 100.0);
        // 296 − 8 = 288，两枚各 144，间隔 8。
        assert_eq!(
            g.buttons,
            vec![
                Rect::new(12.0, 60.0, 144.0, 28.0),
                Rect::new(164.0, 60.0, 144.0, 28.0),
            ]
        );
    }

    #[test]
    fn migrate_hit_priority_close_then_button_then_body() {
        let lines = two_lines();
        let labels = ["立即重启"];
        let card = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &labels,
            has_image: false,
        });
        // 卡高 106 + 10 + 28 = 144；按钮 (12, 104, 296, 28)；叉 (296, 6, 18, 18)。
        let g = card.geometry(320.0);
        assert_eq!(g.card.size.height, 144.0);
        assert_eq!(
            card.hit(&g, Point::new(300.0, 10.0)),
            Some(NotifyHit::Close),
            "叉 ∩ 卡体 → Close 优先"
        );
        assert_eq!(
            card.hit(&g, Point::new(20.0, 110.0)),
            Some(NotifyHit::Action(0)),
            "按钮 ∩ 卡体 → Action 优先于 Body"
        );
        assert_eq!(card.hit(&g, Point::new(100.0, 20.0)), Some(NotifyHit::Body));
        assert_eq!(card.hit(&g, Point::new(400.0, 10.0)), None, "卡外");
        // 半开区间：叉右缘 314 / 下缘 24 不含。
        assert_eq!(
            card.hit(&g, Point::new(313.9, 23.9)),
            Some(NotifyHit::Close)
        );
        assert_eq!(card.hit(&g, Point::new(314.0, 24.0)), Some(NotifyHit::Body));
    }

    #[test]
    fn migrate_hover_changes_colors_but_not_command_count() {
        let lines = two_lines();
        let labels = ["立即重启"];
        let mut card = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &labels,
            has_image: false,
        });
        let g = card.geometry(320.0);
        let theme = MetroTheme::ether_dark();
        let bg = ThemeColor::SurfaceVariant
            .resolve(&theme)
            .with_alpha(NOTIFY_BG_ALPHA);

        let mut plain = Scene::default();
        card.render(&g, &theme, &mut plain);
        assert_eq!(fill_color(&plain, g.close), bg);
        assert_eq!(
            fill_color(&plain, g.buttons[0]),
            blend(bg, ThemeColor::OnSurface.resolve(&theme), NOTIFY_BTN_TINT)
        );
        assert_eq!(close_glyph_tint(&plain), Some(Color::TRANSPARENT));

        card.set_hover(Some(NotifyHit::Close));
        let mut hot_close = Scene::default();
        card.render(&g, &theme, &mut hot_close);
        assert_eq!(
            hot_close.commands.len(),
            plain.commands.len(),
            "悬停不增删命令"
        );
        assert_eq!(
            fill_color(&hot_close, g.close),
            blend(bg, ThemeColor::OnSurface.resolve(&theme), NOTIFY_CLOSE_TINT)
        );
        assert_eq!(
            close_glyph_tint(&hot_close),
            Some(ThemeColor::OnSurfaceVariant.resolve(&theme)),
            "卡片一悬停，叉就点亮"
        );

        card.set_hover(Some(NotifyHit::Action(0)));
        let mut hot_button = Scene::default();
        card.render(&g, &theme, &mut hot_button);
        assert_eq!(
            hot_button.commands.len(),
            plain.commands.len(),
            "悬停不增删命令"
        );
        assert_eq!(
            fill_color(&hot_button, g.buttons[0]),
            blend(
                bg,
                ThemeColor::OnSurface.resolve(&theme),
                NOTIFY_BTN_TINT_HOVER
            )
        );
        assert_eq!(fill_color(&hot_button, g.close), bg, "按钮悬停不点亮叉底");
    }

    #[test]
    fn pressed_action_keeps_old_feedback_until_pointer_leaves() {
        let mut card = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &[],
            buttons: &["打开", "稍后"],
            has_image: false,
        });
        let geometry = card.geometry(320.0);
        let theme = MetroTheme::ether_dark();
        let bg = ThemeColor::SurfaceVariant.resolve(&theme).with_alpha(NOTIFY_BG_ALPHA);
        let fg = ThemeColor::OnSurface.resolve(&theme);
        let mut plain = Scene::default();
        card.render(&geometry, &theme, &mut plain);

        card.set_hover(Some(NotifyHit::Action(0)));
        card.set_pressed_action(Some(0));
        let mut pressed = Scene::default();
        card.render(&geometry, &theme, &mut pressed);
        assert_eq!(fill_color(&pressed, geometry.buttons[0]), blend(bg, fg, 0.18));
        assert_eq!(pressed.commands.len(), plain.commands.len());

        card.set_hover(Some(NotifyHit::Action(1)));
        let mut moved = Scene::default();
        card.render(&geometry, &theme, &mut moved);
        assert_eq!(fill_color(&moved, geometry.buttons[0]), blend(bg, fg, 0.06));
        assert_eq!(fill_color(&moved, geometry.buttons[1]), blend(bg, fg, 0.12));
        assert_eq!(moved.commands.len(), plain.commands.len());
    }

    #[test]
    fn migrate_empty_summary_and_app_do_not_reserve_lines() {
        let lines = vec!["正文".to_string()];
        let bare = MetroNotificationCard::new(NotificationContent {
            app_name: "",
            summary: "",
            body_lines: &lines,
            buttons: &[],
            has_image: false,
        });
        let g = bare.geometry(320.0);
        assert_eq!(g.app, None);
        assert_eq!(g.title.size.height, 0.0);
        // 正文从 12 起：没有应用名 / 标题各行可让。
        assert_eq!(g.body_lines[0].origin.y, 12.0);
        // 12 + 22 + 12 = 46 → 下限 72。
        assert_eq!(g.card.size.height, 72.0);
    }

    #[test]
    fn migrate_body_lines_cap_at_three() {
        let lines: Vec<String> = (0..5).map(|i| format!("行{i}")).collect();
        let card = MetroNotificationCard::new(NotificationContent {
            app_name: "系统",
            summary: "更新",
            body_lines: &lines,
            buttons: &[],
            has_image: false,
        });
        let g = card.geometry(320.0);
        assert_eq!(g.body_lines.len(), NOTIFY_BODY_MAX_LINES, "源上限 3 行");
        assert_eq!(g.body.size.height, 66.0);
        // 12 + 16 + 22 + 66 + 12 = 128。
        assert_eq!(g.card.size.height, 128.0);
    }
}
