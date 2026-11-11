// placement.rs —— 弹层定位的**唯一实现**（元素树 / 控件库 / 命令条 / 工具提示共用一份）。
//
// 为什么要有这一份：旧实现散在四处（控件库 `place_popup` / `place_context_menu` /
// `place_submenu`、命令条 `MetroCommandBarFlyout::place`、元素树 `place_on_side`），
// 规则各不相同 —— 命令条的上翻判据拿**绝对 y** 比高度（副屏 y≠0、或工作区被 TopBar
// 排他区下移时判错），右键菜单遇右缘是「镜像翻左」而不是滑移，元素树又是一套。
// 同一件事四种答案，行为差异没人说得清，也就没人测得全。
//
// 规则按优先级写死（顺序即判据，测试逐条钉住）：
//   1. 尺寸先夹进可用区（`bounds` 各边内缩 `margin`）；
//   2. **首选方向放得下就用首选方向**（缺省「下方优先」）；
//   3. 放不下 → **翻到对侧**；
//   4. 两侧都放不下 → 取空间更大的一侧（面板仍会被夹住，绝不画到区外）；
//   5. 交叉轴按 `align` 对齐锚点，越界则**整体滑移**（`slid = true`）；
//   6. 收尾**夹紧**到可用区。
//
// 成熟做法对照（同一条规则在别处长什么样）：
// - Qt：`QMenuPrivate::popup()` 在 qmenu.cpp:2469-2516 把「右缘越界 → 收拢 / 左侧越界 →
//   推回」「下缘越界 → 翻上（`pos.y() - height`）」「仍超高 → 滚动或贴底」按此顺序写死；
//   可用区取 `screen->availableGeometry()`（qmenu.cpp:292 `QMenuPrivate::popupGeometry`，
//   即扣掉任务栏 / 面板的 workarea）。象限归属按**锚点所在屏幕**定：
//   `QGuiApplication::screenAt(p)`（qmenu.cpp:2309）、`QPlatformWindow::screenForGeometry`
//   用 `geometry().contains(center)`（qplatformwindow.cpp:552-570）。
// - GTK3：`gtk_popover_update_position()`（gtkpopover.c:1068）先算四向 `overshoot`，
//   `overshoot[pos] <= 0` 用首选向、对侧不越界则翻向对侧、都不行则四向取 overshoot 最小；
//   交叉轴落点由 `popover_get_rect()`（gtkwindow.c:6659）用 `CLAMP` 夹进窗口。
// - GTK4：位置交给 `GdkPopupLayout`（flip / slide 约束由 GDK 执行），弹层自己只回读
//   「实际有没有翻」来画箭头方向 —— `did_flip_horizontally` / `did_flip_vertically`
//   （gtkpopover.c:349-380、408-438）。本模块的 `Placement::side` 就是同一个用途。
//
// 坐标口径：`anchor` / `bounds` / 输出矩形必须**同一坐标系**。元素树里是表面局部坐标，
// 多输出时是桌面全局坐标（`place_on_outputs` 按锚点选输出，见下）。

use kanesumi_core::{Rect, Size};

use crate::props::Align;

/// 弹层相对锚点的方位（XAML `FlyoutPlacementMode` 的主方位）。放不下时翻到对侧。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PopupSide {
    /// 锚点下方（默认；下方放不下而上方放得下则上翻）。
    #[default]
    Bottom,
    Top,
    Left,
    Right,
    /// 表面居中（对话框）。锚点不参与定位，只在关闭通知 / 焦点交还里用。
    Center,
}

impl PopupSide {
    /// 对侧方位。`Center` 没有对侧（居中弹层不翻边）。
    pub fn opposite(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Top => Self::Bottom,
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            Self::Center => Self::Center,
        }
    }
}

/// 一个输出：`geometry` = 整块屏幕，`workarea` = 扣掉 TopBar / Dock 等排他区后的可用区。
///
/// 两者分开传的理由：判「锚点在哪个输出」要用整块屏幕（屏幕之间的缝不属于任何可用区），
/// 判「弹层能画到哪」只能用 workarea（弹层不许盖住 TopBar / Dock）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OutputArea {
    pub geometry: Rect,
    pub workarea: Rect,
}

