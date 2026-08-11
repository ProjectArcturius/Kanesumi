// kanesumi-calculator —— 首个以 Kanesumi 控件构建的实用应用（runtime 狗粮化探针）。
//
// 纯逻辑（calc.rs）与 UI（app.rs）分离，跨平台可测；Linux 上经 harness 外壳运行。

pub mod calc;
pub mod app;

pub use app::CalculatorApp;
pub use calc::{Calc, Op};
