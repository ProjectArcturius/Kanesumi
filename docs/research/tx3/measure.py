#!/usr/bin/env python3
# measure.py —— tx3 主测量：真 Win10 截图 × Ether 样张（tx2）。
# 产出 metrics.csv（长表）：每个区域一行，列见 HEADER。
# 用法：python3 measure.py

import csv
from pathlib import Path

import numpy as np

import txlib as L

ROOT = Path(__file__).resolve().parent
WIN10 = ROOT / "../tx1/win10"
ETHER = ROOT / "../tx2/rust"

# Win10 区域表：(图, 框, 标签, 名义逻辑字号, 组)
WIN10_REGIONS = [
    ("设置页内容物-1920_1080-100%.png", (344, 45, 500, 112), "uwp100-title28", 28, "uwp"),
    ("设置页内容物-1920_1080-100%.png", (344, 255, 640, 292), "uwp100-hdr20", 20, "uwp"),
    ("设置页内容物-1920_1080-100%.png", (335, 296, 940, 328), "uwp100-body15", 15, "uwp"),
    ("设置页内容物-1920_1080-100%.png", (344, 388, 600, 432), "uwp100-sec20", 20, "uwp"),
    ("设置页内容物-1920_1080-100%.png", (344, 456, 720, 470), "uwp100-label15", 15, "uwp"),
    ("设置页内容物-1920_1080-100%.png", (348, 598, 600, 622), "uwp100-dd15", 15, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (505, 75, 760, 145), "uwp150-title28", 28, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (505, 240, 860, 290), "uwp150-hdr20", 20, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (500, 296, 1300, 346), "uwp150-body15", 15, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (505, 435, 900, 490), "uwp150-sec20", 20, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (505, 540, 950, 580), "uwp150-label15", 15, "uwp"),
    ("设置页内容物-1920_1080-150%.png", (516, 756, 780, 784), "uwp150-dd15", 15, "uwp"),
    ("设置页-1920_1080-125%.png", (840, 65, 1140, 130), "uwp125-title24", 24, "uwp"),
    ("设置页-1920_1080-125%.png", (200, 278, 340, 305), "uwp125-nav15-net", 15, "uwp"),
    ("设置页-1920_1080-125%.png", (200, 303, 450, 326), "uwp125-desc13", 13, "uwp"),
    ("设置页-1920_1080-125%.png", (1215, 275, 1465, 302), "uwp125-nav15-netip", 15, "uwp"),
    ("设置页-1920_1080-125%.png", (1215, 300, 1440, 328), "uwp125-desc13-vpn", 13, "uwp"),
    ("设置页-1920_1080-125%.png", (1555, 278, 1700, 305), "uwp125-nav15-persona", 15, "uwp"),
    ("文件管理器-1920_1080-150%.png", (464, 428, 700, 458), "gdi150-file1", 12, "gdi"),
    ("文件管理器-1920_1080-150%.png", (464, 458, 700, 490), "gdi150-file2", 12, "gdi"),
    ("文件管理器-1920_1080-150%.png", (105, 148, 330, 172), "gdi150-tab", 12, "gdi"),
    ("文件管理器-1920_1080-150%.png", (92, 440, 215, 470), "gdi150-nav", 12, "gdi"),
    ("文件管理器-1920_1080-150%.png", (435, 392, 1000, 408), "gdi150-colhdr", 12, "gdi"),
    ("文件管理器-1920_1080-150%.png", (806, 345, 1075, 364), "gdi150-search", 12, "gdi"),
]

# Ether 样张：tx2 联系表，8 组 × 3 行，组序固定。
ETHER_PROFILES = ["base", "med", "med-cg", "ol15-cg"]
ETHER_SCALES = [1, 2]
ETHER_BGS = ["light", "dark"]
GROUPS = [(13, "N"), (13, "B"), (15, "N"), (15, "B"),
          (20, "N"), (20, "B"), (28, "N"), (28, "B")]


def find_text_bands(cov, x0, thr=0.04):
    """按行投影找文字行带（返回 [(y0,y1)]，含行间空隙分开的每一条）。"""
    ink = (cov[:, x0:] > thr).sum(axis=1)
    rows = ink > 0
    bands = []
    y = 0
    h = len(rows)
    while y < h:
        if rows[y]:
            y2 = y
            while y2 < h and rows[y2]:
                y2 += 1
            bands.append((y, y2))
            y = y2
        else:
            y += 1
    return bands


