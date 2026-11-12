// style.rs —— 命名画刷 + 视觉状态表：控件不再在 `paint` 里按状态挑颜色。
//
// 为什么（2026-11-11，参 docs/WINUI_REFERENCE_MAP.md #5）：C# 侧一个按钮的常态 / 悬停 /
// 按下只是 `Kanesumi.Xaml/Themes/Controls/Buttons.xaml:14` 的一条
// `{ThemeResource KPressTintBrush}`，加上同文件 `VisualStateManager` 的几行声明；Rust 侧却在
// 每个控件的 `paint` 里 `match ctx.state()` 手写每态画笔（`button.rs:99-123`）。控件越写越厚，
// 同一套状态语义也在各控件里各写一遍。本层只做「命名画刷 + 状态表」：控件声明一份
// `ControlStyle`，框架用 [`resolve`] 按状态取画笔；不做 ControlTemplate，也不做样式字典 /
// 资源解析器（裁定不做，参 docs/WINUI_REFERENCE_MAP.md #5 与 docs/IDIOMS.md §Ⅴ）。
//
// 状态名与 C# `VisualState.Name` 同字面（PascalCase，参 docs/IDIOMS.md §Ⅰ.2）。
// 取值一律是 `ThemeColor` 令牌 —— 本层不出现任何色值（tests/token_discipline.rs 会拦）。

use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use kanesumi_core::{Color, MetroTheme, ThemeColor};

use crate::state::{ControlState, control_state};

/// 五个状态的名字（与 XAML `VisualState x:Name` 同字面）。
pub const STATE_NORMAL: &str = "Normal";
pub const STATE_POINTER_OVER: &str = "PointerOver";
pub const STATE_PRESSED: &str = "Pressed";
pub const STATE_DISABLED: &str = "Disabled";
pub const STATE_FOCUSED: &str = "Focused";

/// 禁用态整笔不透明度 —— 一手源 C# `Buttons.xaml` 的 `Disabled` 状态把
/// `ContentPresenter.Opacity` 设成 0.4；本库 §1 通用规律也是「禁用靠整体降透明度」。
pub const DISABLED_OPACITY: f32 = 0.4;

/// 一条命名画刷：主题令牌 + 该画刷自带的不透明度。
///
/// 不直接存 `Color`：主题在绘制时才知道，令牌必须在**当帧主题**下解析
/// （与 `core/src/brush.rs` 同一条规则：提前解析会让换肤后的旧颜色留在控件里）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleBrush {
    pub token: ThemeColor,
    pub opacity: f32,
}

impl StyleBrush {
    pub const fn new(token: ThemeColor) -> Self {
        Self {
            token,
            opacity: 1.0,
        }
    }

    /// 令牌 × 不透明度（如「强调色 40%」）。
    pub const fn with_opacity(token: ThemeColor, opacity: f32) -> Self {
        Self { token, opacity }
    }

    /// 在给定主题下解析为绘制用颜色。
    pub fn color(self, theme: &MetroTheme) -> Color {
        let c = self.token.resolve(theme);
        c.with_alpha(c.a * self.opacity.clamp(0.0, 1.0))
    }
}

/// [`resolve`] 的结果：选中的令牌 + 该状态的不透明度。
///
/// 与 [`StyleBrush`] 一样保留令牌而不落成 `Color`，由 [`Paint::color`] 按当帧主题解析。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Paint {
    pub token: ThemeColor,
    pub opacity: f32,
}

impl Paint {
    /// 按当帧主题解析为绘制用颜色（不透明度已乘进 alpha）。
    pub fn color(self, theme: &MetroTheme) -> Color {
        let c = self.token.resolve(theme);
        c.with_alpha(c.a * self.opacity.clamp(0.0, 1.0))
    }
}

/// [`ControlStyle`] 里的具名画刷槽（五个状态各一个）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushSlot {
    Normal,
    PointerOver,
    Pressed,
    Disabled,
    Focused,
}

/// 一条视觉状态声明：状态名 → 属性集。对应 XAML `VisualState x:Name`。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualStateDecl {
    pub name: &'static str,
    pub properties: StateProperties,
}

/// 状态覆盖的属性集。
///
/// 目前只有「用哪个画刷槽」与「不透明度覆盖」两项 —— 本层不做模板系统，属性集不承诺
/// 扩成任意 XAML `Setter`（裁定不做 ControlTemplate，参 docs/WINUI_REFERENCE_MAP.md #5）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StateProperties {
    pub brush: BrushSlot,
    /// 覆盖画刷自带的不透明度；`None` = 用画刷的。
    pub opacity: Option<f32>,
}

