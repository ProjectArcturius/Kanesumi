// focus.rs —— 焦点环：UWP `FocusManager` 的最小等价物。
// 参 docs/MATURITY_AUDIT_2026-09-22.md「元件协调」。
//
// 为什么需要它：全仓每个控件各持一个 `pub focused: bool`，既无单实例保证、也无 Tab 顺序、
// 更无跨控件协调 —— 结果是「同时两个控件显示焦点环」与「纯键盘无法操作任何屏幕」。

/// 焦点身份。由 App 决定，须**跨帧稳定**（如 `hash(page, index)`）。
///
/// 不用引用/指针：焦点环不持有视觉树（参 `PLAN.md §4.1` 状态驱动渲染不变量）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FocusId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FocusItem {
    id: FocusId,
    /// 是否占用 Tab 位。`false` = 只接受程序化聚焦（如容器内的可点区域）。
    tab_stop: bool,
    enabled: bool,
}

/// 焦点环 —— 全局唯一焦点真源 + Tab 顺序。
///
/// 契约（与状态驱动渲染一致，不持有视觉树）：
/// 1. 每帧 [`begin_frame`](Self::begin_frame) 清空登记表，App 按**视觉顺序**登记本帧可聚焦项；
/// 2. 控件渲染时以 [`is_focused`](Self::is_focused) 决定是否画焦点环，
///    **不再各持一个 `focused: bool`**（这是「两个控件同时亮焦点环」的根因）；
/// 3. [`focus_next`](Self::focus_next) 只走 `tab_stop && enabled`，跳过禁用项并环绕；
/// 4. [`end_frame`](Self::end_frame) 收敛：焦点项本帧消失或被禁用 → 自动转移到第一个可聚焦项。
///
/// 用法：
/// ```ignore
/// ring.begin_frame();
/// ring.register(FocusId(1), true, true);   // 顺序即 Tab 顺序
/// ring.register(FocusId(2), true, false);  // 禁用 → 被跳过
/// ring.end_frame();
/// if ring.is_focused(FocusId(1)) { /* 画焦点环 */ }
/// ```
#[derive(Debug, Clone, Default)]
pub struct FocusRing {
    items: Vec<FocusItem>,
    focused: Option<FocusId>,
    /// 最近一次聚焦的登记序号（供环绕起点使用）。
    cursor: usize,
}

impl FocusRing {
    pub fn new() -> Self {
        Self::default()
    }

    /// 帧首：清空登记表。焦点本身保留，由 `end_frame` 校验其是否仍存在。
    pub fn begin_frame(&mut self) {
        self.items.clear();
    }

    /// 按**视觉顺序**登记一个可聚焦项。
    pub fn register(&mut self, id: FocusId, tab_stop: bool, enabled: bool) {
        self.items.push(FocusItem {
            id,
            tab_stop,
            enabled,
        });
    }

    /// 当前焦点（全局唯一）。
    pub fn focused(&self) -> Option<FocusId> {
        self.focused
    }

    pub fn is_focused(&self, id: FocusId) -> bool {
        self.focused == Some(id)
    }

    pub fn is_registered(&self, id: FocusId) -> bool {
        self.index_of(id).is_some()
    }

    pub fn blur(&mut self) {
        self.focused = None;
    }

    fn index_of(&self, id: FocusId) -> Option<usize> {
        self.items.iter().position(|i| i.id == id)
    }

    /// 程序化聚焦（鼠标点击/快捷键）。未登记或被禁用 → 忽略并返回 `false`。
    pub fn focus(&mut self, id: FocusId) -> bool {
        match self.index_of(id) {
            Some(i) if self.items[i].enabled => {
                self.focused = Some(id);
                self.cursor = i;
                true
            }
            _ => false,
        }
    }

    /// Tab（`backward = false`）/ Shift+Tab（`true`）推进。
    ///
    /// 只落在 `tab_stop && enabled` 的项上；越过末尾环绕。无可聚焦项 → 清空焦点并返回 `None`。
    pub fn focus_next(&mut self, backward: bool) -> Option<FocusId> {
        let n = self.items.len();
        if n == 0 {
            self.focused = None;
            return None;
        }
        // 无焦点时：向前从「第一项之前」出发，向后从末尾出发 —— 首次 Tab 必须命中首项/末项。
        let start = match self.focused.and_then(|id| self.index_of(id)) {
            Some(i) => i,
            None => {
                if backward {
                    n
                } else {
                    n - 1
                }
            }
        };
        for step in 1..=n {
            let idx = if backward {
                (start + n - step) % n
            } else {
                (start + step) % n
            };
            let it = self.items[idx];
            if it.tab_stop && it.enabled {
                self.focused = Some(it.id);
                self.cursor = idx;
                return Some(it.id);
            }
        }
        // 全是非 tab_stop 或全禁用 → 不保留悬空焦点。
        self.focused = None;
        None
    }

