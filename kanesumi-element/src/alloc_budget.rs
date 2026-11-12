// alloc_budget.rs —— 帧内分配预算（测试专用）。参 BEHAVIOR_CORPUS §一-3。
//
// 为什么要有它：「性能很差」在元素树里最常见的形态不是某一段慢，而是**稳态帧还在分配** ——
// 画面没变，每帧却新建一份 Scene、逐个 clone 文本、让分配器与 GC 似的抖动。这种事在
// 60 Hz 下看不出来，在真机 L470 上就是掉帧。把它钉成断言，比事后看日志可靠。
//
// 结论（本文件的两条断言就是数字的出处）：
//   - 稳态帧（无内容变化）：**0 次**分配 —— 判据是 `Tree::can_reuse_scene`，缓冲原样不动；
//   - 内容变化帧：见 `content_change_budget` 里的实测上界。
//
// 计数是**本线程**的：`cargo test` 默认多线程跑，全局计数会被别的测试污染。

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

thread_local! {
    /// 本线程累计分配次数（alloc / alloc_zeroed / realloc 各计一次）。
    static ALLOCS: Cell<usize> = const { Cell::new(0) };
}

/// 计数分配器：只数次数，不改变分配行为。
struct CountingAlloc;

unsafe impl GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // `try_with`：线程销毁期间 TLS 可能已不可用，那时刻的分配不该 panic。
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
        unsafe { System.realloc(ptr, layout, new_size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn allocations() -> usize {
    ALLOCS.with(|c| c.get())
}

/// 统计 `f` 期间的分配次数。
fn count_allocs<T>(f: impl FnOnce() -> T) -> (T, usize) {
    let before = allocations();
    let out = f();
    (out, allocations() - before)
}

#[cfg(test)]
mod tests {
    use super::count_allocs;
    use crate::diagnostics;
    use crate::diagnostics::strict_test_lock;
    use crate::testing::TestHarness;
    use crate::widgets::{Border, Label};
    use crate::{Align, Insets, LayoutProps, PointerButton, Tree};
    use kanesumi_canvas::Scene;
    use kanesumi_canvas::SceneCommand;
    use kanesumi_core::{Point, Size};

    fn fixed(x: f32, y: f32, w: f32, h: f32) -> LayoutProps {
        LayoutProps {
            width: Some(w),
            height: Some(h),
            margin: Insets { left: x, top: y, right: 0.0, bottom: 0.0 },
            h_align: Align::Start,
            v_align: Align::Start,
            ..LayoutProps::default()
        }
    }

    /// 一棵「像真界面」的树：容器 + 文本 + 图形，命令条数 > 1 才有代表性。
    fn build(tree: &mut Tree) -> crate::WidgetId {
        let root = tree.root();
        let card = tree.insert_with(root, Border::new(), fixed(10.0, 10.0, 200.0, 80.0));
        let inner = tree.insert_with(card, Border::new(), fixed(4.0, 4.0, 120.0, 40.0));
        let label = tree.insert_with(root, Label::new("Kanesumi"), fixed(20.0, 120.0, 180.0, 24.0));
        let _ = inner;
        label
    }

    /// 稳态帧（无内容变化）必须**一次分配都没有**。
    ///
    /// 这不是「优化目标」而是回归门：谁在帧里加了 `Vec::new` / `format!` / `to_string`，
    /// 这条断言立刻变红。改动若确有需要，先想清楚它是不是每帧都得做。
    #[test]
    fn steady_state_frame_allocates_nothing() {
        // 拿锁：别让并行的严格模式用例在我计量期间把开关打开（严格态每帧会多分配）。
        let _guard = strict_test_lock();
        let mut h = TestHarness::new(320.0, 240.0);
        build(&mut h.tree);
        let mut scene = Scene::default();
        let size = h.size;
        let dt = 1.0 / 60.0;

        // 首帧：布局 + 全量拼接（不计量）。
        let first = h.tree.frame_into(&h.engine, size, dt, &mut scene);
        assert!(!first.reused, "首帧不可能复用（缓冲是空的）");
        assert!(!scene.commands.is_empty(), "测试树必须真的画出东西");

        // 第二帧起是稳态：同一份缓冲、没有任何失效。
        let (out, allocs) = count_allocs(|| h.tree.frame_into(&h.engine, size, dt, &mut scene));
        assert!(out.reused, "无内容变化的帧必须走复用路径");
        assert!(out.damage.is_none() && !out.full, "稳态帧不该报损伤");
        assert_eq!(
            allocs, 0,
            "稳态帧分配次数必须为 0（实测 {allocs}）：有东西每帧在分配"
        );

        // 连续多帧仍为 0（防止「第二帧碰巧不分配」）。
        let mut total = 0;
        for _ in 0..5 {
            let (_, n) = count_allocs(|| h.tree.frame_into(&h.engine, size, dt, &mut scene));
            total += n;
        }
        assert_eq!(total, 0, "连续 5 个稳态帧共分配 {total} 次，必须为 0");
    }

    /// 内容变化帧的分配上界。
    ///
    /// 判据取 **5 次同样改动的最小值**：同一段代码的分配次数是确定的，噪声只会把它_抬高_
    /// （并行跑整个测试集时实测偶尔 10 → 17，多出来的是本线程上其它一次性初始化：环境变量
    /// 缓存、日志器、TLS 之类）。取最小值 = 「这段代码自己需要几次」，取最大值量到的是别的
    /// 东西。**上限仍按实测最小值定**：最小值 > 12 就说明确实多分配了。
    ///
    /// 实测（debug，本机）：改变一个叶子节点的固定宽 → **10 次**分配
    /// （Scene 命令 Vec 增长 + 其中若干条命令的克隆 + `painted` 列表 + 布局期临时量）。
    /// 上界取 12 = 10 的 1.2 倍：够容纳「同一处改动多画一两条命令」，又不给「每帧新建整份
    /// 场景」留余地（那种写法是几十到上百次，离 12 很远）。
    #[test]
    fn content_change_frame_stays_within_budget() {
        let _guard = strict_test_lock();
        const BUDGET: usize = 12;
        let mut h = TestHarness::new(320.0, 240.0);
        let label = build(&mut h.tree);
        let mut scene = Scene::default();
        let size = h.size;
        let dt = 1.0 / 60.0;
        h.tree.frame_into(&h.engine, size, dt, &mut scene);
        h.tree.frame_into(&h.engine, size, dt, &mut scene);
        // 把一次性初始化（严格模式的环境变量缓存等）挪到计量之外。
        let _ = crate::diagnostics::strict();

        let mut samples = Vec::new();
        for i in 0..5 {
            let w = if i % 2 == 0 { 160.0 } else { 180.0 };
            // 内容变化：改固定宽（量测 / 排布 / 重画都真的跑一遍，且不动树结构）。
            h.tree.update_props(label, |p| p.width = Some(w));
            let (out, allocs) = count_allocs(|| h.tree.frame_into(&h.engine, size, dt, &mut scene));
            assert!(!out.reused, "内容变了必须重组，不得复用");
            assert!(out.damage.is_some(), "文本变了必须报损伤");
            samples.push(allocs);
        }
        let min = *samples.iter().min().unwrap();
        println!("内容变化帧分配次数 = {samples:?}（最小值 {min}，上界 {BUDGET}）");
        assert!(
            min <= BUDGET,
            "内容变化帧最少也要 {min} 次分配（样本 {samples:?}），超过上界 {BUDGET}"
        );
    }

    /// 缓冲被别人追加过（外壳叠右键菜单）时不得复用 —— 否则菜单会残留到下一帧。
    #[test]
    fn foreign_commands_in_buffer_disable_reuse() {
        let _guard = strict_test_lock();
        let mut h = TestHarness::new(320.0, 240.0);
        build(&mut h.tree);
        let mut scene = Scene::default();
        let size = h.size;
        let dt = 1.0 / 60.0;
        h.tree.frame_into(&h.engine, size, dt, &mut scene);
        h.tree.frame_into(&h.engine, size, dt, &mut scene);

        // 模拟外壳叠一层菜单命令。
        scene.fill_rect(
            kanesumi_core::Color::new(0.1, 0.1, 0.1, 1.0),
            kanesumi_core::Rect::new(0.0, 0.0, 10.0, 10.0),
        );
        let out = h.tree.frame_into(&h.engine, size, dt, &mut scene);
        assert!(!out.reused, "缓冲被追加过，必须重组");
        assert!(
            !scene
                .commands
                .iter()
                .any(|c| matches!(c, SceneCommand::FillRect { rect, .. } if rect.size.width == 10.0 && rect.size.height == 10.0)),
            "重组必须把外来命令清掉"
        );
    }

    // ── 严格模式：几何不变量 / 空命中表 ──────────────────────────────────────

    /// NaN margin → arrange 产出 NaN 坐标：默认只打印，严格模式 panic。
    #[test]
    fn nan_geometry_prints_by_default_and_panics_in_strict_mode() {
        let _guard = strict_test_lock();
        fn build_bad(h: &mut TestHarness) {
            let root = h.root();
            h.tree.insert_with(
                root,
                Border::new(),
                LayoutProps {
                    width: Some(50.0),
                    height: Some(50.0),
                    margin: Insets { left: f32::NAN, top: 0.0, right: 0.0, bottom: 0.0 },
                    ..LayoutProps::default()
                },
            );
        }

        diagnostics::set_strict_for_test(Some(false));
        let mut h = TestHarness::new(200.0, 120.0);
        build_bad(&mut h);
        h.frame(); // 默认路径：不 panic（只记 error 日志）
        diagnostics::set_strict_for_test(None);

        diagnostics::set_strict_for_test(Some(true));
        let mut h = TestHarness::new(200.0, 120.0);
        build_bad(&mut h);
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let _ = h.frame(); }));
        diagnostics::set_strict_for_test(None);
        assert!(hit.is_err(), "严格模式下 NaN 几何必须 panic");
    }

    /// 命中表为空（界面没建起来）却收到指针事件：默认无所谓，严格模式 panic。
    #[test]
    fn pointer_into_empty_hit_table_panics_in_strict_mode() {
        let _guard = strict_test_lock();
        let mut h = TestHarness::new(200.0, 120.0);
        h.frame();

        diagnostics::set_strict_for_test(Some(false));
        h.tree.pointer_down(Point::new(10.0, 10.0), PointerButton::Left, crate::Modifiers::NONE);

        diagnostics::set_strict_for_test(Some(true));
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            h.tree.pointer_down(Point::new(10.0, 10.0), PointerButton::Left, crate::Modifiers::NONE);
        }));
        diagnostics::set_strict_for_test(None);
        assert!(hit.is_err(), "严格模式下空命中表的指针事件必须 panic");
    }

    /// 有序界面里指针事件**不得**误报（严格模式下的反向用例）。
    #[test]
    fn pointer_into_built_ui_is_quiet_in_strict_mode() {
        let _guard = strict_test_lock();
        let mut h = TestHarness::new(320.0, 240.0);
        build(&mut h.tree);
        h.frame();
        diagnostics::set_strict_for_test(Some(true));
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            h.tree.pointer_move(Point::new(30.0, 30.0));
            h.tree.pointer_down(Point::new(30.0, 30.0), PointerButton::Left, crate::Modifiers::NONE);
            h.tree.pointer_up(Point::new(30.0, 30.0), PointerButton::Left, crate::Modifiers::NONE);
        }));
        diagnostics::set_strict_for_test(None);
        assert!(hit.is_ok(), "正常界面在严格模式下不得因指针事件 panic");
    }

    /// 严格模式下的非有限命令检查：控件画出 NaN 矩形 → panic。
    #[test]
    fn non_finite_commands_panic_in_strict_mode() {
        let _guard = strict_test_lock();
        struct BadPaint;
        impl crate::Widget for BadPaint {
            fn measure(&mut self, _ctx: &mut crate::MeasureCtx, _a: Size) -> Size {
                Size::new(20.0, 20.0)
            }
            fn paint(&mut self, _ctx: &mut crate::PaintCtx, scene: &mut Scene) {
                scene.fill_rect(
                    kanesumi_core::Color::new(1.0, 0.0, 0.0, 1.0),
                    kanesumi_core::Rect::new(0.0, 0.0, f32::NAN, 4.0),
                );
            }
        }
        let mut h = TestHarness::new(200.0, 120.0);
        let root = h.root();
        h.tree.insert_with(root, BadPaint, LayoutProps::default());

        diagnostics::set_strict_for_test(Some(true));
        let hit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let _ = h.frame(); }));
        diagnostics::set_strict_for_test(None);
        assert!(hit.is_err(), "严格模式下非有限命令必须 panic");
    }
}
