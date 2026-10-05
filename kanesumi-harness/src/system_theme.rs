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
/// 亚克力背板设置（壁纸 + 模糊 + 着色），与主题同一份 theme.toml。
#[derive(Debug, Clone, PartialEq)]
pub struct Backdrop {
    pub wallpaper: Option<std::path::PathBuf>,
    /// 模糊半径（逻辑像素，0 = 清晰）。
    pub blur: f32,
    /// 着色强度 0..1。
    pub tint: f32,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self { wallpaper: None, blur: 24.0, tint: 0.5 }
    }
}

/// 读背板设置（文件缺失 → 默认：无壁纸）。
pub fn load_backdrop() -> Backdrop {
    std::fs::read_to_string(config_path()).map(|r| parse_backdrop(&r)).unwrap_or_default()
}

/// 读取背板壁纸并解码为 RGBA（按扩展名分派 PNG / JPEG / SVG，参 `rasterize_image`）。
/// 未配置壁纸或解码失败 → None（渲染回退纯色底，不 panic）。
/// 这是壁纸「只解 PNG」断链的接回点：真实壁纸为 JPEG（磨砂/光窗/暮色）与 SVG（花窗等）。
/// 背板是全屏用法，无缩略图目标 → `target` 传 None。
pub fn load_wallpaper() -> Option<kanesumi_canvas::Icon> {
    load_wallpaper_from(&load_backdrop())
}

/// 从给定背板设置解码壁纸（便于测试 / 调用方复用已解析的设置）。
pub fn load_wallpaper_from(backdrop: &Backdrop) -> Option<kanesumi_canvas::Icon> {
    let path = backdrop.wallpaper.as_ref()?;
    kanesumi_canvas::rasterize_image(path, None)
}

pub fn parse_backdrop(raw: &str) -> Backdrop {
    let mut b = Backdrop::default();
    for line in raw.lines() {
        let line = line.trim();
        let Some((k, v)) = line.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim().trim_matches('"'));
        match k {
            "wallpaper" => b.wallpaper = (!v.is_empty()).then(|| std::path::PathBuf::from(v)),
            "acrylic_blur" => {
                if let Ok(x) = v.parse::<f32>() {
                    b.blur = if x.is_finite() { x.clamp(0.0, 120.0) } else { b.blur };
                }
            }
            "acrylic_tint" => {
                if let Ok(x) = v.parse::<f32>() {
                    b.tint = x.clamp(0.0, 1.0);
                }
            }
            _ => {}
        }
    }
    b
}

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
    fn parses_backdrop() {
        let b = parse_backdrop("wallpaper = \"/w.png\"\nacrylic_blur = 0\nacrylic_tint = 0.2\n");
        assert_eq!(b.wallpaper, Some(std::path::PathBuf::from("/w.png")));
        assert_eq!((b.blur, b.tint), (0.0, 0.2));
        assert_eq!(parse_backdrop("").blur, 24.0, "缺省 24");
        assert_eq!(parse_backdrop("acrylic_blur = 9999").blur, 120.0, "夹上限");
    }

    /// 壁纸解码：JPEG 也能读（旧路径只解 PNG）。用仓内 JPEG 夹具，不依赖环境变量。
    #[test]
    fn loads_jpeg_wallpaper_by_magic() {
        let jpg = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../kanesumi-canvas/assets/test_icon.jpg");
        let b = Backdrop {
            wallpaper: Some(jpg),
            blur: 0.0,
            tint: 0.0,
        };
        let icon = load_wallpaper_from(&b).expect("JPEG 壁纸应解码");
        assert_eq!((icon.width, icon.height), (16, 16));
        assert!(
            load_wallpaper_from(&Backdrop::default()).is_none(),
            "无壁纸 → None"
        );
    }

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
