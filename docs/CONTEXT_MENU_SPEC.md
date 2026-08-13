# Kanesumi 右键菜单规格（CONTEXT_MENU_SPEC）

> 上下文菜单（Context Menu）产品/控件规格。参 `CONTROL_SPEC.md` §8（MenuFlyout 项规格）、
> §39（MenuFlyoutSubItem / RadioMenuFlyoutItem）、`popup.rs`（弹层定位/动画）、
> `kanesumi-harness/src/app.rs`（App trait / floating_layers 浮层机制）。
>
> 状态：**已实现（2026-08-13）**。行为参考 Windows 上下文菜单（UWP MenuFlyout / ContextFlyout）。
> Kanesumi 定夺值标注「**定**」，参考值标注「参考」，需 Gallery 逆推确认的标注「待逆推」。

---

# Ⅰ · 定位

## 1. 一句话

**右键菜单是 Kanesumi 的上下文命令面板**：指针按下右键 → 在指针位置弹出菜单，展示
对当前目标（文件/文本/控件/空白区）可用的命令，供点选/键盘执行。

## 2. 与既有菜单的关系

| 菜单 | 触发器 | 锚点 | 遮罩 | 现状 |
|---|---|---|---|---|
| MenuBar（§菜单栏） | 顶栏项 | 顶栏项下方 | 无（面板下接） | ✅ 已有 |
| DropdownMenu（§8） | 点击触发器 | 触发器下方/上方二择 | ✅ 有 | ✅ 已有 |
| **ContextMenu（本文）** | **右键按下** | **指针坐标，四象限翻折** | **无** | ❌ 待做 |

> **核心差异**：下拉菜单锚定触发器矩形（`place_popup`），右键菜单锚定**指针点**（无触发器），
> 四个方向都翻折（`place_context_menu`，本文 §Ⅲ）。

## 3. 职责边界

- 菜单**内容**（哪些命令可用、图标、勾选态）由 App 决定（目标感知）。
- 菜单**行为**（定位、hover、键盘导航、关闭、级联）由控件层（`MetroContextMenu`）负责。
- 右键**路由**（谁收到右键、坐标转换）由 harness 外壳负责（浮层表面分发）。

> **铁律**：控件层不持目标/命令语义——只消费 `Vec<MenuItem>` 纯数据（同 `MetroDropdownMenu`）。

---

# Ⅱ · 触发与关闭

## 1. 触发

| 输入 | 行为 |
|---|---|
| 右键 **Pressed**（`PointerButton::Right`） | 记录指针坐标 `(x, y)`，请求 App 提供菜单 |
| 右键 Pressed 在右键菜单**已打开**的浮层表面上 | 关闭当前菜单（再点开新的，Windows 惯例） |
| 长按（触摸，≈500ms） | Phase 2：等价右键（Windows 长按右键惯例，待定） |

> **按下触发，非释放**：与 Windows 传统一致（右键按下即出菜单，释放即已可点选）。

## 2. 关闭

| 输入 | 行为 |
|---|---|
| 点菜单外（指针 Pressed 落在浮层外） | LightDismiss 关闭（Windows 惯例） |
| `Esc` | 关闭（栈式：先收级联，再收顶层） |
| 左键点选菜单项 | 关闭 + 回调命令 |
| 再按右键 | 关闭 + 重开 |
| 指针离开表面 | 不关闭（浮层保持，Windows 惯例） |

---

# Ⅲ · 定位算法（四象限翻折）

## 1. 目标

菜单面板矩形右上角 = 指针位置；四方向（右下/左下/右上/左上）自适应，**始终完整在屏内**。

## 2. 优先级（锚点 = 指针点 `p`，面板 `w×h`，屏幕 `S`）

```
1. 右下：x = p.x,      y = p.y        （右侧空间够 w 且下方够 h）
2. 左下：x = p.x - w,  y = p.y        （右侧不够，左移）
3. 右上：x = p.x,      y = p.y - h    （下方不够，上移）
4. 左上：x = p.x - w,  y = p.y - h    （两向都不够）
```

每步后夹紧：`x = clamp(x, S.left, S.right - w)`；`y = clamp(y, S.top, S.bottom - h)`。
面板宽高大于屏幕时贴屏边（以屏为基准，允许轻微越界则不裁，参 `place_submenu` 夹紧惯例）。

