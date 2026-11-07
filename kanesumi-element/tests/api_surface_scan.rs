// 公共 API 表面的扫描逻辑（**不是测试**，被 tests/api_surface.rs 与 examples/api_surface.rs 共用）。
//
// 为什么单独一份：生成（示例）与校验（测试）必须用同一套规则，否则「快照」与
// 「实际」会漂移到没人能解释的地步。
//
// 扫描规则见文件末尾注释；语义级比对不做（会随宏与条件编译抖动）。

#[allow(dead_code)]
use std::path::{Path, PathBuf};

/// 受管的库 crate（顺序即快照里的分节顺序）。
///
/// 判据不是「是不是 lib」，而是「下游应用会不会直接 use 它」：core / canvas / element /
/// controls 是应用直接消费的四层，a11y 是外壳要接的桥 —— 这五个的公共面一旦被应用引用，
/// 改动就是破坏。render 是外壳内部实现，暂不纳入（它没有稳定的应用侧契约）。
pub const CRATES: [&str; 5] = [
    "kanesumi-core",
    "kanesumi-canvas",
    "kanesumi-element",
    "kanesumi-controls",
    "kanesumi-a11y",
];

/// workspace 根（各 crate 的上一级）。
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace 根")
        .to_path_buf()
}

fn walk(dir: &Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<_> = entries.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk(&p, out);
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        for line in text.lines() {
            let t = line.trim_start();
            let is_pub = t.starts_with("pub fn ")
                || t.starts_with("pub const fn ")
                || t.starts_with("pub struct ")
                || t.starts_with("pub enum ")
                || t.starts_with("pub trait ");
            if !is_pub {
                continue;
            }
            let sig = t
                .split("//")
                .next()
                .unwrap_or(t)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            if !sig.is_empty() {
                out.push(sig);
            }
        }
    }
}

/// 扫一个 crate 的 `src/**.rs`，返回排序去重的公开签名行。
pub fn scan_crate(crate_name: &str) -> Vec<String> {
    let mut out = Vec::new();
    walk(&workspace_root().join(crate_name).join("src"), &mut out);
    out.sort();
    out.dedup();
    out
}

/// 渲染整份快照文本。
pub fn render() -> String {
    let mut out = String::new();
    out.push_str("# Kanesumi 公共 API 表面快照（自动生成，勿手改）。\n");
    out.push_str("# 生成：cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt\n");
    out.push_str("# 校验：cargo test -p kanesumi-element --test api_surface\n");
    out.push_str("# 读法：分节 = crate；每行 = 一条 pub 签名（归一化空白）。参 docs/API_STABILITY.md。\n");
    for c in CRATES {
        out.push_str(&format!("\n## {c}\n"));
        for sig in scan_crate(c) {
            out.push_str(&sig);
            out.push('\n');
        }
    }
    out
}
