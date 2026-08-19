// 客户端 dmabuf 输出：gbm bo mmap CPU 写 + 导出 fd → zwp_linux_dmabuf_v1 → 合成器 EGLImage。
//
// 替代 `commit_shm_buffers` 的「CpuRenderer → Vec<u8> → SHM pool → 合成器 SHM 上传 GPU」
// 全程 CPU 搬运。CpuRenderer 本就是 CPU 光栅化，只要把像素宿主换成 gbm bo 的 mmap，
// 就能既 CPU 写、又作 dmabuf fd 导出，交给合成器直接建 GPU 纹理（零上传）。
// 参 Ether-main docs/LINUX_DMABUF_PLAN.md §1/§3。
//
// 不变量与 SHM 双缓冲一致：size 变化重建双槽；局部损伤按 buffer-age 回补；在飞行槽
// 收到 wl_buffer.release 前不复用（否则合成器 may be 仍引用 → EBUSY / 撕裂）。

use std::fs::File;
use std::os::fd::AsFd;

use gbm::{BufferObject, BufferObjectFlags, Device, Format};
use kanesumi_core::Rect;
use smithay_client_toolkit::dmabuf::DmabufState;
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1;
use wayland_client::protocol::{wl_buffer, wl_surface};
use wayland_client::{Proxy, QueueHandle};

use crate::platform::Shell;

/// 渲染节点路径候选（gbm Device 打开用）。优先 /dev/dri/renderD*（渲染 + 合成共用）；
/// 回退任意 card*（主 GPU 显示节点）。
fn open_render_node() -> Option<File> {
    for dir in ["/dev/dri", "/dev/drm"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("renderD") || name.starts_with("card") {
                    let p = format!("{dir}/{name}");
                    if let Ok(f) = File::open(&p) {
                        return Some(f);
                    }
                }
            }
        }
    }
    None
}

/// 单个 dmabuf 槽：gbm bo（CPU mmap 写）+ 由它 create_immed 得到的 wl_buffer。
struct Slot {
    bo: BufferObject<()>,
    buffer: Option<wl_buffer::WlBuffer>,
    in_flight: bool,
    needs_full: bool,
    partial: Option<Rect>,
}

/// 客户端 dmabuf 双缓冲池（layer-shell CPU 角色用）。
pub struct DmabufBuffers {
    dev: Option<Device<File>>,
    width: u32,
    height: u32,
    slots: [Option<Slot>; 2],
    next: usize,
    /// gbm init 失败 / 不可用（无 render node）→ None，调用方回退 SHM。
    ready: bool,
}

impl Default for DmabufBuffers {
    fn default() -> Self {
        // ⚠ 绝不能在此打开 gbm device！Shell::new 对**所有**客户端都会构造 DmabufBuffers，
        // 而 gbm::Device::new 在部分 Mesa 驱动上（如 Debian 25.0.x libgallium）会直接段错误
        // （libgallium→libc memcpy 空指针，崩溃栈见 LINUX_DMABUF_PLAN 排障）：这会让每个
        // harness 客户端启动即崩 → 桌面层永不连接 → 纯黑启动遮罩。改为惰性：仅当真正走
        // dmabuf 提交路径（ETHER_DMABUF=1 + 合成器提供 global）时才 init_device()。
        DmabufBuffers {
            dev: None,
            width: 0,
            height: 0,
            slots: [None, None],
            next: 0,
            ready: false,
        }
    }
}

impl DmabufBuffers {
    /// 惰性初始化 gbm device（仅在 dmabuf 提交路径调用；默认 SHM 路径不触发）。
    /// 返回 device 是否可用。opens the render node + gbm::Device::new（详见 Default 注释
    /// 的 Mesa 段错误风险——故只在真正需要 dmabuf 时调用一次）。
    fn init_device(&mut self) -> bool {
        if self.dev.is_some() {
            return true;
        }
        if let Some(f) = open_render_node() {
            if let Ok(d) = Device::new(f) {
                self.dev = Some(d);
                self.ready = true;
                return true;
            }
        }
        false
    }

    pub(crate) fn ready(&self) -> bool {
        self.ready
    }

    /// `wl_buffer.release` → 标记对应槽位可复用。命中返回 true。
    pub(crate) fn mark_released(&mut self, buffer: &wl_buffer::WlBuffer) -> bool {
        for s in self.slots.iter_mut() {
            if let Some(slot) = s {
                if slot.buffer.as_ref() == Some(buffer) {
                    slot.in_flight = false;
                    return true;
                }
            }
        }
        false
    }

