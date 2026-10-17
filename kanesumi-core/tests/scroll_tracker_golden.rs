// scroll_tracker_golden.rs —— 滚动跟踪器对拍真值（Win11 探针 + Lumia 15 段）。
//
// 设计 `tools/dispatch/tasks/fp4-compositor-scroll.md` §Ⅱ；阈值见任务书 C 节与裁定 §134。
// 数据在 `tests/data/`：Win11 `samples_*.csv` 原样；Lumia 取 `tracks.csv` 的 inertia 行 `t_ms,y` 切片。
// 单测（`src/scroll_tracker.rs`）覆盖纯逻辑；本文件对照真值。

use std::path::PathBuf;

use kanesumi_core::scroll_tracker::{Phase, ScrollTracker, TrackerParams};

/// 取 `tests/data/` 下的文件绝对路径。
fn data_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name)
}

/// 读 `t,value` 两列 CSV，跳过 `#` 注释行与表头。
fn load_xy(name: &str) -> Vec<(f64, f64)> {
    let txt = std::fs::read_to_string(data_path(name)).expect("读真值 CSV");
    txt.lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("t"))
        .filter_map(|l| {
            let mut it = l.split(',');
            let a = it.next()?.trim().parse::<f64>().ok()?;
            let b = it.next()?.trim().parse::<f64>().ok()?;
            Some((a, b))
        })
        .collect()
}

/// 从 `samples_default_v300.csv` 之类取初速度数值。
fn velocity_from_name(name: &str) -> f64 {
    let n = name.trim_end_matches(".csv");
    if let Some(v) = n.split("_v").nth(1)
        && let Ok(x) = v.parse::<f64>()
    {
        return x;
    }
    2000.0
}

/// 注入起始状态：直接以给定位置与速度起惯性（精确给出 `(p0, v0)`）。
fn inject(tr: &mut ScrollTracker, p0: [f64; 2], v0: [f64; 2], t_release: f64, min: [f64; 2], max: [f64; 2]) {
    tr.set_bounds(min, max);
    tr.flick(t_release, p0[0], p0[1], v0[0], v0[1]);
}

/// 一维标量拟合：粗扫 + 黄金分割精化（目标函数近似单峰）。
fn fit_scalar(f: impl Fn(f64) -> f64, center: f64, span: f64) -> f64 {
    let n = 400;
    let (mut bx, mut bv) = (center, f64::INFINITY);
    for i in 0..=n {
        let x = center - span + 2.0 * span * (i as f64) / (n as f64);
        let v = f(x);
        if v < bv {
            bv = v;
            bx = x;
        }
    }
    let step = 2.0 * span / (n as f64);
    let (mut a, mut b) = (bx - 2.0 * step, bx + 2.0 * step);
    let gr = 0.618_033_988_749_894_9;
    let (mut c, mut d) = (b - gr * (b - a), a + gr * (b - a));
    let (mut fc, mut fd) = (f(c), f(d));
    for _ in 0..120 {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - gr * (b - a);
            fc = f(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + gr * (b - a);
            fd = f(d);
        }
    }
    0.5 * (a + b)
}

/// 以 `(p0, v0)` 注入、在 `t0` 松手后，对采样点求位置 RMSE（纵向轴）。
fn inertia_rmse(
    t: &[f64],
    x: &[f64],
    p0: f64,
    v0: f64,
    t0: f64,
    params: &TrackerParams,
    bounds: ([f64; 2], [f64; 2]),
) -> f64 {
    let mut tr = ScrollTracker::new(*params);
    inject(&mut tr, [0.0, p0], [0.0, v0], t0, bounds.0, bounds.1);
    let mut s = 0.0;
    for i in 0..t.len() {
        let d = tr.sample(t[i]).0[1] - x[i];
        s += d * d;
    }
    (s / t.len() as f64).sqrt()
}

const WIDE: ([f64; 2], [f64; 2]) = ([-1.0e9, -1.0e9], [1.0e9, 1.0e9]);

/// Lumia 设置主页列表的可滚范围 `[-260, 0]`（`lumia_etw/README.md` §五）。
const LUMIA_MIN: f64 = -260.0;
const LUMIA_MAX: f64 = 0.0;

