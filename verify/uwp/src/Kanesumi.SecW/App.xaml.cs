using System;
using System.Text;
using Windows.ApplicationModel;
using Windows.ApplicationModel.Activation;
using Windows.Storage;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Navigation;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 应用入口与窗口装配。
    ///
    /// 主题相关的一切都在 <c>App.xaml</c> 的资源覆盖里一次性完成（圆角归零 +
    /// 画刷覆盖）。代码里再去逐控件调外观，就是没把纪律放在声明层。
    ///
    /// 这里额外做一件事：**把启动期异常落盘**。UWP 的未处理异常默认只进事件日志，
    /// 而事件日志里的 .NET Runtime 条目**不含异常消息**（只有代号），排查时等于盲猜。
    /// 一个 XAML 资源键写错就会崩，没有消息根本不知道是哪一处。
    /// </summary>
    sealed partial class App : Application
    {
        public App()
        {
            InitializeComponent();

            UnhandledException += OnUnhandledException;
            Suspending += OnSuspending;
        }

        private void OnUnhandledException(object sender, Windows.UI.Xaml.UnhandledExceptionEventArgs e)
        {
            WriteCrashLog(e.Exception);
        }

        /// <summary>把异常写到 LocalFolder\crash.log —— 便于无调试器时定位。</summary>
        internal static void WriteCrashLog(Exception ex)
        {
            try
            {
                var sb = new StringBuilder();
                sb.AppendLine($"--- {DateTimeOffset.Now:O} ---");
                sb.AppendLine(ex?.GetType().FullName);
                sb.AppendLine(ex?.Message);

                var inner = ex?.InnerException;
                var depth = 0;
                while (inner != null && depth < 6)
                {
                    sb.AppendLine($"  [inner {depth}] {inner.GetType().FullName}: {inner.Message}");
                    inner = inner.InnerException;
                    depth++;
                }

                sb.AppendLine(ex?.StackTrace);
                sb.AppendLine();

                var path = System.IO.Path.Combine(
                    ApplicationData.Current.LocalFolder.Path, "crash.log");
                System.IO.File.AppendAllText(path, sb.ToString());
            }
            catch
            {
                // 崩溃日志本身绝不能再抛 —— 那会把「可诊断的失败」变成「不可诊断的失败」。
            }
        }

        protected override void OnLaunched(LaunchActivatedEventArgs e)
        {
            var rootFrame = Window.Current.Content as Frame;

            // 窗口已有内容时不要重复创建 —— 激活路径可能重入。
            if (rootFrame == null)
            {
                rootFrame = new Frame();
                rootFrame.NavigationFailed += OnNavigationFailed;
                Window.Current.Content = rootFrame;
            }

            if (e.PrelaunchActivated == false)
            {
                if (rootFrame.Content == null)
                {
                    // 外壳是 NavigationView + Frame，页面由它自己导航。
                    rootFrame.Navigate(typeof(ShellPage), e.Arguments);
                }

                // 系统标题栏 / 主题 API 在某些上下文会抛（无标题栏的环境、启动早期
                // 尚未有 CoreWindow 等）。它们失败不该让整个应用起不来 ——
                // 标题栏配色是观感，不是功能。失败记进 crash.log。
                try
                {
                    KanesumiTheme.ApplyDark();
                    KanesumiTheme.ApplyTitleBar();
                }
                catch (Exception ex)
                {
                    WriteCrashLog(ex);
                }

                Window.Current.Activate();
            }
        }

        private void OnNavigationFailed(object sender, NavigationFailedEventArgs e)
        {
            WriteCrashLog(new Exception($"无法导航到页面 {e.SourcePageType?.FullName}", e.Exception));
            throw new Exception($"无法导航到页面 {e.SourcePageType?.FullName}");
        }

        private void OnSuspending(object sender, SuspendingEventArgs e)
        {
            var deferral = e.SuspendingOperation.GetDeferral();
            deferral.Complete();
        }
    }
}
