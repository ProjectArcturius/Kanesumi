//! 系统字体只读映射（runtime 轨 A1，裁定 §246：unsafe 只放 harness）。
//!
//! 读取器供 [`kanesumi_canvas::text::TextEngine::load_stack_with`] 使用：仅当
//! `canonicalize` 后路径位于 `/usr/share/fonts/` 之下、普通文件、属主 uid 0、
//! 组与其他用户无写位时，用 `memmap2::Mmap` 只读映射；其余一律回退
//! `std::fs::read`（用户目录字体、符号链接逃逸、权限不符、映射失败）。
//! 缺省关闭，`KANESUMI_FONT_MMAP=1` 打开（§121-3）。每次读取向
//! `~/.local/state/ether/kanesumi-font.log` 追加一行自证：进程名、字体路径、
//! `mmap|read`、字节数（零交互排障口径，参 AGENTS.md 铁律 3）。

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kanesumi_canvas::text::FontBytes;

/// 系统字体目录（映射白名单前缀）。
const SYSTEM_FONT_DIR: &str = "/usr/share/fonts/";

/// 映射资格（纯函数，单测锁定）。`canon` 已 `canonicalize`——符号链接逃逸
/// （链到系统字体目录之外）在这里天然落选，无需单独判链接。
fn mmap_eligible(canon: &Path, is_file: bool, uid: u32, mode: u32) -> bool {
    canon.starts_with(SYSTEM_FONT_DIR) && is_file && uid == 0 && (mode & 0o022) == 0
}

/// 缺省开关（§121-3）：`KANESUMI_FONT_MMAP=1` 才启用映射。
fn mmap_enabled() -> bool {
    std::env::var_os("KANESUMI_FONT_MMAP").is_some_and(|v| v == "1")
}

/// 回退读取：整文件进堆。与旧 `load_stack` 逐字节同源。
fn read_fallback(path: &Path) -> io::Result<FontBytes> {
    Ok(Arc::new(std::fs::read(path)?))
}

/// 读取并标注来源：`(句柄, 是否映射)`。`enabled` 注入便于单测（真进程读环境变量）。
fn read_font_bytes_traced(path: &Path, enabled: bool) -> io::Result<(FontBytes, bool)> {
    if enabled {
        let canon: PathBuf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if let Ok(meta) = std::fs::metadata(&canon) {
            use std::os::unix::fs::MetadataExt;
            if mmap_eligible(&canon, meta.is_file(), meta.uid(), meta.mode()) {
                // 打不开（权限竞态等）→ 落到末尾的 read 回退。
                if let Ok(file) = std::fs::File::open(&canon) {
                    // SAFETY：只读映射一个符合 mmap_eligible 的系统字体文件。
                    // 不变式：映射对象是 root 拥有、组/其他无写位的
                    // /usr/share/fonts 下普通文件，普通进程无法原地改写；
                    // Debian 字体包升级走「写临时文件 + rename() 原子替换」，
                    // rename 只换目录项，旧 inode 与既有只读映射不受影响。
                    // 故映射生命周期内文件内容不会被改写，只读映射安全。
                    match unsafe { memmap2::Mmap::map(&file) } {
                        Ok(mmap) => return Ok((Arc::new(mmap), true)),
                        Err(e) => {
                            log::warn!("字体映射失败，回退 fs::read：{}（{e}）", canon.display());
                        }
                    }
                }
            }
        }
    }
    read_fallback(path).map(|handle| (handle, false))
}

/// 字体读取器（供 `TextEngine::load_stack_with` 的 `read` 参数）：按资格规则
/// mmap 或回退 `fs::read`，并向自证日志追加一行。
pub fn read_font_bytes(path: &Path) -> io::Result<FontBytes> {
    let (handle, mapped) = read_font_bytes_traced(path, mmap_enabled())?;
    let len = handle.as_ref().as_ref().len();
    write_self_report(path, mapped, len);
    Ok(handle)
}

