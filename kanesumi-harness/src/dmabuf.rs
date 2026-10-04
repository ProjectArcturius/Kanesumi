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

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::os::fd::AsFd;
use std::rc::Rc;

// 探测诊断（阶段面包屑 + 信号解码）用。
use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};

use gbm::{BufferObject, BufferObjectFlags, Device, Format, Modifier};
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

/// 进程级**共享** gbm device 句柄：同一客户端的所有表面（主表面 / 浮层 / 候选窗）共用
/// 一个 DRM fd 与一份 Mesa screen。
/// 为什么必须共享：直通铺到多表面后，若每表面各建一个 device，一个 ether-settings 进程会
/// 开 5+ 个 gbm device（各自一次 Mesa screen 创建——**正是 2026-08-19 Debian 段错误所在
/// 调用`driCreateNewScreen3`**），既拖慢启动又浪费 fd/driver 资源。
/// 惰性：仅在真正走 dmabuf 提交时创建（`Default` 绝不碰 gbm）。
#[derive(Clone, Default)]
pub struct DmabufDevice {
    dev: Rc<RefCell<Option<Rc<Device<File>>>>>,
    /// 打开失败过 → 不再重试（避免每个表面重复尝试）。
    failed: Rc<Cell<bool>>,
    /// 崩溃安全探测**证明可用**的组合（进程级；`None` = 全部表面一律 SHM）。
    /// 存在共享 device 上的理由：探测是进程级结论，后建的浮层 / popup 表面应自动继承。
    recipe: Rc<Cell<Option<ProbeRecipe>>>,
}

impl DmabufDevice {
    /// 记录探测结论（platform 构造时灌入；`None` = 未证明可用）。
    pub(crate) fn set_recipe(&self, recipe: Option<ProbeRecipe>) {
        self.recipe.set(recipe);
    }

    /// 探测结论，供各表面初始化 `DmabufBuffers`。
    pub(crate) fn recipe(&self) -> Option<ProbeRecipe> {
        self.recipe.get()
    }

    /// 取共享 device（首次调用时惰性创建）；失败返回 None（调用方回退 SHM）。
    fn get(&self, main_device: Option<libc::dev_t>) -> Option<Rc<Device<File>>> {
        if let Some(d) = self.dev.borrow().as_ref() {
            return Some(d.clone());
        }
        if self.failed.get() {
            return None;
        }
        match open_node_for(main_device) {
            Some((f, path)) => match Device::new(f) {
                Ok(d) => {
                    log::info!(
                        "dmabuf gbm device 就绪（进程共享）：{path}（main_device={:?}）",
                        main_device.map(|v| format!("0x{v:x}"))
                    );
                    let d = Rc::new(d);
                    *self.dev.borrow_mut() = Some(d.clone());
                    Some(d)
                }
                Err(e) => {
                    log::warn!("dmabuf gbm device 打开失败（{path}）：{e}");
                    self.failed.set(true);
                    None
                }
            },
            None => {
                log::warn!("dmabuf 不可用：没有可打开的 DRM 节点");
                self.failed.set(true);
                None
            }
        }
    }
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
    /// 进程级共享 gbm device（多表面共用一个 fd/screen）。
    device: DmabufDevice,
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
    /// 崩溃安全探测**证明可用**的组合（recipe）。`None` = 未探测出可用组合
    /// （或无探测结果）→ 真实提交路径**一律 SHM**，绝不在未验证组合上分配。
    /// 参 `probe_gbm_crash_safe`：只有探测通过的 recipe 才写进这里。
    recipe: Option<ProbeRecipe>,
}

#[allow(clippy::derivable_impls)] // 手写 Default 是为保留「绝不在 Default 里开 gbm」的说明
impl Default for DmabufBuffers {
    fn default() -> Self {
        // ⚠ 绝不能在此打开 gbm device！Shell::new 对**所有**客户端都会构造 DmabufBuffers，
        // 而 gbm::Device::new 在部分 Mesa 驱动上（如 Debian 25.0.x libgallium）会直接段错误
        // （libgallium→libc memcpy 空指针，崩溃栈见 LINUX_DMABUF_PLAN 排障）：这会让每个
        // harness 客户端启动即崩 → 桌面层永不连接 → 纯黑启动遮罩。改为惰性：仅当真正走
        // dmabuf 提交路径时才 init_device()。
        DmabufBuffers {
            device: DmabufDevice::default(),
            width: 0,
            height: 0,
            slots: [None, None],
            next: 0,
            main_device: None,
            formats: Vec::new(),
            unavailable: false,
            chosen: None,
            recipe: None,
        }
    }
}

impl DmabufBuffers {
    /// 灌入进程共享 device 句柄（同进程所有表面共用一份 gbm device）。
    pub(crate) fn set_device(&mut self, device: DmabufDevice) {
        self.device = device;
    }

    /// 合成器 `zwp_linux_dmabuf_feedback_v1` → 主设备 + 格式表（多 GPU 选设备 + 格式校验）。
    pub(crate) fn set_feedback(&mut self, main_device: libc::dev_t, formats: Vec<(u32, u64)>) {
        self.main_device = Some(main_device);
        self.formats = formats;
    }

    /// 灌入崩溃安全探测证明可用的组合。**仅探测通过时调用**；`None` 表示探测未给出
    /// 可用组合 → 后续 `commit` 全部回退（见 `recipe` 字段注释）。
    pub(crate) fn set_probe_recipe(&mut self, recipe: Option<ProbeRecipe>) {
        self.recipe = recipe;
    }

