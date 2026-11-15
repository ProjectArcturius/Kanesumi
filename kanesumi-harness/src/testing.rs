// testing.rs —— 应用级行为转录夹具：迁移前录快照、迁移后逐行比。
//
// 为什么在 harness 层而不是 element 层：`kanesumi_element::testing::TestHarness` 只驱动一棵裸树，
// 而迁移（应用自造控件 → Kanesumi 控件）要证明的是「**整个应用**对同一串输入的可观察反应不变」——
// 包括 `TreeApp::on_action` / `on_key` / 浮层 / 右键路由。这些只在 `App`（`TreeHost`）层才齐。
// 驱动对象是 `App` 而非 `TreeApp`：迁移前后两版代码都实现 `App`，同一份测试可以原样跑两次。
//
// 转录格式照 Ether `docs/supply/BEHAVIOR_CORPUS.md` §三：一行一个输入，后面只写**变了的**探针值。
// 探针只读可观察量（某点的填充色 / 文本、应用暴露的状态），不读控件内部字段 ——
// 这样迁移换掉内部实现时转录不变，换掉了行为时转录一定变。
//
// 时钟固定 1/60 s 一步（不读墙钟），同输入重跑得到同一份转录（AGENTS §4「生成式产物必须幂等」）。

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use kanesumi_canvas::Scene;
use kanesumi_canvas::scene::SceneCommand;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{Color, Point, Rect, Size};
use kanesumi_element::{Key, Modifiers, PointerButton};

use crate::app::{App, InputEvent};

/// 固定帧步长（秒）。与 `snapshot::render_png` 同值。
pub const FRAME_DT: f64 = 1.0 / 60.0;
/// 每个输入之后最多跑多少帧等画面稳定（2 s）。超过仍在重画 = 常驻动画，按此截断。
pub const SETTLE_FRAMES: u32 = 120;
/// 覆盖快照的环境变量。只在「这次行为变化是有意的、且已写进报告」时设。
pub const BLESS_ENV: &str = "KANESUMI_BLESS";

/// 输入投向哪张表面。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Surface {
    /// 主表面（窗口 / Dock / TopBar 条本身）。
    Main,
    /// 第 `n` 个浮层（`App::floating_layers()` 的下标；Launcher 全屏层、TopBar 面板）。
    Floating(usize),
}

/// 一次出帧后各表面的完整 Scene（浮层每帧强制整幅重画，不受损伤剔除影响）。
#[derive(Default)]
pub struct Frames {
    main: Scene,
    floating: BTreeMap<usize, Scene>,
}

impl Frames {
    /// 取某表面的 Scene；浮层未登记或当前隐藏 → 空 Scene。
    pub fn scene(&self, surface: Surface) -> &Scene {
        static EMPTY: Scene = Scene { commands: Vec::new() };
        match surface {
            Surface::Main => &self.main,
            Surface::Floating(i) => self.floating.get(&i).unwrap_or(&EMPTY),
        }
    }
}

type ProbeFn<A> = Box<dyn Fn(&A, &Frames) -> String>;

/// 应用级驱动器：喂输入、出帧、读探针、记转录。
pub struct AppDriver<A: App> {
    app: A,
    engine: TextEngine,
    size: Size,
    floating: BTreeMap<usize, Size>,
    frames: Frames,
    probes: Vec<(String, ProbeFn<A>)>,
    last: Vec<String>,
    started: bool,
    transcript: String,
}

impl<A: App> AppDriver<A> {
    /// 建驱动器。字体走 `kanesumi_element::testing::test_engine`（找不到就 panic，不静默跳过）。
    /// 弹层不分离：右键菜单 / 下拉等画在主表面 Scene 里，探针直接可见。
    pub fn new(app: A, width: f32, height: f32) -> Self {
        Self {
            app,
            engine: kanesumi_element::testing::test_engine(),
            size: Size::new(width, height),
            floating: BTreeMap::new(),
            frames: Frames::default(),
            probes: Vec::new(),
            last: Vec::new(),
            started: false,
            transcript: String::new(),
        }
    }

