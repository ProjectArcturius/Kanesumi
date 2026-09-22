# Windows 侧可研究内容清单（2026-09-22 盘点）

> **用途**：这是一份「**离开 Windows 之前该抓什么**」的清单。Windows workspace 今后可能很少打开，
> 而本清单里的东西**在 Linux 上取不到或要绕大弯**。每项都标注：为什么只有 Windows 能做 /
> 在哪 / 怎么做 / 能解决什么 / 工作量 / 状态。
>
> 已完成的部分见 §F；若只剩最后一次 Windows 窗口，照 §G 的顺序执行即可。

---

## §0 本机资产盘点（2026-09-22 实测）

| 资产 | 状态 | 说明 |
|---|---|---|
| **WSL** | ❌ **未安装** | 本机无法执行 Linux 侧验证（`wsl --install` 会要求重启）⇒ Windows 侧机会是唯一窗口 |
| **dotnet** | ✅ `C:\Program Files\dotnet\dotnet.exe` | 可跑 **WPF 探针**（底层 DirectWrite，可作文本/布局 oracle）与任意 C# 程序 |
| **python** | ✅ 3.14 | 抽取/转换脚本 |
| **cargo / rustc** | ✅ 1.95.0 | 本仓可 `test`/`clippy`（已把测试字体路径补齐，见 §F） |
| **Windows SDK** | ✅ 6 个版本：14393 / 15063 / 16299 / 17134 / **22621** / **26100** | 主题字典与控件模板（26100 = Win11 一代） |
| **SDK 工具** | ✅ `makepri.exe` / `makeappx.exe` / `signtool.exe` | 解包 appx、dump .pri 可能用得上（见 A6） |
| **OS UWP 主题字典** | ✅ 已取数 | `…\UAP\10.0.26100.0\Generic\themeresources.xaml`（暗 L4-1961 / 亮 L3920-5878）+ `generic.xaml`（2.6 MB） |
| **WinMD + 离线 API 文档** | ✅ `Microsoft.UI.Xaml.winmd`(286 KB) + `Microsoft.UI.Xaml.xml`(**426 KB**) | 层 1 权威（API 面与文档化默认值），**尚未系统利用** |
| **WinUI 2.8.6 NuGet** | ⚠️ 只有模板，令牌字面量在 `resources.pri` 的 XBF 里 | 源码字典需联网取（GitHub `winui2/main`） |
| **WinRT 运行时** | ✅ **可用**（经 `powershell.exe` 5.1） | 能读系统真值，如 accent 全档位（见 B4）—— **Linux 永远拿不到的** |
| **兄弟仓（同盘克隆）** | ✅ 31 个 | 至少 6 个与本项目直接相关，见 §C |
| **网络** | ✅ 可用 | `raw.githubusercontent.com` 直连成功（已用于取 WinUI 2/3 源码字典） |

---

## §A UWP / WinUI 一手源补完