/// 自证日志：一行一字体，`<进程名> <路径> <mmap|read> <字节数>`。写失败静默
///（日志是排障辅助，不该打断字体加载）。
fn write_self_report(path: &Path, mapped: bool, len: usize) {
    let dir = match std::env::var_os("XDG_STATE_HOME") {
        Some(d) if !d.is_empty() => PathBuf::from(d).join("ether"),
        _ => PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
            .join(".local/state/ether"),
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let proc_name = std::fs::read_to_string("/proc/self/comm").unwrap_or_default();
    let line = format!(
        "{} {} {} {}\n",
        proc_name.trim_end(),
        path.display(),
        if mapped { "mmap" } else { "read" },
        len,
    );
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("kanesumi-font.log"))
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fs_read_reader(path: &Path) -> io::Result<FontBytes> {
        read_fallback(path)
    }

    /// 资格四则（任务书 §2-4 的前三条回退在此纯函数层锁定）。
    #[test]
    fn mmap_eligibility_rules() {
        // /home 之下 → 不映射。
        assert!(!mmap_eligible(Path::new("/home/u/font.ttf"), true, 0, 0o444));
        // 非属主 uid 0 → 不映射。
        assert!(!mmap_eligible(Path::new("/usr/share/fonts/a.ttf"), true, 1000, 0o444));
        // 组可写 → 不映射。
        assert!(!mmap_eligible(Path::new("/usr/share/fonts/a.ttf"), true, 0, 0o664));
        // 其他用户可写 → 不映射。
        assert!(!mmap_eligible(Path::new("/usr/share/fonts/a.ttf"), true, 0, 0o642));
        // 目录（非普通文件）→ 不映射。
        assert!(!mmap_eligible(Path::new("/usr/share/fonts/noto"), false, 0, 0o755));
        // 符合条件（root、只读、系统目录普通文件）→ 映射。
        assert!(mmap_eligible(Path::new("/usr/share/fonts/noto/a.ttc"), true, 0, 0o644));
    }

    /// 临时字体文件的回退路径：来源 read、字节与 `fs::read` 一致。
    #[test]
    fn fallback_reads_identical_bytes() {
        let dir = std::env::temp_dir().join("kanesumi-font-mmap-test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("fallback.ttf");
        let payload: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
        std::fs::write(&path, &payload).unwrap();
        // enabled=true 也必须回退：路径在 /usr/share/fonts 之外（canonicalize 逃逸）。
        let (handle, mapped) = read_font_bytes_traced(&path, true).unwrap();
        assert!(!mapped, "/home 式临时路径不得映射");
        assert_eq!(handle.as_ref().as_ref(), payload.as_slice());
        // 另一入口：字符串路径引用同一文件。
        let (handle2, mapped2) = read_font_bytes_traced(&path, false).unwrap();
        assert!(!mapped2);
        assert_eq!(handle2.as_ref().as_ref(), payload.as_slice());
        let _ = fs_read_reader(&path);
    }

    /// 指向 /usr/share/fonts 之外的符号链接：canonicalize 解析到白名单外 → 回退。
    #[cfg(target_os = "linux")]
    #[test]
    fn symlink_escape_falls_back() {
        let dir = std::env::temp_dir().join("kanesumi-font-mmap-symlink");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("real.ttf");
        std::fs::write(&target, b"otf-bytes").unwrap();
        let link = dir.join("link.ttf");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let (handle, mapped) = read_font_bytes_traced(&link, true).unwrap();
        assert!(!mapped, "指向系统目录之外的符号链接不得映射");
        assert_eq!(handle.as_ref().as_ref(), b"otf-bytes");
    }

    /// 符合资格的真实系统字体：mmap 生效、字节与 `fs::read` 一致。
    /// 判门环境（Debian + Noto CJK）必有此类文件；缺文件即失败——判门环境不符。
    #[cfg(target_os = "linux")]
    #[test]
    fn eligible_system_font_maps() {
        let dir = match std::fs::read_dir("/usr/share/fonts") {
            Ok(d) => d,
            Err(e) => panic!("/usr/share/fonts 不可读，判门环境不符：{e}"),
        };
        let mut found = None;
        let mut stack = vec![dir];
        while let Some(rd) = stack.pop() {
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Ok(sub) = std::fs::read_dir(&p) {
                        stack.push(sub);
                    }
                } else if let Ok(meta) = std::fs::metadata(&p) {
                    use std::os::unix::fs::MetadataExt;
                    if mmap_eligible(&p, meta.is_file(), meta.uid(), meta.mode()) {
                        found = Some(p);
                        break;
                    }
                }
            }
            if found.is_some() {
                break;
            }
        }
        let path = found.expect("本机 /usr/share/fonts 下无 root 只读字体，判门环境不符");
        let expected = std::fs::read(&path).unwrap();
        let (handle, mapped) = read_font_bytes_traced(&path, true).unwrap();
        assert!(mapped, "符合资格的系统字体应 mmap：{}", path.display());
        assert_eq!(handle.as_ref().as_ref(), expected.as_slice(), "映射字节须与读文件一致");
        // 关闭开关：同一路径回退 read（缺省行为未变）。
        let (_, mapped_off) = read_font_bytes_traced(&path, false).unwrap();
        assert!(!mapped_off, "KANESUMI_FONT_MMAP 关闭时必须回退 fs::read");
    }
}