    /// 标记本缓冲池不再走 dmabuf（合成器拒绝过该组合 / 探测失败）→ 调用方回退 SHM。
    pub(crate) fn mark_unavailable(&mut self) {
        self.unavailable = true;
    }

    /// 仅供单测断言用（生产路径只看 `commit` 的返回结果）。
    #[cfg(test)]
    pub(crate) fn is_unavailable(&self) -> bool {
        self.unavailable
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
    /// 选定输出格式。**优先 ABGR8888（免 R/B 交换快路径）**，其次探测 recipe 的 fourcc
    /// （证明可用的分配格式），再次 ARGB8888。`recipe` 决定 fallback 顺序，货真价实的
    /// 前提是「合成器广告了该格式」（反馈表校验，避免导入失败 → 表面不可见）。
    /// 参 `probe_gbm_raw`：recipe 证明 gbm 侧可分配，这里再证明合成器侧可导入。
    fn choose_format(&mut self, recipe: ProbeRecipe) -> Option<FormatChoice> {
        if let Some(c) = self.chosen {
            return Some(c);
        }
        let abgr = Format::Abgr8888 as u32;
        let argb = Format::Argb8888 as u32;
        let recipe_cc = recipe.fourcc() as u32;
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
        } else if advertised(recipe_cc) {
            // 探测证明的组合也被合成器广告 → 用它（R/B 交换按 fourcc 决定）。
            Some(FormatChoice {
                fourcc: recipe.fourcc(),
                swap_rb: recipe_cc != abgr,
            })
        } else if advertised(argb) {
            Some(FormatChoice {
                fourcc: Format::Argb8888,
                swap_rb: true,
            })
        } else {
            log::warn!(
                "dmabuf 回退 SHM：合成器未广告 ARGB8888/ABGR8888/探测组合（+LINEAR/隐式）—— 广告表 {} 项",
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
    /// `recipe` 为崩溃安全探测**证明可用**的组合：分配方式（显式修饰符或 LINEAR 标志）
    /// 与 fourcc 都按它来（未知组合一律由调用方回退 SHM，绝不在此硬编码推断）。
    fn ensure_slots(
        &mut self,
        dev: &Device<File>,
        width: u32,
        height: u32,
        recipe: ProbeRecipe,
    ) -> Option<()> {
        let fresh = self.width != width || self.height != height || self.slots[0].is_none();
        if !fresh {
            return Some(());
        }
        for s in self.slots.iter_mut() {
            *s = None;
        }
        for s in self.slots.iter_mut() {
            let fourcc = recipe.fourcc();
            // LINEAR：保证 CPU 可 mmap 写。GL/drm 需 LINEAR 才允许 gbm_bo_map。
            // 探测证明只能用显式修饰符分配（部分 Mesa 对 LINEAR 标志分配会失败）→ 走
            // `create_buffer_object_with_modifiers(LINEAR)`；否则用历史 LINEAR 标志路径。
            let bo = if recipe.with_modifier {
                dev.create_buffer_object_with_modifiers::<()>(
                    width,
                    height,
                    fourcc,
                    [Modifier::Linear].into_iter(),
                )
            } else {
                dev.create_buffer_object::<()>(
                    width,
                    height,
                    fourcc,
                    BufferObjectFlags::LINEAR,
                )
            }
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
        // 没有探测证明可用的组合 → 一律 SHM（2026-10-04 g1a：只在验证过的组合上启用）。
        let Some(recipe) = self.recipe else {
            return CommitOutcome::Unavailable;
        };
        let expected = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if rgba.len() < expected || width == 0 || height == 0 {
            return CommitOutcome::Skipped;
        }
        let Some(choice) = self.choose_format(recipe) else {
            self.unavailable = true;
            return CommitOutcome::Unavailable;
        };
        // 惰性共享 gbm device：首次真正提交时创建，同进程所有表面复用（SHM 路径不触发）。
        // 按探测 recipe 决定是否只用 feedback 主设备的节点（use_main_dev）。
        let main_dev = if recipe.use_main_dev { self.main_device } else { None };
        let Some(device) = self.device.get(main_dev) else {
            self.unavailable = true;
            return CommitOutcome::Unavailable;
        };
        if self.ensure_slots(&device, width, height, recipe).is_none() {
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
        if let Some(buffer) = self.ensure_buffer(qh, idx, dmabuf, recipe.fourcc()) {
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
//
// 2026-10-04 g1a：本轮把探测升级为「自带诊断、一次上线即定位」：
//  - 阶段面包屑：子进程每进入一个阶段就向**持久**日志追加一行并 `sync_data`，
//    段错误后日志最后一行即死点（旧版只留一个退出码，段错误时全丢）；
//  - 父进程记录信号编号/名字（ExitStatusExt::signal），不再一律记 255；
//  - 备选探测：首选路径失败时按序再试（主设备节点 / 显式修饰符 / XRGB8888），
//    任一通过即记录「哪种组合可用」，真实提交路径按该组合分配。
// 参 Ether docs/STATE_2026-10-02.md §Ⅳ-8、docs/GPU_COMPOSITION_PLAN.md。

/// 探测阶段（子进程退出码 = 阶段号，便于零交互定位失败原因）。
pub const PROBE_OK: u8 = 0;
pub const PROBE_NO_NODE: u8 = 1;
pub const PROBE_DEVICE: u8 = 2;
pub const PROBE_BO: u8 = 3;
pub const PROBE_MAP: u8 = 4;
pub const PROBE_FD: u8 = 5;
/// 探测超时（父进程按不可用处理）。
pub const PROBE_TIMEOUT: u8 = 254;

/// 探测组合（「recipe」）：节点选择 + 分配方式 + fourcc。纯数据，供单测。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProbeRecipe {
    /// 打开 feedback 主设备对应的 render 节点（父进程已知 dev_t 时传入）。
    use_main_dev: bool,
    /// 用 `create_buffer_object_with_modifiers(LINEAR)` 替代 `LINEAR` 标志。
    with_modifier: bool,
    /// fourcc：true = ARGB8888（带 alpha），false = XRGB8888（去 alpha）。
    argb: bool,
}

impl ProbeRecipe {
    /// 首选方案：任意节点 + LINEAR 标志 + ARGB8888（真实提交路径的历史行为）。
    pub(crate) const PREFERRED: ProbeRecipe = ProbeRecipe {
        use_main_dev: false,
        with_modifier: false,
        argb: true,
    };

    /// 四字描述（日志 / 面包屑行首）。
    fn label(&self) -> String {
        format!(
            "{}+{}+{}",
            if self.use_main_dev { "main-dev" } else { "any-node" },
            if self.with_modifier { "modifier" } else { "linear-flag" },
            if self.argb { "ARGB" } else { "XRGB" }
        )
    }

    /// 目标 fourcc。
    fn fourcc(&self) -> Format {
        if self.argb {
            Format::Argb8888
        } else {
            Format::Xrgb8888
        }
    }

    /// 探测尝试序列（按序）：首选 → 主设备节点 → 显式修饰符 → XRGB8888。
    /// 任一通过即取该 recipe（记录进 `DmabufBuffers.recipe`，真实提交按它分配）。
    ///
    /// 「主设备」项仅在父进程已知 feedback dev_t 时尝试（未知则跳过，避免与首选重复）；
    /// 修饰符 / XRGB 两项用任意节点——若主设备节点本身坏了，(b)(c) 换节点才有意义。
    pub(crate) fn try_order() -> [ProbeRecipe; 4] {
        [
            ProbeRecipe::PREFERRED,
            ProbeRecipe {
                use_main_dev: true,
                with_modifier: false,
                argb: true,
            },
            ProbeRecipe {
                use_main_dev: false,
                with_modifier: true,
                argb: true,
            },
            ProbeRecipe {
                use_main_dev: false,
                with_modifier: true,
                argb: false,
            },
        ]
    }

    /// 在 `try_order` 中的序号（子进程经环境变量 `ETHER_DMABUF_PROBE_RECIPE` 接收）。
    fn ordinal(&self) -> u8 {
        ProbeRecipe::try_order()
            .iter()
            .position(|r| r == self)
            .unwrap_or(0) as u8
    }

    /// 由序号重建（配 `ordinal`；越界回退 PREFERRED）。
    fn from_ordinal(n: u8) -> ProbeRecipe {
        ProbeRecipe::try_order()
            .get(n as usize)
            .copied()
            .unwrap_or(ProbeRecipe::PREFERRED)
    }
}

/// 阶段名（日志用）。
pub fn probe_stage_name(code: u8) -> &'static str {
    match code {
        PROBE_OK => "可用",
        PROBE_NO_NODE => "无可用 DRM 节点",
        PROBE_DEVICE => "gbm device 打开失败/崩溃",
        PROBE_BO => "bo 分配失败",
        PROBE_MAP => "bo mmap 写失败",
        PROBE_FD => "bo 导出 dmabuf fd 失败",
        PROBE_TIMEOUT => "探测超时",
        _ => "探测子进程被信号杀死（疑似段错误）",
    }
}

/// 信号编号 → 名字（`fail:sig11:SIGSEGV@…` 用）。未知编号回退 `sig<N>`。
pub fn signal_name(sig: i32) -> String {
    match sig {
        1 => "SIGHUP".into(),
        2 => "SIGINT".into(),
        3 => "SIGQUIT".into(),
        4 => "SIGILL".into(),
        5 => "SIGTRAP".into(),
        6 => "SIGABRT".into(),
        7 => "SIGBUS".into(),
        8 => "SIGFPE".into(),
        9 => "SIGKILL".into(),
        10 => "SIGUSR1".into(),
        11 => "SIGSEGV".into(),
        13 => "SIGPIPE".into(),
        14 => "SIGALRM".into(),
        15 => "SIGTERM".into(),
        17 => "SIGCHLD".into(),
        18 => "SIGCONT".into(),
        19 => "SIGSTOP".into(),
        31 => "SIGSYS".into(),
        other => format!("sig{other}"),
    }
}

/// 探测面包屑日志路径：`$XDG_STATE_HOME/ether/dmabuf-probe.log`，
/// 缺省 `~/.local/state/ether/`（持久，Debian 崩溃后需读盘定位，绝不写 tmpfs）。
/// 参 `AGENTS.md` 铁律：诊断写持久路径。
fn probe_log_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("ether").join("dmabuf-probe.log"))
}

/// 探测面包屑日志（持久、带轮转）。每进入一个阶段追加一行并 `sync_data`，
/// 段错误后最后一行即死点；超过阈值轮转为 `.1`。
struct ProbeLog {
    file: Option<File>,
}

impl ProbeLog {
    /// 打开（追加）默认持久路径。目录不存在则创建；打不开则 `file=None`
    /// （探测不因日志失败而中断）。
    fn open() -> Self {
        let path = probe_log_path().unwrap_or_else(|| {
            // 无 HOME/XDG_STATE_HOME 的极端环境：落到 XDG_RUNTIME_DIR（会话级，尽力而为）。
            std::env::var_os("XDG_RUNTIME_DIR")
                .map(|d| PathBuf::from(d).join("ether-dmabuf-probe.log"))
                .unwrap_or_else(|| PathBuf::from("/tmp/ether-dmabuf-probe.log"))
        });
        Self::open_at(&path)
    }

    /// 在指定路径打开（单测用固定临时路径，避免并发测试争抢进程级环境变量）。
    fn open_at(path: &Path) -> Self {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        rotate_if_big(path);
        let file = File::options()
            .create(true)
            .append(true)
            .open(path)
            .ok();
        ProbeLog { file }
    }

    /// 追加一行。返回是否写出。
    fn append(&mut self, line: &str) -> bool {
        let Some(f) = self.file.as_mut() else {
            return false;
        };
        let mut ok = f.write_all(line.as_bytes()).is_ok();
        ok &= f.write_all(b"\n").is_ok();
        // ⚠ 必须同步到磁盘：段错误后要有这一行（sync_data 容忍失败）。
        let _ = f.sync_data();
        ok
    }
}

/// 日志超阈值 → 轮转为 `.1`（覆盖旧轮转，保持只留一份历史）。
const PROBE_LOG_MAX: u64 = 256 * 1024;

fn rotate_if_big(path: &Path) {
    let Ok(meta) = path.metadata() else {
        return;
    };
    if meta.len() >= PROBE_LOG_MAX {
        let _ = std::fs::rename(path, path.with_extension("log.1"));
    }
}

/// 探测头（每次探测第一行）：时间、可执行名、pid、内核版本、Mesa 版本（libgbm）、
/// 相关环境变量。字面列举，未知项记 `?`，不 panic。
fn probe_header_text() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "?".into());
    let exe = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "?".into());
    let pid = std::process::id();
    // 内核版本：/proc/sys/kernel/osrelease 即 `uname -r`，避免 shell out。
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "?".into());
    let mesa = mesa_version();
    let envs = [
        "GBM_BACKEND",
        "MESA_LOADER_DRIVER_OVERRIDE",
        "LIBGL_ALWAYS_SOFTWARE",
        "LIBGL_DRIVERS_PATH",
    ]
    .into_iter()
    .map(|k| {
        std::env::var(k)
            .map(|v| format!("{k}={v}"))
            .unwrap_or_else(|_| format!("{k}?(unset)"))
    })
    .collect::<Vec<_>>()
    .join(" ");
    format!(
        "[probe] t={now} exe={exe} pid={pid} kernel={kernel} mesa={mesa} {envs}"
    )
}

