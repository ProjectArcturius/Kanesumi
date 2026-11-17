# k-glyph-complete 交件报告（2026-11-17）

任务书：`k-glyph-complete.md`；原卡：`n1112/k-glyph-consume.md`。
只续做 `rating_control` 与 `navigation_view`；已完成四处（`check_box` / `info_bar` /
`text_box` / `notification_card`）未重做。

## 〇 · 结论

两处控件已改用 L0 图形层，字体星形与内联三线已删除；新增/替换 8 条测试。
原卡 §一.2 的「视觉尺寸不得变」已按用户裁定 C2 转为**明确的行为变更**：
汉堡可视 14×14/线宽 1.5 → **16×14/线宽 2**，满星 22×16 → **24×22**
（参 Ether `docs/DECISIONS_2026-11-17.md` §265；规格已更新到
`docs/CONTROL_SPEC.md` §24/§28）。旧尺寸不是等价门，旧实现数据与对照图保留（§二、§七）。
六道验收命令本轮退出码全 0（§六），本件按「获批 C2，逐项验收」交件。

环境注记：上一轮在 LLD 条件变量等待中失去进展，已保全并取消；本轮按任务书使用
`/home/rain/.local/state/ether-build-20261117/bin/cc`（仅加 `-Wl,--threads=1`）、
`CARGO_BUILD_JOBS=2` 与独立 target，测试输入/断言/门未改。

## 一 · 改了什么

### `kanesumi-controls/src/rating_control.rs`

- 星形改走 `icons::draw_scaled(GlyphName::Star, …)`，不再写字体 `☆` / `★`；
  `star_text_rect` 与 `TextAlign` 依赖随之删除。
- 32 pt 母版居中压进 24 pt 星格（`ICON_32`，已有尺寸档，无新裸常量）；
  控件矩形外加一对 `PushClip` / `PopClip`，防止 32 pt 位图越出控件。
- 空星不再用 `tint.with_alpha`：两条 Image 渲染路径都会用
  `tint.unwrap_or(WHITE).with_alpha(opacity)` 覆盖 `tint.a`（参 §九），
  所以新增私有 `draw_star`，把 0.4（只读再乘非激活档）写进
  `SceneCommand::Image.opacity`；实星同样用 opacity 表达只读态。
- 半星仍只对**本星** `push_clip` / `pop_clip`，复用同一份光栅；新增像素测试证明
  它不会污染后续空星/实星（§四）。

### `kanesumi-controls/src/navigation_view.rs`

- Toggle 的三根 14×1.5 内联 `fill_rect` 改为
  `icons::draw_scaled(GlyphName::Hamburger, …)`；32 pt 母版居中放进原 40×40
  `toggle_rect`，命中区、hover 底、`NavigationAction::Toggle` 均未动。
- 两条 `render` 都保留原签名，新增私有 `render_scaled(…, scale)` 供 1×/2× 测试
  与快照显式指定倍率；公开接口零变化（§五）。

## 二 · 1× / 2× 旧新 alpha 包围盒

测法：`~/.local/state/k-glyph-probe`，同一 `MetroTheme::ether_dark()`、同一系统字体链
（首字体 `NotoSansCJK-Regular.ttc` tag `SC`）、同一逻辑矩形（汉堡 40×40，星 24×24）、
`CpuRenderer` 倍率 1.0 / 2.0，取 alpha ≥ 16 的闭区间包围盒。
旧实现是 `e830107` 的 `rating_control.rs` / `navigation_view.rs` 快照。

| 对象 | 1× 旧 | 1× 新 | 2× 旧 | 2× 新 | 逻辑差 |
|---|---|---|---|---|---|
| 汉堡 | 14×14 | **16×14** | 28×28 | **32×28** | 宽 +2，高 0 |
| 星·满 | 22×16 | **24×22** | 44×32 | **46×44** | 宽 +2，高 +6 |
| 星·半 | 11×16 | **12×22** | 22×32 | **23×44** | 宽 +1，高 +6 |
| 星·空 | 22×16（空心轮廓） | **22×22**（0.4 实心） | 44×32 | **46×43** | 宽 0，高 +6 |

- 汉堡旧：三根 14×1.5、间距 6，包围盒 14×14；新：32 pt 母版三根 16×2、间距 6，
  包围盒 16×14。水平中心都在 `(20, 20)`，命中区仍 40×40。
- 星旧：Noto CJK SC 24 pt 的字体星形 22×16；新：32 pt SVG 墨迹约 22.8×21.7，
  光栅包围盒 24×22（满星）。半星新宽 12 是 24 格的一半，2× 为 23 px（像素边界取整）。
