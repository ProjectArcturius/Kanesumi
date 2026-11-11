# Kanesumi 接口稳定策略（落地前必读）

> 触发点：2026-11-07 用户明确要求 —— **一旦 Kanesumi 落地，改接口就是让上层应用（settings /
> librarian / launcher）编译失败**，所以必须在这一阶段把接口问题定死，而不是边用边改。
>
> 本文是「能不能动接口」的唯一判据。与 `docs/CANON_VS_TEMPORARY.md`（临时**数值**登记）互补：
> 那份管数值，这份管**签名**。

## §一 四条硬规矩

1. **先设计后落地**：新能力先写清「谁消费、怎么用、失败怎么表现」，再写实现。
   没有消费方的 API 不加（现在缺的是消费方，不是 API 数量）。
2. **只增不改**：落地后删字段、改名字、改语义一律禁止；要改就加新名字并把旧的标记废弃
   （见 §三）。
3. **变更必须过闸门**：`docs/API_SURFACE.txt` 是公共签名的快照，
   `cargo test -p kanesumi-element --test api_surface` 会红。红了只有两种处理：
   改回去，或**显式重新生成 + 逐行 review + 在提交信息里说明为什么可以对外**。
4. **进 harness 与进 controls 同价**：往 `kanesumi-harness` 加 `pub` 项，与往
   `kanesumi-controls` 加 `pub` 项走同一道门 —— 同样先写清「谁消费、怎么用、失败怎么表现」，
   再写实现。理由：harness 的 `TreeHost` / `App` / `platform` 就是三个真应用
   （settings / launcher / librarian）直接绑的壳，多一个 `pub` 方法就多一条应用侧要长期
   兼容的契约；「它在壳里、不在控件库里」不是随手加的理由。

## §二 哪些算公共接口

`kanesumi-core` / `kanesumi-canvas` / `kanesumi-element` / `kanesumi-controls` /
`kanesumi-a11y` 五个核心 crate，加上应用壳 `kanesumi-harness`，一共六节；它们的
`pub fn` / `pub struct` / `pub enum` / `pub trait` 全部算（快照约 1.7k 条；2026-11-11 纳入时
harness 一节 240 条）。

harness 纳入的理由：settings / launcher / librarian 三个真应用绑的正是 harness 的
`TreeHost` / `App` / `platform`，它从三个应用接线那天起就已经是应用侧契约。此前「harness
暂不计入，接第一个外部应用时纳入」的前提那时就过期了 —— 闸门等于放养了最该管的一层。

`kanesumi-gallery` 仍**不计入**：它是演示程序，没有下游消费方，它的 `pub` 项不构成契约。

## §三 兼容性对照表

| 改动 | 判定 | 处理 |
|---|---|---|
| 新增 `pub fn` / 新类型 / 新字段（带 `Default`） | 兼容 | 过闸门即可 |
| 给枚举加变体 | **破坏**（下游 exhaustive match） | 枚举标 `#[non_exhaustive]`，且从**第一次**发布就这么标 |
| 给结构体加字段 | **破坏**（下游字面量构造） | 结构体标 `#[non_exhaustive]` + 提供 `Default` 与链式构造 |
| 改函数名 / 改参数顺序 / 改返回类型 | **破坏** | 加新函数，旧函数标 `#[deprecated(note = "…")]`，至少留一个发布周期 |
| 改默认值（行为变化） | 半破坏 | 允许，但必须写进 `CANON_VS_TEMPORARY` 或对应 SPEC 节，并带回归测试 |
| 收紧参数校验（原先接受、现在拒绝） | 半破坏 | 同上；宁可新增严格版本，不悄悄收紧 |
| 删除 `pub` 项 | **破坏** | 先 `deprecated`，再删 |

### 已标 `#[non_exhaustive]` 的枚举（2026-11-07 / 2026-11-11 补）

`AccessRole`（无障碍角色，已从 5 个长到 12 个）与 `LayoutRole`（布局角色，将来可能加页脚 / 侧栏）
都已标注。下游必须留 `_ => …` 兜底，代价是一行 match；换来的是「框架加角色不会让应用编译失败」。
新加的**可扩展**枚举一律从第一天就这么标，别等落地后再补（补的那次本身就是破坏）。

2026-11-11 补：harness 的表面角色 `EtherRole` 与表面类型 `SurfaceKind` 也已标注（§五）。
它们是合成器与五个应用（settings / launcher / librarian / apps/lock / ceyboard）之间的协议面，
标注后「通知表面」「系统对话框表面」只能以**新增变体**进来。

## §四 闸门怎么跑

