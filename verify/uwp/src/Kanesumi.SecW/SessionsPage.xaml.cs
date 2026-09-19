using System;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media.Animation;
using Windows.UI.Xaml.Navigation;

namespace Kanesumi.SecW
{
    /// <summary>会话列表页。</summary>
    public sealed partial class SessionsPage : Page
    {
        public SessionsPage()
        {
            InitializeComponent();
            SessionList.ItemsSource = SampleData.Sessions;
        }

        private void OnSessionClick(object sender, ItemClickEventArgs e)
        {
            if (e.ClickedItem is not SessionItem item)
            {
                return;
            }

            // DrillIn 是 UWP 给「进入层级」的专用转场，与同级切换的 Entrance 不同。
            // 参 ANIMATION_SPEC §Ⅱ：UWP 的词汇直接对应到场景，不必自己发明。
            Frame.Navigate(typeof(SessionDetailPage), item.Id, new DrillInNavigationTransitionInfo());
        }
    }
}
