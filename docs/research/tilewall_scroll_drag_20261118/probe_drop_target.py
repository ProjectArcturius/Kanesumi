#!/usr/bin/env python3
"""复算 launcher `tiles::drop_target` 在 C0 基准布局上的九行读数（2026-11-18）。

只用于交件里核对「表读数」的算术，不是门的一部分；落点语义以
`kanesumi-controls/src/navigation/tile_wall_control.rs::drop_target_in` 与 Ether
`launcher/src/shell/tiles.rs:202` 的实现为准。几何照 C0：两组 18+2 枚 Standard
（墙尺寸 Large 2×2）、墙 0,0 1280×752、提供方度量 cell=70 gap=10 group_gap=70
title_band=72 margin_x=112.4451，拖动前先摘除 app01（组 0 第 0 枚）。

行距 = cell + gap = 80；组宽 = 12×70 + 11×10 = 950；组块高 = 72 + 6×70 + 5×10 = 542。
"""

CELL, GAP, GROUP_GAP = 70.0, 10.0, 70.0
TITLE_BAND, MARGIN_X = 72.0, 112.4451
VIEW_W, VIEW_H = 1280.0, 752.0
BOTTOM_BAND, MIN_ROWS = 48.0, 6
GROUP_W = 12 * CELL + 11 * GAP


def tile_rects(count, gx, grid_y):
    """Large 2×2：6 个宏单元列，行优先。"""
    out = []
    for i in range(count):
        row, col = divmod(i, 6)
        out.append(
            (gx + col * 2 * (CELL + GAP), grid_y + row * 2 * (CELL + GAP),
             2 * CELL + GAP, 2 * CELL + GAP)
        )
    return out


def contains(r, x, y):
    rx, ry, rw, rh = r
    return rx <= x < rx + rw and ry <= y < ry + rh


def drop_target(x, y, groups):
    """launcher tiles.rs::drop_target 的逐行复刻。"""
    for g, (gx, rects) in enumerate(groups):
        if gx <= x < gx + GROUP_W:
            for t, r in enumerate(rects):
                if contains(r, x, y):
                    return ("InGroup", g, t + 1 if x >= r[0] + r[2] / 2 else t)
            return ("InGroup", g, len(rects))  # 组内空白 → 组尾
    for g, (gx, _) in enumerate(groups):
        if x < gx:
            return ("NewGroup", g)
    return ("NewGroup", len(groups))


def main():
    rows = max((17 + 5) // 6 * 2, MIN_ROWS)  # 17 枚 Large → 3 宏单元行 = 6 单元行
    group_h = TITLE_BAND + rows * CELL + (rows - 1) * GAP
    y0 = (VIEW_H + BOTTOM_BAND - group_h) / 2
    grid_y = y0 + TITLE_BAND
    groups = [
        (MARGIN_X, tile_rects(17, MARGIN_X, grid_y)),
        (MARGIN_X + GROUP_W + GROUP_GAP,
         tile_rects(2, MARGIN_X + GROUP_W + GROUP_GAP, grid_y)),
    ]
    print(f"rows={rows} group_h={group_h} y0={y0} grid_y={grid_y}")
    print(f"baseline slots={sum(len(r) for _, r in groups)}")
    probes = [
        (180.0, 276.0), (200.0, 276.0), (250.0, 276.0), (312.0, 276.0),
        (350.0, 276.0), (400.0, 276.0), (-10.0, 376.0), (50.0, 400.0),
        (1270.0, 760.0), (1270.0, 276.0),
    ]
    for x, y in probes:
        print(f"pos=({x:g},{y:g}) -> {drop_target(x, y, groups)}")


if __name__ == "__main__":
    main()
