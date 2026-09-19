using System;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media.Animation;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 应用外壳：NavigationView + Frame。
    ///
    /// 导航用 UWP 的 <see cref="Frame"/> 而不是自己维护页面栈 —— 这是平台标准做法，
    /// 免费拿到返回栈、页面缓存与转场。Kanesumi 不需要重新发明这一层。
    /// </summary>
    public sealed partial class ShellPage : Page
    {
        public ShellPage()
        {
            InitializeComponent();

            // 默认进会话页。
            Nav.SelectedItem = Nav.MenuItems[0];
            ContentFrame.Navigate(typeof(SessionsPage), null, new EntranceNavigationTransitionInfo());
        }

        /// <summary>
        /// 导航选中变化。
        ///
        /// 类型必须完全限定：<c>Windows.UI.Xaml.Controls</c> 与
        /// <c>Microsoft.UI.Xaml.Controls</c> **都有** `NavigationView`，
        /// 而 WinUI 2 的版本才带有 XAML 里用到的那些成员。
        /// 不限定会在生成代码里报「没有与委托匹配的重载」，报错位置离真实原因很远。
        /// </summary>
        private void OnNavSelectionChanged(
            Microsoft.UI.Xaml.Controls.NavigationView sender,
            Microsoft.UI.Xaml.Controls.NavigationViewSelectionChangedEventArgs args)
        {
            if (args.IsSettingsSelected)
            {
                ContentFrame.Navigate(typeof(SettingsPage), null, new EntranceNavigationTransitionInfo());
                return;
            }

            if (args.SelectedItem is NavigationViewItem item && item.Tag is string tag)
            {
                var target = tag switch
                {
                    "sessions" => typeof(SessionsPage),
                    "workspaces" => typeof(WorkspacesPage),
                    "tools" => typeof(ToolsPage),
                    _ => typeof(SessionsPage),
                };

                if (ContentFrame.CurrentSourcePageType != target)
                {
                    // Entrance 用于同级切换；进入层级（列表 → 详情）用 DrillIn，
                    // 那是 UWP 动画库给出的场景映射，直接沿用。
                    ContentFrame.Navigate(target, null, new EntranceNavigationTransitionInfo());
                }
            }
        }
    }
}
