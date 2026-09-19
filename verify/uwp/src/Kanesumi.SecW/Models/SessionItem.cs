using System;
using System.Collections.ObjectModel;
using System.Linq;
using System.Threading.Tasks;
using Windows.UI.Xaml;
using Windows.UI.Xaml.Controls;
using Windows.UI.Xaml.Media.Animation;
using Windows.UI.Xaml.Navigation;

namespace Kanesumi.SecW
{
    /// <summary>
    /// 会话条目 —— 界面数据模型。
    ///
    /// 刻意用可变类而非 record：UWP 的数据绑定对不可变类型支持有限
    /// （x:Bind 的 OneWay 需要可通知的属性），这是平台约束不是设计选择。
    /// </summary>
    public sealed class SessionItem
    {
        public string Id { get; set; }
        public string Title { get; set; }
        public string Preview { get; set; }
        public string Workspace { get; set; }
        public string UpdatedAt { get; set; }
        public int TurnCount { get; set; }
        public string Initials { get; set; }
    }

    /// <summary>
    /// 示例数据。
    ///
    /// 这个验证工程的目的是「证明 Kanesumi 观感在真 UWP 上成立」，不是做产品，
    /// 所以数据是静态的。刻意不接 DSH 协议 —— 那会把验证工程变成一个项目，
    /// 而规格回流才是它的产出。
    /// </summary>
    public static class SampleData
    {
        public static ObservableCollection<SessionItem> Sessions { get; } = new ObservableCollection<SessionItem>
        {
            new SessionItem
            {
                Id = "s1",
                Title = "Arc Deck 骨架与 IME 验证",
                Preview = "中文 IME 在自研事件循环里组合成功，组合串带橙色下划线正确渲染。",
                Workspace = "Projects/arc-deck",
                UpdatedAt = "刚刚",
                TurnCount = 24,
                Initials = "AD",
            },
            new SessionItem
            {
                Id = "s2",
                Title = "Kanesumi 控件移植路线图",
                Preview = "新增规格来源 C：Windows 扇区实证，把前两类的结论升格为验过的。",
                Workspace = "Projects/Ether",
                UpdatedAt = "12 分钟前",
                TurnCount = 41,
                Initials = "KN",
            },
            new SessionItem
            {
                Id = "s3",
                Title = "流式排版性能实测",
                Preview = "两万字符冷启动：断行 0.142ms、塑形 4.313ms、光栅化 32.192ms。",
                Workspace = "Projects/arc-deck",
                UpdatedAt = "1 小时前",
                TurnCount = 8,
                Initials = "PF",
            },
            new SessionItem
            {
                Id = "s4",
                Title = "Sokuou 一致性向量套件",
                Preview = "把「同名 API 约定」从纪律升级成 CI 阻断。",
                Workspace = "Projects/Sokuou",
                UpdatedAt = "昨天",
                TurnCount = 15,
                Initials = "SK",
            },
            new SessionItem
            {
                Id = "s5",
                Title = "Ether launcher 迁移收尾",
                Preview = "文档仍在说 launcher 是 egui 形态，实际早已迁完 —— 文档是陈旧的。",
                Workspace = "Projects/Ether",
                UpdatedAt = "2 天前",
                TurnCount = 33,
                Initials = "ET",
            },
            new SessionItem
            {
                Id = "s6",
                Title = "UWP 动画词汇搬入",
                Preview = "词汇可以搬，模型不能搬：搬词汇表与时序，保留状态驱动渲染。",
                Workspace = "Projects/Ether",
                UpdatedAt = "3 天前",
                TurnCount = 19,
                Initials = "AN",
            },
        };
    }
}
