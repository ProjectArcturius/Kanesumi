// system_theme.rs —— 系统主题来源（Ether 的 Chorus）。
//
// **真源分工**：Chorus（`Ether/chorus/src/theme.rs`）拥有主题状态并落盘
// `~/.config/ether/theme.toml`（`accent` / `scheme`）；Kanesumi 只负责读懂它并转成
// `MetroTheme` 供渲染层消费。
//
// 此前 Kanesumi 完全不读这个文件：用户在 Chorus 把 accent 改成青绿，全部应用仍是写死的橙色。
// 本模块是那条断掉链路的接回点。
//
// 解析器与 Chorus 同款容错：非法 accent 回退默认、未知 scheme 回退浅色、未知键忽略。
// 不引入 toml 依赖（与 Chorus 保持一致，且 harness 侧依赖越少越好）。
// 浅色为系统默认（裁定 Ether docs/DECISIONS_2026-10-04.md §M-87、88）。

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::SystemTime;

use kanesumi_core::{Accent, ColorScheme, MetroTheme};

// ── 进程级固定主题（测试夹具）──
//
// 由来：迁移快照按宿主 `~/.config/ether/theme.toml` 出图，同一份代码在录制机上绿、
// 在未改一行的另一台机器上整片红（Ether `docs/research/m1115_dock/REPORT.md` §二）。
// 夹具要的是「这份测试规定主题」，不是「这台机器恰好是什么主题」。
//
// 为什么是进程级、不是线程局部：主题由外壳在启动时读一次推给 App，之后渲染 / 工作线程
// 还可能各自按需读取；线程局部的固定值只在设置它的线程上生效，spawn 出去的线程仍读到
// 宿主配置 —— 那正是要消除的不确定性。固定值只由测试显式设置与清除，生产进程不 pin。
static PINNED: Mutex<Option<MetroTheme>> = Mutex::new(None);

fn pinned_lock() -> MutexGuard<'static, Option<MetroTheme>> {
    // 中毒不传染：某个用例 panic 不该让后续 `load()` 读到「永远拿不到锁」。
    // 固定值本身是 `Copy`，锁内不跑用户代码，中毒只可能是外部持锁时 panic。
    PINNED.lock().unwrap_or_else(|e| e.into_inner())
}

/// 固定进程主题：此后 [`load`] 直接返回它，不读盘、不看 `ETHER_THEME_CONFIG`。
///
/// **进程级、非线程局部**（理由见上）；**仅测试用**，生产进程不得调用。
/// 只影响主题读取，不影响 [`load_backdrop`] / [`fingerprint`] 等其他读盘入口。
/// 固定期间再次调用即改值；解除用 [`unpin`]。
pub fn pin(theme: MetroTheme) {
    *pinned_lock() = Some(theme);
}

/// 解除固定，恢复「读盘 / `ETHER_THEME_CONFIG`」的旧行为。未固定时调用无副作用。
pub fn unpin() {
    *pinned_lock() = None;
}

/// 当前固定值（未固定 = `None`）。给测试断言用。
pub fn pinned() -> Option<MetroTheme> {
    *pinned_lock()
}

/// `theme.toml` 路径。`ETHER_THEME_CONFIG` 可覆盖（测试与多实例用）。
pub fn config_path() -> PathBuf {
    std::env::var_os("ETHER_THEME_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config/ether/theme.toml")
        })
}

/// 读取系统主题。文件缺失 / 不可读 → 默认（浅色 + 默认强调色，裁定 §M-87、88）。
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

/// 读取系统主题。固定期间（[`pin`]）直接返回固定值：不读盘、不看 `ETHER_THEME_CONFIG`，
/// 也不受 `theme.toml` 变更影响 —— 夹具要的是可复现，不是跟随宿主。
pub fn load() -> MetroTheme {
    if let Some(theme) = pinned() {
        return theme;
    }
    match std::fs::read_to_string(config_path()) {
        Ok(raw) => parse(&raw),
        Err(_) => MetroTheme::light(Accent::default()),
    }
}

