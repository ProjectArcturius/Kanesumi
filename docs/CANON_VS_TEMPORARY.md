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
| T3 | 亮色中性色 | `#FAFAFA` / `#FFFFFF` / `#F0F0F0` / `#D6D6D6` / `#1A1A1A` / `#5A5F66` | `kanesumi-core/src/colors.rs` `MetroColors::light` | UWP light 主题的 `SystemControl*` 笔刷字面值 | Windows SDK `themeresources.xaml`（定位见 `CONTROL_SPEC.md` §11.1） | **待取数**（`CONTROL_SPEC.md` §11.4 早已列为「取具体值待做」）；当前值仅保证 WCAG 可读 |
| T4 | 亮色禁用不透明度 | `0.38`（沿用暗色） | `kanesumi-core/src/indicator.rs` | UWP light 取值 | 同 T3 | 待取数 |
| T5 | 亮色悬停 / 按下 tint | 黑 5% / 黑 10% | `kanesumi-core/src/indicator.rs` | UWP light `HighlightListLow/Medium` 字面值 | 同 T3 | 待取数 |
| T6 | `on_accent` 判据 | 黑 / 白取对比度大者 | `kanesumi-core/src/accent.rs` | 与 Chorus `derive_accent()` **有意不一致**（Chorus 用亮度 0.5 阈值，青绿上白字仅 4.36:1） | 本表 §二·D1 | **有意偏离**：Chorus 侧应对齐 |
| T7 | 字重 | `FontWeight` 是死字段（5 种字重视觉全同） | `kanesumi-canvas/src/text.rs` 只按 `fonts[0]` 光栅化 | 按字重选字面 / 可变字体轴 | 待定 | 未开工（`docs/MATURITY_AUDIT_2026-09-22.md` P2-4） |
| T8 | `MetroColors::press_tint` 与 `MetroIndication::press_tint` | 白 10% 与白 22%，**两个不同的按下 tint 并存** | `colors.rs` / `indicator.rs` | 收敛为一个令牌 | 待裁定 | 待收敛（重复即漂移） |
| T9 | 焦点描边取值 | 由 accent 派生（暗色 = Light2），**原值 `#FFA626` 借自合成器 Dock 聚焦指示线** | `kanesumi-core/src/accent.rs` `focus_for` | 由 accent 派生的正典机制 | 本表 §二·D2 | 机制已换；取值变化需一次视觉确认 |
| T10 | xdg-shell 角色不吃损伤重绘 | 全量重绘 | `kanesumi-harness/src/platform.rs` | 与 CPU 路径同等的损伤重绘 | — | 未开工（审计 §Ⅳ） |

> **已消除的临时项**（保留在此作为历史，避免再次被误认）：
> - 「省略号未启用、超长文本硬裁切」——2026-09-22 已改默认 `Ellipsis` 并补 `label/paragraph`。
> - 「强调色无法派生 Light/Dark 档、控件 hover 不变色」——2026-09-22 已由 `Accent` 补齐。
> - 「只有一套暗色常量、无亮色」——2026-09-22 已由 `MetroColors::{dark,light}` 补齐结构（数值见 T3~T5）。

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

---

## 三、维护者待裁定

1. **T1**：暗色底用正典的 OLED 纯黑 `#000000`，还是保留 `#1A1A1A` 以与合成器桌面底色拉开层次？（正典写的是 `#000000`。）
2. **T2**：`theme.toml` 缺失时的回退强调色，是否就用 `#E57812`？（现 `Accent::DEFAULT_HEX` 与 Chorus `Theme::default()` 一致。）
3. **T8**：两个 `press_tint` 收敛到哪一个值？
4. **T9**：焦点描边改用 accent 派生档后，是否需要一次真机视觉确认再定稿？
