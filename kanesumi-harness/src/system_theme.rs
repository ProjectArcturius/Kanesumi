// system_theme.rs —— 系统主题来源（Ether 的 Chorus）。
//
// **真源分工**：Chorus（`Ether/chorus/src/theme.rs`）拥有主题状态并落盘
// `~/.config/ether/theme.toml`（`accent` / `scheme`）；Kanesumi 只负责读懂它并转成
// `MetroTheme` 供渲染层消费。
//
// 此前 Kanesumi 完全不读这个文件：用户在 Chorus 把 accent 改成青绿，全部应用仍是写死的橙色。
// 本模块是那条断掉链路的接回点。
//
// 解析器与 Chorus 同款容错：非法 accent 回退默认、未知 scheme 回退暗色、未知键忽略。
// 不引入 toml 依赖（与 Chorus 保持一致，且 harness 侧依赖越少越好）。

use std::path::PathBuf;
use std::time::SystemTime;

use kanesumi_core::{Accent, ColorScheme, MetroTheme};

/// `theme.toml` 路径。`ETHER_THEME_CONFIG` 可覆盖（测试与多实例用）。
pub fn config_path() -> PathBuf {
    std::env::var_os("ETHER_THEME_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config/ether/theme.toml")
        })
}

/// 读取系统主题。文件缺失 / 不可读 → 默认（暗色 + 默认 accent）。
pub fn load() -> MetroTheme {
    match std::fs::read_to_string(config_path()) {
        Ok(raw) => parse(&raw),
        Err(_) => MetroTheme::dark(Accent::default()),
    }
}

/// 解析主题文本（容错，未知键忽略）。
pub fn parse(raw: &str) -> MetroTheme {
    let mut accent = Accent::default();
    let mut scheme = ColorScheme::Dark;
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        match k {
            "accent" => accent = Accent::parse(v),
            "scheme" => scheme = ColorScheme::parse(v).unwrap_or(ColorScheme::Dark),
            _ => {}
        }
    }
    MetroTheme::for_scheme(scheme, accent)
}

/// 配置文件指纹（mtime）—— 供外壳判断是否需要重新加载。
pub fn fingerprint() -> Option<SystemTime> {
    std::fs::metadata(config_path())
        .ok()
        .and_then(|m| m.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanesumi_core::Color;

    #[test]
    fn parses_accent_and_scheme() {
        let t = parse("accent = \"00897B\"\nscheme = \"light\"\n");
        assert_eq!(t.scheme, ColorScheme::Light);
        assert_eq!(t.colors.primary, Color::from_hex(0x00_89_7B));
        assert_eq!(t.accent.base, Color::from_hex(0x00_89_7B));
    }

    #[test]
    fn defaults_to_dark_with_default_accent() {
        let t = parse("");
        assert_eq!(t.scheme, ColorScheme::Dark);
        assert_eq!(t.colors.primary, Accent::default().base);
    }

    #[test]
    fn invalid_values_fall_back_without_panicking() {
        let t = parse("accent = \"zzz\"\nscheme = \"neon\"\nunknown = 1\n# comment\n");
        assert_eq!(t.scheme, ColorScheme::Dark);
        assert_eq!(t.colors.primary, Accent::default().base);
    }

    #[test]
    fn ignores_malformed_lines() {
        let t = parse("no_equals_sign\naccent = \"0078D7\"\n");
        assert_eq!(t.colors.primary, Color::from_hex(0x00_78_D7));
    }

    /// 端到端守卫：用户在 Chorus 改 accent → MetroTheme 的主色必须随之改变。
    /// 这正是「改了配置、应用仍橙色」的回归点。
    #[test]
    fn config_change_reaches_primary_token() {
        let orange = parse("accent = \"E57812\"\n");
        let teal = parse("accent = \"00897B\"\n");
        assert_ne!(orange.colors.primary, teal.colors.primary);
    }
}
