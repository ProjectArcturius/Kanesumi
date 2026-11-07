# 菜单（menu）参考实现：KDE / Qt 是怎么处理的

结论先说：菜单**不是一个控件**，是四件东西叠出来的 —— 无装饰弹层表面、抓取（grab）、条目模型、
以及一个「时序 + 安全区」状态机。我们的菜单「缺斤少两」，缺的是第四件：状态机与长菜单的兜底。

本文的每个数值都来自源码（本地 sparse clone，路径见各条），不是推测。参 `CONTROL_SPEC.md` §51
（表格行，同样是先扒成熟实现再定契约的做法）。

## 一、时序与状态机（手感的主体）

| 行为 | KDE / Qt 真值 | 出处 |
|---|---|---|
| 子菜单悬停展开延时 | **150 ms**（Breeze）；Qt 默认 256 ms | `breeze/kstyle/breezestyle.cpp:931`；`qtbase/src/widgets/styles/qcommonstyle.cpp:5077` |
| 悬停高亮 | 立即切换，但**指针没进过菜单前不算**（`hasHadMouse`） | `qtbase/src/widgets/widgets/qmenu.cpp:3497` 附近的 `mouseMoveEvent` |
| 打开瞬间的合成移动 | 用它顶掉：`enterEvent` 里 `motions = -1`，且 `motions > 6` 才当作真实移动 | `qmenu.cpp:3535`、`qmenu.cpp:1585` |
| 安全三角（sloppy submenu） | **有**：指针离开父项后子菜单仍开着，只要朝子菜单方向走；超出三角才关 | `qmenu_p.h:92` `QMenuSloppyState`、`qmenu_p.h:165 processMouseEvent`、斜率比较在 `qmenu_p.h:202-240` |
| 关闭宽限 | **1000 ms**（`SH_Menu_SubMenuSloppyCloseTimeout`） | `qcommonstyle.cpp:5094` |
| 单向模式 | 关（`SH_Menu_SubMenuUniDirection` = false，失败计数 1） | `qcommonstyle.cpp:5085-5088` |
| 长菜单 | **可滚动**：`SH_Menu_Scrollable = true` + `QMenuScroller`（上下箭头 + 滚动区） | `breezestyle.cpp:927`、`qmenu.cpp` 的 `drawScroller` |
| 键盘 | 方向键 / Home / End / PageUp / PageDown / Esc / Return，**外加类型搜索**（`SH_Menu_KeyboardSearch`） | `qmenu.cpp:3402` 的 `keyPressEvent` |
| 分区标题 | 支持（`SH_Menu_SupportsSections = true`） | `breezestyle.cpp:940` |
| 光标跟踪 | 开（`SH_Menu_MouseTracking`）：菜单一出现，指针下的项就高亮 | `breezestyle.cpp:929` |

这一条要分清「指针打开」与「键盘打开」：QMenu 只在指针真的进过菜单后才高亮（`hasHadMouse`），
键盘打开才预选。**我们这块已经做对了**：`menu_flyout.rs:51` 与 `:72`、`context_menu.rs:62` 都是
`if keyboard` 门控，`dropdown_menu.rs:188` 初值为 `None` —— 改造时别把它改成无条件预选。

## 二、位置：翻转 + 按行高整数倍避让

`plasma-workspace/applets/kicker/submenu.cpp` 的 `SubMenu::popupPosition`：

- 左右：先算 `xPopupRight`（弹在父项右侧）与 `xPopupLeft`（弹在左侧），**放不下才翻**，并记住
  `m_facingLeft` 供箭头朝向用。
- 垂直：超出屏幕时不是像素级平移，而是
  `overshoot = ceil(超出量 / 行高) * 行高`，整体上移整数行 —— **菜单项永远不会被屏幕边缘切半**。

这一条比「简单上翻」好用的多，也解释了为什么 KDE 的菜单边缘总是干净的。

## 三、条目度量（Breeze）

`breeze/kstyle/breezehelper.cpp`（`MenuItem_*` 常量）：

```
MarginWidth 4   MarginHeight 4   HighlightGap 4   ExtraLeftMargin 4
ItemSpacing 2   TextLeftMargin 8  AcceleratorSpace 16
```

含义：菜单内边距与外边距都用 4（`PM_MenuHMargin`/`PM_MenuVMargin` 就是 `HighlightGap`，即高亮块
离面板边缘 4），条目之间 2，勾选框 / 图标列与文字之间 8，快捷键列固定让出 16。行高由
`menuItemSizeFromContents`（`breezestyle.cpp:3607`）按字体 + `MarginHeight` 算，不写死。

