# k-glyph-assets 交件报告（2026-11-13）

任务书：`tools/dispatch/tasks/n1112/k-glyph-assets.md`（Ether 主仓）。
规格：资产仓 `docs/GLYPH_ASSETS.md`；消费侧契约：本仓 `docs/GLYPH_ASSETS.md`。

## 一 · 结论

控件底纹图形从「三条互不相通的来源」收成一条：**真源独立成仓**（`ProjectArcturius/Kanesumi-assets`），
本仓以 submodule 引入、编译期内嵌、按 (名字, 像素) 缓存。本仓只出消费入口，**不产生任何生成物**。

## 二 · 与任务书的偏差（都是与用户当面定的）

| 任务书原写 | 实际 | 为什么 |
|---|---|---|
| 真源放本仓 `glyphs/`，`kanesumi-cli` 出 Rust 表 + Kotlin 文件 | 真源独立成仓，本仓无生成物、无生成器 | sec-a 为了 16 个 SVG 不该把整个 Rust workspace 当 submodule 拖进 Android 工程（用户 2026-11-13 裁定） |
| `kanesumi glyphs check/build` 子命令 | 校验与生成都在资产仓（`tools/check.py`、`tools/gen_kotlin.py`） | 生成器要同时出 Rust 与 Kotlin，住任何一家仓都会变成「一家的工具生成另一家的代码」 |
| 拼版出 PNG | 出 `out/sheet.html` | 不为看图引一个 SVG 光栅库；浏览器光栅足够判「不糊不歪」 |
| 4 枚窗控预留要建 | 只在名单里注释登记 | SSD 尚未接元件层，现在建就是死资产 |

## 三 · 数字

| 项 | 数 | 怎么测的 |
|---|---|---|
| 图形 | **16 枚**（+4 枚预留登记） | `GLYPHS.txt` |
| 资产仓 `check.py` | **388 项检查全绿** | `python3 tools/check.py`（Windows/本机） |
| 本仓 `glyph_assets` 测试 | **55 条全绿**（16 枚 × 3 件套 + 7 条合同测试） | `cargo test -p kanesumi-canvas --test glyph_assets`（t14p） |
| `kanesumi-canvas --lib` | **122 条全绿**（无回归） | 同上机 |
| `kanesumi-controls --lib` | **964 条全绿**（无回归） | 同上机 |
| 公共接口 | **+10 / −0** | `cargo run -p kanesumi-element --example api_surface` 重生成后 `diff` 计数 |
| clippy | **新增 0**（余 7 条全在 `font_index.rs` / `accent.rs` / examples） | `cargo clippy -p kanesumi-canvas --all-targets` 逐条定位 |

## 四 · `check.py` 咬得住的四类（实测，不是设计意图）

| 违例 | 结果 |
|---|---|
| 描边宽改 3 | 红：`check[0] 描边宽必须 2（实为 3.0）` |
| 加 `opacity="0.4"` | 红：`minus 出现禁用构造 opacity` |
| 名单删掉 `minus` | 红：`名单与文件不一致：只在名单 []；只在盘上 ['minus']` |
| 改了 SVG 没跑生成器 | 红：`kotlin/MetroGlyphs.kt 与真源不同步` |

## 五 · 没覆盖到的（诚实记）

1. **Kotlin 生成物未编译验证**：本机无 Android SDK / Gradle 依赖，`MetroGlyphs.kt` 只做了语法审查
   （用 `name=` 显式参数避开 Compose 已弃用重载）。**真值判据在 sec-a 的 `assembleDebug`**，
   那一件还没做（见下）。
2. **形状好不好没被自动判**：`check.py` 判规格与同步，判不了「画歪了」。`out/sheet.html` 只解决
   「有没有人看见」，还得人看一眼。
3. **控件仍未改用本层**：`check_box` / `info_bar` / `rating_control` / `text_box` /
   `notification_card` / `navigation_view` 六处照旧（字体字形与内联 rect 都还在）。
   本件只建 API —— 控件行为变更必须带回归，单独一件（`k-glyph-consume`）。

## 六 · 落地后需要注意的两件运维事

1. **嵌套 submodule**：Ether → `shared/kanesumi` → `assets/kanesumi-assets`。
   worker 工作树必须 `git submodule update --init --recursive`，否则 `include_str!` 直接编译失败
   （这是有意的：宁可编不过，也不要静默出空图形）。
2. **t14p 已验证**：`cargo 1.97.1`（`~/.cargo/bin`）+ 递归初始化后 55/122/964 全绿。
