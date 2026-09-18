// 客户端 dmabuf 输出：gbm bo mmap CPU 写 + 导出 fd → zwp_linux_dmabuf_v1 → 合成器 EGLImage。
//
// 替代 `commit_shm_buffers` 的「CpuRenderer → Vec<u8> → SHM pool → 合成器 SHM 上传 GPU」
// 全程 CPU 搬运。CpuRenderer 本就是 CPU 光栅化，只要把像素宿主换成 gbm bo 的 mmap，
// 就能既 CPU 写、又作 dmabuf fd 导出，交给合成器直接建 GPU 纹理（零上传）。
// 参 Ether-main docs/LINUX_DMABUF_PLAN.md §1/§3。
//
// 不变量与 SHM 双缓冲一致：size 变化重建双槽；局部损伤按 buffer-age 回补；在飞行槽
// 收到 wl_buffer.release 前不复用（否则合成器 may be 仍引用 → EBUSY / 撕裂）。
//
// 2026-09-18 扩展（「直通给所有部件用」）：主表面之外的**浮层表面 + IME 候选窗**同样走
// dmabuf；默认开启（`ETHER_DMABUF=0` 显式回退 SHM），并在开启前做**崩溃安全 gbm 探测**
// （见 `probe_gbm_crash_safe`），且按合成器 `zwp_linux_dmabuf_feedback_v1` 的主设备/格式表
// 校验（多 GPU 安全 + 免 R/B 交换的快路径）。

use std::fs::File;
use std::os::fd::AsFd;

use gbm::{BufferObject, BufferObjectFlags, Device, Format};
use kanesumi_core::Rect;
use smithay_client_toolkit::dmabuf::DmabufState;
use smithay_client_toolkit::reexports::protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1;
use wayland_client::protocol::{wl_buffer, wl_surface};
use wayland_client::{Proxy, QueueHandle};

use crate::platform::Shell;

/// `DRM_FORMAT_MOD_LINEAR`（gbm LINEAR bo 的实际修饰符）。
const MOD_LINEAR: u64 = 0;
/// `DRM_FORMAT_MOD_INVALID`（协议里表示「隐式修饰符」）；smithay 对每个 fourcc 都广告它。
const MOD_INVALID: u64 = 0x00ff_ffff_ffff_ffff;

/// 一次提交的结果 —— 调用方据此决定是否回退 SHM（**不可丢帧**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CommitOutcome {
    /// 已 attach + damage + commit。
    Committed,
    /// 本帧未提交但路径仍健康（双槽都在飞）—— 与 SHM 路径同语义，**不要**回退。
    Skipped,
    /// dmabuf 路径不可用（gbm 打不开 / 未广告格式 / 建槽失败）→ 调用方回退 SHM。
    Unavailable,
}

/// 选定的输出格式：fourcc + 是否需要在写入时交换 R/B。
/// ABGR8888 的内存字节序正是 CpuRenderer 的 RGBA 输出 → **免交换**（省一遍逐像素搬运）。
#[derive(Debug, Clone, Copy)]
struct FormatChoice {
    fourcc: Format,
    swap_rb: bool,
}