/// 定位输入。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacementRequest {
    /// 锚点矩形。点锚定（右键菜单 / 指针）传零尺寸矩形。
    pub anchor: Rect,
    /// 弹层期望尺寸（放不下时按可用区缩小，不裁剪内容 —— 内容滚动是弹层自己的事）。
    pub size: Size,
    /// 弹层可用的矩形（锚点所在输出的 workarea；单表面场景 = 表面矩形）。
    pub bounds: Rect,
    /// 与可用区边缘的最小留白（逻辑 pt）。0 = 允许贴边。
    pub margin: f32,
    /// 弹层与锚点之间的间隙（逻辑 pt）。
    pub gap: f32,
    /// 首选方向。
    pub side: PopupSide,
    /// 沿锚点边的对齐（`Start` / `Center` / `End`；`Stretch` 视同 `Start`）。
    pub align: Align,
}

impl PlacementRequest {
    /// 最常用的一组缺省：下方优先、左缘对齐、无间隙、无留白。
    pub fn new(anchor: Rect, size: Size, bounds: Rect) -> Self {
        Self {
            anchor,
            size,
            bounds,
            margin: 0.0,
            gap: 0.0,
            side: PopupSide::Bottom,
            align: Align::Start,
        }
    }
}

/// 定位结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// 最终矩形（保证落在可用区内，尺寸已按可用区缩小）。
    pub rect: Rect,
    /// **实际**方向（翻边后；居中弹层恒 `Center`）。
    pub side: PopupSide,
    /// 是否发生滑移：交叉轴为了留在可用区内被整体推开。
    ///
    /// 只描述第 5 步（滑移），不含第 3 步的翻边（翻边看 `side`）与第 6 步的夹紧
    /// （面板大于可用区时的贴边不算滑移）。
    pub slid: bool,
}

/// 定位主函数。规则顺序见本文件头。
pub fn place(req: PlacementRequest) -> Placement {
    let area = avail(req.bounds, req.margin);
    let size = req.size.normalized();
    let w = size.width.clamp(0.0, area.size.width);
    let h = size.height.clamp(0.0, area.size.height);
    if req.side == PopupSide::Center {
        return Placement {
            rect: Rect::new(
                area.origin.x + (area.size.width - w) / 2.0,
                area.origin.y + (area.size.height - h) / 2.0,
                w,
                h,
            ),
            side: PopupSide::Center,
            slid: false,
        };
    }
    let vertical = matches!(req.side, PopupSide::Bottom | PopupSide::Top);
    // ① 首选方向 → ② 翻边 → ③ 取空间更大的一侧。
    let side = choose_side(&req, w, h, area, vertical);
    // 主轴落点（翻边后可能仍越界，第 6 步夹紧）。
    let main = if vertical {
        match side {
            PopupSide::Bottom => req.anchor.bottom() + req.gap,
            _ => req.anchor.origin.y - req.gap - h,
        }
    } else {
        match side {
            PopupSide::Right => req.anchor.right() + req.gap,
            _ => req.anchor.origin.x - req.gap - w,
        }
    };
    // 交叉轴：先按 align 对齐锚点，再滑移进可用区。
    let (mut x, mut y) = if vertical {
        (align_x(req.anchor, w, req.align), main)
    } else {
        (main, align_y(req.anchor, h, req.align))
    };
    let slid = if vertical {
        // 面板与可用区等宽时没有横向自由度，贴左边不算「滑移」（那是第 6 步夹紧）。
        let free = w < area.size.width - 0.01;
        let moved = clamp_axis(x, w, area.origin.x, area.right());
        let changed = free && (moved - x).abs() > 0.01;
        x = moved;
        changed
    } else {
        let free = h < area.size.height - 0.01;
        let moved = clamp_axis(y, h, area.origin.y, area.bottom());
        let changed = free && (moved - y).abs() > 0.01;
        y = moved;
        changed
    };
    // 收尾夹紧（面板大于可用区时贴左上 / 右下边界，绝不为负）。
    x = clamp_axis(x, w, area.origin.x, area.right());
    y = clamp_axis(y, h, area.origin.y, area.bottom());
    Placement {
        rect: Rect::new(x, y, w, h),
        side,
        slid,
    }
}

