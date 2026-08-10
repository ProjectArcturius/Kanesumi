// 导航模型 —— 对应 UWP `Frame.Navigate` 的状态驱动实现。
//
// 参 Ether-main PLAN.md §4-1（state → progress → resolved spatial state → render）。
// Kanesumi 无保留视觉树：`Navigation` 只是纯状态机（页栈 + 过渡进度），
// 渲染由 App 消费 `current()` 产出 Scene，过渡期 App 同时渲染 entering/leaving 两页。
// 不依赖 kanesumi-anim —— 过渡进度可由应用层 Sokuou 动画驱动（`set_transition_progress`）。

/// 页切换过渡时长（Metro 标准 0.25s，参 CONTROL_SPEC §10 标准过渡）。
pub const DURATION_PAGE_TRANSITION: f64 = 0.25;

/// 导航状态机。`PageId` 为应用自定义页标识（enum 或 String）。
#[derive(Debug, Clone, PartialEq)]
pub struct Navigation<PageId> {
    /// 页栈：栈顶为当前页。
    stack: Vec<PageId>,
    /// 过渡进度 [0,1]。`0` = 开始切换，`1` = 完成。
    transition: f64,
    /// 过渡期间正在退出的页（`None` = 无进行中的过渡）。
    leaving: Option<PageId>,
}

impl<PageId: Clone + PartialEq> Navigation<PageId> {
    /// 以首页初始化。无过渡。
    pub fn new(initial: PageId) -> Self {
        Self {
            stack: vec![initial],
            transition: 1.0,
            leaving: None,
        }
    }

    /// 当前页（栈顶）。
    pub fn current(&self) -> &PageId {
        self.stack.last().expect("导航栈恒非空")
    }

    /// 是否可以返回上一页（栈长 > 1）。
    pub fn can_go_back(&self) -> bool {
        self.stack.len() > 1
    }

    /// 导航到新页：压栈 + 开始过渡。原页进入 `leaving`。
    pub fn navigate_to(&mut self, page: PageId) {
        if self.current() == &page {
            return;
        }
        self.leaving = self.stack.last().cloned();
        self.stack.push(page);
        self.transition = 0.0;
    }

    /// 返回上一页。不可返回时无操作。
    pub fn go_back(&mut self) -> bool {
        if !self.can_go_back() {
            return false;
        }
        self.leaving = self.stack.pop();
        self.transition = 0.0;
        true
    }

    /// 过渡进度。
    pub fn transition_progress(&self) -> f64 {
        self.transition
    }

    /// 由应用层动画驱动过渡进度（Sokuou `Progress` 输出）。完成时清除 `leaving`。
    pub fn set_transition_progress(&mut self, p: f64) {
        self.transition = p.clamp(0.0, 1.0);
        if self.transition >= 1.0 {
            self.leaving = None;
        }
    }

    /// 过渡期间正在退出的页（`None` = 无过渡）。
    pub fn leaving_page(&self) -> Option<&PageId> {
        self.leaving.as_ref()
    }

    /// 是否正在过渡。
    pub fn is_transitioning(&self) -> bool {
        self.leaving.is_some()
    }

    /// 线性推进过渡（无缓动的简化路径）。应用层可用 Sokuou 驱动以获得缓动。
    pub fn advance(&mut self, dt: f64) {
        if self.is_transitioning() {
            let next = self.transition + dt / DURATION_PAGE_TRANSITION;
            self.set_transition_progress(next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum P {
        Home,
        Detail,
        Settings,
    }

    #[test]
    fn starts_on_initial_without_transition() {
        let nav = Navigation::new(P::Home);
        assert_eq!(*nav.current(), P::Home);
        assert!(!nav.is_transitioning());
        assert_eq!(nav.transition_progress(), 1.0);
    }

    #[test]
    fn navigate_pushes_and_starts_transition() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Detail);
        assert_eq!(*nav.current(), P::Detail);
        assert!(nav.is_transitioning());
        assert_eq!(*nav.leaving_page().unwrap(), P::Home);
        assert_eq!(nav.transition_progress(), 0.0);
    }

    #[test]
    fn navigating_to_same_page_is_noop() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Home);
        assert_eq!(nav.stack.len(), 1);
        assert!(!nav.is_transitioning());
    }

    #[test]
    fn go_back_pops_and_completes() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Detail);
        assert!(nav.can_go_back());
        assert!(nav.go_back());
        assert_eq!(*nav.current(), P::Home);
        assert!(!nav.can_go_back());
        // 回到栈底后 leave 亦完成
        nav.set_transition_progress(1.0);
        assert!(!nav.is_transitioning());
    }

    #[test]
    fn go_back_on_single_page_is_noop() {
        let mut nav = Navigation::new(P::Home);
        assert!(!nav.go_back());
        assert_eq!(nav.stack.len(), 1);
    }

    #[test]
    fn completion_clears_leaving() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Settings);
        assert!(nav.is_transitioning());
        nav.set_transition_progress(0.5);
        assert!(nav.is_transitioning());
        nav.set_transition_progress(1.0);
        assert!(!nav.is_transitioning());
        assert!(nav.leaving_page().is_none());
    }

    #[test]
    fn advance_linear_reaches_completion() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Detail);
        nav.advance(DURATION_PAGE_TRANSITION / 2.0);
        let mid = nav.transition_progress();
        assert!(mid > 0.0 && mid < 1.0);
        nav.advance(DURATION_PAGE_TRANSITION);
        assert_eq!(nav.transition_progress(), 1.0);
        assert!(!nav.is_transitioning());
    }

    #[test]
    fn back_stack_is_lifo() {
        let mut nav = Navigation::new(P::Home);
        nav.navigate_to(P::Detail);
        nav.navigate_to(P::Settings);
        nav.set_transition_progress(1.0);
        assert_eq!(*nav.current(), P::Settings);
        nav.go_back();
        nav.set_transition_progress(1.0);
        assert_eq!(*nav.current(), P::Detail);
        nav.go_back();
        nav.set_transition_progress(1.0);
        assert_eq!(*nav.current(), P::Home);
    }
}
