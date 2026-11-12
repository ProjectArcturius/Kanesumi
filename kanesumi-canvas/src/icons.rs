// 控件底纹图形 —— 「名字 → 图」的唯一入口，跨扇区同名同形。
//
// 为什么要有这一层：此前五个控件各自为政 —— chevron 在 `glyph.rs` 里自绘三角，
// 汉堡键是三根 rect 直接写在 `navigation_view.rs`，复选框/评星/严重度靠字体字形
// （`✓ ✕ ★ ☆ i !`）。字体那条在思源黑体下出方框（参 docs/VISUAL_ISSUES.md V7），
// 换个扇区（Kotlin 侧）更是连字形都找不到。
//
// 现在：**名字**在这里，**形状**在 `Kanesumi-assets` 仓的 SVG 里，Rust 与 Kotlin 两扇区共用
// 同一份真源（参 assets 仓 docs/GLYPH_ASSETS.md）。语义图标（文件 / 设备 / 应用）不在这里 ——
// 那归宿主主题，由调用方解好后经 `Icon` 注入。
//
// 两条**内部**路径：
// - `chevron-*` 与 `check` 仍走 `glyph.rs` 的三角几何：零纹理、零上传、损伤友好；
// - 其余走 `rasterize_svg_bytes` → `Scene::image`，按 (名字, 像素) 缓存。
// 调用方只看 [`GlyphName`] 与 [`draw`]，不关心走哪条路。

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use kanesumi_core::{Color, Rect};

use crate::icon::{Icon, rasterize_svg_bytes};
use crate::scene::Scene;

/// 控件底纹图形的名字。与 `Kanesumi-assets` 仓的 `GLYPHS.txt` 逐项对应
/// （`kanesumi-canvas/tests/glyph_assets.rs` 双向校验，漏一边就红）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum GlyphName {
    /// 下向三角：ComboBox / DropdownMenu / Selector 的展开指示。
    ChevronDown,
    /// 上向三角：Expander 展开态、PipsPager。
    ChevronUp,
    /// 左向三角：分页上一页、日历上月、标题栏返回。
    ChevronLeft,
    /// 右向三角：导航子项、面包屑、树节点。
    ChevronRight,
    /// 勾选标记。
    Check,
    /// 减去标记（不确定态）。
    Minus,
    /// 关闭叉。
    Close,
    /// 三条线（导航面板开合）。
    Hamburger,
    /// 横向三点（溢出）。
    MoreHorizontal,
    /// 纵向三点（行级命令）。
    MoreVertical,
    /// 放大镜。
    Search,
    /// 加号。
    Add,
    /// 星形（评分；空星用 alpha、半星用裁剪复用同一枚）。
    Star,
    /// 信息圆。
    Info,
    /// 警告三角。
    Warning,
    /// 圆形叉（错误态，与裸叉 `Close` 区分语义）。
    Error,
}

impl GlyphName {
    /// 全部名字，顺序与 `GLYPHS.txt` 一致。
    pub const ALL: [GlyphName; 16] = [
        GlyphName::ChevronDown,
        GlyphName::ChevronUp,
        GlyphName::ChevronLeft,
        GlyphName::ChevronRight,
        GlyphName::Check,
        GlyphName::Minus,
        GlyphName::Close,
        GlyphName::Hamburger,
        GlyphName::MoreHorizontal,
        GlyphName::MoreVertical,
        GlyphName::Search,
        GlyphName::Add,
        GlyphName::Star,
        GlyphName::Info,
        GlyphName::Warning,
        GlyphName::Error,
    ];

    /// 文件名（不含 `.svg`），即资产仓里的真源名。
    pub fn file(self) -> &'static str {
        match self {
            GlyphName::ChevronDown => "chevron-down",
            GlyphName::ChevronUp => "chevron-up",
            GlyphName::ChevronLeft => "chevron-left",
            GlyphName::ChevronRight => "chevron-right",
            GlyphName::Check => "check",
            GlyphName::Minus => "minus",
            GlyphName::Close => "close",
            GlyphName::Hamburger => "hamburger",
            GlyphName::MoreHorizontal => "more-horizontal",
            GlyphName::MoreVertical => "more-vertical",
            GlyphName::Search => "search",
            GlyphName::Add => "add",
            GlyphName::Star => "star",
            GlyphName::Info => "info",
            GlyphName::Warning => "warning",
            GlyphName::Error => "error",
        }
    }

    /// 内嵌 SVG 源。`include_str!` 在编译期把形状带进二进制，运行期不读磁盘。
    pub fn svg(self) -> &'static str {
        match self {
            GlyphName::ChevronDown => {
                include_str!("../../assets/kanesumi-assets/glyphs/chevron-down.svg")
            }
            GlyphName::ChevronUp => {
                include_str!("../../assets/kanesumi-assets/glyphs/chevron-up.svg")
            }
            GlyphName::ChevronLeft => {
                include_str!("../../assets/kanesumi-assets/glyphs/chevron-left.svg")
            }
            GlyphName::ChevronRight => {
                include_str!("../../assets/kanesumi-assets/glyphs/chevron-right.svg")
            }
            GlyphName::Check => include_str!("../../assets/kanesumi-assets/glyphs/check.svg"),
            GlyphName::Minus => include_str!("../../assets/kanesumi-assets/glyphs/minus.svg"),
            GlyphName::Close => include_str!("../../assets/kanesumi-assets/glyphs/close.svg"),
            GlyphName::Hamburger => include_str!("../../assets/kanesumi-assets/glyphs/hamburger.svg"),
            GlyphName::MoreHorizontal => {
                include_str!("../../assets/kanesumi-assets/glyphs/more-horizontal.svg")
            }
            GlyphName::MoreVertical => {
                include_str!("../../assets/kanesumi-assets/glyphs/more-vertical.svg")
            }
            GlyphName::Search => include_str!("../../assets/kanesumi-assets/glyphs/search.svg"),
            GlyphName::Add => include_str!("../../assets/kanesumi-assets/glyphs/add.svg"),
            GlyphName::Star => include_str!("../../assets/kanesumi-assets/glyphs/star.svg"),
            GlyphName::Info => include_str!("../../assets/kanesumi-assets/glyphs/info.svg"),
            GlyphName::Warning => include_str!("../../assets/kanesumi-assets/glyphs/warning.svg"),
            GlyphName::Error => include_str!("../../assets/kanesumi-assets/glyphs/error.svg"),
        }
    }

    /// 走几何快路径的那几枚（`glyph.rs` 的三角），不需要纹理。
    ///
    /// 公开出来是给调用方/测试判断「这枚会不会占一份纹理」——绘制行为不受影响。
    pub fn is_geometric(self) -> bool {
        matches!(
            self,
            GlyphName::ChevronDown
                | GlyphName::ChevronUp
                | GlyphName::ChevronLeft
                | GlyphName::ChevronRight
                | GlyphName::Check
        )
    }
}

