# Windows 参照样张（tx1，调度者生成于 2026-10-04）

- 生成：Windows 11 本机，PowerShell + System.Drawing（**GDI+ `Graphics.DrawString`**；非 GDI `TextRenderer`、非 DirectWrite）。
- 字体：`Microsoft YaHei UI`（Windows 10 中文界面实际字体，拉丁部分源自 Segoe）一行 + `Segoe UI` 纯拉丁一行；本机无思源 / Noto CJK。
- 字号：逻辑 13 / 15 / 20 / 28 px，`_1x` 为同像素、`_2x` 为 ×2 物理像素；深底 `#1F1F1F` 白字、浅底白底黑字。
- 模式：`cleartype` = `TextRenderingHint.ClearTypeGridFit`（亚像素 + 网格对齐，即 hinting）；`grayscale` = `AntiAliasGridFit`。
- `zoom3_*` = 左上角裁切 3× 最近邻放大。
