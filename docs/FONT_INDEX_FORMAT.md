# Kanesumi 字体索引格式 v1（fonts-v1.idx）内部规格

> 状态：**已签规格（A3.0 实现依据）**。2026-10-31 调度者在 runtime 方向/实施职责内批准
> F1~F10 按 `A3_FORMAT_CONTRACT_DRAFT_2026-10-31.md`（Ether 仓 `docs/research/rt_font_service/`，
> 下称 CONTRACT）的 I-0~I-11 与 §2 唯一推荐落地。**这是未部署的内部缓存版本，不是 R3 第三方
> SDK ABI**；B5（content_hash 复算时机）与 B6（验收门口径）未批；目录 stat 扫描与失效流程
> 只作为规格，不接线。数值上限除已签处外仍是提议（4 MiB / 3000 码点等不升格）。
> 实现文件：`kanesumi-canvas/src/font_index.rs`（0 unsafe、无开关、不被产品调用）。

## 1. 总则：端序、偏移、溢出范围（CONTRACT I-1 / I-2）

- **端序（F1）**：全部多字节整数**小端**。写侧只用 `to_le_bytes`，读侧只用 `from_le_bytes`；
  禁止按本机端序整块转写。
- **偏移（F2）**：`str_off` / `cov_off` / `dirfp_off`（头部）与条目 `cov_off` 一律是
  **相对文件第 0 字节的绝对偏移**，单位字节，`u32`。
- **溢出范围（上界检查，违者整份作废）**：
  - `header_len == 96`；`abi == 1`；`body_len == file_len − 96`（正文必须恰好覆盖文件尾，
    尾部多余字节即损坏）。
  - 条目区 `[96, 96 + 64×font_count)`、字符串区 `[str_off, str_off+str_len)`、
    覆盖区 `[cov_off, cov_off+cov_len)`、指纹区 `[dirfp_off, dirfp_off+24×dirfp_count)`
    全部 ⊆ `[96, file_len)`；所有加法先 `checked_add`。
  - `font_count ≤ 4096`；`dirfp_count ≤ 64`；每条目 `cov_ranges ≤ 65535`（u16 自然上限）且
    `[cov_off, cov_off + 8×cov_ranges)` ⊆ 覆盖区内。
- **条目区位置**：唯一无显式指针的区段，固定紧跟头部（见 §11 写侧布局）。

## 2. 头部（96 B，偏移|字段|宽|语义）

| 偏移 | 字段 | 宽 | 语义（读侧要求） |
|---:|---|---:|---|
| 0 | `magic` | 4 | `"KFSX"`，不符作废 |
| 4 | `abi` | 2 | u16 LE = 1，不符作废 |
| 6 | `flags` | 2 | u16 LE = **1**（I-4：bit0 恒 1，其余位必须 0） |
| 8 | `header_len` | 4 | u32 LE = 96 |
| 12 | `body_len` | 4 | u32 LE = `file_len − 96` |
| 16 | `font_count` | 4 | u32 LE，条目数（I-6），≤ 4096 |
| 20 | `str_off` | 4 | u32 LE，绝对偏移 |
| 24 | `str_len` | 4 | u32 LE |
| 28 | `cov_off` | 4 | u32 LE，绝对偏移 |
| 32 | `cov_len` | 4 | u32 LE |
| 36 | `generation` | 8 | u64 LE，重建计数，不进任何键 |
| 44 | `built_unix_ns` | 8 | u64 LE，诊断 |
| 52 | `stack_identity` | 8 | u64 LE，§7 算法产出 |
| 60 | `source_mask` | 4 | u32 LE，诊断位（候选根被查阅），读侧不解释 |
| 64 | `dirfp_off` | 4 | u32 LE，绝对偏移 |
| 68 | `dirfp_count` | 2 | u16 LE ≤ 64 |
| 70 | `pad0` | 2 | u16 LE = 0 |
| 72 | `checksum` | 8 | u64 LE，§8 |
| 80 | `policy_ver` | 4 | u32 LE = **1**（B3/F8：reserved 前 4 B 升格；不等即作废） |
| 84 | `reserved` | 12 | 全 0，非 0 作废 |

## 3. 条目（64 B × `font_count`，`font_id` == 数组下标，**永不重排**）

