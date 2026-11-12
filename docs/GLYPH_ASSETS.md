# 控件底纹图形（消费侧契约）

> **真源不在本仓**：图形在 `ProjectArcturius/Kanesumi-assets`（本仓 submodule
> `assets/kanesumi-assets`），规格与画稿规矩见那边 `docs/GLYPH_ASSETS.md`。
> 本文只写**本仓怎么消费**：API、缓存、同步合同、迁移面。
>
> 分层归属（L0 元件层 / L1 宿主主题）的正典：Ether 仓 `docs/KANESUMI_ASSET_LAYERS.md`。

---

## §Ⅰ 该用哪一层

| 要画的东西 | 归属 | 怎么拿 |
|---|---|---|
| chevron / 勾 / 减号 / 关闭 / 汉堡 / 更多 / 搜索 / 加 / 星 / 严重度 | **本层**（L0） | `icons::draw` |
| 文件、文件夹、设备、电量、网络、位置、应用图标 | **宿主主题**（L1） | 调用方解好后经 `Icon` 注入（`MetroIconButton::with_svg` / `text_box::delete_icon` 的形状） |
| 媒体动作（`play` / `pause` / `volume` / `brightness`） | **宿主主题**（L1） | 同上（Ether 侧 `ether-theme` 已有 `media-playback-*` / `audio-volume-*` / `display-brightness`） |

判据：**缺了它控件就不成立** → L0；**换个应用就该换一张** → 宿主的事。

---

## §Ⅱ API

```rust
use kanesumi_canvas::icons::{self, GlyphName};

icons::draw(&mut scene, GlyphName::ChevronRight, rect, colors.on_surface_variant);
icons::draw_scaled(&mut scene, GlyphName::Star, rect, color, output_scale); // 2× / 3× 输出必须用这个
```

- `GlyphName::ALL`（16 枚）与 `GlyphName::file()`（资产文件名）；
- **`draw` 是 1× 语义**：位图按 rect 尺寸光栅。2× 屏上用 `draw_scaled(.., scale)`，
  否则 16 pt 的图会被放大成 32 px、边缘发虚；
- 两条内部路径：`chevron-*` / `check` 走 `glyph.rs` 的三角几何（零纹理、零上传、损伤友好）；
  其余走 `rasterize_svg_bytes` → `Scene::image`（按 alpha 蒙版按 `color` 染色）；
- 缓存键 = **(名字, 物理像素)**：1× 与 2× 各一份，**绝不放大复用**；
- 光栅失败（资产损坏）**静默跳过、不 panic** —— 图标缺失不该让整个界面黑掉
  （`SD §IX` 只约束字体，不管图标）；
- `cached_glyphs()` / `cached_pixels(name)` / `clear_cache()` 是诊断与测试入口，不参与绘制语义。

**a11y**：图形本身是装饰，**不进无障碍树**。需要语义时由**控件**给 label ——
与 sec-a 的 `MetroIcon(imageVector, contentDescription)` 同一约定（`String?` 为 null = 装饰）。

---

## §Ⅲ 同步合同（这条是硬的）

`kanesumi-canvas/tests/glyph_assets.rs` 双向校验 **`GlyphName::ALL` ↔ `assets/kanesumi-assets/glyphs/*.svg`**：

- 加图不加枚举 → 红；
- 加枚举不加图 → 编译期就红（`include_str!` 找不到文件）；
- 每枚 「落格 / 染色 / 缓存」三件套各一条测试（16 × 3 = 48 条）。

加一枚图要动**四处**：资产仓加 SVG → 资产仓 `GLYPHS.txt` 加一行 → 跑 `gen_kotlin.py`（给 sec-a）
→ 本仓加 `GlyphName` 变体 + `file()` 一条 + `svg()` 一条 + 测试宏一行。

> submodule 没初始化时 `include_str!` 会编译失败 —— 这是**有意的**：宁可编不过，也不要静默
> 编译出一套空图形。机器上先 `git submodule update --init --recursive`。

---

## §Ⅳ 迁移面（谁还在自己画/写字形）

| 控件 | 现状 | 该改成的 |
|---|---|---|
| `check_box` | 字体 `✓` / `—`（`:203-204`） | `GlyphName::Check` / `GlyphName::Minus` |
| `info_bar` | 字体 `i` / `✓` / `!` / `✕`（`:48-51`） | `Info` / `Check` / `Warning` / `Error` |
| `rating_control` | 字体 `★` / `☆`（`:133,152,168`） | `Star`（空星用 alpha、半星用裁剪） |
| `text_box` | 字体 `✕`（`:555`） | `Close` |
| `notification_card` | 字体 `✕`（`:398`） | `Close` |
| `navigation_view` | 汉堡键内联三根 rect（`:459-465`） | `Hamburger` |
| `command_bar` / `menu_flyout` | 调用方传 glyph 字符串 | 后续 `Add` / `MoreHorizontal` |

本文件落地时**只建 API，不改这些控件**（`k-glyph-consume` 那件才改）——
控件行为变更必须带回归，不能混在资产层里。

---

## §Ⅴ 验收

- `cargo test -p kanesumi-canvas --lib` 与 `--test glyph_assets` 绿（新增 48 + 6 条）；
- `cargo run -p kanesumi-cli -- glyphs check` 在资产仓本地跑绿（**不在本仓**，本仓没有生成器）；
- `cargo test -p kanesumi-element --test api_surface` 只增不减。
