//! metrics.rs —— 静态尺度令牌：间距 / 字级 / 最小命中 / 控件内边距。
//!
//! 存在理由：在它之前，「间距 / 字号 / 行高 / 最小命中」这层口径没有单一出处 ——
//! settings 的页面语法自带 50 条布局常量、全仓散着 160+ 处 `Rect::new`，顶栏单行文字
//! 靠手调的墨迹修正常量。本模块把这层口径收成一组 `const`，供页面与控件引用。
//!
//! 边界（两条，写死在这里）：
//! - **只有 `const`**：不做运行时主题覆盖。深浅色、强调色、运行时缩放都不属于本模块，
//!   它们是 `colors` / `accent` / 外壳的职责。把间距做成可覆盖令牌，等于把静态口径变成
//!   运行时状态，同一页面在两台机器上会长得不一样 —— 本模块明确不提供这个口子。
//! - **数值照抄既有实现**：字级与 `typography::MetroTypography::metro()` 逐项相等（回归
//!   `type_levels_match_typography`），控件口径照抄 `docs/CONTROL_SPEC.md`。若与 SPEC
//!   冲突，以 SPEC 为准并同步改这里。
//!
//! 采纳口径（非临时值）：本表是 Kanesumi 的既定尺度，登记见
//! `docs/CANON_VS_TEMPORARY.md` §〇「采纳口径」；不要把它当「机制没做所以先写死」。

// ── 间距阶梯 ────────────────────────────────────────────────────────────────
//
// 4 的倍数取自 UWP 布局惯例（4 px 网格）；2 是唯一的亚 4 档 —— 控件之间的细缝
// （顶栏悬停底左右各 2，参 settings/src/topbar/align.rs `BAR_ITEM_INSET_X`）。
// 页面不得再造 3 / 5 / 6 / 10 这类中间值：需要更密就用 2，需要更疏就跳下一档。

/// 细缝 2。仅用于「两个控件必须分开、但 4 会读成两个组」的场合。
pub const SPACE_2: f32 = 2.0;
/// 最小常规间距 4（图标与文字内侧、紧邻控件）。
pub const SPACE_4: f32 = 4.0;
/// 8：行内元素之间。
pub const SPACE_8: f32 = 8.0;
/// 12：同一项内标签与控件之间。
pub const SPACE_12: f32 = 12.0;
/// 16：项内边距 / 图标槽。
pub const SPACE_16: f32 = 16.0;
/// 24：页面内容区左内边距。
pub const SPACE_24: f32 = 24.0;
/// 32：大块之间（页标题与首节、卡片内边距）。
pub const SPACE_32: f32 = 32.0;

/// 完整阶梯（升序）。新增间距前先问「能不能落到这七档之一」。
pub const SPACING_LADDER: [f32; 7] = [
    SPACE_2, SPACE_4, SPACE_8, SPACE_12, SPACE_16, SPACE_24, SPACE_32,
];

// ── 字级（五档，含行盒高度）─────────────────────────────────────────────────
//
// 数值与 `MetroTypography::metro()` 的对应字段逐项相等；本表只是给「不持有主题」的
// 调用方一个静态入口（画布量测、页面语法、测试夹具），不另立第二套字号。

/// 一档字级：字号 + 行盒高（逻辑 pt）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TypeLevel {
    pub size: f32,
    pub line_height: f32,
}

/// caption：说明 / 次要信息（= typography.caption）。
pub const CAPTION: TypeLevel = TypeLevel {
    size: 13.0,
    line_height: 18.0,
};
/// body：正文 / 列表 / 菜单（= typography.body）。
pub const BODY: TypeLevel = TypeLevel {
    size: 15.0,
    line_height: 22.0,
};
/// subtitle：分区标题（= typography.subtitle）。
pub const SUBTITLE: TypeLevel = TypeLevel {
    size: 20.0,
    line_height: 28.0,
};
/// title：页标题 / 窗口标题（= typography.title）。
pub const TITLE: TypeLevel = TypeLevel {
    size: 22.0,
    line_height: 28.0,
};
/// display：大标题（= typography.header）。
pub const DISPLAY: TypeLevel = TypeLevel {
    size: 46.0,
    line_height: 56.0,
};