/// C-1 Win11 自由惯性：5 档初速 + 3 档衰减率，位置 RMSE（8000 档放宽到 3 px），停止速度 30。
#[test]
fn c1_win11_free_inertia() {
    let (lo, hi) = WIDE;
    for name in [
        "samples_default_v300.csv",
        "samples_default_v1000.csv",
        "samples_default_v2000.csv",
        "samples_default_v4000.csv",
        "samples_default_v8000.csv",
    ] {
        let d = load_xy(name);
        let t: Vec<f64> = d.iter().map(|p| p.0).collect();
        let x: Vec<f64> = d.iter().map(|p| p.1).collect();
        let v0 = velocity_from_name(name);
        let params = TrackerParams::default();
        let tau = params.tau_s;
        let t0 = fit_scalar(|t0| inertia_rmse(&t, &x, 0.0, v0, t0, &params, (lo, hi)), t[0], 0.3);
        let rmse = inertia_rmse(&t, &x, 0.0, v0, t0, &params, (lo, hi));
        let limit = if v0 >= 8000.0 { 3.0 } else { 0.5 };
        eprintln!("C-1 {name}: t0fit={t0:.5} RMSE={rmse:.4} limit={limit}");
        assert!(rmse <= limit, "{name}: RMSE={rmse:.3} > {limit}");

        // 停止速度：终值 = A − 30·τ（证明停在 30 px/s），且与真值终值同量级。
        let mut tr = ScrollTracker::new(params);
        inject(&mut tr, [0.0, 0.0], [0.0, v0], t0, lo, hi);
        let end = tr.sample(t0 + 10.0).0[1];
        let analytic = v0 * tau * (1.0 - 30.0 / v0);
        assert!((end - analytic).abs() < 1.0e-6, "{name}: 终值 {end} vs {analytic}");
        assert!((end - x[x.len() - 1]).abs() < 1.0, "{name}: 终值偏离真值");
    }

    // 3 档衰减率：τ 取律式 τ = −1/ln(1−d)。三档门槛按裁定 §136 = 数据底 + 0.15 px：
    // 探针 `fit_summary.csv` 的最佳三参数拟合（τ / A / t0 全放）为 0.862 / 0.772 / 0.779 px，
    // 故 decay0.90 ≤ 1.012、decay0.95 ≤ 0.922、decay0.99 ≤ 0.929（原 0.5 px 是调度者写错的采样噪声底）。
    for (name, rate, limit) in [
        ("samples_decay0.90.csv", 0.90f64, 1.012f64),
        ("samples_decay0.95.csv", 0.95f64, 0.922f64),
        ("samples_decay0.99.csv", 0.99f64, 0.929f64),
    ] {
        let d = load_xy(name);
        let t: Vec<f64> = d.iter().map(|p| p.0).collect();
        let x: Vec<f64> = d.iter().map(|p| p.1).collect();
        let params = TrackerParams {
            tau_s: -1.0 / (1.0 - rate).ln(),
            ..Default::default()
        };
        let t0 = fit_scalar(|t0| inertia_rmse(&t, &x, 0.0, 2000.0, t0, &params, (lo, hi)), t[0], 0.3);
        let rmse = inertia_rmse(&t, &x, 0.0, 2000.0, t0, &params, (lo, hi));
        eprintln!("C-1 {name} (decay law): RMSE={rmse:.4} limit={limit} (裁定 §136)");
        assert!(rmse <= limit, "{name}: RMSE={rmse:.3} > {limit}");
    }
}

/// C-2 Win11 撞边界：过冲后单调回贴 300，无振荡、无下穿。
#[test]
fn c2_win11_boundary() {
    let bounds = ([0.0, 0.0], [1.0e5, 300.0]);
    let d = load_xy("samples_boundary_max300_v2000.csv");
    let t: Vec<f64> = d.iter().map(|p| p.0).collect();
    let x: Vec<f64> = d.iter().map(|p| p.1).collect();
    let params = TrackerParams::default();
    let t0 = fit_scalar(|t0| inertia_rmse(&t, &x, 0.0, 2000.0, t0, &params, bounds), t[0], 0.3);
    let mut tr = ScrollTracker::new(params);
    inject(&mut tr, [0.0, 0.0], [0.0, 2000.0], t0, bounds.0, bounds.1);

    let mut ps = Vec::new();
    let mut tc = t0;
    while tc < t0 + 1.5 {
        ps.push(tr.sample(tc).0[1]);
        tc += 1.0 / 1000.0;
    }
    let (ipk, peak) = ps
        .iter()
        .enumerate()
        .fold((0usize, f64::NEG_INFINITY), |(bi, bv), (i, &v)| if v > bv { (i, v) } else { (bi, bv) });
    let overshoot = peak - 300.0;
    eprintln!("C-2 过冲={overshoot:.2} px（真值 35.3），t0fit={t0:.5}");
    assert!((10.0..=60.0).contains(&overshoot), "过冲 {overshoot:.2} px（真值 35.3）");
    for i in ipk + 1..ps.len() {
        assert!(ps[i] <= ps[i - 1] + 1.0e-9, "回程非单调 i={i}");
    }
    assert!(ps[ipk..].iter().all(|&v| v >= 300.0 - 0.01), "回程下穿边界");
    assert!((ps[ps.len() - 1] - 300.0).abs() < 0.01, "终点 {}", ps[ps.len() - 1]);
}

