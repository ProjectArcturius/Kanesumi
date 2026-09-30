use kanesumi_anim::{EasingMode, MetroAnim, UwpEasing};
use kanesumi_canvas::text::TextEngine;
use kanesumi_canvas::{Scene, TextAlign};
use kanesumi_core::{Color, FontWeight, MetroTheme, Point, Rect, TextStyle};

/// MetroTabRow —— 标签行（Pivot 参考）。参 CONTROL_SPEC §6：
/// - Header 高 48、Padding `12,0,12,0`；头字 24 SemiLight、字距 −2.5%；
/// - 选中 = 文字最深 + 底部 2px 强调色管道；
/// - 无头背景高亮（选中只靠文字色 + 管道）。
///
/// V17：切换从"瞬时"改为"管道时长驱动滑行 + 文字色 crossfade"，
/// 参 UWP TabView / Fluent NavigationView Top 的 SelectionIndicator 动画。
/// 用 `MetroAnim` 而非 `SpringAnim`：管道是次要属性过渡（非位移主运动），
/// UWP 时代约定 0.25s Quadratic/EaseOut。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroTab {
    pub label: String,
}

impl MetroTab {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
        }
    }
}

/// 管道滑行时长（秒）。UWP TabView SelectionIndicator = 250ms。
const PIPE_DURATION: f64 = 0.25;

#[derive(Debug, Clone, PartialEq)]
pub struct MetroTabRow {
    pub tabs: Vec<MetroTab>,
    pub selected: usize,
    pub hovered: Option<usize>,
    /// Header 高（UWP 48）。
    pub header_height: f32,
    /// 上一次选中（切换动画起点）。首次 select 前 = selected 自身（无动画）。
    prev_selected: usize,
    /// 管道位置动画（value ∈ [0, 1]，0 = prev、1 = current）。
    /// 稳态在 1（画在 current 上）；`select` 时 jump_to(0) + set_target(1) 重启滑行。
    select_anim: MetroAnim,
}

impl Default for MetroTabRow {
    fn default() -> Self {
        let mut anim = MetroAnim::new(PIPE_DURATION, UwpEasing::Quadratic, EasingMode::EaseOut);
        anim.jump_to(1.0);
        Self {
            tabs: Vec::new(),
            selected: 0,
            hovered: None,
            header_height: 48.0,
            prev_selected: 0,
            select_anim: anim,
        }
    }
}

impl MetroTabRow {
    pub fn new(tabs: Vec<MetroTab>) -> Self {
        Self {
            tabs,
            ..Self::default()
        }
    }

    /// 选中指定 tab。若与当前不同，启动管道滑行 + 文字色 crossfade。
    /// 同一 tab 重复 select 幂等，不重启动画。
    pub fn select(&mut self, index: usize) {
        if index >= self.tabs.len() || index == self.selected {
            return;
        }
        self.prev_selected = self.selected;
        self.selected = index;
        // 归零重启：value 直接落到 0（老 bug 参 dialog.rs L110 —— 不 jump_to
        // 则 set_target 会从残余 value 继续，看起来管道会往回蹦一下）。
        self.select_anim.jump_to(0.0);
        self.select_anim.set_target(1.0);
    }

    /// 每帧推进管道动画。GalleryApp / 宿主须调用；未调用则管道停在 prev（旧退化行为）。
    pub fn update(&mut self, dt: f64) {
        self.select_anim.update(dt);
    }

    /// 管道当前进度 [0, 1]，仅用于测试 / 调试。
    pub fn selection_progress(&self) -> f32 {
        self.select_anim.value() as f32
    }

    /// Header 文字样式：24 SemiLight，字距 −2.5%（UWP CharacterSpacing=−25）。
    /// V16：字距落到 TextStyle.letter_spacing_em，render/measure 全局生效。
    pub fn header_style() -> TextStyle {
        TextStyle::new(24.0, 30.0, FontWeight::Semilight).with_letter_spacing_em(-0.025)
    }

    /// 单个 Header 宽度 = 文字宽（含字距）+ 左右 12px。
    pub fn header_width(&self, engine: &TextEngine, index: usize) -> f32 {
        if index >= self.tabs.len() {
            return 0.0;
        }
        let style = Self::header_style();
        engine.measure_with_spacing(&self.tabs[index].label, style.size, style.letter_spacing_em)
            + 24.0
    }