/// Mesa 版本：扫 `/usr/lib/{arch}/libgbm.so*`（含软链解析），取解析出的库文件名
/// （如 `libgbm.so.1.0.0` → Mesa 版本线索）。找不到返回 `?`。**不 shell out**。
fn mesa_version() -> String {
    for dir in ["/usr/lib/x86_64-linux-gnu", "/usr/lib", "/usr/lib64"] {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if !name.starts_with("libgbm.so") {
                continue;
            }
            let path = e.path();
            // 软链 → 解析到真实文件；文件名字面也能给出版本线索。
            let resolved = std::fs::canonicalize(&path).unwrap_or(path);
            return resolved
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or(name);
        }
    }
    "?".into()
}

/// 子进程入口用：**完整链路烟测** —— 真开 gbm device（会段错误的驱动上直接死掉，这正是
/// 要检测的）→ 分配 bo（按 recipe）→ mmap 写入 → 导出 dmabuf fd，与真实提交路径同款。
/// 每进入一个阶段先写一行**持久面包屑**并 `sync_data`（段错误后最后一行即死点）。
/// 返回阶段码（0 = 可用）。先关 core dump —— 探测失败很可能以 SIGSEGV 结束，绝不能往
/// /var/crash 灌 core。
pub(crate) fn probe_gbm_raw(recipe: ProbeRecipe, main_device: Option<libc::dev_t>) -> u8 {
    use gbm::BufferObjectFlags;
    unsafe {
        let lim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        libc::setrlimit(libc::RLIMIT_CORE, &lim);
    }
    let mut log = ProbeLog::open();
    // 头：时间 / 可执行名 / pid / 内核 / Mesa / 环境变量。
    log.append(&probe_header_text());
    log.append(&format!("[probe] recipe={}", recipe.label()));
    // ⚠ 每个阶段**先写面包屑再做**：段错误后最后一行即「正在进入/执行」的阶段。
    log.append(&format!("[probe] recipe={} stage=entry", recipe.label()));
    fault_die_if_armed("entry");
    let main_dev = if recipe.use_main_dev { main_device } else { None };
    log.append(&format!("[probe] recipe={} stage=open-node", recipe.label()));
    let Some((f, path)) = open_node_for(main_dev) else {
        log.append(&format!("[probe] recipe={} stage=no-node", recipe.label()));
        return PROBE_NO_NODE;
    };
    log.append(&format!(
        "[probe] recipe={} stage=open-node node={path}",
        recipe.label()
    ));
    fault_die_if_armed("open-node");
    // gbm device 创建是 2026-08-19 Debian 段错误所在（driCreateNewScreen3）→ 先落面包屑。
    log.append(&format!("[probe] recipe={} stage=gbm-device", recipe.label()));
    let Ok(dev) = Device::new(f) else {
        log.append(&format!(
            "[probe] recipe={} stage=gbm-device:fail",
            recipe.label()
        ));
        return PROBE_DEVICE;
    };
    fault_die_if_armed("gbm-device");
    // 分配：显式修饰符（DRM_FORMAT_MOD_LINEAR）或 LINEAR 标志，按 recipe。
    log.append(&format!("[probe] recipe={} stage=bo", recipe.label()));
    let bo_res = if recipe.with_modifier {
        dev.create_buffer_object_with_modifiers::<()>(
            64,
            64,
            recipe.fourcc(),
            [Modifier::Linear].into_iter(),
        )
    } else {
        dev.create_buffer_object::<()>(
            64,
            64,
            recipe.fourcc(),
            BufferObjectFlags::LINEAR,
        )
    };
    let Ok(mut bo) = bo_res else {
        log.append(&format!("[probe] recipe={} stage=bo:fail", recipe.label()));
        return PROBE_BO;
    };
    fault_die_if_armed("bo");
    log.append(&format!("[probe] recipe={} stage=map", recipe.label()));
    let mapped = bo.map_mut(0, 0, 64, 64, |mem| {
        if let Some(b) = mem.buffer_mut().first_mut() {
            *b = 0;
        }
    });
    if mapped.is_err() {
        log.append(&format!("[probe] recipe={} stage=map:fail", recipe.label()));
        return PROBE_MAP;
    }
    fault_die_if_armed("map");
    log.append(&format!("[probe] recipe={} stage=fd", recipe.label()));
    if bo.fd().is_err() {
        log.append(&format!("[probe] recipe={} stage=fd:fail", recipe.label()));
        return PROBE_FD;
    }
    fault_die_if_armed("fd");
    log.append(&format!("[probe] recipe={} stage=ok", recipe.label()));
    PROBE_OK
}

