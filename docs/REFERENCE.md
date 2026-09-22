# REFERENCE —— 微软栈参考与批量迁移动线

> **用途**：本文件是 Kanesumi 面向微软 UI 栈的**源清单 + 对照表 + 挖矿流水线**。
> `docs/PORT_ROADMAP.md` §Ⅰ 给出「开源 / 闭源 / 实证」三分类法；本文件给出**每个源能取到什么、
> 取不到什么、对应 Kanesumi 哪个子系统、怎么批量取**。
> `docs/MATURITY_AUDIT_2026-09-22.md` 给出当前缺口与优先级；本文件是其「去哪儿找答案」。
>
> 本文只讲**机制**，不讲外观。外观唯一真源是 `KANESUMI_DESIGN.md`；UWP 默认外观（Fluent 残留：
> 4px/8px 圆角、Acrylic、部分动效时长）是**反面教材**，不得随迁移混入。

---

## §Ⅰ 迁移判断：该抄哪三条

用户 2026-09-22 的判断，本文按此排序（**这是排期依据，不是随手记**）：

| # | UWP 强在哪 | Kanesumi 现状 | 处置 |
|---|---|---|---|
| 1 | **极速开发流程 + 保险机制**：开发者不必提前预防「按钮飞出窗口」「文本比文本框还大」 | 缺（审计：省略号全仓 1 处、容器多不裁剪、控件把量测宽当绘制宽） | **攻坚**，参 §Ⅲ |
| 2 | **流畅一体的动画与设计语言** | Kanesumi + Sokuou **已有技术**（弹簧/进度驱动/UWP 缓动/预设） | **只接线**（补词汇表覆盖），不重建架构 |
| 3 | **极其好的优化** | 有真底子（损伤重绘、dirty 驱动、dt 限幅）但**关键机制缺位**（无保留树、无布局失效、无独立动画、xdg 角色不吃损伤） | **攻坚（最重要）**，参 §Ⅳ |

**一句话**：Ether 推不下去的原因不是「没有动画系统」，而是**没有保险机制 + 没有优化纵深**。

---

## §Ⅱ 源分层：哪些能读、能读到什么

