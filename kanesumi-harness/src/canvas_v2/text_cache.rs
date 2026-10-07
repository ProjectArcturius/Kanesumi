// canvas_v2/text_cache.rs —— 排版结果 LRU 缓存（CanvasV2 专用）。
//
// 参任务 c1-canvas-core 设计 2：旧渲染器每帧对每段 `SceneCommand::Text` 重新排版
// （`emit_text` → `layout_text_glyphs`），静态 UI 一帧内重复排版同一段文本。
// 本缓存按「内容 + 样式 + 文本框尺寸 + 对齐 + 换行 + 行数上限 + 溢出策略 + scale +
// 引擎」缓存**相对文本框原点**的字形放置；命中则只平移 + 出实例，未命中才排版。
//
// key 用两遍独立 FNV-1a 64（常数错开）拼 128 位 —— 文本缓存键撞上会渲染成别的字，
// 单 64 位哈希在万级条目下概率不可忽略，双哈希到可忽略。

use std::collections::HashMap;
use std::collections::VecDeque;

use crate::glyph_layout::GlyphKey;

/// 相对文本框原点的一个字形放置（逻辑像素）。绝对坐标 = rect.origin + (x, y)。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RelGlyph {
    pub(crate) key: GlyphKey,
    /// 字形位图左上角相对文本框原点（逻辑）。
    pub(crate) x: f32,
    pub(crate) y: f32,
    /// 字形位图尺寸（逻辑）。
    pub(crate) w: f32,
    pub(crate) h: f32,
}

/// 128 位缓存键（双 FNV-1a）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct LayoutKey(u64, u64);

/// FNV-1a 64 位（种子错开做第二遍哈希）。
fn fnv1a64(seed: u64, data: &[u8]) -> u64 {
    let mut h = seed;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 由参与排版的全部输入拼缓存键。`extra` 供调用方拼接追加维度（如调色旋钮版本）。
pub(crate) fn layout_key(parts: &[&[u8]]) -> LayoutKey {
    // 分段长度前缀拼接，防「(“ab”,“c”) 与 (“a”,“bc”) 同串」。
    let mut buf = Vec::new();
    for p in parts {
        buf.extend_from_slice(&(p.len() as u32).to_le_bytes());
        buf.extend_from_slice(p);
    }
    LayoutKey(
        fnv1a64(0xcbf2_9ce4_8422_2325, &buf),
        fnv1a64(0x8422_2325_cbf2_9ce4, &buf),
    )
}

/// 排版 LRU。上限按条目数（任务书定 4096）。
pub(crate) struct TextLayoutCache {
    map: HashMap<LayoutKey, Vec<RelGlyph>>,
    order: VecDeque<LayoutKey>,
    cap: usize,
}

impl TextLayoutCache {
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            cap: cap.max(1),
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    /// 命中取放置列表（并把该键提到最新）。
    pub(crate) fn get(&mut self, key: &LayoutKey) -> Option<&[RelGlyph]> {
        if self.map.contains_key(key) {
            // 提到最新：移除旧位次再压尾。
            if let Some(pos) = self.order.iter().position(|k| *k == *key) {
                self.order.remove(pos);
            }
            self.order.push_back(*key);
            self.map.get(key).map(|v| v.as_slice())
        } else {
            None
        }
    }

    /// 写入（key 已存在则覆盖并视为一次新插入；超上限淘汰最旧）。
    pub fn put(&mut self, key: LayoutKey, glyphs: Vec<RelGlyph>) {
        if self.map.contains_key(&key)
            && let Some(pos) = self.order.iter().position(|k| *k == key)
        {
            self.order.remove(pos);
        }
        while self.map.len() >= self.cap
            && let Some(old) = self.order.pop_front()
        {
            self.map.remove(&old);
        }
        self.order.push_back(key);
        self.map.insert(key, glyphs);
    }

    /// 旋钮 / 引擎变更时整体失效（字形几何或字体变了，旧放置不再有效）。
    pub(crate) fn clear(&mut self) {
        self.map.clear();
        self.order.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rel(x: f32, y: f32) -> RelGlyph {
        RelGlyph {
            key: GlyphKey {
                engine_id: 0,
                font_id: 0,
                glyph_id: 0,
                size_bits: 0,
            },
            x,
            y,
            w: 10.0,
            h: 10.0,
        }
    }

    #[test]
    fn 键由内容与样式区分() {
        let a = layout_key(&["标题".as_bytes(), &14.0f32.to_le_bytes()]);
        let b = layout_key(&["标题".as_bytes(), &16.0f32.to_le_bytes()]);
        let c = layout_key(&["标 题".as_bytes(), &14.0f32.to_le_bytes()]);
        assert_ne!(a, b, "字号不同 → 键不同");
        assert_ne!(a, c, "内容不同 → 键不同");
        // 分段边界不混淆：「ab」+「c」与「a」+「bc」。
        let d = layout_key(&[b"ab", b"c"]);
        let e = layout_key(&[b"a", b"bc"]);
        assert_ne!(d, e);
    }

    #[test]
    fn put_get_往返一致() {
        let mut c = TextLayoutCache::new(4);
        let k = layout_key(&[b"x"]);
        c.put(k, vec![rel(1.0, 2.0), rel(3.0, 4.0)]);
        let got = c.get(&k).expect("应命中");
        assert_eq!(got.len(), 2);
        assert_eq!((got[0].x, got[1].y), (1.0, 4.0));
        assert!(c.get(&layout_key(&[b"y"])).is_none(), "未写入的键不命中");
    }

    #[test]
    fn 超上限淘汰最旧() {
        let mut c = TextLayoutCache::new(2);
        let k0 = layout_key(&[b"0"]);
        let k1 = layout_key(&[b"1"]);
        let k2 = layout_key(&[b"2"]);
        c.put(k0, vec![]);
        c.put(k1, vec![]);
        c.put(k2, vec![]);
        assert_eq!(c.len(), 2, "上限 2");
        assert!(c.get(&k0).is_none(), "最旧被淘汰");
        assert!(c.get(&k1).is_some());
        // get 提升新鲜度：再插一个后 k1 不被淘汰、k2 被淘汰。
        let _ = c.get(&k1);
        let k3 = layout_key(&[b"3"]);
        c.put(k3, vec![]);
        assert!(c.get(&k1).is_some(), "刚访问过的不淘汰");
        assert!(c.get(&k2).is_none(), "未访问的被淘汰");
    }

    #[test]
    fn clear_清空全部() {
        let mut c = TextLayoutCache::new(4);
        let k = layout_key(&[b"x"]);
        c.put(k, vec![rel(0.0, 0.0)]);
        c.clear();
        assert_eq!(c.len(), 0);
        assert!(c.get(&k).is_none());
    }
}
