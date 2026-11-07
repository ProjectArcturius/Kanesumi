// form_theme_page —— 迁移演练：把 settings 的「个性化 · 主题」页原样搬成页面语法写法。
//
// 原实现 `settings/src/window/pages/theme.rs`（182 行，含 2 个单测）：页面声明部分用了
// 私有 grammar 的 `labeled` / `label_jump` / `toggle_item` / `section` / `related` 五处，
// 加一个自绘 `ToggleSwitch`、一个 `Col` 游标与 `Page`/`PageCtx`/`Binding` 三套胶水。
//
// 这里保留**页面逻辑不变**（读写 theme.toml 的 dock_autohide、深色/浅色单选、两枚相关设置
// 链接），只把「怎么画」换成 `kanesumi_controls::form`。差异只在三处：
//   1. `grammar::` 前缀 → `Form::` 方法；
//   2. 自绘开关 → 库里的 `MetroSwitch`（`form::toggle` 内部就是它）；
//   3. `Page` trait / `PageCtx` / `Binding` 三个应用级胶水 → 不需要（页面就是一次 build）。
//
// 运行（Linux）：cargo run -p kanesumi-gallery --example form_theme_page
// 出图：cargo run -p kanesumi-gallery --example form_theme_page -- --snapshot out.png 1

use std::path::PathBuf;

use kanesumi_controls::form::{self, Form};
use kanesumi_controls::{FormLinkClicked, MetroRadioButtons, RadioSelectionChanged, SwitchToggled};
use kanesumi_core::{MetroTheme, ThemeColor};
use kanesumi_harness::element::widgets::{Border, Stack};
use kanesumi_harness::element::{Action, Insets, LayoutProps, Tree, WidgetId};
use kanesumi_harness::{AppConfig, EtherRole, TreeApp};

/// 深浅模式（原 `settings::model::ThemeMode`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    const ALL: [Self; 2] = [Self::Dark, Self::Light];

    fn label(self) -> &'static str {
        match self {
            Self::Dark => "深色",
            Self::Light => "浅色",
        }
    }
}

/// `theme.toml` 路径（与合成器同款，测试可注入）。
fn theme_config_path() -> PathBuf {
    if let Ok(p) = std::env::var("ETHER_THEME_CONFIG") {
        return PathBuf::from(p);
    }
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join(".config/ether/theme.toml")
}

/// 从 `path` 读取 `dock_autohide`（缺省 / 文件缺失 = 常显）。
fn read_autohide_at(path: &std::path::Path) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    for line in raw.lines() {
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=')
            && k.trim() == "dock_autohide"
        {
            let v = v.trim().trim_matches('"');
            return v == "true" || v == "1";
        }
    }
    false
}

/// 把 `dock_autohide` 写进 `path`（保留其余行；已有键则替换，否则追加）。
fn write_autohide_at(path: &std::path::Path, value: bool) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let existing = std::fs::read_to_string(path).unwrap_or_default();
    let line = format!("dock_autohide = {value}");
    let mut out = String::new();
    let mut replaced = false;
    for l in existing.lines() {
        if l.trim().split_once('=').map(|(k, _)| k.trim()) == Some("dock_autohide") {
            out.push_str(&line);
            replaced = true;
        } else {
            out.push_str(l);
        }
        out.push('\n');
    }
    if !replaced {
        out.push_str(&line);
        out.push('\n');
    }
    let _ = std::fs::write(path, out);
}

/// 页宽：内容列 600 + 间隔 48 + 相关设置栏 240 + 左右内边距 48。
const PAGE_W: f32 = 936.0;
/// 页高。
const PAGE_H: f32 = 560.0;

struct ThemePage {
    config: AppConfig,
    theme: MetroTheme,
    mode: ThemeMode,
    /// 最近一次被点的「相关设置」链接键（证明动作通了）。
    last_link: Option<String>,
}

