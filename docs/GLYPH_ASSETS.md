# 图形资产层 —— 控件底纹图形归 Kanesumi（2026-11-13 用户裁定）

> **裁定**：Kanesumi 自带一套**控件底纹图形**（chevron / 勾 / 减 / 关闭 / 汉堡 / 更多 / 搜索 / 加 /
> 星 / 严重度），真源在本仓 `glyphs/`，**Rust 扇区与 Android 扇区（`GuitaristRin/Kanesumi-sec-a`）
> 用同一套**。
>
> **语义图标**（文件 / 文件夹 / 设备 / 动作 / 应用）**不归本层**：由宿主提供 —— Ether 走
> `assets/ether-theme`（191 枚单色 + 填充包），sec-a 由调用方给。
>
> 跨仓归属、禁令与消费方规矩见 Ether 仓 `docs/KANESUMI_ASSET_LAYERS.md`。

---

## §Ⅰ 为什么必须有这一层（实测，不是推测）

用户 2026-11-13 指出「sec-a 的图标资产和 Ether 下的 Kanesumi 是独立的，两边该用同一套」。
实测比这更糟：**当前没有任何一条图形来源属于 Kanesumi**，三条来源互不相通。

| 来源 | 位置 | 内容 | 谁能用 |
|---|---|---|---|
| A 自绘几何 | `kanesumi-canvas/src/glyph.rs` | **5 枚**（chevron 四向 + check），**20 处调用 / 12 个控件文件**（gallery `app.rs` 另有 1 处） | 只有 Rust 扇区 |
| B 宿主图标包 | `Ether/assets/ether-theme/icons/ether/` | 191 枚 SVG，经 `find_ether_icon` / `find_ether_glyph` 被 launcher / librarian / settings / 合成器 30+ 处消费 | **只有 Ether 的应用**（ether-theme 是本仓的兄弟 submodule，本仓拿不到） |
| C 字体字形 | `info_bar` `i ✓ ! ✕`（`info_bar.rs:48-51`）、`check_box` `✓ —`（`:203-204`）、`rating_control` `★ ☆`（`:133,152,168`）、`text_box` `✕`（`:555`）、`notification_card` `✕`（`:398`） | 依赖字体里有那个字符 | 不可靠：`VISUAL_ISSUES.md V7` 已记「Menu / Selector 图标 glyph 全是方框」，Ether 正体思源黑体没有 MDL2 私用区 |
| D 内联手绘 | `navigation_view.rs:459-465` | 汉堡键 = 三根 14×1.5 的 rect，直接写在控件里 | 只有这一处，抄不出去 |
| E 名字无供源 | `osd.rs:105` | `OsdKind::glyph()` 返回 `volume` / `mute` / `brightness` / `play` / `pause` / `caps` / `info` **七个名字，谁都不画** | 注释自己写着「外壳可换成主题图标」—— 这正是「向宿主借名」的原样 |

sec-a 侧：**0 枚自带图形**，`MetroIcon(imageVector, contentDescription)` 只负责画，
素材由调用方从 Material Icons 借（sample 用了 7 枚）。

`glyph.rs` 头注那句「Kanesumi 不依赖 Fluent / Segoe MDL2 私有区 codepoint」只解决了
「不依赖 Windows 字体」，**没有解决「谁供给图形」**。于是每个控件要么自己画、要么写字形、
要么伸手向宿主借名字 —— 三种做法的产物彼此不可比，跨扇区更不可比。

---

## §Ⅱ 图形词表（16 枚 + 4 枚预留）

「控件底纹」判据：**控件自身的构图需要它**，缺了控件就不成立。调用方想换图（比如给菜单项配
自定义图标）仍走 `Icon` 注入，不属本层。