/// 可用区：`bounds` 内缩 `margin`。留白大于半个矩形时折半 —— 否则可用区退化成一条线，
/// 弹层会被压成 0 尺寸（比「留白没生效」更难查）。
fn avail(bounds: Rect, margin: f32) -> Rect {
    let b = bounds.normalized();
    let m = margin
        .max(0.0)
        .min((b.size.width - 1.0).max(0.0) / 2.0)
        .min((b.size.height - 1.0).max(0.0) / 2.0);
    b.inset(m, m, m, m)
}

/// 主轴方向选择：首选向 → 对侧 → 空间更大的一侧（GTK `overshoot` 最小 / Qt 的偏向）。
fn choose_side(req: &PlacementRequest, w: f32, h: f32, area: Rect, vertical: bool) -> PopupSide {
    let (first, second, room_first, room_second) = if vertical {
        let below = area.bottom() - (req.anchor.bottom() + req.gap);
        let above = req.anchor.origin.y - req.gap - area.origin.y;
        if req.side == PopupSide::Top {
            (PopupSide::Top, PopupSide::Bottom, above, below)
        } else {
            (PopupSide::Bottom, PopupSide::Top, below, above)
        }
    } else {
        let right = area.right() - (req.anchor.right() + req.gap);
        let left = req.anchor.origin.x - req.gap - area.origin.x;
        if req.side == PopupSide::Left {
            (PopupSide::Left, PopupSide::Right, left, right)
        } else {
            (PopupSide::Right, PopupSide::Left, right, left)
        }
    };
    let need = if vertical { h } else { w };
    if room_first >= need {
        first
    } else if room_second >= need {
        second
    } else if room_first >= room_second {
        first
    } else {
        second
    }
}

fn align_x(anchor: Rect, w: f32, align: Align) -> f32 {
    match align {
        Align::Center => anchor.origin.x + (anchor.size.width - w) / 2.0,
        Align::End => anchor.right() - w,
        Align::Start | Align::Stretch => anchor.origin.x,
    }
}

fn align_y(anchor: Rect, h: f32, align: Align) -> f32 {
    match align {
        Align::Center => anchor.origin.y + (anchor.size.height - h) / 2.0,
        Align::End => anchor.bottom() - h,
        Align::Start | Align::Stretch => anchor.origin.y,
    }
}

/// 单轴夹紧：长度大于可用长度时贴左 / 上界，否则夹进 `[lo, hi - len]`。
fn clamp_axis(v: f32, len: f32, lo: f32, hi: f32) -> f32 {
    v.clamp(lo, (hi - len).max(lo))
}

/// 锚点落在哪个输出：以**锚点中心**为准（Qt `QGuiApplication::screenAt(p)` /
/// `QPlatformWindow::screenForGeometry` 的 `geometry().contains(center)`，
/// qplatformwindow.cpp:552-570）；中心不在任何输出内（锚点跨缝）时取与锚点重叠最多的。
///
/// 为什么以中心为准而不是「重叠最多」：弹层挂在锚点旁，锚点在哪块屏上，人就看着哪块屏。
/// 若按重叠面积选，一个贴缝的输入框会把菜单扔到隔壁屏去（Qt 同样先看中心）。
pub fn output_for_anchor(anchor: Rect, outputs: &[OutputArea]) -> Option<OutputArea> {
    let center = anchor.center();
    if let Some(o) = outputs.iter().find(|o| o.geometry.contains(center)) {
        return Some(*o);
    }
    outputs.iter().copied().max_by(|a, b| {
        a_overlap(*a, anchor)
            .partial_cmp(&a_overlap(*b, anchor))
            .unwrap_or(std::cmp::Ordering::Equal)
    })
}

fn a_overlap(o: OutputArea, r: Rect) -> f32 {
    o.geometry
        .intersect(r)
        .map_or(0.0, |i| i.size.width * i.size.height)
}

