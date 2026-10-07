// kanesumi-element —— 框架持有的保留元素树。参 docs/ELEMENT_TREE.md。
//
// 对应 XAML 的 UIElement / FrameworkElement 层：布局（带缓存与失效）、绘制缓存与损伤、
// 命中与路由事件、指针捕获、焦点与 Tab 顺序、覆盖层（PopupRoot）、视觉状态。
// 控件只实现 `Widget`（量测 / 排列 / 自绘 / 事件），「在哪、点没点到、谁有焦点、
// 哪里要重画」全部由本 crate 负责 —— 这是 Kanesumi 自绘路线必须自己提供的那一层。
//
// 纯逻辑、跨平台、不依赖 harness：外壳由 harness 的 `TreeHost` 接入（参 ELEMENT_TREE §Ⅷ）。

pub mod event;
pub mod id;
pub mod ime;
pub mod layer;
pub mod props;
pub mod testing;
pub mod tree;
pub mod visual_state;
pub mod widget;
pub mod widgets;

pub use event::{Event, Key, Modifiers, PointerButton, ScrollInput, ScrollPhase, ScrollSource};
pub use id::WidgetId;
pub use ime::{ImeContentHint, ImeContext};
pub use layer::{LayerAnimSpec, LayerOp};
pub use props::{Align, Insets, LayoutProps};
pub use tree::{Action, EditCtx, FrameOutput, PopupDismissed, PopupSide, PopupSpec, Tree};
pub use visual_state::VisualState;
pub use widget::{
    AccessInfo, AccessRole, ArrangeCtx, ControlStates, EventCtx, MeasureCtx, PaintCtx, RealizeCtx,
    UpdateCtx, Widget,
};
