// kanesumi-i18n —— Kanesumi 文案层。参 Ether docs/I18N_PLAN.md §Ⅱ-1。
//
// 界面文案不写死在代码里：每个域（`common` / 各应用）每种语言一份 Fluent 资源，键为稳定 ID。
// 四种 1.0 语言全部编译进二进制（`domain!`），缺资源不会出现键名或另一种语言；
// 语言包装到 `/usr/share/ether/i18n/<tag>/<域>.ftl` 的同名文件逐条覆盖（修订译文不必重编），
// 开发态用 `ETHER_I18N_DIR`（可多个目录，按平台路径分隔符）覆盖。界面语言进程启动时判定一次。

mod locale;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, RwLock};

use fluent_bundle::concurrent::FluentBundle;
use fluent_bundle::FluentResource;

pub use fluent_bundle::{FluentArgs, FluentValue};
pub use locale::UiLocale;

/// 系统级资源目录（语言包 deb 安装处）。
pub const SYSTEM_DIR: &str = "/usr/share/ether/i18n";

/// 一个文案域：名字 + 内置的四种语言资源（下标同 [`UiLocale::ALL`]）。
#[derive(Debug, Clone, Copy)]
pub struct Domain {
    pub name: &'static str,
    pub sources: [&'static str; 4],
}

impl Domain {
    /// 某语言的内置资源。
    pub fn source(&self, locale: UiLocale) -> &'static str {
        self.sources[locale.index()]
    }
}

/// 声明一个域：`domain!("topbar", "i18n")` 编译期读入
/// `<CARGO_MANIFEST_DIR>/i18n/{en-US,ja-JP,zh-CN,zh-TW}/topbar.ftl`。四个文件缺一个即编译失败。
#[macro_export]
macro_rules! domain {
    ($name:literal, $dir:literal) => {
        $crate::Domain {
            name: $name,
            sources: [
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $dir, "/en-US/", $name, ".ftl")),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $dir, "/ja-JP/", $name, ".ftl")),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $dir, "/zh-CN/", $name, ".ftl")),
                include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/", $dir, "/zh-TW/", $name, ".ftl")),
            ],
        }
    };
}

/// Kanesumi 控件缺省文案（开 / 关 …）。全局目录自动注册。
pub const COMMON: Domain = domain!("common", "i18n");

/// 运行时资源目录：`ETHER_I18N_DIR` 列出的目录在前（优先），系统目录在后。
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("ETHER_I18N_DIR")
        .map(|v| std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()).collect())
        .unwrap_or_default();
    dirs.push(PathBuf::from(SYSTEM_DIR));
    dirs
}

fn parse(src: String, origin: &str) -> FluentResource {
    match FluentResource::try_new(src) {
        Ok(r) => r,
        Err((r, errs)) => {
            // 语法错只丢坏掉的条目，其余照用；坏条目走回落链。
            log::warn!("kanesumi-i18n 资源 {origin} 有 {} 处语法错，已跳过坏条目", errs.len());
            r
        }
    }
}

/// 源语言资源里定义的全部消息 ID。
pub fn message_ids(source: &str) -> Vec<String> {
    let res = parse(source.to_string(), "<source>");
    res.entries()
        .filter_map(|e| match e {
            fluent_syntax::ast::Entry::Message(m) => Some(m.id.name.to_string()),
            _ => None,
        })
        .collect()
}

/// 完整性检查：内置资源里各语言相对源语言缺哪些键。各应用据此写「译文无缺」测试。
pub fn missing_translations(domain: Domain) -> Vec<(UiLocale, String)> {
    let ids = message_ids(domain.source(UiLocale::SOURCE));
    let mut out = Vec::new();
    for l in UiLocale::ALL {
        if l == UiLocale::SOURCE {
            continue;
        }
        let have: HashSet<String> = message_ids(domain.source(l)).into_iter().collect();
        out.extend(ids.iter().filter(|id| !have.contains(*id)).map(|id| (l, id.clone())));
    }
    out
}

