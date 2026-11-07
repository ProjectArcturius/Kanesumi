//! sizes.rs —— Ether / Kanesumi **唯一尺寸表**（盘点 N18；裁定 `DECISIONS_2026-11-03.md`
//! §253 C2 第一步：把现值原样搬入，像素零变化）。
//!
//! 规则：
//! - 每个令牌的注释必须写值来源（`文件:行` 或裁定号）；无来源的值标「临时值」并登记
//!   `docs/CANON_VS_TEMPORARY.md`，本表现状无临时值。
//! - 本表只收**几何尺寸**：壳层高度、Dock 族、列表行高、图标尺寸档、间距档、圆角枚举。
//!   颜色走 `ThemeColor`，字号走 `typography`，交互阈值（拖动 / 滚轮步距 / 闪烁）走
//!   `interaction`，动画时长走各动画规格 —— 均不入本表。
//! - 各控件 / 应用保留自己的**语义名**，取值一律引用本表（如 `CHECKBOX_SIZE = ICON_20`）；
//!   禁止再新写裸尺寸常量（`kanesumi-controls/tests/size_discipline.rs` 静态守住）。
//! - 第二步整体 ×1.2（§159）另书执行：只有本表允许被整体缩放，前提就是生产段只引用这里。
//!   **本文件内所有值在迁移后必须与原硬编码逐字节相等**（回归 `moved_values_are_verbatim`）。

/// 顶栏逻辑高。来源：`settings/src/topbar/mod.rs:87`（`pub const BAR_H: f32 = 30.0`，
/// 盘点 `docs/research/refactor_census/REPORT_2026-11-03.md` N18 点名的第一处）。
pub const TOPBAR_HEIGHT: f32 = 30.0;

/// 顶栏 layer-shell 排他区。与顶栏高是**同一个 30 的两份硬编码**（盘点 N18：
/// `compositor/src/state/layer_shell.rs:55` `set_external_top_exclusion(30)`），
/// 故随顶栏走，不独立取值；合成器侧迁移排在 TopBar 迁移卡（settings/src/topbar
/// 在 K3 / `rf1-i18n-topbar` 合入之后）。
pub const TOPBAR_EXCLUSIVE_ZONE: f32 = TOPBAR_HEIGHT;

/// Dock 图标 48。来源：§158「悬浮，图标48，模糊30」；§159 明确 48 是 **200% 输出下的
/// 最终值，不随 Ether 尺寸系数二次放大**。现值 `launcher/src/shell/dock_rail.rs:31`。
pub const DOCK_ICON: f32 = 48.0;

/// Dock 槽宽 60（图标居中、槽间无缝）。来源：§158「间距 12（槽 60）」。
/// 现值 `launcher/src/shell/dock_rail.rs:32`。
pub const DOCK_SLOT: f32 = 60.0;

/// Dock 板高 68 = 图标 48 + 板内上下各 10。来源：§158「板内上下 10（板高 68）」。
/// 现值 `launcher/src/shell/dock_rail.rs:34`。
pub const DOCK_PLATE_HEIGHT: f32 = 68.0;

/// Dock 底板左右内边距 12。来源：§158「左右 12」。现值 `launcher/src/shell/dock_rail.rs:41`。
pub const DOCK_PLATE_PAD: f32 = 12.0;

/// Dock 悬浮间隙 8（板底距屏底；命中仍延伸到屏幕底边）。来源：§158「距屏底 8」。
/// 现值 `launcher/src/shell/dock_rail.rs:36`。
pub const DOCK_FLOAT_GAP: f32 = 8.0;

/// Dock 主表面高 = 板 68 + 悬浮 8 = 76。现值 `launcher/src/shell/dock_rail.rs:38`
///（复合常量）；与合成器 `compositor/src/shell/dock_visibility.rs:20` 的
/// `DOCK_HEIGHT: f64 = 76.0` 同值（合成器侧迁移随 TopBar 之后的批次）。
pub const DOCK_HEIGHT: f32 = DOCK_PLATE_HEIGHT + DOCK_FLOAT_GAP;

/// Dock 定位线高 3。来源：§158「定位线 3」。现值 `launcher/src/shell/dock_rail.rs:39`。
pub const DOCK_LINE: f32 = 3.0;

