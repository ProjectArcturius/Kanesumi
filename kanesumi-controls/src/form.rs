// form.rs —— 页面语法（form grammar）：页标题 / 分区标题 / 行 / 说明 / 链接 / 滑杆。
//
// 存在理由：设置类页面几乎全是同一套结构 ——「分区标题 + 一行一个设置项，标签在上、
// 控件在下、说明在最下」。此前这套结构以 755 行私有 `settings/src/window/grammar.rs`
// 的形式存在，等于每个应用自己写一遍页面框架（盘点：settings 36 个自绘控件、librarian
// 15 个、launcher 10 个）。提升到控件库后，写页面只组合语义块，几何与间距由本模块负责，
// 应用不再各写一套。参 `docs/CONTROL_SPEC.md` §49、`docs/DEV_GUIDE.md` §二·五。
//
// 铁律（裁定 `DECISIONS_2026-10-05.md` §110）：**一个控件 = 标签在上（14）+ 控件 +
// 可选说明在下（14 次级色）**，左对齐单列；禁止 Win11 式「左标题右控件」。将来若要支持
// 那种排布，必须作为独立布局变体显式 opt-in，不得改本默认。
//
// 尺寸全部引用 `kanesumi_core::sizes`（页面语法族 `FORM_*`），颜色只用 `ThemeColor` 令牌：
// 本模块内不出现尺寸字面量与颜色构造器，由 `tests/size_discipline.rs` 与
// `tests/token_discipline.rs` 静态守住。

use std::sync::Arc;

use kanesumi_canvas::{Icon, Scene, TextAlign};
use kanesumi_core::sizes;
use kanesumi_core::typography::FontWeight;
use kanesumi_core::{MetroTheme, Size, TextStyle, ThemeColor};
use kanesumi_element::widgets::{Label, Stack};
use kanesumi_element::{
    Align, Event, EventCtx, Insets, LayoutProps, MeasureCtx, PaintCtx, Tree, Widget, WidgetId,
};

use crate::button::MetroButton;
use crate::slider::MetroSlider;
use crate::surface::MetroSurface;
use crate::switch::MetroSwitch;

// ── 语义尺寸（只引用尺寸表，不在此另立值）──────────────────────────────────

/// 内容列设计宽。
pub const COL_WIDTH: f32 = sizes::FORM_COL_WIDTH;
/// 「相关设置」栏宽。
pub const RELATED_WIDTH: f32 = sizes::FORM_RELATED_WIDTH;
/// 内容列与相关设置栏的间隔。
pub const CONTENT_GAP: f32 = sizes::FORM_CONTENT_GAP;
/// 相关设置栏上内边距。
pub const RELATED_TOP: f32 = sizes::FORM_RELATED_TOP;
/// 页标题下距。
pub const PAGE_TITLE_MB: f32 = sizes::FORM_PAGE_TITLE_MB;
/// 分区标题上距（非首节）。
pub const SECTION_MT: f32 = sizes::FORM_SECTION_MT;
/// 分区标题下距。
pub const SECTION_MB: f32 = sizes::FORM_SECTION_MB;
/// 首节分区标题上距。
pub const SECTION_FIRST_MT: f32 = sizes::FORM_SECTION_FIRST_MT;
/// 行标签下距。
pub const LABEL_MB: f32 = sizes::FORM_LABEL_MB;
/// 说明文字上距。
pub const DESC_MT: f32 = sizes::FORM_DESC_MT;
/// 行与行之间距。
pub const ITEM_MB: f32 = sizes::FORM_ITEM_MB;
/// 链接行下距。
pub const LINK_MB: f32 = sizes::FORM_LINK_MB;
/// 表单内滑杆行高。
pub const SLIDER_H: f32 = sizes::FORM_SLIDER_H;
/// 表单内按钮最小宽。
pub const BUTTON_MIN_W: f32 = sizes::FORM_BUTTON_MIN_W;
/// 全宽列表行高。
pub const LIST_ROW_H: f32 = sizes::FORM_LIST_ROW_H;
/// 列表行行首图标槽宽（无图标时留空，保证两种行左缘对齐）。
pub const LIST_ICON_SLOT: f32 = sizes::ICON_24;