/// 环境变量名：父进程 → 子进程的探测参数（子进程是 `current_exe` 重执行，无 Wayland feedback）。
const ENV_PROBE: &str = "ETHER_DMABUF_PROBE";
const ENV_PROBE_RECIPE: &str = "ETHER_DMABUF_PROBE_RECIPE";
const ENV_PROBE_MAIN_DEV: &str = "ETHER_DMABUF_MAIN_DEV";
const ENV_PROBE_FAULT: &str = "ETHER_DMABUF_PROBE_FAULT";

/// 从环境变量读 recipe 序号（父进程写入）。未设置 / 非法返回 None。
fn read_probe_recipe_env() -> Option<ProbeRecipe> {
    std::env::var(ENV_PROBE_RECIPE)
        .ok()
        .and_then(|s| s.parse::<u8>().ok())
        .map(ProbeRecipe::from_ordinal)
}

/// 从环境变量读 feedback 主设备 dev_t（十进制）。未设置 / 非法返回 None。
fn read_main_dev_env() -> Option<libc::dev_t> {
    std::env::var(ENV_PROBE_MAIN_DEV)
        .ok()
        .and_then(|s| s.parse::<libc::dev_t>().ok())
        .filter(|d| *d != 0)
}

/// **探测子进程入口**（由各应用 `main` **第一行**调用；由 `probe_gbm_crash_safe` 拉起）：
/// 若设置了 `ETHER_DMABUF_PROBE`，则按父进程传入的 recipe / main_device 跑完整烟测并
/// `exit(阶段码)`；否则立即返回（正常启动路径）。
///
/// `exit` 是语义必须（`probe_gbm_crash_safe` 按退出码/信号判定可用性），故本函数永不
/// 返回 `!`；正常路径返回后应用继续。**必须插在应用 `main` 第一行**，避免子进程重执行
/// 时先跑应用前半段（日志 / D-Bus / 字体 / wgpu）而把段错误锅扣到 gbm 头上。
pub fn dmabuf_probe_entry() {
    if std::env::var_os(ENV_PROBE).is_none() {
        return;
    }
    let recipe = read_probe_recipe_env().unwrap_or(ProbeRecipe::PREFERRED);
    let main_device = read_main_dev_env();
    let code = probe_gbm_raw(recipe, main_device);
    std::process::exit(code as i32);
}