    /// 登记一个浮层表面（只有登记过的浮层才出帧、才能当输入目标）。
    pub fn with_floating(mut self, index: usize, width: f32, height: f32) -> Self {
        self.floating.insert(index, Size::new(width, height));
        self
    }

    /// 自定义探针：读应用暴露的可观察状态（如「已拉起的 app_id 列表」）。必须在第一次输入前登记。
    pub fn probe(mut self, name: &str, f: impl Fn(&A, &Frames) -> String + 'static) -> Self {
        assert!(!self.started, "探针必须在第一次输入前登记（否则转录首行不完整）");
        self.probes.push((name.to_string(), Box::new(f)));
        self
    }

    /// 探针：某点最上层的填充色（`#rrggbbaa`；没有填充 = `-`）。悬停 / 按下 / 选中的视觉都落在这里。
    pub fn probe_fill(self, name: &str, surface: Surface, at: Point) -> Self {
        self.probe(name, move |_, f| fill_at(f.scene(surface), at).map_or("-".into(), hex))
    }

    /// 探针：某点最上层的文本内容（没有 = `-`）。
    pub fn probe_text(self, name: &str, surface: Surface, at: Point) -> Self {
        self.probe(name, move |_, f| text_at(f.scene(surface), at).map_or("-".into(), |s| format!("{s:?}")))
    }

    /// 探针：表面上全部可见文本，按 (y, x) 排序、以 `|` 连接。适合「菜单开了几项、写着什么」。
    pub fn probe_texts(self, name: &str, surface: Surface) -> Self {
        self.probe(name, move |_, f| texts(f.scene(surface)).join("|"))
    }

    pub fn app(&self) -> &A {
        &self.app
    }

    pub fn app_mut(&mut self) -> &mut A {
        &mut self.app
    }

    pub fn frames(&self) -> &Frames {
        &self.frames
    }

    /// 当前转录全文。
    pub fn transcript(&self) -> &str {
        &self.transcript
    }

    // ── 输入（每个都：投递 → 跑到稳定 → 记一行）────────────────────────────

    /// 记首行（全部探针的初值）。第一次输入前会自动调用；显式调用可以先做别的准备。
    pub fn start(&mut self) {
        if self.started {
            return;
        }
        self.started = true;
        self.settle();
        self.last = self.read_probes();
        let all: Vec<String> = self.probes.iter().zip(&self.last).map(|((n, _), v)| format!("{n}={v}")).collect();
        self.push_line("init", &all);
    }

    /// 指针移到 `at`。`label` 是转录里的名字（坐标不进转录：坐标属于夹具，改坐标 = 改测试）。
    pub fn hover(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.record(&format!("hover({label})"));
    }

    /// 在 `at` 按下左键（先移过去，与真实指针同序）。
    pub fn press(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.send(surface, pressed(at, PointerButton::Left));
        self.record(&format!("press({label})"));
    }

    /// 在 `at` 松开左键（先移过去：按下后移出再松开 = 取消，正是要转录的情形）。
    pub fn release(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.send(surface, released(at, PointerButton::Left));
        self.record(&format!("release({label})"));
    }

    /// 左键单击（移 → 按 → 松，一行）。
    pub fn click(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.send(surface, pressed(at, PointerButton::Left));
        self.send(surface, released(at, PointerButton::Left));
        self.record(&format!("click({label})"));
    }

    /// 左键双击：第二次按下后追加 `DoubleClick`，与外壳 `ClickTracker` 同序（单击语义不丢）。
    pub fn double_click(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.send(surface, pressed(at, PointerButton::Left));
        self.send(surface, released(at, PointerButton::Left));
        self.send(surface, pressed(at, PointerButton::Left));
        let m = Modifiers::NONE;
        self.send(surface, InputEvent::DoubleClick { x: at.x, y: at.y, button: PointerButton::Left, modifiers: m });
        self.send(surface, released(at, PointerButton::Left));
        self.record(&format!("double_click({label})"));
    }