    /// 帧末收敛：焦点项本帧未登记或被禁用 → 转移到第一个可聚焦项（无则清空）。
    pub fn end_frame(&mut self) {
        let alive = self
            .focused
            .and_then(|id| self.index_of(id))
            .map(|i| self.items[i].enabled)
            .unwrap_or(false);
        if !alive {
            self.focused = None;
            let _ = self.focus_next(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: FocusId = FocusId(1);
    const B: FocusId = FocusId(2);
    const C: FocusId = FocusId(3);

    fn ring_with(ids: &[(FocusId, bool, bool)]) -> FocusRing {
        let mut r = FocusRing::new();
        r.begin_frame();
        for (id, tab, en) in ids {
            r.register(*id, *tab, *en);
        }
        r
    }

    #[test]
    fn focus_is_single_instance() {
        let mut r = ring_with(&[(A, true, true), (B, true, true)]);
        assert!(r.focus(A));
        assert!(r.is_focused(A));
        assert!(r.focus(B));
        assert!(!r.is_focused(A), "焦点必须唯一 —— 旧实现的根因是两个控件各持 bool");
        assert!(r.is_focused(B));
    }

    #[test]
    fn tab_moves_in_registration_order_and_wraps() {
        let mut r = ring_with(&[(A, true, true), (B, true, true), (C, true, true)]);
        assert_eq!(r.focus_next(false), Some(A), "无焦点时首次 Tab 命中首项");
        assert_eq!(r.focus_next(false), Some(B));
        assert_eq!(r.focus_next(false), Some(C));
        assert_eq!(r.focus_next(false), Some(A), "末尾环绕");
    }

    #[test]
    fn shift_tab_moves_backward_and_wraps() {
        let mut r = ring_with(&[(A, true, true), (B, true, true), (C, true, true)]);
        assert_eq!(r.focus_next(true), Some(C), "无焦点时 Shift+Tab 命中末项");
        assert_eq!(r.focus_next(true), Some(B));
        assert_eq!(r.focus_next(true), Some(A));
        assert_eq!(r.focus_next(true), Some(C), "首项再后退环绕到末尾");
    }

    #[test]
    fn disabled_items_are_skipped() {
        let mut r = ring_with(&[(A, true, true), (B, true, false), (C, true, true)]);
        assert_eq!(r.focus_next(false), Some(A));
        assert_eq!(r.focus_next(false), Some(C), "禁用的 B 被跳过");
    }

    #[test]
    fn non_tab_stop_items_are_programmatic_only() {
        let mut r = ring_with(&[(A, true, true), (B, false, true), (C, true, true)]);
        assert_eq!(r.focus_next(false), Some(A));
        assert_eq!(r.focus_next(false), Some(C), "非 tab_stop 的 B 不占 Tab 位");
        assert!(r.focus(B), "但仍可程序化聚焦");
        assert!(r.is_focused(B));
    }

    #[test]
    fn focus_rejects_unregistered_and_disabled() {
        let mut r = ring_with(&[(A, true, true), (B, true, false)]);
        assert!(!r.focus(C), "未登记");
        assert!(!r.focus(B), "已禁用");
        assert_eq!(r.focused(), None);
    }

    #[test]
    fn end_frame_transfers_focus_when_item_disappears() {
        let mut r = ring_with(&[(A, true, true), (B, true, true)]);
        r.focus(A);
        // 下一帧 A 消失（如页签被关闭）
        r.begin_frame();
        r.register(B, true, true);
        r.end_frame();
        assert_eq!(r.focused(), Some(B), "焦点项消失 → 自动转移，不留悬空焦点");
    }

    #[test]
    fn end_frame_clears_when_nothing_focusable() {
        let mut r = ring_with(&[(A, true, true)]);
        r.focus(A);
        r.begin_frame();
        r.register(A, true, false); // 被禁用
        r.end_frame();
        assert_eq!(r.focused(), None);
    }

    #[test]
    fn empty_ring_has_no_focus() {
        let mut r = ring_with(&[]);
        assert_eq!(r.focus_next(false), None);
        assert_eq!(r.focused(), None);
    }
}
