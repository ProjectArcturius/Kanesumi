// size_discipline.rs —— N18 静态检查：控件**生产段**不得新增裸尺寸常量。
//
// 存在理由（盘点 `Ether/docs/research/refactor_census/REPORT_2026-11-03.md` N18）：
// 尺寸正典已落在 `kanesumi_core::sizes`（唯一尺寸表），生产段再写 `const X: f32 = 32.0;`
// 这类裸尺寸常量就是在表外另立门户 —— ×1.2 第二步（§159）时必然漏改。颜色有
// `token_discipline.rs`（M1-3）守门，尺寸由本文件守门。
//
// 规则：
//   1. 生产段出现「整值 f32 常量字面量」即违规（`const NAME: f32 = 32.0;`）——
//      整值才是几何尺寸的形态；小数值（0.65、1.05、0.4）是不透明度 / 比例 / 缩放，
//      归颜色与动画纪律管，不在此拦。尺寸的**定义**只能发生在 `kanesumi_core::sizes`，
//      控件保留语义别名引用表值（如 `CHECKBOX_SIZE = sizes::ICON_20`）。
//   2. 历史遗留的旧值进 [`WHITELIST`]（file + 常量名 + 值 三元组）。白名单**只减不增**：
//      每一项都必须仍能对上现存的违规点；迁走一处就必须删掉对应行（stale 项直接红），
//      想新增行 = 新增裸尺寸，评审会拦。
//   3. 行内布局算术（`(w - 16.0) / 2.0` 一类）不拦：那是表达式不是常量，拦了会误伤。
//
// 扫描范围是**生产段**：首个 `#[cfg(test)]` 之前的行（与 token_discipline 同法）；
// 测试里写死尺寸是为了断言，不构成产品缺陷。

use std::fs;
use std::path::{Path, PathBuf};

