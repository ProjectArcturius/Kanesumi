# WinRT/WinUI ↔ Kanesumi 能力对照

2026-11-05 · 监工卡 n1104-idioms（交付 1）。对真基线：kanesumi `integration/n1104` @ `e3674ed`
（= a334008 + 首件文档 + e4 五件 + e3 三件）。

方法：只提取**契约**，不复制代码（参各任务卡与 `~/refs/dotnet/` 只读约定）。
参考仓：`~/refs/dotnet/{Ncrust, PezMax-One, WindowsCommunityToolkit, WinUI-Gallery}`；
本仓引用以 kanesumi 仓相对路径。每条结论带 `文件:行号`；查不到出处的一律标「推断」。

用途：E6（AccessKit/AT-SPI 桥）、e5 框架卡（net_panel 入树）、size-tokens 的契约来源索引；
也是 IDIOMS.md 各条「为什么」的对照锚。

| # | 能力 | WinRT/WinUI 提供 | Kanesumi 现状 | 缺口 | 优先级 | 可借契约 |
|---|---|---|---|---|---|---|
| 1 | 依赖属性 / 失效传播 | `DependencyProperty.RegisterAttached`（WCT `Microsoft.Toolkit.Uwp.Input.GazeInteraction/GazeTargetItem.cs:17`）；`UIElement.InvalidateMeasure`（WCT `…DataGrid/DataGridColumns.cs:284`、SampleApp `Shell.xaml.cs:102`）。DP 支撑绑定 / 样式 / 值继承 | 无 DP（§Ⅺ-2 明确不做）。失效三分：`invalidate_measure/arrange/paint`（`kanesumi-element/src/tree.rs:335/338/341`，EditCtx 同款 `tree.rs:328-343`）；属性变更走 `tree.edit`（`tree.rs:767`） + 控件字段 | 无绑定 / 样式字典 / 值继承 | 不做（已裁定 §Ⅺ-2） | 失效粒度三分法与 WinUI Measure/Arrange/Render 三失效一一对应；「帧内失效不被吞」（§Ⅴ-bis.2） |
| 2 | 布局（Measure/Arrange/Constraints） | 两阶段覆写 `MeasureOverride` / `ArrangeOverride`（WCT `…DataGrid/DataGrid.cs:3299/3270`），约束 = `availableSize` | `Widget::measure(available)→desired` / `arrange(rect)`（`kanesumi-element/src/widget.rs:59/64`）；框架 `arrange_child` 结果一律夹紧（`widget.rs:262`）；量测缓存 + 约束命中（`tree.rs` `measure_node`） | 布局期动画语义未裁定（expander 高度动画，§Ⅹ E3 特殊组遗留） | 中 | 「期望尺寸不含 margin」（XAML Margin 语义，`widget.rs:57` 注释）；子矩形 ⊆ 父矩形框架断言（testing.rs `assert_contained`） |
| 3 | 视觉树 | `VisualTreeHelper.GetChildrenCount/GetChild` 遍历（WCT `…UI/Extensions/DependencyObjectExtensions.cs:105`、`…DataGrid/Utilities/ValidationUtil.cs:193`）；逻辑树 / 视觉树双层 | 树即唯一权威结构：`Node`（`tree.rs:272`）、`children`（`tree.rs:690`）、先序 `all_ids`（`tree.rs:2927`）；id 跨帧稳定 + 代数防悬垂（`element/src/id.rs:26` `to_u64`） | 无 | —（本就更强） | 单树同源：命中 / 绘制 / Tab 序 / 无障碍全由同一棵树派生（§Ⅵ「Tab 顺序由树派生」） |
| 4 | 输入 / 焦点 / 指针捕获 | `CapturePointer / ReleasePointerCapture`（WCT `…DataGrid/DataGridCell.cs:407/422`）；`Focus(FocusState.Keyboard)`（WCT SampleApp `Shell.Search.cs:33`） | **按下即捕获**（`tree.rs:1762` `pointer_down`，注释 `tree.rs:1818`）；焦点 `focus(id, keyboard)`（`tree.rs:2048`，keyboard 才画焦点框 = FocusState::Keyboard 对应）；Tab 序树派生（`tree.rs:2127` `focus_order`）+ 焦点组（`element/src/props.rs:182` `FocusGroupSpec`） | 无（e4 已补右键释放时序与弹层焦点恢复，kanesumi `1bd103c`） | 低 | FocusState 键盘/程序二分 → `focus(id, keyboard)`；路由「目标→冒泡→框架兜底」层级（§Ⅵ 表） |
| 5 | 模板 / 视觉状态 | `VisualStateManager.GoToState`（WCT `…InAppNotification/InAppNotification.cs:83/223/312`）；主题字典（WCT SampleApp `Styles/Themes.xaml:23`）；ControlTemplate / 样式字典 | 无模板系统：控件自绘 `paint`，读 `ctx.state()`（`widget.rs:311` `ControlStates` `widget.rs:22`）；状态过渡插值 `VisualState`（`element/src/visual_state.rs:13`）+ Sokuou 真实时钟（§Ⅴ-bis.3） | 无 ControlTemplate / 样式字典（裁定不做，§Ⅺ-2）；换肤改控件（颜色已由 ThemeColor 隔离，`core/src/brush.rs:16`） | 低 | 状态命名与可达性语义（Normal/PointerOver/Pressed/Disabled/Focused）→ IDIOMS 视觉状态名 PascalCase 强制 |
| 6 | 虚拟化 | `ListView` / `ItemsRepeater`（PezMax-One `windows/src/PezMax.App/Pages/HomePage.xaml:68`） | `ItemsRepeater` keyed 回收 + `realize` 钩子（`element/src/widget.rs:104/108`；`controls/src/items_repeater.rs:448` `wants_realize`、`:187` `realized_count`）；`bring_into_view`（`widget.rs:146`） | 横向 / 多布局虚拟化扩展（UniformGridLayout 已有） | 低 | 「从任意帧幂等收敛」（`widget.rs:108` `wants_realize` 契约注释）；回收保焦点（§Ⅳ-bis） |
| 7 | 无障碍（UIA） | AutomationPeer 模式分派 Invoke/Toggle/SelectionItem/ExpandCollapse（WCT `GazeTargetItem.cs:70-82`）；`AutomationProperties.AutomationId`（WinUI-Gallery `WinUIGallery/MainWindow.xaml:78`） | `accessibility()` 接口（`element/src/widget.rs:184`，controls 45 文件实现）；本批 e3：`AccessNode` 快照（`element/src/access.rs:31`）+ 四条契约（`access.rs:194`）+ 入口 `Tree::access_snapshot`（`tree.rs:3562`） | AT-SPI 桥未接（需新依赖，未授权）；菜单族三文件 focusable 而无 accessibility（复审发现：`controls/src/context_menu.rs:221`、`controls/src/dropdown_menu.rs:867`、`controls/src/menu_flyout.rs:236`，三文件 0 处 `fn accessibility`）；`expanded/selected` 数据源未接；弹层不在快照 | **高**（E6 主线） | 模式分派 → AccessKit 事件语义的映射表（Toggle→checked、SelectionItem→selected、ExpandCollapse→expanded，见 access.rs 结构体注释）；树 id 即 AT 节点 id（ELEMENT_TREE §Ⅸ） |
| 8 | 合成 / 动画 | Compositor / `ElementCompositionPreview.GetElementVisual`（WCT SampleApp `Pages/About.xaml.cs:109`）；隐式动画 `SetImplicitShowAnimation`（WCT `…UI.Animations/Implicit.cs:139`） | 合成器图层 `LayerOp`（`element/src/layer.rs:51`）+ 合成滚动；主题转场 `transitions.rs`；动画只走 Sokuou：`ctx.animate`（`widget.rs:84`，真实时钟 + 自动续帧 `widget.rs:81`） | 无隐式动画系统（Kanesumi 全显式登记：`request_anim_frame` / `wants_anim`） | 中 | 「动画只动视觉」：UpdateCtx 不放量测失效（`widget.rs:457-460` 注释）——与 WinUI Composition 只动 ExpressionAnimation 一致 |
| 9 | 主题 | `ResourceDictionary.ThemeDictionaries`（WCT `Styles/Themes.xaml:23`，Light/Dark/HighContrast 三档） | `MetroTheme`（`core/src/theme.rs:14`）+ `ThemeColor`（`core/src/brush.rs:16`）+ `tree.set_theme`（`tree.rs:745`）；每档完整配色（`MetroTheme::ether_dark/light`） | HighContrast 专档未见（推断——theme.rs 未见 HC 构造，需核实） | 中 | 每档一组完整资源、切换整树失效重画（`set_theme` → invalidate）；颜色只经枚举（common 禁区） |
| 10 | 打包 | `Package.appxmanifest`：DisplayName / TargetDeviceFamily（PezMax-One `windows/src/PezMax.App/Package.appxmanifest:18/24`） | 无打包层：宿主 = harness 外壳 `platform::run`；系统应用打包在 Ether 主仓（一个应用一个 deb，主仓根 `Cargo.toml` `apps/calculator` 注释，SYSTEM_APPS_PLAN §Ⅰ-3）——**推断**（未在本仓核实 deb 描述文件） | 范围外 | 范围外（Ether 主仓职责） | 清单式声明（显示名 / 目标设备族）→ deb 元数据对应物，留给系统应用线 |

## 复审附记（本卡合并时核出）

1. **菜单族无障碍缺口**（上表 #7）：e4 把菜单族接入元素树并 `focusable`，但三文件均未实现
   `accessibility()`，e3 的 `assert_access_contract` 也未接入其 `tree_tests` —— 菜单条目对 AT
   不可见。建议队列给 e4 补一张小卡（3 文件 + tree_tests 契约接入），随 e5 之后再合。
2. 宿主无容器时 e3 门的 clippy 一栏 NotRun；本卡合并后在容器补跑（见本卡 proof.json）。
