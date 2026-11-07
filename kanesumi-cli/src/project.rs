//! project.rs —— `Kanesumi.toml`（应用项目文件）的读取、校验与产物文本。
//!
//! 一个项目文件同时是两样东西的来源：
//! - **deb 侧**（`[package]`）：包名、版本、修订号、架构、维护者、apt 依赖；
//! - **运行时侧**（`[identity]` / `[role]` / `[entry]` / `[capabilities]` / `[presentation]`）：
//!   即 `/usr/share/ether/manifests/<app_id>.toml` 的内容（schema 参 `docs/PACKAGING_PLAN.md` §三）。
//!
//! 拆成两个文件只会漂移，故合并：`[package]` 之外的段原样写进 manifest。

use crate::toml_lite;

/// 一个应用项目。
#[derive(Debug, Clone)]
pub struct Project {
    /// deb 包名 = `identity.app_id`（Debian 允许的字符集与 app_id 相容）。
    pub app_id: String,
    /// 显示名。
    pub name: String,
    /// 应用版本（三节语义版本）。
    pub version: String,
    /// deb 修订号。
    pub revision: u32,
    /// deb 架构（`amd64` / `arm64`）。
    pub architecture: String,
    /// 维护者（`名字 <邮箱>`）。
    pub maintainer: String,
    /// apt 级依赖。
    pub depends: Vec<String>,
    /// 最小 Kanesumi Runtime 版本。
    pub min_runtime: String,
    /// 表面类型（`browser` / `desktop` / `topbar` / `dock` / `launcher`）。
    pub role: String,
    /// 入口可执行文件名。
    pub binary: String,
    /// 图标名（图标主题内的 SVG 名，缺省 = app_id）。
    pub icon: String,
    /// 一句话摘要。
    pub summary: String,
    /// 详细描述（可多行）。
    pub description: String,
    /// `[capabilities]` 段原样保留的行（`键 = 值`），写回 manifest。
    pub capabilities: Vec<String>,
}

impl Project {
    /// 从 `Kanesumi.toml` 文本解析。
    pub fn from_text(text: &str) -> Result<Self, String> {
        let d = toml_lite::parse(text)?;

        let app_id = d.str_or("identity.app_id", "");
        let name = d.str_or("identity.name", "");
        let version = d.str_or("identity.version", "");
        let binary = d.str_or("entry.binary", "");
        if app_id.is_empty() {
            return Err("缺少 `[identity] app_id`（形如 org.example.app）".into());
        }
        if name.is_empty() {
            return Err("缺少 `[identity] name`（显示名）".into());
        }
        if version.is_empty() {
            return Err("缺少 `[identity] version`（应用版本，形如 0.1.0）".into());
        }
        if binary.is_empty() {
            return Err("缺少 `[entry] binary`（安装到 usr/bin 的可执行名）".into());
        }
        validate_app_id(&app_id)?;
        validate_version(&version)?;

        let role = d.str_or("role.kind", "browser");
        if !["browser", "desktop", "topbar", "dock", "launcher"].contains(&role.as_str()) {
            return Err(format!(
                "`[role] kind` 只能是 browser / desktop / topbar / dock / launcher，得到 {role}"
            ));
        }

        // `[capabilities]` 整段原样带进 manifest：能力表会随 runtime 增长，工具链不写死字段。
        let capabilities = d
            .keys()
            .filter(|k| k.starts_with("capabilities."))
            .filter_map(|k| {
                let short = k.trim_start_matches("capabilities.");
                d.get(k).map(|v| format!("{short} = {}", render_value(v)))
            })
            .collect::<Vec<_>>();

        let icon = {
            let v = d.str_or("presentation.icon", "");
            if v.is_empty() { app_id.clone() } else { v }
        };

        let summary_default = name.clone();
        Ok(Self {
            app_id,
            name,
            version,
            revision: match d.get("package.revision") {
                Some(toml_lite::Value::Int(i)) if *i > 0 => *i as u32,
                Some(_) => return Err("`[package] revision` 必须是正整数".into()),
                None => 1,
            },
            architecture: d.str_or("package.architecture", "amd64"),
            maintainer: d.str_or(
                "package.maintainer",
                "Kanesumi Maintainers <maintainers@example.invalid>",
            ),
            depends: d.list_or("package.depends"),
            min_runtime: d.str_or("identity.min_runtime", "0.1.0"),
            role,
            binary,
            icon,
            summary: d.str_or("presentation.summary", summary_default.as_str()),
            description: d.str_or("presentation.description", summary_default.as_str()),
            capabilities,
        })
    }

