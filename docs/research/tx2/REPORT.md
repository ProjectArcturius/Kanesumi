# tx2 文字浓度证据第二轮 —— 报告（2026-10-04）

> 任务 `tx2-weight-outline`。只产证据、不进生产：所有新旋钮默认关，生产路径逐像素不变。
> 承接 tx1（`docs/research/tx1/review/REVIEW.md`）：掩码膨胀产生灰色光晕、边缘发虚，**不可用**；
> 本轮改从**真字重**（Noto Sans CJK 设计笔画）与**轮廓加粗**（沿法线外扩）里找浓度。

## 结论速览

1. **轮廓加粗成立**：在轮廓进光栅器前外扩，边缘仍是干净反走样阶梯。同墨量下，
   轮廓加粗与掩码膨胀的差距在量化上清晰可见（2× 深底）——
   `dil25` 墨量 0.1072 / 边缘 1.07 / 光晕 1.28；`ol30` 墨量 0.1102（更高）/ 边缘 0.81 / 光晕 0.70。
   掩码膨胀的灰色光晕在 3× 放大下肉眼可辨（`_zoom3` / `_holes3x` 对照）。
2. **真字重是最干净的提浓**：`med`（Medium）与 `base`（Normal）边缘 / 光晕几乎不变（0.81 / 0.6 量级），
   墨量 +9.2%（0.0982 → 0.1072）；且 `med` 的墨量**恰好等于** `dil25`，但边缘 0.81 vs 1.07、光晕 0.64 vs 1.28。
3. **对比度 / gamma 保锐且降光晕**：`reg-cg` / `med-cg` / `ol15-cg` 的边缘仍 0.80，光晕 0.36~0.42（低于 base 的 0.60）。
4. 推荐见 §六；Win10 真图仍缺席（§七）。

## 一、实现

### 1.1 轮廓加粗（新旋钮 `outline_embolden_px`，默认 0）

落点 `kanesumi-canvas/src/text.rs`（轮廓提取处，任务书允许）：

- 新增 `OutlineCollector`（实现 `ttf_parser::OutlineBuilder`）：把直线 / 二次 / 三次段收集成
  **闭合折线**（字体单位，Y+ 向上）。曲线按 0.1 物理像素误差容差递归细分（de Casteljau 中点）。
- 新增 `embolden_contours(contours, d)`：逐顶点沿相邻两边法线的**斜接解**外扩
  `d·(n1+n2)/(1+n1·n2)`。环绕方向由面积最大的轮廓（外环）统一决定：外环向外、内环向内，
  非零环绕结构（「口」「器」等内框）保持。
- `FontFace::rasterize_emboldened`：先采轮廓 → 外扩 → 由外扩后轮廓**实算包围盒**得
  `GlyphMetrics` → 折线喂 `ab_glyph_rasterizer`。故 metrics 已含四周溢出，调用方无需补 pad
  （tx1 掩码膨胀需补 pad 且同步平移 metrics）。
- `TextEngine::rasterize_glyph_emboldened(..., embolden_px)` 公开入口；`embolden_px <= 0`
  或非有限时**内部短路回原 `rasterize`**，逐像素与原路径一致（守卫测试见 §八）。

**已知局限**（做法为近似，非 FreeType `FT_Outline_Embolden` 逐值复刻）：

- 斜接在 `n1·n2 → -1` 的尖角 / 回头角会发散，故夹取 `1+n1·n2 >= 0.25`，超出即退化为单位角平分方向外扩 ——
  极尖的角因此被削平。
- 外扩量相对笔画过大会产生自交 / 局部翻面；本任务档位（基准 0.15 / 0.30 px，字号曲线夹在
  [0.5, 2.0] 倍）远小于笔画宽度，样张未见自交破洞。
- 曲线被折线化后再外扩，理论上与原曲线偏移有亚像素差异；容差 0.1 px，样张不可辨。

### 1.2 旋钮与曲线

