# 正典 vs 临时 —— 登记表

> **存在理由**：2026-09-22 出现过一个真实事故 —— 某个 agent 看到「黑底 + 橙色元件」，
> 认定那就是 Kanesumi 风格并据此扩散。事实是：**橙色是因为 accent 机制没做**、
> **黑底是因为浅色主题没做**，两者都是临时方案。
> 临时方案本身不可耻，**被当成设计**才致命。
>
> 本文件是临时方案的**唯一登记处**。规则：
> 1. 任何「因为机制没做所以先写死 / 先只做一半」的值，必须在此登记。
> 2. 登记项**不得**被任何文档、注释、对话引用为「Kanesumi 的既定外观 / 正典」。
> 3. 取到权威值、或机制补齐后，更新本表并删除该行，同时在提交信息里点名。
> 4. 正典是 `Ether/docs/KANESUMI_DESIGN.md`；权威数值来源见 `docs/REFERENCE.md` §Ⅱ。

---

## 一、当前登记在案的临时项

| # | 项 | 现取值 | 位置 | 目标 / 正典 | 权威来源 | 状态 |
|---|---|---|---|---|---|---|
| T1 | 暗色背景 | `#1A1A1A` | `kanesumi-core/src/colors.rs` `MetroColors::dark` | **正典已修正（2026-09-22）**：不再规定「OLED 纯黑」，改为「深浅两套并列方案，取值属实现决策」 | `KANESUMI_DESIGN.md` §Ⅲ.3（已改） | **解除登记**：取值本身是方案内选择，不再是正典冲突；若仍想改纯黑，属视觉调优 |
| T2 | 强调色基色来源 | 代码内默认 `#E57812` | `kanesumi-core/src/accent.rs` `Accent::DEFAULT_HEX` | 应**始终来自 Chorus** 的 `~/.config/ether/theme.toml` 的 `accent` | `Ether/chorus/src/theme.rs` | 机制已建（accent 派生 + 双态）；**消费端接线待做**（harness 读取 + App 跟随） |
| T3 | 亮色中性色 | `#FAFAFA` / `#FFFFFF` / `#F0F0F0` / `#D6D6D6` / `#1A1A1A` / `#5A5F66` | `kanesumi-core/src/colors.rs` `MetroColors::light` | WinUI 3 `SolidBackgroundFillColorBase/Secondary/Tertiary` + `TextFillColorPrimary` | **一手源已取到**（2026-09-22）：`main/controls/dev/CommonStyles/Common_themeresources_any.xaml`（字典边界 暗 L4-207 / 亮 L208-414），并与发布版 `Microsoft.WindowsAppSDK.WinUI/1.8.250906003` 的 `Themes/generic.xaml` **249 个 Color 键逐字节交叉验证（差异 0）** | **部分结案**：`on_background`/`on_surface` 的 `#1A1A1A` **已经就是** `TextFillColorPrimary` 亮色值（`#E4000000` = 黑 89.4%，250×(1−0.894)=0x1A）✓；三个候选更正：`background #FAFAFA → #F3F3F3`、`surface #FFFFFF → #F9F9F9`、`divider #D6D6D6 → #DADADA`（**未落地** —— 属亮色视觉变更，须与 T9 真机确认同批）。`surface_variant` **维持 §Ⅴ-3 裁定不映射**（WinUI 3 无同构令牌，这是该裁定最具体的依据）。另注：暗色同族也非 WinUI 3 值，且 `BaseAlt` 暗 `#0A0A0A` 与亮 `#DADADA` **方向相反**，不可照亮色方式硬塞 |
| T21 | 字号阶梯 | `body 15` / `caption 13` / `label 11` / `title 22` / `page_heading 34` | `kanesumi-core/src/typography.rs` | UWP TypeRamp = **12/14/20/24/34/46**（WinUI 3 = 12/14/18/20/28/40/46/68） | 一手源 `themeresources.xaml`（`ControlContentThemeFontSize = 14`，L28）+ `generic.xaml` L15120-15156 的 7 个 `*TextBlockStyle` | **部分对齐**：`page_heading 34` = Subheader ✓；`title 22` vs UWP 24、`body 15`/`caption 13`/`label 11` **三者都不在 UWP 阶梯上**（属 CJK 可读性自定）。另：TypeRamp **没有行高资源**（只有 `LineStackingStrategy=MaxHeight`），故本仓行高**不得声称来自 UWP**；本仓还丢了 UWP 大字号档的 `Light`/`SemiLight` 字重（与 T7 死字段叠加，宜合并处理） |
| ~~T4~~ | ~~亮色禁用不透明度~~ | 两方案同取 `0.38` | `kanesumi-core/src/indicator.rs` | — | **一手源已查证**：OS UWP 无通用禁用不透明度档（`SystemControlDisabled*` 是纯字面转发），唯一明写的是列表行 `ListViewItemDisabledThemeOpacity` = **0.55**（`themeresources.xaml` L1772） | **结案为 Kanesumi 取值**：0.38 属自定，不再是「待取数」；若要向 UWP 靠，只需把列表行单独提到 0.55 |
| ~~T5~~ | ~~亮色悬停 / 按下 tint~~ | 黑 **10% / 20%** | `kanesumi-core/src/indicator.rs` | UWP 亮色 `SystemListLowColor` / `SystemListMediumColor` | **一手源已取到**：`#19000000` / `#33000000`（`themeresources.xaml` L4144/L4145） | ✅ 2026-09-22 结案（旧值 5%/10% 无依据；UWP 两方案强度对称） |
| T6 | `on_accent` 判据 | 黑 / 白取对比度大者 | `kanesumi-core/src/accent.rs` | 与 Chorus `derive_accent()` **有意不一致**（Chorus 用亮度 0.5 阈值，青绿上白字仅 4.36:1） | 本表 §二·D1 | **有意偏离**：Chorus 侧应对齐 |
| T7 | 字重 | `FontWeight` 是死字段（5 种字重视觉全同） | `kanesumi-canvas/src/text.rs` 只按 `fonts[0]` 光栅化 | 按字重选字面 / 可变字体轴 | 待定 | 未开工（`docs/MATURITY_AUDIT_2026-09-22.md` P2-4） |
| ~~T8~~ | ~~两个 `press_tint` 同名不同义~~ | — | — | **已消除**：按角色拆为 `MetroIndication::press_tint`（**20%**，控件按压）与 `press_subtle_tint`（10%，大面积 / 次要按压），`MetroColors::press_tint` 删除 | `ROADMAP.md` §Ⅴ-4 | ✅ 2026-09-22（20% 为 2026-09-22 一手源更正後的值） |
| T9 | 焦点描边取值 | 由 accent 派生（暗色 = Light2），**原值 `#FFA626` 借自合成器 Dock 聚焦指示线** | `kanesumi-core/src/accent.rs` `focus_for` | 由 accent 派生的正典机制 | 本表 §二·D2 | 机制已换；取值变化需一次视觉确认 |
| T10 | xdg-shell 角色不吃损伤重绘 | 全量重绘 | `kanesumi-harness/src/platform.rs` | 与 CPU 路径同等的损伤重绘 | — | 未开工（审计 §Ⅳ） |
| ~~T11~~ | ~~亮色 `list_hover_tint`~~ | — | — | **已消除**：一手源证明 UWP 列表行悬停就是 `HighlightListLow` = 10%（`themeresources.xaml` L1783→L228/L4144），故该令牌与 `hover_tint` 同值同义，已整体删除 | `UWP_PRIMARY_SOURCES.md` §Ⅱ | ✅ 2026-09-22 |
| ~~T12~~ | ~~ProgressBar 错误态指示条色~~ | `#E81123`（`StatusColors::error_fill`） | `kanesumi-core/src/status.rs` | — | **一手源查证**：UWP 的 `Error` 视觉状态**不换色**，只把指示条 `Opacity` 归零（`generic.xaml` L12228-12235） | **改判为有意偏离**（D8）：桌面上「消失」比「变色」更不可读；数值仍属 Kanesumi |
| ~~T13~~ | ~~SwipeControl Danger 项底色~~ | `#E5534A`（`StatusColors::danger_fill`） | `kanesumi-core/src/status.rs` | — | **一手源查证**：UWP 的 `SwipeItem` **没有**危险底色（危险语义靠调用方自赋 `Background`），其 `PointerOver` 是空状态、`Pressed` = 中性 40% | **改判为有意偏离**（D9）：本库提供内置 Danger 色与鼠标悬停反馈；**已知缺口**：它是文字底，与 `on_surface` 对比度仅约 3.2:1，待真机定夺换值或改用自动前景 |
| ~~T14~~ | ~~`accent_low_tint` 强度~~ | — | — | **已消除**：一手源给出 `HighlightListAccentLow` = accent **0.6 暗 / 0.4 亮**，与列表行选中共用同一笔刷，故令牌删除、选中底统一走 `selection_tint` | `UWP_PRIMARY_SOURCES.md` §Ⅱ | ✅ 2026-09-22 |
| T15 | ProgressBar 轨道底 | `surface_variant` 60%（`MetroColors::track_subtle`） | `kanesumi-core/src/colors.rs` | 亮色轨道与指示条的对比度应 ≥3.0（非文本阈值） | — | **已知缺口**：亮色实测仅 2.7~2.8（轨道本就浅灰再叠白），属亮色中性色待打磨（T3 家族）；`colors.rs` 的自检已钉住「不得更差」的下界。UWP 的轨道笔刷名仍待取（`ProgressBar` 模板内是模板局部） |
| T16 | 前景强度档 0.5 / 0.7 | `inactive_opacity` / `placeholder_focused_opacity` | `kanesumi-core/src/indicator.rs` | UWP `TextFillColorDisabled` / `TextControlPlaceholderForegroundFocused` | **一手源**：Focused 占位 = `SystemChromeBlackMediumLowColor` `#66000000`（黑 40%），但它建立在「暗色聚焦文本框变纯白底」之上（`TextControlBackgroundFocused` = `#FFFFFFFF`）——本库不采用白纸行为，故该值不可直接移植 | 待裁定：**要么接受白纸行为并照抄，要么明确登记为有意偏离**；`secondary_opacity` 0.8 同理（UWP 无独立 0.8 档） |
| T17 | 自定义的「Y 下沉」按压反馈（M6-1 计划） | 计划 100ms Y 下沉 | `kanesumi-anim`（未落地） | UWP 桌面语义是**缩小 + 倾斜**（写 `Projection`/`RenderTransform`），时长与幅度 OS 预置、XAML 读不到 | `UWP_PRIMARY_SOURCES.md` §Ⅲ.5 | **不得写成 UWP 规格**：若实现下沉，须在此登记为 Kanesumi 自定 |
| T18 | ProgressRing 时长与角度 | 2.0s 循环 / 0→900° | `kanesumi-controls/src/progress.rs`（Ring 部分） | **UWP OS 一手值：3.47s / −110°→585°（净 +695°）/ 6 点 stagger 0.167s**（`generic.xaml` L12406-12506） | 同上 | 待裁定：现值为已丢弃快照的遗产，与其留一个来源不明的值，不如二选一后按 UWP 重定 |
| T19 | 文本选区高亮 35% | `MetroColors::text_selection_tint` | `kanesumi-core/src/colors.rs` | UWP `TextControlSelectionHighlightColor` = `SystemControlHighlightAccentBrush` = **accent 100% 不透明**（`themeresources.xaml` L864→L282） | 同上 | **有意偏离候选**：accent 100% 叠在字形之下会压字，35% 是可读性与「看得出选中」的折中 —— 需一次真机确认后转 §二 正式登记 |
| T20 | **强调色档位派生与 OS 不一致** | RGB 向白 lerp `[0.25,0.45,0.65]` / 向黑 lerp `[0.20,0.35,0.50]` | `kanesumi-core/src/accent.rs`（`LIGHT_MIX` / `DARK_MIX`） | OS 的真算法是 **HSV 明度缩放**：Light1/2/3 = V×1.17 / ×1.56 / ×1.925，Dark1/2/3 = V×0.905 / ×0.689 / ×0.49（越界后 V 饱和 + 饱和度下降） | **本机 WinRT 真值**（`UISettings.GetColorValue`，见 `WINDOWS_RESEARCH_BACKLOG.md` §B4）：以 `#0078D4` 为例，OS = `#0091F8`/`#4CC2FF`/`#99EBFF`/`#0067C0`/`#003E92`/`#001A68`，本仓 = `#409ADF`/`#73B5E7`/`#A6D0F0`/`#0060AA`/`#004E8A`/`#003C6A` | 待裁定：**原声明「档位对齐 Win10 SystemAccentColor」已证伪**（注释已改）。二选一：① 按 V 缩放模型改（`accent.rs` 一处 + 以上表为回归数据，约 30 分钟）；② 维持自定档位、把声明删干净。改则所有 hover/pressed/focus/on-accent 派生色随之变化，宜与 T9 真机确认同批 |

