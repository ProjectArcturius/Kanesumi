// TextField —— 文本编辑核心（纯逻辑，跨平台可测）。
//
// TextBox/PasswordBox/NumberBox/AutoSuggestBox 共用的字符级编辑状态机：
// 内容、光标、选区、撤销栈、掩码（PasswordBox）。渲染与输入路由在
// MetroTextBox（text_box.rs）等控件层完成，本层不依赖字体/Scene。
//
// 下标约定：光标/选区均为 **字符下标**（CJK 安全）。`cursor ∈ [0, len]`；
// 选区 = `(anchor, cursor)` 两角点，规范区间 = `min..max`（空选区 = None）。

/// 文本编辑键 —— 控件层的跨平台键契约（harness `Key` 的超集子集）。
/// 由宿主从 harness `InputEvent::KeyPressed` 转换后喂给 `TextField::handle_key`。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextInputKey {
    /// 可打印字符。
    Char(char),
    Enter,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Escape,
    Tab,
}

/// 快照（撤销用）。
#[derive(Debug, Clone, PartialEq)]
struct Snapshot {
    text: Vec<char>,
    cursor: usize,
    anchor: Option<usize>,
}

/// 文本编辑核心。`text` 以 `Vec<char>` 存储（CJK 安全、O(1) 随机编辑）。
#[derive(Debug, Clone, PartialEq)]
pub struct TextField {
    text: Vec<char>,
    /// 光标（字符下标）。
    cursor: usize,
    /// 选区锚点（None = 无选区；Some = 与 `cursor` 构成选区）。
    anchor: Option<usize>,
    /// 掩码字符（PasswordBox 显示用；None = 明文）。
    mask: Option<char>,
    /// 撤销栈（快照为编辑前状态）。
    undo: Vec<Snapshot>,
    /// 撤销栈上限（防无限增长）。
    max_undo: usize,
    /// IME 预编辑（组合态）文本。不入 `text` / 选区 / 撤销栈 —— 是覆盖在
    /// 光标处的瞬态显示流（参 IME_WIRING_PLAN 阶段 A）。空串 = 无组合态。
    preedit: String,
    /// 预编辑光标（字符下标，相对 `preedit` 起点；None = 光标在组合态尾）。
    preedit_cursor: Option<usize>,
}

impl Default for TextField {
    fn default() -> Self {
        Self {
            text: Vec::new(),
            cursor: 0,
            anchor: None,
            mask: None,
            undo: Vec::new(),
            max_undo: 64,
            preedit: String::new(),
            preedit_cursor: None,
        }
    }
}

impl TextField {
    pub fn new() -> Self {
        Self::default()
    }

    /// 带初始文本构造（光标置尾）。
    pub fn with_text(text: impl Into<String>) -> Self {
        let chars: Vec<char> = text.into().chars().collect();
        let len = chars.len();
        Self {
            text: chars,
            cursor: len,
            ..Self::default()
        }
    }

    // ── 只读查询 ──────────────────────────────────────────────

    /// 内容字符数。
    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// 明文内容。
    pub fn text(&self) -> String {
        self.text.iter().collect()
    }

    /// 显示内容（掩码时以掩码字符替代，PasswordBox 用）。
    pub fn display_text(&self) -> String {
        self.display_chars().iter().collect()
    }

    /// 显示字符序列（掩码或明文）。
    pub fn display_chars(&self) -> Vec<char> {
        match self.mask {
            Some(m) => self.text.iter().map(|_| m).collect(),
            None => self.text.clone(),
        }
    }

    /// 光标位置（字符下标）。
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// 选区锚点。
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// 规范选区区间 `(start, end)`（无选区 → None）。
    pub fn selection(&self) -> Option<(usize, usize)> {
        let a = self.anchor?;
        let (lo, hi) = if a <= self.cursor {
            (a, self.cursor)
        } else {
            (self.cursor, a)
        };
        if lo == hi {
            None
        } else {
            Some((lo, hi))
        }
    }

    /// 掩码字符（None = 明文）。
    pub fn mask(&self) -> Option<char> {
        self.mask
    }

    /// 设置掩码（PasswordBox）。
    pub fn set_mask(&mut self, c: Option<char>) {
        self.mask = c;
    }