- 星空新在 0.4 不透明度下，alpha ≥ 16 的门会削掉最外圈抗锯齿，故 1× 量到 22×22、
  2× 量到 46×43；满星同一门是 24×22 / 46×44。
- 旧字体随字体变：DejaVu Sans 满星 19×17，Source Han SC Bold 22×16；
  新 SVG 与字体无关，三个字体下都是 24×22 / 46×44（探针输出见 §二 与本轮实测）。

## 三 · 获批 C2：行为变更与旧尺寸对照

用户裁定 C2 明确批准 32 pt 母版几何，旧尺寸不再是本次等价门。本件据此把原卡 §一.2
的「视觉尺寸不得变」记录为**行为变更**，并在规格里写明旧新值：

- 汉堡：可视 14×14、线宽 1.5 → **16×14、线宽 2**（2×：28×28 → 32×28）；三线间距 6 未变。
- 满星：**22×16 → 24×22**（2×：44×32 → 46×44）；半星 11×16 → 12×22。
- 空星：旧字体空心轮廓 22×16 → 新 0.4 不透明度的实心星 22×22（2× 46×43，
  因 alpha≥16 的测量门削掉最外圈抗锯齿）。
- 垂直落位：旧字体星形的墨迹贴在 24 pt 行盒底部（1× 截图象素实测：相对星格顶 8 pt、
  底 0），新位图居中（顶/底各约 1 pt）；这是字体基线 vs 图像落格带来的可见变化，
  随 C2 一并记录，未用偏移把新星压回旧位置。

原因：L0 真源只有 32 pt 一个母版（资产仓 §Ⅲ、§Ⅶ「不加第二种母版尺寸」）。
`hamburger.svg` 的三条线是 `M8 … L24` + `stroke-width=2`，32 pt 下必然是 16 宽；
`star.svg` 的路径在 32 pt 下墨迹约 22.8×21.7。消费侧没有改字号、线宽、rect 比例或乘系数
去凑旧形。旧图（三张对照图）与旧实现包围盒都保留，作为行为变更的对照，不删。

规格落点：`docs/CONTROL_SPEC.md` §24（星格 24×24、32 pt 母版、空/半/满与只读）
与 §28（PaneToggle 40×40 命中区、汉堡 16×14/线宽 2）。

## 四 · 新增/替换的测试与「原代码预期红」

| 测试 | 断言什么 | 旧 `e830107` 会红在哪 |
|---|---|---|
| `rating::render_emits_star_glyphs`（替换旧 `render_emits_stars`） | 0 个 `Text`、8 个 `Image`；5 张空星 `on_surface_variant` + opacity 0.4，3 张实星 `primary` + opacity 1；逻辑 32×32、1× 位图 32 px；2 个 clip（控件 + 半星 `(56,0,12,24)`） | 旧：8 个 `Text`、0 个 `Image`、1 个 clip |
| `rating::render_scaled_at_two_x_keeps_logical_rect` | 2× 逻辑 rect 仍 32、位图 64 px | 旧无 `Image`，也没有显式倍率入口 |
| `rating::empty_star_is_dimmed_and_gray_in_pixels` | 黑底光栅：空星峰值约 `[101,105,109]`（0.4 线性不透明度）、中性灰；满星 `[57,130,255]`、不透明 | 旧空星是字体空心轮廓、全 alpha；峰值会落在 `on_surface_variant` 的 154+ |
| `rating::read_only_dims_both_star_layers` | 只读：空星 opacity = 0.4×非激活档，实星 opacity = 非激活档 | 旧实星把 alpha 放在字体颜色里，没有 Image opacity |
| `rating::half_star_clip_is_scoped_to_its_own_cell` | 1×/2×：满星宽 24/46、半星 12/23、第 3 颗空星不是半宽（1× ≥22、2× ≥44），半星 clip 不漏到后续命令 | 旧无外层控件 clip、半星是字体文本，宽度 11/22，且无 32 pt 位图 |
| `nav::toggle_renders_hamburger_glyph` | 1 张 `Image`、逻辑 32×32、1× 位图 32 px、`on_surface` 染色、中心等于 `toggle_rect` 中心、0 根 14×1.5 `FillRect` | 旧：0 张 `Image`、3 根 14×1.5 `FillRect` |
| `nav::toggle_glyph_scales_at_two_x` | 2× 位图 64 px、逻辑 32 | 旧无 `Image` |
| `nav::toggle_glyph_pixels_match_measured_32pt_box` | 真实像素包围盒：1× `(12,13)..(27,26)`，2× `(24,26)..(55,53)`；注释写明这是获批 C2 的消费结果 | 旧像素包围盒是 1× `(13,13)..(26,26)`、2× `(26,26)..(53,53)` |