    /// 全部 Header 总宽。
    pub fn total_width(&self, engine: &TextEngine) -> f32 {
        let style = Self::header_style();
        self.tabs
            .iter()
            .map(|t| {
                engine.measure_with_spacing(&t.label, style.size, style.letter_spacing_em) + 24.0
            })
            .sum()
    }

    /// Header 命中测试：`rect` = TabRow 布局矩形；`pos` = **绝对**指针坐标。
    ///
    /// 与 [`MetroSwitch::hit_test`] 约定一致——命中逻辑集中在控件内，调用方
    /// 无需手动减 origin。历史 bug：旧签名 `(engine, x)` 把 x 当**相对**坐标，
    /// Gallery 传绝对 `p.x` → 命中偏移一个 `rect.origin.x`。
    pub fn tab_at(&self, engine: &TextEngine, rect: Rect, pos: Point) -> Option<usize> {
        if !rect.contains(pos) {
            return None;
        }
        let style = Self::header_style();
        let mut cursor = rect.origin.x;
        for (i, tab) in self.tabs.iter().enumerate() {
            let w = engine.measure_with_spacing(&tab.label, style.size, style.letter_spacing_em)
                + 24.0;
            if pos.x >= cursor && pos.x < cursor + w {
                return Some(i);
            }
            cursor += w;
        }
        None
    }

    /// 单个 tab 的（label_x, label_w），x 相对 `rect.origin.x` 展开。
    fn label_geoms(&self, engine: &TextEngine, rect: Rect) -> Vec<(f32, f32)> {
        let style = Self::header_style();
        let mut cursor = rect.origin.x;
        let mut out = Vec::with_capacity(self.tabs.len());
        for tab in &self.tabs {
            let label_w =
                engine.measure_with_spacing(&tab.label, style.size, style.letter_spacing_em);
            out.push((cursor + 12.0, label_w));
            cursor += label_w + 24.0;
        }
        out
    }

    /// 渲染到 `rect`（横向排列，垂直到顶）。
    /// V17：管道从 prev tab 到 current tab 用 SpringAnim 滑行；两者的文字色按同一
    /// 进度做 crossfade（prev 淡出到 variant、current 淡入到 on_surface）。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, rect: Rect, scene: &mut Scene) {
        // 退化输入守卫（2026-09-22 审计 P0-3）：空 TabRow 旧实现走 `geoms[self.selected]`
        // 直接越界 panic（`MetroTabRow::default()` 即 `tabs: Vec::new()`）；空矩形无意义。
        if self.tabs.is_empty() || rect.size.width <= 0.0 || rect.size.height <= 0.0 {
            return;
        }
        let colors = &theme.colors;
        let style = Self::header_style();
        let geoms = self.label_geoms(engine, rect);
        let progress = self.select_anim.value().clamp(0.0, 1.0) as f32;

        // 容器语义 = 裁到自身矩形（2026-09-22 审计 P0-2）：页签按内容定宽，总宽可超控件宽，
        // 无裁剪时最右页签会画到相邻控件上。
        scene.push_clip(rect);

        for (i, tab) in self.tabs.iter().enumerate() {
            let (label_x, label_w) = geoms[i];

            // 文字色：切换中两个头 crossfade；其它按普通规则（hover=on_surface，否则 variant）
            let base = if self.hovered == Some(i) {
                colors.on_surface
            } else {
                colors.on_surface_variant
            };
            let fg = if i == self.selected {
                // current：progress 0 → variant，progress 1 → on_surface
                Color::lerp(base, colors.on_surface, progress as f64)
            } else if i == self.prev_selected && self.prev_selected != self.selected {
                // prev：progress 0 → on_surface，progress 1 → variant
                Color::lerp(colors.on_surface, base, progress as f64)
            } else {
                base
            };

            // 单行标签：不换行（旧实现给的是 header_height 高的框，宽度一旦不足便会折行）。
            // 纵向位置沿用旧值（顶部对齐），只收窄语义不移动视觉。
            let text_rect =
                Rect::new(label_x, rect.origin.y, label_w, style.line_height);
            scene.label(tab.label.clone(), text_rect, fg, style, TextAlign::Left);
        }

