//! # kanesumi —— Kanesumi 应用工具链
//!
//! 目标是对齐 `dotnet` 的手感：一个空目录，一条命令出一个能装进系统的包。
//!
//! ```text
//! kanesumi new   myapp        # 建项目（Kanesumi.toml + Cargo.toml + src/main.rs）
//! kanesumi run                # 开发循环（cargo run）
//! kanesumi build [--release]  # 构建
//! kanesumi package            # 构建 + 打 deb（含 ether-manifest 与 .desktop）
//! kanesumi check              # 只校验项目文件
//! ```
//!
//! 打包不需要 `dpkg-deb`：deb 就是 ar + tar + gzip，本工具自己写（`deb.rs`），
//! 因此 Windows 开发机也能出包。

mod deb;
mod project;
mod toml_lite;

use std::path::{Path, PathBuf};
use std::process::Command;

use project::Project;

/// 项目文件名。
const PROJECT_FILE: &str = "Kanesumi.toml";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match run(&args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("kanesumi: {e}");
            1
        }
    };
    std::process::exit(code);
}

fn run(args: &[String]) -> Result<(), String> {
    let Some(cmd) = args.first().map(String::as_str) else {
        print_help();
        return Ok(());
    };
    let rest = &args[1..];
    match cmd {
        "new" => cmd_new(rest),
        "check" => cmd_check(),
        "build" => cmd_build(rest),
        "run" => cmd_run(),
        "package" => cmd_package(rest),
        "-h" | "--help" | "help" => {
            print_help();
            Ok(())
        }
        // 版本号取编译期 `CARGO_PKG_VERSION`：装到系统后 `kanesumi --version` 就能自证是哪一版。
        "-V" | "--version" => {
            println!("kanesumi {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => Err(format!("未知子命令 `{other}`；`kanesumi --help` 看用法")),
    }
}

fn print_help() {
    println!(
        "kanesumi —— Kanesumi 应用工具链\n\
         \n\
         用法：\n\
         \x20 kanesumi new <名字> [--app-id <反域名>] [--dir <目录>] [--template blank|settings]\n\
         \x20 kanesumi run\n\
         \x20 kanesumi build [--release]\n\
         \x20 kanesumi package [--no-build] [--out <目录>]\n\
         \x20 kanesumi check\n\
         \n\
         package 产出：dist/<app_id>_<version>-<revision>_<arch>.deb\n\
         （内含 usr/bin、usr/share/ether/manifests、usr/share/applications）"
    );
}

// ── new ─────────────────────────────────────────────────────────────────────

fn cmd_new(args: &[String]) -> Result<(), String> {
    let mut name: Option<String> = None;
    let mut app_id: Option<String> = None;
    let mut dir: Option<PathBuf> = None;
    let mut template = Template::Blank;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--app-id" => {
                app_id = Some(take_value(args, i, "--app-id")?);
                i += 2;
            }
            "--dir" => {
                dir = Some(PathBuf::from(take_value(args, i, "--dir")?));
                i += 2;
            }
            "--template" => {
                template = Template::parse(&take_value(args, i, "--template")?)?;
                i += 2;
            }
            other if other.starts_with("--") => return Err(format!("未知选项 `{other}`")),
            other => {
                name = Some(other.to_string());
                i += 1;
            }
        }
    }
    let name = name.ok_or("需要应用名：kanesumi new <名字>")?;
    let slug = slugify(&name);
    let app_id = app_id.unwrap_or_else(|| format!("org.example.{slug}"));
    project::validate_app_id(&app_id)?;

    let dir = dir.unwrap_or_else(|| PathBuf::from(&slug));
    if dir.join(PROJECT_FILE).exists() {
        return Err(format!("{} 已存在 {}", dir.display(), PROJECT_FILE));
    }
    std::fs::create_dir_all(dir.join("src")).map_err(|e| format!("建目录失败：{e}"))?;

    let binary = format!("kanesumi-{slug}");
    let files: [(&str, String); 4] = [
        (PROJECT_FILE, scaffold_project(&name, &app_id, &binary)),
        ("Cargo.toml", scaffold_cargo(&slug, &binary)),
        ("src/main.rs", scaffold_main(&name, template)),
        (".gitignore", "target/\ndist/\n".to_string()),
    ];
    for (path, content) in files {
        let full = dir.join(path);
        std::fs::write(&full, content).map_err(|e| format!("写 {} 失败：{e}", full.display()))?;
    }

    println!("已建项目 {}", dir.display());
    println!("下一步：");
    println!("  cd {}", dir.display());
    println!("  kanesumi run          # 起窗口（需要 Wayland 会话）");
    println!("  kanesumi package      # 出 deb 到 dist/");
    Ok(())
}

