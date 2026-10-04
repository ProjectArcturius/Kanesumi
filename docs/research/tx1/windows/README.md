# Windows 参照样张（tx1，调度者生成于 2026-10-04）

- 生成：Windows 11 本机，PowerShell + System.Drawing（**GDI+ `Graphics.DrawString`**；非 GDI `TextRenderer`、非 DirectWrite）。
- 字体：`Microsoft YaHei UI`（Windows 10 中文界面实际字体，拉丁部分源自 Segoe）一行 + `Segoe UI` 纯拉丁一行；本机无思源 / Noto CJK。
- 字号：逻辑 13 / 15 / 20 / 28 px，`_1x` 为同像素、`_2x` 为 ×2 物理像素；深底 `#1F1F1F` 白字、浅底白底黑字。
- 模式：`cleartype` = `TextRenderingHint.ClearTypeGridFit`（亚像素 + 网格对齐，即 hinting）；`grayscale` = `AntiAliasGridFit`。
- `zoom3_*` = 左上角裁切 3× 最近邻放大。

## 局限（2026-10-04 用户看图反馈）

用户评价：「和我 PC 上的 Windows 字体类似，但一点都不 Win10，没有 Win10 那种强劲」。原因判断：
1. **GDI+ 是 Windows 里最弱的文字路径**（比 GDI 淡，也不是 Win10 UWP 界面用的 DirectWrite）——这批样张不能当「Win10 标准」。
2. 本机是 Win11：默认界面字体 Segoe UI Variable（小字号光学尺寸更细）、Mica / 亚克力半透明表面迫使灰度抗锯齿；光栅算法（GDI ClearType / DirectWrite）本身仍在。
**第一参照改为真 Win10 截图**（`../win10/`，用户舍友的 Win10 笔电，PNG 原图，附分辨率与缩放比例）；本目录降为次要参照。