## 3. 接口（`popup.rs` 新增）

```rust
/// 右键菜单定位：锚定指针点，四象限翻折，始终屏内。
#[must_use]
pub fn place_context_menu(anchor: Point, panel_size: Size, screen: Rect) -> Rect {
    // 按 §2 优先级尝试右下/左下/右上/左上，逐位夹紧。
}
```

> 与 `place_popup`（触发器下方/上方二择）和 `place_submenu`（父项右侧/左侧）并列，
> 三者互不替代。

---

# Ⅳ · 视觉规格

## 1. 形状（复用 CONTROL_SPEC §8 MenuFlyout，覆盖点见下）

| 项 | 值 | 依据 |
|---|---|---|
| 圆角 | **`Square`（直角）** | Kanesumi 铁律（参 KANESUMI_DESIGN §1） |
| 边框 | 1px，`divider` | §8 Presenter 1px 边框 |
| 阴影 | **无** | 铁律 6：深度靠明暗不靠阴影 |
| 底色 | `surface`（不透明） | §8 Metro 时代纯色底（不做 Acrylic） |
| **遮罩** | **无**（本文与 DropdownMenu 关键差异） | Windows 上下文菜单无遮罩，LightDismiss |
| 面板最小宽 | 80px（触控 240） | §8 下拉面板 MinWidth |

## 2. 菜单项（全量复用 §8 / §39 规格）

| 项 | 值 |
|---|---|
| 项高 | **32**（Padding `11,9,11,10`） |
| 图标 | 16×16，有图标/勾选时占位 28px |
| 快捷键 | 右对齐 |
| 分隔线 | 高 1、左右留白 12、`divider` |
| PointerOver | **中性高亮**（非强调色；列表类悬停用中性，通用规律 5） |
| Pressed | 更高中性高亮 |
| 颜色切换 | **瞬时**（通用规律 1） |
| 子菜单 | `MenuFlyoutSubItem` 级联（§39）：悬停展开二级，`place_submenu` 定位 |

## 3. 动画

| 场景 | 参数 | 依据 |
|---|---|---|
| 面板弹出 | `sheet_appear`（0.30s）+ 淡入 | §8 MenuFlyout 弹出动画（~200ms 位移+淡入 ease-out，Kanesumi 用 sheet 对齐） |
| 面板关闭 | `sheet_dismiss`（0.26s）+ 淡出 | 同上 |
| **无遮罩轨道** | 不渲染 `render_overlay`，仅面板动画 | 本文差异 |

> 复用 `PopupAnim`（遮罩轨道留空即可），不新造动画。

---

# Ⅴ · 行为与交互

## 1. 指针

| 操作 | 行为 |
|---|---|
| 悬停菜单项 | PointerOver 中性高亮；悬停嵌套项 → 展开级联（§39） |
| 左键点选 | 关闭 + 回调 `command` |
| 悬停级联与顶层切换 | 打开的子菜单随之切换（§39 `hover_other_item_swaps_submenu`） |
| 点外（浮层外 Pressed） | LightDismiss 关闭 |

## 2. 键盘（焦点在浮层表面时）

| 键 | 行为 |
|---|---|
| ↑ / ↓ | 高亮移动（跨分隔线跳项；同 §8 键盘遍历） |
| → | 进入子菜单；← 返回父菜单 |
| Enter / 空格 | 激活高亮项 |
| `Esc` | 收级联 → 关菜单 |
| Tab | 循环遍历（Phase 2） |

## 3. 级联（§39 复用）

- 悬停/→ 展开二级；子面板定位 `place_submenu`（父项右侧 → 越屏翻左）。
- 点击子菜单项返回 `(parent, child)` 路径。

---

# Ⅵ · App / Harness 契约

## 1. App trait 新增钩子（`app.rs`）

```rust
/// 右键菜单内容。`(x, y)` 为表面本地逻辑坐标（右键按下点）。
/// 返回 `Some(items)` = 在指针位置弹出右键菜单；`None` = 无右键菜单（默认）。
fn context_menu(&self, _x: f32, _y: f32) -> Option<Vec<MenuItem>> {
    None
}

/// 右键菜单项点击回调。`path` = 顶层 + 级联索引路径（同 DropdownMenu）。
fn on_context_command(&mut self, _path: &[usize]) {}
```