fn scaffold_project(name: &str, app_id: &str, binary: &str) -> String {
    format!(
        r#"# Kanesumi 项目文件 —— 既是构建配置，也是安装后的运行时 manifest 来源。
# 参 Kanesumi 仓 docs/DEV_GUIDE.md。

[package]
# deb 修订号：改包不改应用版本时 +1。
revision = 1
architecture = "amd64"
maintainer = "Kanesumi Maintainers <maintainers@example.invalid>"
# apt 级依赖（字体、运行库等）；运行时级依赖写在 [capabilities]。
depends = []

[identity]
app_id      = "{app_id}"
name        = "{name}"
version     = "0.1.0"
min_runtime = "0.1.0"

[role]
kind = "browser"

[entry]
binary = "{binary}"

[capabilities]
notifications = false
global_menu   = false
clipboard     = true
network       = false
storage       = "read"
media         = false
screencast    = false

[presentation]
icon    = "{app_id}"
summary = "{name}"
description = "用 Kanesumi 构建的原生应用"
"#
    )
}

fn scaffold_cargo(slug: &str, binary: &str) -> String {
    format!(
        r#"[package]
name = "{slug}"
version = "0.1.0"
edition = "2024"

[[bin]]
name = "{binary}"
path = "src/main.rs"

[dependencies]
kanesumi-harness = {{ git = "https://github.com/ProjectArcturius/Kanesumi" }}
kanesumi-controls = {{ git = "https://github.com/ProjectArcturius/Kanesumi" }}
kanesumi-core = {{ git = "https://github.com/ProjectArcturius/Kanesumi" }}
"#
    )
}

/// 脚手架模板：`blank` 最小骨架，`settings` 设置页形状。
#[derive(Clone, Copy, PartialEq)]
enum Template {
    /// 最小骨架：一个开关 + 一段说明。
    Blank,
    /// 设置页形状：页标题、分区、滑杆行、开关行、链接行、按钮行。
    Settings,
}

impl Template {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "blank" => Ok(Self::Blank),
            "settings" => Ok(Self::Settings),
            other => Err(format!("未知模板 `{other}`；可选 blank / settings")),
        }
    }
}

fn scaffold_main(name: &str, template: Template) -> String {
    match template {
        Template::Blank => scaffold_main_blank(name),
        Template::Settings => scaffold_main_settings(name),
    }
}

