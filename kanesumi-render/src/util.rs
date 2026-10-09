// util.rs —— 几何裁剪、呈现模式选择与 GPU 计时工具。
use std::sync::Arc;
use kanesumi_core::Rect;

/// 矩形求交（box 语义：内容裁剪到盒内）。不相交返回 None。
pub fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let x0 = a.origin.x.max(b.origin.x);
    let y0 = a.origin.y.max(b.origin.y);
    let x1 = a.right().min(b.right());
    let y1 = a.bottom().min(b.bottom());
    if x1 <= x0 || y1 <= y0 {
        None
    } else {
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }
}

/// 计算视口/裁剪的 scissor 矩形（物理整数像素坐标与尺寸）。
pub fn scissor_rect(
    clip: Option<Rect>,
    scale: f32,
    buffer_width: u32,
    buffer_height: u32,
) -> (u32, u32, u32, u32) {
    let max_x = buffer_width.max(1) as f32;
    let max_y = buffer_height.max(1) as f32;
    match clip {
        Some(clip) => {
            let x = (clip.origin.x * scale).floor().clamp(0.0, max_x - 1.0) as u32;
            let y = (clip.origin.y * scale).floor().clamp(0.0, max_y - 1.0) as u32;
            let right = (clip.right() * scale).ceil().clamp(x as f32 + 1.0, max_x) as u32;
            let bottom = (clip.bottom() * scale).ceil().clamp(y as f32 + 1.0, max_y) as u32;
            (x, y, right - x, bottom - y)
        }
        None => (0, 0, buffer_width.max(1), buffer_height.max(1)),
    }
}

/// 选呈现模式：支持 `KANESUMI_PRESENT=fifo|mailbox` 显式选定；
/// 缺省优先 `Mailbox`，其次 `Immediate`，最后回落 `Fifo`。
pub fn choose_present_mode(available: &[wgpu::PresentMode]) -> wgpu::PresentMode {
    let ov = std::env::var("KANESUMI_PRESENT").ok();
    choose_present_mode_from(available, ov.as_deref())
}

pub fn choose_present_mode_from(
    available: &[wgpu::PresentMode],
    override_mode: Option<&str>,
) -> wgpu::PresentMode {
    if let Some(val) = override_mode {
        match val.to_ascii_lowercase().trim() {
            "fifo" if available.contains(&wgpu::PresentMode::Fifo) => {
                return wgpu::PresentMode::Fifo;
            }
            "mailbox" if available.contains(&wgpu::PresentMode::Mailbox) => {
                return wgpu::PresentMode::Mailbox;
            }
            _ => {}
        }
    }
    if available.contains(&wgpu::PresentMode::Mailbox) {
        wgpu::PresentMode::Mailbox
    } else if available.contains(&wgpu::PresentMode::Immediate) {
        wgpu::PresentMode::Immediate
    } else {
        wgpu::PresentMode::Fifo
    }
}

/// 合并全局损伤裁剪与步骤裁剪（G1 损伤感知）。
pub fn damage_clip(damage: Option<Rect>, clip: Option<Rect>) -> Option<Option<Rect>> {
    match (damage, clip) {
        (None, c) => Some(c),
        (Some(d), None) => Some(Some(d)),
        (Some(d), Some(c)) => intersect(c, d).map(Some),
    }
}

// ── GPU 时间戳计时器 ───────────────────────────────────────────────────────

pub const TIMESTAMP_SLOTS: u64 = 4;
pub const TIMESTAMP_LAG: u64 = 2;

fn timestamp_slot(frame: u64, slots: u64) -> usize {
    (frame % slots) as usize
}

fn readback_slot_for(frame: u64, slots: u64, lag: u64) -> Option<usize> {
    if frame < lag {
        None
    } else {
        Some(((frame - lag) % slots) as usize)
    }
}

fn ticks_to_ms(start: u64, end: u64, period_ns: f32) -> f32 {
    let diff = end.saturating_sub(start);
    (diff as f64 * period_ns as f64 / 1_000_000.0) as f32
}

const GPU_PENDING_CAP: usize = 4096;
const SLOT_STRIDE: u64 = 256;

