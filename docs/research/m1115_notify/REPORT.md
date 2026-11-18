# k-notify-card 报告：通知卡控件 `MetroNotificationCard`

> 卡：通知卡控件（Kanesumi 提供方，k-notify-card），依据 Ether
> `docs/research/n1112_element_bridge/NOTIFY_PLAN.md` §二 步骤 1（只做步骤 1，不含文字改走 canvas）。
> 分支 `oc/k-notify-card`，基线 `main@80f215f4`，工作树 `~/.oc/wt/k-notify-card`。
> 执行机：Debian GNU/Linux 13、rustc 1.97.1、`CARGO_TARGET_DIR=~/.oc/target/k-notify-card`。
> **只改 Kanesumi**：Ether 侧只读了 `compositor/src/shell/notifications.rs` 与 `theme.rs`，未写一字。
> 只提交、不推送。

## 〇 · 结论

`MetroNotificationCard` 已落地 `kanesumi-controls/src/notification_card.rs`，几何 / 命中 / 悬停
逐值抄 Ether 合成器：常量表带来源行，`geometry(width)` 给出卡高与全部子矩形，`hit` 按
关闭叉 > 按钮 > 卡体，`render` 对每个槽位恒定发命令、悬停只换颜色。**没有做文本量测**，
正文行由调用方切好；`wrap_body` / `draw_fitted_text` 仍留在合成器（步骤 2 的范围）。

| 交付物 | 状态 |
|---|---|
| `kanesumi-controls/src/notification_card.rs`（新 API 追加） | 完成，6 条新测试 |
| `lib.rs` 导出四个名字 | 完成 |
| `docs/API_SURFACE.txt` | 重新生成：**+9 / −0** |
| `docs/CONTROL_SPEC.md` §56「通知卡」 | 完成 |
| 本报告（映射表 + 常量表） | 完成 |

## 一 · 旧函数 → 控件方法逐条映射

| 源（`compositor/src/shell/notifications.rs`） | 行 | 控件（`notification_card.rs`） | 迁移说明 |
|---|---|---|---|
| `CardLayout` | 1599-1611 | `NotifyCardGeometry` | `image`→`icon`、`summary`→`title`、`body: Vec`→`body_lines`；新增 `body` 整体盒（派生，见 §五-3）；去掉 `idx` / `id` / `exiting`（堆叠与退场归调用方） |
| `card_height` | 1778-1792 | `MetroNotificationCard::card_height` | 逐项：12 + 应用名 16 + 标题 22 + 22×行数 + 按钮 10+28 + 12，下限 72 |
| `text_width` | 1192-1195 | `geometry` 的 `text_w` | 源用 `CARD_W`，控件用入参宽（步骤 1 调用方传 320，逐值相同） |
| `layout_card` | 1859-1917 | `MetroNotificationCard::geometry` | 图标 / 应用名 / 标题 / 正文 / 按钮 / 关闭叉的全部矩形；舍入照源 `rect_from:1613-1618` |
| `rect_contains` | 1620-1626 | 私有 `contains` | 半开区间，逐值相同 |
| `hit` | 2146-2163 | `MetroNotificationCard::hit` | 关闭叉 > 按钮 > 卡体；卡外 `None`。源里的 `exiting` 与多卡倒序遍历不迁（单卡控件不知道堆叠） |
| `pointer_move` | 2166-2174 | `set_hover` + 调用方比较 | 控件只存悬停目标；「变化才重绘」由调用方比较前后 `hit` 结果 |
| `draw_border` | 1928-1940 | 私有 `draw_border` | 四条 1 px 细矩形；**不用** `stroke_rect`（中心描边会改像素） |
| `blend` | 1628-1635 | 私有 `blend` | 手工插值，α 取 `max`（不用 `Color::lerp`，它连 α 一起插值） |
| 渲染取色 | 2026-2084 | `MetroNotificationCard::render` | 卡底 / 描边 / 文字 / 按钮底 / 关闭叉底全部改走 `ThemeColor` 令牌；顺序保持「底 → 边 → 应用名 → 标题 → 正文 → 按钮 → 关闭叉」 |
| `URGENCY_BAR_W`（critical 竖条） | 61 | **未迁** | `NotificationContent` 没有 urgency 字段，见 §五-4 |

## 二 · 抄来的常量表