def merge_to_lines(bands, n_lines):
    """把碎片 band 合并成恰好 n_lines 条：反复缝合间隙最小的相邻带。"""
    bands = list(bands)
    while len(bands) > n_lines:
        gaps = [(bands[i + 1][0] - bands[i][1], i) for i in range(len(bands) - 1)]
        _, i = min(gaps)
        bands[i] = (bands[i][0], bands[i + 1][1])
        del bands[i + 1]
    return bands


def blocks(cov, y0, y1, x0, gap_thr):
    """列投影分块（词/字组级），返回 [(x0,x1,density)]，density=块内覆盖率均值。

    过滤宽度 <4px 或密度 <0.03 的碎块（杂散 AA 像素、残留图标边缘）。
    """
    ink = (cov[y0:y1] > 0.04).sum(axis=0)
    cols = ink > 0
    out = []
    x = int(x0)
    w = len(cols)
    while x < w:
        if cols[x]:
            x2 = x
            last_ink = x
            while x2 < w:
                if cols[x2]:
                    last_ink = x2
                if x2 - last_ink > gap_thr:
                    break
                x2 += 1
            seg = cov[y0:y1, x:last_ink + 1]
            dens = float(seg.mean())
            if (last_ink + 1 - x) >= 4 and dens >= 0.03:
                out.append((x, last_ink + 1, dens))
            x = x2
        else:
            x += 1
    return out


HEADER = ["system", "sample", "region", "scale", "nominal_px", "mean_ink",
          "edge_tx1_px", "stem_px", "stem_n", "edge1090_px", "cij_24", "cij_12",
          "midtone_5", "midtone_3", "midtone_1", "ink_h_px"]


def row_for(tag, region, scale, nominal, cov):
    cov_t = L.tight_bbox(cov)
    stem, e1090, n, _ = L.stem_stats(cov_t)
    f24, f12, _ = L.fringe_stats(np.zeros((4, 4, 3)), cov_t)  # 占位：彩边在外部按原图算
    ink_h = float(np.sum(cov_t.max(axis=1) > 0.04))
    m5 = L.midtone_hist(cov_t)
    return {"system": tag[0], "sample": tag[1], "region": region,
            "scale": scale, "nominal_px": nominal,
            "mean_ink": round(L.mean_ink(cov_t), 4),
            "edge_tx1_px": round(L.edge_px_tx1(cov_t), 3),
            "stem_px": round(stem, 3), "stem_n": n,
            "edge1090_px": round(e1090, 3),
            "cij_24": "", "cij_12": "",
            "midtone_5": round(m5[0], 3), "midtone_3": round(m5[2], 3),
            "midtone_1": round(m5[4], 3),
            "ink_h_px": round(ink_h, 1)}


