// osd.rs —— OSD 浮层控件（音量 / 亮度 / 媒体 / CapsLock 的自绘提示）。
//
// 供应门 §Ⅰ 的量尺把 `settings/src/topbar/osd.rs`（1552 行、22 处 `Rect::new`）算成 TopBar 的一部分，
// 2026-11-13 复核后更正：**OSD 自成一件**（`docs/research/2026-11-13_topbar_rect_audit.md` §二）。
// 本控件把它的几何与消失计时定死，settings 侧只剩“显示什么”。
//
// 语义（参 `docs/OSD_DESIGN.md`）：
// - 卡片贴屏幕下沿居中，离下沿 48pt；
// - 图标 32 + 标题一行 + （音量 / 亮度）进度条 120×4；
// - 显示时长 2 s，新事件**重置**计时（不是排队）；
// - 静音时不画进度条内容但**保留轨道元素**（元素数稳定，损伤才稳）。

use kanesumi_canvas::{Scene, TextAlign, TextEngine};
use kanesumi_core::typography::{MetroTypography, TextStyle};
use kanesumi_core::{MetroTheme, Point, Rect, sizes};

/// OSD 显示时长（毫秒）。参 `docs/OSD_DESIGN.md`。
pub const OSD_DURATION_MS: f64 = 2000.0;

/// 要显示的内容。
#[derive(Debug, Clone, PartialEq)]
pub enum OsdKind {
    /// 音量（`level` 0..1，`muted` 静音）。
    Volume {
        /// 0..1。
        level: f32,
        /// 是否静音。
        muted: bool,
    },
    /// 亮度（`level` 0..1）。
    Brightness {
        /// 0..1。
        level: f32,
    },
    /// 媒体（`playing` 播放中）。
    Media {
        /// 是否在播。
        playing: bool,
    },
    /// 大写锁定（`on` 开启）。
    CapsLock {
        /// 是否开启。
        on: bool,
    },
    /// 其它一句话提示。
    Text {
        /// 提示文本。
        label: String,
    },
}

impl OsdKind {
    /// 是否有进度条（只有音量 / 亮度有）。
    pub fn has_level(&self) -> bool {
        matches!(self, OsdKind::Volume { .. } | OsdKind::Brightness { .. })
    }

    /// 进度值（0..1，已夹紧）。
    pub fn level(&self) -> f32 {
        match self {
            OsdKind::Volume { level, muted } => {
                if *muted {
                    0.0
                } else {
                    level.clamp(0.0, 1.0)
                }
            }
            OsdKind::Brightness { level } => level.clamp(0.0, 1.0),
            _ => 0.0,
        }
    }

    /// 标题文字（图标位之外的唯一文字行）。
    pub fn label(&self) -> String {
        match self {
            OsdKind::Volume { level, muted } => {
                if *muted {
                    "静音".to_string()
                } else {
                    format!("音量 {}%", (level.clamp(0.0, 1.0) * 100.0).round() as u32)
                }
            }
            OsdKind::Brightness { level } => {
                format!("亮度 {}%", (level.clamp(0.0, 1.0) * 100.0).round() as u32)
            }
            OsdKind::Media { playing } => {
                if *playing {
                    "播放".to_string()
                } else {
                    "暂停".to_string()
                }
            }
            OsdKind::CapsLock { on } => {
                if *on {
                    "大写锁定：开".to_string()
                } else {
                    "大写锁定：关".to_string()
                }
            }
            OsdKind::Text { label } => label.clone(),
        }
    }

    /// 图标字形（外壳可换成主题图标；这里给一个可测的稳定字符）。
    pub fn glyph(&self) -> &'static str {
        match self {
            OsdKind::Volume { muted: true, .. } => "mute",
            OsdKind::Volume { .. } => "volume",
            OsdKind::Brightness { .. } => "brightness",
            OsdKind::Media { playing: true } => "play",
            OsdKind::Media { .. } => "pause",
            OsdKind::CapsLock { .. } => "caps",
            OsdKind::Text { .. } => "info",
        }
    }
}

/// OSD 卡片的几何（绝对坐标）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OsdGeometry {
    /// 卡片。
    pub card: Rect,
    /// 图标位。
    pub icon: Rect,
    /// 标签。
    pub label: Rect,
    /// 进度轨道（无进度条时零尺寸）。
    pub track: Rect,
    /// 进度已填充段（无进度条时零尺寸）。
    pub fill: Rect,
}