```text
# 校验（CI / 提交前）
cargo test -p kanesumi-element --test api_surface

# 确认要改接口时，重新生成并逐行 review
cargo run -q -p kanesumi-element --example api_surface > docs/API_SURFACE.txt
git diff --stat docs/API_SURFACE.txt
```

扫描规则在 `kanesumi-element/tests/api_surface_scan.rs`（生成与校验共用一份，避免两套规则漂移）。
只做**行级签名**比对，不做语义解析：语义比对会随宏与条件编译抖动，反而制造噪音。

## §五 当前明确「还会动」的区域（落地前必须收敛）

| 区域 | 现状 | 结论 |
|---|---|---|
| 对话框按钮 | ✅ **已真节点化**（`MetroDialogButton` + `LayoutRole::Commands` 分区排布，2026-11-07） | 现存语义已按此定稿：`show_modal*` 自动插按钮、焦点落默认按钮、←/→ 挪框架焦点、`access_items` 不再合成按钮。此后只能**增**（如自定义命令区控件），不能再改回自绘 |
| 无障碍平台桥（AT-SPI） | ✅ **已落地、接线、并在 Linux 真机验收通过**（2026-11-07）：`kanesumi-a11y` 把快照映射成 `accesskit::TreeUpdate`（8 条测试）；`TreeHost::enable_a11y()`（harness feature `a11y`）每帧发布；`scripts/a11y_linux.sh` 在 Debian/T14p 上通过 AT-SPI 的 D-Bus 接口读出 9 个节点名字（含对话框三个按钮），判定 5/5 通过 | 纯新增：新 crate + 两个可选 feature，不动任何现有签名。AT 读的一律是快照，故后续加「描述 / 动作回灌」都只是本 crate 的加法。未验的只剩「Orca 真的念出来」（E6 验收门） |
| 画布路径能力 | 闭集图元（无任意路径）；`SceneCommand` **已标 `#[non_exhaustive]`**（2026-11-07） | 若加 `Scene::fill_path`，属**新增**：加图元不再破坏下游穷举 `match`（下游写 `_ => {}` 即可）。四个消费方（`kanesumi-render` CPU/GPU、harness 损伤、element 夹具）都已补显式兜底并留了「新图元必须在此实现」的警告 |
| 命名遗留 | ✅ **三条已收敛**（2026-11-07）：`overflowing` → `has_overflow`、`visible_rows` → `compute_visible_rows`、`path_at` → `try_path_at`；旧名一律 `#[deprecated]` 转发，`tests/deprecated_names.rs` 挡着「旧名坏了没人知道」 | 剩下的只有 `visible_rows` **要不要缓存**（纯性能，不动签名，可落地后再做）。`try_*` 约定成为新代码硬规矩，见 `docs/DEV_GUIDE.md` §八·五 |
| `EventCtx` 的读查询 | `children` / `child_props` / `parent` / `focus_id`（2026-11-07 补齐） | 只读、纯新增。组合型控件要按角色分区、在子节点间挪焦点就必须能读结构；不加则会各自另存一份「当前项」与框架焦点打架 |
| 表面角色 `EtherRole`（`kanesumi-harness/src/role.rs`） | ✅ **已定型（本班）**：8 个变体（`desktop` / `browser` / `topbar` / `dock` / `launcher` / `candidate` / `lock` / `shot-select`）加 `#[non_exhaustive]`（2026-11-11）。五个消费方（settings / launcher / librarian / apps/lock / ceyboard）的角色分派处已补显式降级分支 | 已定型（本班）。角色字符串（`app_id` / namespace）是协议面，不再改；加「通知」「系统对话框」角色只许**新增变体**，下游 `_` 兜底保证不编译失败 |
| 表面类型 `SurfaceKind`（`kanesumi-harness/src/role.rs`） | ✅ **已定型（本班）**：6 个变体加 `#[non_exhaustive]`（2026-11-11）；`EtherRole::surface_kind()` 的现有映射逐条钉死（`role.rs` 单测） | 已定型（本班）。现有映射语义冻结；新增表面类型只加变体，外壳分派与弹层放置对未知类型必须显式降级 |

## §六 判定流程（每次想动接口时照走）

1. 这条签名有消费方吗？没有 → 不加，或先加进「实验区」并在文档标注（目前无实验区，
   所以等于「先找消费方」）。
2. 上游应用会不会因此编译失败？会 → 走 §三 的兼容路径（新增而非修改）。
3. 改完跑闸门，红了就重新生成快照并**逐行**看 diff。
4. 提交信息写清「为什么这次变动可以对外」。
