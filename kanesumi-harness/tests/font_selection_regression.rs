//! A3 主字面选择与自定义字体旁路回归（集成测试，2026-10-31 复审派单）。
//!
//! 上一轮静态复审（first-trial-v2/report.md 问题 2/3）指出：既有 r2 测试
//! `font_index_env_gate_and_fallbacks` 的主字面断言只比 `path`，不比
//! `collection_tag` / `collection_index`；本文件补足「真实面身份」层：
//!
//! - 身份判据 = `TextEngine::identity()`（`refresh_identity`：全部面字节 +
//!   `collection_index` 逐面 FNV-1a，`text.rs`）。同构造两引擎身份相等 ⇔ 同字节
//!   同字面下标——是**真实面身份**，不依赖文件名标签；
//! - 场景 A：有效索引首条目 = 真 Bold 字体时，主字面仍须是链主字面
//!   （检出「Bold 被误选为 Regular 请求的主字面」——A3.1-r2 真机像素回归形态）；
//! - 场景 B：CJK TTC 的 SC / TC 标签必须解析到**不同**字面、未知标签回落面 0
//!   （检出「TTC 中文 collection 选错」——本机夹具 TTC 面 0 为 JP，正是裁定
//!   N-40 的危险形态；标签机制一死，SC/TC 同落面 0，`sc == tc` 即失败）；
//! - 场景 C：`KANESUMI_TEST_FONT` 自定义字体必须在有效索引在场时胜出
//!   （检出「用户自定义字体被系统索引盖掉」——G5 旁路优先契约）。
//!
//! 契约未要求的「不存在路径」行为（静默回退、无自证日志）**不在本文件固化**，
//! 只在派单报告记录，等调度者定口径。
//!
//! 放 `tests/`（集成测试）的原因：独立测试二进制 = 独立进程，与
//! `platform.rs` 单测的 env 持久化操作（`KANESUMI_FONT_INDEX` 等全局量）
//! 无进程内并发竞争；本文件唯一 `#[test]` 顺序执行三场景，结束恢复环境。
//! 既有测试与其断言一律未改动。

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kanesumi_canvas::font_index::{write_index, DirFpSpec, IndexEntrySpec};
use kanesumi_canvas::text::{FontSource, TextEngine};

/// 堆内读取器：与 `TextEngine::load_stack` 同源（mmap 与堆读字节相同，身份一致）。
fn read_heap(path: &Path) -> io::Result<kanesumi_canvas::text::FontBytes> {
    Ok(Arc::new(std::fs::read(path)?))
}

/// 单来源引擎身份（附加字面为空，只看主字面的面集合）。
fn engine_identity(src: &FontSource) -> Option<u64> {
    TextEngine::load_stack_with(src, &[], &read_heap)
        .ok()
        .map(|engine| engine.identity())
}

/// 机器上实际存在的真 Bold 字体（场景 A 诱饵 / 场景 C 自定义字体备选）。
/// 只作**候选**用：断言全部走引擎身份，不以文件名认定任何身份。
const BOLD_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
    "/usr/local/share/fonts/s/SourceHanSansSC_Bold.otf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Bold.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc",
];

/// 与主字面不同源的真实字体（场景 C 自定义字体备选，拷入临时目录使用）。
const NON_PRIMARY_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
];

/// 机器上的 CJK TTC 夹具候选（场景 B；须为多面集合）。
const TTC_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/opentype/noto/NotoSerifCJK-Bold.ttc",
];

/// TTC 集合面数（仅读 12 B 头：magic `ttcf` + 版本 + count，小端）。
/// 非集合或读不出 → None。count < 2 的「集合」无法构造选面回归。
fn ttc_face_count(path: &Path) -> Option<u32> {
    let head = std::fs::read(path).ok()?;
    if head.len() < 12 || head[0..4] != *b"ttcf" {
        return None;
    }
    Some(u32::from_le_bytes([head[8], head[9], head[10], head[11]]))
}

/// 找第一个满足「存在 + 可加载 + 与主字面身份不同」的候选（真实身份判据）。
fn find_decoy(
    candidates: &[&str],
    primary: &FontSource,
    primary_identity: u64,
) -> Option<(PathBuf, u64)> {
    let primary_canon = primary.path.canonicalize().unwrap_or_else(|_| primary.path.clone());
    candidates.iter().map(PathBuf::from).find_map(|path| {
        if !path.exists() {
            return None;
        }
        if path.canonicalize().unwrap_or_else(|_| path.clone()) == primary_canon {
            return None;
        }
        let identity = engine_identity(&FontSource { path: path.clone(), collection_tag: None })?;
        (identity != primary_identity).then_some((path, identity))
    })
}