impl OsdGeometry {
    /// 量一张 OSD 卡：贴底居中，离下沿 48。
    pub fn measure(engine: &TextEngine, kind: &OsdKind, viewport: Rect) -> Self {
        let style = MetroTypography::metro().body_medium;
        let label = kind.label();
        let label_w = engine.measure(&label, style.size);
        let has_level = kind.has_level();
        let width = (sizes::ICON_32 + 12.0 + label_w + 56.0).max(200.0);
        let height = if has_level { 84.0 } else { 64.0 };
        let card = Rect::new(
            viewport.origin.x + (viewport.size.width - width) / 2.0,
            viewport.origin.y + viewport.size.height - 48.0 - height,
            width,
            height,
        );
        let icon = Rect::new(card.origin.x + 16.0, card.origin.y + 16.0, sizes::ICON_32, sizes::ICON_32);
        let label_rect = Rect::new(
            icon.right() + 12.0,
            icon.origin.y + (sizes::ICON_32 - style.line_height) / 2.0,
            (card.right() - 16.0 - (icon.right() + 12.0)).max(0.0),
            style.line_height,
        );
        let (track, fill) = if has_level {
            let t = Rect::new(
                card.origin.x + 16.0,
                card.bottom() - 24.0,
                card.size.width - 32.0,
                4.0,
            );
            let f = Rect::new(t.origin.x, t.origin.y, t.size.width * kind.level(), t.size.height);
            (t, f)
        } else {
            (
                Rect::new(card.origin.x, card.origin.y, 0.0, 0.0),
                Rect::new(card.origin.x, card.origin.y, 0.0, 0.0),
            )
        };
        Self {
            card,
            icon,
            label: label_rect,
            track,
            fill,
        }
    }
}

/// OSD 控件：显示 / 计时 / 渲染。
#[derive(Debug, Clone)]
pub struct MetroOsd {
    kind: Option<OsdKind>,
    left_ms: f64,
}

impl Default for MetroOsd {
    fn default() -> Self {
        Self::new()
    }
}

impl MetroOsd {
    /// 新建（不显示）。
    pub fn new() -> Self {
        Self {
            kind: None,
            left_ms: 0.0,
        }
    }

    /// 显示一条；**重置**计时（新事件不排队，直接替换）。
    pub fn show(&mut self, kind: OsdKind) {
        self.kind = Some(kind);
        self.left_ms = OSD_DURATION_MS;
    }

    /// 立即隐藏。
    pub fn hide(&mut self) {
        self.kind = None;
        self.left_ms = 0.0;
    }

    /// 是否显示中。
    pub fn visible(&self) -> bool {
        self.kind.is_some()
    }

    /// 当前内容。
    pub fn kind(&self) -> Option<&OsdKind> {
        self.kind.as_ref()
    }

    /// 剩余显示时长（毫秒）。
    pub fn remaining_ms(&self) -> f64 {
        self.left_ms
    }

    /// 推进一帧：到点自动隐藏。
    pub fn tick(&mut self, dt_ms: f64) -> bool {
        if self.kind.is_none() {
            return false;
        }
        self.left_ms -= dt_ms.max(0.0);
        if self.left_ms <= 0.0 {
            self.hide();
            return true;
        }
        false
    }

    /// 渲染。未显示 → 什么都不画。
    pub fn render(&self, theme: &MetroTheme, engine: &TextEngine, viewport: Rect, scene: &mut Scene) {
        let Some(kind) = &self.kind else {
            return;
        };
        let c = &theme.colors;
        let g = OsdGeometry::measure(engine, kind, viewport);
        // 卡片底 + 图标位 + 标签 + 轨道 + 填充。
        scene.fill_rect(c.surface, g.card);
        scene.fill_rect(c.divider, g.icon);
        scene.label(
            kind.label(),
            g.label,
            c.on_surface,
            MetroTypography::metro().body_medium,
            TextAlign::Left,
        );
        if kind.has_level() {
            scene.fill_rect(c.divider, g.track);
            scene.fill_rect(c.primary, g.fill);
        }
    }

    /// 图标位中心（外壳据此画真图标；与渲染同源，避免两处算两遍）。
    pub fn icon_center(&self, engine: &TextEngine, viewport: Rect) -> Option<Point> {
        let kind = self.kind.as_ref()?;
        let g = OsdGeometry::measure(engine, kind, viewport);
        Some(Point::new(
            g.icon.origin.x + g.icon.size.width / 2.0,
            g.icon.origin.y + g.icon.size.height / 2.0,
        ))
    }
}

/// 供调用方复用：字体样式（与渲染同源）。
pub fn osd_text_style() -> TextStyle {
    MetroTypography::metro().body_medium
}
