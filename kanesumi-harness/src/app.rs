use kanesumi_canvas::Scene;
use kanesumi_canvas::text::TextEngine;
use kanesumi_core::{MetroTheme, Size};

use crate::appmenu::{AppMenuHandle, MenuTree};
use crate::role::EtherRole;

/// IME 上下文 / 内容提示 —— 定义于控件库（依赖方向 core ← controls ← harness）。
pub use kanesumi_controls::{ImeContentHint, ImeContext};

/// 浮层表面（layer-shell）层别。App 层枚举，外壳映射到 wlr-layer-shell。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    /// 顶部层（TopBar 同级；浮层面板位于 TopBar 之下/覆盖窗口）。
    Top,
    /// 覆盖层（Launcher 同级；最高，覆盖一切）。
    Overlay,
}

/// 浮层表面（layer-shell）锚点。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchorKind {
    /// 顶部右对齐（TopBar 控制面板）。
    TopRight,
    /// 顶部左对齐。
    TopLeft,
    /// 底部居中（Dock 上方浮层）。
    BottomCenter,
    /// 全屏铺满（Launcher overlay）。四边锚定，尺寸 0 = 自适应铺满。
    Fullscreen,
    /// 仅底部锚定（水平居中，不横向拉伸）—— Dock 右键菜单等固定宽度浮层。
    Bottom,
}

/// 浮层表面 —— 与主表面解耦的独立 layer-shell surface。
///
/// 透明底（`transparent=true` 渲染器），控件（面板/菜单）浮在上方 —— 参 Ether 合成器
/// `collect_layer_draws` 按 layer 渲染外部 layer surface。App 关闭面板时渲染空 Scene
/// （透明不可见），或置 `visible=false` 跳过创建。
///
/// 布局：不占排他区域（Neutral），不参与工作区计算 —— 面板是临时浮层，覆盖窗口即可。
#[derive(Debug, Clone, PartialEq)]
pub struct FloatingLayer {
    /// layer-shell namespace（app_id）。合成器可按此辨识。
    pub app_id: &'static str,
    pub layer: LayerKind,
    pub anchor: AnchorKind,
    /// 逻辑尺寸（宽高）。宽 0 = 随输出自适应（Top 锚定时通常保留）。
    pub width: f32,
    pub height: f32,
    /// 上边距（逻辑像素）。用于让 Top 锚定浮层贴 TopBar 下边（不盖住 bar）。
    pub top_margin: f32,
}

impl FloatingLayer {
    pub const fn new(
        app_id: &'static str,
        layer: LayerKind,
        anchor: AnchorKind,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            app_id,
            layer,
            anchor,
            width,
            height,
            top_margin: 0.0,
        }
    }

    /// 设置上边距（贴 TopBar 下边：传入 BAR_H）。参 settings kanesumi_topbar。
    pub const fn with_top_margin(mut self, margin: f32) -> Self {
        self.top_margin = margin;
        self
    }
}

/// 应用配置 —— 身份 + 启动尺寸。app_id 命名空间 `org.ether.*`（ENCS §XI）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppConfig {
    pub app_id: &'static str,
    pub title: &'static str,
    pub role: EtherRole,
    pub width: f32,
    pub height: f32,
    pub min_width: f32,
    pub min_height: f32,
}

impl AppConfig {
    pub const fn new(
        app_id: &'static str,
        title: &'static str,
        role: EtherRole,
        width: f32,
        height: f32,
    ) -> Self {
        Self {
            app_id,
            title,
            role,
            width,
            height,
            min_width: width,
            min_height: height,
        }
    }

    /// 声明 xdg-shell 最小逻辑尺寸。未调用时保持启动尺寸，避免旧应用被压到布局下限以下。
    pub const fn with_min_size(mut self, width: f32, height: f32) -> Self {
        self.min_width = width;
        self.min_height = height;
        self
    }
}

/// 指针按钮。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

/// 逻辑键 —— 键盘事件（wl_keyboard 经 xkbcommon 语义化）的跨平台契约。
/// 可打印字符（含 shift 符号 / 小键盘）→ `Char`；控制键 → 具名变体；未分类 → `Unknown`（原始 keysym 透传）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// 可打印字符（utf8 语义，如 `'+'`、`'%'`、`'7'`）。
    Char(char),
    Enter,
    Backspace,
    Escape,
    Tab,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    Delete,
    /// 未分类 keysym（原始值透传，App 可自行处理）。
    Unknown(u32),
}