/// C-3 Win11 打断：`set_position` 立即 Idle、位置 = 目标。
#[test]
fn c3_win11_interrupt() {
    let d = load_xy("samples_interrupt_t0.08_to100_v2000.csv");
    let t0 = d[0].0 - 0.008;
    let (t_jump, target) = *d.last().unwrap();
    let mut tr = ScrollTracker::new(TrackerParams::default());
    tr.set_bounds([0.0, 0.0], [1.0e5, 1.0e5]);
    inject(&mut tr, [0.0, 0.0], [0.0, 2000.0], t0, [0.0, 0.0], [1.0e5, 1.0e5]);
    assert_eq!(tr.sample(t_jump - 0.001).1, Phase::Inertia);
    tr.set_position(t_jump, 0.0, target);
    let (p, ph) = tr.sample(t_jump);
    assert_eq!(ph, Phase::Idle);
    assert!((p[1] - target).abs() < 1.0e-9, "位置 {}", p[1]);
    assert!((tr.sample(t_jump + 1.0).0[1] - target).abs() < 1.0e-9);
}

/// Lumia 分段（裁定 §134）：60 ms 间隔 + 「单步 |Δy| > 40 且反向」另切；<5 步子段剔除。
/// 返回 `(保留子段, 剔除子段步数)`。
fn lumia_segments() -> (Vec<Vec<(f64, f64)>>, Vec<usize>) {
    let d = load_xy("lumia_tracks.csv");
    let mut base: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut cur = vec![d[0]];
    for p in &d[1..] {
        if p.0 - cur[cur.len() - 1].0 > 60.0 {
            base.push(cur);
            cur = vec![*p];
        } else {
            cur.push(*p);
        }
    }
    base.push(cur);

    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for seg in base {
        let mut cuts = Vec::new();
        let mut prev = 0.0;
        for i in 0..seg.len() - 1 {
            let dy = seg[i + 1].1 - seg[i].1;
            if dy.abs() > 40.0 && prev != 0.0 && (dy > 0.0) != (prev > 0.0) {
                cuts.push(i + 1);
            }
            if dy.abs() > 1.0e-9 {
                prev = dy;
            }
        }
        let mut parts = Vec::new();
        let mut s = 0;
        for c in cuts {
            parts.push(seg[s..c].to_vec());
            s = c;
        }
        parts.push(seg[s..].to_vec());
        for p in parts {
            if p.len() < 5 {
                dropped.push(p.len());
            } else {
                kept.push(p);
            }
        }
    }
    (kept, dropped)
}