| 层 | 代表 | 可得性 | 能取到什么 | **取不到什么** |
|---|---|---|---|---|
| 元数据 | `.winmd`（Windows SDK 随附） | 公开 | **API 面**：类型/方法/枚举成员/属性名与声明的默认值。权威、免猜 | 任何实现与视觉规格 |
| 语言投影 | [`microsoft/cppwinrt`](https://github.com/microsoft/cppwinrt)（MIT）、[`microsoft/CsWinRT`](https://github.com/microsoft/CsWinRT) | 开源 | 投影语义（集合/`IReference`/异步如何映射到语言）。写 C++ 实测工程的入口 | **不是框架实现**，无 XAML 控件内部 |
| 框架实现 | [`microsoft/microsoft-ui-xaml`](https://github.com/microsoft/microsoft-ui-xaml)（WinUI 2.x / 3） | **部分开源**：以 `dev/<Control>/` 下**是否有真 `.cpp`** 为准。微软有「分期走向开放协作」的公开说明（[discussion #10700](https://github.com/microsoft/microsoft-ui-xaml/discussions/10700)） | 较新控件（NavigationView / TabView / InfoBar / Expander / ScrollView / ItemsRepeater / ProgressBar …）的**真实现**：视觉状态、尺寸、动画时长与缓动、溢出处理 | Metro 时代老控件（Button / ListView / ContentDialog / ComboBox …）主体仍下派闭源平台 |
| 闭源平台 | `Windows.UI.Xaml` | 无源 | 仅 Windows SDK `generic.xaml` 的默认样式 + 真机观察 | 控件实现 |
| **布局/属性/事件权威实现** | **WPF**（`dotnet/dotnet` 内 `src/wpf/…`，例：[`UIElement.Measure`](https://github.com/dotnet/dotnet/blob/4a9abac4d5298cf14ef124b36d7126db0b3d8dd3/src/wpf/src/Microsoft.DotNet.Wpf/src/PresentationCore/System/Windows/UIElement.cs)） | **完全开源** | **最被低估的源**：Measure/Arrange 两遍布局、布局失效传播、依赖属性与附加属性、`VisualStateManager`、路由事件、`TabIndex`/`IsTabStop` 焦点模型 —— UWP 是从这套语义演化来的，源码可读性远好于 WinUI | Fluent 时代新增控件；UWP 特有 API |
| 文本栈 | `DirectWrite` / **`DWriteCore`**（Windows App SDK 实现，[文档](https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/dwritecore)） | DWriteCore 属 WinAppSDK；架构可读 | 设备无关文本布局、硬件加速文本、多格式文本、语言支持的**管线划分** | 具体 hinting/栅格数值 |
| 合成 / 2D | `Windows.UI.Composition` / `Microsoft.UI.Composition`（文档齐全）、[`microsoft/Win2D`](https://github.com/microsoft/Win2D)（开源） | 合成 API 为平台 API；Win2D 开源 | **保留视觉树、独立动画、隐式动画、表达式动画**的语义；GPU 2D 立即模式 | 合成器内部实现 |
| 调试 / 诊断 | `DebugSettings`（[文档](https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.debugsettings?view=windows-app-sdk-1.1)） | API 文档完整 | **一整族保险机制的自检开关**，见 §Ⅲ 末 | — |
| 实测 | 本仓来源 C（真 UWP 工程） | 自建 | 实测数值与状态机 | — |

> **使用顺序（用户铁律）**：先判类 → 有源先读源 → 无源读 `generic.xaml` + Gallery 观察 → 仍不确定就实测。
> 猜过的都返工过（参 `PORT_ROADMAP.md` §Ⅶ）。

### §Ⅱ.1 `SystemControl*` 与 WinUI 令牌字典的实际位置（2026-09-22 补齐）

上表第 4 行「闭源平台」曾被认为「只能真机观察」，**其实 SDK 自带整套主题字典**；
WinUI 2.x 的令牌字典也不在 NuGet 的 XAML 里。完整清单、行号与取数命令见
**`docs/UWP_PRIMARY_SOURCES.md`**，此处只留索引：

| 想要的东西 | 去哪取 |
|---|---|
| `SystemControl*Brush` / `System*Color` / 控件状态→笔刷映射 | `C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\Neutral\UAP\<ver>\Generic\themeresources.xaml`（暗色 L4-1961 / 亮色 L3920-5878） |
| 控件模板的 VisualState 与 Storyboard（时长、缓动、KeySpline） | 同目录 `generic.xaml`（2.6 MB） |
| `SubtleFillColor*` / `TextFillColor*` / `ControlFillColor*`（WinUI 2.x 一代） | GitHub `microsoft-ui-xaml` 的 `dev/CommonStyles/Common_themeresources_any.xaml`（raw `winui2/main/…`；本机 NuGet 只含模板，不含色值） |
| 某键**是否**属于 UWP 运行时 | 扫 `C:\Windows\System32\Windows.UI.Xaml.dll` 的 UTF-16 字符串（本次据此证伪 `Control*AnimationDuration` 属 UWP） |

> ⚠ 一个反复踩的坑：**`ControlFastAnimationDuration` 一族（250/167/168/83ms）是 WinUI 专有**，
> UWP 运行时里不存在，UWP 风格 XAML 引用它会解析失败。参见 §9.4 的更正。

---

## §Ⅲ 保险机制对照表（第 1 条：让开发者不必预防溢出）

**这是「UWP 开发快」的真正来源**：不是控件多，而是**结构上不可能出现某些错误**。

| UWP 机制 | 它消灭了什么错误 | Kanesumi 现状（审计证据） | 迁移落点 |
|---|---|---|---|
| 布局约束：`Measure(available)` → `Arrange(finalRect)`，父约束优先 | 子元素画到父之外 | 引擎有（`canvas/layout.rs`），但 **47 控件仅 13 个 `measure()`**、261 处字面量 `Rect::new` | 控件逐个接引擎；`arrange` 强制 `Constraints::max` |
| 容器 `ClipToBounds` 语义 | 滚动/动画期内容越界 | 缺（审计 P0-2；本批已补 6 处） | 作为 `LayoutLeaf` 契约默认 true |
| `TextTrimming` + `MaxLines` + 控件模板默认值 | 文本比文本框大 | 引擎支持，但默认 `Clip`、全仓仅 1 处 `Ellipsis`（本批已改默认） | 已落地 `Scene::label/paragraph` |
| `ScrollViewer` 作为**容器**（任何内容可滚） | 「内容放不下」这一类问题整体消失 | `MetroScrollView` 只有状态机无 `render`；`SelectorFlyout` 504px 以上项不可达；`TreeView` 无滚动 | 通用 ScrollHost（§Ⅶ P0-3） |
| **`CommandBar` 动态溢出**（`SecondaryCommands` / `AppBarButton.IsInOverflow`） | **按钮排到窗口外点不到**（用户原话） | 无任何等价物 | 新增 `MetroCommandBar` 溢出区；与 `AdaptiveTrigger` 配合 |
| `NavigationView` 自适应 `PaneDisplayMode` | 窄宽下侧栏挤爆内容 | `NavigationView` 已有模式，但无断点驱动 | 接 `AdaptiveTrigger` |
| `AdaptiveTrigger` + `VisualStateManager` | 手写断点散落各页 | 无 | 断点状态机（可与 `structure` 合并） |
| `Viewbox` | 图标/内容拉伸失控 | 无（`Scene::image` 直接指定目标 rect） | 低优先，`Scene::image` 加 fit 模式 |
| `FocusManager` / `IsTabStop` / `TabIndex` / `XYFocus` / `KeyboardAccelerator` | 纯键盘不可用、焦点环同时亮两个 | 无（每个控件各持 `pub focused: bool`） | `controls/focus.rs` 焦点环（已起草）+ `App::focus_move` + 外壳 Tab 路由 |
| `AutomationProperties` / UIA | 无障碍与自动化验收 | 零（最大结构缺口） | 语义树（§Ⅶ P2） |

### §Ⅲ.1 `DebugSettings` —— 把「保险」变成可自检的开关

这一族是**直接可抄的设计**（本仓应逐项对标）：

| DebugSettings 属性 | 作用 | Kanesumi 对标 |
|---|---|---|
| `EnableFrameRateCounter` | 帧率 + 每帧 CPU 占用浮层 | 已有帧诊断雏形（`ether-harness-trace.log`），应做成可开关浮层 |
| `IsOverdrawHeatMapEnabled` | 过度绘制热图 | 无 —— 对自绘框架尤其值钱 |
| `LayoutCycleTracingLevel` / `LayoutCycleDebugBreakLevel` | 布局循环追踪与断点 | 无 —— 有 `Arrangement` 后再补 |
| `IsXamlResourceReferenceTracingEnabled` + `XamlResourceReferenceFailed` | **资源键写错不再静默回退** | **正对审计 R16「56 处资源引用从未定义、零提示」**：令牌校验 |
| `IsBindingTracingEnabled` + `BindingFailed` | 绑定失败事件 + 输出 | 待有绑定后对标 |
| `IsTextPerformanceVisualizationEnabled` | 文本性能可视化 | 部分等价：`layout_misses` 计数已有 |
| `FailFastOnErrors` | 错误立即 FailFast 而非静默 | 与「不静默回退」铁律同源 |

> **可迁移的原则**：把「开发者容易犯的错」做成**运行时可打开的自检**，而不是靠纪律。

---

## §Ⅳ 优化对照表（第 3 条：最重要）

| UWP 机制 | 它省掉了什么 | Kanesumi 现状 | 迁移落点 | 收益 / 风险 |
|---|---|---|---|---|
| **保留视觉树 + 失效传播** | 每帧重建整棵 UI 与全部绘制命令 | 每帧从状态重建 `Scene`；`RetainedScene` 反而每帧两次全量 `render_decl` | `controls/retained.rs` 改造 + `decl` keyed reconcile | 高收益；正确性靠「命令序列与全量路径逐条等价」测试守卫 |
| **独立动画（independent animation）**跑在合成线程 | UI 线程卡顿即动画冻结；每帧推进状态 | 动画在 App 的 `update(dt)` 内逐帧推进（`platform.rs` frame callback） | 合成器侧动画指令（Ether）或 harness 独立推进器 | 高收益；需与 Sokuou 的 Progress 语义对齐，**不重建动画架构** |
| **隐式 / 表达式动画**（`ImplicitAnimations` / `ExpressionAnimation`） | 手写每处 property 的起止状态 | 控件逐个手写动画（仅 14 类控件有动画） | `kanesumi-anim` 预设 + 控件接线（补齐词汇表） | 中收益；低风险 |
| **布局失效（invalidate）而非每帧重排** | 每帧全量 Measure/Arrange | canvas 引擎有缓存雏形（galley/layout 缓存），但控件多现算 | `LayoutLeaf` 加失效标记 | 高收益；中风险 |
| **虚拟化 + 容器回收 + 滚动锚点**（`ItemsRepeater` / `IScrollAnchorProvider`） | 长列表内存与布局 O(n) | `MetroRepeater` 只做可见性虚拟化，**无回收**；`arrange` 仍对所有子 `measure` | `repeater` + `MetroList` | 高收益；需项身份（keyed）先落地 |
| 文本：设备无关布局 + 字形缓存/图集 | 每帧重排版/重栅格化 | **已较成熟**：`ShapeKey`/`LayoutKey` 缓存、`Arc` 零拷贝、glyph 位图复用 | 补缓存失效 API（font identity / text-scale） | 中收益；低风险 |
| 损伤 / 局部重绘 | 全帧上传 | CPU 路径已有；**xdg-shell 角色全量重绘** | `platform.rs` 让 xdg 角色吃 damage | 高收益；低风险 |
| GPU 合成解耦（Composition / Win2D） | CPU 光栅瓶颈 | 双路径（CPU 光栅 + wgpu）；dmabuf 直通已全局化 | 已在进行中（`LINUX_DMABUF_PLAN`） | 已投入 |
| 帧统计与热图 | 凭感觉调优 | 部分（帧计数、layout_misses） | 对标 `EnableFrameRateCounter` / `IsOverdrawHeatMapEnabled` | 低风险、高性价比 |

> **Kanesumi 的优化主线**（本文建议的排序）：
> ① 保留树 + 失效传播 → ② 布局失效与接管 → ③ xdg 角色损伤重绘 → ④ 容器回收 + 滚动锚点 →
> ⑤ 独立动画（在 ① 稳定后）。
> 理由：①不成立时，②④都是在流沙上做优化；⑤依赖①的失效机制。

---

## §Ⅴ 开发流程对照表（第 1 条的另一半）

| UWP 机制 | 开发者省掉了什么 | Kanesumi 现状 | 迁移落点 |
|---|---|---|---|
| `{x:Bind}` 编译期绑定 | 手写「状态 → UI」同步 | 每帧手写 `Decl`/Scene；`DeclAction` 只是 match tag | `decl` 扩容（先 keyed，再绑定） |
| `DataTemplate` / `DataTemplateSelector` / `ItemContainerStyle` | 列表行模板复用 | `MetroList` 内置单一样式 | `list`/`repeater` 模板化 |
| `ResourceDictionary` + `ThemeResource` / `StaticResource` + `ThemeDictionaries` | 主题/令牌集中管理 + 深浅色 | `MetroTheme`/tokens 有；**未知键静默回退** | 令牌校验（对标 `XamlResourceReferenceFailed`） |
| `Style` / `Setter` / `BasedOn` / `Template` | 视觉规格与逻辑分离 | 控件内硬编码常量 | 需先有视觉树；与 §Ⅳ① 同批 |
| `VisualStateManager` + `VisualTransition` | 状态切换的集中声明 | 各控件手写状态机（`ControlState`） | 与 §Ⅲ `AdaptiveTrigger` 同批 |
| 依赖属性 / 附加属性 | 可继承、可动画、可绑定的属性系统 | 无（普通 Rust 字段） | 明确**不照搬**：Rust 用显式 build/状态即可，只借「失效传播」语义 |
| `ContentPresenter` / `ContentControl` | 组合任意子树 | `decl` 覆盖 6 元素 2 容器 | `decl` 扩容 |

> **判断**：§Ⅴ 里真正卡开发效率的是**绑定 + 模板 + 令牌校验**三项；依赖属性体系**不要照搬**（Rust 侧代价大于收益）。

---

## §Ⅵ 批量迁移流水线（怎么「大量迁移」而不失控）

对每一个待迁移机制，固定走这五步，产物必须落盘：

1. **判类**：源在 §Ⅱ 哪一层？开源（可读 `.cpp`/C#）／闭源（`generic.xaml` + 观察）／实证。
2. **取源**：开源直接读实现；闭源读 Windows SDK `generic.xaml` + WinUI Gallery；API 面读 `.winmd`（或用 cppwinrt 生成的头）。
3. **写规格**：`CONTROL_SPEC.md`（控件数值/状态/时长）或本文 §Ⅲ/§Ⅳ 的对照行（机制）。**规格是资产，跑规格的工程不是。**
4. **落实现 + 回归**：`kanesumi-*` 对应模块；每个机制至少一条「退化输入不 panic / 不越界」的测试。
5. **回流与登记**：实测结论回流 `CONTROL_SPEC.md` / `ANIMATION_SPEC.md`；刻意偏离正典的差异必须写明理由（否则半年后会被当笔误改掉）。

**批量作业的批量单位** = 「一个机制 × 全部相关控件」。例：`TextTrimming` 默认值是**一个机制**，一次覆盖全部单行标签控件；而不是「一个控件 × 全部机制」。

---

## §Ⅶ 待办映射（与审计 §Ⅲ 对齐，本文只补「源」）

| 优先级 | 事项 | 主要源（§Ⅱ 行） |
|---|---|---|
| P0 | 布局引擎落地（控件接 `measure`/`hit_test`） | WPF `UIElement.Measure/Arrange`；WinUI `ScrollView` |
| P0 | 焦点 + Tab 遍历 | WPF `FocusManager`/`IsTabStop`/`TabIndex`；WinUI `FocusManager` |
| P0 | 通用滚动容器（clip + 虚拟化 + 命中） | WinUI `ScrollView`/`ScrollPresenter`/`ItemsRepeater`（开源 `.cpp`） |
| P0 | 优化：保留树 + 失效传播 | `Windows.UI.Composition` 语义 + WPF 失效传播实现 |
| P1 | 动画词汇补齐（按钮下沉 / stagger / AddDelete） | WinUI `dev/` + `ANIMATION_SPEC.md` 处置表 |
| P1 | keyed reconciliation + 容器回收 | `ItemsRepeater`（开源） |
| P1 | xdg 角色损伤重绘 | 平台内部（无源，自研） |
| P1 | 令牌校验（未知键报错） | 对标 `IsXamlResourceReferenceTracingEnabled` |
| P2 | 语义树 / 无障碍 | UIA 文档 + 自建 AT-SPI 导出 |
| P2 | 剪贴板 / 拖放 | `Windows.ApplicationModel.DataTransfer` 语义 |

---

## §Ⅷ 索引（外部源）

- Windows Runtime 语言投影：<https://github.com/microsoft/cppwinrt>、<https://github.com/microsoft/CsWinRT>
- WinUI 开源实现：<https://github.com/microsoft/microsoft-ui-xaml>（开放进度见 discussion #10700）
- WPF（布局/属性/事件权威实现）：<https://github.com/dotnet/wpf>（现并入 <https://github.com/dotnet/dotnet>）
- 文本：<https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/dwritecore>
- 诊断开关：<https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.debugsettings>
- 命令栏溢出：<https://learn.microsoft.com/en-us/uwp/api/windows.ui.xaml.controls.commandbar.secondarycommands>
- 动画与媒体优化指引：<https://learn.microsoft.com/en-us/windows/uwp/debug-test-perf/optimize-animations-and-media>
- 2D：<https://github.com/microsoft/Win2D>

---

## §Ⅸ 已取到的权威值（2026-09-22 补）

### 9.1 重大修正：WinUI 3 的主题色**定义是开源的**

`CONTROL_SPEC.md` §11.1 说「`SystemControl*` 笔刷的定义不在开源仓里，必须读 SDK
`themeresources.xaml`」—— 那句话针对的是 **UWP / WinUI 2**。**WinUI 3 的定义就在开源仓里**：

```
microsoft-ui-xaml/
  controls/dev/CommonStyles/Common_themeresources_any.xaml   ← 全局色/时长定义（Default / Light / HighContrast 三套）
  controls/dev/InfoBar/InfoBar_themeresources.xaml           ← 控件级取数入口（InfoBar 为例）
  controls/dev/<Control>/<Control>_themeresources.xaml       ← 每个控件都有自己的
```

取数方式：`https://raw.githubusercontent.com/microsoft/microsoft-ui-xaml/main/<路径>`。
许可 MIT。**Agent 不必再"猜"主题色** —— 先读这份，读不到才退回 `generic.xaml` + 实测。

### 9.2 语义状态色（已落地为 `kanesumi-core::StatusColors`）

| 令牌 | 暗色（`Default`） | 亮色（`Light`） |
|---|---|---|
| `SystemFillColorSuccess` | `#6CCB5F` | `#0F7B0F` |
| `SystemFillColorCaution` | `#FCE100` | `#9D5D00` |
| `SystemFillColorCritical` | `#FF99A4` | `#C42B1C` |
| `SystemFillColorNeutral` | `#8BFFFFFF` | `#72000000` |
| `SystemFillColorSolidNeutral` | `#9D9D9D` | `#8A8A8A` |
| `SystemFillColorAttentionBackground` | `#08FFFFFF` | `#80F6F6F6` |
| `SystemFillColorSuccessBackground` | `#393D1B` | `#DFF6DD` |
| `SystemFillColorCautionBackground` | `#433519` | `#FFF4CE` |
| `SystemFillColorCriticalBackground` | `#442726` | `#FDE7E9` |
| `SystemFillColorNeutralBackground` | `#08FFFFFF` | `#06000000` |
| `SystemFillColorSolidAttentionBackground` | `#2E2E2E` | （同族实心，按需取） |

> **关键事实**：`SystemFillColorAttentionBrush` 直接绑定 `SystemAccentColor`（亮）/
> `SystemAccentColorLight2`（暗）—— **attention 就是 accent**，不另设一套「注意色」。
> 这也是「单一强调色」与「语义状态色」能并存的原因：attention 归 accent，其余三档归语义色。
>
> **换算陷阱**：WinUI 写 AARRGGBB，Kanesumi 的 `from_hex` 是 RRGGBBAA。逐项换算；半透明**黑**
> 还必须用 `from_rgba`（其数值 ≤ `0x00FFFFFF`，走 `from_hex` 会被当成不透明 RGB）。

### 9.3 中性色与文本色（可用于替换 `CANON_VS_TEMPORARY.md` 的临时值）

| 令牌 | 暗色 | 亮色 |
|---|---|---|
| `SolidBackgroundFillColorBase` | `#202020` | `#F3F3F3` |
| `SolidBackgroundFillColorSecondary` | `#1C1C1C` | `#EEEEEE` |
| `SolidBackgroundFillColorTertiary` | `#282828` | `#F9F9F9` |
| `SolidBackgroundFillColorBaseAlt` | `#0A0A0A` | `#DADADA` |
| `TextFillColorPrimary` | `#FFFFFF` | `#E4000000` |
| `TextFillColorSecondary` | `#C5FFFFFF` | `#9E000000` |
| `TextFillColorTertiary` | `#87FFFFFF` | `#72000000` |
| `DividerStrokeColorDefault` | `#15FFFFFF` | `#0F000000` |
| `FocusStrokeColorOuter` | `#FFFFFF` | `#E4000000` |

> 注意：**WinUI 的暗色基底是 `#202020` 而非纯黑**（最暗的是 `BaseAlt #0A0A0A`）。这印证了
> 「OLED 纯黑」不是该体系的规则，只是暗色方案的一个可选取值。

### 9.4 动画时长（**已解决既有文档矛盾**）

同文件文末直接给出：

```
ControlNormalAnimationDuration      = 00:00:00.250   (0.250s)
ControlFastAnimationDuration        = 00:00:00.167   (0.167s)
ControlFastAnimationAfterDuration   = 00:00:00.168   (0.168s)
ControlFasterAnimationDuration      = 00:00:00.083   (0.083s)
ControlFastOutSlowInKeySpline       = 0,0,0,1
```

`ANIMATION_SPEC.md` 与 `kanesumi-anim/src/presets.rs` 注释里「180ms vs 0.167s」的分歧到此为止：
**权威值是 0.167s，缓动为 cubic-bezier(0,0,0,1)**。

> **2026-09-22 归属更正**：这四个键列出自 `dev/CommonStyles/Common_themeresources_any.xaml`（WinUI 侧），
> 但本仓此前把它们当成 **UWP** 资源引用。实测：UWP SDK 的 `themeresources.xaml` / `generic.xaml`
> （26100 与 22621 两版）与 `C:\Windows\System32\Windows.UI.Xaml.dll`（UTF-16 全量扫描）
> 对这四个键名 **全部 0 命中**；它们在 WinUI 2.8.6 的 `resources.pri` 里各出现 12 次。
> 结论：**值可用、归属是 WinUI**；UWP 风格 XAML 引用这些键会解析失败。
> `ControlSlowAnimationDuration` **不存在**（0 命中），前文旧表的「Slow」一行作废。

