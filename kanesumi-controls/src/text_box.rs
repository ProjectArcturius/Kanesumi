// MetroTextBox —— 单行文本输入框。参 CONTROL_SPEC §34（TextBox 参考，闭源）。
//
// 数据源：`reference/microsoft-ui-xaml/dev/CommonStyles/TextBox_themeresources_v1.xaml`：
// - 布局三行：Header(Auto) / 内容(*) / Description(Auto)；两列 `*` + Auto（删除按钮）；
// - Border 1px（Focused 2px）；Padding `10,6,6,5`；MinHeight 32；MinWidth 64；
// - 状态色硬切换（Normal/PointerOver/Focused/Disabled）；无颜色过渡动画。
// - 编辑核心在 `text_field.rs`（纯逻辑）：光标 / 选区 / 撤销 / 掩码。
//
// Kanesumi 适配：深色桌面底色 `surface`；选区强调色 35%；光标 2px（V10 HiDPI）。

use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect};

use crate::state::ControlState;
use crate::text_field::{TextInputKey, TextField};

/// 删除按钮列宽（TextBox 右上角 × 按钮，MinWidth 34）。参 themeresources_v1。
pub const TEXTBOX_DELETE_BUTTON_W: f32 = 34.0;
/// 光标宽度（V10：1px 在 HiDPI 亚像素退化 → 2px）。
pub const TEXTBOX_CARET_W: f32 = 2.0;
/// 光标闪烁半周期（on/off 各 0.5s，对齐 UWP 光标闪烁率）。
pub const CARET_BLINK_HALF_PERIOD: f32 = 0.5;

/// MetroTextBox —— 单行文本输入。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTextBox {
    /// 编辑核心（光标 / 选区 / 撤销）。
    pub field: TextField,
    /// 占位文本（空内容时显示，Focused 时转半透明）。
    pub placeholder: String,
    /// 顶部标题（可选，UWP `Header`）。
    pub header: String,
    /// 交互状态。
    pub state: ControlState,
    /// 是否聚焦（显示光标 + Focused 边框）。
    pub focused: bool,
    /// 是否显示删除按钮（UWP ButtonStates：有内容 + hover/聚焦时）。
    pub show_delete: bool,
    /// 水平滚动偏移（内容超宽时，保持光标可见）。
    pub scroll: f32,
    /// 光标闪烁相位累加（update(dt) 推进）。
    blink_phase: f32,
}

impl Default for MetroTextBox {
    fn default() -> Self {
        Self {
            field: TextField::new(),
            placeholder: String::new(),
            header: String::new(),
            state: ControlState::Normal,
            focused: false,
            show_delete: false,
            scroll: 0.0,
            blink_phase: 0.0,
        }
    }
}

impl MetroTextBox {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带占位文本构造。
    pub fn with_placeholder(text: impl Into<String>) -> Self {
        Self {
            placeholder: text.into(),
            ..Self::default()
        }
    }

    /// 带标题构造（UWP `<TextBox Header="…" />`）。
    pub fn with_header(text: impl Into<String>) -> Self {
        Self {
            header: text.into(),
            ..Self::default()
        }
    }

