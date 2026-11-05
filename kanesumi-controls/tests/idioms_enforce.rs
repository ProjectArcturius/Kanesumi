// idioms_enforce.rs —— IDIOMS §Ⅸ 强制层：静态检查（n1104-idioms）。
//
// 与 `token_discipline.rs`（M1-3，controls 裸色）同族：静态扫源码，把「类 .NET 手感」里
// 能机械判定的条款变成测试。分工：
//   - 裸色值（R1）：controls 域已由 token_discipline.rs 把守；本检查把它**推广到
//     kanesumi-element**（现势生产段零违例，2026-11-05 对真）；
//   - 裸尺寸（R2）/ 命中内手算矩形（R3）/ 动画路径墙钟（R4）：作用于 **MANIFEST 清单**——
//     新卡 / 新文件登记后才受约束，先只对新增代码生效，不批量改旧代码（IDIOMS §Ⅸ）；
//     旧代码的历史豁免记 `docs/CANON_VS_TEMPORARY.md`，随 E5 退役。
//
// 反向自检：每条规则喂「应报」与「不应报」两种合成源码，防止检查形同虚设或误报。

use std::fs;
use std::path::{Path, PathBuf};

/// 新增代码清单（相对 kanesumi 仓根）。新卡在此登记自己的生产源文件后才受 R2-R4 约束。
const MANIFEST: &[&str] = &["kanesumi-element/src/access.rs"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// 生产段 = 首个 `#[cfg(test)]` 之前的行（同 token_discipline 语义）。
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

/// 去行注释（同 token_discipline 的逐字符扫描：字符串字面量里的 `//` 不算注释）。
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

/// `sym` 的独立出现（前一字符不是标识符字符）——避免 `ThemeColor::` 误伤 `Color::`。
fn contains_type_use(code: &str, sym: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = code[from..].find(sym) {
        let start = from + pos;
        let prev_ok = start == 0
            || !code[..start]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_');
        if prev_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

/// 取签名行含 `sig` 的函数体（行号, 行）；按花括号配对收束，`;` 结尾的 trait 声明无体。
fn fn_body_lines<'a>(prod: &'a str, sig: &str) -> Vec<(usize, &'a str)> {
    let mut out = Vec::new();
    let mut depth: i64 = 0;
    let mut inside = false;
    for (i, line) in prod.lines().enumerate() {
        if inside {
            depth += line.matches('{').count() as i64;
            depth -= line.matches('}').count() as i64;
            out.push((i + 1, line));
            if depth <= 0 {
                inside = false;
            }
        } else if strip_comment(line).contains(sig) {
            depth = line.matches('{').count() as i64 - line.matches('}').count() as i64;
            if depth > 0 {
                inside = true;
                out.push((i + 1, line));
            }
        }
    }
    out
}

/// R2：裸尺寸 —— `Size::new( / Insets::all( / Insets::new(` 后紧跟 1-9 的字面量。
/// （0 / 令牌 / 具名 const 不拦；测试夹具的字面量在生产段之外，天然豁免。）
fn bare_size(code: &str) -> bool {
    for needle in ["Size::new(", "Insets::all(", "Insets::new("] {
        let mut from = 0;
        while let Some(pos) = code[from..].find(needle) {
            let rest = code[from + pos + needle.len()..].trim_start();
            if matches!(rest.chars().next(), Some(c) if c.is_ascii_digit() && c != '0') {
                return true;
            }
            from += pos + needle.len();
        }
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

// ── R1：裸色值推广到 kanesumi-element（controls 域由 token_discipline.rs 把守）────────

#[test]
fn r1_no_inline_colors_in_element_production() {
    let src = repo_root().join("kanesumi-element/src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();
    assert!(
        files.len() >= 10,
        "扫描到的源文件仅 {} 个，检查形同虚设（路径：{}）",
        files.len(),
        src.display()
    );

    let mut violations = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let name = path.strip_prefix(&src).unwrap_or(path).display().to_string();
        for (line_no, raw) in production_lines(&text) {
            if contains_type_use(strip_comment(raw), "Color::") {
                violations.push(format!(
                    "element/{name}:{line_no} 出现 Color::… —— 颜色只用 ThemeColor / theme.colors（IDIOMS §Ⅵ）"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "kanesumi-element 生产段裸色值 {} 处：\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// ── R2-R4：MANIFEST 清单域（新增代码）───────────────────────────────────────────

#[test]
fn manifest_rules_hold() {
    assert!(!MANIFEST.is_empty(), "清单为空，检查形同虚设（IDIOMS §Ⅸ）");
    let mut violations = Vec::new();
    for rel in MANIFEST {
        let path = repo_root().join(rel);
        let Ok(text) = fs::read_to_string(&path) else {
            violations.push(format!("{rel} 无法读取（清单指向的文件必须存在）"));
            continue;
        };
        let prod_lines = production_lines(&text);
        let prod: String = prod_lines
            .iter()
            .map(|(_, l)| *l)
            .collect::<Vec<_>>()
            .join("\n");
        // R2 裸尺寸。
        for (line_no, raw) in &prod_lines {
            if bare_size(strip_comment(raw)) {
                violations.push(format!(
                    "{rel}:{line_no} 裸尺寸字面量 —— 尺寸只用尺寸正典 / 具名 const（IDIOMS §Ⅵ）：{raw}"
                ));
            }
        }
        // R3 命中内手算矩形：hit_test 体内禁 Rect::new（命中区 = bounds，不另算几何）。
        for (line_no, raw) in fn_body_lines(&prod, "fn hit_test") {
            if strip_comment(raw).contains("Rect::new(") {
                violations.push(format!(
                    "{rel}:{line_no} hit_test 内手算 Rect::new —— 命中区只能来自传入 rect（common 禁区）：{raw}"
                ));
            }
        }
        // R4 动画路径墙钟：update 体内禁 Instant::now()（走 ctx.animate 的真实时钟）。
        for (line_no, raw) in fn_body_lines(&prod, "fn update") {
            if strip_comment(raw).contains("Instant::now()") {
                violations.push(format!(
                    "{rel}:{line_no} update 内 Instant::now() —— 动画只走 ctx.animate（IDIOMS §Ⅲ）：{raw}"
                ));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "新增代码 IDIOMS 违例 {} 处：\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// ── 反向自检（防止检查形同虚设 / 误报）───────────────────────────────────────────

#[cfg(test)]
mod reverse {
    use super::*;

    #[test]
    fn r1_flags_colors_and_skips_tests() {
        let src = "fn paint(&mut self) { let c = Color::WHITE; }\n#[cfg(test)]\nmod t { fn x() { let c = Color::WHITE; } }\n";
        let prod = production_lines(src);
        let hit = prod
            .iter()
            .any(|(_, l)| contains_type_use(strip_comment(l), "Color::"));
        assert!(hit, "R1 必须报生产段裸色");
        let prod_head = prod.len();
        assert_eq!(prod_head, 1, "cfg(test) 之后的行必须被豁免");
        // 边界：ThemeColor::（令牌消费）与 SrgbColor:: 之类前缀型不得误伤。
        assert!(
            !contains_type_use("scene.fill_rect(theme.colors.surface, rect);", "Color::"),
            "theme.colors.x 不含独立 Color::"
        );
        assert!(!contains_type_use("let t = ThemeColor::Background;", "Color::"), "ThemeColor:: 不得误伤");
        assert!(contains_type_use("let t = Color::WHITE;", "Color::"), "裸 Color:: 应报");
    }

    #[test]
    fn r2_flags_nonzero_literals_and_allows_zero_and_tests() {
        assert!(bare_size("let s = Size::new(88.0, 32.0);"), "非零字面量应报");
        assert!(bare_size("let i = Insets::all(2.0);"), "非零字面量应报");
        assert!(!bare_size("let s = Size::new(0.0, 0.0);"), "零不拦");
        assert!(
            !bare_size("let s = Size::new(self.width, token::ROW_H);"),
            "变量 / 令牌不拦"
        );
        // 测试段：整段已被 production_lines 剥除，此处只证 bare_size 对裸行本会报。
        let stripped = production_lines("fn a() { Size::new(1.0, 0.0); }\n#[cfg(test)]\nmod t { fn b() { Size::new(2.0, 0.0); } }\n");
        assert_eq!(stripped.len(), 1, "测试段行被剥除");
        assert!(bare_size(stripped[0].1), "生产段裸尺寸应报");
    }

    #[test]
    fn r3_flags_rect_new_in_hit_test_only() {
        let src = "\
fn hit_test(&self, rect: Rect, pos: Point) -> bool {
    let extra = Rect::new(0.0, 0.0, 8.0, 8.0);
    rect.contains(pos)
}

fn arrange(&mut self, ctx: &mut ArrangeCtx, rect: Rect) {
    let slot = Rect::new(rect.origin.x, rect.origin.y, 8.0, 8.0);
    ctx.arrange_child(ctx.children()[0], slot);
}
";
        let hits = fn_body_lines(src, "fn hit_test");
        assert!(
            hits.iter().any(|(_, l)| l.contains("Rect::new(")),
            "R3 必须报 hit_test 体内手算矩形"
        );
        let arranges = fn_body_lines(src, "fn arrange");
        assert!(
            !arranges.iter().any(|(_, l)| l.contains("hit_test")),
            "arrange 体不属 hit_test（函数切分正确）"
        );
        assert_eq!(hits.len(), 4, "hit_test 体四行（签名 + 两行体 + 闭括号）");
    }

    #[test]
    fn r4_flags_instant_now_in_update_only() {
        let src = "\
fn update(&mut self, ctx: &mut UpdateCtx, dt: f64) {
    let t = Instant::now();
}
fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
    let t = Instant::now();
}
";
        assert!(
            fn_body_lines(src, "fn update")
                .iter()
                .any(|(_, l)| l.contains("Instant::now()")),
            "R4 必须报 update 内墙钟"
        );
        assert!(
            !fn_body_lines(src, "fn event")
                .iter()
                .any(|(_, l)| l.contains("fn update")),
            "event 体不属 update（函数切分正确）"
        );
    }

    #[test]
    fn trait_declarations_without_body_are_ignored() {
        let src = "fn hit_test(&self, rect: Rect, pos: Point) -> bool;\n";
        assert!(fn_body_lines(src, "fn hit_test").is_empty(), "trait 声明无函数体");
    }
}
