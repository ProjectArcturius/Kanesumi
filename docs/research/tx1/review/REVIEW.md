# tx1 调度者审阅（2026-10-04）

对照表（2× 13N 行 3× 放大，自上而下：base / ctr / gam / stem25 / both-md / both-hi / FreeType）：
`sheet_light.png`、`sheet_dark.png`。

与工人报告推荐（both-md）不同的读图结论：

1. **base ≈ FreeType**：2× 下我们的光栅与 FreeType（Pillow，含 CFF 默认笔画加粗）几乎同图。
   kitty 在 1× 的锐利来自 hinting；2× 的「绵软」不能靠「对齐 FreeType」解决。
2. **掩码膨胀式加粗（stem25 / both-*）产生灰色光晕，边缘发虚**——不可进生产。笔画加粗若要做，必须在**轮廓**上做（沿法线外扩），边缘才干净。
3. both-hi 过重（13px 已像 Bold）；both-md 变实但带膨胀光晕。
4. ctr / gam 干净但提升小。

方向：浓度优先来自**真字重**（Noto Sans CJK 自带 DemiLight / Regular / Medium / Bold，设计好的笔画边缘锐利）+ 对比度 / gamma；
裁定 N-41（正文 Normal）不由调度者改动，出样张请用户选。后续证据任务 `tx2-weight-outline`。

## tx2 审阅（2026-10-04 17:10）

1:1 页（`docs/research/tx2/rust/onesheet_light.png` / `onesheet_dark.png`，2× 物理像素不放大）读图：
**只有真字重 Medium（med / med-cg）在实际尺寸下一眼可见地「强劲」且锐利**；对比度 / gamma 与轮廓加粗（reg-cg / ol15 / ol30 / ol15-cg）
在 1:1 下与 base 差别很小，只在放大图上可辨；掩码膨胀（dil25）仍发虚。
结论：**浓度的主杠杆是字重**，渲染层补偿是次要的。工人在不改 N-41 前提下首推的 ol15-cg 在 1:1 下提升有限。
建议用户裁定：正文 Medium（+ contrast 0.5 / gamma 1.4），即修订 N-41；Win10 截图到后复核。
