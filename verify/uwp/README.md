# Kanesumi · UWP 验证工程

> **这里不是产品，是取数用的隔离工程。**

## 角色

在真 UWP（C# + WinUI 2）上跑 Kanesumi 的控件与动画，**实测**官方默认值、
真实视觉状态机与边界情况，然后把结论**回流到主仓的规格文档**：

```
verify/uwp  ──实测──▶  ../../docs/CONTROL_SPEC.md
                       ../../docs/ANIMATION_SPEC.md
                       ../../docs/UWP_FEEDBACK.md（暂存 → 回流）
```

参 `../../docs/PORT_ROADMAP.md §Ⅰ C`（规格来源 C：实证）。

## 为什么不另开仓

**规格是资产，跑规格的工程不是。**

三个平台若各自持有一份规格副本，必然漂移 —— 而这正是 `PORT_ROADMAP` 要防的事。
所以本工程只做取数，结论一到手就回流，不在这里积累状态。

## 技术栈

**C# + WinUI 2 (`Microsoft.UI.Xaml` 2.8.6) + UWP**（target 10.0.22621，min 10.0.17763）。

### 为什么是 C#

| 语言 | 稳定度 | 性能 | Win 生态贴合 | 判定 |
|---|---|---|---|---|
| **C#** | **最高** —— WinUI 2 一等公民，微软全部文档与示例语言 | 足够 | **最高** | ✅ |
| C++/WinRT | 高，但引用计数 / 协程生命周期负担重 | 最高 | 高 | ❌ UI 层过度代价 |
| Rust `windows-rs` | **低** —— 无 WinUI 2 绑定；官方投影面向 WinUI 3 且实验性 | 高 | 中 | ❌ 不成立 |

**性能不是选 C++ 的理由**：UWP 的布局、渲染、文本排版全在原生 XAML 层完成，
C# 只驱动状态与事件，托管代码不在每帧热路径上。

### 为什么是 WinUI 2 而不是 WinUI 3

WinUI 3 是 **Fluent** —— 圆角、Mica/Acrylic、阴影，与 Kanesumi Design
**对立**（正典原话：Fluent 仅作反面教材）。Kanesumi 的技术参考是
**Metro / UWP 时代（Win8 – Win10 1709）**，对应的是 **WinUI 2**。

## 圆角归零（可行性依据）

UWP 的圆角由**两个全局资源**控制，覆盖即得直角，无需重写控件模板：

```xml
<CornerRadius x:Key="ControlCornerRadius">0</CornerRadius>  <!-- 默认 4px -->
<CornerRadius x:Key="OverlayCornerRadius">0</CornerRadius>  <!-- 默认 8px -->
```

依据 [Geometry in Windows](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/geometry)：
*"The default corner radii are controlled by two global resources …
You can override these values in your App.xaml to change the rounding
across all controls in your app."*

已在 `src/Kanesumi.SecW/App.xaml` 落地，并在注释里写明为什么在声明层一次做完，
而不是逐控件设 `CornerRadius="0"`（逐控件必然遗漏）。

## 规矩

1. **发现即记录，不要攒着** —— 攒到最后必然丢失。
2. **以 Kanesumi 正典为准，不以 UWP 默认为准。** UWP 默认带 Fluent 残留，
   而 Kanesumi 的偏离是刻意的；不写下来，半年后会被当成笔误改掉。
3. **不凭文档或记忆填规格表。** `ANIMATION_SPEC.md` 的 UWP 时长表故意留空，
   等实测。那份表会被多平台实现照做，填错比留空代价大。
4. **注释写 why，不写 what**；关键决策记录促成它的那个 bug 或那次实测。

## 构建

需要 **MSBuild**（UWP 不能用 `dotnet build`）：

```powershell
$msbuild = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe"

# 还原
& $msbuild src\Kanesumi.SecW\Kanesumi.SecW.csproj /t:Restore /p:Configuration=Debug /p:Platform=x64

# 构建（产出 exe + msix）
& $msbuild src\Kanesumi.SecW\Kanesumi.SecW.csproj /p:Configuration=Debug /p:Platform=x64

# 图标（正典配色，生成物不入库）
dotnet run --project tools\make-icons -- src\Kanesumi.SecW\Assets
```

环境（已核实）：MSBuild **17.14.51** @ BuildTools · UWP XAML 目标已装 ·
VC 14.44.35207 · Windows SDK 10.0.22621/26100 · .NET SDK 9.0.316。
**无 VS IDE** —— 构建走 BuildTools 的 MSBuild。

## 踩过的坑（记下来免得重踩）

| 坑 | 现象 | 处置 |
|---|---|---|
| WinUI 2 新增控件不在默认 XAML 命名空间 | `XamlCompiler error WMC0001: Unknown type 'InfoBar'` | 加 `xmlns:muxc="using:Microsoft.UI.Xaml.Controls"`，用 `muxc:InfoBar` |
| `Properties/Logo` 走 **StoreLogo** 校验 | `APPX1619: 必须为 50x50 像素` | 它是独立一项，与 `Square150x150Logo` 不同。单独出 50×50 图 |
| `dotnet build` 不支持 UWP | —— | 必须用 MSBuild |

## 目录

```
src/Kanesumi.SecW/
├── App.xaml                    圆角归零 + 资源装配
├── Themes/KanesumiTokens.xaml  正典落地 token（色板 / 字号 / 间距）
├── MainPage.xaml               **控件验证台** —— 逐控件核验直角是否生效
└── Package.appxmanifest
tools/make-icons/               图标生成（正典配色，生成物不入库）
```

## 当前状态

| 项 | 状态 |
|---|---|
| 工程骨架 + WinUI 2 引用 | ✅ 构建通过，产出 MSIX |
| 圆角全局归零 | ✅ 已写入 `App.xaml`，待逐控件目视核验 |
| 正典 token 字典 | ✅ 色板 / 字号 / 间距 |
| 控件验证台 | ✅ 已建，待核验并填写 `docs/UWP_FEEDBACK.md` |
| 实测回流 | ⏳ **核心职责，进行中** |

## 待办

- [ ] 逐控件核验：哪些控件未走那两个圆角资源
- [ ] 实测 `ANIMATION_SPEC.md §Ⅴ` 里留空的 UWP 时长键
- [ ] 实测 IME 组合串的视觉规格（正典空白项）
- [ ] 结论回流 `docs/CONTROL_SPEC.md` / `docs/ANIMATION_SPEC.md`

