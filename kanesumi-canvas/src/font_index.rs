//! 会话级字体索引 v1（`fonts-v1.idx`）读写库（A3.0）。
//!
//! 规格唯一来源：`docs/FONT_INDEX_FORMAT.md`（2026-10-31 已签，CONTRACT I-0~I-11 +
//! §2 唯一推荐）。本模块只做结构 + 写 + 读 + 校验：无开关、无环境变量、不被
//! `text.rs` / harness 调用（不接线，A3.1~A3.3 另件）。
//!
//! 不变量：任何输入字节 [`parse`](Index::parse) 都返回 `Err`、不 panic（切片前一律
//! `checked_add` + 上界比较）；0 unsafe；整数一律小端；所有 `*_off` 为相对文件
//! 第 0 字节的绝对偏移；条目区固定紧跟头部；正文校验和最后验证（结构检查在前，
//! 保证每个损坏变体命中各自的 [`IndexError`]）。
//!
//! 写侧约定：覆盖区段入参必须来自 `CoverageTable::from_face(..).ranges()`
//! （A1b 同一建表函数，禁止另写 cmap 枚举）；`file_size` / `content_hash`
//! 由写侧按传入的文件字节自算（见 [`IndexEntrySpec`]），不复述调用方给的值。

use std::fmt;

/// magic：`"KFSX"`。
const MAGIC: [u8; 4] = *b"KFSX";
/// abi v1：结构布局变化才升（F1 端序 / 偏移基准等落在 abi 内）。
const ABI: u16 = 1;
/// 策略版本（F8/B3）：候选链 / `WEIGHT_INSTANCES` / TTC 选面 / 指纹目录集合任一变化必 +1。
pub(crate) const POLICY_VERSION: u32 = 1;
/// 头部长度（B）。
const HEADER_LEN: usize = 96;
/// 条目长度（B）。
const ENTRY_LEN: usize = 64;
/// 目录指纹记录长度（B）。
const DIRFP_LEN: usize = 24;
/// 条目数上限（规格 §2）。
pub(crate) const MAX_FONT_COUNT: u32 = 4096;
/// 目录指纹条数上限（规格 §4）。
pub(crate) const MAX_DIRFP_COUNT: usize = 64;
/// 覆盖记录宽度（B）：`(start: u32, end: u32)` 紧排小端。
const COV_REC: usize = 8;

/// FNV-1a 64 偏移基 / 素数——与 `text.rs::refresh_identity` 同一组常量，全仓唯一定义处。
const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;

/// FNV-1a 64：逐字节先 XOR 后乘（规格 §7 身份与 §8 校验和共用）。
pub(crate) fn fnv1a64(data: &[u8]) -> u64 {
    let mut h = FNV_OFFSET;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

/// 读侧拒绝原因（规格 §10）。任一命中 → 整份作废、退本地路径（fail-safe，不防伪）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexError {
    /// 文件不足 96 B（连头部都不完整）。
    Truncated,
    /// magic 不是 `"KFSX"`。
    Magic,
    /// `abi` ≠ 1。
    Abi(u16),
    /// `policy_ver` ≠ [`POLICY_VERSION`]（组装策略变了，覆盖内容不可信）。
    PolicyVer(u32),
    /// `header_len` ≠ 96。
    HeaderLen(u32),
    /// `flags` ≠ 1（bit0 必须置位、其余位必须清零，规格 §2）。
    Flags(u16),
    /// `pad0` / `reserved` 非 0（I-4/I-10：未知位即未来 abi 的地盘，当前读取端不猜）。
    Reserved,
    /// `font_count` 超上限或条目区越出文件。
    FontCount(u32),
    /// `body_len` ≠ 文件长 − 96（尾部多余字节也算损坏）。
    BodyLen(u32),
    /// 任一 `*_off` / `(off, len)` 切片越出文件或加法溢出。
    OffsetOutOfBounds,
    /// 目录指纹表：条数超上限、区越界、路径非严格升序或 `pad0` 非 0。
    Dirfp,
    /// 条目覆盖区段乱序 / 重叠 / 相邻未合并 / 越出覆盖区（附条目下标）。
    CovRanges(usize),
    /// 正文校验和不符：（存储值，计算值）。
    Checksum(u64, u64),
}