`kanesumi-harness/src/glyph_layout.rs` 的 `TextRenderTuning` 增字段 `outline_embolden_px`；
原 `stem_darken_px`（tx1 掩码膨胀）保留，仅作反例。两者的「随字号递减」曲线抽成
`darken_amount_for_size` 共用（基准 30 px，反比缩放，夹 [0.5, 2.0]）。`is_default` 纳入新字段，
默认全关 → `layout_text_glyphs` 走原路径，生产逐像素不变。`cpu_raster.rs` 的
`set_text_tuning` 与缓存清空逻辑不变（旋钮变化时清字形 / 布局缓存）。

## 二、档与样张索引

样本程序 `kanesumi-harness/examples/text_density_tx2.rs`（输出目录参数，默认 `docs/research/tx2/rust`）。
文本与字号沿用 tx1（4 逻辑字号 13/15/20/28 × 3 行中英混排；深/浅底；2× 主 / 1× 辅）。
每档绑定一个字重与一组旋钮：

| 档 | 字重 | contrast | gamma | 轮廓加粗 px（2×） | 掩码膨胀 px（2×） |
|---|---|---|---|---|---|
| base | Normal | 0 | 1.0 | 0 | 0 |
| med | Medium | 0 | 1.0 | 0 | 0 |
| med-cg | Medium | 0.5 | 1.4 | 0 | 0 |
| reg-cg | Normal | 0.5 | 1.4 | 0 | 0 |
| ol15 | Normal | 0 | 1.0 | 0.15 | 0 |
| ol30 | Normal | 0 | 1.0 | 0.30 | 0 |
| ol15-cg | Normal | 0.5 | 1.4 | 0.15 | 0 |
| dil25（tx1 反例） | Normal | 0 | 1.0 | 0 | 0.25 |

字体：系统 Noto Sans CJK SC **全字重**（Regular 400 / DemiLight 350 / Medium 500 / Bold 700，
TTC 选 SC 字面，裁定 N-40）。联系表每档含 4 字号 × 2 字重（**档字重 / Bold**）共 8 行 × 3 行文本。

输出（共 83 张 PNG，`rust/index.txt` 为全清单）：

- 联系表：`rd_<档>_<dark|light>_<2x|1x>.png`（8×2×2 = 32 张）。
- 3× 最近邻放大（左上角 13px 首行）：`rd_<档>_<bg>_<scale>_zoom3.png`（32 张）。
- **多内环字专项 3×**（13px 第 3 行「文件资源管理器」含「器」，仅 2×）：`..._holes3x.png`（16 张）。
- 横向对照总表（size=15 各档字重，深底 2×，8 档并排）：`matrix_15_dark_2x.png`。
- **1:1 实际尺寸对照页**（2× 物理像素、**不放大**，8 档上下排列，左侧档名字号 26 物理 px ≥ 24）：
  `onesheet_dark.png` / `onesheet_light.png`。

## 三、量化（辅助，不替代看图）

`docs/research/tx2/analyze.py`（沿用 tx1 的 `mean_ink` / `edge_px`，新增 `halo_px`）：

- `mean_ink`：全图归一化覆盖率均值（背景 0 / 前景 1）。
- `edge_px`：每行最陡一阶差分 g，行边缘宽 ≈ 0.8/g 的中位数，越小越锐。
- **`halo_px`（新）**：文字外侧覆盖率落在 (5%, 50%] 的连续像素带平均宽度。轮廓加粗是干净阶梯，
  此值小；掩码膨胀拖出灰晕，此值大。只为**未放大**的联系表计算（放大图会按倍率放大该值）。

2× 联系表（深底 / 浅底）：