    /// 设置初始内容（光标置尾）。builder 链式。
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.field = TextField::with_text(text);
        self
    }

    /// 直接以内容构造（等同 `new().with_text`）。
    pub fn from_text(text: impl Into<String>) -> Self {
        Self::new().with_text(text)
    }

    /// 聚焦进入。
    pub fn focus(&mut self) {
        self.focused = true;
        self.state = ControlState::Focused;
        self.blink_phase = 0.0;
        // 聚焦时若为空，光标在 0（默认已是）；选择全部（UWP TextBox 聚焦行为：全选）。
        // 但点击定位（place_caret）会再校正。
        self.field.select_all();
    }

    /// 聚焦退出（清除选区，保留光标）。
    pub fn blur(&mut self) {
        self.focused = false;
        self.state = ControlState::Normal;
        // 失焦：光标归一（取消选区）
        self.field.set_cursor(self.field.cursor());
    }

    /// 每帧推进光标闪烁。
    pub fn update(&mut self, dt: f64) {
        if self.focused {
            self.blink_phase = (self.blink_phase + dt as f32) % (CARET_BLINK_HALF_PERIOD * 2.0);
        }
    }

    /// 光标当前是否可见（闪烁相位前半周期）。
    pub fn caret_visible(&self) -> bool {
        self.blink_phase < CARET_BLINK_HALF_PERIOD
    }

    /// 处理一个编辑键（转发给编辑核心）。返回 true = 内容/光标变化。
    pub fn handle_key(&mut self, key: TextInputKey) -> bool {
        let changed = self.field.handle_key(key);
        self.ensure_caret_visible();
        changed
    }

    /// 内容变化后重置闪烁相位（光标立刻亮起）。
    fn reset_blink(&mut self) {
        self.blink_phase = 0.0;
    }

    // ── 几何 ──────────────────────────────────────────────────

    /// 内容区（在 `body` 内扣除边框 + Padding）。UWP Padding `10,6,6,5` + Border。
    pub fn content_rect(&self, theme: &MetroTheme, body: Rect) -> Rect {
        let _ = theme;
        let b = self.border_thickness();
        let pad_l = 10.0;
        let pad_t = 6.0;
        let pad_r = 6.0;
        let pad_b = 5.0;
        Rect::new(
            body.origin.x + b + pad_l,
            body.origin.y + b + pad_t,
            (body.size.width - 2.0 * b - pad_l - pad_r).max(0.0),
            (body.size.height - 2.0 * b - pad_t - pad_b).max(0.0),
        )
    }

    /// 删除按钮矩形（主体右缘，34 宽）。
    pub fn delete_button_rect(&self, _theme: &MetroTheme, body: Rect) -> Rect {
        let b = self.border_thickness();
        Rect::new(
            body.right() - b - TEXTBOX_DELETE_BUTTON_W,
            body.origin.y + b,
            TEXTBOX_DELETE_BUTTON_W,
            (body.size.height - 2.0 * b).max(0.0),
        )
    }

    /// 边框厚度：聚焦 2px，其余 1px（TextControlBorderThemeThickness/Focused）。
    pub fn border_thickness(&self) -> f32 {
        if self.focused {
            2.0
        } else {
            1.0
        }
    }

    /// 控件最小高（UWP TextControlThemeMinHeight 32）。
    pub const fn min_height() -> f32 {
        32.0
    }

    /// 文本显示用字符（掩码或明文）。
    fn display_chars(&self) -> Vec<char> {
        self.field.display_chars()
    }

    /// 光标前的显示文本（度量光标 x 用）。
    fn prefix_display(&self) -> String {
        let chars = self.display_chars();
        chars[..self.field.cursor().min(chars.len())].iter().collect()
    }

    /// 光标 x（相对 body 左缘，含滚动偏移）。
    pub fn caret_x(&self, theme: &MetroTheme, engine: &TextEngine, body: Rect) -> f32 {
        let content = self.content_rect(theme, body);
        let size = theme.typography.body.size;
        let prefix = self.prefix_display();
        let w = engine.measure(&prefix, size);
        content.origin.x - self.scroll + w
    }

    /// 命中测试（整个控件矩形）。
    pub fn hit_test(&self, rect: Rect, pos: Point) -> bool {
        rect.contains(pos)
    }

    /// 点击定位光标：根据 x 找最近的字符下标。返回 true = 光标移动。
    pub fn place_caret_at(&mut self, theme: &MetroTheme, engine: &TextEngine, body: Rect, pos: Point) -> bool {
        let content = self.content_rect(theme, body);
        let size = theme.typography.body.size;
        let chars = self.display_chars();
        let click_x = (pos.x + self.scroll - content.origin.x).max(0.0);
        // 点击最左（第一个字符中点之前）→ 光标 0（UWP TextBox 点框左缘 = 行首）
        if let Some(&first) = chars.first() {
            let first_mid = engine.glyph_metrics(first, size).advance_width / 2.0;
            if click_x < first_mid {
                self.field.set_cursor(0);
                return true;
            }
        }
        let mut best = chars.len();
        let mut best_dist = f32::MAX;
        let mut acc = 0.0;
        for (i, c) in chars.iter().enumerate() {
            let cw = engine.glyph_metrics(*c, size).advance_width;
            let mid = acc + cw / 2.0;
            let d = (click_x - mid).abs();
            if d < best_dist {
                best_dist = d;
                best = i + 1;
            }
            acc += cw;
        }
        self.field.set_cursor(best);
        true
    }

    /// 确保光标在可视范围内（水平滚动夹紧）。内容宽 / 光标 x 由调用方（App 层）
    /// 结合视口宽度驱动 `scroll`；此处仅重设闪烁（内容变化后立刻亮起）。
    fn ensure_caret_visible(&mut self) {
        self.reset_blink();
    }

    // ── 渲染 ──────────────────────────────────────────────────

    /// 渲染到 `rect`。顺序：Header → 底色 → 选区 → 文本/占位 → 光标 → 删除按钮 → 边框。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        let colors = &theme.colors;
        let style = theme.typography.body;
        let disabled = self.state == ControlState::Disabled;
        let alpha = if disabled {
            theme.indication.disabled_opacity
        } else {
            1.0
        };

        // Header（顶部一行，UWP TextBoxTopHeaderMargin 0,0,0,4）
        if !self.header.is_empty() {
            let header_rect = Rect::new(
                rect.origin.x,
                rect.origin.y,
                rect.size.width,
                style.line_height,
            );
            scene.text(
                self.header.clone(),
                header_rect,
                colors.on_surface.with_alpha(colors.on_surface.a * alpha),
                style,
                TextAlign::Left,
            );
        }

        let body_rect = self.body_rect(theme, rect);
        let b = self.border_thickness();
        let inner = Rect::new(
            body_rect.origin.x,
            body_rect.origin.y,
            (body_rect.size.width - 2.0 * b).max(0.0),
            (body_rect.size.height - 2.0 * b).max(0.0),
        );

        // 底色（UWP TextControlBackground；Kanesumi = surface）
        let bg = colors.surface.with_alpha(colors.surface.a * alpha);
        scene.fill_rounded_rect(bg, inner, theme.tokens.corner_radius);

        let content = self.content_rect(theme, body_rect);

        // 选区高亮（TextControlSelectionHighlightColor → 强调色 35%）
        if let Some((lo, hi)) = self.field.selection() {
            let chars = self.display_chars();
            let lo = lo.min(chars.len());
            let hi = hi.min(chars.len());
            let pre: String = chars[..lo].iter().collect();
            let sel: String = chars[lo..hi].iter().collect();
            let x0 = content.origin.x - self.scroll + engine.measure(&pre, style.size);
            let w = engine.measure(&sel, style.size);
            let sel_rect = Rect::new(x0, content.origin.y, w, content.size.height);
            scene.fill_rect(colors.primary.with_alpha(0.35 * alpha), sel_rect);
        }

        // 文本 / 占位
        let has_text = !self.field.is_empty();
        if has_text {
            let text = self.field.display_text();
            let text_x = content.origin.x - self.scroll;
            let text_rect = Rect::new(
                text_x,
                content.origin.y,
                content.size.width + self.scroll,
                style.line_height,
            );
            let fg = colors.on_surface;
            scene.text(
                text,
                text_rect,
                fg.with_alpha(fg.a * alpha),
                style,
                TextAlign::Left,
            );
        } else if !self.placeholder.is_empty() {
            // 占位文本：Focused 时用 TextControlPlaceholderForegroundFocused（略暗）
            let ph = colors.on_surface_variant.with_alpha(
                colors.on_surface_variant.a * alpha * if self.focused { 0.7 } else { 1.0 },
            );
            let ph_rect = Rect::new(
                content.origin.x,
                content.origin.y,
                content.size.width,
                style.line_height,
            );
            scene.text(
                self.placeholder.clone(),
                ph_rect,
                ph,
                style,
                TextAlign::Left,
            );
        }

        // 光标（聚焦 + 可见时显示）
        if self.focused && self.caret_visible() {
            let cx = self.caret_x(theme, engine, body_rect);
            // 光标不超出右缘
            let max_x = body_rect.right() - b - 2.0;
            let cx = cx.min(max_x).max(body_rect.origin.x + b);
            let caret_rect = Rect::new(cx, content.origin.y, TEXTBOX_CARET_W, content.size.height);
            scene.fill_rect(colors.on_surface.with_alpha(0.9), caret_rect);
        }

        // 删除按钮（show_delete + 有内容）：UWP ×（E894）→ Kanesumi 自绘 ×（Triangle 两段）
        if self.show_delete && has_text && !disabled {
            let btn = self.delete_button_rect(theme, body_rect);
            let c = btn.center();
            let r = btn.size.height.min(8.0) / 2.0;
            // ×：两条线用细矩形近似（简化：半透明 × 字形用三角形组合）
            let col = colors.on_surface_variant.with_alpha(0.8);
            let t = 1.5;
            // 用 4 个小三角形拼 × 太碎；改用一个居中 × 文本字形（U+2715），
            // 思源黑体包含乘法叉号。参 V7 不假设 Segoe MDL2。
            scene.text(
                "✕".into(),
                Rect::new(c.x - 8.0, c.y - 8.0, 16.0, 16.0),
                col,
                style,
                TextAlign::Center,
            );
            let _ = r;
            let _ = t;
        }

        // 边框（Focused 2px + focus 色；其余 divider 1px）
        let (stroke, stroke_w) = if self.focused {
            (colors.focus_stroke.with_alpha(alpha), 2.0)
        } else if self.state == ControlState::Hovered {
            (colors.on_surface_variant.with_alpha(0.9 * alpha), 1.0)
        } else {
            (colors.divider.with_alpha(alpha), 1.0)
        };
        scene.stroke_rounded_rect(
            stroke,
            inner,
            stroke_w,
            theme.tokens.corner_radius,
        );
    }

    /// 主体矩形（Header 之下）。
    fn body_rect(&self, theme: &MetroTheme, rect: Rect) -> Rect {
        let style = theme.typography.body;
        let header_h = if self.header.is_empty() {
            0.0
        } else {
            style.line_height + 4.0 // TextBoxTopHeaderMargin 下 4
        };
        Rect::new(
            rect.origin.x,
            rect.origin.y + header_h,
            rect.size.width,
            (rect.size.height - header_h).max(0.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_canvas::SceneCommand;

    fn find_font() -> Option<std::path::PathBuf> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ] {
            let p = std::path::PathBuf::from(p);
            if p.exists() {
                return Some(p);
            }
        }
        None
    }

    fn font_available() -> bool {
        find_font().is_some()
    }

    fn engine() -> TextEngine {
        TextEngine::load(find_font().unwrap()).unwrap()
    }

    fn theme() -> MetroTheme {
        MetroTheme::ether_dark()
    }

    #[test]
    fn typing_via_handle_key() {
        let mut tb = MetroTextBox::new();
        tb.focus();
        for c in ['h', 'i'] {
            tb.handle_key(TextInputKey::Char(c));
        }
        assert_eq!(tb.field.text(), "hi");
        assert_eq!(tb.field.cursor(), 2);
        assert!(tb.handle_key(TextInputKey::Backspace));
        assert_eq!(tb.field.text(), "h");
    }

    #[test]
    fn placeholder_shown_when_empty() {
        if !font_available() {
            return;
        }
        let tb = MetroTextBox::with_placeholder("搜索…");
        let mut scene = Scene::default();
        tb.render(&theme(), &engine(), Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        let texts: Vec<&String> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t.as_str() == "搜索…"), "空框显示占位文本");
    }

    #[test]
    fn text_hides_placeholder() {
        if !font_available() {
            return;
        }
        let tb = MetroTextBox::with_placeholder("搜索…").with_text("Ether");
        let mut scene = Scene::default();
        tb.render(&theme(), &engine(), Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        let texts: Vec<String> = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::Text { content, .. } => Some(content.clone()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "Ether"));
        assert!(!texts.iter().any(|t| t == "搜索…"));
    }

    #[test]
    fn focused_renders_caret() {
        if !font_available() {
            return;
        }
        let mut tb = MetroTextBox::new();
        tb.focus();
        tb.update(0.1); // 闪烁相位 < 0.5 → 可见
        let mut scene = Scene::default();
        tb.render(&theme(), &engine(), Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        // 光标 = 一个 2px 宽的 FillRect（色为 on_surface 0.9）
        let carets = scene
            .commands
            .iter()
            .filter_map(|c| match c {
                SceneCommand::FillRect { rect, color, .. } => {
                    if rect.size.width == TEXTBOX_CARET_W && color.a > 0.8 {
                        Some(())
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .count();
        assert_eq!(carets, 1, "聚焦时渲染光标");
    }

    #[test]
    fn caret_blinks_off_after_half_period() {
        let mut tb = MetroTextBox::new();
        tb.focus();
        tb.update(CARET_BLINK_HALF_PERIOD as f64 + 0.1);
        assert!(!tb.caret_visible(), "过半周期光标隐藏");
        tb.update(CARET_BLINK_HALF_PERIOD as f64 + 0.1);
        assert!(tb.caret_visible(), "再半周期光标再现");
    }

    #[test]
    fn caret_x_after_content() {
        if !font_available() {
            return;
        }
        let e = engine();
        let th = theme();
        let tb = MetroTextBox::from_text("Ether");
        let body = Rect::new(0.0, 0.0, 200.0, 32.0);
        let empty = MetroTextBox::new();
        let x_empty = empty.caret_x(&th, &e, body);
        let x_text = tb.caret_x(&th, &e, body);
        assert!(x_text > x_empty, "有内容时光标更靠右");
        assert!((x_empty - (1.0 + 10.0)).abs() < 0.1, "空框光标 = 边框+左Padding");
    }

    #[test]
    fn place_caret_moves_cursor() {
        if !font_available() {
            return;
        }
        let e = engine();
        let th = theme();
        let mut tb = MetroTextBox::from_text("ab");
        let body = Rect::new(0.0, 0.0, 200.0, 32.0);
        let size = th.typography.body.size;
        let w_a = e.measure("a", size);
        // 点在第 1 字符中部 → 光标 1
        tb.place_caret_at(&th, &e, body, Point::new(11.0 + w_a / 2.0, 16.0));
        assert_eq!(tb.field.cursor(), 1);
        // 点在最左 → 光标 0
        tb.place_caret_at(&th, &e, body, Point::new(5.0, 16.0));
        assert_eq!(tb.field.cursor(), 0);
    }

    #[test]
    fn focus_selects_all() {
        let mut tb = MetroTextBox::from_text("hello");
        tb.focus();
        assert_eq!(tb.field.selection(), Some((0, 5)));
        tb.blur();
        assert_eq!(tb.field.selection(), None, "失焦清除选区");
    }

    #[test]
    fn header_reserves_top() {
        if !font_available() {
            return;
        }
        let tb = MetroTextBox::with_header("用户名");
        let th = theme();
        let rect = Rect::new(0.0, 0.0, 200.0, 60.0);
        let body = tb.body_rect(&th, rect);
        assert!(body.origin.y > 0.0, "Header 占用顶部空间");
    }

    #[test]
    fn disabled_lowers_alpha() {
        if !font_available() {
            return;
        }
        let mut tb = MetroTextBox::from_text("x");
        tb.state = ControlState::Disabled;
        let mut scene = Scene::default();
        tb.render(&theme(), &engine(), Rect::new(0.0, 0.0, 200.0, 32.0), &mut scene);
        // 首命令是底色填充，alpha < 1
        let Some(SceneCommand::FillRect { color, .. }) = scene.commands.first() else {
            panic!("首命令应为底色");
        };
        assert!(color.a < 1.0, "禁用降透明度，实际 a={}", color.a);
    }

    #[test]
    fn hit_test_contains() {
        let tb = MetroTextBox::new();
        let rect = Rect::new(0.0, 0.0, 200.0, 32.0);
        assert!(tb.hit_test(rect, Point::new(100.0, 16.0)));
        assert!(!tb.hit_test(rect, Point::new(300.0, 16.0)));
    }
}