/// 修饰键状态 —— 纯数据、跨平台。事件附带事件发生瞬间的修饰键组合。
/// 由外壳（Wayland `update_modifiers`）维护并注入每个事件；App 据此实现
/// Ctrl+C/V、Shift+范围选等组合（参 `key_to_text_input` 契约：修饰键由宿主组合）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    /// Super / Win / Meta 键。
    pub super_key: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };
}

/// 输入事件 —— 纯数据、跨平台。`x/y` 为表面本地逻辑坐标（指针进入表面后有效）。
/// 非 Copy（IME 变体含 `String`）；App 消费时按值 move。
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// 指针移动。
    PointerMoved { x: f32, y: f32 },
    /// 按下。
    PointerPressed {
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// 释放。
    PointerReleased {
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// 双击 —— 与第二次 `PointerPressed` **同时**下发（后者照常投递，App 的单击
    /// 语义不丢）。外壳按 [`ClickTracker`] 判定：同按钮、间隔 ≤250ms、位移 ≤5px。
    /// App 需「单击选中 / 双击打开」类语义时匹配本变体（不可见双击 = 新单击序列）。
    DoubleClick {
        x: f32,
        y: f32,
        button: PointerButton,
        modifiers: Modifiers,
    },
    /// 滚轮 / 触摸板滚动。`dx`/`dy` 为逻辑像素增量；**正方向 = 表面坐标 +y（下）**，
    /// 即向下滚为正。外壳把 Wayland Axis 的 `discrete`（整格 ~50px）或 `absolute`
    /// （触摸板连续）转换为像素增量。
    Scroll {
        x: f32,
        y: f32,
        modifiers: Modifiers,
    },
    /// 键按下（表面持有键盘焦点时）。释放事件不推（App 一般只关心按下）。
    KeyPressed { key: Key, modifiers: Modifiers },
    /// 指针离开表面。
    PointerLeft,
    /// IME 组合态更新。`cursor_byte` 为组合态内光标字节偏移（None = 光标在尾部，
    /// 对应协议 cursor_begin/end = -1 的隐藏光标）。文本为空 = 清除组合态。
    Preedit {
        text: String,
        cursor_byte: Option<usize>,
    },
    /// IME 提交 —— 以提交文本替换光标处组合态（原子编辑）。
    Commit { text: String },
    /// IME 周边删除 —— 删光标前/后字节数（UTF-8，控件层外扩夹紧）。
    DeleteSurrounding { before_bytes: u32, after_bytes: u32 },
}

/// Kanesumi 应用入口 trait —— 把 Kanesumi 变成应用 SDK 的契约。
///
/// 状态驱动渲染（参 PLAN.md §4-1 / AnimationRules.md §III）：
/// `state → progress → resolved spatial state → render`。
/// App 只产出 `Scene` 绘制命令，GPU 光栅化由 harness 外壳承担——保持纯逻辑、跨平台可测。
pub trait App {
    fn config(&self) -> &AppConfig;

    /// 应用主题（默认 Ether 深色空间桌面）。
    fn theme(&self) -> MetroTheme {
        MetroTheme::ether_dark()
    }

    /// 字体路径。外壳据此加载 `TextEngine` 注入 `render`（排版唯一真源，SD §IX 禁止静默回退）。
    /// 默认 `None` → 外壳按 KANESUMI_TEST_FONT → 系统字体顺序查找。
    fn font_path(&self) -> Option<std::path::PathBuf> {
        None
    }

    /// 请求的动态高度（layer-shell 角色）。返回 `Some(h)` 表示 App 希望表面变为
    /// 该高度（如 TopBar 展开面板）；`None` = 固定 `AppConfig.height`。
    /// 外壳每帧比对后经 wlr-layer-shell `set_size` 提交并**立即生效**（不等 configure
    /// 往返，参旧 topbar.rs `set_height` 模式）；合成器按 cached_state.size.h 扩大命中
    /// 与渲染区域（参 Ether compositor input/pointer.rs `topbar_height`）。
    fn preferred_height(&self) -> Option<f32> {
        None
    }

    /// 浮层表面列表（与主表面解耦的独立 layer-shell surface）。
    /// 外壳启动时为每个浮层创建 surface + 渲染器（透明底），每帧按 `render_floating`
    /// 产出 Scene。面板关闭时 App 应渲染空 Scene（透明不可见）。
    fn floating_layers(&self) -> Vec<FloatingLayer> {
        Vec::new()
    }

    /// 浮层 `index` 是否可见。false → 外壳不渲染该浮层（表面空闲，不请求 frame）。
    /// 默认 true（面板常驻渲染）；Launcher 用此控制开/合（配合 floating_height）。
    fn floating_visible(&self, _index: usize) -> bool {
        true
    }

    /// 浮层 `index` 是否仍需重绘（有动画 / 内容脏）。
    ///
    /// 返回 `false` 时外壳停止请求该浮层的 frame callback（表面空闲，零 CPU）。
    /// 浮层动画由主表面 `update` 推进，动画跑完即可停（对称主表面 `needs_redraw`）。
    /// 默认 `true`（保守：每帧重绘，行为同旧版）。
    fn floating_needs_redraw(&self, _index: usize) -> bool {
        true
    }

    /// 渲染第 `index` 个浮层表面。`size` 为该表面逻辑尺寸（configure 后有效）。
    fn render_floating(&mut self, _engine: &TextEngine, _index: usize, _size: Size) -> Scene {
        Scene::default()
    }

    /// 浮层表面输入事件（指针坐标 = 该表面本地逻辑坐标）。外壳按指针所在表面路由。
    fn floating_input(&mut self, _index: usize, _event: InputEvent) {}

    /// 浮层请求高度（动态显示/收起）。返回 0 = 收起（表面高度 0，无命中无渲染）。
    /// 外壳每帧比对后经 `set_size` 立即生效（参主表面 `preferred_height` 同款机制）。
    fn floating_height(&self, _index: usize) -> f32 {
        0.0
    }

    /// 每帧 tick。`dt` 单位为秒（外壳从 frame callback 计算，参 PLAN.md §4.2 合成器时钟）。
    fn update(&mut self, _dt: f64) {}

    /// 本帧是否有需要呈现的状态变化（「内容脏了吗」；TOPBAR_RENDER_REFACTOR §4.6 I-3）。
    ///
    /// 契约变更（2026-08-16）：语义从「恒 true / 每帧画」改为**脏标记消费前查询**：
    /// - App 在 `update()` / `handle_input()` 中置位自身脏标记（时钟变化 / hover /
    ///   press / 面板开合 / 菜单打开 / 动画推进中等）；
    /// - **App 在 `render()` 末尾清除脏标记**（render 与 commit 一一对应）；
    /// - 返回 true → 外壳置主表面 dirty → 渲染 + commit（静止时零提交）；
    /// - 动画推进中（Progress/SpringAnim 未稳态）应返回 true，外壳以 frame 回调
    ///   作 vsync 提示逐帧渲染（回调丢失有 16ms 超时兜底，绝不冻结）。
    ///
    /// 默认 `true`（保守：每帧渲染，行为同旧版）。应用可覆盖以省 CPU。
    fn needs_redraw(&self) -> bool {
        true
    }

    /// 表面键盘焦点变化回调。`focused=false` 时 App 应关闭弹层（右键菜单/对话框等），
    /// 避免「失焦残留」（参 CONTEXT_MENU_SPEC §Ⅵ LightDismiss 之外的应用侧关闭路径）。
    /// 外壳在 wl_keyboard enter/leave 时调用。
    fn focus_changed(&mut self, _focused: bool) {}

    /// 是否请求关闭表面（窗口/进程退出）。App 置 true（如文件选择器点选完成交付结果后），
    /// 外壳每帧轮询 → 置 running=false → 主循环退出（进程正常收尾）。
    fn should_close(&self) -> bool {
        false
    }

    /// 输入事件（指针位置为表面本地逻辑坐标）。控件命中测试由 App 负责（参 HANDOVER §2 输入层）。
    fn handle_input(&mut self, _event: InputEvent) {}

    /// IME 焦点上下文。返回 `Some(ImeContext)` = 当前有文本输入焦点（TextBox/PasswordBox 等），
    /// 外壳据此 enable text-input 并灌周边文本/光标矩形；`None` = 无 IME（默认）。
    fn ime_focus(&self) -> Option<ImeContext> {
        None
    }

    /// 是否作为 IME 引擎宿主（`zwp_input_method_v2` 引擎侧）。`true` 时外壳绑定
    /// `zwp_input_method_manager_v2` + `grab_keyboard`，把合成器转发的按键经
    /// `ime_engine_key` 投递给本 App，并把 `ime_engine_preedit`/`ime_engine_commit`
    /// 回传上屏。仅 Ceyboard（Candidate 角色）返回 true。
    fn ime_engine_host(&self) -> bool {
        false
    }

    /// 引擎宿主：处理一个按键（来自 `zwp_input_method_keyboard_grab_v2` 的 key 事件，
    /// 已由外壳经 xkbcommon 语义化为 [`Key`] + 修饰键）。引擎内部更新组合态。
    /// `modifiers` 供引擎宿主处理组合键（如 Ctrl+Space 切中英）。
    /// **返回 `true` = 引擎消费了该键**（不重放）；`false` = 未消费，外壳经
    /// `zwp_virtual_keyboard_v1` 重放给焦点客户端（fcitx5 同款透传策略，
    /// arrow/backspace/Home 等导航键须透传，否则焦点应用收不到）。参 CEYBOARD_SPEC §Ⅴ。
    fn ime_engine_key(&mut self, _key: Key, _modifiers: Modifiers) -> bool {
        false
    }

    /// 引擎宿主：当前组合态 preedit（拼音串）及光标字节偏移。
    /// 返回 `(preedit, cursor_byte)`；空串 = 无组合态。
    fn ime_engine_preedit(&self) -> (String, Option<usize>) {
        (String::new(), None)
    }

    /// 引擎宿主：取走待提交文本（选词/空格/回车后引擎产出的 commit）。
    /// 返回 `Some(text)` = 上屏；`None` = 无待提交。
    fn ime_engine_take_commit(&mut self) -> Option<String> {
        None
    }

    /// 引擎宿主：取走待删除的周边字节数（退格等）。返回 `(before, after)`。
    fn ime_engine_take_delete(&mut self) -> (u32, u32) {
        (0, 0)
    }

    /// 引擎宿主：候选窗 popup surface 的 Scene（画在 `zwp_input_popup_surface_v2` 上，
    /// 合成器渲染到 Layer 6 Overlay 并跟随光标）。空 Scene = 不显示候选窗。
    fn ime_engine_popup_scene(&mut self, engine: &TextEngine) -> kanesumi_canvas::Scene {
        let _ = engine;
        kanesumi_canvas::Scene::default()
    }

    /// 引擎宿主：候选窗 popup surface 尺寸（逻辑像素，0×0 = 不显示）。
    /// 合成器据 popup 内容渲染；本值供外壳创建/调整 surface。
    fn ime_engine_popup_size(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// 全局应用菜单声明。返回 `Some(tree)` = 启用 AppMenu —— 外壳（Linux）自动完成
    /// com.canonical.dbusmenu 服务 + org_kde_kwin_appmenu Wayland 绑定 +
    /// com.canonical.AppMenu.Registrar 兜底注册（参 appmenu 模块）；`None` = 无全局菜单。
    /// 点击事件经 `on_menu_command(id)` 路由回应用。
    fn app_menu(&self) -> Option<MenuTree> {
        None
    }

    /// 全局菜单点击回调。`id` = MenuItem::id。外壳在主线程每帧排干命令通道后调用。
    fn on_menu_command(&mut self, _id: i32) {}

    /// 全局菜单句柄注入（外壳安装 AppMenu 后调用一次）。App 保存句柄用于运行时
    /// 更新菜单勾选 / 结构（`AppMenuHandle::set_check` / `update_tree`，主题切换等）。
    fn set_appmenu_handle(&mut self, _handle: AppMenuHandle) {}

    /// 右键菜单内容。`(x, y)` 为表面本地逻辑坐标（右键按下点）。
    /// 返回 `Some(items)` = harness 接管右键路由，在指针位置弹出右键菜单（该右键
    /// 事件不再投递 `handle_input`）；`None` = 无右键菜单，事件照常投递（默认）。
    /// `&mut self`：App 可在返回菜单前更新目标状态（如文件浏览器右键先选中命中项）。
    /// 参 CONTEXT_MENU_SPEC §Ⅵ。
    fn context_menu(&mut self, _x: f32, _y: f32) -> Option<Vec<kanesumi_controls::MenuItem>> {
        None
    }

    /// 右键菜单项点击回调。`path` 为命令路径：顶层 = `[i]`；级联 = `[parent, child]`。
    /// App 据此执行对应命令。参 CONTEXT_MENU_SPEC §Ⅵ。
    fn on_context_command(&mut self, _path: &[usize]) {}

    /// 渲染一帧：把当前状态解析为绘制命令。
    /// `engine` 为外壳注入的 TextEngine（排版唯一真源），App 用它量测文本、外壳用它光栅化。
    fn render(&mut self, engine: &TextEngine, size: Size) -> Scene;
}

/// harness `Key` → 控件层 `TextInputKey` 转换（TextBox 等文本控件路由用）。
///
/// **契约：仅做键身份（identity）映射，不处理修饰键。** 修饰键状态由宿主在调用处
/// 自行维护并组合，`key_to_text_input` 完全不感知 Shift/Ctrl/Alt/Super。典型模式：
///
/// - **Shift + 方向 = 选区扩展**：宿主检测 Shift 按下，把
///   `TextField::move_left/right/…` 的 `select=true` 传下去。
/// - **Ctrl + A / Ctrl + C / Ctrl + Z / Ctrl + V**：宿主检测 Ctrl，不走本函数，
///   直接调 `TextField::select_all` / `copy` / `undo` / `insert`。
/// - **Ctrl + Home/End = 跳到首/尾字符**：同上，由宿主组合。
///
/// 键身份来自 wl_keyboard 经 xkbcommon 语义化后的 keysym，已考虑键盘布局
/// （Dvorak / QWERTY / IME latin），不受物理 scancode 影响。
///
/// `Unknown` / 无法映射的键 → `None`（宿主可不消费）。
pub fn key_to_text_input(key: Key) -> Option<kanesumi_controls::TextInputKey> {
    use kanesumi_controls::TextInputKey as K;
    match key {
        Key::Char(c) => Some(K::Char(c)),
        Key::Enter => Some(K::Enter),
        Key::Backspace => Some(K::Backspace),
        Key::Escape => Some(K::Escape),
        Key::Tab => Some(K::Tab),
        Key::Left => Some(K::Left),
        Key::Right => Some(K::Right),
        Key::Up => Some(K::Up),
        Key::Down => Some(K::Down),
        Key::Home => Some(K::Home),
        Key::End => Some(K::End),
        Key::Delete => Some(K::Delete),
        Key::Unknown(_) => None,
    }
}

/// IME 使能动作 —— `compute_ime_action` 的产物（幂等 reconcile 的决策核心，
/// 参 IME_WIRING_PLAN 阶段 D）。纯逻辑，无 Wayland 依赖，跨平台可测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImeAction {
    /// 表面持键盘焦点 + App 有文本输入焦点 → enable + 灌上下文。
    Enable,
    /// 任一焦点缺失 → disable。
    Disable,
}

/// 幂等决策：根据当前期望（`focus_surface && focus_control`）与已发送状态比较，
/// 返回需要执行的动作。**只在状态翻转时返回动作**（不翻转 = None，无协议流量）。
pub fn compute_ime_action(
    focus_surface: bool,
    focus_control: bool,
    currently_enabled: bool,
) -> Option<ImeAction> {
    let want = focus_surface && focus_control;
    if want == currently_enabled {
        None
    } else if want {
        Some(ImeAction::Enable)
    } else {
        Some(ImeAction::Disable)
    }
}

/// 一帧待应用 IME 事件批 —— done 事件到达前累积的 pending 状态。
/// 协议批次 = 一帧内 Preedit/Commit/DeleteSurrounding 的积压，`done` 触发应用。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PendingImeBatch {
    pub preedit: Option<String>,
    pub cursor_begin: i32,
    pub cursor_end: i32,
    pub commit: Option<String>,
    pub delete_before: u32,
    pub delete_after: u32,
}

