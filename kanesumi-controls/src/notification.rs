// notification.rs —— 通知队列的**纯状态模型**（Win10 语义）。参 docs/CONTROL_SPEC.md「通知」、
// Ether `docs/XDG_PLAN.md`（org.freedesktop.Notifications）、`docs/supply/BEHAVIOR_CORPUS.md`（状态转录）。
//
// 为什么先做模型、不做卡片：供应门 §Ⅰ 判定 Kanesumi 对「通知」这一格 8 项「应用自己顶」，
// 合成器侧 `shell/notifications.rs` 2369 生产行里 11 个手搓函数（换行 / 适配 / 描边 / 混合 /
// 命中 / 布局）全在重造框架已有的能力。模型（超时、悬停暂停、同应用合并、上限淘汰）是
// **语义最容易错、又最不依赖渲染**的一块；先把语义钉死并用状态转录测住，卡片渲染随后接
// `MetroNotificationCard`（复用 `TextEngine::layout_box` 的两行截断与省略号）。
//
// 语义取 Win10（一手来源：`docs/INTERACTION_CANON.md` §通知、UWP Toast 行为）：
// - 自动消失计时**悬停时暂停**，指针离开后从剩余时长继续（不是重置）；
// - 同一个应用的连续通知**合并**成一条并把计数 +1（不堆成多条）；
// - 可见条数有上限，超出时**最早的那条**被挤掉；
// - `timeout_ms == 0` 表示常驻（不自动消失）。

use std::collections::VecDeque;

/// 一条通知的呈现状态。合并计数与超时都挂在这条记录上。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationState {
    /// 入场动画中（外壳负责绘制）。
    Appearing,
    /// 在场，计时进行中。
    Shown,
    /// 指针悬停：计时暂停。
    Hovered,
    /// 退场动画中。
    Dismissing,
}

/// 一条通知。字段公开，外壳直接读去绘制；语义变化一律经 [`NotificationStack`]。
#[derive(Debug, Clone)]
pub struct Notification {
    /// 稳定 id（同一应用合并后 id 不变）。
    pub id: u64,
    /// 应用标识（D-Bus 的 `app_id` / desktop id）。
    pub app_id: String,
    /// 应用显示名（标题行的副标）。
    pub app_name: String,
    /// 标题。
    pub title: String,
    /// 正文（可能多行；换行与截断由渲染侧用 `TextEngine::layout_box` 做，模型不切）。
    pub body: String,
    /// 动作按钮标签（空 = 无动作）。
    pub actions: Vec<String>,
    /// 同应用合并计数（≥1）。
    pub count: u32,
    /// 剩余自动消失时长（毫秒）；`0.0` = 常驻。
    pub remaining_ms: f64,
    /// 状态。
    pub state: NotificationState,
}

impl Notification {
    /// 是否常驻（不自动消失）。
    pub fn persistent(&self) -> bool {
        self.remaining_ms <= 0.0 && self.state != NotificationState::Dismissing
    }
}

/// 一帧里发生的事（外壳据此排动画 / 播声音 / 记历史）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationEvent {
    /// 到期自动消失，已从队列移除。
    Expired(u64),
    /// 被主动关闭（含用户点关闭 / 点动作），已从队列移除。
    Dismissed(u64),
}

/// 退场动画时长：这段时间内条目仍在 `visible()` 里，但不再计时、不再接受动作。
pub const DISMISS_MS: f64 = 150.0;

/// 通知队列。**只有可见的那几条**在结构里；历史上限按 `max_visible` 淘汰最旧的。
#[derive(Debug, Clone)]
pub struct NotificationStack {
    items: VecDeque<Notification>,
    max_visible: usize,
    next_id: u64,
    dismiss_left_ms: Vec<(u64, f64)>,
}

impl Default for NotificationStack {
    fn default() -> Self {
        Self::new(3)
    }
}

impl NotificationStack {
    /// Win10 默认同时可见 3 条。
    pub fn new(max_visible: usize) -> Self {
        Self {
            items: VecDeque::new(),
            max_visible: max_visible.max(1),
            next_id: 1,
            dismiss_left_ms: Vec::new(),
        }
    }

    /// 可见条数上限。
    pub fn max_visible(&self) -> usize {
        self.max_visible
    }

    /// 当前条数（含退场中的）。
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// 按队列顺序（最旧在前）读。
    pub fn visible(&self) -> impl Iterator<Item = &Notification> {
        self.items.iter()
    }

    /// 按 id 取。
    pub fn get(&self, id: u64) -> Option<&Notification> {
        self.items.iter().find(|n| n.id == id)
    }