    /// deb 版本号：`应用版本-修订号`（参 `docs/PACKAGING_PLAN.md` §三·版本语义）。
    pub fn deb_version(&self) -> String {
        format!("{}-{}", self.version, self.revision)
    }

    /// 产物文件名：`<app_id>_<deb 版本>_<架构>.deb`。
    pub fn deb_file_name(&self) -> String {
        format!("{}_{}_{}.deb", self.app_id, self.deb_version(), self.architecture)
    }

    /// `usr/share/ether/manifests/<app_id>.toml` 的内容。
    pub fn manifest_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!("# {}.toml —— 由 `kanesumi package` 从 Kanesumi.toml 生成，勿手改。\n\n", self.app_id));
        s.push_str("[identity]\n");
        s.push_str(&format!("app_id      = \"{}\"\n", self.app_id));
        s.push_str(&format!("name        = \"{}\"\n", self.name));
        s.push_str(&format!("version     = \"{}\"\n", self.version));
        s.push_str(&format!("min_runtime = \"{}\"\n\n", self.min_runtime));
        s.push_str("[role]\n");
        s.push_str(&format!("kind = \"{}\"\n\n", self.role));
        s.push_str("[entry]\n");
        s.push_str(&format!("binary = \"{}\"\n\n", self.binary));
        if !self.capabilities.is_empty() {
            s.push_str("[capabilities]\n");
            for line in &self.capabilities {
                s.push_str(&format!("{line}\n"));
            }
            s.push('\n');
        }
        s.push_str("[presentation]\n");
        s.push_str(&format!("icon    = \"{}\"\n", self.icon));
        s.push_str(&format!("summary = \"{}\"\n", self.summary));
        s.push_str(&format!("description = \"{}\"\n", self.description));
        s
    }

    /// `usr/share/applications/<app_id>.desktop` 的内容（非 Ether DE 的发现桥接）。
    pub fn desktop_text(&self) -> String {
        format!(
            "[Desktop Entry]\nType=Application\nName={}\nExec={}\nIcon={}\nComment={}\nCategories=Utility;\nX-Kanesumi-AppId={}\n",
            self.name, self.binary, self.icon, self.summary, self.app_id
        )
    }

    /// `DEBIAN/control` 的内容。`installed_kb` 是安装树占用的 KiB（对齐 `Installed-Size`）。
    pub fn control_text(&self, installed_kb: u64) -> String {
        let mut s = String::new();
        s.push_str(&format!("Package: {}\n", self.app_id));
        s.push_str(&format!("Version: {}\n", self.deb_version()));
        s.push_str(&format!("Architecture: {}\n", self.architecture));
        s.push_str(&format!("Maintainer: {}\n", self.maintainer));
        s.push_str("Section: x11\nPriority: optional\n");
        s.push_str(&format!("Installed-Size: {installed_kb}\n"));
        if !self.depends.is_empty() {
            s.push_str(&format!("Depends: {}\n", self.depends.join(", ")));
        }
        s.push_str(&format!("Description: {}\n", self.summary));
        for line in self.description.lines() {
            s.push_str(&format!(" {line}\n"));
        }
        s
    }
}

/// 值按 TOML 字面量渲染（写回 manifest 用）。
fn render_value(v: &toml_lite::Value) -> String {
    match v {
        toml_lite::Value::Str(s) => format!("\"{s}\""),
        toml_lite::Value::Int(i) => i.to_string(),
        toml_lite::Value::Float(f) => f.to_string(),
        toml_lite::Value::Bool(b) => b.to_string(),
        toml_lite::Value::List(items) => {
            let inner: Vec<String> = items.iter().map(|i| format!("\"{i}\"")).collect();
            format!("[{}]", inner.join(", "))
        }
    }
}