| # | 内容 | 为什么只有 Windows / 为何值得 | 怎么做 | 能解决 | 工作量 |
|---|---|---|---|---|---|
| A1 | **WinUI 3 现代令牌字典**（`TextFillColor*` / `SubtleFillColor*` / `ControlFillColor*` / `SolidBackgroundFillColor*` / `StrokeColor*`） | 本机 NuGet 只有 WinUI 2；WinUI 3 是现代令牌唯一出处（`StatusColors` 已按其取值，但中性色只借了邻近值） | 拉 `microsoft-ui-xaml` **main** 分支 `dev/CommonStyles/Common_themeresources_any.xaml`（网络已验证可达） | **T3**（亮色中性色"借值"）、`REFERENCE.md` §9.3 | 小 |
| A2 | **`x:Double` / `Thickness` 常量全量 dump** | `themeresources.xaml` 里有成百个 `x:Key` 数值（如 `PaneToggleButtonSize 40`、`NavigationBackButtonWidth 40`），`CONTROL_SPEC` 里大量「未在快照」的尺寸/粗细都能一次补完 | `Select-String 'x:Double x:Key='` / `'Thickness x:Key='` 全量导出成表 | CONTROL_SPEC 的尺寸类缺口；M2-3（控件补 measure）需要权威尺寸 | 小（可脚本化） |
| A3 | **UWP 字号梯度** | 我们的 `MetroTypography` 字形梯度是快照提取值，OS 字典里有权威 FontSize 资源族 | 同 A2，抓 `FontSize`/`ControlContentThemeFontSize` 等 | typography 规格完备性 | 小 |
| A4 | **尚未核对的控件视觉状态表** | `CONTROL_SPEC` 多数章节只覆盖到「有快照」的那批；AutoSuggestBox / NumberBox / ColorPicker / RatingControl / ToolTip / Flyout / CommandBar / NavigationView / Pivot / GridView / ScrollViewer 仍有推断项 | 在 `generic.xaml` 里逐个控件抓 `VisualState` + 笔刷（本次三份报告的方法可直接复用） | CONTROL_SPEC 准确性；多个控件的实现对齐 | 中 |
| A5 | **HighContrast 字典** | OS 字典含完整高对比度套件（`SystemColor*` 引用），我们**完全没有 HC 主题** | 抓 `themeresources.xaml` L1962-3919（HC 字典） | M7 无障碍路线的设计依据 | 小 |
| A6 | **XBF / `.pri` 反编译** | WinUI 2 独占控件的令牌（TeachingTip 全色键、`ProgressBarErrorForegroundColor` 等）只存在于 `Microsoft.UI.Xaml.2.8.appx → resources.pri`（XBF 二进制） | `makeappx.exe unpack` + `makepri.exe dump /if resources.pri /of out.xml /dt detailed`；XBF 本体需要额外解析器 | 需要 WinUI 2 独占令牌时（当前非阻塞） | 中高（工具链不在手） |
| A7 | **WinMD + 426 KB API XML 文档** | 层 1 权威：**API 面 + 文档化默认值**（例：`PointerDownThemeAnimation` 的文档语义、`DebugSettings` 全族开关） | 解析 `Microsoft.UI.Xaml.xml`（先 grep 关心的类型） | 保险机制对照表（`REFERENCE.md` §Ⅲ）的补完；M6/M7 的依据 | 小 |
| A8 | **UWP 控件的动画时长二次核对** | 本次已确认：凡模板写 `<RepositionThemeAnimation/>` 等**无参**主题动画者，时长 OS 预置、XAML 读不到 —— 这类只能实测或查文档 | 走 A7 的文档路径 + 保留 `UWP_PRIMARY_SOURCES.md` §Ⅴ 的「未解决」清单 | M6-1/M6-2 的依据边界 | 小 |
| A9 | **系统字体与可变字体** | Windows 有 Segoe UI Variable / 雅黑可变字体；`FontWeight` 在本仓是死字段（**T7**），是否走可变轴需要本机字体实物 + 实测 | 列 `C:\Windows\Fonts` 里带多重字面/可变轴的字体；用 dotnet (WPF) 或 DWriteCore 实测轴支持；再查 `fontdue` 的能力 | **T7**（字重）路线决策 | 中 |

---

## §B Windows 专属 oracle（本机可执行的行为验证）

这类最值钱：**不是"读文档"，而是拿微软自己的实现当标准答案去量我们的实现**。