    /// 入栈。与**最后一条同 `app_id`** 的通知合并（计数 +1、内容替换、剩余时长重置）；
    /// 否则新开一条。返回条目 id。超出上限时最旧的被挤掉。
    pub fn push(
        &mut self,
        app_id: impl Into<String>,
        app_name: impl Into<String>,
        title: impl Into<String>,
        body: impl Into<String>,
        actions: Vec<String>,
        timeout_ms: f64,
    ) -> u64 {
        let app_id = app_id.into();
        if let Some(last) = self.items.back_mut()
            && last.app_id == app_id
            && last.state != NotificationState::Dismissing
        {
            last.count += 1;
            last.app_name = app_name.into();
            last.title = title.into();
            last.body = body.into();
            last.actions = actions;
            last.remaining_ms = timeout_ms.max(0.0);
            last.state = NotificationState::Appearing;
            self.dismiss_left_ms.retain(|(i, _)| *i != last.id);
            let id = last.id;
            self.trim();
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        self.items.push_back(Notification {
            id,
            app_id,
            app_name: app_name.into(),
            title: title.into(),
            body: body.into(),
            actions,
            count: 1,
            remaining_ms: timeout_ms.max(0.0),
            state: NotificationState::Appearing,
        });
        self.trim();
        id
    }

    /// 入场动画播完：`Appearing → Shown`。
    pub fn shown(&mut self, id: u64) {
        if let Some(n) = self.items.iter_mut().find(|n| n.id == id)
            && n.state == NotificationState::Appearing
        {
            n.state = if n.state == NotificationState::Hovered {
                NotificationState::Hovered
            } else {
                NotificationState::Shown
            };
        }
    }

    /// 悬停切换。悬停中的条目暂停计时；离开后从**剩余时长**继续。
    pub fn set_hover(&mut self, id: Option<u64>) {
        for n in self.items.iter_mut() {
            match (Some(n.id) == id, n.state) {
                (true, NotificationState::Shown) | (true, NotificationState::Appearing) => {
                    n.state = NotificationState::Hovered;
                }
                (false, NotificationState::Hovered) => {
                    n.state = NotificationState::Shown;
                }
                _ => {}
            }
        }
    }

    /// 主动关闭：进入退场，`DISMISS_MS` 后移除并产出 `Dismissed`。
    pub fn dismiss(&mut self, id: u64) -> bool {
        let Some(n) = self.items.iter_mut().find(|n| n.id == id) else {
            return false;
        };
        if n.state == NotificationState::Dismissing {
            return false;
        }
        n.state = NotificationState::Dismissing;
        self.dismiss_left_ms.push((id, DISMISS_MS));
        true
    }

    /// 立即移除（不做退场动画；外壳同步重排时用）。
    pub fn remove(&mut self, id: u64) -> bool {
        self.dismiss_left_ms.retain(|(i, _)| *i != id);
        let before = self.items.len();
        self.items.retain(|n| n.id != id);
        self.items.len() != before
    }

    /// 推进一帧。返回本帧发生的事件（到期 / 关闭完成）。
    pub fn tick(&mut self, dt_ms: f64) -> Vec<NotificationEvent> {
        let dt = dt_ms.max(0.0);
        let mut events = Vec::new();

        // 1. 退场推进
        let mut still = Vec::with_capacity(self.dismiss_left_ms.len());
        for (id, left) in self.dismiss_left_ms.drain(..) {
            let remain = left - dt;
            if remain > 0.0 {
                still.push((id, remain));
            } else {
                self.items.retain(|n| n.id != id);
                events.push(NotificationEvent::Dismissed(id));
            }
        }
        self.dismiss_left_ms = still;

        // 2. 计时（悬停 / 退场 / 常驻不计）
        let mut expired = Vec::new();
        for n in self.items.iter_mut() {
            if n.state != NotificationState::Shown || n.remaining_ms <= 0.0 {
                continue;
            }
            n.remaining_ms -= dt;
            if n.remaining_ms <= 0.0 {
                expired.push(n.id);
            }
        }
        for id in expired {
            self.items.retain(|n| n.id != id);
            events.push(NotificationEvent::Expired(id));
        }
        events
    }

    /// 超出上限：挤掉最旧的（退场中的也一并清）。
    fn trim(&mut self) {
        while self.items.len() > self.max_visible {
            if let Some(dropped) = self.items.pop_front() {
                self.dismiss_left_ms.retain(|(i, _)| *i != dropped.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn push(stack: &mut NotificationStack, app: &str, timeout: f64) -> u64 {
        stack.push(app, app, "标题", "正文", Vec::new(), timeout)
    }

    #[test]
    fn push_adds_one_visible() {
        let mut s = NotificationStack::new(3);
        push(&mut s, "a", 5000.0);
        assert_eq!(s.len(), 1);
        assert_eq!(s.visible().next().unwrap().count, 1);
    }

    #[test]
    fn same_app_merges_and_counts() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "mail", 5000.0);
        let again = push(&mut s, "mail", 5000.0);
        assert_eq!(id, again, "同应用合并后 id 不变");
        assert_eq!(s.len(), 1);
        assert_eq!(s.get(id).unwrap().count, 2);
    }

    #[test]
    fn different_app_does_not_merge() {
        let mut s = NotificationStack::new(3);
        push(&mut s, "mail", 5000.0);
        push(&mut s, "chat", 5000.0);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn merge_does_not_happen_across_trailing_app() {
        let mut s = NotificationStack::new(3);
        push(&mut s, "mail", 5000.0);
        push(&mut s, "chat", 5000.0);
        push(&mut s, "mail", 5000.0);
        assert_eq!(s.len(), 3, "只与最后一条同应用合并");
    }

    #[test]
    fn exceeding_max_evicts_oldest() {
        let mut s = NotificationStack::new(2);
        let a = push(&mut s, "a", 5000.0);
        push(&mut s, "b", 5000.0);
        push(&mut s, "c", 5000.0);
        assert_eq!(s.len(), 2);
        assert!(s.get(a).is_none(), "最早的一条被挤掉");
        let ids: Vec<u64> = s.visible().map(|n| n.id).collect();
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn tick_expires_after_timeout() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 100.0);
        s.shown(id);
        assert!(s.tick(50.0).is_empty(), "未到期不出事件");
        let ev = s.tick(60.0);
        assert_eq!(ev, vec![NotificationEvent::Expired(id)]);
        assert!(s.is_empty());
    }

    #[test]
    fn appearing_does_not_count_down() {
        let mut s = NotificationStack::new(3);
        push(&mut s, "a", 100.0); // 仍在 Appearing
        assert!(s.tick(500.0).is_empty(), "入场动画期间不计时");
        assert_eq!(s.len(), 1);
    }

    #[test]
    fn hover_pauses_and_resumes_from_remaining() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 100.0);
        s.shown(id);
        s.tick(40.0);
        s.set_hover(Some(id));
        assert_eq!(s.get(id).unwrap().state, NotificationState::Hovered);
        assert!(s.tick(1000.0).is_empty(), "悬停期间不消失");
        s.set_hover(None);
        assert!(s.tick(50.0).is_empty(), "从剩余 60 ms 继续，未到期");
        let ev = s.tick(20.0);
        assert_eq!(ev, vec![NotificationEvent::Expired(id)], "剩 60 − 50 − 20 < 0");
    }

    #[test]
    fn persistent_never_expires() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 0.0);
        s.shown(id);
        assert!(s.tick(1_000_000.0).is_empty(), "timeout 0 = 常驻");
        assert!(s.get(id).unwrap().persistent());
    }

    #[test]
    fn dismiss_runs_exit_then_removes() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 0.0);
        s.shown(id);
        assert!(s.dismiss(id));
        assert_eq!(s.get(id).unwrap().state, NotificationState::Dismissing);
        assert!(s.tick(DISMISS_MS - 1.0).is_empty());
        let ev = s.tick(2.0);
        assert_eq!(ev, vec![NotificationEvent::Dismissed(id)]);
        assert!(s.is_empty());
    }

    #[test]
    fn remove_is_immediate_without_event() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 0.0);
        assert!(s.remove(id));
        assert!(s.is_empty());
        assert!(s.tick(100.0).is_empty());
    }

    #[test]
    fn order_is_oldest_first() {
        let mut s = NotificationStack::new(3);
        let a = push(&mut s, "a", 0.0);
        let b = push(&mut s, "b", 0.0);
        let c = push(&mut s, "c", 0.0);
        let ids: Vec<u64> = s.visible().map(|n| n.id).collect();
        assert_eq!(ids, vec![a, b, c]);
    }

    #[test]
    fn dismissing_entry_does_not_merge() {
        let mut s = NotificationStack::new(3);
        let id = push(&mut s, "a", 0.0);
        s.shown(id);
        s.dismiss(id);
        let new_id = push(&mut s, "a", 0.0);
        assert_ne!(new_id, id, "退场中的条目不再参与合并");
    }

    #[test]
    fn max_visible_is_at_least_one() {
        let s = NotificationStack::new(0);
        assert_eq!(s.max_visible(), 1);
    }
}
