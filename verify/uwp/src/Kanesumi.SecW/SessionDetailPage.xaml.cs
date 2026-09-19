using System;
using System.Linq;
using Windows.System;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Navigation;

namespace Kanesumi.SecW
{
    /// <summary>会话详情页。</summary>
    public sealed partial class SessionDetailPage : Page
    {
        private SessionItem _session;

        public SessionDetailPage()
        {
            InitializeComponent();
        }

        protected override void OnNavigatedTo(NavigationEventArgs e)
        {
            base.OnNavigatedTo(e);

            if (e.Parameter is string id)
            {
                _session = SampleData.Sessions.FirstOrDefault(s => s.Id == id);
            }

            if (_session != null)
            {
                TitleText.Text = _session.Title;
                MetaText.Text = $"{_session.Workspace} · {_session.TurnCount} 轮 · {_session.UpdatedAt}";
            }

            Composer.Focus(FocusState.Programmatic);
        }

        private void OnBackClick(object sender, RoutedEventArgs e)
        {
            if (Frame.CanGoBack)
            {
                Frame.GoBack();
            }
        }

        private void OnComposerKeyDown(object sender, Windows.UI.Xaml.Input.KeyRoutedEventArgs e)
        {
            if (e.Key == VirtualKey.Enter)
            {
                e.Handled = true;
                Send();
            }
        }

        private void OnSendClick(object sender, RoutedEventArgs e)
        {
            Send();
        }

        private void Send()
        {
            // 验证工程不接后端 —— 这里只清空并提示，重点是 IME 输入通路本身。
            if (string.IsNullOrWhiteSpace(Composer.Text))
            {
                return;
            }

            var sent = Composer.Text;
            Composer.Text = string.Empty;
            System.Diagnostics.Debug.WriteLine($"[Kanesumi] 提交: {sent}");
        }
    }
}
