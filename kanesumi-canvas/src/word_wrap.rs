use std::collections::HashMap;

/// 是否属于 CJK 字符区间（中日韩统一表意文字、假名、谚文、标点符号）。
fn is_cjk(ch: char) -> bool {
    matches!(ch,
        '\u{1100}'..='\u{11FF}'   // Hangul Jamo
        | '\u{2E80}'..='\u{2EFF}' // CJK Radicals Supplement
        | '\u{3000}'..='\u{303F}' // CJK Symbols and Punctuation
        | '\u{3040}'..='\u{309F}' // Hiragana
        | '\u{30A0}'..='\u{30FF}' // Katakana
        | '\u{3100}'..='\u{312F}' // Bopomofo
        | '\u{3200}'..='\u{32FF}' // Enclosed CJK Letters and Months
        | '\u{3400}'..='\u{4DBF}' // CJK Unified Ideographs Extension A
        | '\u{4E00}'..='\u{9FFF}' // CJK Unified Ideographs
        | '\u{AC00}'..='\u{D7AF}' // Hangul Syllables
        | '\u{F900}'..='\u{FAFF}' // CJK Compatibility Ideographs
        | '\u{FF00}'..='\u{FFEF}' // Halfwidth and Fullwidth Forms
        | '\u{20000}'..='\u{2FA1F}' // CJK Extensions B..F
    )
}

enum BodyToken<'a> {
    Cjk(char),
    Word(&'a str),
    Space,
}

fn tokenize_body_line<'a>(line: &'a str) -> Vec<BodyToken<'a>> {
    let mut tokens = Vec::new();
    let mut word_start = None;

    for (idx, ch) in line.char_indices() {
        if ch.is_whitespace() {
            if let Some(start) = word_start.take() {
                tokens.push(BodyToken::Word(&line[start..idx]));
            }
            tokens.push(BodyToken::Space);
        } else if is_cjk(ch) {
            if let Some(start) = word_start.take() {
                tokens.push(BodyToken::Word(&line[start..idx]));
            }
            tokens.push(BodyToken::Cjk(ch));
        } else if word_start.is_none() {
            word_start = Some(idx);
        }
    }
    if let Some(start) = word_start {
        tokens.push(BodyToken::Word(&line[start..]));
    }
    tokens
}

/// 按调用方的单字宽度折行。中文按字折行，西文短词保持完整；仅在单词
/// 超过行宽时逐字拆分。超过行数上限时保留空段落占位，并在末行补省略号。
pub fn wrap_words_with_width(
    body: &str,
    max_w: f64,
    max_lines: usize,
    mut width: impl FnMut(char) -> f64,
) -> Vec<String> {
    let mut cache = HashMap::new();
    let mut measure_ch = |ch| *cache.entry(ch).or_insert_with(|| width(ch));

    let max_w = max_w.max(1.0);
    let mut lines: Vec<String> = Vec::new();

    for para in body.split('\n') {
        let tokens = tokenize_body_line(para);
        if tokens.is_empty() {
            lines.push(String::new());
            continue;
        }

        let mut cur_line = String::new();
        let mut cur_w = 0.0;
        let mut pending_space = false;

        for tok in tokens {
            match tok {
                BodyToken::Space => {
                    if !cur_line.is_empty() {
                        pending_space = true;
                    }
                }
                BodyToken::Cjk(ch) => {
                    let cw = measure_ch(ch);
                    let space_w = if pending_space { measure_ch(' ') } else { 0.0 };
                    if cur_w + space_w + cw <= max_w {
                        if pending_space {
                            cur_line.push(' ');
                            cur_w += space_w;
                            pending_space = false;
                        }
                        cur_line.push(ch);
                        cur_w += cw;
                    } else {
                        if !cur_line.is_empty() {
                            lines.push(std::mem::take(&mut cur_line));
                            pending_space = false;
                        }
                        cur_line.push(ch);
                        cur_w = cw;
                    }
                }
                BodyToken::Word(word) => {
                    let word_w: f64 = word.chars().map(&mut measure_ch).sum();
                    let space_w = if pending_space { measure_ch(' ') } else { 0.0 };
                    if cur_w + space_w + word_w <= max_w {
                        if pending_space {
                            cur_line.push(' ');
                            cur_w += space_w;
                            pending_space = false;
                        }
                        cur_line.push_str(word);
                        cur_w += word_w;
                    } else {
                        if !cur_line.is_empty() {
                            lines.push(std::mem::take(&mut cur_line));
                            cur_w = 0.0;
                            pending_space = false;
                        }
                        if word_w <= max_w {
                            cur_line.push_str(word);
                            cur_w = word_w;
                        } else {
                            // 单个单词宽度超出整行：逐字符折行
                            for ch in word.chars() {
                                let cw = measure_ch(ch);
                                if cur_w + cw > max_w && !cur_line.is_empty() {
                                    lines.push(std::mem::take(&mut cur_line));
                                    cur_w = 0.0;
                                }
                                cur_line.push(ch);
                                cur_w += cw;
                            }
                        }
                    }
                }
            }
        }
        if !cur_line.is_empty() {
            lines.push(cur_line);
        }
    }

    if lines.len() > max_lines {
        lines.truncate(max_lines);
        let ellipsis_w = measure_ch('…');
        if let Some(last) = lines.last_mut() {
            let mut last_w: f64 = last.chars().map(&mut measure_ch).sum();
            while !last.is_empty() && last_w + ellipsis_w > max_w {
                if let Some(ch) = last.pop() {
                    last_w -= measure_ch(ch);
                }
            }
            last.push('…');
        }
    }

    lines
}