    /// 右键单击。主表面照外壳路由（`platform.rs` 右键分支）：按下照常投递；释放时先问
    /// `App::context_menu`，有内容且 `open_context_popup` 承载成功 → 释放被消费，否则照常投递。
    pub fn right_click(&mut self, label: &str, surface: Surface, at: Point) {
        self.send(surface, InputEvent::PointerMoved { x: at.x, y: at.y });
        self.send(surface, pressed(at, PointerButton::Right));
        let carried = surface == Surface::Main
            && self
                .app
                .context_menu(at.x, at.y)
                .is_some_and(|items| self.app.open_context_popup(items, at.x, at.y));
        if !carried {
            self.send(surface, released(at, PointerButton::Right));
        }
        self.record(&format!("right_click({label})"));
    }

    /// 滚轮（`dy` 逻辑像素，向下为正；一格 = 48，INTERACTION_CANON §Ⅴ）。在指针当前位置滚
    /// （`InputEvent::Scroll` 的 x / y 是增量，位置取自上一次 `hover` / `press`）。
    pub fn scroll(&mut self, label: &str, surface: Surface, dy: f32) {
        self.send(surface, InputEvent::Scroll { x: 0.0, y: dy, modifiers: Modifiers::NONE });
        self.record(&format!("scroll({label},{dy})"));
    }

    /// 指针离开表面。
    pub fn leave(&mut self, surface: Surface) {
        self.send(surface, InputEvent::PointerLeft);
        self.record("leave");
    }

    /// 按键（主表面键盘焦点）。Tab / Shift+Tab 走 `focus_move`，与外壳同路。
    pub fn key(&mut self, label: &str, key: Key, modifiers: Modifiers) {
        self.start();
        if key == Key::Tab {
            self.app.focus_move(modifiers.shift);
        } else {
            self.send(Surface::Main, InputEvent::KeyPressed { key, modifiers, repeat: false });
        }
        self.record(&format!("key({label})"));
    }

    /// 投递浮层键盘（浮层持焦时的按键，如 Launcher 搜索框）。
    pub fn key_floating(&mut self, label: &str, index: usize, key: Key, modifiers: Modifiers) {
        self.send(Surface::Floating(index), InputEvent::KeyPressed { key, modifiers, repeat: false });
        self.record(&format!("key@{index}({label})"));
    }

    /// 输入一串文本（IME 提交语义，一次原子提交）。
    pub fn type_text(&mut self, text: &str) {
        self.send(Surface::Main, InputEvent::Commit { text: text.to_string() });
        self.record(&format!("type({text:?})"));
    }

    /// 表面键盘焦点变化（失焦 = 轻触关闭弹层的另一条路，CONTEXT_MENU_SPEC §Ⅵ）。
    pub fn focus(&mut self, focused: bool) {
        self.start();
        self.app.focus_changed(focused);
        self.record(if focused { "focus_in" } else { "focus_out" });
    }

    /// 让时间走 `secs` 秒（定时器：悬停提示延时、通知超时、OSD 自动消失）。逐帧推进、每帧出帧。
    pub fn wait(&mut self, secs: f64) {
        self.start();
        let n = (secs / FRAME_DT).round().max(1.0) as u32;
        for _ in 0..n {
            self.frame();
        }
        self.record(&format!("wait({secs}s)"));
    }

    /// 在转录里插一行注释（`# …`），给读转录的人看分段。
    pub fn note(&mut self, text: &str) {
        self.start();
        let _ = writeln!(self.transcript, "# {text}");
    }

    // ── 快照 ──────────────────────────────────────────────────────────────

