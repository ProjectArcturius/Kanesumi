// testing.rs —— 无外壳测试夹具。参 ELEMENT_TREE §Ⅹ E1。
//
// 在没有 Wayland 会话的环境（Windows、CI、Arch 终端）里驱动整棵树：出帧、指针、键盘、IME，
// 并提供控件迁移时**每个控件都必须通过**的三条通用保险断言（ROADMAP M2-5）：
//   1. `assert_contained`       —— 所有可见节点矩形 ⊆ 父矩形；
//   2. `assert_no_hit_outside`  —— 控件矩形外 1px 不命中该控件（及其子树）；
//   3. `assert_paint_within`    —— 控件绘制命令 ⊆ rect ⊕ paint_overflow ⊕ 焦点视觉。
//
// 字体找不到时 **panic 并说明**，不静默跳过（静默跳过的测试比没有测试更危险，审计 §Ⅴ-5）。

use std::path::PathBuf;

use kanesumi_canvas::SceneCommand;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Point, Rect, Size};

use crate::event::{Key, Modifiers, PointerButton};
use crate::id::WidgetId;
use crate::tree::{Action, FrameOutput, Tree};

/// 测试字体查找：`KANESUMI_TEST_FONT` → Linux 常见 CJK / 拉丁字体 → Windows 雅黑 / Segoe UI。
pub fn find_test_font() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        let p = PathBuf::from(p);
        if p.exists() {
            return Some(p);
        }
    }
    [
        "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
        "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/segoeui.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

/// 测试用 `TextEngine`。找不到字体直接 panic（带修复提示）。
pub fn test_engine() -> TextEngine {
    let path = find_test_font().expect("测试字体缺失：请设置 KANESUMI_TEST_FONT=<字体路径>");
    TextEngine::load(&path).unwrap_or_else(|e| panic!("测试字体加载失败 {path:?}: {e:?}"))
}

/// 测试夹具：树 + 字体 + 表面尺寸。
pub struct TestHarness {
    pub tree: Tree,
    pub engine: TextEngine,
    pub size: Size,
    pub last: FrameOutput,
}

impl TestHarness {
    pub fn new(width: f32, height: f32) -> Self {
        Self::with_theme(width, height, MetroTheme::ether_dark())
    }

    pub fn with_theme(width: f32, height: f32, theme: MetroTheme) -> Self {
        Self {
            tree: Tree::new(theme),
            engine: test_engine(),
            size: Size::new(width, height),
            last: FrameOutput::default(),
        }
    }

    pub fn root(&self) -> WidgetId {
        self.tree.root()
    }

    /// 出一帧（dt = 1/60s）。
    pub fn frame(&mut self) -> &FrameOutput {
        self.last = self.tree.frame(&self.engine, self.size, 1.0 / 60.0);
        &self.last
    }

    /// 连续出帧直到动画停止（最多 10 秒模拟时间，防死循环）。
    pub fn settle(&mut self) {
        self.frame();
        for _ in 0..600 {
            if !self.last.animating && !self.tree.needs_frame() {
                return;
            }
            self.frame();
        }
        panic!("动画 10 秒内未到稳态");
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.size = Size::new(width, height);
    }

    pub fn rect(&self, id: WidgetId) -> Rect {
        self.tree.rect(id).expect("节点不存在")
    }

    pub fn center(&self, id: WidgetId) -> Point {
        self.rect(id).center()
    }

    pub fn move_to(&mut self, pos: Point) {
        self.tree.pointer_move(pos);
    }

    pub fn press_at(&mut self, pos: Point, button: PointerButton) {
        self.tree.pointer_move(pos);
        self.tree.pointer_down(pos, button, Modifiers::NONE, false);
    }

    pub fn release_at(&mut self, pos: Point, button: PointerButton) {
        self.tree.pointer_up(pos, button, Modifiers::NONE);
    }

    /// 左键点击一个坐标（移动 → 按下 → 释放），随后出一帧。
    pub fn click_at(&mut self, pos: Point) {
        self.press_at(pos, PointerButton::Left);
        self.release_at(pos, PointerButton::Left);
        self.frame();
    }

    /// 左键点击控件中心。
    pub fn click(&mut self, id: WidgetId) {
        let c = self.center(id);
        self.click_at(c);
    }

    pub fn right_click(&mut self, id: WidgetId) {
        let c = self.center(id);
        self.press_at(c, PointerButton::Right);
        self.release_at(c, PointerButton::Right);
        self.frame();
    }

    /// 按键；返回是否被消费。随后出一帧。
    pub fn key(&mut self, key: Key) -> bool {
        self.key_with(key, Modifiers::NONE)
    }

    pub fn key_with(&mut self, key: Key, modifiers: Modifiers) -> bool {
        let handled = self.tree.key_down(key, modifiers);
        self.frame();
        handled
    }

    pub fn tab(&mut self) {
        self.key(Key::Tab);
    }

    pub fn shift_tab(&mut self) {
        self.key_with(
            Key::Tab,
            Modifiers {
                shift: true,
                ..Modifiers::NONE
            },
        );
    }

    /// 以 IME 提交的方式输入文本（与外壳文本输入同路径）。
    pub fn type_text(&mut self, text: &str) {
        self.tree.commit(text.to_string());
        self.frame();
    }

    pub fn take_actions(&mut self) -> Vec<(WidgetId, Action)> {
        self.tree.take_actions()
    }

    /// 取走本批动作中类型为 `A` 的那些（其余丢弃）。
    pub fn take<A: 'static>(&mut self) -> Vec<(WidgetId, A)> {
        self.tree
            .take_actions()
            .into_iter()
            .filter_map(|(id, a)| a.downcast::<A>().ok().map(|a| (id, *a)))
            .collect()
    }

    // ── 保险断言 ────────────────────────────────────────────────────────────

    /// 所有可见节点矩形 ⊆ 父矩形。
    pub fn assert_contained(&self) {
        let mut bad = Vec::new();
        for id in self.tree.all_ids() {
            let (Some(r), Some(p)) = (self.tree.rect(id), self.tree.parent(id)) else {
                continue;
            };
            if !self.tree.is_visible(id) {
                continue;
            }
            let pr = self.rect(p);
            if !within(r, pr, 0.01) {
                bad.push(format!("{} {r:?} ⊄ {} {pr:?}", self.tree.type_name(id), self.tree.type_name(p)));
            }
        }
        assert!(bad.is_empty(), "矩形越出父矩形：\n{}", bad.join("\n"));
    }

    /// 控件矩形外 1px（四边中点 + 四角）不命中该控件及其子树。
    pub fn assert_no_hit_outside(&self, id: WidgetId) {
        let r = self.rect(id);
        let (x0, y0, x1, y1) = (r.origin.x - 1.0, r.origin.y - 1.0, r.right() + 1.0, r.bottom() + 1.0);
        let (cx, cy) = (r.center().x, r.center().y);
        for p in [
            Point::new(x0, cy),
            Point::new(x1, cy),
            Point::new(cx, y0),
            Point::new(cx, y1),
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x0, y1),
            Point::new(x1, y1),
        ] {
            if let Some(h) = self.tree.hit(p) {
                assert!(
                    !self.tree.is_ancestor_or_self(id, h),
                    "{} 矩形 {r:?} 外 1px 的点 {p:?} 仍命中其子树（{}）",
                    self.tree.type_name(id),
                    self.tree.type_name(h)
                );
            }
        }
    }

    /// 控件自绘命令 ⊆ rect ⊕ 焦点视觉（2px）⊕ `extra`（控件声明的 `paint_overflow`）。
    pub fn assert_paint_within(&self, id: WidgetId, extra: crate::props::Insets) {
        let allowed = crate::props::Insets::all(2.0).inflate(extra.inflate(self.rect(id)));
        for cmd in self.tree.painted(id) {
            let r = match cmd {
                SceneCommand::FillRect { rect, .. }
                | SceneCommand::StrokeRect { rect, .. }
                | SceneCommand::Text { rect, .. }
                | SceneCommand::Image { rect, .. }
                | SceneCommand::PushClip { rect } => *rect,
                SceneCommand::Arc {
                    center,
                    radius,
                    thickness,
                    ..
                } => {
                    let e = radius + thickness / 2.0;
                    Rect::new(center.x - e, center.y - e, e * 2.0, e * 2.0)
                }
                SceneCommand::Triangle { p0, p1, p2, .. } => {
                    let x0 = p0.x.min(p1.x).min(p2.x);
                    let y0 = p0.y.min(p1.y).min(p2.y);
                    let x1 = p0.x.max(p1.x).max(p2.x);
                    let y1 = p0.y.max(p1.y).max(p2.y);
                    Rect::new(x0, y0, x1 - x0, y1 - y0)
                }
                SceneCommand::PopClip => continue,
            };
            assert!(
                within(r, allowed, 0.5),
                "{} 绘制越界：{cmd:?} ⊄ {allowed:?}",
                self.tree.type_name(id)
            );
        }
    }
}

fn within(inner: Rect, outer: Rect, eps: f32) -> bool {
    inner.origin.x >= outer.origin.x - eps
        && inner.origin.y >= outer.origin.y - eps
        && inner.right() <= outer.right() + eps
        && inner.bottom() <= outer.bottom() + eps
}