- 默认 `None` → App 完全不感知右键（现有 App 零改动）。
- `Some` → harness 接管右键路由，不把 `PointerPressed{Right}` 再投给 `handle_input`。

## 2. Harness 路由（`platform.rs` / `app.rs`）

```
PointerPressed { button: Right, x, y }
  → App::context_menu(x, y)
      ├─ None   → 事件照常投 handle_input（App 自处理右键）
      └─ Some   → 打开右键菜单浮层（LayerOverlay）
                    ├─ 定位：place_context_menu(指针点, 面板尺寸, 表面)
                    ├─ 渲染：MetroContextMenu → render_floating
                    ├─ 输入：floating_input → 菜单状态机（hover/click/Esc）
                    └─ 点选 → on_context_command(path) → 关闭浮层
```

## 3. 浮层载体

复用 App trait 既有 `floating_layers()` / `render_floating` / `floating_input` /
`floating_height` 机制（layer-shell OVERLAY 独立表面）——**外壳零新增通道**，
只在 App 侧新增一个「右键菜单管理」helper（见 §Ⅶ 实现方案）。

---

# Ⅶ · 实现方案

## 1. 控件层：`MetroContextMenu`（`kanesumi-controls/src/context_menu.rs`）

- 复用 `MenuItem` 类型（`dropdown_menu.rs`）——与 DropdownMenu 共用数据结构。
- 内部持有 `MetroDropdownMenu`（级联/单选组/项渲染全部复用）+ `PopupAnim`（遮罩轨道留空）。
- 新增状态：
  - `anchor: Point`（右键按下点，定位基准）
  - `open: bool`（浮层是否可见）
- 新增方法：
  - `open_at(anchor: Point, items: Vec<MenuItem>, screen: Rect)`
  - `panel_rect() -> Rect`（`place_context_menu` 结果）
  - `hit_menu(rect) / handle_input(InputEvent)`（复用 DropdownMenu 状态机）
- 定位：`place_context_menu`（§Ⅲ）。

## 2. Harness helper：右键菜单管理

`kanesumi-harness` 提供 `ContextMenuState`（App 侧持有）：

```rust
/// 右键菜单状态机（App 持有一个，外壳注入右键事件）。
pub struct ContextMenuState {
    menu: MetroContextMenu,
    open: bool,
}
impl ContextMenuState {
    pub fn update(&mut self, dt: f64);                      // 动画 tick
    pub fn handle_pointer(&mut self, ev: &InputEvent, screen: Rect); // Right→open、外点→close
    pub fn render(&self, engine: &TextEngine) -> Scene;     // 空 Scene = 关闭（浮层透明）
    pub fn height(&self) -> f32;                            // 0 = 收起
}
```

- App 在 `floating_layers()` 声明一个 OVERLAY 浮层（如 `FloatingLayer::overlay("context")`）。
- `render_floating` → `ContextMenuState::render`；`floating_input` →
  `handle_pointer` + 菜单点选 → `on_context_command`。

---

# Ⅷ · 验收标准

- 任意 Kanesumi App（Gallery 起步）在任意位置右键 → 菜单在指针位置弹出。
- 四象限翻折正确（屏角右键菜单不越界）。
- 级联子菜单、单选组、分隔线、图标、快捷键全复用正常。
- 点外 / Esc / 再右键 均正确关闭。
- 右键菜单打开期间，底层 App 收不到右键事件（路由接管），其余输入不受影响。
- 现有 App（未实现 `context_menu`）零改动、无回归。

---

# Ⅸ · 施工顺序

> ✅ 2026-08-13 全部完成（本规范即实现记录）。

1. ✅ `popup.rs`：`place_context_menu`（四象限定位 + 单测）。
2. ✅ `kanesumi-controls`：`MetroContextMenu`（复用 DropdownMenu + 遮罩留空 + 单测）。
3. ✅ `kanesumi-harness`：App trait 两钩子（`context_menu` / `on_context_command`）+
   `ContextMenuState` helper + Gallery 右键路由。
4. ✅ Gallery 演示（任意位置右键 + 级联 + 命令回显）。
5. ✅ CONTROL_MATRIX 登记。