/// 白名单：（文件名, 常量名, 值）。只减不增 —— 删行需随迁移提交，加行需过评审。
/// 现存 116 项 = 本卡建表时生产段的**存量旧值**（多数带 UWP 资源名 / 页面自定语义，
/// 不在本卡七族令牌范围内）；后续迁移一处删一行。
const WHITELIST: &[(&str, &str, &str)] = &[
    ("auto_suggest_box.rs", "AUTOSUGGEST_LIST_MAX_H", "300.0"),
    ("auto_suggest_box.rs", "AUTOSUGGEST_ITEM_H", "40.0"),
    ("auto_suggest_box.rs", "AUTOSUGGEST_ITEM_PAD", "12.0"),
    ("auto_suggest_box.rs", "AUTOSUGGEST_BORDER", "1.0"),
    ("breadcrumb_bar.rs", "ITEM_PAD_X", "1.0"),
    ("breadcrumb_bar.rs", "ITEM_PAD_Y", "3.0"),
    ("breadcrumb_bar.rs", "CHEVRON_GAP", "16.0"),
    ("breadcrumb_bar.rs", "ELLIPSIS_PAD", "6.0"),
    ("button.rs", "PAD_X", "8.0"),
    ("calendar_date_picker.rs", "PICKER_MIN_H", "32.0"),
    ("calendar_date_picker.rs", "PICKER_BORDER", "2.0"),
    ("calendar_date_picker.rs", "PICKER_GLYPH_W", "32.0"),
    ("calendar_view.rs", "DAY_SIZE", "40.0"),
    ("calendar_view.rs", "DAY_MARGIN", "1.0"),
    ("calendar_view.rs", "WEEKDAY_H", "38.0"),
    ("calendar_view.rs", "HEADER_H", "40.0"),
    ("candidate_window.rs", "CANDIDATE_ROW_H", "44.0"),
    ("candidate_window.rs", "CANDIDATE_PAD_X", "12.0"),
    ("candidate_window.rs", "CANDIDATE_PAD_Y", "6.0"),
    ("candidate_window.rs", "CANDIDATE_LABEL_GAP", "2.0"),
    ("candidate_window.rs", "CANDIDATE_HL_PAD", "16.0"),
    ("candidate_window.rs", "CANDIDATE_MAX_W", "640.0"),
    ("check_box.rs", "CHECKBOX_STROKE", "1.0"),
    ("check_box.rs", "CHECKBOX_BOX_GAP", "8.0"),
    ("check_box.rs", "CHECKBOX_MIN_HEIGHT", "32.0"),
    ("check_box.rs", "CHECKBOX_MIN_WIDTH", "120.0"),
    ("color_picker.rs", "COLOR_THUMB", "10.0"),
    ("color_picker.rs", "COLOR_SLIDER_H", "24.0"),
    ("color_picker.rs", "COLOR_PREVIEW_H", "44.0"),
    ("color_picker.rs", "COLOR_HEX_ROW_H", "24.0"),
    ("command_bar_flyout.rs", "COMMANDBAR_BUTTON_SIZE", "40.0"),
    ("command_bar_flyout.rs", "COMMANDBAR_ICON_SIZE", "16.0"),
    ("command_bar_flyout.rs", "COMMANDBAR_BORDER", "1.0"),
    ("dialog.rs", "DIALOG_BOX_H", "240.0"),
    ("drop_down_button.rs", "CHEVRON_SPACE", "18.0"),
    ("dropdown_menu.rs", "MENU_GROUP_GAP", "8.0"),
    ("expander.rs", "EXPANDER_HEADER_HEIGHT", "48.0"),
    ("expander.rs", "EXPANDER_HEADER_PAD_X", "16.0"),
    ("expander.rs", "EXPANDER_CHEVRON_MARGIN_RIGHT", "8.0"),
    ("menu_bar.rs", "HEADER_HEIGHT", "40.0"),
    ("menu_bar.rs", "HEADER_PAD_X", "10.0"),
    ("metro_tile.rs", "TILE_PAD", "8.0"),
    ("metro_tile.rs", "TREE_TILE_CELL", "64.0"),
    ("metro_tile.rs", "TREE_TILE_GAP", "8.0"),
    ("navigation_view.rs", "NAV_PANE_EXPANDED", "320.0"),
    ("navigation_view.rs", "NAV_PANE_COMPACT", "48.0"),
    ("navigation_view.rs", "NAV_TOP_HEIGHT", "48.0"),
    ("navigation_view.rs", "NAV_TOGGLE", "40.0"),
    ("navigation_view.rs", "NAV_ITEM_H", "40.0"),
    ("navigation_view.rs", "NAV_TOP_ITEM_PAD_LEFT", "16.0"),
    ("navigation_view.rs", "NAV_TOP_ITEM_PAD_RIGHT", "12.0"),
    ("navigation_view.rs", "NAV_TOP_ITEM_MAX_W", "240.0"),
    ("navigation_view.rs", "NAV_TOP_ICON_SLOT", "28.0"),
    ("navigation_view.rs", "NAV_TOP_NO_ICON_PAD", "16.0"),
    ("number_box.rs", "NUMBERBOX_SPIN_COLUMN_W", "72.0"),
    ("number_box.rs", "NUMBERBOX_SPIN_BUTTON_W", "32.0"),
    ("number_box.rs", "NUMBERBOX_MIN_WIDTH", "120.0"),
    ("number_box.rs", "NUMBERBOX_SEPARATOR", "1.0"),
    ("pager_control.rs", "NAV_SIZE", "40.0"),
    ("pager_control.rs", "PAGE_BUTTON_MIN_W", "32.0"),
    ("pager_control.rs", "PAGE_BUTTON_MIN_H", "20.0"),
    ("pager_control.rs", "PAGE_SPACING", "5.0"),
    ("pager_control.rs", "INDICATOR_H", "2.0"),
    ("pips_pager.rs", "PIP_HIT_W", "12.0"),
    ("pips_pager.rs", "PIP_HIT_H", "20.0"),
    ("pips_pager.rs", "PIP_NORMAL_BAR", "4.0"),
    ("pips_pager.rs", "PIP_SELECTED_BAR", "6.0"),
    ("pips_pager.rs", "NAV_SIZE", "20.0"),
    ("popup.rs", "POPUP_GAP", "4.0"),
    ("radio_buttons.rs", "RADIO_DOT", "10.0"),
    ("radio_buttons.rs", "RADIO_LABEL_GAP", "6.0"),
    ("radio_buttons.rs", "RADIO_COL_SPACING", "7.0"),
    ("radio_buttons.rs", "RADIO_ROW_SPACING", "8.0"),
    ("rating_control.rs", "RATING_ITEM_SPACING", "4.0"),
    ("scroll_view.rs", "SCROLLBAR_THICKNESS", "8.0"),
    ("scroll_view.rs", "SCROLLBAR_MIN_THUMB", "24.0"),
    ("slider.rs", "SLIDER_TRACK_H", "2.0"),
    ("slider.rs", "SLIDER_THUMB_W", "8.0"),
    ("slider.rs", "SLIDER_THUMB_H", "24.0"),
    ("slider.rs", "SLIDER_MIN_H", "32.0"),
    ("slider.rs", "SLIDER_HEADER_MARGIN", "4.0"),
    ("slider.rs", "SLIDER_MIN_W", "120.0"),
    ("slider.rs", "SLIDER_HEADER_SIZE", "12.0"),
    ("split_button.rs", "SECONDARY_WIDTH", "35.0"),
    ("split_button.rs", "SEPARATOR_WIDTH", "1.0"),
    ("swipe_control.rs", "SWIPE_ITEM_W", "64.0"),
    ("swipe_control.rs", "SWIPE_TAP_SLOP", "4.0"),
    ("tab_row.rs", "OVERFLOW_BTN_W", "40.0"),
    ("tab_view.rs", "TABVIEW_HEADER_PAD", "8.0"),
    ("tab_view.rs", "TABVIEW_ITEM_MIN_H", "32.0"),
    ("tab_view.rs", "TABVIEW_ITEM_MIN_W", "100.0"),
    ("tab_view.rs", "TABVIEW_ITEM_MAX_W", "240.0"),
    ("tab_view.rs", "TABVIEW_ITEM_FONT", "12.0"),
    ("tab_view.rs", "TABVIEW_CLOSE_W", "32.0"),
    ("tab_view.rs", "TABVIEW_CLOSE_H", "24.0"),
    ("tab_view.rs", "TABVIEW_ADD_W", "32.0"),
    ("teaching_tip.rs", "TEACHING_MIN_W", "320.0"),
    ("teaching_tip.rs", "TEACHING_MAX_W", "336.0"),
    ("teaching_tip.rs", "TEACHING_CONTENT_MARGIN", "12.0"),
    ("teaching_tip.rs", "TEACHING_CLOSE_SIZE", "40.0"),
    ("teaching_tip.rs", "S", "8.0"),
    ("teaching_tip.rs", "TIP_GAP", "8.0"),
    ("text_box.rs", "TEXTBOX_DELETE_BUTTON_W", "34.0"),
    ("text_box.rs", "TEXT_INPUT_PAD_L", "10.0"),
    ("text_box.rs", "TEXT_INPUT_PAD_T", "6.0"),
    ("text_box.rs", "TEXT_INPUT_PAD_R", "6.0"),
    ("text_box.rs", "TEXT_INPUT_PAD_B", "5.0"),
    ("text_box.rs", "DASH", "4.0"),
    ("text_box.rs", "GAP", "3.0"),
    ("title_bar.rs", "TITLEBAR_COMPACT", "32.0"),
    ("title_bar.rs", "TITLEBAR_EXPANDED", "48.0"),
    ("title_bar.rs", "TITLEBAR_BACK_WIDTH", "44.0"),
    ("tree_view.rs", "TREE_ITEM_H", "28.0"),
    ("tree_view.rs", "TREE_INDENT", "16.0"),
    ("two_pane_view.rs", "DEFAULT_MIN_WIDE", "641.0"),
    ("two_pane_view.rs", "DEFAULT_MIN_TALL", "641.0"),
];