/// 解析主题文本（容错，未知键忽略）。
pub fn parse(raw: &str) -> MetroTheme {
    let mut accent = Accent::default();
    let mut scheme = ColorScheme::Light;
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
            "scheme" => scheme = ColorScheme::parse(v).unwrap_or(ColorScheme::Light),
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
    fn defaults_to_light_with_default_accent() {
        let t = parse("");
        assert_eq!(t.scheme, ColorScheme::Light);
        assert_eq!(t.colors.primary, Accent::default().base);
    }

    #[test]
    fn invalid_values_fall_back_without_panicking() {
        let t = parse("accent = \"zzz\"\nscheme = \"neon\"\nunknown = 1\n# comment\n");
        assert_eq!(t.scheme, ColorScheme::Light);
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

    // ── 进程级固定主题（pin）──
    //
    // 固定值与 `ETHER_THEME_CONFIG` 都是进程级的：本组用例彼此串行，且用守卫在 panic 时
    // 也还原环境变量与固定值，免得污染同进程的其他用例。

    use std::sync::Mutex as StdMutex;

    static PIN_TEST_LOCK: StdMutex<()> = StdMutex::new(());

    const CONFIG_ENV: &str = "ETHER_THEME_CONFIG";

    /// 还原固定值、环境变量与临时文件。
    struct PinGuard {
        saved: Option<std::ffi::OsString>,
        files: Vec<PathBuf>,
    }

    impl PinGuard {
        fn new() -> Self {
            unpin();
            Self {
                saved: std::env::var_os(CONFIG_ENV),
                files: Vec::new(),
            }
        }

        /// 写一份临时 theme.toml 并把 `ETHER_THEME_CONFIG` 指过去。
        fn point_at(&mut self, tag: &str, raw: &str) -> PathBuf {
            let path = std::env::temp_dir()
                .join(format!("kanesumi-theme-{}-{tag}.toml", std::process::id()));
            std::fs::write(&path, raw).expect("写临时 theme.toml");
            self.files.push(path.clone());
            // 本组用例由 PIN_TEST_LOCK 串行；改的键 ETHER_THEME_CONFIG 只有本模块测试
            // 读取（生产路径按默认配置源读，不读该键）。**残余前提**：PIN_TEST_LOCK 只
            // 串行本模块三个用例，挡不住同进程其它测试线程的 `env::var`（任意键）；
            // libc environ 级别仍有竞争，见 docs/UNSAFE_AUDIT.md §疑点-4。
            // SAFETY: 键只被本模块测试读、用例被锁串行；libc 级并发前提未满足，已记疑点。
            unsafe { std::env::set_var(CONFIG_ENV, &path) };
            path
        }

        /// 把 `ETHER_THEME_CONFIG` 指向一个**不存在**的路径。
        fn point_at_missing(&mut self, tag: &str) -> PathBuf {
            let path = std::env::temp_dir()
                .join(format!("kanesumi-theme-{}-{tag}.toml", std::process::id()));
            let _ = std::fs::remove_file(&path);
            // SAFETY: 同上；仍持有 PIN_TEST_LOCK，键无并发读者。
            unsafe { std::env::set_var(CONFIG_ENV, &path) };
            path
        }
    }

    impl Drop for PinGuard {
        fn drop(&mut self) {
            unpin();
            // SAFETY: 同上；PIN_TEST_LOCK 仍由本用例持有，守卫在用例结束时
            // （含 panic 展开）把键恢复为保存值，期间无并发读者。
            unsafe {
                match self.saved.take() {
                    Some(v) => std::env::set_var(CONFIG_ENV, v),
                    None => std::env::remove_var(CONFIG_ENV),
                }
            }
            for f in &self.files {
                let _ = std::fs::remove_file(f);
            }
        }
    }

    fn serial_lock() -> std::sync::MutexGuard<'static, ()> {
        PIN_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 固定后 `load()` 不碰配置：`ETHER_THEME_CONFIG` 指向不存在的路径也返回固定值，
    /// 而同一路径未固定时读盘失败会回退浅色默认 —— 两者不同才证明短路生效。
    #[test]
    fn pin_wins_over_missing_config_path() {
        let _serial = serial_lock();
        let mut guard = PinGuard::new();
        let missing = guard.point_at_missing("missing");
        assert!(!missing.exists(), "夹具路径不该存在：{}", missing.display());

        let fallback = load();
        assert_eq!(fallback, MetroTheme::light(Accent::default()), "未固定 → 读盘失败回退浅色");

        let pinned_theme = MetroTheme::dark(Accent::parse("00897B"));
        pin(pinned_theme);
        assert_eq!(pinned(), Some(pinned_theme));
        assert_eq!(load(), pinned_theme, "固定期间即使配置路径不存在也返回固定值");
    }

    /// 解除固定后恢复「读 `ETHER_THEME_CONFIG` 指向的文件」的旧行为。
    #[test]
    fn unpin_restores_config_loading() {
        let _serial = serial_lock();
        let mut guard = PinGuard::new();
        let raw = "accent = \"00897B\"\nscheme = \"dark\"\n";
        let path = guard.point_at("restore", raw);
        let on_disk = parse(raw);
        assert_eq!(on_disk.scheme, ColorScheme::Dark);

        pin(MetroTheme::light(Accent::default()));
        assert_eq!(load().scheme, ColorScheme::Light, "固定期间不读盘");

        unpin();
        assert_eq!(pinned(), None);
        assert_eq!(load(), on_disk, "unpin 后回到盘上的主题");
        assert!(path.exists(), "夹具文件应在用例结束时才删");
    }

    /// 暗色固定主题下 `colors.surface` 必须是暗色值，不能是宿主亮色值（用断言，不打印）。
    /// 这正是 m-dock-c1 换机整片红的那个量：宿主亮色 `surface` = 纯白。
    #[test]
    fn dark_pin_surface_is_not_host_light_surface() {
        let _serial = serial_lock();
        let mut guard = PinGuard::new();
        guard.point_at("host-light", "scheme = \"light\"\n");
        let host_light = load();
        assert_eq!(host_light.scheme, ColorScheme::Light);

        let dark = MetroTheme::dark(Accent::default());
        pin(dark);
        let got = load();
        assert_eq!(got.colors.surface, dark.colors.surface);
        assert_ne!(
            got.colors.surface, host_light.colors.surface,
            "暗色固定下 surface 不得是宿主亮色值"
        );
    }
}
