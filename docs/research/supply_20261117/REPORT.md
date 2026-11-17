# t14p 供应门地基复核（2026-11-17）

- 机器：t14p / Debian 13，cargo 1.97.1；Windows 未运行 cargo。
- Fedora 交件使用 `oc/k-dock-v2` 的 e594018（包含 AppDriver），没有用会丢失 AppDriver 的 Fedora main 95e9e1a 替换应用基线。
- 修复：旧 `indicator_rect -> Rect` 与历史几何保留并废弃，新增 `try_indicator_rect -> Option<Rect>`；已有断言未因本修复放宽。
- 用户批准的门修复分为两个独立提交：glyph 倍率用例隔离子进程；通知卡关闭 glyph 探针适配。
- 集成固定主题入口，保留 pin/unpin/pinned 与 AppDriver builder；应用构造前读主题时必须先 pin，builder 不能倒推构造时序。
- `cargo test --workspace --offline --no-fail-fast`：2053 passed / 0 failed / 5 ignored，61 个结果段；忽略项均既有，本轮未新增。
- `cargo clippy --workspace --all-targets --offline`：退出 0。仓内既有告警仍在，改动文件未见新增告警；`-D warnings` 在 accent.rs 的既有 single_match 上失败，不记成通过。
- 相对应用基线 1baf853，API 快照移除 0 行、增加 19 行；旧定位线函数签名保留。
- Dock 视觉：通过 `snapshot::render_png` 生成深浅两主题、1×/2× 四张图，源例保存在 `~/.local/state/supply_dock_review.rs`；调度者亲看两张 2× 图。
- 像素证据：暗色 hover/rest/active = 46/23/46，灰线 51；浅色 = 230/255/230，灰线 214；1×/2× 的探针读数一致。
- 截图仅验证控件几何与状态色；图标由调用方 blit，未把空图标位当作 launcher 的完整视觉验收。
- 日志：`~/.local/state/kanesumi-supply-integrated-20261117.log`、`kanesumi-supply-integrated-clippy-20261117.log`；截图在 Ether `docs/research/kanesumi_supply_20261117/`。
- 供应门尚未通过：消费侧量尺仍为自己顶 5/9；通知 / SSD 桥仍需身份与缓存契约，参 Ether 的 `docs/research/n1112_element_bridge/IDENTITY_REVIEW.md`。
