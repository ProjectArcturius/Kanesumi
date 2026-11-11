// list_view.rs —— 列表行为核心：虚拟化窗口 / 选择集合 / 键入检索 / 滚动锚定。
//
// 只有一套列表：控件仍是 `list.rs` 的 `MetroList`，本文件是它的**纯状态与几何**——
// 不持 Scene、不做命中、不建节点。拆出来的理由：这些判据（窗口上下界、Shift 锚点、
// 前缀检索超时、内容变短后视口停在哪）都不该靠整树夹具间接观察，纯函数才逐条钉得住。
//
// 数值来源：滚轮步长取 `kanesumi_core::interaction::WHEEL_STEP_PX`（INTERACTION_CANON §Ⅴ
// 「一格 = 3 行 = 48 px」）；列表行高下限见 `sizes::LIST_ROW_MIN_HEIGHT`（CONTROL_SPEC §7）。
// 分页步长 = 视口高 − 一行（INTERACTION_CANON §Ⅴ PageUp / PageDown）。

use std::cell::Cell;
use std::ops::RangeInclusive;

use kanesumi_core::interaction::WHEEL_STEP_PX;

/// 每侧 overscan 行数：视口上下各多量两行，滚动一小段不必重新起一次量测。
///
/// **总预算 2 × 2 = 4 行**（任务书「overscan ≤ 4」）。这里刻意不调大：overscan 是
/// 「换滚动平滑」与「单帧量测次数」的对价，调大一行就多量一行，验收数字随之作废。
pub const LIST_OVERSCAN_ROWS: usize = 2;

// 总预算 2 × 2 = 4 行（任务书 overscan ≤ 4）。写成编译期断言：调大常量会先编译失败，
// 不会悄悄把验收数字（「1000 项量测 10 行」）作废。
const _: () = assert!(2 * LIST_OVERSCAN_ROWS <= 4);

/// type-ahead 超时窗口（ms）：连续字符间隔不超过它才串成同一前缀。
///
/// 正典（INTERACTION_CANON）未规定该值。取 1 s：短于 500 ms 会把正常连打（尤其中文
/// 输入法外的拉丁字母）拆成两段检索，长于 1.5 s 会让上一次前缀粘住下一次操作。
/// 登记为临时值，待正典补条后改为引用 `interaction` 常量。
pub const TYPE_AHEAD_TIMEOUT_MS: u64 = 1000;

/// 一帧要量测的行窗口。
///
/// `first..=last` 是**量测窗口**（可见 + 两侧 overscan）；`first_visible..=last_visible`
/// 是**真正落在视口里**的那段（无障碍只报这段：看不见的行不该被读屏读到）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListWindow {
    pub first: usize,
    pub last: usize,
    pub first_visible: usize,
    pub last_visible: usize,
}

impl ListWindow {
    /// 本帧量测行数（含 overscan）。
    pub fn rows(&self) -> usize {
        self.last.saturating_sub(self.first) + 1
    }

    /// 视口内行数（不含 overscan；边缘半行也算一行）。
    pub fn visible_rows(&self) -> usize {
        self.last_visible.saturating_sub(self.first_visible) + 1
    }

    /// 量测窗口的索引区间。
    pub fn span(&self) -> RangeInclusive<usize> {
        self.first..=self.last
    }

    /// 视口窗口的索引区间（无障碍 / 命中只认这段）。
    pub fn visible_span(&self) -> RangeInclusive<usize> {
        self.first_visible..=self.last_visible
    }
}