| # | 名字（文件名） | 用在哪（证据） | 现状 | 动作 |
|---|---|---|---|---|
| 1 | `chevron-down` | `dropdown_menu` / `selector_flyout` / `drop_down_button` / `expander` / `calendar_date_picker` | `glyph::chevron_down` 自绘 | 保留几何快路径 |
| 2 | `chevron-up` | `expander` 展开态 / `pips_pager` | 自绘 | 同上 |
| 3 | `chevron-left` | `pager_control` / `calendar_view` / `title_bar` 返回 / `breadcrumb_bar` 溢出 | 自绘 | 同上 |
| 4 | `chevron-right` | `navigation_view` Top 子项 / `breadcrumb_bar` / `tree_view` / `dropdown_menu` 子菜单 | 自绘 | 同上 |
| 5 | `check` | `dropdown_menu` 勾选列 / `check_box` 选中 / `info_bar` Success | `glyph::check` 自绘，但 `check_box.rs:204` 仍写字体 `✓` | 控件改用本层 |
| 6 | `minus` | `check_box` Indeterminate | 字体 `—`（`check_box.rs:203`） | 改用本层 |
| 7 | `close` | `info_bar` 关闭、`text_box` 清空键、`notification_card:398` 关闭、`dialog` / `tab_row` / `teaching_tip` | 字体 `✕`（`text_box.rs:555`、`info_bar.rs:51`、`notification_card.rs:398`） | 改用本层 |
| 8 | `hamburger` | `navigation_view` Toggle | **内联三根 rect**（`navigation_view.rs:459-465`） | 收进本层 |
| 9 | `more-horizontal` | `command_bar` / `menu_flyout` 溢出 | 无（调用方传 glyph 字符串） | 新增 |
| 10 | `more-vertical` | `list_row` 尾随 / `command_bar` 紧凑态 | 无 | 新增 |
| 11 | `search` | `auto_suggest_box` 查询键、`text_box` 搜索变体 | 无 | 新增 |
| 12 | `add` | `command_bar` / `tab_row` 新增 | 无 | 新增 |
| 13 | `star` | `rating_control`（空 / 半 / 满；半星用 `push_clip` 复用同一枚） | 字体 `★`/`☆`（`rating_control.rs:133,152,168`） | 改用本层 |
| 14 | `info` | `info_bar` Informational | 字体 `i`（`info_bar.rs:48`） | 改用本层 |
| 15 | `warning` | `info_bar` Warning | 字体 `!`（`info_bar.rs:50`） | 改用本层 |
| 16 | `error` | `info_bar` Error | 字体 `✕`（`info_bar.rs:51`） | 改用本层 |

**预留 4 枚**（合成器 SSD 走元件层时才需要，见 Ether 供应门 §Ⅲ-3 与批次 C `k-ssd-wire`）：
`window-close` / `window-minimize` / `window-maximize` / `window-restore`。
现在 `compositor/src/wm/ssd.rs` 只画边框与激活线，**三枚窗控根本没画**，故不阻塞当前迁移。

**不归本层的七个名字**（判据见 §Ⅱ 开头）：`osd.rs:105` 的 `volume` / `mute` / `brightness` /
`play` / `pause` / `caps` / `info` 里，前六个是**设备与媒体语义** —— 归 Ether 的 `ether-theme`
（`audio-volume-*` / `display-brightness` / `caps-lock-*` / `media-playback-*` 都已有），由消费方解好后
走 `OsdGeometry::icon` 注入；只有 `info` 属本层（与 `info_bar` 共用同一枚）。

---

## §Ⅲ 真源与产物

**真源 = SVG**（人手改的只有这个）。产物由本仓 `kanesumi-cli` 出（**不引第二门语言**）。

```
glyphs/
  chevron-down.svg  chevron-up.svg  …  error.svg        ← 真源，16 + 4 预留
  GLYPHS                             ← 名单（顺序即词表顺序，生成器与检查都读它）
kanesumi-canvas/src/glyph_table.rs                     ← 产物：name → include_str! 表（勿手改）
tools/glyphs/out/MetroGlyphs.kt                        ← 产物：交 sec-a 拷入（勿手改）
tools/glyphs/out/sheet.png                             ← 产物：预览拼版，人看用
```

命令：

```bash
cargo run -p kanesumi-cli -- glyphs check    # 规格检查，必须全绿
cargo run -p kanesumi-cli -- glyphs build    # 出三份产物
```

### SVG 规格（`check` 逐条校验）

1. `viewBox="0 0 32 32"`，无 `width`/`height` 属性 —— 32 母版与 `ether-theme` 的图标正典同源；
2. 描边宽 **2**（呈现在 16 pt 时正好 1 px；12 pt 时 0.75 px，允许）；填充型（chevron 三角、星）不设描边；
3. 直角，圆角一律 0；只用单一颜色 `#000`（运行期 tint 染色，颜色值本身不参与语义）；
4. 禁止 `linearGradient` / `radialGradient` / `filter` / `mask` / `clipPath` / `opacity` / `<text>` ——
   违者是 Kanesumi 铁律 6（纯色无渐变）或会让 resvg 与 Compose 两端出图不一致；