impl fmt::Display for IndexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match *self {
            Self::Truncated => "文件不足 96 B".to_string(),
            Self::Magic => "magic 不是 KFSX".to_string(),
            Self::Abi(v) => format!("abi={v} 不受支持（只认 {ABI}）"),
            Self::PolicyVer(v) => format!("policy_ver={v} 与本实现 {POLICY_VERSION} 不符"),
            Self::HeaderLen(v) => format!("header_len={v} 应为 {HEADER_LEN}"),
            Self::Flags(v) => format!("flags={v:#06x} 应为 1"),
            Self::Reserved => "pad0/reserved 非 0".to_string(),
            Self::FontCount(v) => format!("font_count={v} 超上限 {MAX_FONT_COUNT} 或条目区越界"),
            Self::BodyLen(v) => format!("body_len={v} 与文件长不符"),
            Self::OffsetOutOfBounds => "偏移/长度切片越界或溢出".to_string(),
            Self::Dirfp => "目录指纹表条数/顺序/填充不符".to_string(),
            Self::CovRanges(i) => format!("条目 {i} 覆盖区段乱序/重叠/相邻未合并/越界"),
            Self::Checksum(stored, calc) => {
                format!("正文校验和不符: 存 {stored:#018x} 算 {calc:#018x}")
            }
        };
        f.write_str(&s)
    }
}

impl std::error::Error for IndexError {}

/// 读侧小端解码辅助。全部 `Option` 化：越界/溢出一律 `None`，调用方译成对应
/// [`IndexError`]——这是「任何输入字节不 panic」的实现手段。
fn rd2(b: &[u8], o: usize) -> Option<u16> {
    let e = o.checked_add(2)?;
    let s = b.get(o..e)?;
    Some(u16::from_le_bytes([s[0], s[1]]))
}

fn rd4(b: &[u8], o: usize) -> Option<u32> {
    let e = o.checked_add(4)?;
    let s = b.get(o..e)?;
    Some(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn rd8(b: &[u8], o: usize) -> Option<u64> {
    let e = o.checked_add(8)?;
    let s = b.get(o..e)?;
    let mut a = [0u8; 8];
    a.copy_from_slice(s);
    Some(u64::from_le_bytes(a))
}

/// 区间 `[off, off+len)` 是否落在正文区 `[HEADER_LEN, file_len)` 内
///（空区段允许 `off == off+len`）。`checked_add` 防溢出（规格 §1）。
fn in_body(file_len: usize, off: u32, len: u32) -> bool {
    let (o, l) = (u64::from(off), u64::from(len));
    o >= HEADER_LEN as u64
        && o.checked_add(l).is_some_and(|e| e <= file_len as u64)
}

/// 头部已解码字段（小端读出；布局与语义见规格 §2；magic/abi/flags/header_len/
/// body_len 校验后不保留——通过即恒定值）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexHeader {
    /// 条目数（`font_id` == 下标，规格 §3）。
    pub font_count: u32,
    /// 字符串池（绝对偏移 + 长）。
    pub str_off: u32,
    pub str_len: u32,
    /// 覆盖区段表（绝对偏移 + 长）。
    pub cov_off: u32,
    pub cov_len: u32,
    /// 重建计数（不进任何键）。
    pub generation: u64,
    /// 构建时刻（诊断）。
    pub built_unix_ns: u64,
    /// 整栈身份（规格 §7 算法产出）。
    pub stack_identity: u64,
    /// 候选根被查阅的诊断位（读侧不解释）。
    pub source_mask: u32,
    /// 目录指纹表（绝对偏移 + 条数）。
    pub dirfp_off: u32,
    pub dirfp_count: u16,
    /// 正文校验和（规格 §8）。
    pub checksum: u64,
    /// 组装策略版本（F8/B3）。
    pub policy_ver: u32,
}

/// 已通过全部校验的索引视图（借用整个文件字节，零拷贝）。
#[derive(Debug, Clone, Copy)]
pub struct Index<'a> {
    file: &'a [u8],
    header: IndexHeader,
}

