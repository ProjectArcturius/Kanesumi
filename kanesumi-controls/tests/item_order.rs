// item_order.rs —— 静态检查：生产代码不得写在测试模块之后。
//
// 存在理由：`size_discipline` / `token_discipline` 的「生产段」定义是「文件里首个
// `#[cfg(test)]` 之前的行」。这条定义只有在测试模块位于文件末尾时才等价于「全部生产代码」。
// 一旦有人把生产项补在测试模块后面，那段代码就永久落在两道检查的扫描范围之外 ——
// m1115_cleanup 之前 `notification_card.rs` 的旧渲染段（400+ 行）正是这样躲过检查的。
//
// 规则：首个 `#[cfg(test)] mod … { … }` 闭合之后，只允许空白、注释、以及其它
// `#[cfg(test)]` 模块。出现任何其它内容即红。
//
// 修法固定为一条：把测试模块整块挪到文件末尾（只搬位置，不改测试内容，不改生产代码）；
// 多个测试模块按原顺序全部放在末尾。
//
// 不引新依赖：花括号配对按字符手写，跳过注释、字符串、字符字面量与 raw string。

use std::fs;
use std::path::{Path, PathBuf};

/// 一处违规：文件、行号（1 起）、首个违规项的名字。
#[derive(Debug)]
struct Violation {
    file: String,
    line: usize,
    item: String,
}

/// 把注释与字面量内容替换成空格，保留换行与字符数（行号与列号都不变）。
///
/// 必须做这一步：花括号可能出现在注释（`// {`）、字符串（`"{"`）、字符字面量
/// （`'{'`）与 raw string（模板里的 `{name}`）中，直接数会错配。
fn blank_literals(src: &str) -> String {
    let chars: Vec<char> = src.chars().collect();
    let mut out = chars.clone();
    let at = |i: usize| chars.get(i).copied();
    let mut i = 0;
    while i < chars.len() {
        // 行注释：吃到行尾（换行保留）。
        if at(i) == Some('/') && at(i + 1) == Some('/') {
            while i < chars.len() && chars[i] != '\n' {
                out[i] = ' ';
                i += 1;
            }
            continue;
        }
        // 块注释：Rust 允许嵌套。
        if at(i) == Some('/') && at(i + 1) == Some('*') {
            let mut depth = 0usize;
            while i < chars.len() {
                if at(i) == Some('/') && at(i + 1) == Some('*') {
                    depth += 1;
                    out[i] = ' ';
                    out[i + 1] = ' ';
                    i += 2;
                    continue;
                }
                if at(i) == Some('*') && at(i + 1) == Some('/') {
                    depth -= 1;
                    out[i] = ' ';
                    out[i + 1] = ' ';
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                    continue;
                }
                if chars[i] != '\n' {
                    out[i] = ' ';
                }
                i += 1;
            }
            continue;
        }
        // raw string：r"…" / r#"…"# / br#"…"#（可带任意个 #）。
        let raw_prefix = if at(i) == Some('r')
            && !(i > 0 && is_ident(chars[i - 1]))
            && (at(i + 1) == Some('"') || at(i + 1) == Some('#'))
        {
            Some(i + 1)
        } else if at(i) == Some('b')
            && at(i + 1) == Some('r')
            && !(i > 0 && is_ident(chars[i - 1]))
            && (at(i + 2) == Some('"') || at(i + 2) == Some('#'))
        {
            Some(i + 2)
        } else {
            None
        };
        if let Some(mut j) = raw_prefix {
            let mut hashes = 0usize;
            while at(j) == Some('#') {
                hashes += 1;
                j += 1;
            }
            if at(j) == Some('"') {
                j += 1;
                // 找 `"` + hashes 个 `#`。
                while j < chars.len() {
                    if chars[j] == '"' {
                        let mut k = j + 1;
                        let mut seen = 0usize;
                        while seen < hashes && at(k) == Some('#') {
                            seen += 1;
                            k += 1;
                        }
                        if seen == hashes {
                            j = k;
                            break;
                        }
                    }
                    j += 1;
                }
                for slot in out.iter_mut().take(j.min(chars.len())).skip(i) {
                    if *slot != '\n' {
                        *slot = ' ';
                    }
                }
                i = j;
                continue;
            }
        }
        // 普通字符串：转义跳过。
        if at(i) == Some('"') {
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    break;
                }
                i += 1;
            }
            for slot in out.iter_mut().take(i).skip(start) {
                if *slot != '\n' {
                    *slot = ' ';
                }
            }
            continue;
        }
        // 字符字面量（含 '{' / '}' / '\n' / '\u{…}'）；裸 'a 是生命周期，放行。
        if at(i) == Some('\'') {
            let mut j = i + 1;
            let mut is_char = false;
            if at(j) == Some('\\') {
                j += 2;
                if at(j - 1) == Some('u') && at(j) == Some('{') {
                    while j < chars.len() && chars[j] != '}' {
                        j += 1;
                    }
                    j += 1;
                }
                is_char = at(j) == Some('\'');
            } else if at(j).is_some() && at(j) != Some('\'') && at(j + 1) == Some('\'') {
                j += 2;
                is_char = true;
            }
            if is_char {
                for slot in out.iter_mut().take(j).skip(i) {
                    *slot = ' ';
                }
                i = j;
                continue;
            }
        }
        i += 1;
    }
    out.into_iter().collect()
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn starts_word(chars: &[char], at: usize, word: &str) -> bool {
    let w: Vec<char> = word.chars().collect();
    if at + w.len() > chars.len() || chars[at..at + w.len()] != w[..] {
        return false;
    }
    match chars.get(at + w.len()) {
        Some(c) => !is_ident(*c),
        None => true,
    }
}

