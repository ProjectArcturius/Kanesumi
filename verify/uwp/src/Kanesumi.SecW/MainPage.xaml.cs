using System;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 控件验证台。
    ///
    /// 页面的职责是**取证**，不是呈现产品：把 WinUI 2 的常用控件一次性摆出来，
    /// 让「圆角是否真的全局归零」变成肉眼可查的事实。
    /// 未归零的控件 → 记进 <c>docs/UWP_FEEDBACK.md</c>。
    /// </summary>
    public sealed partial class MainPage : Page
    {
        public MainPage()
        {
            InitializeComponent();
        }

        private async void OnOpenDialog(object sender, RoutedEventArgs e)
        {
            // ContentDialog 走 OverlayCornerRadius —— 若有圆角残留，这里最先暴露。
            var dialog = new ContentDialog
            {
                Title = "ContentDialog",
                Content = "默认 8px 圆角。应已由 OverlayCornerRadius 归零。",
                CloseButtonText = "关闭",
                XamlRoot = XamlRoot,
            };

            await dialog.ShowAsync();
        }
    }
}
