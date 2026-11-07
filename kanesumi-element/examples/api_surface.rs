// 打印公共 API 表面快照（供 `docs/API_SURFACE.txt` 生成与人工 review）。
//
// 用法（在仓根执行，重定向由 shell 完成 —— 测试进程在沙箱里不允许写源码树）：
//
// ```text
// cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt
// ```
//
// 校验由 `tests/api_surface.rs` 负责；扫描规则在 `tests/api_surface_scan.rs`（两边共用）。

include!("../tests/api_surface_scan.rs");

fn main() {
    print!("{}", render());
}
