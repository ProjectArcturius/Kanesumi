#!/usr/bin/env python3
# compare.py —— tx3 并排对照图。产出：
#   compare_3x_*.png   （3× 最近邻放大，带档名标签）
#   compare_1to1_*.png （1:1 原始像素，不放大）
# 用法：python3 compare.py

from pathlib import Path

import numpy as np
from PIL import Image, ImageDraw, ImageFont

import txlib as L
from measure import ETHER, WIN10, blocks, find_text_bands, merge_to_lines

FONT = None
for cand in ("C:/Windows/Fonts/msyh.ttc", "C:/Windows/Fonts/msyhbd.ttc"):
    try:
        FONT = ImageFont.truetype(cand, 22)
        break
    except OSError:
        continue

PROFILES = ["base", "med", "med-cg", "ol15-cg"]
MAGENTA = (255, 0, 255)


def ether_band(profile, bg, scale, group_idx):
    """返回 tx2 联系表某组第一行的 (RGB ndarray, y0, y1, x0)。"""
    cov, _, _ = L.coverage(L.load(ETHER / f"rd_{profile}_{bg}_{scale}x.png"))
    x0 = L.text_x0(cov, min_gutter=15 * scale)
    lines = merge_to_lines(find_text_bands(cov, x0=x0), 24)
    y0, y1 = lines[group_idx * 3]
    img = L.load(ETHER / f"rd_{profile}_{bg}_{scale}x.png")
    return img[y0:y1, x0:], y0, y1, x0


def ether_line_rgb(profile, bg, scale, size_logical):
    gi = {13: 0, 15: 2, 20: 4, 28: 6}[size_logical]
    img, y0, y1, x0 = ether_band(profile, bg, scale, gi)
    # 裁掉行尾空白
    cov, _, _ = L.coverage(img)
    _, _, _, c1 = L.tight_slice(cov)
    return img[:, :c1 + 8]


def win10_rgb(box, name):
    img = L.load(WIN10 / name)
    reg = img[box[1]:box[3], box[0]:box[2]]
    cov, _, _ = L.coverage(reg)
    r0, r1, c0, c1 = L.tight_slice(cov)
    return reg[r0:r1, c0:c1 + 8]