/// 写一份「首条目 = 指定真字体」的有效索引，并断言本仓读侧确实接受它
///（「有效索引」前提自证——夹具无效则全部场景失去意义）。
fn write_decoy_index(idx_path: &Path, decoy: &Path, dir: &Path) {
    let decoy_str = decoy.to_string_lossy().into_owned();
    let dir_str = dir.to_string_lossy().into_owned();
    let entry = IndexEntrySpec {
        path: decoy_str.as_str(),
        family: "RegressionDecoy",
        collection_index: 0,
        weight: 700,
        slant: 0,
        flags: 0,
        ttc_faces: 0,
        coverage: &[(0x41, 0x5A)],
        wght_min: 700.0,
        wght_max: 700.0,
        mtime_ns: 0,
    };
    let decoy_bytes = std::fs::read(decoy).expect("诱饵字体字节必须可读");
    let bytes = [decoy_bytes.as_slice()];
    let dirfps = [DirFpSpec { path: dir_str.as_str(), mtime_ns: 0, ino: 0 }];
    let idx = write_index(&[entry], &bytes, &dirfps, 0, 0, 0);
    std::fs::create_dir_all(idx_path.parent().expect("索引路径必有父目录")).unwrap();
    std::fs::write(idx_path, &idx).unwrap();
    let parsed = kanesumi_canvas::font_index::OwnedIndex::open(idx_path)
        .expect("夹具索引必须通过本仓读侧全部校验");
    assert!(parsed.entry(0).is_some(), "夹具索引必须含首条目");
}