/// C-4 Lumia 段（新口径）：逐段拟合初速度（1 dof）、其余参数共享。
///
/// 门槛：**第一步位置严格落在 `[-260, 0]` 内**的段（代码判据，不按段号硬编码）不进
/// 「中位 ≤ 3 / 每段 ≤ 6」门，单列打印其 RMSE 与界内步数；其终点 |误差| < 0.01 px 仍成立
/// （裁定 §137）。其余段入门：逐段 RMSE 中位 ≤ 3 px 且每段 ≤ 6 px。
#[test]
fn c4_lumia_segments() {
    let params = TrackerParams::default();
    let (segs, dropped) = lumia_segments();
    let lo = [-1.0e9, LUMIA_MIN];
    let hi = [1.0e9, LUMIA_MAX];
    let mut rmses = Vec::new();
    let mut exempt = Vec::new();
    for seg in &segs {
        let p0 = seg[0].1;
        let t: Vec<f64> = seg.iter().map(|p| (p.0 - seg[0].0) / 1000.0).collect();
        let x: Vec<f64> = seg.iter().map(|p| p.1).collect();
        let v0first = (seg[1].1 - seg[0].1) / ((seg[1].0 - seg[0].0) / 1000.0);
        let span = (v0first.abs() * 4.0).max(200.0);
        let vfit = fit_scalar(|v| inertia_rmse(&t, &x, p0, v, 0.0, &params, (lo, hi)), v0first, span);
        let rmse = inertia_rmse(&t, &x, p0, vfit, 0.0, &params, (lo, hi));
        // 到边界的段：终点必须精确落在边界（对豁免段同样生效）。
        let last = x[x.len() - 1];
        if last.abs() < 0.5 || (last - LUMIA_MIN).abs() < 0.5 {
            let mut tr = ScrollTracker::new(params);
            inject(&mut tr, [0.0, p0], [0.0, vfit], 0.0, lo, hi);
            let end = tr.sample(10.0).0[1];
            assert!((end - last).abs() < 0.01, "终点 {end:.4} vs {last:.4}（p0={p0:.2}）");
        }
        // §137 豁免判据：第一步位置严格在可滚范围内部 → 不入门槛，单列报告。
        if p0 > LUMIA_MIN && p0 < LUMIA_MAX {
            let in_bounds = x.iter().filter(|&&y| (LUMIA_MIN..=LUMIA_MAX).contains(&y)).count();
            exempt.push((p0, v0first, vfit, rmse, in_bounds));
            continue;
        }
        rmses.push((p0, v0first, vfit, rmse));
    }
    eprintln!(
        "C-4 子段 {}（剔除 <5 步 {:?}），免入门槛（首步界内）{} 段",
        segs.len(),
        dropped,
        exempt.len()
    );
    for (p0, vf, vm, r, n) in &exempt {
        eprintln!(
            "  [豁免/首步界内 p0={p0:.2}] v0first={vf:8.0} v0fit={vm:8.0} ratio={:.2} RMSE={r:.3} 界内步数={n}",
            vm / vf
        );
    }
    for (p0, vf, vm, r) in &rmses {
        eprintln!("  p0={p0:8.2} v0first={vf:8.0} v0fit={vm:8.0} ratio={:.2} RMSE={r:.3}", vm / vf);
    }
    let mut sorted: Vec<f64> = rmses.iter().map(|r| r.3).collect();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = sorted[sorted.len() / 2];
    assert!(median <= 3.0, "RMSE 中位 {median:.3} > 3");
    for (p0, _, _, r) in &rmses {
        assert!(*r <= 6.0, "段 p0={p0:.2} RMSE={r:.3} > 6");
    }
    assert_eq!(exempt.len(), 1, "应恰有一段首步界内（P4 数据前提）");
}

/// C-5 采样无关性（任务书 2026-10-11 补足）：同一手势（拖动、松手、越界、回程）以
/// 60 / 120 / 165 Hz 与固定种子随机抖动（±4 ms）四种采样各跑，1 ms 网格公共时刻位置差 < 1e-9。
#[test]
fn c5_sampling_independence() {
    // 按时间交错送手势、采样与 settle，避免在最终状态上倒查已被丢弃的跟手历史。
    let build = || {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([-1.0e5, -1.0e5], [1.0e5, 200.0]);
        tr.set_position(0.0, 0.0, 0.0);
        tr
    };
    // 四种采样时刻序列。
    let hz: Vec<f64> = (0..30).map(|i| i as f64 / 60.0).collect();
    let hz120: Vec<f64> = (0..60).map(|i| i as f64 / 120.0).collect();
    let hz165: Vec<f64> = (0..80).map(|i| i as f64 / 165.0).collect();
    // 固定种子抖动：标称 60 Hz 步长 ±4 ms。
    let mut s: u64 = 0x0c0f_fee1_1234_5678;
    let mut jitter = Vec::new();
    let mut t = 0.0;
    while t < 0.5 {
        jitter.push(t);
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let frac = ((s >> 11) as f64) / ((1u64 << 53) as f64);
        t += 1.0 / 60.0 + (frac * 2.0 - 1.0) * 0.004;
    }
    let schedules = [hz, hz120, hz165, jitter];

    // 1 ms 网格公共时刻。
    let grid: Vec<f64> = (0..=1000).map(|i| i as f64 / 1000.0).collect();
    let mut baseline: Option<Vec<([f64; 2], Phase)>> = None;
    for sched in &schedules {
        let mut tr = build();
        // 同刻先送输入，再取公共采样，最后模拟可选帧末 settle。
        let mut events: Vec<_> = sched.iter().map(|&t| (t, 2u8, 0usize)).collect();
        events.extend(grid.iter().enumerate().map(|(i, &t)| (t, 1, i)));
        events.extend([(0.0, 0, 0), (0.02, 0, 1), (0.04, 0, 2), (0.05, 0, 3)]);
        events.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let mut vals = Vec::new();
        for (t, kind, i) in events {
            match (kind, i) {
                (0, 0) => tr.finger_begin(t),
                (0, 1 | 2) => tr.finger_delta(t, 0.0, 120.0),
                (0, _) => tr.finger_end(t),
                (1, _) => vals.push(tr.sample(t)),
                _ => {
                    let before = tr.sample(t);
                    tr.settle(t);
                    assert_eq!(tr.sample(t), before);
                }
            }
        }
        assert_eq!(vals[10].1, Phase::Interacting);
        assert_eq!(vals[30].0[1], 120.0);
        assert!(vals[45].0[1] > 200.0);
        assert_eq!(vals[60].1, Phase::Inertia);
        assert_eq!(vals[1000].1, Phase::Idle);
        match &baseline {
            None => baseline = Some(vals),
            Some(b) => {
                for i in 0..grid.len() {
                    let diff = (b[i].0[1] - vals[i].0[1]).abs();
                    assert!(diff < 1.0e-9, "t={} 位置差 {diff}", grid[i]);
                    assert_eq!(b[i].1, vals[i].1);
                }
            }
        }
    }
}