    // ── IME 组合态 ──────────────────────────────────────────────

    /// 预编辑文本（原始字符，非掩码）。
    pub fn preedit(&self) -> &str {
        &self.preedit
    }

    /// 预编辑光标（字符下标，相对 `preedit` 起点；None = 组合态尾）。
    pub fn preedit_cursor(&self) -> Option<usize> {
        self.preedit_cursor
    }

    pub fn has_preedit(&self) -> bool {
        !self.preedit.is_empty()
    }

    /// 设置预编辑文本。`cursor_byte` 为协议字节下标（UTF-8，允许落在字符中间，
    /// 外扩夹紧到该字符起点）；`None` = 光标在组合态尾。空串清除组合态。
    /// 预编辑不入 undo —— 撤销只回滚已提交文本。
    pub fn set_preedit(&mut self, text: &str, cursor_byte: Option<usize>) {
        self.preedit = text.to_string();
        self.preedit_cursor = cursor_byte.map(|b| {
            let b = b.min(text.len());
            let b = snap_byte_to_boundary(text, b);
            text[..b].chars().count()
        });
    }

    /// 清除组合态。
    pub fn clear_preedit(&mut self) {
        self.preedit.clear();
        self.preedit_cursor = None;
    }

    /// 组合态显示文本（掩码时逐字符掩码，PasswordBox 语义）。
    pub fn preedit_display(&self) -> String {
        match self.mask {
            Some(m) => self.preedit.chars().map(|_| m).collect(),
            None => self.preedit.clone(),
        }
    }

    /// 原子提交 IME 文本：快照 → 删选区 → 插入 → 清组合态。
    /// 返回 true = 内容变化（含「空提交取消组合态」）。
    pub fn commit_ime(&mut self, s: &str) -> bool {
        if s.is_empty() {
            // 空提交：仅取消组合态（IME 提交空串 = 清 preedit）。
            let had = self.has_preedit();
            self.clear_preedit();
            return had;
        }
        self.snapshot();
        self.delete_selection();
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        let at = self.cursor;
        self.text.splice(at..at, chars);
        self.cursor += n;
        self.clear_preedit();
        true
    }

    /// IME 周边删除：删光标前 `before_bytes` 字节、后 `after_bytes` 字节
    /// （字节 → 字符转换，UTF-8 边界外扩夹紧 —— 绝不劈开码点）。
    /// 删除基于文本光标（组合态已按协议在光标处折叠，见 done 序列）。
    pub fn delete_surrounding(&mut self, before_bytes: u32, after_bytes: u32) -> bool {
        if before_bytes == 0 && after_bytes == 0 {
            return false;
        }
        self.snapshot();
        let before_chars = if before_bytes > 0 {
            count_back_bytes(&self.text[..self.cursor], before_bytes as usize)
        } else {
            0
        };
        let after_chars = if after_bytes > 0 {
            count_fwd_bytes(&self.text[self.cursor..], after_bytes as usize)
        } else {
            0
        };
        if before_chars == 0 && after_chars == 0 {
            return false;
        }
        let lo = self.cursor - before_chars;
        self.text.drain(lo..(lo + before_chars + after_chars));
        self.cursor -= before_chars;
        self.anchor = None;
        // 周边文本已变，旧组合态不再可信（协议 done 序列会重灌新 preedit）。
        self.clear_preedit();
        true
    }

    /// 周边上下文（灌 `set_surrounding_text` 用）。每侧 cap `max_bytes` 字节，
    /// UTF-8 边界外扩（不劈码点）。返回 `(before, after, cursor_byte, anchor_byte)`，
    /// `before + after` 拼回协议 text，`cursor_byte` = `before` 字节长，
    /// `anchor_byte` = 锚点在拼回串中的字节偏移（无选区 = cursor_byte）。
    pub fn surrounding_text(&self, max_bytes: usize) -> (String, String, usize, usize) {
        let before_chars = &self.text[..self.cursor];
        let after_chars = &self.text[self.cursor..];
        let before = take_bytes_clamped(before_chars, max_bytes);
        let after = take_bytes_clamped(after_chars, max_bytes);
        let cursor_byte = before.len();
        let anchor_byte = match self.anchor {
            Some(a) if a < self.cursor => {
                let off = chars_bytes(before_chars)
                    .get(a.min(before_chars.len()))
                    .copied()
                    .unwrap_or(0);
                off.min(before.len())
            }
            Some(a) if a > self.cursor => {
                let rel = a - self.cursor;
                let off = chars_bytes(after_chars)
                    .get(rel.min(after_chars.len()))
                    .copied()
                    .unwrap_or(0);
                (before.len() + off).min(before.len() + after.len())
            }
            _ => cursor_byte,
        };
        (before, after, cursor_byte, anchor_byte)
    }