## 四、内容从哪来（这才是「给 dock / 应用匣 / 磁贴加右键菜单」的正解）

KDE 不把菜单项写死，而是「**能力角色 + 请求**」：

- `plasma-workspace/libtaskmanager/abstracttasksmodel.h` 给出能力角色：`IsClosable`、
  `IsMinimizable`、`IsMinimized`、`CanLaunchNewInstance`、`IsHidden`…（`:56-93`），
  动作走 `requestClose` / `requestToggleMinimized` 这类**请求**方法（`:149-185`）。
- 菜单界面（`plasma-desktop/applets/taskmanager/qml/ContextMenu.qml`）只读能力角色决定「这一项该
  不该出现」，再调请求方法。同一份模型同时服务任务栏、Dock、窗口列表。
- 应用自身声明的动作来自 `.desktop` 的 `Actions=` 组（freedesktop 规范），不是我们去猜。

对我们的含义：Dock 图标 / 顶栏应用匣 / Launcher 磁贴的右键菜单必须建在
「`foreign-toplevel` 状态 + `.desktop` `Actions=`」这两份数据上的**能力判定**，三处共用一份
「应用菜单条目」生成器，而不是每处各写一串固定菜单项。

跨进程菜单位置：`plasma-workspace/gmenu-dbusmenu-proxy/menu.cpp` 把 GTK 的 DBusMenu 导入成
QMenu，说明「别人的菜单」在 KDE 里也是**换成自己的菜单控件**再显示，语义由我们掌控。

## 五、我们的差距清单（对照 `kanesumi-controls` 现状）

先记**已经做对、别改坏**的三条：指针打开不预选（`if keyboard` 门控）、Home / End 已处理、
Esc / Enter / Space 语义已在（`dropdown_menu.rs:1095`）。

| # | 缺的东西 | 现状 | 补法 |
|---|---|---|---|
| 1 | 子菜单 150 ms 延时 | 无：悬停即开（`dropdown_menu.rs:444-447` 直接 `open_submenu`） | 加悬停计时器，常量进 `sizes` |
| 2 | 安全三角 + 1 s 宽限 | 无 | 照 `QMenuSloppyState` 实现（斜率判定 + 超时 + `motions` 抑制） |
| 3 | 长菜单滚动 | 无：`place_popup` 把面板高压到屏高（`popup.rs:38`），超出的条目没有滚动兜底 | 菜单内滚动区 + 上下箭头 |
| 4 | 类型搜索 / PageUp / PageDown | 无（只有 Up/Down/Left/Right/Home/End 与 Space） | `dropdown_menu.rs` 键盘分支补齐 |
| 5 | 分区标题 | 只有分隔线 | 加 `MenuItem::section(title)` |
| 6 | 位置按行高整数倍避让 | `place_popup` 只翻转 + 夹紧，像素级 | 垂直避让量化到行高 |
| 7 | 右键菜单覆盖面 | dock / 应用匣 / 磁贴 / 桌面空白 均无 | 先做「应用菜单条目生成器」（`.desktop Actions=` + 窗口能力） |

## 六、落地顺序（建议）

1. 先补 1~2（纯库侧、不新增控件，是「好用」的最大杠杆）。
2. 再补 3~5（长菜单与键盘，决定「能不能靠它干活」）。
3. 6 归合成器（`state/popup.rs`），与 1~5 可并行。
4. 7 归应用侧，等 1~6 稳了再铺三处入口，避免在坏地基上写六份菜单。

数值落地时同步进 Ether 仓 `docs/INTERACTION_CANON.md` §Ⅵ（现在那一节只有四行，没写延时、
安全三角与长菜单兜底）。

## 附：源码从哪来（可复现）

本文的 sparse clone 在 Ether 仓 `target/ref-kde/`（`target/` 已在忽略列表内）：

```
git clone --depth 1 --filter=blob:none --sparse https://github.com/KDE/breeze.git
git -C breeze sparse-checkout set kstyle
git clone --depth 1 --filter=blob:none --sparse https://github.com/KDE/plasma-workspace.git
git -C plasma-workspace sparse-checkout set libtaskmanager applets/kicker gmenu-dbusmenu-proxy
git clone --depth 1 --filter=blob:none --sparse https://github.com/KDE/plasma-desktop.git
git -C plasma-desktop sparse-checkout set applets/taskmanager
git clone --depth 1 --filter=blob:none --sparse https://github.com/qt/qtbase.git
git -C qtbase sparse-checkout set src/widgets/widgets src/widgets/styles
```

