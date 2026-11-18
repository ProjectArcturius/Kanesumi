//! 顶栏左上 OSD 卡片，参 CONTROL_SPEC §58。

use kanesumi_canvas::{Icon, Scene, TextAlign};
use kanesumi_core::{MetroTheme, Point, Rect, Size, ThemeColor, sizes};
use kanesumi_element::{Event, EventCtx, PointerButton};

const GRID: sizes::Spacing = sizes::Spacing::base4();
const PAD: f32 = GRID.s16;
const PAD_TOP: f32 = sizes::OSD_OVERLAY_PAD_TOP;
const HEADER_H: f32 = sizes::OSD_OVERLAY_HEADER_HEIGHT;
const TRACK_GAP: f32 = GRID.s12;
const TRACK_H: f32 = GRID.s4;
const BASIC_H: f32 = PAD_TOP + HEADER_H + TRACK_GAP + TRACK_H + PAD;
const STATUS_H: f32 = PAD_TOP + HEADER_H + PAD;
const ART: f32 = sizes::OSD_OVERLAY_ART_SIZE;
const MEDIA_ROW_H: f32 = PAD_TOP + ART + GRID.s12 + 1.0;
const MEDIA_H: f32 = MEDIA_ROW_H + BASIC_H - PAD_TOP + GRID.s12;
const MEDIA_TEXT_GAP: f32 = sizes::OSD_OVERLAY_MEDIA_TEXT_GAP;
const BUTTON: f32 = sizes::ICON_32;
const SLIDE: f32 = GRID.s8;
/// 顶栏静音保留原长度，只把轨道填充置淡到 45%。参 CONTROL_SPEC §58。
const MUTED_TRACK_OPACITY: f32 = 0.45;

/// 由宿主加载的图标，控件负责落点、着色与透明度。
#[derive(Default)]
pub struct OsdOverlayIcons {
    pub left: Option<Icon>,
    pub art: Option<Icon>,
    pub previous: Option<Icon>,
    pub play: Option<Icon>,
    pub pause: Option<Icon>,
    pub next: Option<Icon>,
}

/// 一帧的媒体行内容，不持有系统后端数据。
pub struct OsdOverlayMedia<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub playing: bool,
    pub can_previous: bool,
    pub can_play_pause: bool,
    pub can_next: bool,
}

/// 一帧的基本提示与媒体行。无标题时用整行显示状态文字。
pub struct OsdOverlayContent<'a> {
    pub caption: Option<&'a str>,
    pub value: &'a str,
    pub level: Option<f32>,
    pub muted: bool,
    pub media: Option<OsdOverlayMedia<'a>>,
    pub presence: f32,
}

/// 媒体行的三个动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdMediaButton {
    Previous,
    PlayPause,
    Next,
}

/// 发给宿主的媒体动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsdMediaClicked(pub OsdMediaButton);

/// 发给宿主的指针进入或离开事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsdHoverChanged(pub bool);

/// 带媒体操作的顶栏屏幕提示。宿主提供内容变化与动画时钟。
pub struct MetroOsdOverlay {
    pub icons: OsdOverlayIcons,
    hovered: bool,
    pressed: Option<OsdMediaButton>,
}

impl MetroOsdOverlay {
    pub fn new(icons: OsdOverlayIcons) -> Self {
        Self {
            icons,
            hovered: false,
            pressed: None,
        }
    }

    pub fn size(has_media: bool, has_progress: bool) -> Size {
        let height = if has_media {
            MEDIA_H
        } else if has_progress {
            BASIC_H
        } else {
            STATUS_H
        };
        Size::new(sizes::OSD_OVERLAY_WIDTH, height)
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }

    /// 24 pt 图标在标题行内垂直居中。
    pub fn icon_rect(basic: Rect) -> Rect {
        Rect::new(
            basic.origin.x + PAD,
            basic.origin.y + PAD_TOP + (HEADER_H - sizes::ICON_24) / 2.0,
            sizes::ICON_24,
            sizes::ICON_24,
        )
    }

