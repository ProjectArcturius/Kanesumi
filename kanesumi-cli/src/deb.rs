//! deb.rs —— 最小 Debian 包写出器（ar + ustar + gzip），不依赖 dpkg。
//!
//! 存在理由：`kanesumi package` 要在**任何**机器上出包（开发机可能是 Windows），而
//! `dpkg-deb` 只有 Debian 有；工具链不能强绑运行环境。整包格式其实很小：
//!
//! ```text
//! !<arch>\n            ← ar 归档
//!   debian-binary      ← 内容 "2.0\n"
//!   control.tar.gz     ← control / (md5sums / conffiles 可选)
//!   data.tar.gz        ← 安装树
//! ```
//!
//! 只写这三个成员：`md5sums` 是 dpkg `--verify` 用的可选件，本工具不生成（宁缺不错）。

use std::io::Write;

/// data.tar.gz 里的一个文件。
pub struct DebFile {
    /// 安装树内的路径，不带前导 `/`（如 `usr/bin/kanesumi-demo`）。
    pub path: String,
    /// 文件内容。
    pub data: Vec<u8>,
    /// 权限位（如 `0o644` / `0o755`）。
    pub mode: u32,
}

/// 打一个 deb 包，返回整包字节。
///
/// `control` 是 `DEBIAN/control` 的文本内容。
pub fn build(control: &str, files: &[DebFile]) -> Result<Vec<u8>, String> {
    let control_tar = tar(&[("./control".to_string(), control.as_bytes().to_vec(), 0o644)])?;
    let mut data_entries: Vec<(String, Vec<u8>, u32)> = Vec::with_capacity(files.len());
    for f in files {
        let name = format!("./{}", f.path.trim_start_matches('/'));
        data_entries.push((name, f.data.clone(), f.mode));
    }
    let data_tar = tar(&data_entries)?;

    let members = [
        ("debian-binary".to_string(), b"2.0\n".to_vec()),
        ("control.tar.gz".to_string(), gzip(&control_tar)?),
        ("data.tar.gz".to_string(), gzip(&data_tar)?),
    ];
    Ok(ar(&members))
}

/// gzip 压缩。
fn gzip(raw: &[u8]) -> Result<Vec<u8>, String> {
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(raw).map_err(|e| format!("gzip 写入失败：{e}"))?;
    enc.finish().map_err(|e| format!("gzip 收尾失败：{e}"))
}

/// 写 ar 归档（System V 格式，`!<arch>\n` 头 + 60 字节成员头）。
fn ar(members: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"!<arch>\n");
    for (name, data) in members {
        // 成员名 16 字节：名字 + `/` 结尾（GNU ar 约定），空格补齐。
        let mut name_field = format!("{name}/");
        while name_field.len() < 16 {
            name_field.push(' ');
        }
        debug_assert_eq!(name_field.len(), 16);
        out.extend_from_slice(name_field.as_bytes());
        out.extend_from_slice(pad("0", 12).as_bytes()); // mtime
        out.extend_from_slice(pad("0", 6).as_bytes()); // uid
        out.extend_from_slice(pad("0", 6).as_bytes()); // gid
        out.extend_from_slice(pad("100644", 8).as_bytes()); // mode
        out.extend_from_slice(pad(&data.len().to_string(), 10).as_bytes()); // size
        out.extend_from_slice(b"`\n"); // 成员头结束
        out.extend_from_slice(data);
        if data.len() % 2 == 1 {
            out.push(b'\n'); // ar 成员按偶数字节对齐
        }
    }
    out
}

/// 十进制字段：左对齐、空格补到 `width`。
fn pad(s: &str, width: usize) -> String {
    let mut out = s.to_string();
    while out.len() < width {
        out.push(' ');
    }
    out
}

/// 写 ustar 归档。
fn tar(entries: &[(String, Vec<u8>, u32)]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for (name, data, mode) in entries {
        out.extend_from_slice(&tar_header(name, data.len(), *mode)?);
        out.extend_from_slice(data);
        let pad = (512 - data.len() % 512) % 512;
        out.extend(std::iter::repeat_n(0u8, pad));
    }
    // 归档结束：两个全零块。
    out.extend(std::iter::repeat_n(0u8, 1024));
    Ok(out)
}

/// 写一个 ustar 头（512 字节）。
fn tar_header(name: &str, size: usize, mode: u32) -> Result<[u8; 512], String> {
    if name.len() > 100 {
        return Err(format!("路径超过 100 字节，ustar 单成员头放不下：{name}"));
    }
    let mut h = [0u8; 512];
    let put = |h: &mut [u8; 512], off: usize, bytes: &[u8]| {
        h[off..off + bytes.len()].copy_from_slice(bytes);
    };
    put(&mut h, 0, name.as_bytes());
    // mode / uid / gid / size / mtime：八进制 ASCII，右补 NUL。
    put(&mut h, 100, &octal(mode as u64, 7));
    put(&mut h, 108, &octal(0, 7));
    put(&mut h, 116, &octal(0, 7));
    put(&mut h, 124, &octal(size as u64, 11));
    put(&mut h, 136, &octal(0, 11));
    // 校验和字段先填空格，算完再写。
    for b in &mut h[148..156] {
        *b = b' ';
    }
    h[156] = b'0'; // 普通文件
    put(&mut h, 257, b"ustar\0");
    put(&mut h, 263, b"00");
    let sum: u64 = h.iter().map(|b| *b as u64).sum();
    // 校验和格式：6 位八进制 + NUL + 空格。
    let chk = format!("{:06o}\0 ", sum);
    put(&mut h, 148, chk.as_bytes());
    Ok(h)
}