| 常量 | 值 | 来源行 | 用途 |
|---|---|---|---|
| `NOTIFY_PAD` | 12 | `notifications.rs:42` | 卡内边距 |
| `NOTIFY_IMAGE_SIZE` | 48 | `:44` | 左侧图片边长 |
| `NOTIFY_IMAGE_GAP` | 12 | `:45` | 图片列与文字间距 |
| `NOTIFY_LINE_H` | 22 | `:48` | 标题 / 正文行高 |
| `NOTIFY_APP_LINE_H` | 16 | `:49` | 应用名行高 |
| `NOTIFY_BODY_MAX_LINES` | 3 | `:52` | 正文最多行数 |
| `NOTIFY_BTN_H` | 28 | `:53` | 按钮高度 |
| `NOTIFY_BTN_GAP` | 8 | `:54` | 按钮间距 |
| `NOTIFY_BTN_TOP_GAP` | 10 | `:55` | 正文到按钮行的间距 |
| `NOTIFY_BTN_MAX` | 3 | `:56` | 按钮最多个数 |
| `NOTIFY_CLOSE_SIZE` | 18 | `:58` | 关闭叉命中边长 |
| `NOTIFY_CLOSE_INSET` | 6 | `:59` | 关闭叉内缩 |
| `NOTIFY_BORDER` | 1 | `:1928-1940` | 描边宽度 |
| `NOTIFY_MIN_EXTENT` | 1 | `:1613-1618` | 源 `rect_from` 的宽 / 高下限 |
| `NOTIFY_BG_ALPHA` | 0.97 | `theme.rs:191` | 卡底 `surface_variant @ 0.97` |
| `NOTIFY_BTN_TINT` | 0.06 | `:2062-2064` | 按钮常态底叠色 |
| `NOTIFY_BTN_TINT_HOVER` | 0.12 | `:2062-2064` | 按钮悬停底叠色 |
| `NOTIFY_CLOSE_TINT` | 0.12 | `:2072-2076` | 关闭叉悬停底叠色 |

未进常量表：`CARD_W = config::NOTIFICATION_WIDTH = 320`（`config.rs:67`）——由 `geometry(width)`
入参取代；`APP_TEXT_SCALE = 0.8` / `BTN_TEXT_SCALE = 0.85`（`:50`、`:57`）——源用它缩放已栅格化
纹理，控件不量测文字，改由 `TextStyle` 选档（见 §五-5）。

## 三 · 几何等价性（怎么核对、核到什么程度）

源 `layout_card` 的输入是**已经取整的** `Rectangle<i32, Logical>`；控件 `geometry(width)` 只拿宽，
所以先按源 `rect_from:1613-1618` 把卡宽 / 卡高四舍五入成整数，再推子矩形，舍入规则逐边一致。
宽 320 时所有加法项都是整数，按钮等分的小数逐边取整（3 个按钮宽 93，x = 12 / 113 / 215）。

按常量手算的核对值（也是新测试的期望值，测试不调被测函数生成 expected）：

| 输入（宽 320） | 卡高 | 子矩形 |
|---|---|---|
| 应用名 + 标题 + 2 行正文、无按钮 | 106 | 应用名 `(12,12,296,16)`、标题 `(12,28,296,22)`、正文行 `(12,50,296,22)` / `(12,72,296,22)`、叉 `(296,6,18,18)` |
| 同上 + 有图 | 106 | 图标 `(12,12,48,48)`、文字列 x=72 / 宽 236 |
| 应用名 + 标题、1 按钮 | 100 | 按钮 `(12,60,296,28)` |
| 应用名 + 标题、2 按钮 | 100 | `(12,60,144,28)`、`(164,60,144,28)` |
| 应用名 + 标题 + 2 行正文、1 按钮 | 144 | 按钮 `(12,104,296,28)` |
| 无应用名 / 无标题 + 1 行正文 | 72（下限） | 正文行 y=12（不占应用名 / 标题行） |
| 5 行正文 | 128 | 只排 3 行、正文盒高 66 |

**已核对**：上表每一行都有对应单元测试断言（`migrate_*` 6 条），`cargo test -p kanesumi-controls --lib`
全绿。**还没核对**：与合成器**同进程跑同一输入**的逐矩形对照 —— Kanesumi 侧不能调用 Ether 的
`Rectangle<i32, Logical>` 布局，本卡也没有步骤 0 的 C0 转录可用。上面是「按源常量手算」的等价，
不是「两份实现自动对拍」。这条留给人审或后续桥接时的转录比对。

## 四 · 门与读数

卡面 §四的门按原样执行（两条命令各自单独跑；下面是最终一次、在全部改动之后）：

```text
$ cargo test --offline -p kanesumi-controls --lib
test result: ok. 1030 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 36.02s

$ cargo test --offline -p kanesumi-element --test api_surface
running 1 test
test public_api_surface_matches_snapshot ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.15s
```

- 新增 6 条：`migrate_geometry_matches_source_without_and_with_image`、
  `migrate_button_rows_for_zero_one_two_buttons`、`migrate_hit_priority_close_then_button_then_body`、
  `migrate_hover_changes_colors_but_not_command_count`、
  `migrate_empty_summary_and_app_do_not_reserve_lines`、`migrate_body_lines_cap_at_three`。
- 基线（卡面前的 `main@80f215f4`）是 1024 条；1030 − 1024 = 6，与新增数一致。
- `docs/API_SURFACE.txt`：**+9 / −0**，九行分别是 `NotifyHit`、`NotificationContent`、
  `NotifyCardGeometry`、`MetroNotificationCard` 四个类型与 `new` / `geometry` / `set_hover` /
  `hit` / `render` 五个方法。既有条目一行未动。
- 额外跑过（不在卡面门内，用于确认没有绕过静态检查）：
  `cargo test --offline -p kanesumi-controls --test size_discipline --test token_discipline
  --test notification_card_render` → 13 passed / 0 failed（2 + 2 + 9）。
