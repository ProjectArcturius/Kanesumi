# tx1 文字浓度证据 spike —— 报告（2026-10-04）

> 任务 `tx1-text-density`。只产证据、不进生产：所有旋钮默认关，生产路径逐像素不变。
> 根因诊断见 Ether `docs/TYPE_ENGINE_PLAN.md` §一·五、`docs/DECISIONS_2026-10-04.md` §E-63。
> **第一参照（真 Win10 截图）本次开工时尚缺**（`win10/` 只有 README），本报告结论以读图 +
> FreeType 参照 + 量化三者交叉得出；`windows/`（Win11 GDI+）偏弱，仅次要参照。

## 一、实现（旋钮，默认关）

代码落点与任务书的 `kanesumi-core/src/text.rs` 不符：实际 `TextEngine` 驻
`kanesumi-canvas/src/text.rs`（2026-08-10 已从 core 迁出），且无需改它 —— 两个变换都在
**覆盖率进入混合之前**、由 `kanesumi-harness` 完成，故只改了允许清单内的 harness 文件：

- `kanesumi-harness/src/glyph_layout.rs`：新增 `TextRenderTuning`（唯一入参结构）、
  按前景亮度的对比度/gamma 查表、按字号递减的笔画膨胀。
- `kanesumi-harness/src/cpu_raster.rs`：`CpuRenderer::set_text_tuning`（默认值即现行为）；
  覆盖率在 `blit_coverage` 处过查表；加粗在字形光栅化时施加。
- `kanesumi-harness/src/lib.rs`：重导出 `TextRenderTuning`。
- 新增 `kanesumi-harness/examples/text_density.rs`（样张程序）。

`layout_text_glyphs`（GPU 共享入口）签名不变：新增 `layout_text_glyphs_tuned` 供 CPU 专用，
默认旋钮走原函数，GPU 路径零改动。

### 1.1 对比度增强公式

对覆盖率 `c ∈ [0,1]`，按前景色 `(R,G,B)`（sRGB 直通）算亮度 `luma = 0.2126R + 0.7152G + 0.0722B`：

```
gamma_eff = max(gamma × (1 + 0.30·(clamp(luma,0,1) − 0.5)), 0.05)
c₁ = c ^ (1 / gamma_eff)
c₂ = clamp(0.5 + (c₁ − 0.5)·(1 + contrast), 0, 1)
out = round(255·c₂)
```

思路参照 Skia `SkScalerContext` 的 luminance preblend 与 DirectWrite 增强对比度：
gamma 档由前景亮度选（亮字/深底 ×1.15，暗字/浅底 ×0.85，在 `gamma` 基础上），
contrast 以 0.5 为轴做 S 形拉伸。`contrast=0 && gamma=1` 时返回 `None`，覆盖率原样进混合。

### 1.2 笔画加粗公式

```
amount(size_px) = stem_darken_px × clamp(30 / size_px, 0.5, 2.0)   // 小字号加得多
r = floor(amount)，frac = amount − r
dilated = max(横 r 邻域, 纵 r 邻域) 与半径 r+1 结果按 frac 线性混合
```

形态为十字结构元（横 / 纵最大值滤波逐像素取最大）；四周先补 `ceil(amount)` 像素零边并同步
平移 / 放大字形 metrics，避免膨胀在 bbox 边缘被削。曲线形状参照 Adobe CFF stem darkening
「小字号加得多」，是形状近似而非逐值复刻。

## 二、参数档与样张命名

样张程序 `examples/text_density.rs`（输出目录参数，默认 `docs/research/tx1/rust`）。
7 个旋钮档（`base` 即现状）：

| 档名 | contrast | gamma | stem_darken_px |
|---|---|---|---|
| base | 0.0 | 1.0 | 0.0 |
| ctr | 1.0 | 1.0 | 0.0 |
| gam | 0.0 | 1.8 | 0.0 |
| stem25 | 0.0 | 1.0 | 0.25 |
| stem50 | 0.0 | 1.0 | 0.5 |
| both-md | 0.5 | 1.4 | 0.25 |
| both-hi | 1.0 | 1.8 | 0.5 |