        // 选中管道：SpringAnim 从 prev 到 current 插值 x / w，强调色。
        // 稳态或首启（prev == current）时直接画在 current 下。
        // 索引夹紧：`tabs`/`selected` 均为 pub，App 可能先改 tabs 再改 selected。
        let last = geoms.len() - 1;
        let selected = self.selected.min(last);
        let prev_selected = self.prev_selected.min(last);
        let (cur_x, cur_w) = geoms[selected];
        let (pipe_x, pipe_w) = if prev_selected == selected {
            (cur_x, cur_w)
        } else {
            let (prev_x, prev_w) = geoms[prev_selected];
            (
                prev_x + (cur_x - prev_x) * progress,
                prev_w + (cur_w - prev_w) * progress,
            )
        };
        scene.fill_rect(
            colors.primary,
            Rect::new(pipe_x, rect.origin.y + self.header_height - 4.0, pipe_w, 2.0),
        );

        scene.pop_clip();
    }
}

// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 radio_buttons.rs）──────────
//
// 旧 API（`select` / `update` / `tab_at` / `render`）原样保留给未迁移的 App。选择类语义：
// `PointerUp` 用坐标 + 旧 `tab_at` 选页签，方向键切相邻页签。命中与焦点交给框架；管道滑行
// 由 `update` 转发并在未稳态时续帧（参 switch.rs 的 `update` 写法）。

/// 元素树动作：页签被用户切换（点击 / 方向键）。携带新的选中索引。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabSelectionChanged(pub usize);

impl kanesumi_element::Widget for MetroTabRow {
    /// 固有尺寸：宽 = 全部页签标题宽之和；高 = Header 高（默认 48，参 CONTROL_SPEC §6）。
    /// 宽随 `available.width` 收缩 —— 页签总宽可超宿主宽，窄 rect 下不得画出 rect（§6）。
    fn measure(
        &mut self,
        ctx: &mut kanesumi_element::MeasureCtx,
        available: kanesumi_core::Size,
    ) -> kanesumi_core::Size {
        let w = self.total_width(ctx.engine());
        let w = if available.width.is_finite() {
            w.min(available.width.max(0.0))
        } else {
            w
        };
        kanesumi_core::Size::new(w, self.header_height)
    }

    fn paint(&mut self, ctx: &mut kanesumi_element::PaintCtx, scene: &mut Scene) {
        // render 自己成对 PushClip/PopClip 到 rect，无需再加裁剪。
        self.render(ctx.theme(), ctx.engine(), ctx.rect(), scene);
    }

    fn update(&mut self, ctx: &mut kanesumi_element::UpdateCtx, dt: f64) {
        MetroTabRow::update(self, dt);
        if !self.select_anim.is_steady() {
            ctx.invalidate_paint();
            ctx.request_anim_frame();
        }
    }

    fn event(&mut self, ctx: &mut kanesumi_element::EventCtx, event: &kanesumi_element::Event) {
        use kanesumi_element::{Event, Key, PointerButton};
        match event {
            Event::PointerMove { pos } => {
                if let Some(engine) = ctx.engine() {
                    self.hovered = self.tab_at(engine, ctx.rect(), *pos);
                }
                ctx.invalidate_paint();
            }
            Event::PointerLeave => {
                self.hovered = None;
                ctx.invalidate_paint();
            }
            Event::PointerUp {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                let before = self.selected;
                // 先取出命中结果，结束对 ctx 的不可变借用，再发动作。
                let hit = match ctx.engine() {
                    Some(engine) => self.tab_at(engine, ctx.rect(), *pos),
                    None => None,
                };
                if let Some(i) = hit {
                    self.select(i);
                    if self.selected != before {
                        ctx.emit(TabSelectionChanged(self.selected));
                        ctx.invalidate_paint();
                        ctx.request_anim_frame();
                    }
                    ctx.set_handled();
                }
            }
            Event::KeyDown { key, .. } => {
                // 空表：无相邻页签可切，也不得越界。
                if self.tabs.is_empty() {
                    return;
                }
                let last = self.tabs.len() - 1;
                let next = match key {
                    Key::Left => self.selected.saturating_sub(1),
                    Key::Right => (self.selected + 1).min(last),
                    _ => return,
                };
                if next != self.selected {
                    self.select(next);
                    ctx.emit(TabSelectionChanged(self.selected));
                    ctx.invalidate_paint();
                    ctx.request_anim_frame();
                }
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Group,
            name: String::from("页签"),
            value: self.tabs.get(self.selected).map(|t| t.label.clone()),
            checked: None,
        })
    }
}