> **已消除的临时项**（保留在此作为历史，避免再次被误认）：
> - 「省略号未启用、超长文本硬裁切」——2026-09-22 已改默认 `Ellipsis` 并补 `label/paragraph`。
> - 「强调色无法派生 Light/Dark 档、控件 hover 不变色」——2026-09-22 已由 `Accent` 补齐。
> - 「只有一套暗色常量、无亮色」——2026-09-22 已由 `MetroColors::{dark,light}` 补齐结构（数值见 T3~T5）。
> - 「InfoBar / InfoBadge 内联四个状态色（且只有暗色版）」——2026-09-22 已由 `StatusColors` 令牌化，
>   数值取自 WinUI 3 开源仓；语义色随方案变、不随 accent 变。
> - 「控件里散着颜色字面量与魔法 alpha」——2026-09-22 已全部迁到令牌（M1-2），并由
>   `kanesumi-controls/tests/token_discipline.rs` 静态守住（M1-3）；无源取值转为 T12~T16 登记。
> - 「InfoBar 图标方块写死白字」——2026-09-22 改为 `Color::most_readable_on`（恒 ≥4.58:1），
>   白字画在暗色成功绿上只有 1.9:1 的问题不复存在。
> - 「浅叠族 15% / 25%」——2026-09-22 一手源证明是「笔刷名对、百分比错」（真值 Secondary 5.9%、
>   Tertiary 3.9%，且按下比悬停更淡），两个令牌删除、消费者改用 `hover_tint`/`press_tint`/`subtle_tint`。
> - 「列表行悬停 30%」——一手源证明 UWP 用 `ListLow`（10%）；30% 出自一个**从未被引用的** Win8 遗留键。
> - 「按下 tint 22%」「BaseMediumHigh 90%」「BaseMediumLow 35%」「ComboBox 聚焦衬底 24% + 边框」
>   「输入框悬停边框 ×90%」「弹窗收起 0.26s」——均于 2026-09-22 被一手源更正，见 `UWP_PRIMARY_SOURCES.md` §Ⅱ。