/// 生产段 = 文件里首个 `#[cfg(test)]` 之前的行（行号从 1 起，对应编辑器）。
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

/// 去掉行内 `//` 注释（字符串字面量感知，`"http://…"` 不误切）。
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

/// 单个违规点：文件内位置 + 常量名 + 值。
pub struct BareSize {
    pub file: String,
    pub line: usize,
    pub name: String,
    pub value: String,
}

/// 扫一段生产代码里的「整值 f32 常量字面量」。
///
/// 只认 `const NAME: f32 = <整值字面量>;`（含 `pub const`、函数内局部 const、行中
/// 挤在 `{ ... }` 里的 const）；`= sizes::ICON_16` 这类令牌引用不是字面量，天然放行。
pub fn scan_bare_sizes(file: &str, code: &str) -> Vec<BareSize> {
    let mut out = Vec::new();
    for (line_no, line) in production_lines(code) {
        let code_part = strip_comment(line);
        for (name, value) in extract_const_f32(code_part) {
            // 整值才是几何尺寸的形态；小数是不透明度 / 比例 / 缩放因子；
            // 0.0 是「无间距」语义（如 CANDIDATE_ITEM_GAP），无档可入，不拦。
            let v = value.trim();
            let is_integer_valued = v != "0.0"
                && v != "-0.0"
                && v.split_once('.')
                .map(|(_, frac)| frac.bytes().all(|b| b == b'0'))
                .unwrap_or(false);
            if is_integer_valued {
                out.push(BareSize {
                    file: file.to_string(),
                    line: line_no,
                    name: name.to_string(),
                    value: v.to_string(),
                });
            }
        }
    }
    out
}