/// 一种界面语言下的文案目录：回落链上每种语言一个 bundle，按链序查找。
pub struct Catalog {
    locale: UiLocale,
    dirs: Vec<PathBuf>,
    chain: Vec<(UiLocale, FluentBundle<FluentResource>)>,
    domains: Vec<&'static str>,
    missing: Mutex<HashSet<String>>,
}

impl Catalog {
    /// `dirs` 越靠前优先级越高。不含任何域；用 [`Catalog::add_domain`] 加。
    pub fn new(locale: UiLocale, dirs: Vec<PathBuf>) -> Self {
        let chain = locale
            .fallback_chain()
            .iter()
            .map(|&l| {
                let id: unic_langid::LanguageIdentifier = l.tag().parse().expect("内置语言标签合法");
                let mut b = FluentBundle::new_concurrent(vec![id]);
                // 四种语言都是左到右；隔离符 U+2068/U+2069 只会成方框或零宽噪声。
                b.set_use_isolating(false);
                (l, b)
            })
            .collect();
        Self { locale, dirs, chain, domains: Vec::new(), missing: Mutex::new(HashSet::new()) }
    }

    pub fn locale(&self) -> UiLocale {
        self.locale
    }

    /// 加一个域。同名域重复加忽略。先放内置资源，再让目录里的同名文件逐条覆盖。
    pub fn add_domain(&mut self, domain: Domain) {
        if self.domains.contains(&domain.name) {
            return;
        }
        self.domains.push(domain.name);
        for (l, bundle) in &mut self.chain {
            let res = parse(domain.source(*l).to_string(), domain.name);
            if let Err(errs) = bundle.add_resource(res) {
                log::warn!("kanesumi-i18n 域 {} 内置资源键冲突 {} 处", domain.name, errs.len());
            }
            // 低优先级先放、高优先级后覆盖。
            for dir in self.dirs.iter().rev() {
                let path = dir.join(l.tag()).join(format!("{}.ftl", domain.name));
                if let Ok(src) = std::fs::read_to_string(&path) {
                    bundle.add_resource_overriding(parse(src, &path.display().to_string()));
                }
            }
        }
    }

    pub fn tr(&self, id: &str) -> String {
        self.format(id, None)
    }

    pub fn tr_args(&self, id: &str, args: &FluentArgs) -> String {
        self.format(id, Some(args))
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {
        for (_, bundle) in &self.chain {
            if let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) {
                let mut errs = Vec::new();
                let s = bundle.format_pattern(pattern, args, &mut errs);
                if !errs.is_empty() {
                    log::warn!("kanesumi-i18n 键 {id} 格式化出错：{errs:?}");
                }
                return s.into_owned();
            }
        }
        // 全链缺键：显示键名（可见、可搜），同一键只告警一次。
        if let Ok(mut m) = self.missing.lock()
            && m.insert(id.to_string())
        {
            log::warn!("kanesumi-i18n 缺键 {id}（{}）", self.locale.tag());
        }
        id.to_string()
    }
}

static GLOBAL: OnceLock<RwLock<Catalog>> = OnceLock::new();

fn global() -> &'static RwLock<Catalog> {
    GLOBAL.get_or_init(|| {
        let mut c = Catalog::new(UiLocale::detect(), default_dirs());
        c.add_domain(COMMON);
        RwLock::new(c)
    })
}

/// 进程级注册一个域（应用启动时调用；重复调用无害）。
pub fn register(domain: Domain) {
    if let Ok(mut c) = global().write() {
        c.add_domain(domain);
    }
}

/// 进程界面语言。
pub fn ui_locale() -> UiLocale {
    global().read().map(|c| c.locale()).unwrap_or(UiLocale::SOURCE)
}

/// 取文案。
pub fn tr(id: &str) -> String {
    global().read().map(|c| c.tr(id)).unwrap_or_else(|_| id.to_string())
}