impl PendingImeBatch {
    /// 按协议 done 序列展开为 InputEvent 流：`DeleteSurrounding → Commit → Preedit`
    /// （参 text-input-unstable-v3 done 事件说明）。空字段不产生事件；
    /// 空 preedit 产生 `Preedit { text: "", … }`（= 清除组合态）。
    pub fn apply(&self) -> Vec<InputEvent> {
        let mut out = Vec::new();
        if self.delete_before > 0 || self.delete_after > 0 {
            out.push(InputEvent::DeleteSurrounding {
                before_bytes: self.delete_before,
                after_bytes: self.delete_after,
            });
        }
        if let Some(text) = &self.commit
            && !text.is_empty()
        {
            out.push(InputEvent::Commit { text: text.clone() });
        }
        if let Some(text) = &self.preedit {
            // cursor_begin = -1（隐藏光标）→ None；否则取 begin 字节。
            let cursor_byte = if self.cursor_begin >= 0 {
                Some(self.cursor_begin as usize)
            } else {
                None
            };
            out.push(InputEvent::Preedit {
                text: text.clone(),
                cursor_byte,
            });
        }
        out
    }

    /// done 应用：`serial == current_serial`（非 stale 帧）才返回事件流，否则 None。
    pub fn apply_done(&self, serial: u32, current_serial: u32) -> Option<Vec<InputEvent>> {
        if serial != current_serial {
            return None;
        }
        Some(self.apply())
    }
}

