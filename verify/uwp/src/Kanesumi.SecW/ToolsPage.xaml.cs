using System;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;

namespace Kanesumi.SecW
{
    /// <summary>控件覆盖台 —— 取证用，不是演示页。</summary>
    public sealed partial class ToolsPage : Page
    {
        public ToolsPage()
        {
            InitializeComponent();
        }

        private async void OnOpenDialog(object sender, RoutedEventArgs e)
        {
            // ContentDialog 走 OverlayCornerRadius —— 圆角残留会在这里最先暴露。
            var dialog = new ContentDialog
            {
                Title = "ContentDialog",
                Content = "默认 8px 圆角。应已由 OverlayCornerRadius 归零。\n\n用微软拼音在这里试中文输入。",
                CloseButtonText = "关闭",
                XamlRoot = XamlRoot,
            };

            await dialog.ShowAsync();
        }

        private void OnOpenTeachingTip(object sender, RoutedEventArgs e)
        {
            // TeachingTip 也是 Overlay 类，且它是 WinUI 2 2.x 才有的新控件 ——
            // 新控件是否走那两个圆角资源，正是本页要回答的问题之一。
            var tip = new Microsoft.UI.Xaml.Controls.TeachingTip
            {
                Title = "TeachingTip",
                Subtitle = "WinUI 2 新增控件",
                Content = "观察其圆角与阴影。",
                IsOpen = true,
                PreferredPlacement = Microsoft.UI.Xaml.Controls.TeachingTipPlacementMode.Center,
                XamlRoot = XamlRoot,
            };
            tip.Closed += (_, __) => { };
            tip.IsOpen = true;
        }
    }
}
