# Kanesumi 动画规格

> **本文是 Kanesumi 动画词汇的单一真源。** 各控件节不再各自复述时长与曲线 ——
> 那违反正典铁律 4（单一真源）。`CONTROL_SPEC.md` 的控件节只引用本文的条目名。
>
> 数据来源：**来源 C（Windows 扇区实证）**。见 `PORT_ROADMAP.md §Ⅰ C`。
> 词汇表取自 Windows Runtime 动画库的官方定义
> （[Animations in XAML](https://learn.microsoft.com/en-us/windows/apps/develop/motion/xaml-animation)）。

---

## §Ⅰ 模型：词汇可以搬，模型不能搬

### 这是必须先说清的一件事

UWP 的动画库建在 **Storyboard 时间轴**上：动画是「播放」的，有 `Begin` / `Stop` /
`Pause` / `Resume`，有 `Duration` 与 `BeginTime`，目标属性是 **依赖属性**。

而 Kanesumi 正典对此有明确禁令（§Ⅲ.4 与铁律）：

> **状态驱动渲染**：`state → progress → resolved spatial state → render`，
> **不做 timeline 播放**。

**两者不是语法差异，是模型差异**，后果很具体：

| | Storyboard 时间轴 | Kanesumi 状态驱动 |
|---|---|---|
| 中断 | 要 `Stop` / `Begin` 切换；从当前**视觉**值续接需要额外处理 | **天然可中断** —— 每帧从状态重算 `progress` |
| 目标变化 | 需重建 storyboard 或改 `To` | `set_target` 继承当前值 |
| 与状态一致性 | 可能「播完了但状态已变」 | 状态是唯一真源，不可能不一致 |

Kanesumi 选状态驱动**正是为了可中断**（参 `kanesumi-anim` 的 `Progress` /
`SpringAnim`：`set_target` 继承当前位置与速度）。

### 所以「直接搬过去」的正确解读

| 搬 | 不搬 |
|---|---|
| ✅ **词汇表** —— 哪些转场存在、叫什么、用在什么场景 | ❌ **Storyboard 时间轴模型** |
| ✅ **时序数值** —— Duration / BeginTime / 缓动（在状态驱动模型里同样成立） | ❌ `Begin`/`Stop`/`Pause`/`Resume` 的播放控制 |
| ✅ **场景映射** —— 什么 UI 变化配什么转场 | ❌ 依赖属性（Kanesumi 无此概念） |

**判据**：UWP 的每个词汇问一句「它在状态驱动模型里对应什么」。
对应不上的，标注跳过并说明原因，而不是硬搬。

---

## §Ⅱ 主题转场（Theme Transitions）

UWP 会在特定 UI 条件下**自动**施加，不需要显式触发。这是最值得搬的一类 ——
它们是**场景到动画的映射表**，而 Kanesumi 目前缺的正是这张表。

| UWP 词汇 | 场景 | Kanesumi 处置 |
|---|---|---|
| `EntranceThemeTransition` | 内容**首次**出现 | ✅ 采用。容器内子项**依次错位**进场（UWP 默认开启错位，可用 `IsStaggeringEnabled="False"` 关掉） |
| `ContentThemeTransition` | 内容**变化**（非首次） | ✅ 采用。与 Entrance 区分：首次 vs 变化 |
| `NavigationThemeTransition` | `Frame` 页面导航 | ✅ 采用。`EntranceThemeTransition` 等价于默认参数下的它 |
| `DrillInThemeAnimation` / `DrillOutThemeAnimation` | 逻辑层级**前/后**导航 | ⚠ **Kanesumi 当前缺失** —— 详见 §Ⅳ |
| `AddDeleteThemeTransition` | 列表增删项 | ✅ 采用。**先重排已有项腾位，再加新项**；删除反之 |
| `ReorderThemeTransition` | 列表项换位 | ✅ 采用。与「删了再加」是**不同**动画，不可互相替代 |
| `RepositionThemeTransition` | 元素被移到新位置 | ✅ 采用 |
| `PaneThemeTransition` | 大块边缘 UI（面板 / 任务窗格） | ✅ 采用 |
| `EdgeUIThemeTransition` | 小块边缘 UI（命令栏 / 顶部提示条） | ✅ 采用 |
| `PopupThemeTransition` | 弹出式上下文 UI | ✅ 采用。**light dismiss** 场景的反馈 |
| `SplitOpenThemeAnimation` / `SplitCloseThemeAnimation` | ComboBox 风格的展开/收起 | ✅ 采用 |

### 容器传播语义（值得单独记住）

把转场设在**容器**上时，其**全部子项**参与转场。`EntranceThemeTransition` 设在
面板上 → 子项依次错位进场，形成视觉上有节奏的入场，而不是一齐出现。

> 子项进场的**顺序取决于它们在面板中的顺序**，不一定等于屏幕上的视觉位置。
> 想让某子项先出现，就要让它先排在 children 里 —— 这是实测会踩的点。

---

## §Ⅲ 主题动画（Theme Animations）

针对**单个元素**、存在于视觉状态内。与转场的区别：转场挂在控件属性上、影响
状态**之间**的过渡；主题动画挂在视觉状态里、面向一个具体元素。

| UWP 词汇 | 场景 | Kanesumi 处置 |
|---|---|---|
| `FadeInThemeAnimation` / `FadeOutThemeAnimation` | 显示 / 隐藏瞬时 UI | ✅ 采用。**对话框、工具提示**的推荐动画 |
| `PointerDownThemeAnimation` / `PointerUpThemeAnimation` | 点击 / 轻触反馈 | ✅ **已采用** —— 见下方对照 |
| `PopInThemeAnimation` / `PopOutThemeAnimation` | 弹出组件出现 / 关闭 | ✅ 采用。**不透明度 + 位移复合**。Flyout / 上下文菜单的推荐动画 |
| `RepositionThemeAnimation` | 对象被重新定位 | ✅ 采用（与同名 Transition 配对，一个用于显式、一个用于自动） |
| `DragItemThemeAnimation` / `DragOverThemeAnimation` | 拖拽中 / 拖拽经过 | ⏳ 待定。Kanesumi 暂无拖拽 |
| `DropTargetItemThemeAnimation` | 潜在放置目标 | ⏳ 待定。同上 |

### `PointerDown/UpThemeAnimation` 与 MetroIndication 的对照

**这条是一次确证，不是新增。** 上游 `CONTROL_SPEC.md` 记录过：

> 按压反馈是**位移，不是缩放** —— UWP `PointerDown/UpThemeAnimation`，
> 约 100ms 的 Y 方向下沉/复位，**完全没有 Scale 反馈**。

Windows Runtime 动画库存在这一对词汇，**独立确证了该记录**。
来源从「逆推」升格为「词汇表实证」（参 `PORT_ROADMAP.md §Ⅰ C`）。

---

## §Ⅳ Kanesumi 当前缺失的词汇（这才是搬过来的价值）

以下词汇在 UWP 动画库中存在，而 **`kanesumi-anim` 目前完全没有对应物**。
这不是「UWP 有而我们也要有」，是**真实的能力缺口**：

| 缺失 | 现状 | 为什么需要 |
|---|---|---|
| **层级导航转场**（DrillIn / DrillOut） | `kanesumi-structure` 有 `Navigation` 状态机 + `transition_progress`，但**无层级语义** | 列表 → 详情的「进入层级」与返回，UWP 给了明确的两向词汇。Kanesumi 目前只有通用 `page_transition` |
| **列表突变**（AddDelete） | 无 | 增删列表项时的腾位重排。**这是最常见的动效需求之一** |
| **列表换位**（Reorder） | 无 | 与增删**不同**的动画，拖拽排序场景 |
| **弹出组件**（PopIn / PopOut） | `MetroDialog` / `MetroDropdownMenu` 各自实现 | 缺统一的「弹出出现/关闭」词汇 |
| **边缘 UI**（EdgeUI / Pane） | 无 | 命令栏、面板的进出。**Kanesumi 的「贴边」哲学正需要它** |
| **跨视图连续性**（ConnectedAnimation） | 无 | Win10 1607 引入；元素在导航间「看起来连续移动」。自绘路线的天然强项，但无词汇 |

> 注意最后一类：**自绘渲染器做 ConnectedAnimation 比 XAML 容易** ——
> 因为场景命令由我们产出，让同一元素在前后两帧沿用同一绘制项是自然的。
> 这是自绘路线相对 XAML 的一个**优势点**，值得专门做。

---

## §Ⅴ 时序数值

### Kanesumi 既有取值（权威，勿改）

取自 `kanesumi-anim/src/presets.rs`：

| 常量 | 值 | 曲线 |
|---|---|---|
| `METRO_STANDARD_DURATION` | **0.25 s** | Quadratic / EaseOut |
| `DURATION_QUICK_SWITCH` | **0.167 s** | Quadratic / EaseOut（对齐 UWP `ControlFastAnimationDuration`） |
| `DURATION_TOGGLE_FLIP` | **0.15 s** | Cubic / EaseOut（UWP `RepositionThemeAnimation`） |

弹簧五预设：`standard_interaction(0.50, 0.825)` · `quick_interaction(0.30, 0.60)` ·
`slow_reveal(0.65, 0.85)` · `dialog_enter(0.45, 0.70)` · `page_transition(0.40, 0.80)`。

### UWP 默认值：已从 SDK 取到一部分，其余待实测

**权威来源已确认**：Windows SDK 的主题资源字典，不是 `microsoft-ui-xaml`：

```
C:\Program Files (x86)\Windows Kits\10\DesignTime\CommonConfiguration\
  Neutral\UAP\<SDK 版本>\Generic\themeresources.xaml
```

`microsoft-ui-xaml` 的 `Generic.xaml` 只**引用**这些键（`{ThemeResource …}`），
定义在平台侧 —— 所以读开源仓是读不到的，必须读 SDK 里的这份。

已取到的取值（**不是猜的，是从上述文件读出的字面量**）：

| UWP 键 | 值 | 用途 |
|---|---|---|
| `ScrollBarContractDuration` | **0.1 s** | 滚动条收起 |
| `ScrollBarExpandDuration` | **0.1 s** | 滚动条展开 |
| `ScrollBarContractDelay` | **2 s** | 收起前的静止延迟 |
| `ScrollBarExpandBeginTime` | **0.4 s** | 展开前的延迟 |
| `ScrollViewerSeparatorExpandDuration` | **0.1 s** | 滚动条分隔线展开 |
| `ScrollViewerSeparatorContractDuration` | **0.1 s** | 分隔线收起 |
| `ScrollViewerSeparatorContractDelay` | **2 s** | 分隔线收起延迟 |
| `ScrollViewerSeparatorExpandBeginTime` | **0.4 s** | 分隔线展开延迟 |
| `SplitViewPaneAnimationOpenDuration` | **0.2 s** | 面板打开 |
| `SplitViewPaneAnimationCloseDuration` | **0.1 s** | 面板关闭 |
| `SplitViewPaneAnimationOpenPreDuration` | **0.19999 s** | 面板打开前置阶段 |
| `HandwritingViewGestureTipsElementEaseInDuration` | **0.1 s** | 手写笔手势提示 |

**⚠ 一处需要更正。** 本表早先列过 `ControlFastAnimationDuration`、
`ControlNormalAnimationDuration`、`ControlFasterAnimationDuration` 三个键 ——
**这三项在该 SDK 版本里查不到定义，也未在 `Generic.xaml` 中被引用。**

因此下面这一条注释是**未经证实的**，不要当成依据：

> ~~`DURATION_QUICK_SWITCH` = 0.167 s（对齐 UWP `ControlFastAnimationDuration`）~~

`0.167 s` 这个值本身是 Kanesumi 既有的、有效的取值（§Ⅴ 上表），
但「它对齐某个 UWP 常量」这个说法**没有证据**。要么在应用里实测确认，
要么把注释改成「Kanesumi 自定」。

### 观察到的量级特征（有信息量）

已取到的值几乎都围绕 **0.1 s / 0.2 s**，而**延迟**是 **0.4 s / 2 s** 量级。
这与 Kanesumi 的 `METRO_STANDARD_DURATION = 0.25 s` 处于同一量级，
但 Kanesumi 的取值偏慢一档。

这类「同量级、偏一档」的差异正是需要决定归属的地方 ——
是 Kanesumi 刻意放慢，还是当年读数就偏了。参 §Ⅵ 的两分法。

### 待实测

上表未覆盖的键（尤其是控件状态切换类）仍需在 `verify/uwp` 里实测回填。
**填表方式**：跑控件、掐时长，标注实测条件。
**不要凭文档或记忆填** —— 这份表会被多平台实现照做，填错比留空代价大。

---

## §Ⅵ 处置分歧的原则

与控件规格同一条规矩（参 `PORT_ROADMAP.md §Ⅰ C`）：

> **以 Kanesumi 正典为准，不以 UWP 默认为准。**

UWP 默认带 Fluent 时代残留，而 Kanesumi 与 UWP 的偏离是**刻意**的
（无圆角、深底、单一强调色、状态驱动而非时间轴）。

因此：

1. **正典刻意偏离** → 保留 Kanesumi 取值，**在此处补一句偏离理由**
2. **正典未覆盖** → 采用 UWP 实测值

第 1 类过去完全没有记录，这是隐患：**没有记录下来的刻意偏离，半年后会被当成笔误改掉。**