#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness(props: LayoutProps) -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroTabRow::new(vec![
                MetroTab::new("邮件"),
                MetroTab::new("日历"),
                MetroTab::new("人脉"),
            ]),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..props
            },
        );
        h.frame();
        (h, id)
    }

    /// 第 `i` 个页签的中心点（表面坐标）—— x 用累计标题宽推。
    fn tab_center(h: &TestHarness, id: WidgetId, i: usize) -> Point {
        let rect = h.rect(id);
        let row = h.tree.get::<MetroTabRow>(id).unwrap();
        let mut x = rect.origin.x;
        for k in 0..i {
            x += row.header_width(&h.engine, k);
        }
        x += row.header_width(&h.engine, i) / 2.0;
        Point::new(x, rect.origin.y + rect.size.height / 2.0)
    }

    #[test]
    fn click_selects_tab_and_reports() {
        let (mut h, id) = harness(LayoutProps::default());
        h.click_at(tab_center(&h, id, 2));
        assert_eq!(
            h.take::<TabSelectionChanged>(),
            vec![(id, TabSelectionChanged(2))]
        );
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 2);
        h.settle();
    }

    #[test]
    fn click_same_tab_reports_nothing() {
        let (mut h, id) = harness(LayoutProps::default());
        h.click_at(tab_center(&h, id, 0));
        assert!(h.take::<TabSelectionChanged>().is_empty(), "已选中不重复发动作");
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 0);
        h.settle();
    }

    #[test]
    fn arrow_keys_move_selection_and_tab_focuses() {
        let (mut h, id) = harness(LayoutProps::default());
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Right);
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 1);
        h.key(Key::Right);
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 2);
        h.key(Key::Right); // 末页：不变、不发动作
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 2);
        h.key(Key::Left);
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 1);
        assert_eq!(
            h.take::<TabSelectionChanged>(),
            vec![
                (id, TabSelectionChanged(1)),
                (id, TabSelectionChanged(2)),
                (id, TabSelectionChanged(1)),
            ]
        );
        h.settle();
    }

    #[test]
    fn indicator_settles_after_selection() {
        let (mut h, id) = harness(LayoutProps::default());
        h.tab();
        h.key(Key::Right);
        // 切换后动画未稳态，settle 应能跑到稳态（管道贴 current）。
        assert!(h.tree.get::<MetroTabRow>(id).unwrap().selection_progress() < 1.0);
        h.settle();
        let p = h.tree.get::<MetroTabRow>(id).unwrap();
        assert!((p.selection_progress() - 1.0).abs() < 0.01, "稳态停在 current");
    }

    #[test]
    fn pointer_move_sets_and_leave_clears_hover() {
        let (mut h, id) = harness(LayoutProps::default());
        h.move_to(tab_center(&h, id, 1));
        h.frame();
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().hovered, Some(1));
        h.tree.pointer_leave();
        h.frame();
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().hovered, None);
    }

    #[test]
    fn sizes_and_passes_insurance_checks() {
        let (h, id) = harness(LayoutProps::default());
        assert!(h.rect(id).size.width > 0.0 && h.rect(id).size.height > 0.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn squeezed_still_passes_insurance_checks() {
        // 页签按内容定宽、总宽可超控件宽；挤到 40 时 render 必须裁到 rect（§6）。
        let (h, id) = harness(LayoutProps {
            width: Some(40.0),
            ..LayoutProps::default()
        });
        assert_eq!(h.rect(id).size.width, 40.0);
        h.assert_contained();
        h.assert_no_hit_outside(id);
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn empty_tab_row_is_safe() {
        // P0-3 回归（树内路径）：空表 measure / 键盘 / 渲染都不得 panic。
        let mut h = TestHarness::new(200.0, 100.0);
        let id = h.tree.insert_with(
            h.root(),
            MetroTabRow::default(),
            LayoutProps {
                h_align: Align::Start,
                v_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        h.frame();
        h.tab();
        assert_eq!(h.tree.focused(), Some(id));
        h.key(Key::Left);
        h.key(Key::Right);
        assert!(h.take::<TabSelectionChanged>().is_empty());
        h.assert_contained();
        h.assert_paint_within(id, Insets::ZERO);
    }

    #[test]
    fn disabled_ignores_input() {
        let (mut h, id) = harness(LayoutProps::default());
        h.tree.set_enabled(id, false);
        h.frame();
        h.click_at(tab_center(&h, id, 1));
        assert!(h.take::<TabSelectionChanged>().is_empty());
        assert_eq!(h.tree.get::<MetroTabRow>(id).unwrap().selected, 0);
        h.settle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find_engine() -> Option<TextEngine> {
        if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        for p in [
            "C:/Windows/Fonts/segoeui.ttf",
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ] {
            if let Ok(e) = TextEngine::load(p) {
                return Some(e);
            }
        }
        None
    }

    #[test]
    fn header_height_matches_spec() {
        assert_eq!(MetroTabRow::default().header_height, 48.0);
        assert_eq!(MetroTabRow::header_style().size, 24.0);
    }

    /// P0-3 回归：空 TabRow 渲染不得 panic（旧实现 `geoms[self.selected]` 越界）。
    #[test]
    fn empty_tab_row_render_is_noop() {
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::default();
        let mut scene = Scene::default();
        row.render(
            &MetroTheme::ether_dark(),
            &engine,
            Rect::new(0.0, 0.0, 300.0, 48.0),
            &mut scene,
        );
        assert!(scene.is_empty(), "空表不得产生任何命令");
    }

    /// P0-3 回归：`selected`/`prev_selected` 是 pub 字段，App 可改成任意值 → 必须夹紧。
    #[test]
    fn out_of_range_selection_is_clamped() {
        let Some(engine) = find_engine() else { return };
        let mut row = MetroTabRow::new(vec![MetroTab::new("A"), MetroTab::new("B")]);
        row.selected = 99;
        row.prev_selected = 99;
        let mut scene = Scene::default();
        row.render(
            &MetroTheme::ether_dark(),
            &engine,
            Rect::new(0.0, 0.0, 300.0, 48.0),
            &mut scene,
        );
        assert!(!scene.is_empty());
    }

    /// P0-2 回归：容器语义 = 裁到自身矩形（成对 PushClip/PopClip）。
    #[test]
    fn tab_row_clips_to_its_rect() {
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("Home"), MetroTab::new("Settings")]);
        let rect = Rect::new(10.0, 5.0, 300.0, 48.0);
        let mut scene = Scene::default();
        row.render(&MetroTheme::ether_dark(), &engine, rect, &mut scene);
        assert!(
            matches!(scene.commands.first(), Some(kanesumi_canvas::SceneCommand::PushClip { rect: r }) if *r == rect)
        );
        assert!(matches!(
            scene.commands.last(),
            Some(kanesumi_canvas::SceneCommand::PopClip)
        ));
    }

    /// P0-1 回归：页签标签是单行省略号（旧实现给 header_height 高的框 + 默认换行 → 会折行）。
    #[test]
    fn tab_labels_are_single_line_ellipsis() {
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("标签很长很长很长很长很长")]);
        let mut scene = Scene::default();
        row.render(
            &MetroTheme::ether_dark(),
            &engine,
            Rect::new(0.0, 0.0, 300.0, 48.0),
            &mut scene,
        );
        assert!(scene.commands.iter().any(|c| matches!(
            c,
            kanesumi_canvas::SceneCommand::Text {
                wrap: false,
                max_lines: Some(1),
                overflow: kanesumi_canvas::TextOverflow::Ellipsis,
                ..
            }
        )));
    }

    #[test]
    fn tab_at_maps_x() {
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("Mail"), MetroTab::new("Calendar")]);
        let w0 = row.header_width(&engine, 0);
        // rect 有非零 origin，验证内部相对化正确
        let rect = Rect::new(50.0, 100.0, 400.0, 48.0);
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(50.0 + 1.0, 100.0 + 10.0)),
            Some(0)
        );
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(50.0 + w0 + 2.0, 100.0 + 10.0)),
            Some(1)
        );
        // rect 外
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(10.0, 10.0)),
            None,
            "点在 rect 外应返回 None"
        );
        // rect 内但 x 超过所有 header 宽度
        assert_eq!(
            row.tab_at(&engine, rect, Point::new(400.0, 110.0)),
            None,
            "命中失败应返回 None"
        );
    }

    #[test]
    fn selected_draws_pipe_only_on_selected() {
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroTabRow::new(vec![MetroTab::new("Mail"), MetroTab::new("Calendar")]);
        row.select(1);
        // V17：动画驱动后管道始终画（滑行插值），不再分"选中才画"；
        // 稳态推进后位置落在 current。
        for _ in 0..120 {
            row.update(1.0 / 60.0);
        }
        let mut scene = Scene::default();
        row.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 300.0, 48.0),
            &mut scene,
        );
        let pipes = scene
            .commands
            .iter()
            .filter(|c| matches!(c, kanesumi_canvas::SceneCommand::FillRect { .. }))
            .count();
        assert_eq!(pipes, 1, "画且只画一条管道");
    }

    #[test]
    fn select_out_of_range_ignored() {
        let mut row = MetroTabRow::new(vec![MetroTab::new("A")]);
        row.select(5);
        assert_eq!(row.selected, 0);
    }

    #[test]
    fn select_starts_pipe_slide_from_prev_to_current() {
        // V17：Default 时无动画，progress 稳态在 1.0（管道贴 current 上，因 prev==cur
        // render 分支不看 progress）。select(new) 才 jump_to(0.0) + set_target(1.0) 起滑，
        // 首帧 progress=0（管道在 prev），若干 update 后 progress→1（管道到 current）。
        let Some(engine) = find_engine() else { return };
        let theme = MetroTheme::ether_dark();
        let mut row = MetroTabRow::new(vec![
            MetroTab::new("Mail"),
            MetroTab::new("Calendar"),
            MetroTab::new("People"),
        ]);
        assert!((row.selection_progress() - 1.0).abs() < 1e-6, "Default 稳态 progress=1");
        row.select(2);
        // 刚 select，progress = 0，管道应在 prev（0）位。
        assert_eq!(row.prev_selected, 0);
        assert!((row.selection_progress() - 0.0).abs() < 1e-6);
        let mut scene0 = Scene::default();
        row.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 600.0, 48.0),
            &mut scene0,
        );
        // 稳态推进
        for _ in 0..120 {
            row.update(1.0 / 60.0);
        }
        assert!((row.selection_progress() - 1.0).abs() < 0.01);
        let mut scene1 = Scene::default();
        row.render(
            &theme,
            &engine,
            Rect::new(0.0, 0.0, 600.0, 48.0),
            &mut scene1,
        );
        // 从两次 render 提取 pipe x（最后一个 FillRect）
        let pipe_x = |s: &Scene| {
            s.commands
                .iter()
                .rev()
                .find_map(|c| match c {
                    kanesumi_canvas::SceneCommand::FillRect { rect, .. } => Some(rect.origin.x),
                    _ => None,
                })
                .unwrap()
        };
        let x_start = pipe_x(&scene0);
        let x_end = pipe_x(&scene1);
        assert!(
            x_end > x_start,
            "管道应从 prev tab（0）滑到 current tab（2）：x_start={x_start}, x_end={x_end}"
        );
    }

    #[test]
    fn select_same_index_is_noop() {
        let mut row = MetroTabRow::new(vec![MetroTab::new("A"), MetroTab::new("B")]);
        row.select(1);
        row.update(1.0);
        assert!((row.selection_progress() - 1.0).abs() < 0.01);
        // 再 select 1 —— 幂等，不应重启动画
        row.select(1);
        assert!((row.selection_progress() - 1.0).abs() < 0.01);
    }

    #[test]
    fn header_style_carries_negative_letter_spacing() {
        // V16: header_style 应带 −0.025em 字距（UWP CharacterSpacing=−25）
        let s = MetroTabRow::header_style();
        assert!((s.letter_spacing_em - (-0.025)).abs() < 1e-6);
        // 24px × −0.025 = −0.6 px/字
        assert!((s.letter_spacing_px() - (-0.6)).abs() < 1e-6);
    }

    #[test]
    fn header_width_reflects_letter_spacing() {
        // V16: header_width（含字距）应 < 无字距 measure（负字距收紧）
        let Some(engine) = find_engine() else { return };
        let row = MetroTabRow::new(vec![MetroTab::new("邮件")]);
        let style = MetroTabRow::header_style();
        let w_spaced = row.header_width(&engine, 0);
        let w_raw = engine.measure("邮件", style.size) + 24.0;
        assert!(
            w_spaced < w_raw,
            "字距 −0.025em 应收紧宽度：w_spaced={w_spaced}, w_raw={w_raw}"
        );
        // 差异 = n_chars × letter_spacing_px = 2 × −0.6 = −1.2
        assert!((w_spaced - w_raw - (2.0 * -0.6)).abs() < 0.01);
    }
}