/// GPU 时间戳计时（WGPU 路径）。
pub struct GpuTimer {
    query_set: wgpu::QuerySet,
    resolve_buf: wgpu::Buffer,
    read_bufs: Vec<Arc<wgpu::Buffer>>,
    period_ns: f32,
    frame: u64,
    slot_frame: [Option<u64>; TIMESTAMP_SLOTS as usize],
    slot_mapped: [bool; TIMESTAMP_SLOTS as usize],
    slot_keep: [bool; TIMESTAMP_SLOTS as usize],
    active: bool,
    tx: std::sync::mpsc::Sender<(usize, f32)>,
    rx: std::sync::mpsc::Receiver<(usize, f32)>,
    pending: Vec<f32>,
}

impl GpuTimer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let need =
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
        if !device.features().contains(need) {
            return None;
        }
        let slots = TIMESTAMP_SLOTS as u32;
        let query_set = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("kanesumi-gpu-timer"),
            ty: wgpu::QueryType::Timestamp,
            count: slots * 2,
        });
        let resolve_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kanesumi-gpu-timer-resolve"),
            size: SLOT_STRIDE * slots as u64,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let read_bufs = (0..slots)
            .map(|i| {
                Arc::new(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some(&format!("kanesumi-gpu-timer-read-{i}")),
                    size: 16,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }))
            })
            .collect();
        let (tx, rx) = std::sync::mpsc::channel();
        Some(Self {
            query_set,
            resolve_buf,
            read_bufs,
            period_ns: queue.get_timestamp_period(),
            frame: 0,
            slot_frame: [None; TIMESTAMP_SLOTS as usize],
            slot_mapped: [false; TIMESTAMP_SLOTS as usize],
            slot_keep: [true; TIMESTAMP_SLOTS as usize],
            active: false,
            tx,
            rx,
            pending: Vec::new(),
        })
    }

    pub fn begin(&mut self) -> Option<usize> {
        let s = timestamp_slot(self.frame, TIMESTAMP_SLOTS);
        if self.slot_frame[s].is_some() {
            self.active = false;
            return None;
        }
        self.slot_frame[s] = Some(self.frame);
        self.active = true;
        Some(s)
    }

    pub fn write_first(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        encoder.write_timestamp(&self.query_set, slot as u32 * 2);
    }

    pub fn write_last(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        encoder.write_timestamp(&self.query_set, slot as u32 * 2 + 1);
    }

    pub fn encode_readback(&self, encoder: &mut wgpu::CommandEncoder, slot: usize) {
        let base = slot as u64 * 2;
        encoder.resolve_query_set(
            &self.query_set,
            base as u32..(base + 2) as u32,
            &self.resolve_buf,
            slot as u64 * SLOT_STRIDE,
        );
        encoder.copy_buffer_to_buffer(
            &self.resolve_buf,
            slot as u64 * SLOT_STRIDE,
            &self.read_bufs[slot],
            0,
            16,
        );
    }

    pub fn end(&mut self, device: &wgpu::Device, keep: bool) {
        if self.active {
            let s = timestamp_slot(self.frame, TIMESTAMP_SLOTS);
            self.slot_keep[s] = keep;
        }
        self.frame += 1;
        if let Some(s) = readback_slot_for(self.frame, TIMESTAMP_SLOTS, TIMESTAMP_LAG)
            && let Some(written) = self.slot_frame[s]
            && !self.slot_mapped[s]
            && self.frame >= written + TIMESTAMP_LAG
        {
            self.slot_mapped[s] = true;
            let buf = self.read_bufs[s].clone();
            let reader = buf.clone();
            let period = self.period_ns;
            let tx = self.tx.clone();
            buf.slice(0..16).map_async(wgpu::MapMode::Read, move |res| {
                if res.is_ok() {
                    let data = reader.slice(0..16).get_mapped_range();
                    let start = u64::from_le_bytes(data[0..8].try_into().unwrap_or([0; 8]));
                    let end = u64::from_le_bytes(data[8..16].try_into().unwrap_or([0; 8]));
                    drop(data);
                    let _ = tx.send((s, ticks_to_ms(start, end, period)));
                } else {
                    let _ = tx.send((s, f32::NAN));
                }
            });
        }
        let _ = device.poll(wgpu::Maintain::Poll);
        while let Ok((s, ms)) = self.rx.try_recv() {
            let keep = std::mem::replace(&mut self.slot_keep[s], true);
            self.slot_frame[s] = None;
            self.slot_mapped[s] = false;
            self.read_bufs[s].unmap();
            if keep && ms.is_finite() && self.pending.len() < GPU_PENDING_CAP {
                self.pending.push(ms);
            }
        }
    }

    pub fn drain(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.pending)
    }
}