    /// IME 组合态光标（字符下标，相对 `preedit` 起点）。协议 cursor_begin 为 -1
    /// （隐藏光标）时返回 None（= 组合态尾）。
    pub(crate) fn preedit_caret_char(&self) -> usize {
        self.preedit_cursor
            .unwrap_or_else(|| self.preedit.chars().count())
            .min(self.preedit.chars().count())
    }

    // ── 编辑 ──────────────────────────────────────────────────

    /// 提交编辑前快照（撤销栈）。
    fn snapshot(&mut self) {
        if self.undo.len() >= self.max_undo {
            self.undo.remove(0);
        }
        self.undo.push(Snapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        });
    }

    /// 撤销。返回 true = 有可撤销的编辑。
    pub fn undo(&mut self) -> bool {
        let Some(snap) = self.undo.pop() else {
            return false;
        };
        self.text = snap.text;
        self.cursor = snap.cursor.min(self.text.len());
        self.anchor = snap.anchor;
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// 删除当前选区，光标移到区间起点。返回被删字符数。
    fn delete_selection(&mut self) -> usize {
        let Some((lo, hi)) = self.selection() else {
            return 0;
        };
        self.text.drain(lo..hi);
        self.cursor = lo;
        self.anchor = None;
        hi - lo
    }

    /// 在光标处插入字符（覆盖选区）。返回 true = 内容变化。
    pub fn insert_char(&mut self, c: char) -> bool {
        if c == '\r' {
            return false;
        }
        // 直接键插入会打断 IME 组合态（键盘直入 = 放弃组合）。
        self.clear_preedit();
        self.snapshot();
        self.delete_selection();
        self.text.insert(self.cursor, c);
        self.cursor += 1;
        true
    }

    /// 插入字符串（粘贴用）。
    pub fn insert_str(&mut self, s: &str) -> bool {
        if s.is_empty() {
            return false;
        }
        self.clear_preedit();
        self.snapshot();
        self.delete_selection();
        let chars: Vec<char> = s.chars().collect();
        let n = chars.len();
        let at = self.cursor;
        self.text.splice(at..at, chars);
        self.cursor += n;
        true
    }

    /// Backspace：删选区，否则删光标前一字符。
    pub fn backspace(&mut self) -> bool {
        self.clear_preedit();
        self.snapshot();
        if self.delete_selection() > 0 {
            return true;
        }
        if self.cursor == 0 {
            return false;
        }
        self.cursor -= 1;
        self.text.remove(self.cursor);
        true
    }

    /// Delete：删选区，否则删光标处字符。
    pub fn delete(&mut self) -> bool {
        self.clear_preedit();
        self.snapshot();
        if self.delete_selection() > 0 {
            return true;
        }
        if self.cursor >= self.text.len() {
            return false;
        }
        self.text.remove(self.cursor);
        true
    }

    /// 整体重置内容（撤销栈清空）。
    pub fn set_text(&mut self, text: impl Into<String>) {
        let chars: Vec<char> = text.into().chars().collect();
        self.text = chars;
        self.cursor = self.text.len();
        self.anchor = None;
        self.undo.clear();
    }

    // ── 光标 / 选区移动 ────────────────────────────────────────

    /// 置光标（夹紧到 [0, len]），清除选区。
    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor = pos.min(self.text.len());
        self.anchor = None;
    }

    fn move_cursor(&mut self, delta: isize, select: bool) {
        let new = (self.cursor as isize + delta).clamp(0, self.text.len() as isize) as usize;
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
            self.cursor = new;
        } else {
            self.cursor = new;
            self.anchor = None;
        }
    }

    pub fn move_left(&mut self, select: bool) {
        self.move_cursor(-1, select);
    }

    pub fn move_right(&mut self, select: bool) {
        self.move_cursor(1, select);
    }

    pub fn move_home(&mut self, select: bool) {
        let new = 0;
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
            self.cursor = new;
        } else {
            self.cursor = new;
            self.anchor = None;
        }
    }

    pub fn move_end(&mut self, select: bool) {
        let new = self.text.len();
        if select {
            if self.anchor.is_none() {
                self.anchor = Some(self.cursor);
            }
            self.cursor = new;
        } else {
            self.cursor = new;
            self.anchor = None;
        }
    }

    /// 全选。
    pub fn select_all(&mut self) {
        if self.text.is_empty() {
            self.anchor = None;
            self.cursor = 0;
            return;
        }
        self.anchor = Some(0);
        self.cursor = self.text.len();
    }

    /// 光标 → 位置（点击定位，以字符下标）。清除选区。
    pub fn place_caret(&mut self, pos: usize) {
        self.set_cursor(pos);
    }

    /// 处理一个编辑键。返回 true = 内容/光标发生变化（宿主可据此重绘）。
    pub fn handle_key(&mut self, key: TextInputKey) -> bool {
        match key {
            TextInputKey::Char(c) => self.insert_char(c),
            TextInputKey::Enter => {
                // TextBox 单行模式回车默认不插入（宿主按提交语义处理）。
                false
            }
            TextInputKey::Backspace => self.backspace(),
            TextInputKey::Delete => self.delete(),
            TextInputKey::Left => {
                self.move_left(false);
                true
            }
            TextInputKey::Right => {
                self.move_right(false);
                true
            }
            TextInputKey::Up | TextInputKey::Down => {
                // 单行编辑：上下无光标语义（由宿主/建议弹层消费）。
                false
            }
            TextInputKey::Home => {
                self.move_home(false);
                true
            }
            TextInputKey::End => {
                self.move_end(false);
                true
            }
            TextInputKey::Escape => {
                // 取消选区（光标保留）+ 清 IME 组合态（Escape 取消组合）。
                self.anchor = None;
                self.clear_preedit();
                true
            }
            TextInputKey::Tab => false,
        }
    }
}

