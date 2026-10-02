# k-tooltip 快照

工具提示（CONTROL_SPEC §48）与窄宽 BreadcrumbBar（§18）的视觉核对样张。

## 图

| 文件 | 内容 |
|---|---|
| `tooltip_dark.png` | 深色主题：悬停「应用」按钮满 500 ms 后，提示气泡在其下方显示；长文按最大宽 320 换行。 |
| `tooltip_light.png` | 同上，浅色主题。 |
| `breadcrumb_narrow.png` | 深色主题：面包屑被夹到 140 px，前缀折进溢出 `…`，末段仍放不下 → 末段省略号截断。 |

## 重新生成（任意平台，CPU 光栅）

```bash
cargo run -p kanesumi-gallery --example tooltip_demo -- --scheme dark  --snapshot docs/research/k_tooltip/tooltip_dark.png  3
cargo run -p kanesumi-gallery --example tooltip_demo -- --scheme light --snapshot docs/research/k_tooltip/tooltip_light.png 3
cargo run -p kanesumi-gallery --example breadcrumb_narrow -- --snapshot docs/research/k_tooltip/breadcrumb_narrow.png 3
```

`tooltip_demo` 会在 `render_png` 的帧序列里越过提示初始延迟，故最终帧必含气泡。
