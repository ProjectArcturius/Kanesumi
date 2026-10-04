#!/usr/bin/env python3
# analyze.py —— tx2 样张量化（辅助，不替代看图）。
#
# 沿用 tx1 的 mean_ink / edge_px，新增第三指标 halo_px：
#   mean_ink  —— 「平均墨量」：全图归一化覆盖率均值（背景 0、前景 1）。
#   edge_px   —— 边缘宽：每行最陡一阶差分 g（覆盖率/像素），该行边缘宽 ≈ 0.8/g 的中位数。
#                越小越锐。
#   halo_px   —— 「光晕宽度」：文字外侧覆盖率落在 (5%, 50%] 的连续像素带平均宽度。
#                每行取左右两侧「第一个 ≥50% 像素的外侧过渡带」（紧邻背景时），
#                全图取均值。轮廓加粗是干净阶梯，此值小；掩码膨胀拖出灰晕，此值大。
#
# 依赖 Pillow + numpy。用法：
#   python3 analyze.py
#   python3 analyze.py --optimize      # 额外用 Pillow 无损重压全部 PNG
#
# 仅辅助量化，结论以读图为准。

import argparse
import csv
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent
DIRS = [ROOT / "rust"]

LO = 0.05
HI = 0.50


def profile(img):
    arr = np.asarray(img.convert("L"), dtype=np.float32)
    bg = float(arr[0, 0])
    fg = 255.0 if bg < 128 else 0.0
    span = abs(fg - bg)
    if span < 1e-6:
        return None
    prof = np.clip(np.abs(arr - bg) / span, 0.0, 1.0)
    return prof


def mean_ink(prof):
    return float(prof.mean()) if prof is not None else 0.0


def edge_width(prof):
    """行扫描最陡一阶差分 → 行边缘宽 0.8/g 的中位数。"""
    if prof is None:
        return (0.0, 0)
    grad = np.abs(np.diff(prof, axis=1))
    peaks = grad.max(axis=1)
    peaks = peaks[peaks > 1e-6]
    if peaks.size == 0:
        return (0.0, 0)
    g = float(np.median(peaks))
    return (0.8 / g if g > 0 else 0.0, int(peaks.size))


def halo_width(prof):
    """文字外侧 (LO, HI] 覆盖率像素带平均宽度（物理像素）。"""
    if prof is None:
        return 0.0
    total = 0
    count = 0
    width = prof.shape[1]
    for row in prof:
        above = np.where(row >= HI)[0]
        if above.size == 0:
            continue
        left = int(above[0])
        right = int(above[-1])
        if row[0] < LO:  # 左侧从背景起步
            band = 0
            j = left
            while j - 1 >= 0 and row[j - 1] > LO:
                band += 1
                j -= 1
            total += band
            count += 1
        if row[-1] < LO:  # 右侧回到背景
            band = 0
            j = right
            while j + 1 < width and row[j + 1] > LO:
                band += 1
                j += 1
            total += band
            count += 1
    return total / count if count else 0.0


def optimize(paths):
    from PIL import Image

    before = sum(p.stat().st_size for p in paths)
    for p in paths:
        Image.open(p).save(p, optimize=True)
    after = sum(p.stat().st_size for p in paths)
    print(f"optimize: {len(paths)} 文件 {before/1e6:.1f} MB -> {after/1e6:.1f} MB")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--optimize", action="store_true")
    args = ap.parse_args()
    from PIL import Image

    # 量化只取未放大的联系表（放大图会把边缘 / 光晕宽度按倍率放大，不可比）。
    pngs = sorted(
        p
        for d in DIRS
        if d.exists()
        for p in d.glob("*.png")
        if "_zoom" not in p.name and "_holes" not in p.name
    )
    if args.optimize:
        optimize(pngs)
    rows = []
    for p in pngs:
        prof = profile(Image.open(p))
        ink = mean_ink(prof)
        edge, n = edge_width(prof)
        halo = halo_width(prof)
        rows.append((p.parent.name, p.name, ink, edge, halo, n))
    with (ROOT / "metrics.csv").open("w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(
            ["group", "file", "mean_ink", "edge_px_median", "halo_px", "edge_samples"]
        )
        writer.writerows(rows)
    print(
        f"{'group':6} {'file':42} {'mean_ink':>9} {'edge_px':>8} {'halo_px':>8} {'n':>6}"
    )
    for group, name, ink, edge, halo, n in rows:
        print(f"{group:6} {name:42} {ink:9.4f} {edge:8.2f} {halo:8.2f} {n:6d}")
    print(f"\n→ {ROOT / 'metrics.csv'}")


if __name__ == "__main__":
    main()