5. 图形落在 2 px 内的安全边距里；主笔画不得贴边（防 16 pt 下被裁）；
6. 文件名 = 名字，kebab-case；`GLYPHS` 里登记顺序即渲染拼版顺序。

---

## §Ⅳ 运行期 API

```rust
// kanesumi-canvas/src/icons.rs（新文件；glyph.rs 的 5 枚几何降为内部快路径）
pub enum GlyphName { ChevronDown, …, Error }   // 与 GLYPHS 逐项对应，生成器断言
impl GlyphName {
    pub const ALL: &'static [GlyphName];
    pub fn file(self) -> &'static str;
}

/// 在 `rect` 内居中画出 `name`，按 `color` 染色。
pub fn draw(scene: &mut Scene, name: GlyphName, rect: Rect, color: Color);
```

- **内部两条路**：`chevron-*` / `check` 走 `glyph.rs` 的三角几何（零纹理、零上传、损伤友好）；
  其余走 `rasterize_svg` → `scene.image(icon, rect, Some(color))`（管线已在
  `MetroIconButton::with_svg` 跑通）。
- **缓存键 = (名字, 物理像素)**：1× 与 2× 各光栅一份，**不得**用 1× 图放大（`scale` 一变就重出）。
- **a11y**：图形本身是装饰，不进无障碍树；需要语义时由**控件**给 label（`icon_button` 的
  `accessibility()`、sec-a 的 `contentDescription`），与 sec-a 的 `String?` 语义一致。

---

## §Ⅴ 与 sec-a 的对照义务

1. **同名同层**：`MetroGlyphs.kt` 的枚举项与本仓 `GlyphName` 一一对应，名字逐字相同（kebab 转 Pascal）；
2. **同形**：同一枚在两端渲出来必须同形 —— 验收看 `sheet.png` 与 Compose 预览并排，形状差异
   （不是抗锯齿差异）即返工；
3. sec-a 的 `MetroIcon(imageVector, contentDescription)` 签名**不变**，只把取值从 Material 改成
   `MetroGlyphs.*`；`contentDescription: String?` 的「null = 装饰」语义与本仓一致，保留；
4. 本仓**不**生成 sec-a 的构图代码，只出 `MetroGlyphs.kt` 这一个文件；反向依赖禁止
   （sec-a 不得读本仓任何文件，除该生成物外）。

---

## §Ⅵ 验收判据（可数）

1. `glyphs/` 下 16 枚齐（4 枚预留可缺席，但 `GLYPHS` 里要登记）；
2. `cargo run -p kanesumi-cli -- glyphs check` 全绿；
3. `cargo test -p kanesumi-canvas --lib` 绿，新增 ≥ 16 条：每枚一条「落在 rect 内」「按 color 染色」
   「同尺寸二次调用不重复光栅（缓存命中）」；
4. `kanesumi-controls/src/` 里字体字形命中数 **= 0**：`✓` `—` `✕` `★` `☆` 五个字符 grep 为 0（测试除外）；
5. `navigation_view.rs` 内联三根 rect 已删，改调本层；
6. `glyph_table.rs` 与 `MetroGlyphs.kt` 由 `glyphs build` 出，`GlyphName::ALL` 与 `GLYPHS` 项数相等
   （检查项，不是人肉核对）；
7. `cargo test -p kanesumi-element --test api_surface` 只增不减。

---

## §Ⅶ 不做的事

- **不搬 `ether-theme` 的 191 枚**：那是 Ether 的系统图标库（设备 / 电量 / 网络 / 位置 / 应用），
  含 OS 概念，塞进扇区无关层是错的；
- **不引第三方图标集**（Material Icons / Segoe MDL2 / Font Awesome）：那正是这次要摆脱的；
- **不手改生成物**：`glyph_table.rs`、`MetroGlyphs.kt`、`sheet.png` 上都有「勿手改」头注；
- **语义图标不进本层**：哪怕「顺手」——一进就会长出第二套系统图标库；
- **不改 `MetroIcon` / `MetroIconButton` 的公开签名**（`Icon` 注入仍然保留给宿主专有图标）。