---

## 二、有意偏离正典 / UWP 的登记

偏离**不是错误**，但必须写明理由，否则半年后会被当成笔误改回去（`PORT_ROADMAP.md` §Ⅰ 已提出此风险）。

| # | 偏离 | 理由 |
|---|---|---|
| D1 | `on_accent` 不用「相对亮度 > 0.5」阈值，改取黑/白对比度大者 | 固定阈值在中等亮度 accent 上不达 WCAG 4.5:1（青绿 `#00897B` 白字仅 4.36:1）。取大者已证明覆盖全部亮度恒 ≥4.5。 |
| D2 | 焦点描边由 accent 派生，而非固定的 Dock 指示线色 | 单一强调色是正典 §Ⅲ.3 的支柱；焦点色若独立于 accent，换 accent 就会「一屏两色」。 |
| D3 | 暗色遮罩用黑 70%，而非 UWP dark 的白 60% | Ether 的 `surface` 与 `background` 只差一档，白洗会让背景比对话框更亮，层次倒挂（参 `CONTROL_SPEC.md` §9 + `VISUAL_ISSUES.md` V9）。 |
| D4 | 圆角默认 `Square`，不用 UWP 的 `ControlCornerRadius=4px` | 正典 §Ⅲ.1「直角」；UWP 默认圆角属 Fluent 残留。 |
| D5 | 不搬「依赖属性 / 附加属性」体系 | Rust 侧代价大于收益；只借其「失效传播」语义（参 `docs/REFERENCE.md` §Ⅴ）。 |
| D6 | 正典不再规定「OLED 纯黑底」，改为「深浅两套并列方案」 | 原表述把**暗色方案的一个取值**写成了语言级规则 —— 纯黑是暗色模式的说法，浅色模式另有其道。文档局限已修正（`KANESUMI_DESIGN.md` §Ⅲ.3，2026-09-22）。 |
| D7 | 「单一强调色」限定为**非语义**配色；语义状态色另成一组 | 维护者裁定（方案 B）。语义色与强调色是两类东西：InfoBar 用强调色表达「错误」，才是真正的不成熟。WinUI 的 `SystemFillColorAttention` 亦绑定 accent —— attention 归 accent，success/caution/critical 归语义色。 |
| D8 | ProgressBar 错误态**保留错误色指示条**，而非 UWP 的「把指示条 Opacity 归零」 | 一手源：UWP 的 `Error` 状态只隐藏指示条（`generic.xaml` L12228-12235），在桌面上「进度条突然消失」比「变红」更难判断。取值 `StatusColors::error_fill`（Kanesumi）。 |
| D9 | SwipeControl 提供内置 Danger 色与**鼠标悬停**反馈 | 一手源：UWP `SwipeItem` 无危险底色（靠调用方自赋 `Background`）、`PointerOver` 是空状态、`Pressed` = 中性 40%。本库面向桌面鼠标：需要可点的危险色与悬停可辨性。**遗留**：Danger 底与 `on_surface` 对比度仅 3.2:1，待真机定夺。 |
| D10 | 暗色下**不采用**「聚焦文本框变纯白底」 | 一手源：UWP 暗色 `TextControlBackgroundFocused` = `#FFFFFFFF`（并把 `RequestedTheme` 切 Light），其占位笔刷随之用「黑 40%」。这与 Kanesumi 的深底空间语言冲突（参 SD §II/§III），故保留暗底暗字，`placeholder_focused_opacity` 亦不照抄（T16）。 |