/// 多输出定位：先按锚点选输出，再用**该输出**的 workarea 定位。
///
/// 「弹层跨到相邻输出」的处理就在这里：哪怕弹层本来会压到隔壁屏，也只在本屏 workarea
/// 内滑移 / 夹紧 —— 跨屏弹层在多显示器上会被用户读成「跑到另一块屏去了」。
/// `outputs` 为空时退回 `req.bounds`。
pub fn place_on_outputs(req: PlacementRequest, outputs: &[OutputArea]) -> Placement {
    let bounds = output_for_anchor(req.anchor, outputs).map_or(req.bounds, |o| o.workarea);
    place(PlacementRequest { bounds, ..req })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::Point;

    fn work() -> Rect {
        Rect::new(0.0, 0.0, 800.0, 600.0)
    }

    fn inside(r: Rect, b: Rect) -> bool {
        r.origin.x >= b.origin.x - 0.01
            && r.origin.y >= b.origin.y - 0.01
            && r.right() <= b.right() + 0.01
            && r.bottom() <= b.bottom() + 0.01
    }

    /// 判据 1：下方放得下 → 下方，且与锚点留出 gap。
    #[test]
    fn prefers_below_when_it_fits() {
        let anchor = Rect::new(100.0, 100.0, 200.0, 32.0);
        let p = place(PlacementRequest {
            gap: 4.0,
            ..PlacementRequest::new(anchor, Size::new(200.0, 160.0), work())
        });
        assert_eq!(p.side, PopupSide::Bottom);
        assert_eq!(p.rect.origin, Point::new(100.0, 136.0));
        assert!(!p.slid);
    }

    /// 判据 2：锚点贴底边 → 上翻（Qt qmenu.cpp:2495 / GTK gtkpopover.c:1130 同款）。
    #[test]
    fn flips_up_at_bottom_edge() {
        let anchor = Rect::new(100.0, 540.0, 200.0, 32.0);
        let p = place(PlacementRequest {
            gap: 4.0,
            ..PlacementRequest::new(anchor, Size::new(200.0, 160.0), work())
        });
        assert_eq!(p.side, PopupSide::Top);
        assert_eq!(p.rect.origin.y, 540.0 - 160.0 - 4.0);
        assert!(!p.slid, "翻边不是滑移");
    }

    /// 判据 3：锚点贴右边 → 左移滑进可用区（不是镜像翻左、也不是画出屏）。
    #[test]
    fn slides_left_at_right_edge() {
        let anchor = Rect::new(760.0, 100.0, 20.0, 32.0);
        let p = place(PlacementRequest::new(anchor, Size::new(200.0, 160.0), work()));
        assert_eq!(p.rect.right(), 800.0);
        assert_eq!(p.rect.origin.y, anchor.bottom(), "主轴仍在下方");
        assert!(p.slid, "越界被推开 = 滑移");
    }

    /// 判据 4：屏幕比弹层小 → 尺寸夹到可用区，位置贴可用区左上 / 不外溢。
    #[test]
    fn clamps_popup_larger_than_screen() {
        let anchor = Rect::new(400.0, 300.0, 10.0, 10.0);
        let area = Rect::new(0.0, 0.0, 320.0, 240.0);
        let p = place(PlacementRequest::new(anchor, Size::new(900.0, 700.0), area));
        assert_eq!(p.rect.size, area.size, "尺寸夹到可用区");
        assert!(inside(p.rect, area), "绝不画出可用区：{:?}", p.rect);
    }

    /// 判据 5：多显示器 —— 锚点在左屏右下角、弹层本会压到右屏，仍以左屏 workarea 为准。
    #[test]
    fn multi_output_uses_anchor_output_workarea() {
        let outputs = [
            OutputArea {
                geometry: Rect::new(0.0, 0.0, 1280.0, 1024.0),
                // 左屏顶上有 32px TopBar（workarea 下移、变矮）。
                workarea: Rect::new(0.0, 32.0, 1280.0, 992.0),
            },
            OutputArea {
                geometry: Rect::new(1280.0, 0.0, 1920.0, 1080.0),
                workarea: Rect::new(1280.0, 0.0, 1920.0, 1080.0),
            },
        ];
        // 锚点贴左屏右缘、下缘（中心仍在左屏）。
        let anchor = Rect::new(1200.0, 1000.0, 60.0, 20.0);
        let p = place_on_outputs(
            PlacementRequest::new(anchor, Size::new(360.0, 280.0), Rect::new(0.0, 0.0, 0.0, 0.0)),
            &outputs,
        );
        assert!(
            inside(p.rect, outputs[0].workarea),
            "跨屏弹层被夹回锚点所在输出：{:?}",
            p.rect
        );
        assert!(p.rect.right() <= 1280.0, "不得压到右屏");
        assert!(p.rect.origin.y >= 32.0, "不得盖住 TopBar 排他区");
        assert_eq!(p.side, PopupSide::Top, "下方放不下 → 上翻");
    }

    /// 判据 5b：锚点中心落在谁身上就是谁 —— 右边的锚点必须用右屏的 workarea。
    #[test]
    fn multi_output_picks_output_by_anchor_center() {
        let outputs = [
            OutputArea {
                geometry: Rect::new(0.0, 0.0, 1280.0, 1024.0),
                workarea: Rect::new(0.0, 32.0, 1280.0, 992.0),
            },
            OutputArea {
                geometry: Rect::new(1280.0, 0.0, 1920.0, 1080.0),
                workarea: Rect::new(1280.0, 0.0, 1920.0, 1080.0),
            },
        ];
        let anchor = Rect::new(1300.0, 40.0, 100.0, 24.0);
        let p = place_on_outputs(
            PlacementRequest::new(anchor, Size::new(300.0, 200.0), work()),
            &outputs,
        );
        assert!(p.rect.origin.x >= 1280.0, "留在右屏：{:?}", p.rect);
        assert_eq!(p.rect.origin.y, 64.0, "右屏无 TopBar，不额外下移");
        assert!(inside(p.rect, outputs[1].workarea));
    }

    /// 判据 6：边距为零允许贴边；边距非零时四边都让开。
    #[test]
    fn margin_zero_touches_edge_and_margin_insets() {
        let anchor = Rect::new(0.0, 0.0, 800.0, 20.0);
        let zero = place(PlacementRequest::new(
            anchor,
            Size::new(200.0, 200.0),
            work(),
        ));
        assert_eq!(zero.rect.origin.x, 0.0, "margin=0 允许贴左");

        let inset = place(PlacementRequest {
            margin: 8.0,
            ..PlacementRequest::new(anchor, Size::new(200.0, 200.0), work())
        });
        assert_eq!(inset.rect.origin.x, 8.0, "留白 8 生效");
        assert!(inside(inset.rect, work().inset(8.0, 8.0, 8.0, 8.0)));
    }

    /// 判据 6b：留白大到把可用区吃光时不把弹层压成 0 尺寸（折半兜底）。
    #[test]
    fn huge_margin_does_not_collapse_popup() {
        let p = place(PlacementRequest {
            margin: 500.0,
            ..PlacementRequest::new(
                Rect::new(0.0, 0.0, 10.0, 10.0),
                Size::new(100.0, 80.0),
                Rect::new(0.0, 0.0, 100.0, 80.0),
            )
        });
        assert!(p.rect.size.width > 0.0 && p.rect.size.height > 0.0);
    }

    /// 判据 7：弹层比锚点宽 —— 左缘对齐时右侧滑移；居中时两侧对称但夹紧。
    #[test]
    fn wider_than_anchor_slides_or_clamps() {
        let anchor = Rect::new(40.0, 100.0, 60.0, 20.0);
        let wide = Size::new(400.0, 120.0);
        let start = place(PlacementRequest::new(anchor, wide, work()));
        assert_eq!(start.rect.origin.x, 40.0, "还放得下 → 不动");

        let near_right = Rect::new(600.0, 100.0, 60.0, 20.0);
        let slid = place(PlacementRequest::new(near_right, wide, work()));
        assert!(slid.slid);
        assert_eq!(slid.rect.right(), work().right());

        let middle = Rect::new(300.0, 100.0, 60.0, 20.0);
        let centered = place(PlacementRequest {
            align: Align::Center,
            ..PlacementRequest::new(middle, wide, work())
        });
        assert_eq!(
            centered.rect.origin.x,
            middle.center().x - wide.width / 2.0,
            "居中于锚点"
        );
    }

    /// 判据 8：连续两次弹出位置不抖动（纯函数 + 锚点亚像素微动不翻转）。
    #[test]
    fn repeated_placement_is_stable() {
        let anchor = Rect::new(300.0, 520.0, 120.0, 32.0);
        let size = Size::new(240.0, 80.0);
        let a = place(PlacementRequest::new(anchor, size, work()));
        let b = place(PlacementRequest::new(anchor, size, work()));
        assert_eq!(a, b, "同一输入两次结果必须一致");

        // 指针抖动 0.4pt 不得在「刚好放得下」的边界上翻来覆去。
        let nudged = Rect::new(300.0, anchor.origin.y + 0.4, 120.0, 32.0);
        let c = place(PlacementRequest::new(nudged, size, work()));
        assert_eq!(c.side, a.side, "亚像素抖动不得改方向");
    }

    /// 判据 9：左右首选方向（子菜单）同样走「首选 → 翻边 → 夹紧」。
    #[test]
    fn horizontal_sides_flip_and_clamp() {
        let parent = Rect::new(600.0, 200.0, 160.0, 32.0);
        let req = PlacementRequest {
            side: PopupSide::Right,
            gap: 2.0,
            ..PlacementRequest::new(parent, Size::new(300.0, 160.0), work())
        };
        let p = place(req);
        assert_eq!(p.side, PopupSide::Left, "右侧放不下 → 翻左");
        assert_eq!(p.rect.right(), 600.0 - 2.0);
        assert!(inside(p.rect, work()));
    }

    /// 判据 10：RTL —— **未支持**，此处只钉住现状（`Start` 恒等于左缘 / 上缘）。
    ///
    /// TODO(rtl): Qt 在 RTL 下把菜单右缘对齐指针并反向镜像（qmenu.cpp:2470-2490
    /// `q->isRightToLeft()` 分支），本模块尚无 `direction` 入参，故 RTL 环境下的菜单
    /// 仍按 LTR 放置。支持时给 `PlacementRequest` 加 `rtl: bool`，`align_x` 里把
    /// `Start`/`End` 对调 —— 调用点（控件库 / 元素树）目前都拿不到书写方向，故不在本次做。
    #[test]
    fn rtl_not_supported_yet() {
        let anchor = Rect::new(600.0, 100.0, 160.0, 20.0);
        let p = place(PlacementRequest::new(anchor, Size::new(200.0, 120.0), work()));
        assert_eq!(p.rect.origin.x, 600.0, "Start 仍是左缘（LTR 语义）");
    }

    /// 判据 11：居中弹层（对话框）不吃锚点，恒居中且不越界。
    #[test]
    fn center_ignores_anchor() {
        let p = place(PlacementRequest {
            side: PopupSide::Center,
            ..PlacementRequest::new(
                Rect::new(700.0, 500.0, 10.0, 10.0),
                Size::new(200.0, 100.0),
                work(),
            )
        });
        assert_eq!(p.side, PopupSide::Center);
        assert_eq!(p.rect.origin, Point::new(300.0, 250.0));
        assert!(!p.slid);
    }

    /// 判据 12：两侧都放不下 → 取空间更大的一侧（GTK 四向 overshoot 最小的特例）。
    #[test]
    fn neither_side_fits_picks_the_roomier() {
        // 工作区只有 120 高：高 100 的弹层上方 60、下方 40，两侧都不够 → 取上方。
        let anchor = Rect::new(100.0, 60.0, 100.0, 20.0);
        let short = Rect::new(0.0, 0.0, 800.0, 120.0);
        let p = place(PlacementRequest::new(anchor, Size::new(200.0, 100.0), short));
        assert_eq!(p.side, PopupSide::Top);
        assert_eq!(p.rect.origin.y, 0.0, "翻上后夹到可用区顶");
    }
}