**原代码预期红证据**（探针 `k-glyph-probe` 直接跑 `e830107` 快照，不靠改测试）：

```text
== command counts (rating 2.5 / default nav) ==
old rating 2.5: text=8 image=0 clips=1 bars14x1.5=0
new rating 2.5: text=0 image=8 clips=2 bars14x1.5=0
old nav: text=0 image=0 clips=2 bars14x1.5=3
new nav: text=0 image=1 clips=2 bars14x1.5=0
```

对应关系：`text=0` / `image=8` / `clips=2` 三条旧值都与新断言相反；
nav 的 `image=0`、`bars14x1.5=3` 也与新断言相反。像素包围盒的旧值见 §二。

旧字体符号探针只改了原卡允许的那一条（`render_emits_stars` → `render_emits_star_glyphs`）；
其他既有测试的输入、断言、阈值未动。`navigation_view.rs` 里另有若干 rustfmt 换行，
无语义变化。

## 五 · 公开 API 增删

- `cargo test --offline -p kanesumi-element --test api_surface`：1 passed，快照未改
  （`git status` 无 `docs/API_SURFACE.txt` 变更）。
- **+0 / −0**。两条 `render_scaled` 都是私有 `fn`，只供同模块测试与后续快照；
  没有为了让测试调到而扩大公开面。
- `MetroRatingControl::star_style` 保留原签名（旧 App 仍可调），只是 `render` 不再用它；
  私有 `star_text_rect` 随字体路径删除，不属于公开 API。

## 六 · 验收命令与退出码

全部在本工作树、`CARGO_TARGET_DIR=/home/rain/.oc/target/k-glyph-c2` 下原样执行；
每轮都带 `PATH=/home/rain/.local/state/ether-build-20261117/bin:$PATH`、
`CARGO_BUILD_JOBS=2`（cc 包装只给 LLD 加 `-Wl,--threads=1`，不改编译/测试输入）。
完整日志在 `~/.local/state/k-glyph-c2/gate{1..6}.log`。

| # | 命令 | 退出码 | 汇总 |
|---|---|---|---|
| 1 | `cargo test --offline -p kanesumi-controls --lib` | 0 | 972 passed / 0 failed |
| 2 | `cargo test --offline -p kanesumi-canvas --test glyph_assets` | 0 | 55 passed / 0 failed |
| 3 | `cargo test --offline -p kanesumi-element --test api_surface` | 0 | 1 passed / 0 failed |
| 4 | `cargo test --offline -p kanesumi-controls --test size_discipline` | 0 | 2 passed / 0 failed |
| 5 | `cargo test --offline --workspace` | 0 | 2064 passed / 0 failed / 5 ignored（62 个 test binary） |
| 6 | `cargo clippy --offline --workspace --all-targets` | 0 | 0 error；告警均落在既有代码（两个控件里只有原有的 `find_engine` 探针告警） |

字体字形：本件两个文件里 `★` / `☆` 的**绘制**已清零；全 `kanesumi-controls/src/`
仍有其他控件注释/测试里的 `✓` 等字符（原卡已完成四处的遗留），不在本件范围。

## 七 · 截图与复核

| 文件 | 尺寸 | 内容 |
|---|---|---|
| `hamburger_old_new_1x_2x.png` | 1410×420 | 旧/新 × 1×/2× 四格，最近邻放大 8×/4× |
| `star_old_new_1x.png` | 710×1240 | 满/半/空 × 旧/新，1×，放大 9× |
| `star_old_new_2x.png` | 710×1240 | 满/半/空 × 旧/新，2×，放大 4× |

生成器：`~/.local/state/k-glyph-probe/src/bin/shots.rs`；底色 `#181818`，源像素按
`CpuRenderer` 的预乘 alpha 语义合成。三张图同时保留旧 `e830107` 与新实现，未覆盖旧图。

**限制（如实记）**：本轮模型仍不支持图像输入，`read_image` 返回
`model "deepseek/deepseek-v4.1-flash-1017" does not declare image input`，
所以**没能用眼睛看 PNG**。改用逐格非底色包围盒、逐像素颜色统计与 ASCII 灰度图
复核（见 §二 与探针输出）；「形状好不好看」这一项只能由人眼最终确认，
未被本报告冒充为已看。任务书「截图必须亲自看」这一条因此**未完成**（§八）。

## 八 · 未完成 / 没验证的

1. **截图没有人眼看**（§七）。模型不支持图像输入，已用 ASCII/像素统计代替，
   不等于人眼看过；形状是否好看只能由人眼终判。这是任务书「亲自看」一项的缺口。
