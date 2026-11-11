// 导航类控件：把「一排放什么、摆在哪、点哪里」从应用外壳里提出来。
//
// 目前只有 Dock 轨道的纯几何提炼（`dock_rail`）：输入条目与可用矩形，输出
// 矩形 / 命中区 / 双区分界 / 定位线几何的快照。不持有交互状态，也不画。
// 参 docs/supply/DOCK_RAIL.md。

pub mod dock_rail;

pub use dock_rail::{
    DockChipDirection, DockChipLayout, DockDirection, DockHit, DockIndicatorKind, DockLayout,
    DockMetrics, DockRail, DockRailEntry, DockRailInput, DockSlotLayout, DockZone, DockZoneLayout,
};