// ── 最小命中 ────────────────────────────────────────────────────────────────
//
// 视觉可以更小，命中区不可以（参 docs/INTERACTION_CANON.md §Ⅱ「最小命中区」）。
// 指针 32 = 正典值；触控 44 是触控目标档（44×44 平台口径）；正典里 Win 触控
// 建议 40 仍标「待核」，故 44 先按触控档使用，SPEC 若定 40 以 SPEC 为准）。

/// 指针最小命中边长（正典 32×32）。
pub const MIN_HIT_POINTER: f32 = 32.0;
/// 触控最小命中边长（44×44）。
pub const MIN_HIT_TOUCH: f32 = 44.0;

// ── 控件内边距与行高（照抄 Win10 实测 / CONTROL_SPEC）──────────────────────
//
// 这些是控件模板里的固定口径，不是页面可以随手改的间距。页面间距走上方的阶梯。

/// 控件默认字号 14（`ControlContentThemeFontSize`，CONTROL_SPEC §1 Button）。
pub const CONTROL_TEXT_SIZE: f32 = 14.0;
/// 控件文本行盒 20（CONTROL_SPEC §18 BreadcrumbBar 项 `LineHeight 20`，14 号字）。
pub const CONTROL_LINE_HEIGHT: f32 = 20.0;
/// 控件水平内边距 8（CONTROL_SPEC §1 Button `Padding 8,5,8,6`）。
pub const CONTROL_PAD_X: f32 = 8.0;
/// 控件上内边距 5（同上）。
pub const CONTROL_PAD_TOP: f32 = 5.0;
/// 控件下内边距 6（同上；RS3+ 口径，RS1 为 5）。
pub const CONTROL_PAD_BOTTOM: f32 = 6.0;
/// 单行控件最小高 32（CONTROL_SPEC §8 ComboBox 触发器 / MenuFlyout Presenter MinHeight 32）。
pub const CONTROL_MIN_H: f32 = 32.0;
/// 列表行最小高 40（UWP `ListViewItem`，CONTROL_SPEC §7；与 `sizes::LIST_ROW_MIN_HEIGHT` 同值）。
pub const LIST_ROW_MIN_H: f32 = 40.0;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typography::MetroTypography;

    #[test]
    fn spacing_ladder_is_ordered() {
        assert_eq!(SPACING_LADDER, [2.0, 4.0, 8.0, 12.0, 16.0, 24.0, 32.0]);
        assert!(SPACING_LADDER.windows(2).all(|w| w[0] < w[1]));
    }

    /// 本表不是第二套字号：五档必须与排版令牌逐项相等，改一处就得同步改另一处。
    #[test]
    fn type_levels_match_typography() {
        let t = MetroTypography::metro();
        for (level, style) in [
            (CAPTION, t.caption),
            (BODY, t.body),
            (SUBTITLE, t.subtitle),
            (TITLE, t.title),
            (DISPLAY, t.header),
        ] {
            assert_eq!(level.size, style.size);
            assert_eq!(level.line_height, style.line_height);
        }
    }

    #[test]
    fn hit_targets_follow_pointer_and_touch_profile() {
        assert_eq!(MIN_HIT_POINTER, 32.0);
        assert_eq!(MIN_HIT_TOUCH, 44.0);
        assert!(MIN_HIT_TOUCH > MIN_HIT_POINTER, "触控目标不得小于指针目标");
    }

    /// 控件口径与 CONTROL_SPEC §1 的 `Padding 8,5,8,6` 对齐（改值先改 SPEC）。
    #[test]
    fn control_padding_matches_spec() {
        assert_eq!(
            (CONTROL_PAD_X, CONTROL_PAD_TOP, CONTROL_PAD_BOTTOM),
            (8.0, 5.0, 6.0)
        );
        assert_eq!(CONTROL_TEXT_SIZE, 14.0);
        assert_eq!(CONTROL_LINE_HEIGHT, 20.0);
    }
}