fn scaffold_main_blank(name: &str) -> String {
    format!(
        r#"// {name} —— 用页面语法写的第一个页面：零手写几何。
//
// 整页由 `kanesumi_controls::form` 的语义块拼出（参 Kanesumi 仓 docs/DEV_GUIDE.md §二·五）。
// 动作里只改状态，不碰坐标：位置与间距由页面语法层负责。

use kanesumi_controls::form::{{self, Form}};
use kanesumi_controls::SwitchToggled;
use kanesumi_core::{{MetroTheme, ThemeColor}};
use kanesumi_harness::element::widgets::{{Border, Stack}};
use kanesumi_harness::element::{{Action, Insets, LayoutProps, Tree, WidgetId}};
use kanesumi_harness::{{AppConfig, EtherRole, TreeApp}};

struct App {{
    config: AppConfig,
    theme: MetroTheme,
    enabled: bool,
}}

impl App {{
    fn new() -> Self {{
        Self {{
            config: AppConfig::new(
                "{name}",
                "!TITLE!",
                EtherRole::Browser,
                640.0,
                480.0,
            ),
            theme: MetroTheme::ether_dark(),
            enabled: false,
        }}
    }}
}}

impl TreeApp for App {{
    fn config(&self) -> &AppConfig {{ &self.config }}
    fn theme(&self) -> MetroTheme {{ self.theme }}

    fn build(&mut self, tree: &mut Tree) {{
        let page = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));
        let col = tree.insert_with(
            page,
            Stack::column(),
            LayoutProps {{
                width: Some(form::COL_WIDTH),
                margin: Insets::new(24.0, 20.0, 24.0, 20.0),
                ..LayoutProps::default()
            }},
        );

        let mut f = Form::new(col);
        f.page_title(tree, "首页");
        f.section(tree, "示例", true);
        f.toggle(tree, "启用示例开关", self.enabled, Some("点一下试试。"));
        f.note(tree, "这一页没有一行 Rect，也没有一处手算间距。");
    }}

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {{
        if let Some(t) = action.downcast_ref::<SwitchToggled>() {{
            self.enabled = t.0;
        }}
    }}
}}

fn main() {{
    let host = kanesumi_harness::TreeHost::new(App::new());
    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
    #[cfg(not(target_os = "linux"))]
    {{
        let _ = host;
        println!("实时窗口需要 Linux Wayland 会话。");
    }}
}}
"#
    )
    .replace("!TITLE!", name)
}

/// 设置页模板：示例一页「显示设置」，覆盖页面语法的常用块。
fn scaffold_main_settings(name: &str) -> String {
    format!(
        r#"// {name} —— 设置页模板：分区 + 行 + 相关设置，整页零手写几何。
//
// 每一行都是「标签在上、控件在中、说明在下」，间距由页面语法层负责（参
// Kanesumi 仓 docs/DEV_GUIDE.md §二·五、docs/CONTROL_SPEC.md §四十九）。
// 动作里只改状态，不碰坐标。

use kanesumi_controls::form::{{self, Form}};
use kanesumi_controls::{{FormLinkClicked, SwitchToggled}};
use kanesumi_core::{{MetroTheme, ThemeColor}};
use kanesumi_harness::element::widgets::{{Border, Stack}};
use kanesumi_harness::element::{{Action, Insets, LayoutProps, Tree, WidgetId}};
use kanesumi_harness::{{AppConfig, EtherRole, TreeApp}};

/// 页宽：内容列 600 + 左右内边距 48。
const PAGE_W: f32 = 660.0;
/// 页高。
const PAGE_H: f32 = 640.0;

struct Settings {{
    config: AppConfig,
    theme: MetroTheme,
    auto_brightness: bool,
    night: bool,
    last_link: Option<String>,
}}

impl Settings {{
    fn new() -> Self {{
        Self {{
            config: AppConfig::new("{name}", "显示设置", EtherRole::Browser, PAGE_W, PAGE_H),
            theme: MetroTheme::ether_dark(),
            auto_brightness: true,
            night: false,
            last_link: None,
        }}
    }}
}}

impl TreeApp for Settings {{
    fn config(&self) -> &AppConfig {{ &self.config }}
    fn theme(&self) -> MetroTheme {{ self.theme }}

    fn build(&mut self, tree: &mut Tree) {{
        let page = tree.insert(tree.root(), Border::new().background(ThemeColor::Background));
        let col = tree.insert_with(
            page,
            Stack::column(),
            LayoutProps {{
                width: Some(form::COL_WIDTH),
                margin: Insets::new(24.0, 20.0, 24.0, 20.0),
                ..LayoutProps::default()
            }},
        );

        let mut f = Form::new(col);
        f.page_title(tree, "显示");

        f.section(tree, "亮度", true);
        f.slider(tree, "屏幕亮度", "60%", 0.6, 0.0, 1.0, Some(0.01), None);
        f.toggle(tree, "自动亮度", self.auto_brightness, Some("按环境光调整屏幕亮度。"));

        f.section(tree, "夜间模式", false);
        f.toggle(tree, "定时开启", self.night, Some("日落后自动转为暖色。"));

        f.section(tree, "相关设置", false);
        f.link(tree, "声音设置", "sound");
        f.link(tree, "电源与睡眠", "power");
        f.button(tree, "重置本页设置", Some("只影响本页。"));

        let status = match &self.last_link {{
            Some(t) => format!("最近一次跳转：{{t}}"),
            None => "点上面的链接试试。".to_string(),
        }};
        f.note(tree, &status);
    }}

    fn on_action(&mut self, _tree: &mut Tree, _from: WidgetId, action: Action) {{
        if let Some(t) = action.downcast_ref::<SwitchToggled>() {{
            // 真实应用按行身份路由；模板里两行都翻转以演示动作通路。
            if self.auto_brightness != t.0 {{ self.auto_brightness = t.0; }} else {{ self.night = t.0; }}
        }} else if let Some(l) = action.downcast_ref::<FormLinkClicked>() {{
            self.last_link = Some(l.target.to_string());
        }}
    }}
}}

fn main() {{
    let host = kanesumi_harness::TreeHost::new(Settings::new());
    #[cfg(target_os = "linux")]
    kanesumi_harness::platform::run(Box::leak(Box::new(host)));
    #[cfg(not(target_os = "linux"))]
    {{
        let _ = host;
        println!("实时窗口需要 Linux Wayland 会话。");
    }}
}}
"#
    )
}

