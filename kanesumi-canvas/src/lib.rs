// Kanesumi（矩隅）· 2D 图形层 —— 对应 UWP Win2D（参 Ether-main PLAN.md §6.1）。
//
// 2026-08-10 拆分判定（触发条件 = harness wgpu 外壳落地）：渲染命令模型（`Scene`）与
// 字体度量（`TextEngine`）从 `kanesumi-core` 迁入本 crate。core 回归纯运行时
// （tokens / 主题 / 排版 / 几何）；本 crate 消费 core 并产出绘制命令。
// 依赖方向：core ← canvas ← controls/harness/gallery。

pub mod scene;
pub mod text;

pub use scene::{Scene, SceneCommand, TextAlign};
pub use text::{Line, TextEngine, TextLoadError};