/// 光栅缓存：键 = (名字, 物理像素)。1× 与 2× 各存一份，绝不放大复用。
fn cache() -> &'static Mutex<HashMap<(GlyphName, u32), Icon>> {
    static CACHE: OnceLock<Mutex<HashMap<(GlyphName, u32), Icon>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 缓存里已有多少份光栅（诊断与测试用；不参与绘制语义）。
pub fn cached_glyphs() -> usize {
    cache().lock().map(|map| map.len()).unwrap_or(0)
}

/// 某个名字已缓存的光栅像素尺寸（升序）。诊断与测试用。
///
/// 按名字分开取，测试之间互不干扰：某个测试只关心自己那枚的键有没有新增。
pub fn cached_pixels(name: GlyphName) -> Vec<u32> {
    let mut pixels: Vec<u32> = cache()
        .lock()
        .map(|map| {
            map.keys()
                .filter(|(cached, _)| *cached == name)
                .map(|(_, px)| *px)
                .collect()
        })
        .unwrap_or_default();
    pixels.sort_unstable();
    pixels
}

/// 清空光栅缓存（测试用；正常路径不需要调）。
pub fn clear_cache() {
    if let Ok(mut map) = cache().lock() {
        map.clear();
    }
}

/// 目标像素：正方母版取 `rect` 短边 × 输出倍率，至少 1。
fn pixels(rect: Rect, scale: f32) -> u32 {
    let side = rect.size.width.min(rect.size.height) * scale.abs();
    (side.round() as i64).clamp(1, 4096) as u32
}

/// 在 `rect` 内画出 `name`，按 `color` 染色。
///
/// **1× 用途**。2× / 3× 输出必须用 [`draw_scaled`]，否则位图被放大、边缘发虚。
pub fn draw(scene: &mut Scene, name: GlyphName, rect: Rect, color: Color) {
    draw_scaled(scene, name, rect, color, 1.0);
}

/// 同 [`draw`]，但按 `scale`（逻辑点 → 物理像素倍率）光栅。
pub fn draw_scaled(scene: &mut Scene, name: GlyphName, rect: Rect, color: Color, scale: f32) {
    if rect.size.width <= 0.0 || rect.size.height <= 0.0 {
        return;
    }
    if name.is_geometric() {
        draw_geometric(scene, name, rect, color);
        return;
    }
    let px = pixels(rect, scale);
    let icon = cached(name, px);
    if let Some(icon) = icon {
        scene.image(&icon, rect, Some(color));
    }
}

/// 取缓存或光栅一次。光栅失败（资产损坏）返回 None —— 静默跳过，不 panic。
fn cached(name: GlyphName, px: u32) -> Option<Icon> {
    if let Ok(map) = cache().lock()
        && let Some(icon) = map.get(&(name, px))
    {
        return Some(icon.clone());
    }
    let icon = rasterize_svg_bytes(name.svg().as_bytes(), px)?;
    if let Ok(mut map) = cache().lock() {
        map.insert((name, px), icon.clone());
    }
    Some(icon)
}

/// 几何快路径：三角与折线，直接产出 Scene 命令。
fn draw_geometric(scene: &mut Scene, name: GlyphName, rect: Rect, color: Color) {
    match name {
        GlyphName::ChevronDown => crate::glyph::chevron_down(scene, rect, color),
        GlyphName::ChevronUp => crate::glyph::chevron_up(scene, rect, color),
        GlyphName::ChevronLeft => crate::glyph::chevron_left(scene, rect, color),
        GlyphName::ChevronRight => crate::glyph::chevron_right(scene, rect, color),
        GlyphName::Check => crate::glyph::check(scene, rect, color),
        // `is_geometric` 已把范围锁死，这里不可达。
        _ => {}
    }
}