/// 双击判定参数。参 UWP `GetDoubleClickTime`（默认 500ms）/ X11 `multi-click time`
/// 惯例；Kanesumi 取 250ms（轻盈短促铁律对齐）、位移 5px 容差。
const DOUBLE_CLICK_MS: u32 = 250;
const DOUBLE_CLICK_TOLERANCE_PX: f32 = 5.0;

/// 双击检测器 —— 记录每次指针按下，判定「同按钮、间隔 ≤250ms、位移 ≤5px」为双击。
///
/// 语义：第二次按下判定为双击（外壳随后下发 `InputEvent::DoubleClick`）；判定后复位，
/// 故三次快速点击 = 单击 + 双击 + 单击（第三次重新开始计数，Windows 惯例）。
#[derive(Debug, Clone, Default)]
pub struct ClickTracker {
    last_button: Option<PointerButton>,
    last_time_ms: u32,
    last_x: f32,
    last_y: f32,
}

impl ClickTracker {
    /// 记录一次按下（`time_ms` 为事件时间戳，同序单调）。返回 true = 构成双击。
    pub fn record(&mut self, button: PointerButton, x: f32, y: f32, time_ms: u32) -> bool {
        let double = self.last_button == Some(button)
            && time_ms >= self.last_time_ms
            && time_ms.saturating_sub(self.last_time_ms) <= DOUBLE_CLICK_MS
            && (x - self.last_x).abs() <= DOUBLE_CLICK_TOLERANCE_PX
            && (y - self.last_y).abs() <= DOUBLE_CLICK_TOLERANCE_PX;
        if double {
            // 判定后复位：第三次快速点击视为新单击（Windows 双击语义）。
            self.last_button = None;
            self.last_time_ms = 0;
            self.last_x = 0.0;
            self.last_y = 0.0;
        } else {
            self.last_button = Some(button);
            self.last_time_ms = time_ms;
            self.last_x = x;
            self.last_y = y;
        }
        double
    }