// ── 文本样式（页面语法三档：页标题 / 分区标题 / 行文本）──────────────────────

/// 页标题 28 Light（行高 36）。
pub fn s_page_title() -> TextStyle {
    TextStyle::new(28.0, 36.0, FontWeight::Light)
}

/// 分区标题 20 Regular（行高 28）。
pub fn s_section() -> TextStyle {
    TextStyle::new(20.0, 28.0, FontWeight::Normal)
}

/// 行文本 / 说明 14 Regular（行高 20）。
pub fn s_label() -> TextStyle {
    TextStyle::new(14.0, 20.0, FontWeight::Normal)
}

// ── 单行标签块 ──────────────────────────────────────────────────────────────

/// 单行标签块（返回文本节点 id，供运行时改写）。`mb` = 其下外边距。
fn text_block(
    tree: &mut Tree,
    parent: WidgetId,
    text: &str,
    style: TextStyle,
    color: ThemeColor,
    mb: f32,
) -> WidgetId {
    tree.insert_with(
        parent,
        Label::new(text).style(style).color(color),
        LayoutProps {
            margin: Insets::new(0.0, 0.0, 0.0, mb),
            ..LayoutProps::default()
        },
    )
}

// ── 链接（强调色可点文字）───────────────────────────────────────────────────

/// 链接点击动作：`target` 是应用自定义的跳转键（如 `"display"`），由应用映射到自己的
/// 路由枚举。用字符串而非泛型，是为让控件层不依赖任何应用的路由类型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormLinkClicked {
    /// 跳转键。
    pub target: Arc<str>,
}

/// 说明块（次级色、按列宽折行）。
///
/// 说明文字常常是一条完整句子（「Dock 在不用时收起；指针贴到屏幕最底一行停留或下推时再
/// 出现。」），单行省略号会把后半句吞掉。按 `COMPOSITION.md` 的强制契约，文本必须显式声明
/// 换行策略，这里的策略是**折行不截断**。
fn desc_block(tree: &mut Tree, parent: WidgetId, text: &str, mt: f32) -> WidgetId {
    tree.insert_with(
        parent,
        Label::new(text)
            .style(s_label())
            .color(ThemeColor::OnSurfaceVariant)
            .wrap(None),
        LayoutProps {
            margin: Insets::new(0.0, mt, 0.0, 0.0),
            ..LayoutProps::default()
        },
    )
}

/// 强调色链接（14）：点击投 [`FormLinkClicked`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetroFormLink {
    /// 链接文字。
    pub text: String,
    /// 跳转键。
    pub target: Arc<str>,
}

impl MetroFormLink {
    /// 建一个链接，`target` 为应用自定义跳转键。
    pub fn new(text: &str, target: &str) -> Self {
        Self { text: text.into(), target: Arc::from(target) }
    }
}

impl Widget for MetroFormLink {
    fn measure(&mut self, ctx: &mut MeasureCtx, available: Size) -> Size {
        let st = s_label();
        let tw = ctx.engine().measure_with_spacing_weighted(
            &self.text,
            st.size,
            st.letter_spacing_em,
            st.weight,
        );
        Size::new(tw.min(available.width.max(0.0)), st.line_height)
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        // 链接是强调色的唯一合法出处：取令牌 `Primary`，换强调色即时生效。
        scene.label(
            self.text.clone(),
            ctx.rect(),
            ThemeColor::Primary.resolve(ctx.theme()),
            s_label(),
            TextAlign::Left,
        );
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            ctx.emit(FormLinkClicked { target: self.target.clone() });
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        true
    }
    fn has_visual_states(&self) -> bool {
        true
    }
    fn focus_visual(&self) -> bool {
        false
    }
}

// ── 页列（CSS 外边距折叠模拟）───────────────────────────────────────────────
//
// CSS 里相邻兄弟的垂直外边距会折叠（取较大者），而元素树 Stack 的 margin 不折叠。
// 故用本结构记录上一块的 CSS 下外边距，为下一块算出「折叠后」的上外边距，逐像素对齐预览。

