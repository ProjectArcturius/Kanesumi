#!/usr/bin/env python3
# txlib.py —— tx3 测量公共库：覆盖率、渲染方式判定、笔画与边缘测量。
# 口径与 tx1/tx2 的 analyze.py 一致（mean_ink / edge_px），另加 10–90 过渡宽、
# 竖画宽、中灰分布、彩边占比。仅用 Pillow + numpy。

import numpy as np
from PIL import Image


def load(path):
    """读图 → float32 RGB（0–255）。BMP / PNG 通吃。"""
    return np.asarray(Image.open(path).convert("RGB"), dtype=np.float32)


def coverage(reg):
    """区域 → 归一化覆盖率（前景 1 / 背景 0）。

    背景取区域四边框像素的中位数（文字区边框几乎全是背景）；
    前景取覆盖率极端分位。深底白字与浅底黑字通吃。
    """
    lum = 0.2126 * reg[:, :, 0] + 0.7152 * reg[:, :, 1] + 0.0722 * reg[:, :, 2]
    border = np.concatenate([lum[0, :], lum[-1, :], lum[:, 0], lum[:, -1]])
    bg = float(np.median(border))
    lo, hi = np.percentile(lum, 2), np.percentile(lum, 98)
    fg = lo if abs(bg - hi) < abs(bg - lo) else hi
    span = fg - bg
    if abs(span) < 1e-6:
        return None, bg, fg
    cov = (lum - bg) / span
    return np.clip(cov, 0.0, 1.0), bg, fg


def fringe_stats(reg, cov):
    """彩边判定：边缘像素（覆盖率 15–85%）里 RGB 通道散度 > 24 / > 12 的占比。

    灰度抗锯齿的前后景都是中性灰，边缘像素通道散度应为 0；
    亚像素渲染在竖画边缘出现 R/B 分离。返回 (占比24, 占比12, 边缘像素数)。
    """
    edge = (cov > 0.15) & (cov < 0.85)
    n = int(edge.sum())
    if n == 0:
        return 0.0, 0.0, 0
    spread = reg.max(axis=2) - reg.min(axis=2)
    vals = spread[edge]
    return float((vals > 24).mean()), float((vals > 12).mean()), n


def mean_ink(cov):
    """与 tx1/tx2 analyze.py 同口径：全区域覆盖率均值。"""
    return float(cov.mean())


def tight_bbox(cov, thr=0.04):
    """文字紧包围盒（去四周空白），返回裁剪后的 cov。"""
    rows = np.where(cov.max(axis=1) > thr)[0]
    cols = np.where(cov.max(axis=0) > thr)[0]
    if rows.size == 0 or cols.size == 0:
        return cov
    return cov[rows[0]:rows[-1] + 1, cols[0]:cols[-1] + 1]


def tight_slice(cov, thr=0.04):
    """同 tight_bbox 的包围盒，但返回 (row0, row1, col0, col1) 以便同步裁 RGB。"""
    rows = np.where(cov.max(axis=1) > thr)[0]
    cols = np.where(cov.max(axis=0) > thr)[0]
    if rows.size == 0 or cols.size == 0:
        return 0, cov.shape[0], 0, cov.shape[1]
    return rows[0], rows[-1] + 1, cols[0], cols[-1] + 1


def edge_px_tx1(cov):
    """tx1 口径：每行最陡一阶差分 g → 行边缘宽 0.8/g，取中位数。"""
    grad = np.abs(np.diff(cov, axis=1))
    peaks = grad.max(axis=1)
    peaks = peaks[peaks > 1e-6]
    if peaks.size == 0:
        return 0.0
    return 0.8 / float(np.median(peaks))


def _cross(profile, level, x0, x1, rising):
    """在 [x0,x1) 内用线性插值找 profile 穿越level的位置。"""
    x1 = min(x1, len(profile) - 1)
    for x in range(x0, x1):
        a, b = profile[x], profile[x + 1]
        if (a < level <= b) if rising else (a > level >= b):
            t = (level - a) / (b - a)
            return x + t
    return None


def stem_stats(cov, min_run=0.5, min_rows=6, max_w=10):
    """竖画测量：覆盖率 >50% 的连续段在 ≥min_rows 行中稳定出现的列为竖画。

    返回 (竖画宽中位数, 边缘 10–90 过渡宽中位数, 竖画数, 各竖画宽列表)。
    过渡宽对每条竖画的左右边各取若干行的插值穿越距离。
    """
    binary = cov > min_run
    rowcount = binary.sum(axis=0)
    h = cov.shape[0]
    stems = []
    edges = []
    x = 0
    w = cov.shape[1]
    while x < w:
        if rowcount[x] >= min_rows:
            x2 = x
            while x2 < w and rowcount[x2] >= min_rows:
                x2 += 1
            # 候选竖画列带 [x, x2)：逐行量 >50% 游程宽（取含带中心的游程）
            widths = []
            cx = (x + x2 - 1) / 2.0
            for y in range(h):
                row = binary[y]
                if not row[int(cx)]:
                    continue
                a = int(cx)
                while a > 0 and row[a - 1]:
                    a -= 1
                b = int(cx)
                while b < w - 1 and row[b + 1]:
                    b += 1
                if b - a + 1 <= max_w:
                    widths.append((y, a, b))
            if len(widths) >= min_rows:
                ys = [t[0] for t in widths]
                y_mid = int(np.median(ys))
                ws = [b - a + 1 for (y, a, b) in widths]
                if not ws:
                    x = x2
                    continue
                stems.append(float(np.median(ws)))
                # 过渡宽：中部三行，左右边 10–90 插值穿越
                for (y, a, b) in widths:
                    if abs(y - y_mid) > 2:
                        continue
                    p = cov[y]
                    lw = _cross(p, 0.9, max(a - 3, 0), a + 1, True)
                    l1 = _cross(p, 0.1, max(a - 4, 0), a + 1, True)
                    r9 = _cross(p, 0.9, b, min(b + 4, w - 1), False)
                    r1 = _cross(p, 0.1, b, min(b + 5, w - 1), False)
                    if lw is not None and l1 is not None:
                        edges.append(abs(l1 - lw))
                    if r9 is not None and r1 is not None:
                        edges.append(abs(r1 - r9))
            x = x2
        else:
            x += 1
    if not stems:
        return 0.0, 0.0, 0, []
    med_edge = float(np.median(edges)) if edges else 0.0
    return float(np.median(stems)), med_edge, len(stems), stems


def midtone_hist(cov):
    """边缘像素（5–95%）覆盖率五档分布：contrast/gamma 越强，两端越多。"""
    vals = cov[(cov > 0.05) & (cov < 0.95)]
    if vals.size == 0:
        return [0.0] * 5
    hist, _ = np.histogram(vals, bins=[0.05, 0.2, 0.4, 0.6, 0.8, 0.95])
    return [float(v / vals.size) for v in hist]


def text_x0(cov, min_gutter=15):
    """自动找正文起始列：跳过最左的标签列（窄墨带 + 宽 ≥min_gutter 的空沟）。"""
    ink = (cov > 0.04).sum(axis=0)
    cols = np.where(ink > 0)[0]
    if cols.size == 0:
        return 0
    gaps = np.where(np.diff(cols) >= min_gutter)[0]
    if gaps.size == 0:
        return 0
    return int(cols[gaps[0] + 1])
