# Kanesumi 构图契约

本文定义可变窗口、缩放、字形与本地化条件下的稳定构图规则。它约束 Runtime API，
不改变 `KANESUMI_DESIGN.md` 的视觉语言。

## 平台依据

- UWP / WinUI：递归 `Measure(availableSize)` → `DesiredSize` → `Arrange(finalRect)`；
  使用有效像素、布局舍入、Min/Max、Auto/Star、VisualState 断点与文本换行/裁切/省略。
- macOS AppKit：点坐标、Auto Layout、intrinsic content size、content hugging 与
  compression resistance；SwiftUI 以 proposed size → chosen size → placement 表达同类契约。
- Wayland：逻辑 surface-local 坐标与 buffer 像素分离；分数缩放使用
  `wp_fractional_scale_v1` 建议比例和 `wp_viewport` 逻辑目标尺寸。

权威资料：

- <https://learn.microsoft.com/en-us/windows/apps/develop/ui/layouts-with-xaml>
- <https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.frameworkelement.measureoverride>
- <https://learn.microsoft.com/en-us/windows/apps/design/layout/responsive-design>
- <https://learn.microsoft.com/en-us/windows/windows-app-sdk/api/winrt/microsoft.ui.xaml.controls.textblock.texttrimming>
- <https://developer.apple.com/library/archive/documentation/UserExperience/Conceptual/AutolayoutPG/AnatomyofaConstraint.html>
- <https://developer.apple.com/documentation/appkit/nsview/intrinsiccontentsize>
- <https://developer.apple.com/documentation/swiftui/proposedviewsize>
- <https://wayland.app/protocols/fractional-scale-v1>
- <https://wayland.app/protocols/viewporter>

## 强制契约

1. **逻辑单位唯一**：布局只使用 `f32` 逻辑像素。物理缩放只进入栅格化；不得按显示器
   分辨率分支，不得在中间布局步骤取整。
2. **约束先于尺寸**：父节点给 `Constraints { min, max }`，子节点返回约束内尺寸；
   非法位置归零，NaN/负尺寸归零，正无穷只表示 Measure 无界轴。
3. **一次构图，多方消费**：Arrange 结果同时驱动绘制、命中、弹层锚点与 IME caret；
   禁止为命中再写一套常量坐标。
4. **内在尺寸可压缩**：主轴不足时按 shrink/compression resistance 分配；剩余空间按
   grow/Star 分配。固定尺寸只用于协议或硬件规定的几何。
5. **溢出显式**：文本必须声明 wrap、max lines 与 `Clip`/`Ellipsis`。每个文本框天然是
   paint clip；布局矩形本身不允许内容改变兄弟位置。
6. **裁剪有栈语义**：只允许 `PushClip`/`PopClip` 成对使用；空交集保持空。GPU scissor
   负责裁剪，禁止先切几何后重新圆角化或把图像可见片段映射到完整 UV。
7. **文字按 run 塑形**：BiDi、脚本、OpenType shaping、fallback、字素簇与断行属于同一
   管线。Measure 和 Paint 必须消费同一结果；glyph cache 身份含 font ID、glyph ID、精确字号。
8. **按适配度重排**：普通内容通过测量与重排适配；结构变化才使用逻辑宽度断点。
   Settings 当前在 720 逻辑像素切换展开/紧凑导航，不依赖物理分辨率。
9. **缩放按 surface 持有**：主表面和每个浮层分别响应建议比例。支持分数缩放时
   `buffer_scale=1`，buffer 尺寸为 `logical × scale`，viewport destination 为逻辑尺寸。
10. **缓存包含环境**：边界、主题、字体、locale、方向、UI scale 或 text scale 变化都使
    布局/绘制缓存失效。

## 验收矩阵

构图改动至少验证：`320×240`、`480×320`、`640×480`、`960×640`、`1920×1080`；
1.0 / 1.25 / 1.5 / 2.0 scale；长 CJK、拉丁、阿拉伯文、组合附标与 emoji；强制 RTL；
所有绘制和命中矩形不得越过祖先裁剪，单行标签须省略而不是覆盖相邻控件。