/// 给定内容与视口，算本帧的行窗口。空列表 / 退化行高 / 零视口 → `None`。
///
/// `offset` 应由调用方夹紧到 `[0, max_scroll]`（与 `MetroRepeater::visible_range` 同契约，
/// 参 repeater.rs 该函数注）。这里仍对越界 offset 做兜底：夹到最后一行的位置，
/// 避免内容变短那一拍（夹紧尚未跑）拿到空窗口。
pub fn list_window(
    len: usize,
    row_h: f32,
    viewport_h: f32,
    offset: f32,
) -> Option<ListWindow> {
    if len == 0 || row_h <= 0.0 || viewport_h <= 0.0 {
        return None;
    }
    // 偏移先夹到 [0, max]：越界偏移（旧 App 直接写字段 / 内容刚变短）按末尾画，
    // 与 `MetroList::render` 的防御性夹紧同一口径。
    let max = (len as f32 * row_h - viewport_h).max(0.0);
    let offset = offset.clamp(0.0, max);
    let first_visible = ((offset / row_h).floor() as usize).min(len - 1);
    // 行 i 可见 ⟺ i*row_h < offset + viewport_h；末行 = ceil((offset+viewport)/row_h) − 1。
    let last_visible = ((((offset + viewport_h) / row_h).ceil() as usize)
        .saturating_sub(1))
    .min(len - 1)
    .max(first_visible);
    Some(ListWindow {
        first: first_visible.saturating_sub(LIST_OVERSCAN_ROWS),
        last: (last_visible + LIST_OVERSCAN_ROWS).min(len - 1),
        first_visible,
        last_visible,
    })
}

/// 滚轮一格的实际像素增量（INTERACTION_CANON §Ⅴ：一格 = 48 px）。
///
/// 为什么按格数归一：外壳旧路径每格发 50 px（正典偏离表 §Ⅸ 明列），事件里的
/// `steps` 是正确的格数、`dy` 却按 50 算。按 `steps` 乘 48 才能既纠正旧值、又不丢
/// 高精度滚轮的半格（0.5 格 = 24 px）。`steps` 缺失（0）时退回 `dy` 原值。
pub fn wheel_step_px(dy: f32, steps: f32) -> f32 {
    if steps == 0.0 {
        return dy;
    }
    // 离散步是整数格；把 ±0.1 格内的抖动当整数（50/48 = 1.0417 正好落在里面）。
    let notches = if (steps - steps.round()).abs() <= 0.1 {
        steps.round()
    } else {
        steps
    };
    notches * WHEEL_STEP_PX
}

/// 选中集合 + Shift 锚点。选中项恒为升序，供绘制 / 命中 / 无障碍共用。
///
/// 「焦点行」（lead，`MetroList::selected`）不在这里：Ctrl+点已选项会把它从集合里
/// 去掉，但焦点仍停在它上面（Windows 资源管理器同款：焦点 ≠ 选中）。lead 由控件持有。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListSelection {
    items: Vec<usize>,
    anchor: Option<usize>,
}

impl ListSelection {
    /// 选中项（升序）。
    pub fn items(&self) -> &[usize] {
        &self.items
    }

    pub fn contains(&self, index: usize) -> bool {
        self.items.binary_search(&index).is_ok()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Shift 连选的锚点（普通单击 / Ctrl+点会把它移到点击项）。
    pub fn anchor(&self) -> Option<usize> {
        self.anchor
    }

    /// 清空选中（锚点一并清）。
    pub fn clear(&mut self) {
        self.items.clear();
        self.anchor = None;
    }

    /// 单选 `index` 并把锚点移过去。
    pub fn select_only(&mut self, index: usize) {
        self.items.clear();
        self.items.push(index);
        self.anchor = Some(index);
    }

    /// 只在没有锚点时设锚（Shift 连选的第一下把 lead 记成范围起点）。
    pub fn set_anchor_if_none(&mut self, anchor: usize) {
        if self.anchor.is_none() {
            self.anchor = Some(anchor);
        }
    }

    /// 把选中集合夹回 `[0, len)`：内容变短后丢弃越界项。
    ///
    /// 锚点只在越界时清掉；lead 的收尾在控件侧（`MetroList::normalize_selection`）。
    pub fn retain_within(&mut self, len: usize) {
        self.items.retain(|i| *i < len);
        if self.anchor.is_some_and(|a| a >= len) {
            self.anchor = None;
        }
    }

    /// 全选 `0..len`（锚点保持：Windows 的 Ctrl+A 不改锚点）。
    pub fn select_all(&mut self, len: usize) {
        self.items = (0..len).collect();
    }

    /// 单击：普通 = 单选；Ctrl = 切换单项；Shift = 从锚点到 `index` 的连选（清掉旧集合）。
    ///
    /// 返回点后应有的 lead（焦点行）。`ClickAction` 式的枚举没有必要：三种模式的差别
    /// 全落在集合与锚点上，调用方紧接着要做的事（发事件 / 滚进视口）是一样的。
    pub fn click(&mut self, index: usize, ctrl: bool, shift: bool, len: usize) -> Option<usize> {
        if index >= len {
            return None;
        }
        if ctrl {
            match self.items.binary_search(&index) {
                Ok(pos) => {
                    self.items.remove(pos);
                }
                Err(pos) => self.items.insert(pos, index),
            }
            // Ctrl+点把锚点移到点击项：随后 Shift+点以此为范围起点（Win32 同款）。
            self.anchor = Some(index);
        } else if shift {
            let anchor = self.anchor.unwrap_or(index).min(len.saturating_sub(1));
            self.set_anchor_if_none(anchor);
            self.select_range(anchor, index);
        } else {
            self.select_only(index);
        }
        Some(index)
    }

    /// 连选 `from..=to`（端点先后无关），锚点不动。
    pub fn select_range(&mut self, from: usize, to: usize) {
        let (lo, hi) = if from <= to { (from, to) } else { (to, from) };
        self.items = (lo..=hi).collect();
    }
}

/// 键盘处理结果：`handled` = 列表消费了这次按键（不冒泡）；`changed` = 状态变了
/// （要重画并发 `ListSelectionChanged`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyOutcome {
    pub handled: bool,
    pub changed: bool,
}

