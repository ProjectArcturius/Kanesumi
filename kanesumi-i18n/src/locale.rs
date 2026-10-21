// locale.rs —— 界面语言判定。参 Ether docs/I18N_PLAN.md §Ⅱ-1。
// 1.0 只承诺四种界面语言（Ether 裁定 §178）；其余语言一律落到 en-US。

/// 界面语言。进程启动时判定一次，改语言须注销后生效（同 Win10）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiLocale {
    EnUs,
    JaJp,
    ZhCn,
    ZhTw,
}

impl UiLocale {
    pub const ALL: [UiLocale; 4] = [UiLocale::EnUs, UiLocale::JaJp, UiLocale::ZhCn, UiLocale::ZhTw];

    /// 源语言：随代码同写、编译进二进制作最终回落。
    pub const SOURCE: UiLocale = UiLocale::ZhCn;

    /// 在 [`UiLocale::ALL`] 中的下标（`Domain::sources` 按此排列）。
    pub fn index(self) -> usize {
        match self {
            UiLocale::EnUs => 0,
            UiLocale::JaJp => 1,
            UiLocale::ZhCn => 2,
            UiLocale::ZhTw => 3,
        }
    }

    /// BCP 47 标签，同时是资源目录名（`<dir>/<tag>/<域>.ftl`）。
    pub fn tag(self) -> &'static str {
        match self {
            UiLocale::EnUs => "en-US",
            UiLocale::JaJp => "ja-JP",
            UiLocale::ZhCn => "zh-CN",
            UiLocale::ZhTw => "zh-TW",
        }
    }

    /// 回落链：本语言 → en-US → 内置 zh-CN。
    /// 繁中缺译时给英文而不是简中（与 Windows 一致：不拿另一种文字顶替）。
    pub fn fallback_chain(self) -> &'static [UiLocale] {
        match self {
            UiLocale::ZhCn => &[UiLocale::ZhCn],
            UiLocale::EnUs => &[UiLocale::EnUs, UiLocale::ZhCn],
            UiLocale::JaJp => &[UiLocale::JaJp, UiLocale::EnUs, UiLocale::ZhCn],
            UiLocale::ZhTw => &[UiLocale::ZhTw, UiLocale::EnUs, UiLocale::ZhCn],
        }
    }

    /// 一条 POSIX 区域名（`zh_CN.UTF-8`、`ja_JP@euro`、`zh-Hant` …）→ 界面语言。
    /// `C` / `POSIX` / 空串 / 不识别的语言 → `None`，由调用方决定回落。
    pub fn from_posix(value: &str) -> Option<UiLocale> {
        // 去掉编码（.UTF-8）与修饰（@xxx），统一分隔符。
        let base = value.split(['.', '@']).next().unwrap_or("").trim();
        if base.is_empty() || base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
            return None;
        }
        let mut parts = base.split(['_', '-']);
        let lang = parts.next().unwrap_or("").to_ascii_lowercase();
        let rest: Vec<String> = parts.map(|p| p.to_ascii_lowercase()).collect();
        match lang.as_str() {
            "en" => Some(UiLocale::EnUs),
            "ja" => Some(UiLocale::JaJp),
            "zh" => {
                // 繁体：显式 Hant 文字，或台湾 / 香港 / 澳门地区。
                let traditional = rest.iter().any(|p| matches!(p.as_str(), "hant" | "tw" | "hk" | "mo"));
                Some(if traditional { UiLocale::ZhTw } else { UiLocale::ZhCn })
            }
            _ => None,
        }
    }

    /// 按 POSIX 顺序判定：`LC_ALL` → `LC_MESSAGES` → `LANG` 中第一个非空者为生效区域；
    /// 生效区域为 `C` / `POSIX` 时忽略 `LANGUAGE`（与 glibc 一致），否则 `LANGUAGE`
    /// 列表（冒号分隔）优先。都不识别 → en-US。
    pub fn detect_with(get: impl Fn(&str) -> Option<String>) -> UiLocale {
        let non_empty = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        let effective = non_empty("LC_ALL")
            .or_else(|| non_empty("LC_MESSAGES"))
            .or_else(|| non_empty("LANG"));
        if let Some(eff) = &effective {
            let base = eff.split(['.', '@']).next().unwrap_or("");
            if base.eq_ignore_ascii_case("C") || base.eq_ignore_ascii_case("POSIX") {
                return UiLocale::EnUs;
            }
        }
        if let Some(list) = non_empty("LANGUAGE")
            && let Some(l) = list.split(':').find_map(UiLocale::from_posix)
        {
            return l;
        }
        effective
            .as_deref()
            .and_then(UiLocale::from_posix)
            .unwrap_or(UiLocale::EnUs)
    }

    /// 从进程环境判定。
    pub fn detect() -> UiLocale {
        Self::detect_with(|k| std::env::var(k).ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let m: HashMap<String, String> =
            pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        move |k| m.get(k).cloned()
    }

    #[test]
    fn posix_名映射到四种语言() {
        assert_eq!(UiLocale::from_posix("zh_CN.UTF-8"), Some(UiLocale::ZhCn));
        assert_eq!(UiLocale::from_posix("zh_SG"), Some(UiLocale::ZhCn));
        assert_eq!(UiLocale::from_posix("zh"), Some(UiLocale::ZhCn));
        assert_eq!(UiLocale::from_posix("zh_TW.UTF-8"), Some(UiLocale::ZhTw));
        assert_eq!(UiLocale::from_posix("zh_HK.UTF-8"), Some(UiLocale::ZhTw));
        assert_eq!(UiLocale::from_posix("zh-Hant"), Some(UiLocale::ZhTw));
        assert_eq!(UiLocale::from_posix("ja_JP.UTF-8"), Some(UiLocale::JaJp));
        assert_eq!(UiLocale::from_posix("en_GB.UTF-8"), Some(UiLocale::EnUs));
        assert_eq!(UiLocale::from_posix("fr_FR.UTF-8"), None);
        assert_eq!(UiLocale::from_posix("C.UTF-8"), None);
        assert_eq!(UiLocale::from_posix("POSIX"), None);
        assert_eq!(UiLocale::from_posix(""), None);
    }

    #[test]
    fn 判定顺序_lc_all_压过_lang() {
        let l = UiLocale::detect_with(env(&[("LC_ALL", "ja_JP.UTF-8"), ("LANG", "zh_CN.UTF-8")]));
        assert_eq!(l, UiLocale::JaJp);
        let l = UiLocale::detect_with(env(&[("LC_MESSAGES", "zh_TW.UTF-8"), ("LANG", "en_US.UTF-8")]));
        assert_eq!(l, UiLocale::ZhTw);
    }

    #[test]
    fn language_列表优先_c_区域时忽略() {
        let l = UiLocale::detect_with(env(&[("LANGUAGE", "fr:ja:en"), ("LANG", "zh_CN.UTF-8")]));
        assert_eq!(l, UiLocale::JaJp);
        let l = UiLocale::detect_with(env(&[("LANGUAGE", "ja"), ("LC_ALL", "C")]));
        assert_eq!(l, UiLocale::EnUs);
    }

    #[test]
    fn 无环境或不识别回落英文() {
        assert_eq!(UiLocale::detect_with(env(&[])), UiLocale::EnUs);
        assert_eq!(UiLocale::detect_with(env(&[("LANG", "de_DE.UTF-8")])), UiLocale::EnUs);
        assert_eq!(UiLocale::detect_with(env(&[("LANG", "")])), UiLocale::EnUs);
    }

    #[test]
    fn 回落链以源语言收尾_繁中不经简中顶替() {
        for l in UiLocale::ALL {
            assert_eq!(*l.fallback_chain().last().unwrap(), UiLocale::SOURCE);
            assert_eq!(l.fallback_chain()[0], l);
        }
        assert_eq!(UiLocale::ZhTw.fallback_chain()[1], UiLocale::EnUs);
    }
}