def main():
    rows = []
    # --- Win10 ---
    for name, box, region, nominal, grp in WIN10_REGIONS:
        img = L.load(WIN10 / name)
        reg = img[box[1]:box[3], box[0]:box[2]]
        cov, bg, fg = L.coverage(reg)
        r0, r1, c0, c1 = L.tight_slice(cov)
        cov_t = cov[r0:r1, c0:c1]
        reg_t = reg[r0:r1, c0:c1]
        stem, e1090, n, _ = L.stem_stats(cov_t)
        f24, f12, ne = L.fringe_stats(reg_t, cov_t)
        m5 = L.midtone_hist(cov_t)
        ink_h = float(np.sum(cov_t.max(axis=1) > 0.04))
        scale = float(name.split("-")[2].split("%")[0]) / 100.0
        rows.append({"system": "win10-" + grp, "sample": name, "region": region,
                     "scale": scale, "nominal_px": nominal,
                     "mean_ink": round(L.mean_ink(cov_t), 4),
                     "edge_tx1_px": round(L.edge_px_tx1(cov_t), 3),
                     "stem_px": round(stem, 3), "stem_n": n,
                     "edge1090_px": round(e1090, 3),
                     "cij_24": round(f24, 3), "cij_12": round(f12, 3),
                     "midtone_5": round(m5[0], 3), "midtone_3": round(m5[2], 3),
                     "midtone_1": round(m5[4], 3), "ink_h_px": round(ink_h, 1)})
    # --- Ether ---
    for prof in ETHER_PROFILES:
        for bg in ETHER_BGS:
            for sc in ETHER_SCALES:
                p = ETHER / f"rd_{prof}_{bg}_{sc}x.png"
                img = L.load(p)
                cov, _, _ = L.coverage(img)
                tx0 = L.text_x0(cov, min_gutter=15 * sc)
                bands = find_text_bands(cov, x0=tx0)
                lines = merge_to_lines(bands, 24)
                for gi, (size, weight) in enumerate(GROUPS):
                    y0, y1 = lines[gi * 3]
                    band = cov[y0:y1, tx0:]
                    cov_t = L.tight_bbox(band)
                    stem, e1090, n, _ = L.stem_stats(cov_t)
                    m5 = L.midtone_hist(cov_t)
                    ink_h = float(np.sum(cov_t.max(axis=1) > 0.04))
                    rows.append({"system": "ether", "sample": p.name,
                                 "region": f"{size}{weight}-line1", "scale": sc,
                                 "nominal_px": size,
                                 "mean_ink": round(L.mean_ink(cov_t), 4),
                                 "edge_tx1_px": round(L.edge_px_tx1(cov_t), 3),
                                 "stem_px": round(stem, 3), "stem_n": n,
                                 "edge1090_px": round(e1090, 3),
                                 "cij_24": 0.0, "cij_12": 0.0,
                                 "midtone_5": round(m5[0], 3),
                                 "midtone_3": round(m5[2], 3),
                                 "midtone_1": round(m5[4], 3),
                                 "ink_h_px": round(ink_h, 1)})
    # --- 同字符串墨量（网络和 / Internet / 个性化：Win10 125% 主页导航 vs Ether 正文行）---
    def match_row(sample, region, dens):
        return {"system": "match", "sample": sample, "region": region,
                "scale": "", "nominal_px": "", "mean_ink": round(dens, 4),
                "edge_tx1_px": "", "stem_px": "", "stem_n": "", "edge1090_px": "",
                "cij_24": "", "cij_12": "", "midtone_5": "", "midtone_3": "",
                "midtone_1": "", "ink_h_px": ""}

    ETHER_NAMES = ["设置", "网络和", "Internet", "蓝牙和其他设备", "个性化"]
    for prof in ETHER_PROFILES:
        for sc in ETHER_SCALES:
            p = ETHER / f"rd_{prof}_light_{sc}x.png"
            cov, _, _ = L.coverage(L.load(p))
            x0 = L.text_x0(cov, min_gutter=15 * sc)
            lines = merge_to_lines(find_text_bands(cov, x0=x0), 24)
            for gi, size in ((0, 13), (2, 15)):
                y0, y1 = lines[gi * 3]
                blks = blocks(cov, y0, y1, x0, gap_thr=size * 0.3 * sc)
                if len(blks) != 5:
                    print(f"  ! {p.name} {size}N 分块数 {len(blks)} ≠ 5，跳过")
                    continue
                for name, blk in zip(ETHER_NAMES, blks):
                    rows.append(match_row(p.name, f"{size}N-{name}", blk[2]))
    # Win10 125% 两个导航标题框
    for name, box, reg_name, pairs in [
        ("设置页-1920_1080-125%.png", (1215, 275, 1465, 302), "w10-网络和",
         ["网络和", "Internet"]),
        ("设置页-1920_1080-125%.png", (1555, 278, 1700, 305), "w10-个性化",
         ["个性化"]),
    ]:
        reg = L.load(WIN10 / name)[box[1]:box[3], box[0]:box[2]]
        cov, _, _ = L.coverage(reg)
        r0, r1, _, _ = L.tight_slice(cov)
        blks = blocks(cov, r0, r1, 0, gap_thr=4)
        if len(blks) != len(pairs):
            print(f"  ! {reg_name} 分块数 {len(blks)} ≠ {len(pairs)}，跳过")
            continue
        for pname, blk in zip(pairs, blks):
            rows.append(match_row(name, f"{reg_name}-{pname}", blk[2]))

    with (ROOT / "metrics.csv").open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=HEADER)
        w.writeheader()
        w.writerows(rows)
    print(f"→ {ROOT / 'metrics.csv'}  ({len(rows)} 行)")


if __name__ == "__main__":
    main()