def sheet(rows, zoom, out, bg=(255, 255, 255)):
    """rows = [(标签, RGB ndarray)]；纵向堆叠，左缘标签栏，zoom 倍最近邻。"""
    pad = 6
    label_w = 430
    panels = []
    hmax = max(r[1].shape[0] for r in rows)
    for tag, rgb in rows:
        if rgb.shape[0] < hmax:  # 行高对齐：白底补齐
            rgb = np.concatenate([rgb, np.full((hmax - rgb.shape[0], rgb.shape[1], 3), 255.0)], axis=0)
        im = Image.fromarray(rgb.astype(np.uint8))
        im = im.resize((im.width * zoom, im.height * zoom), Image.NEAREST)
        panels.append((tag, im))
    w = label_w + max(im.width for _, im in panels) + 24
    h = sum(im.height + pad for _, im in panels) + pad
    canvas = Image.new("RGB", (w, h), MAGENTA)
    dr = ImageDraw.Draw(canvas)
    y = pad
    for tag, im in panels:
        dr.text((10, y + im.height // 2 - 14), tag, fill=(20, 20, 20), font=FONT)
        canvas.paste(im, (label_w, y))
        y += im.height + pad
    canvas.save(out)
    print("→", out)


def main():
    # 1) 15 逻辑 px 正文：Win10 100% vs Ether 1×（同物理字号）+ Ether 2×（同逻辑字号的 2× 形态）
    rows = [("Win10 UWP 100% 正文15（60%黑）", win10_rgb((335, 296, 940, 328), "设置页内容物-1920_1080-100%.png"))]
    for p in PROFILES:
        rows.append((f"Ether 1× 15N {p}", ether_line_rgb(p, "light", 1, 15)))
    sheet(rows, 3, "compare_3x_body15px_light.png")

    # 2) 30 物理px：Win10 150% 20px 节头 vs Ether 2× 15N（同为 30 物理 px em）
    rows = [("Win10 UWP 150% 节头20→30px", win10_rgb((505, 435, 900, 490), "设置页内容物-1920_1080-150%.png")),
            ("Win10 UWP 150% 节头20→30px (HD)", win10_rgb((505, 240, 860, 290), "设置页内容物-1920_1080-150%.png"))]
    for p in PROFILES:
        rows.append((f"Ether 2× 15N {p}（30px）", ether_line_rgb(p, "light", 2, 15)))
    sheet(rows, 3, "compare_3x_30pxem_light.png")

    # 3) GDI（资源管理器）ClearType vs UWP 灰度 vs Ether 2×
    w_gdi1 = win10_rgb((462, 428, 620, 458), "文件管理器-1920_1080-150%.png")
    w_gdi2 = win10_rgb((462, 458, 620, 490), "文件管理器-1920_1080-150%.png")
    rows = [("Win10 GDI 150% 文件名（ClearType）", np.concatenate([w_gdi1, w_gdi2], axis=0)),
            ("Win10 UWP 150% 正文15（灰度）", win10_rgb((500, 296, 860, 346), "设置页内容物-1920_1080-150%.png"))]
    for p in ("base", "med-cg"):
        rows.append((f"Ether 2× 15N {p}", ether_line_rgb(p, "light", 2, 15)))
    sheet(rows, 3, "compare_3x_gdi_vs_uwp_light.png")

    # 4) 同字符串「网络和 Internet」「个性化」：Win10 125% 导航 vs Ether 各档
    def blocks_concat(reg_rgb, cov, gap_thr):
        r0, r1, _, _ = L.tight_slice(cov)
        blks = blocks(cov, r0, r1, 0, gap_thr=gap_thr)
        return [reg_rgb[r0:r1, a:b + 2] for a, b, _ in blks]

    reg_net = L.load(WIN10 / "设置页-1920_1080-125%.png")[275:302, 1215:1465]
    cov_net, _, _ = L.coverage(reg_net)
    reg_per = L.load(WIN10 / "设置页-1920_1080-125%.png")[278:305, 1555:1665]
    cov_per, _, _ = L.coverage(reg_per)
    w_net = blocks_concat(reg_net, cov_net, 4)
    w_per = blocks_concat(reg_per, cov_per, 4)
    rows = [("Win10 125% 网络和 Internet（18.75px）", np.concatenate(w_net, axis=1)),
            ("Win10 125% 个性化（18.75px）", np.concatenate(w_per, axis=1))]
    for p in PROFILES:
        for sc, sname in ((1, "1×"), (2, "2×")):
            band = ether_line_rgb(p, "light", sc, 15)
            cov_b, _, _ = L.coverage(band)
            blks = blocks(cov_b, 0, band.shape[0], 0, gap_thr=15 * 0.3 * sc)
            if len(blks) == 5:
                seg = np.concatenate([band[:, a:b + 2] for a, b, _ in
                                      (blks[1], blks[4])], axis=1)
                rows.append((f"Ether {sname} 15N {p} 网络和+个性化", seg))
    sheet(rows, 3, "compare_3x_same_string_light.png")

    # 5) 1:1 原始像素：15 逻辑px 正文
    rows = [("Win10 UWP 100% 正文15（60%黑）", win10_rgb((335, 296, 940, 328), "设置页内容物-1920_1080-100%.png"))]
    for p in PROFILES:
        rows.append((f"Ether 1× 15N {p}", ether_line_rgb(p, "light", 1, 15)))
    rows.append(("Win10 UWP 150% 节头20→30px", win10_rgb((505, 435, 900, 490), "设置页内容物-1920_1080-150%.png")))
    for p in PROFILES:
        rows.append((f"Ether 2× 15N {p}（30px）", ether_line_rgb(p, "light", 2, 15)))
    sheet(rows, 1, "compare_1to1_light.png")

    # 6) 1:1 同字符串
    net_row = np.concatenate(w_net, axis=1)
    per_row = np.concatenate(w_per, axis=1)
    if per_row.shape[0] != net_row.shape[0]:  # 行高对齐
        per_row = np.concatenate([per_row, np.full((net_row.shape[0] - per_row.shape[0], per_row.shape[1], 3), 255.0)], axis=0)
    rows = [("Win10 125% 网络和 Internet / 个性化", np.concatenate(
        [net_row, np.full((net_row.shape[0], 12, 3), 255.0), per_row], axis=1))]
    for p in PROFILES:
        band = ether_line_rgb(p, "light", 2, 15)
        cov_b, _, _ = L.coverage(band)
        blks = blocks(cov_b, 0, band.shape[0], 0, gap_thr=15 * 0.3 * 2)
        if len(blks) == 5:
            seg = np.concatenate([band[:, a:b + 2] for a, b, _ in (blks[1], blks[4])], axis=1)
            rows.append((f"Ether 2× 15N {p} 网络和+个性化", seg))
    sheet(rows, 1, "compare_1to1_same_string_light.png")


if __name__ == "__main__":
    main()