/// 取带参文案（`{ $count }` 等）。
pub fn tr_args(id: &str, args: &FluentArgs) -> String {
    global().read().map(|c| c.tr_args(id, args)).unwrap_or_else(|_| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 下标同 UiLocale::ALL：en-US / ja-JP / zh-CN / zh-TW。
    const DEMO: Domain = Domain {
        name: "demo",
        sources: [
            "demo-hello = Hello
demo-items = { $count ->
    [one] { $count } item
   *[other] { $count } items
}
",
            "demo-hello = こんにちは
",
            "demo-hello = 你好
demo-items = { $count } 项
demo-only-source = 只有源语言
",
            "",
        ],
    };

    /// 每次调用独立目录：并行测试同写一处会读到截断文件。
    fn override_dir(tag: &str, body: &str) -> PathBuf {
        static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let d = std::env::temp_dir().join(format!("kanesumi-i18n-test-{}-{n}", std::process::id()));
        std::fs::create_dir_all(d.join(tag)).unwrap();
        std::fs::write(d.join(tag).join("demo.ftl"), body).unwrap();
        d
    }

    #[test]
    fn 内置资源_无目录也完整() {
        let mut c = Catalog::new(UiLocale::ZhCn, vec![]);
        c.add_domain(DEMO);
        assert_eq!(c.tr("demo-hello"), "你好");
        let mut c = Catalog::new(UiLocale::EnUs, vec![]);
        c.add_domain(DEMO);
        assert_eq!(c.tr("demo-hello"), "Hello");
    }

    #[test]
    fn 回落链_缺译依次落到英文与源语言() {
        let mut ja = Catalog::new(UiLocale::JaJp, vec![]);
        ja.add_domain(DEMO);
        assert_eq!(ja.tr("demo-hello"), "こんにちは");
        // 日文缺 → 英文。
        let mut a = FluentArgs::new();
        a.set("count", 2);
        assert_eq!(ja.tr_args("demo-items", &a), "2 items");
        // 英文也缺 → 源语言。
        assert_eq!(ja.tr("demo-only-source"), "只有源语言");
        // 繁中整份缺 → 英文，不经简中。
        let mut tw = Catalog::new(UiLocale::ZhTw, vec![]);
        tw.add_domain(DEMO);
        assert_eq!(tw.tr("demo-hello"), "Hello");
    }

    #[test]
    fn 英文复数且无隔离符() {
        let mut c = Catalog::new(UiLocale::EnUs, vec![]);
        c.add_domain(DEMO);
        let mut a = FluentArgs::new();
        a.set("count", 1);
        assert_eq!(c.tr_args("demo-items", &a), "1 item");
    }

    #[test]
    fn 目录文件逐条覆盖内置_靠前目录优先() {
        let hi = override_dir("ja-JP", "demo-hello = やあ
");
        let lo = override_dir("ja-JP", "demo-hello = どうも
");
        let mut c = Catalog::new(UiLocale::JaJp, vec![hi, lo]);
        c.add_domain(DEMO);
        assert_eq!(c.tr("demo-hello"), "やあ");
        // 未覆盖的键仍走内置 / 回落。
        assert_eq!(c.tr("demo-only-source"), "只有源语言");
    }

    #[test]
    fn 全链缺键显示键名() {
        let c = Catalog::new(UiLocale::ZhTw, vec![]);
        assert_eq!(c.tr("nope-key"), "nope-key");
    }

    #[test]
    fn 完整性检查列出缺译() {
        let missing = missing_translations(DEMO);
        assert!(missing.contains(&(UiLocale::JaJp, "demo-items".to_string())));
        assert!(missing.contains(&(UiLocale::ZhTw, "demo-hello".to_string())));
        assert!(!missing.iter().any(|(l, id)| *l == UiLocale::EnUs && id == "demo-hello"));
    }

    #[test]
    fn common_四种语言无缺() {
        assert_eq!(missing_translations(COMMON), vec![]);
        for (l, on) in [
            (UiLocale::ZhCn, "开"),
            (UiLocale::ZhTw, "開啟"),
            (UiLocale::JaJp, "オン"),
            (UiLocale::EnUs, "On"),
        ] {
            let mut c = Catalog::new(l, vec![]);
            c.add_domain(COMMON);
            assert_eq!(c.tr("common-on"), on, "{}", l.tag());
        }
    }
}
