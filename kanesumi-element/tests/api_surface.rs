// 公共 API 表面快照门（落地前的接口稳定闸门）。
//
// 为什么要有它：Kanesumi 一旦被上层应用（settings / librarian / launcher）消费，
// 改接口就是改别人的编译错误。人工「注意一下」守不住 —— 必须有一条会红的线。
//
// 机制：把四个库 crate 的 `pub fn` / `pub struct` / `pub enum` / `pub trait` 签名
// 扫成排序去重的清单，与本目录同级的 `docs/API_SURFACE.txt` 比对。**任何增删改签名
// 都会红**，于是每一次接口变动都必须显式重新生成并写进提交信息 —— 变更是有意的。
//
// 重新生成（只在**确认**这次变动可以对外时才做；生成走示例，因为测试进程在沙箱里
// 不允许写源码树）：
//
// ```text
// cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt
// git diff docs/API_SURFACE.txt   # 逐行看，确认没有误删
// ```
//
// 规则见 `tests/api_surface_scan.rs`；策略见 `docs/API_STABILITY.md` §四。

include!("api_surface_scan.rs");

#[test]
fn public_api_surface_matches_snapshot() {
    let snapshot_path = workspace_root().join("docs/API_SURFACE.txt");
    let now = render();
    let old = std::fs::read_to_string(&snapshot_path).unwrap_or_else(|_| {
        panic!(
            "缺少 {}：先跑 cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt",
            snapshot_path.display()
        )
    });
    if now == old {
        return;
    }

    // 只打印差异行，便于一眼看出「谁动了接口」。
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = now.lines().collect();
    let added: Vec<&&str> = new_lines.iter().filter(|l| !old_lines.contains(*l)).collect();
    let removed: Vec<&&str> = old_lines.iter().filter(|l| !new_lines.contains(*l)).collect();
    panic!(
        "公共 API 表面变了（新增 {} 行 / 移除 {} 行）。\n\
         若**确认**这次变动可以对外：cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt，\
         逐行 review 后再提交；否则改回去。\n\
         --- 新增 ---\n{}\n--- 移除 ---\n{}",
        added.len(),
        removed.len(),
        added
            .iter()
            .map(|l| format!("  + {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
        removed
            .iter()
            .map(|l| format!("  - {l}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}
