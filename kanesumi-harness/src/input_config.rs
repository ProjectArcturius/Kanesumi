// input_config.rs —— 交互数值的用户覆盖来源（`~/.config/ether/input.toml`）。
//
// 正典 §Ⅰ.2：用户可调项（双击时长 / 滚轮行数 / 提示延迟）由 Settings 写入该文件，
// harness 运行时读取并覆盖 `kanesumi_core::InteractionSettings` 的默认值。
//
// 解析器容错与 `system_theme.rs` 同款：手写行式、无 toml 依赖、未知键忽略。
// 文件不存在 / 字段缺失 / 非法值 → 用默认值并记日志，绝不 panic。

use std::path::PathBuf;
use std::time::SystemTime;

use kanesumi_core::InteractionSettings;

/// `input.toml` 路径。`ETHER_INPUT_CONFIG` 可覆盖（测试与多实例用）。
pub fn config_path() -> PathBuf {
    std::env::var_os("ETHER_INPUT_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let home = std::env::var_os("HOME").unwrap_or_default();
            PathBuf::from(home).join(".config/ether/input.toml")
        })
}

/// 读取交互设置。文件缺失 / 不可读 → 默认（正典值），记 debug 日志。
pub fn load() -> InteractionSettings {
    match std::fs::read_to_string(config_path()) {
        Ok(raw) => parse(&raw),
        Err(_) => {
            log::debug!("input.toml 不可读，交互数值用正典默认值");
            InteractionSettings::default()
        }
    }
}

/// 解析交互设置文本（容错，未知键忽略，非法值回退默认并 warn）。
pub fn parse(raw: &str) -> InteractionSettings {
    let mut s = InteractionSettings::default();
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
            "double_click_ms" => match v.parse::<u32>() {
                Ok(x) if (1..=2000).contains(&x) => s.double_click_ms = x,
                _ => log::warn!(
                    "input.toml: double_click_ms 非法（{v}），用默认 {}",
                    s.double_click_ms
                ),
            },
            "wheel_lines" => match v.parse::<u32>() {
                Ok(x) if (1..=20).contains(&x) => s.wheel_lines = x,
                _ => log::warn!(
                    "input.toml: wheel_lines 非法（{v}），用默认 {}",
                    s.wheel_lines
                ),
            },
            "tooltip_delay_ms" => match v.parse::<u64>() {
                Ok(x) if (1..=10_000).contains(&x) => s.tooltip_delay_ms = x,
                _ => log::warn!(
                    "input.toml: tooltip_delay_ms 非法（{v}），用默认 {}",
                    s.tooltip_delay_ms
                ),
            },
            _ => {}
        }
    }
    s
}

/// 配置文件指纹（mtime）—— 供外壳判断是否需要重新加载（与 `system_theme` 同款）。
pub fn fingerprint() -> Option<SystemTime> {
    std::fs::metadata(config_path())
        .ok()
        .and_then(|m| m.modified().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_overridable_fields() {
        let s = parse("double_click_ms = 350\nwheel_lines = 5\ntooltip_delay_ms = 800\n");
        assert_eq!(s.double_click_ms, 350);
        assert_eq!(s.wheel_lines, 5);
        assert_eq!(s.tooltip_delay_ms, 800);
        assert_eq!(s.wheel_step_px(), 80.0, "5 行 × 16 = 80");
    }

    #[test]
    fn missing_fields_keep_defaults() {
        let s = parse("wheel_lines = 4\n");
        assert_eq!(s.double_click_ms, 500, "缺字段用默认");
        assert_eq!(s.wheel_lines, 4);
        assert_eq!(s.tooltip_delay_ms, 500);
    }

    #[test]
    fn invalid_values_fall_back_without_panicking() {
        let s = parse("double_click_ms = abc\nwheel_lines = 0\ntooltip_delay_ms = -1\n");
        assert_eq!(s, InteractionSettings::default(), "全部非法 → 全默认");
        let s2 = parse("wheel_lines = 999\n");
        assert_eq!(s2.wheel_lines, 3, "超范围回退默认");
    }

    #[test]
    fn ignores_comments_and_malformed_lines() {
        let s = parse("# 注释\nno_equals\nwheel_lines = 3\nunknown = 1\n");
        assert_eq!(s, InteractionSettings::default());
    }
}
