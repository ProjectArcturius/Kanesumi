// 一帧分段计时（量测 / 排布 / 绘制 / 拼接）契约测试。参 tree.rs / docs/ELEMENT_TREE.md §Ⅴ-bis。
//
// 为什么只断言关系而不写绝对值：分段是墙钟量，绝对值随机器、字体、调试/发布档浮动；
// 定死「量测 0.83 ms」这类数字只会变成下一次改动的噪声源。这里只钉两条不变量：
//   1. 开关关闭 → 四段恒 0（**没有量**，不是「很快」）；
//   2. 开关打开 → 四段之和落在 [0, 整帧墙钟] 内（段内时间必然是整帧的一部分）。

use std::time::Instant;

use kanesumi_element::testing::TestHarness;
use kanesumi_element::widgets::{Border, Label};
use kanesumi_element::{Align, FramePhases, LayoutProps};

/// 固定几何叶子：让一帧里有真实的量测 / 排布 / 绘制工作可量。
fn fixed(x: f32, y: f32, w: f32, h: f32) -> LayoutProps {
    LayoutProps {
        width: Some(w),
        height: Some(h),
        margin: kanesumi_element::Insets { left: x, top: y, right: 0.0, bottom: 0.0 },
        h_align: Align::Start,
        v_align: Align::Start,
        ..LayoutProps::default()
    }
}

/// 一棵简单树：两层堆叠 + 一段文本（文本让量测与绘制都有实质工作）。
fn build(h: &mut TestHarness) {
    let root = h.root();
    let card = h.tree.insert_with(root, Border::new(), fixed(10.0, 10.0, 200.0, 80.0));
    h.tree.insert_with(card, Border::new(), fixed(4.0, 4.0, 120.0, 40.0));
    h.tree.insert_with(root, Label::new("Kanesumi"), fixed(20.0, 120.0, 180.0, 24.0));
}

/// (a) 开关关闭：分段字段保持 0 —— 未开计时不得留下任何毫秒数（否则外壳把「没量」当「量到了」）。
#[test]
fn phases_stay_zero_while_timing_is_off() {
    let mut h = TestHarness::new(320.0, 240.0);
    build(&mut h);
    h.frame();
    h.frame();
    let p = h.last.phases;
    assert_eq!(
        p,
        FramePhases::default(),
        "分段计时缺省关闭，四段必须保持 0：{p:?}"
    );
    assert!(!p.is_sampled(), "未开计时不得报告「采过样」");
    // 树侧的回读口与帧产物同值（未开计时也必须是 0）。
    assert_eq!(h.tree.frame_phases(), FramePhases::default());

    // 开过再关：残账必须清掉，不得把上一帧的毫秒数留给下一次读取。
    h.tree.set_phase_timing(true);
    h.frame();
    h.tree.set_phase_timing(false);
    assert_eq!(
        h.tree.frame_phases(),
        FramePhases::default(),
        "关掉计时后不得残留上一帧的分段毫秒"
    );
}

/// (b) 开关打开：四段之和 ≥ 0 且 ≤ 整帧墙钟，且至少有一段真的量到了。
#[test]
fn phases_sum_fits_inside_one_frame() {
    let mut h = TestHarness::new(320.0, 240.0);
    build(&mut h);
    h.tree.set_phase_timing(true);
    // 首帧付实现 / 排版缓存的钱，量到的分段才不是「全 0 的稳态帧」。
    let t = Instant::now();
    h.frame();
    let frame_ms = t.elapsed().as_secs_f32() * 1000.0;

    let p = h.last.phases;
    assert!(p.is_sampled(), "打开开关后必须量到东西：{p:?}");
    for (name, ms) in [
        ("measure", p.measure_ms),
        ("arrange", p.arrange_ms),
        ("paint", p.paint_ms),
        ("compose", p.compose_ms),
        ("realize", p.realize_ms),
        ("animate", p.animate_ms),
    ] {
        assert!(ms >= 0.0 && ms.is_finite(), "{name} 必须是有限非负毫秒：{ms}");
    }
    let sum = p.scene_sum_ms();
    assert!(sum >= 0.0, "四段之和不得为负：{sum}");
    assert!(
        sum <= frame_ms,
        "四段之和（{sum:.3} ms）不得超过整帧墙钟（{frame_ms:.3} ms）"
    );

    // 稳态帧（无失效）也照量：分段为整段的一部分，不因「没什么可画」而漏取。
    let t = Instant::now();
    h.frame();
    let frame_ms = t.elapsed().as_secs_f32() * 1000.0;
    let sum = h.last.phases.scene_sum_ms();
    assert!(sum >= 0.0 && sum <= frame_ms, "稳态帧四段之和 {sum:.3} ms 越界");
}