impl<'a> Index<'a> {
    /// 从内存字节解析。检查次序固定：结构字段在前、正文校验和最后——保证每个
    /// 损坏变体命中各自的 [`IndexError`]（测试「变体 → 错误变体」表依赖此序）。
    pub fn parse(file: &'a [u8]) -> Result<Self, IndexError> {
        if file.len() < HEADER_LEN {
            return Err(IndexError::Truncated);
        }
        if file[..4] != MAGIC {
            return Err(IndexError::Magic);
        }
        let abi = rd2(file, 4).ok_or(IndexError::Truncated)?;
        if abi != ABI {
            return Err(IndexError::Abi(abi));
        }
        let policy_ver = rd4(file, 80).ok_or(IndexError::Truncated)?;
        if policy_ver != POLICY_VERSION {
            return Err(IndexError::PolicyVer(policy_ver));
        }
        let header_len = rd4(file, 8).ok_or(IndexError::Truncated)?;
        if header_len as usize != HEADER_LEN {
            return Err(IndexError::HeaderLen(header_len));
        }
        let flags = rd2(file, 6).ok_or(IndexError::Truncated)?;
        if flags != 1 {
            return Err(IndexError::Flags(flags));
        }
        if rd2(file, 70) != Some(0) || file[84..96].iter().any(|&b| b != 0) {
            return Err(IndexError::Reserved);
        }
        let font_count = rd4(file, 16).ok_or(IndexError::Truncated)?;
        let n = usize::try_from(font_count).unwrap_or(usize::MAX);
        let entries_end = ENTRY_LEN
            .checked_mul(n)
            .and_then(|t| HEADER_LEN.checked_add(t))
            .ok_or(IndexError::FontCount(font_count))?;
        if font_count > MAX_FONT_COUNT || entries_end > file.len() {
            return Err(IndexError::FontCount(font_count));
        }
        let body_len = rd4(file, 12).ok_or(IndexError::Truncated)?;
        if body_len as usize != file.len() - HEADER_LEN {
            return Err(IndexError::BodyLen(body_len));
        }
        let str_off = rd4(file, 20).ok_or(IndexError::Truncated)?;
        let str_len = rd4(file, 24).ok_or(IndexError::Truncated)?;
        let cov_off = rd4(file, 28).ok_or(IndexError::Truncated)?;
        let cov_len = rd4(file, 32).ok_or(IndexError::Truncated)?;
        let dirfp_off = rd4(file, 64).ok_or(IndexError::Truncated)?;
        let dirfp_count = rd2(file, 68).ok_or(IndexError::Truncated)?;
        if !in_body(file.len(), str_off, str_len) || !in_body(file.len(), cov_off, cov_len) {
            return Err(IndexError::OffsetOutOfBounds);
        }
        if usize::from(dirfp_count) > MAX_DIRFP_COUNT
            || !in_body(file.len(), dirfp_off, u32::from(dirfp_count) * DIRFP_LEN as u32)
        {
            return Err(IndexError::Dirfp);
        }
        let idx = Self {
            file,
            header: IndexHeader {
                font_count,
                str_off,
                str_len,
                cov_off,
                cov_len,
                generation: rd8(file, 36).ok_or(IndexError::Truncated)?,
                built_unix_ns: rd8(file, 44).ok_or(IndexError::Truncated)?,
                stack_identity: rd8(file, 52).ok_or(IndexError::Truncated)?,
                source_mask: rd4(file, 60).ok_or(IndexError::Truncated)?,
                dirfp_off,
                dirfp_count,
                checksum: rd8(file, 72).ok_or(IndexError::Truncated)?,
                policy_ver,
            },
        };
        idx.check_dirfps()?;
        idx.check_entries()?;
        let calc = fnv1a64(&file[HEADER_LEN..]);
        if idx.header.checksum != calc {
            return Err(IndexError::Checksum(idx.header.checksum, calc));
        }
        Ok(idx)
    }

