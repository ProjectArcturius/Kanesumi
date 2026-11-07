// dialog_button.rs —— 对话框命令区按钮：**真框架节点**（可 Tab、可读屏、自己处理激活）。
//
// 为什么单独成控件：对话框按钮此前是盒体自绘的（算矩形 → 画 → 命中），于是
// 「看得见」与「Tab 到得了」「读屏读得到」是三套东西 —— 键盘用户按 Tab 只会落在对话框
// 盒体上，读屏只能靠 `MetroDialog::access_items` 合成一堆**假项**（不参与命中与焦点）。
// 把按钮做成节点后，这三件事由同一份 arrange 结果决定。
//
// 参 CONTROL_SPEC §9（按钮宽 130 / 高 32、默认按钮 Accent、Close 恒最右）、
// docs/ELEMENT_TREE.md §Ⅸ（无障碍契约）、docs/DEV_GUIDE.md §2.6（内容槽与自绘的边界）。

use kanesumi_canvas::Scene;
use kanesumi_core::{Rect, Size};
use kanesumi_element::{Event, EventCtx, Key, MeasureCtx, PaintCtx, Widget};

use crate::button::MetroButton;
use crate::dialog::{DialogButton, DialogResult};
use crate::state::control_state;

/// 命令区按钮宽（CONTROL_SPEC §9）。
pub const DIALOG_BUTTON_W: f32 = kanesumi_core::sizes::DIALOG_BUTTON_WIDTH;
/// 命令区按钮高（CONTROL_SPEC §9）。
pub const DIALOG_BUTTON_H: f32 = kanesumi_core::sizes::DIALOG_BUTTON_HEIGHT;

/// 对话框命令区按钮。
///
/// 尺寸恒为 130×32（规格值，不随标签长度变化）：对话框按钮排是**固定节拍**的一行，
/// 三个按钮各自按标签量宽会让同一对话框在不同语言下宽度不齐。
///
/// 激活（点击 / Enter / Space）时发出 [`DialogResult`]，并关闭**父节点**——父节点即
/// `MetroDialog::show_modal` 打开的那个弹层本体，所以按钮不必自己记住弹层 id。
#[derive(Debug, Clone, PartialEq)]
pub struct MetroDialogButton {
    /// 按钮身份（Primary / Secondary / Close），随 `DialogResult` 报给应用。
    pub slot: DialogButton,
    pub label: String,
    /// 默认按钮（Enter 首选项）：Accent 底，无障碍名字后缀「（默认）」。
    pub is_default: bool,
}

impl MetroDialogButton {
    pub fn new(slot: DialogButton, label: impl Into<String>) -> Self {
        Self {
            slot,
            label: label.into(),
            is_default: false,
        }
    }

    /// 标记为默认按钮（视觉用 Accent 底，读屏名字后缀「（默认）」）。
    pub fn default_button(mut self, is_default: bool) -> Self {
        self.is_default = is_default;
        self
    }

    /// 尺寸（规格固定值）。
    pub fn size() -> Size {
        Size::new(DIALOG_BUTTON_W, DIALOG_BUTTON_H)
    }

    /// 画到 `rect`（即时模式与节点模式共用同一实现，两种模式不可能长得不一样）。
    ///
    /// `state` 由调用方给：节点模式取自 `PaintCtx`（框架状态），即时模式取自对话框
    /// 自己的「当前按钮」字段。
    pub fn paint_at(
        &self,
        theme: &kanesumi_core::MetroTheme,
        engine: &kanesumi_canvas::text::TextEngine,
        state: crate::state::ControlState,
        rect: Rect,
        scene: &mut Scene,
    ) {
        let mut btn = if self.is_default {
            MetroButton::accent(self.label.clone())
        } else {
            MetroButton::new(self.label.clone())
        };
        btn.state = state;
        btn.render(theme, engine, rect, scene);
    }
}

impl Widget for MetroDialogButton {
    fn measure(&mut self, _ctx: &mut MeasureCtx, _available: Size) -> Size {
        Self::size()
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let theme = *ctx.theme();
        let state = control_state(ctx.state());
        let rect = ctx.rect();
        let mut btn = if self.is_default {
            MetroButton::accent(self.label.clone())
        } else {
            MetroButton::new(self.label.clone())
        };
        btn.state = state;
        btn.render(&theme, ctx.engine(), rect, scene);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        match event {
            Event::Click => self.activate(ctx),
            // 与 `MetroButton` 同款：忽略按住连发，否则按住 Enter 会把结果连发多次。
            Event::KeyDown {
                key: Key::Enter | Key::Char(' '),
                repeat: false,
                ..
            } => self.activate(ctx),
            _ => {}
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accessibility(&self) -> Option<kanesumi_element::AccessInfo> {
        // 「默认按钮」在 UIA 里是独立属性，`AccessInfo` 尚无该字段；先按既有约定写在名字里，
        // 与真节点化之前的合成项保持同一句播报，不因改造而丢掉信息。
        let name = if self.is_default {
            format!("{}（默认）", self.label)
        } else {
            self.label.clone()
        };
        Some(kanesumi_element::AccessInfo {
            role: kanesumi_element::AccessRole::Button,
            name,
            value: None,
            checked: None,
        })
    }
}

impl MetroDialogButton {
    /// 激活：报结果 + 关掉包住自己的弹层（父节点 = `open_popup` 的节点本体）。
    fn activate(&mut self, ctx: &mut EventCtx) {
        ctx.emit(DialogResult(self.slot));
        if let Some(parent) = ctx.parent() {
            ctx.close_popup(parent);
        }
        ctx.set_handled();
    }
}
