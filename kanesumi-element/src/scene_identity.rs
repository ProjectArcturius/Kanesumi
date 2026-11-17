// Scene 的身份旁表。参 docs/SCENE_IDENTITY.md；不改变 Scene 的旧字面量结构。
use crate::WidgetId;
use kanesumi_canvas::{Scene, SceneCommand};

/// 绘制命令的稳定节点身份。内容或几何变化不改变键，arena 槽复用由 generation 区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SceneCommandKey {
    owner: u64,
    node: WidgetId,
    phase: Phase,
    slot: usize,
}
impl SceneCommandKey {
    /// 拥有该命令的树节点（包含 generation）。
    pub fn widget_id(self) -> WidgetId {
        self.node
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Phase {
    Paint,
    After,
    ClipStart,
    ClipEnd,
    Focus,
    Scrim,
}

pub(crate) fn mark(
    keys: Option<&mut Vec<SceneCommandKey>>,
    owner: u64,
    node: WidgetId,
    phase: Phase,
    count: usize,
) {
    if let Some(keys) = keys {
        keys.extend((0..count).map(|slot| SceneCommandKey {
            owner,
            node,
            phase,
            slot,
        }));
    }
}

pub(crate) fn append(
    scene: &mut Scene,
    keys: Option<&mut Vec<SceneCommandKey>>,
    owner: u64,
    node: WidgetId,
    phase: Phase,
    commands: &[SceneCommand],
) {
    scene.commands.extend(commands.iter().cloned());
    mark(keys, owner, node, phase, commands.len());
}

// 不同 Tree 的 arena 可以产生同一个 WidgetId，汇聚命令时必须保留其所有者。
pub(crate) fn next_owner() -> u64 {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}
