# Kanesumi 接口稳定策略（落地前必读）

> 触发点：2026-11-07 用户明确要求 —— **一旦 Kanesumi 落地，改接口就是让上层应用（settings /
> librarian / launcher）编译失败**，所以必须在这一阶段把接口问题定死，而不是边用边改。
>
> 本文是「能不能动接口」的唯一判据。与 `docs/CANON_VS_TEMPORARY.md`（临时**数值**登记）互补：
> 那份管数值，这份管**签名**。

## §一 三条硬规矩

1. **先设计后落地**：新能力先写清「谁消费、怎么用、失败怎么表现」，再写实现。
   没有消费方的 API 不加（现在缺的是消费方，不是 API 数量）。
2. **只增不改**：落地后删字段、改名字、改语义一律禁止；要改就加新名字并把旧的标记废弃
   （见 §三）。
3. **变更必须过闸门**：`docs/API_SURFACE.txt` 是公共签名的快照，
   `cargo test -p kanesumi-element --test api_surface` 会红。红了只有两种处理：
   改回去，或**显式重新生成 + 逐行 review + 在提交信息里说明为什么可以对外**。

## §二 哪些算公共接口

`kanesumi-core` / `kanesumi-canvas` / `kanesumi-element` / `kanesumi-controls` 的
`pub fn` / `pub struct` / `pub enum` / `pub trait` 全部算（快照里 4 节，约 1300 条）。
`kanesumi-harness` / `kanesumi-gallery` 目前只有内部消费方，**暂不计入**；接第一个外部应用时
一并纳入。

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

### 已标 `#[non_exhaustive]` 的枚举（2026-11-07）

`AccessRole`（无障碍角色，已从 5 个长到 12 个）与 `LayoutRole`（布局角色，将来可能加页脚 / 侧栏）
都已标注。下游必须留 `_ => …` 兜底，代价是一行 match；换来的是「框架加角色不会让应用编译失败」。
新加的**可扩展**枚举一律从第一天就这么标，别等落地后再补（补的那次本身就是破坏）。

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
| 无障碍平台桥（AT-SPI） | 只有数据层（`access_nodes` / `AccessItem`） | 桥接会把 `AccessSnapshot` 变成平台树的输入；预期只**新增**（`kanesumi-a11y` 桥 crate + harness 接线），不改现有签名 |
| 画布路径能力 | 闭集图元（无任意路径） | 若加 `Scene::fill_path`，属**新增**；但 `SceneCommand` 若加变体会破坏下游 match —— 届时该枚举标 `#[non_exhaustive]` |
| 命名遗留 | `visible_rows`（名像属性、代价像方法）、`try_*` 缺失 | **落地前改**（现在改只动我们自己；落地后改是破坏）。清单见 `docs/research/2026-11-07_library_and_dotnet_style.md` §Ⅲ |
| `EventCtx` 的读查询 | `children` / `child_props` / `parent` / `focus_id`（2026-11-07 补齐） | 只读、纯新增。组合型控件要按角色分区、在子节点间挪焦点就必须能读结构；不加则会各自另存一份「当前项」与框架焦点打架 |

## §六 判定流程（每次想动接口时照走）

1. 这条签名有消费方吗？没有 → 不加，或先加进「实验区」并在文档标注（目前无实验区，
   所以等于「先找消费方」）。
2. 上游应用会不会因此编译失败？会 → 走 §三 的兼容路径（新增而非修改）。
3. 改完跑闸门，红了就重新生成快照并**逐行**看 diff。
4. 提交信息写清「为什么这次变动可以对外」。