    /// 数值贴右内边距，与标题行同高。
    pub fn value_rect(basic: Rect) -> Rect {
        Rect::new(
            basic.right() - PAD - sizes::OSD_OVERLAY_VALUE_WIDTH,
            basic.origin.y + PAD_TOP,
            sizes::OSD_OVERLAY_VALUE_WIDTH,
            HEADER_H,
        )
    }

    /// 标签按框上沿排字，先把文字行盒居中到目标区域。
    pub fn line_rect(area: Rect, line_height: f32) -> Rect {
        Rect::new(
            area.origin.x,
            area.origin.y + (area.size.height - line_height) / 2.0,
            area.size.width,
            line_height,
        )
    }

    pub fn render(
        &self,
        theme: &MetroTheme,
        rect: Rect,
        content: &OsdOverlayContent<'_>,
        scene: &mut Scene,
    ) {
        let alpha = content.presence.clamp(0.0, 1.0);
        let base = Rect::new(
            rect.origin.x,
            rect.origin.y - SLIDE * (1.0 - alpha),
            rect.size.width,
            rect.size.height,
        );
        scene.push_clip(rect);
        scene.fill_rect(ThemeColor::Surface.resolve(theme).with_alpha(alpha), base);
        scene.stroke_rect(
            ThemeColor::Divider.resolve(theme).with_alpha(alpha),
            base,
            1.0,
        );
        if let Some(media) = &content.media {
            self.paint_media(theme, base, media, scene, alpha);
        }
        let h = base.size.height.min(BASIC_H);
        let basic = Rect::new(base.origin.x, base.bottom() - h, base.size.width, h);
        self.paint_basic(theme, basic, content, scene, alpha);
        scene.pop_clip();
    }

    fn paint_basic(
        &self,
        theme: &MetroTheme,
        basic: Rect,
        content: &OsdOverlayContent<'_>,
        scene: &mut Scene,
        alpha: f32,
    ) {
        let foreground = ThemeColor::OnSurface.resolve(theme);
        let secondary = ThemeColor::OnSurfaceVariant.resolve(theme);
        let icon = Self::icon_rect(basic);
        if let Some(image) = &self.icons.left {
            scene.image_with_opacity(image, icon, Some(foreground), alpha);
        }
        let x = icon.right() + GRID.s8;
        let y = basic.origin.y + PAD_TOP;
        let Some(caption) = content.caption else {
            let style = theme.typography.body;
            let area = Rect::new(x, y, basic.right() - PAD - x, HEADER_H);
            scene.label(
                content.value.into(),
                Self::line_rect(area, style.line_height),
                foreground.with_alpha(alpha),
                style,
                TextAlign::Left,
            );
            return;
        };
        let value = Self::value_rect(basic);
        let area = Rect::new(x, y, value.origin.x - x - GRID.s8, HEADER_H);
        let style = theme.typography.caption;
        scene.label(
            caption.into(),
            Self::line_rect(area, style.line_height),
            secondary.with_alpha(alpha),
            style,
            TextAlign::Left,
        );
        let style = theme.typography.title;
        let value_color = if content.muted { secondary } else { foreground };
        scene.label(
            content.value.into(),
            Self::line_rect(value, style.line_height),
            value_color.with_alpha(alpha),
            style,
            TextAlign::Right,
        );
        if let Some(level) = content.level {
            let track = Rect::new(
                basic.origin.x + PAD,
                y + HEADER_H + TRACK_GAP,
                (basic.size.width - PAD * 2.0).max(0.0),
                TRACK_H,
            );
            scene.fill_rect(ThemeColor::Divider.resolve(theme).with_alpha(alpha), track);
            let width = track.size.width * level.clamp(0.0, 1.0);
            if width > 0.0 {
                let fill = if content.muted {
                    secondary.with_alpha(MUTED_TRACK_OPACITY * alpha)
                } else {
                    ThemeColor::Primary.resolve(theme).with_alpha(alpha)
                };
                scene.fill_rect(
                    fill,
                    Rect::new(track.origin.x, track.origin.y, width, TRACK_H),
                );
            }
        }
    }

