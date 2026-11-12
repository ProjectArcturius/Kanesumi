// 导航类控件：把「一排放什么、摆在哪、点哪里」从应用外壳里提出来。
//
// 三族纯几何提炼：Dock 轨道（`dock_rail`）、磁贴墙（`tile_wall`）、文件视图
// （`file_views`：列表 / 侧栏 / 路径栏）。输入条目与可用矩形，输出矩形 / 命中区 /
// 裁剪 / 槽位的快照。不持有交互状态，也不画。
// 参 docs/supply/DOCK_RAIL.md、docs/supply/TILE_WALL_FILE_VIEWS.md。

pub mod dock;
pub mod dock_rail;
pub mod file_views;
pub mod tile_wall;

pub use dock::{DockActivated, DockItem, DockItemState, MetroDock, indicator_rect};
pub use dock_rail::{
    DockChipDirection, DockChipLayout, DockDirection, DockHit, DockIndicatorKind, DockLayout,
    DockMetrics, DockRail, DockRailEntry, DockRailInput, DockSlotLayout, DockZone, DockZoneLayout,
};
pub use file_views::{
    FileListColumn, FileListColumnSlot, FileListColumns, FileListInput, FileListItem,
    FileListLayout, FileListMetrics, FileListRowSlot, PathBarInput, PathBarLayout, PathBarMetrics,
    PathBarSegment, PathBarSegmentSlot, SidebarEntry, SidebarInput, SidebarLayout, SidebarMetrics,
    SidebarSlot, file_list_layout, path_bar_layout, sidebar_layout,
};
pub use tile_wall::{
    TileWallGroup, TileWallGroupLayout, TileWallInput, TileWallLayout, TileWallMetrics,
    TileWallSize, TileWallSlot, TileWallTile, layout as tile_wall_layout,
};