    /// 第 `i` 条目（`font_id` == 下标）。越界返回 `None`。
    pub fn entry(&self, i: usize) -> Option<FontEntry<'a>> {
        entry_at(self.file, &self.header, i)
    }

    /// 按栈序遍历条目（下标即 `font_id`）。
    pub fn entries(&self) -> impl Iterator<Item = FontEntry<'a>> + '_ {
        (0..usize::try_from(self.header.font_count).unwrap_or(0))
            .filter_map(move |i| entry_at(self.file, &self.header, i))
    }

    /// 目录指纹（行序 = 路径字节升序）。
    pub fn dirfps(&self) -> impl Iterator<Item = DirFingerprint<'a>> + '_ {
        (0..usize::from(self.header.dirfp_count))
            .filter_map(move |i| dirfp_at(self.file, &self.header, i))
    }

    /// 第 `i` 条目的覆盖区段视图。
    pub fn coverage(&self, i: usize) -> Option<CoverageRanges<'a>> {
        let e = self.entry(i)?;
        Some(coverage_at(self.file, &self.header, e.cov_off, e.cov_ranges))
    }

    pub fn header(&self) -> &IndexHeader {
        &self.header
    }

    fn pool(&self) -> &'a [u8] {
        let o = self.header.str_off as usize;
        &self.file[o..o + self.header.str_len as usize]
    }

    fn cov_region(&self) -> &'a [u8] {
        let o = self.header.cov_off as usize;
        &self.file[o..o + self.header.cov_len as usize]
    }

    /// 目录指纹校验：`pad0` 为 0、路径切片在池内、按路径字节**严格升序**
    ///（规格 §4；写侧排序去重，读侧不放过乱序）。
    fn check_dirfps(&self) -> Result<(), IndexError> {
        let pool = self.pool();
        let mut prev: Option<&[u8]> = None;
        for i in 0..usize::from(self.header.dirfp_count) {
            let base = self.header.dirfp_off as usize + i * DIRFP_LEN;
            let f = self.file.get(base..base + DIRFP_LEN).ok_or(IndexError::Dirfp)?;
            // 记录布局（规格 §4 勘误版）：path_off u32@0、path_len u16@4、pad0 u16@6、
            // mtime_ns i64@8、ino u64@16。path_len 补上是为了让读取端真能映射回路径
            //（契约 §2.1 原样 `(path_off, pad0 u32, ..)` 无法恢复长度，属起草缺陷）。
            if rd2(f, 6) != Some(0) {
                return Err(IndexError::Dirfp);
            }
            let (po, pl) = (
                rd4(f, 0).ok_or(IndexError::Dirfp)?,
                u64::from(rd2(f, 4).ok_or(IndexError::Dirfp)?),
            );
            let (o, l) = (u64::from(po), pl);
            if o.checked_add(l).is_none_or(|e| e > pool.len() as u64) {
                return Err(IndexError::Dirfp);
            }
            let path = &pool[o as usize..o as usize + l as usize];
            if prev.is_some_and(|p| p >= path) {
                return Err(IndexError::Dirfp);
            }
            prev = Some(path);
        }
        Ok(())
    }

    /// 条目校验：`pad0` 为 0、路径/族名切片在池内、覆盖切片在覆盖区内且
    /// 区段升序 + 互不重叠 + **互不相邻**（规格 §5）。
    fn check_entries(&self) -> Result<(), IndexError> {
        let pool = self.pool();
        let cov = self.cov_region();
        for i in 0..usize::try_from(self.header.font_count).unwrap_or(0) {
            let base = HEADER_LEN + i * ENTRY_LEN;
            let e = self
                .file
                .get(base..base + ENTRY_LEN)
                .ok_or(IndexError::OffsetOutOfBounds)?;
            if rd2(e, 30) != Some(0) {
                return Err(IndexError::Reserved);
            }
            for o in [0usize, 8usize] {
                let (off, len) = (
                    u64::from(rd4(e, o).ok_or(IndexError::OffsetOutOfBounds)?),
                    u64::from(rd4(e, o + 4).ok_or(IndexError::OffsetOutOfBounds)?),
                );
                if off.checked_add(len).is_none_or(|e| e > pool.len() as u64) {
                    return Err(IndexError::OffsetOutOfBounds);
                }
            }
            let n = usize::from(rd2(e, 28).ok_or(IndexError::OffsetOutOfBounds)?);
            let co = u64::from(rd4(e, 24).ok_or(IndexError::OffsetOutOfBounds)?);
            let need = u64::try_from(n.checked_mul(COV_REC).ok_or(IndexError::CovRanges(i))?)
                .map_err(|_| IndexError::CovRanges(i))?;
            let head = u64::from(self.header.cov_off);
            if co < head
                || co.checked_add(need).is_none_or(|e| e > head + u64::from(self.header.cov_len))
            {
                return Err(IndexError::CovRanges(i));
            }
            let start = (co - head) as usize;
            let recs = &cov[start..start + n * COV_REC];
            let mut prev_end: Option<u32> = None;
            for j in 0..n {
                let s = rd4(recs, j * COV_REC).ok_or(IndexError::CovRanges(i))?;
                let e2 = rd4(recs, j * COV_REC + 4).ok_or(IndexError::CovRanges(i))?;
                if s > e2 || prev_end.is_some_and(|p| s <= p + 1) {
                    return Err(IndexError::CovRanges(i));
                }
                prev_end = Some(e2);
            }
        }
        Ok(())
    }
}

