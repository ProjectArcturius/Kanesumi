// notification_card.rs —— 通知卡片：两代实现，**同一文件、不同契约**。
//
// 上半（`CardGeometry` / `render_card`）：量测版 —— 换行 / 省略 / 按钮宽都交给
// `TextEngine`；高度由内容量出来，不写死。参 docs/KANESUMI_DESIGN.md（直角、贴边、
// 单一强调色）、`docs/INTERACTION_CANON.md`（最小命中区）。
//
// 下半（`MetroNotificationCard`）：迁移版 —— 几何逐值抄 Ether 合成器
// `compositor/src/shell/notifications.rs`，正文行由调用方切好，控件不量测、不换行。
// 依据 docs/research/n1112_element_bridge/NOTIFY_PLAN.md §二 步骤 1；规格见
// `docs/CONTROL_SPEC.md` §56，映射与常量出处见 `docs/research/m1115_notify/REPORT.md`。
// 两代在步骤 2（换行迁 canvas）合流，在那之前别把它们混用。

use std::sync::Arc;

use kanesumi_canvas::icons::{self, GlyphName};
use kanesumi_canvas::{Scene, TextAlign, TextEngine, TextLayout, TextLayoutOptions, TextOverflow};
use kanesumi_core::sizes::{ICON_12, ICON_16, ICON_32};
use kanesumi_core::typography::{MetroTypography, TextStyle, TextVAlign, base_weight};
use kanesumi_core::{Color, MetroTheme, Point, Rect, ThemeColor};

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

// ── 迁移版：MetroNotificationCard（k-notify-card） ─────────────────────────
//
// 本文件里有两代通知卡，**不是同一个东西**：
// - 上面的 `CardGeometry` / `render_card` 是「量测版」：换行、单行省略、按钮宽度都交给
//   `TextEngine` 量（k-notification-family）；
// - 下面的 `MetroNotificationCard` 是「迁移版」：几何逐值抄 Ether
//   `compositor/src/shell/notifications.rs`，正文行由调用方切好，控件不量测、不换行
//   （参 NOTIFY_PLAN §二 步骤 1）。步骤 2 把换行交给 canvas 后两代才合流。
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

    // ── 迁移版（k-notify-card）：期望值按 notifications.rs 的常量手算，不调被测函数生成 ──

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
    // 关闭钮：只在悬停时给底，图形始终存在（元素数稳定 —— 位图命令恒有一条）。
    if visual.close_hovered {
        scene.fill_rect(c.surface_variant, geo.close);
    }
    // 图形 12 pt 落在 32 pt 的命中区里居中：命中区管手指，图形管观感，两者不同尺寸。
    icons::draw(
        scene,
        GlyphName::Close,
        Rect::new(
            geo.close.origin.x + (geo.close.size.width - ICON_12) / 2.0,
            geo.close.origin.y + (geo.close.size.height - ICON_12) / 2.0,
            ICON_12,
            ICON_12,
        ),
        if visual.close_hovered {
            c.on_surface
        } else {
            c.on_surface_variant
        },
    );
}