impl ThemePage {
    fn new() -> Self {
        Self {
            config: AppConfig::new("org.ether.form-theme-page", "个性化 · 主题", EtherRole::Browser, PAGE_W, PAGE_H),
            theme: MetroTheme::ether_dark(),
            mode: ThemeMode::Dark,
            last_link: None,
        }
    }
}

impl TreeApp for ThemePage {
    fn config(&self) -> &AppConfig {
        &self.config
    }

    fn theme(&self) -> MetroTheme {
        self.theme
    }

    fn build(&mut self, tree: &mut Tree) {
        let page = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));
        let content = tree.insert_with(
            page,
            Stack::row().with_spacing(form::CONTENT_GAP),
            LayoutProps {
                margin: Insets::new(24.0, 20.0, 24.0, 20.0),
                ..LayoutProps::default()
            },
        );
        let col = tree.insert_with(
            content,
            Stack::column(),
            LayoutProps {
                width: Some(form::COL_WIDTH),
                ..LayoutProps::default()
            },
        );

        let mut f = Form::new(col);
        f.section(tree, "应用模式", true);
        let mut radios =
            MetroRadioButtons::new(ThemeMode::ALL.iter().map(|m| m.label().to_string()).collect());
        radios.max_columns = 2;
        if let Some(i) = ThemeMode::ALL.iter().position(|m| *m == self.mode) {
            radios.select(i);
        }
        f.labeled(tree, "深浅模式", radios, Some("切换后立即生效，并同步所有 Ether 应用。"));

        f.section(tree, "任务栏", false);
        // 读一次磁盘：真实页面同样在 build 时读 theme.toml（Chorus 无该属性）。
        let checked = read_autohide_at(&theme_config_path());
        f.toggle(
            tree,
            "自动隐藏 Dock",
            checked,
            Some("Dock 在不用时收起；指针贴到屏幕最底一行停留或下推时再出现。"),
        );

        let status = match &self.last_link {
            Some(t) => format!("最近一次跳转：{t}"),
            None => "相关设置：强调色、壁纸。".to_string(),
        };
        f.note(tree, &status);

        // 相关设置栏：与内容列并排（窄窗时可改为下方，由外层决定传 None）。
        form::related(
            tree,
            content,
            &[(
                "相关设置",
                &[("强调色", "accent"), ("壁纸", "wallpaper")],
            )],
            Some(form::RELATED_WIDTH),
        );
    }

    fn on_action(&mut self, tree: &mut Tree, _from: WidgetId, action: Action) {
        if let Some(SwitchToggled(v)) = action.downcast_ref::<SwitchToggled>() {
            // 真实页面的行为：立刻写 theme.toml，合成器按 mtime 热加载。
            write_autohide_at(&theme_config_path(), *v);
            tree.invalidate_paint(tree.root());
        } else if let Some(sel) = action.downcast_ref::<RadioSelectionChanged>() {
            if let Some(m) = ThemeMode::ALL.get(sel.0) {
                self.mode = *m;
            }
            tree.invalidate_paint(tree.root());
        } else if let Some(link) = action.downcast_ref::<FormLinkClicked>() {
            self.last_link = Some(link.target.to_string());
            tree.invalidate_paint(tree.root());
        }
    }
}

/// 字体查找（跨平台）。
fn find_font() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("KANESUMI_TEST_FONT") {
        return Some(PathBuf::from(p));
    }
    [
        "C:/Windows/Fonts/msyh.ttc",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/TTF/DejaVuSans.ttf",
    ]
    .into_iter()
    .map(PathBuf::from)
    .find(|p| p.exists())
}

fn main() {
    let mut host = kanesumi_harness::TreeHost::new(ThemePage::new());
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--snapshot") {
        let out = args.get(i + 1).map(String::as_str).unwrap_or("form_theme_page.png");
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
        println!("form_theme_page 实时窗口需要 Linux Wayland 会话；用 --snapshot <out.png> [scale] 出图。");
    }
}
