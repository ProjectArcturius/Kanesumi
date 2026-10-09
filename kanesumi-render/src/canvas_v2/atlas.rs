// canvas_v2/atlas.rs —— 图集货架式分配器（纯逻辑，无 wgpu 依赖）。
//
// 参 Ether docs/CANVAS_PLAN.md §Ⅲ-3（资源长驻）、任务 c1-canvas-core：字形走 R8 图集、
// 图片走 RGBA8 图集，货架（shelf）打包 —— 一行货架容纳同高段，放不下开新货架。
// 每条目外框留 1 px 空隙防相邻渗色；uv 由调用方内缩半 texel 对齐 texel 中心（1:1
// 采样时与「每字形一张纹理」的 clamp+linear 逐像素一致）。

/// 单条货架：页内的一行，高度固定，x 从左往右推进。
#[derive(Debug)]
struct Shelf {
    /// 货架顶边（页内 y）。
    y: u32,
    /// 货架高（已容纳的最大条目外框高）。
    h: u32,
    /// 已占用到的 x（下一条目左外框从此处起）。
    next_x: u32,
}

/// 一张图集页的分配器。坐标为页内 texel；`alloc` 返回**条目**左上角
/// （已扣除 1 px 空隙边距），条目尺寸即请求的 w×h。
#[derive(Debug)]
pub struct ShelfAtlas {
    width: u32,
    height: u32,
    shelves: Vec<Shelf>,
    /// 已分配外框面积（字节估算 / 测试断言用）。
    used_px2: u64,
}

/// 条目间空隙（防图集线性采样的相邻渗色）。
pub const PAD: u32 = 1;

impl ShelfAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            shelves: Vec::new(),
            used_px2: 0,
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// 页内已占用的 texel 数（含空隙），诊断日志用。
    pub fn used_px2(&self) -> u64 {
        self.used_px2
    }

    /// 分配一个 w×h 条目，返回条目左上 texel（页内）。
    ///
    /// 货架复用规则：最后一个货架装得下（外框宽不超页宽、外框高不超货架高）就续排；
    /// 否则新开货架（y = 上一货架底边）。页底放不下 → `None`（调用方决定升级页尺寸
    /// 或走退路）。空尺寸与超页尺寸同样返回 `None`。
    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 {
            return None;
        }
        // 外框（含两侧各 1 px 空隙）。
        let (fw, fh) = (w + 2 * PAD, h + 2 * PAD);
        if fw > self.width || fh > self.height {
            return None;
        }
        if let Some(last) = self.shelves.last_mut()
            && last.next_x + fw <= self.width
            && fh <= last.h
        {
            let (x, y) = (last.next_x + PAD, last.y + PAD);
            last.next_x += fw;
            self.used_px2 += fw as u64 * last.h as u64;
            return Some((x, y));
        }
        // 新货架：紧贴上一货架底边。
        let y = self
            .shelves
            .last()
            .map_or(0, |s| s.y + s.h);
        if y + fh > self.height {
            return None;
        }
        self.shelves.push(Shelf {
            y,
            h: fh,
            next_x: fw,
        });
        self.used_px2 += fw as u64 * fh as u64;
        Some((PAD, y + PAD))
    }

    /// 清空（页尺寸升级重建时用：调用方负责重新上传全部条目）。
    pub fn reset(&mut self) {
        self.shelves.clear();
        self.used_px2 = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 同层同高条目横向连排() {
        let mut a = ShelfAtlas::new(64, 64);
        let p0 = a.alloc(10, 10).expect("首个条目应可分配");
        let p1 = a.alloc(10, 10).expect("同高续排");
        assert_eq!(p0, (1, 1), "首条目在空隙内偏移 (1,1)");
        assert_eq!(p1.0, p0.0 + 12, "外框宽 12 连排");
        assert_eq!(p1.1, p0.1, "同一货架同行");
    }

    #[test]
    fn 放不下开新货架_页满返回_none() {
        let mut a = ShelfAtlas::new(64, 32);
        let first = a.alloc(28, 8).expect("第一货架");
        assert_eq!(first, (1, 1));
        let second = a.alloc(28, 8).expect("同高续排");
        assert_eq!(second, (31, 1), "同货架横向续排（外框 30 宽）");
        // 更高的条目 → 新货架 y = 10，条目含上空隙 → (1, 11)。
        let third = a.alloc(28, 9).expect("第二货架");
        assert_eq!(third, (1, 11), "新货架从上一货架底边起");
        // 同高续排第二货架。
        let fourth = a.alloc(28, 9).expect("第二货架续排");
        assert_eq!(fourth, (31, 11));
        // 第三货架 y = 21（外框高 11，21+11 = 32 恰好放满页底）。
        let fifth = a.alloc(28, 9).expect("第三货架");
        assert_eq!(fifth, (1, 22));
        // 第三货架还能横向续排一格，之后才真正页满。
        let sixth = a.alloc(28, 9).expect("第三货架续排");
        assert_eq!(sixth, (31, 22));
        // 再开货架超页底 → None。
        assert!(a.alloc(28, 9).is_none(), "页满应返回 None");
    }

    #[test]
    fn 宽度超页与零尺寸_超大条目拒绝() {
        let mut a = ShelfAtlas::new(32, 32);
        assert!(a.alloc(33, 4).is_none(), "外框宽超页宽");
        assert!(a.alloc(4, 33).is_none(), "外框高超页高");
        assert!(a.alloc(0, 4).is_none(), "零宽");
        assert!(a.alloc(4, 0).is_none(), "零高");
        // 矮货架不因续排被撑高：货架高 10 装不下外框高 12 的条目，
        // 而页高 21 也开不下第二货架（10 + 12 = 22 > 21）。
        let mut b = ShelfAtlas::new(32, 21);
        assert!(b.alloc(28, 8).is_some());
        assert!(b.alloc(28, 10).is_none(), "已开货架高不够且页内无第二货架空间时失败");
    }

    #[test]
    fn reset_清空可重新分配() {
        let mut a = ShelfAtlas::new(16, 16);
        assert!(a.alloc(14, 14).is_some());
        assert!(a.alloc(14, 14).is_none(), "满");
        a.reset();
        assert_eq!(a.used_px2(), 0);
        assert!(a.alloc(14, 14).is_some(), "重置后可重新分配");
    }

    #[test]
    fn used_px2_随分配增长() {
        let mut a = ShelfAtlas::new(64, 64);
        a.alloc(10, 10); // 外框 12×12
        assert_eq!(a.used_px2(), 144);
        a.alloc(10, 10);
        assert_eq!(a.used_px2(), 288);
    }
}