/// 名字 → 包名片段：小写、非字母数字换成连字符、去首尾连字符。
fn slugify(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let s = out.trim_matches('-').to_string();
    if s.is_empty() {
        "app".to_string()
    } else if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("app-{s}")
    } else {
        s
    }
}

// ── check / build / run ─────────────────────────────────────────────────────

fn cmd_check() -> Result<(), String> {
    let p = load_project()?;
    println!("项目 {}", p.app_id);
    println!("  名称      {} {}", p.name, p.version);
    println!("  deb       {} ({})", p.deb_version(), p.architecture);
    println!("  角色      {}", p.role);
    println!("  入口      usr/bin/{}", p.binary);
    println!("  manifest  usr/share/ether/manifests/{}.toml", p.app_id);

    // 与 Cargo.toml 的 `[[bin]]` 对账：两边名字不一致时 `package` 会「读不到二进制」，
    // 那个报错发生在构建之后，代价高；在这里提前说清楚。
    if let Ok(text) = std::fs::read_to_string("Cargo.toml")
        && let Ok(doc) = toml_lite::parse(&text)
    {
        let bins: Vec<String> = doc
            .keys()
            .filter(|k| k.starts_with("bin.") && k.ends_with(".name"))
            .filter_map(|k| doc.get(k).and_then(toml_lite::Value::as_str).map(str::to_string))
            .collect();
        if !bins.is_empty() && !bins.iter().any(|b| b == &p.binary) {
            println!(
                "  警告      [entry] binary = {} 不在 Cargo.toml 的 [[bin]] 里（实有 {}）",
                p.binary,
                bins.join(", ")
            );
            println!("            两边名字必须一致，否则 package 读不到产物。");
        }
    }
    Ok(())
}

fn cmd_build(args: &[String]) -> Result<(), String> {
    let release = args.iter().any(|a| a == "--release");
    cargo_build(release)
}

fn cmd_run() -> Result<(), String> {
    let status = Command::new("cargo")
        .arg("run")
        .status()
        .map_err(|e| format!("起 cargo run 失败：{e}"))?;
    if !status.success() {
        return Err(format!("cargo run 退出码 {:?}", status.code()));
    }
    Ok(())
}