/// C-6 松手无空档：`finger_end` 后第一个 16.67 ms 位移 ≈ v·Δ（相对误差 < 5%）。
#[test]
fn c6_release_has_no_gap() {
    let mut tr = ScrollTracker::new(TrackerParams::default());
    inject(&mut tr, [0.0, 0.0], [0.0, 400.0], 0.0, WIDE.0, WIDE.1);
    let p0 = tr.sample(0.0).0[1];
    let dt = 1.0 / 60.0;
    let p1 = tr.sample(dt).0[1];
    let rel = ((p1 - p0) - 400.0 * dt).abs() / (400.0 * dt);
    assert!(rel < 0.05, "位移 {} vs v·Δ {}", p1 - p0, 400.0 * dt);
}

/// C-7 `scroll_by`：一次 48 恰停 +48；三次 +144；近边界夹住不过冲；反向归零。
#[test]
fn c7_scroll_by() {
    let mk = || {
        let mut tr = ScrollTracker::new(TrackerParams::default());
        tr.set_bounds([0.0, 0.0], [1.0e5, 1000.0]);
        tr
    };
    let mut tr = mk();
    tr.scroll_by(0.0, 0.0, 48.0);
    assert!((tr.sample(5.0).0[1] - 48.0).abs() < 0.01);
    let mut tr = mk();
    for _ in 0..3 {
        tr.scroll_by(0.0, 0.0, 48.0);
    }
    assert!((tr.sample(5.0).0[1] - 144.0).abs() < 0.01);
    let mut tr = ScrollTracker::new(TrackerParams::default());
    tr.set_bounds([0.0, 0.0], [1.0e5, 100.0]);
    for _ in 0..3 {
        tr.scroll_by(0.0, 0.0, 48.0);
    }
    assert!((tr.sample(5.0).0[1] - 100.0).abs() < 0.01, "近边界未夹住");
    let mut tr = mk();
    tr.scroll_by(0.0, 0.0, 48.0);
    tr.scroll_by(2.0, 0.0, -48.0);
    assert!(tr.sample(5.0).0[1].abs() < 0.01, "反向未归零");
}

/// C-8 越界跟手：位移 = `M·(1 − e^{−x/M})`；松手后回程单调到边界。
#[test]
fn c8_overscroll_follow() {
    let m = TrackerParams::default().overscroll_max;
    let mut tr = ScrollTracker::new(TrackerParams::default());
    tr.set_bounds([0.0, 0.0], [1.0e5, 300.0]);
    tr.set_position(0.0, 0.0, 0.0);
    tr.finger_begin(0.0);
    tr.finger_delta(1.0, 0.0, 400.0);
    let want = 300.0 + m * (1.0 - (-100.0 / m).exp());
    assert!((tr.sample(1.0).0[1] - want).abs() < 1.0e-9, "跟手 {}", tr.sample(1.0).0[1]);
    tr.finger_delta(1.2, 0.0, 0.0);
    tr.finger_delta(1.3, 0.0, 0.0);
    tr.finger_end(1.3);
    let mut prev = tr.sample(1.3).0[1];
    let mut t = 1.3;
    while t < 2.2 {
        t += 1.0 / 120.0;
        let x = tr.sample(t).0[1];
        assert!(x <= prev + 1.0e-9, "回程非单调 t={t}");
        prev = x;
    }
    assert!((prev - 300.0).abs() < 0.01, "回程未到边界 {prev}");
}


