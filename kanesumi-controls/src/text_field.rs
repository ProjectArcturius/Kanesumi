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
                // 取消选区（光标保留）。
                self.anchor = None;
                true
            }
            TextInputKey::Tab => false,
        }
    }
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
}
