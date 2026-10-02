# k-interact —— 交互正典落地快照

任务 `k-interact`（2026-10-02）的视觉证据：键盘焦点态的双层焦点框（正典 §Ⅳ 外 2px +
内 1px 对比色）。用 `focus_snapshot` 示例以小窗口 + 8× scale 放大生成。

## 图

| 文件 | 主题 | 观察 |
|---|---|---|
| `focus_dark.png` | 暗色 | 外圈 2px 强调色 `focus_stroke`，内圈 1px **黑**；再内侧为按钮底色 |
| `focus_light.png` | 亮色 | 外圈 2px 强调色，内圈 1px **白**；再内侧为按钮底色 |

两图均为 160 × 72 逻辑像素、8× 光栅（1280 × 576）。焦点框只在**键盘焦点**时出现
（指针点击不画），由框架统一绘制（`kanesumi-element/src/tree.rs`），控件自绘焦点的
旧路径同样改为 2 + 1（`kanesumi-controls/src/focus.rs::draw_focus_ring`）。

## 生成

```bash
cargo run -p kanesumi-gallery --example focus_snapshot -- --scheme dark  --snapshot docs/research/k_interact/focus_dark.png  8
cargo run -p kanesumi-gallery --example focus_snapshot -- --scheme light --snapshot docs/research/k_interact/focus_light.png 8
```

字体取自系统（`platform::find_font`）；图上中文「确定」为按钮标签。

## 临时项

内圈「对比色」的方向（深黑 / 浅白）无一手源，登记为 `docs/CANON_VS_TEMPORARY.md` T23。