/// 渲染节点路径候选（gbm Device 打开用）。优先 /dev/dri/renderD*（渲染 + 合成共用）；
/// 回退任意 card*（主 GPU 显示节点）。
/// 若给了合成器 feedback 的主设备 dev_t，则**精确匹配该设备**优先（多 GPU 安全）。
fn open_node_for(main_device: Option<libc::dev_t>) -> Option<(File, String)> {
    use std::os::unix::fs::MetadataExt;
    let mut nodes: Vec<(String, libc::dev_t)> = Vec::new();
    for dir in ["/dev/dri", "/dev/drm"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().to_string();
                if name.starts_with("renderD") || name.starts_with("card") {
                    let rdev = e.metadata().map(|m| m.rdev() as libc::dev_t).unwrap_or(0);
                    nodes.push((format!("{dir}/{name}"), rdev));
                }
            }
        }
    }
    // 优先级：① 与主设备 dev_t 精确匹配；② renderD*；③ card*（同类内按名字稳定排序）。
    nodes.sort_by_key(|(name, _)| {
        let render = name.contains("renderD");
        (if render { 0 } else { 1 }, name.clone())
    });
    let mut ordered: Vec<&(String, libc::dev_t)> = Vec::new();
    if let Some(dev) = main_device {
        ordered.extend(nodes.iter().filter(|(_, rdev)| dev != 0 && *rdev == dev));
    }
    ordered.extend(nodes.iter());
    for (path, _) in ordered {
        if let Ok(f) = File::open(path) {
            return Some((f, path.clone()));
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

/// 客户端 dmabuf 双缓冲池（CPU 光栅化角色用：主表面 / 浮层 / IME 候选窗各一份）。
pub struct DmabufBuffers {
    dev: Option<Device<File>>,
    width: u32,
    height: u32,
    slots: [Option<Slot>; 2],
    next: usize,
    /// 合成器 feedback 给出的主设备（dev_t）+ 广告格式表 (fourcc, modifier)。
    /// 空 = 未收到 feedback（v3 合成器）→ 退回旧行为（ARGB8888 + 隐式修饰符）。
    main_device: Option<libc::dev_t>,
    formats: Vec<(u32, u64)>,
    /// 本进程 dmabuf 路径是否已判定不可用（探测失败 / 建槽失败 / 未广告格式）。
    unavailable: bool,
    /// 已选定的输出格式（首次提交时依广告表决议）。
    chosen: Option<FormatChoice>,
    /// 诊断：是否已记录「选定节点的路径」。
    logged_node: bool,
}

impl Default for DmabufBuffers {
    fn default() -> Self {
        // ⚠ 绝不能在此打开 gbm device！Shell::new 对**所有**客户端都会构造 DmabufBuffers，
        // 而 gbm::Device::new 在部分 Mesa 驱动上（如 Debian 25.0.x libgallium）会直接段错误
        // （libgallium→libc memcpy 空指针，崩溃栈见 LINUX_DMABUF_PLAN 排障）：这会让每个
        // harness 客户端启动即崩 → 桌面层永不连接 → 纯黑启动遮罩。改为惰性：仅当真正走
        // dmabuf 提交路径时才 init_device()。
        DmabufBuffers {
            dev: None,
            width: 0,
            height: 0,
            slots: [None, None],
            next: 0,
            main_device: None,
            formats: Vec::new(),
            unavailable: false,
            chosen: None,
            logged_node: false,
        }
    }
}

impl DmabufBuffers {
    /// 合成器 `zwp_linux_dmabuf_feedback_v1` → 主设备 + 格式表（多 GPU 选设备 + 格式校验）。
    pub(crate) fn set_feedback(&mut self, main_device: libc::dev_t, formats: Vec<(u32, u64)>) {
        self.main_device = Some(main_device);
        self.formats = formats;
    }

    /// 标记本缓冲池不再走 dmabuf（合成器拒绝过该组合 / 探测失败）→ 调用方回退 SHM。
    pub(crate) fn mark_unavailable(&mut self) {
        self.unavailable = true;
    }

    pub(crate) fn is_unavailable(&self) -> bool {
        self.unavailable
    }

    /// 惰性初始化 gbm device（仅在 dmabuf 提交路径调用；SHM 路径不触发）。
    /// 设备选择优先合成器 feedback 的主设备（多 GPU 安全），其次 renderD*，最后 card*。
    fn init_device(&mut self) -> bool {
        if self.dev.is_some() {
            return true;
        }
        match open_node_for(self.main_device) {
            Some((f, path)) => match Device::new(f) {
                Ok(d) => {
                    if !self.logged_node {
                        self.logged_node = true;
                        log::info!(
                            "dmabuf gbm device 就绪：{path}（main_device={:?}）",
                            self.main_device.map(|d| format!("0x{d:x}"))
                        );
                    }
                    self.dev = Some(d);
                    true
                }
                Err(e) => {
                    log::warn!("dmabuf gbm device 打开失败（{path}）：{e}");
                    false
                }
            },
            None => {
                log::warn!("dmabuf 不可用：没有可打开的 DRM 节点");
                false
            }
        }
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

    /// 依合成器广告表选定输出格式：**ABGR8888（免 R/B 交换）优先**，其次 ARGB8888（需交换）。
    /// 未收到 feedback（v3 合成器 / 无表）→ 保守用 ARGB8888 + 交换（旧行为，已验证可用）。
    /// 两者都未被广告 → None（调用方回退 SHM，避免合成器导入失败导致表面不可见）。
    fn choose_format(&mut self) -> Option<FormatChoice> {
        if let Some(c) = self.chosen {
            return Some(c);
        }
        let abgr = Format::Abgr8888 as u32;
        let argb = Format::Argb8888 as u32;
        let advertised = |f: u32| {
            self.formats
                .iter()
                .any(|(ff, m)| *ff == f && (*m == MOD_LINEAR || *m == MOD_INVALID))
        };
        let choice = if self.formats.is_empty() {
            // 无 feedback：沿用历史行为（隐式/LINEAR 修饰符 + ARGB8888）。
            Some(FormatChoice {
                fourcc: Format::Argb8888,
                swap_rb: true,
            })
        } else if advertised(abgr) {
            Some(FormatChoice {
                fourcc: Format::Abgr8888,
                swap_rb: false,
            })
        } else if advertised(argb) {
            Some(FormatChoice {
                fourcc: Format::Argb8888,
                swap_rb: true,
            })
        } else {
            log::warn!(
                "dmabuf 回退 SHM：合成器未广告 ARGB8888/ABGR8888（+LINEAR/隐式）—— 广告表 {} 项",
                self.formats.len()
            );
            None
        };
        if let Some(c) = choice {
            log::info!(
                "dmabuf 输出格式：{:?}（R/B 交换={}）",
                c.fourcc,
                c.swap_rb
            );
            self.chosen = Some(c);
        }
        choice
    }

    /// 尺寸变化或未建 → 重建双槽（新建 bo + create_immed 得 wl_buffer）。
    fn ensure_slots(&mut self, width: u32, height: u32, fourcc: Format) -> Option<()> {
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
                .create_buffer_object::<()>(
                    width,
                    height,
                    fourcc,
                    BufferObjectFlags::LINEAR,
                )
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
        fourcc: Format,
    ) -> Option<wl_buffer::WlBuffer> {
        let slot = self.slots[idx].as_mut()?;
        if let Some(b) = slot.buffer.take() {
            return Some(b);
        }
        let params = dmabuf.create_params(qh).ok()?;
        // 修饰符取 LINEAR(0)：gbm LINEAR bo 的实际修饰符。协议里「隐式」是
        // DRM_FORMAT_MOD_INVALID(0x00ffffffffffffff)，但 smithay 对每个 fourcc 同时广告
        // Invalid 与 LINEAR，传 LINEAR 更精确；Mesa 按其实际修饰符解析。
        // 参 LINUX_DMABUF_PLAN §5 核实结论。
        let fd = slot.bo.fd().ok()?;
        params.add(fd.as_fd(), 0, 0, slot.bo.stride(), MOD_LINEAR);
        let (buffer, _params) = params.create_immed(
            self.width as i32,
            self.height as i32,
            fourcc as u32,
            zwp_linux_buffer_params_v1::Flags::empty(),
            qh,
        );
        Some(buffer)
    }

    /// 主表面 dmabuf 提交（等价 `commit_shm_buffers`，缓冲宿主为 gbm bo）。
    /// `rgba` 为 CpuRenderer 输出的 RGBA 直通像素；按选定格式决定是否换成 BGRA 后写 bo mmap。
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
    ) -> CommitOutcome {
        if self.unavailable {
            return CommitOutcome::Unavailable;
        }
        let expected = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if rgba.len() < expected || width == 0 || height == 0 {
            return CommitOutcome::Skipped;
        }
        let Some(choice) = self.choose_format() else {
            self.unavailable = true;
            return CommitOutcome::Unavailable;
        };
        // 惰性 gbm device：仅在真正走 dmabuf 提交时初始化（SHM 路径不触发）。
        if !self.init_device() {
            self.unavailable = true;
            return CommitOutcome::Unavailable;
        }
        if self.ensure_slots(width, height, choice.fourcc).is_none() {
            self.unavailable = true;
            return CommitOutcome::Unavailable;
        }
        // 找一个可写槽位（优先 next，其次另一；双缓冲都在飞则跳过本帧）。
        let idx = if !self.slots[self.next].as_ref().unwrap().in_flight {
            self.next
        } else if !self.slots[1 - self.next].as_ref().unwrap().in_flight {
            1 - self.next
        } else {
            return CommitOutcome::Skipped;
        };
        let fresh = false;
        let write_region = compute_write_region(
            fresh,
            self.slots[idx].as_ref().unwrap().needs_full,
            damage,
            self.slots[idx].as_ref().unwrap().partial,
        );
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
        // 局部拷贝：只写写入区行，其余像素保留 slot 上帧内容（bo mmap 是持久映射，与 SHM
        // pool 同语义；stride 可能与 width*4 不同，按 stride 跳行）。
        // ABGR8888（swap_rb=false）时逐字节直通 —— 省掉旧实现的整区 R/B 交换。
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
                    if choice.swap_rb {
                        for k in 0..cw {
                            let s = src_row + k * 4;
                            let d = dst_row + k * 4;
                            buf[d] = rgba[s + 2];
                            buf[d + 1] = rgba[s + 1];
                            buf[d + 2] = rgba[s];
                            buf[d + 3] = rgba[s + 3];
                        }
                    } else {
                        buf[dst_row..dst_row + cw * 4]
                            .copy_from_slice(&rgba[src_row..src_row + cw * 4]);
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
        if let Some(buffer) = self.ensure_buffer(qh, idx, dmabuf, choice.fourcc) {
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
            CommitOutcome::Committed
        } else {
            self.unavailable = true;
            CommitOutcome::Unavailable
        }
    }
}

// ── gbm 崩溃安全探测（dmabuf 默认开启的前置门）───────────────────────────────

/// 子进程入口用：真开一次 gbm device（**会段错误的驱动上直接死掉**，这正是要检测的）。
/// 先关 core dump —— 探测失败很可能以 SIGSEGV 结束，绝不能往 /var/crash 灌 core。
pub fn probe_gbm_raw() -> bool {
    unsafe {
        let lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &lim);
    }
    match open_node_for(None) {
        Some((f, _)) => Device::new(f).is_ok(),
        None => false,
    }
}

/// 探测结果缓存路径（会话级 tmpfs：每次登录重新探测，跨会话不残留陈旧结论）。
fn probe_cache_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("ether-dmabuf-probe"))
}

