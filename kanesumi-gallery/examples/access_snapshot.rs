// access_snapshot —— 无障碍快照导出（E6 地基，参卡面 n1104-e3-controls）。
//
// 无外壳建一页代表性控件，出帧后把 `Tree::access_snapshot()` 的文本序列化打到
// stdout（`AccessNode::to_text`，树序稳定），供人工核对快照与契约检查结果：
//
//   cargo run -p kanesumi-gallery --example access_snapshot
//   KANESUMI_TEST_FONT=<字体路径> cargo run -p kanesumi-gallery --example access_snapshot [输出路径]
//
// 可选位置参数把同一份文本写入文件。顺带跑 `check_access_contract`：
// 契约违例非空时以非零码退出（正常页面应零违例）。

use kanesumi_controls::{MetroButton, MetroCheckBox, MetroSlider, MetroText, MetroTextBox};
use kanesumi_core::{Color, ThemeColor};
use kanesumi_harness::element::check_access_contract;
use kanesumi_harness::element::props::{Align, Insets, LayoutProps};
use kanesumi_harness::element::testing::TestHarness;
use kanesumi_harness::element::widgets::{Border, Stack};

fn main() {
    // 无外壳夹具：树 + 字体 + 出帧（字体找不到即 panic 并说明，不静默跳过）。
    let mut h = TestHarness::new(360.0, 320.0);

    let page = h.tree.insert(
        h.root(),
        Border::new()
            .background(ThemeColor::Background)
            .padding(Insets::all(16.0)),
    );
    let col = h.tree.insert(page, Stack::column().with_spacing(12.0));
    let start = LayoutProps {
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    };
    h.tree.insert_with(col, MetroButton::new("确定"), start);
    h.tree.insert_with(col, MetroCheckBox::new("启用蓝牙"), start);
    h.tree.insert_with(
        col,
        MetroSlider::new().with_header("音量"),
        LayoutProps {
            width: Some(280.0),
            ..start
        },
    );
    h.tree.insert_with(col, MetroTextBox::with_placeholder("占位文本"), start);
    h.tree.insert_with(
        col,
        MetroText::body("说明文本", Color::WHITE),
        start,
    );
    h.frame();

    let snap = h.tree.access_snapshot();
    let text = snap.to_text();
    print!("{text}");
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(&path, &text).unwrap_or_else(|e| panic!("写 {path} 失败: {e}"));
        println!("-- 已写入 {path}");
    }

    let vs = check_access_contract(&h.tree, &snap);
    if vs.is_empty() {
        println!("-- 无障碍契约：通过（可聚焦均有角色与名字、bounds 与命中一致、禁用态一致）");
    } else {
        for v in &vs {
            println!(
                "-- 契约违例 [{}] {} {}: {}",
                v.id.to_u64(),
                h.tree.type_name(v.id),
                v.kind.label(),
                v.detail
            );
        }
        std::process::exit(1);
    }
}
