#!/usr/bin/env python3
# analyze.py —— tx1 样张量化（辅助，不替代看图）。
#
# 对 rust/ 与 freetype/ 下所有联系表 PNG 计算两个指标：
#   mean_ink  —— 「平均墨量」：全图归一化覆盖率均值（背景 0、前景 1）。
#   edge_px   —— 「笔画边缘对比度」：行扫描 10%→90% 过渡宽度的中位数（物理像素）。
#                越小越锐；灰度抗锯齿下约 1~3 px。加粗 / 对比度增强会改变它。
#
# 依赖 Pillow（读图）；numpy 仅为加速量化（参照脚本 freetype_ref.py 只用 Pillow）。
# 用法：
#   python3 analyze.py                 # 打印表 + 写 metrics.csv
#   python3 analyze.py --optimize      # 额外用 Pillow 无损重压全部 PNG（缩体积）
#
# 仅辅助量化，结论以读图为准。

import argparse
import csv
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent
DIRS = [ROOT / "rust", ROOT / "freetype"]


def profile(img):
    arr = np.asarray(img.convert("L"), dtype=np.float32)
    bg = float(arr[0, 0])
    fg = 255.0 if bg < 128 else 0.0
    span = abs(fg - bg)
    if span < 1e-6:
        return None, bg, fg, span
    prof = np.clip(np.abs(arr - bg) / span, 0.0, 1.0)
    return prof, bg, fg, span


def mean_ink(prof):
    return float(prof.mean()) if prof is not None else 0.0


def edge_width(prof):
    """行扫描 10%→90% 过渡宽度（物理像素）中位数。

    对每行取最陡一阶差分 `g`（归一化覆盖率/像素），该行边缘宽 ≈ 0.8 / g
    （线性 ramp 下 10%→90% 的宽度）。取全图各行边缘宽的中位数。
    这是「最锐边缘」的稳健估计：比阈值穿越法抗细笔画（细笔画峰值到不了 0.9 时
    穿越法会跑到邻字，宽度爆表）。数值越小越锐。
    """
    if prof is None:
        return (0.0, 0)
    grad = np.abs(np.diff(prof, axis=1))
    peaks = grad.max(axis=1)
    peaks = peaks[peaks > 1e-6]
    if peaks.size == 0:
        return (0.0, 0)
    g = float(np.median(peaks))
    return (0.8 / g if g > 0 else 0.0, int(peaks.size))


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

    pngs = sorted(p for d in DIRS if d.exists() for p in d.glob("*.png"))
    if args.optimize:
        optimize(pngs)
    rows = []
    for p in pngs:
        prof, _, _, _ = profile(Image.open(p))
        ink = mean_ink(prof)
        edge, n = edge_width(prof)
        group = "rust" if p.parent.name == "rust" else "freetype"
        rows.append((group, p.name, ink, edge, n))
    with (ROOT / "metrics.csv").open("w", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(["group", "file", "mean_ink", "edge_px_median", "edge_samples"])
        writer.writerows(rows)
    print(f"{'group':8} {'file':40} {'mean_ink':>9} {'edge_px':>8} {'n':>6}")
    for group, name, ink, edge, n in rows:
        print(f"{group:8} {name:40} {ink:9.4f} {edge:8.2f} {n:6d}")
    print(f"\n→ {ROOT / 'metrics.csv'}")


if __name__ == "__main__":
    main()