// ── UTF-8 字节 ↔ 字符 辅助 ────────────────────────────────────────────────
// IME 协议下标全为字节（zwp_text_input_v3），控件内部为字符下标。转换一律
// 走码点边界（外扩夹紧），绝不劈开多字节字符（参 IME_WIRING_PLAN 风险 3）。

/// 把字节下标外扩夹紧到 UTF-8 码点边界（落在字符中间 → 取该字符起点）。
fn snap_byte_to_boundary(s: &str, byte: usize) -> usize {
    let byte = byte.min(s.len());
    let mut b = byte;
    while b > 0 && !s.is_char_boundary(b) {
        b -= 1;
    }
    b
}

/// 字符序列 → 每字符起点字节偏移（前缀表，`v[i]` = 第 i 字符起点字节）。
fn chars_bytes(chars: &[char]) -> Vec<usize> {
    let mut v = Vec::with_capacity(chars.len() + 1);
    let mut acc = 0usize;
    v.push(0);
    for c in chars {
        acc += c.len_utf8();
        v.push(acc);
    }
    v
}

/// 从光标往前数：能装进 `max_bytes` 字节的字符个数（不劈码点，贪心收紧）。
fn count_back_bytes(before: &[char], max_bytes: usize) -> usize {
    let mut bytes = 0usize;
    let mut count = 0usize;
    for c in before.iter().rev() {
        if bytes + c.len_utf8() > max_bytes {
            break;
        }
        bytes += c.len_utf8();
        count += 1;
    }
    count
}

/// 从光标往后数：能装进 `max_bytes` 字节的字符个数。
fn count_fwd_bytes(after: &[char], max_bytes: usize) -> usize {
    let mut bytes = 0usize;
    let mut count = 0usize;
    for c in after {
        if bytes + c.len_utf8() > max_bytes {
            break;
        }
        bytes += c.len_utf8();
        count += 1;
    }
    count
}