    /// 尺寸变化或未建 → 重建双槽（新建 bo + create_immed 得 wl_buffer）。
    fn ensure_slots(&mut self, _qh: &QueueHandle<Shell>, width: u32, height: u32) -> Option<()> {
        let dev = self.dev.as_ref()?;
        let fresh = self.width != width || self.height != height || self.slots[0].is_none();
        if !fresh {
            return Some(());
        }
        for s in self.slots.iter_mut() {
            *s = None;
        }
        for s in self.slots.iter_mut() {
            // LINEAR：保证 CPU 可 mmap 写。GL/drm 需 LINEAR 才允许 gbm_bo_map。
            let bo = dev
                .create_buffer_object::<()>(width, height, Format::Argb8888, BufferObjectFlags::LINEAR)
                .ok()?;
            *s = Some(Slot {
                bo,
                buffer: None,
                in_flight: false,
                needs_full: true,
                partial: None,
            });
        }
        self.width = width;
        self.height = height;
        self.next = 0;
        Some(())
    }

    /// 由当前槽 bo 的 fd 创建（或复用）wl_buffer。
    /// create_immed 直接产出缓冲，不必等 params roundtrip（DH single-plane LINEAR）。
    fn ensure_buffer(
        &mut self,
        qh: &QueueHandle<Shell>,
        idx: usize,
        dmabuf: &DmabufState,
    ) -> Option<wl_buffer::WlBuffer> {
        let slot = self.slots[idx].as_mut()?;
        if let Some(b) = slot.buffer.take() {
            return Some(b);
        }
        let params = dmabuf.create_params(qh).ok()?;
        // ⚠ 隐式修饰符（DRM_FORMAT_MOD_INVALID = 0）：与合成器广告集保持一致 —— smithay EGL
        // 格式表对每个 fourcc 总广告 Modifier::Invalid，而是否广告 LINEAR 依驱动而定。
        // Mesa 以隐式导入按 fd 实际修饰符（线性 bo → LINEAR）解析，通用且无协议/导入风险。
        // 若改传 bo.modifier()（LINEAR）可能在未广告 LINEAR 的驱动上导入失败。参 smithay
        // backend/egl/display.rs get_dmabuf_formats（每 fourcc 附 Modifier::Invalid）。
        let modifier = 0u64;
        let fd = slot.bo.fd().ok()?;
        params.add(fd.as_fd(), 0, 0, slot.bo.stride(), modifier);
        let (buffer, _params) = params.create_immed(
            self.width as i32,
            self.height as i32,
            Format::Argb8888 as u32,
            zwp_linux_buffer_params_v1::Flags::empty(),
            qh,
        );
        Some(buffer)
    }