    /// 指针离开表面时复位（跨表面的快速点击不算双击）。
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_maps_to_text_input() {
        assert_eq!(
            key_to_text_input(Key::Char('a')),
            Some(kanesumi_controls::TextInputKey::Char('a'))
        );
        assert_eq!(
            key_to_text_input(Key::Backspace),
            Some(kanesumi_controls::TextInputKey::Backspace)
        );
        assert_eq!(
            key_to_text_input(Key::Up),
            Some(kanesumi_controls::TextInputKey::Up)
        );
        assert_eq!(key_to_text_input(Key::Unknown(0x00ff)), None);
    }

    #[test]
    fn compute_ime_action_only_flips_on_change() {
        // 期望 = focus_surface && focus_control
        assert_eq!(
            compute_ime_action(true, true, false),
            Some(ImeAction::Enable),
            "两焦点齐 → enable"
        );
        assert_eq!(
            compute_ime_action(true, true, true),
            None,
            "已 enable 且仍期望 → 幂等无动作"
        );
        assert_eq!(
            compute_ime_action(true, false, true),
            Some(ImeAction::Disable),
            "控件失焦 → disable"
        );
        assert_eq!(
            compute_ime_action(false, true, true),
            Some(ImeAction::Disable),
            "表面离开 → disable"
        );
        assert_eq!(
            compute_ime_action(false, false, false),
            None,
            "已 disable → 幂等无动作"
        );
    }