/// Dock 亚克力底板壁纸模糊半径 30。来源：§158「模糊30」（另参 §156）。
/// 现值 `launcher/src/shell/dock_rail.rs:43`。与 `TOPBAR_HEIGHT` 的 30 **同值不同义**，
/// 独立令牌，不得互相替代。
pub const DOCK_ACRYLIC_BLUR: f32 = 30.0;

/// 列表行高下限 40。来源：UWP `ListViewItem` MinHeight（一手源，参
/// `docs/UWP_PRIMARY_SOURCES.md`）。现值 `kanesumi-controls/src/list.rs:63` 的
/// `.max(40.0)`。注意 Launcher「所有应用」行高 56 是该页自定的行高（`all_apps.rs:19`），
/// 不走本令牌 —— 不把两处不同语义的值强行归一。
pub const LIST_ROW_MIN_HEIGHT: f32 = 40.0;

/// 图标尺寸档 —— 现用档位 12 / 16 / 20 / 24 / 32 / 48（盘点 N18「图标尺寸档」，搬现值）。
/// 各档来源见各自注释；档位取值不随用途改，新图标只准落在档位上。
///
/// 12 —— 面板内 chevron 小字形。现值 `kanesumi-controls/src/dropdown_menu.rs:490` 等
/// 7+ 处 `12×12`。
pub const ICON_12: f32 = 12.0;

/// 16 —— 常规图标 / 图标槽。现值 `kanesumi-controls/src/animated_icon.rs:26`、
/// `text_box.rs:547`、`title_bar.rs:181` 等；锁屏电源 / 删除图标
/// `apps/lock/src/app.rs:426/508`。
pub const ICON_16: f32 = 16.0;

/// 20 —— 复选框方框 / 单选圆圈类 20px 图形。现值 `kanesumi-controls/src/check_box.rs:20`
/// `CHECKBOX_SIZE`、`radio_buttons.rs:14` `RADIO_CIRCLE`。
pub const ICON_20: f32 = 20.0;

/// 24 —— 评分星等 24px 图形。现值 `kanesumi-controls/src/rating_control.rs:14`
/// `RATING_ITEM_SIZE`。
pub const ICON_24: f32 = 24.0;

/// 32 —— 次级图标。现值：Dock 入口格（§187，`launcher/src/shell/dock_rail.rs:79`）、
/// 应用列表行图标（`launcher/src/shell/all_apps.rs:21`）、Expander chevron
/// （`kanesumi-controls/src/expander.rs:30`）。
pub const ICON_32: f32 = 32.0;

/// 48 —— 主图标档，与 Dock 图标同值（§158）。独立名列出以保持档位表完整。
pub const ICON_48: f32 = DOCK_ICON;

// ── 页面语法族（form）────────────────────────────────────────────────────────
//
// 来源统一为 `settings/src/window/grammar.rs`（这套版式的原始实现：36 个页面逐项照抄
// 预览页 CSS 得到的现值）。本族把「分区标题 / 行标签 / 控件 / 说明」四层间距与内容列宽
// 收进尺寸表，页面语法层（`kanesumi-controls::form`）只引用本族，不再自带常量。
// 参 `kanesumi-controls/src/form.rs`、`docs/CONTROL_SPEC.md` §49。

/// 内容列设计宽 600。来源：`settings/src/window/grammar.rs:63`（`COL_W`）。
/// 是**设计宽**不是硬宽：窄窗时由外层按可用宽收窄。
pub const FORM_COL_WIDTH: f32 = 600.0;

/// 「相关设置」栏宽 240。来源：`settings/src/window/grammar.rs:64`（`RELATED_W`）。
pub const FORM_RELATED_WIDTH: f32 = 240.0;

/// 内容列与相关设置栏的间隔 48。来源：`settings/src/window/grammar.rs:65`（`CONTENT_GAP`）。
pub const FORM_CONTENT_GAP: f32 = 48.0;

/// 相关设置栏上内边距 56。来源：`settings/src/window/grammar.rs:66`（`RELATED_PT`）。
pub const FORM_RELATED_TOP: f32 = 56.0;

/// 页标题下距 20。来源：`settings/src/window/grammar.rs:68`（`PTITLE_MB`）。
pub const FORM_PAGE_TITLE_MB: f32 = 20.0;