每个「档 × 底色（dark/light）× 缩放（2×/1×）」一张联系表，含 4 逻辑字号（13/15/20/28）
× 2 字重（N/B）× 3 行文本；另出左上角（13N 行）3× 最近邻放大。

- Rust：`rust/rd_<档>_<dark|light>_<2x|1x>.png` + `..._zoom3.png`，共 56 张。
- FreeType 参照：`freetype/ft_<default|darken>_<dark|light>_2x.png` + `..._zoom3.png`，共 8 张。
- 横向对照总表：`rust/matrix_15N_dark_2x.png`（size=15 Normal 深底 2×，6 档并排：
  base / ctr / gam / stem25 / both-md / both-hi）。
- 文件清单与参数：`rust/index.txt`。

文本（按表意空格折成三行，保证各变体换行点一致）：

```
设置　网络和 Internet　蓝牙和其他设备　个性化
Settings　Network & Internet　Bluetooth & devices
0123456789　文件资源管理器　The quick brown fox
```

## 三、FreeType 参照（Arch + Pillow 内置 FreeType 2.14.3）

`freetype_ref.py` 用同一个 Noto Sans CJK SC（TTC 选 SC 字面）渲染同文本、同物理字号（2×）：

```
python3 docs/research/tx1/freetype_ref.py --mode default
python3 docs/research/tx1/freetype_ref.py --mode darken   # 进程内设 FREETYPE_PROPERTIES="cff:no-stem-darkening=0"
```

**结论（读图 + 量化一致）**：本机 FreeType 的 `default` 与 `darken` 两档**几乎无差别**
（dark 2× `mean_ink` 0.1036 vs 0.1039；`edge_px` 均 0.81；放大样张肉眼同图）。
即 FreeType 对本字体（Noto CJK，CFF 轮廓）**默认已启用 CFF 笔画加粗**，
`no-stem-darkening=0` 不再产生额外变化。故 FreeType 一档即可作参照，色深不是它锐的原因。

## 四、量化（辅助，不替代看图）

方法（`analyze.py`，全量见 `metrics.csv`）：

- `mean_ink`：全图归一化覆盖率均值（背景 0 / 前景 1）。同排版内可比；跨 Rust/FreeType
  因两图行高与留白不同，**只作组内对照**。
- `edge_px`：每行最陡一阶差分 `g`（覆盖率/像素），该行边缘宽 ≈ `0.8/g`，取各行中位数。
  越小越锐。对细笔画比阈值穿越法稳健。

2× 联系表（非放大）：

| 文件 | mean_ink | edge_px |
|---|---|---|
| `rust/rd_base_dark_2x.png` | 0.0982 | 0.81 |
| `rust/rd_base_light_2x.png` | 0.0982 | 0.82 |
| `rust/rd_ctr_dark_2x.png` | 0.1005 | 0.80 |
| `rust/rd_ctr_light_2x.png` | 0.0959 | 0.80 |
| `rust/rd_gam_dark_2x.png` | 0.1071 | 0.81 |
| `rust/rd_gam_light_2x.png` | 0.1035 | 0.82 |
| `rust/rd_stem25_dark_2x.png` | 0.1072 | 1.07 |
| `rust/rd_stem25_light_2x.png` | 0.1072 | 1.07 |
| `rust/rd_stem50_dark_2x.png` | 0.1161 | 1.32 |
| `rust/rd_stem50_light_2x.png` | 0.1161 | 1.32 |
| `rust/rd_both-md_dark_2x.png` | 0.1164 | 0.98 |
| `rust/rd_both-md_light_2x.png` | 0.1084 | 0.86 |
| `rust/rd_both-hi_dark_2x.png` | 0.1365 | 0.91 |
| `rust/rd_both-hi_light_2x.png` | 0.1269 | 0.85 |
| `freetype/ft_default_dark_2x.png` | 0.1036 | 0.81 |
| `freetype/ft_default_light_2x.png` | 0.1036 | 0.81 |

