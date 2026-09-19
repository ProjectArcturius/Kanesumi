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
& $msbuild src\Kanesumi.SecW\Kanesumi.SecW.csproj /t:Restore /p:Configuration=Release /p:Platform=x64

# 构建（产出 exe + msix）
& $msbuild src\Kanesumi.SecW\Kanesumi.SecW.csproj /p:Configuration=Release /p:Platform=x64

# 图标（正典配色，生成物不入库）
dotnet run --project tools\make-icons -- src\Kanesumi.SecW\Assets
```

**用 Release 配置，不要用 Debug。** 两个原因：

1. Debug 的 .NET Native 依赖 `Microsoft.NET.Native.Framework.**Debug**.2.2` ——
   调试运行时通常未随系统预装，导致激活失败。
2. Release 才是 UWP 应用的发布形态，也是唯一值得验证的形态。

csproj 里已经设好的两处非默认项，**不要删**：

| 设置 | 为什么 |
|---|---|
| `<LangVersion>10.0</LangVersion>` | UWP 默认 7.3，现代写法（`is not`、switch 表达式）会报 `CS8370` |
| `<UseDotNetNativeToolchain>true</UseDotNetNativeToolchain>` | 让应用自包含，摆脱对 .NET CoreRuntime 框架包的依赖 |

环境（已核实）：MSBuild **17.14.51** @ BuildTools · UWP XAML 目标已装 ·
VC 14.44.35207 · Windows SDK 10.0.22621/26100 · .NET SDK 9.0.316。
**无 VS IDE** —— 构建走 BuildTools 的 MSBuild。

## 踩过的坑（记下来免得重踩）

### 编译期

| 坑 | 现象 | 处置 |
|---|---|---|
| WinUI 2 新增控件不在默认 XAML 命名空间 | `WMC0001: Unknown type 'InfoBar'` | 加 `xmlns:muxc="using:Microsoft.UI.Xaml.Controls"`，用 `muxc:InfoBar` |
| **但系统控件在 muxc: 里也不存在** | `WMC0001: Unknown type 'AutoSuggestBox'`（用 `muxc:` 前缀时） | `AutoSuggestBox` / `Button` / `TextBox` / `ListView` 是 UWP 自带，用**默认**命名空间。分界线只能靠编译器确认 |
| C# 语言版本默认 7.3 | `CS8370: 「not 模式」在 C# 7.3 中不可用` | csproj 加 `<LangVersion>10.0</LangVersion>`。报错位置离真实原因很远 |
| `NavigationView` 有两份 | 生成代码报「没有与委托匹配的重载」 | `Windows.UI.Xaml.Controls` 与 `Microsoft.UI.Xaml.Controls` **都有** `NavigationView`。事件处理器参数必须完全限定用后者 |
| `Properties/Logo` 走 **StoreLogo** 校验 | `APPX1619: 必须为 50x50 像素` | 它是独立一项，与 `Square150x150Logo` 不同。单独出 50×50 图 |
| 清单缺 `PhoneIdentity` | `APPX1673: 缺少必需元素 PhoneIdentity` | 桌面清单里它也是必需元素 |
| `dotnet build` 不支持 UWP | —— | 必须用 MSBuild |

### 部署期（尚未在本机跑通，见下）

| 坑 | 现象 | 说明 |
|---|---|---|
| UWP 不能直接运行 exe | 进程起来随即崩溃 | 必须经包注册/安装后由 Activator 启动 |
| `Add-AppxPackage -Register` 松散文件 | 注册**成功**，但激活报 `0x8027025B 应用未启动` | 松散注册不保证 AppContainer 可运行 |
| Debug 配置的 .NET Native | 依赖 `Microsoft.NET.Native.Framework.**Debug**.2.2` | 调试运行时通常未预装。要跑用 **Release** |
| MSIX 自签名安装 | `0x800B0109: 根证书必须是受信任的证书` | 需把测试证书装进**受信任的根**，而那只写 `CurrentUser\Root` 需要**管理员权限** |

## 已知问题：本机无法启动已构建的应用

**代码是完整的、能编译、产出可安装的 MSIX。但在此开发机上启动失败。**

排查过程与结论（都做了对照实验，不是推测）：

| 实验 | 结果 |
|---|---|
| 本工程（完整应用） | 激活 `0x8027025B` |
| **纯空白 UWP 应用**（不含本工程任何代码） | **同样 `0x8027025B`** |
| **系统自带计算器**（同一探针、同一方式） | **启动成功，88 MB** |
| 装齐 `Microsoft.NET.CoreRuntime.2.2` 等框架包 | 无改善 |
| 关闭 .NET Native 工具链 | 无改善 |
| 签名后 `Add-AppxPackage` 装 MSIX | 被证书信任链挡住（需管理员写 `Root` 存储） |