/// 分区标题上距 36。来源：`settings/src/window/grammar.rs:69`（`SEC_MT`）。
pub const FORM_SECTION_MT: f32 = 36.0;

/// 分区标题下距 12。来源：`settings/src/window/grammar.rs:70`（`SEC_MB`）。
pub const FORM_SECTION_MB: f32 = 12.0;

/// 首节分区标题上距 4（页标题已给出的间隔不再叠加）。来源：
/// `settings/src/window/grammar.rs:71`（`SEC_FIRST_MT`）。
pub const FORM_SECTION_FIRST_MT: f32 = 4.0;

/// 行标签下距 6。来源：`settings/src/window/grammar.rs:73`（`LABEL_MB`）。
pub const FORM_LABEL_MB: f32 = 6.0;

/// 说明文字上距 6。来源：`settings/src/window/grammar.rs:74`（`DESC_MT`）。
pub const FORM_DESC_MT: f32 = 6.0;

/// 行与行之间距 20。来源：`settings/src/window/grammar.rs:75`（`ITEM_MB`）。
pub const FORM_ITEM_MB: f32 = 20.0;

/// 链接行下距 10。来源：`settings/src/window/grammar.rs:90`（`LINK_MB`）。
pub const FORM_LINK_MB: f32 = 10.0;

/// 全宽列表行高 48（标题 14、左侧字形 16、右侧次要信息同行）。
/// 来源：`settings/src/window/grammar.rs:94`（`LIST_ROW_H`）。
/// 与 [`LIST_ROW_MIN_HEIGHT`]（UWP `ListViewItem` 下限 40）**同族不同义**，不互相替代。
pub const FORM_LIST_ROW_H: f32 = 48.0;

/// 表单内滑杆行高 32。来源：`settings/src/window/grammar.rs:800`（`height: Some(32.0)`）。
pub const FORM_SLIDER_H: f32 = 32.0;

// ── 对话框族（dialog）────────────────────────────────────────────────────────
//
// 来源 `CONTROL_SPEC.md` §9（UWP ContentDialog）。此前散在 `kanesumi-controls/src/dialog.rs`
// 与 `dialog_button.rs` 的裸常量里，`size_discipline` 白名单只减不增，故收进本表。

/// 标题与内容之间距 12。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_TITLE_GAP: f32 = 12.0;

/// 内容与命令区之间距 8。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_CONTENT_GAP: f32 = 8.0;

/// 命令区行高（= 命令按钮高）32。来源：`CONTROL_SPEC.md` §9（按钮高 32）。
pub const DIALOG_BUTTON_HEIGHT: f32 = 32.0;

/// 对话框最小高 184。来源：`CONTROL_SPEC.md` §9（Min 184）。
pub const DIALOG_MIN_HEIGHT: f32 = 184.0;

/// 对话框最大高 756。来源：`CONTROL_SPEC.md` §9（Max 756）。
pub const DIALOG_MAX_HEIGHT: f32 = 756.0;

/// 命令区内相邻按钮的水平间距 2。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_BUTTON_GAP: f32 = 2.0;

/// 命令区按钮宽 130。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_BUTTON_WIDTH: f32 = 130.0;

/// 对话框盒体左右内边距 24。来源：`CONTROL_SPEC.md` §9（Padding `24,18,24,24`）。
pub const DIALOG_PAD_X: f32 = 24.0;

/// 对话框盒体上内边距 18。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_PAD_TOP: f32 = 18.0;

/// 对话框盒体下内边距 24。来源：`CONTROL_SPEC.md` §9。
pub const DIALOG_PAD_BOTTOM: f32 = 24.0;

// ── 按钮族（button）──────────────────────────────────────────────────────────

/// 按钮上下内边距（UWP `Button` Padding `8,5,8,6` 的纵向两档）。
/// 来源：`CONTROL_SPEC.md` §1；现值在 `kanesumi-controls/src/button.rs`。
/// 上下不对称是规格值，不是笔误 —— 文字基线偏上，视觉才居中。
pub const BUTTON_PAD_TOP: f32 = 5.0;
/// 见 [`BUTTON_PAD_TOP`]。
pub const BUTTON_PAD_BOTTOM: f32 = 6.0;