fn cargo_build(release: bool) -> Result<(), String> {
    let mut c = Command::new("cargo");
    c.arg("build");
    if release {
        c.arg("--release");
    }
    let status = c.status().map_err(|e| format!("起 cargo build 失败：{e}"))?;
    if !status.success() {
        return Err(format!("cargo build 退出码 {:?}", status.code()));
    }
    Ok(())
}

// ── package ─────────────────────────────────────────────────────────────────

fn cmd_package(args: &[String]) -> Result<(), String> {
    let mut no_build = false;
    let mut out_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--no-build" => {
                no_build = true;
                i += 1;
            }
            "--out" => {
                out_dir = Some(PathBuf::from(take_value(args, i, "--out")?));
                i += 2;
            }
            other => return Err(format!("未知选项 `{other}`")),
        }
    }

    let p = load_project()?;
    let root = std::env::current_dir().map_err(|e| format!("取当前目录失败：{e}"))?;

    if !no_build {
        cargo_build(true)?;
    }
    let bin_path = root.join("target").join("release").join(&p.binary);
    let bin = std::fs::read(&bin_path).map_err(|e| {
        format!(
            "读不到二进制 {}：{e}\n先 `kanesumi build --release`，或确认 Cargo.toml 的 [[bin]] name 与 [entry] binary 一致",
            bin_path.display()
        )
    })?;

    // data 树：入口二进制 + manifest + .desktop +（可选）图标。
    let mut files = vec![
        deb::DebFile { path: format!("usr/bin/{}", p.binary), data: bin, mode: 0o755 },
        deb::DebFile {
            path: format!("usr/share/ether/manifests/{}.toml", p.app_id),
            data: p.manifest_text().into_bytes(),
            mode: 0o644,
        },
        deb::DebFile {
            path: format!("usr/share/applications/{}.desktop", p.app_id),
            data: p.desktop_text().into_bytes(),
            mode: 0o644,
        },
    ];
    let icon_path = root.join("assets").join(format!("{}.svg", p.icon));
    if icon_path.exists() {
        let data = std::fs::read(&icon_path).map_err(|e| format!("读图标失败：{e}"))?;
        files.push(deb::DebFile {
            path: format!("usr/share/icons/hicolor/scalable/apps/{}.svg", p.icon),
            data,
            mode: 0o644,
        });
    }

    let installed_kb = (files.iter().map(|f| f.data.len()).sum::<usize>() as u64).div_ceil(1024);
    let control = p.control_text(installed_kb);
    let deb = deb::build(&control, &files)?;

    let out_dir = out_dir.unwrap_or_else(|| root.join("dist"));
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("建输出目录失败：{e}"))?;
    let out = out_dir.join(p.deb_file_name());
    std::fs::write(&out, &deb).map_err(|e| format!("写 {} 失败：{e}", out.display()))?;

    println!("debian 包 {}", out.display());
    println!("  版本     {}", p.deb_version());
    println!("  架构     {}", p.architecture);
    println!("  装机大小 {installed_kb} KiB（{} 个文件）", files.len());
    Ok(())
}

// ── 共用 ────────────────────────────────────────────────────────────────────

fn load_project() -> Result<Project, String> {
    let path = Path::new(PROJECT_FILE);
    let text = std::fs::read_to_string(path).map_err(|e| {
        format!("当前目录没有 {PROJECT_FILE}（{e}）；先 `kanesumi new <名字>` 或 cd 进项目目录")
    })?;
    Project::from_text(&text)
}

fn take_value(args: &[String], i: usize, flag: &str) -> Result<String, String> {
    args.get(i + 1).cloned().ok_or_else(|| format!("{flag} 后面要跟一个值"))
}

#[cfg(test)]
mod tests {
    use super::slugify;

    #[test]
    fn slugify_makes_a_debian_safe_fragment() {
        assert_eq!(slugify("My App"), "my-app");
        assert_eq!(slugify("计算器"), "app");
        assert_eq!(slugify("3D 视图"), "app-3d");
        assert_eq!(slugify("--weird--name--"), "weird-name");
    }
}