/// 故障注入（仅测试）：`ETHER_DMABUF_PROBE_FAULT=<stage>` 时在对应阶段
/// `raise(SIGSEGV)`；特殊值 `abort` 在入口 `abort()`（SIGABRT），用于验证信号解码。
/// 生产环境不设此变量。
fn fault_die_if_armed(stage: &str) {
    match std::env::var(ENV_PROBE_FAULT).as_deref() {
        Ok(s) if s == stage => unsafe {
            // Rust 运行时的 SIGSEGV 处理器会吞掉非栈溢出的一次性 `raise`（实测入口注入被
            // 吞、下一阶段才死）→ 先恢复默认处置，保证死点精确落在注入阶段。
            let mut act: libc::sigaction = std::mem::zeroed();
            act.sa_sigaction = libc::SIG_DFL;
            libc::sigaction(libc::SIGSEGV, &act, std::ptr::null_mut());
            libc::raise(libc::SIGSEGV);
        },
        Ok("abort") => unsafe {
            libc::abort();
        },
        _ => {}
    }
}

/// 探测结果缓存路径（会话级 tmpfs：每次登录重新探测，跨会话不残留陈旧结论）。
fn probe_cache_path() -> Option<std::path::PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR").map(|d| std::path::PathBuf::from(d).join("ether-dmabuf-probe"))
}

