// id.rs —— 元素身份。参 ELEMENT_TREE §Ⅲ.1。
//
// arena 槽位 + 代数：节点删除后槽位复用，旧 id 的代数对不上 → 查询得 None，
// 而不是悄悄指向别的控件（「悬垂 id 静默命中错控件」比 panic 更难查）。

/// 元素 id。跨帧稳定，App 持有它来编辑控件、识别动作来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WidgetId {
    pub(crate) index: u32,
    pub(crate) generation: u32,
}

impl WidgetId {
    pub(crate) fn new(index: usize, generation: u32) -> Self {
        Self {
            index: index as u32,
            generation,
        }
    }

    pub(crate) fn slot(self) -> usize {
        self.index as usize
    }

    /// 稳定的数值身份（无障碍节点 id / 诊断日志用）。
    pub fn to_u64(self) -> u64 {
        ((self.generation as u64) << 32) | self.index as u64
    }
}
