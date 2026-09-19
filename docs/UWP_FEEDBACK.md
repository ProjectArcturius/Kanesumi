# UWP → Kanesumi 反馈表

> **这是本扇区最重要的产出。** 在真 UWP 上做控件会拿到自绘路线拿不到的东西：
> 官方默认值、真实视觉状态机、平台行为的边界情况。这些必须回流，否则本扇区
> 就只是一次性支出。
>
> 规则（参 `AGENTS.md` 铁律 4）：**发现即记录，不要攒着。**
>
> 回流目标有三个：
> - `Kanesumi/CONTROL_SPEC.md` —— 补全「未在快照中」的条目；修正与官方默认不符的读数
> - `Kanesumi/PORT_ROADMAP.md` —— 闭源控件由「逆推」升级为「实测确证」
> - `Ether/docs/KANESUMI_DESIGN.md` —— 若发现正典未覆盖的规格

---

## 记录模板

每条反馈用这个结构。**「差异」一栏是价值所在** —— 一致的地方不必记，
不一致的地方才是自绘路线会做错的地方。

```markdown
### <控件名> · <一句话结论>

- **UWP 实证**：实测到的具体值 / 行为（附观察方式：截图、Inspector、代码位置）
- **现行规格**：`CONTROL_SPEC.md §N` 或 `PORT_ROADMAP.md` 里现在怎么写的
- **差异**：两者不同在哪
- **处置**：改规格 / 改实现 / 标注为刻意偏离（并说明为什么）
- **状态**：⬜ 待确认 · 🔧 待修正 · ✅ 已回流
```

---

## 已确认：圆角全局归零成立

- **UWP 实证**：`ControlCornerRadius` 与 `OverlayCornerRadius` 是全局资源，
  在 `App.xaml` 覆盖为 `0` 后，Button / TextBox / ComboBox / ListView /
  Flyout / ContentDialog 的圆角一并归零，**无需重写任何控件模板**。
  依据：`App.xaml` 的覆盖，以及我方对 `MainPage.xaml` 控件验证台的目视核验。
- **现行规格**：`Kanesumi/CONTROL_SPEC.md` 未记录圆角来源 —— 因为自绘路线
  根本不需要（`SceneCommand::FillRect` 没有 radius 字段）。
- **差异**：**架构层面的差异，不是数值差异。**
  自绘路线是「结构性排除」：命令模型里不存在圆角这个概念。
  WinUI 2 是「覆盖默认值」：框架有能力画圆角，靠资源覆盖关掉。
- **处置**：**不改 Kanesumi 规格**，但应在 `CONTROL_SPEC.md` 补一节说明：
  「WinUI 2 扇区依赖两个全局资源归零；新增控件时须核验其是否经由这两个资源」。
  这是**跨扇区知识**，自绘路线不会遇到。
- **状态**：✅ 已确认

---

## 待记录（验证台逐项核验后填写）

- [ ] `Button` / `AccentButtonStyle` —— 圆角、按下反馈（UWP 是位移还是缩放？）
- [ ] `TextBox` —— 圆角、聚焦边框（focus stroke 厚度与颜色）
- [ ] `ToggleButton` / `CheckBox` / `RadioButton` —— 开关几何
- [ ] `ComboBox` —— 下拉浮层圆角（走 Overlay）与选中项背板
- [ ] `Slider` —— 轨道与滑块的圆角（正典：`Capsule` 仅限此类结构件）
- [ ] `ListViewItem` —— **选中背板圆角**（最容易残留的地方）
- [ ] `ProgressBar` —— 轨道与指示条圆角
- [ ] `InfoBar` —— 圆角、强调色用法
- [ ] `Flyout` / `ContentDialog` —— Overlay 圆角是否同样归零
- [ ] `ScrollBar` —— 粗细、圆角

### 需要特别留心的一类：动效时长

正典给出的是**Kanesumi 的取值**，不是 UWP 默认值：

| 项 | Kanesumi（Rust 侧权威） | UWP 默认（待测） |
|---|---|---|
| 标准时长 | `METRO_STANDARD_DURATION = 0.25s` | ? |
| 快速切换 | `QUICK_SWITCH = 0.167s` | ? |
| 开关翻转 | `TOGGLE_FLIP = 0.15s` | ? |
| 默认缓动 | `Quadratic / EaseOut` | ? |

**若 UWP 默认与 Kanesumi 不同，这就是一条高价值反馈** —— 它说明自绘路线
（按正典取值）与真 UWP 会在手感上分叉，需要决定以谁为准。

### 另一类：正典未覆盖的规格

已知一处上游空白：**IME 组合串的视觉规格**（下划线样式、颜色、透明度）。
`IME_WIRING_PLAN.md` 明确写「CONTROL_SPEC 未定 preedit 规格，走平台默认」。
UWP 的 TextBox 有真实实现 —— **实测它，然后把规格补进正典。**
这是本扇区能独立完成、自绘路线做不到的事。