/// 子进程探测结果（供 `classify_exit` / `probe_gbm_crash_safe`）。
enum ProbeChild {
    /// 退出码 0：该 recipe 完整链路可用。
    Ok,
    /// 失败：诊断串（含信号编号/名字与死点阶段）。
    Failed(String),
}

/// 缓存内容 → recipe：`ok`（旧版，等价首选）/ `ok:<序号>` 为可用；其余为失败。
fn parse_cache(s: &str) -> Option<ProbeRecipe> {
    if s == "ok" {
        return Some(ProbeRecipe::PREFERRED);
    }
    s.strip_prefix("ok:")
        .and_then(|n| n.parse::<u8>().ok())
        .map(ProbeRecipe::from_ordinal)
}

/// 从一行面包屑取死点阶段（`stage=<name>`，副作用字段如 `:fail` 去掉）。纯函数，供单测。
fn stage_from_log_line(line: &str) -> Option<String> {
    let idx = line.find("stage=")?;
    let stage = line[idx + "stage=".len()..]
        .split_whitespace()
        .next()?
        .split(':')
        .next()
        .unwrap_or("?");
    if stage.is_empty() {
        None
    } else {
        Some(stage.to_string())
    }
}

/// 读面包屑日志末行，取死点阶段。供子进程被信号杀死时把死点写进失败串
/// （`fail:sig11:SIGSEGV@gbm-device`）。
fn last_breadcrumb_stage() -> Option<String> {
    let path = probe_log_path()?;
    let text = std::fs::read_to_string(path).ok()?;
    stage_from_log_line(text.lines().last()?)
}

/// 退出状态 → 探测结果。被信号杀死时用 `ExitStatusExt::signal()` 取编号（旧版一律记 255，
/// 丢失信号信息），并从面包屑末行取死点阶段。`stage` 由调用方传入（便于单测注入）。
fn classify_exit_with(status: std::process::ExitStatus, stage: Option<String>) -> ProbeChild {
    if let Some(code) = status.code() {
        if code == PROBE_OK as i32 {
            return ProbeChild::Ok;
        }
        let stage_code = if (0..=u8::MAX as i32).contains(&code) {
            code as u8
        } else {
            u8::MAX
        };
        return ProbeChild::Failed(format!("fail:{code}:{}", probe_stage_name(stage_code)));
    }
    if let Some(sig) = status.signal() {
        let stage = stage.unwrap_or_else(|| "?".into());
        return ProbeChild::Failed(format!("fail:sig{sig}:{}@{stage}", signal_name(sig)));
    }
    ProbeChild::Failed("fail:unknown".into())
}

/// 见 `classify_exit_with`：死点阶段取当前持久面包屑末行。
fn classify_exit(status: std::process::ExitStatus) -> ProbeChild {
    classify_exit_with(status, last_breadcrumb_stage())
}

/// 为单个 recipe 起一个探测子进程（`current_exe` 重执行 + `ETHER_DMABUF_PROBE=1`），
/// 2s 超时按不可用处理并杀掉。`main_device` 经 `ETHER_DMABUF_MAIN_DEV` 传给子进程。
fn run_probe_child(recipe: ProbeRecipe, main_device: Option<libc::dev_t>) -> ProbeChild {
    let Ok(exe) = std::env::current_exe() else {
        return ProbeChild::Failed("fail:no-exe".into());
    };
    let mut cmd = std::process::Command::new(exe);
    cmd.env(ENV_PROBE, "1")
        .env(ENV_PROBE_RECIPE, recipe.ordinal().to_string())
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(d) = main_device {
        cmd.env(ENV_PROBE_MAIN_DEV, d.to_string());
    }
    let Ok(mut child) = cmd.spawn() else {
        return ProbeChild::Failed("fail:spawn".into());
    };
    // 最多等 2s：完整烟测正常 < 200ms；超时按不可用处理并杀掉，
    // 绝不让探测卡住客户端启动（黑屏事故教训：客户端不可用 = 整个桌面不可用）。
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let status;
    loop {
        match child.try_wait() {
            Ok(Some(s)) => {
                status = s;
                break;
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                log::warn!("dmabuf gbm 探测超时（2s）→ 按不可用处理");
                return ProbeChild::Failed(format!(
                    "fail:timeout:{}",
                    probe_stage_name(PROBE_TIMEOUT)
                ));
            }
        }
    }
    classify_exit(status)
}