| # | 内容 | 为什么只有 Windows 能做 | 怎么做 | 能解决 | 工作量 |
|---|---|---|---|---|---|
| B1 | **DirectWrite 文本度量 oracle** | 本仓 `kanesumi-canvas/src/text.rs`（887 行，全仓最成熟模块）的断行/省略号/行高/字距参数**从未与真·DirectWrite 对过**。WPF 的 `FormattedText`/`TextBlock.Measure` 底层就是 DirectWrite | 写一个 C# 探针（`dotnet run`，无需 GUI）：给定字体/字号/文本，输出每簇 advance、行盒高、`TextTrimming.CharacterEllipsis` 的结果串；与 `TextEngine` 的输出逐项对比 | 文本管线的偏差清单（我们最怕的"画出来不对齐"多半源于此）；也直接服务 M2（排版即量测） | 中 |
| B2 | **WPF 布局语义 oracle** | M2「布局接管」的所有契约（Measure/Arrange、`DesiredSize` 夹紧、`Constraints::max` 强制、`Grid` Star、`TextTrimming` 优先级）在 WPF 里有**可运行**的权威实现（UWP 的布局就是它演化来的） | 同上，写探针：构造 `StackPanel`/`Grid`/`Border` 组合，打印 `DesiredSize`/`ActualWidth`/`RenderSize`，回答「子溢出时父怎么夹」「无限约束怎么传」 | **M2-1 的 `arrange 强制 Constraints::max`**、M2-5 的 `child ⊆ parent` 断言到底该断言什么 | 中 |
| B3 | **`arc-deck` 双实现对照** | 同盘仓 `C:\Users\mc158\Documents\Projects\arc-deck` 是 **UWP/WinUI 2（C#）与 Rust 双实现** —— 正是 2026-09-22 成熟度审计的**起因**（「在 arc-deck 里感到 C# 那边更舒服」）。同一屏两种实现并存，是最直接的"差在哪"证据 | 逐屏对照：C# XAML 写法 vs Rust 控件写法，列出「C# 一行、Rust 几十行」的具体清单 | **审计的起因闭环**；M1/M2/M3 该优先做什么有了实证排序 | 中 |
| B4 | **WinRT 运行时真值（accent / 主题 / 动效开关）** | `SystemAccentColor*` 从不写在任何 XAML 里（运行时注入）——**Linux 上无法取**。本机 `UISettings.GetColorValue` 已实测可用（经 `powershell.exe` 5.1，pwsh 7 无 WinRT 投影） | 见下方脚本；可扩展读 `UIElementColor`、`AnimationsEnabled`、`HighContrast`、系统深浅色 | **T2/T9 与"档位对齐 Win10"这句声明**（见下）；`system_theme.rs` 的对齐 | 小 |
| B5 | **Windows 原生观感度量** | 设计参考（非权威）：Win11 标题栏/任务栏/控件圆角与字号梯度，可与 Ether 的 TopBar 30px / Dock 对齐做对照 | 读注册表/主题资源 + 截图量测 | 设计正典的参考值（`KANESUMI_DESIGN.md` 可补一节"与 Windows 原生对照"） | 小 |

### B4 已兑现的结果：**我们的 accent 档位与 OS 不一致**

实测脚本（可复现；注意必须走 `powershell.exe` 5.1，pwsh 7 无 WinRT 投影）：

```powershell
powershell.exe -NoProfile -Command @'
[void][Windows.UI.ViewManagement.UISettings, Windows.UI.ViewManagement, ContentType=WindowsRuntime]
$s = New-Object Windows.UI.ViewManagement.UISettings
foreach ($t in "Accent","AccentDark1","AccentDark2","AccentDark3","AccentLight1","AccentLight2","AccentLight3") {
  $c = $s.GetColorValue([Windows.UI.ViewManagement.UIColorType]::$t)
  "{0,-14} #{1:X2}{2:X2}{3:X2}" -f $t, $c.R, $c.G, $c.B
}
'@
```

本机实测（系统默认蓝 `#0078D4`）：

| 档位 | **OS 真值** | 本仓 `Accent`（RGB lerp 向白/黑） | 差 |
|---|---|---|---|
| Light1 | `#0091F8` | `#409ADF` | 明显：OS **保持饱和**，我们向白洗成灰调 |
| Light2 | `#4CC2FF` | `#73B5E7` | 明显 |
| Light3 | `#99EBFF` | `#A6D0F0` | 明显 |
| Dark1 | `#0067C0` | `#0060AA` | 明显 |
| Dark2 | `#003E92` | `#004E8A` | 明显 |
| Dark3 | `#001A68` | `#003C6A` | 明显 |

**反推 OS 的算法 = HSV 的「明度 V」缩放**（四点精确吻合，误差 0）：

```
Light1 = V×1.17   ┐ 实测 V=0.83137：×1.17=0.97271 → 248（OS Light1 最大分量 248 ✓）
Dark1  = V×0.905  ┤              ×0.905=0.75239 → 192（OS 192 ✓）
Dark2  = V×0.689  ┤              ×0.689=0.57282 → 146（OS 146 ✓）
Dark3  = V×0.49   ┘              ×0.49 =0.40737 → 104（OS 104 ✓）
Light2 = V×1.56、Light3 = V×1.925 —— 越界后 V 饱和到 1，同时**饱和度下降**（OS Light2 S≈0.70、Light3 S≈0.40）
```