/// 找 `#[cfg(test)]`（返回 `#` 的下标）。
fn find_cfg_test(chars: &[char], from: usize) -> Option<usize> {
    let needle: Vec<char> = "#[cfg(test)]".chars().collect();
    let mut i = from;
    while i + needle.len() <= chars.len() {
        if chars[i..i + needle.len()] == needle[..] {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// 从 `#[cfg(test)]` 起找到它修饰的 `mod … { … }`，返回 `(开括号, 闭括号)` 下标。
fn module_after(chars: &[char], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    loop {
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            return None;
        }
        // 中间夹的属性（如 #[allow(...)]）：跳到 `]`。
        if chars[i] == '#' && chars.get(i + 1) == Some(&'[') {
            while i < chars.len() && chars[i] != ']' {
                i += 1;
            }
            i += 1;
            continue;
        }
        if starts_word(chars, i, "pub") {
            i += 3;
            continue;
        }
        if !starts_word(chars, i, "mod") {
            return None;
        }
        i += 3;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        while i < chars.len() && is_ident(chars[i]) {
            i += 1;
        }
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if chars.get(i) != Some(&'{') {
            return None;
        }
        return matching_brace(chars, i).map(|close| (i, close));
    }
}

fn matching_brace(chars: &[char], open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in chars.iter().enumerate().skip(open) {
        match c {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// 首个违规项的名字：跳过属性与注释行，取实际项的关键字 + 标识符。
fn item_name(src: &str, line: usize) -> String {
    let lines: Vec<&str> = src.lines().collect();
    let mut idx = line.saturating_sub(1);
    while idx < lines.len() {
        let t = lines[idx].trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with("//") {
            idx += 1;
            continue;
        }
        let mut t = t;
        for vis in ["pub(crate) ", "pub(super) ", "pub "] {
            if let Some(rest) = t.strip_prefix(vis) {
                t = rest.trim_start();
                break;
            }
        }
        if let Some(rest) = t.strip_prefix("impl ") {
            let head: String = rest.chars().take_while(|c| *c != '{').collect();
            return format!("impl {}", head.trim()).chars().take(80).collect();
        }
        for kw in [
            "struct ",
            "enum ",
            "trait ",
            "union ",
            "fn ",
            "const ",
            "static ",
            "type ",
            "mod ",
            "use ",
            "macro_rules! ",
        ] {
            if let Some(rest) = t.strip_prefix(kw) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '!')
                    .collect();
                if !name.is_empty() {
                    return format!("{} {name}", kw.trim_end());
                }
            }
        }
        return t.chars().take(60).collect();
    }
    "（未能定位项名）".to_string()
}

/// 扫一份源码：返回首个违规（每文件最多一处，修完再跑即可看到下一处）。
fn scan_source(file: &str, src: &str) -> Vec<Violation> {
    let chars: Vec<char> = blank_literals(src).chars().collect();
    // `#[cfg(test)]` 也标注函数 / 字段（本工作区有 14 处），那些不是测试模块，
    // 不能拿来当作「测试段起点」—— 要一直找到真正修饰 `mod` 的那一个。
    let mut search = 0;
    let mut cfg = loop {
        let Some(at) = find_cfg_test(&chars, search) else {
            return Vec::new();
        };
        if module_after(&chars, at).is_some() {
            break at;
        }
        search = at + 1;
    };
    loop {
        let Some((_, close)) = module_after(&chars, cfg) else {
            return Vec::new();
        };
        let mut i = close + 1;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        if i >= chars.len() {
            return Vec::new();
        }
        let rest: String = chars[i..].iter().collect();
        // 只放行其它 `#[cfg(test)] mod`：cfg(test) 标注的函数不算「测试模块」。
        if rest.starts_with("#[cfg(test)]") && module_after(&chars, i).is_some() {
            cfg = i;
            continue;
        }
        let line = chars[..i].iter().filter(|c| **c == '\n').count() + 1;
        return vec![Violation {
            file: file.to_string(),
            line,
            item: item_name(src, line),
        }];
    }
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

/// 工作区根与其中所有 `kanesumi-*` crate 的 `src/`。
fn workspace_sources() -> (PathBuf, Vec<PathBuf>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("kanesumi-controls 应有上级目录")
        .to_path_buf();
    let mut files = Vec::new();
    let Ok(entries) = fs::read_dir(&root) else {
        return (root, files);
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        if path.is_dir() && name.starts_with("kanesumi-") {
            rs_files(&path.join("src"), &mut files);
        }
    }
    files.sort();
    (root, files)
}

#[test]
fn no_production_code_after_test_modules() {
    let (root, files) = workspace_sources();
    // 空集合会让检查静默通过 —— 与本检查要防的病同源，先行断言。
    assert!(
        files.len() >= 150,
        "扫描到的源文件仅 {} 个，检查形同虚设（工作区根：{}）",
        files.len(),
        root.display()
    );

    let mut violations = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let name = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        violations.extend(scan_source(&name, &text));
    }

    assert!(
        violations.is_empty(),
        "生产代码写在测试模块之后（{} 处）。这些行永远不会被 size_discipline / \
         token_discipline 扫到：\n{}\n\
         修法：把该文件里的 #[cfg(test)] 模块整块挪到文件末尾（只搬位置，不改测试内容，\
         不改生产代码）；多个测试模块按原顺序全部放在末尾。",
        violations.len(),
        violations
            .iter()
            .map(|v| format!("  {}:{}  {}", v.file, v.line, v.item))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn checker_detects_violation_and_accepts_compliant_layout() {
    // 违规：测试模块之后还有生产结构体（含属性行与文档注释）。
    let bad = "\
pub fn before() {}
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!(\"{\", \"{\"); }
}

// ── 后补的生产段 ──
/// 文档。
#[derive(Debug, Default)]
pub struct ListRowMeta {
    pub cells: Vec<String>,
}
";
    let hits = scan_source("bad.rs", bad);
    assert_eq!(hits.len(), 1, "应恰好抓到 1 处：{hits:?}");
    assert_eq!(hits[0].line, 10, "行号应指向首个违规项的属性行");
    assert_eq!(hits[0].item, "struct ListRowMeta", "应报出违规项名字");

    // 合规：生产在前，一个或多个测试模块在末尾；尾部注释不算内容。
    let good = "\
pub fn before() {}
const BRACE: char = '{';
const TEXT: &str = \"}\";
// cfg(test) 标注的函数不是测试模块，检查应跳过它继续找真正的 mod。
#[cfg(test)]
fn test_only_helper() {}
pub fn middle() {}
#[cfg(test)]
mod a {
    #[test]
    fn t() { let _ = '{'; }
}
// 两个测试模块连续放末尾是允许的。
#[cfg(test)]
mod b {
    #[test]
    fn u() { let _ = r#\"{ raw }\"#; }
}
// 文件末尾的注释也允许。
";
    assert!(scan_source("good.rs", good).is_empty(), "合规布局不应报红");

    // 字面量里的花括号不得干扰配对：字符串 / 字符 / raw string / 块注释。
    let braces_in_literals = "\
fn f() -> String { \"{\".to_string() }
const R: &str = r#\"fn fake() { }\"#;
/* { #[cfg(test)] mod x { } */
#[cfg(test)]
mod tests {
    #[test]
    fn t() { assert_eq!('}', '}'); }
}
pub fn after() {}
";
    let hits = scan_source("braces.rs", braces_in_literals);
    assert_eq!(hits.len(), 1, "字面量里的花括号不应影响配对：{hits:?}");
    assert_eq!(hits[0].item, "fn after");
}