/// 抠出一行里**所有** `const NAME: f32 = VALUE;` 形态的 (NAME, VALUE)。
/// 行首（`pub const` / 局部 const 独占一行）与行中（`fn f() { const X: f32 = 1.0; }`）都算；
/// 不是字面量初始化（令牌别名 / 表达式）的返回 None 项自然跳过。
fn extract_const_f32(code: &str) -> Vec<(&str, &str)> {
    let bytes = code.as_bytes();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = code[from..].find("const") {
        let start = from + pos;
        // 词边界：`const` 前不能是标识符字符（排除 `pub_const` 这类不存在的形态，
        // 以及 `#const`/`_const` 后缀误配）。
        let boundary_ok = start == 0
            || !(bytes[start - 1].is_ascii_alphanumeric() || bytes[start - 1] == b'_');
        let mut hit = None;
        if boundary_ok {
            let t = code[start + 5..].trim_start();
            let name_end = t
                .find(|c: char| !(c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
                .unwrap_or(t.len());
            let name = &t[..name_end];
            let rest = t[name_end..].trim_start();
            let parsed = rest
                .strip_prefix(":")
                .and_then(|r| r.trim_start().strip_prefix("f32"))
                .and_then(|r| r.trim_start().strip_prefix('='))
                // 值到本行第一个 `;` 为止：剥注释会留尾部空白，行中形态（`fn f() { …; }`）
                // 分号后还有 `}` —— 都不该让匹配失败。
                .and_then(|r| {
                    r.find(';')
                        .map(|end| r[..end].trim())
                        .filter(|v| !v.is_empty())
                });
            if let Some(value) = parsed.filter(|v| {
                !name.is_empty()
                    // 字面量判定：数字开头（负号也算），拦住 `sizes::X` / 表达式。
                    && (v.starts_with(|c: char| c.is_ascii_digit()) || v.starts_with('-'))
            }) {
                hit = Some((name, value));
            }
        }
        // 从本个 `const` 之后继续找（同一行可能挤多个声明，罕见但不吃亏）。
        from = start + 5;
        if let Some((name, value)) = hit {
            out.push((name, value));
        }
    }
    out
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
fn no_new_bare_size_consts_in_production_code() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    files.sort();
    // 扫描文件数 > 0：空集合会让检查「静默通过」—— 与要防的病同源，先行断言。
    assert!(
        !files.is_empty(),
        "扫描到的源文件为 0，检查形同虚设（路径：{}）",
        src.display()
    );

    let mut hits = Vec::new();
    for f in &files {
        let Ok(text) = fs::read_to_string(f) else {
            continue;
        };
        hits.extend(scan_bare_sizes(&f.file_name().unwrap().to_string_lossy(), &text));
    }

    let mut problems = Vec::new();
    let mut matched = vec![false; WHITELIST.len()];
    for hit in &hits {
        let idx = WHITELIST
            .iter()
            .position(|(file, name, value)| *file == hit.file && *name == hit.name && *value == hit.value);
        match idx {
            Some(i) => matched[i] = true,
            None => problems.push(format!(
                "  {}:{}  const {}: f32 = {}（应收进 kanesumi_core::sizes 或入白名单）",
                hit.file, hit.line, hit.name, hit.value
            )),
        }
    }
    // 白名单只减不增：残留条目 = 违规已消失但白名单没删 —— 责令删行。
    for (i, ok) in matched.iter().enumerate() {
        if !ok {
            let (file, name, value) = WHITELIST[i];
            problems.push(format!(
                "  白名单残留项 (\"{file}\", \"{name}\", \"{value}\") 已无对应违规：删掉该行（白名单只减不增）"
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "生产段裸尺寸常量检查未过（{} 处，白名单 {} 项）：\n{}",
        problems.len(),
        WHITELIST.len(),
        problems.join("\n")
    );
}

/// 反向自检：检测器自己必须真的能抓到靶子、放过令牌引用与小数档。
/// （没有这条，「扫描通过」可能是「扫描器坏了」——M1-3 同款教训。）
#[test]
fn scanner_self_check() {
    // 抓到：整值 f32 裸常量（含 pub / 局部 / 负零形态）。
    let guilty = "\
const BAD_SIZE: f32 = 42.0;
pub const BAD_PUB: f32 = 8.0;
    fn f() { const LOCAL: f32 = 12.0; }
";
    let hits = scan_bare_sizes("self.rs", guilty);
    let names: Vec<&str> = hits.iter().map(|h| h.name.as_str()).collect();
    assert_eq!(names, ["BAD_SIZE", "BAD_PUB", "LOCAL"], "检测器漏抓：{names:?}");

    // 放过：令牌引用别名（迁移的正确形态）。
    let clean_alias = "pub const TREE_CHEVRON: f32 = kanesumi_core::sizes::ICON_16;\n";
    assert!(scan_bare_sizes("self.rs", clean_alias).is_empty(), "别名不应被抓");

    // 放过：小数（不透明度 / 比例，归 token_discipline / 动画纪律管）。
    let fractional = "const ALPHA: f32 = 0.65;\nconst RATIO: f32 = 1.05;\nconst HALF: f32 = 0.0;\n";
    assert!(scan_bare_sizes("self.rs", fractional).is_empty(), "小数不应被抓");

    // 放过：f64 时长（时长归动画规格）与表达式初始化。
    let other = "const MORPH: f64 = 0.1;\nconst DERIVED: f32 = OTHER + 2.0;\n";
    assert!(scan_bare_sizes("self.rs", other).is_empty(), "f64/表达式不应被抓");

    // 生产段边界：#[cfg(test)] 之后不扫。
    let with_tests = "const BAD: f32 = 42.0;\n#[cfg(test)]\nmod tests { const FINE: f32 = 40.0; }\n";
    let hits = scan_bare_sizes("self.rs", with_tests);
    assert_eq!(hits.len(), 1, "cfg(test) 后不应扫：{:?}", hits.iter().map(|h| &h.name).collect::<Vec<_>>());
}