---

## 三、维护者待裁定

> 2026-09-22 复检：**T1 已随正典修正解除**（「纯黑」不再是语言级规则）；**T8 已消除**（按角色拆名）。
> 2026-09-22 M1 收官复检：新增 T12~T16 均为「机制已对、数值待权威值或实测」的类型，不阻塞路线。
> 2026-09-22 **一手源取证复检**（Windows 侧取到 OS UWP 字典与 WinUI 2.x 源码字典，见
> `UWP_PRIMARY_SOURCES.md`）：**T4/T5/T11/T12/T13/T14 六项结案**（两项改判为有意偏离 D8/D9、
> 两项因令牌删除而消失、T5 按权威值落地），**T16 拆出「白纸行为」这一前置裁定**，
> **新增 T17/T18/T19**（按压下沉无一手依据、ProgressRing 时长与角度、选区高亮 35%）。
> 仍属真缺口的只有 **T13 遗留（危险底对比度 3.2:1）与 T15（亮色轨道 2.7）**，各需一次真机确认。

1. **T2**：`theme.toml` 缺失时的回退强调色，是否就用 `#E57812`？（现 `Accent::DEFAULT_HEX` 与 Chorus `Theme::default()` 一致。）
2. **T9**：焦点描边改用 accent 派生档后，是否需要一次真机视觉确认再定稿？
3. **T13 / T15**：SwipeControl 危险项的文字可读性、亮色 ProgressBar 轨道与指示条的分辨度 ——
   两条都需要一次真机确认；`colors.rs` 与 `status.rs` 的自检已把下界钉住，改值时会立刻有测试反馈。
4. **T16 前置**：是否接受 UWP 的「聚焦文本框变纯白底」？接受则占位笔刷照抄「黑 40%」，
   不接受则把本库的暗底暗字与 0.7 档正式登记为有意偏离（D10 已先行记录）。
5. **T18**：ProgressRing 二选一 —— 沿用现存 2.0s/900°（已丢弃快照的遗产），
   还是改按 UWP OS 一手值 3.47s / −110°→585° / 6 点 stagger 0.167s。
   后者不只是调参，还要把「单弧旋转」改成「6 点」结构，属 M6 范畴。
6. **T17**：M6-1 的按压反馈是走 UWP 的**缩小+倾斜**，还是保留本库计划的「Y 下沉」？
   前者有语义依据、无时长；后者有观感预期、无任何依据 —— 无论选哪个都要登记。
