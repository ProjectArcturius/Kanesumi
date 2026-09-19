using System;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Navigation;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 应用入口与窗口装配。
    ///
    /// 这里刻意不做任何「框架观感调教」—— 主题相关的一切都在
    /// <c>App.xaml</c> 的资源覆盖里一次性完成（圆角归零 + 正典 token）。
    /// 代码里再去逐控件调外观，就是没把纪律放在声明层。
    /// </summary>
    sealed partial class App : Application
    {
        public App()
        {
            InitializeComponent();
            Suspending += OnSuspending;
        }

        protected override void OnLaunched(LaunchActivatedEventArgs e)
        {
            var rootFrame = Window.Current.Content as Frame;

            // 窗口已有内容时不要重复创建 —— 激活路径可能重入。
            if (rootFrame == null)
            {
                rootFrame = new Frame();

                // 导航失败时不要静默 —— 静默会把「页面起不来」变成「白屏」。
                rootFrame.NavigationFailed += OnNavigationFailed;

                Window.Current.Content = rootFrame;
            }

            if (e.PrelaunchActivated == false)
            {
                if (rootFrame.Content == null)
                {
                    rootFrame.Navigate(typeof(MainPage), e.Arguments);
                }
                Window.Current.Activate();
            }
        }

        private void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
        {
            throw new Exception($"无法导航到页面 {e.SourcePageType.FullName}");
        }

        private void OnSuspending(object sender, SuspendingEventArgs e)
        {
            var deferral = e.SuspendingOperation.GetDeferral();
            deferral.Complete();
        }
    }
}