    /// 与快照文件逐行比对。快照不存在 → 写入并**判红**（新快照必须有人读过、提交过才算数）；
    /// 不一致 → 判红并打印差异，**不改文件**。只有设了 `KANESUMI_BLESS=1` 才覆盖。
    ///
    /// 迁移纪律（Ether `docs/KANESUMI_MIGRATION_PROTOCOL.md`）：快照在迁移前的提交里录好；
    /// 迁移提交不得改动快照文件，有意的行为变化单独一个提交并在报告里逐行说明。
    pub fn assert_golden(&self, path: impl AsRef<Path>) {
        let path = path.as_ref();
        let actual = normalize(&self.transcript);
        let bless = std::env::var(BLESS_ENV).is_ok_and(|v| v == "1");
        let expected = std::fs::read_to_string(path).ok().map(|s| normalize(&s));
        if expected.as_deref() == Some(actual.as_str()) {
            return;
        }
        if bless || expected.is_none() {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).expect("建快照目录失败");
            }
            std::fs::write(path, &actual).expect("写快照失败");
            if bless {
                eprintln!("[{BLESS_ENV}] 已覆盖快照 {}", path.display());
                return;
            }
            panic!("新快照已写入 {}：先通读确认每一行都是你期望的行为，提交它，再重跑。\n{actual}", path.display());
        }
        panic!(
            "行为转录与快照不一致：{}\n（- 快照 / + 本次；迁移不许改快照，先找出是哪处行为丢了）\n{}",
            path.display(),
            line_diff(expected.as_deref().unwrap_or(""), &actual)
        );
    }

    // ── 内部 ──────────────────────────────────────────────────────────────

    fn send(&mut self, surface: Surface, event: InputEvent) {
        self.start();
        match surface {
            Surface::Main => self.app.handle_input(event),
            Surface::Floating(i) => {
                assert!(self.floating.contains_key(&i), "浮层 {i} 未登记：先 with_floating({i}, w, h)");
                self.app.floating_input(i, event);
            }
        }
    }

    /// 出一帧：固定步长推进时钟与逻辑，主表面与已登记且可见的浮层各出一幅完整 Scene。
    fn frame(&mut self) {
        self.app.advance_clock(FRAME_DT);
        self.app.update(FRAME_DT);
        self.app.render_into(&self.engine, self.size, &mut self.frames.main);
        for (&i, &size) in &self.floating {
            if self.app.floating_visible(i) {
                // 浮层恒开损伤剔除；探针要整幅，所以每帧请求整幅重画。
                self.app.floating_full_repaint(i);
                let scene = self.app.render_floating(&self.engine, i, size);
                self.frames.floating.insert(i, scene);
            } else {
                self.frames.floating.remove(&i);
            }
        }
    }

    /// 至少一帧，然后跑到没有表面再要帧为止（上限 `SETTLE_FRAMES`）。
    fn settle(&mut self) {
        self.frame();
        for _ in 1..SETTLE_FRAMES {
            let busy = self.app.needs_redraw()
                || self
                    .floating
                    .keys()
                    .any(|&i| self.app.floating_visible(i) && self.app.floating_needs_redraw(i));
            if !busy {
                break;
            }
            self.frame();
        }
    }

    fn read_probes(&self) -> Vec<String> {
        self.probes.iter().map(|(_, f)| f(&self.app, &self.frames)).collect()
    }

    fn record(&mut self, input: &str) {
        self.settle();
        let now = self.read_probes();
        let changes: Vec<String> = self
            .probes
            .iter()
            .zip(now.iter().zip(&self.last))
            .filter(|(_, (n, l))| n != l)
            .map(|((name, _), (n, _))| format!("{name}={n}"))
            .collect();
        self.push_line(input, &changes);
        self.last = now;
    }

    fn push_line(&mut self, input: &str, changes: &[String]) {
        if changes.is_empty() {
            let _ = writeln!(self.transcript, "{input}");
        } else {
            let _ = writeln!(self.transcript, "{input:<24}{}", changes.join(" "));
        }
    }
}

// ── Scene 读数（不光栅：直接按命令序合成，快且与字体光栅无关）──────────────

/// 遍历可见命令：带上当前裁剪（None = 无裁剪；空交集保持为空，不退化成无裁剪）。
fn visible(scene: &Scene) -> impl Iterator<Item = (&SceneCommand, Option<Rect>)> {
    let mut stack: Vec<Option<Rect>> = Vec::new();
    scene.commands.iter().filter_map(move |cmd| {
        let clip = stack.last().copied().flatten();
        match cmd {
            SceneCommand::PushClip { rect } => {
                let next = match stack.last() {
                    Some(Some(c)) => Some(c.intersect(*rect).unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0))),
                    _ => Some(*rect),
                };
                stack.push(next);
                None
            }
            SceneCommand::PopClip => {
                stack.pop();
                None
            }
            _ => Some((cmd, clip)),
        }
    })
}