/// 状态表：五个规范状态各一条声明。
///
/// 固定五项而不是 `Vec`：状态名是 WinUI `CommonStates` 的封闭集合，查找不分配、样式可
/// `Copy` 进控件实例。自定义只允许**重映射**已存在的状态（[`VisualStates::set`]），
/// 不接受另造状态名 —— 另造名字只会在跨端对照时多一套叫法。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VisualStates {
    decls: [VisualStateDecl; 5],
}

impl VisualStates {
    /// 默认规则：状态名各取同名的具名画刷，不覆盖不透明度。
    pub const fn derived() -> Self {
        Self {
            decls: [
                VisualStateDecl {
                    name: STATE_NORMAL,
                    properties: StateProperties {
                        brush: BrushSlot::Normal,
                        opacity: None,
                    },
                },
                VisualStateDecl {
                    name: STATE_POINTER_OVER,
                    properties: StateProperties {
                        brush: BrushSlot::PointerOver,
                        opacity: None,
                    },
                },
                VisualStateDecl {
                    name: STATE_PRESSED,
                    properties: StateProperties {
                        brush: BrushSlot::Pressed,
                        opacity: None,
                    },
                },
                VisualStateDecl {
                    name: STATE_DISABLED,
                    properties: StateProperties {
                        brush: BrushSlot::Disabled,
                        opacity: None,
                    },
                },
                VisualStateDecl {
                    name: STATE_FOCUSED,
                    properties: StateProperties {
                        brush: BrushSlot::Focused,
                        opacity: None,
                    },
                },
            ],
        }
    }

    /// 按名取声明（精确匹配，与 XAML `GoToState` 同语义）。
    pub fn get(&self, name: &str) -> Option<&VisualStateDecl> {
        self.decls.iter().find(|d| d.name == name)
    }

    /// 重映射一个已声明状态的属性集；名字不在五态表里返回 `false`（不改表、不 panic）。
    pub fn set(&mut self, name: &str, properties: StateProperties) -> bool {
        match self.decls.iter_mut().find(|d| d.name == name) {
            Some(d) => {
                d.properties = properties;
                true
            }
            None => false,
        }
    }

    /// 状态名 → 属性集。未声明的名字回落到 `Normal` 并计数（不 panic，见 [`resolve`]）。
    fn properties_for(&self, name: &str) -> StateProperties {
        match self.get(name) {
            Some(d) => d.properties,
            None => {
                UNKNOWN_STATE_FALLBACKS.fetch_add(1, Ordering::Relaxed);
                StateProperties {
                    brush: BrushSlot::Normal,
                    opacity: None,
                }
            }
        }
    }
}

impl Default for VisualStates {
    fn default() -> Self {
        Self::derived()
    }
}

/// 一份控件样式：五个具名画刷 + 状态表。
///
/// `normal` 是唯一必填项；其余状态可以缺省 —— 缺省时 [`resolve`] 回落到 `normal`，
/// 默认计数，`KANESUMI_STRICT=1` 下 panic（见 [`resolve`]）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlStyle {
    pub normal: StyleBrush,
    pub pointer_over: Option<StyleBrush>,
    pub pressed: Option<StyleBrush>,
    pub disabled: Option<StyleBrush>,
    pub focused: Option<StyleBrush>,
    pub states: VisualStates,
}

impl ControlStyle {
    /// 只声明常态画刷，其余状态回落到常态。
    pub const fn new(normal: ThemeColor) -> Self {
        Self {
            normal: StyleBrush::new(normal),
            pointer_over: None,
            pressed: None,
            disabled: None,
            focused: None,
            states: VisualStates::derived(),
        }
    }

    /// 强调色控件的五态声明（进度环等）。
    ///
    /// 悬停 / 按下取强调色亮档 / 暗档；禁用取强调色 40%（参 [`DISABLED_OPACITY`]）；
    /// 焦点取 `FocusStroke`。被动指示器收不到悬停 / 按下（`has_visual_states` 缺省 false），
    /// 这两项是**声明**而非当前可观测行为，控件转为可交互时无需再改样式。
    pub const fn accent() -> Self {
        Self {
            normal: StyleBrush::new(ThemeColor::Primary),
            pointer_over: Some(StyleBrush::new(ThemeColor::PrimaryHover)),
            pressed: Some(StyleBrush::new(ThemeColor::PrimaryPressed)),
            disabled: Some(StyleBrush::with_opacity(
                ThemeColor::Primary,
                DISABLED_OPACITY,
            )),
            focused: Some(StyleBrush::new(ThemeColor::FocusStroke)),
            states: VisualStates::derived(),
        }
    }