读表：现状 `base` 墨量最低；纯 contrast/gamma 小幅提浓且保锐（`edge_px` ≤ base）；
纯加粗提浓最多但边缘变软（`edge_px` 随加粗量上升）；`both-*` 兼得，强档墨量最高。

## 五、读图结论与推荐

**看图（2×，13N 行 3× 放大）**：

- 浅底黑字（用户点名的「又细又淡」）：`base` 明显灰而软；`ctr`/`gam` 略深；
  `stem25`/`both-md` 明显转实；`both-hi` 最重。深底白字同理。
- 与 FreeType 比：FreeType 边缘锐、整体实，但**不靠大幅加粗**（其 `mean_ink` 0.1036 介于
  `ctr` 与 `gam`/`stem25` 之间）；Rust 现状偏软的主因确实是缺对比度补偿 + 缺笔画加粗。
- FreeType 放大图显示其笔画比 `base` 更实、边缘更干净，与 `both-md` 观感最接近。

**推荐 2 个组合**：

1. **`both-md`（contrast 0.5 / gamma 1.4 / stem 0.25 px）—— 首选。**
   墨量 `mean_ink` 0.1164（深底，较 base +18.5%），同时 `edge_px` 0.98 仍接近现状；
   contrast/gamma 补对比、stem 补笔画，视觉「浓郁」而不过「胖」，最像 FreeType/Windows 的
   清爽浓郁。浅底黑字同样明显转实（0.1084 vs base 0.0982）。
2. **`both-hi`（contrast 1.0 / gamma 1.8 / stem 0.5 px）—— 最强劲档。**
   墨量 0.1365（较 base +39%），最接近 Windows 10 那种「重」的观感；代价是 13px 下已偏粗，
   边缘略软（`edge_px` 0.91）。若用户嫌 `both-md` 不够劲，用这档。

次级备选：`stem25`（只加粗，最接近 FreeType 墨量，但边缘最软）、`gam`（只提 gamma，
保锐且提浓）；`ctr` 提升最小、适合极保守。

**局限**：本次缺真 Win10 截图（第一参照），推荐以 FreeType 与观感为准；拿到 Win10 原图后应复核。

## 六、复现与验证

```
cargo run --offline -p kanesumi-harness --example text_density docs/research/tx1/rust
python3 docs/research/tx1/freetype_ref.py --mode default
python3 docs/research/tx1/freetype_ref.py --mode darken
python3 docs/research/tx1/analyze.py            # 生成 metrics.csv
python3 docs/research/tx1/analyze.py --optimize # 可选：Pillow 无损重压 PNG
```

- `cargo test --offline -p kanesumi-core -p kanesumi-harness -p kanesumi-controls`：全绿。
  harness 101 → 110（+9），core 54、controls 750+2 不变。
- 默认关（逐像素不变）由 `text_tuning_default_keeps_pixels_identical` 守卫：显式
  `set_text_tuning(默认)` 与不设置渲染结果 `assert_eq` 全缓冲。仓库**无 PNG 金样快照测试**，
  故「现有快照零变化」以该逐像素断言 + 默认短路实现保证。
- 查表单测：端点（0→0、255→255）、单调不减、中间调抬高、按亮度选档；
  加粗量随字号递减、基准值、亚像素线性过渡、十字不扩张对角。
- `cargo clippy --offline -p kanesumi-harness --all-targets`：改动文件（`glyph_layout.rs`、
  `cpu_raster.rs`、`lib.rs`、example）**零新增告警**（render.rs 的 2 条 unused-import 为改动前既有）。

## 七、范围外发现

- 任务书「允许改动」列的 `kanesumi-core/src/text.rs` 在仓库中不存在（`TextEngine` 早已迁至
  `kanesumi-canvas/src/text.rs`）；本任务无需触碰该文件，已按实际落点实现。
- `kanesumi-harness/src/render.rs` 存在 2 条既有 unused-import 告警
  （`TextLayoutOptions`、`PlacedGlyph`/`glyph_key`），非本次引入，未顺手修（范围纪律）。
- `windows/`（Win11 GDI+）样张明显比 FreeType 与 `both-*` 弱，印证其 README 的自述局限，
  不应作为 Win10 标准。

