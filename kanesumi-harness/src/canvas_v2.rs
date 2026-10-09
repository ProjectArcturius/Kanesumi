// canvas_v2.rs —— kanesumi-harness 侧的 CanvasV2 适配层（桥接 Wayland 窗口表面）。

use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::Connection;

pub use kanesumi_render::canvas_v2::*;
pub use kanesumi_render::CanvasV2 as CoreCanvasV2;

use crate::render::{create_wl_surface, GpuContext, RendererError};

/// 适配 Wayland 窗口系统的 CanvasV2 包装结构体。
pub struct CanvasV2(pub kanesumi_render::CanvasV2);

impl Deref for CanvasV2 {
    type Target = kanesumi_render::CanvasV2;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for CanvasV2 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl CanvasV2 {
    /// 用进程共享的 [`GpuContext`] 为单个 wl_surface 建表面 / 管线 / 图集。
    pub fn with_context(
        ctx: Arc<GpuContext>,
        conn: &Connection,
        wl_surface: &WlSurface,
        width: f32,
        height: f32,
        scale: f32,
        transparent: bool,
    ) -> Result<Self, RendererError> {
        let surface = create_wl_surface(&ctx.instance, conn, wl_surface)
            .map_err(RendererError::Surface)?;
        kanesumi_render::CanvasV2::with_surface(ctx, surface, width, height, scale, transparent)
            .map(Self)
    }

    pub fn into_inner(self) -> kanesumi_render::CanvasV2 {
        self.0
    }
}
