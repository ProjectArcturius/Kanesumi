// Kanesumi-sec-w · 图标生成
//
// 用正典配色生成 MSIX 所需的图标资源，不引外部素材。
//
// 图形本身遵循 Kanesumi Design（正典 §Ⅲ.1）：
//   直角、纯色底（#1A1A1A）、单一强调色（#E57812）、无渐变、无阴影。
// 图形语言与 Arc Deck 的「3px 强调条」同源 —— 各扇区共圆心，不共代码。
//
// 用法：dotnet run --project tools/make-icons -- <输出目录>

using System.Drawing;
using System.Drawing.Imaging;

string outDir = args.Length > 0 ? args[0] : ".";
Directory.CreateDirectory(outDir);

// 正典落地取值（跟随桌面扇区 —— 参 AGENTS.md 铁律 3）
var bg = ColorTranslator.FromHtml("#1A1A1A");
var accent = ColorTranslator.FromHtml("#E57812");
var onSurface = ColorTranslator.FromHtml("#F0F0F0");

// 各尺寸都画同一套几何：底 + 3px 比例强调条 + 一个直角方块。
// 缩放时按比例换算，保证「直角」在任何尺寸下都是直角（不被磨圆）。
void Draw(int w, int h, string file)
{
    using var bmp = new Bitmap(w, h, PixelFormat.Format32bppArgb);
    using var g = Graphics.FromImage(bmp);

    // 关掉所有平滑 —— 直角不需要抗锯齿，且插值会磨掉直角的锐利感。
    g.InterpolationMode = System.Drawing.Drawing2D.InterpolationMode.NearestNeighbor;
    g.SmoothingMode = System.Drawing.Drawing2D.SmoothingMode.None;
    g.PixelOffsetMode = System.Drawing.Drawing2D.PixelOffsetMode.Half;

    g.Clear(bg);

    // 强调条：宽度取短边的 1/8，至少 2px —— 与正典的 3px 强调条等比同源。
    int bar = Math.Max(2, Math.Min(w, h) / 8);
    int pad = Math.Max(2, Math.Min(w, h) / 8);
    using (var b = new SolidBrush(accent))
    {
        g.FillRectangle(b, pad, pad, bar, h - pad * 2);
    }

    // 直角方块：象征「以直角丈量边缘」。仅在中大尺寸画，小图标里会糊。
    if (Math.Min(w, h) >= 71)
    {
        int sq = Math.Min(w, h) / 5;
        int sx = w - pad - sq;
        int sy = h - pad - sq;
        using var pen = new Pen(onSurface, Math.Max(1, Math.Min(w, h) / 64));
        g.DrawRectangle(pen, sx, sy, sq, sq);
    }

    bmp.Save(Path.Combine(outDir, file), ImageFormat.Png);
    Console.WriteLine($"  {file,-28} {w}×{h}");
}

Console.WriteLine($"生成图标 → {Path.GetFullPath(outDir)}");

Draw(44, 44, "Square44x44Logo.png");
Draw(50, 50, "StoreLogo.png");
Draw(71, 71, "Square71x71Logo.png");
Draw(150, 150, "Square150x150Logo.png");
Draw(310, 150, "Wide310x150Logo.png");
Draw(310, 310, "Square310x310Logo.png");
Draw(620, 300, "SplashScreen.png");

Console.WriteLine("完成。图形：直角底 + 强调条 + 直角方块 —— 无圆角、无渐变、无阴影。");
