using System;
using Windows.UI;
using Windows.UI.ViewManagement;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Media;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 主题装配。
    ///
    /// 控件画刷由 <c>Themes/KanesumiTheme.xaml</c> 覆盖 —— 那是主要手段，
    /// 让系统控件默认就长成 Kanesumi 的样子，不需要给每个控件写 Style。
    ///
    /// 这里只补 XAML 覆盖不到的部分：**系统 chrome**（标题栏颜色）。
    /// 它是 Win32/合成器层的东西，不属于 XAML 资源系统。
    /// </summary>
    public static class KanesumiTheme
    {
        /// <summary>正典落地取值 —— 跟随桌面扇区（#1A1A1A / #E57812）。</summary>
        public static readonly Color Background = Color.FromArgb(0xFF, 0x1A, 0x1A, 0x1A);
        public static readonly Color Primary = Color.FromArgb(0xFF, 0xE5, 0x78, 0x12);
        public static readonly Color OnBackground = Color.FromArgb(0xFF, 0xF0, 0xF0, 0xF0);

        /// <summary>
        /// 应用系统标题栏配色。
        ///
        /// 为什么必须做：UWP 的标题栏默认跟随系统主题，若系统是浅色而应用是深底，
        /// 就会出现「白标题栏 + 黑内容」的割裂 —— 这恰好是 Kanesumi 最反对的那种
        /// 「chrome 不属于内容」的观感。
        ///
        /// 标题栏不是 XAML 控件，所以资源字典覆盖不到它，必须走 ApplicationView API。
        /// </summary>
        public static void ApplyTitleBar()
        {
            var view = ApplicationView.GetForCurrentView();
            var titleBar = view.TitleBar;
            if (titleBar == null)
            {
                // 某些环境（如 IoT / 部分 HoloLens 配置）没有标题栏。
                return;
            }

            titleBar.BackgroundColor = Background;
            titleBar.ForegroundColor = OnBackground;
            titleBar.InactiveBackgroundColor = Background;
            titleBar.InactiveForegroundColor = Color.FromArgb(0xFF, 0x9A, 0xA0, 0xA6);

            // 按钮区与标题栏同底 —— 不做「按钮区另起一色」的分割。
            titleBar.ButtonBackgroundColor = Background;
            titleBar.ButtonForegroundColor = OnBackground;
            titleBar.ButtonHoverBackgroundColor = Color.FromArgb(0xFF, 0x2E, 0x2E, 0x2E);
            titleBar.ButtonHoverForegroundColor = OnBackground;
            titleBar.ButtonPressedBackgroundColor = Primary;
            titleBar.ButtonPressedForegroundColor = Color.FromArgb(0xFF, 0x1A, 0x1A, 0x1A);
            titleBar.ButtonInactiveBackgroundColor = Background;
            titleBar.ButtonInactiveForegroundColor = Color.FromArgb(0xFF, 0x9A, 0xA0, 0xA6);
        }

        /// <summary>
        /// 强制深色。
        ///
        /// 正典说是 dark/light 二态，而 Kanesumi 的主要形态是深底
        /// （正典 §Ⅲ.3：深底 + 单一强调色）。浅色取值在 KanesumiTheme.xaml 里
        /// 也已给出，但本工程的验证目标是深色形态。
        /// </summary>
        public static void ApplyDark()
        {
            var root = Window.Current.Content as FrameworkElement;
            if (root != null)
            {
                root.RequestedTheme = ElementTheme.Dark;
            }
        }
    }
}