- 编译告警：`kanesumi-controls` 测试构建仍有 5 条既有 `unused_variables` / `unused_mut`
  （candidate_window ×2、expander、radio_buttons、split_button），本卡文件零新增告警。
- **未跑** `cargo test --offline --workspace --features kanesumi-harness/a11y`：卡面注明它是
  Fedora 上的基线红；本机是 Debian 13，且该门与本卡无关（卡面也说了别修）。没有假装跑过。

## 五 · 未完成 / 与卡面的差异

1. **卡面写的 3845 行与工作树实际 3527 行不符**：`/home/rain/projects/Arcturius/Ether/compositor/
   src/shell/notifications.rs` 实测 3527 行（生产 2369 + 测试 1158），卡面写「3845 = 2369 + 1476」。
   卡面表格里的函数行号（`CardLayout:1599`、`hit:2146`、`pointer_move:2166`…）与实际**逐条吻合**，
   差别只在测试段长度，故按实际文件取数，未阻塞。
2. **两代通知卡并存**：同文件里已有量测版 `CardGeometry` / `render_card`（k-notification-family，
   有 9 条集成测试）。本卡没有删它（删就是破坏公共 API 与既有测试），只追加迁移版，并在文件头
   与 §56 写明两者分工。两代合流留给步骤 2。
3. **`body` 是派生盒**：源 `CardLayout` 只有 `body: Vec<Rectangle>`，卡面要求 `body: Rect` +
   `body_lines: Vec<Rect>`。`body` 取「正文首行顶 → 末行底」的整体盒；0 行时高 0，不参与命中。
4. **critical 竖条没迁**：`NotificationContent` 没有 urgency；源 `URGENCY_BAR_W:61` 与
   `render` 的 `urgency >= 2` 分支（`:2032-2039`）留在合成器。
5. **文字样式是控件选择，不是抄来的**：应用名 `body_small`、标题 `base`、正文 `body`、按钮
   `body_small`。源用 `TextStyle::bold` + 纹理缩放（0.8 / 0.85），控件不量测，样式只影响
   Scene 的文字命令，不改矩形。步骤 2 会整体重定，故未按源逐值复刻。
6. **常量用 f64**：与源同精度；`size_discipline` 只拦 `const X: f32 = <整值>`，所以这些常量
   不会被它拦下。它们不是新的设计尺寸，是迁移契约（步骤 2/3 删除），已在 §56.5 与这里登记，
   是否要补进 `CANON_VS_TEMPORARY.md` 由调度者裁定。
7. **没有原点入参**：`geometry(width)` 只给宽，坐标以卡左上为原点；`Scene` 没有平移命令，
   调用方要把命令矩形平移到卡片位置（步骤 3 走 SceneBridge 时再定）。
8. **关闭叉图形**：用 `icons::draw(GlyphName::Close)`（18×18 命中盒、图形铺满），源是 SVG
   栅格化后居中；两者都用同一枚 `close.svg` 资产，但控件侧图形尺寸由 18×18 盒决定，未按源
   纹理的实际宽高比复刻。无悬停时画 `Color::TRANSPARENT`（源 alpha 0），命令仍在。

## 六 · 范围外发现

1. `docs/research/n1112_element_bridge/NOTIFY_PLAN.md` **不在 Kanesumi 仓**，只在 Ether 仓根
   （`/home/rain/projects/Arcturius/Ether/docs/research/...`）。本卡按卡面引用读了 Ether 侧那份，
   未复制进 Kanesumi（避免两份计划漂移）。
2. `notification_card.rs` 的旧结构把 `render_card` 放在 `#[cfg(test)] mod tests` **之后**，
   而 `size_discipline` / `token_discipline` 只扫「首个 `#[cfg(test)]` 之前」——旧渲染段
   实际上不在静态检查范围内。本卡没有搬动旧段（避免扩大审阅面），新代码全部插在测试模块之前，
   确保被扫到。建议下次整理该文件时把测试统一挪到末尾。
3. 两代卡片共用 `notification_card.rs`：`CardGeometry`（量测版）与 `MetroNotificationCard`
   （迁移版）语义不同但名字相近。合流应作为步骤 2 的显式决定，不要在两代之间悄悄换用。
4. Ether 侧 `notifications.rs` 的 `layout_card` 用 `CARD_W`（固定 320），不读传入 rect 的宽；
   本控件把宽度变成入参。若将来合成器允许通知卡变宽，源函数会先失真 —— 迁移时需同步改
   合成器（步骤 3 的活），不是控件单方面能定的。
5. 本工作树初始时 `assets/kanesumi-assets` 子模块未检出（`cargo check` 因缺 `close.svg` 等
   16 个 glyph 直接失败）。已用 `git -c protocol.file.allow=always submodule update --init
   assets/kanesumi-assets` 从本机对象初始化，指向 pin 的 `7af85b9`；未改 `.gitmodules`、
   未联网、未动其它工作树。这是构建环境修复，不算代码交付。
