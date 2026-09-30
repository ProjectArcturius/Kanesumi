# 控件迁移到元素树 —— 操作规范（E3）

> 上游：`docs/ELEMENT_TREE.md`（设计）。本文是**照做即可**的迁移手册，面向批量迁移控件的执行者。
> 参照实现（先读这三段，照抄结构）：
> - `kanesumi-controls/src/button.rs` 中「元素树接入」一节 —— 纯点击控件；
> - `kanesumi-controls/src/check_box.rs` 同名一节 —— 带状态切换 + 补 `measure`；
> - `kanesumi-controls/src/text_box.rs` 同名一节 —— 焦点 + 键盘 + IME。
> 框架 API：`kanesumi-element/src/widget.rs`（`Widget` trait 与各 `*Ctx`）、`event.rs`（`Event`）。

## §1 原则

1. **旧 API 一律保留**。已有的 `render(theme, engine, rect, scene)` / `measure(...)` / `hit_test(...)` /
   `press` / `drag_to` 等方法不删不改签名 —— 未迁移的 App 还在用。`Widget` 实现**调用**它们。
2. **只加不改**：在控件文件末尾、`#[cfg(test)] mod tests` **之前**新增一节
   `// ── 元素树接入（参 docs/ELEMENT_TREE.md §Ⅹ E3；模板同 button.rs）──`，
   内容 = 动作类型 + `impl kanesumi_element::Widget for X` + `#[cfg(test)] mod tree_tests`。
3. **不改 `lib.rs`**（导出由调度者合并时统一加），不改其他控件文件，不改 `kanesumi-element`。
   发现框架缺能力 → 写进报告，不要自己改框架。
4. 行为与视觉**以旧 `render` 为准**，不借迁移改外观。

## §2 `Widget` 各方法怎么写

| 方法 | 写法 |
|---|---|
| `measure(ctx, available)` | 控件已有 `measure` → 转发（参数从 `ctx.engine()` / `ctx.theme()` 取）。没有 → 按 `docs/CONTROL_SPEC.md` 该控件章节的 MinWidth/MinHeight/Padding 写固有尺寸（参 `check_box.rs`）。**返回值不含 margin**；宽度可随 `available.width` 收缩但不得为 NaN/负数。 |
| `paint(ctx, scene)` | 把框架状态映射到控件自己的状态字段，调旧 `render(ctx.theme(), ctx.engine(), ctx.rect(), scene)`，再把字段恢复（参 `button.rs`）。映射用 `crate::state::control_state(ctx.state())`。控件若有自持的 `hovered: bool` 之类字段，用 `ctx.state().hovered` 赋值。 |
| `event(ctx, event)` | 见 §3。处理了就 `ctx.set_handled()`；改了外观就 `ctx.invalidate_paint()`；改了尺寸就 `ctx.invalidate_measure()`；用户意图用 `ctx.emit(动作)`。 |
| `update(ctx, dt)` | 控件有 `update(dt)` / 动画进度时：调用它，未稳态 `ctx.request_anim_frame()` + `ctx.invalidate_paint()`（参 `text_box.rs` 与 `kanesumi-element/src/visual_state.rs`）。**这里不能改布局**。 |
| `focusable()` | 可交互控件返回 `true`（纯展示控件保持默认 false）。 |
| `focus_visual()` | 控件自己画焦点外观（如输入框 2px 边框）才返回 `false`；否则保持默认（框架画）。 |
| `hit_test(rect, pos)` | 默认整矩形。控件只有部分可点（如开关只有轨道）时转发旧 `hit_test`。纯展示控件若不应挡住下层点击，返回 `false`。 |
| `paint_overflow()` | 旧 `render` 会画出 rect（如徽标外溢 4px）→ 返回对应 `Insets`；否则不写。 |
| `accessibility()` | 返回 `AccessInfo { role, name, value, checked }`（参三个参照控件）。 |

## §3 事件对照（旧手写逻辑 → 框架事件）