    #[test]
    fn pending_batch_applies_in_protocol_order() {
        let b = PendingImeBatch {
            preedit: Some("nǐ".into()),
            cursor_begin: 3,
            cursor_end: 3,
            commit: Some("你好".into()),
            delete_before: 3,
            delete_after: 6,
        };
        let events = b.apply();
        assert!(matches!(
            events[0],
            InputEvent::DeleteSurrounding {
                before_bytes: 3,
                after_bytes: 6
            }
        ));
        assert!(matches!(&events[1], InputEvent::Commit { text } if text == "你好"));
        assert!(
            matches!(&events[2], InputEvent::Preedit { text, cursor_byte } if text == "nǐ" && *cursor_byte == Some(3))
        );
    }

    #[test]
    fn pending_batch_stale_serial_is_dropped() {
        let b = PendingImeBatch {
            commit: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(b.apply_done(1, 1).unwrap().len(), 1, "匹配 serial 生效");
        assert_eq!(b.apply_done(0, 1), None, "stale serial 丢弃");
    }

    #[test]
    fn pending_batch_empty_fields_produce_no_events() {
        let b = PendingImeBatch::default();
        assert!(b.apply().is_empty());
    }

    #[test]
    fn pending_batch_cursor_neg_one_is_hidden() {
        let b = PendingImeBatch {
            preedit: Some("ab".into()),
            cursor_begin: -1,
            cursor_end: -1,
            ..Default::default()
        };
        let events = b.apply();
        assert!(matches!(
            &events[0],
            InputEvent::Preedit {
                cursor_byte: None,
                ..
            }
        ));
    }

    #[test]
    fn pending_batch_empty_preedit_clears() {
        let b = PendingImeBatch {
            preedit: Some(String::new()),
            ..Default::default()
        };
        let events = b.apply();
        assert!(matches!(&events[0], InputEvent::Preedit { text, .. } if text.is_empty()));
    }

    #[test]
    fn modifiers_default_none() {
        let none = Modifiers::default();
        assert_eq!(none, Modifiers::NONE);
        assert!(!none.ctrl && !none.shift && !none.alt && !none.super_key);
    }

    #[test]
    fn modifiers_in_events() {
        let ctrl_shift = Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        };
        let ev = InputEvent::KeyPressed {
            key: Key::Char('c'),
            modifiers: ctrl_shift,
        };
        match ev {
            InputEvent::KeyPressed { key, modifiers } => {
                assert_eq!(key, Key::Char('c'));
                assert!(modifiers.ctrl && modifiers.shift);
            }
            _ => panic!("类型不符"),
        }
    }