    /// 主表面 dmabuf 提交（等价 `commit_shm_buffers`，缓冲宿主为 gbm bo）。
    /// `rgba` 为 CpuRenderer 输出的 RGBA 直通像素；R/B 交换成 BGRA 后写入 bo mmap。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn commit(
        &mut self,
        qh: &QueueHandle<Shell>,
        surface: &wl_surface::WlSurface,
        dmabuf: &DmabufState,
        width: u32,
        height: u32,
        rgba: &[u8],
        scale: f32,
        damage: Option<Rect>,
    ) {
        let expected = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if rgba.len() < expected || width == 0 || height == 0 {
            return;
        }
        // 惰性 gbm device：仅在真正走 dmabuf 提交时初始化（默认 SHM 路径不触发）。
        if !self.init_device() {
            return;
        }
        if self.ensure_slots(qh, width, height).is_none() {
            return;
        }
        // 找一个可写槽位（优先 next，其次另一；双缓冲都在飞则跳过本帧）。
        let idx = if !self.slots[self.next].as_ref().unwrap().in_flight {
            self.next
        } else if !self.slots[1 - self.next].as_ref().unwrap().in_flight {
            1 - self.next
        } else {
            return;
        };
        let fresh = false;
        let write_region =
            compute_write_region(fresh, self.slots[idx].as_ref().unwrap().needs_full, damage, self.slots[idx].as_ref().unwrap().partial);
        // 物理拷贝区（与 damage_buffer 一致）。
        let (cx0, cy0, cw, ch) = match write_region {
            Some(d) => {
                let x0 = (d.origin.x * scale).floor().clamp(0.0, width as f32) as u32;
                let y0 = (d.origin.y * scale).floor().clamp(0.0, height as f32) as u32;
                let x1 = (d.right() * scale).ceil().clamp(0.0, width as f32) as u32;
                let y1 = (d.bottom() * scale).ceil().clamp(0.0, height as f32) as u32;
                (x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
            }
            None => (0, 0, width, height),
        };
        let (cw, ch) = (cw as usize, ch as usize);
        // 局部拷贝 + R/B 交换：只写写入区行，其余像素保留 slot 上帧内容（bo mmap 是
        // 持久映射，与 SHM pool 同语义；stride 可能与 width*4 不同，按 stride 跳行）。
        {
            let slot = self.slots[idx].as_mut().unwrap();
            let _ = slot.bo.map_mut(0, 0, width, height, |mem| {
                let stride = mem.stride() as usize;
                let buf = mem.buffer_mut();
                for py in cy0..cy0 + (ch as u32) {
                    let src_row = (py * width + cx0) as usize * 4;
                    let dst_row = (py as usize) * stride + (cx0 as usize) * 4;
                    if src_row + cw * 4 > rgba.len() || dst_row + cw * 4 > buf.len() {
                        break;
                    }
                    for k in 0..cw {
                        let s = src_row + k * 4;
                        let d = dst_row + k * 4;
                        buf[d + 0] = rgba[s + 2];
                        buf[d + 1] = rgba[s + 1];
                        buf[d + 2] = rgba[s + 0];
                        buf[d + 3] = rgba[s + 3];
                    }
                }
            });
        }
        // 槽位状态更新（buffer-age 回补登记，同 SHM 逻辑）。
        self.slots[idx].as_mut().unwrap().needs_full = false;
        self.slots[idx].as_mut().unwrap().partial = None;
        let other = 1 - idx;
        match damage {
            Some(d) => {
                if self.slots[other].as_ref().unwrap().needs_full {
                    self.slots[other].as_mut().unwrap().partial = None;
                } else {
                    self.slots[other].as_mut().unwrap().partial =
                        Some(match self.slots[other].as_ref().unwrap().partial {
                            Some(p) => union_rect(p, d),
                            None => d,
                        });
                }
            }
            None => {
                self.slots[other].as_mut().unwrap().needs_full = true;
                self.slots[other].as_mut().unwrap().partial = None;
            }
        }
        if let Some(buffer) = self.ensure_buffer(qh, idx, dmabuf) {
            surface.attach(Some(&buffer), 0, 0);
            if surface.version() >= 4 {
                surface.damage_buffer(cx0 as i32, cy0 as i32, cw as i32, ch as i32);
            } else {
                surface.damage(0, 0, width as i32, height as i32);
            }
            surface.commit();
            self.slots[idx].as_mut().unwrap().buffer = Some(buffer);
            self.slots[idx].as_mut().unwrap().in_flight = true;
            self.next = other;
        }
    }
}

/// 两矩形并集（外接框）。S4 损坏矩形累积用。与 platform.rs `union_rect` 同款（复制避免
/// 跨模块私有可见性）；dmabuf 路径共用同一 buffer-age 回补语义。
fn union_rect(a: Rect, b: Rect) -> Rect {
    let x0 = a.origin.x.min(b.origin.x);
    let y0 = a.origin.y.min(b.origin.y);
    let x1 = a.right().max(b.right());
    let y1 = a.bottom().max(b.bottom());
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

/// 计算本槽位需写入区（物理像素）：全量帧 / 槽位内容不可用（新建）→ 全量（None）；
/// 局部帧 → 本帧 damage ∪ 自上次写该槽后的累积损伤（buffer-age 回补）。
fn compute_write_region(
    fresh_pool: bool,
    needs_full: bool,
    damage: Option<Rect>,
    partial: Option<Rect>,
) -> Option<Rect> {
    if fresh_pool || needs_full || damage.is_none() {
        None
    } else {
        let d = damage.unwrap();
        Some(match partial {
            Some(p) => union_rect(p, d),
            None => d,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn write_region_damage_fresh_full() {
        assert_eq!(compute_write_region(true, false, Some(r(0.0, 0.0, 8.0, 8.0)), None), None);
    }

    #[test]
    fn write_region_backfills_partial() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        let p = Some(r(200.0, 5.0, 8.0, 8.0));
        let out = compute_write_region(false, false, d, p).unwrap();
        assert_eq!(out, r(10.0, 0.0, 198.0, 13.0));
    }
}
