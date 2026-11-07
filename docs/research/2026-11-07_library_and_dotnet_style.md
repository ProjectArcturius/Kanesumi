# 对照研究：图形库 × C#/.NET 源码风格（2026-11-07）

> 目的：回答「Kanesumi 的 API 形状与画布能力，和成熟库差在哪」。
> 采样：tiny-skia `5d47547`、vello `c161b8a`、egui `3c2531c`、iced `8ff48c4`（均浅克隆只读，
> 在 `Ether/target/refs/`）；.NET 侧引 Framework Design Guidelines 公开页面（2023-10-03 版）。

## §Ⅰ 画布层对照（tiny-skia / vello / 我们）

| 维度 | tiny-skia | vello | Kanesumi（`kanesumi-canvas`） |
|---|---|---|---|
| 场景表示 | 即时：`PixmapMut::fill_path` 立刻光栅 | 保留：先编码成 scene 再交 CPU/GPU 后端 | **保留**：`Scene` 命令列表（`fill_rect` / `fill_rounded_rect` / `stroke_*` / `text` / `triangle` / clip 栈） |
| 任意路径 | 核心能力（`Path` + `PathBuilder`，边表 + 扫描线） | 核心能力（`kurbo` 形状 → 编码 → 展平） | **没有**：只有闭集图元，图标走 SVG→位图 |
| 矩形快路径 | 有：`painter.rs` 里 `fill_rect` 在「恒等变换且不需要 tiler」时直接 `scan::fill_rect[_aa]`，否则退回 `PathBuilder::from_rect` + `fill_path` | 无单独矩形路径（统一走编码/展平） | 有：`fill_rect_aa` 解析式覆盖（无路径构造） |
| 抗锯齿 | 覆盖度扫描线（`alpha_runs` / `mask`） | GPU 解析式 + CPU 后端（`vello_cpu`） | 解析式矩形 + 三角形光栅（`kanesumi-render` CPU） |
| 文字 | 无（不含文字栈） | 外包 parley / skrifa | 自带 fontdue 整形 + 字形图集 |

**结论**：位置是对的 —— 「保留式场景 + 矩形解析快路径」正是 vello（保留）与 tiny-skia（快路径）
两者的交集。真正的能力边界是**没有任意路径**：圆角矩形够用，但圆弧进度圈、矢量图标着色、
细线图表都要靠位图或三角形硬凑。这条要写进正典，而不是留在「以后再说」。

## §Ⅱ 控件树对照（egui / iced / 我们）

| 维度 | egui | iced | Kanesumi 元素树 |
|---|---|---|---|
| 树的生命周期 | 每帧重建（即时模式） | 保留 + `Message` 上行 | **保留** + 事件冒泡到祖先 |
| 状态存放 | 调用方持有 | `Application::update` 集中 | 控件自身字段 + `Tree::edit` |
| 布局 | 逐帧 `Ui` 立即排版 | `layout` 返回 `Node` | `measure` / `arrange` + 框架布局缓存 |
| 无障碍 | accesskit 集成（官方） | 部分（`accesskit` 特性） | 自建 `access_nodes` + 合成项（无 AT-SPI 桥） |

**结论**：我们与 iced 同代（保留式），比 egui 更适合「不每帧重建」的系统界面；差距不在模型，
而在**平台接线**（egui/iced 都接了 accesskit，我们只到数据层）。这也是 AT-SPI 桥优先级高的原因。

## §Ⅲ 命名与 API 形状（.NET 规则 → 我们）

| .NET 规则（原文要点） | 我们的现状 | 判定 |
|---|---|---|
| 方法名用动词 / 动词短语 | `visible_rows` / `item_by_path` / `content_height` 均为名词短语 | **不符**（见下） |
| 属性用名词 / 形容词，且**廉价、无副作用** | `visible_rows()` 每次 `Vec` 分配 + `path.clone()`（源码里自己写了性能备注） | **不符**：名字像属性，代价像方法 |
| 事件用动词 + 时态（`Closing` / `Closed`） | `DialogResult` / `ExpanderToggled` / `SliderValueChanged` / `TreeItemInvoked` | **基本符合**，`DialogResult` 是名词（可保留：它是结果不是事件） |
| 布尔属性用肯定式，可加 `Is` / `Can` / `Has` | `overflowing()` / `hit_test()` / `wants_anim()` | **多半符合**；`overflowing` 建议 `has_overflow` |
| 不用缩写 / 不造生僻名（`CanScrollHorizontally` 优于 `ScrollableX`） | `fan` / `pane` / `dlg` / `msg`（局部变量与字段混用） | 公共 API 基本达标；**私有字段有缩写**，可读性可再提 |
| `Try*` 模式：可能失败且不该抛异常的查询，返回 bool + out | 我们返回 `Option`：`item_by_path` / `find_item` / `path_at` | **语义等价但名字没信号**：调用方看不出「这是可能为空的查询」 |

**就地可做（零 API 破坏）**：新写的「可能查不到」的查询统一 `try_*` 前缀；新写的布尔查询统一
`is_*` / `has_*`。已有 API 不批量改名（会动 7 个控件的公共面），留一张「改名清单」等拍板。

## §Ⅳ 建议动作（按性价比排序）

1. **【已落地】DEV_GUIDE 增「API 命名与 .NET 对照」一节**：把上表浓缩成写作规则，新代码照此写。
2. **【留案·低成本】`visible_rows` 改缓存**：`expand` / 选择 / 过滤变化时失效重算，命中与无障碍
   复用同一份；预计 `access_items` + `hit_test` 路径少一次全量分配。风险：失效点漏掉会显示旧树。
3. **【留案·中成本】`try_*` 改名清单**：`item_by_path` → `try_item`、`path_at` → `try_path_at` 等；
   纯重命名、有编译器兜底，但要同步 7 处消费者与文档。
4. **【留案·高成本】画布加路径能力**：`Scene::fill_path` + CPU 扫描线（对标 tiny-skia
   `edge_builder` + `scan`）。它能替掉「圆角矩形 + 三角形硬凑」的一批绘制；代价是 `kanesumi-render`
   多一条光栅路径与一批像素回归。
5. **【维持】AT-SPI 桥**：§Ⅱ 已说明差距在接线不在模型；用户已批准新增依赖，这项从「待授权」转为
   「可开工」，建议作为下一批首项。