/// 条目解码核心（[`Index`] / [`OwnedIndex`] 共用；前置条件：`parse` 已通过全部校验）。
fn entry_at<'f>(file: &'f [u8], header: &IndexHeader, i: usize) -> Option<FontEntry<'f>> {
    if i >= usize::try_from(header.font_count).unwrap_or(0) {
        return None;
    }
    let base = HEADER_LEN + i * ENTRY_LEN;
    let e = file.get(base..base + ENTRY_LEN)?;
    let (so, sl_) = (header.str_off as usize, header.str_len as usize);
    let pool = &file[so..so + sl_];
    let sl = |o: usize| -> (&'f [u8], u32, u32) {
        let off = rd4(e, o).unwrap_or(0) as usize;
        let len = rd4(e, o + 4).unwrap_or(0) as usize;
        (&pool[off..off + len], off as u32, len as u32)
    };
    let (path, _, _) = sl(0);
    let (family, _, _) = sl(8);
    Some(FontEntry {
        path,
        family,
        collection_index: rd2(e, 16).unwrap_or(0),
        weight: rd2(e, 18).unwrap_or(0),
        slant: e[20],
        flags: e[21],
        ttc_faces: rd2(e, 22).unwrap_or(1),
        cov_off: rd4(e, 24).unwrap_or(0),
        cov_ranges: rd2(e, 28).unwrap_or(0),
        wght_min: f32::from_bits(rd4(e, 32).unwrap_or(0)),
        wght_max: f32::from_bits(rd4(e, 36).unwrap_or(0)),
        file_size: rd8(e, 40).unwrap_or(0),
        mtime_ns: rd8(e, 48).unwrap_or(0) as i64,
        content_hash: rd8(e, 56).unwrap_or(0),
    })
}

/// 目录指纹解码核心（共用）。
fn dirfp_at<'f>(file: &'f [u8], header: &IndexHeader, i: usize) -> Option<DirFingerprint<'f>> {
    let base = header.dirfp_off as usize + i * DIRFP_LEN;
    let f = file.get(base..base + DIRFP_LEN)?;
    let (so, sl_) = (header.str_off as usize, header.str_len as usize);
    let pool = &file[so..so + sl_];
    let off = rd4(f, 0)? as usize;
    let len = usize::from(rd2(f, 4)?);
    Some(DirFingerprint {
        path: &pool[off..off + len],
        mtime_ns: rd8(f, 8)? as i64,
        ino: rd8(f, 16)?,
    })
}

/// 覆盖区段视图构造核心（共用；前置条件：条目/区段校验已通过）。
fn coverage_at<'f>(
    file: &'f [u8],
    header: &IndexHeader,
    cov_off: u32,
    cov_ranges: u16,
) -> CoverageRanges<'f> {
    let head = header.cov_off as usize;
    let start = cov_off as usize - head;
    let n = usize::from(cov_ranges);
    let region = &file[head..head + header.cov_len as usize];
    CoverageRanges {
        recs: &region[start..start + n * COV_REC],
    }
}

/// 拥有字节的索引：文件读入用（[`Index`] 是借用视图，无法持有自己的读入字节）。
#[derive(Debug, Clone)]
pub struct OwnedIndex {
    bytes: Vec<u8>,
    header: IndexHeader,
}

impl OwnedIndex {
    /// 整文件读入并解析。IO 失败与格式失败分型（[`OpenError`]）。
    pub fn open(path: &std::path::Path) -> Result<Self, OpenError> {
        let bytes = std::fs::read(path).map_err(OpenError::Io)?;
        let header = *Index::parse(&bytes).map_err(OpenError::Format)?.header();
        Ok(Self { bytes, header })
    }

    pub fn header(&self) -> &IndexHeader {
        &self.header
    }

    pub fn entry(&self, i: usize) -> Option<FontEntry<'_>> {
        entry_at(&self.bytes, &self.header, i)
    }

    pub fn entries(&self) -> impl Iterator<Item = FontEntry<'_>> + '_ {
        (0..usize::try_from(self.header.font_count).unwrap_or(0))
            .filter_map(|i| entry_at(&self.bytes, &self.header, i))
    }

    pub fn dirfps(&self) -> impl Iterator<Item = DirFingerprint<'_>> + '_ {
        (0..usize::from(self.header.dirfp_count))
            .filter_map(|i| dirfp_at(&self.bytes, &self.header, i))
    }

    pub fn coverage(&self, i: usize) -> Option<CoverageRanges<'_>> {
        let e = self.entry(i)?;
        Some(coverage_at(&self.bytes, &self.header, e.cov_off, e.cov_ranges))
    }
}