impl KeyOutcome {
    pub const IGNORED: KeyOutcome = KeyOutcome {
        handled: false,
        changed: false,
    };
}

/// 键入检索（type-ahead）：超时窗口内连续字符串成一个前缀。
///
/// 比较一律小写前缀（`"BE"` 命中 `"Beta"`）；从 lead 的下一项开始找并回绕，所以
/// 连按同一前缀会停在原项上而不是原地不动（与 Win32 列表框的 `LB_FINDSTRING` 同向）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TypeAhead {
    buffer: String,
    deadline_ms: u64,
}

impl TypeAhead {
    /// 追加一个字符，返回（可能已重置的）前缀。
    ///
    /// 时刻语义：`now_ms` 越过上一字符的 deadline 就先清前缀（**逐键续期**）。
    /// 元素树里没有逐帧时钟，控件传 0、由登记的定时器在**前缀首字符**后 1000 ms 调
    /// [`TypeAhead::reset`] —— 那一路是「自首字符起算」，与这里的逐键续期不同（T27 已登记）。
    pub fn push(&mut self, ch: char, now_ms: u64) -> &str {
        if now_ms > self.deadline_ms {
            self.buffer.clear();
        }
        self.buffer.push(ch);
        self.deadline_ms = now_ms + TYPE_AHEAD_TIMEOUT_MS;
        &self.buffer
    }

    pub fn buffer(&self) -> &str {
        &self.buffer
    }

    /// 前缀是否还在超时窗口内。
    pub fn active(&self, now_ms: u64) -> bool {
        !self.buffer.is_empty() && now_ms <= self.deadline_ms
    }

    pub fn reset(&mut self) {
        self.buffer.clear();
        self.deadline_ms = 0;
    }

    /// 从 `after` 的下一项起找第一条以前缀开头的行；找不到 → `None`。
    pub fn matching(&self, rows: &[String], after: Option<usize>) -> Option<usize> {
        if self.buffer.is_empty() || rows.is_empty() {
            return None;
        }
        let prefix = self.buffer.to_lowercase();
        let len = rows.len();
        let start = match after {
            Some(i) => (i + 1) % len,
            None => 0,
        };
        (0..len)
            .map(|n| (start + n) % len)
            .find(|i| rows[*i].to_lowercase().starts_with(&prefix))
    }
}

/// 单帧量测计数。瞬态统计：不参与相等 / 克隆语义（`PartialEq` 恒真）。
///
/// 用 `Cell` 而不是 `&mut self`：`MetroList::render` 是 `&self`（旧 App 直接调），
/// 而计数要落在「这一帧画了几行」上。验收读 [`MeasureCounter::get`]。
#[derive(Debug, Default)]
pub struct MeasureCounter(Cell<usize>);