⇒ 本仓 `accent.rs` 头部写「档位对齐 Win10 `SystemAccentColor` 的 Light1~3 / Dark1~3」**不成立**
（我们用的是 `LIGHT_MIX=[0.25,0.45,0.65]` / `DARK_MIX=[0.20,0.35,0.50]` 的 RGB lerp）。
处置已登记为 `CANON_VS_TEMPORARY.md` **T20**：要么按上面的 V 缩放模型改（`accent.rs` 一处 +
以本表为回归数据，约 30 分钟），要么把声明删掉、明确标注为 Kanesumi 自定档位。
> 注意：改档位会同时改变所有 hover / pressed / focus / on-accent 派生色，属视觉变更，
> 需一次真机确认（与 T9 同批做最省事）。

---

## §C 兄弟仓（同盘克隆，Windows 侧可读）

| 仓 | 与本项目的关系 | 建议 |
|---|---|---|
| **`arc-deck`** | **UWP/WinUI 2 + Rust 双实现** —— 成熟度审计的起因；design reference 与坑都在里面 | **优先**：B3 |
| **`Sakichan`** | sec-a 的 `MetroTextField`/`MetroDrawer`/`MetroChatInputBar` 自述「来自 Sakichan 实战」—— 输入条/抽屉的**真实用例**来源 | 高：回答「输入框到底该怎么用」（我们是否要 `filled()` 变体、多行范式） |
| **`Ncrust`** | sec-a 要替换 Material 的目标工程，也是其控件 parity 表的来源 | 中：确认控件需求清单（与 `CONTROL_MATRIX.md` 对照） |
| **`PezMax-One`** | `kanesumi-appmenu` 的参考实现（`src/app_menu`）+ Sokuou 同名 API 的出处 | 中：appmenu 的边界情况、D-Bus 细节 |
| **`Sokuou`** + **`sokuou-engine-toolkit`** | 动画引擎上游（本仓以 submodule 消费）+ 一个 toolkit（可能有可视化/回归工具） | 中：M6 动画词汇补齐时可参考/复用 |
| **`kanesumi-studio`** | 名字像设计/预览工具，尚未看过 | 低-中：可能正是"看设计"的地方，值得 10 分钟侦察 |
| **`temp-dsh-refs`** | 疑似参考仓转储 | 低：看一眼是否含合成器参考 |
| `Ritsudou` / `zethora-engine` / `DreamForge` / `brainhorse` / … | 与本项目无直接关系 | 忽略 |

---

## §D 工程与验证基建

| # | 内容 | 为什么现在做 | 怎么做 | 工作量 |
|---|---|---|---|---|
| D1 | **Ether monorepo 在 Windows 上的 `cargo check --workspace`** | 抓「Linux-only 泄漏进 shared crate」；非 Linux 会编 stub，正好能验 `cfg` 门控 | 在 `C:\Users\mc158\Documents\Projects\Ether` 直接跑 | ✅ **本次已做**，结果见下 |
| D2 | **一键验证脚本**（clippy 基线对比 + 全量测试 + 测试字体检查） | 本次手工做了三遍「clippy 与基线逐条一致」的对比，值得固化成脚本（逻辑可移植回 Linux） | 写 `shared/kanesumi/scripts/verify.ps1`（+ 等价的 .sh） | 小 |
| D3 | **字体清单落档** | 本仓测试首次在 Windows 跑通，依赖雅黑/Segoe UI 路径；换机器会再踩 | 把「本机可用测试字体 + `KANESUMI_TEST_FONT` 用法」写进 `CLAUDE.md`（部分已做） | 小 |
| D4 | **上游同步检查**（Kanesumi / Sokuou 远端是否有新提交） | 网络可用时顺手看一眼 | `git -C <repo> fetch && git log --oneline HEAD..origin/main` | 小 |

### D1 实测结果（2026-09-22）