// ── 菜单族（menu）───────────────────────────────────────────────────────────

/// 菜单勾选列宽 16：有可勾选项时左缘固定留出这一列，勾选与否不改变文字起点。
/// 来源：`CONTROL_SPEC.md` §39「视觉」。
pub const MENU_CHECK_GUTTER: f32 = 16.0;

/// 菜单面板最大高 504。来源：同族 ComboBox `MaxDropDownHeight`（`CONTROL_SPEC.md` §8），
/// 属**推断值**、登记在 `docs/CANON_VS_TEMPORARY.md`，待真机对照。
pub const MENU_MAX_HEIGHT: f32 = 504.0;

/// 菜单滚动条宽 3。来源：`kanesumi-controls/src/dropdown_menu.rs` 滚动提示条现值。
pub const MENU_SCROLLBAR_WIDTH: f32 = 3.0;

/// 菜单滚动条上下内缩 4。来源：同上。
pub const MENU_SCROLLBAR_INSET: f32 = 4.0;

/// 间距档：4px 基数网格（s4~s32），UWP 布局惯例。真源即 [`crate::tokens::Spacing`]，
/// 此处转出使「尺寸表」单一路径完整；取值改动须经 `tokens.rs`。
pub use crate::tokens::Spacing;

/// 圆角枚举：`Square`（默认，正典 §Ⅲ.1 直角）/ `Slight`（个案 opt-in）/
/// `Capsule`（仅结构性胶囊）。真源即 [`crate::geometry::CornerRadius`]。
pub use crate::geometry::CornerRadius;

#[cfg(test)]
mod tests {
    use super::*;

    /// 迁移契约：本表值必须与被搬离的原硬编码逐字节相等（§253 C2「像素零变化」）。
    /// 改任何一个值都会先红在这里 —— 值变更必须走独立评审，不许顺手改。
    #[test]
    fn moved_values_are_verbatim() {
        assert_eq!(TOPBAR_HEIGHT, 30.0);
        assert_eq!(TOPBAR_EXCLUSIVE_ZONE, 30.0);
        assert_eq!(DOCK_ICON, 48.0);
        assert_eq!(DOCK_SLOT, 60.0);
        assert_eq!(DOCK_PLATE_HEIGHT, 68.0);
        assert_eq!(DOCK_PLATE_PAD, 12.0);
        assert_eq!(DOCK_FLOAT_GAP, 8.0);
        assert_eq!(DOCK_HEIGHT, 76.0);
        assert_eq!(DOCK_LINE, 3.0);
        assert_eq!(DOCK_ACRYLIC_BLUR, 30.0);
        assert_eq!(LIST_ROW_MIN_HEIGHT, 40.0);
        assert_eq!(ICON_12, 12.0);
        assert_eq!(ICON_16, 16.0);
        assert_eq!(ICON_20, 20.0);
        assert_eq!(ICON_24, 24.0);
        assert_eq!(ICON_32, 32.0);
        assert_eq!(ICON_48, 48.0);
        assert_eq!(FORM_COL_WIDTH, 600.0);
        assert_eq!(FORM_RELATED_WIDTH, 240.0);
        assert_eq!(FORM_CONTENT_GAP, 48.0);
        assert_eq!(FORM_RELATED_TOP, 56.0);
        assert_eq!(FORM_PAGE_TITLE_MB, 20.0);
        assert_eq!(FORM_SECTION_MT, 36.0);
        assert_eq!(FORM_SECTION_MB, 12.0);
        assert_eq!(FORM_SECTION_FIRST_MT, 4.0);
        assert_eq!(FORM_LABEL_MB, 6.0);
        assert_eq!(FORM_DESC_MT, 6.0);
        assert_eq!(FORM_ITEM_MB, 20.0);
        assert_eq!(FORM_LINK_MB, 10.0);
        assert_eq!(FORM_LIST_ROW_H, 48.0);
        assert_eq!(FORM_SLIDER_H, 32.0);
        let s = Spacing::base4();
        assert_eq!((s.s4, s.s8, s.s12, s.s16, s.s24, s.s32), (4.0, 8.0, 12.0, 16.0, 24.0, 32.0));
    }
}