| 旧 App 里的做法 | 元素树里 |
|---|---|
| App 判断点在不在控件里再调 `click` | 直接处理 `Event::Click`（框架已判定：同一控件上按下并在其内释放） |
| App 维护 hover，调 `hover(rect, pos)` | `Event::PointerMove { pos }` 里调 `hover(ctx.rect(), pos)`；离开时 `Event::PointerLeave`（整体 hover 状态也可直接读 `ctx.state().hovered`） |
| App 在按下时调 `press(rect, pos)`，移动时 `drag_to(pos)`，释放 `release()` | `PointerDown` → `press`；`PointerMove` → `drag_to`（**框架在按下后自动捕获指针**，移出控件也会收到 Move）；`PointerUp` → `release`。拖拽类控件可不依赖 `Click` |
| App 把键转给控件 | `Event::KeyDown { key, modifiers }`：Enter / Space（`Key::Char(' ')`）激活，方向键调值。**不要处理 `Key::Tab` 与 `Key::Escape`**（留给框架焦点遍历与弹层关闭）——直接 return 不 set_handled |
| App 在失焦时收起状态 | `Event::FocusOut` |

坐标一律是表面坐标；需要控件局部坐标时用 `ctx.rect()` 换算。

## §4 动作类型命名

每个控件定义自己的动作结构体，放在元素树接入一节开头，带 `#[derive(Debug, Clone, PartialEq)]`（能 `Copy`/`Eq` 就加上）：

| 控件语义 | 命名 | 例 |
|---|---|---|
| 开关 / 勾选类 | `<控件>Toggled(pub bool)` 或携带新状态 | `SwitchToggled(true)` |
| 取值类（滑块、评分、数字框） | `<控件>ValueChanged(pub f64)` | `SliderValueChanged(0.5)` |
| 单选 / 选择类 | `<控件>SelectionChanged(pub usize)` | `RadioSelectionChanged(2)` |
| 纯激活 | `<控件>Clicked` | `IconButtonClicked` |

**拖拽中是否每次移动都发 ValueChanged**：发（UWP Slider 的 ValueChanged 在拖动中持续触发）。

## §5 测试（每个控件都必须有，放 `tree_tests`）

用 `kanesumi_element::testing::TestHarness`（**不要**再自己写找字体的函数）：

```rust
#[cfg(test)]
mod tree_tests {
    use super::*;
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{Align, Insets, Key, LayoutProps, WidgetId};

    fn harness() -> (TestHarness, WidgetId) {
        let mut h = TestHarness::new(400.0, 300.0);
        let id = h.tree.insert_with(h.root(), MetroX::new(...),
            LayoutProps { h_align: Align::Start, v_align: Align::Start, ..LayoutProps::default() });
        h.frame();
        (h, id)
    }
    // ...
}
```

必须覆盖：

1. **保险三断言**（一个测试里）：`h.assert_contained()`、`h.assert_no_hit_outside(id)`、
   `h.assert_paint_within(id, <paint_overflow 的值或 Insets::ZERO>)`。建议再测一次「被挤小」的情形：
   用 `LayoutProps { width: Some(40.0), .. }` 强行压窄后三断言仍成立 —— **挤小时绘制越界是真 bug**，
   按 §6 处理。
2. **交互**：点击 / 拖拽 / 键盘各至少一条，断言动作（`h.take::<动作>()`）与控件状态（`h.tree.get::<MetroX>(id)`）。
   夹具方法：`click(id)`、`click_at(pos)`、`press_at(pos, PointerButton::Left)`、`move_to(pos)`、
   `release_at(pos, ..)`、`tab()`、`key(Key::...)`、`key_with(key, modifiers)`、`settle()`（跑完动画）。
3. **可聚焦控件**：`h.tab()` 后 `h.tree.focused() == Some(id)`。
4. 禁用：`h.tree.set_enabled(id, false)` 后点击不产生动作。

## §6 发现旧控件的问题怎么办

- **挤小时画出矩形**（保险断言失败）：这是迁移要抓的真问题。若修法是「把绘制夹到 rect 内 / 加一对
  `push_clip(rect)`/`pop_clip()`」且只动该控件的 `render`，**可以修**，并在提交正文写明「旧行为 → 新行为」；
  若需要改视觉规格或牵涉别的控件，**不修**，把该断言的压窄用例标 `#[ignore = "原因"]` 并在报告里列出。
- 旧逻辑的其他 bug：不修，写进报告「范围外发现」。

## §7 验证与提交

```bash
cargo test -p kanesumi-controls            # 全绿（含既有 470+ 项）
cargo clippy -p kanesumi-controls --all-targets 2>&1 | grep generated
# 告警数不得超过基线：kanesumi-controls (lib) 2、(lib test) 36
```

一个控件一次提交：`feat(controls): <控件名> 接入元素树。`，正文列动作类型、事件映射、测试、以及任何 §6 的修正。