- ✅ **文档化的跨平台门全绿**：`cargo check -p ether-types -p sokuou -p ether-assets -p ether-ui -p ether-protocol -p ether-manifest`
  → `Finished`，**shared 六个 crate 无 Linux 泄漏**。
- ⚠️ **`cargo check --workspace` 恰好只有一个成员编不过：`ether-settings`**（13 处错误：
  `zbus` / `chrono` / `serde_json` 未解析、`std::os::unix` 不存在）。原因不是泄漏，
  而是它的 Linux-only 依赖写在 `Cargo.toml` 的 `cfg(target_os = "linux")` 里、
  **源码没有对应的 stub 门** —— 即根 `CLAUDE.md` 的「非 Linux 编 stub」约定在它身上不成立。
  影响：Windows 侧的类型检查覆盖不到 settings（Librarian 等同样 Linux-only 的二进制同理）。
  可选改进（低风险、属 Ether 侧改动）：给这类 crate 加非 Linux 的空 lib 存根，
  让 `cargo check --workspace` 在任意平台都能当"编译闸"用。

---

## §E 设计参考测量（可选）

| # | 内容 | 说明 |
|---|---|---|
| E1 | Win11 原生度量（标题栏高、任务栏高、控件圆角、字号梯度） | 与 Ether TopBar 30px / Dock 对照；`KANESUMI_DESIGN.md` 可补「与 Windows 原生对照」一节（**非权威**，仅参考） |
| E2 | 系统应用的像素对照（设置/计算器） | 需运行系统应用并截图量测；可选，价值低于 A/B 两组 |

---

## §F 本次（2026-09-22）已完成

| 项 | 结果 | 提交 |
|---|---|---|
| OS UWP 主题字典 + 控件模板取数 | 十处旧值被推翻并更正；新建 `docs/UWP_PRIMARY_SOURCES.md`（含路径、行号、取数命令、未解决项） | `af43f8e` 及之前 8 个 fix |
| 令牌/控件/列表/动画四族原始取数 | `docs/research/2026-09-22_{tokens,list_states,controls,animation}.md`（247 KB，每条带 `文件:行号`） | `af43f8e` |
| WinUI 2.x 源码字典（网络） | 关掉 T4/T5/T11/T14，新增 T17~T19 | `af43f8e` 等 |
| 跨扇区对照（Android sec-a） | `docs/SECA_SYNC_2026-09-22.md` + 三份报告；回流 3 项、修 1 个真 bug | `1779a87`…`dc8de67` |
| Windows 侧测试通道 | 字体查找补 Windows 路径（此前 gallery 33 红 / calculator 2 红 / 3 处静默跳过） | `b19d924` |
| **WinRT accent 真值** | 取到系统全档位并反推出 OS 算法（§B4）→ 登记 T20、改正 `accent.rs` 的虚假声明 | 本轮提交 |
| **monorepo Windows 编译闸实测** | shared 六件套干净通过；仅 `ether-settings` 因缺 stub 编不过（§D1） | 本轮提交 |
| 本机资产盘点 | 本文件 §0（含「WSL 未装 ⇒ 本机无 Linux 侧执行」这一关键约束） | 本轮提交 |

---

## §G 若只剩最后一次 Windows 窗口：建议顺序

按「价值 ÷ 成本」排序，**前五项约一个工作日**：

1. **B3 `arc-deck` 双实现对照** —— 它是成熟度审计的起因，闭环价值最高；纯读代码，零环境依赖。
2. **B4 的处置**（T20：accent 档位按 OS 模型改 or 改声明）+ **B1/B2 探针**（dotnet 在，写两个小程序即可拿到 DirectWrite 与 WPF 布局的标准答案）。
3. **A1 + A2 + A3**（WinUI 3 令牌 + 尺寸常量 + 字号梯度）—— 全是机械抓取，可脚本化，一次把 `CONTROL_SPEC` 的「未在快照」清掉大半。
4. **C 组的 `Sakichan` / `kanesumi-studio`** 快读（各 15 分钟），补齐"输入框/抽屉从哪来"与"有没有设计预览工具"。
5. **D1 + D2**：monorepo Windows check + 验证脚本落档。
6. 余下按 A4 → A7 → A5 → A9 → A6 推进（A6 成本最高，且当前非阻塞）。