| 偏移 | 字段 | 宽 | 语义 |
|---:|---|---:|---|
| 0 | `path_off` / `path_len` | 4+4 | u32 LE ×2，**绝对文件偏移**；绝对路径（已 canonicalize），池内 UTF-8 |
| 8 | `family_off` / `family_len` | 4+4 | u32 LE ×2，绝对文件偏移；族名（name ID 1/16），诊断用 |
| 16 | `collection_index` | 2 | u16 LE，TTC 字面下标（非 TTC = 0） |
| 18 | `weight` | 2 | u16 LE，OS/2 usWeightClass；VF 条目存**请求目标值** |
| 20 | `slant` | 1 | 0=upright、1=italic、2=oblique；未知按 0，不拒 |
| 21 | `flags` | 1 | bit0 VF、bit1 TTC、bit2 已校验（I-7：客户端不据此跳解析） |
| 22 | `ttc_faces` | 2 | u16 LE，TTC 内字面总数（非 TTC = 1），诊断 |
| 24 | `cov_off` | 4 | u32 LE，指向覆盖段内该字面首记录，**可共享**（I-5） |
| 28 | `cov_ranges` | 2 | u16 LE，区段记录条数 |
| 30 | `pad0` | 2 | u16 LE = 0 |
| 32 | `wght_min` / `wght_max` | 4+4 | f32 位型 LE；非 VF = 0.0 |
| 40 | `file_size` | 8 | u64 LE，一级校验用 |
| 48 | `mtime_ns` | 8 | i64 LE，一级校验用 |
| 56 | `content_hash` | 8 | u64 LE，**纯文件字节** FNV-1a 64，不混 `collection_index`（I-8） |

**条目口径（F7/B1）**：`font_count` = 条目数（`FontFace` 粒度）。VF 按 `WEIGHT_INSTANCES`
7 档展开 7 行（同 path、同 `collection_index`、`weight` 各为目标值）；TTC 按 tag 选 1 面 1 行。
`font_id` == 下标是跨进程契约，失效不改数组（§9）。

## 4. 目录指纹（24 B × `dirfp_count`，B4/F6）

> **勘误（2026-10-31，实现时发现并修正）**：契约 §2.1 原记录 `(path_off: u32, pad0: u32, …)`
> 读取端无法恢复路径长度（映射回路径的目的落空），属起草缺陷。修正为下表：`pad0: u32`
> 拆为 `path_len: u16 + pad0: u16`，总宽 24 B 与 mtime/ino 偏移不变。

| 偏移 | 字段 | 宽 | 语义 |
|---:|---|---:|---|
| 0 | `path_off` | 4 | u32 LE，**绝对文件偏移**，目录路径入字符串池（原 `dir_id:u8` 方案废弃） |
| 4 | `path_len` | 2 | u16 LE，路径字节数 |
| 6 | `pad0` | 2 | u16 LE = 0，非 0 作废 |
| 8 | `mtime_ns` | 8 | i64 LE；**缺席哨兵 = 0** |
| 16 | `ino` | 8 | u64 LE；**缺席哨兵 = 0**（与 mtime_ns 同时为 0 才算缺席） |

- **行序**：按路径字节升序（`BTreeSet` 序），读侧校验乱序即作废。
- **语义**（规格，本件不实现 stat）：指纹集合 = 构建器**实际查阅过**的全部候选路径的父目录
  （命中与未命中都算）；「构建时缺席、现在在场」→ 不匹配 → 重建；反之亦然。
  本件只落 codec 与排序/哨兵不变式；stat 扫描、二级失效判定属 A3.2。

## 5. 覆盖区段（F5/CONTRACT I-5）

- 记录 = `(start: u32 LE, end: u32 LE)`，**8 B 紧排、无对齐填充**；闭区间、按 `start` 升序、
  互不重叠且**互不相邻**（`end + 1 < next.start`；相邻即合并）。
- **来源唯一**：写侧覆盖区段必须由 A1b 同一建表函数 `CoverageTable::from_face`
  （`text.rs:388-416`：cmap 枚举 → 逐点 `glyph_index` 复检 → 升序 + 相邻合并）产出，
  禁止另写 cmap 枚举逻辑。
- **共享**：多条目可同 `cov_off`（可变字体 7 实例同 cmap，写侧按字节去重）；
  读侧不做去重假设，按条目各读。
- 读侧不变式：`[cov_off, cov_off + 8×cov_ranges)` ⊆ `[cov_off_head, cov_off_head+cov_len)`；
  乱序 / 重叠 / 相邻未合并 / 越界 → 整份作废。

## 6. 字符串池

- `str_off` 起 `str_len` 字节，UTF-8（不要求 NUL 终止）；条目/指纹的 `(off, len)`
  是**绝对文件偏移**（I-2 同一条规则，无第二基准），切片必须完整落在
  `[str_off, str_off + str_len)` 内（`checked_add`），越界 → 整份作废。
- 池内字节读侧不做编码校验（`std::str::from_utf8` 失败 → 条目按诊断值处理不拒；
  路径字节参与 §4 stat 时才可能被 OS 拒绝——本件不触 stat）。

## 7. `stack_identity`（F7/B1，逐字节与现码一致）

```text
h = 0xcbf29ce484222325
for e in entries:                      # 栈序 = 条目序 = 回退链序
    for b in read_bytes(e.path):       # e.path 指向的字体文件全部字节（非路径字符串字节）；VF 同文件按条目重复喂 7 次
        h ^= b; h = h.wrapping_mul(0x100000001b3)
    h ^= e.collection_index as u64; h = h.wrapping_mul(0x100000001b3)
```

