// key_repeat.rs —— 客户端侧按键重复调度（参 docs/INTERACTION_CANON.md §Ⅳ「按键重复」）。
//
// Wayland 客户端侧重复：外壳从 `wl_keyboard.repeat_info`（合成器按
// `~/.config/ether/input.toml` 下发的正典值，缺省延迟 500 ms / 速率 30 次每秒）读取
// 重复参数；按下一个**会重复**的键后延迟 `delay` 发第一次重复，其后每 `1000/rate` ms
// 一次。`rate == 0` = 关闭重复（协议语义）。
//
// 设计要点（任务 k-key-repeat §设计 2/3）：
// - 同一时刻只重复**最后按下**的键；松开该键 / 按下另一键 / 失焦（leave）即停止；
// - **不积压**：时间以调用方传入的单调时钟为准；若主循环卡顿错过多次，唤醒后最多补发
//   1 次，下一次从「现在」重新起算（杜绝卡顿后一次性打出一串）；
// - 纯状态机 + 调用方给时钟，便于单测（外壳 / Ceyboard 共用同一实现）。

/// 重复参数：延迟（毫秒）+ 速率（次/秒）。`rate == 0` = 关闭。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RepeatConfig {
    /// 按下到第一次重复的延迟（毫秒）。
    pub delay_ms: u32,
    /// 重复速率（次/秒）；0 = 关闭。
    pub rate: u32,
}

impl RepeatConfig {
    pub const fn new(delay_ms: u32, rate: u32) -> Self {
        Self { delay_ms, rate }
    }

    /// 是否启用重复（rate > 0）。
    pub const fn enabled(&self) -> bool {
        self.rate > 0
    }

    /// 两次重复之间的间隔（秒）；rate 0 → 0。
    pub fn interval_secs(&self) -> f64 {
        if self.rate == 0 {
            0.0
        } else {
            1.0 / self.rate as f64
        }
    }
}

/// 按键重复调度器（纯状态机）。
///
/// 键用 `u64` 标识（外壳用原始 keycode；Ceyboard 用 `key_id`）。
#[derive(Debug, Clone)]
pub struct KeyRepeat {
    config: RepeatConfig,
    /// 当前正在重复的键标识（None = 无）。
    held: Option<u64>,
    /// 下一次该发重复的时刻（秒，与调用方时钟同轴）。
    next_at: Option<f64>,
}

impl KeyRepeat {
    pub fn new(config: RepeatConfig) -> Self {
        Self {
            config,
            held: None,
            next_at: None,
        }
    }

    /// 更新重复参数（合成器 `repeat_info` 变化时）。关闭重复则立即停止。
    pub fn set_config(&mut self, config: RepeatConfig) {
        self.config = config;
        if !config.enabled() {
            self.cancel();
        }
    }

    pub fn config(&self) -> RepeatConfig {
        self.config
    }

    /// 当前重复键标识。
    pub fn held(&self) -> Option<u64> {
        self.held
    }

    /// 按下 `id`：`repeatable` 为假或重复关闭 → 清空；否则记录为当前重复键并安排首次重复。
    /// 按下**另一键**即替换当前键（同刻只有最后按下的键在重复）。
    pub fn press(&mut self, id: u64, now: f64, repeatable: bool) {
        if !self.config.enabled() || !repeatable {
            self.cancel();
            return;
        }
        self.held = Some(id);
        self.next_at = Some(now + self.config.delay_ms as f64 / 1000.0);
    }

    /// 松开 `id`：仅当松开的正是当前重复键时停止（松开别的键不影响）。
    pub fn release(&mut self, id: u64) {
        if self.held == Some(id) {
            self.cancel();
        }
    }

    /// 停止重复（换键非重复 / 失焦 / 关闭）。
    pub fn cancel(&mut self) {
        self.held = None;
        self.next_at = None;
    }

    /// 距下一次重复的秒数（None = 无重复）。供主循环安排事件循环超时。
    pub fn next_wake(&self, now: f64) -> Option<f64> {
        self.next_at.map(|t| (t - now).max(0.0))
    }

    /// 到点则返回当前重复键，并把下一次重排到 `now + interval`（**不积压**：不追赶错过的
    /// 多次，卡顿后至多补发一次且从「现在」重新起算）；未到点返回 None。
    pub fn poll(&mut self, now: f64) -> Option<u64> {
        let next = self.next_at?;
        if now < next {
            return None;
        }
        let id = self.held?;
        self.next_at = Some(now + self.config.interval_secs());
        Some(id)
    }
}

/// xkb `key_repeats` 不可用（keymap 未建立）时的回退：修饰键 / 锁定键不重复，其余可重复。
pub fn default_repeatable(key: &kanesumi_element::Key) -> bool {
    match key {
        kanesumi_element::Key::Unknown(ks) => !is_modifier_keysym(*ks),
        _ => true,
    }
}