| 档 | mean_ink 深 | mean_ink 浅 | edge 深 | edge 浅 | halo 深 | halo 浅 |
|---|---|---|---|---|---|---|
| base | 0.0982 | 0.0982 | 0.81 | 0.82 | 0.60 | 0.60 |
| med | 0.1072 | 0.1072 | 0.81 | 0.81 | 0.64 | 0.65 |
| med-cg | **0.1144** | **0.1098** | **0.80** | **0.80** | **0.38** | 0.40 |
| reg-cg | 0.1054 | 0.1009 | **0.80** | **0.80** | **0.36** | 0.37 |
| ol15 | 0.1042 | 0.1042 | 0.81 | 0.81 | 0.66 | 0.67 |
| ol30 | 0.1102 | 0.1102 | 0.81 | 0.82 | 0.70 | 0.71 |
| ol15-cg | 0.1113 | 0.1067 | **0.80** | **0.80** | 0.42 | 0.38 |
| dil25（反例） | 0.1072 | 0.1072 | 1.07 | 1.07 | 1.28 | 1.31 |

读表：

- `med` 与 `base` 边缘 / 光晕几乎相同，墨量 +9.2% —— 真字重是「零光晕提浓」。
- `med` 与 `dil25` **墨量完全相同（0.1072）**，但 `dil25` 边缘 1.07（软）与光晕 1.28（灰晕），
  `med` 为 0.81 / 0.64 —— 同浓度下真字重完胜掩码膨胀。
- `ol30` 墨量（0.1102）高于 `dil25`（0.1072），边缘 0.81 vs 1.07、光晕 0.70 vs 1.28 ——
  轮廓加粗在**更高浓度下仍更干净**，直接证明其优于掩码膨胀。
- `*-cg`（对比度 / gamma）把边缘压到 0.80 且把光晕降到 0.36~0.42（低于 base），因为覆盖率的
  边缘斜坡被拉陡；`med-cg` / `ol15-cg` 因此兼得高墨量与最低光晕。
- 1× 辅表趋势与 2× 一致（全量见 `metrics.csv`）。
- 与 tx1 的 FreeType 参照（2× `mean_ink` 0.1036 / `edge_px` 0.81）比：`ol15`（0.1042 / 0.81）
  最接近，`med` / `reg-cg` 略高，均在干净区间。

## 四、读图结论（亲自看 3× 放大与 1:1 页）

看的图：8 档 × 深/浅底的 `_zoom3`（13N 首行）、`_holes3x`（13N 第 3 行含「器」）、
`rd_ol30_*_2x` 完整联系表、`matrix_15_dark_2x`、`onesheet_dark` / `onesheet_light`。

- **掩码膨胀的灰晕肉眼可见**：`rd_dil25_dark_2x_zoom3` 与 `_holes3x` 里笔画边缘有一圈发虚的灰，
  与 base 的「细但锐」不同，是 tx1 审阅指出的同款问题；`mean_ink` 与 `med` 相同却明显更糊。
- **轮廓加粗干净**：`rd_ol15/ol30_*_zoom3` 笔画变实、边缘仍是利落的阶梯，无灰晕。
  `rd_ol30_light_2x_holes3x`（白底黑字）与 `rd_ol30_dark_2x_holes3x`（黑底白字）逐字看
  「文件资源管理器」「器」的多内环：内框完整、无黑块、无自交破洞、无裁字。
- **真字重 vs 轮廓加粗**：`med` / `med-cg` 与 `ol15` / `ol15-cg` 观感接近，均干净；
  `med` 靠设计笔画，棱角与笔画对比更「正」；`ol30` 更「胖」一点但仍干净。
- **对比度 / gamma**：`reg-cg` 单独就把浅底黑字从「灰软」拉到「实」，且高光笔画仍透；不糊。
- **1:1 页**（`onesheet_light`，白底黑字，用户点名场景）：`base` 又细又淡；`med` / `reg-cg` 明显转实；
  `med-cg` / `ol15-cg` 最浓且干净；`ol30` 浓但 13~15px 下已略胖；`dil25` 浓而发虚。
  深底页 `onesheet_dark` 趋势一致。
- **无错位 / 裁字**：全部联系表左右对齐、行距均匀；28B 最长行也未折行（`inner_w` 按 28 Bold 定宽）。

## 五、推荐 2 个组合