fn clip_has(clip: Option<Rect>, p: Point) -> bool {
    clip.is_none_or(|c| c.contains(p))
}

/// `p` 处所有填充 / 描边按 source-over 合成后的颜色（直通 alpha）。没有任何覆盖 → None。
/// 合成而不是取最上层：迁移可能把「底色 + 状态层」合并成一个预混色，看起来一样就该判一样。
pub fn fill_at(scene: &Scene, p: Point) -> Option<Color> {
    let mut out: Option<Color> = None;
    for (cmd, clip) in visible(scene) {
        if !clip_has(clip, p) {
            continue;
        }
        let src = match cmd {
            SceneCommand::FillRect { color, rect, .. } if rect.contains(p) => *color,
            SceneCommand::StrokeRect { color, rect, thickness, .. } => {
                let t = *thickness;
                let inner = Rect::new(rect.origin.x + t, rect.origin.y + t, rect.size.width - 2.0 * t, rect.size.height - 2.0 * t);
                if !rect.contains(p) || inner.contains(p) {
                    continue;
                }
                *color
            }
            _ => continue,
        };
        out = Some(over(src, out.unwrap_or(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 })));
    }
    out
}

fn over(s: Color, d: Color) -> Color {
    let a = s.a + d.a * (1.0 - s.a);
    if a <= 0.0 {
        return Color { r: 0.0, g: 0.0, b: 0.0, a: 0.0 };
    }
    let mix = |sc: f32, dc: f32| (sc * s.a + dc * d.a * (1.0 - s.a)) / a;
    Color { r: mix(s.r, d.r), g: mix(s.g, d.g), b: mix(s.b, d.b), a }
}

/// `p` 处最上层文本命令的内容。
pub fn text_at(scene: &Scene, p: Point) -> Option<&str> {
    let mut hit = None;
    for (cmd, clip) in visible(scene) {
        if let SceneCommand::Text { content, rect, .. } = cmd
            && rect.contains(p)
            && clip_has(clip, p)
        {
            hit = Some(content.as_str());
        }
    }
    hit
}

/// 可见文本（与裁剪有交集、非空），按 (y, x) 排序。
pub fn texts(scene: &Scene) -> Vec<&str> {
    let mut v: Vec<(f32, f32, &str)> = visible(scene)
        .filter_map(|(cmd, clip)| match cmd {
            SceneCommand::Text { content, rect, .. }
                if !content.is_empty() && clip.is_none_or(|c| c.intersect(*rect).is_some_and(|r| r.size.width > 0.0 && r.size.height > 0.0)) =>
            {
                Some((rect.origin.y, rect.origin.x, content.as_str()))
            }
            _ => None,
        })
        .collect();
    v.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    v.into_iter().map(|t| t.2).collect()
}

/// `#rrggbbaa`（8 位量化：低于 1/255 的浮点差异不进转录）。
pub fn hex(c: Color) -> String {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}{:02x}", q(c.r), q(c.g), q(c.b), q(c.a))
}

fn pressed(at: Point, button: PointerButton) -> InputEvent {
    InputEvent::PointerPressed { x: at.x, y: at.y, button, modifiers: Modifiers::NONE }
}

fn released(at: Point, button: PointerButton) -> InputEvent {
    InputEvent::PointerReleased { x: at.x, y: at.y, button, modifiers: Modifiers::NONE }
}

/// 换行统一为 `\n`、去行尾空白、保证末尾一个换行（Windows 检出的 CRLF 不算差异）。
fn normalize(s: &str) -> String {
    let mut out: String = s.lines().map(|l| l.trim_end()).collect::<Vec<_>>().join("\n");
    out.push('\n');
    out
}