/// app_id 校验：反域名，至少两段，只允许小写字母 / 数字 / 点 / 连字符。
pub fn validate_app_id(id: &str) -> Result<(), String> {
    if id.split('.').count() < 2 {
        return Err(format!("app_id 必须是反域名（至少两段），得到 {id}"));
    }
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-') {
        return Err(format!("app_id 只允许小写字母、数字、点和连字符，得到 {id}"));
    }
    if id.starts_with('.') || id.ends_with('.') || id.contains("..") {
        return Err(format!("app_id 的点位不合法，得到 {id}"));
    }
    Ok(())
}

/// 版本校验：三节 `major.minor.patch`，每节数字。
pub fn validate_version(v: &str) -> Result<(), String> {
    let parts: Vec<&str> = v.split('.').collect();
    if parts.len() != 3 || !parts.iter().all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())) {
        return Err(format!("版本必须是三节数字（如 0.1.0），得到 {v}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[package]
revision = 2
architecture = "arm64"
maintainer = "Rinta <r@example.org>"
depends = ["libc6", "fonts-noto-cjk"]

[identity]
app_id = "org.example.demo"
name = "Demo"
version = "1.2.3"
min_runtime = "0.1.0"

[role]
kind = "browser"

[entry]
binary = "kanesumi-demo"

[capabilities]
notifications = true
storage = "read-write"

[presentation]
summary = "演示应用"
description = "第一行\n第二行"
"#;

    #[test]
    fn parses_project_file() {
        let p = Project::from_text(SAMPLE).unwrap();
        assert_eq!(p.app_id, "org.example.demo");
        assert_eq!(p.deb_version(), "1.2.3-2");
        assert_eq!(p.deb_file_name(), "org.example.demo_1.2.3-2_arm64.deb");
        assert_eq!(p.icon, "org.example.demo", "图标缺省取 app_id");
        assert_eq!(p.depends, vec!["libc6", "fonts-noto-cjk"]);
    }

    #[test]
    fn manifest_carries_runtime_sections_verbatim() {
        let p = Project::from_text(SAMPLE).unwrap();
        let m = p.manifest_text();
        assert!(m.contains("app_id      = \"org.example.demo\""));
        assert!(m.contains("kind = \"browser\""));
        assert!(m.contains("binary = \"kanesumi-demo\""));
        assert!(m.contains("notifications = true"), "能力表原样带过去");
        assert!(m.contains("storage = \"read-write\""));
        assert!(!m.contains("revision"), "deb 侧字段不进 manifest");
    }

    #[test]
    fn control_has_debian_required_fields() {
        let p = Project::from_text(SAMPLE).unwrap();
        let c = p.control_text(42);
        for key in ["Package: org.example.demo", "Version: 1.2.3-2", "Architecture: arm64", "Installed-Size: 42"] {
            assert!(c.contains(key), "control 缺 {key}：\n{c}");
        }
        assert!(c.contains("Depends: libc6, fonts-noto-cjk"));
        // 多行描述：续行以一个空格开头（Debian 约定）。
        assert!(c.contains("\n 第二行\n"), "描述续行需缩进一格：\n{c}");
    }

    #[test]
    fn rejects_bad_identity() {
        let bad_id = SAMPLE.replace("org.example.demo", "demo");
        assert!(Project::from_text(&bad_id).unwrap_err().contains("反域名"));
        let bad_ver = SAMPLE.replace("version = \"1.2.3\"", "version = \"1.2\"");
        assert!(Project::from_text(&bad_ver).unwrap_err().contains("三节"));
        let bad_role = SAMPLE.replace("kind = \"browser\"", "kind = \"flying\"");
        assert!(Project::from_text(&bad_role).unwrap_err().contains("browser"));
    }
}