/// 八进制字段：`width` 位数字 + 结尾 NUL。
fn octal(v: u64, width: usize) -> Vec<u8> {
    let mut s = format!("{v:0width$o}");
    if s.len() > width {
        s = "0".repeat(width);
    }
    let mut out = s.into_bytes();
    out.push(0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 读回 ar：返回 (成员名, 数据)。
    fn read_ar(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        assert_eq!(&bytes[..8], b"!<arch>\n", "ar 魔数");
        let mut out = Vec::new();
        let mut pos = 8;
        while pos + 60 <= bytes.len() {
            let head = &bytes[pos..pos + 60];
            let name = String::from_utf8_lossy(&head[0..16]).trim().trim_end_matches('/').to_string();
            let size: usize = String::from_utf8_lossy(&head[48..58]).trim().parse().unwrap();
            assert_eq!(&head[58..60], b"`\n", "成员头结束符");
            let data = bytes[pos + 60..pos + 60 + size].to_vec();
            out.push((name, data));
            pos += 60 + size + (size % 2);
        }
        out
    }

    /// 读回 tar：返回 (路径, 内容)。
    fn read_tar(bytes: &[u8]) -> Vec<(String, Vec<u8>)> {
        let mut out = Vec::new();
        let mut pos = 0;
        while pos + 512 <= bytes.len() {
            let h = &bytes[pos..pos + 512];
            if h.iter().all(|b| *b == 0) {
                break;
            }
            let name = String::from_utf8_lossy(&h[0..100]).trim_end_matches('\0').to_string();
            let size_text = String::from_utf8_lossy(&h[124..136]);
            let size = usize::from_str_radix(size_text.trim_end_matches('\0').trim(), 8).unwrap();
            let data = bytes[pos + 512..pos + 512 + size].to_vec();
            out.push((name, data));
            pos += 512 + size + (512 - size % 512) % 512;
        }
        out
    }

    #[test]
    fn writes_three_ar_members_in_dpkg_order() {
        let hashes = [
            DebFile {
                path: "usr/bin/kanesumi-demo".into(),
                data: b"#!/bin/sh\necho hi\n".to_vec(),
                mode: 0o755,
            },
            DebFile {
                path: "usr/share/applications/org.example.demo.desktop".into(),
                data: b"[Desktop Entry]\n".to_vec(),
                mode: 0o644,
            },
        ];
        let deb = build("Package: org.example.demo\nVersion: 0.1.0-1\n", &hashes).unwrap();
        let members = read_ar(&deb);
        let names: Vec<&str> = members.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["debian-binary", "control.tar.gz", "data.tar.gz"]);
        assert_eq!(members[0].1, b"2.0\n");
    }

    #[test]
    fn control_and_data_contain_expected_entries() {
        let files = [DebFile {
            path: "usr/bin/demo".into(),
            data: b"binary".to_vec(),
            mode: 0o755,
        }];
        let deb = build("Package: org.example.demo\nVersion: 0.1.0-1\n", &files).unwrap();
        let members = read_ar(&deb);

        let control_tar = gunzip(&members[1].1);
        let control = read_tar(&control_tar);
        assert_eq!(control.len(), 1);
        assert_eq!(control[0].0, "./control");
        let text = String::from_utf8(control[0].1.clone()).unwrap();
        assert!(text.contains("Package: org.example.demo"), "control 必须带包名");

        let data_tar = gunzip(&members[2].1);
        let data = read_tar(&data_tar);
        assert_eq!(data[0].0, "./usr/bin/demo");
        assert_eq!(data[0].1, b"binary");
    }

    #[test]
    fn tar_header_checksum_matches_reference_rule() {
        let h = tar_header("./control", 7, 0o644).unwrap();
        // 校验和字段按「6 位八进制 + NUL + 空格」写，且等于把该字段当空格时的字节和。
        let mut copy = h;
        for b in &mut copy[148..156] {
            *b = b' ';
        }
        let expect: u64 = copy.iter().map(|b| *b as u64).sum();
        let got = String::from_utf8_lossy(&h[148..154]).to_string();
        assert_eq!(u64::from_str_radix(got.trim(), 8).unwrap(), expect);
        assert_eq!(h[154], 0, "校验和以 NUL 结尾");
        assert_eq!(h[155], b' ', "校验和以空格收尾");
    }

    /// 测试用 gunzip。
    fn gunzip(raw: &[u8]) -> Vec<u8> {
        use std::io::Read;
        let mut out = Vec::new();
        flate2::read::GzDecoder::new(raw).read_to_end(&mut out).unwrap();
        out
    }
}