/// 取字符前缀直到字节数 ≤ `max_bytes`（UTF-8 边界外扩，不劈码点）。
fn take_bytes_clamped(chars: &[char], max_bytes: usize) -> String {
    let mut s = String::new();
    let mut bytes = 0usize;
    for c in chars {
        let b = c.len_utf8();
        if bytes + b > max_bytes {
            break;
        }
        bytes += b;
        s.push(*c);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_character_at_cursor() {
        let mut f = TextField::new();
        f.insert_char('a');
        f.insert_char('b');
        f.insert_char('中');
        assert_eq!(f.text(), "ab中");
        assert_eq!(f.cursor(), 3);
    }

    #[test]
    fn insert_in_middle_shifts_rest() {
        let mut f = TextField::with_text("ac");
        f.set_cursor(1);
        f.insert_char('b');
        assert_eq!(f.text(), "abc");
        assert_eq!(f.cursor(), 2);
    }

    #[test]
    fn backspace_removes_preceding_char() {
        let mut f = TextField::with_text("abc");
        f.move_end(false);
        f.backspace();
        assert_eq!(f.text(), "ab");
        assert_eq!(f.cursor(), 2);
    }

    #[test]
    fn backspace_at_start_noop() {
        let mut f = TextField::with_text("abc");
        f.set_cursor(0);
        assert!(!f.backspace());
        assert_eq!(f.text(), "abc");
    }

    #[test]
    fn delete_removes_at_cursor() {
        let mut f = TextField::with_text("abc");
        f.set_cursor(1);
        f.delete();
        assert_eq!(f.text(), "ac");
        assert_eq!(f.cursor(), 1);
    }

    #[test]
    fn selection_overwritten_by_typing() {
        let mut f = TextField::with_text("hello world");
        f.select_all();
        assert_eq!(f.selection(), Some((0, 11)));
        f.insert_char('X');
        assert_eq!(f.text(), "X");
        assert_eq!(f.selection(), None);
    }

    #[test]
    fn backspace_deletes_selection() {
        let mut f = TextField::with_text("hello world");
        f.set_cursor(5);
        f.move_right(true); // 选中 " "
        f.move_right(true); // 选中 " w"
        f.backspace();
        assert_eq!(f.text(), "helloorld");
        assert_eq!(f.cursor(), 5);
    }

    #[test]
    fn shift_arrow_expands_selection() {
        let mut f = TextField::with_text("abcd");
        f.set_cursor(1);
        f.move_right(true);
        f.move_right(true);
        assert_eq!(f.selection(), Some((1, 3)));
        f.move_right(false); // 取消选区，光标右移（从 3 到 4）
        assert_eq!(f.selection(), None);
        assert_eq!(f.cursor(), 4);
    }

    #[test]
    fn home_end_navigate() {
        let mut f = TextField::with_text("abcd");
        f.move_home(false);
        assert_eq!(f.cursor(), 0);
        f.move_end(false);
        assert_eq!(f.cursor(), 4);
    }

    #[test]
    fn undo_restores_previous_state() {
        let mut f = TextField::new();
        f.insert_str("abc");
        f.backspace();
        assert_eq!(f.text(), "ab");
        assert!(f.undo());
        assert_eq!(f.text(), "abc");
        assert!(f.undo());
        assert_eq!(f.text(), "");
        assert!(!f.undo(), "栈空后无可撤销");
    }

    #[test]
    fn undo_restores_cursor() {
        let mut f = TextField::with_text("abcdef");
        f.set_cursor(2);
        f.insert_str("XY");
        assert_eq!(f.text(), "abXYcdef");
        f.undo();
        assert_eq!(f.text(), "abcdef");
        assert_eq!(f.cursor(), 2);
    }

    #[test]
    fn mask_hides_content() {
        let mut f = TextField::with_text("secret");
        assert_eq!(f.text(), "secret");
        assert_eq!(f.display_text(), "secret");
        f.set_mask(Some('●'));
        assert_eq!(f.display_text(), "●●●●●●");
        assert_eq!(f.text(), "secret", "明文仍保留（PasswordBox 语义）");
    }

    #[test]
    fn cursor_clamped_to_len() {
        let mut f = TextField::with_text("ab");
        f.set_cursor(99);
        assert_eq!(f.cursor(), 2);
        f.move_left(false);
        assert_eq!(f.cursor(), 1);
    }

    #[test]
    fn handle_key_roundtrip() {
        let mut f = TextField::new();
        assert!(f.handle_key(TextInputKey::Char('a')));
        assert!(f.handle_key(TextInputKey::Char('b')));
        assert!(f.handle_key(TextInputKey::Backspace));
        assert_eq!(f.text(), "a");
        assert!(f.handle_key(TextInputKey::Home));
        assert_eq!(f.cursor(), 0);
        assert!(f.handle_key(TextInputKey::Right));
        assert_eq!(f.cursor(), 1);
    }

    #[test]
    fn cjk_survives_editing() {
        // CJK 编辑不破坏字符（下标按 char，非字节）
        let mut f = TextField::with_text("你好世界");
        f.set_cursor(2);
        f.insert_char('的');
        assert_eq!(f.text(), "你好的世界");
        f.backspace();
        assert_eq!(f.text(), "你好世界");
    }

    #[test]
    fn set_text_resets_undo() {
        let mut f = TextField::new();
        f.insert_str("abc");
        f.set_text("xyz");
        assert!(!f.undo(), "set_text 清空撤销栈");
        assert_eq!(f.cursor(), 3);
    }

    #[test]
    fn place_caret_clears_selection() {
        let mut f = TextField::with_text("abcd");
        f.select_all();
        f.place_caret(2);
        assert_eq!(f.selection(), None);
        assert_eq!(f.cursor(), 2);
    }

    // ── IME 组合态（阶段 A，参 IME_WIRING_PLAN） ───────────────

    #[test]
    fn set_preedit_stays_out_of_text_and_selection() {
        let mut f = TextField::with_text("你好世界");
        f.set_cursor(2);
        f.set_preedit("nǐ", Some(0));
        assert_eq!(f.text(), "你好世界", "preedit 不入 text");
        assert_eq!(f.preedit(), "nǐ");
        assert_eq!(f.selection(), None, "preedit 不进选区");
        assert_eq!(f.cursor(), 2, "光标位置不变（组合态在其后）");
    }

    #[test]
    fn set_preedit_cursor_byte_to_char() {
        let mut f = TextField::new();
        // "你好" = 6 字节；光标落在第 3 字节（'好' 起点）→ 字符下标 1。
        f.set_preedit("你好", Some(3));
        assert_eq!(f.preedit_cursor(), Some(1));
        // 落在字符中间（第 4 字节）→ 外扩夹紧到字符起点（3）。
        f.set_preedit("你好", Some(4));
        assert_eq!(f.preedit_cursor(), Some(1));
        // 超界 → 夹到尾。
        f.set_preedit("你好", Some(99));
        assert_eq!(f.preedit_cursor(), Some(2));
        // None = 尾。
        f.set_preedit("你好", None);
        assert_eq!(f.preedit_cursor(), None);
    }

    #[test]
    fn commit_ime_inserts_at_cursor() {
        let mut f = TextField::with_text("你 世界");
        f.set_cursor(2);
        f.set_preedit("ni", None);
        assert!(f.commit_ime("你"));
        assert_eq!(f.text(), "你 你世界");
        assert_eq!(f.cursor(), 3);
        assert!(!f.has_preedit(), "提交后清组合态");
    }

    #[test]
    fn commit_ime_replaces_selection() {
        let mut f = TextField::with_text("hello world");
        f.select_all();
        f.set_preedit("a", None);
        assert!(f.commit_ime("你好"));
        assert_eq!(f.text(), "你好", "提交覆盖选区");
        assert_eq!(f.selection(), None);
    }

    #[test]
    fn commit_ime_empty_cancels_composition() {
        let mut f = TextField::with_text("abc");
        f.move_end(false);
        f.set_preedit("ni", None);
        assert!(f.commit_ime(""), "空提交返回 true（状态变化）");
        assert!(!f.has_preedit());
        assert_eq!(f.text(), "abc");
        assert!(!f.commit_ime(""), "无组合态时空提交 no-op");
    }

    #[test]
    fn commit_ime_is_undoable_atomically() {
        let mut f = TextField::with_text("ab");
        f.move_end(false);
        f.set_preedit("cd", None);
        f.commit_ime("你好");
        assert_eq!(f.text(), "ab你好");
        assert!(f.undo());
        assert_eq!(f.text(), "ab", "撤销一次回滚整个提交");
    }

    #[test]
    fn escape_clears_preedit() {
        let mut f = TextField::with_text("ab");
        f.move_end(false);
        f.set_preedit("cd", None);
        f.handle_key(TextInputKey::Escape);
        assert!(!f.has_preedit(), "Escape 取消组合");
        assert_eq!(f.text(), "ab", "文本不受影响");
    }

    #[test]
    fn typed_char_breaks_composition() {
        let mut f = TextField::with_text("ab");
        f.move_end(false);
        f.set_preedit("cd", None);
        f.insert_char('X');
        assert!(!f.has_preedit(), "直接键插入打断组合态");
        assert_eq!(f.text(), "abX");
    }

    #[test]
    fn delete_surrounding_deletes_cjk_bytes() {
        let mut f = TextField::with_text("你好世界");
        f.set_cursor(2); // 光标在"好"与"世"之间
        // 删前 3 字节（"好"）+ 后 3 字节（"世"）→ "你好世" 剩"你"和"界"
        assert!(f.delete_surrounding(3, 3));
        assert_eq!(f.text(), "你界");
        assert_eq!(f.cursor(), 1);
    }

    #[test]
    fn delete_surrounding_snaps_byte_boundary() {
        let mut f = TextField::with_text("你好a");
        f.set_cursor(1); // 光标在"你"后
        // before=1 字节：'你' 是 3 字节 → 贪心收紧到 0 字符（不劈码点）
        assert!(!f.delete_surrounding(1, 0), "不足一个码点 → 无删除");
        assert_eq!(f.text(), "你好a");
        // before=2 字节仍不足 3 → 0 字符
        assert!(!f.delete_surrounding(2, 0));
        assert_eq!(f.text(), "你好a");
        // before=3 字节 → 删"你"
        assert!(f.delete_surrounding(3, 0));
        assert_eq!(f.text(), "好a");
    }

    #[test]
    fn delete_surrounding_clears_composition() {
        let mut f = TextField::with_text("ab");
        f.move_end(false);
        f.set_preedit("cd", None);
        f.delete_surrounding(0, 0); // no-op 不清
        assert!(f.has_preedit());
        f.delete_surrounding(1, 0);
        assert!(!f.has_preedit(), "周边删除后组合态失效");
    }

    #[test]
    fn surrounding_text_caps_bytes_at_utf8_boundary() {
        let mut f = TextField::with_text("你好世界");
        f.set_cursor(1); // before="你"(3B) after="好世界"
        let (before, after, cursor_byte, anchor_byte) = f.surrounding_text(5);
        assert_eq!(before, "你", "before 截到 ≤5 字节且不劈码点");
        assert_eq!(after, "好", "after 5 字节：好世=6B 超 → 贪心收紧为 好");
        assert_eq!(cursor_byte, 3);
        assert_eq!(anchor_byte, 3, "无选区 anchor = cursor");
        // 超界：after 截到 5 字节 → "好世"（6B 超 → 收为"好世"？"好"+“世”=6>5 → 收为"好"）
        let (_, after, _, _) = f.surrounding_text(3);
        assert_eq!(after, "好", "after 3 字节只装下 “好”");
    }

    #[test]
    fn surrounding_text_reports_anchor_bytes() {
        let mut f = TextField::with_text("abcdef");
        f.set_cursor(4);
        f.move_left(true); // 选区 anchor=4, cursor=3 → 选中 "c"
        let (before, after, cursor_byte, anchor_byte) = f.surrounding_text(1000);
        assert_eq!(before, "abc");
        assert_eq!(after, "def");
        assert_eq!(cursor_byte, 3);
        assert_eq!(anchor_byte, 4, "锚点在光标后 1 字符 → 4 字节");
    }

    #[test]
    fn preedit_is_masked_display() {
        let mut f = TextField::with_text("secret");
        f.move_end(false);
        f.set_mask(Some('●'));
        f.set_preedit("abc", None);
        assert_eq!(f.preedit(), "abc", "原始保留");
        assert_eq!(f.preedit_display(), "●●●", "掩码组合态");
    }
}