/// `Index::open` 的失败分型：IO 与格式分开，避免把「读不到文件」谎报成格式损坏。
#[derive(Debug)]
pub enum OpenError {
    /// 文件读不出来（缺失 / 权限 / IO）。
    Io(std::io::Error),
    /// 读到了但没通过校验。
    Format(IndexError),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "字体索引读取失败: {e}"),
            Self::Format(e) => write!(f, "字体索引无效: {e}"),
        }
    }
}

impl std::error::Error for OpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Format(e) => Some(e),
        }
    }
}

/// 条目视图（字段全量解码；字符串借用池内字节，UTF-8 不保证）。
#[derive(Debug, Clone, Copy)]
pub struct FontEntry<'a> {
    /// 绝对路径字节（已 canonicalize 的构建侧约定；读侧原样借用）。
    pub path: &'a [u8],
    /// 族名字节（诊断用）。
    pub family: &'a [u8],
    /// TTC 字面下标（非 TTC = 0）。
    pub collection_index: u16,
    /// OS/2 usWeightClass；VF 条目存请求目标值。
    pub weight: u16,
    /// 0=upright、1=italic、2=oblique；未知按 0。
    pub slant: u8,
    /// bit0 VF、bit1 TTC、bit2 已校验（I-7：客户端不据此跳解析）。
    pub flags: u8,
    /// TTC 内字面总数（非 TTC = 1，诊断）。
    pub ttc_faces: u16,
    /// 覆盖段内该字面首记录的绝对偏移（可与其他条目共享）。
    pub cov_off: u32,
    /// 区段记录条数。
    pub cov_ranges: u16,
    /// 可变字体 `wght` 轴范围（非 VF 为 0.0；实例坐标 `clamp(target, min, max)` 复算）。
    pub wght_min: f32,
    pub wght_max: f32,
    /// 一级校验：文件大小。
    pub file_size: u64,
    /// 一级校验：mtime（ns）。
    pub mtime_ns: i64,
    /// 纯文件字节 FNV-1a 64（不混 `collection_index`，规格 §7）。
    pub content_hash: u64,
}

impl<'a> FontEntry<'a> {
    pub fn path_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.path).ok()
    }

    pub fn family_str(&self) -> Option<&'a str> {
        std::str::from_utf8(self.family).ok()
    }

    /// 该字面的覆盖区段视图（校验已在 [`Index::parse`] 完成）。
    pub fn coverage(&self, idx: &Index<'a>) -> CoverageRanges<'a> {
        coverage_at(idx.file, &idx.header, self.cov_off, self.cov_ranges)
    }
}

/// 目录指纹视图（规格 §4：路径 + `(mtime_ns, ino)`；双 0 = 构建时缺席哨兵）。
#[derive(Debug, Clone, Copy)]
pub struct DirFingerprint<'a> {
    pub path: &'a [u8],
    pub mtime_ns: i64,
    pub ino: u64,
}

impl<'a> DirFingerprint<'a> {
    /// 是否缺席哨兵（构建时目录不存在）。
    pub fn is_absent_sentinel(&self) -> bool {
        self.mtime_ns == 0 && self.ino == 0
    }
}

/// 覆盖区段二分查询（闭区间、升序、互不重叠不相邻；磁盘侧 [`FontEntry::coverage`]，
/// 进程内侧 `text::CoverageTable`——两者对拍由真机字体的 3000 码点测试锁定）。
#[derive(Debug, Clone, Copy)]
pub struct CoverageRanges<'a> {
    recs: &'a [u8],
}

/// 写侧条目描述。`coverage` **必须**来自 `CoverageTable::from_face(..).ranges()`
///（A1b 同一建表函数；本模块不写第二套 cmap 枚举）。
pub struct IndexEntrySpec<'a> {
    /// 绝对路径（构建侧已 canonicalize）。
    pub path: &'a str,
    /// 族名（name ID 1/16，诊断）。
    pub family: &'a str,
    pub collection_index: u16,
    pub weight: u16,
    pub slant: u8,
    /// bit0 VF、bit1 TTC、bit2 已校验（构建侧双校验通过后置）。
    pub flags: u8,
    pub ttc_faces: u16,
    /// 升序、互不重叠且互不相邻的闭区间（非法序在读侧被拒）。
    pub coverage: &'a [(u32, u32)],
    pub wght_min: f32,
    pub wght_max: f32,
    /// 一级校验用 mtime（fs 元数据，调用方提供）。
    pub mtime_ns: i64,
}