/// 列表行的行首图标槽：只画一张位图，不参与命中与焦点。
///
/// 不直接用 `widgets::Image`：那条路径依赖图片源在帧内解码，元素树里插入位图后
/// 首帧可能还没就绪；列表图标是**已经光栅好的** `Icon`，直接进 `Scene::image` 最稳。
struct FormIcon {
    icon: Icon,
}

impl Widget for FormIcon {
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        Size::new(
            LIST_ICON_SLOT.min(available.width.max(0.0)),
            sizes::ICON_16.min(available.height.max(0.0)),
        )
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        let r = ctx.rect();
        let icon_rect = kanesumi_core::Rect::new(
            r.origin.x,
            r.origin.y + (r.size.height - sizes::ICON_16) / 2.0,
            sizes::ICON_16,
            sizes::ICON_16,
        );
        scene.image(&self.icon, icon_rect, Some(ThemeColor::OnSurface.resolve(ctx.theme())));
    }
}
/// 页列游标：按 CSS 外边距折叠规则逐个追加语义块。
///
/// 用法：`let mut form = Form::new(content_id);` 然后 `form.section(tree, "显示", true);`
/// 等。游标不持有树，故可与同一页里另建的容器交替使用。
///
/// **本层不自带滚动**：内容高于窗口时由调用方把列放进 `MetroScrollView`（settings 的窗口
/// 外壳就是这么做的）。直接把长页面塞进定高 Stack 会被布局压缩，表现为每行文字被裁掉下半。
pub struct Form {
    parent: WidgetId,
    prev_mb: f32,
}

impl Form {
    /// 在 `parent` 里开始一列。
    pub fn new(parent: WidgetId) -> Self {
        Self { parent, prev_mb: 0.0 }
    }

    /// 在另一个容器里接着排（如可整块隐藏的子开关组）：继承本列上一块的下外边距，
    /// 子容器首块与上一块之间照常按 CSS 折叠留距，不会贴在一起。
    pub fn nested(&self, parent: WidgetId) -> Self {
        Self { parent, prev_mb: self.prev_mb }
    }


    /// 开一个新块，外边距按 CSS 折叠规则落在其上；返回**块容器 id**，往里塞自己的子件。
    ///
    /// 这是本模块唯一的低级原语：上面的语义块（`section` / `toggle` / `note` …）都是它的
    /// 特例，应用要写自定义行时也走它，不必自己算「与上一块差多少」。CSS 相邻兄弟外边距
    /// 折叠取两者较大者，而元素树 Stack 的 margin 不折叠，故本结构只把**折叠后**的间距
    /// 落为块的**上**外边距（下外边距为 0），并记住本块的 CSS 下外边距供下一块折叠。
    ///
    /// `my_mt` = 本块的 CSS 上外边距，`my_mb` = 本块的 CSS 下外边距。
    pub fn block(&mut self, tree: &mut Tree, my_mt: f32, my_mb: f32) -> WidgetId {
        let top = my_mt.max(self.prev_mb);
        self.prev_mb = my_mb;
        tree.insert_with(
            self.parent,
            Stack::column(),
            LayoutProps {
                margin: Insets::new(0.0, top, 0.0, 0.0),
                ..LayoutProps::default()
            },
        )
    }

    /// 页标题（28 Light，下距 [`PAGE_TITLE_MB`]）。返回标签 id。
    pub fn page_title(&mut self, tree: &mut Tree, text: &str) -> WidgetId {
        let block = self.block(tree, 0.0, PAGE_TITLE_MB);
        tree.insert_with(
            block,
            Label::new(text).style(s_page_title()).color(ThemeColor::OnSurface),
            LayoutProps::default(),
        )
    }

    /// 分区标题（20 Regular，上 [`SECTION_MT`] / 下 [`SECTION_MB`]；`first` = 首节，
    /// 上距改 [`SECTION_FIRST_MT`]，因为页标题已给出间隔）。返回标题标签 id（供改写）。
    pub fn section(&mut self, tree: &mut Tree, text: &str, first: bool) -> WidgetId {
        let mt = if first { SECTION_FIRST_MT } else { SECTION_MT };
        let block = self.block(tree, mt, SECTION_MB);
        text_block(tree, block, text, s_section(), ThemeColor::OnSurface, 0.0)
    }

