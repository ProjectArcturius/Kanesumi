// token_discipline.rs —— M1-3 静态检查：控件**生产段**不得内联颜色字面量。
//
// 存在理由（对标 WinUI）：「写错的颜色必须报错，不能静默回退成另一个样子」
// （`XamlResourceReferenceFailed` 的教训，参 `docs/REFERENCE.md` §Ⅲ）。Rust 侧没有 XAML 的
// 资源解析期，唯一等价物就是本检查：只要控件生产段里出现颜色构造器或裸 alpha 数字，
// 这里就红。
//
// 规则（参 `docs/ROADMAP.md` M1-3）：
//   1. 禁止 `Color::from_hex` / `from_rgba` / `rgb` / `rgba` / `new` ——
//      颜色的**定义**只能发生在令牌层（`kanesumi-core`），控件只消费令牌；
//   2. 禁止 `with_alpha(<数字>)` —— 强度也要具名（`MetroIndication` 的 tint 与不透明度档）。
//
// 扫描范围是**生产段**：首个 `#[cfg(test)]` 之前的行。测试里写死颜色是为了断言，
// 不构成产品缺陷，强行令牌化反而会让断言失去独立性。
//
// 例外只有一条：没有例外。需要新颜色时，正确的做法是在 `kanesumi-core` 里加令牌
// （并在 `docs/CANON_VS_TEMPORARY.md` 登记无权威来源的取值），而不是在此处网开一面。

use std::fs;
use std::path::{Path, PathBuf};

/// 禁止的颜色构造器（令牌层才有资格出现）。
const FORBIDDEN_CONSTRUCTORS: [&str; 5] = [
    "Color::from_hex(",
    "Color::from_rgba(",
    "Color::rgb(",
    "Color::rgba(",
    "Color::new(",
];

/// 生产段 = 文件里首个 `#[cfg(test)]` 之前的行。
///
/// 返回 `(行号, 行内容)`；行号从 1 起，便于直接对应编辑器。
fn production_lines(src: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    for (i, line) in src.lines().enumerate() {
        if line.trim() == "#[cfg(test)]" {
            break;
        }
        out.push((i + 1, line));
    }
    out
}

/// 去掉行内的注释部分，避免「注释里提到 `Color::from_hex`」被误判。
///
/// 逐字符扫描并跟踪字符串字面量，故 `"http://…"` 这类内容不会被当成注释切掉。
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut escaped = false;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i] as char;
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_string = true;
            i += 1;
            continue;
        }
        if c == '/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            return &line[..i];
        }
        i += 1;
    }
    line
}

/// `with_alpha(` 之后紧跟数字（可能带前导空白）即为违规。
fn has_bare_numeric_alpha(code: &str) -> bool {
    let needle = "with_alpha(";
    let mut from = 0;
    while let Some(pos) = code[from..].find(needle) {
        let start = from + pos + needle.len();
        let rest = &code[start..];
        let first = rest.trim_start().chars().next();
        if matches!(first, Some(c) if c.is_ascii_digit()) {
            return true;
        }
        from = start;
    }
    false
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn no_inline_colors_in_production_code() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();
    // 空集合会让检查「静默通过」—— 与本检查要防的病同源，故先行断言。
    assert!(
        files.len() >= 40,
        "扫描到的源文件仅 {} 个，检查形同虚设（路径：{}）",
        files.len(),
        src.display()
    );

    let mut violations: Vec<String> = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let name = path
            .strip_prefix(&src)
            .unwrap_or(path)
            .display()
            .to_string();
        for (line_no, raw) in production_lines(&text) {
            let code = strip_comment(raw);
            for ctor in FORBIDDEN_CONSTRUCTORS {
                if code.contains(ctor) {
                    violations.push(format!(
                        "{name}:{line_no} 出现 {ctor}… —— 颜色定义只能写在 kanesumi-core 的令牌里"
                    ));
                }
            }
            if has_bare_numeric_alpha(code) {
                violations.push(format!(
                    "{name}:{line_no} 出现 with_alpha(<数字>) —— 强度须用具名令牌（MetroIndication）"
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "控件生产段不得内联颜色字面量（共 {} 处）：\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}

/// 反向自检：检查器本身必须真的抓得到违规 ——
/// 否则「绿」可能只是因为它什么都没检查。
#[test]
fn checker_detects_violations_and_respects_test_sections() {
    let sample = r#"
use kanesumi_core::Color;
fn f(c: Color) -> Color {
    let a = Color::from_hex(0xFF_FF_FF);      // 违规：构造器
    // 注释里出现 Color::rgb(0,0,0) 不算违规
    let b = c.with_alpha(0.42);               // 违规：裸 alpha
    let ok = c.with_alpha(theme.indication.base_medium_high); // 合规
    a.lerp(b, 0.5).lerp(ok, 0.5)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn t() { let _ = Color::from_hex(0x00_00_00).with_alpha(0.5); } // 测试段：不计
}
"#;
    let lines = production_lines(sample);
    assert_eq!(lines.len(), 9, "生产段应在 #[cfg(test)] 处截断");

    let mut hits = 0;
    for (_, raw) in &lines {
        let code = strip_comment(raw);
        if FORBIDDEN_CONSTRUCTORS.iter().any(|c| code.contains(c)) || has_bare_numeric_alpha(code)
        {
            hits += 1;
        }
    }
    assert_eq!(hits, 2, "应恰好抓到 2 处（构造器 + 裸 alpha）");
}