    /// 按槽取画刷；`Normal` 恒有值。
    pub fn brush(&self, slot: BrushSlot) -> Option<StyleBrush> {
        match slot {
            BrushSlot::Normal => Some(self.normal),
            BrushSlot::PointerOver => self.pointer_over,
            BrushSlot::Pressed => self.pressed,
            BrushSlot::Disabled => self.disabled,
            BrushSlot::Focused => self.focused,
        }
    }
}

/// 状态声明了却没给画刷、回落到 `normal` 的累计次数（进程级）。
static MISSING_BRUSH_FALLBACKS: AtomicU64 = AtomicU64::new(0);
/// 未知状态名回落到 `Normal` 的累计次数（进程级）。
static UNKNOWN_STATE_FALLBACKS: AtomicU64 = AtomicU64::new(0);

/// 缺画刷回落的累计次数。测试与诊断用；只增不减。
pub fn missing_brush_fallbacks() -> u64 {
    MISSING_BRUSH_FALLBACKS.load(Ordering::Relaxed)
}

/// 未知状态名回落的累计次数。测试与诊断用；只增不减。
pub fn unknown_state_fallbacks() -> u64 {
    UNKNOWN_STATE_FALLBACKS.load(Ordering::Relaxed)
}

/// `KANESUMI_STRICT=1` 时把「状态缺画刷」从回落升级为 panic。
///
/// 只读一次：绘制热路径不每帧查环境；严格模式是进程启动期的开关。
fn strict_mode() -> bool {
    static STRICT: OnceLock<bool> = OnceLock::new();
    *STRICT.get_or_init(|| strict_flag(std::env::var("KANESUMI_STRICT").ok().as_deref()))
}

/// 门值判定单独成函数，供单测直接喂值（不在测试里改进程环境）。
fn strict_flag(value: Option<&str>) -> bool {
    value == Some("1")
}

/// `ControlStates` → 状态名（默认推导规则）。
///
/// 优先级与 [`crate::state::control_state`] 相同：禁用 > 按下 > 悬停 > 常态。
/// **不推导 `Focused`**：元素树下键盘焦点由框架统一绘制（`Widget::focus_visual`），控件
/// 再按 `Focused` 换画笔就会出现双层焦点视觉；自绘焦点的控件显式
/// `resolve(style, STATE_FOCUSED)` 即可。
pub fn state_for(states: kanesumi_element::ControlStates) -> &'static str {
    state_name(control_state(states))
}

/// `ControlState` → 状态名（含 `Focused`，供自绘焦点的控件显式取）。
pub fn state_name(state: ControlState) -> &'static str {
    match state {
        ControlState::Normal => STATE_NORMAL,
        ControlState::Hovered => STATE_POINTER_OVER,
        ControlState::Pressed => STATE_PRESSED,
        ControlState::Focused => STATE_FOCUSED,
        ControlState::Disabled => STATE_DISABLED,
    }
}

/// 框架按状态取画刷：状态名 → 状态表属性集 → 具名画刷。
///
/// 两类回落分开处理：
/// - **未知状态名**回落到 `Normal` 并计数，**不 panic**（即使 `KANESUMI_STRICT=1`）：
///   名字是调用方数据，拼错不该炸掉整帧；
/// - **声明了状态却没给画刷**是配置缺陷：默认回落到 `normal` 并计数，
///   `KANESUMI_STRICT=1` 下 panic，消息带门名、状态名与槽名。
pub fn resolve(style: &ControlStyle, state: &str) -> Paint {
    resolve_with_strict(style, state, strict_mode())
}