/// 三场景顺序执行（单函数：本文件唯一测试，进程内无并发 env 访问）；
/// 结束恢复环境三元组。缺夹具的机器跳过对应场景并打印原因（记录式跳过，
/// 断言本身不因机器差异放宽）。
#[test]
fn font_selection_regression() {
    let saved = (
        std::env::var("KANESUMI_FONT_INDEX").ok(),
        std::env::var("XDG_RUNTIME_DIR").ok(),
        std::env::var("KANESUMI_TEST_FONT").ok(),
    );
    // SAFETY：本文件唯一 #[test]，单线程顺序执行，进程内无并发 env 访问。
    unsafe {
        std::env::remove_var("KANESUMI_TEST_FONT");
        std::env::remove_var("KANESUMI_FONT_INDEX");
    }

    let chain = kanesumi_canvas::text::system_font_chain();
    let Some(primary) = chain.first().cloned() else {
        println!("skip: 本机无回退链字体（canvas 正典链为空），三场景全部跳过");
        restore(saved);
        return;
    };
    let Some(primary_identity) = engine_identity(&primary) else {
        panic!("链主字面必须可加载：{}", primary.path.display());
    };
    // 跨实现一致性（r2 契约「不另造第二套链」）：harness find_font_source
    // （旁路清空态）必须与 canvas 正典链主字面同路径同 tag——上一轮复审指出的
    // 「主字面 collection_tag 未被断言」缺口在此补足。
    let picked = kanesumi_harness::platform::find_font_source().expect("主字面必须存在");
    assert_eq!(picked.path, primary.path, "harness 主字面与 canvas 正典链漂移");
    assert_eq!(
        picked.collection_tag, primary.collection_tag,
        "harness 主字面 collection_tag 与 canvas 正典链漂移"
    );
    assert_eq!(
        engine_identity(&picked),
        Some(primary_identity),
        "harness 主字面真实面身份与 canvas 正典链漂移"
    );

    let tmp = std::env::temp_dir().join(format!(
        "kmi-fontsel-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    std::fs::create_dir_all(tmp.join("kanesumi")).unwrap();
    std::fs::create_dir_all(tmp.join("fonts")).unwrap();
    let idx_path = tmp.join("kanesumi/fonts-v1.idx");
    // SAFETY：同上，单线程顺序执行。
    unsafe { std::env::set_var("XDG_RUNTIME_DIR", &tmp) };

    // ── 场景 A：有效索引首条目 = 真 Bold，主字面不得被带歪 ──────────────
    if let Some((decoy, decoy_identity)) = find_decoy(BOLD_CANDIDATES, &primary, primary_identity) {
        write_decoy_index(&idx_path, &decoy, &tmp.join("fonts"));
        // SAFETY：同上。
        unsafe { std::env::set_var("KANESUMI_FONT_INDEX", "1") };
        let picked = kanesumi_harness::platform::find_font_source().expect("主字面必须存在");
        assert_eq!(picked.path, primary.path, "索引在场：主字面路径被索引首条目带歪");
        assert_eq!(
            picked.collection_tag, primary.collection_tag,
            "索引在场：主字面 collection_tag 被改变"
        );
        assert_eq!(
            engine_identity(&picked),
            Some(primary_identity),
            "索引在场：主字面真实面身份被改变（Bold 被误选为 Regular 主字面的形态未被拦住；诱饵 = {}）",
            decoy.display()
        );
        // SAFETY：同上。
        unsafe { std::env::remove_var("KANESUMI_FONT_INDEX") };
        assert_eq!(
            kanesumi_harness::platform::find_font_source().map(|s| s.path),
            Some(primary.path.clone()),
            "env 关复检：主字面与 env 开不一致"
        );
        println!("场景 A：诱饵 Bold = {}（身份 {decoy_identity:#x}）", decoy.display());
    } else {
        println!("skip 场景 A：本机无可与主字面区分的真 Bold 字体");
    }

    // ── 场景 B：TTC 中文 collection 选面机制 ────────────────────────────
    let ttc = TTC_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| ttc_face_count(p).is_some_and(|count| count >= 2));
    if let Some(ttc) = ttc {
        let face0 = TextEngine::load(&ttc).expect("TTC 夹具必须可加载").identity();
        let tag_of = |tag: Option<&'static str>| {
            engine_identity(&FontSource { path: ttc.clone(), collection_tag: tag })
                .expect("TTC 夹具按标签必须可加载")
        };
        let sc = tag_of(Some("SC"));
        let tc = tag_of(Some("TC"));
        let bogus = tag_of(Some("NOSC"));
        assert_ne!(sc, tc, "SC 与 TC 解析到同一字面：collection 选择机制失效（选错中文 collection）");
        assert_ne!(sc, face0, "SC 与面 0 同字面且 TC 亦异：若面 0 非 SC 即为 JP 误选（N-40 形态）");
        assert_eq!(bogus, face0, "未知标签必须回落面 0（unwrap_or(0) 语义）");
        println!("场景 B：TTC = {}（面数 {}）", ttc.display(), ttc_face_count(&ttc).unwrap());
    } else {
        println!("skip 场景 B：本机无多面 CJK TTC");
    }
    // 结构性钉死：canvas 正典链上所有 .ttc 条目必须带 SC 标签——构建器按此链
    // 枚举条目，标签一丢，消费端静默落到面 0（Debian 实测为 JP）。
    for src in &chain {
        if src.path.extension().is_some_and(|ext| ext == "ttc") {
            assert_eq!(src.collection_tag, Some("SC"), "链上 TTC 条目缺 SC 标签：{}", src.path.display());
        }
    }

    // ── 场景 C：自定义字体必须胜过在场索引 ──────────────────────────────
    if let Some((source, source_identity)) =
        find_decoy(NON_PRIMARY_CANDIDATES, &primary, primary_identity)
    {
        let custom = tmp.join("fonts/custom-user-font.ttf");
        std::fs::copy(&source, &custom).expect("自定义字体夹具必须可复制");
        let custom_identity = engine_identity(&FontSource {
            path: custom.clone(),
            collection_tag: None,
        })
        .expect("复制出的自定义字体必须可加载");
        assert_eq!(custom_identity, source_identity, "复制字体身份必须与源一致");
        // 对抗性夹具：索引首条目 = 系统链主字面本身（索引在「推荐」系统字体）。
        write_decoy_index(&idx_path, &primary.path, &tmp.join("fonts"));
        // SAFETY：同上。
        unsafe {
            std::env::set_var("KANESUMI_TEST_FONT", custom.to_string_lossy().as_ref());
            std::env::set_var("KANESUMI_FONT_INDEX", "1");
        }
        let picked = kanesumi_harness::platform::find_font_source().expect("主字面必须存在");
        assert_eq!(picked.path, custom, "自定义字体被系统索引/链盖掉");
        assert_eq!(picked.collection_tag, None, "自定义字体不得被附加 TTC 标签");
        assert_eq!(
            engine_identity(&picked),
            Some(custom_identity),
            "自定义字体真实面身份被改变（诱饵索引首条目 = {}）",
            primary.path.display()
        );
        println!("场景 C：自定义字体 = {}（源 {}）", custom.display(), source.display());
    } else {
        println!("skip 场景 C：本机无可与主字面区分的备选真实字体");
    }

    restore(saved);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// 恢复环境三元组（与 `platform.rs::font_index_env_gate_and_fallbacks` 同款）。
fn restore(saved: (Option<String>, Option<String>, Option<String>)) {
    // SAFETY：唯一 #[test] 已收尾，单线程顺序执行。
    unsafe {
        if let Some(v) = saved.0 {
            std::env::set_var("KANESUMI_FONT_INDEX", v);
        } else {
            std::env::remove_var("KANESUMI_FONT_INDEX");
        }
        if let Some(v) = saved.1 {
            std::env::set_var("XDG_RUNTIME_DIR", v);
        } else {
            std::env::remove_var("XDG_RUNTIME_DIR");
        }
        if let Some(v) = saved.2 {
            std::env::set_var("KANESUMI_TEST_FONT", v);
        } else {
            std::env::remove_var("KANESUMI_TEST_FONT");
        }
    }
}
