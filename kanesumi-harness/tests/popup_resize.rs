// 弹层内容尺寸变化：**同一个表面**换缓冲，不重建、不重放入场。参 docs/FEEL_PLAN.md §Ⅰ-4。
//
// 回归对象：tb-net 面板点一行展开 → 整块面板销毁重建 + 重播 300 ms 入场。
// 外壳侧判据（`(key, grab)`，`rect` 不再参与）的单元测试在 platform/popups.rs；
// 本文件用像素断言证明尺寸变化真的落到了内容上：展开后底部多出一行、
// 收起后整幅仍然画满（不露未绘制区）。

use std::cell::Cell;
use std::rc::Rc;

use kanesumi_core::{Color, Rect, Size};
use kanesumi_harness::app::{App, AppConfig};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{
    Action, LayoutProps, MeasureCtx, PaintCtx, PopupSpec, Tree, Widget, WidgetId,
};
use kanesumi_harness::role::EtherRole;
use kanesumi_harness::{CpuRenderer, TreeApp, TreeHost};

const W: f32 = 120.0;
const HEADER_H: f32 = 80.0;
const ROW_H: f32 = 44.0;
/// 0.2 灰：面板底。
const BG: (u8, u8, u8) = (51, 51, 51);
/// 0.4 灰：展开出来的那一行。
const ROW: (u8, u8, u8) = (102, 102, 102);

/// 高度随状态走的一行：模拟「展开 / 收起一行」的内容。
struct ExpandRow {
    height: Rc<Cell<f32>>,
}

impl Widget for ExpandRow {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        Size::new(W, self.height.get())
    }
    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut kanesumi_canvas::Scene) {
        let h = self.height.get();
        if h <= 0.0 {
            return;
        }
        let r = ctx.rect();
        scene.fill_rect(
            Color::rgb(0.4, 0.4, 0.4),
            Rect::new(r.origin.x, r.origin.y, r.size.width, h),
        );
    }
}

/// 带一个弹层的应用：弹层 = 固定 80 高的底板 + 高度可变的一行。
struct PanelApp {
    config: AppConfig,
    row: Rc<Cell<f32>>,
    anchor: Option<WidgetId>,
    row_id: Option<WidgetId>,
}

impl PanelApp {
    fn new(row: Rc<Cell<f32>>) -> Self {
        Self {
            config: AppConfig::new(
                "org.ether.test",
                "popup-resize",
                EtherRole::Browser,
                400.0,
                300.0,
            ),
            row,
            anchor: None,
            row_id: None,
        }
    }

    fn open(&mut self, tree: &mut Tree) {
        let a = self.anchor.unwrap();
        let p = tree.open_popup(
            Border::new().background(Color::rgb(0.2, 0.2, 0.2)),
            PopupSpec {
                anchor: Some(a),
                ..PopupSpec::default()
            },
        );
        let col = tree.insert(p, Stack::column());
        tree.insert_with(
            col,
            Border::new().background(Color::rgb(0.2, 0.2, 0.2)),
            LayoutProps {
                width: Some(W),
                height: Some(HEADER_H),
                ..LayoutProps::default()
            },
        );
        let row = tree.insert_with(
            col,
            ExpandRow {
                height: self.row.clone(),
            },
            LayoutProps {
                width: Some(W),
                ..LayoutProps::default()
            },
        );
        self.row_id = Some(row);
    }
}

impl TreeApp for PanelApp {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn build(&mut self, tree: &mut Tree) {
        let col = tree.insert(tree.root(), Stack::column());
        self.anchor = Some(tree.insert_with(
            col,
            Border::new().background(Color::rgb(0.1, 0.1, 0.1)),
            LayoutProps {
                width: Some(60.0),
                height: Some(24.0),
                ..LayoutProps::default()
            },
        ));
    }

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, _action: Action) {}
}

fn frame<A: TreeApp>(h: &mut TreeHost<A>) {
    h.advance_clock(1.0 / 60.0);
    h.update(1.0 / 60.0);
    let mut scene = kanesumi_canvas::Scene::default();
    h.render_into(
        &kanesumi_harness::element::testing::test_engine(),
        Size::new(400.0, 300.0),
        &mut scene,
    );
}

fn host(row: Rc<Cell<f32>>) -> TreeHost<PanelApp> {
    let mut h = TreeHost::new(PanelApp::new(row));
    assert!(h.enable_popups(Rect::new(0.0, 0.0, 400.0, 300.0)));
    frame(&mut h);
    let (app, tree) = h.parts_mut();
    app.open(tree);
    frame(&mut h);
    h
}

