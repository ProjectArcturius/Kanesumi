//! toml_lite.rs —— 项目文件用的 TOML **子集**读取器。
//!
//! 存在理由：`kanesumi package` 只读一个项目文件（`Kanesumi.toml`），字段固定十几项。
//! 为这点需求拉 `toml` + `serde` 进来，会让工具链在离线 / 无 vendor 的机器上装不上；
//! 而本文件是纯标准库的九十来行，跟着 CLI 一起编译，任何机器都能跑。
//!
//! 支持的语法（够用且明确报错，不做「差不多解析」）：
//! - 注释：`#` 到行尾（字符串内的 `#` 不切）。
//! - 分节：`[identity]`、`[role]`、`[entry]`… 单层表名。
//! - 键值：`key = value`，`key` 允许 `a.b` 这种点号（本工具用不到，但允许）。
//! - 值：双引号字符串（支持 `\"` 与 `\\`）、整数、浮点、布尔、单行字符串数组 `["a", "b"]`。
//!
//! 不支持：多行字符串、内联表、日期、数组换行。遇到就报错，不猜。

use std::collections::BTreeMap;

/// 一个 TOML 值。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// 字符串。
    Str(String),
    /// 整数。
    Int(i64),
    /// 浮点。
    Float(f64),
    /// 布尔。
    Bool(bool),
    /// 字符串数组。
    List(Vec<String>),
}

impl Value {
    /// 取字符串；类型不符返回 `None`。
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }


    /// 取字符串数组；类型不符返回 `None`。
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            Self::List(v) => Some(v),
            _ => None,
        }
    }
}

/// 解析后的 TOML 子集：`(“表.键”, 值)`。
///
/// 用扁平键而非嵌套表：本工具按名字取十来个字段，扁平键够用且省掉一层 `BTreeMap` 嵌套。
#[derive(Debug, Default)]
pub struct Doc {
    entries: BTreeMap<String, Value>,
}

impl Doc {
    /// 按 `表.键` 取值；顶层键直接写键名。
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.entries.get(key)
    }

    /// 取字符串，缺省返回空串。
    pub fn str_or(&self, key: &str, default: &str) -> String {
        self.get(key).and_then(Value::as_str).unwrap_or(default).to_string()
    }

    /// 取字符串数组，缺省返回空。
    pub fn list_or(&self, key: &str) -> Vec<String> {
        self.get(key).and_then(Value::as_list).map(<[String]>::to_vec).unwrap_or_default()
    }

    /// 全部键（测试与诊断用）。
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.entries.keys()
    }
}

/// 解析 `text`；出错时返回带行号的中文说明。
pub fn parse(text: &str) -> Result<Doc, String> {
    let mut doc = Doc::default();
    let mut table = String::new();

    for (i, raw) in text.lines().enumerate() {
        let line_no = i + 1;
        let line = strip_comment(raw).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest
                .strip_suffix(']')
                .ok_or_else(|| format!("第 {line_no} 行：表名缺少 `]` —— {line}"))?;
            let name = name.trim();
            if name.is_empty() || name.contains(['[', ']']) {
                return Err(format!("第 {line_no} 行：表名不合法 —— {name}"));
            }
            table = name.to_string();
            continue;
        }

        let (key, val) = line
            .split_once('=')
            .ok_or_else(|| format!("第 {line_no} 行：缺少 `=` —— {line}"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(format!("第 {line_no} 行：键为空 —— {line}"));
        }
        let full = if table.is_empty() { key.to_string() } else { format!("{table}.{key}") };
        let value = parse_value(val.trim())
            .map_err(|e| format!("第 {line_no} 行（{full}）：{e}"))?;
        doc.entries.insert(full, value);
    }

    Ok(doc)
}

/// 切掉行内注释：`#` 之后丢弃，但字符串内部的 `#` 保留。
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_str = false;
    let mut escaped = false;
    for (i, b) in bytes.iter().enumerate() {
        let c = *b as char;
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == '#' {
            return &line[..i];
        }
    }
    line
}

/// 解析一个值。
fn parse_value(s: &str) -> Result<Value, String> {
    if s.is_empty() {
        return Err("值为空".into());
    }
    if s.starts_with('"') {
        return parse_string(s).map(Value::Str);
    }
    if s.starts_with('[') {
        return parse_list(s);
    }
    match s {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        _ => {}
    }
    if let Ok(i) = s.parse::<i64>() {
        return Ok(Value::Int(i));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Ok(Value::Float(f));
    }
    Err(format!("无法识别的值 —— {s}（字符串请加双引号）"))
}

/// 解析双引号字符串（必须整段用掉 `s`）。
fn parse_string(s: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    // 跳过开引号。
    chars.next();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 't')) => out.push('\t'),
                Some((_, other)) => return Err(format!("不支持的转义 `\\{other}`")),
                None => return Err("字符串以 `\\` 结尾".into()),
            },
            '"' => {
                let rest = s[i + 1..].trim();
                if !rest.is_empty() {
                    return Err(format!("字符串后面还有内容 —— {rest}"));
                }
                return Ok(out);
            }
            other => out.push(other),
        }
    }
    Err("字符串缺少收尾引号".into())
}

/// 解析单行字符串数组：`["a", "b"]`。
fn parse_list(s: &str) -> Result<Value, String> {
    let inner = s
        .strip_prefix('[')
        .and_then(|r| r.strip_suffix(']'))
        .ok_or_else(|| format!("数组缺少 `]` —— {s}"))?;
    let mut out = Vec::new();
    for part in split_top_level(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if !part.starts_with('"') {
            return Err(format!("数组元素必须是字符串 —— {part}"));
        }
        out.push(parse_string(part)?);
    }
    Ok(Value::List(out))
}

/// 按逗号切分，但不切字符串内部的逗号。
fn split_top_level(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_str = false;
    let mut escaped = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        if in_str {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
        } else if c == '"' {
            in_str = true;
        } else if c == ',' {
            out.push(&s[start..i]);
            start = i + 1;
        }
    }
    out.push(&s[start..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_strings_bools_numbers_and_lists() {
        let text = r#"
# 顶注
name = "显示设置"          # 行尾注释也吃掉
version = 3
ratio = 0.5
flag = true
list = ["a", "b"]

[identity]
app_id = "org.ether.demo"
"#;
        let d = parse(text).unwrap();
        assert_eq!(d.get("name").and_then(Value::as_str), Some("显示设置"));
        assert_eq!(d.get("version"), Some(&Value::Int(3)));
        assert_eq!(d.get("ratio"), Some(&Value::Float(0.5)));
        assert_eq!(d.get("flag"), Some(&Value::Bool(true)));
        assert_eq!(d.list_or("list"), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(d.str_or("identity.app_id", ""), "org.ether.demo");
    }

    #[test]
    fn keeps_hash_inside_strings() {
        let d = parse(r#"summary = "v1 #2 修正""#).unwrap();
        assert_eq!(d.str_or("summary", ""), "v1 #2 修正");
    }

    #[test]
    fn rejects_bad_input_with_line_number() {
        let err = parse("ok = 1\nbroken\n").unwrap_err();
        assert!(err.contains("第 2 行"), "错误信息应带行号：{err}");

        let err = parse("x = unquoted").unwrap_err();
        assert!(err.contains("加双引号"), "未加引号的字符串应明确提示：{err}");

        let err = parse("x = [\"a\"").unwrap_err();
        assert!(err.contains("]"), "缺 `]` 应报错：{err}");
    }
}
