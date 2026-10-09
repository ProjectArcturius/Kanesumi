//! Kanesumi（矩隅）· 平台无关渲染基座
//!
//! 包含 GPU 实例化批渲染画布（CanvasV2）、字形排版（glyph_layout）、
//! CPU 光栅器（CpuRenderer）以及平台无关的 GPU 上下文与几何裁剪工具。

pub mod glyph_layout;
pub mod cpu_raster;
pub mod canvas_v2;
pub mod context;
pub mod util;

pub use glyph_layout::{
    GlyphKey, PlacedGlyph, TextRenderTuning, layout_text_glyphs, layout_text_glyphs_tuned,
};
pub use cpu_raster::CpuRenderer;
pub use canvas_v2::CanvasV2;
pub use canvas_v2 as canvas;
pub use context::{GpuContext, RendererError};
pub use util::{
    choose_present_mode, choose_present_mode_from, damage_clip, intersect, scissor_rect, GpuTimer,
};