**结论：UWP 激活机制本身正常（计算器可跑），问题出在「旁加载应用的信任链」** ——
自签名证书未被信任，而安装可信根需要管理员权限，本会话没有。

这不是代码缺陷：空白应用到完整应用表现完全一致，说明触发点是**部署前置条件**而非实现。

### 在有管理员权限的机器上应该这么跑

```powershell
# 1) 用 Release 配置构建（Debug 依赖未预装的调试运行时）
$msbuild = "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\MSBuild\Current\Bin\MSBuild.exe"
& $msbuild src\Kanesumi.SecW\Kanesumi.SecW.csproj /p:Configuration=Release /p:Platform=x64

# 2) 生成并信任测试证书（需要管理员）
$cert = New-SelfSignedCertificate -Type Custom -Subject "CN=TakahashiRinta" `
    -KeyUsage DigitalSignature -FriendlyName "Kanesumi Test" `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3","2.5.29.19={text}")
Export-Certificate -Cert $cert -FilePath KanesumiTest.cer
Import-Certificate -FilePath KanesumiTest.cer -CertStoreLocation Cert:\LocalMachine\Root   # ← 需管理员

# 3) 签名并安装
& "C:\Program Files (x86)\Windows Kits\10\bin\10.0.22621.0\x64\signtool.exe" `
    sign /fd SHA256 /a src\Kanesumi.SecW\AppPackages\**\*_x64.msix
Add-AppxPackage -Path src\Kanesumi.SecW\AppPackages\**\*_x64.msix

# 4) 启动
Start-Process "shell:appsFolder\TakahashiRinta.KanesumiSecW_98kk3q0vty278!App"
```

> **排查用过的工具**：`ApplicationActivationManager` 的 COM 接口（`IApplicationActivationManager.ActivateApplication`）。
> `Start-Process shell:appsFolder\...` 只告诉你「已请求启动」，失败时给不出 HRESULT；
> 直接调这个接口才能拿到 `0x8027025B` 这类真实原因。**下次先用它。**
>
> 另：`Get-AppxPackage -Name` **不接受通配符数组**（`-Name "*a*","*b*"` 会报参数类型错误），
> 用 `Where-Object { $_.Name -match ... }` 过滤。我因此一度误判「框架包没装」。

## 目录

```
src/Kanesumi.SecW/
├── App.xaml                    圆角归零 + 资源字典装配
├── Themes/
│   ├── KanesumiTheme.xaml      **UWP 控件画刷覆盖**（深/浅/高对比三态）
│   └── KanesumiListRow.xaml    列表行模板重写（清掉默认留白）
├── ShellPage.xaml              NavigationView + Frame 外壳
├── SessionsPage.xaml           会话列表（顶栏 = 列表第一项）
├── SessionDetailPage.xaml      会话详情（工具卡片 / 代码块 / 输入框）
├── WorkspacesPage.xaml         工作区
├── ToolsPage.xaml              **控件覆盖台** —— 逐控件核验直角
├── SettingsPage.xaml           色板实况 + 关于
└── Models/                     数据模型与主题装配
tools/make-icons/               图标生成（正典配色，生成物不入库）
```

## 当前状态

| 项 | 状态 |
|---|---|
| 完整 UWP 应用（NavigationView + 5 页 + 约 16 个控件） | ✅ **编译通过，产出 MSIX** |
| 圆角全局归零（`App.xaml` 两个资源） | ✅ 已落地 |
| UWP 控件画刷覆盖成 Kanesumi 色板 | ✅ 深 / 浅 / 高对比三态 |
| 列表行模板重写（贴边） | ✅ |
| 系统标题栏配色 | ✅ 走 `ApplicationView` API |
| **启动运行** | ❌ **本机受信任链限制**，需管理员权限（见上） |
| 实测回流 `CONTROL_SPEC` / `ANIMATION_SPEC` | ⏳ 待应用能跑起来后进行 |

## 待办

- [ ] 在有管理员权限处签名部署，真正跑起来
- [ ] 逐控件核验：哪些控件未走那两个圆角资源
- [ ] 实测 `ANIMATION_SPEC.md §Ⅴ` 里留空的 UWP 时长键
- [ ] 实测 IME 组合串的视觉规格（正典空白项）
- [ ] 结论回流 `docs/CONTROL_SPEC.md` / `docs/ANIMATION_SPEC.md`