    /// 开关行：标签在上 + 开关 + 可选说明在下。返回开关 id。
    pub fn toggle(
        &mut self,
        tree: &mut Tree,
        label: &str,
        checked: bool,
        desc: Option<&str>,
    ) -> WidgetId {
        let block = self.block(tree, 0.0, ITEM_MB);
        text_block(tree, block, label, s_label(), ThemeColor::OnSurface, LABEL_MB);
        let sw = tree.insert_with(
            block,
            MetroSwitch::new().with_checked(checked).with_state_text("开", "关"),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        if let Some(d) = desc {
            desc_block(tree, block, d, DESC_MT);
        }
        sw
    }

    /// 开关行（说明恒建）：返回 `(开关 id, 说明标签 id)`。与 [`Form::toggle`] 同布局，
    /// 差别是说明节点总是存在，供运行时改写文案或隐藏（如「由硬件开关控制」）。
    pub fn toggle_desc(
        &mut self,
        tree: &mut Tree,
        label: &str,
        checked: bool,
        desc: &str,
    ) -> (WidgetId, WidgetId) {
        let block = self.block(tree, 0.0, ITEM_MB);
        text_block(tree, block, label, s_label(), ThemeColor::OnSurface, LABEL_MB);
        let sw = tree.insert_with(
            block,
            MetroSwitch::new().with_checked(checked).with_state_text("开", "关"),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let desc_id = desc_block(tree, block, desc, DESC_MT);
        (sw, desc_id)
    }

    /// 按钮行：可选说明（在按钮之上）+ 按钮。返回 `(按钮 id, 说明标签 id)`。
    pub fn button(
        &mut self,
        tree: &mut Tree,
        label: &str,
        desc: Option<&str>,
    ) -> (WidgetId, Option<WidgetId>) {
        let block = self.block(tree, 0.0, ITEM_MB);
        let desc_id = desc
            .map(|d| text_block(tree, block, d, s_label(), ThemeColor::OnSurfaceVariant, DESC_MT));
        let button = tree.insert_with(
            block,
            MetroButton::new(label),
            LayoutProps {
                // 页面级最小宽：控件本身按 UWP 无 MinWidth（内容 + 内边距），两字按钮会窄到
                // 44 宽、点不准；设置页的按钮行由页面语法给出 120 的下限。
                min: Size::new(BUTTON_MIN_W, 0.0),
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        (button, desc_id)
    }

    /// 任意控件行：标签在上 + `control` + 可选说明在下。返回控件 id。
    ///
    /// 开关 / 滑杆 / 按钮之外的控件（单选组、下拉、色板）都走这里，避免每种控件再写一遍
    /// 同样的三行结构。
    pub fn labeled(
        &mut self,
        tree: &mut Tree,
        label: &str,
        control: impl Widget,
        desc: Option<&str>,
    ) -> WidgetId {
        let block = self.block(tree, 0.0, ITEM_MB);
        text_block(tree, block, label, s_label(), ThemeColor::OnSurface, LABEL_MB);
        let id = tree.insert_with(
            block,
            control,
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        if let Some(d) = desc {
            desc_block(tree, block, d, DESC_MT);
        }
        id
    }

    /// 只读信息项：标签在上 + 值（主色）+ 可选说明在下。返回值文本 id（运行时改写）。
    pub fn info_item(
        &mut self,
        tree: &mut Tree,
        label: &str,
        value: &str,
        desc: Option<&str>,
    ) -> WidgetId {
        let block = self.block(tree, 0.0, ITEM_MB);
        text_block(tree, block, label, s_label(), ThemeColor::OnSurface, LABEL_MB);
        let vid = text_block(tree, block, value, s_label(), ThemeColor::OnSurface, 0.0);
        if let Some(d) = desc {
            text_block(tree, block, d, s_label(), ThemeColor::OnSurfaceVariant, 0.0);
        }
        vid
    }

    /// 说明块：整宽次级色说明文字（服务不可用 / 状态行等提示文案）。返回文本 id。
    pub fn note(&mut self, tree: &mut Tree, text: &str) -> WidgetId {
        let block = self.block(tree, 0.0, ITEM_MB);
        // 说明文字按列宽折行（不截断）：长说明里常带要照做的命令或路径，省略号会吞掉关键信息。
        tree.insert(
            block,
            Label::new(text)
                .style(s_label())
                .color(ThemeColor::OnSurfaceVariant)
                .wrap(None),
        )
    }

    /// 链接行：强调色链接 + 下距 [`LINK_MB`]。返回链接 id。
    pub fn link(&mut self, tree: &mut Tree, text: &str, target: &str) -> WidgetId {
        let block = self.block(tree, 0.0, LINK_MB);
        tree.insert_with(
            block,
            MetroFormLink::new(text, target),
            LayoutProps {
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        )
    }

    /// 全宽列表行：可选行首图标 + 标题 + 右侧次要信息（同排、次级色）。
    ///
    /// 行高 [`LIST_ROW_H`]、图标档 [`sizes::ICON_16`]，图标占 24 宽的槽（无图标时留空，
    /// 保证有图标与无图标两种行左缘对齐）。返回 `(标题 id, 次要信息 id)`，供运行时改写。
    ///
    /// 这是「列表」与「设置页」共用的那一层：settings 的 Wi-Fi / 蓝牙行、librarian 的文件行、
    /// launcher 的应用行此前各写一遍三件套（图标 + 主文本 + 右侧文本），形状其实一样。
    pub fn list_item(
        &mut self,
        tree: &mut Tree,
        leading: Option<Icon>,
        title: &str,
        secondary: &str,
        mb: f32,
    ) -> (WidgetId, WidgetId) {
        let block = self.block(tree, 0.0, mb);
        let row = tree.insert_with(
            block,
            Stack::row().with_spacing(12.0),
            LayoutProps {
                min: Size::new(0.0, LIST_ROW_H),
                ..LayoutProps::default()
            },
        );
        match leading {
            Some(icon) => {
                tree.insert_with(
                    row,
                    FormIcon { icon },
                    LayoutProps {
                        v_align: Align::Center,
                        ..LayoutProps::default()
                    },
                );
            }
            None => {
                tree.insert_with(
                    row,
                    Stack::row(),
                    LayoutProps {
                        width: Some(LIST_ICON_SLOT),
                        ..LayoutProps::default()
                    },
                );
            }
        }
        let title_id = tree.insert_with(
            row,
            Label::new(title).style(s_label()).color(ThemeColor::OnSurface),
            LayoutProps {
                grow: 1.0,
                v_align: Align::Center,
                ..LayoutProps::default()
            },
        );
        let secondary_id = tree.insert_with(
            row,
            Label::new(secondary).style(s_label()).color(ThemeColor::OnSurfaceVariant),
            LayoutProps {
                v_align: Align::Center,
                ..LayoutProps::default()
            },
        );
        (title_id, secondary_id)
    }

    /// 命令行：一行右对齐的按钮（面板底部「设置 / 锁屏 / 关机」那种），返回各按钮 id。
    ///
    /// 这里**不用** [`Form::button`]：那个有 120 的页面级最小宽，三枚并排会把 360 宽的面板撑爆。
    /// 命令行按内容宽排、间距 8、整体靠右 —— 面板与页面都可以用。
    pub fn actions(&mut self, tree: &mut Tree, labels: &[&str]) -> Vec<WidgetId> {
        let block = self.block(tree, 0.0, ITEM_MB);
        let row = tree.insert_with(
            block,
            Stack::row().with_spacing(8.0),
            LayoutProps {
                h_align: Align::End,
                ..LayoutProps::default()
            },
        );
        labels
            .iter()
            .map(|label| tree.insert(row, MetroButton::new(*label)))
            .collect()
    }
    /// 可点击的列表行：主文本 + 右侧次要文本，**整行可点**，投 [`crate::ButtonClicked`]。
    ///
    /// 与 [`Form::list_item`] 同一套尺寸（行高 [`LIST_ROW_H`]、次要信息右对齐），差别是整行是
    /// 一个按钮：用于「任务列表 / 连接列表」这类点一行就切走的场合。按钮的 `label` 同时是
    /// 无障碍名字，所以主文本不要重复写进 tag。
    ///
    /// 返回 `(按钮 id, 主文本 id, 次文本 id)`。
    pub fn row_button(
        &mut self,
        tree: &mut Tree,
        index: usize,
        label: &str,
        secondary: &str,
        mb: f32,
    ) -> (WidgetId, WidgetId, WidgetId) {
        let block = self.block(tree, 0.0, mb);
        let button = tree.insert_with(
            block,
            MetroRowButton { index, label: label.to_string() },
            LayoutProps {
                min: Size::new(0.0, LIST_ROW_H),
                ..LayoutProps::default()
            },
        );
        // 内容槽模式：按钮画底色与交互态，子节点由框架按内容排版（参 CONTROL_SPEC §1）。
        let row = tree.insert(button, Stack::row().with_spacing(12.0));
        let title = tree.insert_with(
            row,
            Label::new(label).style(s_label()).color(ThemeColor::OnSurface),
            LayoutProps {
                grow: 1.0,
                v_align: Align::Center,
                ..LayoutProps::default()
            },
        );
        let secondary_id = tree.insert_with(
            row,
            Label::new(secondary).style(s_label()).color(ThemeColor::OnSurfaceVariant),
            LayoutProps {
                v_align: Align::Center,
                ..LayoutProps::default()
            },
        );
        (button, title, secondary_id)
    }
    /// 行容器（全宽块，行内子件自排水平；`mb` = 与下一块的 CSS 下外边距）。
    /// 供「标签 + 控件 + 行尾按钮」式列表行使用。
    pub fn row_block(&mut self, tree: &mut Tree, mb: f32) -> WidgetId {
        self.block(tree, 0.0, mb)
    }

    /// 滑杆行：标签在上 + 滑杆 + 值行（次级色）+ 可选说明。返回 `(滑杆 id, 值 id)`。
    #[allow(clippy::too_many_arguments)] // 布局参数（值域 + 步进 + 可选说明），拆结构体反而更绕。
    pub fn slider(
        &mut self,
        tree: &mut Tree,
        label: &str,
        value_text: &str,
        value: f64,
        min: f64,
        max: f64,
        step: Option<f64>,
        desc: Option<&str>,
    ) -> (WidgetId, WidgetId) {
        let block = self.block(tree, 0.0, ITEM_MB);
        text_block(tree, block, label, s_label(), ThemeColor::OnSurface, LABEL_MB);
        let mut slider = MetroSlider::new().with_range(min, max);
        if let Some(s) = step {
            slider = slider.with_step(s);
        }
        slider.set_value(value);
        let sid = tree.insert_with(
            block,
            slider,
            LayoutProps {
                height: Some(SLIDER_H),
                h_align: Align::Start,
                ..LayoutProps::default()
            },
        );
        let vid = text_block(tree, block, value_text, s_label(), ThemeColor::OnSurfaceVariant, 0.0);
        if let Some(d) = desc {
            text_block(tree, block, d, s_label(), ThemeColor::OnSurfaceVariant, 0.0);
        }
        (sid, vid)
    }
}

/// 面板：浮层表面（`MetroSurface::panel`）+ 内边距 + 一列内容，返回**内容列 id**。
///
/// 顶栏的控制中心 / 音频 / 电源 / 通知中心都是这个形状：一个按内容撑开的面板底，
/// 里面一列分区与行。用法与页面一致 —— 拿到列 id 之后照常 `let mut f = Form::new(col);`。
///
/// ```ignore
/// let col = form::panel(tree, tree.root(), theme, form::COL_WIDTH);
/// let mut f = Form::new(col);
/// f.section(tree, "音量", true);
/// f.slider(tree, "扬声器", "60", 0.6, 0.0, 1.0, Some(0.01), None);
/// ```
///
/// 容器不自带滚动、不负责出现位置与展开动画（那些归外壳），参 `docs/CONTROL_SPEC.md` §50。
pub fn panel(tree: &mut Tree, parent: WidgetId, theme: MetroTheme, width: f32) -> WidgetId {
    let surface = tree.insert_with(
        parent,
        MetroSurface::panel(theme),
        LayoutProps {
            h_align: Align::Start,
            ..LayoutProps::default()
        },
    );
    panel_in(tree, surface, width)
}

/// 在**已经存在的面板节点**里铺一列内容，返回内容列 id。
///
/// 配合 `Tree::open_popup` 用：弹层只能由**应用层**打开（`Widget::event` 里拿不到 `Tree`），
/// 所以「打开弹层 → 往里塞页面语法的行」这个流程是：
///
/// ```ignore
/// let panel = tree.open_popup(MetroSurface::panel(theme), spec);
/// let col = form::panel_in(tree, panel, form::COL_WIDTH);
/// let mut f = Form::new(col);
/// f.section(tree, "正在运行", true);
/// f.row_button(tree, "主目录", "Librarian", form::ITEM_MB);
/// ```
pub fn panel_in(tree: &mut Tree, node: WidgetId, width: f32) -> WidgetId {
    tree.insert_with(
        node,
        Stack::column(),
        LayoutProps {
            width: Some(width),
            margin: Insets::new(16.0, 16.0, 16.0, 16.0),
            ..LayoutProps::default()
        },
    )
}
/// 整行可点的列表行：按钮外观 + 行内两列文本，点击投 [`RowButtonClicked`]（带行号）。
///
/// 为什么不用 `MetroButton`：按钮没有载荷，点哪一行都只能收到同一个 `ButtonClicked`，
/// 分不出是哪一行；列表行必须带回行号（或身份键）才能路由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetroRowButton {
    /// 行号（调用方按自己的列表顺序给）。
    pub index: usize,
    /// 无障碍名字（也是按钮的语义标签）。
    pub label: String,
}

/// 列表行被点击：`0` 是 [`Form::row_button`] 传入的行号。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowButtonClicked(pub usize);

impl Widget for MetroRowButton {
    fn measure(&mut self, _ctx: &mut MeasureCtx, available: Size) -> Size {
        Size::new(available.width.max(0.0), LIST_ROW_H)
    }

    fn arrange(&mut self, ctx: &mut kanesumi_element::ArrangeCtx, rect: kanesumi_core::Rect) {
        // 子节点（一列横幅：Stack::row）铺满行矩形，内部再自排。
        for c in ctx.children() {
            ctx.arrange_child(c, rect);
        }
    }

    fn paint(&mut self, ctx: &mut PaintCtx, scene: &mut Scene) {
        MetroButton::new(self.label.as_str())
            .render_chrome(ctx.theme(), ctx.rect(), scene);
    }

    fn event(&mut self, ctx: &mut EventCtx, event: &Event) {
        if let Event::Click = event {
            ctx.emit(RowButtonClicked(self.index));
            ctx.set_handled();
        }
    }

    fn focusable(&self) -> bool {
        true
    }
    fn has_visual_states(&self) -> bool {
        true
    }
}
/// 「相关设置」栏：宽 [`RELATED_WIDTH`]、上内边距 [`RELATED_TOP`]；每组 = 分区标题 + 链接
/// 列表。`width = None` 表示随内容列宽拉伸（窄窗时整块移到内容列下方）。
///
/// 目标键与 [`MetroFormLink`] 同款：应用自定义跳转键，点击投 [`FormLinkClicked`]。
pub fn related(
    tree: &mut Tree,
    parent: WidgetId,
    sections: &[(&str, &[(&str, &str)])],
    width: Option<f32>,
) -> WidgetId {
    let column = tree.insert_with(
        parent,
        Stack::column(),
        LayoutProps {
            width,
            shrink: 1.0,
            margin: Insets::new(0.0, RELATED_TOP, 0.0, 0.0),
            ..LayoutProps::default()
        },
    );
    let mut form = Form::new(column);
    for (i, (title, links)) in sections.iter().enumerate() {
        form.section(tree, title, i == 0);
        for (text, target) in links.iter() {
            form.link(tree, text, target);
        }
    }
    column
}
