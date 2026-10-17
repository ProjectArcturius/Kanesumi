// 复审独立反例。参 Ether docs/research/w_scroll_tracker/REVIEW_2026-10-11.md。
use kanesumi_core::scroll_tracker::{Phase, ScrollTracker, TrackerParams};

fn tracker(max: f64) -> ScrollTracker {
    let mut tr = ScrollTracker::new(TrackerParams::default());
    tr.set_bounds([0.0, 0.0], [0.0, max]);
    tr.set_axes(true, false);
    tr
}

#[test]
fn review_settle_preserves_position_and_phase() {
    let mut tr = tracker(1000.0);
    tr.scroll_by(0.0, 0.0, 48.0);
    let before = tr.sample(10.0);
    assert_eq!(before.1, Phase::Idle);
    assert!((before.0[1] - 48.0).abs() < 0.01);
    tr.settle(10.0);
    assert_eq!(tr.sample(10.0), before);
    eprintln!("review settle: {before:?}");
}

#[test]
fn review_natural_stop_precedes_boundary() {
    let mut tr = tracker(30.0);
    tr.flick(0.0, 0.0, 0.0, 0.0, 100.0);
    let expected = 100.0 * TrackerParams::default().tau_s * (1.0 - 30.0 / 100.0);
    for i in 0..=2000 {
        let y = tr.sample(i as f64 / 1000.0).0[1];
        assert!(y <= expected + 1e-9, "自然停点前不应触边：{y}");
    }
    let end = tr.sample(10.0);
    assert_eq!(end.1, Phase::Idle);
    assert!((end.0[1] - expected).abs() < 0.01);
    eprintln!("review natural stop: {end:?}, expected={expected}");
}

#[test]
fn review_press_at_overscroll_peak_is_continuous() {
    let mut tr = tracker(300.0);
    tr.flick(0.0, 0.0, 299.0, 0.0, 8000.0);
    let peak = (0..=10000)
        .map(|i| i as f64 / 10000.0)
        .max_by(|a, b| tr.sample(*a).0[1].total_cmp(&tr.sample(*b).0[1]))
        .unwrap();
    let before = tr.sample(peak).0[1];
    tr.finger_begin(peak);
    let after = tr.sample(peak).0[1];
    assert!(after.is_finite());
    assert!((after - before).abs() < 1e-6);
    eprintln!("review overscroll press: t={peak}, before={before}, after={after}");
}

#[test]
fn review_stationary_release_ignores_old_velocity() {
    let mut tr = tracker(1000.0);
    tr.finger_begin(0.0);
    tr.finger_delta(0.05, 0.0, 30.0);
    tr.finger_end(1.0);
    let after = tr.sample(1.1);
    assert_eq!(after.1, Phase::Idle);
    assert!((after.0[1] - 30.0).abs() < 1e-9);
    eprintln!("review stationary release: {after:?}");
}