1. **`ol15-cg`（Normal + contrast 0.5 / gamma 1.4 + 轮廓加粗 0.15 px）—— 首选（不触碰 N-41）。**
   正文仍 Normal、标题仍 Bold，只在渲染层提浓：2× 深底墨量 0.1113（较 base +13.3%），
   边缘 0.80（比 base 更锐），光晕 0.42（低于 base）。轮廓加粗干净、对比度保锐，
   是「不动字重裁定 N-41」前提下最稳妥的提浓。
2. **`med-cg`（Medium + contrast 0.5 / gamma 1.4）—— 最浓且最锐（需用户复核 N-41）。**
   2× 深底墨量 0.1144（较 base +16.5%，本轮最高），边缘 0.80、光晕 0.38（本轮最低）；
   真字重让笔画对比更「正」，浅底 0.1098 同样明显转实。代价：正文改用 Medium 字面，
   须由用户裁定是否修订 N-41。

次级备选：`ol30`（更强轮廓，浓但 13px 略胖）、`med`（只换字重，等墨于 dil25 但干净）、
`reg-cg`（最保守的纯对比度提浓）。**`dil25` 不推荐**（灰晕 + 边缘软，tx1 结论复现）。

## 六、与 Windows 10 的比较

`docs/research/tx1/win10/` 目前**仍只有 README，无用户提供的真 Win10 截图**（第一参照缺席），
故无法并排对照。可用的旁证：

- tx1 的 FreeType 参照（同字体、同物理字号）：本轮 `ol15` 的墨量与边缘（0.1042 / 0.81）
  最接近 FreeType（0.1036 / 0.81），`med` / `reg-cg` 略高 —— 即推荐档落在「FreeType 同级浓度」附近。
- tx1 的 `windows/`（Win11 GDI+ ClearType / 灰度）自述局限、明显弱于 FreeType，不作为 Win10 标准。

拿到真 Win10 原图后应并排复核（尤其 UWP/DirectWrite 场景）。

## 七、复现与验证

```
cargo run --offline -p kanesumi-harness --example text_density_tx2 docs/research/tx2/rust
python3 docs/research/tx2/analyze.py            # 生成 metrics.csv
python3 docs/research/tx2/analyze.py --optimize # 可选：Pillow 无损重压 PNG
```

- `cargo test --offline -p kanesumi-canvas -p kanesumi-harness -p kanesumi-controls`：全绿，
  不少于基线（harness 110 → **111**；canvas 55 → **58**；controls 750 + 2 不变）。
- 新增测试：
  - canvas `outline_embolden_zero_matches_plain_raster`：加粗 0 时与原 `rasterize` 的 metrics
    与位图**逐像素一致**（用 Noto CJK「口」字，覆盖外 / 内环）。
  - canvas `embolden_square_grows_by_two_d`：正方形外扩 d 后四边各 +d、边长 +2d（容差）。
  - canvas `embolden_hole_shrinks_inward`：外环 + 内环整体外扩时，内环四周**向内收** d。
  - harness `outline_embolden_increases_ink`：轮廓外扩开启后墨量与覆盖像素增加。
  - 既有 tx1 守卫 `text_tuning_default_keeps_pixels_identical` 继续通过（默认逐像素不变）。
- `cargo clippy --offline -p kanesumi-canvas -p kanesumi-harness --all-targets`：改动文件
  （`text.rs`、`glyph_layout.rs`、`cpu_raster.rs`、新 example）**零新增告警**
  （`render.rs` 的 2 条 unused-import 为改动前既有）。

## 八、范围外发现

- 任务书允许清单里的 `kanesumi-harness/src/lib.rs` 无需改动：`TextRenderTuning` 的重导出
  沿用 `pub use glyph_layout::TextRenderTuning`，新字段随之可见。
- tx1 的 `examples/text_density.rs` 因 `TextRenderTuning` 新增字段而必须补 `outline_embolden_px: 0.0`
  （仅补字段，arm 与输出不变，tx1 样张仍可原样复现）。
- `docs/research/tx1/win10/` 仍是空参照（只有 README），本任务沿用其缺位说明，未新增截图。
- `kanesumi-harness/src/render.rs` 的 2 条既有 unused-import 告警非本次引入，未顺手修（范围纪律）。