    // ── ClickTracker（双击检测） ────────────────────────────────────────────

    #[test]
    fn click_tracker_second_quick_press_is_double() {
        let mut t = ClickTracker::default();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1000));
        assert!(t.record(PointerButton::Left, 10.0, 10.0, 1100), "250ms 内同点同按钮 = 双击");
        // 复位后第三次快速点击重新计数（新单击）。
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1200), "双击后复位，第三次为新单击");
    }

    #[test]
    fn click_tracker_slow_second_press_is_single() {
        let mut t = ClickTracker::default();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1000));
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1300), ">250ms 不算双击");
    }

    #[test]
    fn click_tracker_move_resets_interval() {
        let mut t = ClickTracker::default();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1000));
        assert!(!t.record(PointerButton::Left, 30.0, 10.0, 1100), "位移 >5px 不算双击");
    }

    #[test]
    fn click_tracker_different_button_is_single() {
        let mut t = ClickTracker::default();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1000));
        assert!(!t.record(PointerButton::Right, 10.0, 10.0, 1100), "不同按钮不算双击");
    }

    #[test]
    fn click_tracker_reset_clears() {
        let mut t = ClickTracker::default();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1000));
        t.reset();
        assert!(!t.record(PointerButton::Left, 10.0, 10.0, 1100), "离开表面后复位");
    }
}
