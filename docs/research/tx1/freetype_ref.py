#!/usr/bin/env python3
# freetype_ref.py —— FreeType 参照样张（tx1，2026-10-04）。
#
# 用 Pillow 内置 FreeType 渲染与 Rust 样张同一组文本、同一字体（Noto Sans CJK SC）、
# 同一物理像素字号（2×），出两种模式：
#   default —— 引擎默认；
#   darken  —— FREETYPE_PROPERTIES="cff:no-stem-darkening=0"（打开 CFF 笔画加粗）。
# 后者需在 FreeType 初始化前设好环境变量，脚本会自检并 re-exec 自身。
#
# 运行：
#   python3 freetype_ref.py --mode default
#   python3 freetype_ref.py --mode darken
# 输出目录默认 ../rust 同级的 docs/research/tx1/freetype。

import argparse
import os
import sys
from pathlib import Path

FONT_REG = "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"
FONT_BLD = "/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc"
SIZES = [13, 15, 20, 28]
PAD = 12
LABEL_W = 52
LABEL_SIZE = 12
ROW_GAP = 12
SCALE = 2  # 物理像素倍率（主形态）

TEXT_LINES = [
    "设置　网络和 Internet　蓝牙和其他设备　个性化",
    "Settings　Network & Internet　Bluetooth & devices",
    "0123456789　文件资源管理器　The quick brown fox",
]

BG_DARK = (31, 31, 31)
FG_DARK = (255, 255, 255)
BG_LIGHT = (255, 255, 255)
FG_LIGHT = (0, 0, 0)


def reexec_if_darken(mode):
    """darken 模式需在 FreeType 加载任何字体前设好环境变量。"""
    want = "cff:no-stem-darkening=0"
    if mode == "darken" and os.environ.get("FREETYPE_PROPERTIES") != want:
        os.environ["FREETYPE_PROPERTIES"] = want
        os.execv(sys.executable, [sys.executable] + sys.argv)


def sc_face_index(path):
    """找族名含 CJK SC 的 TTC 字面下标（裁定 N-40）。"""
    from PIL import ImageFont

    for i in range(40):
        try:
            font = ImageFont.truetype(path, 24, index=i)
        except Exception:
            break
        family, _ = font.getname()
        if "CJK SC" in (family or "").upper():
            return i
    return 0


def inner_width_px(font_bold):
    """内容列宽（物理像素）：以 28 Bold 最长行定宽，保证不折行。"""
    return int(max(font_bold.getlength(line) for line in TEXT_LINES)) + 8


def render_contact(mode, dark, out_dir, reg_idx, bld_idx):
    from PIL import Image, ImageDraw, ImageFont

    bg, fg = (BG_DARK, FG_DARK) if dark else (BG_LIGHT, FG_LIGHT)
    # 字号与列宽一律物理像素（与 Rust 样张 size_phys = 逻辑 × SCALE 对齐）。
    bold28 = ImageFont.truetype(FONT_BLD, 28 * SCALE, index=bld_idx)
    inner_w = inner_width_px(bold28)
    label_font = ImageFont.truetype(FONT_REG, LABEL_SIZE * SCALE, index=reg_idx)

    # 行高累加（与 Rust 的 engine.line_height 近似：FreeType ascent+descent）。
    rows = []
    content_h = 0
    for size in SIZES:
        for bold in (False, True):
            font = ImageFont.truetype(FONT_BLD if bold else FONT_REG, size * SCALE,
                                      index=bld_idx if bold else reg_idx)
            ascent, descent = font.getmetrics()
            lh = ascent + descent
            rows.append((size, bold, font, lh))
            content_h += 3 * lh + ROW_GAP
    total_h = PAD * SCALE + content_h + PAD * SCALE
    lw = (PAD + LABEL_W) * SCALE + inner_w + PAD * SCALE
    img = Image.new("RGB", (lw, total_h), bg)
    draw = ImageDraw.Draw(img)

    y = PAD * SCALE
    for size, bold, font, lh in rows:
        tag = f"{size}{'B' if bold else 'N'}"
        draw.text((PAD * SCALE, y), tag, font=label_font, fill=fg)
        for i, content in enumerate(TEXT_LINES):
            draw.text(((PAD + LABEL_W) * SCALE, y + i * lh), content, font=font, fill=fg)
        y += 3 * lh + ROW_GAP
    tag = "dark" if dark else "light"
    name = f"ft_{mode}_{tag}_2x.png"
    img.save(out_dir / name, optimize=True)
    # 与 Rust 同区域：左上角 13N 行，3× 最近邻。
    lh13 = rows[0][3]
    box = (0, 0, (PAD + LABEL_W + 360) * SCALE, PAD * SCALE + lh13 + 6)
    zoom = img.crop(box)
    zoom = zoom.resize((zoom.width * 3, zoom.height * 3), Image.NEAREST)
    zoom.save(out_dir / f"ft_{mode}_{tag}_2x_zoom3.png", optimize=True)
    return name, img


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--mode", choices=["default", "darken"], required=True)
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent / "freetype"))
    args = ap.parse_args()
    reexec_if_darken(args.mode)

    out_dir = Path(args.out)
    out_dir.mkdir(parents=True, exist_ok=True)
    reg_idx, bld_idx = sc_face_index(FONT_REG), sc_face_index(FONT_BLD)
    made = []
    for dark in (True, False):
        made.append(render_contact(args.mode, dark, out_dir, reg_idx, bld_idx)[0])
    print(f"freetype_ref[{args.mode}]: {', '.join(made)} -> {out_dir}")


if __name__ == "__main__":
    main()