    fn button_rects(rect: Rect) -> [Rect; 3] {
        let y = rect.origin.y + PAD_TOP + (ART - BUTTON) / 2.0;
        let next = rect.right() - PAD - BUTTON;
        let play_pause = next - BUTTON - GRID.s4;
        let previous = play_pause - BUTTON - GRID.s4;
        [
            Rect::new(previous, y, BUTTON, BUTTON),
            Rect::new(play_pause, y, BUTTON, BUTTON),
            Rect::new(next, y, BUTTON, BUTTON),
        ]
    }

    fn paint_media(
        &self,
        theme: &MetroTheme,
        card: Rect,
        media: &OsdOverlayMedia<'_>,
        scene: &mut Scene,
        alpha: f32,
    ) {
        let cover = Rect::new(card.origin.x + PAD, card.origin.y + PAD_TOP, ART, ART);
        if let Some(art) = &self.icons.art {
            scene.image_with_opacity(art, cover, None, alpha);
        } else {
            scene.fill_rect(ThemeColor::Primary.resolve(theme).with_alpha(alpha), cover);
        }
        let buttons = Self::button_rects(card);
        let x = cover.right() + MEDIA_TEXT_GAP;
        let width = (buttons[0].origin.x - MEDIA_TEXT_GAP - x).max(0.0);
        let secondary = ThemeColor::OnSurfaceVariant.resolve(theme);
        if width > 0.0 {
            let mid = cover.center().y;
            let title = theme.typography.body;
            scene.label(
                media.title.into(),
                Rect::new(x, mid - title.line_height, width, title.line_height),
                ThemeColor::OnSurface.resolve(theme).with_alpha(alpha),
                title,
                TextAlign::Left,
            );
            let artist = theme.typography.caption;
            scene.label(
                media.artist.into(),
                Rect::new(x, mid, width, artist.line_height),
                secondary.with_alpha(alpha),
                artist,
                TextAlign::Left,
            );
        }
        scene.fill_rect(
            ThemeColor::Divider.resolve(theme).with_alpha(alpha),
            Rect::new(
                card.origin.x + PAD,
                card.origin.y + MEDIA_ROW_H - 1.0,
                card.size.width - 2.0 * PAD,
                1.0,
            ),
        );
        let middle = if media.playing {
            &self.icons.pause
        } else {
            &self.icons.play
        };
        let icons = [
            (&self.icons.previous, media.can_previous),
            (middle, media.can_play_pause),
            (&self.icons.next, media.can_next),
        ];
        for (button, (icon, enabled)) in buttons.into_iter().zip(icons) {
            let Some(icon) = icon else { continue };
            let side = sizes::ICON_20;
            let target = Rect::new(
                button.origin.x + (button.size.width - side) / 2.0,
                button.origin.y + (button.size.height - side) / 2.0,
                side,
                side,
            );
            let opacity = if enabled {
                alpha
            } else {
                alpha * theme.indication.inactive_opacity
            };
            scene.image_with_opacity(icon, target, Some(secondary), opacity);
        }
    }

    fn button_at(rect: Rect, point: Point) -> Option<OsdMediaButton> {
        Self::button_rects(rect)
            .into_iter()
            .zip([
                OsdMediaButton::Previous,
                OsdMediaButton::PlayPause,
                OsdMediaButton::Next,
            ])
            .find_map(|(rect, button)| rect.contains(point).then_some(button))
    }

    pub fn event(&mut self, ctx: &mut EventCtx, event: &Event, has_media: bool) {
        match event {
            Event::PointerMove { .. } if !self.hovered => {
                self.hovered = true;
                ctx.emit(OsdHoverChanged(true));
            }
            Event::PointerLeave => {
                if self.hovered {
                    self.hovered = false;
                    ctx.emit(OsdHoverChanged(false));
                }
                self.pressed = None;
            }
            Event::PointerDown {
                pos,
                button: PointerButton::Left,
                ..
            } => {
                self.pressed = has_media
                    .then(|| Self::button_at(ctx.rect(), *pos))
                    .flatten();
                if self.pressed.is_some() {
                    ctx.set_handled();
                }
            }
            Event::PointerUp {
                button: PointerButton::Left,
                ..
            } => {
                if let Some(button) = self.pressed.take() {
                    ctx.emit(OsdMediaClicked(button));
                }
            }
            _ => {}
        }
    }
}