/// 最小 LCS 行差异（转录通常几十行，O(n·m) 够用）。只打印改动行及前后各一行上下文。
fn line_diff(a: &str, b: &str) -> String {
    let (a, b): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let mut l = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            l[i][j] = if a[i] == b[j] { l[i + 1][j + 1] + 1 } else { l[i + 1][j].max(l[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut ops) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] {
            ops.push((' ', a[i]));
            (i, j) = (i + 1, j + 1);
        } else if j < b.len() && (i == a.len() || l[i][j + 1] >= l[i + 1][j]) {
            ops.push(('+', b[j]));
            j += 1;
        } else {
            ops.push(('-', a[i]));
            i += 1;
        }
    }
    let near = |k: usize| ops[k.saturating_sub(1)..(k + 2).min(ops.len())].iter().any(|o| o.0 != ' ');
    let mut out = String::new();
    for (k, (op, line)) in ops.iter().enumerate() {
        if near(k) {
            let _ = writeln!(out, "{op} {line}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::AppConfig;
    use crate::role::EtherRole;
    use crate::tree_host::{TreeApp, TreeHost};
    use kanesumi_controls::{ButtonClicked, MetroButton};
    use kanesumi_element::widgets::Stack;
    use kanesumi_element::{Action, Align, LayoutProps, Tree, WidgetId};

    struct Demo {
        config: AppConfig,
        button: Option<WidgetId>,
        clicks: u32,
    }

    impl TreeApp for Demo {
        fn config(&self) -> &AppConfig {
            &self.config
        }
        fn build(&mut self, tree: &mut Tree) {
            let col = tree.insert(tree.root(), Stack::column());
            let start = LayoutProps { h_align: Align::Start, ..LayoutProps::default() };
            self.button = Some(tree.insert_with(col, MetroButton::new("确定"), start));
        }
        fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
            if action.is::<ButtonClicked>() {
                self.clicks += 1;
            }
        }
    }

    /// 驱动器 + 按钮中心 + 一处界外点。
    fn driver() -> (AppDriver<TreeHost<Demo>>, Point, Point) {
        let host = TreeHost::new(Demo {
            config: AppConfig::new("org.ether.test", "testkit", EtherRole::Browser, 400.0, 300.0),
            button: None,
            clicks: 0,
        });
        let mut d = AppDriver::new(host, 400.0, 300.0);
        // 先出一帧拿到按钮几何，再登记探针（探针只要求在首行 `init` 之前登记）。
        d.frame();
        let id = d.app().app().button.expect("按钮已建");
        let center = d.app().tree().rect(id).expect("按钮已排版").center();
        let outside = Point::new(380.0, 280.0);
        let d = d
            .probe_fill("btn", Surface::Main, center)
            .probe_text("label", Surface::Main, center)
            .probe("clicks", |h: &TreeHost<Demo>, _| h.app().clicks.to_string());
        (d, center, outside)
    }

    /// 转录只写变化：悬停改底色、单击计数 +1、离开底色复原；标签全程不变所以只在首行出现。
    #[test]
    fn transcript_lists_only_changed_probes() {
        let (mut d, btn, _) = driver();
        d.hover("btn", Surface::Main, btn);
        d.click("btn", Surface::Main, btn);
        d.leave(Surface::Main);
        let t = d.transcript().to_string();
        eprintln!("{t}"); // `--nocapture` 时给人看一份真实转录长什么样
        let lines: Vec<&str> = t.lines().collect();
        assert_eq!(lines.len(), 4, "{t}");
        assert!(lines[0].starts_with("init") && lines[0].contains("label=\"确定\"") && lines[0].contains("clicks=0"), "{t}");
        assert!(lines[1].starts_with("hover(btn)") && lines[1].contains("btn=#"), "悬停应改底色：{t}");
        assert!(lines[2].contains("clicks=1") && !lines[2].contains("label="), "{t}");
        assert!(lines[3].starts_with("leave") && lines[3].contains("btn=#"), "离开应复原底色：{t}");
        let init_fill = lines[0].split("btn=").nth(1).unwrap().split(' ').next().unwrap();
        let left_fill = lines[3].split("btn=").nth(1).unwrap().split(' ').next().unwrap();
        assert_eq!(init_fill, left_fill, "离开后底色应回到初值：{t}");
    }

    /// 按下后移出再松开 = 取消（BEHAVIOR_CORPUS §三 例），计数不变。
    #[test]
    fn press_then_release_outside_does_not_click() {
        let (mut d, btn, out) = driver();
        d.press("btn", Surface::Main, btn);
        d.release("outside", Surface::Main, out);
        assert_eq!(d.app().app().clicks, 0, "{}", d.transcript());
        assert!(!d.transcript().contains("clicks=1"));
    }

    /// 同输入重跑 = 同转录（固定时钟，不读墙钟）。
    #[test]
    fn transcript_is_deterministic() {
        let run = || {
            let (mut d, btn, out) = driver();
            d.hover("btn", Surface::Main, btn);
            d.wait(0.5);
            d.press("btn", Surface::Main, btn);
            d.release("outside", Surface::Main, out);
            d.click("btn", Surface::Main, btn);
            d.transcript().to_string()
        };
        assert_eq!(run(), run());
    }

    fn tmp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("kn-testkit-{}-{name}.txt", std::process::id()))
    }

    /// 快照三态：缺失 → 写入并判红；一致 → 绿；不一致 → 判红且不改文件。
    #[test]
    fn golden_missing_writes_then_matches_then_rejects_drift() {
        if std::env::var(BLESS_ENV).is_ok() {
            return; // 覆盖模式下本用例的「判红」语义不成立
        }
        let path = tmp("golden");
        let _ = std::fs::remove_file(&path);
        let (mut d, btn, _) = driver();
        d.click("btn", Surface::Main, btn);
        let first = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.assert_golden(&path)));
        assert!(first.is_err(), "缺失快照必须判红");
        assert!(path.exists(), "缺失快照应已写入");
        d.assert_golden(&path); // 一致 → 绿

        // CRLF / 行尾空白不算差异（Windows 检出）。
        let crlf = std::fs::read_to_string(&path).unwrap().replace('\n', "  \r\n");
        std::fs::write(&path, &crlf).unwrap();
        d.assert_golden(&path);

        let drifted = crlf.replace("clicks=1", "clicks=2");
        std::fs::write(&path, &drifted).unwrap();
        let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| d.assert_golden(&path)))
            .expect_err("行为漂移必须判红");
        let msg = err.downcast_ref::<String>().cloned().unwrap_or_default();
        assert!(msg.contains("- ") && msg.contains("clicks=2") && msg.contains("+ ") && msg.contains("clicks=1"), "{msg}");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), drifted, "不一致时不得改快照文件");
        let _ = std::fs::remove_file(&path);
    }

    /// 合成：半透明状态层叠在底色上 = 预混色（迁移把两层合一层不算行为变化）；裁剪外不计。
    #[test]
    fn fill_at_composites_and_respects_clip() {
        let base = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };
        let layer = Color { r: 1.0, g: 1.0, b: 1.0, a: 0.5 };
        let r = Rect::new(0.0, 0.0, 100.0, 100.0);
        let mut two = Scene::default();
        two.commands.push(SceneCommand::FillRect { color: base, rect: r, corner_radius: 0.0 });
        two.commands.push(SceneCommand::FillRect { color: layer, rect: r, corner_radius: 0.0 });
        let mut one = Scene::default();
        let mixed = Color { r: 0.5, g: 0.5, b: 1.0, a: 1.0 };
        one.commands.push(SceneCommand::FillRect { color: mixed, rect: r, corner_radius: 0.0 });
        let p = Point::new(10.0, 10.0);
        assert_eq!(fill_at(&two, p).map(hex), fill_at(&one, p).map(hex));

        let mut clipped = Scene::default();
        clipped.commands.push(SceneCommand::PushClip { rect: Rect::new(50.0, 50.0, 10.0, 10.0) });
        clipped.commands.push(SceneCommand::FillRect { color: base, rect: r, corner_radius: 0.0 });
        clipped.commands.push(SceneCommand::PopClip);
        assert_eq!(fill_at(&clipped, p), None, "裁剪外不计");
        assert!(fill_at(&clipped, Point::new(55.0, 55.0)).is_some());
        assert_eq!(fill_at(&Scene::default(), p), None);
    }
}