fn resolve_with_strict(style: &ControlStyle, state: &str, strict: bool) -> Paint {
    let props = style.states.properties_for(state);
    let Some(brush) = style.brush(props.brush) else {
        if strict {
            panic!(
                "KANESUMI_STRICT=1：ControlStyle 缺少状态「{state}」的画刷（槽 {:?}）。\
                 请为该状态声明 StyleBrush，或把该状态从状态表里去掉。",
                props.brush
            );
        }
        MISSING_BRUSH_FALLBACKS.fetch_add(1, Ordering::Relaxed);
        return Paint {
            token: style.normal.token,
            opacity: props.opacity.unwrap_or(style.normal.opacity),
        };
    };
    Paint {
        token: brush.token,
        opacity: props.opacity.unwrap_or(brush.opacity),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::{MetroTheme, Size};
    use kanesumi_element::testing::TestHarness;
    use kanesumi_element::{ControlStates, LayoutProps, Widget};

    use crate::progress::MetroProgressRing;

    fn five_state_style() -> ControlStyle {
        ControlStyle {
            normal: StyleBrush::new(ThemeColor::Primary),
            pointer_over: Some(StyleBrush::new(ThemeColor::PrimaryHover)),
            pressed: Some(StyleBrush::new(ThemeColor::PrimaryPressed)),
            disabled: Some(StyleBrush::with_opacity(
                ThemeColor::Primary,
                DISABLED_OPACITY,
            )),
            focused: Some(StyleBrush::new(ThemeColor::FocusStroke)),
            states: VisualStates::derived(),
        }
    }

    #[test]
    fn five_states_pick_their_named_brush() {
        let theme = MetroTheme::ether_dark();
        let style = five_state_style();
        for (name, token) in [
            (STATE_NORMAL, ThemeColor::Primary),
            (STATE_POINTER_OVER, ThemeColor::PrimaryHover),
            (STATE_PRESSED, ThemeColor::PrimaryPressed),
            (STATE_FOCUSED, ThemeColor::FocusStroke),
        ] {
            let paint = resolve(&style, name);
            assert_eq!(paint.token, token, "状态「{name}」取错画刷");
            assert_eq!(paint.opacity, 1.0, "状态「{name}」不透明度");
            assert_eq!(paint.color(&theme), token.resolve(&theme));
        }
        // 禁用态的令牌仍是 Primary，但整笔乘 0.4（C# Disabled 的 ContentPresenter.Opacity）。
        let disabled = resolve(&style, STATE_DISABLED);
        assert_eq!(disabled.token, ThemeColor::Primary);
        assert_eq!(disabled.opacity, DISABLED_OPACITY);
        let c = disabled.color(&theme);
        assert!((c.a - theme.colors.primary.a * DISABLED_OPACITY).abs() < 1e-6);
    }

    #[test]
    fn unknown_state_falls_back_to_normal_without_panic() {
        let style = five_state_style();
        let before = unknown_state_fallbacks();
        // 大小写不对、完全是别的词，都按未知处理；严格模式下也不得 panic。
        for name in ["pointerover", "Hover", ""] {
            let paint = std::panic::catch_unwind(|| resolve_with_strict(&style, name, true))
                .expect("未知状态名不得 panic");
            assert_eq!(paint.token, style.normal.token);
            assert_eq!(paint.opacity, style.normal.opacity);
        }
        assert!(
            unknown_state_fallbacks() >= before + 3,
            "未知状态名应逐个计数"
        );
    }

    #[test]
    fn missing_brush_falls_back_to_normal_and_counts() {
        let mut style = five_state_style();
        style.disabled = None;
        let before = missing_brush_fallbacks();
        let paint = resolve_with_strict(&style, STATE_DISABLED, false);
        assert_eq!(paint.token, style.normal.token);
        assert_eq!(paint.opacity, style.normal.opacity);
        assert!(
            missing_brush_fallbacks() > before,
            "缺画刷回落必须计数（默认不 panic）"
        );
    }

    #[test]
    fn strict_mode_panics_with_readable_message() {
        let mut style = five_state_style();
        style.disabled = None;
        let err = std::panic::catch_unwind(|| resolve_with_strict(&style, STATE_DISABLED, true))
            .expect_err("KANESUMI_STRICT=1 下缺画刷必须 panic");
        let msg = err
            .downcast_ref::<String>()
            .map(String::as_str)
            .unwrap_or_default();
        assert!(msg.contains("KANESUMI_STRICT=1"), "消息应带门名：{msg}");
        assert!(msg.contains(STATE_DISABLED), "消息应带状态名：{msg}");
        assert!(msg.contains("Disabled"), "消息应带槽名：{msg}");
    }

    #[test]
    fn strict_flag_only_accepts_literal_one() {
        assert!(strict_flag(Some("1")));
        for v in [Some("0"), Some("true"), Some(""), None] {
            assert!(!strict_flag(v), "只有字面量 1 打开严格模式：{v:?}");
        }
    }

    #[test]
    fn state_derivation_follows_control_states_priority() {
        let base = ControlStates::default();
        assert_eq!(state_for(base), STATE_NORMAL);
        assert_eq!(
            state_for(ControlStates {
                hovered: true,
                ..base
            }),
            STATE_POINTER_OVER
        );
        assert_eq!(
            state_for(ControlStates {
                hovered: true,
                pressed: true,
                ..base
            }),
            STATE_PRESSED,
            "按下优先于悬停"
        );
        assert_eq!(
            state_for(ControlStates {
                hovered: true,
                pressed: true,
                disabled: true,
                ..base
            }),
            STATE_DISABLED,
            "禁用优先于按下"
        );
        assert_eq!(
            state_for(ControlStates {
                focused: true,
                ..base
            }),
            STATE_NORMAL,
            "焦点视觉由框架画，默认推导不取 Focused（参 state.rs 文件头）"
        );
        assert_eq!(
            state_name(ControlState::Focused),
            STATE_FOCUSED,
            "自绘焦点的控件仍可显式取 Focused"
        );
    }

    #[test]
    fn state_table_can_remap_but_not_invent_names() {
        let mut style = five_state_style();
        let pressed = style.pressed.unwrap();
        assert!(style.states.set(
            STATE_POINTER_OVER,
            StateProperties {
                brush: BrushSlot::Pressed,
                opacity: Some(0.5),
            }
        ));
        let paint = resolve(&style, STATE_POINTER_OVER);
        assert_eq!(paint.token, pressed.token, "重映射到按下画刷");
        assert_eq!(paint.opacity, 0.5, "状态级不透明度覆盖画刷自带值");
        assert!(
            !style.states.set(
                "Hover",
                StateProperties {
                    brush: BrushSlot::Pressed,
                    opacity: None,
                }
            ),
            "五态表是封闭集合，不另造状态名"
        );
    }

    /// 状态切换 → 只重画，不重新量测（Kanesumi 铁律 4：状态只动视觉属性）。
    ///
    /// 用计数包装器量 `measure` 次数：`set_enabled` 让子树失效后出一帧，矩形不变、量测次数
    /// 也不得增加；同帧弧色应已切到样式表的禁用画刷。
    #[test]
    fn state_switch_repaints_without_remeasuring() {
        use std::cell::Cell;
        use std::rc::Rc;

        struct CountingRing {
            inner: MetroProgressRing,
            measures: Rc<Cell<usize>>,
        }

        impl Widget for CountingRing {
            fn measure(&mut self, ctx: &mut kanesumi_element::MeasureCtx, available: Size) -> Size {
                self.measures.set(self.measures.get() + 1);
                self.inner.measure(ctx, available)
            }

            fn paint(
                &mut self,
                ctx: &mut kanesumi_element::PaintCtx,
                scene: &mut kanesumi_canvas::Scene,
            ) {
                self.inner.paint(ctx, scene);
            }
        }

        let measures = Rc::new(Cell::new(0));
        let mut h = TestHarness::new(300.0, 100.0);
        let id = h.tree.insert_with(
            h.root(),
            CountingRing {
                inner: {
                    let mut ring = MetroProgressRing::new();
                    ring.set_value(0.5); // 确定进度 0 时环形不画弧，看不出颜色
                    ring
                },
                measures: measures.clone(),
            },
            LayoutProps::default(),
        );
        h.settle();
        let measured = measures.get();
        assert!(measured > 0, "首帧必须量测过");

        h.tree.set_enabled(id, false);
        h.frame();
        assert_eq!(
            measures.get(),
            measured,
            "状态切换不得触发重新量测（只重画）"
        );

        let color = h
            .last
            .scene
            .commands
            .iter()
            .find_map(|c| match c {
                kanesumi_canvas::SceneCommand::Arc { color, .. } => Some(*color),
                _ => None,
            })
            .expect("禁用态仍应画弧（active=true）");
        let primary = h.tree.theme().colors.primary;
        assert!(
            (color.a - primary.a * DISABLED_OPACITY).abs() < 1e-6,
            "禁用弧应取样式表的禁用画刷（主色 × 0.4），实际 alpha {}",
            color.a
        );
    }
}