/// **崩溃安全**探测：gbm 打开在有问题的 Mesa（Debian 2026-08-19 黑屏根因）上会段错误，
/// 段错误无法在进程内捕获 → 在**子进程**里试。返回**探测证明可用**的组合（`None` = 全部
/// 失败 → 调用方一律 SHM，绝不在未验证组合上分配）。
///
/// 按 `ProbeRecipe::try_order()` 顺序逐个 recipe 起子进程（各写自己的面包屑），任一通过即
/// 记录「哪种组合可用」，真实提交路径按它分配。子进程每阶段的持久日志见 `ProbeLog`；
/// 被信号杀死时按 `ExitStatusExt::signal()` 记信号编号/名字与死点阶段。
/// 结果缓存到 `$XDG_RUNTIME_DIR/ether-dmabuf-probe`（`ok:<序号>` / `fail:…`），
/// 同一会话内多个 harness 客户端只探一次。
///
/// `main_device`：调用时若已从合成器 feedback 得知主设备则传入（多 GPU 精确节点）；
/// 未知传 `None`（此时跳过「主设备节点」备选，只试其余组合）。
pub(crate) fn probe_gbm_crash_safe(main_device: Option<libc::dev_t>) -> Option<ProbeRecipe> {
    let cache = probe_cache_path();
    if let Some(p) = cache.as_ref()
        && let Ok(s) = std::fs::read_to_string(p)
    {
        let s = s.trim().to_string();
        let recipe = parse_cache(&s);
        log::info!(
            "dmabuf gbm 探测（缓存）：{}",
            match recipe {
                Some(r) => format!("可用（{}）", r.label()),
                None => format!("不可用（{s}）"),
            }
        );
        return recipe;
    }
    let mut log = ProbeLog::open();
    let mut result: Option<ProbeRecipe> = None;
    let mut last_note = String::from("fail:unknown");
    for recipe in ProbeRecipe::try_order() {
        // 主设备未知时「主设备节点」备选无从谈起（与首选重复）→ 跳过。
        if recipe.use_main_dev && main_device.is_none() {
            continue;
        }
        match run_probe_child(recipe, main_device) {
            ProbeChild::Ok => {
                result = Some(recipe);
                last_note = format!("ok:{} recipe={}", recipe.ordinal(), recipe.label());
                break;
            }
            ProbeChild::Failed(note) => {
                log::warn!("dmabuf gbm 探测失败（{}）：{note}", recipe.label());
                last_note = note;
            }
        }
    }
    // 父进程总结写同一持久日志：一眼看到整轮尝试与结论。
    log.append(&format!("[parent] result={last_note}"));
    let cache_note = match result {
        Some(r) => format!("ok:{}", r.ordinal()),
        None => last_note.clone(),
    };
    if let Some(p) = cache.as_ref() {
        let _ = std::fs::write(p, &cache_note);
    }
    match result {
        Some(r) => log::info!("dmabuf gbm 探测：可用（组合 {}）", r.label()),
        None => log::warn!("dmabuf gbm 探测：不可用（{last_note}）→ 全部表面走 SHM"),
    }
    result
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
    /// g1a：探测 recipe 的 fourcc 作为合成器校验的第二候选（证明 gbm 可分配 + 合成器可导入）。
    #[test]
    fn format_choice_prefers_abgr_then_recipe_then_argb_then_none() {
        let abgr = Format::Abgr8888 as u32;
        let argb = Format::Argb8888 as u32;
        let xrgb = Format::Xrgb8888 as u32;
        let argb_recipe = ProbeRecipe {
            use_main_dev: false,
            with_modifier: false,
            argb: true,
        };
        // 两者都广告 → ABGR8888 + 免交换。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(argb, MOD_LINEAR), (abgr, MOD_INVALID)]);
        let c = b.choose_format(argb_recipe).unwrap();
        assert_eq!(c.fourcc as u32, abgr);
        assert!(!c.swap_rb);
        // 仅 ARGB8888 → 交换。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(argb, MOD_LINEAR)]);
        let c = b.choose_format(argb_recipe).unwrap();
        assert_eq!(c.fourcc as u32, argb);
        assert!(c.swap_rb);
        // 仅 recipe 的 fourcc（这里 XRGB8888）被广告 → 采用 recipe 组合。
        let xrgb_recipe = ProbeRecipe {
            use_main_dev: false,
            with_modifier: false,
            argb: false,
        };
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(xrgb, MOD_LINEAR)]);
        let c = b.choose_format(xrgb_recipe).unwrap();
        assert_eq!(c.fourcc as u32, xrgb);
        assert!(c.swap_rb);
        // recipe 是 ARGB、合成器只广告 XRGB → 都不匹配 → None → 调用方 SHM 回退。
        let mut b = DmabufBuffers::default();
        b.set_feedback(0, vec![(xrgb, MOD_LINEAR)]);
        assert!(b.choose_format(argb_recipe).is_none());
        // 无 feedback（老合成器）→ 沿用 ARGB8888 + 交换（不看 recipe）。
        let mut b = DmabufBuffers::default();
        let c = b.choose_format(argb_recipe).unwrap();
        assert_eq!(c.fourcc as u32, argb);
        assert!(c.swap_rb);
    }

    #[test]
    fn unavailable_short_circuits_commit() {
        let mut b = DmabufBuffers::default();
        b.mark_unavailable();
        assert!(b.is_unavailable());
    }

    /// 信号编号 → 名字解码（旧版被信号杀死一律记 255，丢了信号信息）。
    #[test]
    fn signal_names_decode() {
        assert_eq!(signal_name(11), "SIGSEGV");
        assert_eq!(signal_name(6), "SIGABRT");
        assert_eq!(signal_name(7), "SIGBUS");
        assert_eq!(signal_name(99), "sig99");
    }

    /// 备选组合序列：首选在最前；序号 ↔ recipe 往返；越界回退首选。
    /// 参 `ProbeRecipe::try_order`（g1a 备选探测）。
    #[test]
    fn recipe_order_and_roundtrip() {
        let order = ProbeRecipe::try_order();
        assert_eq!(order[0], ProbeRecipe::PREFERRED);
        for r in order {
            assert_eq!(ProbeRecipe::from_ordinal(r.ordinal()), r);
        }
        assert_eq!(ProbeRecipe::from_ordinal(200), ProbeRecipe::PREFERRED);
        // 显式修饰符（ARGB / XRGB）与主设备节点备选都在序列里。
        assert!(order.iter().any(|r| r.with_modifier && r.argb));
        assert!(order.iter().any(|r| r.with_modifier && !r.argb));
        assert!(order.iter().any(|r| r.use_main_dev));
    }

    /// 缓存解析：旧版 `ok` = 首选；`ok:<序号>` 还原 recipe；失败串 → None。
    #[test]
    fn cache_parse_roundtrip() {
        assert_eq!(parse_cache("ok"), Some(ProbeRecipe::PREFERRED));
        assert_eq!(parse_cache("ok:2"), Some(ProbeRecipe::from_ordinal(2)));
        assert_eq!(parse_cache("fail:sig11:SIGSEGV@gbm-device"), None);
    }

    /// 面包屑死点解析（去副作用后缀）。
    #[test]
    fn stage_from_line_parses() {
        assert_eq!(
            stage_from_log_line("[probe] recipe=x stage=gbm-device").as_deref(),
            Some("gbm-device")
        );
        assert_eq!(
            stage_from_log_line("[probe] recipe=x stage=bo:fail").as_deref(),
            Some("bo")
        );
        assert_eq!(stage_from_log_line("[probe] recipe=x"), None);
    }

    /// 面包屑：append 写盘（含换行）+ 超阈值轮转为 `.1`。
    #[test]
    fn probe_log_appends_and_rotates() {
        let dir = std::env::temp_dir().join(format!("ether-probe-log-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("dmabuf-probe.log");
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(dir.join("dmabuf-probe.log.1"));
        {
            let mut log = ProbeLog::open_at(&path);
            assert!(log.append("[probe] stage=entry"));
            assert!(log.append("[probe] stage=open-node"));
        }
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("stage=entry"));
        assert_eq!(
            stage_from_log_line(text.lines().last().unwrap()).as_deref(),
            Some("open-node")
        );
        // 轮转：写超阈值 → open_at 触发 rename 为 `.1`，并新建当前文件。
        std::fs::write(&path, vec![b'x'; PROBE_LOG_MAX as usize + 1]).unwrap();
        let _log2 = ProbeLog::open_at(&path);
        assert!(dir.join("dmabuf-probe.log.1").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 子进程辅助测试：正常 `cargo test` 下 `ETHER_DMABUF_PROBE` 未设 → `dmabuf_probe_entry`
    /// 立即返回（空测试通过）；父测试带环境变量拉起本测试，令其在探测入口 `abort()`。
    #[test]
    fn probe_entry_child_helper() {
        dmabuf_probe_entry();
    }

    /// 故意 `abort()` 的子进程 → 父进程用 `ExitStatusExt::signal()` 解出 SIGABRT 并写进诊断串
    /// （此前被信号杀死一律记 255，丢失信号）。参 g1a 任务「信号解码」验收。
    #[test]
    fn signal_decode_from_real_abort_child() {
        let exe = std::env::current_exe().unwrap();
        let dir = std::env::temp_dir().join(format!("ether-probe-abort-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let status = std::process::Command::new(&exe)
            .args(["--exact", "dmabuf::tests::probe_entry_child_helper", "--nocapture"])
            .env(ENV_PROBE, "1")
            .env(ENV_PROBE_RECIPE, "0")
            .env(ENV_PROBE_FAULT, "abort")
            .env("XDG_STATE_HOME", &dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(libc::SIGABRT),
            "子进程应被 SIGABRT 杀死"
        );
        match classify_exit_with(status, Some("entry".into())) {
            ProbeChild::Failed(note) => {
                assert_eq!(note, "fail:sig6:SIGABRT@entry", "诊断串格式须含信号+死点");
            }
            ProbeChild::Ok => panic!("被信号杀死不应判为可用"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 故障注入（SIGSEGV）：子进程在入口 `raise(SIGSEGV)` → 持久面包屑文件末行即死点；
    /// 父侧解出 `sig11`/`SIGSEGV` 与死点阶段。参 g1a 任务验收第 3 条（只验诊断链路，
    /// 与真机 gbm 是否可用无关——注入点选在打开节点之前）。
    #[test]
    fn segv_child_records_breadcrumb_and_signal() {
        let exe = std::env::current_exe().unwrap();
        let dir = std::env::temp_dir().join(format!("ether-probe-segv-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let status = std::process::Command::new(&exe)
            .args(["--exact", "dmabuf::tests::probe_entry_child_helper", "--nocapture"])
            .env(ENV_PROBE, "1")
            .env(ENV_PROBE_RECIPE, "0")
            .env(ENV_PROBE_FAULT, "entry")
            .env("XDG_STATE_HOME", &dir)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert_eq!(
            std::os::unix::process::ExitStatusExt::signal(&status),
            Some(libc::SIGSEGV)
        );
        // 持久面包屑：`$XDG_STATE_HOME/ether/dmabuf-probe.log` 末行即死点阶段。
        let log = dir.join("ether").join("dmabuf-probe.log");
        let text = std::fs::read_to_string(&log).expect("面包屑日志须落盘");
        assert!(text.contains("[probe] t="), "头部（时间/环境）须写入");
        assert_eq!(
            stage_from_log_line(text.lines().last().unwrap()).as_deref(),
            Some("entry"),
            "末行须是死点阶段"
        );
        match classify_exit_with(status, Some("entry".into())) {
            ProbeChild::Failed(note) => {
                assert_eq!(note, "fail:sig11:SIGSEGV@entry");
            }
            ProbeChild::Ok => panic!("被信号杀死不应判为可用"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