2. **`DECISIONS_2026-11-17.md` §265 尚未落盘**：任务书要求引用该节，本轮核对
   Ether 该文件正文止于 §264（`grep '^265\.'` 为 0 命中）。规格与代码按任务书
   给定的编号引用；请调度者把 §265 的裁定正文补进 Ether 文档，否则引用是悬空的。
3. **GPU 路径没在本机跑**：本工作树没有可用合成器会话，只做了 CPU 光栅与
   `SceneCommand` 断言。2× 的「真实位图 64 px」是命令与 CPU 光栅两层证据，
   没有 wgpu 读回；`draw_scaled` 的 GPU 采样未亲眼验。
4. **只读态只验了 opacity 命令值，没做黑底像素对比**；空/满星的像素对比已做。
5. **旧字体测量只覆盖本机三种字体**（Noto CJK SC / DejaVu Sans / Source Han SC Bold）。
   其他机器若字体不同，旧值会变；新 SVG 值与字体无关。

## 九 · 范围外发现（不在允许改动内，未动）

1. **Image 的 `tint.a` 被两条渲染路径覆盖**：
   `kanesumi-harness/src/render.rs:1779` 与
   `kanesumi-render/src/cpu_raster.rs:1039-1044,1194` 都用
   `tint.unwrap_or(WHITE).with_alpha(opacity)`，`tint.a` 不参与最终 alpha。
   本件只能在控件里把 0.4 写进 `SceneCommand::Image.opacity`（§一）；
   更干净的长期做法是给 `icons::draw_scaled` 增一个 opacity 参数，
   让 L0 层统一处理，但那要改 `kanesumi-canvas`，超出本件允许文件。
2. **`docs/GLYPH_ASSETS.md §Ⅳ` 的迁移表仍把 `rating_control` /
   `navigation_view` 列为「字体/内联三根 rect」**。任务书只授权改
   `docs/CONTROL_SPEC.md` §24/§28，故未动；该表需要维护者随后同步。
   `CONTROL_SPEC.md` 的旧 `★/☆` 描述已在本件更新。
3. `navigation_view.rs` 的 WIP 里含若干既有测试的 rustfmt 换行，无语义变化；
   为保持续做现场未回退。
4. 原卡要求的「字体字形命中清零」在本件两个文件里对**绘制**成立；
   全 `kanesumi-controls/src/` 的注释/旧测试仍有 `✓` 等字符，属已完成四处的遗留，
   按「不重做已完成四处」未动。

## 十 · 文件与复现

改动：

- `kanesumi-controls/src/rating_control.rs`
- `kanesumi-controls/src/navigation_view.rs`
- `docs/CONTROL_SPEC.md`（§24 RatingControl / §28 NavigationView）
- `docs/research/glyph_consume_20261117/REPORT.md`
- `docs/research/glyph_consume_20261117/hamburger_old_new_1x_2x.png`
- `docs/research/glyph_consume_20261117/star_old_new_1x.png`
- `docs/research/glyph_consume_20261117/star_old_new_2x.png`

未动资产仓与主仓 submodule 指针；`git submodule status` 仍是
`7af85b9c… assets/kanesumi-assets (heads/main)`。

复现测量与截图（诊断工程只写 `~/.local/state/`）：

```bash
cd ~/.local/state/k-glyph-probe
cargo run --offline --release --bin k-glyph-probe   # 包围盒 + 命令计数
cargo run --offline --release --bin shots           # 三张对照图
```

探针的 path 依赖指向本工作树；`src/old_rating.rs` / `src/old_nav.rs` 是 `e830107`
的两文件快照，未改一字。

## 十一 · 调度者集成复核（2026-11-17）

- 用户 C2 几何批准已记入 Ether `docs/DECISIONS_2026-11-17.md` §265，提交 aab76db4。
- 在 Kanesumi 6e23633e 上集成两个控件；原 worker 提交 a4d9c233 与原树保留。
- 独立执行 `cargo test --offline --workspace --features kanesumi-harness/a11y`：
  2070 passed / 0 failed / 5 个既有 ignored。Clippy 退出 0，无新增改动路径告警。
  日志在 `~/.local/state/glyph-c2-integrated-{tests,clippy}-20261117.log`。
- 调度者用真实 render 生成深浅 × 1×/2× 图，按 measure 返回的完整控件宽度排布。
  亲看两张 2×：五颗星完整、第三颗半星只裁本格、空星较淡、只读态为非激活色阶，汉堡居中。
  图片在 Ether `docs/research/kanesumi_supply_20261117/glyph-c2-controls/`。
  只读像素路径已覆盖；本轮仍未亲验 GPU glyph 采样，不以 CPU 图代替该证据。
- 两控件分开提交，规格随各自行为更新；迁移表同步已完成这两行，其余历史条目未据此推断。