/// 是否修饰 / 锁定键的 keysym（xkb 判定不可重复的一族；值取自 xkeysym）。
fn is_modifier_keysym(ks: u32) -> bool {
    matches!(
        ks,
        0xffe1 | 0xffe2 // Shift_L / Shift_R
            | 0xffe3 | 0xffe4 // Control_L / Control_R
            | 0xffe5 // Caps_Lock
            | 0xffe7 | 0xffe8 // Meta_L / Meta_R
            | 0xffe9 | 0xffea // Alt_L / Alt_R
            | 0xffeb | 0xffec // Super_L / Super_R
            | 0xffed | 0xffee // Hyper_L / Hyper_R
            | 0xff7f // Num_Lock
            | 0xff14 // Scroll_Lock
            | 0xfe03 // ISO_Level3_Shift
    )
}

/// `Key` → 稳定 `u64` 标识（Ceyboard 自重复用；同一逻辑键恒定）。
pub fn key_id(key: &kanesumi_element::Key) -> u64 {
    match key {
        kanesumi_element::Key::Char(c) => 0x1000_0000 | (*c as u64),
        kanesumi_element::Key::Unknown(ks) => 0x2000_0000 | (*ks as u64),
        other => {
            // 具名变体（Backspace / 方向 / Enter / Tab / Esc / F(n) 等）按其 Debug 名做稳定哈希。
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{other:?}").hash(&mut h);
            0x4000_0000 | (h.finish() & 0x0fff_ffff)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CFG: RepeatConfig = RepeatConfig::new(500, 30);

    /// 模拟主循环：按 `next_wake` 推进到点并 poll，统计 `limit` 秒内的重复次数。
    fn run_for(r: &mut KeyRepeat, mut now: f64, limit: f64) -> (u32, f64) {
        let mut count = 0;
        loop {
            let Some(dt) = r.next_wake(now) else {
                break;
            };
            now += dt;
            if now > limit + 1e-9 {
                break;
            }
            if r.poll(now).is_some() {
                count += 1;
            } else {
                break;
            }
        }
        (count, now)
    }

    #[test]
    fn delay_500_rate_30_hold_2000_yields_46() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        let (count, _) = run_for(&mut r, 0.0, 2.0);
        assert_eq!(count, 46, "⌊(2000−500)·30/1000⌋+1 = 46");
    }

    #[test]
    fn rate_zero_disables() {
        let mut r = KeyRepeat::new(RepeatConfig::new(500, 0));
        r.press(1, 0.0, true);
        assert_eq!(r.held(), None);
        assert_eq!(run_for(&mut r, 0.0, 2.0).0, 0);
    }

    #[test]
    fn non_repeatable_press_clears() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        assert_eq!(r.held(), Some(1));
        r.press(2, 0.1, false); // 按下修饰键等不可重复键 → 立即停止
        assert_eq!(r.held(), None);
    }

    #[test]
    fn only_last_pressed_key_repeats() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        r.press(2, 0.1, true);
        assert_eq!(r.held(), Some(2));
        assert_eq!(r.poll(0.7), Some(2));
    }

    #[test]
    fn release_stops_only_matching_key() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        r.release(9); // 松开别的键不影响
        assert_eq!(r.held(), Some(1));
        r.release(1);
        assert_eq!(r.held(), None);
        assert_eq!(r.poll(5.0), None);
    }

    #[test]
    fn cancel_stops() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        r.cancel();
        assert_eq!(r.next_wake(0.0), None);
    }

    /// 卡顿 300 ms：错过多次只补 1 次，之后间隔恢复 33.3 ms。
    #[test]
    fn stall_catches_up_at_most_once() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        // 到点：0.5s 发第一次。
        assert_eq!(r.poll(0.5), Some(1));
        // 卡顿到 1.1s（本应已发约 18 次）→ 只补一次。
        assert_eq!(r.poll(1.1), Some(1));
        // 紧接着再次 poll 不再补发（下一次从 1.1 起算 + 33.3ms）。
        assert_eq!(r.poll(1.1), None);
        let gap = r.next_wake(1.1).unwrap();
        assert!((gap - 0.033_333).abs() < 1e-4, "间隔恢复 33.3ms，实得 {gap}");
        assert_eq!(r.poll(1.1 + gap), Some(1));
    }

    #[test]
    fn set_config_disable_stops_and_enable_preserves_none() {
        let mut r = KeyRepeat::new(CFG);
        r.press(1, 0.0, true);
        r.set_config(RepeatConfig::new(500, 0));
        assert_eq!(r.held(), None);
    }

    #[test]
    fn key_id_distinct_for_named_variants() {
        use kanesumi_element::Key;
        let a = key_id(&Key::Backspace);
        let b = key_id(&Key::Enter);
        assert_ne!(a, b, "不同具名键的 id 必须不同");
        assert_eq!(a, key_id(&Key::Backspace), "相同键 id 稳定");
    }
}
