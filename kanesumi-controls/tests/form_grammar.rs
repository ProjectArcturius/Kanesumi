// form_grammar.rs —— 页面语法（`kanesumi_controls::form`）的行为契约。
//
// 守住三件事：① CSS 外边距折叠的落位（块间距取两者的较大者）；② 开关行投出的动作是
// 库自己的 `SwitchToggled`（不再由应用自绘开关）；③ 链接投出 `FormLinkClicked` 且带应用
// 自定义跳转键。参 `docs/CONTROL_SPEC.md` §44。

use kanesumi_controls::form::{self, Form};
use kanesumi_controls::{FormLinkClicked, SwitchToggled};
use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::Stack;
use kanesumi_element::{LayoutProps, WidgetId};

/// 建好的页面里要用到的节点。
struct Page {
    title: WidgetId,
    first_section: WidgetId,
    second_section: WidgetId,
    switch: WidgetId,
    link: WidgetId,
}

/// 在测试台里建一列「页标题 + 首节 + 开关行 + 次节 + 链接」，返回各节点 id。
fn build_page(h: &mut TestHarness) -> Page {
    let mut ids: Vec<WidgetId> = Vec::new();
    h.frame_with_layout(|tree| {
        let col = tree.insert_with(tree.root(), Stack::column(), LayoutProps::default());
        let mut f = Form::new(col);
        ids.push(f.page_title(tree, "显示"));
        ids.push(f.section(tree, "亮度", true));
        ids.push(f.toggle(tree, "自动亮度", false, Some("随环境光调整")));
        ids.push(f.section(tree, "夜间模式", false));
        ids.push(f.link(tree, "更多显示设置", "display"));
    });
    // `frame_with_layout` 的插入发生在该帧布局之后，故再出一帧让新节点拿到矩形与命中表。
    h.settle();
    Page {
        title: ids[0],
        first_section: ids[1],
        switch: ids[2],
        second_section: ids[3],
        link: ids[4],
    }
}

#[test]
fn first_section_gap_folds_to_page_title_margin() {
    let mut h = TestHarness::new(600.0, 900.0);
    let p = build_page(&mut h);
    let gap = h.rect(p.first_section).origin.y - h.rect(p.title).bottom();
    // 页标题下距 20 与首节上距 4 折叠 → 取较大者 20。
    assert!(
        (gap - form::PAGE_TITLE_MB).abs() < 0.05,
        "首节与页标题的间隔应为折叠后的 {}，实际 {gap}",
        form::PAGE_TITLE_MB
    );
}

#[test]
fn later_section_gap_folds_to_section_top_margin() {
    let mut h = TestHarness::new(600.0, 900.0);
    let p = build_page(&mut h);
    // 上一个块的 CSS 下外边距是开关行的 20，次节上距 36 → 折叠取 36。
    let gap = h.rect(p.second_section).origin.y - h.rect(p.switch).bottom();
    assert!(
        gap >= form::SECTION_MT,
        "次节与上一行的间隔应折叠到 {} 以上，实际 {gap}",
        form::SECTION_MT
    );
}

#[test]
fn toggle_emits_library_switch_action() {
    let mut h = TestHarness::new(600.0, 900.0);
    let p = build_page(&mut h);
    h.click(p.switch);
    let toggled: Vec<SwitchToggled> = h.take::<SwitchToggled>().into_iter().map(|(_, a)| a).collect();
    assert_eq!(toggled, vec![SwitchToggled(true)], "点开关应投出 SwitchToggled(true)");
}

#[test]
fn link_emits_target_key() {
    let mut h = TestHarness::new(600.0, 900.0);
    let p = build_page(&mut h);
    h.click(p.link);
    let clicked: Vec<FormLinkClicked> =
        h.take::<FormLinkClicked>().into_iter().map(|(_, a)| a).collect();
    assert_eq!(clicked.len(), 1, "点链接应投出一次 FormLinkClicked");
    assert_eq!(&*clicked[0].target, "display");
}

#[test]
fn list_item_has_fixed_row_height_and_right_aligned_secondary() {
    let mut h = TestHarness::new(600.0, 400.0);
    let mut ids: Vec<WidgetId> = Vec::new();
    h.frame_with_layout(|tree| {
        let col = tree.insert_with(tree.root(), Stack::column(), LayoutProps::default());
        let mut f = Form::new(col);
        let (title, secondary) = f.list_item(tree, None, "家庭 Wi-Fi", "已连接", form::ITEM_MB);
        ids.push(title);
        ids.push(secondary);
    });
    h.settle();
    let title = h.rect(ids[0]);
    let secondary = h.rect(ids[1]);
    // 行高取尺寸表的列表档（48），标题与次要信息同排、次要信息在右。
    let row_h = title.size.height.max(secondary.size.height);
    assert!(row_h <= form::LIST_ROW_H, "行内容高度不应超过行高：{row_h}");
    assert!(
        secondary.origin.x >= title.right() - 0.01,
        "次要信息应排在标题右侧：title.right={}, secondary.x={}",
        title.right(),
        secondary.origin.x
    );
}

#[test]
fn row_button_is_clickable_and_has_list_height() {
    let mut h = TestHarness::new(600.0, 400.0);
    let mut ids: Vec<WidgetId> = Vec::new();
    h.frame_with_layout(|tree| {
        let col = tree.insert_with(tree.root(), Stack::column(), LayoutProps::default());
        let mut f = Form::new(col);
        let (btn, title, second) = f.row_button(tree, 7, "主目录", "Librarian", form::ITEM_MB);
        ids.extend([btn, title, second]);
    });
    h.settle();
    let row = h.rect(ids[0]);
    assert!(row.size.height >= form::LIST_ROW_H - 0.01, "整行高度应不小于列表行高：{}", row.size.height);
    // 点行内任意位置（这里点整行中心）都应投出 ButtonClicked。
    h.click(ids[0]);
    let hit: Vec<kanesumi_controls::RowButtonClicked> =
        h.take::<kanesumi_controls::RowButtonClicked>().into_iter().map(|(_, a)| a).collect();
    assert_eq!(hit.first().map(|c| c.0), Some(7), "点击应带回行号");
    assert!(h.rect(ids[2]).origin.x >= h.rect(ids[1]).right() - 0.01, "次要信息在右");
}