/// 改展开行高度（控件自测，故只需量测失效）+ 推进一帧。
fn set_row(h: &mut TreeHost<PanelApp>, px: f32) {
    let (app, tree) = h.parts_mut();
    app.row.set(px);
    let id = app.row_id.expect("弹层内已有展开行");
    tree.invalidate_measure(id);
    frame(h);
}

/// 弹层 Scene 光栅化成 RGBA（与外壳弹层表面的提交路径同一个光栅器）。
fn raster(h: &mut TreeHost<PanelApp>, key: u64, size: Size) -> (u32, u32, Vec<u8>) {
    let engine = kanesumi_harness::element::testing::test_engine();
    // 外壳（`TreeHost`）实现 `App`：`render_popup` 就是把树里该弹层的 Scene 取出来。
    let scene = h.render_popup(&engine, key, size);
    let mut cpu = CpuRenderer::new(size.width, size.height, 1.0);
    let (w, ph) = cpu.physical_size();
    let rgba = cpu.render(&engine, &scene, None).to_vec();
    (w, ph, rgba)
}

fn px(buf: &[u8], w: u32, x: u32, y: u32) -> (u8, u8, u8, u8) {
    let o = ((y * w + x) * 4) as usize;
    (buf[o], buf[o + 1], buf[o + 2], buf[o + 3])
}

fn near(p: (u8, u8, u8, u8), want: (u8, u8, u8)) -> bool {
    let d = |a: u8, b: u8| a.abs_diff(b) <= 2;
    d(p.0, want.0) && d(p.1, want.1) && d(p.2, want.2) && p.3 >= 250
}

/// 整幅都已绘制（alpha 全满）——「缩小时不得露出未绘制区」的像素判据。
fn unpainted(buf: &[u8], w: u32, h: u32) -> Option<(u32, u32)> {
    for y in 0..h {
        for x in 0..w {
            if px(buf, w, x, y).3 < 250 {
                return Some((x, y));
            }
        }
    }
    None
}

/// 展开一行：**同一个 key** 的表面换尺寸（+44），底部多出一行像素。
#[test]
fn expanding_row_grows_the_same_popup_surface() {
    let row = Rc::new(Cell::new(0.0));
    let mut h = host(row);
    let req = h.popups().first().cloned().expect("应申报一个弹层表面");
    assert_eq!(req.rect.size, Size::new(W, HEADER_H), "未展开 = 80 高");
    let key = req.key;

    set_row(&mut h, ROW_H);
    let req2 = h.popups().first().cloned().expect("展开后仍申报同一弹层");
    assert_eq!(req2.key, key, "尺寸变化不得换 key（表面复用）");
    assert_eq!(
        req2.rect.size,
        Size::new(W, HEADER_H + ROW_H),
        "高度随内容长一行"
    );

    let (w, ph, buf) = raster(&mut h, key, req2.rect.size);
    assert_eq!((w, ph), (W as u32, (HEADER_H + ROW_H) as u32));
    assert_eq!(unpainted(&buf, w, ph), None, "新尺寸整幅都已绘制");
    // 未展开时只画到 80：新增的 80..124 就是展开出来的那一行（0.4 灰），不是面板底。
    assert!(
        near(px(&buf, w, 10, 100), ROW),
        "展开后底部多出一行（(10,100) 应为行色，实际 {:?}）",
        px(&buf, w, 10, 100)
    );
    assert!(near(px(&buf, w, 10, 40), BG), "底板仍是面板底色");
}

/// 收起一行：高度回到 80，且整幅仍然画满（不闪未绘制区）。
#[test]
fn collapsing_row_repaints_the_shrunk_surface() {
    let row = Rc::new(Cell::new(0.0));
    let mut h = host(row);
    let key = h.popups().first().expect("弹层").key;
    set_row(&mut h, ROW_H);
    set_row(&mut h, 0.0);

    let req = h.popups().first().cloned().expect("收起后仍是同一弹层");
    assert_eq!(req.key, key, "收起同样复用同一表面");
    assert_eq!(req.rect.size, Size::new(W, HEADER_H), "收起后回到 80 高");
    let (w, ph, buf) = raster(&mut h, key, req.rect.size);
    assert_eq!(
        unpainted(&buf, w, ph),
        None,
        "缩小后的整幅内容先画满（提交前无未绘制区）"
    );
    assert!(near(px(&buf, w, 10, 70), BG), "收起后底边回到面板底");
}
