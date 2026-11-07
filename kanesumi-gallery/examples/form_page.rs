// form_page —— 设置页参照页：整页只用 `kanesumi_controls::form` 的语义块拼出来。
//
// 这张卡证明「写设置页不需要画」：整页零 `Rect::new`、零命中函数、零 margin 凑间距 ——
// 分区标题、标签、控件、说明的位置与间距由页面语法层给出（`CONTROL_SPEC.md` §49），
// 应用只声明「哪个分区有哪些行」。settings 的 36 个页面就是照这个结构迁移的目标形态。
//
// 运行（Linux，任意支持 xdg-shell 的合成器）：
//   cargo run -p kanesumi-gallery --example form_page
// 出快照 PNG（任意平台）：
//   cargo run -p kanesumi-gallery --example form_page -- --snapshot out.png 1

use kanesumi_controls::form::{self, Form};
use kanesumi_controls::{FormLinkClicked, MetroRadioButtons, SwitchToggled};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

/// 页宽（内容列 600 + 左右各 24 内边距 + 一点余量，好看清边界）。
const PAGE_W: f32 = 660.0;
/// 页高。
const PAGE_H: f32 = 780.0;

struct SettingsPage {
    config: AppConfig,
    theme: MetroTheme,
    auto_brightness: bool,
    night_schedule: bool,
    /// 最近一次被点的链接跳转键（证明动作通了）。
    last_link: Option<String>,
}

impl SettingsPage {
    fn new() -> Self {
        Self {
            config: AppConfig::new(
                "org.ether.form-page-demo",
                "显示设置",
                EtherRole::Browser,
                PAGE_W,
                PAGE_H,
            ),
            theme: MetroTheme::ether_dark(),
            auto_brightness: true,
            night_schedule: false,
            last_link: None,
        }
    }
}

impl TreeApp for SettingsPage {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        // 页面底：只要一个铺满的背景容器，几何由框架负责。
        let page = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));

        // 内容列：宽 = 设计宽，内边距给一圈呼吸。
        let col = tree.insert_with(
            page,
            Stack::column(),
            LayoutProps {
                width: Some(form::COL_WIDTH),
                margin: Insets::new(24.0, 20.0, 24.0, 20.0),
                ..LayoutProps::default()
            },
        );

        let mut f = Form::new(col);
        f.page_title(tree, "显示");

        f.section(tree, "亮度", true);
        f.slider(tree, "屏幕亮度", "60%", 0.6, 0.0, 1.0, Some(0.01), None);
        f.toggle(
            tree,
            "自动亮度",
            self.auto_brightness,
            Some("按环境光调整屏幕亮度，手动拖动滑杆会暂时接管。"),
        );

        f.section(tree, "夜间模式", false);
        f.toggle(tree, "定时开启", self.night_schedule, Some("日落后自动转为暖色。"));
        f.note(tree, "夜间模式只改变色温，不改变亮度；外接显示器需自行支持。");

        // 任意控件走 labeled：单选组、下拉、色板都不必再各写一遍「标签在上」三行结构。
        let mut radios = MetroRadioButtons::new(vec!["深色".to_string(), "浅色".to_string()]);
        radios.max_columns = 2;
        f.labeled(tree, "应用模式", radios, Some("跟随系统时由 Chorus 决定。"));

        f.section(tree, "相关设置", false);
        f.link(tree, "声音设置", "sound");
        f.link(tree, "电源与睡眠", "power");

        let (button, _) = f.button(tree, "重置本页设置", Some("只影响本页，不影响其他页面的设置。"));
        let _ = button;
    }

    /// 动作里只改状态，不碰几何：几何由页面语法层负责，应用不该在动作里算坐标。
    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(toggled) = action.downcast_ref::<SwitchToggled>() {
            // 两行开关各自翻转；本例用「与当前值不同」判定，真实应用应按行的身份路由。
            if self.auto_brightness != toggled.0 {
                self.auto_brightness = toggled.0;
            } else {
                self.night_schedule = toggled.0;
            }
        } else if let Some(link) = action.downcast_ref::<FormLinkClicked>() {
            self.last_link = Some(link.target.to_string());
        }
    }
}

/// 字体查找（跨平台）：`KANESUMI_TEST_FONT` 优先，其次各系统常见路径。
fn find_font() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        return Some(std::path::PathBuf::from(p));
    }
    [
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(std::path::PathBuf::from)
    .find(|p| p.exists())
}

fn main() {
    let mut host = kanesumi_harness::TreeHost::new(SettingsPage::new());
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).map(String::as_str).unwrap_or("form_page.png");
        let scale = args.get(i + 2).and_then(|s| s.parse().ok()).unwrap_or(1.0);
        let font = find_font().expect("未找到字体");
        let engine = kanesumi_canvas::text::TextEngine::load(&font).expect("字体加载失败");
        let (w, h) = kanesumi_harness::snapshot::render_png(
            &mut host,
            &engine,
            kanesumi_core::Size::new(PAGE_W, PAGE_H),
            scale,
            3,
            std::path::Path::new(out),
        )
        .expect("快照失败");
        println!("snapshot {out} {w}x{h} font={}", font.display());
        return;
    }

    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));

    #[cfg(not(target_os = "linux"))]
    {
        let _ = &mut host;
        println!("form_page 实时窗口需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
    }
}