/// 写侧目录指纹描述。
pub struct DirFpSpec<'a> {
    pub path: &'a str,
    /// `(mtime_ns, ino)`；目录不存在时传 `(0, 0)` 哨兵。
    pub mtime_ns: i64,
    pub ino: u64,
}

/// 写出整份索引字节（纯函数：不碰文件系统、不选路径策略）。
///
/// `entry_bytes[i]` = 条目 i 的整份文件字节（VF 7 条目共享同一切片）：
/// `file_size` / `content_hash` / `stack_identity` 全部据此计算（规格 §3/§7），
/// 调用方不传、写侧不复述。条目 ≤ [`MAX_FONT_COUNT`]、指纹 ≤ [`MAX_DIRFP_COUNT`]，
/// 违反是调用方错误（`debug_assert`）。
pub fn write_index(
    entries: &[IndexEntrySpec<'_>],
    entry_bytes: &[&[u8]],
    dirfps: &[DirFpSpec<'_>],
    generation: u64,
    built_unix_ns: u64,
    source_mask: u32,
) -> Vec<u8> {
    debug_assert_eq!(entries.len(), entry_bytes.len(), "条目与字节必须一一对应");
    debug_assert!(entries.len() <= MAX_FONT_COUNT as usize, "条目超上限");
    debug_assert!(dirfps.len() <= MAX_DIRFP_COUNT, "目录指纹超上限");

    // 字符串池：条目 path + family 按栈序，随后指纹路径（排序去重）。
    let mut pool: Vec<u8> = Vec::new();
    let mut str_slabs: Vec<(u32, u32)> = Vec::new(); // (绝对偏移后回填基准, len) 先记池内相对
    for e in entries {
        for s in [e.path.as_bytes(), e.family.as_bytes()] {
            str_slabs.push((pool.len() as u32, s.len() as u32));
            pool.extend_from_slice(s);
        }
    }
    // 指纹：按路径字节升序；同路径去重（保留首条）→ 严格升序成立。
    let mut fps: Vec<&DirFpSpec<'_>> = dirfps.iter().collect();
    fps.sort_by(|a, b| a.path.as_bytes().cmp(b.path.as_bytes()));
    fps.dedup_by(|a, b| a.path == b.path);
    let mut fp_slabs: Vec<(u32, u32)> = Vec::new();
    for f in &fps {
        fp_slabs.push((pool.len() as u32, f.path.len() as u32));
        pool.extend_from_slice(f.path.as_bytes());
    }

    // 覆盖区段：字节级去重（VF 7 实例同 cmap 只存一份，规格 §5）。
    let mut cov: Vec<u8> = Vec::new();
    let mut cov_offsets: Vec<u32> = Vec::with_capacity(entries.len());
    let mut dedupe: std::collections::HashMap<Vec<u8>, u32> = std::collections::HashMap::new();
    for e in entries {
        debug_assert!(
            e.coverage.len() <= u16::MAX as usize,
            "覆盖区段条数超 u16"
        );
        let mut rec = Vec::with_capacity(e.coverage.len() * COV_REC);
        for &(s, t) in e.coverage {
            rec.extend_from_slice(&s.to_le_bytes());
            rec.extend_from_slice(&t.to_le_bytes());
        }
        let next = cov.len() as u32;
        let off = match dedupe.get(&rec) {
            Some(&o) => o,
            None => {
                cov.extend_from_slice(&rec);
                dedupe.insert(rec, next);
                next
            }
        };
        cov_offsets.push(off);
    }

    // 布局：头部 96 → 条目 64×N → 池 → 覆盖 → 指纹；三个 *_off 为绝对偏移。
    let str_off = (HEADER_LEN + ENTRY_LEN * entries.len()) as u32;
    let cov_off = str_off + pool.len() as u32;
    let dirfp_off = cov_off + cov.len() as u32;
    let mut out = Vec::with_capacity(dirfp_off as usize + DIRFP_LEN * fps.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&ABI.to_le_bytes()); // 4
    out.extend_from_slice(&1u16.to_le_bytes()); // 6 flags: bit0 恒 1
    out.extend_from_slice(&(HEADER_LEN as u32).to_le_bytes()); // 8
    out.extend_from_slice(&0u32.to_le_bytes()); // 12 body_len 占位
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes()); // 16
    out.extend_from_slice(&str_off.to_le_bytes()); // 20
    out.extend_from_slice(&(pool.len() as u32).to_le_bytes()); // 24
    out.extend_from_slice(&cov_off.to_le_bytes()); // 28
    out.extend_from_slice(&(cov.len() as u32).to_le_bytes()); // 32
    out.extend_from_slice(&generation.to_le_bytes()); // 36
    out.extend_from_slice(&built_unix_ns.to_le_bytes()); // 44
    out.extend_from_slice(&0u64.to_le_bytes()); // 52 identity 占位
    out.extend_from_slice(&source_mask.to_le_bytes()); // 60
    out.extend_from_slice(&dirfp_off.to_le_bytes()); // 64
    out.extend_from_slice(&(fps.len() as u16).to_le_bytes()); // 68
    out.extend_from_slice(&0u16.to_le_bytes()); // 70 pad0
    out.extend_from_slice(&0u64.to_le_bytes()); // 72 checksum 占位
    out.extend_from_slice(&POLICY_VERSION.to_le_bytes()); // 80
    out.extend_from_slice(&[0u8; 12]); // 84 reserved

    // 条目区：file_size / content_hash 由 entry_bytes 自算；identity 逐条目喂字节。
    let mut identity = FNV_OFFSET;
    for (i, (e, bytes)) in entries.iter().zip(entry_bytes).enumerate() {
        identity = {
            let mut h = identity;
            for &b in *bytes {
                h ^= u64::from(b);
                h = h.wrapping_mul(FNV_PRIME);
            }
            h ^= u64::from(e.collection_index);
            h = h.wrapping_mul(FNV_PRIME);
            h
        };
        let (po, pl) = (str_off + str_slabs[i * 2].0, str_slabs[i * 2].1);
        let (fo, fl) = (str_off + str_slabs[i * 2 + 1].0, str_slabs[i * 2 + 1].1);
        out.extend_from_slice(&po.to_le_bytes());
        out.extend_from_slice(&pl.to_le_bytes());
        out.extend_from_slice(&fo.to_le_bytes());
        out.extend_from_slice(&fl.to_le_bytes());
        out.extend_from_slice(&e.collection_index.to_le_bytes());
        out.extend_from_slice(&e.weight.to_le_bytes());
        out.push(e.slant);
        out.push(e.flags);
        out.extend_from_slice(&e.ttc_faces.to_le_bytes());
        out.extend_from_slice(&(cov_off + cov_offsets[i]).to_le_bytes());
        out.extend_from_slice(&(e.coverage.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // pad0
        out.extend_from_slice(&e.wght_min.to_bits().to_le_bytes());
        out.extend_from_slice(&e.wght_max.to_bits().to_le_bytes());
        out.extend_from_slice(&(bytes.len() as u64).to_le_bytes()); // file_size
        out.extend_from_slice(&e.mtime_ns.to_le_bytes());
        out.extend_from_slice(&fnv1a64(bytes).to_le_bytes()); // content_hash
    }
    out.extend_from_slice(&pool);
    out.extend_from_slice(&cov);
    for (i, f) in fps.iter().enumerate() {
        out.extend_from_slice(&(str_off + fp_slabs[i].0).to_le_bytes());
        out.extend_from_slice(&(f.path.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // pad0
        out.extend_from_slice(&f.mtime_ns.to_le_bytes());
        out.extend_from_slice(&f.ino.to_le_bytes());
    }

    // 回填三个占位：body_len、stack_identity、checksum（规格 §8：正文最后校验）。
    let body_len = (out.len() - HEADER_LEN) as u32;
    out[12..16].copy_from_slice(&body_len.to_le_bytes());
    out[52..60].copy_from_slice(&identity.to_le_bytes());
    let checksum = fnv1a64(&out[HEADER_LEN..]);
    out[72..80].copy_from_slice(&checksum.to_le_bytes());
    out
}

impl<'a> CoverageRanges<'a> {
    pub fn len(&self) -> usize {
        self.recs.len() / COV_REC
    }

    pub fn is_empty(&self) -> bool {
        self.recs.is_empty()
    }

    /// 码点 `c` 是否落在任一区间（O(log n) 二分；与 `text::CoverageTable::has` 同语义）。
    pub fn has(&self, c: char) -> bool {
        let cp = u32::from(c);
        let n = self.len();
        let mut lo = 0usize;
        let mut hi = n;
        while lo < hi {
            let mid = (lo + hi) / 2;
            if rd4(self.recs, mid * COV_REC).unwrap_or(u32::MAX) <= cp {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return false;
        }
        rd4(self.recs, (lo - 1) * COV_REC + 4).is_some_and(|end| end >= cp)
    }
}