/// **崩溃安全**探测：gbm 打开在有问题的 Mesa（Debian 2026-08-19 黑屏根因）上会段错误，
/// 段错误无法在进程内捕获 → 在**子进程**里试一次，父进程按退出状态决定 dmabuf 是否启用。
/// 结果缓存到 `$XDG_RUNTIME_DIR/ether-dmabuf-probe`，同一会话内多个 harness 客户端只探一次。
pub fn probe_gbm_crash_safe() -> bool {
    let cache = probe_cache_path();
    if let Some(p) = cache.as_ref()
        && let Ok(s) = std::fs::read_to_string(p)
    {
        let ok = s.trim() == "ok";
        log::info!("dmabuf gbm 探测（缓存）：{}", if ok { "可用" } else { "不可用" });
        return ok;
    }
    let ok = match std::env::current_exe() {
        Ok(exe) => match std::process::Command::new(exe)
            .env("ETHER_DMABUF_PROBE", "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(mut child) => {
                // 最多等 2s：gbm 打开是本地操作（正常 < 100ms）；超时按不可用处理并杀掉，
                // 绝不让探测卡住客户端启动（黑屏事故教训：客户端不可用 = 整个桌面不可用）。
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
                loop {
                    match child.try_wait() {
                        Ok(Some(status)) => break status.success(),
                        Ok(None) if std::time::Instant::now() < deadline => {
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                        _ => {
                            let _ = child.kill();
                            let _ = child.wait();
                            log::warn!("dmabuf gbm 探测超时（2s）→ 按不可用处理");
                            break false;
                        }
                    }
                }
            }
            Err(_) => false,
        },
        Err(_) => false,
    };
    let note = if ok { "ok" } else { "fail" };
    if let Some(p) = cache.as_ref() {
        let _ = std::fs::write(p, note);
    }
    if ok {
        log::info!("dmabuf gbm 探测：可用（子进程）");
    } else {
        log::warn!("dmabuf gbm 探测：不可用（子进程失败/崩溃）→ 全部表面走 SHM");
    }
    ok
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
        assert_eq!(
            compute_write_region(true, false, Some(r(0.0, 0.0, 8.0, 8.0)), None),
            None
        );
    }

    #[test]
    fn write_region_backfills_partial() {
        let d = Some(r(10.0, 0.0, 40.0, 10.0));
        let p = Some(r(200.0, 5.0, 8.0, 8.0));
        let out = compute_write_region(false, false, d, p).unwrap();
        assert_eq!(out, r(10.0, 0.0, 198.0, 13.0));
    }

    /// 格式决议：ABGR8888（免 R/B 交换）优先于 ARGB8888；未广告 → None（回退 SHM）。
    #[test]
    fn format_choice_prefers_abgr_then_argb_then_none() {
        let abgr = Format::Abgr8888 as u32;
        let argb = Format::Argb8888 as u32;
        // 两者都广告 → ABGR8888 + 免交换。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(argb, MOD_LINEAR), (abgr, MOD_INVALID)]);
        let c = b.choose_format().unwrap();
        assert_eq!(c.fourcc as u32, abgr);
        assert!(!c.swap_rb);
        // 仅 ARGB8888 → 交换。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(argb, MOD_LINEAR)]);
        let c = b.choose_format().unwrap();
        assert_eq!(c.fourcc as u32, argb);
        assert!(c.swap_rb);
        // 未广告（例如只给 XRGB8888）→ None → 调用方 SHM 回退。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(Format::Xrgb8888 as u32, MOD_LINEAR)]);
        assert!(b.choose_format().is_none());
        // 无 feedback（老合成器）→ 沿用 ARGB8888 + 交换。
        let mut b = DmabufBuffers::default();
        let c = b.choose_format().unwrap();
        assert_eq!(c.fourcc as u32, argb);
        assert!(c.swap_rb);
    }

    #[test]
    fn unavailable_short_circuits_commit() {
        let mut b = DmabufBuffers::default();
        b.mark_unavailable();
        assert!(b.is_unavailable());
    }
}