参数与 `TextEngine::refresh_identity`（`text.rs:1189-1200`）完全一致：标准 FNV-1a 64、
逐字节先 XOR 后乘、条目间混入 `collection_index`。写侧读文件字节计算；不哈希头部、
不哈希路径。`content_hash`（I-8）= 同参数 FNV-1a 64 over 单文件全部字节，
**不混 `collection_index`**，同文件多条目共享同值。

**客户端采信条款（2026-11-01 裁定 (a)）**：客户端在 `KANESUMI_FONT_INDEX=1`、索引已加载、
`font_count` 与字面数一致，且**全部字面按位**（条目 i ↔ 字面 i）`content_hash` 与
`collection_index` 命中时，可采信 `header.stack_identity` 作为本栈身份、跳过整趟栈 FNV。
任一不中（索引缺失 / 损坏、数目不符、任一位 content 不中）即回退现行逐字节自算，回退值与
不采信时逐位一致（栈哈希不依赖 content，采信尝试预置的 content_hash 留用）。本条款不改
本节值定义与写侧算法，`POLICY_VERSION` 不因此 bump；按位比对同时校验条目序，序错即不中。

## 8. `checksum`（F3/CONTRACT I-3）

= FNV-1a 64（同 §7 参数）over `[header_len, header_len + body_len)` = `[96, file_len)`：
条目区 + 字符串池 + 覆盖区 + 指纹区全部计入。**头部不计入**（`checksum` 自身在头部，
`built_unix_ns` 每次必变）；头部安全由 §1 上界检查负责。只查意外损坏，不防伪（威胁模型如实）。

## 9. 失效保位语义（F10/B2——规格记录，本件不实现）

- 索引文件不可变，条目**永不删行**，`font_id` == 下标恒成立。
- 覆盖信任**按条目**：一级 `stat` 过 → 用索引路径/覆盖区段；不过但文件可读 → 该条目退本地
  （字节照读 + 本地建表），槽位不动。
- 身份信任**按整栈**：任一条目一级不符 → 不信 `stack_identity`（整栈重算）；
  文件缺失/不可读 → 整份弃用全本地。
- 局部重载必须保持 `collection_index` / weight 实例元数据（调度者 2026-10-31 批注）。
- 本件不实现：无 stat 调用、无环境变量、不被 `text.rs`/harness 触达。

## 10. 读侧拒绝表（每种原因一个 `IndexError` 变体，全部整份作废）

`magic` / `abi` / `policy_ver` / `header_len` / `body_len` / `font_count`（>4096 或条目区越界）/
`flags`（≠1）/ `offset_out_of_bounds`（str/cov/dirfp 及条目 `cov`，含 overflow）/
`checksum` / `dirfp_order_or_count`（乱序 / >64 / 区越界）/ `truncated`（< 96 B）/
`cov_ranges`（覆盖区段乱序、重叠或相邻未合并，§5 语义；实现 `font_index.rs` `check_entries` → `CovRanges(i)`）。

> 勘误（2026-11-01）：`cov_ranges` 一项实现自 A3.0 起即拒绝、与 §5 一致，本表原漏列（c1101 03 夜 2 复核）。

## 11. 写侧布局（`write_index` 纯函数；顺序 = 头部 96 B → 条目区 → 字符串池 → 覆盖区 → 指纹区）

```text
body   = entries (64×N) ++ pool ++ cov ++ dirfp     # 条目区固定紧跟头部
header = 按 §2 各字段填值；body_len = body.len()
         checksum = FNV(body)；stack_identity = §7
out    = header ++ body                              # str/cov/dirfp_off 按此定值
```

## 12. 验收映射（任务书七条 ↔ 本规格）

1. `cargo test -p kanesumi-canvas font_index` 全绿（§10 全变体 + 往返 + §5 不变式）。
2. 真机 TTC 栈 3000 码点 × 每字面 `index.has(c)` ↔ `Face::glyph_index(c).is_some()` **双向相等**；
   缺字体 → **失败并打印原因**（跳过不算通过）。
3. 同一栈写索引，`header.stack_identity` == `TextEngine::load_stack(..).identity()`，
   打印两端十六进制与条目数。§7 客户端采信条款生效时 `identity()` 为采信值；
   采信前提（按位 content 全中）即采信与自算的等价判定，回归用例锁定
   「采信值 == 自算值」与「任一不中回退自算」。
4. ≥10 损坏变体 `Err` 不 panic，打印「变体 → 错误变体」表。
5. canvas 全量测试不退化；6. `font_index.rs` 无 `unsafe`；7. 白名单干净。
   白名单外（已批）：`text.rs` 四处 `pub(crate)`（`CoverageTable` / `from_face` / `has` /
   `collection_index_matching`）+ 只读 ranges 访问器（若实现仍被挡，加一处不改行为）。