impl MeasureCounter {
    /// 开一帧：清零。
    pub fn reset(&self) {
        self.0.set(0);
    }

    /// 本帧又量了一行。
    pub fn add(&self, rows: usize) {
        self.0.set(self.0.get() + rows);
    }

    /// 本帧量测行数。
    pub fn get(&self) -> usize {
        self.0.get()
    }
}

impl Clone for MeasureCounter {
    fn clone(&self) -> Self {
        Self(Cell::new(self.0.get()))
    }
}

impl PartialEq for MeasureCounter {
    /// 量测计数是瞬态观测值，不定义控件相等性。
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl Eq for MeasureCounter {}

/// 内容变化前的滚动锚：视口首行是谁、行内偏移多少、它的标签是什么。
///
/// 为什么要标签：`rows: Vec<String>` 没有稳定 id，光记索引的话「上面插入 100 行」
/// 会让同一索引指向另一条内容——视口里那一项就跳了。按标签在新数据里就近找回来，
/// 能把「插入 / 删除在视口之上」也保住；重名行取离旧索引最近的一处（见 `find_anchor`）。
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollAnchor {
    pub top_index: usize,
    /// 首行被滚出视口的高度（`0.0` = 首行顶边贴视口顶边）。
    pub within_row: f32,
    pub label: Option<String>,
}

/// 在 `rows` 里找 `label`，优先同索引，其次向两侧扩（重名行取最近的一处）。
pub fn find_anchor(rows: &[String], label: &str, near: usize) -> Option<usize> {
    if rows.is_empty() {
        return None;
    }
    let near = near.min(rows.len() - 1);
    if rows[near] == label {
        return Some(near);
    }
    let extent = near.max(rows.len() - 1 - near);
    for step in 1..=extent {
        if let Some(i) = near.checked_sub(step)
            && rows[i] == label
        {
            return Some(i);
        }
        let i = near + step;
        if i < rows.len() && rows[i] == label {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::list::MetroList;
    use kanesumi_element::{Key, Modifiers};

    const ROW_H: f32 = 40.0;
    const VIEWPORT: f32 = 200.0;

    fn list(n: usize) -> MetroList {
        MetroList::new((0..n).map(|i| format!("行 {i}")).collect())
    }

    fn named(rows: &[&str]) -> MetroList {
        MetroList::new(rows.iter().map(|s| s.to_string()).collect())
    }

    fn key(l: &mut MetroList, k: Key, m: Modifiers, now_ms: u64) -> bool {
        l.handle_key(k, m, ROW_H, VIEWPORT, now_ms).handled
    }

    fn sel(l: &MetroList) -> String {
        if l.selection.is_empty() {
            String::new()
        } else {
            l.selection
                .items()
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    fn lead(l: &MetroList) -> String {
        l.selected.map(|i| i.to_string()).unwrap_or_else(|| "-".into())
    }

    fn anchor(l: &MetroList) -> String {
        l.selection
            .anchor()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "-".into())
    }

    /// 状态转录：一行 = 一次输入 + 该输入后的可观察状态。
    ///
    /// 格式同 `kanesumi-element/src/interaction.rs`（参 ELEMENT_TREE §Ⅵ）：手感问题不拿
    /// 截图当判据，同一条输入序列必须产出逐字节相同的转录。`KANESUMI_TRANSCRIPT=1`
    /// 时把原文打到 stdout，报告里贴的就是这段。
    #[derive(Default)]
    struct Trace(Vec<String>);

    impl Trace {
        fn state(&mut self, seq: u32, input: &str, l: &MetroList) {
            self.0.push(format!(
                "{seq:02} {input:<24} lead={} sel=[{}] anchor={} hover={} scroll={:.0}",
                lead(l),
                sel(l),
                anchor(l),
                l.hovered
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| "-".into()),
                l.scroll
            ));
        }

        fn expect(&self, expected: &str) {
            if std::env::var_os("KANESUMI_TRANSCRIPT").is_some() {
                println!("----\n{}", self.0.join("\n"));
            }
            let squash = |s: &str| {
                s.lines()
                    .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            assert_eq!(
                squash(&self.0.join("\n")),
                squash(expected),
                "状态转录与钉死的行为不符"
            );
        }
    }

    // ── ① 空列表 / 单项 ─────────────────────────────────────────────────────

    #[test]
    fn transcript_empty_list_consumes_navigation() {
        let mut l = list(0);
        let mut t = Trace::default();
        t.state(1, "start", &l);
        assert!(key(&mut l, Key::Down, Modifiers::NONE, 0), "空列表也消费导航键");
        t.state(2, "key Down", &l);
        assert!(key(&mut l, Key::PageDown, Modifiers::NONE, 0));
        assert!(key(&mut l, Key::End, Modifiers::NONE, 0));
        t.state(3, "key PageDown/End", &l);
        assert!(!l.click_row(0, false, false), "空表点击无变化");
        t.state(4, "click(0)", &l);
        assert_eq!(l.window(ROW_H, VIEWPORT), None, "空表没有量测窗口");
        t.expect(
            "01 start                    lead=- sel=[] anchor=- hover=- scroll=0\n\
             02 key Down                 lead=- sel=[] anchor=- hover=- scroll=0\n\
             03 key PageDown/End         lead=- sel=[] anchor=- hover=- scroll=0\n\
             04 click(0)                 lead=- sel=[] anchor=- hover=- scroll=0",
        );
    }

    #[test]
    fn transcript_single_item_stays_put() {
        let mut l = list(1);
        let mut t = Trace::default();
        for (seq, input, k) in [
            (1, "key Down", Key::Down),
            (2, "key Down again", Key::Down),
            (3, "key Up", Key::Up),
            (4, "key End", Key::End),
            (5, "key Home", Key::Home),
        ] {
            assert!(key(&mut l, k, Modifiers::NONE, 0));
            t.state(seq, input, &l);
        }
        t.state(6, "click(0)", {
            l.click_row(0, false, false);
            &l
        });
        t.expect(
            "01 key Down                 lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             02 key Down again           lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             03 key Up                   lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             04 key End                  lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             05 key Home                 lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             06 click(0)                 lead=0 sel=[0] anchor=0 hover=- scroll=0",
        );
    }

    // ── ② 虚拟化量测 ───────────────────────────────────────────────────────

    #[test]
    fn virtualization_measures_visible_plus_overscan_for_1000_rows() {
        let mut l = list(1000);
        let w = l.measure_window(ROW_H, 300.0).expect("1000 行有窗口");
        assert_eq!(w.visible_rows(), 8, "300 / 40 = 8 行落在视口里");
        assert_eq!(w.rows(), 10, "顶端没有上方行可 overscan：8 可见 + 下方 2");
        assert!(
            w.rows() <= w.visible_rows() + 2 * LIST_OVERSCAN_ROWS,
            "单帧量测上界 = 可见 + overscan"
        );
        assert_eq!(l.measures_last_frame(), 10, "计数器读到同一数字");

        // 滚到中段：窗口仍是 12 行，不随总行数增长。
        l.scroll = 20_000.0; // 首行 = 500
        let w = l.measure_window(ROW_H, 300.0).unwrap();
        assert_eq!((w.first_visible, w.last_visible), (500, 507));
        assert_eq!((w.first, w.last), (498, 509));
        assert_eq!(l.measures_last_frame(), 12);

        // 到底：窗口夹到最后一行，不越界。
        l.scroll = l.max_scroll_with(ROW_H, 300.0);
        let w = l.measure_window(ROW_H, 300.0).unwrap();
        assert_eq!(w.last, 999);
        assert_eq!(w.rows(), 10, "末尾同样只多出单侧 overscan");
    }

    // ── ③ 键盘边界 ─────────────────────────────────────────────────────────

    #[test]
    fn transcript_arrows_stop_at_both_boundaries() {
        let mut l = list(20);
        let mut t = Trace::default();
        t.state(1, "start", &l);
        for seq in 2..=7 {
            assert!(key(&mut l, Key::Down, Modifiers::NONE, 0));
            t.state(seq, "key Down", &l);
        }
        // 已到视口下沿：第 5 行（索引 5）必须滚进来，偏移 = 5*40 + 40 − 200 = 40。
        assert_eq!(l.scroll, 40.0);
        for _ in 0..20 {
            key(&mut l, Key::Down, Modifiers::NONE, 0);
        }
        t.state(8, "key Down x20", &l);
        for _ in 0..30 {
            key(&mut l, Key::Up, Modifiers::NONE, 0);
        }
        t.state(9, "key Up x30", &l);
        t.expect(
            "01 start                    lead=- sel=[] anchor=- hover=- scroll=0\n\
             02 key Down                 lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             03 key Down                 lead=1 sel=[1] anchor=1 hover=- scroll=0\n\
             04 key Down                 lead=2 sel=[2] anchor=2 hover=- scroll=0\n\
             05 key Down                 lead=3 sel=[3] anchor=3 hover=- scroll=0\n\
             06 key Down                 lead=4 sel=[4] anchor=4 hover=- scroll=0\n\
             07 key Down                 lead=5 sel=[5] anchor=5 hover=- scroll=40\n\
             08 key Down x20             lead=19 sel=[19] anchor=19 hover=- scroll=600\n\
             09 key Up x30               lead=0 sel=[0] anchor=0 hover=- scroll=0",
        );
    }

    #[test]
    fn transcript_page_down_reaches_bottom_and_page_up_returns() {
        let mut l = list(100);
        let mut t = Trace::default();
        key(&mut l, Key::Down, Modifiers::NONE, 0);
        t.state(1, "key Down", &l);
        // 视口 200 / 行 40 → 5 行，分页 = 5 − 1 = 4。
        assert_eq!(MetroList::page_rows(ROW_H, VIEWPORT), 4);
        assert!(key(&mut l, Key::PageDown, Modifiers::NONE, 0));
        t.state(2, "key PageDown", &l);
        for _ in 0..40 {
            key(&mut l, Key::PageDown, Modifiers::NONE, 0);
        }
        t.state(3, "key PageDown x40", &l);
        assert_eq!(l.selected, Some(99), "PageDown 到底停在末项");
        assert_eq!(l.scroll, 99.0 * 40.0 + 40.0 - 200.0, "末项底边贴视口底");
        assert!(key(&mut l, Key::PageUp, Modifiers::NONE, 0));
        t.state(4, "key PageUp", &l);
        t.expect(
            "01 key Down                 lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             02 key PageDown             lead=4 sel=[4] anchor=4 hover=- scroll=0\n\
             03 key PageDown x40        lead=99 sel=[99] anchor=99 hover=- scroll=3800\n\
             04 key PageUp               lead=95 sel=[95] anchor=95 hover=- scroll=3800",
        );
    }

    #[test]
    fn transcript_home_end_move_to_edges_and_scroll() {
        let mut l = list(100);
        let mut t = Trace::default();
        t.state(1, "start", &l);
        assert!(key(&mut l, Key::End, Modifiers::NONE, 0));
        t.state(2, "key End", &l);
        assert!(key(&mut l, Key::Home, Modifiers::NONE, 0));
        t.state(3, "key Home", &l);
        t.expect(
            "01 start                    lead=- sel=[] anchor=- hover=- scroll=0\n\
             02 key End                  lead=99 sel=[99] anchor=99 hover=- scroll=3800\n\
             03 key Home                 lead=0 sel=[0] anchor=0 hover=- scroll=0",
        );
    }

    // ── ④ type-ahead ───────────────────────────────────────────────────────

    #[test]
    fn transcript_type_ahead_chars_form_one_prefix() {
        let mut l = named(&["Alpha", "Beta", "Bravo", "Gamma"]);
        let mut t = Trace::default();
        assert!(key(&mut l, Key::Char('b'), Modifiers::NONE, 0));
        t.state(1, "type 'b'", &l);
        assert_eq!(l.type_ahead.buffer(), "b");
        assert!(key(&mut l, Key::Char('r'), Modifiers::NONE, 300));
        t.state(2, "type 'r' (+300ms)", &l);
        assert_eq!(l.type_ahead.buffer(), "br", "窗口内串成同一前缀");
        assert!(l.type_ahead.active(1_200), "1000 ms 窗口内仍有效");
        t.expect(
            "01 type 'b'                 lead=1 sel=[1] anchor=1 hover=- scroll=0\n\
             02 type 'r' (+300ms)        lead=2 sel=[2] anchor=2 hover=- scroll=0",
        );
    }

    #[test]
    fn transcript_type_ahead_resets_after_timeout() {
        let mut l = named(&["Beta", "Echo"]);
        let mut t = Trace::default();
        key(&mut l, Key::Char('b'), Modifiers::NONE, 0);
        t.state(1, "type 'b'", &l);
        // 1200 ms 后（> 1000 ms 窗口）：前缀重开，只剩 "e" → Echo；若旧前缀粘住
        // （"be"）会命中 Beta，状态就停在 0 —— 这条转录专抓「超时不重置」。
        key(&mut l, Key::Char('e'), Modifiers::NONE, 1_200);
        t.state(2, "type 'e' (+1200ms)", &l);
        assert_eq!(l.type_ahead.buffer(), "e");
        t.expect(
            "01 type 'b'                 lead=0 sel=[0] anchor=0 hover=- scroll=0\n\
             02 type 'e' (+1200ms)       lead=1 sel=[1] anchor=1 hover=- scroll=0",
        );
    }

    // ── ⑤ 选择三模式 ───────────────────────────────────────────────────────

    #[test]
    fn transcript_ctrl_click_toggles_items() {
        let mut l = list(6);
        let mut t = Trace::default();
        l.click_row(1, false, false);
        t.state(1, "click(1)", &l);
        l.click_row(3, true, false);
        t.state(2, "ctrl+click(3)", &l);
        l.click_row(1, true, false);
        t.state(3, "ctrl+click(1)", &l);
        l.click_row(1, true, false);
        t.state(4, "ctrl+click(1) again", &l);
        t.expect(
            "01 click(1)                 lead=1 sel=[1] anchor=1 hover=- scroll=0\n\
             02 ctrl+click(3)            lead=3 sel=[1,3] anchor=3 hover=- scroll=0\n\
             03 ctrl+click(1)            lead=1 sel=[3] anchor=1 hover=- scroll=0\n\
             04 ctrl+click(1) again      lead=1 sel=[1,3] anchor=1 hover=- scroll=0",
        );
    }

    #[test]
    fn transcript_shift_click_extends_from_anchor() {
        let mut l = list(10);
        let mut t = Trace::default();
        l.click_row(2, false, false);
        t.state(1, "click(2)", &l);
        l.click_row(6, false, true);
        t.state(2, "shift+click(6)", &l);
        l.click_row(4, false, true);
        t.state(3, "shift+click(4)", &l);
        l.click_row(8, true, false);
        t.state(4, "ctrl+click(8)", &l);
        l.click_row(6, false, true);
        t.state(5, "shift+click(6)", &l);
        t.expect(
            "01 click(2)                 lead=2 sel=[2] anchor=2 hover=- scroll=0\n\
             02 shift+click(6)           lead=6 sel=[2,3,4,5,6] anchor=2 hover=- scroll=0\n\
             03 shift+click(4)           lead=4 sel=[2,3,4] anchor=2 hover=- scroll=0\n\
             04 ctrl+click(8)            lead=8 sel=[2,3,4,8] anchor=8 hover=- scroll=0\n\
             05 shift+click(6)           lead=6 sel=[6,7,8] anchor=8 hover=- scroll=0",
        );
    }

    #[test]
    fn transcript_hover_and_selected_coexist_without_collapsing() {
        let mut l = list(6);
        let mut t = Trace::default();
        l.hovered = Some(2);
        t.state(1, "hover(2)", &l);
        l.click_row(1, false, false);
        t.state(2, "click(1) while hover(2)", &l);
        assert!(
            !l.is_selected(2),
            "悬停行不因悬停进入选中集合（hover ≠ selected）"
        );
        l.hovered = None;
        t.state(3, "pointer leave", &l);
        t.expect(
            "01 hover(2)                 lead=- sel=[] anchor=- hover=2 scroll=0\n\
             02 click(1) while hover(2)  lead=1 sel=[1] anchor=1 hover=2 scroll=0\n\
             03 pointer leave            lead=1 sel=[1] anchor=1 hover=- scroll=0",
        );
    }

    // ── ⑥ 滚轮 / 锚定 ──────────────────────────────────────────────────────

    #[test]
    fn transcript_wheel_clamps_at_both_ends() {
        let mut l = list(10); // 内容 400，视口 200 → max 200
        let mut t = Trace::default();
        t.state(1, "start", &l);
        assert!(l.scroll_wheel(50.0, 50.0 / 48.0, ROW_H, VIEWPORT));
        t.state(2, "wheel +1 (legacy 50px)", &l);
        for _ in 0..10 {
            l.scroll_wheel(50.0, 50.0 / 48.0, ROW_H, VIEWPORT);
        }
        t.state(3, "wheel +10 more", &l);
        assert!(!l.scroll_wheel(50.0, 1.0, ROW_H, VIEWPORT), "到底不消费");
        t.state(4, "wheel at bottom", &l);
        assert!(l.scroll_wheel(-50.0, -1.0, ROW_H, VIEWPORT));
        t.state(5, "wheel -1", &l);
        t.expect(
            "01 start                    lead=- sel=[] anchor=- hover=- scroll=0\n\
             02 wheel +1 (legacy 50px)   lead=- sel=[] anchor=- hover=- scroll=48\n\
             03 wheel +10 more           lead=- sel=[] anchor=- hover=- scroll=200\n\
             04 wheel at bottom          lead=- sel=[] anchor=- hover=- scroll=200\n\
             05 wheel -1                 lead=- sel=[] anchor=- hover=- scroll=152",
        );
    }

    #[test]
    fn transcript_content_shrink_keeps_top_row() {
        let mut l = list(1000);
        l.scroll = 2000.0; // 首行 = 50
        let mut t = Trace::default();
        t.state(1, "scrolled to row 50", &l);
        l.set_rows(list(600).rows, ROW_H, VIEWPORT);
        t.state(2, "rows 1000 -> 600", &l);
        assert_eq!(l.scroll, 2000.0, "首行（行 50）不动");
        l.set_rows(list(10).rows, ROW_H, VIEWPORT);
        t.state(3, "rows 600 -> 10", &l);
        t.expect(
            "01 scrolled to row 50       lead=- sel=[] anchor=- hover=- scroll=2000\n\
             02 rows 1000 -> 600         lead=- sel=[] anchor=- hover=- scroll=2000\n\
             03 rows 600 -> 10           lead=- sel=[] anchor=- hover=- scroll=200",
        );
    }

    #[test]
    fn transcript_content_insert_above_keeps_anchor_label() {
        let mut l = list(500);
        l.select(Some(50));
        l.scroll = 2000.0; // 首行 = 行 50
        let mut t = Trace::default();
        t.state(1, "scrolled to row 50", &l);
        let mut rows: Vec<String> = (0..100).map(|i| format!("前置 {i}")).collect();
        rows.extend((0..500).map(|i| format!("行 {i}")));
        l.set_rows(rows, ROW_H, VIEWPORT);
        t.state(2, "insert 100 rows above", &l);
        assert_eq!(l.scroll, 6000.0, "行 50 现在索引 150，视口仍停在它");
        t.expect(
            "01 scrolled to row 50       lead=50 sel=[50] anchor=50 hover=- scroll=2000\n\
             02 insert 100 rows above    lead=50 sel=[50] anchor=50 hover=- scroll=6000",
        );
    }

    #[test]
    fn select_all_and_clear_keep_lead_separate() {
        let mut l = list(4);
        l.selection.select_all(4);
        assert_eq!(l.selection.items(), &[0, 1, 2, 3]);
        assert_eq!(l.selected, None, "全选不改焦点行");
        l.select(None);
        assert!(l.selection.is_empty() && l.selection.anchor().is_none());
        l.selection.select_all(4);
        l.set_rows(vec!["只剩一行".into()], ROW_H, VIEWPORT);
        assert_eq!(l.selection.items(), &[0], "内容变短后越界选中项被丢掉");
    }
}
